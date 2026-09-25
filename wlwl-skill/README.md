# writing-wlwl skill bundle

Claude Skills-format bundle for authoring **WLWL v0.9** `.wll` sources.

```
wlwl-skill/
|-- SKILL.md          <- skill entry (frontmatter + writing guide)
|-- reference.md      <- lookup tables (ops, errors, OOP, concurrency, v0.9 备忘)
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

This skill targets **wlwl-spec-v0.9** (file at
`../docs/standard/wlwl-spec-v0.9.md`).

- **v0.6 core** (truthiness overhaul, `&&/||` short-circuit, `IF` ERR-routing,
  `!` canonical, `AT_K` rename, string subscript, `LET MUT`, overflow→`E0035`,
  `${...}` interpolation, `RESULT`/`OK`/`ERR_PAYLOAD`) — unchanged; still
  required knowledge.
- **v0.7 §17** adds structured concurrency + channels
  (`SCOPE` / `SPAWN` / `AWAIT` / `YIELD` / `TASK_*` / `SHIELD` / `CHANNEL_*`).
- **v0.8** is clarification / alignment over v0.7 (no breaking observable
  behaviour): `EXPECT_ERR` consumer, `=` triple-identity, `LET`/`FUN`
  asymmetry, `SUB` length semantics, float exponents, `MUT` as identifier,
  string-literal subscript, and related prose fixes.
- **v0.9** is the current standard:
  - **True-suspension concurrency** — `YIELD` legal at any expression
    position inside task bodies; blocking `CHANNEL_SEND`/`RECV` suspend
    (no `ChannelWouldBlock`).
  - **Structured cancellation** — `TASK_CANCEL(task, reason?)` carries a
    DICT reason; `AWAIT` payload includes `reason`; non-DICT → `E0066`.
  - **Deadlock detection L1** — `E0065` (strict) / `W0065` + `E0053` (soft).
  - **OOP §13–§16** — `CLASS` / `NEW` / `THIS` / `GET_PROP` / `SET_PROP` /
    `CALL_METHOD` with session-type protocols (sequence + ⊕ + μ) and
    linear `THIS`.
  - **Types** `CLASS` / `INSTANCE`; object-identity equality.
  - **Error codes** — `E0055`/`E0057` removed; `E0065`/`E0066`/`W0065`/`W0066`
    added; `E0014`/`E0032`/`E0050`/`E0051` redefined.

Spec archives: `../docs/history/wlwl-spec-v0.6.md`,
`../docs/history/wlwl-spec-v0.7.md`, `../docs/history/wlwl-spec-v0.8.md`
(each version additive on the previous; v0.9 is the current normative
source).

Compiler version is independent of the spec version (see root
`CHANGELOG.md`). `wlwl run` is always the source of truth.

## Contents

| File | Use |
|---|---|
| `SKILL.md` | Writing flow, antipatterns (24 rows: v0.6–v0.9) |
| `reference.md` | Operators, type/`TYPE` names, error codes, §8.3 consumers (14), OOP (§14–§16), concurrency matrix (§21), v0.9 增量备忘 (§23) |
| `interp.wll` | String-interpolation gold example |
| `examples/truthiness.wll` | §2.3 falsy table |
| `examples/control_flow.wll` | `IF` / `WHILE` / `FOR` / `MATCH` |
| `examples/error_propagation.wll` | §8.2 + consumers |
| `examples/match.wll` | pattern clauses |
| `examples/interpolation.wll` | `${...}` forms |
| `examples/import_stdlib.wll` | `wlwl:std.*` imports |
| `examples/concurrency.wll` | §17 SCOPE / SPAWN / AWAIT / YIELD + channel fan-in |
| `examples/concurrency_cancel.wll` | SHIELD / cancel with reason / `ChannelClosed` kind |
| `examples/oop.wll` | §13–§15 CLASS / NEW / methods / protocol / linear THIS |

Run any example:

```bash
wlwl run wlwl-skill/examples/oop.wll
```

## Scope

- **In scope**: writing and reviewing `.wll` programs against
  wlwl-spec-v0.9 — v0.6 core + v0.7 §17 concurrency + v0.8 clarifications
  + v0.9 true-suspension / OOP / session types / linear THIS.
- **Out of scope**: the Rust implementation (`impl/`), formatter design,
  ADRs, release engineering. For those, read the repo docs directly.

## Concurrency quick rules (v0.9 §17)

1. Every `SPAWN` must live inside `SCOPE` → else `E0058`.
2. `YIELD()` is legal at **any** expression position inside a task body
   (v0.9 true suspension). Outside a task → `E0014`.
3. Close signal = `ERR(kind="ChannelClosed")`, never `NULL`.
4. Blocking `CHANNEL_SEND` / `RECV` **suspend** when full/empty with no
   peer. For non-blocking poll semantics use `CHANNEL_TRY_*`.
5. Cancel is cooperative and structured: `TASK_CANCEL(h, ["why": "…"])`;
   `AWAIT` of a cancelled task yields `ERR(kind="Cancelled", reason: …)`.
6. Deadlock L1: 2+ tasks parked on channel ops in one SCOPE with no peer
   → `E0065` (or `W0065` + `E0053` when `strict_deadlock_detect = false`).

## OOP quick rules (v0.9 §13–§16)

1. `CLASS(name, parent, members)` — `members` is an ARRAY of
   `[STRING, value]` pairs. First param named `self` = method.
2. `NEW(cls, args...)` runs `init` with `self` injected; arity must match.
3. `THIS()` is **linear**: once per method call; never escape the body.
4. Declare a session protocol in `CLASS`'s second arg to enforce call
   order; violations raise `E0051` / `E0050`.

## Maintaining the skill

- When the **spec** gains a section, update `SKILL.md` + `reference.md`
  and add an `examples/` miniature; record in `CHANGELOG.md`.
- When the **implementation** deviates, cite the version's deviation
  register under `../docs/history/` — do not invent fixes here.
- Keep gold examples runnable: every `examples/*.wll` must `wlwl run`
  with exit 0.

## See also

- Language spec: `../docs/standard/wlwl-spec-v0.9.md`
- Spec archives: `../docs/history/wlwl-spec-v0.8.md`,
  `../docs/history/wlwl-spec-v0.7.md`, `../docs/history/wlwl-spec-v0.6.md`
- Builtin registry (Appendix G): `../docs/appendix_G.md`
- Concurrency fixtures: `../impl/tests/concurrency/`
