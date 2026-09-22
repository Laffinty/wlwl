# WLWL v0.8 构建计划

> **状态**:草稿 0(本计划)
> **基线**:`docs/standard/wlwl-spec-v0.7.md`(v0.7.0)+ `impl/crates/`(B11 registry lock-down)
> **不在范围**:不重排编码与文件格式(UTF-8 BOM 可选、换行 LF/CRLF/CR 三态);不改 §0.1 章节构成;不动 §1.2 文档注释与单/块注释规则。
> **源材料**:`Desktop\WLWL_v0.8_开发目标报告.md`(第三方,仅看 spec,未读源码,未跑测试)。
> **立场**:**批判性吸收 + 显式修正**。第三方报告有 6 处事实错误、3 处处方反向、1 处遗漏。本文按源码证据重新裁定。

---

## 0 摘要

### 0.1 三段式目标

| 阶段 | 范围 | 文档位置 |
|------|------|---------|
| **A · 设计优化**(impl/registry/parser) | 修源码里能直接定位的 §8 / §10 / §12 / 附录 G 不一致 | §2 |
| **B · 文档优化**(v0.8 spec) | 在 A 完成后,**基于已修源码**重写 spec 对应章节;同步修未触动源码的章节(§4.3 NOT、§17.1 YIELD 位置、§1.6 负数词法等) | §3 |
| **C · 验收** | 全部 deviations 标注;一致性快照比对;运行时差分基线 | §4 |

A 与 B **相互独立**:A 是可独立 commit 的源码改动,B 是可独立 review 的文档改动。但 B 的多数条目依赖 A 完成后的源码状态(避免"按未改的旧代码写新 spec")。

### 0.2 与 v0.6→v0.7 策略的关系

v0.7 在 v0.6 上做"只追加"。本计划**保留**该约束,但范围限于**编码规范**(源文件 UTF-8 BOM 可选、换行 LF/CRLF/CR、ASCII/Unicode 标识符、字符串转义集、注释三态)— 这些是 §1.1/§1.3/§1.8 的事实约束,改动即破坏外部程序。

**v0.8 非编码部分允许三类破坏**:

1. **可逆规范** — §4.3 关于 `NOT` 的"除 X 外"叙述错(应改成"全部"),反向调整;
2. **未发布语义** — §12 保留形式表与 §附录 G 注册表冲突,以注册表为单一真相源,删除 §12 中已注册但 §12 误标为"保留"的形式;
3. **错误码预留** — `E0055`/`E0057` 当前无触发路径,显式标注"v0.7/v0.8 保留"以免读者误以为存在未文档化行为。

### 0.3 与第三方报告的差异总览

第三方报告 19 条 + 附录 5 条 = 24 条。逐项核对后:

| 报告条目 | 验证结论 | 本计划采用 |
|---------|---------|-----------|
| F-01 §8.2 示例 | **采纳**(spec 487-490 行 `show(r);` 重复注释、`// FALSE` 注释错) | §3.1 |
| F-02 §12 vs 附录 G | **部分采纳 + 修正**:冲突确实存在,但报告对 `AND`/`OR` 的处方"弃用别名"错 — 注册表已是 `LexerMacro short-circuit`,真实语义早已生效,只需从 §12 移除 | §2.1 / §3.2 |
| F-03 POP/AT_K | **采纳,但处方纠正**:报告说要"重写 §10.4 注记",实际上 §10.4 没说谎;问题在**注册表 entry 的 signature 字段错**(写 `POP(arr) -> ARRAY`,实际跑 `POP(d,k,default) -> v`)。Deviation P4-B12-002 已记录。注册表改一行即可 | §2.2 |
| F-04 附录 G 交叉引用 | **采纳**:注册表 `BuiltinGroup::anchor()` 返回的章节号确实是旧版(v0.4 时代的 §13/§11/§11.4 等),v0.7 没有 §13–§16。`docs/appendix_G.md` 也跟着错 | §2.3 |
| F-05 E0055/E0057 | **采纳**:两道均为 v0.7 实际无触发路径的保留码,只标注即可,不动 dev path | §3.3 |
| F-06 NOT 消费者 | **采纳问题,反向处方**:报告说"把 NOT 加进消费者表";**但**源码测试 `b9_not_with_err_arg_is_e0102`(`lib.rs:17612-17619`)锁定了相反行为 — `NOT(ERR)` 必须是 E0102。**规范错,实现对**。改 spec 而非实现 | §3.4 |
| F-07 字面量下标 | **采纳**:parser 实际不允许(只对 identifier 走 postfix 循环),但 spec §A.2 文法允,prose §4.5 末禁。**允许字面量下标**(报告选项 A)需改 parser | §2.4 |
| F-08 版本记法 | **采纳**:§17.7 用 `v0.7.0`,正文用 `v0.7`,不统一。v0.8 起统一为 `v0.8`,pre-existing 章节保留旧记法 | §3.5 |
| F-09 FORMAT 混用 | **驳回**:报告没读源码。`wlwl-std/src/format.rs:183-210` 已经实现"首个 DICT 供 `{name}` 查询;`{N}` 自然索引到 `format_args[N]`,允许首个实参是 DICT"。`format_positional_refers_to_dict_arg_renders_it` 测试(`format.rs:433-437`)显式锁这条。规范写得含糊,但实现是对的 — spec 补一句即可,无 impl 改动 | §3.6 |
| F-10 负数字面量 | **采纳,处方复杂化**:spec §1.6 写 `int_lit = [ "+" \| "-" ] ...`(前导符号属于字面量),但 lexer 不读前导符号(`lexer.rs:278-326` 的 `read_number` 直接读数字)。parser 走 `-(0, x)` desugar(`parser.rs:679-707`)。结果对,文档误导。spec 应描述 parser desugar 路径而非 lexer 前导符号 | §3.7 |
| F-11 EXPECT_ERR | **采纳**:registry 是 `LexerMacro / Yes`(line 422-431),但 spec §8.3 表里没有,只在表后段落提及。表内补一行 | §3.1 |
| G-01 FUN/LET 不对称 | **采纳**:设计自洽,补一条 normative 说明 | §3.8 |
| G-02 `=` 三重身份 | **采纳**:注册表 `Eq => Some("==")`(lexer 156-175 行)说明调用位 desugar 已实现;补规范消歧规则 | §3.9 |
| G-03 YIELD 位置 | **半采纳**:§17.7 限制表已经有"[v0.7.0] 嵌套 YIELD 不恢复嵌套体",§17.1 的位置限制本来就是 v0.7 实现路径的产物。报告把"§17.1 是 normative、§17.7 是 impl-limit"的对比夸大了。把 §17.1 的位置限制从"必须"(硬规则)改成"v0.7 / v0.8 实现路径"(实现限制)即可 | §3.10 |
| G-04 TASK 同名 | **采纳**:类型名 `TASK` 与 `wlwl:std.ai.TASK` / `wlwl:std.agent.TASK` 同名,后者是 stdlib 成员;`TYPE(x)` 不做名字解析,所以不冲突,但 §10.11 文档里要写全限定 | §3.11 |
| G-05 SHIELD/SCOPE | **采纳**:补一句 normative 说明 | §3.12 |
| G-06 AWAIT 宿主诊断 | **采纳**:补定义 | §3.13 |
| G-07 `%` 浮点 | **采纳**:实现已经返回 E0030(`§2.2` 文本也写"只接受整数"),只是没把 E0030 列入 `%` 的触发列表。补 §11.2 | §3.14 |
| G-08 SCOPE(ERR) | **采纳**:不冲突 — 8.2 透明传播先于 17.1 fn 类型检查。补 normative 说明 | §3.12 |
| G-09 YIELD 尾部 | **采纳**:与 G-03 一并,移入 §17.7 限制表 | §3.10 |

