# WLWL v0.7 Reference (on-demand)

> Loaded only when `SKILL.md` points here. This is a lookup catalogue,
> not a tutorial. Section numbers (`§1.5`, `§8.3`, `§10.4`, `§17`, etc.)
> refer to `docs/standard/wlwl-spec-v0.7.md` (v0.6 archived at
> `docs/history/wlwl-spec-v0.6.md`; v0.7 is additive over v0.6).

## Contents

- [§1. Truthy / falsy (§2.3)](#1-truthy--falsy-23)
- [§2. v0.6 decisions (full table)](#2-v06-decisions-full-table)
- [§3. AST shapes (§A.2)](#3-ast-shapes-a2)
- [§4. Lexer traps (§1)](#4-lexer-traps-1)
- [§5. Control flow (§6)](#5-control-flow-6)
- [§6. Pattern matching (§7)](#6-pattern-matching-7)
- [§7. Imports (§9.2, §9.3)](#7-imports-92-93)
- [§8. Error model (§8)](#8-error-model-8)
- [§9. Standard library catalogue (§10)](#9-standard-library-catalogue-10)
- [§10. Error codes (§11.2, §11.3)](#10-error-codes-112-113)
- [§11. Display / STR quirks (§2.5)](#11-display--str-quirks-25)
- [§12. INDEX vs `xs[i]` (§10.4, §4.5)](#12-index-vs-xsi-104-45)
- [§13. Container immutability (§3.3, §10.4)](#13-container-immutability-33-104)
- [§14. Reserved forms (§12)](#14-reserved-forms-12)
- [§15. `--format json|jsonl` diagnostics (§11.1)](#15---format-jsonjsonl-diagnostics-111)
- [§16. `EXPORT` / `wlwl.toml` features (§9.1, §9.4)](#16-export--wlwltoml-features-91-94)
- [§17. Anti-patterns (extended)](#17-anti-patterns-extended)
- [§18. Concurrency (§17)](#18-concurrency-17)
- [§19. Notes on `interp.wll`](#19-notes-on-interpwll)

---

## §1. Truthy / falsy (§2.3)

Exactly **eight** falsy values:

| Value | Type |
|-------|------|
| `FALSE` | BOOLEAN |
| `NULL` | NULL |
| `0` | INTEGER |
| `0.0` | FLOAT |
| `""` | empty STRING |
| `[]` | empty ARRAY |
| `DICT()` | empty DICT (only constructor; `[:]` is not parseable) |
| `NaN` | FLOAT (unreachable in current impl — `0/0` raises `E1003`) |

Everything else is truthy, including `OK(FALSE)`, non-empty strings,
non-empty containers whose elements are falsy (e.g. `[0]` is truthy
because the array is non-empty), and v0.7 `TASK` / `CHANNEL` handles.

**`ERR(...)` is NOT in this list.** It is its own propagation mechanism per §8.2 — passing `ERR(x)` to a non-consumer function forwards the ERR; it does not cause the function to evaluate as if its argument were falsy. See §8 for the consumer registry.

## §2. v0.6 decisions (full table)

| # | v0.5 → v0.6 | Spec | Code-level effect |
|---|-------------|------|-------------------|
| A | Truthy overhaul | §B.1 | Falsy = 8 values listed above; everything else truthy |
| B | `&&` / `\|\|` short-circuit | §B.2 | Right operand evaluated only when left cannot decide |
| C | `IF(cond, then, else)` catches `ERR` | §B.3 | ERR on `then` path routes to `else`; without `else` → `NULL` silently |
| D | `!` and `NOT` both canonical | §B.4 | `!(FALSE)` and `NOT(FALSE)` both yield `TRUE`; no warning |
| E | `POP` → `AT_K` rename | §B.5 | `AT_K(d, k, default)` is canonical; `POP` is a `ResolvedCompat` alias |
| F | String subscript read | §B.6 | `s[i]` returns single-codepoint STRING; OOB raises `E0036` (not `NIL`/`""`) |
| G | Explicit `LET MUT` | §B.7 | `LET MUT(x, v)` for mutable; `SET` on plain `LET` raises `E0024` |
| H | Integer overflow → `ERR(E0035)` | §B.8 | Was saturate + `W0015` in v0.5. Note: current impl surfaces as runtime diagnostic; check `docs/history/deviations-v0.7.md` |
| J | `${expr}` interpolation | §B.9 | One `StrStart/StrEnd` pair per literal regardless of segment count; nested string literal inside `${...}` is an error (`E0001`) |

**v0.7 is additive** (spec §17 + Appendix G): new types `TASK`/`CHANNEL`,
17 concurrent builtins, error codes `E0052`–`E0058`, ERR kinds
`ChannelClosed` / `ChannelWouldBlock` / `Cancelled`. §0–§12 of v0.6 are
unchanged.

## §3. AST shapes (§A.2)

Spec Appendix A.2 enumerates these node kinds; the v0.6 ones are:

```rust
// Expr::Let carries an optional mut_ flag (decision G).
Expr::Let { name, value, mut_: bool }

// Literal splits into plain vs interpolated (decision J).
Literal::Str(String)                         // no interpolation
Literal::Interpolated(Vec<StrPart>)          // has ${...} segments

enum StrPart {
    Text(String),                            // raw chars between ${...}
    Expr(Box<Expr>),                         // the ${...} payload
}

// New in v0.6 (not in v0.5):
//   mut_ on Let, Interpolated literal, StrPart
```

v0.7 adds **no** new AST node kinds — concurrency is all ordinary
`Call` nodes (`SCOPE`, `SPAWN`, …).

## §4. Lexer traps (§1)

- `//` line comments and `/* */` block comments.
- `MUT` is a **contextual** keyword only between `LET` and `(`. Elsewhere it is a normal identifier.
- `${` starts interpolation inside `"..."`; nested string literals inside `${...}` are `E0001`.
- Escapes: `\$`, `\n`, `\t`, `\r`, `\\`, `\"`, `\/`, `\0`, `\b`, `\f`.
- `=` is the equality alias (not assignment). Assignment is `SET`.

## §5. Control flow (§6)

| Form | Notes |
|------|-------|
| `IF(cond, then)` / `IF(cond, then, else)` | 2-arg: else is `NULL`. 3-arg required for ERR catch. |
| `WHILE(cond, body)` | Value `NULL`. |
| `FOR(x, iterable, body)` | ARRAY / DICT / STRING. Value `NULL`. |
| `RETURN(v?)` / `BREAK()` / `CONTINUE()` | Illegal outside fn/loop → `E0014`. |

## §6. Pattern matching (§7)

`MATCH(value, clauses[, default])`. Patterns: literal, identifier, `_`,
`[a, *rest]`, `["k": v]`, `OK(p)` / `ERR(p)`. No hit and no default →
`NULL`. Destructuring mismatch in `LET` → `E0026`.

## §7. Imports (§9.2, §9.3)

```
IMPORT(path, ["a", "b"])
IMPORT(path, ["orig": "alias"])
IMPORT(path, [])
```

- `./` `../` relative; `wlwl:std.*` built-in namespaces.
- Errors: `E0040` missing, `E0041` cycle, `E0023` not exported, `E0021` duplicate.

v0.7 concurrency names are **global** (Appendix G) — never import them.

## §8. Error model (§8)

### §8.1 RESULT variants

`OK(v)` / `ERR(e)`; `e` must be STRING or DICT (`E0030` otherwise).

**v0.7 ERR kinds** (always dict payloads):

| `kind` | Source |
|--------|--------|
| `"ChannelClosed"` | RECV/TRY_RECV after close + drain |
| `"ChannelWouldBlock"` | SEND full / RECV empty (v0.7.0) |
| `"Cancelled"` | `AWAIT` of a cancelled task |

### §8.2 Transparent propagation

Non-consumer + ERR arg → body does not run; ERR forwarded. This
applies to **all** v0.7 concurrency builtins (they are not consumers).

### §8.3 Consumer registry

`IS_OK`, `IS_ERR`, `UNWRAP_OR`, `OR_DIE` (deprecated), `TRY`, `UNWRAP`,
`ERR_PAYLOAD`, `WRAP`, `TYPE`, `==`/`!=`, `IF` condition, `&&`/`||`
left, `BOOL`.

### §8.4 PANIC

`PANIC(msg)` / `UNWRAP(ERR)` / `NEG(i64::MIN)` → `E0100`.

### §8.5 Top-level escape

Uncaught ERR → `E0102`, exit 1.

## §9. Standard library catalogue (§10)

See `SKILL.md` pointers. Highlights: `wlwl:std.collection` (MAP/FILTER/…),
`std.json`, `std.fs`, `std.test`, `std.format`, `std.io`, `std.ai`.
Concurrent primitives are **global**, not under `wlwl:std.*`.

## §10. Error codes (§11.2, §11.3)

### Errors (E-codes) — v0.7 concurrent additions highlighted

| Code | Meaning |
|------|---------|
| `E0052` | **[v0.7]** SCOPE/SPAWN/SHIELD arg is not a function |
| `E0053` | **[v0.7]** invalid task/channel handle; TASK_CURRENT outside a task |
| `E0054` | **[v0.7]** SEND/TRY_SEND on closed channel |
| `E0055` | **[v0.7 reserved]** close-on-read (user sees `ERR(ChannelClosed)`) |
| `E0056` | **[v0.7]** SPAWN arity (including non-zero-param `fn`) |
| `E0057` | **[v0.7 reserved]** cross-task immutable cell (reuses `E0024`) |
| `E0058` | **[v0.7]** top-level SPAWN without SCOPE |
| `E0014` | illegal RETURN/BREAK/CONTINUE **or YIELD not in a Block** |
| `E0024` | SET on immutable binding |
| `E0030` | type error (incl. ERR payload not STRING/DICT) |
| `E0036` | array/string index OOB |
| `E0100` | PANIC |
| `E0102` | ERR escaped to top level |
| `E1003` | divide/mod by zero |

Full table: spec §11.2.

### Warnings (W-codes)

`W0010` unused LET · `W0011` unused param · `W0014` non-ASCII case ·
`W0020` mixed dict literal · `W0030` shadow · `W0040` TODO(agent) ·
`W0051` deprecated alias · `W0053` formatter drift.

### Exit codes (§11.4)

`0` ok · `1` diagnostic · `101` impl crash (bug).

## §11. Display / STR quirks (§2.5)

| Value | Display |
|-------|---------|
| bool | `TRUE` / `FALSE` |
| null | `NULL` |
| array | `[e1, e2]` |
| dict | `[k: v, ...]` |
| fun | `<fun(params)>` |
| **v0.7 task** | `<task handle id=N gen=M>` |
| **v0.7 channel** | `<channel handle id=N gen=M>` |
| float whole | one decimal (`2.0`) |

## §12. INDEX vs `xs[i]` (§10.4, §4.5)

`xs[i]` is subscript read; OOB → `E0036`. `INDEX(xs, v)` is search →
`-1` if missing (not an error). `INDEX_SET` rejects STRING receiver.

## §13. Container immutability (§3.3, §10.4)

`PUSH`/`INDEX_SET`/… return **new** containers. Mutate only via `SET` on
`LET MUT` bindings.

## §14. Reserved forms (§12)

`CLASS` / `NEW` / `THIS` / `MODULE` / `MODULE_REF` / `CALL` /
`ARRAY(...)` / `AND` / `OR` — reserved; calls raise `E0020` (or notes in
§12). Use `&&` / `||` for logic.

## §15. `--format json|jsonl` diagnostics (§11.1)

Machine-readable diagnostics: `error_schema_version`,
`errorCategory`, `retryable`, `suggestion_code`, `related`, `trace`.
JSONL = one diagnostic per line.

## §16. `EXPORT` / `wlwl.toml` features (§9.1, §9.4)

`EXPORT(name_array)` at top level. Manifest `[features]`:

| Feature | Default | Effect |
|---------|---------|--------|
| `strict_types` | `false` | call-boundary type checks (`E0033`) |
| `allow_builtin_shadow` | `false` | shadow builtins with `W0030` instead of `E0025` |

v0.7 concurrent names **are** builtins — shadowing them needs the feature flag.

## §17. Anti-patterns (extended)

Beyond the top-15 in `SKILL.md`:

- **Out-of-range array index returns `NIL`** — FALSE; it raises `E0036` (§4.5).
- **`IS_NIL(x)` exists** — FALSE; the literal is `NULL`.
- **`APPEND(arr, x)` is a builtin** — FALSE; the global is `PUSH` (§10.4).
- **`E0009` is divide-by-zero** — FALSE; it's `E1003` (§2.2).
- **`LIST(1, 2, 3)` is the literal** — FALSE; use `[1, 2, 3]` (§4.8).
- **`FORMAT("…{0}…", [x])` unpacks `x`** — FALSE; FORMAT is variadic positional.
- **`POP(d, k, default)` is canonical** — DEPRECATED; prefer `AT_K`.
- **Passing `ERR(x)` to a non-consumer lets the function decide** — FALSE; §8.2 forwards.

## §18. Concurrency (§17)

### Builtins (all global, **not** §8.3 consumers)

| Builtin | Signature | Notes |
|---------|-----------|-------|
| `SCOPE(fn)` | `-> v` | structured scope; nestable; `fn` must be 0-ary function |
| `SPAWN(fn)` | `-> TASK` | child task; **must** be inside SCOPE (`E0058`) |
| `AWAIT(task)` | `-> v` | terminal value; child ERR is a **value** |
| `YIELD()` | `-> NULL` | cooperative checkpoint |
| `TASK_CURRENT()` | `-> TASK` | outside task → `E0053` |
| `TASK_IS_CANCELLED()` | `-> BOOLEAN` | outside task → `FALSE` |
| `TASK_CANCEL(task)` | `-> NULL` | advisory; never raises |
| `TASK_CANCEL_PARENT()` | `-> NULL` | self + scope subtree |
| `SHIELD(fn)` | `-> v` | defer cancel; does **not** absorb `fn`'s ERR |
| `CHANNEL_NEW(buf)` | `-> CHANNEL` | `buf=0` = sync |
| `CHANNEL_SEND(ch, v)` | `-> NULL` / `ERR` | full → WouldBlock; closed → `E0054` |
| `CHANNEL_RECV(ch)` | `-> v` / `ERR` | empty → WouldBlock; closed+drain → ChannelClosed |
| `CHANNEL_TRY_SEND(ch, v)` | `-> BOOLEAN` | `TRUE`/`FALSE`; closed → `E0054` |
| `CHANNEL_TRY_RECV(ch)` | `-> v` / `NULL` / `ERR` | empty → `NULL` (not close) |
| `CHANNEL_CLOSE(ch)` | `-> NULL` | idempotent |
| `CHANNEL_LEN(ch)` / `CHANNEL_CAP(ch)` | `-> INTEGER` | inspect |

### Types & equality

| TYPE string | Equality | Display |
|-------------|----------|---------|
| `"TASK"` | `(id, generation)` identity | `<task handle id=N gen=M>` |
| `"CHANNEL"` | `(id, generation)` identity | `<channel handle id=N gen=M>` |

`==(h, h)` is `TRUE`. Handles are truthy (§2.3).

### YIELD placement (path B)

```wlwl
FUN(() , a; YIELD(); b)       // OK — Block direct child
IF(TRUE, YIELD(), 1)          // E0014
IF(TRUE, (YIELD(); 1), 1)     // syntactically OK; nested remainder not resumed (v0.7.0)
LET(x, YIELD())               // E0014
```

Trailing `YIELD()` on the last segment completes the task with `NULL`.

### Channel close protocol

Detect close with `IS_ERR` + `AT_K(ERR_PAYLOAD(r), "kind", "?") == "ChannelClosed"`.
**Never** treat `NULL` from `TRY_RECV` as close.

### v0.7.0 limits (spec §17.7)

- Single-thread cooperative scheduler (no CPU parallel speedup).
- `CHANNEL_SEND`/`RECV` do not suspend (WouldBlock).
- No deadlock detector.
- Nested YIELD does not resume nested-constructor remainder.
- Prefer `LET MUT` for shared mutable cells.

## §19. Notes on `interp.wll`

`interp.wll` is the in-skill gold-standard for v0.6 core (blocks A–N).
Concurrency gold minis: `examples/concurrency.wll` and
`examples/concurrency_cancel.wll`. Block (H) demonstrates overflow via
`safe_add`; actual `+(i64::MAX, 1)` is `E0035` + exit 1 (see
`docs/history/deviations-v0.7.md`).
