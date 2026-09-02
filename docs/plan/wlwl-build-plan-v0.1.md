# WLWL v0.3 实施构建计划 v0.1(初版)

> **状态**:初版草案(关键决策已锁定,见 §0.1)
> **基于规范**:`docs/standard/wlwl-spec-v0.3(MD5_4308b3d2071ebed5cb52eba61272b1ea).md`
> **核心约束**:本计划**不属于** WLWL 语言规范。规范只定义"语言该长什么样",本计划定义"如何把语言造出来"。两者解耦——规范演进时,本计划同步跟进;但本计划的工程决策**不**回流到规范。
> **决策原则**:技术栈选型、模块划分、阶段划分等**均为建议**,最终决策权归 Li;本计划以"待 Li 拍板"为默认状态。

## 0.1 关键决策(2026-09-02 锁定)

| # | 决策项 | 决策 | 备注 |
|---|--------|------|------|
| 1 | 技术栈 | **接受**:Rust + tree-walking | 沿用 v0.2 选型 |
| 2 | 工期预算 | **接受**:20 周(Phase 1-4)/ 24 周(加 Phase 5) | AI Coding 工作流支持,接受延期风险 |
| 3 | Phase 5 形式化 | **做**:Coq 机械化 §19 核心子集 | 4 周 + 与 Rust 实现等价性测试 |
| 4 | CI 平台 | **GitHub Actions** | 默认;三平台测试(Linux/macOS/Windows) |
| 5 | 许可证 | **GPL v2** | 强 copyleft;编译器 + std 库 + 工具链统一许可;用户编写的 .wl 程序不受约束 |
| 6 | 目标用户 | **公开** | 需准备 README、文档站、发布流程;Phase 4 末启动公开筹备 |

---

## 0. 文档定位

| 维度 | WLWL 规范(v0.3) | 本构建计划 |
|------|------------------|------------|
| 范围 | 语言的语法、语义、标准库、错误信息格式 | 实现技术栈、阶段路线、模块划分、工具链 |
| 受众 | 编译器实现者、文档作者、AI 工具、用户 | 实施工程师、维护者 |
| 演进方式 | 严格版本化 + MD5 内容寻址(v0.3 已采用) | 与规范版本号解耦,独立版本号 |
| 决策依据 | 学术共识、工业实践、AI 友好 | 工程实际、团队能力、时间预算 |

> **关键提示**:v0.3 规范 §17 实现参考已删除,§16 工具设计原则不规定具体命令。这意味着 v0.3 规范**未**对实现做任何约束,所有工程决策**由本计划决定**。

---

## 1. 实施总览

### 1.1 目标

在 **6 个月内**(粗略,待 Li 拍板)交付一个可运行的 WLWL v0.3 实现,核心能力:
- 支持 v0.3 全部核心子集(§1–§18)
- 工具链最小集:`wlwl run` / `wlwl check`
- AI 友好错误输出:`--format=json` + `--format=jsonl`
- `std.io` / `std.json` 完整实现
- 形式化附录(§19)的**部分**机械化(在 Coq 或 Rust 内部 DSL)

### 1.2 原则(与规范 §1.2 设计原则对齐)

1. **简单优于聪明**:技术选型优先考虑"团队已掌握"或"学习曲线低",不追新。
2. **可测试性优先**:每个核心语义必须有独立单元测试,关键错误码必须有 insta 快照。
3. **AI 友好是硬要求**:错误信息必须符合 §14 全部 schema,任何"近似但不一致"视为 bug。
4. **规范变更的容错**:实现与规范可能短期不一致(规范演进快),但每个不一致必须**显式记录**在 §10 偏离清单。

### 1.3 风险等级(初评)

| 风险 | 等级 | 缓解 |
|------|------|------|
| ERR 透明传播(§12.6)实现复杂度 | 中 | 先跑通,再优化;参考 §19 形式化定义 |
| 树遍历解释器性能 | 高 | 早期就埋点(profile),v0.4 决定是否引入字节码 |
| 跨平台兼容(Win/macOS/Linux) | 中 | CI 三平台,优先 Linux/macOS,Windows 最后 |
| AI 工具集成验证 | 中 | §14.7 契约**逐条**写测试用例 |
| 形式化附录机械化 | 高 | v0.1 阶段只做最小子集;若成本过高,降级为"自然语言附录 + 实现内自检" |

---

## 2. 技术栈选型(建议,待 Li 拍板)

### 2.1 核心选型

