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
//! ## 附录 G markdown 生成
//!
//! `generate_appendix_g_md()` 把注册表渲染成 markdown。Phase B11 把它写
//! 到 `docs/appendix_G.md`(手工调一次);之后每次动注册表只需重跑这个
//! 函数即可。锁测试 `b11_generated_md_matches_registry` 守住生成器 +
//! 注册表的一致性。

// ──────────────────────────────────────────────────────────────────────
// Types
// ──────────────────────────────────────────────────────────────────────

/// One row of spec 附录 G, mirrored as a Rust const.
#[derive(Debug, Clone, Copy)]
pub struct BuiltinSpec {
    /// Spec 写的名字 (大写 / 操作符字面)
    pub name: &'static str,
    /// Spec 的签名(`fn(args) → ret` 风格)
    pub signature: &'static str,
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
    /// `docs/plan/wlwl-build-plan-v0.8.md` §2.3 (deviation D8-002)。
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
        group: BuiltinGroup::Conv,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V02,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§10.3",
    },
    BuiltinSpec {
        name: "INT",
        signature: "INT(s) -> OK(INTEGER) / ERR(ParseError)",
        group: BuiltinGroup::Conv,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V02,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§10.3",
    },
    BuiltinSpec {
        name: "FLOAT",
        signature: "FLOAT(s) -> OK(FLOAT) / ERR(ParseError)",
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
        group: BuiltinGroup::Result,
        err_consumer: ErrConsumerStatus::Yes,
        macro_fn: true,
        version: Version::V04,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§8.3",
    },
    BuiltinSpec {
        name: "UNWRAP",
        signature: "UNWRAP(x) -> v / PANIC",
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
        group: BuiltinGroup::Result,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: true,
        version: Version::V04,
        dispatch: DispatchStatus::LexerMacro,
        section: "§8.1",
    },
    BuiltinSpec {
        name: "EXPECT_ERR",
        signature: "EXPECT_ERR(expr) -> OK(payload) / ERR(E0049)",
        group: BuiltinGroup::Result,
        err_consumer: ErrConsumerStatus::Yes,
        macro_fn: true,
        version: Version::V04,
        dispatch: DispatchStatus::LexerMacro,
        section: "§8.3",
    },
    BuiltinSpec {
        name: "ERR",
        signature: "ERR(e) -> RESULT",
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
        signature: "WHILE(cond, body) -> v",
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
        group: BuiltinGroup::Control,
        err_consumer: ErrConsumerStatus::Na,
        macro_fn: true,
        version: Version::V02,
        dispatch: DispatchStatus::LexerMacro,
        section: "§6",
    },
    BuiltinSpec {
        name: "AND",
        signature: "AND(a, b) -> BOOLEAN (short-circuit)",
        group: BuiltinGroup::Control,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: true,
        version: Version::V02,
        dispatch: DispatchStatus::LexerMacro,
        section: "§4.3",
    },
    BuiltinSpec {
        name: "OR",
        signature: "OR(a, b) -> BOOLEAN (short-circuit)",
        group: BuiltinGroup::Control,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: true,
        version: Version::V02,
        dispatch: DispatchStatus::LexerMacro,
        section: "§4.3",
    },
    BuiltinSpec {
        name: "NOT",
        signature: "NOT(a) -> BOOLEAN (取反)",
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
        signature: "=(a, b) -> BOOLEAN / ERR 透传",
        group: BuiltinGroup::Op,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V02,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§4.3",
    },
    BuiltinSpec {
        name: "!=",
        signature: "!(a, b) -> BOOLEAN / ERR 透传",
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
        group: BuiltinGroup::Op,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V02,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§4.3",
    },
    BuiltinSpec {
        name: "+",
        signature: "+(a, b) -> INTEGER / FLOAT",
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
        group: BuiltinGroup::Array,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V02,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§10.4",
    },
    BuiltinSpec {
        name: "INDEX",
        signature: "INDEX(arr, x) -> INTEGER / -1",
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
        signature: "INDEX_GET(coll, k) -> v / E0031",
        group: BuiltinGroup::Subscript,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V04,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§4.5",
    },
    BuiltinSpec {
        name: "INDEX_SET",
        signature: "INDEX_SET(coll, k, v) -> NULL",
        group: BuiltinGroup::Subscript,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V04,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§4.5",
    },
    BuiltinSpec {
        name: "AT",
        signature: "AT(coll, i) -> v",
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
        group: BuiltinGroup::String,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V02,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§10.5",
    },
    BuiltinSpec {
        name: "SUB",
        signature: "SUB(s, start, end?) -> STRING",
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
        signature: "MODULE_REF(path) -> MODULE",
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
        group: BuiltinGroup::Module,
        err_consumer: ErrConsumerStatus::Na,
        macro_fn: true,
        version: Version::V02,
        dispatch: DispatchStatus::LexerMacro,
        section: "§9",
    },
    BuiltinSpec {
        name: "IMPORT",
        signature: "IMPORT(path, names, opts?) -> NULL",
        group: BuiltinGroup::Module,
        err_consumer: ErrConsumerStatus::Na,
        macro_fn: true,
        version: Version::V02,
        dispatch: DispatchStatus::LexerMacro,
        section: "§9",
    },
    BuiltinSpec {
        name: "MODULE",
        signature: "MODULE(name?, body) -> NULL",
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
        signature: "CLASS(name?, parent, members) -> CLASS",
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
        group: BuiltinGroup::Oop,
        err_consumer: ErrConsumerStatus::Na,
        macro_fn: true,
        version: Version::V02,
        dispatch: DispatchStatus::LexerMacro,
        section: "§13",
    },
    BuiltinSpec {
        name: "THIS",
        signature: "THIS -> 当前实例",
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
        signature: "ARRAY(items...) / ARRAY()",
        group: BuiltinGroup::Ctor,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: true,
        version: Version::V02,
        dispatch: DispatchStatus::LexerMacro,
        section: "§10.9",
    },
    BuiltinSpec {
        name: "DICT",
        signature: "DICT(pairs...) / DICT()",
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
        group: BuiltinGroup::Concurrent,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V07,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§17.3",
    },
    BuiltinSpec {
        name: "TASK_CANCEL",
        signature: "TASK_CANCEL(task) -> NULL",
        group: BuiltinGroup::Concurrent,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V07,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§17.3",
    },
    BuiltinSpec {
        name: "TASK_CANCEL_PARENT",
        signature: "TASK_CANCEL_PARENT() -> NULL",
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
        group: BuiltinGroup::Concurrent,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V07,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§17.2",
    },
    BuiltinSpec {
        name: "CHANNEL_SEND",
        signature: "CHANNEL_SEND(ch, v) -> NULL / ERR(ChannelWouldBlock)",
        group: BuiltinGroup::Concurrent,
        err_consumer: ErrConsumerStatus::No,
        macro_fn: false,
        version: Version::V07,
        dispatch: DispatchStatus::ResolvedBuiltin,
        section: "§17.2",
    },
    BuiltinSpec {
        name: "CHANNEL_RECV",
        signature: "CHANNEL_RECV(ch) -> v / ERR(ChannelClosed|ChannelWouldBlock)",
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
    out.push_str("> 对照规范:`docs/standard/wlwl-spec-v0.8.md` 附录 G (规范性)。\n\n");
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

    // body (already sorted by group in BUILTIN_REGISTRY)
    let mut current_group: Option<BuiltinGroup> = None;
    for spec in BUILTIN_REGISTRY {
        if current_group != Some(spec.group) {
            let count = BUILTIN_REGISTRY
                .iter()
                .filter(|s| s.group == spec.group)
                .count();
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
            spec.name,
            spec.signature,
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
            110,
            "expected 110 entries: 93 (v0.6) + 17 concurrent builtins (v0.7 §17)"
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
