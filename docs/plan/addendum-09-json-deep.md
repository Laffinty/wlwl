# 09 · `std.json` 深度(低层词法层 / 增量 / schema 校验)

> **层级** L2 · **硬前置** 00(R0 治本)—— 增量解析要反复构建容器,不治本就是平方级
> **状态** 未动工 · **规范登记** 「不做流式 / 增量 API」写在**本主计划 v0.11.3
> 构建计划 §3.2 非目标**(本份是**重开**该非目标)。
> ⚠️ 不是 stdlib 规范的 §3.2 —— 该册**没有 §3.2**(`std.json` 是 `## 3` 章,无 .2
> 子节;全册 .1 子节只有 4.1 / 5.1 / 7.1 / 11.1–11.5 / 12.1 / 13.1–13.3 / 14.1)。
> **[2026-10-05 更正]** 原稿引「规范 §3.2」,按该编号去 stdlib 规范里查不到任何
> 对应文字。

## 0. 契约摘要

- **目标**:给 `std.json` 补三样东西 —— **低层词法层**、**增量解析**、
  **schema 校验**;让 `std.json` 从「两个函数」变成能承载真实协议解析的面。
- **非目标**(违反即范围事故):
  - **不扩语法**。JSON 语法由 RFC 8259 定死,`NaN` / `Infinity` / 注释 /
    尾逗号**一律不接受**(现有实现的口径保持)。
  - 不做 YAML / TOML / CBOR / MessagePack(每种格式一份独立方案;仓里的
    `wlwl-toml` crate 是 manifest 解析,**不是** `wlwl:std` 成员面)。
  - **不做 JSON Schema 全量**(draft 2020-12 的全部关键字)。本批只做
    **闭合子集**(见 §2)。
  - 不做「JSON → wlwl 类型」的自动双向映射生成。
- **为什么值得做**:`PARSE` / `STRINGIFY` 两个成员无法写「逐字段读取一个
  10 MB 的响应」,也无法在解析失败时**指出错在哪**。低层词法层是所有
  精确错误定位与增量处理的前提。

## 1. 基线(可核验)

```powershell
cd docs
Select-String -Path stdlib/wlwl-stdlib-spec-v0.11.md -Pattern '## 3 `std.json`' -Context 0,10
Select-String -Path stdlib/wlwl-stdlib-spec-v0.11.md -Pattern '流式' -Context 0,2
cd ..\impl
Select-String -Path crates/wlwl-std/src/json.rs -Pattern '^pub fn |^fn ' | Measure-Object
```

