# writing-wlwl skill bundle

Claude Skills-format bundle for authoring **WLWL v0.7** `.wll` sources.

```
wlwl-skill/
|-- SKILL.md          <- skill entry (frontmatter + writing guide)
|-- reference.md      <- lookup tables (ops, errors, concurrency)
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

This skill targets **wlwl-spec-v0.7** (file at
`../docs/standard/wlwl-spec-v0.7.md`).

- **v0.6 core** (truthiness, `LET MUT`, ERR model, modules) is unchanged —
  still required knowledge.
- **v0.7 §17** adds structured concurrency + channels
  (`SCOPE` / `SPAWN` / `AWAIT` / `YIELD` / `TASK_*` / `SHIELD` / `CHANNEL_*`).
- v0.6 text is archived at `../docs/history/wlwl-spec-v0.6.md` (additive
  relationship; non-concurrent programs behave the same).

Compiler version is independent of the spec version (see root
`CHANGELOG.md`). `wlwl run` is always the source of truth.

## Contents

| File | Use |
|---|---|
| `SKILL.md` | Writing flow, antipatterns (now 15 rows, concurrency included) |
| `reference.md` | Operators, type/`TYPE` names, error codes (`E0052`–`E0058`), concurrency matrix |
| `interp.wll` | String-interpolation gold example |
| `examples/truthiness.wll` | §2.3 falsy table |
| `examples/control_flow.wll` | `IF` / `WHILE` / `FOR` / `MATCH` |
| `examples/error_propagation.wll` | §8.2 + consumers |
| `examples/match.wll` | pattern clauses |
| `examples/interpolation.wll` | `${...}` forms |
| `examples/import_stdlib.wll` | `wlwl:std.*` imports |
| `examples/concurrency.wll` | **v0.7** SCOPE/SPAWN/AWAIT/YIELD + channel fan-in |
| `examples/concurrency_cancel.wll` | **v0.7** SHIELD / cancel / ChannelClosed kind |
| `examples/concurrency.wll` | **new** §17 SCOPE/SPAWN/AWAIT/YIELD + channel TRY_* |
| `examples/concurrency_cancel.wll` | **new** TASK_CANCEL / SHIELD / ChannelClosed kind |

Run any example:

```bash
wlwl run wlwl-skill/examples/concurrency.wll
```

## Scope

- **In scope**: writing and reviewing `.wll` programs against
  wlwl-spec-v0.7 (v0.6 core + §17 concurrency).
- **Out of scope**: the Rust implementation (`impl/`), formatter design,
  ADRs, release engineering. For those, read the repo docs directly.

## Concurrency quick rules (v0.7)

1. Every `SPAWN` must live inside `SCOPE` → else `E0058`.
2. `YIELD()` only as a direct child of a multi-statement block.
3. Close signal = `ERR(kind="ChannelClosed")`, never `NULL`.
4. v0.7.0: `CHANNEL_SEND`/`RECV` do not suspend — use `CHANNEL_TRY_*`.
5. Cancel is advisory; `SHIELD` defers it but does not swallow `ERR`.

## Maintaining the skill

- When the **spec** gains a section, update `SKILL.md` + `reference.md`
  and add an `examples/` miniature; record in `CHANGELOG.md`.
- When the **implementation** deviates, cite
  `../docs/history/deviations-v0.7.md` — do not invent fixes here.
- Keep gold examples runnable: every `examples/*.wll` must `wlwl run`
  with exit 0.

## See also

- Language spec: `../docs/standard/wlwl-spec-v0.7.md`
- Builtin registry (Appendix G): `../docs/appendix_G.md`
- Concurrency fixtures: `../impl/tests/concurrency/`
- Spec-vs-impl register: `../docs/history/deviations-v0.7.md`
