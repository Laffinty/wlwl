# v0.8 偏差登记 (deviations)

> 维护说明:`docs/history/deviations-v0.7.md` 是 v0.7 的偏差档案,**已封存**(在 `docs/history/` 下)。
> 本文件是 v0.8 周期新增偏差登记,放在 `docs/plan/` 而非 `docs/history/`,体现
> "v0.8 实施期间活跃文档"的定位。v0.8 完成后整体迁入 `docs/history/`。
>
> 编号约定:`D8-NNN`,按发现/登记顺序连续递增。
> 每条 deviation 包含:发现时机、影响范围、处置、与 v0.7 兼容性的破坏程度。

---

## D8-001 · POP 注册表条目与 dispatch 实际语义不一致

- **状态**:已修复(commit 8XXXX 待补)
- **发现**:v0.8 §2.2 实施时(2026-09-23)
- **历史编号**:沿用 v0.7 的 `P4-B12-002`(lib.rs:1792 / 17947 注释中的旧编号)
- **影响范围**:`impl/crates/wlwl-eval/src/registry.rs::BUILTIN_REGISTRY` 中 `POP` 条目

### 现象
POP 注册表条目(`registry.rs:714-727` pre-v0.8):
```rust
BuiltinSpec {
    name: "POP",
    signature: "POP(arr) -> ARRAY (v0.4/v0.5 alias for AT_K semantics)",
    group: BuiltinGroup::Array,           // 错 — POP 不再是 ARRAY 操作
    err_consumer: ErrConsumerStatus::No,
    macro_fn: false,
    version: Version::V02,                // 错 — 3-arg 改写在 v0.6
    dispatch: DispatchStatus::ResolvedCompat,
    section: "§10.4",                     // 对 — 与 AT_K 同节
},
```

实际 dispatch 路由到 `builtin_at_k`(`lib.rs:4092`),实现的是 **3 元 DICT 取值**:
```rust
POP(["a":1,"b":2], "a", NULL) -> 1
```

7 个 `pop_dict_*` 测试(`lib.rs:13341-13386`)全部锁定 3 元 DICT 语义。

### 根本原因
v0.6(E decision)把 `POP` 重命名为 `AT_K`,同时把 `POP` 路由到 `builtin_at_k` 以保留 v0.5
源码兼容。但注册表条目文本没同步更新,残留 v0.4/v0.5 时代的 `POP(arr) -> ARRAY` 描述。

### 处置(v0.8 修复)
| 字段 | pre-v0.8 | post-v0.8 |
|------|----------|-----------|
| `signature` | `POP(arr) -> ARRAY (v0.4/v0.5 alias for AT_K semantics)` | `POP(d, k, default) -> v (v0.6 compat alias for AT_K; signature kept 3-arg)` |
| `group` | `BuiltinGroup::Array` | `BuiltinGroup::Dict` |
| `version` | `Version::V02` | `Version::V06` |
| 其他 | (保持) | (保持) |

dispatch 路径不动 — `lib.rs:4092` 仍路由到 `builtin_at_k`。

### 兼容性影响
- **零用户代码影响**:`builtin_at_k` 实现没动,7 个 `pop_dict_*` 测试照常通过。
- 仅注册表文档(metadata)对齐实际行为,便于读者按 §10.4 容器操作理解。

### 回归锁测试
新增 `pop_registry_entry_matches_dispatch`(`registry.rs::tests`),断言:
- `pop.group == BuiltinGroup::Dict`
- `pop.version == Version::V06`
- `pop.signature` 含 `d, k, default` 或 `3-arg`
- `pop.dispatch == ResolvedCompat` / `section == "§10.4"` / `!macro_fn`

---

## D8-002 · 注册表 anchor 章节号系统过期(per-entry section 实际生效)

- **状态**:已修复(commit `849dc3c`)
- **发现**:v0.8 §2.3 实施时(2026-09-23)
- **影响范围**:`registry.rs::BUILTIN_REGISTRY` 110 个 entry 中的 85 个 `section` 字段 +
  `generate_appendix_g_md()` 的列头硬编码

### 现象
plan §2.3 原文把变更目标定为 `BuiltinGroup::anchor()`(`registry.rs:91-108`)。
实施时检视发现:

1. `anchor()` 在 impl/ 工作区内**零调用方**(grep 全 workspace 无匹配)。
2. md 生成器 `generate_appendix_g_md()`(`registry.rs:1477-1484`)实际用每个
   `BuiltinSpec.section` 字段渲染"实现位置"列。
3. 因此改 `anchor()` 函数体对 `docs/appendix_G.md` 视觉输出**零影响**。
4. 真要修,必须改 110 个 entry 各自的 `section` 字段(其中 **85 个过期**)。
5. 列头硬编码还有两处 stale 引用:`ERR 消费者 (§12.7)` 和 `宏函数 (§3.4)` ——
   v0.7 spec §8.3 是消费者注册表、§1.4 是关键字。

### 根本原因
v0.7 spec 章节结构在 v0.7 周期内多次重构(`§10.x` 重排、`§12` 整体迁至 `§8.x`、
OOP 章节未实现),但 `registry.rs` 中 110 个 entry 的 `section` 字段是 v0.4/v0.6 时代
填写的,从来没批量同步过。`anchor()` 是早期预留的 group-level fallback 接口,
impl 内从未真正接入。

### 处置(v0.8 修复)
- 85 个 `BuiltinSpec.section` 字段按 21 个 `(group, current_section)` 桶批量对齐
  v0.7 spec 实际章节(详见 `docs/plan/wlwl-build-plan-v0.8.md` §2.3 表)。
- 列头 `§12.7 → §8.3`、`§3.4 → §1.4`(`registry.rs:1465`)。
- `BuiltinGroup::anchor()` 维持 v0.7 baseline 字符串 + 加 doc 注释说明 dead-code 现状。
- `docs/appendix_G.md` 通过 `cargo run --bin gen-appendix-g -- ../docs/appendix_G.md` 重生成。

### 兼容性影响
- **零行为影响**:纯文档元数据更新,所有 dispatch / 行为不变。
- `cargo test -p wlwl-eval --lib`:738 passed, 0 failed。

### 回归锁测试(待补 §2.9)
计划新增 `appendix_g_anchors_match_v07_section_numbers`:读取 `docs/appendix_G.md`,
对所有 110 行表格"实现位置"列做白名单校验,锚必须从 v0.7 spec 已定义章节集合
(`{"§2.x", "§4.x", "§6", "§8.x", "§9", "§10.x", "§11 [占位...]", "§17.x"}`)取。

---

## D8-004 · 字面量下标允许(规范与实现重新一致)

- **状态**:已修复(commit 待补)
- **发现**:v0.8 §2.4 实施时(2026-09-23)
- **影响范围**:
  - `impl/crates/wlwl-parser/src/lib.rs::parse_call_or_ident` 末尾 inline postfix loop
  - `impl/crates/wlwl-parser/src/lib.rs::parse_array_or_dict` 末尾
  - `parser/tests/spec_v3_alignment.rs`(新增 4 个 round-trip)
  - `eval/src/lib.rs::tests`(新增 4 个端到端)

### 现象
v0.7 spec §A.2 grammar `PostfixExpr = Primary { Postfix }` 而 `Primary` 含 `ArrayLit | DictLit`,
**字面上**允许 `[1,2,3][0]`;但 spec §4.5 prose 末句明确禁:

> "下标链挂在变量、调用或属性访问之后;对数组、字典字面量直接施加下标不在本规范内。"

而 parser `parse_array_or_dict` 在 v0.7 实现里也确实 reject 字面量下标 — 三层完全自相
矛盾,v0.7 行为偏向 prose。

### 根本原因
v0.4 时代 §A.2 grammar 与 prose 一致时,parser 拒绝字面量下标;v0.5/v0.6 演进时 grammar
放宽(允许 ArrayLit/DictLit 进入 PostfixExpr),prose 与 parser 都没跟上。Plan §2.4 报告选项
A(采纳:允许)是正确的修复方向,让三层都对齐 "允许"。

### 处置(v0.8 修复)
- 抽出 `apply_postfix_loop(&mut self, mut base, line, col) -> WlwlResult<Expr>`
  作为 parser 公共方法(parser.rs:1882 起 ~120 行)。
- `parse_call_or_ident` 末尾的 inline loop 替换为单行调用:
  `base = self.apply_postfix_loop(base, line, col)?;`
- `parse_array_or_dict` 末尾把 `Expr::Dict { ... }` / `Expr::Array { ... }` 包成 `base`,
  然后也调一次 `apply_postfix_loop`。
- v0.8 §3.15 同步删除 spec §4.5 末句的禁句(prose 改"允许")。

