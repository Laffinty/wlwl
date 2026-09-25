# WLWL v0.10.0 迭代技术路线建议（正式版）

| 项 | 内容 |
|---|---|
| 文件性质 | **正式技术路线建议**（立项/排期/验收依据） |
| 效力关系 | 采纳 `wlwl-v0.10.0-路线辩论结论.md`（实现方）的代码论据与硬约束；据此**修订并取代** `wlwl-v0.10.0-继续路线研究报告.md` 中与之冲突的结论 |
| 上游输入 | 六方 AI 结论（`AI/`）、`wlwl-spec-v0.9.md`、`CHANGELOG (1).md`、实现方辩论结论 |
| 证据基线 | `D:\Project\wlwl` 真实仓库（E1 代码 / E2 项目文档 / E3 规范 / E4 外推；**E1 优先**） |
| 业主硬约束 | **凡涉及包管理器的提议，一律彻底否决**（含骨架、预留、必配、求解器、注册表等一切变体） |

---

## 0. 本版相对研究报告的效力声明

| 冲突点 | 研究报告原判断 | 本正式版裁决 | 依据 |
|---|---|---|---|
| 包管理/依赖/锁文件「缺失」，须作 v0.10 主轴 | Pillar C-C4「最小包管理」，综合分 8.4，排名 2 | **事实纠正 + 彻底否决扩展** | `wlwl-toml` 已实现 manifest/lock/MVS；ADR-0011 已否决 PubGrub；业主禁止扩展 |
| 类型系统「落地可行性高」，A1–A6 一揽子 | 可行性高，v0.10 全量进 | **采纳方向，改道为独立静态 pass，砍范围** | 现状是注解字符串对比运行时 `TYPE`；ADR-0010 曾否决全程序推断 |
| ADT Variant / Open Row 进 v0.10 | Pillar B 主打 | **本版否决** | 需扩 `Pattern::Constructor` + 运行时 `Value` + 穷尽规则三联动 |
| 宏、用户效果、类型收窄进本版 | 配套项 | **推迟 / 否决本版** | 注册表 24 条宏构造已是稳定语法面；效果半落地未收口 |
| 「综合迫切度」可作排序依据 | 六方打分平均 | **降级为参考**；改以**挂载点 + 人日 + 验收**排期 | ADR-0011 以「1 天 vs 1–2 周」否决 PubGrub——本项目已有成本否决先例 |

**方法论修正（吸收实现方批评）**：

1. 没进仓库的综合，分数再平均也是空谈；
2. 「想要」与「人日」必须分列；
3. 「落地可行性」必须写到挂载点；
4. 包管理议题属「已裁决 + 业主否决」双重关门，不再以任何名义回潮。

---

## 1. 硬约束与证据纪律

### 1.1 业主硬约束（无折中）

| 对象 | 裁决 |
|---|---|
| 新增/扩展任何包管理能力（PubGrub、semver 求解骨架、注册表、新依赖字段、远程源） | **彻底否决** |
| DeepSeek「复用 pubgrub crate」 | **彻底否决**（倒裁 ADR-0011） |
| Qwen「类型要跨包 → 必配包管理」 | **否决推论**：类型契约在 `ModuleLoader` 边界即可验证 |
| Grok「包注册表 + 远程源」 | **彻底否决**（供应链/运维产品线，非语言语义） |
| 既有 `wlwl-toml`（manifest/lock/mvs）与 CLI 锁生成 | **保留、冻结**：修 bug 可以，v0.10 不扩展 |
| `E0041` / `E0045` | **维持现状触发语义**，不新增包管理类错误码 |
| 包管理器作为独立产品 | 可以存在，但**不进 v0.10 路线、不进语言规范章节** |

### 1.2 证据纪律

| 等级 | 含义 | 用法 |
|---|---|---|
| **E1 代码事实** | `D:\Project\wlwl` 直接读到的结构/行为/测试 | 裁决硬依据 |
| **E2 项目文档** | ADR / CHANGELOG / build plan / deviations | 佐证「为何这样做」 |
| **E3 规范文本** | `wlwl-spec-v0.9.md` | 只约束语言语义边界，**不能代替实现现状** |
| **E4 工程外推** | 成本、算法移植等经验判断 | **必须标明**，不得写成「已验证」 |

