# wlwl v0.12.0_rc1 第三方测试报告核实 + 修复计划

> **本文件是工作文档，不是发布说明。**
> 状态标记四态：**已核实**（附原始读数）/ **已复现**（附复现命令）/ **已裁决**（规范有明文）/ **待裁决**（需业主拍板）。
> **每条事实自带证伪命令**；命令红了以命令为准，不要以本文的结论为准。

- 对象：`wlwl 0.12.0`（`v0.12.0_rc1` 快照 + `wlwl-spec-v1 v1.0.2`）
- 报告来源：`C:/Users/ikrx2/Desktop/wlweb/REPORT.md`（独立测试方，2026-10-25）
- 核实日期：2026-10-25
- 规范基线：本仓 `docs/wlwl-spec-reference.md` ＝ 已发布规范 `Laffinty/wlwl-spec-v1` v1.0.2（tag `v1.0.2` ⇒ 提交 `294dc07`）的合并册抄件

---

## §0 格式契约（怎么读、怎么改）

1. **§2 的每一条**都带三样东西：**报告怎么说** / **实测是什么** / **怎么自己复现**。
   复现命令在 `impl/` 与 `docs/` 之下都能跑；命令的读数是权威，本文的转述不是。
2. **判定只有四档**：
   - **成立且是缺陷** —— 实现偏离了规范或自身契约 ⇒ 进 §3 修复批次。
   - **现象成立，归因错** —— 现象可复现，但报告给的机制与修法不对 ⇒ 进 §2，并在 §5 把正确机制写进 skill（**这是本次最危险的一类**：照报告的修法改会做无用功甚至引入回归）。
   - **规范明文如此** —— 实现符合规范 ⇒ 不改代码，进 §5 写成纪律。
   - **待裁决** —— 规范有歧义 ⇒ 不动代码，登记待业主/规范仓拍板。
3. **改这份文档时**：新增条目必须带「命令 + 读数」，**不得**只写结论；
   状态从「待裁决」改到别的档时，在条目末尾追加一行 `裁决：<谁> <日期>`。
4. **本文件不替代规范**：规范与实测冲突时走一致性册 §5 的差异登记流程（实现仓
   `script/conformance_registry_map.json` 必须同步加行），**不得**在本文件里悄悄改口径。

---

## §1 结论摘要

**报告的 8 条实现级发现里，0 条可以照报告的修法直接改。**

| 报告条目 | 本次判定 | 一句话 |
|---|---|---|
| A1 发送后对端收不到，要到进程退出 | **现象成立，归因错** | 真因是**并发 `SLEEP` 阻塞整个运行时**，不是缺 flush；`write_all` 无用户态缓冲，`flush()` 是空操作 ⇒ 报告的修法是**空操作** |
| A2 就绪事件被丢弃 ⇒ 白等满超时 | **现象成立，归因错** | 实测**事件没丢**（两次都唤醒成功）；真因是**段首重放重跑了 `TCP_ACCEPT`，它再次挂起** ⇒ 违反并发册 §17.0 第 2 条「不得让它再次挂起」 |
| A3 段首重放重做副作用 ⇒ 泄漏 5 个 socket | **成立，但是规范明文** | 并发册 §17.0 第 1 条明写「副作用不回滚、不承诺恰好一次」；底层缺口项目已登记（addendum-12 §0.1(b)） |
| A4 `SCOPE` 不等孙任务 ⇒ 客户端只看到 RST | **不是缺陷** | 并发册 §17.3 第 6 条明写「作用域退出时仍非终态的子任务被**强制取消**」 |
| A5 「父任务等多个子任务」无干净表达 | **不是缺陷，是已知语言缺口** | `AWAIT` 已结束句柄 ⇒ `E0053` 是 §17.1 明文；多子任务汇合与 A3 同源，受 ADR-0017 Step 3 阻塞 |
| B1 `SCOPE` 是否等孙任务 | **规范已答** | §17.3 第 6 条（见 A4）；残余只是措辞 |
| B2 「重放会不会重复副作用」规范没说 | **规范已答** | §17.0 第 1 条明写「已经发生的副作用**不**回滚；实现**不**承诺『恰好一次』」 |
| C1 `PRINT` 在管道重定向时全缓冲 | **不成立** | `std_print` 用 `println!` ⇒ Rust stdout 是 `LineWriter` ⇒ **行缓冲**；实测重定向到文件时 `FIRST` 在进程存活期间即出现 |
| C2 任务失败报告延迟到进程退出 | **规范明文如此** | 并发册 §17.9 + 「两条路径互斥」⇒ 失败先挂起、收尾时报 |

