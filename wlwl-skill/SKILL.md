---
name: writing-wlwl
description: "Writes WLWL v0.11 .wll source files (spec docs/spec/wlwl-spec-v0.11.md). Covers v0.6 core (truthy overhaul, &&/|| short-circuit, IF ERR-routing, ! canonical, AT_K rename, string subscript, LET MUT, overflow->E0035, ${} interpolation), v0.7 §17 concurrency, v0.8 clarifications, v0.9 (true-suspension concurrency + OOP §13-§16 with session-type protocols and linear THIS), and v0.10 static contracts (all DEFAULT-OFF, runtime semantics unchanged; carried over unchanged into v0.11): type annotations FUN((a: INTEGER) : INTEGER, ...) plus container types ARRAY[T]/DICT[K,V]/OPTION[T]/RESULT[T,E] and bounded type variables `T: Comparable`; [features] gradual_typing = off|warn|error; module signature sidecars `foo.wll.sig` and `SEALED([...])`; MATCH exhaustiveness. New codes E0110-E0116 / W0110-W0117, all compile-time only. Use when the user asks for WLWL code, a .wll file, or anything targeting wlwl-spec-v0.11 (or v0.6/v0.7/v0.8/v0.9/v0.10 core). Do NOT use for v0.5 or earlier (POP-not-AT_K, no LET MUT), the Rust implementation, or the formatter."
---

# Writing WLWL (v0.11)

## When to load / NOT to use

**Use when:**
- The user asks for a `.wll` source file, a WLWL program, or a v0.6 / v0.7 / v0.8 / v0.9 / v0.10 / v0.11 idiom.
- A task targets `wlwl-spec-v0.11.md` (current) or any superseded spec (v0.6–v0.10, archived in condensed form under `../docs/history/`).
- Reviewing or debugging v0.6–v0.11 source (concurrency, OOP, session protocols, static contracts included).

