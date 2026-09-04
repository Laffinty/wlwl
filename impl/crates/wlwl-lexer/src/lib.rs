//! WLWL lexical analyzer.
//!
//! Phase 2 token set per v0.3 §3:
//! - Keywords: TRUE, FALSE, NULL, LET, FUN, RETURN, IF, WHILE, FOR, BREAK,
//!             CONTINUE, CLASS, NEW, THIS
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

use wlwl_error::{extract_line, ErrorCode, Location, Suggestion, WlwlDiagnostic, WlwlError, WlwlResult};

/// A token in the source code.
#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // Keywords (v0.3 §3.2 — full set; Phase 1+2 emits all of these)
    True,
    False,
    Null,
    Let,
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
    // Literals
    Integer(i64),
    Float(f64),
    StringLit(String),
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
    Plus,         // +
    Minus,        // -
    Star,         // *
    Slash,        // /
    Percent,      // %
    EqEq,         // ==
    BangEq,       // !=
    /// P3-011 §8.2: single `=` is used in default-parameter
    /// bindings (`name = expr`) and in future let-bindings. The
    /// lexer must NOT collapse a bare `=` into `==`; `==` is its own
    /// token and is matched first.
    Eq,
    Lt,           // <
    Gt,           // >
    LtEq,         // <=
    GtEq,         // >=
    AmpAmp,       // &&
    PipePipe,     // ||
    Bang,         // !
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
            TokenKind::BangEq => Some("!="),
            TokenKind::Lt => Some("<"),
            TokenKind::Gt => Some(">"),
            TokenKind::LtEq => Some("<="),
            TokenKind::GtEq => Some(">="),
            TokenKind::AmpAmp => Some("&&"),
            TokenKind::PipePipe => Some("||"),
            TokenKind::Bang => Some("!"),
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
                ).into(),
            }),
            ErrorCode::E0002 => d.with_suggestion(Suggestion::Note {
                description:
                    "add a closing `\"` before end of line, or split into \
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
            let v: f64 = text
                .parse()
                .map_err(|_| self.err(ErrorCode::E0001, format!("invalid float '{}'", text), line, col))?;
            let end_col = self.col;
            (TokenKind::Float(v), (line, col, line, end_col))
        } else {
            let v: i64 = text
                .parse()
                .map_err(|_| self.err(ErrorCode::E0001, format!("invalid integer '{}'", text), line, col))?;
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
        let text = std::str::from_utf8(&self.src[start..self.pos]).unwrap().to_string();
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
            _ => TokenKind::Ident(text),
        };
        Ok(Token {
            kind,
            span: (line, col, line, end_col),
        })
    }

    fn read_string(&mut self) -> WlwlResult<Token> {
        let line = self.line;
        let col = self.col;
        self.bump(); // opening '"'
        // v0.3 §4.2: strings are double-quoted, may contain any UTF-8
        // (including 中文 — see also §3.1 identifier note). The lexer
        // previously pushed individual bytes as `char`, which mangles
        // multi-byte sequences into Latin-1 mojibake. We now accumulate
        // raw bytes and decode once at the closing quote.
        let mut s_bytes: Vec<u8> = Vec::new();
        loop {
            match self.peek() {
                Some(b'"') => {
                    self.bump();
                    let end_col = self.col;
                    let s = String::from_utf8(s_bytes).map_err(|e| {
                        // Should be unreachable: we only ever push
                        // valid UTF-8 sequences. Report E0001 if it
                        // somehow happens.
                        self.err(
                            ErrorCode::E0001,
                            format!("invalid UTF-8 in string literal: {}", e),
                            line,
                            col,
                        )
                    })?;
                    return Ok(Token {
                        kind: TokenKind::StringLit(s),
                        span: (line, col, line, end_col),
                    });
                }
                Some(b'\\') => {
                    self.bump();
                    match self.bump() {
                        Some(b'n') => s_bytes.push(b'\n'),
                        Some(b't') => s_bytes.push(b'\t'),
                        Some(b'r') => s_bytes.push(b'\r'),
                        Some(b'\\') => s_bytes.push(b'\\'),
                        Some(b'"') => s_bytes.push(b'"'),
                        Some(b'0') => s_bytes.push(b'\0'),
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
                Some(b'\n') | None => {
                    return Err(self.err(
                        ErrorCode::E0002,
                        "unterminated string",
                        line,
                        col,
                    ));
                }
                Some(b) => {
                    s_bytes.push(b);
                    self.bump();
                }
            }
        }
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
                    return Err(self.err(ErrorCode::E0003, "unterminated block comment", line, col));
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
                    tokens.push(Token { kind: TokenKind::LParen, span: (line, col, line, self.col) });
                }
                b')' => {
                    self.bump();
                    tokens.push(Token { kind: TokenKind::RParen, span: (line, col, line, self.col) });
                }
                b'[' => {
                    self.bump();
                    tokens.push(Token { kind: TokenKind::LBracket, span: (line, col, line, self.col) });
                }
                b']' => {
                    self.bump();
                    tokens.push(Token { kind: TokenKind::RBracket, span: (line, col, line, self.col) });
                }
                b',' => {
                    self.bump();
                    tokens.push(Token { kind: TokenKind::Comma, span: (line, col, line, self.col) });
                }
                b';' => {
                    self.bump();
                    tokens.push(Token { kind: TokenKind::Semicolon, span: (line, col, line, self.col) });
                }
                b':' => {
                    self.bump();
                    tokens.push(Token { kind: TokenKind::Colon, span: (line, col, line, self.col) });
                }
                b'.' => {
                    self.bump();
                    tokens.push(Token { kind: TokenKind::Dot, span: (line, col, line, self.col) });
                }
                b'"' => tokens.push(self.read_string()?),
                b'/' if self.peek_at(1) == Some(b'/') => {
                    self.skip_line_comment();
                }
                b'/' if self.peek_at(1) == Some(b'*') => {
                    self.skip_block_comment()?;
                }
                b'/' => {
                    self.bump();
                    tokens.push(Token { kind: TokenKind::Slash, span: (line, col, line, self.col) });
                }
                b'+' => {
                    self.bump();
                    tokens.push(Token { kind: TokenKind::Plus, span: (line, col, line, self.col) });
                }
                b'-' => {
                    self.bump();
                    tokens.push(Token { kind: TokenKind::Minus, span: (line, col, line, self.col) });
                }
                b'*' => {
                    self.bump();
                    tokens.push(Token { kind: TokenKind::Star, span: (line, col, line, self.col) });
                }
                b'%' => {
                    self.bump();
                    tokens.push(Token { kind: TokenKind::Percent, span: (line, col, line, self.col) });
                }
                b'=' if self.peek_at(1) == Some(b'=') => {
                    self.bump();
                    self.bump();
                    tokens.push(Token { kind: TokenKind::EqEq, span: (line, col, line, self.col) });
                }
                b'=' => {
                    // P3-011 §8.2: single `=` is the default-parameter
                    // separator. Lex as TokenKind::Eq.
                    self.bump();
                    tokens.push(Token { kind: TokenKind::Eq, span: (line, col, line, self.col) });
                }
                b'!' if self.peek_at(1) == Some(b'=') => {
                    self.bump();
                    self.bump();
                    tokens.push(Token { kind: TokenKind::BangEq, span: (line, col, line, self.col) });
                }
                b'!' => {
                    self.bump();
                    tokens.push(Token { kind: TokenKind::Bang, span: (line, col, line, self.col) });
                }
                b'<' if self.peek_at(1) == Some(b'=') => {
                    self.bump();
                    self.bump();
                    tokens.push(Token { kind: TokenKind::LtEq, span: (line, col, line, self.col) });
                }
                b'<' => {
                    self.bump();
                    tokens.push(Token { kind: TokenKind::Lt, span: (line, col, line, self.col) });
                }
                b'>' if self.peek_at(1) == Some(b'=') => {
                    self.bump();
                    self.bump();
                    tokens.push(Token { kind: TokenKind::GtEq, span: (line, col, line, self.col) });
                }
                b'>' => {
                    self.bump();
                    tokens.push(Token { kind: TokenKind::Gt, span: (line, col, line, self.col) });
                }
                b'&' if self.peek_at(1) == Some(b'&') => {
                    self.bump();
                    self.bump();
                    tokens.push(Token { kind: TokenKind::AmpAmp, span: (line, col, line, self.col) });
                }
                b'|' if self.peek_at(1) == Some(b'|') => {
                    self.bump();
                    self.bump();
                    tokens.push(Token { kind: TokenKind::PipePipe, span: (line, col, line, self.col) });
                }
                c if c.is_ascii_digit() => tokens.push(self.read_number()?),
                c if c.is_ascii_alphabetic() || c == b'_' => tokens.push(self.read_ident_or_keyword()?),
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
        let toks = lex("42 3.14 0", "t.wl").unwrap();
        assert!(matches!(toks[0].kind, TokenKind::Integer(42)));
        assert!(matches!(toks[1].kind, TokenKind::Float(f) if (f - 3.14).abs() < 1e-9));
        assert!(matches!(toks[2].kind, TokenKind::Integer(0)));
    }

    #[test]
    fn lex_keywords() {
        let toks = lex("TRUE FALSE NULL LET", "t.wl").unwrap();
        assert_eq!(toks[0].kind, TokenKind::True);
        assert_eq!(toks[1].kind, TokenKind::False);
        assert_eq!(toks[2].kind, TokenKind::Null);
        assert_eq!(toks[3].kind, TokenKind::Let);
    }

    #[test]
    fn lex_string_with_escape() {
        let toks = lex(r#""hello\nworld""#, "t.wl").unwrap();
        match &toks[0].kind {
            TokenKind::StringLit(s) => assert_eq!(s, "hello\nworld"),
            _ => panic!("expected string"),
        }
    }

    #[test]
    fn lex_unterminated_string() {
        let err = lex(r#""unterminated"#, "t.wl").unwrap_err();
        let d = err.diagnostic();
        assert_eq!(d.code, ErrorCode::E0002);
    }

    #[test]
    fn lex_illegal_char() {
        let err = lex("@", "t.wl").unwrap_err();
        let d = err.diagnostic();
        assert_eq!(d.code, ErrorCode::E0001);
    }

    #[test]
    fn lex_nested_block_comment() {
        let toks = lex("/* outer /* inner */ still comment */ x", "t.wl").unwrap();
        // After comment, identifier "x" should be the last non-EOF token.
        let x = toks.iter().find(|t| matches!(&t.kind, TokenKind::Ident(s) if s == "x"));
        assert!(x.is_some(), "expected to find identifier x after nested comment");
    }

    // v0.3 §3.1 (P3-011): identifiers may contain Chinese (or any
    // non-ASCII letter). The lexer now walks UTF-8 code points in
    // `read_ident_or_keyword`. These tests cover the multi-byte
    // path that the parser-level chain (`spec_v3_alignment`) only
    // exercises end-to-end.
    #[test]
    fn lex_chinese_identifier_token() {
        let toks = lex("计数", "t.wl").unwrap();
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
        let toks = lex("count计数", "t.wl").unwrap();
        let id = toks.iter().find_map(|t| match &t.kind {
            TokenKind::Ident(s) if s == "count计数" => Some(()),
            _ => None,
        });
        assert!(id.is_some(), "expected identifier count计数, got {:?}", toks);
    }

    #[test]
    fn lex_two_byte_utf8_identifier() {
        // 2-byte UTF-8 (Latin-1 supplement, e.g. é = 0xC3 0xA9).
        // Exercises the `b < 0xE0` branch in read_ident_or_keyword.
        let toks = lex("café", "t.wl").unwrap();
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
        let toks = lex("x😀y", "t.wl").unwrap();
        let id = toks.iter().find_map(|t| match &t.kind {
            TokenKind::Ident(s) if s == "x😀y" => Some(()),
            _ => None,
        });
        assert!(id.is_some(), "expected identifier x😀y, got {:?}", toks);
    }

    #[test]
    fn lex_line_comment() {
        let toks = lex("x // comment\ny", "t.wl").unwrap();
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
        let toks = lex("+ - * / % == != < > <= >= && || !", "t.wl").unwrap();
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