| 维度 | 选择 | 理由 | 备选 |
|------|------|------|------|
| **实现语言** | **Rust** | 内存安全、错误处理生态丰富(miette)、tree-walking 简单 | Zig / OCaml / Go |
| **执行模型** | **Tree-walking 解释器**(递归 → 显式栈) | 保留完整 AST 位置信息,精准报错;实现简单,迭代快 | 字节码 VM(性能更好,但实现复杂) |
| **解析器** | **手写递归下降** | 错误信息精确,完全控制 | `pest` / `nom` / `lalrpop`(模板化好,但错误信息更难做) |
| **AST 表示** | `enum Expr` + `Box<Expr>` + `Rc<Expr>`(共享) | Rust 习惯;支持闭包共享 | typed-arena(分配更快) |
| **错误处理** | `thiserror`(内部) + `miette`(用户) | `miette` 是 2025-2026 Rust 错误信息事实标准 | `anyhow`(信息不够结构化) |
| **序列化** | `serde` + `serde_json` | `--format=json/jsonl` 必备 | `simd-json`(性能,但 v0.1 不需要) |
| **测试** | `cargo test` + `insta`(快照) | insta 是错误码快照的工业标准 | `expect-test` / `k9` |
| **CLI** | `clap`(v4,derive) | 标准 | `argh`(更轻量) |
| **配置文件** | `toml` + `serde` | `wlwl.toml` 必备 | `ron` / `json` |

### 2.2 备选技术栈对比

#### Zig

- **优势**:更接近 C 的控制力,无 GC,适合解释器
- **劣势**:生态比 Rust 薄 5–10 年,错误处理不如 miette 成熟
- **结论**:**不推荐**——Rust 生态对 v0.3 规范更友好

#### OCaml

- **优势**:函数式优先,与 WLWL 美学天然契合;AST 处理强大
- **劣势**:Windows 支持差;国内 Rust 工程师比 OCaml 多
- **结论**:**不推荐**——生态与人手限制

#### Go

- **优势**:开发速度快,部署简单
- **劣势**:错误处理弱(无法做 ERR 透明传播的细粒度控制);泛型弱
- **结论**:**不推荐**——ERR 透明传播是 v0.3 核心语义,Go 难以优雅实现

### 2.3 决策待 Li 拍板

- [ ] **实现语言**:Rust(默认建议) / Zig / 其他?
- [ ] **执行模型**:Tree-walking(默认建议) / 字节码 VM / AOT?
- [ ] **解析器**:手写递归下降(默认) / `pest` / `nom`?
- [ ] **内存管理**:AST 用 `Box`(默认) / `Rc` 共享(子表达式优化)?

---

## 3. 分阶段路线图

### Phase 1:MVP(目标 4 周,对应 v0.1 规范子集)

**目标**:能跑通 `LET(x, 1); PRINT(x);` 这种最简单的程序。

**交付物**:
- 词法分析器(支持关键字、字面量、`;` `(` `)` `,`)
- 语法分析器(LET / 函数调用 / 块表达式)
- 简单求值器(字面量、变量、函数调用)
- `wlwl run <file>` 命令
- `PRINT` 内置函数

**测试**:
- 5 个基础集成测试(空程序、单表达式、多语句、嵌套、错误捕获)

**规范对应**:§3 词法、§4 字面量、§5 表达式(子集)、§6.1 LET

**不在本阶段**:
- 错误处理(无 OK/ERR/TRY)
- 模块系统
- OOP
- 结构化错误

### Phase 2:核心语义(目标 6 周,对应 v0.2 规范子集)

**目标**:支持完整控制流、函数、错误处理,达到 v0.2 水平。

**交付物**:
- 完整控制流(IF / WHILE / FOR / RETURN / BREAK / CONTINUE)
- 函数一等公民 + 闭包
- 错误处理(OK / ERR / PANIC / TRY / OR_DIE / IS_OK / IS_ERR)
- **§12.6 ERR 透明传播**——本阶段最关键
- 模块系统基础(文件即模块、显式 EXPORT/IMPORT)
- 重复导入报错(§13.3)
- 循环导入检测(§13.7)
- 完整词法 + 语法 + 解析错误码

**测试**:
- 错误处理专题:至少 20 个错误传播场景
- 闭包测试:计数器、捕获变量
- 模块测试:单目录、跨文件
- 错误码快照:insta 覆盖 E0001-E0102

**规范对应**:§6–§13(模块子集)、§14.4 错误码 23 个

**关键挑战**:
- ERR 透明传播的正确性:必须用 §19.6 形式化定义做交叉验证
- 闭包的环境捕获:用 `Rc<RefCell<Env>>` 模式

