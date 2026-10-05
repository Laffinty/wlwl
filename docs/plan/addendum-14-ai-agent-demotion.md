# 14 · `std.ai` / `std.agent` 移出 `wlwl:std.*`(**减法**)

> **层级** L3 · **前置** L1 全部落地 · **状态** 未动工
> **性质** 本目录唯一的**减法**方案:不新增任何成员,只**搬**

## 0. 契约摘要

- **目标**:把 `std.ai` / `std.agent` 从 `wlwl:std.*` 移出为**官方包**,
  让 `wlwl:std.*` 回到「语言通用设施」的定位。
- **非目标**(违反即范围事故):
  - **不改任何成员的名字、签名、语义**(那是 breaking,见 §3.2)。
  - **不删功能**。本份是「搬」不是「砍」。
  - **不把 `std.ai` 的成员并入 `std.agent`**(它们是两回事:`std.ai` 是
    单次推理调用,`std.agent` 是工具 / 循环 / 上下文)。
  - **不引入「第三方包」机制**。只做「官方包」这一档(见 §3.1)。
- **为什么值得做**:这两个命名空间**绑定特定 LLM API 形态**
  (`EMBED` / `COMPLETE` / `MODEL` 的参数形状来自 OpenAI 风格),而
  `wlwl:std.*` 里的其它成员都是**语言无关**的。混着放会让 std 的信噪比下降 ——
  读到「`std.*`」时无法判断哪些是语言设施、哪些是厂商胶水。

## 1. 基线(可核验)

```powershell
cd docs
Select-String -Path stdlib/wlwl-stdlib-spec-v0.11.md -Pattern '降级为官方包|小核心' -Context 0,2
cd ..\impl
Select-String -Path crates/wlwl-std/src/lib.rs -Pattern 'ai|agent|real-ai' -Context 0,1
Select-String -Path crates/wlwl-std/Cargo.toml -Pattern 'real-ai' -Context 0,3
```

| 项 | 现值 | 出处 |
|---|---|---|
| `std.ai` 成员 | **5**(`ASK` `EMBED` `COMPLETE` `ASK_STREAM` `ASK_ALL`) | 附录 A |
| `std.agent` 成员 | **5**(`TASK` `TOOL` `CALL_TOOL` `MODEL` `CONTEXT`) | 附录 A |
| feature 门控 | `std.ai` 的 5 个成员受 **`real-ai`** 门控,默认构建**不可见** | 附录 A / `Cargo.toml` |
| 门禁 | `cargo check --locked -p wlwl-std --features real-ai` 是 §3.3 的一条 | 计划 §3.3 |
| 规范已登记 | §14:「`std.ai` / `std.agent` 降级为官方包,移出 `wlwl:std.*`(**业界先例:Rust / Julia 的 std 小核心原则**)」 | 规范 §14 |
| 行业先例 | Rust(`rand` / `regex` / `serde` 都在 std 外)、Julia(标准库 + 独立包) | — |

## 2. 目标形态

**移出后**:

| 位置 | 内容 |
|---|---|
| `wlwl:std.ai` | **不存在**(不是空壳 —— 空壳会让 `resolve()` 仍返回它,四件套第 2 条会以为它还在) |
| `wlwl:std.agent` | **不存在** |
| 官方包 | `wlwl:ai` / `wlwl:agent`(命名待裁决 §3.3),带**自己的**规范文件 / 参考副本 / feature 门控 |
| `wlwl:std.*` | 回到 **11 个命名空间 / 94 个成员**(104 − 10) |

**成员面数字的后果**:标准库规范的附录 A 会从 13 行变 11 行;`stdlib_appendix_a_sync`
会红(它按实现生成,实现改了它就跟着变)⇒ 这是**预期内**的红,不是回归。
契约测试 `sanitize_contract` 之类解析 §13 成员表的**不受影响**(它们只看
`std.sanitize` 等还在的章节),但**任何断言「命名空间数 = 13」的守卫都要改**。

## 3. 语义裁决点(开写前必须逐条回答)

