//! [v0.11.3 M3 W-11] HTML tokenizer(WHATWG 精神的容错子集)。
//!
//! 产出四种 token:文本(实体已解码)、开始标签(属性值已解码)、结束标签、
//! 注释。`script` / `style` 等 raw-text 元素产出 [`Token::RawText`](内容
//! **不**解码)—— 它们的整棵子树在过滤阶段删除,原样保留正文即可。
//!
//! 容错口径(畸形输入不报错):`<` 后不是字母 / `!` / `/` → 字面量;
//! 属性名里的怪字符直接吞进名字;引号不闭合 → 值取到 `>` 或串尾;
//! 结束标签里的属性忽略(HTML5 同款)。**本解析器只对「白名单子集」负责,
//! 差异由幂等性测试兜底(见计划 W-12)—— 不承诺与浏览器逐位一致。**

use super::entities;

#[cfg(test)]
use std::cell::Cell;

// ── [D13-059 / N-007]raw-text 关闭扫描的计量口径(测试用)─────────────
//
// raw-text 关闭扫描**读过**的输入字节数(测试计量口径)。
//
// ⚠ 定义就是「这段扫描为找匹配而碰过的输入字节」。任何重新引入「物化整段
// 余文」的做法都必然读满余文 ⇒ 必须在这里计数,否则复杂度契约空转。
#[cfg(test)]
thread_local! {
    static CLOSE_SCAN_PROBES: Cell<usize> = const { Cell::new(0) };
}

/// 取走当前线程的计量值(取走即清零)。
#[cfg(test)]
pub(crate) fn take_close_scan_probes() -> usize {
    CLOSE_SCAN_PROBES.with(|c| c.replace(0))
}

/// 非测试构建下编译为空(不留任何运行时开销)。
#[inline]
fn scan_probe(n: usize) {
    #[cfg(test)]
    CLOSE_SCAN_PROBES.with(|c| c.set(c.get() + n));
    #[cfg(not(test))]
    let _ = n;
}

