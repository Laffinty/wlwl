# Language tour

A whirlwind tour of WLWL — by example. For the canonical
reference, see
[docs/standard/wlwl-spec-v0.7.md](../standard/wlwl-spec-v0.7.md)
(v0.6 archived at [`docs/history/wlwl-spec-v0.6.md`](../history/wlwl-spec-v0.6.md)).

## 1. Every form is a call

```wlwl
LET(add, FUN((a, b), +(a, b)));
PRINT(add(2, 3));     // 5
```

`+` is a regular function. `IF`, `WHILE`, `LET`, `FUN`, `PRINT` —
all of them are function calls. There are exactly two syntax forms:

- `LET(name, expr)` for immutable binding
- `LET MUT(name, expr)` for mutable binding (required for `SET`)
- `(operator operand1 … operandN)` for everything else

Comments are `// line` or `/* block */`.

## 2. ERR is a value

```wlwl
LET(r, +(1, ERR("oops")));
PRINT(r);    // ERR("oops")
```

ERRs propagate transparently. `+(1, ERR("oops"))` evaluates as ERR,
but `UNWRAP_OR(+(1, ERR("oops")), -1)` extracts the fallback `-1`
when the chain ERRs out. (v0.6 §8.3: `UNWRAP_OR` and `BOOL` are ERR
consumers.)

`IF(ERR("oops"), then, else)` routes to the `else` branch (or
propagates `ERR` if there is no else). `BOOL(ERR("x"))` returns
`TRUE` without triggering propagation.

## 3. Mutable cells + closures

```wlwl
LET MUT(n, 0);
LET(step, FUN((), (SET(n, +(n, 1)); n)));
PRINT(step());   // 1
PRINT(step());   // 2
PRINT(step());   // 3
```

The closure shares the `LET MUT(n, …)` cell. v0.6 §3.1 made
mutability explicit: `SET` on a plain `LET` (immutable) binding
raises `E0024`. There is no longer a "first closure call upgrades
the cell" magic.

## 4. Patterns in `LET` and `MATCH`

```wlwl
LET([x, y], [10, 20]);           // destructure into two cells
PRINT(MATCH(x,
    0, "zero",
    1, "one",
    _, "many"));                  // "many"
```

See [examples/match.wll](https://github.com/Laffinty/wlwl/blob/main/impl/examples/match.wll)
and [examples/destruct.wll](https://github.com/Laffinty/wlwl/blob/main/impl/examples/destruct.wll).

## 5. Short-circuit `&&` / `||`

```wlwl
LET(count, 0);
LET(touch, FUN((), (SET(count, +(count, 1)); TRUE)));
PRINT(IF(&&(FALSE, touch()), "no-touch", "touched"));  // "no-touch"; count is still 0
PRINT(IF(||(TRUE, touch()),  "no-touch", "touched"));  // "no-touch"; count is still 0
PRINT(count);  // 0
```

v0.6 §4.3 made `&&` / `||` short-circuit. The decisive side's
result decides the answer; the other side is not evaluated.

## 6. String interpolation

```wlwl
LET(name, "WLWL");
LET(version, 6);
PRINT("Hello, ${name} v${version}!");   // Hello, WLWL v6!
PRINT("escaped \${literal}");            // escaped ${literal}
```

`"${expr}"` segments are evaluated and `STR`-rendered. An `ERR`
inside `${...}` propagates without producing partial output.

## 7. String subscript

```wlwl
LET(s, "Hello");
PRINT(s[0]);    // "H"
PRINT(s[-1]);   // "o"
```

v0.6 §4.5 added `s[i]` (single-codepoint string). Writes
(`s[i] = "x"`) are rejected with `E0030`; use `SUB` / `SPLIT`
/ `CODEPOINTS` for string manipulation.

## 8. Truthiness

```wlwl
IF(0, "truthy", "falsy");       // "falsy"
IF("", "truthy", "falsy");      // "falsy"
IF([], "truthy", "falsy");      // "falsy"
IF(NULL, "truthy", "falsy");    // "falsy"
IF(42, "truthy", "falsy");      // "truthy"
IF("x", "truthy", "falsy");     // "truthy"
```

v0.6 §2.3: eight values are falsy — `FALSE`, `NULL`, `0`, `0.0`,
`""`, empty ARRAY, empty DICT, `NaN`. Everything else is truthy.

## 9. Stdlib

```wlwl
IMPORT("wlwl:std.json", ["PARSE", "STRINGIFY"]);
LET(obj, PARSE("{\"a\":1,\"b\":[2,3]}"));
PRINT(STRINGIFY(obj));     // {"a":1,"b":[2,3]}
```

Run it:

```bash
wlwl run examples/showcase.wll
```

## 10. ERR consumer + `EXPECT_ERR` testing

```wlwl
IMPORT("wlwl:std.test", ["ASSERT", "EXPECT_ERR"]);
LET(div, FUN((n, d),
    IF(==(d, 0), ERR("div-by-zero"), /(n, d))));
PRINT(EXPECT_ERR(div(1, 0)));   // OK
PRINT(EXPECT_ERR(div(1, 1)));   // FAIL → E0049 "input not ERR"
```

See [the contributing guide on testing](contributing.md#testing)
for `ASSERT`, `EXPECT_EQ`, `ASSERT_EQ`, `ASSERT_NEQ`, `EXPECT_ERR`.

## 11. Modules and `wlwl.lock`

```toml
# wlwl.toml
[package]
name = "myproj"
version = "0.1.0"
language_version = "0.6"

[dependencies]
"wlwl:std-extra" = "0.6.0"
```

```bash
wlwl lock           # resolve + write wlwl.lock (Cargo-style, ADR-0011)
```

If two dependencies disagree about a transitive version, `wlwl`
emits `E0045` with the full backtrack tree in the diagnostic
`related` field.

## 12. AI tooling surface

Every diagnostic has a stable JSON shape (schema `1.1.0`):

```bash
wlwl run examples/bad.wll --format=json
```

```json
{
  "code": "E0020",
  "error_category": "name",
  "error_schema_version": "1.1.0",
  "message": "undefined name: foo",
  "location": { "file": "bad.wll", "line": 1, "col": 1, … },
  "suggestion_code": ["E0020"],
  "related": [],
  "retry_after": null,
  "retryable": false,
  "trace": null,
  "cause": null
}
```

`wlwl ast foo.wll --format=json` dumps the AST with stable node
IDs (SHA-256 over canonical-form source, ADR-0013).

## Next steps

- Read [Examples](examples.md) for idiomatic `.wll`
- Browse the [ADR catalog](adrs.md) for design decisions
- Verify against the spec via [Spec & conformance](spec.md)
