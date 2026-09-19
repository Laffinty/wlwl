# wlwl-fuzz

[Phase G6] cargo-fuzz harness for WLWL lexer, parser, and evaluator.

## Setup

```sh
rustup toolchain install nightly --component rust-dev
cargo +nightly install cargo-fuzz --locked
```

## Running

```sh
cd impl/fuzz
cargo +nightly fuzz run fuzz_target_lexer  -- -max_total_time=300
cargo +nightly fuzz run fuzz_target_parser -- -max_total_time=300
cargo +nightly fuzz run fuzz_target_eval   -- -max_total_time=600
```

Each target maintains its own corpus under
`impl/fuzz/corpus/fuzz_target_<name>/`. Crashes (if any) land under
`impl/fuzz/artifacts/fuzz_target_<name>/`.

## Targets

| Target             | Driver input | Notes |
|--------------------|--------------|-------|
| `fuzz_target_lexer` | raw bytes    | Will not panic on any byte sequence |
| `fuzz_target_parser` | raw bytes    | First runs lexer; only fuzzes if lex succeeds |
| `fuzz_target_eval` | raw bytes    | Caps evaluator at 1024 steps per iteration |

## CI

Per P4-G6-001, fuzz targets do NOT run in CI. Run locally
on-demand. The CI matrix (Linux/macOS/Windows × Rust stable/nightly)
would otherwise balloon build time beyond budget. See
`docs/plan/deviations.md` P4-G6-001.
