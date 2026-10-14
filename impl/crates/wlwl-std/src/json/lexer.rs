//! [v0.11.3 M7 / addendum-09 W-02] `std.json` 的**低层词法层**。
//!
//! `PARSE` 是 `serde_json` 的薄封装,它**只**给「对/错 + 一个诊断」;
//! 本模块给的是「**每个 token 的位置**」—— 所有精确错误定位与增量处理的前提。
//!
//! ## 为什么手写而不是复用 `serde_json`
//!
//! `serde_json` 不暴露 token 流(它的 `StreamDeserializer` 只给已解析的值),
//! 也不给行/列(只给 `line()` / `column()`,且是**它自己解析时顺带算的**,拿不到
//! 「第 k 个 token 从哪开始」)。而本批的核心产出就是位置(`path` / `line` /
//! `column`),所以词法层必须自己写。
//!
//! ## ⚠ 头号风险:与 `PARSE` 的接受集必须**逐条一致**
//!
//! `addendum-09` §3.6 冻结了旧两成员,而 `JSON_VALIDATE` 是**同一份语法的第二套
//! 判定**。两套一旦分叉,用户就会拿到「`PARSE` 说对、`JSON_VALIDATE` 说错」或
//! 反过来 —— 那比没有新成员更糟。⇒ 实测出 `serde_json` 比 RFC / Python **更严**
//! 的三处,本模块**必须照样严**,否则分叉:
//!
//! | 输入 | `serde_json`(⇒ `PARSE`) | Python `json` | 本模块
//! |---|---|---|---|
//! | `"\uD800"` / `"\uDC00"`(孤立代理) | **拒** | 收 | **拒** |
//! | `"\uDD1E\uD834"`(代理对换序) | **拒** | 收 | **拒** |
//! | 嵌套 **200** 层 | **拒**(上限 128) | 收 | **拒**(`MAX_DEPTH`) |
//! | `1e400`(f64 溢出) | **拒** | 收(`inf`) | **拒** |
//!
//! 孤立代理那条的根据是 Unicode:代理项必须成对,单独出现不是合法标量值,
//! 编不成 UTF-8。Python 收它是因为它的 `json` 产出的是 Python `str`(可以装
//! 孤立代理),而 wlwl 的字符串必须是合法 UTF-8 —— ⇒ **serde 在这里是对的,
//! Python 是宽松的**。
//!
//! ## ⚠ 偏移单位:**码点**,不是字节
//!
//! 全语言唯一的字符串切分原语是 `SUB(s, start, len?)`,规范 §10.5 明写「**按码点**」,
//! 而整份语言规范里「字节偏移 / byte offset」出现 **0 次**。⇒ 本模块发字节偏移
//! 等于发一个**用户用不了**的数字:喂不进 `SUB`,也不是 `CODEPOINTS` 的下标。
//! 故 `start` / `end` / `offset` / `token_start` 一律**码点、0 基**;
//! `line` / `column` **1 基**(`column` 也是码点列)。

/// 嵌套深度上限。与 `serde_json` 的默认递归上限一致 —— 实测 30 层收、200 层拒。
///
/// ⚠ 这条是**与 `PARSE` 对齐**的必要条件,不是「我们觉得 128 够了」。
/// 若把它调大,`JSON_VALIDATE` 就会比 `PARSE` 宽松 ⇒ 分叉。
pub const MAX_DEPTH: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Colon,
    Comma,
    String,
    Number,
    True,
    False,
    Null,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::LBrace => "lbrace",
            Kind::RBrace => "rbrace",
            Kind::LBracket => "lbracket",
            Kind::RBracket => "rbracket",
            Kind::Colon => "colon",
            Kind::Comma => "comma",
            Kind::String => "string",
            Kind::Number => "number",
            Kind::True => "true",
            Kind::False => "false",
            Kind::Null => "null",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Token {
    pub kind: Kind,
    /// 码点下标,0 基,闭区间左端 / 开区间右端(即 `[start, end)`)。
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone)]
pub struct JsonError {
    /// 出错处的码点下标(0 基)。
    pub offset: usize,
    /// 1 基行号 / 码点列号。
    pub line: usize,
    pub column: usize,
    pub reason: String,
}

impl JsonError {
    fn at(chars: &[char], offset: usize, reason: impl Into<String>) -> Self {
        let (line, column) = line_col(chars, offset);
        JsonError {
            offset,
            line,
            column,
            reason: reason.into(),
        }
    }
}

