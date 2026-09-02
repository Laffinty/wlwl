//! WLWL error types and reporting.
//!
//! Phase 2 implements the extended error schema from v0.3 §14:
//! - `code` (E0001-E0014 lex/syn, E0020-E0023 name/module, E0030-E0032 type,
//!   E0040-E0043 module resolution, E0100 internal, E0102 unhandled ERR escape)
//! - `severity` ("error" | "warning" | "note")
//! - `message` (human-readable)
//! - `location` (file/line/col)
//! - `source_line` (the source line of the error)
//! - `error_schema_version` (SemVer string)
//!
//! Phase 3 will add: `errorCategory`, `retryable`, `suggestion_code`.
//!
//! Output formats: human-readable (default) and JSON (`--format=json`).
//! JSONL streaming is Phase 3 (with the `retryable` field).

use serde::{Deserialize, Serialize};
use std::fmt;

/// Stable error codes (Phase 2 subset of v0.3 §14.4; E0044+ are reserved).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorCode {
    // ── Lexical (E0001-E0009) ────────────────────────────────────────
    E0001, // illegal character
    E0002, // unterminated string
    E0003, // unterminated block comment
    // ── Syntax (E0010-E0019) ─────────────────────────────────────────
    E0010, // expected expression
    E0011, // expected ')'
    E0012, // expected ','
    E0013, // expected ';'
    E0014, // RETURN / BREAK / CONTINUE in illegal position
    // ── Name (E0020-E0029) ───────────────────────────────────────────
    E0020, // undefined name
    E0021, // duplicate definition / duplicate IMPORT
    E0022, // function arity mismatch
    E0023, // name not exported by module
    // ── Type (E0030-E0039) ───────────────────────────────────────────
    E0030, // type error (e.g. arithmetic on non-numeric)
    E0031, // subscript/key type error
    E0032, // property/method not found
    // ── Module (E0040-E0049) ─────────────────────────────────────────
    E0040, // module not found (path / project-root boundary)
    E0041, // circular IMPORT
    E0042, // module file IO error
    E0043, // namespace path syntax error
    // ── Internal (E0100-E0102) ───────────────────────────────────────
    E0100, // internal error
    E0102, // unhandled ERR escaped to top level (§12 / §19.6 Corollary 19.1)
}

impl ErrorCode {
    pub fn as_str(&self) -> &'static str {
        match self {
            ErrorCode::E0001 => "E0001",
            ErrorCode::E0002 => "E0002",
            ErrorCode::E0003 => "E0003",
            ErrorCode::E0010 => "E0010",
            ErrorCode::E0011 => "E0011",
            ErrorCode::E0012 => "E0012",
            ErrorCode::E0013 => "E0013",
            ErrorCode::E0014 => "E0014",
            ErrorCode::E0020 => "E0020",
            ErrorCode::E0021 => "E0021",
            ErrorCode::E0022 => "E0022",
            ErrorCode::E0023 => "E0023",
            ErrorCode::E0030 => "E0030",
            ErrorCode::E0031 => "E0031",
            ErrorCode::E0032 => "E0032",
            ErrorCode::E0040 => "E0040",
            ErrorCode::E0041 => "E0041",
            ErrorCode::E0042 => "E0042",
            ErrorCode::E0043 => "E0043",
            ErrorCode::E0100 => "E0100",
            ErrorCode::E0102 => "E0102",
        }
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Source location (subset of `wlwl_ast::Span` so the error crate has no ast dep).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Location {
    pub file: String,
    pub line: u32,
    pub col: u32,
    pub line_end: u32,
    pub col_end: u32,
}

impl Location {
    pub fn point(file: impl Into<String>, line: u32, col: u32) -> Self {
        Self {
            file: file.into(),
            line,
            col,
            line_end: line,
            col_end: col,
        }
    }
}

/// Extract the text of a given 1-based line from source.
///
/// Returns `None` if `line` is out of range. Used by the lexer/parser to
/// populate `source_line` on diagnostics (v0.3 §14.2).
pub fn extract_line(source: &str, line: u32) -> Option<String> {
    if line == 0 {
        return None;
    }
    let bytes = source.as_bytes();
    let mut cur = 1u32;
    let mut start: Option<usize> = None;
    for (i, &b) in bytes.iter().enumerate() {
        if cur == line && start.is_none() && b != b'\n' {
            start = Some(i);
        }
        if b == b'\n' {
            if cur == line {
                return Some(String::from_utf8_lossy(&bytes[start.unwrap_or(i)..i]).to_string());
            }
            cur += 1;
        }
    }
    // Last line (no trailing newline)
    if cur == line {
        let s = start.unwrap_or(bytes.len());
        return Some(String::from_utf8_lossy(&bytes[s..]).to_string());
    }
    None
}

/// The structured error object (v0.3 §14.2 — Phase 1 subset).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WlwlDiagnostic {
    /// Schema version (SemVer). v0.3 Phase 1 → "0.3.0".
    pub error_schema_version: String,
    /// Stable error code (see v0.3 §14.4).
    pub code: ErrorCode,
    /// "error" | "warning" | "note"
    pub severity: Severity,
    /// Human-readable one-line description.
    pub message: String,
    /// Source location.
    pub location: Location,
    /// The source line where the error occurred (for AI context).
    pub source_line: Option<String>,
    /// Optional fix hint (natural language; `suggestion_code` is Phase 3).
    pub hint: Option<String>,
}

