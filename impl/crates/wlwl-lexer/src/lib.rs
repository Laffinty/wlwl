#![allow(clippy::doc_overindented_list_items)]

//! WLWL lexical analyzer.
//!
//! Phase 2 token set per v0.3 §3:
//! - Keywords: TRUE, FALSE, NULL, LET, FUN, RETURN, IF, WHILE, FOR, BREAK,
//!   CONTINUE, CLASS, NEW, THIS
//! - Operators (used as function names in Call positions; see v0.3 §9):
//!   + - * / % == != < > <= >= && || !
//! - Literals: integer, float, string (with escape sequences per §4.2)
//! - Identifiers: ASCII letters, digits, underscores
//! - Symbols: `( ) [ ] , ; : .`
//! - Comments: `//` single-line, `/* */` block (nesting supported, see §3.4)
//!
//! Errors (v0.3 §14.4):
//! - E0001 illegal character
//! - E0002 unterminated string
//! - E0003 unterminated block comment

// P4-G1-007: `WlwlError` is large by design (carries spec §14.2
// diagnostic schema with trace + cause + location + suggestion);
// restructuring it is out of scope for Phase G1. Allowed at the
// crate level rather than per-function because almost every public
// API in this lexer returns `WlwlResult<_>`.
#![allow(clippy::result_large_err)]

use wlwl_error::{
    extract_line, ErrorCode, Location, Suggestion, WlwlDiagnostic, WlwlError, WlwlResult,
};

/// A token in the source code.
#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // Keywords (v0.3 §3.2 — full set; Phase 1+2 emits all of these)
    True,
    False,
    Null,
    Let,
    /// v0.6 §1.4 contextual keyword: only special in the position
    /// `LET MUT(name, value)`. Other positions treat it as a normal
    /// identifier. The lexer does not enforce context — that is the
    /// parser's job.
    Mut,
    Fun,
    Return,
    If,
    While,
    For,
    Break,
    Continue,
    Class,
    New,
    This,
    // §12 error handling keywords (Phase 2)
    Ok,
    Err,
    Panic,
    Try,
    IsOk,
    IsErr,
    OrDie,
    // §13 module keywords (Phase 2)
    Import,
    Export,
    // v0.4 §7.6: \MATCH\ is a macro-function (§3.4) but is
    // tokenized as a keyword to keep parser dispatch in lock-step
    // with IF/WHILE/TRY. The spec text classifies MATCH as a macro
    // (not a §3.3 keyword) so it is intentionally absent from the
    // 14-keyword list; the lexer/parser treat it the same as the
    // other Phase-2 macro keywords.
    Match,
    // Literals
    Integer(i64),
    Float(f64),
    /// Plain string literal (v0.3 §4.2). Used when the literal
    /// contains no `${...}` interpolation — the common case.
    StringLit(String),
    /// [项目扩展] 字节串字面量 `b"..."` —— **原始字节**，不做 UTF-8 校验。
    ///
    /// ⚠ 为什么要它：语言里**没有 BYTES 类型**（`wlwl schema` 的类型表只有
    ///   INTEGER/FLOAT/STRING/BOOLEAN/NULL/ARRAY/DICT/OPTION/RESULT/FUN/TASK/
    ///   CHANNEL/CLASS/INSTANCE）⇒ 二进制只能落成 `ARRAY[INTEGER]`。但
    ///   `ARRAY[INTEGER]` 字面量写起来是 `[104, 105]` —— 不可读、不可 diff、
    ///   也不能表达 `\x00` 这类字节。
    /// ⇒ 解析器把 `b"..."` **脱糖**成 `ARRAY[INTEGER]`（见 parser 的
    ///   `parse_literal`）—— **不引入新类型、不引入新 AST 节点**。
    ///
    /// 与普通字符串的差别：① 不做 UTF-8 校验；② **不支持插值**
    ///   （`${...}` 在字节串里是错误 —— 字节串是常量）；③ 多一个 `\xNN` 转义。
    BytesLit(Vec<u8>),
    // v0.6 §1.8: interpolated string literal. The lexer emits a
    // sequence of three token kinds bracketing zero or more
    // expression tokens (lexed recursively):
    //   `StrStart`, [ `StrText(text)` | <inner expression tokens> ]*, `StrEnd`
    /// Marker at the start of an interpolated string.
    StrStart,
    /// Text segment inside an interpolated string.
    StrText(String),
    /// Marker at the end of an interpolated string (the closing `"`).
    StrEnd,
    // Identifiers
    Ident(String),
    // Symbols
    LParen,
    RParen,
    LBracket,
    RBracket,
    Comma,
    Semicolon,
    Colon,
    Dot,
    // Operators (Phase 2, v0.3 §9). The lexer emits these as their own
    // token kinds; the parser treats them as function names when they
    // appear in a Call position (followed by `(`).
    Plus,    // +
    Minus,   // -
    Star,    // *
    Slash,   // /
    Percent, // %
    EqEq,    // ==
    BangEq,  // !=
    /// P3-011 §8.2: single `=` is used in default-parameter
    /// bindings (`name = expr`) and in future let-bindings. The
    /// lexer must NOT collapse a bare `=` into `==`; `==` is its own
    /// token and is matched first.
    Eq,
    Lt,       // <
    Gt,       // >
    LtEq,     // <=
    GtEq,     // >=
    AmpAmp,   // &&
    PipePipe, // ||
    Bang,     // !  (v0.6: canonical alongside NOT; both are accepted without warning)
    /// v0.6 §1.5: `NOT` keyword is the lexical name for logical
    /// negation. The single-char `!` is also accepted (no warning in
    /// v0.6 — v0.5's `W0054` deprecation was removed). Both produce
    /// the same builtin call.
    Not,
    // End of file
    Eof,
}

impl TokenKind {
    pub fn is_reserved_keyword(&self) -> bool {
        matches!(
            self,
            TokenKind::Fun
                | TokenKind::Return
                | TokenKind::If
                | TokenKind::While
                | TokenKind::For
                | TokenKind::Break
                | TokenKind::Continue
                | TokenKind::Class
                | TokenKind::New
                | TokenKind::This
        )
    }

    /// If this token kind is a built-in operator (v0.3 §9), return its
    /// surface name. The parser uses this to translate operator tokens
    /// into function names in Call positions (e.g. `+(1, 2)` → Call "+").
    pub fn as_op_name(&self) -> Option<&'static str> {
        match self {
            TokenKind::Plus => Some("+"),
            TokenKind::Minus => Some("-"),
            TokenKind::Star => Some("*"),
            TokenKind::Slash => Some("/"),
            TokenKind::Percent => Some("%"),
            TokenKind::EqEq => Some("=="),
            // Phase I1 (spec §9.2): `=(a, b)` is the spec's equality
            // spelling; the implementation's builtin is registered as
            // `==`, so a single `=` in Call position desugars to it.
            // Default-parameter `name = default` is consumed earlier by
            // the parser's FUN parameter list and never reaches here.
            TokenKind::Eq => Some("=="),
            TokenKind::BangEq => Some("!="),
            TokenKind::Lt => Some("<"),
            TokenKind::Gt => Some(">"),
            TokenKind::LtEq => Some("<="),
            TokenKind::GtEq => Some(">="),
            TokenKind::AmpAmp => Some("&&"),
            TokenKind::PipePipe => Some("||"),
            TokenKind::Bang => Some("!"),
            TokenKind::Not => Some("NOT"),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: (u32, u32, u32, u32), // (line_start, col_start, line_end, col_end)
}

/// Lex the input source. The `file` parameter is used for diagnostic locations.
///
/// **D11-023 — spec §1.1: a source file is "UTF-8 encoded (with or without
/// BOM)".** The BOM (`U+FEFF`) was previously glued onto the first
/// identifier: the token dispatch routes any byte `>= 0xC0` to
/// `read_ident_or_keyword` (the deliberate P3-011 relaxation that lets
/// identifiers hold non-ASCII letters such as Chinese), and a BOM's
/// leading byte is `0xEF`, so `PRINT` lexed as `"\u{FEFF}PRINT"` and died
/// with `E0020: undefined name`. Stripping it in the identifier scanner
/// instead would fight P3-011 — the lexer is byte-oriented and cannot tell
/// a CJK letter from a zero-width no-break space.
///
/// Two details are load-bearing:
///
/// - **Exactly one**, via `strip_prefix` and *not* `trim_start_matches`.
///   A `U+FEFF` anywhere else in the file is a ZWNBSP, which P3-011's
///   relaxation consumes as identifier content; `trim_start_matches` would
///   silently swallow a second one and change what that file means.
/// - **Before `Lexer::new`**, because `Lexer` advances `col` per *byte*
///   (U+FEFF is 3 bytes). Stripping inside the token loop would shift every
///   column on line 1 by 3.
///
/// `&str` slicing is a fat-pointer adjustment, so this allocates nothing,
/// and the signature is unchanged.
pub fn lex(input: &str, file: &str) -> WlwlResult<Vec<Token>> {
    let input = input.strip_prefix('\u{FEFF}').unwrap_or(input);
    let mut lx = Lexer::new(input, file);
    lx.run()
}

struct Lexer<'a> {
    src: &'a [u8],
    pos: usize,
    line: u32,
    col: u32,
    file: String,
}

