//! WLWL parser (hand-written recursive descent).
//!
//! Phase 2 grammar (v0.3 §3-§13, subset):
//! ```text
//! program     := block
//! block       := (statement ';')* (statement)?
//! statement   := expression
//! expression  := let | control_expr | error_expr | import_export | call_or_ident | literal | '(' block ')'
//! let         := 'LET' '(' IDENT ',' expression ')'
//! control_expr :=
//!     | 'IF'    '(' expr ',' expr (',' expr)? ')'
//!     | 'WHILE' '(' expr ',' expr ')'
//!     | 'FOR'   '(' IDENT ',' expr ',' expr ')'
//!     | 'FUN'   '(' '(' (IDENT (',' IDENT)*)? ')' ',' expr ')'
//!     | 'RETURN' '(' (expr)? ')'
//!     | 'BREAK'   '(' ')'
//!     | 'CONTINUE' '(' ')'
//! error_expr  :=
//!     | 'OK'     '(' expr ')'
//!     | 'ERR'    '(' expr ')'
//!     | 'PANIC'  '(' expr ')'
//!     | 'TRY'    '(' expr ')'
//!     | 'IS_OK'  '(' expr ')'
//!     | 'IS_ERR' '(' expr ')'
//!     | 'OR_DIE' '(' expr ',' expr ')'
//! import_export :=
//!     | 'IMPORT' '(' STRING ',' '[' (import_item (',' import_item)*)? ']' ')'
//!     | 'EXPORT' '(' '[' (import_item (',' import_item)*)? ']' ')'
//! import_item := IDENT (':' IDENT)?
//! call_or_ident := IDENT_OR_OP ( '(' (expression (',' expression)*)? ')' )?
//! literal     := INTEGER | FLOAT | STRING | TRUE | FALSE | NULL | array | dict
//! array       := '[' (expression (',' expression)*)? ']'
//! dict        := '[' (expression ':' expression (',' expression ':' expression)*)? ']'
//! ```
//!
//! Error codes (v0.3 §14.4):
//! - E0010 expected expression
//! - E0011 expected ')'
//! - E0012 expected ','
//! - E0013 expected ';'
//! - E0014 RETURN/BREAK/CONTINUE in illegal position
//! - E0020 undefined name (emitted at eval time, not parse)
//! - E0043 namespace path syntax error

use wlwl_ast::{Expr, FunParam, ImportName, Literal, Span, TypeAnnotation, TypeExpr};
use wlwl_error::{extract_line, ErrorCode, Location, WlwlDiagnostic, WlwlError, WlwlResult};
use wlwl_lexer::{lex, Token, TokenKind};

/// Parse source code into a single block expression (the whole program).
pub fn parse(input: &str, file: &str) -> WlwlResult<Expr> {
    let toks = lex(input, file)?;
    let mut p = Parser {
        toks,
        pos: 0,
        file: file.to_string(),
        source: input.to_string(),
    };
    let block = p.parse_block(false)?;
    p.expect(TokenKind::Eof)?;
    Ok(block)
}

struct Parser {
    toks: Vec<Token>,
    pos: usize,
    file: String,
    /// Original source text, used to populate `source_line` on diagnostics
    /// (v0.3 §14.2).
    source: String,
}

impl Parser {
    fn peek(&self) -> &TokenKind {
        &self.toks[self.pos].kind
    }

    fn peek_at(&self, offset: usize) -> &TokenKind {
        // Safe: lex always emits an EOF sentinel, so any offset is in-bounds.
        &self.toks[self.pos + offset].kind
    }

    fn span_here(&self) -> (u32, u32, u32, u32) {
        self.toks[self.pos].span
    }

    fn advance(&mut self) -> Token {
        let t = self.toks[self.pos].clone();
        if self.pos < self.toks.len() - 1 {
            self.pos += 1;
        }
        t
    }

    fn at_eof(&self) -> bool {
        matches!(self.peek(), TokenKind::Eof)
    }

    fn err_at(&self, code: ErrorCode, message: impl Into<String>, span: (u32, u32, u32, u32)) -> WlwlError {
        let loc = Location {
            file: self.file.clone(),
            line: span.0,
            col: span.1,
            line_end: span.2,
            col_end: span.3,
        };
        let mut d = WlwlDiagnostic::new(code, message, loc);
        if let Some(s) = extract_line(&self.source, span.0) {
            d = d.with_source_line(s);
        }
        d.into()
    }

    fn expect(&mut self, kind: TokenKind) -> WlwlResult<Token> {
        if std::mem::discriminant(self.peek()) == std::mem::discriminant(&kind) {
            Ok(self.advance())
        } else {
            let (line, col, _, _) = self.span_here();
            let expected = match kind {
                TokenKind::RParen => "')'",
                TokenKind::Comma => "','",
                TokenKind::Semicolon => "';'",
                TokenKind::Eof => "end of file",
                _ => "<token>",
            };
            Err(self.err_at(
                ErrorCode::E0011,
                format!("expected {}, got {:?}", expected, self.peek()),
                (line, col, line, col),
            ))
        }
    }

    fn expect_specific(&mut self, code: ErrorCode, expected: &str) -> WlwlResult<Token> {
        if self.at_eof() {
            let (line, col, _, _) = self.span_here();
            return Err(self.err_at(
                code,
                format!("expected {}, got end of file", expected),
                (line, col, line, col),
            ));
        }
        // Peek first; only consume if it matches. This way a mismatch
        // doesn't eat the wrong token.
        let peeked = self.peek().clone();
        let actual = format!("{:?}", peeked);
        let want = match expected {
            "'('" => TokenKind::LParen,
            "')'" => TokenKind::RParen,
            "'['" => TokenKind::LBracket,
            "']'" => TokenKind::RBracket,
            "','" => TokenKind::Comma,
            "';'" => TokenKind::Semicolon,
            "':'" => TokenKind::Colon,
            "'.'" => TokenKind::Dot,
            "'LET'" => TokenKind::Let,
            "'IF'" => TokenKind::If,
            "'WHILE'" => TokenKind::While,
            "'FOR'" => TokenKind::For,
            "'FUN'" => TokenKind::Fun,
            "'RETURN'" => TokenKind::Return,
            "'BREAK'" => TokenKind::Break,
            "'CONTINUE'" => TokenKind::Continue,
            "'OK'" => TokenKind::Ok,
            "'ERR'" => TokenKind::Err,
            "'PANIC'" => TokenKind::Panic,
            "'TRY'" => TokenKind::Try,
            "'IS_OK'" => TokenKind::IsOk,
            "'IS_ERR'" => TokenKind::IsErr,
            "'OR_DIE'" => TokenKind::OrDie,
            "'IMPORT'" => TokenKind::Import,
            "'EXPORT'" => TokenKind::Export,
            other => {
                return Err(self.err_at(
                    code,
                    format!("internal: unknown expected token '{}'", other),
                    self.toks[self.pos].span,
                ));
            }
        };
        if std::mem::discriminant(&peeked) == std::mem::discriminant(&want) {
            Ok(self.advance())
        } else {
            let (line, col, _, _) = self.span_here();
            Err(self.err_at(
                code,
                format!("expected {}, got {}", expected, actual),
                (line, col, line, col),
            ))
        }
    }

