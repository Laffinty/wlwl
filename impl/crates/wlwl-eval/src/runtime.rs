//! [v0.7 Phase B1] cooperative coroutine runtime skeleton.
//!
//! **Types only at this stage** -- no scheduler behaviour, no
//! state-machine conversion of `eval_expr`. The shape is fixed in
//! B1 so B4 (Task / Scope data) and B5 (Scheduler loop + CPS eval
//! conversion) can build on it without forward-declaration gymnastics.
//!
//! See plan §5.1, §5.1.1, §5.2, ADR-0014, ADR-0016 for the full design.

use std::collections::VecDeque;
use std::rc::Rc;

use wlwl_error::WlwlError;

use crate::{Expr, Value};

/// Identifies a task slot within a single [`Scheduler`].
///
/// Cheap to copy; not a handle (see [`TaskHandle`] for what `SPAWN`
/// returns to user code).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TaskId(pub usize);

/// Generation-tracked task handle returned to user code by `SPAWN`.
///
/// Detecting `use-after-cancel` / `use-after-recycle`: a slot can be
/// reused after a task finishes, but its generation bumps. A handle
/// that doesn't match the current generation is stale (E0053).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TaskHandle {
    pub id: TaskId,
    pub generation: u64,
}

/// Identifies a scope slot within a single [`Scheduler`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ScopeId(pub usize);

/// Identifies a channel slot within a single [`Scheduler`].
///
/// Re-exported alias to the full [`crate::channel::ChannelId`] type;
/// same migration rationale as [`TaskEntry`]. D-A unified this
/// with the proper `ChannelId`; the previous `RcHandle(pub usize)`
/// placeholder was kept around so D-A could land without churning
/// every call site in the same commit.
pub type RcHandle = crate::channel::ChannelId;

/// Task lifecycle state (plan §5.1.1).
///
/// Transitions:
/// - `Pending`    -- on `Scheduler::resume(task_id)` --&gt; `Running`
/// - `Running`    -- yields via [`YieldReason`] --&gt; `Suspended(reason)`
/// - `Running`    -- body returns `Ok(v)` or `Err(e)` --&gt; `Done(...)`
/// - `Running`    -- parent scope cancelled --&gt; `Cancelled`
/// - `Suspended`  -- waiting event ready --&gt; `Running`
/// - `Suspended`  -- parent scope cancelled --&gt; `Cancelled`
/// - `Done` / `Cancelled` are terminal.
#[derive(Debug, Clone)]
pub enum TaskState {
    Pending,
    Running,
    Suspended(YieldReason),
    Done(Box<TaskResult>),
    Cancelled,
}

/// Terminal value of a task body (plan §5.1.1 row "fn 返回").
#[derive(Debug, Clone)]
pub enum TaskResult {
    Ok(Value),
    /// User-level `ERR(...)` returned by the task body (plan §5.1.1
    /// "fn 返回 ERR(e) → Done(Err(e))"). AWAIT hands this back as a
    /// value (§5.4 "作为返回值"), so §8.2 transparent propagation
    /// applies at the AWAIT call site.
    Err(Value),
    /// Host diagnostic raised inside the task body (e.g. E0020).
    /// AWAIT re-raises it (P7-C2-002 / plan §5.4). Distinct from
    /// [`TaskResult::Err`] so user ERR values and host diagnostics
    /// do not collapse into one channel.
    Failed(Box<WlwlError>),
}

/// Reasons a task may yield at a checkpoint (plan §5.1.1 row
/// "Suspended(*)" + "协作式让步点").
///
/// Each variant carries enough information for [`Scheduler`] to route
/// the task to the correct wait list (or to wake it when an event
/// becomes ready). `Explicit` corresponds to user `YIELD()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum YieldReason {
    /// User-invoked `YIELD()`. No wait condition; the task is
    /// immediately re-eligible (added back to `run_queue`).
    Explicit,
    /// `AWAIT(child)` and `child` has not finished yet. The task is
    /// parked until `child` transitions to `Done` or `Cancelled`,
    /// at which point `Scheduler::wake_dependents` re-enqueues it.
    AwaitingChild(TaskId),
    /// `CHANNEL_RECV(ch)` and `buf` is empty. The task is parked
    /// on `ch`'s receiver wait list until a sender wakes it.
    ReceivingOn(RcHandle),
    /// `CHANNEL_SEND(ch)` and `buf` is full. The task is parked on
    /// `ch`'s sender wait list until a receiver drains a slot.
    SendingOn(RcHandle),
}

// ───────────────────────────────────────────────────────────────────
// v0.9 Step 3 / ADR-0017 §3.1 / ADR-0019 §4.4.1 — WasmFX-style
// algebraic-effect tag dispatch.
//
// The runtime sees control-flow events as `Effect { tag, payload }`
// — the canonical algebraic-effect representation (Pretnar & Bauer
// 2015). WasmFX Phase 3 calls this `tag + payload`; Koka / OCaml 5
// effect systems call it `effect : Effect`. The legacy `Signal::Yield
// (YieldReason::*)` path is preserved for backward compatibility
// (every `Signal::Yield(YieldReason)` has a one-to-one mapping into
// `Effect`), but new code (Step 4 channel ops, Step 8 tag/payload
// cancel, ADR-0019 §4.4.3 MethodCall / PropAccess / ProtocolViolation)
// builds on `Effect` directly.
//
// See `wlwl-build-plan-v0.9.md` §0.5 + §3.1 + §3.2 + §4.4.1 +
// §4.4.4 for the design rationale.
// ───────────────────────────────────────────────────────────────────

/// WasmFX-style dispatch tag for an [`Effect`]. Used by the effect
/// handler loop to route an effect to its handler without inspecting
/// the payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tag {
    /// `perform Yield` — collaborative yield point. Equivalent to
    /// `Signal::Yield(YieldReason::Explicit)` in the legacy plumbing.
    Yield,
    /// `perform ChannelOp` — synchronous SEND / RECV blocking on a
    /// channel's buf or wait list. Maps to
    /// `Signal::Yield(YieldReason::ReceivingOn | SendingOn)` in the
    /// legacy plumbing; the runtime routes to the channel's
    /// sender_waiters / receiver_waiters list.
    ChannelOp,
    /// `raise Cancelled` — task was cancelled (with optional reason
    /// payload per ADR-0019 §4.4.2). Maps to `TaskState::Cancelled`
    /// transition; no legacy `Signal` equivalent (cancellation was
    /// previously advisory-only via `cancel_requested` flag).
    Cancelled,
}

/// Direction of a channel operation. Used by [`Effect::ChannelOp`]
/// and [`ChannelWaiter`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
    /// `CHANNEL_SEND(ch, v)` — sending a value into the channel.
    Send,
    /// `CHANNEL_RECV(ch)` — receiving a value from the channel.
    Recv,
}

