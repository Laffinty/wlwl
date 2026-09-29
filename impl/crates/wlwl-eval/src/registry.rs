//! Spec v0.4 **附录 G 全局内建注册表** lock-down (Phase B11).
//!
//! 该模块是单源真相 (single source of truth):所有 spec v0.4 附录 G 列出的
//! 内建函数,**名称 / 签名 / ERR 消费者 / 宏函数 / 引入 / 分组** 都在
//! `BUILTIN_REGISTRY` 里钉死。任何对 `resolve_builtin` 表、`ERR_CONSUMER_REGISTRY`、
//! lexer macro lowering 的改动,必须在 BUILTIN_REGISTRY 留下对应条目,否则
//! 锁测试会在 `cargo test` 阶段挂掉。
//!
//! ## 4 个 dispatch 状态
//!
//! | `DispatchStatus`     | 含义                                                          |
//! |----------------------|---------------------------------------------------------------|
//! | `ResolvedBuiltin`    | `resolve_builtin(name)` 返回 `Some(_)`, 走 eval 通用 call 路径 |
//! | `ResolvedCompat`     | `resolve_builtin(name)` 返回 `Some(_compat)`,会先发 W0051 / W0054 再委托 canonical fn |
//! | `LexerMacro`         | 词法层关键字 → parser 降为 `Expr::*`,**不** 走 resolve_builtin  |
//! | `Deferred`           | spec 列了但 impl 还没接 (Phase B12+)                          |
//!
//! ## 锁测试 (在 lib.rs 的 tests 模块)
//!
//! 1. `b11_registry_covers_resolve_builtin` — 每个 `resolve_builtin` 的
//!    名字都必须在注册表里出现 (ResolvedBuiltin 或 ResolvedCompat)
//! 2. `b11_resolve_builtin_covers_registry` — 反向:每个
//!    `ResolvedBuiltin`/`ResolvedCompat` 都必须 `resolve_builtin(name).is_some()`
//! 3. `b11_err_consumer_registry_consistent` — `ERR_CONSUMER_REGISTRY`
//!    与本注册表的 `err_consumer = Yes` 条目一致 (LexerMacro 例外:见注释)
//! 4. `b11_macro_fn_attribute_matches_dispatch` — `macro_fn = true` 必须
//!    对应 `LexerMacro` 或被 `ResolvedBuiltin` 显式接受 (只有 `NOT` /
//!    `UNWRAP_OR` / `OR_DIE` 这三个特例 —— spec 标 macro,但实现走 builtin)
//! 5. `b11_registry_count_matches_spec_table` — 总条目数 ≥ 88 (spec 附录 G)
//!
//! ## [v0.10.1 / R10-064] 签名分叉:注册表也曾经是漂移的一方
//!
//! 6. `spec_appendix_g_sync.rs`(集成测试,不在本文件)—— spec 附录 G 与
//!    `BUILTIN_REGISTRY` 的**签名逐条字节相等**。
//!
//! 上一条只查**条目数**,不查**签名**,所以两份副本能悄悄分叉。实测
//! 110 条里有 **37 条**写法不同。乍看像是 spec 漂移,逐条对着
//! `wlwl-eval` 的 dispatch 实现核实之后的结论**正好相反**:注册表在
//! **11 处**说的与实现相反 —— `SUB` 第三参是 `len` 不是 `end`
//! (v0.8.1 D8-010 推翻了 v0.8.0 的 end-index 语义)、`INT` / `FLOAT`
//! 成功返回**裸** INTEGER / FLOAT 而不是 `OK(...)` 包一层、
//! `INDEX_SET` 返回新容器而不是 `NULL`、`WHILE` 恒返回 `NULL`、
//! `AT` 强制 3 参、`IMPORT` 只接 2 参、`THIS` 必须是 `THIS()`、
//! `==` / `!=` 的签名列各掉了一个字符,以及 `ARRAY(items...)` /
//! `DICT(pairs...)` 这两个**根本不存在**的带参形式。
//!
//! 所以本轮是**先修注册表,再让 spec 跟上**,不是反过来。方向永远以
//! 实现为准:注册表驱动运行期行为,但它自己也会写错。
//!
//! ## 附录 G markdown 生成
//!
//! `generate_appendix_g_md()` 把注册表渲染成 markdown。Phase B11 把它写
//! 到 `docs/appendix_G.md`(手工调一次);之后每次动注册表只需重跑这个
//! 函数即可。锁测试 `b11_generated_md_matches_registry` 守住生成器 +
//! 注册表的一致性。

// ──────────────────────────────────────────────────────────────────────
// Types
// ──────────────────────────────────────────────────────────────────────

/// 结构化签名的一段(Step 5 · A6′)。
///
/// **只表示顶层形状,不带类型参数。** 容器一律是无参的
/// [`SigTy::Array`] / [`SigTy::Dict`] —— 元素类型是 `Dynamic`。
/// 这是刻意的:嵌套泛型与数组元素匹配在运行时 `E0033` 那条路径上就是
/// *deliberately deferred*(`wlwl-eval/src/lib.rs:8588`),静态层首批
/// 不该比运行时更激进。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SigTy {
    Integer,
    Float,
    String,
    Boolean,
    Null,
    /// 无参容器(元素类型未建模)
    Array,
    /// 无参容器(键值类型未建模)
    Dict,
    /// `OK` / `ERR` 变体;两侧载荷类型未建模
    Result,
    /// 闭包
    Function,
    /// 推不出来 —— 静态层落 `Ty::Dynamic`,不产生任何诊断。
    ///
    /// 典型来源:多态返回(`+` 可得 INTEGER / FLOAT / STRING / ARRAY,
    /// 见 spec 附录 G)、取值透传(`AT_K` / `GET_PROP` / `CALL_METHOD`
    /// 的 `-> v`)、有副作用因而类型随分支变化(`SCOPE` / `SHIELD`)。
    Dynamic,
}

/// 结构化签名(Step 5 · A6′ 首批 = **只给返回类型**)。
///
/// # 为什么首批不带形参类型
///
/// spec 附录 G 大量条目有**可选形参**(`SUB(s, start, end?)`、
/// `POP(d, k, default)`、`SLICE(arr, start, end?)`),也有 `args...`
/// 变长(`PRINT(args...)`)。任何「精确元数」表示都会在这些条目上误报,
/// 与 ADR-0020 的「不误报优先」直接冲突。故 `params` 显式为
/// `Option<&'static [SigTy]>`:
///
/// - `None` = **元数未知 / 不检查**。首批全部条目都是 `None`。
/// - `Some(slice)` = 形参类型已知,可做元数与类型检查。留给后续批次
///   ——前提是逐条对着 `wlwl-eval` 的 dispatch 实现核过形参类型。
impl BuiltinSig {
    /// 首批用的构造:只有返回类型,形参**不检查**(可选形参会让精确元数
    /// 必然误报,见 `BuiltinSig::params` 的文档)。
    pub const fn ret_only(ret: SigTy) -> BuiltinSig {
        BuiltinSig { ret, params: None }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuiltinSig {
    /// 返回类型。`SigTy::Dynamic` = 推不出来,静态层不据此产生诊断。
    pub ret: SigTy,
    /// 形参类型;`None` = 元数未知,不做任何形参检查(首批全部条目)。
    pub params: Option<&'static [SigTy]>,
}

/// One row of spec 附录 G, mirrored as a Rust const.
#[derive(Debug, Clone, Copy)]
pub struct BuiltinSpec {
    /// Spec 写的名字 (大写 / 操作符字面)
    pub name: &'static str,
    /// Spec 的签名(`fn(args) → ret` 风格)
    pub signature: &'static str,
    /// 结构化签名(Step 5 · A6′ 首批)。
    ///
    /// **`None` 是常态而非缺口**:首批只覆盖 60 条高频内建的返回类型,
    /// 其余 ~50 条(Subscript / Module / Oop / Property / Concurrent /
    /// Result 宏 / Control 流程)刻意留 `None`,静态层对它们落
    /// `Dynamic`、**不产生诊断**。`signature` 文档串**始终保留**,
    /// 附录 G 生成器继续用它 —— 结构化字段是**加法**,不替换文档面。
    pub sig: Option<BuiltinSig>,
    /// 功能分组 (用于 markdown 表头)
    pub group: BuiltinGroup,
    /// §12.7 ERR 消费状态
    pub err_consumer: ErrConsumerStatus,
    /// §3.4 是否宏函数 (词法层展开)
    pub macro_fn: bool,
    /// 引入版本
    pub version: Version,
    /// impl 现状 (决定锁测试是否把它算进 dispatch 覆盖)
    pub dispatch: DispatchStatus,
    /// Spec 章节锚
    pub section: &'static str,
}

/// Spec 附录 G 按行分的功能分组。
///
/// 排序与 spec 表格行序一致 (`Io → Conv → Result → Control → Op →
/// Array → Dict → Subscript → String → Format → Module → Oop →
/// Property → Ctor`),方便 markdown 生成时直接 `group as u8` 排序。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BuiltinGroup {
    Io = 0,
    Conv = 1,
    Result = 2,
    Control = 3,
    Op = 4,
    Array = 5,
    Dict = 6,
    Subscript = 7,
    String = 8,
    Format = 9,
    Module = 10,
    Oop = 11,
    Property = 12,
    Ctor = 13,
    /// [v0.7] structured concurrency (SCOPE / SPAWN / AWAIT / YIELD /
    /// TASK_* / SHIELD / CHANNEL_*).
    Concurrent = 14,
}

impl BuiltinGroup {
    /// Spec 表格行号 / 章节锚,markdown 用作 § 链接。
    ///
    /// 注(v0.8):该函数当前在 impl/ 工作区内零调用方(无 grep 结果);
    /// md 表格"实现位置"列实际使用每个 `BuiltinSpec.section` 字段(见
    /// `generate_appendix_g_md()` 的 match spec.dispatch)。`anchor()` 暂
    /// 保留为对外表面以避免破坏外部 crate 引用;v0.8 §2.3 实际工作转向
    /// 更新 31 个 `BuiltinSpec.section` 字段,详见
    /// `docs/history/wlwl-build-plan-v0.8-COMPLETED.md` §2.3 (deviation D8-002)。
    pub fn anchor(self) -> &'static str {
        match self {
            BuiltinGroup::Io => "§15.1",
            BuiltinGroup::Conv => "§10 / §9.5 / §2.5 / §8.3",
            BuiltinGroup::Result => "§12",
            BuiltinGroup::Control => "§7 / §3.4",
            BuiltinGroup::Op => "§9",
            BuiltinGroup::Array => "§10.1",
            BuiltinGroup::Dict => "§10.2",
            BuiltinGroup::Subscript => "§10.1-§10.2",
            BuiltinGroup::String => "§10.3",
            BuiltinGroup::Format => "§10.6",
            BuiltinGroup::Module => "§13",
            BuiltinGroup::Oop => "§11",
            BuiltinGroup::Property => "§11.4",
            BuiltinGroup::Ctor => "§10.1 / §10.2",
            BuiltinGroup::Concurrent => "§17",
        }
    }