    /// Parse a block. If `require_semi` is true, expects ';' after each statement.
    fn parse_block(&mut self, in_paren: bool) -> WlwlResult<Expr> {
        let start_span = self.span_here();
        let mut stmts = Vec::new();

        if matches!(self.peek(), TokenKind::RParen) {
            // empty
        } else {
            loop {
                if self.at_eof() || (in_paren && matches!(self.peek(), TokenKind::RParen)) {
                    break;
                }
                let e = self.parse_expr()?;
                stmts.push(e);
                if matches!(self.peek(), TokenKind::Semicolon) {
                    self.advance();
                    // After ';', continue collecting statements.
                    if matches!(self.peek(), TokenKind::RParen | TokenKind::Eof) {
                        break;
                    }
                } else {
                    if !in_paren {
                        if self.at_eof() {
                            break;
                        }
                        let (line, col, _, _) = self.span_here();
                        return Err(self.err_at(
                            ErrorCode::E0013,
                            "expected ';' after expression",
                            (line, col, line, col),
                        ));
                    }
                    break;
                }
            }
        }

        let end_span = self.span_here();
        let span = Span {
            file: self.file.clone(),
            line_start: start_span.0,
            col_start: start_span.1,
            line_end: end_span.2,
            col_end: end_span.3,
        };

        if stmts.is_empty() {
            return Ok(Expr::Literal(Literal::Null, span));
        }
        if stmts.len() == 1 {
            return Ok(stmts.into_iter().next().unwrap());
        }
        Ok(Expr::Block { exprs: stmts, span })
    }

    fn parse_expr(&mut self) -> WlwlResult<Expr> {
        let (line, col, _, _) = self.span_here();
        let kind = self.peek().clone();
        // Unary-minus sugar: `-x` desugars to `-(0, x)`. Only triggered
        // when `-` is NOT followed by `(`, so binary minus like
        // `-(a, b)` continues to work.
        if matches!(kind, TokenKind::Minus) && !matches!(self.peek_at(1), TokenKind::LParen) {
            let (l2, c2, _, _) = self.span_here();
            self.advance(); // '-'
            let inner = self.parse_expr()?;
            let (_, _, le, ce) = self.span_here();
            return Ok(Expr::Call {
                name: "-".to_string(),
                args: vec![Expr::Literal(Literal::Integer(0), Span {
                    file: self.file.clone(),
                    line_start: l2,
                    col_start: c2,
                    line_end: l2,
                    col_end: c2,
                }), inner],
                span: Span {
                    file: self.file.clone(),
                    line_start: l2,
                    col_start: c2,
                    line_end: le,
                    col_end: ce,
                },
            });
        }
        match kind {
            TokenKind::Let => self.parse_let(),
            // §7 control flow
            TokenKind::If => self.parse_if(),
            TokenKind::While => self.parse_while(),
            TokenKind::For => self.parse_for(),
            TokenKind::Fun => self.parse_fun(),
            TokenKind::Return => self.parse_return(),
            TokenKind::Break => self.parse_break(),
            TokenKind::Continue => self.parse_continue(),
            // §12 error handling
            TokenKind::Ok => self.parse_err_ctor("'OK'", |v, s| Expr::Ok { value: v, span: s }),
            TokenKind::Err => self.parse_err_ctor("'ERR'", |v, s| Expr::Err { value: v, span: s }),
            TokenKind::Panic => self.parse_err_ctor("'PANIC'", |v, s| Expr::Panic { value: v, span: s }),
            TokenKind::Try => self.parse_err_ctor("'TRY'", |v, s| Expr::Try { value: v, span: s }),
            TokenKind::IsOk => self.parse_err_ctor("'IS_OK'", |v, s| Expr::IsOk { value: v, span: s }),
            TokenKind::IsErr => self.parse_err_ctor("'IS_ERR'", |v, s| Expr::IsErr { value: v, span: s }),
            TokenKind::OrDie => self.parse_or_die(),
            // §13 modules (single-directory subset; cross-dir/namespace is Phase 4)
            TokenKind::Import => self.parse_import(),
            TokenKind::Export => self.parse_export(),
            // Literals and composites
            TokenKind::Integer(_)
            | TokenKind::Float(_)
            | TokenKind::StringLit(_)
            | TokenKind::True
            | TokenKind::False
            | TokenKind::Null
            | TokenKind::LBracket => self.parse_literal(),
            TokenKind::LParen => self.parse_paren_block(),
            // Identifiers OR operator tokens (in Call position)
            TokenKind::Ident(_) => self.parse_call_or_ident(),
            ref k if k.as_op_name().is_some() => self.parse_call_or_ident(),
            _ => {
                let span = self.span_here();
                Err(self.err_at(
                    ErrorCode::E0010,
                    format!("expected expression, got {:?}", self.peek()),
                    span,
                ))
            }
        }
        .map_err(|e| {
            // Attach file context if not already there (defensive)
            e
        })
    }

    fn parse_let(&mut self) -> WlwlResult<Expr> {
        let (line, col, _, _) = self.span_here();
        self.expect_specific(ErrorCode::E0010, "'LET'")?;
        self.expect_specific(ErrorCode::E0011, "'('")?;
        let name = match self.advance() {
            Token { kind: TokenKind::Ident(s), .. } => s,
            other => {
                return Err(self.err_at(
                    ErrorCode::E0010,
                    format!("expected identifier in LET, got {:?}", other.kind),
                    other.span,
                ));
            }
        };
        // v0.3 Sec. 2.4: optional type annotation, parsed not checked.
        let type_annotation = self.parse_type_annotation()?;
        self.expect_specific(ErrorCode::E0012, "','")?;
        let value = self.parse_expr()?;
        self.expect_specific(ErrorCode::E0011, "')'")?;
        let (_, _, line_end, col_end) = self.span_here();
        Ok(Expr::Let {
            name,
            type_annotation,
            value: Box::new(value),
            span: Span {
                file: self.file.clone(),
                line_start: line,
                col_start: col,
                line_end,
                col_end,
            },
        })
    }

