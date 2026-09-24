# v0.9 偏差登记 (deviations)

> **维护说明**:本文件是 v0.9 实施期间的活跃偏差登记(沿用 v0.7 / v0.8 周期惯例,
> 周期结束后整体迁入 `docs/history/`)。
>
> v0.9 release commit 时,本文件改名保留为 `docs/history/deviations-v0.9.md`
> 的形式(已经在),并起一份 `docs/plan/deviations.md` 走 v0.9.1 周期。
>
> **编号约定**:`D9-NNN`,按发现/登记顺序连续递增。
> 每条 deviation 包含:发现时机、影响范围、现象、处置选项、与 v0.8 兼容性的破坏程度。
>
> 章节号引用基准:`docs/standard/wlwl-spec-v0.9.md`(wip0.9 wip;v0.9.0 release 时
> spec v0.9 文本作为 baseline 冻结)。
>
> impl 工作集:`impl/crates/wlwl-eval/src/{lib,runtime,protocol,channel,task}.rs` +
> `impl/crates/wlwl-eval/src/registry.rs::BUILTIN_REGISTRY`。

---

## D9-001 · spec §15.1 容器越界与 §14.3.2 `E0050` after-end 区分未实装

- **状态**:已修复(2026-09-25)
- **发现时机**:wip0.9 阶段 Step 12 子阶段 2 锁测试覆盖盘点(2026-09-25)
- **关联 commit**:
  - `2f2901e` feat(impl): v0.9 Step 9 quality + 9b session types (P0/P1 + protocol)
    — 该 commit message 声明 "P1: THIS escape checks (ARRAY/DICT/FUN/SPAWN/return) fire E0032";
    实际只实装 **THIS outside method body** 一条路径,ARRAY / DICT / FUN / SPAWN / return
    五种容器越界 impl **未跟进**;
  - `1ede6bb` docs(spec): v0.9 Step 12 sub-phase 1 — §13 / §14 / §15 / §16 OOP spec 字面
    — 该 commit 把 spec §15.1 三类越界边界(容器 / call / AWAIT)+ §14.3.2
    `E0050` priority-on-state-machine-terminal 落到文字,但未关联新 impl 提交。
- **影响范围**:impl `wlwl-eval/src/lib.rs::builtin_this` 与相关边界检查器;
  spec v0.9 §13 / §14 / §15 锁测试覆盖表中 ~5 项 lock-test 命名条目缺位。
- **历史编号**:续 v0.8 D-002(THIS 越界偏差,在 v0.8 §17.4 注记)。

### 现象

| spec 章节 | 锁测试期望 | 当前 impl 行为 |
|-----------|----------|---------------|
| §15.1 (容器) | `LET(_d, [this: THIS])` inside self → `E0032` | 不报 `E0032`(`LET(_d, ...)` 落进 DICT 字面量,未触发 THIS 越界) |
| §15.1 (容器) | `LET(_a, [THIS])` inside self → `E0032` | 不报 `E0032` |
| §15.1 (闭包) | `LET(_g, FUN(() , THIS))` inside self → `E0032` | 不报 `E0032` |
| §15.1 (return) | 函数字面量返回 `THIS` → `E0032` | 不报 `E0032` |
| §14.3.2 (`E0050` priority) | `CALL_METHOD` 协议状态机 `Done` → `E0050` | 命中 `E0051`(顺序错位),非 `E0050`(状态不符) |

仅 spec §15.1 "outside method body" 一条已有 `v09s9a3_this_outside_method_body_returns_e0032`
覆盖(impl 在 `lib.rs:3476-3478` / `lib.rs:6328-6332` 注释中明确);其余 5 项边界
与一个 `E0050` priority 区分 impl 缺口。

### 处置选项

- **选项 A (推荐)**:在 Step 12 子阶段 2 impl 阶段继续跟进,**6 项锁测试**作为
  acceptance gate。`v09s12_*` 命名锁测试在 spec §13 / §14 / §15 落地时已经起草
  (见本文档的 commit 引用),impl 跟上后可直接启用。
- **选项 B**:v0.9.0 release 时,spec §15.1 容器 / return 越界与 §14.3.2 `E0050`
  priority 落地作为 v0.9.1 子集;v0.9.0 release commit 简化为"spec §15 impl
  收口到 outside-method-body only",§15.1 / §14.3.2 全文为 v0.9.1 文字,
  wip0.9 spec v0.9 章节号保持冻结(§13-§16 不重新编号)。
- **选项 C (不建议)**:移除 spec v0.9 §15.1 容器 / return 边界与 §14.3.2
  `E0050` priority 文字,等同于放宽语言保证。**违背** §9.1 Step 12 子阶段 2
  字面要求 + ADR-0019 §4.4.3 草稿 2 章节号冻结规则。

### 与 v0.8 兼容性的破坏程度

| 当前 v0.8 impl 行为 | v0.9 spec 期望 | 破坏程序可见行为? |
|--------------------|----------------|--------------------|
| THIS 在 DICT / ARRAY 内不报错 | `E0032` | **是**(原程序"将 THIS 存为字段"在 v0.9 编译失败);但 v0.8 历史上没有"跨字段保留 THIS" 的合理用例 |
| 协议状态机 Done 后继续调用返 E0051 | `E0050` | **否**(仅错误码差异,§11.2.4 priority 决定谁先抛;`E0050` 与 `E0051` 都属 v0.9 新启用,旧程序不依赖) |
| THIS 跨 AWAIT 边界 | `E0032` | **是**(spec §16.2 收紧;原 v0.8 行为未约束),Step 12 子阶段 2 留 P2 |