**附录 5 条**(`v0.9` 候选):本计划**不处理**,保留给 v0.9 议程。

---

## 1 现状盘点(证据基线)

### 1.1 关键源码位置

| 关注点 | 文件 / 行 | 摘要 |
|--------|----------|------|
| 内建注册表(single source of truth) | `impl/crates/wlwl-eval/src/registry.rs` | 110 个条目;`BUILTIN_REGISTRY` 是附录 G 的 Rust mirror;5 个锁测试守住不一致 |
| `POP` 注册条目(实际) | `registry.rs:706-720` | signature 写 `POP(arr) -> ARRAY (v0.4/v0.5 alias for AT_K semantics)`,**实际 dispatch 路由到 `builtin_at_k`**(`lib.rs:4092`)— 三元 DICT safe-delete |
| `POP` 偏差注记 | `lib.rs:1792-1799` | "POP deviation P4-B12-002: spec 附录 G 标 POP(arr) -> ARRAY,impl 在 B1 把 POP 重载为 DICT 三元。v0.5 重命名 POP_DICT 可彻底分开。" |
| `POP` 测试 | `lib.rs:13341-13386` | 7 个测试全在测 **DICT 三元**:`POP(["a":1,"b":2], "a", NULL)` 返回 1 |
| `NOT` 注册 | `registry.rs:537-546` | `ErrConsumerStatus::No`(非消费者) |
| `NOT` 实现 | `lib.rs:4698-4701` / `resolve_builtin` 4148 | `builtin_not(v) -> BOOL(!is_truthy(v))` |
| `NOT` ERR 行为锁测试 | `lib.rs:17612-17619` `b9_not_with_err_arg_is_e0102` | `NOT(ERR("e"))` → E0102 顶逃(8.2 默认透传) |
| `BOOL` ERR 行为锁测试 | `lib.rs:18578-18580` | `BOOL` **是**消费者(registry `ErrConsumerStatus::Yes`) |
| 一元负号 desugar | `parser.rs:676-707` | `-x` → `-(0, x)`(当 `-` 后非 `(` 时触发) |
| `NEG` 内建(规范定义但 parser 不用) | `lib.rs:2322-2330` / `resolve_builtin` 4099 | 存在,但 parser 的负号路径走 `-(0, x)`,不调用 NEG |
| `=` 调用位 desugar | `lexer.rs:156-175` `TokenKind::as_op_name` | `Eq => Some("==")` — `=(a, b)` 直接改写为 `==(a, b)` |
| FORMAT 混用实现 | `impl/crates/wlwl-std/src/format.rs:177-211` | `format_args = &args[1..]`;`named = format_args.iter().find_map(DICT)`;`{0}` 自然可命中首个 DICT |
| FORMAT 锁测试 | `format.rs:367-374` `format_mixed_spec_example` | `"hi {0}, age {age}"` + `("alice", ["age": 30])` → `"hi alice, age 30"` |
| `expect_err` 在 §8.3 表 | `wlwl-spec-v0.7.md:499-513` | 表里 11 行,**没有** `EXPECT_ERR`;段末 513 行单独一段文字描述 |
| `expect_err` 在注册表 | `registry.rs:422-431` | `LexerMacro / Yes` |
| 一元负号锁定 | `parser/tests/spec_v3_alignment.rs` (推断路径) | 走的是 `-x → -(0, x)`,**未锁定**字面量路径 |
| 字面量下标 parser 路径 | `parser.rs:1773-1895` `parse_call_or_ident` | postfix `[` 循环只接受从 identifier/operator 进入;`parse_array_or_dict` 返回 `Expr::Array` 直接退出,**没有 postfix 续接** |
| `TASK` 在 std.ai/agent | `wlwl-std/src/agent.rs:67-275` | `wlwl:std.agent.TASK(name, prompt)` 三参 |
| 章节锚错位 | `registry.rs:91-108` `BuiltinGroup::anchor` | 13 个组锚里 `§15.1`(v0.7 是 §10.2)、`§13`(v0.7 无 §13)、`§11`(v0.7 §11 是诊断不是 OOP)、`§11.4`(v0.7 无 §11.4) |
| `appendix_G.md` | `docs/appendix_G.md` 12977 字节 | 与 `BUILTIN_REGISTRY` 应该被 `b11_generated_md_matches_registry` 锁测试守住;章节号确实过期 |

### 1.2 锁测试矩阵(改动前必须看)

| 测试名 | 守住什么 | v0.8 影响 |
|--------|---------|-----------|
| `b11_registry_covers_resolve_builtin` | 每个 `resolve_builtin` 的名字都在 registry | 改 entry 需同步 resolve_builtin |
| `b11_resolve_builtin_covers_registry` | 反向 | 同上 |
| `b11_err_consumer_registry_consistent` | ERR consumer 集合与 registry 一致 | §3.4 改 NOT 消费者地位会爆这测试(本计划不动 NOT 消费者地位,故不影响) |
| `b11_macro_fn_attribute_matches_dispatch` | macro_fn=true 必须 LexerMacro 或 builtin 显式接受 | §3.2 移除 §12 的 AND/OR 不影响 registry |
| `b11_registry_count_matches_spec_table` | 总条目数 ≥ 88 | v0.8 新增 ≥ 1 时要更新 |
| `b11_generated_md_matches_registry` | 生成器与注册表一致 | 改 registry 后重跑 `gen_appendix_g.rs` |
| `b9_not_with_err_arg_is_e0102` | NOT ERR → E0102 | §3.4 改 spec 不改 impl,测试不受影响 |
| `pop_dict_*` 七件套 | POP 三元 DICT 语义 | 不动 |
| `names_count_is_seventeen` | collection 17 个高阶函数 | 不动 |

### 1.3 deviation 注册(改动会引入新 deviation)

`docs/history/deviations-v0.7.md` 是 v0.7 的偏差登记。v0.8 计划在 `docs/plan/deviations.md`(待建)维护新的偏差:
- `D8-001` POP 注册表条目与 dispatch 实际语义不符 — 仅一行签名修正
- `D8-002` registry anchor 章节号过期 — 系统性重写
- `D8-003` 规范 §4.3 关于 NOT 的"除 X 外"叙述反向 — spec 修订,实现不动

