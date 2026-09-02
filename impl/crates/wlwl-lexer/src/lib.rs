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

use wlwl_error::{extract_line, ErrorCode, Location, WlwlDiagnostic, WlwlError, WlwlResult};

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
        while let Some(b) = self.peek() {
            if b.is_ascii_alphanumeric() || b == b'_' {
                self.bump();
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
        let mut s = String::new();
        loop {
            match self.peek() {
                Some(b'"') => {
                    self.bump();
                    let end_col = self.col;
                    return Ok(Token {
                        kind: TokenKind::StringLit(s),
                        span: (line, col, line, end_col),
                    });
                }
                Some(b'\\') => {
                    self.bump();
                    match self.bump() {
                        Some(b'n') => s.push('\n'),
                        Some(b't') => s.push('\t'),
                        Some(b'r') => s.push('\r'),
                        Some(b'\\') => s.push('\\'),
                        Some(b'"') => s.push('"'),
                        Some(b'0') => s.push('\0'),
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
                    s.push(b as char);
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
