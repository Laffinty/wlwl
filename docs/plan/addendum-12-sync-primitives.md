# 12 · 同步原语(`std.sync` —— 通道之外的那一半)

> **层级** L2 · **前置** 01(`std.time` 的假时钟气泡是测同步原语的**免费**工具)
> **状态** 未动工 · **规范登记** 本批**不在** §14 清单里 —— 由本方案首次提出

## 0. 契约摘要

- **目标**:补上 `MUTEX` / `CONDITION` / `ONCE` / `SEMAPHORE` /
  `WAIT_GROUP` 这类**同步原语**,让 `SCOPE` / `STEP` / `SPAWN` + 通道之上
  能写出正确的数据共享。
- **非目标**(违反即范围事故):
  - **不动通道**。通道是全局内建(附录 G 106 条之一),本份**不新增、不改
    签名、不改语义**。
  - **不做无锁数据结构**(无锁队列 / 无锁哈希)。正确性证明成本极高,
    而可复现执行下无锁的收益远低于锁。
  - **不做 `std.thread` / OS 线程**。wlwl 的并发是**协作式**(真挂起,
    语言规范 §17)—— 加真线程会与「确定性调度」这个卖点正面冲突。
  - 不做 `RWLock`(读多写少场景在没有 `RWLock` 时用 `MUTEX` 替代;
    在协作式调度下读写锁的收益比线程模型小得多)。
- **为什么值得做**:真挂起(v0.9)让「阻塞」成为一等公民,但**没有锁** ——
  两个任务共享一个 `DICT` 只能靠 `SPAWN` 的隔离约定。这是能力上的**半截**。

## 1. 基线(可核验)

```powershell
cd docs
Select-String -Path spec/wlwl-spec-v0.11.md -Pattern '17|真挂起|SPAWN|SCOPE|STEP' -Context 0,2
Select-String -Path stdlib/wlwl-stdlib-spec-v0.11.md -Pattern 'std.sync|同步' -Context 0,1
cd ..\impl
Select-String -Path crates/wlwl-std/src/lib.rs -Pattern 'ALL_SPECS' -Context 0,2
```

| 项 | 现值 | 出处 |
|---|---|---|
| 并发设施 | 通道 / `SCOPE` / `STEP` / `SPAWN` / 真挂起 | 附录 G(**全局内建**,非 `std.*`) |
| 锁类成员 | **0** | 附录 A(104 成员) |
| 调度模型 | 协作式 + 确定性(`STEP`);真挂起(v0.9) | 语言规范 §17 |
| 规范登记 | §14 **无** `std.sync` 条目 —— 本份首次提出 | — |
| 假时钟 | **尚不存在**(01 在做) | `addendum-01-std-time.md` |
| 主流对照 | Go `sync`(Mutex / RWMutex / WaitGroup / Once / atomic)、Java `java.util.concurrent.locks`、Rust `std::sync`、Python `threading.Lock` | — |

## 2. 成员面

**层:R2 全员**(锁状态必须跨任务共享,语言层做不到)。

| 成员 | 语义要点 | 失败口径 |
|---|---|---|
| `MUTEX()` | 创建互斥锁,初始**未加锁** | — |
| `MUTEX_LOCK(m)` | 不可重入。**已持有时再锁 ⇒ 死锁**(不是报错)—— 规范要写明,并提供 `MUTEX_TRY_LOCK` 供探测 | — |
| `MUTEX_TRY_LOCK(m)` | 非阻塞;失败返 `FALSE` | — |
| `MUTEX_UNLOCK(m)` | 未持有时解锁 ⇒ `E0030`(这是**程序员的错**,必须响) | `E0030` |
| `CONDITION(m)` | 条件变量,**必须**绑定一个 `MUTEX` | 传非 `MUTEX` → `E0030` |
| `COND_WAIT(c)` | 释放锁 → 挂起 → 被唤醒后重获锁。**必须**在持有对应锁时调用 | 未持锁 → `E0030` |
| `COND_SIGNAL(c)` / `COND_BROADCAST(c)` | 唤醒一个 / 全部 | — |
| `ONCE(f)` | 首次调用执行 `f`,之后返回**同一**结果 | — |
| `SEMAPHORE(n)` / `SEM_ACQUIRE` / `SEM_RELEASE` / `SEM_TRY_ACQUIRE` | 计数信号量 | `acquire` 超过 `n` ⇒ `E0030` |
| `WAIT_GROUP()` | `WG_ADD` / `WG_DONE` / `WG_WAIT` | `DONE` 多于 `ADD` → `E0030` |

