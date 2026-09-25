# wlwl-skill CHANGELOG

Skill-bundle changes (spec lives at `../docs/standard/wlwl-spec-v0.9.md`
and is authoritative).

## [0.9.0] — 2026-09-25

### Changed

- **Target spec** bumped from `wlwl-spec-v0.8.md` (archived at
  `docs/history/`) to **`wlwl-spec-v0.9.md`** (current). v0.9 is a
  *language upgrade* over v0.8: true-suspension concurrency, structured
  cancellation, deadlock L1, and OOP with session-type protocols.
- **SKILL.md description + title** rewritten: "Writing WLWL (v0.9)"
  covers v0.9 concurrency semantics, OOP (§13–§16), linear THIS, and
  the error-code delta. Frontmatter description lists the v0.9 surface.
- **`wlwl-spec-v0.8.md` references** updated throughout SKILL.md /
  reference.md / README.md to point at v0.9; `docs/history/` archive
  paths preserved.
- **Concurrency section rewritten** (SKILL.md + reference.md §21):
  - `YIELD()` legal at **any** expression position inside a task body
    (v0.7/v0.8 "Block direct child" restriction removed).
  - Blocking `CHANNEL_SEND` / `RECV` **suspend** when full/empty with
    no peer; `ERR(ChannelWouldBlock)` path **removed**.
  - `TASK_CANCEL(task, reason?)` / `TASK_CANCEL_PARENT(reason?)` carry
    an optional DICT reason (default `{}`); non-DICT → `E0066`.
  - `AWAIT` of a cancelled task returns
    `ERR(kind="Cancelled", reason: <dict>)`.
  - Close wakes parked receivers with `ERR(ChannelClosed)` and parked
    senders with `E0054`.
- **OOP section added** (SKILL.md + reference.md §14–§16):
  `CLASS` / `NEW` / `GET_PROP` / `SET_PROP` / `CALL_METHOD` / `THIS`;
  session-type protocols (sequence + ⊕ + μ); linear `THIS` discipline.
- **Error-code table** (reference.md §10): `E0055` / `E0057` marked
  removed; `E0065` / `E0066` / `W0065` / `W0066` added; `E0014` /
  `E0032` / `E0050` / `E0051` redefined per v0.9 §11.2.
- **`wlwl.toml` features** (reference.md §19): added
  `strict_deadlock_detect`, `native_channel_close`,
  `channel_large_buf_threshold`.
- **Anti-patterns expanded to 24 rows**: new rows cover linear `THIS`
  escapes, external `SET_PROP`, protocol-order violations, non-DICT
  cancel reason, `ChannelWouldBlock` expectation, and deadlocks.

### Added

- **`examples/oop.wll`** — CLASS / NEW / methods with `self` injection,
  session-protocol sequence, and linear-`THIS` usage.
- **`reference.md` §23 v0.9 增量备忘** — compact index of every v0.9
  item affecting `.wll` writing (concurrency, OOP, error codes, types,
  features).
- **Types `CLASS` / `INSTANCE`** in the type/display/equality tables
  (reference.md §1, §11, §21).
- **`MODULE_REF(path)`** documented (reference.md §7) — loads module
  dict without binding names.

### Verified

- All `examples/*.wll` and `interp.wll` run with exit 0 against the
  v0.9 reference implementation.

---

## [0.8.1] — 2026-09-23

### Changed

- **Target spec** bumped from `wlwl-spec-v0.7.md` (archived at
  `docs/history/`) to **`wlwl-spec-v0.8.md`** (current). v0.8 is a
  *clarification / documentation / alignment* patch over v0.7 — v0.7
  core + §17 concurrency still required knowledge.
- **SKILL.md description + title** rewritten: "Writing WLWL (v0.8.1)"
  covers v0.8 字面增改 + v0.8.1 patch 修复. Frontmatter description
  lists every v0.8 字面增改 (12 items) and v0.8.1 patch 修复 (5 items).
- **`wlwl-spec-v0.7.md` references** updated throughout SKILL.md /
  reference.md / README.md to point at v0.8; `docs/history/` archive path
  preserved.
- **§12 reserved forms** rewritten in SKILL.md and reference.md:
  `CLASS` / `NEW` / `THIS` / `MODULE` / `MODULE_REF` / `CALL` /
  `ARRAY(items...)` / `AND` / `OR` are NOT reserved — all have working
  `BuiltinSpec` records in `BUILTIN_REGISTRY`. The registry
  (`docs/appendix_G.md`) is the single source of truth; section §12 now
  points to it. Audit-driven (see `docs/history/audit-report-v0.8.1.md`
  §3.2 + spec §12 rewrite D8-005).

### Added

- **4 new anti-patterns (rows 16–19)**, audit §8.2 reproducer-driven:
  - **#16** Builtin name as value (`LET(f, +)` → E0020; `LET(f, PRINT)` → E0020; spec §4.3 op-tokens + §5.4 user-defined function only)
  - **#17** Float integer overflow / divide-by-zero NOT catchable (these are native codes E0035 / E1003 per §11.2 — not ERR; `EXPECT_ERR(/(1, 0))` does NOT save; spec §11.2 + §2.2)
  - **#18** Style guide rebalance: SUB 3rd arg is length, not end-index (v0.8.1 D8-010 observable change; spec §10.5; `SUB("Hello", 7, 5)` now returns `"world"`)
  - **#19** Float exponent literals work in v0.8.1 (`1.5e2`, `1e-3`, `1E3` per spec §1.7 EBNF `digits exponent`; pre-D8-01 raised E0011).