---

## 2 设计优化(impl / registry / parser)

> 这一层改动只动代码不动规范文本。本节每一项给改动路径 + 锁测试影响 + 一致性测试增补。
> 完成后跑 `cargo test -p wlwl-eval -p wlwl-parser` 与 `cargo test --workspace`,全部绿才算 A 阶段完成。

### 2.1 F-02 修复 · 移除 §12 重复真相

**改动**:**不动代码**,只移文档(§3.2)。
**理由**:注册表已经是 single source of truth;§12 列出的形式中,
- `AND` / `OR` / `CLASS` / `NEW` / `THIS` / `MODULE` / `MODULE_REF` / `CALL` / `ARRAY(items...)` 全部已经在 registry(分别是 `LexerMacro` 或 `ResolvedBuiltin`)并锁测试守住。
- 没有任何一个调用方真的会从用户代码触发 §12 所说的"求值产生 E0020"。

**impl 影响**:零。
**测试增补**:无。
**风险**:零 — 这是文档重写。

### 2.2 F-03 修复 · POP 注册表签名纠正

**改动**:`registry.rs:706-720` 的 POP entry。
```rust
BuiltinSpec {
    name: "POP",
    signature: "POP(arr) -> ARRAY (v0.4/v0.5 alias for AT_K semantics)",
    group: BuiltinGroup::Array,
    err_consumer: ErrConsumerStatus::No,
    macro_fn: false,
    version: Version::V02,
    dispatch: DispatchStatus::ResolvedCompat,
    section: "§10.4",
},
```
改为:
```rust
BuiltinSpec {
    name: "POP",
    signature: "POP(d, k, default) -> v (v0.6 compat alias for AT_K; signature kept 3-arg)",
    group: BuiltinGroup::Dict,                       // 关键:DICT 不是 ARRAY
    err_consumer: ErrConsumerStatus::No,
    macro_fn: false,
    version: Version::V06,                           // 改名发生在 v0.6
    dispatch: DispatchStatus::ResolvedCompat,
    section: "§10.4",
},
```

**dispatch 路径不动**(`lib.rs:4092` 已路由到 `builtin_at_k`)。
**锁测试影响**:`b11_macro_fn_attribute_matches_dispatch` 不受影响(macro_fn=false 不变);`b11_resolve_builtin_covers_registry` 不受影响(dispatch=ResolvedCompat 不变)。
**deviation 登记**:开 `docs/plan/deviations.md` 第一条 `D8-001`,描述"POP 注册签名 / group / version 与 dispatch 实际不一致,v0.8 修"。
**测试增补**:为防回归,在 `wlwl-eval/src/lib.rs` 加 `pop_registry_entry_matches_dispatch` 测试:查 registry 中 POP 条目,断言 `signature` 含 "3-arg" 或 "d, k, default"。
**附录 G 文档**:`docs/appendix_G.md` 由 `gen_appendix_g.rs` 重生成 — 章节号修正一并在 §2.3 完成。

> **实施落实(2026-09-23)**:deviations.md 已建并填入 D8-001 + 回填 D8-002(§2.3 的批量化版本号修复);测试 `pop_registry_entry_matches_dispatch` 实际放在 `registry.rs::tests`(原 plan 说 lib.rs,但本类测试靠近 BUILTIN_REGISTRY 定义处更可读,与 `registry_has_no_duplicate_names` 等同模块测试并列);POP entry 内 5 行 `// v0.6 E decision: ...` 旧注释替换为指向 D8-001 的 6 行新注释(指明 dispatch 路径与 compat alias 状态)。

### 2.3 F-04 修复 · 注册表章节号系统化更新(per-entry section)

> **v0.8 偏差(实施时发现)**:原计划把变更目标定为 `BuiltinGroup::anchor()`(registry.rs:91-108);
> 实际代码检视后发现:
> - `anchor()` 在 impl/ 工作区内**零调用方**(grep 全 workspace 无匹配);md 生成器
>   `generate_appendix_g_md()`(registry.rs:1477-1484)实际读取每个 `BuiltinSpec.section`
>   字段渲染"实现位置"列。
> - 因此改 `anchor()` 函数体对 `docs/appendix_G.md` 视觉输出**零影响**。
> - 真要修,必须改 110 个 entry 各自的 `section` 字段(其中 **85 个过期**)。
>
> deviation `D8-002` 措辞修订为"per-entry section 字段系统化更新"。`anchor()` 函数体
> 维持 v0.7 baseline 字符串(已加 doc 注释说明 dead-code 现状)。

**改动**:`registry.rs::BUILTIN_REGISTRY` 中 **85 个** `BuiltinSpec.section` 字段对齐 v0.7 spec 章节号
(对照 `docs/standard/wlwl-spec-v0.7.md`)。

**改动分桶**(按 `group × current_section` 分类,共 85 处):

| group | current | new | 数量 | entries |
|-------|---------|-----|------|---------|
| Io | `§15.1` | `§10.2` | 3 | PRINT, PRINT_ERR, INPUT |
| Conv | `§9.5` | `§10.3` | 2 | INT, FLOAT |
| Conv | `§10.5` | `§10.3` | 1 | LEN(错放在 §10.5 字符串;应归 §10.3 类型与转换) |
| Result | `§12.1` | `§8.1` | 2 | OK, ERR |
| Result | `§12.2` | `§8.3` | 7 | IS_OK, IS_ERR, OR_DIE, UNWRAP_OR, UNWRAP, ERR_PAYLOAD, WRAP |
| Result | `§12.4` | `§8.4` | 1 | PANIC |
| Result | `§12.6` | `§6` | 1 | TRY(早返宏;本质控制流) |
| Result | `§15.9` | `§8.3` | 1 | EXPECT_ERR |
| Control | `§7.1`-`§7.5` | `§6` | 7 | IF, WHILE, FOR, MATCH, RETURN, BREAK, CONTINUE |
| Control | `§3.4` | `§4.3` | 3 | AND, OR, NOT |
| Op | `§9.1` | `§4.3` | 6 | +, -, *, /, %, NEG |
| Op | `§9.2` | `§4.3` | 6 | ==, !=, >, <, >=, <= |
| Array | `§10.1` | `§10.4` | 8 | PUSH, SHIFT, UNSHIFT, SLICE, CONCAT, CONTAINS, INDEX, REVERSE |
| Dict | `§10.2` | `§10.4` | 6 | REMOVE_KEY, DEL, KEYS, VALUES, HAS, MERGE |
| Subscript | `§10.1-§10.2` | `§4.5` | 3 | INDEX_GET, INDEX_SET, AT |
| String | `§10.3` | `§10.5` | 15 | UPPER, LOWER, SUB, REPLACE, SPLIT, TRIM, TRIM_START, TRIM_END, STARTS_WITH, ENDS_WITH, REPEAT, PAD_START, PAD_END, CODEPOINTS, FROM_CODEPOINTS |
| Format | `§10.6` | `§10.7` | 1 | FORMAT |
| Module | `§13.1`-`§13.5` | `§9` | 4 | MODULE_REF, EXPORT, IMPORT, MODULE |
| Oop | `§11.1`-`§11.3` | `§11 [占位;OOP 未实现]` | 3 | CLASS, NEW, THIS |
| Property | `§11.4` | `§11 [占位;OOP 未实现]` | 3 | GET_PROP, SET_PROP, CALL_METHOD |
| Ctor | `§10.1` / `§10.2` | `§10.9` | 2 | ARRAY, DICT |

