# writing-wlwl skill bundle

Claude Skills-format bundle for authoring **WLWL v0.10** `.wll` sources
(v0.10.1 implementation; the spec version is still **v0.10**).

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

This skill targets **wlwl-spec-v0.11** (file at
`../docs/spec/wlwl-spec-v0.11.md`).

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
- **v0.9** (archived) contributed:
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
- **v0.10** added **static contracts, all default-off** (this bundle tracks
  the v0.10.1+ implementation of it).
  Its runtime is identical to v0.9's; nothing below changes behaviour unless a
  `wlwl.toml` opts in.
  - **Type annotations enter the spec** — `LET(x: INTEGER, 1)`,
    `FUN((a: INTEGER, b: INTEGER) : INTEGER, +(a, b))`.
  - **Container/function types** — `ARRAY[T]`, `DICT[K,V]`, `OPTION[T]`
    (annotation sugar for `RESULT[T, NULL]`), `RESULT[T,E]`, `FUN`.
  - **Bounded type variables** — `FUN((a: T: Comparable, b: T: Comparable) :
    T: Comparable, …)`; erased at runtime, so `Value` is unchanged.
  - **`[features] gradual_typing`** = `off` (default) / `warn` / `error`, plus
    `match_exhaustiveness` for the `MATCH` checks.
  - **Module contracts** — `foo.wll.sig` sidecar signatures and
    `SEALED([...])` public faces; both absent by default.
  - **Error codes** — `E0110`–`E0116` / `W0110`–`W0117`, all compile-time only;
    `W0117` deliberately has no `E0117`.
  - **New CLI** — `wlwl sig`, `wlwl sig-gen`, `wlwl interface`, `wlwl schema`,
    `wlwl lsp`.

Superseded specs (v0.6–v0.10) are archived in condensed form in
`../docs/history/20260902-09.md` / `../docs/history/20260915-22.md`
(full text via git history; each version additive on the previous;
v0.11 — the renumbered v0.10 "v2 cleaned" edition, semantics unchanged —
is the current normative source).

Compiler version is independent of the spec version (see root
`CHANGELOG.md`). `wlwl run` is always the source of truth.

## Contents

| File | Use |
|---|---|
| `SKILL.md` | Writing flow, static contracts (§2.6/§5.2/§9.1/§9.6), concurrency hard rules, antipatterns (24 rows: v0.6–v0.9) |
| `reference.md` | Operators, type/`TYPE` names, error codes (incl. `E0064` / `W0001` / `E0010`–`E0025`), §8.3 consumers (13 global), OOP (§14–§16), concurrency matrix (§21), v0.9 增量备忘 (§23), v0.10 增量备忘 (§24) |
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
| `examples/static_contracts.wll` | v0.10 §5.2 type annotations, container types, `T: Comparable` |

Run any example:

```bash
wlwl run wlwl-skill/examples/oop.wll
```

## Scope

- **In scope**: writing and reviewing `.wll` programs against
  wlwl-spec-v0.11 — v0.6 core + v0.7 §17 concurrency + v0.8 clarifications
  + v0.9 true-suspension / OOP / session types / linear THIS
  + v0.10 static contracts (annotations, module signatures, MATCH checks)
  carried over unchanged into v0.11.
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

## Static contracts quick rules (v0.10 §2.6 / §5.2 / §5.2.1 / §9.1 / §9.6)

Everything here is **default-off** — the runtime is v0.9's until a `wlwl.toml`
opts in.

1. Annotations go on the `LET` binding and on each `FUN` parameter, with the
   return type after the closing paren:
   `LET(f, FUN((a: INTEGER, b: INTEGER) : INTEGER, +(a, b)));`
2. **Square brackets only.** `ARRAY[T]` / `DICT[K,V]` / `OPTION[T]` /
   `RESULT[T,E]`. The angle-bracket and arrow forms are parse errors
   (`E0011` / `E0012`) — note the spec still describes them as silent no-ops,
   which is stale.
3. `OPTION[T]` is annotation sugar for `RESULT[T, NULL]`; there is no runtime
   `OPTION` type.
4. Type variables take an explicit bound: `T: Comparable`, resolved at the
   call site, erased at runtime. Constraining a concrete type
   (`ARRAY[INTEGER]: Comparable`) is `E0010` (the spec says `E0012` — stale).
5. Turn the gate on in `wlwl.toml`:
   `[features]` `gradual_typing = "off" | "warn" | "error"`, plus
   `match_exhaustiveness`.
6. `E0110`–`E0116` / `W0110`–`W0117` are **compile-time only** — never `ERR`,
   so no §8.3 consumer can catch them. `W0117` has no `E0117` by design.
7. Library modules can publish a `foo.wll.sig` sidecar or a `SEALED([...])`
   face. Without either, module loading is exactly as in v0.9.

## Maintaining the skill

- When the **spec** gains a section, update `SKILL.md` + `reference.md`
  and add an `examples/` miniature; record in `CHANGELOG.md`.
- When the **implementation** deviates, cite the version's deviation
  register under `../docs/history/` — do not invent fixes here.
- Keep gold examples runnable: every `examples/*.wll` must `wlwl run`
  with exit 0.

## See also

- Language spec: `../docs/spec/wlwl-spec-v0.11.md`
- Spec archives (condensed): `../docs/history/20260902-09.md`,
  `../docs/history/20260915-22.md`
- Builtin registry (Appendix G): `../docs/appendix_G.md`
- Concurrency fixtures: `../impl/tests/concurrency/`
