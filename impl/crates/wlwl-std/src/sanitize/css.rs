//! [v0.11.3 M7 / addendum-13 W-03] `HTML_SANITIZE_CSS` 的白名单抽取内核。
//!
//! ## 为什么是**白名单**而不是转义(规范 §13.5 · `ADR-0030` D1)
//!
//! OWASP 的 CSS 上下文指引第一句是「**allowlist it**(例如固定的一组颜色
//! token)」,CSS-escape 是**补充**而不是替代 —— 这与 §13.3 否掉 SQL 转义是
//! 同一条纪律:转义垫底、白名单当第一防线。⇒ 实现顺序上 `HTML_SANITIZE_CSS`
//! 先于两个 `CSS_ESCAPE_*`。
//!
//! ## 两条分开的失败口径
//!
//! * **不在白名单** → **静默丢弃**,产出里看不到它。这是白名单**正常工作**
//!   的样子,不是失败。
//! * **命中拒绝构造**(`@import` / `expression(` / `javascript:` /
//!   `behavior` / `-moz-binding` / 任意 `url(`)→ 整个输入被判为攻击,
//!   报 `ERR([kind: SanitizeError])`。让调用方看见「这**是**攻击」与
//!   「只是没匹配上」是两回事。
//!
//! ## 解析范围(与计划的非目标一致)
//!
//! 只到**声明级**:`prop: value` 对。**不做选择器语义、不做值语法解析。**
//! 但拒绝构造的扫描是**文本级**的(先扫全文再解析)—— 藏在注释外、
//! 畸形声明里、`url(data:...;base64,...)` 的分号之后的攻击,都不能靠
//! 「解析不出来就丢掉」蒙混过去。

/// 允许的 CSS 属性(小写比较)。
///
/// 选取口径:**不能引用外部资源、不能执行、不能影响布局之外的渲染管线**。
/// 凡是能取 `url()` 的(`background` / `background-image` / `cursor` /
/// `content` / `list-style-image` / `filter` / `src` / `border-image`)
/// **一律不在列** —— 即便值侧再查一遍,也不靠单点防御。
pub(crate) const ALLOWED_PROPS: &[&str] = &[
    // 颜色
    "color",
    "background-color",
    "border-color",
    "border-top-color",
    "border-right-color",
    "border-bottom-color",
    "border-left-color",
    "outline-color",
    // 边框(不含 image)
    "border",
    "border-top",
    "border-right",
    "border-bottom",
    "border-left",
    "border-width",
    "border-style",
    "border-radius",
    "border-top-width",
    "border-right-width",
    "border-bottom-width",
    "border-left-width",
    "border-top-style",
    "border-right-style",
    "border-bottom-style",
    "border-left-style",
    "border-top-left-radius",
    "border-top-right-radius",
    "border-bottom-left-radius",
    "border-bottom-right-radius",
    "outline",
    "outline-width",
    "outline-style",
    // 盒模型
    "width",
    "height",
    "min-width",
    "min-height",
    "max-width",
    "max-height",
    "margin",
    "margin-top",
    "margin-right",
    "margin-bottom",
    "margin-left",
    "padding",
    "padding-top",
    "padding-right",
    "padding-bottom",
    "padding-left",
    "box-sizing",
    "display",
    "visibility",
    "overflow",
    "overflow-x",
    "overflow-y",
    "vertical-align",
    // 排版
    "font-family",
    "font-size",
    "font-style",
    "font-weight",
    "font-variant",
    "line-height",
    "letter-spacing",
    "word-spacing",
    "text-align",
    "text-decoration",
    "text-decoration-line",
    "text-decoration-color",
    "text-decoration-style",
    "text-indent",
    "text-transform",
    "white-space",
    "word-break",
    "overflow-wrap",
    "list-style-type",
    "direction",
    // 其他无副作用的
    "opacity",
    "z-index",
    "table-layout",
    "caption-side",
    "empty-cells",
    "resize",
];

/// 拒绝构造(文本级,先于解析扫描)。一律**小写**比较。
///
/// ⚠ `url(` 无条件拒 —— 不做「URL 看起来安全就放行」的判断。那需要解析
/// `url()` 的完整语法并处理 `data:` 的 MIME 白名单,是**另一个成员**的事;
/// 在这里它只会变成一个看起来能用的漏洞。
const REJECTED_TOKENS: &[&str] = &[
    "@import",
    "expression(",
    "javascript:",
    "vbscript:",
    "behavior:",
    "-moz-binding",
    "url(",
];

