# wlwl-skill CHANGELOG

Skill-bundle changes (spec lives at `../docs/standard/wlwl-spec-v0.7.md`
and is authoritative).

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
