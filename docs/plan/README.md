# docs/plan

本目录留给**当前迭代**的构建计划与偏差登记。

## 进行中

| 计划 | 覆盖 | 状态 |
|---|---|---|
| [`wlwl-v0.11.3-build-plan.md`](wlwl-v0.11.3-build-plan.md) | v0.11.3:输出安全 `std.sanitize`(**全员 R2**,性能为契约 —— 树构建净化器 + 全表实体 + 基准存档)+ `std.encode` 哈希 + SQL / 加解密否决成文 + `std.time` ADR-0024 与 regex 调研(只文档) | **M0–M4 全部完成**(M0 `601e76b`、M1 `613f13e`、M2 `875b699`、M3 `d5b47ad` + `2572fba` 收掉 D13-003~010、M4 `200ff9c`);**追加 M5–M8 未动工**(甲路径,见总纲 §4);分支 `wip0.11.3`;**tag 继续按住不发**,收口后由维护者发 |
| [`wlwl-v0.11.3-addendum.md`](wlwl-v0.11.3-addendum.md) | **v0.11.3 追加批次总纲**:15 份独立构建方案的分层依赖图、六个全局裁决点、版本口径后果、格式契约 | **版本路径已裁决:甲(业主 2026-10-05)** —— 并入 v0.11.3,分 **M5(L0)/ M6(L1 八份)/ M7(L2 五份)/ M8(L3)**;15 份方案 **82 个工作项全部未动工(0/82)**;G1–G6 与五份立项确认仍未闭合 |

### 追加批次 · 15 份独立方案(业主 2026-10-05 决定「一次性完善标准库」)

总纲:[`wlwl-v0.11.3-addendum.md`](wlwl-v0.11.3-addendum.md) §2 有完整表、
依赖图与裁决点。**`00` 必须最先** —— 它是 L2 三份的硬前置。

| # | 方案 | # | 方案 |
|---|---|---|---|
| 00 | [R0 解释器治本(D11-012)](addendum-00-r0-foundations.md) | 08 | [`std.rand`(先出 ADR)](addendum-08-std-rand.md) |
| 01 | [`std.time` + 假时钟气泡](addendum-01-std-time.md) | 09 | [`std.json` 深度](addendum-09-json-deep.md) |
| 02 | [RE2 式线性正则](addendum-02-regex.md) | 10 | [`std.math` 扩展](addendum-10-numeric-extended.md) |
| 03 | [Unicode 规范化(UCD 一整块)](addendum-03-unicode-normalization.md) | 11 | [`RESULT` 组合子](addendum-11-result-optional-combinators.md) |
| 04 | [KDF / CSPRNG / 常数时间比较](addendum-04-crypto-kdf-secrets.md) | 12 | [同步原语 `std.sync`](addendum-12-sync-primitives.md) |
| 05 | [`std.net`](addendum-05-std-net.md) | 13 | [`std.sanitize` 扩展](addendum-13-sanitize-extend.md) |
| 06 | [`std.process` / `std.env` / 路径](addendum-06-std-process-env.md) | 14 | [`std.ai` / `agent` 移出(减法)](addendum-14-ai-agent-demotion.md) |
| 07 | [压缩](addendum-07-compression.md) | | |


> v0.11.2 已于 2026-10-02 收口(M0–M6 全部完成,复核 N-6 的修复 D12-013 亦已落地),
> 计划与三份审查/复核报告已按归档规则精简入 [`../history/20261002.md`](../history/20261002.md)。
> v0.11.3 立项依据 = 业主 2026-10-02/03 裁决(标准库拓展研究)+ **2026-10-03 追加:
> `std.sanitize` 为旗舰特色功能,性能是硬指标** —— 据此计划重写:`HTML_*` 全族
> 下沉 R2(归层证据沿 ADR-0021 由 W-08 的 R1 原型对照基准留档)、净化器采
> **树构建形态**(tokenizer 方案被 IEEE S&P 2024 mXSS 研究否决)、实体**全表**
> 经生成器落地、幂等性为安全不变量、线性承诺与吞吐目标进 §7.4。
> 命名空间名 `std.sanitize`(`std.html` 被否决,裁决记录见计划附录 A);
> **成员面 99 → 104,命名空间 12 → 13 —— 已达成**(对着生成器锁定的附录 A
> 逐行点过:`std.io` 3 / `std.fs` 3 / `std.json` 2 / `std.format` 1 /
> `std.encode` 8 / `std.text` 2 / `std.sanitize` 3 / `std.collection` 27 /
> `std.str` 7 / `std.math` 32 / `std.test` 6 / `std.ai` 5 / `std.agent` 5)。
> §10 三个开工前裁决点均已闭合:`HMAC_SHA256` 同批落地、W-08 归层对照真做
> (R1 原型在 1 MB 输入上「跑不完」,见 baseline.txt M1 段)、**W-10 缺省净化
> 策略内容**由业主 2026-10-05 指示继续 M4 收口时确认按现状冻结(计划 §10 有记录)。

