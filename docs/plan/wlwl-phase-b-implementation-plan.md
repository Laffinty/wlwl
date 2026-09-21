<!-- # Phase B 实施计划 — 协程 runtime 骨架 -->

> **状态**:WIP — 计划阶段,未启动 B1 实施
> **前置**:v0.7 plan(`docs/plan/wlwl-build-plan-v0.7.md`)+ ADRs 0014/0015/0016 + Phase A 关闭
> **目标**:在 v0.6 tree-walking 解释器之上落地协同式单线程协程调度器骨架,B3 fidelity 测试守住单任务路径不退化
> **本阶段**:B1 类型定义 + 后续 B2-B6 规划;实际状态机改造留到 B5

## 0. 文档导读

### 0.1 Phase A 关闭确认(2026-09-21)

| 任务 | 状态 | 证据 |
|------|------|------|
| A1 plan 文档 | ✅ | `docs/plan/wlwl-build-plan-v0.7.md`, commit `01ff9d6` |
| A2 spec 草稿 §17 | ✅ | plan §附录 F |
| A3 ADR 0014/0015/0016 | ✅ | `docs/adr/0014-structured-concurrency-v0.7.md` 等 3 文件, commit `0d463ad` |
| A4 deviations.md v0.7 章节 | ✅ | `docs/plan/deviations.md` 顶部, commit `e9d2106`(+ 编码修复 `7421e2c` + `.gitattributes` `15ef61a`) |

**Phase A 100% 关闭**。

### 0.2 调研发现 — 必须在 B 开工前处理的项

#### 0.2.1 错误码冲突(E0050/51 已被 OOP 占用)

plan §4.4 把 `E0050`-`E0056` 预留给 v0.7 任务系统。但 `impl/crates/wlwl-error/src/lib.rs` 已占用 `E0050`/`E0051` 给 OOP(class inheritance / INIT arity)。

| 计划 | 实际可用 | 用途 |
|------|---------|------|
| ~~E0050~~ | **E0052** | SCOPE 中 fn 不是函数 (Phase C) |
| ~~E0051~~ | **E0053** | TASK 句柄无效 (Phase C/F) |
| E0052 | **E0054** | CHANNEL 已关闭后写入 (Phase D) |
| E0053 | **E0055** | CHANNEL 已关闭后读取 (Phase D) |
| E0054 | **E0056** | SPAWN 中 fn 参数个数 (Phase C) |
| E0055 | **E0057** | 跨 task 共享 cell 但 cell 不可变 (Phase C7) |
| E0056 | **E0058** | 顶层 SPAWN 无活跃 SCOPE (Phase C1) |

**决议**:全部下移 2 位(plan §4.4 表 → 实际用 E0052-E0058)。新增 `ErrorCategory::Concurrent` 一个分类。

**改动文件**:
- `impl/crates/wlwl-error/src/lib.rs` — enum 加 7 个 variant + 一个 category
- `docs/plan/wlwl-build-plan-v0.7.md` — §4.4 表更新码号,§附录 F spec 草稿同步
- `docs/plan/deviations.md` — 新增 `P7-B0-001` 偏离条目

#### 0.2.2 wlwl-eval 当前结构(本计划改写基础)

| 项 | 现状 |
|----|------|
| `lib.rs` 行数 | **14125 行 / 554 KB**(单文件,已膨胀) |
| `Evaluator` struct | 行 3547。已有字段: `env`, `source`, `file`, `loader`, `std_ctx`, `call_stack`, `warnings`, `current_span`, `format_cache`, `test_registry` |
| `Env` struct | 行 199。`scopes: Vec<HashMap<String, Cell>>` |
| `Cell` type | 行 184。`type Cell = Rc<RefCell<Binding>>`(单层,clone-by-Rc) |
| `Binding` struct | 行 174。`{ value: Value, mutable: bool }` |
| `eval_call` 方法 | 行 4493。签名 `fn eval_call(&mut self, name: &str, args: &[Expr], span: &Span) -> WlwlResult<Outcome>` |
| 评估模型 | **递归调用** — 每个 Expr 分支递归调用 `eval_expr` 处理子表达式 |

**关键观察**:当前 evaluator 是**纯递归**,没有栈机/状态机。要落地 plan §5.1 的"enum-driven stack machine",需要把递归改写成显式 continuation-passing,这是 Phase B 真正的难点。

#### 0.2.3 conformance 与 bench 基础设施

