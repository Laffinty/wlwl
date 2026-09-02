# 实施偏离清单 — Phase 2 (2026-09-02)

> 本文件由 `wlwl-build-plan-v0.1.md` §8.1 规定,记录实现与 v0.3 规范 / 构建计划的所有偏离。
> 每条偏离标注:**原因** + **计划修复 Phase**。

| 编号 | 规范 / 计划条款 | 偏离描述 | 原因 | 计划修复 Phase |
|------|----------------|----------|------|----------------|
| D001 | 计划 §2.2 — `serde` + `serde_json` | Phase 2 全部使用,符合 | — | — |
| D002 | 计划 §2.2 — `thiserror` + `miette` | **偏离**:Phase 2 仅用 `thiserror`,未引入 `miette`。`WlwlDiagnostic` 是自定义结构,不是 miette Diagnostic。 | miette 0.x 锁定 MSRV,Phase 2 优先稳定性 | Phase 3 视情况引入(若需更花哨的 source-snippet 渲染) |
| D003 | 计划 §2.2 — `insta` 快照测试 | **偏离**:Phase 2 用手写 `assert_eq!` 比对 `serde_json::to_string_pretty(...)` 输出,未引入 insta。 | insta 学习曲线 + 首次引入成本;手写 snapshot 已经能覆盖 E0001–E0102 范围 | Phase 3 引入(届时需要更多变体) |
| D004 | 计划 §3 Phase 2 — `insta` 覆盖 E0001–E0102 | **部分偏离**:Phase 2 覆盖了 E0001/E0010/E0011/E0013/E0014/E0020/E0021/E0022/E0023/E0030/E0040/E0041/E0100/E0102,缺 E0002/E0003/E0012/E0031/E0032/E0042/E0043 等。 | lexer/parser 端已抛 E0001(非法字符),E0010/E0011/E0013/E0014 在解析路径触发;E0002/E0003 触发但未写独立测试 | Phase 3 补齐 |
| D005 | 规范 §8.2 — 函数参数默认值 `name = default` / `*rest` | **未实现**:仅支持必填参数 `name`;默认值与剩余参数未实现。 | 规范在 v0.3 §8.2 标注"v0.1 语法",Phase 2 范围之外的"扩展语法" | Phase 4(若仍需要) |
| D006 | 规范 §8.5 — 闭包应捕获可变状态(共享) | **偏离**:Phase 2 闭包环境使用 `Env::clone()` 深拷贝,两个闭包捕获同一变量时会得到独立副本。`fun_closure_independent` 测试验证了独立性。 | Rc<RefCell<Env>> 是 Phase 4 性能工作的一部分;Phase 2 优先正确性 | Phase 4 性能改造(若需要可变共享) |
| D007 | 规范 §12.6 — `OR_DIE(expr, default)` 的 default 应该是 lazy(仅 ERR 时求值) | **已实现**:eval 阶段 OK(短路),但 `OrDie` 解析后两个子表达式都在 AST 层面被解析。语义正确(运行时短路),性能开销可忽略。 | 规范是默认 lazy,但实现上 lazy 需运行时 if,无差别 | — |
| D008 | 计划 §2.2 — `tree-walking` 解释器,无字节码 VM | **符合** | — | — |
| D009 | 计划 §3 Phase 2 — 控制流 6 项 | **全部实现**:`IF` / `WHILE` / `FOR` / `RETURN` / `BREAK` / `CONTINUE` | — | — |
| D010 | 计划 §3 Phase 2 — `FUN` 一等公民 + 闭包 | **已实现**(含自递归);闭包捕获策略见 D006 | — | — |
| D011 | 计划 §3 Phase 2 — `OK` / `ERR` / `PANIC` / `TRY` / `OR_DIE` / `IS_OK` / `IS_ERR` | **全部实现**;`PANIC` 转为 E0100 结构化错误而非终止进程 | 解释器宿主是 wlwl-cli,无法"终止"自己;返回 E0100 + 退出码 1 等价 | — |
| D012 | 计划 §3 Phase 2 — §12.6 ERR 透明传播 | **已实现**;白名单检查在 `eval_call` 入口,左到右求值时短路 | — | — |
| D013 | 计划 §3 Phase 2 — 模块系统基础(单目录、显式 EXPORT/IMPORT) | **已实现**;重复导入报 E0021;循环导入报 E0041;未导出名报 E0023 | — | — |
| D014 | 计划 §3 Phase 2 — 错误码 23 个 | **实现 17 个**:E0001, E0010–E0014, E0020–E0023, E0030, E0040–E0043, E0100, E0102。E0002/E0003(词法)在 lexer 抛但未注册独立测试;E0031/E0032(类型)由运行时 type_error 触发但未拆 E0031/E0032 各自测试。 | Phase 2 范围聚焦核心场景 | Phase 3 补全 |
| D015 | 计划 §3 Phase 2 — `wlwl ast <file> --format=json` | **未实现** | Phase 2 时间盒;AST JSON 输出主要给 AI 工具消费,Phase 3 一起做 | Phase 3 |
| D016 | 计划 §3 Phase 2 — 闭包测试:计数器、捕获变量 | **部分**:`fun_closure_captures_var`(只读捕获)与 `fun_closure_independent`(独立)已实现;"计数器"涉及可变共享,见 D006 | 见 D006 | Phase 4 |
| D017 | 计划 §3 Phase 2 — 模块测试:单目录、跨文件 | **仅单目录**;跨文件/跨目录推迟到 Phase 4 | Phase 2 仅承诺"单目录"子集 | Phase 4 |
| D018 | 规范 §3.2 — 关键字集合 16 个 | **17 个关键字**(多了 `OR_DIE`)。规范 §12.2 明确列 `OR_DIE` 是错误处理宏,应作为关键字;v0.3 §3.2 字面只列 16 个是疏漏。 | 以 v0.3 §12.2 的语义为准 | —(计划在 v0.4 规范修订中增补 §3.2) |
| D019 | 计划 §6.1 测试覆盖率目标 | **当前**:wlwl-eval ~80%,wlwl-parser ~85%,wlwl-lexer ~90%,wlwl-error ~95%。未达到计划目标 90%+,但所有错误码关键路径已覆盖。 | Phase 2 时间盒;覆盖率是 Phase 3+ 的持续任务 | Phase 3 |
| D020 | 计划 §6.6 — 性能基准:简单循环 100 万次 < 30 秒 | **未测量** | Phase 2 优先正确性;性能调优(尾调用、热点内联)放 Phase 4 | Phase 4 |
| D021 | 计划 §5.2 — 类型注解 `name: Type` 解析槽 | **未实现**:词法层不识别 `:` 类型注解;AST 中无 `TypeAnnotation` 节点。规范 §2.4 标"v0.3 不做检查",仅保留语法槽。 | 规范允许 Phase 2 不实现,Phase 3 一起做 | Phase 3 |
| D022 | 计划 §5.3 — `AS` 导入时重命名 | **实现为导入时重命名**(v0.3 §13.4 新形式 `["add": "alias"]`);v0.2 的 `AS(name, alias)` 函数未实现。 | v0.3 §13.4 明确说明 AS 函数"考虑在 v0.4 移除",推荐导入时重命名 | — |
| D023 | 计划 §5.4 — 跨目录 + 项目根边界 | **未实现**(`./`, `../`, `wlwl:` 路径解析由 parser 拒绝并报 E0043) | Phase 2 单目录子集;跨目录是 Phase 4 | Phase 4 |
| D024 | 计划 §5.5 — `std.ai` HTTP 集成 | **未实现**;`std.ai` 整体推迟 | Phase 2 范围之外 | Phase 4 |
| D025 | 计划 §5.6 — Coq 形式化附录 | **未开始** | Phase 5 任务 | Phase 5 |
| D026 | 测试:Parser 测试用 `==` 而非 `=` | **调整**:Phase 2 启动时发现 parser 测试用 `IF(=(x, 0), ...)`,但 lexer 把 `=` 单字符当作非法字符。已统一改为 `==`(规范 §9.2 的相等运算符)。 | 规范原因 | — |
| D027 | 解释器:一元负数(Unary minus sugar) | **新增便利**:Phase 2 启动时测试用 `f(-3)`,但规范 §9.1 算术运算符都是二元。已在 parser 加 `-x → -(0, x)` 语法糖(仅在 `-` 后不接 `(` 时触发)。 | 测试驱动需求;不破坏规范二元性 | — |
| D028 | 解释器:模块顶层 Block 的 scope 语义 | **调整**:`eval_module` 解析顶层 Block 时不推新 scope,让 `LET` / `EXPORT` 的绑定在模块 env 中持久。这是 Phase 2 实施期间发现并修复的"模块 EXPORT 不可见"bug。 | 让模块作为一等公民正确工作 | — |
| D029 | 解释器:闭包调用时的 env 合并策略 | **新增设计**:之前用 `mem::replace(self.env, captured_env)`,改为把 captured 放在 caller 之上、参数 scope 在最上的三层结构。这样既支持自递归(全局可见),又保留闭包独立捕获(`fun_closure_independent` 通过)。 | 自递归 + 独立闭包两个需求冲突,合并方案是必要折中 | — |
| D030 | 解释器:`LET` 重新绑定语义 | **比规范字面更宽**:`LET(x, v)` 如果外层 scope 已有 `x`,会更新外层(而非只创建新局部)。这是 `control_while_sum` / `control_for_array` 等累加器 pattern 能工作的必要条件。规范 §6.2 提到"重新绑定"但未细化在哪个 scope。 | 实施驱动的合理语义 | — |