impl<'a> Lexer<'a> {
    fn new(src: &'a str, file: &str) -> Self {
        Self {
            src: src.as_bytes(),
            pos: 0,
            line: 1,
            col: 1,
            file: file.to_string(),
        }
    }

    fn peek(&self) -> Option<u8> {
        self.src.get(self.pos).copied()
    }

    fn peek_at(&self, offset: usize) -> Option<u8> {
        self.src.get(self.pos + offset).copied()
    }

    fn bump(&mut self) -> Option<u8> {
        let b = self.peek()?;
        self.pos += 1;
        if b == b'\n' {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
        Some(b)
    }

    fn skip_whitespace(&mut self) {
        while let Some(b) = self.peek() {
            if b.is_ascii_whitespace() {
                self.bump();
            } else {
                break;
            }
        }
    }

    #[allow(dead_code)] // P4-G1-reserved: convenience accessor for diagnostic / test helpers
    fn line_text(&self, line: u32) -> Option<String> {
        // Convenience wrapper used by tests / older code paths.
        extract_line(&self.src_text(), line)
    }

    fn src_text(&self) -> String {
        // Recover the source as &str (the lexer holds it as &[u8]).
        std::str::from_utf8(self.src).unwrap_or("").to_string()
    }

    fn err(&self, code: ErrorCode, message: impl Into<String>, line: u32, col: u32) -> WlwlError {
        let loc = Location::point(&self.file, line, col);
        let mut d = WlwlDiagnostic::new(code, message, loc);
        if let Some(s) = extract_line(&self.src_text(), line) {
            d = d.with_source_line(s);
        }
        d = match code {
            ErrorCode::E0001 => d.with_suggestion(Suggestion::Note {
                description: concat!(
                    "valid identifier characters: a-z, A-Z, 0-9, _ ; ",
                    "valid string escapes: `\" \\ / \u{8} \u{c} \n \r \t \0` ; ",
                    "(numbers must be ASCII digits, optionally with one '.')"
                )
                .into(),
            }),
            ErrorCode::E0002 => d.with_suggestion(Suggestion::Note {
                description: "add a closing `\"` before end of line, or split into \
                     two adjacent strings (WLWL concatenates them at parse time)"
                    .into(),
            }),
            ErrorCode::E0003 => d.with_suggestion(Suggestion::Note {
                description: "add a closing `*/` to terminate the block comment".into(),
            }),
            _ => d,
        };
        d.into()
    }

    fn read_number(&mut self) -> WlwlResult<Token> {
        // v0.8 spec §1.6 / §4.3 (cross-ref with parser `parse_expr`
        // unary-minus branch): this function INTENTIONALLY does not
        // consume a leading `+` / `-` sign. The lexer reads bare
        // digits only; the parser rewrites `-x` into `-(0, x)` at
        // the next layer up. Splitting sign-off from digit-recognition
        // keeps the lexer grammar-free (no lookahead for the parser)
        // and concentrates the negation in one place — same path
        // produces observable behavior equivalent to a leading-sign
        // literal, including INTEGER_MIN overflow semantics per §2.2
        // (E0034 raised at the runtime negation step, not here).
        let line = self.line;
        let col = self.col;
        let start = self.pos;
        // [项目扩展] 十六进制字面量 `0x1F`。
        //
        // ⚠ 为什么需要：字节值只能写成 `ARRAY[INTEGER]`，而十进制字节值
        //   （`b"\x1f"` 的等价物是 `[31]`）不直观；`0xFF` 是字节语境的自然写法。
        //   整数语义与十进制完全一致（同一个 `Literal::Integer`），**不新增类型**。
        if self.peek() == Some(b'0') && matches!(self.peek_at(1), Some(b'x') | Some(b'X')) {
            self.bump(); // '0'
            self.bump(); // 'x' | 'X'
            let hex_start = self.pos;
            while let Some(b) = self.peek() {
                if (b as char).is_ascii_hexdigit() {
                    self.bump();
                } else {
                    break;
                }
            }
            if self.pos == hex_start {
                return Err(self.err(
                    ErrorCode::E0001,
                    "invalid hexadecimal literal: no digits after 0x",
                    line,
                    col,
                ));
            }
            let digits = std::str::from_utf8(&self.src[hex_start..self.pos]).unwrap_or_default();
            let v = i64::from_str_radix(digits, 16).map_err(|_| {
                self.err(
                    ErrorCode::E0001,
                    format!("invalid hexadecimal literal '0x{digits}'"),
                    line,
                    col,
                )
            })?;
            let end_col = self.col;
            return Ok(Token {
                kind: TokenKind::Integer(v),
                span: (line, col, line, end_col),
            });
        }
        while let Some(b) = self.peek() {
            if b.is_ascii_digit() {
                self.bump();
            } else {
                break;
            }
        }
        let mut is_float = false;
        if self.peek() == Some(b'.') && matches!(self.peek_at(1), Some(b) if b.is_ascii_digit()) {
            is_float = true;
            self.bump(); // '.'
            while let Some(b) = self.peek() {
                if b.is_ascii_digit() {
                    self.bump();
                } else {
                    break;
                }
            }
        }
        // v0.8.1 D8-009: support `e` / `E` exponent in float literals
        // per spec §1.7 EBNF:
        //   float_lit = digits "." digits | digits exponent
        //              | digits "." digits exponent .
        //   exponent  = ( "e" | "E" ) [ "+" | "-" ] digits .
        //
        // The fractionless path `1e2` reaches here with `is_float = false`;
        // we promote it. The `1.5e2` path already had `is_float = true`.
        //
        // Only enter the exponent branch when its `[+|-]?digits` payload is
        // well-formed (next char after `e`/`E` is a sign or digit); otherwise
        // leave the letter for the next token so that `1east` still lexes
        // as Integer(1) followed by Ident("east"), not as a malformed float.
        if matches!(self.peek(), Some(b'e') | Some(b'E')) {
            let exp_payload_present = matches!(self.peek_at(1), Some(b'+') | Some(b'-'))
                || matches!(self.peek_at(1), Some(b) if b.is_ascii_digit());
            if exp_payload_present {
                is_float = true;
                self.bump(); // 'e' | 'E'
                if matches!(self.peek(), Some(b'+') | Some(b'-')) {
                    self.bump();
                }
                let exp_start = self.pos;
                while let Some(b) = self.peek() {
                    if b.is_ascii_digit() {
                        self.bump();
                    } else {
                        break;
                    }
                }
                if self.pos == exp_start {
                    // Sign consumed but no digits after — spec mandates
                    // `digits` after the optional sign, so `1e+` / `1e-`
                    // is malformed. Surface E0001 with the partial slice
                    // for diagnosis.
                    let bad_text = std::str::from_utf8(&self.src[start..self.pos])
                        .unwrap_or_default()
                        .to_string();
                    return Err(self.err(
                        ErrorCode::E0001,
                        format!("invalid float exponent in '{}'", bad_text),
                        line,
                        col,
                    ));
                }
            }
        }
        let text = std::str::from_utf8(&self.src[start..self.pos]).unwrap();
        let (kind, span) = if is_float {
            let v: f64 = text.parse().map_err(|_| {
                self.err(
                    ErrorCode::E0001,
                    format!("invalid float '{}'", text),
                    line,
                    col,
                )
            })?;
            let end_col = self.col;
            (TokenKind::Float(v), (line, col, line, end_col))
        } else {
            let v: i64 = text.parse().map_err(|_| {
                self.err(
                    ErrorCode::E0001,
                    format!("invalid integer '{}'", text),
                    line,
                    col,
                )
            })?;
            let end_col = self.col;
            (TokenKind::Integer(v), (line, col, line, end_col))
        };
        Ok(Token { kind, span })
    }

    /// Read a **byte string** literal `b"..."` (项目扩展).
    ///
    /// 与 [`Self::read_string`] 的差别：
    /// - **不做 UTF-8 校验** —— 字节串就是字节序列（`b"\xff"` 合法）。
    /// - **没有插值** —— `${` 在字节串里是**错误**（字节串是常量）。
    /// - 多一个 `\xNN` 转义（两位十六进制 ⇒ 一个字节）。
    ///
    /// 产出一个 `BytesLit` token；解析器再把它脱糖成 `ARRAY[INTEGER]`。
    fn read_bytes(&mut self) -> WlwlResult<Vec<Token>> {
        let line = self.line;
        let col = self.col;
        self.bump(); // opening '"'
        let mut bytes: Vec<u8> = Vec::new();
        loop {
            match self.peek() {
                Some(b'"') => {
                    self.bump();
                    let span = (line, col, self.line, self.col);
                    return Ok(vec![Token {
                        kind: TokenKind::BytesLit(bytes),
                        span,
                    }]);
                }
                Some(b'\\') => {
                    self.bump();
                    match self.bump() {
                        Some(b'n') => bytes.push(b'\n'),
                        Some(b't') => bytes.push(b'\t'),
                        Some(b'r') => bytes.push(b'\r'),
                        Some(b'\\') => bytes.push(b'\\'),
                        Some(b'"') => bytes.push(b'"'),
                        Some(b'/') => bytes.push(b'/'),
                        Some(b'b') => bytes.push(0x08),
                        Some(b'f') => bytes.push(0x0c),
                        Some(b'0') => bytes.push(b'\0'),
                        Some(b'$') => bytes.push(b'$'),
                        // `\xNN` —— 两位十六进制 ⇒ 一个字节（字节串专用）。
                        Some(b'x') => {
                            let hi = self.bump();
                            let lo = self.bump();
                            let (Some(hi), Some(lo)) = (hi, lo) else {
                                return Err(self.err(
                                    ErrorCode::E0002,
                                    "unterminated byte escape (\\x at EOF)",
                                    line,
                                    col,
                                ));
                            };
                            let Some(h) = (hi as char).to_digit(16) else {
                                return Err(self.err(
                                    ErrorCode::E0001,
                                    format!("invalid hex digit in \\x escape: {}", hi as char),
                                    line,
                                    col,
                                ));
                            };
                            let Some(l) = (lo as char).to_digit(16) else {
                                return Err(self.err(
                                    ErrorCode::E0001,
                                    format!("invalid hex digit in \\x escape: {}", lo as char),
                                    line,
                                    col,
                                ));
                            };
                            bytes.push(((h << 4) | l) as u8);
                        }
                        Some(c) => {
                            return Err(self.err(
                                ErrorCode::E0001,
                                format!("invalid escape '\\{}'", c as char),
                                line,
                                col,
                            ));
                        }
                        None => {
                            return Err(self.err(
                                ErrorCode::E0002,
                                "unterminated byte string (escape at EOF)",
                                line,
                                col,
                            ));
                        }
                    }
                }
                Some(b'$') if self.peek_at(1) == Some(b'{') => {
                    return Err(self.err(
                        ErrorCode::E0001,
                        "byte string literals do not support ${...} interpolation",
                        line,
                        col,
                    ));
                }
                Some(b'\n') | None => {
                    return Err(self.err(ErrorCode::E0002, "unterminated byte string", line, col));
                }
                Some(b) => {
                    bytes.push(b);
                    self.bump();
                }
            }
        }
    }

    fn read_ident_or_keyword(&mut self) -> WlwlResult<Token> {
        let line = self.line;
        let col = self.col;
        let start = self.pos;
        // v0.3 §3.1: identifiers allow letters/digits/underscore with
        // a non-digit first character, and explicitly allow Chinese
        // (and by extension any Unicode letter — see §3.1 note "建议
        // 在生产代码中使用 ASCII 标识符以提升 AI 编码效率"). We
        // accept ASCII alphanumeric + `_` plus any UTF-8 multi-byte
        // sequence (the lexer is permissive; the parser-level `is_ident`
        // check stays in place for ASCII).
        while let Some(b) = self.peek() {
            if b.is_ascii_alphanumeric() || b == b'_' {
                self.bump();
            } else if b >= 0xC0 {
                // UTF-8 leading byte: count continuation bytes (1-3)
                // and bump through the whole code point.
                let cont = if b < 0xE0 {
                    1
                } else if b < 0xF0 {
                    2
                } else if b < 0xF8 {
                    3
                } else {
                    break;
                };
                let mut ok = true;
                for i in 1..=cont {
                    match self.peek_at(i as usize) {
                        Some(cb) if (cb & 0xC0) == 0x80 => {}
                        _ => {
                            ok = false;
                            break;
                        }
                    }
                }
                if !ok {
                    break;
                }
                for _ in 0..=cont {
                    self.bump();
                }
            } else {
                break;
            }
        }
        let text = std::str::from_utf8(&self.src[start..self.pos])
            .unwrap()
            .to_string();
        let end_col = self.col;
        let kind = match text.as_str() {
            "TRUE" => TokenKind::True,
            "FALSE" => TokenKind::False,
            "NULL" => TokenKind::Null,
            "LET" => TokenKind::Let,
            "FUN" => TokenKind::Fun,
            "RETURN" => TokenKind::Return,
            "IF" => TokenKind::If,
            "WHILE" => TokenKind::While,
            "FOR" => TokenKind::For,
            "BREAK" => TokenKind::Break,
            "CONTINUE" => TokenKind::Continue,
            "CLASS" => TokenKind::Class,
            "NEW" => TokenKind::New,
            "THIS" => TokenKind::This,
            "OK" => TokenKind::Ok,
            "ERR" => TokenKind::Err,
            "PANIC" => TokenKind::Panic,
            "TRY" => TokenKind::Try,
            "IS_OK" => TokenKind::IsOk,
            "IS_ERR" => TokenKind::IsErr,
            "OR_DIE" => TokenKind::OrDie,
            "IMPORT" => TokenKind::Import,
            "EXPORT" => TokenKind::Export,
            // v0.4 Sec. 7.6 macro-function (lexer-level keyword).
            "MATCH" => TokenKind::Match,
            // Phase B9 (spec §3.4 macro-function §14.5): `NOT` is the
            // v0.4 canonical spelling for logical negation. Lexed as
            // a keyword (not an ident) so the parser emits the
            // v0.6 §1.4: `MUT` is a contextual keyword; the lexer
            // tokenizes it as `Mut`, and the parser consumes it only
            // in the `LET MUT(...)` position. Other uses become a
            // regular identifier (handled by the fallback below).
            "MUT" => TokenKind::Mut,
            // `NOT` is the canonical keyword for logical negation.
            // The single-char `!` is also accepted (no warning in v0.6)
            // — both produce the same builtin call.
            "NOT" => TokenKind::Not,
            _ => TokenKind::Ident(text),
        };
        Ok(Token {
            kind,
            span: (line, col, line, end_col),
        })
    }

    /// Read a string literal (v0.3 §4.2 + v0.6 §1.8).
    ///
    /// Returns either:
    /// - a single `StringLit(String)` token (no `${...}` interpolation), or
    /// - a sequence `StrStart, [StrText(String) | <inner expression tokens>]*, StrEnd`
    ///   that brackets the WHOLE interpolated string. Each `${...}`
    ///   contributes one or more inner expression tokens (recursively
    ///   lexed); the lexer emits ONE `StrStart` at the first `${`
    ///   boundary and ONE `StrEnd` at the closing `"`.
    ///
    /// Earlier drafts emitted one `StrStart`/`StrEnd` pair per
    /// `${...}`, which caused nested parsing to consume the outer
    /// `StrEnd`. v0.6.1 collapses to a single bracketed pair.
    fn read_string(&mut self) -> WlwlResult<Vec<Token>> {
        let line = self.line;
        let col = self.col;
        self.bump(); // opening '"'
                     // v0.3 §4.2: strings are double-quoted, may contain any UTF-8
                     // (including 中文 — see also §3.1 identifier note). The lexer
                     // previously pushed individual bytes as `char`, which mangles
                     // multi-byte sequences into Latin-1 mojibake. We now accumulate
                     // raw bytes and decode once at the closing quote / interpolation boundary.
        let mut s_bytes: Vec<u8> = Vec::new();
        let mut parts: Vec<Token> = Vec::new();
        let mut str_start_emitted = false;
        let span_close = |s: &Lexer<'_>| (line, col, s.line, s.col);
        loop {
            match self.peek() {
                Some(b'"') => {
                    self.bump();
                    let span = span_close(self);
                    if !str_start_emitted {
                        let s = String::from_utf8(s_bytes).map_err(|e| {
                            self.err(
                                ErrorCode::E0001,
                                format!("invalid UTF-8 in string literal: {}", e),
                                line,
                                col,
                            )
                        })?;
                        return Ok(vec![Token {
                            kind: TokenKind::StringLit(s),
                            span,
                        }]);
                    }
                    if !s_bytes.is_empty() {
                        let s = String::from_utf8(s_bytes).map_err(|e| {
                            self.err(
                                ErrorCode::E0001,
                                format!("invalid UTF-8 in string literal: {}", e),
                                line,
                                col,
                            )
                        })?;
                        parts.push(Token {
                            kind: TokenKind::StrText(s),
                            span,
                        });
                    }
                    parts.push(Token {
                        kind: TokenKind::StrEnd,
                        span,
                    });
                    return Ok(parts);
                }
                Some(b'\\') => {
                    self.bump();
                    match self.bump() {
                        Some(b'n') => s_bytes.push(b'\n'),
                        Some(b't') => s_bytes.push(b'\t'),
                        Some(b'r') => s_bytes.push(b'\r'),
                        Some(b'\\') => s_bytes.push(b'\\'),
                        Some(b'"') => s_bytes.push(b'"'),
                        // v0.6 §1.8: added escapes. The v0.3 lexer
                        // already accepted `/` and `b`/`f` informally; we
                        // now formalize them and add `\$`.
                        Some(b'/') => s_bytes.push(b'/'),
                        Some(b'b') => s_bytes.push(0x08),
                        Some(b'f') => s_bytes.push(0x0c),
                        Some(b'0') => s_bytes.push(b'\0'),
                        Some(b'$') => s_bytes.push(b'$'),
                        Some(c) => {
                            return Err(self.err(
                                ErrorCode::E0001,
                                format!("invalid escape '\\{}'", c as char),
                                line,
                                col,
                            ));
                        }
                        None => {
                            return Err(self.err(
                                ErrorCode::E0002,
                                "unterminated string (escape at EOF)",
                                line,
                                col,
                            ));
                        }
                    }
                }
                Some(b'$') if self.peek_at(1) == Some(b'{') => {
                    // v0.6 §1.8: start of `${expr}` interpolation.
                    // Emit ONE `StrStart` per interpolated string (at
                    // the first `${...}` we encounter), flush any
                    // pending text as `StrText`, then recurse for the
                    // expression body. Subsequent `${...}` segments
                    // contribute only their inner tokens — no extra
                    // `StrStart` / `StrEnd`.
                    let interp_start = span_close(self);
                    if !str_start_emitted {
                        parts.push(Token {
                            kind: TokenKind::StrStart,
                            span: interp_start,
                        });
                        str_start_emitted = true;
                    }
                    if !s_bytes.is_empty() {
                        let s = String::from_utf8(s_bytes).map_err(|e| {
                            self.err(
                                ErrorCode::E0001,
                                format!("invalid UTF-8 in string literal: {}", e),
                                line,
                                col,
                            )
                        })?;
                        parts.push(Token {
                            kind: TokenKind::StrText(s),
                            span: interp_start,
                        });
                        s_bytes = Vec::new();
                    }
                    // Consume the `${` opener.
                    self.bump(); // '$'
                    self.bump(); // '{'
                    let inner = self.read_interp_body()?;
                    parts.extend(inner);
                    // After `read_interp_body`, the closing `}` has been
                    // consumed; continue reading text until the next
                    // `"` or `${`.
                }
                Some(b'\n') | None => {
                    return Err(self.err(ErrorCode::E0002, "unterminated string", line, col));
                }
                Some(b) => {
                    s_bytes.push(b);
                    self.bump();
                }
            }
        }
    }

    /// Read the expression body of a `${...}` interpolation (v0.6 §1.8).
    ///
    /// Returns the inner expression tokens (excluding the closing
    /// `}`, which this method consumes). Three rules:
    ///
    ///   1. Top-level `{` / `}` count for depth (so `{LET(x, {y: 1}), x}`
    ///      parses correctly).
    ///   2. `${...}` pairs inside the body are **skipped over** — they
    ///      belong to the OUTER string's next interpolation, not the
    ///      inner expression. Without this skip, `"${1}${2}"` lexes
    ///      "1${2}" as the body of the first `${...}`, and the
    ///      recursive `lex()` chokes on the bare `$`.
    ///   3. **Nested string literals are skipped over, not rejected.**
    ///
    /// ⚠ **[v0.11.3 / R2-005]** 第 3 条是修进来的。规范 §1.8 的文法是
    /// `interpolation = "${" Expression "}"`,而 §1.8 正文逐字写着
    /// 「`${...}` 内的表达式按**完整表达式语法**求值」—— 字符串字面量是
    /// `Expression` 的合法组成,所以 `${ERR("x")}` **必须**能写。
    ///
    /// 修之前这里见 `"` 就报 E0001「nested string literal … is not allowed」,
    /// 那条注释自称「per spec grammar」,**与 §1.8 的文法实际不符**。而且这条
    /// 限制长在**词法层**,绕不开:`"${"x"}"`、`"${ERR("boom")}"` 一律在词法期
    /// 被拒,连「换个写法」都不行。
    ///
    /// 跳过时要处理三件事,少一件就会把边界数错:
    ///   ① **转义引号**:`\"` 不是结束符;
    ///   ② 嵌套字符串里的 `{}` **不是**插值体的括号(否则深度计数被带偏,
    ///      `}` 会提前收尾);
    ///   ③ 嵌套字符串里**自己**的插值(`${…}`)同样不算本插值体的括号。
    fn read_interp_body(&mut self) -> WlwlResult<Vec<Token>> {
        let start = self.pos;
        let end = match scan_interp_region(self.src, self.pos) {
            Ok(end) => end,
            Err(ScanFail::Interp) => {
                return Err(self.err(
                    ErrorCode::E0002,
                    "unterminated string interpolation (missing `}`); \
                     hint: `${...}` body is **expression** context — write string \
                     literals with plain `\"` delimiters (do **not** write `\\\"`, \
                     which is the outer string's escape and confuses the scan)",
                    self.line,
                    self.col,
                ));
            }
            Err(ScanFail::NestedInterp) => {
                return Err(self.err(
                    ErrorCode::E0002,
                    "unterminated nested string interpolation (missing `}`); \
                     hint: inside `${...}` use plain `\"` for nested string args \
                     (e.g. `AT_K(m, \"k\", \"?\")`), **not** `\\\"` — escaped quotes \
                     belong to the outer string and break brace pairing",
                    self.line,
                    self.col,
                ));
            }
            Err(ScanFail::String) => {
                return Err(self.err(
                    ErrorCode::E0002,
                    "unterminated string literal inside `${...}` interpolation; \
                     hint: nested string args use plain `\"` delimiters; `\\\"` is \
                     the outer string's escape and will not close the inner literal",
                    self.line,
                    self.col,
                ));
            }
        };
        let inner_bytes = &self.src[start..end];
        let inner_str = std::str::from_utf8(inner_bytes).unwrap_or("");
        // Skip past the inner bytes plus the closing `}`.
        while self.pos < end + 1 {
            self.bump();
        }
        // Recursively lex the inner expression as a fresh source
        // buffer; strip the trailing EOF the recursive lexer emits.
        let mut inner_tokens = lex(inner_str, &self.file)?;
        if matches!(
            inner_tokens.last(),
            Some(Token {
                kind: TokenKind::Eof,
                ..
            })
        ) {
            inner_tokens.pop();
        }
        Ok(inner_tokens)
    }

    fn skip_line_comment(&mut self) {
        while let Some(b) = self.peek() {
            if b == b'\n' {
                break;
            }
            self.bump();
        }
    }

    fn skip_block_comment(&mut self) -> WlwlResult<()> {
        let line = self.line;
        let col = self.col;
        self.bump(); // '/'
        self.bump(); // '*'
        let mut depth = 1u32;
        while depth > 0 {
            match (self.peek(), self.peek_at(1)) {
                (Some(b'/'), Some(b'*')) => {
                    self.bump();
                    self.bump();
                    depth += 1;
                }
                (Some(b'*'), Some(b'/')) => {
                    self.bump();
                    self.bump();
                    depth -= 1;
                }
                (Some(_), _) => {
                    self.bump();
                }
                (None, _) => {
                    return Err(self.err(
                        ErrorCode::E0003,
                        "unterminated block comment",
                        line,
                        col,
                    ));
                }
            }
        }
        Ok(())
    }

    fn run(&mut self) -> WlwlResult<Vec<Token>> {
        let mut tokens = Vec::new();
        loop {
            self.skip_whitespace();
            let line = self.line;
            let col = self.col;
            let Some(b) = self.peek() else {
                tokens.push(Token {
                    kind: TokenKind::Eof,
                    span: (line, col, line, col),
                });
                return Ok(tokens);
            };
            match b {
                b'(' => {
                    self.bump();
                    tokens.push(Token {
                        kind: TokenKind::LParen,
                        span: (line, col, line, self.col),
                    });
                }
                b')' => {
                    self.bump();
                    tokens.push(Token {
                        kind: TokenKind::RParen,
                        span: (line, col, line, self.col),
                    });
                }
                b'[' => {
                    self.bump();
                    tokens.push(Token {
                        kind: TokenKind::LBracket,
                        span: (line, col, line, self.col),
                    });
                }
                b']' => {
                    self.bump();
                    tokens.push(Token {
                        kind: TokenKind::RBracket,
                        span: (line, col, line, self.col),
                    });
                }
                b',' => {
                    self.bump();
                    tokens.push(Token {
                        kind: TokenKind::Comma,
                        span: (line, col, line, self.col),
                    });
                }
                b';' => {
                    self.bump();
                    tokens.push(Token {
                        kind: TokenKind::Semicolon,
                        span: (line, col, line, self.col),
                    });
                }
                b':' => {
                    self.bump();
                    tokens.push(Token {
                        kind: TokenKind::Colon,
                        span: (line, col, line, self.col),
                    });
                }
                b'.' => {
                    self.bump();
                    tokens.push(Token {
                        kind: TokenKind::Dot,
                        span: (line, col, line, self.col),
                    });
                }
                // [项目扩展] 字节串字面量 `b"..."`。
                // ⚠ `b` 与 `"` **必须紧邻**（中间有空白 ⇒ 仍是标识符 `b` 后面跟一个字符串），
                //    与 Rust 的 `b"..."` 同一约定。
                b'b' if self.peek_at(1) == Some(b'"') => {
                    self.bump(); // 'b'
                    tokens.extend(self.read_bytes()?);
                }
                b'"' => tokens.extend(self.read_string()?),
                b'/' if self.peek_at(1) == Some(b'/') => {
                    self.skip_line_comment();
                }
                b'/' if self.peek_at(1) == Some(b'*') => {
                    self.skip_block_comment()?;
                }
                b'/' => {
                    self.bump();
                    tokens.push(Token {
                        kind: TokenKind::Slash,
                        span: (line, col, line, self.col),
                    });
                }
                b'+' => {
                    self.bump();
                    tokens.push(Token {
                        kind: TokenKind::Plus,
                        span: (line, col, line, self.col),
                    });
                }
                b'-' => {
                    self.bump();
                    tokens.push(Token {
                        kind: TokenKind::Minus,
                        span: (line, col, line, self.col),
                    });
                }
                b'*' => {
                    self.bump();
                    tokens.push(Token {
                        kind: TokenKind::Star,
                        span: (line, col, line, self.col),
                    });
                }
                b'%' => {
                    self.bump();
                    tokens.push(Token {
                        kind: TokenKind::Percent,
                        span: (line, col, line, self.col),
                    });
                }
                b'=' if self.peek_at(1) == Some(b'=') => {
                    self.bump();
                    self.bump();
                    tokens.push(Token {
                        kind: TokenKind::EqEq,
                        span: (line, col, line, self.col),
                    });
                }
                b'=' => {
                    // P3-011 §8.2: single `=` is the default-parameter
                    // separator. Lex as TokenKind::Eq.
                    self.bump();
                    tokens.push(Token {
                        kind: TokenKind::Eq,
                        span: (line, col, line, self.col),
                    });
                }
                b'!' if self.peek_at(1) == Some(b'=') => {
                    self.bump();
                    self.bump();
                    tokens.push(Token {
                        kind: TokenKind::BangEq,
                        span: (line, col, line, self.col),
                    });
                }
                b'!' => {
                    self.bump();
                    tokens.push(Token {
                        kind: TokenKind::Bang,
                        span: (line, col, line, self.col),
                    });
                }
                b'<' if self.peek_at(1) == Some(b'=') => {
                    self.bump();
                    self.bump();
                    tokens.push(Token {
                        kind: TokenKind::LtEq,
                        span: (line, col, line, self.col),
                    });
                }
                b'<' => {
                    self.bump();
                    tokens.push(Token {
                        kind: TokenKind::Lt,
                        span: (line, col, line, self.col),
                    });
                }
                b'>' if self.peek_at(1) == Some(b'=') => {
                    self.bump();
                    self.bump();
                    tokens.push(Token {
                        kind: TokenKind::GtEq,
                        span: (line, col, line, self.col),
                    });
                }
                b'>' => {
                    self.bump();
                    tokens.push(Token {
                        kind: TokenKind::Gt,
                        span: (line, col, line, self.col),
                    });
                }
                b'&' if self.peek_at(1) == Some(b'&') => {
                    self.bump();
                    self.bump();
                    tokens.push(Token {
                        kind: TokenKind::AmpAmp,
                        span: (line, col, line, self.col),
                    });
                }
                b'|' if self.peek_at(1) == Some(b'|') => {
                    self.bump();
                    self.bump();
                    tokens.push(Token {
                        kind: TokenKind::PipePipe,
                        span: (line, col, line, self.col),
                    });
                }
                c if c.is_ascii_digit() => tokens.push(self.read_number()?),
                c if c.is_ascii_alphabetic() || c == b'_' => {
                    tokens.push(self.read_ident_or_keyword()?)
                }
                // P3-011 §3.1: identifiers allow non-ASCII letters
                // (e.g. Chinese). The UTF-8 leading byte alone is not
                // an ASCII alphabetic, so route through the same
                // identifier reader which knows how to walk multi-byte
                // code points.
                c if c >= 0xC0 => tokens.push(self.read_ident_or_keyword()?),
                c => {
                    return Err(self.err(
                        ErrorCode::E0001,
                        format!("illegal character '{}'", c as char),
                        line,
                        col,
                    ));
                }
            }
        }
    }
}

// ── 插值体扫描器(词法层的边界判定) ────────────────────────────
//
// ⚠ [v0.11.3 / R2-005] 这两个自由函数是「插值体里允许嵌套字符串字面量」
// 那条修复的**全部**实现。规范 §1.8:`interpolation = "${" Expression "}"`,
// 且 §1.8 正文明写「`${...}` 内的表达式按**完整表达式语法**求值」⇒
// 字符串字面量是合法组成,词法层**不得**把它拒掉。
//
// 边界判定的难点不在「找 `}`」,而在**三种会把边界数错的东西**:
//   ① 嵌套字符串里的 `}`(它属于字符串,不属于插值体);
//   ② 转义引号 `\"`(它不是字符串的结束符);
//   ③ 嵌套字符串里**自己**的插值 `${…}`(它也不属于插值体的括号)。
// 三者任意漏掉一条,`"${ERR("a}b")}"` 这类源码就会被截断成半截表达式。

/// 扫描失败的原因。三种分开报,是因为它们的**修复动作**不同:
/// 补一个 `}`、补一个 `"`、还是整段重写。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScanFail {
    /// 当前这一层插值体没有闭合的 `}`。
    Interp,
    /// 插值体里**嵌套**的 `${…}` 没有闭合的 `}`。
    NestedInterp,
    /// 字符串字面量没有闭合的 `"`。
    String,
}

