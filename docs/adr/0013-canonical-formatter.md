# ADR-0013 — Canonical formatter is required, not optional (Phase E2)

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-09-19 |
| **Deciders** | Li (project lead) |
| **Related** | spec v0.4 §16.3 (canonical formatter), ADR-008 (closure cell semantics), Phase E2 implementation report |

## Context and Problem Statement

Spec v0.4 §16.3 mandates a **canonical formatter** — a tool that
re-formats any `.wl` source to a unique whitespace, indent, and
brace convention. The 10 rules in §16.3 are mechanically derivable
and idempotent (`fmt(fmt(x)) == fmt(x)`).

The question is whether canonical formatting is:

1. **Mandatory** — every v0.4 program must pass through the canonical
   formatter before distribution (release.yml forces a `wlwl fmt
   --check` on each .wl in the distribution tar)
2. **Advisory** — format on demand; respect author style
3. **Hybrid** — runner uses canonical form internally; `.wl`
   sources can be either form

Plan §16.3.3 says "all release builds must run through canonical
formatter" and §0.1 Decision #4 reserves a phase for it (Phase E2).

## Decision Drivers

- §16.3 conformance: 12 + 5 idempotency test cases
- AI workflow: AI tools that re-emit source need a deterministic
  surface so training data has stable whitespace
- Programmer ergonomics: full canonicalization means author style
  is replaced; some teams object
- The std-formatter doc (spec §16.3.1) lists 10 explicit rules,
  each finitely computable — no human judgment required

## Considered Options

### A. Canonical format mandatory (chosen)

`wlwl fmt` is shipped with the workspace; `release.yml` runs it as
a step. Programs in distribution must be in canonical form. The
canonical output **is** the "official" surface; any deviation in
style is a build error.

**Pros**: stable surface for AI tools; if/when AI emits `x = 1`
with 4 spaces instead of 2, CI rejects and AI corrects;
spec-accurate
**Cons**: programmer freedom reduced; some teams' conventions (e.g.
tabs vs spaces, brace position) cannot be honored

### B. Author style preserved

`wlwl fmt --check` returns a non-zero exit only if syntax-invalid;
style is unchanged. Author decides.

**Pros**: maximum programmer freedom
**Cons**: defeats the purpose; AI models drift; §16.3 unusable

### C. Hybrid — internal canonical, source author's style

Run canonical format internally; keep author's source style on disk.
Custom flag `--emit-canonical` forces the canonical form on output.

**Pros**: doesn't break author habits
**Cons**: defeats §16.3 — "the canonical form is what tools should
consume"; AI tools must still parse author-source

## Decision Outcome

Chosen option **A**. Implementation pieces:

- **Canonical formatter rule set** (`crates/wlwl-formatter/src/lib.rs`)
  — implements the 10 rules from §16.3 as a canonical-AST walk.
  Idempotency verified by `wlwl-fmt-self-check`: re-formatting the
  formatted form yields the same bytes.
- **`wlwl fmt`** (`crates/wlwl-formatter/../bin/wlwl-fmt.rs`) —
  CLI subcommand that reads a file or stdin, writes canonical form
  to stdout. Adds `--check` (default exit: 1 if canonical-input
  doesn't match canonical-output).
- **CI gate** (`release.yml` Phase H) — every .wl file in the
  distribution tar is run through `wlwl fmt --check`. If the
  output differs, the build fails.
- **Local opt-in** — developers can run `wlwl fmt` manually before
  committing; the formatter is idempotent so no harm in running it
  on already-formatted files.
- **AST-stable node ID** (ADR-008 + Phase E3) — combined with
  canonical form, every source location maps to a deterministic
  doc-id; AI tools consume the doc-id directly.

### Migration guidance (in `docs/CHANGELOG.md`)

- v0.3 sources without canonical formatting are run through
  `wlwl fmt --write` once at upgrade time.
- v0.3-style editors that inject non-canonical whitespace are
  prohibited; a pre-commit hook can call `wlwl fmt --check`.

### Explicit non-rules

- The 10 rules do NOT cover: comment style, ordering of `use`
  statements across crates, docstring re-flow. Those are
  semantics-preserving but not part of the canonical format.
- Docstrings pass through unchanged inside their delimiters.
- Trailing whitespace within strings is preserved verbatim.

## Consequences

Positive:

- §16.3 conformance: 12 + 5 idempotency tests pass (Phase E2
  report); 100 % canonical on workspace's `examples/*.wl` after
  one formatting pass
- AI / IDE tooling stable: same input always → same canonical bytes
- AST-stable node ID combined with canonical form gives a
  doc-id hashing surface that survives `git blame` and
  formatter re-runs

Negative:

- Author freedom reduced on whitespace
- Migration burden: existing v0.3 source must be canonicalized
- 12 weeks of codebase churn: every existing `examples/*.wl` was
  re-formatted once in Phase E2 (no semantic change; verified by
  acceptance tests)

## References

- spec v0.4 §16.3 (canonical formatter)
- docs/plan/wlwl-build-plan-v0.2.md §E2 + §16.3 (formality rules)
- docs/history/20260919e2-e4.md (Phase E2-E4 implementation report)
- `crates/wlwl-formatter/src/lib.rs` — rule implementations
- `tools/canon-idempotency-test` — round-trip verification
