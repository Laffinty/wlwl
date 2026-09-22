# Changelog

All notable changes to WLWL (the implementation) are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

> **Note.** The compiler version is **independent of the language spec version**.
> The language spec lives in `docs/standard/` and is identified by version + name.
> This file tracks the **compiler / tooling** releases. The spec is currently at
> **v0.7** (`docs/standard/wlwl-spec-v0.7.md`); v0.6 is archived at
> `docs/history/wlwl-spec-v0.6.md`.

## [v0.7.0] — 2026-09-22

First concurrency release. Spec: **wlwl-spec-v0.7** (additive over v0.6).
Build plan + deviations: `docs/history/wlwl-build-plan-v0.7-COMPLETED.md`,
`docs/history/deviations-v0.7.md`.

### Added (language)

- **Structured concurrency + channels** (spec §17). New global builtins
  (17), none of which are ERR consumers — `ERR` arguments still
  transparently propagate per §8.2:
  - Scope / task: `SCOPE(fn)`, `SPAWN(fn)`, `AWAIT(task)`, `YIELD()`
  - Cancel: `TASK_CURRENT()`, `TASK_IS_CANCELLED()`, `TASK_CANCEL(task)`,
    `TASK_CANCEL_PARENT()`, `SHIELD(fn)`
  - Channel: `CHANNEL_NEW(buf)`, `CHANNEL_SEND`, `CHANNEL_RECV`,
    `CHANNEL_TRY_SEND`, `CHANNEL_TRY_RECV`, `CHANNEL_CLOSE`,
    `CHANNEL_LEN`, `CHANNEL_CAP`
- **New value types** `TASK` and `CHANNEL` (TYPE strings). Handle
  identity equality (`==(h, h)` is TRUE); display
  `<task handle id=N gen=M>` / `<channel handle id=N gen=M>`.
- **New error codes** (category `Concurrent`):
  - `E0052` SCOPE/SPAWN/SHIELD arg is not a function
  - `E0053` invalid task/channel handle (`TASK_CURRENT` outside a task)
  - `E0054` SEND/TRY_SEND on a closed channel
  - `E0055` reserved (close-on-read surfaces as `ERR(kind="ChannelClosed")`)
  - `E0056` SPAWN arity (including non-zero-param `fn`)
  - `E0057` reserved (cross-task immutable cell currently reuses `E0024`)
  - `E0058` top-level `SPAWN` without an active `SCOPE` (no implicit runtime scope)
- **Structured ERR kinds** (dict payloads, §8.1):
  `"ChannelClosed"` | `"ChannelWouldBlock"` | `"Cancelled"`.
  `NULL` is **not** a close signal — detect via `IS_ERR` + `ERR_PAYLOAD`.
- **Runtime modules** in `wlwl-eval`: `runtime` (scheduler / handles),
  `task`, `channel`, `yield_split` (path-B body segmentation).
- **BUILTIN_REGISTRY** 93 → **110** (17 concurrent entries + `Version::V07`
  + `BuiltinGroup::Concurrent`). Regenerate the table with
  `cargo run --bin gen-appendix-g -- ../docs/appendix_G.md`.
- **Conformance fixtures** `impl/tests/concurrency/*.wll` (yield, channel,
  SCOPE ERR surfacing, ERR consumers cross-task, 3× SHIELD).
- **Bench suite** `concurrency` (single-task fidelity, scope spawn/exit,
  channel throughput, close→RECV latency).

### Changed (non-breaking)

- **Function / handle equality** (`==`, `!=`): implementations now match
  v0.6 §2.4 instance identity for closures and add identity for
  `TASK`/`CHANNEL` handles (previously both always returned FALSE).
- **Trailing `YIELD()`** on the last body segment completes the task
  with `NULL` instead of aborting the process (path-B regression fix).
- **Spec lock** (`b11_registry_count_matches_spec_table`) pinned at 110.

### Known limits (v0.7.0)

