# WLWL

**A small experimental programming language in which every syntactic form is a
function call.**

WLWL has no statement syntax to learn. Binding, branching, iteration, pattern
matching, modules, classes and concurrency are all ordinary function calls with
ordinary signatures, so the whole language is described by a single grammar and
a single builtin table. The standard library is layered the same way: each
namespace is imported by name, and every member's contract is written down in
the standard-library specification.

## Highlights

- **One syntax for everything.** `LET`, `SET`, `IF`, `WHILE`, `FOR`, `MATCH`,
  `FUN` and `IMPORT` are function calls, not keywords with a grammar of their
  own.
- **Errors are values.** A failing call yields an `ERR` value that propagates
  through expressions; `OR DIE` turns it into a diagnostic carrying a stable
  error code, and `RESULT` / `OPTION` carry recoverable failures.
- **Closures capture cells.** `LET MUT` creates a mutable cell; closures that
  capture it share it, which is how state is threaded through callbacks.
- **Concurrency lives in the language.** `SCOPE`, `SPAWN`, `AWAIT`, `YIELD` and
  the `CHANNEL_*` family are language constructs, not a library convention.
- **Modules and classes.** `IMPORT` / `EXPORT` split a program into units;
  `CLASS` / `NEW` / `CALL_METHOD` give those units state and behavior.
- **A standard library with contracts.** I/O, files, JSON, regex, time,
  randomness, collections, hashing and output safety — each member's behavior
  is specified, and contract tests freeze the observable results.
- **A toolchain, not just a compiler.** `wlwl run`, `check`, `ast`, `fmt`,
  `sig`, `sig-gen`, `interface`, `schema` and `lsp`.

## Quick start

```wlwl
IMPORT("wlwl:std.io", ["PRINT"]);

// A closure captures a mutable cell; the cell is shared by every call.
LET MUT(total, 0);
LET(accumulate, FUN((amount), (
    SET(total, +(total, amount));
    total
)));

PRINT(accumulate(10));   // 10
PRINT(accumulate(5));    // 15 — the same cell, shared across calls
```

Run it with the built binary:

```bash
wlwl run program.wll
```

## Build from source

Requires Rust ≥ 1.75; the exact toolchain is pinned in `rust-toolchain.toml`.

```bash
git clone https://github.com/Laffinty/wlwl
cd wlwl/impl
cargo build --release
./target/release/wlwl run examples/hello.wll
```

For development setup, quality gates and the workspace layout, see
[`CONTRIBUTING.md`](./CONTRIBUTING.md).

## Documentation

The language specification lives in
[`docs/spec/wlwl-spec-v0.11.md`](./docs/spec/wlwl-spec-v0.11.md), the
standard-library contract in
[`docs/stdlib/wlwl-stdlib-spec-v0.11.md`](./docs/stdlib/wlwl-stdlib-spec-v0.11.md),
and the release history in [`CHANGELOG.md`](./CHANGELOG.md).

## License

GPL-2.0-only — see [LICENSE](./LICENSE). Your `.wll` programs are your own
work and are not affected by the compiler's license.