### 兼容性影响
- **可观察行为变更**(v0.7 → v0.8):`[1,2,3][0]` 从解析错误(`E0010` / `E0011`)变为 `1`。
  这是本次 v0.8 唯一会改变 v0.7 程序行为的边界场景,已在 plan §4.4 CHANGELOG
  兼容承诺段标注。
- 现有 v0.7 不会因此破坏,因为 v0.7 写不出 `[1,2,3][0]` 这种代码。

### 回归锁测试(新增 8 项)
parser(4 round-trip):
- `parser_array_literal_subscript_roundtrip`
- `parser_dict_literal_subscript_roundtrip`
- `parser_mixed_literal_subscript_chain`
- `parser_nested_literal_subscript_in_array`

eval(4 端到端):
- `eval_array_literal_subscript`
- `eval_dict_literal_subscript`
- `eval_chained_literal_subscript`
- `eval_literal_subscript_with_set_sugar`

`cargo test --workspace` ~1366 项全绿(原 ~1358 + 新增 8)。

---

## D8-005 · §12 重写:附录 G 注册表为单一真相源

- **状态**:已修复(commit `b266700`)
- **发现**:v0.8 §3.2 实施时(2026-09-23)
- **影响范围**:`docs/standard/wlwl-spec-v0.7.md` §12

### 现象
v0.7 §12 列了 6 个"保留形式"(`CLASS` / `NEW` / `THIS` / `MODULE` / `MODULE_REF` / `CALL` / `ARRAY` / `AND` / `OR`),prose 写"求值产生 E0020 / E0030"等。这些名字在 `BUILTIN_REGISTRY` 里都有正式 `BuiltinSpec` 条目 — 实测运行时是合法的 `ResolvedBuiltin` / `LexerMacro` / `ResolvedCompat`,不会触发保留诊断。表格与现实已脱节。

### 根本原因
v0.4 时代 §12 是"我们有名字但还没实现"的占位清单。v0.5 / v0.6 / v0.7 把每个条目落地为注册表,但 prose 一直没删 / 改。读者误以为这些"保留"是未文档化行为。

### 处置(v0.8 §3.2)
- §12 头部改为:"v0.8 起本章不再列出'保留形式'清单。... 全部以附录 G 注册表为准 — 该表是单一真相源"。
- §12.1 "真正的保留集合"表为空(v0.8 没有未注册且被词法保留的具名构造)。
- 列出实际守注册表与附录 G 的锁测试:
  - `b11_registry_count_matches_spec_table`(总数 = 110;93 v0.6 + 17 v0.7 §17)
  - `b11_registry_covers_resolve_builtin`
  - `b11_resolve_builtin_covers_registry`
  - `b11_err_consumer_registry_consistent`
  - `b11_macro_fn_attribute_matches_dispatch`
  - `registry::tests::appendix_g_anchors_match_v07_section_numbers`(§2.9 新增)

### 兼容性影响
- **零行为影响**:纯 spec prose 改写。
- 锁测试覆盖确认所有历史条目在运行时实际有定义。

### 回归锁测试
既有 `b11_*` 五件 + §2.9 新增的 `appendix_g_anchors_match_v07_section_numbers` 守住。

---

## D8-006 · §4.3 NOT 透明传播规则反向

- **状态**:已修复(commit `32c9ada`)
- **发现**:v0.8 §3.4 实施时(2026-09-23)
- **影响范围**:`docs/standard/wlwl-spec-v0.7.md` §4.3 一行 prose

### 现象
v0.7 §4.3 写:

> "任一操作数为 ERR 时,除 NOT 外全部透明传播(8.2)"

即把 `NOT` 列为 §8.2 透明传播的唯一例外。**实现一直是反过来的**:`NOT` 不是 ERR 消费者(`registry.rs` §4.3 组 `ErrConsumerStatus::No`),`b9_not_with_err_arg_is_e0034`(`lib.rs:17612`)锁定 `NOT(ERR("e")) → E0102`。

### 根本原因
v0.6 之前 spec 早期版本可能把 NOT 设计成 ERR 消费者,prose 残留。实际代码从未实施。

### 处置(v0.8 §3.4)
- §4.3 prose 改写为"所有运算符调用按 §8.2 透明传播 — 包括 NOT。`BOOL(x)` 是单独的 ERR 消费者..."
- 推荐惯用法:`NOT(BOOL(ERR(...)))` → `FALSE`(BOOL 先消费,NOT 后取反)
- 实现不动 — `b9_not_with_err_arg_is_e0034` 一直守着正确行为

### 兼容性影响
- **零行为影响**:实现未变,prose 改写为匹配实际行为。
- 用户代码 `NOT(ERR(...))` 的结果不变(E0102),只是文档不再误导。

### 回归锁测试
`b9_not_with_err_arg_is_e0034` + `b11_err_consumer_registry_consistent`。

---

## D8-007 · §17.1 YIELD 位置限制从硬规则降为实现路径

- **状态**:已修复(commit `f028860`)
- **发现**:v0.8 §3.10 实施时(2026-09-23)
- **影响范围**:`docs/standard/wlwl-spec-v0.7.md` §17.1 + §17.7

### 现象
v0.7 §17.1 写:

> "`YIELD()` 必须出现在多语句 Block 的直接子项位置"

+ §17.7 限制行:`[v0.7.0] 见 §17.1 限制`。

把"v0.7.0 实现采用任务体静态切段"产物的限制写成 normative 硬规则。意味着未来若实现切换至挂起式调度(path A),该放宽会被视为"破坏性修改" — 实际上语言理想语义不需要这条限制。

### 根本原因
v0.7.0 实施选择 path B(静态切段)以避开 mid-body 挂起的复杂度。prose 把实施选择误升为语言规则。

### 处置(v0.8 §3.10)
- §17.1 改写:"YIELD() 的理想语义是出现在任何表达式位置...v0.7 / v0.8 参考实现采用任务体静态切段调度,因此仅支持 YIELD() 出现在多语句 Block 的直接子项位置...该限制是 v0.7 / v0.8 实现路径的产物,不是语言语义规则;未来版本(挂起式调度)可放宽而不视为破坏性修订。"
- §17.7 嵌套 YIELD 行改:`[v0.7.0]` → `[v0.7 / v0.8 实现路径]`,措辞增加"未来版本可放宽(见 §17.1)"
- §17.7 新增"尾部 YIELD"行(per plan G-09)

### 兼容性影响
- **零行为影响**:`yield_split.rs` 未变;`b9` yield 测试不受影响。

### 回归锁测试
既有 yield_split tests + `yield_directly_as_if_branch_is_rejected` 等。

---

## D8-008 · spec prose alignment batch(§3.1, §3.3, §3.5-§3.9, §3.11-§3.15)

- **状态**:已修复(commit `2bf9093`)
- **发现**:v0.8 Phase B 批量(2026-09-23)
- **影响范围**:`docs/standard/wlwl-spec-v0.7.md` 12 处 prose 改动(详见 commit message)

### 现象
v0.7 spec 与 §2.x 实施修正后的真实行为在多处 prose 上不一致;且部分章节存在长期歧义(版本记法、`=` 三重身份、`NOT` 在 §4.3 反向等)。

### 根本原因
v0.6 / v0.7 spec 期间迭代时,实施与 prose 同步有滞后;某些 prose 段落是 v0.4 / v0.5 时代表述。

### 处置(v0.8 Phase B 批量)
12 条 plan §3.x 合并 commit:

| plan 项 | 章节 | 改动 |
|--------|------|------|
| §3.1 | §8.2/§8.3/§8.5 | 三件示例重塑 + EXPECT_ERR 入表 + §8.5 顶层 ExprStmt 注 |
| §3.3 | §11.2 | E0055/E0057 标注"v0.7/v0.8 无触发路径" |
| §3.5 | §0.1/§17.7 | §0.1 加版本约定段;§17.7 历史 [v0.7.0] 行保留不动 |
| §3.6 | §10.7 | FORMAT `{N}` / `{name}` 混用规则 + 锁测试引用 |
| §3.7 | §1.6 | 拆 `int_lit` / `int_literal`,加 lexer 不读前导符号注 |
| §3.8 | §5.1 | LET/FUN 不对称 normative(LET 不得作实参) |
| §3.9 | §1.5 | `=` 三重身份消歧(OpToken / INDEX_SET / Param 默认) |
| §3.11 | §1.4 | CLASS/NEW/THIS 标注"已注册非保留" |
| §3.12 | §17.3/§17.5 | SHIELD 不创建新作用域 + SCOPE(ERR) 透传注 |
| §3.13 | §17.1 | AWAIT 宿主诊断 vs 用户 ERR 区分 |
| §3.14 | §2.2 | `%` 浮点 E0030 列入触发列表 |
| §3.15 | §4.5 | 字面量下标删禁句 |