**统一形态**:每个锁类对象的值形状是 `DICT`(`{"kind": "mutex", "locked": …}`)
—— 形状是**公开契约**,字段名变更即 breaking。

## 3. 语义裁决点(开写前必须逐条回答)

1. **归内建还是归 `std.sync`?**(这是本份的**头号**裁决)
   现状:通道 / `SCOPE` / `STEP` / `SPAWN` 是**全局内建**(附录 G)。
   但 ADR-0023 **§v0.x 4** 有一条硬约束:**「新增命名空间不得重导出全局内建同名
   成员」**。若 `std.sync` 叫 `SPAWN` / `SCOPE` 就违规;叫 `MUTEX` 不违规
   (内建里没有 `MUTEX`)。
   → 裁决:`MUTEX` 族是**新名字**,放 `std.sync` **不违规**;但「为什么锁在
   std 而通道在内建」需要一句理由写进规范(否则下一个 agent 会「顺手统一」
   —— 那是一次 breaking)。**建议措辞**:内建是**语言语法对应的形态**,
   锁是**库设施**。
2. **锁是不是新内建类型?** 倾向**否**,用 `DICT`(同 10 §3.1、11 §3.2 的立场)。
   ⇒ 每个锁操作要有一次 `DICT` 构造,**性能要测**(§4)。
3. **可重入吗?** 倾向**不可重入 + `TRY_LOCK` 探测**(可重入需要线程身份,
   而协作式调度下「当前任务」的身份模型要先定)。
4. **死锁检测做不做?** 不可重入 + 协作式调度下,`LOCK` 已持有的锁 = 整个
   气泡停摆(而不是只挂一个任务)。→ 至少要在规范里**明写这个后果**;
   可选:检测「气泡内所有任务都在等锁」时报错。⇒ 这会与 01 的
   `WAIT_BLOCKED` 联动。
5. **`WAIT_GROUP` 与 `SCOPE` 的关系**:`SCOPE` 退出时会不会自动 `WG_WAIT`?
   若会,`WAIT_GROUP` 就只在跨 `SCOPE` 时才需要。→ 需确认语言语义。
6. **`STEP` 确定性下,锁的获取顺序是确定的**吗?若是,那锁反而**强化**了
   可复现性(而不是削弱)—— 这点值得写进规范当卖点。
7. **`std.sync` 会不会也该收 `std.test` 的 `ASSERT`?** 不该(测试面 vs
   生产面,同 11 §2)。

## 4. 验收门禁

```powershell
cd impl
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --all-targets
cargo deny --locked --all-features check
$env:RUSTDOCFLAGS='-D warnings'; cargo doc --locked --no-deps
cargo check --locked -p wlwl-std --features real-ai
```

契约表 `tests/sync_contract.rs`(新)。**关键:测试必须用 01 的假时钟气泡**,
否则任何等锁的用例都会真等:

| 组 | 用例 | 备注 |
|---|---|---|
| 形态 | 元数 / 类型 / `UNLOCK` 未持有 → `E0030` | 逐字 |
| 基本互斥 | 气泡内两个任务抢锁,**计数器断言临界区不重叠** | 机制断言 |
| `TRY_LOCK` | 已持有时返 `FALSE` 且**不挂起** | |
| `COND` | 生产者-消费者:气泡内 `SLEEP` 让出,断言**顺序正确且不丢不重** | 依赖 01 |
| `ONCE` | 100 个任务并发调,**`f` 只执行一次**且结果相同 | |
| `SEMAPHORE` | `n` 个许可 ⇒ 恰好 `n` 个并发 | |
| `WAIT_GROUP` | 全部 `DONE` 前 `WAIT` 必挂起;完成后放行 | |
| **确定性** | 同一程序跑两次,任务完成顺序**逐字节相同** | **本语言卖点,锁不该削弱它** |
| **死锁后果** | 气泡内重复 `LOCK` ⇒ `WAIT_BLOCKED` 停摆(断言**当前行为**,写进规范) | 裁决 §3.4 |
| 回归 | `SCOPE` / `STEP` / `SPAWN` / 通道的既有行为**逐条不动** | 防动内建 |

**性能断言**:`MUTEX_LOCK` + `UNLOCK` 100 万次 ≥ 50 MB/s(无竞争路径)。

## 5. 九处表落点

九处见 `docs/history/20261002.md` §5.0。**本份九处全中**,且第 7 处是重点:

| 落点 | 本份动作 |
|---|---|
| 1 规范成员表 | 新增 §23 `std.sync` 章 + 成员表 + **「为什么锁在 std 而通道在内建」条款** + 死锁后果条款 |
| 2 附录 A | 跑 `gen-appendix-a` |
| 3 `SPEC.functions` | 新 `src/sync.rs` + N 个 `StdFn` |
| 4 契约测试硬编码 | `sync_contract` 成员数与点名清单 |
| 5 模块登记 | `ALL_SPECS` 登记 `sync`;`src/` 文件数守卫 |
| 6 probe | ≥ 2 条(互斥 / 确定性),`EXPECTED_CASE_COUNT` 同步 |
| 7 附录 G | **重点**:确认**不动**通道 / `SCOPE` / `STEP` / `SPAWN`;若有任何改动 `spec_appendix_g_sync` 会红 —— 那正是要的效果 |
| 8 skill 指针 | `SKILL.md` + `reference.md` §9.15;**反模式**加「共享可变状态靠 `SPAWN` 隔离约定」是错的 |
| 9 CHANGELOG | 成员面 +N;命名空间 +1 |

## 6. 风险与已知代价

- ⚠️ **裁决 §3.1 是本份的前提**。若「统一到内建」被提上,那就是**动附录 G
  的 106 条之一** ⇒ breaking ⇒ 要申报。**本份的立场是「不统一」,并把理由
  写进规范** —— 否则下一个人会以为那是遗漏。
- ⚠️ **死锁在协作式调度下后果更重**:一个任务重复加锁会**停摆整个气泡**,
  而不是只挂自己。规范必须写明,skill 反模式要写。
- ⚠️ **依赖 01**。没有假时钟气泡,同步原语的测试要么真等(慢且脆),
  要么写得看不出问题。⇒ **01 未落地前不要开写本份的测试**(成员可先落)。
- ⚠️ **`DICT` 承载锁状态**意味着每次加解锁一次字典构造;若性能不可接受,
  就得回 §3.2 争「新内建类型」—— 那是语言变更。**先测再争**(同 10 的纪律)。
- **明确不做**:无锁结构 / OS 线程 / `RWLock` / 改通道与 `SCOPE` / `STEP` /
  `SPAWN` 的任何语义 / 可重入锁。

## 7. 检查点

| 子项 | 内容 | 状态 | commit |
|---|---|---|---|
| W-01 | 归内建 vs `std.sync` 裁决(§3.1)+ 数据形状裁决 | 未动工 | — |
| W-02 | `MUTEX` 族 + 形态契约(**不依赖 01**) | 未动工 | — |
| W-03 | `CONDITION` / `ONCE` / `SEMAPHORE` / `WAIT_GROUP` 实现 | 未动工 | — |
| W-04 | **气泡内**并发契约(依赖 01)+ 确定性断言 | 未动工 | — |
| W-05 | 死锁后果条款 / 性能 / 收口 | 未动工 | — |
