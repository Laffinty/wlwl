# Spec & conformance

The WLWL specification lives in its own repository,
[`Laffinty/wlwl-spec-v1`](https://github.com/Laffinty/wlwl-spec-v1) — the
**v1.0** edition, published as eleven volumes: an overview, the language core,
objects and session types, concurrency, conformance, and six
standard-library volumes. That repository is the authoritative source — spec
revisions are decided there, with a revision process and a register of
differences.

This repository keeps a **merged copy** of those volumes in
[`docs/wlwl-spec-reference.md`](../wlwl-spec-reference.md): one file, so the
machine guards and AI agents can read a chapter without crossing a repository
boundary. It is a secondary carrier — when the two disagree, the spec
repository wins. `git log --follow` is the source of truth for content
changes. Superseded specs are archived under `docs/history/`.

## Current reference file

**[`docs/wlwl-spec-reference.md`](../wlwl-spec-reference.md)** — the merged
v1.0 document: three specification parts plus the agent notes, in this order —

- `## 语言规范 · N` — the language spec: lexis, types and values, evaluation,
  the error model, modules, objects and session types, concurrency, and the
  complexity / memory chapters;
- `## 一致性册 · N` — conformance: definitions, stability policy, the public
  surface, the register of spec-vs-implementation differences, and the
  verification requirements;
- `## 标准库规范 · N` — every namespace member's contract;
- `## 代理工作说明 · N` — notes for AI agents (not normative).

**Why the headings carry a volume prefix.** Every volume numbers its own
sections, so after merging there would be many `## 0`, `## 1`, `## 11` …
headings. The second-level titles are therefore prefixed with the volume name
— you will meet `## 标准库规范 · 11`, not a bare `## 11`. The chapter numbers
themselves did not move: the language chapters are still §1–§19 and the
standard-library chapters still §1–§22, so references that were valid before
are still valid. The two numbering spaces are independent, though — language
§5 and standard library §5 are different sections, so a cross-volume
reference has to name the volume.

The language volume also carries the static-contract layer introduced in
v0.10 (unchanged in v1.0):

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

| Version | Status |
|---|---|
| v1.0 | current — published in the separate `wlwl-spec-v1` repo; merged copy in `docs/wlwl-spec-reference.md` |
| v0.11 | superseded by v1.0 (2026-10-22); the three v0.11 volumes are archived verbatim in `docs/history/20261022-spec-archive.md` |
| v0.10 | superseded by v0.11 (2026-09-30) |
| v0.9 | superseded by v0.10 (2026-09-28) |
| v0.8 | superseded by v0.9 (2026-09-25) |
| v0.7 | superseded by v0.8 (2026-09-23) |
| v0.6 | superseded by v0.7 (2026-09-22) |
| v0.5 | (never published) |
| v0.4 | (SHA-1 `97524ced037b5ef0a5820a2ebd5bafb4ba4e239b`) |
| v0.3 | (MD5 `4308b3d2071ebed5cb52eba61272b1ea`) |
| v0.2.x | (planned) |
| v0.1 | (planned) |

Archived specs v0.6 – v0.10 (with their build plans, deviation ledgers,
reviews and daily logs) are condensed in
[`20260902-09.md`](../history/20260902-09.md) and
[`20260915-22.md`](../history/20260915-22.md); full text via git history.
The v0.11 spec set (language + standard library + agent notes) is archived
verbatim in
[`20261022-spec-archive.md`](../history/20261022-spec-archive.md).

> **v0.5 was never published** — the spec was drafted but not tagged,
> and the implementation never claimed compliance with it. v0.6 was the
> first versioned spec the implementation aligned to; every version since
> only appends.

> **Why MD5 for v0.3 but SHA-1 for v0.4+?** The v0.3 file was authored
> before we standardised on SHA-1. The MD5 hash is stable; v0.4 onwards
> use SHA-1.

## Compiler vs spec version

The compiler release **does not have to match** the spec version.
The current compiler release (**v0.12.1**) targets the **v1.0.4** spec; the
differences that are still open are registered in the conformance volume
(§5) and listed in the release notes. CI tracks
`wlwl --version` (compiler) and spec anchors via
`impl/crates/wlwl-cli/tests/conformance.rs` plus the spec-consistency
lock tests in `crates/wlwl-error` (error-code table) and
`crates/wlwl-eval` (appendix G anchors).

### Appendix A is a frozen snapshot — two counts, not one

The standard library's **appendix A** (in the registry volume of
[`docs/wlwl-spec-reference.md`](../wlwl-spec-reference.md)) is the normative
member registry, and it is a **frozen snapshot**: per the spec's own registry
rules it is **not** regenerated from the implementation and must **not** be
rewritten to chase a compiler change.

That means two different counts are always in play:

- **The implementation surface** — everything the compiler exposes, printed by
  `wlwl namespace` (the output is labelled `"surface": "implementation"`). On
  the current build that is **288 members across 25 namespaces** (23
  `wlwl:std.*` plus the `ai` / `agent` packages).
- **Appendix A's registry** — the spec's public-surface list. Members that
  exist in the compiler but are not yet in appendix A are a **conformance gap**
  recorded through the spec's revision process, **not** extra members.

Use `wlwl namespace` when you are writing code; use appendix A when you are
reasoning about conformance. Mixing the two is the classic way to write a
stale claim into documentation.

### Extensions are outside both lists

A native dynamic library loaded with `wlwl run --ext /abs/path/lib.so` mounts
its own namespaces (`tls`, `crypto`) at runtime. Those members appear in
**neither** count: they are not standard-library members and do not enter
appendix A — they live in the conformance volume's implementation-extension
register. Loading such a library hands the whole process's permissions to it.

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
all transitive dependencies; see the deviation-ledger digests in
[`docs/history/20260915-22.md`](../history/20260915-22.md).

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
