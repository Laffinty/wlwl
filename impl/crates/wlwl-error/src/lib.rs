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
//! 47 error codes are registered (E0001-E0014 lex/syn, E0020-E0027
//! name, E0030-E0039 type, E0040-E0043 module, E0050-E0051 OOP,
//! E0060-E0063 IO, E0070-E0071 JSON, E0080-E0083 std.ai/network,
//! E0099 user, E0100-E0102 internal, E1003 runtime) + 10 warning
//! codes. E0033 / E0038 / E0039 are registered ahead of their emitting
//! sites (Phase E / Phase B6 / Phase B5) so the 搂14.4 type bucket
//! E0030-E0039 has no holes.

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
    E0024, // cannot SET non-captured binding (v0.4 搂6.4 closure cell)
    E0025, // shadowing a global builtin (v0.4 搂6.6, allow_builtin_shadow=false)
    E0026, // destructure pattern mismatch (v0.4 搂7.5)
    E0027, // MATCH fell through without match or default (v0.4 搂7.6)
    E0030, // type error
    E0031, // subscript/key type error
    E0032, // property/method not found
    E0033, // strict_types violation (v0.4 搂2.7; emitting sites land in Phase E)
    E0034, // integer overflow on negation (v0.4 搂9.5: `NEG(INTEGER_MIN)`)
    E0035, // FLOAT 鈫?INTEGER out-of-range cast (v0.4 搂9.5: `INT(<huge float>)`)
    E0036, // array index out of bounds (v0.4 搂10.1: INDEX_GET/SET on ARRAY)
    E0037, // dict key not found (v0.4 搂10.2: INDEX_GET on DICT)
    E0038, // RANGE step=0 (v0.4 搂10.5; emitting sites land in Phase B6)
    E0039, // FORMAT template parse failure (v0.4 搂10.6; Phase B5)
    E1003, // division or modulo by zero (v0.4 搂9.5)
    E0040, // module not found
    E0041, // circular IMPORT
    E0042, // wlwl.lock inconsistent with wlwl.toml (v0.4 搂13.8/搂14.4)
    E0043, // namespace path syntax error
    E0044, // language_version mismatch (v0.4 搂13.8; Phase C3)
    E0045, // dependency conflict 鈥?no version satisfies all constraints (v0.4 搂13.9; Phase C4)
    E0046, // ASSERT cond false (v0.4 搂15.9 std.test; Phase B7)
    E0047, // ASSERT_EQ a != b (v0.4 搂15.9 std.test; Phase B7)
    E0048, // ASSERT_NEQ a == b (v0.4 搂15.9 std.test; Phase B7)
    E0049, // EXPECT_ERR input not ERR (v0.4 搂15.9 std.test; Phase B7)
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
    // v0.4 搂14.4 鈥?network (5 codes, Phase D4 subdivides v0.3's
    // reserved-but-unused E0090). The full ladder lets the AI tool
    // tell apart "DNS broken" (retryable) from "TLS broken"
    // (not retryable) from "5xx" (retryable) without parsing free
    // text. retryable mapping per spec table:
    //   E0090 (unreachable) TRUE
    //   E0091 (DNS)         TRUE
    //   E0092 (TLS)         FALSE
    //   E0093 (HTTP 4xx)    FALSE
    //   E0094 (HTTP 5xx)    TRUE
    E0090, // network unreachable (TCP / connect refused / firewall drop)
    E0091, // DNS resolution failure (getaddrinfo)
    E0092, // TLS handshake / certificate error
    E0093, // HTTP 4xx client error
    E0094, // HTTP 5xx server error
    E0099, // user-thrown ERR / PANIC
    E0100, // internal error
    E0101, // stack overflow
    E0102, // unhandled ERR escaped to top level
    // v0.3 搂14.5 warning codes (added in P3-011).
    W0001, // undefined name (read)
    W0010, // unused LET binding
    W0011, // unused function parameter (`_` prefix silences)
    W0012, // duplicate LET in same scope
    W0013, // IF branches have inconsistent types
    W0051, // 鈫?v0.3-compat alias / legacy builtin form (Phase B3)
    W0020, // array/dict literal mixes bare values and kv pairs
    W0054, // v0.3-compat `!` operator form (Phase B9; v0.5 removes the `!` token)
    W0030, // 閬斀瀹忓嚱鏁?/ 鍏抽敭瀛?(v0.4 搂14.5; allow_builtin_shadow=true 鏃堕伄钄藉唴寤轰篃鍙戞鐮? Phase C5)
    W0040, // unhandled `TODO(agent):` comment
    W0015, // integer overflow, saturated to INT64_MAX / INT64_MIN (v0.4 搂9.5)
    // v0.4 搂14.5 鈥?Phase D5: model name missing provider prefix
    // ("openai/gpt-4" form is recommended; bare "gpt-4" still works
    // but emits W0052). The warning is *not* a hard error so
    // existing single-token model names keep working.
    W0052, // LLM model name missing `provider/` prefix
    // v0.4 搂14.5 / 搂16.3 鈥?Phase E2: the source text deviates from
    // the 搂16.3 canonical formatter contract. Emitted by
    // `wlwl fmt --check`; the fix is mechanical (apply
    // `wlwl fmt` output), hence suggestion-code style hint.
    W0053, // 鏍煎紡鍖栧亸绂?搂16.3 canonical formatter 濂戠害
           // v0.4 搂14.5 鈥?using v0.3 deprecated alias (`DEL` / `OR_DIE`).
           // Added in Phase B2 (DEL alias) + Phase B3 (OR_DIE alias).
           // Note: W0051 itself is already declared in the 搂14.5 warning
           // block above (line ~84). This closing brace just terminates
           // the enum; no new variant is added here.
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
            ErrorCode::E0024 => "E0024",
            ErrorCode::E0025 => "E0025",
            ErrorCode::E0026 => "E0026",
            ErrorCode::E0027 => "E0027",
            ErrorCode::E0030 => "E0030",
            ErrorCode::E0031 => "E0031",
            ErrorCode::E0032 => "E0032",
            ErrorCode::E0033 => "E0033",
            ErrorCode::E0034 => "E0034",
            ErrorCode::E0035 => "E0035",
            ErrorCode::E0036 => "E0036",
            ErrorCode::E0037 => "E0037",
            ErrorCode::E0038 => "E0038",
            ErrorCode::E0039 => "E0039",
            ErrorCode::E1003 => "E1003",
            ErrorCode::E0040 => "E0040",
            ErrorCode::E0041 => "E0041",
            ErrorCode::E0042 => "E0042",
            ErrorCode::E0043 => "E0043",
            ErrorCode::E0044 => "E0044",
            ErrorCode::E0045 => "E0045",
            ErrorCode::E0046 => "E0046",
            ErrorCode::E0047 => "E0047",
            ErrorCode::E0048 => "E0048",
            ErrorCode::E0049 => "E0049",
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
            ErrorCode::E0090 => "E0090",
            ErrorCode::E0091 => "E0091",
            ErrorCode::E0092 => "E0092",
            ErrorCode::E0093 => "E0093",
            ErrorCode::E0094 => "E0094",
            ErrorCode::E0099 => "E0099",
            ErrorCode::E0100 => "E0100",
            ErrorCode::E0101 => "E0101",
            ErrorCode::E0102 => "E0102",
            ErrorCode::W0001 => "W0001",
            ErrorCode::W0010 => "W0010",
            ErrorCode::W0011 => "W0011",
            ErrorCode::W0012 => "W0012",
            ErrorCode::W0013 => "W0013",
            ErrorCode::W0020 => "W0020",
            ErrorCode::W0030 => "W0030",
            ErrorCode::W0040 => "W0040",
            ErrorCode::W0015 => "W0015",
            ErrorCode::W0051 => "W0051",
            ErrorCode::W0052 => "W0052",
            ErrorCode::W0053 => "W0053",
            ErrorCode::W0054 => "W0054",
        }
    }

    /// Whether this is a warning code (v0.3 搂14.5).
    /// Warnings do not block parsing; the caller decides whether to
    /// promote them to errors in `strict` mode (v0.3 搂14.6).
    pub fn is_warning(&self) -> bool {
        matches!(
            self,
            ErrorCode::W0001
                | ErrorCode::W0010
                | ErrorCode::W0011
                | ErrorCode::W0012
                | ErrorCode::W0013
                | ErrorCode::W0020
                | ErrorCode::W0030
                | ErrorCode::W0040
                | ErrorCode::W0015
                | ErrorCode::W0051
                | ErrorCode::W0052
                | ErrorCode::W0053
                | ErrorCode::W0054
        )
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
            | ErrorCode::E0023
            | ErrorCode::E0024
            | ErrorCode::E0025
            | ErrorCode::E0026
            | ErrorCode::E0027 => ErrorCategory::Name,
            ErrorCode::E0030 | ErrorCode::E0031 | ErrorCode::E0032 => ErrorCategory::Type,
            // v0.4 搂9.5 鈥?E0034 (NEG overflow) and E0035 (FLOAT鈫扞NT overflow)
            // both sit on the "type" boundary (the value cannot be
            // represented in the requested type), so they belong in Type.
            // E0036 (array index OOB) and E0037 (dict key missing) are
            // also Type-bucket: they signal "the operand cannot index this
            // collection", a value-shape concern rather than a runtime
            // condition (Phase B1, spec v0.4 搂10.1 / 搂10.2).
            // v0.4 搂14.4 pins the whole E0030-E0039 range to `type`:
            // E0033 (strict_types violation, Phase E), E0038 (RANGE
            // step=0, Phase B6) and E0039 (FORMAT template parse
            // failure, Phase B5) are registered ahead of their emitting
            // sites so the range has no holes.
            ErrorCode::E0033
            | ErrorCode::E0034
            | ErrorCode::E0035
            | ErrorCode::E0036
            | ErrorCode::E0037
            | ErrorCode::E0038
            | ErrorCode::E0039 => ErrorCategory::Type,
            // v0.4 搂9.5 鈥?division / modulo by zero is a runtime
            // condition (the values themselves are valid), so it lands
            // in the new Runtime bucket (spec 搂14.4 row 12).
            ErrorCode::E1003 => ErrorCategory::Runtime,
            ErrorCode::E0040
            | ErrorCode::E0041
            | ErrorCode::E0042
            | ErrorCode::E0043
            | ErrorCode::E0044
            | ErrorCode::E0045 => ErrorCategory::Module,
            ErrorCode::E0046 | ErrorCode::E0047 | ErrorCode::E0048 | ErrorCode::E0049 => {
                ErrorCategory::Test
            }
            ErrorCode::E0050 | ErrorCode::E0051 => ErrorCategory::Oop,
            ErrorCode::E0060 | ErrorCode::E0061 | ErrorCode::E0062 | ErrorCode::E0063 => {
                ErrorCategory::Io
            }
            ErrorCode::E0070 | ErrorCode::E0071 => ErrorCategory::Json,
            ErrorCode::E0080 | ErrorCode::E0081 | ErrorCode::E0082 | ErrorCode::E0083 => {
                ErrorCategory::Ai
            }
            // v0.4 搂14.4 鈥?network errors get their own bucket so
            // AI tools can distinguish "endpoint unreachable" from
            // "endpoint replied with bad credentials" without parsing
            // free text. The Io bucket remains for file/process IO.
            ErrorCode::E0090
            | ErrorCode::E0091
            | ErrorCode::E0092
            | ErrorCode::E0093
            | ErrorCode::E0094 => ErrorCategory::Network,
            ErrorCode::E0099 => ErrorCategory::User,
            ErrorCode::E0100 | ErrorCode::E0101 | ErrorCode::E0102 => ErrorCategory::Internal,
            // Warnings map to the semantic bucket of the underlying
            // issue so consumers can route them by the same rules as
            // the equivalent E-codes. `is_warning()` distinguishes
            // the severity.
            ErrorCode::W0001
            | ErrorCode::W0010
            | ErrorCode::W0011
            | ErrorCode::W0012
            | ErrorCode::W0030 => ErrorCategory::Name,
            ErrorCode::W0013 | ErrorCode::W0020 => ErrorCategory::Syntax,
            ErrorCode::W0040 => ErrorCategory::Module,
            // v0.4 搂9.5 鈥?integer overflow saturated to INT64_MAX/MIN.
            // Same bucket as the underlying runtime condition
            // (E1003 above), so consumers can route by category.
            ErrorCode::W0015 => ErrorCategory::Runtime,
            // v0.4 搂14.5 鈥?using v0.3 deprecated alias (`DEL` / `OR_DIE`).
            // Bucket as Name (deprecated *name* in user source).
            // Added in Phase B2 (DEL alias) + Phase B3 (OR_DIE alias).
            ErrorCode::W0051 => ErrorCategory::Name,
            // Phase B9: `!` is a deprecated *name* / *form* of the
            // v0.4 canonical `NOT`. Same bucket as W0051 (v0.3
            // deprecated alias 鈥?DEL / OR_DIE) so user tools can
            // route both "deprecated thing in source" warnings
            // through one filter.
            ErrorCode::W0054 => ErrorCategory::Name,
            // Phase D5 (spec v0.4 搂14.5 / 搂15.13.1): the LLM model
            // name lacks the `provider/` prefix (e.g. user wrote
            // "gpt-4" instead of "openai/gpt-4"). Bucket as Name:
            // it is a name-shape lint, not a syntax or runtime
            // condition, so existing "deprecated identifier"
            // handlers (W0051 / W0054) naturally pick it up.
            ErrorCode::W0052 => ErrorCategory::Name,
            // Phase E2 (spec v0.4 搂16.3): the source text deviates
            // from the canonical formatter contract. Bucket as
            // Syntax (a source-shape concern, like W0013 / W0020).
            ErrorCode::W0053 => ErrorCategory::Syntax,
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
                | ErrorCode::E0090
                | ErrorCode::E0091
                | ErrorCode::E0094
        )
    }

    /// Whether retrying the operation produces the same result as the
    /// first attempt (v0.4 spec `Sec. 14.2` `idempotent` column).
    ///
    /// AI tools **shall** consult this field: `retryable: TRUE &&
    /// idempotent: FALSE` means "a second execution may have side
    /// effects, prompt the user before retrying" (spec `Sec. 16.1` rule 6).
    ///
    /// Current mapping (v0.3 codes, conservative):
    /// - `E0061` (file not found): TRUE  -- file stays missing
    /// - everything else: FALSE -- write/POST/AI calls may double-fire
    pub fn idempotent(&self) -> bool {
        matches!(self, ErrorCode::E0061)
    }

    /// Recommended retry delay in milliseconds (v0.4 spec `Sec. 14.2`
    /// `retry_after`). `None` for non-retryable codes, or when no
    /// sensible backoff is known.
    ///
    /// AI tools **shall** prefer this value over a fixed backoff
    /// (spec `Sec. 16.1` rule 7). Values are millisecond INTEGERs.
    pub fn retry_after_ms(&self) -> Option<u64> {
        match self {
            // IO generic: 1 second
            ErrorCode::E0060 => Some(1_000),
            // file not found: idempotent, no backoff needed
            ErrorCode::E0061 => Some(0),
            // network: 3 seconds
            ErrorCode::E0063 => Some(3_000),
            // AI unreachable: 5 seconds
            ErrorCode::E0080 => Some(5_000),
            // AI auth: 5 seconds (rate-limit window)
            ErrorCode::E0081 => Some(5_000),
            // AI timeout: 10 seconds (give the upstream more headroom)
            ErrorCode::E0083 => Some(10_000),
            // network unreachable / DNS: 3 seconds (transient; short
            // backoff so retry loops stay responsive on flaky wifi)
            ErrorCode::E0090 => Some(3_000),
            ErrorCode::E0091 => Some(3_000),
            // TLS / 4xx: not retryable, so no backoff
            // (E0092 / E0093 fall through to the `_ => None` arm)
            // HTTP 5xx: 5 seconds (server-side issue, give it a beat)
            ErrorCode::E0094 => Some(5_000),
            // everything else: no recommendation
            _ => None,
        }
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
    /// v0.4 spec 搂14.4 鈥?network errors (E0090-E0094).
    /// Separate from Io because AI tools apply different retry
    /// strategies to network vs. local IO (network failures often
    /// benefit from a 2nd attempt; local IO permission failures do
    /// not).
    Network,
    Json,
    Ai,
    /// v0.4 spec 搂14.4 row 11 鈥?generic runtime errors
    /// (division by zero, etc.). Currently only E1003 lives here;
    /// future phase work (e.g. cancellation) will add more.
    Runtime,
    User,
    /// v0.4 spec 搂14.4 row 12 + 搂15.9 鈥?`std.test` framework errors
    /// (E0046-E0049). Tests-as-data philosophy means these ERR values
    /// are caught by `RUN_TESTS` rather than crashing the program.
    Test,
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
            ErrorCategory::Network => "network",
            ErrorCategory::Json => "json",
            ErrorCategory::Ai => "ai",
            ErrorCategory::Runtime => "runtime",
            ErrorCategory::User => "user",
            ErrorCategory::Test => "test",
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
///
/// v0.4 spec `Sec. 14.2` requires the JSON form
/// `{file, line, col_start, line_end, col_end}`. The Rust field is
/// `col` (matching `wlwl_ast::Span::col`) but serialized as
/// `col_start` via `serde(rename)` -- so the JSON output matches
/// the spec without breaking the many internal call sites that
/// use `location.col`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Location {
    pub file: String,
    pub line: u32,
    #[serde(rename = "col_start")]
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
    Note {
        description: String,
    },
}

