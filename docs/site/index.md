# Welcome to WLWL

WLWL is a small experimental programming language in which **every
syntactic form is a function call**. There are no operators, no
control-flow keywords, and no statement-level syntax — every line
is `(operator operand1 operand2 ...)`. The compiler ships with a
fully spec-accurate interpreter, a standard library specified in the
**v1.0** specification (published in the separate
[`wlwl-spec-v1`](https://github.com/Laffinty/wlwl-spec-v1) repository),
and tooling aimed at AI workflows.

## Why a function-call-everywhere language?

> "Every syntactic form is a function call" means there's exactly
> one syntactic and semantic mechanism. HOMOICONICITY pops out
> naturally, and the AI/AST surface is regular enough that LLMs
> emit `.wll` syntax that's already correct.

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
./target/release/wlwl run examples/hello.wll
```

→ **[Install page](install.md)** has the binary-download, SBOM, and
cosign verify steps.

## Where to go next

| If you want… | Read… |
|---|---|
| to write your first program | [Language tour](tour.md) |
| to see idiomatic `.wll` | [Examples](examples.md) |
| to understand a v0.4 design choice | [ADR catalog](adrs.md) |
| to verify the language spec compliance | [Spec & conformance](spec.md) |
| to contribute patches | [Contributing](contributing.md) |

## Project status

| Component | Version | Notes |
|---|---|---|
| Language spec | v1.0 | published in the separate `wlwl-spec-v1` repo; the merged copy in `docs/wlwl-spec-reference.md` is what this repo's guards and agents read (older editions archived to `docs/history/`) |
| Compiler (this repo) | v0.12.0 | structured concurrency + channels; ships the first batch of the v1.0 semantics |
| Standard library | v1.0 | the `wlwl:std.*` namespaces plus the `ai` / `agent` packages |
| Error schema | 1.1.0 | stable four-digit error and warning codes, machine-readable as JSON / JSONL |
| License | GPL-2.0-only | Your `.wll` programs are yours |
