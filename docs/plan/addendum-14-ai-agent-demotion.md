# 14 · `std.ai` / `std.agent` 移出 `wlwl:std.*`(**减法**)

> **层级** L3 · **前置** L1 全部落地 · **状态** **W-01 ~ W-07 全部落地(2026-10-15)**,按 §6 记为**部分完成**(形态差距已如实登记)
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
| feature 门控 | ~~`std.ai` 的 5 个成员受 **`real-ai`** 门控,默认构建**不可见**~~ ⛔ **该行经 2026-10-15 实测证伪,见下方更正** | ~~附录 A / `Cargo.toml`~~ |
| 门禁 | `cargo check --locked -p wlwl-std --features real-ai` 是 §3.3 的一条 | 计划 §3.3 |
| 规范已登记 | §14:「`std.ai` / `std.agent` 降级为官方包,移出 `wlwl:std.*`(**业界先例:Rust / Julia 的 std 小核心原则**)」 | 规范 §14 |
| 行业先例 | Rust(`rand` / `regex` / `serde` 都在 std 外)、Julia(标准库 + 独立包) | — |

> ⛔ **[2026-10-15 实测更正:上面「feature 门控」那一行是错的]**
>
> 原表写「`std.ai` 的 5 个成员受 `real-ai` 门控,默认构建**不可见**」。**实测相反**:
> **默认构建**(不带 `real-ai`)下 `IMPORT("wlwl:std.ai", [ASK, EMBED, COMPLETE,
> ASK_STREAM, ASK_ALL])` **成功绑定全部 5 个成员**(唯一报错是 `ASK` 元数不对,
> 说明名字已解析、函数确实存在)。
>
> 真实形状:`wlwl-std/Cargo.toml` 的 `real-ai = ["dep:reqwest"]` —— 它门控的是
> **HTTP 传输层**(mock ↔ real),**不是成员**;`src/ai.rs` 的
> `#[cfg(feature = "real-ai")] mod real` 只包住真实后端,成员函数本身一直编译。
>
> ⇒ **两条下游结论随之作废**:
> ① §3.4「`real-ai` 移出后门禁命令会失效」—— 在 **ADR-0034 D1 选的 (c)** 下**不成立**:
>    crate 仍是 `wlwl-std`,feature 仍挂在它上面,`cargo check -p wlwl-std --features
>    real-ai` **继续有效** ⇒ W-07 的「门禁命令三处同步」在 (c) 下**不需要做**。
> ② §4 契约表的「默认构建下成员**不可见**;带 feature 可见」—— 当前恰恰相反,
>    且 `addendum-14` §0 的非目标写明**不改任何成员的语义** ⇒ 搬完之后成员
>    **应当仍然在默认构建下可见**。这条要照现状写进契约,**不是**照初稿。

## 2. 目标形态

**移出后**:

| 位置 | 内容 |
|---|---|
| `wlwl:std.ai` | **不存在**(不是空壳 —— 空壳会让 `resolve()` 仍返回它,四件套第 2 条会以为它还在) |
| `wlwl:std.agent` | **不存在** |
| 官方包 | `wlwl:ai` / `wlwl:agent`(命名待裁决 §3.3),带**自己的**规范文件 / 参考副本 / feature 门控 |
| `wlwl:std.*` | 回到「语言通用设施」定位(**本行的 11 个 / 94 是 M6 扩容前的快照,已过期 —— 见下方更正**) |

> ⚠ **[2026-10-15 更正:本表的成员面数字已过期,别照抄进规范]**
> 「11 个命名空间 / 94 个成员(104 − 10)」写于 M6 扩容**之前** —— 当时
> `std.time` / `regex` / `rand` / `env` / `process` / `compress` / `net` /
> `result` 都还没开,`std.math` 也还没被 M7 扩充。按现行附录 A 表体实测:
> **`wlwl:std.*` 19 个命名空间 / 204 成员**,官方包 **2 个 / 10 成员**,
> 登记表 **21 行 / 214 成员**。见标准库规范 §0 的同款更正框。