### Phase 3:AI 友好(目标 4 周,对应 v0.3 规范)

**目标**:达到 v0.3 全部能力,特别是 AI 工具契约。

**交付物**:
- 错误信息 schema 完整实现:`errorCategory` / `retryable` / `error_schema_version` / `suggestion_code`
- JSON + JSONL 双输出模式
- 错误码扩展:23 → 33(IO / JSON / std.ai / 网络)
- 单错误恢复(§14.8)
- 类型注解语法槽(§2.4,**仅解析,不检查**)
- DICT 键严格相等(§2.3)

**测试**:
- AI 契约测试:模拟 AI 工具消费 JSONL,逐字段验证
- `suggestion_code` 应用测试
- 错误码分类(13 个 errorCategory)覆盖

**规范对应**:§14 全部 + §15 std.ai 占位

### Phase 4:扩展(目标 6 周)

**目标**:补齐 std.ai、跨目录、包管理、性能优化。

**交付物**:
- `std.ai` 完整实现(§15.11):ASK / EMBED / COMPLETE 同步调用
  - HTTP 客户端:`reqwest`
  - 错误码 E0080-E0083
- 跨目录引用(§13.5):相对路径 + 项目根边界
- 命名空间路径(§13.6):`wlwl:` 标准库 + 第三方包
- `wlwl.toml` 解析(§13.8)
- `wlwl.lock` 生成(基本算法)
- DICT 遍历插入序(已规范,v0.3 实现保证)
- Tree-walking 优化:尾调用优化、热点内联

**测试**:
- std.ai 集成测试(可选,可能用 mock 模式)
- 大型项目测试(50+ 模块)
- 性能基准:简单循环 100 万次 < 30 秒

**规范对应**:§13.5–§13.8、§15.11

### Phase 5:形式化附录(目标 4 周,可选)

**目标**:把 §19 的核心子集用 Coq 或 Rocq 机械化,作为实现正确性的"参考实现"。

**交付物**:
- Coq 项目:WLWL-Core,定义核心表达式的 small-step 求值
- 关键定理机械化:§19.6 ERR 透明传播白名单 + 推论
- 实现与 Coq 模型的等价性测试(关键 case)

**为什么放最后**:v0.1 规范不强制形式化,§19 是"无歧义参考"而非"必须机械化"。如果实施时间紧张,**可降级为**:自然语言 §19 + Rust 内部"reference impl"作为一致性对照。

**规范对应**:§19

### 总时间估算

| Phase | 目标周期 | 累计 |
|-------|----------|------|
| Phase 1 | 4 周 | 4 周 |
| Phase 2 | 6 周 | 10 周 |
| Phase 3 | 4 周 | 14 周 |
| Phase 4 | 6 周 | 20 周 |
| Phase 5 | 4 周(可选) | 24 周 |

**总计**:20 周(不含 Phase 5)/ 24 周(含 Phase 5)。粗略估算,实际取决于团队规模和并行度。

---

## 4. 关键模块划分(基于 Rust crate 结构)

### 4.1 顶层 crate

> **修正(2026-09-02)**:为避免污染工作目录根,Rust workspace 实际放在 `wlwl/impl/` 子目录。根目录 `wlwl/` 保留为 `docs/`(规范、历史、计划)的容器。

```
wlwl/                          # 工作目录根
├── docs/
│   ├── standard/              # WLWL 规范(v0.3 现行)
│   ├── history/               # 历史归档
│   └── plan/                  # 构建计划
└── impl/                      # [2026-09-02 修正]Rust workspace 根
    ├── Cargo.toml
    ├── crates/
    │   ├── wlwl-lexer/        # 词法分析
    │   ├── wlwl-parser/       # 语法分析 → AST
    │   ├── wlwl-ast/          # AST 定义(共享类型)
    │   ├── wlwl-eval/         # 求值器(tree-walking)
    │   ├── wlwl-module/       # 模块解析、IMPORT/EXPORT(Phase 2+)
    │   ├── wlwl-error/        # 错误定义 + miette 集成
    │   ├── wlwl-std/          # std.io / std.fs / std.json / std.ai(Phase 1 只 std.io)
    │   ├── wlwl-cli/          # wlwl run / check / fmt
    │   └── wlwl-formal/       # (Phase 5)Coq 桥接
    ├── tests/                 # 集成测试
    ├── examples/              # 示例 .wl 程序
    ├── LICENSE                # GPL v2
    └── README.md
```

