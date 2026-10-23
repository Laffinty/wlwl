---
title: X-02 机制验证与实施前置 —— `SHIELD` 的取消推迟口径(到达时点 vs 来源判据)
reason: 2026-10-23 业主授权的**只读机制验证**(接替执行 v1.0 对齐批次);X-02 是未收敛清单仅剩两条之一
---

# X-02:`SHIELD` 的取消推迟口径

## 0. 这份文件是什么

**是**:X-02 的机制验证结论 + 实施前置条件 + 待裁决清单。
**不是**:实施方案(未获授权);不是规范文本(权威在独立仓 `Laffinty/wlwl-spec-v1`,本文引用的条款都是**只读引用**)。

**状态标记**(全篇统一):

| 标记 | 含义 |
|---|---|
| `[已核实]` | 有**实测读数**或**规范原文**支撑 |
| `[待裁决]` | 需业主拍板(语义裁决)⇒ 未拍板前不动实现 |
| `[待验证]` | 源码推演,**未实测** |

⚠ **行号会漂** ⇒ 下文以**函数名**为主定位,行号只是当时的辅助坐标。

## 1. 结论(先看这一段)

1. **缺口真实存在。** 实测:跨任务 `TASK_CANCEL(a)` 命中一个**正处在 `SHIELD` 块内**的任务 ⇒
   **取消立即生效** `[已核实]`(§2 探针 A)⇒ 违反一致性册 §5.4 的 X-02 行
   ——「`SHIELD` 屏蔽**屏蔽块内收到的**取消请求」。
2. **现行实现是「来源判据」。** `shield_depth` 挂在**共享 `Evaluator`** 上;只有
   `TASK_CANCEL_PARENT()` 在**自己**深度 > 0 时才缓冲;`TASK_CANCEL(handle)` 直接写
   `cancel_requested`,**完全不看屏蔽状态** `[已核实]`(§3)。
   ⚠ 这是**当初的刻意设计**(`git log -S shield_depth` 全仓只有一次提交 `62317f1`,
   其 commit message 明写「`SHIELD` only blocks external cancel via `TASK_CANCEL_PARENT`;
   `TASK_CANCEL(handle)` from another task is unaffected」)⇒ 改成「到达时点」是**语义变更**,
   不是补 bug ⇒ 需进 CHANGELOG 发布说明 + 一致性册登记。
3. **机制可行,但不是「换一个判据」那么小。** 需要 per-task 屏蔽状态 + 投递点改按**目标**判定
   + **3 条语义裁决** + **2 项新能力**(§4 / §5)。
4. **不必等 `ADR-0017` Step 3。** 「目标挂起在屏蔽块内」那一支只需在**挂起点**捕获深度
   (挂起时屏蔽帧仍在栈上)⇒ 不需要续体 `[已核实]`(§7)。
5. ⚠ **探针把原假设推翻了一半。** 规范并发册 §17.0 的「**不完成**」条款规定:挂起点所在的
   最外层表达式**不完成**,含它的顶层语句随之**不完成**,该语句其余部分**不执行** ⇒
   `YIELD` 落在 `SHIELD` 块内时,**屏蔽块体的剩余语句被丢弃** ⇒
   「取消到达时目标还在不在屏蔽块内」这个问题比预想的更尖锐 `[已核实]`(§2 探针 C / D、§6 Q3)。
6. **裁决已拆成三条可判定的岔口**(§6):**Q1** 判据主体(按目标 vs 按发起方)/
   **Q2** 硬清理豁免(作用域退出 / 兄弟失败连带)/ **Q3** 屏蔽块边界(帧口径 vs 语句口径)
   ⇒ 每条都带**实测读数**与**可见后果**,并给出推荐 ⇒ **等业主拍板**。
   ⚠ 「检查点集合」**不是**独立问题:被推迟的取消**释放后**走同一条 `cancel_requested`
   通道 ⇒ 与「立即生效」逐字一致 ⇒ **不需要新增检查点**(§6.4)。
7. ✅ **三条裁决已于 2026-10-23 落定**(§6.6):**Q1 按目标** / **Q2 只有作用域退出豁免** /
   **Q3 帧口径** ⇒ 全部取推荐项 ⇒ **可实现规格**见 §6.7(S1–S7 + 豁免取表 + 预计的
   可见面变更 + 要钉的契约),**规范侧要改什么**见 §6.8。⚠ 三条裁决**都不需要改规范正文**
   ⇒ 规范侧只动「实现注」+ 一条新增差异登记。

## 2. 实测读数(2026-10-23)

**读数来源**:`impl/target/debug/wlwl.exe`(构建于 2026-10-23 21:20,含本批 X-15 / G-02 / rest 通道改动;
这三项与 `SHIELD` / 取消**无交集** ⇒ 读数对本项有效)。
**探针位置**:仓外 TEMP(`%TEMP%\wlwl_probe_20261023`)⇒ **不入库**(探针程序不进仓)。
**CLI 用法**:`wlwl.exe run <file>`(直接给路径会报 `unrecognized subcommand`)。

| # | 探针 | 场景 | 实测 stdout | 结论 |
|---|---|---|---|---|
| **A** | `x02_a_cross_task_cancel` | 任务 `a` 在 `SHIELD` 块内 `YIELD`;兄弟任务 `b` 调 `TASK_CANCEL(a)` | `a: in-shield` / `a-cancelled` | **取消立即生效**,`a` 直接转 `Cancelled` ⇒ **X-02 缺口本体** |
| **B** | `x02_b_self_cancel_in_shield` | 任务**自己**在 `SHIELD` 块内调 `TASK_CANCEL_PARENT()` | `b: in-shield` / `b: inside-cancelled? no` / `b: after-shield cancelled? yes` / `b-not-cancelled` | **来源判据生效**:块内 `TASK_IS_CANCELLED()` = FALSE,退出后 = TRUE ⇒ H.4.4 金标准保住 |
| **C** | `x02_c_cancel_after_yield_in_shield` | 任务在 `SHIELD` 块内 `YIELD`,**恢复后**(屏蔽块外)再 `TASK_CANCEL_PARENT()` | `c: in-shield` / `c: after-shield` / `c: after-self-cancel cancelled? yes` / `c-not-cancelled` | ① `c: after-yield-in-shield` **没打印** ⇒ **屏蔽块体剩余语句被丢弃**(§17.0「不完成」)② 恢复点在屏蔽块**之外** ③ 该次自取消**不被缓冲** ⇒ 屏蔽深度在**挂起点**就已回退 |
| **D** | `x02_d_nested_yield_loss` | 任务体里 `LET(f, FUN(… YIELD() …))` + `CALL(f)`,`f` 内 `YIELD` 之后有 `PRINT` | `d: nested in` / `d-not-cancelled` | `d: nested after-yield` **与** `d: after-call` **都没打印** ⇒ 嵌套闭包尾巴 + 所在段的剩余语句**一起丢弃** |
| **E** | `r4_sibling_fate` | 被**屏蔽**的任务发 `TASK_CANCEL_PARENT()`;同域**未**屏蔽的兄弟读自己的 `TASK_IS_CANCELLED()` | `0` | 兄弟**不会**被取消(整棵子树一起推迟)⇒ 这是 §6 Q1 两个选项的分水岭:按**目标**判定 ⇒ 读数会变 `1`;按**发起方**判定 ⇒ 仍 `0` |

