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

fn is_ascii_alnum(c: u8) -> bool {
    c.is_ascii_alphanumeric()
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
            Some(c) if is_ascii_alnum(*c) => {
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
                            let lower = format!("</{}", name);
                            let rest = src[next..].to_ascii_lowercase();
                            let close = rest.find(&lower).map(|p| next + p);
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
            Some(b'/') if b.get(i + 2).map(|c| is_ascii_alnum(*c)).unwrap_or(false) => {
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
                            entities::decode(&v)
                        }
                        _ => {
                            // 无引号:取到空白或 '>'。
                            let vstart = k;
                            while k < b.len() && !b[k].is_ascii_whitespace() && b[k] != b'>' {
                                k += 1;
                            }
                            entities::decode(&src[vstart..k])
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

    #[test]
    fn self_closing_and_malformed_are_tolerated() {
        let toks = tokenize("<br/><hr><p x=1");
        assert_eq!(tag_of(&toks[0]), "br");
        assert_eq!(tag_of(&toks[1]), "hr");
        assert_eq!(tag_of(&toks[2]), "p");
    }
}