    /// Parse an optional `':' Type` annotation.
    ///
    /// If the next token is not `:`, returns `Ok(None)`. Otherwise
    /// consumes the `:` and a balanced type expression (e.g.
    /// `INTEGER`, `ARRAY[INTEGER]`, `OK[ERR[STRING]]`). The raw text
    /// of the type expression is preserved -- Phase 3 does NOT check
    /// the type. This is per v0.3 `Sec. 2.4` which explicitly defers
    /// checking to a later phase.
    fn parse_type_annotation(&mut self) -> WlwlResult<Option<TypeAnnotation>> {
        if !matches!(self.peek(), TokenKind::Colon) {
            return Ok(None);
        }
        let (sl, sc, _, _) = self.span_here();
        self.advance(); // ':'
        // Now consume a balanced type expression. Stop at top-level
        // `,` or `)`. Allow nested brackets.
        let mut depth: i32 = 0;
        let mut pieces: Vec<String> = Vec::new();
        let mut last_span = (sl, sc, sl, sc);
        loop {
            let k = self.peek().clone();
            match &k {
                TokenKind::Eof => break,
                TokenKind::Comma if depth == 0 => break,
                TokenKind::RParen if depth <= 0 => break,
                TokenKind::RBracket if depth <= 0 => break,
                TokenKind::LParen | TokenKind::LBracket => {
                    depth += 1;
                    pieces.push(self.token_text(&k));
                }
                TokenKind::RParen | TokenKind::RBracket => {
                    depth -= 1;
                    pieces.push(self.token_text(&k));
                }
                _ => {
                    pieces.push(self.token_text(&k));
                }
            }
            last_span = self.span_here();
            self.advance();
        }
        if pieces.is_empty() {
            return Err(self.err_at(
                ErrorCode::E0010,
                "expected type expression after ':'",
                (sl, sc, sl, sc),
            ));
        }
        let text = pieces.join(" ");
        let expr = self.parse_type_expr_from_pieces(&pieces, sl, sc)?;
        let ann_span = Span {
            file: self.file.clone(),
            line_start: sl,
            col_start: sc,
            line_end: last_span.2,
            col_end: last_span.3,
        };
        Ok(Some(TypeAnnotation::new(expr, text, ann_span)))
    }

    /// Source-text representation of a token, used to build raw
    /// type-annotation strings. Returns "<tok>" for tokens we
    /// don't bother spelling out.
    fn token_text(&self, k: &TokenKind) -> String {
        match k {
            TokenKind::Ident(s) => s.clone(),
            TokenKind::Integer(n) => n.to_string(),
            TokenKind::Float(f) => f.to_string(),
            TokenKind::StringLit(s) => format!("\"{}\"", s),
            TokenKind::True => "TRUE".into(),
            TokenKind::False => "FALSE".into(),
            TokenKind::Null => "NULL".into(),
            TokenKind::Let => "LET".into(),
            TokenKind::Fun => "FUN".into(),
            TokenKind::Return => "RETURN".into(),
            TokenKind::If => "IF".into(),
            TokenKind::While => "WHILE".into(),
            TokenKind::For => "FOR".into(),
            TokenKind::Break => "BREAK".into(),
            TokenKind::Continue => "CONTINUE".into(),
            TokenKind::Class => "CLASS".into(),
            TokenKind::New => "NEW".into(),
            TokenKind::This => "THIS".into(),
            TokenKind::Ok => "OK".into(),
            TokenKind::Err => "ERR".into(),
            TokenKind::Panic => "PANIC".into(),
            TokenKind::Try => "TRY".into(),
            TokenKind::IsOk => "IS_OK".into(),
            TokenKind::IsErr => "IS_ERR".into(),
            TokenKind::OrDie => "OR_DIE".into(),
            TokenKind::Import => "IMPORT".into(),
            TokenKind::Export => "EXPORT".into(),
            TokenKind::LParen => "(".into(),
            TokenKind::RParen => ")".into(),
            TokenKind::LBracket => "[".into(),
            TokenKind::RBracket => "]".into(),
            TokenKind::Comma => ",".into(),
            TokenKind::Semicolon => ";".into(),
            TokenKind::Colon => ":".into(),
            TokenKind::Dot => ".".into(),
            TokenKind::Plus => "+".into(),
            TokenKind::Minus => "-".into(),
            TokenKind::Star => "*".into(),
            TokenKind::Slash => "/".into(),
            TokenKind::Percent => "%".into(),
            TokenKind::EqEq => "==".into(),
            TokenKind::BangEq => "!=".into(),
            TokenKind::Lt => "<".into(),
            TokenKind::Gt => ">".into(),
            TokenKind::LtEq => "<=".into(),
            TokenKind::GtEq => ">=".into(),
            TokenKind::AmpAmp => "&&".into(),
            TokenKind::PipePipe => "||".into(),
            TokenKind::Bang => "!".into(),
            TokenKind::Eof => "<eof>".into(),
        }
    }

    // ── §7 Control flow ──────────────────────────────────────────────

    fn parse_if(&mut self) -> WlwlResult<Expr> {
        let (line, col, _, _) = self.span_here();
        self.expect_specific(ErrorCode::E0010, "'IF'")?;
        self.expect_specific(ErrorCode::E0011, "'('")?;
        let cond = self.parse_expr()?;
                self.expect_specific(ErrorCode::E0012, "','")?;
        // The branches may be multi-statement blocks; parse them as
        // such (terminated by the closing `)` of the IF, or a `,` for
        // the else branch).
        let then_branch = self.parse_block(true)?;
        let else_branch = if matches!(self.peek(), TokenKind::Comma) {
            self.advance();
            Some(Box::new(self.parse_block(true)?))
        } else {
            None
        };
        self.expect_specific(ErrorCode::E0011, "')'")?;
        let (_, _, line_end, col_end) = self.span_here();
        Ok(Expr::If {
            cond: Box::new(cond),
            then_branch: Box::new(then_branch),
            else_branch,
            span: Span {
                file: self.file.clone(),
                line_start: line,
                col_start: col,
                line_end,
                col_end,
            },
        })
    }

    fn parse_while(&mut self) -> WlwlResult<Expr> {
        let (line, col, _, _) = self.span_here();
        self.expect_specific(ErrorCode::E0010, "'WHILE'")?;
        self.expect_specific(ErrorCode::E0011, "'('")?;
        let cond = self.parse_expr()?;
        self.expect_specific(ErrorCode::E0012, "','")?;
        // Body is a block (terminated by the closing `)` of WHILE).
        let body = self.parse_block(true)?;
        self.expect_specific(ErrorCode::E0011, "')'")?;
        let (_, _, line_end, col_end) = self.span_here();
        Ok(Expr::While {
            cond: Box::new(cond),
            body: Box::new(body),
            span: Span {
                file: self.file.clone(),
                line_start: line,
                col_start: col,
                line_end,
                col_end,
            },
        })
    }

    fn parse_for(&mut self) -> WlwlResult<Expr> {
        let (line, col, _, _) = self.span_here();
        self.expect_specific(ErrorCode::E0010, "'FOR'")?;
        self.expect_specific(ErrorCode::E0011, "'('")?;
        let var = match self.advance() {
            Token { kind: TokenKind::Ident(s), .. } => s,
            other => {
                return Err(self.err_at(
                    ErrorCode::E0010,
                    format!("expected identifier in FOR, got {:?}", other.kind),
                    other.span,
                ));
            }
        };
        self.expect_specific(ErrorCode::E0012, "','")?;
        let iter = self.parse_expr()?;
        self.expect_specific(ErrorCode::E0012, "','")?;
        // Body is a block (terminated by the closing `)` of FOR).
        let body = self.parse_block(true)?;
        self.expect_specific(ErrorCode::E0011, "')'")?;
        let (_, _, line_end, col_end) = self.span_here();
        Ok(Expr::For {
            var,
            iter: Box::new(iter),
            body: Box::new(body),
            span: Span {
                file: self.file.clone(),
                line_start: line,
                col_start: col,
                line_end,
                col_end,
            },
        })
    }

