# ADR-0022 — 值直通边界(拆分 `wlwl-value`)

| | |---|
|---|---|
| **Status** | Accepted(2026-10-01) |
| **Date** | 2026-10-01 |
| **Deciders** | Li (project lead) |
| **Related** | **ADR-0021**(分层模型,本 ADR 为其实现前提)、ADR-0009(ERR 消费者注册)、`crates/wlwl-std/src/lib.rs:11-17`(被本 ADR 废止的既有契约) |

## Context and Problem Statement

`wlwl-std` 自成形起以 `serde_json::Value`(别名 `StdValue`)为跨界类型:
eval 侧在调用边界做 `Value ↔ serde_json::Value` 双向转换。该边界有三个
结构性缺陷:

1. **值域降维**:serde_json::Value 装不下 `FUNCTION` / `TASK` / `CHANNEL` /
   `CLASS` / `INSTANCE` —— 函数值过不了界,回调型 API(排序键、断言体、
   流式回调)在原生侧无法表达;
2. **双实现事实**:为绕开边界,`std.collection` / `std.test` 的实现寄居
   `wlwl-eval`(`src/collection.rs`、`src/test.rs`),`wlwl-std` 里只剩
   名字目录 —— 「同一名义、两种事实」,直接违反调用面一致要求;
3. **转换成本**:每个跨调用点双向转换,热路径无谓开销。

## Decision Drivers

- 业主 2026-10-01 裁决:值直通**必须做,纳入底座**;`wlwl-value` 只承载
  **值与句柄**,函数调用经 **trait/回调注入**,保持单向依赖;
- `wlwl-eval` 单文件约 2.4 万行 —— 拆分必须是**纯搬移**,不夹带语义改动;
- 顺序裁决:先拆 `wlwl-value`,再解除边界,最后宿主归位
  (collection/test 迁出 eval)。

## Considered Options

### A. 给 StdValue 加 Function 变体(装不透明句柄)

改动最小。但句柄的解析仍要回 eval 查表,`wlwl-std` 与 eval 的耦合只是被
藏进句柄;两种值世界(serde_json 值 + 真值)长期并存。**拒绝**。

### B. 维持边界,只做 R1 纯 wlwl,原生侧不碰回调

回避问题。混合实现(如 std.test 的断言体回调)无法成立,ADR-0021 的混合
模块形态落空。**拒绝**。

### C. 拆 `wlwl-value`,原生函数直接收发真 Value(采纳)

## Decision

1. **新 crate `wlwl-value`**,只承载三样东西:
   - `Value`:语言规范 §2.1 的 13 类值(存储布局自 `wlwl-eval` **纯搬移**);
   - `Handle`:宿主侧对象(闭包环境、类、实例、任务、通道)的不透明句柄;
   - `Callable` trait:回调注入点 —— 「收一组 `Value`,还一个 `Value`」。
2. **依赖方向单向**:`wlwl-eval → wlwl-value ← wlwl-std`。eval 为 wlwl 闭包
   实现 `Callable`(句柄 → 求值器注册表解析);std 原生函数只依赖 trait,
   对「回调背后是谁」不可知。
3. **`StdCtx` 增 Callable 注入槽**:eval 装配运行时注入;`wlwl-std` 仍不
   依赖 eval(环继续被禁止)。
4. **跨界类型只有 `Value`**:原生实现内部用什么中间表示是实现自由(如
   `std.json` 的解析内核继续用 serde_json),但**边界上**不再有任何转换层。
5. **宿主归位**:`collection.rs` / `test.rs` 迁出 `wlwl-eval`;
   `lib.rs:11-17` 的「std 边界拒绝闭包值」契约废止。`std.collection` 的
   终态是 R1 纯 wlwl(ADR-0021),`std.test` 为混合实现(R1 门面 + R2
   原生内核)。

## Consequences

- **正面**:函数值可过界,回调型原生 API 可表达;每个调用点省去两次转换;
  `wlwl-value` 成为值语义的单源定义,eval / std 共享;
- **代价**:一次纯搬移重构(自约 2.4 万行的单文件搬出),以「workspace
  锁测试全绿」为唯一闸;`Rc` 循环引用风险与现状持平(无 GC,非本 ADR 议题);
- **后续纪律**:本 ADR 落地后,**新增**原生 API 一律直通边界,不得再引入
  任何中间边界类型。