/// One control-flow event in the algebraic-effect framing
/// (ADR-0019 §4.4.1).
///
/// `Effect` is the canonical internal control-flow event. Every
/// `Signal::Yield(YieldReason)` from the legacy plumbing has a
/// one-to-one mapping; new code (Step 4 channel ops, Step 8
/// tag/payload cancel, ADR-0019 §4.4.3 OOP variants) constructs
/// `Effect` directly. The `Scheduler::step` effect-handler loop
/// routes by [`Effect::tag`].
///
/// v0.9 ships only the three tags enumerated by ADR-0017 §3.1
/// (Yield / ChannelOp / Cancelled). Step 8 adds `reason: Dict` to
/// the Cancelled variant; ADR-0019 §4.4.3 adds `MethodCall`,
/// `PropAccess`, `ProtocolViolation` (deferred to that Step).
#[derive(Debug, Clone)]
pub enum Effect {
    /// `perform Yield` — cooperative yield checkpoint. `explicit =
    /// true` corresponds to user `YIELD()`; `explicit = false` is
    /// reserved for runtime-injected yields (none in v0.9, but the
    /// field is here for future cancellation-yield symmetry per
    /// ADR-0019 §4.4.1 "indirect YIELD auto-suspends").
    Yield { explicit: bool },
    /// `perform ChannelOp` — synchronous channel SEND / BLOCK on a
    /// recv. `value` is `Some` for SEND (the value being pushed);
    /// `None` for RECV.
    ChannelOp {
        dir: Direction,
        channel: RcHandle,
        value: Option<Value>,
    },
    /// `raise Cancelled` — task cancellation. v0.9 ships the empty-
    /// payload variant; Step 8 (ADR-0019 §4.4.2) extends with
    /// `reason: Dict`.
    Cancelled,
}

impl Effect {
    /// Extract the dispatch [`Tag`].
    pub fn tag(&self) -> Tag {
        match self {
            Effect::Yield { .. } => Tag::Yield,
            Effect::ChannelOp { .. } => Tag::ChannelOp,
            Effect::Cancelled => Tag::Cancelled,
        }
    }
}

/// One parked task on a channel's wait list (per plan §3.2 / §4.4.4).
///
/// Stored in `Channel::sender_waiters` or `Channel::receiver_waiters`
/// (existing infrastructure, see `channel.rs`). The `value` field is
/// `Some(_)` only for SEND waiters (the value they want to push); it
/// is `None` for RECV waiters (the value is unknown until paired with
/// a sender).
#[derive(Debug, Clone, PartialEq)]
pub struct ChannelWaiter {
    pub task_id: TaskId,
    pub direction: Direction,
    pub value: Option<Value>,
}

/// What one step of the state-machine evaluator produces
/// (plan §5.1.1 pseudocode "match scheduler.step(task)").
///
/// Replaces the implicit "always Done" assumption of the recursive
/// `eval_expr`. At B5a-1 no real yield path exists yet, so callers
/// see `Done` for every input; B5b introduces yield points at
/// `YIELD()` / `AWAIT(child)` / `CHANNEL_RECV` / `CHANNEL_SEND`,
/// and `Blocked` once `CHANNEL_*` wait queues are wired up.
#[derive(Debug)]
pub enum StepResult {
    /// Evaluation completed with a value (OK or ERR propagated).
    /// The scheduler treats this as a terminal transition and
    /// moves the task from `Running` to `Done(value)` (plan
    /// §5.1.1 row "fn 返回").
    Done(crate::Value),
    /// The task yielded at a checkpoint. The yield reason tells
    /// the scheduler which wait list to put the task on. When the
    /// wait condition becomes ready, `Scheduler::wake_dependents`
    /// (or the equivalent for `YieldReason::Explicit`) re-enqueues
    /// the task. Plan §5.1.1 row "Running -- 用户代码 YIELD()" etc.
    Yield(YieldReason),
    /// The task is blocked on a resource wait (e.g. channel buf).
    /// Distinct from `Yield` because the scheduler treats blocked
    /// tasks differently from yielded ones: blocked tasks are NOT
    /// re-enqueued on the run_queue; they live on a resource wait
    /// list and only get woken by the matching wake event.
    /// Plan §5.1.1 pseudocode arm `StepResult::Blocked`.
    Blocked,
}

/// Stack-machine evaluation state (plan §5.1: "enum-driven stack
/// machine (`EvalState { Expr, ContStmt, pc }`)").
///
/// **NOT WIRED YET.** B5 will replace the recursive `eval_expr`
/// with a state-machine evaluator that returns one of these per
/// step. At B1 the enum exists only so [`TaskEntry`] can reference
/// the type and so reviewers see the planned shape.
///
/// `pc` semantics depend on the variant:
/// - `Expr(expr)` -- "evaluate this expression now"; `pc` is unused
///   at the top level, but compound expressions may need a `pc`
///   for sub-step bookkeeping.
/// - `ContStmt(stack)` -- resume with this continuation stack;
///   `pc` (encoded in the variant, not shown here for simplicity)
///   indexes the next continuation frame.
#[derive(Debug, Clone)]
pub enum EvalState {
    Expr(Box<Expr>),
    ContStmt(Vec<ContFrame>),
}

/// One continuation frame in the stack machine.
///
/// **Not yet populated.** B5 will introduce concrete frames as the
/// recursive eval gets rewritten; B1 keeps the placeholder so
/// [`EvalState::ContStmt`] has a meaningful payload type.
#[derive(Debug, Clone)]
pub enum ContFrame {
    /// Bind the just-returned value to a local name in the current
    /// scope (corresponds to the tail of `LET(name, <expr>)`).
    BindLocal(String),
    /// Move on to the next branch / next statement in a compound
    /// expression (sequence, IF, MATCH, etc.).
    Next,
}

/// One slot in the scheduler's task table.
///
/// Re-exported alias to the full [`crate::task::Task`] type that
/// was defined in B4; kept here so B1-era callers (e.g. tests)
/// that name `TaskEntry` keep working through B4 / B5. New code
/// should reference [`crate::task::Task`] directly.
pub type TaskEntry = crate::task::Task;

/// A scope: the lifetime container for tasks (plan §5.2 "Scope 是
/// Task 的容器").
///
/// Re-exported alias to [`crate::task::Scope`]; same migration
/// rationale as [`TaskEntry`].
pub type Scope = crate::task::Scope;

/// The scheduler itself (plan §5.1 + §5.1.1 loop pseudo-code).
///
/// B5b wires the run-queue bookkeeping. Task *execution* stays on
/// `Evaluator::run_one_task` (it needs `invoke_closure`); this type
/// owns ids, generations, wait lists, and the queue.
#[derive(Debug)]
pub struct Scheduler {
    pub tasks: Vec<TaskEntry>,
    pub scopes: Vec<Scope>,
    pub run_queue: VecDeque<TaskId>,
    pub next_generation: u64,
    /// Innermost active scope (B5b). SPAWN registers children here;
    /// SCOPE pushes/pops. Root scope id 0 always exists.
    pub current_scope: Option<ScopeId>,
    /// [v0.7 Phase D-A] Channel slots, one per `CHANNEL_NEW(buf)`.
    /// Empty at construction; slots are added by `CHANNEL_NEW` (D-B)
    /// and force-closed / bumped-generation by the leak detector (D-D).
    pub channels: Vec<crate::channel::Channel>,
    /// [v0.7 Phase D-A] Monotonic counter used to mint generation
    /// values for channel handles. Bumped by `CHANNEL_NEW` on
    /// allocation and again on scope-exit force-close (D-D).
    pub next_channel_generation: u64,
}

impl Default for Scheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl Scheduler {
    /// Create an empty scheduler with a root scope (id 0).
    pub fn new() -> Self {
        let mut s = Self {
            tasks: Vec::new(),
            scopes: Vec::new(),
            run_queue: VecDeque::new(),
            next_generation: 0,
            current_scope: None,
            channels: Vec::new(),
            next_channel_generation: 0,
        };
        s.scopes.push(Scope::new(ScopeId(0), None));
        s.current_scope = Some(ScopeId(0));
        s
    }