**另发现 4 条报告没写、且确属实现缺陷的问题（§3）**，其中 **N2 是对并发册 §17.0 第 2 条的实打实违反**，
**N1 是数据丢失**。

---

## §2 逐条核实

### 2.1 A1 —— 发送后对端收不到（现象成立，归因错）

**报告说**：`TCP_SEND` 后字节要等到进程退出才对端可见；建议在 `wlwl-std/src/tcp.rs` 的
`tcp_send`（`write_all` 之后）加显式 flush / `TCP_NODELAY`。

**实测**：现象可复现（`probe/send_only.py` ⇒ 3015 ms），但**真因与发送无关**。

| 变体 | 结构 | 实测 |
|---|---|---|
| A1a | 与报告同构：另起一个任务 `SLEEP(3000)` | `accept_done=3003` `send_done=3003` —— **accept 本身**到 3003 ms 才完成 |
| A1b | 只把 `SLEEP` 挪到发送**之后** | `accept_done=53` `send_done=53`，客户端 **82 ms** 收到字节 |

⇒ 差别不在「发」上，在**谁先占住解释器线程**：`std.time::SLEEP` 是**阻塞原语**
（并发册 §17.0「与阻塞的区别」段：「`SLEEP` **不**是挂起点：它**阻塞整个调度器**」），
它跑起来时解释器不去处理 socket 就绪 ⇒ accept 无法完成 ⇒ 发送自然被推迟到 `SLEEP` 结束。
A1a 的 3003 ms 与 `SLEEP(3000)` 几乎相等，就是这个原因。

**⇒ 报告的修法是空操作**：Rust 的 `TcpStream::write_all` 直接下 `write` 系统调用，
**没有用户态缓冲**；`TcpStream::flush()` 是 `Ok(())` 的空实现。`TCP_NODELAY` 解决的是
Nagle 延迟（小包合并），与「整段数据滞留到进程退出」无关。

**复现**：
```powershell
cd D:\Project\wlwl\.scratch
python probe.py a1\a1_sleep_task.wll --connect   # send_done=3003
python probe.py a1\a1_sleep_after.wll --connect   # send_done=53
```

**处置**：不改代码。**正确机制写进 skill**（§5）：并发代码里禁用 `SLEEP`，用 `SLEEP_ASYNC`。

---

### 2.2 A2 —— 就绪事件被丢弃（现象成立，归因错）

**报告说**：`pump_socket_readiness` 里 `for sid in take_ready()` 会把「目标任务尚未 park」
的事件**永久丢弃**；建议加 per-socket pending 标志。

**实测**：**事件没有丢**。在 `wake_socket_waiter` 与 `pump_socket_readiness` 上临时加追踪后
（追踪已回滚）：

```
[262ms] pump drain2 sid=1      wake=成功（找到了 parked 的等待者）
[315ms] pump drain2 sid=2      wake=成功
[5345ms] error[E0030]: TCP_RECV: expected integer, got null
```

两次唤醒**都成功**。真正的失败链是：

1. 任务体（**不切段**，无 `YIELD`）在 `TCP_ACCEPT` 上挂起 ⇒ 恢复时**整段重跑**（并发册 §17.0 明文）。
2. 重跑 ⇒ `TCP_ACCEPT` **再次执行**：这次连接已在 `pending` 队列里 ⇒ 返回句柄 ✓。
3. 继续跑到 `TCP_RECV` ⇒ 挂起（客户端还没发数据）。
4. 数据到达 ⇒ 唤醒 ⇒ **再次整段重跑** ⇒ `TCP_ACCEPT` **第三次执行** ⇒ `pending` 已空 ⇒
   **它又挂起了** ⇒ 违反并发册 §17.0 第 2 条「实现**不得**让它再次挂起（否则任务原地活锁）」。
5. 于是 accept 把自己的 5000 ms 预算耗尽 ⇒ 超时返回 `NULL` ⇒ `conn` 变成 `NULL` ⇒
   `TCP_RECV(conn, …)` 拿到 `NULL` ⇒ **`E0030`**（实测 5345 ms ≈ 5000 + 抖动）。