/// 由**码点下标**求 1 基的 `(line, column)`。
///
/// ⚠ 列按**码点**数,不是字节 —— 见模块注释「偏移单位」。
pub fn line_col(chars: &[char], offset: usize) -> (usize, usize) {
    let mut line = 1usize;
    let mut col = 1usize;
    for &c in chars.iter().take(offset.min(chars.len())) {
        if c == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}

fn is_ws(c: char) -> bool {
    // ⚠ RFC 8259 §2 只认这四个。实测 `\x0b` / `\x0c` / U+00A0 都**不是**空白
    // (与 Python `json` 一致),把它们当空白会让本模块比两边都宽松。
    matches!(c, ' ' | '\t' | '\n' | '\r')
}

/// 纯词法:切出 token 流,**不看结构**。
///
/// 刻意**不**做结构检查 —— `addendum-09` §2.1 写明 `JSON_TOKENIZE`「纯词法,无结构」,
/// 所以 `[}` 在这里合法(它不是词法错误)。结构由 [`validate`] 管。
pub fn tokenize(chars: &[char]) -> Result<Vec<Token>, JsonError> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < chars.len() {
        let c = chars[i];
        if is_ws(c) {
            i += 1;
            continue;
        }
        let start = i;
        let kind = match c {
            '{' => {
                i += 1;
                Kind::LBrace
            }
            '}' => {
                i += 1;
                Kind::RBrace
            }
            '[' => {
                i += 1;
                Kind::LBracket
            }
            ']' => {
                i += 1;
                Kind::RBracket
            }
            ':' => {
                i += 1;
                Kind::Colon
            }
            ',' => {
                i += 1;
                Kind::Comma
            }
            '"' => {
                scan_string(chars, &mut i)?;
                Kind::String
            }
            '-' | '0'..='9' => {
                scan_number(chars, &mut i)?;
                Kind::Number
            }
            't' => {
                scan_lit(chars, &mut i, "true")?;
                Kind::True
            }
            'f' => {
                scan_lit(chars, &mut i, "false")?;
                Kind::False
            }
            'n' => {
                scan_lit(chars, &mut i, "null")?;
                Kind::Null
            }
            other => {
                return Err(JsonError::at(
                    chars,
                    i,
                    format!("unexpected character {other:?}"),
                ))
            }
        };
        out.push(Token {
            kind,
            start,
            end: i,
        });
    }
    Ok(out)
}

/// 字符串:`i` 指向开头的 `"`;成功时 `i` 落在闭引号**之后**。
///
/// ⚠ 两处比 RFC 更严(见模块注释的表):**裸控制字符**(< 0x20)拒,
/// **`\uXXXX` 必须是成对代理**才收。
fn scan_string(chars: &[char], i: &mut usize) -> Result<(), JsonError> {
    let chars_ref = chars;
    *i += 1; // 开引号
    loop {
        let Some(&c) = chars_ref.get(*i) else {
            return Err(JsonError::at(chars_ref, *i, "unterminated string"));
        };
        match c {
            '"' => {
                *i += 1;
                return Ok(());
            }
            '\\' => {
                let esc_at = *i;
                *i += 1;
                let Some(&e) = chars_ref.get(*i) else {
                    return Err(JsonError::at(chars_ref, esc_at, "unterminated escape"));
                };
                match e {
                    '"' | '\\' | '/' | 'b' | 'f' | 'n' | 'r' | 't' => *i += 1,
                    'u' => {
                        // ⚠ `i` 此刻指向 `'u'`,四个十六进制位在 `i+1..=i+4`。
                        //   `read_hex4` 从 `i+1` 读,读完后必须落到**最后一位之后**
                        //   —— 即 `i += 5`,不是 `+= 4`。写成 `+= 4` 会停在第四位
                        //   **本身**上,于是代理对的第二个 `\u` 检查读到的是十六
                        //   进制位而不是 `\`,合法的 `\uD834\uDD1E` 被误判成
                        //   「lone high surrogate」。
                        //   ⚠ 这个 bug 是 `JSON_VALIDATE` vs `PARSE` 的 **76 条
                        //   对账**抓出来的(76 条里只错这 1 条)—— 手上没有那份
                        //   对账,这个偏移就写对了、也永远不会有人发现。
                        let first = read_hex4(chars_ref, i, esc_at)?;
                        *i += 5;
                        if (0xD800..0xDC00).contains(&first) {
                            // 高代理:必须紧跟 `\uDC00..\uDFFF`
                            if chars_ref.get(*i) == Some(&'\\')
                                && chars_ref.get(*i + 1) == Some(&'u')
                            {
                                let esc2 = *i;
                                // ⚠ 与外层同构:`esc_at` 停在 `\` **本身**,要落到
                                //   `u` 上得 `+= 1`,不是 `+= 2`。写成 `+= 2` 会落到
                                //   第一个十六进制位上,于是 `read_hex4` 从那里起读
                                //   四位、读到的最后一个是闭引号 ⇒ 报
                                //   「needs 4 hex digits, got '"'」——
                                //   而真实的失败是「合法的代理对被判成非法」。
                                *i += 1;
                                let second = read_hex4(chars_ref, i, esc2)?;
                                *i += 5; // 同上:落到最后一个十六进制位之后
                                if !(0xDC00..0xE000).contains(&second) {
                                    return Err(JsonError::at(
                                        chars_ref,
                                        esc_at,
                                        "high surrogate not followed by a low surrogate",
                                    ));
                                }
                            } else {
                                return Err(JsonError::at(
                                    chars_ref,
                                    esc_at,
                                    "lone high surrogate",
                                ));
                            }
                        } else if (0xDC00..0xE000).contains(&first) {
                            return Err(JsonError::at(chars_ref, esc_at, "lone low surrogate"));
                        }
                    }
                    other => {
                        return Err(JsonError::at(
                            chars_ref,
                            esc_at,
                            format!("invalid escape \\{other}"),
                        ))
                    }
                }
            }
            other if (other as u32) < 0x20 => {
                return Err(JsonError::at(
                    chars_ref,
                    *i,
                    format!("raw control character U+{:04X} in string", other as u32),
                ))
            }
            _ => *i += 1,
        }
    }
}