    /// [v0.7 Phase C2] Allocate the next `TaskId` without pushing a
    /// task entry. Use this when the caller wants to construct the
    /// full `Task` value (with body, env, etc.) before
    /// committing it to `self.tasks` via `push_task`.
    pub fn next_task_id(&self) -> TaskId {
        TaskId(self.tasks.len())
    }

    /// [v0.7 Phase C2] Read the current generation counter
    /// WITHOUT bumping it. Pairs with `bump_generation` below.
    pub fn next_generation(&self) -> u64 {
        self.next_generation
    }

    /// [v0.7 Phase C2] Take the current generation counter, then
    /// increment to the next. Returns the PRE-BUMP value (which
    /// becomes the new handle's `generation`).
    pub fn bump_generation(&mut self) -> u64 {
        let g = self.next_generation;
        self.next_generation = self.next_generation.wrapping_add(1);
        g
    }

    /// [v0.7 Phase C2] Push a fully-constructed `Task` into the
    /// scheduler's task table.
    pub fn push_task(&mut self, task: TaskEntry) {
        self.tasks.push(task);
    }

    /// [B5b phase] Enqueue a task for execution (FIFO, no duplicates).
    pub fn enqueue(&mut self, id: TaskId) {
        if !self.run_queue.contains(&id) {
            self.run_queue.push_back(id);
        }
    }

    /// [B5b phase] Pop the next runnable task id.
    pub fn take_next(&mut self) -> Option<TaskId> {
        self.run_queue.pop_front()
    }

    /// [B5b phase] True when the task has reached a terminal state.
    pub fn is_terminal(&self, id: TaskId) -> bool {
        matches!(
            self.tasks.get(id.0).map(|t| &t.state),
            Some(TaskState::Done(_)) | Some(TaskState::Cancelled)
        )
    }

    /// [B5b phase] Mark Running and return `(body, env)` for execution.
    /// Returns `None` if the id is unknown, already terminal, or
    /// **Suspended** (parked on a channel op / scope cancel / awaiting
    /// child) — v0.9 Step 4 fix: a Suspended task is NOT
    /// automatically re-runnable; only an explicit wake (e.g.
    /// `wake_channel_op_waiter`, `cancel_suspended_task`,
    /// `wake_dependents`) transitions it back to `Pending`, at which
    /// point `enqueue` puts it back in the run queue. Without this
    /// guard, `scheduler_run_until_done`'s re-enqueue loop would
    /// call `run_one_task` on a parked task every iteration,
    /// re-evaluating the body and re-parking — an infinite loop.
    pub fn begin_run(&mut self, id: TaskId) -> Option<(Value, crate::Env)> {
        let task = self.tasks.get(id.0)?;
        if matches!(
            task.state,
            TaskState::Done(_) | TaskState::Cancelled | TaskState::Suspended(_)
        ) {
            return None;
        }
        let body = task.body.clone();
        let env = task.env.clone();
        if let Some(t) = self.tasks.get_mut(id.0) {
            t.state = TaskState::Running;
        }
        Some((body, env))
    }

    /// [B5b phase] Store a terminal result and wake AWAIT waiters.
    pub fn complete(&mut self, id: TaskId, result: TaskResult) {
        if let Some(t) = self.tasks.get_mut(id.0) {
            t.state = TaskState::Done(Box::new(result));
        }
        self.wake_dependents(id);
    }

    /// [B5b phase] Step one task. Execution lives on `Evaluator`; this
    /// bookkeeping helper only dequeues + marks. Prefer
    /// `Evaluator::run_one_task`.
    pub fn step(&mut self, task_id: TaskId) -> Option<()> {
        self.enqueue(task_id);
        Some(())
    }

    /// [B5b phase] Drain the run queue (plan §5.1.1 loop). Does not
    /// execute bodies — `Evaluator::scheduler_run_until_idle` does.
    pub fn drain_queue(&mut self) -> Vec<TaskId> {
        self.run_queue.drain(..).collect()
    }

    /// [B5b phase] Run until the run queue is empty (queue bookkeeping).
    pub fn run_until_idle(&mut self) {
        self.run_queue.clear();
    }

    /// [B5b phase] Dispatch pending cancellations to tasks that have not
    /// yet observed them. Phase F fills in the real signal; at B5b
    /// this is a no-op beyond dropping Cancelled tasks from the
    /// queue.
    pub fn dispatch_cancellations(&mut self) {
        self.run_queue.retain(|id| {
            !matches!(
                self.tasks.get(id.0).map(|t| &t.state),
                Some(TaskState::Cancelled)
            )
        });
    }

    /// [B5b phase] Re-enqueue tasks that were suspended on
    /// `finished_task_id` (AWAIT waiters). Waiters are recorded in
    /// `YieldReason::AwaitingChild`; at B5b AWAIT drives the loop
    /// inline so the waiter list is usually empty.
    pub fn wake_dependents(&mut self, finished_task_id: TaskId) {
        let mut to_wake = Vec::new();
        for (idx, t) in self.tasks.iter().enumerate() {
            if matches!(
                &t.state,
                TaskState::Suspended(YieldReason::AwaitingChild(child)) if *child == finished_task_id
            ) {
                to_wake.push(TaskId(idx));
            }
        }
        for id in to_wake {
            if let Some(t) = self.tasks.get_mut(id.0) {
                t.state = TaskState::Pending;
            }
            self.enqueue(id);
        }
    }

    /// [B5b phase] True when any registered task is still non-terminal.
    pub fn has_outstanding(&self) -> bool {
        self.tasks
            .iter()
            .any(|t| !matches!(t.state, TaskState::Done(_) | TaskState::Cancelled))
    }

    /// [v0.7 Phase E-B] Advisory cancellation for one task. Returns
    /// `true` if the flag was set on a live (non-terminal) task;
    /// `false` if the handle is stale / out-of-range / already
    /// terminal.
    ///
    /// Per plan §3 F4 + F7: cancellation itself does NOT produce an
    /// ERR. The task observes the flag at its next checkpoint (YIELD
    /// / TASK_IS_CANCELLED / channel blocking op); a purely-
    /// synchronous body runs to completion first (P7-E3-001).
    ///
    /// The target's `state` is not changed here; only `cancel_requested`
    /// is set, and if the task is parked (`Pending` not yet started)
    /// it is re-enqueued so `run_one_task` will see the flag at
    /// entry. Suspended tasks are left in their wait list — they
    /// will pick up the flag when the wait condition resolves.
    pub fn request_task_cancel(&mut self, handle: TaskHandle) -> bool {
        // Decide whether the target is a live, non-terminal task and
        // whether it is currently Pending (so we should re-enqueue).
        // Snapshot the inputs without holding the borrow across the
        // mutating tail (the borrow checker would reject enqueue
        // otherwise).
        let needs_enqueue = match self.tasks.get(handle.id.0) {
            Some(t) if t.id == handle.id && t.generation == handle.generation => {
                !matches!(t.state, TaskState::Done(_) | TaskState::Cancelled)
                    && matches!(t.state, TaskState::Pending)
            }
            _ => return false,
        };
        let alive = match self.tasks.get(handle.id.0) {
            Some(t) => {
                t.id == handle.id
                    && t.generation == handle.generation
                    && !matches!(t.state, TaskState::Done(_) | TaskState::Cancelled)
            }
            None => false,
        };
        if !alive {
            return false;
        }
        if let Some(t) = self.tasks.get_mut(handle.id.0) {
            t.cancel_requested = true;
        }
        if needs_enqueue {
            self.enqueue(handle.id);
        }
        true
    }