### 兼容性影响
- **零行为影响**:纯 spec prose 修正;impl / 测试不变。
- `cargo test --workspace`:0 FAILED(commit 时验证)

### 回归锁测试
所有既有 b9_* / b11_* / pop_dict_* / literal_subscript_* / e0055_* / e0057_* 测试在 §2.x 系列 commit 已加 + 本批量不影响;回归风险低。

---

## D8-003 · E0055 / E0057 错误码预留(v0.7 / v0.8 无触发路径)

- **状态**:已修复(commit 待补)
- **发现**:v0.8 §2.6 实施时(2026-09-23)
- **影响范围**:
  - `impl/crates/wlwl-error/src/lib.rs` `ErrorCode` enum 注释行(头部 doc + 单行注释)
  - `impl/crates/wlwl-eval/src/lib.rs::tests`(新增 2 个锁测试)

### 现象
v0.7 注册表里 `ErrorCode` enum 包含 `E0055` 和 `E0057` 两个码,但**生产代码无任何触发路径**:

- **E0055**:原意"CHANNEL_RECV / TRY_RECV 在通道关闭后产生原生错误码"。
  实际路径(channel.rs D-C):RECV 关闭后返回结构化 `Value::Err(Dict{kind: "ChannelClosed", ...})`
  字典载荷,**不是** WlwlError(E0055)。
- **E0057**:原意"跨任务共享单元格但单元格不可变 → 原生错误码"。
  实际路径:`SET` 对不可变单元格抛 `WlwlError(E0024)`(immutable-cell 通用码),
  没有专门的跨任务检查路径,因此 E0057 永不被发射。

两个码都是 v0.4/v0.5 时代预留,v0.6/v0.7 重构时实现路径绕开了它们,但 enum 保留 ——
读者误以为"还存在未文档化行为"。

### 根本原因
v0.6 §8.x 重构 ERR 处理时,把"通道关闭"信号从 native code 改为 dict 载荷(更易跨任务边界
保留 kind 字段),把"不可变单元格"统一为 E0024。但 enum 的 `E0055` / `E0057` 残影没清理 —
可能是"将来切换至原生码"的占位,也可能是疏忽。Plan §2.6 显式标"v0.7 / v0.8 无触发路径"。

### 处置(v0.8 修复)
- `wlwl-error/src/lib.rs` 头部 master 注释行 + 单行 enum 注释都改为 "RESERVED":
  ```
  E0055, // RESERVED — v0.7/v0.8 无触发路径;见 deviation D8-003
  E0057, // RESERVED — v0.7/v0.8 无触发路径;见 deviation D8-003
  ```
  + 引用 §8.1 ERR(kind="ChannelClosed") 载荷路径与 E0024 统一路径。

### 兼容性影响
- **零行为影响**:只是文档元数据,E0055 / E0057 本来就没触发过。
- 不改 dev path,不删 enum(怕外部 crate 引用)。

### 回归锁测试(新增 2 项)
Plan §2.6 写 `wlwl-error/tests/` 加;实际落地在 `wlwl-eval/src/lib.rs::tests` —
因为 wlwl-error 没有 wlwl-eval dev-dep,需要 eval 才能真跑触发路径:
- `e0055_channel_recv_after_close_does_not_raise_native_code`:
  CHANNEL_NEW → CLOSE → TRY_RECV → 断言 `Value::Err(payload)` 且 `AT_K(ERR_PAYLOAD(v), "kind", "?") == "ChannelClosed"`,
  NOT WlwlError(E0055)。
- `e0057_immutable_cell_set_raises_e0024_not_e0057`:
  `LET(y, 0); SET(y, 1);` → 断言 WlwlError.code == E0024, NOT E0057。
  (单 task 形式 — runtime 把单/跨任务不可变单元格 mutation 都坍缩到 E0024,
  E0057 没有专属路径,assert "无 E0055/E0057 出现" 即可锁住意图。)

`cargo test --workspace` 全绿;eval 746 passed(原 744 + 2 新)。

---

## D8-009 · §1.7 浮点指数字面量启用(v0.8.0 漏实现)

- **状态**:已修复(commit 待补;v0.8.1 第一轮)
- **发现**:v0.8.1 F-A 由 `docs/AUDIT_REPORT.md §1.1`(2026-09-23 独立第三方测评)+ `docs/plan/wlwl-build-plan-v0.8.1.md §1.1` 驱动
- **影响范围**:
  - `impl/crates/wlwl-lexer/src/lib.rs::read_number` line 311-359(原 299-310,加 exponent 分支)
  - `impl/crates/wlwl-lexer/src/lib.rs::tests` 新增 6 项(原 23 → 29)
  - `impl/crates/wlwl-eval/src/lib.rs::tests` 新增 6 项 end-to-end(原 748 → 754)
  - `wlwl-spec-v0.8.md` 不动(用户约束:v0.8.x 严格遵循 v0.8 spec)

### 现象
第三方审计与本仓实测一致:

```
$ wlwl.exe run -e "PRINT(1.5e2)"
error[E0011]: expected ')', got Ident("e2")
```

spec §1.7 EBNF:
```
float_lit      = digits "." digits | digits exponent | digits "." digits exponent .
exponent       = ( "e" | "E" ) [ "+" | "-" ] digits .
```

三种 float 形态显式承诺,但 lexer 实现只覆盖 `digits "." digits` 一支,
其余含 exponent 的两条路径直接被拒绝,`1.5e2` 在 parser 阶段报 E0011。

### 根本原因
`impl/crates/wlwl-lexer/src/lib.rs` 原 `read_number`:

```rust
// (修前)
let mut is_float = false;
if self.peek() == Some(b'.') && matches!(self.peek_at(1), Some(b) if b.is_ascii_digit()) {
    is_float = true;
    self.bump(); // '.'
    while /* digits */ { ... }
}
let text = std::str::from_utf8(&self.src[start..self.pos]).unwrap();
let (kind, span) = if is_float {
    Float(text.parse()?)
} else {
    Integer(text.parse()?)
};
```

只在 `digits "." digits` 后停步,从不消费 `e`/`E`。`e2` 退回主循环被识别为 identifier 首字母,
`read_ident_or_keyword` 弹出 `Ident("e2")` token,与先行的 `Integer(1)` 在 parser
层合成 `1 e2`(E0011 期望 `,` 或 `)`)。

### 处置
仅 lexer 一处改动 + 两组测试。在 `read_number` 的小数部分消费后插入
**exponent 分支**:

```rust
if matches!(self.peek(), Some(b'e') | Some(b'E')) {
    let exp_payload_present = matches!(
        self.peek_at(1),
        Some(b'+') | Some(b'-')
    ) || matches!(self.peek_at(1), Some(b) if b.is_ascii_digit());
    if exp_payload_present {
        is_float = true;
        self.bump(); // 'e' | 'E'
        if matches!(self.peek(), Some(b'+') | Some(b'-')) {
            self.bump();
        }
        let exp_start = self.pos;
        while /* digits */ { ... }
        if self.pos == exp_start {
            // 1e+ / 1e- 无 digits:spec 不容,返 E0001
            return Err(/* invalid float exponent in '...' */);
        }
    }
}
```

**两个关键边界**:

1. **`exp_payload_present` 守卫**:必须先验 `e`/`E` 之后是 `+`/`-`/digit,否则
   不消费、退回整数路径。这保留了 `1east` 仍 lex 为 `Integer(1) + Ident("east")`
   的既有行为,防止"任何字母 e"被误吞成 exponent。

2. **`exp_start == pos` 守卫**:仅当 sign 已消费但后无 digits 时触发(`1e+` / `1e-`
   形态)。spec §1.7 不容无 digits 的 exponent,故抛 E0001 而不是悄悄降级为
   整数(那会掩盖真实错误)。`1e+` / `1e-` / `1.5e+` 都覆盖。

### 兼容性影响
**零行为扩展**(纯增量):
- `1.5e2` / `1e2` / `1.5e-2` / `1E3` / `2.5e+1` 等 spec §1.7 显式承诺的字面量 v0.8.0
  报 E0011,v0.8.1 起通过 parse + 求值到正确 f64。
- `1east` / `1eval` / `1empty` 等"数字 + 字母 ident"形态保持原有 lexer 行为
  (`Integer(1)` + `Ident("...")`),`exp_payload_present` 守卫守住这条不变式。
- 现有 23 个 lexer 数字/标识符/操作符测试全部不受影响(无 `e`/`E` 后的数字
  payload 输入,旧路径走到整数终点)。
- 整 workspace 测试 ~1380 项全绿(原 ~1366 + lexer 6 + eval 6)。

唯一可见的字典化升级:`1e` 之类输入在 v0.8.0 走 `Integer(1) + Ident("e")` 路径(lexer
不报错,但 parser 后续可能因 `e` 不是合法表达式位置 E0010);v0.8.1 起 `1e` 视作
`exp_payload_present = false`(peek_at(1) 是 None 不是 sign/digit)退回整数,parser
层不变。

