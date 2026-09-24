# WLWL 语言规范 v0.9 (WIP — wip0.9 派生)

> **[WIP 状态]** 本文件是 `wip0.9` 分支的工作草案,基于 v0.8 增量派生;正式
> `v0.9.0` release commit 时再整理为正式版本。
>
> - 基础文本来自 `docs/standard/wlwl-spec-v0.8.md`(v0.8.1 patch 无 spec 改动)。
> - 本稿**只重写第 17 章**;其余章节以"沿用 v0.8"占位标注,具体内容见 `wlwl-spec-v0.8.md`
>   对应小节。
> - wip0.9 release commit 时:本文件改名为正式 `wlwl-spec-v0.9.md`,
>   原 `wlwl-spec-v0.8.md` 移入 `docs/history/`。

## 版本差异(v0.8 → v0.9 摘要)

1. **并发运行时从静态切段升级为真挂起**(plan §3.1 / §3.2 / §3.3):§17.1 / §17.2 修订;
   v0.7 / v0.8 spec §17.1 行 879 自承的限制同步解除。
2. **代数效果(algebraic-effect) framing 规范化**(plan §4.4.1 + ADR-0017 §3.1 +
   ADR-0019 §4.4.1):§17.4 新增 normative 段;`TaskState::Suspended { tag, reason }`
   内部命名落地(plan §9.1 Step 10 落地)。
3. **取消携带结构化 reason 载荷**(plan §4.4.2):§17.3 修订;
   `TASK_CANCEL(task, reason?)` / `TASK_CANCEL_PARENT(reason?)` 签名扩展;
   `reason` 非法类型 → 新错误码 `E0066`。
4. **`ChannelWouldBlock` 载荷路径移除**(plan §3.2):同步通道 `buf=0` 真挂起后
   无该触发路径;§17.2 / §17.7 同步修订。
5. **`E0055` / `E0057` 从错误码表移除**(ADR-0018 选项 B):关闭后 RECV 仍走
   `ERR(kind="ChannelClosed")` 载荷路径;跨任务不可变单元格继续走 `E0024`。
6. **死锁检测 L1 引入**(plan §3.4):§17.7 列出范围与触发条件,
   同 scope 内 ≥ 2 个 task 阻塞在 `ChannelOp` 且无法推进 → `W0065` /
   `E0065`(开发模式 / 生产模式)。
7. **`§17.8` 公平性 / 可观测性新增小节**(plan §3.5):v0.9 起 best-effort FIFO;
   在飞兄弟取消保证在下一个 YIELD 或挂起点可观测。

## 章节状态(v0.9 派生进度)

| 章节 | v0.9 状态 |
|------|----------|
| §0 导言 | 沿用 v0.8 |
| §1 词法结构 | 沿用 v0.8 |
| §2 类型与值 | 沿用 v0.8 |
| §3 变量、作用域与单元格 | 沿用 v0.8 |
| §4 表达式 | 沿用 v0.8 |
| §5 函数 | 沿用 v0.8 |
| §6 控制流 | 沿用 v0.8 |
| §7 模式匹配 | 沿用 v0.8 |
| §8 错误模型 | 沿用 v0.8 |
| §9 模块与程序 | 沿用 v0.8 |
| §10 标准库 | §10.11 沿用 v0.8(协议细节上提到 spec 的注记保留;具体行项 Step 11+ 单独补) |
| **§11 诊断** | **§11.2 / §11.3 增量已落地**(本稿) — 见下文 §11 字面 |
| §12 保留形式 | 沿用 v0.8 |
| §13-§16 OOP 真实实现 | **占位**,章节号冻结(Step 12 派生) |
| **§17 并发(algebraic-effect 模型)** | **本稿主体重写** — 见下文 |
| §18+ | 保留(未来) |
| 附录 A 文法 | 沿用 v0.8 |
| 附录 B 与早期草案差异 | 沿用 v0.8 |
| 附录 C 参考实现 | 沿用 v0.8 |
| 附录 D v0.7 / v0.8 / v0.9 摘要 | **本稿新增 v0.9 增量** |
| 附录 E / F | 保留(沿用 v0.8 注) |
| **附录 G 全局内建注册表** | **本稿嵌入 wip0.9 状态实际产物**(Step 11 子阶段 2 完成) — 见下文附录 G |

## 0 — 12 章节沿用说明

§0-§10 + §12 的具体内容文本**与 v0.8 完全一致**,本稿只在章节状态表中标"沿用 v0.8"。
请直接参阅 `docs/standard/wlwl-spec-v0.8.md` 对应小节(行号见 §17.7 引用清单)。

**§11 诊断章节(§11.2 错误码表 / §11.3 警告码表)在 v0.9 由本稿修订**,见下文 §11 字面。
**§10.11** 修订内容:为 `wlwl:std.ai` / `wlwl:std.agent` 协议细节增加最小正式契约行(plan §5),
wip0.9 阶段沿用 v0.8 §10.11 文本(协议细节上提到 spec 的注记保留),
具体行项 Step 11 + Step 13 派生期间单独补。

**§12** 表格中 v0.8 列出的"OOP 行"(`/ §17` 列表项)在 v0.9 中保留,
具体移除 / 替换由 Step 12 OOP 真实实现后一并处理。

## 11 诊断(v0.9 修订)

> **[v0.9 增量章节]** 本章 §11.1 / §11.4 沿用 v0.8 文本不动;
> §11.2 错误码表 / §11.3 警告码表由本稿修订。其余诊断格式与退出码与 v0.8 §11 一致。

### 11.1 结构

(沿用 v0.8 §11.1)诊断是错误(`E`)或警告(`W`),带四位数字码。错误形式的 CLI 输出包含
码、消息、位置(文件、行列)与源行摘录;`--format json`/`jsonl` 产生机器可读结构(含
`error_schema_version`、`errorCategory`、`retryable`、`idempotent`、`suggestion_code`、
`trace` 等字段)。

### 11.2 错误码

**(沿用 v0.8 / 跨章节同等段落)**:v0.8 §11.2 表中标注的 `E0001`/`E0002`/`E0003`
… 一直到 `E1003` 的所有基本错误码**与 v0.8 完全一致**,不再在本稿重复列举;
本节仅列出 v0.9 删除 / 新增 / 重命名的条目。

#### 11.2.1 v0.9 移除的错误码

| 码 | v0.7 / v0.8 状态 | v0.9 决议(ADR-0018 选项 B,plan §3.6 / §9.2) |
|----|------------------|------------------------------------------|
| `E0055` | 保留(v0.7 / v0.8 无触发路径,deviation D8-003):通道关闭信号由 `ERR(kind="ChannelClosed")` 字典载荷承担(§8.1 / §17.2) | **从 §11.2 表移除**;关闭后 RECV / TRY_RECV 仍走 `ERR(kind="ChannelClosed")` 载荷路径;`wlwl.toml` 设 `[native_channel_close] true` 可启用原生码(opt-in,用于强类型捕获场景) |
| `E0057` | 保留(v0.7 / v0.8 无触发路径,deviation D8-003):跨任务不可变单元格实际触发 `E0024`(§17.4) | **从 §11.2 表移除**;跨任务与单任务的不可变单元格错误统一走 `E0024`(与单任务路径一致) |

#### 11.2.2 v0.9 新增的错误码

| 码 | 含义 | 触发点 | 触发条件 |
|----|------|--------|----------|
| **[v0.9]** `E0065` | 结构化并发死锁检测 L1(顶层) | `Scheduler::detect_deadlock_l1` 在 scope 内 ≥ 2 个 task 阻塞在 `ChannelOp` 且无任何推进时 | 同 scope 内 `Suspended { tag: ChannelOp, .. }` task 数 ≥ 2,且 run queue 已排空,无法推进任何 task |
| **[v0.9]** `E0066` | 任务取消 `reason` 字段非法类型 | `TASK_CANCEL(task, reason)` 或 `TASK_CANCEL_PARENT(reason)` 收到非 DICT 类型的 reason | plan §4.4.2 + §3.3 同步:`r != DICT` → `E0066`;类型校验在 arity / 句柄校验之后 |

