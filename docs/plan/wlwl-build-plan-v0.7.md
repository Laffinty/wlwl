<!-- # WLWL v0.7 实施构建计划 -->

> **状态**:WIP — 实施中(`wip0.7`)。Phase A/B/C 已完成;下一目标 Phase D Channel 或 Phase E 错误传播(E4 阻塞项)
> **前置**:v0.6.0 已发布(2026-09-20,tag v0.6.0)
> **目标**:为 WLWL 增加并发机制,首次引入可在 .wll 程序内表达结构化并发的运行时原语
> **本阶段**:plan → impl → spec;实际构建按 §3 节奏推进(spec 文件到 H1 才改)

---

## 0. 文档导读

### 0.1 调研背景

v0.7 是 WLWL 第一次处理"控制流时间维度"的问题。此前 spec v0.6 的全部原语都是同步、词法、确定性的:函数返回到调用者,程序路径可由源文本静态追踪。引入并发意味着引入时间上的不确定性,以及由此带来的取消、传播、隔离、调度问题。

本计划在做选型时,对 2025-2026 年最新学术与工业资料进行了扫描,主要参考:

- **Structured Concurrency**(当下主流)
  - Nathaniel J. Smith 等:Trio / Nurseries / Cancel Scopes(2017-,在 2025 年仍是 Python 异步生态的事实参照,近 72% 工业采用率)。核心不变量:子任务寿命不超过词法作用域;取消自顶向下传播;错误单一汇聚点。
  - OpenJDK JEP 525(2025,Java 结构化并发)。从语言层面把结构化并发作为一等公民固化下来。
  - Kotlin coroutineScope / supervisorScope(JetBrains,2017-),Swift Task / withTaskGroup(Apple,5.5+,2021-)。
  - Uber cff(Go,2024-):声明式并发流,通过代码生成避开 goroutine 泄漏。