/// 扫描一层插值体:从 `i`(指向 `${` **之后**的第一个字符)开始,
/// 返回**闭合 `}` 的位置**(指向那个 `}` 本身)。
///
/// 形如 `{LET(x, {y: 1}), x}` 的表达式:花括号按深度配对。
fn scan_interp_region(src: &[u8], mut i: usize) -> Result<usize, ScanFail> {
    let mut depth: u32 = 0;
    while i < src.len() {
        match src[i] {
            // 嵌套字符串字面量:整段跳过。它里面的 `{}`、转义引号、
            // 自己的插值都**不是**本层的边界。
            b'"' => i = scan_nested_string(src, i)?,
            // 插值体里**又**出现 `${…}`:整段跳过 —— 它属于外层字符串的
            // 下一次插值,不是本插值体的表达式(修之前的老代码也是这个意思,
            // 只是它的括号计数不认字符串)。
            b'$' if src.get(i + 1) == Some(&b'{') => {
                let end = scan_interp_region(src, i + 2).map_err(|_| ScanFail::NestedInterp)?;
                i = end + 1; // 跳过闭合 `}`
            }
            b'{' => {
                depth += 1;
                i += 1;
            }
            b'}' => {
                if depth == 0 {
                    return Ok(i);
                }
                depth -= 1;
                i += 1;
            }
            _ => i += 1,
        }
    }
    Err(ScanFail::Interp)
}