/// 从 `from` 起找**首个** `</name`(大小写不敏感),返回其**绝对**下标。
///
/// 🔴 [D13-059 / N-007]此处此前是
/// `src[from..].to_ascii_lowercase().find(&format!("</{name}"))`:每遇一个
/// raw-text 元素就**分配并小写化整个余文** ⇒ 1 MB 输入 + 1 万个 raw-text
/// 就有 GB 级拷贝(规范 §13.2 的 O(n) 承诺)。本实现**只读、零分配**。
///
/// ⚠ 语义**逐字节不变**,三条:
/// 1. 找的是 `</name`,**不含结尾 `>`** —— 命中后那段(`</name` 到 `>`)
///    由调用方跳过;
/// 2. **首个**匹配即用;
/// 3. 大小写不敏感(`eq_ignore_ascii_case`,与「先整体小写再找」等价)。
///
/// ⚠ [`RAWTEXT`] 的八个名字**互不为前缀**(`title`/`textarea`、
/// `script`/`style`、`noembed`/`noframes` 都不是彼此的前缀)⇒ 匹配语义不
/// 可能因「needle 撞上另一个 needle」而歧义;前缀只可能来自**输入里更长的
/// 标签名**(如 `</scriptx>`),那是本模块已登记的容错口径(module doc
/// 「容错口径」),此处**不收紧**,以免把「改算法」变成「改语义」。
fn find_rawtext_close(src: &str, from: usize, name: &str) -> Option<usize> {
    let hay = src.as_bytes();
    let nb = name.as_bytes();
    let mut p = from;
    while p < hay.len() {
        // ⚠ needle 首字节恒是 `<` ⇒ 先跳到下一个 `<` 再比,不做逐字节空比。
        match hay[p..].iter().position(|&c| c == b'<') {
            Some(off) => {
                scan_probe(off + 1);
                p += off;
            }
            None => {
                scan_probe(hay.len() - p);
                return None;
            }
        }
        if hay.get(p + 1) == Some(&b'/') && p + 2 + nb.len() <= hay.len() {
            scan_probe(nb.len() + 1);
            if hay[p + 2..p + 2 + nb.len()].eq_ignore_ascii_case(nb) {
                return Some(p);
            }
        }
        p += 1;
    }
    None
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Attr {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Token {
    Text(String),
    Start {
        name: String,
        attrs: Vec<Attr>,
        self_closing: bool,
    },
    End(String),
    Comment(String),
    /// raw-text 元素:`name` = 标签名,`body` = 到对应结束标签为止的原文。
    RawText {
        name: String,
        body: String,
    },
}

/// 内容不作为标记解析的元素(HTML5 raw-text / RCDATA;含 iframe —— 其
/// 内文按 HTML5 也是「通常无内容」,统一按 raw-text 整段吞掉最安全)。
pub(crate) const RAWTEXT: &[&str] = &[
    "script", "style", "textarea", "title", "xmp", "noembed", "noframes", "iframe",
];

/// 标签名的**首**字符。⚠ **只能是字母** —— WHATWG *tag open state* 的定义
/// 就是「`U+003C` 后面跟一个 ASCII 字母」;跟一个数字时它是**字面量 `<`**。
///
/// 🔴 [D13-033 / N-015]此处此前用 `is_ascii_alnum`(**含数字**)⇒ 串中段的
/// `3<4 and 5<6` 会造出一个名为 `4` 的标签,并把 `and 5` 收成它的属性 ⇒
/// `4` 不在白名单 → 整棵拆壳 → **输出 `"3&lt;6"`,`"4 and 5"` 静默蒸发**。
/// HTML5 会原样输出 `3&lt;4 and 5&lt;6`。
///
/// ⚠ **结束标签分支此前是同一个错**(`</3>` 也会造假结束标签),两处一起改。
/// ⚠ 契约 `malformed_lone_lt` 用的 `3<4` 之所以此前也绿,是因为它**串尾**——
/// `parse_start_tag` 走到串尾返回 `None` 走了字面量分支,恰好蒙对。补上
/// `3<4 and 5<6` 这个形状才真正照到它。
fn is_ascii_alphabetic(c: u8) -> bool {
    c.is_ascii_alphabetic()
}

/// 标签名 / 属性名:字母数字连体(HTML5 容许的字符远多,但白名单过滤
/// 只会保留小写字母数字名,这里从宽收集、让过滤器删)。
fn is_name_byte(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'-' || c == b'_' || c == b':' || c == b'.'
}

pub(crate) fn tokenize(src: &str) -> Vec<Token> {
    let b = src.as_bytes();
    let mut out: Vec<Token> = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] != b'<' {
            // 文本段:到下一个 '<' 为止,实体解码。
            let start = i;
            while i < b.len() && b[i] != b'<' {
                i += 1;
            }
            out.push(Token::Text(entities::decode(&src[start..i])));
            continue;
        }
        // —— '<' 开头的候选 ——
        match b.get(i + 1) {
            Some(c) if is_ascii_alphabetic(*c) => {
                match parse_start_tag(src, i) {
                    Some(((name, attrs), next, self_closing)) => {
                        out.push(Token::Start {
                            name: name.clone(),
                            attrs,
                            self_closing,
                        });
                        if !self_closing && RAWTEXT.contains(&name.as_str()) {
                            // raw-text:吞到对应的 `</name`(大小写不敏感),
                            // 找不到就吞到串尾。
                            let close = find_rawtext_close(src, next, &name);
                            let (body, next2) = match close {
                                Some(p) => (&src[next..p], p),
                                None => (&src[next..], b.len()),
                            };
                            out.push(Token::RawText {
                                name,
                                body: body.to_string(),
                            });
                            if close.is_some() {
                                // 跳过结束标签本体(到 '>' 为止)。
                                let mut j = next2;
                                while j < b.len() && b[j] != b'>' {
                                    j += 1;
                                }
                                i = (j + 1).min(b.len());
                            } else {
                                i = next2;
                            }
                            continue;
                        }
                        i = next;
                    }
                    None => {
                        // 解析失败(串尾截断)→ 字面量 '<'。
                        out.push(Token::Text("<".into()));
                        i += 1;
                    }
                }
            }
            Some(b'/')
                if b.get(i + 2)
                    .map(|c| is_ascii_alphabetic(*c))
                    .unwrap_or(false) =>
            {
                // D13-033 / N-015
                // 结束标签:取名字,跳到 '>'(结束标签的属性按 HTML5 忽略)。
                let mut j = i + 2;
                while j < b.len() && is_name_byte(b[j]) {
                    j += 1;
                }
                let name = src[i + 2..j].to_ascii_lowercase();
                out.push(Token::End(name));
                while j < b.len() && b[j] != b'>' {
                    j += 1;
                }
                i = (j + 1).min(b.len());
            }
            Some(b'!') => {
                // 注释 / doctype / CDATA:全部按注释吞(内容原样,不解码)。
                if src[i..].starts_with("<!--") {
                    // ⚠🔴 **[D13-025 / N-001]WHATWG 的 abrupt closing:
                    // `<!-->` 与 `<!--->` 是两个「合法空注释」** ——
                    // comment start state 见到 `>` 直接 emit 并回 data state;
                    // comment start dash state 见到 `-->` 同理。
                    //
                    // 这条分支**此前不存在**,而它是一整条 mXSS 链的起点:
                    // 输入 `<!--><img src=x onerror=alert(1)>` 此前找不到
                    // `-->` ⇒ **整个余文成为一条「未闭合注释」的数据** ⇒
                    // `<img>` **从未进入过滤阶段**,连白名单都��看过一眼。
                    // 序列化时又原样写回 `<!--` + 数据 + `-->`,输出串里
                    // 于是含 `<!-->` —— 而浏览器重解析时把它读成**空注释**,
                    // `<img …>` 落在注释之外成为**活标记** ⇒ XSS。
                    //
                    // ⚠ 为什么幂等性测试抓不住:二次 sanitize 解析回的**仍是**
                    // 同一条 Comment(本模块口径自洽),三组幂等用例全绿。
                    // ⇒ 「自幂等」是重解析安全的**必要不充分**条件。
                    if src[i + 4..].starts_with('>') {
                        out.push(Token::Comment(String::new()));
                        i += 5;
                        continue;
                    }
                    if src[i + 4..].starts_with("->") {
                        out.push(Token::Comment(String::new()));
                        i += 6;
                        continue;
                    }
                    match src[i + 4..].find("-->") {
                        Some(p) => {
                            out.push(Token::Comment(src[i + 4..i + 4 + p].to_string()));
                            i = i + 4 + p + 3;
                        }
                        None => {
                            out.push(Token::Comment(src[i + 4..].to_string()));
                            i = b.len();
                        }
                    }
                } else {
                    let mut j = i + 2;
                    while j < b.len() && b[j] != b'>' {
                        j += 1;
                    }
                    out.push(Token::Comment(src[(i + 2).min(b.len())..j].to_string()));
                    i = (j + 1).min(b.len());
                }
            }
            Some(b'/') => {
                // `</` 后不是字母 → 字面量 "</"。
                out.push(Token::Text("</".into()));
                i += 2;
            }
            _ => {
                // `<` 后是结尾或非字母 → 字面量。
                out.push(Token::Text("<".into()));
                i += 1;
            }
        }
    }
    out
}

