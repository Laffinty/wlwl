//! `wlwl:std.format` — `FORMAT` (v0.4 §10.6 + §15.8, error code E0039).
//!
//! This module owns the **template grammar** for `FORMAT` (see
//! [`parse_template`]); both FORMAT entry points share it:
//!
//! - the global builtin in `wlwl-eval` (`resolve_builtin("FORMAT")`,
//!   per spec appendix G `FORMAT` is a global builtin, not a macro),
//!   which renders on the evaluator's `Value` type;
//! - this module's [`std_format`], the `StdFn` bound by
//!   `IMPORT("wlwl:std.format", ["FORMAT"])`, which renders on
//!   `StdValue` (`serde_json::Value`) at the std boundary.
//!
//! Rendering therefore exists twice (once per value world) but the
//! grammar exists once. The std-side render cannot see closures /
//! native fns — the eval→std boundary rejects them with E0030 before
//! dispatch (existing contract for every std module), while the global
//! builtin renders them via `Value::display()` (`<fun(...)>`), matching
//! `STR` semantics (§10.3). One more cosmetic difference: this side
//! renders dict keys in `serde_json::Map` order (BTreeMap unless
//! `preserve_order` is enabled), the eval side in insertion order.

use crate::{type_error, StdCtx, StdError, StdFn, StdValue, ModuleSpec};
use wlwl_error::ErrorCode;

/// One parsed piece of a FORMAT template (v0.4 §10.6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormatSegment {
    /// Literal text outside `{...}` (a stray `}` is literal too).
    Literal(String),
    /// `{012}` — all-ASCII-digit placeholder; indexes the format args
    /// (the arguments **after** the template).
    Positional(usize),
    /// `{name}` — non-digit, non-empty placeholder; looked up by key in
    /// the first DICT among the format args (in the pure-named pattern
    /// that dict is `args[0]`, exactly as §10.6 specifies).
    Named(String),
}

/// Parse a FORMAT template into segments (v0.4 §10.6).
///
/// Grammar:
/// - `{N}` / `{name}` placeholders: all-ASCII-digits → positional,
///   anything else non-empty → named;
/// - a stray `}` (no open `{`) is a literal character;
/// - **E0039** (`malformed format template`) on a `{` that never
///   closes before end-of-template, and on an empty placeholder `{}`.
///
/// Note: "unmatched" is **not** a parse failure — whether a positional
/// index is in range or a named key exists is a *render-time* concern
/// decided by the args, so an out-of-range `{5}` or missing `{name}`
/// renders as the original text (§10.6 末段: 未匹配的 `{...}` 保留原样).
/// The single parse-time exception is an all-digit index that
/// overflows `usize`: it can never match any arg, so it is folded into
/// a `Literal` segment here rather than erroring.
pub fn parse_template(template: &str) -> Result<Vec<FormatSegment>, StdError> {
    let chars: Vec<(usize, char)> = template.char_indices().collect();
    let mut segments: Vec<FormatSegment> = Vec::new();
    let mut lit = String::new();
    let mut i = 0;
    while i < chars.len() {
        let (open_at, c) = chars[i];
        if c != '{' {
            lit.push(c);
            i += 1;
            continue;
        }
        // Find the closing `}`; E0039 if we hit end-of-template first.
        let mut j = i + 1;
        while j < chars.len() && chars[j].1 != '}' {
            j += 1;
        }
        if j >= chars.len() {
            return Err(StdError {
                code: ErrorCode::E0039,
                message: format!(
                    "FORMAT: malformed format template: unclosed '{{' at offset {}",
                    open_at
                ),
            });
        }
        let content: String = chars[i + 1..j].iter().map(|(_, c)| *c).collect();
        if content.is_empty() {
            return Err(StdError {
                code: ErrorCode::E0039,
                message: format!(
                    "FORMAT: malformed format template: empty placeholder '{{}}' at offset {}",
                    open_at
                ),
            });
        }
        if !lit.is_empty() {
            segments.push(FormatSegment::Literal(std::mem::take(&mut lit)));
        }
        if content.chars().all(|c| c.is_ascii_digit()) {
            match content.parse::<usize>() {
                Ok(n) => segments.push(FormatSegment::Positional(n)),
                // All digits but too large for usize: can never match an
                // arg — keep the original text (unmatched, not malformed).
                Err(_) => segments.push(FormatSegment::Literal(format!("{{{}}}", content))),
            }
        } else {
            segments.push(FormatSegment::Named(content));
        }
        i = j + 1;
    }
    if !lit.is_empty() {
        segments.push(FormatSegment::Literal(lit));
    }
    Ok(segments)
}

