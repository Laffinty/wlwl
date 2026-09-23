# WLWL v0.9 构建计划

> **状态**:草稿 4(2026-09-24 第三方三次审计后修订,**FINAL 待用户批准**)
>   - ✅ OOP 路径:**α 真实现 + 行为类型 + 先锋增强**(Memory 2026-09-24 已固化)
>   - ✅ 草稿 2 审计 7 项 + 草稿 3 二次审计 5 项 + 草稿 4 三次审计 5 项已采纳
>   - ✅ 草稿 4 修复:§0.4 重复行 / §11.1 锁测试数 1441→1465 / §11.2 错误码算术 61+6 / §11.2 新增 W0065/W0066 警告码行 / §3.5 §17.9 → §17.8 新增小节(显式标注)
>   - ✅ **所有决策点已确定,3 处硬伤已修,2 处遗漏已补**
>   - ⏳ 3 处小问题(负 buf 错误码 / E0065 退出码 / 关闭唤醒顺序)留 Step 11/12 spec 派生时定
> **基线**:`docs/standard/wlwl-spec-v0.8.md`(v0.8.1 patch 无 spec 改动)+ `impl/crates/wlwl-eval/src/{runtime,task,channel,yield_split}.rs`
> **不在范围**:编码规范(UTF-8 BOM / 换行 / 标识符 / 转义);§0.1 章节构成;外部 std 库的语义冻结边界(§17.6 维持现状)。
> **源材料**:
> - 仓库内:`docs/adr/0014-structured-concurrency-v0.7.md`、`0015-brown-9-dimension-decision.md`、`0016-scheduler-single-thread-boundary.md`、`docs/standard/wlwl-spec-v0.8.md §17`、`docs/history/deviations-v0.8.md` D8-009..D8-014。
> - 仓库外(本次综合研究):
>   - Gray, Krishnamurthi, Crichton. **A Design Space Exploration of Async/Await**. OOPSLA 2026 / PACMPL Vol. 10, DOI 10.1145/3839519,arXiv 2608.20677(Brown Cognitive Engineering Lab,2026-08)。
>   - Smith, N. J. *Notes on structured concurrency, or: Go statement considered harmful*(2018)。
>   - Sústrik, M. *Structured Concurrency*(libdill, 2016)。
>   - JEP 428 / 453 / 462 / 505 / 525 / 533(Project Loom / Structured Concurrency,OpenJDK)。
>   - Elizarov, R. *Structured Concurrency*(Kotlin 2018-2019)。
>   - Leijen, D. *Koka: Functional Programming with Algebraic Effects and Effect Handlers*(2014+;v3.x 2024-2026)。
>   - Sivaramakrishnan, K. *Multicore OCaml / OCaml 5 Effects*(2022,稳定于 2024)。
>   - Lindley, S. *Lightweight Functional Session Types*(Links,Edinburgh)。
>   - McCabe, F.; Lindley, S. *WebAssembly Stack Switching Proposal*(WasmFX,Phase 3,2026-05)。
>   - Hüttel et al. *Foundations of Session Types and Behavioural Contracts*(ACM Comp. Surv. 2016)。
>   - Armstrong, J. *Making reliable distributed systems in the presence of errors*(BEAM / Erlang,2003)。
>   - Crichton, W. *Async in depth*(Tokio 教程 2024-2026)。
>
> **立场**:**承接 v0.7 决策,不重排 ADR-0014/0015/0016**。本计划在这三份 ADR 之上,把 §17 从"静态切段 + WouldBlock 通道"升级为"可挂起协作调度",并对 §11.2 的保留错误码与 §12/附录 G 的 OOP 占位给出**实现或移除**决议。
>
> **核心论断**(详见 §2):v0.9 应以**并发语义收口**为唯一主题,而不是再做新语言特性。

---

## 0 摘要

### 0.1 主题论断

| 维度 | 命题 | 论据 |
|------|------|------|
| **P0 主线** | v0.9 = "**以并发语义收口为 P0 主线**(`从静态切段并发走向可挂起协作调度`),同时完成 OOP 先锋实现作为 P1" | §17.1 已承认 YIELD 位置限制是实现路径,不是语言语义;§17.7 的限制表(ChannelWouldBlock、嵌套 YIELD 不恢复嵌套体、缺死锁检测)阻碍结构化并发可用性;E0055/E0057 是规范债务;OOP 占位是真实现 vs 移除的顺手债 |
| **不做** | 不加新 std 模块实现;不改编码规范;不引入第三方 scheduler 框架;不重开 Brown 9 维度(ADR-0015 已锁);不做 OS 线程化(ADR-0016 边界) | v0.7 已确立结构化并发选型(ADR-0014);单线程边界不动(ADR-0016);9 维度已决议 |
| **P0 并发收口** | §17.1 YIELD 任意位置合法;§17.2 同步通道真挂起;§3.4 L1 死锁检测;§3.5 公平性;§3.6 E0055/E0057 决议;§3.4 进程内 algebraic-effect runtime 命名 | §17.1 / §17.2 / §17.7 / §11.2 已留扩展点 |
| **P1 OOP 真实现 + 先锋增强** | §4.3 CLASS/NEW/THIS 真实现 + 错误码启用;§4.4.1 algebraic-effect framing(spec + runtime 命名);§4.4.2 tag/payload cancellation;§4.4.3 session-typed methods(含 choice + μ 递归骨架);§4.4.4 WasmFX-style tag dispatch | 用户 2026-09-24 选定 α + 先锋 |
| **P2 AI / std 契约** | §5 仅做最小正式契约 | `wlwl:std.ai` / `wlwl:std.agent` 协议细节上提到 spec |
| **可观测收益** | YIELD 任意表达式位置;同步通道 `buf=0` 真挂起;嵌套 YIELD 恢复;OOP 真可用 + 行为类型 + 线性 THIS;algebraic-effect 内部命名 | §3 / §4 全部落地 |
| **风险等级** | 中-高 | 真挂起重写 + 行为类型 + tag/payload + OOP 真结合,工作量 ~14-18 人·周;并发 conformance 重测;L1 死锁假阳性控制 |

### 0.2 与 v0.6→v0.7→v0.8 决策的关系

| 历史 ADR | 状态 | v0.9 是否改动 |
|---------|------|---------------|
| ADR-0014 *Structured concurrency + channels adopted* | Accepted 2026-09-21 | **不变** |
| ADR-0015 *Brown 9-dimension async/await design space resolved* | Accepted 2026-09-21 | **延伸**(§6 增补 1 项:挂起可达性;§7 改 Suspension 的工程含义) |
| ADR-0016 *v0.7 scheduler keeps single-threaded boundary* | Accepted 2026-09-21 | **不变** |
| ADR-0008..0013(无关并发) | Accepted | 不变 |

**v0.9 新增 ADR 计划**(草稿):

| 编号 | 主题 | 草案要点 |
|------|------|---------|
| **0017** | *Cooperative suspension-based scheduler* | 把"静态切段 + WouldBlock"升级为"真挂起 + 调度循环";给出 §17.7 五项限制的逐项决议路径 |
| **0018** | *§11.2 E0055 / E0057 retention decision* | 启用 / 降级 / 移除的决议路径与对应 dev path |
| **0019** | *OOP minimal implementation w/ behavioral types + algebraic-effect runtime* | §4 真实现路径 α(用户 2026-09-24 选定)+ 草稿 2 四项先锋增强(algebraic-effect framing spec+impl runtime、tag/payload cancellation、session-typed methods 含 choice+μ、behavioral class types + 真线性 THIS);**§13-§16 章节号冻结规则** |

### 0.3 三段式目标

| 阶段 | 范围 | 文档位置 |
|------|------|---------|
| **A · 并发运行时重写** | `yield_split.rs` 切段 → 真实挂起;`channel.rs` 同步通道可挂起;`runtime.rs` 调度循环支持 `Suspended(YieldReason::*)` 阻塞态 | §3 |
| **B · 文档 / 注册表 / D9-NNN 偏差登记** | §17.1 / §17.2 / §17.7 / §11.2 修订;新增 ADR-0017/0018;附录 G 锚点回归 | §5 / §6 |
| **C · 验收** | v07_fidelity baseline 重派生;并发 conformance 重写;单 task 性能回归 < 10% 不变 | §10 |

A 与 B **相互依赖**:B 多数条目依赖 A 完成后源码已变;但 B 是可独立 review 的文档改动。

### 0.4 v0.8.1 baseline(本计划不重写)

- spec v0.8(1265 行),v0.8.1 patch 无 spec 改动;
- impl/crates/wlwl-eval BUILTIN_REGISTRY 110 entries;`b11_*` lock 测试守住与附录 G 一致;
- `runtime.rs` 718 行 / `channel.rs` 342 行 / `task.rs` 314 行 / `yield_split.rs` 492 行;
- v07_fidelity baseline(closure_cell 等 1 条锁定),其余 v0.6 fixtures 走 `wlwl-cli/tests/conformance.rs`;
- §17.7 限制表 7 项 / §11.2 错误码表 E0055、E0057 标"保留 / 无触发路径";
- v0.8.1 锁测试总计数 1409 passed,workspace 0 failed。

v0.9 任何 commit **不得降低上述指标**。

### 0.5 先锋技术路线(用户 2026-09-24 选定 + Memory 固化)

本计划在与"工业共识"并存的环节,**默认高亮**学术前沿(alphabetically by source):

| 设计点 | 选型 | 学术依据 | 工业对照(传统) |
|--------|------|---------|---------------|
| **§17 文档化模型** | Algebraic-effect framing(`YIELD` = `perform Yield`,`TASK_CANCEL` = `raise Cancelled`) | Koka / OCaml 5 effects;Pretnar & Bauer 2015 | 文档只说"协作让步",无效果处理器术语 |
| **§17.3 取消协议** | Tag/payload cancellation(`TASK_CANCEL(task, reason_dict)`;reason 透传到 `Cancelled.payload.reason`) | WasmFX Phase 3;Swift `Task.CancellationError` payload 扩展方向 | Swift/Trio/Kotlin 仅 bool / 无 payload |
| **§4 CLASS 第二参数** | Session-typed 协议表达式(`{init: end, get: ?int.end, inc: end, get: ?int.end, close: end}`) | Honda, Yoshida, Carbone 2008;Lindley & Morris(Links) | Java/Kotlin/Swift/Python 均无对象方法协议静态保证 |
| **§4 THIS** | 线性 capability(越界返 E0032) | Wadler 1990 linear types;Roc / Austral | 主流 OOP 均无 THIS 线性约束 |
| **runtime 内部命名** | WasmFX-style `Suspended { tag, payload }`(`tag: Yield \| ChannelOp \| Cancelled`) | WasmFX Phase 3 提案 § Explainer | 主流 runtime 内部命名按自家风格 |
| **取消传播形式化** | 文档化 §17 与 §4 行为类型的 Curry-Howard 对应(channel 协议 = 线性逻辑命题) | Caires & Pfenning 2010 | 无形式化映射 |

**为什么这是 v0.9 的关键赌注**:wlwl v0.7 / v0.8 已经通过 Brown 9 维度 + ADR-0015 锁定了工业共识子集(与 Kotlin / Swift / Trio 一致)。**v0.9 的差异化方向**在于把会话类型 + 效果处理器 + 线性类型三个学术前沿融进 spec —— 这不是"加 feature",是给 wlwl 一个**学术身份**。后续 v0.10+ 若加 Wasm 后端,WasmFX 内部命名已经对齐,迁移成本极低。

**保守路径与之并存的可能性**:用户 2026-09-24 已选先锋路径;若任一子项(行为类型 / 线性 THIS / algebraic-effect 文档化)在 Step 9 / 12 落地时引入 ≥ 2 项 spec 争议,该子项回退到保守写法(具体回退路径见 §10 风险表)。

---

## 1 现状盘点(证据基线)

### 1.1 关键源码位置