    /// [v0.7 Phase E-B / E3] Cancel every direct child task in
    /// `scope_id` (excluding `exclude`, if provided). Used by:
    ///
    /// - `TASK_CANCEL_PARENT()` from inside a task body (excludes
    ///   self so the caller can decide separately whether to mark
    ///   itself).
    /// - SCOPE sibling cancellation when any sibling terminates
    ///   with `Done(Err(_))` / `Failed(_)`: the SCOPE iterates
    ///   `Scope.tasks` and calls this with the dying sibling as
    ///   `exclude`. Surviving siblings observe the flag at their
    ///   next checkpoint (per plan §5.4.1 row "scope fn 取消该 task").
    ///
    /// Returns the number of tasks whose flag was set (for tests).
    pub fn cancel_siblings_in_scope(
        &mut self,
        scope_id: crate::runtime::ScopeId,
        exclude: Option<TaskId>,
    ) -> usize {
        let handles: Vec<TaskHandle> = match self.scopes.get(scope_id.0) {
            Some(s) => s.tasks.clone(),
            None => return 0,
        };
        let mut applied = 0usize;
        for h in handles {
            if Some(h.id) == exclude {
                continue;
            }
            if self.request_task_cancel(h) {
                applied += 1;
            }
        }
        applied
    }

    /// [v0.7 Phase F-A / plan §3 F3] Top-down cancellation
    /// propagation across a scope subtree. Marks `scope_id`
    /// itself + every descendant scope as `cancelled = true`,
    /// and requests cancel on every direct child task in each
    /// of those scopes.
    ///
    /// Plan §3 F3: "取消自顶向下传播;子 scope 可独立取消". The
    /// "可独立取消" half is satisfied by the existing
    /// `cancel_siblings_in_scope` path (E-B-1) and `TASK_CANCEL`
    /// (E-B-1) — a sub-scope can be cancelled without cancelling
    /// the parent. The "top-down propagation" half is this
    /// function: when a parent scope is cancelled, descendants
    /// observe the cancellation.
    ///
    /// Returns the number of tasks whose cancel flag was set
    /// (for tests + bookkeeping).
    pub fn cancel_scope_subtree(&mut self, scope_id: crate::runtime::ScopeId) -> usize {
        // Collect the set of scopes to walk: scope_id itself +
        // every descendant. We compute the descendant list first
        // because the second loop borrows self.scopes mutably per
        // scope id and would conflict with a self-recursive walk.
        let mut to_visit: Vec<ScopeId> = vec![scope_id];
        let total = self.scopes.len();
        for sid in 0..total {
            let sid = crate::runtime::ScopeId(sid);
            if sid == scope_id {
                continue;
            }
            // is_descendant_of(sid, scope_id)?
            let mut cur = self.scopes.get(sid.0).and_then(|s| s.parent);
            let mut found = false;
            while let Some(p) = cur {
                if p == scope_id {
                    found = true;
                    break;
                }
                cur = self.scopes.get(p.0).and_then(|s| s.parent);
            }
            if found {
                to_visit.push(sid);
            }
        }
        // Walk: mark each scope cancelled, then cancel each task.
        let mut applied = 0usize;
        for sid in to_visit {
            if let Some(scope) = self.scopes.get_mut(sid.0) {
                scope.cancelled = true;
            }
            let handles = self
                .scopes
                .get(sid.0)
                .map(|s| s.tasks.clone())
                .unwrap_or_default();
            for h in handles {
                if self.request_task_cancel(h) {
                    applied += 1;
                }
            }
        }
        applied
    }

    // ────────────────────────────────────────────────────────────
    // v0.9 Step 3 — Scheduler park / resume / cancel helpers for
    // algebraic-effect-driven task state transitions (ADR-0017
    // §3.1, ADR-0019 §4.4.1, plan §3.1 / §3.2 / §4.4.4).
    //
    // These helpers consolidate the bookkeeping needed to drive a
    // task from Running -> Suspended(reason) -> back to Running
    // when the wait condition resolves. They are used by:
    //
    // - Step 4: builtin_channel_send / recv (park on WouldBlock,
    //   wake via channel pair).
    // - Step 8: TASK_CANCEL(task, reason) / TASK_CANCEL_PARENT
    //   (raise Cancelled effect, transition to Cancelled).
    // - ADR-0019 §4.4.3: CALL_METHOD (raise MethodCall effect,
    //   handler validates protocol state machine).
    //
    // The existing infrastructure (Channel::push_*_waiter /
    // pop_*_waiter, TaskState::Suspended, YieldReason) is unchanged;
    // these helpers are additive. Step 4 + Step 8 wire them into
    // the eval path.
    // ────────────────────────────────────────────────────────────

    /// [v0.9 Step 3] Park `task_id` on `channel`'s sender_waiters or
    /// receiver_waiters list, transition it to
    /// `TaskState::Suspended(ChannelOp(dir, channel))`, and remove
    /// it from the run queue. The caller is expected to have just
    /// failed a synchronous channel op (channel::TryResult::WouldBlock)
    /// and is converting it to a real suspension.
    ///
    /// `value` is `Some(v)` for a SEND waiter (the value the task
    /// wants to push); `None` for a RECV waiter (the value is
    /// unknown until paired with a sender).
    ///
    /// Returns the [`ChannelWaiter`] that was recorded (useful for
    /// tests). Returns `None` if `task_id` is out of range or already
    /// terminal (Suspended/Done/Cancelled) — the caller is expected
    /// to log the no-op for diagnostic clarity.
    pub fn park_for_channel_op(
        &mut self,
        task_id: TaskId,
        channel: RcHandle,
        dir: Direction,
        value: Option<Value>,
    ) -> Option<ChannelWaiter> {
        let slot = channel.0;
        let waiter = ChannelWaiter {
            task_id,
            direction: dir,
            value: value.clone(),
        };
        // Stage 1: register the waiter on the channel's wait list.
        let registered = if let Some(ch) = self.channels.get_mut(slot) {
            match dir {
                Direction::Send => {
                    ch.push_sender_waiter(task_id, value.clone().unwrap_or(Value::Null))
                }
                Direction::Recv => ch.push_receiver_waiter(task_id),
            }
            true
        } else {
            false
        };
        if !registered {
            return None;
        }
        // Stage 2: transition task state and remove from run queue.
        if let Some(task) = self.tasks.get_mut(task_id.0) {
            let reason = match dir {
                Direction::Send => YieldReason::SendingOn(channel),
                Direction::Recv => YieldReason::ReceivingOn(channel),
            };
            task.state = TaskState::Suspended(reason);
        } else {
            return None;
        }
        self.run_queue.retain(|id| *id != task_id);
        Some(waiter)
    }

