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
//! ## 拒绝扫描扫**两遍**:原文 + 按 CSS 转义解码后(N-017)
//!
//! `\` 加十六进制是 CSS 自己的转义(CSS Syntax §4.3.7),浏览器读到的
//! `e\78 pression(` 就是 `expression(`。只扫原文,等于让攻击者自己决定要不要
//! 写转义 ⇒ 拒绝扫描额外扫一遍**按规范解码后**的文本。
//!
//! ⚠ **两遍都不能少**:原文那一遍挡住「注释里藏一个真声明」(剥注释后它就没
//! 了),解码那一遍挡住转义绕过。解码结果**只**用于扫描,不参与输出。
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
///
/// ⚠ [N-017] 这张表比较的是**已解码**的文本,所以表里的每个 token 都有**两种**
/// 拼法(字面 / CSS 转义)要扫,见 [`sanitize_css`] 第①步。
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
    //
    //    [N-017] 原文那一遍之后**再扫一遍按 CSS 转义解码后的文本**:只扫原文
    //    等于让攻击者自己决定要不要写转义(`e\78 pression(` 就是
    //    `expression(`)。两遍的并集才是拒绝域 —— 误拒方向可接受、漏放不可接受,
    //    与上面同一取舍。解码只服务本步,后面的解析与输出仍吃**原文**。
    if hits_rejected_token(text) {
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

/// [N-017] 拒绝扫描 = **原文** ∪ **按 CSS 转义解码后的原文**。
///
/// 两遍都做小写化后逐个 token 子串比较(表里全是 ASCII,故 `to_ascii_lowercase`
/// 足够,也与既有口径一致)。
fn hits_rejected_token(text: &str) -> bool {
    let lowered = text.to_ascii_lowercase();
    if REJECTED_TOKENS.iter().any(|t| lowered.contains(t)) {
        return true;
    }
    // 没有 `\` ⇒ 不存在 CSS 转义,解码是恒等映射 ⇒ 不必再分配一遍文本。
    // ⚠ 这条快路径是**纯优化**:它成立的前提是解码不会改动不含 `\` 的串。
    if !text.contains('\\') {
        return false;
    }
    let decoded = decode_css_escapes(text).to_ascii_lowercase();
    REJECTED_TOKENS.iter().any(|t| decoded.contains(t))
}

/// [N-017] 按 **CSS Syntax Level 3 §4.3.7 *Consume an escaped code point*** 把
/// CSS 转义解码成浏览器看到的码位。
///
/// 规范出处(W3C CSS Syntax Module Level 3,CRD 2026-10-01,
/// <https://www.w3.org/TR/css-syntax-3/>):
/// <https://www.w3.org/TR/css-syntax-3/#consume-escaped-code-point>。逐条:
///
/// - 前提:「It assumes that the U+005C REVERSE SOLIDUS (`\`) has already been
///   consumed and that the next input code point has already been verified to be
///   part of a valid escape.」—— 合法与否由 §4.3.8 *Check if two code points are a
///   valid escape* 判:「if the second code point is a newline, return false.」
/// - **hex digit 分支**(§4.2:digit ∪ `A-F` ∪ `a-f`):「Consume as many hex digits
///   as possible, but no more than 5. **Note that this means 1-6 hex digits have
///   been consumed in total.**」「**If the next input code point is whitespace,
///   consume it as well.**」⇒ 那个尾随空白是十六进制位数的**消歧符**,被吃掉,
///   于是 `\65 xpression` 解成 `expression`。
/// - 「Interpret the hex digits as a hexadecimal number. If this number is zero,
///   or is for a surrogate, or is greater than the maximum allowed code point,
///   return U+FFFD REPLACEMENT CHARACTER.」maximum allowed code point = U+10FFFF
///   (§4.2)。
/// - **EOF 分支**：「This is a parse error. Return U+FFFD REPLACEMENT CHARACTER.」
/// - **anything else 分支**：「Return the current input code point.」
///   ⚠ 这一支**不再吃尾随空白** —— 规范只在 hex 分支写了那句 consume。非规范性
///   的 §2.1 *Escaping* 同一口径:「One to six hex digits, followed by an optional
///   whitespace.」与「Any Unicode code point that is not a hex digits or a
///   newline. The escape sequence is replaced by that code point.」
///
/// ## 两个 §4.3.7 前提不成立的角落
///
/// 1. **末尾孤立的 `\`**(后面没有第二个码位):不构成 §4.3.8 的 valid escape,
///    §4.3.7 的前提不成立 ⇒ **原样保留**。丢掉它只会把两侧字符合并,对拒绝扫描
///    没有任何好处。
/// 2. **`\` + 换行**:§4.3.8 明写这种情况不是 valid escape ⇒ 两个字节都留着,
///    不合并。newline 按 §4.2 只含 U+000A,但 §3.3 *Preprocessing the input
///    stream* 把 `CR` `FF` `CRLF` 都折成 `LF` ⇒ 原始串里的 `\`+`CR` / `\`+`FF`
///    同样进不了转义分支。
///
/// ## 只用于扫描,绝不用于输出
///
/// 输出侧是另一套(`harden_for_css` 把 `\` 转义掉)。本函数的解码结果**只**
/// 喂给拒绝扫描:让转义绕过在**扫描这一层**失效,与「输出能不能越位」是两个
/// 独立问题。
fn decode_css_escapes(s: &str) -> String {
    let b: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0usize;
    while i < b.len() {
        if b[i] != '\\' {
            out.push(b[i]);
            i += 1;
            continue;
        }
        // 见上文「两个 §4.3.7 前提不成立的角落」①②。
        if i + 1 >= b.len() || is_css_newline(b[i + 1]) {
            out.push('\\');
            if i + 1 < b.len() {
                out.push(b[i + 1]);
                i += 2;
            } else {
                i += 1;
            }
            continue;
        }
        if is_hex_digit(b[i + 1]) {
            // 「1-6 hex digits in total」(反斜杠本身已消费一位之后还允许 5 位)
            let mut value: u32 = 0;
            let mut digits = 0usize;
            let mut j = i + 1;
            while j < b.len() && digits < 6 && is_hex_digit(b[j]) {
                // ⚠ 循环条件里的 `digits < 6` 就是那句「no more than 5」的落点:
                // 写成 `< 7` 会把 7 位十六进制也吞掉,第 7 位不是规范的一部分。
                value = value * 16 + b[j].to_digit(16).unwrap_or(0);
                digits += 1;
                j += 1;
            }
            // 规范原文:"If the next input code point is whitespace, consume it as well."
            if j < b.len() && is_css_whitespace(b[j]) {
                j += 1;
            }
            out.push(code_point_from_hex(value));
            i = j;
            continue;
        }
        // "Return the current input code point." ⚠ **不**消费尾随空白。
        out.push(b[i + 1]);
        i += 2;
    }
    out
}

/// §4.3.7 的收尾三档:0 / 代理区 / 超过 U+10FFFF ⇒ U+FFFD。
fn code_point_from_hex(value: u32) -> char {
    if value == 0 || (0xd800..=0xdfff).contains(&value) || value > 0x10_ffff {
        '\u{fffd}'
    } else {
        // 已在上面的三档里滤掉非法值,`from_u32` 只可能因为同一个原因失败。
        char::from_u32(value).unwrap_or('\u{fffd}')
    }
}

/// §4.2 *Definitions*:hex digit = digit ∪ `A-F` ∪ `a-f`。
fn is_hex_digit(c: char) -> bool {
    // ⚠ `is_ascii_hexdigit` 覆盖的正是 `0-9` `A-F` `a-f` 三段,与 §4.2 的
    // hex digit 定义逐字一致(不是「能当数字用」的宽松读法)。
    c.is_ascii_hexdigit()
}

/// §4.2 *Definitions*:whitespace = newline ∪ U+0009 ∪ U+0020。
fn is_css_whitespace(c: char) -> bool {
    is_css_newline(c) || c == '\t' || c == ' '
}

/// §4.2 *Definitions*:newline = U+000A(CR / FF 由 §3.3 预处理折成 LF)。
fn is_css_newline(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\u{c}')
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

    /// [N-017] 解码逐条照 **CSS Syntax §4.3.7 *Consume an escaped code point***
    /// (CRD 2026-10-01,`css-syntax-3/#consume-escaped-code-point`)。
    ///
    /// 这些期望值是**规范原文推出来的**,不是「跑一遍看解出什么」:
    /// 「Consume as many hex digits as possible, but no more than 5. Note that
    /// this means 1-6 hex digits have been consumed in total.」+「If the next
    /// input code point is whitespace, consume it as well.」
    #[test]
    fn escape_decoding_follows_css_syntax_4_3_7() {
        // ① hex 分支:最长十六进制序列 + **一个**尾随空白被消费。
        assert_eq!(decode_css_escapes(r"e\78 pression(x)"), "expression(x)");
        assert_eq!(decode_css_escapes(r"\65 xpression"), "expression");
        // ⚠ 空格只吃**一个**:第二个空格是普通字符(所以解出来是 `ae b` 而不是
        // `aeb b` —— 少写这条就很容易在这里写下想当然的期望值)。
        assert_eq!(decode_css_escapes(r"a\65  b"), "ae b");
        // 制表符同样是 §4.2 的 whitespace。
        assert_eq!(decode_css_escapes("a\\65\tb"), "aeb");
        // 没有尾随空白就不消费下一个字符(它不是 whitespace ⇒ 直接留给下一轮)。
        assert_eq!(decode_css_escapes(r"\65xpression"), "expression");
        // ⚠ **6 位封顶**:第 7 位不是十六进制序列的一部分,留在后面。
        assert_eq!(decode_css_escapes(r"\000065xpression"), "expression");
        assert_eq!(decode_css_escapes(r"\0000065xpression"), "\u{6}5xpression");
        // 大写十六进制同样是 hex digit(§4.2 明写 `A-F`)。
        assert_eq!(decode_css_escapes(r"\65 XPRESSION"), "eXPRESSION");
        // ② 0 / 代理区 / 超过 U+10FFFF ⇒ U+FFFD(§4.3.7 + §4.2)。
        assert_eq!(decode_css_escapes(r"\0"), "\u{fffd}");
        assert_eq!(decode_css_escapes(r"\d800"), "\u{fffd}");
        assert_eq!(decode_css_escapes(r"\110000"), "\u{fffd}");
        assert_eq!(decode_css_escapes(r"\10ffff"), "\u{10ffff}");
        // ③ anything else:「Return the current input code point.」⚠ **不**吃尾随
        //    空白(规范只在 hex 分支有那句 consume)—— 少这个 `⚠` 就会把
        //    `\  x` 解成 ` x`,凭空合并两个字符。
        assert_eq!(decode_css_escapes(r"\url(x)"), "url(x)");
        assert_eq!(decode_css_escapes(r"\  x"), "  x");
        assert_eq!(decode_css_escapes(r"\-moz"), "-moz");
        // ④ `\` + 换行**不是** valid escape(§4.3.8),两个字节都留着。
        assert_eq!(decode_css_escapes("a\\\nb"), "a\\\nb");
        // ⚠ §4.2 只把 U+000A 算 newline,但 §3.3 预处理把 CR / FF 折成 LF ⇒
        // 原始串里这两种同样进不了转义分支。
        assert_eq!(decode_css_escapes("a\\\rb"), "a\\\rb");
        assert_eq!(decode_css_escapes("a\\\u{c}b"), "a\\\u{c}b");
        // ⑤ 末尾孤立的 `\`:§4.3.7 前提不成立 ⇒ 原样保留,不被丢掉。
        assert_eq!(decode_css_escapes("a\\"), "a\\");
    }

    /// [N-017] 复核样本:修复前 `e\78 pression(alert(1))` 只扫原文 ⇒ 放行。
    #[test]
    fn css_escape_spelling_cannot_bypass_the_rejection_scan() {
        assert!(sanitize_css(r"width: e\78 pression(alert(1))").is_err());
        assert!(sanitize_css(r"width: \65 xpression(alert(1))").is_err());
        assert!(sanitize_css(r"width: \000065xpression(alert(1))").is_err());
        assert!(sanitize_css(r"@import \75 rl(evil.css)").is_err());
        assert!(sanitize_css(r"color: \javascript:alert(1)").is_err());
        assert!(sanitize_css(r"color: j\61 vascript:alert(1)").is_err());
        assert!(sanitize_css(r"color: vb\73 cript:msgbox(1)").is_err());
        assert!(sanitize_css(r"\62 ehavior: \75 rl(#default#time2)").is_err());
        assert!(sanitize_css(r"-\6d oz-binding: anything").is_err());
        // 注释里藏的转义版同样被拒 —— 原文那一遍与解码那一遍是**并集**。
        assert!(sanitize_css(r"color: /* \75 rl(x) */ red").is_err());
    }

    /// [N-017] 反向守卫:解码**不得**把白名单里的正常值判成攻击。
    ///
    /// 这条钉的是「不误拒」方向 —— 上一条的解码若写成「一律吃尾随空白」或
    /// 「只吃一位十六进制」,这一条不会红;所以两条必须一起看。
    #[test]
    fn escape_decoding_does_not_reject_legitimate_values() {
        assert_eq!(
            sanitize_css(r#"font-family: "\5FAE\8BA4""#).unwrap(),
            r#"font-family: \22 \5c 5FAE\5c 8BA4\22 ;"#
        );
        assert!(sanitize_css(r"font-family: Menlo").is_ok());
        assert!(sanitize_css(r"color: red; padding: 4px").is_ok());
        // `\65 x` 是 hex 分支,但解出来只是 `ex` —— 不含任何拒绝构造。
        assert!(sanitize_css(r"color: \65 x").is_ok());
    }
}