Documented in spec §17.7 and `deviations-v0.7.md` — not silent gaps:

- Single-thread cooperative scheduling (no CPU parallel speedup).
- `CHANNEL_SEND`/`CHANNEL_RECV` do **not** suspend: full/empty yields
  `ERR(kind="ChannelWouldBlock")` (`P7-D2-001`). Use `TRY_*` or buffer.
- No bounded-channel deadlock detector (`P7-D8-001`).
- Nested `YIELD` inside `WHILE`/`FOR`/`IF` does not resume the nested
  construct remainder (path-B segmentation).
- In-flight sibling cancel may be unobservable under sync run
  (`P7-E3-001`); `SCOPE` still surfaces the first uncaught child `ERR`.
- Captured-`LET` upgrade (legacy E-CloCap) still diverges from a strict
  reading of §3.3 — prefer `LET MUT` for shared mutation.

## [v0.6.0] — 2026-09-20

### Changed (breaking)

The implementation now matches the **v0.6 specification** (the v0.5 spec
was never published; this is the first versioned release). Nine breaking
semantic changes (per the spec's Appendix B) are now in force:

- **A · Truthiness**: `0`, `0.0`, `""`, empty ARRAY, empty DICT, `NaN` are
  now falsy. Previously only `FALSE` and `NULL` were. (Matches Python /
  JS / Ruby conventions; the v0.5 design was deemed too unusual.)
- **B · `&&` / `||` short-circuit**: left operand's truthiness decides
  whether the right is evaluated; ERR on the left propagates without
  touching the right. Previously both sides were always evaluated.
- **C · `IF` consumes `ERR`**: an `ERR` condition routes to the `else`
  branch (or propagates `ERR` if no else). Previously ERR was treated
  as truthy and took the then-branch.
- **D · `!` and `NOT` accepted without warning**: `W0054` (the
  `!`-is-deprecated warning) was removed. Both forms are now equally
  canonical.
- **E · `POP` renamed `AT_K`**: the dict lookup-with-default helper
  is now `AT_K(d, k, default)`. `POP` remains as a back-compat alias
  but emits no warning. `REMOVE_KEY` is the dedicated removal function.
- **F · String subscript read**: `s[i]` returns a single-codepoint
  string; negative indexes count from the end. `s[i] = ...` is
  rejected (`E0030`).
- **G · Explicit `LET MUT`**: mutable bindings must be declared
  `LET MUT(name, value)`. The v0.5 "first closure call upgrades cell
  to mutable" rule was deemed too magical and removed. `SET` on a
  `LET` (immutable) binding raises `E0024`.
- **H · Integer overflow throws `E0035`**: replaced the v0.5
  "saturate to i64::MAX/MIN + emit `W0015`" behavior with an
  explicit error. `E0035` is now shared with float-to-int overflow.
- **J · String interpolation**: `"hi ${name}!"` form, with `\$`,
  `\n`, `\t`, `\\`, `\"`, `\/`, `\0`, `\b`, `\f` escapes. Inner
  expressions are evaluated and `STR`-rendered; an `ERR` inside
  `${...}` propagates without producing partial output.

### Added

- **String interpolation lexer**: 3-token form `StrStart`,
  `StrText(String)`, `StrEnd`; recursive sub-lex for the inner
  expression. The AST adds `Literal::Interpolated(Vec<StrPart>)` and
  a `StrPart` enum (`Text` / `Expr`).
- **`MUT` keyword** (lexer-level contextual): `TokenKind::Mut`,
  consumed by the parser only between `LET` and `(`. Other positions
  treat it as a regular identifier.
- **`BOOL` registered as ERR consumer**: `BOOL(ERR(...))` returns a
  boolean instead of triggering §8.2 transparent propagation.
- **`&&`, `||` registered as ERR consumers**: short-circuit paths
  live in a new `eval_logical_short_circuit` helper, intercepted in
  `eval_call` before the generic ERR-propagation block.
- **`IF` registered as ERR consumer**: documented in the registry
  for completeness; the actual handling lives in `eval_if`.
- **String subscript AST path**: `INDEX_GET` accepts `(String,
  Integer)`; `INDEX_SET` rejects `String` first arg with `E0030`.

### Changed (non-breaking)

- **Cell mutability flag is permanent**: `Binding.mutable` is set
  at binding creation; no "closure-capture upgrade" mechanism
  remains. The dead `Env::upgrade_all_to_mutable` was removed.
- **Error message updated**: `SET` on an immutable binding now
  suggests `LET MUT` in its message.
- **Registry**: `Version::V06` added; `&&`/`||`/`AT_K` added as
  `ResolvedBuiltin`; `POP` demoted to `ResolvedCompat`. Total
  entries: 90 → 93.
- **`BOOL` is now an ERR consumer in the registry**, matching its
  runtime behavior.
- **Removed `W0015`** (integer saturate warning) — now `E0035` on
  overflow.
- **Removed `W0054`** (`!` deprecation) — `!` is canonical.

### Fixed

- **Parser**: `LET MUT(name, value)` now correctly accepts the
  `MUT` keyword between `LET` and `(`. (Earlier draft had the
  syntax as `LET(MUT name, value)`, which was off-spec.)
- **Formatter**: `Literal::Interpolated` segments are rendered
  with proper escape sequences (`escape_str_text` helper); `MUT`
  keyword preserved when re-formatting `LET MUT(...)` expressions.

## [Unreleased — v0.5.0]

### Added

- **wlwl-spec-v0.6** — supersedes v0.5 with the nine breaking
  semantic changes above. The v0.3 and v0.4 specs have been moved
  to the trash (recoverable).

> **Note.** v0.5 was never published — the spec was drafted but not
> tagged, and the implementation never claimed compliance with it.
> The v0.5 spec is in `docs/standard/` was moved to trash on
> 2026-09-20 along with v0.3 and v0.4.

## [Unreleased — v0.4.0]

### Added

## [Unreleased -- v0.6.1] (extension rename)

### Changed (breaking)

- **File extension renamed from .wl to .wll**: Wolfram
  Language (Mathematica) has long claimed .wl, which causes
  editors, GitHub Linguist, Shiki/Rouge/Prism highlighters, and the
  Wolfram VSCode extension to mis-identify WLWL source files. The
  .wll extension has no prior claim from any other language, so
  adopting it costs zero ecosystem work. This is the first breaking
  change at the *file-format* level (the spec semantics are unchanged).

### Migration

- All tracked .wl files renamed to .wll (12 in impl/examples/ + 1
  in wlwl-skill/). git log --follow preserves history.
- Test fixtures, error messages, doc paths, CI commands, and skill
  bundle references updated.
- examples/showcase.wll still contains a pre-existing bug (lowercase
  	rue keyword); unrelated to this rename.
- Conformance test fixtures (impl/tests/conformance/*.wll) were left
  untouched; their internal references to .wll source files were
  updated.


### Note on the spec filename

- Renamed docs/standard/wlwl-spec-v0.6(SHA1_cdb548cb5161e61d836aad2208fd33adc0917861).md to docs/standard/wlwl-spec-v0.6.md. The SHA1 in the old filename never matched the file's content (the v0.5 spec had the same issue), so the content-address fiction is dropped entirely. Specs are now identified by version + name only; git history (git log -p --follow) is the source of truth for content changes.

## [Docs archival] — 2026-09-22

- `docs/standard/wlwl-spec-v0.6.md` → `docs/history/wlwl-spec-v0.6.md`
- `docs/plan/*` → `docs/history/` (`wlwl-build-plan-v0.7-COMPLETED.md`,
  `wlwl-phase-b-implementation-plan.md`, `deviations-v0.7.md`)
- `docs/plan/` now holds only a README pointing at the archive;
  next iteration starts a fresh plan + `deviations.md` there.