### 4.2 核心数据结构(借鉴 v0.1 §17 思路,不再出现在规范,本计划独占)

```rust
// AST 节点(wlwl-ast)
pub enum Expr {
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
    Import { module: ModulePath, names: Vec<ImportName> },  // v0.3 扩展
    Export(Vec<ExportSpec>),  // v0.3 扩展,支持契约
    Ok(Box<Expr>),
    Err(Box<Expr>),
    Panic(Box<Expr>),
    IsOk(Box<Expr>),
    IsErr(Box<Expr>),
    OrDie(Box<Expr>, Box<Expr>),
    TypeAnnotation { expr: Box<Expr>, ty: Type },  // v0.3 类型注解(运行时忽略)
}

// Span(wlwl-ast 必备)
pub struct Span {
    pub file: String,
    pub line_start: u32,
    pub col_start: u32,
    pub line_end: u32,
    pub col_end: u32,
}

// 表达式必须携带 Span
pub struct Spanned<T> {
    pub node: T,
    pub span: Span,
}
```

### 4.3 求值器核心

```rust
// wlwl-eval
pub struct Evaluator {
    globals: Env,
    modules: ModuleRegistry,
    config: EvalConfig,  // strict_types 等
}

impl Evaluator {
    pub fn eval(&mut self, expr: &Spanned<Expr>) -> Result<Value, EvalError>;
    
    // §12.6 ERR 透明传播:在 call_with_args 入口检查
    fn call_with_args(&mut self, f: Value, args: Vec<Value>) 
        -> Result<Value, EvalError> {
        // 白名单检查:只有 IS_OK/IS_ERR/OR_DIE/TRY 可消费 ERR
        if let Some(err) = args.iter().find(|v| matches!(v, Value::Err(_))) {
            return Ok(err.clone());  // 透明传播
        }
        // 正常求值
        ...
    }
}
```

---

## 5. 关键技术挑战与对策

### 5.1 ERR 透明传播(§12.6,§19.6)

**挑战**:
- 普通函数(算术、字符串、I/O)对 ERR 透明,必须**在每个函数入口检查**
- 性能开销:每次调用都扫描参数
- 类型擦除:Value 是 enum,运行时无类型信息,无法"编译器强制"

**对策**:
- 在 `call_with_args` 统一拦截(避免每个内置函数单独写)
- 白名单函数(IS_OK/IS_ERR/OR_DIE/TRY)显式标记 `consumes_err: true`
- 单元测试覆盖 50+ 透明传播场景
- Phase 5:用 Coq 机械化定义,跑等价性测试

### 5.2 类型注解"无副作用"集成(§2.4,§2.6)

**挑战**:
- 解析时必须识别 `name: Type` 语法
- AST 中携带类型注解,但**运行时完全忽略**
- 不能让任何函数根据注解改变行为

**对策**:
- 解析器把注解收纳到 `TypeAnnotation` AST 节点,但求值器直接返回 `expr` 字段
- 不存储类型信息(零运行时开销)
- 工具/AI 可以单独走 AST 提取注解(不依赖运行时)

### 5.3 AS 导入时重命名(§13.4)

**挑战**:
- 解析时识别 `["name": "alias"]` 语法
- 重命名必须发生在 IMPORT 求值时,不是之后

**对策**:
- `Import` 节点的 `names` 字段改成 `Vec<ImportName>`,其中 `ImportName` 携带可选别名
- 解析时检测 `:` 分隔
- 求值时把别名作为新绑定的 key

### 5.4 跨目录 + 项目根边界(§13.5)

**挑战**:
- 项目根目录(有 `wlwl.toml` 的目录)是搜索的最高边界
- 相对路径、命名空间路径、简单名三种形式要正确解析
- 不能"越界"——避免引用外部代码

**对策**:
- Phase 4 之前先做 PoC:单目录多文件,验证 IMPORT 链式
- 跨目录用 `std::path::Path` 的归一化
- 项目根用"第一次找到 `wlwl.toml` 的祖先目录"作为边界
- 越界检测:在 ModuleResolver 中显式拒绝

### 5.5 std.ai HTTP 集成(§15.11)

**挑战**:
- 同步调用 vs 异步(I/O 阻塞)
- 超时、重试、错误传播
- 凭据管理(API key)

**对策**:
- 用 `reqwest` 同步 API(在 spawn_blocking 中执行,避免阻塞主线程)
- 超时:`tokio::time::timeout` 包装
- 凭据:仅从环境变量读取,**不**支持文件
- 错误映射:`reqwest::Error` → `E0080`/`E0081`/`E0082`