**探针程序**(仓外 TEMP,复跑时照抄):

```wlwl
// A —— 跨任务取消命中屏蔽块内的目标
LET(out, SCOPE(FUN(() ,
    LET(a, SPAWN(FUN(() ,
        SHIELD(FUN(() , PRINT("a: in-shield"); YIELD(); PRINT("a: after-yield-in-shield"); 0));
        PRINT("a: after-shield"); 1)));
    LET(_, SPAWN(FUN(() , TASK_CANCEL(a); 0)));
    LET(r, AWAIT(a)); LET(_, r);
    IF(IS_ERR(r), "a-cancelled", "a-not-cancelled"))));
PRINT(out)

// B —— 自己取消(来源判据)
LET(out, SCOPE(FUN(() ,
    LET(a, SPAWN(FUN(() ,
        SHIELD(FUN(() , PRINT("b: in-shield"); TASK_CANCEL_PARENT();
            LET(inside, IF(TASK_IS_CANCELLED(), "yes", "no"));
            PRINT(+( "b: inside-cancelled? ", inside )); 0));
        LET(after, IF(TASK_IS_CANCELLED(), "yes", "no"));
        PRINT(+( "b: after-shield cancelled? ", after )); 0)));
    LET(r, AWAIT(a)); LET(_, r);
    IF(IS_ERR(r), "b-cancelled", "b-not-cancelled"))));
PRINT(out)

// C —— 屏蔽块内 YIELD 之后自取消
LET(out, SCOPE(FUN(() ,
    LET(a, SPAWN(FUN(() ,
        SHIELD(FUN(() , PRINT("c: in-shield"); YIELD(); PRINT("c: after-yield-in-shield"); 0));
        PRINT("c: after-shield"); TASK_CANCEL_PARENT();
        LET(after, IF(TASK_IS_CANCELLED(), "yes", "no"));
        PRINT(+( "c: after-self-cancel cancelled? ", after )); 0)));
    LET(r, AWAIT(a)); LET(_, r);
    IF(IS_ERR(r), "c-cancelled", "c-not-cancelled"))));
PRINT(out)

// D —— 嵌套闭包里的 YIELD(与 SHIELD 无关,隔离变量)
LET(out, SCOPE(FUN(() ,
    LET(a, SPAWN(FUN(() ,
        LET(f, FUN(() , PRINT("d: nested in"); YIELD(); PRINT("d: nested after-yield"); 0));
        CALL(f); PRINT("d: after-call"); 0)));
    LET(r, AWAIT(a)); LET(_, r);
    IF(IS_ERR(r), "d-cancelled", "d-not-cancelled"))));
PRINT(out)

// E —— 被屏蔽的任务发 TASK_CANCEL_PARENT() 时,同域**未**屏蔽的兄弟的命运
//     (返回值 = 兄弟读到的 TASK_IS_CANCELLED();1 = 是,0 = 否)
//     现状 = 0(整棵子树一起推迟);§6 Q1 的「按目标」判定 ⇒ 会变成 1
LET(out, SCOPE(FUN(() ,
    LET MUT(bh, 0);
    LET(a, SPAWN(FUN(() ,
        SHIELD(FUN(() ,
            TASK_CANCEL_PARENT();
            LET(h, bh);
            LET(r, AWAIT(h));
            LET(_, r);
            0));
        0)));
    SET(bh, SPAWN(FUN(() ,
        LET(c, IF(TASK_IS_CANCELLED(), 1, 0));
        c)));
    LET(ra, AWAIT(a));
    LET(_, ra);
    0)));
PRINT(out)
```

⚠ **语法坑**(本轮踩到):字符串拼接是 **`+(a, b)`**(函数形态),**不是**中缀 `+`;`IF(c, a, b)` 可作表达式
出现在实参位;`TASK_CANCEL(handle)` 的实参必须是**句柄**,不是任务值。

## 3. 现行实现的机制事实(全部 `[已核实]`)

### 3.1 屏蔽状态是**共享**的,不是 per-task

`shield_depth` / `shield_pending_cancel` / `shield_pending_reason` 三个字段挂在 **共享 `Evaluator`** 上
(`wlwl-eval/src/lib.rs`),而全仓只有**一个**求值器 ⇒ 任务在它上面轮流跑。

⇒ **跨任务泄漏窗口** = 「A 在屏蔽块内把控制权交回调度器」的那段时间:
* `YIELD()` / 阻塞型通道 park ⇒ 切走时深度**已回 0**(见 §3.3)⇒ B 不受影响,但 A 的「逻辑屏蔽区间」凭空消失;
* A 在屏蔽块内 `AWAIT` / `SCOPE` 退出 drain ⇒ 切走时深度**仍为 1** ⇒ B 被拉进来跑 ⇒
  B 的 `TASK_CANCEL_PARENT` 被**误缓冲**、B 自己的 `SHIELD` **误嵌套**(深度 2,退出回 1 ⇒
  「深度 == 0 才回放待发取消」的判据不成立 ⇒ **B 的待发取消永不触发**)。

### 3.2 取消投递的汇聚点

| 写入 `cancel_requested = true` 的地方 | 语义 | 是否经汇聚点 |
|---|---|---|
| `runtime.rs` `request_task_cancel_with_reason` | **唯一汇聚点** | — |
| `lib.rs` `builtin_task_cancel_parent` 的 `shield_depth > 0` 分支 | `TASK_CANCEL_PARENT()` 标记**自己** | ❌ 绕过 |
| `lib.rs` `builtin_shield` 退出时的待发取消回放 | 最外层 `SHIELD` 退出 | ❌ 绕过 |

汇聚点的上游(三条路):`TASK_CANCEL(handle)`、`TASK_CANCEL_PARENT()` 的**子树**部分、
SCOPE **兄弟失败连带取消** ⇒ **改汇聚点一处 ⇒ 三条路同时受益**(但三条语义不同 ⇒ 需加「是否绕过屏蔽」入参)。

### 3.3 「屏蔽块在挂起点就退出了」

`invoke_closure` 把 `Signal::Yield` 当普通结果上抛 ⇒ `builtin_shield` 里**递减 `shield_depth` 的那几行照常执行**
⇒ 深度在挂起点回退、待发取消在挂起点即触发。实测证据 = 探针 C 的 ②③。

### 3.4 取消的观察点只有 2 个

