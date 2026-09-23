---
name: writing-wlwl
description: "Writes WLWL v0.8.1 .wll source files (spec docs/standard/wlwl-spec-v0.8.md; v0.8.1 is a patch on v0.8 with zero spec changes). Covers v0.6 core (truthy overhaul, &&/|| short-circuit, IF ERR-routing, ! canonical, AT_K rename, string subscript, LET MUT, overflow->E0035, ${} interpolation), v0.7 §17 concurrency (SCOPE/SPAWN/AWAIT/YIELD/TASK_*/SHIELD/CHANNEL_*), v0.8 字面增改 (§12 重写至 BUILTIN_REGISTRY; EXPECT_ERR 入 §8.3 表; % float E0030 列入触发列表; SHIELD / SCOPE(ERR) / AWAIT 宿主诊断澄清; = 三重身份消歧; §5.1 LET/FUN 不对称 normative; §2.1/§10.11 TASK 同名 normative; §17.1 YIELD 位置 demoted; §4.3 NOT 透传 prose 反向; §1.6 拆 int_lit/int_literal), and v0.8.1 patch 修复 (浮点指数字面量 §1.7; SUB 第三参数按 length §10.5; MUT 作 LET binding 名 §1.4; 字符串字面量下标 §A.2). Use when the user asks for WLWL code, a .wll file, or anything targeting wlwl-spec-v0.8 (or v0.6/v0.7 core). Do NOT use for v0.5 or earlier (POP-not-AT_K, no LET MUT), the Rust implementation, or the formatter."
---

# Writing WLWL (v0.8.1)

## When to load / NOT to use

**Use when:**
- The user asks for a `.wll` source file, a WLWL program, or a v0.6 / v0.7 / v0.8 idiom.
- A task targets `wlwl-spec-v0.8.md` (current) or `wlwl-spec-v0.6.md` / `wlwl-spec-v0.7.md` (archived).
- Reviewing or debugging v0.6 / v0.7 / v0.8 source (concurrency included).

