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

| 项 | 内容 |
|----|------|
| **目标** | 新建 crate `wlwl-types`:静态类型表示、类型环境、诊断类型;**与运行时 `TYPE` 严格分层** |
| **挂载点** | 消费 `wlwl-ast` 的 `TypeExpr` / `TypeAnnotation` / `FunParam.type_annotation` / `return_type`;**不进 `wlwl-eval`** |
| **变更面** | `impl/crates/wlwl-types/`(新);`impl/Cargo.toml` workspace 成员 +1;`wlwl-types/src/{lib,ty,env,diag}.rs` |
| **静态类型最小集** | `INTEGER` / `FLOAT` / `STRING` / `BOOLEAN` / `NULL` / `ARRAY<T>` / `DICT<K,V>` / `OPTION<T>` / `RESULT<T,E>` / `FUN(T…) -> U` / **`Dynamic`**(未标注回退) |
| **明确不做** | HM / let-多态;流敏感收窄(推迟);`Value` 模型改动 |
| **预估人日** | **3–4 人日**(E4) |
| **验收** | `cargo test -p wlwl-types`;类型 round-trip 单测;`TypeExpr` → 静态类型映射锁测试 |

**设计约束**:

- 静态类型是 `wlwl-types` 内部 IR,**不改** `wlwl-ast::TypeExpr` 语法面(additive only;若需扩展类型名,走 `Generic { name, args }` 既有槽位);
- `Dynamic` 是顶/底元:与任何类型可互通,未标注即 `Dynamic`;
- 运行时 `TYPE(x)` 字符串对比路径**不动**(S3)。

### 3.2 A3 — `gradual_typing = off \| warn \| error`

| 项 | 内容 |
|----|------|
| **目标** | 新编译期边界检查;运行时 `E0033` **保留**;默认 `off` |
| **挂载点** | `Check` 从 parse 升级:`parse → 可选静态 check`;`run_file` 挂载同一入口 |
| **变更面** | `wlwl-cli/src/main.rs`(Check/run 接线);`wlwl-toml/src/manifest.rs` **仅读** `[features] gradual_typing`(新键,默认 `"off"`;**不改求解/锁/依赖字段**);`wlwl-types/src/check.rs` |
| **配置** | `wlwl.toml [features] gradual_typing = "off" \| "warn" \| "error"`(默认 `"off"`);非法值 → 诊断并回落 `off` |
| **兼容句** | `gradual_typing=off` 时 v0.9 程序行为不变;新增编译期诊断只在显式开启后出现 |
| **预估人日** | **3–4 人日**(E4) |
| **验收** | 默认关:`cargo test --workspace` 全绿;开启:注解失配夹具报稳定诊断;`check_only_parses` 升级为分层测试(parse / static / run) |

**诊断码草案**(决策点 D-1,见 §10.2;默认新建静态段,不占用 `E0030-E0039` 运行时类型段):

| 条件 | `error` 模式 | `warn` 模式 |
|------|-------------|------------|
| 注解失配(边界) | `E0110` static annotation mismatch | `W0110` |
| 调用参数个数/类型失配 | `E0111` static call mismatch | `W0111` |
| 返回类型失配 | `E0112` static return mismatch | `W0112` |

> 既有 `E0033`(运行时 `strict_types`)语义与触发路径**不变**。静态码与运行时码可在同一程序并存(先静态拦、再运行时兜底)。

### 3.3 A4 — 容器 / 函数类型最小集

| 项 | 内容 |
|----|------|
| **目标** | `ARRAY<T>` / `DICT<K,V>` / `OPTION` / `RESULT` / 函数类型的静态表示与边界检查 |
| **挂载点** | `wlwl-types` 类型层加法;复用 `TypeExpr::{Array,Generic}` 解析结果 |
| **变更面** | `wlwl-types/src/ty.rs` + 边界检查规则;注册表结构化签名(§3.5)可提供内建返回类型的参照 |
| **兼容** | 运行时嵌套匹配仍可「deliberately deferred」;静态层**不要求**运行时升级 `E0033` 的嵌套能力 |
| **预估人日** | **2–3 人日**(E4) |
| **验收** | 注解 round-trip 锁测试;`ARRAY<INTEGER>` 边界失配诊断;函数类型 `FUN(INTEGER) -> STRING` 参数/返回检查 |

### 3.4 A2′ — 期望类型向下传播(砍掉 HM)