**ADR 纪律**：与既有 ADR 冲突的立项（尤其是重开 ADR-0010「静态类型检查器」）必须**单列为有意推翻**，写明成本与迁移，不得写成顺手升级。

---

## 2. 立项前提：代码现实画像（E1）

### 2.1 已存在、不得再当「缺口」的

| 能力 | 证据 | 含义 |
|---|---|---|
| 清单 + 锁文件 + 版本求解 | `wlwl-toml/src/{manifest,lock,mvs}.rs`；CLI 生成 `wlwl.lock` | 「包管理缺失」诊断为假 |
| 依赖求解算法裁决 | ADR-0011：Cargo-style MVS（1 天）**否决** PubGrub（1–2 周） | 禁止倒裁 |
| 类型注解 AST 槽位 | `wlwl-ast/src/lib.rs:133,181,360`（`TypeAnnotation`、参数/返回注解） | 静态层有挂载面 |
| `wlwl ast --format=json` | `wlwl-cli/src/main.rs:59-61,156-208` | 机器可读导出已有先例 |
| std 模块分层 | `wlwl-std` 已按 `io/fs/json/collection/format/test/ai/agent` 分文件 | 「分层空白」不成立 |
| `strict_types` 运行时边界检查 | `wlwl-eval/src/lib.rs:8584-8627`；默认关；失配 `E0033` | 运行时路径有测试锁，须保留 |
| 会话/效果/OOP/线性 `THIS` 运行时 | v0.9 已落地 | 本版不深挖并发 |

### 2.2 真缺口（经代码核验）

| 缺口 | 证据 | 对应动作 |
|---|---|---|
| **无静态类型层**：注解只是字符串，与运行时 `TYPE` 名对比；「Nested generic / array element matching is deliberately deferred」 | `lib.rs:8587-8600`；ADR-0010 | 新建 `wlwl-types` 静态 pass |
| **`check` 只 parse**，无语义/类型检查 | `main.rs:51-52`；测试 `check_only_parses` | `check` 语义化是天然挂载点 |
| **无模块导出契约**（无签名、无私有、无 SEAL） | 模块加载在 `lib.rs:797-871`，无签名校验 | 模块签名 + 可见性 |
| **MATCH 穷尽性无检查**；`Pattern` 仅 6 形态，`Constructor` 仅 OK/ERR | `ast/lib.rs:228-254` | Maranget 在此形态集属小型算法 |
| **注册表签名是文档串**，非结构化类型 | `registry.rs:44-70`（`&'static str`） | 分批结构化 |
| **效果半落地**：`Effect::MethodCall`/`ProtocolViolation` 未在 CALL_METHOD 路径 raise | `runtime.rs:186-265,220`；CHANGELOG Known limitations | 收口：真正 raise **或** 规范降级 |
| **求值器单体风险**：`wlwl-eval/src/lib.rs` ≈ 21,765 行 / 789 内嵌测试 | 实测 | 新语义进新 crate/pass，勿侵入 eval |
| 测试夹具偏薄：`.wll` 仅 22 个，锁测试以 Rust 单测为主 | 实测 | 类型/模块需新增 conformance 目录 |

### 2.3 ADR 既往裁决对本版的约束

| ADR | 内容 | 对 v0.10 的含义 |
|---|---|---|
| **ADR-0010** | 否决「全程序类型推断/编译期检查器」（理由：triple the work），选运行时 transient cast | v0.10 **重开静态 pass = 有意推翻**，须单列成本；范围从「全程序推断」缩到「边界检查 + 期望类型向下传播」 |
| **ADR-0011** | MVS 胜 PubGrub（1 天 vs 1–2 周） | 包管理扩展否决；**人日否决纪律**适用于所有先锋项 |
| **ADR-0016** | 调度单线程边界 | 会话 `par` / 多线程不做 |

---

## 3. 主题与成功标准

### 3.1 主题

> **WLWL v0.10.0 — `Static Contracts`**
> **类型最小闭环 + 模块契约 + MATCH 穷尽性**
>
> （无包管理、无宏、无用户效果、无新 ADT 值形态）

