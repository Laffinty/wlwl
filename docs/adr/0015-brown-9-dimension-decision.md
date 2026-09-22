# ADR-0015 — Brown 9-dimension async/await design space resolved for v0.7

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-09-21 |
| **Deciders** | Li (project lead) |
| **Related** | spec v0.7 §17.1, ADR-0014 (structured concurrency + channels), wlwl-build-plan-v0.7 §2.3 / §10 D2-D10 |

## Context and Problem Statement

Brown et al. 2025 (*A Design Space Exploration of Async/Await*) identify
9 independent design dimensions along which async/await runtimes diverge:

1. **Eagerness** — Lazy vs Eager
2. **Suspension** — Static vs Dynamic
3. **Extent** — Indefinite vs Dynamic
4. **Reference Strength** — Strong vs Weak
5. **Destruction** — Awaited vs Cancelled vs Terminated
6. **Propagation** — Destructive vs Never
7. **Awareness** — Aware vs Unaware
8. **Direction** — Top-Down / Bottom-Up / Simultaneous
9. **Persistence** — Persistent vs Transient

Across the 7 mainstream runtimes analysed (Python Asyncio/Trio, Rust
Tokio/Smol, C#, JavaScript, Swift) no two runtimes pick the same point in
all 9 dimensions; a single async program can yield 4 different outputs
depending on runtime.

WLWL v0.7 must declare explicit choices on each dimension. This ADR
captures those choices and the rationale; the runtime must conform to
them.

## Decision Drivers

- Must align with ADR-0014 (structured concurrency + channels).
- Must avoid free-floating tasks (D17).
- Must reuse v0.6 §8.2 ERR transparent propagation (D12).
- Must remain cooperative single-threaded per ADR-0016.
- Must let users write cooperative cancellation checkpoints explicitly
  (D8 Aware).
- Must prevent zombie tasks (D5 Strong).

## Considered Options (per dimension)

For each dimension the plan §2.3 + §10 D2-D10 enumerates the options.
The chosen points are listed below; for Persistence the rationale is
non-trivial and gets a dedicated subsection.

| Dimension | Choice | Notes |
|---|---|---|
| Eagerness | **Lazy** | Trio / Kotlin / cff all pick Lazy; consistent with WLWL's "everything is an expression". |
| Suspension | **Dynamic** | 5/7 runtimes pick Dynamic; fidelity > simplicity. |
| Extent | **Dynamic** | Structured-concurrency invariant: lifetime is the scope. |
| Reference Strength | **Strong** | Trio / Kotlin / Swift all pick Strong; prevents zombie task. |
| Destruction | **Awaited** | Trio + Kotlin align. Swift's `Cancelled` and Tokio's `Cancelled` rejected because they don't reuse ERR. |
| Propagation | **Destructive** | Trio / Kotlin / Swift all pick Destructive; reuses v0.6 §8.2 ERR. |
| Awareness | **Aware** | Trio / Kotlin / Swift / Asyncio all Aware; lets users call `TASK_IS_CANCELLED()` at checkpoints. |
| Direction | **Top-Down** | Trio / Kotlin / Tokio / Smol all Top-Down. |
| Persistence | **Transient** | See dedicated subsection below. |

### Persistence — Transient (intentional divergence from Trio)

Trio picks **Persistent** (every long-running op checks cancellation
automatically); Kotlin / cff pick **Transient** (cancellation rides on
explicit checkpoints). WLWL v0.7 picks Transient. Three concrete reasons:

1. **Explicit-mutability philosophy.** WLWL's state changes are always
   explicit — `LET MUT` + `SET`. There is no implicit state mutation
   anywhere in the language. Cancellation should follow the same rule:
   the user calls `TASK_IS_CANCELLED()` or `YIELD()` at well-known
   checkpoints, rather than being interrupted at an arbitrary one.

2. **SHIELD complexity.** Transient fits naturally with `SHIELD(fn)`
   (plan §3 Phase F5). If we picked Persistent, `SHIELD` would need an
   extra protocol — save the cancellation request during the block,
   rethrow on exit (Trio's `CancelScope.shield`). With Transient, the
   next `YIELD` checkpoint after `SHIELD` exit simply observes the cancel
   flag; SHIELD stays trivial.

3. **Single-threaded cooperation is enough.** WLWL v0.7's scheduler is
   cooperative single-threaded (ADR-0016). CPU-bound tasks don't execute
   in parallel. In this model, forcing a Persistent interruption
   releases no resources (no other task is running anyway); letting the
   user pick their own checkpoint gives precise cleanup control.

## Decision Outcome

Chosen points locked into the v0.7 runtime:

- Runtime modules must implement the Brown 9-dimension table above
  verbatim. Deviations require an ADR amendment first.
- `wlwl-spec-v0.7.md` §17.1 will publish the table as a normative
  reference.
- Persistence = Transient is documented as an *intentional divergence
  from Trio*; spec §17.7 marks it as such for downstream readers.

**Migration guidance:**

- Authors coming from Trio should expect `YIELD()` and `TASK_IS_CANCELLED()`
  calls at cooperative checkpoints in long-running fns.
- Authors coming from Kotlin will find the model familiar (Transient +
  Structured).

### Explicit non-rules

- No "ambient cancellation" — cancellation is not implicitly checked at
  every op.
- No `Persistent` mode toggle in v0.7 (deferred if ever needed).
- No Bottom-Up or Simultaneous propagation directions.

## Consequences

**Positive:**

- Aligned with Kotlin and cff on Persistence; users with Kotlin
  experience can transfer mental models directly.
- `SHIELD` implementation is trivial (Transient + Aware + Top-Down).
- Predictable cancellation: every cancelled fn ends at a user-visible
  checkpoint, never mid-operation.

**Negative:**

- Diverges from Trio on Persistence (low-priority community concern —
  plan §9 R6; ~20 % probability).
- Users must remember to add `YIELD()` checkpoints in long-running fns;
  omitting them means cancellation has no effect on that fn. Phase G5
  + dev docs must surface this prominently.
- Any runtime that wants to change Persistence later will need an ADR
  amendment + spec change.

## References

- `docs/history/wlwl-build-plan-v0.7-COMPLETED.md` §2.3 / §10 D2-D10 / §附录 B ADR-0015
- `docs/adr/0014-structured-concurrency-v0.7.md`
- `docs/adr/0016-scheduler-single-thread-boundary.md`
- Brown et al. 2025 *A Design Space Exploration of Async/Await*
- Trio / Kotlin `coroutineScope` / Swift `TaskGroup` / Tokio / Smol /
  Asyncio / C# / JS — runtime choices for cross-reference
- Trio `CancelScope.shield` — pattern that motivates the Transient choice