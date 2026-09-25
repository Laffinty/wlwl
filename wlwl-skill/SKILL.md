---
name: writing-wlwl
description: "Writes WLWL v0.9 .wll source files (spec docs/standard/wlwl-spec-v0.9.md). Covers v0.6 core (truthy overhaul, &&/|| short-circuit, IF ERR-routing, ! canonical, AT_K rename, string subscript, LET MUT, overflow->E0035, ${} interpolation), v0.7 §17 concurrency, v0.8 clarifications (EXPECT_ERR, = triple-identity, LET/FUN asymmetry, SUB length, float exponents, MUT as identifier, string-literal subscript), and v0.9 (true-suspension concurrency: YIELD at any expression position in task bodies, blocking CHANNEL_SEND/RECV suspend, no ChannelWouldBlock; TASK_CANCEL(task, reason?) structured cancellation; deadlock L1 E0065/W0065; OOP §13-§16: CLASS/NEW/THIS/GET_PROP/SET_PROP/CALL_METHOD with session-type protocols and linear THIS; CLASS/INSTANCE types; error codes E0055/E0057 removed, E0065/E0066/W0065/W0066 added). Use when the user asks for WLWL code, a .wll file, or anything targeting wlwl-spec-v0.9 (or v0.6/v0.7/v0.8 core). Do NOT use for v0.5 or earlier (POP-not-AT_K, no LET MUT), the Rust implementation, or the formatter."
---

# Writing WLWL (v0.9)

## When to load / NOT to use

**Use when:**
- The user asks for a `.wll` source file, a WLWL program, or a v0.6 / v0.7 / v0.8 / v0.9 idiom.
- A task targets `wlwl-spec-v0.9.md` (current) or `wlwl-spec-v0.6.md` / `wlwl-spec-v0.7.md` / `wlwl-spec-v0.8.md` (archived).
- Reviewing or debugging v0.6–v0.9 source (concurrency, OOP, session protocols included).