- **conformance 套件**: `impl/tests/conformance/*.wll`(10 文件,覆盖 core_subsets / err_propagation / index_bounds / numeric / match_patterns / destruct / closure_cell / module_paths / format_template / error_schema)
- **harness**: `impl/crates/wlwl-cli/tests/conformance.rs`,shell out `wlwl run <fixture>` 检查 exit code + JSONL
- **bench 套件**: `impl/crates/wlwl-eval/benches/eval_hot_paths.rs` — 5 个 workloads(simple_loop_1m / closure_density / string_concat / array_higher_order / error_propagation)
- **baseline**: `impl/crates/wlwl-eval/benches/baseline.txt`(Phase I1, 2026-09-19),Phase G8 性能门槛是 110% baseline
- **Plan §6.4.1 退化预算**: 单 task `eval_call` 6-8 ns → +2 ns(任务 ctx push/pop),总退化 ≤ +30%(留余量到 10% 阈值)

---

## 1. Phase B 任务总览(plan §3)

| # | 任务 | 本计划范围 | 风险 | 状态 |
|---|------|-----------|------|------|
| B0 | 错误码重映射(E0052-E0058) | **本计划内**(开工前置) | 低 | ⏸ 准备 |
| B1 | 新增 `wlwl-eval/src/runtime.rs`,定义 `Scheduler/TaskId/TaskState` + EvalState 类型 stub | **本计划内**(本会话做) | 低 | ⏸ 准备 |
| B2 | `Evaluator` 增加 `current_task: Option<TaskId>` | **本计划内**(本会话做) | 低 | ⏸ 准备 |
| B3 | **fidelity 测试**: v0.6 conformance fixture 在新 runtime 下输出不变 | **本计划内**(本会话建立 baseline) | 中 | ⏸ 准备 |
| B4 | 新增 `wlwl-eval/src/task.rs`,定义 `Task` | **本计划** | 中 | ⏸ 后续会话 |
| B5 | 调度循环 `Scheduler::run_until_idle()` + **eval 递归 → 状态机改造** | **本计划**(规划,实施留 B 后续会话) | **高** | ⏸ 后续会话 |
| B6 | 基础 benchmark: 单 task 退化 < 10% | **本计划**(规划,实施留 B 后续会话) | 中 | ⏸ 后续会话 |

---

## 2. Phase B 详细实施步骤

### B0 — 错误码重映射(开工前置,**必做**)

**目标**: 让 v0.7 任务/通道系统有独占错误码段(E0052-E0058),不与 OOP 冲突。

**改动**:
1. `impl/crates/wlwl-error/src/lib.rs`
   - enum `ErrorCode` 新增 7 个 variant:`E0052` `E0053` `E0054` `E0055` `E0056` `E0057` `E0058`
   - 每个加 `as_str()` 映射
   - 新增 `ErrorCategory::Concurrent`,7 个 variant 全部归到 Concurrent
   - 更新 header 注释行(行 23-24 附近)反映新段

2. `docs/plan/wlwl-build-plan-v0.7.md`
   - §4.4 表里 E0050-E0056 全部 +2(E0050→E0052 等)
   - §附录 F spec 草稿里 §17 段落同步引用

3. `docs/plan/deviations.md`
   - 在 v0.7 段下加 `P7-B0-001` 条目:偏离 plan §4.4 预分配码号,下移 2 位避免与 OOP 冲突

**验收**:
- `cargo build -p wlwl-error` 通过
- `cargo test -p wlwl-error` 全过(原有 snapshot 测试,新加 7 个 variant 不破坏现有)
- `cargo test -p wlwl-cli --test conformance` v0.6 fixture 全部通过(确认 OOP E0050/51 语义不变)

**commit message**: `fix(error): reserve E0052-E0058 for v0.7 task/channel system`

---

### B1 — `runtime.rs` 类型定义(**本会话实施**)

**目标**: 新增 `wlwl-eval/src/runtime.rs`,只定义类型 stub,**不连任何调度逻辑**。让类型先入库,后续 B4/B5 在此基础上加行为。