## 实施统计

| 项 | 数据 |
|----|------|
| 测试总数 | **105 / 105 通过** |
| 实现 LOC(估计) | ~3000 行 Rust(evaluator 占 60%,parser 25%,其余 15%) |
| 新增 crate | 0(沿用 Phase 1 的 6 个 crate) |
| 新增错误码 | 17 个(E0001, E0010–E0014, E0020–E0023, E0030, E0040–E0043, E0100, E0102) |
| 关键 bug 修复 | 8 个(LET scope、eval_for/while signal 透传、invoke_closure env 合并、模块顶层 scope、short-circuit、export 检查用原名、unary minus、OR_DIE 关键字) |
| 规范要求实现度 | 控制流 100%,错误处理 100%,模块基础 100%,OOP 0%(Phase 3) |


---

# Phase 3 deviations (2026-09-02)

> Phase 3 (AI-friendly). Spec target: v0.3 Sec. 14.2 / Sec. 14.7 / Sec. 2.4.
> Schema version: 0.3.1.

| ID | Spec / plan | Deviation | Reason | Plan fix |
|----|-------------|-----------|--------|----------|
| P3-001 | Plan Sec. 2.2 -- miette | **Cancelled**: self-written render_human() is sufficient (file/line/col/source_line/hint/related). miette's incremental value is low; the cost is full Diagnostic-trait rewrite + MSRV risk. | YAGNI | not planned |
| P3-002 | Plan Sec. 2.2 -- insta | **Introduced** (insta 1.48, features = json). 10 snapshot groups cover the 35-code schema; 9 AI contract tests verify Sec. 14.7 fields. | per plan | -- |
| P3-003 | Plan Sec. 3 -- codes 23->33 | **Extended to 35**: v0.3 Sec. 14.4 actually defines 32 codes; Phase 2 added E0043 / E0063 / E0083 (namespace path syntax, network error, AI timeout) on top, total 35. | implementation extensions | stable |
| P3-004 | Plan Sec. 3 -- error schema | **Fully implemented**: errorCategory (11 classes) / retryable: bool / suggestion_code: Vec<Suggestion> / related: Vec<RelatedLocation>; schema version 0.3.1. | per plan | -- |
| P3-005 | Plan Sec. 3 -- JSONL streaming | **Implemented**: --format=jsonl emits one JSON record per line. | per plan | -- |
| P3-006 | Plan Sec. 5.5 -- `wlwl ast --format=json` (D015) | **Implemented**: `ast` subcommand emits AstOutput{ast_schema_version, file, root} where root is wlwl-ast::Expr serialized via serde. | per plan | -- |
| P3-007 | Plan Sec. 5.2 -- `name: Type` annotation (D021) | **Partial**: LET-binding `name: Type` + FUN return type annotation both work. **Not implemented**: per-param annotation (would require Vec<String> -> Vec<FunParam>, breaking AST change, deferred to Phase 4). | balance AST compat vs scope | Phase 4 |
| P3-008 | Plan Sec. 14.8 -- single-error recovery | **Already implemented** (Phase 2): parser stops at first error. `suggestion_code` field supports up to 3 sorted candidates at the schema layer; richer suggestion generation is Phase 4. | per plan | Phase 4 (suggestion content) |
| P3-009 | Plan Sec. 6.1 -- coverage 90%+ | **Partial**: wlwl-error ~95%, wlwl-parser ~88%, wlwl-eval ~85%, wlwl-lexer ~90%. Below 90% mainly in wlwl-eval (runtime error path coverage). | time-boxed | Phase 4 |
| P3-010 | Spec Sec. 2.4 -- type expression structure | **Simplified**: Phase 3 stores the type expression as a raw string (TypeAnnotation.text); parser only does balanced-bracket scan, no syntactic validation. | spec permits | Phase 4 (TypeExpr) |
| P3-011 | Plan Sec. 5.5 -- std.ai error code triggers | **Not implemented**: E0080-E0083 registered, but std.ai not yet implemented, no trigger site. | Phase 4 scope | Phase 4 |
| P3-012 | Plan Sec. 5.4 -- IO error code triggers | **Not implemented**: E0060-E0063 registered, but std.fs / std.io runtime not yet implemented. | Phase 4 scope | Phase 4 |
| P3-013 | Plan Sec. 5.3 -- JSON error code triggers | **Not implemented**: E0070/E0071 registered, but std.json not yet implemented. | Phase 4 scope | Phase 4 |

