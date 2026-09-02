# WLWL

WLWL is a small experimental programming language in which every syntactic form is a function call.

[![License: GPL v2](https://img.shields.io/badge/License-GPL%20v2-blue.svg)](./LICENSE)
[![Spec: v0.3](https://img.shields.io/badge/spec-v0.3-green.svg)](./docs/standard/wlwl-spec-v0.3(MD5_4308b3d2071ebed5cb52eba61272b1ea).md)
[![Phase: 4](https://img.shields.io/badge/phase-4%20%E2%9C%93-brightgreen.svg)](./docs/plan/wlwl-build-plan-v0.1.md)

## Build & Run

Requires Rust ≥ 1.75.

```bash
cd impl
cargo build
./target/debug/wlwl run examples/hello.wl
```

## Example

```wlwl
LET(add, FUN((a, b), +(a, b)));
PRINT(add(2, 3));            // 5

LET(r, OR_DIE(+(1, ERR("oops")), -1));   // r = -1  (ERR transparently propagates)
PRINT(r);
```

## Commands

| Command | What it does |
|---|---|
| `wlwl run <file>` | run a `.wl` program |
| `wlwl run <file> --format=json` | emit errors as JSON (AI-friendly) |
| `wlwl run <file> --format=jsonl` | emit errors as JSONL stream (AI tools) |
| `wlwl check <file>` | parse + name-check only, no execution |
| `wlwl ast <file> --format=json` | dump the AST as JSON (for AI input) |

## See also

- Language spec: [`docs/standard/`](./docs/standard/) (v0.3, content-addressed by MD5)
- Build plan: [`docs/plan/wlwl-build-plan-v0.1.md`](./docs/plan/wlwl-build-plan-v0.1.md)
- Deviations log: [`docs/plan/deviations.md`](./docs/plan/deviations.md)
- Example: [`impl/examples/phase2_demo.wl`](./impl/examples/phase2_demo.wl)



## Build status

Latest release on `main`: `67e1b02` (2026-09-03). **223 / 223 tests passing**
across the workspace (`impl/crates/{wlwl-ast,wlwl-cli,wlwl-error,wlwl-eval,wlwl-lexer,wlwl-parser,wlwl-std,wlwl-toml}`).

### Phases shipped

- **Phase 1** (MVP): lexer, parser, evaluator, `wlwl run`, `PRINT` builtin.
- **Phase 2** (core semantics): control flow, functions, error handling (§12.6 transparent ERR propagation), single-directory modules.
- **Phase 3** (AI-friendly): 35 stable error codes across 11 categories, `retryable` / `suggestion_code` / `related` fields, JSON / JSONL streaming, `wlwl ast` subcommand, type-annotation slots (parsed, not checked).
- **Phase 4 batch 1**: `wlwl:std.io` (`PRINT`, `INPUT`) + `wlwl:std.fs` (`READ_FILE`, `WRITE_FILE`, `EXISTS`) + `wlwl:std.json` (`PARSE`, `STRINGIFY`) — triggers for E0060 / E0061 / E0062 / E0070 / E0071.
- **Phase 4 batch 2**: cross-directory IMPORTs (`./`, `../`), `ns:name` third-party namespace registry, `wlwl.toml` (`[package]` / `[dependencies]` / `[namespaces]`), `wlwl.lock` (JSON + SHA-256 + atomic write), full E0041 cycle path per §13.7.
- **Phase 4 batch 3**: `wlwl:std.ai` (mock — `ASK` / `EMBED` / `COMPLETE` with reserved-model error tokens for E0080–E0083), `wlwl-cli` lock generation after a successful `wlwl run`.
- **post-Phase 4**: per-parameter `name: Type` annotations on `FUN`, structured `TypeAnnotation` payload (`TypeExpr::Ident` / `::Array` / `::Generic`).

### Deferred / not yet started

- `P3-008` rich `suggestion_code` content (schema is in place, per-error-site codegen is the follow-up work).
- `P3-009` formal coverage measurement (no `cargo tarpaulin` run yet; test count is high but % is unmeasured).
- Phase 5 (Coq formalization of §19) — optional per build plan §3.
- Performance: tail-call optimization + hot-inline (deferred past Phase 4).
- Public release prep (README hardening, docs site, GitHub Release workflow) — build plan §0.1 decision 6.

See `docs/plan/deviations.md` for the full deviation log (Phase 2 / 3 / 4 / post-Phase 4) and
`docs/plan/wlwl-build-plan-v0.1.md` for the implementation tracker.
## License

GPL v2 — see [LICENSE](./LICENSE). Your `.wl` programs are your own work and are not affected by the compiler's license.
