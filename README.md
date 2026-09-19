# WLWL

WLWL is a small experimental programming language in which every syntactic form is a function call.

> **Status: v0.4 development (Phase G complete)**. The compiler
> implements the v0.4 stable subset plus the v0.3 back-compat
> surface (closure cell semantics, error schema 1.1.0, MVS
> dependency resolution, canonical formatter, AS keyword removal).
> Track `docs/plan/wlwl-build-plan-v0.2.md` for what's still in
> flight before the v0.4.0 tag.

## Install

Pre-built binaries for Linux (x86_64), macOS (x86_64 + aarch64) and
Windows (x86_64) are attached to every release. Each tarball/zip
carries a CycloneDX SBOM and a sigstore/cosign signature. Download,
extract, and put the `wlwl` binary on your `PATH`:

```bash
# Linux/macOS
tar -xJf wlwl-v0.4.0-x86_64-unknown-linux-gnu.tar.xz
export PATH="$PWD/wlwl-v0.4.0-x86_64-unknown-linux-gnu:$PATH"

# Verify signature
cosign verify-blob \
  --certificate-identity-regexp 'github.com/Laffinty/wlwl' \
  --certificate-oidc-issuer 'https://token.actions.githubusercontent.com' \
  wlwl-v0.4.0-x86_64-unknown-linux-gnu.tar.xz
```

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

A tour of the Phase 4 std modules (JSON round-trip, std.io, std.test)
lives in [`impl/examples/showcase.wl`](./impl/examples/showcase.wl).
Per-feature miniatures: `match.wl` / `destruct.wl` /
`std_test.wl` / `closure_cell.wl` / `format.wl`.

## Commands

| Command | What it does |
|---|---|
| `wlwl run <file>` | run a `.wl` program |
| `wlwl run <file> --format=json` | emit errors as JSON (AI-friendly) |
| `wlwl run <file> --format=jsonl` | emit errors as JSONL stream (AI tools) |
| `wlwl check <file>` | parse + name-check only, no execution |
| `wlwl ast <file> --format=json` | dump the AST as JSON (for AI input) |
| `wlwl fmt <file>` | canonical-formatter rewrite in place |
| `wlwl lock` | resolve + write `wlwl.lock` (Cargo-style) |

The full error schema (58 stable codes across 12 categories + 13 warnings)
lives in `wlwl-error` with `insta` snapshot tests that pin the schema
to `1.1.0`.

## Phase G quality gates (v0.4 hard requirements)

- `cargo fmt --all -- --check` — formatting
- `cargo clippy --locked --workspace --all-targets -- -D warnings` — clippy
- `cargo doc --workspace --no-deps -- -D warnings` — rustdoc lint
- `cargo deny --locked --all-features check` — license + advisory
- `cargo test --locked --all-targets` — full suite (1137+ tests)
- `cargo bench --locked -p wlwl-eval --bench eval_hot_paths` — perf smoke

Release-side:
- CycloneDX SBOM (`sbom.json`) attached to every GitHub release
- `cosign sign-blob` keyless OIDC signature per binary archive
- Weekly `cargo audit` run pinned to `taiki-e/install-action@v0.21`

## See also

- Language spec: [`docs/standard/`](./docs/standard/) (v0.4, SHA-1 content-addressed)
- Build plan: [`docs/plan/wlwl-build-plan-v0.2.md`](./docs/plan/wlwl-build-plan-v0.2.md)
- Architecture decisions: [`docs/adr/`](./docs/adr/) (ADR-0008..0013)
- Deviations log: [`docs/plan/deviations.md`](./docs/plan/deviations.md)
- Changelog: [`CHANGELOG.md`](./CHANGELOG.md)
- Contributing guide: [`CONTRIBUTING.md`](./CONTRIBUTING.md)
- Examples:
  [`hello.wl`](./impl/examples/hello.wl) ·
  [`math.wl`](./impl/examples/math.wl) ·
  [`phase2_demo.wl`](./impl/examples/phase2_demo.wl) ·
  [`showcase.wl`](./impl/examples/showcase.wl) ·
  [`test_array.wl`](./impl/examples/test_array.wl) ·
  [`match.wl`](./impl/examples/match.wl) ·
  [`destruct.wl`](./impl/examples/destruct.wl) ·
  [`std_test.wl`](./impl/examples/std_test.wl) ·
  [`closure_cell.wl`](./impl/examples/closure_cell.wl) ·
  [`format.wl`](./impl/examples/format.wl)

## License

GPL v2 — see [LICENSE](./LICENSE). Your `.wl` programs are your own
work and are not affected by the compiler's license.