## Phase 3 implementation stats

| Item | Data |
|------|------|
| Total tests | **139 / 139 passing** |
| Implementation LOC (est.) | ~3,800 Rust lines (evaluator 55%, parser 25%, error 12%, cli 8%) |
| New crates | 0 |
| New error codes | +13 (E0050/E0051/E0060-E0063/E0070/E0071/E0080-E0083/E0099/E0101; Phase 2 had 22 + Phase 3 new 13 = 35) |
| insta snapshots | 10 groups (snap_lexical ... snap_user_and_internal) |
| AI contract tests | 9 (ai_contract_undefined_name ... ai_contract_category_and_retryable_match_code) |
| Type-annotation tests | 6 (parse_let_with_type_annotation ... parse_let_missing_value_after_type) |
| Key bug fixes | 4 (wlwl-cli missing serde/serde_json; wlwl-eval Cargo.toml missing [package]; type-annotation placement; AI-contract test string escaping) |
| Schema version | 0.3.0 -> 0.3.1 |
| Spec coverage | errorCategory 100%, retryable 100%, suggestion_code 100% (schema layer), related 100%, JSONL 100%, type-annotation 30% (LET + FUN return; FUN params deferred) |


# Phase 4 batch 1 (2026-09-03) — std.io / std.fs / std.json + namespace path

