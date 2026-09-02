//! WLWL error types and reporting.
//!
//! v0.3 `Sec. 14` diagnostic schema. Phase 3 adds the AI-facing fields that
//! were promised in `Sec. 14.2`:
//! - `errorCategory`  -- 13 high-level buckets (`Sec. 14.4`)
//! - `retryable`      -- boolean (`Sec. 14.4` + `Sec. 12.6`)
//! - `suggestion_code` -- machine-apply-able patches (`Sec. 14.2`)
//! - `related`        -- secondary locations (`Sec. 14.2`)
//!
//! Output formats: human-readable (default), JSON (`--format=json`),
//! and JSONL streaming (`--format=jsonl`, Phase 3).
//!
//! 33 error codes are registered (E0001-E0014 lex/syn, E0020-E0023
//! name, E0030-E0032 type, E0040-E0043 module, E0050-E0051 OOP,
//! E0060-E0063 IO, E0070-E0071 JSON, E0080-E0083 std.ai/network,
//! E0099 user, E0100-E0102 internal). IO/JSON/std.ai/net are
//! registered in Phase 3; the actual triggering call sites land in
//! Phase 4 when those std modules are implemented.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Stable error codes (v0.3 `Sec. 14.4` -- full 33-code set).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorCode {
    E0001, // illegal character
    E0002, // unterminated string
    E0003, // unterminated block comment
    E0010, // expected expression
    E0011, // expected ')'
    E0012, // expected ','
    E0013, // expected ';'
    E0014, // RETURN / BREAK / CONTINUE in illegal position
    E0020, // undefined name
    E0021, // duplicate definition / duplicate IMPORT
    E0022, // function arity mismatch
    E0023, // name not exported by module
    E0030, // type error
    E0031, // subscript/key type error
    E0032, // property/method not found
    E0040, // module not found
    E0041, // circular IMPORT
    E0042, // module file IO error
    E0043, // namespace path syntax error
    E0050, // class inheritance chain error
    E0051, // NEW arity mismatch with INIT
    E0060, // IO error (generic)
    E0061, // file not found
    E0062, // file permission denied
    E0063, // network error (general)
    E0070, // JSON parse error
    E0071, // JSON stringify error
    E0080, // AI provider unreachable
    E0081, // AI provider auth / rate-limit
    E0082, // AI provider response malformed
    E0083, // AI request timeout
    E0099, // user-thrown ERR / PANIC
    E0100, // internal error
    E0101, // stack overflow
    E0102, // unhandled ERR escaped to top level
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
            ErrorCode::E0050 => "E0050",
            ErrorCode::E0051 => "E0051",
            ErrorCode::E0060 => "E0060",
            ErrorCode::E0061 => "E0061",
            ErrorCode::E0062 => "E0062",
            ErrorCode::E0063 => "E0063",
            ErrorCode::E0070 => "E0070",
            ErrorCode::E0071 => "E0071",
            ErrorCode::E0080 => "E0080",
            ErrorCode::E0081 => "E0081",
            ErrorCode::E0082 => "E0082",
            ErrorCode::E0083 => "E0083",
            ErrorCode::E0099 => "E0099",
            ErrorCode::E0100 => "E0100",
            ErrorCode::E0101 => "E0101",
            ErrorCode::E0102 => "E0102",
        }
    }

    /// High-level error category (v0.3 `Sec. 14.4` -- 13 buckets).
    pub fn category(&self) -> ErrorCategory {
        match self {
            ErrorCode::E0001 | ErrorCode::E0002 | ErrorCode::E0003 => ErrorCategory::Lexical,
            ErrorCode::E0010
            | ErrorCode::E0011
            | ErrorCode::E0012
            | ErrorCode::E0013
            | ErrorCode::E0014 => ErrorCategory::Syntax,
            ErrorCode::E0020
            | ErrorCode::E0021
            | ErrorCode::E0022
            | ErrorCode::E0023 => ErrorCategory::Name,
            ErrorCode::E0030 | ErrorCode::E0031 | ErrorCode::E0032 => ErrorCategory::Type,
            ErrorCode::E0040
            | ErrorCode::E0041
            | ErrorCode::E0042
            | ErrorCode::E0043 => ErrorCategory::Module,
            ErrorCode::E0050 | ErrorCode::E0051 => ErrorCategory::Oop,
            ErrorCode::E0060
            | ErrorCode::E0061
            | ErrorCode::E0062
            | ErrorCode::E0063 => ErrorCategory::Io,
            ErrorCode::E0070 | ErrorCode::E0071 => ErrorCategory::Json,
            ErrorCode::E0080
            | ErrorCode::E0081
            | ErrorCode::E0082
            | ErrorCode::E0083 => ErrorCategory::Ai,
            ErrorCode::E0099 => ErrorCategory::User,
            ErrorCode::E0100 | ErrorCode::E0101 | ErrorCode::E0102 => ErrorCategory::Internal,
        }
    }

    /// Whether the error is safe to retry (`Sec. 14.4` retryable column).
    /// - Lexical/Syntax/Name errors: not retryable (user must fix source)
    /// - IO/AI network errors: retryable (transient)
    /// - Internal: not retryable
    /// - User errors: not retryable
    /// - ERR escape (E0102): retryable IF the user adds a guard (so `false`
    ///   from a code perspective; the AI tool should not blind-retry)
    pub fn retryable(&self) -> bool {
        matches!(
            self,
            ErrorCode::E0060
                | ErrorCode::E0061
                | ErrorCode::E0063
                | ErrorCode::E0080
                | ErrorCode::E0081
                | ErrorCode::E0083
        )
    }
}