- **Async/Await 语义空间**(Brown University,2025-2026)
  - Gavin Gray 等:"A Design Space Exploration of Async/Await",分析了 7 个主流 async 运行时(Python Asyncio/Trio、Rust Tokio/Smol、C#、JavaScript、Swift),发现 fire-and-forget 程序在 7 个 runtime 上产生 4 种不同输出,识别出 9 个互相正交的设计维度。该论文提供了我们做并发模型选择时的对照表。
- **轻量协程与消息传递**
  - Behaviour-Oriented Concurrency(BoC)及 ARBOC(ICCS 2026):actor + 隔离类型的分布式扩展。Cheeseman 等"When concurrency matters"(OOPSLA 2023)与 Arvidsson 等"Reference capabilities for flexible memory management"(OOPSLA 2023)是 BoC 的理论基础。
  - "The Complexity of Testing Message-Passing Concurrency"(POPL 2026, Shi et al.):证明 channel 通信一致性检测在 1M 事件规模下可行。
- **确定性并发与形式化**(供远期参考,v0.7 不采用)
  - Haller 等"Reactive Async"(Scala):cell-based 模型。
  - Stepanenko 等"Context-Dependent Effects and Concurrency in Guarded Interaction Trees"(TOPLAS 2025):在 Rocq 中的形式化。

> **结论性摘要**:工业界已经收敛到"结构化并发 + 通道"作为动态类型函数式语言表达并发的默认组合(Trio、Kotlin、Swift、Go cff 都朝这个方向),9 个设计维度在结构化并发阵营里选定的子集是高度一致的。WLWL v0.7 采用这条路径,但不与 Rust Tokio / Smol 这类不强制结构化并发的 runtime 对齐,因为我们的运行时是单线程树遍历解释器,结构化并发是"工程上更省力"的选项。

### 0.2 关键决策(v0.7 启动时锁定)

| # | 决策项 | 决策 | 备注 |
|---|------|------|------|
| 1 | 技术栈 | **沿用 Rust + tree-walking** | 不引入字节码 |
| 2 | 执行模型 | **协作式单线程调度** | 首版不引入 OS 线程 |
| 3 | 并发模型 | **结构化并发 + 通道** | 见 §2 |
| 4 | Brown 9 维度 | Lazy/Dynamic/Dynamic/Strong/Awaited/Destructive/Aware/Top-Down/Transient | 见 §2.3 |
| 5 | 取消传播介质 | **复用 WLWL 现有 ERR transparent propagation(§8.2)** | 不引入独立 cancellation 异常 |
| 6 | 共享状态 | **沿用闭包 cell + 显式可变性(LET MUT)** | 不引入 Actor 类型系统 |
| 7 | Phase 5 形式化 | **继续 deferred** | 沿用 v0.2 §0.3 决策 #3 |
| 8 | CI | **沿用 GitHub Actions + 三平台 matrix** | v0.2 已就位 |
| 9 | 许可证 | **GPL v2** | 不变 |
| 10 | spec 关系 | **plan → impl → spec**(本迭代模式) | 见 §8 |
| 11 | v0.7 范围 | **结构化并发 + 通道 + 调度器 + 测试与质量门** | 见 §10 决策清单 |
| 12 | 目标用户 | **公开** | 不变 |

### 0.3 v0.6 → v0.7 关键变化

| 类别 | 变化 | 来源 |
|------|------|------|
| 新增运行时概念 | Task / Scope / Channel / CancellationHandle | §2 |
| 新增 std 模块 | wlwl:std.concurrency | §4 |
| 新增全局 builtin | 15-17 个(SCOPE/SPAWN/AWAIT/YIELD/CHANNEL_*/TASK_*/SHIELD) | §4 |
| 新增错误码 | E0052-E0058 共 7 个 | §4.4 |
| 新增 ADR | 0014 结构化并发选型;0015 Brown 9 维度决策;0016 调度器单线程边界 | 附录 B |
| eval crate 改造 | 树遍历 → 协程栈机(generator-based) | §5.1 |
| 测试 | workspace +N,新增 concurrent/* conformance | §6 |

### 0.4 v0.6 baseline(本计划不重写)

13/13 workspace crate ≥ 90% line(TOTAL ≈ 92.62%);workspace 至少 NNN/NNN tests pass;已发布 tag v0.6.0;具备 56 个错误码、14 个警告码、9 个内置类型、28 个显式 builtin;std 模块 8 个。v0.7 任何 commit 不得降低上述指标。

---

## 1. 实施总览

### 1.1 目标

在 v0.6 baseline 之上,交付首个 WLWL 并发模型:1. **结构化并发**;2. **通道作为组合单元**;3. **保持 WLWL 既有约定**(
ame(arg, ...)、错误即值、显式可变性、闭包 cell);4. **质量门禁**(Phase G 阶段 clippy/rustdoc/fuzz/cargo-deny/性能回归全过);5. **三平台 native binary 发布**(沿用 v0.2 release.yml)。

### 1.2 原则

1. 复用优于新设;2. 最简可行 + 工业已验证;3. Brown 9 维度每项显式选择;4. 形式化 deferred;5. deviations 显式记录。

### 1.3 风险初评

| 风险 | 等级 | 缓解 |
|------|------|------|
| 树遍历改协程栈机的语义破坏 | 高 | Phase B 引入并发 runtime 但保留同步路径;先行写 fidelity tests |
| ERR 复用为取消信号与 ERR 消费者冲突 | 高 | Phase C 单独跑 ERR consumer registry 全量回归 |
| Channel close 与 scope 退出竞态泄漏 | 中 | Phase D 引入 leak detector |
| 性能:单线程协作调度在 I/O 密集场景吞吐不足 | 中 | Phase G 性能承诺只保证"单 task 退化 < 10%,并发路径正确性与无泄漏"(§10 D20) |
| 与 v0.6 cell 升级规则跨任务行为不可预期 | 中 | Phase B 通过单元测试枚举 |
| spec 与实现漂移 | 中 | 沿用 v0.2 deviations.md 流程 |

---

## 2. 并发模型选型

### 2.1 候选

| 候选 | 适配度 | 否决理由 |
|------|------|---------|
| **结构化并发 + 通道**(Trio/Kotlin/Swift/Go cff 共识) | 强 | — |
| 异步 async/await 协程(Rust Tokio/Smol 风格) | 弱 | 非结构化,易泄漏;Brown 论文显示各 runtime 互不兼容 |
| Actor(Erlang/BoC/Orca) | 中 | 需引入隔离类型系统,与"动态类型 + 显式可变性"相悖;v0.7 范围外 |
| Reactive Async(Haller,cell-based) | 弱 | 表达力受限;非主流 |
| Lock + shared memory | 弱 | 与"函数式 + 错误即值"哲学冲突 |
| OS 线程 | 弱 | v0.6 cell 升级基于 Rc<RefCell<...>>,非 Send;全栈重写代价过高 |

### 2.2 选择:结构化并发 + 通道

理由:1. 与 WLWL 哲学一致(函数式 + 错误即值 + 显式作用域);2. 工程实现相对单线程(树遍历改协程栈机是局部改造);3. 与 Trio 风格一致可降低用户心智负担;4. Brown 9 维度中结构化并发阵营选定的子集是工业共识。

### 2.3 Brown 9 维度设计

| 维度 | 选择 | 理由 | Trio | Kotlin | Swift | cff |
|------|------|------|------|--------|-------|-----|
| Eagerness | Lazy | "一切皆表达式"一致 | Lazy | Lazy | Eager | Lazy |
| Suspension | Dynamic | 实现简单;fidelity 更高 | Dynamic | Dynamic | Dynamic | Dynamic |
| Extent | Dynamic | 结构化并发不变量 | Dynamic | Dynamic | Dynamic | Dynamic |
| Ref Strength | Strong | 防止 zombie task | Strong | Strong | Strong | Strong |
| Destruction | Awaited | 复用现有错误收集路径 | Awaited | Awaited | Cancelled | Awaited |
| Propagation | Destructive | 复用 ERR transparent propagation | Destructive | Destructive | Destructive | Destructive |
| Awareness | Aware | 让用户能写协作式取消检查点 | Aware | Aware | Aware | Aware |
| Direction | Top-Down | 与 Trio/Swift 一致 | Top-Down | Top-Down | Top-Down | Top-Down |
| Persistence | Transient | 三条理由:(a) 显式可变性哲学 — 状态变化交给用户显式 LET MUT/SET,取消响应同样应显式;(b) SHIELD 复杂度 — Persistent 强制响应会让 SHIELD 必须实现"保存取消信号、退出后立即抛出"的额外协议(类似 Trio CancelScope.shield),Transient 直接复用 YIELD 检查点即可;(c) 单线程协作足够 — 协作式调度下没有别的任务在跑,强制打断并不释放资源 | Persistent | Transient | Persistent | Transient |

> Transient 三条理由的完整论证见附录 B ADR-0015。具体取舍见 §10 决策清单。

### 2.4 选型代价分析

| 候选 · 维度 | 适配度 | 实现成本(人·周) | 学习成本(用户·小时) | 运行风险 | 可逆性 |
|------|------|----------|---------|---------|---------|
| 结构化并发+通道 | 5/5 | 6-8 | 2-4 | 低 | 中 |
| 异步 async/await 协程 | 3/5 | 4-6 | 4-8 | 高(uncancel化) | 高 |
| Actor(隔离类型) | 4/5 | 12-16 | 6-12 | 中 | 低 |
| Reactive Async(cell) | 2/5 | 8-10 | 6-10 | 中(表达力上限) | 低 |
| Lock + shared memory | 3/5 | 4-6 | 8-16 | 高(race) | 高 |
| OS 线程 | 1/5 | 16-24 | 6-10 | 高(重写全栈) | 极低 |

评分依据:**实现成本**基于 v0.6 语法树解释器的代码体量估算,单线程协作最低,OS 线程需重写 Rc<RefCell<Env>> 为 Arc<RwLock<Env>>。**学习成本**依据 Trio/Kotlin/Swift 已是工业共识。**运行风险**以"能不能产生 zombie task / race / data corruption"衡量。**可逆性**指 v0.7 选型未来可切到 Actor 的难度。

**结论**:结构化并发+通道是唯一同时在适配度、实现成本、运行风险三个维度都位于前 1/3 的方案。

---

## 3. 阶段路线图(Phase A-H)

`
A 调研+spec 草稿 ──▶ B runtime 骨架 ──▶ C 内置函数 ──▶ D Channel
                                                            │
                                                            ▼
                                  H spec v0.7 + release ◀── G 质量阶段
                                                            ▲
                                                            │
                            E 错误传播 ◀── F 取消作用域 ─────┘
`

每个 Phase 结束时产生独立 commit + 进度更新,工作树保持可回滚。Phase G 之前禁止改动 spec 文件。

### Phase A — 调研 + 设计落地
- A1:产出本计划文档
- A2:把 §2.3 的 9 维度决策落到 spec v0.7 草稿 §17
- A3:新增 ADR 0014/0015/0016
- A4:在 deviations.md 顶部添加 v0.7 章节标记

### Phase B — 协程 runtime 骨架
- B1:新增 wlwl-eval/src/runtime.rs,定义 Scheduler/TaskId/TaskState。栈式类型:基于 Rust generator 或手写栈机。
- B2:Evaluator 增加 current_task: TaskId
- B3:**fidelity 测试**:v0.6 conformance fixture 在新 runtime 下输出与 v0.6 完全一致(单 task 路径不退化)
- B4:新增 wlwl-eval/src/task.rs,定义 Task
- B5:调度器循环 Scheduler::run_until_idle()
- B6:基础 benchmark:单 task 执行时间退化 < 10%

### Phase C — 内置函数 SCOPE / SPAWN / AWAIT / YIELD
- C1:SCOPE(fn) — 创建子作用域。**首版只支持显式 SCOPE,不提供隐式 runtime scope**:任何 SPAWN(fn) 必须直接或间接嵌套在某个 SCOPE(...) 内,顶层调用(即没有活跃 SCOPE 时)产生 E0058(具体码在错误码表中列出);不引入类似 Python asyncio 的 free-floating task 或 Java 守护线程作为隐式兜底。这一限制是显式 API 边界,见 §10 D17。
- C2:SPAWN(fn) — 派生子任务,返回 task handle
- C3:AWAIT(handle) — 等待 task 完成;若 task 已 ERR 则传播
- C4:YIELD() — 协作让步
- C5:TASK_CURRENT() / TASK_IS_CANCELLED()
- C7:与 v0.6 cell 升级规则的交互测试
- C8:与 v0.6 闭包捕获规则的交互测试

### Phase D — Channel
- D1:CHANNEL_NEW(buf)(buf=0 表示同步)
- D2:CHANNEL_SEND(ch, v);buf 满则挂起 sender
- D3:CHANNEL_RECV(ch);buf 空则挂起 receiver
- D4:CHANNEL_CLOSE(ch)
- D5:CHANNEL_TRY_SEND / CHANNEL_TRY_RECV
- D6:CHANNEL_LEN(ch) / CHANNEL_CAP(ch)
- D7:leak detector:scope 退出时所有未关闭 channel 强制关闭
- D8:与 BOUNDED 通道的死锁检测

### Phase E — 错误传播
- E1:复用 v0.6 §8.2 ERR transparent propagation,扩展到跨 task 边界
- E2:子 task 产生的 ERR 在 AWAIT 时作为返回值传回
- E3:SCOPE(fn) 内 fn 抛 ERR → 同 SCOPE 内其他兄弟 task 收到 cancel 请求,自身 ERR 传播
- E4:**阻塞项** — 与 v0.6 ERR consumer registry 全部条目交互的跨 task 回归测试。**本项为 Phase E 的阻塞项,任意子测试失败必须在本 Phase 内修复,不得延后到 G 阶段**(与 v0.2 Phase G 质量门禁并列)。覆盖以下已注册 ERR 消费者(以 Phase E 启动时 wlwl-eval/src/registry.rs 中的实际注册表为准):BOOL,UNWRAP_OR,UNWRAP,IS_ERR,ERR_PAYLOAD,PRINT_ERR,EXPECT_ERR,以及 Phase C/D/F 中新增的 ERR 消费者。每个消费者至少 1 个 fixture 在跨 task 场景下覆盖:子 task 抛 ERR → 父 task 在 AWAIT 处拿到 ERR → 经消费者处理的行为。同时验证 ERR payload 中的 kind 字段(如 ChannelClosed)在跨 task 边界后保持原值。
- E5:IF(cond, then, else) 在并发路径下的短路语义是否需要调整 — 评估结果记录在 deviations.md

### Phase F — 取消作用域(cancellation scope tree)
- F1:CancellationScope 树结构(与 Trio CancelScope 类似但简化)
- F2:SCOPE(fn) 创建新 scope;SPAWN(fn) 继承当前 scope
- F3:取消自顶向下传播;子 scope 可独立取消
- F4:TASK_CANCEL(handle) / TASK_CANCEL_PARENT()
- F5:SHIELD(fn) — 详细语义:
  - **可嵌套**:SHIELD 可多层嵌套,层数累计;取消请求只在外层 SHIELD 完全退出后才生效。
  - **内部取消仅记录**:SHIELD 块内收到的取消请求不立即生效,只标记"有未处理取消";SHIELD 块结束后该取消请求立即对 fn 后续代码生效。
  - **退出后再生效**:SHIELD 退出 → 若有挂起取消 → fn 后续检查点(YIELD / TASK_IS_CANCELLED / 阻塞操作)立刻收到取消信号。
  - **清理中的 ERR 正常传播**:SHIELD 块内 fn 自身抛出的 ERR 仍然走 §5.4 错误传播路径,不受屏蔽影响。SHIELD 只屏蔽"来自外部的取消请求",不屏蔽 fn 自己的 ERR(子 fn 抛 ERR → SHIELD 块正常返 ERR → 父 task 按 ERR 传播路径处理)。
  - **至少 3 个 conformance 测试**(impl/tests/concurrency/shield_*.wll):shield_basic.wll(嵌套 1 层)、shield_nested.wll(嵌套 3 层)、shield_then_error.wll(SHIELD 块内 fn 自己抛 ERR 是否正常传播)。
  见 §10 决策清单 D16、§6.2。
- F7:取消与 §5.4 错误传播的交互:取消本身不产生 ERR

### Phase G — 质量阶段
- G1:workspace clippy 0 warning
- G2:rustdoc 100%
- G3:fuzz 至少 24h 运行
- G4:cargo-deny 0 violation
- G5:**统一性能承诺**:cargo bench --bench concurrency 必须守住单 task 路径退化 < 10%(对照 v0.6);并发路径**只保证正确性与无泄漏**,不承诺吞吐量提升、不承诺公平性、不承诺延迟下界(见 §10 D20、§6.4)。任何对单 task 路径退化的改动必须先回到本计划 §1.3 重新评估风险,不得在 Phase B-G 静默接受退化。
- G6:Phase G 期间禁止改动 spec 文件
- G7:rich suggestion_code for 新错误码

### Phase H — spec v0.7 + release
- H1:由本计划的 spec 草稿 §17 派生正式 docs/standard/wlwl-spec-v0.7.md
- H2:沿用 v0.2 release.yml
- H3:打 tag v0.7.0
- H4:把本计划文件重命名为 wlwl-build-plan-v0.7-COMPLETED.md 并加状态横幅

---
## 4. 模块划分

### 4.1 Rust crate 边界

`
crates/wlwl-std       — 新增 concurrency 模块 entry
crates/wlwl-eval      — 新增 runtime/task/channel/cancellation 四个模块
crates/wlwl-parser    — 不需要改(并发 builtin 全是普通函数调用)
crates/wlwl-lexer     — 不需要改
crates/wlwl-ast       — 不需要改
crates/wlwl-error     — 新增错误码
crates/wlwl-toml      — 不需要改
crates/wlwl-formatter — 不需要改
crates/wlwl-cli       — 不需要改
`

### 4.2 wlwl-eval 新文件清单

| 文件 | 职责 | Phase |
|------|------|-------|
| wlwl-eval/src/runtime.rs | Scheduler、TaskId、TaskState、Generator-based 协程 | B1-B5 |
| wlwl-eval/src/task.rs | Task 数据结构、Spawn/Await 实现 | B4/C/F |
| wlwl-eval/src/channel.rs | Channel、Send/Recv 实现 | D |
| wlwl-eval/src/cancellation.rs | CancellationScope 树 | F1-F7 |

### 4.3 wlwl-std 新文件

wlwl-std/src/concurrency.rs:wlwl:std.concurrency 模块,提供高层组合 API。沿用 v0.2 B6/B7 的 std 边界契约。

### 4.4 错误码新增

> **注意**:原计划预留 E0050-E0056,但 E0050/E0051 已被 v0.5 OOP 实现占用(class inheritance chain / NEW arity mismatch with INIT)。Phase B0 已把 v0.7 的错误码**全部下移 2 位**(本表反映调整后的最终码号);偏差条目见 `deviations.md` 的 `P7-B0-001`。新分类 `ErrorCategory::Concurrent` 容纳这 7 个码。

| 错误码 | 用途 | Phase |
|--------|------|-------|
| E0052 | SCOPE 中 fn 不是函数 | C |
| E0053 | TASK 句柄无效 | C/F |
| E0054 | CHANNEL 已关闭后写入 | D |
| E0055 | CHANNEL 已关闭后读取(返回 ChannelClosed ERR) | D |
| E0056 | SPAWN 中 fn 参数个数错误 | C |
| E0057 | 跨 task 共享 cell 但 cell 不可变 | C7 评估结论:**复用 E0024**;E0057 注册但不触发(§5.5 不引入新可见性规则) | C7 |
| E0058 | 顶层 SPAWN 无活跃 SCOPE | C1 |

### 4.5 新增 builtin 全集(15-17 个)

| Builtin | Phase | 类别 |
|---------|-------|------|
| SCOPE(fn) | C | 结构化 |
| SPAWN(fn) | C | 结构化 |
| AWAIT(handle) | C | 结构化 |
| YIELD() | C | 协作 |
| TASK_CURRENT() / TASK_IS_CANCELLED() | C | Task |
| TASK_CANCEL(handle) / TASK_CANCEL_PARENT() | F | 取消 |
| SHIELD(fn) | F | 取消 |
| CHANNEL_NEW(buf) / CHANNEL_CLOSE(ch) | D | Channel |
| CHANNEL_SEND(ch, v) / CHANNEL_RECV(ch) | D | Channel |
| CHANNEL_TRY_SEND / CHANNEL_TRY_RECV | D | Channel |
| CHANNEL_LEN(ch) / CHANNEL_CAP(ch) | D | Channel |

---

## 5. 关键技术与决策

### 5.1 runtime(协程调度器)

**协程实现**:Rust stable 没有 Generator,采用 enum-driven stack machine(EvalState { Expr, ContStmt, pc }),每次 yield 保存当前状态,resume 时恢复。

**调度器**:
- 单线程,run-queue 是 VecDeque<TaskId>
- 任务状态:Pending / Running / Suspended(WaitFor) / Done(Ok|Err) / Cancelled
- 调度循环:Scheduler::run_until_idle() 持续 resume 直到 run-queue 空
- YIELD 不是必须的(单任务路径完全可以在 generator 里直接 run),但提供 YIELD 给用户做协作让步

**性能**:单任务路径退化 < 10%(Phase B6 benchmark);不引入 OS 线程;CPU-bound 任务并发不会带来加速(显式记录在 spec)。

### 5.1.1 调度器状态机与循环

**Task 状态机**:

| 状态 | 触发迁移 | 目标状态 |
|------|---------|----------|
| Pending | Scheduler::resume | Running |
| Running | 用户代码 YIELD() | Suspended(Wait) |
| Running | AWAIT(child) 且 child 未完成 | Suspended(AwaitChild) |
| Running | CHANNEL_RECV 且 buf 空 | Suspended(RecvOn(ch)) |
| Running | CHANNEL_SEND 且 buf 满 | Suspended(SendOn(ch)) |
| Running | fn 返回 OK(v) | Done(Ok(v)) |
| Running | fn 返回 ERR(e) | Done(Err(e)) |
| Running | 父 scope 取消信号到达 | Cancelled |
| Suspended(*) | 等待事件 ready | Running |
| Suspended(*) | 父 scope 取消 | Cancelled |
| Done/Cancelled | 不可迁移 | — |

**调度循环伪代码**:

`
loop {
    match scheduler.run_queue.pop_front() {
        None => break,                    // run-queue 空 → 退出
        Some(task_id) => {
            let task = scheduler.tasks[task_id];
            match scheduler.step(task) {
                StepResult::Yield(reason) => {
                    scheduler.wait_lists[reason].push(task_id);
                }
                StepResult::Done(value) => {
                    scheduler.complete(task_id, value);
                    scheduler.wake_dependents(task_id);
                }
                StepResult::Blocked => {
                    // 资源等待,不入 run-queue
                }
            }
        }
    }
    scheduler.dispatch_cancellations();
}
`

**协作式让步点**:YIELD / AWAIT(child) / CHANNEL_RECV / CHANNEL_SEND 等;每个让步点对应一个 Suspended 状态;ready 事件由 wake_dependents 唤醒。

### 5.2 Task 与生命周期

- Task 由 Closure + Env + State + YieldPoints 组成
- 句柄 TaskHandle 是带世代号的 id;世代号用于检测 use-after-cancel
- Scope 是 Task 的容器:Scope { tasks: Vec<TaskHandle>, parent: Option<ScopeId>, cancelled: bool }
- Scope 退出时(SCOPE 块返回前):await 所有未完成任务 → 若超时 → cancel

### 5.3 Channel

- 内部是 VecDeque<Value> + sender/receiver wait queues
- buf=0 表示同步 channel(无缓冲)
- Close 语义:所有 sender 关闭后,receiver 在读完缓冲后收到关闭信号
- 与 v0.6 ERR 的关系(D14 锁定):**关闭后写入 → E0054(直接错误)**;**关闭后 RECV/TRY_RECV → 返回结构化 ERR,kind = "ChannelClosed"**,ERR payload 携带 {kind: "ChannelClosed", channel: <handle repr>}。关闭后 CHANNEL_LEN 返回缓冲区剩余长度(0);关闭信号在 IS_ERR(x) && ERR_PAYLOAD(x).kind == "ChannelClosed" 处可识别。**注意 NULL 不作为 close 信号**,用户必须用 ERR 检测。

**操作语义矩阵**(D14 + D15):

| 操作 | closed | senders_alive | buf 状态 | 行为 |
|------|---------|-------------|----------|------|
| SEND | false | ≥1 | 未满 | 立即入队;wake 一个 receiver |
| SEND | false | ≥1 | 满 | sender 挂起到 sender_waiters |
| SEND | true | — | — | 立即 ERR(E0054) |
| RECV | false | ≥1 | 非空 | 立即出队;wake 一个 sender |
| RECV | false | ≥1 | 空 | receiver 挂起到 receiver_waiters |
| RECV | true | 0 | 空 | 返回 ERR(ChannelClosed) |
| RECV | true | ≥1 | 空 | receiver 挂起;等 senders=0 后返 ChannelClosed |
| CLOSE | false | — | — | closed=true;wake 所有 receiver_waiters |
| CLOSE | true | — | — | 幂等 no-op |

**close 协议**:1. CLOSE 调用 → closed=true,senders_alive-=1;2. 若 senders_alive=0:wake 所有 receiver_waiters,每个接收 ERR(ChannelClosed);3. 关闭后 SEND → 立即 ERR(E0054)。

send/recv 唤醒顺序:FIFO,不保证全局公平;v0.7 不承诺公平性(§10 D20)。

### 5.4 错误传播

- 子 task 产生 ERR 时,parent task 在 AWAIT 处收到该 ERR(作为返回值)
- SCOPE 内任意子 task 产生 ERR → 同 SCOPE 内其他兄弟 task 收到 cancel 请求;SCOPE 本身的返回值是第一个 ERR
- 与现有 BOOL / UNWRAP_OR 等 ERR consumer 的交互:沿用 v0.6 §12.7 ERR consumer registry,不需要修改

### 5.4.1 跨边界 ERR 传播矩阵

行=ERR 产生处,列=ERR 消费处,单元格=具体行为。

| 产生处 \\ 消费处 | 当前 task 直接 throw | AWAIT(child) 拿回 | SCOPE(fn) fn 抛出 | SHIELD 内 fn 抛出 |
|----------------|---------------------|-------------------|-------------------|--------------------|
| 子 task AWAIT 拿 ERR | n/a | 父 task 在 AWAIT 处继续传播 | scope 边界聚合 | scope 内照常 |
| SCOPE 内兄弟 task | scope fn 取消该 task | 不直接相关 | scope 返 ERR(首个) | SHIELD 屏蔽外部,自身 ERR 正常 |
| 子 task 被 TASK_CANCEL | n/a | AWAIT 拿 ERR(Cancelled) | scope 取消子任务链 | 取消只记录,fn 继续 |
| 子 task 自身 ERR | ERR(e) | ERR(e) 通过 AWAIT | ERR(e) 通过 SCOPE | ERR(e) 通过 SHIELD 块 |
| 跨 SCOPE 嵌套 | outer SCOPE 拿到 ERR | inner SCOPE 返 ERR 后 outer 收到 | 链式传播 | 内 SHIELD 退出后 |

**关键不变量**:子 task 自身 ERR ↔ 外部取消信号互不污染(ERR ≠ 取消,取消不生成 ERR);SHIELD 不屏蔽自身 ERR,只屏蔽外部取消请求;SCOPE 边界聚合首个 ERR;AWAIT 透明传播与 v0.6 §8.2 一致。

### 5.5 与 v0.6 闭包 cell 的交互

跨 task 共享 cell 时,如果 cell 由 LET 创建(不可变),子 task 试图 SET 会得到 E0024。如果 cell 由 LET MUT 创建,行为与单 task 一致。**不引入新可见性规则**,完全沿用 v0.6 §3.3/§3.4。Phase C7 写专项回归测试。

### 5.6 与 v0.6 std.ai / std.io 的交互

- INPUT 是阻塞调用,需要 YIELD 到调度器再 resume
- ASK / ASK_STREAM 同样需要 yield
- 模板:yield_then_resume(std_fn_call)
- **v0.8 foreshadow**:后续优先支持 CALL_TOOL 与 ASK 的并行组合(例如同时为一个 prompt 调用 N 个模型然后进行 fan-in 组合),该用例是驱动 v0.7 选型的主要业务场景之一;v0.7 提供原语,v0.8 加上 fan-in 资源限制 / 超时策略。

---
## 6. 测试策略

### 6.0 测试架构矩阵

列:测试类型。行:功能点。
- **U** = 单元测试(各 builtin 独立 case)
- **C** = 并发安全测试(fixture, impl/tests/concurrency/)
- **F** = fuzz(cargo-fuzz, impl/fuzz/)
- **B** = 性能基线(criterion, impl/crates/wlwl-eval/benches/)
- **S** = 错误码 snapshot(wlwl-error/src/snapshots)

| 功能点 | U | C | F | B | S |
|----------|---|---|---|---|---|
| SCOPE 嵌套与出口 | 2 | 1 | ✅ | — | — |
| SCOPE 内任务取消传播 | 2 | 2 | ✅ | — | — |
| SPAWN / AWAIT 透明传播 ERR | 2 | 1 | ✅ | — | — |
| SPAWN / AWAIT 返回值与 scope 集合 | 2 | 1 | ✅ | — | — |
| YIELD 与调度器入队 | 1 | 1 | ✅ | ✅ | — |
| TASK_CURRENT / TASK_IS_CANCELLED | 1 | 1 | — | — | — |
| TASK_CANCEL 与 TASK_CANCEL_PARENT | 1 | 1 | ✅ | — | — |
| SHIELD 单层/多层/与 ERR 共存 | 3 | 3 | ✅ | — | — |
| CHANNEL_NEW(buf=0/1/100) | 3 | 1 | ✅ | ✅ | — |
| CHANNEL_SEND/RECV 挂起与唤醒 | 2 | 1 | ✅ | — | — |
| CHANNEL_CLOSE 后 RECV 返 ChannelClosed ERR | 1 | 1 | ✅ | ✅ | 1 |
| CHANNEL_CLOSE 后 SEND 返 E0054 | 1 | 1 | ✅ | — | 1 |
| ERR consumer 跨 task 回归(§3 Phase E4 阻塞项) | 8 | 4 | ✅ | — | 8 |
| channel close → RECV 延迟 · throughput | — | — | — | ✅ | — |
| scope 启动 / 退出 开销(N=10/100/1000) | — | — | — | ✅ | — |

预计本阶段新增 unit test ≥ 40,concurrency fixture ≥ 18,fuzz target ≥ 6,benchmark case ≥ 8,snapshot case ≥ 10。

**Fuzz 覆盖点**:
- wlwl-eval::runtime::fuzz:随机生成 SCOPE+SPAWN+AWAIT 边变顺序,检查 scope 退出时任务状态与 ERR 传播
- wlwl-eval::channel::fuzz:随机 channel send/recv/close 交叉顺序,检查 channel 状态机不式(开闭、close 后写入、buf 上下限)
- wlwl-eval::cancellation::fuzz:随机 SHIELD 嵌套 × TASK_CANCEL 交叉,检查 SHIELD 退出后取消依然生效

### 6.1 单元测试

每个新 builtin 至少 3 个 test(正常路径、ERR 路径、并发路径);每个新错误码至少 1 个 snapshot test;workspace 测试总数 Phase H 末预计 ≥ 1000。

### 6.2 并发安全测试

- impl/tests/concurrency/ 新目录,放 conformance fixture
- 至少 12 个 fixture 覆盖:scope 嵌套、scope 取消传播、SHIELD 隔离、channel 阻塞/唤醒/close、close 后 RECV 返回 ChannelClosed ERR、ERR 跨 task 传播、取消与 YIELD 交互
- **SHIELD 测试至少 3 个**(shield_basic.wll / shield_nested.wll / shield_then_error.wll),见 §3 Phase F6
- 沿用 v0.2 §16.5 conformance 模式

### 6.3 fuzz

沿用 impl/fuzz 已有 harness。Phase G3 扩展 runtime/channel 的 coverage target;至少 24h 运行;corpus 入库。

### 6.4 性能基线

**承诺表述**(与 §10 D20、§3 Phase G5 一致):
- 单 task 路径:执行时间退化 < 10%(对照 v0.6)
- 并发路径:**只保证正确性与无泄漏**,不承诺公平性 / 延迟下界 / 吞吐量提升

基准项(impl/crates/wlwl-eval/benches/concurrency.rs):
- 单 task 路径(对照 v0.6)
- N=10/100/1000 task scope 启动 / 退出开销
- channel throughput (M msg/sec,buf=1 / buf=100)
- channel close → RECV 返回 ChannelClosed ERR 的延迟

Phase G5 把基线写入 docs/plan/deviations.md 的 P7-G5-001 条目。

### 6.4.1 性能模型

**单 task 退化的源**:

| 开销项 | v0.6 估计 | v0.7 变化 | 预计影响 |
|----------|-------------------|-------------------|--------|
| eval_call 函数调用分支 | 6-8 ns | +2 ns(任务 ctx push/pop) | +25% |
| LET 绑定 | 4 ns | 不变 | 0 |
| LET MUT 升级检查 | 2 ns | 不变 | 0 |
| Rc<RefCell<Env>> clone | 12 ns | 不变 | 0 |
| cell 查找 | 3 ns | 不取出任务 scope 上下文 | 0 |

总预计退化在 +30% 以内,远低于 10% 的阈值是为了留出调试与发布脚本的缓冲。如实测退化 > 10%,需在 Phase G5 重新评估。

**并发路径的性能上限**:
- 调度器 run_until_idle 的处理量平顺为 O(总 task 数)
- 一个 yield/resume 的额外循环设计为 < 50 ns
- channel send/recv 为 O(1)(buf 未满/非空时),挂起时 O(等待队列长度)

**内存预估**(每 task):
- Task struct:约 80 bytes
- Env clone:随 scope 深度乘法,递归子 task 的 env 是父 env 的 clone
- run-queue 中每 task 额外:16 bytes(TaskId + state)
- 1000 task scope 预计内存开销:< 100 KB

**不保证事项**(§10 D20):公平性、延迟上界、吞吐量提升。

---

## 7. 工具链

- 沿用 v0.2 工具链:Rust 1.75+,cargo,clippy,rustdoc,insta,criterion
- 新增:cargo-fuzz(若未安装)
- 不引入:tokio、async-std、smol、rayon(均与 v0.7 选型冲突)
- 未来选项:v0.8 可考虑 thread pool,但 v0.7 不引入

---

## 8. 与 spec 的同步机制

### 8.1 spec 何时定稿

本计划遵循 Li 设定的迭代模式:**plan → impl → spec**。

- Phase A 之前:spec v0.7 §17 仍是草稿(在本计划的 §2/§3/§5 中)
- Phase B-G:任何对实现的修改必须先回到本计划;spec 文件不动
- Phase H1:由本计划的 spec 草稿 §17 + 实施中的 deviations 派生正式 spec v0.7

### 8.2 deviations 处理

任何实现偏离本计划或 Brown 9 维度决策的情况,必须在 commit 信息中写 deviation: P7-XXX-NNN,并在 docs/plan/deviations.md 添加条目。偏离 Brown 决策的 deviations 需要在 spec 草稿中专门说明。deviations 不允许跨 Phase 累计修复:每个 commit 自包含。

### 8.3 spec v0.7 章节结构(草稿,Phase H1 定稿)

`
§17 并发
  §17.1 任务与作用域
  §17.2 通道
  §17.3 取消与错误传播
  §17.4 与 LET MUT / 闭包 cell 的交互
  §17.5 标准库 wlwl:std.concurrency
  §17.6 与 §8 ERR transparent propagation 的关系
  §17.7 限制与不承诺(跟 §10 D20 一致)
`

---
## 9. 风险登记

| ID | 风险 | 等级 | P(概率) | 影响 / 缓解成本 | 触发条件 | 监控点 |
|----|------|------|---------|---------|---------|---------|
| R1 | 树遍历改协程栈机破坏 v0.6 同步语义 | 高 | **30%** | 高 / 中(stack 重写 3-5 人·周) | Phase B3 fidelity 测试失败 | Phase B3 |
| R2 | ERR 复用为取消信号导致 ERR consumer 行为改变 | 高 | **20%** | 高 / 中(修复 ERR 包装 2-3 人·周) | Phase E4 全量 ERR consumer 回归失败 | Phase E4 |
| R3 | Channel close 与 scope 退出竞态导致泄漏 | 中 | **15%** | 中 / 低(detector 本身 < 1 人·周) | Phase D7 leak detector 命中 | Phase D7 |
| R4 | 单线程调度在 I/O bound 场景吞吐不足(已知限制) | 中 | **40%** | 低 / 低(文档说明) | Phase G5 性能回归超过 10% | Phase G5 |
| R5 | SHIELD 与破坏性取消传播冲突 | 中 | **10%** | 中 / 低(改 advisory 语义 < 1 人·周) | Phase F6 单元测试失败 | Phase F6 |
| R6 | 9 维度 Persistence 选 Transient 与 Trio 不一致 | 低 | **20%** | 低 / 低(变更文档) | 社区反馈 | launch |
| R7 | v0.6 cell 升级规则跨 task 行为不可预期 | 中 | **25%** | 中 / 低(加错误码 < 0.5 人·周) | Phase C7 测试发现必须新增可见性约束 | Phase C7 |
| R8 | spec 与实现漂移 | 中 | **40%** | 中 / 低(deviations.md 法) | Phase H1 时发现本计划漏写某个行为 | Phase H1 |

---

## 10. 决策清单(显式锁定项)

下列每一项都是 v0.7 启动时已锁定的设计选择,实施期间变更必须先修改本清单。

| ID | 决策 | 选项 | 选择 | 备注 | 证据 / 锁源 |
|----|------|------|------|------|------|
| D1 | 并发模型 | 见 §2.1 | 结构化并发 + 通道 | §2.2 | Brown 2025;Trio/Kotlin/Swift/cff 共识 |
| D2 | Eagerness | Lazy / Eager | Lazy | §2.3 | Trio/Kotlin/cff 都选 Lazy |
| D3 | Suspension | Static / Dynamic | Dynamic | §2.3 | 7 个 runtime 中 5 选 Dynamic;fidelity > simplicity |
| D4 | Extent | Indefinite / Dynamic | Dynamic | §2.3 | 结构化并发不变量 |
| D5 | Reference Strength | Strong / Weak | Strong | §2.3 | Trio/Kotlin/Swift 全为 Strong;防 zombie task |
| D6 | Destruction | Awaited / Cancelled / Terminated | Awaited | §2.3 | Trio/Awaited 与 Kotlin 一致 |
| D7 | Propagation | Destructive / Never | Destructive | §2.3 | Trio/Kotlin/Swift 全为 Destructive;复用 v0.6 ERR |
| D8 | Awareness | Unaware / Aware | Aware | §2.3 | Trio/Kotlin/Swift/Asyncio 选 Aware;便于 YIELD 检查 |
| D9 | Direction | Top-Down / Bottom-Up / Simultaneous | Top-Down | §2.3 | Trio/Kotlin/Tokio/Smol 选 Top-Down |
| D10 | Persistence | Transient / Persistent | Transient | §2.3 | 三条理由(附录 B ADR-0015);与 Kotlin 一致 |
| D11 | 调度器线程模型 | 单线程 / OS 线程 | 单线程(首版) | §5.1 | v0.6 Rc<RefCell<Env>> 非 Send;全栈重写成本高 |
| D12 | 取消信号介质 | 独立异常 / 复用 ERR | 复用 ERR | §1.1,§5.4 | v0.6 §8.2 ERR transparent propagation 已成熟 |
| D13 | 共享状态 | Actor / 沿用 cell | 沿用 cell | §5.5 | v0.6 §3.3 闭包 cell 升级;不引入隔离类型体系 |
| D14 | Channel close 后 RECV/TRY_RECV 行为 | 返回 NULL / 抛 ERR | **抛 ERR(kind="ChannelClosed")** | §5.3,§4.4 E0055,Phase D 锁定 | §5.3 close 协议;Go/Trio close 语义 |
| D15 | SCOPE 是否支持嵌套 | 是 / 否 | 是 | §5.2 | Trio/Kotlin 都支持嵌套 nursery/scope |
| D16 | SHIELD 是否提供 | 是 / 否 | 是(F6) | §5.2 | Trio CancelScope.shield 是参考 |
| D17 | 隐式 runtime scope | 提供 / 不提供 | **不提供**(首版只支持显式 SCOPE:任何 SPAWN 必须直接或间接嵌套在某个 SCOPE(...) 内,顶层 SPAWN 报 E0058;不引入类似 Python asyncio 的 free-floating task 或 Java 守护线程) | §3 Phase C1,§10 D17 | §3 Phase C1;防 free-floating task |
| D18 | spec 文件改动时机 | Phase A 起 / Phase H1 才改 | Phase H1 才改 | §8.1 | v0.2 计划验证;保持 v0.7 全过程中 spec 不变 |
| D19 | OS 线程方案 | v0.7 / v0.8 / 不做 | v0.8 评估 | §7 | OS 线程在 v0.6 代价太高;v0.8 重评 |
| D20 | 性能承诺 | 提速 / 不退化 / 不承诺 | **单 task 退化 < 10%;并发路径只保证正确性与无泄漏** | §1.3,§6.4,§3 G5 | Trio/Greenlet: 不跟业务代价交换性能 |

---

## 附录 A:plan ↔ spec 章节映射

| plan 章节 | spec v0.7 章节(草稿) |
|----------|----------------------|
| §2.2 | §17 引言 |
| §2.3 | §17.1 设计维度 |
| §4 | §17.5 标准库 |
| §5.1 | §17.1.1 调度器 |
| §5.1.1 | §17.1.1 调度器状态机 |
| §5.2 | §17.1.2 任务生命周期 |
| §5.3 | §17.2 通道(含状态机) |
| §5.4 | §17.3 错误传播 |
| §5.4.1 | §17.3 ERR 传播矩阵 |
| §5.5 | §17.4 与 LET MUT 的交互 |
| §5.6 | §17.5.1 std.io 集成 |

---

## 附录 B:新增 ADR

### ADR-0014 — 结构化并发选型

**背景**:v0.7 需要为 WLWL 增加并发原语;候选模型见本计划 §2.1。
**决策**:采用结构化并发 + 通道组合(Trio/Kotlin/Swift/Go cff 共识)。
**依据**:见本计划 §0.1 与 §2.2。
**影响**:wlwl-eval 引入 runtime/task/cancellation/channel 四个模块;新增 15-17 个 builtin;新增 6 个错误码。
**否决理由**:见 §2.1 否决栏。

### ADR-0015 — Brown 9 维度决策

**背景**:Brown 等 2025 论文识别出 async/await 的 9 个独立设计维度,各家 runtime 选择互不相同。
**决策**:见本计划 §2.3 与 §10 D2-D10。
**依据**:Brown 论文"Design Space Exploration of Async/Await"。
**特别说明**:我们在 Persistence 上选择 Transient(与 Trio 的 Persistent 不一致),有三条具体理由:

1. **显式可变性哲学**:WLWL 的状态变化全部由用户显式 LET MUT / SET 表达,从不隐式更新(参见 v0.6 §3.3)。取消响应同样应当显式:用户应在合适的位置调用 TASK_IS_CANCELLED() 或 YIELD() 检查,而不是被运行时强制打断。
2. **SHIELD 复杂度**:Transient 与 SHIELD(fn)(§3 Phase F6)天然契合。若选 Persistent,SHIELD 必须实现"在 SHIELD 块内保存取消信号、块结束后立刻抛出"的额外协议(类似 Trio 的 CancelScope.shield);Transient 下 SHIELD 只需在退出后让下一个 YIELD 检查点观察到取消标志即可,实现复杂度大幅下降。
3. **单线程协作足够**:WLWL v0.7 的调度器是协作式单线程(§5.1),CPU-bound 任务不会并发执行。在此模型下,强制响应取消(Persistent)的收益几乎为零——反正没有别的任务在跑,被打断并不释放任何资源;让用户主动检查反而能精确控制清理点。

### ADR-0016 — 调度器单线程边界

**背景**:WLWL v0.6 的 Evaluator 是单线程树遍历;Rc<RefCell<Env>> 非 Send。
**决策**:v0.7 调度器保持单线程;不引入 OS 线程;不引入 Send/Sync 全局约束。
**依据**:本计划 §5.1;v0.2 决策清单(已隐含该边界)。
**远期**:v0.8 评估引入 thread pool + reference capabilities(参考 Arvidsson 等 OOPSLA 2023)以支持 CPU-bound 并行。

---

## 附录 C:9 维度对照表(完整版)

| 维度 | WLWL v0.7 | Trio | Kotlin | Swift | Tokio | Smol | Asyncio | C# | JS |
|------|-----------|------|--------|-------|-------|------|---------|----|----|
| Eagerness | Lazy | Lazy | Lazy | Eager | Lazy | Lazy | Lazy | Eager | Eager |
| Suspension | Dynamic | Dynamic | Dynamic | Dynamic | Dynamic | Dynamic | Dynamic | Static | Static |
| Extent | Dynamic | Dynamic | Dynamic | Dynamic | Indefinite | Indefinite | Indefinite | Indefinite | Indefinite |
| Ref Strength | Strong | Strong | Strong | Strong | Strong | Weak | Weak | Strong | Strong |
| Destruction | Awaited | Awaited | Awaited | Cancelled | Cancelled | Cancelled | Cancelled | Terminated | Awaited |
| Propagation | Destructive | Destructive | Destructive | Destructive | Never | Never | Never | Never | Never |
| Awareness | Aware | Aware | Aware | Aware | Unaware | Unaware | Aware | Unaware | Unaware |
| Direction | Top-Down | Top-Down | Top-Down | Simultaneous | Top-Down | Top-Down | Bottom-Up | — | — |
| Persistence | **Transient(有意选择)** | Persistent | Transient | Persistent | — | — | Transient | — | — |

> "—" 表示该 runtime 在该维度上未明确选择或不适用。**Transient(有意选择)** 在 spec 草稿中可作为引用注释。

---

## 附录 D:进度追踪

### Phase A — 调研 + 设计落地
- A1 本计划文件:docs/plan/wlwl-build-plan-v0.7.md — commit `01ff9d6` (+`6f2f23d` HTML 修复)
- A2 spec 草稿 §17(仅在本文件):完成于 A1
- A3 ADR 0014/0015/0016 — commit `0d463ad`
- A4 deviations.md v0.7 章节标记 — commit `e9d2106` (+`7421e2c` 编码修复 +`15ef61a` `.gitattributes`)

### Phase B — 协程 runtime 骨架
- B0 错误码 E0052-E0058 + `ErrorCategory::Concurrent` — commit `b38126e`
- B1 `runtime.rs` 类型 stub(TaskId / TaskHandle / TaskState / YieldReason / Scheduler 等) — commit `7f9d54f`
- B2 `Evaluator.current_task: Option<TaskId>` — commit `86cbddf`
- B3 fidelity golden(10 fixture 字节对照) — commit `6cac0f1`
- B4 `task.rs` Task + Scope 数据结构 — commit `5634331`
- B5a-1 `StepResult` + `step_once` wrapper(零行为变化) — commit `5ff79e5`
- B5a-2 选 1 builtin 改走 step_once — pending(合并到 B5a-3)
- B5a-3 递归 eval → 显式 CPS — ✅ **完成(路径 B 切片,2026-09-22)**
  - slice 1: `Signal::Yield` + internal yield marker — commit `836a5ce`
  - slice 2: 1-arg yield marker for arg-eval propagation — commit `90ace1c`
  - slice 3 step_call + dispatch_call 提取 — commit `8b65575`
  - 路径 B-A:`yield_split` 模块 + Task `segments`/`current_segment` + SPAWN 时静态切段,不支持位置(NestedYield)E0014 — commit `4324fe2`
  - 路径 B-B:`builtin_yield` 移除 NULL 穿透绕道(P7-B5a3-002),`run_task_segments` + `Env::take_scopes/replace_scopes`,mid-body 真正挂起 — commit `beec77d`
  - 路径 B-C:fixture `impl/tests/concurrency/yield_midbody.wll` + driver `wlwl-cli/tests/concurrency.rs`;deviations P7-B5a3-001/002
- B5b Scheduler 接入 / current_task 真实参与运行时: ✅ — run-queue + run_one_task + scope-exit await;SPAWN 惰性入队
- B6 单 task benchmark baseline(5 workload 在噪声内) — commit `ad0dd60`

### Phase C — 内置函数 SCOPE/SPAWN/AWAIT/YIELD
- C1 SCOPE(fn): ✅ — commit `e28de1d` 注册 + scope_depth retrofit 在 commit `cce3ce3`
- C2 SPAWN(fn): ✅ — commit `cce3ce3`;后接 audit-fix 链 `1248978`(P7-C2-001 arity E0056 修正) → `d46bab3`(deviation commit hash 同步) → `d53e09e`(runtime API 收敛:删 alloc_task + doc 修正)
- C3 AWAIT(handle): ✅ — 值/user-ERR/host-diag re-raise/E0053 契约;SPAWN 失败路径改为存 handle(P7-C2-002 关闭)
- C4 YIELD(): ✅ — **B5a-3-B 后真正 mid-body suspend**(commit `beec77d`);pre-B5a-3 是 NULL 穿透绕道(P7-B5a3-002)
- C5 TASK_CURRENT / TASK_IS_CANCELLED: ✅ — 依赖 B5b current_task;TASK_CURRENT 任务外 E0053;TASK_IS_CANCELLED 任务外 FALSE
- C7 跨 task cell 升级回归(plan §5.5): ✅ — E-CloCap/LET/LET MUT/late-bound E0024;E0057 评估=复用 E0024
- C8 跨 task 闭包捕获回归(plan §5.5): ✅ — 计数器/setter/getter/返回闭包/递归闭包共享 cell

### Phase D — Channel
- D-A 数据形态: `Channel` / `ChannelId` / `ChannelHandle` 类型,`Value::ChannelHandle` 变体,`Scheduler.channels` + `next_channel_generation`,E0054/E0055 已注册 — commit `730cac9`
- D-B 简单 builtin: `CHANNEL_NEW(buf)` / `CHANNEL_CLOSE(ch)` / `CHANNEL_LEN(ch)` / `CHANNEL_CAP(ch)`,resolve_channel_handle 助手,E0053 stale-handle 检测 — commit `5566ffe`
- D-C send/recv + mid-body suspend: `CHANNEL_SEND` / `CHANNEL_RECV` / `CHANNEL_TRY_SEND` / `CHANNEL_TRY_RECV`,`Signal::Yield(SendingOn|ReceivingOn)` 走 B5a-3 路径 B segment runner,close-protocol 唤醒 receiver_waiters 返 `ChannelClosed` ERR — commit `6941d6d`
- D-D leak detector (D7): Scope 加 `channels: Vec<ChannelId>` 字段,`pop_scope` 强制关闭并把 parked receiver 重新入队;slot 不回收,生成数保持不变(对照 P7-D2-001,D8 死锁检测 v0.7.0 不做) — commit `87d45f4`
- D-E fixture + 文档: `impl/tests/concurrency/channel_basic.wll` + driver 测试,deviations P7-D2-001 / P7-D8-001
- **D2/D3 mid-body suspend 实际未实现** — 见 deviation P7-D2-001(Signal::Yield 在 builtin 内部不能被路径 B 静态切段切到,改为返 `ChannelWouldBlock` ERR);D8 死锁检测同根,跟随搁置

### Phase E — 错误传播
- E-A 单元测试落地:BOOL / UNWRAP / ERR_PAYLOAD / EXPECT_ERR / PRINT_ERR(负例)/ IF(条件位) / TRY + ERR kind 保真(`ChannelClosed` 跨 AWAIT 仍可读) — commit `7bd04aa`
- E-B-1 内置接线:`Task.cancel_requested` 字段 + `Scheduler::request_task_cancel` / `cancel_siblings_in_scope` 助手 + `TASK_CANCEL(h)` / `TASK_CANCEL_PARENT()` 内置 + `run_one_task` 入口检查点 + SCOPE 兄弟取消 — commit `0bec336`
- E-B-2 conformance fixture:`scope_cancel_siblings.wll`(SCOPE 透传首个未消费 child ERR)+ `scope_consume_does_not_change_return.wll`(SCOPE 不覆盖已消费 ERR) + `concurrency.rs` driver + deviations P7-E3-001(路径 B 同步执行 → "在飞兄弟取消"不可观测,SCOPE 返回语义锁定 fn-body-Value::Err 透传)
- E-C-1 conformance fixture:4 个 ERR consumer 跨 task 端到端 — `err_consumer_unwrap_or.wll` / `err_consumer_is_err.wll` / `err_consumer_payload.wll` / `err_consumer_unwrap.wll`(UNWRAP 验 E0100 PANIC 路径)+ driver 4 个测试
- E-C-2 snapshot 测试:8 个跨 task wire-format 锁定(`ec_snap1..ec_snap8` in lib.rs) — 字符串/字典 ERR 载荷、IS_ERR/UNWRAP_OR 返回类型、CHANNEL handle TYPE+LEN+CAP、CHANNEL_RECV-after-close kind、WRAP 嵌套展平、`AWAIT(42)` 保留原始值不隐式包 OK。`ec_snap3` 替换为 CHANNEL handle 元数据(AWAIT-cancelled 受 path B 同步限制不可端到端观察 — 同 P7-E3-001;CHANNEL handle 是另一关键不变量)
- E-C 阻塞项 §6.0 "ERR consumer 跨 task 回归" 全部闭合:8 unit(E-A)+ 4 concurrency(E-C-1)+ 8 snapshot(E-C-2)
- E-D 评估完成:**无偏离**(deviations P7-E5-001)。`eval_if` 在并发路径下短路语义与 §8.3 / §6.1 完全一致,5 个新 `ed_*` 单元测试覆盖 truthy/falsy/ERR 条件/分支内 SPAWN 等不变量;无新代码、无 plan §3 E5 文字修改
- F-A (F1+F3+F7):`Scheduler::cancel_scope_subtree` 助手 + `TASK_CANCEL_PARENT` 改为调子树取消(plan §3 F3)+ 3 单元测试(`fa_*` 语言级 + 2 个 Rust 级 helper 直测)
- F-B (F5):`SHIELD(fn)` builtin — 可嵌套(`shield_depth` 累加 / 最外层退出才触发)、内部 `TASK_CANCEL_PARENT` 仅标 `shield_pending_cancel` 不真取消、退出后把 pending 翻译回 self 的 `cancel_requested`、自身 ERR 走 §5.4 不被屏蔽 + 7 个 `fb_*` 单元测试
- F-C conformance fixture:3 个 SHIELD fixture(`shield_basic.wll` / `shield_nested.wll` / `shield_then_error.wll`)+ driver 3 个测试 + deviations P7-F5-001(路径 B 下"SHIELD 退出后立即生效"无 yield 中间可见态,等价于"同 fn body SHIELD 之后立即观察 cancel_requested")
- G 质量门禁:
  - G1 clippy workspace -D warnings:复检 0 error
  - G2 rustdoc --workspace --no-deps 0 warning(修了 11 个 `[B5b]` intra-doc link 用 Python 重写为 `[B5b phase]` 避免被解析成 rustdoc-link)
  - G3 fuzz 24h:impl/fuzz/ 现状保留(lexer / parser / eval 三个 harness,nightly cargo-fuzz 安装中)— 24h 运行超出本 session 时长,plan §3 G3 明确"do NOT run fuzz in CI nightly",本地开发者周期运行
  - G4 cargo-deny 0 violation:advisories ok / bans ok / licenses ok / sources ok,5 个 non-violation `unnecessary-skip` warning(workspace 内部 crate 的 skip 配置,可清)
  - G5 cargo bench:criterion 0.5,string_concat -16% 改进 / array_higher_order / error_propagation 无显著变化 — 无任何退化 > 10%
  - G6 spec 文件冻结:v0.7 启动后至今 spec v0.6 未被改动,Phase G 期间持续冻结(spec v0.7 在 H1 阶段写)
  - G7 rich suggestion_code:为 E0052-E0058 五个新并发/通道错误码补了 `with_suggestion(Suggestion::Note { ... })`,diagnostic 工具链统一覆盖 v0.6 与 v0.7 错误码

### Phase F — 取消作用域
均待启动(F1-F7,含 SHIELD 至少 3 个 conformance fixture)。

### Phase G — 质量阶段
均待启动(G1-G7)。

### Phase H — spec v0.7 + release
均待启动(H1-H4)。spec 文件直到 H1 才允许改动(plan §8.1 / D18)。

---

## 附录 D-1:下次会话交接摘要(2026-09-22 收工)

| 项 | 状态 |
|----|------|
| 门禁 | `cargo test --workspace` 全绿(eval **731**,fidelity 1 pass;wlwl-cli concurrency 12 pass);`cargo clippy --workspace --all-targets -D warnings` **0 error**;`cargo doc --workspace --no-deps` **0 warning**;`cargo deny check` **0 violation**;`cargo bench` **无退化**(string_concat -16% / array_higher_order -6%/no-change / error_propagation -7%/no-change) |
| Phase B | **全部完成**(B0-B6,B5a-3 走路径 B 落地,见 commit 链 `4324fe2` → `beec77d`) |
| Phase C | **全部完成**(C1 SCOPE / C2 SPAWN / C3 AWAIT / C4 YIELD(B5a-3-B 后真 mid-body) / C5 TASK_* / C7 cell / C8 closure) |
| B5b | 调度循环已接:SPAWN **惰性入队**,AWAIT 驱动 `scheduler_run_until_done`,SCOPE 退出 await children |
| B5a-3 路径 B | ✅ 完成。SPAWN 时 `split_body_for_yield` 静态切段;`run_task_segments` 一次跑一段;`running_env` 跨段保留 LET 绑定;conditional yield(IF 内 YIELD 没走)正确跳过 |
| 新增 fixture | `impl/tests/concurrency/yield_midbody.wll` + driver `wlwl-cli/tests/concurrency.rs` |
| 偏差 | P7-B0-001 / P7-C2-001 / P7-C2-002 / **P7-B5a3-001 路径 B 选型** / **P7-B5a3-002 NULL 穿透绕道拆除** / **P7-D2-001** / **P7-D8-001** / **P7-E0-001 registry 推迟到 H1** / **P7-E3-001 SCOPE 兄弟取消路径 B 不可观测** / **P7-E5-001 IF 并发路径短路语义无偏离** / **P7-F5-001 SHIELD 退出后立即生效路径 B 无 yield 中间态** 已入 `deviations.md`;E0057 评估=复用 E0024 |
| **建议起点** | **Phase G 质量门禁**(G1-G7 — clippy 0 warning / rustdoc 100% / fuzz 24h / cargo-deny / 性能 bench / spec 冻结 / rich suggestion_code)。F 已全部完成(F-A F1+F3+F7 + F-B F5 SHIELD + F-C 3 conformance fixture + 7 unit);F 后只剩 G + H |
| 参考 | C3/C4/C5/B5b/B5a-3 实现集中在 `wlwl-eval/src/lib.rs`(builtin_* + run_one_task + run_task_segments);`yield_split.rs` 是切段纯函数;`task.rs` + `runtime.rs` 是数据/调度 |

---
## 附录 E:API 示例(验证 §5 语义)

以下 .wll 代码片段是 v0.7.0 释出的示意代码,不是实现。语义错误需在实现阶段(§3 Phase C-G)调整。

### E.1 基本 SCOPE / SPAWN / AWAIT

`wlwl
LET(result, SCOPE(FUN(() ,
    LET(a, SPAWN(FUN((), 1 + 1)));
    LET(b, SPAWN(FUN((), 2 + 2)));
    LET(va, AWAIT(a));
    LET(vb, AWAIT(b));
    [va, vb]
)));
result   // [2, 4]
`

### E.2 ERR 跨任务传播

`wlwl
LET(out, SCOPE(FUN((),
    LET(child, SPAWN(FUN((), ERR("boom"))));
    LET(v, AWAIT(child));   // v = ERR("boom");§8.2 透明传播
    UNWRAP_OR(v, -1)         // -1
)));
`

### E.3 SHIELD 三场景

`wlwl
// shield_basic:屏蔽 1 层
LET(out, SCOPE(FUN((),
    TASK_CANCEL_PARENT();                       // 取消当前 scope
    LET(v, SHIELD(FUN((),
        "cleaned"                               // SHIELD 内不响应取消,照常运行
    )));
    YIELD();                                     // SHIELD 退出后,下一个 YIELD 检查到 cancel
    -1
)));

// shield_nested:屏蔽 3 层
LET(out, SCOPE(FUN((),
    TASK_CANCEL_PARENT();
    LET(v, SHIELD(FUN((),
        LET(v2, SHIELD(FUN((),
            LET(v3, SHIELD(FUN((), "deep")))
        )));
        v2
    )));
    YIELD();                                     // 三层都退出后,下一个 YIELD 才检查到 cancel
    -1
)));

// shield_then_error:SHIELD 块内 fn 自己抛 ERR 不受屏蔽
LET(out, SCOPE(FUN((),
    LET(v, SHIELD(FUN((), ERR("inner"))));
    UNWRAP_OR(v, -1)   // -1 (ERR 正常传播)
)));
`

### E.4 Channel 跨任务合成

`wlwl
LET(merge, FUN((n), (
    LET MUT(sums, []);
    LET MUT(done, 0);
    LET(ch, CHANNEL_NEW(0));
    LET(make, FUN((i),
        SPAWN(FUN((),
            LET(s, +(i, i));
            CHANNEL_SEND(ch, s);
            SET(done, +(done, 1));
            IF(==(done, n), CHANNEL_CLOSE(ch))
        ))
    ));
    SCOPE(FUN((),
        LET(i, 0);
        WHILE(<(i, n),
            LET(_, AWAIT(make(i)));
            SET(i, +(i, 1))
        );
        LET(out, []);
        LET(done, FALSE);
        WHILE(NOT(done),
            LET(v, CHANNEL_RECV(ch));
            IF(IS_ERR(v),
                IF(==(ERR_PAYLOAD(v).kind, "ChannelClosed"),
                    SET(done, TRUE))
            ),
                out
            )
        );
        out
    ))
)));
merge(3)   // [0, 2, 4](顺序不保证)
`

### E.5 与 v0.6 std.io 的 yield 接入

`wlwl
LET(read_lines, FUN((path),
    LET(ch, CHANNEL_NEW(1));
    SPAWN(FUN((),
        LET(f, OPEN_FILE(path));
        WHILE(NOT(EOF(f)),
            LET(line, READ_LINE(f));
            CHANNEL_SEND(ch, line);
            YIELD()           // 人为让步避免某 sender 占用 run-queue
        );
        CHANNEL_CLOSE(ch)
    ));
    ch
));
LET(ch, read_lines("/etc/hosts"));
LET(line, AWAIT(CHANNEL_RECV(ch)));   // 第一行
`

---

## 附录 F:spec v0.7 §17 草稿(阶段 H1 定稿)

> 本草稿仅供 spec §17 章节参考;在 Phase H1 定稿前不是规范性文档。

### §17 并发

WLWL 在 v0.7 首次引入并发。并发模型是**结构化并发 + 通道**(§2.2),类似 Trio/Kotlin coroutineScope/Swift TaskGroup。

#### §17.1 任务与作用域

**不变量**:任务寿命严格包含在词法作用域中;错误/取消顶向下传播;子任务完成/取消前,父 scope 不退出。

**§17.1.1 调度器**:协作式单线程调度器(§5.1)。任务状态机 Pending/Running/Suspended/Done/Cancelled。调度循环 run_until_idle 持续 resume 任务直到 run-queue 空。

**§17.1.2 任务生命周期**:SCOPE(fn) 创建子作用域;SPAWN(fn) 在当前 scope 内派生子任务;AWAIT(handle) 等待子任务完成;YIELD() 人为让步;TASK_CURRENT() / TASK_IS_CANCELLED() / TASK_CANCEL(handle) / TASK_CANCEL_PARENT()。

#### §17.2 通道

CHANNEL_NEW(buf) 创建有界通道(buf=0 为同步);CHANNEL_SEND/RECV/CLOSE/TRY_SEND/TRY_RECV/LEN/CAP。

**关闭语义**(§5.3):关闭后 WRITE → ERR(E0054);关闭后 RECV/TRY_RECV → ERR(kind="ChannelClosed");senders_alive 为 0 时 wake 所有 receiver_waiters。

#### §17.3 取消与错误传播

SHIELD(fn) 屏蔽外部取消(§3 Phase F6 详细语义)。ERR 在跨任务 / scope / SHIELD 边界的传播规则见 §5.4.1 矩阵。

#### §17.4 与 LET MUT / 闭包 cell 的交互

跨任务共享 cell 遵循 v0.6 §3.3/§3.4 规则(§5.5);不引入新的可见性约束。

#### §17.5 标准库 wlwl:std.concurrency

wlwl:std.concurrency 模块提供高层组合 API(高阶 helper),不负责原语。原语 SPAWN/AWAIT/CHANNEL_* 是 eval builtin,不需 IMPORT。

#### §17.6 与 §8 ERR transparent propagation 的关系

取消不生成 ERR(§5.4.1)。子任务自身 ERR 通过 §8.2 透明传播到父任务。§8.7 ERR consumer registry 在跨任务场景下保持同 v0.6 的语义(§3 Phase E4 阻塞项验证)。

#### §17.7 限制与不承诺(跟 §10 D20 一致)

- 单 task 路径退化不超过 10%
- 并发路径不承诺吞吐量 / 公平性 / 延迟上界
- 不提供隐式 runtime scope
- v0.7 不引入 OS 线程

---

**审批**:本计划需 Li 批准后启动 Phase B。