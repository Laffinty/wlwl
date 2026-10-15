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
//!
//! ## 输出侧也要硬化(§13.5 · `D13-022`)
//!
//! 白名单管的是「哪些声明能活」,不管「活下来的值能不能越出它在 CSS / HTML
//! 里的位置」。`HTML_SANITIZE_SCRIPT` 侧有 `harden_for_script`,CSS 侧本来
//! 没有 —— 输出可含裸 `</style>`,嵌进 `<style>` 即破壳(N-018)。见
//! [`harden_for_css`]。

use super::escape_codepoint;

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
        // [D13-022 · N-018] 声明值**必须**加固后才能出本函数。属性名不用:
        // `prop_l` 命中 `ALLOWED_PROPS`(只含 `[a-z-]`),天然无元字符。
        out.push(format!("{prop_l}: {}", harden_for_css(value)));
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

/// [D13-022 · N-018] 把存活声明的**值**加固成「可安全内联进 CSS」的形式。
///
/// ## 为什么不能照抄 `harden_for_script` 的 `\u003C`
///
/// JS 字符串字面量的转义形状是 `\uXXXX`,CSS **没有这个形状** —— CSS 的
/// 转义是「反斜杠 + 十六进制码位」(CSS Syntax §4.3.7 *Consume an escaped
/// code point*:1–6 位十六进制 + 一个可选的空白消歧符)。照抄 `\u003C` 在 CSS
/// 里只是标识符文本 `u003C`,`<` **原样留在输出里**,破壳照旧。
/// 转义形状统一走 CSSOM §"Common Serializing Idioms" 的
/// *escape a character as code point*(小写十六进制 + **一个空格**),与本模块
/// `CSS_ESCAPE_IDENT` / `CSS_ESCAPE_STRING` 复用 [`super::escape_codepoint`]
/// —— 一份规则两处用,免得两套十六进制写法在同一个样式表里打架。
///
/// ## 输出必须同时在**四种**上下文里成立
///
/// 白名单只保证「值语法无害」,不保证「值不能越出它在 CSS / HTML 里的
/// 位置」。调用方可能把它放进 `<style>…</style>`、`style="…"`、CSS 字符串,
/// 甚至 `/* … */` 里,故四条都要成立:
///
/// 1. **HTML raw text(`<style>` 内)**:HTML 分词器只靠 `</style` 转移出
///    raw text。输出里**一个 `<` 都没有** ⇒ 那个序列无从拼出 ⇒ 任何输入都
///    不能终止 `<style>` 元素(与 §13.4 的 `harden_for_script` 同一判据)。
/// 2. **CSS 字符串**:`"` `'` 与裸换行(CR / LF / FF)都能终止字符串 ⇒ 全转。
/// 3. **CSS 注释**:能提前闭合的是 `*/`。只转 `*` 就够 —— 序列里少了未转义的
///    `*`,`*/` 就**拼不出来**;`/` 反而是合法值里的常见字符(`font: 12px/1.5`
///    族),转它没有收益。
/// 4. **CSS 规则体**(调用方写 `p { …输出… }`):`;` 是声明分隔符,`{` `}` 能
///    结束整条规则并另起一条 —— 攻击者借此把**任意**声明塞进样式表。
///
/// ## ⚠ `\` 必须与本文插入的转义**一起**处理
///
/// CSS 里 `\` 后面跟十六进制才是转义。若不先处理输入自带的 `\`,它会与本函数
/// 插入的 `\3c ` 连成另一个序列(`\\3c ` 被读成「字面反斜杠 + 标识符 `3c`」),
/// 转义就不再是「值的逐字映射」,正确性论证也随之失效。转义 `\` 之后,
/// 攻击者的反斜杠在 CSS 里**只是个字符**,造不出新的十六进制转义。
/// 副作用(有意的取舍):调用方自己写的嵌套 CSS 转义(中文自定义字体名
/// `\5FAE\8BA4` 一族)会被当数据,字体名退化为不匹配 → 回退字体。
/// 对**不可信数据**而言,嵌套转义语法正是 N-017 的攻击面,不能留给它。
fn harden_for_css(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 16);
    for ch in s.chars() {
        match ch {
            // ① 标记上下文:`</style` 的 `<` 消不掉就没有破壳;`>` 一并转是
            //    OWASP 同款清单,顺带封掉 `-->` 类注释提前闭合的读法。
            '<' | '>' => escape_codepoint(ch, &mut out),
            // ② CSS 字符串:`"` `'` 终止字符串,裸换行把它变成 bad-string
            //    token(同样等于闭合)。⚠ 这些在合法值里本就不该出现,
            //    转义它们等于**修好**畸形输入。
            '"' | '\'' | '\n' | '\r' | '\u{c}' => escape_codepoint(ch, &mut out),
            // ③ CSS 注释:只转 `*`,让 `*/` 无法拼出。
            '*' => escape_codepoint(ch, &mut out),
            // ④ 规则体:分隔符与块界。白名单属性的合法值里三者都不出现,
            //    转义没有功能代价(不转才有)。
            ';' | '{' | '}' => escape_codepoint(ch, &mut out),
            // ⑤ 见上文的 `\` 论证。
            '\\' => escape_codepoint(ch, &mut out),
            other => out.push(other),
        }
    }
    out
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