| 关注点 | 文件 / 行 | 摘要 |
|--------|----------|------|
| YIELD 切段器 | `impl/crates/wlwl-eval/src/yield_split.rs` | 492 行;静态 AST 切段,validate + collect 两遍走;E0014 在非 Block 直接子项位置触发 |
| 通道 + WouldBlock | `impl/crates/wlwl-eval/src/channel.rs:42-58` | `TryResult<T>::WouldBlock` / `Closed` 枚举,SEND/RECV 不挂起时翻译 |
| 运行时调度循环 | `impl/crates/wlwl-eval/src/runtime.rs` | `Scheduler` + `TaskState::{Pending,Running,Suspended(reason),Done,Cancelled}` |
| TASK / SCOPE / SHIELD 内建 | `impl/crates/wlwl-eval/src/registry.rs` | `BuiltinGroup::Concurrent` 17 项;§17.1 / §17.3 |
| 取消协议 | spec §17.3、§17.7 行 988 | Transient + Top-Down + Aware(ADR-0015);"不产生 ERR",AWAIT 已取消任务返回 `ERR(kind="Cancelled")` |
| 错误码保留位 | spec §11.2 行 ~745 | `E0055` "保留" / `E0057` "保留",deviation D8-003 解释 |
| OOP 占位 | spec 附录 G 行 1223-1233 | `CLASS` / `NEW` / `THIS` / `GET_PROP` / `SET_PROP` / `CALL_METHOD` 标记 `OOP 未实现` |
| 嵌套 YIELD 限制 | spec §17.7 行 987 | 嵌套 `WHILE`/`FOR`/`IF` 内 YIELD 不恢复嵌套体剩余,继续外层 Block 下一段 |
| 同步通道 buf=0 | spec §17.2 / §17.7 | 满/空返回 `ERR(kind="ChannelWouldBlock")`,需 `TRY_*` 或预置缓冲 |

### 1.2 §17.7 限制表(逐项决议路径)

| 项 | v0.8.1 限制文本 | v0.9 决议(草) |
|----|----------------|---------------|
| 线程模型 | 单线程协作调度 | **保留** — ADR-0016 边界不动 |
| 性能 | 单 task 退化 < 10%;并发路径只保证正确性 + 无泄漏 | **保留** |
| 公平性 | 不承诺 | **保留** — 注释加"v0.9 起尝试 best-effort FIFO" |
| 延迟 / 吞吐 | 不承诺 | **保留** |
| **阻塞挂起** | `CHANNEL_SEND`/`RECV` 满/空不挂起,返 `ChannelWouldBlock` | **改** — 改为真挂起;`TRY_*` 保持非阻塞 |
| **死锁检测** | `[v0.7.0 不做]` | **决策点** — 最小 L1(同 scope 同句柄循环) |
| **嵌套 YIELD** | 嵌套构造内 YIELD 不恢复嵌套体剩余 | **改** — 改为恢复;切段器被取代 |
| 尾部 YIELD | 任务体末尾 YIELD 完成 NULL | **保留** — 真挂起后语义不变 |
| 在飞兄弟取消 | 同步模型下可能不可观测 | **保留 / 改善** — 改写为"v0.9 起保证在下一个 YIELD 或挂起点可观测" |
| 隐式 scope | 不提供 | **保留** — ADR-0014 决策 |

### 1.3 §17.1 YIELD 位置(已承认是实现路径)

```wlwl
LET(x, YIELD());   // v0.7 / v0.8: E0014
IF(c, YIELD(), 0); // v0.7 / v0.8: E0014
[1, 2, YIELD()];   // v0.7 / v0.8: E0014
LET(y, YIELD); y();// v0.7 / v0.8: 静默运行(间接调用)
```

spec §17.1 行 879:**"该限制是 v0.7 / v0.8 实现路径的产物,不是语言语义规则;未来版本(挂起式调度)可放宽而不视为破坏性修订"** — 即 v0.9 是显式允许放宽的"未来版本"。

### 1.4 Brown 9 维度的 v0.7 决策(ADR-0015 表)与外部研究冲突检查

| 维度 | v0.7 选 | Gray 等 OOPSLA 2026 对该维度的描述 | 7 个 runtime 选 |
|------|-------|-----------------------------------|----------------|
| Eagerness | Lazy | 一致 | Lazy:Python,Rust;Eager:C#,JavaScript |
| Suspension | Dynamic | 一致 | Static:JavaScript;Dynamic:C#,Swift,Tokio,Smol,Asyncio,Trio |
| Extent | Dynamic | 与结构化并发不变量耦合 | Indefinite:JS,C#,Tokio,Smol,Asyncio;Dynamic:Swift,Trio |
| Ref Strength | Strong | Trio/Swift 选 Weak 时允许退出 scope 后被回收 | Strong:JS,C#,Tokio;Weak:Asyncio,Smol |
| Destruction | Awaited | Kotlin/Swift 选 Cancelled 时为节省资源 | Awaited:JS,Trio;Cancelled:Swift,Tokio,Smol,Asyncio;Terminated:C# |
| Propagation | Destructive | Java/Kotlin 风格 | Destructive:Trio;Never:其余六个 |
| Awareness | Aware | Rust 是唯一 Unaware 主流 | Aware:Asyncio,Trio,Swift |
| Direction | Top-Down | Kotlin/Swift 自动 vs. Tokio 显式 | Top-Down:Rust;Bottom-Up:Asyncio,Trio;Simultaneous:Swift |
| Persistence | Transient | Swift 选 Persistent(取消一旦发出不可忽略) | Transient:Asyncio;Persistent:Trio,Swift |

**v0.9 结论**:v0.7 选定的 9 维度全部与 Gray 等论文对维度的分类一致,无须重新决策。**仅在 Suspension 一项的工程实现层面,把"动态调度但静态切段"升级为"动态调度且真挂起"** — 维度定义未变,只是技术路径从 B(切段)换到 C(挂起)。

---

## 2 主题论证(外部研究综合)

### 2.1 为什么 v0.9 必须做并发语义收口

**核心证据链**:

1. **spec §17.1 已自承限制是实现路径**(行 879)。这是 v0.9 还债的天然窗口。
2. **§17.7 限制表里的并发项最多、最硬**(1.2 节)。其中"阻塞挂起"与"嵌套 YIELD"两项直接决定 `buf=0` 同步通道与 `YIELD()` 在表达式位置的可用性 — 即"结构化并发 + 通道"是否真成立。
3. **E0055 / E0057 是规范债务**(§11.2 + D8-003)。"保留但无触发路径"在 v0.7 起步时合理,v0.9 须在"启用 / 移除"二选一。
4. **Gray, Krishnamurthi, Crichton OOPSLA 2026** 用 9 维空间 + 形式化核心演算证明:同一段 async 程序在 7 个 runtime 上输出 4 种结果;v0.7 选定的子空间是对的,但子空间内的"Suspension = Dynamic"在工程上不能仅靠"动态切段"实现 — 必须真有挂起点才能让所有 9 维空间内"动态语义"的承诺兑现。

### 2.2 挂起式调度的工程含义(三类实现的取舍)

外部研究综合出三类实现路径,**v0.9 走 C 类**:

| 类 | 代表 | 实现方式 | 性能 | 内存 | 取消协议 | 与 v0.7 baseline 适配 |
|----|------|---------|------|------|---------|---------------------|
| **A. 有栈协程**(stackful) | Go goroutine,BEAM process,Lua | 真实栈切段;M:N 调度;抢占式(BEAM reduction budget)或协作式(Go 1.14+) | 高(I/O bound 卓越;CPU bound 受调度器抖动影响) | 每任务 ~1-8 KB | 显式 cancel() / linked;长 NIF 用 dirty scheduler | **冲突**:v0.6 baseline 是 Rc<RefCell<...>>,非 Send;BEAM 模型有重写 Rc 升级到 Arc 的代价 |
| **B. 静态切段**(stackless,compile-time) | v0.7 / v0.8 wlwl(impl yield_split.rs) | 编译器/解释器在每个 YIELD / await 点切成状态块,块间切换 | 中(CPU bound 不阻塞;切段本身有开销) | 几乎为零(块共享 Rc<Env>) | YIELD 即切换点;切段内 YIELD 不存在(被吸收) | **当前路径**:有 E0014 限制;嵌套构造处理受限 |
| **C. 真挂起 + 调度循环**(stackless,runtime) | Kotlin coroutines,C# async,Rust async,Swift TaskGroup | 编译器生成状态机 + 调度器在挂起点切走任务;每个 await 是 potential suspension point | 高(零成本抽象;Tokio 2.x 工作窃取) | 每 Future ~100-500 字节(state machine + spilled locals) | 显式 Job / Task / handle 取消;Awaitable 对象 | **目标路径**:与 v0.6 Rc<RefCell<Env>> 兼容;只是状态机在挂起点切换而不是切段器静态切 |
| **D. 延续(continuation) + 效果处理器** | Koka,OCaml 5,Multicore OCaml,WebAssembly Stack Switching(WasmFX, Phase 3) | 一等延续 / 效果处理器;`resume` 0/1/N 次;多 shot 可表达非确定性 | 中-高(单 shot 零成本;多 shot 多需栈段切换) | 视实现而定 | effect handler 决定;可表达 backtracking / generator / async 全集 | **超长路径**:对 v0.7 状态机代价过大;仅作为 v0.10+ 远期目标(WasmFX 是 Phase 3,尚无浏览器生产) |

**v0.9 选 C** 的三条理由:

1. **与 v0.6 baseline 兼容**:C 类仍是 stackless,`Rc<RefCell<Env>>` 不动,只把"切段"换为"调度循环驱动状态机"。无 Rc→Arc 重写。
2. **单线程协作保持**:调度循环仍在一根 OS 线程上,ADR-0016 边界不动。
3. **取消语义干净**:TaskState::Suspended(reason) 直接被 C 类支持;cancel 信号到达后调度器把状态从 Suspended(YieldReason::ReceivingOn) 翻到 Cancelled,后续 READ 在挂起点返回 `ERR(kind="Cancelled")`。
4. **与 Brown 9 维空间 + Gray 等论文一致**:Dynamic Suspension 在 C 类下是真挂起,语义对所有 9 维空间位置都自洽。

### 2.3 工业对照表(2026-09 截面)

| 维度 | Kotlin coroutines | Swift TaskGroup | Java StructuredTaskScope (JEP 525 / 533) | Python asyncio.TaskGroup | wlwl v0.8 | wlwl v0.9 目标 |
|------|------------------|-----------------|----------------------------------------|--------------------------|----------|---------------|
| 启动模式 | Lazy | Eager | Eager | Lazy | Lazy | Lazy(不变) |
| 挂起粒度 | 真实 await / suspension | 真实 await | 真实 JEP 444 virtual thread 挂起点 | 真实 await | **静态切段** | **真挂起**(改) |
| 作用域 | coroutineScope { } | withTaskGroup { } | StructuredTaskScope.open() | async with TaskGroup() | SCOPE(fn) | SCOPE(fn)(不变) |
| 取消 | Job 树 + CooperativeCancellationException | CancellationError 经类型系统标注 | ShutdownOnFailure / ShutdownOnSuccess | CancelledError | TASK_CANCEL + Transient(ADR-0015) | 同 v0.8 |
| 阻塞挂起 | suspend 函数真挂起 | async 函数真挂起 | virtual thread 挂载 carrier | await 真挂起 | **返 WouldBlock** | **真挂起** |
| 死锁检测 | 自定义(无标准) | 自定义 | Long Schedule 监测 | RuntimeWarning(`Enable -W error`) | 不做 | **最小 L1** |
| 在浏览器 | Kotlin/Wasm(Phase 4 协议栈切换) | Apple 平台原生 | 仅 JVM | 不支持 | 不支持 | 不支持 |

### 2.4 形式化基础(可选 / 不在 P0)

- **会话类型**(Honda, Yoshida, Carbone 2008;Caires, Pfenning 线性逻辑对应):为通道提供静态保证,确保协议符合。**v0.9 不引入到通道**(通道走 §3.2 真挂起的载荷路径);**v0.9 引入到 OOP CLASS 方法签名**(§4.4.3)。v0.10+ 路线图:把会话类型扩展到通道 + 跨任务消息协议,需类型系统扩展。
- **结构化并发不变量**(Smith 2018 四不变量:树 / 父等子 / 错向上 / 取消向下):**已经满足**,v0.9 维持即可。
- **Brown 9 维空间**(Gray, Krishnamurthi, Crichton 2026):**v0.7 已逐项决议**,v0.9 维持。**新增维度(草)**:`可达性`(Reachable) — 任务挂起后是否能恢复,v0.9 把这条隐式属性显式化:挂起是可达的(改:`Suspended(reason) → Running` 必可达)。

### 2.5 v0.9 议程内 vs 议程外(2026-09-24 用户确认 α + 先锋后重整)

#### 议程内(用户确认)