    /// Markdown 表 / `display` 用的中文标签。
    pub fn label(self) -> &'static str {
        match self {
            BuiltinGroup::Io => "I/O",
            BuiltinGroup::Conv => "类型 / 转换",
            BuiltinGroup::Result => "RESULT 处理",
            BuiltinGroup::Control => "控制流 / 逻辑",
            BuiltinGroup::Op => "运算符",
            BuiltinGroup::Array => "ARRAY 操作",
            BuiltinGroup::Dict => "DICT 操作",
            BuiltinGroup::Subscript => "下标",
            BuiltinGroup::String => "STRING 操作",
            BuiltinGroup::Format => "格式化",
            BuiltinGroup::Module => "模块系统",
            BuiltinGroup::Oop => "OOP",
            BuiltinGroup::Property => "属性 / 方法",
            BuiltinGroup::Ctor => "构造器",
            BuiltinGroup::Concurrent => "并发 / 通道",
        }
    }
}

/// Spec §12.7 ERR 消费三态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrConsumerStatus {
    /// ✔ 消费 ERR (IS_OK / IS_ERR / OR_DIE / UNWRAP_OR / UNWRAP /
    /// ERR_PAYLOAD / WRAP / TYPE / TRY / EXPECT_ERR) — 不透明传播
    Yes,
    /// ❌ 不消费 ERR,透明传播 (§12.6 默认)
    No,
    /// n/a 控制流 / 模块 / IO,语义上"无所谓"
    Na,
}

impl ErrConsumerStatus {
    /// Markdown ✔ / ❌ / n/a 三态。
    pub fn glyph(self) -> &'static str {
        match self {
            ErrConsumerStatus::Yes => "✔",
            ErrConsumerStatus::No => "❌",
            ErrConsumerStatus::Na => "n/a",
        }
    }
}

/// Spec 引入版本。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Version {
    V02,
    V04,
    /// v0.6 (current dev cycle): short-circuit `&&`/`||`, explicit
    /// `LET MUT` mutability, truthiness overhaul, string interpolation.
    V06,
    /// v0.7 (current): structured concurrency + channels (additive).
    V07,
}

impl Version {
    pub fn as_str(self) -> &'static str {
        match self {
            Version::V02 => "v0.2",
            Version::V04 => "v0.4",
            Version::V06 => "v0.6",
            Version::V07 => "v0.7",
        }
    }
}

/// Impl 现状。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispatchStatus {
    /// `resolve_builtin(name)` 返回 `Some(clean_fn)`,无 warning
    ResolvedBuiltin,
    /// `resolve_builtin(name)` 返回 `Some(_compat_fn)`,会先 emit
    /// `W0051` (deprecated_alias) 或 `W0054` (deprecated_op_form)
    ResolvedCompat,
    /// 词法层关键字 → parser 降为 `Expr::*`,不经过 `resolve_builtin`
    LexerMacro,
    /// spec 列了,impl 尚未接 (Phase B12+)
    Deferred,
}

impl DispatchStatus {
    pub fn label(self) -> &'static str {
        match self {
            DispatchStatus::ResolvedBuiltin => "✓ builtin",
            DispatchStatus::ResolvedCompat => "✓ compat (W0051/W0054)",
            DispatchStatus::LexerMacro => "✓ macro",
            DispatchStatus::Deferred => "⏳ deferred",
        }
    }
}
// ──────────────────────────────────────────────────────────────────────
// Registry table
// ──────────────────────────────────────────────────────────────────────

