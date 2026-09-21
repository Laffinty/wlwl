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

use crate::{Env, Expr, Value};

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

/// Channel handle placeholder (plan §5.3 + Phase D). At B1 the
/// channel module does not exist; this type exists so [`YieldReason`]
/// can mention it without a forward-declaration loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RcHandle(pub usize);

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
    Done(TaskResult),
    Cancelled,
}

/// Terminal value of a task body (plan §5.1.1 row "fn 返回").
#[derive(Debug, Clone)]
pub enum TaskResult {
    Ok(Value),
    Err(Value),
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
    Expr(Expr),
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

/// One slot in the scheduler's task table (plan §5.2 "Task 由
/// Closure + Env + State + YieldPoints 组成").
///
/// At B1 this carries placeholder `yield_points`; B4 will replace
/// `Closure` with the concrete `Value::Closure` extraction plus
/// the call-shape needed by `Scheduler::step`.
#[derive(Debug)]
pub struct TaskEntry {
    pub state: TaskState,
    pub env: Env,
    pub yield_points: Vec<EvalState>,
}

/// A scope: the lifetime container for tasks (plan §5.2 "Scope 是
/// Task 的容器").
///
/// At B1 fields are typed but no behaviour; B4 wires scope creation
/// to `SCOPE(fn)` and B5 wires scope-exit `await-all-then-cancel`.
#[derive(Debug)]
pub struct Scope {
    pub id: ScopeId,
    pub parent: Option<ScopeId>,
    pub tasks: Vec<TaskHandle>,
    pub cancelled: bool,
}

/// The scheduler itself (plan §5.1 + §5.1.1 loop pseudo-code).
///
/// At B1 the methods are stubs returning `None` / panicking on use
/// -- they exist so the API surface is reviewable. B5 will fill
/// in `step`, `run_until_idle`, `dispatch_cancellations`, and
/// `wake_dependents`.
#[derive(Debug, Default)]
pub struct Scheduler {
    pub tasks: Vec<TaskEntry>,
    pub scopes: Vec<Scope>,
    pub run_queue: VecDeque<TaskId>,
    pub next_generation: u64,
}

impl Scheduler {
    /// Create an empty scheduler. Real construction (taking a root
    /// task / scope) lands in B5.
    pub fn new() -> Self {
        Self::default()
    }

    /// Step one task to its next yield point. **Not yet implemented**
    /// at B1; returns `None` for any input. The real implementation
    /// in B5 will return `Some(StepResult)` for "made progress" and
    /// `None` for "no task to run".
    pub fn step(&mut self, _task_id: TaskId) -> Option<()> {
        None
    }

    /// Run until the run queue is empty. **Not yet implemented**
    /// at B1; present so the API surface is visible.
    pub fn run_until_idle(&mut self) {
        // B5 implementation: the pseudo-code from plan §5.1.1.
    }

    /// Dispatch pending cancellations to tasks that have not yet
    /// observed them. **Not yet implemented** at B1.
    pub fn dispatch_cancellations(&mut self) {
        // B5 implementation.
    }

    /// Re-enqueue tasks that were suspended on `finished_task_id`
    /// (e.g. `AWAIT(finished_task_id)` waiters). **Not yet
    /// implemented** at B1.
    pub fn wake_dependents(&mut self, _finished_task_id: TaskId) {
        // B5 implementation.
    }

    /// Allocate a fresh task slot, returning its `(id, handle)` pair.
    /// At B1 the slot is pushed onto `tasks` and the entry is
    /// placeholder-initialised; B4 replaces the placeholder with
    /// a real `Closure + Env` once `SPAWN` semantics land.
    pub fn alloc_task(&mut self) -> (TaskId, TaskHandle) {
        let id = TaskId(self.tasks.len());
        let handle = TaskHandle {
            id,
            generation: self.next_generation,
        };
        self.next_generation = self.next_generation.wrapping_add(1);
        self.tasks.push(TaskEntry {
            state: TaskState::Pending,
            env: Env::new(),
            yield_points: Vec::new(),
        });
        self.run_queue.push_back(id);
        (id, handle)
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
    fn alloc_task_returns_distinct_handles() {
        let mut s = Scheduler::new();
        let (_, h1) = s.alloc_task();
        let (_, h2) = s.alloc_task();
        assert_ne!(h1.id, h2.id, "different slots must give different ids");
        assert_ne!(
            h1.generation, h2.generation,
            "different allocations must bump generation"
        );
        assert_eq!(s.tasks.len(), 2);
        assert_eq!(s.run_queue.len(), 2);
    }

    #[test]
    fn yield_reason_explicit_is_copy() {
        let r = YieldReason::Explicit;
        let _ = r; // unused warning silencer; just needs to be Copy
        fn is_copy<T: Copy>() {}
        is_copy::<YieldReason>();
    }

    #[test]
    fn scheduler_default_matches_new() {
        let a = Scheduler::default();
        let b = Scheduler::new();
        assert_eq!(a.tasks.len(), b.tasks.len());
        assert_eq!(a.run_queue.len(), b.run_queue.len());
        assert_eq!(a.next_generation, b.next_generation);
    }
}