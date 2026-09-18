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
}

impl BuiltinGroup {
    /// Spec 表格行号 / 章节锚,markdown 用作 § 链接。
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
}

impl Version {
    pub fn as_str(self) -> &'static str {
        match self {
            Version::V02 => "v0.2",
            Version::V04 => "v0.4",
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
/// 行数统计:B11 = **88 unique entries**(spec 附录 G 表格 89 行,
/// `CALL` 在 spec 表出现两次,这里只算 1 条)。
pub const BUILTIN_REGISTRY: &[BuiltinSpec] = &[
    // ── I/O (3) ──────────────────────────────────────────────────
    BuiltinSpec { name: "PRINT",      signature: "PRINT(args...) -> NULL",                       group: BuiltinGroup::Io,       err_consumer: ErrConsumerStatus::No,  macro_fn: false, version: Version::V02, dispatch: DispatchStatus::ResolvedBuiltin, section: "§15.1" },
    BuiltinSpec { name: "PRINT_ERR",  signature: "PRINT_ERR(args...) -> NULL",                   group: BuiltinGroup::Io,       err_consumer: ErrConsumerStatus::No,  macro_fn: false, version: Version::V04, dispatch: DispatchStatus::ResolvedBuiltin, section: "§15.1" },
    BuiltinSpec { name: "INPUT",      signature: "INPUT(prompt?) -> STRING",                     group: BuiltinGroup::Io,       err_consumer: ErrConsumerStatus::No,  macro_fn: false, version: Version::V02, dispatch: DispatchStatus::Deferred,         section: "§15.1" },

    // ── 类型 / 转换 (7) ─────────────────────────────────────────
    BuiltinSpec { name: "LEN",        signature: "LEN(coll) -> INTEGER",                         group: BuiltinGroup::Conv,     err_consumer: ErrConsumerStatus::No,  macro_fn: false, version: Version::V02, dispatch: DispatchStatus::ResolvedBuiltin, section: "§10.5" },
    BuiltinSpec { name: "STR",        signature: "STR(x) -> STRING",                             group: BuiltinGroup::Conv,     err_consumer: ErrConsumerStatus::No,  macro_fn: false, version: Version::V02, dispatch: DispatchStatus::ResolvedBuiltin, section: "§10.3" },
    BuiltinSpec { name: "INT",        signature: "INT(s) -> OK(INTEGER) / ERR(ParseError)",      group: BuiltinGroup::Conv,     err_consumer: ErrConsumerStatus::No,  macro_fn: false, version: Version::V02, dispatch: DispatchStatus::ResolvedBuiltin, section: "§9.5" },
    BuiltinSpec { name: "FLOAT",      signature: "FLOAT(s) -> OK(FLOAT) / ERR(ParseError)",      group: BuiltinGroup::Conv,     err_consumer: ErrConsumerStatus::No,  macro_fn: false, version: Version::V04, dispatch: DispatchStatus::ResolvedBuiltin, section: "§9.5" },
    BuiltinSpec { name: "TYPE",       signature: "TYPE(x) -> STRING (RESULT -> \"RESULT\")",    group: BuiltinGroup::Conv,     err_consumer: ErrConsumerStatus::Yes, macro_fn: true,  version: Version::V02, dispatch: DispatchStatus::ResolvedBuiltin, section: "§2.5" },
    BuiltinSpec { name: "BOOL",       signature: "BOOL(x) -> BOOLEAN",                           group: BuiltinGroup::Conv,     err_consumer: ErrConsumerStatus::No,  macro_fn: false, version: Version::V02, dispatch: DispatchStatus::Deferred,         section: "§2.2" },
    BuiltinSpec { name: "CALL",       signature: "CALL(fn, args...) -> v",                       group: BuiltinGroup::Conv,     err_consumer: ErrConsumerStatus::No,  macro_fn: false, version: Version::V02, dispatch: DispatchStatus::Deferred,         section: "§8.3" },

    // ── RESULT 处理 (11) ────────────────────────────────────────
    BuiltinSpec { name: "IS_OK",      signature: "IS_OK(x) -> BOOLEAN",                          group: BuiltinGroup::Result,   err_consumer: ErrConsumerStatus::Yes, macro_fn: true,  version: Version::V02, dispatch: DispatchStatus::LexerMacro,       section: "§12.2" },
    BuiltinSpec { name: "IS_ERR",     signature: "IS_ERR(x) -> BOOLEAN",                         group: BuiltinGroup::Result,   err_consumer: ErrConsumerStatus::Yes, macro_fn: true,  version: Version::V02, dispatch: DispatchStatus::LexerMacro,       section: "§12.2" },
    BuiltinSpec { name: "OR_DIE",     signature: "OR_DIE(x, default) -> v (v0.3 alias)",         group: BuiltinGroup::Result,   err_consumer: ErrConsumerStatus::Yes, macro_fn: true,  version: Version::V02, dispatch: DispatchStatus::ResolvedCompat,    section: "§12.2" },
    BuiltinSpec { name: "UNWRAP_OR",  signature: "UNWRAP_OR(x, default) -> v",                   group: BuiltinGroup::Result,   err_consumer: ErrConsumerStatus::Yes, macro_fn: true,  version: Version::V04, dispatch: DispatchStatus::ResolvedBuiltin,   section: "§12.2" },
    BuiltinSpec { name: "UNWRAP",     signature: "UNWRAP(x) -> v / PANIC",                       group: BuiltinGroup::Result,   err_consumer: ErrConsumerStatus::Yes, macro_fn: false, version: Version::V04, dispatch: DispatchStatus::ResolvedBuiltin,   section: "§12.2" },
    BuiltinSpec { name: "ERR_PAYLOAD",signature: "ERR_PAYLOAD(x) -> e / E0030",                 group: BuiltinGroup::Result,   err_consumer: ErrConsumerStatus::Yes, macro_fn: false, version: Version::V04, dispatch: DispatchStatus::ResolvedBuiltin,   section: "§12.2" },
    BuiltinSpec { name: "WRAP",       signature: "WRAP(err, ctx) -> ERR / OK",                   group: BuiltinGroup::Result,   err_consumer: ErrConsumerStatus::Yes, macro_fn: false, version: Version::V04, dispatch: DispatchStatus::ResolvedBuiltin,   section: "§12.2" },
    BuiltinSpec { name: "TRY",        signature: "TRY(e) -> v / early-RETURN",                   group: BuiltinGroup::Result,   err_consumer: ErrConsumerStatus::Yes, macro_fn: true,  version: Version::V02, dispatch: DispatchStatus::LexerMacro,       section: "§12.6" },
    BuiltinSpec { name: "PANIC",      signature: "PANIC(msg) -> 终止",                              group: BuiltinGroup::Result,   err_consumer: ErrConsumerStatus::Na,  macro_fn: true,  version: Version::V02, dispatch: DispatchStatus::LexerMacro,       section: "§12.4" },
    BuiltinSpec { name: "OK",         signature: "OK(v) -> RESULT",                              group: BuiltinGroup::Result,   err_consumer: ErrConsumerStatus::No,  macro_fn: true,  version: Version::V04, dispatch: DispatchStatus::LexerMacro,       section: "§12.1" },
    BuiltinSpec { name: "EXPECT_ERR", signature: "EXPECT_ERR(expr) -> OK(payload) / ERR(E0049)",      group: BuiltinGroup::Result,   err_consumer: ErrConsumerStatus::Yes, macro_fn: true,  version: Version::V04, dispatch: DispatchStatus::LexerMacro,       section: "§15.9" },
    BuiltinSpec { name: "ERR",        signature: "ERR(e) -> RESULT",                             group: BuiltinGroup::Result,   err_consumer: ErrConsumerStatus::No,  macro_fn: true,  version: Version::V04, dispatch: DispatchStatus::LexerMacro,       section: "§12.1" },

    // ── 控制流 / 逻辑 (10) ──────────────────────────────────────
    BuiltinSpec { name: "IF",         signature: "IF(cond, t, e?) -> v",                         group: BuiltinGroup::Control,  err_consumer: ErrConsumerStatus::No,  macro_fn: true,  version: Version::V02, dispatch: DispatchStatus::LexerMacro,       section: "§7.1" },
    BuiltinSpec { name: "WHILE",      signature: "WHILE(cond, body) -> v",                       group: BuiltinGroup::Control,  err_consumer: ErrConsumerStatus::No,  macro_fn: true,  version: Version::V02, dispatch: DispatchStatus::LexerMacro,       section: "§7.2" },
    BuiltinSpec { name: "FOR",        signature: "FOR(var, iter, body) -> NULL",                 group: BuiltinGroup::Control,  err_consumer: ErrConsumerStatus::No,  macro_fn: true,  version: Version::V02, dispatch: DispatchStatus::LexerMacro,       section: "§7.3" },
    BuiltinSpec { name: "MATCH",      signature: "MATCH(v, clauses, default?) -> v",             group: BuiltinGroup::Control,  err_consumer: ErrConsumerStatus::No,  macro_fn: true,  version: Version::V04, dispatch: DispatchStatus::LexerMacro,       section: "§7.4" },
    BuiltinSpec { name: "RETURN",     signature: "RETURN(v?) -> 早返",                              group: BuiltinGroup::Control,  err_consumer: ErrConsumerStatus::Na,  macro_fn: true,  version: Version::V02, dispatch: DispatchStatus::LexerMacro,       section: "§7.5" },
    BuiltinSpec { name: "BREAK",      signature: "BREAK() -> 跳出",                                group: BuiltinGroup::Control,  err_consumer: ErrConsumerStatus::Na,  macro_fn: true,  version: Version::V02, dispatch: DispatchStatus::LexerMacro,       section: "§7.5" },
    BuiltinSpec { name: "CONTINUE",   signature: "CONTINUE() -> 跳到下轮",                          group: BuiltinGroup::Control,  err_consumer: ErrConsumerStatus::Na,  macro_fn: true,  version: Version::V02, dispatch: DispatchStatus::LexerMacro,       section: "§7.5" },
    BuiltinSpec { name: "AND",        signature: "AND(a, b) -> BOOLEAN (short-circuit)",         group: BuiltinGroup::Control,  err_consumer: ErrConsumerStatus::No,  macro_fn: true,  version: Version::V02, dispatch: DispatchStatus::LexerMacro,       section: "§3.4" },
    BuiltinSpec { name: "OR",         signature: "OR(a, b) -> BOOLEAN (short-circuit)",          group: BuiltinGroup::Control,  err_consumer: ErrConsumerStatus::No,  macro_fn: true,  version: Version::V02, dispatch: DispatchStatus::LexerMacro,       section: "§3.4" },
    BuiltinSpec { name: "NOT",        signature: "NOT(a) -> BOOLEAN (取反)",                     group: BuiltinGroup::Control,  err_consumer: ErrConsumerStatus::No,  macro_fn: true,  version: Version::V02, dispatch: DispatchStatus::ResolvedBuiltin,   section: "§3.4" },

    // ── 运算符 (12) ─────────────────────────────────────────────
    BuiltinSpec { name: "==",         signature: "=(a, b) -> BOOLEAN / ERR 透传",                group: BuiltinGroup::Op,       err_consumer: ErrConsumerStatus::No,  macro_fn: false, version: Version::V02, dispatch: DispatchStatus::ResolvedBuiltin,   section: "§9.2" },
    BuiltinSpec { name: "!=",         signature: "!(a, b) -> BOOLEAN / ERR 透传",                group: BuiltinGroup::Op,       err_consumer: ErrConsumerStatus::No,  macro_fn: false, version: Version::V02, dispatch: DispatchStatus::ResolvedBuiltin,   section: "§9.2" },
    BuiltinSpec { name: ">",          signature: ">(a, b) -> BOOLEAN / ERR 透传",                group: BuiltinGroup::Op,       err_consumer: ErrConsumerStatus::No,  macro_fn: false, version: Version::V02, dispatch: DispatchStatus::ResolvedBuiltin,   section: "§9.2" },
    BuiltinSpec { name: "<",          signature: "<(a, b) -> BOOLEAN / ERR 透传",                group: BuiltinGroup::Op,       err_consumer: ErrConsumerStatus::No,  macro_fn: false, version: Version::V02, dispatch: DispatchStatus::ResolvedBuiltin,   section: "§9.2" },
    BuiltinSpec { name: ">=",         signature: ">=(a, b) -> BOOLEAN / ERR 透传",               group: BuiltinGroup::Op,       err_consumer: ErrConsumerStatus::No,  macro_fn: false, version: Version::V02, dispatch: DispatchStatus::ResolvedBuiltin,   section: "§9.2" },
    BuiltinSpec { name: "<=",         signature: "<=(a, b) -> BOOLEAN / ERR 透传",               group: BuiltinGroup::Op,       err_consumer: ErrConsumerStatus::No,  macro_fn: false, version: Version::V02, dispatch: DispatchStatus::ResolvedBuiltin,   section: "§9.2" },
    BuiltinSpec { name: "+",          signature: "+(a, b) -> INTEGER / FLOAT",                   group: BuiltinGroup::Op,       err_consumer: ErrConsumerStatus::No,  macro_fn: false, version: Version::V02, dispatch: DispatchStatus::ResolvedBuiltin,   section: "§9.1" },
    BuiltinSpec { name: "-",          signature: "-(a, b) -> INTEGER / FLOAT",                   group: BuiltinGroup::Op,       err_consumer: ErrConsumerStatus::No,  macro_fn: false, version: Version::V02, dispatch: DispatchStatus::ResolvedBuiltin,   section: "§9.1" },
    BuiltinSpec { name: "*",          signature: "*(a, b) -> INTEGER / FLOAT",                   group: BuiltinGroup::Op,       err_consumer: ErrConsumerStatus::No,  macro_fn: false, version: Version::V02, dispatch: DispatchStatus::ResolvedBuiltin,   section: "§9.1" },
    BuiltinSpec { name: "/",          signature: "/(a, b) -> INTEGER / FLOAT",                   group: BuiltinGroup::Op,       err_consumer: ErrConsumerStatus::No,  macro_fn: false, version: Version::V02, dispatch: DispatchStatus::ResolvedBuiltin,   section: "§9.1" },
    BuiltinSpec { name: "%",          signature: "%(a, b) -> INTEGER",                           group: BuiltinGroup::Op,       err_consumer: ErrConsumerStatus::No,  macro_fn: false, version: Version::V02, dispatch: DispatchStatus::ResolvedBuiltin,   section: "§9.1" },
    BuiltinSpec { name: "NEG",        signature: "NEG(a) -> -a",                                 group: BuiltinGroup::Op,       err_consumer: ErrConsumerStatus::No,  macro_fn: false, version: Version::V02, dispatch: DispatchStatus::Deferred,         section: "§9.1" },
    // ── ARRAY 操作 (9) ──────────────────────────────────────────
    BuiltinSpec { name: "PUSH",       signature: "PUSH(arr, x) -> ARRAY",                         group: BuiltinGroup::Array,    err_consumer: ErrConsumerStatus::No,  macro_fn: false, version: Version::V02, dispatch: DispatchStatus::ResolvedBuiltin,   section: "§10.1" },
    BuiltinSpec { name: "POP",        signature: "POP(arr) -> ARRAY",                            group: BuiltinGroup::Array,    err_consumer: ErrConsumerStatus::No,  macro_fn: false, version: Version::V02, dispatch: DispatchStatus::ResolvedBuiltin,   section: "§10.1" },
    BuiltinSpec { name: "SHIFT",      signature: "SHIFT(arr) -> ARRAY",                          group: BuiltinGroup::Array,    err_consumer: ErrConsumerStatus::No,  macro_fn: false, version: Version::V02, dispatch: DispatchStatus::ResolvedBuiltin,   section: "§10.1" },
    BuiltinSpec { name: "UNSHIFT",    signature: "UNSHIFT(arr, x) -> ARRAY",                     group: BuiltinGroup::Array,    err_consumer: ErrConsumerStatus::No,  macro_fn: false, version: Version::V02, dispatch: DispatchStatus::ResolvedBuiltin,   section: "§10.1" },
    BuiltinSpec { name: "SLICE",      signature: "SLICE(arr, start, end?) -> ARRAY",             group: BuiltinGroup::Array,    err_consumer: ErrConsumerStatus::No,  macro_fn: false, version: Version::V02, dispatch: DispatchStatus::ResolvedBuiltin,   section: "§10.1" },
    BuiltinSpec { name: "CONCAT",     signature: "CONCAT(a, b) -> ARRAY",                       group: BuiltinGroup::Array,    err_consumer: ErrConsumerStatus::No,  macro_fn: false, version: Version::V02, dispatch: DispatchStatus::ResolvedBuiltin,   section: "§10.1" },
    BuiltinSpec { name: "CONTAINS",   signature: "CONTAINS(arr, x) -> BOOLEAN",                  group: BuiltinGroup::Array,    err_consumer: ErrConsumerStatus::No,  macro_fn: false, version: Version::V02, dispatch: DispatchStatus::ResolvedBuiltin,   section: "§10.1" },
    BuiltinSpec { name: "INDEX",      signature: "INDEX(arr, x) -> INTEGER / -1",                group: BuiltinGroup::Array,    err_consumer: ErrConsumerStatus::No,  macro_fn: false, version: Version::V02, dispatch: DispatchStatus::ResolvedBuiltin,   section: "§10.1" },
    BuiltinSpec { name: "REVERSE",    signature: "REVERSE(arr) -> ARRAY",                        group: BuiltinGroup::Array,    err_consumer: ErrConsumerStatus::No,  macro_fn: false, version: Version::V02, dispatch: DispatchStatus::ResolvedBuiltin,   section: "§10.1" },

    // ── DICT 操作 (5) ───────────────────────────────────────────
    BuiltinSpec { name: "REMOVE_KEY", signature: "REMOVE_KEY(dict, k) -> DICT",                   group: BuiltinGroup::Dict,     err_consumer: ErrConsumerStatus::No,  macro_fn: false, version: Version::V04, dispatch: DispatchStatus::ResolvedBuiltin,   section: "§10.2" },
    BuiltinSpec { name: "DEL",        signature: "DEL(dict, k) -> DICT (v0.3 alias, W0051)",            group: BuiltinGroup::Dict,     err_consumer: ErrConsumerStatus::No,  macro_fn: false, version: Version::V02, dispatch: DispatchStatus::ResolvedCompat,    section: "§10.2" },
    BuiltinSpec { name: "KEYS",       signature: "KEYS(dict) -> ARRAY",                          group: BuiltinGroup::Dict,     err_consumer: ErrConsumerStatus::No,  macro_fn: false, version: Version::V02, dispatch: DispatchStatus::Deferred,         section: "§10.2" },
    BuiltinSpec { name: "VALUES",     signature: "VALUES(dict) -> ARRAY",                        group: BuiltinGroup::Dict,     err_consumer: ErrConsumerStatus::No,  macro_fn: false, version: Version::V02, dispatch: DispatchStatus::Deferred,         section: "§10.2" },
    BuiltinSpec { name: "HAS",        signature: "HAS(dict, k) -> BOOLEAN",                      group: BuiltinGroup::Dict,     err_consumer: ErrConsumerStatus::No,  macro_fn: false, version: Version::V02, dispatch: DispatchStatus::Deferred,         section: "§10.2" },
    BuiltinSpec { name: "MERGE",      signature: "MERGE(a, b) -> DICT",                          group: BuiltinGroup::Dict,     err_consumer: ErrConsumerStatus::No,  macro_fn: false, version: Version::V02, dispatch: DispatchStatus::Deferred,         section: "§10.2" },

    // ── 下标 (3) ───────────────────────────────────────────────
    BuiltinSpec { name: "INDEX_GET",  signature: "INDEX_GET(coll, k) -> v / E0031",              group: BuiltinGroup::Subscript,err_consumer: ErrConsumerStatus::No,  macro_fn: false, version: Version::V04, dispatch: DispatchStatus::ResolvedBuiltin,   section: "§10.1-§10.2" },
    BuiltinSpec { name: "INDEX_SET",  signature: "INDEX_SET(coll, k, v) -> NULL",                group: BuiltinGroup::Subscript,err_consumer: ErrConsumerStatus::No,  macro_fn: false, version: Version::V04, dispatch: DispatchStatus::ResolvedBuiltin,   section: "§10.1-§10.2" },
    BuiltinSpec { name: "AT",         signature: "AT(coll, i) -> v",                             group: BuiltinGroup::Subscript,err_consumer: ErrConsumerStatus::No,  macro_fn: false, version: Version::V04, dispatch: DispatchStatus::ResolvedBuiltin,   section: "§10.1-§10.2" },

    // ── STRING 操作 (15) ────────────────────────────────────────
    BuiltinSpec { name: "UPPER",          signature: "UPPER(s) -> STRING",                       group: BuiltinGroup::String, err_consumer: ErrConsumerStatus::No,  macro_fn: false, version: Version::V02, dispatch: DispatchStatus::ResolvedBuiltin,   section: "§10.3" },
    BuiltinSpec { name: "LOWER",          signature: "LOWER(s) -> STRING",                       group: BuiltinGroup::String, err_consumer: ErrConsumerStatus::No,  macro_fn: false, version: Version::V02, dispatch: DispatchStatus::ResolvedBuiltin,   section: "§10.3" },
    BuiltinSpec { name: "SUB",            signature: "SUB(s, start, end?) -> STRING",            group: BuiltinGroup::String, err_consumer: ErrConsumerStatus::No,  macro_fn: false, version: Version::V02, dispatch: DispatchStatus::ResolvedBuiltin,   section: "§10.3" },
    BuiltinSpec { name: "REPLACE",        signature: "REPLACE(s, old, new) -> STRING",           group: BuiltinGroup::String, err_consumer: ErrConsumerStatus::No,  macro_fn: false, version: Version::V02, dispatch: DispatchStatus::ResolvedBuiltin,   section: "§10.3" },
    BuiltinSpec { name: "SPLIT",          signature: "SPLIT(s, sep) -> ARRAY",                    group: BuiltinGroup::String, err_consumer: ErrConsumerStatus::No,  macro_fn: false, version: Version::V02, dispatch: DispatchStatus::ResolvedBuiltin,   section: "§10.3" },
    BuiltinSpec { name: "TRIM",           signature: "TRIM(s) -> STRING",                        group: BuiltinGroup::String, err_consumer: ErrConsumerStatus::No,  macro_fn: false, version: Version::V02, dispatch: DispatchStatus::ResolvedBuiltin,   section: "§10.3" },
    BuiltinSpec { name: "TRIM_START",     signature: "TRIM_START(s) -> STRING",                  group: BuiltinGroup::String, err_consumer: ErrConsumerStatus::No,  macro_fn: false, version: Version::V02, dispatch: DispatchStatus::ResolvedBuiltin,   section: "§10.3" },
    BuiltinSpec { name: "TRIM_END",       signature: "TRIM_END(s) -> STRING",                    group: BuiltinGroup::String, err_consumer: ErrConsumerStatus::No,  macro_fn: false, version: Version::V02, dispatch: DispatchStatus::ResolvedBuiltin,   section: "§10.3" },
    BuiltinSpec { name: "STARTS_WITH",    signature: "STARTS_WITH(s, pre) -> BOOLEAN",           group: BuiltinGroup::String, err_consumer: ErrConsumerStatus::No,  macro_fn: false, version: Version::V02, dispatch: DispatchStatus::ResolvedBuiltin,   section: "§10.3" },
    BuiltinSpec { name: "ENDS_WITH",      signature: "ENDS_WITH(s, suf) -> BOOLEAN",             group: BuiltinGroup::String, err_consumer: ErrConsumerStatus::No,  macro_fn: false, version: Version::V02, dispatch: DispatchStatus::ResolvedBuiltin,   section: "§10.3" },
    BuiltinSpec { name: "REPEAT",         signature: "REPEAT(s, n) -> STRING",                   group: BuiltinGroup::String, err_consumer: ErrConsumerStatus::No,  macro_fn: false, version: Version::V02, dispatch: DispatchStatus::ResolvedBuiltin,   section: "§10.3" },
    BuiltinSpec { name: "PAD_START",      signature: "PAD_START(s, n, c?) -> STRING",            group: BuiltinGroup::String, err_consumer: ErrConsumerStatus::No,  macro_fn: false, version: Version::V02, dispatch: DispatchStatus::ResolvedBuiltin,   section: "§10.3" },
    BuiltinSpec { name: "PAD_END",        signature: "PAD_END(s, n, c?) -> STRING",              group: BuiltinGroup::String, err_consumer: ErrConsumerStatus::No,  macro_fn: false, version: Version::V02, dispatch: DispatchStatus::ResolvedBuiltin,   section: "§10.3" },
    BuiltinSpec { name: "CODEPOINTS",     signature: "CODEPOINTS(s) -> ARRAY",                   group: BuiltinGroup::String, err_consumer: ErrConsumerStatus::No,  macro_fn: false, version: Version::V02, dispatch: DispatchStatus::ResolvedBuiltin,   section: "§10.3" },
    BuiltinSpec { name: "FROM_CODEPOINTS",signature: "FROM_CODEPOINTS(arr) -> STRING",            group: BuiltinGroup::String, err_consumer: ErrConsumerStatus::No,  macro_fn: false, version: Version::V02, dispatch: DispatchStatus::ResolvedBuiltin,   section: "§10.3" },

    // ── 格式化 (1) ─────────────────────────────────────────────
    BuiltinSpec { name: "FORMAT",     signature: "FORMAT(template, args...) -> STRING",          group: BuiltinGroup::Format,   err_consumer: ErrConsumerStatus::No,  macro_fn: false, version: Version::V04, dispatch: DispatchStatus::ResolvedBuiltin,   section: "§10.6" },

    // ── 模块系统 (4) ───────────────────────────────────────────
    BuiltinSpec { name: "MODULE_REF", signature: "MODULE_REF(path) -> MODULE",                   group: BuiltinGroup::Module,   err_consumer: ErrConsumerStatus::No,  macro_fn: false, version: Version::V04, dispatch: DispatchStatus::Deferred,         section: "§13.5" },
    BuiltinSpec { name: "EXPORT",     signature: "EXPORT(names) -> NULL",                        group: BuiltinGroup::Module,   err_consumer: ErrConsumerStatus::Na,  macro_fn: true,  version: Version::V02, dispatch: DispatchStatus::LexerMacro,       section: "§13.2" },
    BuiltinSpec { name: "IMPORT",     signature: "IMPORT(path, names, opts?) -> NULL",           group: BuiltinGroup::Module,   err_consumer: ErrConsumerStatus::Na,  macro_fn: true,  version: Version::V02, dispatch: DispatchStatus::LexerMacro,       section: "§13.1" },
    BuiltinSpec { name: "MODULE",     signature: "MODULE(name?, body) -> NULL",                  group: BuiltinGroup::Module,   err_consumer: ErrConsumerStatus::Na,  macro_fn: true,  version: Version::V02, dispatch: DispatchStatus::LexerMacro,       section: "§13.1" },

    // ── OOP (3) ────────────────────────────────────────────────
    BuiltinSpec { name: "CLASS",      signature: "CLASS(name?, parent, members) -> CLASS",       group: BuiltinGroup::Oop,      err_consumer: ErrConsumerStatus::Na,  macro_fn: true,  version: Version::V02, dispatch: DispatchStatus::LexerMacro,       section: "§11.1" },
    BuiltinSpec { name: "NEW",        signature: "NEW(cls, args...) -> INSTANCE",                group: BuiltinGroup::Oop,      err_consumer: ErrConsumerStatus::Na,  macro_fn: true,  version: Version::V02, dispatch: DispatchStatus::LexerMacro,       section: "§11.2" },
    BuiltinSpec { name: "THIS",       signature: "THIS -> 当前实例",                                group: BuiltinGroup::Oop,      err_consumer: ErrConsumerStatus::Na,  macro_fn: true,  version: Version::V02, dispatch: DispatchStatus::LexerMacro,       section: "§11.3" },

    // ── 属性 / 方法 (3) ────────────────────────────────────────
    BuiltinSpec { name: "GET_PROP",   signature: "GET_PROP(obj, k) -> v / E0037",                group: BuiltinGroup::Property, err_consumer: ErrConsumerStatus::No,  macro_fn: false, version: Version::V02, dispatch: DispatchStatus::Deferred,         section: "§11.4" },
    BuiltinSpec { name: "SET_PROP",   signature: "SET_PROP(obj, k, v) -> NULL",                  group: BuiltinGroup::Property, err_consumer: ErrConsumerStatus::No,  macro_fn: false, version: Version::V02, dispatch: DispatchStatus::Deferred,         section: "§11.4" },
    BuiltinSpec { name: "CALL_METHOD",signature: "CALL_METHOD(obj, m, args...) -> v",            group: BuiltinGroup::Property, err_consumer: ErrConsumerStatus::No,  macro_fn: false, version: Version::V02, dispatch: DispatchStatus::Deferred,         section: "§11.4" },

    // ── 构造器 (2) ─────────────────────────────────────────────
    BuiltinSpec { name: "ARRAY",      signature: "ARRAY(items...) / ARRAY()",                   group: BuiltinGroup::Ctor,     err_consumer: ErrConsumerStatus::No,  macro_fn: true,  version: Version::V02, dispatch: DispatchStatus::LexerMacro,       section: "§10.1" },
    BuiltinSpec { name: "DICT",       signature: "DICT(pairs...) / DICT()",                     group: BuiltinGroup::Ctor,     err_consumer: ErrConsumerStatus::No,  macro_fn: true,  version: Version::V02, dispatch: DispatchStatus::LexerMacro,       section: "§10.2" },
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
    out.push_str("> 单源真相是 `crates/wlwl-eval/src/registry.rs::BUILTIN_REGISTRY`,本 markdown 是镜像。\n");
    out.push_str("> 修改流程:改注册表 -> 跑本函数重写本文件 -> 跑 `cargo test` 验证 lock test。\n\n");
    out.push_str("> 对照规范:`docs/standard/wlwl-spec-v0.4*.md` 第 3663 行起 (附录 G 规范性)。\n\n");
    let n_resolved = BUILTIN_REGISTRY
        .iter()
        .filter(|s| matches!(
            s.dispatch,
            DispatchStatus::ResolvedBuiltin | DispatchStatus::ResolvedCompat
        ))
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
    out.push_str("| 名称 | 签名 | ERR 消费者 (§12.7) | 宏函数 (§3.4) | 引入 | 状态 | 实现位置 |\n");
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
            DispatchStatus::ResolvedCompat => format!("`resolve_builtin` (compat, {})", spec.section),
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
        for spec in BUILTIN_REGISTRY.iter().filter(|s| s.dispatch == DispatchStatus::Deferred) {
            by_version.entry(spec.version).or_default().push(spec.name);
        }
        for (v, names) in by_version {
            let _ = writeln!(out, "- **{}** ({} 项): {}\n", v.as_str(), names.len(), names.join(", "));
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
        assert_eq!(original_len, names.len(), "BUILTIN_REGISTRY has duplicate names");
        assert_eq!(BUILTIN_REGISTRY.len(), 90, "expected 90 entries per spec 附录 G + compat aliases");
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
            assert!(md.contains(&format!("`{}`", spec.name)), "missing {} in md", spec.name);
        }
        if !deferred_names().is_empty() {
            assert!(md.contains("## 实现 gap"));
        }
        assert!(md.contains("I/O"));
        assert!(md.contains("控制流"));
        assert!(md.contains("构造器"));
    }
}