**历史**:v0.11(已发布)、v0.11.1(已发布)、v0.11.2(已发布,tag `v0.11.2`)。v0.10.x 批次已全部收口;`std.web` 于 2026-10-01 宣告失败并回滚。全部历史归档(语言规范 v0.6–v0.10、
构建计划、偏差登记、审查报告、日次日志)已按归档规则**合并精简**为
`../history/` 下四份汇总:

| 汇总件 | 覆盖范围 |
|---|---|
| [`../history/20260902-09.md`](../history/20260902-09.md) | 2026-09-02 ~ 09-09 日志 + v0.1 / v0.2 / Phase B 构建计划 |
| [`../history/20260915-22.md`](../history/20260915-22.md) | 2026-09-15 ~ 09-22 日志 + 规范 v0.6–v0.10 / 构建计划 v0.7–v0.10 / 偏差登记 v0.7–v0.10 / 全部版本时代归档件 |
| [`../history/20261001.md`](../history/20261001.md) | **v0.11 批次 A 标准库底座收口** + **v0.11.1 全部产出**(`std.web` 立项、推进与终止) + **v0.11 偏差台账 D11-001 – D11-026 + 终止条 D11-037** |
| [`../history/20261002.md`](../history/20261002.md) | **v0.11.2 构建计划(含偏差台账 D12-001 – D12-013)** + **三份审查/复核报告**(v0.11.0 审查、v0.11.1 复核、v0.11.2 复核) |

各份的**完整原文在 git 历史**,按 `git log --follow docs/plan/<原文件名>` 追溯。

## 当前状态(2026-10-05)

| 项 | 状态 |
|---|---|
| 现行规范 | [`../spec/wlwl-spec-v0.11.md`](../spec/wlwl-spec-v0.11.md)(v0.11;§10 已机制化,命名空间成员契约外迁) |
| 现行标准库规范 | [`../stdlib/wlwl-stdlib-spec-v0.11.md`](../stdlib/wlwl-stdlib-spec-v0.11.md)(v0.11;**v0.11.2 起与语言规范同号** —— 本批成员面 58→99 但无 breaking,故不升版,变更登记在册首「本版变更」块) |
| agent 专用规范 | [`../spec/wlwl-agent-spec-v0.11.md`](../spec/wlwl-agent-spec-v0.11.md)(与上面两册**冲突时以人读规范为准**;专供 AI agent,可随时改) |
| 最新实现 | v0.11.3(M0–M4 全部完成,代码收口;**tag 继续按住不发** —— 追加批次走甲路径并入本版) |
| 进行中的计划 | **追加批次 M5–M8**(总纲 §4 已裁决甲)。M5 的 **L0-A-1 ~ L0-A-4 全部结项**,累计 **13 个成员**归 R2(余 14 成员仍 R1;`EXPORT` 全程未动 ⇒ 成员面 0 变化)。8 k 档实测:`WINDOW` 24 472→5.5 ms(4 479×)、`ENUMERATE` 7 437→3.2 ms、`FLAT` 9 814→3.7 ms;`GROUP_BY` 4 k 档 4 512→**14.4 ms(314×)**;**`UNIQ` 仍平方级**(残余是成员查找那笔)。**`SORT`/`SORT_BY` 按裁决甲不归层**(n=2 000 需 9.4 分钟;`sort_by` 方案经实测推翻,推导见 `addendum-00` §5.3),`乙案`(改契约冻结值)保留为独立语义裁决。**M5 收口只差 L0-A-4 的差分回归测试与契约零改动核验** |
| 下一迭代 | 未立项。候选:批次 B `YIELD` 续体保存(**0 细化**,开细化前须先定三件事,见 `../history/20261001.md` §1 —— 其第 2 件与 M5 的 L0 同根「R0 原语补齐」,两份范围尚未对齐)、`std.rand` 的全局状态裁决(需先出 ADR,总纲 G3)、UCD 数据依赖成员合成一块做(需先定 Unicode 版本 G1 与「半张表怎么办」G2)、regex(RE2 式线性模拟,调研见 v0.11.3 计划附录 B)、R1 预编译快照(其价值取决于 M5 完成后 R1 成员还剩多少) |
| 门禁 | `cargo fmt --check` 0 diff;`clippy -D warnings` 0;`cargo test --locked --all-targets` **0 failed**(42 套件 1955 passed);`cargo deny check` exit 0;`cargo doc --no-deps -D warnings` 0 error(**D11-015 之后已并入本地门禁**;注意 `cargo doc` 不接受 `-D`,须走 `$env:RUSTDOCFLAGS='-D warnings'`);`cargo check --locked -p wlwl-std --features real-ai` exit 0(S-P1-1 之后新增);probe **158** 用例 —— 158 是 `impl/tests/probe/cases/` 的**用例目录数**(v0.11.3 M5 L0-A-2 新增 `M5_std_collection_l0a2_r2_scale`,5 000 元素大数组规模用例),由单个 `probe_suite` 函数驱动并有 `probe_case_count_matches_inventory` 守卫,**不是** `#[test]` 函数数(`--test probe` 只跑 3 个函数);契约表 `collection` 127 / `str_math` 156 / `format` 39 / `encode` 47 / `text` 20 / `sanitize` 103 条;附录 A 由 `gen-appendix-a` 双向锁绿;附录 G 逐字节不变;基准存档 `benches/baseline.txt` M1 / M2 / M3 / **L0** / **L0-A-2** / **L0-A-3a** / **L0-A-3b** 七段(路径 `impl/crates/wlwl-eval/benches/`) |
| 已立项范围 | v0.11:**批次 A 标准库底座**(ADR-0021/0022/0023)**已收口**;批次 B `YIELD` 续体保存**已立项但未细化**;v0.11.1:`std.web` **已终止并回滚**;v0.11.2:**已收口**(M0–M6 + 复核 N-6 修复 D12-013),待发 tag;v0.11.3:**已立项**(输出安全 + 哈希,SQL / 加解密否决成文,time / regex 只出文档) |

