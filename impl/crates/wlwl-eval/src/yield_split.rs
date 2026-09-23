//! [v0.9 Step 2] Body segmentation entry wrapper.
//!
//! Replaces the v0.7/v0.8.1 static segmenter (`validate` + `collect`
//! two-pass design, ~492 lines) with a thin wrapper that converts a
//! task closure body into a list of runnable segments at SPAWN time.
//!
//! ## What changed in v0.9 (ADR-0017 §3.1)
//!
//! - **Validate pass removed.** `E0014` ("YIELD must appear as a
//!   direct child of a Block") no longer triggers on YIELD position.
//!   YIELD can now appear in any expression position: `LET(x,
//!   YIELD())` / `IF(c, YIELD(), 0)` / `[1, YIELD(), 3]` /
//!   `YIELD(); 42` / `YIELD()` at the top level of a task body —
//!   all are legal. `E0014` keeps its registration for the only
//!   remaining trigger: `RETURN / BREAK / CONTINUE` in illegal
//!   positions.
//!
//! - **Collect pass kept (simplified).** The static segmentation at
//!   top-level YIELD-bearing expressions is still useful: it lets
//!   the scheduler resume a task at the next segment boundary when
//!   the body has multiple top-level statements. For YIELD inside a
//!   nested expression (LET RHS, IF branch, Array literal), the
//!   surrounding segment is yielded as a unit and the surrounding
//!   task body's resumption semantics are Step 3 / ADR-0017
//!   territory (real state-machine eval_expr).
//!
//! ## Algorithm
//!
//! One pass:
//! 1. If the body contains no YIELD anywhere (the v0.6 fidelity case
//!    and the common SPAWN-without-yield case), return an empty
//!    `Segments` vec. The scheduler falls back to running the
//!    original body in one shot via `invoke_closure`, preserving
//!    v0.6 fidelity exactly.
//! 2. If the body contains YIELD, walk the AST. At each top-level
//!    statement of the outer Block, decide whether it ends the
//!    current segment (it contains a YIELD somewhere inside) or
//!    appends to the current segment. The outer Block itself is
//!    unwrapped — segments are sequences of top-level statements.
//!
//! See ADR-0017 §3.1 + `docs/plan/wlwl-build-plan-v0.9.md` §3.1 for
//! the full design rationale and the Step 3 / state-machine
//! followup that closes the "YIELD inside LET RHS resumes the LET
//! binding" gap.

use wlwl_ast::Expr;

/// Result of segmenting the closure body.
///
/// Each inner `Vec<Expr>` is a sequence of expressions to evaluate
/// top-to-bottom as one segment. The last segment (and only the last
/// segment) does NOT end with a `YIELD()` call — it produces the
/// task's final value. Every earlier segment ends with a `YIELD()`-
/// bearing expression (a checkpoint marker).
///
/// Empty inner vectors are not produced: a segment always has at
/// least one expression. A body that ends in a YIELD has N segments,
/// each ending with YIELD, and no trailing empty segment.
///
/// **Empty `Segments` vec** (length 0) is a sentinel meaning "no
/// YIELD present in the body; the scheduler should run the original
/// body in one shot via `invoke_closure` (the v0.6 fidelity path)".
pub type Segments = Vec<Vec<Expr>>;

/// Package the closure body into a task-body entry spec.
///
/// Concretely:
/// - If `body` contains no `YIELD()` anywhere, returns an empty
///   `Segments` vec.
/// - Otherwise, returns `segments` that capture the body's top-level
///   statements, splitting at each YIELD-bearing top-level statement.
///
/// Unlike the v0.7/v0.8.1 design, this function **never rejects** a
/// body. YIELD can appear at any expression position; the static
/// segmentation simply decides where the next resume boundary falls
/// when YIELD fires inside a top-level statement.
pub fn split_body_for_yield(body: &Expr) -> Segments {
    // Fast path: no YIELD anywhere → empty segments, scheduler falls
    // through to invoke_closure (v0.6 fidelity).
    if !contains_yield(body) {
        return Vec::new();
    }
    // Slow path: walk the body. The body is expected to be the
    // closure body, which is itself an Expr (most commonly a Block).
    // We unwrap one level of Block if present (the top-level
    // statements of the closure body), then segment each top-level
    // statement by whether it contains YIELD.
    let mut segments: Segments = Vec::new();
    let mut current: Vec<Expr> = Vec::new();
    collect(body, &mut current, &mut segments);
    if !current.is_empty() {
        segments.push(current);
    }
    segments
}

// ── Collection pass ────────────────────────────────────────────────