/// 解析开始标签。返回 `(标签, '>' 之后的下标, 是否自闭合)`。
/// `src[i]` 必须是 '<',且 `src[i+1]` 是 ASCII 字母。
#[allow(clippy::type_complexity)]
fn parse_start_tag(src: &str, i: usize) -> Option<((String, Vec<Attr>), usize, bool)> {
    let b = src.as_bytes();
    let mut j = i + 1;
    while j < b.len() && is_name_byte(b[j]) {
        j += 1;
    }
    if j >= b.len() {
        return None; // 串尾截断:没有 '>' —— 字面量处理。
    }
    let name = src[i + 1..j].to_ascii_lowercase();
    let mut attrs: Vec<Attr> = Vec::new();
    let mut self_closing = false;
    loop {
        while j < b.len() && b[j].is_ascii_whitespace() {
            j += 1;
        }
        match b.get(j) {
            Some(b'>') => {
                j += 1;
                break;
            }
            Some(b'/') => {
                self_closing = true;
                j += 1;
                // 自闭合后允许紧跟 '>'(再多的字符按属性解析容错吞掉)。
            }
            Some(&c) if is_name_byte(c) => {
                let name_start = j;
                while j < b.len() && is_name_byte(b[j]) {
                    j += 1;
                }
                let attr_name = src[name_start..j].to_ascii_lowercase();
                // 跳过 '=' 前后的空白。
                let mut k = j;
                while k < b.len() && b[k].is_ascii_whitespace() {
                    k += 1;
                }
                let value = if b.get(k) == Some(&b'=') {
                    k += 1;
                    while k < b.len() && b[k].is_ascii_whitespace() {
                        k += 1;
                    }
                    match b.get(k) {
                        Some(q @ (b'"' | b'\'')) => {
                            let quote = *q;
                            k += 1;
                            let vstart = k;
                            while k < b.len() && b[k] != quote {
                                k += 1;
                            }
                            let v = src[vstart..k.min(b.len())].to_string();
                            k = (k + 1).min(b.len());
                            entities::decode_in(&v, entities::Ctx::Attribute)
                        }
                        _ => {
                            // 无引号:取到空白或 '>'。
                            let vstart = k;
                            while k < b.len() && !b[k].is_ascii_whitespace() && b[k] != b'>' {
                                k += 1;
                            }
                            entities::decode_in(&src[vstart..k], entities::Ctx::Attribute)
                        }
                    }
                } else {
                    String::new() // 布尔属性:存在即真,值为空串。
                };
                if !attr_name.is_empty() {
                    attrs.push(Attr {
                        name: attr_name,
                        value,
                    });
                }
                j = j.max(k);
            }
            Some(b'<') | None => {
                // '<' 出现在标签内(畸形)或串尾:按 HTML5 的容错,
                // 把已收到的属性吐出去会丢信息 —— 直接结束标签于 '<' 前。
                break;
            }
            _ => {
                j += 1; // 其它怪字符吞掉(容错)。
            }
        }
        if self_closing && b.get(j - 1) == Some(&b'>') {
            break;
        }
    }
    Some(((name, attrs), j, self_closing))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tag_of(t: &Token) -> &str {
        match t {
            Token::Start { name, .. } => name,
            _ => panic!("not a start tag"),
        }
    }

    #[test]
    fn text_and_tags() {
        let toks = tokenize("a<b>hi</b>");
        // 4 个 token:「hi」是独立文本节点 —— 文本段按 '<' 切分,标记
        // 两侧的文本不会跨过标记合并。
        assert_eq!(toks.len(), 4);
        assert_eq!(toks[0], Token::Text("a".into()));
        assert_eq!(tag_of(&toks[1]), "b");
        assert_eq!(toks[2], Token::Text("hi".into()));
        assert_eq!(toks[3], Token::End("b".into()));
    }

    #[test]
    fn attributes_decoded_and_lowercased() {
        let toks = tokenize(r#"<A HREF="x?a=1&amp;b=2" checked>z</A>"#);
        match &toks[0] {
            Token::Start { name, attrs, .. } => {
                assert_eq!(name, "a");
                assert_eq!(attrs[0].name, "href");
                assert_eq!(attrs[0].value, "x?a=1&b=2");
                assert_eq!(attrs[1].name, "checked");
                assert_eq!(attrs[1].value, "");
            }
            _ => panic!(),
        }
    }

    #[test]
    fn stray_lt_is_literal() {
        let toks = tokenize("1 < 2 and 3<4");
        let mut text = String::new();
        for t in &toks {
            if let Token::Text(x) = t {
                text.push_str(x);
            }
        }
        assert_eq!(text, "1 < 2 and 3<4");
    }

    /// [D13-033 / N-015]**串中段**的 `<` + 数字必须是字面量。
    ///
    /// 期望值出处:WHATWG HTML *tag open state* —— `<` 后跟 ASCII **字母**
    /// 才进标签态;跟数字走 character reference / bogus comment 态,一律当
    /// 字面量。浏览器对 `3<4 and 5<6` 的结果是 `3&lt;4 and 5&lt;6`。
    ///
    /// ⚠ **这条必须带「串中段的后续内容」**。既有的 `stray_lt_is_literal`
    ///   用的 `3<4` 在**串尾**,此前是靠 `parse_start_tag` 走到串尾返回
    ///   `None` 才走到字面量分支的 —— **恰好蒙对**,照不到这个 bug。
    ///   去掉 ` and 5<6` 那个尾巴,守卫就废了。
    #[test]
    fn lt_digit_in_the_middle_is_literal_not_a_tag() {
        for src in ["3<4 and 5<6", "x<1 y<2", "a<9b"] {
            let toks = tokenize(src);
            assert!(
                !toks
                    .iter()
                    .any(|t| matches!(t, Token::Start { .. } | Token::End(_))),
                "{src:?} must produce no tags, got {toks:?}"
            );
            let mut text = String::new();
            for t in &toks {
                if let Token::Text(x) = t {
                    text.push_str(x);
                }
            }
            assert_eq!(text, src, "every character must survive as text");
        }
    }

    /// ⚠ 结束标签分支此前是**同一个错**:`</3>` 也会造出一个假结束标签。
    #[test]
    fn end_tag_branch_has_the_same_letter_only_rule() {
        assert_eq!(
            tokenize("a</3b"),
            vec![
                Token::Text("a".into()),
                Token::Text("</".into()),
                Token::Text("3b".into())
            ]
        );
        // 真标签照旧。
        assert_eq!(
            tokenize("a</b>c"),
            vec![
                Token::Text("a".into()),
                Token::End("b".into()),
                Token::Text("c".into())
            ]
        );
    }

    #[test]
    fn raw_text_swallows_markup() {
        let toks = tokenize(r#"<script>if (a<b) { x(); }</script>after"#);
        // 3 个 token:Start(属性在此) + RawText(正文原样,`<b)` 不作标记
        // 解析)+ Text。`</script>` 本体被 tokenizer 吞掉,**不发** End ——
        // 树构建改为在收到 RawText 时弹栈(tree.rs,见 D13-003)。
        assert_eq!(toks.len(), 3);
        match &toks[0] {
            Token::Start { name, .. } => assert_eq!(name, "script"),
            _ => panic!("toks[0] should be the start tag"),
        }
        match &toks[1] {
            Token::RawText { name, body } => {
                assert_eq!(name, "script");
                assert_eq!(body, "if (a<b) { x(); }");
            }
            _ => panic!("toks[1] should be the raw text"),
        }
        assert_eq!(toks[2], Token::Text("after".into()));
    }

    #[test]
    fn comments_swallowed_with_tolerance() {
        assert_eq!(
            tokenize("<!-- x > y -->ok"),
            vec![Token::Comment(" x > y ".into()), Token::Text("ok".into()),]
        );
        assert_eq!(
            tokenize("<!DOCTYPE html>"),
            vec![Token::Comment("DOCTYPE html".into()),]
        );
    }

    /// [D13-025 / N-001]`<!-->` 与 `<!--->` 是 WHATWG 的两个**合法空注释**
    /// (abrupt closing)。注释在此结束,余文回到 data state **继续按标记解析**。
    ///
    /// 期望值出处:WHATWG HTML §13.2.5.44 *comment start state*(见到 `>` 即
    /// abrupt-closing-of-empty-comment 并回 data state)与 *comment end state*
    /// (`-->` 收尾)。
    ///
    /// ⚠ 回归的判别式是最后那条 `Comment` / `Start(img)` 的**顺序与个数** ——
    /// 修复前整段余文被并成**一条**注释,`img` 连 token 都不是。
    #[test]
    fn abrupt_closing_ends_the_comment_and_content_resumes() {
        assert_eq!(
            tokenize("<!-->ok"),
            vec![Token::Comment(String::new()), Token::Text("ok".into())]
        );
        assert_eq!(
            tokenize("<!--->ok"),
            vec![Token::Comment(String::new()), Token::Text("ok".into())]
        );
        // mXSS 那条真实载荷:`img` 必须作为**开始标签**被看见,才能进过滤阶段。
        assert_eq!(
            tokenize("<!--><img src=x>"),
            vec![
                Token::Comment(String::new()),
                Token::Start {
                    name: "img".into(),
                    attrs: vec![Attr {
                        name: "src".into(),
                        value: "x".into()
                    }],
                    self_closing: false,
                },
            ]
        );
        // 边界:只有开括号(`<!--`)仍是「未闭合注释」;`<!--a-->` 仍是普通注释。
        assert_eq!(
            tokenize("<!--a-->b"),
            vec![Token::Comment("a".into()), Token::Text("b".into())]
        );
        assert_eq!(tokenize("<!--"), vec![Token::Comment(String::new())]);
    }

    #[test]
    fn self_closing_and_malformed_are_tolerated() {
        let toks = tokenize("<br/><hr><p x=1");
        assert_eq!(tag_of(&toks[0]), "br");
        assert_eq!(tag_of(&toks[1]), "hr");
        assert_eq!(tag_of(&toks[2]), "p");
    }

    /// N-007 **修复前**的关闭扫描(整体小写化 + `find`),只作语义参照。
    ///
    /// ⚠ 与 [`super::tree`] 里的 `build_reference` 同一口径:缺陷在**内存
    /// 流量**而不在语义,所以修复前的结果正是修复后必须逐样相等的参照物。
    fn find_close_reference(src: &str, from: usize, name: &str) -> Option<usize> {
        let lower = format!("</{name}");
        let rest = src[from..].to_ascii_lowercase();
        rest.find(&lower).map(|p| from + p)
    }

    /// [D13-059 / N-007]语义不变量:换算法不得改一个字节。
    ///
    /// 语料按 [`RAWTEXT`] 的**全部八个**名字铺开(任何漏测的名字都可能自带
    /// 前缀歧义),每个名字再配「首个大写 / 全小写 / 无闭合 / 命中后还有第二
    /// 个同名结束标签 / 命中的是更长标签名的前缀」五种形状。
    #[test]
    fn the_close_scan_is_byte_identical_to_the_old_one() {
        for name in RAWTEXT {
            let upper = name.to_ascii_uppercase();
            for src in [
                format!("<{name}>a</{name}>b"),
                format!("<{name}>a</{upper}>b"),
                format!("<{name}>a</{}>b</{name}>c", upper.to_lowercase()),
                format!("<{name}>a"),                   // 无闭合 → 吞到串尾
                format!("<{name}>a</{name}x>b"),        // 命中更长名的前缀
                format!("<{name}></{name}>"),           // 空正文
                format!("<{name}>a</{name}"),           // 命中在串尾
                format!("</{name}><{name}>a</{name}>"), // 命中前置的结束标签
                format!("<{name}>a</{name} >b"),        // 结尾 `>` 前有空白
            ] {
                for start in [0, src.len() / 3, src.len() / 2] {
                    assert_eq!(
                        find_rawtext_close(&src, start, name),
                        find_close_reference(&src, start, name),
                        "关闭扫描变了: name={name} start={start} src={src:?}"
                    );
                }
            }
        }
        // 起点恰在串尾 = 无匹配(修复前同款)。
        assert_eq!(find_rawtext_close("abc", 3, "script"), None);
    }

    /// [D13-059 / N-007]起点越界是 `None` 而不是 panic。
    ///
    /// ⚠ 这**不是**兼容性负担,是修复前那个写法顺带带来的差别:它是
    /// `src[from..]` 切片,`from > len` 会 panic。生产路径上 `from` 恒为
    /// `parse_start_tag` 返回的下标(≤ `len`),故这条此前不可达 —— 写成
    /// 契约是为了**不把「会 panic」这件事悄悄带进新函数**,而不是宣称修了
    /// 什么缺陷(否定向验证时它会转红:旧形态在此处 panic)。
    #[test]
    fn an_out_of_range_start_is_no_match_not_a_panic() {
        assert_eq!(find_rawtext_close("abc", 99, "script"), None);
        assert_eq!(find_rawtext_close("", 0, "script"), None);
    }

    /// [D13-059 / N-007]首个大写不敏感的 `</name` 即正文终点。
    ///
    /// 期望值出处:WHATWG HTML §13.2.5 的 *RAWTEXT / script data end tag*
    /// —— raw-text 元素在**遇到与起始标签同名**的结束标签时结束(大小写不敏感,
    /// HTML 的标签名一律 ASCII 大小写不敏感),且**首个**结束标签生效。
    #[test]
    fn the_first_matching_close_tag_ends_the_raw_text() {
        // `(输入, raw-text 元素名, 正文, 之后的内容)`
        for (src, want_name, want_body, want_rest) in [
            ("<script>a</SCRIPT>b", "script", "a", "b"),
            ("<script>a</ScRiPt>b", "script", "a", "b"),
            ("<title>a</title>b", "title", "a", "b"),
            // 首个即止:第二个 `</script>` 是**正文之外**的一个普通结束标签
            // (WHATWG 回到 data state 后按普通结束标签处理)。
            (
                "<script>a</script>b</script>c",
                "script",
                "a",
                "b</script>c",
            ),
            // 命中在串尾 ⇒ 其后无内容。
            ("<script>a</script", "script", "a", ""),
            // 无闭合 ⇒ 吞到串尾(WHATWG *script data* 到 EOF 同样是 EOF)。
            ("<script>a<b", "script", "a<b", ""),
            // 结束标签本体被吞掉(不发 End token)。
            ("<script>a</script>", "script", "a", ""),
        ] {
            let toks = tokenize(src);
            let raw = toks
                .iter()
                .find_map(|t| match t {
                    Token::RawText { name, body } => Some((name.as_str(), body.as_str())),
                    _ => None,
                })
                .unwrap_or_else(|| panic!("no raw text token in {src:?}"));
            assert_eq!(raw.0, want_name, "src={src:?}");
            assert_eq!(raw.1, want_body, "src={src:?}");
            let after: String = toks
                .iter()
                .skip_while(|t| !matches!(t, Token::RawText { .. }))
                .skip(1)
                .map(|t| -> String {
                    match t {
                        Token::Text(s) => s.clone(),
                        Token::End(s) => format!("</{s}>"),
                        Token::Comment(s) => format!("<!--{s}-->"),
                        Token::Start { name, .. } => format!("<{name}>"),
                        Token::RawText { .. } => unreachable!(),
                    }
                })
                .collect();
            assert_eq!(after, want_rest, "src={src:?}");
        }
    }

    /// [D13-059 / N-007]复杂度契约:关闭扫描的**总**输入字节量必须线性。
    ///
    /// 规范依据:§13.2 性能契约「**任何线性输入不得触发超线性时间**」与
    /// 「O(n) 线性」。语料形状 = 审查报告点名的「1 MB 输入 + 上万个
    /// raw-text」:这里取 K = 10_000 个 `<script></script>`(18 K 字节,
    /// 每个元素的余文都还在 ~180 KB 量级)。
    ///
    /// ⚠ 判据是**计量值**而非耗时:`probes` = 关闭扫描读过多少输入字节。
    /// 旧写法每个元素都**读满余文**(还额外小写化写一遍)⇒ Σ 余文 =
    /// Θ(n·k) = 4.5×10⁸,与线性上限 `4 × len`(= 7.2×10⁵)差三个数量级 ——
    /// 转红不靠余量,靠量级。⚠ 与树侧同一条局限:只覆盖**埋了计数**的扫描点。
    #[test]
    fn the_close_scan_does_not_read_the_whole_tail_per_element() {
        const K: usize = 10_000;
        let src = "<script></script>".repeat(K);
        let _ = take_close_scan_probes();
        let toks = tokenize(&src);
        let probes = take_close_scan_probes();
        // 语义不变:每个元素一条 Start + 一条 RawText(正文为空)。
        assert_eq!(toks.len(), 2 * K, "token 数变了 —— 关闭扫描吃掉了别的东西");
        assert_eq!(
            toks[1],
            Token::RawText {
                name: "script".into(),
                body: String::new()
            }
        );
        assert!(
            probes <= 4 * src.len(),
            "关闭扫描读过 {probes} 字节,超过线性上限 {} —— \
             「每个 raw-text 都物化整个余文」回来了?",
            4 * src.len()
        );
        // 无闭合时同理:扫到串尾是**一趟**,不是每元素一趟。
        let open_ended = "<script>x".repeat(K);
        let _ = take_close_scan_probes();
        let _ = tokenize(&open_ended);
        let probes = take_close_scan_probes();
        assert!(
            probes <= 2 * open_ended.len(),
            "无闭合语料下关闭扫描读过 {probes} 字节,超过上限 {}",
            2 * open_ended.len()
        );
    }
}