**Don't use when:**
- The source targets WLWL v0.5 or earlier (different truthy rules, `POP`-not-`AT_K`, no `LET MUT`).
- The task is editing the Rust implementation in `impl/` or the formatter spec.
- The user wants `wlwl.exe` CLI documentation (that's `wlwl help` / `wlwl <subcmd> --help`).
- The artifact is a Rust test, ADRs, or build-system file.

## Writing flow

Tick these as you go:

```
WLWL writing progress (v0.10):
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
- [ ] 7. Static contracts wanted? (v0.10, ALL DEFAULT-OFF — runtime is
       unchanged either way). Annotate `LET(x: T, …)` / `FUN((a: T) : R, …)`,
       and flip `[features] gradual_typing = "warn" | "error"` in wlwl.toml.
       Use `SEALED([...])` + a `foo.wll.sig` sidecar only for library modules.
- [ ] 8. Write the .wll file (one stmt per line, semicolons, no leading indent)
- [ ] 9. Run `wlwl run <file>` — MUST exit 0 with expected stdout
- [ ] 10. If 9 fails: consult reference.md for the failing token/operator
```

Do not skip step 9. `wlwl run` is the source of truth. `wlwl fmt --check` checks canonical layout — read the W0053 rules in the Verification loop before dismissing it.

## Truthy / falsy — spec §2.3

Eight falsy values: `FALSE`, `NULL`, `0`, `0.0`, `""`, `[]` (empty array), `DICT()` (empty dict), `NaN`.

**Everything else is truthy**, including non-empty strings, non-empty containers, `OK(FALSE)`, `TASK`/`CHANNEL` handles, and `CLASS`/`INSTANCE` values.

`ERR(...)` is **not** in this list — it is its own propagation mechanism per §8.2. Passing `ERR(x)` to a non-consumer function transparently forwards the `ERR`; it does NOT make the function body "skip" because the arg was falsy. Only §8.3 consumers (`IS_OK`, `IS_ERR`, `UNWRAP_OR`, `OR_DIE`, `TRY`, `UNWRAP`, `ERR_PAYLOAD`, `WRAP`, `TYPE`, `==`/`!=`, `IF` condition, `&&`/`||`, `BOOL`, `EXPECT_ERR`) can inspect an ERR safely.

## Operators / builtins — cheat sheet (v0.6–v0.9)

**Equality** (spec §2.4): `=(a, b)` and `==(a, b)` are aliases (§1.5 call-position desugar; `=` triple-identity — NOT same as `INDEX_SET`'s `=` or `Param`'s `=`). `!=(a, b)`. Cross-type numeric: `==(1, 1.0)` is `TRUE`. Function/handle identity: `==(f, f)` and `==(h, h)` are `TRUE`. **v0.9 object identity**: `==(cls, cls)` and `==(obj, obj)` are `TRUE` for the same class/instance; two `NEW(...)` results are never equal.

**Comparison**: `<(a, b)`, `>(a, b)`, `<=(a, b)`, `>=(a, b)`.

**Arithmetic**: `+(a, b)`, `-(a, b)`, `*(a, b)`, `/(a, b)`, `%(a, b)`. `+` is also string concat and array concat. Integer overflow → native `E0035`; divide-by-zero → native `E1003` (note: **not** `E0009`; these are native codes per §11.2 — NOT ERR, cannot be caught by `EXPECT_ERR` / `UNWRAP_OR` / `TRY`; see anti-pattern #17).

**Boolean**: `&&(a, b)` / `||(a, b)` short-circuit; right side not evaluated when left decides. `NOT(x)` and `!(x)` are equivalent. `NOT(ERR(...))` propagates ERR (§4.3); safe coercion idiom: `NOT(BOOL(ERR(...)))` (BOOL is a §8.3 consumer).

**Indexing**: `xs[i]` returns one element; out-of-range array/string index raises `E0036`. `INDEX(xs, v)` **searches for the value `v`** and returns its **1-based** position, or `-1` if not found (NOT an error). ⚠ `INDEX` is the one index API that is **1-based** while `xs[i]` and `INDEX_GET(xs, i)` are **0-based** — the spec calls this the single easiest trap to get wrong. So `INDEX([7, 8, 9], 8)` → `2` but `INDEX_GET([7, 8, 9], 1)` → `8`. Never feed one API's result to the other.
> *Verified against wlwl 0.11.0 (2026-10-01): `INDEX([10,20,30], 10)` → `1`, `INDEX([10,20,30], 20)` → `2`, `INDEX([10,20,30], 99)` → `-1`, `INDEX_GET([10,20,30], 0)` → `10`, `[10,20,30][0]` → `10`.*

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
- `FUN((MUT), +(MUT, 1))` / `PRINT(MUT)` / `${MUT}` — all valid

**Everything is a prefix call — there is no infix syntax** (spec §4.3):
`+(a, b)`, never `a + b`. There is no binary-expression production in the grammar
at all, so `LET(x, a + b)` is a hard `E0011`. The only place `+` appears infix-like
is inside a string interpolation, and it desugars to a call.

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

## Static contracts — spec §2.6, §5.2, §5.2.1, §7.4, §9.1, §9.6 (v0.10)

**Everything in this section is default-OFF.** v0.10's runtime is byte-for-byte
v0.9's; you only see these diagnostics when a `wlwl.toml` opts in. A program that
does not use annotations and does not ship a `.sig` behaves exactly as before.

### Type annotations

Type annotation grammar enters the spec for the first time in v0.10 (v0.9 had
`name: Type` only as a table metavariable).

```wlwl
LET(x: INTEGER, 1);
LET(name: STRING, "wlwl");
LET(f, FUN((a: INTEGER, b: INTEGER) : INTEGER, +(a, b)));
```

`LET(name, value)` takes the annotation on the **binding**; `FUN` takes it on
each **parameter** and after the closing paren for the **return** type.

Type names: `INTEGER`, `FLOAT`, `BOOLEAN`, `STRING`, `CHAR`, `NULL`, plus the
container/function forms below. `CLASS` / `INSTANCE` name user classes.

**Containers** (§5.2):

```wlwl
FUN((xs: ARRAY[INTEGER]) : INTEGER, INDEX_GET(xs, 0))
FUN((d: DICT[STRING, INTEGER]) : INTEGER, AT_K(d, "k", 0))
FUN((o: OPTION[INTEGER]) : BOOLEAN, IS_OK(o))
FUN((r: RESULT[INTEGER, STRING]) : BOOLEAN, IS_OK(r))
```

`OPTION[T]` is **annotation sugar only** — there is no runtime `OPTION` type
(spec §2.1 has no such builtin); it desugars to `RESULT[T, NULL]`.

Note when writing `RESULT`-typed functions: an `ERR` reaching a **non-consumer**
parameter propagates transparently (§8.2), so a predicate like the one above
works on values that are already in hand, but `f(ERR("e"))` propagates rather
than returning `FALSE`. Consume with `IS_OK`/`IS_ERR` first.

**Bounded type variables** (§5.2.1, erased at runtime — `Value` is unchanged):

```wlwl
LET(max, FUN((a: T: Comparable, b: T: Comparable) : T: Comparable, IF(<(a, b), a, b)));
```

The constraint is written as `T: Comparable` — a **bare identifier** followed by
`:` and a bound name. `Comparable` is the only bound in v0.10. The variable is
resolved at the **call site**, where it is instantiated against the actual
argument types. Because the variable is erased at runtime, this changes no
runtime behaviour.

Attaching a constraint to a concrete type is an error, not a no-op:
`ARRAY[INTEGER]: Comparable` raises **`E0010`** ("a type constraint may only
follow a bare type variable … not a type like ARRAY[…] or DICT[…]") —
constraints constrain *variables*, not types. (Spec **§2.6** fact #3 said
`E0012` here; that is wrong twice over — `E0012` is a *syntax* error, and the
return-type-mismatch code is **`E0112`**. Observed `E0010`. Recorded in
v0.10.1.)

A constraint may not be **nested** either: `T: Comparable: Integer` raises
**`E0010`** ("a type constraint may not be nested"). The grammar allows at
most one constraint per type variable.

**Do not** use the arrow form `FUN(INTEGER) -> STRING` or the angle-bracket
form `DICT<STRING, INTEGER>`.

The two are **not** equally bad, which is worth knowing before you guess:

| Form | Result |
|---|---|
| `DICT<STRING, INTEGER>` | **parse error** `E0011 expected ')', got Gt` |
| `DICT<STRING>` (one arg) | **silently absorbed** into the type name `DICT < STRING >` |
| `FUN(INTEGER) -> STRING` | **silently absorbed** into `FUN ( INTEGER ) - > STRING` |
| bare `ARRAY` | **parse error** `E0010` |
| bare `DICT` / `OPTION` / `RESULT` | parses as a `Dynamic`-parameterised form, no error |

The silently-absorbed rows are the trap: no error, no check, and a type name
nobody defined. (Spec **§2.6** fact #2 used to claim all of them were silent;
it is not, and §2.6 has been corrected in v0.10.1.)

### The `gradual_typing` switch (§2.6)

**A `wlwl.toml` needs BOTH halves, and the `[package]` half is not optional
decoration.** Since v0.10.1 a `[features]`-only manifest is honoured (the loader
reads the feature table independently of `[package]`) — but if the manifest is
*malformed* you get `W0001`, and the static layer is easy to lose track of:

```toml
[package]          # ← all three, or this is not a manifest
name = "myapp"
version = "0.1.0"
entry = "main.wll"

[features]
gradual_typing = "error"        # "off" | "warn" | "error"
```

| What you wrote | What happens |
|---|---|
| No `wlwl.toml` at all | Static layer **never runs**. Zero diagnostics. This is the v0.9 behaviour and it is guaranteed (ADR-0020 S1) |
| `[features]` only, no `[package]` | **v0.10.1: honoured** — the switch still takes effect, *and* you get `W0001` saying the manifest isn't valid |
| Bad feature *value* (e.g. `"sideways"`) | `W0001` naming the bad key; switch falls back to `off` |
| Valid manifest | Switch takes effect |

> **v0.10 and earlier:** a `[features]`-only manifest was **silently swallowed** —
> the whole manifest layer went quiet, no `W0001`, exit code 0. The documented
> incantation produced a green build that checked nothing. Fixed in v0.10.1. If you
> hit `W0001` on a manifest you thought was fine, you are almost certainly missing
> `version` or `entry`.

Values:

| Value | Effect |
|-------|--------|
| `off` (default) | no static diagnostics at all |
| `warn` | annotation **and module-contract** mismatches become `W0110`–`W0115` |
| `error` | annotation **and module-contract** mismatches become `E0110`–`E0115` (blocking exit code) |

**`gradual_typing` governs `E0110`–`E0115`** (and their `W` twins `W0110`–`W0115`):
the three annotation codes (`E0110`/`E0111`/`E0112`) **plus** the three
module-contract codes of §9.6 (`E0113`/`E0114`/`E0115`). It does **not** govern
`E0116` (`MATCH` exhaustiveness, §7.4), which has its own switch below.

Module-contract checks need **both** a signature and the switch: a wrong
`foo.wll.sig` is **silent** with the key absent, and reports `E0113`–`E0115` with
`gradual_typing = "error"`. No signature = no contract = nothing to report
either way.
> *Verified against wlwl 0.11.0 (2026-10-01).* A `.wll.sig` declaring a name the
> module never exports: exit 0 and zero output with the key absent; `E0114`
> (exit 1) with `gradual_typing = "error"`. Same pattern for `E0113` (imported
> name absent from the signature — without the switch you get the unrelated
> runtime `E0023` instead) and `E0115` (signature type vs annotation conflict).
> The implementation's own note is `wlwl-types/src/diag.rs`: *"`gradual_typing`
> 管类型诊断（`E0110`-`E0112` + Step 6 的模块契约）"*.

`E0110`/`E0111`/`E0112` are the annotation mismatches; `E0113`/`E0114`/`E0115` are
module-contract mismatches; `E0116` is non-exhaustive `MATCH`. **All of them are
compile-time only** — none can be raised at runtime, and none is an `ERR`, so
`EXPECT_ERR` / `TRY` / `UNWRAP_OR` never catch them.

`match_exhaustiveness` is a separate `[features]` key for the `MATCH` checks
(§7.4). **Its default is "follow `gradual_typing`", not `off`** — with the key
absent it inherits whatever `gradual_typing` is set to. Write it explicitly if you
want it independent.

### Module signatures + `SEALED` (§9.1, §9.6)

A module may publish a **sidecar** signature file next to the source, named
`<module>.wll.sig`. One declaration per line:

```
EXPORT add (INTEGER, INTEGER) : INTEGER
EXPORT PI : INTEGER
```

The signature is the module's **whole public surface**. Two violation shapes,
both caught statically:

- `E0113` — `EXPORT`/`IMPORT` names a symbol the signature does not declare
  (or that `SEALED` does not include)
- `E0114` — the signature (or `SEALED`) declares a symbol the module does not
  `EXPORT`
- `E0115` — the signature's type disagrees with the implementation's annotation

`SEALED([...])` is a module-top-level declaration of the same idea without a
sidecar file. It is **not a keyword** — it is written in prefix-call form like
`CLASS(...)` / `NEW(...)`, so the spec's keyword table does not change in
v0.10 and the reserved-form set stays empty.

A module with **no** `.sig` and **no** `SEALED` behaves exactly as in v0.9.

### New diagnostics — v0.10

| Code | Meaning | Pair |
|------|---------|------|
| `E0110` | annotation mismatch (parameter / `LET` annotation vs actual type) | `W0110` |
| `E0111` | call mismatch (argument count, or the n-th argument's type) | `W0111` |
| `E0112` | return mismatch (`RETURN` value vs return annotation) | `W0112` |
| `E0113` | module contract: extra export/import not in the signature | `W0113` |
| `E0114` | module contract: declared but not exported | `W0114` |
| `E0115` | signature type conflicts with the implementation annotation | `W0115` |
| `E0116` | `MATCH` non-exhaustive (missing constructor, no default arm) | `W0116` |
| `W0117` | `MATCH` unreachable clause / dead default arm | **none — warning only, by design** |

`W0117` is **always** a warning and deliberately has **no** `E0117` partner.

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
2. **`YIELD` must be a statement in a task body — its value cannot be consumed.**
   The v0.7/v0.8 "direct child of a Block" restriction is **gone** (§17.1): `YIELD`
   is legal in any expression position *syntactically*. **But there is no
   continuation capture**, so what happens depends entirely on whether an outer
   expression *uses* its value. Measured on v0.10.1:

   | Form | Actual result |
   |---|---|
   | `LET(x, 7); YIELD(); x` — **`YIELD` as a statement** | ✅ `7`. The rest of the sequence runs; bindings made before the suspend point survive |
   | `LET(x, YIELD())` | ❌ the whole `LET` **never completes** — `x` is **not bound at all**; referencing `x` later → `E0020: undefined name` |
   | `IF(TRUE, YIELD(), 42)` | ❌ the whole `IF` collapses to **`NULL`**, *not* `42` |
   | `[1, YIELD(), 3]` | ❌ the whole literal collapses to **`NULL`**, *not* `[1, NULL, 3]` |
   | `+(YIELD(), 100)` | ❌ the whole call collapses to **`NULL`**, *not* `100` |
   | `YIELD()` inside a `WHILE` / `FOR` body | ❌ the loop runs **one round only** (counter ends at `1`, not `3`); statements after `YIELD` in the body do not run |
   | `LET(y, YIELD); y()` | ❌ `E0020: undefined name 'YIELD'` — `YIELD` is not a bindable value; there is no indirect form |

   **The rule:** *the expression containing a consumed `YIELD` does not complete —
   the whole outermost expression collapses to `NULL`.* It is the **outermost**
   expression that dies, not just the `YIELD` sub-position. Suspension happens,
   but the in-progress state at the suspend point (argument lists, branch choice,
   loop counter) is not captured, so there is nothing to resume into.

   **This produces no diagnostic at all** — a loop that runs one round and returns
   a plausible-looking number is the single easiest way to ship a silently wrong
   concurrent program. **Put `YIELD()` in statement position**, never inside an
   expression whose value you need.

   `YIELD` **outside** any task (top level, `SCOPE` body) → `E0014`.

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
// [v0.10.1 / R10-014] 两个要点,缺一不可:
//
//   1. `CHANNEL_RECV` **必须在任务里**。SCOPE 主体不是任务,在那里直接
//      `CHANNEL_RECV` 会报 `E0053: CHANNEL_RECV requires an active task`。
//      消费逻辑要自己 SPAWN 出来。
//   2. 通道**给缓冲**(`CHANNEL_NEW(8)` 这种),不要用 0。运行期把停泊的
//      任务从段首重跑,所以同一对任务在一条无缓冲通道上只能交接一次 ——
//      第二次会得到 `E0064`(详见下面的「无缓冲通道」小节)。
//
// 缓冲给够条数,发送侧就不会停泊,消费侧一次跑完循环。
SCOPE(FUN(() ,
    LET(ch, CHANNEL_NEW(8));
    LET(p, SPAWN(FUN(() ,
        CHANNEL_SEND(ch, 1);
        CHANNEL_SEND(ch, 2);
        CHANNEL_CLOSE(ch);
        0
    )));
    LET(c, SPAWN(FUN(() ,
        LET MUT(acc, 0);
        LET MUT(done, FALSE);
        WHILE(NOT(done),
            LET(x, CHANNEL_RECV(ch));
            IF(IS_ERR(x),
                SET(done, TRUE),
                SET(acc, +(acc, x)))
        );
        acc
    )));
    LET(total, AWAIT(c));
    AWAIT(p);
    total                        // → 3
))

// Structured cancellation with reason
SCOPE(FUN(() ,
    LET(h, SPAWN(FUN(() , WHILE(TRUE, YIELD()))));
    TASK_CANCEL(h, ["why": "stop"]);
    LET(r, AWAIT(h));   // ERR(kind="Cancelled", reason: [why: stop])
    IF(IS_ERR(r), ERR_PAYLOAD(r), r)
))
```

### 无缓冲通道(`CHANNEL_NEW(0)`)能做什么

无缓冲通道是真正的 rendezvous:**交接成功之前,两边都挂起**。v0.10.1 起
两种 spawn 顺序都跑得通(v0.9–v0.10 只有消费者先那条能跑),但有一硬限制:

> **同一对任务在一条无缓冲通道上只能交接一次。**
> 第二次会报 `E0064`。

原因值得知道,因为它决定了怎么写代码:运行期把停泊的任务从**段首重跑**,
不是从挂起点继续。循环里的计数器回到初值,投出去的还是同一个值 —— 状态
和上一轮完全相同,永远推不动。与其让用户拿到一个挂死的进程,不如给一条
能照着改的诊断。

于是:

| 想要的效果 | 怎么写 |
|---|---|
| 传若干条数据 | **缓冲通道**,容量给够条数(`CHANNEL_NEW(8)`),发送侧不挂起 |
| 严格的一次交接同步点 | 无缓冲通道 + 一对任务,各跑一次 |
| 「发 N 条收 N 条」走无缓冲 | 做不到,用缓冲通道 |

需要真正的多轮无缓冲传递,那是运行期的段内恢复,本版没有 —— 一旦要改
那个模型,§0.1「运行期语义与 v0.9 逐条一致」就不成立了。


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

**§8.3 ERR consumers** (13 **global** entries): `IS_OK`, `IS_ERR`, `UNWRAP_OR`, `OR_DIE` (deprecated — `W0051`), `TRY` (function-body only), `UNWRAP` (PANICs on ERR — `E0100`), `ERR_PAYLOAD`, `WRAP`, `TYPE`, `==`/`!=`, `IF` (condition position), `&&`/`||` (left side), `BOOL`. v0.9 adds **no** new consumers — OOP and concurrency builtins all propagate.

> **`EXPECT_ERR` is not a global.** It is an export of the standard-library
> test module: `IMPORT("wlwl:std.test", ["EXPECT_ERR"])`. It takes **one**
> argument, and it does **not** catch native codes — `E1003` divide-by-zero
> still escapes. Measured on v0.10.1: the bare global name → `E0020:
> undefined name EXPECT_ERR`; `EXPECT_ERR(1, 2)` → `E0022: expects 1
> argument(s), got 2`; `EXPECT_ERR(/(1, 0))` → `E1003` uncaught.

**§8.4 PANIC**: `PANIC(msg)` (msg must be STRING or DICT) terminates with `E0100`. Bypasses propagation entirely. `UNWRAP` on ERR and `NEG(i64::MIN)` also PANIC.

**§8.5 top-level escape**: an uncaught ERR reaching the top level terminates with `E0102` and exit code 1. Always consume at the boundary.

**Concurrent ERR kinds** (§8.1): `ChannelClosed` · `Cancelled` (with `reason` dict). `ChannelWouldBlock` **removed in v0.9**.

**Host diagnostic vs user ERR** (§17.5): `AWAIT` of a child task re-raises *host* diagnostics (`E0101` stack overflow, invalid handle `E0053`, etc.) — distinct from user `ERR(...)` values which pass through as ordinary `RESULT` values. `SCOPE(ERR("x"))` propagates the `ERR` per §8.2 BEFORE the parameter-type check (`E0052` does not fire).

## Standard library pointers

Categories (the per-namespace tables live in `reference.md` §9 and §25):

- **Global builtins (§10.2–§10.5 + §10.7 + §10.9 + §13–§17)**: `PRINT`, `PRINT_ERR`, `INPUT`, `LEN`, `TYPE`, `STR`, `INT`, `FLOAT`, `BOOL`, container ops, string ops, `FORMAT`, `AT_K`/`POP`, OOP (`CLASS`/`NEW`/`THIS`/`GET_PROP`/`SET_PROP`/`CALL_METHOD`), and the §17 concurrency set.
- **`wlwl:std.collection`** (§10.6): `MAP`, `FILTER`, `REDUCE`, `SORT`, `SORT_BY`, `RANGE`, `ZIP`, `ENUMERATE`, `TAKE`, `DROP`, `FLAT`, `UNIQ`, `GROUP_BY`, `ANY`, `ALL`, `FIND`, `JOIN`. **v0.11.2 adds** `CHUNK`, `WINDOW`, `DEDUP_BY`, `MIN_BY`, `MAX_BY`, `SUM`, `PRODUCT`, `FOLD_RIGHT`, `POSITION`, `KEY_BY` — see `reference.md` §9.4. ⚠ **Scale**: everything here that builds an array is **quadratic** in the language layer (immutable arrays + `PUSH` copies). `CHUNK` on 5 000 elements ≈ 0.6 s, `WINDOW` ≈ 20 s. `SUM` / `PRODUCT` / `KEY_BY` / `DEDUP_BY` / `FOLD_RIGHT` / `MIN_BY` / `MAX_BY` are near-linear. Measured table in stdlib §5.
- **`wlwl:std.json`** (§10.8): `STRINGIFY`, `PARSE`.
- **`wlwl:std.encode`** (stdlib §11, **v0.11.2 new**): `BASE64_ENCODE`, `BASE64_DECODE`, `HEX_ENCODE`, `HEX_DECODE`, `URL_ENCODE`, `URL_DECODE` — see `reference.md` §9.1. Encoding never fails; **decode failures are `ERR` values** with `kind = "DecodeError"`, not native diagnostics. Arity/type mistakes are `E0022`/`E0030`. `URL_DECODE` keeps `+` literal (RFC 3986, not form encoding); `BASE64_DECODE` accepts **both** alphabets and ignores CR/LF.
- **`wlwl:std.fs`** (§10.8): `WRITE_FILE`, `READ_FILE`, `EXISTS`.
- **`wlwl:std.str`** (stdlib §6, **v0.11 new**): `JOIN`, `SPLIT_LINES`, `CHAR_AT`, `COUNT`, `QUOTE` — see `reference.md` §25. `INDEX_OF`, `CONTAINS_SUB` added v0.11.2 (`INDEX_OF` is the only way to get a substring **position** — global `INDEX` is array-only). All three reject an **empty** `sub` with `E0030`.
- **`wlwl:std.text`** (stdlib §12, **v0.11.2 new**): `TO_UPPER`, `TO_LOWER` — full-Unicode simple case mapping, **no Unicode data files** (Rust's `char` tables). The global `UPPER` / `LOWER` are **ASCII-only and stay that way**: `UPPER("straße")` = `STRAßE`. Use `TO_UPPER` when non-ASCII must move. Length can change (`ß` → `SS`); Greek `Σ` lowercases to `ς` word-finally and `σ` medially.
- **`wlwl:std.math`** (stdlib §7, **v0.11 new**): `ABS`, `MIN`, `MAX`, `FLOOR`, `CEIL`, `ROUND`, `SQRT`, `POW`, `CLAMP`, `PI`, `E` — see `reference.md` §25.
- **`wlwl:std.test`** (§10.10): `TEST`, `ASSERT`, `ASSERT_EQ`, `ASSERT_NEQ`, `EXPECT_ERR` (**1 argument**), `RUN_TESTS` — all import-gated, none is a global. ⚠ **`ASSERT` changed in v0.11** — see `reference.md` §25.
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
| 18 | `SUB(s, 7, 5)` expected as end-index | Third arg is **length**: `SUB("Hello, world", 7, 5)` → `"world"` | Migration: `SUB(s, start, end_old)` → `SUB(s, start, -(end_old, start))` — **length = end − start**, so the subtraction order is `end_old` first. Do **not** wrap it in `NEG` (that re-negates the length and yields `""`). `SLICE` is **not** an alternative — it takes an `ARRAY` first arg and rejects strings with `E0030`. Note `-` is a 2-arg builtin only: `-(5)` is an `E0022`. See `examples/sub_migration.wll`. |
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

# 2. SECONDARY: is the source already in canonical form?
#    Read W0053 before dismissing it — see below.
wlwl fmt --check path/to/file.wll

# 3. For AI agents: machine-readable diagnostics.
wlwl run --format jsonl path/to/file.wll
```

**What `W0053` actually means, and when you may ignore it.**

`wlwl fmt --check` compares your source against the canonical form produced by
rebuilding the AST. Two rules decide whether a `W0053` is *yours* or the
formatter's:

- **Canonical form has no trailing `;` on the last statement** (spec **§A.3** —
  v0.11 moved the formatter out of the old §16.3 into the appendix).
  A one-statement file must be `PRINT("x")`, not `PRINT("x");`. Verified on
  wlwl 0.11.0: `LET( x ,1 );PRINT( x );` normalises to `LET(x, 1);` + `PRINT(x)`
  — the first statement keeps its `;`, the last one loses it.
- **`fmt` drops comments** (the AST rebuild does not preserve them) and
  tolerates them in `--check` mode, so a comment-heavy file usually still passes.
- **Line endings are irrelevant** since v0.10.1: CRLF and LF compare equal.

So: `W0053` on a file you wrote means the layout is off (usually a stray `;`, a
stray space, or two statements on one line) and is worth fixing. It is *not*
noise. The one case that is genuinely the formatter's fault is exotic whitespace
inside expressions; the fix there is still to let `wlwl fmt` rewrite the file and
read the result.

**Every `.wll` in `examples/` currently fails `fmt --check`.** They are readable
teaching material written by hand, not canonical files — `wlwl run` is what
matters for them. Do not use them as a style reference.

`wlwl run` remains the source of truth. `wlwl fmt --check` reports canonical
layout per the rules above — it is not "best-effort", and a `W0053` on your own
file is a real (if cosmetic) deviation.

## References

- **Authoritative spec**: `../docs/spec/wlwl-spec-v0.11.md` — defer to this on any disagreement (§13–§16 = OOP + session types + linear THIS; §17 = concurrency; §2.6/§5.2/§9.1/§9.6 = static contracts). Where §5.2.1's "三条实测事实" disagrees with the compiler (the angle-bracket / arrow forms, and the constraint-on-a-concrete-type code), **the compiler wins** — this bundle records the measured values.
- **Superseded specs (archived)**: v0.6–v0.10, condensed in `../docs/history/20260902-09.md` / `../docs/history/20260915-22.md` (full text via git history) — v0.11 is a *renumbering* of the spec (the formatter moved to §A.3) **plus real content**: two new stdlib namespaces and one breaking `ASSERT` change, both in `reference.md` §25. v0.10 added static contracts on top of v0.9's runtime; v0.9 added true-suspension concurrency + OOP §13–§16; the older archives' §0–§17 remain valid as history.
- **Validity window for "measured" claims**: anything in this bundle stated as *measured* / *实测* / *verified* carries the compiler version it was checked against (e.g. "Measured on wlwl 0.11.0 (2026-10-01)"). Those are observations of one release, not spec guarantees — when the version is not named, assume it predates v0.11 and re-run it. `wlwl run` on your own file is always the final word.
- **A code block that has not been run is a hypothesis (v0.11.2)**: two rules follow from the rule above, both learned the hard way. (1) A snippet using a namespace member must carry its own `IMPORT` line — a name is a global builtin **only** if it appears in `../docs/appendix_G.md`; `STRINGIFY` belongs to `wlwl:std.json` and `SORT` to `wlwl:std.collection`, so a snippet that names them bare dies with `E0020` and looks like a compiler bug. (2) Never write a diagnostic's message from memory — run it and copy the string. Excerpts in this bundle have shipped with both defects.
- **Lookup tables** (operators, error codes, AST shapes, OOP, concurrency): `reference.md` in this folder.
- **Built-in registry (single source of truth)**: `../docs/appendix_G.md`.
- **Gold concurrency fixtures**: `../impl/tests/concurrency/*.wll`.
