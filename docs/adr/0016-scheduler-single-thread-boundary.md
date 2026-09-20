# ADR-0016 — v0.7 scheduler keeps single-threaded boundary

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-09-21 |
| **Deciders** | Li (project lead) |
| **Related** | spec v0.7 §17.1.1, ADR-0014 (structured concurrency + channels), ADR-0015 (Brown 9-dim decisions), wlwl-build-plan-v0.7 §5.1 / §10 D11 |

## Context and Problem Statement

WLWL v0.6's evaluator is a single-threaded tree-walking interpreter. The
runtime environment is held in `Rc<RefCell<Env>>` — `Rc` is `!Send`, which
prevents moving `Env` across thread boundaries without an owner-side
rewrite.

v0.7 introduces a scheduler (ADR-0014) that needs to manage multiple
cooperative tasks. The question is whether the scheduler should be
single-threaded (v0.6 baseline kept) or whether v0.7 should rewrite the
environment to `Arc<RwLock<Env>>` and run a thread pool.

## Decision Drivers

- v0.6 baseline uses `Rc<RefCell<Env>>`, which is `!Send` and entangled
  with every entry point of `wlwl-eval`.
- OS-threaded full-stack rewrite cost: 16-24 person-weeks per plan §2.4.
- v0.7 target: feature parity + structured concurrency; not a from-scratch
  rewrite.
- Brown 9-dimension Persistence = Transient (ADR-0015) only makes sense
  on a single-threaded cooperative scheduler — multi-threaded would
  benefit from Persistent interrupts, undoing that choice.
- v0.8 already discusses thread-pool + reference capabilities (Arvidsson
  et al. OOPSLA 2023) as a future option.

## Considered Options

### A. Single-threaded cooperative scheduler (chosen)

Tasks run on the same OS thread; yield/resume via the scheduler. No
`Send`/`Sync` constraints introduced. v0.6's `Rc<RefCell<Env>>` continues
to work unchanged.

**Pros:**

- Avoids 16-24 person-week rewrite of the entire eval layer.
- Performance budget (single-task regression < 10 %) is achievable.
- Local change: minimal blast radius across the workspace.
- `Transient` cancellation (ADR-0015) is naturally correct here.
- All v0.6 fixtures pass through unchanged via the fidelity gate
  (Phase B3).

**Cons:**

- CPU-bound tasks don't parallelise; serialised onto the one thread.
- I/O-bound workloads have a throughput ceiling (acknowledged in plan
  §1.3 risk R4 and §10 D20).
- Anyone wanting true parallelism must wait for v0.8.

### B. Multi-threaded with thread pool + `Arc<RwLock<Env>>`

Full rewrite: every `Rc<RefCell<Env>>` becomes `Arc<RwLock<Env>>`, every
internal API gains `Send + Sync` bounds, a thread pool drives N
evaluator threads.

**Pros:** True parallelism; CPU-bound tasks scale.

**Cons:** 16-24 person-weeks of mechanical work across the entire
`wlwl-eval` surface. High regression risk on v0.6 baseline invariants.
Conflicts with the v0.7 plan-impl-spec iterative mode (plan §8.1).

### C. Hybrid: single-threaded v0.7, threaded v0.8

Same as A for v0.7, but design the scheduler API so a thread-pool swap
in v0.8 is non-invasive.

**Pros:** Captures A's wins; keeps the door open for B later.

**Cons:** Adds design constraint that A doesn't yet need; speculative
work. YAGNI argument: design the abstraction when v0.8 starts, not
before.

## Decision Outcome

Chosen option **A**.

- `wlwl-eval`'s scheduler runs on the single OS thread that drives
  `Evaluator::run`.
- No `Send` / `Sync` bounds are added to `Env` or any `wlwl-eval`
  internal type.
- `Rc<RefCell<Env>>` continues as the environment sharing primitive.
- v0.8 may revisit per the v0.2/v0.7 foreshadow ("evaluating thread
  pool + reference capabilities in v0.8"); that work is out of scope
  for v0.7.

**Performance budget (D20):**

- Single-task path: < 10 % regression vs v0.6 (Phase G5 bench).
- Concurrent path: correctness + leak-freeness only. No throughput /
  latency / fairness promises (plan §6.4.1 + §10 D20).

### Explicit non-rules

- No `Send` / `Sync` constraints added in v0.7.
- No thread pool introduced.
- No `Arc<RwLock<Env>>` rewrite.
- No atomic-based environment sharing.

## Consequences

**Positive:**

- Engineering cost stays bounded: scheduler is a local addition, not a
  rewrite.
- v0.6 baseline invariants protected: ≥ 90 % line coverage, 56 error
  codes, 28 builtins — fidelity test in Phase B3.
- Performance budget is feasible without heroic optimisation.
- `Transient` cancellation (ADR-0015) is naturally correct without
  cross-thread synchronisation.

**Negative:**

- CPU-bound workloads don't scale (deferred to v0.8 — explicit, not a
  bug).
- I/O-bound throughput ceiling acknowledged in plan §1.3 risk R4.
- Anyone reading the code may mistake single-threadedness for an
  oversight; the boundary must be documented (this ADR + spec §17.7).

## References

- `docs/plan/wlwl-build-plan-v0.7.md` §1.3 (R4) / §5.1 / §6.4 / §7 / §10 D11 / §10 D19 / §附录 B ADR-0016
- `docs/adr/0014-structured-concurrency-v0.7.md`
- `docs/adr/0015-brown-9-dimension-decision.md`
- v0.6 evaluator baseline (`Rc<RefCell<Env>>` in `wlwl-eval`)
- Arvidsson et al. OOPSLA 2023 *Reference capabilities for flexible memory management* — long-term reference for v0.8 thread pool design
- Trio / Greenlet model — single-threaded cooperative precedents