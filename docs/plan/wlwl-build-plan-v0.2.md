# WLWL v0.4 实施构建计划 v0.2(基于 v0.1 推进 + spec v0.4 合规)

> **状态**:v0.1 完成后的下一阶段计划
> **基于规范**:`docs/standard/wlwl-spec-v0.4(SHA1_97524ced037b5ef0a5820a2ebd5bafb4ba4e239b).md`
> **前置**:`docs/plan/wlwl-build-plan-v0.1.md`(全部 Phase 1-4 + P3-007..P3-013 已完成,13/13 crate ≥ 90% line,522/522 tests pass,tag v0.3.0 已发布)
> **核心约束**:本计划**不属于** WLWL 语言规范。规范只定义"语言该长什么样",本计划定义"如何把 v0.1 已发布实现推进到 spec v0.4 全合规 + 发布 v0.4.0 可执行二进制"。两者解耦——规范演进时,本计划同步跟进;但本计划的工程决策**不**回流到规范。
> **决策原则**:技术栈选型、模块划分、阶段划分等**均为建议**,最终决策权归 Li;本计划以"待 Li 拍板"为默认状态。
> **v0.1 已完成项**:见 v0.1 文档 §实施进度跟踪 + `docs/history/20260903.md`;本计划**不重述**已完成工作,只列出 v0.2 范围内的工作与跨阶段依赖。

---

## 0. 文档导读

### 0.1 关键决策(从 v0.1 沿用,本计划更新)

| # | 决策项 | 决策 | 备注 |
|---|--------|------|------|
| 1 | 技术栈 | **接受**:Rust + tree-walking | 沿用 v0.1 |
| 2 | 工期预算 | **接受**:v0.1 实际 1 天(v0.4 工作量约 7 周估算,AI workflow 加速后可能更短) | v0.1 实际进度远超估算,本计划保留估算供 discipline |
| 3 | Phase 5 形式化 | **降级为可选**:v0.4 附录 E 修复了 E-Lit / IF / closure / TransparentErr,但未机械化 | 与 v0.1 决策一致;v0.4 形式化覆盖度提升后仍允许跳过 Coq |
| 4 | CI 平台 | **GitHub Actions** | 沿用;三平台 matrix (Linux/macOS/Windows) |
| 5 | 许可证 | **GPL v2** | 沿用;v0.1 §0.1 已锁定 |
| 6 | 目标用户 | **公开** | 沿用 |
| 7 | **v0.2 范围**(本计划新增) | **spec v0.4 全合规 + 独立质量 phase + 发布 v0.4.0 exe** | 由 Li 在本计划启动时锁定 |
| 8 | **Phase G 质量 phase**(本计划新增) | **强制**:所有 v0.2 提交必须通过 G 阶段门禁(zero clippy warning / 100% rustdoc / fuzz / etc.) | 由 Li 在本计划启动时锁定 |
| 9 | **exe 发布目标**(本计划新增) | **三平台 native binary**(Linux x86_64, macOS universal, Windows MSVC),`release.yml` 跨平台构建 + GitHub Release | v0.1 release.yml 已就位,Phase H 复用 |

> **本计划启动时** Li 需就 #7 / #8 / #9 拍板;其余沿用 v0.1。

### 0.2 v0.1 → v0.2 关键变化

| 类别 | 变化 | 来源 |
|------|------|------|
| **规范基础** | v0.3 (MD5 `4308b3d...`) → v0.4 (SHA1 `97524ced...`) | 新 spec 2026-09-05 冻结 |
| **错误码总数** | 35 (含 E0063 / E0083 / E1003 散落) → 56 唯一(§14.4 总表首次统一) | spec §14.4 修正计数错误 |
| **警告码** | 8 (W0001-W0040) → 14 (W0001 删除并入 E0020,新增 W0014/W0015/W0016/W0050-W0054) | spec §14.5 |
| **错误 schema** | 1.0.0 → 1.1.0(MINOR bump,新增 trace/cause/retry_after/idempotent,location 由数组改对象) | spec §14.2 |
| **内建类型** | 8 → 9(新增 `RESULT`,`OK(...)` / `ERR(...)` 为一等值) | spec §2.2.1 |
| **关键字** | 16/14(自相矛盾) → 14 关键字 + 显式宏函数清单(§3.4,本规范首次给出) | spec §3.3 / §3.4 |
| **新宏函数** | 0 显式清单 → 28 显式宏函数(IF/WHILE/FOR/MATCH/TRY/UNWRAP/...) | spec §3.4 |
| **新内建** | — | INDEX_GET/INDEX_SET/AT/REMOVE_KEY/UNWRAP_OR/UNWRAP/ERR_PAYLOAD/WRAP/FORMAT/MODULE_REF/NOT/PRINT_ERR/FLOAT/TRIM(START/END)/STARTS_WITH/ENDS_WITH/REPEAT/PAD_START/PAD_END/CODEPOINTS/FROM_CODEPOINTS 全部新增 |
| **删除项** | — | `AS` 函数(§13.4 钉死,迁移 LET 写法);`DEL`(改为 REMOVE_KEY 别名 + W0051);`OR_DIE`(改为 UNWRAP_OR 别名 + W0051);`!` 单字符(改为 NOT 宏函数 + W0054) |
| **新标准库** | 8 模块 | 新增 `std.collection`(§15.7,16 个高阶函数)、`std.format`(§15.8,FORMAT 归属)、`std.test`(§15.9,TEST/ASSERT/RUN_TESTS)、`std.agent`(§15.14,Agent 编排) |
| **核心语义补全** | — | 闭包 cell 语义(§6.4,修复 v0.3 §6.3 vs §8.5 矛盾);比较返回类型规则(§9.2,修复矛盾);真值表补 OK/ERR/DICT/NAN(§9.4);元数公式(§8.4 修复);DICT/ARRAY 下标访问与越界(§10.1/§10.2,E0036/E0037);解构绑定(§7.5,E0026);模式匹配 MATCH(§7.6,E0027);数值跨类型(§9.5,E0034/E0035/E1003/W0015) |
| **错误系统** | 4 项白名单 → 9 项注册表(§12.7) | 注册表可扩展(规范升级时增) |
| **新错误码** | — | E0024-E0029、E0033-E0039、E0042、E0044-E0045、E1003(已存在,入总表)、E0046-E0049(std.test) |
| **strict_types** | v0.3 未决 | spec §2.7 定型:wlwl.toml 字段 + 边界检查 + E0033 |
| **allow_builtin_shadow** | v0.3 不存在 | spec §6.6:默认 false,E0025 拒绝遮蔽,开 true 时允许但 W0030 |
| **MVS 依赖求解** | v0.3 未决 | spec §13.9 定型 Cargo 风格 MVS,E0045 冲突 |
| **language_version** | v0.3 不存在 | spec §13.8 必填字段 + E0044 不匹配 |
| **canonical formatter** | v0.3 不规定 | spec §16.3 必规范化细节;实现 W0053 警告 |
| **AST/Patch 协议** | v0.3 仅 suggestion_code 文本 | spec §16.4 新增 node_id / hash / ast_rewrite |
| **一致性测试套件** | v0.3 不存在 | spec §16.5 实现不通过不得自称 "v0.4 compliant" |
| **EBNF 文法** | 散落正文 | spec 附录 F(规范性)首次给出 |
| **全局内建注册表** | 散落各章 | spec 附录 G(规范性)首次统一 |
| **形式化附录** | 5 处内部缺陷 | 附录 E 修复 E-Lit 自循环 / IF truthiness / closure mutation / E-TransparentErr;新增 BREAK/CONTINUE/SET/MATCH/INDEX_GET/INDEX_SET 形式化 |
| **新增 ADR 候选项** | — | 闭包 cell 升级;ERR 消费者注册表;strict_types 行为;MVS;AS 删除;canonical formatter |

### 0.3 实施进度跟踪(从 v0.1 沿用 + 续)

> v0.1 完成状态(2026-09-04 截止,本计划启动时基线):
>
> - Phase 1-4 + post-Phase 4 + P3-007..P3-013 全部完成
> - 13/13 workspace file ≥ 90% line coverage(TOTAL 92.94% / region 92.65% / func 96.90%)
> - 522/522 tests pass
> - tag v0.3.0 已发布,跨平台二进制 + GitHub Release 已创建
> - **未启动项**(从 v0.1 推迟至 v0.2):
>   - P3-014:mkdocs 文档站 → 本计划 G11
>   - P3-015:跨目录引用(实际 parser 已接受,ModuleLoader::load 跨目录待补) → 本计划 C7
>   - P3-016:真实 std.ai HTTP 集成(reqwest) → 本计划 D1-D5
>   - P3-017:wlwl.toml 完整解析 + lock 算法(基本已实现,补 language_version/MVS) → 本计划 C3-C4
>   - P3-018:性能 TCO + hot-inline → 本计划 F
>   - P3-019:Phase 5 Coq 形式化 → 本计划 #3 决策,沿用 v0.1 选 optional
>   - W0020 linter 端独立 walk → 本计划 E4
>   - rich `suggestion_code` content(35+ 错误码 per-site 候选) → 本计划 G7
>
> v0.2 进度跟踪将按 Phase A-H 序列追加,每批结束 git add + commit + push,工作树保持可回滚状态。

---

## 0. 文档定位

| 维度 | WLWL 规范(v0.4) | 本构建计划 |
|------|------------------|------------|
| 范围 | 语言的语法、语义、标准库、错误信息格式 | 实现技术栈、阶段路线、模块划分、工具链 |
| 受众 | 编译器实现者、文档作者、AI 工具、用户 | 实施工程师、维护者 |
| 演进方式 | 严格版本化 + SHA1 内容寻址(v0.4 沿用 v0.3 MD5 模式) | 与规范版本号解耦,独立版本号 |
| 决策依据 | 学术共识、工业实践、AI 友好 | 工程实际、团队能力、时间预算 |

> **关键提示**:v0.4 规范 §0.4 新增"符合性(Conformance)"要求——实现必须接受附录 F 全部合法程序、拒绝 §X.Y 实现定义行为外的非法程序、可观察行为符合 §2-§14 与附录 E。这意味着 v0.2 计划的**最低成功标准是 spec v0.4 §16.5 一致性测试套件全过**。

---

## 1. 实施总览

### 1.1 目标

在 **v0.1 基线之上**(已发布的 v0.3.0 实现 + 522 tests + 13/13 crate ≥ 90% line),交付 spec v0.4 全合规的 **v0.4.0 可执行二进制** 发布:

- 接受 spec v0.4 附录 F 全部合法 WLWL 程序
- 拒绝 spec v0.4 范围外的非法程序,或按 §X.Y 实现定义行为处理
- 可观察行为符合 §2-§14 与附录 E 形式化(冲突时自然语言为准,§0.7)
- 错误信息符合 §14 schema 1.1.0
- 工具链最小集扩展:`wlwl run` / `wlwl check` / `wlwl ast` / `wlwl fmt`(新增)/ `wlwl repl`(可选)
- 56 个错误码 + 14 个警告码全部就位
- 通过 spec §16.5 一致性测试套件
- **Phase G 质量门禁** 全部通过(clippy / rustdoc / fuzz / cargo-deny / 性能回归)
- **三平台 native binary 发布**(Linux x86_64 / macOS universal / Windows MSVC)

### 1.2 原则(与规范 §1.2 设计原则对齐,沿用 v0.1 §1.2)

1. **简单优于聪明**:v0.4 范围比 v0.3 大 ~2x(56 vs 35 codes, 28 宏函数, 9 内建类型),坚持"最小实现满足规范"原则
2. **可测试性优先**:每个新错误码 / 警告码必须有独立测试;每个 spec §16.5 一致性测试用例必须有 fixture
3. **AI 友好是硬要求**:error schema 1.1.0 升级要 100% 向后兼容(旧 AI 工具忽略新字段,新 AI 工具享受 trace/cause)
4. **规范变更的容错**:v0.4 是"封闭规范",任何实现与规范不一致必须**显式**记录在 `plan/deviations.md`
5. **v0.1 baseline 保护**:v0.2 阶段 13/13 crate ≥ 90% line / 522 tests pass 基线**不倒退**;每次 commit 必须 `cargo test --workspace` + `cargo llvm-cov --workspace` 通过

### 1.3 风险等级(初评)

| 风险 | 等级 | 缓解 |
|------|------|------|
| 闭包 cell 语义(§6.4)改造波及 eval | **高** | 显式 cell 升级逻辑;`SET` 调用点的 mutability 检查;Phase A 隔离 |
| 错误码 35→56 总表重构 | **高** | 一次 PR 提交;deviations.md 显式登记;spec §14.4 总表是 source of truth |
| strict_types 运行时检查性能 | 中 | spec §2.7 给出 ≤ 10% 开销目标;Phase E 隔离;failing-fast |
| 真实 std.ai HTTP 集成网络/凭据 | **高** | mock fallback 保留;spec §15.13 凭据仅 env 变量;Phase D 隔离 |
| canonical formatter 行为 | 中 | spec §16.3 10 条规则机械可推导;Phase E2 idem­potent test |
| 一致性测试套件(spec §16.5) 失败 | **高** | Phase H1 强制;failing 时降级为 v0.3.x release |
| AS 函数完全删除破坏 v0.3 代码 | 中 | spec §13.4 钉死;v0.3 程序改写为 `LET(alias, orig)`;migration doc |
| Phase G 质量门禁误报 | 中 | 工具配置进 workspace 顶层;clippy.toml / deny.toml / .cargo/config.toml |
| 三平台 release.yml 平台差异 | 中 | 沿用 v0.1 release.yml;Phase H 仅校验 |

---

## 2. 技术栈选型(沿用 v0.1 + v0.4 调整)

### 2.1 核心选型(沿用 v0.1)

