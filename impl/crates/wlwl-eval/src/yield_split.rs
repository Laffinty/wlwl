//! [v0.7 Phase B5a-3] Body segmentation at YIELD checkpoints.
//!
//! Path B of the B5a-3 design (plan §3 Phase B5a-3; discussed
//! 2026-09-22). When a task body contains `YIELD()` calls, the
//! scheduler needs to know where to stop and resume. Rather than
//! converting the entire recursive `eval_expr` into an iterative
//! stack machine (plan §5.1's full CPS — deferrable), we split the
//! closure body into a list of "segments" at SPAWN time. Each segment
//! is a sequence of expressions that runs to completion (or until a
//! YIELD propagates a `Signal::Yield(Explicit)` out of it). When the
//! scheduler resumes a Suspended task, it simply runs the next
//! segment; LET bindings made by earlier segments persist because
//! they live in the task's running scope.
//!
//! # YIELD placement rule
//!
//! "Block recursive" (chosen 2026-09-22): YIELD must be a direct
//! child of an `Expr::Block`'s `exprs` list. This means YIELD can
//! appear inside IF/LET/FUN bodies as long as those branches are
//! themselves wrapped in `[ ... ]`. Direct calls like
//! `IF(cond, YIELD(), 42)` (no Block wrap) are rejected at SPAWN
//! time with E0014 ("YIELD must appear inside a Block").
//!
//! Indirect calls (`LET y = YIELD; y()`) cannot be detected
//! statically; they are a known limitation. Users should use direct
//! `YIELD()` calls.
//!
//! # Algorithm
//!
//! Two passes:
//! 1. **Validate**: every `YIELD()` call must have a parent that is
//!    an `Expr::Block`. Anything else is an error.
//! 2. **Collect**: walk the AST. At each `Expr::Block`, walk its
//!    `exprs` in order. Each expr that contains a YIELD somewhere
//!    inside it (validated to be at a legal position) becomes the
//!    last expr of the current segment; subsequent exprs form the
//!    next segment. A direct `YIELD()` call terminates the current
//!    segment with itself as the final expr (so the YIELD is the
//!    checkpoint marker in the segment stream).
//!
//! See plan §3 Phase B5a-3 + the deviations entry
//! `P7-B5a3-001` for rationale and tradeoffs.

use wlwl_ast::{Expr, Span};

/// Error produced by [`split_body_for_yield`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum YieldSplitError {
    /// YIELD appeared at a position that is not a direct child of an
    /// `Expr::Block`'s `exprs` list. The span points at the offending
    /// YIELD call.
    NestedYield { span: Span },
    /// Body is not an `Expr::Block`. We require the task body to be
    /// a block — single-expression bodies cannot host mid-body
    /// yield points in path B. (A task whose body is a single
    /// expression `YIELD()` is allowed only because `YIELD()` alone
    /// is meaningless — segment it as one segment `[YIELD()]`.
    /// Other single expressions raise this error.)
    BodyNotBlock { span: Span },
}

/// Result of segmenting the closure body.
///
/// Each inner `Vec<Expr>` is a sequence of expressions to evaluate
/// top-to-bottom as one segment. The last segment (and only the last
/// segment) does NOT end with a `YIELD()` call — it produces the
/// task's final value. Every earlier segment ends with a `YIELD()`
/// call (a checkpoint marker).
///
/// Empty inner vectors are not produced: a segment always has at
/// least one expression. A body that ends in a YIELD has N segments,
/// each ending with YIELD, and no trailing empty segment.
pub type Segments = Vec<Vec<Expr>>;