**一句话定位**（吸收六方叙事 + 实现方收束）：

> v0.9 把控制与运行时前沿做完了；v0.10 不再横向加能力，而是给已有能力补上**静态契约层**——让 `check` 能在默认关闭时零破坏、开启后抓住注解/签名/MATCH 错误。

### 3.2 成功标准（可测，全部 E1 可验）

| # | 标准 | 验收方式 |
|---|---|---|
| S1 | **默认零破坏**：`gradual_typing` 等新特性默认关闭时，现有 ≈1518 测试全绿 | `cargo test --workspace` |
| S2 | **开启可抓错**：开启后能稳定诊断注解失配、模块签名违例、MATCH 非穷尽/不可达 | 新增 conformance 夹具 + 错误码锁测试 |
| S3 | **运行时兼容**：`strict_types` 运行时 `E0033` 路径测试保持绿 | 既有 `lib.rs` 测试锁 |
| S4 | **悬空承诺清零**：`Effect::MethodCall`/`ProtocolViolation` 要么真正 raise，要么规范明文降级 | CHANGELOG Known limitations 条目消除或改写 |
| S5 | **无包管理动作**：`wlwl-toml` 无字段/算法/生态面扩展 | diff 评审 |
| S6 | **人日可核**：每项立项写明挂载点与预估人日（沿用 ADR-0011 纪律） | 立项单 |

---

## 4. 正式范围

**容量上限（开发者口径，替代「感觉超载」）**：

> **1 个新语义子系统（静态类型 pass）+ 2 个生态增量（模块契约、工具链 JSON/LSP 薄壳）+ 1 个算法增量（MATCH 穷尽性）+ 纯加法库。**
> **泛型是唯一可挤进本版的深水区，且必须擦除 + 限形。**

禁止再「类型 + ADT + 泛型 + 模块 + LSP + 库 + 效果」齐上（v0.9 四件套后仍留 Known limitations 的教训）。

### 4.1 P0 — 主轴（必须进 v0.10.0）

#### P0-1 新静态类型 pass（Pillar A 改道版）

| 子项 | 内容 | 挂载点 | 验收 |
|---|---|---|---|
| **A1** 类型 AST / 静态类型环境 | 与运行时 `TYPE` 严格分层；新 crate `wlwl-types`（名可议） | 消费 `wlwl-ast` 注解槽位；不进 `wlwl-eval` | 类型语法树单测 |
| **A3** `gradual_typing = off \| warn \| error` | **新编译期边界检查**；运行时 `E0033` **保留** | `Check` 从 parse 升级：`parse → 可选静态 check`（`run_file` 挂载） | 默认关全绿；开启诊断稳定 |
| **A4** 容器/函数类型最小集 | `ARRAY<T>` / `DICT<K,V>` / `OPTION` / `RESULT` / 函数类型 | 类型层加法 | 注解 round-trip + 边界检查锁测试 |
| **A2′** 期望类型向下传播 | **砍掉 HM / let-多态**；只做局部、上下文驱动 | `wlwl-types` 内部 | 未标注处回退 `Dynamic` |
| **A6′** 注册表结构化签名 | **分批**：先高频内建，其余 `Dynamic` | `BuiltinSpec.signature` 由文档串升级为结构化（保留文档列）；同步 `gen_appendix_g` | 首批条目锁测试；不承诺 110 条一次完成 |

**明确砍掉（本版）**：

- A2 全量局部推导 / HM → 只保留期望类型向下传播；
- A5 流敏感类型收窄 → **推迟**（与 MATCH/IF 分支分析耦合，范围失控）。

**兼容句（E1 可验证）**：`strict_types=false` 时 v0.9 程序行为不变；新增编译期诊断只在显式开启后出现；运行时 `E0033` 路径测试保持绿。

**与 ADR-0010 的关系（须在 CHANGELOG/ADR 中明文）**：本项**有意重开**「静态检查」方向，但范围限缩为「边界契约 + 向下传播」，不是 ADR-0010 当年否决的「全程序类型推断」。建议**新立 ADR**（如 ADR-00xx：Gradual Static Contracts）记录推翻范围与人日预算，避免历史歧义。

