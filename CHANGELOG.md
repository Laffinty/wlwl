# Changelog

All notable changes to WLWL (the implementation) are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

> **Note.** The compiler version is **independent of the language spec version**.
> The language spec lives in `docs/standard/` and is identified by version + name.
> This file tracks the **compiler / tooling** releases. The spec is currently at
> **v0.8** (`docs/standard/wlwl-spec-v0.8.md`); v0.7 is archived at
> `docs/history/wlwl-spec-v0.7.md` (v0.6 at `docs/history/wlwl-spec-v0.6.md`).

## [v0.8.0] — 2026-09-23

Spec: **wlwl-spec-v0.8** (released 2026-09-23; v0.7 archived at
`docs/history/wlwl-spec-v0.7.md`). Build plan + deviations
(archived post-release):
`docs/history/wlwl-build-plan-v0.8-COMPLETED.md`,
`docs/history/deviations-v0.8.md`.

### Compatibility commitment (per plan §4.4)

Any v0.7 program that does **not** depend on the §12 "保留形式" old
table (which is rewritten away in v0.8 — see below) yields identical
results in v0.8. Confirmed observable-behavior changes:

1. **Literal subscripts now allowed**: `[1, 2, 3][0]` was a parse
   error (`E0010` / `E0011`) in v0.7 and earlier; in v0.8 it parses
   and evaluates to `1`. (`["a": 1]["a"]`, `[[1,2][0], 3]`, etc. —
   all allowed for the first time.) See `2bd11b3` and deviation D8-004.
2. **No other program-visible change.** Everything else is either
   prose-only clarification, metadata-only (registry fields,
   appendix_G anchors), or aligns spec text with behavior the
   implementation has had all along.

### Changed (non-breaking)

- **§12 保留形式重写 (D8-005)**: the "reserved but undefined" list
  (CLASS / NEW / THIS / MODULE / MODULE_REF / CALL / ARRAY / AND / OR)
  was implementation-stale — all entries have working `BuiltinSpec`
  records in `BUILTIN_REGISTRY` (LexerMacro / ResolvedBuiltin /
  ResolvedCompat). §12 now points to the registry as the single source
  of truth; the `b11_*` lock tests + the new
  `registry::tests::appendix_g_anchors_match_v07_section_numbers`
  (§2.9 / `061b0fd`) gate consistency. See `b266700`.
- **§4.3 NOT 透明传播 prose 反向 (D8-006)**: v0.7 prose wrongly
  framed NOT as the sole exception to §8.2 transparent propagation.
  Implementation has always been E0102-on-`NOT(ERR(...))`; prose now
  matches. `b9_not_with_err_arg_is_e0034` and
  `b11_err_consumer_registry_consistent` were already enforcing the
  corrected behavior. Recommended idiom: `NOT(BOOL(ERR(...)))` for
  safe coercion. See `32c9ada`.
- **§17.1 YIELD 位置限制 demoted (D8-007)**: the "YIELD() 必须
  出现在 Block 直接子项" rule was normative prose over what is
  actually an implementation choice (static task-body segmentation
  in `yield_split.rs`). §17.1 now frames it as "理想语义 + v0.7 /
  v0.8 实现路径" so future suspension-based schedulers can lift
  the limitation without claiming a breaking change. See `f028860`.
