//! [v0.7 Phase B1] cooperative coroutine runtime skeleton.
//!
//! **Types only at this stage** -- no scheduler behaviour, no
//! state-machine conversion of `eval_expr`. The shape is fixed in
//! B1 so B4 (Task / Scope data) and B5 (Scheduler loop + CPS eval
//! conversion) can build on it without forward-declaration gymnastics.
//!
//! See plan §5.1, §5.1.1, §5.2, ADR-0014, ADR-0016 for the full design.
pub use wlwl_value::{RcHandle, Tag, TaskHandle, TaskId, YieldReason};

use std::collections::VecDeque;
use std::rc::Rc;

use wlwl_error::WlwlError;

use crate::{Expr, Value};

/// Identifies a task slot within a single [`Scheduler`].
///
/// Cheap to copy; not a handle (see [`TaskHandle`] for what `SPAWN`
/// returns to user code).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ScopeId(pub usize);

/// Identifies a channel slot within a single [`Scheduler`].
///
/// Re-exported alias to the full [`crate::channel::ChannelId`] type;
/// Task lifecycle state (plan §5.1.1).
///
/// Transitions:
/// - `Pending`    -- on `Scheduler::resume(task_id)` --&gt; `Running`
/// - `Running`    -- yields via [`YieldReason`] --&gt; `Suspended { tag, reason }`
/// - `Running`    -- body returns `Ok(v)` or `Err(e)` --&gt; `Done(...)`
/// - `Running`    -- parent scope cancelled --&gt; `Cancelled`
/// - `Suspended`  -- waiting event ready --&gt; `Running`
/// - `Suspended`  -- parent scope cancelled --&gt; `Cancelled`
/// - `Done` / `Cancelled` are terminal.
///
/// v0.9 Step 10 (plan §9.1): `Suspended` uses WasmFX-style
/// `Suspended { tag, payload }` field naming (ADR-0019 §4.4.4,
/// §4.4.1). `tag` mirrors [`YieldReason::tag`] and is the
/// algebraic-effect dispatch key; `reason` is the internal
/// scheduler bookkeeping carried over from v0.8. Construct via
/// [`TaskState::suspended`] so the two fields cannot drift apart.
#[derive(Debug, Clone)]
pub enum TaskState {
    Pending,
    Running,
    Suspended {
        /// WasmFX-style dispatch tag. Derived from `reason` via
        /// [`YieldReason::tag`]; callers must use
        /// [`TaskState::suspended`] to keep the pair consistent.
        tag: Tag,
        /// Internal scheduler wait condition.
        reason: YieldReason,
    },
    Done(Box<TaskResult>),
    Cancelled,
}

/// [X-02b / S1] 取消请求是否**豁免**于 `SHIELD` 的推迟。
///
/// 规范并发册 §17.3:「`SHIELD(fn)`: 调用 `fn` 并**屏蔽屏蔽块内收到的取消
/// 请求**」⇒ 默认(`Deferrable`)**按目标的状态**判定:目标当前在 `SHIELD`
/// 块内 ⇒ 推迟到它退出自己的屏蔽块。
///
/// 只有**作用域退出**这种**生命周期边界**才豁免(`Bypass`)—— 否则被屏蔽
/// 的子任务会活过 `SCOPE`,违反「子任务寿命不超过词法 `SCOPE`」那条结构化
/// 并发不变量(§6.7 的 S4)。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShieldPolicy {
    /// 受屏蔽保护(默认):目标在 `SHIELD` 块内 ⇒ 推迟。
    Deferrable,
    /// 豁免(硬清理):无视目标的屏蔽状态,立即生效。
    Bypass,
}

impl TaskState {
    /// Build a `Suspended { tag, reason }` from a [`YieldReason`],
    /// deriving the WasmFX-style [`Tag`] in one place. Use this in
    /// every park path so the fields cannot diverge.
    pub fn suspended(reason: YieldReason) -> Self {
        TaskState::Suspended {
            tag: reason.tag(),
            reason,
        }
    }
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
// See `docs/history/20260915-22.md` §0.5 + §3.1 + §3.2 + §4.4.1 +
// §4.4.4 for the design rationale.
// ───────────────────────────────────────────────────────────────────

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
/// the Cancelled variant; ADR-0019 §4.4.3 / spec §16.4 add
/// `MethodCall` and `ProtocolViolation` as **reserved tags** for a future
/// effect-handler backend (D-4: 明文化,本版不产生;见 spec §17.4 末段).
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
    /// `perform MethodCall` — OOP method invocation event
    /// (spec §16.4). Reserved for the CALL_METHOD effect-handler
    /// path; not yet raised by the evaluator in v0.9.0.
    MethodCall {
        #[allow(dead_code)]
        method: String,
    },
    /// `raise ProtocolViolation` — session-type protocol miss
    /// (spec §16.4). Reserved; currently surfaced as E0050 / E0051.
    ProtocolViolation {
        #[allow(dead_code)]
        method: String,
    },
}