**⇒ 报告的机制描述（事件被丢弃）与修法（pending 标志）都指向了错误的层**：
即便把事件保住，accept 该重挂还是重挂。**真正的缺陷是 §17.0 第 2 条被违反** ⇒ 见 §3 的 **N2**。

**⇒ 但报告的 pending 标志想法本身有价值**：就绪事件在「无等待者」时被丢弃，会带来
最多 50 ms 的额外延迟（`pump_socket_readiness` 的轮询粒度），并且在 accept 循环里会
吃掉监听 socket 的那一次就绪通知 ⇒ 见 §3 的 **N3**。

**复现**：
```powershell
cd D:\Project\wlwl\.scratch
python probe.py a2\a2_no_yield.wll --connect --timeout 6   # 5 s 后 E0030
python probe.py a2\a2_double_yield.wll --connect            # recv_wait=0（切段路径正常）
```

---

### 2.3 A3 —— 段首重放重做副作用（成立，但是规范明文）

**报告说**：挂起点之前的副作用会被重做 ⇒ 5 次 `TCP_LISTEN` 留下 5 个 LISTENING socket 且句柄被覆盖、无法关闭。

**实测**：**复现**（`probe/test_leak.py` ⇒ 峰值 5 个 LISTENING）。

**但这是规范明文**：并发册 §17.0 第 1 条：
> **当前顶层语句被整体重跑。** …… 已经发生的副作用**不**回滚；实现**不**承诺「恰好一次」。

§15.5 也早就写明「任何**非分段**任务体一旦被挂起再唤醒，重入走**整段重跑**（绑定从头重来、副作用重放）」。

**⇒ 不是实现缺陷**，实现是合规的。真正的缺口在更下面一层：**挂起后原地续跑需要保存续体**，
而并发册 §17.0 末段明写「本规范**不保存续体**……兑现续体保存需要改运行期栈模型，
是一项独立改造，**不**在本规范范围内」。实现仓对此已有登记：
`docs/active/addendum-12-sync-primitives.md` §0.1(b)、W-02 / W-03 标 ⛔ **受阻**，
**解锁条件单一**：`ADR-0017` Step 3（真 state-machine 续跑）。

**⇒ 本次不改这段语义**（改它 = 动规范 + 动运行时栈模型 = ADR-0017 Step 3 的范围，
且 §17.0 第 1 条的「不承诺恰好一次」会被推翻 ⇒ 需要规范仓同步改文本）。

**⇒ 报告的 7 条纪律价值很高**，但其中多数是**规范纪律**而非实现 bug ⇒ 进 §5（写进 skill）。

---

### 2.4 A4 —— `SCOPE` 不等孙任务（不是缺陷）

**规范明文**：并发册 §17.3 第 6 条：
> **作用域退出的硬清理豁免**：`SCOPE` 返回前的强制清理……作用域退出时仍非终态的子任务被**强制取消**。

孙任务在词法 `SCOPE` 内 ⇒ 作用域退出时它被强制取消 ⇒ 客户端看到 RST ⇒ **符合规范**。

**⇒ 报告的 B1（「是否覆盖孙任务」）已被 §17.3 第 6 条回答**：覆盖，且处置方式就是「强制取消」，
不是「等待」。§17.1 那句「`SCOPE` 返回前**必须**等待其子任务结束」在 §17.3 第 6 条处被具体化。

**⇒ 不改代码**；把这条写进 skill（**服务端要让 handler 跑完，必须自己计数，不能指望 `SCOPE` 等**）。

---

### 2.5 A5 —— 父任务等多个子任务（不是缺陷，是已知缺口）

- `AWAIT` 一个**已结束**的句柄 ⇒ `E0053`：**§17.1 明文**（「非句柄 / **过期句柄** → `E0053`」）⇒ 合规。
- 「父任务等**多个**子任务全部完成」无干净表达：与 A3 同源（需要「等 N 个信号」这种汇合原语，
  依赖 ADR-0017 Step 3 的段内恢复）⇒ addendum-12 W-02 / W-03 同一条解锁条件。

**⇒ 不改代码**；写进 skill（**替代写法**：完成计数 + 共享 cell + 让 `accept` 超时当节拍）。

---

### 2.6 C1 —— `PRINT` 全缓冲（不成立）