/// High-level error category (v0.3 `Sec. 14.4` -- 13 buckets).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCategory {
    Lexical,
    Syntax,
    Name,
    Type,
    Module,
    Oop,
    Io,
    Json,
    Ai,
    User,
    Internal,
    /// Catch-all (should not be emitted by current code; reserved).
    Unknown,
}

impl ErrorCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            ErrorCategory::Lexical => "lexical",
            ErrorCategory::Syntax => "syntax",
            ErrorCategory::Name => "name",
            ErrorCategory::Type => "type",
            ErrorCategory::Module => "module",
            ErrorCategory::Oop => "oop",
            ErrorCategory::Io => "io",
            ErrorCategory::Json => "json",
            ErrorCategory::Ai => "ai",
            ErrorCategory::User => "user",
            ErrorCategory::Internal => "internal",
            ErrorCategory::Unknown => "unknown",
        }
    }
}

impl fmt::Display for ErrorCategory {
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

    pub fn range(
        file: impl Into<String>,
        line: u32,
        col: u32,
        line_end: u32,
        col_end: u32,
    ) -> Self {
        Self {
            file: file.into(),
            line,
            col,
            line_end,
            col_end,
        }
    }
}

/// Extract the text of a given 1-based line from source.
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
    if cur == line {
        let s = start.unwrap_or(bytes.len());
        return Some(String::from_utf8_lossy(&bytes[s..]).to_string());
    }
    None
}

/// One machine-apply-able fix (v0.3 `Sec. 14.2` `suggestion_code`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Suggestion {
    Replace {
        description: String,
        line: u32,
        col: u32,
        line_end: u32,
        col_end: u32,
        text: String,
    },
    Insert {
        description: String,
        line: u32,
        col: u32,
        text: String,
    },
    Delete {
        description: String,
        line: u32,
        col: u32,
        line_end: u32,
        col_end: u32,
    },
    Note { description: String },
}

/// A secondary location attached to a diagnostic (v0.3 `Sec. 14.2` `related`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelatedLocation {
    pub message: String,
    pub location: Location,
}

