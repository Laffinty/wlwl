//! [v0.7 Phase B4] Task and Scope data structures for the cooperative
//! scheduler.
//!
//! The scheduler itself stays in [`crate::runtime`]; this module
//! holds the *rich* per-task and per-scope data that B5 will wire
//! up. No behaviour lands here -- this is the data shape only.
//!
//! Plan §5.2: "Task 由 Closure + Env + State + YieldPoints 组成"
//! Plan §5.2: "Scope 是 Task 的容器"
//! Plan §5.1.1: Task lifecycle (5 states: Pending / Running /
//!               Suspended / Done / Cancelled).
//!
//! See plan §3 Phase B4 + ADR-0014, ADR-0016 for the full design.

use std::collections::HashMap;

use crate::runtime::{EvalState, ScopeId, TaskHandle, TaskId, TaskState};
use crate::{Cell, Env, Value};
use wlwl_ast::Expr;

/// One scheduled task (plan §5.2).
///
/// At SPAWN time the user passes a closure (`Value::Closure`); the
/// scheduler records it as `body` and captures the surrounding
/// `env`. The closure captures cells according to v0.6 §3.4 (shared
/// cell references, not deep clones), so mutations made by the
/// child are visible to the parent and to siblings that captured
/// the same name -- see plan §5.5.
///
/// `yield_points` is the resumable-execution stack: each time a
/// task yields (`YIELD()`, `AWAIT(child)`, `CHANNEL_RECV` empty,
/// `CHANNEL_SEND` full), the next `EvalState` is pushed here. B5
/// will populate; B4 just declares the field so the type is real.
///
/// `parent_scope` is the scope that owns this task's lifetime. The
/// scheduler uses this for top-down cancellation: when the scope
/// transitions to `cancelled = true`, every task with this id in
/// `parent_scope` gets a cancel signal.
///
/// `segments` / `current_segment` (Phase B5a-3 Path B, see
/// `crate::yield_split`): the closure body is pre-segmented into a
/// sequence of "runnable chunks" at SPAWN time. The scheduler runs
/// `segments[current_segment]` per `run_one_task` call and advances
/// `current_segment` on each `Signal::Yield`. The last segment does
/// not contain a YIELD (it produces the task's terminal value). For
/// task bodies that contain no YIELD at all, `segments` is left
/// empty (the runtime falls back to running the original body
/// directly, preserving v0.6 fidelity for non-yielding tasks).
#[derive(Debug)]
pub struct Task {
    pub id: TaskId,
    pub generation: u64,
    /// Closure body to execute. Expected to be `Value::Closure`
    /// at SPAWN time; the scheduler extracts `(params, body, env)`
    /// from it before invoking. B5 will narrow this with a type
    /// check at allocation time (E0052 if not a function value).
    pub body: Value,
    /// Environment captured at SPAWN. Per v0.6 §3.4 closure-capture
    /// semantics the cells in this Env are shared with the lexical
    /// scope at the SPAWN call site -- not deep-cloned -- so
    /// mutating `LET MUT x` from the child is visible to siblings
    /// and to the parent. plan §5.5.
    pub env: Env,
    pub state: TaskState,
    /// Resumable-execution stack; pushed on every yield (plan
    /// §5.1.1 "协作式让步点"). Empty until B5 wires the CPS
    /// conversion.
    pub yield_points: Vec<EvalState>,
    /// Scope that owns this task's lifetime. Top-down cancellation
    /// (plan §10 D9) walks the scope tree from any cancelled scope
    /// to mark all descendant tasks Cancelled.
    pub parent_scope: ScopeId,
    /// [Phase B5a-3 Path B] Body segmentation. Each inner `Vec<Expr>`
    /// is a sequence to run as one segment; see
    /// `crate::yield_split::split_body_for_yield`. Empty means
    /// "no segmentation; scheduler runs the original body in one
    /// shot" (the v0.6 fidelity path for tasks without YIELD).
    pub segments: Vec<Vec<Expr>>,
    /// [Phase B5a-3 Path B] Index of the next segment to run.
    /// `0` on first run; incremented when a segment produces a
    /// `Signal::Yield(Explicit)` that the scheduler captures.
    pub current_segment: usize,
    /// [Phase B5a-3 Path B] Saved env scope-stack at the moment a
    /// segment yielded. `None` on first run (and after the final
    /// segment completes). The runner installs this Vec directly on
    /// the Evaluator's `env.scopes` for the next segment so LET
    /// bindings made in earlier segments stay accessible. Cells are
    /// `Rc<RefCell<...>>`, so the clone is cheap.
    pub running_env: Option<Vec<HashMap<String, Cell>>>,
    /// [v0.7 Phase E-B] Advisory cancellation request. Set by
    /// `TASK_CANCEL(handle)` (cross-task) or by `TASK_CANCEL_PARENT()`
    /// (current scope siblings) or by SCOPE sibling cancellation when
    /// any sibling terminates with `Done(Err(_))` / `Failed(_)`.
    ///
    /// The task is **not** cancelled immediately; the cancellation
    /// is observed at the next checkpoint: `run_one_task` entry
    /// (before `eval_block`), and (in Phase F5 SHIELD) after exiting
    /// a SHIELD block. A purely-synchronous task body that yields
    /// no checkpoints runs to completion before observing the
    /// cancel — this is a path-B limitation documented as
    /// `P7-E3-001`.
    ///
    /// `TASK_IS_CANCELLED()` reads this flag; `AWAIT(h)` of a
    /// cancelled task surfaces `ERR(kind="Cancelled")` (the existing
    /// builtin_await Cancelled branch).
    pub cancel_requested: bool,
}

