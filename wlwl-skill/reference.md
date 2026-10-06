# WLWL v0.11 Reference (on-demand)

> Loaded only when `SKILL.md` points here. This is a lookup catalogue,
> not a tutorial. Section numbers (`§1.5`, `§8.3`, `§10.4`, `§13`, `§17`, etc.)
> refer to `docs/spec/wlwl-spec-v0.11.md` (superseded specs v0.6–v0.10
> archived in condensed form in `docs/history/20260902-09.md` /
> `docs/history/20260915-22.md`; full text via git history).

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
- [§25. v0.11 增量备忘 — `std.str` / `std.math` / `ASSERT` breaking](#25-v011-)

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
- **Semicolons**: every statement needs one, `PRINT("a")` on its own line is not a
  statement until it is followed by `;` or end-of-file. The *last* expression of a
  `Program` or a `Block` may omit it (that is the block's value). So
  `LET(x, 1); PRINT(x)` is canonical and `LET(x, 1); PRINT(x);` is not.
- **No `{}` blocks.** A multi-statement body is a parenthesised sequence:
  `FUN((a), ( LET(b, 1); PRINT(b) ))`. Writing `{` gives `E0001: illegal
  character '{'`.
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

`EXPECT_ERR` is **not** in this list, and (as of v0.10.1) the **global** entry has
been **removed** from the registry for being unreachable. What remains is the
`wlwl:std.test` export — import it, it takes **one** argument, and it does
**not** catch native codes:

None of these can intercept native codes (`E0034`, `E0035`, `E1003`,
`E0100` PANIC, `E0102` top-level ERR escape, `E0052`–`E0066` host
diagnostics, `E0030`/`E0032` type / linear-THIS errors, etc.) — those
are not `RESULT` variants.

### §8.4 PANIC

`PANIC(msg)` / `UNWRAP(ERR)` / `NEG(i64::MIN)` → `E0100`.

### §8.5 Top-level escape

Uncaught ERR → `E0102`, exit 1.

## §9. Standard library catalogue (§10)

Full member lists live with the v0.11 namespaces: `std.collection` /
`std.json` / `std.fs` / `std.test` / `std.format` / `std.io` / `std.ai` /
`std.agent` / `std.encode` are listed in `SKILL.md` "Standard library pointers";
the **v0.11-new** namespaces `std.str` and `std.math` have signed tables in
§9.3 / §9.5 below, and the **v0.11.3-new** `std.sanitize` in §9.6. Concurrent
and OOP primitives are **global**, not under `wlwl:std.*`.
The authoritative registry of every global builtin (106 entries) is
`../docs/appendix_G.md`.

### §9.5 `wlwl:std.math` — the v0.11.2 twenty-one (stdlib §7.1)

| Member | Layer | Notes |
|---|---|---|
| `LN` / `LOG2` / `LOG10` | R2 | Natural / base-2 / base-10 log. `x < 0` → `ERR([kind: DomainError])`. **`LN(0)` is `-inf`, not an error** (matches C / Python / Go). |
| `EXP` | R2 | Natural exponential, total. Overflows to `+inf`. |
| `TRUNC` | R2 | Toward zero. **Not `INT`** — see below. |
| `SIN` `COS` `TAN` | R2 | Radians, total. |
| `ASIN` `ACOS` | R2 | Domain `[-1, 1]`; outside → `DomainError`. |
| `ATAN` | R2 | Total. |
| `ATAN2(y, x)` | R2 | Quadrant-aware. **Total** — `ATAN2(0, 0)` is `0.0`, not an error. |
| `SINH` `COSH` `TANH` | R2 | Total; overflow to `±inf`. |
| `POW_MOD(base, exp, mod)` | R2 | Modular power, square-and-multiply. `mod = 0` or `exp < 0` → `DomainError`. |
| `SIGN(x)` | R1 | `-1` / `0` / `1`. **Always `INTEGER`**, even for a float input. |
| `DIV_CEIL(a, b)` | R1 | Ceiling integer division. **Integers only** — `DIV_CEIL(7.0, 2)` is `E0030`, not a silent promotion. `b = 0` → `DomainError`. |
| `GCD(a, b)` / `LCM(a, b)` | R1 | Euclidean. **Absolute values**: `GCD(-4, 6)` is `2`. `LCM` with a zero is `0`. |
| `IS_SQRT(n)` | R2-adjacent (R1) | Integer square root, floored. `n < 0` → `DomainError`. |

Six things that are easy to get wrong:

1. **`TRUNC` is not `INT`.** Inside ±2^53 they agree exactly. Outside,
   `INT(1e20)` raises **E0035 and aborts the run**, while `TRUNC(1e20)`
   returns the value. That is the same reason `FLOOR` / `CEIL` / `ROUND`
   exist instead of being `INT` variants.
2. **`LN(0) = -inf` is a result, not an error.** Same convention as
   `EXP(1000) = +inf`.
3. **`DIV_CEIL`'s four sign combinations**: `(7,2)→4`, `(-7,2)→-3`,
   `(7,-2)→-3`, `(-7,-2)→4`. The rule is "+1 when there is a remainder
   **and both operands share a sign**" — the language's `/` truncates toward
   zero while `ceil` goes toward +∞.
4. **`POW_MOD` uses the Euclidean remainder**, so a negative base gives a
   non-negative result: `POW_MOD(-7, 3, 5)` is `2` (`-7 ≡ 3`, `3³ = 27 ≡ 2`),
   **not** Rust's `%` which would give `-1`.
5. **`POW_MOD` is a kernel, not a facade** — a facade cannot tell whether the
   square-and-multiply intermediate `m²` overflowed `i64`. It works for
   `mod = 2^61 - 1` (a Mersenne prime): `POW_MOD(2, 61, 2^61 - 1)` is `1`.
6. **Watch the inverse pairs.** `EXP`'s inverse is `LN`, *not* `LOG2`:
   `LOG2(EXP(3))` is `3·log2(e) ≈ 4.328`, not `3`.

> *Measured on wlwl 0.11.2 (2026-10-02):* `LN(EXP(2.5))` = `2.5`;
> `LOG2(1024.0)` = `10.0`; `TRUNC(1e20)` = `100000000000000000000.0`;
> `ATAN2(1.0, -1.0)` > π/2; `ATAN2(0.0, 0.0)` = `0.0`;
> `[DIV_CEIL(7,2), DIV_CEIL(-7,2), DIV_CEIL(7,-2), DIV_CEIL(-7,-2)]` =
> `[4, -3, -3, 4]`; `IS_SQRT(4611686018427387904)` = `2147483648`;
> `POW_MOD(2, 10, 1000)` = `24`; `POW_MOD(-7, 3, 5)` = `2`.

### §9.4 `wlwl:std.collection` — the v0.11.2 ten (stdlib §5.1)

| Member | Notes |
|---|---|
| `CHUNK(arr, n)` | Fixed-size blocks; **the tail block may be short**. `n > LEN(arr)` → one block. |
| `WINDOW(arr, n)` | Sliding windows, step 1, count `LEN(arr) - n + 1`. `n > LEN(arr)` → `[]`. |
| `DEDUP_BY(arr, key)` | Dedup by `key(v)`, **order-preserving** (first occurrence wins). |
| `MIN_BY(arr, key)` / `MAX_BY(arr, key)` | The **element**, not the key. Empty → `NULL`. |
| `SUM(arr)` / `PRODUCT(arr)` | Int/float promote per §2.2. Empty → `0` / `1` (identity). |
| `FOLD_RIGHT(arr, f, init)` | Right fold. **Argument order matches `REDUCE(arr, f, init)`.** |
| `POSITION(arr, pred)` | **0-based** index of the first match, else `-1`. |
| `KEY_BY(arr, key)` | Dict, **later wins** (the opposite of `GROUP_BY`). |

Four things that are easy to get backwards:

1. **`CHUNK` and `WINDOW` disagree when `n > LEN(arr)`** — one block vs zero
   blocks. That is the definitions, not a bug.
2. **`FOLD_RIGHT` takes `(arr, f, init)`** — `f` before `init`, same as
   `REDUCE`. `FOLD_RIGHT(["a","b","c"], f, "")` gives `"abc"`, `REDUCE` gives
   `"cba"`.
3. **`POSITION` gives an index, `FIND` gives an element.** Same predicate,
   different return type: on `[10,20,30]` with `x > 15` you get `1` and `20`.
4. **Keys are compared by their `STR` rendering**, the same rule as
   `GROUP_BY` — so integer `1` and string `"1"` collide as keys.

`n < 1` is `E0030` for both `CHUNK` and `WINDOW` (a size of `0` would make the
loop non-terminating). An **empty** `sub`-style key is not involved here, but
note the same rule in `std.str`: `INDEX_OF` / `CONTAINS_SUB` / `COUNT` all
reject an empty `sub`.

> ⚠ **Scale warning.** As of **v0.11.3**, `MAP` / `FILTER` / `CHUNK` / `WINDOW` /
> `ENUMERATE` / `ZIP` / `FLAT` / `JOIN` are **R2 and linear** — the language
> layer's `PUSH` copies the whole array, so the R1 versions were quadratic.
> Measured on wlwl 0.11.3 at **8 000 elements**: `MAP` 24 ms, `FILTER` 30 ms,
> `CHUNK` 3 ms, `WINDOW` 5 ms, `ENUMERATE` 3 ms, `ZIP` 4 ms, `FLAT` 4 ms,
> `JOIN` 3 ms (was 2 033 / 1 436 / 370 / 24 472 / 7 437 / 9 341 / 9 814 /
> 76 ms). `WINDOW`'s **output alone** is Θ(n·k) elements, so it is expensive by
> definition, not by accident.
>
> **Still quadratic** (same root cause, not yet re-layered): `SORT`, `SORT_BY`.
> On 5 000 elements `CHUNK` used to be ≈ 0.6 s and `WINDOW` ≈ 20 s. As of
> v0.11.3, `SORT` on **2 000** elements takes **9.4 minutes** — it is the one
> member re-layering would help most, but swapping in `Vec::sort_by` changes the
> comparator's **call count** (R1 calls it exactly C(n,2) times), which is
> observable to a side-effecting comparator; that is pending an owner decision.
> **`UNIQ` is a half-way case**: re-layered in v0.11.3 and 8.4× faster at 8 000
> elements (1 611 → 191 ms), but **still quadratic** — the `PUSH` rebuild went
> away, the linear *membership scan* did not, and that scan cannot be hashed
> without deciding hash semantics for `Value` (`1`/`1.0` compare equal, `Dict`
> compares unordered, closures compare structurally, and `NaN` is **not** equal
> to itself under `values_equal`, so `UNIQ([NaN, NaN])` keeps both).
> Note `DEDUP_BY` is a **different** dedup basis from `UNIQ`: its key is the
> `STR` **rendering**, so `DEDUP_BY([1, 1.0, 2], id)` keeps all three while
> `UNIQ([1, 1.0, 2])` keeps two.
> Near-linear already: `SUM`, `PRODUCT`, `FOLD_RIGHT`, `MIN_BY`, `MAX_BY`. Full
> table in `../docs/stdlib/wlwl-stdlib-spec-v0.11.md` §5 and
> `../impl/crates/wlwl-eval/benches/baseline.txt` **L0 / L0-A-2 / L0-A-3a / L0-A-3b** 段。

> *Measured on wlwl 0.11.2 (2026-10-02):* `CHUNK([1,2,3,4,5], 2)` =
> `[[1,2],[3,4],[5]]`; `WINDOW([1,2,3,4,5], 9)` = `[]`; `SUM([1, 2.5])` = `3.5`;
> `PRODUCT([])` = `1`; `MIN_BY([], f)` = `NULL`;
> `KEY_BY(["a1","b1","a2"], first-char)` = `[a: a2, b: b1]`.

### §9.2 `wlwl:std.text` (stdlib §12, v0.11.2)

| Member | Notes |
|---|---|
| `TO_UPPER(s)` | Full-Unicode simple uppercase. **Length may change**: `ß` → `SS`. |
| `TO_LOWER(s)` | Full-Unicode simple lowercase. Greek `Σ` → `ς` word-finally, `σ` medially. |

**The global `UPPER` / `LOWER` are ASCII-only and this is not fixed** — §12
deliberately keeps both rather than replacing a global (that would be
breaking). `UPPER("straße")` = `STRAßE`, `UPPER("héllo")` = `HéLLO`. Pick
`TO_UPPER` when non-ASCII must move; pick `UPPER` when you want the ASCII-only
behaviour on purpose. There is no Unicode data file involved — the mapping
comes from the host runtime's built-in `char` tables.

Deferred and *not* stubbed: `NFC` / `NFD` / `GRAPHEME_COUNT` / `WIDTH`. They
all need the same UCD bundle, so they are one decision, not four (§12.1). Do
not expect `a == NFC(a)`: `"é"` as one codepoint and as `e` + U+0301 are
**not** equal in wlwl, and that stays true until the bundle lands.

### §9.3 `wlwl:std.str` string search (stdlib §6, v0.11.2)

| Member | Notes |
|---|---|
| `INDEX_OF(s, sub, from?)` | 0-based **codepoint** index of the first `sub` at or after `from`; `-1` if none. `from` negative clamps to 0. |
| `CONTAINS_SUB(s, sub)` | Whether `sub` occurs. |
| `COUNT(s, sub)` | Already existed — non-overlapping occurrence count. |

The index is a **codepoint** offset, same frame as `SUB` / `CHAR_AT`, so
`SUB(s, INDEX_OF(s, x), LEN(x))` slices the hit straight back out. `from` is
what makes "find every occurrence" a loop instead of a full rescan from 0
each time.

> **Empty `sub` is an error, not a wildcard.** All three members raise `E0030`
> (`COUNT: sub must not be an empty string`, and likewise for the other two).
> `CONTAINS_SUB(s, "")` is **not** `TRUE` and `INDEX_OF(s, "")` does **not**
> return 0 — the three members share one rule, matching the pre-existing
> `COUNT` behaviour.

> *Measured on wlwl 0.11.2 (2026-10-02):* `INDEX_OF("hello","ll")` = 2;
> `INDEX_OF("hello","zz")` = -1; `INDEX_OF("hello","l",3)` = 3;
> `INDEX_OF("hello","l",-5)` = 2; `CONTAINS_SUB("hello","ell")` = TRUE;
> `TO_UPPER("straße")` = `STRASSE` (length 7); `TO_UPPER("héllo")` = `HÉLLO`
> while `UPPER("héllo")` = `HéLLO`.

### §9.1 `wlwl:std.encode` (stdlib §11, v0.11.3)

All eight members take/return `STRING` and work on its **UTF-8 bytes**.
`STRING` is not a byte buffer: a round trip preserves text, not arbitrary
binary — to carry binary, encode on this side and decode on the other.

| Member | Notes |
|---|---|
| `BASE64_ENCODE(s, url_safe?)` | RFC 4648. `url_safe=TRUE` swaps `+/` → `-_`. Never fails. |
| `BASE64_DECODE(s)` | Accepts **both** alphabets (wider than Go's two `Encoding`s) and ignores `\r` / `\n`. |
| `HEX_ENCODE(s)` | Lowercase, no separator. Never fails. |
| `HEX_DECODE(s)` | Accepts upper- and lowercase digits. |
| `URL_ENCODE(s)` | RFC 3986; unreserved set is `ALPHA / DIGIT / "-" / "." / "_" / "~"`. |
| `URL_DECODE(s)` | **`+` stays literal** — that is `application/x-www-form-urlencoded`, not RFC 3986. |
| `SHA256(s)` | **v0.11.3 new.** FIPS 180-4, lowercase hex. Never fails. |
| `HMAC_SHA256(key, s)` | **v0.11.3 new.** RFC 2104. Never fails. **Arity 2, key first.** |

**Hashing is not encryption.** `SHA256` is a digest, not a cipher: there is no
inverse, and it is deliberately **not** keyed. For keyed integrity use
`HMAC_SHA256`. Neither is a password hash — there is no salt, no work factor,
and no stretching; a password needs a dedicated KDF (none in this library).
`MD5` / `SHA-1` are **not** provided (both broken); SHA-3 / BLAKE3 are recorded
as future direction, not implemented.

**Failure split** (this is the part worth memorizing): encode and hash never
fail; decode failures are `ERR` **values** carrying `kind = "DecodeError"` with an
`op` and a `reason`. Arity and type mistakes stay native diagnostics
(`E0022` / `E0030`). A decode that yields invalid UTF-8 is a `DecodeError`
rather than a lossy `U+FFFD` substitution — so
`AT_K(ERR_PAYLOAD(BASE64_DECODE(s)), "kind", "?")` is the way to inspect it.
`AT_K` is **not** an ERR consumer; without `ERR_PAYLOAD` the ERR propagates
to `E0102`.

> *Measured on wlwl 0.11.2 (2026-10-02):* `BASE64_ENCODE("foobar")` =
> `Zm9vYmFy`; `URL_ENCODE("a b&c=d")` = `a%20b%26c%3Dd`; `URL_DECODE("a+b")` =
> `a+b`; `BASE64_DECODE("!!!")` → `ERR([kind: DecodeError, ...])`. Full vector
> table in `../docs/stdlib/wlwl-stdlib-spec-v0.11.md` §11.4; the base64
> expectations are copied from RFC 4648 §10, not from this implementation.
> `SHA256` / `HMAC_SHA256` vectors come from FIPS 180-4 and RFC 4231 **TC2**
> (TC1 / TC3 keys are binary, which a UTF-8 `STRING` cannot hold — those were
> cross-checked against Python `hashlib` instead).

#### Password hashing: `PBKDF2_ITER` / `ARGON2ID` (v0.11.3, M6/W-03)

`SHA256` / `HMAC_SHA256` are **fast digests, never password hashes**. For a
password you want the slow, parameterised members — and every parameter is
explicit, with no defaults:

| Member | Call | Bounds that are enforced (`E0030` outside) |
|---|---|---|
| `PBKDF2_ITER` | `PBKDF2_ITER(password, salt, iter, len, algo)` | `iter` ∈ [600 000, 10 000 000]; `len` ∈ [1, 1024] bytes; `algo` = `"sha256"` only |
| `ARGON2ID` | `ARGON2ID(password, salt, t_cost, m_cost, p_cost, len)` | `t_cost` ∈ [2, 32]; `m_cost` ∈ [19 456, 1 048 576] KiB; `p_cost` ∈ [1, 16]; `len` ∈ [4, 1024] bytes; decoded `salt` ≥ 8 bytes |

- The **lower bounds are OWASP 2025 minimums**, not recommendations: without
  them an all-explicit interface would still let a caller configure `iter = 1`,
  which is worse than not offering the member at all.
- **`salt` is a hex text string** (RFC salts are arbitrary byte strings; a
  wlwl `STRING` is UTF-8 text) — so the natural source is `RANDOM_HEX`, and
  the bounds are counted in bytes *after* hex decoding.
- **Memory:** `ARGON2ID` allocates about 19 MiB at the floor and is deliberately
  expensive. Never call it per-item in a hot loop.
- **No `secret` / `assoc` parameters on `ARGON2ID`.** An earlier plan added them
  so that the RFC 9106 §5.3 vector could be fed in; measurement showed the Rust
  crate's `keyid` / `data` path disagrees with the reference implementation
  there, and a pepper that no other implementation reproduces is worse than no
  pepper. See `addendum-04` §3.5.
- Adding a KDF **does not** mean the cryptography story is complete: symmetric
  encryption and key generation remain declined (spec §11.5 / §11.6).
- Vectors: `PBKDF2_ITER` cross-checked against Python `hashlib.pbkdf2_hmac`;
  `ARGON2ID` against a second implementation that first reproduces the Argon2
  reference KAT block for block. Neither expectation is this implementation's
  own output.

#### Entropy and constant-time comparison (v0.11.3, M6/W-04)

| Member | Call | Notes |
|---|---|---|
| `RANDOM_BYTES` | `RANDOM_BYTES(n)` | OS entropy. Returns an **array** of integers 0–255 — the only lossless byte carrier here. `n = 0` is legal; `n < 0` is `E0030`; no upper bound |
| `RANDOM_HEX` | `RANDOM_HEX(n)` | 2 `n` lowercase hex chars. **How you make a salt or a token** |
| `TIMING_SAFE_EQ` | `TIMING_SAFE_EQ(a, b)` | Constant-time over UTF-8 bytes. **Never fails**: unequal lengths give `FALSE`, not an error |

- **Never compare secrets with `=`** (or `==`): the comparison short-circuits and
  the timing leaks which prefix matched. `TIMING_SAFE_EQ` walks `max(len_a,
  len_b)` and folds the length difference into the same accumulator, so nothing
  branches on the result. Honest bound: timing still depends on *length* — what
  it hides is *which bytes differ*.
- Two draws of `RANDOM_HEX(16)` are independent; a member that cached one seed
  and replayed it would fail the contract case that compares them.
- Use `std.rand` (not `RANDOM_*`) when you want a **reproducible** sequence, and
  seed it from `RANDOM_HEX`.
- Entropy-source failure surfaces as `E0060` (IO error), distinct from `E0030`
  (you passed a bad argument) and from a `DecodeError` value (bad data).

### §9.9 `wlwl:std.regex` — linear-time patterns (stdlib §16, v0.11.3)

```wlwl
IMPORT("wlwl:std.regex", ["RE", "RE_SEARCH", "RE_REPLACE"]);

LET(pat, RE("(?i)([a-z]+)@([a-z]+)"));
PRINT(RE_SEARCH(pat, "x someone@example y"));   // [matched: …, start: 2, …]
PRINT(RE_REPLACE(RE("([a-z]+)=(\w+)"), "a=1 b=2", "$2:$1"));  // 1:a 2:b
```

- `RE(pattern)` returns a **pattern object** `[pattern, groups]` — a dictionary,
  because wlwl has no object type; `groups` is the capture count so callers can
  index without recompiling.
- **A syntax error aborts with native `E0030` including the column.** Not an
  `ERR` value: a bad pattern is a programmer error, the same line §11.1 draws for
  the rest of `std`.
- **A non-match returns `NULL`, never `ERR`.** Regex "did not match" is a normal
  business branch; making it an `ERR` would force `ERR_PAYLOAD` at every call site.
- ⚠ **These are rejected, and that is the point** — `(?=…)` / `(?<=…)`,
  backreferences (`\1`, `(?P=…)`), named groups, `\p{…}`. Each of them can blow up
  exponentially, and **linear time is this member's only reason to exist**: `(a*)*b`
  over 10 000 `a`s finishes instantly here and never finishes in a backtracker.
- ⚠ **Semantics are leftmost-first, not leftmost-longest.** This matches RE2's
  *default* (and Perl / Python / JS / Java); leftmost-longest is RE2's POSIX mode.
- `(?m)` here means **line anchors** (the Perl/Python sense). RE2's `(?m)` means the
  opposite — do not copy docs from RE2 on that one flag.
- `\w` `\d` `\s` and `(?i)` are **ASCII-only**; use `std.text` for Unicode folding.
- `TRY_RE` does not exist yet: a pattern that comes from user input cannot be
  "trial-compiled" without aborting.

### §9.10 `wlwl:std.rand` — explicitly seeded randomness (stdlib §17, v0.11.3)

```wlwl
IMPORT("wlwl:std.rand", ["SEED", "UNIFORM", "INT_RANGE", "CODEPOINT"]);

// ⚠ THE draw members return [value, next_state] — not a bare scalar.
LET(r0, SEED("order-2026-10-06"));
LET([n, r1], INT_RANGE(r0, 1, 7));      // die roll
LET([u, r2], UNIFORM(r1));              // [0, 1)
LET([s, r3], UNIFORM(r2, 10.0, 20.0));  // [10, 20)
PRINT(n, u, s);
```

- ⚠ **You must thread the state back.** `DICT` is **immutable** (spec §2.1) and
  `SET_PROP` / `INDEX_SET` on a dict *return a new dict, receiver unchanged*
  (§13.4). A draw member that returned only a scalar would therefore advance
  **nothing** — the same `rng` value would yield the same number forever. Because
  the new state comes back **with** the value, "forgot to advance" is not an
  available idiom: you cannot get the value without also getting the state.
  Use wlwl's existing array-pattern binding (§7.3) — that is what the `[a, b]`
  form in `LET` is for.
- **The state is an ordinary value, and that is the point.** `SEED(entropy)`
  makes a value; every member's first argument receives it. There is **no
  `SET_SEED()`, no "current global RNG", no hidden state anywhere.** Two chains
  built from the same entropy produce byte-identical output — which is what makes
  `std.rand` reproducible, and is what the contract table freezes.
- ⚠ **Entropy comes only from an explicit argument.** `SEED` does **not** read the
  OS entropy source itself. For real randomness take it from `std.encode` and pass
  it in: `SEED(RANDOM_HEX(16))` (§9.8). `std.encode` is the source; `std.rand`
  consumes it one-way. For an educational language whose selling point is
  reproducibility, *visible* dependencies are a feature.
- **`std.rand` is not cryptographic.** Salts, tokens and key material go through
  `RANDOM_BYTES` / `RANDOM_HEX` in `std.encode`. Two namespaces, two jobs, not
  merged.
- `UNIFORM` takes exactly **1 or 3** arguments; **2 arguments is `E0022`**, *not*
  "`a` defaulted". The unit interval is half-open and the upper bound is
  **guaranteed** unreachable (53-bit mantissa), not "happens not to happen".
  `UNIFORM(rng, a, b)` accepts integer endpoints and coerces them.
- `INT_RANGE` is **unbiased** (modulo-with-rejection) and **never goes through
  `FLOAT`** — a float has a 53-bit mantissa, so a float round-trip both biases and
  silently narrows ranges above 2^53. `lo >= hi` is an empty interval → `E0030`.
- `CODEPOINT` exists as its own member because a code point is **not** a uniform
  distribution: UTF-16 surrogates (`0xD800..=0xDFFF`) and Unicode noncharacters
  are excluded. It returns a **single-character string**.
- ⚠ **`std.rand`'s home in `wlwl:std.*` is a transitional form.** Spec §14 already
  records that `std.ai` / `std.agent` should become official packages outside std;
  the same logic applies here once the official-package mechanism exists. Do not
  treat its placement as permanent.
- Only uniform distributions. There is no `NORMAL` / Poisson / exponential and
  there is not planned to be — build those yourself from `UNIFORM` (polar /
  inverse-transform) when you need them; that is your algorithm choice.

### §9.6 `wlwl:std.sanitize` — the v0.11.3 three (stdlib §13)

**Pick by output context. They are not interchangeable, and choosing the wrong
one is an error, not caution:**

| You are writing | Use | Why not the others |
|---|---|---|
| `href="…"` / `src="…"` — a **URL** | `URL_ENCODE` (§9.1) | `HTML_ESCAPE` leaves `&` readable and does not percent-encode; `HTML_SANITIZE` will strip the attribute entirely if the scheme isn't on the list |
| text or a quoted attribute — **markup** | `HTML_ESCAPE` | `HTML_SANITIZE` deletes every tag you wanted to keep |
| **rich HTML** from an untrusted source | `HTML_SANITIZE` | `HTML_ESCAPE` shows the user the tags instead of removing them |

| Member | Notes |
|---|---|
| `HTML_ESCAPE(data)` | Escapes **exactly five** ASCII chars: `&` `<` `>` `"` `'` (apostrophe → `&#39;`). Non-ASCII passes through byte-identical. |
| `HTML_UNESCAPE(data)` | **Full** WHATWG table (2125 semicolon + 106 legacy entries). `&copy;` really decodes to `©`. Unknown named entities stay **verbatim** — that is HTML5's own tolerance, not a bug. |
| `HTML_SANITIZE(data, policy?)` | Whitelist sanitizer. `policy` is an optional `DICT`; omit it for the built-in conservative one. |

**`HTML_SANITIZE` behaviour, in the order you will hit it:**

- Tags not on the whitelist are **removed but their text is kept** (unwrapped).
  `<div onclick="x">y</div>` → `y`. The tag goes, the words stay.
- `script` / `style` / `iframe` / `textarea` / `title` / `xmp` / `noembed` /
  `noframes` (plus `object` / `embed` / `noscript` / `template` / `svg` /
  `math`) are removed **with their contents** — their inner text is raw text
  that must never be re-parsed as markup.
- Attributes are filtered **per tag** (`a` allows only `href` and `title` by
  default). A URL attribute survives only if its scheme is on the list
  (`http` / `https` / `mailto`; **relative URLs always pass**). When a tag is
  unwrapped, its attributes go with it.
- **Idempotent, not round-trip.** `HTML_SANITIZE(HTML_SANITIZE(x, p), p) ==
  HTML_SANITIZE(x, p)` byte-for-byte (a normative invariant). But the output
  bears **no** positional relation to the input — nesting is corrected and
  attributes are dropped. Do not try to map one back to the other.

**Escaping gotchas:**

- `HTML_ESCAPE(HTML_ESCAPE(s))` ≠ `HTML_ESCAPE(s)` — the second pass eats the
  `&` of `&amp;`. The **round-trip theorem runs the other way only**:
  `HTML_UNESCAPE(HTML_ESCAPE(s)) == s` for every `s`.
- Use a **double quote** for an attribute you escaped with `HTML_ESCAPE`; the
  five-char set already covers `"`, but single-quoted attributes are the caller's
  problem.
- `HTML_SANITIZE` escapes text on output with the same five characters, so a
  sanitized document never needs a second pass.

**Never fails on content.** The only diagnostics are `E0022` (arity — 1 or 2
arguments) and `E0030` (non-string `data`, or a `policy` that is not a `DICT` /
has a wrongly-typed key). Malformed HTML, unclosed tags, stray `<` — all
recovered silently, because a sanitizer that throws on hostile input is a
denial-of-service vector.

> *Measured on wlwl 0.11.3:* 10 KB article **17.05 MiB/s**; per-byte cost falls
> 114.0 → 41.6 ns/B from 1 KB to 100 KB (linear, no superlinear growth);
> 100 000-deep nesting completes in 60.87 ms without stack overflow. The
> entity table is the point of the full table: a half table is not a "known
> limitation" in a security context, it is **wrong data handed to the caller**.

## §10. Error codes (§11.2, §11.3)

### Errors (E-codes) — v0.9 additions / changes highlighted

| Code | Meaning |
|------|---------|
| `E0010` | type-annotation grammar error (`ARRAY` without `[...]`, `a type constraint may only follow a bare type variable`) |
| `E0011` | **parse** error — expected `)` / `,` / `;` and got something else |
| `E0012` | **parse** error — the `,` after a closure's parameter list is missing (`FUN((a, b) ...)` is fine; `FUN (a, b),` is not) |
| `E0013` | expected `;` after a statement (single-statement programs also need one, see §1) |
| `E0014` | illegal RETURN/BREAK/CONTINUE **or YIELD outside a task** (v0.9: YIELD-Block-direct-child restriction removed) |
| `E0020` | **undefined name** — a name is not bound. The one to read first when a program "should" work |
| `E0022` | arity mismatch (too many/few arguments) |
| `E0024` | SET on immutable binding (single-task and cross-task alike) |
| `E0025` | cannot shadow a built-in that is not a context keyword |
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
| `E0064` | **[v0.10.1]** **livelock guard** on an unbuffered channel: the same task pair rendezvoused a second time. Distinct from `E0065` — the peer *is* there, but a parked task is restarted from the top of its body, so the replay can never make progress. Use a buffered channel (`CHANNEL_NEW` with n > 0) or hand off exactly once |
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

`W0001` **manifest problem** — `wlwl.toml` is malformed (typically a missing `version` or `entry` in `[package]`). v0.10 swallowed this silently; v0.10.1 reports it. The `[features]` table is still applied if it parsed, so a `[features]`-only manifest both works *and* warns |
`W0010` unused LET · `W0011` unused param · `W0012` duplicate LET ·
`W0013` IF branches inconsistent ·
`W0020` mixed dict literal · `W0030` shadow ·
`W0051` deprecated alias · `W0053` canonical-form deviation (see §24.4) ·
**`W0065`** deadlock L1 soft warning (`strict_deadlock_detect = false`) ·
**`W0066`** `CHANNEL_NEW` large buf (above `channel_large_buf_threshold`) ·
**`W0110`–`W0116`** the `gradual_typing = "warn"` tier of `E0110`–`E0116` ·
**`W0117`** `MATCH` unreachable clause / dead default arm (**no `E` twin, ever**).

**Two codes that do not exist**: `W0015` (integer overflow saturated+warn) and
`W0040` (unhandled `TODO(agent):`). The spec (§11.2) explicitly declines to
define them — integer overflow *raises* `E0035`, there is no saturating path to
warn about, and the lexer does not retain comment text, so there is no input to
check. Do not expect either from the compiler.

### Exit codes (§11.4)

| Code | Meaning |
|------|---------|
| `0` | ok |
| `1` | diagnostic — runtime (`E0020` undefined name, `E0022` arity, `E0100` PANIC, `E0102` top-level `ERR`, `E0065` deadlock, …) **or** a static contract caught by the gate (`E0110`–`E0116`) |
| `2` | command-line usage error — unknown subcommand, missing required argument, mutually exclusive options |
| `3` | **an input file failed to parse** — exactly the 7 parser codes `E0001` `E0002` `E0003` (lexical) and `E0010` `E0011` `E0012` `E0013` (syntax). A malformed `.wll.sig` also lands here |
| `101` | implementation crash — a bug, not a user error |

`check` and `run` share this table, so they must agree on parse failures.
`E0014` is **not** in the `3` bucket: it is an evaluation-position error
(`YIELD`/`RETURN` misplaced) in an otherwise legal file, so the exit code is `1`.
> *Measured on wlwl 0.11.0 (2026-10-01): `wlwl bogus` → `2`; `wlwl check` with no
> `<FILE>` → `2`; unparsable source → `3`; `PRINT(NOPE)` under `run` → `1`.*

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
**1-based** position of `v`, or `-1` if missing (not an error). `INDEX_SET` rejects STRING receiver
(`E0030`).

**⚠ `INDEX` is 1-based; `xs[i]` and `INDEX_GET` are 0-based.** The spec
(§4.5) calls this the easiest trap in the section: `INDEX([7, 8, 9], 8)` → `2`,
while `INDEX_GET([7, 8, 9], 1)` → `8`. The two bases are deliberately opposite,
so never round-trip a position from one into the other.
> *Measured on wlwl 0.11.0 (2026-10-01):* with `xs = [10, 20, 30]` —
> `INDEX(xs, 10)`=`1`, `INDEX(xs, 20)`=`2`, `INDEX(xs, 99)`=`-1`,
> `INDEX_GET(xs, 0)`=`10`, `INDEX_GET(xs, 1)`=`20`, `xs[0]`=`10`, `xs[1]`=`20`.

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
Third arg is **length**, not end-index. Migration: `SUB(s, start, end_old)` → `SUB(s, start, -(end_old, start))`. The subtraction order matters: length is `end_old − start`, so `-(end_old, start)`, **not** `-(start, end_old)` — the latter yields a negative length and `SUB` returns `""`. `-` is a 2-arg builtin only (`-(5)` is an `E0022`).

Two ways to get this wrong, both of which have shipped in this file before:

- **Do not wrap the result in `NEG`.** `NEG(-(end_old, start))` re-negates the length you just
  computed correctly, so the third argument goes negative and `SUB` returns `""`.
- **`SLICE` is not an alternative.** `SLICE(arr, start, end?)` takes an **`ARRAY`** first arg
  and rejects a string with `E0030`. Its start/end semantics apply to arrays only; for a string
  the only rewrite is the `SUB` form above.

> *Measured on wlwl 0.11.1 (2026-10-02): `SUB("Hello, world", 7, 5)` = `"world"`;
> `SUB("Hello, world", 7, -(12, 7))` = `"world"`; `SUB("Hello, world", 7, NEG(-(12, 7)))` = `""`;
> `SLICE("Hello, world", 7, 12)` = `E0030: SLICE: expected ARRAY as first arg, got string`.*
> Runnable form: [`examples/sub_migration.wll`](examples/sub_migration.wll).

**FORMAT templates** (§10.7): `FORMAT(template, args...)`. `{N}` is the
`N`th variadic arg (template is index -1). `{name}` is the first DICT
arg's key. Mixed `{N}` and `{name}` placeholders see the first DICT
arg at `{0}`. Unmatched `{...}` preserved literally; malformed template
(unmatched `{`) raises `E0039`. **Important**: `FORMAT("head={0} tail={1}",
x, y)` is correct; `FORMAT("head={0} tail={1}", [x, y])` renders the
whole list at `{0}` and leaves `{1}` literal — variadic, NOT
single-list arg.

## §13. Container immutability (§3.3, §10.4)

`PUSH` / `POP` / `UNSHIFT` / `SHIFT` / `CONCAT` / `SLICE` / `REVERSE` /
`INDEX_SET` / `REMOVE_KEY` / `DEL` / `MERGE` all return a **new** container and
leave the receiver untouched — rebind it: `LET(xs, PUSH(xs, 1));`.

The only in-place mutation in the language is `SET` on a `LET MUT` binding.
(Instance fields are the second path, and only from inside a method with a `self`
first parameter — see §14.)

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
- **"Sanitize the input" makes HTML safe** — FALSE, and OWASP calls input-side
  cleaning a last resort. `HTML_SANITIZE` belongs at the **output** point, for
  rich HTML only. For plain text or an attribute, use `HTML_ESCAPE`; for a URL,
  `URL_ENCODE`. See §9.6 — the three are not interchangeable.
- **`HTML_SANITIZE` "keeps what isn't dangerous"** — FALSE for tags: an
  off-whitelist tag is **removed** (only its text survives), and
  `script` / `style` / `iframe` go **with their contents**. It never round-trips:
  output bytes bear no relation to input.
- **Double-escaping with `HTML_ESCAPE`** — `HTML_ESCAPE(HTML_ESCAPE(s))` differs
  from `HTML_ESCAPE(s)`; the theorem only runs `UNESCAPE ∘ ESCAPE`. See §9.6.
- **`SHA256` is a password hash / a cipher** — FALSE. It is an unkeyed digest
  with no salt and no work factor; use `HMAC_SHA256` for keyed integrity, and
  note that neither is a KDF. `MD5` / `SHA-1` are not provided. See §9.1.

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

Every v0.9 item that affects writing `.wll` source — pulled from the
v0.9 spec's 附录 D (archived in condensed form in
`docs/history/20260915-22.md`; full text via git history).

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
`docs/spec/wlwl-spec-v0.11.md` 附录 D (unchanged from the v0.10 edition).

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

### §24.1 Annotation grammar traps (measured against the reference impl, re-verified on wlwl 0.11.0 / 2026-10-01)

1. **`ARRAY` requires brackets.** A bare `ARRAY` is `E0010`. But a bare
   `DICT` / `OPTION` / `RESULT` parses as an *opaque named type* with **no
   error** — verified on wlwl 0.11.0: `LET(d, FUN((x: DICT) : INTEGER, 1))` runs clean.
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
   > `E0010`. The spec's code is doubly wrong: `E0012` is a *syntax* error, and
   > the return-type-mismatch code is **`E0112`** (static contract, §11.2).

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
| `gradual_typing` | `off` | `warn` → `W0110`–`W0115`; `error` → `E0110`–`E0115` (blocking) — annotations **and** module contracts |
| `match_exhaustiveness` | **follows `gradual_typing`** | `E0116`/`W0116` non-exhaustive + `W0117` unreachable clauses |

**`gradual_typing` governs `E0110`–`E0115`** — *not* `E0116`. The first three are
the annotation mismatches; the second three are the module-contract mismatches
(§9.6). `E0116` follows the other switch. Implementation note
(`wlwl-types/src/diag.rs`): *"`gradual_typing` 管类型诊断（`E0110`-`E0112` +
Step 6 的模块契约）"*. (An older doc comment in `main.rs` said only
`W0110`-`W0112` / `E0110`-`E0112`; that comment undercounts and is not the
behaviour.)
> *Measured on wlwl 0.11.0 (2026-10-01), all three module-contract codes, each
> with a deliberately wrong `foo.wll.sig` and no other change:*
> - `E0113` — importer names a symbol the signature does not declare. Key
>   absent → the unrelated **runtime** `E0023`; key on → `E0113`, exit 1.
> - `E0114` — signature declares a name the module never exports. Key absent →
>   **exit 0, zero output**; key on → `E0114`, exit 1.
> - `E0115` — signature type conflicts with the implementation annotation.
>   Key absent → **exit 0**; key on → `E0115`, exit 1.

Practical consequence: a manifest that writes only
`gradual_typing = "error"` has **`E0116` switched on as well**, whether or not
you intended it. Write both keys explicitly when you care about the difference.

`E0113`–`E0116` are **compile-time only**. None is an `ERR`, so `EXPECT_ERR` /
`TRY` / `UNWRAP_OR` cannot catch them.

### §24.3 CLI additions (v0.10)

| Command | What it does |
|---------|--------------|
| `wlwl check <file>` | parse + unused-binding warnings (`W0010`–`W0012`) + static-contract diagnostics when the manifest enables them. It **walks the import graph**, so it is where module signatures (`E0113`–`E0115`) actually surface. **It does NOT do name resolution** (spec §11.4) — `wlwl check` on `PRINT(NOPE)` exits 0; only `wlwl run` reports `E0020`. Not to be confused with `wlwl run` |
| `wlwl ast <file>` | dump the parsed AST (text or jsonl) — useful when a parse error points somewhere surprising |
| `wlwl sig <file>` | print the module's signature (text or JSON) |
| `wlwl sig-gen <file>` | write/refresh the `*.wll.sig` sidecar |
| `wlwl interface <file>` | export the public surface as JSON |
| `wlwl schema` | type system + constraints + static-contract codes as JSON |
| `wlwl lsp` | stdio JSON-RPC thin shell: `diagnostics` / `definition` / `hover` + registry-driven completion (no `rename`, no `format`) |

`wlwl fmt` / `wlwl fmt --check`: canonical form per spec **§A.3** (v0.11 moved
the formatter out of the old §16.3 into appendix A.3 — "§16.3" no longer
resolves). **`--check`
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

### §24.5 Spec / impl / docs 三处引用 (v0.11 current)

| 文档 | 路径 | 用途 |
|------|------|------|
| 权威规范 | `../docs/spec/wlwl-spec-v0.11.md` | 唯一真相源 |
| 历史归档(v0.6–v0.10 精简) | `../docs/history/20260902-09.md` / `20260915-22.md` | 旧版上下文(原文在 git 历史) |
| 内建注册表 | `../docs/appendix_G.md` | 106 条内建单一真相源 |

### §24.6 Standard library spec

`../docs/stdlib/wlwl-stdlib-spec-v0.11.md` — per-namespace signatures, `std.str`
§6, `std.math` §7, `std.test` §8.

---

## §25. v0.11 增量备忘 — `std.str` / `std.math` / `ASSERT` breaking

v0.11 renumbers the spec (the formatter moved to **§A.3**) *and* adds real
content: two new import-gated namespaces and one **breaking** change to
`ASSERT`. Nothing here is reachable from the v0.10 notes above.

Both namespaces are `IMPORT`-gated like every other `wlwl:std.*` member — none
is a global builtin, so an unqualified call is `E0020: undefined name`
(that includes `MIN` / `MAX`, which are easy to assume are global; they are not).

### §25.1 `wlwl:std.str` — string extensions (stdlib §6, new in v0.11)

Index base is the same as `SUB`: codepoints, and **negative counts from the
tail**, so `CHAR_AT("abc", -1)` is `"c"` rather than out-of-range.

| Signature | Behaviour | Failure |
|-----------|-----------|---------|
| `JOIN(arr, sep) -> STRING` | each element rendered with `STR`, joined by `sep`; empty array → `""` | — |
| `SPLIT_LINES(s) -> ARRAY` | split on `\n`, strip a trailing `\r`; a trailing newline yields no empty tail element; `""` → `[]`; no newline → `[s]` | — |
| `CHAR_AT(s, i) -> STRING` | the `i`-th codepoint; equivalent to `SUB(s, i, 1)` | out-of-range as `SUB` |
| `COUNT(s, sub) -> INTEGER` | **non-overlapping** occurrences of `sub` | `sub` is `""` → `E0030` |
| `QUOTE(s) -> STRING` | wraps in double quotes; escapes inner `"` and `\` as `\"` / `\\`, and `\n` / `\t` / `\r`; everything else verbatim | — |

> *Measured on wlwl 0.11.0 (2026-10-01):* `JOIN([1,2,3], "-")` → `"1-2-3"` ·
> `SPLIT_LINES("a\nb")` → `[a, b]` · `CHAR_AT("abc", -1)` → `"c"` ·
> `COUNT("ababab", "ab")` → `3` (non-overlapping).

### §25.2 `wlwl:std.math` — math basics (stdlib §7, new in v0.11)

`ABS` / `MIN` / `MAX` / `FLOOR` / `CEIL` / `ROUND` / `CLAMP` and the constants
are pure-wlwl facades; `SQRT` / `POW` need float instructions and come from the
R2 float kernel. Transcendental families (`sin`/`cos`/`tan`/`exp`/`log`) are
**not** in this version. Arithmetic promotion and overflow follow spec §2.2;
**domain violations return an `ERR`** (`["kind": "DomainError", …]`) and wrong
argument types are `E0030`.

`MIN` / `MAX` / `CLAMP` promote their **return** type too: if any argument is
`FLOAT` the result is `FLOAT` (`CLAMP` considers **all three**).

| Signature | Behaviour | Failure |
|-----------|-----------|---------|
| `ABS(x)` | absolute value, same type as the argument | `INTEGER` lower bound (`-ABS(INT_MIN)`) → `E0034` |
| `MIN(a, b)` / `MAX(a, b)` | binary min / max; mixed int/float promote per §2.2 | — |
| `FLOOR(x)` / `CEIL(x)` | round down / up; `INTEGER` is identity; `FLOAT` → `FLOAT`; `NaN` / `±inf` returned as-is | — |
| `ROUND(x)` | half-away-from-zero rounding; otherwise as above | — |
| `SQRT(x) -> FLOAT` | square root; `-0.0` → `-0.0` | `x < 0` → `ERR(["kind": "DomainError"])` |
| `POW(a, b) -> FLOAT` | power; both args promoted to `FLOAT` | negative exponent of `0`, or a non-integer exponent of a negative base → `ERR(["kind": "DomainError"])` |
| `CLAMP(x, lo, hi)` | clamp into `[lo, hi]`; promotion as `MIN`/`MAX` | `lo > hi` → `ERR(["kind": "DomainError"])` |
| `PI` / `E` | `FLOAT` constants | — |

**`FLOOR` / `CEIL` / `ROUND` are not `INT`.** `INT` truncates toward zero
(§2.2), which differs from all three — the implementation goes `INT` + sign
correction. `FLOAT`s beyond `±2^53` that are already integral, plus `NaN` /
`±inf`, are returned unchanged.

> *Measured on wlwl 0.11.0 (2026-10-01):* `SQRT(2)` → `1.4142135623730951` ·
> `POW(2, 10)` → `1024.0` · `CLAMP(5, 1, 3)` → `3` · `ROUND(2.5)` → `3.0` ·
> `MAX(3, 9)` → `9` · `PI` → `3.141592653589793`.

### §25.3 ⚠ BREAKING: `ASSERT` now rejects all eight falsy values

`ASSERT(cond, msg?)` (stdlib §8, `wlwl:std.test`) judges `cond` with the full
**§2.3 falsy set** — `FALSE` / `NULL` / `0` / `0.0` / `""` / empty array / empty
dict / `NaN`. Truthy → `OK(TRUE)`; falsy → `ERR(E0046)`.

**Before v0.11 only `BOOLEAN(false)` and `NULL` counted as false.** So
`ASSERT(0)`, `ASSERT("")`, `ASSERT([])`, `ASSERT(0.0)` and `ASSERT(DICT())`
used to **pass** and now **fail**. Declared as a breaking change (ADR-0023).

Why this bites: an assertion written as a *smell check* — "non-empty?",
"non-zero?" — now does what it looks like, so the failure moves from runtime
data to test time. Audit existing suites before upgrading; the intent is
almost always right, but the failure is real and immediate.

> *Measured on wlwl 0.11.0 (2026-10-01), one file per case, all via
> `IMPORT("wlwl:std.test", ["ASSERT"])`:* `ASSERT(0)` → `E0046` (exit 1) ·
> `ASSERT("")` → `E0046` · `ASSERT([])` → `E0046` · `ASSERT(DICT())` → `E0046` ·
> `ASSERT(FALSE)` → `E0046` · `ASSERT(NULL)` → `E0046` · `ASSERT(42)` →
> `OK(TRUE)` (exit 0).

`ASSERT_EQ(a, b)` / `ASSERT_NEQ(a, b)` are unchanged (`E0047` / `E0048`).