| 观察点 | 位置 | 状态 |
|---|---|---|
| 任务步进入口 | `run_one_task` | ✅ 有 |
| `TASK_IS_CANCELLED()` | `builtin_task_is_cancelled` | ✅ 有(且第三条路径读**父作用域** `cancelled`,绕过 per-task flag) |
| `YIELD()` 处 | `builtin_yield` | ❌ **不存在**(规范 §17.3 第 3 条要求) |
| 阻塞型 `CHANNEL_SEND` / `CHANNEL_RECV` 入口 | — | ❌ **不存在** |

### 3.5 作用域退出**没有**真正的强制取消

`pop_scope` 只做「排干运行队列 + 强关通道」;`cancel_suspended_task` 与
`Scope::outstanding_tasks` **只有测试调用点** ⇒ 「scope 退出等子任务、逾时强制取消」是**未兑现的承诺**。

⇒ **对 X-02 的直接影响**:一旦允许「推迟」,就可能让子任务活过 scope ⇒ 必须先裁决「scope 退出时怎么办」(§6 Q2)。

## 4. 落点清单(按「到达时点」判定所需)

⚠ **裁决已落定(§6.6)⇒ 下表的「改什么」列已按裁决结果定稿**;规格条款编号 **S1–S7**
见 §6.7。

| # | 落点 | 改什么 | 裁决后 | 状态 |
|---|---|---|---|---|
| L1 | `runtime.rs` `request_task_cancel_with_reason` | **主改**:读**目标**的 per-task 屏蔽状态;命中 ⇒ 写**目标的**待发取消缓冲,**不**置 `cancel_requested` | S1 | ⬜ X-02b |
| L2 | `runtime.rs` `cancel_scope_subtree_with_reason` | 子树取消 ⇒ **逐目标**投递(不再「整棵子树随发起方」);⚠ 它同时置 `scope.cancelled = true` ⇒ 该标志会被 `TASK_IS_CANCELLED` 的第三条路径读到 ⇒ 配合 L8 | S1 + S2 | ⬜ X-02b |
| L3 | `runtime.rs` `cancel_siblings_in_scope_with_reason` | 兄弟连带取消 ⇒ **受屏蔽保护**(今天直接写 flag、不看屏蔽状态)⇒ 走 L1 的逐目标投递 | S3 | ⬜ X-02b |
| L4 | `lib.rs` `builtin_task_cancel_parent` 的 `shield_depth > 0` 分支 | **来源判据所在地** ⇒ 改后**不再提前返回**:「推迟自己」与「取消子树」拆开;⚠ H.4.4 金标准正由它实现 ⇒ 改后必须仍得同样结果 | S2 | ⬜ X-02b |
| L5 | `lib.rs` `builtin_shield` 的深度增减 | 深度改 per-task,增减时**写穿**到 `scheduler.tasks[current_task]`;顶层沿用求值器局部计数(顶层 `SHIELD` 是 no-op) | — | ✅ **已做(X-02a)** |
| L6 | `lib.rs` `builtin_shield` 退出回放 | 待发取消缓冲从全局移到 **per-task**;退出时只回放**本任务**的缓冲 | — | ✅ **已做(X-02a)** |
| L7 | `lib.rs` `builtin_yield` + 通道 park 处 | **挂起点捕获**深度 ⇒ 「目标挂起在屏蔽块内」这一支的唯一数据来源 | — | ➖ **不需要**(Q3 选帧口径 ⇒ 挂起点即结束 ⇒ 无需捕获) |
| L8 | `lib.rs` `builtin_task_is_cancelled` | 被推迟的取消在生效前必须读作 **FALSE**;⚠ **第三条路径(父作用域 `cancelled`)必须尊重本任务的屏蔽状态**,否则被屏蔽的任务照样读出「已取消」⇒ 屏蔽形同虚设 | S5 | ⬜ X-02b |
| L9 | `lib.rs` `run_one_task` 入口 | 目标有「已推迟取消」且此刻**不在**屏蔽块内 ⇒ 生效 | S6 | ➖ **不需要**(释放点 = 屏蔽块退出 ⇒ 由 L6 的回放负责) |
| L10 | `runtime.rs` `cancel_suspended_task`(**未接线**)+ `lib.rs` `pop_scope` | **作用域退出的硬清理**(今天不存在)⇒ 「推迟」不泄漏的前提;⚠ 该路径**豁免**屏蔽 | S4 | ⬜ X-02b |

## 5. 前置能力(非裁决,是纯实现)

| # | 前置 | 依据 | 状态 |
|---|---|---|---|
| P1 | `Task` 上的 **per-task** 屏蔽状态(`shield_depth` + 待发取消两字段) | §3.1 / §3.3 | ✅ **已做(X-02a)**;`suspended_shield_depth`(挂起点捕获)**只在 Q3 选乙案时才需要** |
| P2 | 深度增减**写穿**到 `scheduler.tasks[...]` | L5 | ✅ **已做(X-02a)** |
| P3 | 投递点按**目标**判据 + 「绕过屏蔽」入参 | L1–L3 | ⏳ **X-02b**(依赖 Q1 / Q2 的裁决) |

## 6. 待裁决清单(⚠ **语义裁决** ⇒ 未拍板前不动实现)

**为什么是三条、而不是一条**:X-02b 要改的是「取消请求**什么时候**生效」。这件事有三个
**互相独立**的岔口,每条都改变**用户可见的行为** ⇒ 必须分别拍板:

| # | 岔口 | 一句话 |
|---|---|---|
| **Q1** | **判据主体**:按**发起方**的状态判定,还是按**每个目标**各自的状态判定? | 决定 `TASK_CANCEL_PARENT()` 的**同域兄弟**会不会被连带立即取消 |
| **Q2** | **硬清理豁免**:哪些取消路径**豁免**于屏蔽? | 决定「作用域退出」和「兄弟失败连带」受不受屏蔽保护 |
| **Q3** | **屏蔽块的边界**:屏蔽块在哪一刻算结束?缓冲的取消在哪一刻释放? | 决定 `YIELD` 落在屏蔽块内时,**挂起之后**到达的取消算不算「块内收到的」 |

⚠ 三条**互不覆盖**:Q1 说「谁的状态说了算」,Q2 说「哪些路径豁免」,Q3 说「何时算块内」
⇒ **三条都定 ⇒ 才有可实现的规格 ⇒ 才动实现**。

### 6.1 Q1 判据主体(按目标 vs 按发起方)

**事实基础**:

* 一致性册 §5.4 的 **X-02** 行给的收敛路线原文是「判定从「取消的**来源**」改成「取消
  **到达时点**是否在屏蔽块内**」」⇒ **到达时点**落在**目标**身上 ⇒ 字面上就是**按目标**。
* 并发册 §17.3 的**规范**原文:「`SHIELD(fn)`: 调用 `fn` 并**屏蔽屏蔽块内收到的取消请求**」
  ⇒ 同一个读法。
