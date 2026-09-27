# WLWL v0.10.0 构建计划

> **状态**:草稿 0(依据 `wlwl-v0.10.0-迭代技术路线建议.md` 生成,**待用户批准**)
> **主题**:**Static Contracts** — 类型最小闭环 + 模块契约 + MATCH 穷尽性
> **基线**:impl v0.9.0(wip0.9,锁测试 ≈1517 passed / 0 failed)+ `docs/standard/wlwl-spec-v0.9.md`
> **上游裁决**:`wlwl-v0.10.0-迭代技术路线建议.md`(正式版,以下简称《路线》);与研究报告冲突处以《路线》为准
> **效力关系**:采纳实现方 `wlwl-v0.10.0-路线辩论结论.md` 代码论据;**包管理议题双重关门**(ADR-0011 + 业主硬约束),本计划任何条目不得回潮
>
> **不在范围**:包管理器一切扩展;宏/用户元编程;ADT Variant 新值形态;类型收窄;用户代数效果处理器;ownership/lifetime;Wasm/字节码后端交付;会话 `par`/多线程(ADR-0016);调试器全量/HAMT/依赖类型/AI 专用语法
>
> **源材料**:
> - 仓库内:`docs/plan/wlwl-v0.10.0-迭代技术路线建议.md`、`docs/adr/0010-strict-types-behavior.md`、`0011-mvs-dependency-resolution.md`、`0016-scheduler-single-thread-boundary.md`、`docs/standard/wlwl-spec-v0.9.md`、`CHANGELOG.md` v0.9.0 段
> - 证据锚点:`impl/crates/wlwl-ast/src/lib.rs`(TypeAnnotation / Pattern)、`wlwl-eval/src/lib.rs`(ModuleLoader / strict_types / E0033)、`wlwl-eval/src/registry.rs`(BuiltinSpec.signature)、`wlwl-eval/src/runtime.rs`(Effect 半落地)、`wlwl-cli/src/main.rs`(Check / ast --json)、`wlwl-toml/src/{manifest,lock,mvs}.rs`
>
> **立场**:v0.9 把控制与运行时前沿做完了;v0.10 **不再横向加能力**,而是给已有能力补上**静态契约层**。默认关闭时零破坏,开启后抓住注解/签名/MATCH 错误。
>
> **核心纪律**:
> 1. **默认零破坏** — `gradual_typing` 等新特性默认关闭时,现有 ≈1518 测试全绿;
> 2. **新语义进新 crate** — 静态 pass 落在 `wlwl-types`,不侵入 21,765 行的 `wlwl-eval`;
> 3. **无人日不立项** — 每项立项单含「挂载点 → 变更面 → 预估人日 → 验收命令」(ADR-0011 纪律);
> 4. **包管理零动作** — `wlwl-toml` 冻结,`E0041`/`E0045` 触发语义不变。

---

## 0 摘要

### 0.1 主题论断

| 维度 | 命题 | 论据 |
|------|------|------|
| **主题** | v0.10.0 = **Static Contracts**:类型最小闭环 + 模块契约 + MATCH 穷尽性 | 《路线》§3.1;六方 6/6 一致认为类型薄弱是最大缺口 |
| **P0 主线** | P0-1 `wlwl-types` 静态 pass + P0-2 模块契约(签名/可见性/sig-gen) | 《路线》§4.1;类型注解 AST 槽位已存在(`wlwl-ast/lib.rs:133,181,360`),`check` 只 parse(`main.rs:51-52`)是天然挂载点 |
| **P1** | MATCH 穷尽性(6 形态 Maranget)+ 泛型限形(擦除)+ 工具链薄壳 | 《路线》§4.2;Pattern 形态集窄,算法增量成本可控 |
| **P2 余力** | std 加法(BYTES/TIME/MATH/RANDOM)+ 效果半落地收口 + 能力沙盒旁路原型 | 《路线》§4.3;不阻塞发版 |
| **不做** | 包管理一切扩展、宏、Variant、收窄、用户效果、ownership、后端交付、par、多线程 | 《路线》§5;业主硬约束 + ADR-0011/0016 |
| **可观测收益** | `check` 从 parse 升级为可选静态检查;注解失配/签名违例/MATCH 非穷尽可诊断;`wlwl lsp` 薄壳 + interface/schema JSON | 默认关闭时零可观察变化 |
| **风险等级** | 中 | 静态 pass 侵入 eval / 注册表结构化低估 / 模块签名破坏 IMPORT / 范围蠕变 — 均有缓解(§9) |

### 0.2 与 v0.9 及既有 ADR 的关系

| 历史裁决 | 状态 | v0.10 是否改动 |
|---------|------|---------------|
| **ADR-0010** *strict_types runtime check via transient cast* | Accepted 2026-09-19 | **有意推翻局部** — 重开「静态检查」方向,但范围限缩为「边界契约 + 期望类型向下传播」,**不是**当年否决的「全程序类型推断/HM」。须**新立 ADR-0020** 明文推翻范围与人日预算 |
| **ADR-0011** *MVS dependency resolution* | Accepted | **不变** — 包管理冻结;**人日否决纪律**适用于所有 v0.10 立项 |
| **ADR-0016** *scheduler single-threaded boundary* | Accepted | **不变** — 会话 `par`/多线程不做 |
| ADR-0017/0018/0019(并发/OOP/effects) | Accepted | **不变** — 运行时 `E0033`/`strict_types`/OOP 路径保持;仅 P2 收口 `Effect::MethodCall`/`ProtocolViolation` 悬空承诺 |
| `wlwl-toml` manifest/lock/MVS | 已实现 | **冻结** — 修 bug 可以,v0.10 无字段/算法/生态面扩展 |

### 0.3 三段式目标

| 阶段 | 范围 | 文档位置 |
|------|------|---------|
| **A · 静态契约核心** | `wlwl-types` 新 crate;`gradual_typing` 开关;容器/函数类型最小集;期望类型向下传播;注册表结构化签名首批 | §3 |
| **B · 模块契约 + 算法 + 工具链** | 模块签名/可见性/`SEALED`/sig-gen;MATCH 穷尽性;泛型限形;`check` 语义化 + `wlwl lsp` 薄壳 + interface/schema JSON | §4 / §5 |
| **C · 文档 / 验收** | ADR-0020;spec v0.10 派生;conformance 目录;CHANGELOG;README 一致性表;D10-NNN 偏差登记 | §6 / §10 / §11 |

A 是 B 的类型底座;C 与 A/B 并行可独立 review。P2(§7)为余力项,不阻塞出口。

### 0.4 v0.9.0 baseline(本计划不重写)

- spec v0.9(§1–§17 + 附录 A–G);OOP §13–§16 已填充;§17 algebraic-effect framing 已落地;
- 错误码:**61 激活 + 6 保留**(共 67);警告码含 W0065/W0066;
- 内建 **110** 条;`BuiltinSpec.signature` 仍为 `&'static str` 文档串(`registry.rs:44-70`);
- `wlwl-eval/src/lib.rs` ≈ **21,765 行** / 789 内嵌测试;workspace 锁测试 ≈ **1517 passed** / 0 failed;
- `strict_types` 运行时边界检查(`lib.rs:8584-8627`),默认关,失配 `E0033`;「Nested generic / array element matching is deliberately deferred」(`lib.rs:8588`);
- `check` 子命令 = parse only(`main.rs:51-52`,测试 `check_only_parses`);
- `wlwl ast --format=json` 机器可读导出已有(`main.rs:59-61,156-208`);
- 模块加载 `ModuleLoader`(`lib.rs:797-871`):EXPORT 名集合 + 循环检测 + 命名空间,**无签名/无私有/无 SEALED**;
- `.wll` 夹具约 22–30 个(conformance + concurrency + examples);
- `Effect::MethodCall` / `Effect::ProtocolViolation` 仅 enum 表面(`runtime.rs:186-265`),CALL_METHOD 路径**未 raise**(CHANGELOG Known limitations);
- `wlwl-toml`:manifest / lock / MVS 已实现,CLI 可生成 `wlwl.lock`。

v0.10 任何 commit **不得降低上述指标**(锁测试数只增不减;`wlwl-toml` 零功能扩展)。

### 0.5 容量口径(替代「感觉超载」)

> **1 个新语义子系统(静态类型 pass)+ 2 个生态增量(模块契约、工具链 JSON/LSP 薄壳)+ 1 个算法增量(MATCH 穷尽性)+ 纯加法库。**
> **泛型是唯一可挤进本版的深水区,且必须擦除 + 限形。**

禁止「类型 + ADT + 泛型 + 模块 + LSP + 库 + 效果」齐上(v0.9 四件套后仍留 Known limitations 的教训)。

---

## 1 现状盘点(证据基线)

### 1.1 关键源码位置

| 关注点 | 文件 / 行 | 摘要 |
|--------|----------|------|
| 类型注解 AST | `wlwl-ast/src/lib.rs:86-126,133-146` | `TypeExpr::{Ident, Array, Generic}`;`TypeAnnotation { expr, span, text }` |
| 参数/返回注解槽位 | `wlwl-ast/src/lib.rs:181,360` | `FunParam.type_annotation`;`return_type` — 静态层有挂载面 |
| Pattern 六形态 | `wlwl-ast/src/lib.rs:228-254` | Ident / Wildcard / Literal / Array / Dict / Constructor(仅 OK/ERR) |
| Check = parse | `wlwl-cli/src/main.rs:51-52,85-88,630` | `Cmd::Check` → `run_file(..., false)`;测试 `check_only_parses` |
| `ast --json` | `wlwl-cli/src/main.rs:59-61,156-208` | 机器可读导出先例 |
| 运行时类型边界 | `wlwl-eval/src/lib.rs:8584-8627` | `strict_types` 注解对比 `TYPE` 名;失配 `E0033`;嵌套泛型/数组元素**故意推迟** |
| `with_strict_types` | `wlwl-eval/src/lib.rs:6530-6548` | 默认 `false`;CLI 从 `wlwl.toml [features] strict_types` 读入(`main.rs:138-146,502-525`) |
| 模块加载 | `wlwl-eval/src/lib.rs:774-871,1036-1133,1215-1224` | `ModuleLoader` + `collect_exports`;EXPORT 名集合;无签名校验 |
| 注册表签名串 | `wlwl-eval/src/registry.rs:44-70` | `BuiltinSpec.signature: &'static str` — 文档串,非结构化类型 |
| 效果半落地 | `wlwl-eval/src/runtime.rs:186-265,220` | `Effect::MethodCall`/`ProtocolViolation` 枚举已建,**未 raise** |
| 包管理三件套 | `wlwl-toml/src/{lib,manifest,lock,mvs}.rs` | manifest/lock/MVS 已实现;**冻结** |
| std 分层 | `wlwl-std/src/{io,fs,json,collection,format,test,ai,agent}.rs` | 分层已存在,「分层空白」不成立 |
| 错误码注册 | `wlwl-error/src/lib.rs:26+` | 47+ 注册码段;`E0030-E0039` type;`E0040-E0043` module |