| 事 | 范围 |
|----|------|
| 并发收口(§3) | P0:真挂起 + algebraic-effect runtime 命名 + 死锁 L1(排除 YieldReason::Explicit)+ tag/payload cancel + E0055/E0057 决议 |
| OOP 真实现 + 行为类型 + 先锋增强(§4) | P1 α 路径:CLASS/NEW/THIS 真实现;session-typed methods(含 choice + μ 骨架);线性 THIS(防止外部别名);algebraic-effect framing(spec + impl);WasmFX-style tag dispatch |
| `wlwl:std.ai` / `wlwl:std.agent` 最小正式契约(§5) | P2:E0080-E0094 触发条件 normative |

#### 议程外(预先否决)

| 事 | 否决理由 |
|----|---------|
| 加 `wlwl:std.collection` / `wlwl:std.format` 等新 std 模块**实现** | 历史偏差(deviations-v0.8)留有并发特性开关等 5 项 P2 候选,留 v0.9.1 单独立项 |
| 切到多线程(ADR-0016 边界外) | 用户明令 v0.7 不重写 Rc;v0.9 不越界 |
| 引入第三方 scheduler 框架(tokio / async-std) | 与 ADR-0016 单线程边界冲突;且没有"已实现且可被采纳"的零成本方案 |
| Brown 9 维空间二次决议 | Gray 等论文不提供"重选";v0.7 选定的子集是工业共识 |
| 完整线性类型系统(扩展到所有函数参数与返回值) | 仅 THIS 走线性 capability;v0.9 维持 v0.7 动态类型 + 显式可变性哲学 |
| 命名协议 + 类型推断(`typealias`) | 留 v0.9.1;v0.9.0 仅提供 μ 骨架,named-protocol 系统作为 v0.10+ 议题 |
| Wasm 后端 + WasmFX Phase 4 编译 | WasmFX 仍在 Phase 3,2026-09 仍未进浏览器生产;留 v0.10+ |

---

## 3 P0 · 并发语义收口

> 本章是 v0.9 唯一大型工作。所有子项要么直接取消 v0.7/v0.8 的限制,要么给出"实现 / 移除"决议。

### 3.1 §17.1 YIELD 位置限制取消 — 切段器改为真挂起

**目标**:YIELD 可在任意表达式位置合法;不需 Block 包。

**当前路径(B)**:
- `yield_split.rs:split_body_for_yield` 在 SPAWN 时把 body AST 切成 `Segments = Vec<Vec<Expr>>`;
- validate 阶段拒绝 YIELD 不在 Block 直接子项位置(E0014);
- 调度器在 segment 边界 resume。

**目标路径(C)**:
- 删除 `yield_split.rs` 的 validate + collect;
- `yield_split.rs` 退化为"task body 入口包装器",仅负责包装闭包为 `Task::Pending`,不再切段;
- 调度器(`runtime.rs`)驱动 task body 时,在 YIELD 调用点**直接挂起**:`TaskState::Suspended(YieldReason::Explicit)`,调度循环选下一个 runnable task;
- 调度循环返回挂起 task 时:`TaskState::Suspended(YieldReason::*) → Running`,直接重新调 `eval_expr` 进入 YIELD 之后的中间表示。

**具体改动**:

| 文件 | v0.8.1 状态 | v0.9 改动 |
|------|------------|----------|
| `yield_split.rs` | 492 行,validate + collect | 简化为 ~100 行,只生成 `TaskSpec { body_expr, env, suspended_at: Option<ExprId> }` |
| `runtime.rs::Scheduler::resume` | 从 segment 表恢复 | 改为 `TaskSpec.resume()`,无切段 |
| `runtime.rs::Scheduler::step` | 跳过 `yield_split` 直接走 `eval_expr` | 同上 |
| `eval_expr` 在 YIELD 调用处 | 当前是 v0.7 在 `yield_split` 已确认位置 | 改为真 `Signal::Yield(Explicit)`,调度器捕获 |
| 错误码 `E0014` | 触发点:嵌套 YIELD / 非 Block 直接子项 | **移除该触发路径**(改 `E0014` 注册文本:"RETURN / BREAK / CONTINUE 出现在非法位置") |

**测试增量**(草):

| 锁测试 | 触发场景 | 当前(v0.8.1) | 目标(v0.9) |
|--------|---------|-------------|-----------|
| `eval_yield_in_let_position` | `LET(x, YIELD()); 42` | E0014 | 42(挂起后恢复) |
| `eval_yield_in_if_branch` | `IF(c, YIELD(), 0)` | E0014 | 视 c 走分支 |
| `eval_yield_in_array_literal` | `[1, YIELD(), 3]` | E0014 | `[1, NULL, 3]` |
| `eval_nested_yield_in_while` | `WHILE(YIELD(); c, body)` | 不恢复嵌套体 | 恢复嵌套体剩余 |
| `eval_nested_yield_in_for` | `FOR(x, YIELD(); c, body)` | 同上 | 同上 |
| `eval_indirect_yield_via_var` | `LET(y, YIELD); y()` | 静默运行(无挂起) | **决策点**:保留静默 / 加 `E0014` / 运行时检测 |
| `yield_split_no_longer_splits` | 任一 SPAWN 任务 | 切段数 > 1 | 切段数 == 1(或 0) |
| `eval_yield_outside_block` | 顶层 `YIELD();` 后跟表达式 | E0014 或 OK | **决策点**:顶层 YIELD 合法? |

**决策点**:**`eval_indirect_yield_via_var` 与 `eval_yield_outside_block`** 两项需用户在 9.2 节确认。**草稿默认**:间接调用保留静默运行(运行时检测成本高于收益);顶层 YIELD 合法(已隐式支持)。

### 3.2 §17.2 同步通道真挂起

**目标**:`buf=0` 同步通道的 `CHANNEL_SEND` / `CHANNEL_RECV` 在满/空时真挂起;`TRY_*` 保持非阻塞。

**当前路径**:
- `channel.rs:TryResult<T>::WouldBlock` 枚举;
- `eval_builtin_channel_send` / `recv` 路径翻译为 `ERR(kind="ChannelWouldBlock")`;
- 用户必须用 `TRY_*` 或预置缓冲绕过。

**目标路径**:
- 删除 `WouldBlock` 枚举的 `Eval` 路径;
- 新增 `ChannelWaiter { task_id, direction: Send | Recv, value: Option<Value> }` 列表;
- `Channel::send` 路径:
  1. 若 `closed` → E0054(不变);
  2. 若有 receiver 在等 → 直接 pair up(`ChannelWaiter::pair`),双方同时 resume;
  3. 若 channel 有缓冲空位 → 入队,返回 `NULL`;
  4. 否则 → 把当前 task 加入 `Channel::senders`,状态 `TaskState::Suspended(YieldReason::SendingOn(channel_id))`;
- `Channel::recv` 路径:对称。
- 调度器在 `ChannelWaiter::pair` 成功后把两个 task 标 `TaskState::Running`,在下一轮调度执行。

**关闭协议不变**(§17.2):CHANNEL_CLOSE 后,sender / receiver 列表清空,所有 waiter 在 `TaskState::Suspended → Cancelled` 转移后,下一轮被 `TASK_IS_CANCELLED` 检测到,返回 `ERR(kind="Cancelled")`(而不是 `ChannelClosed` — 后者只给已经成功 enter 的读)。

**错误码影响**:

| 码 | v0.8.1 触发 | v0.9 触发 |
|----|-----------|---------|
| E0054 | 关闭后 SEND | 不变 |
| `ERR(kind="ChannelWouldBlock")` | 满/空 | **移除**(无 v0.9 触发路径) |
| `ERR(kind="Cancelled")` | AWAIT 已取消任务 | 不变;**新增**:挂起等待 SEND/RECV 的 task 在 channel close 后被取消,返回 Cancelled |
| `ERR(kind="ChannelClosed")` | RECV 空 | 不变 |

**测试增量**(草):

| 锁测试 | 场景 | 当前 | 目标 |
|--------|------|------|------|
| `channel_send_buf0_suspends_until_recv` | `SCOPE(FUN(() , SPAWN(...send...)); SPAWN(...recv...))` | 返 WouldBlock | 真挂起后完成 |
| `channel_recv_buf0_suspends_until_send` | 对称 | 返 WouldBlock | 同上 |
| `channel_try_send_recv_still_nonblocking` | `TRY_*` | 不变(TRY 路径) | 不变 |
| `channel_close_wakes_pending_waiters_with_cancelled` | 关闭后 waiter | 漏测 / 无 | 返回 `ERR(kind="Cancelled")` |
| `channel_pair_no_yield_in_buffered_path` | buf=1 SEND + RECV | 不挂起 | 不挂起(走缓冲) |

### 3.3 嵌套 YIELD 恢复嵌套体剩余

**目标**:`WHILE(c, ...YIELD()...)` / `FOR(x, ...YIELD()...)` / `IF(c, ...YIELD()...)` 在 YIELD 挂起后,resume 从嵌套构造内**继续**而不是跳到外层 Block 下一段。

**当前限制**:路径 B 切段在嵌套构造边界断开,YIELD 在嵌套体内被吸收或跳到外层下一段(§17.7 行 987)。

**目标路径**:状态机天然支持。嵌套构造本身是 expr,内含 YIELD 时,YIELD 触发挂起,resume 后从 YIELD 后面的 expr 继续。状态机里 `IF` 的 state 转换:`state_if_cond_done → state_if_branch_done → state_if_else_done`,每段独立。

**不需要新的实现代码** — 只是 §3.1 的真挂起落地后,该限制自动消失。

**测试增量**:复用 §3.1 的 `eval_nested_yield_in_while` / `_in_for` / `_in_if_branch`,只是断言改为"嵌套体剩余部分执行"。

### 3.4 死锁检测(最小 L1)

**目标**:检测同 scope 内,所有 task 都在等待 channel / scope cancel 而无进展的**简单循环**(L1 = 长度 ≤ 4 的循环)。

**当前状态**:[v0.7.0 不做]。

**L1 范围(草)**:
- **检测对象**:同一 `Scope` 内,**所有 `TaskState::Suspended(YieldReason::ChannelOp | YieldReason::ScopeCancel)`** task 集合。**显式 `YieldReason::Explicit` / `YieldReason::CancellationCheck` 不算等待状态**(真挂起落地后,大量 task 会显式 yield 互相等待——这是 cooperative scheduling 的正常状态,不是死锁);
- **触发条件**:调度器连续 N 轮(N = 当前 scope 内 `Suspended(ChannelOp | ScopeCancel)` task 数)未推进任何 task state;
- **报告**:顶层 `E0065` 新错误码(暂定),载荷 `{ kind: "DeadlockCycle", scope: <handle>, tasks: [<handle>, ...], cycle: [from, to, ...] }`;
- **严格限制**:不跨 scope 检测;**不检测显式 YIELD 导致的"假死锁"**(两 task 互让但不互发);不检测单个 `Suspended(ChannelOp)` task(单点不是死锁,只是等不到对端);
- **关闭路径**:开发模式默认 `W0065` 警告;生产模式可由 `wlwl.toml` 开启 `[strict_deadlock_detect]` 转 `E0065`。

**测试增量**:

| 锁测试 | 场景 | 期望 |
|--------|------|------|
| `deadlock_simple_two_way` | `SCOPE(FUN(() , SPAWN(ch1.send → wait ch2.recv); SPAWN(ch2.send → wait ch1.recv)))` | `E0065` 触发,带 cycle 报告 |
| `deadlock_three_way_cycle` | 三 task 链 | 同上 |
| `no_false_positive_in_yield_loop` | 两个 task 各做 `YIELD();` 而无 channel 依赖 | 无 E0065 |
| `no_false_positive_in_pure_yield_chain` | `SPAWN(YIELD()); SPAWN(YIELD());` 各自单独 yield(真挂起后常见) | 无 E0065 |
| `deadlock_across_scope_not_detected` | 不同 scope 内的死锁 | 无 E0065(L1 严格范围) |

**P0 内暂缓条件**:若 L1 检测在 §3.2 落地后引入的 bug 数 > 3 项,降级为"开发模式 opt-in",v0.9.1 再做。

### 3.5 公平性 / 可观测性改善