/// Spec 附录 G 全表 (B11 单源真相)。
///
/// 排序:按 `group` 升序,然后按 spec 表格行内顺序。同一 group 内
/// 不强行字母序,以保留 spec 表格的语义次序。
///
/// 行数统计:v0.6 = **93** unique entries;v0.7 additively appends the
/// **17** concurrent builtins → **110** total (lock in
/// `b11_registry_count_matches_spec_table`).
pub const BUILTIN_REGISTRY: &[BuiltinSpec] = &[
    // ── I/O (3) ──────────────────────────────────────────────────
    BuiltinSpec {
        name: "PRINT",
        signature: "PRINT(args...) -> NULL",
        sig: Some(BuiltinSig::ret_only(SigTy::Null)),
        group: BuiltinGroup::Io,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V02,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§10.2",
    },
    BuiltinSpec {
        name: "PRINT_ERR",
        signature: "PRINT_ERR(args...) -> NULL",
        sig: Some(BuiltinSig::ret_only(SigTy::Null)),
        group: BuiltinGroup::Io,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V04,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§10.2",
    },
    BuiltinSpec {
        name: "INPUT",
        signature: "INPUT(prompt?) -> STRING",
        sig: Some(BuiltinSig::ret_only(SigTy::String)),
        group: BuiltinGroup::Io,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V02,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§10.2",
    },
    // ── 类型 / 转换 (7) ─────────────────────────────────────────
    BuiltinSpec {
        name: "LEN",
        signature: "LEN(coll) -> INTEGER",
        sig: Some(BuiltinSig::ret_only(SigTy::Integer)),
        group: BuiltinGroup::Conv,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V02,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§10.3",
    },
    BuiltinSpec {
        name: "STR",
        signature: "STR(x) -> STRING",
        sig: Some(BuiltinSig::ret_only(SigTy::String)),
        group: BuiltinGroup::Conv,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V02,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§10.3",
    },
    BuiltinSpec {
        name: "INT",
        // [v0.10.1 / R10-064] 签名列描述**运行期**契约:成功返回裸
        // INTEGER,失败返回 ERR(ParseError) —— 外面没有 OK(...) 包一层。
        // 实测 `PRINT(INT("42"))` -> `42`。
        //
        // 下面的 `sig.ret` 仍是 `Result`,这是**刻意的静态近似**,不是
        // 笔误:成功路径的类型就是 INTEGER,再套一层 RESULT 反而是
        // 错的。之所以不改,是 `sig.ret` 经 `check.rs:684`
        // (`check_call_with_sig`) 参与静态层判定,改它是一次行为变更,
        // 不该混在文档同步里。实测当前静态层不拿内建返回类型去卡
        // `LET` 注解(`LET(i: STRING, INT("42"))` 零诊断),所以这个
        // 偏差今天不产生任何错误诊断,只影响 LSP hover 的显示文本。
        signature: "INT(x) -> INTEGER / ERR(ParseError)",
        sig: Some(BuiltinSig::ret_only(SigTy::Result)),
        group: BuiltinGroup::Conv,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V02,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§10.3",
    },
    BuiltinSpec {
        name: "FLOAT",
        signature: "FLOAT(x) -> FLOAT / ERR(ParseError)",
        sig: Some(BuiltinSig::ret_only(SigTy::Result)),
        group: BuiltinGroup::Conv,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V04,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§10.3",
    },
    BuiltinSpec {
        name: "TYPE",
        signature: "TYPE(x) -> STRING (RESULT -> \"RESULT\")",
        sig: None,
        group: BuiltinGroup::Conv,
        err_consumer: ErrConsumerStatus::Yes,
        macro_fn: true,
        version: Version::V02,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§2.5",
    },
    BuiltinSpec {
        name: "BOOL",
        signature: "BOOL(x) -> BOOLEAN",
        sig: Some(BuiltinSig::ret_only(SigTy::Boolean)),
        group: BuiltinGroup::Conv,
        // v0.6 §8.3 + Appendix B.17: BOOL is registered as an ERR
        // consumer so `BOOL(ERR(...))` returns a BOOLEAN instead of
        // triggering §12.6 transparent propagation.
        err_consumer: ErrConsumerStatus::Yes,
        macro_fn: false,
        version: Version::V02,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§2.2",
    },
    BuiltinSpec {
        name: "CALL",
        signature: "CALL(fn, args...) -> v",
        sig: Some(BuiltinSig::ret_only(SigTy::Dynamic)),
        group: BuiltinGroup::Conv,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V02,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§8.3",
    },
    // ── RESULT 处理 (11) ────────────────────────────────────────
    BuiltinSpec {
        name: "IS_OK",
        signature: "IS_OK(x) -> BOOLEAN",
        sig: None,
        group: BuiltinGroup::Result,
        err_consumer: ErrConsumerStatus::Yes,
        macro_fn: true,
        version: Version::V02,
        dispatch: DispatchStatus::LexerMacro,
        section: "§8.3",
    },
    BuiltinSpec {
        name: "IS_ERR",
        signature: "IS_ERR(x) -> BOOLEAN",
        sig: None,
        group: BuiltinGroup::Result,
        err_consumer: ErrConsumerStatus::Yes,
        macro_fn: true,
        version: Version::V02,
        dispatch: DispatchStatus::LexerMacro,
        section: "§8.3",
    },
    BuiltinSpec {
        name: "OR_DIE",
        signature: "OR_DIE(x, default) -> v (v0.3 alias)",
        sig: None,
        group: BuiltinGroup::Result,
        err_consumer: ErrConsumerStatus::Yes,
        macro_fn: true,
        version: Version::V02,
        dispatch: DispatchStatus::ResolvedCompat,
        section: "§8.3",
    },
    BuiltinSpec {
        name: "UNWRAP_OR",
        signature: "UNWRAP_OR(x, default) -> v",
        sig: None,
        group: BuiltinGroup::Result,
        err_consumer: ErrConsumerStatus::Yes,
        macro_fn: true,
        version: Version::V04,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§8.3",
    },
    BuiltinSpec {
        name: "UNWRAP",
        signature: "UNWRAP(x) -> v / E0100 (不可捕获)",
        sig: None,
        group: BuiltinGroup::Result,
        err_consumer: ErrConsumerStatus::Yes,
        macro_fn: false,
        version: Version::V04,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§8.3",
    },
    BuiltinSpec {
        name: "ERR_PAYLOAD",
        signature: "ERR_PAYLOAD(x) -> e / E0030",
        sig: None,
        group: BuiltinGroup::Result,
        err_consumer: ErrConsumerStatus::Yes,
        macro_fn: false,
        version: Version::V04,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§8.3",
    },
    BuiltinSpec {
        name: "WRAP",
        signature: "WRAP(err, ctx) -> ERR / OK",
        sig: None,
        group: BuiltinGroup::Result,
        err_consumer: ErrConsumerStatus::Yes,
        macro_fn: false,
        version: Version::V04,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§8.3",
    },
    BuiltinSpec {
        name: "TRY",
        signature: "TRY(e) -> v / early-RETURN",
        sig: None,
        group: BuiltinGroup::Result,
        err_consumer: ErrConsumerStatus::Yes,
        macro_fn: true,
        version: Version::V02,
        dispatch: DispatchStatus::LexerMacro,
        section: "§6",
    },
    BuiltinSpec {
        name: "PANIC",
        signature: "PANIC(msg) -> 终止",
        sig: None,
        group: BuiltinGroup::Result,
        err_consumer: ErrConsumerStatus::Na,
        macro_fn: true,
        version: Version::V02,
        dispatch: DispatchStatus::LexerMacro,
        section: "§8.4",
    },
    BuiltinSpec {
        name: "OK",
        signature: "OK(v) -> RESULT",
        sig: None,
        group: BuiltinGroup::Result,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: true,
        version: Version::V04,
        dispatch: DispatchStatus::LexerMacro,
        section: "§8.1",
    },
    BuiltinSpec {
        name: "ERR",
        signature: "ERR(e) -> RESULT",
        sig: None,
        group: BuiltinGroup::Result,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: true,
        version: Version::V04,
        dispatch: DispatchStatus::LexerMacro,
        section: "§8.1",
    },
    // ── 控制流 / 逻辑 (10) ──────────────────────────────────────
    BuiltinSpec {
        name: "IF",
        signature: "IF(cond, t, e?) -> v",
        sig: None,
        group: BuiltinGroup::Control,
        // v0.6 §6.1 + §8.3: IF is a partial ERR consumer at the
        // condition position; ERR → goes to else branch (or
        // propagates if no else). Handled in `eval_if`, not via
        // the generic §12.6 short-circuit block.
        err_consumer: ErrConsumerStatus::Yes,
        macro_fn: true,
        version: Version::V02,
        dispatch: DispatchStatus::LexerMacro,
        section: "§6",
    },
    BuiltinSpec {
        name: "WHILE",
        signature: "WHILE(cond, body) -> NULL",
        sig: None,
        group: BuiltinGroup::Control,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: true,
        version: Version::V02,
        dispatch: DispatchStatus::LexerMacro,
        section: "§6",
    },
    BuiltinSpec {
        name: "FOR",
        signature: "FOR(var, iter, body) -> NULL",
        sig: None,
        group: BuiltinGroup::Control,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: true,
        version: Version::V02,
        dispatch: DispatchStatus::LexerMacro,
        section: "§6",
    },
    BuiltinSpec {
        name: "MATCH",
        signature: "MATCH(v, clauses, default?) -> v",
        sig: None,
        group: BuiltinGroup::Control,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: true,
        version: Version::V04,
        dispatch: DispatchStatus::LexerMacro,
        section: "§6",
    },
    BuiltinSpec {
        name: "RETURN",
        signature: "RETURN(v?) -> 早返",
        sig: None,
        group: BuiltinGroup::Control,
        err_consumer: ErrConsumerStatus::Na,
        macro_fn: true,
        version: Version::V02,
        dispatch: DispatchStatus::LexerMacro,
        section: "§6",
    },
    BuiltinSpec {
        name: "BREAK",
        signature: "BREAK() -> 跳出",
        sig: None,
        group: BuiltinGroup::Control,
        err_consumer: ErrConsumerStatus::Na,
        macro_fn: true,
        version: Version::V02,
        dispatch: DispatchStatus::LexerMacro,
        section: "§6",
    },
    BuiltinSpec {
        name: "CONTINUE",
        signature: "CONTINUE() -> 跳到下轮",
        sig: None,
        group: BuiltinGroup::Control,
        err_consumer: ErrConsumerStatus::Na,
        macro_fn: true,
        version: Version::V02,
        dispatch: DispatchStatus::LexerMacro,
        section: "§6",
    },
    BuiltinSpec {
        name: "NOT",
        signature: "NOT(a) -> BOOLEAN (取反)",
        sig: Some(BuiltinSig::ret_only(SigTy::Boolean)),
        group: BuiltinGroup::Control,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: true,
        version: Version::V02,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§4.3",
    },
    // ── 运算符 (12) ─────────────────────────────────────────────
    BuiltinSpec {
        name: "==",
        signature: "==(a, b) -> BOOLEAN / ERR 透传",
        sig: Some(BuiltinSig::ret_only(SigTy::Boolean)),
        group: BuiltinGroup::Op,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V02,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§4.3",
    },
    BuiltinSpec {
        name: "!=",
        signature: "!=(a, b) -> BOOLEAN / ERR 透传",
        sig: Some(BuiltinSig::ret_only(SigTy::Boolean)),
        group: BuiltinGroup::Op,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V02,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§4.3",
    },
    BuiltinSpec {
        name: ">",
        signature: ">(a, b) -> BOOLEAN / ERR 透传",
        sig: Some(BuiltinSig::ret_only(SigTy::Boolean)),
        group: BuiltinGroup::Op,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V02,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§4.3",
    },
    BuiltinSpec {
        name: "<",
        signature: "<(a, b) -> BOOLEAN / ERR 透传",
        sig: Some(BuiltinSig::ret_only(SigTy::Boolean)),
        group: BuiltinGroup::Op,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V02,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§4.3",
    },
    BuiltinSpec {
        name: ">=",
        signature: ">=(a, b) -> BOOLEAN / ERR 透传",
        sig: Some(BuiltinSig::ret_only(SigTy::Boolean)),
        group: BuiltinGroup::Op,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V02,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§4.3",
    },
    BuiltinSpec {
        name: "<=",
        signature: "<=(a, b) -> BOOLEAN / ERR 透传",
        sig: Some(BuiltinSig::ret_only(SigTy::Boolean)),
        group: BuiltinGroup::Op,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V02,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§4.3",
    },
    BuiltinSpec {
        name: "+",
        signature: "+(a, b) -> INTEGER / FLOAT / STRING / ARRAY",
        sig: Some(BuiltinSig::ret_only(SigTy::Dynamic)),
        group: BuiltinGroup::Op,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V02,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§4.3",
    },
    BuiltinSpec {
        name: "-",
        signature: "-(a, b) -> INTEGER / FLOAT",
        sig: Some(BuiltinSig::ret_only(SigTy::Dynamic)),
        group: BuiltinGroup::Op,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V02,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§4.3",
    },
    BuiltinSpec {
        name: "*",
        signature: "*(a, b) -> INTEGER / FLOAT",
        sig: Some(BuiltinSig::ret_only(SigTy::Dynamic)),
        group: BuiltinGroup::Op,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V02,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§4.3",
    },
    BuiltinSpec {
        name: "/",
        signature: "/(a, b) -> INTEGER / FLOAT",
        sig: Some(BuiltinSig::ret_only(SigTy::Dynamic)),
        group: BuiltinGroup::Op,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V02,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§4.3",
    },
    BuiltinSpec {
        name: "%",
        signature: "%(a, b) -> INTEGER",
        sig: Some(BuiltinSig::ret_only(SigTy::Integer)),
        group: BuiltinGroup::Op,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V02,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§4.3",
    },
    BuiltinSpec {
        name: "&&",
        signature: "&&(a, b) -> BOOLEAN (v0.6 §4.3 short-circuit)",
        sig: Some(BuiltinSig::ret_only(SigTy::Boolean)),
        group: BuiltinGroup::Op,
        // v0.6 §8.3: short-circuit `&&` consumes ERR — left-side
        // ERR propagates without evaluating `b`. The actual logic
        // lives in `eval_logical_short_circuit` (intercepted in
        // `eval_call` before the generic §12.6 short-circuit block).
        err_consumer: ErrConsumerStatus::Yes,
        macro_fn: false,
        version: Version::V06,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§4.3",
    },
    BuiltinSpec {
        name: "||",
        signature: "||(a, b) -> BOOLEAN (v0.6 §4.3 short-circuit)",
        sig: Some(BuiltinSig::ret_only(SigTy::Boolean)),
        group: BuiltinGroup::Op,
        // v0.6 §8.3: short-circuit `||` consumes ERR — left-side
        // ERR propagates without evaluating `b`. The actual logic
        // lives in `eval_logical_short_circuit`.
        err_consumer: ErrConsumerStatus::Yes,
        macro_fn: false,
        version: Version::V06,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§4.3",
    },
    BuiltinSpec {
        name: "NEG",
        signature: "NEG(a) -> -a",
        sig: Some(BuiltinSig::ret_only(SigTy::Dynamic)),
        group: BuiltinGroup::Op,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V02,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§4.3",
    },
    // ── ARRAY 操作 (9) ──────────────────────────────────────────
    BuiltinSpec {
        name: "PUSH",
        signature: "PUSH(arr, x) -> ARRAY",
        sig: Some(BuiltinSig::ret_only(SigTy::Array)),
        group: BuiltinGroup::Array,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V02,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§10.4",
    },
    BuiltinSpec {
        name: "POP",
        signature: "POP(d, k, default) -> v (v0.6 compat alias for AT_K; signature kept 3-arg)",
        sig: Some(BuiltinSig::ret_only(SigTy::Dynamic)),
        group: BuiltinGroup::Dict,
        // v0.8 D8-001 deviation (was P4-B12-002): registry entry now
        // documents the actual 3-arg DICT semantics. Dispatch still routes
        // to `builtin_at_k` (`lib.rs:4092`) for source-compat with v0.5
        // programs that called POP(arr) on DICTs. `AT_K` is the canonical
        // name; `POP` survives as a compat alias emitting no warning
        // (unlike ResolvedCompat aliases, since the rename path predates
        // the W0051 machinery).
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V06,
        dispatch: DispatchStatus::ResolvedCompat,
        section: "§10.4",
    },
    BuiltinSpec {
        name: "AT_K",
        signature: "AT_K(d, k, default) -> v (v0.6 §10.4)",
        sig: Some(BuiltinSig::ret_only(SigTy::Dynamic)),
        group: BuiltinGroup::Dict,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V06,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§10.4",
    },
    BuiltinSpec {
        name: "SHIFT",
        signature: "SHIFT(arr) -> ARRAY",
        sig: Some(BuiltinSig::ret_only(SigTy::Array)),
        group: BuiltinGroup::Array,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V02,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§10.4",
    },
    BuiltinSpec {
        name: "UNSHIFT",
        signature: "UNSHIFT(arr, x) -> ARRAY",
        sig: Some(BuiltinSig::ret_only(SigTy::Array)),
        group: BuiltinGroup::Array,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V02,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§10.4",
    },
    BuiltinSpec {
        name: "SLICE",
        signature: "SLICE(arr, start, end?) -> ARRAY",
        sig: Some(BuiltinSig::ret_only(SigTy::Array)),
        group: BuiltinGroup::Array,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V02,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§10.4",
    },
    BuiltinSpec {
        name: "CONCAT",
        signature: "CONCAT(a, b) -> ARRAY",
        sig: Some(BuiltinSig::ret_only(SigTy::Array)),
        group: BuiltinGroup::Array,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V02,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§10.4",
    },
    BuiltinSpec {
        name: "CONTAINS",
        signature: "CONTAINS(arr, x) -> BOOLEAN",
        sig: Some(BuiltinSig::ret_only(SigTy::Boolean)),
        group: BuiltinGroup::Array,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V02,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§10.4",
    },
    BuiltinSpec {
        name: "INDEX",
        signature: "INDEX(arr, x) -> INTEGER (1-based) / -1",
        sig: Some(BuiltinSig::ret_only(SigTy::Integer)),
        group: BuiltinGroup::Array,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V02,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§10.4",
    },
    BuiltinSpec {
        name: "REVERSE",
        signature: "REVERSE(arr) -> ARRAY",
        sig: Some(BuiltinSig::ret_only(SigTy::Array)),
        group: BuiltinGroup::Array,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V02,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§10.4",
    },
    // ── DICT 操作 (5) ───────────────────────────────────────────
    BuiltinSpec {
        name: "REMOVE_KEY",
        signature: "REMOVE_KEY(dict, k) -> DICT",
        sig: Some(BuiltinSig::ret_only(SigTy::Dict)),
        group: BuiltinGroup::Dict,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V04,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§10.4",
    },
    BuiltinSpec {
        name: "DEL",
        signature: "DEL(dict, k) -> DICT (v0.3 alias, W0051)",
        sig: Some(BuiltinSig::ret_only(SigTy::Dict)),
        group: BuiltinGroup::Dict,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V02,
        dispatch: DispatchStatus::ResolvedCompat,
        section: "§10.4",
    },
    BuiltinSpec {
        name: "KEYS",
        signature: "KEYS(dict) -> ARRAY",
        sig: Some(BuiltinSig::ret_only(SigTy::Array)),
        group: BuiltinGroup::Dict,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V02,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§10.4",
    },
    BuiltinSpec {
        name: "VALUES",
        signature: "VALUES(dict) -> ARRAY",
        sig: Some(BuiltinSig::ret_only(SigTy::Array)),
        group: BuiltinGroup::Dict,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V02,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§10.4",
    },
    BuiltinSpec {
        name: "HAS",
        signature: "HAS(dict, k) -> BOOLEAN",
        sig: Some(BuiltinSig::ret_only(SigTy::Boolean)),
        group: BuiltinGroup::Dict,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V02,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§10.4",
    },
    BuiltinSpec {
        name: "MERGE",
        signature: "MERGE(a, b) -> DICT",
        sig: Some(BuiltinSig::ret_only(SigTy::Dict)),
        group: BuiltinGroup::Dict,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V02,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§10.4",
    },
    // ── 下标 (3) ───────────────────────────────────────────────
    BuiltinSpec {
        name: "INDEX_GET",
        signature: "INDEX_GET(coll, k) -> v / E0031, E0036, E0037",
        sig: None,
        group: BuiltinGroup::Subscript,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V04,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§4.5",
    },
    BuiltinSpec {
        name: "INDEX_SET",
        signature: "INDEX_SET(coll, k, v) -> ARRAY / DICT",
        sig: None,
        group: BuiltinGroup::Subscript,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V04,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§4.5",
    },
    BuiltinSpec {
        name: "AT",
        signature: "AT(coll, k, default) -> v",
        sig: None,
        group: BuiltinGroup::Subscript,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V04,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§4.5",
    },
    // ── STRING 操作 (15) ────────────────────────────────────────
    BuiltinSpec {
        name: "UPPER",
        signature: "UPPER(s) -> STRING",
        sig: Some(BuiltinSig::ret_only(SigTy::String)),
        group: BuiltinGroup::String,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V02,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§10.5",
    },
    BuiltinSpec {
        name: "LOWER",
        signature: "LOWER(s) -> STRING",
        sig: Some(BuiltinSig::ret_only(SigTy::String)),
        group: BuiltinGroup::String,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V02,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§10.5",
    },
    BuiltinSpec {
        name: "SUB",
        signature: "SUB(s, start, len?) -> STRING",
        sig: Some(BuiltinSig::ret_only(SigTy::String)),
        group: BuiltinGroup::String,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V02,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§10.5",
    },
    BuiltinSpec {
        name: "REPLACE",
        signature: "REPLACE(s, old, new) -> STRING",
        sig: Some(BuiltinSig::ret_only(SigTy::String)),
        group: BuiltinGroup::String,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V02,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§10.5",
    },
    BuiltinSpec {
        name: "SPLIT",
        signature: "SPLIT(s, sep) -> ARRAY",
        sig: Some(BuiltinSig::ret_only(SigTy::Array)),
        group: BuiltinGroup::String,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V02,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§10.5",
    },
    BuiltinSpec {
        name: "TRIM",
        signature: "TRIM(s) -> STRING",
        sig: Some(BuiltinSig::ret_only(SigTy::String)),
        group: BuiltinGroup::String,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V02,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§10.5",
    },
    BuiltinSpec {
        name: "TRIM_START",
        signature: "TRIM_START(s) -> STRING",
        sig: Some(BuiltinSig::ret_only(SigTy::String)),
        group: BuiltinGroup::String,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V02,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§10.5",
    },
    BuiltinSpec {
        name: "TRIM_END",
        signature: "TRIM_END(s) -> STRING",
        sig: Some(BuiltinSig::ret_only(SigTy::String)),
        group: BuiltinGroup::String,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V02,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§10.5",
    },
    BuiltinSpec {
        name: "STARTS_WITH",
        signature: "STARTS_WITH(s, pre) -> BOOLEAN",
        sig: Some(BuiltinSig::ret_only(SigTy::Boolean)),
        group: BuiltinGroup::String,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V02,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§10.5",
    },
    BuiltinSpec {
        name: "ENDS_WITH",
        signature: "ENDS_WITH(s, suf) -> BOOLEAN",
        sig: Some(BuiltinSig::ret_only(SigTy::Boolean)),
        group: BuiltinGroup::String,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V02,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§10.5",
    },
    BuiltinSpec {
        name: "REPEAT",
        signature: "REPEAT(s, n) -> STRING",
        sig: Some(BuiltinSig::ret_only(SigTy::String)),
        group: BuiltinGroup::String,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V02,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§10.5",
    },
    BuiltinSpec {
        name: "PAD_START",
        signature: "PAD_START(s, n, c?) -> STRING",
        sig: Some(BuiltinSig::ret_only(SigTy::String)),
        group: BuiltinGroup::String,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V02,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§10.5",
    },
    BuiltinSpec {
        name: "PAD_END",
        signature: "PAD_END(s, n, c?) -> STRING",
        sig: Some(BuiltinSig::ret_only(SigTy::String)),
        group: BuiltinGroup::String,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V02,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§10.5",
    },
    BuiltinSpec {
        name: "CODEPOINTS",
        signature: "CODEPOINTS(s) -> ARRAY",
        sig: Some(BuiltinSig::ret_only(SigTy::Array)),
        group: BuiltinGroup::String,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V02,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§10.5",
    },
    BuiltinSpec {
        name: "FROM_CODEPOINTS",
        signature: "FROM_CODEPOINTS(arr) -> STRING",
        sig: Some(BuiltinSig::ret_only(SigTy::String)),
        group: BuiltinGroup::String,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V02,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§10.5",
    },
    // ── 格式化 (1) ─────────────────────────────────────────────
    BuiltinSpec {
        name: "FORMAT",
        signature: "FORMAT(template, args...) -> STRING",
        sig: Some(BuiltinSig::ret_only(SigTy::String)),
        group: BuiltinGroup::Format,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V04,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§10.7",
    },
    // ── 模块系统 (4) ───────────────────────────────────────────
    BuiltinSpec {
        name: "MODULE_REF",
        // [v0.10.1 / R10-064] 此前这里写 `-> MODULE`,而 `MODULE` 根本不是
        // 一个类型构造子。R10-046 当初只改了 spec 正文与附录 G,**漏了注册表**,
        // 于是自动生成的 mirror 一直在跟着说这句假话。实测
        // `TYPE(MODULE_REF("wlwl:std.json"))` 返回 `DICT`。
        signature: "MODULE_REF(path) -> DICT",
        sig: None,
        group: BuiltinGroup::Module,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V04,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§9",
    },
    BuiltinSpec {
        name: "EXPORT",
        signature: "EXPORT(names) -> NULL",
        sig: None,
        group: BuiltinGroup::Module,
        err_consumer: ErrConsumerStatus::Na,
        macro_fn: true,
        version: Version::V02,
        dispatch: DispatchStatus::LexerMacro,
        section: "§9",
    },
    BuiltinSpec {
        name: "IMPORT",
        signature: "IMPORT(path, names) -> NULL",
        sig: None,
        group: BuiltinGroup::Module,
        err_consumer: ErrConsumerStatus::Na,
        macro_fn: true,
        version: Version::V02,
        dispatch: DispatchStatus::LexerMacro,
        section: "§9",
    },
    // ── OOP (3) ────────────────────────────────────────────────
    // [v0.9 Step 9a-6 / plan §4.3 / ADR-0019 §4.3] Section
    // anchors for the OOP keywords. Plan §4.3 "spec §13
    // OOP 关键字与对象模型" pins CLASS / NEW / THIS / GET_PROP
    // / SET_PROP / CALL_METHOD to §13 — §16, with THIS
    // primarily anchored in §15 ("THIS 与线性 capability")
    // and the property / method ops in §13. The v0.8.1
    // placeholder `§11 [占位;OOP 未实现]` is removed; the
    // section list below extends to include §13 / §14 / §15
    // / §16 so the snapshot test can recognise them.
    BuiltinSpec {
        name: "CLASS",
        signature: "CLASS(name, parent, members) -> CLASS (name 传 NULL = 匿名类)",
        sig: None,
        group: BuiltinGroup::Oop,
        err_consumer: ErrConsumerStatus::Na,
        macro_fn: true,
        version: Version::V02,
        dispatch: DispatchStatus::LexerMacro,
        section: "§13",
    },
    BuiltinSpec {
        name: "NEW",
        signature: "NEW(cls, args...) -> INSTANCE",
        sig: None,
        group: BuiltinGroup::Oop,
        err_consumer: ErrConsumerStatus::Na,
        macro_fn: true,
        version: Version::V02,
        dispatch: DispatchStatus::LexerMacro,
        section: "§13",
    },
    BuiltinSpec {
        name: "THIS",
        signature: "THIS() -> INSTANCE",
        sig: None,
        group: BuiltinGroup::Oop,
        err_consumer: ErrConsumerStatus::Na,
        macro_fn: true,
        version: Version::V02,
        dispatch: DispatchStatus::LexerMacro,
        section: "§15",
    },
    // ── 属性 / 方法 (3) ────────────────────────────────────────
    BuiltinSpec {
        name: "GET_PROP",
        signature: "GET_PROP(obj, k) -> v / E0037",
        sig: None,
        group: BuiltinGroup::Property,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V02,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§13",
    },
    BuiltinSpec {
        name: "SET_PROP",
        signature: "SET_PROP(obj, k, v) -> NULL",
        sig: None,
        group: BuiltinGroup::Property,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V02,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§13",
    },
    BuiltinSpec {
        name: "CALL_METHOD",
        signature: "CALL_METHOD(obj, m, args...) -> v",
        sig: None,
        group: BuiltinGroup::Property,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V02,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§13",
    },
    // ── 构造器 (2) ─────────────────────────────────────────────
    BuiltinSpec {
        name: "ARRAY",
        signature: "ARRAY() -> ARRAY",
        sig: Some(BuiltinSig::ret_only(SigTy::Array)),
        group: BuiltinGroup::Ctor,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: true,
        version: Version::V02,
        dispatch: DispatchStatus::LexerMacro,
        section: "§10.9",
    },
    BuiltinSpec {
        name: "DICT",
        signature: "DICT() -> DICT",
        sig: Some(BuiltinSig::ret_only(SigTy::Dict)),
        group: BuiltinGroup::Ctor,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: true,
        version: Version::V02,
        dispatch: DispatchStatus::LexerMacro,
        section: "§10.9",
    },
    // ── 并发 / 通道 (17) [v0.7 additive] ─────────────────────────
    // None of these are §8.3 ERR consumers: an ERR argument
    // transparently propagates (§8.2) and the builtin body does not
    // run. See spec v0.7 §17.
    BuiltinSpec {
        name: "SCOPE",
        signature: "SCOPE(fn) -> v",
        sig: None,
        group: BuiltinGroup::Concurrent,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V07,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§17.1",
    },
    BuiltinSpec {
        name: "SPAWN",
        signature: "SPAWN(fn) -> TASK",
        sig: None,
        group: BuiltinGroup::Concurrent,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V07,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§17.1",
    },
    BuiltinSpec {
        name: "AWAIT",
        signature: "AWAIT(task) -> v",
        sig: None,
        group: BuiltinGroup::Concurrent,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V07,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§17.1",
    },
    BuiltinSpec {
        name: "YIELD",
        signature: "YIELD() -> NULL",
        sig: None,
        group: BuiltinGroup::Concurrent,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V07,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§17.1",
    },
    BuiltinSpec {
        name: "TASK_CURRENT",
        signature: "TASK_CURRENT() -> TASK",
        sig: None,
        group: BuiltinGroup::Concurrent,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V07,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§17.3",
    },
    BuiltinSpec {
        name: "TASK_IS_CANCELLED",
        signature: "TASK_IS_CANCELLED() -> BOOLEAN",
        sig: None,
        group: BuiltinGroup::Concurrent,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V07,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§17.3",
    },
    BuiltinSpec {
        name: "TASK_CANCEL",
        signature: "TASK_CANCEL(task, reason?) -> NULL",
        sig: None,
        group: BuiltinGroup::Concurrent,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V07,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§17.3",
    },
    BuiltinSpec {
        name: "TASK_CANCEL_PARENT",
        signature: "TASK_CANCEL_PARENT(reason?) -> NULL",
        sig: None,
        group: BuiltinGroup::Concurrent,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V07,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§17.3",
    },
    BuiltinSpec {
        name: "SHIELD",
        signature: "SHIELD(fn) -> v",
        sig: None,
        group: BuiltinGroup::Concurrent,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V07,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§17.3",
    },
    BuiltinSpec {
        name: "CHANNEL_NEW",
        signature: "CHANNEL_NEW(buf) -> CHANNEL",
        sig: None,
        group: BuiltinGroup::Concurrent,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V07,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§17.2",
    },
    BuiltinSpec {
        name: "CHANNEL_CLOSE",
        signature: "CHANNEL_CLOSE(ch) -> NULL",
        sig: None,
        group: BuiltinGroup::Concurrent,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V07,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§17.2",
    },
    BuiltinSpec {
        name: "CHANNEL_SEND",
        signature: "CHANNEL_SEND(ch, v) -> NULL",
        sig: None,
        group: BuiltinGroup::Concurrent,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V07,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§17.2",
    },
    BuiltinSpec {
        name: "CHANNEL_RECV",
        signature: "CHANNEL_RECV(ch) -> v / ERR(ChannelClosed)",
        sig: None,
        group: BuiltinGroup::Concurrent,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V07,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§17.2",
    },
    BuiltinSpec {
        name: "CHANNEL_TRY_SEND",
        signature: "CHANNEL_TRY_SEND(ch, v) -> BOOLEAN",
        sig: None,
        group: BuiltinGroup::Concurrent,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V07,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§17.2",
    },
    BuiltinSpec {
        name: "CHANNEL_TRY_RECV",
        signature: "CHANNEL_TRY_RECV(ch) -> v / NULL / ERR(ChannelClosed)",
        sig: None,
        group: BuiltinGroup::Concurrent,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V07,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§17.2",
    },
    BuiltinSpec {
        name: "CHANNEL_LEN",
        signature: "CHANNEL_LEN(ch) -> INTEGER",
        sig: None,
        group: BuiltinGroup::Concurrent,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V07,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§17.2",
    },
    BuiltinSpec {
        name: "CHANNEL_CAP",
        signature: "CHANNEL_CAP(ch) -> INTEGER",
        sig: None,
        group: BuiltinGroup::Concurrent,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V07,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§17.2",
    },
];
// ──────────────────────────────────────────────────────────────────────
// Lookup helpers
// ──────────────────────────────────────────────────────────────────────