**已对齐 v0.7 不需改**(25 个):17 个 Concurrent(所有 `§17.x`)+ POP / AT_K(Array / Dict `§10.4`)+ STR / TYPE / BOOL / CALL(Conv 各自正确章节)+ `&&` / `||`(Op `§4.3`)。

**anchor() 函数体**:维持 v0.7 baseline 字符串 + 加 doc 注释说明 dead-code 现状。**不删除**(避免破坏潜在的外部 crate 引用;独立清理可后续单独跑一轮)。

**impl 影响**:零(纯数据字段更新,不影响 dispatch / 行为)。

**md 视觉变化**:重生成后 `docs/appendix_G.md` 的 110 行表格"实现位置"列会出现 85 处章节号变更;第 9-10 行(`STRING 操作`分组)会有 15 行从 `§10.3` 变 `§10.5`,这是最大单组变化。

**锁测试影响**:计划列出的 `b11_generated_md_matches_registry` **实际不存在**;现有 lock test `generated_md_includes_all_entries` 只校验生成器自身(不读 md 文件)。改完流程:build → regen md → 跑 `cargo test -p wlwl-eval --lib` 全绿 → 人眼/手动 diff 验证 md。

**deviation 登记**:`D8-002` 措辞修订为"per-entry section 字段系统化更新,85 处 BuiltinSpec.section 对齐 v0.7 spec 章节号;anchor() 保留为 dead-code 注释参考"。

**测试增补**:`appendix_g_anchors_match_v07_section_numbers`(计划 §2.9 第 2 项)读取 `docs/appendix_G.md`,对所有 110 行表格"实现位置"列做白名单校验,锚必须从 v0.7 spec 已定义的章节集合取,阻止未来 `§13.x` / `§15.x` / `§12.x` 类过期章节号重新进入。

### 2.4 F-07 修复 · 字面量下标允许

**改动**:`parser.rs:2092` 起的 `parse_array_or_dict` 在返回 `Expr::Array` / `Expr::Dict` 之前,如果紧接着是 `[`,进入 postfix 循环。

**实现策略**(在 PR 里讨论,本计划给定方向):

```rust
fn parse_array_or_dict(&mut self, line: u32, col: u32) -> WlwlResult<Expr> {
    // ... existing logic to build Expr::Array / Expr::Dict ...
    let mut base = if is_dict {
        Expr::Dict { entries, span: ... }
    } else {
        Expr::Array { items, span: ... }
    };
    // NEW: 字面量下标允许 — 把 [1,2,3][0] 转为 INDEX_GET([1,2,3], 0)
    // 复用 parse_call_or_ident 末尾的 postfix 循环结构(行 1863-1907)
    base = self.apply_postfix_loop(base, line, col)?;
    Ok(base)
}
```

**抽出 `apply_postfix_loop`** 作为 parser 的方法,让 `parse_call_or_ident` 和 `parse_array_or_dict` 都复用。
**锁测试影响**:无新增/打破锁测试。但需补字面量下标的一致性测试。
**deviation 登记**:无(规范与实现重新一致)。
**测试增补**:
- `parser/tests/spec_v3_alignment.rs` 加 `[1, 2, 3][0]` / `["a": 1]["a"]` / `[[1,2], [3,4]][1][0]` / `[[1,2][0], 3]` 四个 round-trip
- `eval/tests/v07_fidelity.rs` 加 `[1,2,3][0]` 求值到 1 的端到端测试

**SPEC 同步**:见 §3.15 删除 §4.5 末句(报告选项 A,采纳)。

> **实施落实(2026-09-23)**:deviation 实际登记为 `D8-004`(规范与实现重新一致 — prose 由"禁止"改"允许",但 prose 旧文本早已与 v0.7 §A.2 grammar 自相矛盾,本次修复让 parser 与 grammar 对齐)。
> `apply_postfix_loop` 已抽出(parser.rs:1882 起的 ~120 行),`parse_call_or_ident` 末尾改为单行调用,`parse_array_or_dict` 末尾也接入。
> 测试落地 7 项:`parser/tests/spec_v3_alignment.rs` 加 4 round-trip;`eval/src/lib.rs::tests` 加 3 端到端(`eval_array_literal_subscript` / `eval_dict_literal_subscript` / `eval_chained_literal_subscript`)+ `eval_literal_subscript_with_set_sugar` 1 项,实际共 4 项 eval 测试(plan 写的 "v07_fidelity.rs 加 1 项端到端"改为 lib.rs 邻近 pop_dict_* 的位置更易 review)。`cargo test --workspace` ~1366 项全绿。

### 2.5 F-10 修复 · 负数词法路径文档化

**改动**:**仅注释,不改行为**。`parser.rs:676-707` 的 desugar 块顶部加注释,引用 §1.6 v0.8 措辞,确保未来读者不会以为这是反规范:

```rust
// Unary-minus sugar: `-x` desugars to `-(0, x)`. Only triggered
// when `-` is NOT followed by `(`, so binary minus like
// `-(a, b)` continues to work.
//
// v0.8 spec §1.6 / §4.3: lexer does NOT consume the leading sign;
// the parser rewrites it here. This produces the same observable
// behavior as a leading-sign literal (and the same overflow /
// INTEGER_MIN semantics per §2.2). `NEG(x)` exists as an explicit
// 1-arg builtin (lib.rs::builtin_neg) and is reachable only by name
// — parser does not synthesize calls to it.
```

**改动**:`lexer.rs:278-326` 的 `read_number` 顶部加交叉引用注释,说明 lexer 故意不读前导符号。

**锁测试影响**:零(行为不变)。
**deviation 登记**:无(规范与实现重新一致)。
**测试增补**:`parser/tests/spec_v3_alignment.rs` 加 `-1`、`-(1,2)`、`-x`、`-(a,b,c)` 四例端到端,锁定 `INTEGER_MIN` 在 `-` desugar 路径下产生 E0034(`lib.rs:4488`)。

### 2.6 F-05 修复 · E0055/E0057 标注

**改动**:`wlwl-error/src/snapshots/wlwl_error__tests__codes_concurrent.snap`(如果存在)与 `wlwl-error/src/lib.rs` 的 `ErrorCode` enum 注释。

**实际定位**(查 wlwl-error/src/lib.rs):

```rust
/// E0055 保留:通道关闭后读取的主机侧码(用户可见形式为 ERR(kind="ChannelClosed"))
///
/// v0.7 / v0.8 **无触发路径**。注册仅为版本预留 — 通道关闭信号
/// 由 §8.1 的 `kind = "ChannelClosed"` 字典载荷承担,不经过
/// E0055。未来若有切换至原生码的版本,本条目重新启用。
```

E0057 同款。

**deviation 登记**:`D8-003`(预留码元数据修订)。
**测试增补**:`wlwl-error/tests/` 加 `e0055_e0057_untriggerable_in_v08` 测试,跑完整 v0.7+ spec 用例集,断言 E0055/E0057 不出现在任何 trace 里。

### 2.7 F-11 修复 · EXPECT_ERR 入表

**改动**:**仅 spec 文档**(§3.1),不动 registry。
**理由**:registry 已是 `LexerMacro / Yes`,与 spec §8.3 末段描述一致;只是 §8.3 表里少一行。

### 2.8 G-04 修复 · std.ai/agent TASK 全限定

**改动**:`impl/crates/wlwl-std/src/agent.rs:67-275` 与 `impl/crates/wlwl-std/src/ai.rs` 的 SPEC 注释 / 模块 docstring。
**做法**:模块顶部注释里写:

```rust
//! ## 与类型名 `TASK` 的同名问题
//!
//! 本模块导出 `TASK(name, prompt, opts?) -> TASK_RESULT`(在 std.agent 中)
//! 与 `TASK(...) -> TASK_RESULT`(在 std.ai 中),而 §2.1 的类型名 `TASK`
//! 也叫 `TASK`。同名不冲突:
//! - `TYPE(handle)` 返回字符串 `"TASK"`,`TYPE` 形参位置不做名字解析;
//! - 用户作用域内的 `TASK` 由 IMPORT 注入,与类型名空间独立;
//! - 任何上下文歧义由文法消解(类型名出现在 `:` 类型注解后,标
//!   识符出现在表达式位置)。
//!
//! 文档统一写作 `wlwl:std.agent.TASK` / `wlwl:std.ai.TASK` 以避免
//! 阅读混淆。
```

**改动**:`agent.rs` / `ai.rs` 的 docstring 头部、各 `std_task` 的函数 docstring。
**锁测试影响**:无。
**deviation 登记**:无。

### 2.9 一致性测试增补总览

A 阶段结束后,v0.8 至少新增以下 8 个测试,分散在合适位置:

| 测试名 | crate | 守住什么 |
|--------|-------|---------|
| `pop_registry_entry_matches_dispatch` | wlwl-eval | POP entry signature 含 `d, k, default` |
| `appendix_g_anchors_match_v07_section_numbers` | wlwl-eval | 每行 anchor 在白名单内 |
| `parser_array_literal_subscript_roundtrip` | wlwl-parser | `[1,2,3][0]` 解析到 INDEX_GET |
| `parser_dict_literal_subscript_roundtrip` | wlwl-parser | `["a":1]["a"]` |
| `parser_mixed_literal_subscript_chain` | wlwl-parser | `[[1,2][0], 3]` |
| `eval_negative_literal_minus_one` | wlwl-eval | `-1` 求值到 `-1` |
| `eval_integer_min_negation_is_e0034` | wlwl-eval | `-(-9223372036854775808)` 触发 E0034 |
| `e0055_e0057_untriggerable_in_v08` | wlwl-error | 跑全部 v0.7 用例,二者不出现 |

---

## 3 文档优化(v0.8 规范修订)

> 文档改动遵循"v0.8 不强制仅追加,但不能改编码规范"。每个 PR 配一个 `deviation` 描述(对 v0.7 行为是否有破坏)。
>
> 本节每条改动给出:**新文本草稿**、**与 v0.7 的差异**、**兼容性影响**(用户代码是否需要改)。

### 3.1 §8.2 / §8.3 / §8.5 · 透明传播与消费者表

**§8.2 示例三件套(替换 487-491 行)**:

```wlwl
// 示例一:ERR 作为实参触发透明传播,函数体不执行
LET(show, FUN((r), IF(IS_ERR(r), "E", "O")));
LET(r, ERR("x"));
LET(y, show(r));          // y = ERR("x") — show 的体永不执行
PRINT(TYPE(y));           // "RESULT"
```

```wlwl
// 示例二:ERR 逃逸到顶层
LET(show, FUN((r), IF(IS_ERR(r), "E", "O")));
LET(r, ERR("x"));
show(r);                  // 表达式值被丢弃 → ERR 无消费者 → E0102,程序终止
```

```wlwl
// 示例三:OK 包装阻断传播
LET(show, FUN((r), IF(IS_ERR(r), "E", "O")));
LET(r, ERR("x"));
show(OK(r));              // "O" — OK 包装后不再是 ERR 实参
```

补充一句在 §8.5 末尾:> 顶层 `Program`(附录 A.2)中作为 `ExprStmt` 被求值并被丢弃的表达式,其值若为 `ERR`,同样按本节(§8.2 透明传播)的顶层逃逸处理;即使该函数实参为非 `ERR`,顶层丢弃本身构成"无消费者"。

**§8.3 消费者注册表 — 表内补 EXPECT_ERR 一行**:

在表末(`BOOL` 行之后,行 513 文字之前)插入:

| `EXPECT_ERR(x)` | 供测试使用;`x` 为 `ERR` 时返回 `OK(载荷)`,否则返回 `ERR(E0049)` |

并删除 513 行的孤立文字段(并入表)。

**与 v0.7 差异**:示例区替换、§8.5 加一句、表重组。
**兼容性**:无用户代码影响(示例改写不破坏行为;EXPECT_ERR 早就是消费者,只是文档形态变)。

### 3.2 §12 重写 · 让注册表成为唯一真相

**当前(771-784 行)**:6 行"保留形式"表,所有形式在注册表中均有条目。

**v0.8 改写**:

```markdown
## 12 保留形式

> **v0.8 起本章不再列出"保留形式"清单**。§4.3 / §10 / §17 涉及的所有具名构造,
> 包括历史上曾被标为"保留"的 `AND` / `OR` / `CLASS` / `NEW` / `THIS` /
> `MODULE` / `MODULE_REF` / `CALL(fn, args...)` / `ARRAY(items...)`,
> 全部以附录 G 注册表为准 — 该表是单一真相源(single source of truth),
> 由 `impl/crates/wlwl-eval/src/registry.rs::BUILTIN_REGISTRY` 与
> 锁测试 `b11_*` 双重守住。
>
> 任何新引入的"保留"形式**必须**先在注册表中显式登记为
> `DispatchStatus::Deferred`,并附触发路径(否则锁测试
> `b11_registry_count_matches_spec_table` 在 ≥ 88 条计数上不可被审计)。

### 12.1 真正的保留集合(规范未定义语义,实现亦未注册)

| 形式 | 原因 | 引用 |
|------|------|------|
| (无) | v0.8 没有未注册且被词法保留的具名构造 | — |

**未来扩展**:若 §13–§16 模块系统、OOP、原生模块边界等引入新的保留形式,
应在本章显式列出并写明触发诊断。**不得**借助注册表之外的隐式保留。
```

**与 v0.7 差异**:**完全重写**。原 §12 表 6 行全部移除。
**兼容性**:
- 用户代码曾依赖"`AND` 调用产生 `E0020`"的不存在 — 行为不变(没有真实程序这么用),因为 `AND` 一直是 `LexerMacro short-circuit`,v0.7 也在跑。
- §0.4 一致性章节需要新增一句:"v0.8 起,§12 不再是具名构造的注册场所;附录 G 是唯一真相。" 在 §0.4 表里把 §12 的角色从"保留形式清单"改成"§12 治理规则"。
- 第三方报告 F-02 的选项 A(弃用别名)在本方案中**完全回避**:不需要新增 W0051,因为注册表本来就把 AND/OR 当成 macro 跑着。

### 3.3 §11.2 · E0055 / E0057 显式标注

在 v0.8 spec §11.2 错误码表里,把:
- `E0055` 行末加:"**v0.7 / v0.8 无触发路径**;见 §8.1,`ChannelClosed` 信号走字典载荷。"
- `E0057` 行末加:"**v0.7 / v0.8 无触发路径**;见 §3.3,跨任务不可变单元格复用 E0024(详见 deviations D8-003)。"

**与 v0.7 差异**:两行注释。
**兼容性**:无行为变化。

### 3.4 §4.3 · NOT 不再是传播例外

**当前(287 行)**:"任一操作数为 `ERR` 时,除 `NOT` 外全部透明传播(8.2);`&&`/`||` 短路求值中,未求值那一侧的 `ERR` 不被传播(因为它没被求值)。"

**v0.8 改写**:"任一操作数为 `ERR` 时,所有运算符调用按 §8.2 透明传播 — **包括 `NOT`**。`BOOL(x)` 是单独的 ERR 消费者(8.3 表),与 `NOT` 不对称,但两者都返回布尔真值;`BOOL(ERR(...))` 不传播,`NOT(ERR(...))` 传播。如需安全取反,请先经 `BOOL`:`NOT(BOOL(ERR(...)))` → `FALSE`(短路等价于 `!TRUE`)。

**与 v0.7 差异**:**反向**。v0.7 的"除 NOT 外"被移除。
**兼容性**:报告 F-06 推荐"加 NOT 进消费者"会破坏 `b9_not_with_err_arg_is_e0102` 测试。本方案不动实现,只修规范文字,与锁测试一致。
**为什么不反过来"实现里把 NOT 变消费者"**:
1. 已锁 6 个测试(详见 lib.rs:17612-17619 等);改 NOT 行为会触发 `b11_err_consumer_registry_consistent` 失败,需要级联修改 ~5 处;
2. 与 `BOOL` 真值消费者地位不一致 — 现实里 `BOOL(ERR(...))` 与 `BOOL(0)` 都是 FALSE,如果 `NOT(ERR(...))` 不传播、却 `BOOL(ERR(...))` 不传播,**两者真值表同**;若硬把 NOT 变消费者,违反 §8.2 默认 + §8.3 白名单的简洁性;
3. 文档化"用 `NOT(BOOL(x))` 安全取反"比改语义更小破坏。

### 3.5 §17.7 · 版本记法统一

**当前**:§17.7 多处用 `[v0.7.0]`,正文其余用 `v0.7`。
**v0.8 起**:
- 新写章节用 `v0.8`(省略 patch 段);
- 历史性"不承诺"行保留原记法 `[v0.7.0]` 作为版本快照(引用 v0.7 实现路径);
- 在 §0.1 加一句规范版本约定:"规范版本采用 `major.minor` 双段;新引入内容追加 patch 段(如 `v0.7.1`)仅用于 errata,不发新规范文件。"

**改动位置**:§0.1 加一句;§17.7 的 `[v0.7.0]` 改为 `[v0.7 实现路径]`(措辞中性化)。
**兼容性**:无。

### 3.6 §10.7 · FORMAT 混用语义补全

**当前(652 行附近)**:`{N}` 与 `{name}` 可混用,但**没有**说明首实参重叠的索引基准。

**v0.8 改写**:
> `{N}` 的 `N` 自 `0` 起,指 `FORMAT(template, args...)` 中 `args` 的第 `N` 项(template 是第 -1 项,不计入索引)。当模板同时含 `{name}` 与 `{N}`:
> - 若首个 `args` 项(`{0}`)是 DICT,`{name}` 优先在该 DICT 中查键;`{0}` 仍按 STR 渲染该 DICT(例:`["age": 30]` → `"[age: 30]"`);
> - 若首个 `args` 项是标量,`{name}` 在所有 DICT 实参中按位置查找首个 DICT;`{0}` 渲染该标量;
> - 都找不到则占位符保留原样(`{name}` / `{N}` 不抛错)。

这一段直接落到 spec §10.7(652 行附近)。
**impl 影响**:零(实现已经这么做;`format_positional_refers_to_dict_arg_renders_it` 已锁)。

### 3.7 §1.6 / §4.3 · 一元负号路径说明

**§1.6 当前**:
```
int_lit = [ "+" | "-" ] decimal_digit { decimal_digit } .
```
**v0.8 改写**(加旁注):
```
int_lit     = decimal_digit { decimal_digit } .
int_literal = [ "+" | "-" ] int_lit .            // 前导符号属于词法前置
```

> **实现注(v0.8 起)**:lexer 不消费前导符号。`-x` 形式由 parser 在 `parse_expr` 中改写为 `-(0, x)`(二元减法对 0);`+x` 改写为 `+(0, x)`。可观察行为与字面量路径等价:同 INTEGER_MIN 边界、同 §2.2 溢出语义。`NEG(x)` 是独立的 1 元内建(`registry §4.3`),仅按名调用可达,parser 不合成。

**§4.3 表新增行**:
| `-(0, x)` / `-(x)`(一元 desugar) | 取负,等价于 `NEG(x)` | 数字 / ERR 透传 |

**与 v0.7 差异**:§1.6 引入 `int_literal = [sign] int_lit` 复合非终结符,描述实际两步路径。`NEG` 的位置不变(parser 不合成,只通过 `NEG(x)` 名调用可达)。

### 3.8 §3.1 / §5.1 · LET / FUN 不对称 normative

**§5.1 末尾新增段**:
> `FUN(name(params), body)` 与 `LET(name, FUN(...))` 等价,二者表达式的值都是该函数值;`LET(name, value)` 的值是 `NULL`(绑定动作已完成,无值可传递)。`LET` 因此**不得**作为实参或运算符操作数(语义上是"声明语句"而非"值表达式")。如需在表达式位置产生绑定效果,请用 `LET MUT` + `SET` 序列或将值绑定到一个 `FUN` 表达式。

**与 v0.7 差异**:新增一段 normative。
**impl 影响**:无 — 现有 grammar 已经隐含限制(`LetExpr` 是 statement 位置,不在 `Expression` 子类里)。

### 3.9 §1.5 / §4.5 / §5.2 · `=` 三重身份消歧

**§1.5 末尾新增段**:
> `=` 的角色由**文法位置**唯一决定:
> 1. 出现在 `OpToken` 位置(`=` 后紧跟 `(`)→ 相等函数调用 `=(a, b)`,与 `==` 同义(注册表 `Eq => "=="` desugar)。
> 2. 出现在 `Postfix` 形式的 `=` 位置(`... [ Expression ] "=" Expression`)→ `INDEX_SET` 语法糖。
> 3. 出现在形参表 `Param` 内(`name = Expression`)→ 默认值分隔符。
>
> 三者互不重叠,无需词法前瞻。

**§A.2 文法补充**:
```
Postfix     = "." identifier [ "(" [ Args ] ")" ]
            | "[" Expression "]"                              // INDEX_GET
            | "[" Expression "]" "=" Expression .              // INDEX_SET
                                                              // 注:此 `=` 是终结符,非 OpToken
```

**与 v0.7 差异**:新增消歧段落,§A.2 Postfix 文法加注。

### 3.10 §17.1 / §17.7 · YIELD 位置限制降级

**§17.1 当前 815-831 行**:把 `YIELD` 必须出现在 Block 直接子项写成硬规则。

**v0.8 改写**:
> `YIELD()` 的**理想语义**是:出现在任何表达式位置,均可在该点让出。**v0.7 / v0.8 参考实现**采用任务体静态切段调度(impl `wlwl-eval/src/yield_split.rs`),因此仅支持 `YIELD()` 出现在多语句 `Block`(`( a; b; … )` 或函数体 `;` 序列)的直接子项位置;其他位置产生 `E0014`。**该限制是 v0.7 / v0.8 实现路径的产物**,不是语言语义规则;未来版本(挂起式调度)可放宽而不视为破坏性修订。

§17.7 限制表的对应行从"位置限制在 §17.1"改为"`[v0.7 / v0.8 实现路径]` 嵌套构造内 `YIELD` 不恢复嵌套体剩余部分",措辞一致化。

尾部 `YIELD` 一并移到 §17.7 限制表(报告 G-09)。
**与 v0.7 差异**:§17.1 从硬规则降为"理想语义 + 实现限制";§17.7 措辞统一。

### 3.11 §2.1 / §10.11 · TASK 类型名 vs 命名空间成员

**§2.1 末尾新增段**:
> 类型名(`NULL`/`BOOLEAN`/`INTEGER`/`FLOAT`/`STRING`/`ARRAY`/`DICT`/`FUNCTION`/`RESULT`/`TASK`/`CHANNEL`)是 `TYPE` 返回的字符串,**不**构成标识符绑定。命名空间成员可以与类型名同名(如 `wlwl:std.agent.TASK`),二者在各自作用域中独立解析。`TYPE` 的参数位置不做名字解析 — 它取运行时值,返回字符串,过程中不查任何名字表。

**§10.11 改写**:对所有 `TASK` 标识符加 `wlwl:std.agent.` 前缀(只改文档示例,不改 §10.11 实际签名表格里的"显示形式")。

### 3.12 §17.3 / §17.5 · SHIELD / SCOPE(ERR)澄清

**§17.3 末尾新增段**:
> `SHIELD(fn)` **不**创建新作用域,只是取消屏蔽的包装。它可以出现在任何位置;在顶层调用时,`SHIELD` 仅屏蔽来自当前任务的取消(顶层任务无取消源,故为 no-op)。`SHIELD` 与 `SCOPE` 是两个正交维度:作用域负责生命周期,屏蔽负责取消推迟。两者可独立组合(`SCOPE(FUN(() , SHIELD(...))))`)。

**§17.5 新增段**:
> `SCOPE(ERR("x"))` 中,实参 `ERR("x")` 在 §8.2 透明传播阶段先于 §17.1 参数类型检查(`fn` 必须为函数)生效,故结果是 `ERR("x")` 透传,**不**产生 `E0052`。`E0052` 仅在 §8.2 透明传播放过之后、参数被识别为非函数时触发。

### 3.13 §17.1 · AWAIT 宿主诊断

**§17.1 AWAIT 行(原 804 行)末尾新增段**:
> "宿主诊断"特指实现内部错误(如 `E0101` 栈溢出、底层通道句柄失效 `E0053` 等)。该类诊断在 `AWAIT` 处重新抛出,使用 `WlwlError` 的 trace 链把目标任务的失败帧关联起来;**用户 `ERR` 值**(RESULT 变体)按 §17.5 作为普通结果返回。

### 3.14 §11.2 / §2.2 · `%` 浮点

**§2.2 当前(162 行)**:`%(a, b)` 只接受整数。**§11.2 末尾补一行**:

| 触发位置 | 错误码 | 触发条件 |
|---------|--------|---------|
| `%` | `E0030` | 至少一个实参是 `FLOAT` |

### 3.15 §4.5 · 删除字面量下标禁止句

**当前 §4.5 末句**(308 行):
> 下标链挂在变量、调用或属性访问之后;对数组、字典字面量直接施加下标不在本规范内。

**v0.8 删除该句**,并在 §A.2 PostfixExpr 注释里写:
> `Postfix` 接在 `Primary` 之后;`Primary` 含 `ArrayLit` / `DictLit`,因此 `[1,2,3][0]` 与 `["a": 1]["a"]` 都是合法表达式,等价于 `INDEX_GET` 字面量。

(impl 已配合 §2.4 改 parser。)
**兼容性**:无 — v0.7 禁止、v0.8 允许,只是放宽而非收紧。

### 3.16 附录 G 重生成

按 §2.3 改完 registry anchor 后,跑:
```
cargo run -p wlwl-eval --bin gen_appendix_g
```
写入 `docs/appendix_G.md`。
**锁测试**:`b11_generated_md_matches_registry` 必须绿(锁的是注册表与 md 一致)。

---

## 4 验收(Phase C)

### 4.1 必须跑的测试集

```powershell
cd D:\Project\wlwl\impl
cargo test -p wlwl-parser            # parser roundtrip + lint
cargo test -p wlwl-eval               # 全套 evals + b11_* + b9_not + pop_*
cargo test -p wlwl-std                # std 模块 (FORMAT/AGENT/AI)
cargo test -p wlwl-error              # E-code snapshots
cargo test -p wlwl-formatter          # formatter
cargo test -p wlwl-toml               # manifest / lock
cargo test -p wlwl-ast                # stable API surface
cargo test --workspace                # 集成 + conformance
```

全部绿 → A 阶段完成。

### 4.2 文档 → 行为差分基线

`impl/crates/wlwl-eval/tests/fixtures/v07_fidelity_v06_baseline.jsonl` 是 v0.6→v0.7 的差分基线。v0.8 在该基线上跑:
- 输出必须包含 v0.7 baseline 的全部 PASS
- 任何 v0.8 新行为(字面量下标允许、`EXPECT_ERR` 入表)在该 JSONL 上添加新行
- `§3.4 NOT 透传`反向 — 不应改变 v0.6/v0.7 任何 baseline,因为实现一直是这样

### 4.3 deviations 登记

新建 `docs/plan/deviations.md`,记录:
- D8-001 POP 注册签名(§2.2)
- D8-002 附录 G 章节号(§2.3)
- D8-003 NOT ERR 行为规范反向(§3.4)
- D8-004 字面量下标放宽(§2.4 / §3.15)
- D8-005 E0055/E0057 标注(§2.6 / §3.3)

### 4.4 与 v0.7 的兼容承诺(写入 CHANGELOG.md)

> v0.8 兼容承诺:任何在 v0.7 下有定义、且**不依赖 §12 旧保留形式表**(v0.8 起由附录 G 替代)的程序,在 v0.8 下求值结果相同。已确认会改变可观察行为的边界场景:
>
> 1. `[1,2,3][0]` 在 v0.7 下为解析错误(`E0010` / `E0011`),v0.8 起为 `1`。
> 2. `EXPECT_ERR` 在 v0.7 下文档孤立(§8.3 段末);v0.8 起作为 §8.3 表内正式条目 — 行为不变,仅文档重组。
>
> 未改变的行为(无兼容性风险):
> - §3.4 反向 NOT:实现始终是 E0102,仅规范文本 v0.7 写错。
> - §3.6 FORMAT:实现始终支持混用,仅规范文本 v0.7 含糊。
> - §2.3 / §3.16 附录 G 章节号:仅 markdown 链接更新,不影响执行。

---

## 5 不在本计划范围

1. **§13–§16 章节填充**(模块系统 / OOP / 运行时等)— 留 v0.9
2. **报告附录列出的 5 项 v0.9 候选**(concurrency 特性开关、RUN_TESTS 字段缺失、STRINGIFY 排序、CHANNEL_NEW buf 上界、格式化器文档注释)— v0.9 议程
3. **编码规范修改**(UTF-8、换行三态、标识符字符集、字符串转义)— 用户明示禁止
4. **新语言特性**(§12.4 PANIC 重写、§17.3 SHIELD 加参数等)— v0.9+

---

## 6 实施顺序

```
[Step 1] 改 impl 改动(A 阶段 §2.1–§2.8)
   ├─ §2.3 anchor 修正
   ├─ §2.2 POP 签名修正
   ├─ §2.6 E0055/E0057 注释
   ├─ §2.8 std.ai/agent TASK 文档注释
   ├─ §2.4 parser 字面量下标允许(抽 apply_postfix_loop)
   └─ §2.5 parser / lexer 一元负号注释
[Step 2] 一致性测试增补(§2.9 八件套)
[Step 3] cargo test --workspace 全绿
[Step 4] cargo run -p wlwl-eval --bin gen_appendix_g
[Step 5] 改文档(B 阶段 §3.1–§3.16)
   ├─ §3.2 §12 重写(独立 PR)
   ├─ §3.4 §4.3 NOT(独立 PR)
   ├─ §3.10 §17.1 YIELD 降级(独立 PR)
   └─ 其余条目并入 §17 / §8 / §11 章节
[Step 6] 写 deviations.md / 更新 CHANGELOG.md / 更新 README §0.4 一致性表
[Step 7] 端到端验收(§4)
```

Step 1–4 是 A 阶段;Step 5 是 B 阶段;A 与 B 之间不强求 PR 合并顺序,但 B 必须在 A 完成后才能保证文档与代码一致。

---

## 7 风险与回退

| 风险 | 影响 | 缓解 |
|------|------|------|
| §2.3 改 anchor 触发 `b11_generated_md_matches_registry` 失败 | 附录 G 与注册表脱钩 | 改 anchor → 重生成 md → 再跑测试三步走,在同一 commit 完成 |
| §2.4 字面量下标改动 parser 触发现有 parse 测试失败 | 字面量下标路径与现有解析路径冲突 | `apply_postfix_loop` 必须与 `parse_call_or_ident` 末尾循环语义完全一致;先抽函数、跑现有测试,再加字面量分支 |
| §3.4 反向 NOT 被外部 reader 误读为"§4.3 削弱" | 第三方 / 工具作者抱怨 | 在 CHANGELOG.md / README.md §0.4 一致性表里写明"实现未变,文档反向;v0.7 文字本身就是 bug" |
| `gen_appendix_g` 二进制不在 PATH | 无法重生成 md | 检查 `impl/crates/wlwl-eval/src/bin/gen_appendix_g.rs` 存在性;不存在则手写 anchor 替换 |
| deviations 与历史文档格式冲突 | v0.7 的 `deviations-v0.7.md` 已归档到 `history/`,新 `plan/deviations.md` 独立维护 | 本计划明示 `docs/plan/deviations.md`(新),不走 `history/` |

---

## 8 与第三方报告的具体差异记录

> 这段不是规范内容,是给将来 v0.8 评审者看的设计意图备忘。

| 报告条目 | 报告处方 | 本计划处方 | 差异理由 |
|---------|---------|-----------|---------|
| F-02 §12 vs 附录 G | 选项 A:`AND`/`OR` 弃用别名产生 W0051 | 直接让注册表成唯一真相,AND/OR 保持 macro,§12 不再列 | 注册表早就把它们当 macro,加弃用别名只是噪音 |
| F-03 POP | "重写 §10.4 注记" | 只改 registry 一行;§10.4 注记没错 | 报告没看 impl/lib.rs:1792-1799 的 deviation |
| F-04 交叉引用 | "由 gen_appendix_g 重新生成" | 同;并把 `BuiltinGroup::anchor` 的旧章节号修齐 | 一致 |
| F-06 NOT | "推荐:把 NOT 加入消费者表" | **反向**:改 spec 移除"除 NOT 外" | 报告没看 lib.rs:17612 `b9_not_with_err_arg_is_e0102` 锁测试 |
| F-09 FORMAT | "明确 `{N}` 索引基准" | **驳回**:实现已正确;只补一句规范说明 | 报告没看 `wlwl-std/src/format.rs:183-210` 与 `format_positional_refers_to_dict_arg_renders_it` |
| F-10 负数 | "补消歧规则 `-1` 是字面量,`-(...)` 是调用" | 描述改成 lexer 不读 + parser desugar | 报告以为 lexer 读前导,实际是 parser 改写 |
| G-03 YIELD | "把 §17.1 位置限制降级为实现限制" | 同 | 一致 |
| G-04 TASK 同名 | "§2.1 补一条 normative 声明 + §10.11 用全限定" | 同 | 一致 |

**采纳率**:13/19 条目采纳为"建议合理";6/19 条目采纳问题但反向处方。

---

> **下一步**:Step 1 — 改 `registry.rs` anchor + POP entry,然后跑 `cargo test -p wlwl-eval --lib` 确认锁测试全绿。这是 v0.8 第一个可独立 commit 的源码 PR,预计 diff 在 60 行以内。