/// 扫描一个**嵌套字符串字面量**:从 `i`(指向开引号 `"`)开始,
/// 返回闭合引号**之后**的位置。
///
/// 字符串里可以有插值 `${…}`,插值体里又可能有字符串 ⇒ 必须**递归**。
fn scan_nested_string(src: &[u8], mut i: usize) -> Result<usize, ScanFail> {
    i += 1; // 跳过开引号
    while i < src.len() {
        match src[i] {
            // 转义:被转义的那个字符**既不是**结束符,也不是括号。
            // ⚠ 末尾那个反斜杠会让 `i` 越过末尾 ⇒ 下面的循环条件接住它,
            // 报「字符串没闭合」而不是越界 panic。
            b'\\' => i += 2,
            b'"' => return Ok(i + 1),
            // 字符串里的插值:整段跳过(它的 `}` 不是本字符串的边界)。
            b'$' if src.get(i + 1) == Some(&b'{') => {
                let end = scan_interp_region(src, i + 2).map_err(|_| ScanFail::NestedInterp)?;
                i = end + 1; // 跳过闭合 `}`
            }
            _ => i += 1,
        }
    }
    Err(ScanFail::String)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lex_integers_and_floats() {
        // Float literals avoid approx-PI constants (`3.14`) to keep
        // `clippy::approx_constant` happy; we just need any non-int
        // literal that the lexer accepts.
        let toks = lex("42 1.25 0", "t.wll").unwrap();
        assert!(matches!(toks[0].kind, TokenKind::Integer(42)));
        assert!(matches!(toks[1].kind, TokenKind::Float(f) if (f - 1.25).abs() < 1e-9));
        assert!(matches!(toks[2].kind, TokenKind::Integer(0)));
    }

    // v0.8.1 D8-009 — spec §1.7 `digits exponent` / `digits "." digits exponent`
    // path. Prior to this fix, the lexer passed `e` / `E` straight to
    // `read_ident_or_keyword`, splitting `1e2` into Integer(1) + Ident("e2")
    // (a parse error downstream). The new branch in `read_number` consumes
    // the exponent when its `[+|-]?digits` payload is well-formed, and lets
    // `1east` lex as Integer(1) + Ident("east") unchanged.
    #[test]
    fn lex_exponent_digits_only() {
        // spec §1.7 form: `digits exponent` (no fraction).
        // `1e2` → Float(100.0).
        let toks = lex("1e2", "t.wll").unwrap();
        assert_eq!(toks.len(), 2, "got {:?}", toks);
        match &toks[0].kind {
            TokenKind::Float(f) => assert!((f - 100.0).abs() < 1e-9, "got {}", f),
            other => panic!("expected Float, got {:?}", other),
        }
    }

    #[test]
    fn lex_exponent_with_fraction() {
        // spec §1.7 form: `digits "." digits exponent`.
        // `1.5e2` → Float(150.0). Already partly supported as a fraction
        // before the fix; this test pins down the *post-fix* exponent
        // path so the trailing `e2` is not emitted as a separate ident.
        let toks = lex("1.5e2", "t.wll").unwrap();
        match &toks[0].kind {
            TokenKind::Float(f) => assert!((f - 150.0).abs() < 1e-9, "got {}", f),
            other => panic!("expected Float, got {:?}", other),
        }
    }

    #[test]
    fn lex_exponent_negative() {
        // `1.5e-2` → Float(0.015). Exercises the optional `[+|-]` sign.
        let toks = lex("1.5e-2", "t.wll").unwrap();
        match &toks[0].kind {
            TokenKind::Float(f) => assert!((f - 0.015).abs() < 1e-9, "got {}", f),
            other => panic!("expected Float, got {:?}", other),
        }
    }

    #[test]
    fn lex_exponent_uppercase_e() {
        // spec §1.7 accepts both `e` and `E`. `1E3` → Float(1000.0).
        let toks = lex("1E3", "t.wll").unwrap();
        match &toks[0].kind {
            TokenKind::Float(f) => assert!((f - 1000.0).abs() < 1e-9, "got {}", f),
            other => panic!("expected Float, got {:?}", other),
        }
    }

    #[test]
    fn lex_exponent_after_letter_is_ident() {
        // Regression guard: a letter following a digit is NOT an exponent
        // (no `[+|-]?digits` payload), so `1east` must still split into
        // Integer(1) + Ident("east"). The fix's `exp_payload_present` gate
        // is what preserves this invariant; without it, the lexer would
        // consume `e` and then fail with E0001 on `ast` (not a digit/sign).
        let toks = lex("1east", "t.wll").unwrap();
        assert!(matches!(toks[0].kind, TokenKind::Integer(1)));
        assert!(matches!(&toks[1].kind, TokenKind::Ident(s) if s == "east"));
    }

    #[test]
    fn lex_exponent_missing_digits_after_sign_errors_e0001() {
        // spec §1.7: `exponent` mandates digits after the optional sign.
        // `1e+` is invalid — fraction sign consumed but no digits remain.
        // Surface E0001 with the partial slice for diagnosis; do NOT
        // silently downgrade to an Integer(1) (would obscure the bug).
        let err = lex("1e+", "t.wll").unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0001);
        let msg = err.diagnostic().message.to_string();
        assert!(
            msg.contains("invalid float exponent"),
            "unexpected message: {}",
            msg
        );
    }

    #[test]
    fn lex_keywords() {
        let toks = lex("TRUE FALSE NULL LET", "t.wll").unwrap();
        assert_eq!(toks[0].kind, TokenKind::True);
        assert_eq!(toks[1].kind, TokenKind::False);
        assert_eq!(toks[2].kind, TokenKind::Null);
        assert_eq!(toks[3].kind, TokenKind::Let);
    }

    #[test]
    fn lex_not_keyword() {
        // v0.6 §1.5: `NOT` is the canonical keyword for logical
        // negation. The single-char `!` is also accepted (no warning
        // in v0.6) — both produce the same builtin call.
        let toks = lex("NOT", "t.wll").unwrap();
        assert_eq!(toks[0].kind, TokenKind::Not);
        // `!` remains its own token (parser turns it into a Call
        // with name "!").
        let toks = lex("!", "t.wll").unwrap();
        assert_eq!(toks[0].kind, TokenKind::Bang);
        // Mixed: lexes both as expected.
        let toks = lex("NOT(!TRUE)", "t.wll").unwrap();
        assert_eq!(toks[0].kind, TokenKind::Not);
        assert_eq!(toks[1].kind, TokenKind::LParen);
        assert_eq!(toks[2].kind, TokenKind::Bang);
        assert_eq!(toks[3].kind, TokenKind::True);
        assert_eq!(toks[4].kind, TokenKind::RParen);
        assert_eq!(toks[5].kind, TokenKind::Eof);
    }

    #[test]
    fn lex_mut_keyword() {
        // v0.6 §1.4: `MUT` is a contextual keyword — tokenized as
        // `Mut`. The parser decides when it has special meaning
        // (only after `LET`).
        let toks = lex("MUT", "t.wll").unwrap();
        assert_eq!(toks[0].kind, TokenKind::Mut);
        let toks = lex("LET MUT(x, 0)", "t.wll").unwrap();
        assert_eq!(toks[0].kind, TokenKind::Let);
        assert_eq!(toks[1].kind, TokenKind::Mut);
        assert_eq!(toks[2].kind, TokenKind::LParen);
        assert_eq!(toks[3].kind, TokenKind::Ident("x".into()));
        assert_eq!(toks[4].kind, TokenKind::Comma);
        assert_eq!(toks[5].kind, TokenKind::Integer(0));
    }

    #[test]
    fn lex_interpolated_string_basic() {
        // v0.6 §1.8: `"hi ${name}"` produces three bracketing tokens
        // around the inner expression tokens.
        let toks = lex("\"hi ${name}\"", "t.wll").unwrap();
        assert_eq!(toks[0].kind, TokenKind::StrStart);
        assert_eq!(toks[1].kind, TokenKind::StrText("hi ".into()));
        assert_eq!(toks[2].kind, TokenKind::Ident("name".into()));
        assert_eq!(toks[3].kind, TokenKind::StrEnd);
    }

    #[test]
    fn lex_interpolated_string_only_text() {
        // No `${` → still a plain `StringLit` (fast path).
        let toks = lex("\"plain\"", "t.wll").unwrap();
        assert_eq!(toks[0].kind, TokenKind::StringLit("plain".into()));
    }

    #[test]
    fn lex_interpolated_string_with_expr() {
        // `"x = ${+(a, b)}"` should bracket `+(a, b)` as inner tokens.
        let toks = lex("\"x = ${+(a, b)}\"", "t.wll").unwrap();
        assert_eq!(toks[0].kind, TokenKind::StrStart);
        assert_eq!(toks[1].kind, TokenKind::StrText("x = ".into()));
        assert_eq!(toks[2].kind, TokenKind::Plus);
        assert_eq!(toks[3].kind, TokenKind::LParen);
        assert_eq!(toks[4].kind, TokenKind::Ident("a".into()));
        assert_eq!(toks[5].kind, TokenKind::Comma);
        assert_eq!(toks[6].kind, TokenKind::Ident("b".into()));
        assert_eq!(toks[7].kind, TokenKind::RParen);
        assert_eq!(toks[8].kind, TokenKind::StrEnd);
    }

    #[test]
    fn lex_string_dollar_escape() {
        // v0.6 §1.8: `\$` produces a literal `$`.
        let toks = lex(r#""a\$b""#, "t.wll").unwrap();
        assert_eq!(toks[0].kind, TokenKind::StringLit("a$b".into()));
    }

    #[test]
    fn lex_string_b_f_escape() {
        // v0.6 §1.8: `\b` is backspace (U+0008), `\f` is form-feed
        // (U+000C). The v0.3 lexer already supported these informally.
        let toks = lex(r#""a\bb\fc""#, "t.wll").unwrap();
        if let TokenKind::StringLit(s) = &toks[0].kind {
            assert_eq!(s, "a\x08b\x0cc");
        } else {
            panic!("expected StringLit, got {:?}", toks[0].kind);
        }
    }

    #[test]
    fn lex_interpolation_two_consecutive_segments() {
        // v0.6 §1.8 §1.8.1: `"hi ${a}${b}!"` must lex as one bracketed
        // interpolated string with two expressions in a row — no
        // empty StrText, no nested StrStart/StrEnd. Token shape:
        //
        //   StrStart, StrText("hi "), Ident(a), Ident(b), StrText("!"), StrEnd
        let toks = lex(r#""hi ${a}${b}!""#, "t.wll").unwrap();
        let kinds: Vec<&TokenKind> = toks.iter().map(|t| &t.kind).collect();
        let expected = [
            TokenKind::StrStart,
            TokenKind::StrText("hi ".into()),
            TokenKind::Ident("a".into()),
            TokenKind::Ident("b".into()),
            TokenKind::StrText("!".into()),
            TokenKind::StrEnd,
            TokenKind::Eof,
        ];
        assert_eq!(kinds.len(), expected.len(), "got {:?}", kinds);
        for (i, (got, exp)) in kinds.iter().zip(expected.iter()).enumerate() {
            assert_eq!(
                std::mem::discriminant(*got),
                std::mem::discriminant(exp),
                "token {} mismatch: got {:?}, expected variant of {:?}",
                i,
                got,
                exp
            );
        }
    }

    #[test]
    fn lex_interpolation_two_int_segments() {
        // Same as above, but with integer literals inside the
        // interpolations — catches a regression where `read_interp_body`
        // doesn't skip over the `${` of the second segment.
        let toks = lex(r#""${1}${2}""#, "t.wll").unwrap();
        // Print tokens for debugging.
        for (i, t) in toks.iter().enumerate() {
            eprintln!("  [{}] {:?}", i, t.kind);
        }
        let kinds: Vec<&TokenKind> = toks.iter().map(|t| &t.kind).collect();
        // Find both integer tokens — they should both be present.
        let int_count = kinds
            .iter()
            .filter(|k| matches!(k, TokenKind::Integer(_)))
            .count();
        assert_eq!(int_count, 2, "expected 2 Integer tokens, got {:?}", kinds);
    }

    #[test]
    fn lex_interpolation_unterminated_brace() {
        // Missing `}` inside an interpolation is detected by the
        // outer string loop: the inner `"` closes the string first,
        // so the test really checks that the lexer rejects a string
        // that ends mid-interpolation.
        //
        // ⚠ [v0.11.3 / R2-005] 断言的码从 E0001 变成 E0002:插值体里的
        // 嵌套字符串字面量现在被**跳过**而不是被拒(规范 §1.8 的
        // `interpolation = "${" Expression "}"`,字符串字面量是 `Expression`
        // 的合法组成)。于是这个输入的失败点变成「那个嵌套字符串没有闭合
        // 的 `"`」⇒ 词法码 E0002(未终止字符串),不再是 E0001(非法字符)。
        let err = lex("\"hi ${name\"", "t.wll").unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0002);
    }

    #[test]
    fn lex_string_with_escape() {
        let toks = lex(r#""hello\nworld""#, "t.wll").unwrap();
        match &toks[0].kind {
            TokenKind::StringLit(s) => assert_eq!(s, "hello\nworld"),
            _ => panic!("expected string"),
        }
    }

    #[test]
    fn lex_unterminated_string() {
        let err = lex(r#""unterminated"#, "t.wll").unwrap_err();
        let d = err.diagnostic();
        assert_eq!(d.code, ErrorCode::E0002);
    }

    #[test]
    fn lex_illegal_char() {
        let err = lex("@", "t.wll").unwrap_err();
        let d = err.diagnostic();
        assert_eq!(d.code, ErrorCode::E0001);
    }

    #[test]
    fn lex_nested_block_comment() {
        let toks = lex("/* outer /* inner */ still comment */ x", "t.wll").unwrap();
        // After comment, identifier "x" should be the last non-EOF token.
        let x = toks
            .iter()
            .find(|t| matches!(&t.kind, TokenKind::Ident(s) if s == "x"));
        assert!(
            x.is_some(),
            "expected to find identifier x after nested comment"
        );
    }

    // v0.3 §3.1 (P3-011): identifiers may contain Chinese (or any
    // non-ASCII letter). The lexer now walks UTF-8 code points in
    // `read_ident_or_keyword`. These tests cover the multi-byte
    // path that the parser-level chain (`spec_v3_alignment`) only
    // exercises end-to-end.
    #[test]
    fn lex_chinese_identifier_token() {
        let toks = lex("计数", "t.wll").unwrap();
        let id = toks
            .iter()
            .find(|t| matches!(&t.kind, TokenKind::Ident(s) if s == "计数"));
        assert!(id.is_some(), "expected identifier 计数, got {:?}", toks);
    }

    #[test]
    fn lex_mixed_ascii_and_chinese_identifier() {
        // Mixed scripts inside one identifier — the UTF-8 walk
        // must accept the ASCII prefix and the multi-byte tail
        // together.
        let toks = lex("count计数", "t.wll").unwrap();
        let id = toks.iter().find_map(|t| match &t.kind {
            TokenKind::Ident(s) if s == "count计数" => Some(()),
            _ => None,
        });
        assert!(
            id.is_some(),
            "expected identifier count计数, got {:?}",
            toks
        );
    }

    #[test]
    fn lex_two_byte_utf8_identifier() {
        // 2-byte UTF-8 (Latin-1 supplement, e.g. é = 0xC3 0xA9).
        // Exercises the `b < 0xE0` branch in read_ident_or_keyword.
        let toks = lex("café", "t.wll").unwrap();
        let id = toks.iter().find_map(|t| match &t.kind {
            TokenKind::Ident(s) if s == "café" => Some(()),
            _ => None,
        });
        assert!(id.is_some(), "expected identifier café, got {:?}", toks);
    }

    #[test]
    fn lex_four_byte_utf8_identifier() {
        // 4-byte UTF-8 (supplementary plane, e.g. 😀 = 0xF0 0x9F
        // 0x98 0x80). Exercises the `b < 0xF8` branch.
        let toks = lex("x😀y", "t.wll").unwrap();
        let id = toks.iter().find_map(|t| match &t.kind {
            TokenKind::Ident(s) if s == "x😀y" => Some(()),
            _ => None,
        });
        assert!(id.is_some(), "expected identifier x😀y, got {:?}", toks);
    }

    #[test]
    fn lex_line_comment() {
        let toks = lex("x // comment\ny", "t.wll").unwrap();
        let names: Vec<_> = toks
            .iter()
            .filter_map(|t| match &t.kind {
                TokenKind::Ident(s) => Some(s.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(names, vec!["x", "y"]);
    }

    #[test]
    fn lex_operators() {
        // Phase 2: operators get their own token kinds; the parser later
        // turns them into function calls in Call positions.
        let toks = lex("+ - * / % == != < > <= >= && || !", "t.wll").unwrap();
        let kinds: Vec<_> = toks
            .iter()
            .filter(|t| !matches!(t.kind, TokenKind::Eof))
            .map(|t| match &t.kind {
                TokenKind::Plus => "+",
                TokenKind::Minus => "-",
                TokenKind::Star => "*",
                TokenKind::Slash => "/",
                TokenKind::Percent => "%",
                TokenKind::EqEq => "==",
                TokenKind::Eq => "=",
                TokenKind::BangEq => "!=",
                TokenKind::Lt => "<",
                TokenKind::Gt => ">",
                TokenKind::LtEq => "<=",
                TokenKind::GtEq => ">=",
                TokenKind::AmpAmp => "&&",
                TokenKind::PipePipe => "||",
                TokenKind::Bang => "!",
                TokenKind::Not => "NOT",
                other => panic!("unexpected token {:?}", other),
            })
            .collect();
        assert_eq!(
            kinds,
            vec!["+", "-", "*", "/", "%", "==", "!=", "<", ">", "<=", ">=", "&&", "||", "!"]
        );
    }

    #[test]
    fn as_op_name_round_trip() {
        // Each operator token kind must map back to its source spelling.
        for (kind, expected) in [
            (TokenKind::Plus, "+"),
            (TokenKind::Minus, "-"),
            (TokenKind::Star, "*"),
            (TokenKind::Slash, "/"),
            (TokenKind::Percent, "%"),
            (TokenKind::EqEq, "=="),
            (TokenKind::BangEq, "!="),
            (TokenKind::Lt, "<"),
            (TokenKind::Gt, ">"),
            (TokenKind::LtEq, "<="),
            (TokenKind::GtEq, ">="),
            (TokenKind::AmpAmp, "&&"),
            (TokenKind::PipePipe, "||"),
            (TokenKind::Bang, "!"),
        ] {
            assert_eq!(kind.as_op_name(), Some(expected), "{:?}", kind);
        }
        // Non-operators return None.
        assert_eq!(TokenKind::Ident("foo".into()).as_op_name(), None);
        assert_eq!(TokenKind::Let.as_op_name(), None);
    }

    // ---------------------------------------------------------------------
    // D11-023 — spec §1.1 "UTF-8 encoded (with or without BOM)".
    //
    // Fixtures are built by concatenating the raw BOM at test time rather
    // than committing a BOM'd file: `.gitattributes` sets `*.wll text
    // eol=lf`, and although git's text filter does not touch BOMs, a
    // committed U+FEFF is an *invisible* byte in review.
    //
    // Each test was checked against four wrong implementations to prove it
    // discriminates rather than merely passing. Measured 2026-10-01:
    //
    //   A no strip at all .................. 5 red
    //   B trim_start_matches (over-strip) .. 1 red  (only_one_leading_bom)
    //   C strip after Lexer::new (col +3) .. 3 red
    //   D replace() all U+FEFF (over-fix) .. 2 red
    //
    // Every test below is red under at least one of A-D, so none of them
    // is dead weight. Note that `bom_inside_the_file_is_left_alone` is
    // *designed* to stay green under A and B — it guards against D, not
    // against removing the fix.
    // ---------------------------------------------------------------------

    const BOM: &str = "\u{FEFF}";

    /// The exact program the D11-023 repro used.
    const REPRO: &str = "PRINT(\"hi\", 1, 2, 3);";

    /// 1. A leading BOM must not change the token stream at all — same
    ///    kinds *and* same spans, so line/column reporting is unaffected.
    #[test]
    fn d11_023_bom_yields_an_identical_token_stream() {
        let plain = lex(REPRO, "t.wll").unwrap();
        let bommed = lex(&format!("{BOM}{REPRO}"), "t.wll").unwrap();
        assert_eq!(plain, bommed);
        assert!(!plain.is_empty());
    }

    /// 2. The first token must be the bare `PRINT` identifier **and** start
    ///    at column 1. Both halves are load-bearing:
    ///
    ///    - the name assertion makes this test red when the strip is
    ///      removed (without it the identifier is `"\u{FEFF}PRINT"`);
    ///    - the column assertion catches the other plausible wrong shape —
    ///      stripping *after* `Lexer::new` or inside the token loop. `Lexer`
    ///      advances `col` per **byte** and `U+FEFF` is 3 bytes, so that
    ///      variant shifts every column on line 1 by 3 while still
    ///      producing a correctly-named identifier.
    #[test]
    fn d11_023_bom_yields_a_bare_first_token_at_column_one() {
        let toks = lex(&format!("{BOM}{REPRO}"), "t.wll").unwrap();
        match &toks[0].kind {
            TokenKind::Ident(name) => assert_eq!(name, "PRINT", "BOM must not reach the name"),
            other => panic!("expected an Ident, got {other:?}"),
        }
        let (_, col_start, _, _) = toks[0].span;
        assert_eq!(col_start, 1, "first token must start at column 1");
    }

    /// 3. **Exactly one.** A second leading U+FEFF is *not* a BOM, it is a
    ///    ZWNBSP, and P3-011's identifier relaxation consumes it as part of
    ///    the identifier. This is the discriminating case between
    ///    `strip_prefix` and `trim_start_matches`; under the latter the
    ///    second one would vanish and this assertion would fail.
    #[test]
    fn d11_023_only_one_leading_bom_is_stripped() {
        let toks = lex(&format!("{BOM}{BOM}{REPRO}"), "t.wll").unwrap();
        match &toks[0].kind {
            TokenKind::Ident(name) => assert_eq!(
                name,
                &format!("{BOM}PRINT"),
                "the second U+FEFF must survive as identifier content"
            ),
            other => panic!("expected an Ident, got {other:?}"),
        }
    }

    /// 4. A U+FEFF that is *not* at offset 0 is untouched. Two placements
    ///    that matter in practice: inside a string literal, and mid-file
    ///    where the permissive identifier reader will absorb it.
    #[test]
    fn d11_023_bom_inside_the_file_is_left_alone() {
        let toks = lex("PRINT(\"a\u{FEFF}b\");", "t.wll").unwrap();
        let literal = toks
            .iter()
            .find_map(|t| match &t.kind {
                TokenKind::StringLit(s) => Some(s.clone()),
                _ => None,
            })
            .expect("string literal token");
        assert!(
            literal.contains('\u{FEFF}'),
            "U+FEFF inside a string literal must survive, got {literal:?}"
        );
    }

    /// 5. Guards the guard: a file that is *only* a BOM must lex exactly
    ///    like an empty file. The lexer always terminates with a single
    ///    `Eof` token, so this asserts stream equality rather than an
    ///    empty vec — and it catches the strip manufacturing an identifier
    ///    or an E0001 out of a degenerate input.
    #[test]
    fn d11_023_bom_only_input_lexes_like_an_empty_file() {
        assert_eq!(lex(BOM, "t.wll").unwrap(), lex("", "t.wll").unwrap());
    }

    /// 6. A file with no BOM is byte-for-byte unaffected — the common path
    ///    must not regress. Compared against a source that starts with a
    ///    character the lexer already had to skip past.
    #[test]
    fn d11_023_non_bom_input_is_unchanged() {
        let leading_comment = lex("// c\nPRINT(1);", "t.wll").unwrap();
        let bommed = lex(&format!("{BOM}// c\nPRINT(1);"), "t.wll").unwrap();
        assert_eq!(leading_comment, bommed);
    }

    // ---------------------------------------------------------------------
    // Spec §1.1, the *other* normative clause: "换行可以是 `\n`、`\r\n` 或
    // `\r`,三者等价".
    //
    // This clause has been satisfied by the implementation for the whole
    // project and had **no** test pinning it (D11-026). It is pinned here
    // at the lexer level rather than as a probe case, and the reason is
    // worth recording: `.gitattributes` sets `*.wll text eol=lf`, so a
    // committed fixture containing CRLF would be normalised to LF by git
    // and the test would silently stop testing anything. Making it a probe
    // case would require a second byte-injection feature in the harness
    // (`eol`, alongside the `bom` key added by D11-026) that nothing else
    // needs. A lexer-level test cannot be defeated by line-ending
    // normalisation, so it is the honest place for this clause.
    // ---------------------------------------------------------------------

    /// 7. All three line terminators are equivalent. Only the *line/column*
    ///    bookkeeping is allowed to differ, so the comparison drops spans.
    fn kinds_of(src: &str) -> Vec<TokenKind> {
        lex(src, "t.wll")
            .unwrap()
            .into_iter()
            .map(|t| t.kind)
            .collect()
    }

    #[test]
    fn d11_026_lf_crlf_and_cr_are_equivalent() {
        let lf = kinds_of("LET a 1\nPRINT a\n");
        let crlf = kinds_of("LET a 1\r\nPRINT a\r\n");
        let cr = kinds_of("LET a 1\rPRINT a\r");
        assert_eq!(lf, crlf, "CRLF must lex identically to LF");
        assert_eq!(lf, cr, "a bare CR must lex identically to LF");
    }

    /// 8. The terminator inside a **string literal** is normalised too:
    ///    `SPLIT_LINES` (spec §10.5, R1 `std.str`) strips the trailing CR.
    ///    Without this, a CRLF checkout would leak `\r` into string values.
    #[test]
    fn d11_026_crlf_inside_a_block_comment_is_absorbed() {
        assert_eq!(
            kinds_of("/* a\r\nb */ PRINT 1"),
            kinds_of("/* a\nb */ PRINT 1")
        );
    }
}