| 维度 | 选择 | 备注 |
|------|------|------|
| **实现语言** | **Rust** | 沿用 v0.1 ADR-001 |
| **执行模型** | **Tree-walking 解释器** | 沿用 ADR-002;Phase F 评估 TCO/bytecode |
| **解析器** | **手写递归下降** | 沿用 ADR-004 |
| **AST 表示** | `enum Expr` + `Box<Expr>` + `Rc<Expr>` | 沿用 v0.1;Phase A5 加 `Cell<Option<Value>>` 闭包 cell 升级 |
| **错误处理** | `thiserror`(内部)+ 自定义 `WlwlDiagnostic` | 沿用 v0.1 D002(miette 取消);Phase A1 schema 1.1.0 升级 |
| **序列化** | `serde` + `serde_json` | 沿用;Phase A1 schema 1.1.0 加 `trace` / `cause` |
| **测试** | `cargo test` + `insta` + `cargo-llvm-cov` | 沿用;Phase G 加 `cargo-fuzz` |
| **CLI** | `clap`(v4,derive) | 沿用;Phase E2 加 `wlwl fmt` 子命令 |
| **配置文件** | `toml` + `serde` | 沿用;Phase C3-C4 加 `language_version` + MVS |
| **错误诊断** | `thiserror` 内部 + `WlwlDiagnostic` 用户层 | 沿用 D002 |

### 2.2 v0.4 新增 / 调整依赖

| 依赖 | 用途 | 引入 Phase | 备注 |
|------|------|-----------|------|
| `reqwest`(blocking) | std.ai 真实 HTTP 调用 | D1-D2 | 沿用 v0.1 ADR 备注;`WLWL_AI_API_KEY` / `WLWL_AI_ENDPOINT` env 变量 |
| `tokio`(runtime) | ASK_STREAM 流式响应(若 Phase D 选同步 + thread 方案则不需要) | D1 | 若选 spawn_blocking + sync reqwest,需要最小 tokio |
| `chrono` 或 `time` | std.time FORMAT_TIME / PARSE_TIME(§15.11.1) | B 续 | v0.1 §15.11 留到 v0.4 决策;`time` 选 0.3(无 unsafe) |
| `sha2` | lock 哈希已用 v0.1 内置;Phase G SBOM 也用 | G10 | 已内置复用 |
| `ed25519-dalek` | 中央仓库签名验证(spec §13.9 v0.5 议程) | **不引入**(v0.4 留 v0.5) | v0.4 MVS 仅本地路径,签名验证延后 |
| `cargo-llvm-cov`(dev) | 形式化覆盖率 | 沿用 | 90% 阈值 |
| `cargo-fuzz`(dev) | fuzz parser + lexer + 关键 eval | G6 | 引入 libfuzzer-sys |
| `cargo-deny`(dev) | 依赖 license / advisory / source 审计 | G2 | 引入 deny.toml |
| `cargo-miri`(dev) | unsafe 段验证(目前 codebase 0 unsafe,Phase F 引入前必跑) | G5 | nightly toolchain |
| `insta`(dev) | 快照测试 | 沿用 v0.1 | 100+ snapshots |
| `criterion`(dev) | 性能基准 | F1 | 5 个基准 |
| `cyclonedx-bom`(dev) | SBOM 生成 | G10 | CycloneDX 格式 |
| `mkdocs` + `mkdocs-material`(dev) | 文档站 | G11 | GitHub Pages |

### 2.3 决策待 Li 拍板

- [x] **实现语言**:Rust(default)— 选 Rust(2026-09-02 锁定 in v0.1 §0.1)
- [x] **执行模型**:Tree-walking(沿用)
- [x] **解析器**:hand-written(沿用)
- [x] **内存管理**:Box / Rc / Cell(沿用 v0.1 §2.3)
- [x] **HTTP 客户端**:sync reqwest + spawn_blocking(v0.1 R008 决策) — Phase D 验证
- [ ] **std.time 库**:`time` 0.3(无 unsafe,推荐)/ `chrono`(成熟,带 unsafe)/ 纯手写(spec §15.11.1 简单格式可手写)
- [ ] **strict_types 性能基线**:1.0x 阈值(不严格模式 ≥ 0% 开销)实际如何测?proptest 基准?
- [ ] **centralized registry**(`[dependencies]` version spec):v0.4 是否启用?MVS 依赖 registry 还是仅 path?默认 path-only,version spec 解析留 v0.5
- [ ] **WASM 目标**:v0.4 不引入,沿用 v0.1 §10.2 远期 TODO

---

## 3. 分阶段路线图

