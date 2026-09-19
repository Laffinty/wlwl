# ADR-0008 — Closure capture semantics: cell-based upgrade (Phase A2)

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-09-19 |
| **Deciders** | Li (project lead) |
| **Supersedes** | — |
| **Superseded-by** | — |
| **Related** | spec v0.4 §6.4, ADR-002 (tree-walking interpreter), Phase A2 implementation report `docs/history/20260907.md` |

## Context and Problem Statement

WLWL v0.3 §6.4 requires that closures (anonymous `FUN(...) -> T` literals) share
mutations to enclosing local variables with their enclosing scope. This is
needed because WLWL programs frequently use the "make-counter" idiom:

```wlwl
LET(counter, FUN((), LET(n, +(n, 1)); n));
LET(c, counter());   // 1
LET(c, counter());   // 2  ← observable stateful shared with caller
```

v0.1's closure representation is a `Vec<(String, Value)>` of the literal
environment, **cloned by value** when a closure crosses a scope boundary
(D006 — "deep clone" cost in §6.4). This produces three observable bugs
in v0.3 spec §6.4 examples:

1. `LET(n, 0); LET(step, FUN((), LET(n, +(n, 1)))); step(); step();` — `n`
   must remain shared across calls. v0.1 deep-clones → closure sees its
   own copy of `n`.
2. Closures of size N bytecode trigger O(N) clone per call site rather
   than O(1) `Rc::clone`.
3. Recursive closures that close over `self` (technically allowed under
   §6.3) cannot survive at all because the clone loses identity.

The fix must preserve the public observable semantics from §6.4 example
suite (sec.6.4.1..6.4.4) while keeping the rewrite within the existing
interpreter structure.

## Decision Drivers

- §6.4 spec must-pass (4 example programs)
- `heap_usage` benchmark must stay ≤ v0.3 baseline + 10 % (plan §2.7)
- Cell-based refactor must not break `let_module` (Phase A3) or `match`
  (Phase A4) when rolled forward
- No stdlib break
- Implementation must fit within Phase A2 budget (1 day per plan §A)

## Considered Options

### A. Add explicit `REF` keyword — closures declare which outer vars are mutable

```wlwl
LET(n, 0);
LET(step, FUN((), LET(REF(n), +(n, 1))));   // REF declares mutable
```

**Pros**: matches Cell semantics explicitly; type-checker-friendly
**Cons**: breaks spec §6.4 verbatim (the spec uses plain `LET(n, +(n, 1))`
without `REF`); larger break; requires parser + checker rework

### B. Implicit deep-clone of outer env per closure call site (v0.1 default)

**Pros**: zero changes to evaluation
**Cons**: spec-failing; O(N) cost; identity loss for recursive closures

### C. Cell-based upgrade (chosen)

Every `LET`-introduced local gets a `Cell<Rc<RefCell<Binding>>>` slot in
the environment. Closures copy the **cell pointer**, not the value. On
`LET` rebind, the cell is re-assigned in-place. On `SET`/`LET` re-bind
the type-checker verifies mutability.

**Pros**: spec-faithful (no language change), O(1) closure capture, works
for recursive closures; preserves heap accounting
**Cons**: interpreter gets a per-binding indirection; small ~5 % overhead
on tight loops (accepted per §2.7)

## Decision Outcome

Chosen option **C**. Implemented in Phase A2 (commit history in
`docs/history/20260907.md`); 8 new tests + 1 rewrite (净 +9) all pass;
workspace 541/541. Setup at §6.4.1..6.4.4 of spec updated example suite
verifies shared-state.

### Mutation semantics (post-ADR)

- A `LET(n, expr)` on an existing binding **always** rebinds the cell,
  never creates a new one — this is what makes recursive counter work.
- A captured cell exposed to a closure: `SET`-style assignment from
  inside the closure is forbidden (§2.6) unless the binding was
  marked mutable at let site.
- Closure equality: two closures compare equal iff they capture the same
  cells **and** have the same body — this gives the §6.4.3 word-count
  example the expected output (two closures created in a loop share cells).

## Consequences

Positive:

- §6.4 verbatim example suite passes without parser change
- closure_density benchmark: 5 % slower per call vs v0.1 (within budget)
- heap-stability benchmarks remain green
- foundation for A3 let_module and A4 match — both consume cell-aware env
  via the same trait

Negative:

- 6.4.4 set-cell-in-closure test now exposes a precedent: bindings
  created inside a closure's body and never observable outside are still
  upgraded to cells, which slightly bloats the heap (1 cell/closure for
  transient bodies). Documented in Phase A deviations P4-A2-001.
- v0.5 plan: if heap accounting becomes noisy, profile C1 (per-cell cost)
  before considering box-vs-direct representation.

## References

- spec v0.4 §6.4 (Closure capture)
- docs/plan/wlwl-build-plan-v0.2.md §A2 + §3
- docs/history/20260907.md (Phase A2 implementation report)
- specs/history/20260906.md (precursor discussion on cell costs)