    fn parse_fun(&mut self) -> WlwlResult<Expr> {
        let (line, col, _, _) = self.span_here();
        self.expect_specific(ErrorCode::E0010, "'FUN'")?;
        self.expect_specific(ErrorCode::E0011, "'('")?;
        self.expect_specific(ErrorCode::E0011, "'('")?;
        // Parameter list: zero or more Ident separated by commas.
        let mut params = Vec::new();
        if !matches!(self.peek(), TokenKind::RParen) {
            loop {
                match self.advance() {
                    Token { kind: TokenKind::Ident(s), span } => {
                        let type_annotation = self.parse_type_annotation()?;
                        let param_span = Span {
                            file: self.file.clone(),
                            line_start: span.0,
                            col_start: span.1,
                            line_end: span.2,
                            col_end: span.3,
                        };
                        params.push(FunParam {
                            name: s,
                            type_annotation,
                            span: param_span,
                        });
                        }
                    other => {
                        return Err(self.err_at(
                            ErrorCode::E0010,
                            format!("expected parameter name, got {:?}", other.kind),
                            other.span,
                        ));
                    }
                }
                if matches!(self.peek(), TokenKind::Comma) {
                    self.advance();
                } else {
                    break;
                }
            }
        }
        self.expect_specific(ErrorCode::E0011, "')'")?;

        // v0.3 Sec. 2.4: optional return type annotation on FUN.
        let return_type = self.parse_type_annotation()?;
        self.expect_specific(ErrorCode::E0012, "','")?;

        // The body is a block — it may contain multiple statements
        // separated by `;`. We use `parse_block(true)` so that the
        // block terminates at the closing `)` of the FUN call.
        let body = self.parse_block(true)?;
        self.expect_specific(ErrorCode::E0011, "')'")?;
        let (_, _, line_end, col_end) = self.span_here();
        Ok(Expr::Fun {
            params,
            return_type,
            body: Box::new(body),
            span: Span {
                file: self.file.clone(),
                line_start: line,
                col_start: col,
                line_end,
                col_end,
            },
        })
    }

    fn parse_return(&mut self) -> WlwlResult<Expr> {
        let (line, col, _, _) = self.span_here();
        self.expect_specific(ErrorCode::E0010, "'RETURN'")?;
        self.expect_specific(ErrorCode::E0011, "'('")?;
        let value = if matches!(self.peek(), TokenKind::RParen) {
            None
        } else {
            Some(Box::new(self.parse_expr()?))
        };
        self.expect_specific(ErrorCode::E0011, "')'")?;
        let (_, _, line_end, col_end) = self.span_here();
        Ok(Expr::Return {
            value,
            span: Span {
                file: self.file.clone(),
                line_start: line,
                col_start: col,
                line_end,
                col_end,
            },
        })
    }

    fn parse_break(&mut self) -> WlwlResult<Expr> {
        let (line, col, _, _) = self.span_here();
        self.expect_specific(ErrorCode::E0010, "'BREAK'")?;
        self.expect_specific(ErrorCode::E0011, "'('")?;
        self.expect_specific(ErrorCode::E0011, "')'")?;
        let (_, _, line_end, col_end) = self.span_here();
        Ok(Expr::Break {
            span: Span {
                file: self.file.clone(),
                line_start: line,
                col_start: col,
                line_end,
                col_end,
            },
        })
    }

    fn parse_continue(&mut self) -> WlwlResult<Expr> {
        let (line, col, _, _) = self.span_here();
        self.expect_specific(ErrorCode::E0010, "'CONTINUE'")?;
        self.expect_specific(ErrorCode::E0011, "'('")?;
        self.expect_specific(ErrorCode::E0011, "')'")?;
        let (_, _, line_end, col_end) = self.span_here();
        Ok(Expr::Continue {
            span: Span {
                file: self.file.clone(),
                line_start: line,
                col_start: col,
                line_end,
                col_end,
            },
        })
    }

    // ── §12 Error handling constructors ──────────────────────────────
    //
    // All §12 single-arg constructors share the same shape:
    //   `KW '(' expr ')'`
    // so we factor it out. (`OR_DIE` is two-arg and handled inline in
    // the keyword dispatch.)

    fn parse_err_ctor<F>(&mut self, kw: &str, ctor: F) -> WlwlResult<Expr>
    where
        F: FnOnce(Box<Expr>, Span) -> Expr,
    {
        let (line, col, _, _) = self.span_here();
        self.expect_specific(ErrorCode::E0010, kw)?;
        self.expect_specific(ErrorCode::E0011, "'('")?;
        let value = self.parse_expr()?;
        self.expect_specific(ErrorCode::E0011, "')'")?;
        let (_, _, line_end, col_end) = self.span_here();
        let span = Span {
            file: self.file.clone(),
            line_start: line,
            col_start: col,
            line_end,
            col_end,
        };
        Ok(ctor(Box::new(value), span))
    }

    /// `OR_DIE(value, default)` — the only two-arg §12 constructor.
    fn parse_or_die(&mut self) -> WlwlResult<Expr> {
        let (line, col, _, _) = self.span_here();
        self.expect_specific(ErrorCode::E0010, "'OR_DIE'")?;
        self.expect_specific(ErrorCode::E0011, "'('")?;
        let value = self.parse_expr()?;
        self.expect_specific(ErrorCode::E0012, "','")?;
        let default = self.parse_expr()?;
        self.expect_specific(ErrorCode::E0011, "')'")?;
        let (_, _, line_end, col_end) = self.span_here();
        Ok(Expr::OrDie {
            value: Box::new(value),
            default: Box::new(default),
            span: Span {
                file: self.file.clone(),
                line_start: line,
                col_start: col,
                line_end,
                col_end,
            },
        })
    }

    // ── §13 Modules (subset) ─────────────────────────────────────────

    fn parse_import(&mut self) -> WlwlResult<Expr> {
        let (line, col, _, _) = self.span_here();
        self.expect_specific(ErrorCode::E0010, "'IMPORT'")?;
        self.expect_specific(ErrorCode::E0011, "'('")?;
        // path: must be a string literal
        let path = match self.advance() {
            Token { kind: TokenKind::StringLit(s), .. } => s,
            other => {
                return Err(self.err_at(
                    ErrorCode::E0043,
                    format!(
                        "IMPORT path must be a string literal, got {:?}",
                        other.kind
                    ),
                    other.span,
                ));
            }
        };
        // Phase 4 batch 2: the parser now accepts any non-empty
        // string literal as an IMPORT path. Resolution is entirely
        // the `ModuleLoader::load` job: it dispatches `wlwl:std.X`
        // to the std registry, `<ns>:<name>` to the project
        // manifest, `./` / `../` to the file system relative to
        // the importing module, and bare names to the same
        // directory / project root. Surface-level path errors
        // (empty / whitespace-only) still fail here with E0043.
        if path.is_empty() {
            return Err(self.err_at(
                ErrorCode::E0043,
                "IMPORT path is empty".to_string(),
                (line, col, line, col),
            ));
        }
        self.expect_specific(ErrorCode::E0012, "','")?;
        let names = self.parse_import_name_list("'IMPORT'")?;
        self.expect_specific(ErrorCode::E0011, "')'")?;
        let (_, _, line_end, col_end) = self.span_here();
        Ok(Expr::Import {
            path,
            names,
            span: Span {
                file: self.file.clone(),
                line_start: line,
                col_start: col,
                line_end,
                col_end,
            },
        })
    }