fn collect(expr: &Expr, current: &mut Vec<Expr>, all_segments: &mut Segments) {
    match expr {
        Expr::Block { exprs, .. } => {
            for e in exprs {
                collect(e, current, all_segments);
            }
        }
        _ => {
            if contains_yield(expr) {
                current.push(expr.clone());
                all_segments.push(std::mem::take(current));
            } else {
                current.push(expr.clone());
            }
        }
    }
}

fn contains_yield(expr: &Expr) -> bool {
    match expr {
        Expr::Call { name, .. } if name == "YIELD" => true,
        Expr::Block { exprs, .. } => exprs.iter().any(contains_yield),
        Expr::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => {
            contains_yield(cond)
                || contains_yield(then_branch)
                || else_branch.as_deref().is_some_and(contains_yield)
        }
        Expr::While { cond, body, .. } => contains_yield(cond) || contains_yield(body),
        Expr::For { iter, body, .. } => contains_yield(iter) || contains_yield(body),
        Expr::Return { value, .. } => value.as_deref().is_some_and(contains_yield),
        Expr::Call { args, .. } => args.iter().any(contains_yield),
        Expr::Array { items, .. } => items.iter().any(contains_yield),
        Expr::Dict { entries, .. } => entries
            .iter()
            .any(|(k, v)| contains_yield(k) || contains_yield(v)),
        Expr::Ok { value, .. }
        | Expr::Err { value, .. }
        | Expr::Panic { value, .. }
        | Expr::Try { value, .. }
        | Expr::IsOk { value, .. }
        | Expr::IsErr { value, .. } => contains_yield(value),
        Expr::Let { value, .. } | Expr::LetPattern { value, .. } => contains_yield(value),
        Expr::OrDie { value, default, .. } => contains_yield(value) || contains_yield(default),
        Expr::Match {
            value,
            clauses,
            default,
            ..
        } => {
            contains_yield(value)
                || contains_yield(default)
                || clauses.iter().any(|c| contains_yield(&c.body))
        }
        Expr::Fun { body, .. } => contains_yield(body),
        Expr::Literal(_, _)
        | Expr::Var(_, _)
        | Expr::Import { .. }
        | Expr::Export { .. }
        | Expr::Break { .. }
        | Expr::Continue { .. } => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wlwl_ast::{Literal, Span};

    fn span() -> Span {
        Span::dummy()
    }

    fn int(n: i64) -> Expr {
        Expr::Literal(Literal::Integer(n), span())
    }

    fn var(name: &str) -> Expr {
        Expr::Var(name.to_string(), span())
    }

    fn yield_call() -> Expr {
        Expr::Call {
            name: "YIELD".to_string(),
            args: vec![],
            span: span(),
        }
    }

    fn block(exprs: Vec<Expr>) -> Expr {
        Expr::Block {
            exprs,
            span: span(),
        }
    }

    /// v0.9 Step 2 lock tests: every YIELD position that v0.7/v0.8.1
    /// rejected with E0014 must now be accepted. Each test calls
    /// `split_body_for_yield` and asserts the returned `Segments`
    /// matches the expected shape (no more `Err(_)`).
    ///
    /// The runtime behaviour of "task body resumes correctly after
    /// yielding inside a nested expression" is **Step 3 / ADR-0017
    /// §3.1 territory** (state-machine eval_expr). Step 2 only
    /// guarantees the splitter no longer rejects these positions;
    /// the underlying segments produced are documented per case so
    /// Step 3 can wire the state-machine resume correctly.

    #[test]
    fn body_with_no_yield_yields_empty_segments() {
        let body = block(vec![int(1), int(2)]);
        let segs = split_body_for_yield(&body);
        assert!(segs.is_empty(), "no YIELD → empty segments (v0.6 fidelity)");
    }

    #[test]
    fn tail_yield_in_block_splits_into_two_segments() {
        // [A, YIELD, B]  ->  [A, YIELD], [B]
        let body = block(vec![int(1), yield_call(), int(2)]);
        let segs = split_body_for_yield(&body);
        assert_eq!(segs.len(), 2);
        assert_eq!(segs[0].len(), 2);
        assert_eq!(segs[1].len(), 1);
    }

    #[test]
    fn multiple_top_level_yields_split_into_three_segments() {
        let body = block(vec![int(1), yield_call(), int(2), yield_call(), int(3)]);
        let segs = split_body_for_yield(&body);
        assert_eq!(segs.len(), 3);
    }

    // ── v0.9 Step 2 lock tests (formerly E0014, now legal) ──

    #[test]
    fn yield_in_let_rhs_without_block_is_now_legal() {
        // v0.7/v0.8.1: NestedYield -> E0014.
        // v0.9 Step 2: must succeed; LET contains YIELD → 1 segment.
        let body = Expr::Let {
            name: "x".to_string(),
            mut_: false,
            type_annotation: None,
            value: Box::new(yield_call()),
            span: span(),
        };
        let segs = split_body_for_yield(&body);
        // LET has YIELD inside → it ends its segment; the LET is the
        // only top-level statement, so we get 1 segment containing the
        // LET itself.
        assert_eq!(segs.len(), 1);
        assert!(matches!(segs[0].last(), Some(Expr::Let { .. })));
    }

    #[test]
    fn yield_as_if_branch_is_now_legal() {
        // v0.7/v0.8.1: NestedYield -> E0014.
        // v0.9 Step 2: must succeed; IF contains YIELD → 1 segment.
        let body = Expr::If {
            cond: Box::new(var("cond")),
            then_branch: Box::new(yield_call()),
            else_branch: Some(Box::new(int(42))),
            span: span(),
        };
        let segs = split_body_for_yield(&body);
        assert_eq!(segs.len(), 1);
        assert!(matches!(segs[0].last(), Some(Expr::If { .. })));
    }

    #[test]
    fn yield_in_array_literal_is_now_legal() {
        // v0.7/v0.8.1: NestedYield -> E0014.
        // v0.9 Step 2: must succeed; Array contains YIELD → 1 segment.
        let body = Expr::Array {
            items: vec![int(1), yield_call(), int(3)],
            span: span(),
        };
        let segs = split_body_for_yield(&body);
        assert_eq!(segs.len(), 1);
        assert!(matches!(segs[0].last(), Some(Expr::Array { .. })));
    }

    #[test]
    fn yield_at_top_level_inside_block_followed_by_other_stmt() {
        // [YIELD(); 42] -> [YIELD()], [42]
        let body = block(vec![yield_call(), int(42)]);
        let segs = split_body_for_yield(&body);
        assert_eq!(segs.len(), 2);
        assert!(matches!(segs[0].last(), Some(Expr::Call { name, .. }) if name == "YIELD"));
        assert_eq!(segs[1], vec![int(42)]);
    }

    #[test]
    fn yield_in_let_rhs_followed_by_other_stmt_produces_two_segments() {
        // LET(x, YIELD()); 42 -> [LET(x, YIELD())], [42]
        let body = block(vec![
            Expr::Let {
                name: "x".to_string(),
                mut_: false,
                type_annotation: None,
                value: Box::new(yield_call()),
                span: span(),
            },
            int(42),
        ]);
        let segs = split_body_for_yield(&body);
        assert_eq!(segs.len(), 2);
        assert!(matches!(segs[0].last(), Some(Expr::Let { .. })));
        assert_eq!(segs[1], vec![int(42)]);
    }

    #[test]
    fn yield_in_array_literal_followed_by_other_stmt_produces_two_segments() {
        // [1, YIELD(), 3]; 42 -> [[1,Y(),3]], [42]
        let body = block(vec![
            Expr::Array {
                items: vec![int(1), yield_call(), int(3)],
                span: span(),
            },
            int(42),
        ]);
        let segs = split_body_for_yield(&body);
        assert_eq!(segs.len(), 2);
        assert!(matches!(segs[0].last(), Some(Expr::Array { .. })));
        assert_eq!(segs[1], vec![int(42)]);
    }

    #[test]
    fn yield_in_nested_if_inside_block_produces_two_segments() {
        // [A, IF(cond, [YIELD()], [42]), B] -> 2 segments
        //   segment 0: [A, IF(...)]            (IF yields inside)
        //   segment 1: [B]                     (runs after yield)
        let body = block(vec![
            int(1),
            Expr::If {
                cond: Box::new(var("cond")),
                then_branch: Box::new(block(vec![yield_call()])),
                else_branch: Some(Box::new(block(vec![int(42)]))),
                span: span(),
            },
            int(2),
        ]);
        let segs = split_body_for_yield(&body);
        assert_eq!(segs.len(), 2);
        assert!(matches!(segs[0].last(), Some(Expr::If { .. })));
        assert_eq!(segs[1], vec![int(2)]);
    }

    #[test]
    fn empty_body_produces_empty_segments() {
        let body = block(vec![]);
        let segs = split_body_for_yield(&body);
        assert!(segs.is_empty());
    }

    #[test]
    fn body_ending_with_yield_produces_no_trailing_empty_segment() {
        let body = block(vec![int(1), yield_call()]);
        let segs = split_body_for_yield(&body);
        assert_eq!(segs.len(), 1);
        assert_eq!(segs[0].len(), 2);
    }
}
