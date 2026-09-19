# Spec & conformance

The WLWL language spec lives in `docs/standard/` and is
**content-addressed** so every spec revision is reproducible.

## Current spec

[v0.4](../standard/wlwl-spec-v0.4(SHA1_97524ced037b5ef0a5820a2ebd5bafb4ba4e239b).md)
(SHA-1 `97524ced037b5ef0a5820a2ebd5bafb4ba4e239b`)

Older revisions:

| Version | SHA-1 | Status |
|---|---|---|
| [v0.3](../standard/wlwl-spec-v0.3(MD5_4308b3d2071ebed5cb52eba61272b1ea).md) | (MD5 `4308b3d2071ebed5cb52eba61272b1ea`) | superseded |
| v0.2.x | (planned) | superseded |
| v0.1 | (planned) | history only |

> **Why MD5 for v0.3 but SHA-1 for v0.4?** The v0.3 file was authored
> before we standardised on SHA-1. The MD5 hash is stable; v0.4 uses
> SHA-1 going forward.

## Compiler vs spec version

The compiler release **does not have to match** the spec version.
Today the compiler is `v0.4.0-rc` implementing `v0.3 + selected v0.4`
work. The CI tracks both `wlwl --version` (compiler) and the spec
SHA in `docs/standard/`.

## Conformance test suite

The Phase H1 conformance suite is a v0.4 follow-up. It will:

- run every program in `examples/` against the patched-up spec
- verify the JSON error output against golden files
- check `wlwl ast` output byte-stability per canonical-form source

Until H1 lands, the bundled `insta` snapshots in
`crates/wlwl-error/src/snapshots/` form a partial conformance
harness for error-schema stability. See
[the Contributing page on snapshot testing](contributing.md#snapshot-testing).

## Spec test helpers

`cargo doc --workspace --no-deps` ensures all docs cross-reference
real symbols (no broken-intra-doc-link warnings).

`cargo deny --locked --all-features check` enforces the licenses of
all 200 transitive dependencies; see [`docs/plan/deviations.md` P4-G2-*](../plan/deviations.md).

## Versioning policy

We follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html):

- **Major** bumps when the spec has a breaking runtime change
- **Minor** bumps for new builtins / new spec sections adopted
- **Patch** for doc-only or internal-only changes

See [`CHANGELOG.md`](../../CHANGELOG.md) for the actual version
history. Phase H plans the v0.4.0 release; that release's tag
will be the first to follow this policy strictly.