    fn parse_export(&mut self) -> WlwlResult<Expr> {
        let (line, col, _, _) = self.span_here();
        self.expect_specific(ErrorCode::E0010, "'EXPORT'")?;
        self.expect_specific(ErrorCode::E0011, "'('")?;
        let names = self.parse_import_name_list("'EXPORT'")?;
        self.expect_specific(ErrorCode::E0011, "')'")?;
        let (_, _, line_end, col_end) = self.span_here();
        Ok(Expr::Export {
            names,
            span: Span {
                file: self.file.clone(),
                line_start: line,
                col_start: col,
                line_end,
                col_end,
            },
        })
    }

    /// Parse a list of `name` or `"name": "alias"` entries inside `[...]`.
    /// Used by both `IMPORT` and `EXPORT`.
    fn parse_import_name_list(&mut self, _ctx: &str) -> WlwlResult<Vec<ImportName>> {
        self.expect_specific(ErrorCode::E0011, "'['")?;
        let mut names = Vec::new();
        if !matches!(self.peek(), TokenKind::RBracket) {
            loop {
                let (nline, ncol, _, _) = self.span_here();
                let name_tok = self.advance();
                // v0.3 §13.3: name-list entries are STRING LITERALS in
                // both the simple form `["add", "PI"]` and the rename
                // form `["add": "math_add"]`. We also accept a bare
                // identifier as a convenience (it has the same effect
                // as the equivalent string).
                let name = match name_tok.kind {
                    TokenKind::StringLit(s) => s,
                    TokenKind::Ident(s) => s,
                    other => {
                        return Err(self.err_at(
                            ErrorCode::E0010,
                            format!(
                                "expected identifier or string in name list, got {:?}",
                                other
                            ),
                            name_tok.span,
                        ));
                    }
                };
                let alias = if matches!(self.peek(), TokenKind::Colon) {
                    self.advance();
                    match self.advance() {
                        Token { kind: TokenKind::StringLit(s), .. } => Some(s),
                        Token { kind: TokenKind::Ident(s), .. } => Some(s),
                        other => {
                            return Err(self.err_at(
                                ErrorCode::E0010,
                                format!("expected string alias, got {:?}", other.kind),
                                other.span,
                            ));
                        }
                    }
                } else {
                    None
                };
                let (_, _, nle, nce) = self.span_here();
                names.push(ImportName {
                    name,
                    alias,
                    span: Span {
                        file: self.file.clone(),
                        line_start: nline,
                        col_start: ncol,
                        line_end: nle,
                        col_end: nce,
                    },
                });
                if matches!(self.peek(), TokenKind::Comma) {
                    self.advance();
                } else {
                    break;
                }
            }
        }
        self.expect_specific(ErrorCode::E0011, "']'")?;
        Ok(names)
    }

    // ── §5 / §8.3 Call / operator-named call ────────────────────────

    fn parse_call_or_ident(&mut self) -> WlwlResult<Expr> {
        let (line, col, _, _) = self.span_here();
        let t = self.advance();
        // Either an identifier or an operator token (treated as a function name).
        let name = match t.kind {
            TokenKind::Ident(s) => s,
            ref k => match k.as_op_name() {
                Some(op) => op.to_string(),
                None => {
                    return Err(self.err_at(
                        ErrorCode::E0010,
                        format!("expected identifier or operator, got {:?}", t.kind),
                        t.span,
                    ));
                }
            },
        };

        if matches!(self.peek(), TokenKind::LParen) {
            // Function call: name(args)
            self.advance(); // '('
            let mut args = Vec::new();
            if !matches!(self.peek(), TokenKind::RParen) {
                loop {
                    args.push(self.parse_expr()?);
                    if matches!(self.peek(), TokenKind::Comma) {
                        self.advance();
                    } else {
                        break;
                    }
                }
            }
            self.expect_specific(ErrorCode::E0011, "')'")?;
            let (_, _, line_end, col_end) = self.span_here();
            Ok(Expr::Call {
                name,
                args,
                span: Span {
                    file: self.file.clone(),
                    line_start: line,
                    col_start: col,
                    line_end,
                    col_end,
                },
            })
        } else {
            // Variable reference
            Ok(Expr::Var(
                name,
                Span {
                    file: self.file.clone(),
                    line_start: line,
                    col_start: col,
                    line_end: t.span.2,
                    col_end: t.span.3,
                },
            ))
        }
    }

    // ── §4 / §10 Literals ──────────────────────────────────────────

    fn parse_literal(&mut self) -> WlwlResult<Expr> {
        let (line, col, _, _) = self.span_here();
        let t = self.advance();
        let lit = match t.kind {
            TokenKind::Integer(v) => Literal::Integer(v),
            TokenKind::Float(v) => Literal::Float(v),
            TokenKind::StringLit(s) => Literal::String(s),
            TokenKind::True => Literal::Boolean(true),
            TokenKind::False => Literal::Boolean(false),
            TokenKind::Null => Literal::Null,
            TokenKind::LBracket => {
                return self.parse_array_or_dict(line, col);
            }
            other => {
                return Err(self.err_at(
                    ErrorCode::E0010,
                    format!("expected literal, got {:?}", other),
                    t.span,
                ));
            }
        };
        Ok(Expr::Literal(lit, Span {
            file: self.file.clone(),
            line_start: line,
            col_start: col,
            line_end: t.span.2,
            col_end: t.span.3,
        }))
    }

    fn parse_array_or_dict(&mut self, line: u32, col: u32) -> WlwlResult<Expr> {
        if matches!(self.peek(), TokenKind::RBracket) {
            self.advance(); // ']'
            return Ok(Expr::Array {
                items: vec![],
                span: Span {
                    file: self.file.clone(),
                    line_start: line,
                    col_start: col,
                    line_end: line,
                    col_end: col,
                },
            });
        }
        let first = self.parse_expr()?;
        if matches!(self.peek(), TokenKind::Colon) {
            self.advance();
            let first_value = self.parse_expr()?;
            let mut entries = vec![(first, first_value)];
            while matches!(self.peek(), TokenKind::Comma) {
                self.advance();
                if matches!(self.peek(), TokenKind::RBracket) {
                    break;
                }
                let k = self.parse_expr()?;
                self.expect_specific(ErrorCode::E0010, "':'")?;
                let v = self.parse_expr()?;
                entries.push((k, v));
            }
            self.expect_specific(ErrorCode::E0011, "']'")?;
            let (_, _, line_end, col_end) = self.span_here();
            Ok(Expr::Dict {
                entries,
                span: Span {
                    file: self.file.clone(),
                    line_start: line,
                    col_start: col,
                    line_end,
                    col_end,
                },
            })
        } else {
            let mut items = vec![first];
            while matches!(self.peek(), TokenKind::Comma) {
                self.advance();
                if matches!(self.peek(), TokenKind::RBracket) {
                    break;
                }
                items.push(self.parse_expr()?);
            }
            self.expect_specific(ErrorCode::E0011, "']'")?;
            let (_, _, line_end, col_end) = self.span_here();
            Ok(Expr::Array {
                items,
                span: Span {
                    file: self.file.clone(),
                    line_start: line,
                    col_start: col,
                    line_end,
                    col_end,
                },
            })
        }
    }

