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