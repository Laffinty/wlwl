# writing-wlwl skill bundle

Claude Skills-format bundle for authoring **WLWL v0.8.1** `.wll` sources.

```
wlwl-skill/
|-- SKILL.md          <- skill entry (frontmatter + writing guide)
|-- reference.md      <- lookup tables (ops, errors, concurrency, v0.8+v0.8.1 备忘)
|-- interp.wll        <- gold-standard interpolation example
|-- examples/         <- runnable miniatures
|-- README.md         <- this file
`-- CHANGELOG.md      <- skill-bundle changes
```

## Install

Copy or symlink this folder into your agent skills directory, e.g.:

```bash
cp -r wlwl-skill ~/.claude/skills/writing-wlwl
# or
ln -s "$PWD/wlwl-skill" ~/.claude/skills/writing-wlwl
```

The skill is loaded when the task matches the `description` in
`SKILL.md` (writing / reviewing `.wll` against the language spec).

## Target version

This skill targets **wlwl-spec-v0.8** (file at
`../docs/standard/wlwl-spec-v0.8.md`); v0.8.1 is the current **patch
release** on top of v0.8 — it changes six impl/deviation items
documented in `../docs/history/audit-report-v0.8.1.md` and
`../docs/history/deviations-v0.8.md` (D8-009..D8-014) but ships **zero
spec text changes** (the v0.8 spec stays normative).

- **v0.6 core** (truthiness overhaul, `&&/||` short-circuit, `IF` ERR-routing,
  `!` canonical, `AT_K` rename, string subscript, `LET MUT`, overflow→`E0035`,
  `${...}` interpolation, `RESULT`/`OK`/`ERR_PAYLOAD`) — unchanged; still
  required knowledge.
- **v0.7 §17** adds structured concurrency + channels
  (`SCOPE` / `SPAWN` / `AWAIT` / `YIELD` / `TASK_*` / `SHIELD` / `CHANNEL_*`).
- **v0.8** is **clarification / alignment over v0.7** with no breaking
  observable behaviour: §12 reserved forms rewritten to point at
  `BUILTIN_REGISTRY`; `EXPECT_ERR` added as a §8.3 consumer; `%` (float `E0030`)
  listed in the strict-types trigger set; `SHIELD` / `SCOPE(ERR)` / `AWAIT`
  host-diagnostic clarifications; `=` triple-identity disambiguation;
  `LET`/`FUN` asymmetry normative; §2.1 / §10.11 `TASK` same-name
  normative; §17.1 `YIELD` placement demoted to prose; §4.3 `NOT`
  transparency prose reversed; §1.6 splits `int_lit` / `int_literal`.
- **v0.8.1 patch** ships six impl/deviation fixes: float exponent
  literals (§1.7), `SUB` third arg now length not end-index (§10.5),
  `MUT` usable as ordinary identifier in five dispatch sites (§1.4),
  string-literal subscript (§A.2), `closure_cell.wll` example
  realigned (§3.3), and a deviation note on nested-string in
  interpolation (§1.8). See §20 of `reference.md` for the full table.

Spec archives: `../docs/history/wlwl-spec-v0.6.md`,
`../docs/history/wlwl-spec-v0.7.md` (each version additive on the
previous; v0.8 is the current normative source).

Compiler version is independent of the spec version (see root
`CHANGELOG.md`). `wlwl run` is always the source of truth.

## Contents

| File | Use |
|---|---|
| `SKILL.md` | Writing flow, antipatterns (now **19 rows**: v0.6/v0.7 core + v0.8 clarification + v0.8.1 patch) |
| `reference.md` | Operators, type/`TYPE` names, error codes, §8.3 consumers (14 incl. `EXPECT_ERR`), concurrency matrix, §20 v0.8+v0.8.1 字面增改备忘 |
| `interp.wll` | String-interpolation gold example |
| `examples/truthiness.wll` | §2.3 falsy table |
| `examples/control_flow.wll` | `IF` / `WHILE` / `FOR` / `MATCH` |
| `examples/error_propagation.wll` | §8.2 + consumers |
| `examples/match.wll` | pattern clauses |
| `examples/interpolation.wll` | `${...}` forms |
| `examples/import_stdlib.wll` | `wlwl:std.*` imports |
| `examples/concurrency.wll` | §17 SCOPE / SPAWN / AWAIT / YIELD + channel fan-in |
| `examples/concurrency_cancel.wll` | SHIELD / cancel / `ChannelClosed` kind |

Run any example:

```bash
wlwl run wlwl-skill/examples/concurrency.wll
```

## Scope

- **In scope**: writing and reviewing `.wll` programs against
  wlwl-spec-v0.8 (with v0.8.1 patch fixes folded in) — v0.6 core +
  v0.7 §17 concurrency + v0.8 clarifications.
- **Out of scope**: the Rust implementation (`impl/`), formatter design,
  ADRs, release engineering. For those, read the repo docs directly.

## Concurrency quick rules (v0.7 §17, unchanged in v0.8 / v0.8.1)

1. Every `SPAWN` must live inside `SCOPE` → else `E0058`.
2. `YIELD()` only as a direct child of a multi-statement block.
3. Close signal = `ERR(kind="ChannelClosed")`, never `NULL`.
4. v0.7.0: `CHANNEL_SEND` / `RECV` do not suspend — use
   `CHANNEL_TRY_SEND` / `CHANNEL_TRY_RECV`.
5. Cancel is advisory; `SHIELD` defers it but does not swallow `ERR`.

## Maintaining the skill

- When the **spec** gains a section, update `SKILL.md` + `reference.md`
  and add an `examples/` miniature; record in `CHANGELOG.md`.
- When the **implementation** deviates, cite
  `../docs/history/deviations-v0.8.md` (or the version file whose
  `D8-NNN` numbering the deviation was registered under) — do not
  invent fixes here.
- v0.8.1 is a **patch** release: spec text is unchanged, but six
  impl/deviation items landed; the §20 table in `reference.md` is the
  compact index, and `../docs/history/audit-report-v0.8.1.md` is the
  full cycle report.
- Keep gold examples runnable: every `examples/*.wll` must `wlwl run`
  with exit 0.

## See also

- Language spec: `../docs/standard/wlwl-spec-v0.8.md`
- Spec archives: `../docs/history/wlwl-spec-v0.7.md`,
  `../docs/history/wlwl-spec-v0.6.md`
- Builtin registry (Appendix G): `../docs/appendix_G.md`
- v0.8.1 cycle audit report: `../docs/history/audit-report-v0.8.1.md`
- Spec-vs-impl register: `../docs/history/deviations-v0.8.md`
- Concurrency fixtures: `../impl/tests/concurrency/`