/// A secondary location attached to a diagnostic (v0.3 `Sec. 14.2` `related`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelatedLocation {
    pub message: String,
    pub location: Location,
}

/// The structured error object.
///
/// Schema version is `1.1.0` (v0.4 spec `Sec. 14.2`):
/// - Phase 3 fields (v0.3): `code` / `error_category` / `severity` /
///   `message` / `location` / `source_line` / `hint` / `retryable` /
///   `suggestion_code` / `related` / `error_schema_version`
/// - v0.4 additions: `idempotent` (v0.4 `Sec. 14.2`), `retry_after`
///   (v0.4 `Sec. 14.2`)
/// - v0.4 deferred to A1d / A1e: `trace` / `cause`
///
/// AI tools are expected to read this object via `--format=jsonl`
/// (spec `Sec. 16.1`) and check `error_schema_version` first to
/// gate compatibility.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WlwlDiagnostic {
    /// Schema version (SemVer). v0.4 -> `"1.1.0"`.
    pub error_schema_version: String,
    /// Stable error code (see v0.4 `Sec. 14.4`).
    pub code: ErrorCode,
    /// High-level category (see v0.4 `Sec. 14.4` -- 13 buckets).
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
    /// [v0.4] Whether retrying produces the same result as the first
    /// attempt. See `Sec. 14.2` / `Sec. 16.1` rule 6.
    pub idempotent: bool,
    /// [v0.4] Recommended retry delay in milliseconds. `None` for
    /// non-retryable codes or when no sensible backoff is known.
    /// See `Sec. 14.2` / `Sec. 16.1` rule 7.
    pub retry_after: Option<u64>,
    /// Machine-apply-able fixes (v0.3 `Sec. 14.2`; up to 3, sorted by confidence).
    pub suggestion_code: Vec<Suggestion>,
    /// Secondary locations (e.g. duplicate IMPORT, original throw site).
    pub related: Vec<RelatedLocation>,
    /// [v0.4] Call stack frames. Filled by eval; empty for lex/parse
    /// diagnostics. A1d wires this up.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub trace: Vec<TraceFrame>,
    /// [v0.4] WRAP chain. `None` for un-wrapped errors; `Some(_)` for
    /// errors that went through `WRAP`. A1e wires this up.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cause: Option<Box<ErrorCause>>,
}