fn read_hex4(chars: &[char], i: &usize, at: usize) -> Result<u32, JsonError> {
    let mut v = 0u32;
    for k in 0..4 {
        let Some(&c) = chars.get(i + 1 + k) else {
            return Err(JsonError::at(chars, at, "truncated \\uXXXX escape"));
        };
        let Some(d) = c.to_digit(16) else {
            return Err(JsonError::at(
                chars,
                at,
                format!("\\uXXXX escape needs 4 hex digits, got {c:?}"),
            ));
        };
        v = v * 16 + d;
    }
    Ok(v)
}

/// 数字:`-? (0 | [1-9][0-9]*) (. [0-9]+)? ([eE] [+-]? [0-9]+)?`
///
/// ⚠ **前导零不收**(`01` / `01.5` 拒)—— RFC 8259 §6,与 `serde_json` / Python 一致。
fn scan_number(chars: &[char], i: &mut usize) -> Result<(), JsonError> {
    let start = *i;
    if chars.get(*i) == Some(&'-') {
        *i += 1;
    }
    match chars.get(*i) {
        Some('0') => {
            *i += 1;
            if matches!(chars.get(*i), Some(c) if c.is_ascii_digit()) {
                return Err(JsonError::at(chars, start, "leading zero in number"));
            }
        }
        Some(c) if c.is_ascii_digit() => {
            while matches!(chars.get(*i), Some(c) if c.is_ascii_digit()) {
                *i += 1;
            }
        }
        _ => return Err(JsonError::at(chars, start, "expected a digit")),
    }
    if chars.get(*i) == Some(&'.') {
        *i += 1;
        if !matches!(chars.get(*i), Some(c) if c.is_ascii_digit()) {
            return Err(JsonError::at(chars, *i, "expected a digit after '.'"));
        }
        while matches!(chars.get(*i), Some(c) if c.is_ascii_digit()) {
            *i += 1;
        }
    }
    if matches!(chars.get(*i), Some('e') | Some('E')) {
        *i += 1;
        if matches!(chars.get(*i), Some('+') | Some('-')) {
            *i += 1;
        }
        if !matches!(chars.get(*i), Some(c) if c.is_ascii_digit()) {
            return Err(JsonError::at(chars, *i, "expected a digit in the exponent"));
        }
        while matches!(chars.get(*i), Some(c) if c.is_ascii_digit()) {
            *i += 1;
        }
    }
    Ok(())
}

fn scan_lit(chars: &[char], i: &mut usize, lit: &str) -> Result<(), JsonError> {
    let start = *i;
    for (k, want) in lit.chars().enumerate() {
        if chars.get(start + k) != Some(&want) {
            return Err(JsonError::at(chars, start, format!("expected {lit}")));
        }
    }
    *i = start + lit.chars().count();
    Ok(())
}