**改动文件**:
1. 新建 `impl/crates/wlwl-eval/src/runtime.rs`,内容:
   ```rust
   //! v0.7 coroutine runtime skeleton (Phase B1).
   //!
   //! Types only; no scheduler behaviour yet. Wires up in B4/B5.

   use std::collections::VecDeque;
   use std::rc::Rc;
   use std::cell::RefCell;
   use crate::{Env, Value};

   /// Identifies a task within a single Scheduler instance.
   #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
   pub struct TaskId(pub usize);

   /// Generation-tracked task handle returned to user code (`SPAWN`).
   #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
   pub struct TaskHandle {
       pub id: TaskId,
       pub generation: u64,
   }

   /// Task lifecycle state (plan §5.1.1).
   #[derive(Debug, Clone)]
   pub enum TaskState {
       Pending,
       Running,
       Suspended(YieldReason),
       Done { result: TaskResult },
       Cancelled,
   }

   #[derive(Debug, Clone)]
   pub enum TaskResult {
       Ok(Value),
       Err(Value),
   }

   /// Reasons a task may yield (plan §5.1.1).
   #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
   pub enum YieldReason {
       Explicit,           // YIELD()
       AwaitingChild(TaskId),
       ReceivingOn(RcHandle),
       SendingOn(RcHandle),
   }

   // Channel handle placeholder (placeholder type; Phase D defines real Channel).
   #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
   pub struct RcHandle(pub usize);

   /// Stack-machine evaluation state. NOT YET WIRED to eval_expr.
   /// Real implementation lands in B5; this stub exists so B4 Task
   /// can reference the type without forward declarations.
   #[derive(Debug)]
   pub enum EvalState {
       Expr(crate::Expr),
       ContStmt(Vec<ContFrame>),
   }

   #[derive(Debug, Clone)]
   pub enum ContFrame {
       /// Return value `v` from evaluating an expression; bind to local.
       BindLocal(String),
       /// Continue to next branch of compound expression.
       Next,
   }

   /// The scheduler. Skeleton; no behaviour in B1.
   pub struct Scheduler {
       pub tasks: Vec<TaskEntry>,
       pub run_queue: VecDeque<TaskId>,
       pub next_generation: u64,
   }

   #[derive(Debug)]
   pub struct TaskEntry {
       pub state: TaskState,
       pub env: Env,
       pub yield_points: Vec<EvalState>,
   }

   impl Scheduler {
       pub fn new() -> Self {
           Self {
               tasks: Vec::new(),
               run_queue: VecDeque::new(),
               next_generation: 0,
           }
       }
   }

   #[cfg(test)]
   mod tests {
       use super::*;
       #[test]
       fn taskid_distinct() {
           assert_ne!(TaskId(0), TaskId(1));
       }
       #[test]
       fn handle_includes_generation() {
           let h1 = TaskHandle { id: TaskId(0), generation: 1 };
           let h2 = TaskHandle { id: TaskId(0), generation: 2 };
           assert_ne!(h1, h2);
       }
   }
   ```

2. `impl/crates/wlwl-eval/src/lib.rs`
   - 加 `pub mod runtime;`(在文件顶部 module 声明区)
   - **不要** 在 Evaluator 加任何字段(B2 才加)
   - **不要** import `runtime::*` 到 lib.rs 顶层

**验收**:
- `cargo build -p wlwl-eval` 通过
- `cargo test -p wlwl-eval runtime` 新加的 2 个单元测试过
- `cargo test -p wlwl-cli --test conformance` 全 10 fixture 通过(确认 B1 没引入任何运行时副作用)
- 现有 `cargo test --workspace` 全过

**commit message**: `feat(eval): add runtime skeleton types for v0.7 (Phase B1)`

---

### B2 — `Evaluator.current_task` 字段(**本会话实施**)

**目标**: 在 `Evaluator` 加 `current_task: Option<TaskId>` 字段。**当前始终为 None**(没有调度器,没有任务运行)。后续 B4/B5 接入后才会有非 None 值。

**改动**:
1. `impl/crates/wlwl-eval/src/lib.rs` 行 3547 附近的 `Evaluator` struct:
   ```rust
   /// [v0.7 Phase B2] Current task in the cooperative scheduler.
   /// `None` until B5 wires the scheduler in; legacy (pre-v0.7) callers
   /// see `None` throughout -- the tree-walking evaluator runs as if
   /// it were the single implicit task.
   pub current_task: Option<crate::runtime::TaskId>,
   ```

2. `impl Evaluator { pub fn new() -> Self { ... } }` 初始化块加:
   ```rust
   current_task: None,
   ```

3. 在所有 `Evaluator::new()` 变体 / `with_source` / `Default` impl / 测试 helper 里同步初始化(全 codebase grep 一遍 `Evaluator::new`)

**验收**:
- `cargo build -p wlwl-eval` 通过
- `cargo test --workspace` 全过
- `cargo test -p wlwl-cli --test conformance` 全过(确认 v0.6 行为零退化)

