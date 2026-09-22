---
name: writing-wlwl
description: "Writes WLWL v0.7 .wll source files (spec docs/standard/wlwl-spec-v0.7.md). Covers the 9 v0.6 decisions (truthy overhaul, &&/|| short-circuit, IF ERR-routing, ! canonical, AT_K rename, string subscript, LET MUT, overflow->E0035, ${} interpolation) plus v0.7 §17 concurrency (SCOPE/SPAWN/AWAIT/YIELD/TASK_*/SHIELD/CHANNEL_*). Use when the user asks for WLWL code, a .wll file, a wlwl script, or anything targeting wlwl-spec-v0.7 (or v0.6 core). Do NOT use for v0.5 or earlier (POP-not-AT_K, no LET MUT), the Rust implementation, or the formatter."
---

# Writing WLWL (v0.7)

## When to load / NOT to use

**Use when:**
- The user asks for a `.wll` source file, a WLWL program, or a v0.6/v0.7 idiom.
- A task targets `wlwl-spec-v0.7.md` (current) or `wlwl-spec-v0.6.md` (archived).
- Reviewing or debugging v0.6 / v0.7 source (concurrency included).

**Don't use when:**
- The source targets WLWL v0.5 or earlier (different truthy rules, `POP`-not-`AT_K`, no `LET MUT`).
- The task is editing the Rust implementation in `impl/` or the formatter spec.
- The user wants `wlwl.exe` CLI documentation (that's `wlwl help` / `wlwl <subcmd> --help`).
- The artifact is a Rust test, ADRs, or build-system file.

## Writing flow

Tick these as you go:

```
WLWL writing progress:
- [ ] 1. Sketch the AST shape (use the operators/builtins section below)
- [ ] 2. Decide mutation — any SET later means LET MUT(name, value) now
- [ ] 3. Concurrency? wrap SPAWN in SCOPE; YIELD only in multi-statement blocks
- [ ] 4. Write the .wll file (one stmt per line, semicolons, no leading indent)
- [ ] 5. Run `wlwl run <file>` — MUST exit 0 with expected stdout
- [ ] 6. If 5 fails: consult reference.md for the failing token/operator
```

Do not skip step 5. `wlwl run` is the source of truth. `wlwl fmt --check` is best-effort (see Verification loop).

## Truthy / falsy — spec §2.3

Eight falsy values: `FALSE`, `NULL`, `0`, `0.0`, `""`, `[]` (empty array), `DICT()` (empty dict), `NaN`.

**Everything else is truthy**, including non-empty strings, non-empty containers, `OK(FALSE)`, and v0.7 `TASK`/`CHANNEL` handles.

`ERR(...)` is **not** in this list — it is its own propagation mechanism per §8.2. Passing `ERR(x)` to a non-consumer function transparently forwards the `ERR`; it does NOT make the function body "skip" because the arg was falsy. Only §8.3 consumers (`IS_OK`, `IS_ERR`, `UNWRAP_OR`, `OR_DIE`, `TRY`, `UNWRAP`, `ERR_PAYLOAD`, `WRAP`, `TYPE`, `==`/`!=`, `IF` condition, `&&`/`||`, `BOOL`) can inspect an ERR safely.

## Operators / builtins — the v0.6 cheat sheet

**Equality** (spec §2.4, §B.12): `=(a, b)` and `==(a, b)` are aliases. `!=(a, b)` or `<>(a, b)`. Cross-type numeric: `==(1, 1.0)` is `TRUE`. Function/handle identity: `==(f, f)` and `==(h, h)` are `TRUE` (v0.7 locks handle identity).

**Comparison**: `<(a, b)`, `>(a, b)`, `<=(a, b)`, `>=(a, b)`.

**Arithmetic**: `+(a, b)`, `-(a, b)`, `*(a, b)`, `/(a, b)`, `%(a, b)`. `+` is also string concat and array concat. Integer overflow → `ERR(E0035)`; divide-by-zero → `ERR(E1003)` (note: not `E0009`).

**Boolean**: `&&(a, b)` / `||(a, b)` short-circuit; right side not evaluated when left decides. `NOT(x)` and `!(x)` are equivalent.

**Indexing**: `xs[i]` returns one element; out-of-range array/string index raises `E0036`. `INDEX(xs, v)` returns the index of `v` in `xs`, or `-1` if not found (NOT an error).

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

## Concurrency — spec §17 (v0.7)

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
2. **`YIELD` placement.** Only as a direct child of a multi-statement block:

   ```wlwl
   FUN(() , a; YIELD(); b)     // OK
   IF(TRUE, YIELD(), 1)        // E0014 — wrap: IF(TRUE, (YIELD(); 1), 1)
   LET(x, YIELD())             // E0014
   ```

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

**§8.3 ERR consumers** (13 entries): `IS_OK`, `IS_ERR`, `UNWRAP_OR`, `OR_DIE` (deprecated — `W0051`), `TRY` (function-body only), `UNWRAP` (PANICs on ERR — `E0100`), `ERR_PAYLOAD`, `WRAP`, `TYPE`, `==`/`!=`, `IF` (condition position), `&&`/`||` (left side), `BOOL`. Each either extracts payload info or returns a default.

**§8.4 PANIC**: `PANIC(msg)` (msg must be STRING or DICT) terminates with `E0100`. Bypasses propagation entirely. `UNWRAP` on ERR and `NEG(i64::MIN)` also PANIC.

**§8.5 top-level escape**: an uncaught ERR reaching the top level terminates with `E0102` and exit code 1. Always consume at the boundary.

**v0.7 concurrent ERR kinds** (§8.1): `ChannelClosed` · `ChannelWouldBlock` · `Cancelled` (always dict payloads).

**v0.7 concurrent diagnostics**: `E0052`–`E0058` (see reference.md).

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

- **Authoritative spec**: `../docs/standard/wlwl-spec-v0.7.md` — defer to this on any disagreement (§17 = concurrency).
- **v0.6 (archived)**: `../docs/history/wlwl-spec-v0.6.md` — §0–§12 core still valid (v0.7 is additive).
- **Lookup tables** (operators, error codes, AST shapes, concurrency): `reference.md` in this folder.
- **Spec-vs-impl register** (cite, do not load): `../docs/history/deviations-v0.7.md`.
- **Gold concurrency fixtures**: `../impl/tests/concurrency/*.wll`.