| 项 | 内容 |
|----|------|
| **目标** | 只做**局部、上下文驱动**的期望类型向下传播;未标注处回退 `Dynamic` |
| **挂载点** | `wlwl-types` 内部(check/propagate);**不引入** let-多态、不引入全程序推断 |
| **变更面** | `wlwl-types/src/check.rs`;无 eval 改动 |
| **规则(E4)** | 字面量按期望类型定型;容器字面量元素受 `ARRAY<T>`/`DICT<K,V>` 约束;`IF`/`MATCH` 合流取 lub 或 `Dynamic`;`CALL` 参数按被调签名向下传播 |
| **预估人日** | **2–3 人日**(E4) |
| **验收** | 未标注程序在开启模式下不误报;已标注边界失配可报;锁测试覆盖「回退 Dynamic 不诊断」 |

### 3.5 A6′ — 注册表结构化签名(**分批**)

| 项 | 内容 |
|----|------|
| **目标** | `BuiltinSpec.signature` 由文档串升级为**结构化**(保留文档列);同步 `gen_appendix_g` |
| **挂载点** | `wlwl-eval/src/registry.rs:44-70` 的 `BuiltinSpec`;`gen-appendix-g` |
| **变更面** | `BuiltinSpec` 增加 `sig: Option<StructuredSig>`(或并列 `structured_signature` 字段);**保留** `signature: &'static str` 文档串;附录 G 生成器读结构化字段补全 |
| **分批策略** | **首批**:高频内建(算术/比较/STRING/ARRAY/DICT/RESULT 消费者/控制流 ~40–60 条);其余显式 `Dynamic`/`None`,**不承诺 110 条一次完成** |
| **预估人日** | **3–4 人日**(首批)+ 后续批次滚动(E4) |
| **验收** | 首批条目锁测试(签名 ↔ 类型映射);`gen-appendix-g` 重生成无 diff 回归;未结构化条目行为不变 |

> **风险**:注册表结构化工作量被低估 — 用分批 + `Dynamic` 回退控制;若首批超 **5 人日**,削减首批条目数,不延期本项类型。

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
| **签名载体(默认)** | **可选**旁路文件 `foo.wll.sig`(或 `foo.sig.wll`,决策点 D-2);**无签名 = 现行为** |
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
| **挂载点** | 解析器(`wlwl-parser`)识别 `SEALED` 模块头(或 `SEALED MODULE(...)` 形式,决策点 D-3)+ 模块加载 |
| **现状** | `EXPORT(["a","b"])` 声明导出集合;`collect_exports`(`lib.rs:1215-1224`);**无 SEALED、无 PRIVATE 关键字**;导入未导出名 → `E0023`(未绑定) |
| **规则** | 未 `EXPORT` 的绑定默认**模块私有**(与现行为一致);`SEALED MODULE` 额外禁止:外部通过任何途径(包括 `MODULE_REF` 动态面,若存在)触达非导出符号 → 诊断 |
| **变更面** | parser 关键字/头部语法(**加法**);`ModuleLoader` 加载时记录 sealed 标记;导入边界检查 |
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
| **目标** | 参数化容器/函数:`ARRAY<T>`、`FUN(T) -> U` 等;**运行时擦除** |
| **约束** | 无完整 trait / typeclass 推断;无 monomorphization;**`Value` 模型不变** |
| **轻量约束** | 最多 `T: Comparable` 级别的**显式**约束,不做隐式解析;不引入 trait 求解器 |
| **挂载点** | 类型层(`wlwl-types`);注册表结构化签名中的参数化类型;**不改** `wlwl-eval::Value` 变体 |
| **编译期** | 实例化错误(如 `ARRAY<INTEGER>` 与 `ARRAY<STRING>` 误用)在静态层报;运行时仍擦除 |
| **预估人日** | **4–6 人日**(E4) |
| **验收** | 编译期实例化错误锁测试;`Value` 快照/行为不变;默认关零影响 |

**禁令**:不改 `Value`;不引入 monomorphization;不做高阶 typeclass。若超 **6 人日**,降级为「仅容器参数化、函数泛型推迟到 v0.10.1」。

### 5.3 P1-3 · 工具链薄壳

| 子项 | 内容 | 挂载点 | 人日 | 验收 |
|------|------|--------|------|------|
| **check 语义化** | `Check` 接静态 pass;分层测试(parse / static / run) | `main.rs:51-52,85-88,630` | 1–2 | `check_only_parses` → 分层;默认关行为不变 |
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

### 6.1 ADR-0020 草案 — *Gradual Static Contracts*

| | |
|---|---|
| **Status** | Proposed(本计划批准后 Accepted) |
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
| §2.4 类型注解 | 增补「静态解释」小节(默认不启用) |
| §2.7 `strict_types` | 明文区分**运行时** `strict_types` 与**编译期** `gradual_typing` |
| §7.6 MATCH | 增补穷尽性/不可达诊断(可选开启) |
| §13 / IMPORT-EXPORT | 模块签名、可见性、`SEALED MODULE` |
| §11.2 错误/警告码 | 注册 `E0110+` / `W0110+` 静态契约段 |
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
[Step 0] 本计划 + ADR-0020 定稿(用户审阅)
   ├─ 决策点 D-1..D-4 拍板
   └─ 动手改 impl 前锁定范围,避免返工