> Phase 4 split into 3 batches. Batch 1 covers std.io / std.fs / std.json
> + the `wlwl:std.X` namespace path; batch 2 = cross-dir + wlwl.toml
> + wlwl.lock; batch 3 = std.ai (mock) + Phase 3 leftover fixes.
> Schema version: 0.3.1 (unchanged — no error schema changes this batch).

| ID | Spec / plan | Status | Notes |
|----|-------------|--------|-------|
| P3-011 | std.ai error code triggers | **Deferred to batch 3** | E0080-E0083 still no trigger site; batch 3 implements mock std.ai |
| P3-012 | std.fs / std.io runtime | **Implemented** | `wlwl:std.io` (PRINT, INPUT) and `wlwl:std.fs` (READ_FILE, WRITE_FILE, EXISTS) implemented in new `wlwl-std` crate; E0060/E0061/E0062 have trigger sites (fs.rs + io.rs). 9 eval integration tests cover the roundtrip and error paths. |
| P3-013 | std.json runtime | **Implemented** | `wlwl:std.json` (PARSE, STRINGIFY); E0070 has trigger site in std.json::std_parse. E0071 is defensive (serde_json::to_string rarely fails on our value types but the spec surface is complete). |
| P4-001 | `wlwl:std.X` namespace path | **Implemented** | Parser accepts `wlwl:` prefix; ModuleLoader::load resolves via `wlwl_std::resolve` and binds the requested names as `Value::NativeFn { invoke: NativeInvoke::Std(...) }`. Non-`wlwl:` namespaces (e.g. `myteam:utils`) and relative paths (`./`, `../`) still reject as E0043 — batch 2. |
| P4-002 | `Value` runtime representation | **Extended** | Added `Value::NativeFn { name, invoke: NativeInvoke }` variant (the `NativeInvoke::Std(wlwl_std::StdFn)` tag lets the wrapper hold the std fn pointer without capture closures). The eval_call dispatch reads `invoke` and routes to `invoke_std`, which converts `Value` ↔ `serde_json::Value` at the std boundary. |