| 项 | 现值 | 出处 |
|---|---|---|
| `std.json` 成员 | **2**(`PARSE` / `STRINGIFY`) | 附录 A |
| 流式的立场 | §3.2 非目标:「不做流式 / 增量 API(本批输入为整串;流式留演进方向)」 | 计划 §3.2 |
| 语法容错 | 规范 §3 的现有口径(不接受 NaN / 注释 / 尾逗号) | 规范 §3 |
| 主流对照 | Go 1.25 推出**实验性** `encoding/json/v2` + `encoding/json/jsontext`(低层词法层),见 [Go 1.25 Release Notes](https://golang.google.cn/doc/go1.25) | 当期证据 |

> **那条当期证据的用法**:Go 把 json/v2 放在 `GOEXPERIMENT=jsonv2` 后面,
> 且**保留了旧 `encoding/json` 的行为**(「marshaling and unmarshaling behavior
> is unaffected, but the text of errors may change」)。wlwl 应当照这个体例 ——
> **新增成员,不改 `PARSE` / `STRINGIFY` 的可观察行为**。

## 2. 成员面

**层:R2 全员。**

### 2.1 低层词法层(仿 `jsontext`)

| 成员 | 语义要点 |
|---|---|
| `JSON_TOKENIZE(s)` | 产出 `ARRAY[DICT]` 的 token 流,每个含 `{kind, start, end, text}`。**纯词法,无结构** |
| `JSON_LOCATE(s, offset)` | 给字节偏移 → `{line, column, token_start}`。**错误定位的底座** |
| `JSON_VALIDATE(s)` | 只验语法,返 `OK` / `ERR([kind: JsonError, path, line, column, reason])` |

### 2.2 增量 / 定位

| 成员 | 语义要点 |
|---|---|
| `JSON_GET(doc, path)` | 按路径取子值,缺失返 `NULL`。路径用 `ARRAY` 表达(`JSON_GET(d, ["a", 0, "b"])`) |
| `JSON_SET(doc, path, value)` | 返**新**文档(不可变语义) |
| `JSON_DELETE(doc, path)` | 同上 |
| `JSON_MERGE(a, b)` | 语义合并(对象深合并 / 数组替换)—— **必须写明「不是补丁格式」** |
| `JSON_CHUNK_ITEMS(doc)` | 把顶层数组**惰性**切块,供「逐条处理」用(不是流式解析,是流式**消费**) |

### 2.3 schema 校验(闭合子集)

| 成员 | 语义要点 |
|---|---|
| `JSON_CHECK(doc, schema)` | schema 是 `DICT`;支持 `type` / `required` / `properties` / `items` / `enum` / `minimum` / `maximum` / `minLength` / `maxLength`。**不做的关键字不报错,只当没约束** |

**失败的统一口径**:
- **数据不合 schema** → `ERR` 值,`kind = "JsonError"`,**带 `path` / `line` /
  `column`**(这是本份最有价值的部分:现在报错不带位置)。
- **元数 / 类型错** → `E0030` / `E0022`。
- **schema 自身写错**(未知 `type`)→ `E0030`(程序员的错,不是数据)。

## 3. 语义裁决点(开写前必须逐条回答)

1. **增量解析做不做?** §3.2 把它列为非目标。本份的**替代方案**是
   `JSON_TOKENIZE`(词法层)+ `JSON_CHUNK_ITEMS`(消费层)—— 全量 `PARSE` 仍
   是「输入为整串」。→ 裁决:这算不算重开「不做流式」?建议**明写「本批仍不
   做流式解析」**,把 token 流定位为**诊断与定位工具**,避免名实不符。
2. **`JSON_GET` 的路径类型**:用 `ARRAY` 表达(`["a", 0, "b"]`)还是新语法?
   新语法 = 语言变更。→ 建议 `ARRAY`。
3. **`JSON_MERGE` 的语义**必须逐字写死:对象是**递归深合并**还是**顶层替换**?
   数组是替换还是拼接?含糊的 merge 是配置事故的常见源。
4. **schema 子集的边界**:写了未知关键字**静默忽略**还是 `E0030`?
   静默忽略 ⇒ 调用方以为约束生效了(与 §12.1「半张表比没有更糟」同款风险);
   报错 ⇒ 不能向前兼容。→ **裁决**,倾向**报错**(与全表纪律一致)。
5. **错误消息形态**:`reason` 用英文还是本仓既有风格?与 `DecodeError` 的
   `op` / `reason` 体例对齐即可,但要带 `path`。
6. **`PARSE` / `STRINGIFY` 一条都不许动** —— Go 的 json/v2 保留了旧行为,
   本份照做。任何需要改旧成员行为的需求,都不属本批。

## 4. 验收门禁

```powershell
cd impl
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --all-targets
cargo deny --locked --all-features check
$env:RUSTDOCFLAGS='-D warnings'; cargo doc --locked --no-deps
cargo check --locked -p wlwl-std --features real-ai
```

契约表扩 `tests/json_contract.rs`(新):

| 组 | 用例 | 出处 |
|---|---|---|
| 语法 | **RFC 8259 §3 / §Appendix A 的示例全量跑** | RFC 8259 |
| 拒收 | `NaN` / `Infinity` / 注释 / 尾逗号 / 单引号 / 前导零 → 全部 `ERR` | RFC 8259 的「不应接受」清单 |
| **错误定位** | 每个用例断言 `line` / `column` **逐字节冻结** | 本批的核心产出 |
| `JSON_GET` / `SET` / `DELETE` | 路径语法 + 缺失返 `NULL` + 不可变性(原文档不被改) | |
| `JSON_MERGE` | 深合并 / 数组替换的**逐条**语义用例 | 裁决点 §3.3 落定后冻结 |
| schema | 子集内每个关键字**正反各一条** | |
| **不静默** | 未知 schema 关键字 → `E0030`(若裁决 §3.4 选报错) | 防「以为约束生效了」 |
| **回归** | 既有 `PARSE` / `STRINGIFY` 行为**逐条不动** | 防重开旧行为 |
| 性能 | 10 KB ≥ 50 MB/s;`JSON_GET` ≥ 100 MB/s | 新基准入 `baseline.txt` L2 段 |

**规模契约**(依赖 00):`JSON_PARSE` 在 1 MB 语料上**线性**,每字节成本与
1 KB 档持平。

## 5. 九处表落点

九处见 `docs/history/20261002.md` §5.0。**扩已有命名空间**:

| 落点 | 本份动作 |
|---|---|
| 1 规范成员表 | §3 补 11 行 + 错误定位条款 + schema 子集条款 + 「未知关键字不静默」条款 |
| 2 附录 A | 跑 `gen-appendix-a`(`std.json` 2 → 13 成员) |
| 3 `SPEC.functions` | `json.rs` 追加 11 个 `StdFn` |
| 4 契约测试硬编码 | `json_contract` 成员数与点名清单 |
| 5 模块登记 | 无新模块(若词法层独立成文件,注意 `src/` 文件数守卫) |
| 6 probe | ≥ 2 条(错误定位 / 路径取),`EXPECTED_CASE_COUNT` 同步 |
| 7 附录 G | **不动** |
| 8 skill 指针 | `SKILL.md` + `reference.md` §9.13;反模式加「`PARSE` 报错不带位置就完了」 |
| 9 CHANGELOG | 成员面 +11;**明标 `PARSE` / `STRINGIFY` 行为不变** |

## 6. 风险与已知代价

- ⚠️ **硬前置 00**:增量与路径操作要反复构建容器,在平方级成本上做 = 做出
  一个慢到没人用的面。**00 未落地前不要开写本份的 `JSON_CHUNK_ITEMS`。**
  (低层词法层是**只读扫描**,不依赖 00 —— 可以先落。)
- ⚠️ **schema 子集是「半张表」的同款风险**(§3.4)。若选了静默忽略,调用方
  会以为约束生效。倾向前者(报错)是因为 §12.1 立的纪律就是「半张表比没有
  更糟」。
- ⚠️ **`JSON_MERGE` 的语义含糊 = 配置事故源**。必须逐字写死并各有正反用例。
- **明确不做**:JSON 语法扩展 / YAML / TOML / CBOR / MessagePack /
  流式解析 / JSON Schema 全量 / 自动映射生成。

## 7. 检查点

| 子项 | 内容 | 状态 | commit |
|---|---|---|---|
| W-01 | 语义裁决(是否算重开流式 / merge 语义 / 未知关键字) | 未动工 | — |
| W-02 | 低层词法层 `JSON_TOKENIZE` / `JSON_LOCATE` / `JSON_VALIDATE`(**不依赖 00**) | 未动工 | — |
| W-03 | 定位契约(逐字节冻结) | 未动工 | — |
| W-04 | 路径族 `GET` / `SET` / `DELETE` / `MERGE` / `CHUNK_ITEMS`(**依赖 00**) | 未动工 | — |
| W-05 | `JSON_CHECK` + schema 子集契约 | 未动工 | — |
| W-06 | 性能 / 规模契约 / 收口 | 未动工 | — |
