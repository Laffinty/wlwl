# WLWL v0.10 Reference (on-demand)

> Loaded only when `SKILL.md` points here. This is a lookup catalogue,
> not a tutorial. Section numbers (`§1.5`, `§8.3`, `§10.4`, `§13`, `§17`, etc.)
> refer to `docs/standard/wlwl-spec-v0.10.md` (v0.9 archived at
> `docs/history/wlwl-spec-v0.9.md`; v0.8 at `docs/history/wlwl-spec-v0.8.md`;
> v0.7 at `docs/history/wlwl-spec-v0.7.md`;
> v0.6 at `docs/history/wlwl-spec-v0.6.md`).

## Contents

- [§1. Truthy / falsy (§2.3)](#1-truthy--falsy-23)
- [§2. Version decisions](#2-version-decisions)
- [§3. AST shapes (§A.2)](#3-ast-shapes-a2)
- [§4. Lexer traps (§1)](#4-lexer-traps-1)
- [§5. Control flow (§6)](#5-control-flow-6)
- [§6. Pattern matching (§7)](#6-pattern-matching-7)
- [§7. Imports & MODULE_REF (§9)](#7-imports--module_ref-9)
- [§8. Error model (§8)](#8-error-model-8)
- [§9. Standard library catalogue (§10)](#9-standard-library-catalogue-10)
- [§10. Error codes (§11.2, §11.3)](#10-error-codes-112-113)
- [§11. Display / STR quirks (§2.5)](#11-display--str-quirks-25)
- [§12. INDEX vs `xs[i]` (§10.4, §4.5)](#12-index-vs-xsi-104-45)
- [§13. Container immutability (§3.3, §10.4)](#13-container-immutability-33-104)
- [§14. OOP (§13–§16)](#14-oop-1316)
- [§15. Session-type protocols (§14)](#15-session-type-protocols-14)
- [§16. Linear THIS (§15)](#16-linear-this-15)
- [§17. Reserved forms (§12)](#17-reserved-forms-12)
- [§18. `--format json|jsonl` diagnostics (§11.1)](#18---format-jsonjsonl-diagnostics-111)
- [§19. `EXPORT` / `wlwl.toml` features (§9.1, §9.4)](#19-export--wlwltoml-features-91-94)
- [§20. Anti-patterns (extended)](#20-anti-patterns-extended)
- [§21. Concurrency (§17)](#21-concurrency-17)
- [§22. Notes on `interp.wll`](#22-notes-on-interpwll)
- [§23. v0.9 增量备忘](#23-v09-)
- [§24. v0.10 增量备忘 — 静态契约](#24-v010-)

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
because the array is non-empty), `TASK` / `CHANNEL` handles, and
`CLASS` / `INSTANCE` values.

**`ERR(...)` is NOT in this list.** It is its own propagation mechanism per §8.2 — passing `ERR(x)` to a non-consumer function forwards the ERR; it does not cause the function to evaluate as if its argument were falsy. See §8 for the consumer registry.

## §2. Version decisions

| Version | What changed for writers |
|---------|--------------------------|
| v0.6 | Truthy overhaul; `&&`/`\|\|` short-circuit; `IF` ERR-routing; `!`/`NOT` canonical; `POP`→`AT_K`; string subscript; `LET MUT`; overflow→`E0035`; `${expr}` interpolation |
| v0.7 | §17 concurrency: `SCOPE`/`SPAWN`/`AWAIT`/`YIELD`/`TASK_*`/`SHIELD`/`CHANNEL_*`; types `TASK`/`CHANNEL`; codes `E0052`–`E0058` |
| v0.8 | Clarifications: `EXPECT_ERR` in §8.3; `=` triple-identity; `LET`/`FUN` asymmetry; `%` float `E0030`; SHIELD/SCOPE(ERR)/AWAIT host-diagnostic notes; literal subscripts; `SUB` length semantics; float exponents; `MUT` as identifier; string-literal subscript |
| **v0.9** | **True-suspension concurrency** (YIELD anywhere in task bodies; blocking SEND/RECV suspend; no `ChannelWouldBlock`); **structured cancellation** (`reason` DICT); **deadlock L1** (`E0065`/`W0065`); **OOP §13–§16** (`CLASS`/`NEW`/`THIS`/`GET_PROP`/`SET_PROP`/`CALL_METHOD`, session protocols, linear THIS); types `CLASS`/`INSTANCE`; codes `E0055`/`E0057` removed, `E0065`/`E0066`/`W0065`/`W0066` added |
| **v0.10** | **Static contracts — all default-off, runtime unchanged from v0.9**: type annotation grammar enters the spec (`LET(x: INTEGER, …)`, `FUN((a: INTEGER, b: INTEGER) : INTEGER, …)`); container/function types `ARRAY[T]` / `DICT[K,V]` / `OPTION[T]` / `RESULT[T,E]`; bounded type variables `T: Comparable` (erased at runtime); `[features] gradual_typing = off\|warn\|error`; module signature sidecars `foo.wll.sig` + `SEALED([...])`; `MATCH` exhaustiveness behind `match_exhaustiveness`; codes `E0110`–`E0116` / `W0110`–`W0117` added (compile-time only) |

## §3. AST shapes (§A.2)

Spec Appendix A.2 enumerates the node kinds:

```rust
// Expr::Let carries an optional mut_ flag.
Expr::Let { name, value, mut_: bool }

// Literal splits into plain vs interpolated.
Literal::Str(String)                         // no interpolation
Literal::Interpolated(Vec<StrPart>)          // has ${...} segments

enum StrPart {
    Text(String),                            // raw chars between ${...}
    Expr(Box<Expr>),                         // the ${...} payload
}
```

v0.7 adds **no** new AST node kinds — concurrency is all ordinary `Call` nodes. v0.9 likewise: OOP is `CLASS`/`NEW`/`THIS` keyword-calls and ordinary `Call` nodes for `GET_PROP`/`SET_PROP`/`CALL_METHOD`; session protocols are ordinary `ArrayLit`/`DictLit` values passed to `CLASS`.

## §4. Lexer traps (§1)

- `//` line comments and `/* */` block comments (nestable).
- **Everything is a prefix call — there is no infix syntax.** `+(a, b)`, not
  `a + b`; `&&(a, b)`, not `a && b`. The parser has no binary-expression
  production at all (spec §4.3), so `LET(x, a + b)` is a hard `E0011`
  (it parses `a` then hits `+` and expects `,` or `)`). `${a + b}` inside a
  string literal is the one place addition appears, and it desugars to a call.
- `MUT` is a **contextual** keyword — **only** between `LET` and `(`.
  Elsewhere it is a normal identifier: `LET(MUT, "x")`,
  `LET MUT(MUT, 1)`, `FUN((MUT), +(MUT, 1))`, `PRINT(MUT)`, `${MUT}`
  all parse.
- `CLASS` / `NEW` / `THIS` / `NOT` are keywords (§1.4). `CLASS(...)` /
  `NEW(...)` / `THIS()` parse as keyword-calls. **`THIS` alone does not work** —
  bare `THIS` is resolved as a plain name and → `E0020: undefined name 'THIS'`.
  Always write `THIS()`. (A bare `THIS` is also not an alias for `self`.)
- `${` starts interpolation inside `"..."`; nested string literals inside `${...}` are `E0001`.
- Escapes: `\$`, `\n`, `\t`, `\r`, `\\`, `\"`, `\/`, `\0`, `\b`, `\f`.
- `=` is the equality alias (not assignment). Three roles per §1.5:
  (a) `=` is `==` in **call position** (`=(a, b)` → `==(a, b)`);
  (b) `=` separates INDEX_SET sugar (`a[i] = v` → `INDEX_SET(a, i, v)`);
  (c) `=` separates default-param in `FUN((name = expr), body)`.
  No lookahead needed; the three roles don't overlap.
- Float exponents (§1.7 EBNF `digits exponent`):
  `1e2`, `1.5e2`, `1.5e-2`, `1E3`, `2.5e+1` all valid float literals.
  Bare `1e` (no digits after) raises `E0001`.
- **Negative sign** in front of integer literal: `lexer` does not consume
  the sign (per §1.6 implementation note); parser rewrites `-x` →
  `-(0, x)`. Observable behavior matches leading-sign literal (incl.
  `INTEGER_MIN` overflow via §2.2 `E0034`).

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

## §7. Imports & MODULE_REF (§9)

```
IMPORT(path, ["a", "b"])
IMPORT(path, ["orig": "alias"])
IMPORT(path, [])
MODULE_REF(path)          // -> DICT: module exports, no local binding
```

- `./` `../` relative; `wlwl:std.*` built-in namespaces.
- Errors: `E0040` missing, `E0041` cycle, `E0023` not exported, `E0021` duplicate.

Concurrency and OOP names are **global** (Appendix G) — never import them.

## §8. Error model (§8)

### §8.1 RESULT variants

`OK(v)` / `ERR(e)`; `e` must be STRING or DICT (`E0030` otherwise).

**Concurrent ERR kinds** (always dict payloads):

| `kind` | Source |
|--------|--------|
| `"ChannelClosed"` | RECV/TRY_RECV after close + drain (also parked receivers woken by close) |
| `"Cancelled"` | `AWAIT` of a cancelled task; payload includes `reason` dict |

**`ChannelWouldBlock` is removed in v0.9** — blocking SEND/RECV suspend instead.

### §8.2 Transparent propagation

Non-consumer + ERR arg → body does not run; ERR forwarded. This
applies to **all** concurrency and OOP builtins (they are not consumers).

### §8.3 Consumer registry (14 entries)

`IS_OK`, `IS_ERR`, `UNWRAP_OR`, `OR_DIE` (deprecated), `TRY`, `UNWRAP`,
`ERR_PAYLOAD`, `WRAP`, `TYPE`, `==`/`!=`, `IF` condition, `&&`/`||`
left, `BOOL`. v0.9 adds no new consumers.

`EXPECT_ERR` is **not** in this list: it is a `wlwl:std.test` export, not a global
builtin. Measured v0.10.1 — import it, it takes **one** argument, and it does **not**
catch native codes:

None of these can intercept native codes (`E0034`, `E0035`, `E1003`,
`E0100` PANIC, `E0102` top-level ERR escape, `E0052`–`E0066` host
diagnostics, `E0030`/`E0032` type / linear-THIS errors, etc.) — those
are not `RESULT` variants.

### §8.4 PANIC

`PANIC(msg)` / `UNWRAP(ERR)` / `NEG(i64::MIN)` → `E0100`.

### §8.5 Top-level escape

Uncaught ERR → `E0102`, exit 1.

## §9. Standard library catalogue (§10)

See `SKILL.md` pointers. Highlights: `wlwl:std.collection` (MAP/FILTER/…),
`std.json`, `std.fs`, `std.test`, `std.format`, `std.io`, `std.ai`,
`std.agent`. Concurrent and OOP primitives are **global**, not under
`wlwl:std.*`.

## §10. Error codes (§11.2, §11.3)

### Errors (E-codes) — v0.9 additions / changes highlighted

| Code | Meaning |
|------|---------|
| `E0014` | illegal RETURN/BREAK/CONTINUE **or YIELD outside a task** (v0.9: YIELD-Block-direct-child restriction removed) |
| `E0024` | SET on immutable binding (single-task and cross-task alike) |
| `E0030` | type error (incl. ERR payload not STRING/DICT; OOP receiver type) |
| `E0031` | subscript/key type error; NaN key; `CHANNEL_NEW` buf not non-negative integer |
| `E0032` | **[v0.9]** linear `THIS` violation or read-only field write (see §16) |
| `E0036` | array/string index OOB |
| `E0050` | **[v0.9]** object-model state error (malformed protocol; parent-chain cycle/depth; init not function; NEW arity; call after protocol `end`) |
| `E0051` | **[v0.9]** session-protocol step violation (wrong order / unchosen ⊕ branch / μ mismatch) |
| `E0052` | SCOPE/SPAWN/SHIELD arg is not a function |
| `E0053` | invalid task/channel handle; TASK_CURRENT outside a task; blocking SEND/RECV outside a task; AWAIT of a task parked with no peer |
| `E0054` | SEND/TRY_SEND on closed channel (also post-close RECV when `native_channel_close`) |
| `E0056` | SPAWN arity (including non-zero-param `fn`) |
| `E0058` | top-level SPAWN without SCOPE |
| `E0065` | **[v0.9]** structured-concurrency deadlock L1 (default strict mode) |
| `E0066` | **[v0.9]** `TASK_CANCEL`/`TASK_CANCEL_PARENT` reason not DICT |
| `E0110` | **[v0.10]** annotation mismatch (parameter / `LET` annotation vs actual type) — compile-time only |
| `E0111` | **[v0.10]** call mismatch (argument count, or the n-th argument's type) — compile-time only |
| `E0112` | **[v0.10]** return mismatch (`RETURN` value vs return annotation) — compile-time only |
| `E0113` | **[v0.10]** module contract: `EXPORT`/`IMPORT` a name the signature or `SEALED` face does not declare |
| `E0114` | **[v0.10]** module contract: signature/`SEALED` declares a name the module does not `EXPORT` |
| `E0115` | **[v0.10]** signature type conflicts with the implementation's annotation |
| `E0116` | **[v0.10]** `MATCH` non-exhaustive (missing constructor, default arm omitted) — needs `match_exhaustiveness` |
| `E0100` | PANIC |
| `E0102` | ERR escaped to top level |
| `E1003` | divide/mod by zero |
| ~~`E0055`~~ | **removed in v0.9** (close-on-read uses `ERR(ChannelClosed)`) |
| ~~`E0057`~~ | **removed in v0.9** (cross-task immutable cell uses `E0024`) |

Full table: spec §11.2.

**There is deliberately no `E0117`.** `W0117` (unreachable `MATCH` clause / dead
default arm) is a warning in every configuration — see spec §11.3.

### Warnings (W-codes)

`W0010` unused LET · `W0011` unused param · `W0012` duplicate LET ·
`W0013` IF branches inconsistent · `W0015` integer overflow (saturated) ·
`W0020` mixed dict literal · `W0030` shadow · `W0040` TODO(agent) ·
`W0051` deprecated alias · `W0053` canonical-form deviation (see §24.4) ·
**`W0065`** deadlock L1 soft warning (`strict_deadlock_detect = false`) ·
**`W0066`** `CHANNEL_NEW` large buf (above `channel_large_buf_threshold`) ·
**`W0110`–`W0116`** the `gradual_typing = "warn"` tier of `E0110`–`E0116` ·
**`W0117`** `MATCH` unreachable clause / dead default arm (**no `E` twin, ever**).

### Exit codes (§11.4)

`0` ok · `1` diagnostic (incl. `E0065`) · `101` impl crash (bug).

## §11. Display / STR quirks (§2.5)

| Value | Display |
|-------|---------|
| bool | `TRUE` / `FALSE` |
| null | `NULL` |
| array | `[e1, e2]` |
| dict | `[k: v, ...]` |
| fun | `<fun(params)>` |
| task | `<task handle id=N gen=M>` |
| channel | `<channel handle id=N gen=M>` |
| **class** | `<class Name>` / `<class>` (anonymous) |
| **instance** | `<Name instance>[N fields]` / `<instance>[N fields]` |
| float whole | one decimal (`2.0`) |

## §12. INDEX vs `xs[i]` (§10.4, §4.5)

`xs[i]` is subscript read; OOB → `E0036`. `INDEX(xs, v)` is search →
`-1` if missing (not an error). `INDEX_SET` rejects STRING receiver
(`E0030`).

**Literal subscripts** (§A.2 grammar `PostfixExpr = Primary { Postfix }`):
- Array literal: `[1, 2, 3][0]` → `1`
- Dict literal: `["a": 1]["a"]` → `1`
- String literal: `"hi"[0]` → `"h"`
- Mixed chains: `[[1,2], [3,4]][1][0]` → `1`
- **Integer / Float / Boolean / NULL literal postfix is rejected** at
  parser (no `INDEX_GET` semantics for those types).

**SUB(s, start, len?) — length semantics** (§10.5):
```wlwl
SUB("Hello, world", 0, 5)   → "Hello"    // chars[0..5]
SUB("Hello, world", 7, 5)   → "world"    // chars[7..12] = 5 codepoints
SUB("Hello, world", 0)      → "Hello, world"  // 2-arg default
SUB("Hello", 0, -1)         → ""          // negative len → 0
SUB("Hello", -1, 1)         → "o"         // negative start = from tail
```
Third arg is **length**, not end-index. Migration: `SUB(s, start, end_old)` → `SUB(s, start, -(start, end_old))` or `SLICE(s, start, end_old)` (SLICE on arrays is start/end).

**FORMAT templates** (§10.7): `FORMAT(template, args...)`. `{N}` is the
`N`th variadic arg (template is index -1). `{name}` is the first DICT
arg's key. Mixed `{N}` and `{name}` placeholders see the first DICT
arg at `{0}`. Unmatched `{...}` preserved literally; malformed template
(unmatched `{`) raises `E0039`. **Important**: `FORMAT("head={0} tail={1}",
x, y)` is correct; `FORMAT("head={0} tail={1}", [x, y])` renders the
whole list at `{0}` and leaves `{1}` literal — variadic, NOT
single-list arg.

## §13. Container immutability (§3.3, §10.4)

`PUSH`/`INDEX_SET`/… return **new** containers. Mutate only via `SET` on
`LET MUT` bindings. (Instance fields are the second mutation path — see §14.)

## §14. OOP (§13–§16)

### CLASS

```
CLASS(name, parent, members) -> CLASS
```

| Arg | Shape | Notes |
|-----|-------|-------|
| `name` | STRING \| NULL | NULL = anonymous; display only |
| `parent` | NULL \| CLASS \| protocol expr | NULL = no parent/protocol; CLASS = single inheritance; DICT/ARRAY/STRING = session protocol (§15) |
| `members` | ARRAY of `[STRING, value]` | pairs only |

Member roles: `"init"` key = constructor; closure with first param named `self` = instance method; anything else = class field (read-only). Parent chain: no cycles, depth ≤ 64 (`E0050`).

### NEW

```
NEW(cls, args...) -> INSTANCE
```

- `cls` must be CLASS (`E0030` otherwise).
- With `init`: arity must match exactly (`E0050`); `self` is injected; protocol `init` step consumed (§15).
- Without `init`: empty instance.
- `TYPE(instance)` = `"INSTANCE"`, `TYPE(cls)` = `"CLASS"`.

### GET_PROP / SET_PROP / CALL_METHOD

| Call | Receiver | Behavior |
|------|----------|----------|
| `GET_PROP(obj, k)` | DICT \| INSTANCE | INSTANCE: instance fields → class fields up parent chain; miss → `E0037` |
| `SET_PROP(obj, k, v)` | DICT \| INSTANCE | DICT: returns new dict. INSTANCE: only inside self method (`E0032` outside); class fields read-only (`E0032`); new keys become instance fields |
| `CALL_METHOD(obj, m, args...)` | DICT \| INSTANCE | method lookup + protocol check + call; `obj.m(args...)` is sugar |

`self` injection (§4.6): receiver injected as first arg **only when the first formal is named `self`**. Native members: no injection.

## §15. Session-type protocols (§14)

Protocol = ordinary WLWL value in `CLASS`'s second arg.

| Form | Meaning |
|------|---------|
| `"end"` / `["end"]` | terminate |
| `["then"`/`"seq"`, `"m", next]` | call `m`, then `next` |
| `["recv", T, next]` | payload type annotation, then `next` |
| `["seq", "a", "b", "c"]` | flat sequence a→b→c→end |
| `["choice", {m1: c1, m2: c2}]` | internal choice ⊕ |
| `["mu", "X", body]` | recursion binder μX.body |
| `["var", "X"]` | recursion variable |
| `{m1: v1, m2: v2}` | sequence; values are payload-type names or nested continuations |

State machine: Unrestricted (no protocol) | Live(remaining) | Done.
Violations: wrong order / unchosen branch → `E0051`; call after `end` →
`E0050`; malformed protocol at `CLASS` → `E0050`. μ unfold budget 64.

**v0.9 scope**: sequence + ⊕ + μ. External choice `&`, named protocols,
`par` — not supported (malformed → `E0050`).

## §16. Linear THIS (§15)

`THIS()` = current method receiver (INSTANCE). Rules:

1. Once per method call (second → `E0032`).
2. Never escape: array/dict literal, closure capture, method return,
   `SPAWN` body/capture, `AWAIT` arg, `SET_PROP` value → all `E0032`.
3. Outside a method body → `E0032`.
4. `SET_PROP` on INSTANCE outside a self method of that instance → `E0032`.

## §17. Reserved forms (§12)

**None reserved.** All named constructs have defined semantics and
registered entries in `BUILTIN_REGISTRY` (`docs/appendix_G.md`).
`CLASS`/`NEW`/`THIS` are keywords with real OOP semantics since v0.9.

## §18. `--format json|jsonl` diagnostics (§11.1)

Machine-readable diagnostics: `error_schema_version`,
`errorCategory`, `retryable`, `suggestion_code`, `related`, `trace`.
JSONL = one diagnostic per line. `E0065` has `errorCategory`
`"deadlock"`.

## §19. `EXPORT` / `wlwl.toml` features (§9.1, §9.4)

`EXPORT(name_array)` at top level. Manifest `[features]`:

| Feature | Default | Effect |
|---------|---------|--------|
| `strict_types` | `false` | call-boundary type checks (`E0033`) |
| `allow_builtin_shadow` | `false` | shadow builtins with `W0030` instead of `E0025` |
| `strict_deadlock_detect` | `true` | deadlock L1 → `E0065`; `false` → `W0065` + `E0053` |
| `native_channel_close` | `false` | post-close RECV → `E0054` instead of `ERR(ChannelClosed)` |
| `channel_large_buf_threshold` | `1024` | `CHANNEL_NEW` buf above threshold → `W0066`; `0` disables |

## §20. Anti-patterns (extended)

Beyond the top-24 in `SKILL.md`:

- **Out-of-range array index returns `NIL`** — FALSE; it raises `E0036` (§4.5).
- **`IS_NIL(x)` exists** — FALSE; the literal is `NULL`.
- **`APPEND(arr, x)` is a builtin** — FALSE; the global is `PUSH` (§10.4).
- **`E0009` is divide-by-zero** — FALSE; it's `E1003` (§2.2).
- **`LIST(1, 2, 3)` is the literal** — FALSE; use `[1, 2, 3]` (§4.8).
- **`FORMAT("…{0}…", [x])` unpacks `x`** — FALSE; FORMAT is variadic positional.
- **`POP(d, k, default)` is canonical** — DEPRECATED; prefer `AT_K`.
- **Passing `ERR(x)` to a non-consumer lets the function decide** — FALSE; §8.2 forwards.
- **`SUB(s, start, end_index)` reads start/end** — FALSE; third arg is LENGTH. See §12.
- **`1.5e2` is a parse error** — FALSE; float exponents are per §1.7 EBNF. See §4.
- **`LET(MUT, …)` is invalid (`MUT` is reserved)** — FALSE; `MUT` is a context keyword. See §4.
- **`EXPECT_ERR(/(1, 0))` rescues division by zero** — FALSE; native codes are not `RESULT`-shaped.
- **`NOT(ERR(x))` returns `FALSE`** — FALSE; `NOT` propagates ERR. Safe: `NOT(BOOL(ERR(x)))`.
- **`ERR(ChannelWouldBlock)` from blocking SEND/RECV** — FALSE in v0.9; they suspend. Use `TRY_*`.
- **`E0055` / `E0057` can be caught** — FALSE in v0.9; both codes removed. Use `ERR(ChannelClosed)` / `E0024`.
- **`CLASS("C", null, [count: 0])` with a dict of fields** — FALSE; `members` must be an ARRAY of `[STRING, value]` pairs: `[["count", 0]]`.
- **`THIS` is a plain alias for `self`** — FALSE in v0.9; `THIS` is a linear capability (consume once; never escape).

## §21. Concurrency (§17)

### Builtins (all global, **not** §8.3 consumers)

| Builtin | Signature | Notes |
|---------|-----------|-------|
| `SCOPE(fn)` | `-> v` | structured scope; nestable; `fn` must be 0-ary function |
| `SPAWN(fn)` | `-> TASK` | child task; **must** be inside SCOPE (`E0058`) |
| `AWAIT(task)` | `-> v` | terminal value; child ERR is a **value** |
| `YIELD()` | `-> NULL` | cooperative checkpoint; **any expression position in a task body** (v0.9) |
| `TASK_CURRENT()` | `-> TASK` | outside task → `E0053` |
| `TASK_IS_CANCELLED()` | `-> BOOLEAN` | outside task → `FALSE` |
| `TASK_CANCEL(task, reason?)` | `-> NULL` | cooperative; `reason` DICT (default `{}`); non-DICT → `E0066` |
| `TASK_CANCEL_PARENT(reason?)` | `-> NULL` | self + scope subtree |
| `SHIELD(fn)` | `-> v` | defer cancel; does **not** absorb `fn`'s ERR |
| `CHANNEL_NEW(buf)` | `-> CHANNEL` | `buf=0` = sync; bad buf → `E0031`; huge → `W0066` |
| `CHANNEL_SEND(ch, v)` | `-> NULL` | **suspends** when full + no peer (v0.9); closed → `E0054` |
| `CHANNEL_RECV(ch)` | `-> v` / `ERR` | **suspends** when empty + no peer; closed+drain → `ChannelClosed` |
| `CHANNEL_TRY_SEND(ch, v)` | `-> BOOLEAN` | `TRUE`/`FALSE`; closed → `E0054` |
| `CHANNEL_TRY_RECV(ch)` | `-> v` / `NULL` / `ERR` | empty → `NULL` (not close) |
| `CHANNEL_CLOSE(ch)` | `-> NULL` | idempotent; wakes parked waiters |
| `CHANNEL_LEN(ch)` / `CHANNEL_CAP(ch)` | `-> INTEGER` | inspect |

### Types & equality

| TYPE string | Equality | Display |
|-------------|----------|---------|
| `"TASK"` | `(id, generation)` identity | `<task handle id=N gen=M>` |
| `"CHANNEL"` | `(id, generation)` identity | `<channel handle id=N gen=M>` |
| `"CLASS"` | object identity | `<class Name>` |
| `"INSTANCE"` | object identity | `<Name instance>[N fields]` |

`==(h, h)` is `TRUE`. Handles/objects are truthy (§2.3).

### YIELD placement — legal everywhere, but only usable in statement position

Syntactically, `YIELD` may appear in any expression position inside a task body
(the v0.7/v0.8 "direct child of a Block" restriction is gone). **There is no
continuation capture**, so an outer expression that *consumes* `YIELD`'s value
does not complete. Measured on v0.10.1:

```wlwl
FUN(() , a; YIELD(); b)   // OK — statement position, `b` runs afterwards
IF(TRUE, YIELD(), 1)      // → NULL      (NOT 1)
LET(x, YIELD())           // → x NEVER BOUND; using x → E0020: undefined name
[1, YIELD(), 3]           // → NULL      (NOT [1, NULL, 3])
+(YIELD(), 100)           // → NULL      (NOT 100)
WHILE(c, ...YIELD()...)   // → loop runs ONE round; the counter never advances
LET(y, YIELD); y()        // → E0020: undefined name 'YIELD'
```

Note *what* dies: it is the **outermost** expression that collapses, not just the
`YIELD` sub-position — which is why `IF(TRUE, YIELD(), 1)` loses the whole `IF`
rather than yielding `1` from the untaken branch.

`YIELD` outside any task (top level / `SCOPE` body) → `E0014`.

**There is no indirect form**: `YIELD` is a statement keyword, not a value you
can bind. `LET(y, YIELD)` → `E0020`.

**None of this produces a diagnostic.** A loop that runs one round and returns a
plausible number is the easiest silently-wrong concurrent program to ship.
Keep `YIELD()` in statement position.

### Channel close protocol

Detect close with `IS_ERR` + `AT_K(ERR_PAYLOAD(r), "kind", "?") == "ChannelClosed"`.
**Never** treat `NULL` from `TRY_RECV` as close. Parked receivers woken
by close get `ERR(ChannelClosed)`; parked senders get `E0054`.

### Host diagnostic vs user ERR (§17.5)

`AWAIT(task)` re-raises *host* diagnostics (`E0101`, `E0053`, etc.) —
distinct from user `ERR(...)` values which pass through as ordinary
`RESULT` values. `SCOPE(ERR("x"))` propagates `ERR("x")` per §8.2
BEFORE the §17.1 fn-type check; `E0052` does not fire.

### v0.9 limits (spec §17.7)

- Single-thread cooperative scheduler (no CPU parallel speedup).
- Blocking SEND/RECV **suspend** (v0.9) — no `ChannelWouldBlock`.
- Deadlock detection L1: ≥ 2 tasks in one SCOPE parked on channel ops
  with no peer → `E0065` (strict) / `W0065` + `E0053` (soft). Pure
  YIELD mutual-yield is not a deadlock.
- Fairness: best-effort FIFO (§17.8). In-flight sibling cancel is
  observable at the next checkpoint.
- Prefer `LET MUT` for shared mutable cells.

## §22. Notes on `interp.wll`

`interp.wll` is the in-skill gold-standard for v0.6 core (blocks A–N)
plus patch examples (SUB length semantics, `MUT` as binding name, float
exponents, string literal subscript). Concurrency gold minis:
`examples/concurrency.wll` and `examples/concurrency_cancel.wll`.
OOP gold mini: `examples/oop.wll`.

Block (H) demonstrates overflow handling via `safe_add`; actual
`+(i64::MAX, 1)` is native `E0035` + exit 1 (NOT a `RESULT`-shaped
ERR; cannot be caught by §8.3 consumers — see §20 anti-patterns).

## §23. v0.9 增量备忘

Every v0.9 item that affects writing `.wll` source — pulled from
`docs/standard/wlwl-spec-v0.9.md` 附录 D.

| # | § | Change |
|---|---|--------|
| 1 | §17.1 / §17.2 | **True suspension**: `YIELD` at any expression position in task bodies; nested constructors resume remainder; blocking `CHANNEL_SEND`/`RECV` suspend |
| 2 | §8.1 / §17.2 | **`ChannelWouldBlock` removed** — use `TRY_*` for non-blocking semantics |
| 3 | §17.3 | **Structured cancellation**: `TASK_CANCEL(task, reason?)` / `TASK_CANCEL_PARENT(reason?)`; `AWAIT` payload has `reason`; non-DICT → `E0066` |
| 4 | §17.4 | **Algebraic-effect execution model**: yield / channel-op / cancel as effects; scheduler is the handler (no user-defined handlers) |
| 5 | §17.7 | **Deadlock L1**: `E0065` strict / `W0065` + `E0053` soft; same-SCOPE channel parks only |
| 6 | §17.8 | **Fairness / observability**: best-effort FIFO; in-flight cancel visible at next checkpoint |
| 7 | §11.2 | **Codes**: `E0055`/`E0057` removed; `E0065`/`E0066`/`W0065`/`W0066` added; `E0014`/`E0032`/`E0050`/`E0051` redefined |
| 8 | §13–§16 | **OOP**: `CLASS`/`NEW`/`THIS`/`GET_PROP`/`SET_PROP`/`CALL_METHOD`; session protocols (sequence + ⊕ + μ); linear `THIS` |
| 9 | §2.1 | **Types** `CLASS` / `INSTANCE`; object-identity equality |
| 10 | §9.2 / §9.4 | `MODULE_REF` defined; features `strict_deadlock_detect` / `native_channel_close` / `channel_large_buf_threshold` |

## §24. v0.10 增量备忘 — 静态契约

Every v0.10 item that affects writing `.wll` source — pulled from
`docs/standard/wlwl-spec-v0.10.md` 附录 D.

**v0.10's runtime is identical to v0.9's.** Everything below is opt-in; a
program that adds no annotation and ships no `.sig` behaves exactly as before.

| # | § | Change |
|---|---|--------|
| 1 | §5.2 / 附录 A | **Type annotation grammar enters the spec** for the first time (v0.9 had `name: Type` only as a table metavariable) |
| 2 | §5.2 | **Container/function types**: `ARRAY[T]`, `DICT[K,V]`, `OPTION[T]`, `RESULT[T,E]`, `FUN` |
| 3 | §5.2.1 | **Bounded type variables** `T: Comparable`; erased at runtime, so `Value` is unchanged |
| 4 | §2.6 | **`[features] gradual_typing` = `off`(default) / `warn` / `error`** |
| 5 | §9.1 / §9.6 | **Module signature sidecars** `foo.wll.sig` and `SEALED([...])`; both default to absent |
| 6 | §7.4 | **`MATCH` exhaustiveness** behind `[features] match_exhaustiveness` |
| 7 | §11.2 / §11.3 | **Codes**: `E0110`–`E0116` / `W0110`–`W0117` added, all compile-time only; `W0117` has **no** `E0117` |
| 8 | §1.4 | **Keyword table unchanged** — `SEALED` is prefix-call form, not a keyword, so the reserved-form set stays empty |

### §24.1 Annotation grammar traps (measured against the reference impl)

1. **`ARRAY` requires brackets.** A bare `ARRAY` is `E0010`. But a bare
   `DICT` / `OPTION` / `RESULT` parses as an *opaque named type* with **no
   error** — verified: `LET(d, FUN((x: DICT) : INTEGER, 1))` runs clean.
2. **Angle-bracket and arrow forms are parse errors.** `DICT<STRING, INTEGER>`
   → `E0011 expected ')', got Gt`; `FUN(INTEGER) -> STRING` → `E0012
   expected ',', got Minus`.
   > ⚠ Spec §5.2.1 fact #2 still says these parse "into a single opaque type"
   > **without an error**. That is **stale** — the parser now rejects both.
   > Treat this table as authoritative over that sentence.
3. **A constraint may only follow a bare identifier.** `T: Comparable` is
   legal; `ARRAY[INTEGER]: Comparable` is **`E0010`**
   ("a type constraint may only follow a bare type variable … not a type like
   `ARRAY[…]` or `DICT[…]`").
   > ⚠ Spec §5.2.1 fact #3 says `E0012` for this; the implementation says
   > `E0010`. `E0012` is the return-type-mismatch code and is unrelated.

### §24.2 `wlwl.toml` — the two v0.10 feature keys

`[package]` is **required** for a manifest to be a manifest — all three of
`name` / `version` / `entry`. Since v0.10.1 the `[features]` table is read
independently of `[package]`, so a `[features]`-only file still switches things
on **and** emits `W0001`; before v0.10.1 it was swallowed silently and the
static layer never ran at all (that was the bug R10-010 fixed).

```toml
[package]
name = "myapp"
version = "0.1.0"
entry = "main.wll"

[features]
gradual_typing = "error"          # "off" | "warn" | "error"
match_exhaustiveness = "error"   # "off" | "warn" | "error"
```

| Feature | Default | Effect |
|---------|---------|--------|
| `gradual_typing` | `off` | `warn` → `W0110`–`W0112`; `error` → `E0110`–`E0112` (blocking) |
| `match_exhaustiveness` | **follows `gradual_typing`** | `E0116`/`W0116` non-exhaustive + `W0117` unreachable clauses |

**`gradual_typing` governs `E0110`–`E0112` only** — *not* `E0113`–`E0116`. Verified
against the implementation: `main.rs` documents `warn` as "report `W0110`-`W0112`"
and `error` as "report `E0110`-`E0112`, block with exit 1"; the key is absent →
`MatchExhaustivenessSetting::following(gradual)`, with a lock test
(`match_exhaustiveness_follows_gradual_typing_when_absent`) on that default.

Practical consequence: a manifest that writes only
`gradual_typing = "error"` has **`E0116` switched on as well**, whether or not
you intended it. Write both keys explicitly when you care about the difference.

`E0113`–`E0116` are **compile-time only**. None is an `ERR`, so `EXPECT_ERR` /
`TRY` / `UNWRAP_OR` cannot catch them.

### §24.3 CLI additions (v0.10)

| Command | What it does |
|---------|--------------|
| `wlwl check <file>` | parse + name resolution + static-contract diagnostics; **walks the import graph**, so it is where module signatures (`E0113`–`E0115`) actually surface. Not to be confused with `wlwl run` |
| `wlwl ast <file>` | dump the parsed AST (text or jsonl) — useful when a parse error points somewhere surprising |
| `wlwl sig <file>` | print the module's signature (text or JSON) |
| `wlwl sig-gen <file>` | write/refresh the `*.wll.sig` sidecar |
| `wlwl interface <file>` | export the public surface as JSON |
| `wlwl schema` | type system + constraints + static-contract codes as JSON |
| `wlwl lsp` | stdio JSON-RPC thin shell: `diagnostics` / `definition` / `hover` + registry-driven completion (no `rename`, no `format`) |

`wlwl fmt` / `wlwl fmt --check`: canonical form per spec §16.3. **`--check`
exits 1 with `W0053` when the source is not canonical.** Two things to know:

- **The canonical form has no trailing `;` on the last statement.**
  `PRINT("x")` is canonical; `PRINT("x");` is not. Interior statements keep
  theirs. Comments and indentation are handled (comments are stripped before
  comparison, leading whitespace dropped).
- **Line endings do not matter** since v0.10.1 — CRLF and LF compare equal
  (v0.10 and earlier reported `W0053` for *any* CRLF file, which made the gate
  unusable on Windows checkouts).

### §24.4 `W0053` in one paragraph

`W0053` means "this file is not in canonical layout". It is about **layout**, not
correctness: a file that trips it usually still runs fine. The usual causes are a
trailing `;` on the last statement, stray spaces inside a call, or two statements
on one line. Fix it by running `wlwl fmt <file>` and reading the output — the
formatter is the authority on canonical form, not this document. The `.wll` files
under `examples/` are hand-written teaching material and are **not** canonical;
`wlwl run` is what matters for them.

### §24.5 Spec / impl / docs 三处引用 (v0.10 current)

| 文档 | 路径 | 用途 |
|------|------|------|
| 权威规范 | `../docs/standard/wlwl-spec-v0.10.md` | 唯一真相源 |
| v0.9 归档 | `../docs/history/wlwl-spec-v0.9.md` | 历史 §0–§17 上下文 |
| v0.8 归档 | `../docs/history/wlwl-spec-v0.8.md` | 历史 §0–§12 上下文 |
| v0.7 归档 | `../docs/history/wlwl-spec-v0.7.md` | 历史 §17 上下文 |
| v0.6 归档 | `../docs/history/wlwl-spec-v0.6.md` | §0–§12 核心 |
| 内建注册表 | `../docs/appendix_G.md` | 110 条内建单一真相源 |