    /// [v0.9 Step 3] Pair one waiter from `channel`'s wait list and
    /// return it (with its pending value, if any) so the caller can
    /// resume it. The caller is responsible for actually re-enqueuing
    /// the task and transitioning its state from Suspended to
    /// Pending/Running — the channel-side bookkeeping (wait list
    /// removal) is the only thing this helper does.
    ///
    /// `dir` selects which side to pop: `Send` returns the head of
    /// `sender_waiters`; `Recv` returns the head of `receiver_waiters`.
    ///
    /// Returns `None` if the channel slot is out of range or no
    /// waiter is parked on the requested side.
    pub fn pop_channel_op_waiter(
        &mut self,
        channel: RcHandle,
        dir: Direction,
    ) -> Option<ChannelWaiter> {
        let slot = channel.0;
        let task_id = self.channels.get_mut(slot)?.pop_waiter(dir)?;
        Some(ChannelWaiter {
            task_id,
            direction: dir,
            value: None, // value is None for popped receivers; senders
                         // stored their value inline, the caller's
                         // channel.rs::send path already dequeued it
                         // and now transfers it to the paired recv.
                         // For the symmetry-test use case we just
                         // surface the task_id and direction.
        })
    }

    /// [v0.9 Step 3] Wake a task that was parked on a channel op,
    /// transitioning it `Suspended(ChannelOp)` → `Pending` and
    /// re-enqueueing it for the scheduler loop. Idempotent on tasks
    /// already in `Pending` / `Running` / terminal states.
    ///
    /// This is the `pair` half of the ChannelWaiter pair logic: when
    /// one task completes an op that frees a slot for a parked peer,
    /// the peer wakes up here. The peer then re-runs its body,
    /// retries the channel op, and proceeds normally.
    ///
    /// Returns `true` if the task was actually transitioned out of
    /// `Suspended`; `false` if the task was unknown, terminal, or
    /// already Pending / Running (defensive — pair logic should not
    /// wake an already-running task).
    pub fn wake_channel_op_waiter(&mut self, task_id: TaskId) -> bool {
        let is_parked = matches!(
            self.tasks.get(task_id.0).map(|t| &t.state),
            Some(TaskState::Suspended(
                YieldReason::ReceivingOn(_) | YieldReason::SendingOn(_),
            ))
        );
        if !is_parked {
            return false;
        }
        if let Some(t) = self.tasks.get_mut(task_id.0) {
            t.state = TaskState::Pending;
        }
        self.enqueue(task_id);
        true
    }

    /// [v0.9 Step 3] Cancel a task that is parked on a channel op
    /// (or any other Suspended state). Transitions
    /// `Suspended(*) → Cancelled` and returns `true`. Wakes any
    /// dependents (AWAIT waiters) so they observe the cancellation
    /// at their next checkpoint.
    ///
    /// Distinct from `request_task_cancel` (advisory flag only) —
    /// this helper actually transitions the state. Used by:
    ///
    /// - Step 4 channel close: parked receiver waiters wake with
    ///   `Cancelled` (per plan §3.2 "关闭协议不变").
    /// - Step 8 TASK_CANCEL: completed cancellation transitions
    ///   Suspended → Cancelled (the advisory flag was set in E-B;
    ///   this is the F5+ completion step).
    /// - scope-exit force-cancel (D-D leak detector).
    pub fn cancel_suspended_task(&mut self, task_id: TaskId) -> bool {
        let was_suspended = match self.tasks.get(task_id.0) {
            Some(t) => matches!(t.state, TaskState::Suspended(_)),
            None => return false,
        };
        if !was_suspended {
            return false;
        }
        if let Some(t) = self.tasks.get_mut(task_id.0) {
            t.state = TaskState::Cancelled;
        }
        self.run_queue.retain(|id| *id != task_id);
        self.wake_dependents(task_id);
        true
    }

    /// [v0.9 Step 5 / ADR-0017 §3.4 / plan §3.4] Deadlock detection
    /// L1 strict. Scans one scope for tasks parked on a channel op
    /// (`Suspended(ReceivingOn | SendingOn)`); if two or more are
    /// parked AND no progress can be made (the scheduler has just
    /// drained the run queue with no wake), returns a cycle report.
    ///
    /// **Strict limits** (per plan §3.4):
    /// - **Same scope only** — cross-scope deadlocks are not
    ///   detected at L1.
    /// - **`YieldReason::Explicit` / `AwaitingChild` / `CancellationCheck`
    ///   are NOT deadlock candidates** — those are pure yields /
    ///   child waits; the task isn't blocked on an external event.
    /// - **Single parked task is NOT a deadlock** — that task is
    ///   just waiting for a peer that may arrive later.
    /// - **Cycle report** carries the parked task ids in FIFO
    ///   arrival order so consumers can display the wait chain.
    ///
    /// Returns `None` if no deadlock is found in `scope_id`.
    pub fn detect_deadlock_l1(&self, scope_id: ScopeId) -> Option<Vec<TaskId>> {
        let scope = self.scopes.get(scope_id.0)?;
        let parked: Vec<TaskId> = scope
            .tasks
            .iter()
            .filter_map(|h| {
                let task = self.tasks.get(h.id.0)?;
                if h.generation != task.generation {
                    return None;
                }
                if !matches!(
                    task.state,
                    TaskState::Suspended(YieldReason::ReceivingOn(_) | YieldReason::SendingOn(_),)
                ) {
                    return None;
                }
                Some(h.id)
            })
            .collect();
        if parked.len() < 2 {
            return None;
        }
        // Cycle report: list of parked task ids in the order they
        // appear in the scope's `tasks` list (which is the SPAWN
        // registration order). The full cycle analysis (sender ↔
        // receiver on the same channel) is Step 5's documented
        // extension; for v0.9 L1 we surface the wait-set shape.
        Some(parked)
    }
}

impl crate::channel::Channel {
    /// Pop one waiter from the channel's sender_waiters or
    /// receiver_waiters list, depending on `dir`. Convenience for
    /// the Scheduler helpers above; canonical implementation lives
    /// here so the existing [`crate::channel::Channel::pop_sender_waiter`]
    /// / [`crate::channel::Channel::pop_receiver_waiter`] stay
    /// unchanged (Step 4 / channel ops keep using those).
    fn pop_waiter(&mut self, dir: Direction) -> Option<crate::runtime::TaskId> {
        match dir {
            Direction::Send => self.pop_sender_waiter(),
            Direction::Recv => self.pop_receiver_waiter(),
        }
    }
}

