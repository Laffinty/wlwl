# Welcome to WLWL

WLWL is a small experimental programming language in which **every
syntactic form is a function call**. There are no operators, no
control-flow keywords, and no statement-level syntax — every line
is `(operator operand1 operand2 ...)`. The compiler ships with a
fully spec-accurate interpreter, a standard library modelled on
the v0.3 §15 spec, and tooling aimed at AI workflows.

## Why a function-call-everywhere language?

> "Every syntactic form is a function call" means there's exactly
> one syntactic and semantic mechanism. HOMOICONICITY pops out
> naturally, and the AI/AST surface is regular enough that LLMs
> emit `.wl` syntax that's already correct.

The trade-off: no operator precedence (use parentheses), and
"infix" operators like `+` are postfix because `a + b` parses as
`+(a, b)` in WLWL. The canonical formatter (Phase E2) keeps that
consistent.

## Get started in 30 seconds

```bash
# On a host with rust ≥ 1.75 installed
git clone https://github.com/Laffinty/wlwl
cd wlwl/impl
cargo build --release
./target/release/wlwl run examples/hello.wl
```

→ **[Install page](install.md)** has the binary-download, SBOM, and
cosign verify steps.

## Where to go next

| If you want… | Read… |
|---|---|
| to write your first program | [Language tour](tour.md) |
| to see idiomatic `.wl` | [Examples](examples.md) |
| to understand a v0.4 design choice | [ADR catalog](adrs.md) |
| to verify the language spec compliance | [Spec & conformance](spec.md) |
| to contribute patches | [Contributing](contributing.md) |

## Project status

| Component | Version | Notes |
|---|---|---|
| Language spec | v0.4 | SHA-1 content-addressed in `docs/standard/` |
| Compiler (this repo) | v0.4.0-rc | "rc" until G12 mkdocs lands (you're looking at it) |
| Standard library | v0.4 | `wlwl:std.{io, fs, json, format, ai, agent, test, collection}` |
| Error schema | 1.1.0 | 58 error codes + 13 warning codes |
| License | GPL-2.0-only | Your `.wl` programs are yours |
