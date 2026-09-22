# WLWL

> **Spec: v0.8 (in progress on `wip0.8` branch)** — clarifies v0.7 prose,
> aligns `BUILTIN_REGISTRY` to v0.7 chapter numbers, allows literal
> subscripts (`[1,2,3][0]`), demotes `YIELD` position limit from
> normative rule to implementation path, marks `E0055` / `E0057` as
> RESERVED. Implementation tracks **wlwl-spec-v0.7** with the v0.8
> prose additions in-place; release flow renames to
> `wlwl-spec-v0.8.md` and archives the v0.7 file to `docs/history/`.
> Build from source — no prebuilt binaries are published yet; v1.0
> will be the first release with signed artifacts.

WLWL is a small experimental programming language in which every syntactic form is a function call.

## Highlights

- Every syntactic form is a function call: no operators, no statements,
  no operator precedence. `LET`, `IF`, `+`, `=`, `[...]`, `IMPORT`,
  `MATCH`, `FOR`, `WHILE`, `${...}` — all are `name(arg, ...)` calls.
- Dynamic typing, prefix-functional. Types belong to values, not names.
- Mainstream truthiness: `0`, `0.0`, `""`, empty array, empty dict, and
  `NaN` are falsy. Every other value is truthy.
- Errors are values. `IF(cond, then, else)` routes `ERR(...)` from the
  `then` branch to `else`. `ERR_PAYLOAD(x)` recovers the message;
  `IS_ERR(x)` checks. Integer overflow throws `E0035`.
- String interpolation: `"hello ${name}!"`, multi-segment, escaped with
  `\$`. String subscript read: `s[i]` returns a single-codepoint string.
- Explicit mutability: `LET(name, value)` is immutable forever;
  `LET MUT(name, value)` allows `SET` rebinding. Re-bindings need to be
  visible up front.
- **Structured concurrency (v0.7)**: `SCOPE` / `SPAWN` / `AWAIT` /
  `YIELD`, advisory cancel (`TASK_*`, `SHIELD`), and channels
  (`CHANNEL_*`). Single-thread cooperative scheduler; no zombie tasks —
  child lifetime is bounded by the enclosing `SCOPE`.
- Self-hosted canonical formatter (§A.3): `wlwl fmt <file>` rewrites
  source to the canonical form, `wlwl fmt <file> --check` validates.
- Stable AST and error schemas for tooling (JSON / JSONL).

## Install

Build from source — requires Rust ≥ 1.75:

```bash
git clone https://github.com/Laffinty/wlwl
cd wlwl/impl
cargo build --release
./target/release/wlwl --version
./target/release/wlwl run examples/hello.wll
```

## Example