**目标**:`§17.7 "公平性 不承诺"` 改为"`§17.8 公平性 / 可观测性(v0.9 新增小节):v0.9 起尝试 best-effort FIFO,YIELD 检查点间隔下,同优先级 task 轮转次序"`;`§17.7 "在飞兄弟取消"` 改为"v0.9 起保证在下一个 YIELD 或挂起点可观测"。

**spec 章节号变更**(2026-09-24 草稿 3 第三方审计明确):
- v0.8 规范 §17 范围 §17.0 - §17.7;
- v0.9 规范**新增 §17.8 公平性 / 可观测性小节**(原 §17.7 限制表 7 项中关于公平性 / 在飞兄弟取消两行,迁入 §17.8 单独 normative 段);
- v0.9 规范**§17.7 限制表**内容相应缩减(移除已上迁至 §17.8 的两行,保留线程模型 / 性能 / 延迟-吞吐 / 阻塞挂起 / 死锁检测 / 嵌套 YIELD / 尾部 YIELD / 隐式 scope 共 7 行);
- Step 11 spec 派生时显式标注"§17.8 公平性 / 可观测性 — v0.9 新增小节",避免派生 spec 时漏。

**具体改动**:
- 调度循环从"任意 runnable"改为"按入队次序的 FIFO 队列";
- `ChannelWaiter::pair` 时,把 sender 和 receiver 都标 `Running` 但加入队列尾部,避免一对抢先多个;
- 在每轮调度前扫一遍:`Suspended(Cancelled)` task 标 `Cancelled`,对应的 waiter 也清空。

### 3.6 E0055 / E0057 决议(2026-09-24 草稿 2 修正)

| 码 | v0.8.1 状态 | v0.9 选项 A | v0.9 选项 B | 推荐(2026-09-24) |
|----|-----------|------------|------------|------|
| E0055 | 保留,触发路径走 `ERR(kind="ChannelClosed")` | **启用**:关闭后 RECV 改返 `WlwlError(E0055)` 而非 ERR 载荷 | **移除**:从注册表删除,只保留 `ChannelClosed` 载荷路径 | **B**(改) |
| E0057 | 保留,触发路径走 `E0024`(复用) | **启用**:跨任务不可变单元格用 `E0057`,单任务用 `E0024`,从复用拆分 | **移除**:跨任务路径直接 `E0024`,无 E0057 | **B** |

**推荐理由**(草稿 2 修正):
- E0055 **从 A 改为 B**:理由与 §0.5 algebraic-effect framing 一致 —— 关闭后 RECV 应作为 **tagged effect** 处理(`Cancelled { reason: {kind: "ChannelClosed", channel: ...} }`),而不是绕过 §8.2 透明传播直接抛原生错误码。这样 §17 整体保持"控制流事件都是 effect + payload"的统一叙事,与 Koka / OCaml 5 / WasmFX 提案对齐。**风险**:失去原生诊断码的强类型捕获点;用户需要 `IS_ERR` + `ERR_PAYLOAD` 模式检测(已是规范强制要求)。
- E0057 维持 B:跨任务与单任务的 immutable-cell 错误本质相同(都是 `LET`(不可变)上 `SET`),**形式上保持统一**更符合 §3.3 的"flag 不再改变"原则;`E0057` 是 §11.2 中的注册冗余,移除可减少规范表行数。

**保留 A 的例外路径**:若 §3.2 落地后,开发者**强烈偏好**"关闭后 RECV 在 lex 静态可分析时返原生码"——可保留 A 作为 opt-in,需 wlwl.toml 设 `[native_channel_close] true`。默认走 B(载荷路径)。

### 3.7 偏差登记(D9-NNN)

`docs/history/deviations-v0.8.md` 归档后,`docs/plan/deviations.md` 启动空白 D9-NNN 流水。每条偏差必须有:
- **状态**:待修复 / 已修复 / 接受保留;
- **spec 段**:行号 + 章节;
- **impl 段**:文件 + 行号;
- **原因**:实现路径 vs 语言语义;
- **影响**:程序可见行为 / 测试覆盖;
- **决议路径**:v0.9 后续步骤。

---

## 4 P1 · OOP 真实现 + 先锋增强(用户 2026-09-24 选定 α)

> 用户在 v0.9 计划会话中明确选择 **α 路径 + 优先采纳更先锋的技术路线**(Memory 2026-09-24 已固化)。
> 本章实现 OOP **最小骨架 + 三项先锋增强**:algebraic-effect framing、tag/payload cancellation、session-typed methods、behavioral class types。

### 4.1 OOP 现状(沿用 §1 概览)

| 项 | v0.8.1 状态 |
|----|-----------|
| 关键字 `CLASS` / `NEW` / `THIS` | parser → `Expr::*`,宏实现空壳 |
| 内建 `GET_PROP` / `SET_PROP` / `CALL_METHOD` | 注册表登记,`resolve_builtin` 占位空壳 |
| 错误码 `E0032` / `E0050` / `E0051` | 保留,无触发路径 |
| spec 附录 G 行 1223-1233 | 标"`OOP 未实现`" |

**v0.9 翻转**:从"已注册但未实现"翻为"真实现 + 行为类型 + 效果框架化"。

### 4.2 设计要点(α 真实现 + 先锋增强)

| 维度 | 设计 | 学术参考 |
|------|------|---------|
| `CLASS(name, parent_proto, fields)` | 创建对象字典;`parent_proto` 是会话类型表达式;`fields` 是字段名→值映射 | Links(Sam Lindley);Roc |
| `NEW(cls, args...)` | 实例化对象;若 `cls` 有 `init(args...)` 协议则按协议状态机依次校验 | Koka |
| `THIS` | 在 `CALL_METHOD` / `INIT` / `SET_PROP` 内为**真线性 capability**(不可被存入 DICT / ARRAY / 闭包捕获 / 跨 call 边界传递 / 跨 `AWAIT` 边界传递);越界返 `E0032`。**这是 Wadler 线性类型的实质,不仅是"越界报错"** | Linear type theory(Wadler 1990);Roc;Austral |
| `GET_PROP(obj, key)` | 普通字典读;`obj` 必须为 INSTANCE 类型;key 不存在 → `E0037` | — |
| `SET_PROP(obj, key, value)` | `obj` 持有对应字段,且字段标 `mutable`;否则 `E0032` | v0.6 §3.4 |
| `CALL_METHOD(obj, method, args...)` | **行为类型校验**:方法的会话类型签名被静态存储,调用时检查方法当前状态 → 下一状态;违规返 `E0051` | Session types(Honda, Yoshida, Carbone 2008);Links |
| **Algebraic-effect framing** | spec §17 用效果处理器术语:`YIELD` = `perform(Yield)`;`TASK_CANCEL` = `raise(Cancelled)`;`CHANNEL_RECV` = `await ChannelRecv(ch)`;`CHANNEL_SEND` = `perform ChannelSend(ch, v)`;运行时的 `Scheduler::step` = effect handler | Koka / OCaml 5 effects;Pretnar & Bauer 2015 |
| **Tag/payload cancellation** | `TASK_CANCEL(task, reason_dict)`:取消携带结构化载荷;`SHIELD` 块内的 `TASK_CANCEL_PARENT` 把 reason 透传到 `Cancelled.reason`;Swift/Trio 仅支持 bool / str,wlwl 走 dict 类型 | Swift Task.CancellationError;Trio cancel scopes |
| **WasmFX-style tag dispatch** | 内部取消传播复用 WasmFX 的 tag/payload 模式:`Suspended(tag: Cancelled, payload: {reason, scope, ...})`;为未来 WasmFX 后端对接留好接口 | WasmFX Phase 3(McCabe, Lindley 2026) |

### 4.3 真实现路径具体动作

| 项 | 改动 |
|----|------|
| 关键字 `CLASS` / `NEW` / `THIS` | parser 已生成 `Expr::*`;改造为**真实现**:CLASS 是对象字典构造器;NEW 是调用 init 协议的状态机;THIS 是线性 capability 引用 |
| 内建 `GET_PROP` / `SET_PROP` / `CALL_METHOD` | `resolve_builtin` 实现:`GET_PROP` 字典读;`SET_PROP` 加 mutable 校验;`CALL_METHOD` 走行为类型状态机 |
| 错误码 `E0032` / `E0050` / `E0051` | **启用**:`E0032` 不可变 SET_PROP / THIS 越界;`E0050` CLASS 协议不符;`E0051` CALL_METHOD 协议状态机违规 |
| spec §11.2 | 上述三码正式注册,触发条件 normative |
| spec §13-§16 | **新增填充**:§13 OOP 关键字与对象模型;§14 行为类型与会话类型协议;§15 THIS 与线性 capability;§16 OOP 与并发的交互(`CALL_METHOD` 内的 `YIELD` / `CHANNEL_*`) |
| 附录 G 锚点 | 新增 CLASS / NEW / THIS / GET_PROP / SET_PROP / CALL_METHOD 真实现的章节号(§13 / §14 / §15);跑 `gen-appendix-g` 重生成 |
| §12 保留形式表 | 移除 OOP 行;保留 §12.4 PANIC 等其他保留项 |
| 测试 | `parser_class_*` / `eval_class_*` / `call_method_session_type_*` / `this_linear_capability_*` 锁测试(估 ~15 项) |

### 4.4 先锋增强落地动作

#### 4.4.1 Algebraic-effect framing(spec + impl 内部命名)

**spec §17 加 normative 段**:

> "本章并发原语建模为 algebraic effects。`YIELD()` = `perform Yield`,`TASK_CANCEL(t, reason)` = `raise Cancelled(reason)`,`CHANNEL_SEND/RECV` = `perform ChannelOp`。运行时调度器是 effect handler,捕获 `Suspended` 状态作为 continuation,把控制流切到下一个 `perform` 现场。这是 Koka / OCaml 5 effect system 的具象化,wlwl v0.9 不暴露用户自定义效果(留 v0.10+),但术语与模型对齐。"

**impl 内部命名升级**(从 v0.8.1 的 `Signal` / `YieldReason` 改为 effect 风格):

```rust
// impl/crates/wlwl-eval/src/effect.rs (新文件,~80 行)
enum Effect {
    /// perform Yield — 协作让步检查点
    Yield { explicit: bool },
    /// perform ChannelOp — 同步通道 SEND/RECV 阻塞
    ChannelOp {
        dir: Send | Recv,
        channel: ChannelId,
        value: Option<Value>,  // SEND 时携带
    },
    /// raise Cancelled — 任务被取消(携带 reason)
    Cancelled { reason: Dict },
}