> **死锁检测 L1 触发模式**(继 plan §3.4 / §9.2):
> - **开发模式**(默认):触达条件时仅发 `W0065` 软警告(参见 §11.3),不抛错误;
> - **生产模式**:`wlwl.toml` 设 `[strict_deadlock_detect] true` → 升级为顶层错误 `E0065`;
> - **P0 内暂缓条件**:若 L1 在 §3.2 落地后引入 bug 数 > 3,降级为开发模式 opt-in,
>   v0.9.1 再做(plan §3.4 风险表)。
> - **不跨 scope**;**不检测显式 `Yield` 互让**(两 task 互让但不互发不算死锁);
>   **单点通道挂起不算死锁**(plan §3.4 严格范围)。

#### 11.2.3 §17.1 YIELD 位置 E0014 触发路径解除(继 §17.1.6)

v0.7 / v0.8 spec §17.1 行 879 自承"YIELD 位置限制是 v0.7 / v0.8 实现路径的产物",
`E0014` 在并发下追加用途是"v0.9 不再适用":v0.9 起 §11.2 表中 `E0014` 行的触发
范围**只剩** `RETURN` / `BREAK` / `CONTINUE` 出现在非法位置。YIELD 位置错误
(`LET(x, YIELD())` / `IF(cond, YIELD(), 42)` / 数组字面量内部 / 间接调用等)
在 v0.9 起**均合法**,不再触发任何错误码。

#### 11.2.4 v0.9 兼容性

- v0.8 程序**不**依赖 `E0055` / `E0057` 触发路径 → 在 v0.9 上行为不变;
- v0.8 程序依赖 `ChannelWouldBlock` ERR 载荷 → 同步通道 SEND / RECV 在 v0.9 真挂起后
  **不再**返该载荷(详 §17.2.6);迁移方式:用 `TRY_SEND` / `TRY_RECV` 替代即可;
- v0.8 `TASK_CANCEL(task)` 不带 reason 的旧语法 → v0.9 仍合法,reason 隐式 `{}`;
- v0.8 `AWAIT` 已取消任务返 `ERR(kind="Cancelled")` → 在 v0.9 仍合法,新增可选
  `reason` 字段(详 §17.3.2),旧消费代码按 `kind` 匹配仍兼容。

### 11.3 警告码

v0.9 §11.3 警告码表沿用 v0.8 原有 8 行(`W0010` / `W0011` / `W0014` / `W0020` /
`W0030` / `W0040` / `W0051` / `W0053`),**新增** 1 行:

| 码 | 含义 |
|----|------|
| **`W0065`**(新增) | 结构化并发死锁检测 L1 软警告:同 scope 内 ≥ 2 个 task 阻塞在 `ChannelOp` 且无任何推进时,开发模式下发出该警告(默认行为);生产模式由 `wlwl.toml` 设 `[strict_deadlock_detect]` 升级为 `E0065` 顶层错误(参见 §11.2.2)。**警告不改变程序语义**,只是提示开发模式进入堆栈排查;`check` 子命令与解析期诊断流会输出该警告,运行期警告进入诊断流。 |

警告不得改变程序语义(沿用 v0.8 §11.3 末段);`check` 子命令与解析期诊断流会输出
警告,运行期警告进入诊断流,由实现决定呈现时机。

### 11.4 退出码

(沿用 v0.8 §11.4)退出码与 v0.8 一致:

| 退出码 | 含义 |
|--------|------|
| `0` | 成功 |
| `1` | 以诊断终止(语法错误、`E0100` PANIC、`E0102` 顶层 `ERR`、`E0065` 死锁检测等) |
| `101` | 实现内部崩溃(非规范性;出现即属实现缺陷) |

> v0.9 新增 `E0065` 退出码路径:`Scheduler::detect_deadlock_l1` 触发后,CLI 退出码为 `1`,
> 与其他错误码路径一致;`--format json` 输出的 `errorCategory` 字段值为 `"deadlock"`。

## 13 — 16 OOP 真实实现占位

> **§13-§16 章节号在 v0.9.0 release tag 上冻结**(ADR-0019 §4.4.3 草稿 2 采纳)。
> v0.9.0 派生时(`Step 12` 阶段)将填充:
>
> - **§13 OOP 关键字与对象模型**(`CLASS` / `NEW` / `THIS` / `GET_PROP` / `SET_PROP` /
>   `CALL_METHOD` 等),对象字典构造器 + `NEW` 实例化 + 字段可变性;
> - **§14 行为类型与会话类型协议**,含 v0.9.0 落地集:
>   - 顺序:完整支持;
>   - 选择:`⊕` 内部选择(调用方决定);
>   - 递归:μ 骨架完整支持(`{rec: μX. { get: ?int.X, close: end }}`);
>   - 外部选择 `&` / 命名协议 `typealias` / 并行 `par` 留 v0.9.1+。
>   `CALL_METHOD` 走协议状态机,违规 → `E0051`。
> - **§15 `THIS` 与线性 capability**,Wadler 1990 风格实质检查:不可存入容器 /
>   不可跨 call 边界 / 不可跨 `AWAIT` 边界,越界 → `E0032`;
> - **§16 OOP 与并发的交互**(`CALL_METHOD` 内的 `YIELD` / `CHANNEL_*`),
>   algebraic-effect framing 与 §17 共享术语。

附录 G"标记 `OOP 未实现` 占位条目"在 Step 12 填入后全部迁移到真实现条目。
本占位章节号不会改变;v0.9.1 只能追加 sub-section(§14.1 / §14.2 / ...),
不重新编号。

## 17 并发(结构化并发与通道 — algebraic-effect 模型)

> **[v0.9 重写章节]** 取代 v0.7 / v0.8 §17 静态切段版本。
>
> 本章把并发控制流建模为 **algebraic effects**,调度器担当 effect handler,与 Koka /
> OCaml 5 / WasmFX Phase 3 术语对齐。实现层面:
> - 任务的协作让步(`YIELD()`)≡ `perform Yield` effect;
> - 任务的取消(`TASK_CANCEL(t, reason)`)≡ `raise Cancelled { reason }` effect;
> - 同步通道 SEND / RECV 在满/空触发 `perform ChannelOp` effect。
>
> impl 内部命名约定:`TaskState::Suspended { tag: Tag, reason: YieldReason }`
> (plan §9.1 Step 10);`Signal::Yield(YieldReason)` 是 eval-stack 层的别名信号,
> 保留向后兼容路径;`Effect` 枚举(`Yield { explicit } | ChannelOp { dir, channel,
> value? } | Cancelled { reason: Dict }`)覆盖三个 v0.9 effect tag。
>
> 第 8 章错误模型(含 §8.2 透明传播、§8.3 消费者注册表)对本章全部内建**原样适用**。

### 17.0 设计摘要

v0.9 沿用 v0.7 / v0.8 的**结构化并发 + 通道**,但在 effect 层面把控制流统一为三类:

| 用户语言构造 | effect 形式 |
|--------------|-------------|
| `YIELD()` | `perform Yield { explicit: true }` |
| `TASK_CANCEL(t, reason)` | `raise Cancelled { reason }` |
| `CHANNEL_SEND(ch, v)` | `perform ChannelOp { dir: Send, channel, value: Some(v) }` |
| `CHANNEL_RECV(ch)` | `perform ChannelOp { dir: Recv, channel, value: None }` |

调度器(`Scheduler::step`)担当 effect handler 循环:捕获 effect 后把任务
从 `Running` 翻为 `Suspended { tag, reason }`,把控制流切到下一个 runnable
task;当前 task 留在对应 wait list(通道 SEND/RECV / 任务 AWAIT);满足前置
条件时,`Suspended { tag, reason } → Running`,`eval_expr` 从挂起点 continuation
继续。挂起点位置对外**透明**(§17.4)。

