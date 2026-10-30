# WLWL v1.0 Reference (on-demand)

> Loaded only when `SKILL.md` points here. This is a lookup catalogue,
> not a tutorial. Section numbers (`§1.5`, `§8.3`, `§10.4`, `§13`, `§17`, etc.)
> refer to `docs/wlwl-spec-reference.md` — the merged copy of the **v1.0**
> specification (authoritative source: the separate
> [`wlwl-spec-v1`](https://github.com/Laffinty/wlwl-spec-v1) repository). That
> file merges the 11 volumes into one, so its second-level headings carry a
> **volume prefix**: `## 语言规范 · 8 错误模型`, `## 标准库规范 · 11 std.encode`,
> `## 一致性册 · 5 规范与实现的差异登记`. The `§N` numbering is unchanged from
> v0.11, so every reference below still resolves — but **cross-volume
> references need the volume name** (language §5 and standard library §5 are
> different sections). Superseded specs v0.6–v0.11 are archived in condensed
> form in `docs/history/20260902-09.md` / `20260915-22.md`; full text via git
> history.
>
> ⚠ **The compiler this bundle targets is `wlwl` v0.12.1** — its public-surface
> expansion (17 new members) and **seven semantic changes (D-14…D-20)** are in
> **§28**. The **language** surface has not moved since v0.6.

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
- [§26. v0.11.2 增量备忘 — 编码 / Unicode 大小写 / 字符串检索 / 集合 / 超越函数](#26-v0112-)
- [§27. v0.11.3 增量备忘 — 哈希与 KDF / Unicode / 输出安全 / 时钟 / 正则 / 随机 / 进程 / 压缩 / 网络 / `RESULT` / 精确数 / JSON 深度 / `ai`·`agent` 降级](#27-v0113-)
- [§28. v0.12.0 增量备忘 — 传输层 / 字节层 / 宿主挂起 / `TAKE_ERR` / `INT` 规则](#28-v0120-)
- [§29. v0.12.1 增量备忘 — `TCP_SERVE` / `HTTP_REQUEST_ASYNC` / `std.http` 中间件 / TLS 扩展 / `STR`·`SPLIT` 订正](#29-v0121-)

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
| **v0.11** | **Two new `wlwl:std.*` namespaces and one breaking `ASSERT` change**, on a renumbered spec (the formatter moved to §A.3): `wlwl:std.str` (`JOIN` `SPLIT_LINES` `CHAR_AT` `COUNT` `QUOTE`) and `wlwl:std.math` (`ABS` `MIN` `MAX` `FLOOR` `CEIL` `ROUND` `SQRT` `POW` `CLAMP` `PI` `E`); ⚠ **`ASSERT` now rejects all eight falsy values** (`ASSERT(0)` used to pass). Full tables in §25 |
| **v0.11.2** | **Standard library only — the language surface is frozen at v0.11** and `../docs/appendix_G.md` is unchanged. **41 new members**: `wlwl:std.encode` (6 encoders), `wlwl:std.text` (`TO_UPPER` `TO_LOWER`), `wlwl:std.str` (`INDEX_OF` `CONTAINS_SUB`), `wlwl:std.collection` (10), `wlwl:std.math` (21 transcendental / integer-math). Plus the `SUB` end-index migration formula fix. Full memo in §26 |
| **v0.11.3** | **Standard library only — 115 new members plus one visible-surface change.** New namespaces `std.time` `std.regex` `std.rand` `std.env` `std.process` `std.compress` `std.net` `std.result` `std.sanitize`; `std.fs` grew from 3 to 18; `std.encode` gained the digest / KDF / CSPRNG family; `std.text` gained the Unicode family; `std.math` grew to 61; `std.json` gained a lexical layer and a path family. ⚠ **`ai` / `agent` left `wlwl:std.*`** — they are official packages now, imported as `"ai:ask"` / `"agent:task"`, and `wlwl:std.ai` no longer resolves (`E0040`). Full memo in §27 |
| **v1.0** | **The spec moved to its own repository** (`wlwl-spec-v1`, 11 volumes); this repo keeps a merged copy at `../docs/wlwl-spec-reference.md`. **The `§N` numbering does not move** — every reference in this bundle still resolves. **Member surface**: the **implementation** surface this bundle targets is **288 members across 25 namespaces** as printed by `wlwl namespace` (`23` × `wlwl:std.*` + the `ai` / `agent` packages) — ⚠ that reading moves as the compiler grows; the spec's appendix A is a **frozen snapshot** and is a *different* list (read the table itself, not a quoted count). What is new for writers: **§8.6 unconsumed `ERR` is reported (`E0103`)** — see §8 below; the **complexity / memory chapters** (§18–§19) and the **conformance volume** (its §5 registers the spec-vs-implementation differences X-01…X-25). The old stdlib **§14** (演进方向) was removed — its "revival conditions" moved into the individual chapters and "rejected" into the overview volume; ⚠ language §14 is a different section (session-type protocols) |
| **v0.12.0** | **Standard library expansion — 17 new public members, the language surface unchanged.** (The member-count delta of this batch is recorded in `CHANGELOG.md`, the historical record; for the **current** implementation surface read `wlwl namespace`.) `std.net` gains the **9-member transport family** (`TCP_LISTEN` `TCP_ACCEPT` `TCP_RECV` `TCP_SEND` `TCP_PEER` `TCP_LOCAL_ADDR` `TCP_SET_TIMEOUT` `TCP_CLOSE` `TCP_CLOSE_LISTENER`) ⇒ **a server can now be written in WLWL itself**; `std.encode` gains the byte layer (`UTF8_ENCODE` `UTF8_DECODE` `BYTE_LEN`); `std.text` gains `TO_INT`; `std.str` gains `CAT`; `std.env` gains `SCRIPT_PATH`; `std.result` gains `TAKE_ERR`; `std.time` gains `SLEEP_ASYNC`. ⚠ **Seven semantic changes** (**D-14…D-20**): transport waits become a **host suspension** (park **only** the calling task — D-18); `std.fs` IO failures become **`ERR` values** instead of terminating diagnostics (D-14); `TAKE_ERR` plus the `E0103` rule on segmented task bodies (D-15); **host suspension + the task-failure report obligation** (D-16 — an orphaned task's failure is reported on stderr, the program continues, exit code 0); **strict `INT` parsing rules** (D-17 — `std.text::TO_INT` stays the lenient one); `SLEEP_ASYNC` inside a `TEST_BUBBLE` behaves exactly like `SLEEP` (D-19); the alias-in-a-write-sequence shape is **excluded** from the amortised-cost promise (D-20, spec-side only — no implementation change). Full memo in **§28** |

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
left, `BOOL` — **13 global entries** — plus **`TAKE_ERR`**, the
`wlwl:std.result` member (import it, then call it as a bare name) ⇒ **14**.
v0.9 adds no new consumers; **v0.12.0 adds exactly one: `TAKE_ERR`**.

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

### §8.6 Unconsumed `ERR` (v1.0)

An `ERR` nobody consumed is now **reported** — the program ends with
**`E0103`** (exit 1). Under v0.11 it was silently dropped, which is how a
failing comparator could return a plausible-looking result with **zero**
diagnostics.

Three ways to dispose of one:

1. **a consumer takes it** — `ERR_PAYLOAD` / `UNWRAP_OR` / `OR_DIE` / `TRY` /
   `UNWRAP` / `WRAP` / `EXPECT_ERR` / `TAKE_ERR`;
2. **explicit discard** — `LET(_, expr)`, the only explicit discard form in the
   language;
3. it becomes the program's value at the top level (that is `E0102`).

⚠ **Observation-only consumers do not dispose of it.** `IS_ERR` / `IS_OK` /
`BOOL` / `TYPE` (and `==`/`!=`, `IF` condition, `&&`/`||` left) let you *look*
at an `ERR` but leave it unconsumed — after looking, take it or discard it
explicitly. The drop points are exactly three: a discarded statement value, a
binding being overwritten (re-bind / `SET`), and the end of a scope.

✅ **The two former gaps are closed** — the conformance volume's **X-06 / X-07**
rows are now **implemented**: a binding holds **every** unconsumed token (two
unconsumed `ERR`s in one container ⇒ **two** `E0103` reports, in order) and a
standard-library callback's argument channel **does** carry a token (an
unconsumed `ERR` inside a callback **is** reported). ⇒ **converged: not one
`ERR` is silently lost.**

## §9. Standard library catalogue (§10)

Full member lists live with the v0.11 namespaces: `std.collection` /
`std.json` / `std.fs` / `std.test` / `std.format` / `std.io` /
`std.encode` are listed in `SKILL.md` "Standard library pointers";
`std.str` has a signed table in §9.3, **`std.math` has all 61 of its members
in §9.5** (grouped by the spec's five families), and the **v0.11.3-new**
`std.sanitize` is in §9.6 with `std.result` in **§9.15**. Concurrent
and OOP primitives are **global**, not under `wlwl:std.*`.
⚠ **`std.ai` / `std.agent` are no longer under `wlwl:std.*`** (v0.11.3 M8,
`ADR-0034`) — they are **official packages** imported as `"ai:ask"` /
`"agent:task"`, and `wlwl:std.ai` / `wlwl:std.agent` no longer resolve.
The authoritative registry of every global builtin (106 entries) is
`../docs/appendix_G.md`.

### §9.5 `wlwl:std.math` — the 61 members (stdlib §7, v0.11 → v0.11.3)

`std.math` is **one namespace with 61 members**, in five families. It is split
here by family because one 61-row table is unreadable; the normative tables are
in `../docs/wlwl-spec-reference.md` §7.

**Layers are mixed**: an R1 facade member may call an R2 kernel underneath
(`TRUNC`), and the four families appended in v0.11.3 (`DECIMAL` / `FRACTION` /
approximate comparison / descriptive statistics) are **R2 only**. Member counts
per family: **A 32** (11 in v0.11 + 21 in v0.11.2) · **B 8** · **C 8** ·
**D 4** · **E 9**.

Conventions (stdlib §7): arithmetic promotion and overflow follow language
spec §2.2; a **domain violation** returns an `ERR` *value*
(`ERR(["kind": "DomainError", …])`); a wrong argument **type** is the native
diagnostic `E0030`. `MIN` / `MAX` / `CLAMP` promote their **return** type too —
any `FLOAT` argument makes the result `FLOAT` (`CLAMP` counts **all three**
arguments).

**A. Basics and float kernels — 32 members (stdlib §7.1)**

| Member | Layer | Notes |
|---|---|---|
| `ABS(x)` | R1 | Absolute value; returns the **same type as the argument**. `INTEGER` lower bound → `E0034`. |
| `MIN(a, b)` / `MAX(a, b)` | R1 | Binary min / max; mixed int/float promote per §2.2. |
| `FLOOR(x)` / `CEIL(x)` | R1 | Round down / up. `INTEGER` is identity; `FLOAT` → `FLOAT`; `NaN` / `±inf` returned as-is. |
| `ROUND(x)` | R1 | Half-**away-from-zero** rounding; otherwise as `FLOOR` / `CEIL`. |
| `TRUNC(x)` | R1 facade + R2 float kernel | Toward zero. **Not `INT`** — see the traps below. Non-numeric → `E0030`. |
| `CLAMP(x, lo, hi)` | R1 | Clamp into `[lo, hi]`; promotion as `MIN` / `MAX`. `lo > hi` → `ERR([kind: "DomainError"])`. |
| `SQRT(x) -> FLOAT` | R2 | Square root; `-0.0` → `-0.0`. `x < 0` → `ERR([kind: "DomainError"])`. |
| `POW(a, b) -> FLOAT` | R2 | Power; **both** arguments are promoted to `FLOAT`. `0` to a negative exponent, or a negative base with a non-integer exponent → `ERR([kind: "DomainError"])`. |
| `PI` / `E` | R1 | `FLOAT` constants. |
| `LN(x)` / `LOG2(x)` / `LOG10(x) -> FLOAT` | R2 | Natural / base-2 / base-10 log. **`x = 0` is not an error — it returns `-inf`** (matches C / Python / Go). `x < 0` → `ERR([kind: "DomainError"])`. |
| `EXP(x) -> FLOAT` | R2 | Natural exponential; total. Overflows to `+inf`. |
| `SIN(x)` / `COS(x)` / `TAN(x) -> FLOAT` | R2 | Trigonometry in **radians**; total. |
| `ASIN(x)` / `ACOS(x) -> FLOAT` | R2 | Domain `[-1, 1]`; `ABS(x) > 1` → `ERR([kind: "DomainError"])`. |
| `ATAN(x) -> FLOAT` | R2 | Total. |
| `ATAN2(y, x) -> FLOAT` | R2 | Quadrant-aware. **Total** — `ATAN2(0, 0)` is `0.0`, not an error. |
| `SINH(x)` / `COSH(x)` / `TANH(x) -> FLOAT` | R2 | Hyperbolic; total; overflow to `±inf`. |
| `SIGN(x) -> INTEGER` | R1 | `-1` / `0` / `1`. **The return type is always `INTEGER`** — it does not follow the argument's promotion. |
| `DIV_CEIL(a, b) -> INTEGER` | R1 | Ceiling integer division. **Integers only** — `DIV_CEIL(7.0, 2)` is `E0030`, not a silent promotion. `b = 0` → `ERR([kind: "DomainError"])`. |
| `GCD(a, b) / LCM(a, b) -> INTEGER` | R1 | Euclidean. **Absolute values**: `GCD(-4, 6)` is `2`. `LCM` with a zero is `0`; `LCM` divides before multiplying so it avoids overflow. `INTEGER` lower bound while taking the absolute value → `E0034`; `LCM`'s intermediate product overflowing `i64` → `E0035`. |
| `IS_SQRT(n) -> INTEGER` | R1 | Integer square root, floored — **it returns the root, not a boolean predicate** (unrelated to the `IS_OK` / `IS_ERR` family). `n < 0` → `ERR([kind: "DomainError"])`. |
| `POW_MOD(base, exp, mod) -> INTEGER` | R2 | Modular power (square-and-multiply, `O(log exp)`); **`mod` may be a cryptographic prime such as `2^61-1`**. The remainder is the **Euclidean** one (it takes the sign of the modulus), not Rust's `%`. `mod = 0` or `exp < 0` → `ERR([kind: "DomainError"])`. |

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

**B. `DECIMAL` family — 8 members (stdlib §7.2, new in v0.11.3)**

The **value shape is a public contract** — a field-name change is breaking:

```json
{"sign": INTEGER, "digits": STRING, "scale": INTEGER}
```

It denotes `sign × digits × 10^-scale`. Canonical form: `digits` has **no
leading zero** (zero is `"0"`) and holds ASCII digits only; `scale >= 0`;
`sign ∈ {-1, 1}` and **zero is always `{1, "0", 0}`** — that removes the two
representations of the same number.

**`scale` is meaningful and is not normalised away**: `DECIMAL("1.00")` renders
back as `"1.00"`. Money code depends on that — it carries the "how many
decimal places" business fact.

| Member | Layer | Notes |
|---|---|---|
| `DECIMAL(literal) -> DICT` | R2 | Build from a literal; accepts `[+-]?digits[.digits]`, including `".5"` and `"-0"`. Illegal literal → `E0030`. |
| `DECIMAL_ADD(a, b) -> DICT` | R2 | Exact addition. The result's `scale` = the **larger** of the two. Illegal `DICT` → `E0030`. |
| `DECIMAL_SUB(a, b) -> DICT` | R2 | Exact subtraction; same `scale` rule as `DECIMAL_ADD`. Illegal `DICT` → `E0030`. |
| `DECIMAL_MUL(a, b) -> DICT` | R2 | Exact multiplication. The result's `scale` = the **sum** of the two. Illegal `DICT` → `E0030`. |
| `DECIMAL_DIV(a, b, scale) -> DICT` | R2 | Exact to `scale` digits; rounding is always `HALF_EVEN`. **`scale` is mandatory**; `b = 0` → `ERR([kind: "DomainError", "op": "DIV"])`. |
| `DECIMAL_CMP(a, b) -> INTEGER` | R2 | `-1` / `0` / `1`, compared **by value** — `1.0` and `1.00` are equal. Never fails. |
| `DECIMAL_TO_STRING(a) -> STRING` | R2 | Rendered **by `scale`** (`"1.50"` / `"-0.25"` / `"3"`). Never fails. |
| `DECIMAL_ROUND(a, scale, mode) -> DICT` | R2 | Round to `scale` digits by `mode`. **`mode` has no default**; unknown mode → `E0030`. |

Four things that are easy to get wrong:

1. **`scale` is mandatory because silent truncation is a lie about precision.**
   The caller has to state how many digits it wants.
2. **`mode` is mandatory because banker's rounding and half-up rounding are a
   *business* decision** — a default would silently pick one for the caller.
   The modes are `HALF_EVEN` (banker's), `HALF_UP` (half-up), `DOWN` (toward
   zero), `UP` (away from zero), `FLOOR` (toward `-inf`), `CEILING` (toward
   `+inf`). **The directional modes differ only on negatives**:
   `FLOOR(-2.1)` = `-3` but `DOWN(-2.1)` = `-2`.
3. **An illegal `DICT` is `E0030`, not a repair.** A caller can hand-write
   `{"sign": 0, "digits": "abc", "scale": 0}` — the spec prices that
   compromise in rather than pretending it cannot happen.
4. **The family has no `MEAN` member** — `std.collection::SUM` plus `/`
   already covers it.

> *Measured on wlwl 0.11.3 (2026-10-20):* `DECIMAL("1.00")` =
> `[sign: 1, digits: 100, scale: 2]`; `DECIMAL("-.5")` =
> `[sign: -1, digits: 5, scale: 1]`; `DECIMAL("-0")` =
> `[sign: 1, digits: 0, scale: 0]` (zero is always positive);
> `DECIMAL_TO_STRING(DECIMAL_ADD(DECIMAL("1.25"), DECIMAL("0.5")))` = `"1.75"`;
> `DECIMAL_MUL` of the same pair = `"0.625"` (scale 2 + 1);
> `DECIMAL_DIV(DECIMAL("1"), DECIMAL("3"), 4)` = `"0.3333"`;
> `DECIMAL_CMP(DECIMAL("1.0"), DECIMAL("1.00"))` = `0`;
> `DECIMAL_CMP(DECIMAL("1.0"), DECIMAL("1.01"))` = `-1`;
> `DECIMAL_ROUND(DECIMAL("2.5"), 0, "HALF_EVEN")` = `"2"`;
> `DECIMAL_ROUND(DECIMAL("-2.1"), 0, "FLOOR")` = `"-3"`;
> `DECIMAL_ROUND(DECIMAL("-2.1"), 0, "DOWN")` = `"-2"`.

**C. `FRACTION` family — 8 members (stdlib §7.3, new in v0.11.3)**

The **value shape is a public contract** — a field-name change is breaking:

```json
{"num": INTEGER, "den": INTEGER}
```

Canonical form: `den > 0`, `gcd(|num|, den) = 1` (**reduced to lowest terms**),
and **zero is always `{0, 1}`**.

| Member | Layer | Notes |
|---|---|---|
| `FRACTION(n, d) -> DICT` | R2 | Construct and reduce; a negative denominator flips **both** numerator and denominator. `d = 0` → `ERR([kind: "DomainError"])`. |
| `FRACTION_ADD(a, b) -> DICT` | R2 | Exact addition; the result is auto-reduced. Overflow → `ERR([kind: "OverflowError"])`; illegal `DICT` → `E0030`. |
| `FRACTION_SUB(a, b) -> DICT` | R2 | Exact subtraction; same rules as `FRACTION_ADD`. |
| `FRACTION_MUL(a, b) -> DICT` | R2 | Exact multiplication; auto-reduced. Overflow → `ERR([kind: "OverflowError"])`; illegal `DICT` → `E0030`. |
| `FRACTION_DIV(a, b) -> DICT` | R2 | Exact division. Zero divisor or overflow → `ERR`. |
| `FRACTION_CMP(a, b) -> INTEGER` | R2 | `-1` / `0` / `1`. **Never fails** — the cross-multiplication goes through `i128`, so an intermediate overflow must not be turned into a wrong answer. |
| `FRACTION_TO_STRING(f) -> STRING` | R2 | `"3/4"` / `"-1/2"` / `"2"` (no denominator when it is `1`). |
| `FRACTION_TO_DECIMAL(f, scale) -> DICT` | R2 | Convert to a **`DECIMAL` dict** (not a `FRACTION` one), rounding always `HALF_EVEN`. **`scale` is mandatory**; outside `0..=30` → `E0030`. |

Four things that are easy to get wrong:

1. **The numerator and denominator are `i64`, not arbitrary precision.** A long
   chain of fraction multiplications hits the `i64` ceiling easily, so the
   family **checks before the overflow happens** and reports
   `ERR([kind: "OverflowError"])`. The language-level `E0035` — which
   **terminates the whole run** — must never escape from a library member.
2. **An unreduced / `den <= 0` / zero-not-`0/1` `DICT` is `E0030`, not a
   repair** — the same priced compromise as the `DECIMAL` family.
3. **`scale` is mandatory** because `1/3`'s decimal expansion **does not
   terminate**; silently returning a truncated value would be a precision lie.
4. **`FRACTION_TO_DECIMAL` returns a `DECIMAL` dict.** Feeding it to
   `FRACTION_TO_STRING` is `E0030` — the shapes are not interchangeable.

> *Measured on wlwl 0.11.3 (2026-10-20):* `FRACTION(3, 6)` =
> `[num: 1, den: 2]`; `FRACTION(1, -2)` = `[num: -1, den: 2]`;
> `FRACTION_TO_STRING(FRACTION_ADD(FRACTION(1, 2), FRACTION(1, 3)))` = `"5/6"`;
> `FRACTION_SUB` of the same pair = `"1/6"`;
> `FRACTION_MUL(FRACTION(1, 2), FRACTION(2, 3))` = `"1/3"`;
> `FRACTION_DIV(FRACTION(1, 2), FRACTION(1, 4))` = `"2"`;
> `FRACTION_CMP(FRACTION(1, 2), FRACTION(2, 4))` = `0`;
> `FRACTION_TO_STRING(FRACTION(2, 1))` = `"2"`.

**D. Approximate comparison — 4 members (stdlib §7.4, new in v0.11.3)**

| Member | Layer | Notes |
|---|---|---|
| `APPROX_EQ(a, b, rel_tol, abs_tol) -> BOOLEAN` | R2 | `\|a - b\| <= max(rel_tol × max(\|a\|, \|b\|), abs_tol)`. Arity 4; a non-numeric argument → `E0030`. |
| `IS_NAN(x) -> BOOLEAN` | R2 | Is `x` `NaN`. A non-numeric argument → `E0030`. |
| `IS_INF(x) -> BOOLEAN` | R2 | Is `x` `±inf`. Same. |
| `SIGNIFICANT_DIGITS(x, n) -> BOOLEAN` | R2 | Is `x` **exactly equal** to its value after keeping `n` significant digits. `n` outside `1..=17` → `E0030`. |

Three things that are easy to get wrong:

1. **Neither tolerance has a default.** Whether `0.1` and `0.1000000001` are
   equal is a **business decision**; guessing a default picks the semantics for
   the caller.
2. **`IS_INF` fills a gap, it is not a convenience.** `==(x, x)` is **TRUE**
   for `±inf` — the self-equality test cannot see it, and this library had
   neither `IS_INF` nor `IS_NAN` before this family. `NaN` at least has the
   classic `x != x` workaround.
3. **`SIGNIFICANT_DIGITS` is a predicate.** "How many significant digits does
   `x` have" would be a *different* member, and it is not one.

All three are **total functions** — a special value must not make them crash:

| Case | `APPROX_EQ` | `SIGNIFICANT_DIGITS` |
|---|---|---|
| `NaN` against any value (including itself) | `FALSE` | `FALSE` |
| `±inf` against the **same-sign** `±inf` | `TRUE` | `FALSE` |
| `±inf` against `-∞` or a finite value | `FALSE` | `FALSE` |
| `0.0` | normal criterion | `TRUE` (zero is "already there" for any `n >= 1`) |

> *Measured on wlwl 0.11.3 (2026-10-20):* `APPROX_EQ(1.0, 1.0, 0.0, 0.0)` =
> `TRUE`; `IS_NAN(0.0)` = `FALSE`; `IS_INF(1.0)` = `FALSE`;
> `SIGNIFICANT_DIGITS(1.5, 2)` = `TRUE`;
> `SIGNIFICANT_DIGITS(1.5, 3)` = `TRUE` (keeping 3 digits gives `1.50`, which
> is exactly `1.5`).

**E. Descriptive statistics — 9 members (stdlib §7.5, new in v0.11.3)**

| Member | Layer | Notes |
|---|---|---|
| `MEDIAN(arr) -> v` | R2 | Median after sorting; **an even length averages the middle two** (via `f64` — integer `/` would truncate). `[]` → `NULL`. |
| `MODE(arr) -> v` | R2 | The most frequent value; **ties go to the first occurrence**; if every value occurs once there is no mode. `[]` → `NULL`. |
| `VARIANCE_S(arr) -> v` | R2 | **Sample** variance (denominator `n-1`). `[]` → `NULL`. |
| `VARIANCE_P(arr) -> v` | R2 | **Population** variance (denominator `n`). `[]` → `NULL`. |
| `STDDEV_S(arr) -> v` | R2 | The square root of `VARIANCE_S`. `[]` → `NULL`. |
| `STDDEV_P(arr) -> v` | R2 | The square root of `VARIANCE_P`. `[]` → `NULL`. |
| `PERCENTILE(arr, p) -> v` | R2 | `p ∈ [0, 100]`, **linear interpolation** (endpoints included). `p` out of range → `NULL`. |
| `CORRELATION(xs, ys) -> v` | R2 | Pearson correlation coefficient; the two arrays must be **the same length**. Zero variance in either direction → `NULL`. |
| `LINEAR_FIT(xs, ys) -> DICT` | R2 | Least-squares line ⇒ `{"slope": FLOAT, "intercept": FLOAT}`. Zero variance in either direction → `NULL`. |

Five things that are easy to get wrong:

1. **Sample and population variance are two separate members**, not one
   member with a flag. `VARIANCE_S` / `STDDEV_S` divide by `n-1`;
   `VARIANCE_P` / `STDDEV_P` divide by `n`.
2. **"Empty" and "undefined" both give `NULL`, not an error** — and the two
   are different inputs:

   | Input | Result | Why |
   |---|---|---|
   | `[]` | `NULL` | No samples, consistent with `std.collection` §5 |
   | sample variance / stddev of `[x]` | `NULL` | Denominator `n-1 = 0` — **mathematically undefined** |
   | population variance / stddev of `[x]` | `0.0` | Denominator `n = 1` — defined |
   | any element is `NaN` / `±inf` | `NULL` | See below |
   | `CORRELATION` / `LINEAR_FIT` with zero variance in either direction | `NULL` | The coefficient / slope is undefined, **not 0** |
   | `PERCENTILE` with `p` out of range | `NULL` | `p` is **data**, not a wrong argument |

3. **`NaN` does not seep into a statistic.** wlwl has no convention for
   propagating float exceptions (`/(0.0, 0.0)` is `E1003`), and a `NaN` that
   quietly enters a statistic makes the result untrustworthy in a way the
   caller cannot see. `NULL` is visible.
4. **A mixed `ARRAY[INTEGER]` / `ARRAY[FLOAT]` does not raise `E0030`** — the
   same rule as `SUM`. Tightening it would split the family from `SUM`, and a
   split means one of the two is wrong.
5. **`PERCENTILE`'s interpolation method is observable**, so it is written
   down: on the same data, the linear / lower / higher / midpoint / Haudrey
   methods give **different** values. This spec takes linear (endpoints
   included) — equivalent to NumPy `method="linear"` and Python
   `statistics.quantiles(method="inclusive")`.

**`CORRELATION` is always `±1` at `n = 2`** — two points are always collinear.
That is a mathematical fact, not a defect. The family has **no `MEAN`
member** — `SUM` plus `/` already covers it.

> *Measured on wlwl 0.11.3 (2026-10-20):* `MEDIAN([1, 2, 3, 4])` = `2.5`
> (the even-length average); `MEDIAN([1, 3, 2])` = `2.0`;
> `MODE([1, 1, 2, 3])` = `1.0`; `MODE([1, 2, 3])` = `NULL` (no mode);
> `VARIANCE_S([1,2,3,4])` = `1.6666666666666667`;
> `VARIANCE_P([1,2,3,4])` = `1.25`;
> `STDDEV_S([1,2,3,4])` = `1.2909944487358056`;
> `STDDEV_P([1,2,3,4])` = `1.118033988749895`;
> `VARIANCE_S([7])` = `NULL`; `VARIANCE_P([7])` = `0.0`; `STDDEV_S([7])` =
> `NULL`; `PERCENTILE([1,2,3,4], 50.0)` = `2.5`;
> `PERCENTILE([1,2,3,4], 101.0)` = `NULL`;
> `CORRELATION([1,2,3], [2,4,6])` = `1.0`;
> `CORRELATION([1,2,3], [1,1,1])` = `NULL`;
> `LINEAR_FIT([1,2,3], [2,4,6])` = `[slope: 2.0, intercept: 0.0]`;
> `MEDIAN([])` = `NULL`; `MODE([])` = `NULL`.
> ⚠ The whole family returns **`FLOAT`** even for integer input
> (`MODE([1, 1, 2, 3])` is `1.0`, not `1`).

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
> table in `../docs/wlwl-spec-reference.md` §5 and
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
| `TO_INT(s)` | **v0.12.0 new.** The **lenient** text→integer parse: trims surrounding whitespace, accepts hexadecimal, truncates a fraction toward zero. A parse failure is an `ERR([kind:"ParseError", ...])` **value**. |

⚠ **`TO_INT` and the global `INT` are deliberately different** (v0.12.0 / D-17):
`TO_INT` is for **input parsing** (lenient), `INT` is the **strict internal
conversion** — `INT` does **not** trim whitespace, accepts **decimal only**,
**rejects** a string with a fractional part, and reports a parse failure as an
`ERR` value too. Pick by intent, not by convenience.

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
| `CAT(parts...)` | **v0.12.0 new.** **Variadic** concatenation — the language's `+` is **binary**, so joining N parts otherwise needs N−1 nested calls. Each part is rendered with `STR` first; `CAT()` with no parts gives `""`. |

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

**Sixteen members**, in two layers. The **13 text-layer** members take and return
`STRING` and work on its **UTF-8 bytes**; the **3 byte-layer** members added in
v0.12.0 (`UTF8_ENCODE` / `UTF8_DECODE` / `BYTE_LEN`) cross the boundary and
travel as **`ARRAY[INTEGER]`** (`0..=255`) — `UTF8_ENCODE(s)` → an array,
`UTF8_DECODE(bytes)` → a `STRING`, `BYTE_LEN(s)` → an integer.
> *Measured on wlwl 0.12.0 (2026-10-26):* `UTF8_ENCODE("héllo")` =
> `[104, 195, 169, 108, 108, 111]`; `BYTE_LEN("héllo")` = **6** while
> `LEN("héllo")` = **5**.
`STRING` is not a byte buffer: a round trip preserves text, not arbitrary
binary — to carry binary, encode on this side and decode on the other.
(The table below lists **11** of the 16; the other five — `PBKDF2_ITER`
`ARGON2ID` `RANDOM_BYTES` `RANDOM_HEX` `TIMING_SAFE_EQ` — are in the two
subsections further down.)

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
| `UTF8_ENCODE(s)` | **v0.12.0 new (byte layer).** `STRING` → `ARRAY[INTEGER]`, each `0..=255`. Never fails. |
| `UTF8_DECODE(bytes)` | **v0.12.0 new.** `ARRAY[INTEGER]` → `STRING`. ⚠ **No lossy replacement** — invalid bytes give `ERR([kind:"DecodeError", ...])`, never `U+FFFD`. |
| `BYTE_LEN(s)` | **v0.12.0 new.** The **UTF-8 byte count**. ⚠ `LEN(s)` gives **code points** — they differ for any non-ASCII text, and a `Content-Length` header needs the **byte** count. |

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

⚠ **This shape stays as-is — `std.result` cannot shorten it** (§9.15). The
combinators' `ERR` branches never run (§8.2), so a decode failure has to be
inspected in the caller's own scope, exactly as written above.

> *Measured on wlwl 0.11.2 (2026-10-02):* `BASE64_ENCODE("foobar")` =
> `Zm9vYmFy`; `URL_ENCODE("a b&c=d")` = `a%20b%26c%3Dd`; `URL_DECODE("a+b")` =
> `a+b`; `BASE64_DECODE("!!!")` → `ERR([kind: DecodeError, ...])`. Full vector
> table in `../docs/wlwl-spec-reference.md` §11.4; the base64
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
- Entropy-source failure surfaces as the **native IO-error diagnostic** (the code
  is in the spec's §11.2 error table), distinct from `E0030` (you passed a bad
  argument) and from a `DecodeError` value (bad data).
### §9.7 `wlwl:std.json` — the deep face: lexical layer, paths, schema (stdlib §3, v0.11.3)

Eleven members. The two you already knew — `PARSE`, `STRINGIFY` — are **frozen**:
their behaviour may not change. The nine added here are all additive.

| group | members | what it is for |
|---|---|---|
| lexical | `JSON_TOKENIZE` `JSON_LOCATE` `JSON_VALIDATE` | **positions**. Every error from here on says *where* |
| paths | `JSON_GET` `JSON_SET` `JSON_DELETE` `JSON_MERGE` `JSON_CHUNK_ITEMS` | addressing and immutable edits |
| schema | `JSON_CHECK` | a **closed subset** of JSON Schema |

⚠ **Two failure conventions live in this one namespace, and that is the spec, not an
oversight.** `PARSE` / `STRINGIFY` fail with **`E0070` / `E0071` diagnostics** (they
abort the run — v0.3-era behaviour, frozen). The nine new members return an
**`ERR` value** you consume with `IS_ERR` / `ERR_PAYLOAD` / `UNWRAP_OR`, shaped
`[kind: "JsonError", op, …]` per stdlib §11.

⚠ **`PRINT(<ERR>)` prints nothing** and the `ERR` escapes as `E0102`. Bind it first,
then `ERR_PAYLOAD` it — that is the whole idiom:

```
LET(e, JSON_VALIDATE(body));
IF(IS_ERR(e), PRINT(ERR_PAYLOAD(e)), …)
```

**What each position field means, and what it does not mean:**

| member | `path` | `line` / `column` |
|---|---|---|
| `JSON_VALIDATE` / `JSON_TOKENIZE` | ✗ absent — they do not look at structure | ✓ the offending byte |
| `JSON_SET` / `JSON_DELETE` / `JSON_MERGE` | ✓ the conflicting path | ✗ absent — **a path is not a location** |
| `JSON_CHECK` | ✓ | ✓ |

The omitted fields are omitted on purpose. A field that is present but meaningless
(`line: 1` for "the document is not an array") is worse than an absent one: you
cannot tell a real position from a filler.

⚠ **Offsets are codepoints, not bytes.** `start` / `end` / `offset` / `token_start`
are **0-based codepoint** indices; `line` / `column` are **1-based**, and `column`
counts codepoints too. The only string-slicing primitive in the language,
`SUB(s, start, len?)`, is codepoint-indexed — byte offsets would be numbers you
cannot use. Same reason `JSON_LOCATE` reports the start of the token **containing**
the offset, not the previous one.

⚠ **`JSON_CHECK` takes the document as a `STRING`, not a parsed value.** stdlib §2.3
sketched the signature with a value while also asking for `line` / `column`; those
cannot both hold, because the same value can come from any text. Positions are the
point of this batch, so the text is what it takes. An unknown schema key is
**`E0030`, never silently ignored** — a half-enforced schema is worse than none,
because the caller believes the constraint is live. The message lists **every** legal
key, so you do not have to go looking it up.

⚠ **`type: "integer"` means `INTEGER`, not "no fractional part".** `PARSE("2.0")`
gives you a `FLOAT`, and `JSON_CHECK` will reject it under `integer`. wlwl *remembers*
whether a number was an integer; flattening that here would throw away information
you can already get from `TYPE()`, and make the verdict unpredictable from the value
alone. Deliberate divergence from JSON Schema — the same kind as `STDDEV_S([7])`
returning `NULL`.

⚠ **`JSON_MERGE` recurses on objects and REPLACES arrays.** `{"a":{"x":1}}` merged
with `{"a":{"y":2}}` keeps both `x` and `y`; `{"a":[1,2]}` merged with `{"a":[9]}`
gives `[9]`, never `[1,2,9]`. Vague merge semantics are a configuration-incident
generator, so both directions are pinned by contract cases.

⚠ **`JSON_GET` / `JSON_SET` / `JSON_DELETE` copy the whole document on every call.**
`Value` is a deep-copying owning tree, so passing a document in *is* a deep clone —
that is the price of the language's immutable value semantics, not a cost this
module invented. `JSON_SET` creates missing intermediate containers (typed by the
**next** path segment), so building a config tree one `SET` at a time works;
passing **through** a scalar is a real conflict and returns an `ERR`.

⚠ **`JSON_TOKENIZE` is ~10× slower than `JSON_VALIDATE` on the same lexer.** Both
scan identically; tokenizing also *materializes* a `DICT` and a `STRING` per token —
about six heap allocations each. If all you want is "is this valid?", call
`JSON_VALIDATE`, not `JSON_TOKENIZE`.

⚠ **`PARSE` does not preserve document order — keys come back sorted** (stdlib §3
says otherwise; registered as **D13-017**, root cause: `serde_json` without
`preserve_order`, so its `Map` is a `BTreeMap`). Do not rely on key order coming out
of `PARSE`; `STRINGIFY` sorting keys *is* specified.

**Anti-pattern: treating a `PARSE` failure as "the data is bad".** `E0070` carries no
position, so a failing response tells you nothing about *which* byte. Use
`JSON_VALIDATE` when you need to point a human at the problem — that is the member
that exists for it.


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

### §9.14 `wlwl:std.net` — URLs and plaintext HTTP (stdlib §21, v0.11.3)

```wlwl
IMPORT("wlwl:std.net", ["URL_PARSE", "URL_JOIN", "HTTP_GET", "HTTP_POST", "HTTP_REQUEST"]);

// Pure functions — RFC 3986 generic syntax. These never touch the network.
URL_PARSE("http://user:pw@example.com:8080/p?q=1#f");
// -> ["scheme": "http", "userinfo": "user:pw", "host": "example.com",
//     "port": 8080, "path": "/p", "query": "q=1", "fragment": "f"]
URL_JOIN("http://a/b/c/d;p?q", "../g");   // "http://a/b/g"

// Plaintext HTTP. Returns ["status", "headers", "body"].
LET(r, HTTP_GET("http://example.com/api"));
AT_K(r, "status", NULL);                  // 200
HTTP_POST("http://example.com/api", body, ["Content-Type": "application/json"]);
HTTP_REQUEST("PUT", "http://example.com/api", "payload");
```

- ⚠ **`URL_PARSE` always returns all seven keys.** A component that never
  appeared is `NULL`; one that appeared but is empty is `""`. That distinction is
  **normative** (RFC 3986 §5.3): the separator was absent vs present-and-immediately
  followed by the next separator. Collapsing them makes `query` and `path`
  indistinguishable and `remove_dot_segments` then mangles a `/./` inside a query.
- ⚠ **`userinfo` is a key, not discarded.** RFC 3986 §7.6:
  `ftp://cnn.example.com&story=breaking_news@10.0.0.1/top_story.htm` reads as
  host `cnn.example.com` to a human while the real host is `10.0.0.1`. A parser
  that swallows the segment hides a known semantic attack. `host` keeps IPv6
  literals bracketed (`"[::1]"`) so it round-trips; `port` is an `INTEGER`.
- **What counts as an invalid URL** is pinned: the input must match
  `URI-reference` in RFC 3986 Appendix A. Control characters/spaces, a `:` whose
  prefix is not a legal `scheme`, a bare `%`, a `[`/`]` outside a well-formed
  IP-literal, a non-`DIGIT` or `> 65535` port → `E0030`. **Empty host and missing
  scheme are accepted** — judging those invalid would break `URL_JOIN` on §5.4's
  `//g` and `""` inputs. IPv6's full grammar is deliberately **not** validated
  (RFC §3.2.2 defers it to RFC 3513 and calls it hard to specify).
- ⚠ **`URL_JOIN` is strict-mode.** RFC §5.4.2's `http:g` has two answers;
  we take the one RFC lists first (`http:g`). **WHATWG URL gives the other**
  (`http://a/b/c/g`) — do not carry expectations across specs. No percent-decoding,
  no host lower-casing, no scheme normalisation: those are §6.2's
  syntax/scheme normalisation, a **different semantic**.
- ⚠ **`HTTP_*` is a blocking primitive.** Nothing else is scheduled while it runs
  and the whole `SCOPE` freezes. Same cost as `SLEEP` / `PROCESS_RUN`. **Never
  `SPAWN` several HTTP calls in one `SCOPE` expecting concurrency** — they
  serialise, and that is a *wrong* program, not a slow one.
- ⚠ **Timeout defaults to 30 000 ms**, and the default is not decoration: the
  block is bounded by the **wall clock** (unlike `SLEEP`, there is no virtual
  clock), so omitting it hangs the whole program. Pass a smaller one for
  fan-out.
- ⚠ **Redirects are not followed.** 3xx comes back with `status` and `Location`
  intact; following can carry the request to a host you did not intend (SSRF).
- ⚠ **Unsupported framing is `ERR`, never a best-effort parse.** Refused:
  `Transfer-Encoding` (**including `chunked`** — a smuggling vector, so it is
  *not* decoded), duplicate `Content-Length` (even with equal values), obs-fold
  (a header line starting with whitespace), a bare CR/LF in a header value, and
  a response with neither `Content-Length` nor `Connection: close` (we do not
  guess the body length). The caps are **64 KiB** of header block and **8 MiB** of
  body — past them you get an `ERR`, **not** an OOM, because a server can trigger
  that.
- ⚠ **Failures are `ERR([kind:"HttpError", op, reason])` values, not native
  diagnostics.** An unreachable host is an expected business branch (retry,
  degrade, log); a diagnostic would **terminate the run** and leave the caller
  unable to retry. Arity / type / non-token method / header-injection mistakes are
  still `E0022` / `E0030` — those *are* programmer errors.
- Zero new third-party dependencies: plaintext HTTP/1.1 over
  `std::net::TcpStream` plus the system resolver. TLS, WebSocket/SSE/gRPC,
  connection pooling, retry policy, proxying and cookie jars are all **not**
  provided.
- **Correctness evidence.** `URL_JOIN`'s evidence is the **42 reference-resolution
  vectors of RFC 3986 §5.4, frozen verbatim** — extracted by a script from the
  RFC text, not typed by hand and not copied from this implementation. `URL_PARSE`'s
  evidence is the Appendix B example plus Appendix A's ABNF.
- ⚠ **The blocking shape is conditionally frozen.** `ADR-0028` D1 holds *because*
  `ADR-0017` Step 3 (true state-machine resumption) has not landed. If it does,
  the guard `adr_conditional_guard.rs` turns red and **D1 must be re-decided** —
  the blocking cost (and with it the 30 s timeout floor) would no longer be
  necessary.

#### §9.14.1 The transport family — `TCP_*` (v0.12.0)

```wlwl
IMPORT("wlwl:std.net", ["TCP_LISTEN", "TCP_ACCEPT", "TCP_RECV", "TCP_SEND",
                        "TCP_PEER", "TCP_LOCAL_ADDR", "TCP_SET_TIMEOUT",
                        "TCP_CLOSE", "TCP_CLOSE_LISTENER"]);

LET(l, TCP_LISTEN("127.0.0.1", 0));        // port 0 => host picks a free port
AT_K(TCP_LOCAL_ADDR(l), "port", 0);         // read the real port back
LET(c, TCP_ACCEPT(l, 5000));                // => connection handle, or NULL on timeout
LET(chunk, TCP_RECV(c, 65536, 3000));       // => {"bytes": [...], "text": "...", "eof": FALSE}
TCP_SEND(c, "hello");                       // => number of bytes written
TCP_PEER(c);                                // => {"host": "...", "port": N}
TCP_SET_TIMEOUT(c, 1000);                   // the connection's read timeout (0 = infinite)
TCP_CLOSE(c); TCP_CLOSE_LISTENER(l);        // idempotent; never a diagnostic
```

| Member | Contract |
|---|---|
| `TCP_LISTEN(host, port)` | Binds and listens; returns a **listener handle**. `port = 0` ⇒ the host picks a free port (read it back with `TCP_LOCAL_ADDR`). |
| `TCP_ACCEPT(listener, timeout_ms)` | Accepts one connection → a connection handle. ⚠ **Timeout ⇒ `NULL`** — a **normal value**, not `ERR`. `timeout_ms = 0` ⇒ **no deadline**. |
| `TCP_RECV(conn, max_bytes, timeout_ms)` | Reads **at most** `max_bytes` bytes → `{"bytes": ARRAY[INTEGER], "text": STRING, "eof": BOOLEAN}`. ⚠ **Timeout ⇒ empty bytes with `eof = FALSE`** — **never** EOF (§21.6.2). `max_bytes = 0` reads **0** bytes and consumes nothing. |
| `TCP_SEND(conn, payload)` | Writes `payload` (`STRING` as UTF-8, `ARRAY[INTEGER]` as raw bytes `0..=255`) → the number of bytes written. |
| `TCP_PEER(conn)` | `{"host": STRING, "port": INTEGER}`. |
| `TCP_LOCAL_ADDR(listener)` | `{"host": STRING, "port": INTEGER}`. |
| `TCP_SET_TIMEOUT(conn, ms)` | The connection's **read timeout** (`0` = infinite). The call-site budget and this one race — **whichever comes first wins** (§21.6.4). |
| `TCP_CLOSE(conn)` / `TCP_CLOSE_LISTENER(listener)` | Idempotent: closing twice, or closing a handle that never existed, returns `NULL` and produces **no diagnostic**. |

⚠ **A transport wait is a HOST SUSPENSION, not a blocking call** (v0.12.0 /
D-18): it parks **only the calling task**; other tasks keep running. This is
the one thing that differs from `HTTP_*` / `SLEEP` / `PROCESS_RUN`, which
**freeze the whole `SCOPE`**. ⇒ several connections in one `SCOPE` are fine
with `TCP_*` and **wrong** with `HTTP_*`.

⚠ **The wait obeys the segment model** (§17.0): host suspension is a
*non-segmenting* suspend point ⇒ the resume **replays the segment from its
head** ⇒ the statements before the wait **run again**. The wait must be the
**first statement of its segment**, and there must be **one wait per segment**.
⇒ the eight rules in `SKILL.md` ("Network programming") are not stylistic.

⚠ **Runtime failures are `ERR` values**, not diagnostics: an unknown handle, a
handle of the wrong kind, or a failed write gives
`ERR([kind:"SocketError", op, reason])`. Arity / type mistakes stay
`E0022` / `E0030`.

⚠ **The transport layer carries no protocol.** No HTTP parsing, no
`Content-Length` handling, no status codes — those live in WLWL code. There is
a mechanical guard (`no_http_semantics_in_the_transport_layer`) that keeps the
native layer free of them.

⚠ **There is no `TCP_CONNECT`** — the transport family can listen and serve,
but it cannot open an outbound connection. Use `HTTP_*` for that (and remember
those **block** the whole `SCOPE`).

### §9.15 `wlwl:std.result` — `OK` / `ERR` combinators (stdlib §22, v0.11.3)

```wlwl
IMPORT("wlwl:std.result", ["RESULT_MAP", "AND_THEN", "FLATTEN", "NULL_OR"]);

// Composing steps that may fail.
RESULT_MAP(OK(5), FUN((x), +(x, 1)));          // OK(6)
AND_THEN(OK(3), FUN((x), OK(+(x, 1))));        // OK(4)
FLATTEN(OK(OK(7)));                            // OK(7)

// Anything that isn't NULL passes through untouched.
NULL_OR(NULL, 5);                              // 5
NULL_OR("", 5);                                // ""      ← not NULL
NULL_OR([], 5);                                // []      ← not NULL

// One-step "check and consume" (v0.12.0).
LET(p, TAKE_ERR(some_err));   // the payload — and the unconsumed state is CLEARED
TAKE_ERR(42);                  // NULL — not an ERR, nothing to take
```

- ⚠ **`TAKE_ERR(x)` is a one-step "check and consume"** (v0.12.0 / D-15): if `x`
  is an `ERR`, it returns the **payload** *and clears the unconsumed state*; if
  `x` is not an `ERR`, it returns `NULL`. ⚠ It is a **taking consumer** (§8.3) —
  merely *observing* with `IS_ERR` does **not** clear the unconsumed state, and
  an unconsumed `ERR` that is dropped is still reported as **`E0103`** (§8.6.2).
  ⚠ The `E0103` rule applies to **segmented task bodies** exactly as it does to
  top-level code and ordinary closures — segmentation changes the **resume
  granularity**, not the obligation.

- ⚠ **`RESULT_MAP` is not `MAP`.** `std.collection` has exported `MAP` since
  v0.10; importing both in one scope is a hard **`E0021`** (it refuses rather
  than shadowing). `AND_THEN` / `FLATTEN` / `NULL_OR` collide with nothing.
- ⚠ **These three cannot see an `ERR`.** Per §8.2 a call whose **argument** is an
  `ERR` never evaluates the callee's body — the `ERR` *becomes the result*. The
  registry check is keyed on the **callee's name**, and §8.3's registry lists
  **`TAKE_ERR` only** from this namespace — `RESULT_MAP` / `AND_THEN` /
  `FLATTEN` / `NULL_OR` / `RESULT_ALL` / `RESULT_ANY` are **not** in it. So
  `RESULT_MAP(ERR(e), f)` returns `ERR(e)` **because the
  body never ran**, not because the member inspected it. Don't read a passing
  counter as evidence that the `ERR` branch ran.
- ⚠ **That same wall applies to collection elements.** Handing an `ERR` element to
  any closure is itself such a call: `ANY(rs, FUN((x), IS_ERR(x)))` and
  `MAP(rs, FUN((x), x))` both die with `E0102`. This is why `RESULT_ALL` /
  `RESULT_ANY` **take no predicate argument** — they inspect elements themselves.
- ⚠ **`OR_ELSE` / `ERR_MAP` / `ERR_OR_ELSE` do not exist.** Their `ERR` branches
  have to *do* something (take the payload, call a fallback, replace the payload)
  and the body never runs, so no implementation layer can. Branch in your own
  scope instead — that is exactly what §8.2's corollary prescribes:

  ```wlwl
  // OR_ELSE(r, fallback):  ERR(e) -> fallback(e)
  LET(r, risky());
  LET(p, ERR_PAYLOAD(r));                  // consume HERE, in the owning scope
  LET(y, IF(IS_ERR(r), fallback(p), r));   // IF short-circuits: only one branch runs
  ```

- ⚠ **`AND_THEN` does not re-wrap.** It returns `f(x)` **verbatim**, so a bare
  value from `f` breaks the chain: `AND_THEN(OK(3), FUN((x), +(x, 1)))` is `4`,
  not `OK(4)`. Same contract as Rust's `and_then` — `f` must return a `RESULT`.
- ⚠ **`RESULT_MAP` nests.** `RESULT_MAP(OK(1), FUN((x), ERR(e)))` is
  `OK(ERR(e))`. That is what `FLATTEN` is for.
- ⚠ **`NULL_OR` is not `UNWRAP_OR`, and there is no `IS_NULL`.** `NULL` is not
  `ERR`: `""` / `[]` / `0` / `FALSE` all pass through, and an `ERR` argument
  **propagates** rather than taking the default (swallowing an error into a
  default would be wrong). The only discriminator is `==(TYPE(v), "NULL")` — a
  truthiness test like `IF(v, …)` would swallow `""` / `[]` / `0` too.
- `UNWRAP` stays a **global builtin** and is not re-exported here; it fails with
  `E0100` PANIC, which is a different code from `E0102` ("`ERR` reached the top").

### §9.13 `wlwl:std.compress` — Zstandard and the DEFLATE family (stdlib §20, v0.11.3)

```wlwl
IMPORT("wlwl:std.compress", ["ZSTD_COMPRESS", "ZSTD_DECOMPRESS", "GZIP_COMPRESS", "GZIP_DECOMPRESS"]);

LET(bytes, [119, 108, 119, 108]);                  // ARRAY[INTEGER], not STRING
LET(packed, ZSTD_COMPRESS(bytes));
ZSTD_DECOMPRESS(packed);                            // -> the original bytes
```

- The payload is an **`ARRAY[INTEGER]`** with each element `0..=255` — the same shape
  as `READ_BYTES`, decided in addendum-06. Compressed input is by definition not
  UTF-8, and a `STRING` would lossily substitute `U+FFFD`. An out-of-range element
  is `E0030`, **never truncated** — a silent truncation would give one payload two
  readings.
- ⚠ **Compression is not encryption.** Zero confidentiality; anyone with the input
  decompresses it. Treat "gzip it so nobody sees it" as a security defect, not a
  usage.
- ⚠ **`level` out of range is `E0030`, never clamped.** Clamping makes "I passed 9"
  and "I passed 999" produce the same output — that is lying to the caller. Omit it
  to take the library default (and note: *the default is not the recommended value*).
- ⚠ **The 64 MiB decompression cap is a spec clause**, not a tunable in the
  implementation: past it you get an `ERR`, not an OOM. It is also a **known
  limitation** — this batch does not stream, so a larger single payload has no
  decompression path yet.
- ⚠ `DEFLATE_RAW_*` carries **no checksum**, so corruption is *undetectable* there:
  a truncated stream can decode to a correct prefix without complaining. Use zlib
  (Adler-32) or gzip (CRC32) when integrity matters.
- ⚠ **Do not assert compression ratios.** How small the output is depends on the
  compressor implementation — the same 104-byte input came out at **66 bytes** from
  Python's `zlib` and **115 bytes** (larger than the input!) from `miniz_oxide`.
  What is contractual: round-tripping, determinism, and decoding *another*
  implementation's output (the contract table feeds us gzip/zlib streams produced
  by Python's standard library, which is the check that proves the decoders really
  follow the RFCs rather than only understanding their own output).

### §9.12 `wlwl:std.fs` / `std.env` / `std.process` (stdlib §2 / §18 / §19, v0.11.3)

```wlwl
IMPORT("wlwl:std.fs", ["PATH_JOIN", "PATH_NORMALIZE", "LIST_DIR", "WALK_DIR", "MKDIRS"]);

PATH_JOIN("a", "b", "c");          // "a/b/c"  — always `/`, on every platform
PATH_NORMALIZE("a/./b/../c");      // "a/c"    — purely lexical
MKDIRS("out/data");                // mkdir -p, idempotent
LIST_DIR("out");                   // sorted; symlinks listed, not followed
WALK_DIR(".", "**/*.md");          // `**` crosses directories, `*` does not
```

- ⚠ **The spec separator is `/` on every platform.** `PATH_JOIN` never emits `\`.
  Windows *additionally accepts* `\` on input (that is the pre-existing behaviour of
  the original three members; removing it would be breaking), and POSIX does **not**
  translate it — there `\` is a legal filename character.
  **Why this is decided rather than inherited:** with the platform's own separator,
  `PATH_JOIN("a", "b")` returns `a\b` on Windows and `a/b` on Linux, so the same
  program produces different output on different machines and the contract tables
  **cannot be frozen byte-for-byte** — and byte-frozen contracts are what this whole
  repository stands on.
- Boundary cases that silently break a chain of path operations if you get them
  wrong: `PATH_DIR("a.txt")` is `""` (not `"."`) so
  `PATH_JOIN(PATH_DIR(x), PATH_BASE(x))` round-trips; `PATH_EXT("a.tar.gz")` is
  `"gz"`; `PATH_EXT(".gitignore")` is `""` (a dotfile has **no** extension, otherwise
  base + ext would not rebuild the name); `PATH_EXT` has **no leading dot**;
  `PATH_NORMALIZE` is **purely lexical** — it does not know whether `a` in `a/../b`
  is a symlink, because a normaliser that called `canonicalize` would have IO side
  effects and would change behaviour when the path does not exist.
- `LIST_DIR` / `WALK_DIR` results are **sorted** — the OS directory order varies
  between runs, and determinism is this language's first selling point.
  `WALK_DIR` **does not descend into symlinked directories** unless you pass
  `follow = TRUE`; a symlink cycle would otherwise recurse forever.
- `MKDIR` reports `ERR` when the path already exists — silently succeeding would
  hide a race (another process just created it). `MKDIRS` is `mkdir -p` and
  idempotent.
- ⚠ **Binary IO is `ARRAY[INTEGER]`** (`READ_BYTES` / `WRITE_BYTES`), the same shape
  as `std.encode::RANDOM_BYTES`, because `STRING` is UTF-8 and would lossily
  substitute `U+FFFD`. It is meant for **small** payloads (config, icons, key
  material fragments). **Large binary belongs on disk** — compression and copying
  happen as byte streams inside R2; round-tripping a big payload through language
  values walks straight back into the interpreter's superlinear array construction.
- ⚠ **Relative paths resolve against the process CWD**, *not* the program's import
  base dir (that one only affects `IMPORT`).
- `std.env`: `ENV_KEYS()` is **sorted** (the host's iteration order varies) and
  `ENV_GET` of an unset variable returns `NULL`, not `""` — "unset" and "set to
  empty" are different states. `ENV_SET` is process-local and never touches disk.
  `ARGS()` includes `argv[0]`, like Go's `os.Args` and Python's `sys.argv`.
  `SCRIPT_PATH()` (**v0.12.0 new**) returns the **entry script's path** as a
  `STRING`, or `NULL` when there is none. Use it when a program must locate
  files **relative to itself** rather than to the current working directory.
- **`std.process`** — both members are **R2**; a failure is an **`ERR` value**,
  not a diagnostic:

| Member | Notes |
|---|---|
| `PROCESS_RUN(argv, opts?) -> DICT` | Returns `[code: INTEGER, stdout: STRING, stderr: STRING]`. ⚠ **`argv` must be an `ARRAY[STRING]`** — concatenating into one string and letting the OS re-parse **is command injection**, so the string form is rejected and there is deliberately no `SYSTEM` / `sh -c` member. A **nonzero exit code is data, not an error**. Unknown `opts` keys (e.g. `shell`) are **rejected, not ignored**: ignoring and forbidding are different things in a security context. ⚠ **It blocks** — in this single-threaded cooperative runtime nothing else is scheduled while a child runs (same cost as `std.time::SLEEP`). |
| `PROCESS_ID() -> INTEGER` | This process's id. It takes **no arguments** — a non-zero arity is `E0022`. |

  A start-up failure or a timeout is `ERR([kind: "ProcessError", op, reason])`.
  There is deliberately **no `PROCESS_EXIT_CODE` member**: it would need "the
  exit code of the last `PROCESS_RUN`", which is hidden mutable state across
  calls, and the `code` field is already in the returned value.

**`std.fs` — the tree-mutation family** (stdlib §2.2; all four are **R2** and
all four fail with an **`ERR` value**, not a native diagnostic):

| Member | Notes |
|---|---|
| `REMOVE(p, recursive?) -> NULL` | Delete a file or a directory. **A non-empty directory without `recursive = TRUE` fails** — it is not silently emptied. |
| `COPY(src, dst) -> NULL` | Copy a **file**. **Directories are not copied** — there is no recursive-copy member. |
| `MOVE(src, dst) -> NULL` | Move / rename. A **cross-device** move automatically falls back to "copy + delete". ⚠ **Symbolic links are handled differently depending on the device**: **same device** (the `rename` path) ⇒ the **link is preserved**; **cross-device** (the copy + delete fallback) ⇒ the link is **expanded by content** (a directory link becomes a real directory). This is **specified behaviour**, not an accident — see stdlib §2.3. If your code depends on "the moved thing is still a link", **make sure it stays on one device**. |
| `FILE_SIZE(p) -> INTEGER` | Size in **bytes**. |

The failure payload for the whole path / directory / byte family is
`ERR([kind: "FsError", op: <member name>, reason: <path and OS reason>])`.

⚠ **`WRITE_BYTES` has a per-call cap of 64 MiB** (`MAX_WRITE_BYTES`) — a larger
array is an `ERR`, **not** a truncation and **not** an OOM. ⚠ The cap is checked
**before** the write, so an over-cap call **leaves the target file untouched**.
⚠ **Written ≠ readable**: `READ_BYTES` reads at most **1 MiB** per call, so a
file larger than 1 MiB can be written but **not** read back in one go — leave
big payloads in files and move them with the byte-stream members
(`std.compress`), not through a language-value array.
"Directory does not exist" and "already exists" are **expected runtime
results**, so the caller gets a branchable `ERR` instead of a diagnostic that
terminates the run. (⚠ **as of v0.12.0** the three **frozen** v0.3 members —
`READ_FILE` / `WRITE_FILE` / `EXISTS` — follow this rule too: their IO failures
are `ERR([kind:"FsError", op, reason])` **values**, not native diagnostics (the
fs IO codes `E0061` / `E0062` are **retired** since v1.0.2). Those codes are
**not deleted** — `std.fs` simply no longer
produces them, so a program that branched on them has to branch on the `ERR`
value instead. `EXISTS` still has no failure path at all.)

⚠ **`MOVE`'s cross-device fallback is not a plain copy.** It expands symlinks
under the same discipline as `WALK_DIR(follow = TRUE)` and keeps a `visited`
set to break cycles; **hitting a cycle returns an `ERR` rather than skipping
it**. Skipping would let the follow-up delete wipe the source side's link —
losing a link is worse than failing. ⚠ **Whether a symlink survives the move
depends on the devices** (stdlib §2.2, D13-051): a **same-device** move is a
plain rename and keeps the link **as a link**; a **cross-device** move falls
back to copy + delete and therefore expands the link **by content** (a
directory link becomes a real directory, a file link becomes the target's
bytes). A caller that depends on "the thing I moved is still a symlink" must
confirm it stays on one device.

> *Measured on wlwl 0.11.3 (2026-10-20):* `FILE_SIZE` of a 5-byte file = `5`;
> `COPY` then `MOVE` then `REMOVE` behave as above; the failure payload of
> `REMOVE` / `COPY` / `MOVE` / `FILE_SIZE` on a missing path is
> `[kind: FsError, op: REMOVE, reason: …: not found: … (os error 2)]` — the
> `op` field is the member's own name.

### §9.11 `wlwl:std.time` — clocks, sleep and the fake-clock bubble (stdlib §15, v0.11.3)

```wlwl
IMPORT("wlwl:std.time", ["NOW", "MONOTONIC", "SLEEP", "TEST_BUBBLE", "TIMEOUT"]);

// Real clock.
LET(t0, MONOTONIC());
SLEEP(50);
PRINT(-(MONOTONIC(), t0));          // >= 50, measured in real milliseconds

// Virtual clock: the hour-long wait costs no wall time, and is reproducible.
PRINT(TEST_BUBBLE(FUN(() , (SLEEP(3600000); MONOTONIC()))));   // 3600000
PRINT(TEST_BUBBLE(FUN(() , IS_ERR(TIMEOUT(100, FUN(() , SLEEP(200)))))));  // TRUE
```

- `NOW() -> INTEGER` is Unix **milliseconds**; `MONOTONIC() -> INTEGER` is
  process-relative and monotonic. ⚠ **`MONOTONIC`'s absolute value is meaningless
  — only differences are valid**, and it is the one to measure with. `NOW` can jump
  backwards (NTP, manual clock changes) and a program that reads it is **no longer
  reproducible**; the runtime cannot detect that, so it is only declared on the
  member row. **Test code especially should use `MONOTONIC`.**
- ⚠ **`SLEEP` blocks the calling thread** and **no other task is scheduled
  meanwhile** — the same inside a task, and the same while a blocking HTTP call is
  in flight. That is a documented deterministic rule, not an accident.
  ⚠ **This is the trap behind "my server never answers"**: a `SLEEP` anywhere in
  the program delays **every** socket wait by its full duration, because the
  runtime is not scheduled while it runs. In concurrent code use
  **`SLEEP_ASYNC`**.
- ⚠ **`SLEEP_ASYNC(ms)` is the suspending sleep** (v0.12.0): it parks **only the
  calling task**, so other tasks keep running — that is the whole difference
  from `SLEEP`. A task body may sleep **more than once**, and a sleep inside a
  loop sleeps **every** iteration.
  ⚠ **Inside a `TEST_BUBBLE` it behaves exactly like `SLEEP`** (it advances the
  virtual clock and returns immediately) — the bubble's clock only moves at a
  `SLEEP`, so a suspending sleep there would never wake up (v0.12.0 / D-19).
  ⇒ a test that expected `SLEEP_ASYNC` to yield to another task inside a bubble
  must be rewritten, not "fixed".
- `TEST_BUBBLE(body)` runs `body` against a **virtual clock** starting at 0, so
  `SLEEP(3600000)` returns immediately and the same program yields a
  **byte-identical** time series on every run. The virtual wall origin is a **fixed
  constant** (2023-11-14) — `NOW()` inside a bubble is **not** the real time.
- ⚠ **Bubbles do not nest** (`E0030`). Tasks spawned inside belong to the bubble
  and **cannot escape**: `SPAWN` without a `SCOPE` is `E0058`, so they are always
  under a `SCOPE` inside the body, and structured concurrency drains them before the
  body returns.
- ⚠ **A bubble is deterministic *time*, not Go-`synctest` interleaving.** The
  virtual clock jumps **at the `SLEEP` call site**, so a task sleeping an hour does
  not let another task observe an intermediate moment. Getting that needs "resume
  mid-body for any suspension reason", which the runtime does not have yet.
- `ADVANCE(ms)` jumps the virtual clock forward (`E0030` outside a bubble) — use it
  to skip a wait without advancing time by sleeping twice.
- `TIMEOUT(d, body)` returns `ERR([kind: "Timeout"])` once the virtual clock passes
  `d`; the body is abandoned **at the nearest function boundary** (measured: the
  return signal does **not** escape the bubble), and `TIMEOUT`'s **return value is
  always correct** even when the abort could only reach part of the way. Bubble-only:
  outside one there is nowhere in a single-threaded runtime to check a deadline.
  Nested `TIMEOUT`s are allowed and the **tightest armed** deadline wins.

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
  it in: `SEED(RANDOM_HEX(16))` (§9.1). `std.encode` is the source; `std.rand`
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
- ⚠ **`std.rand`'s home in `wlwl:std.*` is a transitional form.** Spec §14 records
  that `std.ai` / `std.agent` become official packages outside std, and **v0.11.3
  M8 has now done exactly that** (`ai:` / `agent:` prefixes, `wlwl:std.ai` no
  longer resolves). The same logic applies to `std.rand` — it **has not** been
  moved yet, so do not treat its placement as permanent.
- Only uniform distributions. There is no `NORMAL` / Poisson / exponential and
  there is not planned to be — build those yourself from `UNIFORM` (polar /
  inverse-transform) when you need them; that is your algorithm choice.

### §9.6 `wlwl:std.sanitize` — output safety across five contexts (stdlib §13)

**Pick by output context. They are not interchangeable, and choosing the wrong
one is an error, not caution:**

| You are writing | Use | Why not the others |
|---|---|---|
| `href="…"` / `src="…"` — a **URL** | `URL_ENCODE` (§9.1) | `HTML_ESCAPE` leaves `&` readable and does not percent-encode; `HTML_SANITIZE` will strip the attribute entirely if the scheme isn't on the list |
| text or a quoted attribute — **markup** | `HTML_ESCAPE` | `HTML_SANITIZE` deletes every tag you wanted to keep |
| **rich HTML** from an untrusted source | `HTML_SANITIZE` | `HTML_ESCAPE` shows the user the tags instead of removing them |
| a **CSS identifier** — class / `id` / custom property | `CSS_ESCAPE_IDENT` | `HTML_ESCAPE` escapes the wrong set (`&` is legal in a CSS ident; `:` is not) |
| a **CSS string literal** — e.g. `content:` | `CSS_ESCAPE_STRING` | an unescaped `"` ends the literal early |
| a **CSS declaration** from untrusted input | `HTML_SANITIZE_CSS` — **allowlist first** | escaping alone never stops `expression(` / `url(javascript:)`; OWASP's CSS guidance leads with the allowlist for exactly this reason |
| a URL you intend to put in `href` / `src` | `HTML_SANITIZE_URL` | `URL_ENCODE` percent-encodes but **does not** judge the scheme |
| **server-rendered state** inlined into `<script>` | `HTML_SANITIZE_SCRIPT` | `STRINGIFY` alone is **not** an output-encoding function (see below) |

| Member | Notes |
|---|---|
| `HTML_ESCAPE(data)` | Escapes **exactly five** ASCII chars: `&` `<` `>` `"` `'` (apostrophe → `&#39;`). Non-ASCII passes through byte-identical. |
| `HTML_UNESCAPE(data)` | **Full** WHATWG table (2125 semicolon + 106 legacy entries). `&copy;` really decodes to `©`. Unknown named entities stay **verbatim** — that is HTML5's own tolerance, not a bug. |
| `HTML_SANITIZE(data, policy?)` | Whitelist sanitizer. `policy` is an optional `DICT`; omit it for the built-in conservative one. |
| `HTML_SANITIZE_SCRIPT(value)` | Serializes a **value** into a complete `<script type="application/json">…</script>` block. Takes a value, not a string — you cannot apply it partially. |
| `CSS_ESCAPE_IDENT(s)` | CSSOM "serialize an identifier". `123` → `\31 23` (**with the trailing space** — it disambiguates the hex length); `-` → `\-`; `café` unchanged. |
| `CSS_ESCAPE_STRING(s)` | CSSOM "serialize a string": wrapped in `"`. **`'` is not escaped** — CSSOM serializes strings with `"` always. |
| `HTML_SANITIZE_CSS(text)` | Allowlist extractor over a **declaration list**. Two *separate* outcomes: a non-allowlisted property is **silently dropped** (that is the allowlist working), while a rejected construct (`@import`, `expression(`, `javascript:`, `behavior:`, `-moz-binding`, **any** `url(`) yields `ERR`. |
| `HTML_SANITIZE_URL(u)` | Normalizes (trims control chars / surrounding whitespace) and judges the scheme with **the same logic** the sanitizer uses for `href`. Rejected → `ERR`. |

⚠ **`STRINGIFY` alone is not enough for inline script.** OWASP states plainly
that it "is not an output-encoding function": its output is valid JSON, but
embedding it in HTML needs protection for the *destination* context. Measured here —
`STRINGIFY` emits `<`, `>`, `&`, U+2028 and U+2029 **verbatim**. `HTML_SANITIZE_SCRIPT`
turns exactly those five into `\uXXXX`, which removes every `<` and therefore every
way to leave the script-data state. **It is total**: no value can make it fail on
content, so the "forgot to harden it" failure mode does not exist.

⚠ **`HTML_SANITIZE_URL` and the sanitizer agree by construction.** Both use one
`scheme_allowed`; they are not two implementations that happen to look alike. If you
ever see them disagree, one of them is wrong — that is a bug, not a policy.

⚠ **`HTML_SANITIZE_CSS` drops silently but reports loudly, on purpose.** "Not on the
allowlist" and "this is an attack" are different facts and the caller usually wants to
tell them apart.

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
  `URL_ENCODE`. See §9.6 — **none of them are interchangeable**, and picking the
  wrong one is an error, not caution.
- **Building an inline `<script>` payload by string concatenation** — FALSE, and
  this is the one that bites. `STRINGIFY(user)` gives valid JSON, but it emits
  `<`, `>`, `&`, U+2028 and U+2029 **verbatim**, so `</script>` in the data
  closes the element early and the rest runs as markup. OWASP: "`JSON.stringify()`
  is **not** an output-encoding function". Use `HTML_SANITIZE_SCRIPT(user)` — it
  takes the **value**, serializes, hardens and wraps in one step, so there is no
  partial application to forget. Its preferred form is still
  `<script type="application/json">` read back with `JSON.parse` — see §9.6.
- **`URL_ENCODE` makes a URL safe to put in `href`** — it percent-encodes, but it
  does **not** judge the scheme; `javascript:alert(1)` encodes perfectly and is
  still executable. Use `HTML_SANITIZE_URL` (or `HTML_SANITIZE`, which applies the
  same scheme list to URL attributes).
- **CSS-escaping makes untrusted CSS safe** — FALSE. OWASP's CSS guidance leads with
  the **allowlist**, not escaping: an escaped `expression(` is still `expression(`.
  Use `HTML_SANITIZE_CSS` for dynamic declarations; the two `CSS_ESCAPE_*` members
  are for identifiers and string literals, not for vetting a whole stylesheet.
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
- ⚠ **Do not use `LET MUT` to share mutable state between tasks.** `LET MUT` is the
  right tool for **single-task** mutable state, and that is exactly what the
  accumulator optimization is built on. But a cell reachable from two tasks in
  one `SCOPE` is an **anti-pattern** in this language — spec §17.0.1, and the
  reason is not taste: the scheduler only checkpoints at segment boundaries while
  the critical section is drawn by user code, so the two granularities cannot be
  aligned, and **reproducibility** (this language's first selling point) breaks
  when the interleaving depends on that hand-drawn boundary. Pass messages through
  channels; each task owns its own state.

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
`docs/wlwl-spec-reference.md` 附录 D (unchanged from the v0.10 edition).

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

### §24.5 Spec / impl / docs 三处引用 (v1.0 current)

| 文档 | 路径 | 用途 |
|------|------|------|
| 权威规范 | 独立仓 [`wlwl-spec-v1`](https://github.com/Laffinty/wlwl-spec-v1)(v1.0,11 册) | 唯一真相源;改规范走那边 |
| 仓内合并册 | `../docs/wlwl-spec-reference.md` | 次级载体 —— 机器守卫与 AI agent 读它(标题带册别前缀) |
| 历史归档(v0.6–v0.11 精简) | `../docs/history/20260902-09.md` / `20260915-22.md` | 旧版上下文(原文在 git 历史) |
| 内建注册表 | `../docs/appendix_G.md` | 106 条内建单一真相源 |

### §24.6 Standard library spec

`../docs/wlwl-spec-reference.md` — per-namespace signatures, `std.str`
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

### §25.4 `wlwl:std.text` — Unicode normalization, clusters, display width (stdlib §12, v0.11.3)

```wlwl
IMPORT("wlwl:std.text", ["NFC", "NFD", "NFC_QC", "GRAPHEME_COUNT", "WIDTH"]);

NFC("é");                       // -> "é"   (one codepoint)
NFD("é");                       // -> "é" (base + U+0301)
NFC_QC("é");                    // TRUE
GRAPHEME_COUNT("👨‍👩‍👧");       // -> 1
WIDTH("👨‍👩‍👧");                // -> 2
```

Unicode data version is pinned to **18.0.0**. Correctness evidence lives
**outside** the contract table, because the UCD source files are not in the repo:
`bin/grapheme-check` (official `GraphemeBreakTest.txt`, **all 853 rows**) and
`bin/width-check` (`EastAsianWidth.txt`, **all 1 114 112 code points**).

- ⚠ **Two coordinate systems.** `LEN` gives **code points**;
  `GRAPHEME_COUNT` gives **grapheme clusters**. For `"👨‍👩‍👧"` that is `5`
  versus `1`. Using `LEN` as a cluster count misaligns every line after the
  first emoji.
- ⚠ **No normalization means not equal.** `"é"` and `"é"` render
  identically, and `==` says FALSE. Compare with `==(NFC(a), NFC(b))` when
  you mean "the same user-visible string".
- ⚠ **`NFC_QC` is one-directional.** `TRUE` = definitely already NFC.
  `FALSE` = **"no guarantee"**, *not* "not NFC". Writing the latter teaches
  callers a false fact; the official suite has rows that are already NFC yet
  answer `FALSE`.
- `NFD` always increases the code-point count; `NFC` can only decrease it
  or leave it alone. Both are idempotent.
- ⚠ **`WIDTH` is cluster-aware.** Three emoji plus two ZWJ is 8 if you sum
  code-point widths, and **2** — the number of columns a terminal actually
  uses — if you take the max within each cluster.
- **Width rules** (all three measured, see stdlib §12.3): `A` (Ambiguous) =
  **1** · default-emoji-presentation `N` = **2** · `Mn`/`Me`/`Cf` = **0**
  (`Mc` is **not** zero — it is a spacing mark).
- ⚠ **`WIDTH` performs no normalization.** U+2122 and U+00A5 are the same
  character and both happen to measure 1 here, but that is two independent
  table lookups, **not** a recognized equivalence. Normalize first if you
  mean equivalence.

---

## §26. v0.11.2 增量备忘 — 编码 / Unicode 大小写 / 字符串检索 / 集合 / 超越函数

v0.11.2 added **41 standard-library members** and **no language-surface change**
— the language spec is frozen at v0.11 and `../docs/appendix_G.md` is
byte-for-byte unchanged. Every member below is `IMPORT`-gated like every other
`wlwl:std.*` member.

| Namespace | New members | Detail |
|---|---|---|
| `wlwl:std.encode` | `BASE64_ENCODE` `BASE64_DECODE` `HEX_ENCODE` `HEX_DECODE` `URL_ENCODE` `URL_DECODE` | §9.1 |
| `wlwl:std.text` | `TO_UPPER` `TO_LOWER` | §9.2 |
| `wlwl:std.str` | `INDEX_OF` `CONTAINS_SUB` | §9.3 |
| `wlwl:std.collection` | `CHUNK` `WINDOW` `DEDUP_BY` `MIN_BY` `MAX_BY` `SUM` `PRODUCT` `FOLD_RIGHT` `POSITION` `KEY_BY` | §9.4 |
| `wlwl:std.math` | `LN` `LOG2` `LOG10` `EXP` `TRUNC` `SIN` `COS` `TAN` `ASIN` `ACOS` `ATAN` `ATAN2` `SINH` `COSH` `TANH` `POW_MOD` `SIGN` `DIV_CEIL` `GCD` `LCM` `IS_SQRT` | §9.5 |

The five traps this batch introduced, each written up in the section it
belongs to:

1. **Decode failures are `ERR` values, not diagnostics** — `kind = "DecodeError"`,
   deliberately *not* `DomainError`, so a caller can tell "the base64 string
   was malformed" from "the argument was out of the function's domain".
2. **`URL_DECODE` keeps `+` literal** — that is RFC 3986, not
   `application/x-www-form-urlencoded`.
3. **`BASE64_DECODE` accepts both alphabets** and ignores `CR` / `LF`, and
   `BASE64_ENCODE(url_safe = TRUE)` swaps `+/` → `-_`.
4. **The global `UPPER` / `LOWER` stay ASCII-only** — replacing a global
   builtin is breaking, so `std.text` added *new* members instead. `ß` → `SS`
   changes the length; Greek `Σ` lowercases to `ς` word-finally.
5. **`INDEX_OF` / `CONTAINS_SUB` / `COUNT` all reject an empty `sub`** with
   `E0030` — an empty `sub` is not a wildcard. `INDEX_OF`'s offset is a
   **codepoint**, the same frame as `SUB`, so the hit can be sliced straight
   back out.

Plus one **prose fix that changed a published formula**: the `SUB` end-index
migration is `SUB(s, start, -(end_old, start))` — **length = end − start**,
subtraction order `end_old` first, and **do not wrap it in `NEG`** (that
re-negates the length and yields `""`). `SLICE` is **not** an alternative — it
takes an `ARRAY` first argument and rejects strings with `E0030`, and `E0030` is
a native diagnostic no §8.3 consumer can catch, so the "migration" would abort
the program instead of degrading it. `examples/sub_migration.wll` was added for
this and is gated with `PANIC`, not `RUN_TESTS` (a failing `RUN_TESTS` still
exits 0 and would not stop CI).

`std.math` also grew to **61 members** by v0.11.3 — §9.5 has the full
catalogue, grouped by family.

---

## §27. v0.11.3 增量备忘 — 哈希与 KDF / Unicode / 输出安全 / 时钟 / 正则 / 随机 / 进程 / 压缩 / 网络 / `RESULT` / 精确数 / JSON 深度 / `ai`·`agent` 降级

v0.11.3 added **115 standard-library members** plus **one visible-surface
change** (the `ai` / `agent` demotion, below). The language surface is
unchanged.

The spec's appendix A is a **frozen snapshot** (registry of the standard
library's member surface) — read that table for its contents rather than a
count quoted here, because the compiler keeps growing past it and a member
that exists in the compiler but not in appendix A is a **conformance gap**,
not an extra member. By the stdlib spec's own
per-chapter 设计依据 the four batches split as **58 (v0.11 and earlier) /
41 (v0.11.2) / 115 (v0.11.3) / 17 (v0.12.0)**. ⚠ **Current implementation
surface** (what you can actually call): **288 members across 25 namespaces**,
measured with `wlwl namespace`.

| Namespace | New members | Detail |
|---|---|---|
| `wlwl:std.encode` | `SHA256` `HMAC_SHA256` `PBKDF2_ITER` `ARGON2ID` `RANDOM_BYTES` `RANDOM_HEX` `TIMING_SAFE_EQ` | §9.1 |
| `wlwl:std.text` | `NFC` `NFD` `NFC_QC` `GRAPHEME_COUNT` `WIDTH` | §25.4 |
| `wlwl:std.sanitize` | `HTML_ESCAPE` `HTML_UNESCAPE` `HTML_SANITIZE` `HTML_SANITIZE_SCRIPT` `CSS_ESCAPE_IDENT` `CSS_ESCAPE_STRING` `HTML_SANITIZE_CSS` `HTML_SANITIZE_URL` | §9.6 |
| `wlwl:std.time` | `NOW` `MONOTONIC` `SLEEP` `TEST_BUBBLE` `ADVANCE` `TIMEOUT` | §9.11 |
| `wlwl:std.regex` | `RE` `RE_TEST` `RE_SEARCH` `RE_FIND_ALL` `RE_REPLACE` `RE_SPLIT` `RE_GROUP_COUNT` | §9.9 |
| `wlwl:std.rand` | `SEED` `UNIFORM` `INT_RANGE` `CODEPOINT` | §9.10 |
| `wlwl:std.env` | `ENV_GET` `ENV_SET` `ENV_KEYS` `ARGS` | §9.12 |
| `wlwl:std.process` | `PROCESS_RUN` `PROCESS_ID` | §9.12 |
| `wlwl:std.fs` | the path / directory / byte families — `PATH_JOIN` `PATH_DIR` `PATH_BASE` `PATH_EXT` `PATH_NORMALIZE` `LIST_DIR` `WALK_DIR` `MKDIR` `MKDIRS` `REMOVE` `COPY` `MOVE` `FILE_SIZE` `READ_BYTES` `WRITE_BYTES` | §9.12 |
| `wlwl:std.compress` | `ZSTD_COMPRESS` `ZSTD_DECOMPRESS` `ZLIB_COMPRESS` `ZLIB_DECOMPRESS` `GZIP_COMPRESS` `GZIP_DECOMPRESS` `DEFLATE_RAW_COMPRESS` `DEFLATE_RAW_DECOMPRESS` | §9.13 |
| `wlwl:std.net` | `URL_PARSE` `URL_JOIN` `HTTP_GET` `HTTP_POST` `HTTP_REQUEST` | §9.14 |
| `wlwl:std.result` | `RESULT_MAP` `AND_THEN` `FLATTEN` `NULL_OR` `RESULT_ALL` `RESULT_ANY` | §9.15 |
| `wlwl:std.math` | the `DECIMAL` family (8) + the `FRACTION` family (8) + approximate comparison (4) + descriptive statistics (9) | §9.5 B–E |
| `wlwl:std.json` | the low-level lexical layer and the path family — `JSON_TOKENIZE` `JSON_LOCATE` `JSON_VALIDATE` `JSON_GET` `JSON_SET` `JSON_DELETE` `JSON_MERGE` `JSON_CHUNK_ITEMS` `JSON_CHECK` | §9.7 |

### The v0.12.0 batch — 17 more members

| Namespace | New members | Detail |
|---|---|---|
| `wlwl:std.net` | `TCP_LISTEN` `TCP_ACCEPT` `TCP_RECV` `TCP_SEND` `TCP_PEER` `TCP_LOCAL_ADDR` `TCP_SET_TIMEOUT` `TCP_CLOSE` `TCP_CLOSE_LISTENER` | §9.14.1 |
| `wlwl:std.encode` | `UTF8_ENCODE` `UTF8_DECODE` `BYTE_LEN` | §9.1 |
| `wlwl:std.text` | `TO_INT` | §9.2 |
| `wlwl:std.str` | `CAT` | §9.3 |
| `wlwl:std.env` | `SCRIPT_PATH` | §9.12 |
| `wlwl:std.result` | `TAKE_ERR` | §9.15 |
| `wlwl:std.time` | `SLEEP_ASYNC` | §9.11 |

⇒ the full v0.12.0 memo (the seven semantic changes included) is **§28**.

`wlwl:std.collection` gained **no new members** in this batch, but **13 of its
27 members moved from R1 to R2** (M5) — `MAP` `FILTER` `CHUNK` `WINDOW`
`ENUMERATE` `ZIP` `UNIQ` `FLAT` `JOIN` `GROUP_BY` `DEDUP_BY` `KEY_BY`. That is
a **layer** change, not a visible-surface change: the member names, signatures,
semantics and diagnostics are unchanged, and the spec's §0.7 rule says a
layer move is not breaking. The pay-off is that the array-building members
stopped being quadratic — but `SORT` / `SORT_BY` and `UNIQ` are **still**
quadratic, and §9.4 has the measured numbers.

### §27.1 ⚠ Visible-surface change: `ai` / `agent` left `wlwl:std.*`

This is the one thing in v0.11.3 that changes **existing** code, so it is
worth reading on its own (`ADR-0034`, M8).

**The import line changed:**

```wlwl
IMPORT("ai:ask", ["ASK", "EMBED", "COMPLETE", "ASK_STREAM", "ASK_ALL"]);
IMPORT("agent:task", ["TASK", "TOOL", "CALL_TOOL", "MODEL", "CONTEXT"]);
```

**What stops resolving, and how it fails:**

- `IMPORT("wlwl:std.ai", …)` and `IMPORT("wlwl:std.agent", …)` → **`E0040`**
  ("module 'wlwl:std.ai' not found"). This is deliberate: an *empty* shell
  would make "did it move or not" invisible.
- `IMPORT("wlwl:ai", …)` → also **`E0040`**. `wlwl:` is the **reserved std
  prefix**, so the resolver returns "no" for any path starting with it, the
  path falls through to the bare-module-name branch, and the runtime goes
  looking for a file named `…\wlwl:ai.wll`. **Only the `ai:` / `agent:` prefix
  forms work.**

**There is no namespace-qualified call syntax** (stdlib §0.6): the grammar has
no `ns.MEMBER(...)` production, and the colon appears **only** inside the
`IMPORT` path string and in the rename slot. So `ai:ask.ASK(…)`, `ai.ASK(…)`
and `wlwl:std.ai.ASK(…)` are all parse errors, not runtime errors. After
`IMPORT`, a member is reachable **only as a bare name**.

**Rename at `IMPORT` time to dodge a type-name collision** — that is the
escape hatch:

```wlwl
IMPORT("agent:task", ["TASK": "AGENT_TASK", "TOOL", "MODEL", "CONTEXT"]);
```

**What did *not* change:** the member names, the signatures, the semantics and
the error codes (`E0080`–`E0083` for `ai`, `E0090`–`E0094` for `agent`) are
**unchanged**. Only the visible surface moved. The packages still compile
inside `wlwl-std` under the `real-ai` feature — that feature gates the **HTTP
transport** (mock ↔ real), **not** the members, so all five `ai` members are
visible and importable in a default build.

> *Measured on wlwl 0.11.3 (2026-10-20):* `IMPORT("ai:ask", …)` +
> `EMBED("hello")` returns the four-dimensional mock vector;
> `IMPORT("agent:task", ["TASK": "AGENT_TASK", …])` + `TOOL("t1", "desc")`
> returns `[description: desc, name: t1, params_schema: NULL,
> returns_schema: NULL, v: 0.4.0]`; `IMPORT("wlwl:std.ai", …)` fails with
> `E0040`.

### §27.2 The rest of v0.11.3, in one line each

The full contracts live in the §9.x sections named in the table above. The
one-line shape of each family:

- **Hashing and KDF** (§9.1) — `SHA256` / `HMAC_SHA256` are **fast digests**,
  never password hashes; `PBKDF2_ITER` / `ARGON2ID` are the password hashes
  and take **every parameter explicitly with no defaults**. `salt` is **hex
  text**, so a salt comes from `RANDOM_HEX` — never from raw bytes. The KDF
  lower bounds are OWASP 2025 minimums and out-of-range values are `E0030`;
  `algo` accepts only `"sha256"`. `TIMING_SAFE_EQ` never fails — unequal
  lengths are `FALSE`, not an error.
- **Unicode** (§9.2, §25.4) — two coordinate systems (`LEN` counts code
  points, `GRAPHEME_COUNT` counts clusters), and **no normalization means not
  equal**: compare with `==(NFC(a), NFC(b))`. `NFC_QC` is **one-directional** —
  `FALSE` means "no guarantee", not "not NFC".
- **Output safety** (§9.6) — **pick by output context**; `HTML_ESCAPE`,
  `HTML_SANITIZE`, `CSS_ESCAPE_*` and `HTML_SANITIZE_URL` are not
  interchangeable. `HTML_SANITIZE` is an **output-point** tool, not input-side
  cleaning.
- **Clocks** (§9.11) — `NOW()` is Unix **milliseconds** (wall clock — never
  use it to measure elapsed time); `MONOTONIC()`'s **absolute value is
  meaningless**, only differences are valid; `SLEEP` and `TEST_BUBBLE` give
  **deterministic** time, and bubbles **do not nest**.
- **Regex** (§9.9) — linear time, no backtracking: lookarounds, backreferences,
  named groups and `\p{…}` are **rejected**, not "added later".
- **Randomness** (§9.10) — **explicitly seeded, no hidden state**; draw
  members return `[value, next_state]`, not a scalar. `std.rand` is **not**
  cryptographic; salts and tokens go through `std.encode`.
- **Environment and processes** (§9.12) — `ENV_KEYS()` is **sorted**;
  `ENV_GET` of an unset variable is `NULL`, not `""`. `argv` must be an
  `ARRAY[STRING]` — the string form is command injection, and there is
  deliberately no `SYSTEM` member. `PROCESS_RUN` **blocks** the whole runtime.
- **Compression** (§9.13) — payload is `ARRAY[INTEGER]`, **compression is not
  encryption**, and the decompression cap is a hard boundary, not an OOM.
- **Network** (§9.14) — **plaintext `http:` only, no TLS**; `HTTP_*` is a
  **blocking primitive**; redirects are **not** followed; failures are `ERR`
  values, not diagnostics.
- **`RESULT` combinators** (§9.15) — `RESULT_MAP` / `AND_THEN` / `FLATTEN` /
  `NULL_OR` / `RESULT_ALL` / `RESULT_ANY`. ⚠ They **cannot see an `ERR`** —
  a call whose argument is an `ERR` never runs the callee's body, so
  `RESULT_MAP(ERR(e), f)` returns `ERR(e)` *because the body never ran*.
- **Exact numbers** (§9.5 B–E) — `DECIMAL` and `FRACTION` are `DICT`-shaped
  and their **field names are a public contract**. `scale` is **mandatory** on
  `DECIMAL_DIV` and `FRACTION_TO_DECIMAL`; `mode` is **mandatory** on
  `DECIMAL_ROUND`. Fractions are `i64`, not arbitrary precision — an overflow
  is reported as `ERR([kind: "OverflowError"])`, never as the run-terminating
  `E0035`.
- **JSON depth** (§9.7) — the new members fail with **`ERR` values** while
  `PARSE` / `STRINGIFY` still use the `E0070` / `E0071` diagnostics: **two
  failure conventions in one namespace, both by spec**.

---

## §28. v0.12.0 增量备忘 — 传输层 / 字节层 / 宿主挂起 / `TAKE_ERR` / `INT` 规则

> 对应规范侧 `wlwl-spec-v1` **v1.0.2**（本仓抄件 `../docs/wlwl-spec-reference.md`）。
> 编译器：**wlwl v0.12.0**。
> ⚠ **语言面没有动** —— 运算符、内建、语句自 v0.6 起未变；本版动的是
> **公共面（+17 员）** 与 **七处语义**（**D-14…D-20**）。

### §28.1 公共面扩充 17 员（D-13）

| 命名空间 | 新成员 | 说明 |
|---|---|---|
| `wlwl:std.net` | `TCP_LISTEN` `TCP_ACCEPT` `TCP_RECV` `TCP_SEND` `TCP_PEER` `TCP_LOCAL_ADDR` `TCP_SET_TIMEOUT` `TCP_CLOSE` `TCP_CLOSE_LISTENER` | 传输层族（§21.6）⇒ **服务端现在可以用 WLWL 自己写**。签名与超时形状见 §9.14.1 |
| `wlwl:std.encode` | `UTF8_ENCODE` `UTF8_DECODE` `BYTE_LEN` | 字节层（§11.2）⇒ `Content-Length` 要的**字节数**在这里。见 §9.1 |
| `wlwl:std.text` | `TO_INT` | **宽松**的文本 → 整数（裁剪 / 十六进制 / 小数向零截断）。见 §9.2 |
| `wlwl:std.str` | `CAT` | **变长**拼接（语言的 `+` 是二元的）。见 §9.3 |
| `wlwl:std.env` | `SCRIPT_PATH` | 入口脚本的路径（没有时 `NULL`）。见 §9.12 |
| `wlwl:std.result` | `TAKE_ERR` | 一步式「检查并消费」。见 §9.15 |
| `wlwl:std.time` | `SLEEP_ASYNC` | **可挂起**的睡眠。§28.2 的陷阱 |

⇒ 当时（v0.12.0）命名空间数与成员面读数见 `CHANGELOG.md`（历史记录）。
⚠ **别把那批读数当成现状** —— 当前实现成员面是 `wlwl namespace` 实测的
**288 members / 25 namespaces**，见 §29。

### §28.2 七处语义变更（D-14…D-20）

1. **传输层的等待由「阻塞冻结 `SCOPE`」改为「宿主挂起」**（**D-18** · breaking）。
   等待期间**只挂起当前任务**，**别的任务照常跑**；`timeout_ms` 是**那一次
   等待**的截止时间，`0` = **无截止时间**。
   ⇒ **迁移写法**：原先指望「等待期间别的任务不跑」的程序**要改** ——
   同一个 `SCOPE` 里**并发发多个连接**从「错的写法」变成「对的写法」。
2. **`std.fs` 的 IO 失败从原生诊断改为 `ERR` 值**（**D-14** · breaking）：
   读 / 写失败 ⇒ `ERR([kind:"FsError", op, reason])` **值**，**不再终止运行**；
   `E0061` / `E0062` 自此**不再由 `std.fs` 产生**（两个码本身不删）。
   ⇒ **迁移写法**：改用 `IS_ERR` / `ERR_PAYLOAD` 判定并自行处置。
3. **`ERR` 处置新增一步式构造 `TAKE_ERR`**（**D-15**），
   并补齐语言核心册 §8.6.2 第 3 项在**切段任务体路径**上的判定 ——
   顶层 / 普通闭包 / **切段**任务体三者行为**一致**（切段**不构成豁免**，
   它只改恢复粒度）。
4. **并发册 §17 增「宿主挂起」挂起点与「任务失败的报告义务」**（**D-16**）：
   宿主挂起恢复时**从段首重放**（与**不切段**一致）；**无人 `AWAIT` 的任务
   失败必须被报告** —— stderr **一行**
   （`[wlwl] unhandled task failure (task #N): …`），程序**继续**、
   **退出码 0**；被 `AWAIT` 的失败仍在**调用点**抛出且**不重复报告**。
   ⇒ 实质说明见 §28.4（`SKILL.md` 里也有一节）。
5. **语言核心册 §10 补 `INT` 的空白与解析规则**（**D-17** · 补充）：
   空白**一律不裁剪**、**只接受十进制**、字符串小数形态**拒**、解析失败是
   `ERR` **值**（非诊断）；`std.text::TO_INT` 仍是**宽松**那一个（裁剪 /
   十六进制 / 小数截断）—— 两者**故意不同**。
   ⇒ **迁移写法**：`INT` 内部严格转换、`TO_INT` 外部输入解析。
6. **气泡内 `SLEEP_ASYNC` 等同 `SLEEP`**（**D-19** · breaking）——
   气泡的虚拟钟**只在 `SLEEP` 时前进** ⇒ 气泡内挂起者**永醒不来**。
   ⇒ **迁移写法**：原先指望「气泡内 `SLEEP_ASYNC` 会挂起 / 会让别人插进来」
   的程序**要改**。
7. **「别名在写序列内部每轮重建」不再有摊还承诺**（**D-20** · 规范侧 breaking，
   **实现侧无行为改动**）⇒ 别把它**放在写序列内部**建立 ——
   把它**提到写序列之外**建立一次 ⇒ 只在首次写拷贝一次、之后原地写。

### §28.3 传输层：最容易踩且最容易被误诊的三条

`SKILL.md` 的 "Network programming" 一节有完整版（八条，每条都带实测故障形态）。
这里只留**最容易误诊**的三条：

- ⚠ **`SLEEP` 是阻塞原语**，它停住**整个运行时**（含 socket 就绪处理）⇒
  并发代码里用 `SLEEP_ASYNC`。
  ⚠ **典型误诊**：「字节要等进程退出才对端可见」看起来像**缺 flush**，
  但 `TCP_SEND` 直接写 socket（**没有用户态缓冲可 flush**）。把 `SLEEP`
  挪到发送**之后**，同一份程序从 ~3000 ms 变成 ~50 ms 送达。
  ⇒ **不要加 flush / `TCP_NODELAY` 的补丁 —— 没有东西可加。**
  `TCP_CLOSE` 同样**不是**送达的前提（虽然关掉是对的：对端会看到 EOF）。
- ⚠ **段首重放**（并发册 §17.0）：宿主挂起是**不切段**格 ⇒ 恢复从**当前段首**
  重跑 ⇒ 挂起点**之前**的语句**再执行一遍** ⇒
  **等待必须是所在段的第一条语句，且一段只放一次等待**；
  **有副作用的初始化放在任务之外**（否则实测泄漏 5 个 LISTENING socket）。
- ⚠ **`SCOPE` 先等、后取消**（并发册 §17.1 主条款 + §17.3 第 6 条）：作用域退出时
  运行期**先排干**调度器（把挂着的子任务**等完**）⇒ 停在 socket 上的孙任务
  **正常结束** ⇒ 客户端拿到**完整响应**，**不是 RST**；**之后**才**强制取消**
  仍非终态的那部分。⇒ 服务端仍要**自己数完成数**（别依赖 `SCOPE` 替你收尾）。
  ⇒ 订正记录见 §28.6 的 A4 行（这条曾被写成「客户端只看到 RST」，**错的**）。

### §28.4 任务失败的报告义务（D-16）

无人 `AWAIT` 的任务失败**必须**被报告到 **stderr 一行**（含任务身份 / 诊断码 /
诊断消息），程序**继续**、**退出码不变**；被 `AWAIT` 的失败仍在**调用点**抛出
且**不重复报告**。

⚠ 判定**延后**到程序收尾（失败发生的那一刻判不准「有没有人等」）
⇒ **「失败报告出现得晚」不是「没有报告」** —— 实时诊断链路要按进程退出或
显式 flush 来做。

⇒ **同一节的正文版在 `SKILL.md` 的「Task failures — the reporting obligation
(§17.9)」** —— 只读 `SKILL.md` 的 agent 也拿得到这条（那是本节被提上来的原因）。

### §28.5 本版的实测读数

- 契约测试 **2995 例全绿** / 0 failed / 2 ignored（102 个 target）
- `cargo fmt --check` **0 diff**；`clippy -D warnings` **0**
- 一致性册 §5.4 双向门禁 **25 行全部一致**（**0 红**）

### §28.6 二期复测带回来的订正（2026-10-25 · v0.12.0-rc2）

第三方对 `v0.12.0-rc2` 的二期实测 ⇒ **本仓审计里两条结论被推翻**，skill 的两条纪律
也因此**教错了**（已订正）：

| 条目 | 本仓审计原结论 | 二期实测（rc2） | 现在的口径 |
|---|---|---|---|
| **A4** 孙任务 | 「作用域退出 ⇒ 强制取消 ⇒ 客户端看到 RST ⇒ 符合规范」 | 作用域**先排干**（`scheduler_run_until_idle`）**再**强制取消 ⇒ 挂在 socket 上的孙任务**被等完** ⇒ 客户端收到**完整响应** | **先等、后取消**：§17.1「必须等待其子任务结束」是主条款；§17.3 第 6 条说的是「等不动的那部分怎么处置」。⇒ skill 的第 7 条纪律已改写 |
| **A5** `AWAIT` 已结束句柄 | 「⇒ `E0053` ⇒ §17.1 明文 ⇒ 合规」 | 对同一已结束句柄再次 `AWAIT` ⇒ **返回终值**；`E0053` 只在「不是句柄」或「句柄已失效（generation 不匹配）」时出现 | §17.1 的「过期句柄」= **失效**（stale），不是**已结束**。⇒ skill 的第 8 条纪律已改写 |
| **N5**（新） | — | **超时路径上没有恢复提示** ⇒ 超时唤醒不设提示 ⇒ 重放的 `TCP_ACCEPT` 不查备忘 ⇒ 弹出空队列 ⇒ `NULL` ⇒ `TCP_RECV(NULL)` ⇒ **`E0030`** ⇒ 任务死掉、连接句柄丢失、客户端收到 reset ⇒ **规范要求的超时形态在该形态下不可达** | **已修**：恢复提示改为**每一次「从挂起恢复」都置**（socket 就绪 / 截止时间到期 / 定时器到点 / 通道对端），挂起点不是 socket 时用哨兵值 `RESUME_NON_SOCKET` |
| **A3** 段首重放重做副作用 | 「规范明文，不算缺陷」 | 重放后 LISTENING **5 → 10** ⇒ 仍是规范明文（§17.0 第 1 条） | 维持：**不修**，但它是「写服务端时最该记住的一条」 |

⚠ **这条订正的通用教训**：本仓审计里那两条结论是**只读规范文本**得出的（没有实测），
而二期的实测证明**实现的行为比规范的字面读法更「宽」**。
⇒ **凡是能实测的结论，不要只靠读规范定案** —— 这是本仓审计方法本身要改的地方。

---

## §29. v0.12.1 增量备忘 — `TCP_SERVE` / `HTTP_REQUEST_ASYNC` / `std.http` 中间件 / TLS 扩展 / `STR`·`SPLIT` 订正

> 编译器：当前构建（`wlwl --version` 报 `0.12.1`；v0.12.0 批次的收口已并入 v0.12.1）。
> ⚠ **语言面仍然没有动** —— 运算符、内建、语句自 v0.6 起未变。本批动的是
> **标准库成员面**，并订正了三处**教错的口径**（H-1 / H-2 / H-3）。
> ⚠ **成员面读数**：`wlwl namespace` 实测 **288 members / 25 namespaces**
> （`23` × `wlwl:std.*` + 官方包 `ai` / `agent`）。规范附录 A 是**冻结快照**，
> 本批成员**尚待规范仓落表** ⇒ 那是**一致性缺口**，不是「额外成员」。

### §29.1 三处口径订正（H-1 / H-2 / H-3）

| 项 | 旧说法（**已废**，此处只作概述、不照抄原句） | 实测口径 |
|---|---|---|
| **H-1** | 旧稿称 `IF` 的第三实参不接受 `(a; b)` 形态 | **支持**。`IF(c, x, (a; b))` 第三实参是多语句组，取**最后一句**的值。实测 `IF(FALSE, 1, (LET(a,1); LET(b,2); +(a,b)))` → `3`。第二实参同理。⇒ 回归守护在 `wlwl-cli/tests/skill_examples.rs` |
| **H-2** | 旧稿称缺少数字 → 文本的转换 | **全局内建 `STR(x) -> STRING`**（附录 G §10.3），无需 `IMPORT`。实测 `STR(42)` → `"42"` |
| **H-3** | 旧稿称缺少通用的切分成员 | **全局内建 `SPLIT(s, sep) -> ARRAY`**（附录 G §10.5），无需 `IMPORT`。实测 `SPLIT("a,b,c", ",")` → `[a, b, c]`。`std.str` 只导出 `SPLIT_LINES`（按 `\n` 切） |

⚠ **`+` 不做任何隐式强转**。实测 `+(1, "a")` 与 `+("a", 1)` 都是
`E0030: +: cannot add integer and string`，提示原文含
「v0.3 has no implicit numeric coercion」。数字 → 文本走 `STR`；
变长拼接走 `std.str::CAT(parts...)`。

### §29.2 `std.net` 新增两员

| 成员 | 契约 |
|---|---|
| `HTTP_REQUEST_ASYNC(method, url, body?, headers?, timeout_ms?) -> DICT` | **非阻塞出站**。请求在工作线程上跑，**只挂起当前任务**（别的任务照常跑）。返回值与 `HTTP_REQUEST` **同形**（`{status, headers, body}`），实参形状**逐字同形**，运行期失败仍是 `ERR([kind:"HttpError", op, reason])` 值。**唯一差别 = 阻塞 vs 挂起**。元数/类型错 `E0022`/`E0030`。⚠ 段首重放 ⇒ 成员内部有幂等备忘，同一请求不会发两次 |
| `TCP_SERVE(listener, timeout_ms, opts, handler) -> INTEGER` | **原生帧内的接受循环**：accept → 读一次请求 → `handler(request)` → 回响应 → 下一条。返回**成功服务的连接数**；accept 超时 ⇒ 退出循环返回计数（不是错误）。**不关**监听器（调用方自己 `TCP_CLOSE_LISTENER`，幂等） |

**`TCP_SERVE` 的形状**

- **请求**（handler 实参）= `DICT`：`{method, path, version, headers, body}`；`headers` 键**小写**；`path` 是请求目标原样（含查询串）；`body` 是**有损 UTF-8**。
- **响应**（handler 返回值）= `DICT`：`{status: INTEGER, headers: DICT, body: STRING | ARRAY[INTEGER]}`。**三键缺一即那一条作废**，循环继续。
- **单条连接异常 ⇒ 记一笔、继续下一条**：对端断开 / 读失败 / handler 抛错 / 响应形状不对，**都只终止那一条**。失败以 `ERR([kind:"SocketError", op:"TCP_SERVE", reason])` 的形状落进宿主**告警槽**（软告警，不改程序语义）。
- **`opts`**（`DICT` 或 `NULL`；注意 `[]` 是空**数组**）：`max_header_bytes`（缺省 64 KiB）/ `max_body_bytes`（缺省 8 MiB）/ `read_timeout_ms`（缺省 `0` = 无上限）。**未知选项名 ⇒ `E0030`**（不静默忽略）。

⚠⚠ **handler 跑在原生帧里，不是任务体** ⇒ **handler 内不得调用宿主挂起点成员**
（`TCP_RECV` / `TCP_ACCEPT` / `SLEEP_ASYNC` …）。误用报**既有 `E0014`**
（不新增诊断码）：**指名该成员**、位置**指向调用点**、沿用
「used outside a task body」文案。`TCP_SERVE` 的可观察语义**不变**：
误用那条连接**不算**被服务、失败只记一笔、循环继续、对端收到 `500`。
⇒ **handler 里只返回 `DICT`**，别做 I/O。`TCP_SERVE_STREAM`（`std.http`
的 `SERVE_STREAM`）的 handler **同一契约**。

⚠ **`TCP_SERVE` 阻塞调用线程**：它的等待在原生帧内 ⇒ 期间**别的任务不跑**
（与 `TCP_ACCEPT`/`TCP_RECV` 的**宿主挂起**不同）。这是「接受循环在原生帧内
推进」这条候选的代价。⚠ **`TCP_SERVE` 与 `TCP_ACCEPT` 抢同一个待取队列**
⇒ 同一个监听**只交给其中一个**。

### §29.3 `TCP_SERVE` 迁移指引（从旧 HTTP handler 形状迁过来）

旧形状 = **静态展开的 accept 链**：一个 acceptor 任务 + 每条连接一个
`serve_one` 辅助闭包 + `TCP_ACCEPT`/`TCP_RECV` 写在任务体里。
**该形状现已不合规** —— `TCP_RECV` 在任务体外报 `E0014`（指名成员 + 指向
调用点）。迁到 `TCP_SERVE` 即可，**功能面不受影响**。

| 旧形状 | 新形状 |
|---|---|
| `LET(conn, TCP_ACCEPT(l, ms)); YIELD(); LET(chunk, TCP_RECV(conn, n, ms)); …` | 一句 `TCP_SERVE(l, timeout_ms, opts, handler)`；accept + 读都在原生帧里 |
| `serve_one(conn)` 闭包：先读再写 | `handler(request)` 闭包：收**已解析**的请求 `DICT`，**返回**响应 `DICT` |
| 手写请求解析 + `Content-Length` | `TCP_SERVE` 已解析好请求，只管构造响应 |
| 每条连接自己记「完了没」 | `TCP_SERVE` 返回服务计数；坏连接作废、不计数 |
| handler 里改状态 | 状态放**顶层 `LET MUT`**（或 `std.sync`），在 handler 外改 |

```wlwl
// ── 旧（现在在 TCP_RECV 处报 E0014）────────────────────────────
LET(acceptor, SPAWN(FUN(() , (
  LET(conn, TCP_ACCEPT(listener, 1000));
  YIELD();
  LET(chunk, TCP_RECV(conn, 4096, 1000));   // ← E0014: used outside a task body
  TCP_SEND(conn, build_response(AT_K(chunk, "text", "")));
  TCP_CLOSE(conn)))));

// ── 新（合规）──────────────────────────────────────────────────
LET(served, TCP_SERVE(listener, 1000, NULL, FUN((req),
  (LET(body, FORMAT("echo:{0}", AT_K(req, "body", "")));
   ["status": 200,
    "headers": ["content-type": "text/plain"],
    "body": body]))));
```

⚠ handler 里 `IF` 的两个分支都可以是多语句组 `(a; b)`（H-1，见 §29.1）。

### §29.4 `std.http` 中间件原语（限流 / 规则热加载 / 审计 / 真实 IP）

失败口径沿用两档（`E0022` / `E0030` + `ERR` 值），**零新增诊断码**；
`ERR` 载荷是 `{"kind": "HttpError", op, reason}` 一族。

**`RATE_LIMITER(name, limit, window_sec, opts?) -> DICT`** = `{"allowed":
BOOLEAN, "remaining": INTEGER, "reset": INTEGER, "limit": INTEGER, "mode":
STRING}` —— **判一次**限流。`opts`：`mode`（`"sliding"` | `"token_bucket"`，
缺省 `"sliding"`）、`refill`（仅令牌桶，每秒补给 ≥ 0，缺省 `limit`）、
`state`（`std.sync::SHARED_DICT` 句柄作跨求值器注册表）、`audit`（缺省
`TRUE`）、`client_ip` / `path` / `request_id`（审计归属，缺省 `""`）。
**未知键 / 类型错 / 取值不合法 ⇒ `ERR`**（不静默忽略）。
- **滑动窗口**：按**秒分桶**的 `std.sync` 原子计数器，窗口和 = 最近
  `window_sec` 秒之和；`allowed ⇔ 本次计上后窗口和 ≤ limit`，超限**回滚**
  （被拒的请求不占名额）。
- **令牌桶**：容量 `limit`、每秒补 `refill`（`0` = 永不补给）、每次判定扣 1，
  不够扣 ⇒ `allowed = FALSE`（**不欠账**）；桶初始**满**。
- `reset` = Unix 毫秒：下一次可能腾出名额的时刻。`remaining` = 还能通过几次。
⚠ **并发边界（实测，如实）**：计数走 `COUNTER_INC`（原子）⇒ 计数**不丢**，
同一秒内判定精确；**跨秒边界的瞬时竞态**理论上可多放行至多并发宽度个请求。
令牌桶的**补给记账**是读-改-写 ⇒ 极端并发下可能重复补给一次；**判定与扣减**
仍是原子精确的。

**`RULESET_LOAD(path, opts?) -> DICT`** = `{"rules": ARRAY[STRING], "mtime":
INTEGER, "version": INTEGER, "path": STRING}`。每行一条规则；空行与 `#`
开头跳过；行尾 `\r` 剥掉（CRLF 兼容）。
**热切换语义（钉死）**：`mtime` 变 ⇒ 重读文件、构造**全新**数组、一次
`SHARED_DICT_SET` **原子换引用**（分配新 `version`）⇒ **在途请求持有的旧数组
继续可用、内容不变**（数组是值语义快照）；`mtime` 未变 ⇒ **复用**现数组
（`version` 不动，审计 `result="cached"`）。`version` = **装载序号**
（每缓存条目一个 `COUNTER`，1 起）。文件不存在 / 读失败 ⇒ `ERR` 值
（`op:"RULESET_LOAD"`）。

**`CLIENT_IP(headers_or_request, remote_addr, trusted_proxies) -> STRING`**
—— 三参形、**无 opts**；每次调用产出一条审计行。`headers_or_request` =
头字典（键小写）**或**完整请求字典（取其 `"headers"` 子字典）；
`remote_addr` = **裸 IP 字面量**（不含端口，调用方先剥）；`trusted_proxies`
= `ARRAY[STRING]` 裸 IP。**比较是精确字符串比较**（两侧 `LOWER` + `TRIM`
后逐字比；不做 DNS / CIDR / 子网展开）。
**语义（钉死）**：只有 `remote_addr` **命中** `trusted_proxies` 才采信
`X-Forwarded-For`（多个头按出现序拼接再按 `,` 切跳）⇒ 取**从右往左第一个
不在可信列表里的跳**；全可信 ⇒ 取**最左**跳；没有 XFF / 空 ⇒ 回
`remote_addr`。`remote_addr` **不**可信 ⇒ **无视 XFF**、回 `remote_addr`
（攻击者自带的 XFF **绝不**当真实 IP）。审计 `result`：`"xff_used"` |
`"xff_all_trusted"` | `"ignored_xff"` | `"no_xff"`。

**审计（不导出的 `_AUDIT`）**：把 `{time, event, client_ip, path, result,
request_id}` 装进 `std.log::LOG` 的 `fields` ⇒ 落盘/落流那一行**是合法 JSON**
（`{"ts":…,"level":"INFO","msg":"http.audit","fields":{六字段}}`）。`time` =
`NOW()`（Unix 纪元毫秒）。三个新成员每次调用产出一条审计行（`opts.audit =
FALSE` 可关；`CLIENT_IP` 恒产出）。

⚠ **状态按 name 复用**：`std.sync` 的 `COUNTER` / `SHARED_DICT` **每次调用新建
容器**（无名字、无按名查回）⇒ 「同 name 复用同一计数/桶」由 `std.http` 补一层
注册表（一张 `SHARED_DICT`，name ⇒ 句柄）。缺省注册表句柄记在 `StdCtx.env`
（键 `WLWL_STD_HTTP_STATE`）⇒ **同一程序内**同 name 复用；传 `opts.state`
可跨求值器/线程。⚠ `StdCtx.env` 是**求值器级**快照（不是 OS 环境）⇒ 进程内
两个程序各有各的缺省注册表。⚠ 滑动窗口的秒桶按 `name × 秒` 懒创建、**不回收**
（长跑进程句柄线性增长）—— 如实登记。

### §29.5 TLS 不在标准库（`--ext` 显式加载的原生扩展）

`std.net` 是**明文 `http:` only**，`https://` 返回
`ERR([kind:"HttpError", … reason: scheme "https" is not supported])`。
TLS **不进标准库**，以**原生动态库扩展**（`wlwl-tls.dll` / `.so` / `.dylib`）
交付，由**宿主显式授权加载**：

```bash
wlwl run --ext /abs/path/wlwl-tls.dll --ext-sha256 <64-hex> server.wll
# 或 WLWL_EXT_PATH=/abs/path/wlwl-tls.dll
```

加载后其命名空间（`tls` / `crypto`）可 `IMPORT`。⚠ **扩展成员不进附录 A、
不进公共面清单**，只进一致性册 §5.5 的**实现侧扩展台账**。

⚠⚠ **加载一个原生库 = 把整个进程的权限交给它**（读文件、连网络、改内存、
起进程）—— **它不是沙箱**。宿主在加载前打印一行醒目告警。**信任模型七条
硬规则**（不可让渡）：

| # | 规则 |
|---|---|
| **T1** | **加载只能由宿主发起**：CLI `--ext <path>` 或 `WLWL_EXT_PATH`。**语言面不提供任何 `LOAD` 成员** ⇒ 脚本自己触发不了加载 |
| **T2** | **绝不从项目配置 / 项目目录自动发现**：`wlwl.toml`、程序同目录、`./ext/` 一律不扫 |
| **T3** | **路径必须绝对**、存在、是普通文件 |
| **T4** | **可选哈希钉扎**：`--ext-sha256 <hex>`；给了就必须匹配 |
| **T5** | **扩展不得占用 `std.*`**，也不得覆盖既有命名空间 ⇒ 宿主拒绝 |
| **T6** | **加载 = 交出全部进程权限**；宿主告警；**不得**宣称沙箱 |
| **T7** | **扩展成员只以「运行期注册」出现**：不进附录 A、不进公共面清单、只进 §5.5 台账 |

⇒ 因为 T1，`.wll` 程序**无法自行加载 TLS** —— 是**运维方**决定的。

### §29.6 已知边界（如实，不粉饰）

- **`std.fs::PATH_JOIN` 在 POSIX 上吃前导 `/`**：实测
  `PATH_JOIN("/tmp/x", "f")` ⇒ `"tmp/x/f"`，绝对根丢了。这是既有
  `std.fs` 成员的缺陷（已单独上报）；`std.http` 用 `CAT(root, "/", rel)`
  自己拼路径绕开它 ⇒ **不要**用 `PATH_JOIN` 去延展绝对根。
- **multipart 文件 part 落盘的是有损 UTF-8 文本**：本语言请求正文是
  `STRING`，二进制上传无法逐字节往返。二进制一律走 `ARRAY[INTEGER]`
  （`0..=255`）。
- **`STATIC_FILE` 的包含性判定是词法的**：`PATH_CONTAINS` **不解析符号链接**
  ⇒ `root` 底下指向 `root` 之外的符号链接仍判合法。另有 `READ_BYTES`
  **1 MiB** 策略上限（`WRITE_BYTES` 是 64 MiB）⇒ **写得出 ≠ 读得回**。
- **限流**：滑窗跨秒瞬时竞态 / 令牌桶极端并发重复补给 —— 见 §29.4
  `RATE_LIMITER` 的并发边界。