// impl/crates/wlwl-eval/src/runtime.rs::Scheduler
fn step(&mut self) -> Step {
    // effect handler 循环:捕获 Effect → 切走 → 恢复
    match self.current_task.effect() {
        Effect::Yield { .. } => self.suspend_current(),
        Effect::ChannelOp { dir: Send, channel, value } => self.handle_send(...),
        Effect::Cancelled { reason } => self.handle_cancel(reason),
        // ...
    }
}
```

**间接 YIELD 一并修复**(2026-09-24 审计第 3 项):`LET(y, YIELD); y()` 中 `y()` 是普通函数调用,在 effect handler 模型下每次调用都 perform `Yield`,自动挂起——**无额外实现成本,一致性免费提升**。spec §17.1 增补:"YIELD 作为值传递并经其他路径调用,与直接 `YIELD()` 等价:都是 perform `Yield` effect。"

**工作量**:~80 行重命名 + 注释(spec 同时改)。**收益**:impl 与 spec 完全对齐;为 v0.10+ 真正暴露 `perform` / `handle` 留接口;间接 YIELD 自动挂起(决策点 §9.2 间接 YIELD 默认从 C 改为 B)。

#### 4.4.2 Tag/payload cancellation(impl + spec)

spec §17.3:`TASK_CANCEL` / `TASK_CANCEL_PARENT` 签名扩展为接受可选 `reason` 参数(默认 `{}`):

| 构造 | v0.8.1 签名 | v0.9 签名 |
|------|----------|----------|
| `TASK_CANCEL(task)` | reason 缺失 | `TASK_CANCEL(task, reason)`;reason 是 DICT |
| `TASK_CANCEL_PARENT()` | reason 缺失 | `TASK_CANCEL_PARENT(reason)` |

`SHIELD(fn)` 退出后若 fn 内被 raise 的 `Cancelled` 仍未消费,该 `Cancelled` 值的 `payload.reason` 直接暴露用户传入的 dict。

落地:`task.rs` + `channel.rs` 改动约 ~200 行;新错误码 `E0066`(reason 非法 / 非 DICT 类型)。

#### 4.4.3 Session-typed methods(impl + spec,2026-09-24 草稿 2 升级)

spec §14 新章节:

> "每个 `CLASS` 可声明**会话类型协议**,约束其方法调用的合法次序。wlwl v0.9 支持三种协议构造,**对齐 Honda-Yoshida-Carbone 2008 / Lindley-Morris Links 风格**:
>
> 1. **顺序**:`{a: T1, b: T2, c: end}` —— a 完成后才能 b,b 完成后才能 c;
> 2. **选择**:`{a: T1 ⊕ b: T2}` —— 调用方必须在 a / b 二选一;`⊕` 是内部选择(调用方决定),`&` 是外部选择(被调用方提供);
> 3. **递归**(μ 骨架):`{rec: μX. {get: ?int.X, close: end}}` —— 协议可在末尾递归引用自身。
>
> 示例:
>
> ```
> CLASS("Counter", {
>     init: end,
>     rec: μX. {
>         get: ?int.X,
>         inc: end,
>         get: ?int.X,
>         close: end
>     }
> }, {count: 0})
> ```
>
> `CALL_METHOD(obj, "inc", ...)` 后必须先 `CALL_METHOD(obj, "get", ...)` 才能再 `inc`,否则触发 `E0051`;`get` 后回到 μ 头部,可继续 inc 或 close。"

**v0.9.0 落地集**(2026-09-24 审计第 4 项采纳):
- **顺序**:完整支持
- **选择**(`⊕` 内部选择):完整支持
- **递归**(μ 骨架):**完整支持**,语法 `μX. { ... X ... }`
- **外部选择**(`&`):**留 v0.9.1**(分支爆炸,测试矩阵 4×N)
- **命名协议**(`typealias`):**留 v0.10+**(需类型系统扩展)
- **并行分支**(`par`):**留 v0.10+**(语义复杂)

落地:`wlwl-eval/src/class.rs` 新文件(~500 行,vs 原估 400);`wlwl-eval/src/protocol.rs` 新文件(~300 行,μ + 选择解析);`BUILTIN_REGISTRY` 加 `CALL_METHOD` 的协议解析路径;`parser` 支持 `CLASS` 第二个参数是协议表达式。

#### 4.4.4 WasmFX-style tag dispatch(internal,2026-09-24 与 §4.4.1 合并)

**与 §4.4.1 合并**:tag dispatch 是 algebraic-effect framing 的运行时形态——`Effect` 枚举本身就是 WasmFX-style 的 `tag + payload`。impl 内部命名 `Effect::Yield` / `Effect::ChannelOp` / `Effect::Cancelled` 即是 tag 命名;`step()` 调度循环是 WasmFX `resume` 的对应物。

未来对接 Wasm 后端时一一映射:
| wlwl `Effect` 变体 | WasmFX `tag` | WasmFX `suspend` 指令 |
|----|----|----|
| `Yield { explicit }` | `$tag_yield` | `suspend $tag_yield` |
| `ChannelOp { dir, channel, value }` | `$tag_channel_send` / `$tag_channel_recv` | `suspend $tag_channel_*` |
| `Cancelled { reason }` | `$tag_cancelled` | `suspend $tag_cancelled` |

**工作量**:已在 §4.4.1 计入(80 行重命名)。

### 4.5 `MODULE_REF` / 原生模块边界

spec §9.2 行 570:模块字典值经 `IMPORT` 之外获得(`MODULE_REF`)是保留形式(第 12 章)。

**v0.9 决议**:
- `MODULE_REF` 在 `BUILTIN_REGISTRY` 里已注册(`registry.rs` 行 ~1223);
- **保留**该注册,但显式注明语义:`MODULE_REF(path) -> MODULE` 返回模块字典,与 `IMPORT` 等价但不绑定名字到当前作用域;
- spec §9.2 加一句 normative:"`MODULE_REF` 用于运行时动态加载(测试 / 反射 / 自省),其返回字典与 `IMPORT` 的展开结果不可区分;不是新机制,是 `IMPORT` 的非绑定形式";
- 同步 §17.7 / §11.2 中相关条目。

---

## 5 P2 · AI / 标准库协议(最小正式契约)

> v0.9 仅做"最小正式契约",不实现新 std 模块。

### 5.1 `wlwl:std.ai` / `wlwl:std.agent` 协议最小契约

spec §10.11 行 725:**"其协议细节属于实现文档,不在本规范内"**。v0.9 改为:**最小契约在规范内,实现细节仍可演进**。

**最小契约内容**(草):

| 项 | 契约 |
|----|------|
| `wlwl:std.ai.TASK(name, prompt, ...)` | 异步构造 AI 请求任务;返回 TASK 句柄;失败产 `E0080`-`E0083` |
| `wlwl:std.ai.MODEL(name)` | 选模型;返回模型描述 dict;`E0080` |
| `wlwl:std.ai.TOOL(name, schema, fn)` | 注册工具;`E0081`(重复注册 / 协议不符) |
| `wlwl:std.ai.CALL_TOOL(name, args)` | 同步调用工具;`E0082` |
| `wlwl:std.ai.CONTEXT(set, get, ...)` | 上下文;失败 `E0083` |
| `wlwl:std.agent.TASK(...)` | 高级代理任务;返回 TASK;失败 `E0090`-`E0094` |
| 错误码语义 | `E0080` 请求失败;`E0081` 协议错误;`E0082` 工具错误;`E0083` 上下文错误;`E0090`-`E0094` 同上 |

**E0080-E0094 的 v0.9 落点**:
- 触发路径在 `wlwl:std.ai` / `wlwl:std.agent` 实现内,本计划不重写;
- **只需在 spec §11.2 + §10.11 加 normative 行**:每道 E-code 的触发条件 + 错误载荷形式;
- 与并发收口的关系:这五条 E-code 跨 `TASK` 句柄,跨任务路径要复用 §3.2 的真挂起协议;`AWAIT` 已取消任务返 `Cancelled` 的约定要在这里重申。

### 5.2 §10.6 / §10.5 / §10.11 文档冲突清理

历史:`deviations-v0.8.md` 列出多个 §10 章节与注册表不同步。v0.9 在 §3 完成后,重跑 `gen-appendix-g` 重生成附录 G(锚点回归测试 `b11_*` 已守住),同步修 §10.6 / §10.5 / §10.11 prose。

### 5.3 `RUN_TESTS` / `STRINGIFY` / `CHANNEL_NEW buf` 上界 — v0.8 报告附录 5 条

| 项 | v0.8 状态 | v0.9 决议 |
|----|---------|---------|
| `RUN_TESTS` 字段缺失 | §10.x 提到但未定义 | 在 §10.11 加 normative:返回 DICT `{name, status, duration_ms, ...}` |
| `STRINGIFY` 排序 | 与 `STR` 重复? | 审计;若重复则合并;若独立则在 §10.5 留行 |
| `CHANNEL_NEW buf` 上界 | 无上界 | 决议:无上界(运行时分配) vs 上界 `1<<32`(文档化)<br>**推荐**:无上界,但大 buf 产 `W0066` 软警告 |

---

## 6 ADR-0017 / 0018 / 0019 草案

> 本节给出新增 ADR 草案骨架,正文待用户审阅后定稿。

### 6.1 ADR-0017 — Cooperative suspension-based scheduler

**Status**: Proposed(v0.9 草稿)
**Deciders**: Li (project lead)
**Related**: ADR-0014 / 0015 / 0016;spec v0.9 §17;wlwl-build-plan-v0.9 §3

**Context**:v0.7 走"静态切段 + WouldBlock 通道",承认 YIELD 位置限制是实现路径(§17.1 行 879)。v0.9 把 §17 从切段升级为真挂起。

**Decision**(草):
- 删除 `yield_split.rs:split_body_for_yield` 的切段逻辑;
- `Channel::send` / `recv` 满/空时挂起当前 task;
- 调度循环驱动 task 在 `Suspended(reason) → Running` 之间;
- §17.7 限制表对应行修订;
- E0014 不再触发 YIELD 位置;
- Brown 9 维度维持 v0.7 选择,Suspension 一项的工程实现从"动态切段"改为"动态真挂起"。

**Consequences**:
- Positive:`buf=0` 同步通道可用;嵌套 YIELD 恢复;YIELD 可在任意表达式位置;
- Negative:重写 `yield_split.rs` + `runtime.rs` + `channel.rs` 三大块;并发 conformance 全量重测;性能预算不变(单 task 退化 < 10%)。

### 6.2 ADR-0018 — E0055 / E0057 retention decision(草稿 2 同步修订)

**Status**: Proposed(2026-09-24 草稿 2 已与 §3.6 / §9.2 推荐路径同步)
**Related**: spec v0.9 §11.2;wlwl-build-plan-v0.9 §3.6

**Decision**(草稿 2 修订):
- E0055:**移除**(草稿 2 改,与 §3.6 / §9.2 一致) — 关闭后 CHANNEL_RECV 走 `ERR(kind="ChannelClosed")` 载荷路径,与 §0.5 algebraic-effect framing 的"控制流事件都是 effect + payload"统一叙事对齐。**保留 opt-in 路径**:`wlwl.toml` 设 `[native_channel_close] true` 可启用原生码(用于强类型捕获需求)。
- E0057:**移除** — 跨任务不可变单元格继续走 `E0024`(与单任务路径统一)。

**Consequences**(草稿 2):
- Positive:§11.2 表精简(两道保留码都移除);`E0057` 不会再"已注册但永不触发";§17 整体保持 algebraic-effect + tagged effect 一致性;opt-in 路径保留诊断码灵活性;
- Negative:失去默认原生诊断码的强类型捕获点;用户需 `IS_ERR` + `ERR_PAYLOAD` 模式检测(已是规范强制要求)。

### 6.3 ADR-0019 — OOP minimal implementation w/ behavioral types(α + 先锋)

**Status**: Accepted(用户 2026-09-24 选定;2026-09-24 草稿 2 审计修订)
**Deciders**: Li (project lead)
**Related**: spec v0.9 §13-§16 填充;wlwl-build-plan-v0.9 §4;ADR-0017 / 0018;Lindley & Morris (Links);Honda, Yoshida, Carbone (session types);Wadler 1990 (linear types)

**Context**:v0.7 / v0.8.1 的 OOP 占位(`CLASS` / `NEW` / `THIS` / `GET_PROP` / `SET_PROP` / `CALL_METHOD`)已注册但未实现,附录 G 标"`OOP 未实现`",属规范债务。用户在 v0.9 计划会话中明确选择 α 路径 + 优先采纳更先锋的技术路线(2026-09-24 Memory 固化)。2026-09-24 第三方审计后做四项修正。

**Decision**:**真实现 OOP 最小骨架 + 四项先锋增强(草稿 2 升级)**。

1. **OOP 最小骨架**:
   - `CLASS(name, parent_proto, fields)` 创建对象字典;`NEW(cls, args...)` 实例化;
   - `THIS` 是**真线性 capability** —— 不可存入容器 / 不可跨 call 边界 / 不可跨 `AWAIT` 边界;Wadler 1990 风格实质检查,不只是"越界报错";
   - `GET_PROP` / `SET_PROP` / `CALL_METHOD` 走字典 + 行为类型状态机;
   - 错误码 `E0032` / `E0050` / `E0051` 启用,§11.2 normative。

2. **Algebraic-effect framing**(草稿 2 升级:spec + impl):
   - §17 用 effect handler 术语:`YIELD` = `perform Yield`,`TASK_CANCEL` = `raise Cancelled`;
   - **impl 升级**:新增 `effect.rs`(~80 行),`enum Effect { Yield, ChannelOp { dir, channel, value }, Cancelled { reason: Dict } }`;`Scheduler::step` 改为 effect handler 循环;
   - **间接 YIELD 自动挂起**:`LET(y, YIELD); y()` 中 `y()` 作为函数调用,在 effect handler 模型下每次都 perform `Yield` effect,自动挂起(无额外实现成本);
   - 不暴露用户自定义效果(留 v0.10+),但术语 + impl 内部命名与 Koka / OCaml 5 对齐。

3. **Tag/payload cancellation**(impl + spec):
   - `TASK_CANCEL(task, reason_dict)` / `TASK_CANCEL_PARENT(reason_dict)`;
   - `SHIELD` 内的 cancelled 把 reason 透传到 `Cancelled.payload.reason`;
   - 新错误码 `E0066`(reason 非法类型)。

4. **Session-typed methods**(impl + spec,§14 新章节,草稿 2 升级):
   - 每个 CLASS 可声明会话类型协议,约束方法调用次序;
   - v0.9.0 落地集:**顺序 + 选择(`⊕`)+ μ 递归骨架**;外部选择 `&` / 命名协议 / 并行 `par` 留 v0.9.1+;
   - `CALL_METHOD` 走协议状态机,违规返 `E0051`;
   - 参考 Links(Lindley & Morris);Honda, Yoshida, Carbone 2008。

5. **WasmFX-style tag dispatch**(internal,与 §4.4.1 合并):
   - 内部 `Effect` 枚举本身就是 WasmFX-style 的 `tag + payload`;`Scheduler::step` 对应 WasmFX `resume`;
   - 为 v0.10+ Wasm 后端留接口(`suspend $tag_yield` / `$tag_channel_*` / `$tag_cancelled` 一一映射)。

**章节号冻结规则**(草稿 2 新增,审计第 5 项采纳):
- §13 / §14 / §15 / §16 **章节号在 v0.9.0 release tag 上冻结**;
- v0.9.1 只能向**已存在章节追加 sub-section**(§14.1 / §14.2 / ...),不重新编号;
- v0.10+ 若章节饱和,新增 §18+ 但不得回填进 §13-§16;
- 附录 G 锚点与 spec 章节号一一对应(`b11_*` 锁测试守住)。

**Consequences**(草稿 2):
- Positive:OOP 表达力 + §13-§16 章节填充 + **四个**学术前沿设计同时落地;wlwl 在结构化并发 + 行为类型方向**跻身学术前沿阵营**(Koka / Links / OCaml 5 / WasmFX 风格);章节号冻结规则避免后续 v0.9.1 撕裂 spec;
- Negative:工作量 ~14-18 人·周(草稿 2 上调:并发收口 ~5-7 + algebraic-effect runtime ~1 + OOP 真实现 ~3 + session types 含 choice+μ ~3-4 + 死锁 L1 ~1 + tag/payload ~1 + spec + 测试 ~3.5);spec 与 impl 同步复杂;新错误码 + 新章节号回归测试;`gen-appendix-g` 重生成两次(并发段 + OOP 段)。
- 风险:session types 选择 + μ 落地有可能引入 spec 复杂性争议(尤其 `⊕` 与 E0050 / E0051 边界);E0051 触发路径需细致测试覆盖(估 ~10 项锁测试);真线性 THIS 运行时检查对每次 `CALL_METHOD` 入口有少量开销(~5%,估计)。

---

## 7 Brown 9 维空间再确认(基于 Gray 等 OOPSLA 2026)

> Gray, Krishnamurthi, Crichton. *A Design Space Exploration of Async/Await*. OOPSLA 2026, DOI 10.1145/3839519,arXiv 2608.20677。

### 7.1 论文核心结论(原文摘要)

> "We dissect several existing languages, and show how no two of them agree as a whole on design decisions that affect the presence and ordering of execution."

论文用 9 维空间 + 核心演算的形式化模型证明:**同一段 async 程序在 7 个主流 runtime(Python Asyncio/Trio,Rust Tokio/Smol,C#,JavaScript,Swift)上输出 4 种结果,且**没有任何两个 runtime 在所有 9 维度上同时一致。

### 7.2 v0.7 决策与论文对照

| 维度 | v0.7 选 | 论文对维度定义 | v0.7 选是否自洽 |
|------|-------|----------------|---------------|
| Eagerness | Lazy | "调用异步函数是否立即开始执行" | ✔ 与 Trio/Kotlin/cff 一致 |
| Suspension | Dynamic(切段) | "await 点是否保证挂起" | ⚠ v0.7 用"动态切段"实现 Dynamic,严格说是"切段即挂起"与论文 Dynamic 不完全对齐 |
| Extent | Dynamic | "任务默认能存活多久" | ✔ 与 Swift/Trio 一致 |
| Ref Strength | Strong | "运行时持任务的引用类型" | ✔ |
| Destruction | Awaited | "extent 结束时任务如何清理" | ✔ 与 Trio/Java 一致 |
| Propagation | Destructive | "异常是否被依赖者重抛" | ✔ 与 Trio 一致 |
| Awareness | Aware | "任务能否响应取消" | ✔ |
| Direction | Top-Down | "取消如何沿任务图传播" | ✔ |
| Persistence | Transient | "取消请求是否持久" | ✔(与 cff / Kotlin 一致;**有意偏离** Trio / Swift Persistent) |

### 7.3 v0.9 改动对 9 维空间的影响

**Suspension 一项从"动态切段"升格为"动态真挂起"** — 这是 v0.9 唯一触及 9 维空间工程实现的一项。其他八项不变。

论文中,Dynamic Suspension 的工程含义是"await 点真挂起,await 不必为真挂起"。v0.7 的切段在严格意义上是"动态但部分点位已被吸收";v0.9 真挂起后,Suspension = Dynamic 的承诺完整成立。

### 7.4 论文对 v0.9 设计的两个隐式支持

1. **"Suspension = Dynamic 比 Static 更适合 fidelity"**(论文 §3):v0.9 走 Dynamic 是对的;
2. **"Persistence = Transient 在单线程协作下足够"**(论文 §5,以及 ADR-0015 已论证):v0.7 选 Transient,单线程边界 ADR-0016 不变,v0.9 维持。

---

## 8 挂起式调度技术路线(对照)

> 本节汇总外部研究(v0.9 综合材料)的实现路径,作为 §3.1 / §3.2 的工程依据。

### 8.1 三大主流路径

#### 8.1.1 Kotlin coroutines(无栈状态机 + 调度器)

**路径**:编译器把 `suspend fun` 经 CPS 转换为状态机类(`ContinuationImpl` 子类),每个挂起点是状态标签;调度器在挂起点挂起,resume 时跳到对应状态。

**v0.9 对照**:`eval_expr` 在 YIELD 调用点产 `Signal::Yield`,调度器捕获 → 任务挂起 → 下一轮调度 → `eval_expr` 从挂起点之后继续。**等价于 Kotlin 模式但由解释器而非编译器做**。

**关键代码形态**(类比 Kotlin `loadProfileContinuation`):

```
// 伪代码:eval_expr 内部
loop {
    match current_state {
        Initial => {
            current_state = AfterYield;
            return Signal::Yield(Explicit);
        }
        AfterYield => {
            // 继续执行 YIELD 之后的代码
        }
    }
}
```

#### 8.1.2 Rust async/await(无栈状态机 + Pin + Executor)

**路径**:`async fn` 编译为实现 `Future` trait 的匿名 enum;`poll` 方法驱动;`Pin` 保证内存稳定。

**v0.9 对照**:不需要 Pin(v0.6 是解释器 + Rc<Env>,无栈搬迁问题);但 Future 模型可作为状态机设计的概念骨架。

**关键区别**:Rust Future 由调用方 poll 推进,wlwl v0.9 由调度器 resume 推进(类似 Kotlin 而非 Rust)。

#### 8.1.3 BEAM process(有栈 + 抢占式)

**路径**:每个 Erlang process 有独立栈 + 堆;VM 维护 reduction budget;budget 用完则抢占。

**v0.9 对照**:**不采用**(ADR-0016 边界外)。BEAM 是有栈 + M:N;wlwl v0.9 是无栈 + 1:1。两者在"调度"上有概念同构,在"实现"上根本不同。

### 8.2 WebAssembly Stack Switching(WasmFX,Phase 3)

> McCabe, Lindley. *WebAssembly Stack Switching Proposal*. 2026-05。

**路径**:为 Wasm 加 `cont.new` / `cont.bind` / `resume` / `suspend` / `switch` 等指令;延续是一等公民;引擎真正切换栈。

**v0.9 对照**:**长期路径**,v0.9 不采用(impl 是 Rust 解释器,非 Wasm)。但**v0.9 的"真挂起 + 状态机"模式是 WasmFX 之前的最佳近似** — 同一段源码在 v0.9 与 WasmFX 引擎上语义一致(挂起点 = suspend)。

**未来**:**v0.10+** 若 wlwl 加 Wasm 后端且 WasmFX 进入 Phase 4,可考虑把 impl 的调度循环翻译为 WasmFX 指令(类比:Linear Logical / Multicore OCaml 已经验证这种翻译可行)。

### 8.3 形式化基础(可选)

- **会话类型**(Honda, Yoshida, Carbone 2008):为通道提供静态协议保证。**v0.9 不引入到通道**(通道走 §3.2 真挂起的载荷路径);**v0.9 引入到 OOP 方法签名**(§4.4.3)。
- **结构化并发四不变量**(Smith 2018):**已满足**,v0.9 维持。
- **Brown 9 维空间**(Gray 等 2026):**v0.7 已逐项决议**,v0.9 维持 + Suspension 升格。
- **WasmFX continuation 类型**:长期目标,v0.9 不动。

---

## 9 实施顺序

### 9.1 阶段切分

```
[Step 1] ADR-0017 / 0018 / 0019 定稿(用户审阅)
   ├─ 0017:挂起式调度;0018:E0055/E0057 决议;0019:OOP 真实现 + 行为类型 + 先锋增强(用户已 2026-09-24 选定 α + 先锋)
   └─ 三份 ADR 必须在动手改 impl 前定稿,避免来回返工