`impl/crates/wlwl-std/src/io.rs` 的 `std_print` 用的是 `println!` ⇒ Rust 的 `Stdout` 是
`LineWriter` ⇒ **行缓冲**（管道/文件重定向下同样如此）。

**实测**（重定向到文件，进程仍在运行时查看文件内容）：
```
--- content at t=700ms (process still running) ---
FIRST
--- content after exit ---
FIRST
SECOND
```

**⇒ 报告观察到的「日志行在进程退出时才一次性出现」是 A1 的副作用**（程序本身晚到那一步才打印），
不是缓冲问题。

**⇒ 不改代码**。

---

### 2.7 C2 —— 任务失败报告延迟到收尾（规范明文）

并发册 §17.9「任务失败的报告义务」要求：无人 `AWAIT` 的任务失败**必须**被报告到 stderr 一行；
被 `AWAIT` 的失败仍在调用点抛出且**不重复报告**。为了保证「两条路径互斥」，
**判定必须延后到程序收尾**（失败发生的那一刻判不准「有没有人等」）。

实现侧同一决策写在 `impl/crates/wlwl-eval/src/lib.rs`（`pending_orphan_failures`）。

**⇒ 不改代码**；写进 skill（**不要把「失败报告出现得晚」当成没有报告**）。

---

## §3 报告没写、但确属实现缺陷的四条

### N1 · `tcp_recv` / `tcp_accept` 先读「已超时」再读缓冲 ⇒ **数据丢失**

**位置**：`impl/crates/wlwl-std/src/tcp.rs`
- `tcp_recv`：`if host.take_timed_out() { return recv_value(&[], false) }` **在**缓冲检查**之前**；
- `tcp_accept`：`if host.take_timed_out() { return Value::Null }` **在** `pending` 队列检查**之前**。

**症状**：数据在截止时间**之前**就到了缓冲区，但截止时间先到 ⇒ 成员返回
「空字节 + `eof = FALSE`」（`TCP_RECV`）或 `NULL`（`TCP_ACCEPT`）⇒ **已经到达的数据被丢弃**。
`TCP_RECV` 的超时形状本来就表示「这一次没等到」，而数据其实在 ⇒ 与 §21.6.4 的口径不符。

**修法**：把「条件已满足？」的检查提到「已超时？」**之前**——
先看缓冲 / `pending` 里有没有货，有就交付（并清掉超时标志），没有才走超时形状。
`tcp_recv` 与 `tcp_accept` 各改一处，语义顺序即修复。

**验收**：新增契约用例 —— 数据在截止时间前到达但就绪通知晚于截止时间 ⇒ 必须拿到数据，不是空字节。

---

### N2 · `TCP_ACCEPT` 在重放中**再次挂起** ⇒ 违反并发册 §17.0 第 2 条

**规范**：并发册 §17.0 第 2 条：
> 恢复时挂起点被**再次求值** —— 等待条件既已满足，它**必须**立即完成；实现**不得**让它再次挂起（否则任务原地活锁）。

**现状**：`TCP_ACCEPT` 第一次挂起后，连接被 `pending` 队列消费掉；重放时它再次求值 ⇒
队列已空 ⇒ 再次挂起 ⇒ 活锁 ⇒ 烧掉整个超时预算 ⇒ 返回 `NULL` ⇒ 调用方拿到 `E0030`
（§2.2 的实测），**且第一次 accept 到的那个连接句柄被覆盖、再也无法关闭**（泄漏）。

**⇒ 这是本次唯一一条「实现偏离规范明文」的并发缺陷**，也是报告 A2/A3/A5 的共同上游。

**修法（resume 提示 + 每任务 accept 备忘）**：

1. **运行期**：`Scheduler` 记 `resume_hint: Option<i64>`（任务从哪个 socket 被唤醒）。
   `wake_socket_waiter` 置位；任务再次挂起或终止时清位。
2. **宿主接口**：`StdHost` 加三个**带默认实现**的方法（`wlwl-value`）：
   `fn resume_hint(&self) -> Option<i64>`、`fn clear_resume_hint(&mut self)`、
   `fn current_task_key(&self) -> Option<i64>`（按任务记账要用）。
   默认实现返回 `None` / 空操作 ⇒ **不破坏既有实现者**。