### 1.2 已存在、不得再当「缺口」的

| 能力 | 证据 | 含义 |
|------|------|------|
| 清单 + 锁文件 + 版本求解 | `wlwl-toml/src/{manifest,lock,mvs}.rs` | 「包管理缺失」诊断为假 |
| 依赖求解算法裁决 | ADR-0011:MVS(1 天)**否决** PubGrub(1–2 周) | 禁止倒裁 |
| 类型注解 AST 槽位 | `TypeAnnotation` / 参数/返回注解 | 静态层有挂载面 |
| `wlwl ast --format=json` | CLI 已有 | 机器可读导出先例 |
| std 模块分层 | `wlwl-std` 已按域分文件 | 仅需命名/文档整理(C5′) |
| `strict_types` 运行时边界检查 | `lib.rs:8584-8627`;默认关;`E0033` | 运行时路径有测试锁,**须保留** |
| 会话/效果/OOP/线性 `THIS` 运行时 | v0.9 已落地 | 本版不深挖并发 |

### 1.3 真缺口(经代码核验)

| 缺口 | 证据 | 对应动作 |
|------|------|---------|
| **无静态类型层**:注解只是字符串,与运行时 `TYPE` 名对比 | `lib.rs:8587-8600`;ADR-0010 | 新建 `wlwl-types` 静态 pass |
| **`check` 只 parse**,无语义/类型检查 | `main.rs:51-52`;`check_only_parses` | `check` 语义化是天然挂载点 |
| **无模块导出契约**(无签名、无私有、无 SEAL) | `lib.rs:797-871` | 模块签名 + 可见性 |
| **MATCH 穷尽性无检查**;`Pattern` 仅 6 形态 | `ast/lib.rs:228-254` | Maranget 在此形态集属小型算法 |
| **注册表签名是文档串**,非结构化类型 | `registry.rs:44-70` | 分批结构化 |
| **效果半落地** | `runtime.rs:186-265`;CHANGELOG Known limitations | 收口:真正 raise **或** 规范降级(P2/S4) |
| **求值器单体风险** | 21,765 行 / 789 测试 | 新语义进新 crate/pass,**勿侵入 eval** |
| **测试夹具偏薄** | `.wll` 仅 ~22–30 | 类型/模块需新增 conformance 目录 |

### 1.4 证据纪律

| 等级 | 含义 | 用法 |
|------|------|------|
| **E1 代码事实** | 仓库直接读到的结构/行为/测试 | 裁决硬依据 |
| **E2 项目文档** | ADR / CHANGELOG / build plan | 佐证「为何这样做」 |
| **E3 规范文本** | `wlwl-spec-v0.9.md` | 只约束语言语义边界,不能代替实现现状 |
| **E4 工程外推** | 成本、算法移植等经验判断 | **必须标明**,不得写成「已验证」 |

**ADR 纪律**:与 ADR-0010 冲突的立项必须单列为**有意推翻**,写明成本与迁移,不得写成顺手升级。

---

## 2 主题论证

### 2.1 为什么 v0.10 是「静态契约」而不是「再加语言能力」

1. **缺口在契约层,不在表达力**。v0.9 已交付并发真挂起、OOP、行为类型、线性 `THIS`、algebraic-effect 命名;继续横向加 Variant/宏/效果用户化只会复刻 v0.9 的 Known limitations 债。
2. **类型注解已进 AST 但未进语义**。`TypeAnnotation` 自 v0.3 起就挂在参数/返回位,运行时只做 `strict_types` 边界对比且**故意推迟**嵌套匹配——补静态层是「兑现已有语法承诺」,不是新语法面。
3. **`check` 是零成本挂载点**。今天 `check` = parse;升级为 `parse → 可选静态 check` 不改变 `run` 路径,默认关闭即可保证 S1。
4. **模块边界是类型契约的天然验证点**。`ModuleLoader` 已有 EXPORT 集合;加签名即可在边界验收,**不依赖包管理器**(否决 Qwen「类型跨包必配包管理」推论)。
5. **MATCH 穷尽性是算法增量**。Pattern 仅 6 形态、Constructor 仅 OK/ERR、无 or-pattern/guard/ref pattern — Maranget usefulness 在此形态集是小型算法,比六方估计更可落地(《路线》上调成立)。

### 2.2 与 ADR-0010 的关系(必须明文)

ADR-0010 当年以「全程序类型推断 = triple the work」否决静态检查,选运行时 transient cast。v0.10 **有意重开静态方向**,但:

| 维度 | ADR-0010 否决的对象 | v0.10 实际范围 |
|------|-------------------|---------------|
| 范围 | 全程序 HM / let-多态 / 完整推断 | 边界契约检查 + **期望类型向下传播**(局部、上下文驱动) |
| 落点 | 假想的编译期检查器 | **独立 crate `wlwl-types`**,不侵入 eval |
| 运行时 | 替代 transient cast | **`E0033` 路径保留**;静态层是额外关卡 |
| 默认 | — | **默认关闭**,零可观察变化 |
| 预算 | 「triple the work」 | **单列人日**(§8.4),超支降级 |

**配套动作**:新立 **ADR-0020 *Gradual Static Contracts***(草案见 §6.1),在 CHANGELOG 与 ADR 中明文「推翻范围限缩」,避免历史歧义。

### 2.3 工业对照(简表,E4 外推)

| 能力 | 同类系统 | WLWL v0.10 定位 |
|------|---------|----------------|
| 渐进类型 / 默认关 | TypeScript `strict`、Python typing、Lua LS | `gradual_typing = off \| warn \| error` |
| 模块签名 | OCaml `.mli`、TypeScript `.d.ts`、Rust `pub` | 可选签名文件 + `SEALED MODULE` |
| MATCH 穷尽 | Rust `match`、Scala、Elm | 6 形态 usefulness / Maranget |
| 泛型擦除 | Java erasure、Python | 运行时 `Value` 不变,无 monomorphization |
| 机器可读诊断 | LSP diagnostics、`ast --json` | 顺增量 `interface`/`schema` JSON |

---

## 3 P0-1 · `wlwl-types` 静态 pass(Pillar A 改道版)

> **人日纪律**:挂载点(文件/符号)→ 变更面 → 预估人日 → 验收命令。缺任一项不予立项。

### 3.1 A1 — 类型 AST / 静态类型环境

> **状态:已完成(Step 1,commit `859400b`)。** 下表的记法与实现细节已按实测结果修订,
> 与 `wlwl-types` 源码逐条对齐。

| 项 | 内容 |
|----|------|
| **目标** | 新建 crate `wlwl-types`:静态类型表示、类型环境、诊断类型;**与运行时 `TYPE` 严格分层** |
| **挂载点** | 消费 `wlwl-ast` 的 `TypeExpr` / `TypeAnnotation` / `FunParam.type_annotation` / `return_type`;**不进 `wlwl-eval`** |
| **变更面** | `impl/crates/wlwl-types/`(新);`impl/Cargo.toml` workspace 成员 +1;`wlwl-types/src/{lib,ty,env,diag}.rs`(Step 2 起加 `check.rs`) |
| **静态类型最小集** | `INTEGER` / `FLOAT` / `STRING` / `BOOLEAN` / `NULL` / `ARRAY[T]` / `DICT[K, V]` / `OPTION[T]` / `RESULT[T, E]` / `FUN[T, …] -> U` / **`Dynamic`**(未标注回退) / `Named`(本层不结构化建模的具名类型) |
| **明确不做** | HM / let-多态;流敏感收窄(推迟);`Value` 模型改动 |
| **预估人日** | **3–4 人日**(E4) |
| **验收** | `cargo test -p wlwl-types`;类型 round-trip 单测;`TypeExpr` → 静态类型映射锁测试 |
| **实测** | 19 项测试;`cargo test -p wlwl-types` 19/0(Step 1 收尾时) |

**设计约束**:

- 静态类型是 `wlwl-types` 内部 IR,**不改** `wlwl-ast::TypeExpr` 语法面(additive only;若需扩展类型名,走 `Generic { name, args }` 既有槽位);
- `Dynamic` 是顶/底元:与任何类型可互通,未标注即 `Dynamic`;
- 运行时 `TYPE(x)` 字符串对比路径**不动**(S3)。

**记法(实测修订 —— 本文档原先用尖括号 `ARRAY<T>`,与实现不符)**:

> `wlwl-ast::TypeExpr` 的解析结果只有 `Ident` / `Array` / `Generic` 三形态,
> `Array { element }` 与 `Generic { name, args }` **都用方括号**
> (`wlwl-parser/src/lib.rs:2429-2473` 的 `parse_braced` 只认 `[`)。
> 因此本计划正文一律改用方括号 `ARRAY[T]` / `DICT[K, V]`。这不是风格偏好 ——
> 尖括号写法的 `parse(display(ty)) == ty` **往返不成立**,方括号才成立
> (`ty::tests::display_roundtrips_through_parser` 锁住了这一点)。

**实施中定下的三条语义澄清(原计划未写,Step 1 实测后确定)**:

1. **`OPTION[T]` 是注解糖,无运行时对应类型**。spec v0.9 §2.1 的 13 个运行时类型
   里没有 `OPTION` ——「无值」由 `NULL` 或 `RESULT` 表达。故 `Ty::Option` 只在
   注解侧存在;它与运行时值的匹配规则归 A4 决定,A1/A3 不猜。