/// `HTML_SANITIZE_CSS(text) -> STRING` 的核心。
///
/// `Err(())` = 命中拒绝构造(攻击);`Ok(清洗后的声明串)` = 正常,可能为空。
pub(crate) fn sanitize_css(text: &str) -> Result<String, ()> {
    // ① 先做文本级拒绝扫描 —— **在**剥注释与解析之前。剥完注释再扫会让
    //    「注释里藏一个真声明」这类写法绕过;扫完再剥则注释里的 `url(` 会
    //    造成误拒。取舍:误拒只损失一次调用,漏放是漏洞 ⇒ 先扫原文。
    let lowered = text.to_ascii_lowercase();
    if REJECTED_TOKENS.iter().any(|t| lowered.contains(t)) {
        return Err(());
    }
    // ② 剥注释。`/* … */`;未闭合的按 CSS 规则吃到结尾(浏览器同款)。
    let stripped = strip_comments(text);
    // ③ 声明级切分。
    let mut out: Vec<String> = Vec::new();
    for (prop, value) in split_declarations(&stripped) {
        let prop_l = prop.trim().to_ascii_lowercase();
        if !ALLOWED_PROPS.contains(&prop_l.as_str()) {
            continue; // 白名单正常工作:静默丢弃
        }
        let value = value.trim();
        if value.is_empty() {
            continue;
        }
        out.push(format!("{prop_l}: {value}"));
    }
    // 什么都没有存活 ⇒ **空串**,不是 `";"`。空串能被 `==` 与拼接安全处理;
    // 一个孤零零的分号会让「无输出」看起来像坏掉的声明列表。
    if out.is_empty() {
        return Ok(String::new());
    }
    // 声明之间**必须**有 `;` —— `color: red font-weight: bold` 不是合法 CSS
    // (尾随 `;` 可省,分隔符不可省)。这条曾被写成 `" "` 连接,产出不可用的
    // 样式表。
    Ok(format!("{};", out.join("; ")))
}

fn strip_comments(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let b: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < b.len() {
        if b[i] == '/' && i + 1 < b.len() && b[i + 1] == '*' {
            i += 2;
            while i + 1 < b.len() && !(b[i] == '*' && b[i + 1] == '/') {
                i += 1;
            }
            // 未闭合 ⇒ 跳过到结尾(浏览器同款)。
            i = (i + 2).min(b.len());
            out.push(' ');
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    out
}

/// 切分成 `(属性, 值)` 对。分号与冒号都**不在引号内、不在圆括号内**才算分隔符。
///
/// ⚠ 括号感知是必需的:`url(data:image/png;base64,AAA)` 里有分号,
/// `content: "a;b"` 里有分号,天真 split 会把它们切碎。
fn split_declarations(s: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    let chars: Vec<char> = s.chars().collect();
    let mut start = 0usize;
    let mut i = 0usize;
    let mut depth = 0i32;
    let mut quote: Option<char> = None;
    while i < chars.len() {
        let c = chars[i];
        match quote {
            Some(q) => {
                if c == q {
                    quote = None;
                }
            }
            None => match c {
                '"' | '\'' => quote = Some(c),
                '(' => depth += 1,
                ')' => depth -= 1,
                ';' if depth == 0 => {
                    push_decl(&chars[start..i], &mut out);
                    start = i + 1;
                }
                _ => {}
            },
        }
        i += 1;
    }
    push_decl(&chars[start..], &mut out);
    out
}

fn push_decl(chunk: &[char], out: &mut Vec<(String, String)>) {
    if chunk.iter().all(|c| c.is_whitespace()) {
        return;
    }
    // 第一个**不在括号 / 引号内**的冒号才是分隔符。
    let mut depth = 0i32;
    let mut quote: Option<char> = None;
    let mut colon = None;
    for (i, &c) in chunk.iter().enumerate() {
        match quote {
            Some(q) => {
                if c == q {
                    quote = None;
                }
            }
            None => match c {
                '"' | '\'' => quote = Some(c),
                '(' => depth += 1,
                ')' => depth -= 1,
                ':' if depth == 0 => {
                    colon = Some(i);
                    break;
                }
                _ => {}
            },
        }
    }
    let Some(pos) = colon else {
        // 没有冒号 = 不是声明,丢弃。
        return;
    };
    out.push((
        chunk[..pos].iter().collect(),
        chunk[pos + 1..].iter().collect(),
    ));
}