3. **成员**（`wlwl-std/src/tcp.rs`）：`TCP_ACCEPT` 维护每任务备忘 `accept_memo: HashMap<task, (listener, conn)>`：
   - 挂起 ⇒ **清掉**本任务在本 listener 上的备忘；
   - 弹出连接 ⇒ **记下**备忘；
   - 恢复后被重放时（`resume_hint` 有值）：
     - 若 `resume_hint == 本 listener`（我就是挂起点）⇒ **不再挂起**；
     - 若备忘里有连接 ⇒ **返回同一个连接**（不重新 pop）；
     - 否则照常 pop。
   - `TCP_CLOSE(conn)` ⇒ 清掉与该连接相关的备忘（**备忘表不随连接数无界长**）。

**为何这样能同时满足 accept 循环与 handler 两种形态**（关键是「accept 挂起 ⇒ 清备忘」）：
- accept 循环：第 2 次 accept 挂起 ⇒ 清备忘 ⇒ 恢复后 pop 到新连接 ✓（不会把旧连接重复交给用户）。
- handler 形态：accept pop 到连接后**记备忘** ⇒ 之后 recv 挂起/恢复的重放里，
  accept **返回同一个连接** ⇒ 不再二次挂起 ✓。

**风险**：中等（动的是并发路径）。**缓解**：只在 `std.net` 传输层内生效；
契约测试必须覆盖「accept 循环」与「handler 形态」两种正例 + 一条负例（不得把同一连接交给两个任务）。

---

### N3 · 就绪事件在「无等待者」时被丢弃 ⇒ 额外延迟 + 吃掉监听 socket 的通知

**位置**：`impl/crates/wlwl-eval/src/runtime.rs` 的 `wake_socket_waiter`
（`self.socket_waiters.remove(&socket_id)` 后若无等待者直接 `return false`）
＋ `impl/crates/wlwl-eval/src/lib.rs` 的 `pump_socket_readiness`（`take_ready()` 一次性排干）。

**症状**：
- 事件被排干时若无人等待 ⇒ 事件**永久消失** ⇒ 下一次 park 只能等**下一次**推送
  ⇒ 最多 **50 ms**（`pump_socket_readiness(50)` 的轮询粒度）的额外延迟；
- 监听 socket 更糟：acceptor 线程**只在接到新连接时**推一次 ⇒ 这一次通知被吃掉后，
  下一次 accept 只能等**下一个**连接 ⇒ 表现为「偶发卡住一个连接」。

**修法**（pending 标志 + **条件变量**）：
- `Scheduler` 增加 `pending_ready: HashSet<i64>`；
- `wake_socket_waiter` 找不到等待者时 ⇒ 把 socket id 放进 `pending_ready`（**不丢**）；
- `park_for_socket` 时若 `pending_ready` 命中该 socket ⇒ **立刻**唤醒那个任务
  （等待立即结束，不白等一个轮询周期）。
- ⚠ **另外**：解释器侧等就绪事件原本是「**睡 50 ms 再看一眼**」⇒ 一次「就绪」
  最多白等 50 ms（第三方报告实测：客户端 33 ms 发数据、服务端 51 ms 才拿到）。
  ⇒ 给就绪队列加**条件变量**，I/O 线程一推进就 `notify_all` ⇒ 延迟 ≈ 0。
  （实测：同一条用例 51 ms ⇒ **0 ms**。）

**风险**：低。**验收**：契约用例 —— 事件先到、等待者后到 ⇒ 等待**立即**返回，不得等满超时。

---

### N4 · 一个 socket 只允许挂一个等待者 ⇒ 后者静默顶掉前者

**位置**：`impl/crates/wlwl-eval/src/runtime.rs`
`socket_waiters: HashMap<i64, TaskId>` ＋ `park_for_socket` 的 `insert`（**覆盖**）。

**症状**：两个任务读**同一条连接** ⇒ 第二个 `park_for_socket` 覆盖掉第一个 ⇒
第一个任务**永远等不到唤醒**，只能耗到自己的截止时间；而它的等待者登记已被删掉 ⇒
调度循环连它的超时都判不准。

**修法**：`socket_waiters` 改为一 socket 多等待者（`HashMap<i64, Vec<TaskId>>`），
`wake_socket_waiter` 唤醒**全部**等待者（socket 就绪对所有读者都是真）；
取消路径同步清理（并发册 附录 H.4.3 那条清空登记的逻辑要跟着改）。

**风险**：低—中。**验收**：契约用例 —— 两个任务读同一连接 ⇒ 两者都被唤醒。

---

## §4 修复批次与验收