[Step 2] impl:yield_split.rs 简化为 ~100 行入口包装器
   ├─ 删除 validate + collect 路径
   ├─ eval_expr 直接产 Signal::Yield(Explicit)
   └─ 锁测试:eval_yield_in_let_position / _in_if_branch / _in_array_literal
[Step 3] impl:runtime.rs::Scheduler 加 Suspended(YieldReason::*) 处理
   ├─ 调度循环支持 Blocking + Cancellation
   ├─ ChannelWaiter 列表 + pair 逻辑
   ├─ WasmFX-style tag dispatch 内部命名(Tag::Yield / Tag::ChannelOp / Tag::Cancelled)
   └─ 锁测试:scheduler_resume_after_suspend / _after_cancel
[Step 4] impl:channel.rs 同步通道真挂起
   ├─ 删除 TryResult<T>::WouldBlock 在 SEND/RECV 路径上的使用(TRY_* 保留)
   ├─ ChannelWaiter { task_id, direction, value } 数据结构
   └─ 锁测试:channel_send_buf0_suspends_until_recv / _recv_buf0_suspends_until_send / _close_wakes_pending_waiters
[Step 5] impl:死锁检测 L1(若 §3.4 落地)
   ├─ Scope 内的 Suspended task 全集 + N 轮未推进触发
   ├─ E0065 错误码注册(开发模式 opt-in)
   └─ 锁测试:deadlock_simple_two_way / _three_way_cycle / _no_false_positive
[Step 6] impl:E0055/E0057 决议(草稿 2 §3.6 默认 B=全部移除)
   ├─ E0055 移除:从注册表删除,关闭后 RECV 走 `ERR(kind="ChannelClosed")` 载荷路径(同 §0.5 algebraic-effect 叙事)
   ├─ E0057 移除:跨任务不可变单元格继续走 E0024
   ├─ §11.2 表更新:移除两道保留码
   └─ 锁测试:registry_no_e0055_no_e0057_after_v09 + channel_close_returns_channelclosed_payload_not_native
[Step 7] impl:wlwl:std.ai / wlwl:std.agent 错误码契约同步
   ├─ §10.11 prose 增补 E0080-E0094 触发条件
   └─ 锁测试:registry_e0080_to_e0094_in_appendix_g
[Step 8] impl:tag/payload cancellation(§4.4.2 先锋增强之一)
   ├─ task.rs: TASK_CANCEL(task, reason) / TASK_CANCEL_PARENT(reason)
   ├─ Cancelled 错误载荷增加 payload.reason
   ├─ 新错误码 E0066(reason 非法类型)
   └─ 锁测试:cancel_with_reason_payload_propagates / _no_reason_defaults_to_empty_dict
[Step 9] impl:OOP 最小骨架(§4.3 + §4.4.3 行为类型)
   ├─ parser: CLASS 第二个参数支持协议表达式;THIS 在 CALL_METHOD/SET_PROP 内识别为线性 capability
   ├─ wlwl-eval/src/class.rs 新文件(~400 行):对象字典构造、协议状态机、CALL_METHOD 校验
   ├─ registry.rs: GET_PROP/SET_PROP/CALL_METHOD 替换为真实现
   ├─ 错误码: E0032/E0050/E0051 启用,§11.2 normative
   └─ 锁测试:parser_class_with_session_type / _call_method_violates_protocol_returns_e0051 / _this_escape_returns_e0032 / _set_prop_immutable_returns_e0032 / ~15 项
