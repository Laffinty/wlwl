# ADR-0017 — Cooperative suspension-based scheduler for v0.9

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-09-24 (proposed) / 2026-09-25 (accepted, plan §9.2 默认 Approved) |
| **Deciders** | Li (project lead) |
| **Related** | spec v0.8.1 §17.1 / §17.2 / §17.7 / §11.2, ADR-0014 (structured concurrency + channels), ADR-0015 (Brown 9-dim decisions), ADR-0016 (single-thread scheduler boundary), ADR-0018 (E0055/E0057 retention), ADR-0019 (OOP), `wlwl-build-plan-v0.9` §3 |

## Context and Problem Statement

WLWL v0.7 / v0.8.1 implements structured concurrency (ADR-0014) on a
single-threaded cooperative scheduler (ADR-0016) using a
**static-segmented** model: `yield_split.rs:split_body_for_yield`
pre-slices every spawned task body at `YIELD` / `await` points, the
interpreter walks one segment at a time, and the scheduler resumes only
at segment boundaries. Synchronous channels (`buf=0`) return
`ERR(kind="ChannelWouldBlock")` instead of suspending.

This was the cheapest path to land structured concurrency (B1–B5 in
v0.7, ~3-5 person-weeks per plan §2.4). spec v0.8.1 §17.1 (line 879)
already documents that the resulting restrictions are an
**implementation-path artefact**, not language semantics:

> "该限制是 v0.7 / v0.8 实现路径的产物,不是语言语义规则;未来版本
> (挂起式调度)可放宽而不视为破坏性修订。"

v0.9 is the version that explicitly takes that future-version window:

- §17.7 "阻塞挂起" / "嵌套 YIELD" / "E0055/E0057 保留无触发路径"
  three rows together block `buf=0` synchronous channels from being
  actually usable, force every `YIELD()` to be wrapped in a `Block`
  direct child, and keep two error codes registered but never fired.
- Brown, Krishnamurthi, Crichton (*A Design Space Exploration of
  Async/Await*, OOPSLA 2026) confirms that v0.7's 9-dimension subset is
  correct, but the **Dynamic Suspension** dimension cannot honestly be
  implemented by static segmentation — Dynamic Suspension requires a
  real suspension point per `await` site, not a compile-time split
  table.
- The `Rc<RefCell<Env>>` baseline (ADR-0016) is preserved across the
  change: a real-suspension state machine on top of the existing
  interpreter is still stackless (no `Arc<RwLock<Env>>` rewrite, no
  thread pool, no `Send`/`Sync`).

The question is **how to upgrade the scheduler** from
"static-segmented" to "true-suspension + state-machine" without
violating ADR-0016's single-thread boundary, ADR-0014's structured
concurrency invariants, or v0.6's `Rc<RefCell<Env>>` entanglement.

## Decision Drivers

- **Fidelity gate.** v0.9 must preserve v0.6 fixtures passing through
  unchanged. The fidelity baseline (`v07_fidelity`) is locked at 1
  entry; the rest of v0.6 conformance is checked via
  `wlwl-cli/tests/conformance.rs`. v0.9 must not regress either.
- **Single-thread boundary (ADR-0016).** No `Send`/`Sync` bounds on
  `Env`; no thread pool; no `Arc<RwLock<Env>>`. Real suspension on top
  of the existing interpreter is stackless and compatible.
- **Structured concurrency invariants (ADR-0014).** Tree, parent-equals-
  child, errors propagate up, cancellation propagates down — all four
  must be preserved. Real suspension does not relax any of these; it
  only adds another legitimate suspension point.
- **Brown 9-dim subset is already correct.** ADR-0015 picks
  Lazy / Dynamic / Dynamic / Strong / Awaited / Destructive / Aware /
  Top-Down / Transient — Gray et al. OOPSLA 2026 confirms this subset
  is industrial-consensus (Trio / Kotlin / Swift). v0.9 does **not**
  re-open those decisions; it only changes the engineering
  implementation of **Suspension = Dynamic** from "static segment" to
  "true suspension". A new dimension `Reachable` is added to the
  resolution table: in v0.9 `Suspended(reason) → Running` is always
  reachable (§6 v0.9 below).
- **Algebraic-effect framing (user-pioneering choice, 2026-09-24).**
  spec §17 is upgraded to document concurrency primitives as
  algebraic effects: `YIELD()` = `perform Yield`,
  `TASK_CANCEL(t, r)` = `raise Cancelled(r)`,
  `CHANNEL_SEND/RECV` = `perform ChannelOp`. The runtime `Effect`
  enum is the WasmFX-style `tag + payload` representation
  (WasmFX Phase 3; McCabe & Lindley 2026-05). Users do not get custom
  effects in v0.9 (deferred to v0.10+), but terminology and internal
  naming align with Koka / OCaml 5 effects.
