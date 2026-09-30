# Spec & conformance

The current WLWL language spec lives in `docs/standard/`.
Specs are identified by **version + name** (content-address filenames
were dropped); `git log --follow` is the source of truth for content
changes. Superseded specs are archived under `docs/history/`.

## Current spec

**[v0.11](../standard/wlwl-spec-v0.11.md)** — the "v2 cleaned" edition of
the v0.10 spec, renumbered with **zero semantic change**; everything
introduced in v0.10 carries over:

- the `Type` annotation grammar enters the spec for the first time
  (Appendix A) — until v0.9 it existed only in the implementation;
- a compile-time static-contract layer (`[features] gradual_typing`,
  default `off`; §2.6 / §5.2.1), orthogonal to the runtime
  `strict_types`;
- module contracts: sidecar signature files (`<module>.wll.sig`) and the
  `SEALED([...])` public-surface declaration (§9.1 / §9.6, none by
  default);
- `MATCH` exhaustiveness + unreachable-clause diagnostics
  (`[features] match_exhaustiveness`, defaults to `gradual_typing`;
  §7.4);
- bounded type variables `T: Comparable`, erased at runtime (§5.2.1).

New diagnostic codes `E0110`–`E0116` / `W0110`–`W0117` are emitted
**only at compile time**; `W0117` is always a warning and `E0117` is
deliberately not a code (see the spec's §7.4 / §11.2).

## Archived revisions

| Version | Location | Status |
|---|---|---|
| v0.10 | [`docs/history/wlwl-spec-v0.10.md`](../history/wlwl-spec-v0.10.md) | superseded by v0.11 (2026-09-30) |
| v0.9 | [`docs/history/wlwl-spec-v0.9.md`](../history/wlwl-spec-v0.9.md) | superseded by v0.10 (2026-09-28) |
| v0.8 | [`docs/history/wlwl-spec-v0.8.md`](../history/wlwl-spec-v0.8.md) | superseded by v0.9 (2026-09-25) |
| v0.7 | [`docs/history/wlwl-spec-v0.7.md`](../history/wlwl-spec-v0.7.md) | superseded by v0.8 (2026-09-23) |
| v0.6 | [`docs/history/wlwl-spec-v0.6.md`](../history/wlwl-spec-v0.6.md) | superseded by v0.7 (2026-09-22) |
| v0.5 | (never published) | superseded |
| v0.4 | (SHA-1 `97524ced037b5ef0a5820a2ebd5bafb4ba4e239b`) | superseded |
| v0.3 | (MD5 `4308b3d2071ebed5cb52eba61272b1ea`) | superseded |
| v0.2.x | (planned) | superseded |
| v0.1 | (planned) | history only |

> **v0.5 was never published** — the spec was drafted but not tagged,
> and the implementation never claimed compliance with it. v0.6 was the
> first versioned spec the implementation aligned to; every version since
> only appends.

> **Why MD5 for v0.3 but SHA-1 for v0.4+?** The v0.3 file was authored
> before we standardised on SHA-1. The MD5 hash is stable; v0.4 onwards
> use SHA-1.

## Compiler vs spec version

The compiler release **does not have to match** the spec version.
The current compiler implements the **v0.11 spec** in full. CI tracks
`wlwl --version` (compiler) and spec anchors via
`impl/crates/wlwl-cli/tests/conformance.rs` plus the spec-consistency
lock tests in `crates/wlwl-error` (error-code table) and
`crates/wlwl-eval` (appendix G anchors).

## Conformance test suite

- `cargo test -p wlwl-cli --test conformance` — the AST golden
  snapshots and the conformance fixtures.
- `cargo test -p wlwl-cli --test lsp_smoke` — the language-server thin
  shell over a real subprocess (framing, handshake, clean exit).
- `cargo test -p wlwl-formatter --test formatter_tests` — verifies
  that every example in `impl/examples/` round-trips through
  `wlwl fmt` (canonical-form re-emission must re-parse cleanly).
- `cargo test --workspace` — full suite (1728 tests across 10 crates as
  of v0.10 Step 12).

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

`cargo run --bin gen-appendix-g` regenerates `docs/appendix_G.md`
(appendix G's impl-view mirror) from the builtin registry; the
`appendix_g_anchors_match_v07_section_numbers` lock test in
`crates/wlwl-eval` guards the anchors, and the spec's own normative
appendix G stays hand-maintained.

## Versioning policy

We follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html):

- **Major** bumps when the spec has a breaking runtime change
- **Minor** bumps for new builtins / new spec sections adopted
- **Patch** for doc-only or internal-only changes

See [`CHANGELOG.md`](../../CHANGELOG.md) for the actual version
history. v0.6 is the first release where this policy is applied
strictly.