/// 名字 -> 注册表条目。`O(N)` 线性扫描 (B11 88 条目,性能不是瓶颈)。
pub fn lookup(name: &str) -> Option<&'static BuiltinSpec> {
    BUILTIN_REGISTRY.iter().find(|s| s.name == name)
}

/// 名字 -> 结构化签名(Step 5 · A6′)。
///
/// `None` = 该条目不在首批 60 条内,或返回类型本身就推不出来
/// (如 `+` 的多态返回、`AT_K` 的 `-> v` 透传)。两种 `None` 对静态层
/// 是**同一种**结果:落 `Ty::Dynamic`,不产生诊断。
///
/// 这是 `wlwl-eval` 暴露给静态层的**唯一**结构化入口 —— `wlwl-types`
/// 不依赖本 crate(ADR-0020 Decision 1),映射由 `wlwl-cli` 那一层做。
pub fn builtin_sig(name: &str) -> Option<BuiltinSig> {
    lookup(name).and_then(|s| s.sig)
}

/// 所有 ERR 消费者名字(spec `err_consumer = Yes`)。
pub fn err_consumer_names() -> Vec<&'static str> {
    BUILTIN_REGISTRY
        .iter()
        .filter(|s| s.err_consumer == ErrConsumerStatus::Yes)
        .map(|s| s.name)
        .collect()
}