- **`YIELD()` placement note** softened: v0.7 / v0.8 spec §17.1 frames this
  as "理想语义 + 实现路径" (static task-body segmentation in
  `yield_split.rs`), not a hard rule. Future suspension-based schedulers
  may lift the limitation without breaking change (D8-007).
- **`NOT(ERR(x))` note**: spec §4.3 prose corrected (D8-006) — all
  operators including NOT propagate ERR transparently. Safe idiom:
  `NOT(BOOL(ERR(x)))` for safe coercion.
- **String literal subscript** (§4.5 prose + §A.2 grammar, v0.8.1
  D8-012): `"hi"[0]` parses and evaluates to `"h"`. INT / FLOAT /
  Boolean / NULL literal postfix remains rejected at parser (no
  `INDEX_GET` semantics).
- **`MUT` as binding name** (§1.4, v0.8.1 D8-011): `LET(MUT, "x")` is
  valid; MUT is a context keyword only after LET-modifier slot.
- **`=` triple-identity** (§1.5, D8-008): `=` is `==` in call position
  AND a separator for INDEX_SET sugar AND for default-param — three
  roles, no lookahead needed.
- **`%` float E0030** (§2.2 + §11.2, D8-008): `%` with any FLOAT arg
  raises E0030 (was previously listed in only spec prose; E0030 row
  updated).
- **`EXPECT_ERR`** (§8.3, D8-008): added to the §8.3 consumer table
  (was in prose only — explicit table entry for lookup).
- **`SHIELD` / `SCOPE(ERR)` / `AWAIT` host diagnostic clarifications**
  (§17.1 / §17.5, D8-008): user ERR vs host diagnostic now distinct;
  `SCOPE(ERR("x"))` propagates transparently (E0052 not fired).
- **`TASK` name-collision** (§2.1 / §10.11, D8-008): type name `TASK`
  vs `wlwl:std.agent.TASK` / `wlwl:std.ai.TASK` — same name, no
  conflict; recommended full-qualified write style.

### Verified

- All `examples/*.wll` in the bundle still parse under v0.8.1 spec
  (mechanical re-check; no source change to examples required).
- `interp.wll` (gold) extends with a `SUB` length example
  (`SUB("Hello, world", 7, 5)` → `"world"`) demonstrating D8-010 fix.

---

## [0.7.0] — 2026-09-22

### Added

- **Concurrency (spec §17)** section in `SKILL.md`: SCOPE / SPAWN /
  AWAIT / YIELD / TASK_* / SHIELD / CHANNEL_* with hard rules
  (explicit SCOPE, YIELD placement, ChannelClosed kind, TRY_* preference).
- **5 new antipatterns** (rows 11–15): top-level SPAWN, bare-branch
  YIELD, NULL-as-close, non-suspending SEND/RECV, unhandled Cancelled
  AWAIT.
- `examples/concurrency.wll` — SCOPE + two SPAWN + YIELD + channel
  fan-in (TRY_*), self-printing PASS lines.
- `examples/concurrency_cancel.wll` — TASK_CANCEL_PARENT inside SHIELD,
  deferred `TASK_IS_CANCELLED`, ChannelClosed kind check.
- `reference.md` §12 concurrency reference: builtin table, ERR kinds,
  `E0052`–`E0058`, type names `TASK`/`CHANNEL`, equality/display notes.

### Changed

- Target spec **wlwl-spec-v0.7** (additive over v0.6). Authoritative
  path: `../docs/standard/wlwl-spec-v0.7.md`; v0.6 archived at
  `../docs/history/wlwl-spec-v0.6.md`.
- Deviations pointer → `../docs/history/deviations-v0.7.md`
  (former `docs/plan/deviations.md`).
- `SKILL.md` description + title cover v0.7 concurrency; writing flow
  checklist gains a concurrency step.
- Equality note: `==(f, f)` / `==(h, h)` are TRUE (handle identity).

## [0.6.1] — 2026-09-20

### Changed

- Extension notes for `.wll` (was `.wl`).
- Authoritative spec path → `../docs/standard/wlwl-spec-v0.6.md`.

### Known impl/spec deviations (not skill bugs)

- Integer overflow: spec says `ERR(E0035)` consumable via `UNWRAP_OR`;
  current impl surfaces a runtime diagnostic (exit 1). Recorded in
  `docs/history/deviations-v0.7.md`.
- Captured-`LET` upgrade (legacy E-CloCap) vs §3.3 strict reading —
  prefer `LET MUT` for shared mutation (skill antipattern #1).

## [0.6.0] — 2026-09-20

### Added

- Nine v0.6 decisions documented (truthy overhaul, short-circuit,
  IF-ERR routing, `!` canonical, `AT_K`, string subscript, `LET MUT`,
  overflow `E0035`, `${}` interpolation).
- Gold examples through `N` blocks in `interp.wll`.