* ⚠ `TASK_CANCEL_PARENT()` 取消的是**自己 + 同域兄弟 + 子树** ⇒「每个目标各自判定」会让
  **同域兄弟**与发起方**分道扬镳**。
* **实测(现状)**:被屏蔽的任务发出 `TASK_CANCEL_PARENT()` 时,同域**未**被屏蔽的兄弟
  **不会被取消**(整棵子树一起推迟)⇒ 读数 `0`(探针 `r4_sibling_fate.wll`)。

**选项**:

| 选项 | 口径 | 同域兄弟的命运 | 与规范的关系 | 代价 |
|---|---|---|---|---|
| **甲案** | **按目标**:每个目标各自按「请求到达我时我是否在屏蔽块内」判定 | 不在屏蔽块内 ⇒ **立即被取消** ⇒ 读数从 `0` 变 `1` | **完全对齐** §17.3 字面 + X-02 行的收敛路线 | ⚠ **breaking**;实现**更简单**(投递点一处、逐目标判定) |
| **乙案** | `TASK_CANCEL_PARENT()` 保持**按发起方**(整棵子树随发起方);`TASK_CANCEL(handle)` 按**目标** | 与今天**一致**(读数仍 `0`) | 关闭 X-02 行**字面**登记的缺口;但「子树部分」仍偏离 §17.3 字面 ⇒ 差异项**只关一半** | 改动面小、无 breaking;但子树遍历里要**另外**接一条发起方口径(投递点一处不够) |
| ~~丙案~~ | 全部按发起方 | 与今天一致 | **X-02 不修** | 排除 |

**我的推荐:甲案。** ① X-02 行自己写的收敛路线就是「**到达时点**」⇒ 按目标;② §17.3 的
规范原文「屏蔽屏蔽块内**收到的**」是同一个读法 ⇒ 乙案会让 X-02 **只关一半**;③ 实现
**更简单**(投递点一处、逐目标判定)⇒ 乙案要在子树遍历里**另外**接一条口径。
⚠ 代价是 breaking:今天「被屏蔽 ⇒ 本作用域内谁都不会被取消」这形状会变 ⇒ 必须进
CHANGELOG 发布说明 + 一致性册登记。

### 6.2 Q2 硬清理豁免

**事实基础**:

* **作用域退出** today **根本没有**强制取消(`cancel_suspended_task` 与
  `Scope::outstanding_tasks` 都只有测试调用点)⇒ 并发册 §17.1「子任务寿命不超过词法 `SCOPE`」
  这条**结构化并发不变量**今天靠「排干运行队列」兜着,不是靠取消。
* **兄弟失败连带**走的是同一个投递点(`cancel_siblings_in_scope_with_reason` →
  `request_task_cancel_with_reason`)⇒ **今天就已经绕过屏蔽**(直接写 `cancel_requested`,
  不看屏蔽状态)。
* ⚠ 这两条性质**不同**:兄弟失败连带是**业务级**清理(某兄弟已失败 ⇒ 作用域结果已定 ⇒
  其余兄弟的活没意义了);作用域退出是**生命周期边界**(词法 `SCOPE` 返回 ⇒ 子任务必须结束)。

**选项**:

| 选项 | 作用域退出 | 兄弟失败连带 | 后果 |
|---|---|---|---|
| **甲案** | **豁免**(硬清理、绕过屏蔽) | **受屏蔽保护**(按到达时点逐目标判定) | 被屏蔽的兄弟能**跑完临界区**再被取消 ⇒ 这正是 `SHIELD` 的用途(清理代码有机会跑);最终仍由作用域退出的硬清理收尾 ⇒ **不会死锁** |
| **乙案** | **豁免** | **也豁免**(= 今天的形态) | 兄弟连带路径**零行为变更**;但被屏蔽的兄弟**立即**被杀 ⇒ 它的清理代码**跑不到** ⇒ 与 `SHIELD` 的用途相悖 |

**我的推荐:甲案。** ① 「屏蔽屏蔽块内**收到的**取消请求」是**规范原文** ⇒ 兄弟的请求
到达它时它在屏蔽块内 ⇒ 受保护;② `SHIELD` 的用途是让被屏蔽区**跑完**(清理代码要有机
会执行)⇒ 乙案让屏蔽对兄弟失败**完全无效**;③ 作用域退出**必须**豁免 —— 否则子任务
可能活过 `SCOPE` ⇒ 违反结构化并发不变量(甲案已豁免 ⇒ 两条路径都不死锁)。
⚠ 甲案意味着「兄弟失败连带」那条路径**从今天的行为变成受屏蔽保护** ⇒ 可见面变更,
要进发布说明。

### 6.3 Q3 屏蔽块的边界与释放点

**事实基础**:

* 并发册 §17.0 的「**不完成**」条款:挂起点所在的**最外层表达式不完成** ⇒ 含它的顶层
  语句随之**不完成**、该语句的其余部分**不执行**。
* **实测(探针 C)**:`YIELD` 落在 `SHIELD` 内 ⇒ **屏蔽块体的剩余语句被丢弃**;恢复点在
  屏蔽块**之外** ⇒ 屏蔽的「退出事件」在**挂起点**就发生了。
* **实测(探针 D)**:嵌套闭包里的 `YIELD` ⇒ 闭包尾巴 + 所在段的剩余语句**一起丢弃**
  (同一条款)。
* ⚠ 阻塞型通道操作**不切段** ⇒ 恢复时**段首重放** ⇒ 屏蔽块体**再执行一遍** ⇒「最外层
  退出」事件出现**两次**。
* ⚠ `AWAIT` **不**产生 Yield 信号 ⇒ 屏蔽帧**留在栈上** ⇒ 目标在 `AWAIT` 窗口内**仍在**
  屏蔽块内 ⇒ 这一支两种口径**一致**(都算「块内」)。

**选项**:

| 选项 | 口径 | `YIELD` 落在屏蔽块内 | 缓冲的取消在哪一刻释放 | 代价 |
|---|---|---|---|---|
| **甲案** | **帧口径**:屏蔽块 = `SHIELD` 语句在**栈上**的那段时间 | 挂起点**保护即结束** ⇒ 挂起之后到达的取消**立即生效**(= 今天的探针 C 读数) | 挂起点(= 现状) | 与 §17.0「不完成」一致 + 与现状一致 ⇒ X-02b **只改「跨任务取消」那一支** |
| **乙案** | **语句口径**:屏蔽块 = `SHIELD` 语句**完成**为止 | 挂起点只是**暂停**,保护**继续** ⇒ 挂起期间到达的取消也**推迟** | 放弃那一刻(= 语句被丢弃处) | ⚠ 要**挂起点捕获深度**(新机制,且与段重放耦合)⇒ 改 `YIELD` 那一支的现状行为 ⇒ **breaking** |