```wlwl
LET(greeting, "Hello, ${name}!");     // string interpolation
LET(name, "world");
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

### Concurrency (v0.7)

```wlwl
// Two tasks + channel fan-in inside a structured SCOPE
LET(v, SCOPE(FUN(() ,
    LET(ch, CHANNEL_NEW(2));
    LET(a, SPAWN(FUN(() ,
        CHANNEL_TRY_SEND(ch, 21);
        YIELD();
        CHANNEL_TRY_SEND(ch, 21);
        0
    )));
    LET(b, SPAWN(FUN(() ,
        CHANNEL_TRY_SEND(ch, 100);
        0
    )));
    AWAIT(a);
    AWAIT(b);
    LET(x, CHANNEL_TRY_RECV(ch));
    LET(y, CHANNEL_TRY_RECV(ch));
    LET(z, CHANNEL_TRY_RECV(ch));
    +(+(x, y), z)
)));
PRINT(v);   // 142
```

Rules of thumb:

- Every `SPAWN` must sit inside an explicit `SCOPE` (top-level `SPAWN`
  → `E0058`). No free-floating tasks.
- `YIELD()` only as a direct child of a multi-statement block
  (`1; YIELD(); 2`) — not as a bare `IF` branch.
- Prefer `CHANNEL_TRY_SEND` / `CHANNEL_TRY_RECV`; v0.7.0 blocking
  `SEND`/`RECV` return `ERR(kind="ChannelWouldBlock")` instead of
  suspending.
- Cancel is cooperative and advisory (`TASK_CANCEL*` never raises);
  observe with `TASK_IS_CANCELLED()`. `SHIELD(fn)` defers cancel until
  the outermost shield exits, but does **not** swallow `fn`'s own `ERR`.

More: `impl/tests/concurrency/*.wll` and the
[`wlwl-skill/`](./wlwl-skill/) concurrency cheatsheet.

A tour of the std modules (JSON round-trip, std.io, std.test) lives in
[`impl/examples/showcase.wll`](./impl/examples/showcase.wll).
Per-feature miniatures: `match.wll` / `destruct.wll` / `std_test.wll` /
`closure_cell.wll` / `format.wll` / `interp.wll`.

## What changed in v0.7 (additive over v0.6)

| Area | Before | After |
|---|---|---|
| Types | 9 types (… `FUNCTION`, `RESULT`) | + `TASK`, `CHANNEL` handles |
| Builtins | 93 registry entries | **110** (+17 concurrent, all non-ERR-consumer) |
| Errors | up to `E0051` / OOP reserved | + `E0052`–`E0058` (`Concurrent`) |
| ERR kinds | free-form dict `kind` | + `ChannelClosed`, `ChannelWouldBlock`, `Cancelled` |
| Control flow | sync only | `SCOPE`/`SPAWN`/`AWAIT`/`YIELD` + cancel + channels |
| Spec | v0.7 | **v0.8 wip0.8** — §12 保留形式 rewrite, 85 registry
section fields aligned, `[1,2,3][0]` literals allowed,
`NOT`/§17.1/`E0055`/`E0057` prose reconciled with implementation. |
| Spec | v0.6 | **v0.7** §17 (v0.6 text unchanged — additive only) |

v0.7 programs that do not depend on the old §12 "保留形式" table are
observationally unchanged in v0.8 except for literal subscripts
(see `CHANGELOG.md` Compatibility commitment). v0.6 programs that
do not use the new names are observationally unchanged. See
[`CHANGELOG.md`](./CHANGELOG.md) and
[`docs/standard/wlwl-spec-v0.7.md`](./docs/standard/wlwl-spec-v0.7.md)
§17.7 for known limits (path-B `YIELD` placement, non-suspending
`SEND`/`RECV`, single-thread scheduler).

## What changed in v0.8 (wip0.8)

Pure documentation + 1 syntax relaxation; no semantic rewrites. See
`CHANGELOG.md [v0.8.0]` for the full delta.

- **Spec §12 "保留形式" rewrite** (D8-005): historical stale list
  retired; registry is the single source of truth.
- **Literal subscripts allowed**: `[1,2,3][0]`, `["a":1]["a"]`, etc.
  This is the only observable-behavior change.
- **Spec / registry §-anchor alignment**: 85 `BuiltinSpec.section`
  fields + appendix_G header updated to v0.7 chapter numbers.
- **Doc reconciliations**: NOT transparent propagation reversed,
  `YIELD` position limit demoted, `=` triple identity, `E0055` /
  `E0057` marked RESERVED, AWAIT host diagnostic, SHIELD/SCOPE(ERR),
  `%` float `E0030`, FORMAT mixed placeholder, etc.

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

`BOOL` is an ERR consumer — `BOOL(ERR(...))` returns a boolean
without triggering §8.2 transparent propagation.

## Source file extension: `.wll`

WLWL source files use the `.wll` extension. (Previously `.wl`; renamed
in v0.6.1 because `.wl` is claimed by Wolfram Language and caused
editor / GitHub mis-identification. The rename is purely cosmetic — the
lexer, parser, evaluator, and formatter all operate on file content,
not filename, so behaviour is identical for `.wl` and `.wll`.)

Conformance fixtures in `impl/tests/conformance/*.wll` and
`impl/tests/concurrency/*.wll` use the same extension.

## Commands

| Command | What it does |
|---|---|
| `wlwl --version` | print version |
| `wlwl run <file>` | run a `.wll` program |
| `wlwl run <file> --format=json` | emit errors as JSON (AI-friendly) |
| `wlwl run <file> --format=jsonl` | emit errors as JSONL stream (AI tools) |
| `wlwl check <file>` | parse + name-check only, no execution |
| `wlwl ast <file> --format=json` | dump the AST as JSON (for AI input) |
| `wlwl fmt <file>` | canonical-formatter rewrite in place |
| `wlwl fmt <file> --check` | verify canonical form (emits `W0053` on drift) |
| `wlwl lock` | resolve + write `wlwl.lock` (Cargo-style) |

The full error schema (stable codes across categories + warnings) lives
in `wlwl-error`. v0.7 adds the `Concurrent` category
(`E0052`–`E0058`); v0.6 removed `W0015`/`W0054` and extended `E0035`.

## Workspace layout

```
impl/
├── crates/
│   ├── wlwl-ast/        AST nodes (Expr / Literal / StrPart / Pattern)
│   ├── wlwl-lexer/      Token stream + string interpolation lexer
│   ├── wlwl-parser/     EBNF-driven parser, AST builder
│   ├── wlwl-eval/       Tree-walking interpreter + concurrent runtime
│   │                    (runtime / task / channel / yield_split)
│   ├── wlwl-std/        wlwl:std.* namespaces (io, fs, json, collection, ...)
│   ├── wlwl-error/      Diagnostic schema (codes E0xxx / W0xxx)
│   ├── wlwl-formatter/  Canonical formatter (A.3)
│   ├── wlwl-toml/       wlwl.toml manifest + lockfile
│   └── wlwl-cli/        CLI entry (run / check / ast / fmt / lock)
├── examples/            .wll programs (showcase, miniatures)
├── tests/               conformance/ · concurrency/ · integration / fuzz
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
cargo run --bin gen-appendix-g -- ../docs/appendix_G.md   # after registry edits
```

## Docs map

| Doc | Path |
|---|---|
| Language spec (current) | [`docs/standard/wlwl-spec-v0.7.md`](./docs/standard/wlwl-spec-v0.7.md) (v0.8 wip0.8 branch accumulates in-place; rename at release) |
| Builtin registry (Appendix G mirror) | [`docs/appendix_G.md`](./docs/appendix_G.md) |
| ADRs | [`docs/adr/`](./docs/adr/) |
| Next-iteration plan home | [`docs/plan/README.md`](./docs/plan/README.md) |
| History (specs, completed plans, deviations) | [`docs/history/`](./docs/history/) |
| v0.7 build plan (COMPLETED) | [`docs/history/wlwl-build-plan-v0.7-COMPLETED.md`](./docs/history/wlwl-build-plan-v0.7-COMPLETED.md) |
| Deviations log (v0.7) | [`docs/history/deviations-v0.7.md`](./docs/history/deviations-v0.7.md) |
| Changelog | [`CHANGELOG.md`](./CHANGELOG.md) |
| Contributing | [`CONTRIBUTING.md`](./CONTRIBUTING.md) |
| Agent authoring skill | [`wlwl-skill/`](./wlwl-skill/) (Claude Skills format — drop into `~/.claude/skills/` to author v0.7-correct code) |
| Examples | [`hello.wll`](./impl/examples/hello.wll) · [`math.wll`](./impl/examples/math.wll) · [`showcase.wll`](./impl/examples/showcase.wll) · [`match.wll`](./impl/examples/match.wll) · [`destruct.wll`](./impl/examples/destruct.wll) · [`std_test.wll`](./impl/examples/std_test.wll) · [`closure_cell.wll`](./impl/examples/closure_cell.wll) · [`format.wll`](./impl/examples/format.wll) · [`interp.wll`](./impl/examples/interp.wll) |
| Concurrency fixtures | `impl/tests/concurrency/*.wll` |

## License

GPL v2 — see [LICENSE](./LICENSE). Your `.wll` programs are your own
work and are not affected by the compiler's license.