/// 所有词法层宏函数名字 (LexerMacro) + macro_fn 标 true 的 builtin (NOT / UNWRAP_OR)。
pub fn macro_names() -> Vec<&'static str> {
    BUILTIN_REGISTRY
        .iter()
        .filter(|s| {
            s.dispatch == DispatchStatus::LexerMacro
                || (s.macro_fn && s.dispatch == DispatchStatus::ResolvedBuiltin)
        })
        .map(|s| s.name)
        .collect()
}

/// 所有 `resolve_builtin` 接管的 (canonical + compat)。
pub fn resolved_builtin_names() -> Vec<&'static str> {
    BUILTIN_REGISTRY
        .iter()
        .filter(|s| {
            matches!(
                s.dispatch,
                DispatchStatus::ResolvedBuiltin | DispatchStatus::ResolvedCompat
            )
        })
        .map(|s| s.name)
        .collect()
}

/// 所有 `Deferred` 的名字 — 用于"实现 gap"报告 / Phase B12+ 排期。
pub fn deferred_names() -> Vec<&'static str> {
    BUILTIN_REGISTRY
        .iter()
        .filter(|s| s.dispatch == DispatchStatus::Deferred)
        .map(|s| s.name)
        .collect()
}
// ──────────────────────────────────────────────────────────────────────
// Markdown generator
// ──────────────────────────────────────────────────────────────────────