---

#### P0-2 模块契约（Pillar C 的 C1–C3；C4 已否决）

| 子项 | 内容 | 挂载点 | 验收 |
|---|---|---|---|
| **C1** 模块签名 | 接口清单：导出类型、函数、（可选）效果/能力声明 | `ModuleLoader` 边界（`eval/lib.rs:797-871`） | 签名不匹配有错误码 |
| **C2** 可见性 / `SEALED` | `PRIVATE` / 显式 `EXPORT`；`SEALED MODULE` 禁止外部触达非导出符号 | 解析器 + 模块加载 | 越界访问可诊断 |
| **C3** 签名校验 + `sig-gen` | 编译期「实现满足签名」；自动生成签名骨架 | 与 `wlwl ast --json` 同风格 CLI | 生成骨架可 parse/check |
| **C5′** std 命名整理 | **仅命名/文档整理**（分层已存在） | `wlwl-std` | 无行为变化 |

**注意**：类型契约在模块边界验证即可，**不依赖包管理器**（否决 Qwen 推论）。

---

### 4.2 P1 — 本版应做（优先级次于 P0）

#### P1-1 MATCH 穷尽性 / 冗余检测

| 内容 | 挂载点 | 验收 |
|---|---|---|
| 在现有 **6 种 `Pattern`**（Ident/Wildcard/Literal/Array/Dict/Constructor）上实现 usefulness / Maranget | 现有 MATCH 求值前的静态 pass；`Constructor` 仅 OK/ERR | 缺失模式列出；不可达子句警告；锁用例 |

**性质**：**算法增量，且比六方估计更可落地**——形态集窄，无 or-pattern / guard / ref pattern。报告原「成本可控」结论**成立并上调**。

#### P1-2 泛型限形（唯一允许的深水区）

| 内容 | 约束 | 验收 |
|---|---|---|
| 参数化容器/函数：`ARRAY<T>`、`FUN(T) -> U` 等 | **运行时擦除**；无完整 trait / typeclass 推断 | 编译期实例化错误；**`Value` 模型不变** |
| 轻量约束 | 最多 `T: Comparable` 级别的显式约束，不做隐式解析 | 不引入 trait 求解器 |

**禁令**：不改 `wlwl-eval` 的 `Value` 变体；不引入 monomorphization；不做高阶 typeclass。

#### P1-3 工具链薄壳

| 内容 | 挂载点 | 验收 |
|---|---|---|
| `check` 语义化 | 现有 `Check` 只 parse（`main.rs:51-52`）→ 接静态 pass | `check_only_parses` 语义升级为分层测试 |
| `wlwl lsp` 薄壳 | 包装 parser + error + 静态诊断：diagnostics / definition / hover | LSP 集成冒烟；补全由注册表驱动 |
| 扩展机器可读导出 | 已有 `ast --json` 顺增量：`interface` / `schema` JSON | **JSON schema 锁测试** |

**性质**：Compiler Feedback API 在此处以低成本落地（`ast --json` 已有先例），服务 AI 工具链，**不做 AI 专用语法**。

---

### 4.3 P2 — 余力项（不阻塞发版）

| 项 | 内容 | 约束 |
|---|---|---|
| **标准库加法** | `BYTES` / `TIME` / `MATH` / `RANDOM`（`REGEX` 视实现依赖再定） | 纯加法；`wlwl-std` 独立文件；不碰求值核心 |
| **Known limitations 收口** | `Effect::MethodCall` / `ProtocolViolation` **真正 raise 或规范降级** | 悬空承诺清零（S4） |
| **能力沙盒旁路原型** | 默认路径零变化；至多实验开关 | **不进 P0**；std 调用面大 |

---

## 5. 明确不做（v0.10.0）