[Step 1] impl:wlwl-types 骨架(A1)
   ├─ 新 crate + workspace 接线
   ├─ TypeExpr → 静态类型 IR;Dynamic 回退
   └─ 锁测试:type_ir_roundtrip / dynamic_fallback
[Step 2] impl:gradual_typing 开关 + check 接线(A3)
   ├─ manifest 只读键 gradual_typing = off|warn|error
   ├─ Check: parse → 可选静态 check
   ├─ 诊断码 E0110-E0112 / W0110-W0112 注册
   └─ 锁测试:default_off_zero_diag / warn_mode_soft / error_mode_hard
[Step 3] impl:容器/函数类型最小集(A4)
   ├─ ARRAY<T> / DICT<K,V> / OPTION / RESULT / FUN
   └─ 锁测试:container_boundary_mismatch / fun_sig_check
[Step 4] impl:期望类型向下传播(A2′)
   ├─ 字面量/容器/IF-MATCH 合流/CALL 参数
   └─ 锁测试:no_false_positive_on_unannotated / propagate_call_args
[Step 5] impl:注册表结构化签名首批(A6′)
   ├─ BuiltinSpec 增加结构化 sig 字段(保留文档串)
   ├─ 首批 ~40-60 条高频内建
   ├─ gen_appendix_g 同步
   └─ 锁测试:builtin_sig_batch1 / appendix_g_regen_stable
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

### 10.2 决策点(待用户拍板)

| 决策点 | 默认 | 选项 | 状态 |
|--------|------|------|------|
| **D-1 静态诊断码段** | **A:新建 E0110+/W0110+ 静态契约段** | A:新段(默认);B:复用 E0030-E0039 旁挂;C:仅 W 码 | ⏳ 待批 |
| **D-2 签名文件载体** | **A:旁路 `*.wll.sig` 文本(与源文件同目录)** | A:旁路文件;B:源内 `SIG(...)` 构造;C:两者 | ⏳ 待批 |
| **D-3 SEALED 语法** | **A:模块头 `SEALED MODULE(...)` 或前置 `SEALED(...)` 声明(加法)** | A:模块头声明;B:`EXPORT(..., sealed)` 旗标;C:推迟到 v0.10.1 | ⏳ 待批 |
| **D-4 Effect 半落地收口** | **A:先评估真 raise 成本;>3 人日则规范降级** | A:真 raise;B:规范降级;C:维持 Known limitations | ⏳ 待批 |
| **D-5 泛型约束深度** | **A:可选显式 `T: Comparable` 级;默认无约束** | A:最小显式约束;B:纯参数化无约束;C:推迟函数泛型 | ⏳ 待批 |
| **D-6 LSP 薄壳范围** | **A:diagnostics + definition + hover;补全注册表驱动** | A:最小三项;B:加 rename/format;C:不做 lsp 只做 JSON | ⏳ 待批 |

### 10.3 锁测试预期增量

- 现有 ≈**1517 passed** + 0 failed(保留,只增不减)
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
| 错误码 | 61 激活 + 6 保留(共 67) | **+ E0110–E011x 静态契约段**(具体个数随 D-1 定稿);运行时码不变;包管理码零新增 |
| 警告码 | + W0065/W0066 | **+ W0110+ 静态契约警告段** |
| 内建数 | 110 | **110**(零新增;签名结构化不改内建集合) |
| `wlwl-toml` | manifest/lock/MVS | **冻结**;至多 `[features] gradual_typing` 只读键 |
| spec 章节 | §1–§17 + 附录 A–G | §2.4/§2.7/§7.6/§13/§11.2 增补 + 新附录(签名/JSON);附录 G 重生成 |
| 静态类型层 | 无 | **`wlwl-types` 新 crate** |
| 模块签名/可见性 | EXPORT 名集合 | **可选签名 + SEALED + sig-gen** |
| MATCH 穷尽 | 无 | **6 形态 usefulness** |
| 泛型 | 语法槽位有 | **擦除 + 限形(编译期实例化)** |
| LSP / JSON | `ast --json` | **+ interface/schema JSON;+ lsp 薄壳** |
| 锁测试 | ≈1517 | **≈1600±30**(含新 conformance 夹具) |
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
6. 每条 P0/P1 立项单人日数与实际偏差 **> +50%** 时须补 D10-NNN 说明。

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