### 与 v0.9 / v0.8 兼容性的承诺

- v0.8 程序**不**依赖 §15.1 容器 / return 宽松行为 → 在 v0.9 上行为不变;
- v0.8 程序**不**依赖 `E0050` / `E0051` 触发路径 → 在 v0.9 上行为不变;
- v0.8 程序若依赖 §15.1 跨 AWAIT 边界宽松(实测稀见) → v0.9 D9-001 闭合后行为改变。

---

## D9-002 · `BUILTIN_REGISTRY` 签名字符串 wip0.9 中间状态

- **状态**:已修复(2026-09-25)
- **发现时机**:Step 11 子阶段 2 跑 `gen-appendix-g` 重生成时(commit `dbf849d`)
- **影响范围**:`impl/crates/wlwl-eval/src/registry.rs::BUILTIN_REGISTRY` 内的
  函数签名 `signature: String` 字段(impl `registry.rs::BuiltinSpec` 结构);

### 现象

`gen-appendix-g` 跑出来的 wip0.9 状态产物(`docs/appendix_G.md`)+ spec v0.9
附录 G 嵌入段(`wlwl-spec-v0.9.md`)里,以下签名仍按 v0.8 字面展示:

- `TASK_CANCEL(task) -> NULL`(应当 `TASK_CANCEL(task, reason?)`);
- `TASK_CANCEL_PARENT() -> NULL`(应当 `TASK_CANCEL_PARENT(reason?)`);
- `CHANNEL_SEND(ch, v) -> NULL / ERR(ChannelWouldBlock)`(`ChannelWouldBlock`
  路径已删除(§17.2.6));
- `CHANNEL_RECV(ch) -> v / ERR(ChannelClosed|ChannelWouldBlock)`;
- 注册表 §11.2 / §10.11 prose 字面未文本化扩 reason 可选参数。

错误码触发路径层面已对齐 v0.9(impl 处的 `E0055` / `E0057` 移除 + `E0065` /
`E0066` / `W0065` 注册已有锁测试覆盖),但 **签名文本字符串** 未跟进。

### 处置选项

- **选项 A (推荐)**:Step 12 子阶段 2 落地时同步改 `registry.rs::BuiltinSpec.signature`
  字段为 `TASK_CANCEL(task, reason?)` / `CHANNEL_SEND(ch, v) -> NULL`(移除
  `ChannelWouldBlock` 字符串)等;然后再跑一次 `gen-appendix-g` 重生成产物。
- **选项 B**:v0.9.0 release 时 spec §17 / 附录 G 文字与 wip 中间产物在 release
  commit 中"v0.9 release 收口"一起合入,统一 release commit 改字符串 + 跑
  `gen-appendix-g` 一次。

### 与 v0.8 兼容性

签名文本不进入用户可见行为;registry 字段差仅在 `gen-appendix-g` 产物上显形。
无程序兼容性影响。

---

## D9-003 · `.github/workflows/ci.yml` 触发仅匹配 `branches: [main]`

- **状态**:已知(2026-09-25)
- **影响**:wip0.9 push 不触发 CI gate,Step 10 / Step 11 / Step 12 子阶段 1
  提交到 origin 的 `wip0.9` 推送无 GHA workflow run 反馈。

### 现象

`ci.yml` line 5 / line 7:

```yaml
on:
  push:
    branches: [main]
  pull_request:
    branches: [main]
```

`GET /repos/:owner/:repo/actions/runs?branch=wip0.9` 返回 `{ total_count: 0,
workflow_runs: [] }`(2026-09-25 验证)。wip 分支的 commit 走 GHA only by
PR / merge to main 流程。

### 处置选项

- **选项 A (本次决议 2026-09-25)**:留 wip0.9 wip 期间不开 CI gate;依赖本地
  fmt + clippy + test 三道闸 + 主分支 push 时跑 CI。wip0.9 上累计 4 commit
  (`5997c93` / `a2233ff` / `dbf849d` / `1ede6bb`)在 commit 时各自跑本地绿。
- **选项 B**:改 `ci.yml` 的 `on:push.branches` 加上 `wip0.9`。每 wip commit
  跑 fmt + clippy + test gate。增加 GHA 用量。
- **选项 C**:合并 wip0.9 到 main(会与 main 历史红 Run 129-131 冲突,需先
  修 main CI 才能 fast-forward)。

### 与 v0.8 兼容性

无程序兼容性影响;仅 CI 反馈时效。

---

## 后续动作(2026-09-25 wip0.9 阶段评估)

- D9-001 **已闭合**(2026-09-25):`v09s12_*` 8 项锁测试落地 —
  ARRAY / DICT / FUN free-var / SPAWN capture / AWAIT / SET_PROP /
  method return / alias return 全部 `E0032`;`ProtocolError::Exhaused`
  映射改为 `E0050`(spec §14.3.2 priority)。
- D9-002 **已闭合**(2026-09-25):`registry.rs` 四条签名文本更新
  (`TASK_CANCEL(task, reason?)` / `TASK_CANCEL_PARENT(reason?)` /
  `CHANNEL_SEND` 无 `ChannelWouldBlock` / `CHANNEL_RECV` 仅
  `ChannelClosed`);`gen-appendix-g` 重生成;spec v0.9 附录 G 嵌入段同步。
- D9-003 在 wip0.9 wip 阶段留原状;v0.9 release 前评估是否改 `ci.yml`。

---

> wip0.9 D9-001 + D9-002 已全部闭合(2026-09-25)。
