# ADR-0034 — `std.ai` / `std.agent` 降级为官方包:机制选型与命名

| | |
|---|---|
| **Status** | **Accepted**(2026-10-15,业主「采纳建议」采纳 (c) 与 `ai:` / `agent:` 命名) |
| **Date** | 2026-10-15 |
| **Deciders** | Li(project lead) |
| **Related** | [`ADR-0023`](0023-stdlib-stability-policy.md) §v0.x 1(可见面变更须四件套同步)、[`ADR-0021`](0021-stdlib-layering.md)(R1/R2 分层)、[`ADR-0022`](0022-value-passthrough.md)、[`addendum-14-ai-agent-demotion.md`](../plan/addendum-14-ai-agent-demotion.md) §3.1 / §3.2 / §3.3 / §6、`docs/spec/wlwl-spec-v0.11.md` §9.2(IMPORT 四形态)/ §9.4(清单)、`wlwl-toml/src/manifest.rs`(`Package` / `resolve_namespace`)/ `mvs.rs` / `lock.rs`、规范 §14(「`std.ai` / `std.agent` 降级为官方包」) |

## Context

`addendum-14` 要把 `wlwl:std.ai` / `wlwl:std.agent`(各 5 成员)从 `wlwl:std.*` 移出,
让 std 回到「语言通用设施」的定位。规范 §14 已登记这条方向。

方案原本标注的**头号阻塞**是「官方包机制不存在」。**2026-10-15 的只读核实推翻了它**,
本 ADR 的第一件事就是把那个前提纠正过来 —— 因为机制选型的**代价评估**建立在它上面。

## 1. 核实:「官方包」机制**已经存在且可用**

`addendum-14` §3.1 原文:「**没有「包」的概念** —— 没有版本、没有依赖声明、
没有独立的分发单元」;并据此建议 (a) 独立 crate,警告它「连带『包机制』这个更大的问题,
**可能超出本份**」。**三样东西都在:**

| 原稿说「没有」 | 实测 | 出处 |
|---|---|---|
| 版本 | `[package] name / version / entry / description / license` | `manifest.rs:50` `struct Package` |
| 依赖声明 | `[dependencies]` + **MVS semver 求解**(精确 / `^` / 比较子 / 逗号 AND) | `manifest.rs:106`、`mvs.rs` |
| 独立分发单元 | `[namespaces] <ns> = <dir>`,模块在 `<dir>/<name>.wll` | `manifest.rs:108` / `resolve_namespace:827` |
| 锁文件 | `wlwl_toml::lock::hash_dependency_dir` | `lock.rs` |

**功能验证(带对照组,不是读代码推断)**:目录里只有 `wlwl.toml` + `vendor/greet/utils.wll`,
`[namespaces] greet = "vendor/greet"`,入口 `IMPORT("greet:utils", ["greet"])`:

| | 结果 |
|---|---|
| 有 `wlwl.toml` | 打印 `hi from a wlwl.toml namespace`,rc=**0** |
| **挪走 `wlwl.toml`** | `E0043 … not registered`,rc=**1** |

⚠ **`addendum-14` §6 的「明确不做:引入完整包管理(版本 / 依赖声明 / 语义化版本)」
也随之过期** —— 那三样**已经在仓里**,不需要引入,也不该再去「引入」一遍。

## 2. 硬约束:`wlwl:` 是保留前缀,第三方命名空间**不能**用它开头

这是 §3.3 命名裁决里最要紧的一条,而它**不在**原方案的任何一个候选里。实测:

| 路径 | 结果 |
|---|---|
| `wlwl:ai`(最直觉的候选) | ❌ **E0040**,且错误显示它把 `wlwl:ai` 当**文件名**去找(`…\wlwl:ai.wll`) |
| `wlwl:std.rand`(同为 `wlwl:` 前缀) | ✅ `std namespace reachable` |

机制在 `wlwl-eval/src/lib.rs` 的 `resolve_source`:第 1 步只认 `wlwl:std.X`;
第 2 步 `parse_ns_path` 对 **`wlwl:` 开头的路径直接返回 `None`**(注释原文:「std;
handled above」)⇒ 它落到第 4 步「裸模块名」分支,被当成文件名。

⇒ **`wlwl:ai` / `wlwl:agent` 两条最直觉的命名方案结构上不可行**,必须换前缀。

## Decision