#[cfg(test)]
mod tests {
    use super::*;

    /// [D13-022 · N-018] 转义形状 = CSSOM *escape a character as code point*:
    /// 反斜杠 + **小写**十六进制 + **一个空格**(与 `CSS_ESCAPE_IDENT` 同一
    /// 份规则,两处不各写一套)。
    #[test]
    fn hex_escape_shape_follows_cssom() {
        assert_eq!(harden_for_css("a<b"), r"a\3c b");
        assert_eq!(harden_for_css("a>b"), r"a\3e b");
        assert_eq!(harden_for_css(r"a\b"), r"a\5c b");
        assert_eq!(harden_for_css("a\"b"), r"a\22 b");
        assert_eq!(harden_for_css("a'b"), r"a\27 b");
        assert_eq!(harden_for_css("a*b"), r"a\2a b");
        assert_eq!(harden_for_css("a;b"), r"a\3b b");
        assert_eq!(harden_for_css("a{b"), r"a\7b b");
        assert_eq!(harden_for_css("a}b"), r"a\7d b");
        assert_eq!(harden_for_css("a\nb"), r"a\a b");
        assert_eq!(harden_for_css("a\rb"), r"a\d b");
        assert_eq!(harden_for_css("a\u{c}b"), r"a\c b");
        // 非转义集原样。
        assert_eq!(harden_for_css("red 1px / 2px"), "red 1px / 2px");
    }

    /// 复核样本:修复前 `</style>` 原样穿过本函数。
    #[test]
    fn css_output_cannot_terminate_a_style_element() {
        let out = sanitize_css("color: red</style><img src=x onerror=alert(1)>").unwrap();
        assert!(!out.contains('<'), "裸 `<` 留在输出里: {out}");
        assert_eq!(
            out,
            r"color: red\3c /style\3e \3c img src=x onerror=alert(1)\3e ;"
        );
    }

    /// ⚠ 攻击者自带的 `\` 若不先转义,会与插入的 `\3c ` 连成别的转义序列 ——
    /// 这条钉住「先处理 `\`」这件事本身,而不只是转义集里有个 `\`。
    #[test]
    fn attacker_backslash_cannot_swallow_the_inserted_escape() {
        assert_eq!(harden_for_css(r"a\</style>"), r"a\5c \3c /style\3e ");
        // 转义之后,`\` 在 CSS 里只是个字面字符,造不出新的十六进制转义。
        assert_eq!(harden_for_css(r"e\78 pression(x)"), r"e\5c 78 pression(x)");
    }

    /// CSS 注释上下文:`*/` 在输出里**拼不出来**(只转 `*`,不转 `/`)。
    #[test]
    fn css_output_cannot_close_a_comment() {
        assert_eq!(harden_for_css("*/"), r"\2a /");
        assert!(!sanitize_css("color: a*/").unwrap().contains("*/"));
    }
}
