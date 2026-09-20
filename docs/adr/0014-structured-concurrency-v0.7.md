# ADR-0014 — Structured concurrency + channels adopted for v0.7

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-09-21 |
| **Deciders** | Li (project lead) |
| **Related** | spec v0.7 §17, wlwl-build-plan-v0.7 §2 / §附录 B, ADR-0015 (Brown 9-dim decision), ADR-0016 (single-thread scheduler boundary) |

## Context and Problem Statement

WLWL v0.6 is fully synchronous: every primitive in the spec returns to its
caller, and program paths can be traced statically through source text.
v0.7 introduces time as a control-flow dimension — the first primitives that
let `.wll` programs express structured concurrency.

The choice is which concurrency model WLWL will adopt. Candidates (per plan
§2.1):

1. **Structured concurrency + channels** (Trio / Kotlin `coroutineScope` /
   Swift `TaskGroup` / Go cff consensus)
2. Async/await coroutines (Rust Tokio / Smol style)
3. Actor (Erlang / BoC / Orca)
4. Reactive Async (Haller, cell-based)
5. Lock + shared memory
6. OS threads

## Decision Drivers

- **Philosophical fit with WLWL.** Functional + error-as-value + explicit
  scoping. Structured concurrency treats scope as the unit of lifetime and
  error as the unit of failure — both concepts already exist in WLWL.
- **Engineering cost on a single-threaded baseline.** v0.6's evaluator is
  tree-walking with `Rc<RefCell<Env>>`. Local change (cooperative scheduler
  on top of the existing interpreter) vs. full rewrite (OS threads with
  `Arc<RwLock<Env>>`).
- **Industrial consensus.** Brown et al. 2025 (*A Design Space Exploration
  of Async/Await*) observed that the structured-concurrency camp selects a
  near-identical subset of the 9-dimension space; Trio, Kotlin, Swift and
  Go cff all converge there.
- **No free-floating tasks.** No equivalent of Python's
  `asyncio.create_task()` or Java daemon threads. Every task's lifetime is
  bounded by a lexically-visible scope.
- **Cancellation as data, not as exception.** Reuse v0.6 §8.2 ERR
  transparent propagation rather than introduce a new cancellation exception
  hierarchy.

## Considered Options

### A. Structured concurrency + channels (chosen)

`SCOPE(fn)` opens a structured scope; `SPAWN(fn)` produces a child task
whose lifetime ends at scope exit; `AWAIT(handle)` waits for the child;
`CHANNEL_*` are the only communication primitive. Top-down cancellation;
first ERR aggregates.

**Pros:**

- Aligns with WLWL philosophy (functional + error-as-value + explicit scope).
- Engineering cost is local: tree-walking evaluator → generator-based stack
  machine (Phase B1-B5).
- Industry consensus (Trio / Kotlin / Swift / cff).
- Free-floating tasks structurally impossible.

**Cons:**

- Cooperative single-threaded scheduler means CPU-bound tasks don't
  parallelise (deferred to v0.8 — see ADR-0016).
- Tree-walking → generator rewrite is non-trivial (3-5 person-weeks per
  plan §2.4).
- Surface-area growth: 15-17 new builtins + 6 new error codes.

### B. Async/await coroutines (Tokio / Smol style)

Eager futures, fire-and-forget permitted, lifetime independent of scope.

**Pros:** Familiar to Rust developers; ergonomic for I/O-bound workloads.

**Cons:** Free-floating tasks permitted → leak surface; Brown 2025 shows
runtime incompatibility (one program, four different outputs across 7
runtimes); non-structured cancellation requires explicit try/catch at every
spawn.

### C. Actor (Erlang / BoC / Orca)

Isolated-process model with message passing; isolation enforced by the
type system.

**Pros:** Strong isolation guarantees; proven at scale (Erlang/OTP).

**Cons:** Requires introducing an isolation type system into WLWL —
directly contradicts the "dynamic types + explicit mutability" stance.
Engineering cost: 12-16 person-weeks per plan §2.4. Deferred to a future
version per plan §2.1.

### D. Reactive Async (Haller, cell-based)

Cell-based asynchronous event model (Scala).

**Pros:** Mathematical elegance.

**Cons:** Limited expressive power; outside mainstream.

### E. Lock + shared memory

Threads + `Arc<Mutex<...>>`.

**Pros:** Familiar model.

**Cons:** Directly conflicts with WLWL's "functional + error-as-value"
stance; race conditions are a class of bug WLWL is explicitly avoiding.

### F. OS threads

Native threads + `Send`/`Sync`.

**Pros:** True parallelism.

**Cons:** Full evaluator rewrite (`Rc<RefCell<Env>>` →
`Arc<RwLock<Env>>`); 16-24 person-weeks per plan §2.4; v0.6 baseline
entanglement; not viable in v0.7 scope.

## Decision Outcome

Chosen option **A**. Concretely:

- `wlwl-eval` gains four modules: `runtime.rs` (scheduler + generator-based
  stack machine), `task.rs`, `channel.rs`, `cancellation.rs`.
- 15-17 new builtins: `SCOPE` / `SPAWN` / `AWAIT` / `YIELD` / `SHIELD` /
  `CHANNEL_*` / `TASK_*`.
- 6 new error codes: E0050-E0056.
- Brown 9-dimension space resolved explicitly per ADR-0015.
- Single-thread boundary per ADR-0016.
- `wlwl:std.concurrency` high-level combinators shipped as the user-facing
  layer.

**Migration guidance:**

- No v0.6 source change required. Concurrency is opt-in via `SCOPE` /
  `SPAWN` / `AWAIT`.
- v0.6 conformance fixtures must pass unchanged under the new scheduler
  (Phase B3 fidelity gate).

### Explicit non-rules

- No `Promise` / `Future` types exposed to `.wll` programs.
- No `asyncio.run`-style implicit top-level scope (D17 in plan §10).
- No `Thread` / `Task::spawn` (Tokio-style) — every task is structured.
- No cancel-throwing exception type — cancellation rides on the existing
  ERR channel (D12 in plan §10).

## Consequences

**Positive:**

- v0.6 baseline invariants (≥ 90 % line coverage, 56 error codes, 28
  builtins) preserved; fidelity test in Phase B3 enforces it.
- WLWL's existing philosophy (functional, error-as-value, explicit
  scoping) carries through.
- Single-source-of-truth for cancellation: ERR.
- Future migration to an OS-threaded scheduler (v0.8+) becomes a scheduler
  swap, not a language rewrite.

**Negative:**

- Cooperative single-thread model: I/O-bound throughput ceiling
  acknowledged in plan §1.3 risk R4 and §10 D20. Performance budget is
  *"single-task regression < 10 %; concurrent path guarantees correctness
  + leak-freeness only"* — no throughput / latency promises.
- Brown 9-dimension Persistence choice (Transient — ADR-0015) diverges
  from Trio; documented divergence but may invite community friction (plan
  §9 R6).
- Tree-walking evaluator → generator-based stack machine is the largest
  single engineering cost (3-5 person-weeks; Phase B1-B5).

## References

- `docs/plan/wlwl-build-plan-v0.7.md` §2.1 / §2.2 / §2.4 / §10 D1 / §附录 B
- `docs/adr/0015-brown-9-dimension-decision.md`
- `docs/adr/0016-scheduler-single-thread-boundary.md`
- Brown et al. 2025 *A Design Space Exploration of Async/Await*
- Trio Nurseries, Kotlin `coroutineScope` / `supervisorScope`,
  Swift `withTaskGroup`, Go cff
- JEP 525 (2025) *Structured Concurrency* — Java