### D1 · 机制选 **(c)**(路线 c:R1 模块 + `wlwl.toml` `[namespaces]` 声明的命名空间)

**为什么 (c) 而不是 (a)**:(a) 与 (c) 的差距已从「造机制 vs 不造」缩成
「crate 边界 vs 清单边界」—— 机制那一半(c)已经验证可用。而 `std.ai` / `std.agent`
本就是 **R2 成员**(受 `real-ai` feature 门控),走清单声明的命名空间与它们的形态
天然吻合:「不在 `wlwl:std.*` 里」正是降级的全部可见面变化。

**按 `addendum-14` §4 自己给的判据,(c) 是「真搬」**:`resolve("wlwl:std.ai")` 必须
返回 **`None`**(不是空 `SPEC`)—— 空壳会让 `resolve()` 仍返回它,而那正是「假搬」。

⚠ **形态上的差距,如实记下**:(c) 下包**仍以 `wlwl-std` 的 `real-ai` feature 编译**,
只是**不再出现在 `wlwl:std.*` 的可见面里**。它**没有**成为独立的分发产物
(独立的 crate / 独立的 `wlwl.toml` 随二进制分发)。这是 (c) 与 (a) 的真实差别,
不是话术。⇒ 若日后要求「一个 `std.*` 之外的包能脱离本仓单独分发」,那是 (a),
**另立一项**,不在本 ADR 范围内。

### D2 · 命名取 **`ai:` / `agent:`**(沿用 §3.3 要求的「提名 / 理由 / 落选者」体例)

| 候选 | 判决 | 理由 |
|---|---|---|
| **`ai:ask` / `agent:tool`** | ✅ **采纳** | 短;与包的**自身名字**一致(`[package] name = "ai"`);与 std 形态正交(`wlwl:std.rand` 用点、`ai:ask` 用冒号,一眼可分);`resolve_namespace` 的键就是 `<ns>` 本身,`ai` 直接可用 |
| `wlwl:ai` / `wlwl:agent` | ❌ **结构上不可行** | `wlwl:` 是 std 保留前缀,实测 E0040(见 §2) |
| `wlwl:pkg.ai` / `wlwl:pkg.agent` | ❌ 落选 | 同上,`wlwl:` 前缀不可用 |
| `pkg:ai` / `pkg:agent` | ❌ 落选 | 引入了一个本仓**并不存在**的概念名 `pkg`,而真实机制里没有「pkg」这一层 —— 名字会撒谎 |
| `std.ai2` / `std.agent2` | ❌ 落选 | 仍在 `std` 里,与「移出 std」的整份方案自相矛盾 |

**迁移写法**(CHANGELOG 必须给出,`addendum-14` §6 已要求):

```wlwl
; 旧
IMPORT("wlwl:std.ai", ["ASK"]);
; 新
IMPORT("ai:ask", ["ASK"]);      # 需项目 wlwl.toml 声明 [namespaces] ai = "…"
```

### D3 · 可见面变更的申报口径(沿 §3.2,本 ADR 只确认不扩写)

成员的**名字 / 签名 / 语义一字未改**,但**可见面**变了 ⇒ 按 `ADR-0023` §v0.x 1
属「**需要申报的可见面变更**」;因**不是移除成员**,故**不走弃用两步**(警告码 +
保留一个次版本)。⚠ CHANGELOG 必须**明写新命名空间与迁移写法**,不能只写「已移动」。

## Consequences

**能拿到的**:`wlwl:std.*` 回到 11 命名空间 / 94 成员;§3.1 的头号阻塞解除;
`addendum-14` 的 W-01 / W-02 两项裁决落地。

**明确不做**(`addendum-14` §0):不扩语法 / 不改任何成员语义 / **不把 `std.rand`
顺带搬走**(那是 `08-std-rand` 自己的 ADR A6,两份不捆绑)/ 不引入第三方包机制
(已有一部分,见 §1)/ 不砍功能。

**留给实现阶段(W-03 起)**:门禁命令随 `real-ai` 的归属改动而改
(`addendum-14` §3.4,三处:计划 §3.3 完成定义 / README 门禁行 / skill 门禁说明);
skill 的 `reference.md` §9.x 编号顺延须**全文 grep 复核**(§6 已警告 v0.11.3 犯过
一次失效交叉引用)。

⚠ **本 ADR 只解 W-01 / W-02**(裁决)。实现(W-03 ~ W-07)未动一行代码。