Brown 9 维度(ADR-0015,plan §2.4,v0.7 选定):

> Lazy / Dynamic-Suspension / Dynamic-Extent / Strong / Awaited / Destructive /
> Aware / Top-Down / **Transient**。

**Suspension 一项** 由 v0.7 / v0.8 的"动态调度 / 静态切段"升级为"动态调度 /
真挂起"(plan §3.1);其余 8 项维度定义**不变**,仅工程实现改变。

### 17.1 任务与作用域

`SCOPE(fn)` / `SPAWN(fn)` / `AWAIT(task)` / `TASK_CURRENT()` / `TASK_IS_CANCELLED()`
语义与 v0.8 §17.1 兼容;`YIELD()` 行为重大变化,见下。

#### 17.1.1 `YIELD` 位置 — 真挂起(plan §3.1)

`YIELD()` 在 v0.9 中**可在任何表达式位置**合法出现:

| 构造 | v0.7 / v0.8 | v0.9 |
|------|-------------|------|
| `LET(x, YIELD())` | E0014 | OK — 挂起后继续,`x` 取 YIELD 后的值(若引用 x,则为 `NULL`) |
| `IF(cond, YIELD(), 42)` | E0014 | OK — 视 cond 走分支 |
| `[1, YIELD(), 3]` | E0014 | OK — `[1, NULL, 3]` |
| `LET(y, YIELD); y()` | 静默运行(无挂起) | **自动挂起**(间接调用等价为直接 `YIELD()`) |
| `WHILE(c, ...YIELD()...)` | 不恢复嵌套体 | **恢复嵌套体剩余** |
| `FOR(x, ...YIELD()...)` | 不恢复嵌套体 | **恢复嵌套体剩余** |
| `IF(c, ...YIELD()...)` 分支 | 不恢复嵌套体 | **恢复嵌套体剩余** |

`YIELD()` 在挂起点的语义是 `perform Yield { explicit: true }` — 调度器捕获该
effect,把任务状态从 `Running` 翻为
`Suspended { tag: Tag::Yield, reason: YieldReason::Explicit }`,
选下一个 runnable task;恢复时
`Suspended { tag: Tag::Yield, reason: YieldReason::Explicit } → Running`,
直接重新进入 YIELD 之后的中间表示。

#### 17.1.2 顶层 YIELD

顶层 `YIELD();` 后跟表达式 — v0.9 隐式合法(已隐式支持)。

#### 17.1.3 嵌套 YIELD(plan §3.3)

嵌套构造(`WHILE` / `FOR` / `IF` 分支等)内的 YIELD 在挂起 → 恢复后,**继续
执行嵌套体剩余部分**,而不是继续外层 `Block` 的下一段。该限制是 v0.7 / v0.8
切段器(`yield_split.rs`)的实现路径限制,v0.9 真挂起后自动消除 — 不需新增
实现代码,只是 §3.1 真挂起落地后该限制自动消失(plan §3.3 字面)。

#### 17.1.4 作用域与生命周期(沿用 v0.7 / v0.8)

- 子任务寿命不超过词法 `SCOPE`;`SCOPE` 返回前等待其子任务结束(结构化并发不变量)。
- **无隐式 runtime scope**:任何 `SPAWN` 必须直接或间接嵌套在某个 `SCOPE(...)`
  内;顶层 `SPAWN` → `E0058`。不提供 free-floating task / 守护线程。
- `SCOPE` 的返回值是 `fn` 主体的值;若 `fn` 主体给出未消费的 `ERR`,
  该 `ERR` 成为 `SCOPE` 的返回值(与 §8.2 一致);已被消费者处理掉的 `ERR`
  **不**再上浮。

#### 17.1.5 AWAIT / 句柄语义(沿用 v0.8)

- 任务以用户 `ERR` 结束时,`AWAIT` **返回该 `ERR` 值**(不自动消费)。
- 宿主诊断(实现内部错误,如 `E0101` 栈溢出 / 底层通道句柄失效 `E0053` 等)
  在 `AWAIT` 处重新抛出,使用 `WlwlError` 的 trace 链把目标任务的失败帧
  关联起来;**用户 `ERR` 值**(`RESULT` 变体)按 §17.5 作为普通结果返回,
  不重新抛出。
- 已取消任务的 `AWAIT` 在 v0.9 起返回
  `ERR(kind="Cancelled", reason: <dict>)`(见 §17.3,新增 `reason` 字段;
  v0.7 / v0.8 旧消费代码按 `kind` 匹配仍兼容)。
- 非句柄 / 过期句柄 → `E0053`。

#### 17.1.6 v0.9 解除的旧限制

v0.7 / v0.8 spec §17.1 行 879 自承:

> "该限制是 v0.7 / v0.8 实现路径的产物,不是语言语义规则;未来版本(挂起式调度)
> 可放宽而不视为破坏性修订。"

v0.9 是显式允许放宽的"未来版本",该段在 v0.9 不再适用 — YIELD 位置限制
**完全解除**(`E0014` 不再触发该路径;`E0014` 注册文本仅保留于 RETURN /
BREAK / CONTINUE 出现在非法位置)。

### 17.2 通道

#### 17.2.1 构造与签章(沿用 v0.7 / v0.8 §17.2,兼容)

| 构造 | 签名 | 语义 |
|------|------|------|
| `CHANNEL_NEW(buf)` | `-> CHANNEL` | 创建缓冲大小为 `buf` 的通道;`buf = 0` 为同步通道。`buf` 须为非负 `INTEGER`。 |
| `CHANNEL_SEND(ch, v)` | `-> NULL` | 发送。**真挂起**:无空位且无 peer 时挂起任务于 `sender_waiters`;关闭后发送 → `E0054`。 |
| `CHANNEL_RECV(ch)` | `-> v` | 接收。**真挂起**:缓冲空且无 peer 时挂起任务于 `receiver_waiters`;关闭且读空 → `ERR(kind="ChannelClosed")`。 |
| `CHANNEL_TRY_SEND(ch, v)` | `-> BOOLEAN` | **不挂起**:成功 `TRUE`,满 `FALSE`;关闭后 → `E0054`。 |
| `CHANNEL_TRY_RECV(ch)` | `-> v / NULL` | **不挂起**:成功返回值;空返回 `NULL`(**不是**关闭信号);关闭且读空 → `ERR(kind="ChannelClosed")`。 |
| `CHANNEL_CLOSE(ch)` | `-> NULL` | 关闭。幂等。 |
| `CHANNEL_LEN(ch)` | `-> INTEGER` | 当前缓冲长度。 |
| `CHANNEL_CAP(ch)` | `-> INTEGER` | 配置容量;同步通道为 `0`。 |

#### 17.2.2 真挂起同步通道(plan §3.2)

`buf=0` 同步通道的 `CHANNEL_SEND` / `CHANNEL_RECV` 在满/空时**真挂起**:

1. 若有 peer waiter → `ChannelWaiter::pair`,双方状态翻 `Running`,进入下一轮调度;
2. 若 channel 有空位(SEND) / 缓冲非空(RECV) → 走缓冲路径,立即返回;
3. 否则 → push 当前 task 到 `Channel::sender_waiters` 或 `receiver_waiters`,
   状态翻为
   ```
   Suspended {
       tag: Tag::ChannelOp,
       reason: YieldReason::SendingOn(channel) | YieldReason::ReceivingOn(channel),
   }
   ```
   移出 run queue。

`CHANNEL_TRY_SEND` / `CHANNEL_TRY_RECV` **保持非阻塞**(v0.7 起即有,plan §3.2)。

#### 17.2.3 操作语义矩阵(v0.9)