| 项 | 理由 |
|---|---|
| **包管理器一切扩展**（PubGrub / 注册表 / semver 骨架 / 新依赖字段 / 远程源） | 业主硬约束 + ADR-0011 + 已有工具冻结 |
| **宏 / 用户元编程** | 注册表 24 条宏构造已是稳定语法面；再开用户宏 = 把 AST 变 ABI |
| **Variant / Open Row 新值形态** | `Pattern::Constructor` + 运行时 `Value` + 穷尽规则三联动；先原型 |
| **类型收窄 / Guard 语法预留** | 范围失控 / 污染稳定面 |
| **用户代数效果处理器**（含 shallow） | 先收口 `Effect::MethodCall` 半落地；否则半落地叠半落地 |
| **ownership / lifetime / borrow checker** | 「不要把 WLWL 变成 Rust」；只可在类型层预留 `Unique`/`Resource`/`Capability` 名字 |
| **Wasm/字节码后端交付** | 只许设计文档；workspace 无后端 crate |
| **会话 `&`/`par`、多方会话、多线程** | ADR-0016 + 六方一致 + 用户禁止深挖并发 |
| **调试器全量 / HAMT / 依赖类型 / AI 专用语法** | 单家或研究项，不进本版 |

**v0.10.1+ 候选**（未承诺）：类型收窄、Variant 原型、shallow 效果（在半落地收口后）、`defer`、宏评估。  
**包管理器**：**不进语言版本路线**；既有 `wlwl-toml` 冻结。

---

## 6. 容量、风险与人日纪律

### 6.1 容量口径（E1）

| 口径 | 现状 | 对排期的含义 |
|---|---|---|
| 求值器 | 21,765 行单文件，789 测试 | 任何 eval 内改动高风险；新语义进**新 crate/新 pass** |
| 测试 | ≈1518 `#[test]`，`.wll` 夹具仅 22 | 类型/模块须**新增 conformance 目录**，不只加 `#[test]` |
| CLI | 4 子命令；`check` = parse | 类型 pass 挂载点现成 |
| ADR 预算先例 | MVS 1 天 vs PubGrub 1–2 周 | **先锋项必须给人日**；写不出人日的一律降级 |

### 6.2 风险清单

| 风险 | 缓解 |
|---|---|
| 重开 ADR-0010 被视为推翻项目纪律 | **新立 ADR** 明文范围限缩 + 人日预算；CHANGELOG 同步 |
| 静态 pass 侵入 eval | 强制 `wlwl-types` 独立 crate；eval 只保留运行时 `E0033` |
| 注册表结构化工作量被低估 | A6′ 分批；首批高频内建，其余 `Dynamic` |
| 模块签名破坏现有 IMPORT 行为 | 签名文件可选；无签名 = 现行为 |
| 范围蠕变（效果/宏/包管理回潮） | 本文件「明确不做」清单作评审门禁 |
| 测试锁不足 | 新增 conformance 目录；JSON schema 锁测试 |

### 6.3 人日纪律（沿用 ADR-0011）

每个 P0/P1 立项单必须包含：**挂载点（文件/符号）→ 变更面 → 预估人日 → 验收命令**。缺任一项不予立项。

---

## 7. 兼容性承诺（对外句式）

> **任何 v0.9 程序，在 v0.10.0 上默认可观察行为不变。**
>
> 例外：无。
>
> 新增编译期诊断仅在 `gradual_typing` / 模块签名校验等显式开启后出现；运行时 `strict_types`/`E0033` 路径不变；`wlwl-toml` 及 `E0041`/`E0045` 触发语义不变。

配套工程动作：

1. 既有 ≈1518 测试全绿（S1）；
2. 运行时 `E0033` 测试锁保持（S3）；
3. CHANGELOG 沿用「可观察变化逐条列明」文化；本版预期**零可观察变化**（纯加法 + 默认关）。

---

## 8. 发版切分

| 版本 | 内容 | 出口条件 |
|---|---|---|
| **v0.10.0** | P0-1 静态类型 pass + P0-2 模块契约 + P1-1 MATCH 穷尽 + P1-2 泛型限形 + P1-3 工具链薄壳；P2 余力项 | S1–S6 全过；无包管理 diff |
| **v0.10.1** | 类型收窄、Variant 原型、`defer`、能力原型扩展；shallow 效果（仅当 P2 收口完成） | 向后兼容扩展 |
| **v0.11+** | 效果用户化、静态会话、后端设计→试点 | 另立 ADR / build plan |
| **包管理器** | **不进语言版本序列** | 独立产品，独立决策 |

---