### 5.6 形式化附录(§19)

**挑战**:
- Coq/Rocq 学习曲线陡
- 与 Rust 实现的同步成本

**对策**(Phase 5):
- 只机械化合集:LET、IF、Call、ERR 传播
- 用 Coq extraction 自动生成 Rust 代码(可选)
- 关键性质做证明,其他用 property-based 测试

---

## 6. 测试策略

### 6.1 测试金字塔

```
                    ┌─────────────┐
                    │  AI 契约    │  (Phase 3+)
                    │  端到端     │
                ┌───┴─────────────┴───┐
                │   集成测试          │  (每 Phase 都加)
                │   (标准库 + 主程序) │
            ┌───┴─────────────────────┴───┐
            │     单元测试                │  (每个核心语义)
            │     (求值器、模块、错误)    │
        ┌───┴─────────────────────────────┴───┐
        │     错误码 insta 快照               │  (每个错误码)
        │     (33 个错误 + 8 个警告)          │
        └─────────────────────────────────────┘
```

### 6.2 单元测试覆盖目标

| 模块 | 目标覆盖率 |
|------|------------|
| 词法 | 100% |
| 语法 | 95% |
| 求值器(核心) | 90% |
| 求值器(std) | 80% |
| 模块解析 | 90% |
| 错误处理 | 95% |

### 6.3 错误码 insta 快照

每个错误码(E0001-E0102 + E0060-E0083 = 33 个)**至少 1 个** insta 快照,包含:
- 触发该错误的最小 .wl 程序
- 期望的 JSON 输出
- 期望的 CLI 人类可读输出

### 6.4 AI 契约测试(Phase 3+)

模拟 AI 工具消费 JSONL,逐字段验证:
- `error_schema_version` 存在
- `code` 在合法表内
- `severity` 合法
- `errorCategory` 与 code 一致(映射表)
- `retryable` 与错误类型语义一致
- `suggestion_code` 至少在常见错误中提供

### 6.5 端到端测试

每个 Phase 至少 5 个端到端 .wl 程序,覆盖:
- 基础 I/O
- 错误处理链
- 模块使用
- std.json / std.ai

### 6.6 性能基准(Phase 4+)

简单循环 100 万次 < 30 秒(在 Linux/macOS 基准机)。

---

## 7. 工具链规划

### 7.1 命令最小集(对应 v0.3 §16.3 "不在规范范围" 部分)

| 命令 | 作用 | Phase |
|------|------|-------|
| `wlwl run <file>` | 运行 .wl 程序 | 1 |
| `wlwl check <file>` | 仅做词法/语法/名字检查,不执行 | 1 |
| `wlwl run <file> --format=json` | 输出 JSON 错误(单条/数组) | 2 |
| `wlwl run <file> --format=jsonl` | 输出 JSONL 流式错误 | 3 |
| `wlwl check <file> --strict` | 警告变错误 | 3 |
| `wlwl fmt <file>` | 格式化(可选,Phase 3+) | 3 |
| `wlwl ast <file> --format=json` | 输出 JSON AST(供 AI 消费) | 2 |
| `wlwl repl` | REPL(可选,Phase 4+) | 4 |

### 7.2 命令行约定

- 所有错误输出**至少**支持 `--format=json`
- AI 工具**应**使用 `--format=jsonl`(v0.3 §14.7 契约)
- `--strict` 影响退出码(warning 也返回 1)
- 默认人类可读输出

### 7.3 配置文件(对应 §13.8)

- `wlwl.toml` 解析(Phase 4)
- `wlwl.lock` 生成与读取(Phase 4,基本算法)
- 配置文件错误信息也走 §14 schema

---

## 8. 与 v0.3 规范的同步机制

### 8.1 偏离清单(本计划独占,规范无)

任何**实现**与 v0.3 规范**不一致**的地方,必须记录在 `plan/deviations.md`:

```markdown
# 实施偏离清单

| 编号 | 规范条款 | 偏离描述 | 原因 | 计划修复 Phase |
|------|----------|----------|------|----------------|
| D001 | §2.4 类型注解 | 暂未解析 `name: Type` 语法 | Phase 1 不需要 | Phase 3 |
| D002 | §14.9 JSONL | 暂只支持 JSON | Phase 3 实现 | Phase 3 |
| ... | ... | ... | ... | ... |
```

### 8.2 规范变更的影响评估

每次规范(v0.3.md)MD5 变化时,**必须**重新评估本计划的影响范围:
- 哪个 Phase 受影响?
- 哪些模块需要重写?
- 时间预算是否需要调整?