**我的推荐:甲案。** ① §17.0「不完成」条款**已经**规定挂起点所在语句不完成、其余不执行
⇒ 屏蔽块在挂起点就结束是**规范已有的**语义,不是新发明;② 与现状**逐字一致** ⇒
X-02b 的 diff 只落在「取消的判定」上,不动 `YIELD` 那一支 ⇒ 回归面最小、可审;③ 乙案
要引入「挂起点捕获深度」,而挂起点捕获与段重放耦合(同一段重放时进入/退出不成对)⇒
复杂度与风险都上一个台阶,且 `ADR-0017` Step 3 落地后可能还要重答。

### 6.4 「检查点集合」**不是**独立问题

被推迟的取消**释放**之后走的是**同一条** `cancel_requested` 通道 ⇒ 与「立即生效」那条
路径**逐字一致** ⇒ **不需要新增任何检查点**。
⚠ 并发册 §17.3 第 3 条列的 `YIELD` / 阻塞操作两处取消检查**今天就不存在** ⇒ 那是与
本项**无关**的既有差距 ⇒ 属**另立差异登记**的范围,不在 X-02b 里混进来。

### 6.5 三条的依赖关系

* **Q1 与 Q2 正交**:Q1 说「谁的状态说了算」,Q2 说「哪些路径豁免」。
* **Q3 独立**于两者,但影响 Q1 甲案下「到达时点」的**判定时刻**(挂起点之后的到达算
  不算「块内」)。
* ⇒ 三条都定 ⇒ 才有可实现的规格 ⇒ 才动实现。

### 6.6 裁决结果(2026-10-23,业主拍板)

| # | 裁决 | 结果 |
|---|---|---|
| **Q1** | 判据主体 | ✅ **按目标判定**(甲案)—— 每个目标各自按「请求到达我时我是否在屏蔽块内」判断 |
| **Q2** | 硬清理豁免 | ✅ **只有作用域退出豁免**(甲案)—— 兄弟失败连带**受**屏蔽保护 |
| **Q3** | 屏蔽块边界 | ✅ **帧口径**(甲案)—— 屏蔽块 = `SHIELD` 语句在栈上的那段时间,挂起点即结束 |

⇒ 三条**全部取推荐项**。⇒ **可实现规格**见 §6.7;**规范侧要改什么**见 §6.8。

### 6.7 裁决 ⇒ 可实现规格(实施 X-02b 时逐条照做)

**总则**:取消请求的**投递**与**观察**分开看 ——
投递点决定「立即置 `cancel_requested`」还是「记进**该目标**的待发槽」;
观察点(`TASK_IS_CANCELLED`)只读**本任务自己**的三个来源。

| # | 规格条款 | 落点 | 依据 |
|---|---|---|---|
| **S1** | 投递点**逐目标**判定:目标**当前在屏蔽块内**(`target.shield_depth > 0`)且该路径**不豁免** ⇒ 取消记进**该目标**的待发槽(`shield_pending_cancel` / `shield_pending_reason`),**不**置 `cancel_requested`;否则立即置 | `runtime.rs` `request_task_cancel_with_reason`(加一个「是否豁免屏蔽」的入参) | Q1 甲案 + Q3 甲案(帧口径 ⇒ 深度就是「是否在块内」) |
| **S2** | `TASK_CANCEL_PARENT()` 在**自己**被屏蔽时**不再提前返回** ⇒ 「推迟自己」与「取消子树」**拆开**:自己的取消进自己的待发槽;子树照常**逐目标**投递 | `lib.rs` `builtin_task_cancel_parent`(今天 shielded 分支直接 `return`) | Q1 甲案 ⇒ 同域兄弟**不再**随发起方一起推迟 |
| **S3** | 兄弟失败连带**受**屏蔽保护 ⇒ 走 S1 的逐目标投递(今天它直接写 flag、不看屏蔽状态) | `runtime.rs` `cancel_siblings_in_scope_with_reason` | Q2 甲案 ⇒ **可见面变更** |
| **S4** | 作用域退出的**硬清理**豁免屏蔽 ⇒ 绕过 S1 的推迟分支;⚠ 且**必须补上今天不存在的强制取消**(`cancel_suspended_task` / `Scope::outstanding_tasks` 现在只有测试调用点)⇒ 否则被屏蔽的子任务会活过 `SCOPE` | `runtime.rs` `cancel_suspended_task` + `lib.rs` `pop_scope` | Q2 甲案 + 并发册 §17.1「子任务寿命不超过词法 `SCOPE`」 |
| **S5** | ⚠ **`TASK_IS_CANCELLED()` 的第三条路径(父作用域 `cancelled`)必须尊重本任务的屏蔽状态**:本任务在屏蔽块内 ⇒ 该路径**不**参与判定(它自己的待发槽已记了作用域级取消);否则照旧 | `lib.rs` `builtin_task_is_cancelled` | Q1/Q2 甲案的**连带约束**(R6)—— 不改这里 ⇒ 被屏蔽的任务**照样**读出「已取消」⇒ 屏蔽形同虚设 |
| **S6** | 释放点 = **屏蔽块退出**(深度回 0)⇒ 待发槽转成 `cancel_requested` —— **X-02a 已就位**(`shield_exit`) | `lib.rs` `shield_exit` | Q3 甲案(帧口径) |
| **S7** | **不新增任何取消检查点**:被推迟的取消**释放后**走的是**同一条** `cancel_requested` 通道 ⇒ 与「立即生效」那条路径**逐字一致** ⇒ `YIELD` / 阻塞操作两处检查点的缺口属**另立差异登记**,不在本项 | — | §6.4 |

**「豁免」标记的取表**(实施时逐个调用点核对):

| 调用点 | 语义 | 豁免? | S? |
|---|---|---|---|
| `TASK_CANCEL(handle)` | 点名取消某个目标 | ❌ | S1(按**目标**判定) |
| `TASK_CANCEL_PARENT()` 的**自己**部分 | 标记自己 | ❌ | S2(自己被屏蔽 ⇒ 推迟自己) |
| `TASK_CANCEL_PARENT()` 的**子树**部分(`cancel_scope_subtree_with_reason`) | 自顶向下取消子树 | ❌ | S1 + S2(逐目标 ⇒ **兄弟不再随发起方**) |
| SCOPE **兄弟失败连带**(`cancel_siblings_in_scope_with_reason`) | 业务级清理 | ❌ | S3(受屏蔽保护) |
| **作用域退出**的硬清理(**今天不存在**) | 生命周期边界 | ✅ | S4(绕过屏蔽) |

**预计的可见面变更**(要进 CHANGELOG 发布说明 + 一致性册登记):

1. **被屏蔽的目标**被跨任务 `TASK_CANCEL(handle)` 命中 ⇒ **推迟**到它的屏蔽块退出(今天立即生效)。
2. 被屏蔽的任务发 `TASK_CANCEL_PARENT()` ⇒ **同域未屏蔽的兄弟立即被取消**(今天一起推迟)⇒ 探针 E 的读数 `0` → `1`。
3. 兄弟失败连带 ⇒ **被屏蔽的兄弟受保护**(今天立即被杀)⇒ 被屏蔽的兄弟能跑完临界区。

