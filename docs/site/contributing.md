# Contributing

Welcome to WLWL. Most of the codebase is small (≈120 KLOC of Rust
across 12 crates) and the per-PR load is light. We welcome
contributions across language design, stdlib, tooling, and docs.

## Quick setup

```bash
git clone https://github.com/Laffinty/wlwl
cd wlwl/impl
cargo test --locked --all-targets
```

That runs the unit + integration tests, the Insta snapshot
suite, and the bench compilation. The toolchain is pinned to 1.99.0
in the repo-root `rust-toolchain.toml`, so `rustup` selects it for
you.

## Branches & commits

- `main` is v0.12.1 (published as the pre-release `v0.12.1_rc1`);
  only reviewed PRs land here.
- Commit messages start with the shorthand, e.g.
  `docs: 记录 v0.12.1_rc1 tag 已发`, `fix(impl): ...`.

## Local pre-PR checks

Run these before pushing:

```bash
# Format
cargo fmt --all -- --check

# Lint
cargo clippy --locked --workspace --all-targets -- -D warnings

# Doc
cargo doc --workspace --no-deps

# License + advisory
cargo deny --locked --all-features check

# Bench smoke
cargo bench --no-run --locked
```

The CI workflow `.github/workflows/ci.yml` runs the same checks on
Linux / macOS / Windows. Windows is the slowest leg (MSVC linker)
and is the canonical "does it build?" answer.

## Testing

Three testing tiers:

1. **Unit + integration**: `cargo test --locked --all-targets`
   covers everything below.
2. **Insta snapshots**: `crates/wlwl-error/src/snapshots/` pins
   the diagnostic JSON schema. To review changes locally:
   `cargo insta review`.
3. **Example programs**: 11 files in `impl/examples/`. They are
   the demonstrative set — additions to the language
   should come with an example.

### Snapshot testing

The snapshots live in `crates/wlwl-error/src/snapshots/`. Adding
a new `ErrorCode` variant requires extending one of the
`snap_*` tests AND the `all_error_codes_have_snapshots` meta-test
(see Phase G7). The meta-test fails CI if the union drops below
59 codes.

## Fuzz

The fuzz harness ships scaffold-only in `impl/fuzz/`. To exercise:

```bash
rustup toolchain install nightly --component rust-dev
cargo +nightly install cargo-fuzz --locked
cd impl/fuzz
cargo +nightly fuzz run fuzz_target_lexer -- -max_total_time=300
```

Fuzz does NOT run in CI (libFuzzer cycles take hours). Local devs
run periodically. See the ADRs under `docs/adr/` for the design
decisions it guards.

## Benchmarks

```bash
cd impl/crates/wlwl-eval
cargo bench --bench eval_hot_paths
```

The baseline measurements live in `benches/baseline.txt` and are
the source of truth for the perf-regression gate. CI runs the
benchmarks in smoke mode only (no full regression enforcement).

## Adding a new error code

1. Add the variant to `enum ErrorCode` in `crates/wlwl-error/src/lib.rs`.
2. Add it to a category mapping (`category()`).
3. Update one of the `snap_*` test JSON objects.
4. The meta-test (`all_error_codes_have_snapshots`) will pass
   automatically once the new code is in any snapshot.

## Documentation

User-facing docs live under `docs/site/` (mkdocs source) and ship
as the GitHub Pages site (target: `gh-pages` branch, deploy hook
not in this repo yet). Rebuild locally:

```bash
pip install mkdocs mkdocs-material
mkdocs serve
```

## Code of conduct

Be kind. The project lead (Li) is the only person with merge
rights; PRs from new contributors are patiently reviewed.
