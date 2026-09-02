# WLWL

WLWL is a small experimental programming language in which every syntactic form is a function call.

[![License: GPL v2](https://img.shields.io/badge/License-GPL%20v2-blue.svg)](./LICENSE)
[![Spec: v0.3](https://img.shields.io/badge/spec-v0.3-green.svg)](./docs/standard/wlwl-spec-v0.3(MD5_4308b3d2071ebed5cb52eba61272b1ea).md)
[![Phase: 3](https://img.shields.io/badge/phase-3%20%E2%9C%93-brightgreen.svg)](./docs/plan/wlwl-build-plan-v0.1.md)

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
- Phase 2 deviations: [`docs/plan/deviations.md`](./docs/plan/deviations.md)
- Example: [`impl/examples/phase2_demo.wl`](./impl/examples/phase2_demo.wl)



## Phase 3 (AI-friendly)

Phase 3 ships the v0.3 AI-facing diagnostic contract:

- 35 stable error codes across 11 categories (`errorCategory` field)
- `retryable: bool` per code (AI tools can auto-retry transient IO/AI errors)
- `suggestion_code: Vec<Suggestion>` (machine-apply-able patches; up to 3 per diagnostic, sorted by confidence)
- `related: Vec<RelatedLocation>` (secondary locations for E0021 duplicate, E0041 cycle, etc.)
- Streaming JSONL output (`--format=jsonl`) for line-oriented AI consumers
- `wlwl ast` subcommand for AI tools to ingest the parsed AST as JSON
- Optional type annotations `name: Type` on LET and FUN return types (parsed, not checked) per v0.3 `Sec. 2.4`
- 10 insta JSON snapshots pin the error-code schema; 9 AI contract tests enforce the `Sec. 14.7` field guarantees

See `docs/plan/deviations.md` for the full Phase 3 deviation log.

## License

GPL v2 — see [LICENSE](./LICENSE). Your `.wl` programs are your own work and are not affected by the compiler's license.