**成员面数字的后果**:标准库规范的附录 A 会从 13 行变 11 行;`stdlib_appendix_a_sync`
会红(它按实现生成,实现改了它就跟着变)⇒ 这是**预期内**的红,不是回归。
契约测试 `sanitize_contract` 之类解析 §13 成员表的**不受影响**(它们只看
`std.sanitize` 等还在的章节),但**任何断言「命名空间数 = 13」的守卫都要改**。

## 3. 语义裁决点(开写前必须逐条回答)

1. **「官方包」机制长什么样?**(§3.1 是**本份的头号阻塞**)
   > ✅ **[2026-10-15 核实:本条的基线断言已过期,头号阻塞其实不存在]**
   >
   > 初稿写:「现状:`wlwl` 有 `--std-src` 源码覆盖轨与 release 内嵌 R1 源码的
   > 机制,但**没有「包」的概念** —— 没有版本、没有依赖声明、没有独立的分发单元。」
   > **三样东西现在都在,而且 `[namespaces]` 已经接进 `wlwl-eval` 的模块解析并可用:**
   >
   > | 初稿说「没有」 | 实测 | 出处 |
   > |---|---|---|
   > | 版本 | `[package] name / version / entry / description / license` | `wlwl-toml/src/manifest.rs:50` `struct Package` |
   > | 依赖声明 | `[dependencies]` + **MVS semver 求解**(精确 / `^` caret / `>=`/`<` 比較子 / 逗号 AND) | `manifest.rs:106`、`mvs.rs` |
   > | 独立的分发单元 | `[namespaces] <ns> = <dir>` + 该目录下的 `<name>.wll` | `manifest.rs:108` `namespaces` 字段 |
   > | 锁文件 | `wlwl_toml::lock::hash_dependency_dir` | `wlwl-toml/src/lock.rs` |
   >
   > **功能验证(不是读代码推断)**:一个只有 `wlwl.toml` + `vendor/greet/utils.wll`
   > 的目录,`[namespaces] greet = "vendor/greet"`,`IMPORT("greet:utils", ["greet"])`
   > ⇒ 打印 `hi from a wlwl.toml namespace` rc=0;**把 `wlwl.toml` 挪走即 `E0043` rc=1**。
   >
   > ⇒ **路线 (c)「R1 模块 + 独立命名空间前缀」不需要新机制 —— 它已经在了。**
   > 初稿建议 (a) 独立 crate 并警告它「连带『包机制』这个更大的问题,**可能超出本份**」,
   > 那个警告建立在过期基线上。⇒ **§3.1 的三选一需要按新事实重答**:
   > (a) 与 (c) 的工作量差距已不是「造机制 vs 不造」,而是「crate 边界 vs 清单边界」。
   > ⚠ **但「官方包要不要独立 crate」仍是契约面决定**,不在本轮自行选定。

   三条路(⚠ 代价评估已按上面的事实修正):
   - (a) **独立 crate**(`wlwl-ai` / `wlwl-agent`)+ 独立 feature + 独立
     规范文件。最接近 Rust。
   - (b) **仍在同一 crate,但用 feature 隔离**(`--features wlwl-std/ai`),
     `resolve()` 不返回它们。改动最小,但**形态上仍在 `wlwl-std` 里**,
     只是「看不见」—— 这算不算真搬?**要如实认定**。
   - (c) **R1 模块 + `wlwl.toml` `[namespaces]` 声明的独立命名空间**。
     **⚠ 这一条的机制已存在且已验证可用**(见上框),它是三者里最省的。
   → 初稿建议 (a)。**该建议的**前提**(「要连带造包机制」)已不成立,建议本身待重评。
   **若最终裁决停在 (b)/(c),本份应记为「部分完成」并写明差距**,不能宣称搬完了。
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
| 可见 | `resolve("ai:ask")` ⇒ 有 5 个成员 | ⚠ **2026-10-20 订正**:原写 `resolve("wlwl:ai")` —— 该形态**结构上不可行**(`wlwl:` 是 std 保留前缀,解析器对它开头直接返 `None`,落进「裸模块名」分支被当文件名,实测 `E0040`);可用前缀只有 `ai:` / `agent:`(见 §7 W-02) |
| 行为不变 | 10 个成员的**契约用例逐条照抄**,只改 `IMPORT` 的命名空间前缀 | 防「搬顺手改了语义」 |
| 门控 | 默认构建下 `ai:ask` 的成员**可见**;`real-ai` 只门 HTTP 传输、**不门成员** | ⚠ **2026-10-20 订正**:原写「默认构建下 `wlwl:ai` 的成员**不可见**;带 feature 可见」—— **已被 2026-10-15 实测证伪**(默认构建下 `ai:` 的 5 个成员可见;且 addendum §0 的非目标写明「不改任何成员的语义」⇒ 搬完仍应可见)。**这条要照现状写进契约,不是照初稿** |
| 附录 A | 生成器重跑后**不再有** `std.ai` / `std.agent` 两行;`stdlib_appendix_a_sync` 绿 | |
| 命名空间数 | ⚠ **目标值已过期**(这是 M6 扩容**之前**的快照):按附录 A 表体实测为 `wlwl:std.*` **19** 个 / 204 成员 + 官方包 **2** 个 / 10 成员 = **21 行 / 214 成员** | 代码里**没有写死的 13/11** —— `stdlib_appendix_a_sync` 与 `layer_meta_covers_exactly_the_resolvable_namespaces` 都是拿 `namespace_paths()` 夹逼登记表的双向**动态**对账 ⇒ 无计数守卫可改 |

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

