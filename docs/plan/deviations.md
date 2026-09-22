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