**Don't use when:**
- The source targets WLWL v0.5 or earlier (different truthy rules, `POP`-not-`AT_K`, no `LET MUT`).
- The task is editing the Rust implementation in `impl/` or the formatter spec.
- The user wants `wlwl.exe` CLI documentation (that's `wlwl help` / `wlwl <subcmd> --help`).
- The artifact is a Rust test, ADRs, or build-system file.

## Writing flow

Tick these as you go:

```
WLWL writing progress (v0.9):
- [ ] 1. Sketch the AST shape (use the operators/builtins section below)
- [ ] 2. Decide mutation — any SET later means LET MUT(name, value) now
       (MUT itself can also be the binding name, e.g. LET(MUT, "x"))
- [ ] 3. Concurrency? wrap SPAWN in SCOPE; YIELD legal at ANY expression
       position inside a task body (v0.9 true suspension). Blocking
       CHANNEL_SEND/RECV suspend — no ChannelWouldBlock.
- [ ] 4. OOP? CLASS(name, parent, members) with [STRING, value] pair
       members; methods are closures whose first param is named `self`;
       declare a session protocol for call-order control (§14).
- [ ] 5. THIS is linear: consume at most once per method call; never
       store it in containers/closures/SPAWN/AWAIT/return (E0032).
- [ ] 6. SUB uses length semantics: SUB(s, start, len) where len is
       LENGTH (codepoint count)
- [ ] 7. Write the .wll file (one stmt per line, semicolons, no leading indent)
- [ ] 8. Run `wlwl run <file>` — MUST exit 0 with expected stdout
- [ ] 9. If 8 fails: consult reference.md for the failing token/operator
```

Do not skip step 8. `wlwl run` is the source of truth. `wlwl fmt --check` is best-effort (see Verification loop).

## Truthy / falsy — spec §2.3

Eight falsy values: `FALSE`, `NULL`, `0`, `0.0`, `""`, `[]` (empty array), `DICT()` (empty dict), `NaN`.

**Everything else is truthy**, including non-empty strings, non-empty containers, `OK(FALSE)`, `TASK`/`CHANNEL` handles, and `CLASS`/`INSTANCE` values.

`ERR(...)` is **not** in this list — it is its own propagation mechanism per §8.2. Passing `ERR(x)` to a non-consumer function transparently forwards the `ERR`; it does NOT make the function body "skip" because the arg was falsy. Only §8.3 consumers (`IS_OK`, `IS_ERR`, `UNWRAP_OR`, `OR_DIE`, `TRY`, `UNWRAP`, `ERR_PAYLOAD`, `WRAP`, `TYPE`, `==`/`!=`, `IF` condition, `&&`/`||`, `BOOL`, `EXPECT_ERR`) can inspect an ERR safely.

## Operators / builtins — cheat sheet (v0.6–v0.9)

**Equality** (spec §2.4): `=(a, b)` and `==(a, b)` are aliases (§1.5 call-position desugar; `=` triple-identity — NOT same as `INDEX_SET`'s `=` or `Param`'s `=`). `!=(a, b)`. Cross-type numeric: `==(1, 1.0)` is `TRUE`. Function/handle identity: `==(f, f)` and `==(h, h)` are `TRUE`. **v0.9 object identity**: `==(cls, cls)` and `==(obj, obj)` are `TRUE` for the same class/instance; two `NEW(...)` results are never equal.

**Comparison**: `<(a, b)`, `>(a, b)`, `<=(a, b)`, `>=(a, b)`.

**Arithmetic**: `+(a, b)`, `-(a, b)`, `*(a, b)`, `/(a, b)`, `%(a, b)`. `+` is also string concat and array concat. Integer overflow → native `E0035`; divide-by-zero → native `E1003` (note: **not** `E0009`; these are native codes per §11.2 — NOT ERR, cannot be caught by `EXPECT_ERR` / `UNWRAP_OR` / `TRY`; see anti-pattern #17).

**Boolean**: `&&(a, b)` / `||(a, b)` short-circuit; right side not evaluated when left decides. `NOT(x)` and `!(x)` are equivalent. `NOT(ERR(...))` propagates ERR (§4.3); safe coercion idiom: `NOT(BOOL(ERR(...)))` (BOOL is a §8.3 consumer).

**Indexing**: `xs[i]` returns one element; out-of-range array/string index raises `E0036`. `INDEX(xs, v)` returns the index of `v` in `xs`, or `-1` if not found (NOT an error).

**Literal subscripts** (spec §A.2 grammar `PostfixExpr = Primary { Postfix }`):
- Array literal: `[1, 2, 3][0]` → `1`
- Dict literal: `["a": 1]["a"]` → `1`
- String literal: `"hi"[0]` → `"h"`
- Mixed chains: `["a": 1]["a"][0][0]`, `[[1,2], [3,4]][1][0]` all legal
- Integer / Float / Boolean / NULL literal postfix **rejected** at parser (no `INDEX_GET` semantics)

**Float exponents** (spec §1.7 EBNF `digits exponent`):
```
1e2          → 100.0
1.5e2        → 150.0
1.5e-2       → 0.015
1E3          → 1000.0
2.5e+1       → 25.0
```
Bare `1e` (no digits after) raises `E0001`.

**Identifier names** (spec §1.4): `MUT` is a **context keyword** — only after `LET` modifier slot. Other positions can use `MUT` as a regular identifier:
- `LET(MUT, "x")` — binding name = `"MUT"`, immutable
- `LET MUT(MUT, 1)` — first `MUT` is modifier, second is binding name, mutable
- `FUN((MUT), MUT + 1)` / `PRINT(MUT)` / `${MUT}` — all valid

**SUB(s, start, len?) — length semantics** (spec §10.5):
The third arg is **length** (codepoint count), not end-index.
```wlwl
SUB("Hello, world", 0, 5)   → "Hello"    // chars[0..5]
SUB("Hello, world", 7, 5)   → "world"    // chars[7..12] = 5 codepoints
SUB("Hello, world", 0)      → "Hello, world"  // 2-arg default = to end
SUB("Hello", 0, -1)         → ""          // negative len → 0
SUB("Hello", -1, 1)         → "o"         // negative start counts from tail
```

## Control flow — spec §6

- `IF(cond, then, else)` — `else` is required for ERR routing; without it the false branch is `NULL`.
- `WHILE(cond, body)` — value is `NULL`; body ERR terminates the loop and propagates.
- `FOR(x, iterable, body)` — iter over ARRAY (by element), DICT (by insertion-order key), STRING (by code point, binding single-char STRING). Value is `NULL`.
- `RETURN(expr?)` / `BREAK()` / `CONTINUE()` — only valid inside a function or loop body; outside → `E0014`.

## Pattern matching — spec §7

`MATCH(value, clauses[, default])`. Patterns:

- Literal: `0`, `"foo"`, `TRUE`.
- Identifier: binds the name to the value.
- Wildcard: `_` matches anything without binding.
- Array: `[a, b, *rest]` — positional; `*rest` collects the tail.
- Dict: `["k": v, ...]` — partial by listed keys.
- Variant: `OK(p)` / `ERR(p)` — match `RESULT` variants, then match the payload.

Clauses are tried in order; first hit wins. No match and no default → `NULL` (not an error). Pattern mismatch in a `LET` destructuring IS an error (`E0026`).

## OOP — spec §13–§16 (v0.9)

`CLASS` / `NEW` / `THIS` / `GET_PROP` / `SET_PROP` / `CALL_METHOD` are fully defined in v0.9. Types: `CLASS`, `INSTANCE` (§2.1). None are §8.3 ERR consumers.

### CLASS / NEW

```wlwl
LET(C, CLASS("Counter", NULL, [
    ["init", FUN((self), SET_PROP(self, "n", 0))],
    ["inc", FUN((self), SET_PROP(self, "n", +(GET_PROP(self, "n"), 1)))],
    ["get", FUN((self), GET_PROP(self, "n"))]
]));
LET(o, NEW(C));
CALL_METHOD(o, "inc");
CALL_METHOD(o, "get");   // 1
```

- `CLASS(name, parent, members)`: `name` is STRING or `NULL` (anonymous); `parent` is `NULL` | CLASS (single inheritance) | **session-protocol expression**; `members` is an ARRAY of `[STRING, value]` pairs.
- Member classification: key `"init"` = constructor; other keys whose value is a closure with first param named `self` = instance methods; everything else = **class field** (read-only).
- `NEW(cls, args...)` allocates an instance and runs `init` with `self` injected (arity must match exactly — `E0050` on mismatch). No `init` → empty instance.
- Parent chain: no cycles, depth ≤ 64 (`E0050` on violation).

### GET_PROP / SET_PROP / CALL_METHOD

```wlwl
GET_PROP(obj, "k")            // instance field, then class field up the parent chain; miss → E0037
SET_PROP(obj, "k", v)         // instance fields only; class fields are read-only (E0032)
CALL_METHOD(obj, "m", args...)  // method lookup + protocol check + call
o.m(args...)                  // sugar for CALL_METHOD(o, "m", args...)
```

`SET_PROP` on an INSTANCE is only legal **inside a self method of that instance** — external writes → `E0032`. New keys become instance fields. `SET_PROP` on a DICT returns a **new** dict (value semantics).

`self` injection rule (§4.6): the receiver is injected as the first argument **only when the first formal is named `self`**. Native std members receive no injection.

### Session-type protocols — spec §14

Declare a protocol as `CLASS`'s second argument (ordinary WLWL values):

```wlwl
// Sequence: a → b → c
CLASS("C", ["a": "end", "b": "end", "c": "end"], [...])

// Internal choice ⊕: pick open or close
CLASS("C", ["choice", ["open": "end", "close": "end"]], [...])

// μ recursion: get repeats, close ends
CLASS("C", ["mu", "X", ["choice", [
    ["get", ["recv", "int", ["var", "X"]]],
    ["close", "end"]
]]], [...])
```

Protocol forms: `"end"` · `["then"/"seq", "m", next]` · `["recv", T, next]` · `["seq", "a", "b", "c"]` · `["choice", {...}]` · `["mu", "X", body]` · `["var", "X"]` · `{m1: v1, m2: v2}` (sequence). Violations: wrong order / unchosen branch → `E0051`; call after `end` → `E0050`; malformed protocol at `CLASS` → `E0050`. v0.9 supports sequence + `⊕` + μ; external choice `&`, named protocols, and `par` are not in this version.

### Linear THIS — spec §15

`THIS()` returns the current method receiver. Rules:

1. **At most once per method call** — second `THIS()` → `E0032`.
2. **Never escape the method body** — `E0032` when stored in an array/dict literal, captured by a closure, returned from the method, passed into `SPAWN`, used as `AWAIT` arg, or used as `SET_PROP` value.
3. **Only inside a method** — `THIS()` at top level → `E0032`.

```wlwl
LET(C, CLASS("C", NULL, [
    ["ok", FUN((self), LET(x, THIS()); GET_PROP(x, "k"))],     // fine: consume once, use locally
    ["bad", FUN((self), LET(d, ["t": THIS()]))]                 // E0032: dict escape
]));
```

## Reserved forms — spec §12

**None reserved.** All named constructs have defined semantics (§4 / §10 / §13–§17) and registered entries in `BUILTIN_REGISTRY` (`docs/appendix_G.md`). `CLASS` / `NEW` / `THIS` are keywords with real OOP semantics (§13–§15) since v0.9; `MODULE_REF` is defined in §9.2.

## Concurrency — spec §17 (v0.9 true-suspension model)

All concurrent names are **global builtins** (no IMPORT). None are ERR consumers: `AWAIT(ERR("x"))` transparently propagates (§8.2).

### Core

```wlwl
LET(v, SCOPE(FUN(() ,
    LET(h, SPAWN(FUN(() ,
        LET(x, YIELD());   // legal at ANY expression position (v0.9)
        42
    )));
    AWAIT(h)              // 42; child ERR comes back as a VALUE
)));
```

| Builtin | Role |
|---|---|
| `SCOPE(fn)` | structured scope; returns `fn`'s value; nestable |
| `SPAWN(fn)` | child task → `TASK` handle; `fn` must take 0 params |
| `AWAIT(task)` | wait / read terminal value (ERR is a value — consume explicitly) |
| `YIELD()` | cooperative checkpoint; **legal at any expression position in a task body** |
| `TASK_CURRENT()` / `TASK_IS_CANCELLED()` | handle / cancel probe |
| `TASK_CANCEL(h, reason?)` / `TASK_CANCEL_PARENT(reason?)` | cooperative cancel; optional `reason` DICT (default `{}`); non-DICT → `E0066` |
| `SHIELD(fn)` | defer cancel until outermost shield exits; **does not** absorb `fn`'s ERR |
| `CHANNEL_NEW(buf)` | `buf=0` → sync channel; negative/non-int buf → `E0031`; huge buf → `W0066` |
| `CHANNEL_SEND` / `CHANNEL_RECV` | **suspend** when full/empty with no peer (v0.9 true suspension) |
| `CHANNEL_TRY_SEND` / `CHANNEL_TRY_RECV` | non-blocking: `TRUE`/`FALSE`, value / `NULL` |
| `CHANNEL_CLOSE` / `CHANNEL_LEN` / `CHANNEL_CAP` | close (idempotent) / inspect |

### Hard rules

1. **Explicit SCOPE only.** `SPAWN` outside any `SCOPE` → `E0058`. No free-floating tasks.
2. **`YIELD` anywhere in a task body (v0.9).** The v0.7/v0.8 "direct child of a Block" restriction is **gone**. `LET(x, YIELD())`, `IF(c, YIELD(), 1)`, `[1, YIELD(), 3]`, nested `WHILE`/`FOR`/`IF` bodies — all legal. Suspension resumes the nested remainder. `YIELD` **outside** any task (top level, `SCOPE` body) → `E0014`. Indirect `LET(y, YIELD); y()` also suspends.

3. **Close signal is ERR, not NULL.**

   ```wlwl
   LET(r, CHANNEL_TRY_RECV(ch));
   IF(IS_ERR(r),
       IF(==(AT_K(ERR_PAYLOAD(r), "kind", "?"), "ChannelClosed"),
           "done",
           "other"),
       r)   // plain value
   ```

   Kinds: `"ChannelClosed"` | `"Cancelled"` (with `reason` dict). **`ChannelWouldBlock` is gone in v0.9** — blocking ops suspend instead.

4. **Blocking ops suspend (v0.9).** `CHANNEL_SEND` on full and `CHANNEL_RECV` on empty **park the task** until a peer arrives or the channel closes. They require an active task (`E0053` outside a task) — use `CHANNEL_TRY_*` in non-task contexts. Close wakes parked receivers with `ERR(ChannelClosed)` and parked senders with `E0054`.

5. **Cancel is cooperative and structured.** `TASK_CANCEL(task, reason?)` sets flags; `reason` must be a DICT (`E0066` otherwise). Observe with `TASK_IS_CANCELLED()`. `AWAIT` of a cancelled task returns `ERR(kind="Cancelled", reason: <dict>)` — consume it or it escapes as `E0102`.

6. **Shared state = `LET MUT`.** Prefer `LET MUT` for cells written from children. `SET` on a `LET` binding is `E0024` (single-task and cross-task alike).

7. **Deadlock detection L1 (v0.9).** ≥ 2 tasks in one `SCOPE` parked on channel ops with no possible peer → `E0065` (default strict) or `W0065` + `E0053` (`[features] strict_deadlock_detect = false`). Pure `YIELD` mutual-yield is NOT a deadlock.

### Canonical shapes

```wlwl
// Producer / consumer with true suspension (v0.9)
SCOPE(FUN(() ,
    LET(ch, CHANNEL_NEW(0));
    LET(p, SPAWN(FUN(() ,
        CHANNEL_SEND(ch, 1);
        CHANNEL_SEND(ch, 2);
        CHANNEL_CLOSE(ch);
        0
    )));
    LET MUT(acc, 0);
    LET MUT(done, FALSE);
    WHILE(NOT(done),
        LET(x, CHANNEL_RECV(ch));
        IF(IS_ERR(x),
            SET(done, TRUE),
            SET(acc, +(acc, x)))
    );
    AWAIT(p);
    acc
))

// Structured cancellation with reason
SCOPE(FUN(() ,
    LET(h, SPAWN(FUN(() , WHILE(TRUE, YIELD()))));
    TASK_CANCEL(h, ["why": "stop"]);
    LET(r, AWAIT(h));   // ERR(kind="Cancelled", reason: [why: stop])
    IF(IS_ERR(r), ERR_PAYLOAD(r), r)
))
```

## Imports — spec §9.2

Three forms:

```
IMPORT(path, ["a", "b"])                  // bind exported names a, b
IMPORT(path, ["orig": "alias"])           // bind `orig` as local `alias`
IMPORT(path, [])                          // side-effect only
```

Path prefixes:
- `./` or `../` — filesystem relative (`.wll` suffix optional).
- `wlwl:std.*` — built-in namespaces (`collection`, `json`, `fs`, `test`, `io`, `format`, `ai`, `agent`).

Names from `wlwl:std.*` are NOT global — they must be IMPORTed before use. Errors: missing module `E0040`; cycle `E0041`; name not exported `E0023`; duplicate `E0021`. `MODULE_REF(path)` loads and returns the module dict **without** binding names.

Concurrency and OOP names **are** global (Appendix G) — do not import them.

## Error model — spec §8

**§8.2 transparent propagation**: any function call whose argument evaluates to `ERR` does not run the body; the `ERR` is forwarded. Pass plain values to non-consumer functions; consume `ERR` only via §8.3.

**§8.3 ERR consumers** (14 entries): `IS_OK`, `IS_ERR`, `UNWRAP_OR`, `OR_DIE` (deprecated — `W0051`), `TRY` (function-body only), `UNWRAP` (PANICs on ERR — `E0100`), `ERR_PAYLOAD`, `WRAP`, `TYPE`, `==`/`!=`, `IF` (condition position), `&&`/`||` (left side), `BOOL`, `EXPECT_ERR` (test-only — returns `OK(载荷)` on `ERR` input, `ERR(E0049)` on non-ERR input). v0.9 adds **no** new consumers — OOP and concurrency builtins all propagate.

**§8.4 PANIC**: `PANIC(msg)` (msg must be STRING or DICT) terminates with `E0100`. Bypasses propagation entirely. `UNWRAP` on ERR and `NEG(i64::MIN)` also PANIC.

**§8.5 top-level escape**: an uncaught ERR reaching the top level terminates with `E0102` and exit code 1. Always consume at the boundary.

**Concurrent ERR kinds** (§8.1): `ChannelClosed` · `Cancelled` (with `reason` dict). `ChannelWouldBlock` **removed in v0.9**.

**Host diagnostic vs user ERR** (§17.5): `AWAIT` of a child task re-raises *host* diagnostics (`E0101` stack overflow, invalid handle `E0053`, etc.) — distinct from user `ERR(...)` values which pass through as ordinary `RESULT` values. `SCOPE(ERR("x"))` propagates the `ERR` per §8.2 BEFORE the parameter-type check (`E0052` does not fire).

## Standard library pointers

For the full ~70-name catalogue see `reference.md` §9. Categories:

- **Global builtins (§10.2–§10.5 + §10.7 + §10.9 + §13–§17)**: `PRINT`, `PRINT_ERR`, `INPUT`, `LEN`, `TYPE`, `STR`, `INT`, `FLOAT`, `BOOL`, container ops, string ops, `FORMAT`, `AT_K`/`POP`, OOP (`CLASS`/`NEW`/`THIS`/`GET_PROP`/`SET_PROP`/`CALL_METHOD`), and the §17 concurrency set.
- **`wlwl:std.collection`** (§10.6): `MAP`, `FILTER`, `REDUCE`, `SORT`, `SORT_BY`, `RANGE`, `ZIP`, `ENUMERATE`, `TAKE`, `DROP`, `FLAT`, `UNIQ`, `GROUP_BY`, `ANY`, `ALL`, `FIND`, `JOIN`.
- **`wlwl:std.json`** (§10.8): `STRINGIFY`, `PARSE`.
- **`wlwl:std.fs`** (§10.8): `WRITE_FILE`, `READ_FILE`, `EXISTS`.
- **`wlwl:std.test`** (§10.10): `TEST`, `ASSERT`, `ASSERT_EQ`, `ASSERT_NEQ`, `EXPECT_ERR`, `RUN_TESTS`.
- **`wlwl:std.ai` / `wlwl:std.agent`** (§10.11): `TASK`, `MODEL`, `TOOL`, `CALL_TOOL`, `CONTEXT`.

## Build & runtime config — spec §9.1, §9.4

- `EXPORT([names])` at top level makes module bindings visible to importers.
- `wlwl.toml` (per-module, optional) declares `[features]`:

| Feature | Default | Effect |
|---------|---------|--------|
| `strict_types` | `false` | call-boundary type checks (`E0033`) |
| `allow_builtin_shadow` | `false` | shadow builtins with `W0030` instead of `E0025` |
| `strict_deadlock_detect` | `true` | deadlock L1 raises `E0065`; `false` downgrades to `W0065` + `E0053` |
| `native_channel_close` | `false` | post-close RECV raises `E0054` instead of `ERR(ChannelClosed)` |
| `channel_large_buf_threshold` | `1024` | `CHANNEL_NEW` buf above threshold → `W0066`; `0` disables |

## Top antipatterns

| # | Bug | Symptom | Fix |
|---|-----|---------|-----|
| 1 | Missing `MUT` on a binding later `SET`-ed | `E0024` cannot SET immutable | `LET MUT(name, value)` |
| 2 | Using `POP` for new code | Deprecation hint (`W0051`) | Prefer `AT_K(d, k, default)`; `POP` is the alias |
| 3 | Interpolating `ERR(x)` | The whole literal becomes `ERR` (§1.8) | Extract payload first: `LET(p, ERR_PAYLOAD(x)); "got: ${p}"` |
| 4 | `IF(cond, then)` two-arg form | `else` is `NULL` silently; ERR on `then` propagates without being routed | Use three-arg `IF(cond, then, else)` for ERR catching |
| 5 | `SET` outside `LET MUT` | `E0024` | Use `LET MUT` at the binding site, then `SET` |
| 6 | `RETURN`/`BREAK`/`CONTINUE` at top level or outside loop | `E0014` | Only inside a function body / loop body |
| 7 | `xs[i]` out of range | `E0036` | Bound-check first: `IF(>=(i, 0), IF(<(i, LEN(xs)), xs[i], default), default)` |
| 8 | Nested string literal inside `${...}` | `E0001` "nested string literal inside `${...}` interpolation is not allowed" | Bind the inner value to a LET first: `LET(v, ...); "outer ${v}"` |
| 9 | `FORMAT("…{0}…", [x])` | Renders the whole list at `{0}` (the list, not the first element) | Pass each placeholder as its own variadic arg: `FORMAT("…{0}…", x)` (§10.7) |
| 10 | Confusing `ERR` with falsy | Treating `IF(ERR("x"), "t", "f")` as "ERR is falsy" | ERR is its own propagation mechanism (§8.2); it's neither truthy nor falsy in §2.3 |
| 11 | `SPAWN` outside `SCOPE` | `E0058` | Always `SCOPE(FUN(() , … SPAWN …))` |
| 12 | `YIELD()` outside a task | `E0014` | Put `YIELD()` inside a `SPAWN` task body. Inside a task, any expression position is legal (v0.9) |
| 13 | Treating `NULL` from `TRY_RECV` as close | Infinite loop / lost close | Close is `IS_ERR` + `kind == "ChannelClosed"` |
| 14 | Expecting `ERR(ChannelWouldBlock)` from blocking SEND/RECV | v0.9: they **suspend** instead | Use `CHANNEL_TRY_SEND` / `CHANNEL_TRY_RECV` for non-blocking poll semantics |
| 15 | `AWAIT` of cancelled task left unhandled | `E0102` at top level | `IS_ERR` / `UNWRAP_OR` / `ERR_PAYLOAD` the AWAIT result (payload now has `reason`) |
| 16 | Builtin name as first-class value (`LET(f, +)` / `LET(f, PRINT)`) | `E0020` undefined name | User-defined functions only are first-class (§5.4). Write `FUN((a, b), +(a, b))` to get a callable. |
| 17 | `EXPECT_ERR(/(1, 0))` to catch integer / div-by-zero | Native code `E1003` (or `E0035` for overflow); `EXPECT_ERR` returns `ERR(E0049)` — not `OK(载荷)` | These are **native** codes per §11.2, not `RESULT`-shaped ERR; cannot be caught by any §8.3 consumer. |
| 18 | `SUB(s, 7, 5)` expected as end-index | Third arg is **length**: `SUB("Hello, world", 7, 5)` → `"world"` | Migration: `SUB(s, start, end_old)` → `SUB(s, start, -(start, end_old))` or `SLICE(s, start, end_old)` |
| 19 | `1.5e2` (or `1e-3` / `1E3`) as a float literal | All valid per §1.7 EBNF. Bare `1e` (no digits after) raises `E0001`. | — |
| 20 | `THIS()` twice in one method, or stored in a container/closure/return/SPAWN/AWAIT | `E0032` (linear THIS) | Consume `THIS()` once; keep it local to the method body |
| 21 | `SET_PROP(obj, "k", v)` from outside a self method | `E0032` | Write instance fields from inside `init` or a `self` method |
| 22 | `CALL_METHOD` out of protocol order | `E0051` (step) / `E0050` (after `end`) | Follow the session protocol; use `["choice", ...]` for alternatives |
| 23 | `TASK_CANCEL(h, "reason")` with a non-DICT reason | `E0066` | Pass a dict: `TASK_CANCEL(h, ["why": "stop"])` |
| 24 | Deadlock: 2+ tasks parked on channel ops with no peer | `E0065` (strict) / `W0065` + `E0053` | Add a peer, close the channel, or set `strict_deadlock_detect = false` |

## Verification loop

```bash
# 1. PRIMARY: does it run and produce expected output?
wlwl run path/to/file.wll

# 2. SECONDARY (best-effort): is the source already canonical?
wlwl fmt --check path/to/file.wll

# 3. For AI agents: machine-readable diagnostics.
wlwl run --format jsonl path/to/file.wll
```

`wlwl run` is the source of truth. `wlwl fmt --check` has known idempotency drift — failures on syntactically valid source are formatter quirks, not bugs in your code.

## References

- **Authoritative spec**: `../docs/standard/wlwl-spec-v0.9.md` — defer to this on any disagreement (§13–§16 = OOP + session types + linear THIS; §17 = concurrency).
- **v0.8 (archived)**: `../docs/history/wlwl-spec-v0.8.md` — §0–§12 core still valid (v0.9 is additive + concurrency/OOP upgrade over v0.8).
- **v0.7 (archived)**: `../docs/history/wlwl-spec-v0.7.md` — §0–§17 core still valid.
- **v0.6 (archived)**: `../docs/history/wlwl-spec-v0.6.md` — §0–§12 core still valid.
- **Lookup tables** (operators, error codes, AST shapes, OOP, concurrency): `reference.md` in this folder.
- **Built-in registry (single source of truth)**: `../docs/appendix_G.md`.
- **Gold concurrency fixtures**: `../impl/tests/concurrency/*.wll`.
