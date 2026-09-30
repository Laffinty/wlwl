# docs/plan

本目录留给**当前迭代**的构建计划与偏差登记。

**当前迭代:v0.11**(分支 `wip0.11`)。v0.10.x 批次(含 v0.10.2 规范符合性
修复)已全部收口,材料归档至 `docs/history/`;本目录暂无进行中的计划。

## 当前状态(2026-09-30)

| 项 | 状态 |
|---|---|
| 现行规范 | [`../standard/wlwl-spec-v0.11.md`](../standard/wlwl-spec-v0.11.md)(v0.11 —— 由 v0.10 的 v2 清洗版改号而来,规范内容不变) |
| 最新实现 | v0.10.4 已发布 |
| 进行中的计划 | 无;v0.11 的构建计划与偏差分册(`deviations-v0.11.md`,编号从 **D11-001** 起)开工时建在本目录 |
| 门禁 | `cargo fmt --check` 0 diff;`clippy -D warnings` 0;`cargo test --workspace` 全绿;probe **129** 条 |
| v0.11 已立项范围 | **`YIELD` 的续体保存**(裁决 D-3 = 拆两半,规范侧已在 v0.10.1 降级为已知限制) |

## v0.10 批次归档件

| 归档件 | 路径 | 状态 |
|---|---|---|
| 语言规范 v0.10(原版) | [`../history/wlwl-spec-v0.10.md`](../history/wlwl-spec-v0.10.md) | 被 v0.11(v2 清洗版改号)取代 |
| **v0.10.2 规范符合性修复计划**(源自第三方黑盒审查) | [`../history/wlwl-spec-compliance-修复计划.md`](../history/wlwl-spec-compliance-修复计划.md) | ✅ 已完成,发布为 v0.10.2 |
| **v0.10.1 第三方黑盒审查**(报告 + README) | [`../history/v0.10.1审查/`](../history/v0.10.1审查/) | 已核验(8 属实 / 2 根因错 / 3 不成立,另 2 条漏报真缺陷) |
| v0.10.0 构建计划(Static Contracts) | [`../history/wlwl-build-plan-v0.10-COMPLETED.md`](../history/wlwl-build-plan-v0.10-COMPLETED.md) | 已发布 |
| v0.10.1 修复计划(源自 v0.10.0 两份 REVIEW) | [`../history/wlwl-v0.10.1-修复计划.md`](../history/wlwl-v0.10.1-修复计划.md) | ✅ 13 个 Step 全部落地 |
| v0.10 技术路线建议(范围裁决上游) | [`../history/wlwl-v0.10.0-迭代技术路线建议.md`](../history/wlwl-v0.10.0-迭代技术路线建议.md) | 已消费 |
| v0.10.0 两份 REVIEW(动态 97 probe / 静态 4 路) | [`../history/REVIEW-wlwl-v0.10.0.md`](../history/REVIEW-wlwl-v0.10.0.md)、[`../history/wlwl-v0.10.0-static-review.md`](../history/wlwl-v0.10.0-static-review.md) | 已消费 |
| 偏差登记 v0.10 分册(`D10-001`…`D10-018`) | [`../history/deviations-v0.10.md`](../history/deviations-v0.10.md) | 活跃至 v0.10.1 收口 |

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
| 日次 / 阶段史(2026-09-30 起按 5~10 日窗口合并精简) | `../history/20260902-09.md`、`../history/20260915-22.md`(合并前的原文在 git 历史) |

## v0.11 开工时要做的两件事

1. 在本目录建 `deviations-v0.11.md` 偏差分册,编号从 **D11-001** 起
   (**不要**接着 D10 往下数 —— 台账按周期分册,跨周期续号会让「这个偏差属于
   哪个版本」变得不可查)。
2. 把 v0.10.1 的**未完成项**接过来,按
   [`../history/wlwl-v0.10.1-修复计划.md`](../history/wlwl-v0.10.1-修复计划.md)
   与 [`../history/v0.10.1审查/`](../history/v0.10.1审查/) 的记录逐条确认状态。
