# Changelog

> **Note.** The compiler version is **independent of the language
> spec version**. The language spec lives in `docs/standard/` and
> is content-addressed. This page tracks the **compiler / tooling**
> releases.

!!! note "On this page"

    - [Unreleased — v0.7.0](#unreleased--v070)
    - [v0.6.0](#v060)
    - [v0.3.0](#v030)

## [Unreleased — v0.7.0] {#unreleased--v070}

> **WIP.** `wip0.7` branch. Plan: `docs/plan/wlwl-build-plan-v0.7.md`.
> Spec file (Phase H1) is **not** yet cut.

This release introduces the **structured-concurrency runtime**: a
cooperative single-thread scheduler (§5.1), `SCOPE` / `SPAWN` /
`AWAIT` / `YIELD` / `TASK_*` builtins (Phase C), and the
cancellation scope tree (Phase F). The error / channel / SHIELD
work (Phases D / E / F) is still pending — see the plan.

### Added

- **Phase A (research + design landed)** — `docs/plan/wlwl-build-plan-v0.7.md`
  (commit `01ff9d6`), ADR-0014 / 0015 / 0016 (commit `0d463ad`),
  `deviations.md` v0.7 section marker (commit `e9d2106`).
- **Phase B (coroutine runtime skeleton)** — error codes
  `E0052`-`E0058` + `ErrorCategory::Concurrent` (`b38126e`);
  `runtime.rs` type stubs — `TaskId`, `TaskHandle`,
  `TaskState`, `YieldReason`, `Scheduler` (`7f9d54f`);
  `Evaluator.current_task: Option<TaskId>` (`86cbddf`);
  v0.6 fidelity golden (`6cac0f1`); `Task` + `Scope` data
  structures (`5634331`); `StepResult` + `step_once` wrapper
  (`5ff79e5`); single-task benchmark baseline (`ad0dd60`);
  Scheduler allocation API reconciled for B5b prep
  (`d53e09e`).
- **Phase C1 / C2 (built-in partial scope)** — `SCOPE(fn)`
  (`e28de1d` + `cce3ce3`); `SPAWN(fn)` (`cce3ce3`, with the
  audit-fix chain `1248978` / `d46bab3` / `d53e09e`).
- **Phase B5a-3 slice 1 (state-machine yield plumbing)** —
  `Signal::Yield(YieldReason)` variant + propagation through
  `eval_expr` (loops, blocks, closures all propagate Yield
  upward like `Return`); `Evaluator::step_once` rewritten as a
  real step (mirrors `eval`'s E0102 promotion, then translates
  `Outcome.signal` into `StepResult`); internal `__YIELD_TEST__`
  marker for smoke tests. v0.6 fidelity golden preserved
  byte-for-byte. (Commit `836a5ce`.)
- **Phase B5a-3 slice 2 (smoke baseline + full refactor)** —
  `__YIELD_AFTER_ARG_TEST__(fn)` 1-arg marker that exercises
  `Signal::Yield` propagation through `eval_call`'s
  argument-eval for loop (`90ace1c`); full slice 2 refactor —
  extracted `eval_arg_values` / `dispatch_call` /
  `outcome_to_step_result` helpers from `eval_call`, added
  parallel `step_call` (iterative / step-friendly), and routed
  top-level `Expr::Call` through `step_call` in `step_once`
  (`8b65575`).

### Pending (next-phase work, not yet committed)

- Phase C3 `AWAIT(handle)` / C4 `YIELD()` / C5 `TASK_*`
- Phase B5b Scheduler wire-up
- Phase D Channels (`CHANNEL_NEW` / `_SEND` / `_RECV` / ...,
  8 sub-items)
- Phase E error propagation (with **E4** as the blocker:
  ERR-consumer registry cross-task regression)
- Phase F cancellation scope tree (incl. SHIELD with ≥3
  conformance fixtures)
- Phase G quality gates (clippy 0-warning, rustdoc 100%,
  fuzz 24h, cargo-deny 0, single-task perf ≤ 10% regression,
  spec-file freeze)
- Phase H `docs/standard/wlwl-spec-v0.7.md` + `v0.7.0` tag
  + plan-file rename with COMPLETED banner

## [v0.6.0] {#v060}

> Released 2026-09-20 (commit `d0a4742`). The notes below were
> originally tracked under an "Unreleased — v0.4.0" heading and
> shipped as a single jump to v0.6.0 — no separate v0.4 or v0.5
> compiler release was cut. For the canonical Keep-a-Changelog
> format, see the root `CHANGELOG.md` file.

### Added

- **Phase A1–A8** — closure cell semantics (Phase A2), error
  schema 1.1.0 with `trace` / `cause` (Phase A1d), `LET` patterns
  (A3), `MATCH` (A4), cell-upgrade in `LET` (A5), ERR consumer
  registry (A6), cause-chain `WRAP` / `UNWRAP` / `ERR_PAYLOAD`
  (A7), 47 + error codes (A8).
- **Phase B1–B15** — `INDEX_GET`/`SET`/`AT`/`REMOVE_KEY`/`POP`
  (B1), `DEL` alias (B2), `OR_DIE` canonicalization (B3),
  `UNWRAP`/`ERR_PAYLOAD`/`WRAP` (B4), `FORMAT` global builtin +
  `wlwl:std.format` (B5), `wlwl:std.collection` 17 higher-order
  functions (B6), `wlwl:std.test` `ASSERT`/`EXPECT_*` framework
  (B7), 11 string builtins (B8), `NOT` (B9), `PRINT_ERR` to
  stderr (B10), appendix G registry (B11), ARRAY/STRING/DICT
  ops global (B12–B14), `INPUT`/`BOOL`/`CALL`/`NEG` + OOP-stub
  (B15).
- **Phase C1–C7** — `AS` keyword removal (C1), `MODULE_REF` real
  impl (C2), `language_version` + E0044 (C3), MVS Cargo-style +
  E0045 (C4), `allow_builtin_shadow` + E0025/W0030 (C5),
  `wlwl.lock` consistency + E0042 (C6), project-root boundary +
  E0040 (C7).
- **Phase D1–D5** — real-ai HTTP bridge (D1), `wlwl:std.agent`
  (D3), W0052 (D5), E0090–E0094 network ladder (D4), ai/agent
  module coverage.
- **Phase E1–E4** — `strict_types` runtime check (E1), canonical
  formatter + `wlwl fmt` (E2), AST-stable node ID + SHA-256
  hash (E3), W-code unified channel (E4).
- **Phase G1–G12 quality gates** — clippy zero-warning + fmt gate
  (G1), cargo-deny license+advisory gate (G2), rustdoc validity
  gate (G3), 6 ADRs ADR-0008..0013 (G4), 0-unsafe miri scaffolding
  (G5), cargo-fuzz harness for lexer/parser/eval (G6), insta
  snapshot meta-coverage (G7), `cargo bench` smoke (G8), weekly
  cargo-audit supply-chain workflow (G9), CycloneDX SBOM + cosign
  keyless signature per release (G10), MkDocs user site (G11),
  README + 5 new examples (G12).

### Changed

- workspace license `GPL-2.0` → canonical SPDX `GPL-2.0-only`.
- `AS` keyword removed entirely (lexer drops `TokenKind::As`,
  parser emits E0011 with migration note).
- closure capture is now cell-based (ADR-0008), O(1) `Rc::clone`
  per call.
- error schema bumped to 1.1.0 — every diagnostic now carries
  `trace`, `cause`, `related`, `retry_after` per spec §14.2.

### Removed

- `AS` keyword (v0.3 §4.2.1) — use `INT(x)`/`FLOAT(x)`/`STR(x)`/`BOOL(x)`.

## [v0.3.0] {#v030}

### Added

- Lexer, parser, eval (tree-walking interpreter).
- 35 error codes + 10 warning codes.
- Stdlib v0.3 (`std.io`, `std.fs`, `std.json`, `std.format`,
  `std.ai`, `std.agent`).
- Per-site `suggestion_code` codegen (P3-008).
- Formal coverage instrumentation (P3-009).
