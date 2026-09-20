# WLWL v0.6 Reference (on-demand)

> Loaded only when `SKILL.md` points here. This is a lookup catalogue,
> not a tutorial. Section numbers (`§1.5`, `§8.3`, `§10.4`, etc.) refer
> to `docs/standard/wlwl-spec-v0.6.md`.

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
- [§18. Notes on `interp.wll`](#18-notes-on-interpwll)

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

Everything else is truthy, including `OK(FALSE)`, non-empty strings, and non-empty containers whose elements are falsy (e.g. `[0]` is truthy because the array is non-empty).

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
| H | Integer overflow → `ERR(E0035)` | §B.8 | Was saturate + `W0015` in v0.5. Note: current impl surfaces as runtime diagnostic; check `docs/plan/deviations.md` |
| J | `${expr}` interpolation | §B.9 | One `StrStart/StrEnd` pair per literal regardless of segment count; nested string literal inside `${...}` is an error (`E0001`) |

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
Expr::Try { value }                          // TRY(x) — function-body only
Expr::Return { value: Option<Expr> }         // RETURN(expr?)
Expr::Break                                  // BREAK()
Expr::Continue                               // CONTINUE()
Expr::While { cond, body }
Expr::For { var, iter, body }
Expr::Match { scrutinee, clauses, default }
Expr::Ok { value }                           // OK(x)
Expr::Err { value }                          // ERR(x)
Expr::Panic { msg }                          // PANIC(msg)
Expr::Import { path, names }

// Pattern nodes (§7.2).
Pattern::Ident(Ident)
Pattern::Wildcard                            // _
Pattern::Literal(Literal)
Pattern::Array(Vec<Pattern>, Option<Ident>)  // [a, b, *rest]
Pattern::Dict(Vec<(Key, Pattern)>)
Pattern::VariantOk(Box<Pattern>)
Pattern::VariantErr(Box<Pattern>)

// Op tokens (§1.5, §4.3).
OpToken: "+" | "-" | "*" | "/" | "%" | "==" | "=" | "!=" | "<" | ">"
       | "<=" | ">=" | "&&" | "||" | "!"
```

## §4. Lexer traps (§1)

1. **`MUT` is a contextual keyword** (§1.4). Lexer always emits `Mut`; the parser consumes it only between `LET` and `(`. Elsewhere `MUT` is a regular identifier.
2. **Interpolated strings emit one `StrStart`/`StrEnd` pair per literal**, regardless of segment count. Bug fixed in commit `98e124a` (P5-V06-002) used to emit nested pairs.
3. **Nested block comments** (`/* outer /* inner */ still */`) are supported; unbalanced `/* */` is `E0011`.
4. **Comments are stripped** by the canonical formatter (per §A.3). `--check` ignores them on both sides.
5. **String escapes**: `\"`, `\\`, `\n`, `\t`, `\r`, `\/`, `\0`, `\b`, `\f`, `\$` (per §1.8). Unknown `\X` → `E0007`.
6. **Integer literal sign prefix** (§1.6): `[+|-]digits`. `-9223372036854775808` (= i64::MIN) is fine; `-(-9223372036854775808)` raises `E0034` (NEG of i64::MIN).
7. **Reserved tokens** (eval = `E0020`): `CLASS`, `NEW`, `THIS`, `AND`, `OR`. See §14.

## §5. Control flow (§6)

| Form | Spec | Notes |
|------|------|-------|
| `IF(cond, then[, else])` | §6.1 | `cond` consumer; missing `else` → `NULL` on false |
| `WHILE(cond, body)` | §6.2 | value `NULL`; body ERR propagates |
| `FOR(x, iter, body)` | §6.3 | iter = ARRAY/DICT/STRING; binding fresh per iter |
| `RETURN(expr?)` | §6.4 | inside function only; default `NULL` |
| `BREAK()` | §6.4 | innermost WHILE/FOR |
| `CONTINUE()` | §6.4 | next iteration of innermost WHILE/FOR |
| Any of `RETURN`/`BREAK`/`CONTINUE` outside function/loop | §6.4 | `E0014` |

Example:

```wlwl
LET MUT(n, 0);
LET MUT(out, []);
WHILE(<(n, 5),
  (
    SET(out, PUSH(out, *(n, n)));
    SET(n, +(n, 1))
  )
);
PRINT(out);                                   // [0, 1, 4, 9, 16]
```

## §6. Pattern matching (§7)

`MATCH(value, [[pat, body], ...][, default])`. First-hit-wins; clauses tried in source order; no match + no default → `NULL`.

| Pattern | Spec | Matches |
|---------|------|---------|
| `ident` | §7.2 | anything; binds `ident` |
| `_` | §7.2 | wildcard, no binding |
| `literal` | §7.2 | `==` match (§2.4) |
| `[p1, p2, *rest]` | §7.2 | array; positional; `*rest` binds tail |
| `["k": pat, ...]` | §7.2 | dict; partial by listed keys |
| `OK(p)` / `ERR(p)` | §7.2 | `RESULT` variants, then match payload |

Pattern mismatch is NOT an error; only an unhit. `LET([a, b], xs)` destructuring mismatch IS `E0026`.

## §7. Imports (§9.2, §9.3)

Grammar (EBNF, §A.2):

```
Import     = 'IMPORT' "(" string_lit "," name_array ")" .
NameItem   = string_lit | string_lit ":" string_lit .
```

Three forms:

```
IMPORT("./helpers", ["add", "sub"])          // plain
IMPORT("./helpers", ["add": "sum"])          // rename: bind `add` as local `sum`
IMPORT("./helpers", [])                      // side-effect only (load module)
```

Path resolution:
- `./` or `../` prefix → filesystem relative; `.wll` suffix optional.
- `wlwl:std.*` prefix → built-in namespace (`collection`, `json`, `fs`, `test`, `io`, `format`, `ai`, `agent`).

Errors:
- `E0040` module not found.
- `E0023` name not exported.
- `E0041` circular import.
- `E0021` duplicate name in same scope.

Members of `wlwl:std.*` namespaces are NOT global — they must be IMPORTed before use.

## §8. Error model (§8)

### §8.1 RESULT variants

- `OK(v)` — wraps `v` (any value) as a `RESULT` success.
- `ERR(e)` — wraps `e` as failure. `e` MUST be STRING or DICT (else `E0030`).
- `TYPE(OK(1))` and `TYPE(ERR("x"))` both return `"RESULT"`.

### §8.2 Transparent propagation

Any function call (built-in or user-defined) whose argument evaluates to `ERR` does NOT run its body; the `ERR` is forwarded unchanged. `&&`/`||` short-circuit: the unevaluated side's ERR does not propagate; the evaluated side's ERR does, per usual rules.

**Idiom**: extract payload before passing to non-consumer functions:

```wlwl
LET(r, risky());                             // r is ERR
LET(p, ERR_PAYLOAD(r));                      // p is plain value (consumer)
PRINT("failed: ${p}");                       // safe — non-ERR value
```

Do NOT pass `ERR(x)` to `PRINT`, `+`, `LEN`, `STR`, etc. — it propagates.

### §8.3 Consumer registry

These 13 are the ONLY constructions that can inspect an ERR without propagating it. Each accepts ERR as an argument and returns a plain value.

| Consumer | ERR behavior |
|----------|--------------|
| `IS_OK(x)` / `IS_ERR(x)` | return BOOLEAN |
| `UNWRAP_OR(x, d)` | ERR → `d`; OK(v) → `v` |
| `OR_DIE(x, d)` | deprecated alias of `UNWRAP_OR` (`W0051`) |
| `TRY(x)` | ERR → equivalent to `RETURN(err)`; OK(v) → `v`. Function-body only. |
| `UNWRAP(x)` | OK(v) → `v`; ERR → `E0100` PANIC |
| `ERR_PAYLOAD(x)` | ERR(e) → `e`; OK or non-RESULT → `E0030` |
| `WRAP(x, ctx)` | ERR(e) → `ERR(["original": e, "context": ctx])`; OK(v) → `v` |
| `TYPE(x)` | returns `"RESULT"` |
| `==(a,b)` / `!=(a,b)` | per §2.4; transparent ERR propagation rather than BOOL |
| `IF(cond, ...)` | cond ERR = falsy; with else → else; without → propagates |
| `&&(a,b)` / `\|\|(a,b)` | short-circuit; left ERR propagates, right ERR per §8.2 |
| `BOOL(x)` | returns BOOLEAN (FALSE for ERR); does NOT propagate |

Plus `EXPECT_ERR(x)` from `wlwl:std.test`: `x` is ERR → `OK(payload)`; else → `ERR(E0049)`.

### §8.4 PANIC

`PANIC(msg)` where `msg` MUST be STRING or DICT (else `E0030`). Immediately terminates with `E0100`. Does NOT participate in transparent propagation. `UNWRAP` on ERR and `NEG(i64::MIN)` also PANIC.

### §8.5 Top-level escape

An ERR that reaches the top level without being consumed raises `E0102` and exits with code 1. Always consume at the program boundary.

## §9. Standard library catalogue (§10)

### §10.2 I/O (global)

| Signature | Notes |
|-----------|-------|
| `PRINT(args...)` | stdout; args joined by single space (§10.2) |
| `PRINT_ERR(args...)` | stderr |
| `INPUT(prompt?)` | reads one line from stdin |

### §10.3 Type & conversion (global)

| Signature | Returns | Notes |
|-----------|---------|-------|
| `LEN(coll)` | INTEGER | array length, dict key count, string code-point count |
| `TYPE(x)` | STRING | type name; OK/ERR both report `"RESULT"` |
| `STR(x)` | STRING | display rendering per §2.5 |
| `INT(x)` | INTEGER | float truncates toward zero; string parses; failure → `ERR(["kind":"ParseError","input":s])` |
| `FLOAT(x)` | FLOAT | integer promotes; parse failures like `INT` |
| `BOOL(x)` | BOOLEAN | truthiness per §2.3; does NOT propagate ERR (§8.3) |
| `CALL(fn, args...)` | — | reserved form (§12); user closures → `E0030` |

### §10.4 Container ops (global)

All return NEW containers; receivers are immutable (§3.3).

| Signature | Returns | Purpose |
|-----------|---------|---------|
| `PUSH(arr, x)` | ARRAY | append `x` to tail |
| `UNSHIFT(arr, x)` | ARRAY | prepend `x` |
| `SHIFT(arr)` | ARRAY | drop first element |
| `AT_K(d, k, default)` | v | dict lookup or `default` (canonical name) |
| `POP(d, k, default)` | v | `AT_K` back-compat alias (deprecated hint) |
| `SLICE(arr, start, end?)` | ARRAY | half-open `[start, end)`; negatives count from end |
| `CONCAT(a, b)` | ARRAY | concatenate two arrays |
| `CONTAINS(coll, x)` | BOOLEAN | member test; strings: substring |
| `INDEX(arr, x)` | INTEGER | first occurrence index of `x`, or `-1` |
| `REVERSE(arr)` | ARRAY | reversed copy |
| `INDEX_GET(coll, k)` | v | subscript read (`a[i]`); OOB → `E0036` |
| `INDEX_SET(coll, k, v)` | ARRAY/DICT | subscript write (`a[i] = v`); forbidden on STRING → `E0030` |
| `AT(coll, k, default)` | v | safe subscript; OOB/missing → `default` |
| `KEYS(d)` | ARRAY | insertion order |
| `VALUES(d)` | ARRAY | insertion order |
| `HAS(d, k)` | BOOLEAN | key existence |
| `MERGE(a, b)` | DICT | `b` overrides `a` |
| `REMOVE_KEY(d, k)` | DICT | remove key; missing → returns `d` unchanged |
| `DEL(d, k)` | DICT | `REMOVE_KEY` deprecated alias (`W0051`) |

### §10.5 String ops (global)

| Signature | Notes |
|-----------|-------|
| `SUB(s, start, len?)` | by code point; default `len` to end |
| `SPLIT(s, sep)` | array of parts |
| `REPLACE(s, old, new)` | global replace |
| `UPPER(s)` / `LOWER(s)` | ASCII only; non-ASCII may emit `W0014` |
| `TRIM(s)` / `TRIM_START(s)` / `TRIM_END(s)` | whitespace |
| `STARTS_WITH(s, pre)` / `ENDS_WITH(s, suf)` | prefix/suffix |
| `REPEAT(s, n)` | repeat n times |
| `PAD_START(s, n, c)` / `PAD_END(s, n, c)` | single-char pad |
| `CODEPOINTS(s)` / `FROM_CODEPOINTS(arr)` | int array ↔ STRING |

### `wlwl:std.collection` (§10.6) — IMPORT required

`MAP`, `FILTER`, `REDUCE`, `SORT`, `SORT_BY`, `RANGE` (step=0 → `E0038`), `ZIP`, `ENUMERATE`, `TAKE`, `DROP`, `FLAT`, `UNIQ`, `GROUP_BY`, `ANY`, `ALL`, `FIND`, `JOIN`. Callback `ERR` propagates whole call.

### `wlwl:std.json` (§10.8) — IMPORT required

| Signature | Notes |
|-----------|-------|
| `STRINGIFY(x)` | serialize; dict keys sorted |
| `PARSE(s)` | parse JSON; object order preserved |

### `wlwl:std.fs` (§10.8) — IMPORT required

| Signature | Notes |
|-----------|-------|
| `WRITE_FILE(path, text)` | UTF-8; parent dir must exist |
| `READ_FILE(path)` | missing file → `E0061` |
| `EXISTS(path)` | BOOLEAN |

### `wlwl:std.test` (§10.10) — IMPORT required

| Signature | Notes |
|-----------|-------|
| `TEST(name, body)` | registers a zero-arg body |
| `ASSERT(cond, msg?)` | `TRUE` → `OK(TRUE)`; else `ERR(E0046)` |
| `ASSERT_EQ(a, b)` | failure → `ERR(E0047)` |
| `ASSERT_NEQ(a, b)` | failure → `ERR(E0048)` |
| `EXPECT_ERR(x)` | ERR → `OK(payload)`; else `ERR(E0049)` |
| `RUN_TESTS()` | ARRAY of `["name":..., "passed":..., "duration_ms":..., "return_value"|"error":...]` |

### `wlwl:std.format` (§10.7) — IMPORT required

Same `FORMAT` as the global. `FORMAT(template, args...) → STRING`. `{N}` placeholders use `STR(args[N])`; `{name}` looks up `args[0]["name"]`. Unmatched `{...}` left literal. Isolated `{` → `E0039`.

## §10. Error codes (§11.2, §11.3)

### Errors (E-codes)

| Code | Meaning | Common cause |
|------|---------|--------------|
| `E0001`–`E0003` | illegal char / unterminated string / unterminated block comment | lexer |
| `E0010`–`E0013` | syntax errors (expected expr/`)`/`,`/`;`) | parser |
| `E0014` | `RETURN`/`BREAK`/`CONTINUE` outside function/loop | §6.4 |
| `E0020` | undefined name; call of non-function; evaluating reserved keyword | §1.4, §12 |
| `E0021` | duplicate import name in one scope | §9.2 |
| `E0022` | function arity mismatch | §5.3 |
| `E0023` | name not exported by module | §9.2 |
| `E0024` | `SET` on immutable binding | §3.4 |
| `E0025` | shadowing builtin (without `allow_builtin_shadow`) | §3.5 |
| `E0026` | destructuring pattern mismatch | §3.1, §7.3 |
| `E0030` | type error (incl. misuse of `ERR_PAYLOAD`/`WRAP`/`CALL`, indexing STRING for write) | §8.3 |
| `E0031` | wrong subscript/key type; NaN as key | §2.1 |
| `E0034` | `NEG` of `INTEGER` lower bound | §2.2 |
| `E0035` | numeric overflow (INT ops, FLOAT→INT overflow) | §2.2 |
| `E0036` | array/string index out of range | §4.5 |
| `E0037` | dict key not present (subscript, not `AT_K`) | §4.5 |
| `E0038` | `RANGE` step zero | §10.6 |
| `E0039` | malformed `FORMAT` template | §10.7 |
| `E0040` / `E0041` | module missing / circular import | §9.2 |
| `E0046`–`E0049` | `ASSERT` / `ASSERT_EQ` / `ASSERT_NEQ` / `EXPECT_ERR` failures | §10.10 |
| `E0061` | file not found | §10.8 |
| `E0070` / `E0071` | JSON parse / stringify failure | §10.8 |
| `E0100` | PANIC (`UNWRAP` on ERR, `NEG` i64::MIN, explicit `PANIC`) | §8.4 |
| `E0102` | top-level ERR escape | §8.5 |
| `E1003` | divide-by-zero / modulo-by-zero (both INT and FLOAT) | §2.2 |

### Warnings (W-codes)

| Code | Meaning |
|------|---------|
| `W0010` | unused `LET` binding |
| `W0011` | unused function parameter (prefix `_` is silent) |
| `W0014` | non-ASCII case-conversion behavior unspecified |
| `W0020` | mixed bare/key-value items in container literal |
| `W0030` | shadowing reserved keyword OR permitted builtin shadow |
| `W0040` | unhandled `TODO(agent):` in comment |
| `W0051` | deprecated alias used (`OR_DIE`, `DEL`, `POP`) |
| `W0053` | source deviates from formatter output (§A.3) |

### Exit codes (§11.4)

| Code | Meaning |
|------|---------|
| `0` | success |
| `1` | terminated by diagnostic (syntax error, `E0100`, `E0102`, etc.) |
| `101` | implementation internal crash (non-normative) |

## §11. Display / STR quirks (§2.5)

| Type | Renders as |
|------|------------|
| BOOLEAN | `TRUE` / `FALSE` |
| NULL | `NULL` |
| INTEGER | decimal digits (no thousands separator) |
| FLOAT | preserves decimal point — **`STR(FLOAT(2))` is `"2.0"`, not `"2"`** |
| STRING | identity |
| ARRAY | `[e1, e2, ...]` (empty `[]` is ambiguous with empty dict) |
| DICT | `[k: v, ...]` (insertion order) |
| FUNCTION | `<fun(params)>` (params verbatim) |
| RESULT | both variants render as `OK(v)` / `ERR(e)`; `TYPE` reports `"RESULT"` |

`PRINT(args...)` joins the rendered args with a single space.

## §12. INDEX vs `xs[i]` (§10.4, §4.5)

These look similar but differ on the not-found case:

| Form | Not found | OOB |
|------|-----------|-----|
| `INDEX(arr, x)` | returns `-1` (NOT an error) | n/a |
| `arr[i]` | `E0037` (dict) or `E0036` (array/string) | `E0036` |

Use `INDEX` when "not found" is a normal path. Use `arr[i]` when OOB is a programming bug you want to surface.

## §13. Container immutability (§3.3, §10.4)

All container operations are non-destructive — they return new containers. The receiver is unchanged. To "update" a binding, you must:

1. Bind mutable: `LET MUT(arr, [1, 2, 3]);`
2. Reassign: `SET(arr, PUSH(arr, 4));`

There is no `arr.push(x)` in-place form. `SET` on a plain `LET` is `E0024`.

## §14. Reserved forms (§12)

These evaluate as `E0020` (or `E0030` for `CALL`); programs must not rely on them:

| Form | Status |
|------|--------|
| `CLASS` / `NEW` / `THIS` | reserved OOP (§12) |
| `MODULE(name?, body)` | reserved module declaration |
| `MODULE_REF(path)` | reserved dynamic module fetch |
| `CALL(fn, args...)` | reserved; user closures → `E0030` |
| `ARRAY(items...)` | reserved non-empty array ctor — use `[1, 2, 3]` literals |
| `AND` / `OR` | reserved legacy — use `&&` / `\|\|` |

## §15. `--format json|jsonl` diagnostics (§11.1)

```
wlwl run --format json  path/to/file.wll      # single JSON object per v0.3 §14.3
wlwl run --format jsonl path/to/file.wll      # one diagnostic per line per v0.3 §14.7
```

JSONL schema (12 fields) per `impl/crates/wlwl-cli/tests/conformance.rs:95–108`:

```
code, error_category, error_schema_version, message, location, severity,
retryable, retry_after, related, suggestion_code, idempotent, trace
```

Use JSONL for AI-friendly error introspection: stream-parse one diagnostic per line.

## §16. `EXPORT` / `wlwl.toml` features (§9.1, §9.4)

`EXPORT(name_array)` at top level makes module bindings visible to importers. The grammar is in §A.2; un-exported bindings are invisible outside the module.

`wlwl.toml` (per-module, optional):

| Feature | Default | Effect |
|---------|---------|--------|
| `strict_types` | `false` | enable call-boundary type-annotation checks (`E0033` on mismatch) |
| `allow_builtin_shadow` | `false` | permit builtin-name shadowing with `W0030` instead of `E0025` |

Other manifest fields (dependencies, version solving, lock files) are the package manager's concern and are not defined by the language spec.

## §17. Anti-patterns (extended)

Beyond the top-10 in `SKILL.md`:

- **Out-of-range array index returns `NIL`** — FALSE; it raises `E0036` (§4.5). Use `AT(xs, i, default)` if you want a safe-default read.
- **`IS_NIL(x)` exists** — FALSE; the literal is `NULL`. Use `==(x, NULL)` or `BOOL(==(x, NULL))` (§1.9, §2.1).
- **`APPEND(arr, x)` is a builtin** — FALSE; the global is `PUSH` (§10.4).
- **`E0009` is divide-by-zero** — FALSE; it's `E1003` (§2.2).
- **`LIST(1, 2, 3)` is the literal** — FALSE; use `[1, 2, 3]` literals (spec §4.8); `LIST()` is not a v0.6 builtin.
- **`FORMAT("…{0}…", [x])` unpacks `x` from the list** — FALSE; `FORMAT(template, args...)` is variadic positional (§10.7). `FORMAT("…{0}…", [x])` renders the whole list `[x]` at `{0}`. Use `FORMAT("…{0}…", x)` (variadic) or `"…${x}…"` (interpolation).
- **`POP(d, k, default)` is the canonical dict lookup** — DEPRECATED; `AT_K(d, k, default)` is canonical (§B.5); `POP` still works as a back-compat alias and surfaces a deprecation hint.
- **Passing `ERR(x)` to a non-consumer function lets the function decide what to do** — FALSE; §8.2 transparently forwards ERR. Always consume first.

## §18. Notes on `interp.wll`

`interp.wll` is the in-skill gold-standard example. Each block (A through N) exits 0 and prints documented output. Block (H) currently demonstrates the overflow-generation path via a `safe_add` helper that bounds-checks operands before `+`; the actual `+(i64::MAX, 1)` would terminate with `E0035` + exit 1 in the current implementation (the spec says it should produce `ERR(E0035)` consumable via `UNWRAP_OR` — see `docs/plan/deviations.md`). Blocks (K), (L), (M), (N) cover `RETURN`, `MATCH`, `IMPORT`, and the `["orig": "alias"]` rename form, which the prior version did not exercise.
