# docs/plan

本目录留给**当前迭代**的构建计划与偏差登记。

**当前进行中:v0.10.2 规范符合性修复批次**(第三方黑盒合规审查的核验与修复)。
来源报告 [`../reviews/v0.10.1审查/wlwl-spec-v0.10-合规审查报告.md`](../reviews/v0.10.1审查/wlwl-spec-v0.10-合规审查报告.md)、
计划书 [`wlwl-spec-compliance-修复计划.md`](wlwl-spec-compliance-修复计划.md)。
v0.10.0 与 v0.10.1 已收口,材料全部在 `docs/history/`。

## 当前状态(2026-09-30)

| 项 | 状态 |
|---|---|
| 现行规范 | [`../standard/wlwl-spec-v0.10.md`](../standard/wlwl-spec-v0.10.md)(v0.10) |
| 最新实现 | v0.10.1 已发布;**v0.10.2 批次进行中** —— 规范**版本号不变**,改的是规范文本与实现对齐 |
| 进行中的计划 | [`wlwl-spec-compliance-修复计划.md`](wlwl-spec-compliance-修复计划.md) —— **已完成**,已发布为 `v0.10.2` |
| 门禁 | `cargo fmt --check` 0 diff;`clippy -D warnings` 0;`cargo test --workspace` 全绿;probe **129** 条(本轮 +28) |
| 下一迭代 | v0.11 —— 唯一已立项的是 **`YIELD` 的续体保存**(裁决 D-3 = 拆两半,规范侧已在 v0.10.1 降级为已知限制) |

v0.10.2 收口后本目录**没有进行中的迭代**;下一次迭代(v0.11)开工时,新的构建计划
与偏差分册(`deviations-v0.11.md`,编号从 **D11-001** 起)放回本目录。

## v0.10 / v0.10.1 归档件

| 归档件 | 路径 | 状态 |
|---|---|---|
| v0.10.0 构建计划(Static Contracts) | [`../history/wlwl-build-plan-v0.10-COMPLETED.md`](../history/wlwl-build-plan-v0.10-COMPLETED.md) | 已发布 |
| **v0.10.1 修复计划**(源自 v0.10.0 两份 REVIEW) | [`../history/wlwl-v0.10.1-修复计划.md`](../history/wlwl-v0.10.1-修复计划.md) | ✅ 13 个 Step 全部落地 |
| v0.10 技术路线建议(范围裁决上游) | [`../history/wlwl-v0.10.0-迭代技术路线建议.md`](../history/wlwl-v0.10.0-迭代技术路线建议.md) | 已消费 |
| v0.9 构建计划(FINAL / 实施基线) | [`../history/wlwl-build-plan-v0.9-COMPLETED.md`](../history/wlwl-build-plan-v0.9-COMPLETED.md) | 已发布 |
| **偏差登记 v0.10 分册**(`D10-001`…`D10-018`) | [`../history/deviations-v0.10.md`](../history/deviations-v0.10.md) | 活跃至 v0.10.1 收口 |
| v0.10.0 两份 REVIEW(动态 97 probe / 静态 4 路) | [`../reviews/`](../reviews/) | — |

**归档命名约定**:`wlwl-build-plan-vN.N-COMPLETED.md` 表示该构建计划已实施完毕
(见 v0.7 / v0.8 / v0.8.1 的先例);非构建计划类文档(路线建议、修复计划、审计报告)
保持原名。

## 更早的归档件

| 归档件 | 路径 |
|---|---|
| 语言规范 v0.6 – v0.9 | `../history/wlwl-spec-v0.{6,7,8,9}.md` |
| v0.7 / v0.8 / v0.8.1 构建计划 | `../history/wlwl-build-plan-v0.{7,8,8.1}-COMPLETED.md` |
| Phase B 子计划 / 早期计划 | `../history/wlwl-phase-b-implementation-plan.md`、`wlwl-build-plan-v0.{1,2}*.md` |
| 偏差登记 v0.7 – v0.9 分册 | `../history/deviations-v0.{7,8,9}.md` |
| v0.8.1 第三方审计源材料 | `../history/audit-report-v0.8.1.md` |
| 日次 / 阶段史 | `../history/2026*.md`、`p3-011-spec-alignment.md` |

## 下一迭代开工时要做的两件事

1. 建 [`../history/deviations-v0.11.md`](../history/deviations-v0.11.md) 偏差分册,
   编号从 **D11-001** 起(**不要**接着 D10 往下数 —— 台账按周期分册,跨周期续号
   会让「这个偏差属于哪个版本」变得不可查)。
2. 把 v0.10.1 的**未完成项**接过来,按 `../reviews/` 与
   `../history/wlwl-v0.10.1-修复计划.md` 的记录逐条确认状态。