> **核心结构**:v0.2 共 **8 个 phase** (A-H),其中 **Phase G 为独立质量 phase**(本计划 §0.1 决策 #8),必须在 Phase H 发布前全部通过。Phase A-F 为功能/合规工作,Phase G 为质量门禁,Phase H 为发布。
>
> **v0.1 沿用基线**:Phase 1-4 + P3-007..P3-013 全部已完成;本计划 A-H 不重述 v0.1 已完成工作,只列出 v0.2 范围内新工作。

### Phase A:Spec v0.4 核心语义合规(目标 1 周)

**目标**:实现 spec v0.4 核心语义层(§2-§12)与 v0.1 实现的语义差异闭合。

**交付物**:

- A1. **错误 schema 1.1.0 升级**(§14.2)
  - `error_schema_version: "1.1.0"`(从 v0.3 的 `"0.3.1"` 升级)
  - `location` 字段从 v0.3 数组形式 `[file, line, col, line_end, col_end]` 改为对象 `{file, line, col_start, line_end, col_end}`(v0.3 旧实现已用对象形式,本次确认一致)
  - 新增 `trace: Vec<Frame>` 字段(§14.2):实现栈式 frame,最少 1 帧,建议 ≥ 32 帧
  - 新增 `cause: Option<DICT-or-STRING>` 字段(§14.2):由 `WRAP` 形成链
  - 新增 `retry_after: Option<INTEGER>` 字段(§14.2):毫秒,NULL 或 INTEGER,仅 `retryable: TRUE` 时有意义
  - 新增 `idempotent: BOOLEAN` 字段(§14.2):重试产生与首次相同结果则 TRUE
  - AI 工具兼容性:旧工具忽略新字段(serde default = None / false)

- A2. **闭包 cell 语义**(§6.4)
  - `LET` 引入的名字创建 `Cell<Option<Value>>` 单元
  - 闭包捕获 cell **引用**而非值副本
  - 闭包内 `SET(count, ...)` 修改 cell,所有共享 cell 的闭包都看得到
  - 未被子作用域捕获的 cell,子作用域 `SET` → `E0024`(本计划新增错误码)
  - 现有 D006 / D016 闭包独立性测试需要重新评估:v0.3 闭包 deep-clone 行为是**与 v0.4 矛盾的**,本 phase 修正

- A3. **解构绑定**(§7.5)
  - `LET([a, b, *rest], [1, 2, 3, 4])` 数组解构 → a=1, b=2, rest=[3,4]
  - `LET(["x": x, "y": y], ["x": 1, "y": 2])` 字典解构
  - `LET([_, x, _], [10, 20, 30])` `_` 占位
  - 嵌套解构:`LET([[a, b], [c, d]], [[1, 2], [3, 4]])`
  - 解构失败(数组长度不足 / 字典缺键)→ `E0026`

- A4. **`MATCH` 模式匹配**(§7.6)
  - `MATCH(value, clauses, default?)` 语法
  - 模式形态:字面量 / 构造器 `OK(x)` / `ERR(e)` / 通配符 `_` / 数组 / 字典 / 嵌套
  - 首个匹配胜出;无匹配无 default → `E0027`
  - 模式绑定仅在对应 result 内可见

- A5. **`RESULT` 一等值类型**(§2.2.1)
  - `OK(v)` / `ERR(e)` 返回 `RESULT` 类型
  - `TYPE(OK(1))` 返回 `"RESULT"`(已实现)
  - `ERR(e)` 强制 `e` 是 STRING 或 DICT(违反 → `E0030`)
  - `OK` / `ERR` 是构造器宏(不是关键字)——已实现,需在 std-error / parser 标注
  - `IS_OK` / `IS_ERR` / `OR_DIE` / `UNWRAP_OR` 行为确认(已实现)

- A6. **ERR 消费者注册表**(§12.7)
  - v0.3 的"白名单"改为"注册表"模型
  - 注册表初始 9 项:`IS_OK` / `IS_ERR` / `OR_DIE` / `UNWRAP_OR` / `TRY` / `UNWRAP` / `ERR_PAYLOAD` / `WRAP` / `TYPE` / `=` / `!=` / `IF`
  - 实现 `consumer_registry: HashSet<String>` 数据结构,call_with_args 检查函数是否在注册表
  - 实际不消费 ERR 的白名单扩展需 spec 升级,实现不得私自扩展(§12.7 末段)

- A7. **数值与跨类型语义**(§9.5)
  - 整除截断:`/(7, 2) = 3` / `/(-7, 2) = -3`
  - 整数溢出饱和到 `INT64_MAX` / `INT64_MIN`,产生 `W0015`
  - `NEG(INTEGER_MIN)` → `E0034`
  - `/(FLOAT, 0.0)` → `E1003`(即使 IEEE 754 允许 Inf)
  - `FLOAT → INTEGER` 越界 → `E0035`
  - `=(1, 1.0)` 视为相等(本规范新增)
  - 现有 v0.3 算术测试需要重新评估截断行为

- A8. **比较返回类型规则**(§9.2)
  - 两操作数均非 ERR → BOOLEAN
  - 任一操作数是 ERR → 透明传播(已在白名单内,行为不变)
  - 文档化,无需 impl 改动(本项作为 spec 澄清 commit)

**规范对应**:§2.2.1, §2.7, §6.3-§6.4, §7.5-§7.6, §8.4, §9.2, §9.4-§9.5, §12.2, §12.6-§12.7, §14.2

**不在本阶段**:
- v0.1 已实现的 LET/IF/WHILE/FOR/RETURN/BREAK/CONTINUE 核心控制流(保留不破坏)
- OK/ERR/PANIC/TRY/IS_OK/IS_ERR/OR_DIE 已实现(本阶段仅文档化变更)
- 下标访问/字符串/字典/集合内建操作 → Phase B
- 模块系统/锁文件/命名空间 → Phase C
- std.ai 真实集成 → Phase D
- strict_types / formatter / AST stable ID → Phase E
- 性能 → Phase F
- 质量门禁 → Phase G
- 一致性测试套件 → Phase H

**任务分解**:

| 任务 | 周期 | 备注 |
|------|------|------|
| 闭包 cell 重构 + 现有测试重写 | 2 天 | D006 / D016 测试重评估 |
| 解构绑定 parser + eval | 1 天 | §7.5 5 个子模式 |
| MATCH parser + eval | 1 天 | §7.6 模式形态 |
| 错误 schema 1.1.0 升级 | 1 天 | location 对象化已就位,新增字段 |
| ERR 消费者注册表化 | 0.5 天 | 4 → 9 项 |
| 数值语义 §9.5 | 0.5 天 | 整除 / 溢出 / 跨类型 |
| RESULT 类型注解 + 现有测试 | 0.5 天 | TYPE 返回值测试 |
| **Phase A 缓冲** | **1 天** |  |

---

### Phase B:Spec v0.4 内建/库扩展(目标 1.5 周)

**目标**:把 v0.1 已实现的内建/库扩到 spec v0.4 全量;新增 `std.collection` / `std.format` / `std.test` 三个模块;补 spec §3.4 全部宏函数注册表。

**交付物**:

- B1. **`INDEX_GET` / `INDEX_SET` / `AT` 下标访问**(§10.1 / §10.2)
  - `INDEX_GET(arr, i)`:`-LEN(arr) ≤ i < LEN(arr)`,越界 → `E0036`
  - `INDEX_SET(arr, i, v)`:同上,越界 → `E0036`
  - `INDEX_GET(d, k)`:键不存在 → `E0037`
  - `INDEX_SET(d, k, v)`:插入或更新
  - `AT(arr_or_d, k, default)`:越界 / 缺键返回 default
  - `arr[i]` 语法糖 = `INDEX_GET(arr, i)`
  - `arr[i] = v` 语法糖 = `INDEX_SET(arr, i, v)`
  - `i` 非 INTEGER → `E0031`
  - **`POP(d, k, default)` 字典版**(§10.2)新增

- B2. **`REMOVE_KEY` 重命名**(§10.2)
  - 内部实现重命名为 `REMOVE_KEY`
  - `DEL` 保留为 v0.3 兼容别名,使用 `DEL` 触发 `W0051` 弃用警告
  - `wlwl.toml` `language_version` 字段校验:v0.4 程序不再使用 `DEL`

- B3. **`UNWRAP_OR` 主推**(§12.2)
  - 内部实现重命名 `OR_DIE` → `UNWRAP_OR`
  - `OR_DIE` 保留为 v0.3 兼容别名,使用触发 `W0051` 警告
  - 文档与示例优先用 `UNWRAP_OR`

- B4. **`UNWRAP` / `ERR_PAYLOAD` / `WRAP` 错误链原语**(§12.2)
  - `UNWRAP(OK(v)) → v`
  - `UNWRAP(ERR(e)) → PANIC E0100`(cause=e)
  - `UNWRAP(non-RESULT) → E0030`
  - `ERR_PAYLOAD(ERR(e)) → e`
  - `ERR_PAYLOAD(OK(v)) → E0030`
  - `WRAP(ERR(e), ctx) → ERR(["original": e, "context": ctx])`
  - `WRAP(OK(v), _) → OK(v)`
  - `cause` 字段通过 `WRAP` 链构建

- B5. **`FORMAT` 字符串格式化 + `std.format` 模块**(§10.6 / §15.8)
  - `FORMAT(template, args...)` 全大写宏函数
  - 模板语法:`{0}` / `{1}` / `{name}` 占位
  - 模板解析失败 → `E0039`
  - `args[i]` 非 STRING / 非 DICT → `STR(args[i])` 转换
  - 归属模块:`wlwl:std.format`

- B6. **`std.collection` 高阶集合函数**(§15.7)
  - 16 个函数全部就位:`MAP` / `FILTER` / `REDUCE` / `SORT` / `SORT_BY` / `ZIP` / `RANGE` / `ANY` / `ALL` / `FIND` / `ENUMERATE` / `TAKE` / `DROP` / `FLAT` / `UNIQ` / `GROUP_BY` / `JOIN`
  - `RANGE` step=0 → `E0038`
  - 不作为全局内建,必须 `IMPORT("wlwl:std.collection", ...)` 引入
  - ERR 透明传播对 collection 函数也生效

- B7. **`std.test` 内建测试框架**(§15.9)
  - `TEST(name, body)` / `ASSERT(cond, msg?)` / `ASSERT_EQ(a, b, msg?)` / `ASSERT_NEQ(a, b, msg?)` / `EXPECT_ERR(expr)` / `RUN_TESTS()`
  - 新增错误码:`E0046` / `E0047` / `E0048` / `E0049`
  - `ASSERT` 失败抛 `ERR(["code": "E0046", ...])`,`RUN_TESTS` 用 `TRY` 捕获每条
  - `RUN_TESTS` 返回 ARRAY of DICT(`name` / `passed` / `duration_ms` / `error?`)

- B8. **字符串内建扩展**(§10.3)
  - 新增:`TRIM` / `TRIM_START` / `TRIM_END` / `STARTS_WITH` / `ENDS_WITH` / `REPEAT` / `PAD_START` / `PAD_END` / `CODEPOINTS` / `FROM_CODEPOINTS`
  - `FLOAT(s)` 转换函数
  - 非 ASCII `UPPER` / `LOWER` 行为实现定义,缺实现时 emit `W0014`

- B9. **`NOT` 宏函数名**(附录 G 末注)
  - v0.3 用 `!` 单字符(可能与 §3.3 标点冲突)
  - v0.4 改为 `NOT(a, b, ...)` 全大写宏函数名
  - `!` 单字符保留为 v0.3 兼容别名,使用触发 `W0054`
  - 空参 `NOT()` 返回 `TRUE`(vacuous truth)

- B10. **`PRINT_ERR`**(§15.2 / 附录 G)
  - v0.3 PRINT 已存在
  - v0.4 新增 `PRINT_ERR` 写 stderr
  - 全局内建 + `wlwl:std.io` 暴露

- B11. **更新附录 G 全局内建注册表实现**
  - 每个内建函数的 `is_in_global_registry: bool` 标志
  - `wlwl check` / `wlwl run` 启动时检查
  - 非注册表函数 ID 报错 E0020(已实现)

**规范对应**:§3.4, §10.1-§10.6, §12.2, §15.2-§15.9, 附录 G

**不在本阶段**:
- `MODULE_REF` 模块作为值 → Phase C2
- `std.ai` 真实集成 → Phase D
- `std.agent` → Phase D3
- `std.os` / `std.time` 完整签名 → B 续(独立 phase,本计划归到 Phase H 一致性测试范围)

**任务分解**:

| 任务 | 周期 | 备注 |
|------|------|------|
| `INDEX_GET` / `INDEX_SET` / `AT` + 语法糖 | 1.5 天 | E0036 / E0037 / E0031 三个新码 |
| `REMOVE_KEY` 重命名 + `DEL` 别名 + W0051 | 0.5 天 | 警告码基础设施先就位 |
| `UNWRAP_OR` 重命名 + `OR_DIE` 别名 + W0051 | 0.5 天 |  |
| `UNWRAP` / `ERR_PAYLOAD` / `WRAP` + cause 链 | 1 天 | 错误链原语 |
| `FORMAT` + `std.format` 模块 | 1 天 | E0039 模板解析 |
| `std.collection` 16 个高阶函数 | 2 天 | 最大单模块工作 |
| `std.test` 框架 + E0046-E0049 | 1.5 天 | 测试框架设计 |
| 字符串扩展 10 个 + `FLOAT` + `NOT` | 1 天 |  |
| `PRINT_ERR` + 注册表实现 | 0.5 天 |  |
| **Phase B 缓冲** | **1 天** |  |

---

### Phase C:Spec v0.4 模块系统与配置(目标 0.5 周)

**目标**:完成 spec §13 模块系统的 v0.4 修订(AS 删除 / language_version / MVS / 命名空间精化 / 错误码补漏)。

**交付物**:

- C1. **`AS` 函数完全删除**(§13.4)
  - v0.3 的 `AS("y", "x")` 形式完全删除
  - 实现 `AS` 不再是宏函数 / 全局内建
  - 引用 `AS` → `E0020` 未定义
  - 迁移文档:v0.3 `AS(name, alias)` → v0.4 `LET(alias, name)`

- C2. **`MODULE_REF` 模块作为值**(§13.12)
  - `LET(m, MODULE_REF("wlwl:std.io"))` 加载模块但**不绑定名字**
  - `m.PRINT("hi")` 通过 `CALL_METHOD(m, "PRINT", ["hi"])` 调用
  - 实现 `WlwlModule` 类型(类值 / 模块对象的 DICT 形式)
  - 不污染全局命名空间

- C3. **`language_version` 字段**(§13.8)
  - `wlwl.toml` 的 `[package]` 必填字段
  - 加载时校验:`language_version` 与实现支持版本比较
  - 不匹配 → `E0044: language_version mismatch: package requires X.Y, implementation supports A.B`

- C4. **MVS 依赖求解**(§13.9)
  - Cargo 风格 MVS:对每个依赖,选择**满足所有约束的最低版本**
  - 冲突(无版本满足所有约束)→ `E0045: dependency conflict`
  - v0.4 仅本地 path 依赖 + lock 文件;中央仓库 registry 留 v0.5

- C5. **`allow_builtin_shadow` 标志**(§6.6 / §13.8)
  - `wlwl.toml` 字段,默认 `false`
  - `false` 时遮蔽全局内建 → `E0025`
  - `true` 时遮蔽允许但 emit `W0030`
  - 同样适用于遮蔽宏函数 / 关键字(产生 `W0030`)

- C6. **`E0042` lock-toml 不一致**(§13.8 / §14.4)
  - 加载时检查 `wlwl.lock` 与 `wlwl.toml` 一致
  - 不一致 → `E0042: lock file inconsistent with wlwl.toml`

- C7. **跨目录引用项目根边界强化**(§13.5)
  - v0.1 P3-015 推迟项
  - ModuleLoader::load 跨目录实现已就位(沿用 v0.3 batch 2)
  - 本 phase 强化:项目根边界外访问 → `E0040`,命名空间解析统一
  - 新增 spec §13.5 v0.4 路径优先级测试

**规范对应**:§6.6, §13.4, §13.5, §13.8-§13.9, §13.12, §14.4

**不在本阶段**:
- 中央仓库协议(registry)→ 留 v0.5
- 镜像 / 抢注 / 签名验证 → 留 v0.5

**任务分解**:

| 任务 | 周期 | 备注 |
|------|------|------|
| `AS` 删除 + 迁移文档 | 0.5 天 |  |
| `MODULE_REF` + 类值语法 | 1 天 | §13.12 |
| `language_version` 字段 + E0044 | 0.5 天 |  |
| MVS 依赖求解 + E0045 | 1.5 天 | 算法骨架已在 spec §13.9.1 |
| `allow_builtin_shadow` + E0025 | 0.5 天 |  |
| `E0042` lock-toml 一致性 | 0.5 天 |  |
| 跨目录强化 + 项目根边界测试 | 0.5 天 |  |
| **Phase C 缓冲** | **0.5 天** |  |

---

### Phase D:Spec v0.4 std.ai 真实集成(目标 1 周)

**目标**:把 v0.1 mock std.ai 替换为真实 HTTP 集成;新增 std.agent 薄包装;细分网络错误码。

**交付物**:

- D1. **`ASK_STREAM` 真实 HTTP 流式**(§15.13.4)
  - v0.3 mock(P3-012):callback 接受并 ignore
  - v0.4 真实:用 `reqwest` blocking + `read_chunk` / 流式 callback
  - 同步阻塞(在 spawn_blocking 中执行)
  - 错误码:`E0080` 超时 / `E0081` 响应超长 / `E0082` 凭据缺失 / `E0083` 模型未找到(已实现触发点)
  - `WLWL_AI_API_KEY` / `WLWL_AI_ENDPOINT` / `WLWL_AI_DEFAULT_MODEL` env 变量配置

- D2. **`ASK_ALL` per-element OK/ERR**(§15.13.5)
  - v0.3 mock:整体 reject(任一非 string → `E0030`)
  - v0.4 真实:每条 prompt 独立 OK/ERR,收集为 ARRAY
  - 同步顺序执行(并发留 v0.5 §17.4 决策后)

- D3. **`std.agent` 薄包装**(§15.14)
  - `TASK(goal, tools, opts)`:v0.4 实现 = `ASK(model, build_prompt(goal, tools, opts), opts)` 薄包装
  - `TOOL(name, description, params, body)`:返回 TOOL 对象
  - `CALL_TOOL(tool, params)`:同步执行
  - `MODEL(provider, name)`:显式构造模型对象
  - `CONTEXT(files, symbols, diagnostics)`:构造 Agent 上下文(签名 v0.5 待定,本 phase 仅 stub)

- D4. **网络错误码细分**(§14.4)
  - v0.3 `E0090` 网络不可达 → 细分为 `E0090` / `E0091` DNS 失败 / `E0092` TLS 错误 / `E0093` HTTP 4xx / `E0094` HTTP 5xx
  - `retryable` 标记正确:`E0090` / `E0091` / `E0094` = TRUE;`E0092` / `E0093` = FALSE

- D5. **模型名格式校验**(§15.13.1 / 警告 W0052)
  - 强烈建议 `provider/model_name` 形式(`"openai/gpt-4"`)
  - v0.4 兼容裸模型名如 `"gpt-4"`,但产生 `W0052` 警告

**规范对应**:§14.4, §15.13.1-§15.13.5, §15.14

**不在本阶段**:
- 异步 ASK_STREAM / ASK_ALL(并发模型 v0.5 §17.4 决策)
- 中央仓库(registry)协议

**任务分解**:

| 任务 | 周期 | 备注 |
|------|------|------|
| `reqwest` 依赖引入 + sync API 封装 | 0.5 天 |  |
| `ASK` 真实 HTTP(替换 mock) | 1 天 | v0.1 已 mock,本 phase 接真实 |
| `ASK_STREAM` 真实流式 | 1.5 天 | 核心 |
| `ASK_ALL` per-element 重构 | 1 天 | v0.3 整体 reject → v0.4 独立 |
| `std.agent` 5 个函数 stub + 文档 | 0.5 天 | v0.4 薄包装 |
| 网络错误码细分 E0090-E0094 | 0.5 天 |  |
| 模型名格式 W0052 | 0.5 天 |  |
| **Phase D 缓冲** | **1 天** | 网络 / 凭据 / mock 兼容 |

---

### Phase E:Spec v0.4 严格类型与工具(目标 1 周)

**目标**:实现 `strict_types` 运行时边界检查、`canonical formatter`、AST 稳定 node ID;把 v0.1 推迟的 W0020 linter walk 收尾。

**交付物**:

- E1. **`strict_types` 运行时边界检查**(§2.7)
  - `wlwl.toml` 的 `[features] strict_types = true` 字段
  - 函数入口(public API)边界检查:形参类型注解与实参 `TYPE` 不一致 → `E0033`
  - IMPORT 边界检查:导入符号的实际值与契约声明类型不一致 → `E0033`
  - 不开 strict_types:行为与 v0.3 完全一致
  - 性能预算:严格模式 ≤ 10% 开销(spec §2.7 末段)
  - 实现层 benchmark 验证(用 `cargo bench`)

- E2. **`wlwl fmt` canonical formatter**(§16.3)
  - 10 条规范化规则(附录 G 末注)
  - 实现 `wlwl fmt <file>` 子命令
  - 不符合 §16.3 的实现产生 `W0053` 警告
  - 格式化输出 idempotent(`fmt(fmt(x)) = fmt(x)`)
  - **风格修正**:4 空格缩进 / ≤ 100 字符行长度 / `;` 必填 / 逗号无尾随 / 多语句块折行

- E3. **AST 稳定 node ID**(§16.4.1)
  - 每个 AST 节点带 `node_id`(`module:fn:body:kind-index`)与 `hash`(sha256)
  - 用于 AI 多轮编辑的并发冲突检测
  - JSON 序列化格式:每节点含 `node_id` / `kind` / `span` / `hash` / 子节点
  - 影响 `wlwl ast --format=json` 输出

- E4. **W0020 linter 端独立 walk**
  - v0.1 P3-013 已实现 parser 端 `parse_array_or_dict` 状态机 + emit W0020
  - 本 phase:加 `pub fn lint(expr: &Expr) -> Vec<Warning>` post-parse walk
  - 暴露给 `wlwl check` 子命令
  - 集成所有 W 码(W0010-W0054)统一通道
  - `parse_with_warnings` 已就位(沿用 v0.1 P3-011),本 phase 增强

**规范对应**:§2.7, §6.6, §16.3, §16.4

**不在本阶段**:
- AST 重写(suggestion_code 的 `ast_rewrite` 字段)— 留 v0.5
- SARIF 输出格式 — 留 v0.5 评估
- LSP 协议完整实现 — 留 v0.5+

**任务分解**:

| 任务 | 周期 | 备注 |
|------|------|------|
| `strict_types` 边界检查 + E0033 | 1.5 天 | 性能 benchmark 同步做 |
| `wlwl fmt` 实现 + 10 条规则 | 2 天 |  |
| AST 稳定 node ID + hash | 1 天 |  |
| W0020 linter walk + 全部 W 码统一通道 | 1 天 |  |
| **Phase E 缓冲** | **1 天** |  |

---

### Phase F:性能优化(目标 0.5 周,v0.1 推迟项)

**目标**:把 v0.1 推迟的"性能 TCO + hot-inline"工作收尾,达到 spec §2.7 末段 ≤ 10% 开销目标;为 Phase G 性能回归测试建立 baseline。

**交付物**:

- F1. **性能基准建立**
  - `cargo bench` 框架引入
  - 5 个基准测试:简单循环 100万次 / 闭包密集调用 / 字符串拼接 / ARRAY 高阶函数 / 错误传播密集
  - 目标:简单循环 < 30 秒(spec §6.6 v0.3 沿用)
  - Linux/macOS 基准机统一

- F2. **解释器热点内联**
  - `eval_expr` 主循环热点路径分析(`cargo flamegraph`)
  - 关键内建函数(算术 / 比较 / LEN / PUSH)内联标注
  - 闭包 cell 共享减少 Rc 拷贝

- F3. **Cargo release profile 调优**
  - `[profile.release]` 已配 `lto = "thin"` / `codegen-units = 1`
  - 评估:codegen-units = 16(LTO thin 配合)是否更快
  - 评估:`[profile.bench]` debug = true / opt-level = 3
  - 评估:`[profile.release.package."*"]` opt-level = 3 是否对小 crate 提升

- F4. **闭包 cell 共享优化**
  - v0.3 闭包 deep-clone(D006)— v0.4 cell-based 重构后(Phase A2)不再有此开销
  - 但需要 benchmark 验证 cell 引用计数 < deep-clone

- F5. **错误码 / 警告码性能**
  - schema 1.1.0 升级(A1)新增 trace/cause 字段 — trace 构造开销需评估
  - 仅 `retryable: TRUE` 时计算 `retry_after`
  - `idempotent` 是静态属性(查表),无开销

**规范对应**:附录 E 形式化操作语义的高效实现;spec §2.7 性能预算

**不在本阶段**:
- 字节码 VM(完全替代 tree-walking)→ 留 v0.5 评估
- WASM 后端 → 留 v0.5+

**任务分解**:

| 任务 | 周期 | 备注 |
|------|------|------|
| `cargo bench` 框架 + 5 个基准 | 1 天 |  |
| 解释器热点内联 + flamegraph | 1 天 |  |
| Release profile 调优 | 0.5 天 |  |
| 闭包 cell 共享验证 | 0.5 天 |  |

---

### Phase G:代码质量(目标 1 周,本计划独立追加的强制 phase)

**目标**:本 phase **独立**于功能工作,作为发布 v0.4.0 的**质量门禁**。Phase H 发布前必须全部通过。任何 G 子项失败,Phase H 不能发布,必须修复后重新跑 G 全套门禁。

**核心原则**:
- **门禁是 hard gate**,不是 nice-to-have
- G 失败 → Phase H 阻塞 → 不发 v0.4.0
- 每个 G 子项的工件(`.cargo/config.toml` / `clippy.toml` / `deny.toml` / `fuzz/` / `benches/` / `docs/`)必须 **入版本控制**
- CI 必须跑 G 全套门禁,任一失败 PR 不能 merge

**交付物**:

- **G1. `rustfmt` + `clippy` zero warning**
  - `cargo fmt --check` 通过
  - `cargo clippy --workspace --all-targets -- -D warnings` 通过(零警告)
  - `clippy.toml` workspace 顶层:`# deny` + `# warn` 等级定制
  - CI 集成:每次 push 必跑

- **G2. `cargo-deny` 依赖审计**
  - `deny.toml` workspace 顶层
  - **licenses** 段:禁止 GPL-incompatible 依赖(`Unicode-DFS-2016` / `MIT` / `Apache-2.0` / `BSD-*` 通过)
  - **advisories** 段:`database = "RustSec"` 拉取 + 失败即报错
  - **bans** 段:禁止 `serde_json` 之外的 JSON 库(simd-json)/ 禁止 `chrono` 若选 `time` 0.3
  - **sources** 段:仅 crates.io + workspace local
  - `cargo deny check` 通过

- **G3. `rustdoc` 100% public API 文档覆盖**
  - `cargo doc --workspace --no-deps --document-private-items=false` 通过
  - `#![warn(missing_docs)]` workspace 顶层(每个 crate `lib.rs`)
  - 现有代码补全 rustdoc 注释
  - `///` doc-comment 格式符合 spec §3.6
  - 第三方 `wlwl-std` 等每个 `EXPORT` 函数有 `# 错误:` + `# 示例:` 段

- **G4. ADRs for v0.4 设计选择**
  - `docs/adr/` 目录创建(沿用 v0.1 §12 风格)
  - 至少 6 个新 ADR:
    - **ADR-008**:闭包 cell 升级(§6.4)— 为何选 cell-based 而非 explicit `REF` 关键字
    - **ADR-009**:ERR 消费者注册表(§12.7)— 为何注册表取代封闭白名单
    - **ADR-010**:`strict_types` 行为(§2.7)— Transient cast insertion 而非 Natural monitoring
    - **ADR-011**:MVS 依赖求解(§13.9)— 为何选 Cargo 风格而非 PubGrub
    - **ADR-012**:`AS` 函数删除(§13.4)— 为何完全删除而非保留别名
    - **ADR-013**:`canonical formatter` 行为(§16.3)— 为何必规范化

- **G5. `cargo-miri` unsafe 段验证**
  - 当前 codebase **0 unsafe**(沿用 v0.1)— baseline 验证 `cargo miri test` 通过
  - Phase F 引入 unsafe 段前必跑 miri
  - nightly toolchain 集成 CI(可选,加 `[toolchain]` 标注)
  - `unsafe` keyword 全仓搜索,确认仅依赖 crate 内部使用(如 `parking_lot` 内部)

- **G6. `cargo-fuzz` fuzz harness**
  - `fuzz/` 目录:3 个 fuzz target
    - `fuzz_parser`:随机字节流喂 lexer + parser,验证不 panic / 不 infinite loop
    - `fuzz_eval`:合法 AST 随机化,验证 eval 不 panic / 不违反 ERR 透明传播
    - `fuzz_std`:std.* 模块边界条件 fuzz(空字符串 / 超大数组 / NaN / null bytes)
  - `cargo +nightly fuzz run <target>` 跑 ≥ 10 分钟(corpus 保留)
  - corpus 入仓(`fuzz/corpus/`)作 regression baseline

- **G7. 错误信息质量 pass**
  - 每个错误码(56 个)+ 警告码(14 个)都有:
    - 人类可读输出(`--format=human`)单测
    - JSON 输出(`--format=json`)单测
    - JSONL 输出(`--format=jsonl`)单测
  - v0.1 P3-008 已加 per-site `Suggestion::Note` for 35 码,本 phase 扩到全 56 + 14
  - `suggestion_code` 内容质量验证:每条 suggestion 可被 AI 工具自动 apply 成功(用 v0.1 §14.7 AI 契约脚本)

- **G8. 性能回归测试**
  - 5 个基准测试(Phase F1 已建立)入 CI
  - `cargo bench` baseline 写入 `benches/baseline.txt`
  - CI fail if bench > 110% baseline
  - simple loop 100万次 < 30 秒持续监控

- **G9. 依赖供应链监控**
  - `deps.rs` README badge
  - GitHub Dependabot 启用(每周一扫描)
  - `cargo audit` 入 CI

- **G10. SBOM + 签名 release 制品**
  - `cargo cyclonedx` 生成 SBOM(CycloneDX 格式)
  - release.yml 加 step:`- name: Generate SBOM` / `- name: Sign artifacts (cosign)`
  - SHA-256 checksums `SHA256SUMS` 入 release

- **G11. mkdocs 文档站(v0.1 P3-014 推迟项)**
  - `mkdocs.yml` 顶层
  - `docs/` 目录结构:
    - `index.md`(首页)— spec v0.4 摘要 + 链接
    - `spec/`:符号链接到 `docs/standard/wlwl-spec-v0.4`
    - `plan/`:符号链接到 `docs/plan/wlwl-build-plan-v0.2`
    - `history/`:符号链接到 `docs/history/`
    - `tutorial/`:**新增** — 从 Hello World 到 OOP / 模块 / 错误处理 / std.test 完整教程
    - `examples/`:≥ 5 个完整 .wl 示例(从 v0.1 showcase 扩)
  - GitHub Pages 部署:release.yml 触发 `mkdocs gh-deploy`
  - `material` 主题

- **G12. README / CHANGELOG / CONTRIBUTING polish + examples 套件**
  - README:从 v0.1 polish,加 v0.4 spec 摘要 + 错误码表 + examples 链接
  - CHANGELOG:Keep-a-Changelog 格式,v0.4.0 完整条目
  - CONTRIBUTING:v0.1 已就位,加 Phase G 质量门禁说明
  - examples/:`hello.wl` / `math.wl` / `showcase.wl`(沿用) + 新增 `match.wl` / `destruct.wl` / `std_test.wl` / `closure_cell.wl` / `format.wl`(v0.4 特性展示)

**规范对应**:本 phase 不直接对应 spec 章节,而是 spec §16.5 一致性测试套件的基础设施 + spec §0.4 Conformance 的工程保障。

**任务分解**:

| 任务 | 周期 | 备注 |
|------|------|------|
| G1 clippy + rustfmt | 1 天 |  |
| G2 cargo-deny | 0.5 天 |  |
| G3 rustdoc 100% | 1 天 |  |
| G4 ADR 6 个 | 1 天 |  |
| G5 miri 验证 | 0.5 天 | 0 unsafe,主要流程演练 |
| G6 cargo-fuzz 3 target | 1.5 天 |  |
| G7 错误信息质量 pass 70 个码 | 1 天 | 56 + 14 |
| G8 性能回归 | 0.5 天 | 沿用 F1 基准 |
| G9 供应链监控 | 0.5 天 |  |
| G10 SBOM + 签名 | 0.5 天 |  |
| G11 mkdocs | 1 天 |  |
| G12 README + examples | 1 天 |  |
| **Phase G 集成 + CI 配置** | **1 天** | 全部子项入 CI.yml |
| **Phase G 缓冲** | **1 天** |  |

---

### Phase H:发布(目标 0.5 周)

**目标**:把 spec v0.4 一致性测试套件跑通,跨平台验证,发布 v0.4.0 三平台 native binary。

**交付物**:

- **H1. 一致性测试套件**(§16.5)
  - `tests/core.wlt` 完整测试用例(spec §16.5 第 1-10 项)
  - `tests/conformance.py` / `tests/conformance.sh` 运行脚本
  - 全部 10 项覆盖:
    1. 核心子集(§1-§10 每条规则)
    2. ERR 透明传播(每种 §12.7 注册表函数)
    3. 下标边界(INDEX_GET / INDEX_SET 越界 / 空数组 / 空 DICT)
    4. 数值语义(§9.5 每条规则)
    5. 模式匹配(MATCH 各种模式)
    6. 解构绑定(§7.5 每种模式)
    7. 闭包可变性(§6.4 cell 语义 / 捕获变量 SET / 非捕获变量 SET 报错 E0024)
    8. 模块路径(三种路径形式 / 循环导入 / MVS 求解)
    9. FORMAT 模板(各种占位符 / 模板解析失败 E0039)
    10. 错误 schema 一致(附录 B JSONL 示例逐字段比对)
  - **发布要求**:实现不通过不得自称 "v0.4 compliant"

- **H2. 跨平台 CI 验证**
  - Linux x86_64:`cargo test --workspace` + `cargo llvm-cov --workspace` + 一致性套件
  - macOS universal(arm64 + x86_64):同上
  - Windows MSVC:同上
  - release.yml:trigger on `v*.*.*` tag push,跨平台构建
  - smoke job(v0.1 已就位)— 加 v0.4 一致性套件 step

- **H3. tag v0.4.0 + push**
  - `git tag -a v0.4.0 -m "v0.4.0: spec v0.4 compliance + Phase G quality gate"`
  - `git push origin v0.4.0`
  - tag 触 release.yml

- **H4. release.yml 异步构建**
  - 三平台 binary:Linux x86_64 / macOS universal / Windows MSVC
  - release artifacts:`wlwl-{platform}-{arch}.tar.gz` / `wlwl-{platform}-{arch}.zip`
  - SHA-256 checksums
  - SBOM(CycloneDX)— Phase G10
  - 签名(cosign)— Phase G10

- **H5. GitHub Release 制品**
  - release notes:v0.4.0 完整变更(spec §0.2 + Phase A-H 摘要)
  - 三平台 binary 下载链接
  - SBOM + checksums
  - "What's Changed" — auto-generated from PRs since v0.3.0
  - breaking changes 段(AS 删除 / 错误码增量 / schema 1.1.0 升级)
  - migration 段:v0.3 → v0.4 程序迁移指南

- **H6. Post-release smoke + 监控**
  - download 三平台 binary,各跑 `hello.wl` + 1 个示例
  - 监控 GitHub Issues
  - 7 天后,若无 P0 bug,标记 v0.4.0 stable

**规范对应**:§0.4 Conformance, §16.5 一致性测试套件

**任务分解**:

| 任务 | 周期 | 备注 |
|------|------|------|
| H1 一致性测试套件 10 项 | 1.5 天 | **最大** |
| H2 CI 跨平台 + release.yml | 0.5 天 | 沿用 v0.1 |
| H3 tag + push | 0.1 天 |  |
| H4 release.yml 异步构建 | 异步(无需本地时间) |  |
| H5 GitHub Release 制品 + notes | 0.5 天 |  |
| H6 Post-release 监控 | 持续 |  |

---

### 总时间估算

| Phase | 目标周期 | 累计 | 状态 |
|-------|----------|------|------|
| v0.1 baseline(沿用) | 0 周(已完成) | 0 | ✅ 13/13 crate ≥ 90% line, 522 tests |
| Phase A:核心语义 | 1 周 | 1 |  |
| Phase B:内建/库扩展 | 1.5 周 | 2.5 |  |
| Phase C:模块系统 | 0.5 周 | 3 |  |
| Phase D:std.ai 真实 | 1 周 | 4 |  |
| Phase E:严格类型 + 工具 | 1 周 | 5 |  |
| Phase F:性能 | 0.5 周 | 5.5 |  |
| **Phase G:质量(独立 phase)** | **1 周** | **6.5** |  |
| Phase H:发布 | 0.5 周 | 7 |  |
| **总计(估算)** | **7 周** |  |  |

**与 v0.1 实际对比**(参考):v0.1 估算 20 周(Phase 1-4)+ 4 周(Phase 5)= 24 周,实际 1 个工作日(2026-09-03)集中完成。AI Coding workflow 加速远超估算,本计划估算**仅供参考**。实际推进时,各 phase 仍按 git commit 批次切分,每批结束 `cargo test --workspace` + `cargo llvm-cov --workspace` + 13/13 ≥ 90% line 基线不倒退。

> **质量门禁硬约束**:**Phase G 不通过,Phase H 不能发布 v0.4.0**。这是 Li 在 §0.1 决策 #8 锁定的硬规则。

---

## 4. 关键模块划分(基于 Rust crate 结构)

### 4.1 顶层 crate(v0.1 沿用,v0.4 调整)

```text
wlwl/
├── docs/
│   ├── standard/              # WLWL 规范(v0.4 现行)
│   ├── history/               # 历史归档
│   ├── adr/                   # [v0.2 新增]Architecture Decision Records (G4)
│   └── plan/                  # 构建计划
└── impl/                      # Rust workspace 根(沿用 v0.1)
    ├── Cargo.toml
    ├── crates/
    │   ├── wlwl-lexer/        # 词法分析
    │   ├── wlwl-parser/       # 语法分析 → AST
    │   ├── wlwl-ast/          # AST 定义(共享类型) + Phase A2 Cell
    │   ├── wlwl-eval/         # 求值器(tree-walking) + Phase A2 cell 升级
    │   ├── wlwl-error/        # 错误定义 + schema 1.1.0 (Phase A1)
    │   ├── wlwl-std/          # std.io / fs / json / ai / collection / format / test / string / math / time / os / agent
    │   ├── wlwl-toml/         # manifest / lock / MVS (Phase C3-C4)
    │   ├── wlwl-formatter/    # [v0.2 新增]Phase E2 canonical formatter
    │   ├── wlwl-fuzz/         # [v0.2 新增]Phase G6 fuzz harness
    │   └── wlwl-cli/          # run / check / ast / fmt(新增) / repl(可选)
    ├── tests/                 # 集成测试 + Phase H1 一致性套件
    ├── examples/              # ≥ 5 个示例 + Phase G12 新增 5 个
    ├── benches/               # [v0.2 新增]Phase F1 性能基准
    ├── fuzz/                  # [v0.2 新增]Phase G6 fuzz targets
    ├── .cargo/                # [v0.2 新增]Phase G 工具配置
    ├── clippy.toml            # [v0.2 新增]Phase G1
    ├── deny.toml              # [v0.2 新增]Phase G2
    ├── mkdocs.yml             # [v0.2 新增]Phase G11
    ├── LICENSE                # GPL v2
    └── README.md
```

### 4.2 核心数据结构(v0.1 沿用,v0.4 关键修订)

```rust
// AST 节点(wlwl-ast)—— v0.4 关键扩展

// Phase A2:Cell-based capture
pub struct Cell {
    value: Option<Value>,
    mutability: Mutability,  // MUTABLE / IMMUTABLE
}

pub enum Mutability { MUTABLE, IMMUTABLE }

// 闭包 cell 升级:Phase A2
pub struct Closure {
    params: Vec<FunParam>,
    body: Box<Expr>,
    captured: Vec<(String, Cell)>,  // 显式 cell 引用
    span: Span,
}

// Phase A3:解构模式
pub enum Pattern {
    Ident(String),
    Array(Vec<Pattern>, Option<Box<Pattern>>),  // [a, b, *rest]
    Dict(Vec<(String, Pattern)>),
    Wildcard,  // _
}

pub enum Expr {
    // v0.1 已实现
    Literal(Literal),
    Var(String),
    Call { name: String, args: Vec<Expr> },
    MethodCall { receiver: Box<Expr>, method: String, args: Vec<Expr> },
    Let { name: String, value: Box<Expr> },
    If { cond: Box<Expr>, then: Box<Expr>, else_: Option<Box<Expr>> },
    While { cond: Box<Expr>, body: Box<Expr> },
    For { var: String, iter: Box<Expr>, body: Box<Expr> },
    Fun { params: Vec<Param>, body: Box<Expr> },
    Return(Option<Box<Expr>>),
    Break,
    Continue,
    Block(Vec<Expr>),
    Class { name: String, parent: Option<String>, members: HashMap<String, Expr> },
    New { class: String, args: Vec<Expr> },
    GetProp { obj: Box<Expr>, name: String },
    SetProp { obj: Box<Expr>, name: String, value: Box<Expr> },
    Try(Box<Expr>),

    // v0.4 新增
    LetPattern { pattern: Pattern, value: Box<Expr> },  // Phase A3 解构
    Match { value: Box<Expr>, clauses: Vec<(Pattern, Box<Expr>)>, default: Option<Box<Expr>> },  // Phase A4
    Import { module: ModulePath, names: Vec<ImportName> },
    Export(Vec<ExportSpec>),
    Ok(Box<Expr>),
    Err(Box<Expr>),
    Panic(Box<Expr>),
    IsOk(Box<Expr>),
    IsErr(Box<Expr>),
    OrDie(Box<Expr>, Box<Expr>),
    UnwrapOr(Box<Expr>, Box<Expr>),  // Phase B3 主推
    Unwrap(Box<Expr>),  // Phase B4
    ErrPayload(Box<Expr>),
    Wrap(Box<Expr>, Box<Expr>),
    IndexGet(Box<Expr>, Box<Expr>),  // Phase B1
    IndexSet(Box<Expr>, Box<Expr>, Box<Expr>),  // Phase B1
    At(Box<Expr>, Box<Expr>, Box<Expr>),  // Phase B1
    ModuleRef(String),  // Phase C2
    Format(Box<Expr>, Vec<Expr>),  // Phase B5
    TypeAnnotation { expr: Box<Expr>, ty: Type },
}

// Phase E3:AST 稳定 node ID
pub struct Spanned<T> {
    pub node: T,
    pub span: Span,
    pub node_id: String,        // module:fn:body:kind-index
    pub node_hash: String,      // sha256
}

// error schema 1.1.0(Phase A1)
pub struct WlwlError {
    pub error_schema_version: String,  // "1.1.0"
    pub code: ErrorCode,
    pub severity: Severity,
    pub error_category: ErrorCategory,
    pub retryable: bool,
    pub idempotent: bool,               // [v0.4 新增]
    pub retry_after: Option<u64>,       // [v0.4 新增]
    pub message: String,
    pub location: Location,             // 对象形式(v0.3 已用)
    pub source_line: String,
    pub hint: String,
    pub suggestion_code: Vec<Suggestion>,
    pub related: Vec<Location>,
    pub trace: Vec<Frame>,              // [v0.4 新增]
    pub cause: Option<Box<ErrorPayload>>, // [v0.4 新增]WRAP 链
}

pub struct Frame {
    pub frame: String,                  // 函数名
    pub location: Location,
}

pub enum ErrorPayload {
    String(String),
    Dict(Dict),
}
```

### 4.3 求值器核心(v0.4 关键变更)

```rust
// wlwl-eval —— Phase A2 cell 升级
impl Evaluator {
    pub fn eval(&mut self, expr: &Spanned<Expr>) -> Result<Value, EvalError>;

    fn call_with_args(&mut self, f: Value, args: Vec<Value>)
        -> Result<Value, EvalError> {
        // ERR 透明传播(§12.7 注册表模型)
        for arg in &args {
            if matches!(arg, Value::Err(_)) && !self.consumer_registry.contains(&f.name) {
                return Ok(arg.clone());  // 透明传播,短路
            }
        }
        // 正常求值
        ...
    }

    // Phase A2:SET 检查 mutability
    fn set_var(&mut self, name: &str, value: Value) -> Result<(), EvalError> {
        let cell = self.env.get_cell_mut(name)
            .ok_or_else(|| EvalError::UndefinedName(name.to_string()))?;
        if matches!(cell.mutability, Mutability::IMMUTABLE) {
            return Err(EvalError::new(E0024, format!("cannot SET non-captured binding '{}'", name)));
        }
        cell.value = Some(value);
        Ok(())
    }
}

// Phase C2:MODULE_REF
pub struct WlwlModule {
    pub name: String,
    pub members: HashMap<String, Value>,
}

// Phase B4:错误链
pub struct ErrorChain {
    pub payload: ErrorPayload,    // 当前层
    pub cause: Option<Box<ErrorChain>>,
}
```

---

## 5. 关键技术挑战与对策

### 5.1 闭包 cell 语义(§6.4,Phase A2)

**挑战**:
- v0.3 D006 闭包 deep-clone 与 spec §6.3 "不可写" 一致
- v0.4 §6.4 钉死 cell 升级:同一词法位置的 `LET` 在所有引用它的闭包间共享 cell
- v0.3 `fun_closure_independent` 测试(v0.1 D006)与 v0.4 cell 语义**矛盾**,需要重写

**对策**:
- `Cell { value, mutability }` 数据结构
- 闭包捕获 cell 引用,`SET` 修改 cell
- 闭包创建时升级 cell mutability 为 `MUTABLE`(E-CloCap in 附录 E 形式化)
- 重写 `fun_closure_independent` → `fun_closure_shares_cell`(验证 cell 共享)
- 新增 `fun_closure_non_captured_set_e0024`(验证 E0024)

### 5.2 ERR 消费者注册表(§12.7,Phase A6)

**挑战**:
- v0.3 D012 实现了 4 项白名单
- v0.4 §12.7 注册表 9 项(扩展 `UNWRAP` / `ERR_PAYLOAD` / `WRAP` / `TYPE` / `=` / `!=` / `IF`)
- 扩展性原则:新函数需"消费 ERR"**必须**经 spec 升级,实现不得私自扩展

**对策**:
- `consumer_registry: HashSet<String>` 全局表
- `call_with_args` 入口检查函数是否在注册表
- spec 升级时,通过 spec PR + 实现 PR 同步扩展
- 测试:`err_consumer_registry_size_9`(固定 9 项,未来 spec 升级时此测试提醒更新)

### 5.3 error schema 1.1.0 升级(§14.2,Phase A1)

**挑战**:
- MINOR bump:新增 `trace` / `cause` / `retry_after` / `idempotent` 字段
- 旧 AI 工具必须能继续解析(忽略未知字段)
- `cause` 递归 DICT 结构,与 `errorCategory` 字段协同

**对策**:
- 所有新字段 `#[serde(default)]` + `skip_serializing_if = "Option::is_none"`
- AI 工具契约测试(沿用 v0.1 §6.4 P3-002 insta snapshot)扩展到 schema 1.1.0
- `trace` 构造:实现栈式 frame,最少 1 帧,建议 ≥ 32 帧
- `cause` 通过 `WRAP` 函数构建,测试 `wrap_creates_cause_chain`

### 5.4 INDEX_GET / INDEX_SET 边界(§10.1/§10.2,Phase B1)

**挑战**:
- ARRAY 越界:`-LEN(arr) ≤ i < LEN(arr)`,支持负数索引
- DICT 缺键 → `E0037`
- ARRAY / DICT / STRING / OBJECT 统一接口
- `i` 非 INTEGER → `E0031`

**对策**:
- 单一 `IndexGet(coll, i)` 函数,内部 match 容器类型
- 越界错误区分:`E0036` ARRAY / `E0037` DICT
- 语法糖 `arr[i]` 在 parser 端 desugar 为 `INDEX_GET(arr, i)`
- `arr[i] = v` 同样 desugar
- v0.3 隐式 `arr[i]` 不存在(无 ARRAY 越界保护),本 phase 首次引入

### 5.5 FORMAT 模板解析(§10.6,Phase B5)

**挑战**:
- 模板语法:`{0}` / `{1}` / `{name}`
- 混合:`FORMAT("hi {0}, age {age}", "alice", ["age": 30])`
- 模板解析失败 → `E0039`
- `args[i]` 非 STRING / 非 DICT → `STR(args[i])` 转换
- 跨平台 / 性能

**对策**:
- 解析器:简单状态机,识别 `{...}` 占位
- 占位类型区分:纯数字 → 位置占位;非数字 → 名字占位(从 args[0] DICT 取)
- 模板 `{` 单独出现(无 `}`)→ `E0039`
- 缓存:相同 template 复用解析结果

### 5.6 std.collection 16 个高阶函数(§15.7,Phase B6)

**挑战**:
- 16 个函数全部实现:语义正确 + 性能 + ERR 透明传播
- `REDUCE` 初始值 + 累加器函数 + 空数组返回 init
- `GROUP_BY` 返回 DICT 格式
- `SORT` 默认 `<`,可指定 cmp
- `RANGE` step=0 → `E0038`

**对策**:
- 逐函数实现,每个函数独立单测
- 利用 v0.3 已有 `MAP` / `FILTER` / `REDUCE` 基础(如有)
- 性能:用 `iter()` 风格 + `Rc<Vec<Value>>` 复用
- ERR 透明传播:输入含 ERR → 整体输出 ERR(短路)

### 5.7 std.test 框架设计(§15.9,Phase B7)

**挑战**:
- `TEST(name, body)` 在模块顶层声明,但 body 不立即执行
- `RUN_TESTS()` 触发执行,收集结果
- `ASSERT(cond)` 失败抛 `ERR(E0046)`,`RUN_TESTS` 用 `TRY` 捕获
- 返回 `ARRAY of DICT`(`name` / `passed` / `duration_ms` / `error?`)

**对策**:
- 全局 `TestRegistry`:`Vec<(name, body)>` 收集
- `TEST(name, body)` 注册不执行
- `RUN_TESTS()`:遍历,每条 `TRY(body)`,捕获 ERR
- 每条测试用 `Instant::now()` 计时
- 新错误码 E0046 / E0047 / E0048 / E0049(spec §14.4)
- 测试程序继续运行,失败不 panic(spec §15.9 末注)

### 5.8 真实 std.ai HTTP 集成(§15.13,Phase D)

**挑战**:
- v0.3 mock(P3-012)用 reserved model 名触发
- v0.4 真实 HTTP:`reqwest` blocking + 流式 chunk
- 凭据:`WLWL_AI_API_KEY` env 变量(从不入文件)
- 超时 / 错误码细分 / 同步阻塞

**对策**:
- `reqwest::blocking::Client` + `client.post().body(...)`
- `ASK_STREAM`:用 `read_chunk()` 循环 + callback
- 错误映射:`reqwest::Error` → E0090-E0094(网络细分)
- 凭据:`std::env::var("WLWL_AI_API_KEY")`,缺失 → E0082
- 测试:CI mock 模式(`WLWL_AI_MOCK=1`)返回固定响应

### 5.9 strict_types 运行时检查(§2.7,Phase E1)

**挑战**:
- 默认(不严格)行为与 v0.3 完全一致
- 严格模式:`wlwl.toml` `[features] strict_types = true`
- 边界检查:函数入口 / IMPORT 边界
- 性能预算:严格模式 ≤ 10% 开销
- 失败 → `E0033`

**对策**:
- 实现层每个函数入口加可选 `strict_check` 步骤
- `strict_types: true` 时开启,默认 false 时跳过(零开销)
- 性能:proptest benchmark 验证 ≤ 10% 开销
- `TypeExpr` 已在 v0.1 落地,直接复用

### 5.10 canonical formatter 10 条规则(§16.3,Phase E2)

**挑战**:
- 10 条规范化规则(附录 G 末注)机械可推导
- `wlwl fmt` 子命令实现
- 不符合 → `W0053` 警告
- idempotent:`fmt(fmt(x)) = fmt(x)`

**对策**:
- 单独 crate `wlwl-formatter`(Phase E2)
- 实现策略:parser → AST → formatter 重建(从 spec §16.3 规则输出)
- 不用直接文本改写(易出 idempotency bug)
- 风格模板:4 空格 / ≤ 100 字符 / `;` 必填 / 逗号无尾随
- 单测:`fmt_idempotent`(对 100 个 fixture 跑 `fmt(fmt(x)) = fmt(x)`)

### 5.11 AST 稳定 node ID(§16.4.1,Phase E3)

**挑战**:
- `node_id` 跨源码行号变化保持稳定
- `node_hash`(sha256)内容变化检测
- 现有 AST 序列化需要扩展

**对策**:
- AST 节点附加 `node_id: String` + `node_hash: String` 字段
- `node_id` 格式:`module:fn:body:kind-index`
- `wlwl ast --format=json` 输出新 schema
- 现有 snapshot 测试 fixture 更新

### 5.12 一致性测试套件(§16.5,Phase H1)

**挑战**:
- 10 项必含测试覆盖(spec §16.5)
- "实现不通过不得自称 v0.4 compliant"
- 与现有 v0.1 cargo test 集成

**对策**:
- `tests/core.wlt` 格式自定义(类似 test262 风格)
- `tests/conformance.py` 跑 `wlwl check` + `wlwl run` 验证
- 10 项逐项独立测试文件:
  - `tests/conformance/{core_subsets,err_propagation,index_bounds,numeric,match_patterns,destruct,closure_cell,module_paths,format_template,error_schema}.wlt`
- CI 集成:`cargo test --workspace` + `python tests/conformance.py`(双轨)

### 5.13 W0020 linter 端独立 walk(Phase E4)

**挑战**:
- v0.1 P3-013 已实现 parser 端状态机 + emit W0020
- 本 phase:加 `pub fn lint(expr: &Expr) -> Vec<Warning>` post-parse walk
- 暴露给 `wlwl check` 子命令

**对策**:
- 沿用 v0.1 `parse_with_warnings` 通道
- 加 `pub fn lint(expr: &Expr) -> Vec<Warning>` 独立 walk fn
- 收集未使用变量(W0010)/ 未使用函数参数(W0011)/ 同名重复 LET(W0012)/ 遮蔽宏函数(W0030) 等
- `wlwl check` 集成,默认 lint + strict mode

### 5.14 性能基线建立(Phase F1)

**挑战**:
- `cargo bench` 框架
- 5 个基准测试:简单循环 / 闭包 / 字符串 / 集合 / 错误传播
- baseline 入仓(`benches/baseline.txt`)

**对策**:
- `criterion` crate 引入(标准 benchmark 框架)
- 5 个独立 bench 函数
- `cargo bench --bench simple_loop` 等
- CI:检测 ± 10% 偏差 fail

### 5.15 Phase G 质量门禁集成(Phase G 集成 + CI)

**挑战**:
- 12 个 G 子项(略)分别实现,但 CI 必须统一运行
- 任何 G 失败 → Phase H 阻塞

**对策**:
- `.github/workflows/ci.yml` 加 step:
  - `cargo fmt --check`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `cargo deny check`
  - `cargo test --workspace`
  - `cargo llvm-cov --workspace` (≥ 90% line 13/13)
  - `cargo doc --workspace --no-deps --document-private-items=false`
  - `cargo bench` (可选,merge 前跑)
  - `python tests/conformance.py`
  - `mkdocs build --strict`
- 每个 G 子项的工件入版本控制
- release.yml 加 G 全套门禁(v0.4.0 tag 前必跑)

---

## 6. 测试策略

### 6.1 测试金字塔(沿用 v0.1 §6.1)

```text
                    ┌─────────────┐
                    │  Conformance│  (Phase H1)spec §16.5
                    │  (core.wlt) │
                ┌───┴─────────────┴───┐
                │   集成测试          │  (每 Phase 都加)
                │   (标准库 + 主程序) │
            ┌───┴─────────────────────┴───┐
            │     单元测试                │  (每个核心语义)
            │     (求值器、模块、错误)    │
        ┌───┴─────────────────────────────┴───┐
        │     错误码 insta 快照               │  (每个错误码)
        │     (56 + 14 = 70 个)             │
        └─────────────────────────────────────┘
```

### 6.2 单元测试覆盖目标(v0.1 沿用,Phase G8 加固)

| 模块 | 目标覆盖率 | Phase G 验收 |
|------|------------|------------|
| 词法 (wlwl-lexer) | 100% | ≥ 90% line 维持 |
| 语法 (wlwl-parser) | 95% | ≥ 90% line 维持 |
| 求值器核心 (wlwl-eval) | 90% | ≥ 90% line 维持 |
| 求值器 std (wlwl-std/*) | 80% | ≥ 90% line 维持 |
| 模块解析 (wlwl-toml) | 90% | ≥ 90% line 维持 |
| 错误处理 (wlwl-error) | 95% | ≥ 90% line 维持 |
| AST (wlwl-ast) | n/a | ≥ 90% line 维持 |
| CLI (wlwl-cli) | n/a | ≥ 90% line 维持 |
| **TOTAL** | 90% | ≥ 90% line 维持 + region ≥ 90% |

**v0.1 baseline**(2026-09-04):13/13 workspace file ≥ 90% line,TOTAL 92.94% / region 92.65% / func 96.90%。**v0.2 维持 + Phase F 性能不影响覆盖**。

### 6.3 错误码 insta 快照(Phase G7 扩到 56 + 14)

每个错误码(56 个 E + 14 个 W = 70 个)**至少 1 个** insta 快照:
- 触发该错误/警告的最小 .wl 程序
- 期望的 JSON 输出(schema 1.1.0)
- 期望的 CLI 人类可读输出
- 期望的 JSONL 输出

v0.1 P3-008 已加 35 码的 per-site `Suggestion::Note`,Phase G7 扩到 56 + 14 全量 + suggestion_code content 实质化。

### 6.4 AI 契约测试(沿用 v0.1 §6.4)

spec §16.1 11 条 AI 契约(spec §16.1 列出)逐条写测试:
- 优先 `--format=jsonl` 流式消费
- 总是读取 `suggestion_code` 字段并尝试自动 apply
- 总是把 `related` 字段全部纳入上下文
- 总是尊重 `severity` 字段
- 总是根据 `retryable` 字段决定是否重试
- 总是根据 `idempotent` 字段决定是否询问用户后重试
- 总是根据 `retry_after` 字段选择重试延迟
- 总是根据 `errorCategory` 字段选择修复策略
- 总是优先使用 §14.4 错误码表做错误分类
- 总是先检查 `error_schema_version` 字段
- 总是消费 `trace` 字段作为调用栈上下文

### 6.5 端到端测试(沿用 v0.1 §6.5 + Phase G12 扩到 10)

每个 Phase 至少 5 个端到端 .wl 程序;Phase G12 扩到 10 个:
- v0.1 沿用:`hello.wl` / `math.wl` / `showcase.wl` / `phase2_demo.wl` / `test_array.wl`
- Phase G12 新增:`match.wl` / `destruct.wl` / `std_test.wl` / `closure_cell.wl` / `format.wl`

### 6.6 性能基准(沿用 v0.1 §6.6,Phase F1 收尾 + G8 CI)

简单循环 100 万次 < 30 秒 + 4 个其他基准(Linux/macOS 基准机);Phase G8 CI fail if > 110% baseline。

### 6.7 fuzz 测试(Phase G6 新增)

3 个 fuzz target(parser / eval / std),`cargo +nightly fuzz run <target> -- -max_total_time=600` ≥ 10 分钟,corpus 入仓。

### 6.8 错误信息 schema 一致(Phase H1 第 10 项)

附录 B JSONL 示例必须由实现产出(逐字段比对);spec §16.5 发布要求。

---

## 7. 工具链规划

### 7.1 命令最小集(v0.1 沿用 + v0.4 扩展)

| 命令 | 作用 | 引入 Phase | 备注 |
|------|------|-----------|------|
| `wlwl run <file>` | 运行 .wl 程序 | 1 (v0.1) | 沿用 |
| `wlwl check <file>` | 仅做词法/语法/名字检查,不执行 | 1 (v0.1) | 沿用 + Phase E4 lint |
| `wlwl run <file> --format=json` | 输出 JSON 错误(单条/数组) | 2 (v0.1) | schema 1.1.0 (Phase A1) |
| `wlwl run <file> --format=jsonl` | 输出 JSONL 流式错误 | 3 (v0.1) | schema 1.1.0 (Phase A1) |
| `wlwl check <file> --strict` | 警告变错误 | 3 (v0.1) | 沿用 |
| `wlwl fmt <file>` | canonical formatter | **E2 (v0.2 新增)** | spec §16.3 |
| `wlwl ast <file> --format=json` | 输出 JSON AST(供 AI 消费) | 2 (v0.1) | node_id + hash (Phase E3) |
| `wlwl repl` | REPL | **v0.5+ 评估** | v0.1 标 4+ 可选,本计划保留 v0.5 |
| `wlwl run <file> --format=ast-node-id` | 输出 AST 带稳定 node ID | **E3 (v0.2 新增)** | spec §16.4 |
| `wlwl check <file> --strict_types` | 严格类型边界检查 | **E1 (v0.2 新增)** | spec §2.7 |

### 7.2 命令行约定(v0.1 沿用 + v0.4 补)

- 所有错误输出**至少**支持 `--format=json`
- AI 工具**应**使用 `--format=jsonl`(v0.3 §14.7 契约 → v0.4 §16.1 升级)
- `--strict` 影响退出码(warning 也返回 1)
- 默认人类可读输出
- **新增** `--strict_types`(Phase E1)

### 7.3 配置文件(§13.8,v0.4 扩展)

```toml
[package]
name = "myapp"                          # 必填
version = "0.1.0"                       # 必填
language_version = "0.4"                # [v0.2 新增]Phase C3
entry = "src/main.wl"                   # 必填
description = "A WLWL app"              # 可选
license = "MIT"                         # 可选
allow_builtin_shadow = false            # [v0.2 新增]Phase C5

[dependencies]
# 路径依赖(本地开发)
"myteam:utils" = { path = "../utils" }
# 版本依赖(中央仓库,Phase C4 MVS 准备,v0.4 仍仅 path)

[namespaces]
# 显式命名空间映射(可选,通常自动推断)
"myteam" = "./vendor/myteam"

[features]
strict_types = false                    # [v0.2 新增]Phase E1
default_encoding = "utf-8"
allow_builtin_shadow = false            # 同 [package]
```

### 7.4 canonical formatter `wlwl fmt`(Phase E2,spec §16.3)

- 10 条规范化规则(附录 G 末注)
- 输出 idempotent
- 风格修正:4 空格 / ≤ 100 字符 / `;` 必填 / 逗号无尾随
- 不符合实现 → `W0053` 警告

### 7.5 AST/Patch 协议(Phase E3,spec §16.4)

- 节点稳定 ID + hash(§16.4.1)
- Patch IR / AST 序列化 / Symbol table / Module graph / Diagnostic(§16.4.3)
- 与 LSP 3.17 对齐(§16.4.4)

### 7.6 一致性测试套件(Phase H1,spec §16.5)

- `tests/core.wlt` + `tests/conformance.py`
- 10 项必含测试覆盖
- 实现不通过不得自称 "v0.4 compliant"

---

## 8. 与 v0.4 规范的同步机制(沿用 v0.1 §8)

### 8.1 偏离清单(本计划独占,规范无)

任何**实现**与 v0.4 规范**不一致**的地方,必须记录在 `plan/deviations.md`:

```markdown
# 实施偏离清单

| 编号 | 规范条款 | 偏离描述 | 原因 | 计划修复 Phase |
|------|----------|----------|------|----------------|
| D031 | §6.4 闭包 cell | v0.3 闭包 deep-clone,违反 cell 共享 | Phase A2 改造 | Phase A2 |
| D032 | §14.2 trace 字段 | 暂未实现 | Phase A1 升级 | Phase A1 |
| ... | ... | ... | ... | ... |
```

### 8.2 规范变更的影响评估(沿用 v0.1 §8.2)

每次规范 v0.4.md SHA1 变化时,**必须**重新评估本计划的影响范围:
- 哪个 Phase 受影响?
- 哪些模块需要重写?
- 时间预算是否需要调整?

### 8.3 规范 → 计划的双向同步(沿用 v0.1 §8.3)

- 规范新条款 → 本计划对应 Phase 增加任务
- 本计划发现规范模糊 → 反馈给规范(在 v0.4.x 微调)
- 严禁实施时**默默**修改规范;规范变更必须**显式**走版本号

---

## 9. 风险登记

| ID | 风险 | 等级 | 缓解策略 | 触发条件 |
|----|------|------|----------|----------|
| R001 | 闭包 cell 改造波及 eval | **高** | 显式 cell 升级逻辑;`SET` mutability 检查;Phase A2 隔离;D006/D016 测试重写 | 任意闭包测试失败 |
| R002 | 错误码 35→56 重构遗漏 | **高** | spec §14.4 总表是 source of truth;deviations.md 显式登记;P3-008 + Phase G7 全 70 码覆盖 | 任意错误码缺触发点 |
| R003 | error schema 1.1.0 不向后兼容 | 中 | 旧字段保留;新字段 `#[serde(default)]`;AI 工具契约测试 | 旧 AI 工具解析失败 |
| R004 | strict_types 性能 > 10% 开销 | 中 | spec §2.7 给出目标;Phase F1 + E1 proptest benchmark | bench fail |
| R005 | 真实 std.ai HTTP 国内访问 OpenAI | **高** | mock fallback 保留;`WLWL_AI_MOCK=1`;CI 强制 mock 模式 | std.ai 测试连接失败 |
| R006 | canonical formatter idempotent 失败 | 中 | 不用直接文本改写;从 AST 重建;100 fixture 单测 | 任意 fixture `fmt(fmt(x)) ≠ fmt(x)` |
| R007 | 一致性测试套件(spec §16.5) 失败 | **高** | Phase H1 强制;Phase G 缓冲 1 天 | 任意项失败 |
| R008 | Phase G 质量门禁误报 | 中 | 工具配置进 workspace 顶层;clippy.toml / deny.toml;`.cargo/config.toml` | CI 频繁 fail |
| R009 | Windows 兼容性 | 中 | 沿用 v0.1 CI 三平台;release.yml 已就位 | Windows CI 失败 |
| R010 | v0.1 闭包 deep-clone 行为被外部依赖 | 中 | v0.1 闭包独立性测试不暴露;Phase A2 改造 | spec v0.4 §6.4 实施后 |
| R011 | AS 函数删除破坏 v0.3 代码 | 中 | spec §13.4 钉死;v0.3 程序改写为 `LET(alias, orig)`;migration doc | 用户报告 v0.3 代码不兼容 |
| R012 | cargo-fuzz / cargo-miri 引入需 nightly | 低 | optional CI step;主 CI 仍用 stable | nightly 破坏 |
| R013 | mkdocs 部署失败 | 低 | 沿用 release.yml;`mkdocs gh-deploy` 加 retry | GitHub Pages 失败 |
| R014 | 性能回归测试 CI 波动 | 中 | 5 个基准取 median;± 10% 阈值;非阻 fail 改为 warn | bench fail 偶发 |
| R015 | 中央仓库(registry)协议 v0.4 留白 | **v0.4 不实现** | v0.4 留 v0.5;spec §13.9 MVS 仅 path | v0.5 议程 |
| R016 | 形式化附录机械化工作量 | **v0.4 跳过** | v0.1 §0.1 #3 沿用;v0.4 附录 E 修复未机械化 | Phase 5 启动时评估 |

---

## 10. 决策清单

> **状态:9 项关键决策已锁定**(见 §0.1)。本节保留作为"未来补充决策"的入口。

### 10.1 已锁定

| # | 决策项 | 决策 | 锁定时间 |
|---|--------|------|----------|
| 1 | 技术栈 | Rust + tree-walking | 2026-09-02 (v0.1) |
| 2 | 工期预算 | v0.1 实际 1 天 / v0.2 估算 7 周 | 2026-09-02 (v0.1) + 本计划 |
| 3 | Phase 5 形式化 | 可选 | 2026-09-02 (v0.1) |
| 4 | CI 平台 | GitHub Actions | 2026-09-02 (v0.1) |
| 5 | 许可证 | GPL v2 | 2026-09-02 (v0.1) |
| 6 | 目标用户 | 公开 | 2026-09-02 (v0.1) |
| 7 | v0.2 范围 | spec v0.4 全合规 + 独立质量 phase + 发布 v0.4.0 exe | 本计划启动时 |
| 8 | Phase G 质量 phase | 强制门禁 | 本计划启动时 |
| 9 | exe 发布目标 | 三平台 native binary | 本计划启动时 |

### 10.2 未来待定(Phase A+)

- [ ] std.time 库:`time` 0.3(无 unsafe,推荐)/ `chrono`/ 纯手写
- [ ] strict_types 性能基线:1.0x 阈值(不严格模式 ≥ 0% 开销)实际如何测?proptest 基准?
- [ ] centralized registry:v0.4 是否启用?MVS 依赖 registry 还是仅 path?默认 path-only
- [ ] WASM 目标:v0.4 不引入,沿用 v0.1 §10.2 远期 TODO
- [ ] std.agent 完整实现:v0.4 薄包装,v0.5 完整
- [ ] REPL 优先级:v0.5 评估
- [ ] LSP 协议完整实现:v0.5+ 评估
- [ ] 中央仓库签名验证(明文 / SHA-256 / ed25519):v0.5 议程

---

## 11. 附录 A:本计划与 v0.4 规范的章节映射

| v0.4 规范章节 | 本计划 Phase | 本计划模块 |
|---------------|--------------|------------|
| §0.4 Conformance | Phase H1 | 一致性测试套件 |
| §0.5 规范性语言 | (本计划遵守) | — |
| §0.6 规范性引用 | (本计划遵守) | — |
| §0.7 形式化优先级 | (本计划遵守) | — |
| §1.5 规范优先级 | (本计划遵守) | — |
| §2.2.1 RESULT 类型 | Phase A5 | wlwl-eval |
| §2.4 类型注解(Transient) | v0.1 已实现 | wlwl-ast / wlwl-eval |
| §2.7 strict_types | Phase E1 | wlwl-eval |
| §3.2 标识符 | v0.1 P3-011 已实现 | wlwl-lexer |
| §3.3 关键字(14) | v0.1 P3-011 已修正 | wlwl-parser |
| §3.4 宏函数清单 | Phase A + B | wlwl-parser / wlwl-eval |
| §3.6 /// 文档注释 | Phase G3 rustdoc | wlwl-eval / wlwl-std |
| §3.8 命名约定 | Phase E4 + G7 | wlwl-parser (W0050) |
| §4.5 `[]` 恒为 ARRAY | v0.1 已实现 | wlwl-parser |
| §5.5 链式访问 | v0.1 P3-011 已实现 | wlwl-parser |
| §6.3 词法作用域 | v0.1 已实现 | wlwl-eval |
| §6.4 闭包 cell 语义 | **Phase A2 关键改造** | wlwl-eval |
| §6.6 遮蔽 + allow_builtin_shadow | Phase C5 | wlwl-parser / wlwl-error |
| §7.5 解构绑定 | **Phase A3** | wlwl-parser / wlwl-eval |
| §7.6 MATCH 模式匹配 | **Phase A4** | wlwl-parser / wlwl-eval |
| §8.2 FUN 具名 / 默认 / *rest | v0.1 P3-011 已实现 | wlwl-parser |
| §8.4 元数公式 | (v0.3 实现已对齐) | wlwl-eval |
| §8.6 NEW/INIT 协议 | v0.1 已实现 | wlwl-eval |
| §9.2 比较返回类型 | Phase A8 (文档化) | wlwl-eval |
| §9.4 真值表补 OK/ERR/DICT/NAN | Phase A8 (测试化) | wlwl-eval |
| §9.5 数值与跨类型 | **Phase A7 关键改造** | wlwl-eval |
| §10.1/§10.2 下标访问 | **Phase B1** | wlwl-std / wlwl-eval |
| §10.3 字符串扩展 | **Phase B8** | wlwl-std / wlwl-eval |
| §10.5 高阶集合函数 | **Phase B6 std.collection** | wlwl-std |
| §10.6 FORMAT 字符串格式化 | **Phase B5** | wlwl-std / wlwl-eval |
| §11.4 链式 + self 注入 | v0.1 P3-011 已实现 | wlwl-parser / wlwl-eval |
| §12.2 UNWRAP/UNWRAP_OR/ERR_PAYLOAD/WRAP | **Phase B3 + B4** | wlwl-eval / wlwl-std |
| §12.3 TRY 示例修正 | v0.1 已修正 | wlwl-eval |
| §12.6 ERR 透明传播示例修正 | v0.1 已修正 | wlwl-eval |
| §12.7 ERR 消费者注册表 | **Phase A6** | wlwl-eval |
| §12.8 错误链 | **Phase B4** | wlwl-error |
| §13.4 AS 函数删除 | **Phase C1** | wlwl-parser / wlwl-error |
| §13.5 跨目录引用 | Phase C7 | wlwl-toml / wlwl-eval |
| §13.6 命名空间路径 | v0.1 已实现 | wlwl-toml / wlwl-eval |
| §13.8 wlwl.toml 扩展 | **Phase C3 + C5 + C6** | wlwl-toml |
| §13.9 MVS 依赖求解 | **Phase C4** | wlwl-toml |
| §13.11 模块接口契约 | v0.1 已实现 | wlwl-toml |
| §13.12 模块作为值 MODULE_REF | **Phase C2** | wlwl-eval |
| §14.2 error schema 1.1.0 | **Phase A1 关键升级** | wlwl-error |
| §14.3 顶层输出格式 | Phase A1 + G7 | wlwl-cli |
| §14.4 错误码总表(56) | **Phase A1 + B 触发点 + G7** | wlwl-error / wlwl-eval / wlwl-std |
| §14.5 警告码(14) | **Phase A1 + E4 + G7** | wlwl-error / wlwl-parser |
| §14.6 严重程度与退出码 | v0.1 已实现 | wlwl-cli |
| §14.7 AI 工具约定 | v0.1 P3-008 已实现 + Phase G7 扩 | wlwl-cli |
| §14.8 单错误终止限定 | v0.1 已修正 | wlwl-parser / wlwl-cli |
| §14.9 JSONL 流式输出 | v0.1 已实现 | wlwl-cli |
| §14.10 schema 版本化 | Phase A1 | wlwl-error |
| §15.1 全局内建注册表 | **Phase B11** | wlwl-eval / wlwl-std |
| §15.2 std.io (PRINT_ERR) | **Phase B10** | wlwl-std |
| §15.3 std.fs | v0.1 已实现 | wlwl-std |
| §15.4 std.json | v0.1 已实现 | wlwl-std |
| §15.5 std.math | v0.1 已实现(承诺升级) | wlwl-std |
| §15.6 std.string | **Phase B8 扩展** | wlwl-std |
| §15.7 std.collection | **Phase B6 全新** | wlwl-std |
| §15.8 std.format | **Phase B5 全新** | wlwl-std |
| §15.9 std.test | **Phase B7 全新** | wlwl-std |
| §15.11 std.time | B 续(独立 phase) | wlwl-std |
| §15.12 std.os | v0.1 已实现(沿用) | wlwl-std |
| §15.13 std.ai(真实) | **Phase D 全新** | wlwl-std |
| §15.14 std.agent | **Phase D3 薄包装** | wlwl-std |
| §16.1 AI 契约 | v0.1 已实现 + Phase G7 扩 | wlwl-cli |
| §16.3 canonical formatter | **Phase E2** | wlwl-formatter(新 crate) |
| §16.4 AST/Patch 协议 | **Phase E3** | wlwl-ast / wlwl-cli |
| §16.5 一致性测试套件 | **Phase H1 必含** | tests/conformance |
| §17.1 已解决 | (v0.4 收口) | — |
| 附录 A 示例 | Phase G12 examples 套件 | examples/ |
| 附录 B JSONL 示例 | Phase A1 | wlwl-cli |
| 附录 C v0.3→v0.4 变更日志 | (规范,本计划遵守) | — |
| 附录 D v0.4→v0.5 已知问题 | (资料性) | — |
| 附录 E 形式化语义修复 | v0.4 已修复(spec) | (实现跟随修复) |
| 附录 F EBNF 文法 | (规范,实现 parser 遵守) | wlwl-parser |
| 附录 G 全局内建注册表 | **Phase B11** | wlwl-eval / wlwl-std |
| 附录 H 术语表 | (规范) | — |
| 附录 I 规范性引用 | (规范) | — |
| 附录 J 惯用法 | (规范) | — |

---

## 12. 附录 B:关键技术决策记录(ADR 风格,沿用 v0.1 + v0.4 新增)

> **ADR-001 ~ ADR-007**:v0.1 §12 锁定(技术栈 / 执行模型 / 错误处理 / 解析器 / 许可证 / CI 平台 / 目标用户)

> **ADR-008 ~ ADR-013** (本计划新增,Phase G4 实现):

> **ADR-008:闭包 cell 升级(§6.4)**
> - **状态**:建议,待 Li 拍板
> - **背景**:v0.3 §6.3 "子作用域不可写父作用域绑定" 与 §8.5 示例 `SET(count, +(count, 1))` 矛盾。v0.4 §6.4 钉死 cell 升级:同一词法位置的 LET 在所有引用它的闭包间共享 cell,SET 修改 cell。
> - **决策**:实现层采用 cell-based capture。每个 LET 创建 `Cell { value, mutability }`,闭包捕获 cell 引用(而非值副本)。闭包创建时升级 cell mutability 为 MUTABLE。未被子作用域捕获的 cell,子作用域 SET → E0024。
> - **后果**:D006 / D016 闭包独立性测试与 v0.4 cell 语义矛盾,需重写。新增 `fun_closure_shares_cell` / `fun_closure_non_captured_set_e0024` 测试。cell 升级有轻微性能开销(Rc<Cell<>>)但 v0.3 deep-clone 开销更大。
> - **替代**:1) 显式 `REF` 关键字(类似 Rust `&mut`,违反 §1.2 "显式优于隐式");2) 维持 v0.3 不可写行为(与 spec §8.5 示例矛盾);3) 引入 `nonlocal` 关键字(违反 v0.4 §6.3 "禁止 nonlocal/global 关键字")。

> **ADR-009:ERR 消费者注册表(§12.7)**
> - **状态**:建议,待 Li 拍板
> - **背景**:v0.3 仅有 IS_OK / IS_ERR / OR_DIE / TRY 4 个白名单函数,造成"无法打印错误"的痛点(v0.3 §12.6 末注)。v0.4 §12.7 改为注册表机制,初始 9 项:IS_OK / IS_ERR / OR_DIE / UNWRAP_OR / TRY / UNWRAP / ERR_PAYLOAD / WRAP / TYPE / = / != / IF。
> - **决策**:实现 `consumer_registry: HashSet<String>` 全局表,call_with_args 入口检查函数是否在注册表。新函数需"消费 ERR"必须经 spec 升级加入注册表,实现不得私自扩展。
> - **后果**:ERR 消费者扩展需 spec PR + 实现 PR 同步,降低实现层偏离风险。注册表固定 9 项在 Phase A6 写测试 `err_consumer_registry_size_9`,未来 spec 升级时此测试提醒更新。
> - **替代**:1) 维持 v0.3 封闭白名单(无法支持 ERR_PAYLOAD / UNWRAP);2) 开放实现层扩展(AI 工具不可预测 ERR 行为)。

> **ADR-010:strict_types 行为(§2.7)**
> - **状态**:建议,待 Li 拍板
> - **背景**:v0.3 §2.6 留 strict_types 行为未决。v0.4 §2.7 钉死 Transient cast insertion(非 Natural monitoring)。Transient 立场:类型注解不强制运行时检查,严格模式仅在边界插入 cast。
> - **决策**:`wlwl.toml` 的 `[features] strict_types = true` 字段开启运行时边界检查(函数入口 / IMPORT 边界),不开则行为与 v0.3 完全一致。失败 → E0033。
> - **后果**:性能预算 ≤ 10% 开销(spec §2.7 末段)。Transient 立场:不强制内层代码,严格模式检查不修改运行时行为(失败的边界调用直接抛 ERR,不改写控制流)。
> - **替代**:1) Natural + monitoring(违反 §2.6 立场);2) 完全不开 strict_types(违反 spec §2.7 必须);3) 引入 CAP 类型系统(推迟到 v0.5 §17.5)。

> **ADR-011:MVS 依赖求解(§13.9)**
> - **状态**:建议,待 Li 拍板
> - **背景**:v0.3 把"Cargo-style 还是 npm-style"挂起到 v0.4。v0.4 §13.9 决策 Cargo 风格 MVS(Minimal Version Selection)。
> - **决策**:MVS 对每个依赖,选择**满足所有约束的最低版本**。算法可解释,与 PubGrub 求解器相比是"线性扫描",AI 工具可静态分析。循环依赖(逻辑)→ E0041,冲突(无版本满足所有约束)→ E0045。
> - **后果**:v0.4 仅本地 path 依赖 + lock 文件;中央仓库 registry 留 v0.5 议程。MVS 足够(v0.x 阶段不需要 PubGrub)。若中央仓库引入后冲突频繁,可平滑升级为 PubGrub。
> - **替代**:1) PubGrub(复杂度高,适合"已有生态冲突压力"的场景如 npm/cargo 中心化仓库);2) 始终最新版本(无版本约束);3) lock-only(放弃 toml 约束)。

> **ADR-012:AS 函数删除(§13.4)**
> - **状态**:建议,待 Li 拍板
> - **背景**:v0.3 保留单参数 AS 函数作为"已绑定名字的本地别名"。v0.4 §13.4 删除 AS:与 §1.2 第 7 条 "一语义仅一种标准写法"冲突(导入时重命名已覆盖所有合理需求)。
> - **决策**:完全删除 AS 函数。引用 AS → E0020 未定义。v0.3 引入 AS 的代码改写为 `LET(alias, orig)`(等价直白)。
> - **后果**:v0.3 → v0.4 迁移文档:v0.3 `AS("y", "x")` → v0.4 `LET(y, x)`。规范删除 AS 后,AI 工具解析 IMPORT 即可拿到完整绑定,无需二次扫描。
> - **替代**:1) 保留 AS 作为别名(违反 §1.2 第 7 条);2) 保留 AS 但加 W0055 弃用警告(增加认知负担);3) 引入新语法替代 AS(过度设计)。

> **ADR-013:canonical formatter 行为(§16.3)**
> - **状态**:建议,待 Li 拍板
> - **背景**:gofmt 经验证明"单一规范排版 = AI agent 的 diff 噪音趋零"。v0.4 §16.3 必规范化 10 条规则:4 空格 / ≤ 100 字符 / `;` 必填 / 逗号无尾随 / `(` `)` 内不强制空格 / 块表达式末表达式无 `;` / IMPORT 独立行 / CLASS members 折行 / 空集合 `ARRAY()`/`DICT()`。
> - **决策**:实现 `wlwl fmt <file>` 子命令,从 AST 重建输出(非直接文本改写,保证 idempotent)。不符合 §16.3 的实现产生 W0053 警告。
> - **后果**:formatter 行为机械可推导,AI 工具可预测输出。fmt(fmt(x)) = fmt(x) idempotent test 100 个 fixture 验证。
> - **替代**:1) 不规范(违反 §16.3 必须);2) 多种风格可选(增加测试矩阵);3) 文本改写(易出 idempotency bug)。

---

## 13. 附录 C:时间估算细化(沿用 v0.1 + v0.4 调整)

### Phase A 任务分解(1 周)

| 任务 | 周期 | 备注 |
|------|------|------|
| 闭包 cell 重构 + D006/D016 重写 | 2 天 | Phase A2 关键 |
| 解构绑定 parser + eval | 1 天 | Phase A3 |
| MATCH parser + eval | 1 天 | Phase A4 |
| error schema 1.1.0 + trace/cause/idempotent/retry_after | 1 天 | Phase A1 |
| ERR 消费者注册表 + 4→9 项 | 0.5 天 | Phase A6 |
| 数值 §9.5(整除/溢出/NaN/跨类型) | 0.5 天 | Phase A7 |
| **Phase A 缓冲** | **0.5 天** |  |

### Phase B 任务分解(1.5 周)

| 任务 | 周期 | 备注 |
|------|------|------|
| `INDEX_GET`/`INDEX_SET`/`AT` + 语法糖 | 1.5 天 | Phase B1 |
| `REMOVE_KEY` 重命名 + `DEL` 别名 + W0051 | 0.5 天 | Phase B2 |
| `UNWRAP_OR` 重命名 + `OR_DIE` 别名 + W0051 | 0.5 天 | Phase B3 |
| `UNWRAP`/`ERR_PAYLOAD`/`WRAP` + cause 链 | 1 天 | Phase B4 |
| `FORMAT` + `std.format` | 1 天 | Phase B5 |
| `std.collection` 16 个高阶函数 | 2 天 | Phase B6 关键 |
| `std.test` 框架 + E0046-E0049 | 1.5 天 | Phase B7 |
| 字符串扩展 + `FLOAT` + `NOT` | 1 天 | Phase B8 + B9 |
| `PRINT_ERR` + 附录 G 注册表 | 0.5 天 | Phase B10 + B11 |
| **Phase B 缓冲** | **1 天** |  |

### Phase C 任务分解(0.5 周)

| 任务 | 周期 | 备注 |
|------|------|------|
| AS 删除 + 迁移文档 | 0.5 天 | Phase C1 |
| MODULE_REF + 类值语法 | 1 天 | Phase C2 |
| language_version + E0044 | 0.5 天 | Phase C3 |
| MVS 依赖求解 + E0045 | 1.5 天 | Phase C4 关键 |
| allow_builtin_shadow + E0025 | 0.5 天 | Phase C5 |
| E0042 lock-toml 一致性 | 0.5 天 | Phase C6 |
| 跨目录强化 | 0.5 天 | Phase C7 |
| **Phase C 缓冲** | **0.5 天** |  |

### Phase D 任务分解(1 周)

| 任务 | 周期 | 备注 |
|------|------|------|
| reqwest 依赖引入 | 0.5 天 |  |
| ASK 真实 HTTP(替换 mock) | 1 天 |  |
| ASK_STREAM 真实流式 | 1.5 天 | Phase D1 关键 |
| ASK_ALL per-element | 1 天 | Phase D2 |
| std.agent 5 个函数 stub | 0.5 天 | Phase D3 |
| 网络错误码细分 E0090-E0094 | 0.5 天 | Phase D4 |
| 模型名格式 W0052 | 0.5 天 | Phase D5 |
| **Phase D 缓冲** | **1 天** | 网络 / 凭据 / mock 兼容 |

### Phase E 任务分解(1 周)

| 任务 | 周期 | 备注 |
|------|------|------|
| strict_types 边界检查 + E0033 | 1.5 天 | Phase E1 |
| `wlwl fmt` + 10 条规则 | 2 天 | Phase E2 关键 |
| AST 稳定 node ID + hash | 1 天 | Phase E3 |
| W0020 linter walk + 全部 W 码统一通道 | 1 天 | Phase E4 |
| **Phase E 缓冲** | **1 天** |  |

### Phase F 任务分解(0.5 周)

| 任务 | 周期 | 备注 |
|------|------|------|
| `cargo bench` 框架 + 5 个基准 | 1 天 | Phase F1 |
| 解释器热点内联 + flamegraph | 1 天 | Phase F2 |
| Release profile 调优 | 0.5 天 | Phase F3 |
| 闭包 cell 共享验证 | 0.5 天 | Phase F4 |

### Phase G 任务分解(1 周,本计划独立追加)

| 任务 | 周期 | 备注 |
|------|------|------|
| G1 clippy + rustfmt | 1 天 |  |
| G2 cargo-deny | 0.5 天 |  |
| G3 rustdoc 100% | 1 天 |  |
| G4 ADR 6 个(ADR-008 ~ ADR-013) | 1 天 |  |
| G5 miri 验证 | 0.5 天 | 0 unsafe,主要流程演练 |
| G6 cargo-fuzz 3 target | 1.5 天 |  |
| G7 错误信息质量 pass 70 个码 | 1 天 |  |
| G8 性能回归(沿用 F1 基准) | 0.5 天 |  |
| G9 供应链监控 | 0.5 天 |  |
| G10 SBOM + 签名 | 0.5 天 |  |
| G11 mkdocs 文档站 | 1 天 |  |
| G12 README + examples 5+5 | 1 天 |  |
| **Phase G 集成 + CI 配置** | **1 天** |  |
| **Phase G 缓冲** | **1 天** |  |

### Phase H 任务分解(0.5 周)

| 任务 | 周期 | 备注 |
|------|------|------|
| H1 一致性测试套件 10 项 | 1.5 天 | **最大** |
| H2 CI 跨平台 + release.yml | 0.5 天 |  |
| H3 tag + push | 0.1 天 |  |
| H4 release.yml 异步构建 | 异步 |  |
| H5 GitHub Release 制品 + notes | 0.5 天 |  |
| H6 Post-release 监控 | 持续 |  |

---

**文档结束。生成时间:2026-09-05。**

---

## 14. 附录 D:实施进度跟踪

> v0.2 阶段沿用 v0.1 §实施进度跟踪格式。每 phase 收尾 git add + commit + push,工作树保持可回滚状态。Phase G 门禁 CI 必跑。

> **占位 — v0.2 各 phase 收尾时填充**(参考 v0.1 §实施进度跟踪格式):

### Phase A 收尾(待填)

### Phase B 收尾(待填)

### Phase C 收尾(待填)

### Phase D 收尾(待填)

### Phase E 收尾(待填)

### Phase F 收尾(待填)

### Phase G 收尾(待填)

### Phase H 收尾 + v0.4.0 发布(待填)