**Don't use when:**
- The source targets WLWL v0.5 or earlier (different truthy rules, `POP`-not-`AT_K`, no `LET MUT`).
- The task is editing the Rust implementation in `impl/` or the formatter spec.
- The user wants `wlwl.exe` CLI documentation (that's `wlwl help` / `wlwl <subcmd> --help`).
- The artifact is a Rust test, ADRs, or build-system file.

## Writing flow

Tick these as you go:

```
WLWL writing progress (v0.8.1):
- [ ] 1. Sketch the AST shape (use the operators/builtins section below)
- [ ] 2. Decide mutation — any SET later means LET MUT(name, value) now
       (v0.8.1: MUT itself can also be the binding name, e.g. LET(MUT, "x"))
- [ ] 3. Concurrency? wrap SPAWN in SCOPE; YIELD mostly OK outside
       strict blocks (v0.8 spec §17.1 demoted "Block 直接子项" to
       implementation-path note)
- [ ] 4. SUB uses length semantics: SUB(s, start, len) where len is
       LENGTH (codepoint count) — v0.8.1 D8-010 observable change
       (pre-D8-010 treated len as end-index)
- [ ] 5. Float exponents now work: 1.5e2 / 1e-3 / 1E3 all valid
       (v0.8.1 D8-009; pre-D8-009 raised E0011)
- [ ] 6. String literal subscript works: "hi"[0] → "h"
       (v0.8.1 D8-012; pre-D8-012 raised E0011 in LET slot)
- [ ] 7. Write the .wll file (one stmt per line, semicolons, no leading indent)
- [ ] 8. Run `wlwl run <file>` — MUST exit 0 with expected stdout
- [ ] 9. If 8 fails: consult reference.md for the failing token/operator
```

Do not skip step 8. `wlwl run` is the source of truth. `wlwl fmt --check` is best-effort (see Verification loop).

## Truthy / falsy — spec §2.3

Eight falsy values: `FALSE`, `NULL`, `0`, `0.0`, `""`, `[]` (empty array), `DICT()` (empty dict), `NaN`.

**Everything else is truthy**, including non-empty strings, non-empty containers, `OK(FALSE)`, and v0.7+ `TASK`/`CHANNEL` handles.

`ERR(...)` is **not** in this list — it is its own propagation mechanism per §8.2. Passing `ERR(x)` to a non-consumer function transparently forwards the `ERR`; it does NOT make the function body "skip" because the arg was falsy. Only §8.3 consumers (`IS_OK`, `IS_ERR`, `UNWRAP_OR`, `OR_DIE`, `TRY`, `UNWRAP`, `ERR_PAYLOAD`, `WRAP`, `TYPE`, `==`/`!=`, `IF` condition, `&&`/`||`, `BOOL`, `EXPECT_ERR`) can inspect an ERR safely.

## Operators / builtins — cheat sheet (v0.6 / v0.7 / v0.8 + v0.8.1 patch)

**Equality** (spec §2.4): `=(a, b)` and `==(a, b)` are aliases (§1.5 call-position desugar; §1.5 `=` triple-identity — NOT same as `INDEX_SET`'s `=` or `Param`'s `=`). `!=(a, b)`. Cross-type numeric: `==(1, 1.0)` is `TRUE`. Function/handle identity: `==(f, f)` and `==(h, h)` are `TRUE` (v0.7+ handle identity lock).

**Comparison**: `<(a, b)`, `>(a, b)`, `<=(a, b)`, `>=(a, b)`.

**Arithmetic**: `+(a, b)`, `-(a, b)`, `*(a, b)`, `/(a, b)`, `%(a, b)`. `+` is also string concat and array concat. Integer overflow → native `E0035`; divide-by-zero → native `E1003` (note: **not** `E0009`; these are native codes per §11.2 — NOT ERR, cannot be caught by `EXPECT_ERR` / `UNWRAP_OR` / `TRY`; see anti-pattern #17).

**Boolean**: `&&(a, b)` / `||(a, b)` short-circuit; right side not evaluated when left decides. `NOT(x)` and `!(x)` are equivalent. `NOT(ERR(...))` propagates ERR (per §4.3 corrected prose in v0.8 D8-006); safe coercion idiom: `NOT(BOOL(ERR(...)))` (BOOL is a §8.3 consumer).

**Indexing**: `xs[i]` returns one element; out-of-range array/string index raises `E0036`. `INDEX(xs, v)` returns the index of `v` in `xs`, or `-1` if not found (NOT an error).

**Literal subscripts** (v0.8.1 D8-012 + spec §A.2 grammar `PostfixExpr = Primary { Postfix }`):
- Array literal: `[1, 2, 3][0]` → `1` (v0.8 D8-004)
- Dict literal: `["a": 1]["a"]` → `1` (v0.8 D8-004)
- String literal: `"hi"[0]` → `"h"` (v0.8.1 D8-012 — pre-D8-012 raised E0011)
- Mixed chains: `["a": 1]["a"][0][0]`, `[[1,2], [3,4]][1][0]` all legal
- Integer / Float / Boolean / NULL literal postfix **rejected** at parser (no `INDEX_GET` semantics; pre-existing behavior preserved)

**Float exponents** (v0.8.1 D8-009 + spec §1.7 EBNF `digits exponent`):
```
1e2          → 100.0
1.5e2        → 150.0
1.5e-2       → 0.015
1E3          → 1000.0
2.5e+1       → 25.0
```
Pre-D8-009 all of these raised `E0011 expected ')', got Ident("e2")`.

**Identifier names** (v0.8.1 D8-011 + spec §1.4): `MUT` is a **context keyword** — only after `LET` modifier slot. Other positions can use `MUT` as a regular identifier:
- `LET(MUT, "x")` — binding name = `"MUT"`, immutable
- `LET MUT(MUT, 1)` — first `MUT` is modifier, second is binding name, mutable
- `LET(MUT: INTEGER, 0)` — binding name + type annotation
- `FUN((MUT), MUT + 1)` — FUN parameter position
- `PRINT(MUT)` / `+(MUT, 1)` — call-arg / expr position (all valid)
- `${MUT}` inside string interpolation — valid

**SUB(s, start, len?) — length semantics** (v0.8.1 D8-010 + spec §10.5):
The third arg is **length** (codepoint count), not end-index.
```wlwl
SUB("Hello, world", 0, 5)   → "Hello"    // chars[0..5]
SUB("Hello, world", 7, 5)   → "world"    // chars[7..12] = 5 codepoints
SUB("Hello, world", 1, 5)   → "ello,"   // chars[1..6] = 5 codepoints
SUB("Hello, world", 0)      → "Hello, world"  // 2-arg default = to end
SUB("Hello", 0, -1)         → ""          // negative len → 0
SUB("Hello", -1, 1)         → "o"         // negative start counts from tail
```
Pre-D8-010 (v0.8.0 and earlier) treated `len` as end-index — `SUB("Hello, world", 7, 5)` returned `""` not `"world"`. Migration: `SUB(s, start, end_old)` → `SUB(s, start, -(start, end_old))` or `SLICE(s, start, end_old)` (SLICE uses start/end semantics on arrays; SUB is the string-specialized variant).

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

## Reserved forms — spec §12 (v0.8 rewrite)

**None reserved.** §12 v0.8 rewrote the "保留形式" list to point at the
`BUILTIN_REGISTRY` (`docs/appendix_G.md`) as the single source of truth:

- `CLASS` / `NEW` / `THIS` — `ResolvedBuiltin` (OOP stub paths; callable, returns the class table / new instance / self)
- `MODULE` / `MODULE_REF` — `ResolvedBuiltin` / `LexerMacro`
- `CALL(fn, args...)` — `ResolvedBuiltin`
- `ARRAY(items...)` — `LexerMacro` (constructor for empty / single-element arrays)
- `AND` / `OR` — `LexerMacro` (`&&` / `||` short-circuit sugar)

Use them as ordinary function names; no reserved-keyword prohibition in
modern (v0.7+) source. The `b11_*` lock tests + the
`registry::tests::appendix_g_anchors_match_v07_section_numbers` test gate
registry consistency.

## Concurrency — spec §17 (v0.7, with v0.8 clarifications)

All concurrent names are **global builtins** (no IMPORT). None are ERR
consumers: `AWAIT(ERR("x"))` transparently propagates (§8.2).

### Core

```wlwl
LET(v, SCOPE(FUN(() ,
    LET(h, SPAWN(FUN(() ,
        YIELD();          // must be a direct child of a multi-stmt block
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
| `YIELD()` | cooperative checkpoint |
| `TASK_CURRENT()` / `TASK_IS_CANCELLED()` | handle / cancel probe |
| `TASK_CANCEL(h)` / `TASK_CANCEL_PARENT()` | advisory cancel (returns `NULL`, never raises) |
| `SHIELD(fn)` | defer cancel until outermost shield exits; **does not** absorb `fn`'s ERR |
| `CHANNEL_NEW(buf)` | `buf=0` → sync channel; type `CHANNEL` |
| `CHANNEL_SEND` / `CHANNEL_RECV` | see limits — prefer TRY_* in v0.7.0 |
| `CHANNEL_TRY_SEND` / `CHANNEL_TRY_RECV` | non-blocking: `TRUE`/`FALSE`, value / `NULL` |
| `CHANNEL_CLOSE` / `CHANNEL_LEN` / `CHANNEL_CAP` | close (idempotent) / inspect |

### Hard rules

1. **Explicit SCOPE only.** `SPAWN` outside any `SCOPE` → `E0058`. No free-floating tasks.
2. **`YIELD` placement (v0.8 §17.1 demoted prose).** The hard rule is
   the implementation path (static task-body segmentation in
   `wlwl-eval/src/yield_split.rs`): `YIELD()` works cleanly as a
   direct child of a multi-statement block. Other positions may produce
   `E0014`:

   ```wlwl
   FUN(() , a; YIELD(); b)     // OK
   IF(TRUE, YIELD(), 1)        // E0014 — wrap: IF(TRUE, (YIELD(); 1), 1)
   LET(x, YIELD())             // E0014
   ```

   v0.8 spec §17.1 explicitly frames this as "理想语义 + v0.7 / v0.8
   实现路径", not a normative language rule. Future suspension-based
   schedulers may lift the limitation without claiming a breaking
   change (deviation D8-007).

3. **Close signal is ERR, not NULL.**

   ```wlwl
   LET(r, CHANNEL_TRY_RECV(ch));
   IF(IS_ERR(r),
       IF(==(AT_K(ERR_PAYLOAD(r), "kind", "?"), "ChannelClosed"),
           "done",
           "other"),
       r)   // plain value
   ```

   Kinds: `"ChannelClosed"` | `"ChannelWouldBlock"` | `"Cancelled"`.

4. **v0.7.0 blocking ops do not suspend.** `CHANNEL_SEND` on full and
   `CHANNEL_RECV` on empty return `ERR(kind="ChannelWouldBlock")`.
   Use `CHANNEL_TRY_*` in a poll loop or size the buffer.

5. **Cancel is cooperative.** `TASK_CANCEL*` only sets flags. Observe
   with `TASK_IS_CANCELLED()`. `AWAIT` of a cancelled task returns
   `ERR(kind="Cancelled")` — consume it or it escapes as `E0102`.

6. **Shared state = `LET MUT`.** Prefer `LET MUT` for cells written from
   children. `SET` on a local/late-bound `LET` is `E0024`.

### Canonical shapes

```wlwl
// Producer / consumer with TRY_* (v0.7.0-safe)
SCOPE(FUN(() ,
    LET(ch, CHANNEL_NEW(4));
    LET(p, SPAWN(FUN(() ,
        CHANNEL_TRY_SEND(ch, 1);
        CHANNEL_TRY_SEND(ch, 2);
        CHANNEL_CLOSE(ch);
        0
    )));
    AWAIT(p);
    LET MUT(acc, 0);
    LET MUT(done, FALSE);
    WHILE(NOT(done),
        LET(x, CHANNEL_TRY_RECV(ch));
        IF(IS_ERR(x),
            SET(done, TRUE),
            SET(acc, +(acc, x)))
    );
    acc
))

// SHIELD cleanup then observe cancel
SCOPE(FUN(() ,
    LET(h, SPAWN(FUN(() ,
        SHIELD(FUN(() , "cleanup"));
        TASK_IS_CANCELLED()
    )));
    AWAIT(h)
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

Names from `wlwl:std.*` are NOT global — they must be IMPORTed before use. Errors: missing module `E0040`; cycle `E0041`; name not exported `E0023`; duplicate `E0021`.

v0.7 concurrency names **are** global (Appendix G) — do not import them.

## Error model — spec §8

**§8.2 transparent propagation**: any function call whose argument evaluates to `ERR` does not run the body; the `ERR` is forwarded. Pass plain values to non-consumer functions; consume `ERR` only via §8.3.

**§8.3 ERR consumers** (14 entries — v0.8 added `EXPECT_ERR` to the explicit table): `IS_OK`, `IS_ERR`, `UNWRAP_OR`, `OR_DIE` (deprecated — `W0051`), `TRY` (function-body only), `UNWRAP` (PANICs on ERR — `E0100`), `ERR_PAYLOAD`, `WRAP`, `TYPE`, `==`/`!=`, `IF` (condition position), `&&`/`||` (left side), `BOOL`, **`EXPECT_ERR`** (test-only — returns `OK(载荷)` on `ERR` input, `ERR(E0049)` on non-ERR input). Each either extracts payload info or returns a default.

**§8.4 PANIC**: `PANIC(msg)` (msg must be STRING or DICT) terminates with `E0100`. Bypasses propagation entirely. `UNWRAP` on ERR and `NEG(i64::MIN)` also PANIC.

**§8.5 top-level escape**: an uncaught ERR reaching the top level terminates with `E0102` and exit code 1. Always consume at the boundary.

**v0.7 concurrent ERR kinds** (§8.1): `ChannelClosed` · `ChannelWouldBlock` · `Cancelled` (always dict payloads).

**v0.7 concurrent diagnostics**: `E0052`–`E0058` (see reference.md).

**v0.8 spec §17.5 host diagnostic clarification**: `AWAIT` of a child task re-raises *host* diagnostics (`E0101` stack overflow, invalid handle `E0053`, etc.) using the `WlwlError` trace chain — distinct from user `ERR(...)` values which pass through as ordinary `RESULT` values. Don't conflate the two when debugging.

**v0.8 spec §17.5 `SCOPE(ERR("x"))` clarification**: `SCOPE(ERR("x"))` propagates the `ERR` per §8.2 BEFORE the parameter-type check (`fn` must be a function → `E0052`); the result is `ERR("x")`, not `E0052`.

## Standard library pointers

For the full ~70-name catalogue see `reference.md` §9. Categories:

- **Global builtins (§10.2–§10.5 + §17)**: `PRINT`, `PRINT_ERR`, `INPUT`, `LEN`, `TYPE`, `STR`, `INT`, `FLOAT`, `BOOL`, container ops (`PUSH`/`SHIFT`/`SLICE`/`CONCAT`/`INDEX`/`REVERSE`/`INDEX_GET`/`INDEX_SET`/`AT`/`KEYS`/`VALUES`/`HAS`/`MERGE`/`REMOVE_KEY`/`DEL`), string ops (`SUB`/`SPLIT`/`REPLACE`/`UPPER`/`LOWER`/`TRIM`/`STARTS_WITH`/`ENDS_WITH`/`REPEAT`/`PAD_*`/`CODEPOINTS`/`FROM_CODEPOINTS`), `AT_K`/`POP`, and the §17 concurrency set.
- **`wlwl:std.collection`** (§10.6): `MAP`, `FILTER`, `REDUCE`, `SORT`, `SORT_BY`, `RANGE`, `ZIP`, `ENUMERATE`, `TAKE`, `DROP`, `FLAT`, `UNIQ`, `GROUP_BY`, `ANY`, `ALL`, `FIND`, `JOIN`.
- **`wlwl:std.json`** (§10.8): `STRINGIFY`, `PARSE`.
- **`wlwl:std.fs`** (§10.8): `WRITE_FILE`, `READ_FILE`, `EXISTS`.
- **`wlwl:std.test`** (§10.10): `TEST`, `ASSERT`, `ASSERT_EQ`, `ASSERT_NEQ`, `EXPECT_ERR`, `RUN_TESTS`.

## Build & runtime config — spec §9.1, §9.4

- `EXPORT([names])` at top level makes module bindings visible to importers.
- `wlwl.toml` (per-module, optional) declares `[features]`: `strict_types` (call-boundary type checks → `E0033`), `allow_builtin_shadow` (permit builtin-name shadowing with `W0030` instead of `E0025`).

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
| 12 | `YIELD()` as bare `IF` branch / LET RHS | `E0014` or mis-segment | Multi-statement block: `(YIELD(); v)` or `a; YIELD(); b` |
| 13 | Treating `NULL` from `TRY_RECV` as close | Infinite loop / lost close | Close is `IS_ERR` + `kind == "ChannelClosed"` |
| 14 | Expecting `CHANNEL_SEND` to block | `ERR(ChannelWouldBlock)` | `CHANNEL_TRY_SEND` / larger buf (v0.7.0) |
| 15 | `AWAIT` of cancelled task left unhandled | `E0102` at top level | `IS_ERR` / `UNWRAP_OR` / `ERR_PAYLOAD` the AWAIT result |
| 16 | Builtin name as first-class value (`LET(f, +)` / `LET(f, PRINT)`) | `E0020` undefined name | User-defined functions only are first-class (§5.4). Operator tokens (`+`/`-`/`==`) and `PRINT`/`LEN`/`MAP`/`FILTER`/etc. are **not** retrievable as values — write `FUN((a, b), +(a, b))` to get a callable. |
| 17 | `EXPECT_ERR(/(1, 0))` to catch integer / div-by-zero | Native code `E1003` (or `E0035` for overflow); `EXPECT_ERR` returns `ERR(E0049)` — not `OK(载荷)` | These are **native** codes per §11.2, not `RESULT`-shaped ERR; cannot be caught by any §8.3 consumer. Migration: write the program to avoid overflow / divide-by-zero (assert precondition or use `IF(==(b, 0), default, /(a, b))`). |
| 18 | `SUB(s, 7, 5)` returning `""` (or any pre-D8-010 expectation) | Pre-v0.8.1: third arg was end-index. | v0.8.1 third arg is **length**: `SUB("Hello, world", 7, 5)` → `"world"`. Migration: `SUB(s, start, end_old)` → `SUB(s, start, -(start, end_old))` or `SLICE(s, start, end_old)`. |
| 19 | `1.5e2` (or `1e-3` / `1E3`) as a float literal | Pre-v0.8.1: `E0011 expected ')', got Ident("e2")` | v0.8.1 supports `digits exponent` per §1.7 EBNF. Forms: `1e2`, `1.5e2`, `1.5e-2`, `1E3`, `2.5e+1`. Bare `1e` (no digits after) still raises `E0001`. |

## Verification loop

```bash
# 1. PRIMARY: does it run and produce expected output?
wlwl run path/to/file.wll

# 2. SECONDARY (best-effort): is the source already canonical?
wlwl fmt --check path/to/file.wll

# 3. For AI agents: machine-readable diagnostics.
wlwl run --format jsonl path/to/file.wll      # 12-field schema per conformance.rs
```

`wlwl run` is the source of truth. `wlwl fmt --check` has known idempotency drift — failures on syntactically valid source are formatter quirks, not bugs in your code. `--format jsonl` emits one structured diagnostic per line for AI-friendly error introspection.

## References

- **Authoritative spec**: `../docs/standard/wlwl-spec-v0.8.md` — defer to this on any disagreement (§17 = concurrency; v0.8 is clarification / alignment over v0.7).
- **v0.7 (archived)**: `../docs/history/wlwl-spec-v0.7.md` — §0–§17 core still valid (v0.7 is additive over v0.6; v0.8 is clarification over v0.7 with no breaking observable behavior).
- **v0.6 (archived)**: `../docs/history/wlwl-spec-v0.6.md` — §0–§12 core still valid (v0.6 was first versioned release).
- **Lookup tables** (operators, error codes, AST shapes, concurrency): `reference.md` in this folder.
- **Built-in registry (single source of truth)**: `../docs/appendix_G.md` (regenerated from `impl/crates/wlwl-eval/src/registry.rs`).
- **Spec-vs-impl register** (cite, do not load): `../docs/history/deviations-v0.8.md` — includes v0.8.0 (`D8-001`..`D8-008`) + v0.8.1 patch (`D8-009`..`D8-014`).
- **Third-party audit source** (cited by build plan; not normative): `../docs/history/audit-report-v0.8.1.md`.
- **Gold concurrency fixtures**: `../impl/tests/concurrency/*.wll`.