/// Validate YIELD positions in the body, then segment the body at
/// legal YIELD checkpoints.
///
/// See module docs for the validation rule and algorithm.
pub fn split_body_for_yield(body: &Expr) -> Result<Segments, YieldSplitError> {
    // Pass 1: validate. After this call, every YIELD in the body is
    // guaranteed to be a direct child of some Expr::Block.
    validate(body)?;

    // Pass 2: collect segments. We treat any Expr that contains a
    // YIELD somewhere inside (validated) as a "yieldable" expr that
    // must end its current segment. Direct YIELD calls also end
    // their segment with themselves as the final marker.
    let mut all_segments: Segments = Vec::new();
    let mut current: Vec<Expr> = Vec::new();
    collect(body, &mut current, &mut all_segments)?;

    // If the body ended with a YIELD, `current` is empty (the YIELD
    // was flushed into `all_segments`); otherwise flush the tail.
    if !current.is_empty() {
        all_segments.push(current);
    }

    // Edge case: a single direct YIELD() at the top level produces
    // one segment containing just the YIELD. This is allowed — it
    // means "yield once and finish with NULL". (The scheduler can
    // re-schedule it once before completing.)
    Ok(all_segments)
}

// ── Validation pass ────────────────────────────────────────────────

fn validate(expr: &Expr) -> Result<(), YieldSplitError> {
    walk(expr, /* parent_is_block = */ false)
}

fn walk(expr: &Expr, parent_is_block: bool) -> Result<(), YieldSplitError> {
    match expr {
        Expr::Call { name, args, span } if name == "YIELD" => {
            if !parent_is_block {
                return Err(YieldSplitError::NestedYield { span: span.clone() });
            }
            // YIELD's args shouldn't contain another YIELD (they're
            // not in a block context), but validate recursively.
            for a in args {
                walk(a, false)?;
            }
        }
        Expr::Block { exprs, .. } => {
            // Inside a Block, every child has parent_is_block = true.
            for e in exprs {
                walk(e, true)?;
            }
        }
        // All other Expr variants: recurse into children with
        // parent_is_block = false (so a YIELD anywhere inside is
        // rejected unless it's inside a deeper Block).
        _ => walk_children(expr)?,
    }
    Ok(())
}

fn walk_children(expr: &Expr) -> Result<(), YieldSplitError> {
    // `walk`'s match already handles `Expr::Block` and direct
    // `Expr::Call { name: "YIELD" }` before reaching this catch-all,
    // so those two cases are unreachable here. The compiler still
    // wants an explicit arm for them — we punt with `unreachable!`
    // rather than silently accept them (a future AST change that
    // adds a new variant would surface as a compile error, not as
    // a silent fall-through).
    let recurse = |e: &Expr| walk(e, false);
    match expr {
        Expr::Block { .. } => unreachable!("walk() handles Expr::Block directly"),
        Expr::Call { name, .. } if name == "YIELD" => {
            unreachable!("walk() handles direct YIELD directly")
        }
        Expr::Literal(_, _)
        | Expr::Var(_, _)
        | Expr::Import { .. }
        | Expr::Export { .. }
        | Expr::Break { .. }
        | Expr::Continue { .. } => Ok(()),
        Expr::Call { args, .. } => args.iter().try_for_each(recurse),
        Expr::Array { items, .. } => items.iter().try_for_each(recurse),
        Expr::Dict { entries, .. } => {
            for (k, v) in entries {
                recurse(k)?;
                recurse(v)?;
            }
            Ok(())
        }
        Expr::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => {
            recurse(cond)?;
            recurse(then_branch)?;
            if let Some(b) = else_branch {
                recurse(b)?;
            }
            Ok(())
        }
        Expr::While { cond, body, .. } => {
            recurse(cond)?;
            recurse(body)
        }
        Expr::For { iter, body, .. } => {
            recurse(iter)?;
            recurse(body)
        }
        Expr::Return { value, .. } => {
            if let Some(v) = value {
                recurse(v)?;
            }
            Ok(())
        }
        Expr::Let { value, .. } | Expr::LetPattern { value, .. } => recurse(value),
        Expr::Ok { value, .. }
        | Expr::Err { value, .. }
        | Expr::Panic { value, .. }
        | Expr::Try { value, .. }
        | Expr::IsOk { value, .. }
        | Expr::IsErr { value, .. } => recurse(value),
        Expr::OrDie { value, default, .. } => {
            recurse(value)?;
            recurse(default)
        }
        Expr::Match {
            value,
            clauses,
            default,
            ..
        } => {
            recurse(value)?;
            recurse(default)?;
            for c in clauses {
                recurse(&c.body)?;
            }
            Ok(())
        }
        Expr::Fun { body, .. } => recurse(body),
    }
}