2. **`FUN[T, …] -> U` 是 IR 变体;方括号形式可从源码到达,箭头形式不可**
   —— **Step 3 实测修正**。`wlwl-ast/src/lib.rs:83-84` 把 `FUN(...) -> T`
   标为 "reserved for v0.4",至今未实现;lexer 57 个 `TokenKind` 里**没有**
   `->` 终结符(`-` 与 `>` 是两个独立 token)。但要分清两件事:
   - `FUN[INTEGER, STRING]`(方括号)**能正常解析**并映射到 `Ty::Fun`
     (形参取全部类型参数,返回 `Dynamic`)→ IR 变体**是**可达的;
   - `FUN(INTEGER) -> STRING`(箭头)**不报错,但被静默吸收**成
     `Named { name: "( INTEGER ) - > STRING" }` → 详见 §3.3 的陷阱说明。

   接 parser 的箭头语法待 **D-5**。
3. **spec v2.1 的 `TASK` / `CHANNEL` / `CLASS` / `INSTANCE` 有意走 `Named`**,而不是
   各开一个变体 —— 变体集合严格等于本表的最小集,不多开一个。`Named` 同时兜住
   未知头与元数错误两种情形,后者**保名保参**不静默丢弃,好让 A4 报出
   「元数不对」而不是「类型不认识」。


### 3.2 A3 — `gradual_typing = off \| warn \| error`

> **状态:已完成(Step 2,commit `3442c9a`)。** 诊断码已由「草案」定稿为 D-1 裁决结果。

| 项 | 内容 |
|----|------|
| **目标** | 新编译期边界检查;运行时 `E0033` **保留**;默认 `off` |
| **挂载点** | `Check` 从 parse 升级:`parse → 可选静态 check`;`run_file` 挂载同一入口 |
| **变更面** | `wlwl-cli/src/main.rs`(Check/run 接线);`wlwl-toml/src/manifest.rs` **仅读** `[features] gradual_typing`(新键,默认 `"off"`;**不改求解/锁/依赖字段**);`wlwl-types/src/check.rs`;`wlwl-error/src/lib.rs`(码段注册 + 新增 `WlwlDiagnostic::with_severity`) |
| **配置** | `wlwl.toml [features] gradual_typing = "off" \| "warn" \| "error"`(默认 `"off"`);非法值 → 诊断并回落 `off` |
| **兼容句** | `gradual_typing=off` 时 v0.9 程序行为不变;新增编译期诊断只在显式开启后出现 |
| **预估人日** | **3–4 人日**(E4) |
| **验收** | 默认关:`cargo test --workspace` 全绿;开启:注解失配夹具报稳定诊断;`check_only_parses` 升级为分层测试(parse / static / run) |
| **实测** | `default_off_zero_diag` / `warn_mode_soft` / `error_mode_hard` / `error_mode_does_not_reject_clean_programs` / `invalid_value_falls_back_to_off_and_is_reported` / `check_is_layered_parse_static_run`;`cargo test --workspace` 1568 / 0 |

**诊断码定稿(决策 D-1 已拍板,见 §10.2 —— 新建静态段,不占用 `E0030-E0039` 运行时类型段)**:

| 条件 | `error` 模式 | `warn` 模式 |
|------|-------------|------------|
| 注解失配(边界) | `E0110` static annotation mismatch | `W0110` |
| 调用参数个数/类型失配 | `E0111` static call mismatch | `W0111` |
| 返回类型失配 | `E0112` static return mismatch | `W0112` |

> 既有 `E0033`(运行时 `strict_types`)语义与触发路径**不变**。静态码与运行时码可在同一程序并存(先静态拦、再运行时兜底)。

**A3 的能力边界(实测修订 —— 原计划未写清,必须在 spec 与 CHANGELOG 如实声明)**:

1. **A3 抓不到内建调用**。注册表结构化签名归 A6′(§3.5,Step 5);在那之前
   `BuiltinSpec.signature` 仍是文档串 `&'static str`(`registry.rs:48`),内建返回
   类型**不可知** → 一律落 `Dynamic` → 不报。所以 A3 只能抓**用户定义的、带注解
   的**函数边界。这是范围裁剪,不是缺陷,也不是待修的 bug。
2. **因此不得宣称「开了就等于完备类型检查」**。spec 派生(§6.2)必须明写覆盖率
   边界,否则用户会形成错误期望。
3. **`INTEGER -> FLOAT` 算可赋值**。ADR-0010 明写这是 *silent upcast, even under
   strict*;静态层若不认这条安全方向,`error` 档会拒掉运行时认为合法的程序,直接
   违反「不误报」验收项。反方向 `FLOAT -> INTEGER` 仍然拒绝
   (`ty::tests::assignability_allows_only_the_safe_numeric_widening`)。
4. **返回值要查「块尾表达式」,不只查显式 `RETURN`**。spec §4.4 规定括号序列的值
   即最后一个表达式的值,`FUN((a): STRING, 42)` 这种最常见写法**根本不含
   `RETURN` 节点**,只查 `Return` 会整类漏报。
5. **不重复报未定义名字**。那由 parser 的 `lint` pass 负责(`W0001`);静态 pass
   重复报会让一个问题出两张诊断。
6. **`off` 档是「不调用 checker」,不是「调用后丢弃」** —— 这是「零开销」的实现点。

### 3.3 A4 — 容器 / 函数类型最小集

> **状态:已完成(Step 3)。** 验收项按实测修订如下 —— 原文有一条不可达。

| 项 | 内容 |
|----|------|
| **目标** | `ARRAY[T]` / `DICT[K, V]` / `OPTION[T]` / `RESULT[T, E]` / 函数类型的静态表示与边界检查 |
| **挂载点** | `wlwl-types` 类型层加法;复用 `TypeExpr::{Array,Generic}` 解析结果 |
| **变更面** | `wlwl-types/src/ty.rs`(**新增 `satisfies_annotation` 注解边界规则层**)+ `check.rs`(边界判定切层 + `RESULT` 解包族) |
| **兼容** | 运行时嵌套匹配仍可「deliberately deferred」;静态层**不要求**运行时升级 `E0033` 的嵌套能力 |
| **预估人日** | **2–3 人日**(E4);**实测落在下限** |
| **验收(修订)** | ①注解 round-trip 锁测试 ✅;②`ARRAY[INTEGER]` 边界失配诊断(Step 2 已交付,本项未重复做);③泛型形参实例化检查(**归 Step 9**);④**函数类型的源码级检查移出本项** |

**实际交付**:

- **`OPTION[T]` 有了运行时见证**。spec §2.1 的 13 个运行时类型里没有 `OPTION`,
  所以 `Ty::Option` 必须在注解边界上承认两个见证:`NULL`("就是没有")与
  `RESULT[T, ·]`("或者是一个 T",错误侧不要求已知)。**裸 `T` 不通过** ——
  否则 `OPTION` 退化成 `T` 的别名,失去全部意义。
- **`satisfies_annotation` 与 `is_assignable_to` 分成两层**(A4 引入的关键结构):
  前者是**注解边界判定**(多认 `OPTION` 糖),后者是**类型关系**(结构一致 +
  安全数值放宽)。两层不得混淆,否则 `OPTION` 的宽松会漏到别的注解上。
- **`RESULT` 解包族全部接上**:`TRY` / `OR_DIE`(parser 降级出的 `Expr::*`)
  与 `UNWRAP` / `UNWRAP_OR` / `ERR_PAYLOAD`(走通用 `Expr::Call`)都能推出
  载荷类型。**只在另一侧未知(`Dynamic`)时给结论** —— 两侧都具体化时
  `OK`/`ERR` 是运行期分支,静态层无从知走哪一支,落 `Dynamic` 不报。
- **`Ty::Fun` 的逆变 / 协变规则已锁**:形参逆变、返回协变。**注意**:`Dynamic`
  是顶底元,只要任一侧是 `Dynamic`,两个方向都恒真 —— 逆变/协变只能用
  真实的数值 ladder(`INTEGER` / `FLOAT`)见证,拿 `Dynamic` 当例子测不出东西。
- **异构嵌套一律不报**(元素类型不一致 → 统一为 `Dynamic`)。这是「不误报
  优先」的直接后果,已锁测试固定 —— 否则高度嵌套下的异构值会被误报。
- **字典键类型约束(spec §2.1「键限 STRING / INTEGER」)明确不做静态检查** ——
  D-1 只分配了 `E0110`-`E0112` / `W0110`-`W0112` 三个条件,本项无码可用,
  而 §4.1 草案已把 `E0113`-`E0115` 预留给模块签名。**报错前必须先分配码号**,
  否则就违反「凭空分配诊断码是破坏性变更」这条纪律。已用锁测试固定「当前不报」,
  防止后人当成漏项悄悄补上。

> **验收项 ③/④ 的修订理由**:原文的「函数类型 `FUN(INTEGER) -> STRING`
> 参数/返回检查」**在本版不可达**。函数类型的**语法**接入归 **D-5 / Step 9(P1-2)**,
> A4 只负责 `Ty::Fun` 这一 IR 变体上的**结构可赋值规则**。

> **⚠️ 一个必须写进 spec 的静默陷阱(Step 3 实测,与本节上一版判断相反)**
>
> 上一版写「写 `FUN(...) -> T` 今天报 `E0010`」。**Step 3 实测:能 parse,
> 而且不报错。** `parse_type_annotation` 把类型区采集成**平铺 token 列**,而
> `parse_type_expr_from_pieces` 在解析完 `FUN` 头后把剩余 pieces 直接拼成一个
> **`Named`**(`wlwl-parser/src/lib.rs:2378-2390` 的兜底分支)。于是
> `FUN(INTEGER) -> STRING` 落成 `Named { name: "( INTEGER ) - > STRING" }` ——
> **用户看不到任何错误,也得不到任何检查**。
>
> 对照:方括号形式 `FUN[INTEGER, STRING]` **能正常解析**并映射到 `Ty::Fun`
> (形参取全部类型参数,返回 `Dynamic`)。故 §3.1 澄清 2 需修正为:
> **`Ty::Fun` 变体是可从源码到达的(方括号形式),不可达的只是箭头形式**。
>
> 处置:**不在 A4 补检查**(无码可分配;且 `UnresolvedTypeName` 的 `codes()`
> 返回 `None`,报出来会被 CLI 过滤掉,比不报更糟)。改为:(a) 用锁测试固定
> 当前行为;(b) spec v0.10 必须明文声明箭头形式不支持;(c) 归入 **D-5**,
> 待 `->` 终结符真正落地时一并解决。

