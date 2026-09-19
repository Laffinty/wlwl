# Examples

Ten `.wl` programs live in [`impl/examples/`](https://github.com/Laffinty/wlwl/tree/main/impl/examples).
Each demonstrates a single Phase G12 mini-pattern.

## Quick reference

| File | Demonstrates | Spec |
|---|---|---|
| [`hello.wl`](https://github.com/Laffinty/wlwl/blob/main/impl/examples/hello.wl) | `LET` / `FUN` / `PRINT` | §2 |
| [`math.wl`](https://github.com/Laffinty/wlwl/blob/main/impl/examples/math.wl) | numeric and `INT`/`FLOAT` conversions | §9.5 |
| [`phase2_demo.wl`](https://github.com/Laffinty/wlwl/blob/main/impl/examples/phase2_demo.wl) | Phase 2 surface — `LET`, `IF`, `WHILE` | Phase 2 baseline |
| [`showcase.wl`](https://github.com/Laffinty/wlwl/blob/main/impl/examples/showcase.wl) | stdlib (json, io, fs, format) | §15 |
| [`test_array.wl`](https://github.com/Laffinty/wlwl/blob/main/impl/examples/test_array.wl) | array indexing | §10.1 |
| [`match.wl`](https://github.com/Laffinty/wlwl/blob/main/impl/examples/match.wl) | `MATCH` arms + default | §7.6 |
| [`destruct.wl`](https://github.com/Laffinty/wlwl/blob/main/impl/examples/destruct.wl) | array / dict pattern bindings | §7.5 |
| [`std_test.wl`](https://github.com/Laffinty/wlwl/blob/main/impl/examples/std_test.wl) | `RUN_TESTS` + `ASSERT` / `EXPECT_EQ` / `EXPECT_ERR` | §15.9 |
| [`closure_cell.wl`](https://github.com/Laffinty/wlwl/blob/main/impl/examples/closure_cell.wl) | closures capturing `LET` cells | §6.4 |
| [`format.wl`](https://github.com/Laffinty/wlwl/blob/main/impl/examples/format.wl) | FORMAT templates + `:fmt` specifiers | §10.6 |

## Try them all

```bash
for f in impl/examples/*.wl; do
  echo "=== $f ==="
  wlwl run "$f" 2>&1 | head -10
done
```

Or as a smoke test in CI:

```yaml
- name: run examples
  working-directory: impl
  run: |
    for f in examples/*.wl; do
      cargo run -q --release --bin wlwl -- run "$f" --format=json
    done
```

## Adding your own

The canonical formatter will re-format your program
automatically, so don't worry about whitespace:

```bash
wlwl fmt examples/my_new.wl
```

See [examples/CLAUDE.md](https://github.com/Laffinty/wlwl/blob/main/impl/examples/CLAUDE.md)
(if present) for AI-generated walkthrough notes; otherwise the
[Contributing guide](contributing.md) covers the same workflow.
