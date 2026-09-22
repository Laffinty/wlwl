# Spec & conformance

The current WLWL language spec lives in `docs/standard/`.
Specs are identified by **version + name** (content-address filenames
were dropped); `git log --follow` is the source of truth for content
changes. Superseded specs are archived under `docs/history/`.

## Current spec

**[v0.7](../standard/wlwl-spec-v0.7.md)** — additive over v0.6
(structured concurrency + channels; §17 + Appendix G).

## Archived revisions

| Version | Location | Status |
|---|---|---|
| v0.6 | [`docs/history/wlwl-spec-v0.6.md`](../history/wlwl-spec-v0.6.md) | superseded by v0.7 (2026-09-22) |
| v0.5 | (never published) | superseded |
| v0.4 | (SHA-1 `97524ced037b5ef0a5820a2ebd5bafb4ba4e239b`) | superseded |
| v0.3 | (MD5 `4308b3d2071ebed5cb52eba61272b1ea`) | superseded |
| v0.2.x | (planned) | superseded |
| v0.1 | (planned) | history only |

> **v0.5 was never published** — the spec was drafted but not tagged,
> and the implementation never claimed compliance with it. v0.6 was the
> first versioned spec the implementation aligned to; v0.7 only appends.

> **Why MD5 for v0.3 but SHA-1 for v0.4+?** The v0.3 file was authored
> before we standardised on SHA-1. The MD5 hash is stable; v0.4 onwards
> use SHA-1.

## Compiler vs spec version

The compiler release **does not have to match** the spec version.
The current compiler implements the **v0.7 spec** in full (v0.6
semantics + §17 concurrency). CI tracks `wlwl --version` (compiler)
and spec anchors via `impl/crates/wlwl-cli/tests/conformance.rs`.

## Conformance test suite

- `cargo test -p wlwl-cli --test conformance` — checks that the
  current spec file is present and that the AST golden snapshots match.
- `cargo test -p wlwl-formatter --test formatter_tests` — verifies
  that every example in `impl/examples/` round-trips through
  `wlwl fmt` (canonical-form re-emission must re-parse cleanly).
- `cargo test --workspace` — full suite (~1340+ tests across 9
  crates).

Until the broader Phase H1 conformance suite lands, the bundled
`insta` snapshots in `crates/wlwl-error/src/snapshots/` form a
partial conformance harness for error-schema stability. See
[the Contributing page on snapshot testing](contributing.md#snapshot-testing).

## Spec test helpers

`cargo doc --workspace --no-deps` ensures all docs cross-reference
real symbols (no broken-intra-doc-link warnings).

`cargo deny --locked --all-features check` enforces the licenses of
all transitive dependencies; see
[`docs/history/deviations-v0.7.md`](../history/deviations-v0.7.md).

## Versioning policy

We follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html):

- **Major** bumps when the spec has a breaking runtime change
- **Minor** bumps for new builtins / new spec sections adopted
- **Patch** for doc-only or internal-only changes

See [`CHANGELOG.md`](../../CHANGELOG.md) for the actual version
history. v0.6 is the first release where this policy is applied
strictly.