### 3.4 A2′ — 期望类型向下传播(砍掉 HM)

> **状态:已完成(Step 4)。** 实测结论:本项的收益主要是**推断精度**,
> 而不只是「多抓几个错」—— 原计划把它写成检出能力,是不准确的。

| 项 | 内容 |
|----|------|
| **目标** | 只做**局部、上下文驱动**的期望类型向下传播;未标注处回退 `Dynamic` |
| **挂载点** | `wlwl-types` 内部(check/propagate);**不引入** let-多态、不引入全程序推断 |
| **变更面** | `wlwl-types/src/check.rs`;无 eval 改动 |
| **规则(E4)** | 字面量按期望类型定型;容器字面量元素受 `ARRAY[T]`/`DICT[K, V]` 约束;`IF`/`MATCH` 合流取 lub 或 `Dynamic`;`CALL` 参数按被调签名向下传播 |
| **预估人日** | **2–3 人日**(E4);**实测落在下限** |
| **验收** | 未标注程序在开启模式下不误报;已标注边界失配可报;锁测试覆盖「回退 Dynamic 不诊断」 |
| **实测** | `a2p_container_elements_are_typed_by_the_declared_element_type` / `a2p_call_arguments_receive_the_callee_param_types` / `a2p_lub_joins_numeric_widening_but_never_narrows_dynamic` / `a2p_if_branches_are_joined_instead_of_degrading` / `a2p_still_never_reports_on_unannotated_programs` |

**实际交付**:

- **容器字面量逐元素下推 + 逐元素校验**。有声明元素类型时**按元素报**,
  而不是等整体失配:`LET(x: ARRAY[INTEGER], [1, "a"])` 精确指向 `"a"`。
  校验通过后返回**声明的元素类型**而非统一结果 —— 既让整体检查自动通过
  (同一问题不报两次),又让推断结果不再是笼统的 `Dynamic`。
- **`CALL` 实参按被调签名的形参类型逐位下推**。必须在推导实参**之前**取到
  签名(字面量只有拿到期望才会按期望定型)。无签名的调用(内建 / 未知名字)
  照旧传 `None`。
- **`Ty::lub` 落地**,`IF` / `MATCH` 分支合流从「相同才保留」升级为取上界。
  `MATCH` 的 `default` 也参与合流 —— 它在无子句命中时就是结果值。
- **`lub` 有一条刻意的反向规则**:`Dynamic` 只能被合流**稀释**,绝不能被
  收窄。某一支「不知道」时结论也必须是「不知道」,否则会把未标注分支的
  `Dynamic` 凭空变成具体类型。

> **⚠️ 两处 Step 3 的结论被 A2′ 正确超越,测试随之修订(不是回归)**
>
> 1. **异构容器的静默被切成两半**。无声明元素类型时,异构元素统一为
>    `Dynamic` → 仍静默(「不误报优先」);但**有**声明元素类型时,异构不再
>    被 `Dynamic` 吞掉 —— `ARRAY[INTEGER]` 里放数组会被逐元素报出来。
>    这是新增检出能力。
> 2. **诊断从「整体一条」变成「每元素一条」**。`LET(x: ARRAY[STRING], [1, 2])`
>    从 1 条变 2 条:两个坏元素就是两个问题。定位更准,条数变多,不是重复报告。

> **实测中修掉的一个真回归(已修正,留档)**:第一版实现为了避免误报,把字典
> **键**侧也改成不下推 + 不检查,结果 `LET(x: DICT[STRING, INTEGER], [1, 2])`
> 的键类型失配被漏掉了。折中后定为:**键不接收期望**(下推会把字面量按期望
> 错误定型),但**键类型仍参与整体比较** —— 两头都不牺牲。


### 3.5 A6' — 注册表结构化签名(**分批**)

> **状态:首批完成(Step 5)。** 首批实做 **60 条**,落在计划给的 40–60 上沿。

| 项 | 内容 |
|----|------|
| **目标** | `BuiltinSpec.signature` 由文档串升级为**结构化**(保留文档列) |
| **挂载点** | `wlwl-eval/src/registry.rs` 的 `BuiltinSpec` |
| **变更面** | `BuiltinSpec` 增加 `sig: Option<BuiltinSig>`;**保留** `signature: &'static str` 文档串;`wlwl-types` 加 `check_program_with_builtins`;`wlwl-cli` 做 `SigTy` 到 `Ty` 的映射 |
| **分批策略** | **首批 = 60 条**:算术 / 比较 / STRING / ARRAY / DICT / Conv / 控制流布尔算子 / Format / Ctor / Io。其余 **50 条显式 `None`** |
| **预估人日** | **3–4 人日**(首批);后续批次滚动(E4) |
| **验收** | 首批条目锁测试;附录 G 重生成无 diff 回归;未结构化条目行为不变 |
| **实测** | `builtin_sig_batch1_*`(5 项)+ `appendix_g_regen_is_stable_and_ignores_the_structured_field` + CLI 端到端 5 项 |

**实际交付 —— 三个由实测决定的设计**:

1. **首批只给返回类型,一律不带形参**。spec 附录 G 大量条目有**可选形参**
   (`SUB(s, start, end?)`、`POP(d, k, default)`)与**变长实参**
   (`PRINT(args...)`),任何「精确元数」表示都会在这些条目上误报,与
   「不误报优先」直接冲突。故 `BuiltinSig.params` 做成
   `Option<&'static [SigTy]>`,首批**全部为 `None`** = 不做元数检查、
   不检查形参类型。锁测试 `builtin_sig_params_are_always_unknown_in_batch1`
   死守这一点,防止后人偷偷带上形参。
2. **容器不带类型参数**。`ARRAY` / `DICT` 的元素与键值类型一律 `Dynamic` ——
   嵌套泛型与数组元素匹配在运行时 `E0033` 那条路径上本来就是
   *deliberately deferred*(`wlwl-eval/src/lib.rs:8588`),静态层首批
   **不比运行时更激进**。
3. **分层靠 CLI 而非新 crate**。内建签名住在 `wlwl-eval` 的注册表,
   而 ADR-0020 Decision 1 规定 `wlwl-eval` 不依赖 `wlwl-types`、
   反之亦然。映射只能落在**同时依赖两者**的 `wlwl-cli` 那一层;
   `wlwl-types` 侧只接受**已转成 `Ty` 的表**。附带一条语义:
   **局部绑定优先于内建表** —— `allow_builtin_shadow` 场景下用户定义赢,
   否则会对用户的定义报错。

**为什么这 50 条留 `None` 而不填**(锁测试 `unstructured_entries_stay_none_for_a_known_reason`
逐条钉住名单,防止「漏了」被当成 bug 顺手补上):

| 分组 | 条数 | 理由 |
|------|------|------|
| Control 流程形 + Result 宏 | 19 | 由 parser 降级成 `Expr::*`(`IF`/`WHILE`/`FOR`/`MATCH`/`RETURN`/`BREAK`/`CONTINUE`/`IS_OK`/`IS_ERR`/`TRY`/`OR_DIE`/`UNWRAP`/`UNWRAP_OR`/`ERR_PAYLOAD`/`OK`/`ERR`/`PANIC`/`WRAP`/`EXPECT_ERR`),**根本不走 `Expr::Call`**,注册表签名对静态层无增益;其中解包族已由 A4 在 `check_call` 特判覆盖 |
| Concurrent | 17 | 整组留给后续批次;并发语义本版不深挖(§5 明确) |
| Subscript / Module / Oop / Property | 13 | 留给后续批次;且 `GET_PROP` / `CALL_METHOD` / `AT_K` 类本身是取值透传,填了也是 `Dynamic` |

**`gen_appendix_g` 未改动,且这是有意的**:结构化字段是**加法**,生成器读的
仍是 `signature` 文档串,故附录 G 逐字节不变。要在附录 G 里增列结构化
签名是 **Step 12(spec 派生)** 的事。锁测试
`appendix_g_regen_is_stable_and_ignores_the_structured_field` 守住
「加 60 条结构化签名不得改变附录 G 一个字节」。

> **风险处置记录**:计划 §3.5 预置了「若首批超 **5 人日**,削减首批条目数」。
> **未触发** —— 实做落在 3–4 人日预算内,60 条全量保留。降级路径仍然有效:
> 后续批次若遇同样压力,按上表「整组留 `None`」的粒度削,不动首批。


### 3.6 P0-1 立项单汇总

| 子项 | 挂载点 | 变更面 | 人日 | 验收命令 |
|------|--------|--------|------|---------|
| A1 类型 IR/环境 | `wlwl-ast` 注解槽位 → 新 crate | `wlwl-types/` + workspace | 3–4 | `cargo test -p wlwl-types` |
| A3 gradual_typing | Check / run_file | CLI + manifest 只读键 + check.rs | 3–4 | `cargo test --workspace`(off);夹具(on) |
| A4 容器/函数类型 | 类型层加法 | `ty.rs` / `check.rs` | 2–3 | round-trip + 边界锁测试 |
| A2′ 向下传播 | `wlwl-types` 内部 | `check.rs` | 2–3 | 误报锁测试 + 失配锁测试 |
| A6′ 注册表结构化 | `registry.rs` BuiltinSpec | registry + gen_appendix_g | 3–4 | 首批签名锁测试 + 附录 G |
| **P0-1 小计** | | | **13–18 人日(≈2.5–3.5 人周)** | |

---

## 4 P0-2 · 模块契约(Pillar C1–C3 + C5′;C4 已否决)

> **注意**:类型契约在 `ModuleLoader` 边界验证即可,**不依赖包管理器**(否决 Qwen 推论)。`wlwl-toml` 无任何扩展。

