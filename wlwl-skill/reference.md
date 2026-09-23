# WLWL v0.8 Reference (on-demand)

> Loaded only when `SKILL.md` points here. This is a lookup catalogue,
> not a tutorial. Section numbers (`§1.5`, `§8.3`, `§10.4`, `§17`, etc.)
> refer to `docs/standard/wlwl-spec-v0.8.md` (v0.7 archived at
> `docs/history/wlwl-spec-v0.7.md`; v0.6 archived at
> `docs/history/wlwl-spec-v0.6.md`; v0.7 is additive over v0.6; v0.8 is
> clarification / alignment over v0.7 with **no breaking observable
> behavior**).

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
- [§14. Reserved forms (§12 — v0.8 rewrite)](#14-reserved-forms-12--v08-rewrite)
- [§15. `--format json|jsonl` diagnostics (§11.1)](#15---format-jsonjsonl-diagnostics-111)
- [§16. `EXPORT` / `wlwl.toml` features (§9.1, §9.4)](#16-export--wlwltoml-features-91-94)
- [§17. Anti-patterns (extended)](#17-anti-patterns-extended)
- [§18. Concurrency (§17)](#18-concurrency-17)
- [§19. Notes on `interp.wll`](#19-notes-on-interpwll)
- [§20. v0.8 + v0.8.1 patch 字面增改备忘](#20-v08--v081-patch-)

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
| H | Integer overflow → `E0035` (native) | §B.8 / §2.2 | Was saturate + `W0015` in v0.5. v0.6+: native code per §11.2 — **NOT** `RESULT`-shaped `ERR`; cannot be caught by `EXPECT_ERR` / `UNWRAP_OR` / `TRY`. Migration: assert preconditions or restructure to avoid overflow. |
| J | `${expr}` interpolation | §B.9 / §1.8 | One `StrStart/StrEnd` pair per literal regardless of segment count; nested string literal inside `${...}` is `E0001` (by-implementation; spec prose ambiguous — see deviation D8-014) |

**v0.7 is additive** (spec §17 + Appendix G): new types `TASK`/`CHANNEL`,
17 concurrent builtins, error codes `E0052`–`E0058`, ERR kinds
`ChannelClosed` / `ChannelWouldBlock` / `Cancelled`. §0–§12 of v0.6 are
unchanged.

**v0.8 is clarification / alignment over v0.7** (no breaking observable
behavior; see `docs/history/deviations-v0.8.md` for the 12-item spec /
registry alignment batch): §12 rewrite (registry = single source of truth);
§4.3 NOT-propagation prose corrected; §17.1 YIELD placement demoted to
"理想语义 + 实现路径"; EXPECT_ERR promoted to §8.3 table row; `%`
float `E0030` listed; SHIELD / SCOPE(ERR) / AWAIT host diagnostic
clarifications; `=` triple-identity disambiguation; §5.1 LET/FUN
asymmetry normative; §2.1 / §10.11 `TASK` name-collision normative;
§0.1 version convention.

**v0.8.1 is a patch** with zero spec changes — just impl fixes for spec
字面 compliance: D8-009 (浮点指数 `1e2` / `1.5e2`); D8-010 (SUB third
arg is **length**, observable change); D8-011 (MUT 作 binding name);
D8-012 (字符串字面量下标 `"hi"[0]`); D8-013 (closure_cell.wll §3.3);
D8-014 (§1.8 内嵌字符串备忘留 v0.9).

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

- `//` line comments and `/* */` block comments (nestable).
- `MUT` is a **contextual** keyword — **only** between `LET` and `(`.
  Elsewhere (v0.8.1 D8-011) it is a normal identifier: `LET(MUT, "x")`,
  `LET MUT(MUT, 1)`, `FUN((MUT), MUT + 1)`, `PRINT(MUT)`, `+(MUT, 1)`
  all parse.
- `${` starts interpolation inside `"..."`; nested string literals inside `${...}` are `E0001` (by-implementation; spec prose ambiguous — D8-014留 v0.9).
- Escapes: `\$`, `\n`, `\t`, `\r`, `\\`, `\"`, `\/`, `\0`, `\b`, `\f`.
- `=` is the equality alias (not assignment). Three roles per §1.5:
  (a) `=` is `==` in **call position** (`=(a, b)` → `==(a, b)`);
  (b) `=` separates INDEX_SET sugar (`a[i] = v` → `INDEX_SET(a, i, v)`);
  (c) `=` separates default-param in `FUN((name = expr), body)`.
  No lookahead needed; the three roles don't overlap.
- Float exponents (v0.8.1 D8-009 + §1.7 EBNF `digits exponent`):
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

### §8.3 Consumer registry (14 entries — v0.8 added `EXPECT_ERR`)

`IS_OK`, `IS_ERR`, `UNWRAP_OR`, `OR_DIE` (deprecated), `TRY`, `UNWRAP`,
`ERR_PAYLOAD`, `WRAP`, `TYPE`, `==`/`!=`, `IF` condition, `&&`/`||`
left, `BOOL`, **`EXPECT_ERR`** (test-only: `x` is `ERR` → `OK(载荷)`;
non-`ERR` → `ERR(E0049)`).

None of these can intercept native codes (`E0034`, `E0035`, `E1003`,
`E0100` PANIC, `E0102` top-level ERR escape, `E0052`-`E0058` host
diagnostics, `E0030` type errors, etc.) — those are not `RESULT`
variants.

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
`-1` if missing (not an error). `INDEX_SET` rejects STRING receiver
(`E0030`).

**Literal subscripts** (v0.8 D8-004 + v0.8.1 D8-012, §A.2 grammar
`PostfixExpr = Primary { Postfix }`):
- Array literal: `[1, 2, 3][0]` → `1`
- Dict literal: `["a": 1]["a"]` → `1`
- String literal: `"hi"[0]` → `"h"` (v0.8.1 D8-012 — was E0011 pre-fix)
- Mixed chains: `[[1,2], [3,4]][1][0]` → `1`
- **Integer / Float / Boolean / NULL literal postfix is rejected** at
  parser (no `INDEX_GET` semantics for those types; INT/FLOAT/BOOL/NULL
  literal is not a "Primary" subject for INDEX_GET).

**SUB(s, start, len?) — length semantics** (v0.8.1 D8-010, §10.5):
```wlwl
SUB("Hello, world", 0, 5)   → "Hello"    // chars[0..5]
SUB("Hello, world", 7, 5)   → "world"    // chars[7..12] = 5 codepoints
SUB("Hello, world", 1, 5)   → "ello,"   // chars[1..6] = 5 codepoints
SUB("Hello, world", 0)      → "Hello, world"  // 2-arg default
SUB("Hello", 0, -1)         → ""          // negative len → 0
SUB("Hello", -1, 1)         → "o"         // negative start = from tail
```
Pre-D8-010 (v0.8.0) treated `len` as end-index — `SUB("Hello, world", 7, 5)` returned `""` not `"world"`. This was the **only v0.8.1 observable-behavior change**. Migration: `SUB(s, start, end_old)` → `SUB(s, start, -(start, end_old))` or `SLICE(s, start, end_old)` (SLICE on arrays is start/end).

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
`LET MUT` bindings.

## §14. Reserved forms (§12 — v0.8 rewrite)

**None reserved.** Per v0.8 §12 rewrite (D8-005), the §12 "保留形式"
list has been moved out — `CLASS` / `NEW` / `THIS` / `MODULE` /
`MODULE_REF` / `CALL` / `ARRAY(items...)` / `AND` / `OR` all have
working `BuiltinSpec` records in `BUILTIN_REGISTRY`
(`docs/appendix_G.md`) and behave as ordinary function names. The
registry is the single source of truth; the `b11_*` lock tests +
`registry::tests::appendix_g_anchors_match_v07_section_numbers` gate
consistency. Use `&&` / `||` for logic (they are `LexerMacro` short-circuit
sugar — `AND` / `OR` are aliases).

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

Beyond the top-19 in `SKILL.md` (rows 16–19 are v0.8.1 additions:

- **Out-of-range array index returns `NIL`** — FALSE; it raises `E0036` (§4.5).
- **`IS_NIL(x)` exists** — FALSE; the literal is `NULL`.
- **`APPEND(arr, x)` is a builtin** — FALSE; the global is `PUSH` (§10.4).
- **`E0009` is divide-by-zero** — FALSE; it's `E1003` (§2.2).
- **`LIST(1, 2, 3)` is the literal** — FALSE; use `[1, 2, 3]` (§4.8).
- **`FORMAT("…{0}…", [x])` unpacks `x`** — FALSE; FORMAT is variadic positional.
- **`POP(d, k, default)` is canonical** — DEPRECATED; prefer `AT_K`.
- **Passing `ERR(x)` to a non-consumer lets the function decide** — FALSE; §8.2 forwards.
- **`SUB(s, start, end_index)` reads start/end (pre-v0.8.1)** — FALSE since
  D8-010; third arg is now LENGTH. See §12 above.
- **`1.5e2` is a parse error** — FALSE since D8-009; float exponents are
  per §1.7 EBNF. See §4 above.
- **`LET(MUT, …)` is invalid (`MUT` is reserved)** — FALSE since
  D8-011; `MUT` is a context keyword, identifier-allowed elsewhere. See §4 above.
- **`"%f" % (1, 2)` with FLOAT raises `%` runtime error** — FALSE per
  spec §2.2 (v0.8 listed `%` + FLOAT → `E0030` type error).
- **`EXPECT_ERR(/(1, 0))` rescues division by zero** — FALSE; native
  codes `E0034`/`E0035`/`E1003` are not `RESULT`-shaped and bypass
  §8.3 consumers entirely. Avoid the conditions instead.
- **`NOT(ERR(x))` returns `FALSE`** — FALSE per v0.8 §4.3 corrected
  prose (D8-006); `NOT` propagates ERR (§8.2). Safe coercion:
  `NOT(BOOL(ERR(x)))`.
- **`class MyClass:` / `new MyClass` / `this` syntax** — TRUE now
  (v0.8 §12 rewrite); callable as function names. (`MyClass` is a
  `LexerMacro` / `ResolvedBuiltin` stub path; real OOP not defined in
  v0.8.)

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

### YIELD placement (理想语义 + v0.7 / v0.8 实现路径 — D8-007)

```wlwl
FUN(() , a; YIELD(); b)       // OK — Block direct child
IF(TRUE, YIELD(), 1)          // E0014 (current impl; ideal semantics allows)
IF(TRUE, (YIELD(); 1), 1)     // syntactically OK; nested remainder not resumed (v0.7.0 / v0.8 path-B segmentation)
LET(x, YIELD())               // E0014 (current impl)
```

Trailing `YIELD()` on the last segment completes the task with `NULL`.

v0.8 spec §17.1 demoted the "YIELD() must be in multi-statement Block
direct child" rule from hard normative to **implementation path** — the
hard rule is the static task-body segmentation in `yield_split.rs`.
Future suspension-based schedulers may lift the limitation without
claiming a breaking change.

### Channel close protocol

Detect close with `IS_ERR` + `AT_K(ERR_PAYLOAD(r), "kind", "?") == "ChannelClosed"`.
**Never** treat `NULL` from `TRY_RECV` as close.

### Host diagnostic vs user ERR (v0.8 §17.5 clarification)

`AWAIT(task)` re-raises *host* diagnostics (`E0101` stack overflow,
`E0053` invalid handle, etc.) using the `WlwlError` trace chain —
distinct from user `ERR(...)` values which pass through as ordinary
`RESULT` values. `SCOPE(ERR("x"))` propagates `ERR("x")` per §8.2
BEFORE the §17.1 fn-type check; `E0052` does not fire on this.

### v0.7.0 / v0.8 limits (spec §17.7)

- Single-thread cooperative scheduler (no CPU parallel speedup).
- `CHANNEL_SEND`/`RECV` do not suspend (WouldBlock).
- No deadlock detector.
- Nested YIELD does not resume nested-constructor remainder (path B; may lift in future).
- In-flight sibling cancel may be unobservable under sync run.
- Prefer `LET MUT` for shared mutable cells.

## §19. Notes on `interp.wll`

`interp.wll` is the in-skill gold-standard for v0.6 core (blocks A–N)
plus v0.8.1 patch examples (SUB length semantics, `MUT` as binding
name, float exponents, string literal subscript). Concurrency gold
minis: `examples/concurrency.wll` and `examples/concurrency_cancel.wll`.

Block (H) demonstrates overflow handling via `safe_add`; actual
`+(i64::MAX, 1)` is native `E0035` + exit 1 (NOT a `RESULT`-shaped
ERR; cannot be caught by §8.3 consumers — see §17 anti-patterns).

## §20. v0.8 + v0.8.1 patch 字面增改备忘

For reference. Every v0.8 / v0.8.1 item that affects writing `.wll`
source — pulled from `docs/history/deviations-v0.8.md` D8-001..D8-014
and `docs/history/audit-report-v0.8.1.md`.

### v0.8 (12-item batch — D8-005 + D8-006 + D8-007 + D8-008 + Phase B)

| # | Dev | § | Change |
|---|-----|---|--------|
| 1 | D8-005 | §12 | §12 "保留形式" 重写至 BUILTIN_REGISTRY;`CLASS`/`NEW`/`THIS`/`MODULE`/`MODULE_REF`/`CALL`/`ARRAY(...)`/`AND`/`OR` 全部有 `BuiltinSpec` |
| 2 | D8-006 | §4.3 | NOT 不再是 §8.2 透传的唯一例外;`NOT(ERR(...))` 透传 ERR(同所有运算符);安全写法 `NOT(BOOL(ERR(...)))` |
| 3 | D8-007 | §17.1 | YIELD 位置限制从 hard rule 降为 "理想语义 + 实现路径"(yield_split.rs 静态切段) |
| 4 | D8-008 | §1.5 | `=` 三重身份消歧:OpToken(= → ==)/ INDEX_SET 语法糖 / Param 默认值 |
| 5 | D8-008 | §4.5 + §A.2 | 字面量下标允许 (`[1, 2, 3][0]` / `["a":1]["a"]`);grammar 早允许,prose 改一致 |
| 6 | D8-008 | §1.6 | `int_lit` (裸数字) / `int_literal` (带符号) 拆分;lexer 不读前导符号,parser 在 unary-minus 路径 desugar |
| 7 | D8-008 | §5.1 | LET / FUN 不对称 normative:LET 是声明语句不是值表达式;不得作实参 |
| 8 | D8-008 | §8.3 | `EXPECT_ERR` 入 §8.3 consumer 表(原仅 prose 段末) |
| 9 | D8-008 | §2.2 | `%` 浮点 E0030 列入触发列表 |
| 10 | D8-008 | §17.3 / §17.5 | SHIELD 不创建新作用域;SCOPE(ERR) 透传(不触发 E0052);AWAIT 宿主诊断 vs 用户 ERR 区分 |
| 11 | D8-008 | §2.1 + §10.11 | 类型名 `TASK` 与 `wlwl:std.agent.TASK` / `wlwl:std.ai.TASK` 同名 — 不冲突;推荐全限定 |
| 12 | D8-008 | §0.1 | 版本约定 `major.minor`;v0.8 起新章节用 `v0.8` (省 patch 段),`[v0.7.0]` 等历史性 "不承诺" 行保留 |

### v0.8.1 patch (impl-only — zero spec changes; spec 仍是 v0.8)

| # | Dev | § | Fix |
|---|-----|---|-----|
| 1 | D8-009 | §1.7 | 浮点指数字面量启用(`1e2` / `1.5e2` / `1.5e-2` / `1E3` / `2.5e+1`) — lexer `read_number` 漏实现,加 `e`/`E` 指数路径 |
| 2 | **D8-010** | §10.5 | **SUB 第三参数严格按 length 实现**(原 end-index 语义错位,**唯一 observable change**);`SUB("Hello, world", 7, 5)` → `"world"` |
| 3 | D8-011 | §1.4 | MUT 作普通标识符在所有非 modifier 位置允许(`LET(MUT, ...)` / `PRINT(MUT)` / `FUN((MUT), ...)` / pattern 位置) |
| 4 | D8-012 | §A.2 grammar + §4.5 prose | 字符串字面量允许下标 postfix (`"hi"[0]` → `"h`);仅 StringLit 走 apply_postfix_loop,Int/Float/Bool/Null 仍拒 |
| 5 | D8-013 | §3.3 | `examples/closure_cell.wll` 改写为 `LET MUT + SET` 版本(原 shadow 形式返 NULL);`v07_fidelity` baseline 同步更新 |
| 6 | D8-014 | §1.8 | 内嵌字符串字面量在 `${...}` 内不允许 — 当前 impl 与 spec prose 同处理,**留 v0.9 spec 增补 normative note** |

### Spec / impl / docs 三处引用

| 文档 | 路径 | 用途 |
|------|------|------|
| 权威规范 | `../docs/standard/wlwl-spec-v0.8.md` | 唯一真相源 |
| v0.7 归档 | `../docs/history/wlwl-spec-v0.7.md` | 历史 §17 上下文 |
| v0.6 归档 | `../docs/history/wlwl-spec-v0.6.md` | §0–§12 核心 |
| 偏差登记 | `../docs/history/deviations-v0.8.md` | D8-001..D8-014 |
| 第三方审计源 | `../docs/history/audit-report-v0.8.1.md` | 本周期工作的源头驱动 |
| 内建注册表 | `../docs/appendix_G.md` | 110 条内建单一真相源 |