- ⚠⚠ **[2026-10-20 订正]本条原写「本份的头号阻塞是『官方包机制不存在』(§3.1)」——
  与同一文件 §3.1 的更正框自相矛盾**:§3.1 已于 2026-10-15 用**实测**证伪该
  基线(版本 / 依赖声明 / 独立分发单元 / 锁文件**四样都已存在**,且
  `[namespaces]` 已接进 `wlwl-eval` 的模块解析并可用)⇒ **头号阻塞并不存在**。
  ⚠ 真正的差距是**形态**层面 —— 无独立 crate 边界 / 无独立分发产物 /
  无包级版本·依赖·锁·签名 —— **不是「没有包机制」**。
  ⚠ 裁决已冻结在 (b′)(`ADR-0034` D1:就地冻结、不再重裁)⇒ 形态差距**按 §7 W-01
  的差距清单登记**;⚠ 差距**必须写明,不能宣称搬完**;⚠⚠ **M8 的状态标签是否由
  「部分完成」改为「已结项 + 形态差距清单 + 复活条件 + 零可观察行为差」,
  待业主裁决**(2026-10-20 登记,见 `build-plan` §9 台账 D13-085)。
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
| W-01 | **「官方包」机制裁决**(§3.1)—— 决定本份是「真搬」还是「部分完成」 | ✅ **已裁决并就地冻结于 (b′) 同 crate + 换前缀**(2026-10-15,`ADR-0034` D1)。⚠ **本条的选型反复过三轮**(倾向 (a) → 改选 (c) → 撤回 → 定 (b′)),根源始终是同一个:**把「机制能不能用」当成了「机制适不适合这两个成员」**。(b′) 满足 §4 自己给的判据(`resolve("wlwl:std.ai") == None`,非空 `SPEC`),形态差距(无独立 crate / 无独立分发产物)按 §6 **如实记为部分完成** —— 而那是**纯形态**差距,零可观察行为差。(a) 要的「原生包注册通道」是**语言运行时的机制变更**,推迟到独立批次,D1 已写明复活条件 | `a46d0f3` / `14fcd0c` / 本笔 |
| W-02 | 命名空间命名裁决(沿计划附录 A 的体例:提名 / 理由 / 落选者) | ✅ **已裁决(2026-10-15)** —— 取 **`ai:` / `agent:`**(`ADR-0034` D2)。⚠ **实测击死了两个最直觉的候选**:`wlwl:ai` 结构上**不可行** —— `wlwl:` 是 std 的保留前缀,`parse_ns_path` 对 `wlwl:` 开头直接返回 `None`,它会落到「裸模块名」分支被当成文件名(实测 `E0040`,错误里展示它在找 `…\wlwl:ai.wll`);对照 `wlwl:std.rand` 则正常可达。落选者:`wlwl:pkg.ai`(同死于前缀)/ `pkg:ai`(引入了本仓不存在的概念名)/ `std.ai2`(仍在 std 里)| `W01_W02_SHA` |
| W-03 | 迁移实现 + `resolve()` 不可见断言 | ✅ 已落地 `f9da8ba` —— `resolve()` 匹配表新增 `ai:ask`…`agent:context` 十个键,`wlwl:std.ai` / `wlwl:std.agent` **返回 `None`**(分界断言 `std_ai_and_agent_are_demoted_to_official_packages`)。⚠ 走的是 **(b′)** 同 crate 换前缀 ⇒ 按 §6 字面**记为部分完成** | `f9da8ba` |
| W-04 | 契约用例改导入(逐条照抄,不改期望值) | ✅ 已落地 `f9da8ba` —— 四条测试抓到写死的旧路径,全部改前缀;`ai` / `agent` 可解析、旧 `wlwl:std.*` 为 `None` 的分界断言在 `stdlib_appendix_a_sync` | `f9da8ba` |
| W-05 | 附录 A 重生成 + 命名空间数守卫 13 → 11 | ✅ **部分完成,已如实记下差距**。**附录 A 已随 `f9da8ba` 重生成**(两行改成 `ai` / `agent`)。⚠ **「守卫 13 → 11」无对象可改**:代码里**没有任何写死的 13/11** —— `stdlib_appendix_a_sync` 与 `layer_meta_covers_exactly_the_resolvable_namespaces` 都是拿 `namespace_paths()` 夹逼登记表的双向**动态**对账。真要改的写死计数在**规范散文**里(`1cae28c`)。⚠ **本子项的目标值本身已过期**:§2 的「13 → 11 命名空间 / 104 − 10 = 94 成员」是 **M6 扩容之前**的快照(`std.time`/`regex`/`rand`/`env`/`process`/`compress`/`net`/`result` 都还没开,`std.math` 也还没被 M7 扩),照抄即等于把假状态写进规范 ⇒ 按附录 A 表体实测重算为 **`wlwl:std.*` 19 个 / 204 成员 + 官方包 2 个 / 10 成员 = 21 行 / 214 成员**。规范 §9 / §10 同时改写为官方包正文 | `f9da8ba` + `1cae28c` |
| W-06 | **skill 全文交叉引用顺延 + grep 复核** | ✅ 已完成,但**「顺延」无对象可顺** —— `reference.md` 的 §9.x 序列里**从来没有 ai/agent 章节**,现存的是 §9.1–§9.7 / §9.9–§9.15,**§9.8 空号**。⚠ §6 预言的「§9.9 之后全部前移一位」是**基于「§9.8/§9.9 是 ai/agent」的旧前提**,不成立。真正该做的是 §6 点名的失效交叉引用 —— 写了个复核脚本机械查,查出 **`reference.md:877` 的 `§9.8` 是条悬空引用**(指向不存在的章节;`RANDOM_HEX` 实际在 §9.1),已改正。`SKILL.md` 的 ai/agent 指针行、§9 章首与 `std.rand` 过渡形态三处同步更新。复核脚本现报 **0 条悬空** | `1cae28c` 后一笔 |
| W-07 | 门禁命令三处同步 + CHANGELOG 迁移写法 | ✅ CHANGELOG 迁移写法已写(明标「可见面变更但**非移除**」+ 前后 `IMPORT` 对照 + `wlwl:ai` 不可行 + 无全限定语法)。⚠ **「门禁命令三处同步」已证伪不做** —— `real-ai` 仍挂在 `wlwl-std` 上,`cargo check -p wlwl-std --features real-ai` **继续有效** | 见 CHANGELOG |