### 4.1 C1 — 模块签名

| 项 | 内容 |
|----|------|
| **目标** | 接口清单:导出类型、函数、(可选)效果/能力声明 |
| **挂载点** | `ModuleLoader` 边界(`wlwl-eval/src/lib.rs:797-871`);加载后 EXPORT 集合比对 |
| **签名载体** | **D-2 已定**:可选旁路文件 `foo.wll.sig`(与源同目录)。**「无签名 = v0.9 行为」因此是结构性保证,不是约定** —— 没有任何内嵌语法,不存在「忘了写签名标记」这回事 |
| **签名内容** | 导出名 + 可选类型注解(复用 `TypeExpr`)+ 可选能力/效果标签(仅记录,v0.10 不强制) |
| **校验** | 编译期(check 开启时):实现 EXPORT ⊆ 签名(或多出未声明 → 诊断);签名声明但未导出 → 诊断 |
| **变更面** | `ModuleLoader` 加载路径增加签名发现/解析/比对;`wlwl-types` 提供签名中的类型解析;**eval 求值语义不变**(校验可在 check 层完成;run 时可选强制) |
| **预估人日** | **3–4 人日**(E4) |
| **验收** | 签名不匹配有错误码;无签名模块行为与 v0.9 一致(回归) |

**诊断码草案**(决策点 D-1 同池):

| 条件 | 码 |
|------|----|
| 导出名不在签名中 | `E0113` module signature extra export |
| 签名声明未导出 | `E0114` module signature missing export |
| 签名类型与实现注解冲突 | `E0115` module signature type mismatch |

### 4.2 C2 — 可见性 / `SEALED`

| 项 | 内容 |
|----|------|
| **目标** | `PRIVATE` / 显式 `EXPORT`;`SEALED MODULE` 禁止外部触达非导出符号 |
| **挂载点** | 解析器(`wlwl-parser`)识别 `SEALED` 模块头 / 前置 `SEALED(...)` 声明 + 模块加载。**D-3 已定**:走**前缀调用 / 模块头声明**形式,与既有 `CLASS(...)` / `EXPORT([...])` 同构 |
| **现状** | `EXPORT(["a","b"])` 声明导出集合;`collect_exports`(`lib.rs:1215-1224`);**无 SEALED、无 PRIVATE 关键字**;导入未导出名 → `E0023`(未绑定) |
| **规则** | 未 `EXPORT` 的绑定默认**模块私有**(与现行为一致);`SEALED MODULE` 额外禁止:外部通过任何途径(包括 `MODULE_REF` 动态面,若存在)触达非导出符号 → 诊断 |
| **变更面** | parser 新增前缀构造(**加法**);`ModuleLoader` 加载时记录 sealed 标记;导入边界检查 |
| **词法面影响(实测修订)** | **§1.4 关键字表不变、§12 保留形式继续留空** —— 这是走前缀调用而非保留字的直接收益。原计划写「parser 关键字/头部语法」暗示要动关键字表,现已排除。仍需**新 AST 节点 + 新错误码** |
| **预估人日** | **2–3 人日**(E4) |
| **验收** | 越界访问可诊断;非 SEALED 模块行为不变 |

### 4.3 C3 — 签名校验 + `sig-gen`

| 项 | 内容 |
|----|------|
| **目标** | 编译期「实现满足签名」;自动生成签名骨架 |
| **挂载点** | 与 `wlwl ast --json` 同风格 CLI:`wlwl sig-gen <file>` / `wlwl sig <file>` |
| **变更面** | `wlwl-cli/src/main.rs` 子命令 +1~2;复用 C1 签名 IO;生成骨架**可 parse/check** |
| **预估人日** | **3–4 人日**(E4) |
| **验收** | `sig-gen` 产物可被 `check` 通过;桩测试锁 JSON/文本格式 |

### 4.4 C5′ — std 命名整理(**仅命名/文档**)

| 项 | 内容 |
|----|------|
| **目标** | 仅命名/文档整理(分层已存在);**无行为变化** |
| **挂载点** | `wlwl-std` 文档注释 / README / 附录表 |
| **变更面** | docs only(或纯 rename 的 `#[doc]`);**禁止**改函数语义/导出名 |
| **预估人日** | **1 人日**(E4) |
| **验收** | 锁测试全绿;`git diff` 无 `wlwl-std` 语义改动 |

### 4.5 P0-2 立项单汇总

| 子项 | 挂载点 | 变更面 | 人日 | 验收命令 |
|------|--------|--------|------|---------|
| C1 模块签名 | ModuleLoader | loader + types + 可选 sig 文件 | 3–4 | 签名夹具 + 回归 |
| C2 可见性/SEALED | parser + loader | 加法语法 + 边界检查 | 2–3 | 越界诊断夹具 |
| C3 校验 + sig-gen | CLI | 子命令 + 签名 IO | 3–4 | `sig-gen` 产物可 check |
| C5′ std 命名 | wlwl-std docs | 无行为 | 1 | 全绿 + diff 评审 |
| **P0-2 小计** | | | **9–12 人日(≈2 人周)** | |

---

## 5 P1 — 本版应做

### 5.1 P1-1 · MATCH 穷尽性 / 冗余检测

| 项 | 内容 |
|----|------|
| **目标** | 在现有 **6 种 `Pattern`** 上实现 usefulness / Maranget;缺失模式列出;不可达子句警告 |
| **挂载点** | MATCH 求值前的静态 pass(`wlwl-types` 或 `wlwl-types` 子模块);`Pattern::{Ident,Wildcard,Literal,Array,Dict,Constructor}`;`Constructor` **仅 OK/ERR** |
| **算法范围** | 无 or-pattern / guard / ref pattern — 形态集窄,属小型算法(《路线》成本可控结论**成立并上调**) |
| **诊断** | 非穷尽 → `E0116`(error) / `W0116`(warn);不可达子句 → `W0117`(恒警告,不因 error 模式升级为硬错,避免误伤渐进代码) |
| **配置** | `gradual_typing` 开启时启用;子开关可选 `match_exhaustiveness = "off" \| "warn" \| "error"`(默认随 `gradual_typing`) |
| **预估人日** | **3–5 人日**(E4) |
| **验收** | 缺失模式列出具体构造;不可达子句警告;锁用例 ≥ 8;默认关不报 |

**明确不做(本版)**:嵌套 or-pattern、guard 收窄、与流敏感类型的耦合(推迟)。

### 5.2 P1-2 · 泛型限形(唯一允许的深水区)

| 项 | 内容 |
|----|------|
| **目标** | 参数化容器/函数:`ARRAY[T]`、函数类型等;**运行时擦除**。**决策 D-5 已定为「最小显式约束」**,故本项含「类型产生式加 `:` 与约束名」这一**语法面**改动 |
| **约束** | 无完整 trait / typeclass 推断;无 monomorphization;**`Value` 模型不变** |
| **轻量约束** | **D-5 已定**:最小显式约束 `T: Comparable` 级,不做隐式解析;不引入 trait 求解器 |
| **语法代价(实测定位)** | `parse_braced` 当前只接受 `,` 与 `]`,类型产生式里出现 `:` 报 `E0012`。要支持 `T: Comparable` 必须改 `parse_braced`,**这是 v0.10 唯一一处真正的语法面扩张**,须在 §12 出口条件里单列 |
| **挂载点** | 类型层(`wlwl-types`);注册表结构化签名中的参数化类型;**不改** `wlwl-eval::Value` 变体 |
| **编译期** | 实例化错误(如 `ARRAY[INTEGER]` 与 `ARRAY[STRING]` 误用)在静态层报;运行时仍擦除 |
| **预估人日** | **4–6 人日**(E4) |
| **验收** | 编译期实例化错误锁测试;`Value` 快照/行为不变;默认关零影响 |

**禁令**:不改 `Value`;不引入 monomorphization;不做高阶 typeclass。若超 **6 人日**,降级为「仅容器参数化、函数泛型推迟到 v0.10.1」。

### 5.3 P1-3 · 工具链薄壳

| 子项 | 内容 | 挂载点 | 人日 | 验收 |
|------|------|--------|------|------|
| **check 语义化** | ~~`Check` 接静态 pass;分层测试~~ | `main.rs:51-52,85-88,630` | **0(已由 Step 2 / A3 交付)** | ✅ `check_is_layered_parse_static_run`;`default_off_zero_diag` 锁「默认行为不变」 |
| **`wlwl lsp` 薄壳** | 包装 parser + error + 静态诊断:diagnostics / definition / hover;补全由注册表驱动 | CLI 新子命令;复用 `wlwl-types` 诊断 | 2–3 | LSP 集成冒烟;无独立 LSP server 进程框架依赖(薄壳) |
| **interface / schema JSON** | 已有 `ast --json` 顺增量:`interface` JSON(导出面)、`schema` JSON(类型) | `main.rs` 导出路径 | 1–2 | **JSON schema 锁测试** |
| **P1-3 小计** | | | **4–6 人日** | |

> **性质**:Compiler Feedback API 在此处以低成本落地(`ast --json` 已有先例),服务 AI 工具链,**不做 AI 专用语法**。

### 5.4 P1 立项单汇总

| 子项 | 挂载点 | 变更面 | 人日 | 验收命令 |
|------|--------|--------|------|---------|
| P1-1 MATCH 穷尽 | Pattern 6 形态 | `wlwl-types` 穷尽 pass | 3–5 | 穷尽/不可达锁用例 |
| P1-2 泛型限形 | 类型层,Value 不变 | `ty.rs`/check/registry sig | 4–6 | 实例化错误 + Value 不变 |
| P1-3 工具链薄壳 | Check / lsp / json | CLI + types | 4–6 | 分层测试 + LSP 冒烟 + JSON schema 锁 |
| **P1 小计** | | | **11–17 人日(≈2–3.5 人周)** | |

---

## 6 ADR 与规范派生

### 6.1 ADR-0020 — *Gradual Static Contracts*

> **状态:已定稿并 Accepted(commit `c535818`)。**
> 权威文本是 `docs/adr/0020-gradual-static-contracts.md`;本节只是摘要,
> 两者冲突时**以 ADR 文件为准**。ADR 的 Ratification Status 节记录了
> Step 1/2 的实测结论。

