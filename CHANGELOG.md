# Changelog

All notable changes to WLWL (the implementation) are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

> **Note.** The compiler version is **independent of the language spec version**.
> The language spec lives in `docs/standard/` and is content-addressed by MD5.
> This file tracks the **compiler / tooling** releases. The spec is currently at
> **v0.3** (`docs/standard/wlwl-spec-v0.3(MD5_4308b3d2071ebed5cb52eba61272b1ea).md`).

## [Unreleased]

Nothing yet.

## [0.3.0] — 2026-09-03

**First public release.** The compiler implements the language described by the
v0.3 spec across Phases 1–4 plus the post-Phase 4 batch. The package version
is bumped from the internal `0.1.0` to `0.3.0` to match the spec version the
implementation targets.

### Highlights

- **Tree-walking interpreter** for the v0.3 language core (lexer, parser,
  evaluator, error machinery), 223 / 223 tests passing.
- **AI-friendly diagnostics**: 35 stable error codes across 11 categories
  (`E0001` … `E0083`); `errorCategory`, `retryable`, `suggestion_code`,
  `related`, `error_schema_version` (currently `0.3.1`).
- **JSON / JSONL streaming output** (`--format=json` / `--format=jsonl`) for
  machine consumption (per v0.3 §14.7).
- **`wlwl ast <file> --format=json`** subcommand — dumps the AST as JSON for
  AI tools.
- **Standard library (first batches)**:
  - `wlwl:std.io`   — `PRINT`, `INPUT` (§15.1)
  - `wlwl:std.fs`   — `READ_FILE`, `WRITE_FILE`, `EXISTS` (§15.3)
  - `wlwl:std.json` — `PARSE`, `STRINGIFY` (E0070 / E0071)
  - `wlwl:std.ai`   — `ASK` / `EMBED` / `COMPLETE` (mock; reserved E0080–E0083)
- **Modules and packaging**:
  - Cross-directory `IMPORT` (`./`, `../`)
  - `ns:name` third-party namespace registry
  - `wlwl.toml` (`[package]` / `[dependencies]` / `[namespaces]`)
  - `wlwl.lock` (JSON + SHA-256, atomic write)
  - Full E0041 cycle-path reporting per §13.7
- **Type-annotation slots**: per-parameter `name: Type` annotations on `FUN`,
  structured `TypeExpr` payload (`TypeExpr::Ident` / `::Array` / `::Generic`).
  Annotations are **parsed, not type-checked** (v0.3 §2.4 permits this).
- **Auto lock generation**: `wlwl-cli` writes `wlwl.lock` after a successful
  `wlwl run`.

### Subcommands

| Command | Purpose |
|---|---|
| `wlwl run <file>` | parse, name-check, execute |
| `wlwl run <file> --format=json` | emit errors as JSON |
| `wlwl run <file> --format=jsonl` | emit errors as JSONL (AI tools) |
| `wlwl check <file>` | parse + name-check only, no execution |
| `wlwl ast <file> --format=json` | dump the AST as JSON |

### Phases shipped (cumulative, from the build plan)

- **Phase 1 (MVP)** — lexer, parser, evaluator, `wlwl run`, `PRINT` builtin.
- **Phase 2 (core semantics)** — control flow (`IF` / `WHILE` / `FOR` /
  `RETURN` / `BREAK` / `CONTINUE`), `FUN` first-class with closures, error
  handling (`OK` / `ERR` / `PANIC` / `TRY` / `OR_DIE` / `IS_OK` / `IS_ERR`),
  §12.6 transparent ERR propagation, single-directory modules.
- **Phase 3 (AI-friendly)** — 35 stable error codes, `retryable` /
  `suggestion_code` / `related` fields, JSON / JSONL streaming, `wlwl ast`
  subcommand, type-annotation slots (parsed, not checked).
- **Phase 4 batch 1** — `wlwl:std.io` / `wlwl:std.fs` / `wlwl:std.json` and
  `wlwl:` namespace path.
- **Phase 4 batch 2** — cross-directory IMPORT, `ns:name` third-party
  namespace registry, `wlwl.toml` / `wlwl.lock`.
- **Phase 4 batch 3** — `wlwl:std.ai` (mock) + CLI lock generation.
- **post-Phase 4** — per-parameter type annotations on `FUN` + structured
  `TypeExpr`.

### Known gaps (deferred, not regressions)

These are tracked in `docs/plan/deviations.md` and are explicitly **not**
considered bugs for 0.3.0:

- `P3-008` — per-site `suggestion_code` codegen (the schema is in place; the
  per-error-site generation is the follow-up).
- `P3-009` — formal coverage measurement (no `cargo tarpaulin` run yet; the
  test count is high but the percentage is unmeasured).
- **Phase 5 (Coq formalization of §19)** — optional per build plan §3.
- **Performance** — tail-call optimization and hot-inline are deferred past
  Phase 4.
- **D005** — default values and `*rest` parameters on `FUN` are not
  implemented (spec §8.2 marks them as v0.1 syntax, not v0.3 core).
- **D006** — closure captured-state is independent (deep-copied via
  `Env::clone`); mutable shared capture would need `Rc<RefCell<Env>>`.

### Compatibility

- Compiler tracks the **v0.3** language spec
  (`docs/standard/wlwl-spec-v0.3(MD5_4308b3d2071ebed5cb52eba61272b1ea).md`).
- Source compatibility with v0.2 programs is **not** guaranteed; the
  `AS(name, alias)` function form was removed in favor of import-time
  renaming (per spec §13.4).
- A `.wl` program you write is your own work; it is **not** affected by the
  compiler's GPL v2 license.

## Earlier versions (pre-public, internal numbering)

The 0.3.0 release is the first tagged public version. Earlier work was
tracked by phase rather than by SemVer; see
[`docs/plan/wlwl-build-plan-v0.1.md`](docs/plan/wlwl-build-plan-v0.1.md) for
the full phase history and [`docs/plan/deviations.md`](docs/plan/deviations.md)
for the per-phase deviation log.

[Unreleased]: https://github.com/Laffinty/wlwl/compare/v0.3.0...HEAD
[0.3.0]: https://github.com/Laffinty/wlwl/releases/tag/v0.3.0