    fn parse_paren_block(&mut self) -> WlwlResult<Expr> {
        let (line, col, _, _) = self.span_here();
        self.advance(); // '('
        let block = self.parse_block(true)?;
        self.expect_specific(ErrorCode::E0011, "')'")?;
        let (_, _, line_end, col_end) = self.span_here();
        let new_span = Span {
            file: self.file.clone(),
            line_start: line,
            col_start: col,
            line_end,
            col_end,
        };
        Ok(match block {
            Expr::Block { exprs, .. } => Expr::Block { exprs, span: new_span },
            other => other,
        })
    }
    // ── Type expression parser (P3-010) ─────────────────────────────
    //
    // parse_type_annotation collects the type annotation's tokens
    // as a flat Vec<String>, then hands it to this entry point
    // which builds a structured TypeExpr (Ident / Array / Generic).
    fn parse_type_expr_from_pieces(
        &self,
        pieces: &[String],
        sl: u32,
        sc: u32,
    ) -> WlwlResult<TypeExpr> {
        let mut p = TypeExprParser {
            pieces,
            pos: 0,
            file: self.file.clone(),
        };
        let expr = p.parse_expr(sl, sc);
        if p.pos < p.pieces.len() {
            let rest: Vec<String> = p.pieces[p.pos..].to_vec();
            return Ok(TypeExpr::Generic {
                name: rest.join(" "),
                args: vec![],
                span: Span {
                    file: self.file.clone(),
                    line_start: sl,
                    col_start: sc,
                    line_end: sl,
                    col_end: sc,
                },
            });
        }
        expr
    }
}

// ── TypeExprParser (free helper) ────────────────────────────

struct TypeExprParser<'a> {
    pieces: &'a [String],
    pos: usize,
    file: String,
}

impl<'a> TypeExprParser<'a> {
    fn parse_expr(&mut self, sl: u32, sc: u32) -> WlwlResult<TypeExpr> {
        let head = self.peek().to_string();
        if head == "ARRAY" {
            self.pos += 1;
            return self.parse_braced("ARRAY", sl, sc);
        }
        if !is_ident(&head) {
            return Err(WlwlDiagnostic::new(
                ErrorCode::E0010,
                format!("expected type expression, got `{}`", head),
                Location::point(&self.file, sl, sc),
            )
            .into());
        }
        self.pos += 1;
        if self.peek() == "[" {
            return self.parse_braced(&head, sl, sc);
        }
        Ok(TypeExpr::Ident {
            name: head,
            span: self.span_here(sl, sc),
        })
    }

    fn parse_braced(&mut self, head: &str, sl: u32, sc: u32) -> WlwlResult<TypeExpr> {
        if self.peek() != "[" {
            return Err(WlwlDiagnostic::new(
                ErrorCode::E0010,
                format!("expected `[` after `{}` in type expression", head),
                Location::point(&self.file, sl, sc),
            )
            .into());
        }
        self.pos += 1;
        let mut args = Vec::new();
        loop {
            args.push(self.parse_expr(sl, sc)?);
            match self.peek() {
                "," => {
                    self.pos += 1;
                }
                "]" => {
                    self.pos += 1;
                    break;
                }
                other => {
                    return Err(WlwlDiagnostic::new(
                        ErrorCode::E0012,
                        format!(
                            "expected `,` or `]` in type expression, got `{}`",
                            other
                        ),
                        Location::point(&self.file, sl, sc),
                    )
                    .into());
                }
            }
        }
        let span = self.span_here(sl, sc);
        if head == "ARRAY" && args.len() == 1 {
            Ok(TypeExpr::Array {
                element: Box::new(args.into_iter().next().unwrap()),
                span,
            })
        } else {
            Ok(TypeExpr::Generic {
                name: head.to_string(),
                args,
                span,
            })
        }
    }

    fn peek(&self) -> &str {
        if self.pos < self.pieces.len() {
            self.pieces[self.pos].as_str()
        } else {
            ""
        }
    }

    fn span_here(&self, sl: u32, sc: u32) -> Span {
        Span {
            file: self.file.clone(),
            line_start: sl,
            col_start: sc,
            line_end: sl,
            col_end: sc,
        }
    }
}