- **Performance budget.** Single-task regression < 10% vs v0.6
  baseline (carried over from ADR-0016); concurrent path guarantees
  correctness + leak-freeness only; no throughput / latency promises.
- **Engineering cost.** Static-segmented → true-suspension is bounded
  by the existing interpreter's hot path; estimate 5–7 person-weeks
  (yield_split rewrite + scheduler + channel + tests).

## Considered Options

### A. True-suspension + algebraic-effect state machine (chosen)

Keep the cooperative single-thread scheduler (ADR-0016) but replace
the static segmenter with a real suspension state machine driven by
the scheduler. `yield_split.rs` degenerates to a ~100-line entry
wrapper that packages a task body into
`TaskSpec { body_expr, env, suspended_at: Option<ExprId> }` without
slicing. The scheduler loop runs `eval_expr` directly, captures
`Signal::Yield` / `Effect::*` at any expression position, parks the
task in `TaskState::Suspended(reason)`, and resumes it at the
**continuation after the suspension point**, not at the next segment
boundary.

Concretely:

- `yield_split.rs:split_body_for_yield` and `validate_yield_position`
  are deleted. `E0014` keeps its registration but is **no longer
  triggered by YIELD position**; its remaining trigger is
  `RETURN / BREAK / CONTINUE` in illegal positions only.
- `runtime.rs::Scheduler` gains
  `TaskState::Suspended(YieldReason::*)` handling for `Yield(Explicit)`
  / `Yield(ChannelOp)` / `Yield(CancellationCheck)`. The match is an
  effect-handler loop: `Step::Yield` parks the current task,
  `Step::ChannelOp(dir, channel, value)` consults `Channel::waiters`,
  `Step::Cancelled(reason)` raises cancellation per ADR-0015.
- `channel.rs::send` / `recv` for `buf=0` channels no longer return
  `ERR(kind="ChannelWouldBlock")`; instead they park the current task
  in `ChannelWaiter { task_id, direction, value }` and return
  `Signal::Yield(YieldReason::ChannelOp)`. `TRY_*` paths stay
  non-blocking (unaffected).
- Internal enum `Effect { Yield { explicit: bool }, ChannelOp { dir,
  channel, value }, Cancelled { reason: Dict } }` is the canonical
  control-flow event (WasmFX-style `tag + payload`); `Scheduler::step`
  is the canonical handler.
- §17.7 limitations table is updated:
  - "阻塞挂起" row → CHANGED: `CHANNEL_SEND/RECV` 满/空 now **suspend**
    instead of returning `ChannelWouldBlock`.
  - "嵌套 YIELD" row → CHANGED: nested constructs (WHILE/FOR/IF) resume
    their inner body after `YIELD`, not the outer Block.
  - "死锁检测" row → CHANGED: minimal L1 detection enabled (see ADR
    reasoning; per-plan §3.4: L1 strict, excluding `YieldReason::
    Explicit` / `YieldReason::CancellationCheck`).
  - "公平性" / "在飞兄弟取消" rows → MOVED to a new §17.8 normative
    sub-section ("fairness / observability, added in v0.9"); §17.7
    shrinks to 7 rows (线程模型 / 性能 / 延迟-吞吐 / 阻塞挂起 /
    死锁检测 / 嵌套 YIELD / 尾部 YIELD / 隐式 scope).
- spec §17.1 YIELD 任意表达式位置合法 (no longer an implementation-
  path restriction). spec §17.2 同步通道真挂起 normative. spec
  §17.3 tag/payload cancellation normative. spec §17 algebraic-effect
  framing normative 段(per ADR-0019 §4.4.1).

**Pros:**