| 操作 | closed | buf | 行为 | 备注 |
|------|--------|-----|------|------|
| SEND | false | 有空位 | 入队 | 不变 |
| SEND | false | 满 | **挂起**(`Suspended { tag: ChannelOp, reason: SendingOn }`,加入 `sender_waiters`) | **v0.9 改** |
| SEND | true | — | `E0054` | 不变 |
| RECV | false | 非空 | 出队 | 不变 |
| RECV | false | 空 | **挂起**(`Suspended { tag: ChannelOp, reason: ReceivingOn }`,加入 `receiver_waiters`) | **v0.9 改** |
| RECV | true | 空 | `ERR(kind="ChannelClosed")` | 不变 |
| RECV | true | 非空 | 先出队,读空后下一 RECV 得 `ChannelClosed` | 不变 |
| CLOSE | false | — | `closed = true` | 不变 |
| CLOSE | true | — | 幂等 no-op | 不变 |

#### 17.2.4 关闭协议(沿用 v0.7 / v0.8)

1. `CHANNEL_CLOSE(ch)` 置 `closed = true`。
2. 关闭后 `SEND` / `TRY_SEND` → `E0054`。
3. 关闭后 `RECV` / `TRY_RECV` 在缓冲读空后返回
   `ERR(kind="ChannelClosed", channel: <handle 显示>)`。
4. `NULL` **不得**用作关闭信号;**必须**用 `IS_ERR` + `ERR_PAYLOAD` 判定。

#### 17.2.5 v0.9 新增:关闭后 waiter 行为(plan §3.2)

`CHANNEL_CLOSE` 后,通道的 `sender_waiters` / `receiver_waiters` 列表被清空;所有
waiter 在 state machine 中翻为 `Cancelled`,后续调度观察到
`ERR(kind="Cancelled")`(而非 `ChannelClosed` — 后者只给已经成功 enter 的读)。

> 这与 v0.7 / v0.8 的"关闭通知"路径一致,只是 v0.9 把它从语义约束升级为
> 实现层的明确强制。

#### 17.2.6 v0.9 移除:旧的 `ERR(kind="ChannelWouldBlock")` 触发路径

v0.8 §17.2 阻塞挂起行(v0.7)同步移除。`ERR(kind="ChannelWouldBlock")` 在 v0.9
**没有**触发路径(同步通道真挂起后,只有 `TRY_*` 仍可能返回 boolean `FALSE`,
**不**通过 ERR 载荷)。

`TYPE(ch)` 为 `"CHANNEL"`。句柄恒等见 §2.4。

### 17.3 取消(tag/payload cancellation,plan §4.4.2)

#### 17.3.1 构造与签章(扩展)

| 构造 | v0.8 签名 | v0.9 签名 | 变化 |
|------|----------|-----------|------|
| `TASK_CANCEL(task)` | `-> NULL` | `TASK_CANCEL(task, reason?) -> NULL` | reason 缺省 `{}`(空 DICT);**reason 必须是 DICT 类型,其它类型 → `E0066`** |
| `TASK_CANCEL_PARENT()` | `-> NULL` | `TASK_CANCEL_PARENT(reason?) -> NULL` | 同上 |
| `SHIELD(fn)` | `SHIELD(fn) -> v` | 不变 | v0.8 已确立 |

#### 17.3.2 Advisory → structured

- `TASK_CANCEL(task, reason)` 携带结构化 `reason` 载荷;
- 在实现内部,**取消**触发 effect `raise Cancelled { reason: Dict }`,
  对应 `TaskState` 转移
  `Running → Suspended { tag: Tag::Cancelled, reason: ... }`(在 v0.9 内部命名下);
- 在 SCHEDULE 循环下一次调度时,`Cancelled` 状态翻为 terminal `Cancelled`;
- 用户层面 ERR 载荷:`ERR(kind="Cancelled", reason: <dict>)`,与 v0.7
  `kind: "Cancelled"` 兼容,**新增** `reason: Dict` 字段;
- AWAIT 已取消任务时,`payload.reason` 直接透传用户传入的 dict,
  不再丢弃。

#### 17.3.3 SHIELD 块内行为

1. SHIELD 块内 `TASK_CANCEL_PARENT(reason)` 仅记录挂起取消与 reason;
2. 最外层 SHIELD 退出后,reason 透传到任务下一个检查点(`TASK_IS_CANCELLED` /
   `YIELD` / 阻塞操作);
3. SHIELD 自身的 `ERR` 不屏蔽(沿用 v0.8 §17.3);

#### 17.3.4 取消与错误

- 取消本身**不**生成用户 `ERR` 值(仅在 AWAIT 观察点产生
  `ERR(kind="Cancelled", reason)`)。
- 取消自顶向下传播;`Persistence` = **Transient**(协作检查点响应,不强制打断)。
- 观察取消用 `TASK_IS_CANCELLED()`,reason 通过 `ERR_PAYLOAD(err).reason` 取得。

#### 17.3.5 v0.9 兼容性

`TASK_CANCEL(task)` / `TASK_CANCEL_PARENT()` 不带 reason 的旧语法在 v0.9 仍合法,
reason 隐式为 `{}`(空 DICT);旧消费代码(`IS_ERR` + `ERR_PAYLOAD(err).kind
== "Cancelled"` 匹配)继续工作。

### 17.4 代数效果执行模型(新 normative 段,plan §4.4.1 + ADR-0017 §3.1 + ADR-0019 §4.4.1)

本章并发原语建模为 algebraic effects,术语与 Koka / OCaml 5 / WasmFX Phase 3
对应:

- **Koka**(Leijen 2014+,v3.x 2024-2026):algebraic effect + effect handler 模型;
- **OCaml 5 / Multicore OCaml**(Sivaramakrishnan 2022,稳定于 2024):
  effect system 扩展;
- **WasmFX Phase 3**(McCabe, Lindley 2026-05):`tag + payload` 模型,
  `suspend` 指令族。

#### 17.4.1 用户语言构造 → effect 形式

| 用户语言构造 | effect 形式 |
|--------------|-------------|
| `YIELD()` | `perform Yield { explicit: true }` |
| `TASK_CANCEL(t, reason)` | `raise Cancelled { reason }` |
| `CHANNEL_SEND(ch, v)` | `perform ChannelOp { dir: Send, channel, value: Some(v) }` |
| `CHANNEL_RECV(ch)` | `perform ChannelOp { dir: Recv, channel, value: None }` |

#### 17.4.2 调度器担当 effect handler

- 任务从 `Running` 状态发出 effect;
- 调度器的 step 循环捕获 effect,`TaskState::Suspended { tag, reason }`,其中:
  - `tag` 来自 effect 分类(`Yield` / `ChannelOp` / `Cancelled`);
  - `reason` 是 effect 携带的载荷(`YieldReason` enum variant);
- 把控制流切到下一个 runnable task,当前 task 留在 `wait list`;
- 满足前置条件时,从 `Suspended { tag, reason } → Running`,
  `eval_expr` 从挂起点的 continuation 继续。

#### 17.4.3 实现内部命名(`wlwl-eval/src/runtime.rs`)

```rust
enum TaskState {
    Pending,
    Running,
    Suspended {
        tag: Tag,               // Tag::Yield | Tag::ChannelOp | Tag::Cancelled
        reason: YieldReason,    // effect 携带的载荷
    },
    Done(Box<TaskResult>),
    Cancelled,
}

enum Tag { Yield, ChannelOp, Cancelled }

enum YieldReason {
    Explicit,                   // YIELD() perform Yield
    AwaitingChild(TaskId),      // AWAIT(t) 等待子任务结束
    ReceivingOn(RcHandle),      // CHANNEL_RECV 挂起
    SendingOn(RcHandle),        // CHANNEL_SEND 挂起
}

enum Effect {
    Yield { explicit: bool },
    ChannelOp { dir: Direction, channel: RcHandle, value: Option<Value> },
    Cancelled,                  // v0.9.0 默认无显式 reason(结构化 reason 透传在 Err 载荷层)
}
```

- `Signal::Yield(YieldReason)` 是 eval-stack 层(impl `wlwl-eval/src/lib.rs`)到
  `TaskState::Suspended` 的桥接信号;
