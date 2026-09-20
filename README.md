# WLWL

WLWL is a small experimental programming language in which every syntactic form is a function call.

> **Status: v0.6 (development)**. The compiler implements the v0.6
> stable subset: mainstream truthiness (`0`/`""`/empty containers/NaN
> are falsy), short-circuit `&&`/`||`, `IF` consumes `ERR` at the
> condition position, integer overflow throws `E0035`, strings
> support subscript read + interpolation `${expr}`, mutable bindings
> require explicit `LET MUT`. Track `docs/standard/` (SHA-1
> content-addressed) for the canonical spec.

## Install

Build from source — requires Rust ≥ 1.75:

```bash
git clone https://github.com/Laffinty/wlwl
cd wlwl/impl
cargo build --release
./target/release/wlwl run examples/hello.wll
```

No pre-built binaries are published yet (the v1.0 release will be the
first).

## Example

```wlwl
LET(greeting, "Hello, ${PRINT_VERSION()}!");  // string interpolation
PRINT(greeting);

LET MUT(counter, 0);                          // mutable binding
LET(step, FUN((), (
    SET(counter, +(counter, 1));
    counter
)));
LET(c, step());                               // 1
LET(c, step());                               // 2 — counter shared via closure

LET(r, UNWRAP_OR(+(1, ERR("oops")), -1));     // r = -1  (ERR transparently propagates)
PRINT(r);
```

A tour of the std modules (JSON round-trip, std.io, std.test) lives in
[`impl/examples/showcase.wll`](./impl/examples/showcase.wll).
Per-feature miniatures: `match.wll` / `destruct.wll` / `std_test.wll` /
`closure_cell.wll` / `format.wll` / `interp.wll`.

## What changed in v0.6

| Decision | Before | After |
|---|---|---|
| Truthiness (A) | only `FALSE`/`NULL` falsy (8 falsy values: `FALSE`, `NULL`, `0`, `0.0`, `""`, empty ARRAY, empty DICT, `NaN`) | mainstream — matches Python/JS/Ruby |
| `&&` / `\|\|` (B) | always evaluate both sides | **short-circuit**; left ERR propagates without evaluating right |
| `IF` + `ERR` (C-1) | `IF(ERR, ...)` takes then branch | `IF(ERR, ...)` routes to else branch (or propagates if no else) |
| `!` vs `NOT` (D-2) | `!` emits `W0054` deprecation | both forms accepted, no warning |
| `POP` (E) | ambiguous — sometimes removed + returned | renamed `AT_K` (lookup only); `POP` is alias for back-compat |
| String subscript (F) | strings had no subscript | `s[i]` returns single-codepoint string; `s[i] = ...` rejected |
| `LET MUT` (G-1) | cell upgraded to mutable on first closure capture | explicit `LET MUT(name, value)`; flag is permanent |
| Integer overflow (H-2) | saturate to `INT_MIN`/`INT_MAX` + `W0015` | throws `E0035` |
| String interpolation (J) | none | `"${expr}"` form; ERR propagates |

`BOOL` is now an ERR consumer — `BOOL(ERR(...))` returns a boolean
without triggering §8.2 transparent propagation.

## Commands

| Command | What it does |
|---|---|
| `wlwl run <file>` | run a `.wll` program |
| `wlwl run <file> --format=json` | emit errors as JSON (AI-friendly) |
| `wlwl run <file> --format=jsonl` | emit errors as JSONL stream (AI tools) |
| `wlwl check <file>` | parse + name-check only, no execution |
| `wlwl ast <file> --format=json` | dump the AST as JSON (for AI input) |
| `wlwl fmt <file>` | canonical-formatter rewrite in place |
| `wlwl fmt <file> --check` | verify canonical form (emits `W0053` on drift) |
| `wlwl lock` | resolve + write `wlwl.lock` (Cargo-style) |

The full error schema (stable codes across categories + warnings) lives
in `wlwl-error`. v0.6 changes: `W0015` (saturate warning) removed;
`E0035` extended to integer overflow; `W0054` (`!` deprecation) removed.

## Workspace layout

```
impl/
├── crates/
│   ├── wlwl-ast/        AST nodes (Expr / Literal / StrPart / Pattern)
│   ├── wlwl-lexer/      Token stream + string interpolation lexer
│   ├── wlwl-parser/     EBNF-driven parser, AST builder
│   ├── wlwl-eval/       Tree-walking interpreter, builtin registry
│   ├── wlwl-std/        wlwl:std.* namespaces (io, fs, json, collection, ...)
│   ├── wlwl-error/      Diagnostic schema (codes E0xxx / W0xxx)
│   ├── wlwl-formatter/  Canonical formatter (A.3)
│   ├── wlwl-toml/       wlwl.toml manifest + lockfile
│   └── wlwl-cli/        CLI entry (run / check / ast / fmt / lock)
├── examples/            .wll programs (showcase, miniatures)
├── tests/               integration / conformance / fuzz
└── Cargo.toml           workspace manifest
```

## Quality gates

Run from `impl/`:

```bash
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo doc --workspace --no-deps -- -D warnings
cargo test --locked --all-targets
cargo bench --locked -p wlwl-eval --bench eval_hot_paths
```

Current test count: **~1340+** tests across 9 crates, all green.

## See also

- Language spec: [`docs/standard/`](./docs/standard/)
  (`wlwl-spec-v0.6(SHA1_cdb548cb5161e61d836aad2208fd33adc0917861).md`)
- Build plan: [`docs/plan/wlwl-build-plan-v0.2.md`](./docs/plan/wlwl-build-plan-v0.2.md)
- Deviations log: [`docs/plan/deviations.md`](./docs/plan/deviations.md)
- Changelog: [`CHANGELOG.md`](./CHANGELOG.md)
- Contributing guide: [`CONTRIBUTING.md`](./CONTRIBUTING.md)
- Examples:
  [`hello.wll`](./impl/examples/hello.wll) ·
  [`math.wll`](./impl/examples/math.wll) ·
  [`showcase.wll`](./impl/examples/showcase.wll) ·
  [`match.wll`](./impl/examples/match.wll) ·
  [`destruct.wll`](./impl/examples/destruct.wll) ·
  [`std_test.wll`](./impl/examples/std_test.wll) ·
  [`closure_cell.wll`](./impl/examples/closure_cell.wll) ·
  [`format.wll`](./impl/examples/format.wll) ·
  [`interp.wll`](./impl/examples/interp.wll)

## License

GPL v2 — see [LICENSE](./LICENSE). Your `.wll` programs are your own
work and are not affected by the compiler's license.
