# WLWL

**A small experimental programming language where every operation is a named
function call.**

WLWL has one mechanism instead of five. There is no infix syntax and no
operator precedence: arithmetic is `+(a, b)`, comparison is `==(a, b)`, and
control flow, binding, iteration, pattern matching, modules, classes and
concurrency are written in the same call-shaped form with ordinary argument
lists. The whole language fits one grammar and one builtin table — and the
standard library is layered the same way, with every member's contract
written down.

[![CI](https://github.com/Laffinty/wlwl/actions/workflows/ci.yml/badge.svg)](https://github.com/Laffinty/wlwl/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/Laffinty/wlwl)](https://github.com/Laffinty/wlwl/releases)
[![License](https://img.shields.io/badge/license-GPL--2.0--only-blue.svg)](./LICENSE)

## Example

```wlwl
// A closure captures a mutable cell; every call shares that same cell.
LET MUT(total, 0);
LET(accumulate, FUN((amount), (
    SET(total, +(total, amount));
    total
)));

PRINT(accumulate(10));   // 10
PRINT(accumulate(5));    // 15 — the same cell, shared across calls
```

```bash
wlwl run program.wll
```

`wlwl` is the interpreter — [install a prebuilt binary](#install) or build it
from source.

Failures are values rather than exceptions:

```wlwl
LET(r, +(1, ERR("boom")));   // an ERR value, not a crash
PRINT(IS_ERR(r));             // TRUE
PRINT(UNWRAP_OR(r, -1));      // -1
```

## Why WLWL?

- **One syntax to learn.** `IF(cond, then, else)`, `WHILE(cond, body)`,
  `FOR(x, items, body)`, `MATCH(v, [clauses])`, `LET(name, value)`,
  `CLASS(name, parent, members)` — argument lists, not grammar branches.
  There is no precedence table to memorise and nothing for a parser to
  disambiguate.
- **Errors are values.** A failing operation yields an `ERR(...)` value that
  travels up the call chain untouched; only the registered consumers —
  `IS_ERR`, `ERR_PAYLOAD`, `UNWRAP_OR`, `TRY`, `WRAP` — can handle it. An
  error nobody handles surfaces as a stable diagnostic code (`E0102` when it
  escapes to the top level, `E0103` when it is discarded without being
  consumed) and a non-zero exit code.
- **Mutable state is explicit.** `LET` binds an immutable cell, `LET MUT`
  marks it mutable, `SET` rewrites it. Closures capture the cell rather than
  its value, so several closures share one piece of state — no globals, no
  hidden mutation.
- **Concurrency is part of the language.** `SCOPE`, `SPAWN`, `AWAIT`,
  `YIELD` and the `CHANNEL_*` family are language constructs, not a library
  convention: a cooperative single-threaded scheduler with real suspension,
  structured cancellation, and task lifetimes bounded by their lexical
  scope. There is no implicit runtime scope and no free-floating task.
- **Contracts, not conventions.** `SEALED([...])` states a module's
  complete public surface, and a sidecar `.wll.sig` file states its types.
  Both are checked in both directions, so a module cannot quietly grow
  beyond what it promised. The standard library is specified the same way,
  member by member, in its own volume.

## Standard library

23 `std.*` namespaces with 278 members, plus two official packages — `ai`
and `agent` — for AI inference and agent tool calls. That is 288 members
across 25 namespaces; `wlwl namespace` prints the live list.

| Namespace | What it covers |
|---|---|
| `std.io` `std.fs` `std.process` `std.env` | input/output, files and paths, subprocesses without a shell, process environment |
| `std.json` `std.format` `std.text` | JSON, template formatting, Unicode case handling and normalisation |
| `std.collection` `std.str` `std.math` `std.result` | collections, string helpers, exact decimals and fractions, `RESULT` combinators |
| `std.encode` `std.sanitize` `std.compress` | base64/hex/percent, SHA-256, HMAC, PBKDF2, Argon2id, output safety, zstd and the DEFLATE family |
| `std.time` `std.regex` `std.rand` `std.net` `std.test` | wall and monotonic clocks, linear-time regex, explicitly seeded randomness, TCP and HTTP, the test harness |
| `std.http` `std.sync` `std.log` `std.signal` | HTTP parsing, routing, static files and middleware; atomic counters and shared dictionaries; structured logs and metrics; signal handling and graceful shutdown |

Each member's contract — including its failure modes — is specified in the
standard-library specification and frozen by contract tests.

## Toolchain

`wlwl` is a tree-walking interpreter with a toolchain around it.

| Command | What it does |
|---|---|
| `wlwl run <file>` | evaluate a program |
| `wlwl check <file>` | parse, report unused bindings and static-contract diagnostics (no name resolution) |
| `wlwl ast <file>` | print the parse tree as JSON |
| `wlwl fmt <file>` | canonical formatter — `--check` to verify, `-w` to rewrite in place |
| `wlwl sig` / `wlwl sig-gen` | print a module signature / generate a `.wll.sig` skeleton |
| `wlwl interface <file>` | dump a module's public surface as JSON |
| `wlwl namespace` | dump every namespace and its members, as JSON |
| `wlwl schema` | dump the type system and static-contract codes as JSON |
| `wlwl lsp` | language server on stdio: diagnostics, definition, hover, completion |
| `wlwl repl` | a minimal REPL — evaluate one expression at a time |
| `wlwl tasks` | dump the runtime task view, as JSON |

Diagnostics carry stable four-digit codes and are available as
machine-readable JSON or JSONL.

## Documentation

| Document | Contents |
|---|---|
| [Specification reference](docs/wlwl-spec-reference.md) | the merged specification — the language, the standard library, and the conformance rules — in one file |
| [Language tour](docs/site/tour.md) | a guided introduction |
| [Agent skill pack](wlwl-skill/) | an AI-agent skill package with 22 runnable `.wll` examples |
| [Changelog](CHANGELOG.md) | release history |
| [Contributing](CONTRIBUTING.md) | development workflow, quality gates, conventions |

The authoritative specification lives in the separate
[`Laffinty/wlwl-spec-v1`](https://github.com/Laffinty/wlwl-spec-v1) repository;
the file linked above is the merged copy kept in this repository for the machine
guards and AI agents.

## Install

Prebuilt binaries for Linux, macOS and Windows are published on the
[Releases page](https://github.com/Laffinty/wlwl/releases). Each archive
carries the binary, the example programs, a CycloneDX SBOM and a cosign
signature bundle.

## Build from source

Requires Rust ≥ 1.85 (MSRV); the toolchain is pinned in
`rust-toolchain.toml`.

```bash
git clone https://github.com/Laffinty/wlwl
cd wlwl/impl
cargo build --release
./target/release/wlwl run examples/hello.wll
```

## Status

WLWL is an experimental language. The language and the standard library are
written down in full, but the implementation is young and some corners are
deliberately unfinished.

- There are no blocking synchronisation primitives — no mutex, no
  condition variable. The specification makes shared mutable state an
  anti-pattern and puts the burden on channels instead; locks cannot align
  with the scheduler's suspension points. What `std.sync` does provide is
  the part that does not need to suspend: atomic counters and shared
  dictionaries.
- **Workspace version: 0.12.1** (`impl/Cargo.toml`). What is *published* is the
  pre-release
  [v0.12.1_rc1](https://github.com/Laffinty/wlwl/releases/tag/v0.12.1_rc1) —
  a **snapshot taken at that tag**, **not** the current tree: fixes landed
  after the tag are **not** in it, and ⚠ **its archive contains no TLS
  extension** (that packaging step was added after the tag). The latest
  *final* release is
  [v0.12.0](https://github.com/Laffinty/wlwl/releases/tag/v0.12.0); the final
  `v0.12.1` has not been released yet.

## License

GPL-2.0-only — see [LICENSE](./LICENSE). Your `.wll` programs are your own
work and are not affected by the compiler's license.
