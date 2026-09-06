# WLWL

WLWL is a small experimental programming language in which every syntactic form is a function call.

> **Status: under active development.** The language spec is being
> revised from v0.3 to v0.4; the compiler implements the v0.3 stable
> subset plus selected v0.4 work (closure cell semantics, error
> schema 1.1.0). Breaking changes between minor versions are expected
> before v1.0. Track the v0.2 build plan in
> [`docs/plan/wlwl-build-plan-v0.2.md`](./docs/plan/wlwl-build-plan-v0.2.md)
> for what's in flight.

## Install

Pre-built binaries for Linux, macOS (x86_64 + aarch64) and Windows (x86_64)
are attached to every release on the
[Releases page](https://github.com/Laffinty/wlwl/releases). Download, extract,
and put the `wlwl` binary on your `PATH`.

Or build from source — requires Rust ≥ 1.75:

```bash
git clone https://github.com/Laffinty/wlwl
cd wlwl/impl
cargo build --release
./target/release/wlwl run examples/hello.wl
```

## Example

```wlwl
LET(add, FUN((a, b), +(a, b)));
PRINT(add(2, 3));            // 5

LET(r, OR_DIE(+(1, ERR("oops")), -1));   // r = -1  (ERR transparently propagates)
PRINT(r);
```

A more complete tour (Phase 4 std modules, JSON round-trip, modules across
directories) lives in [`impl/examples/showcase.wl`](./impl/examples/showcase.wl).

## Commands

| Command | What it does |
|---|---|
| `wlwl run <file>` | run a `.wl` program |
| `wlwl run <file> --format=json` | emit errors as JSON (AI-friendly) |
| `wlwl run <file> --format=jsonl` | emit errors as JSONL stream (AI tools) |
| `wlwl check <file>` | parse + name-check only, no execution |
| `wlwl ast <file> --format=json` | dump the AST as JSON (for AI input) |

The full error schema (35 stable codes across 11 categories) is defined in
v0.3 §14. The `wlwl-error` crate ships with `insta` snapshot tests that pin
the schema to `0.3.1`.

## See also

- Language spec: [`docs/standard/`](./docs/standard/) (v0.3, content-addressed by MD5)
- Build plan: [`docs/plan/wlwl-build-plan-v0.1.md`](./docs/plan/wlwl-build-plan-v0.1.md)
- Deviations log: [`docs/plan/deviations.md`](./docs/plan/deviations.md)
- Changelog: [`CHANGELOG.md`](./CHANGELOG.md)
- Contributing guide: [`CONTRIBUTING.md`](./CONTRIBUTING.md)
- Examples:
  [`hello.wl`](./impl/examples/hello.wl) ·
  [`phase2_demo.wl`](./impl/examples/phase2_demo.wl) ·
  [`showcase.wl`](./impl/examples/showcase.wl)


## License

GPL v2 — see [LICENSE](./LICENSE). Your `.wl` programs are your own work and are not affected by the compiler's license.