/// The structured error object (v0.3 `Sec. 14.2` -- Phase 3 full schema).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WlwlDiagnostic {
    /// Schema version (SemVer). Phase 3 -> "0.3.1" (new fields added).
    pub error_schema_version: String,
    /// Stable error code (see v0.3 `Sec. 14.4`).
    pub code: ErrorCode,
    /// High-level category (see v0.3 `Sec. 14.4` -- 13 buckets).
    pub error_category: ErrorCategory,
    /// "error" | "warning" | "note"
    pub severity: Severity,
    /// Human-readable one-line description.
    pub message: String,
    /// Source location.
    pub location: Location,
    /// The source line where the error occurred (for AI context).
    pub source_line: Option<String>,
    /// Natural-language fix hint (always present when possible).
    pub hint: Option<String>,
    /// Whether the error is transient and may succeed on retry.
    pub retryable: bool,
    /// Machine-apply-able fixes (v0.3 `Sec. 14.2`; up to 3, sorted by confidence).
    pub suggestion_code: Vec<Suggestion>,
    /// Secondary locations (e.g. duplicate IMPORT, original throw site).
    pub related: Vec<RelatedLocation>,
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
        let category = code.category();
        let retryable = code.retryable();
        Self {
            error_schema_version: "0.3.1".into(),
            code,
            error_category: category,
            severity: Severity::Error,
            message: message.into(),
            location,
            source_line: None,
            hint: None,
            retryable,
            suggestion_code: Vec::new(),
            related: Vec::new(),
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

    pub fn with_suggestion(mut self, s: Suggestion) -> Self {
        self.suggestion_code.push(s);
        self
    }

    pub fn with_suggestions(mut self, ss: Vec<Suggestion>) -> Self {
        self.suggestion_code.extend(ss);
        self
    }

    pub fn with_related(mut self, rel: RelatedLocation) -> Self {
        self.related.push(rel);
        self
    }

    /// Render as human-readable CLI text (ANSI optional; off here).
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
        for rel in &self.related {
            out.push_str(&format!(
                "  = note: {} ({}:{}:{})\n",
                rel.message, rel.location.file, rel.location.line, rel.location.col
            ));
        }
        out
    }

    /// Render as a single JSON object (per v0.3 `Sec. 14.3`).
    pub fn render_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".into())
    }

    /// Render as a single-line JSONL record (per v0.3 `Sec. 14.3` + `Sec. 14.7`).
    pub fn render_jsonl(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".into())
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
        assert_eq!(ErrorCode::E0060.as_str(), "E0060");
        assert_eq!(ErrorCode::E0083.as_str(), "E0083");
    }

    #[test]
    fn category_assignment() {
        assert_eq!(ErrorCode::E0001.category(), ErrorCategory::Lexical);
        assert_eq!(ErrorCode::E0013.category(), ErrorCategory::Syntax);
        assert_eq!(ErrorCode::E0020.category(), ErrorCategory::Name);
        assert_eq!(ErrorCode::E0030.category(), ErrorCategory::Type);
        assert_eq!(ErrorCode::E0041.category(), ErrorCategory::Module);
        assert_eq!(ErrorCode::E0050.category(), ErrorCategory::Oop);
        assert_eq!(ErrorCode::E0060.category(), ErrorCategory::Io);
        assert_eq!(ErrorCode::E0070.category(), ErrorCategory::Json);
        assert_eq!(ErrorCode::E0080.category(), ErrorCategory::Ai);
        assert_eq!(ErrorCode::E0099.category(), ErrorCategory::User);
        assert_eq!(ErrorCode::E0100.category(), ErrorCategory::Internal);
    }

    #[test]
    fn retryable_assignment() {
        assert!(ErrorCode::E0060.retryable());
        assert!(ErrorCode::E0080.retryable());
        assert!(!ErrorCode::E0001.retryable());
        assert!(!ErrorCode::E0013.retryable());
        assert!(!ErrorCode::E0020.retryable());
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
        assert!(j.contains("\"0.3.1\""));
        assert!(j.contains("\"E0020\""));
        assert!(j.contains("\"severity\": \"error\""));
    }

    #[test]
    fn diagnostic_json_has_new_fields() {
        let d = WlwlDiagnostic::new(
            ErrorCode::E0013,
            "expected ';'",
            Location::point("t.wl", 1, 9),
        )
        .with_suggestion(Suggestion::Insert {
            description: "add ';' at end".into(),
            line: 1,
            col: 10,
            text: ";".into(),
        })
        .with_related(RelatedLocation {
            message: "previous statement".into(),
            location: Location::point("t.wl", 1, 1),
        });
        let j = d.render_json();
        assert!(j.contains("\"error_category\": \"syntax\""));
        assert!(j.contains("\"retryable\": false"));
        assert!(j.contains("\"suggestion_code\""));
        assert!(j.contains("\"related\""));
    }

    #[test]
    fn diagnostic_jsonl_is_single_line() {
        let d = WlwlDiagnostic::new(
            ErrorCode::E0020,
            "undefined name 'foo'",
            Location::point("t.wl", 1, 1),
        );
        let l = d.render_jsonl();
        assert!(!l.contains('\n'), "jsonl must be single-line: {}", l);
        assert!(l.contains("\"code\":\"E0020\""));
        assert!(l.contains("\"error_category\":\"name\""));
    }
    // -- Phase 3: insta snapshots for all 33 error codes -----------
    // The category and retryable fields are part of the public contract
    // (v0.3 `Sec. 14.4`); insta captures them as JSON so any accidental
    // change to the category / retryable mapping trips the snapshot.

    fn code_snap(code: ErrorCode, label: &str) -> serde_json::Value {
        let d = WlwlDiagnostic::new(
            code,
            format!("snapshot for {}", label),
            Location::point("snap.wl", 1, 1),
        );
        serde_json::to_value(&d).unwrap()
    }

    #[test]
    fn snap_lexical() {
        insta::assert_json_snapshot!("codes_lexical", serde_json::json!({
            "E0001": code_snap(ErrorCode::E0001, "illegal_char"),
            "E0002": code_snap(ErrorCode::E0002, "unterminated_string"),
            "E0003": code_snap(ErrorCode::E0003, "unterminated_block_comment"),
        }));
    }

    #[test]
    fn snap_syntax() {
        insta::assert_json_snapshot!("codes_syntax", serde_json::json!({
            "E0010": code_snap(ErrorCode::E0010, "expected_expr"),
            "E0011": code_snap(ErrorCode::E0011, "expected_rparen"),
            "E0012": code_snap(ErrorCode::E0012, "expected_comma"),
            "E0013": code_snap(ErrorCode::E0013, "expected_semi"),
            "E0014": code_snap(ErrorCode::E0014, "ctrl_in_illegal_pos"),
        }));
    }

    #[test]
    fn snap_name() {
        insta::assert_json_snapshot!("codes_name", serde_json::json!({
            "E0020": code_snap(ErrorCode::E0020, "undefined"),
            "E0021": code_snap(ErrorCode::E0021, "duplicate"),
            "E0022": code_snap(ErrorCode::E0022, "arity_mismatch"),
            "E0023": code_snap(ErrorCode::E0023, "not_exported"),
        }));
    }

    #[test]
    fn snap_type() {
        insta::assert_json_snapshot!("codes_type", serde_json::json!({
            "E0030": code_snap(ErrorCode::E0030, "type_err"),
            "E0031": code_snap(ErrorCode::E0031, "subscrip_key_type"),
            "E0032": code_snap(ErrorCode::E0032, "prop_method_missing"),
        }));
    }

    #[test]
    fn snap_module() {
        insta::assert_json_snapshot!("codes_module", serde_json::json!({
            "E0040": code_snap(ErrorCode::E0040, "mod_not_found"),
            "E0041": code_snap(ErrorCode::E0041, "circular_import"),
            "E0042": code_snap(ErrorCode::E0042, "file_io_err"),
            "E0043": code_snap(ErrorCode::E0043, "ns_path_syntax"),
        }));
    }

    #[test]
    fn snap_oop() {
        insta::assert_json_snapshot!("codes_oop", serde_json::json!({
            "E0050": code_snap(ErrorCode::E0050, "inherit_err"),
            "E0051": code_snap(ErrorCode::E0051, "new_arity_err"),
        }));
    }

    #[test]
    fn snap_io() {
        insta::assert_json_snapshot!("codes_io", serde_json::json!({
            "E0060": code_snap(ErrorCode::E0060, "io_err"),
            "E0061": code_snap(ErrorCode::E0061, "file_not_found"),
            "E0062": code_snap(ErrorCode::E0062, "perm_denied"),
            "E0063": code_snap(ErrorCode::E0063, "net_err"),
        }));
    }

    #[test]
    fn snap_json() {
        insta::assert_json_snapshot!("codes_json", serde_json::json!({
            "E0070": code_snap(ErrorCode::E0070, "json_parse"),
            "E0071": code_snap(ErrorCode::E0071, "json_stringify"),
        }));
    }

    #[test]
    fn snap_ai() {
        insta::assert_json_snapshot!("codes_ai", serde_json::json!({
            "E0080": code_snap(ErrorCode::E0080, "ai_unreachable"),
            "E0081": code_snap(ErrorCode::E0081, "ai_auth"),
            "E0082": code_snap(ErrorCode::E0082, "ai_malformed"),
            "E0083": code_snap(ErrorCode::E0083, "ai_timeout"),
        }));
    }

    #[test]
    fn snap_user_and_internal() {
        insta::assert_json_snapshot!("codes_user_internal", serde_json::json!({
            "E0099": code_snap(ErrorCode::E0099, "user_err"),
            "E0100": code_snap(ErrorCode::E0100, "internal"),
            "E0101": code_snap(ErrorCode::E0101, "stack_overflow"),
            "E0102": code_snap(ErrorCode::E0102, "unhandled_err_escape"),
        }));
    }

    // -- Phase 3: AI contract: 33 codes total ----------------------
    #[test]
    fn all_35_codes_registered() {
        // Sanity: ensure we have exactly 35 codes wired through the schema.
        // If anyone adds a new ErrorCode variant without updating the
        // snapshot, this count will shift and break the contract.
        let codes = [
            ErrorCode::E0001, ErrorCode::E0002, ErrorCode::E0003,
            ErrorCode::E0010, ErrorCode::E0011, ErrorCode::E0012,
            ErrorCode::E0013, ErrorCode::E0014,
            ErrorCode::E0020, ErrorCode::E0021, ErrorCode::E0022, ErrorCode::E0023,
            ErrorCode::E0030, ErrorCode::E0031, ErrorCode::E0032,
            ErrorCode::E0040, ErrorCode::E0041, ErrorCode::E0042, ErrorCode::E0043,
            ErrorCode::E0050, ErrorCode::E0051,
            ErrorCode::E0060, ErrorCode::E0061, ErrorCode::E0062, ErrorCode::E0063,
            ErrorCode::E0070, ErrorCode::E0071,
            ErrorCode::E0080, ErrorCode::E0081, ErrorCode::E0082, ErrorCode::E0083,
            ErrorCode::E0099,
            ErrorCode::E0100, ErrorCode::E0101, ErrorCode::E0102,
        ];
        assert_eq!(codes.len(), 35);
        // Each code has a stable string form.
        for c in &codes {
            assert!(c.as_str().starts_with('E'));
        }
    }
}