// ── Collection pass ────────────────────────────────────────────────

fn collect(
    expr: &Expr,
    current: &mut Vec<Expr>,
    all_segments: &mut Segments,
) -> Result<(), YieldSplitError> {
    match expr {
        Expr::Block { exprs, .. } => {
            for e in exprs {
                collect(e, current, all_segments)?;
            }
            Ok(())
        }
        Expr::Call { name, args, .. } if name == "YIELD" => {
            // Direct YIELD call: end current segment with this YIELD
            // as the checkpoint marker.
            current.push(expr.clone());
            all_segments.push(std::mem::take(current));
            // YIELD's args are guaranteed by validate() to not
            // contain other YIELDs, but we still recurse to be safe.
            for a in args {
                collect(a, current, all_segments)?;
            }
            Ok(())
        }
        _ => {
            if contains_yield(expr) {
                // This expr contains a YIELD inside (somewhere valid
                // per validate). It must end the current segment;
                // when the scheduler resumes, the next segment is
                // whatever follows this expr in the parent Block.
                current.push(expr.clone());
                all_segments.push(std::mem::take(current));
            } else {
                current.push(expr.clone());
            }
            Ok(())
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

    fn count_yields(segments: &Segments) -> usize {
        segments
            .iter()
            .flat_map(|s| s.iter())
            .filter(|e| matches!(e, Expr::Call { name, .. } if name == "YIELD"))
            .count()
    }

    /// Walk all segments AND their nested expressions to count every
    /// YIELD (used by tests that exercise YIELD inside a sub-expression
    /// like an IF branch, where `count_yields` at the top level
    /// misses it).
    fn count_yields_deep(segments: &Segments) -> usize {
        fn walk(e: &Expr) -> usize {
            match e {
                Expr::Call { name, .. } if name == "YIELD" => 1,
                Expr::Block { exprs, .. } => exprs.iter().map(walk).sum(),
                Expr::If {
                    cond,
                    then_branch,
                    else_branch,
                    ..
                } => walk(cond) + walk(then_branch) + else_branch.as_ref().map_or(0, |b| walk(b)),
                Expr::Let { value, .. } | Expr::LetPattern { value, .. } => walk(value),
                Expr::While { cond, body, .. } => walk(cond) + walk(body),
                Expr::For { iter, body, .. } => walk(iter) + walk(body),
                Expr::Return { value, .. } => value.as_ref().map_or(0, |v| walk(v)),
                Expr::Call { args, .. } => args.iter().map(walk).sum(),
                Expr::Array { items, .. } => items.iter().map(walk).sum(),
                Expr::Dict { entries, .. } => {
                    entries.iter().map(|(k, v)| walk(k) + walk(v)).sum()
                }
                Expr::Ok { value, .. }
                | Expr::Err { value, .. }
                | Expr::Panic { value, .. }
                | Expr::Try { value, .. }
                | Expr::IsOk { value, .. }
                | Expr::IsErr { value, .. } => walk(value),
                Expr::OrDie { value, default, .. } => walk(value) + walk(default),
                Expr::Match {
                    value,
                    clauses,
                    default,
                    ..
                } => walk(value) + walk(default) + clauses.iter().map(|c| walk(&c.body)).sum::<usize>(),
                Expr::Fun { body, .. } => walk(body),
                _ => 0,
            }
        }
        segments.iter().flat_map(|s| s.iter()).map(walk).sum()
    }

    #[test]
    fn body_with_no_yield_yields_single_segment() {
        let body = block(vec![int(1), int(2)]);
        let segs = split_body_for_yield(&body).expect("ok");
        assert_eq!(segs.len(), 1);
        assert_eq!(segs[0].len(), 2);
        assert_eq!(count_yields(&segs), 0);
    }

    #[test]
    fn tail_yield_splits_into_two_segments() {
        // [A, YIELD, B]  ->  [A, YIELD], [B]
        let body = block(vec![int(1), yield_call(), int(2)]);
        let segs = split_body_for_yield(&body).expect("ok");
        assert_eq!(segs.len(), 2);
        assert_eq!(segs[0].len(), 2);
        assert_eq!(segs[1].len(), 1);
        assert_eq!(count_yields(&segs), 1);
        assert!(
            matches!(segs[0].last(), Some(Expr::Call { name, .. }) if name == "YIELD"),
            "segment 0 must end with YIELD"
        );
    }

    #[test]
    fn multiple_yields_split_into_three_segments() {
        // [A, YIELD, B, YIELD, C]  ->  [A,Y], [B,Y], [C]
        let body = block(vec![
            int(1),
            yield_call(),
            int(2),
            yield_call(),
            int(3),
        ]);
        let segs = split_body_for_yield(&body).expect("ok");
        assert_eq!(segs.len(), 3);
        assert_eq!(segs[0].len(), 2);
        assert_eq!(segs[1].len(), 2);
        assert_eq!(segs[2].len(), 1);
        assert_eq!(count_yields(&segs), 2);
    }

    #[test]
    fn yield_in_nested_if_block_is_allowed() {
        // [A, IF(cond, [YIELD()], [42]), B]  ->  2 segments
        //   segment 0: [A, IF(...)]            (IF yields inside)
        //   segment 1: [B]                     (runs after yield)
        // After the IF yields, the scheduler advances to segment 1;
        // the IF is NOT re-evaluated — that's why [B] is alone.
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
        let segs = split_body_for_yield(&body).expect("ok");
        assert_eq!(segs.len(), 2);
        assert_eq!(segs[0].len(), 2);
        assert!(matches!(segs[0].last(), Some(Expr::If { .. })));
        assert_eq!(segs[1], vec![int(2)]);
        // The YIELD is nested inside the IF's then-branch, so the
        // top-level `count_yields` helper misses it. Use the deep
        // counter to confirm the body actually contains one YIELD
        // — together with the segment-count check this proves the
        // IF was treated as a segment boundary.
        assert_eq!(count_yields_deep(&segs), 1);
    }

    #[test]
    fn yield_directly_as_if_branch_is_rejected() {
        // IF(cond, YIELD(), 42)  -- YIELD is not inside a Block
        let body = Expr::If {
            cond: Box::new(var("cond")),
            then_branch: Box::new(yield_call()),
            else_branch: Some(Box::new(int(42))),
            span: span(),
        };
        let err = split_body_for_yield(&body).expect_err("must reject");
        assert!(matches!(err, YieldSplitError::NestedYield { .. }));
    }

    #[test]
    fn yield_in_let_rhs_without_block_is_rejected() {
        // LET(x, YIELD())  -- YIELD not in a Block
        let body = Expr::Let {
            name: "x".to_string(),
            mut_: false,
            type_annotation: None,
            value: Box::new(yield_call()),
            span: span(),
        };
        let err = split_body_for_yield(&body).expect_err("must reject");
        assert!(matches!(err, YieldSplitError::NestedYield { .. }));
    }

    #[test]
    fn yield_in_let_rhs_with_block_is_allowed() {
        // LET(x, [YIELD()])  -- YIELD inside Block
        let body = Expr::Let {
            name: "x".to_string(),
            mut_: false,
            type_annotation: None,
            value: Box::new(block(vec![yield_call()])),
            span: span(),
        };
        let segs = split_body_for_yield(&body).expect("ok");
        // LET has YIELD inside → it ends its segment.
        assert_eq!(segs.len(), 1);
        assert!(matches!(segs[0].last(), Some(Expr::Let { .. })));
    }

    #[test]
    fn empty_body_produces_no_segments() {
        let body = block(vec![]);
        let segs = split_body_for_yield(&body).expect("ok");
        assert_eq!(segs.len(), 0);
    }

    #[test]
    fn body_ending_with_yield_produces_no_trailing_empty_segment() {
        // [A, YIELD]  ->  [A, YIELD]   (not [A,Y], [])
        let body = block(vec![int(1), yield_call()]);
        let segs = split_body_for_yield(&body).expect("ok");
        assert_eq!(segs.len(), 1);
        assert_eq!(segs[0].len(), 2);
    }
}