- 与 effect 体系有 1-to-1 映射(plan §4.4.1 保留向后兼容路径);
- ADR-0019 §4.4.3 规划的 `Effect::MethodCall` / `Effect::PropAccess` /
  `Effect::ProtocolViolation` 留待 v0.9 OOP 章节(§13-§16)派生。

#### 17.4.4 间接 YIELD 自动挂起

`LET(y, YIELD); y()` 形态中,`y()` 作为函数调用,在 effect-handler 模型下每次
调用都 perform `Yield`,自动挂起 — 与直接 `YIELD()` 等价。无额外实现成本。

#### 17.4.5 v0.10+ 对齐

若 wlwl 引入 Wasm 后端,`TaskState::Suspended { tag, reason }` 与 WasmFX
`suspend $tag_*` 一一映射:

| wlwl `Effect` | WasmFX `tag` | WasmFX `suspend` |
|---------------|--------------|------------------|
| `Yield { explicit }` | `$tag_yield` | `suspend $tag_yield` |
| `ChannelOp { Send, ... }` | `$tag_channel_send` | `suspend $tag_channel_send` |
| `ChannelOp { Recv, ... }` | `$tag_channel_recv` | `suspend $tag_channel_recv` |
| `Cancelled` | `$tag_cancelled` | `suspend $tag_cancelled` |

v0.9 不暴露用户自定义 effect-handler;用户构造 `perform` / `handle` 留 v0.10+。
但术语与内部命名已对齐,届时用户构造与内部一致。

### 17.5 与第 8 章的关系

- 本章全部 17 个内建**均不是** §8.3 消费者:实参求值为 `ERR` 时按 §8.2 透明
  传播,函数体不执行。
- `AWAIT` **返回**子任务的 `ERR` 作为普通结果值;是否消费由调用方按 §8.3
  决定。
- 跨任务边界后,`ERR` 载荷(含 `kind` 与 v0.9 新增的 `reason`)保持原值。
- 未消费的 `ERR` 到达顶层仍为 `E0102`(§8.5)。

> **v0.9 行为变更(继 v0.8 §17.5 注记)**:
>
> 1. `SCOPE(ERR("x"))` 中实参 `ERR("x")` 透明传播后,`E0052` 仅在参数被识别为
>    非函数时触发(沿用 v0.8);
> 2. v0.9 移除 §17.1 关于 `E0014` 在 YIELD 位置的触发路径(见 §17.1.6);
> 3. v0.9 移除 `ERR(kind="ChannelWouldBlock")` 触发路径(见 §17.2.6);
> 4. v0.9 `Cancelled` 载荷新增 `reason` 字段(见 §17.3)。

> **v0.8 SCOPE / SHIELD / yield-split 偏差处理**:v0.8 spec §17.4 [实现偏差,
> 非规范性注记] 关于闭包调用可升级单元格的历史行为沿用
> (`deviations-v0.8.md` 有专门条目;v0.9 release 时迁移到 `deviations-v0.9.md`,
> Step 14)。

### 17.6 标准库

- v0.9 **不**新增 `wlwl:std.*` 模块实现(plan §2.5 议程外);
- 并发原语全部为全局内建(附录 G"并发 / 通道"组);
- `wlwl:std.ai` / `wlwl:std.agent` 的最小正式契约见 §10.11(Step 11 子阶段 2
  落实,WIP)。

### 17.7 限制与不承诺(v0.9 重写)

基于 plan §1.2 决议路径逐项改写(继 v0.8 表)。

| 项 | v0.9 状态 | 说明 |
|----|-----------|------|
| 线程模型 | **单线程协作调度** | ADR-0016 边界不动 |
| 性能 | 单任务路径相对 v0.6 退化 < 10%;并发路径只保证正确性 + 无泄漏 | 不变 |
| 公平性 | best-effort FIFO(v0.9 起尝试) | 详见 §17.8 |
| 延迟 / 吞吐 | **不**承诺 | 不变 |
| **阻塞挂起** | **真挂起**:`buf=0` 同步通道的 SEND / RECV 在满/空时挂起 task;`TRY_*` 保持非阻塞 | **v0.9 改**(plan §3.2) |
| **死锁检测** | **L1 默认启用**(同 scope 内 ≥ 2 个 task 阻塞在 `ChannelOp`,且任一 task 上无法推进);开发模式 `W0065` 警告,生产模式由 `wlwl.toml` 设 `[strict_deadlock_detect]` 转 `E0065` | **v0.9 引入**(plan §3.4) |
| **嵌套 YIELD** | **已解除**:嵌套构造内 YIELD 恢复后继续执行嵌套体剩余部分;旧限制是 v0.7 / v0.8 切段器实现路径 | **v0.9 改**(plan §3.3) |
| 尾部 YIELD | 任务体末尾的 `YIELD()` 使任务以 `NULL` 值结束 | 不变 |
| **在飞兄弟取消** | **保证在下一个 YIELD 或挂起点可观测** | **v0.9 改**(plan §3.5) |
| 隐式 scope | 不提供(ADR-0014) | 不变 |
| **`ChannelWouldBlock` 载荷** | **已移除**(无 v0.9 触发路径) | **v0.9 改**(plan §3.2) |
| 关闭后 RECV 行为 | `ERR(kind="ChannelClosed")` 载荷,与 v0.7 / v0.8 一致 | 不变 |
| **`E0055` 错误码** | **已从 §11.2 表移除**(ADR-0018 选项 B);关闭后 RECV 仍走 `ERR(kind="ChannelClosed")` 载荷路径;`wlwl.toml` 设 `[native_channel_close] true` 可启用原生码(opt-in) | **v0.9 改** |
| **`E0057` 错误码** | **已从 §11.2 表移除**(ADR-0018 选项 B);跨任务不可变单元格继续走 `E0024`(与单任务路径统一) | **v0.9 改** |
| 死锁检测 L1 严格范围 | **不跨 scope**;**不检测显式 `Yield` 互让**(两 task 互让但不互发不算死锁);**单点通道挂起不算死锁** | plan §3.4 严格限制 |
| P0 内死锁检测假阳性门槛 | L1 引入 bug > 3 项时,降级为开发模式 opt-in | plan §3.4 暂缓条件 |
| Effect handler 边界 | v0.9 不暴露用户自定义 effect-handler(留 v0.10+) | plan §4.4.1 |

> **v0.7 / v0.8 spec §17.1 行 879 自承的限制** 在 v0.9 起**不**作为限制列出
> — v0.9 是显式允许放宽的"未来版本"(见 §17.1.6)。

### 17.8 公平性 / 可观测性(v0.9 新增小节,plan §3.5)

| 维度 | v0.9 承诺 | 工程实现 |
|------|-----------|----------|
| 调度公平性 | best-effort FIFO:YIELD 检查点间隔下,同优先级 task 按入队次序轮转 | 调度循环从"任意 runnable"改为"按入队次序的 FIFO 队列"(plan §3.5) |
| `ChannelWaiter::pair` 顺序 | sender 与 receiver 都标 `Running` 后,加入 run queue 尾部 | 避免一对抢先多个 |
| 在飞兄弟取消可观测性 | 保证在下一个 `YIELD` 或 `Suspended` 检查点(通道 SEND / RECV / `TASK_IS_CANCELLED`)可观测中断 | 每轮调度前扫描:`Suspended(Cancelled)` 标 `Cancelled`,对应 waiter 清空 |
| Effect 内部循环可见 | 每轮调度前扫描:`Suspended(Cancelled)` 标 `Cancelled`,对应 waiter 清空 | 与上同 |