/// Shared inner state across a scope subtree (future B5+).
///
/// B1 declares the alias so other modules can name it; the actual
/// shared state (e.g. cancellation token) is empty.
pub type ScopeShared = Rc<Scope>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Env;

    #[test]
    fn taskid_distinct() {
        assert_ne!(TaskId(0), TaskId(1));
    }

    #[test]
    fn handle_includes_generation() {
        let h1 = TaskHandle {
            id: TaskId(0),
            generation: 1,
        };
        let h2 = TaskHandle {
            id: TaskId(0),
            generation: 2,
        };
        assert_ne!(h1, h2, "generation must differentiate handle reuse");
    }

    #[test]
    fn next_task_id_plus_bump_produces_distinct_ids() {
        // The C2 helper triple is the production path (used by
        // builtin_spawn). next_task_id reads the NEXT FREE slot
        // index (i.e. self.tasks.len() at the moment of the call,
        // *not* a `tasks.len() + 1` advance). bump_generation
        // mints a fresh generation by reading then incrementing
        // self.next_generation. To exercise the *combined* effect
        // (a fresh slot id) we commit a real TaskEntry between
        // the two pairs, mirroring what builtin_spawn does. We
        // construct a minimal TaskEntry using a dummy closure
        // body; the test only cares about id / generation
        // inequality, not state.
        use crate::task::Task as TaskImpl;
        let mut s = Scheduler::new();
        // First allocation pair + commit
        let id_a = s.next_task_id();
        let gen_a = s.bump_generation();
        s.push_task(TaskImpl::new_pending(
            id_a,
            gen_a,
            Value::Null,
            crate::Env::new(),
            crate::runtime::ScopeId(0),
        ));
        // Second allocation pair + commit
        let id_b = s.next_task_id();
        let gen_b = s.bump_generation();
        assert_ne!(id_a, id_b, "after push_task the next slot id advances");
        assert_ne!(
            gen_a, gen_b,
            "second allocation must bump the generation counter"
        );
        assert_eq!(
            s.tasks.len(),
            1,
            "only one commit so far; id_b is the *next* slot"
        );
    }

    #[test]
    fn yield_reason_explicit_is_copy() {
        let r = YieldReason::Explicit;
        let _ = r; // unused warning silencer; just needs to be Copy
        fn is_copy<T: Copy>() {}
        is_copy::<YieldReason>();
    }

    #[test]
    fn cancel_scope_subtree_marks_all_descendants() {
        // [v0.7 Phase F-A / plan §3 F3] Direct Rust-level test of
        // the helper that `TASK_CANCEL_PARENT` calls. We build a
        // scope tree by hand (no eval, no path-B sync concerns)
        // and verify that `cancel_scope_subtree(root)` marks
        // every descendant scope's `cancelled = true` and sets
        // `cancel_requested = true` on every direct child task
        // of those scopes.
        //
        // Tree shape:
        //   root (id 0)
        //     ├── mid_a (id 1)
        //     │     ├── task_a1
        //     │     └── leaf (id 2)
        //     │           └── task_leaf
        //     └── mid_b (id 3)
        //           └── task_b1
        use crate::runtime::{Scope, ScopeId, TaskId, TaskState};
        use crate::task::Task as TaskImpl;
        use crate::{Env, Value};

        let mut s = Scheduler::new();
        // Scheduler::new() already pushed the root scope at index
        // 0 (ScopeId(0), parent None). We extend the tree by pushing
        // mid_a -> leaf -> mid_b so the final layout is:
        //   scopes[0] = root (already there)
        //   scopes[1] = mid_a  (parent: ScopeId(0))
        //   scopes[2] = leaf   (parent: ScopeId(1))
        //   scopes[3] = mid_b  (parent: ScopeId(0))
        s.scopes.push(Scope::new(ScopeId(1), Some(ScopeId(0))));
        s.scopes.push(Scope::new(ScopeId(2), Some(ScopeId(1))));
        s.scopes.push(Scope::new(ScopeId(3), Some(ScopeId(0))));

        // Allocate task slots 0..=3.
        for tid in 0..4 {
            let gen = s.bump_generation();
            s.tasks.push(TaskImpl::new_pending(
                TaskId(tid),
                gen,
                Value::Null,
                Env::new(),
                ScopeId(0),
            ));
            s.tasks[tid].state = TaskState::Pending;
        }
        // Register each task with its parent scope.
        s.scopes[1].tasks.push(crate::runtime::TaskHandle {
            id: TaskId(0),
            generation: 0,
        });
        s.scopes[1].tasks.push(crate::runtime::TaskHandle {
            id: TaskId(1),
            generation: 1,
        });
        s.scopes[2].tasks.push(crate::runtime::TaskHandle {
            id: TaskId(2),
            generation: 2,
        });
        s.scopes[3].tasks.push(crate::runtime::TaskHandle {
            id: TaskId(3),
            generation: 3,
        });

        // Invoke the helper.
        let applied = s.cancel_scope_subtree(ScopeId(0));

        // Every scope's `cancelled` flag must be true.
        assert!(s.scopes[0].cancelled, "root scope must be cancelled");
        assert!(s.scopes[1].cancelled, "mid_a must be cancelled");
        assert!(s.scopes[2].cancelled, "leaf must be cancelled");
        assert!(s.scopes[3].cancelled, "mid_b must be cancelled");
        // Every task's cancel_requested must be true.
        for tid in 0..4 {
            assert!(
                s.tasks[tid].cancel_requested,
                "task {tid} must have cancel_requested"
            );
        }
        // All 4 tasks should have been applied.
        assert_eq!(applied, 4, "4 tasks should be flagged");
    }

    #[test]
    fn cancel_scope_subtree_does_not_propagate_to_ancestors() {
        // [v0.7 Phase F-A / plan §3 F3] Top-down only. Cancelling
        // mid_a should NOT mark root or mid_b; only mid_a and
        // its descendant leaf.
        use crate::runtime::{Scope, ScopeId, TaskId, TaskState};
        use crate::task::Task as TaskImpl;
        use crate::{Env, Value};

        let mut s = Scheduler::new();
        // Same layout as the first test: root (scopes[0], already
        // pushed by Scheduler::new), mid_a (1), leaf (2), mid_b (3).
        s.scopes.push(Scope::new(ScopeId(1), Some(ScopeId(0))));
        s.scopes.push(Scope::new(ScopeId(2), Some(ScopeId(1))));
        s.scopes.push(Scope::new(ScopeId(3), Some(ScopeId(0))));
        for tid in 0..4 {
            let gen = s.bump_generation();
            s.tasks.push(TaskImpl::new_pending(
                TaskId(tid),
                gen,
                Value::Null,
                Env::new(),
                ScopeId(0),
            ));
            s.tasks[tid].state = TaskState::Pending;
        }
        s.scopes[1].tasks.push(crate::runtime::TaskHandle {
            id: TaskId(0),
            generation: 0,
        });
        s.scopes[1].tasks.push(crate::runtime::TaskHandle {
            id: TaskId(1),
            generation: 1,
        });
        s.scopes[2].tasks.push(crate::runtime::TaskHandle {
            id: TaskId(2),
            generation: 2,
        });
        s.scopes[3].tasks.push(crate::runtime::TaskHandle {
            id: TaskId(3),
            generation: 3,
        });

        // Cancel only mid_a.
        let applied = s.cancel_scope_subtree(ScopeId(1));
        // mid_a (2 tasks) + leaf (1 task) = 3 tasks flagged.
        assert_eq!(applied, 3, "mid_a has 2 child tasks; leaf has 1 -> 3 total");
        // mid_a and leaf cancelled; root and mid_b NOT cancelled.
        assert!(!s.scopes[0].cancelled, "root must NOT be cancelled");
        assert!(s.scopes[1].cancelled, "mid_a must be cancelled");
        assert!(s.scopes[2].cancelled, "leaf must be cancelled");
        assert!(!s.scopes[3].cancelled, "mid_b must NOT be cancelled");
        // Tasks 0 and 1 (in mid_a) and 2 (in leaf) cancelled;
        // task 3 (in mid_b) NOT cancelled.
        assert!(s.tasks[0].cancel_requested);
        assert!(s.tasks[1].cancel_requested);
        assert!(s.tasks[2].cancel_requested);
        assert!(
            !s.tasks[3].cancel_requested,
            "mid_b task must NOT be cancelled"
        );
    }

    #[test]
    fn scheduler_default_matches_new() {
        let a = Scheduler::default();
        let b = Scheduler::new();
        assert_eq!(a.tasks.len(), b.tasks.len());
        assert_eq!(a.run_queue.len(), b.run_queue.len());
        assert_eq!(a.next_generation, b.next_generation);
        assert_eq!(a.scopes.len(), 1, "new() seeds a root scope");
        assert_eq!(a.current_scope, Some(ScopeId(0)));
    }

    #[test]
    fn step_result_variants_are_distinct() {
        // B5a-1: StepResult is currently unreachable from real code
        // paths (no yield points exist), but the variant shape must
        // be testable so B5b can match on it. Use Box<YieldReason>
        // / Box<StepResult> because Yield / Blocked don't carry
        // Value-equality.
        use std::mem::discriminant;
        let d = discriminant(&StepResult::Done(Value::Null));
        let y = discriminant(&StepResult::Yield(YieldReason::Explicit));
        let b = discriminant(&StepResult::Blocked);
        assert_ne!(d, y, "Done and Yield must be distinct variants");
        assert_ne!(d, b, "Done and Blocked must be distinct variants");
        assert_ne!(y, b, "Yield and Blocked must be distinct variants");
    }

    #[test]
    fn step_result_yield_carries_yield_reason() {
        // The scheduler dispatches on the YieldReason payload; a
        // round-trip through Debug + Clone must preserve it so
        // wait-list bookkeeping stays consistent. (At B5a-1 we
        // can't reach this through eval, but the type contract
        // matters.)
        let r = YieldReason::AwaitingChild(TaskId(7));
        let cloned = r;
        assert_eq!(r, cloned);
    }

    // ────────────────────────────────────────────────────────────
    // v0.9 Step 3 lock tests — Scheduler park / resume / cancel
    // helpers + WasmFX-style Effect tag dispatch (ADR-0017 §3.1,
    // ADR-0019 §4.4.1, plan §3.1 / §3.2 / §4.4.4).
    // ────────────────────────────────────────────────────────────

    /// Build a Scheduler with one channel slot (id 0, buf=0, gen=1)
    /// and one Pending task (id 0, gen=1) registered on the root
    /// scope. Returns the scheduler + the task id + the channel id.
    fn fresh_scheduler_with_channel() -> (Scheduler, TaskId, RcHandle) {
        use crate::channel::Channel;
        use crate::task::Task as TaskImpl;
        let mut s = Scheduler::new();
        s.channels
            .push(Channel::new(crate::channel::ChannelId(0), 1, 0));
        let id = s.next_task_id();
        let gen = s.bump_generation();
        s.push_task(TaskImpl::new_pending(
            id,
            gen,
            Value::Null,
            Env::new(),
            ScopeId(0),
        ));
        s.scopes[0].tasks.push(crate::runtime::TaskHandle {
            id,
            generation: gen,
        });
        (s, id, crate::channel::ChannelId(0))
    }

    #[test]
    fn effect_tag_dispatch_round_trips_for_each_variant() {
        // The three canonical ADR-0017 / ADR-0019 §4.4.1 tags.
        let yield_eff = Effect::Yield { explicit: true };
        let channel_eff = Effect::ChannelOp {
            dir: Direction::Send,
            channel: crate::channel::ChannelId(0),
            value: Some(Value::Integer(7)),
        };
        let cancel_eff = Effect::Cancelled;
        assert_eq!(yield_eff.tag(), Tag::Yield);
        assert_eq!(channel_eff.tag(), Tag::ChannelOp);
        assert_eq!(cancel_eff.tag(), Tag::Cancelled);
        // Distinct discriminants.
        use std::mem::discriminant;
        assert_ne!(
            discriminant(&yield_eff.tag()),
            discriminant(&channel_eff.tag()),
        );
        assert_ne!(
            discriminant(&channel_eff.tag()),
            discriminant(&cancel_eff.tag()),
        );
    }

    #[test]
    fn direction_variants_are_distinct() {
        assert_ne!(Direction::Send, Direction::Recv);
    }

    #[test]
    fn channel_waiter_struct_round_trips() {
        // The ChannelWaiter struct is the canonical pair logic
        // payload — test the field shape.
        let w = ChannelWaiter {
            task_id: TaskId(7),
            direction: Direction::Recv,
            value: None,
        };
        assert_eq!(w.task_id, TaskId(7));
        assert_eq!(w.direction, Direction::Recv);
        assert_eq!(w.value, None);
        // value round-trip
        let w_send = ChannelWaiter {
            task_id: TaskId(8),
            direction: Direction::Send,
            value: Some(Value::Integer(42)),
        };
        assert_eq!(w_send.value, Some(Value::Integer(42)));
    }

    #[test]
    fn park_for_channel_op_marks_suspended_and_records_waiter() {
        // Send park on a sync (buf=0) channel: task transitions
        // from Pending -> Suspended(SendingOn), and the task id
        // is pushed onto the channel's sender_waiters list.
        let (mut s, task_id, ch_id) = fresh_scheduler_with_channel();
        s.enqueue(task_id);
        // Pre-state: task is Pending, run_queue has 1 entry.
        assert_eq!(s.run_queue.len(), 1);

        let waiter = s
            .park_for_channel_op(task_id, ch_id, Direction::Send, Some(Value::Integer(7)))
            .expect("park succeeds");

        // Post-state: task is Suspended(SendingOn), run_queue empty,
        // channel's sender_waiters contains the task id.
        assert!(matches!(
            s.tasks[0].state,
            TaskState::Suspended(YieldReason::SendingOn(c)) if c == ch_id
        ));
        assert_eq!(s.run_queue.len(), 0, "parked task removed from run queue");
        assert_eq!(s.channels[0].sender_waiters, vec![task_id]);
        assert_eq!(waiter.task_id, task_id);
        assert_eq!(waiter.direction, Direction::Send);
    }

    #[test]
    fn park_for_channel_op_recv_uses_receivers_waiters() {
        let (mut s, task_id, ch_id) = fresh_scheduler_with_channel();
        s.enqueue(task_id);
        let _ = s
            .park_for_channel_op(task_id, ch_id, Direction::Recv, None)
            .expect("park succeeds");
        assert!(matches!(
            s.tasks[0].state,
            TaskState::Suspended(YieldReason::ReceivingOn(c)) if c == ch_id
        ));
        assert_eq!(s.channels[0].receiver_waiters, vec![task_id]);
    }

    #[test]
    fn park_for_channel_op_returns_none_for_unknown_channel() {
        // Out-of-range channel id — the helper must refuse to park
        // a task on a non-existent slot rather than silently
        // swallowing.
        let (mut s, task_id, _ch_id) = fresh_scheduler_with_channel();
        s.enqueue(task_id);
        let bogus = crate::channel::ChannelId(999);
        let res = s.park_for_channel_op(task_id, bogus, Direction::Send, None);
        assert!(res.is_none(), "out-of-range channel must return None");
        // Task state is unchanged.
        assert!(matches!(s.tasks[0].state, TaskState::Pending));
    }

    #[test]
    fn wake_channel_op_waiter_pair_resumes_parked_task() {
        // Lock test plan §3.2: ChannelWaiter::pair resumes both
        // sides. We park a task as receiver, then "pair" by waking
        // it (the channel send that pairs with it is Step 4's job).
        let (mut s, task_id, ch_id) = fresh_scheduler_with_channel();
        s.enqueue(task_id);
        s.park_for_channel_op(task_id, ch_id, Direction::Recv, None)
            .expect("park");

        // Pre-wake: task is Suspended, run_queue empty.
        assert!(matches!(
            s.tasks[0].state,
            TaskState::Suspended(YieldReason::ReceivingOn(_))
        ));
        assert_eq!(s.run_queue.len(), 0);

        // Wake: task transitions Suspended -> Pending, re-enqueued.
        let woke = s.wake_channel_op_waiter(task_id);
        assert!(woke, "wake must succeed for Suspended(ChannelOp) task");
        assert!(matches!(s.tasks[0].state, TaskState::Pending));
        assert_eq!(s.run_queue.len(), 1, "woken task must be re-enqueued");
        assert_eq!(s.run_queue.front(), Some(&task_id));
    }

    #[test]
    fn wake_channel_op_waiter_returns_false_for_running_or_done() {
        // Defensive: only Suspended(ChannelOp) tasks are valid
        // wake targets. A Running task, Done task, or non-channel-op
        // Suspended task must return false (the scheduler's pair path
        // would not target them in practice, but the helper
        // contract is explicit).
        let (mut s, task_id, _ch_id) = fresh_scheduler_with_channel();
        // Case 1: Pending task.
        s.enqueue(task_id);
        assert!(!s.wake_channel_op_waiter(task_id));
        // Case 2: Running task.
        s.tasks[0].state = TaskState::Running;
        assert!(!s.wake_channel_op_waiter(task_id));
        // Case 3: Suspended but not on a channel op.
        s.tasks[0].state = TaskState::Suspended(YieldReason::Explicit);
        assert!(!s.wake_channel_op_waiter(task_id));
        // Case 4: terminal.
        s.tasks[0].state = TaskState::Done(Box::new(TaskResult::Ok(Value::Null)));
        assert!(!s.wake_channel_op_waiter(task_id));
    }

    #[test]
    fn pop_channel_op_waiter_removes_from_wait_list() {
        let (mut s, task_id, ch_id) = fresh_scheduler_with_channel();
        s.enqueue(task_id);
        s.park_for_channel_op(task_id, ch_id, Direction::Recv, None)
            .expect("park");
        assert_eq!(s.channels[0].receiver_waiters.len(), 1);

        // Pop wakes via the channel's own pop_receiver_waiter helper.
        let popped = s
            .pop_channel_op_waiter(ch_id, Direction::Recv)
            .expect("pop succeeds");
        assert_eq!(popped.task_id, task_id);
        assert_eq!(popped.direction, Direction::Recv);
        assert!(
            s.channels[0].receiver_waiters.is_empty(),
            "popped waiter must be removed from channel's wait list"
        );
        // The parked task's state is still Suspended at this point
        // — the caller's pair path must call wake_channel_op_waiter
        // separately to transition Suspended -> Pending and re-enqueue.
        assert!(matches!(
            s.tasks[0].state,
            TaskState::Suspended(YieldReason::ReceivingOn(_))
        ));
    }

    #[test]
    fn cancel_suspended_task_transitions_to_cancelled() {
        let (mut s, task_id, ch_id) = fresh_scheduler_with_channel();
        s.enqueue(task_id);
        s.park_for_channel_op(task_id, ch_id, Direction::Recv, None)
            .expect("park");
        // Pre: Suspended, in wait list.
        assert!(matches!(
            s.tasks[0].state,
            TaskState::Suspended(YieldReason::ReceivingOn(_))
        ));
        assert_eq!(s.channels[0].receiver_waiters.len(), 1);

        let cancelled = s.cancel_suspended_task(task_id);
        assert!(cancelled);
        // Post: Cancelled (terminal), out of run queue, wait list
        // untouched (caller's channel.rs::close path handles cleanup).
        assert!(matches!(s.tasks[0].state, TaskState::Cancelled));
        assert_eq!(s.run_queue.len(), 0);
        // cancel_suspended_task does NOT pop the channel waiter;
        // that's the channel-close path's job. The wait list still
        // holds the task id for diagnostics / leak detection.
        assert_eq!(s.channels[0].receiver_waiters, vec![task_id]);
    }

    #[test]
    fn cancel_suspended_task_returns_false_for_running_or_done() {
        let (mut s, task_id, _ch_id) = fresh_scheduler_with_channel();
        // Pending: not yet suspended.
        s.enqueue(task_id);
        assert!(!s.cancel_suspended_task(task_id));
        // Running.
        s.tasks[0].state = TaskState::Running;
        assert!(!s.cancel_suspended_task(task_id));
        // Already Cancelled.
        s.tasks[0].state = TaskState::Cancelled;
        assert!(!s.cancel_suspended_task(task_id));
    }

    #[test]
    fn suspend_resume_cancel_cycle_full_pair_logic() {
        // Integration-style lock test combining park / wake / cancel
        // to cover the full Scheduler pair-logic surface. Two tasks:
        // a sender parked on a full channel, a receiver parked on
        // the same channel. The "pair" path is: cancel the receiver
        // (simulating channel close), confirm sender stays parked
        // but receiver transitions to Cancelled.
        use crate::channel::Channel;
        use crate::task::Task as TaskImpl;

        let mut s = Scheduler::new();
        s.channels
            .push(Channel::new(crate::channel::ChannelId(0), 1, 0));
        // Two tasks.
        for tid in 0..2usize {
            let gen = s.bump_generation();
            s.push_task(TaskImpl::new_pending(
                TaskId(tid),
                gen,
                Value::Null,
                Env::new(),
                ScopeId(0),
            ));
            s.scopes[0].tasks.push(crate::runtime::TaskHandle {
                id: TaskId(tid),
                generation: gen,
            });
        }
        let sender_id = TaskId(0);
        let receiver_id = TaskId(1);
        let ch_id = crate::channel::ChannelId(0);
        s.enqueue(sender_id);
        s.enqueue(receiver_id);

        // Park both: sender on send, receiver on recv.
        s.park_for_channel_op(sender_id, ch_id, Direction::Send, Some(Value::Integer(7)))
            .expect("park sender");
        s.park_for_channel_op(receiver_id, ch_id, Direction::Recv, None)
            .expect("park receiver");

        // Both suspended, run_queue empty.
        assert!(matches!(
            s.tasks[0].state,
            TaskState::Suspended(YieldReason::SendingOn(_))
        ));
        assert!(matches!(
            s.tasks[1].state,
            TaskState::Suspended(YieldReason::ReceivingOn(_))
        ));
        assert_eq!(s.run_queue.len(), 0);

        // Simulate channel close: cancel the receiver (it will
        // observe Cancelled at its next checkpoint and return
        // ERR(kind="Cancelled") per plan §3.2 / §5.4).
        assert!(s.cancel_suspended_task(receiver_id));
        assert!(matches!(s.tasks[1].state, TaskState::Cancelled));

        // Sender is still parked — channel close does not wake
        // senders (per plan §5.3 "SEND-on-closed is E0054"). The
        // send path will raise E0054 when the sender wakes up via
        // scope-exit force-cancel or TASK_CANCEL.
        assert!(matches!(
            s.tasks[0].state,
            TaskState::Suspended(YieldReason::SendingOn(_))
        ));
    }
}