[Step 10] impl:WasmFX-style tag dispatch 内部命名清理
   ├─ runtime.rs: Suspended { tag, payload } 字段重命名
   ├─ 文档注释对照 WasmFX 提案
   └─ 锁测试:scheduler_internal_tag_construction(无外部观察变化)
[Step 11] spec v0.9 派生 — 并发段
   ├─ §17 algebraic-effect framing normative 段(§4.4.1)
   ├─ §17.1 YIELD 位置限制解除
   ├─ §17.2 同步通道真挂起
   ├─ §17.3 tag/payload cancellation 签名扩展
   ├─ §17.7 限制表对应行修订
   ├─ §11.2 E0055/E0057 / E0065 / E0066 修订
   ├─ 附录 G 重新生成(gen-appendix-g)
   └─ 锁测试:appendix_g_anchors_match_v09_section_numbers
[Step 12] spec v0.9 派生 — OOP 段
   ├─ §13 OOP 关键字与对象模型
   ├─ §14 行为类型与会话类型协议(新增)
   ├─ §15 THIS 与线性 capability(新增)
   ├─ §16 OOP 与并发的交互(CALL_METHOD 内 YIELD / CHANNEL_*)
   ├─ §11.2 E0032/E0050/E0051 启用注册
   ├─ 附录 G 二次重新生成(OOP 段锚点)
   └─ 锁测试:appendix_g_oop_anchors_match_v09_section_numbers
[Step 13] 重生成 v07_fidelity baseline
   ├─ 重新跑所有 v0.6 conformance fixture
   ├─ 锁测试:v07_fidelity_v08_baseline → v07_fidelity_v09_baseline
   └─ 性能基准:单 task 退化 < 10% 不变
