---
name: writing-wlwl
description: Writes WLWL v0.6 .wll source files (spec docs/standard/wlwl-spec-v0.6.md). Covers the 9 v0.6 decisions: truthy overhaul, &&/|| short-circuit, IF ERR-routing, ! canonical, AT_K rename, string subscript, LET MUT, overflow->E0035, ${} interpolation. Use when the user asks for WLWL code, a .wll file, a wlwl script, or anything targeting wlwl-spec-v0.6. Do NOT use for v0.5 or earlier (POP-not-AT_K, no LET MUT), the Rust implementation, or the formatter.
---

# Writing WLWL v0.6

## When to load / NOT to use

**Use when:**
- The user asks for a `.wll` source file, a WLWL program, or a v0.6 idiom.
- A task targets `wlwl-spec-v0.6.md` (SHA-1 anchored at `docs/standard/`).
- Reviewing or debugging v0.6 source.

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
- [ ] 3. Write the .wll file (one stmt per line, semicolons, no leading indent)
- [ ] 4. Run `wlwl run <file>` — MUST exit 0 with expected stdout
- [ ] 5. If 4 fails: consult reference.md for the failing token/operator
```

Do not skip step 4. `wlwl run` is the source of truth. `wlwl fmt --check` is best-effort in v0.6 (see Verification loop).

## Truthy / falsy — spec §2.3

Eight falsy values: `FALSE`, `NULL`, `0`, `0.0`, `""`, `[]` (empty array), `DICT()` (empty dict), `NaN`.

**Everything else is truthy**, including non-empty strings, non-empty containers, and `OK(FALSE)`.

`ERR(...)` is **not** in this list — it is its own propagation mechanism per §8.2. Passing `ERR(x)` to a non-consumer function transparently forwards the `ERR`; it does NOT make the function body "skip" because the arg was falsy. Only §8.3 consumers (`IS_OK`, `IS_ERR`, `UNWRAP_OR`, `OR_DIE`, `TRY`, `UNWRAP`, `ERR_PAYLOAD`, `WRAP`, `TYPE`, `==`/`!=`, `IF` condition, `&&`/`||`, `BOOL`) can inspect an ERR safely.

## Operators / builtins — the v0.6 cheat sheet

**Equality** (spec §2.4, §B.12): `=(a, b)` and `==(a, b)` are aliases. `!=(a, b)` or `<>(a, b)`. Cross-type numeric: `==(1, 1.0)` is `TRUE`.

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

## Error model — spec §8

**§8.2 transparent propagation**: any function call whose argument evaluates to `ERR` does not run the body; the `ERR` is forwarded. Pass plain values to non-consumer functions; consume `ERR` only via §8.3.

**§8.3 ERR consumers** (13 entries): `IS_OK`, `IS_ERR`, `UNWRAP_OR`, `OR_DIE` (deprecated — `W0051`), `TRY` (function-body only), `UNWRAP` (PANICs on ERR — `E0100`), `ERR_PAYLOAD`, `WRAP`, `TYPE`, `==`/`!=`, `IF` (condition position), `&&`/`||` (left side), `BOOL`. Each either extracts payload info or returns a default.

**§8.4 PANIC**: `PANIC(msg)` (msg must be STRING or DICT) terminates with `E0100`. Bypasses propagation entirely. `UNWRAP` on ERR and `NEG(i64::MIN)` also PANIC.

**§8.5 top-level escape**: an uncaught ERR reaching the top level terminates with `E0102` and exit code 1. Always consume at the boundary.

## Standard library pointers

For the full ~70-name catalogue see `reference.md` §9. Categories:

- **Global builtins (§10.2–10.5)**: `PRINT`, `PRINT_ERR`, `INPUT`, `LEN`, `TYPE`, `STR`, `INT`, `FLOAT`, `BOOL`, container ops (`PUSH`/`SHIFT`/`SLICE`/`CONCAT`/`INDEX`/`REVERSE`/`INDEX_GET`/`INDEX_SET`/`AT`/`KEYS`/`VALUES`/`HAS`/`MERGE`/`REMOVE_KEY`/`DEL`), string ops (`SUB`/`SPLIT`/`REPLACE`/`UPPER`/`LOWER`/`TRIM`/`STARTS_WITH`/`ENDS_WITH`/`REPEAT`/`PAD_*`/`CODEPOINTS`/`FROM_CODEPOINTS`), and `AT_K`/`POP` for dict lookup.
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

## Verification loop

```bash
# 1. PRIMARY: does it run and produce expected output?
wlwl run path/to/file.wll

# 2. SECONDARY (best-effort): is the source already canonical?
wlwl fmt --check path/to/file.wll

# 3. For AI agents: machine-readable diagnostics.
wlwl run --format jsonl path/to/file.wll      # 12-field schema per conformance.rs
```

`wlwl run` is the source of truth. `wlwl fmt --check` has known idempotency drift in v0.6 — failures on syntactically valid source are formatter quirks, not bugs in your code. `--format jsonl` emits one structured diagnostic per line for AI-friendly error introspection.

## References

- **Authoritative spec**: `../docs/standard/wlwl-spec-v0.6.md` — defer to this on any disagreement.
- **Lookup tables** (operators, error codes, AST shapes): `reference.md` in this folder.
- **Feature showcase**: `interp.wll` runs 13 blocks (A–N) and exits 0; diff your program against it.
- **Single-purpose examples**: `examples/truthiness.wll`, `error_propagation.wll`, `match.wll`, `control_flow.wll`, `import_stdlib.wll`, `interpolation.wll`.
- **Spec-vs-impl register** (cite, do not load — 211 KB): `../docs/plan/deviations.md`.
- **Impl-side examples** (do not edit; cite as "see also"): `../impl/examples/interp.wll`, `match.wll`, `destruct.wll`, `format.wll`, `closure_cell.wll`, `phase2_demo.wll`.
- **Local idioms tour** (mkdocs source, not deployed): `../docs/site/tour.md`.
