# Changelog

All notable changes to WLWL (the implementation) are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

> **Note.** The compiler version is **independent of the language spec version**.
> The language spec lives in `docs/standard/` and is content-addressed by SHA-1.
> This file tracks the **compiler / tooling** releases. The spec is currently at
> **v0.4** (`docs/standard/wlwl-spec-v0.4(SHA1_97524ced037b5ef0a5820a2ebd5bafb4ba4e239b).md`).

## [Unreleased — v0.4.0]

### Added

- **Phase A1–A8** — closure cell semantics (Phase A2), error schema 1.1.0
  with `trace` / `cause` (Phase A1d), `LET` patterns (A3), `MATCH` (A4),
  cell-upgrade in `LET` (A5), ERR consumer registry (A6), cause-chain
  `WRAP` / `UNWRAP` / `ERR_PAYLOAD` (A7), 47 + error codes (A8).
- **Phase B1–B15** — INDEX_GET/SET/AT/REMOVE_KEY/POP (B1), DEL alias
  (B2), OR_DIE canonicalization (B3), UNWRAP/ERR_PAYLOAD/WRAP (B4),
  FORMAT global builtin + `wlwl:std.format` (B5),
  `wlwl:std.collection` 17 higher-order functions (B6),
  `wlwl:std.test` ASSERT/EXPECT_* framework (B7), 11 string builtins
  (B8), NOT (B9), PRINT_ERR to stderr (B10), appendix G registry
  (B11), ARRAY/STRING/DICT ops global (B12-B14), INPUT/BOOL/CALL/NEG
  + OOP-stub (B15).
- **Phase C1–C7** — AS keyword removal (C1), MODULE_REF real impl
  (C2), `language_version` + E0044 (C3), MVS Cargo-style + E0045 (C4),
  `allow_builtin_shadow` + E0025/W0030 (C5), `wlwl.lock` consistency
  + E0042 (C6), project-root boundary + E0040 (C7).
- **Phase D1–D5** — real-ai HTTP bridge (D1), `wlwl:std.agent`
  (D3), W0052 (D5), E0090–E0094 network ladder (D4), ai/agent
  module coverage.
- **Phase E1–E4** — `strict_types` runtime check (E1),
  canonical formatter + `wlwl fmt` (E2), AST-stable node ID + SHA-256
  hash (E3), W-code unified channel (E4).
- **Phase G1–G12 quality gates** — clippy zero-warning + fmt gate
  (G1), cargo-deny license+advisory gate (G2), rustdoc validity
  gate (G3), 6 ADRs ADR-0008..0013 (G4), 0-unsafe miri scaffolding (G5),
  cargo-fuzz harness for lexer/parser/eval (G6), insta snapshot
  meta-coverage (G7), `cargo bench` smoke (G8), weekly cargo-audit
  supply-chain workflow (G9), CycloneDX SBOM + cosign keyless
  signature per release (G10).

### Changed

- workspace license `GPL-2.0` → canonical SPDX `GPL-2.0-only`.
- AS keyword removed entirely (Lexer drops `TokenKind::As`,
  parser emits E0011 with migration note).
- closure capture is now cell-based (ADR-0008), O(1) `Rc::clone`
  per call.
- error schema bumped to 1.1.0 — every diagnostic now carries
  `trace`, `cause`, `related`, `retry_after` per spec §14.2.

### Removed

- `AS` keyword (v0.3 §4.2.1) — use INT(x)/FLOAT(x)/STR(x)/BOOL(x).

## [v0.3.0] — initial release

### Added

- Lexer, parser, eval (tree-walking interpreter).
- 35 error codes + 10 warning codes.
- Stdlib v0.3 (`std.io`, `std.fs`, `std.json`, `std.format`, `std.ai`, `std.agent`).
- Per-site `suggestion_code` codegen (P3-008).
- Formal coverage instrumentation (P3-009).

### Notes

- See `docs/history/20260902.md` through `docs/history/20260919g89.md`
  for per-phase implementation reports (G2 already pushed; G3-G10 in flight).
- 5 architecture decisions captured in `docs/adr/0008..0013.md`.
- 5 deviations registered: P4-G3-001, P4-G3-002, P4-G5-001, P4-G6-001,
  P4-G7-001, P4-G8-001, P4-G9-001, P4-G2-007..010. See `docs/plan/deviations.md`.

[Unreleased]: #compare-v0.3.0...HEAD
[v0.3.0]: https://github.com/Laffinty/wlwl/releases/tag/v0.3.0