impl Task {
    /// Construct a fresh Task in the `Pending` state with empty
    /// yield stack. Generation is set from the caller's counter so
    /// stale handles (E0053) are detectable.
    ///
    /// At B4 this is used by tests only; B5 wires it into
    /// `Scheduler::alloc_task` once `SPAWN` semantics land.
    pub fn new_pending(
        id: TaskId,
        generation: u64,
        body: Value,
        env: Env,
        parent_scope: ScopeId,
    ) -> Self {
        Self {
            id,
            generation,
            body,
            env,
            state: TaskState::Pending,
            yield_points: Vec::new(),
            parent_scope,
            segments: Vec::new(),
            current_segment: 0,
            running_env: None,
            cancel_requested: false,
        }
    }

    /// The handle that user code receives from `SPAWN`.
    pub fn handle(&self) -> TaskHandle {
        TaskHandle {
            id: self.id,
            generation: self.generation,
        }
    }
}

/// A scope (plan §5.2 "Scope 是 Task 的容器").
///
/// Scopes form a tree via `parent`. `tasks` are the direct child
/// tasks that belong to this scope (not the entire subtree).
/// `cancelled` is set to true by `TASK_CANCEL(handle)` /
/// `TASK_CANCEL_PARENT()`; when set, every direct child task that
/// is still `Pending` / `Running` / `Suspended` gets a cancel signal
/// via `Scheduler::dispatch_cancellations` (plan §5.1.1).
///
/// Scope exit (when the SCOPE block returns) waits for all direct
/// child tasks to reach `Done` or `Cancelled`; if any linger past
/// a future timeout, the scope forces cancellation. The timeout
/// itself is deferred to a future phase -- D20 / plan §6.4 only
/// promises "correctness + leak-freeness", not latency.
#[derive(Debug)]
pub struct Scope {
    pub id: ScopeId,
    pub parent: Option<ScopeId>,
    pub tasks: Vec<TaskHandle>,
    pub cancelled: bool,
    /// [v0.7 Phase D-D] Channel slots owned by this scope. Each
    /// `CHANNEL_NEW(buf)` called while the scope is `current_scope`
    /// appends its `ChannelId` here. When the scope exits the leak
    /// detector walks this list and force-closes each channel
    /// (plan §3 D7); the channel's generation is then bumped so any
    /// handle still held by user code fails `E0053` at the next op.
    /// Channels created at module top-level (before any SCOPE
    /// block) are owned by the root scope (id 0).
    pub channels: Vec<crate::channel::ChannelId>,
}

impl Scope {
    /// Construct a fresh scope with no tasks and `cancelled = false`.
    pub fn new(id: ScopeId, parent: Option<ScopeId>) -> Self {
        Self {
            id,
            parent,
            tasks: Vec::new(),
            cancelled: false,
            channels: Vec::new(),
        }
    }