### 8.3 规范 → 计划的双向同步

- 规范新条款 → 本计划对应 Phase 增加任务
- 本计划发现规范模糊 → 反馈给规范(在 v0.3.x 微调)
- 严禁实施时**默默**修改规范;规范变更必须**显式**走版本号

---

## 9. 风险登记

| ID | 风险 | 等级 | 缓解策略 | 触发条件 |
|----|------|------|----------|----------|
| R001 | ERR 透明传播实现错误 | 高 | Phase 2 写 20+ 测试,Phase 5 Coq 验证 | 任意 ERR 传播测试失败 |
| R002 | tree-walking 性能不达标 | 中 | Phase 4 性能基准,失败则引入字节码 | 100 万次循环 > 30 秒 |
| R003 | 形式化附录成本过高 | 高 | Phase 5 可降级,只做核心子集 | Phase 5 启动时评估工作量 |
| R004 | 规范演进速度 > 实施速度 | 中 | 偏离清单,延迟实施 | v0.3.x 修订 > 每月 1 次 |
| R005 | Windows 兼容性 | 中 | CI 三平台测试,优先 Linux/macOS | Windows CI 失败 |
| R006 | AI 工具契约验证困难 | 中 | 写模拟 AI 消费脚本,逐条测试 | AI 工具报告错误信息解析失败 |
| R007 | std.ai 网络问题(国内访问 OpenAI) | 高 | 提供 mock 模式,默认不调用真实 API | std.ai 测试连接失败 |
| R008 | 学习 Rust async 栈(本计划用 sync) | 低 | 用 `reqwest::blocking` + `spawn_blocking` | 无 |

---

## 10. 决策清单

> **状态:6 项关键决策已锁定**(见 §0.1)。本节保留作为"未来补充决策"的入口。

### 10.1 已锁定(2026-09-02)

| # | 决策项 | 决策 |
|---|--------|------|
| 1 | 技术栈 | Rust + tree-walking |
| 2 | 工期预算 | 20 周(Phase 1-4)/ 24 周(含 Phase 5) |
| 3 | Phase 5 形式化 | 做 |
| 4 | CI 平台 | GitHub Actions |
| 5 | 许可证 | GPL v2 |
| 6 | 目标用户 | 公开 |

### 10.2 未来待定(Phase 2+)

- [ ] std.ai 默认 LLM 提供商(OpenAI / Anthropic / 本地 Ollama)
- [ ] std.json 库选择(`serde_json` / `simd-json` / 自研)
- [ ] LSP 优先级(Phase 4 末评估)
- [ ] 包管理中央仓库协议(Phase 5+)
- [ ] 是否提供 WASM 目标(远期)

---

## 11. 附录 A:本计划与 v0.3 规范的章节映射

| v0.3 规范章节 | 本计划 Phase | 本计划模块 |
|---------------|--------------|------------|
| §1–§2 | Phase 1 | wlwl-parser, wlwl-ast |
| §3–§5 | Phase 1 | wlwl-lexer, wlwl-parser |
| §6–§9 | Phase 1–2 | wlwl-eval |
| §10 | Phase 2 | wlwl-eval |
| §11 | Phase 3 | wlwl-eval(class) |
| §12(含 §12.6 ERR 传播) | Phase 2 | wlwl-eval, wlwl-error |
| §13 模块系统 | Phase 2(基础)–Phase 4(完整) | wlwl-module |
| §14 错误信息 | Phase 2(基础)–Phase 3(完整) | wlwl-error |
| §15 标准库 | Phase 1(io)–Phase 4(ai) | wlwl-std |
| §16 工具设计原则 | Phase 1–3 | wlwl-cli |
| §18 未决问题 | (本计划不解决,反馈给规范) | — |
| §19 形式化附录 | Phase 5(可选) | wlwl-formal |

## 12. 附录 B:关键技术决策记录(ADR 风格)

> **ADR-001:实现语言选择 Rust**
> - **状态**:建议,待 Li 拍板
> - **背景**:WLWL v0.3 规范要求"统一函数调用""结构化错误""面向 AI"。Rust + miette 生态对结构化错误最友好。
> - **决策**:Rust 作为主实现语言。
> - **后果**:实施需 Rust 熟练度;但生态成熟,长期可维护。
> - **替代**:Zig(生态薄)/ Go(错误处理弱)/ OCaml(Windows 差)。

