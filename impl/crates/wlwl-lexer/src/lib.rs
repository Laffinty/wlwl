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
pub fn lex(input: &str, file: &str) -> WlwlResult<Vec<Token>> {
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
                        return Ok(vec![Token { kind: TokenKind::StringLit(s), span }]);
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
    /// `}`, which this method consumes). Two rules:
    ///
    ///   1. Top-level `{` / `}` count for depth (so `{LET(x, {y: 1}), x}`
    ///      parses correctly).
    ///   2. `${...}` pairs inside the body are **skipped over** — they
    ///      belong to the OUTER string's next interpolation, not the
    ///      inner expression. Without this skip, `"${1}${2}"` lexes
    ///      "1${2}" as the body of the first `${...}`, and the
    ///      recursive `lex()` chokes on the bare `$`.
    fn read_interp_body(&mut self) -> WlwlResult<Vec<Token>> {
        let start = self.pos;
        let mut depth: u32 = 1;
        let mut i = self.pos;
        while i < self.src.len() {
            let b = self.src[i];
            // Skip nested `${...}` pairs: they belong to the outer
            // string, not the inner expression we are collecting.
            if b == b'$' && self.src.get(i + 1) == Some(&b'{') {
                i += 2; // consume `${`
                let mut nest_depth: u32 = 1;
                while i < self.src.len() && nest_depth > 0 {
                    match self.src[i] {
                        b'{' => nest_depth += 1,
                        b'}' => nest_depth -= 1,
                        _ => {}
                    }
                    if nest_depth > 0 {
                        i += 1;
                    }
                }
                if nest_depth != 0 {
                    return Err(self.err(
                        ErrorCode::E0002,
                        "unterminated nested string interpolation (missing `}`)",
                        self.line,
                        self.col,
                    ));
                }
                i += 1; // skip the closing `}` of the nested `${...}`
                continue;
            }
            match b {
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                b'"' => {
                    // Nested string literals are not allowed inside
                    // `${...}` per spec grammar — they would break the
                    // simple brace-counting strategy. Report E0001.
                    return Err(self.err(
                        ErrorCode::E0001,
                        "nested string literal inside `${...}` interpolation is not allowed",
                        self.line,
                        self.col,
                    ));
                }
                _ => {}
            }
            i += 1;
        }
        if depth != 0 {
            return Err(self.err(
                ErrorCode::E0002,
                "unterminated string interpolation (missing `}`)",
                self.line,
                self.col,
            ));
        }
        let inner_bytes = &self.src[start..i];
        let inner_str = std::str::from_utf8(inner_bytes).unwrap_or("");
        // Skip past the inner bytes plus the closing `}`.
        while self.pos < i + 1 {
            self.bump();
        }
        // Recursively lex the inner expression as a fresh source
        // buffer; strip the trailing EOF the recursive lexer emits.
        let mut inner_tokens = lex(inner_str, &self.file)?;
        if matches!(inner_tokens.last(), Some(Token { kind: TokenKind::Eof, .. })) {
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
        assert_eq!(
            kinds.len(),
            expected.len(),
            "got {:?}",
            kinds
        );
        for (i, (got, exp)) in kinds.iter().zip(expected.iter()).enumerate() {
            assert_eq!(
                std::mem::discriminant(*got),
                std::mem::discriminant(exp),
                "token {} mismatch: got {:?}, expected variant of {:?}",
                i, got, exp
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
        let int_count = kinds.iter().filter(|k| matches!(k, TokenKind::Integer(_))).count();
        assert_eq!(int_count, 2, "expected 2 Integer tokens, got {:?}", kinds);
    }

    #[test]
    fn lex_interpolation_unterminated_brace() {
        // Missing `}` inside an interpolation is detected by the
        // outer string loop: the inner `"` closes the string first,
        // so the test really checks that the lexer rejects a string
        // that ends mid-interpolation. The current implementation
        // refuses nested string literals inside `${...}` with E0001.
        let err = lex("\"hi ${name\"", "t.wll").unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0001);
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
}