impl Effect {
    /// Extract the dispatch [`Tag`].
    pub fn tag(&self) -> Tag {
        match self {
            Effect::Yield { .. } => Tag::Yield,
            Effect::ChannelOp { .. } => Tag::ChannelOp,
            Effect::Cancelled => Tag::Cancelled,
            Effect::MethodCall { .. } => Tag::MethodCall,
            Effect::ProtocolViolation { .. } => Tag::ProtocolViolation,
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
    /// [项目扩展 / G-02] 挂在 socket 上的等待者：**socket id → 任务**。
    ///
    /// ⚠ 这是「可挂起的 I/O 等待」的落点：原生成员（`wlwl-std`）**不能**碰调度器
    ///   （解释器的值模型是 `Rc`，不是 `Send`）⇒ 宿主 I/O 线程只能把「就绪」推进
    ///   就绪队列，由**解释器线程**取走并在这里唤醒任务。
    pub socket_waiters: std::collections::HashMap<i64, TaskId>,
    /// [G-24] 正被某个 `AWAIT` 驱动的任务（`run_until_done` 的目标）。
    ///
    /// ⚠ 用来区分两种失败：**有人在等**（⇒ `AWAIT` 会把它重新抛出来，失败是
    ///   「被认领」的）与**没人管**（fire-and-forget ⇒ 失败会无声无息地消失）。
    pub awaiting_targets: std::collections::HashSet<TaskId>,
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
            socket_waiters: std::collections::HashMap::new(),
            awaiting_targets: std::collections::HashSet::new(),
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
            TaskState::Done(_) | TaskState::Cancelled | TaskState::Suspended { .. }
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
                TaskState::Suspended {
                    reason: YieldReason::AwaitingChild(child),
                    ..
                } if *child == finished_task_id
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
        self.request_task_cancel_with_reason(handle, None)
    }

    /// [v0.9 Step 8 / plan §4.4.2 / ADR-0019 §4.4.2] Tag/payload
    /// cancellation. Same as `request_task_cancel` but records a
    /// `reason` DICT on the task. First non-None reason wins (FIFO
    /// observation order via `Task::merge_cancel_reason`); the
    /// reason surfaces as `payload.reason` on the `Cancelled`
    /// `Value::Err` returned by `AWAIT(h)`.
    /// [X-02b / S1] Tag/payload cancellation with a **shield policy**.
    ///
    /// Same as `request_task_cancel_with_reason` but records a
    /// `reason` DICT on the task. First non-None reason wins (FIFO
    /// observation order via `Task::merge_cancel_reason`); the
    /// reason surfaces as `payload.reason` on the `Cancelled`
    /// `Value::Err` returned by `AWAIT(h)`.
    pub fn request_task_cancel_with_reason(
        &mut self,
        handle: TaskHandle,
        reason: Option<Value>,
    ) -> bool {
        self.request_task_cancel_with_reason_policy(handle, reason, ShieldPolicy::Deferrable)
    }

    /// [X-02b / S1] 同上,但带**屏蔽策略**。
    ///
    /// `Deferrable`(默认):目标**当前在 `SHIELD` 块内**
    /// (`Task::shield_depth > 0`)⇒ 取消**记进该目标自己的**待发槽
    /// (`shield_pending_cancel` / `shield_pending_reason`),**不**置
    /// `cancel_requested` ⇒ 它在**自己的**屏蔽块退出时才生效(释放点 =
    /// `Evaluator::shield_exit`)。
    ///
    /// `Bypass`:**豁免**屏蔽 —— 无视目标的屏蔽状态,立即置
    /// `cancel_requested`。只给**硬清理**路径用(作用域退出)。
    ///
    /// 依据:规范并发册 §17.3「`SHIELD(fn)`: 调用 `fn` 并**屏蔽屏蔽块内
    /// 收到的取消请求**」⇒ 判定按**目标**的状态(§6.7 的 S1)。
    pub fn request_task_cancel_with_reason_policy(
        &mut self,
        handle: TaskHandle,
        reason: Option<Value>,
        policy: ShieldPolicy,
    ) -> bool {
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
            if policy == ShieldPolicy::Deferrable && t.shield_depth > 0 {
                // [X-02b / S1] 推迟:记进**该目标**的待发槽 —— 不置
                // `cancel_requested` ⇒ 目标在自己的屏蔽块退出前读不到取消。
                t.shield_pending_cancel = true;
                t.shield_pending_reason = reason;
            } else {
                t.cancel_requested = true;
                t.merge_cancel_reason(reason);
            }
        }
        if needs_enqueue {
            self.enqueue(handle.id);
        }
        true
    }

    /// [X-02b / S4 + X-24] 作用域退出时的**硬清理** —— **豁免** `SHIELD` 的推迟,
    /// 覆盖**所有**仍非终态的子任务。
    ///
    /// 规范并发册 §17.1:「子任务寿命**不超过**词法 `SCOPE`」+「`SCOPE` 返回前
    /// **必须**等待其子任务结束」⇒ 作用域退出时仍非终态的子任务必须收尾。
    ///
    /// [X-24] 范围是**全部**非终态 —— 包括停在**通道**上、既没被取消也不在
    /// 屏蔽块内的子任务(例如停在**外层作用域**的通道上、本作用域无法强关唤醒
    /// 的)。⚠ 此前刻意收窄到「屏蔽块内 / 已被请求取消」两类 ⇒ 通道停泊那一支
    /// 活过 `SCOPE` ⇒ 违 §17.1。
    ///
    /// ⚠ 本作用域**自己**的通道上的停泊方已由 `pop_scope` 先唤醒、重跑完
    /// (§17.2.3 的 E0054 / ChannelClosed)后再进到这里 ⇒ 到这一步仍非终态的
    /// 只会是「别的通道上停泊 / 仍被屏蔽」等**收不到唤醒**的形态。
    ///
    /// 返回被硬清理的任务数。
    pub fn force_cancel_scope_children(&mut self, scope_id: crate::runtime::ScopeId) -> usize {
        let handles: Vec<TaskHandle> = match self.scopes.get(scope_id.0) {
            Some(s) => s.tasks.clone(),
            None => return 0,
        };
        let mut forced = 0usize;
        for h in handles {
            let needs_force = match self.tasks.get(h.id.0) {
                Some(t) => {
                    t.id == h.id
                        && t.generation == h.generation
                        && !matches!(t.state, TaskState::Done(_) | TaskState::Cancelled)
                }
                None => false,
            };
            if !needs_force {
                continue;
            }
            // Suspended ⇒ 转 `Cancelled`(并唤醒依赖者);其余非终态 ⇒ 置标志
            // + 转 `Cancelled`。
            let suspended = matches!(
                self.tasks.get(h.id.0).map(|t| &t.state),
                Some(TaskState::Suspended { .. })
            );
            if suspended {
                if self.cancel_suspended_task(h.id) {
                    forced += 1;
                }
            } else if let Some(t) = self.tasks.get_mut(h.id.0) {
                t.cancel_requested = true;
                t.state = TaskState::Cancelled;
                forced += 1;
            }
        }
        forced
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
        self.cancel_siblings_in_scope_with_reason(scope_id, exclude, None)
    }