    /// Register a newly-spawned task as a direct child of this
    /// scope. Returns `true` if the scope was already cancelled at
    /// registration time -- the caller is responsible for then
    /// triggering a cancel on the new task. (B5 will wire this
    /// check into `SPAWN`.)
    pub fn register_task(&mut self, h: TaskHandle) -> bool {
        let was_cancelled = self.cancelled;
        self.tasks.push(h);
        was_cancelled
    }

    /// Number of direct child tasks that have NOT reached a
    /// terminal state (`Done` / `Cancelled`). Used by scope-exit
    /// to decide whether to wait or to force cancellation.
    pub fn outstanding_tasks(&self, scheduler_tasks: &[Task]) -> usize {
        self.tasks
            .iter()
            .filter(|h| {
                scheduler_tasks
                    .get(h.id.0)
                    .map(|t| !matches!(t.state, TaskState::Done(_) | TaskState::Cancelled))
                    .unwrap_or(true) // unknown handle = treat as outstanding (paranoid)
            })
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::{TaskHandle, TaskState};

    fn dummy_task(id: usize, generation: u64, scope: usize) -> Task {
        Task::new_pending(
            TaskId(id),
            generation,
            Value::Null, // body: not yet typed-narrowed at B4
            Env::new(),
            ScopeId(scope),
        )
    }

    #[test]
    fn task_handle_round_trips() {
        let t = dummy_task(7, 3, 0);
        assert_eq!(
            t.handle(),
            TaskHandle {
                id: TaskId(7),
                generation: 3
            }
        );
    }

    #[test]
    fn task_starts_pending_and_no_cancel() {
        let t = dummy_task(0, 1, 0);
        assert!(matches!(t.state, TaskState::Pending));
        assert!(t.yield_points.is_empty());
        assert!(
            !t.cancel_requested,
            "fresh task must not have a cancel request pending"
        );
    }

    #[test]
    fn scope_starts_empty_and_uncancelled() {
        let s = Scope::new(ScopeId(0), None);
        assert!(s.tasks.is_empty());
        assert!(!s.cancelled);
        assert_eq!(s.parent, None);
    }

    #[test]
    fn scope_register_records_task() {
        let mut s = Scope::new(ScopeId(0), None);
        let h = TaskHandle {
            id: TaskId(0),
            generation: 1,
        };
        assert!(!s.register_task(h), "scope not yet cancelled at register");
        assert_eq!(s.tasks.len(), 1);
        assert_eq!(s.tasks[0], h);
    }

    #[test]
    fn scope_register_after_cancel_returns_flag() {
        let mut s = Scope::new(ScopeId(0), None);
        s.cancelled = true;
        let h = TaskHandle {
            id: TaskId(0),
            generation: 1,
        };
        assert!(
            s.register_task(h),
            "register on a cancelled scope must report the cancel"
        );
    }

    #[test]
    fn outstanding_counts_nonterminal_only() {
        let mut tasks = vec![
            dummy_task(0, 1, 0),
            dummy_task(1, 1, 0),
            dummy_task(2, 1, 0),
        ];
        let mut s = Scope::new(ScopeId(0), None);
        s.register_task(TaskHandle {
            id: TaskId(0),
            generation: 1,
        });
        s.register_task(TaskHandle {
            id: TaskId(1),
            generation: 1,
        });
        s.register_task(TaskHandle {
            id: TaskId(2),
            generation: 1,
        });

        // All Pending -> 3 outstanding
        assert_eq!(s.outstanding_tasks(&tasks), 3);

        // Task 1 reaches Done -> 2 outstanding
        tasks[1].state = TaskState::Done(Box::new(crate::runtime::TaskResult::Ok(Value::Null)));
        assert_eq!(s.outstanding_tasks(&tasks), 2);

        // Task 0 is Cancelled -> 1 outstanding
        tasks[0].state = TaskState::Cancelled;
        assert_eq!(s.outstanding_tasks(&tasks), 1);
    }

    #[test]
    fn outstanding_treats_unknown_handle_as_outstanding() {
        // Defensive: if a handle references a TaskId that doesn't
        // exist (out-of-range slot), treat it as still running so the
        // scope doesn't accidentally exit early.
        let tasks = vec![dummy_task(0, 1, 0)];
        let mut s = Scope::new(ScopeId(0), None);
        s.register_task(TaskHandle {
            id: TaskId(99),
            generation: 1,
        });
        assert_eq!(s.outstanding_tasks(&tasks), 1);
    }
}