### 回归锁测试
**新增 6 项 lexer**(位于 `impl/crates/wlwl-lexer/src/lib.rs::tests`):
- `lex_exponent_digits_only`:`1e2` → `Float(100.0)` 锁 `digits exponent` 形态
- `lex_exponent_with_fraction`:`1.5e2` → `Float(150.0)` 锁 `digits "." digits exponent` 形态
- `lex_exponent_negative`:`1.5e-2` → `Float(0.015)` 锁负号分支
- `lex_exponent_uppercase_e`:`1E3` → `Float(1000.0)` 锁 `E` 大写等价
- `lex_exponent_after_letter_is_ident`:`1east` → `Integer(1) + Ident("east")` 锁**不消费**守卫
- `lex_exponent_missing_digits_after_sign_errors_e0001`:`1e+` → E0001 + 文案含 "invalid float exponent"

**新增 6 项 eval end-to-end**(位于 `impl/crates/wlwl-eval/src/lib.rs::tests`,
与 D8-008 锁测试集并列,§2.9 consistency tests):
- `eval_floating_exponent_digits_only`:`1e2;` → `Float(100.0)`
- `eval_floating_exponent_with_fraction`:`1.5e2;` → `Float(150.0)`
- `eval_floating_exponent_uppercase_e`:`1E3;` → `Float(1000.0)`
- `eval_floating_exponent_negative_sign`:`1.5e-2;` → `Float(0.015)`
- `eval_floating_exponent_positive_sign`:`2.5e+1;` → `Float(25.0)`
- `eval_floating_exponent_in_let_binding`:`LET(x, 1.5e2); x;` → `Float(150.0)`
  (与 audit §1.1 复现脚本同形,通过 lexer + parser + eval 全链路)

---

## D8-010 · §10.5 SUB 第三参数严格按 length 实现(原 end-index 语义错位)

- **状态**:已修复(commit 待补;v0.8.1 第二轮,**唯一可观察行为变更项**)
- **发现**:v0.8.1 F-B 由 `docs/AUDIT_REPORT.md §7.1`(2026-09-23 独立第三方测评)+ `docs/plan/wlwl-build-plan-v0.8.1.md §1.2` 驱动
- **影响范围**:
  - `impl/crates/wlwl-eval/src/lib.rs::builtin_substr` line 2066-2138(原 2066-2125,重写 length 语义)
  - `impl/crates/wlwl-eval/src/lib.rs::tests` 三处旧测试断言按 spec §10.5 重写 + 新增 3 项(原 754 → 757)
  - `wlwl-spec-v0.8.md` 不动(用户约束)

### 现象
第三方审计 §7.1:

```
SUB("Hello, world", 0, 5)  → "Hello"     OK
SUB("Hello, world", 1, 5)  → "ello"      OK   (注:audit 此行按 end-index 算)
SUB("Hello, world", 7, 1)  → ""          BUG (expect "w")
SUB("Hello, world", 7, 3)  → ""          BUG
SUB("Hello, world", 7, 5)  → ""          BUG
SUB("Hello, world", 0)     → "Hello, world"   OK
```

实测本仓 v0.8.0:

```
$ wlwl.exe run test_audit/02_sub_bug.wll
Hello
ello

                ← (空行;SUB("Hello, world", 7, *) 全空)
Hello, world
```

spec `docs/standard/wlwl-spec-v0.8.md:649-650` §10.5:
```
| `SUB(s, start, len?) -> STRING` | 自 `start` 起长 `len` 的子串;缺省 `len` 到末尾;按码点 |
```

第三参数 spec 显式承诺是 **length** (起 start 长 len)。impl 把第三参数当 **end-index** 处理,
所有 `SUB(s, 7, *)` 因 `end ≤ start` 走 `norm_end <= norm_start` 早返 "" 分支。

### 根本原因
`impl/crates/wlwl-eval/src/lib.rs` 原 `builtin_substr` (line 2069-2125):

```rust
// (修前,节录)
let end_raw = if args.len() == 3 {
    /* 读 third arg into end_raw */
} else {
    len
};
let norm_end = if end_raw < 0 {
    (end_raw + len).max(0)
} else {
    end_raw.min(len)
};
if norm_end <= norm_start {
    return Ok(Outcome::normal(Value::String(String::new())));
}
// chars[start..end]
```

变量名 + doc comment 都按 `end` 处理:
- 函数 doc(原 line 2066):"`SUB(s, start, end?) -> STRING`: 半开区间 [start, end)"
- 实现语义:`chars[norm_start..norm_end]`
- 副作用:`SUB(s, n, k)` 当 `k ≤ n` 时永远返 `""`(因为 norm_end ≤ norm_start)

SLICE 数组版的 `builtin_slice` (line 1840) 也是 end-index,但 SLICE 是数组语义,
spec §10.4 "区间 [start, end)" 措辞本身允许 end-index。SUB 的 spec §10.5 措辞
"长 len 的子串" 不允许 — v0.8.0 impl 一开始就把 SUB 当 SLICE 抄,
无视 §10.5 字面承诺。

### 处置
**重写 `builtin_substr` 的 length 路径** (line 2066-2138):

```rust
let len_raw = if args.len() == 3 { /* type-check as INTEGER */ } else {
    len - norm_start  // 缺省:从 norm_start 取到末尾
};
// spec §10.5: len 须为非负;负 len clamp 到 0(空串)。
let norm_len = if len_raw < 0 { 0 } else { len_raw };
if norm_len == 0 || norm_start >= len {
    return Ok(Outcome::normal(Value::String(String::new())));
}
let chars: Vec<char> = s.chars().collect();
let start = norm_start as usize;
let end = (norm_start + norm_len).min(len) as usize;  // 单边 clamp 到字符串末尾
Ok(Outcome::normal(Value::String(chars[start..end].iter().collect())))
```

**两个关键边界**:

1. **负 len clamp 到 0**(`norm_len = max(len_raw, 0)`):spec §10.5 不接受负 len,
   沿用 SLICE 的负 end "从尾数"语义会与 length 语义混淆,故删除。
   旧 `SUB("abc", -3, -1) = "ab"` 是把 -1 当 end-position-from-tail 算的;
   新语义下 `SUB("abc", 0, -1) = ""`。需要 "tail substring via negative index"
   的用户改用负 start: `SUB(s, -(end_pos), end_pos - start_pos)`。

2. **start + len 单边 clamp 到字符串末尾** (`end = min(start + len, len)`):
   不对称 — start clamp 在 norm_start 已做,len 端只在越界时钳,
   不在越界时悄返 ""。`SUB("Hello", 0, 100) = "Hello"`(不是 "")。

**doc comment 同步重写**:
```rust
/// `SUB(s, start, len?) -> STRING`: 自 `start` 起长 `len` 的子串。
/// start INTEGER(可为负,从尾数计;越界 clamp 到 [0, len_total])；
/// len INTEGER(可省,缺省取到末尾;非负;负 len 视为 0);按码点。
///
/// v0.8.1 D8-010: spec §10.5 字面承诺 "自 start 起长 len 的子串" —
/// 第三参数是 **长度**,不是 end-index。
```

### 兼容性影响
**6 步 v0.8.1 修复中唯一会改变可观察行为**的一项。理由:

- spec §10.5 在 v0.6 → v0.7 → v0.8 三轮都是 "len" 措辞。
- v0.8.0 是"实现落后于 spec",不是 spec 变更。
- 但凡是用 3-参 `SUB(s, start, end_old)` 当 end-index 用的 v0.8 程序
  在 v0.8.1 后都会得到不同的字符串输出。已更新既有 `b13_sub_*` 系列测试
  反映新语义。

**8 行矩阵的兼容性差异表**(spec §10.5 → 新行为):

| `SUB(s, start, len)` | v0.8.0(旧) | v0.8.1(新) | 用户迁移 |
|---------------------|-----------|-----------|---------|
| `("Hello, world", 0, 5)` | `"Hello"` | `"Hello"` | — |
| `("Hello, world", 1, 5)` | `"ello"` | `"ello,"`(chars[1..6]) | 想要 `"ello"`:`SUB(s, 1, 4)` 或 `SLICE(s, 1, 5)` |
| `("Hello, world", 7, 1)` | `""` | `"w"` | 想要 `""`: `SUB(s, 7, 0)` |
| `("Hello, world", 7, 3)` | `""` | `"wor"` | — |
| `("Hello, world", 7, 5)` | `""` | `"world"` | — |
| `("Hello", -1, 1)` | `""` | `"o"`(start norm=4, len=1) | — |
| `("Hello", 0, -1)` | (走 end-index 把 -1 当尾数,返 "Hello") | `""`(负 len clamp) | 需要 tail-取子串:`SUB(s, -n, n)` 走负 start |
| `("Hello, world", 0)` | `"Hello, world"` | `"Hello, world"` | — |