[Step 14] deviations-v0.9.md 启动(D9-NNN 流水)
[Step 15] CHANGELOG.md v0.9 段
[Step 16] end-to-end 验收
```

### 9.2 决策点(草稿 3 全部拍板,等用户最后确认)

| 决策点 | 默认 | 用户选项 | 状态 |
|--------|------|---------|------|
| **§3.1 间接 YIELD**(`LET(y, YIELD); y()`) | **B:显式允许 + 运行时挂起** | A:加 `E0014`(运行时检测);**B:显式允许(默认,效果处理器下自动挂起)**;C:静默 | ✅ 草稿 2 调整 |
| **§3.1 顶层 YIELD** | **A:合法(已隐式支持)** | A:合法(默认);B:加 `E0014` | ✅ 草稿 3 默认 |
| **§3.4 死锁检测** | **A:L1 默认启用(严格排除 `YieldReason::Explicit`),生产模式 opt-in** | A:L1 启用(默认,2026-09-24 收紧范围);B:v0.9 不做,留 v0.9.1;C:仅开发模式 | ✅ 草稿 2 调整 + 草稿 3 默认 |
| **§3.6 E0055** | **B:移除(与 tag/payload 一致)** | A:启用;**B:移除(默认,与 §0.5 algebraic-effect 一致)**;C:保留 v0.8 | ✅ 草稿 2 调整 |
| **§3.6 E0057** | **A:移除** | A:移除(默认,与 E0055 对称);B:启用;C:保留 v0.8 | ✅ 草稿 3 默认 |
| **§4 OOP** | **α 真实现 + 行为类型 + 先锋增强** | **用户 2026-09-24 选定 α** | ✅ 已确定 |
| **§4.4.3 session types 集合** | 顺序 + 选择 + μ 递归骨架;**外部选择 & / 命名协议 / 并行分支留 v0.9.1+** | A:最小集(顺序);**B:v0.9.0 = 顺序 + 选择 + μ(默认)**;C:全集合 | ✅ 草稿 2 调整 |
| **§5.3 CHANNEL_NEW buf 上界** | **A:无上界 + 大 buf W0066 软警告** | A:无上界(默认);B:1<<32 上界;C:无上界,无警告 | ✅ 草稿 3 默认 |
| **§6 ADR-0017 / 0018** | Proposed | A:全部 Approved;B:逐项决策 | ✅ 草稿 3 默认(全部 Approved) |
| **§6 ADR-0019** | **Accepted**(α + 先锋) | — | ✅ 已确定 |

### 9.3 锁测试预期增量

- 现有 1409 passed +0 failed(保留)
- 增量(估):
  - §3.1 ~10(YIELD 位置 + 间接 YIELD 自动挂起)
  - §3.2 ~5(通道真挂起)
  - §3.3 ~3(嵌套 YIELD 恢复)
  - §3.4 ~5(死锁检测 L1,含 `no_false_positive_in_pure_yield_chain`)
  - §3.5 ~2(公平性)
  - §3.6 ~1(E0057 移除验证)
  - §4.4.1 ~3(algebraic-effect runtime 命名 + 间接 YIELD 自动挂起)
  - §4.4.2 ~3(tag/payload cancel + E0066 类型校验)
  - §4.4.3 ~20(OOP 最小骨架 ~10 + 行为类型顺序/选择/μ ~10)
  - §4.4.4 ~1(WasmFX-style tag dispatch,已在 §4.4.1 计入)
  - §5 ~3(std.ai/agent 契约)
- 合计估:**~56 项**
- v07_fidelity baseline 重派生 + **0**(基线本身不变项数)
- **合计估:~1465 passed**(待 §11 验收)

---

## 10 风险与回退

| 风险 | 等级 | 缓解 | 回退路径 |
|------|------|------|---------|
| `yield_split.rs` 重写破坏现有 SPAWN 行为 | 高 | 锁测试覆盖所有 YIELD 位置场景(§3.1);SPAWN 测试通过基线比对 | git revert 到 v0.8.1 baseline |
| 同步通道真挂起导致 v0.7 concurrency conformance 半数失败 | 中-高 | v0.7 conformance 需重写(必须);锁测试在新版本上重派生 | 回退到 v0.8.1 WouldBlock 路径(临时) |
| E0055 启用破坏 §8.2 ERR 消费者协议 | 中 | E0055 与 `ChannelClosed` 载荷路径并存,消费者可二选一 | 降级为 B(移除 E0055) |
| 死锁检测 L1 假阳性率 > 1% | 中 | opt-in 默认;`W0065` 软警告而非 `E0065`;v0.9.1 调阈值 | 关闭 L1 检测,留 v0.9.1 |
| OOP 行为类型(session types)实现过复杂 | 中 | v0.9.0 落地集 = 顺序 + ⊕ 选择 + μ 递归骨架;外部选择 `&` / 命名协议 / 并行 `par` 留 v0.9.1+;E0051 触发路径全锁测试覆盖 | μ 递归落地时若超预期(>2 人·周),降级 μ 为 v0.9.1,v0.9.0 仅顺序 + ⊕ |
| OOP 实现时间超 6 人·周 | 中 | 把 session types 部分延后到 v0.9.1;v0.9.0 只实现 CLASS/NEW/THIS/GET/SET_PROP 最小骨架(2-3 人·周) | v0.9.0 不含 OOP,v0.9.1 加 |
| tag/payload cancel 与 SHIELD 协议冲突 | 低 | reason 透传路径独立于 SHIELD 嵌套计数;锁测试覆盖 | E0066 触发时回退到无 reason 模式 |
| OOP 移除破坏现有 OOP 用户代码 | 中(预期低) | OOP 未实现,无生产代码;锁测试守住 | 切回保留 + E0011 抛出路径 |
| 单 task 性能退化 > 10% | 中 | 调度循环优化(批量调度);`cargo bench --workspace` | 优化空间不足则放宽 §10 D20 阈值(需用户审) |
| spec §17.1 / §17.7 修订引入与 §11.2 / §12 的连锁不一致 | 中 | §9 Step 11 集中跑 `gen-appendix-g` 重生成;`b11_*` 守住 | 分章节逐步提交,锁测试每次 |
| §13-§16 章节填充与 §17 修订连锁冲突 | 中 | OOP 段派生(Step 12)在并发段派生(Step 11)完成后做;每段独立跑 `gen-appendix-g` | 锁测试每次守住 |

---

## 11 验收

### 11.1 自动验收

```
cargo test --workspace     # 0 failed,~1465 passed(目标,§9.3 估 ~56 项增量)
cargo fmt --check          # 0 diff
cargo clippy --locked --workspace --all-targets -- -D warnings  # 0 warnings
cargo bench --workspace    # 单 task 退化 < 10%(vs v0.6 baseline)
cargo doc --workspace      # 0 warnings
```

### 11.2 一致性表(README §0.4 / CHANGELOG.md)

| 一致性项 | v0.8.1 | v0.9 目标(α + 先锋 + 草稿 2 修正) |
|---------|-------|----------|
| 错误码数 | **56 激活 + 11 保留**(共 67 总注册) | **61 激活 + 6 保留**(共 67 总注册,总数不变)。明细:56 激活 + 3(E0032/E0050/E0051 从保留转正)+ 2(E0065/E0066 新增)- 0 = 61;11 保留 - 3(转正)- 2(移除 E0055/E0057)= 6 |
| 警告码数 | (v0.8.1 警告码注册表见 spec §11.2,**未在 v0.9 一致性表追踪**) | **+ W0065**(死锁检测软警告,§3.4)+ **+ W0066**(大 buf 软警告,§5.3);**impl 内部诊断,spec §11.2 不正式注册**(留 v0.9.1 spec 增补) |
| 内建数 | 110 | 110 + E0065 死锁检测 + E0066 reason 类型 + OOP 真实现 0 新增(GET/SET_PROP/CALL_METHOD 已在内,只是从占位变真实现)= **110**(净不变,但 §13-§16 章节填充) |
| 字面量 / 关键字 | 不变 | 不变 |
| spec 章节数 | §1-§17 + 附录 A-G | §17 修订 + **新增 §13-§16** = §1-§17 + 附录 A-G(章节号外推);附录 G 二次重新生成 |
| Brown 9 维度 | 9 选定 | 9 不变 + Suspension 升格 |
| 行为类型(session types) | 无 | **新增** —— CLASS 协议表达式 + CALL_METHOD 状态机(顺序 + 选择 + μ 递归骨架) |
| THIS 线性 capability | 无 | **新增** —— 真线性检查(不可存入容器 / 不可跨 call 边界 / 不可跨 AWAIT 边界),违规返 E0032 |
| Algebraic-effect framing | 无 | **新增** —— spec 文档化 + impl `enum Effect { Yield, ChannelOp, Cancelled }` runtime 命名 |
| Tag/payload cancellation | 不支持 | **新增**(impl + spec);`TASK_CANCEL(task, reason_dict)` |
| WasmFX-style tag dispatch | 无 | **新增**(impl 内部命名 + 与 §4.4.1 effect 合并) |
| 锁测试数 | 1409 | **~1465**(估,~56 项增量) |
| **工作量(更现实估)** | — | **14-18 人·周**(草稿 2 上调:并发收口 ~5-7 + algebraic-effect runtime ~1 + OOP 真实现 ~3 + session types 含 choice+μ ~3-4 + 死锁 L1 ~1 + tag/payload ~1 + spec + 测试 ~3.5)。**降级路径**:若 session types μ 递归实现超预期(估 >2 人·周),降级 μ 为 v0.9.1,v0.9.0 仅顺序 + ⊕,省 ~1-2 人·周,总工作量降至 ~12-15 人·周 |

### 11.3 文档验收

- `docs/standard/wlwl-spec-v0.9.md` 派生;
- `docs/adr/0017-cooperative-suspension-scheduler.md` Accepted;
- `docs/adr/0018-e0055-e0057-retention-decision.md` Accepted;
- `docs/adr/0019-oop-minimal-implementation-with-behavioral-types.md` Accepted(α + 先锋);
- `docs/plan/deviations.md` 启动 + D9-NNN 流水;
- `CHANGELOG.md` v0.9 段;
- `README.md` §0.4 一致性表更新;
- `docs/standard/wlwl-spec-v0.9.md §13-§16` 填充(OOP 关键字、对象模型、行为类型、THIS 线性、OOP-并发交互);
- `docs/standard/wlwl-spec-v0.9.md §17` algebraic-effect framing normative 段;
- `docs/standard/wlwl-spec-v0.9.md §17.3` tag/payload cancellation 签名扩展;
- `docs/standard/wlwl-spec-v0.9.md §17.8` 公平性 / 可观测性新增小节(§3.5);
- `docs/standard/wlwl-spec-v0.9.md §17.7` 限制表缩减(7 项,原 §17.7 限制表内的"公平性" / "在飞兄弟取消"两行已上迁至 §17.8 独立小节)。

### 11.4 留 Step 11/12 spec 派生时定的 3 处小问题(草稿 4 三次审计标"可以不动")

| 问题 | 拟决议(草) |
|------|-----------|
| **负 `buf` 错误码**(§17.2 "`buf` 须为非负 INTEGER" 未定错误码) | **E0030**(类型错,沿用 §2.4)或 **E0038**(类比 RANGE 步长零);Step 4 落地时由实现选定,在 §17.2 prose 加一行 normative |
| **`E0065` 退出码**(死锁检测返 E0065 时进程退出码) | **走 1**(与 `E0100` / `E0102` 同类未消费 ERR 退出码);与现有退出码表一致 |
| **通道关闭唤醒顺序**(多个 waiter 同时被唤醒时的顺序未定义) | **未定义,实现可任选**(显式标 "implementation-defined");v0.9.1 加 FIFO 选项(`wlwl.toml [channel_close_wakeup_fifo] true`);Step 11 prose 加 normative 段 |

### 11.5 spec 章节号冻结清单(草稿 2 启动;草稿 4 复核)

| spec 章节 | v0.8.1 状态 | v0.9 状态 | 冻结 |
|----------|-----------|---------|------|
| §13 OOP 关键字与对象模型 | 空 | v0.9 新增 | ✅ |
| §14 行为类型与会话类型协议 | 空 | v0.9 新增 | ✅ |
| §15 THIS 与线性 capability | 空 | v0.9 新增 | ✅ |
| §16 OOP 与并发的交互 | 空 | v0.9 新增 | ✅ |
| §17.8 公平性 / 可观测性 | (原 §17.7 限制表 7 行中的 2 行) | v0.9 上迁为独立小节 | ✅(草稿 4 确认) |
| §17.7 限制表 | 7 项 | 缩减为 7 项(移除 2 行迁移到 §17.8) | ✅ |
| §18+ | 不存在 | 不预留 | ✅ |

---

## 附录 A · 工业对照表(2026-09 截面)

| 维度 | Kotlin coroutines | Swift TaskGroup | Java StructuredTaskScope | Python asyncio.TaskGroup | wlwl v0.8 | wlwl v0.9 目标 |
|------|------------------|-----------------|------------------------|--------------------------|----------|---------------|
| 启动 | Lazy | Eager | Eager | Lazy | Lazy | Lazy |
| 挂起粒度 | 真 await | 真 await | 真 JEP 444 挂起点 | 真 await | **静态切段** | **真挂起** |
| 作用域 | coroutineScope { } | withTaskGroup { } | StructuredTaskScope.open() | async with TaskGroup() | SCOPE(fn) | SCOPE(fn) |
| 取消 | Job 树 + CooperativeCancellationException | CancellationError 经类型系统标注 | ShutdownOnFailure / ShutdownOnSuccess | CancelledError | TASK_CANCEL + Transient | 同 v0.8 |
| 阻塞挂起 | suspend 真挂起 | async 真挂起 | virtual thread 挂载 carrier | await 真挂起 | 返 WouldBlock | 真挂起 |
| 死锁检测 | 自定义 | 自定义 | Long Schedule 监测 | RuntimeWarning(`-W error`) | 不做 | L1 opt-in |
| 形式化基础 | — | — | — | — | — | Brown 9 + Smith 四不变量 |
| 引入年份 | 2018 | 2021 | 2024(JEP 462 第五预览)/ 2026(JDK 27 第七预览) | 2022(Python 3.11) | 2026.09(v0.7) | 2026.09+(v0.9) |

---

## 附录 B · 与 WasmFX 栈切换的对接(长期路径)

**WasmFX 当前状态**(2026-09):
- Phase 3 (Implementation),W3C/Bytecode Alliance;
- 主 champion:Francis McCabe(Google),Sam Lindley(Edinburgh);
- 仓库最新 commit 2026-05-27(PR #153);
- Wasm 3.0(2026-06-13)未包含栈切换;JSPI 已 Phase 5(标准化);
- Wasmtime 已实现,无浏览器生产。

**对接路径**(v0.10+):
1. wlwl impl 加 Wasm 后端(WasmGC 已 Phase 4,大部分基础在);
2. 把 v0.9 的调度循环翻译为 WasmFX 指令:
   - `eval_expr` 入口 = `cont.new`;
   - YIELD 触发 = `suspend` 到最近的 `resume` handler;
   - 调度器轮转 = `switch`;
3. 同一段 v0.9 `.wll` 源码在 Rust 后端与 Wasm 后端上语义一致;
4. ADR-0017 的工程实现(真挂起状态机)作为中间层,在 Wasm 后端直接对应 WasmFX 的延续语义。

**前置条件**:
- WasmFX 进入 Phase 4(预计 2027);
- Wasmtime / Wasmer 提供稳定的 Stack Switching flag;
- wlwl impl 加 Wasm 后端(独立议题,与并发无关)。

**不在 v0.9 议程**:仅作长期路线参考。

---

## 附录 C · 会话类型与结构化并发的形式化基础

**会话类型**(Honda, Yoshida, Carbone 2008):
- 为通信协议提供静态保证;
- 与线性逻辑对应(Caires, Pfenning 2002-2010);
- 工业实现:Rust(EnsembleS, sesh), Scala(lchannels, Effpi),OCaml(FuSe, session-ocaml),Swift(Swift Sessions),Haskell(Priority Sesh),ATS(原生)。

**结构化并发四不变量**(Smith 2018):
1. 任务形成树;
2. 父等子;
3. 错向上;
4. 取消向下。

**Brown 9 维空间**(Gray, Krishnamurthi, Crichton 2026):见 §7。

**v0.9 与上述三者的关系**:
- 四不变量:**已满足**(ADR-0014);
- 9 维空间:**v0.7 选定的子集不变**(§7);
- 会话类型:**v0.9 不引入到通道**(通道走 §3.2 真挂起的载荷路径);**v0.9 引入到 OOP 方法签名**(§4.4.3)。v0.10+ 才扩展到通道协议(需类型系统扩展)。

---

## 附录 D · 参考资料

### D.1 仓库内

| 路径 | 内容 |
|------|------|
| `docs/standard/wlwl-spec-v0.8.md` §17 | 并发(结构化并发与通道) |
| `docs/standard/wlwl-spec-v0.8.md` §17.7 | 限制与不承诺 |
| `docs/standard/wlwl-spec-v0.8.md` §11.2 | 错误码表(E0055/E0057 保留) |
| `docs/standard/wlwl-spec-v0.8.md` 附录 G | 内建注册表(OOP 标注未实现) |
| `docs/adr/0014-structured-concurrency-v0.7.md` | 结构化并发 + 通道选型 |
| `docs/adr/0015-brown-9-dimension-decision.md` | Brown 9 维度 v0.7 决议 |
| `docs/adr/0016-scheduler-single-thread-boundary.md` | 单线程边界 |
| `docs/history/deviations-v0.8.md` | v0.8 偏差登记(D8-001..D8-014) |
| `impl/crates/wlwl-eval/src/yield_split.rs` | 切段器(492 行,目标:重写) |
| `impl/crates/wlwl-eval/src/runtime.rs` | 调度器(718 行) |
| `impl/crates/wlwl-eval/src/channel.rs` | 通道(342 行) |
| `impl/crates/wlwl-eval/src/task.rs` | 任务(314 行) |
| `impl/crates/wlwl-eval/src/registry.rs` | 内建注册表(110 entries) |
| `docs/plan/README.md` | 当前迭代构建计划目录 |

### D.2 仓库外(本次综合研究)

#### D.2.1 结构化并发 / 异步设计空间

- Gray, G.; Krishnamurthi, S.; Crichton, W. *A Design Space Exploration of Async/Await*. **OOPSLA 2026 / PACMPL Vol. 10**, DOI: 10.1145/3839519,arXiv:2608.20677(Brown Cognitive Engineering Lab, 2026-08-21)。
- Smith, N. J. *Notes on structured concurrency, or: Go statement considered harmful*(2018-04-25)。
- Sústrik, M. *Structured Concurrency*(libdill, 2016-02-07)。
- Elizarov, R. *Structured concurrency*(Kotlin, 2018-09-12)。
- *LWN.net Weekly Edition 2026-04-30*(讨论 Erlang / Trio / Kotlin / Swift / Zig 结构化并发分歧)。

#### D.2.2 工业实现

- **Project Loom**(OpenJDK):JEP 444 (Virtual Threads, JDK 21, 2023),JEP 462 (Structured Concurrency Preview, JDK 22+),JEP 505 (JEP 525 / 533 / JDK 25-27 第七预览,预计 JDK 28 标准化)。
- **Kotlin Coroutines**:Elizarov, R. *Kotlin Coroutines: How suspend Compiles to a State Machine* / *CPS Transformation*(2017+,持续演进)。
- **Swift TaskGroup**:Apple Developer Documentation, *Concurrency*(Swift 5.5, 2021)。
- **Rust async/await**:Tokio Tutorial, *Async in depth*;Tokio 2.x 工作窃取调度器。
- **Python asyncio.TaskGroup**:PEP 654 / 789(Python 3.11+);Smith, N. J. *Trio*(2017+)。
- **BEAM / Erlang**:Armstrong, J. *Making reliable distributed systems in the presence of errors*(2003);OTP 28.0(2025-05)。

#### D.2.3 形式化

- Honda, K.; Yoshida, N.; Carbone, M. *Multiparty Asynchronous Session Types*(POPL 2008)。
- Caires, L.; Pfenning, F. *Session Types as Intuitionistic Linear Logic Propositions*(CONCUR 2010)。
- Hüttel, H. et al. *Foundations of Session Types and Behavioural Contracts*(ACM Comp. Surv. 2016)。
- Lindley, S.; Morris, J. G. *Lightweight Functional Session Types*(Links,Edinburgh)。

#### D.2.4 代数效应 / 效果处理器

- Leijen, D. *Koka: Functional Programming with Algebraic Effects and Effect Handlers*(2014+,Koka v3.x 2024-2026)。
- Pretnar, M.; Bauer, A. *An Introduction to Algebraic Effects and Handlers*(2015)。
- Sivaramakrishnan, K. *Multicore OCaml / OCaml 5 Effects*(2022,稳定于 2024)。
- *NII Shonan Seminar No. 146*(2024):Programming and Reasoning with Algebraic Effects and Effect Handlers。

#### D.2.5 WebAssembly Stack Switching

- McCabe, F.; Lindley, S. *WebAssembly Stack Switching Proposal*(Phase 3, 2026-05)。
- *Continuing WebAssembly with Effect Handlers (WasmFX)*(POPL 2024,arXiv 2308.08347)。
- *State of WebAssembly 2026*(Devnewsletter, 2026-Q2)。

---

## 附录 E · 术语表

| 术语 | 含义 |
|------|------|
| **Brown 9 维空间** | Gray 等 OOPSLA 2026 论文界定的 async/await 9 个独立设计维度 |
| **结构化并发** | Smith 2018 定义的四不变量范式(任务成树、父等子、错向上、取消向下) |
| **WasmFX** | WebAssembly Stack Switching Proposal,Wasm 一等延续提案 |
| **真挂起** | 调度循环在任务挂起点真正切换到其他任务,挂起任务在后续轮转时从原状态恢复 |
| **静态切段** | v0.7 / v0.8 路径:在 SPAWN 时把任务体切成多段,YIELD 是段边界 |
| **Session Type** | 为通道通信协议提供静态保证的类型系统扩展 |
| **Effect Handler** | Koka / OCaml 5 的核心机制,提供可恢复的非局部控制流 |
| **Reduction Budget** | BEAM 的抢占式调度单位(每 process 4000 reductions / 2000 per timeslice) |

---

> 本计划**等待用户审阅**。审阅点见 §9.2 决策点。批准后,Step 1 三件 ADR 先定稿,Step 2 开始动手改 impl。