- Aligns spec wording with implementation (no more "future version can
  relax this" escape clause).
- §17.7 限制表 3 项关键限制一次性取消(buf=0 真挂起 / 嵌套 YIELD /
  YIELD 位置);另 5 项保留或迁 §17.8。
- Brown 9-dim `Suspension = Dynamic` is honestly implemented.
- Algebraic-effect framing provides a clean terminology for v0.10+
  user-defined effects and WasmFX backend migration.
- Indirect YIELD (`LET(y, YIELD); y()`) auto-suspends at every call
  site with **zero additional implementation cost** — the effect
  handler model means every `YIELD`-bound function call performs the
  same `Yield` effect as the direct call (per plan §4.4.1 audit
  finding 3, 2026-09-24).
- Single-thread boundary preserved (ADR-0016): no `Send`/`Sync`, no
  thread pool, no `Arc<RwLock<Env>>`. `Rc<RefCell<Env>>` continues.
- Performance budget (single-task < 10% regression) is feasible —
  the segmenter already cost a comparable amount per `YIELD`; real
  suspension has a similar per-suspend/resume pair cost plus a small
  state-machine dispatch.

**Cons:**

- Re-writes three files end-to-end:
  `yield_split.rs` (492 → ~100), `runtime.rs` (718 → ~800),
  `channel.rs` (342 → ~450).
- Concurrent conformance suite (v0.7 fixtures) must be re-derived —
  every `TRY_*` test stays; every "would block" expectation must be
  rewritten to "suspends until paired".
- Deadlock L1 detection adds an opt-in development-time check; false
  positives must be controlled by the
  `YieldReason::Explicit` exclusion.
- Algebraic-effect framing adds an internal-only naming churn (no
  user-visible breakage) that ripples into dev docs.

### B. Keep static segmentation, document limitations as permanent

Accept §17.7 limitations as language semantics; document
`buf=0` synchronous channels as "non-suspending; use `TRY_*` or
pre-buffer" in spec; keep `E0014` firing on YIELD position.

**Pros:** No implementation work.

**Cons:**

- spec v0.8.1 line 879 already says this is not language semantics;
  relabelling as permanent would be a regression of the spec's own
  promise.
- `buf=0` synchronous channels remain half-usable in practice (only
  pre-buffered paths are real).
- Brown 9-dim `Suspension = Dynamic` becomes a half-truth.
- 后续 v0.10+ 暴露 algebraic effect / WasmFX 后端时还得回头改
  scheduler,届时成本叠加。

### C. Stackful coroutines (Go / BEAM style)

Introduce real per-task stacks; M:N scheduling; either pre-emptive
(BEAM reduction budget) or cooperative (Go 1.14+). Same single-thread
boundary still applies, but per task needs ~1-8 KB stack.

**Pros:** Most ergonomic model for users; mature in industry.

**Cons:**

- Conflicts with `Rc<RefCell<Env>>` baseline — each task needs its own
  `Env` snapshot, requiring `Rc → Arc` upgrades or task-local env
  cloning (significant refactor).
- v0.7 plan §2.4 estimated 16-24 person-weeks for the full stackful
  rewrite. v0.9 budget is 14-18 person-weeks for the entire scope
  (concurrent rewrite + OOP + algebraic effects).
- Cancelled/ChannelOp propagation through a real stack needs
  unwinding protocol — adds API surface for the same problem option A
  solves with a state-machine dispatch.

### D. Continuations + effect handlers (Koka / OCaml 5 / WasmFX Phase 4)

First-class continuations; multi-shot effects; backtracking /
generators expressible.

**Pros:** Maximum expressive power; aligns with v0.10+ WasmFX
direction; Curry-Howard map to session types is clean.

**Cons:**

- WasmFX is Phase 3 (2026-05); not yet in any browser production
  (Wasm 3.0 2026-06-13 ships without stack switching).
- Multi-shot continuations on the `Rc<RefCell<Env>>` baseline have
  unresolved ownership issues.
- Plan v0.9 §2.2 (类 D) explicitly labels this as **v0.10+ long-term
  goal**, not v0.9 scope. User 2026-09-24 confirmed α+先锋 within
  v0.9, not D.

## Decision Outcome

Chosen option **A**. Concretely, v0.9 commits to:

- **Scheduler.** `wlwl-eval/src/runtime.rs` gains a `Scheduler::step`
  effect-handler loop driving `TaskState::{Pending, Running,
  Suspended(YieldReason::*), Done, Cancelled}`; `ChannelWaiter`
  pairing; FIFO runnable queue (best-effort, see §17.8).
- **Yield segmenter retired.** `wlwl-eval/src/yield_split.rs` shrinks
  to a ~100-line task-body entry wrapper; no more `split_body_for_yield`,
  no more `validate_yield_position`.
- **Channels suspend, not return WouldBlock.**
  `wlwl-eval/src/channel.rs` deletes `TryResult<T>::WouldBlock` from
  the SEND/RECV eval path; new
  `ChannelWaiter { task_id, direction, value }` data structure; close
  protocol wakes pending waiters via `Cancelled` propagation
  (§17.3, ADR-0015 Persistence = Transient).
- **Algebraic-effect internal naming.**
  `wlwl-eval/src/effect.rs` (new file, ~80 lines) houses
  `enum Effect { Yield { explicit: bool }, ChannelOp { dir, channel,
  value }, Cancelled { reason: Dict } }`; `Scheduler::step` is the
  effect handler; spec §17 adds a normative "algebraic-effect
  framing" paragraph aligned with Koka / OCaml 5.
- **§17.7 / §17.8 split.** spec v0.9 §17.7 retains the limitation
  table but reduced to 7 rows; spec v0.9 §17.8 is a new
  fairness / observability section absorbing the migrated rows.
- **E0014 narrowed.** `E0014` keeps its registration but its only
  remaining trigger is `RETURN / BREAK / CONTINUE` in illegal
  positions; YIELD position no longer triggers it.
- **E0065 / W0065.** A new deadlock-detection code pair is
  registered: `W0065` is the dev-mode soft warning (default),
  `E0065` is the production-mode opt-in via
  `wlwl.toml [strict_deadlock_detect] true`. Implementation
  is per plan §3.4 (L1 strict, excluding `YieldReason::Explicit` /
  `YieldReason::CancellationCheck`).
- **Brown 9-dim subset unchanged.** Eagerness = Lazy, Ref Strength =
  Strong, Destruction = Awaited, Propagation = Destructive,
  Awareness = Aware, Direction = Top-Down, Persistence = Transient
  (per ADR-0015). **Suspension = Dynamic** keeps the same value but
  is now honestly implemented. A new dimension `Reachable` is added
  (always-true by construction: every `Suspended` has a documented
  resume path).
- **Performance budget.** Single-task regression < 10% vs v0.6
  (carried over from ADR-0016); concurrent path correctness +
  leak-freeness only. Plan §10 risk table tracks fallbacks.
- **Fidelity gate.** v0.6 conformance fixtures must pass unchanged;
  `v07_fidelity` baseline is re-derived in Step 13 to v0.9 spec but
  the closure-cell entry stays at 1 lock-test delta.

### Explicit non-rules

- No `Send`/`Sync` bounds added to `Env` or any `wlwl-eval`
  internal type (ADR-0016 boundary intact).
- No thread pool; no `Arc<RwLock<Env>>`; no atomic-based
  environment sharing.
- No `Promise` / `Future` types exposed to `.wll` programs
  (ADR-0014 boundary intact).
- No user-defined effects in v0.9 (deferred to v0.10+); the
  algebraic-effect framing is internal naming + spec wording only.
- No WasmFX compile target in v0.9 (deferred to v0.10+ when WasmFX
  reaches Phase 4 / browser production).
- No multi-shot continuations in v0.9 (option D path).
- No external selection (`&`) / named protocols / parallel branches
  in session types for v0.9 (those are ADR-0019 §4.4.3 v0.9.1+
  deferred items).

### Migration guidance

- `.wll` source code is **unchanged** at the language level. v0.6 /
  v0.7 / v0.8.1 source files compile and run unchanged on v0.9.
- `TRY_*` and `TRY_*` users keep working: `TRY_*` paths stay
  non-blocking even after the channel rewrite.
- Lock-test count growth: v0.8.1 = 1409 passed → v0.9 target
  ~1465 passed (per plan §9.3; +56 incremental lock tests across
  §3.1-§3.6, §4.4.1-§4.4.3, §5).
- New error codes: `E0065` (deadlock, opt-in), `E0066` (reason
  non-DICT in tag/payload cancel, per ADR-0019 §4.4.2). Total
  activated error codes: 56 (v0.8.1) + 3 (E0032/E0050/E0051 from
  reserve to active, per ADR-0019 §4.3) + 2 (E0065/E0066 new) = 61.
- Total reserved error codes: 11 (v0.8.1) - 3 (promoted) - 2
  (E0055/E0057 removed per ADR-0018) = 6.

## Consequences

**Positive:**

- spec v0.8.1 line 879's "future version can relax this" promise is
  paid off: §17.1 YIELD 任意表达式位置合法,§17.2 同步通道真挂起,
  §17.3 tag/payload cancellation,§17.7 限制表缩减为 7 行,§17.8
  新增公平性 / 可观测性 normative 段。
- Three v0.7/v0.8.1 limitations cancelled in one motion
  (buf=0 / 嵌套 YIELD / YIELD 位置);two reserve codes removed
  (E0055/E0057 per ADR-0018);two codes promoted from reserve to
  active (E0032/E0050/E0051 per ADR-0019);three new codes added
  (E0065, E0066, this ADR).
- Indirect YIELD (`LET(y, YIELD); y()`) becomes correct-by-construction:
  no spec change, no runtime check, just consistent effect
  propagation.
- Algebraic-effect framing unlocks a clean path for v0.10+ user-
  defined effects (`perform` / `handle`) without changing the
  scheduler again.
- Brown 9-dim 9-dim-subset is now honest: every chosen dimension
  is honoured by the runtime, not just documented.
- Indirect YIELD = zero implementation cost due to effect-handler
  unification; §17.1 prose update is one normative paragraph.
- Single-thread boundary preserved (ADR-0016): zero `Send`/`Sync`
  bounds, no thread pool, no env rewrite.
- Lock-test count growth (+56) is bounded and tested per step
  (§9.3); every Step 2-10 has explicit lock-test entries.

**Negative:**

- Re-writes three core eval files end-to-end:
  `yield_split.rs` (492 → ~100), `runtime.rs` (718 → ~800),
  `channel.rs` (342 → ~450). Plus new file `effect.rs` (~80).
- Concurrent conformance suite (v0.7 fixtures) must be re-derived
  in Step 13; every "would block" expectation rewritten to
  "suspends until paired".
- Deadlock L1 detection adds an opt-in dev-time check; false
  positives must be controlled by the `YieldReason::Explicit`
  exclusion (per plan §3.4; 草稿 2 audit 2026-09-24 narrowed L1 to
  exclude both `Explicit` and `CancellationCheck`).
- Algebraic-effect naming churn is internal-only but ripples into
  dev docs, §17 prose, and Step 11 spec derivation.
- Brown 9-dim Reachable = always-true needs an explicit guarantee
  in spec §17 (otherwise the dimension is just an assertion).
- Concurrent performance budget (no throughput / latency promises)
  is unchanged but more visible because §17.8 is now normative.

### Fallback paths (per plan §10)

- If `yield_split.rs` rewrite breaks existing SPAWN behaviour beyond
  Step 2 lock-test coverage → revert to v0.8.1 baseline (git revert
  per single-commit policy).
- If `Channel::send` / `recv` rewrite breaks > 30% of v0.7
  concurrency conformance → revert to v0.8.1 `WouldBlock` path
  temporarily; defer the rewrite to v0.9.1.
- If deadlock L1 false-positive rate > 1% → downgrade to
  dev-mode-opt-in only; v0.9.1 re-tune the threshold.
- If algebraic-effect runtime naming churn regresses eval hot-path
  > 5% → keep effect framing as spec-only (drop `effect.rs` file),
  fall back to internal `Signal::Yield` naming.

## References

- `docs/plan/wlwl-build-plan-v0.9.md` §3 (P0 并发语义收口) / §9.1
  Step 1-7 / §10 risk table / §11.2 consistency table / §11.3 doc
  acceptance
- `docs/standard/wlwl-spec-v0.8.md` §17.1 (line 879) / §17.2 /
  §17.3 / §17.7 / §11.2 (E0055/E0057 保留位)
- `docs/history/deviations-v0.8.md` D8-003 (E0055/E0057 reservation)
- `docs/adr/0014-structured-concurrency-v0.7.md`
- `docs/adr/0015-brown-9-dimension-decision.md`
- `docs/adr/0016-scheduler-single-thread-boundary.md`
- `docs/adr/0018-e0055-e0057-retention-decision.md`
- `docs/adr/0019-oop-minimal-implementation-with-behavioral-types.md`
- Gray, Krishnamurthi, Crichton. *A Design Space Exploration of
  Async/Await*. OOPSLA 2026 / PACMPL Vol. 10, DOI 10.1145/3839519,
  arXiv 2608.20677
- Leijen, D. *Koka: Functional Programming with Algebraic Effects
  and Effect Handlers* (2014+; v3.x 2024-2026)
- Sivaramakrishnan, K. *Multicore OCaml / OCaml 5 Effects* (2022,
  stabilized 2024)
- McCabe, F.; Lindley, S. *WebAssembly Stack Switching Proposal*
  (WasmFX, Phase 3, 2026-05)
- Pretnar & Bauer 2015 *An Effect System for Algebraic Effects and
  Effect Handlers* — algebraic-effects foundational paper
- Crichton, W. *Async in depth* (Tokio tutorial 2024-2026)
- Trio Nurseries, Kotlin `coroutineScope`, Swift `withTaskGroup`,
  JEP 525 / 533 (Java StructuredTaskScope) — industrial-reference
  comparison points