## 挂账项

| 编号 | 内容 |
|---|---|
| D11-004 | ~~`SORT` 的自定义比较器返回 `ERR` 时返回值被拆成载荷~~ **已了结(v0.11.1,实测证实)** —— 复核报告 §1.3 判为已修,本轮实测 `SORT([2,1],cmp)` → `IS_ERR=TRUE` / `TYPE=RESULT` / `ERR_PAYLOAD="cmp-boom"`。此前此处长期挂着,根因与三条修法见 `../history/20261001.md` |
| D11-022 | `StdCtx::warnings` 是无人排空的死汇,`W0052` 永不浮现。**凡依赖「推一条警告就能被看到」的设计都不成立** —— 走警告通道等于静默丢弃 |
| D12-001 – D12-013 | v0.11.2 偏差登记(含 2 条**已知限制**:`NFC`/`NFD` 判不等(D12-004)、`CHUNK`/`WINDOW` 的平方级上限(D12-009);D12-013 为复核 N-6 的 `MIN_BY`/`MAX_BY` 空数组缺陷,**已修复**)。逐条状态与证据见 [`../history/20261002.md`](../history/20261002.md) §1 |

## 历史归档去哪找

| 要找什么 | 去哪 |
|---|---|
| 2026-09-02 ~ 09-09 日志 + v0.1 / v0.2 / Phase B 构建计划精简 | [`../history/20260902-09.md`](../history/20260902-09.md) |
| 2026-09-15 ~ 09-22 日志 + 规范 v0.6–v0.10 / 构建计划 v0.7–v0.10 / 偏差登记 v0.7–v0.10 / 全部版本时代归档件精简 | [`../history/20260915-22.md`](../history/20260915-22.md) |
| v0.11 / v0.11.1 全部产出(含 `std.web` 的失败记录与复活前置条件)+ 偏差台账 | [`../history/20261001.md`](../history/20261001.md) |
| v0.11.2 构建计划(含 D12 台账)+ 三份审查/复核报告 | [`../history/20261002.md`](../history/20261002.md) |
| 任一归档件的**完整原文** | git 历史(`git log --follow docs/plan/<原文件名>` 或 `git log --follow docs/history/<原文件名>`) |