    /// [v0.9 Step 8 / plan §4.4.2] Same as
    /// `cancel_siblings_in_scope` but records `reason` on every
    /// targeted task (first non-None reason wins per
    /// `Task::merge_cancel_reason`).
    pub fn cancel_siblings_in_scope_with_reason(
        &mut self,
        scope_id: crate::runtime::ScopeId,
        exclude: Option<TaskId>,
        reason: Option<Value>,
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
            if self.request_task_cancel_with_reason(h, reason.clone()) {
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
        self.cancel_scope_subtree_with_reason(scope_id, None)
    }

    /// [v0.9 Step 8 / plan §4.4.2 / ADR-0019 §4.4.2] Same as
    /// `cancel_scope_subtree` but records `reason` on every targeted
    /// task (first non-None reason wins per
    /// `Task::merge_cancel_reason`).
    pub fn cancel_scope_subtree_with_reason(
        &mut self,
        scope_id: crate::runtime::ScopeId,
        reason: Option<Value>,
    ) -> usize {
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
                if self.request_task_cancel_with_reason(h, reason.clone()) {
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
    // task from Running -> Suspended { tag, reason } -> back to
    // Running when the wait condition resolves. They are used by:
    //
    // - Step 4: builtin_channel_send / recv (park on WouldBlock,
    //   wake via channel pair).
    // - Step 8: TASK_CANCEL(task, reason) / TASK_CANCEL_PARENT
    //   (raise Cancelled effect, transition to Cancelled).
    // - ADR-0019 §4.4.3: CALL_METHOD —— `MethodCall` tag 保留但**本版
    //   不产生**(D-4)。协议状态机当前是**同步**校验:违规当场报
    //   `E0050` / `E0051`,不走效果处理器。
    //
    // The existing infrastructure (Channel::push_*_waiter /
    // pop_*_waiter, TaskState::Suspended, YieldReason) is unchanged;
    // these helpers are additive. Step 4 + Step 8 wire them into
    // the eval path.
    // ────────────────────────────────────────────────────────────

    /// [v0.9 Step 3] Park `task_id` on `channel`'s sender_waiters or
    /// receiver_waiters list, transition it to
    /// `TaskState::Suspended { tag: Tag::ChannelOp, reason: YieldReason::SendingOn|_ | ReceivingOn|_ }`
    /// (via [`TaskState::suspended`], plan §9.1 Step 10), and
    /// remove it from the run queue. The caller is expected to
    /// have just failed a synchronous channel op
    /// (channel::TryResult::WouldBlock) and is converting it to a
    /// real suspension.
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
        let task = self.tasks.get_mut(task_id.0)?;
        let reason = match dir {
            Direction::Send => YieldReason::SendingOn(channel),
            Direction::Recv => YieldReason::ReceivingOn(channel),
        };
        // WasmFX-style `Suspended { tag, reason }`. `tag` is derived
        // from `reason` via [`TaskState::suspended`] to keep the pair
        // consistent.
        task.state = TaskState::suspended(reason);
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
            Some(TaskState::Suspended {
                reason: YieldReason::ReceivingOn(_) | YieldReason::SendingOn(_),
                ..
            })
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

    // ── [项目扩展 / G-02] socket 就绪等待 ───────────────────────────────
    //
    // 「可挂起的 I/O 等待」：原生成员发现「现在没有数据」时返回
    // `Signal::Yield(YieldReason::SocketReady(id))`；求值器把任务挂到
    // `socket_waiters[id]`，并在 socket 就绪时唤醒它。
    //
    // ⚠ 与通道等待的差别只在**唤醒方**：通道由对端任务唤醒，socket 由宿主
    //   I/O 线程经就绪队列唤醒（`wlwl_std::tcp::take_ready`）。

    /// 把任务挂到某个 socket 上（等 I/O 就绪），并从运行队列里摘掉它。
    pub fn park_for_socket(&mut self, task_id: TaskId, socket_id: i64) {
        if let Some(t) = self.tasks.get_mut(task_id.0) {
            t.state = TaskState::suspended(YieldReason::SocketReady(socket_id));
        }
        self.run_queue.retain(|id| *id != task_id);
        self.socket_waiters.insert(socket_id, task_id);
    }

    /// socket 就绪 ⇒ 唤醒挂在它上面的任务。返回是否真的唤醒了。
    pub fn wake_socket_waiter(&mut self, socket_id: i64) -> bool {
        let Some(task_id) = self.socket_waiters.remove(&socket_id) else {
            return false;
        };
        let still_parked = matches!(
            self.tasks.get(task_id.0).map(|t| &t.state),
            Some(TaskState::Suspended {
                reason: YieldReason::SocketReady(_),
                ..
            })
        );
        if !still_parked {
            return false;
        }
        if let Some(t) = self.tasks.get_mut(task_id.0) {
            t.state = TaskState::Pending;
            // [v0.12.0 / Q1] **这次等待结束了** ⇒ 它的截止时间**一并清掉**
            // （那是「那一次等待」的预算，socket 已就绪 ⇒ 用不上了）。
            // ⚠ 不清 ⇒ 那个截止时间会**继续挂着** ⇒ 之后某个无关的等待会被它
            //   打断（症状与「莫名超时」无法区分）。
            t.deadline_ms = None;
        }
        self.enqueue(task_id);
        true
    }

    /// 当前挂在 socket 上的等待者数量（调度循环据此决定「要不要等就绪」）。
    pub fn socket_waiter_count(&self) -> usize {
        self.socket_waiters.len()
    }

    // ── [G-08] 定时睡眠（挂起到某个绝对时刻）──────────────────────────
    //
    // 与 `deadline_ms`（G-13 的「最多等多久」）是**两件事**：
    //   * deadline 到点 ⇒ 等待**失败** ⇒ 置 `timed_out`；
    //   * 睡眠到点 ⇒ 等待**正常结束** ⇒ 任务继续跑。
    //
    // ⇒ 两套字段、两套唤醒，**不混**。

    /// 把任务挂起到绝对时刻 `at_ms`（单调钟毫秒）。
    ///
    /// 任务状态转 `Suspended(SleepUntil)` 并从运行队列摘掉；到点后由
    /// [`Scheduler::wake_due_sleepers`] 唤醒。
    pub fn park_for_timer(&mut self, task_id: TaskId, at_ms: i64) {
        if let Some(t) = self.tasks.get_mut(task_id.0) {
            t.state = TaskState::suspended(YieldReason::SleepUntil(at_ms));
            t.sleep_until_ms = Some(at_ms);
        }
        self.run_queue.retain(|id| *id != task_id);
    }

    /// 把**到期**的睡眠任务唤醒（转 `Pending` + 入队）。返回被唤醒的任务 id。
    ///
    /// ⚠ 与 [`Scheduler::expire_deadlines`] 分开：那里的到期是「**超时**」
    ///   （等待失败 ⇒ 置 `timed_out`）；这里到期是「**睡够了**」（正常结束）。
    pub fn wake_due_sleepers(&mut self, now_ms: i64) -> Vec<TaskId> {
        let mut woken = Vec::new();
        for idx in 0..self.tasks.len() {
            let Some(at) = self.tasks[idx].sleep_until_ms else {
                continue;
            };
            if at > now_ms {
                continue;
            }
            // 只有**仍挂着**的任务才该被唤醒（已被别的理由唤醒的 ⇒ 它已经跑过了）。
            let still_parked = matches!(
                self.tasks[idx].state,
                TaskState::Suspended {
                    reason: YieldReason::SleepUntil(_),
                    ..
                }
            );
            if !still_parked {
                self.tasks[idx].sleep_until_ms = None;
                continue;
            }
            // ⚠ 清定时器，但**置**「睡够了」标志 —— 后者由**睡眠成员**消费
            //   （它要认出「我是被唤醒后重跑的」⇒ 否则会再睡一次 ⇒ 无限循环）。
            self.tasks[idx].sleep_until_ms = None;
            self.tasks[idx].sleep_elapsed = true;
            self.tasks[idx].state = TaskState::Pending;
            let id = self.tasks[idx].id;
            self.enqueue(id);
            woken.push(id);
        }
        woken
    }

    /// 所有睡眠任务里**最早**的那个到点时刻（调度循环据此决定「要等多久」）。
    pub fn earliest_sleep_deadline(&self) -> Option<i64> {
        self.tasks.iter().filter_map(|t| t.sleep_until_ms).min()
    }
    /// 某个任务是否正挂在 socket 上（等 I/O 就绪）。
    pub fn task_parked_on_socket(&self, task_id: TaskId) -> bool {
        matches!(
            self.tasks.get(task_id.0).map(|t| &t.state),
            Some(TaskState::Suspended {
                reason: YieldReason::SocketReady(_),
                ..
            })
        )
    }

    // ── [G-13] 截止时间（deadline）────────────────────────────────────
    //
    // 「最多等多久」这件事**必须**住在**调度器**里：等待是**可挂起**的
    // （任务被摘出运行队列），而「到点没有等到」这件事只有调度器能观察到
    // ——它是唯一一个既知道「谁挂着」又能让它「重新变成可运行」的地方。
    //
    // ⚠ 口径：**绝对**截止时间（真实单调钟毫秒）⇒ 语义是「这个任务最多还能
    //   活多久」，不是「每次等待各 ms」。这正是服务端要的：连接最多处理 5 秒 /
    //   优雅关闭的最后期限。
    //
    // ⚠ 计时用**真实单调钟**（`wlwl_value::real_mono_ms`），**不是**虚拟钟气泡
    //   里的那个钟 —— 气泡的虚拟钟只在 `SLEEP` 时前进，拿它计时 ⇒ 「气泡内的
    //   截止时间永远不到」。

    /// 给任务设**绝对**截止时间（真实单调钟毫秒）。返回是否设上了。
    pub fn set_task_deadline(&mut self, task_id: TaskId, abs_ms: i64) -> bool {
        let Some(t) = self.tasks.get_mut(task_id.0) else {
            return false;
        };
        t.deadline_ms = Some(abs_ms);
        true
    }

    /// 读任务的截止时间。
    pub fn task_deadline(&self, task_id: TaskId) -> Option<i64> {
        self.tasks.get(task_id.0).and_then(|t| t.deadline_ms)
    }

    /// 清掉任务的截止时间（「恢复成没有截止时间」）。
    pub fn clear_task_deadline(&mut self, task_id: TaskId) {
        if let Some(t) = self.tasks.get_mut(task_id.0) {
            t.deadline_ms = None;
        }
    }

    /// 读并**清掉**「已超时」标志。
    ///
    /// 谁读它：**等待操作**（`CHANNEL_RECV` / `CHANNEL_SEND`）在**重跑**时先读它
    /// ⇒ 命中就返回**超时值**，而不是再挂一次（否则会挂成**永远**）。
    pub fn take_timed_out(&mut self, task_id: TaskId) -> bool {
        let Some(t) = self.tasks.get_mut(task_id.0) else {
            return false;
        };
        std::mem::take(&mut t.timed_out)
    }

    /// 所有任务里**最早**的截止时间（调度循环据此决定「要等多久」）。
    ///
    /// ⚠ 只算**真正在等**的任务 —— 且只算**会被截止时间打断**的那三类等待
    ///   （通道收 / 通道发 / socket 读）。理由：
    ///   ① 一个**已经跑完**的任务身上不该还挂着截止时间（否则调度循环会为它
    ///      白等到点，白白拖慢别人）；
    ///   ② `AWAIT` 等子任务那种等待**不**认这个截止时间（超时后重跑会重新
    ///      `AWAIT`，语义上不是「这次等待超时了」）⇒ 不该被它打断。
    pub fn earliest_deadline(&self) -> Option<i64> {
        self.tasks
            .iter()
            .filter(|t| matches!(t.state, TaskState::Suspended { .. }))
            .filter_map(|t| match t.state {
                TaskState::Suspended {
                    reason:
                        YieldReason::ReceivingOn(_)
                        | YieldReason::SendingOn(_)
                        | YieldReason::SocketReady(_),
                    ..
                } => t.deadline_ms,
                _ => None,
            })
            .min()
    }

    /// 把**已到期**的任务唤醒，并置上「已超时」标志。返回被唤醒的任务 id。
    ///
    /// ⚠ 只有**仍挂着**（`Suspended`）的任务会超时 —— 已被对端唤醒的任务
    ///   早就拿到值了，不能再被超时抢走。
    ///
    /// ⚠ 只打断**会被截止时间打断**的那三类等待（通道收 / 通道发 / socket 读）
    ///   —— 见 [`Scheduler::earliest_deadline`] 的理由。
    ///
    /// ⚠ 唤醒前**必须把它从等待者名单里摘掉**（通道 / socket）——
    ///   否则对端来配对时会**再唤醒一次**（幽灵唤醒：一次超时等值两次）。
    pub fn expire_deadlines(&mut self, now_ms: i64) -> Vec<TaskId> {
        let mut fired = Vec::new();
        for idx in 0..self.tasks.len() {
            let Some(dl) = self.tasks[idx].deadline_ms else {
                continue;
            };
            if dl > now_ms {
                continue;
            }
            // 已经被对端唤醒（不是 Suspended）⇒ 它已经拿到值了 ⇒ 只清截止时间。
            let reason = match &self.tasks[idx].state {
                TaskState::Suspended { reason, .. } => Some(*reason),
                _ => None,
            };
            let Some(reason) = reason else {
                self.tasks[idx].deadline_ms = None;
                continue;
            };
            // 只打断「会被截止时间打断」的那三类等待。
            let (dir, ch) = match reason {
                YieldReason::ReceivingOn(ch) => (Direction::Recv, Some(ch)),
                YieldReason::SendingOn(ch) => (Direction::Send, Some(ch)),
                YieldReason::SocketReady(sid) => {
                    self.socket_waiters.remove(&sid);
                    (Direction::Recv, None)
                }
                _ => continue,
            };
            // 摘掉等待者（通道 / socket）—— 否则会有幽灵唤醒。
            if let Some(ch) = ch {
                if let Some(c) = self.channels.get_mut(ch.0) {
                    c.remove_waiter(self.tasks[idx].id, dir);
                }
            }
            self.tasks[idx].timed_out = true;
            self.tasks[idx].deadline_ms = None;
            self.tasks[idx].state = TaskState::Pending;
            let id = self.tasks[idx].id;
            self.enqueue(id);
            fired.push(id);
        }
        fired
    }

    // ── [G-24] 「有没有人管这个任务」──────────────────────────────────
    //
    // 一个任务失败时，失败只存在它自己的 `TaskResult::Failed` 里 ⇒
    // **只有 `AWAIT` 它的人才会看到**；没人 `AWAIT` ⇒ 失败**完全静默**
    // （没有诊断、没有 stderr、没有退出码）。对「每连接一个任务」的服务器来说，
    // 这意味着**一个 handler 崩了，服务器照常运行、只是不响应那个连接**。

    /// 有没有任务正停在 `AwaitingChild(this)` 上（有人在等它）。
    pub fn has_dependents(&self, task_id: TaskId) -> bool {
        self.tasks.iter().any(|t| {
            matches!(
                &t.state,
                TaskState::Suspended {
                    reason: YieldReason::AwaitingChild(c),
                    ..
                } if c.0 == task_id.0
            )
        })
    }

    /// 这个任务是不是正被某个 `AWAIT` 驱动着（`run_until_done` 的目标）。
    pub fn is_being_awaited(&self, task_id: TaskId) -> bool {
        self.awaiting_targets.contains(&task_id)
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
        // [v0.12.0 / H.4.3] 先取出**宿主等待登记**（socket id / 睡眠时刻），
        // 再改状态 —— `self.tasks` 与 `self.socket_waiters` 不能同时可变借用。
        let (socket_wait, timer_wait) = match self.tasks.get(task_id.0) {
            Some(t) => match &t.state {
                TaskState::Suspended {
                    reason: YieldReason::SocketReady(sid),
                    ..
                } => (Some(*sid), None),
                TaskState::Suspended {
                    reason: YieldReason::SleepUntil(at),
                    ..
                } => (None, Some(*at)),
                TaskState::Suspended { .. } => (None, None),
                _ => return false,
            },
            None => return false,
        };
        // ⚠ 并发册附录 H.4.3:取消 ⇒ **清空宿主等待登记**。
        //   不清 ⇒ 被取消的 socket 等待者仍留在 `socket_waiters` 里 ⇒
        //   `socket_waiter_count() > 0` ⇒ 调度循环会**空等**就绪事件
        //   （那个任务永远不会来唤醒它）；睡眠者也会一直占着 `sleep_until_ms`。
        if let Some(sid) = socket_wait {
            self.socket_waiters.remove(&sid);
        }
        if let Some(t) = self.tasks.get_mut(task_id.0) {
            t.state = TaskState::Cancelled;
            // 截止时间一并清掉:被取消的任务不该再被「到点」逻辑碰到。
            t.deadline_ms = None;
            t.timed_out = false;
            if timer_wait.is_some() {
                t.sleep_until_ms = None;
            }
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
                    TaskState::Suspended {
                        reason: YieldReason::ReceivingOn(_) | YieldReason::SendingOn(_),
                        ..
                    }
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

    // ────────────────────────────────────────────────────────────
    // [X-02b / S4] 作用域退出的硬清理(白盒守卫)
    // ────────────────────────────────────────────────────────────

    /// [X-02b / S4] 作用域退出时,**已被请求取消**且**仍停泊**的子任务必须被
    /// **强制取消**(`Suspended` ⇒ `Cancelled`)⇒ 不会因为「取消被推迟」而
    /// 活过作用域(并发册 §17.1「子任务寿命不超过词法 `SCOPE`」)。
    ///
    /// ⚠ 这是**白盒**守卫:`SHIELD` 那一支(「仍在屏蔽块内」)在用户程序里
    /// **不可达**(`AWAIT` **不停泊**调用方 —— 它把调度器跑到底、拿不到值
    /// 就报 `E0053`;阻塞型通道 park 又会**展开**屏蔽帧)⇒ 只有「已被请求
    /// 取消 + 仍停泊」这一支能被观测 ⇒ 在这里直接构造调度器状态来钉它。
    #[test]
    fn force_cancel_scope_children_terminates_a_cancelled_parked_child() {
        let (mut s, id, ch) = fresh_scheduler_with_channel();
        // Park the child on the channel op, then request a cancel for it.
        s.tasks[id.0].state = TaskState::suspended(YieldReason::ReceivingOn(ch));
        let handle = s.tasks[id.0].handle();
        assert!(
            s.request_task_cancel_with_reason(handle, None),
            "the advisory cancel must be accepted"
        );
        assert!(
            s.tasks[id.0].cancel_requested,
            "the advisory flag must be set"
        );
        assert!(
            matches!(s.tasks[id.0].state, TaskState::Suspended { .. }),
            "precondition: the child is still parked"
        );
        // Scope exit hard-cleanup => the parked + cancelled child is terminated.
        let forced = s.force_cancel_scope_children(ScopeId(0));
        assert_eq!(
            forced, 1,
            "exactly one cancelled+parked child must be force-cancelled"
        );
        assert!(
            matches!(s.tasks[id.0].state, TaskState::Cancelled),
            "the hard cleanup must transition Suspended -> Cancelled"
        );
    }

    /// [X-24] 子任务**停泊**在通道上、既没被取消也不在屏蔽块内 ⇒ 硬清理
    /// **同样**终止它。
    ///
    /// ⚠ 本期望于 2026-10-24 **改过**(X-24 收敛)—— 此前范围刻意收窄
    /// (只清「屏蔽块内 / 已被请求取消」)⇒ 这里断言「不动它」;而
    /// §17.1「`SCOPE` 返回前**必须**等待其子任务结束」要求**所有**非终态
    /// 子任务收尾 ⇒ 旧期望**不再代表规范** ⇒ 改为断言被终止。
    #[test]
    fn force_cancel_scope_children_terminates_a_plain_parked_child() {
        let (mut s, id, ch) = fresh_scheduler_with_channel();
        s.tasks[id.0].state = TaskState::suspended(YieldReason::ReceivingOn(ch));
        let forced = s.force_cancel_scope_children(ScopeId(0));
        assert_eq!(
            forced, 1,
            "an uncancelled parked child must also be force-cancelled (X-24)"
        );
        assert!(
            matches!(s.tasks[id.0].state, TaskState::Cancelled),
            "the parked child must be terminated, not left suspended"
        );
    }

    #[test]
    fn effect_tag_dispatch_round_trips_for_each_variant() {
        // The three canonical ADR-0017 / ADR-0019 §4.4.1 tags plus
        // the two §16.4 reserved OOP tags.
        let yield_eff = Effect::Yield { explicit: true };
        let channel_eff = Effect::ChannelOp {
            dir: Direction::Send,
            channel: crate::channel::ChannelId(0),
            value: Some(Value::Integer(7)),
        };
        let cancel_eff = Effect::Cancelled;
        let method_eff = Effect::MethodCall {
            method: "get".into(),
        };
        let proto_eff = Effect::ProtocolViolation {
            method: "inc".into(),
        };
        assert_eq!(yield_eff.tag(), Tag::Yield);
        assert_eq!(channel_eff.tag(), Tag::ChannelOp);
        assert_eq!(cancel_eff.tag(), Tag::Cancelled);
        assert_eq!(method_eff.tag(), Tag::MethodCall);
        assert_eq!(proto_eff.tag(), Tag::ProtocolViolation);
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
        assert_ne!(
            discriminant(&method_eff.tag()),
            discriminant(&proto_eff.tag()),
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

        // Post-state: task is Suspended { tag: ChannelOp, reason: SendingOn(ch) },
        // run_queue empty, channel's sender_waiters contains the task id.
        assert!(matches!(
            s.tasks[0].state,
            TaskState::Suspended {
                reason: YieldReason::SendingOn(c),
                ..
            } if c == ch_id
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
            TaskState::Suspended {
                reason: YieldReason::ReceivingOn(c),
                ..
            } if c == ch_id
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

        // Pre-wake: task is Suspended { reason: ReceivingOn(_) }, run_queue empty.
        assert!(matches!(
            s.tasks[0].state,
            TaskState::Suspended {
                reason: YieldReason::ReceivingOn(_),
                ..
            }
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
        s.tasks[0].state = TaskState::suspended(YieldReason::Explicit);
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
            TaskState::Suspended {
                reason: YieldReason::ReceivingOn(_),
                ..
            }
        ));
    }

    #[test]
    fn cancel_suspended_task_transitions_to_cancelled() {
        let (mut s, task_id, ch_id) = fresh_scheduler_with_channel();
        s.enqueue(task_id);
        s.park_for_channel_op(task_id, ch_id, Direction::Recv, None)
            .expect("park");
        // Pre: Suspended { reason: ReceivingOn(_) }, in wait list.
        assert!(matches!(
            s.tasks[0].state,
            TaskState::Suspended {
                reason: YieldReason::ReceivingOn(_),
                ..
            }
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
            TaskState::Suspended {
                reason: YieldReason::SendingOn(_),
                ..
            }
        ));
        assert!(matches!(
            s.tasks[1].state,
            TaskState::Suspended {
                reason: YieldReason::ReceivingOn(_),
                ..
            }
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
            TaskState::Suspended {
                reason: YieldReason::SendingOn(_),
                ..
            }
        ));
    }

    // ────────────────────────────────────────────────────────────
    // [v0.9 Step 8 / plan §4.4.2 / ADR-0019 §4.4.2] Tag/payload
    // cancellation — runtime-level lock tests.
    //
    // These tests pin the wire-format contract introduced by Step 8
    // on the Scheduler API surface:
    //   * `Task::merge_cancel_reason` is **FIFO**: the first
    //     non-None reason wins; subsequent reasons are ignored.
    //     This matches plan §4.4.2 ("the reason the task happened to
    //     be cancelled", not a merge of all reasons).
    //   * `Scheduler::request_task_cancel_with_reason` records the
    //     reason on `Task::cancel_reason` so `builtin_await` can
    //     surface it as `payload.reason` on the AWAIT(h)
    //     `ERR(Cancelled)` dict.
    //   * `Scheduler::cancel_siblings_in_scope_with_reason` and
    //     `Scheduler::cancel_scope_subtree_with_reason` propagate
    //     the reason to every targeted task in the subtree (first
    //     non-None wins per task — independent FIFO clocks).

    #[test]
    fn task_merge_cancel_reason_first_non_none_wins_fifo() {
        use crate::task::Task as TaskImpl;
        let mut t = TaskImpl::new_pending(TaskId(0), 1, Value::Null, Env::new(), ScopeId(0));
        assert!(t.cancel_reason.is_none());

        // First non-None reason wins.
        let r1 = Value::dict(vec![(
            Value::String("from".into()),
            Value::String("first".into()),
        )]);
        t.merge_cancel_reason(Some(r1.clone()));
        assert_eq!(
            t.cancel_reason,
            Some(r1.clone()),
            "first non-None reason must stick"
        );

        // Second non-None reason is dropped (FIFO).
        let r2 = Value::dict(vec![(
            Value::String("from".into()),
            Value::String("second".into()),
        )]);
        t.merge_cancel_reason(Some(r2));
        assert_eq!(
            t.cancel_reason,
            Some(r1),
            "subsequent non-None reasons must be ignored"
        );

        // None after a stuck reason is also dropped.
        t.merge_cancel_reason(None);
        assert!(
            t.cancel_reason.is_some(),
            "None after a stuck reason must NOT clear the field"
        );
    }

    #[test]
    fn request_task_cancel_with_reason_records_dict_in_cancel_reason() {
        use crate::task::Task as TaskImpl;
        let mut s = Scheduler::new();
        // Allocate one task slot and put it in a scope so request
        // can address it by handle.
        let t = TaskImpl::new_pending(TaskId(0), 1, Value::Null, Env::new(), ScopeId(0));
        // State Pending is the entry condition for the
        // `needs_enqueue` branch in
        // `request_task_cancel_with_reason`.
        assert!(matches!(t.state, TaskState::Pending));
        s.push_task(t);
        // ScopeId(0) is auto-created by Scheduler::new(); we don't
        // need to register the task in any scope — the test only
        // exercises `request_task_cancel_with_reason` directly.
        let h = TaskHandle {
            id: TaskId(0),
            generation: 1,
        };
        let reason = Value::dict(vec![
            (
                Value::String("reason".into()),
                Value::String("shutdown".into()),
            ),
            (Value::String("code".into()), Value::Integer(42)),
        ]);
        assert!(s.request_task_cancel_with_reason(h, Some(reason.clone())));
        assert_eq!(
            s.tasks[0].cancel_reason,
            Some(reason),
            "cancel_reason must reflect the supplied dict"
        );
        assert!(s.tasks[0].cancel_requested, "cancel_requested must flip");
    }

    #[test]
    fn cancel_siblings_in_scope_with_reason_propagates() {
        use crate::task::Task as TaskImpl;
        let mut s = Scheduler::new();
        // One scope, three tasks (siblings). Each starts Pending so
        // cancel_requested + cancel_reason both stick.
        for i in 0..3 {
            let t = TaskImpl::new_pending(TaskId(i), 1, Value::Null, Env::new(), ScopeId(0));
            s.push_task(t);
            s.scopes[0].register_task(TaskHandle {
                id: TaskId(i),
                generation: 1,
            });
        }
        let reason = Value::dict(vec![(
            Value::String("graceful".into()),
            Value::Boolean(true),
        )]);
        let applied =
            s.cancel_siblings_in_scope_with_reason(ScopeId(0), None, Some(reason.clone()));
        assert_eq!(applied, 3, "all 3 siblings must be marked cancelled");
        for i in 0..3 {
            assert!(s.tasks[i].cancel_requested, "task {} must be cancelled", i);
            assert_eq!(
                s.tasks[i].cancel_reason,
                Some(reason.clone()),
                "task {} must carry the same reason",
                i
            );
        }
    }

    #[test]
    fn cancel_scope_subtree_with_reason_propagates_to_descendants() {
        use crate::task::Task as TaskImpl;
        let mut s = Scheduler::new();
        // `Scheduler::new()` pre-creates ScopeId(0) as the root
        // (parent = None). To exercise cross-scope propagation we
        // add a child scope ScopeId(1) with parent = Some(ScopeId(0))
        // — this is the typical SCOPE(...) shape from
        // `builtin_scope`'s scope-creation path.
        s.scopes.push(Scope::new(ScopeId(1), Some(ScopeId(0))));
        // Three tasks: two in the root (ScopeId(0)), one in the
        // descendant (ScopeId(1)). Each starts Pending so the
        // `needs_enqueue` branch in
        // `request_task_cancel_with_reason` fires.
        for (i, sid) in [ScopeId(0), ScopeId(0), ScopeId(1)].iter().enumerate() {
            let t = TaskImpl::new_pending(TaskId(i), 1, Value::Null, Env::new(), *sid);
            s.push_task(t);
            s.scopes[sid.0].register_task(TaskHandle {
                id: TaskId(i),
                generation: 1,
            });
        }
        let reason = Value::dict(vec![(
            Value::String("tag".into()),
            Value::String("preempt".into()),
        )]);
        let applied = s.cancel_scope_subtree_with_reason(ScopeId(0), Some(reason.clone()));
        assert_eq!(
            applied, 3,
            "all 3 tasks in root + descendant must be cancelled"
        );
        for i in 0..3 {
            assert!(s.tasks[i].cancel_requested, "task {} cancel flag", i);
            assert_eq!(
                s.tasks[i].cancel_reason,
                Some(reason.clone()),
                "task {} reason propagation across scope depth",
                i
            );
        }
    }
    // ── 决策 D-4(v0.10 范围澄清):保留 tag 面 + 明文「本版不产生」 ──
    //
    // 这两条一起钉住 B′ 的两头,任何一头单独漂移都不会被 CI 抓到:
    //   ①枚举里两个变体**仍在**(代数效果后端迁移的 tag 面就位);
    //   ②规范**明文声明**本版不产生(不许实现依赖它们产生可观察行为)。
    // 删变体、或把「不产生」这句从规范里删掉,都会让下面某条红。

    /// tag 面就位:`MethodCall` / `ProtocolViolation` 仍在 `Effect` 与
    /// `Tag` 里,且一一对应。删掉它们就要回头改 WasmFX 后端的 tag 面。
    #[test]
    fn the_reserved_oop_effect_tags_are_still_in_the_enum() {
        let m = Effect::MethodCall {
            method: "get".into(),
        };
        assert_eq!(m.tag(), Tag::MethodCall);
        let p = Effect::ProtocolViolation {
            method: "inc".into(),
        };
        assert_eq!(p.tag(), Tag::ProtocolViolation);
    }

    /// 规范必须**明文**写出「不由运行期产生」。这是 D-4 落到纸面的唯一凭据:
    /// 枚举里有 tag 是一半,规范承认它不被 raise 是另一半。
    #[test]
    fn the_spec_states_the_oop_effect_tags_are_not_raised() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("..")
            .join("docs")
            .join("wlwl-spec-reference.md");
        let spec = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read the spec at {}: {}", path.display(), e));
        for tag in ["MethodCall", "ProtocolViolation"] {
            assert!(
                spec.contains(tag),
                "the spec must name the reserved tag {}",
                tag
            );
        }
        // 「不由运行期产生」这句必须在场,否则规范等于没裁决 D-4。
        assert!(
            spec.contains("不由运行期产生"),
            "the spec must state that the reserved OOP tags are not produced \
             by the runtime (decision D-4)"
        );
    }
}