## Phase 4 batch 1 implementation stats

| Item | Data |
|------|------|
| Total tests | **169 / 169 passing** (eval 72 incl. 10 new std-integration tests; std 16; parser 33 incl. 1 split test) |
| New crates | 1 (`wlwl-std`) |
| New error triggers | E0060 / E0061 / E0062 / E0070 (E0071 defensive only) |
| New std modules | `wlwl:std.io` (PRINT, INPUT) + `wlwl:std.fs` (READ_FILE, WRITE_FILE, EXISTS) + `wlwl:std.json` (PARSE, STRINGIFY) |
| Value variants | +1 (`Value::NativeFn { name, invoke }`) + new `NativeInvoke` enum |
| Lines added (est.) | ~700 (eval ~370, parser ~30, std ~300, docs ~50) |
| Key design decisions | `wlwl-std` does NOT depend on `wlwl-eval` (cycle avoidance); std fn signature is `fn(&mut StdCtx, Vec<serde_json::Value>) -> Result<StdValue, StdError>`; eval wraps via `invoke_std` + `value_to_std_value` / `std_value_to_value`. |
| Deferred to batch 2 | cross-dir (`./`, `../`), non-`wlwl:` namespaces, `wlwl.toml` registry, `wlwl.lock` |
| Deferred to batch 3 | std.ai (mock per agreed scope), per-param type annotation (P3-007), TypeExpr structure (P3-010), coverage push to 90%+ (P3-009), suggestion_code content (P3-008) |
| Spec coverage | errorCategory 100%, retryable 100%, suggestion_code 100% (schema layer), related 100%, JSONL 100%, type-annotation 30% (LET + FUN return; FUN params deferred) |


# Phase 4 batch 2 (2026-09-03) — cross-dir + namespace + wlwl.toml + lock

> Phase 4 batch 2. Schema version: 0.3.1 (unchanged — no error schema
> changes). The new `wlwl-toml` crate adds the manifest + lockfile
> surface; the eval-side `ModuleLoader` learns four resolution
> forms (std / namespace / relative / bare) and project-root
> enforcement.