/// Severity levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
    Note,
}

impl Severity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Note => "note",
        }
    }
}

impl WlwlDiagnostic {
    pub fn new(code: ErrorCode, message: impl Into<String>, location: Location) -> Self {
        Self {
            error_schema_version: "0.3.0".into(),
            code,
            severity: Severity::Error,
            message: message.into(),
            location,
            source_line: None,
            hint: None,
        }
    }

    pub fn with_source_line(mut self, line: impl Into<String>) -> Self {
        self.source_line = Some(line.into());
        self
    }

    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// Render as human-readable CLI text (with ANSI colors disabled for now).
    pub fn render_human(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "{}[{}]: {}\n",
            self.severity.as_str(),
            self.code.as_str(),
            self.message
        ));
        out.push_str(&format!(
            "  --> {}:{}:{}\n",
            self.location.file, self.location.line, self.location.col
        ));
        if let Some(src) = &self.source_line {
            out.push_str(&format!("  |\n{:>3} | {}\n", self.location.line, src));
        }
        if let Some(hint) = &self.hint {
            out.push_str(&format!("  = hint: {}\n", hint));
        }
        out
    }

    /// Render as JSON (single object). Phase 3 will add JSONL streaming.
    pub fn render_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".into())
    }
}

/// Result alias used throughout the WLWL toolchain.
pub type WlwlResult<T> = Result<T, WlwlError>;

/// The unified error enum returned by lexer/parser/eval.
#[derive(Debug, Clone, thiserror::Error)]
pub enum WlwlError {
    #[error("{}", .0.render_human())]
    Diagnostic(WlwlDiagnostic),
}

impl WlwlError {
    pub fn diagnostic(&self) -> &WlwlDiagnostic {
        match self {
            WlwlError::Diagnostic(d) => d,
        }
    }
}

impl From<WlwlDiagnostic> for WlwlError {
    fn from(d: WlwlDiagnostic) -> Self {
        WlwlError::Diagnostic(d)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_to_string() {
        assert_eq!(ErrorCode::E0001.as_str(), "E0001");
        assert_eq!(ErrorCode::E0100.as_str(), "E0100");
    }

    #[test]
    fn diagnostic_human_render() {
        let d = WlwlDiagnostic::new(
            ErrorCode::E0013,
            "expected ';'",
            Location::point("t.wl", 1, 9),
        )
        .with_source_line("LET(x, 1)")
        .with_hint("add ';' at end of statement");
        let s = d.render_human();
        assert!(s.contains("E0013"));
        assert!(s.contains("expected ';'"));
        assert!(s.contains("LET(x, 1)"));
    }

    #[test]
    fn diagnostic_json_has_schema_version() {
        let d = WlwlDiagnostic::new(
            ErrorCode::E0020,
            "undefined name 'foo'",
            Location::point("t.wl", 1, 1),
        );
        let j = d.render_json();
        assert!(j.contains("\"error_schema_version\""));
        assert!(j.contains("\"0.3.0\""));
        assert!(j.contains("\"E0020\""));
        // v0.3 §14.2: severity must be lowercase ("error" / "warning" / "note")
        assert!(j.contains("\"severity\": \"error\""));
    }
}
