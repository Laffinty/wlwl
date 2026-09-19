# Language tour

A whirlwind tour of WLWL v0.4 — by example. For the canonical
reference, see [docs/standard/wlwl-spec-v0.4.md](../standard/wlwl-spec-v0.4(SHA1_97524ced037b5ef0a5820a2ebd5bafb4ba4e239b).md).

## 1. Every form is a call

```wlwl
LET(add, FUN((a, b), +(a, b)));
PRINT(add(2, 3));     // 5
```

`+` is a regular function. `IF`, `WHILE`, `LET`, `FUN`, `PRINT` —
all of them are function calls. There are exactly two syntax forms:

- `LET(name, expr)` for assignment
- `(operator operand1 … operandN)` for everything else

Comments are `// line` or `/* block */`.

## 2. ERR is a value

```wlwl
LET(r, +(1, ERR("oops")));
PRINT(r);    // ERR("oops")
```

ERRs propagate transparently. `(+(1, ERR("oops")), -1)` would
evaluate as ERR, but `OR_DIE(+(1, ERR("oops")), -1)` extracts the
fallback `-1` when the chain ERRs out.

## 3. Cells + closures

```wlwl
LET(n, 0);
LET(step, FUN((), LET(n, +(n, 1))));
PRINT(step());   // 1
PRINT(step());   // 2
PRINT(step());   // 3
```

The closure shares the `LET(n, …)` cell (Phase A2 / ADR-0008). No
`REF` keyword; the cell is implicit.

## 4. Patterns in `LET` and `MATCH`

```wlwl
LET([x, y], [10, 20]);           // destructure into two cells
PRINT(MATCH(x,
    0, "zero",
    1, "one",
    _, "many"));                  // "many"
```

See [examples/match.wl](https://github.com/Laffinty/wlwl/blob/main/impl/examples/match.wl)
and [examples/destruct.wl](https://github.com/Laffinty/wlwl/blob/main/impl/examples/destruct.wl).

## 5. Stdlib

```wlwl
IMPORT("@std/json") AS json;
LET(obj, json::PARSE("{\"a\":1,\"b\":[2,3]}"));
PRINT(json::STRINGIFY(obj));     // {"a":1,"b":[2,3]}
```

Run it:

```bash
wlwl run examples/std_json.wl
```

## 6. ERR consumer + `EXPECT_ERR` testing

```wlwl
IMPORT("@std/test") AS test;
LET(div, FUN((n, d),
    IF(EQ(d, 0), ERR("div-by-zero"), /(n, d))));
PRINT(test::EXPECT_ERR(div(1, 0)));   // OK
PRINT(test::EXPECT_ERR(div(1, 1)));   // FAIL → E0049 "input not ERR"
```

See [the contributing guide on testing](contributing.md#testing)
for `ASSERT`, `EXPECT_EQ`, `ASSERT_EQ`, `ASSERT_NEQ`, `EXPECT_ERR`.

## 7. Modules and `wlwl.lock`

```toml
# wlwl.toml
[package]
name = "myproj"
version = "0.1.0"
language_version = "0.4"

[dependencies]
"wlwl:std-extra" = "0.4.1"
```

```bash
wlwl lock           # resolve + write wlwl.lock (Cargo-style, ADR-0011)
```

If two dependencies disagree about a transitive version, `wlwl`
emits `E0045` with the full backtrack tree in the diagnostic
`related` field.

## 8. AI tooling surface

Every diagnostic has a stable JSON shape (schema `1.1.0`):

```bash
wlwl run examples/bad.wl --format=json
```

```json
{
  "code": "E0020",
  "error_category": "name",
  "error_schema_version": "1.1.0",
  "message": "undefined name: foo",
  "location": { "file": "bad.wl", "line": 1, "col": 1, … },
  "suggestion_code": ["E0020"],
  "related": [],
  "retry_after": null,
  "retryable": false,
  "trace": null,
  "cause": null
}
```

`wlwl ast foo.wl --format=json` dumps the AST with stable node
IDs (SHA-256 over canonical-form source, ADR-0013).

## Next steps

- Read [Examples](examples.md) for idiomatic `.wl`
- Browse the [ADR catalog](adrs.md) for v0.4 design decisions
- Verify against the spec via [Spec & conformance](spec.md)
