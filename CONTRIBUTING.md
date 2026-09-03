# Contributing to WLWL

Thanks for your interest in WLWL. This document covers the day-to-day
contribution workflow. For deeper design context, read:

- [`docs/standard/`](docs/standard/) — the language spec (content-addressed by MD5)
- [`docs/plan/wlwl-build-plan-v0.1.md`](docs/plan/wlwl-build-plan-v0.1.md) — the implementation build plan
- [`docs/plan/deviations.md`](docs/plan/deviations.md) — known deviations from the spec / plan

## Ground rules

1. **The spec is authoritative.** If your change conflicts with the spec,
   the spec wins — open a spec revision issue first. Implementation-only
   deviations must be added to `docs/plan/deviations.md` per build plan §8.
2. **The compiler is GPL v2.** By contributing, you agree your contribution
   is licensed under GPL v2. Your `.wl` programs are not affected.
3. **Small PRs.** One concern per PR. If a change touches the parser and
   the evaluator, it is probably two PRs.
4. **AI-friendly diagnostics are a hard requirement.** Any change to error
   handling must keep the v0.3 §14 schema fields (`errorCategory`,
   `retryable`, `suggestion_code`, `related`, `error_schema_version`)
   intact. Schema changes go through a spec revision.

## Development setup

Requires Rust ≥ 1.75 (the MSRV pinned in `impl/Cargo.toml`).

```bash
git clone https://github.com/Laffinty/wlwl
cd wlwl/impl
cargo build
cargo test
```

Run an example:

```bash
cargo run -- run examples/hello.wl
cargo run -- run examples/phase2_demo.wl
cargo run -- check examples/phase2_demo.wl     # parse + name-check only
cargo run -- ast   examples/phase2_demo.wl --format=json | head
```

## Project layout

```
wlwl/
├── docs/
│   ├── standard/   # the v0.3 language spec (MD5-content-addressed)
│   ├── plan/       # the implementation build plan + deviations log
│   └── history/    # dated progress notes
├── impl/
│   ├── Cargo.toml  # workspace root
│   ├── crates/
│   │   ├── wlwl-ast/      # AST data structures + serde
│   │   ├── wlwl-lexer/    # hand-written lexer
│   │   ├── wlwl-parser/   # hand-written recursive-descent parser
│   │   ├── wlwl-eval/     # tree-walking interpreter
│   │   ├── wlwl-error/    # diagnostic + 35 stable error codes
│   │   ├── wlwl-std/      # wlwl:std.* modules
│   │   ├── wlwl-toml/     # wlwl.toml + wlwl.lock
│   │   └── wlwl-cli/      # the `wlwl` binary
│   └── examples/   # .wl programs used in tests + docs
└── .github/workflows/
    ├── ci.yml        # test on Linux / macOS / Windows
    └── release.yml   # cross-platform binary release on tag
```

## Coding conventions

- **Rust**: standard `rustfmt` + `clippy` defaults; CI runs both with
  `-D warnings` on the workspace. Run `cargo fmt` before opening a PR.
- **Error codes**: every new error site must pick the closest existing
  code from `wlwl-error::ErrorCode`. If none fits, propose a new code in
  the PR description and update the snapshot test in
  `wlwl-error/src/snapshots/`.
- **Tests**: every new error path needs an `insta` snapshot or an
  explicit `assert_eq!` in the corresponding `tests` module. The CI smoke
  job exercises the binary against the bundled examples.

## Pull request checklist

- [ ] `cargo fmt --all`
- [ ] `cargo build --all-targets`
- [ ] `cargo test --all-targets` (all tests pass)
- [ ] `cargo clippy --all-targets --all-features -- -D warnings`
- [ ] New error codes documented in `wlwl-error/src/lib.rs`
- [ ] New behavior added to the example in `examples/` if it is
      user-visible
- [ ] If you changed the spec, **do not** — open a spec issue first

## Release process

The release engineer (currently `@Laffinty`) bumps the workspace version
in `impl/Cargo.toml`, updates `CHANGELOG.md`, commits to `main`, tags
`vX.Y.Z`, and pushes the tag. The `release.yml` workflow builds
cross-platform binaries and creates the GitHub Release automatically.

If you want to make a backport or hotfix, propose it in an issue first.