/// Render a `StdValue` for insertion into a FORMAT result. Mirrors the
/// evaluator's `Value::display()` (which is `STR` semantics, §10.3):
/// STRING as-is, INTEGER `42`, FLOAT `30.0` / `3.14`, BOOLEAN
/// `TRUE`/`FALSE`, NULL `NULL`, ARRAY / DICT structurally.
fn std_display(v: &StdValue) -> String {
    match v {
        StdValue::Null => "NULL".into(),
        StdValue::Bool(b) => if *b { "TRUE" } else { "FALSE" }.into(),
        StdValue::String(s) => s.clone(),
        StdValue::Number(n) => {
            if let Some(i) = n.as_i64() {
                i.to_string()
            } else if let Some(f) = n.as_f64() {
                if f.fract() == 0.0 {
                    format!("{:.1}", f)
                } else {
                    f.to_string()
                }
            } else {
                // serde_json guards against NaN/Inf at construction;
                // this arm is unreachable in practice.
                n.to_string()
            }
        }
        StdValue::Array(items) => format!(
            "[{}]",
            items
                .iter()
                .map(std_display)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        StdValue::Object(m) => format!(
            "[{}]",
            m.iter()
                .map(|(k, v)| format!("{}: {}", k, std_display(v)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

/// `FORMAT(template, args...) → STRING` — the `wlwl:std.format` module
/// entry (v0.4 §10.6 + §15.8).
///
/// - `args[0]` must be a STRING (the template), else E0030;
/// - `{N}` inserts the N-th format arg (args after the template),
///   rendered via STR semantics; out of range → the placeholder text
///   is kept as-is;
/// - `{name}` looks up the first DICT among the format args; missing
///   key / no DICT arg → kept as-is;
/// - template parse failure → E0039.
pub fn std_format(_ctx: &mut StdCtx, args: Vec<StdValue>) -> Result<StdValue, StdError> {
    if args.is_empty() {
        return Err(StdError {
            code: ErrorCode::E0022,
            message: "FORMAT: function expects at least 1 argument (template), got 0".into(),
        });
    }
    let template = match &args[0] {
        StdValue::String(s) => s.as_str(),
        other => return Err(type_error("FORMAT", "string (template) as first arg", other)),
    };
    let segments = parse_template(template)?;
    let format_args = &args[1..];
    // Named-lookup source: the first DICT among the format args. In the
    // pure-named pattern this is args[0] exactly as §10.6 specifies;
    // scanning (instead of hardcoding args[0]) is what makes the spec's
    // own mixed example work: FORMAT("hi {0}, age {age}", "alice", ["age": 30]).
    let named = format_args.iter().find_map(|a| match a {
        StdValue::Object(m) => Some(m),
        _ => None,
    });
    let mut out = String::new();
    for seg in &segments {
        match seg {
            FormatSegment::Literal(s) => out.push_str(s),
            FormatSegment::Positional(n) => match format_args.get(*n) {
                Some(v) => out.push_str(&std_display(v)),
                None => {
                    out.push('{');
                    out.push_str(&n.to_string());
                    out.push('}');
                }
            },
            FormatSegment::Named(name) => {
                let hit = named.and_then(|m| m.get(name.as_str()));
                match hit {
                    Some(v) => out.push_str(&std_display(v)),
                    None => {
                        out.push('{');
                        out.push_str(name);
                        out.push('}');
                    }
                }
            }
        }
    }
    Ok(StdValue::String(out))
}

pub static SPEC: ModuleSpec = ModuleSpec {
    path: "wlwl:std.format",
    functions: &[("FORMAT", std_format as StdFn)],
};

#[cfg(test)]
mod tests {
    use super::*;

    fn segs(t: &str) -> Vec<FormatSegment> {
        parse_template(t).expect("template parses")
    }

    fn fmt(args: Vec<StdValue>) -> Result<String, StdError> {
        let mut ctx = StdCtx::default();
        std_format(&mut ctx, args).map(|v| match v {
            StdValue::String(s) => s,
            other => panic!("FORMAT must return a string, got {:?}", other),
        })
    }

    fn s(v: &str) -> StdValue {
        StdValue::String(v.into())
    }

    // ── parse_template ─────────────────────────────────────────────

    #[test]
    fn parse_empty_template_is_ok_empty() {
        assert_eq!(segs(""), vec![]);
    }

    #[test]
    fn parse_plain_text_is_single_literal() {
        assert_eq!(segs("hello world"), vec![FormatSegment::Literal("hello world".into())]);
    }

    #[test]
    fn parse_positional_placeholders() {
        assert_eq!(
            segs("hi {0}, you are {1}"),
            vec![
                FormatSegment::Literal("hi ".into()),
                FormatSegment::Positional(0),
                FormatSegment::Literal(", you are ".into()),
                FormatSegment::Positional(1),
            ]
        );
    }

    #[test]
    fn parse_named_placeholder() {
        assert_eq!(
            segs("{name}!"),
            vec![FormatSegment::Named("name".into()), FormatSegment::Literal("!".into())]
        );
    }

    #[test]
    fn parse_mixed_placeholders() {
        assert_eq!(
            segs("{0}-{name}"),
            vec![FormatSegment::Positional(0), FormatSegment::Literal("-".into()), FormatSegment::Named("name".into())]
        );
    }

    #[test]
    fn parse_leading_zero_index_is_positional() {
        assert_eq!(segs("{00}"), vec![FormatSegment::Positional(0)]);
        assert_eq!(segs("{01}"), vec![FormatSegment::Positional(1)]);
    }

    #[test]
    fn parse_non_ascii_digit_content_is_named() {
        // Only ASCII digits are positional; a unicode digit is a name.
        assert_eq!(segs("{٣}"), vec![FormatSegment::Named("٣".into())]);
    }

    #[test]
    fn parse_stray_close_brace_is_literal() {
        assert_eq!(segs("a } b"), vec![FormatSegment::Literal("a } b".into())]);
    }

    #[test]
    fn parse_unclosed_brace_is_e0039() {
        let err = parse_template("hi {0").unwrap_err();
        assert_eq!(err.code, ErrorCode::E0039);
        assert!(err.message.contains("unclosed"), "{}", err.message);
    }

    #[test]
    fn parse_lone_open_brace_is_e0039() {
        // The spec §10.6 example of a parse failure: "`{` 单独出现".
        let err = parse_template("{").unwrap_err();
        assert_eq!(err.code, ErrorCode::E0039);
    }

    #[test]
    fn parse_empty_placeholder_is_e0039() {
        let err = parse_template("a {} b").unwrap_err();
        assert_eq!(err.code, ErrorCode::E0039);
        assert!(err.message.contains("empty"), "{}", err.message);
    }

    #[test]
    fn parse_overflow_index_folds_to_literal_not_error() {
        assert_eq!(
            segs("{99999999999999999999999}"),
            vec![FormatSegment::Literal("{99999999999999999999999}".into())]
        );
    }

    // ── std_format rendering ───────────────────────────────────────

    #[test]
    fn format_positional_spec_example() {
        // Spec §10.6 example 1.
        let out = fmt(vec![s("hi {0}, you are {1} years old"), s("alice"), StdValue::from(30)])
            .unwrap();
        assert_eq!(out, "hi alice, you are 30 years old");
    }

    #[test]
    fn format_named_spec_example() {
        // Spec §10.6 example 2 (pure-named pattern: the dict IS args[0]
        // of the format args).
        let out = fmt(vec![
            s("hi {name}, age {age}"),
            serde_json::json!({"name": "alice", "age": 30}),
        ])
        .unwrap();
        assert_eq!(out, "hi alice, age 30");
    }

    #[test]
    fn format_mixed_spec_example() {
        // Spec §10.6 example 3 — the mixed pattern that motivates the
        // "first DICT among args" lookup rule.
        let out = fmt(vec![
            s("hi {0}, age {age}"),
            s("alice"),
            serde_json::json!({"age": 30}),
        ])
        .unwrap();
        assert_eq!(out, "hi alice, age 30");
    }

    #[test]
    fn format_repeated_placeholder() {
        let out = fmt(vec![s("{0} and {0} and {0}"), s("x")]).unwrap();
        assert_eq!(out, "x and x and x");
    }

    #[test]
    fn format_conversions_via_str_semantics() {
        assert_eq!(fmt(vec![s("{0}"), StdValue::from(42)]).unwrap(), "42");
        assert_eq!(fmt(vec![s("{0}"), StdValue::from(30.0)]).unwrap(), "30.0");
        assert_eq!(fmt(vec![s("{0}"), StdValue::from(3.14)]).unwrap(), "3.14");
        assert_eq!(fmt(vec![s("{0}"), StdValue::Bool(true)]).unwrap(), "TRUE");
        assert_eq!(fmt(vec![s("{0}"), StdValue::Bool(false)]).unwrap(), "FALSE");
        assert_eq!(fmt(vec![s("{0}"), StdValue::Null]).unwrap(), "NULL");
        assert_eq!(
            fmt(vec![s("{0}"), serde_json::json!([1, 2, 3])]).unwrap(),
            "[1, 2, 3]"
        );
        assert_eq!(
            fmt(vec![s("{0}"), serde_json::json!({"a": 1})]).unwrap(),
            "[a: 1]"
        );
    }

    #[test]
    fn format_unmatched_positional_kept_literal() {
        let out = fmt(vec![s("{0} + {5}"), s("a")]).unwrap();
        assert_eq!(out, "a + {5}");
    }

    #[test]
    fn format_unmatched_named_key_kept_literal() {
        let out = fmt(vec![s("{name}"), serde_json::json!({"other": 1})]).unwrap();
        assert_eq!(out, "{name}");
    }

    #[test]
    fn format_named_without_dict_kept_literal() {
        let out = fmt(vec![s("hi {name}"), s("alice")]).unwrap();
        assert_eq!(out, "hi {name}");
    }

    #[test]
    fn format_named_skips_non_dict_args() {
        // First DICT among the format args wins; scalars are skipped.
        let out = fmt(vec![
            s("{name}"),
            StdValue::from(1),
            serde_json::json!({"name": "n"}),
        ])
        .unwrap();
        assert_eq!(out, "n");
    }

    #[test]
    fn format_positional_refers_to_dict_arg_renders_it() {
        // {0} pointing at the dict itself renders it structurally.
        let out = fmt(vec![s("{0}"), serde_json::json!({"a": 1})]).unwrap();
        assert_eq!(out, "[a: 1]");
    }

    #[test]
    fn format_template_not_string_is_e0030() {
        let err = fmt(vec![StdValue::from(42)]).unwrap_err();
        assert_eq!(err.code, ErrorCode::E0030);
    }

    #[test]
    fn format_zero_args_is_e0022() {
        let mut ctx = StdCtx::default();
        let err = std_format(&mut ctx, vec![]).unwrap_err();
        assert_eq!(err.code, ErrorCode::E0022);
        assert!(err.message.contains("at least 1"), "{}", err.message);
    }

    #[test]
    fn format_malformed_template_is_e0039() {
        let err = fmt(vec![s("bad { template")]).unwrap_err();
        assert_eq!(err.code, ErrorCode::E0039);
    }

    #[test]
    fn format_empty_template_and_no_args() {
        assert_eq!(fmt(vec![s("")]).unwrap(), "");
    }

    #[test]
    fn spec_lists_format_only() {
        assert_eq!(SPEC.path, "wlwl:std.format");
        let names: Vec<&str> = SPEC.functions.iter().map(|(n, _)| *n).collect();
        assert_eq!(names, vec!["FORMAT"]);
    }
}