CHANGELOG v0.8.1 节需要单独标注:
> "**§10.5 SUB 第三参数严格按 length 实现**(D8-010,observable change):
> 迁移:`SUB(s, start, end_old)` → `SUB(s, start, -(start, end_old))` 或
> `SLICE(s, start, end_old)`。"

### 回归锁测试
**新增 3 项 end-to-end**(位于 `impl/crates/wlwl-eval/src/lib.rs::tests`):
- `substr_length_semantics_8_cases`:覆盖上表 8 行(含 audit §7.1 的 "world"
  reproducer),逐条 assert v0.8.1 长度语义输出。
- `substr_len_overflow_clamps_to_string_end`:`SUB("Hello", 0, 100)` → `"Hello"`、
  `SUB("Hello", 3, 100)` → `"lo"`、`SUB("Hello", 5, 100)` → `""`。
  单边 clamp 到 LEN(s),不走 "end ≤ start → 空" 路径。
- `substr_len_zero_returns_empty`:`SUB("Hello", 0, 0)` / `(5, 0)` / `(0, -7)` / `(100, 5)`
  全返 `""`,锁早返分支。

**重写 3 件旧 `b13_sub_*` 测试的断言**(test 名不变,断言按 spec §10.5 更新):
- `b13_sub_basic_negative_oob`:
  - `SUB("hello", 1, 3)` → `"ell"`(原 `1, 4` → `"ell"` 在新语义下变 `"ello"`,故改 length)
  - `SUB("hello", 3, 0)` / `SUB("hello", 0, 0)` → `""`(替代旧 `3, 3` → `""`,
    后者在新语义下变 `"lo"`)
- `b13_sub_negative_index_normalization`:
  - 保留 `SUB("abc", -1)` → `"c"`(2-arg default len)
  - 删 `SUB("abc", -3, -1) → "ab"`(依赖旧 end-from-tail,新语义返 "")
  - 加 `SUB("abc", -2, 2)` → `"bc"`(表达"负 start 取尾"的现代写法)
  - 加 `SUB("abc", 0, -1)` → `""`(显式断言负 len → 0)
- `b13_unicode_preserved`:
  - `SUB("héllo", 1, 3)` → `"éll"`(原 `1, 4` → `"éll"` 在新语义下变 `"éllo"`,
    故改 length 为 3)

`cargo test --workspace`:~1380 项全绿(原 ~1378 + SUB 新 3 项);b13_sub_* 三件
全部仍 pass(断言重写后语义对齐 spec §10.5)。

---

## D8-011 · §1.4 MUT 作为普通标识符在所有非 modifier 位置允许

- **状态**:已修复(commit 待补;v0.8.1 第三轮)
- **发现**:v0.8.1 F-C 由 `docs/AUDIT_REPORT.md §1.2`(2026-09-23 独立第三方测评)+ `docs/plan/wlwl-build-plan-v0.8.1.md §1.3` 驱动
- **影响范围**:
  - `impl/crates/wlwl-parser/src/lib.rs::parse_let` line 829-892(在原 match arm 中增补 `TokenKind::Mut` arm)
  - `impl/crates/wlwl-parser/src/lib.rs::parse_expr` line 798-803(`Mut => parse_call_or_ident()` arm)
  - `impl/crates/wlwl-parser/src/lib.rs::parse_call_or_ident` line 1806-1809(`Mut => "MUT"` name)
  - `impl/crates/wlwl-parser/src/lib.rs::parse_pattern` line 958-962(`Mut => Pattern::Ident("MUT")`)
  - `impl/crates/wlwl-parser/src/lib.rs::parse_fun` line 1382-1395(`Mut` 参数名 arm)
  - `impl/crates/wlwl-parser/tests/spec_v3_alignment.rs` 新增 4 件(原 80 → 84)
  - `impl/crates/wlwl-eval/src/lib.rs::tests` 新增 4 件(原 757 → 761)
  - `wlwl-spec-v0.8.md` 不动(用户约束)

### 现象
第三方审计 §1.2:

```
LET(MUT, "x")   →  E0010 expected identifier in LET, got Mut
LET MUT(MUT, "x") → 同上
```

spec `docs/standard/wlwl-spec-v0.8.md:81` §1.4:

> "`MUT` 是**上下文关键字**:仅在 `LET` 之后的位置具有特殊含义(3.1);**其他位置可作普通标识符使用**,不与关键字冲突。"

实测 `parse_let` 把 binding 槽位(在 `(` 之后)也当作 modifier 槽,只接受
`TokenKind::LBracket | TokenKind::Ident(_)`,命中 `_` arm 抛 E0010。

本仓实测:

```
$ wlwl.exe run -e 'LET(MUT, 99)'
error[E0010]: expected identifier in LET, got Mut
```

### 根本原因
`impl/crates/wlwl-parser/src/lib.rs` 原 `parse_let` (line 818-892) 假设
"`MUT` 是 LET 后续必走的 modifier 路径"的二元决策:modifier slot 之后
第一个 token 必须是 binding pattern(纯 ident / LBracket destructuring),
`MUT` 永远走 modifier slot。这一假设在 spec §1.4 之后已不成立 — spec
明确 "其他位置可作普通标识符"。

类似地,`parse_expr` / `parse_call_or_ident` / `parse_pattern` / `parse_fun`
四处均只接受 `TokenKind::Ident(_)` 作为名字,把 `MUT` 全部排除在 identifier
外,与 spec §1.4 的 "其他位置可作普通标识符" 直接冲突。

### 处置
按 spec §1.4 "其他位置可作普通标识符" 全放宽,**5 处 dispatch 点**加
`TokenKind::Mut` 作为普通标识符 `MUT` 处理:

1. **`parse_let` binding 槽位**(line 883-914):新增 `TokenKind::Mut` arm,
   直接构造 `Expr::Let { name: "MUT", mut_: is_mut, ... }`。第一处。

2. **`parse_expr` 顶层 dispatch**(line 798-803):新增
   `TokenKind::Mut => self.parse_call_or_ident()`。这让 `PRINT(MUT)` 、
   `+(MUT, 1)` 等含 MUT 的表达式合法 — `parse_call_or_ident` 会进一步
   把 Mut 解析为 Var / Call("MUT")。

3. **`parse_call_or_ident` name match**(line 1806-1809):新增
   `TokenKind::Mut => "MUT".to_string()`,与 `Class | New | This` 同款
   contextual-keyword-as-ident 路径。

4. **`parse_pattern` 第一个 token**(line 958-962):新增
   `TokenKind::Mut => Pattern::Ident("MUT", span)`,让 `FUN((MUT), ...)` 、
   `LET([MUT, ...], arr)` 等 pattern 位置接受 MUT 作 binding name。

5. **`parse_fun` 参数列表**(line 1382-1395):在
   `match self.advance() { Token { kind: TokenKind::Ident(s), ... } ... }`
   里增补 `TokenKind::Mut` 分支,参数名 = "MUT"。

> **注意**: build plan §2.3 推荐 (A) "更窄只影响 LET",但实测发现
> 仅有 LET 改动不能让 `LET(MUT, "x"); PRINT(MUT);` 通过(`PRINT(MUT)`
> 的 MUT 在 expr 位置仍被当关键字拒)。spec §1.4 字面承诺"其他位置可
> 作普通标识符",所以全 5 处均放宽 — 这是 spec 正确读法,plan §2.3 的
> (A) 推荐是 hedge,经实测证明不符合 spec 字面。

### 兼容性影响
**零行为扩展**(纯增量):
- v0.8.0 报 E0010 / E0011 的输入现在通过 — `LET(MUT, 99)` /
  `PRINT(MUT)` / `+(MUT, 1)` / `FUN((MUT), ...)` / `LET MUT(MUT, ...)` /
  `LET(MUT: INTEGER, 0)` 全部合法。
- 不引入新的 keyword 槽位,不影响其它 token kind。
- `is_mut = true` (modifier 槽位的 MUT) + binding 名 "MUT" (binding 槽
  位的 MUT) 并存:第一处 MUT 决定 mut_,第二处 MUT 决定 name — 与 spec
  §3.1 + §1.4 双约束。