**commit message**: `feat(eval): add Evaluator.current_task stub for v0.7 (Phase B2)`

---

### B3 — Fidelity 测试建立(**本会话实施**)

**目标**: 量化"什么都没改"情况下的基线,作为后续每次 B 步骤的对照参考。

**方法**:
1. 在 `impl/crates/wlwl-eval/tests/` 下新建 `v07_fidelity.rs`(workspace 级别 integration test)
2. 复用 `wlwl-cli/tests/conformance.rs` 的 fixture 列表,挨个跑 `wlwl run <fixture>`,捕获 stdout / exit code
3. 把当前(改动前)的输出存为 `tests/fixtures/v07_fidelity_v06_baseline.jsonl` 作为 golden
4. 测试断言: 每条 fixture 输出与 golden 字节相等

**验收**:
- 新测试 `cargo test -p wlwl-eval --test v07_fidelity` 通过
- 后续 B4/B5 改动后再跑一次,任何差异必须显式 `bless`(更新 golden)并附 commit message 说明

**commit message**: `test(eval): capture v0.6 conformance baseline as v0.7 fidelity golden (Phase B3)`

---

### B4 — `task.rs` 定义(**后续会话**)

**目标**: 定义 `Task` 数据结构 + Scope 容器。

**改动**:
1. 新建 `impl/crates/wlwl-eval/src/task.rs`,内容包含 `Task { id, generation, body: Rc<Closure>, env, state, yield_points, parent_scope }` + `Scope { id, tasks, parent, cancelled }` + 相关方法
2. `wlwl-eval/src/lib.rs` 加 `pub mod task;`
3. **不连调度器**

**风险**: 中(类型设计错了后面要返工)

**commit message**: `feat(eval): define Task and Scope types for v0.7 (Phase B4)`

---

### B5 — 调度循环 + eval 状态机改造(**后续会话,本次只规划**)

**目标**: 实现 `Scheduler::run_until_idle()`,把递归 eval_expr 改成 state-machine eval,**核心难点**。

**改动**:
1. `impl/crates/wlwl-eval/src/runtime.rs`: 加 `Scheduler::step()`, `Scheduler::run_until_idle()`, `Scheduler::dispatch_cancellations()`, `Scheduler::wake_dependents()`
2. `impl/crates/wlwl-eval/src/lib.rs`: 把 `eval_expr` / `eval_call` 改写成 state-machine 风格(`EvalState::Expr(...) + ContStmt(...)`),在 yield 点返回 `StepResult::Yield(reason)`
3. `Evaluator::current_task` 真正参与运行时:每次进 `eval_expr` 切换到对应 task

**风险**: **高**。Recursive → CPS 重写是 B 阶段最容易出 bug 的环节。建议分两步:
- B5a: 改造 `eval_expr` 但**不接调度**,只验证单 task 路径不退化(B3 fidelity 仍然通过)
- B5b: 接入 `Scheduler`,让 current_task 真的被设置/读取,引入 YIELD/Scope 内置函数

**commit message**:
- B5a: `refactor(eval): convert recursive eval_expr to state machine (Phase B5a)`
- B5b: `feat(eval): wire Scheduler into Evaluator with SCOPE/SPAWN stubs (Phase B5b)`

---

### B6 — 基准 + 退化验证(**后续会话**)

**目标**: 单任务路径退化 < 10%(plan §10 D20 + §6.4.1)。

**改动**:
1. 重跑 `cargo bench -p wlwl-eval --bench eval_hot_paths`,对比 `baseline.txt` 的 Phase I1 numbers
2. 如果退化 > 10%,在 deviations.md 加 `P7-G5-001` 条目并回到 plan §1.3 重评风险(plan §3 G5 锁定)
4. 把新 baseline 写进 `baseline.txt` 头部 commit 注释

**风险**: 中(state machine 改造可能引入 30-50% 退化,需要小心优化)

**commit message**: `bench(eval): record Phase B6 single-task regression baseline`

---

## 3. 风险登记(Phase B 特有)