/// 把单元格里的 `|` 转义成 `\|`,否则 markdown 表格会被竖线劈开。
///
/// [v0.10.1 / R10-065] 修之前 `generate_appendix_g_md()` 直接把
/// `spec.name` / `spec.signature` 插进 `| ... |`,于是 `||` 这个运算符
/// 在 `docs/appendix_G.md` 里生成成
///
/// ```text
/// | `||` | `||(a, b) -> BOOLEAN (v0.6 §4.3 short-circuit)` | ✔ | ... |
/// ```
///
/// 这一行对任何 markdown 表格解析器来说都是 **9 列而不是 8 列** ——
/// `||` 把签名格劈成了两半。spec 自己那一份一直是转义过的
/// (`docs/standard/wlwl-spec-v0.10.md` 写 `\|\|`),只有这个生成器
/// 忘了。名字和签名两列都过一遍:`name` 同样可能是运算符。
fn escape_cell(s: &str) -> String {
    s.replace('|', "\\|")
}

/// 把注册表渲染成 spec 附录 G 风格的 markdown 表格。
///
/// 用法:在 B11 实施时手工调用一次写到 `docs/appendix_G.md`。之后
/// 任何改动注册表后,再重跑一遍(单元测试 `b11_generated_md_matches_registry`
/// 守住生成器本身)。
///
/// 输出格式 = spec v0.4 附录 G 表格镜像(分组小标题 + 表格),便于
/// 用户 / 文档工具 grep。
pub fn generate_appendix_g_md() -> String {
    use std::fmt::Write;
    let mut out = String::new();

    // header
    out.push_str("# 附录 G:全局内建注册表 (impl 视角)\n\n");
    out.push_str("> 本文件由 `wlwl-eval::registry::generate_appendix_g_md()` 自动生成。\n");
    out.push_str(
        "> 单源真相是 `crates/wlwl-eval/src/registry.rs::BUILTIN_REGISTRY`,本 markdown 是镜像。\n",
    );
    out.push_str(
        "> 修改流程:改注册表 -> 跑本函数重写本文件 -> 跑 `cargo test` 验证 lock test。\n\n",
    );
    out.push_str("> 对照规范:`docs/standard/wlwl-spec-v0.10.md` 附录 G (规范性)。\n\n");
    let n_resolved = BUILTIN_REGISTRY
        .iter()
        .filter(|s| {
            matches!(
                s.dispatch,
                DispatchStatus::ResolvedBuiltin | DispatchStatus::ResolvedCompat
            )
        })
        .count();
    let n_macro = BUILTIN_REGISTRY
        .iter()
        .filter(|s| s.dispatch == DispatchStatus::LexerMacro)
        .count();
    let n_deferred = deferred_names().len();
    let _ = writeln!(
        out,
        "总条目数:**{}** | 已实现:**{}** | LexerMacro:**{}** | Deferred:**{}**\n\n",
        BUILTIN_REGISTRY.len(),
        n_resolved,
        n_macro,
        n_deferred,
    );

    // column header
    out.push_str("| 名称 | 签名 | ERR 消费者 (§8.3) | 宏函数 (§1.4) | 引入 | 状态 | 实现位置 |\n");
    out.push_str("|------|------|--------------------|---------------|------|------|----------|\n");

    // body
    //
    // [v0.10.1 / R10-065] 修之前这里写的是 `for spec in BUILTIN_REGISTRY`,
    // 注释声称「already sorted by group」—— **但注册表本身没排**。
    // `PUSH` 是 `Array`、紧随其后的 `POP` / `AT_K` 是 `Dict`、再往下
    // `SHIFT` 又回到 `Array`,于是 `current_group != Some(spec.group)`
    // 在相邻行反复成立,同一个分组表头在 `docs/appendix_G.md` 里
    // 重复出现七八次,并且 ARRAY 操作下面挂着 DICT 的条目。实测原文件
    // 65 / 67 / 70 / 78 行连着写了两次「ARRAY 操作 (8 条)」和两次
    // 「DICT 操作 (8 条)」。
    //
    // 修法:生成前按 group 稳定排序,让每个分组表头**恰好出现一次**。
    // `BuiltinGroup` 的声明序(`Io -> Conv -> Result -> ... -> Concurrent`)
    // 就是 `Ord` 的顺序,也与 spec 表格的行序一致,不需要额外映射表。
    let mut rows: Vec<&BuiltinSpec> = BUILTIN_REGISTRY.iter().collect();
    rows.sort_by_key(|s| s.group);

    let mut current_group: Option<BuiltinGroup> = None;
    for spec in &rows {
        if current_group != Some(spec.group) {
            let count = rows.iter().filter(|s| s.group == spec.group).count();
            let _ = writeln!(out, "<!-- {} ({} 条) -->", spec.group.label(), count);
            current_group = Some(spec.group);
        }
        let macro_glyph = if spec.macro_fn { "✔" } else { "❌" };
        let location = match spec.dispatch {
            DispatchStatus::ResolvedBuiltin => format!("`resolve_builtin` ({})", spec.section),
            DispatchStatus::ResolvedCompat => {
                format!("`resolve_builtin` (compat, {})", spec.section)
            }
            DispatchStatus::LexerMacro => format!("parser -> `Expr::*` ({})", spec.section),
            DispatchStatus::Deferred => "—".into(),
        };
        let _ = writeln!(
            out,
            "| `{}` | `{}` | {} | {} | {} | {} | {} |",
            escape_cell(spec.name),
            escape_cell(spec.signature),
            spec.err_consumer.glyph(),
            macro_glyph,
            spec.version.as_str(),
            spec.dispatch.label(),
            location,
        );
    }

    // footer with impl gap summary
    let deferred = deferred_names();
    if !deferred.is_empty() {
        out.push_str("\n## 实现 gap (Deferred)\n\n");
        out.push_str("> spec 列了,impl 尚未接;按 spec 版本分桶:\n\n");
        let mut by_version: std::collections::BTreeMap<Version, Vec<&str>> =
            std::collections::BTreeMap::new();
        for spec in BUILTIN_REGISTRY
            .iter()
            .filter(|s| s.dispatch == DispatchStatus::Deferred)
        {
            by_version.entry(spec.version).or_default().push(spec.name);
        }
        for (v, names) in by_version {
            let _ = writeln!(
                out,
                "- **{}** ({} 项): {}\n",
                v.as_str(),
                names.len(),
                names.join(", ")
            );
        }
    }

    out
}