⚠ **单任务**形态**逐字不变**(H.4.4 金标准 `FALSE` / `TRUE` / `1`)。

**要钉的契约**(新表,形状同 `x02a_shield_per_task_contract.rs`):
① 跨任务 `TASK_CANCEL(handle)` 命中被屏蔽目标 ⇒ 推迟(屏蔽块内读 FALSE、退出后读 TRUE);
② 探针 E 形态 ⇒ 兄弟读数 `1`;③ 兄弟失败连带 ⇒ 被屏蔽的兄弟**不**立即被取消;
④ 作用域退出 ⇒ 被屏蔽的子任务**被强制取消**(S4);⑤ 单任务金标准不动;
⑥ `TASK_CANCEL(handle)` 打未屏蔽目标 ⇒ 立即生效(不变)。

### 6.8 规范侧要改什么(实施 X-02b 时一并做)

| # | 改哪 | 改什么 | 性质 |
|---|---|---|---|
| 1 | 并发册 §17.3 的**实现注**(非规范段) | 现在那句「参考实现当前把「屏蔽」的作用域限定为**由屏蔽块内的 `TASK_CANCEL_PARENT()` 发起**的取消请求」**已过时** ⇒ 改成「参考实现按**目标**判定:取消请求在**到达目标**时若目标正在 `SHIELD` 块内则被推迟」 | **实现注**(非规范) |
| 2 | 一致性册 §5.4 | **登记**「作用域退出 = 硬清理,**豁免**于 `SHIELD`」这条**例外**(Q2 甲案)⇒ 规范文本「屏蔽屏蔽块内收到的取消请求」本身不含这个例外 ⇒ 属**规范与实现的差异**,要显式登记 | **新增差异登记** |
| 3 | 一致性册 §5.4 的 **X-02** 行 | 收敛注记(⚠ **状态列按惯例不改**,以基线 `46b153b` 为准) | 收敛注记 |
| 4 | — | **规范正文不动**:Q1 甲案对齐 §17.3 字面;Q2 甲案(兄弟连带受保护)对齐 §17.3 字面;Q3 甲案对齐 §17.0「不完成」条款 ⇒ 三条裁决**都不需要改规范正文** | — |



## 7. 与 `ADR-0017` Step 3 的关系

**不在关键路径上。** 「目标挂起在屏蔽块内」那一支只需在**挂起点**捕获深度:
挂起发生时屏蔽帧**仍在 Rust 栈上** ⇒ 此刻读到的深度是精确的
(`YIELD()` 处、通道 park 之前)。这**不需要**保存续体。

⚠ **但不能用「段起基准深度」** —— 反例:「`AWAIT` 在屏蔽块内」。`AWAIT` 不产生 Yield 信号
(它把调度器跑到底再返回普通值)⇒ 屏蔽帧留在栈上、深度仍是 1,而取消正是在这段窗口里到达
⇒ 段起点深度 = 0、挂起点深度 = 1 ⇒ **段起基准漏判**。

⚠ `ADR-0017` 里「保存挂起位置/续体」那块是 `runtime.rs` 的 `EvalState` / `ContFrame`
(**NOT WIRED YET**)⇒ X-02 只需它的**一个子集**(存一个整数)。
⚠ Step 3 落地后 **Q3** 的答案要**重答**(与 `docs/active/addendum-12-sync-primitives.md` 的既有登记纪律一致)。

## 8. 风险点

| # | 风险 | 处置建议 |
|---|---|---|
| R1 | **屏蔽深度跨任务泄漏**(§3.1)—— 今天就是真 bug(B 的待发取消永不触发) | per-task 化(L5)**必须与** X-02 同批落地,否则换判据时基准就是错的 |
| R2 | **段首重跑的副作用重放**:通道 park 恢复时整段重跑 ⇒ 屏蔽块体**再执行一遍** ⇒ 「最外层 `SHIELD` 退出」事件出现两次 | **Q3** 裁决时按「重放后的退出」定释放点 |
| R3 | **作用域退出的强制取消是否绕过屏蔽** | 见 **Q2** ⇒ 建议豁免 + 补 L10 |
| R4 | **`TASK_CANCEL_PARENT` 的子树语义变化**(⇒ **Q1** 的直接后果):现状「块内发起 ⇒ 整棵子树推迟」;到达时点判据下只有**在屏蔽块内的目标**被推迟 ⇒ 同域兄弟**立即**被取消 | breaking 候选 ⇒ 进 CHANGELOG 发布说明 + 一致性册登记 |
| R5 | 待发取消缓冲是**全局**的 ⇒ A 的待发取消会被 B 的屏蔽块退出回放/清空 | 随 L6 一并 per-task 化 |
| R6 | `TASK_IS_CANCELLED` 有**第三条路径**读父作用域 `cancelled` ⇒ 只改 flag 路径时,作用域级取消仍会绕过屏蔽 | L1/L2 的「绕过屏蔽」入参必须覆盖作用域级取消 |

## 9. 实施路线建议

**建议:拆成两批,先做「per-task 化」这一小步。**

* **X-02a(先行,独立可交付)**:把 `shield_depth` 与待发取消缓冲 **per-task 化**
  (L5 / L6 / P1 / P2)⇒ 修掉 R1 这个**今天就存在的真 bug**(跨任务泄漏 ⇒ B 的待发取消永不触发),
  并新增一条跨任务 `SHIELD` 契约表把它钉住。**不改判据** ⇒ 现有 H.4.4 金标准逐字不变。
  理由:这是 X-02b 的**硬前置**,且它本身就是一个可独立验证的缺陷修复;两者混在一批会让 diff 无法审。
  ⇒ ✅ **已落地(2026-10-23)** —— 见 §9.1。
* **X-02b(随后)**:裁决落定后一次性实施 L1–L4 / L7–L10 ⇒ 「到达时点」判据。
  ⇒ ✅ **裁决已于 2026-10-23 落定**(§6.6:Q1 按目标 / Q2 只有作用域退出豁免 / Q3 帧口径)
  ⇒ **可实现规格**见 §6.7(S1–S7 + 豁免取表 + 可见面变更 + 要钉的契约),
  **规范侧要改什么**见 §6.8 ⇒ ⇒ ✅ **已落地(2026-10-23)** —— 见 §9.2。

⚠ **X-14** 与 X-02 **无耦合** ⇒ 仍按业主决定**不排期**。

### 9.1 X-02a 落地记录(2026-10-23)

**改了什么**(只动状态,不动判据):