1. **「官方包」机制长什么样?**(§3.1 是**本份的头号阻塞**)
   现状:`wlwl` 有 `--std-src` 源码覆盖轨(规范 §0.5, debug 硬门控)与
   release 内嵌 R1 源码的机制,但**没有「包」的概念** —— 没有版本、没有
   依赖声明、没有独立的分发单元。
   → 三条路:
   - (a) **独立 crate**(`wlwl-ai` / `wlwl-agent`)+ 独立 feature + 独立
     规范文件。最接近 Rust,工作量最大。
   - (b) **仍在同一 crate,但用 feature 隔离**(`--features wlwl-std/ai`),
     `resolve()` 不返回它们。改动最小,但**形态上仍在 `wlwl-std` 里**,
     只是「看不见」—— 这算不算真搬?**要如实认定**。
   - (c) **R1 模块 + 独立加载路径**。复用 R1 的 `EXPORT` 机制,像
     `wlwl:std.*` 的 R1 成员一样,但用独立命名空间前缀。
   → **建议 (a)**,但它连带「包机制」这个更大的问题,可能超出本份。
   **若裁决停在 (b)/(c),本份应记为「部分完成」并写明差距**,不能宣称搬完了。
2. **这是 breaking 吗?** 成员的**名字 / 签名 / 语义**一字未改,但
   **可见面**变了(从 `wlwl:std.ai` 变成别的)。ADR-0023 §v0.x 1:「可见面变更
   (新增/改签名/改语义/移除/弃用)必须同批次完成四件套同步并留 CHANGELOG
   条目」。→ 结论:是**需要申报的可见面变更**,但**不是**「移除成员」,
   故**不需要走弃用两步**(警告码 + 保留一个次版本)。
   **这一点要在 CHANGELOG 明标**,并让调用方知道新名字。
3. **新命名空间叫什么?** `wlwl:ai` / `wlwl:agent`?还是 `wlwl:pkg.ai`?
   现有 `wlwl:std.*` 的前缀是 `std`,对应 Rust 的 `std`。移出后前缀必须
   变(否则还是 `std`),但叫什么要有依据 —— 见计划附录 A 的命名裁决体例
   (那套体例是「提名 + 理由 + 落选者」)。
4. **`real-ai` feature 怎么办?** 它现在挂在 `wlwl-std` 上。移出后应该挂在
   新包上 ⇒ **门禁命令要改**(`cargo check -p wlwl-std --features real-ai`
   会失效)。§3.3 的完成定义要同步改。
5. **`std.agent` 里的 `TASK` / `TOOL` / `CALL_TOOL` 与语言的 OOP / 工具
   调用有没有重叠?** 语言规范 §14–§16 有 OOP(CLASS / NEW / GET_PROP /
   SET_PROP / CALL_METHOD)与线性 `THIS`。`std.agent` 的 `TASK` / `TOOL` 是
   **AI 工具调用语义**,不是 OOP ⇒ 不重叠。但要在规范里写明,免得
   「为什么不用 CLASS」这个问题每次都要重答。
6. **要不要顺手把 `08-std-rand` 也一并搬出去?**(§14.1 否决 `std.rand` 时
   引的正是 Rust 先例)—— **本份不做**,那是 08 自己的 ADR A6。**两份分开,
   不要捆绑。**

## 4. 验收门禁

```powershell
cd impl
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --all-targets
cargo deny --locked --all-features check
$env:RUSTDOCFLAGS='-D warnings'; cargo doc --locked --no-deps
cargo check --locked -p <新包> --features <新 feature>     # 门禁命令随 §3.1 改
```

契约表:

| 组 | 用例 | 备注 |
|---|---|---|
| **不可见** | `resolve("wlwl:std.ai")` ⇒ **`None`**(不是空 `SPEC`) | **这条是「真搬」与「只 feature 掉」的分界** |
| 可见 | `resolve("wlwl:ai")` ⇒ 有 5 个成员 | |
| 行为不变 | 10 个成员的**契约用例逐条照抄**,只改 `IMPORT` 的命名空间前缀 | 防「搬顺手改了语义」 |
| 门控 | 默认构建下 `wlwl:ai` 的成员**不可见**;带 feature 可见 | 沿 `real-ai` 现状 |
| 附录 A | 生成器重跑后**不再有** `std.ai` / `std.agent` 两行;`stdlib_appendix_a_sync` 绿 | |
| 命名空间数 | 11 个 / 94 成员(104 − 10) | 硬编码守卫要改 |