> v0.9 起,v0.8 spec §17.7 中"**公平性 不承诺**" 与 "**在飞兄弟取消**" 两行
> 迁入本节;v0.9 §17.7 表中相应行保留该承诺方向,但具体工程约束在本节定义。
>
> v0.9 spec 章节编号变更(2026-09-24 草稿 3 第三方审计采纳):
> - v0.8 规范 §17 范围 §17.0 - §17.7;
> - v0.9 规范**新增 §17.8 公平性 / 可观测性小节**(原 §17.7 限制表 7 项中
>   关于公平性 / 在飞兄弟取消两行,迁入 §17.8 单独 normative 段);
> - v0.9 规范**§17.7 限制表**内容相应缩减(移除已上迁至 §17.8 的两行,保留
>   线程模型 / 性能 / 延迟-吞吐 / 阻塞挂起 / 死锁检测 / 嵌套 YIELD / 尾部 YIELD
>   / 在飞兄弟取消 / 隐式 scope / `ChannelWouldBlock` / `E0055` / `E0057` 等共 12+ 行)。

---

## 18+ 保留(未来章节)

§18+ 为 v0.10+ 远期议题预留:

- §18 Wasm 后端 + WasmFX Phase 4 编译(plan §2.5 议程外,长期路径);
- §19 用户自定义 effect-handler 暴露(`perform` / `handle`)— 计划 v0.10+;
- §20+ 会话类型全集(命名协议 / 并行 `par` / 外部选择 `&` / 跨任务消息协议)
  — 计划 v0.10+。

## 附录 A 文法 — 沿用 v0.8 §A.1 + §A.2 + §A.3

参见 `docs/standard/wlwl-spec-v0.8.md` 第 995-1067 行。本稿未派生。

## 附录 B 与早期草案差异 — 沿用 v0.8 §B

参见 `docs/standard/wlwl-spec-v0.8.md` 第 1069-1091 行(18 条差异)。本稿未
派生。

## 附录 C 参考实现 — 沿用 v0.8 §C

参见 `docs/standard/wlwl-spec-v0.8.md` 第 1093-1104 行。本稿未派生。

## 附录 D v0.7 / v0.8 / v0.9 追加摘要

v0.7 / v0.8 部分沿用 v0.8 spec §1108-1123。**本节新增 v0.9 增量**;v0.8 的 1108-1123
行内容见 v0.8 spec 对应位置。

### D.v0.9 增量

1. **规范章节**:§17 整体重写 — algebraic-effect 描述模型;新增 §17.4
   normative 段、§17.8 公平性 / 可观测性小节;
2. **章节号冻结规则**:§13 / §14 / §15 / §16 章节号在 v0.9.0 release tag
   上冻结(ADR-0019 草稿 2);
3. **错误码**:
   - `E0055` / `E0057` 从 §11.2 表移除(ADR-0018 选项 B);
   - `E0065`(死锁检测 L1 顶层错误码)与 `W0065`(开发模式软警告)新增;
   - `E0066`(取消 reason 非法类型)新增。
4. **`ERR` kind**:
   - `ChannelWouldBlock` 触发路径删除(无 v0.9 路径);
   - `Cancelled` 新增 `reason: Dict` 字段(继 tag/payload cancellation);
   - `ChannelClosed` 行为不变。
5. **章节**:§17 章节号与 v0.7 维持(v0.9.0 release tag 上冻结);
6. **Brown 9 维度**:维持 v0.7 选定的 9 项,Suspension 由"动态切段"升格为
   "动态真挂起"(plan §3.1);
7. **代数效果内部命名落地**:`TaskState::Suspended { tag: Tag, reason: YieldReason }`
   取代 v0.7 / v0.8 的 tuple variant `Suspended(YieldReason)`(plan §9.1 Step 10
   落地);
8. **附录 G**:**TBD** Step 11 子阶段 2 重生成;预计 v0.9.0 release 时总计
   ~110 - 2(`E0055` / `E0057`)+ 2(`E0065` / `E0066`)= ~110 条。

### D.v0.9 与 v0.8 兼容承诺

- 任何 v0.8 程序若不依赖 v0.9 移除的 `E0055` / `E0057` 触发路径,且不依赖
  `ChannelWouldBlock` 载荷,在 v0.9 上行为不变;
- `TASK_CANCEL(task, reason)` 旧语法 `TASK_CANCEL(task)` 仍合法,reason
  隐式 `{}`;
- v0.8 spec §17.4 [实现偏差,非规范性注记] 关于闭包调用可升级单元格的历史
  行为在 v0.9 不变(详见 `deviations-v0.9.md`,Step 14 启动)。

## 附录 E / F — 保留(沿用 v0.8)

参见 `docs/standard/wlwl-spec-v0.8.md` 第 1123 行 + §E / §F 占位注。本稿未派生。

## 附录 G 全局内建注册表(规范性)

> **[v0.9 wip0.9 重生成]** 本附录由 `wlwl-eval::registry::generate_appendix_g_md()`
> 在 v0.9 wip0.9 状态下自动生成(单源真相:
> `impl/crates/wlwl-eval/src/registry.rs::BUILTIN_REGISTRY`)。
>
> **wip0.9 状态说明**:
> - 错误码触发路径层面(§11.2 / §11.3)已对齐 v0.9(`E0055` / `E0057` 移除 +
>   `E0065` / `E0066` / `W0065` 新增全部锁测试覆盖,见 `wlwl-eval` Step 5 / 6 / 8);
> - 内建**签名**层面(`TASK_CANCEL(task, reason?)` / `TASK_CANCEL_PARENT(reason?)`
>   / `CHANNEL_SEND` 真挂起等):在 wip0.9 中已通过组合内建 + 任务调度验证,
>   **但 registry 的"签名文本"字符串截至 wip0.9 子阶段 2 时尚未同步** —
>   本附录签位列**仍按 v0.8 字面**展示(参见下方表内 `CHANNEL_SEND` 等行的 `ERR(ChannelWouldBlock)`);
> - **后续 Step 12+ 完成 BUILTIN_REGISTRY 签名字符串 / 结构规范化时**再跑一次
>   `cargo run --bin gen-appendix-g -- ../docs/appendix_G.md` 重生成,届时
>   同步更新此处。
>
> 重生成命令:`cargo run --bin gen-appendix-g -- ../docs/appendix_G.md`。
> 修改流程:改注册表 → 跑本函数重写本文件 → 跑 `cargo test` 验证 lock test。
>
> 遮蔽保护(§3.5)以本表登记名为准。

总条目数:**110** | 已实现:**86** | LexerMacro:**24** | Deferred:**0**