| 批次 | 内容 | 风险 | 状态 | 验收 |
|---|---|---|---|---|
| **B1** | N1（缓冲先于超时）、N3（pending 就绪 + 条件变量）、N4（多等待者） | 低 | ✅ **已落地** | `socket_wait_contract.rs` 4 条守卫 + `runtime.rs` 单测 3 条；`tcp_contract` 23/23；全量绿 |
| **B2** | N2（accept 不再重放中挂起） | 中 | ✅ **已落地** | `socket_wait_contract.rs` 2 条行为用例（重放不再挂起 / accept 循环仍给出不同连接）；第三方那份 HTTP 服务器在新构建上 **6/6 路由 200/404**、`/utf8` = 45 字节 |
| **B3** | skill 更新 | 低 | ✅ **已落地** | `wlwl-skill/examples/socket_server.wll` 实跑通过（超时路径与应答路径都验过） |

### B1 / B2 的落点

| 缺陷 | 落点 | 改了什么 |
|---|---|---|
| N1 | `impl/crates/wlwl-std/src/tcp.rs` | `tcp_recv` / `tcp_accept`：「缓冲/`pending` 里有没有货」**先于**「有没有超时」 |
| N3 | `impl/crates/wlwl-std/src/tcp.rs` + `impl/crates/wlwl-eval/src/{lib,runtime}.rs` | 就绪队列加**条件变量**（解释器不再「睡 50 ms 再看一眼」）；无等待者时事件记进 `pending_ready`，下一次 park 立刻消费 |
| N4 | `impl/crates/wlwl-eval/src/runtime.rs` | `socket_waiters` 改一 socket **多等待者**；取消/超时只摘自己 |
| N2 | `impl/crates/wlwl-eval/src/{lib,runtime,task}.rs` + `impl/crates/wlwl-value/src/lib.rs` + `impl/crates/wlwl-std/src/tcp.rs` | 运行期记「恢复提示」（挂起点是哪个 socket）⇒ 宿主接口加 `resume_hint` / `clear_resume_hint` / `current_task_key`（**都带默认实现**）⇒ `TCP_ACCEPT` 记住「本任务 accept 到的连接」，重放时**返回同一个**、不再二次挂起 |

**⚠ N2 改变了可观察行为**：修复前「不切段」的任务体（accept + recv 写在同一个任务体、
没有 `YIELD`）在客户端连上后会**白等整个 accept 超时预算**再报 `E0030`；修复后它
**正常拿到数据**（实测 5345 ms + `E0030` ⇒ 230 ms + 5 字节）。
⇒ 这**不是**语义变更，是并发册 §17.0 第 2 条（「实现**不得**让它再次挂起」）的落地。

**门禁命令**（本地复现 CI 的确切口径）：
```powershell
cd D:\Project\wlwl\impl
cargo test --workspace --no-fail-fast
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
```

⚠ **每批单独提交**（`fix(impl):` / `test(eval):` / `docs(skill):` 风格），**不 push**。

---

## §5 `wlwl-skill` 更新计划（AI agent 侧）

**现状**：skill 的基线停在 **v1.0 规范 + v0.11.3 增量**（`reference.md` 的 §27 是最后一节增量备忘），
**完全没有 v0.12.0 的内容** —— 而 v0.12.0 正是把「网络能力」变成公共面的那一版。

**要补的东西**（按 AI agent 的使用顺序排）：

1. **`SKILL.md` 头部**：`description` 的触发面从 v1.0 基线改为 **v0.12.0**；
   「运行符速查表」的标题补 v0.12.0；版本表补 v0.12.0 行。
2. **`SKILL.md`「标准库指针」段**：补 **17 个新公共面成员**（D-13）：
   `std.net` +9（`TCP_LISTEN` / `TCP_ACCEPT` / `TCP_RECV` / `TCP_SEND` / `TCP_PEER` /
   `TCP_LOCAL_ADDR` / `TCP_SET_TIMEOUT` / `TCP_CLOSE` / `TCP_CLOSE_LISTENER`）、
   `std.encode` +3（`UTF8_ENCODE` / `UTF8_DECODE` / `BYTE_LEN`）、`std.text` +1（`TO_INT`）、
   `std.str` +1（`CAT`）、`std.env` +1（`SCRIPT_PATH`）、`std.result` +1（`TAKE_ERR`）、
   `std.time` +1（`SLEEP_ASYNC`）。