| | |
|---|---|
| **Status** | Accepted(2026-09-27) |
| **Date** | v0.10 立项时 |
| **Deciders** | Li (project lead) |
| **Related** | ADR-0010(局部推翻)、ADR-0011(人日纪律)、《路线》§4.1 |

**Context**:ADR-0010 否决「全程序类型推断」,选运行时 transient cast。类型注解 AST 槽位已存在但语义为空;`check` 只 parse。六方一致认为类型是最大缺口。

**Decision**:
1. 新建独立 crate `wlwl-types` 静态 pass,**不侵入** `wlwl-eval`;
2. 范围限缩为「**边界契约检查 + 期望类型向下传播**」,**不是** ADR-0010 否决的 HM / 全程序推断;
3. `gradual_typing = off \| warn \| error`,**默认 off**;运行时 `E0033`/`strict_types` **保留**;
4. 人日预算:P0-1 13–18 人日;超支削减 A6′ 首批与 A2′ 传播深度,**不延期** `gradual_typing` 主开关;
5. 包管理零动作。

**Consequences**:
- 有意推翻 ADR-0010 的「不做静态检查」方向,**仅限上述范围**;ADR-0010 的运行时 transient cast 决策**继续有效**;
- v0.9 程序默认行为不变(S1/S3);
- 后续类型收窄/Variant/shallow 效果不在本 ADR 授权范围。

### 6.2 spec v0.10 派生范围(预告)

| 章节 | 动作 |
|------|------|
| **§1 / §5.2(新增产生式)** | **v0.9 spec 全文从未定义 `Type` 产生式** —— §5.2 只写了 `name: Type` 这个元符号,§5.1 的 `Function` 产生式连返回值注解都没收录。即类型注解文法**只存在于实现,不是规范**。本版必须第一次把它写进 spec。**照实写出三条实测事实**:(a) 容器类型用**方括号**;(b) parser 对 `ARRAY` 特判强制要求方括号,裸 `ARRAY` 报 `E0010`,而 `DICT`/`OPTION`/`RESULT` 裸写能解析成 `Ident` —— **这条不对称必须写明**;(c) 函数类型 `FUN(...) -> U` 本版**仍未进语法**(见 D-5) |
| §2.4 类型注解 | 增补「静态解释」小节(默认不启用) |
| §2.7 `strict_types` | 明文区分**运行时** `strict_types` 与**编译期** `gradual_typing` |
| §7.6 MATCH | 增补穷尽性/不可达诊断(可选开启) |
| §13 / IMPORT-EXPORT | 模块签名、可见性、`SEALED`(**D-2 / D-3 已定**:旁路 `*.wll.sig` + 前缀调用/模块头声明,故 `SEALED` **不进 §1.4 关键字表**,§12 保留形式继续留空) |
| §11.2 错误/警告码 | 注册 `E0110-E0112` / `W0110-W0112` 静态契约段(**D-1 已定,Step 2 已落地**) |
| §2.7 / §9.4 | `gradual_typing` 进 §9.4 清单特性表,并**明文区分于运行时 `strict_types`**;同时声明 A3 的覆盖率边界(抓不到内建调用) |
| 新附录(或 §附) | `gradual_typing` / 签名文件格式;`interface`/`schema` JSON schema |
| 附录 G | `gen-appendix-g` 重生成(A6′ 结构化签名) |

**本版预期零可观察变化**(纯加法 + 默认关);CHANGELOG 沿用「可观察变化逐条列明」文化。

---

## 7 P2 — 余力项(不阻塞发版)

| 项 | 内容 | 约束 | 人日(E4) | 验收 |
|----|------|------|----------|------|
| **标准库加法** | `BYTES` / `TIME` / `MATH` / `RANDOM`(`REGEX` 视实现依赖再定) | 纯加法;`wlwl-std` 独立文件;**不碰求值核心** | 3–5 | 新模块锁测试;既有全绿 |
| **Known limitations 收口(S4)** | `Effect::MethodCall` / `ProtocolViolation` **真正 raise 或规范降级** | 悬空承诺清零 | 2–3 | CHANGELOG Known limitations 条目消除或改写 |
| **能力沙盒旁路原型** | 默认路径零变化;至多实验开关 | **不进 P0**;std 调用面大 | 2–3 | 默认关回归 |

**S4 收口决策点(D-4)**:
- **路径 A**:CALL_METHOD 路径真正 raise `Effect::MethodCall`,协议错 raise `ProtocolViolation`,接入 `Scheduler::step` 效果循环;
- **路径 B**:规范明文降级 — 声明二者为「保留命名,不承诺 handler 语义」,从 Known limitations 移到规范保留段。
- **默认**:先评估 A 的真实挂点成本;若 **> 3 人日**,走 B 并在 ADR-0020 附注。

---

## 8 明确不做(v0.10.0)

| 项 | 理由 |
|----|------|
| **包管理器一切扩展**(PubGrub / 注册表 / semver 骨架 / 新依赖字段 / 远程源) | 业主硬约束 + ADR-0011 + `wlwl-toml` 冻结 |
| **宏 / 用户元编程** | 注册表 24 条宏构造已是稳定语法面;再开用户宏 = 把 AST 变 ABI |
| **Variant / Open Row 新值形态** | `Pattern::Constructor` + 运行时 `Value` + 穷尽规则三联动;先原型 |
| **类型收窄 / Guard 语法预留** | 范围失控 / 污染稳定面 |
| **用户代数效果处理器**(含 shallow) | 先收口 `Effect::MethodCall` 半落地;否则半落地叠半落地 |
| **ownership / lifetime / borrow checker** | 「不要把 WLWL 变成 Rust」;只可在类型层预留 `Unique`/`Resource`/`Capability` **名字** |
| **Wasm/字节码后端交付** | 只许设计文档;workspace 无后端 crate |
| **会话 `&`/`par`、多方会话、多线程** | ADR-0016 + 六方一致 + 用户禁止深挖并发 |
| **调试器全量 / HAMT / 依赖类型 / AI 专用语法** | 单家或研究项,不进本版 |
| **A2 全量局部推导 / HM / let-多态** | 只保留期望类型向下传播 |
| **A5 流敏感类型收窄** | 与 MATCH/IF 分支分析耦合,范围失控 |
| **110 条注册表一次结构化** | A6′ 分批;超支先砍批次 |

**v0.10.1+ 候选**(未承诺):类型收窄、Variant 原型、shallow 效果(仅当 P2 收口完成)、`defer`、宏评估、函数泛型(若 P1-2 降级)。

**包管理器**:不进语言版本路线;既有 `wlwl-toml` 冻结。

---

## 9 兼容性承诺(对外句式)

> **任何 v0.9 程序,在 v0.10.0 上默认可观察行为不变。**
>
> 例外:无。
>
> 新增编译期诊断仅在 `gradual_typing` / 模块签名校验等**显式开启**后出现;运行时 `strict_types`/`E0033` 路径不变;`wlwl-toml` 及 `E0041`/`E0045` 触发语义不变。

配套工程动作:

1. 既有 ≈1518 测试全绿(**S1**);
2. 运行时 `E0033` 测试锁保持(**S3**);
3. CHANGELOG 本版预期**零可观察变化**(纯加法 + 默认关);
4. `wlwl-toml` diff 评审零功能扩展(**S5**)。

---

## 10 实施顺序

### 10.1 阶段切分

```
[Step 0] 本计划 + ADR-0020 定稿(用户审阅)          ✅ 完成 c535818
   ├─ 决策点 D-1..D-6 拍板 → 实际 D-1/D-2/D-3/D-5 已定
   │  (D-4 / D-6 未拍板,分别只卡 Step 11 余力项与 Step 10)
   └─ 动手改 impl 前锁定范围,避免返工
[Step 1] impl:wlwl-types 骨架(A1)                  ✅ 完成 859400b
   ├─ 新 crate + workspace 接线
   ├─ TypeExpr → 静态类型 IR;Dynamic 回退
   └─ 锁测试:type_ir_roundtrip / dynamic_fallback
[Step 2] impl:gradual_typing 开关 + check 接线(A3)   ✅ 完成 3442c9a
   ├─ manifest 只读键 gradual_typing = off|warn|error
   ├─ Check: parse → 可选静态 check
   ├─ 诊断码 E0110-E0112 / W0110-W0112 注册
   └─ 锁测试:default_off_zero_diag / warn_mode_soft / error_mode_hard
[Step 3] impl:容器/函数类型最小集(A4)             ✅ 完成
   ├─ ARRAY[T] / DICT[K, V] / OPTION[T] / RESULT[T, E]
   │  + Ty::Fun 的结构可赋值规则(含形参逆变)
   ├─ 新增 satisfies_annotation 注解边界规则层(OPTION 糖)
   ├─ RESULT 解包族:TRY / OR_DIE / UNWRAP / UNWRAP_OR / ERR_PAYLOAD
   ├─ 注意:OPTION[T] 是注解糖,无运行时对应类型(spec §2.1 无 OPTION)
   ├─ 注意:函数类型的**箭头**语法接入不在本项(见 §3.3 陷阱说明 + D-5);
   │  方括号形式本就可解析
   └─ 锁测试:OPTION 双向语义 / 解包族 / Fun 逆变 / 异构嵌套不报
      (container 边界失配已由 Step 2 交付)
[Step 4] impl:期望类型向下传播(A2′)             ✅ 完成
   ├─ 容器字面量逐元素下推 + 逐元素校验
   ├─ CALL 实参按被调签名逐位下推
   ├─ Ty::lub:IF / MATCH 分支合流(Dynamic 只稀释不收窄)
   └─ 锁测试:传播精度 / 新增检出 / lub 方向 / 不误报
      (异构容器静默与「每元素一条」的性质变化见 §3.4)
[Step 5] impl:注册表结构化签名首批(A6′)          ✅ 完成
   ├─ BuiltinSpec 增加 sig 字段(保留文档串不动)
   ├─ 首批 60 条:算术/比较/STRING/ARRAY/DICT/Conv
   │  /控制流布尔算子/Format/Ctor/Io;其余 50 条显式 None
   ├─ 首批只给返回类型,不带形参(可选形参/变长实参会误报)
   ├─ SigTy -> Ty 映射落在 wlwl-cli(保住 ADR-0020 分层)
   ├─ gen_appendix_g 未改动:结构化字段是加法,附录 G 逐字节不变
   └─ 锁测试:builtin_sig_batch1_*(5) / appendix_g_regen_stable
      / CLI 端到端 5 项(生效/未覆盖静默/遮蔽优先/不查元数/off 不建表)
[Step 6] impl:模块签名 C1 + 可见性/SEALED C2
   ├─ 可选签名文件解析
   ├─ ModuleLoader 边界比对
   ├─ SEALED 模块头(加法语法)
   └─ 锁测试:sig_mismatch_e0113_e0115 / sealed_violation / no_sig_behaves_as_v09
[Step 7] impl:sig-gen / sig 校验 CLI(C3) + std 命名整理(C5′)
   ├─ wlwl sig-gen / wlwl sig
   └─ 锁测试:sig_gen_roundtrip_parse_check
[Step 8] impl:MATCH 穷尽性 / 冗余(P1-1)
   ├─ 6 形态 usefulness / Maranget
   ├─ 诊断 E0116 / W0116 / W0117
   └─ 锁测试:match_missing_ctor / match_unreachable_clause ≥ 8
[Step 9] impl:泛型限形擦除(P1-2)
   ├─ 参数化容器/函数;显式约束(可选)
   ├─ Value 模型零改动
   └─ 锁测试:generic_instance_error / value_model_unchanged
[Step 10] impl:工具链薄壳(P1-3)
   ├─ check 分层测试
   ├─ wlwl lsp 薄壳(diagnostics/definition/hover)
   ├─ interface / schema JSON
   └─ 锁测试:json_schema_lock / lsp_smoke
[Step 11] (余力) P2:std 加法 / 效果收口 S4 / 能力旁路
   └─ 仅当 P0/P1 出口条件已满足
[Step 12] spec v0.10 派生
   ├─ §2.4 / §2.7 / §7.6 / §13 / §11.2 / 新附录
   ├─ 附录 G 重生成
   └─ 锁测试:appendix_g_anchors + 错误码表一致
[Step 13] deviations-v0.10 启动(D10-NNN 流水)
[Step 14] CHANGELOG v0.10.0 段 + README 一致性表
[Step 15] end-to-end 验收(§11)
```