/// 词法 + **结构** + 深度 + 数值范围。这是 `JSON_VALIDATE` 的内核。
///
/// 刻意与 `tokenize` 分开:前者是「纯词法」(§2.1 明写「无结构」),
/// 后者才是「语法对不对」。`[}` 在 `tokenize` 下合法、在 `validate` 下非法。
pub fn validate(chars: &[char]) -> Result<(), JsonError> {
    let toks = tokenize(chars)?;
    for t in &toks {
        if t.kind == Kind::Number {
            let text: String = chars[t.start..t.end].iter().collect();
            // ⚠ `1e400` 必须在**这里**拒:`serde_json` 把它解析成 f64 并溢出成 inf
            // 而报错。Python 收下 `inf` ⇒ 只跟 Python 走就会与 `PARSE` 分叉。
            match text.parse::<f64>() {
                Ok(v) if v.is_finite() => {}
                Ok(_) => {
                    return Err(JsonError::at(
                        chars,
                        t.start,
                        format!("number out of f64 range: {text}"),
                    ))
                }
                Err(e) => return Err(JsonError::at(chars, t.start, format!("bad number: {e}"))),
            }
        }
    }
    // 递归下降的状态机。
    let mut p = 0usize;
    let mut depth = 0usize;
    value(chars, &toks, &mut p, &mut depth)?;
    if p != toks.len() {
        return Err(JsonError::at(
            chars,
            toks[p].start,
            "trailing content after the top-level value",
        ));
    }
    Ok(())
}

fn value(
    chars: &[char],
    toks: &[Token],
    p: &mut usize,
    depth: &mut usize,
) -> Result<(), JsonError> {
    let Some(&t) = toks.get(*p) else {
        return Err(JsonError::at(chars, chars.len(), "unexpected end of input"));
    };
    match t.kind {
        Kind::LBrace => {
            *depth += 1;
            if *depth > MAX_DEPTH {
                return Err(JsonError::at(
                    chars,
                    t.start,
                    format!("nesting deeper than {MAX_DEPTH}"),
                ));
            }
            *p += 1;
            if toks.get(*p).map(|x| x.kind) == Some(Kind::RBrace) {
                *p += 1;
            } else {
                loop {
                    if toks.get(*p).map(|x| x.kind) != Some(Kind::String) {
                        let at = toks.get(*p).map(|x| x.start).unwrap_or(chars.len());
                        return Err(JsonError::at(chars, at, "expected an object key"));
                    }
                    *p += 1;
                    if toks.get(*p).map(|x| x.kind) != Some(Kind::Colon) {
                        let at = toks.get(*p).map(|x| x.start).unwrap_or(chars.len());
                        return Err(JsonError::at(chars, at, "expected ':'"));
                    }
                    *p += 1;
                    value(chars, toks, p, depth)?;
                    match toks.get(*p).map(|x| x.kind) {
                        Some(Kind::Comma) => *p += 1,
                        Some(Kind::RBrace) => {
                            *p += 1;
                            break;
                        }
                        _ => {
                            let at = toks.get(*p).map(|x| x.start).unwrap_or(chars.len());
                            return Err(JsonError::at(chars, at, "expected ',' or '}'"));
                        }
                    }
                }
            }
            *depth -= 1;
            Ok(())
        }
        Kind::LBracket => {
            *depth += 1;
            if *depth > MAX_DEPTH {
                return Err(JsonError::at(
                    chars,
                    t.start,
                    format!("nesting deeper than {MAX_DEPTH}"),
                ));
            }
            *p += 1;
            if toks.get(*p).map(|x| x.kind) == Some(Kind::RBracket) {
                *p += 1;
            } else {
                loop {
                    value(chars, toks, p, depth)?;
                    match toks.get(*p).map(|x| x.kind) {
                        Some(Kind::Comma) => *p += 1,
                        Some(Kind::RBracket) => {
                            *p += 1;
                            break;
                        }
                        _ => {
                            let at = toks.get(*p).map(|x| x.start).unwrap_or(chars.len());
                            return Err(JsonError::at(chars, at, "expected ',' or ']'"));
                        }
                    }
                }
            }
            *depth -= 1;
            Ok(())
        }
        Kind::String | Kind::Number | Kind::True | Kind::False | Kind::Null => {
            *p += 1;
            Ok(())
        }
        other => Err(JsonError::at(
            chars,
            t.start,
            format!("unexpected token {}", other.as_str()),
        )),
    }
}

/// 给定**码点下标**,返回该处**所在 token** 的起始下标。
///
/// ⚠ 第一版写成 `toks.iter().find(|t| t.start <= offset)` —— 那是「**第一个**开始
/// 早于它的 token」,于是查字符串中间的偏移会回**上一个** token 的起点
/// (`["é"]` 的码点 2 回 0 而不是 1)。⇒ 必须取**最后一个** `start <= offset`
/// 的那个,即包含该偏移的 token。
///
/// `offset` 落在空白上(不属于任何 token)时退回「最近的、开始早于它的 token」;
/// `offset` 在最前面或输入为空 ⇒ `None`(调用方映射成 `NULL` —— 0 会被读成
/// 「第 0 个 token 开始」,那不存在)。
pub fn token_start_at(chars: &[char], offset: usize) -> Option<usize> {
    let toks = tokenize(chars).ok()?;
    toks.iter()
        .rev()
        .find(|t| t.start <= offset)
        .map(|t| t.start)
}