3. **`SKILL.md`「并发」段**：补 **宿主挂起**（D-16 / D-18）的四条纪律 ——
   ① 等待**只挂起当前任务**；② `timeout_ms = 0` = 无截止时间；
   ③ **段首重放**⇒ 挂起点之前的副作用会**重做**（§17.0 第 1 条）；
   ④ **`SLEEP` 是阻塞原语** ⇒ 并发代码里用 `SLEEP_ASYNC`（**这条正是 A1 的真因**）。
4. **`SKILL.md`「错误模型」段**：补 `TAKE_ERR`（一步式检查并消费）、`INT` 的严格规则（D-17）、
   `std.fs` 的 IO 失败改 `ERR` 值（D-14）。
5. **`reference.md`**：§9 标准库目录补 17 员；新增 **§28 v0.12.0 增量备忘**；
   §21 并发段补宿主挂起一节 + **「网络编程 7 条纪律」**（来自报告，但要按本次核实订正：
   纪律 5「`TCP_SEND` 之后必须 `TCP_CLOSE`」**不成立** —— 见 §2.1）。
6. **`wlwl-skill/examples/`**：新增 `socket_server.wll`（纯 WLWL 的最小 TCP 服务，
   演示「accept 在任务外 / 等待在段首 / 副作用在段外」的正确形状）。
7. **`CHANGELOG.md` / `README.md`**：补 v0.12.0 段与目标版本。

**⇒ skill 的纪律清单要以本次核实为准**，不能照抄报告：
报告 7 条纪律里，**第 5 条**（"`TCP_SEND` 之后必须 `TCP_CLOSE`"）经实测**不成立**
（同机纯 Python 对照 289 ms 即达；wlwl 侧把 `SLEEP` 挪后即 82 ms 达），
**第 1 条**（「顶层求值里挂起 ⇒ `E0014`」）与**第 6 条**（「别在任务体里 `SET` 不可变绑定」）
成立但归因要写准；**第 2 / 3 / 4 / 7 条**成立且是本版最有价值的部分。

---

## §6 本次不做 / 待裁决

| 项 | 处置 | 理由 |
|---|---|---|
| A3 的语义（段首重放重做副作用） | **不改** | 规范明文；改动 = ADR-0017 Step 3 + 规范仓改文本，超出本轮 |
| A5（多子任务汇合） | **不改** | 同上（addendum-12 W-02/W-03 的解锁条件） |
| `AWAIT` 已结束句柄报 `E0053` | **不改** | §17.1 明文 |
| 「父任务等多个子任务」的更好原语 | **登记待裁决** | 属语言面变更 ⇒ 需业主拍板 + 走 ADR-0023 §v0.x |
| 规范 §17.1「必须等待其子任务结束」与 §17.3 第 6 条「强制取消」的措辞张力 | **登记待裁决** | 规范仓 `Laffinty/wlwl-spec-v1` 措辞收敛（§2.4 已给出可判定的读法） |
| N2 的规范侧确认 | **落地前登记** | 实现改的是「不得再次挂起」这条的落地方式，不改语义 ⇒ 走一致性册 §5.4 接线表同步 |

---

## §7 证据索引（本轮新增的复现件）

均在 `D:\Project\wlwl\.scratch\`（**不入库**，`.gitignore` 已忽略 `.scratch/`）：

| 路径 | 作用 |
|---|---|
| `probe.py` | 通用驱动（起 wlwl、连客户端、打印时间线） |
| `a1/a1_sleep_task.wll` | A1a：并发 `SLEEP` ⇒ 发送被推迟到 3003 ms |
| `a1/a1_sleep_after.wll` | A1b：`SLEEP` 在发送之后 ⇒ 53 ms 即达 |
| `a2/a2_no_yield.wll` | A2：不切段任务体 ⇒ accept 重放中再挂起 ⇒ 5 s 后 `E0030` |
| `a2/a2_yield_first.wll` | A2：切段路径 ⇒ `recv_wait=0`（对照） |
| `a2/a2_double_yield.wll` | A2：报告 `lostwake_a` 的等价物 |
| `c1/print_buffer.wll` | C1：`PRINT` 行缓冲验证 |

⇒ 追踪用的临时 `eprintln!`（`pump_socket_readiness` / `wake_socket_waiter`）**已回滚**，
本轮落地改动不包含它们。
