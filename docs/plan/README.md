# docs/plan

本目录留给**当前迭代**的构建计划与偏差登记。

2026-09-23 v0.8.1 归档后,v0.8 / v0.7 及更早材料在 `docs/history/`:

| 归档件 | 路径 |
|--------|------|
| 语言规范 v0.6 | `../history/wlwl-spec-v0.6.md` |
| v0.7 构建计划(COMPLETED) | `../history/wlwl-build-plan-v0.7-COMPLETED.md` |
| Phase B 子计划 | `../history/wlwl-phase-b-implementation-plan.md` |
| 偏差登记 v0.7 分册 | `../history/deviations-v0.7.md` |
| 语言规范 v0.7 | `../history/wlwl-spec-v0.7.md` |
| v0.8 构建计划(COMPLETED) | `../history/wlwl-build-plan-v0.8-COMPLETED.md` |
| **v0.8.1 构建计划(COMPLETED)** | `../history/wlwl-build-plan-v0.8.1-COMPLETED.md` |
| **v0.8.1 第三方审计源材料** | `../history/audit-report-v0.8.1.md` |
| 偏差登记 v0.8 分册(含 v0.8.0 全部 D8-NNN + v0.8.1 D8-009..D8-014) | `../history/deviations-v0.8.md` |
| 既有日次 / 阶段史 | `../history/2026*.md`、`p3-011-spec-alignment.md` |
| 早期计划 | `../history/wlwl-build-plan-v0.1-COMPLETED.md`、`wlwl-build-plan-v0.2.md` |

**现行规范**:`../standard/wlwl-spec-v0.8.md` (v0.8.1 patch 无 spec 改动)。
新一轮迭代请在本目录新建 `wlwl-build-plan-v0.9.md` 与空白 `deviations.md`。

**v0.8.1 patch 周期回顾** (commit 链,本地 ahead of origin/main 6 个):

| Commit | D8 | 类型 | 改动 |
|---|---|---|---|
| `939c59a` | D8-009 | impl | 浮点指数字面量(§1.7) |
| `3b4db19` | D8-010 | impl | SUB length 语义(§10.5,**唯一 observable change**) |
| `9c1ab6b` | D8-011 | impl | MUT 作普通标识符(§1.4,5 处 dispatch) |
| `29f3130` | D8-012 | impl | 字符串字面量 postfix(§A.2) |
| `8ee8c77` | D8-013 | docs | closure_cell.wll §3.3 对齐 + v07_fidelity baseline 更新 |
| `a7392e7` | D8-014 | docs | §1.8 内嵌字符串备忘(留 v0.9) |

**测试累计**:eval 754 → 767 (+13) ;parser 80 → 89 (+9) ;workspace ~1400 项 0 failed。

详见 `../history/deviations-v0.8.md` D8-009..D8-014 段(约 800 行)。