| 名称 | 签名 | ERR 消费者 (§8.3) | 宏函数 (§1.4) | 引入 | 状态 | 实现位置 |
|------|------|--------------------|---------------|------|------|----------|
<!-- I/O (3 条) -->
| `PRINT` | `PRINT(args...) -> NULL` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§10.2) |
| `PRINT_ERR` | `PRINT_ERR(args...) -> NULL` | ❌ | ❌ | v0.4 | ✓ builtin | `resolve_builtin` (§10.2) |
| `INPUT` | `INPUT(prompt?) -> STRING` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§10.2) |
<!-- 类型 / 转换 (7 条) -->
| `LEN` | `LEN(coll) -> INTEGER` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§10.3) |
| `STR` | `STR(x) -> STRING` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§10.3) |
| `INT` | `INT(s) -> OK(INTEGER) / ERR(ParseError)` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§10.3) |
| `FLOAT` | `FLOAT(s) -> OK(FLOAT) / ERR(ParseError)` | ❌ | ❌ | v0.4 | ✓ builtin | `resolve_builtin` (§10.3) |
| `TYPE` | `TYPE(x) -> STRING (RESULT -> "RESULT")` | ✔ | ✔ | v0.2 | ✓ builtin | `resolve_builtin` (§2.5) |
| `BOOL` | `BOOL(x) -> BOOLEAN` | ✔ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§2.2) |
| `CALL` | `CALL(fn, args...) -> v` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§8.3) |
<!-- RESULT 处理 (12 条) -->
| `IS_OK` | `IS_OK(x) -> BOOLEAN` | ✔ | ✔ | v0.2 | ✓ macro | parser -> `Expr::*` (§8.3) |
| `IS_ERR` | `IS_ERR(x) -> BOOLEAN` | ✔ | ✔ | v0.2 | ✓ macro | parser -> `Expr::*` (§8.3) |
| `OR_DIE` | `OR_DIE(x, default) -> v (v0.3 alias)` | ✔ | ✔ | v0.2 | ✓ compat (W0051/W0054) | `resolve_builtin` (compat, §8.3) |
| `UNWRAP_OR` | `UNWRAP_OR(x, default) -> v` | ✔ | ✔ | v0.4 | ✓ builtin | `resolve_builtin` (§8.3) |
| `UNWRAP` | `UNWRAP(x) -> v / PANIC` | ✔ | ❌ | v0.4 | ✓ builtin | `resolve_builtin` (§8.3) |
| `ERR_PAYLOAD` | `ERR_PAYLOAD(x) -> e / E0030` | ✔ | ❌ | v0.4 | ✓ builtin | `resolve_builtin` (§8.3) |
| `WRAP` | `WRAP(err, ctx) -> ERR / OK` | ✔ | ❌ | v0.4 | ✓ builtin | `resolve_builtin` (§8.3) |
| `TRY` | `TRY(e) -> v / early-RETURN` | ✔ | ✔ | v0.2 | ✓ macro | parser -> `Expr::*` (§6) |
| `PANIC` | `PANIC(msg) -> 终止` | n/a | ✔ | v0.2 | ✓ macro | parser -> `Expr::*` (§8.4) |
| `OK` | `OK(v) -> RESULT` | ❌ | ✔ | v0.4 | ✓ macro | parser -> `Expr::*` (§8.1) |
| `EXPECT_ERR` | `EXPECT_ERR(expr) -> OK(payload) / ERR(E0049)` | ✔ | ✔ | v0.4 | ✓ macro | parser -> `Expr::*` (§8.3) |
| `ERR` | `ERR(e) -> RESULT` | ❌ | ✔ | v0.4 | ✓ macro | parser -> `Expr::*` (§8.1) |
<!-- 控制流 / 逻辑 (10 条) -->
| `IF` | `IF(cond, t, e?) -> v` | ✔ | ✔ | v0.2 | ✓ macro | parser -> `Expr::*` (§6) |
| `WHILE` | `WHILE(cond, body) -> v` | ❌ | ✔ | v0.2 | ✓ macro | parser -> `Expr::*` (§6) |
| `FOR` | `FOR(var, iter, body) -> NULL` | ❌ | ✔ | v0.2 | ✓ macro | parser -> `Expr::*` (§6) |
| `MATCH` | `MATCH(v, clauses, default?) -> v` | ❌ | ✔ | v0.4 | ✓ macro | parser -> `Expr::*` (§6) |
| `RETURN` | `RETURN(v?) -> 早返` | n/a | ✔ | v0.2 | ✓ macro | parser -> `Expr::*` (§6) |
| `BREAK` | `BREAK() -> 跳出` | n/a | ✔ | v0.2 | ✓ macro | parser -> `Expr::*` (§6) |
| `CONTINUE` | `CONTINUE() -> 跳到下轮` | n/a | ✔ | v0.2 | ✓ macro | parser -> `Expr::*` (§6) |
| `AND` | `AND(a, b) -> BOOLEAN (short-circuit)` | ❌ | ✔ | v0.2 | ✓ macro | parser -> `Expr::*` (§4.3) |
| `OR` | `OR(a, b) -> BOOLEAN (short-circuit)` | ❌ | ✔ | v0.2 | ✓ macro | parser -> `Expr::*` (§4.3) |
| `NOT` | `NOT(a) -> BOOLEAN (取反)` | ❌ | ✔ | v0.2 | ✓ builtin | `resolve_builtin` (§4.3) |
<!-- 运算符 (14 条) -->
| `==` | `=(a, b) -> BOOLEAN / ERR 透传` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§4.3) |
| `!=` | `!(a, b) -> BOOLEAN / ERR 透传` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§4.3) |
| `>` | `>(a, b) -> BOOLEAN / ERR 透传` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§4.3) |
| `<` | `<(a, b) -> BOOLEAN / ERR 透传` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§4.3) |
| `>=` | `>=(a, b) -> BOOLEAN / ERR 透传` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§4.3) |
| `<=` | `<=(a, b) -> BOOLEAN / ERR 透传` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§4.3) |
| `+` | `+(a, b) -> INTEGER / FLOAT` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§4.3) |
| `-` | `-(a, b) -> INTEGER / FLOAT` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§4.3) |
| `*` | `*(a, b) -> INTEGER / FLOAT` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§4.3) |
| `/` | `/(a, b) -> INTEGER / FLOAT` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§4.3) |
| `%` | `%(a, b) -> INTEGER` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§4.3) |
| `&&` | `&&(a, b) -> BOOLEAN (v0.6 §4.3 short-circuit)` | ✔ | ❌ | v0.6 | ✓ builtin | `resolve_builtin` (§4.3) |
| `||` | `||(a, b) -> BOOLEAN (v0.6 §4.3 short-circuit)` | ✔ | ❌ | v0.6 | ✓ builtin | `resolve_builtin` (§4.3) |
| `NEG` | `NEG(a) -> -a` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§4.3) |
<!-- ARRAY 操作 (8 条) -->
| `PUSH` | `PUSH(arr, x) -> ARRAY` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§10.4) |
<!-- DICT 操作 (8 条) -->
| `POP` | `POP(d, k, default) -> v (v0.6 compat alias for AT_K; signature kept 3-arg)` | ❌ | ❌ | v0.6 | ✓ compat (W0051/W0054) | `resolve_builtin` (compat, §10.4) |
| `AT_K` | `AT_K(d, k, default) -> v (v0.6 §10.4)` | ❌ | ❌ | v0.6 | ✓ builtin | `resolve_builtin` (§10.4) |
<!-- ARRAY 操作 (8 条) -->
| `SHIFT` | `SHIFT(arr) -> ARRAY` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§10.4) |
| `UNSHIFT` | `UNSHIFT(arr, x) -> ARRAY` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§10.4) |
| `SLICE` | `SLICE(arr, start, end?) -> ARRAY` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§10.4) |
| `CONCAT` | `CONCAT(a, b) -> ARRAY` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§10.4) |
| `CONTAINS` | `CONTAINS(arr, x) -> BOOLEAN` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§10.4) |
| `INDEX` | `INDEX(arr, x) -> INTEGER / -1` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§10.4) |
| `REVERSE` | `REVERSE(arr) -> ARRAY` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§10.4) |
<!-- DICT 操作 (8 条) -->
| `REMOVE_KEY` | `REMOVE_KEY(dict, k) -> DICT` | ❌ | ❌ | v0.4 | ✓ builtin | `resolve_builtin` (§10.4) |
| `DEL` | `DEL(dict, k) -> DICT (v0.3 alias, W0051)` | ❌ | ❌ | v0.2 | ✓ compat (W0051/W0054) | `resolve_builtin` (compat, §10.4) |
| `KEYS` | `KEYS(dict) -> ARRAY` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§10.4) |
| `VALUES` | `VALUES(dict) -> ARRAY` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§10.4) |
| `HAS` | `HAS(dict, k) -> BOOLEAN` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§10.4) |
| `MERGE` | `MERGE(a, b) -> DICT` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§10.4) |
<!-- 下标 (3 条) -->
| `INDEX_GET` | `INDEX_GET(coll, k) -> v / E0031` | ❌ | ❌ | v0.4 | ✓ builtin | `resolve_builtin` (§4.5) |
| `INDEX_SET` | `INDEX_SET(coll, k, v) -> NULL` | ❌ | ❌ | v0.4 | ✓ builtin | `resolve_builtin` (§4.5) |
| `AT` | `AT(coll, i) -> v` | ❌ | ❌ | v0.4 | ✓ builtin | `resolve_builtin` (§4.5) |
<!-- STRING 操作 (15 条) -->
| `UPPER` | `UPPER(s) -> STRING` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§10.5) |
| `LOWER` | `LOWER(s) -> STRING` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§10.5) |
| `SUB` | `SUB(s, start, end?) -> STRING` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§10.5) |
| `REPLACE` | `REPLACE(s, old, new) -> STRING` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§10.5) |
| `SPLIT` | `SPLIT(s, sep) -> ARRAY` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§10.5) |
| `TRIM` | `TRIM(s) -> STRING` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§10.5) |
| `TRIM_START` | `TRIM_START(s) -> STRING` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§10.5) |
| `TRIM_END` | `TRIM_END(s) -> STRING` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§10.5) |
| `STARTS_WITH` | `STARTS_WITH(s, pre) -> BOOLEAN` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§10.5) |
| `ENDS_WITH` | `ENDS_WITH(s, suf) -> BOOLEAN` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§10.5) |
| `REPEAT` | `REPEAT(s, n) -> STRING` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§10.5) |
| `PAD_START` | `PAD_START(s, n, c?) -> STRING` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§10.5) |
| `PAD_END` | `PAD_END(s, n, c?) -> STRING` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§10.5) |
| `CODEPOINTS` | `CODEPOINTS(s) -> ARRAY` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§10.5) |
| `FROM_CODEPOINTS` | `FROM_CODEPOINTS(arr) -> STRING` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§10.5) |
<!-- 格式化 (1 条) -->
| `FORMAT` | `FORMAT(template, args...) -> STRING` | ❌ | ❌ | v0.4 | ✓ builtin | `resolve_builtin` (§10.7) |
<!-- 模块系统 (4 条) -->
| `MODULE_REF` | `MODULE_REF(path) -> MODULE` | ❌ | ❌ | v0.4 | ✓ builtin | `resolve_builtin` (§9) |
| `EXPORT` | `EXPORT(names) -> NULL` | n/a | ✔ | v0.2 | ✓ macro | parser -> `Expr::*` (§9) |
| `IMPORT` | `IMPORT(path, names, opts?) -> NULL` | n/a | ✔ | v0.2 | ✓ macro | parser -> `Expr::*` (§9) |
| `MODULE` | `MODULE(name?, body) -> NULL` | n/a | ✔ | v0.2 | ✓ macro | parser -> `Expr::*` (§9) |
<!-- OOP (3 条) -->
| `CLASS` | `CLASS(name?, parent, members) -> CLASS` | n/a | ✔ | v0.2 | ✓ macro | parser -> `Expr::*` (§13) |
| `NEW` | `NEW(cls, args...) -> INSTANCE` | n/a | ✔ | v0.2 | ✓ macro | parser -> `Expr::*` (§13) |
| `THIS` | `THIS -> 当前实例` | n/a | ✔ | v0.2 | ✓ macro | parser -> `Expr::*` (§15) |
<!-- 属性 / 方法 (3 条) -->
| `GET_PROP` | `GET_PROP(obj, k) -> v / E0037` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§13) |
| `SET_PROP` | `SET_PROP(obj, k, v) -> NULL` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§13) |
| `CALL_METHOD` | `CALL_METHOD(obj, m, args...) -> v` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§13) |
<!-- 构造器 (2 条) -->
| `ARRAY` | `ARRAY(items...) / ARRAY()` | ❌ | ✔ | v0.2 | ✓ macro | parser -> `Expr::*` (§10.9) |
| `DICT` | `DICT(pairs...) / DICT()` | ❌ | ✔ | v0.2 | ✓ macro | parser -> `Expr::*` (§10.9) |
<!-- 并发 / 通道 (17 条) -->
| `SCOPE` | `SCOPE(fn) -> v` | ❌ | ❌ | v0.7 | ✓ builtin | `resolve_builtin` (§17.1) |
| `SPAWN` | `SPAWN(fn) -> TASK` | ❌ | ❌ | v0.7 | ✓ builtin | `resolve_builtin` (§17.1) |
| `AWAIT` | `AWAIT(task) -> v` | ❌ | ❌ | v0.7 | ✓ builtin | `resolve_builtin` (§17.1) |
| `YIELD` | `YIELD() -> NULL` | ❌ | ❌ | v0.7 | ✓ builtin | `resolve_builtin` (§17.1) |
| `TASK_CURRENT` | `TASK_CURRENT() -> TASK` | ❌ | ❌ | v0.7 | ✓ builtin | `resolve_builtin` (§17.3) |
| `TASK_IS_CANCELLED` | `TASK_IS_CANCELLED() -> BOOLEAN` | ❌ | ❌ | v0.7 | ✓ builtin | `resolve_builtin` (§17.3) |
| `TASK_CANCEL` | `TASK_CANCEL(task) -> NULL` | ❌ | ❌ | v0.7 | ✓ builtin | `resolve_builtin` (§17.3) |
| `TASK_CANCEL_PARENT` | `TASK_CANCEL_PARENT() -> NULL` | ❌ | ❌ | v0.7 | ✓ builtin | `resolve_builtin` (§17.3) |
| `SHIELD` | `SHIELD(fn) -> v` | ❌ | ❌ | v0.7 | ✓ builtin | `resolve_builtin` (§17.3) |
| `CHANNEL_NEW` | `CHANNEL_NEW(buf) -> CHANNEL` | ❌ | ❌ | v0.7 | ✓ builtin | `resolve_builtin` (§17.2) |
| `CHANNEL_CLOSE` | `CHANNEL_CLOSE(ch) -> NULL` | ❌ | ❌ | v0.7 | ✓ builtin | `resolve_builtin` (§17.2) |
| `CHANNEL_SEND` | `CHANNEL_SEND(ch, v) -> NULL / ERR(ChannelWouldBlock)` | ❌ | ❌ | v0.7 | ✓ builtin | `resolve_builtin` (§17.2) |
| `CHANNEL_RECV` | `CHANNEL_RECV(ch) -> v / ERR(ChannelClosed|ChannelWouldBlock)` | ❌ | ❌ | v0.7 | ✓ builtin | `resolve_builtin` (§17.2) |
| `CHANNEL_TRY_SEND` | `CHANNEL_TRY_SEND(ch, v) -> BOOLEAN` | ❌ | ❌ | v0.7 | ✓ builtin | `resolve_builtin` (§17.2) |
| `CHANNEL_TRY_RECV` | `CHANNEL_TRY_RECV(ch) -> v / NULL / ERR(ChannelClosed)` | ❌ | ❌ | v0.7 | ✓ builtin | `resolve_builtin` (§17.2) |
| `CHANNEL_LEN` | `CHANNEL_LEN(ch) -> INTEGER` | ❌ | ❌ | v0.7 | ✓ builtin | `resolve_builtin` (§17.2) |
| `CHANNEL_CAP` | `CHANNEL_CAP(ch) -> INTEGER` | ❌ | ❌ | v0.7 | ✓ builtin | `resolve_builtin` (§17.2) |

---

> **wip0.9 阶段引用清单**(v0.8 spec 行号,便于 review diff):
>
> - §11.2 / §11.3(v0.8 spec 第 744-800 行)—— §11.2 错误码表 / §11.3 警告码表 v0.9 修订见本稿 §11 字面;
> - §17.0-17.7(v0.8 spec 第 854-991 行)— 第 17 章整体重写;
> - §17.8(v0.9 新增小节);
> - §17.4 / §17.7 引用 v0.8 spec 第 962 行(实现偏差注记);
> - 附录 D(v0.8 spec 第 1108-1123 行)—— v0.9 增量已独立撰写,见上文 D.v0.9 节;
> - 附录 G(v0.8 spec 第 1127+ 行 + `docs/appendix_G.md` wip0.9 重生成 12922 字节)
>   —— 已嵌入本稿;签名字符串 wip 状态说明见附录 G 头部注释。