fn is_ident(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_integer_literal() {
        let e = parse("42", "t.wl").unwrap();
        assert!(matches!(e, Expr::Literal(Literal::Integer(42), _)));
    }

    #[test]
    fn parse_let() {
        let e = parse("LET(x, 1);", "t.wl").unwrap();
        match e {
            Expr::Let { name, value, .. } => {
                assert_eq!(name, "x");
                assert!(matches!(*value, Expr::Literal(Literal::Integer(1), _)));
            }
            _ => panic!("expected LET"),
        }
    }

    #[test]
    fn parse_if_then_else() {
        let e = parse(r#"IF(==(x, 0), "zero", "non-zero");"#, "t.wl").unwrap();
        match e {
            Expr::If { else_branch, .. } => assert!(else_branch.is_some()),
            _ => panic!("expected IF"),
        }
    }

    #[test]
    fn parse_if_no_else() {
        let e = parse("IF(==(x, 0), PRINT(0));", "t.wl").unwrap();
        match e {
            Expr::If { else_branch, .. } => assert!(else_branch.is_none()),
            _ => panic!("expected IF"),
        }
    }

    #[test]
    fn parse_while() {
        let e = parse("WHILE(<(x, 10), LET(x, +(x, 1)));", "t.wl").unwrap();
        assert!(matches!(e, Expr::While { .. }));
    }

    #[test]
    fn parse_for() {
        let e = parse("FOR(i, [1, 2, 3], PRINT(i));", "t.wl").unwrap();
        match e {
            Expr::For { var, .. } => assert_eq!(var, "i"),
            _ => panic!("expected FOR"),
        }
    }

    #[test]
    fn parse_fun_zero_params() {
        let e = parse("FUN((), 42);", "t.wl").unwrap();
        match e {
            Expr::Fun { params, .. } => assert_eq!(params.len(), 0),
            _ => panic!("expected FUN"),
        }
    }

    #[test]
    fn parse_fun_two_params() {
        let e = parse("FUN((a, b), +(a, b));", "t.wl").unwrap();
        match e {
            Expr::Fun { params, .. } => assert_eq!(params.iter().map(|p| p.name.clone()).collect::<Vec<_>>(), vec!["a".to_string(), "b".to_string()]),
            _ => panic!("expected FUN"),
        }
    }

    #[test]
    fn parse_return_with_value() {
        let e = parse("RETURN(42);", "t.wl").unwrap();
        match e {
            Expr::Return { value, .. } => assert!(value.is_some()),
            _ => panic!("expected RETURN"),
        }
    }

    #[test]
    fn parse_return_void() {
        let e = parse("RETURN();", "t.wl").unwrap();
        match e {
            Expr::Return { value, .. } => assert!(value.is_none()),
            _ => panic!("expected RETURN"),
        }
    }

    #[test]
    fn parse_break_continue() {
        assert!(matches!(parse("BREAK();", "t.wl").unwrap(), Expr::Break { .. }));
        assert!(matches!(parse("CONTINUE();", "t.wl").unwrap(), Expr::Continue { .. }));
    }

    #[test]
    fn parse_ok_err_panic() {
        assert!(matches!(parse("OK(1);", "t.wl").unwrap(), Expr::Ok { .. }));
        assert!(matches!(parse("ERR(\"bad\");", "t.wl").unwrap(), Expr::Err { .. }));
        assert!(matches!(parse("PANIC(\"oops\");", "t.wl").unwrap(), Expr::Panic { .. }));
    }

    #[test]
    fn parse_try_isok_iserr_ordie() {
        assert!(matches!(parse("TRY(OK(1));", "t.wl").unwrap(), Expr::Try { .. }));
        assert!(matches!(parse("IS_OK(OK(1));", "t.wl").unwrap(), Expr::IsOk { .. }));
        assert!(matches!(parse("IS_ERR(ERR(1));", "t.wl").unwrap(), Expr::IsErr { .. }));
        let e = parse("OR_DIE(ERR(1), 0);", "t.wl").unwrap();
        match e {
            Expr::OrDie { value, default, .. } => {
                assert!(matches!(*value, Expr::Err { .. }));
                assert!(matches!(*default, Expr::Literal(Literal::Integer(0), _)));
            }
            _ => panic!("expected OR_DIE"),
        }
    }

    #[test]
    fn parse_or_die_via_call() {
        // OR_DIE is not a lexer keyword in Phase 1, so this will parse as a Call.
        // (We only added the keywords IF/WHILE/... in the lexer, OR_DIE was
        // never a keyword.) Verify the keyword IS present in the lexer.
        // (Skip the actual call parse — the Phase 1 lexer doesn't have it.)
    }

    #[test]
    fn parse_import_simple() {
        let e = parse(r#"IMPORT("math", ["add", "PI"]);"#, "t.wl").unwrap();
        match e {
            Expr::Import { path, names, .. } => {
                assert_eq!(path, "math");
                assert_eq!(names.len(), 2);
                assert_eq!(names[0].name, "add");
                assert_eq!(names[0].alias, None);
            }
            _ => panic!("expected IMPORT"),
        }
    }

    #[test]
    fn parse_import_with_rename() {
        let e = parse(
            r#"IMPORT("math", ["add": "math_add", "PI": "MATH_PI"]);"#,
            "t.wl",
        )
        .unwrap();
        match e {
            Expr::Import { names, .. } => {
                assert_eq!(names[0].name, "add");
                assert_eq!(names[0].alias.as_deref(), Some("math_add"));
                assert_eq!(names[1].name, "PI");
                assert_eq!(names[1].alias.as_deref(), Some("MATH_PI"));
            }
            _ => panic!("expected IMPORT"),
        }
    }

    #[test]
    fn parse_export() {
        let e = parse(r#"EXPORT(["add", "PI"]);"#, "t.wl").unwrap();
        match e {
            Expr::Export { names, .. } => {
                assert_eq!(names.len(), 2);
                assert_eq!(names[1].name, "PI");
            }
            _ => panic!("expected EXPORT"),
        }
    }

    #[test]
    #[test]
    fn parse_import_accepts_wlwl_namespace_path() {
        // Phase 4 batch 1: `wlwl:std.X` namespace prefix is accepted
        // at parse time. The module loader resolves it to a std
        // module; if the name is unknown the loader surfaces E0040.
        let e = parse(r#"IMPORT("wlwl:std.io", ["PRINT"]);"#, "t.wl").unwrap();
        match e {
            Expr::Import { path, names, .. } => {
                assert_eq!(path, "wlwl:std.io");
                assert_eq!(names.len(), 1);
                assert_eq!(names[0].local_name(), "PRINT");
            }
            _ => panic!("expected IMPORT"),
        }
    }

    #[test]
    #[test]
    fn parse_import_accepts_third_party_namespace() {
        // Phase 4 batch 2: the parser accepts any non-empty path,
        // including `<ns>:<name>` third-party references. The
        // ModuleLoader resolves them against the project manifest;
        // an unregistered namespace is E0043 at eval time, not
        // parse time.
        let e = parse(r#"IMPORT("myteam:utils", ["x"]);"#, "t.wl").unwrap();
        match e {
            Expr::Import { path, names, .. } => {
                assert_eq!(path, "myteam:utils");
                assert_eq!(names[0].local_name(), "x");
            }
            _ => panic!("expected IMPORT"),
        }

        let e = parse(r#"IMPORT("./other", ["x"]);"#, "t.wl").unwrap();
        match e {
            Expr::Import { path, .. } => assert_eq!(path, "./other"),
            _ => panic!("expected IMPORT"),
        }
    }

    #[test]
    fn parse_import_rejects_empty_path() {
        // Only surface-level error left: an empty IMPORT path.
        let err = parse(r#"IMPORT("", ["x"]);"#, "t.wl").unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0043);
    }

    #[test]
    fn parse_call_no_args() {
        let e = parse("PRINT();", "t.wl").unwrap();
        match e {
            Expr::Call { name, args, .. } => {
                assert_eq!(name, "PRINT");
                assert_eq!(args.len(), 0);
            }
            _ => panic!("expected call"),
        }
    }

    #[test]
    fn parse_call_with_args() {
        let e = parse("PRINT(\"hi\");", "t.wl").unwrap();
        match e {
            Expr::Call { name, args, .. } => {
                assert_eq!(name, "PRINT");
                assert_eq!(args.len(), 1);
            }
            _ => panic!("expected call"),
        }
    }

    #[test]
    fn parse_call_with_operator_name() {
        // + / == etc. become Call nodes with the operator spelling as the name.
        let e = parse("+(1, 2);", "t.wl").unwrap();
        match e {
            Expr::Call { name, args, .. } => {
                assert_eq!(name, "+");
                assert_eq!(args.len(), 2);
            }
            _ => panic!("expected call with op name"),
        }
        let e = parse("==(x, 0);", "t.wl").unwrap();
        match e {
            Expr::Call { name, .. } => assert_eq!(name, "=="),
            _ => panic!("expected == call"),
        }
    }

    #[test]
    fn parse_array() {
        let e = parse("[1, 2, 3];", "t.wl").unwrap();
        match e {
            Expr::Array { items, .. } => assert_eq!(items.len(), 3),
            _ => panic!("expected array"),
        }
    }

    #[test]
    fn parse_dict() {
        let e = parse("[\"a\": 1, \"b\": 2];", "t.wl").unwrap();
        match e {
            Expr::Dict { entries, .. } => assert_eq!(entries.len(), 2),
            _ => panic!("expected dict"),
        }
    }

    #[test]
    fn parse_missing_semicolon() {
        let err = parse("LET(x, 1) LET(y, 2);", "t.wl").unwrap_err();
        let d = err.diagnostic();
        assert_eq!(d.code, ErrorCode::E0013);
    }

    // -- Phase 3: v0.3 Sec. 2.4 type annotations (parsed, not checked) --

    #[test]
    fn parse_let_with_type_annotation() {
        let e = parse("LET(x: INTEGER, 1);", "t.wl").unwrap();
        match e {
            Expr::Let { name, type_annotation, .. } => {
                assert_eq!(name, "x");
                let ann = type_annotation.expect("expected annotation");
                assert_eq!(ann.text, "INTEGER");
            }
            _ => panic!("expected LET"),
        }
    }

    #[test]
    fn parse_let_without_type_annotation() {
        let e = parse("LET(x, 1);", "t.wl").unwrap();
        match e {
            Expr::Let { name, type_annotation, .. } => {
                assert_eq!(name, "x");
                assert!(type_annotation.is_none());
            }
            _ => panic!("expected LET"),
        }
    }

    #[test]
    fn parse_let_with_complex_type_annotation() {
        let e = parse("LET(xs: ARRAY[INTEGER], [1, 2, 3]);", "t.wl").unwrap();
        match e {
            Expr::Let { type_annotation, .. } => {
                let ann = type_annotation.expect("expected annotation");
                assert_eq!(ann.text, "ARRAY [ INTEGER ]");
            }
            _ => panic!("expected LET"),
        }
    }

    #[test]
    fn parse_fun_with_return_type_annotation() {
        let e = parse("FUN((a, b): INTEGER, +(a, b));", "t.wl").unwrap();
        match e {
            Expr::Fun { params, return_type, .. } => {
                assert_eq!(params.iter().map(|p| p.name.clone()).collect::<Vec<_>>(), vec!["a".to_string(), "b".to_string()]);
                let ann = return_type.expect("expected return annotation");
                assert_eq!(ann.text, "INTEGER");
            }
            _ => panic!("expected FUN"),
        }
    }

    #[test]
    fn parse_fun_without_return_type_annotation() {
        let e = parse("FUN((a, b), +(a, b));", "t.wl").unwrap();
        match e {
            Expr::Fun { params, return_type, .. } => {
                assert_eq!(params.iter().map(|p| p.name.clone()).collect::<Vec<_>>(), vec!["a".to_string(), "b".to_string()]);
                assert!(return_type.is_none());
            }
            _ => panic!("expected FUN"),
        }
    }

    #[test]
    fn parse_let_missing_value_after_type() {
        // Type annotation without trailing comma -> E0012 (expected ",")
        let err = parse("LET(x: INTEGER,);", "t.wl").unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0010); // value missing
    }

    #[test]
    fn parse_missing_rparen() {
        let err = parse("LET(x, 1;", "t.wl").unwrap_err();
        let d = err.diagnostic();
        assert_eq!(d.code, ErrorCode::E0011);
    }

    #[test]
    fn parse_fun_with_per_param_type_annotation() {
        // P3-007: per-parameter `name: Type` annotations on FUN.
        // The annotation is parsed not checked; the AST stores
        // `params: Vec<FunParam>` with `type_annotation: Some(...)`.
        let e = parse(
            "FUN((x: INTEGER, y: STRING), PRINT(x, y));",
            "t.wl",
        )
        .unwrap();
        match e {
            Expr::Fun { params, .. } => {
                assert_eq!(params.len(), 2);
                assert_eq!(params[0].name, "x");
                let ann0 = params[0]
                    .type_annotation
                    .as_ref()
                    .expect("x has annotation");
                // Structured: TypeExpr::Ident { name: "INTEGER", ... }
                match &ann0.expr {
                    wlwl_ast::TypeExpr::Ident { name, .. } => {
                        assert_eq!(name, "INTEGER");
                    }
                    _ => panic!("expected Ident, got {:?}", ann0.expr),
                }
                assert_eq!(params[1].name, "y");
                let ann1 = params[1]
                    .type_annotation
                    .as_ref()
                    .expect("y has annotation");
                match &ann1.expr {
                    wlwl_ast::TypeExpr::Ident { name, .. } => {
                        assert_eq!(name, "STRING");
                    }
                    _ => panic!("expected Ident, got {:?}", ann1.expr),
                }
            }
            _ => panic!("expected FUN"),
        }
    }

    #[test]
    fn parse_fun_per_param_array_type() {
        // P3-010: structured `ARRAY<T>` type expression. Parses to
        // `TypeExpr::Array { element: Box<TypeExpr> }` (not Generic).
        let e = parse(
            "FUN((xs: ARRAY[INTEGER]), PRINT(xs));",
            "t.wl",
        )
        .unwrap();
        match e {
            Expr::Fun { params, .. } => {
                let ann = params[0]
                    .type_annotation
                    .as_ref()
                    .expect("xs has annotation");
                match &ann.expr {
                    wlwl_ast::TypeExpr::Array { element, .. } => {
                        match &**element {
                            wlwl_ast::TypeExpr::Ident { name, .. } => {
                                assert_eq!(name, "INTEGER");
                            }
                            _ => panic!("expected inner Ident"),
                        }
                    }
                    _ => panic!("expected Array, got {:?}", ann.expr),
                }
            }
            _ => panic!("expected FUN"),
        }
    }

    #[test]
    fn parse_fun_per_param_generic_dict_type() {
        // P3-010: `DICT<K, V>` is `TypeExpr::Generic { name: "DICT", args }`.
        let e = parse(
            "FUN((m: DICT[STRING, INTEGER]), PRINT(m));",
            "t.wl",
        )
        .unwrap();
        match e {
            Expr::Fun { params, .. } => {
                let ann = params[0]
                    .type_annotation
                    .as_ref()
                    .expect("m has annotation");
                match &ann.expr {
                    wlwl_ast::TypeExpr::Generic { name, args, .. } => {
                        assert_eq!(name, "DICT");
                        assert_eq!(args.len(), 2);
                    }
                    _ => panic!("expected Generic, got {:?}", ann.expr),
                }
            }
            _ => panic!("expected FUN"),
        }
    }

    #[test]
    fn parse_fun_mixed_annotated_and_bare_params() {
        // P3-007: only some parameters carry annotations; the
        // others must be `FunParam { name, type_annotation: None, .. }`.
        let e = parse("FUN((a, b: INTEGER, c), PRINT(a, b, c));", "t.wl").unwrap();
        match e {
            Expr::Fun { params, .. } => {
                assert_eq!(params.len(), 3);
                assert_eq!(params[0].name, "a");
                assert!(params[0].type_annotation.is_none());
                assert_eq!(params[1].name, "b");
                assert!(params[1].type_annotation.is_some());
                assert_eq!(params[2].name, "c");
                assert!(params[2].type_annotation.is_none());
            }
            _ => panic!("expected FUN"),
        }
    }
}