| ID | 风险 | 概率 | 影响 | 缓解 |
|----|------|------|------|------|
| RB1 | 状态机改写后单 task 退化 > 10% | 50% | 高 | B5a 单独跑 fidelity,B5b 才接调度;退化超标回到 plan §1.3 重评 |
| RB2 | E0050/51 冲突漏处理,Phase C 实施时才暴露 | 100%(已知) | 中 | B0 在 B1 之前完成 |
| RB3 | `current_task: Option<TaskId>` 加在 `Evaluator` 里被 `&mut self` 借用检查器阻断 | 20% | 中 | 用 `Cell<TaskId>` 或 thread-local;B2 验证 |
| RB4 | 14125 行的 lib.rs 加新字段需要散落更新多处 `Evaluator::new()` | 30% | 低 | B2 实施时 grep `Evaluator::new` 全收 |
| RB5 | fidelity baseline golden 漂移(conformance 行为在 B 过程中被微小改变) | 30% | 中 | 每次漂移要求显式 commit message 解释 |

---

## 4. 验收总清单

Phase B 完整收尾时,以下全部成立:

- [ ] B0-E0050/51 冲突解决,E0052-E0058 归 Concurrent
- [ ] B1-`runtime.rs` 类型 stub 入库
- [ ] B2-`Evaluator.current_task: Option<TaskId>` 入库,默认 None
- [ ] B3-fidelity golden baseline 落盘,后续 B 步骤不漂移
- [ ] B4-`Task` + `Scope` 类型定义入库
- [ ] B5a-recursive eval → state machine 改造完成,单 task 路径 0 退化
- [ ] B5b-`Scheduler` 接入,current_task 真实参与调度
- [ ] B6-单 task 退化 < 10%,新 baseline 记录
- [ ] `cargo test --workspace` 全过
- [ ] `cargo test -p wlwl-cli --test conformance` 10 fixture 全过
- [ ] `cargo bench -p wlwl-eval --bench eval_hot_paths` 退化 ≤ 110% Phase I1 baseline
- [ ] `cargo clippy --workspace -- -D warnings` 0 warning
- [ ] `cargo doc --workspace --no-deps` 0 warning

---

## 5. 不在本计划范围(留给后续 Phase)

- **C/D/E/F**: SCOPE/SPAWN/AWAIT/YIELD/CHANNEL 内置函数注册,跨任务 ERR 传播,取消作用域 — Phase B 完成且 fidelity 通过后才开工
- **G**: 质量门禁(clippy / rustdoc / fuzz / cargo-deny / perf 承诺)— B6 通过且全部 decision D1-D20 在 §10 仍锁定时开工
- **H**: spec v0.7 定稿 + release — Phase G 通过后开工

---

## 6. 决策日志(本计划内做的微决策)

| 决策 | 选项 | 选择 | 理由 |
|------|------|------|------|
| B0 码号位移方向 | 下移(E0050→E0052)/ 上移(留 E0050s 给 OOP) / 拆分(同码号不同 category) | **下移 2 位** | 视觉上"任务/通道"作为一个连续段,容易在 OOP E0050/51 之后追加 |
| B1 EvalState 实现 | 完整实现 / 类型 stub | **类型 stub** | B5 才需要真正的状态机,B1 先把类型固化下来防止 B4 改来改去 |
| B2 current_task 类型 | `Option<TaskId>` / `Cell<TaskId>` / thread-local | **`Option<TaskId>`** | v0.6 没有并发语义,所有路径都 `None`;B5 接入时若借用冲突再换 Cell |
| B3 baseline 位置 | repo 内的 tests/fixtures/ / repo 外 CI artifact | **repo 内** | 让 reviewer 能 diff baseline 漂移 |
| B5 拆分粒度 | 一步到位 / 拆 a/b | **拆 a/b** | 5a(改造)+5b(接调度)风险量级不同,失败回退成本不同 |

---

## 7. 参考

- `docs/plan/wlwl-build-plan-v0.7.md` §3 Phase B / §5.1 / §5.1.1 / §5.2 / §5.4 / §6.4.1 / §附录 B ADR-0014/0015/0016
- `docs/adr/0014-structured-concurrency-v0.7.md`
- `docs/adr/0015-brown-9-dimension-decision.md`
- `docs/adr/0016-scheduler-single-thread-boundary.md`
- `impl/crates/wlwl-eval/src/lib.rs` — v0.6 evaluator(14125 行)
- `impl/crates/wlwl-error/src/lib.rs` — 错误码注册表
- `impl/tests/conformance/` — 10 fixture (B3 golden 来源)
- `impl/crates/wlwl-eval/benches/eval_hot_paths.rs` + `baseline.txt` — B6 性能预算参照

---

**审批**:本计划需 Li 批准后启动 B0 → B1 → B2 → B3。B4-B6 留后续会话。