- **Spec / registry alignment (D8-008, 12-item batch)**:
  - 85 `BuiltinSpec.section` fields updated to v0.7 chapter numbers
    (registry was full of v0.4 / v0.6 stale numbers like §13.x /
    §15.x / §12.x / §7.x / §9.1 / §10.6 / §11.4) — `849dc3c`.
  - `POP` entry signature / group / version corrected
    (`POP(d, k, default) -> v` / `Dict` / `V06`) — `f8a5908`.
  - Column header in `docs/appendix_G.md` fixed
    (`ERR 消费者 (§12.7)` → `(§8.3)`, `宏函数 (§3.4)` → `(§1.4)`) —
    folded into `849dc3c`.
  - Spec §4.5 prose on literal subscripts rewritten to ALLOW; §A.2
    grammar was already permissive — `2bd11b3`.
  - Spec §1.6 split into `int_lit` (bare digits) + `int_literal`
    (optionally signed); parser / lexer source comments updated to
    document the two-step lexer-bare-digits + parser-desugar
    reality — `9c8bc08`.
  - `EXPECT_ERR` folded into the §8.3 consumer table; §2.4 %
    float `E0030` listed; SHIELD / SCOPE(ERR) / AWAIT host
    diagnostic clarifications; §1.5 `=` triple-identity
    disambiguation; §5.1 LET/FUN asymmetry normative; §2.1 / §10.11
    `TASK` name-collision normative — `2bf9093`.
  - `wlwl:std.agent` and `wlwl:std.ai` module docstrings updated with
    a "与类型名 TASK 的同名问题" section — `d1239e7`.

### Marked RESERVED (no trigger path; documentation only)

- **E0055** "CHANNEL_RECV after close native code": signal
  surfaces as `ERR(kind="ChannelClosed")` dict payload (§8.1 /
  §17.2), not as native code. `e0055_channel_recv_after_close_does_not_raise_native_code`
  test gates future regressions — `1b150db` + D8-003.
- **E0057** "cross-task immutable cell native code": same condition
  triggers `E0024` (unified immutable-cell code, §17.4).
  `e0057_immutable_cell_set_raises_e0024_not_e0057` gates it — same
  commit / deviation.

### Added (consistency tests)

- `pop_registry_entry_matches_dispatch` (D8-001 regression) — `f8a5908`
- 4 parser literal-subscript round-trip tests
  (`parser_array_literal_subscript_roundtrip`,
  `parser_dict_literal_subscript_roundtrip`,
  `parser_mixed_literal_subscript_chain`,
  `parser_nested_literal_subscript_in_array`) — `2bd11b3`
- 4 eval literal-subscript end-to-end tests
  (`eval_array_literal_subscript`, `eval_dict_literal_subscript`,
  `eval_chained_literal_subscript`,
  `eval_literal_subscript_with_set_sugar`) — `2bd11b3`
- 4 parser unary-minus round-trip tests
  (`unary_minus_integer_literal_desugars`,
  `minus_call_with_paren_is_not_sugar`,
  `unary_minus_variable_desugars`,
  `minus_call_three_args_is_not_sugar`) — `9c8bc08`
- `eval_unary_minus_integer_min_throws_e0034_via_desugar` (locks
  full sugar path → `E0034`) — `9c8bc08`
- `eval_negative_literal_minus_one` (`-1 → -1`, `--1 → 1`,
  `LET(x, 7); -x → -7`) — `061b0fd`
- `appendix_g_anchors_match_v07_section_numbers` (locks §-anchor
  whitelist in `docs/appendix_G.md`) — `061b0fd`
- `e0055_channel_recv_after_close_does_not_raise_native_code`
- `e0057_immutable_cell_set_raises_e0024_not_e0057`

`cargo test --workspace`: ~1366 tests passing, 0 FAILED.

### Known limits (unchanged from v0.7)

Documented in spec §17.7:

- Single-thread cooperative scheduling.
- `CHANNEL_SEND` / `CHANNEL_RECV` do not suspend (path B); use
  `TRY_*` or pre-sized buffers.
- No deadlock detector.
- Nested `YIELD` inside `WHILE` / `FOR` / `IF` does not resume the
  nested construct remainder (path B).
- Captured-`LET` upgrade (legacy E-CloCap) still diverges from a strict
  reading of §3.3.

### Phase B summary

Spec prose alignment batch (commit `2bf9093`) rolled 12 plan items
into a single doc-only commit; see the commit message for the per-section
delta. Spec §0.1 adds a "规范版本约定" paragraph establishing the
`major.minor` notation; historical `[v0.7.0]` rows in §17.7 are
preserved as version snapshots.

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
