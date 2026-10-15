# ADR-0034 — `std.ai` / `std.agent` 降级为官方包:机制选型与命名

| | |
|---|---|
| **Status** | **D2 已定**(命名取 `ai:` / `agent:`);**D1 于 2026-10-15 当日撤回,待重裁** —— 见 D1 的撤回框 |
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

## 2b. 第二条过期基线:`real-ai` **不门控成员**,只门控 HTTP 传输

`addendum-14` §1 写「`std.ai` 的 5 个成员受 `real-ai` 门控,默认构建**不可见**」。
**实测相反**(2026-10-15,默认构建):

```
IMPORT("wlwl:std.ai", ["ASK","EMBED","COMPLETE","ASK_STREAM","ASK_ALL"]);
⇒ 5 个成员全部绑定成功(唯一报错是 ASK 元数不对 —— 名字已解析、函数存在)
```

真实形状:`wlwl-std/Cargo.toml` 的 **`real-ai = ["dep:reqwest"]`** —— 它门控的是
**HTTP 传输层**(mock ↔ real);`src/ai.rs` 的 `#[cfg(feature = "real-ai")] mod real`
只包住真实后端,**成员函数本身一直编译**。

⇒ **两条下游结论随之作废**:

1. `addendum-14` §3.4「`real-ai` 移出后门禁命令会失效」—— **在 D1 选的 (c) 下不成立**。
   crate 仍是 `wlwl-std`、feature 仍挂在它上面,`cargo check -p wlwl-std --features
   real-ai` **继续有效** ⇒ **W-07 的「门禁命令三处同步」在 (c) 下不需要做**,
   计划 §3.3 完成定义 / README 门禁行 / skill 门禁说明三处**都不动**。
   ⚠ (a) 路线下这条才会成立 —— 这是 (c) 与 (a) 的一处**具体**代价差。
2. `addendum-14` §4 契约表的「默认构建下成员**不可见**;带 feature 可见」—— 当前
   **恰恰相反**,而 §0 的非目标写明「不改任何成员的语义」⇒ 搬完之后成员
   **应当仍然在默认构建下可见**。契约要照**现状**写,**不是**照初稿。

## Decision

### D1 · ~~机制选 **(c)**~~ ⛔ **D1 于 2026-10-15 当日撤回,待重裁**

> 🔴 **撤回理由(同日核实,补上了上一轮漏查的那一点)**
>
> 上一轮选 (c) 的依据是「清单机制已存在且功能验证可用」。但那个验证用的是
> **R1 `.wll` 模块**(`vendor/greet/utils.wll`),而 **`std.ai` / `std.agent` 是
> **R2 原生**(真 HTTP 走 `reqwest`,不可能改写成 `.wll`)。
>
> **清单机制到不了原生后端**:
> - `wlwl-eval/src/lib.rs:448` `enum ResolvedSource` 只有两个变体:
>   `Std(wlwl_std::StdBackend)` 与 `File { path, name }`;
> - `Std` **只**由 `wlwl_std::resolve(path)` 填充,那是 `wlwl-std/src/lib.rs:401`
>   一张**硬编码的 `wlwl:std.*` 匹配表**;
> - 清单里**没有任何**指向原生后端的字段(`allow_builtin_shadow` 等是无关的特性开关);
> - `ns:name` 分支(`lib.rs:551`)无条件构造 `<dir>/<name>.wll` 并返回 `File`。
>
> ⇒ **一个 `wlwl.toml` 包只能是磁盘上的 R1 源文件**,没有任何路径能让它到达
> 原生 Rust `SPEC`。**⇒ (c) 对 `std.ai` / `std.agent` 不成立。**
>
> ⚠ **教训**:上一轮的核实验证了「机制能不能用」,却**没验证「它能不能承载这两个
> 成员」** —— 后者才是选型的前提。**机制存在 ≠ 机制适配**。

**待重裁的选项(第四种是本轮新发现的,前三份方案都没列)**:

| 方案 | 形态 | 代价 / 差距 |
|---|---|---|
| **(a)** 独立 crate | `wlwl-ai` / `wlwl-agent` 独立 crate + 注册点 | 最接近 Rust;但 `Std` 变体只由 `wlwl-std::resolve` 填充 ⇒ 需要**新增一个原生注册通道**,工作量最大的那一项 |
| **(b')** 同 crate + 换前缀 | `wlwl-std` 不变(故 `real-ai` feature **不动**),只把匹配表里的 `wlwl:std.ai` 改成 `ai:ask` 等 10 个键 | ⚠ 形态上仍在 `wlwl-std` 里,**`addendum-14` §6 的「不算真搬」条款按字面适用** ⇒ 须如实记为**部分完成**。但它**满足 §4 自己给的判据**(`resolve("wlwl:std.ai") == None`) |
| (b) 原样 | 同 crate 内只做 feature 隔离 | 命名空间仍是 `std`,与「移出 std」自相矛盾 ⇒ 已排除 |

⚠ **(a) 与 (b') 的取舍是「要不要新增原生注册通道」**,而不是「要不要造包机制」——
**包机制已经有了**,缺的是「原生包怎么被解析到」这一段。

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
