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
| Compiler (this repo) | v0.12.1 | structured concurrency + channels; ships the v1.0 semantics + the v0.12.1 public-surface batch (spec v1.0.4) |
| Standard library | v1.0 | the `wlwl:std.*` namespaces plus the `ai` / `agent` packages |
| Error schema | 1.1.0 | stable four-digit error and warning codes, machine-readable as JSON / JSONL |
| License | GPL-2.0-only | Your `.wll` programs are yours |

## Standard-library surface

Two counts exist and are easy to mix up:

- **What you can actually call today** — the implementation surface, printed
  by `wlwl namespace` (it labels itself `"surface": "implementation"`):

  ```bash
  wlwl --help       # the subcommands
  wlwl namespace    # every namespace and its members, as JSON
  ```

  On the current build that is **288 members across 25 namespaces**:
  23 `wlwl:std.*` namespaces plus the two official packages `ai` / `agent`.

- **What the spec's appendix A registers** — a *frozen snapshot* registry.
  Per the spec's own rule it does **not** drift with the compiler, so a member
  that exists in the compiler but is not in appendix A is a **conformance
  gap** being recorded through the spec's revision process — not an extra
  member. Read the appendix itself for its contents:
  [`docs/wlwl-spec-reference.md`](../wlwl-spec-reference.md).

`wlwl namespace` is the right tool when you are writing code; appendix A is
the right tool when you are reasoning about spec conformance.

Newest additions worth knowing about:

| Area | What landed |
|---|---|
| Servers | `std.net::TCP_SERVE(listener, timeout_ms, opts, handler)` runs the accept loop in a native frame; `std.http::SERVE_STREAM` does the Keep-Alive variant. The handler returns a response `DICT` and must **not** call a host-suspending member. |
| Non-blocking outbound | `std.net::HTTP_REQUEST_ASYNC` — same arguments and same `{status, headers, body}` result as `HTTP_REQUEST`, but it suspends only the calling task instead of blocking the runtime. |
| HTTP middleware | `std.http::RATE_LIMITER` (sliding window / token bucket), `RULESET_LOAD` (hot-reloaded rule file), `CLIENT_IP` (real client IP behind trusted proxies), with JSON audit rows via `std.log`. |
| Shared state | `std.sync` — atomic `COUNTER` and `SHARED_DICT` containers. |
| Shutdown | `std.signal` — `ON_SIGNAL` / `STOP`. |
| TLS | **Not in the standard library.** `std.net` is plaintext `http:` only. TLS ships as a native dynamic library loaded explicitly by the host (`wlwl run --ext /abs/path/lib.so`) — and loading it hands the whole process's permissions to that library. |
| Text conversion | `STR(x)` (number → text) and `SPLIT(s, sep)` are **global builtins**, no `IMPORT` needed. `+` performs **no** implicit coercion. |
