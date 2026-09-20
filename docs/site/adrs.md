# Architecture decisions (ADR catalog)

Nine architecture decisions have been captured. ADRs 0008–0013 cover
the v0.4 language/runtime spec; ADRs 0014–0016 cover the v0.7
structured-concurrency design. Each ADR follows the MADR template:
status, context, decision drivers, considered options, decision
outcome, consequences.

## Index

| ADR | Title | Spec | Status |
|---|---|---|---|
| [ADR-0008](https://github.com/Laffinty/wlwl/blob/main/docs/adr/0008-closure-cell-upgrade.md) | Closure capture semantics: cell-based upgrade | §6.4 | Accepted |
| [ADR-0009](https://github.com/Laffinty/wlwl/blob/main/docs/adr/0009-err-consumer-registry.md) | `ERR` consumer registry replaces closed allow-list | §12.7 | Accepted |
| [ADR-0010](https://github.com/Laffinty/wlwl/blob/main/docs/adr/0010-strict-types-behavior.md) | `strict_types` runtime check via transient cast insertion | §2.7 | Accepted |
| [ADR-0011](https://github.com/Laffinty/wlwl/blob/main/docs/adr/0011-mvs-dependency-resolution.md) | Module Version Solver uses Cargo-style resolver | §13.9 | Accepted |
| [ADR-0012](https://github.com/Laffinty/wlwl/blob/main/docs/adr/0012-as-function-removal.md) | `AS` keyword removed entirely | §13.4 | Accepted |
| [ADR-0013](https://github.com/Laffinty/wlwl/blob/main/docs/adr/0013-canonical-formatter.md) | Canonical formatter is mandatory | §16.3 | Accepted |
| [ADR-0014](https://github.com/Laffinty/wlwl/blob/wip0.7/docs/adr/0014-structured-concurrency-v0.7.md) | Structured concurrency + channels adopted for v0.7 | v0.7 §17 | Accepted |
| [ADR-0015](https://github.com/Laffinty/wlwl/blob/wip0.7/docs/adr/0015-brown-9-dimension-decision.md) | Brown 9-dimension async/await design space resolved for v0.7 | v0.7 §17.1 | Accepted |
| [ADR-0016](https://github.com/Laffinty/wlwl/blob/wip0.7/docs/adr/0016-scheduler-single-thread-boundary.md) | v0.7 scheduler keeps single-threaded boundary | v0.7 §17.1.1 | Accepted |

## How to read them

Each ADR lists:

1. **Context and Problem Statement** — what we were solving.
2. **Decision Drivers** — what constraints the decision must honor.
3. **Considered Options** — alternatives we weighed (typically 3).
4. **Decision Outcome** — what we picked, and why.
5. **Consequences** — positive AND negative effects.
6. **References** — links to spec sections, code paths, related reports.

## How to add a new ADR

1. Pick the next number (`0014` after the current six).
2. Copy the MADR template used by `docs/adr/0008-closure-cell-upgrade.md`.
3. Write at least: `Context`, `Decision Drivers` (≥ 3 bullets),
   `Considered Options` (≥ 2 alternatives), `Decision Outcome`,
   `Consequences` (positive + negative), `References`.
4. Open a PR. Add `P4-NNNN-001` to `docs/plan/deviations.md` if
   the ADR supersedes a plan-level assumption.

## Why MADR

MADR ([https://adr.github.io/madr/](https://adr.github.io/madr/)) is
the de-facto standard ADR template in the Rust ecosystem. It keeps
the structure flat and the prose compact — three pages of ADR is
a healthy upper bound; longer ADRs usually want to be a design
spec instead.