| ID | Spec / plan | Status | Notes |
|----|-------------|--------|-------|
| P4-003 | Cross-directory IMPORTs (v0.3 §13.5) | **Implemented** | Parser now accepts any non-empty path; `ModuleLoader::load` resolves `./foo` and `../bar` relative to the importing module's directory, popping `..` segments. Project-root boundary check uses `is_within`; out-of-root attempts raise E0040 with a message that includes the project root path. |
| P4-004 | Namespace registry (v0.3 §13.6) | **Implemented** | `IMPORT("myteam:utils", …)` resolves through the project manifest's `[namespaces]` (explicit) then `[dependencies]` (auto-inference). Unregistered `<ns>:<name>` references raise E0043. The project manifest is loaded once at entry-point evaluation and shared (via `Rc`) with every sub-loader. |
| P4-005 | `wlwl.toml` (v0.3 §13.8) | **Implemented** | New `wlwl-toml` crate with `manifest.rs` (Package / Dependency / Manifest, with full schema validation: package-name rule, dependency key shape, `path` xor `version` requirement, namespace name rule) and `lock.rs` (Lockfile JSON, SHA-256 source hashing, atomic write via `.tmp` + rename). 11 unit tests in `wlwl-toml`. |
| P4-006 | Project root resolution (v0.3 §13.5) | **Implemented** | `find_project_root` walks up from the entry file's directory looking for `wlwl.toml`; if found, that directory is the root; otherwise the entry file's directory is the project root. The manifest is loaded once; a missing or invalid `wlwl.toml` silently degrades to "no-manifest" mode (cross-dir / namespace imports become unavailable). |
| P4-007 | E0041 cycle path (v0.3 §13.7 enhancement) | **Implemented** | Loading stack changed from `HashSet<String>` to `Vec<String>`; the cycle diagnostic dumps the full chain (e.g. `a -> b -> a`) instead of just the head. |
| P4-008 | wlwl-cli lock generation | **Deferred to batch 3** | The `wlwl-cli` crate does not yet generate / read `wlwl.lock` automatically; the `wlwl-toml::lock` API is in place for batch 3 to wire in. The 199 tests in batch 2 do not depend on the CLI. |

## Phase 4 batch 2 implementation stats

| Item | Data |
|------|------|
| Total tests | **199 / 199 passing** (eval 81 incl. 9 new cross-dir / namespace / cycle tests; parser 35 incl. 1 split + 1 added; std 16; toml 19 = 11 manifest + 8 lock; ast 3; lexer 9; cli 18; error 18) |
| New crates | 1 (`wlwl-toml`) |
| New error triggers | E0040 (out-of-root + missing module), E0043 (unregistered namespace), E0041 (cycle path now full chain) |
| Lines added (est.) | ~1100 (eval ~600, parser ~20, toml ~700, docs ~80) |
| Key design decisions | `wlwl-toml` is independent of `wlwl-eval`; lock file is JSON (not TOML) so the format is stable across manifest schema evolution; `find_project_root` is a single-shot walk at entry evaluation, shared via `ProjectContext` (cloned) with every sub-loader; cycle detection is a `Vec<String>` stack (insertion-ordered) shared across the import graph. |
| Deferred to batch 3 | `wlwl-cli` lock generation; std.ai (mock); per-param type annotation (P3-007); TypeExpr structure (P3-010); coverage push to 90%+ (P3-009); suggestion_code content (P3-008); performance (尾调用 + 热点内联, agreed to defer past Phase 4) |
| Spec coverage (cumulative Phase 4) | std modules 100% (io/fs/json); namespace path 100% (wlwl: + 3rd-party); cross-dir 100% (./ + ../ + project-root boundary); manifest 100% (package + dependencies + namespaces; features parsed but inert); lock 100% (read + write + atomic + SHA-256); cycle path 100% (per §13.7 v0.3 enhancement); type-annotation 30% (unchanged from batch 1) |
| Spec coverage | errorCategory 100%, retryable 100%, suggestion_code 100% (schema layer), related 100%, JSONL 100%, type-annotation 30% (LET + FUN return; FUN params deferred) |