> **ADR-002:执行模型选 tree-walking**
> - **状态**:建议,待 Li 拍板
> - **背景**:v0.3 不规定实现模型(§17 已删)。需在"实现简单"和"性能"间权衡。
> - **决策**:Phase 1-3 用 tree-walking(递归 → 显式栈),Phase 4 评估是否需要字节码。
> - **后果**:前期迭代快、报错精准;性能可能不达工业级,Phase 4 决定是否优化。

> **ADR-003:错误处理 crate 选 thiserror + miette**
> - **状态**:建议,待 Li 拍板
> - **背景**:v0.3 §14 要求结构化错误。Rust 生态有 thiserror(库)/ anyhow(应用)/ miette(用户诊断) 三种风格。
> - **决策**:内部用 thiserror(穷举错误),用户层用 miette(漂亮的诊断输出)。
> - **后果**:错误信息可以做到 rustc 级别;需要团队熟悉 miette 的 Diagnostic trait。

> **ADR-004:解析器手写递归下降**
> - **状态**:建议,待 Li 拍板
> - **背景**:解析器风格有手写 / 工具生成(pest/lalrpop/nom)。
> - **决策**:手写递归下降,便于错误信息精确控制。
> - **后果**:实现量稍大,但错误位置、恢复策略都可控。Phase 5 形式化也更容易对应。

> **ADR-005:许可证选择 GPL v2**
> - **状态**:已锁定(2026-09-02)
> - **背景**:Li 决定公开 WLWL 编译器,需要选择开源许可证。
> - **决策**:**GPL v2** 强 copyleft。
> - **后果**:
>   - 编译器、std 库、工具链:**全部 GPL v2**,衍生作品必须同样开源
>   - **不**传染给用户用 WLWL 写的程序(.wl 文件不受约束,作者自选许可证)
>   - std.ai 通过 HTTP 调用 OpenAI/Anthropic 等闭源 API,**不**算"链接",所以 GPL v2 不会传染给这些 API
>   - 与 GCC、Linux 内核一致
> - **风险**:商业用户可能避用;但符合"面向 AI Coding 公开生态"的定位
> - **替代**:Apache 2.0(更宽松,但失去 copyleft 保护)/ MIT(同前)

> **ADR-006:CI 平台 GitHub Actions + 三平台测试**
> - **状态**:已锁定(2026-09-02)
> - **背景**:Li 决定用 GitHub Actions。
> - **决策**:**GitHub Actions** + Linux / macOS / Windows 三平台 matrix。
> - **后果**:
>   - 仓库托管在 GitHub(隐含)
>   - Windows 兼容性测试是必须项(本地开发环境)
>   - Rust 工具链在 GitHub Actions 上有官方模板

> **ADR-007:目标用户公开 + Phase 4 末启动公开筹备**
> - **状态**:已锁定(2026-09-02)
> - **背景**:Li 决定公开 WLWL。
> - **决策**:**目标用户为公开**,Phase 4 末启动公开筹备(README、文档站、示例程序、社区渠道)。
> - **后果**:
>   - Phase 1-3 重点是技术实现,文档是技术向(internal doc)
>   - Phase 4 末必须交付:README、用户手册、示例项目(>= 5 个 .wl 程序)
>   - 文档站可用 GitHub Pages + mdBook,或类似方案

---

## 13. 附录 C:时间估算细化

### Phase 1 任务分解(4 周)

| 任务 | 周期 | 备注 |
|------|------|------|
| 仓库脚手架 + CI | 1 天 | GitHub Actions,三平台 |
| 词法分析器 | 2 天 | ~500 行 Rust |
| 语法分析器(基础) | 3 天 | LET / Call / Block |
| AST 定义 + 单元测试 | 2 天 | insta 快照 |
| 求值器(基础) | 3 天 | 递归求值 |
| 内置 PRINT | 1 天 | |
| `wlwl run` CLI | 1 天 | clap |
| 集成测试 + 文档 | 3 天 | 5 个集成测试 |
| **Phase 1 缓冲** | **3 天** | |

### Phase 2 任务分解(6 周)

| 任务 | 周期 | 备注 |
|------|------|------|
| 完整控制流(IF/WHILE/FOR/RETURN/BREAK/CONTINUE) | 1 周 | |
| 函数 + 闭包(Rc<RefCell<Env>>) | 1 周 | |
| 错误处理(OK/ERR/PANIC/TRY) | 1 周 | **重点** |
| **§12.6 ERR 透明传播** | 1 周 | **最关键,20+ 测试** |
| 模块系统基础(EXPORT/IMPORT) | 1 周 | 重复导入报错 |
| 循环导入检测 | 2 天 | 拓扑排序 |
| 错误码 E0001-E0102 | 1 周 | insta 快照 |
| **Phase 2 缓冲** | **3 天** | |