## 5. 九处表落点

九处见 `docs/history/20261002.md` §5.0。**九处全中,且第 1 / 2 / 3 / 4 处是
「删除 + 新增」双向**:

| 落点 | 本份动作 |
|---|---|
| 1 规范成员表 | **删** §9 `std.ai` / §10 `std.agent` 两章的成员表(章号顺延);**新** 官方包规范文件 |
| 2 附录 A | 跑 `gen-appendix-a`(13 行 → 11 行) |
| 3 `EXPORT` / `SPEC.functions` | 从 `ALL_SPECS` 移除;新包另登记 |
| 4 契约测试硬编码 | 10 个成员的导入组改名;**任何断言命名空间数的守卫改 13 → 11** |
| 5 模块登记 | `src/ai.rs` / `src/agent.rs` 移出;`src/` 文件数守卫同步 |
| 6 probe | 相关 probe 改导入;**若无相关 probe 则不动** |
| 7 附录 G | **不动**(它们是 std 成员,不是全局内建) |
| 8 skill 指针 | `SKILL.md` 指针表 + `reference.md` 的 §9.x 编号**顺延**(§9.9 之后全部前移一位)—— **这是最易漏的一处** |
| 9 CHANGELOG | **必须明标「可见面变更但非移除」**(§3.2),并给新命名空间 |

## 6. 风险与已知代价

- ⚠️ **本份的头号阻塞是「官方包机制不存在」**(§3.1)。若裁决停在 (b)
  「同 crate 内 feature 隔离」,那**形态上它还在 `wlwl-std` 里**,只是默认
  构建看不见 —— 那**不算真搬**。本份必须**如实记为部分完成**并写明差距,
  不能宣称搬完。
- ⚠️ **章节编号顺延会波及 skill 的所有交叉引用**。`reference.md` 的
  §9.5 / §9.6 / … 全部要改,漏一处就是「按图索骥扑空」—— 这类失效交叉引用
  在 v0.11.3 已经犯过一次(§9 章首指向 §25),**要在收口时全文 grep 一遍**。
- ⚠️ **门禁命令要改**(§3.4)。`cargo check -p wlwl-std --features real-ai`
  会失效 ⇒ 计划 §3.3 的完成定义、README 门禁行、skill 的门禁说明三处
  都要同步。**三处漏一处 = 下一个 agent 照着跑会报「no such feature」。**
- ⚠️ **这是一个 breaking 面的可见面变更**。虽然不是「移除成员」,但调用方
  的 `IMPORT` 要改 ⇒ CHANGELOG 必须给出**迁移写法**。
- **明确不做**:引入完整包管理(版本 / 依赖声明 / 语义化版本) / 改任何成员的
  语义 / 顺手把 `std.rand` 也搬 / 砍功能。

## 7. 检查点

| 子项 | 内容 | 状态 | commit |
|---|---|---|---|
| W-01 | **「官方包」机制裁决**(§3.1)—— 决定本份是「真搬」还是「部分完成」 | 未动工 | — |
| W-02 | 命名空间命名裁决(沿计划附录 A 的体例:提名 / 理由 / 落选者) | 未动工 | — |
| W-03 | 迁移实现 + `resolve()` 不可见断言 | 未动工 | — |
| W-04 | 契约用例改导入(逐条照抄,不改期望值) | 未动工 | — |
| W-05 | 附录 A 重生成 + 命名空间数守卫 13 → 11 | 未动工 | — |
| W-06 | **skill 全文交叉引用顺延 + grep 复核** | 未动工 | — |
| W-07 | 门禁命令三处同步 + CHANGELOG 迁移写法 | 未动工 | — |
