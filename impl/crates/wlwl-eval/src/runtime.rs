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
    /// Returns `None` if the id is unknown or already terminal.
    pub fn begin_run(&mut self, id: TaskId) -> Option<(Value, crate::Env)> {
        let task = self.tasks.get(id.0)?;
        if matches!(task.state, TaskState::Done(_) | TaskState::Cancelled) {
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
}

/// Shared inner state across a scope subtree (future B5+).
///
/// B1 declares the alias so other modules can name it; the actual
/// shared state (e.g. cancellation token) is empty.
pub type ScopeShared = Rc<Scope>;

#[cfg(test)]
mod tests {
    use super::*;

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
}
