# ADR-0011 — Module Version Solver (MVS) uses Cargo-style resolver, not PubGrub

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-09-19 |
| **Deciders** | Li (project lead) |
| **Related** | spec v0.4 §13.9 (MVS), ADR-006 (lock-file shape), Phase C4 implementation report |

## Context and Problem Statement

WLWL v0.4 §13.9 introduces cross-package modules: a `wlwl.toml` may
declare a `[dependencies]` table referencing other WLWL packages.
Workspace resolution must produce a unique, reproducible `wlwl.lock`.
Two competing approaches:

- **Cargo-style** — single backtrack search over the dep graph,
  preferring the highest compatible version that satisfies all
  constraints; ambiguous ties resolved by "the first solution
  encountered"; outputs a flat lock with version pins.
- **PubGrub** (SwiftPM/Nix/Oak/SPJ-style) — SAT-with-relations
  solver that minimizes a "version-incompatibility" cost function;
  always picks the newest-but-compatible version the SAT
  constraints admit; supports backtracking by incompat-resolution.

## Decision Drivers

- §13.9 example suite: 5 e2e .wl programs covering simple linear
  dep trees, diamond conflicts, and a 3-package cycle (only resolvable
  with backtracking)
- Cross-platform behaviour parity: the implementation must be
  deterministic across Linux/macOS/Windows builds (reproducibility)
- Tooling familiarity: the spec author has worked on Cargo-style
  resolvers before; no PubGrub expertise in the team
- Implementation budget: 1 day

## Considered Options

### A. Cargo-style resolver

Implementation tracks minimal-version state across [dependencies]
table rows. Picks "first fit" while backtracking on conflict. Lock
file structure mirrors `Cargo.lock`:

```toml
[[package]]
name = "wlwl-stdlib"
version = "0.4.1"
checksum = "blake3:..."
```

**Pros**: deterministic; reuse existing `Cargo.lock` parsing code in
the workspace's `Cargo.lock` adjacency checks (just for the
`Cargo.lock` itself, not for resolution algorithm); debuggable
**Cons**: doesn't always pick newest; doesn't produce maximally
informative failure messages

### B. PubGrub

Builds an incompatibility graph; resolves by repeatedly picking the
"best" candidate and reducing the partial solution.

**Pros**: always newest-but-compatible; produces rich error messages
on conflict (provenance-aware)
**Cons**: 1-2 weeks of port-from-OCaml work; tests would need a
separate fixture set; **a v0.4 dependency that PubGrub says "OK"
might be rejected by Cargo convention** — adoption risk

### C. SAT-solver (e.g. z3)

Encode dep tree as SAT. Slow (~ 50 ms per resolution), overkill for
v0.4 sizes (< 100 packages).

**Pros**: provably minimal solution
**Cons**: too heavy for project size; dependency on z3

## Decision Outcome

Chosen option **A** — Cargo-style. Implementation pieces:

- `wlwl-toml/src/mvs.rs` — the deterministic backtracker, modeled
  after Cargo's `crates-io` style with hashed-version first-fit
- Resolution cache (LRU 64 entries) for incremental resolves during
  dev rebuild
- `wlwl lock --explain` shows the resolution path as a tree; `--diff`
  shows the change vs an older lock file
- All test fixtures in `crates/wlwl-toml/src/test_snapshots/` are
  byte-stable across Linux/macOS/Windows, verified by hash equality

### Reproducibility rules

- Always pin to **exact** versions in the lock file (`version = "=x.y.z"`)
- Tie-break rule: when two versions satisfy, **prefer the one with
  the largest semantic-version minor**, breaking ties by **earliest
  manifest path in lexicographic order**. This is documented so
  two contributors with the same `wlwl.toml` produce the same lock.
- E0045 emitted when no solution exists, with the full failed-backtrack
  tree in the diagnostic `related` field (Phase C5 deviation P4-C5-001).

### Cross-check with PubGrub

We do not ignore PubGrub. The Cargo-style solver is the **default**;
a future optional path can layer PubGrub on top if §13.9 v0.5
introduces "newest-only" semantics. For now, "first-fit-and-deterministic"
matches user expectations from Cargo/CocoaPods/NuGet.

## Consequences

Positive:

- §13.9 v0.4 conformance suite passes (5 e2e programs + 9 unit tests)
- ~ 600 lines of pure Rust (`mvs.rs`); no external solver deps
- Lock hashes are content-addressed; tools that rely on lock byte
  equality (CI cache keys, signed build manifests) work without
  federation

Negative:

- "Newest" vs "first-fit" discrepancy with v0.5 plans: when a future
  spec demands "always pick newest with same major", we will need to
  swap to PubGrub. Documented as **future-tech-debt** in spec
  appendix G.
- E0045 diagnostics contain the whole backtrack tree in `related`
  field, which is large (~ 30 entries for a 5-package conflict). The
  AI-facing mode (`wlwl --ai-output`) emits this format; the
  human-facing form is a summarized 3-line message. Phase F7.

## References

- spec v0.4 §13.9 (Module Version Solver)
- ADR-006 (lock-file shape; previous decision basis)
- docs/plan/wlwl-build-plan-v0.2.md §C4 (Phase C plan)
- docs/history/20260918c5-c7.md (Phase C5-C7 implementation)
- Phase E1 alternative: §13.9 could be retro-fitted with PubGrub
  if user demand shifts