- `MUT` 不在 BUILTIN_REGISTRY 110 条之列(prose 强调"MUT 不与关键字
  冲突"),shadow 检查 (§3.5) 不会触发 E0025 / W0030。
- 既存测试不受影响:原 23 件 lexer + 80 件 parser + 754 件 eval 测试在
  改动后仍 pass。

### 回归锁测试
**新增 4 件 parser round-trip**(位于
`impl/crates/wlwl-parser/tests/spec_v3_alignment.rs`,`§3.1 Chinese identifiers`
段之后):
- `let_paren_mut_as_binding_name_roundtrips`:`LET(MUT, 99)` →
  `Expr::Let { name: "MUT", mut_: false }`
- `let_double_mut_first_modifier_second_binding`:`LET MUT(MUT, 99)` →
  `Expr::Let { name: "MUT", mut_: true }`(锁第一 MUT 是 modifier、第二 MUT 是 binding name)
- `let_mut_as_binding_does_not_shadow_built_in`:`LET(MUT, 99)` parse
  成功,MUT 不触发 E0025(spec §3.5 检查 BUILTIN_REGISTRY)
- `let_mut_as_binding_with_type_annotation`:`LET(MUT: INTEGER, 0)` →
  `Expr::Let { name: "MUT", mut_: false, type_annotation: Some(...) }`

**新增 4 件 eval end-to-end**(位于
`impl/crates/wlwl-eval/src/lib.rs::tests`,与 D8-009 / D8-010 锁测试集
并列,§2.9 consistency tests):
- `eval_let_paren_mut_as_binding_name_immutable`:`LET(MUT, "x"); PRINT(MUT);`
  parse + 求值通过,验证 binding 解析。
- `eval_let_double_mut_creates_mutable_cell`:`LET MUT(MUT, 1); SET(MUT, +(MUT, 1)); MUT;`
  → `Integer(2)`,验证 SET 写可变单元格 + 表达式位置 MUT 解析为 var。
- `eval_let_mut_as_binding_referenced_as_value`:`LET(MUT, 42); +(MUT, 1);`
  → `Integer(43)`,验证 call-arg 位置 MUT 解析为 var。
- `eval_let_mut_in_fun_param_does_not_collide`:`LET(f, FUN((MUT), +(MUT, 1))); LET(_r, f(99));`
  parse + 求值通过,验证 FUN 参数位置接受 MUT。

`cargo test --workspace`: ~1388 项全绿(原 ~1380 + parser 4 + eval 4);
wlwl-spec-v0.8.md 不动;D8-009 + D8-010 锁测试不受影响。

---

## D8-012 · §A.2 grammar 字符串字面量允许下标 postfix (§4.5 prose 跟进留 v0.9)

- **状态**:已修复(commit 待补;v0.8.1 第四轮)
- **发现**:v0.8.1 F-D 由 `docs/AUDIT_REPORT.md §4.4`(2026-09-23 独立第三方测评)+ `docs/plan/wlwl-build-plan-v0.8.1.md §1.4` 驱动
- **影响范围**:
  - `impl/crates/wlwl-parser/src/lib.rs::parse_literal` line 2083-2149(在 StringLit 分支后挂 `apply_postfix_loop`)
  - `impl/crates/wlwl-parser/tests/spec_v3_alignment.rs` 新增 5 件(原 84 → 89)
  - `impl/crates/wlwl-eval/src/lib.rs::tests` 新增 4 件(原 761 → 765)
  - `wlwl-spec-v0.8.md` 不动(用户约束)

### 现象
第三方审计 §4.4:

```
LET(_x, "hi"[5]);    // E0011 expected ')', got LBracket
```

实测:

```
$ wlwl.exe run -e 'LET(_x, "hi"[0])'
error[E0011]: expected ')', got LBracket
```

但同一 expr 顶层位置:`[1, 2, 3][0]` 通过(D8-004 v0.8 已修),而 `"hi"[0]` 失败。

spec `docs/standard/wlwl-spec-v0.8.md`:

> §A.2 grammar line 1030-1036:
> ```
> PostfixExpr = Primary { Postfix } .
> Primary     = Literal | identifier | OperatorCall | ...
> Postfix     = "." identifier [ "(" [ Expression { "," Expression } ] ")" ]
>             | "[" Expression "]" | "[" Expression "]" "=" Expression .
> ```
>
> §4.5 prose line 321:
> > "INDEX_GET(coll, k):...**字符串接受整数索引**,按码点返回单字符字符串,越界产生 E0036"

`Literal` 是 `Primary` 的合法形态 → `PostfixExpr` 可挂 `[...]` postfix。spec
§4.5 prose 同节又显式承诺 INDEX_GET 接受字符串 receiver。

但 §4.5 prose line 313-323 末段:
> "下标链可挂在变量、调用、属性访问、**或数组/字典字面量之后**(v0.8 起;...)。"

只列举 array / dict literal — 没有显式包含 string literal。

### 根本原因
`impl/crates/wlwl-parser/src/lib.rs::parse_literal` (line 2083-2120 原版)
对 `TokenKind::LBracket` 调用 `parse_array_or_dict`(已在末尾挂
`apply_postfix_loop`,D8-004 v0.8 修复);对 `TokenKind::StrStart` 调用
`parse_interpolated_string`(没有 postfix 循环);对其余字面量
(Integer / Float / StringLit / True / False / Null) 直接返回
`Expr::Literal(...)`,**不挂 postfix 循环**。

`"hi"[0]` 经 lexer → `TokenKind::StringLit("hi")` → parse_expr 调
`parse_literal` → 消费 StringLit 后停在 `]`,parser 主循环在 `]` 处
期望 `,` 或 `)`,报 E0011。

audit §4.4 reproducer `LET(_x, "hi"[5])` 同样停在 LET 的 `)` 期望:
consume `LET(`,进入 binding slot parse_expr,parse_expr 调 parse_literal
消费 `"hi"`,停在 `]`,parser 主循环此时在 `(` 解析 LET 的 binding
实参 list,期望 `,` 或 `)`,报 E0011 expected ')' got LBracket。

### 处置
按 spec §A.2 grammar 字面读法("Primary = Literal" + "PostfixExpr =
Primary { Postfix }") + §4.5 prose ("INDEX_GET 字符串接受整数索引"),
在 `parse_literal` 末尾对 `TokenKind::StringLit` 调用 `apply_postfix_loop`,
与 `parse_array_or_dict` 末尾同款形态:

```rust
// 仅 StringLit 走 apply_postfix_loop(其它字面量 postfix 无语义):
//   "hi"[0]            → INDEX_GET("hi", 0)
//   "hi"[1]            → INDEX_GET("hi", 1)
//   "hi"[0] = "x"      → INDEX_SET("hi", 0, "x")(eval 端会拒,spec §4.5)
// 整数字面量 / 浮点字面量 / Boolean / Null 不允许 postfix:
//   3[0]               → parser 仍报 E0010 / E0011 (现有路径 unchanged)
//   TRUE[0]            → 同上
if is_string_lit {
    base = self.apply_postfix_loop(base, line, col)?;
}
```

`is_string_lit` 在 match arm 前用 `matches!` 捕获(`t.kind` 一次性 move 后
不能再访问),`apply_postfix_loop` 是已经在 `parse_call_or_ident` /
`parse_array_or_dict` 末尾复用的公共方法。

注意:此修复扩大了字面量 postfix 语义的覆盖面,**超出当前 §4.5 prose 字面
列举的"array/dict literal"**。但 §A.2 grammar 与 §4.5 INDEX_GET 字符串
receiver 联合支持,且 spec §0.1 v0.8 放宽原则下"宁滥勿缺"。

**spec 跟进**留 v0.9:
> §4.5 prose 末句 "或数组/字典字面量之后" 建议改为
> "**或字面量之后**(字符串按 §4.5 INDEX_GET 接受整数索引,
> 数组/字典同款;整数 / 浮点 / Boolean / Null 字面量 postfix 无 INDEX_GET
> 语义,仍报 E0010 / E0011)" — 这是 v0.9 spec 修订项,本轮 v0.8.1 不改
> spec。

### 兼容性影响
**零行为扩展**(纯增量):
- `"hi"[0]` / `PRINT("hi"[1])` / `LET(x, "hi"[0])` 等顶层 / call arg / LET
  槽位的字符串字面量下标 v0.8.0 报 E0011,v0.8.1 起通过 parse + 求值。
- 链式 `"hi"[0][0]` 也合法(apply_postfix_loop 复用,允许多轮)。
- `INDEX_SET` 不接受字符串 receiver(spec §4.5):parser 端允许
  `"hi"[0] = "x"` parse 通过,eval 端 INDEX_SET 抛 E0030。
  parser/eval 两段分离行为由 `parser_string_literal_subscript_set_sugar_rejected_at_eval`
  + `eval_string_literal_subscript_set_rejected_with_e0030` 两件锁测试覆盖。
- 越界 `"hi"[5]` 现在到达 eval 端,触发 INDEX_GET 的 E0036
  (原 v0.8.0 是 parse 阶段 E0011)。
- 整数字面量 / 浮点 / Boolean / Null 字面量 postfix 仍被 parser 拒绝
  (走 `parse_call_or_ident` / 主 expr 路径,无 INDEX_GET 语义),回归锁测试
  `parser_integer_literal_postfix_still_rejected` 守住不变式。
- 既存测试不受影响:既有的 `"hi"[0]` 等都是经 `parse_call_or_ident`
  的 ident → 后续 `[0]` postfix 路径(已工作);新增的是 StringLit 直接
  作为 primary 的情形。

### 回归锁测试
**新增 5 件 parser round-trip**(位于
`impl/crates/wlwl-parser/tests/spec_v3_alignment.rs`,D8-004 字面下标段
之后):
- `parser_string_literal_subscript_top_level`:顶层 `"hi"[0]` →
  `INDEX_GET(Literal("hi"), 0)`
- `parser_string_literal_subscript_in_let_slot`:audit §4.4 reproducer
  `LET(_x, "hi"[1])` parse 通过
- `parser_string_literal_subscript_chained`:链式 `"hi"[0][0]` 两轮
  INDEX_GET
- `parser_integer_literal_postfix_still_rejected`:`3[0]` parse 拒绝
  (守住"非 StringLit 字面量 postfix 不启用")
- `parser_string_literal_subscript_set_sugar_rejected_at_eval`:
  `"hi"[0] = "x"` parse 通过(eval 端拒 INDEX_SET)

**新增 4 件 eval end-to-end**(位于
`impl/crates/wlwl-eval/src/lib.rs::tests`,与 D8-009 / D8-010 / D8-011
锁测试集并列):
- `eval_string_literal_subscript_h`:顶层 `"hi"[0]` → `"h"`
- `eval_string_literal_subscript_in_let_binding`:LET(_x, "hi"[1]) 全链路
- `eval_string_literal_subscript_out_of_range_returns_e0036`:`"hi"[5]`
  越界抛 E0036(原 v0.8.0 parse E0011)
- `eval_string_literal_subscript_set_rejected_with_e0030`:`"hi"[0] = "x"`
  eval 端抛 E0030(spec §4.5)

`cargo test --workspace`: ~1397 项全绿(原 ~1388 + parser 5 + eval 4);
wlwl-spec-v0.8.md 不动;eval 761 → 765;parser 84 → 89;D8-004 锁测试
(8 件)不受影响。

---

## D8-013 · examples/closure_cell.wll 与 spec §3.3 对齐(纯 docs 修复)

- **状态**:已修复(commit 待补;v0.8.1 第五轮,**纯 example 修复,无 impl 改动**)
- **发现**:v0.8.1 F-E 由 `docs/AUDIT_REPORT.md §3.1`(2026-09-23 独立第三方测评)+ `docs/plan/wlwl-build-plan-v0.8.1.md §1.5` 驱动
- **影响范围**:
  - `impl/examples/closure_cell.wll`(改写:`LET(n, 0)` + shadow → `LET MUT(n, 0)` + `SET`)
  - `impl/tests/conformance/closure_cell.wll`(同步改写)
  - `impl/crates/wlwl-eval/tests/fixtures/v07_fidelity_v06_baseline.jsonl`
    第 1 行 baseline stdout 由 `"NULL NULL NULL\n"` → `"1 2 3\n"`
  - `impl/crates/wlwl-eval/src/lib.rs::tests` 新增 2 件 end-to-end(原 765 → 767)
  - `wlwl-spec-v0.8.md` 不动(用户约束)

### 现象
第三方审计 §3.1 reproducer:

```wlwl
// examples/closure_cell.wll (旧)
LET(n, 0);
LET(step, FUN((), LET(n, +(n, 1))));
LET(c, step()); LET(c, step()); LET(c, step());
PRINT(c);   // 注释期望 3
```

实测 v0.8.0:

```
$ wlwl.exe run examples/closure_cell.wll
NULL
```

spec `docs/standard/wlwl-spec-v0.8.md:238-254` §3.3:

> "单元格可变标志由绑定形式决定:`LET` 建立的为不可变,`LET MUT` 建立的为可变。
> **该标志一经设定不再改变**。"

按 §3.3 字面读法:`LET(n, 0)` 不可变,FUN 体 `LET(n, +(n, 1))` 是 shadow 新 binding
(§3.2),旧 n 永远不变;FUN 返回 §3.1 "LET 表达式本身的值是 NULL",三次
调用 c 都是 NULL。

> v0.6 CHANGELOG §G:
> "`LET MUT`: mutable bindings must be declared `LET MUT(name, value)`."
> 已知 limits 段:
> "Captured-`LET` upgrade (legacy E-CloCap) still diverges from a strict
> reading of §3.3 — prefer `LET MUT` for shared mutation."

`README.md` (line 13-19) 用的是正确形式:
```wlwl
LET MUT(counter, 0);
LET(step, FUN((), (
    SET(counter, +(counter, 1));
    counter
)));
PRINT(step());   // 1
PRINT(step());   // 2
```

`examples/closure_cell.wll` 与 README 打架,注释期望 3、实测 NULL —
**文档/示例与 §3.3 + impl 同时脱节**。

### 根本原因
v0.4-v0.5 时代 `closure_cell.wll` 写成 `LET(n, 0)` shadow 形式,当时
impl 的 "captured-`LET` upgrade" 把外部 n 升级为 mutable(legacy
E-CloCap),输出 1/2/3。v0.6 §G 把 LET MUT 强制化,upgrade 路径删除,
impl 现在严格按 §3.3 走,但**示例文件没同步改写**,注释仍期望 3。

### 处置
**纯 docs 修复**,无 impl / parser / eval 改动:

1. **`impl/examples/closure_cell.wll`**(改写,15 行):
   - `LET(n, 0)` → `LET MUT(n, 0)`
   - FUN 体从 `LET(n, +(n, 1))` (shadow 形式,NULL) 改成
     `(SET(n, +(n, 1)); n)` (block 形式,返回 SET 后的 n)
   - 顶部 doc 注释加引 `spec §3.3` + §3.4,说清"§3.3 永久不变,共享
     mutation 必须用 `LET MUT` + `SET`,旧 shadow 形式返 NULL"
   - 与 README.md `Quick example` 块对齐

2. **`impl/tests/conformance/closure_cell.wll`**(同步改写):
   - spec §16.5 mandatory test 7 文件同步换成 LET MUT + SET 版本
   - 顶部注释引用 §3.3 / §3.4 + examples/closure_cell.wll 链接

3. **`impl/crates/wlwl-eval/tests/fixtures/v07_fidelity_v06_baseline.jsonl`**
   baseline 同步更新:
   - 第 1 行 `closure_cell.wll` 由 `"NULL NULL NULL\n"` → `"1 2 3\n"`
   - v07_fidelity 测试目的是"v0.6 → v0.7+ 行为稳定";本 baseline 的旧值
     是已知的错误(v0.6 captured-LET upgrade 副产品),与 v0.8.1 修复对齐
     后变正确值。
   - **零覆盖损失**:baseline 仍然抓 v0.7+ 相对 v0.6 的固定点;
     `closure_cell.wll` 的旧"NULL" 是错误基线,新"1 2 3" 是正确基线。

### 兼容性影响
**零行为扩展**:
- 这是 docs-only 修复,**没有任何用户程序因本轮而改变行为**。
- 任何依赖"examples/closure_cell.wll 输出 NULL"的用户都没有 — 该文件
  本身就有"3"的注释期望,实际 NULL 与文档冲突,无人会同时信赖两者。
- `v07_fidelity_v06_baseline.jsonl` baseline 更新是 v0.7 周期内隐藏的
  数据修正,与 v0.6 → v0.7 的 9 项 breaking changes 不在同一抽象层,
  不算 v0.8.1 的兼容性破坏。

### 回归锁测试
**新增 2 件 eval end-to-end**(位于
`impl/crates/wlwl-eval/src/lib.rs::tests`,与 D8-009 / D8-010 / D8-011
/ D8-012 锁测试集并列,§2.9 consistency tests):
- `eval_closure_cell_mut_three_invocations_yield_three`:
  `LET MUT(n, 0); LET(step, FUN((), (SET(n, +(n, 1)); n)));`
  三次 step() 后 c = 3,锁定 example 改写后的正确行为。
- `eval_closure_cell_immutable_let_shadows_returns_null`:
  守住"§3.3 strict 不变式":用普通 LET(而非 LET MUT)+ 闭包内 LET shadow
  的形式,FUN 体返回 NULL(§3.1)而不是累加值。**这条锁测试是反向 regression
  guard**:任何人未来如果想"恢复 legacy captured-LET upgrade"以让旧 example
  输出 1/2/3,这条测试会立即失败,提醒"§3.3 是 normative,不能再次破坏"。

**v07_fidelity 更新**:baseline 第 1 行已同步为 `"1 2 3\n"`,`v07_fidelity_matches_v06_baseline`
测试由 fail 状态恢复 pass(因为旧 baseline 锁的是错误行为);其余 10 件
fixture (core_subsets / destruct / error_schema / numeric / match_patterns /
module_paths / format_template / err_propagation / index_bounds / interpolation 等)
baseline 不动。

`cargo test --workspace`: ~1400 项全绿(原 ~1397 + eval 2 + 1 项
v07_fidelity 从 fail 恢复 pass);wlwl-spec-v0.8.md 不动;eval 765 → 767;
v07_fidelity 0 pass → 1 pass。

---

## D8-014 · §1.8 / §A.2 `${...("...")}` 内嵌字符串字面量不允许(spec 灰色地带,留 v0.9)

- **状态**:待修复(spec 留 v0.9,**v0.8.1 不实施,纯备忘登记**)
- **发现**:v0.8.1 F-F 由 `docs/AUDIT_REPORT.md §1.3`(2026-09-23 独立第三方测评)+ `docs/plan/wlwl-build-plan-v0.8.1.md §1.6` 驱动
- **影响范围**:
  - `wlwl-spec-v0.8.md` 不动(用户约束:v0.8.x 严格遵循 v0.8 spec)
  - `impl/crates/wlwl-lexer/src/lib.rs::read_interp_body` line 633-643
    (现状:报 E0001 "nested string literal inside `${...}` interpolation is not allowed")
  - 不引入代码改动,只在 deviations 留备忘

### 现象
第三方审计 §1.3:

> "spec §1.8 与 §A.2 grammar **均未显式禁止**内嵌 string literal,只是 §A.2
> 末尾注 "为避免歧义,实现通常在词法阶段以括号配对定位插值边界"。
> 实际影响面非常广。任何 `${FUN_CALLED("literal arg")}`、`${ARR["key"]}`、
> `${ARR[0] = "x"}` 等长度在一行的内插表达式都不可写。"

实测:

```
$ wlwl.exe run -e 'PRINT("hi ${greet("alice")}")'
error[E0001]: nested string literal inside `${...}` interpolation is not allowed
```

但同位置 `${greet("alice")}` 的**非字符串内嵌**用法通过(impl 走 brace 配对,
遇到 `"` 才拒)。

### spec 引用 (`docs/standard/wlwl-spec-v0.8.md`)

§1.8 (line 121-132):
> "字符串插值:`"Hello, ${name}!"` 中 `${...}` 内的表达式按完整表达式
> 语法求值;求值结果以 `STR`(10.3) 渲染后嵌入字符串;求值若产生
> `ERR` 则按 8.2 透传,整个字符串字面量表达式的结果为该 `ERR`。"

§A.2 grammar line 1049:
> ```
> interpolation = "${" Expression "}" .
> ```

**§1.8 / §A.2 字面允许 `Expression` 在 `${...}` 内**。Expression 含 Literal
→ Literal 含 string_lit → 语法上允许 `${"hello"}` 这种嵌套字符串字面量。

但 §A.2 末尾 line 1062:
> "`string_lit` 中的 `Expression` 是完整的 `Expression` 文法,但**为避免歧义**,
> **实现通常在词法阶段以括号配对定位插值边界**。"

"为避免歧义"是 implementation hint,**不是 normative 禁止**;impl 选择 brace
配对是合理的实现简化,但 spec 没要求"必须拒绝内嵌字符串"。

### 现状 (`impl/crates/wlwl-lexer/src/lib.rs`)

`read_interp_body` (line 593-647) 用 brace-counting 扫描 `${...}` 体内:

```rust
b'"' => {
    // Nested string literals are not allowed inside `${...}` per spec
    // grammar — they would break the simple brace-counting strategy.
    // Report E0001.
    return Err(self.err(
        ErrorCode::E0001,
        "nested string literal inside `${...}` interpolation is not allowed",
        self.line, self.col,
    ));
}
```

第一次遇到 `"` 直接返 E0001,不去 parse 嵌套字符串。这是 brace-counting 简化
策略的产物,**不是** spec 显式要求。

### 灰色地带根因

| 立场 | 论点 |
|------|------|
| **审计意见**(认为这是 bug) | spec grammar §A.2 允许 `${Expression}`,Expression 含 string_lit,所以 `${"hi"}` 应该 parse 通过 |
| **impl 立场**(认为这是 by design) | brace-counting 简化策略决定拒绝嵌套,行为稳定;spec 没禁止即允许但工程量大 |
| **spec 立场**(实操模糊) | §1.8 / §A.2 字面未禁止;§A.2 末尾 implementation hint 暗示 brace 配对是合理的;无 normative "不允许" 文本 |

### 处置
**v0.8.1 不实施**,仅登记 deviations:

修复成本(估计):
- 重写 `read_interp_body` 用 recursive lex,引入 string-literal scope 状态
- 新增嵌套深度匹配锁测试(`${"hello"}` / `${a + "b" + c}` 等)
- 锁定 `(` `[` `{` 在字符串内的对称性,避免误吞

收益:开启 `${FUN_CALLED("arg")}` / `${ARR["key"]}` 等行的内插表达式 — 
**但**这些用法本身已经能用 LET 提前存值绕过(`LET(x, FUN_CALLED("arg")); PRINT("hi ${x}")`),所以是表达力提升而非必须。

**留 v0.9 spec 增补 normative note**:在 §1.8 加一句:
> "**字符串插值体内不允许再写字符串字面量**(`${"..."}` 不合法),以简化词法
> 边界定位。需要内嵌字符串字面量时,先用 `LET` 存值再插值:
> ```
> LET(s, "literal arg"); PRINT("hi ${FUN(s)}")
> ```"
>
> 这样规范明确禁止,impl 与 spec 完全对齐,不依赖 "implementation hint"。

### 兼容性影响
**零**:v0.8.1 不实施任何代码改动。

**v0.9 实施后**:
- 零行为扩展:既有的 `${name}` / `${expr}` 用法不变。
- 范围扩展:`${"literal"}` 从 E0001 升格为...取决于 spec 决议:
  - 选项 A(放开):允许 `${"literal"}` parse,${"hi ${name}"}` 这种嵌套插值仍
    按 spec §1.8 字符串 escape 规则处理 — 但需要解决 `${` 嵌套识别。
  - 选项 B(明确禁止):在 §1.8 加 normative note,**维持 E0001 现状**,但
    spec 与 impl 完全对齐,无需 "implementation hint" 兜底。
- 推荐选项 B:零 impl 改动 + spec 文档完备 + 用户对照规范时不会再次误以为
  这是 bug。工程量最低。

### 回归锁测试
**无** (v0.8.1 不实施)。

**v0.9 实施时**(选项 B 路径,推荐):
- `lexer/tests/lex_interpolation_unterminated_brace` (line 1090-1099 既有)
  现有测试 `lex("\"hi ${name\"", "t.wll")` 已经验证 nested-string 被拒;
  v0.9 实施后只需在 doc 注释里把 "Nested string literals are not allowed
  inside ${...} per spec grammar" 改成 "spec §1.8 normative",从
  implementation hint 升级为 spec 要求。
- 新增 1 件 doc-test 风格 lock:确认 spec §1.8 文本含 normative note
  (grep-style doc test,验证字符串子串"不允许再写字符串字面量"在 spec
  文件第 X-Y 行)。

**v0.9 实施时**(选项 A 路径,如果选择放开):
- 重写 `read_interp_body` 加 string-literal scope 状态机
- 新增 4-5 件 lexer 测试覆盖 `${"hello"}` / `${a + "b" + c}` / `${if(c, "yes", "no")}`
  等合法形式 + `${"a"b"c"}` 等畸形(应拒)
- 工程量:1-2 天 + 锁测试。

---

## 模板(后续登记用)

```
## D8-NNN · <简短标题>

- **状态**:待修复 / 已修复(commit XXXXXXX)
- **发现**:v0.8 §X.Y 实施时(YYYY-MM-DD)
- **影响范围**:具体文件 + 行号

### 现象
(贴代码 / 测试输出)

### 根本原因
(为什么会出现)

### 处置
(改了哪些字段)

### 兼容性影响
(用户代码 / dispatch 行为是否改变)

### 回归锁测试
(新增 / 既有测试名)
```

```
## D8-NNN · <简短标题>

- **状态**:待修复 / 已修复(commit XXXXXXX)
- **发现**:v0.8 §X.Y 实施时(YYYY-MM-DD)
- **影响范围**:具体文件 + 行号

### 现象
(贴代码 / 测试输出)

### 根本原因
(为什么会出现)

### 处置
(改了哪些字段)

### 兼容性影响
(用户代码 / dispatch 行为是否改变)

### 回归锁测试
(新增 / 既有测试名)
```