### Phase 3 任务分解(4 周)

| 任务 | 周期 | 备注 |
|------|------|------|
| 类型注解语法槽(§2.4) | 2 天 | 解析,不检查 |
| errorCategory 字段 | 2 天 | 13 个分类映射 |
| retryable 字段 | 1 天 | 错误码 → retryable 表 |
| error_schema_version 字段 | 1 天 | 字符串常量 |
| JSONL 流式输出 | 1 周 | serde + 逐行输出 |
| 错误码扩展 23→33 | 1 周 | IO/JSON/std.ai/网络 |
| 单错误恢复(§14.8) | 3 天 | parser 改造 |
| AI 契约测试 | 1 周 | 模拟 AI 消费 |
| **Phase 3 缓冲** | **3 天** | |

### Phase 4 任务分解(6 周)

| 任务 | 周期 | 备注 |
|------|------|------|
| std.io 完整 | 2 天 | INPUT + 错误处理 |
| std.json 完整 | 1 周 | PARSE / STRINGIFY + E0070/E0071 |
| std.ai HTTP 集成 | 2 周 | reqwest + 错误码 |
| 跨目录 IMPORT | 1 周 | 路径解析 + 项目根 |
| 命名空间路径 | 1 周 | wlwl: + 第三方 |
| wlwl.toml 解析 | 1 周 | toml + serde |
| wlwl.lock 生成 | 1 周 | 简化算法 |
| 性能优化 | 1 周 | 尾调用 + 热点内联 |
| **Phase 4 缓冲** | **3 天** | |

### Phase 5 任务分解(4 周,可选)

| 任务 | 周期 | 备注 |
|------|------|------|
| Coq 项目搭建 | 1 周 | 基础设置 |
| 核心表达式求值机械化 | 1 周 | LET/IF/Call |
| ERR 传播白名单证明 | 1 周 | **关键定理** |
| 实现等价性测试 | 1 周 | 对照 Rust 行为 |

---

**文档结束。生成时间:2026-09-02。**


---

## 实施进度跟踪(2026-09-03 起)

> 实际推进时按"3 批"切分 Phase 4(见 Phase 4 §任务分解)。每批结束 git add + commit + push,工作树保持可回滚状态。

### Phase 4 批 1 — std.io / std.fs / std.json + `wlwl:` namespace 路径 — **完成 ✅ 2026-09-03**

| 项 | 状态 |
|---|---|
| `wlwl:std.io` (PRINT, INPUT) | ✅ |
| `wlwl:std.fs` (READ_FILE, WRITE_FILE, EXISTS) | ✅ |
| `wlwl:std.json` (PARSE, STRINGIFY) | ✅ |
| 触发 E0060 / E0061 / E0062 (IO) | ✅ |
| 触发 E0070 (JSON parse), E0071 defensive | ✅ |
| 解析器接受 `wlwl:` 前缀 | ✅ |
| ModuleLoader 支持 namespace path → std crate 注入 | ✅ |
| Value::NativeFn { invoke: NativeInvoke::Std(StdFn) } 接入 | ✅ |
| 全 workspace 测试 | **169 / 169 通过** |
| 新 crate | `wlwl-std`(纯 Rust,不依赖 `wlwl-eval` 避免 cycle) |
| 偏离记录 | docs/plan/deviations.md — P4-001 / P4-002;修复 P3-012 / P3-013 |

### Phase 4 批 2 — 跨目录 + 命名空间 + wlwl.toml + lock — **完成 ✅ 2026-09-03**

**已完成 (199 / 199 tests pass):**
- 新 crate `wlwl-toml`(manifest 解析 + lockfile JSON + 内置 SHA-256)
- `ModuleLoader` 改造: 4 种 path 形式(std / namespace / relative / bare)+ 项目根边界
- parser 接受任何非空 path(批 1 的 E0043 限制移除)
- E0041 增强: 错误信息列出完整环路路径(spec §13.7)
- 触发 E0040(越界/未找到)+ E0043(未注册 namespace)
- wlwl-cli 的 lock 集成 推迟到批 3
### Phase 4 批 3 — std.ai (mock) + Phase 3 收尾 — **待开始**


**附:本计划不替代 v0.3 规范。规范的权威性高于本计划——任何"实施偏离"必须显式记录,不能默默修改规范。**