**依赖关系**:Step 1 → 2 → (3 ∥ 4) → 5;Step 6 可与 3–5 并行(依赖 Step 1 的类型 IR);Step 8–10 依赖 Step 2 的诊断管道。Step 11 仅余力。

### 10.2 决策点(2026-09-27 用户拍板:D-1 / D-2 / D-3 / D-5 已定)

| 决策点 | **裁决** | 选项 | 状态 |
|--------|------|------|------|
| **D-1 静态诊断码段** | **A:新建 `E0110+` / `W0110+` 静态契约段** | A:新段;B:复用 E0030-E0039 旁挂;C:仅 W 码 | ✅ 已定(Step 2 落地) |
| **D-2 签名文件载体** | **A:旁路 `*.wll.sig` 文本(与源文件同目录)** | A:旁路文件;B:源内 `SIG(...)` 构造;C:两者 | ✅ 已定(卡 Step 6) |
| **D-3 SEALED 语法** | **A:前缀调用 / 模块头 `SEALED(...)` 声明(加法)** | A:模块头声明;B:`EXPORT(..., sealed)` 旗标;C:推迟到 v0.10.1 | ✅ 已定(卡 Step 6) |
| **D-4 Effect 半落地收口** | (未拍板) | A:真 raise;B:规范降级;C:维持 Known limitations | ⏳ 待批(仅卡 Step 11 余力项) |
| **D-5 泛型约束深度** | **A:最小显式约束 `T: Comparable` 级** | A:最小显式约束;B:纯参数化无约束;C:推迟函数泛型 | ✅ 已定(卡 Step 9) |
| **D-6 LSP 薄壳范围** | (未拍板) | A:最小三项;B:加 rename/format;C:不做 lsp 只做 JSON | ⏳ 待批(卡 Step 10) |

**D-1 裁决依据(实测,非推测)**:裁决当日的错误码占用为 E 码 67 个(止于 `E0102`,
外加 `E1003` 逃逸码)、W 码 15 个(止于 `W0066`)。`E0030`-`E0039` 这 10 个
**已 10/10 占满**且语义全部绑在运行时(`E0030` type error、`E0031` 键类型、
`E0033` strict_types、`E0034`/`E0035` 数值溢出、`E0036`/`E0037` 索引、
`E0038` RANGE、`E0039` FORMAT),复用会让同一码号随 `gradual_typing` 开关
含义漂移,§12.2 的「运行时码不变」承诺将无法验证。`E0110+` / `W0110+` 是完全
空档,一个码号只对应一种条件。

> **D-3 附带效果**:走前缀调用 / 模块头声明意味着 `SEALED` **不进入 §1.4
> 关键字表**,§12 保留形式(v0.9 为空表)继续留空 —— 词法面零扩张。
> **D-2 附带效果**:`*.wll.sig` 是旁路文件,「无签名 = v0.9 行为」因此是
> 结构性保证,不是约定。

### 10.3 锁测试预期增量(**Step 0–2 已实测,其余仍为估计**)

| 时点 | 实测总数 | 增量 |
|------|---------|------|
| v0.9.0 基线 | 1517 / 0 failed | — |
| Step 1 收尾 | 1536 / 0 | +19(A1) |
| Step 2 收尾 | **1568 / 0** | **+32**(A3:wlwl-types 20 + wlwl-toml 6 + wlwl-cli 6) |

> 目标 ≈1600±30 目前看**偏保守但仍合理** —— P0-1 剩余的 A2′ / A4 / A6′
> 尚未贡献,故不必下调。**不虚报**:每项以实测为准。

- v0.9.0 基线 ≈**1517 passed** + 0 failed(保留,只增不减)
- 增量(估,E4):
  - A1 ~5(类型 IR / Dynamic)
  - A3 ~8(off 零诊断 / warn / error / E0033 并存)
  - A4 ~6(容器/函数边界)
  - A2′ ~5(传播 + 无误报)
  - A6′ ~8(首批签名 + 附录 G)
  - C1/C2/C3 ~12(签名/SEALED/sig-gen)
  - P1-1 ~10(穷尽/不可达)
  - P1-2 ~6(泛型实例化 + Value 不变)
  - P1-3 ~8(check 分层 / JSON schema / LSP 冒烟)
  - conformance `.wll` 夹具 ~15–25(新目录)
- **合计估:~80–90 项**
- **目标:≈1600±30 passed**(以 §11 实测为准,不虚报)

### 10.4 conformance 目录规划

| 目录 | 内容 |
|------|------|
| `impl/tests/conformance/static_types/` | 注解匹配/失配、容器类型、函数签名、Dynamic 回退 |
| `impl/tests/conformance/module_sig/` | 签名匹配/违例、SEALED、无签名回归 |
| `impl/tests/conformance/match_exh/` | 穷尽/非穷尽/不可达子句 |
| `impl/tests/conformance/generics/` | 参数化实例化错误、擦除语义 |
| 既有 `impl/tests/conformance/` | 保持;新目录**并列**,不改旧夹具 |

---

## 11 容量、风险与人日

### 11.1 总人日预算

| 块 | 人日(E4) | 降级路径 |
|----|----------|---------|
| P0-1 静态类型 pass | 13–18 | 削 A6′ 首批条目;A2′ 只做字面量+CALL |
| P0-2 模块契约 | 9–12 | C3 sig-gen 可后置到 patch;C5′ 顺延 |
| P1-1 MATCH | 3–5 | 仅非穷尽,不做不可达 |
| P1-2 泛型 | 4–6 | 仅容器参数化 |
| P1-3 工具链 | 4–6 | 不做 lsp,只做 JSON(降 C) |
| 文档/spec/ADR/验收 | 3–5 | — |
| **P0+P1 合计** | **36–52 人日(≈7–10.5 人周)** | 超上限则砍 P1-2/P1-3 下限 |
| P2 余力 | 7–11 | 不阻塞发版 |
| **含 P2** | **43–63 人日** | |

**人日纪律**:每一项实施 commit 的 PR/提交说明须回链立项单行(挂载点/人日/验收)。**写不出人日的一律降级**(ADR-0011 先例)。

### 11.2 风险清单

| 风险 | 等级 | 缓解 | 回退路径 |
|------|------|------|---------|
| 重开 ADR-0010 被视为推翻项目纪律 | 中 | **ADR-0020** 明文范围限缩 + 人日预算;CHANGELOG 同步 | 维持 ADR-0010,静态层只做 warn |
| 静态 pass 侵入 eval | 高 | 强制 `wlwl-types` 独立 crate;eval 只保留 `E0033` | 代码评审门禁;发现侵入即 revert |
| 注册表结构化工作量被低估 | 中 | A6′ 分批;其余 `Dynamic` | 削减首批;文档串保持权威 |
| 模块签名破坏现有 IMPORT 行为 | 中 | **签名文件可选**;无签名 = 现行为 | 全局关闭签名校验 |
| 范围蠕变(效果/宏/包管理回潮) | 中 | §8「明确不做」清单作**评审门禁** | PR 拒收 |
| 测试锁不足 | 中 | 新增 conformance 目录;JSON schema 锁测试 | 出口条件不满足不发版 |
| 泛型/穷尽算法延期 | 低-中 | P1 可降级;P0 不依赖 | 砍 P1 深度保 P0 |
| `gradual_typing` 误报损害信任 | 中 | `Dynamic` 回退宽松;warn 先于 error;无误报锁测试 | 默认退回 off |
| manifest 新键被误认为包管理扩展 | 低 | **只读** `[features] gradual_typing`;diff 评审 S5 | 移到独立 config 文件 |

### 11.3 成功标准(S1–S6,全部 E1 可验)

