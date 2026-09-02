# WLWL

> A language where every syntactic form is a function call.

[![License: GPL v2](https://img.shields.io/badge/License-GPL%20v2-blue.svg)](./LICENSE)
[![Spec: v0.3](https://img.shields.io/badge/spec-v0.3-green.svg)](./docs/standard/wlwl-spec-v0.3(MD5_541e3fbbbba2492258df5d13cc5f71ae).md)
[![Phase: 2](https://img.shields.io/badge/phase-2%20%E2%9C%93-yellow.svg)](./docs/plan/wlwl-build-plan-v0.1.md)

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
| `wlwl check <file>` | parse + name-check only, no execution |

## See also

- Language spec: [`docs/standard/`](./docs/standard/) (v0.3, content-addressed by MD5)
- Build plan: [`docs/plan/wlwl-build-plan-v0.1.md`](./docs/plan/wlwl-build-plan-v0.1.md)
- Phase 2 deviations: [`docs/plan/deviations.md`](./docs/plan/deviations.md)
- Example: [`impl/examples/phase2_demo.wl`](./impl/examples/phase2_demo.wl)

## License

GPL v2 — see [LICENSE](./LICENSE). Your `.wl` programs are your own work and are not affected by the compiler's license.
