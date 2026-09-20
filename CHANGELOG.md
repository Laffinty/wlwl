# Changelog

All notable changes to WLWL (the implementation) are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

> **Note.** The compiler version is **independent of the language spec version**.
> The language spec lives in `docs/standard/` and is content-addressed by SHA-1.
> This file tracks the **compiler / tooling** releases. The spec is currently at
> **v0.6** (`docs/standard/wlwl-spec-v0.6(SHA1_cdb548cb5161e61d836aad2208fd33adc0917861).md`).

## [Unreleased — v0.6.0]

### Changed (breaking)

The implementation now matches the **v0.6 specification** (the v0.5 spec
was never published; this is the first versioned release). Nine breaking
semantic changes (per the spec's Appendix B) are now in force:

- **A · Truthiness**: `0`, `0.0`, `""`, empty ARRAY, empty DICT, `NaN` are
  now falsy. Previously only `FALSE` and `NULL` were. (Matches Python /
  JS / Ruby conventions; the v0.5 design was deemed too unusual.)
- **B · `&&` / `||` short-circuit**: left operand's truthiness decides
  whether the right is evaluated; ERR on the left propagates without
  touching the right. Previously both sides were always evaluated.
- **C · `IF` consumes `ERR`**: an `ERR` condition routes to the `else`
  branch (or propagates `ERR` if no else). Previously ERR was treated
  as truthy and took the then-branch.
- **D · `!` and `NOT` accepted without warning**: `W0054` (the
  `!`-is-deprecated warning) was removed. Both forms are now equally
  canonical.
- **E · `POP` renamed `AT_K`**: the dict lookup-with-default helper
  is now `AT_K(d, k, default)`. `POP` remains as a back-compat alias
  but emits no warning. `REMOVE_KEY` is the dedicated removal function.
- **F · String subscript read**: `s[i]` returns a single-codepoint
  string; negative indexes count from the end. `s[i] = ...` is
  rejected (`E0030`).
- **G · Explicit `LET MUT`**: mutable bindings must be declared
  `LET MUT(name, value)`. The v0.5 "first closure call upgrades cell
  to mutable" rule was deemed too magical and removed. `SET` on a
  `LET` (immutable) binding raises `E0024`.
- **H · Integer overflow throws `E0035`**: replaced the v0.5
  "saturate to i64::MAX/MIN + emit `W0015`" behavior with an
  explicit error. `E0035` is now shared with float-to-int overflow.
- **J · String interpolation**: `"hi ${name}!"` form, with `\$`,
  `\n`, `\t`, `\\`, `\"`, `\/`, `\0`, `\b`, `\f` escapes. Inner
  expressions are evaluated and `STR`-rendered; an `ERR` inside
  `${...}` propagates without producing partial output.

### Added

- **String interpolation lexer**: 3-token form `StrStart`,
  `StrText(String)`, `StrEnd`; recursive sub-lex for the inner
  expression. The AST adds `Literal::Interpolated(Vec<StrPart>)` and
  a `StrPart` enum (`Text` / `Expr`).
- **`MUT` keyword** (lexer-level contextual): `TokenKind::Mut`,
  consumed by the parser only between `LET` and `(`. Other positions
  treat it as a regular identifier.
- **`BOOL` registered as ERR consumer**: `BOOL(ERR(...))` returns a
  boolean instead of triggering §8.2 transparent propagation.
- **`&&`, `||` registered as ERR consumers**: short-circuit paths
  live in a new `eval_logical_short_circuit` helper, intercepted in
  `eval_call` before the generic ERR-propagation block.
- **`IF` registered as ERR consumer**: documented in the registry
  for completeness; the actual handling lives in `eval_if`.
- **String subscript AST path**: `INDEX_GET` accepts `(String,
  Integer)`; `INDEX_SET` rejects `String` first arg with `E0030`.

### Changed (non-breaking)

- **Cell mutability flag is permanent**: `Binding.mutable` is set
  at binding creation; no "closure-capture upgrade" mechanism
  remains. The dead `Env::upgrade_all_to_mutable` was removed.
- **Error message updated**: `SET` on an immutable binding now
  suggests `LET MUT` in its message.
- **Registry**: `Version::V06` added; `&&`/`||`/`AT_K` added as
  `ResolvedBuiltin`; `POP` demoted to `ResolvedCompat`. Total
  entries: 90 → 93.
- **`BOOL` is now an ERR consumer in the registry**, matching its
  runtime behavior.
- **Removed `W0015`** (integer saturate warning) — now `E0035` on
  overflow.
- **Removed `W0054`** (`!` deprecation) — `!` is canonical.

### Fixed

- **Parser**: `LET MUT(name, value)` now correctly accepts the
  `MUT` keyword between `LET` and `(`. (Earlier draft had the
  syntax as `LET(MUT name, value)`, which was off-spec.)
- **Formatter**: `Literal::Interpolated` segments are rendered
  with proper escape sequences (`escape_str_text` helper); `MUT`
  keyword preserved when re-formatting `LET MUT(...)` expressions.

## [Unreleased — v0.5.0]

### Added

- **wlwl-spec-v0.6** — supersedes v0.5 with the nine breaking
  semantic changes above. The v0.3 and v0.4 specs have been moved
  to the trash (recoverable).

> **Note.** v0.5 was never published — the spec was drafted but not
> tagged, and the implementation never claimed compliance with it.
> The v0.5 spec is in `docs/standard/` was moved to trash on
> 2026-09-20 along with v0.3 and v0.4.

## [Unreleased — v0.4.0]

### Added
