# ADR-0009 — `ERR` consumer registry replaces a closed allow-list (Phase A6)

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-09-19 |
| **Deciders** | Li (project lead) |
| **Related** | spec v0.4 §12.7, ADR-007 (rule-wide registry pattern), Phase A6 implementation report, `wlwl-eval/src/registry.rs` |

## Context and Problem Statement

Spec v0.4 §12.7 introduces a static check: when a callable returns `ERR`,
the compiler must verify that **someone** in the call graph has registered
the `ERR_CONSUMER_REGISTRY` capability for that specific error code.

v0.3 had a closed hard-coded allow-list embedded in the name-checker
(`spec §12.7` v0.3 wording: "the following stdlib functions are
recognized as legitimate consumers …"). This made adding a new consumer
function (e.g. a v0.4 stdlib extension) require a compiler patch.

The v0.4 §12.7 rewrite instead treats consumer registration as a
**runtime-registered** capability, parallel to how `StdCtx.warnings` and
`StdCtx.errors` sinks work (Phase D registry).

## Decision Drivers

- §12.7 v0.4 example suite: 6 e2e .wll programs that depend on registering
  consumers per code
- Avoid forcing compiler patch for each stdlib addition
- Keep the static check (still fail at compile-time if no consumer at
  runtime) — this is not pure dynamic dispatch
- Forward-compat with v0.5 wildcards

## Considered Options

### A. Closed allow-list (v0.1/v0.3 baseline)

Hard-coded list of consumer function names in
`wlwl-parser::check_err_consumer`. Re-add per release.

**Pros**: simplest model, fully static
**Cons**: requires compiler patch per stdlib function; §12.7 v0.4
explicitly says "the registry is open-ended"

### B. Function attribute on stdlib functions

Tag every consumer function with `#[wlwl_err_consumer("E0083")]`. Compile
fails when a stdlib function uses an unregistered code.

**Pros**: declarative; re-uses Rust attribute syntax
**Cons**: requires `wlwl-std` to depend on `wlwl-eval`'s proc-macro; tight
crate coupling; proc-macros add build time

### C. Runtime-mutated registry (chosen)

Single global `ErrConsumerRegistry` (lock-free `DashMap` or `RwLock`
behind `HashMap` depending on perf needs) keyed by error code. Consumer
functions register themselves in the registry at evaluator startup
(Phase A6 `wlwl-eval/src/registry.rs`). The compiler's static check is
just "is there any entry in this registry for code X by the time we
emit the function?". For §12.7 examples this is checked at first call.

**Pros**: stdlib / user functions can register codes themselves with
no compiler coordination; matches the existing `Evaluator.test_registry`
pattern (P3-007) and the Warnings sink (D2)
**Cons**: 1 µs lookup per static check; small race window between
registration and first call

## Decision Outcome

Chosen option **C**, codified as:

- `wlwl-eval/src/registry.rs::ErrConsumerRegistry` — `RwLock<HashMap<ErrorCode, Vec<ConsumerHint>>>`
  with `register_consumer(code, hint)` and `consumers_for(code)` API
- Compiler emits `Evaluator::verify_err_consumer(code)` at the call site
- Stdlib registers its own consumers in its `init` function
  (`wlwl-std/src/ai.rs`, `wlwl-std/src/agent.rs`, …) so the registry is
  populated before any user function runs

Trade-off accepted: the "static" part is now a property of the stdlib
*initialization order*, not of the source code alone. Spec §12.7 is
re-interpreted to mean "is registered by the time the call fires",
which matches how `EXPECT_ERR` (Phase B7) already worked.

### Tradeoffs documented in deviation P4-A6-001

- Subtle: code that runs before stdlib `init` (theoretically possible
  with `--no-stdlib` flag) cannot use ERR registration. We keep this
  as a Phase F feature, not a v0.4.0 blocker.

## Consequences

Positive:

- Spec §12.7 v0.4 e2e tests pass (12 added in Phase A6; +17 net new in
  workspace total)
- Open-ended: any future stdlib function can register codes
- Compatible with D2 warnings sink — same architecture
- Test coverage in `wlwl-error/src/snapshots` for the 56 + 14 codes
  (Phase G7 target)

Negative:

- 1 µs RTT for `verify_err_consumer(code)` at each call site that returns ERR
- The "static" promise of §12.7 is now "static by initialization time",
  which weaker than the v0.1 wording — explicitly documented in spec
  appendix G so AI tools and external docs know

## References

- spec v0.4 §12.7 (ERR consumer registration)
- docs/plan/wlwl-build-plan-v0.2.md §A6 (Phase A plan)
- docs/history/20260908.md (Phase A6 implementation report)
- Phase B7 (`std_test` test framework) consumes this registry
- Phase D2 (warnings sink) is the architectural template
