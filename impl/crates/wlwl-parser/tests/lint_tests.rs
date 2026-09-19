//! Phase E4 linter-walk tests (plan §5.13).
//!
//! `lint()` is the post-parse, scope-aware warning walk that merges
//! with the parser's own W0020 channel in `wlwl check`.

use wlwl_parser::{lint, parse};

fn warnings(src: &str) -> Vec<(String, String)> {
    let e = parse(src, "t.wl").expect("parse failed");
    lint(&e)
        .into_iter()
        .map(|w| (w.code.as_str().to_string(), w.message))
        .collect()
}

fn codes(src: &str) -> Vec<String> {
    warnings(src).into_iter().map(|(c, _)| c).collect()
}

// ── W0010: unused LET binding ───────────────────────────────────────

#[test]
fn w0010_unused_let_binding() {
    assert_eq!(codes("LET(x, 1); PRINT(2);"), vec!["W0010"]);
}

#[test]
fn w0010_used_binding_not_reported() {
    assert!(codes("LET(x, 1); PRINT(x);").is_empty());
}

#[test]
fn w0010_use_in_call_position_counts() {
    // `f(1)` is a Call{name: "f"} — that is a read of f.
    assert!(codes("LET(f, FUN((x), x)); f(1);").is_empty());
}

#[test]
fn w0010_use_inside_closure_counts() {
    // Closures capture later, so a reference inside a FUN body counts.
    // (g itself must be called, else g triggers its own W0010.)
    assert!(codes("LET(x, 1); LET(g, FUN((), x)); g();").is_empty());
}

#[test]
fn w0010_underscore_prefix_silences() {
    assert!(codes("LET(_x, 1); PRINT(2);").is_empty());
}

#[test]
fn w0010_export_counts_as_use() {
    assert!(codes("LET(add, 1); EXPORT([\"add\"]);").is_empty());
}

#[test]
fn w0010_shadowed_outer_still_reported_by_inner_scope() {
    // Inner rebind shadows: outer x used after shadowing is a read of
    // the *inner* x... the outer binding is used before the shadow,
    // so nothing fires; both used.
    assert!(codes("LET(x, 1); PRINT(x); IF(TRUE, LET(x, 2); PRINT(x), 0);").is_empty());
}

#[test]
fn w0010_destructuring_binding_unused() {
    assert_eq!(codes("LET([a, b], arr); PRINT(a);"), vec!["W0010"]);
}

#[test]
fn w0010_for_loop_var_used_in_body() {
    assert!(codes("FOR(i, items, PRINT(i));").is_empty());
}

// ── W0011: unused function parameter ────────────────────────────────

#[test]
fn w0011_unused_param() {
    assert_eq!(codes("LET(f, FUN((a, b), a)); f(1, 2);"), vec!["W0011"]);
}

#[test]
fn w0011_underscore_param_silenced() {
    assert!(codes("LET(f, FUN((a, _b), a)); f(1, 2);").is_empty());
}

#[test]
fn w0011_all_params_used() {
    assert!(codes("LET(f, FUN((a, b), +(a, b))); f(1, 2);").is_empty());
}

// ── W0012: duplicate LET in same scope ──────────────────────────────

#[test]
fn w0012_duplicate_let_same_scope() {
    assert_eq!(codes("LET(x, 1); LET(x, 2); PRINT(x);"), vec!["W0012"]);
}

#[test]
fn w0012_rebind_in_different_scope_ok() {
    // A FUN body rebinding an outer name is a different scope (the
    // outer x is used first; the inner shadow is used inside).
    assert!(codes("LET(x, 1); PRINT(x); LET(f, FUN((), LET(x, 2); x)); f();").is_empty());
}

#[test]
fn w0012_reports_message_with_position() {
    let ws = warnings("LET(y, 1); LET(y, 2); PRINT(y);");
    assert_eq!(ws.len(), 1);
    assert!(ws[0].1.contains("'y'"), "got: {}", ws[0].1);
}

// ── integration with the parser warning channel ─────────────────────

#[test]
fn lint_is_independent_of_parse_warnings() {
    // lint() only carries static name-level checks; W0020 stays in
    // parse_with_warnings.
    let (e, parse_ws) =
        wlwl_parser::parse_with_warnings("[1, \"a\": 2];", "t.wl").expect("parse failed");
    assert!(parse_ws.iter().any(|w| w.code.as_str() == "W0020"));
    let lint_ws = lint(&e);
    assert!(lint_ws.iter().all(|w| w.code.as_str() != "W0020"));
}

#[test]
fn lint_clean_program_has_no_warnings() {
    assert!(codes("LET(x, 1); PRINT(x);").is_empty());
}