| # | 标准 | 验收方式 |
|---|------|---------|
| **S1** | **默认零破坏**:`gradual_typing` 等默认关闭时,现有 ≈1518 测试全绿 | `cargo test --workspace` |
| **S2** | **开启可抓错**:注解失配、模块签名违例、MATCH 非穷尽/不可达可稳定诊断 | 新增 conformance 夹具 + 错误码锁测试 |
| **S3** | **运行时兼容**:`strict_types` / `E0033` 路径测试保持绿 | 既有 `lib.rs` 测试锁 |
| **S4** | **悬空承诺清零**:`Effect::MethodCall`/`ProtocolViolation` 真正 raise **或** 规范明文降级 | CHANGELOG Known limitations 条目消除或改写 |
| **S5** | **无包管理动作**:`wlwl-toml` 无字段/算法/生态面扩展 | diff 评审 |
| **S6** | **人日可核**:每项立项写明挂载点与预估人日 | 立项单(§3.6/§4.5/§5.4) |

---

## 12 验收

### 12.1 自动验收

```
cargo test --workspace     # 0 failed;≈1517+ 增量(目标 ≈1600±30)
cargo fmt --check          # 0 diff
cargo clippy --locked --workspace --all-targets -- -D warnings  # 0 warnings
cargo doc --workspace      # 0 warnings
# 默认关闭回归(必须在 clean tree 与开启夹具各跑一遍)
cargo test --workspace --test conformance
```

**开启模式专项**(夹具工程内固定 `gradual_typing`):

```
# 静态契约 conformance(新目录)
cargo test -p wlwl-cli --test conformance_static
cargo test -p wlwl-types
```

### 12.2 一致性表(README §0.4 / CHANGELOG)

| 一致性项 | v0.9.0 | v0.10.0 目标 |
|---------|--------|-------------|
| 错误码 | 61 激活 + 6 保留(共 67) | **67 激活 + 6 保留(共 73)** —— D-1 已定并落地:`+E0110` 注解失配 / `+E0111` 调用失配 / `+E0112` 返回失配。运行时码**逐个不变**;包管理码零新增。⚠️ 数字变化本身**不违反兼容承诺**:新增码号对 v0.9 程序不可观察(默认 `off`,且 `off` 档不调用 checker) |
| 警告码 | 15(止于 W0066) | **18** —— `+W0110` / `+W0111` / `+W0112`(同号 `warn` 档) |
| 内建数 | 110 | **110**(零新增;签名结构化不改内建集合) |
| `wlwl-toml` | manifest/lock/MVS | **冻结**;至多 `[features] gradual_typing` 只读键 —— **已落地且零 schema 变更**:`[features]` 本就是不透明 `BTreeMap<String, toml::Value>`(`manifest.rs:33-37`),加键不需要动结构 |
| spec 章节 | §1–§17 + 附录 A–G | §2.4/§2.7/§7.6/§13/§11.2 增补 + 新附录(签名/JSON);附录 G 重生成 |
| 静态类型层 | 无 | **`wlwl-types` 新 crate**(已落地:IR + `TypeEnv` + `TypeDiag` + `check.rs`) |
| 模块签名/可见性 | EXPORT 名集合 | **可选 `*.wll.sig` + `SEALED` + sig-gen**(D-2/D-3 已定形态;**词法/关键字表不变**) |
| MATCH 穷尽 | 无 | **6 形态 usefulness**。注意 Step 8 依赖 Step 2 的诊断管道,而 `TypeDiagKind::UndefinedName` 等 kind **刻意不分配码号**(`codes()` 返回 `None`)—— 宁可不给码,也不猜一个:猜错的码会进 spec §11.2 并被锁测试固定,事后改号是破坏性变更 |
| 泛型 | 语法槽位有(但 `ARRAY` 强制方括号、裸头不可写) | **擦除 + 限形(编译期实例化)**。D-5 已定最小显式约束 → **v0.10 唯一一处真正的语法面扩张**(`parse_braced` 要接受 `:`) |
| LSP / JSON | `ast --json` | **+ interface/schema JSON;+ lsp 薄壳** |
| 锁测试 | 1517 | **1568 已达成**(Step 0–2);全量目标 ≈1600±30(含新 conformance 夹具) |
| **工作量** | v0.9 14–18 人周 | **P0+P1 ≈ 7–10.5 人周(36–52 人日)**;含 P2 ≈ 8.5–12.5 人周 |

### 12.3 文档验收

- `docs/plan/wlwl-build-plan-v0.10.md`(本文件)批准;
- `docs/adr/0020-gradual-static-contracts.md` Accepted;
- `docs/plan/deviations.md` 重置为 **v0.10 / D10-NNN** 流水;
- `docs/standard/wlwl-spec-v0.10.md` 派生(v0.9 归档至 `docs/history/`);
- `CHANGELOG.md` v0.10.0 段(兼容句 + 零可观察变化声明);
- `README.md` §0.4 一致性表更新;
- `docs/appendix_G.md` 重生成(A6′);
- `wlwl-skill` 同步(新开关/诊断码/示例)。

### 12.4 出口条件(发版门禁)

1. S1–S6 全过;
2. **无包管理 diff**(`wlwl-toml` 功能面零扩展);
3. 默认模式下 `cargo test --workspace` 0 failed;
4. 开启模式 conformance 夹具稳定(连续 3 次本地运行一致);
5. §8「明确不做」清单零触碰;
6. 每条 P0/P1 立项单人日数与实际偏差 **> +50%** 时须补 D10-NNN 说明;
7. **唯一一处语法面扩张(D-5 泛型约束 `T: Comparable`)已落地并单列** ——
   §1.4 运算符表 / 关键字表除该处外**零变化**,§12 保留形式(v0.9 为空表)仍为空。

---

## 13 发版切分

| 版本 | 内容 | 出口条件 |
|------|------|---------|
| **v0.10.0** | P0-1 + P0-2 + P1-1 + P1-2 + P1-3;P2 余力 | S1–S6 全过;无包管理 diff |
| **v0.10.1** | 类型收窄、Variant 原型、`defer`、能力原型扩展;shallow 效果(仅当 P2 收口完成);函数泛型(若降级) | 向后兼容扩展 |
| **v0.11+** | 效果用户化、静态会话、后端设计→试点 | 另立 ADR / build plan |
| **包管理器** | **不进语言版本序列** | 独立产品,独立决策 |

---

## 附录 A · 对《路线》立项摘要的映射

| 《路线》§10 | 本计划 |
|-------------|--------|
| P0-1 `wlwl-types` 静态 pass,挂载 Check / ast 注解槽位 | §3(Step 1–5) |
| P0-2 模块签名 + 可见性 + sig-gen,挂载 ModuleLoader | §4(Step 6–7) |
| P1-1 MATCH 穷尽/冗余,挂载 Pattern 6 形态 | §5.1(Step 8) |
| P1-2 泛型限形(擦除),Value 不变 | §5.2(Step 9) |
| P1-3 check 语义化 + lsp 薄壳 + interface/schema JSON | §5.3(Step 10) |
| P2 std 加法 / 效果收口 / 能力旁路 | §7(Step 11) |
| 禁令清单 | §8(评审门禁) |
| 验收:默认 1518 全绿;开启可抓错;E0033 绿;悬空清零;无人日不立项 | §11.3 S1–S6 + §12 |

## 附录 B · 证据索引(便于复核)

| 主题 | 锚点 |
|------|------|
| workspace / crate 划分 | `impl/Cargo.toml` |
| eval 单体(≈21765 行;strict_types @8584-8627) | `impl/crates/wlwl-eval/src/lib.rs` |
| 类型注解 AST | `wlwl-ast/src/lib.rs:86-126,133,181,360` |
| Pattern 六形态 | `wlwl-ast/src/lib.rs:228-254` |
| Check = parse | `wlwl-cli/src/main.rs:51-52,85-88,630` |
| `ast --json` | `wlwl-cli/src/main.rs:59-61,156-208` |
| BuiltinSpec 签名串 | `wlwl-eval/src/registry.rs:44-70` |
| 模块边界 | `wlwl-eval/src/lib.rs:774-871,1036-1133,1215-1224` |
| 效果半落地 | `wlwl-eval/src/runtime.rs:186-265,220`;`CHANGELOG.md` v0.9 Known limitations |
| 包:manifest / lock / mvs | `wlwl-toml/src/{lib,manifest,lock,mvs}.rs` |
| 否决 PubGrub | `docs/adr/0011-mvs-dependency-resolution.md` |
| 否决全程序静态类型(历史) | `docs/adr/0010-strict-types-behavior.md` |
| 调度单线程边界 | `docs/adr/0016-scheduler-single-thread-boundary.md` |
| 正式技术路线 | `docs/plan/wlwl-v0.10.0-迭代技术路线建议.md` |
| 错误码注册 | `wlwl-error/src/lib.rs` |

## 附录 C · 术语表

| 术语 | 含义 |
|------|------|
| **Static Contracts** | v0.10.0 主题:在运行时能力之上补静态契约层(类型边界、模块签名、MATCH 穷尽) |
| **gradual_typing** | 新编译期开关 `off \| warn \| error`;与运行时 `strict_types` 正交 |
| **Dynamic** | 静态类型层的顶元/回退;未标注即 Dynamic,不诊断 |
| **期望类型向下传播** | A2′:上下文把期望类型传给子表达式;非 HM 推断 |
| **sig-gen** | 从模块导出面自动生成签名骨架的 CLI |
| **SEALED MODULE** | 禁止外部触达非导出符号的模块标记 |
| **usefulness / Maranget** | 模式匹配穷尽性判定算法;在 WLWL 6 形态集上为小型实现 |
| **擦除泛型** | 泛型仅存在于静态层;运行时 `Value` 不带类型参数 |
| **E1–E4** | 证据等级:代码事实 / 项目文档 / 规范文本 / 工程外推 |
| **D10-NNN** | v0.10 偏差登记流水号 |

---

*本构建计划以《wlwl-v0.10.0-迭代技术路线建议.md》为范围裁决,以实现代码论据为挂载点,以业主硬约束为门禁。凡与 `wlwl-v0.10.0-继续路线研究报告.md` 冲突之处,以《路线》为准。**包管理器相关提议维持彻底否决。***
