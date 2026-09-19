# ADR-0010 — `strict_types` runtime check via transient cast insertion (Phase E1)

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-09-19 |
| **Deciders** | Li (project lead) |
| **Related** | spec v0.4 §2.7, ADR-003 (v0.1 type-system baseline), Phase E1 implementation report |

## Context and Problem Statement

WLWL v0.3 §2.7 introduces the `strict_types` flag (manifest
`[strict_types] = true`). When enabled, type annotations on functions
**must** be honored at every call site, including implicit coercions
between numeric types (e.g. `INTEGER` vs `FLOAT`), and assignments from
literals with annotations.

The v0.1 baseline (`wlwl-toml/src/manifest.rs::strict_types` reads the
flag but does nothing) kept type checks purely at name-resolution time:
a function declared `(a: INTEGER) -> INTEGER` would silently accept
`FLOAT` arguments as long as the names match.

There are two implementation strategies:

1. **Natural monitoring** — collect type checks during evaluation,
   find violations at the end. (v0.1 default behavior; just records
   statistics.)
2. **Transient cast insertion** — at evaluation time, when the strict
   flag is on, the evaluator checks the actual value's type against
   the expected type and either inserts an `E0031 numeric coercion`
   or returns E0032 (annotation mismatch).

Plan §2.7 mandates ≤10 % overhead on the strict path. We need a choice
that:

- Triggers per-call with minimal amortized cost when `strict_types =
  false` (the default)
- ≤10 % overhead when `strict_types = true`
- Does not change v0.1 evaluation outcomes (numerical coercions that
  silently succeed stay silent when strict_types = false)

## Decision Drivers

- §2.7 perf budget (≤10 % overhead)
- Backward compat with v0.1 (when strict_types = false)
- Concrete user-facing errors when strict_types = true (E0031/E0032)
- No re-compile needed — flag is runtime-mutable from `wlwl.toml`

## Considered Options

### A. Natural monitoring (always-on)

Always collect type-check statistics. Replay at the end of run.

**Pros**: zero per-call cost on hot path
**Cons**: only finds violations at end-of-run; doesn't fail-fast;
violations accumulate through nested calls making root cause hard
to attribute

### B. Transient cast insertion per call (v0.1 baseline strategy, generalized)

Each call site checks: does the argument's actual `Value::kind()` match
the formal-parameter type? When `strict_types = false`, the check is
**only** an upcast-tracker (INTEGER-to-FLOAT is upcast; STRING-to-INTEGER
is downcast and emits E0031 only when strict). When `strict_types = true`,
**any** mismatch emits the appropriate diagnostic.

**Pros**: matches spec wording for strict_types; per-call cost measurable
explicitly; enables fail-fast on the first offending call
**Cons**: 4 % overhead measured (plan §3 §6.6 perf benchmark) — within
budget

### C. Whole-program type inference at parse time

Static analysis: infer every expression's type from the parse tree,
emit errors at compile time when `strict_types = true`.

**Pros**: zero runtime cost
**Cons**: requires a static type-checker; triple the work for what v0.3
spec §2.7 documents as a runtime flag; would not unify with `runtime
check` semantics

## Decision Outcome

Chosen option **B** (transient cast insertion). Implementation:

- `wlwl-eval/src/registry.rs::strict_types_invocation_check` is the
  `Strict cast` insertion point; it adds:
  - `INTEGER → FLOAT` (silent upcast, even under strict)
  - `INTEGER → INTEGER` (silent, narrowing is no-op)
  - All other mismatches → `E0031 numeric coercion` or
    `E0032 annotation mismatch`
- `wlwl-toml` reads `manifest.strict_types()` (added in Phase E1.1)
  and propagates as `Evaluator.strict_types: bool`.
- `wlwl-cli/src/run.rs` exposes `--strict-types` flag (CLI override
  in case manifests don't ship strict_types yet)
- `wlwl-error` adds `WlwlDiagnostic::with_strict_types_violation(..)`
  helper so the strict-mode error path is one method call

Implementer notes: this approach gives us **spec-accurate semantics
with the same per-call cost as v0.1's "monitoring" mode** when
strict_types = false. When strict = true, the cost is amortized
across the per-call site — only types referenced in the call's
signature get a check.

## Consequences

Positive:

- ≤10 % overhead spec-aligned (measured 4 % on simple_loop_1m benchmark
  in `benches/baseline.txt`)
- Fail-fast diagnostic path on first offending call (better UX than
  end-of-run dump)
- CLI flag allows phased rollout without manifest bump
- Tests +12 in Phase E1 (also part of the missing_docs surface in G3
  P4-G3-001)

Negative:

- The v0.1 `monitoring` path (option A) is removed. We lose the "end of
  run" type dump capability. Documented as a v0.4 feature-loss; v0.5
  brings it back as an `--explain-types` debug flag.
- No upstream interaction with `expect_type` annotations (Phase F
  follow-up) — those are currently parsed but not consulted.

## References

- spec v0.4 §2.7 (strict_types flag)
- docs/plan/wlwl-build-plan-v0.2.md §E1 (Phase E plan)
- docs/history/20260919e1.md (Phase E1 implementation report)
- Phase G3 P4-G3-001 covers leftover missing_docs in this subsystem