## 9. 对六方报告的采纳/否决对照（审计表）

| 六方主张 | 来源 | 本版处理 |
|---|---|---|
| 类型薄弱是最大缺口 | 六方 6/6 | **采纳**（P0-1），范围改道 |
| 模块契约 / 可见性 | DB/CG/QW/GRK | **采纳** C1–C3（P0-2） |
| MATCH 穷尽性 | DS/CG/DB | **采纳并上调**（P1-1，形态集窄） |
| 泛型 | DS 10 分 / CG 9 分 | **采纳限形版**（P1-2，擦除 + 无 trait 求解） |
| LSP / 机器可读诊断 | DS/CG | **采纳薄壳**（P1-3） |
| 标准库补 BYTES/TIME/… | CG/QW/GRK | **采纳纯加法**（P2） |
| 效果半落地须收口 | DS | **采纳为 P2 验收项 S4** |
| 最小包管理 / PubGrub / 注册表 | QW/DS/GRK/CG | **彻底否决** |
| ADT Variant 进 v0.10 | CG 10 分 | **否决本版**（先原型） |
| 用户效果 shallow 进 v0.10 | DB/GRK/CG | **推迟** |
| 宏进 v0.10 | DB/GG/GRK | **否决本版** |
| 类型收窄 / Guard | CG/DS | **推迟** |
| 能力沙盒 P0 | GG 10 分 | **降为 P2 旁路原型** |
| HAMT / 后端交付 / 多线程 / 会话 par | 单家或低分 | **不做** |

---

## 10. 立项摘要（可直接进 build-plan）

```
主题: Static Contracts
容量: 1 语义子系统 + 2 生态增量 + 1 算法增量 + 纯加法库 (+限形泛型)

P0-1  wlwl-types 静态 pass     挂载: Check / wlwl-ast 注解槽位
P0-2  模块签名 + 可见性 + sig-gen  挂载: ModuleLoader
P1-1  MATCH 穷尽性/冗余         挂载: Pattern 6 形态
P1-2  泛型限形(擦除)            挂载: 类型层, Value 不变
P1-3  check 语义化 + lsp 薄壳 + interface/schema JSON
P2    std 加法 / 效果收口 / 能力旁路

禁: 包管理扩展、宏、Variant、收窄、用户效果、ownership、后端交付、par、多线程
验收: 默认 1518 全绿; 开启可抓错; E0033 绿; 悬空清零; 无人日不立项
```

---

## 附录 · 证据索引（实现方辩论结论所锚定，便于复核）

| 主题 | 锚点 |
|---|---|
| workspace / crate 划分 | `impl/Cargo.toml:1-20` |
| eval 单体（≈21765 行；strict_types @8584-8627） | `impl/crates/wlwl-eval/src/lib.rs` |
| 类型注解 AST | `wlwl-ast/src/lib.rs:133,181,360` |
| Pattern 六形态 | `wlwl-ast/src/lib.rs:228-254` |
| Check = parse | `wlwl-cli/src/main.rs:51-52,85-88,630` |
| `ast --json` | `wlwl-cli/src/main.rs:59-61,156-208` |
| BuiltinSpec 签名串 | `wlwl-eval/src/registry.rs:44-70` |
| 包：manifest / lock / mvs | `wlwl-toml/src/{lib,manifest,lock,mvs}.rs` |
| 否决 PubGrub | `docs/adr/0011-mvs-dependency-resolution.md` |
| 否决静态类型检查器（历史） | `docs/adr/0010-strict-types-behavior.md` |
| 效果半落地 | `wlwl-eval/src/runtime.rs:186-265,220`；`CHANGELOG.md:76-80` |
| 模块边界 | `wlwl-eval/src/lib.rs:774-871,1077-1078` |
| 调度单线程边界 | `docs/adr/0016`（实现方所引） |
| 规范「包管理出界」 | `wlwl-spec-v0.9.md:609`（仅说明**语言语义**出界，非实现缺口） |

---

*本正式版以实现方代码论据为准，以业主硬约束为门禁。凡与 `wlwl-v0.10.0-继续路线研究报告.md` 冲突之处，以本文件为准。包管理器相关提议维持彻底否决。*