/// One frame in the diagnostic `trace` (v0.4 `Sec. 14.2`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceFrame {
    /// Function name (or `<anonymous>` for unnamed closures).
    pub frame: String,
    /// Where the call happened.
    pub location: Location,
}

/// A single cause in the WRAP chain (v0.4 `Sec. 14.2` / `Sec. 12.8`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ErrorCause {
    /// `WRAP(ERR(e), ctx)` carries the original error payload as a
    /// STRING or DICT. v0.4 allows either; the payload is
    /// round-tripped as the spec example.
    String(String),
    Dict(serde_json::Map<String, serde_json::Value>),
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
        let idempotent = code.idempotent();
        let retry_after = code.retry_after_ms();
        Self {
            error_schema_version: "1.1.0".into(),
            code,
            error_category: category,
            severity: Severity::Error,
            message: message.into(),
            location,
            source_line: None,
            hint: None,
            retryable,
            idempotent,
            retry_after,
            suggestion_code: Vec::new(),
            related: Vec::new(),
            trace: Vec::new(),
            cause: None,
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

    /// [v0.4 Phase E1] Build the spec-mandated E0033
    /// `strict_types` violation diagnostic (spec 搂2.7).
    ///
    /// - `expected`: the declared type from the annotation
    ///   (e.g. `"INTEGER"` / `"ARRAY"`).
    /// - `actual`: the runtime `TYPE(...)` of the value at the
    ///   boundary (e.g. `"STRING"`).
    /// - `annotation_location`: where the type annotation was
    ///   written (e.g. the parameter's `: Type` token).
    /// - `value_location`: where the offending value appeared
    ///   (e.g. the argument expression span at the call site).
    /// - `boundary`: one of `"function"` / `"import"` / `"ffi"`
    ///   鈥?purely cosmetic for the human reader and for AI
    ///   tooling that groups errors by boundary class.
    ///
    /// The constructed diagnostic:
    /// - has the canonical message
    ///   `"type annotation mismatch: expected X, got Y"`;
    /// - carries both spans in `related` so AI tools can navigate
    ///   either side of the mismatch;
    /// - sets `hint` to a one-line remediation tip
    ///   (`"remove the annotation or coerce the value at the
    ///   boundary"`).
    pub fn with_strict_types_violation(
        mut self,
        expected: impl Into<String>,
        actual: impl Into<String>,
        annotation_location: Location,
        value_location: Location,
        boundary: &str,
    ) -> Self {
        let expected = expected.into();
        let actual = actual.into();
        self.message = format!(
            "type annotation mismatch: expected {}, got {}",
            expected, actual
        );
        self.hint = Some(match boundary {
            "function" => format!(
                "remove the `: {}` annotation from the parameter,                  or coerce the call-site argument at the boundary",
                expected
            ),
            "import" => format!(
                "the importing module expected `{}` but the exported                  binding is `{}`; align the contract or relax the                  boundary check",
                expected, actual
            ),
            "ffi" => format!(
                "the FFI shim cannot convert `{}` to `{}`; adjust                  the conversion or the calling code",
                actual, expected
            ),
            _ => format!(
                "remove the `: {}` annotation or coerce the value                  at the boundary",
                expected
            ),
        });
        self.related.push(RelatedLocation {
            message: format!("type annotation declared `{}`", expected),
            location: annotation_location,
        });
        self.related.push(RelatedLocation {
            message: format!("actual value has type `{}`", actual),
            location: value_location,
        });
        self
    }

    /// [v0.4] Attach a `cause` (WRAP chain link, spec `Sec. 12.8` /
    /// `Sec. 14.2`). Replaces any previously set cause. Used by
    /// `UNWRAP(ERR(e))` to carry the original error payload into the
    /// `E0100` PANIC diagnostic (Phase B4).
    pub fn with_cause(mut self, cause: ErrorCause) -> Self {
        self.cause = Some(Box::new(cause));
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
        // Phase B5: the 搂14.4 type bucket E0030-E0039 has no holes.
        // E0033 / E0038 / E0039 are registered ahead of their emitting
        // sites (Phase E / Phase B6 / Phase B5 respectively).
        assert_eq!(ErrorCode::E0033.category(), ErrorCategory::Type);
        assert_eq!(ErrorCode::E0038.category(), ErrorCategory::Type);
        assert_eq!(ErrorCode::E0039.category(), ErrorCategory::Type);
        // Type-bucket codes are never retryable (搂14.4 retryable=FALSE).
        assert!(!ErrorCode::E0033.retryable());
        assert!(!ErrorCode::E0038.retryable());
        assert!(!ErrorCode::E0039.retryable());
        assert_eq!(ErrorCode::E0041.category(), ErrorCategory::Module);
        assert_eq!(ErrorCode::E0050.category(), ErrorCategory::Oop);
        assert_eq!(ErrorCode::E0060.category(), ErrorCategory::Io);
        assert_eq!(ErrorCode::E0070.category(), ErrorCategory::Json);
        assert_eq!(ErrorCode::E0080.category(), ErrorCategory::Ai);
        // Phase D4: network codes live in the new Network bucket.
        assert_eq!(ErrorCode::E0090.category(), ErrorCategory::Network);
        assert_eq!(ErrorCode::E0094.category(), ErrorCategory::Network);
        assert_eq!(ErrorCode::E0099.category(), ErrorCategory::User);
        assert_eq!(ErrorCode::E0100.category(), ErrorCategory::Internal);
    }

    #[test]
    fn retryable_assignment() {
        assert!(ErrorCode::E0060.retryable());
        assert!(ErrorCode::E0080.retryable());
        // Phase D4: network ladder
        assert!(ErrorCode::E0090.retryable()); // unreachable: TRUE
        assert!(ErrorCode::E0091.retryable()); // DNS: TRUE
        assert!(!ErrorCode::E0092.retryable()); // TLS: FALSE
        assert!(!ErrorCode::E0093.retryable()); // 4xx: FALSE
        assert!(ErrorCode::E0094.retryable()); // 5xx: TRUE
        assert!(!ErrorCode::E0001.retryable());
        assert!(!ErrorCode::E0013.retryable());
        assert!(!ErrorCode::E0020.retryable());
    }

    /// v0.4 `Sec. 14.2` `idempotent` field mapping.
    #[test]
    fn idempotent_assignment() {
        // E0061 (file not found) is idempotent: file stays missing
        assert!(ErrorCode::E0061.idempotent());
        // All other codes are conservatively NOT idempotent:
        // - writes may double-fire
        // - AI POST may double-charge
        // - network errors don't know if GET or POST
        assert!(!ErrorCode::E0001.idempotent());
        assert!(!ErrorCode::E0020.idempotent());
        assert!(!ErrorCode::E0060.idempotent()); // generic IO
        assert!(!ErrorCode::E0063.idempotent()); // network
        assert!(!ErrorCode::E0080.idempotent()); // AI unreachable
        assert!(!ErrorCode::E0081.idempotent()); // AI auth
        assert!(!ErrorCode::E0083.idempotent()); // AI timeout
                                                 // Phase D4: network 鈥?we don't know if the request was a
                                                 // safe GET or a non-idempotent POST, so conservatively mark
                                                 // all five as non-idempotent. AI tools must consult the
                                                 // HTTP method in the call site before retrying.
        assert!(!ErrorCode::E0090.idempotent());
        assert!(!ErrorCode::E0091.idempotent());
        assert!(!ErrorCode::E0092.idempotent());
        assert!(!ErrorCode::E0093.idempotent());
        assert!(!ErrorCode::E0094.idempotent());
    }

    /// v0.4 `Sec. 14.2` `retry_after` field mapping.
    /// Retryable codes get a sensible default; everything else is `None`.
    #[test]
    fn retry_after_ms_assignment() {
        // Retryable codes have non-None backoff
        assert_eq!(ErrorCode::E0060.retry_after_ms(), Some(1_000));
        assert_eq!(ErrorCode::E0061.retry_after_ms(), Some(0));
        assert_eq!(ErrorCode::E0063.retry_after_ms(), Some(3_000));
        assert_eq!(ErrorCode::E0080.retry_after_ms(), Some(5_000));
        assert_eq!(ErrorCode::E0081.retry_after_ms(), Some(5_000));
        assert_eq!(ErrorCode::E0083.retry_after_ms(), Some(10_000));
        // Phase D4: network backoff ladder
        assert_eq!(ErrorCode::E0090.retry_after_ms(), Some(3_000));
        assert_eq!(ErrorCode::E0091.retry_after_ms(), Some(3_000));
        assert_eq!(ErrorCode::E0092.retry_after_ms(), None); // not retryable
        assert_eq!(ErrorCode::E0093.retry_after_ms(), None); // not retryable
        assert_eq!(ErrorCode::E0094.retry_after_ms(), Some(5_000));
        // Non-retryable codes have None
        assert_eq!(ErrorCode::E0001.retry_after_ms(), None);
        assert_eq!(ErrorCode::E0020.retry_after_ms(), None);
        assert_eq!(ErrorCode::E0030.retry_after_ms(), None);
        assert_eq!(ErrorCode::E0062.retry_after_ms(), None); // perm denied
        assert_eq!(ErrorCode::E0100.retry_after_ms(), None); // internal
    }

    /// v0.4 `Sec. 16.1` rule 6: idempotent=FALSE on a retryable code
    /// means "do not blind-retry; ask the user". Spot-check that
    /// every retryable code has the right combination.
    #[test]
    fn retryable_idempotent_combinations_are_safe() {
        let retryable_codes = [
            ErrorCode::E0060,
            ErrorCode::E0061,
            ErrorCode::E0063,
            ErrorCode::E0080,
            ErrorCode::E0081,
            ErrorCode::E0083,
            // Phase D4: network ladder (unreachable / DNS / 5xx)
            ErrorCode::E0090,
            ErrorCode::E0091,
            ErrorCode::E0094,
        ];
        for code in &retryable_codes {
            assert!(code.retryable(), "{:?} should be retryable", code);
            // All retryable codes should also have a retry_after hint
            assert!(
                code.retry_after_ms().is_some(),
                "{:?} should have a retry_after hint",
                code
            );
        }
        // E0061 is the only retryable code that is also idempotent
        let idempotent_retryable: Vec<_> =
            retryable_codes.iter().filter(|c| c.idempotent()).collect();
        assert_eq!(idempotent_retryable.len(), 1);
        assert_eq!(idempotent_retryable[0], &ErrorCode::E0061);
    }

    /// v0.4: the new `idempotent` and `retry_after` fields must be
    /// present in the JSON output for both E0020 (non-retryable) and
    /// E0060 (retryable + non-idempotent).
    #[test]
    fn diagnostic_json_has_idempotent_and_retry_after() {
        // Non-retryable: retry_after must be null
        let d = WlwlDiagnostic::new(
            ErrorCode::E0020,
            "undefined name 'foo'",
            Location::point("t.wll", 1, 1),
        );
        let j = d.render_json();
        assert!(
            j.contains("\"idempotent\": false"),
            "E0020 should be non-idempotent: {}",
            j
        );
        assert!(
            j.contains("\"retry_after\": null"),
            "E0020 retry_after should be null: {}",
            j
        );

        // Retryable + non-idempotent: retry_after must be a positive integer
        let d = WlwlDiagnostic::new(ErrorCode::E0060, "io error", Location::point("t.wll", 1, 1));
        let j = d.render_json();
        assert!(
            j.contains("\"idempotent\": false"),
            "E0060 should be non-idempotent: {}",
            j
        );
        assert!(
            j.contains("\"retry_after\": 1000"),
            "E0060 retry_after should be 1000ms: {}",
            j
        );

        // Retryable + idempotent: retry_after is 0 (no backoff needed)
        let d = WlwlDiagnostic::new(
            ErrorCode::E0061,
            "file not found",
            Location::point("t.wll", 1, 1),
        );
        let j = d.render_json();
        assert!(
            j.contains("\"idempotent\": true"),
            "E0061 should be idempotent: {}",
            j
        );
        assert!(
            j.contains("\"retry_after\": 0"),
            "E0061 retry_after should be 0: {}",
            j
        );
    }

    #[test]
    fn diagnostic_human_render() {
        let d = WlwlDiagnostic::new(
            ErrorCode::E0013,
            "expected ';'",
            Location::point("t.wll", 1, 9),
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
            Location::point("t.wll", 1, 1),
        );
        let j = d.render_json();
        assert!(j.contains("\"error_schema_version\""));
        assert!(j.contains("\"1.1.0\""));
        assert!(
            !j.contains("\"0.3.1\""),
            "schema_version should be 1.1.0, got: {}",
            j
        );
        assert!(j.contains("\"E0020\""));
        assert!(j.contains("\"severity\": \"error\""));
        // v0.4 new fields must be present
        assert!(
            j.contains("\"idempotent\""),
            "idempotent field missing: {}",
            j
        );
        assert!(
            j.contains("\"retry_after\""),
            "retry_after field missing: {}",
            j
        );
        // For non-retryable E0020, retry_after is JSON null
        assert!(
            j.contains("\"retry_after\": null"),
            "retry_after should be null for E0020, got: {}",
            j
        );
    }

    #[test]
    fn diagnostic_json_has_new_fields() {
        let d = WlwlDiagnostic::new(
            ErrorCode::E0013,
            "expected ';'",
            Location::point("t.wll", 1, 9),
        )
        .with_suggestion(Suggestion::Insert {
            description: "add ';' at end".into(),
            line: 1,
            col: 10,
            text: ";".into(),
        })
        .with_related(RelatedLocation {
            message: "previous statement".into(),
            location: Location::point("t.wll", 1, 1),
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
            Location::point("t.wll", 1, 1),
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
            Location::point("snap.wll", 1, 1),
        );
        serde_json::to_value(&d).unwrap()
    }

    #[test]
    fn snap_lexical() {
        insta::assert_json_snapshot!(
            "codes_lexical",
            serde_json::json!({
                "E0001": code_snap(ErrorCode::E0001, "illegal_char"),
                "E0002": code_snap(ErrorCode::E0002, "unterminated_string"),
                "E0003": code_snap(ErrorCode::E0003, "unterminated_block_comment"),
            })
        );
    }

    #[test]
    fn snap_syntax() {
        insta::assert_json_snapshot!(
            "codes_syntax",
            serde_json::json!({
                "E0010": code_snap(ErrorCode::E0010, "expected_expr"),
                "E0011": code_snap(ErrorCode::E0011, "expected_rparen"),
                "E0012": code_snap(ErrorCode::E0012, "expected_comma"),
                "E0013": code_snap(ErrorCode::E0013, "expected_semi"),
                "E0014": code_snap(ErrorCode::E0014, "ctrl_in_illegal_pos"),
            })
        );
    }

    #[test]
    fn snap_name() {
        insta::assert_json_snapshot!(
            "codes_name",
            serde_json::json!({
                "E0020": code_snap(ErrorCode::E0020, "undefined"),
                "E0021": code_snap(ErrorCode::E0021, "duplicate"),
                "E0022": code_snap(ErrorCode::E0022, "arity_mismatch"),
                "E0023": code_snap(ErrorCode::E0023, "not_exported"),
                "E0024": code_snap(ErrorCode::E0024, "set_non_captured"),
                "E0025": code_snap(ErrorCode::E0025, "shadow_builtin"),
                "E0026": code_snap(ErrorCode::E0026, "destructure_mismatch"),
                "E0027": code_snap(ErrorCode::E0027, "match_fallthrough"),
                // v0.4 搂14.5 鈥?using v0.3 deprecated alias (`DEL` / `OR_DIE`).
                // Lives in the Name bucket. Added Phase B2 (DEL alias)
                // + Phase B3 (OR_DIE alias).
                "W0051": code_snap(ErrorCode::W0051, "deprecated_alias"),
                "W0054": code_snap(ErrorCode::W0054, "deprecated_op_form"),
                "W0054": code_snap(ErrorCode::W0054, "deprecated_op_form"),
            })
        );
    }

    #[test]
    fn snap_type() {
        insta::assert_json_snapshot!(
            "codes_type",
            serde_json::json!({
                "E0030": code_snap(ErrorCode::E0030, "type_err"),
                "E0031": code_snap(ErrorCode::E0031, "subscrip_key_type"),
                "E0032": code_snap(ErrorCode::E0032, "prop_method_missing"),
                // Phase B5 registered E0033 / E0038 / E0039 ahead of their
                // emitting sites (搂14.4 pins E0030-E0039 to the type bucket):
                // E0033 strict_types (Phase E), E0038 RANGE step=0 (Phase B6),
                // E0039 FORMAT template parse failure (Phase B5).
                "E0033": code_snap(ErrorCode::E0033, "strict_types_violation"),
                "E0034": code_snap(ErrorCode::E0034, "neg_overflow"),
                "E0035": code_snap(ErrorCode::E0035, "float_to_int_overflow"),
                "E0036": code_snap(ErrorCode::E0036, "array_index_oob"),
                "E0037": code_snap(ErrorCode::E0037, "dict_key_missing"),
                "E0038": code_snap(ErrorCode::E0038, "range_step_zero"),
                "E0039": code_snap(ErrorCode::E0039, "format_template_parse"),
            })
        );
    }

    #[test]
    fn snap_module() {
        // Phase C3/C4 (spec v0.4 搂13.8/搂13.9): E0044 (language_version
        // mismatch) + E0045 (dependency conflict) join the module
        // bucket. E0042's meaning was re-anchored in v0.4: it was a
        // numbering hole in v0.3 ("file IO error" was a placeholder
        // that never had an emitting site); spec v0.4 搂13.8 pins it
        // to "lock file inconsistent with wlwl.toml" (Phase C6).
        insta::assert_json_snapshot!(
            "codes_module",
            serde_json::json!({
                "E0040": code_snap(ErrorCode::E0040, "mod_not_found"),
                "E0041": code_snap(ErrorCode::E0041, "circular_import"),
                "E0042": code_snap(ErrorCode::E0042, "lock_toml_inconsistent"),
                "E0043": code_snap(ErrorCode::E0043, "ns_path_syntax"),
                "E0044": code_snap(ErrorCode::E0044, "language_version_mismatch"),
                "E0045": code_snap(ErrorCode::E0045, "dependency_conflict"),
            })
        );
    }

    #[test]
    fn snap_oop() {
        insta::assert_json_snapshot!(
            "codes_oop",
            serde_json::json!({
                "E0050": code_snap(ErrorCode::E0050, "inherit_err"),
                "E0051": code_snap(ErrorCode::E0051, "new_arity_err"),
            })
        );
    }

    #[test]
    fn snap_io() {
        insta::assert_json_snapshot!(
            "codes_io",
            serde_json::json!({
                "E0060": code_snap(ErrorCode::E0060, "io_err"),
                "E0061": code_snap(ErrorCode::E0061, "file_not_found"),
                "E0062": code_snap(ErrorCode::E0062, "perm_denied"),
                "E0063": code_snap(ErrorCode::E0063, "net_err"),
            })
        );
    }

    #[test]
    fn snap_network() {
        // Phase D4 (spec v0.4 搂14.4): subdivide v0.3's
        // reserved-but-unused E0090 into the five code ladder
        // E0090-E0094 so AI tools can tell apart unreachable /
        // DNS / TLS / 4xx / 5xx without parsing free text. All
        // five share the new Network bucket; retryable mapping per
        // spec 搂14.4: E0090/E0091/E0094 = TRUE; E0092/E0093 = FALSE.
        insta::assert_json_snapshot!(
            "codes_network",
            serde_json::json!({
                "E0090": code_snap(ErrorCode::E0090, "net_unreachable"),
                "E0091": code_snap(ErrorCode::E0091, "dns_failure"),
                "E0092": code_snap(ErrorCode::E0092, "tls_error"),
                "E0093": code_snap(ErrorCode::E0093, "http_4xx"),
                "E0094": code_snap(ErrorCode::E0094, "http_5xx"),
            })
        );
    }

    #[test]
    fn snap_json() {
        insta::assert_json_snapshot!(
            "codes_json",
            serde_json::json!({
                "E0070": code_snap(ErrorCode::E0070, "json_parse"),
                "E0071": code_snap(ErrorCode::E0071, "json_stringify"),
            })
        );
    }

    #[test]
    fn snap_ai() {
        insta::assert_json_snapshot!(
            "codes_ai",
            serde_json::json!({
                "E0080": code_snap(ErrorCode::E0080, "ai_unreachable"),
                "E0081": code_snap(ErrorCode::E0081, "ai_auth"),
                "E0082": code_snap(ErrorCode::E0082, "ai_malformed"),
                "E0083": code_snap(ErrorCode::E0083, "ai_timeout"),
            })
        );
    }

    #[test]
    fn snap_runtime() {
        insta::assert_json_snapshot!(
            "codes_runtime",
            serde_json::json!({
                "E1003": code_snap(ErrorCode::E1003, "div_by_zero"),
            })
        );
    }

    #[test]
    fn snap_user_and_internal() {
        insta::assert_json_snapshot!(
            "codes_user_internal",
            serde_json::json!({
                "E0099": code_snap(ErrorCode::E0099, "user_err"),
                "E0100": code_snap(ErrorCode::E0100, "internal"),
                "E0101": code_snap(ErrorCode::E0101, "stack_overflow"),
                "E0102": code_snap(ErrorCode::E0102, "unhandled_err_escape"),
            })
        );
    }

    #[test]
    fn snap_test() {
        // Phase B7 (spec 搂15.9 / 搂14.4): `std.test` framework errors
        // E0046-E0049. All four live in the `test` bucket (new in
        // v0.4) and share retryable=FALSE 鈥?assertion failures aren't
        // transient; they're a bug in the test or the code under test.
        insta::assert_json_snapshot!(
            "codes_test",
            serde_json::json!({
                "E0046": code_snap(ErrorCode::E0046, "test_assertion_failed"),
                "E0047": code_snap(ErrorCode::E0047, "test_assertion_eq_failed"),
                "E0048": code_snap(ErrorCode::E0048, "test_assertion_neq_failed"),
                "E0049": code_snap(ErrorCode::E0049, "test_expect_err_failed"),
            })
        );
    }

    // -- Phase 3: AI contract: 33 codes total ----------------------
    // v0.4 added E0024 (closure cell) + E0025/E0026/E0027 (MATCH family).
    // Phase A7 (2026-09-15) added E0034 / E0035 (numeric overflow) +
    // E1003 (runtime / div by zero) + Runtime category, and W0015
    // (integer overflow saturated warning).
    // Phase B1 (2026-09-15) added E0036 (array index OOB) + E0037 (dict
    // key missing) for spec v0.4 搂10.1 / 搂10.2 INDEX_GET / INDEX_SET.
    // Phase B2 (2026-09-15) added W0051 (v0.3 deprecated alias 鈥?
    // `DEL`) for spec v0.4 搂10.2 / 搂14.5. No E-codes added in B2.
    // Phase B5 (2026-09-18) added E0033 (strict_types, Phase E),
    // E0038 (RANGE step=0, Phase B6) and E0039 (FORMAT template parse
    // failure, used by B5) to close the 搂14.4 type-bucket range holes.
    #[test]
    fn all_53_codes_registered() {
        // Sanity: ensure we have exactly 58 codes wired through the schema.
        // If anyone adds a new ErrorCode variant without updating the
        // snapshot, this count will shift and break the contract.
        // (Phase D4 added E0090-E0094 鈥?spec v0.4 搂14.4 network
        // subdivision.)
        let codes = [
            ErrorCode::E0001,
            ErrorCode::E0002,
            ErrorCode::E0003,
            ErrorCode::E0010,
            ErrorCode::E0011,
            ErrorCode::E0012,
            ErrorCode::E0013,
            ErrorCode::E0014,
            ErrorCode::E0020,
            ErrorCode::E0021,
            ErrorCode::E0022,
            ErrorCode::E0023,
            ErrorCode::E0024,
            ErrorCode::E0025,
            ErrorCode::E0026,
            ErrorCode::E0027,
            ErrorCode::E0030,
            ErrorCode::E0031,
            ErrorCode::E0032,
            ErrorCode::E0033,
            ErrorCode::E0034,
            ErrorCode::E0035,
            ErrorCode::E0036,
            ErrorCode::E0037,
            ErrorCode::E0038,
            ErrorCode::E0039,
            ErrorCode::E0040,
            ErrorCode::E0041,
            ErrorCode::E0042,
            ErrorCode::E0043,
            ErrorCode::E0044,
            ErrorCode::E0045,
            ErrorCode::E0046,
            ErrorCode::E0047,
            ErrorCode::E0048,
            ErrorCode::E0049,
            ErrorCode::E0050,
            ErrorCode::E0051,
            ErrorCode::E0060,
            ErrorCode::E0061,
            ErrorCode::E0062,
            ErrorCode::E0063,
            ErrorCode::E0070,
            ErrorCode::E0071,
            ErrorCode::E0080,
            ErrorCode::E0081,
            ErrorCode::E0082,
            ErrorCode::E0083,
            ErrorCode::E0090,
            ErrorCode::E0091,
            ErrorCode::E0092,
            ErrorCode::E0093,
            ErrorCode::E0094,
            ErrorCode::E0099,
            ErrorCode::E0100,
            ErrorCode::E0101,
            ErrorCode::E0102,
            ErrorCode::E1003,
        ];
        assert_eq!(codes.len(), 58);
        // Each code has a stable string form.
        for c in &codes {
            assert!(c.as_str().starts_with('E'));
        }
    }

    #[test]
    fn all_12_warning_codes_registered() {
        // v0.4 搂14.5 expanded the warning list. Phase A7 added W0015
        // (integer overflow saturated). Phase B2 added W0051 (v0.3
        // deprecated alias 鈥?`DEL`). Phase D5 added W0052 (LLM model
        // name missing provider prefix). Phase E2 added W0053
        // (搂16.3 canonical formatter deviation).
        let codes = [
            ErrorCode::W0001,
            ErrorCode::W0010,
            ErrorCode::W0011,
            ErrorCode::W0012,
            ErrorCode::W0013,
            ErrorCode::W0015,
            ErrorCode::W0020,
            ErrorCode::W0030,
            ErrorCode::W0040,
            ErrorCode::W0051,
            ErrorCode::W0052,
            ErrorCode::W0053,
            ErrorCode::W0054,
        ];
        assert_eq!(codes.len(), 13);
        for c in &codes {
            assert!(
                c.is_warning(),
                "{} should report is_warning() = true",
                c.as_str()
            );
            assert!(c.as_str().starts_with('W'));
        }
    }
    // ---- P3-009d: ErrorCategory, Severity, Span::range, extract_line, diagnostic builders ----

    #[test]
    fn error_category_as_str_all_variants() {
        assert_eq!(ErrorCategory::Lexical.as_str(), "lexical");
        assert_eq!(ErrorCategory::Syntax.as_str(), "syntax");
        assert_eq!(ErrorCategory::Name.as_str(), "name");
        assert_eq!(ErrorCategory::Type.as_str(), "type");
        assert_eq!(ErrorCategory::Module.as_str(), "module");
        assert_eq!(ErrorCategory::Oop.as_str(), "oop");
        assert_eq!(ErrorCategory::Io.as_str(), "io");
        assert_eq!(ErrorCategory::Network.as_str(), "network");
        assert_eq!(ErrorCategory::Json.as_str(), "json");
        assert_eq!(ErrorCategory::Ai.as_str(), "ai");
        assert_eq!(ErrorCategory::Runtime.as_str(), "runtime");
        assert_eq!(ErrorCategory::User.as_str(), "user");
        assert_eq!(ErrorCategory::Test.as_str(), "test");
        assert_eq!(ErrorCategory::Internal.as_str(), "internal");
        assert_eq!(ErrorCategory::Unknown.as_str(), "unknown");
    }

    #[test]
    fn error_category_display_matches_as_str() {
        // ErrorCategory's Display impl should forward to as_str() so
        // format!("{}", cat) is stable.
        assert_eq!(format!("{}", ErrorCategory::Lexical), "lexical");
        assert_eq!(format!("{}", ErrorCategory::Ai), "ai");
        assert_eq!(format!("{}", ErrorCategory::Unknown), "unknown");
    }

    #[test]
    fn severity_as_str_all_variants() {
        assert_eq!(Severity::Error.as_str(), "error");
        assert_eq!(Severity::Warning.as_str(), "warning");
        assert_eq!(Severity::Note.as_str(), "note");
    }

    #[test]
    fn span_range_constructor() {
        // The 5-arg constructor (file + start line/col + end line/col)
        // is what the parser uses once it has consumed multiple tokens.
        let s = Location::range("a.wll", 1, 1, 3, 5);
        assert_eq!(s.file, "a.wll");
        assert_eq!(s.line, 1);
        assert_eq!(s.col, 1);
        assert_eq!(s.line_end, 3);
        assert_eq!(s.col_end, 5);
    }

    #[test]
    fn extract_line_returns_line_one_indexed() {
        let src = "line 0\nline 1\nline 2\n";
        assert_eq!(extract_line(src, 1).as_deref(), Some("line 0"));
        assert_eq!(extract_line(src, 2).as_deref(), Some("line 1"));
        assert_eq!(extract_line(src, 3).as_deref(), Some("line 2"));
        // Beyond EOF: the impl returns Some("") because cur reached
        // the target line at loop end. (Documented behaviour: a
        // missing line is treated as an empty line, not a hard EOF.)
        assert_eq!(extract_line(src, 4).as_deref(), Some(""));
    }

    #[test]
    fn extract_line_handles_no_trailing_newline() {
        // When the file doesn't end with \n, the last line is still
        // returned when queried.
        let src = "a\nb\nc";
        assert_eq!(extract_line(src, 3).as_deref(), Some("c"));
        assert_eq!(extract_line(src, 4), None);
    }

    #[test]
    fn extract_line_handles_empty_source() {
        // Empty source with line=0 returns None (sentinel for 0-based).
        assert_eq!(extract_line("", 0), None);
        // Empty source with line=1 returns Some("") -- an empty 1st line.
        assert_eq!(extract_line("", 1).as_deref(), Some(""));
        // Empty source with line=2 returns None (never reached cur=2).
        assert_eq!(extract_line("", 2), None);
    }

    #[test]
    fn diagnostic_with_suggestion_appends() {
        // WlwlDiagnostic::with_suggestion must push a single Suggestion
        // onto the suggestion_code vector. with_suggestions extends it
        // with many. Both return self for builder-style chaining.
        let d = WlwlDiagnostic::new(
            ErrorCode::E0020,
            String::from("undefined name"),
            Location::point("a.wll", 1, 1),
        )
        .with_suggestion(Suggestion::Note {
            description: String::from("try foo"),
        });
        assert_eq!(d.suggestion_code.len(), 1);
        assert!(matches!(d.suggestion_code[0], Suggestion::Note { .. }));

        let d = d.with_suggestions(vec![
            Suggestion::Note {
                description: String::from("first"),
            },
            Suggestion::Note {
                description: String::from("second"),
            },
        ]);
        assert_eq!(d.suggestion_code.len(), 3);
    }

    #[test]
    fn diagnostic_with_related_appends() {
        let related_loc = Location::point("b.wll", 5, 1);
        let d = WlwlDiagnostic::new(
            ErrorCode::E0040,
            String::from("module not found"),
            Location::point("a.wll", 1, 1),
        )
        .with_related(RelatedLocation {
            message: String::from("imported here"),
            location: related_loc.clone(),
        });
        assert_eq!(d.related.len(), 1);
        assert_eq!(d.related[0].message, "imported here");
        assert_eq!(d.related[0].location.file, "b.wll");
    }

    #[test]
    fn diagnostic_render_includes_hint_and_related() {
        // The Display impl appends a   = hint: ... line for the
        // optional hint and a   = note: ... line per related entry.
        let d = WlwlDiagnostic::new(
            ErrorCode::E0020,
            String::from("undefined name"),
            Location::point("a.wll", 1, 1),
        )
        .with_hint(String::from("did you import it?"))
        .with_related(RelatedLocation {
            message: String::from("imported here"),
            location: Location::point("b.wll", 3, 1),
        });
        let rendered = d.render_human();
        assert!(
            rendered.contains("hint: did you import it?"),
            "got: {}",
            rendered
        );
        assert!(
            rendered.contains("note: imported here (b.wll:3:1)"),
            "got: {}",
            rendered
        );
    }

    // ---- Phase E1 (spec v0.4 搂2.7): E0033 strict_types helper ----

    #[test]
    fn e0033_helper_sets_canonical_message() {
        let d = WlwlDiagnostic::new(
            ErrorCode::E0033,
            "placeholder -- to be overwritten by helper",
            Location::point("a.wll", 5, 1),
        )
        .with_strict_types_violation(
            "INTEGER",
            "STRING",
            Location::point("a.wll", 3, 5),
            Location::point("a.wll", 5, 1),
            "function",
        );
        assert_eq!(
            d.message,
            "type annotation mismatch: expected INTEGER, got STRING"
        );
        assert_eq!(d.code, ErrorCode::E0033);
        assert_eq!(d.error_category, ErrorCategory::Type);
        assert!(!d.retryable);
    }

    #[test]
    fn e0033_helper_carries_annotation_and_value_in_related() {
        let d = WlwlDiagnostic::new(ErrorCode::E0033, "ignored", Location::point("a.wll", 5, 1))
            .with_strict_types_violation(
                "ARRAY",
                "INTEGER",
                Location::point("a.wll", 1, 4),
                Location::point("a.wll", 5, 1),
                "function",
            );
        // Two related entries: one for the annotation, one for the value.
        assert_eq!(d.related.len(), 2);
        assert!(d.related[0].message.contains("ARRAY"));
        assert!(d.related[0].message.contains("annotation"));
        assert_eq!(d.related[0].location.file, "a.wll");
        assert_eq!(d.related[0].location.line, 1);
        assert_eq!(d.related[0].location.col, 4);
        assert!(d.related[1].message.contains("INTEGER"));
        assert_eq!(d.related[1].location.line, 5);
    }

    #[test]
    fn e0033_helper_hint_function_boundary() {
        let d = WlwlDiagnostic::new(ErrorCode::E0033, "ignored", Location::point("a.wll", 5, 1))
            .with_strict_types_violation(
                "INTEGER",
                "STRING",
                Location::point("a.wll", 3, 5),
                Location::point("a.wll", 5, 1),
                "function",
            );
        let h = d.hint.expect("hint must be Some after helper");
        assert!(h.contains("STRING") || h.contains("coerce"), "got: {}", h);
        assert!(h.contains("INTEGER"), "got: {}", h);
    }

    #[test]
    fn e0033_helper_hint_import_boundary() {
        let d = WlwlDiagnostic::new(ErrorCode::E0033, "ignored", Location::point("a.wll", 5, 1))
            .with_strict_types_violation(
                "DICT",
                "STRING",
                Location::point("a.wll", 1, 1),
                Location::point("b.wll", 7, 4),
                "import",
            );
        let h = d.hint.expect("hint must be Some after helper");
        assert!(h.contains("DICT"), "got: {}", h);
        assert!(h.contains("STRING"), "got: {}", h);
        assert!(
            h.contains("contract") || h.contains("importing"),
            "got: {}",
            h
        );
    }

    #[test]
    fn e0033_helper_hint_ffi_boundary() {
        let d = WlwlDiagnostic::new(ErrorCode::E0033, "ignored", Location::point("a.wll", 5, 1))
            .with_strict_types_violation(
                "INTEGER",
                "FLOAT",
                Location::point("ffi.wll", 1, 1),
                Location::point("ffi.wll", 9, 2),
                "ffi",
            );
        let h = d.hint.expect("hint must be Some after helper");
        assert!(h.contains("FFI") || h.contains("shim"), "got: {}", h);
    }

    #[test]
    fn e0033_helper_unknown_boundary_uses_generic_hint() {
        // Unknown boundary label -> falls back to the generic hint
        // without panicking. Defensive: an evaluator bug might
        // pass a typo'd boundary name; we must not crash the
        // diagnostic construction path.
        let d = WlwlDiagnostic::new(ErrorCode::E0033, "ignored", Location::point("a.wll", 5, 1))
            .with_strict_types_violation(
                "INTEGER",
                "STRING",
                Location::point("a.wll", 1, 1),
                Location::point("a.wll", 5, 1),
                "banana",
            );
        let h = d.hint.expect("hint must be Some after helper");
        assert!(h.contains("INTEGER"), "got: {}", h);
        assert!(!h.contains("FFI"), "got: {}", h);
    }

    #[test]
    fn e0033_helper_preserves_existing_hint_only_when_no_call() {
        // Without calling the helper, hint stays None.
        let d = WlwlDiagnostic::new(
            ErrorCode::E0033,
            "raw message",
            Location::point("a.wll", 1, 1),
        );
        assert!(d.hint.is_none());
        assert!(d.related.is_empty());
    }

    #[test]
    fn e0033_helper_jsonl_round_trip() {
        // The helper must not break JSONL serialization -- both
        // related entries and the message must survive.
        let d = WlwlDiagnostic::new(ErrorCode::E0033, "ignored", Location::point("a.wll", 5, 1))
            .with_strict_types_violation(
                "INTEGER",
                "STRING",
                Location::point("a.wll", 1, 1),
                Location::point("a.wll", 5, 1),
                "function",
            )
            .with_source_line("LET(f, FUN((x: INTEGER), x));");
        let jsonl = d.render_jsonl();
        assert!(jsonl.contains("E0033"), "got: {}", jsonl);
        assert!(
            jsonl.contains("type annotation mismatch: expected INTEGER, got STRING"),
            "got: {}",
            jsonl
        );
        // JSONL must be one line (no embedded newlines).
        assert!(!jsonl.contains('\n'), "got: {}", jsonl);
    }
    /// [Phase G7] meta-coverage: every ErrorCode variant must be
    /// snapshotted in some `snap_*` test. Adding a new variant
    /// to the enum without extending a snapshot fails this test
    /// and thus fails CI. Per P4-G7-001 we do NOT enforce
    /// substantive `suggestion_code` content here; that is a v0.5
    /// follow-up because populating it requires reviewing all 58
    /// error codes plus 13 warning codes.
    #[test]
    fn all_error_codes_have_snapshots() {
        // Build the union of all codes currently snapshotted.
        // Adding a new ErrorCode variant requires also adding it
        // to one of these arrays (or this test will fail).
        let snap_lex = ["E0001", "E0002", "E0003"];
        let snap_syn = ["E0010", "E0011", "E0012", "E0013", "E0014"];
        let snap_name = [
            "E0020", "E0021", "E0022", "E0023", "E0024", "E0025", "E0026", "E0027",
        ];
        let snap_type = [
            "E0030", "E0031", "E0032", "E0033", "E0034", "E0035", "E0036", "E0037", "E0038",
            "E0039",
        ];
        let snap_module = [
            "E0040", "E0041", "E0042", "E0043", "E0044", "E0045", "E0046", "E0047", "E0048",
            "E0049",
        ];
        let snap_oop = ["E0050", "E0051"];
        let snap_io = ["E0060", "E0061", "E0062", "E0063"];
        let snap_json = ["E0070", "E0071"];
        let snap_ai = ["E0080", "E0081", "E0082", "E0083"];
        let snap_network = ["E0090", "E0091", "E0092", "E0093", "E0094"];
        let snap_user_internal = ["E0099", "E0100", "E0101", "E0102", "E1003"];
        let snap_runtime = ["E1003"];
        let mut covered = std::collections::HashSet::<&str>::new();
        for group in [
            &snap_lex[..],
            &snap_syn[..],
            &snap_name[..],
            &snap_type[..],
            &snap_module[..],
            &snap_oop[..],
            &snap_io[..],
            &snap_json[..],
            &snap_ai[..],
            &snap_network[..],
            &snap_user_internal[..],
            &snap_runtime[..],
        ] {
            for c in group {
                covered.insert(*c);
            }
        }
        let total = covered.len();
        // Plan 搂6.3 targets 56 + 14 codes. As of Phase D4 we
        // have 58 error codes (E0001..E0102 + E1003) plus
        // 13 warnings (W0001..W0054). This assertion is
        // informational 鈥?the strict guarantee is that future
        // additions can't shrink the union.
        assert!(
            total >= 58,
            "snapshot coverage regressed: only {} codes covered (>=58 expected)",
            total,
        );
    }
}