| 落点 | 改动 |
|---|---|
| `task.rs` `Task` | 新增三个 **per-task** 字段:`shield_depth` / `shield_pending_cancel` / `shield_pending_reason` |
| `lib.rs` `Evaluator` | 原有三个字段改为「**顶层**(`current_task` 为 `None`)兜底」的语义(顶层 `TASK_CANCEL_PARENT()` 本就是 no-op ⇒ 兜底槽实际只服务深度计数) |
| `lib.rs` 新增 `impl Evaluator` 块 | `shield_depth_now()` / `shield_enter()` / `shield_exit(owner)` / `shield_record_pending(reason)` |
| `lib.rs` `builtin_shield` | 深度增减走 `shield_enter()` / `shield_exit(owner)`;**归属任务在进入时捕获** ⇒ 退出时减的是同一个任务 |
| `lib.rs` `builtin_task_cancel_parent` | 判据改读 `shield_depth_now()`(**本任务**的深度);待发取消写进 `shield_record_pending()`(**本任务**的槽) |

⚠ **判据一个字没改**:`TASK_CANCEL_PARENT()` 仍**只**在自己的 `SHIELD` 块内被推迟;
`TASK_CANCEL(handle)` 仍**立即**生效(它不看屏蔽状态 —— 这条语义本轮不动)。

**修前 / 修后读数**(同一批程序;修前用 `git stash` + 重建的**改动前二进制**,修后用改动后二进制,
**各自实跑**):

| 程序 | 观察点 | 修前 | 修后 | 判别性 |
|---|---|---|---|---|
| V1 | 别的任务在**无屏蔽**时自取消 ⇒ 取消是否**立即**生效 | **0** | **1** | ✅ 判别 |
| V2 | 别的任务在**自己的**屏蔽块退出后 ⇒ 它的待发取消是否生效 | **0** | **1** | ✅ 判别 |
| V3 | 本任务在**自己的**屏蔽块退出后 ⇒ 它的待发取消是否生效 | 1 | 1 | ➖ **回归守卫**(两边同值) |

**V1 / V2 / V3 的程序原文**见 §2 的同一批程序(结构相同,只是把 `PRINT` 换成返回值)。

⚠ **可见面变更(breaking)**:只有「**跨任务**的 `SHIELD` 与 `TASK_CANCEL_PARENT()` 交错」
这一支 ⇒ 修前那次取消**被吞**(假阴性)⇒ 修后**生效**。**单任务**形态逐字不变
(既有 H.4.4 金标准 `FALSE` / `TRUE` / `1` 不动)。

**契约**:新表 `impl/crates/wlwl-eval/tests/x02a_shield_per_task_contract.rs` **9 例**:

| # | 用例 | 钉什么 |
|---|---|---|
| a | 别的任务的 `SHIELD` 不得遮挡本任务的取消 | `1` |
| b | 每个任务缓冲**自己**的待发取消 | `1` |
| c | 本任务自己的待发取消在**自己**的屏蔽块退出时生效(**回归守卫**) | `1` |
| d | 两个任务**各自**缓冲、**各自**在退出时生效(待发取消槽不共享) | `[1, 1]` |
| e | 单任务金标准(H.4.4):块内 / 块外 | `[0, 1]` |
| f | 嵌套 `SHIELD`:推迟到**最外层**退出 | `[0, 1]` |
| g | `TASK_CANCEL(handle)` 仍**立即**生效(**未改动的语义**) | `1` |
| h | 别的任务的嵌套 `SHIELD` 不得**误嵌套**本任务 | `1` |
| i | 顶层(无 `SCOPE`)的 `SHIELD` 兜底路径不崩不误报 | `0` |

**负向验证**(回退 per-task 改动 ⇒ 共享深度 + 共享待发取消槽 ⇒ 重跑本表):

| 用例 | 回退后读数 | 断言值 | 结果 |
|---|---|---|---|
| a | `0` | `1` | 🔴 红 |
| b | `0` | `1` | 🔴 红 |
| d | `[1, 0]` | `[1, 1]` | 🔴 红 |
| h | `0` | `1` | 🔴 红 |
| c / e / f / g / i | 与断言**同值** | — | 🟢 绿(守形状的守卫) |

⇒ **4 条判别性** + 5 条守卫 ⇒ 断言确实钉住了 per-task 化、**不是恒真** ⇒ 还原后 9 例全绿。

**全量门禁**(`impl`):`cargo test --locked --workspace --all-targets --no-fail-fast` ⇒
**89 个测试目标 / 2894 例 / 0 失败**;clippy `-D warnings` ⇒ **0**;fmt `--check` ⇒ **0 diff**;
rustdoc `-D warnings` ⇒ **0 error**;规范守卫(hygiene / needle scope)⇒ 绿;`check-appendix-a` ⇒ **OK**;
`python script/build_spec_reference.py` ⇒ **幂等**。

**R1 / R5 的处置结果**:R1(跨任务遮挡)✅ **已修**(V1 / V2 / 用例 a / b / h);R5(待发取消被别的任务串)
✅ **已随 per-task 化一并消除**(用例 d)。R2 / R3 / R4 / R6 属 X-02b 的范围 ⇒ 仍挂在 §8。

### 9.2 X-02b 落地记录(2026-10-23)

**改了什么**(按 §6.7 的 S1–S7):

| 规格 | 落点 | 改动 |
|---|---|---|
| **S1** | `runtime.rs` `request_task_cancel_with_reason_policy`(新) | 新增 `ShieldPolicy{Deferrable, Bypass}`;投递点**逐目标**判定:目标在屏蔽块内 ⇒ 记进**该目标**的待发槽、**不**置 `cancel_requested` |
| **S2** | `lib.rs` `builtin_task_cancel_parent` | **不再提前返回** ⇒「推迟自己」与「取消子树」拆开;子树改**逐目标**投递 |
| **S3** | `runtime.rs` `cancel_siblings_in_scope_with_reason` | 走 S1 的逐目标投递 ⇒ 兄弟失败连带**受屏蔽保护**(今天直接写 flag) |
| **S4** | `runtime.rs` `force_cancel_scope_children`(新)+ `lib.rs` `pop_scope` | 作用域退出的**硬清理** ⇒ 非终态且(仍在屏蔽块内 或 已被请求取消)的子任务在此收尾 |
| **S5** | `lib.rs` `builtin_task_is_cancelled` | 第三条路径(父作用域 `cancelled`)**尊重本任务的屏蔽状态** ⇒ 本任务在屏蔽块内时该路径不参与判定 |
| **S6** | `lib.rs` `shield_exit` | 释放点 = 屏蔽块退出 —— **X-02a 已就位**,未改 |
| **S7** | — | **不新增任何取消检查点** |

**读数**(改动后二进制实跑;改动前的读数见 §2):

| 形态 | 观察点 | 改动后 |
|---|---|---|
| 跨任务 `TASK_CANCEL(handle)` 命中**被屏蔽**的目标 | 屏蔽块内 / 退出后 | **`no` / `yes`** ⇒ 推迟,释放点 = 屏蔽块退出 |
| 跨任务 `TASK_CANCEL(handle)` 命中**未屏蔽**的目标 | 目标读数 | **`yes`** ⇒ 立即生效(不变) |
| 被屏蔽的任务发 `TASK_CANCEL_PARENT()` | 同域未屏蔽的兄弟 | **被立即取消** ⇒ `AWAIT(兄弟)` 得到 `ERR(kind="Cancelled")` |

