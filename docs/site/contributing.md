# Contributing

Welcome to WLWL. Most of the codebase is small (≈30 KLOC of Rust
across 9 crates) and the per-PR load is light. We welcome
contributions across language design, stdlib, tooling, and docs.

## Quick setup

```bash
git clone https://github.com/Laffinty/wlwl
cd wlwl/impl
cargo test --locked --all-targets
```

That runs the 1100+ unit + integration tests, the Insta snapshot
suite, and the bench compilation.

## Branches & commits

- `main` is v0.4.0-rc; only reviewed PRs land here.
- Phase-tagged branches follow the Phase letter: `phase/D4`,
  `phase/E1`. We retire them once merged.
- Commit messages start with the phase / commit shorthand:
  `G2 (Phase G): cargo-deny license + advisory gate`.

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
3. **Example programs**: 10 files in `impl/examples/`. They are
   the v0.4 spec demonstrative set — additions to the language
   should come with an example.

### Snapshot testing

The snapshots live in `crates/wlwl-error/src/snapshots/`. Adding
a new `ErrorCode` variant requires extending one of the
`snap_*` tests AND the `all_error_codes_have_snapshots` meta-test
(see Phase G7). The meta-test fails CI if the union drops below
58 codes (the v0.4 baseline).

## Fuzz

The fuzz harness ships scaffold-only in `impl/fuzz/`. To exercise:

```bash
rustup toolchain install nightly --component rust-dev
cargo +nightly install cargo-fuzz --locked
cd impl/fuzz
cargo +nightly fuzz run fuzz_target_lexer -- -max_total_time=300
```

Fuzz does NOT run in CI (libFuzzer cycles take hours). Local devs
run periodically. See [`docs/adr/` ADR absent] and `deviations.md`
P4-G6-001.

## Benchmarks

```bash
cd impl/crates/wlwl-eval
cargo bench --bench eval_hot_paths
```

The 5 baseline measurements live in `benches/baseline.txt` and are
the Phase F1 source of truth. CI runs the benchmarks in smoke
mode only (P4-G8-001 — full 110% regression enforcement is v0.5).

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