// ──────────────────────────────────────────────────────────────────────
// Unit tests (in-module)
// ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ---- Step 5 (A6'): 首批结构化签名 ----

    #[test]
    fn appendix_g_regen_is_stable_and_ignores_the_structured_field() {
        // `sig` 是**加法**字段:附录 G 生成器读的仍是 `signature` 文档串,
        // 所以加 60 条结构化签名**不得**改变附录 G 的任何一字节。
        //
        // 锁法:生成两次必须逐字节相同(纯函数),且生成的表格行数仍等于
        // 注册表条数 —— 少一行就说明生成器读错了字段。
        let a = generate_appendix_g_md();
        let b = generate_appendix_g_md();
        assert_eq!(a, b, "generate_appendix_g_md must be deterministic");

        // 每一行都带文档签名;结构化字段不参与渲染。
        //
        // [v0.10.1 / R10-065] 比较的是**转义后**的名字。生成器现在会把
        // 单元格里的 `|` 写成 `\|`,所以 `||` 这个运算符在输出里是
        // `\|\|`,用裸 `s.name` 去 `contains` 会对它假性失败 —— 这正是
        // 修转义时这条测试转红的原因。行的存在性判据要用同一个渲染口径。
        for s in BUILTIN_REGISTRY {
            assert!(
                a.contains(&escape_cell(s.name)),
                "appendix G lost the row for {}",
                s.name
            );
        }
        // 首批里带结构化签名的那条,文档签名原样出现在附录里。
        assert!(a.contains("LEN(coll) -> INTEGER"));
        assert!(a.contains("UPPER(s) -> STRING"));
    }

    #[test]
    fn builtin_sig_batch1_covers_exactly_the_planned_entries() {
        // 首批 = 58 条。**数字本身是锁**:计划 §3.5 定的是 ~40-60,
        // 超 5 人日就削条目。锁定它意味着任何人扩/减一条时,必须先
        // 改这条断言并说明理由 —— 而不是悄悄改变首批覆盖面。
        //
        // [v0.10.1 / R10-064] 60 → 58:`AND` / `OR` 此前带结构化签名,实测
        // **调不通**(`E0020: undefined name`),已从注册表移除。
        // 这不是「范围裁剪」,是把一个假的覆盖面改回真的。
        let with_sig: Vec<&str> = BUILTIN_REGISTRY
            .iter()
            .filter(|s| s.sig.is_some())
            .map(|s| s.name)
            .collect();
        assert_eq!(
            with_sig.len(),
            58,
            "batch 1 must stay at 58 entries; got {:?}",
            with_sig
        );
        // 反向:未结构化的条目必须显式是 `None`,不能靠「忘了写」。
        let without: Vec<&str> = BUILTIN_REGISTRY
            .iter()
            .filter(|s| s.sig.is_none())
            .map(|s| s.name)
            .collect();
        // [v0.10.1 / R10-064] 50 → 48:`MODULE` / `EXPECT_ERR` 此前就没有
        // 结构化签名(与 `AND` / `OR` 那两条不同),移除后未结构化面少了 2 条。
        assert_eq!(without.len(), 48);
    }

    #[test]
    fn builtin_sig_batch1_covers_the_groups_the_plan_named() {
        // 计划点名的分组:算术 / 比较 / STRING / ARRAY / DICT /
        // RESULT 消费者 / 控制流。首批必须**整组覆盖**这些,而不是
        // 挑几个好看的。
        let covered = |name: &str| builtin_sig(name).is_some();
        for n in ["+", "-", "*", "/", "%", "==", "!=", ">", "<", ">=", "<="] {
            assert!(covered(n), "Op group member {n} must be in batch 1");
        }
        for n in [
            "UPPER",
            "LOWER",
            "SUB",
            "REPLACE",
            "SPLIT",
            "TRIM",
            "TRIM_START",
            "TRIM_END",
            "STARTS_WITH",
            "ENDS_WITH",
            "REPEAT",
            "PAD_START",
            "PAD_END",
            "CODEPOINTS",
            "FROM_CODEPOINTS",
        ] {
            assert!(covered(n), "String group member {n} must be in batch 1");
        }
        for n in [
            "PUSH", "SHIFT", "UNSHIFT", "SLICE", "CONCAT", "CONTAINS", "INDEX", "REVERSE",
        ] {
            assert!(covered(n), "Array group member {n} must be in batch 1");
        }
        for n in [
            "AT_K",
            "POP",
            "REMOVE_KEY",
            "DEL",
            "KEYS",
            "VALUES",
            "HAS",
            "MERGE",
        ] {
            assert!(covered(n), "Dict group member {n} must be in batch 1");
        }
        for n in ["LEN", "STR", "INT", "FLOAT", "BOOL", "CALL"] {
            assert!(covered(n), "Conv group member {n} must be in batch 1");
        }
        // [v0.10.1 / R10-064] `AND` / `OR` 此前在这里,已随它们从注册表
        // 移除而删去 —— 留着会断言一个不存在的条目必须有结构化签名。
        //
        // 保留循环形态:这是一张「Control 组的布尔成员」名单,下一批结构化签名
        // 加进来时直接加一行即可。clippy 的 `single_element_loop` 在这里是
        // 误报(它不知道这张名单还会长),故显式豁免并说明。
        #[allow(clippy::single_element_loop)]
        for n in ["NOT"] {
            assert!(covered(n), "Control boolean member {n} must be in batch 1");
        }
    }

    #[test]
    fn builtin_sig_batch1_return_types_match_the_doc_signature() {
        // 首批是**从 spec 附录 G 的文档串转写**过来的,所以每一条都必须
        // 与同一条目的 `signature` 字符串自洽。锁住这个映射,是为了
        // 防止以后有人改结构化字段却忘了同步文档串(或反之)。
        let expected: &[(&str, SigTy)] = &[
            ("LEN", SigTy::Integer),
            ("STR", SigTy::String),
            ("INT", SigTy::Result),
            ("BOOL", SigTy::Boolean),
            ("INPUT", SigTy::String),
            ("PRINT", SigTy::Null),
            ("UPPER", SigTy::String),
            ("SPLIT", SigTy::Array),
            ("STARTS_WITH", SigTy::Boolean),
            ("CODEPOINTS", SigTy::Array),
            ("PUSH", SigTy::Array),
            ("CONTAINS", SigTy::Boolean),
            ("INDEX", SigTy::Integer),
            ("KEYS", SigTy::Array),
            ("HAS", SigTy::Boolean),
            ("MERGE", SigTy::Dict),
            ("==", SigTy::Boolean),
            (">=", SigTy::Boolean),
            ("%", SigTy::Integer),
            ("&&", SigTy::Boolean),
            // [v0.10.1 / R10-064] `AND` 此前在这里;它实测调不通,已随
            // 注册表条目一起移除。`OR` 同理,原本就不在这张表里。
            ("NOT", SigTy::Boolean),
            ("FORMAT", SigTy::String),
            ("ARRAY", SigTy::Array),
            ("DICT", SigTy::Dict),
        ];
        for (name, ret) in expected {
            let sig = builtin_sig(name).unwrap_or_else(|| panic!("{name} missing from batch 1"));
            assert_eq!(sig.ret, *ret, "{name} return type");
        }
    }

    #[test]
    fn builtin_sig_params_are_always_unknown_in_batch1() {
        // 首批**一律不检查形参**。可选形参(`SUB(s, start, end?)`)与
        // 变长实参(`PRINT(args...)`)让任何精确元数表示都必然误报,与
        // ADR-0020 的「不误报优先」冲突。锁死:首批不得偷偷带上形参。
        for s in BUILTIN_REGISTRY.iter().filter(|s| s.sig.is_some()) {
            assert!(
                s.sig.expect("checked above").params.is_none(),
                "{} must not carry params in batch 1",
                s.name
            );
        }
    }

    #[test]
    fn polymorphic_and_passthrough_returns_are_marked_dynamic() {
        // 这几条**看起来**有确定返回类型,实际不是,必须留在 `Dynamic`:
        // - `+` / `-` / `*` / `/`:spec 附录 G 写的是 `INTEGER / FLOAT
        //   / STRING / ARRAY`(字符串拼接也走 `+`),多态。
        // - `AT_K` / `POP`:文档串就是 `-> v`,取值透传。
        // - `CALL` / `NEG`:同理。
        for n in ["+", "-", "*", "/", "NEG", "AT_K", "POP", "CALL"] {
            let sig = builtin_sig(n).unwrap_or_else(|| panic!("{n} is in batch 1"));
            assert_eq!(sig.ret, SigTy::Dynamic, "{n} must stay Dynamic");
        }
    }

    #[test]
    fn doc_signature_column_is_never_emptied() {
        // 结构化字段是**加法**:文档串一条都不能丢。附录 G 生成器
        // (`generate_appendix_g_md`)读的仍是 `signature`。
        for s in BUILTIN_REGISTRY {
            assert!(
                !s.signature.trim().is_empty(),
                "{} lost its doc signature",
                s.name
            );
        }
    }

    #[test]
    fn unstructured_entries_stay_none_for_a_known_reason() {
        // 未入首批的分组各有其理由,锁住名单防止「漏了」被当成 bug 顺手
        // 补上(那会绕过 D-1 / 预算纪律):
        // - Control 流程形(IF / WHILE / FOR / MATCH / RETURN / BREAK /
        //   CONTINUE)与 Result 宏(IS_OK / IS_ERR / TRY / OR_DIE /
        //   UNWRAP / UNWRAP_OR / ERR_PAYLOAD / OK / ERR / PANIC / WRAP /
        //   EXPECT_ERR)由 parser 降级成 `Expr::*`,**根本不走
        //   `Expr::Call`**,注册表签名对静态层无增益;
        // - Subscript / Module / Oop / Property / Concurrent 共 30 条
        //   留给后续批次。
        for n in [
            "IF",
            "WHILE",
            "FOR",
            "MATCH",
            "RETURN",
            "BREAK",
            "CONTINUE",
            "IS_OK",
            "IS_ERR",
            "TRY",
            "OR_DIE",
            "UNWRAP",
            "UNWRAP_OR",
            "ERR_PAYLOAD",
            "OK",
            "ERR",
            "PANIC",
            "WRAP",
            "EXPECT_ERR",
        ] {
            assert!(
                builtin_sig(n).is_none(),
                "{n} is lowered to an Expr node; a builtin sig would be dead weight"
            );
        }
        for n in [
            "INDEX_GET",
            "INDEX_SET",
            "AT",
            "MODULE_REF",
            "EXPORT",
            "IMPORT",
            "MODULE",
            "CLASS",
            "NEW",
            "THIS",
            "GET_PROP",
            "SET_PROP",
            "CALL_METHOD",
            "SCOPE",
            "SPAWN",
            "AWAIT",
            "YIELD",
            "SHIELD",
            "CHANNEL_NEW",
            "CHANNEL_CLOSE",
            "CHANNEL_SEND",
            "CHANNEL_RECV",
            "CHANNEL_TRY_SEND",
            "CHANNEL_TRY_RECV",
            "CHANNEL_LEN",
            "CHANNEL_CAP",
            "TASK_CURRENT",
            "TASK_IS_CANCELLED",
            "TASK_CANCEL",
            "TASK_CANCEL_PARENT",
        ] {
            assert!(builtin_sig(n).is_none(), "{n} belongs to a later batch");
        }
    }

    #[test]
    fn registry_has_no_duplicate_names() {
        let mut names: Vec<&str> = BUILTIN_REGISTRY.iter().map(|s| s.name).collect();
        names.sort();
        let original_len = names.len();
        names.dedup();
        assert_eq!(
            original_len,
            names.len(),
            "BUILTIN_REGISTRY has duplicate names"
        );
        assert_eq!(
            BUILTIN_REGISTRY.len(),
            106,
            "expected 106 entries: 93 (v0.6) + 17 concurrent (v0.7 §17) \
             - 4 removed in v0.10.1 (AND / OR / MODULE / EXPECT_ERR: \
             registered but unreachable, see tests/lexer_macro_coverage.rs)"
        );
    }

    #[test]
    fn all_groups_represented() {
        for g in [
            BuiltinGroup::Io,
            BuiltinGroup::Conv,
            BuiltinGroup::Result,
            BuiltinGroup::Control,
            BuiltinGroup::Op,
            BuiltinGroup::Array,
            BuiltinGroup::Dict,
            BuiltinGroup::Subscript,
            BuiltinGroup::String,
            BuiltinGroup::Format,
            BuiltinGroup::Module,
            BuiltinGroup::Oop,
            BuiltinGroup::Property,
            BuiltinGroup::Ctor,
            BuiltinGroup::Concurrent,
        ] {
            let count = BUILTIN_REGISTRY.iter().filter(|s| s.group == g).count();
            assert!(count >= 1, "group {:?} has no entries", g);
        }
    }

    #[test]
    fn lookup_works() {
        assert!(lookup("PRINT").is_some());
        assert!(lookup("NOT").is_some());
        assert!(lookup("==").is_some());
        assert!(lookup("NEG").is_some());
        assert!(lookup("__NOT_A_REAL_BUILTIN__").is_none());
    }

    #[test]
    fn resolved_builtin_names_matches_filter() {
        let resolved = resolved_builtin_names();
        assert!(resolved.contains(&"PRINT"));
        assert!(resolved.contains(&"NOT"));
        assert!(resolved.contains(&"FORMAT"));
        assert!(resolved.contains(&"OR_DIE"));
        assert!(!resolved.contains(&"IF"));
        assert!(!resolved.contains(&"OK"));
    }

    #[test]
    fn generated_md_includes_all_entries() {
        let md = generate_appendix_g_md();
        for spec in BUILTIN_REGISTRY.iter().take(5) {
            assert!(
                md.contains(&format!("`{}`", spec.name)),
                "missing {} in md",
                spec.name
            );
        }
        if !deferred_names().is_empty() {
            assert!(md.contains("## 实现 gap"));
        }
        assert!(md.contains("I/O"));
        assert!(md.contains("控制流"));
        assert!(md.contains("构造器"));
    }

    /// v0.8 D8-001: POP registry entry must document the actual 3-arg
    /// DICT semantics that dispatch (`builtin_at_k` at lib.rs:4092)
    /// implements. Pre-v0.8 the entry said `POP(arr) -> ARRAY` under
    /// `BuiltinGroup::Array` — wrong on three fields (signature, group,
    /// version). This test guards against re-introduction.
    #[test]
    fn pop_registry_entry_matches_dispatch() {
        let pop = BUILTIN_REGISTRY
            .iter()
            .find(|s| s.name == "POP")
            .expect("POP must be in BUILTIN_REGISTRY");
        assert_eq!(
            pop.group,
            BuiltinGroup::Dict,
            "POP must be in Dict group (it dispatches to builtin_at_k which \
             takes a DICT, not an ARRAY); pre-v0.8 was incorrectly Array"
        );
        assert_eq!(
            pop.version,
            Version::V06,
            "POP became a 3-arg DICT alias in v0.6 (P4-B12-002); pre-v0.8 \
             said V02"
        );
        assert!(
            pop.signature.contains("d, k, default") || pop.signature.contains("3-arg"),
            "POP signature must mention 3-arg / 'd, k, default'; got: {:?}",
            pop.signature
        );
        assert_eq!(
            pop.dispatch,
            DispatchStatus::ResolvedCompat,
            "POP keeps ResolvedCompat dispatch for source-level back-compat \
             with v0.5 programs"
        );
        assert_eq!(pop.section, "§10.4");
        assert!(!pop.macro_fn, "POP is a resolved builtin, not a macro");
    }

    /// v0.8 §2.9 / D8-002 regression guard: every §-anchor in the
    /// generated `docs/appendix_G.md` must come from the v0.7 spec
    /// chapter whitelist. Locks §13.x / §15.x / §12.x / §7.x / §9.1 /
    /// §9.2 / §10.6 / §11.4 / §10.1-§10.2 / etc. (the v0.4/v0.6 stale
    /// numbers) from ever creeping back in via a future registry edit.
    ///
    /// Reads the on-disk `docs/appendix_G.md` (relative to repo root),
    /// extracts every `§X[.Y]` / `§X [placeholder]` token inside the
    /// last column of each table row, and asserts each is in the
    /// v0.7 spec chapter whitelist.
    #[test]
    fn appendix_g_anchors_match_v07_section_numbers() {
        // Locate `docs/appendix_G.md` relative to repo root.
        // CARGO_MANIFEST_DIR = impl/crates/wlwl-eval, so:
        //   ../..  -> impl/
        //   ../../.. -> D:\Project\wlwl
        let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let md_path = manifest_dir
            .join("..")
            .join("..")
            .join("..")
            .join("docs")
            .join("appendix_G.md");
        let md_text = std::fs::read_to_string(&md_path).unwrap_or_else(|e| {
            panic!(
                "cannot read appendix_G.md at {}: {}; \
                 run `cargo run --bin gen-appendix-g` to regenerate",
                md_path.display(),
                e
            )
        });

        // v0.7 spec chapter whitelist. Anything outside this set is
        // v0.4/v0.6-era and must not appear in regenerated md.
        // (OOP/Property use the bracketed placeholder form.)
        let whitelist: &[&str] = &[
            "§2.1",
            "§2.2",
            "§2.3",
            "§2.4",
            "§2.5",
            "§3.1",
            "§3.2",
            "§3.3",
            "§3.4",
            "§3.5",
            "§4.1",
            "§4.2",
            "§4.3",
            "§4.5",
            "§4.6",
            "§4.7",
            "§4.8",
            "§5.1",
            "§5.2",
            "§5.3",
            "§5.4",
            "§6",
            "§8.1",
            "§8.2",
            "§8.3",
            "§8.4",
            "§8.5",
            "§9",
            "§10.1",
            "§10.2",
            "§10.3",
            "§10.4",
            "§10.5",
            "§10.6",
            "§10.7",
            "§10.8",
            "§10.9",
            "§10.10",
            "§10.11",
            "§11",
            "§11 [占位;OOP 未实现]",
            // [v0.9 Step 9a-6 / plan §11.5] New OOP chapter
            // numbers per plan §4.3 "spec §13 OOP 关键字与对象模型;
            // §14 行为类型与会话类型协议; §15 THIS 与线性
            // capability; §16 OOP 与并发的交互". Adding these to
            // the valid-section list lets the snapshot test
            // recognise them on the OOP builtin rows above.
            "§13",
            "§14",
            "§15",
            "§16",
            "§17.0",
            "§17.1",
            "§17.2",
            "§17.3",
            "§17.4",
            "§17.5",
            "§17.6",
            "§17.7",
        ];

        // Hand-rolled scanner (no `regex` dep available): find every
        // `§` in each table row, consume digits + optional `.digits`,
        // then optionally consume ` [占位;OOP 未实现]`.
        let scan = |line: &str| -> Vec<String> {
            let bytes = line.as_bytes();
            let mut tokens = Vec::new();
            let mut i = 0;
            while i < bytes.len() {
                if bytes[i] == b'\xc2' && i + 1 < bytes.len() && bytes[i + 1] == b'\xa7' {
                    // '§' is U+00A7 = 0xC2 0xA7 in UTF-8.
                    let start = i;
                    let mut j = i + 2;
                    // Consume leading digits.
                    while j < bytes.len() && bytes[j].is_ascii_digit() {
                        j += 1;
                    }
                    // Consume optional `.digits`.
                    if j < bytes.len() && bytes[j] == b'.' {
                        j += 1;
                        while j < bytes.len() && bytes[j].is_ascii_digit() {
                            j += 1;
                        }
                    }
                    // Consume optional ` [占位;OOP 未实现]` suffix.
                    // The literal bytes for ' [占位;OOP 未实现]' are:
                    //   20 5B E5 8D A0 E4 BD 8D 3B 4F 4F 50 20 E6 9C AA E5 AE 9E E7 8E B0 5D
                    let suffix: &[u8] =
                        b" [\xe5\x8d\xa0\xe4\xbd\x8d;OOP \xe6\x9c\xaa\xe5\xae\x9e\xe7\x8e\xb0]";
                    if j + suffix.len() <= bytes.len() && &bytes[j..j + suffix.len()] == suffix {
                        j += suffix.len();
                    }
                    if j > start + 2 {
                        // We consumed at least one digit (or the
                        // prefix was just "§" with no digits — skip).
                        tokens.push(String::from_utf8_lossy(&bytes[start..j]).into_owned());
                    }
                    i = j;
                } else {
                    i += 1;
                }
            }
            tokens
        };

        let mut bad: Vec<(usize, String)> = Vec::new();
        for (idx, line) in md_text.lines().enumerate() {
            if !line.starts_with("| `") {
                continue; // skip header / non-table rows
            }
            for token in scan(line) {
                if !whitelist.contains(&token.as_str()) {
                    bad.push((idx + 1, token));
                }
            }
        }
        assert!(
            bad.is_empty(),
            "appendix_G.md has {} stale §-anchor(s) outside v0.7 spec \
             whitelist; first 5: {:?}\n\
             To fix: regenerate via `cargo run --bin gen-appendix-g` \
             AFTER fixing the underlying registry entry / spec drift.",
            bad.len(),
            bad.iter().take(5).collect::<Vec<_>>()
        );
    }
}