⚠ **单任务**形态**逐字不变**(H.4.4 金标准 `FALSE` / `TRUE` / `1`)。

**⚠ 三条裁决里,只有 Q1 有可观察效果** —— 这是落地时查明的:

* **Q1(按目标)** ⇒ **可见**:跨任务取消对被屏蔽的目标**推迟**;同域兄弟**立即**被取消。
* **Q2(只有作用域退出豁免)** ⇒ 在 **S4 存在**的前提下**与「兄弟连带也豁免」观察等价** ——
  兄弟失败连带只在**作用域退出**那一刻触发(并发册 §5.4.1 的兄弟失败路径在 `SCOPE`
  出口),那时被屏蔽的兄弟无论如何都会被 S4 收尾 ⇒ ⇒ 两口径的**最终状态相同**。
  ⇒ Q2 甲案是**防御性**的选择(为「将来兄弟连带在作用域退出之外触发」留口径)。
* **Q3(帧口径)** ⇒ 表现为**不改 `YIELD` 那一支**(今天的实测读数不变)⇒ 本身不产生新可见面。

**⚠ S4 的可达性也查明了**:`AWAIT` **不会**停泊调用方(它把调度器跑到底,拿不到值就报
`E0053`),阻塞型通道 park 又会**展开**屏蔽帧 ⇒ ⇒ **「被屏蔽且停泊」的任务在作用域退出
时不存在** ⇒ S4 的「仍在屏蔽块内」那一支是**防御性**的;真正可达的是
「**已被请求取消**且**仍停泊**」⇒ 那些任务在作用域退出时被终止,而不是继续停泊。

**既有契约表的更新**(`x02a_shield_per_task_contract.rs`):判据切换让其中 **3 条**的
期望或程序失效 ⇒ 逐条判定「这条期望**还代表规范吗**」⇒ 结论都是**过时**(规范侧口径
变了)⇒ 改成新口径下的期望 + 在各自注释与文件头写明**为何变**:
`b`(程序改写:发起方不再发 `TASK_CANCEL_PARENT()`)、
`d`(改为钉**新口径**本身:被屏蔽的发起方 ⇒ 同域未屏蔽的兄弟立即被取消)、
`g`(期望 `1` → `0`:跨任务取消打**被屏蔽**的目标 ⇒ 按目标判定 ⇒ 推迟)。

**规范侧**(§6.8):并发册 §17.3 的**实现注**改写 + 一致性册 §5.4 **新增一条登记**
(「作用域退出 = 硬清理,豁免于 `SHIELD`」) + **X-02 行的收敛注记** ⇒ **规范正文不动**。

**契约与负向验证**:
* 新表 `impl/crates/wlwl-eval/tests/x02b_arrival_time_contract.rs` **3 例**
  (命中被屏蔽目标 ⇒ 推迟 / 命中未屏蔽目标 ⇒ 立即 / 被屏蔽的发起方 ⇒ 同域未屏蔽兄弟
  立即被取消)。
* ⚠ 既有 `x02a_shield_per_task_contract.rs` 有 **3 条**因判据切换而失效 ⇒ 逐条判定
  「这条期望**还代表规范吗**」⇒ 均为**过时**(规范侧口径变了,**不是**判据被放宽)⇒
  改期望/改程序 + 写明**为何变**。
* 另补 **2 条 S4 白盒守卫**(`wlwl-eval` `runtime.rs` 单测)—— ⚠ `SHIELD` 那一支在
  用户程序里**不可达** ⇒ 只有「已被请求取消 + 仍停泊」这一支能被观测 ⇒ 在调度器层
  直接构造状态来钉。
* **负向验证**(回退实现改动):`x02b` **2 条红**(`a` `[1, 1]` / `c` `0`;`b` 非判别⇒绿)、
  `x02a` **2 条红**(`d` `0` / `g` `1`)⇒ 断言钉得住、**不是恒真** ⇒ 还原后全绿。
* **全量门禁**:**90 个测试目标 / 2899 例 / 0 失败**;clippy / fmt / rustdoc ⇒ 全 0;
  规范守卫 ⇒ 绿;`check-appendix-a` ⇒ **OK**;合并册重建 ⇒ **幂等**。

## 10. 复跑命令

```powershell
# 探针(仓外 TEMP,不入库)
$exe = 'D:\Project\wlwl\impl\target\debug\wlwl.exe'
$dir = Join-Path $env:TEMP 'wlwl_probe_20261023'
& $exe run (Join-Path $dir 'x02_a_cross_task_cancel.wll')   # ⇒ a: in-shield / a-cancelled
& $exe run (Join-Path $dir 'x02_b_self_cancel_in_shield.wll')
& $exe run (Join-Path $dir 'x02_c_cancel_after_yield_in_shield.wll')
& $exe run (Join-Path $dir 'x02_d_nested_yield_loss.wll')
& $exe run (Join-Path $dir 'r4_sibling_fate.wll')     # ⇒ 0(Q1 的现状基线)

# 门禁(实施批次才需要)
cd D:\Project\wlwl\impl
cargo test --locked --workspace --all-targets --no-fail-fast
cargo clippy --locked --workspace --all-targets -- -D warnings
```

## 11. 相关文件

| 文件 | 关系 |
|---|---|
| `impl/crates/wlwl-eval/src/lib.rs` | `builtin_shield` / `builtin_task_cancel_parent` / `builtin_yield` / `builtin_task_is_cancelled` / `run_one_task` / `invoke_closure` / `shield_depth` 字段 |
| `impl/crates/wlwl-eval/src/runtime.rs` | `request_task_cancel_with_reason`(汇聚点)/ `cancel_scope_subtree_with_reason` / `cancel_siblings_in_scope_with_reason` / `cancel_suspended_task`(**未接线**) |
| `impl/crates/wlwl-eval/src/task.rs` | `Task`(per-task 运行状态的落点) |
| `impl/crates/wlwl-eval/src/yield_split.rs` | 段切分(含「嵌套 FUN 体内的 `YIELD`」的判定) |
| `impl/crates/wlwl-cli/tests/scheduling_semantics.rs` | H.4.4 金标准(`FALSE` / `TRUE` / `1`) |
| `impl/crates/wlwl-cli/tests/concurrency.rs` + `impl/tests/concurrency/*.wll` | 既有 `SHIELD` / `YIELD` 契约(**全部是单任务** ⇒ 跨任务形态无既有守卫) |
| `docs/adr/0017-cooperative-suspension-scheduler.md` | Step 3(真 state-machine 续跑)**未立项** |
| `docs/active/v1.0-residual-batch.md` §6 | 上游处置册对 X-02 的说明(本文是其展开) |
