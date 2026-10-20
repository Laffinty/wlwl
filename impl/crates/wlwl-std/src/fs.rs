//! `wlwl:std.fs` — 文本读写 + **路径 / 目录 / 字节**(v0.3 §15.3 · v0.11.3 M6 扩)。
//!
//! Error code mapping (matches spec §14.4) for the **three original** members:
//!   - E0061 — file not found(**读侧**:`READ_FILE` 读不到)
//!   - E0062 — **写侧失败**(`WRITE_FILE` 写不进:父目录不存在 / 权限不足 /
//!     其他写失败);`READ_FILE` 撞上权限不足时也是它
//!   - E0060 — other I/O error (retryable)
//!
//! ⚠️ [F-03] **读侧与写侧是两个入口**(`read_io_error` / `write_io_error`),
//! 不是同一个映射:父目录不存在在 OS 层同样落进 `ErrorKind::NotFound`
//! ⇒ 共用一个映射会把「写不进」报成「文件不存在」(`E0061`)。stdlib §2.0
//! 明写「写失败 → `E0062`」,故**写侧一律 `E0062`**。
//!
//! [v0.11.3 M6 / addendum-06] 新增的 15 个成员走**另一种失败口径**:数据 / 状态
//! 失败是 **`ERR` 值**(`kind = "FsError"` + `op` + `reason`),而**元数 / 类型错**
//! 仍是原生 `E0022` / `E0030`。理由与旧的 E0060–E0062 分开:那三个码是 v0.3
//! 的**原生诊断**口径(终止运行),而「目录不存在」「已存在」这类是**可预期的
//! 运行期结果** —— 调用方多半要 `IS_ERR` 分支处理它(与 `AWAIT` 被取消返回
//! `Err { kind: "Cancelled" }` 同款)。⇒ 两套口径**并存**,不互相取代。
//!
//! [v0.11.3 M6] ⚠️ 本文件里**两种内部风格并存**,是**有意的**:
//! 旧的三个成员走 `crate::compat`(serde_json 边界层,v0.11 M2 的历史产物),
//! 新成员走直通的 `wlwl_value::Value`(与 `time.rs` / `rand.rs` 同款)。
//! 路径 / 目录 / 字节是**性能敏感面**(一次 `LIST_DIR` 可能上千个条目、一次
//! `READ_BYTES` 可能上万个整数),不该为它们多绕一层 serde_json。

use crate::compat::*;
use crate::{ModuleSpec, StdCtx, StdFn};
use std::io::ErrorKind;
use wlwl_error::ErrorCode;
use wlwl_error::WlwlError;
use wlwl_value::{Outcome, StdHost, Value};

pub(super) fn std_read_file_inner(
    _ctx: &mut StdCtx,
    args: Vec<StdValue>,
) -> Result<StdValue, StdError> {
    let path = expect_string("READ_FILE", &args, 0, 1)?;
    match std::fs::read_to_string(path) {
        Ok(content) => Ok(StdValue::String(content)),
        Err(e) => Err(read_io_error("READ_FILE", e, path)),
    }
}

pub(super) fn std_write_file_inner(
    _ctx: &mut StdCtx,
    args: Vec<StdValue>,
) -> Result<StdValue, StdError> {
    if args.len() != 2 {
        return Err(arity_error("WRITE_FILE", args.len(), 2));
    }
    let path = expect_string("WRITE_FILE", &args, 0, 2)?;
    let content = match &args[1] {
        StdValue::String(s) => s.clone(),
        other => return Err(type_error("WRITE_FILE", "string", other)),
    };
    match std::fs::write(path, &content) {
        Ok(()) => Ok(StdValue::Null),
        Err(e) => Err(write_io_error("WRITE_FILE", e, path)),
    }
}

pub(super) fn std_exists_inner(
    _ctx: &mut StdCtx,
    args: Vec<StdValue>,
) -> Result<StdValue, StdError> {
    let path = expect_string("EXISTS", &args, 0, 1)?;
    Ok(StdValue::Bool(std::fs::metadata(path).is_ok()))
}

/// Map a `std::io::Error` from **`READ_FILE`** to the v0.3 §14.4 IO error
/// codes. `E0060` is the catch-all; `E0061` for NotFound; `E0062` for
/// PermissionDenied.
///
/// ⚠️ **读侧口径一字未动**(F-03 只拆映射,不改读侧语义):「文件不存在 →
/// `E0061`」是 v0.3 冻结契约(语言规范 §11.2)。
fn read_io_error(fn_name: &str, e: std::io::Error, path: &str) -> StdError {
    let (code, label) = match e.kind() {
        ErrorKind::NotFound => (ErrorCode::E0061, "file not found"),
        ErrorKind::PermissionDenied => (ErrorCode::E0062, "permission denied"),
        _ => (ErrorCode::E0060, "I/O error"),
    };
    StdError {
        code,
        message: format!("{}: {} ({}): {}", fn_name, label, path, e),
    }
}

/// Map a `std::io::Error` from **`WRITE_FILE`** to the v0.3 §14.4 IO error
/// codes —— **写失败一律 `E0062`**(stdlib §2.0:「`WRITE_FILE` … 写失败 →
/// `E0062`」)。
///
/// ⚠️ **为什么写侧不复用读侧的映射**(F-03 的根因):父目录不存在在 OS 层
/// 同样是 `ErrorKind::NotFound` ⇒ 共用映射会把「写不进」报成「文件不存在」
/// (`E0061`),调用方按码分流时**分不出「读不到」与「写不进」**。故两侧各
/// 一个入口:读侧 `E0061` = 文件不存在,写侧 `E0062` = 写失败。
///
/// 消息按 kind 分三种(语义必须分得开):
/// - `parent directory does not exist` —— 父目录不存在(写侧最常见的一种);
/// - `permission denied` —— 权限不足;
/// - `<os error>` —— 其余写失败(路径是目录、只读卷、磁盘满 …),把 OS 原话
///   带出来,别把它压成一句「写失败」。
fn write_io_error(fn_name: &str, e: std::io::Error, path: &str) -> StdError {
    let reason = match e.kind() {
        ErrorKind::NotFound => "parent directory does not exist".to_string(),
        ErrorKind::PermissionDenied => "permission denied".to_string(),
        _ => e.to_string(),
    };
    StdError {
        code: ErrorCode::E0062,
        message: format!("{fn_name}: cannot write ({path}): {reason}"),
    }
}

pub static SPEC: ModuleSpec = ModuleSpec {
    path: "wlwl:std.fs",
    functions: &[
        // ── 三个 v0.3 原有成员(契约冻结,签名与语义一字未动)──────────
        ("READ_FILE", std_read_file as StdFn),
        ("WRITE_FILE", std_write_file as StdFn),
        ("EXISTS", std_exists as StdFn),
        // ── v0.11.3 M6 / addendum-06 新增 ────────────────────────────
        ("PATH_JOIN", path_join as StdFn),
        ("PATH_DIR", path_dir as StdFn),
        ("PATH_BASE", path_base as StdFn),
        ("PATH_EXT", path_ext as StdFn),
        ("PATH_NORMALIZE", path_normalize as StdFn),
        ("LIST_DIR", list_dir as StdFn),
        ("WALK_DIR", walk_dir as StdFn),
        ("MKDIR", mkdir as StdFn),
        ("MKDIRS", mkdirs as StdFn),
        ("REMOVE", remove as StdFn),
        ("COPY", copy as StdFn),
        ("MOVE", move_path as StdFn),
        ("FILE_SIZE", file_size as StdFn),
        ("READ_BYTES", read_bytes as StdFn),
        ("WRITE_BYTES", write_bytes as StdFn),
    ],
};

/// [v0.11 M2 / ADR-0022] 直通边界包装:Value→内部表示→Value。
pub fn std_read_file(host: &mut dyn StdHost, args: Vec<Value>) -> Result<Outcome, WlwlError> {
    crate::wrap(host, std_read_file_inner, args)
}

/// [v0.11 M2 / ADR-0022] 直通边界包装:Value→内部表示→Value。
pub fn std_write_file(host: &mut dyn StdHost, args: Vec<Value>) -> Result<Outcome, WlwlError> {
    crate::wrap(host, std_write_file_inner, args)
}

/// [v0.11 M2 / ADR-0022] 直通边界包装:Value→内部表示→Value。
pub fn std_exists(host: &mut dyn StdHost, args: Vec<Value>) -> Result<Outcome, WlwlError> {
    crate::wrap(host, std_exists_inner, args)
}

// ═══════════════════════════════════════════════════════════════════════
// v0.11.3 M6 / addendum-06 —— 路径 / 目录 / 字节
// ═══════════════════════════════════════════════════════════════════════

// ── 分隔符口径(业主 2026-10-08 裁决「甲」)─────────────────────────────
//
// **规范分隔符恒为 `/`**。理由不是「统一好看」,是**平台原生形态让契约表
// 无法逐字冻结**:`PATH_JOIN("a","b")` 若用平台分隔符,Windows 返 `a\b`、
// Linux 返 `a/b` ⇒ 同一份程序跨平台产出不同字符串,而逐字节冻结是本仓
// 所有契约表的立足点。
//
// **输入端 Windows 额外容忍 `\`** —— 那是 `std.fs` 三个原有成员**既有的**
// 行为(它们把 path 原样交给 Rust),收掉就是破坏性变更。
// ⚠️ **POSIX 上不做这个替换**:`\` 在 POSIX 里是**合法文件名字符**,把它换成
// `/` 会让一个真实存在的文件「找不到」。

/// 规范路径(恒 `/`)→ 平台原生路径。
fn native(p: &str) -> std::path::PathBuf {
    if cfg!(windows) {
        std::path::PathBuf::from(p.replace('\\', "/"))
    } else {
        std::path::PathBuf::from(p)
    }
}

/// 平台原生路径 → 规范路径(恒 `/`)。
///
/// ⚠🔴 **[D13-044 / N-030] 这里必须与 `native()` 用**同一套**平台判定。**
/// 原实现是无条件 `replace('\\', "/")`,于是 POSIX 上一个**合法的**文件名
/// `a\b.txt` 被吐成 `a/b.txt` —— 而这个字符串再喂回 `READ_FILE` / `MOVE` /
/// `REMOVE` 指向的是**另一个路径**:一次**不可逆的往返破坏**(丢的是名字,
/// 没有任何补救余地)。方向上的不对称也在:输入端**已经**按平台判定过
/// (`native`),出参端却不是 ⇒ 同一个文件里两套口径。
///
/// ⚠ Windows 上**保留**替换:那里 `\` 是分隔符、文件名里不可能出现 `\` ⇒
/// 替换无损,「规范分隔符恒 `/`」那条裁决继续成立。
/// ⚠ **POSIX 侧未经实测**:本轮开发机是 Windows,这条是按代码分支推理 +
/// `native()` 的既有口径推出的,需要在 Linux / macOS 上补一次实测。
fn spec(p: &std::path::Path) -> String {
    let s = p.to_string_lossy();
    if cfg!(windows) {
        s.replace('\\', "/")
    } else {
        s.into_owned()
    }
}

// ── 失败口径(新成员)────────────────────────────────────────────────

/// `ERR([kind: "FsError", op, reason])` —— **值**,不是原生诊断。
///
/// 为什么新成员不用旧三个的 `E0060`–`E0062`:那三个是 v0.3 的**原生诊断**口径
/// (终止运行);而「目录不存在」「已存在」是**可预期的运行期结果**,调用方多半要
/// `IS_ERR` 分支(与 `AWAIT` 被取消返回 `Err { kind: "Cancelled" }` 同款)。
/// ⇒ 两套口径并存,新成员用后者。
fn fs_err(op: &str, reason: impl Into<String>) -> Value {
    Value::Err(Box::new(Value::Dict(vec![
        (
            Value::String("kind".into()),
            Value::String("FsError".into()),
        ),
        (Value::String("op".into()), Value::String(op.into())),
        (Value::String("reason".into()), Value::String(reason.into())),
    ])))
}

fn fs_arity(host: &mut dyn StdHost, name: &str, got: usize, want: usize) -> WlwlError {
    host.diag(
        ErrorCode::E0022,
        format!("{name}: function expects {want} argument(s), got {got}"),
    )
}

fn fs_type(host: &mut dyn StdHost, name: &str, want: &str, got: &Value) -> WlwlError {
    host.diag(
        ErrorCode::E0030,
        format!("{name}: expected {want}, got {}", crate::value_kind(got)),
    )
}

fn str_arg(host: &mut dyn StdHost, name: &str, v: &Value) -> Result<String, WlwlError> {
    match v {
        Value::String(s) => Ok(s.clone()),
        other => Err(fs_type(host, name, "a string", other)),
    }
}

fn ok(v: Value) -> Result<Outcome, WlwlError> {
    Ok(Outcome::normal(v))
}

fn strings(vs: Vec<String>) -> Value {
    Value::Array(vs.into_iter().map(Value::String).collect())
}

// ── 路径拆解(纯函数,全量表驱动)────────────────────────────────────

/// `PATH_JOIN(parts...) -> STRING`
///
/// 变参(≥1),全部实参必须是字符串;用 `/` 连接并**合并重复分隔符**。
/// 空片段被跳过(`PATH_JOIN("a", "", "b") = "a/b"`)—— 让调用方能用
/// `FILTER(MAP(...), 空则丢弃)` 的结果直接拼。
pub fn path_join(host: &mut dyn StdHost, args: Vec<Value>) -> Result<Outcome, WlwlError> {
    const NAME: &str = "PATH_JOIN";
    if args.is_empty() {
        return Err(fs_arity(host, NAME, 0, 1));
    }
    let mut out = String::new();
    for a in &args {
        let p = str_arg(host, NAME, a)?;
        for seg in p.split(['/', '\\']) {
            if seg.is_empty() {
                continue;
            }
            if !out.is_empty() {
                out.push('/');
            }
            out.push_str(seg);
        }
    }
    ok(Value::String(out))
}

/// `PATH_DIR(p) -> STRING` —— 父目录;**无父目录时返回 `""`**(不是 `"."`)。
///
/// 选 `""` 而不是 Go 的 `"."`:本语言里 `PATH_JOIN(PATH_DIR(x), PATH_BASE(x))`
/// 是取回 `x` 的惯用写法,`""` 让它成立而不必特判。
pub fn path_dir(host: &mut dyn StdHost, args: Vec<Value>) -> Result<Outcome, WlwlError> {
    const NAME: &str = "PATH_DIR";
    if args.len() != 1 {
        return Err(fs_arity(host, NAME, args.len(), 1));
    }
    let p = str_arg(host, NAME, &args[0])?;
    let trimmed = p.trim_end_matches(['/', '\\']);
    let dir = match trimmed.rfind(['/', '\\']) {
        None => String::new(),
        Some(0) => "/".to_string(),
        Some(i) => trimmed[..i].to_string(),
    };
    ok(Value::String(dir))
}

/// `PATH_BASE(p) -> STRING` —— 最后一段。**不剥扩展名**(`a.tar.gz` → `a.tar.gz`)。
pub fn path_base(host: &mut dyn StdHost, args: Vec<Value>) -> Result<Outcome, WlwlError> {
    const NAME: &str = "PATH_BASE";
    if args.len() != 1 {
        return Err(fs_arity(host, NAME, args.len(), 1));
    }
    let p = str_arg(host, NAME, &args[0])?;
    let trimmed = p.trim_end_matches(['/', '\\']);
    let base = match trimmed.rfind(['/', '\\']) {
        Some(i) => &trimmed[i + 1..],
        None => trimmed,
    };
    ok(Value::String(base.to_string()))
}

/// `PATH_EXT(p) -> STRING` —— 扩展名,**不含前导点**(业主 2026-10-08 裁决)。
///
/// 两条边界:
/// - **无扩展名 → `""`**(不是 `NULL`);
/// - **点文件没有扩展名**:`.gitignore` → `""`。点必须在文件名里**不是第一个
///   字符**才算扩展名的起点,否则 `PATH_BASE` + `PATH_EXT` 拼不回原名。
///
/// 「要带点的原样拼回去」是调用方一行的事,且与 `PATH_BASE` 的处理不对称
/// (base 不剥扩展名),所以这里从简。
pub fn path_ext(host: &mut dyn StdHost, args: Vec<Value>) -> Result<Outcome, WlwlError> {
    const NAME: &str = "PATH_EXT";
    if args.len() != 1 {
        return Err(fs_arity(host, NAME, args.len(), 1));
    }
    let p = str_arg(host, NAME, &args[0])?;
    let base = match p.rfind(['/', '\\']) {
        Some(i) => &p[i + 1..],
        None => p.as_str(),
    };
    let ext = match base.rfind('.') {
        Some(i) if i > 0 => &base[i + 1..],
        _ => "",
    };
    ok(Value::String(ext.to_string()))
}

/// `PATH_NORMALIZE(p) -> STRING` —— 消 `.` / `..` / 冗余分隔符。**不碰符号链接**。
///
/// ⚠️ 纯**词法**归一化:它**不知道** `a/../b` 里的 `a` 是不是符号链接,所以
/// 「消 `..`」可能与真实文件系统不一致。这是刻意的 —— 一个会去 `canonicalize`
/// 的 `NORMALIZE` 就有 I/O 副作用、且在路径不存在时行为突变,那不是「归一化」。
/// 需要真实解析用 `EXISTS` / `LIST_DIR`。
pub fn path_normalize(host: &mut dyn StdHost, args: Vec<Value>) -> Result<Outcome, WlwlError> {
    const NAME: &str = "PATH_NORMALIZE";
    if args.len() != 1 {
        return Err(fs_arity(host, NAME, args.len(), 1));
    }
    let p = str_arg(host, NAME, &args[0])?;
    // Windows 盘符 `C:` 是路径的**一部分**,不能当普通片段处理。
    let (prefix, rest) = split_drive(&p);
    let absolute = rest.starts_with('/');
    let mut out: Vec<&str> = Vec::new();
    for seg in rest.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                if matches!(out.last(), Some(&"..")) || out.is_empty() {
                    if !absolute {
                        out.push("..");
                    }
                    // 绝对路径下 `/..` 仍是 `/` ⇒ 直接弹掉(没有可弹的就不动)
                } else {
                    out.pop();
                }
            }
            s => out.push(s),
        }
    }
    let body = out.join("/");
    let joined = if absolute { format!("/{body}") } else { body };
    ok(Value::String(format!("{prefix}{joined}")))
}

/// 拆出 Windows 盘符前缀(`C:` / `C:/`);POSIX 下永远是空串。
fn split_drive(p: &str) -> (String, &str) {
    let b = p.as_bytes();
    if b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':' {
        // 只在 `:` 后面紧跟 `/`、分隔符或字符串结尾时才算盘符(否则 `a:b` 是文件名)
        if b.len() == 2 || b[2] == b'/' {
            return (p[..2].to_string(), &p[2..]);
        }
    }
    (String::new(), p)
}

// ── 目录 ───────────────────────────────────────────────────────────

/// `LIST_DIR(p) -> ARRAY[STRING]`
///
/// 列目录(不递归),**结果排序**(确定性是本语言卖点第一条,OS 的目录迭代序
/// **逐次可能不同**)。
///
/// ⚠️ **符号链接**:本成员**列出**符号链接条目(你有权知道它存在),但**不会
/// 跟随**它 —— 它本来就不递归。「默认不跟随」这条裁决真正约束的是 `WALK_DIR`。
pub fn list_dir(host: &mut dyn StdHost, args: Vec<Value>) -> Result<Outcome, WlwlError> {
    const NAME: &str = "LIST_DIR";
    if args.len() != 1 {
        return Err(fs_arity(host, NAME, args.len(), 1));
    }
    let p = str_arg(host, NAME, &args[0])?;
    let rd = match std::fs::read_dir(native(&p)) {
        Ok(rd) => rd,
        Err(e) => return ok(fs_err(NAME, io_reason(&p, &e))),
    };
    let mut out = Vec::new();
    for entry in rd {
        match entry {
            Ok(e) => out.push(spec(&e.path())),
            Err(e) => return ok(fs_err(NAME, e.to_string())),
        }
    }
    out.sort();
    ok(strings(out))
}

/// `WALK_DIR(p, pattern?, follow?) -> ARRAY[STRING]`
///
/// 递归遍历,返回**相对 `p` 的路径**(规范分隔符 `/`),**排序**。
///
/// - `pattern`:**glob** —— `*` 不跨分隔符、`**` 跨目录、`?` 匹配单个字符
///   (业主 2026-10-08 裁决;与 `std.regex` 互补,正则已有 `RE`)。pattern **含 `/`
///   时匹配整条相对路径,否则只匹配文件名**。缺省 = 匹配全部。
/// - `follow`:**默认 `FALSE`** —— **不下降进符号链接目录**(成环的链接目录会让
///   递归永不终止)。置 `TRUE` 时**下降进**符号链接目录,并记 **visited 集合**
///   (按 `canonicalize` 的目录)防环。⚠ 这条在 **POSIX 与 Windows 上语义相同**
///   ——`[N-010 / D13-060]` 修之前它在 POSIX 上是**死参数**(见 `walk` 里的注)。
///
/// ⚠ **失败口径**:`p` **本身**打不开(不存在 / 不是目录 / 没权限)⇒
/// `ERR([kind: "FsError", op, reason])`,**与 `LIST_DIR` 同一口径**
/// (D13-045)。⚠ 递归途中某个**子**目录打不开则静默跳过 —— 那是合理的
/// (权限、并发删除),不该让整次遍历因一个坏子目录而无产出。
pub fn walk_dir(host: &mut dyn StdHost, args: Vec<Value>) -> Result<Outcome, WlwlError> {
    const NAME: &str = "WALK_DIR";
    if args.is_empty() || args.len() > 3 {
        return Err(fs_arity(host, NAME, args.len(), 2));
    }
    let root = str_arg(host, NAME, &args[0])?;
    let pattern = match args.get(1) {
        None | Some(Value::Null) => None,
        Some(v) => Some(str_arg(host, NAME, v)?),
    };
    let follow = match args.get(2) {
        None | Some(Value::Null) => false,
        Some(Value::Boolean(b)) => *b,
        Some(other) => return Err(fs_type(host, NAME, "a boolean", other)),
    };
    let base = native(&root);
    let mut out: Vec<String> = Vec::new();
    let mut visited: Vec<std::path::PathBuf> = Vec::new();
    // ⚠🔴 **[D13-045 / N-031] 根目录打不开 ⇒ `ERR`,与 `LIST_DIR` 对齐。**
    // 顶层原本忽略 `walk` 的全部产出 ⇒ **根不存在**与**根是个空目录**返回
    // 完全相同的 `[]`,调用方无从分辨(而 `LIST_DIR` 对同一情形是响的,
    // 同一个模块里两套失败口径)。
    // ⚠ 递归途中某个**子**目录打不开仍然静默跳过 —— 那是合理的(权限、并发
    // 删除),不该让整次遍历因一个坏子目录而无产出。这两件事必须分开。
    if let Err(e) = walk(
        &base,
        "",
        pattern.as_deref(),
        follow,
        &mut out,
        &mut visited,
    ) {
        return ok(fs_err(NAME, io_reason(&root, &e)));
    }
    out.sort();
    ok(strings(out))
}

/// 返回 `Err` **只**代表「`dir` 打不开」,且只有**根**会走到那条路 ——
/// `rel == ""` 只可能来自顶层调用(递归层的 `rel` 恒非空)。
fn walk(
    dir: &std::path::Path,
    rel: &str,
    pattern: Option<&str>,
    follow: bool,
    out: &mut Vec<String>,
    chain: &mut Vec<std::path::PathBuf>,
) -> std::io::Result<()> {
    // 🔴🔴 [D13-065 / CI 抓到] 防环集合必须是**当前递归链上的祖先集合**,
    // 不是「本轮见过的所有目标」。
    //
    // ⚠⚠ 此前它是 `visited`(全局已见集合,键 = `canonicalize()` 的**目标**),
    //   于是 **`link -> real` 这种「同目标别名」被当成「环」**:只要 `real` 先
    //   被遍历到,`link` 就被跳过 —— 而它**根本不是环**,是另一条合法路径。
    //
    // ⚠ **这就是 CI 连红三次的原因**(2026-10-19,run 236/237/238,**只有
    //   macOS 失败**):`std::fs::read_dir` 的**迭代顺序未定义**,Linux 上
    //   恰好先返回 `link` ⇒ 断言通过;macOS 上先返回 `real` ⇒ 输出只有
    //   `[real/inside.txt]`。
    //   ⇒ **ubuntu 当初「验证通过」是碰巧,不是行为正确** —— 这条更正比
    //   修复本身更值得记:单平台绿灯证明不了顺序相关的性质。
    //
    // 正确口径(`find -L` 同款):**只有回到祖先才是环**。指向别处的链接是
    // 合法别名,照常展开 —— 于是 `real` 与 `link` 两条路径**都**出现在结果里。
    let canon = if follow {
        dir.canonicalize().ok()
    } else {
        None
    };
    if let Some(c) = canon.as_ref() {
        if chain.contains(c) {
            // 回到祖先 = 环。未 push,所以这里不需要 pop。
            return Ok(());
        }
    }
    if let Some(c) = canon.as_ref() {
        chain.push(c.clone());
    }
    let r = walk_body(dir, rel, pattern, follow, out, chain);
    if canon.is_some() {
        chain.pop();
    }
    r
}

/// [`walk`] 的循环体。与防环判定分开,是为了让「入栈 / 出栈」这对状态操作
/// 只有**一处**,不会被某个 `return` 漏掉 pop。
fn walk_body(
    dir: &std::path::Path,
    rel: &str,
    pattern: Option<&str>,
    follow: bool,
    out: &mut Vec<String>,
    chain: &mut Vec<std::path::PathBuf>,
) -> std::io::Result<()> {
    let rd = match std::fs::read_dir(dir) {
        Ok(rd) => rd,
        // 根:冒泡(见 D13-045)。
        Err(e) if rel.is_empty() => return Err(e),
        // 子目录:静默跳过(见 D13-045)。
        Err(_) => return Ok(()),
    };
    for entry in rd.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        let child_rel = if rel.is_empty() {
            name.clone()
        } else {
            format!("{rel}/{name}")
        };
        // `symlink_metadata` **不跟随**符号链接 ⇒ 用它判断「它是不是链接」。
        let is_link = entry
            .path()
            .symlink_metadata()
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false);
        // ⚠🔴 **[N-010 / D13-060] 判「能不能下降」在 `follow = TRUE` 时必须用
        // **跟随**语义。**
        // `DirEntry::file_type()` 的契约是「**不**跟随符号链接」
        // (Rust `std::fs::DirEntry::file_type`:"This function will not traverse
        // symlinks if this entry points at a symlink")⇒ 在 **POSIX** 上「符号链接
        // → 目录」这项的 `is_dir()` **恒 `false`** ⇒ 下面 `!is_link || follow`
        // 那条**放行分支永远到不了** ⇒ `follow` 在 POSIX 上是**死参数**。
        // ⚠ **同一段代码在两个平台上语义分叉**:Windows 上目录符号链接带
        // `FILE_ATTRIBUTE_DIRECTORY` 位,`file_type().is_dir()` 是 **true**
        // ⇒ `follow` 照常生效。⇒ 同一份代码、同一个参数,Windows 有效、
        // POSIX 无效,而**两边都不报错**。
        // 修法:`follow` 为真时改用 `Path::is_dir()`(跟随语义)。
        // ⚠⚠ **非 follow 分支绝不能一起改** —— 那会让 `WALK_DIR(follow = FALSE)`
        // **开始跟随链接**,那是**语义变更**(成环的链接目录会让递归不止),
        // 不是 bug 修复。`follow` 的默认值就是 `FALSE`(见 `walk_dir` 签名),
        // 改错方向会把默认行为整个翻过来。
        let is_dir = if follow {
            // 跟随语义:`stat`(走完链接)。
            path.is_dir()
        } else {
            // 非跟随语义:`lstat`(链接就是链接)。
            entry.file_type().map(|t| t.is_dir()).unwrap_or(false)
        };
        if glob_hit(pattern, &child_rel) {
            out.push(child_rel.clone());
        }
        // ⚠ 这行的 `!is_link || follow` 在 `follow = TRUE` 时恒真(那正是
        // 上面换判定的原因);`follow = FALSE` 时它才是**唯一**把链接目录挡在
        // 门外的守卫 —— Windows 上目录链接的 `is_dir` 是 true,挡它的只有这里。
        if is_dir && (!is_link || follow) {
            // `?` 只可能拿到子目录的错误,而子目录的错误在上面已被吞成
            // `Ok(())` ⇒ 这里不会把「某个坏子目录」升级成整次遍历的失败。
            walk(&path, &child_rel, pattern, follow, out, chain)?;
        }
    }
    Ok(())
}

/// pattern 是否命中这条相对路径。`None` = 全中。
fn glob_hit(pattern: Option<&str>, rel: &str) -> bool {
    let Some(p) = pattern else { return true };
    // 含 `/` 匹配整条相对路径,否则只匹配文件名 —— 与 `git ls-files` / fnmatch 同款。
    let text = if p.contains('/') { rel } else { base_name(rel) };
    glob(p, text)
}

fn base_name(rel: &str) -> &str {
    match rel.rfind('/') {
        Some(i) => &rel[i + 1..],
        None => rel,
    }
}

/// glob 匹配:**段级** `**` + **段内** `*` / `?`。
fn glob(pattern: &str, text: &str) -> bool {
    let pat: Vec<&str> = pattern.split('/').collect();
    let txt: Vec<&str> = text.split('/').collect();
    glob_segments(&pat, &txt)
}

fn glob_segments(pat: &[&str], txt: &[&str]) -> bool {
    let Some(&head) = pat.first() else {
        return txt.is_empty();
    };
    if head == "**" {
        // `**` 匹配**零个或多个**整段
        (0..=txt.len()).any(|i| glob_segments(&pat[1..], &txt[i..]))
    } else {
        match txt.first() {
            None => false,
            Some(t) => wild(head.as_bytes(), t.as_bytes()) && glob_segments(&pat[1..], &txt[1..]),
        }
    }
}

/// 段内匹配:`*` 任意长(**不跨 `/`,因为段里本就没有 `/`**)、`?` 恰好一个字符。
fn wild(pat: &[u8], txt: &[u8]) -> bool {
    match pat.first() {
        None => txt.is_empty(),
        Some(b'*') => {
            // 合并连续的 `*` 后回溯
            let rest = &pat[1..];
            wild(rest, txt) || (0..txt.len()).any(|i| wild(rest, &txt[i + 1..]))
        }
        Some(&c) => !txt.is_empty() && (c == b'?' || c == txt[0]) && wild(&pat[1..], &txt[1..]),
    }
}

/// `MKDIR(p) -> NULL` —— 建**单层**目录;**已存在 → `ERR`**(业主 2026-10-08 裁决)。
///
/// 静默吞掉「已存在」会掩盖竞态(另一个进程刚建了它),所以这里响。
/// 要幂等语义用 `MKDIRS`。
pub fn mkdir(host: &mut dyn StdHost, args: Vec<Value>) -> Result<Outcome, WlwlError> {
    const NAME: &str = "MKDIR";
    if args.len() != 1 {
        return Err(fs_arity(host, NAME, args.len(), 1));
    }
    let p = str_arg(host, NAME, &args[0])?;
    match std::fs::create_dir(native(&p)) {
        Ok(()) => ok(Value::Null),
        Err(e) => ok(fs_err(NAME, io_reason(&p, &e))),
    }
}

/// `MKDIRS(p) -> NULL` —— 建**多级**目录,等价 `mkdir -p`:**已存在即成功**。
pub fn mkdirs(host: &mut dyn StdHost, args: Vec<Value>) -> Result<Outcome, WlwlError> {
    const NAME: &str = "MKDIRS";
    if args.len() != 1 {
        return Err(fs_arity(host, NAME, args.len(), 1));
    }
    let p = str_arg(host, NAME, &args[0])?;
    match std::fs::create_dir_all(native(&p)) {
        Ok(()) => ok(Value::Null),
        Err(e) => ok(fs_err(NAME, io_reason(&p, &e))),
    }
}

/// `REMOVE(p, recursive?) -> NULL` —— 删文件 / 删目录。
///
/// 目录且**未**带 `recursive = TRUE` 时用 `remove_dir`(非空即失败)—— 不静默删树。
pub fn remove(host: &mut dyn StdHost, args: Vec<Value>) -> Result<Outcome, WlwlError> {
    const NAME: &str = "REMOVE";
    if args.is_empty() || args.len() > 2 {
        return Err(fs_arity(host, NAME, args.len(), 1));
    }
    let p = str_arg(host, NAME, &args[0])?;
    let recursive = match args.get(1) {
        None | Some(Value::Null) => false,
        Some(Value::Boolean(b)) => *b,
        Some(other) => return Err(fs_type(host, NAME, "a boolean", other)),
    };
    let native_p = native(&p);
    let is_dir = native_p.is_dir();
    let r = if is_dir && recursive {
        std::fs::remove_dir_all(&native_p)
    } else if is_dir {
        std::fs::remove_dir(&native_p)
    } else {
        std::fs::remove_file(&native_p)
    };
    match r {
        Ok(()) => ok(Value::Null),
        Err(e) => ok(fs_err(NAME, io_reason(&p, &e))),
    }
}

/// `COPY(src, dst) -> NULL` —— 复制**文件**(目录复制**不做**,见规范)。
pub fn copy(host: &mut dyn StdHost, args: Vec<Value>) -> Result<Outcome, WlwlError> {
    const NAME: &str = "COPY";
    if args.len() != 2 {
        return Err(fs_arity(host, NAME, args.len(), 2));
    }
    let src = str_arg(host, NAME, &args[0])?;
    let dst = str_arg(host, NAME, &args[1])?;
    let s = native(&src);
    if s.is_dir() {
        return ok(fs_err(
            NAME,
            format!("{src} is a directory — COPY handles files only"),
        ));
    }
    match std::fs::copy(s, native(&dst)) {
        Ok(_) => ok(Value::Null),
        Err(e) => ok(fs_err(NAME, io_reason(&src, &e))),
    }
}

/// `MOVE(src, dst) -> NULL` —— 移动 / 重命名。
///
/// 跨设备时 `rename` 会失败(那是 OS 事实,不是缺陷)⇒ **回退**到「复制 + 删除」,
/// 否则「移动到另一个盘」会变成一条只有报错的功能。
pub fn move_path(host: &mut dyn StdHost, args: Vec<Value>) -> Result<Outcome, WlwlError> {
    const NAME: &str = "MOVE";
    if args.len() != 2 {
        return Err(fs_arity(host, NAME, args.len(), 2));
    }
    let src = str_arg(host, NAME, &args[0])?;
    let dst = str_arg(host, NAME, &args[1])?;
    let s = native(&src);
    let d = native(&dst);
    if std::fs::rename(&s, &d).is_ok() {
        return ok(Value::Null);
    }
    // 回退:目录树 / 跨设备 ⇒ 递归复制再删源。
    if s.is_dir() {
        let mut chain: Vec<std::path::PathBuf> = Vec::new();
        match copy_tree(&s, &d, &mut chain) {
            Ok(()) => match std::fs::remove_dir_all(&s) {
                Ok(()) => ok(Value::Null),
                Err(e) => ok(fs_err(
                    NAME,
                    format!("copied to {dst} but removing {src} failed: {e}"),
                )),
            },
            Err(e) => ok(fs_err(NAME, e)),
        }
    } else {
        match std::fs::copy(&s, &d) {
            Ok(_) => match std::fs::remove_file(&s) {
                Ok(()) => ok(Value::Null),
                Err(e) => ok(fs_err(
                    NAME,
                    format!("copied to {dst} but removing {src} failed: {e}"),
                )),
            },
            Err(e) => ok(fs_err(NAME, io_reason(&src, &e))),
        }
    }
}

/// 跨设备回退(`MOVE`)用的递归复制。
///
/// ⚠🔴 **[D13-046 / N-032] 符号链接 + 防环。** 原实现无 `visited`、且用
/// `Path::is_dir()` 判递归 —— 而 `is_dir()` 走 `metadata()`、**跟随**符号链接
/// ⇒ 一个指向自身祖先的目录链接会无限递归打挂。
///
/// ⚠🔴 **[D13-066] 防环集合必须是「当前递归链上的祖先」,不是「全局已见」。**
/// 同模块的 `walk` 犯过一模一样的错(`D13-065`,CI 连红三轮、只有 macOS),
/// 这次是**第二份副本**。判据只有一条:**回到祖先才是环**;指向别处的链接是
/// 合法别名,必须照常展开(`find -L` / `cp -rL` 同款)。
///
/// ⚠ **与 `walk` 的缺陷形态不同,别照抄那条描述。** `walk` 命中环是**静默跳过**
/// ⇒ 表现为「少一条路径」,**顺序相关**。本函数命中环是**硬 ERR**,而「同目标
/// 别名」在**两个遍历顺序下都失败**(第二个被遍历到的那个必然命中)⇒ **这是
/// 确定性失败,不是顺序相关的缺陷**。顺序只影响**失败前已经写出去多少**
/// —— 即失败后目标目录会**留在那儿且是半份的**,源目录不动。
///
/// 命中环 **返回 `ERR`** 而不是「跳过」—— 跳过会让 `MOVE` 紧接着的
/// `remove_dir_all(src)` 把那个链接删掉,即**搬完少了一个链接**,那比响更坏。
///
/// - 指向文件的链接仍按 `std::fs::copy` 展开(跟随),与原行为一致;
/// - **不做**「把链接原样重建」:Windows 上建符号链接要特权 / 开发者模式,
///   而跨设备重建的语义也不明确。
///
/// ⚠ **「链接成环未经实测」这个保留已撤销**(`D13-066`):Windows 的**目录联接
/// (junction)不需要管理员**,且 `canonicalize()` / `is_dir()` 都照样解析它
/// ⇒ 成环与别名两条路径**本机就能真跑**,不必再依赖 POSIX 平台。
fn copy_tree(
    src: &std::path::Path,
    dst: &std::path::Path,
    chain: &mut Vec<std::path::PathBuf>,
) -> Result<(), String> {
    // ⚠ 防环检查**放在**展开之前:成环的判定必须先于 `create_dir_all`,
    // 否则环上的每一层都会先在目标侧建出目录。
    let canon = src
        .canonicalize()
        .map_err(|e| format!("{}: cannot resolve for the cycle check: {e}", src.display()))?;
    if chain.contains(&canon) {
        return Err(format!(
            "{}: symlink cycle — MOVE cannot copy this tree across devices",
            src.display()
        ));
    }
    // ⚠⚠ **`D13-066`:** `push` 之后必须在函数出口 `pop` —— 集合是**祖先链**,
    // 不是「本次调用见过的所有目录」。少了这个 `pop`,两个**兄弟**路径指向
    // 同一目录(别名)时,第二个就会被当成环。
    chain.push(canon);
    let r = copy_tree_body(src, dst, chain);
    chain.pop();
    r
}

/// [`copy_tree`] 的循环体。与防环判定拆开,是为了让「入栈 / 出栈」这对状态
/// 操作**只有一处**,不会被某个 `return` 漏掉 `pop`。
fn copy_tree_body(
    src: &std::path::Path,
    dst: &std::path::Path,
    chain: &mut Vec<std::path::PathBuf>,
) -> Result<(), String> {
    std::fs::create_dir_all(dst).map_err(|e| e.to_string())?;
    for entry in std::fs::read_dir(src).map_err(|e| e.to_string())? {
        // ⚠ 原实现是 `.flatten()`,把**读不出**的条目静默丢掉 ⇒ 源侧那份还在、
        // 目标侧那份没了,而调用方拿到的是「成功」。这与同模块「不做一半」的
        // 纪律(`COPY` 拒绝目录)冲突,一起收掉。
        let entry = entry.map_err(|e| e.to_string())?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if from.is_dir() {
            copy_tree(&from, &to, chain)?;
        } else {
            std::fs::copy(&from, &to).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// `FILE_SIZE(p) -> INTEGER` —— 字节数。
pub fn file_size(host: &mut dyn StdHost, args: Vec<Value>) -> Result<Outcome, WlwlError> {
    const NAME: &str = "FILE_SIZE";
    if args.len() != 1 {
        return Err(fs_arity(host, NAME, args.len(), 1));
    }
    let p = str_arg(host, NAME, &args[0])?;
    match std::fs::metadata(native(&p)) {
        Ok(m) => ok(Value::Integer(i64::try_from(m.len()).unwrap_or(i64::MAX))),
        Err(e) => ok(fs_err(NAME, io_reason(&p, &e))),
    }
}

/// `READ_BYTES` 的**单次读取上限** —— 与其它成员的护栏同族。
///
/// ⚠🔴 **[D13-047 / N-038] 这个上限此前只存在于文档注释里。** 同一个仓里
/// `std.net` 有 8 MiB 的载荷上限、`std.compress` 有
/// [`crate::compress::MAX_DECOMPRESSED`],**唯独** `READ_BYTES` 把护栏全押在
/// 一句「这是给小二进制用的」上。那个**不对称**才是缺陷本身:「读一个 4 GiB
/// 文件」因此变成一次无界的内存分配 + 一个超大 `ARRAY[INTEGER]`,而**没有**
/// 任何一条分支会响。
///
/// **为什么是 1 MiB 而不是照抄 net 的 8 MiB**:那两个上限把数据当**字节流**
/// 搬;`READ_BYTES` 把它物化成**语言值数组**,仓内已有一条相关结论
/// (D11-012 / D12-009:解释器侧数组构建是超线性的)。
/// ⚠ 「每字节放大多少倍」在仓内**查不到出处**,本轮也**未实测** ⇒ 这里不拿
/// 它当依据,只依据「同族成员都有上限」这一条可核验的事实。
pub const MAX_READ_BYTES: u64 = 1024 * 1024;

/// `READ_BYTES(p) -> ARRAY[INTEGER]` —— 二进制读,元素是 `0..=255`(业主 2026-10-08 裁决)。
///
/// 形状与 `std.encode::RANDOM_BYTES` **同款** —— 它已被规范定为「本语言唯一
/// 无损字节载体」:wlwl 的 `STRING` 是 UTF-8,装不了任意字节,硬塞只能**有损
/// 替换**成 `U+FFFD`(规范 §11.3-3 已把「不做有损替换」立成条款)。
///
/// ⚠️ **范围**:这是给**小**二进制用的(配置、图标、密钥材料片段)。
/// **大载荷的正解是让它待在文件里** —— 压缩 / 拷贝都在 R2 侧走字节流;绕语言值
/// 搬一趟会重新踩上解释器侧数组构建的超线性证据(D11-012 / D12-009);
/// 超 [`MAX_READ_BYTES`] 一律返 `ERR`,不是截断、不是 OOM。
pub fn read_bytes(host: &mut dyn StdHost, args: Vec<Value>) -> Result<Outcome, WlwlError> {
    const NAME: &str = "READ_BYTES";
    if args.len() != 1 {
        return Err(fs_arity(host, NAME, args.len(), 1));
    }
    let p = str_arg(host, NAME, &args[0])?;
    // ⚠ **先量后读**:`std::fs::read` 把整个文件一次性读进内存,那时上限已经
    // 没意义了 —— 上限必须在分配**之前**判。
    let mut f = match std::fs::File::open(native(&p)) {
        Ok(f) => f,
        Err(e) => return ok(fs_err(NAME, io_reason(&p, &e))),
    };
    let over = |len: u64| {
        fs_err(
            NAME,
            format!(
                "{p}: file is {len} bytes — READ_BYTES caps one read at \
                 {MAX_READ_BYTES} bytes (leave big payloads in files)"
            ),
        )
    };
    match f.metadata() {
        Ok(m) if m.len() > MAX_READ_BYTES => return ok(over(m.len())),
        Ok(_) => {}
        Err(e) => return ok(fs_err(NAME, io_reason(&p, &e))),
    }
    let mut bytes: Vec<u8> = Vec::new();
    match std::io::Read::read_to_end(&mut f, &mut bytes) {
        // ⚠ 再查一次:文件在 `metadata` 与 `read` 之间被写大了(并发写入)。
        Ok(_) if bytes.len() as u64 > MAX_READ_BYTES => ok(over(bytes.len() as u64)),
        Ok(_) => ok(Value::Array(
            bytes
                .into_iter()
                .map(|b| Value::Integer(i64::from(b)))
                .collect(),
        )),
        Err(e) => ok(fs_err(NAME, io_reason(&p, &e))),
    }
}

/// `WRITE_BYTES(p, data) -> NULL` —— 二进制写;`data` 是 `ARRAY[INTEGER]`。
///
/// 元素**逐个校验** `0..=255`:静默截断会让「同一个文件」有两种解释,而字节 IO
/// 最不能容忍的就是这种二义性。
///
/// ⚠ **一次写入的字节数组上限** —— 与 `std.compress` 的 `MAX_DECOMPRESSED`
/// **同值同源**:它是「语言值里一个字节数组能有多大」的**唯一**口径。
/// ⚠ **不另立第三个数字** —— 照 `D13-021` 的做法(「口径直接复用,不另立数字」):
/// 另立会让同一份输入在三个成员里得到**三种接受集**,而那种分叉没人说得清
/// 是哪一处错了。
/// ⚠ **为什么与 `READ_BYTES` 的 1 MiB 不同档**:那个是**读**的**策略**上限
/// (读进来要**物化成语言值数组**,超线性证据在那边);这个是**写**的**规模**上限
/// —— 写出去的是字节流、**不物化** ⇒ 取与解压护栏同档。
///
/// ⚠⚠ **写得出 ≠ 读得回**:`READ_BYTES` 一次只读 1 MiB,所以**大于 1 MiB 的文件
/// 是 `WRITE_BYTES` 写得出来、`READ_BYTES` 读不回来的**。这不是缺陷,是两个成员的
/// 档位不同;**真正的大载荷通路**是「让它待在文件里」+ 压缩 / 拷贝走字节流
/// (`std.compress`),绕语言值搬一趟会重新踩上解释器侧数组构建的超线性证据。
/// 流式写 / 分段读成员**本册不做**,见 §2.3 的说明。
pub const MAX_WRITE_BYTES: usize = crate::compress::MAX_DECOMPRESSED;

pub fn write_bytes(host: &mut dyn StdHost, args: Vec<Value>) -> Result<Outcome, WlwlError> {
    write_bytes_with_cap(host, args, MAX_WRITE_BYTES)
}

/// 实现体;`cap` 只为**可测**而参数化 —— 真实路径传 [`MAX_WRITE_BYTES`]。
///
/// ⚠ **上限必须在落盘之前判**:`std::fs::write` 会**截断**目标文件,
/// 判晚了就已经把原文件截掉了 ⇒ 「超限」必须表现为「原封不动」,
/// 而不是「被清空」。
fn write_bytes_with_cap(
    host: &mut dyn StdHost,
    args: Vec<Value>,
    cap: usize,
) -> Result<Outcome, WlwlError> {
    const NAME: &str = "WRITE_BYTES";
    if args.len() != 2 {
        return Err(fs_arity(host, NAME, args.len(), 2));
    }
    let p = str_arg(host, NAME, &args[0])?;
    let Value::Array(items) = &args[1] else {
        return Err(fs_type(host, NAME, "a byte array of integers", &args[1]));
    };
    // ⚠ **先判长度、再逐元素校验**:超限的数组**不该**被逐元素走一遍
    // (那是 O(n) 的无用功),更不该走到落盘那一步。
    if items.len() > cap {
        return ok(fs_err(
            NAME,
            format!(
                "array is {} bytes — WRITE_BYTES caps one write at {cap} bytes",
                items.len()
            ),
        ));
    }
    let mut bytes = Vec::with_capacity(items.len());
    for it in items {
        let Value::Integer(b) = it else {
            return Err(fs_type(host, NAME, "a byte array of integers", it));
        };
        if !(0..=255).contains(b) {
            return ok(fs_err(NAME, format!("byte {b} is outside 0..=255")));
        }
        bytes.push(*b as u8);
    }
    match std::fs::write(native(&p), &bytes) {
        Ok(()) => ok(Value::Null),
        Err(e) => ok(fs_err(NAME, io_reason(&p, &e))),
    }
}

/// 失败原因里带上路径 —— `ERR` 是**值**,调用方要能看见是哪一条路径出的问题。
fn io_reason(path: &str, e: &std::io::Error) -> String {
    let kind = match e.kind() {
        ErrorKind::NotFound => "not found",
        ErrorKind::PermissionDenied => "permission denied",
        ErrorKind::AlreadyExists => "already exists",
        _ => "I/O error",
    };
    format!("{path}: {kind}: {e}")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// fs 成员**只**通过 `diag` 回话(出错时),从不回调宿主 / 不碰 `StdCtx`
    /// —— 与 `rand.rs` / `compress.rs` 同款的「把不经宿主变成可执行断言」。
    struct NullHost;

    impl StdHost for NullHost {
        fn ctx(&mut self) -> &mut crate::StdCtx {
            panic!("wlwl:std.fs must not touch StdCtx")
        }
        fn call(
            &mut self,
            _f: &Value,
            _args: Vec<Value>,
            _name: &str,
        ) -> wlwl_error::WlwlResult<Outcome> {
            panic!("wlwl:std.fs has no member that calls back into the host")
        }
        fn diag(&mut self, code: ErrorCode, message: String) -> wlwl_error::WlwlError {
            wlwl_error::WlwlDiagnostic::new(
                code,
                message,
                wlwl_error::Location::point("<test>", 0, 0),
            )
            .into()
        }
    }

    fn tmpfile(suffix: &str) -> std::path::PathBuf {
        let mut p = std::env::temp_dir();
        let unique = format!(
            "wlwl_std_fs_test_{}_{}{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            suffix
        );
        p.push(unique);
        p
    }

    #[test]
    fn write_then_read_roundtrip() {
        let mut ctx = StdCtx::default();
        let path = tmpfile(".txt");
        let p_str = path.to_string_lossy().into_owned();

        std_write_file_inner(
            &mut ctx,
            vec![
                StdValue::String(p_str.clone()),
                StdValue::String("hello\nworld".into()),
            ],
        )
        .unwrap();

        let v = std_read_file_inner(&mut ctx, vec![StdValue::String(p_str.clone())]).unwrap();
        assert_eq!(v, StdValue::String("hello\nworld".into()));

        let _ = std::fs::remove_file(&path);
    }

    /// [D13-052] `WRITE_BYTES` 的上限 —— **判定表**逐档覆盖。
    ///
    /// ⚠ **为什么用 `_with_cap` 而不是真的造 64 MiB 的数组**:上限是 64 MiB
    /// ⇒ 物化那个数组要 ~1 GiB 的语言值,测试机上不现实。⇒ **上限值本身**
    /// 由 `write_bytes_cap_is_the_shared_byte_array_limit` 钉住,**判定逻辑**
    /// 由这里用一个小 cap 覆盖 —— 两者合起来覆盖全部路径,且**没有一条路径
    /// 只在测试里存在**。
    #[test]
    fn write_bytes_refuses_an_array_over_the_cap() {
        let mut host = NullHost;
        let path = tmpfile(".bin");
        let p_str = path.to_string_lossy().into_owned();
        // ⚠ 先放一个**已存在**的文件 —— 超限的写**必须不碰它**
        std::fs::write(&path, b"ORIGINAL").unwrap();

        let over = write_bytes_with_cap(
            &mut host,
            vec![
                Value::String(p_str.clone()),
                Value::Array(vec![Value::Integer(1); 5]),
            ],
            4,
        )
        .unwrap();
        // 超限 ⇒ 返 **ERR 值**(不是 Rust Err):与同族成员一致
        assert!(
            matches!(over.value, Value::Err(_)),
            "over-cap write must return an ERR value"
        );
        // ⚠⚠ **原文件必须原封不动** —— `std::fs::write` 会**截断**目标文件,
        // 判晚了就已经把文件清空了。这条是这个上限真正的风险点。
        assert_eq!(std::fs::read(&path).unwrap(), b"ORIGINAL");

        let _ = std::fs::remove_file(&path);
    }

    /// 边界:数组长度**正好等于**上限时**必须放行**(判据是 `>`,不是 `>=`)。
    #[test]
    fn write_bytes_accepts_an_array_exactly_at_the_cap() {
        let mut host = NullHost;
        let path = tmpfile(".bin");
        let p_str = path.to_string_lossy().into_owned();

        let v = write_bytes_with_cap(
            &mut host,
            vec![
                Value::String(p_str.clone()),
                Value::Array(vec![Value::Integer(7); 4]),
            ],
            4,
        )
        .unwrap();
        assert_eq!(v.value, Value::Null);
        assert_eq!(std::fs::read(&path).unwrap(), vec![7u8; 4]);

        let _ = std::fs::remove_file(&path);
    }

    /// ⚠ **上限不得另立数字** —— 它必须与 `std.compress` 的 `MAX_DECOMPRESSED`
    /// **同值同源**。另立会让同一份输入在三个成员里得到三种接受集,
    /// 而那种分叉没人说得清是哪一处错了。
    #[test]
    fn write_bytes_cap_is_the_shared_byte_array_limit() {
        assert_eq!(MAX_WRITE_BYTES, crate::compress::MAX_DECOMPRESSED);
        assert_eq!(MAX_WRITE_BYTES, 64 * 1024 * 1024);
    }

    #[test]
    fn read_nonexistent_is_e0061() {
        let mut ctx = StdCtx::default();
        let err = std_read_file_inner(
            &mut ctx,
            vec![StdValue::String(
                "Z:/this/path/should/definitely/not/exist/abc_xyz_123".into(),
            )],
        )
        .unwrap_err();
        assert_eq!(err.code, ErrorCode::E0061);
    }

    #[test]
    fn exists_true_for_real_and_false_for_missing() {
        let mut ctx = StdCtx::default();
        let path = tmpfile(".txt");
        std::fs::write(&path, b"x").unwrap();
        let p_str = path.to_string_lossy().into_owned();

        let v_true = std_exists_inner(&mut ctx, vec![StdValue::String(p_str.clone())]).unwrap();
        assert_eq!(v_true, StdValue::Bool(true));

        let _ = std::fs::remove_file(&path);
        let v_false = std_exists_inner(&mut ctx, vec![StdValue::String(p_str)]).unwrap();
        assert_eq!(v_false, StdValue::Bool(false));
    }

    #[test]
    fn arity_mismatch_is_e0022() {
        let mut ctx = StdCtx::default();
        let err = std_read_file_inner(&mut ctx, vec![]).unwrap_err();
        assert_eq!(err.code, ErrorCode::E0022);
    }

    #[test]
    fn spec_lists_every_member() {
        assert_eq!(SPEC.path, "wlwl:std.fs");
        let names: Vec<&str> = SPEC.functions.iter().map(|(n, _)| *n).collect();
        // 前三个是 v0.3 契约冻结的原有成员,顺序不可动(成员表镜像的惯例:
        // 老成员在前、新成员按功能追加)。
        assert_eq!(
            &names[..3],
            &["READ_FILE", "WRITE_FILE", "EXISTS"],
            "the three original members must stay first and in order"
        );
        // 后 15 个是 v0.11.3 M6 / addendum-06 新增 —— **成员面是契约**。
        assert_eq!(
            &names[3..],
            &[
                "PATH_JOIN",
                "PATH_DIR",
                "PATH_BASE",
                "PATH_EXT",
                "PATH_NORMALIZE",
                "LIST_DIR",
                "WALK_DIR",
                "MKDIR",
                "MKDIRS",
                "REMOVE",
                "COPY",
                "MOVE",
                "FILE_SIZE",
                "READ_BYTES",
                "WRITE_BYTES",
            ]
        );
        assert_eq!(names.len(), 18, "wlwl:std.fs exports 18 members");
    }

    // ── [D13-044] `spec()` 的往返不变量 ─────────────────────────────────

    /// **本条才是 `spec()` 真正要守的东西**:它吐出来的字符串必须能**原样**喂回
    /// `native()` 指到同一个路径。`LIST_DIR` 的出参就是拿它喂 `READ_FILE` /
    /// `MOVE` / `REMOVE` 的,所以这条不变量一破就是**不可逆的往返破坏**。
    ///
    /// ⚠ 本测试在两个平台上都成立(不靠 `cfg!` 分支),所以它是可执行的。
    #[test]
    fn spec_output_feeds_straight_back_into_native() {
        let p = std::env::temp_dir()
            .join("wlwl_fs_spec_probe")
            .join("a.txt");
        let s = spec(&p);
        assert_eq!(
            native(&s),
            p,
            "spec() output must round-trip through native(): {s}"
        );
    }

    /// ⚠ `[D13-044]` POSIX 上的 `a\b.txt` 是一个**文件名**,不是一个路径。
    ///
    /// ⚠ **POSIX 那一侧未经实测**:本轮开发机是 Windows ⇒ CI 实际只跑
    /// `cfg!(windows)` 那一支。这条测试的作用是「把 POSIX 那一支的判定写死
    /// 在代码里」,等真有 Linux / macOS 构建机时它会**真的**跑起来。
    #[test]
    fn a_backslash_in_a_filename_survives_on_posix() {
        let name = std::path::PathBuf::from("a\\b.txt");
        let got = spec(&name);
        if cfg!(windows) {
            // Windows 上 `\` 是分隔符、文件名里不可能有 ⇒ 替换无损。
            assert_eq!(got, "a/b.txt");
        } else {
            assert_eq!(got, r"a\b.txt", "POSIX must not rewrite a legal filename");
        }
    }

    // ── [D13-047] `READ_BYTES` 的上限 ──────────────────────────────────

    /// 上限常量被写死(它是**规范条款**,不是一个可随手调的数字),并守住
    /// **恰好等于上限**这个边界:等于上限**必须**能读(只超一个字节才拒),
    /// 否则「上限」就变成了「上限减一」那种查不出毛病的偏差。
    #[test]
    fn read_bytes_reads_up_to_the_cap_and_refuses_one_byte_more() {
        assert_eq!(
            MAX_READ_BYTES,
            1024 * 1024,
            "the READ_BYTES cap is a spec clause; changing it is a spec change"
        );
        let at = tmpfile(".bin");
        let over = tmpfile(".bin");
        std::fs::write(&at, vec![7u8; MAX_READ_BYTES as usize]).unwrap();
        std::fs::write(&over, vec![7u8; MAX_READ_BYTES as usize + 1]).unwrap();

        let ok_out = read_bytes(
            &mut NullHost,
            vec![Value::String(at.to_string_lossy().into_owned())],
        )
        .expect("at the cap must decode")
        .value;
        match ok_out {
            Value::Array(items) => assert_eq!(items.len(), MAX_READ_BYTES as usize),
            other => panic!("expected an array, got {other:?}"),
        }

        let r = read_bytes(
            &mut NullHost,
            vec![Value::String(over.to_string_lossy().into_owned())],
        )
        .expect("over the cap must be an ERR value, not a diagnostic");
        assert!(
            matches!(r.value, Value::Err(_)),
            "one byte over the cap must be an ERR value, got {:?}",
            r.value
        );

        // ⚠ **未经实测**的推断:文件**恰好**在上限上时本实现分配了 ≥ 1 MiB。
        //   本轮没有为它单独测峰值(见报告);这里只保证它不炸。
        let _ = std::fs::remove_file(&at);
        let _ = std::fs::remove_file(&over);
    }

    /// ⚠🔴 **[D13-066] 同目标别名** —— `link -> real` 两条**兄弟**路径指向同
    /// 一个真实目录,**都必须展开**;它**不是环**。
    ///
    /// 缺陷形态:防环集合是**全局已见集合**且**从不出栈** ⇒ 兄弟里第二个被
    /// 遍历到的那个必然命中 `visited.contains` ⇒ `MOVE` 对一棵**完全合法**的
    /// 树直接报 `symlink cycle`。⚠ 与 `D13-065`(`walk`)同源但**不是同一形状**:
    /// `walk` 是**静默跳过**(少一条路径、顺序相关,正是 CI 只红 macOS 的原因);
    /// 本条是**硬 ERR**,且**两个遍历顺序都失败** ⇒ 确定性失败,顺序只影响
    /// 「失败前写出去多少」。
    ///
    /// ⚠ **断言「两条都在」而不是只断言「没报错」** —— 只钉不报错的话,
    /// 「只搬了一份、另一份被吃掉」这个方向照不出来(`D13-065` 犯过同样的错)。
    #[cfg(any(unix, windows))]
    #[test]
    fn copy_tree_expands_a_same_target_alias_instead_of_calling_it_a_cycle() {
        let dir = tmpfile("");
        std::fs::create_dir_all(dir.join("real")).unwrap();
        std::fs::write(dir.join("real/inside.txt"), b"x").unwrap();
        alias_dir(&dir.join("real"), &dir.join("link"));
        let dst = tmpfile("_alias_dst");

        let r = copy_tree(&dir, &dst, &mut Vec::new());
        assert!(r.is_ok(), "an alias is not a cycle, got Err: {r:?}");
        assert!(
            dst.join("real/inside.txt").exists(),
            "the real path must be copied: {dst:?}"
        );
        assert!(
            dst.join("link/inside.txt").exists(),
            "the aliased path must be copied too: {dst:?}"
        );

        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&dst);
    }

    /// ⚠ **N-032 的真实成环实测** —— 此前那条守卫靠**预置 `visited`** 绕开,
    /// 「链接指回祖先」这条路**从来没被真的走过**。⚠ junction / symlink 让它
    /// 在本机就能跑,不必等 POSIX 平台。
    ///
    /// ⚠ 这条**不是缺陷守卫,是回归守卫**:旧的全局已见集合也能挡住真环
    /// (它是**更严**而不是更松)⇒ 缺陷形态下它也绿。它的作用是钉住
    /// `D13-066` 改成祖先链之后**没有把真环放过** —— 祖先链比全局已见**松**,
    /// 「松」的那部分必须逐条有守卫。
    #[cfg(any(unix, windows))]
    #[test]
    fn copy_tree_refuses_a_real_cycle_made_by_a_link_back_to_an_ancestor() {
        let dir = tmpfile("");
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("sub/leaf.txt"), b"x").unwrap();
        // `sub/loop` 指回**根** ⇒ 跟到底就是无限下降。
        alias_dir(&dir, &dir.join("sub/loop"));
        let dst = tmpfile("_cycle_dst");

        // ⚠ 测试能跑完本身就是「有界」的证据:缺陷形态(无防环)会无限递归,
        // 这条用例会**挂死到 CI 超时**而不是返回。
        let reason = match copy_tree(&dir, &dst, &mut Vec::new()) {
            Ok(()) => panic!("a real cycle must be refused, got Ok(())"),
            Err(e) => e,
        };
        assert!(
            reason.contains("cycle"),
            "the reason must name the cycle: {reason}"
        );
        // ⚠ **只有环那一层**不许在目标侧留痕迹(父层 `dst/sub` 早该建好)。
        assert!(
            !dst.join("sub/loop").exists(),
            "the cycle guard must fire before create_dir_all on the cycling level: {dst:?}"
        );

        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&dst);
    }

    /// 造一条「指向 `target` 的目录别名」。**跨平台都不需要管理员。**
    ///
    /// ⚠ Windows 上 `symlink_dir` 要特权 / 开发者模式(实测本机报「此操作需要
    /// 管理员权限」)⇒ 改用**目录联接(junction)**。⚠ 它同样能被 `canonicalize()`
    /// 与 `is_dir()` 解析(实测 `real` 与 `link` 的 `canonicalize()` **完全相等**,
    /// 都是 `\\?\…\real`)⇒ 对本文件要验的那条路径**等价**,而权限要求低得多。
    #[cfg(unix)]
    fn alias_dir(target: &std::path::Path, link: &std::path::Path) {
        std::os::unix::fs::symlink(target, link).expect("create a directory symlink");
    }

    /// ⚠ `cmd` **不存在于 POSIX** ⇒ 这一支只编进 Windows(同 `D13-054` 的
    /// 教训:平台原生程序名不能让 probe / 单测在另一个平台上编译失败)。
    #[cfg(windows)]
    fn alias_dir(target: &std::path::Path, link: &std::path::Path) {
        use std::os::windows::process::CommandExt;
        // ⚠⚠ **必须用 `raw_arg`,不能用 `.arg()`。** `.arg()` 碰到含空格的参数
        // 会自己加一层外引号、并把**内层**的 `"` 转义成 `\"` ⇒ **`cmd` 不认
        // 反斜杠转义**,`mklink` 收到的路径带反斜杠,报「文件名、目录名或卷标
        // 语法不正确」(实测连栽两次)。`raw_arg` 跳过 Rust 的引号处理,`cmd`
        // 拿到的就是原样那条命令。
        //
        // ⚠ 引号本身是必需的:临时目录名里可能有空格(实测去掉引号即
        // 「The syntax of the command is incorrect」)。
        let out = std::process::Command::new("cmd")
            .raw_arg(format!(
                "/C mklink /J \"{}\" \"{}\"",
                link.display(),
                target.display()
            ))
            .output()
            .expect("cmd /C mklink /J runs");
        assert!(
            out.status.success(),
            "mklink /J failed: {}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }

    /// 「这个路径是不是链接」—— **跨平台**。
    ///
    /// ⚠ Windows 上 `FileType::is_symlink()` 对**目录联接(junction)**返 **false**
    /// (Rust 只看 `FILE_ATTRIBUTE_SYMLINK`,而 junction 是 reparse point)⇒
    /// 必须自己看 `FILE_ATTRIBUTE_REPARSE_POINT`。
    #[cfg(unix)]
    fn is_link(p: &std::path::Path) -> bool {
        std::fs::symlink_metadata(p)
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false)
    }

    #[cfg(windows)]
    fn is_link(p: &std::path::Path) -> bool {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        std::fs::symlink_metadata(p)
            .map(|m| m.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0)
            .unwrap_or(false)
    }

    /// [D13-051] **同设备** `MOVE` 必须**保留链接**(不展开成实体目录)。
    ///
    /// ⚠ **跨设备那半边做不成 CI 契约**:`rename` 在同设备必成功 ⇒ 跨设备回退
    /// 在**单卷环境不可达**(要第二个卷)。⇒ 本条钉住**可达**的那半边;跨设备
    /// 那半边由规范 §2.3 的明文 + 本机 D: → C: 实测承担。
    ///
    /// ⚠ 断言的是「**链接还在**」而不是「目录在」—— 展开成实体目录**也**满足
    /// 后者(这正是 `D13-065` 犯过的错:只钉「没报错」照不出「被吃掉」那个方向)。
    #[cfg(any(unix, windows))]
    #[test]
    fn move_within_one_device_preserves_a_link() {
        let dir = tmpfile("");
        std::fs::create_dir_all(dir.join("real")).unwrap();
        std::fs::write(dir.join("real/inside.txt"), b"x").unwrap();
        alias_dir(&dir.join("real"), &dir.join("link"));
        let dst = tmpfile("_link_dst");

        let mut host = NullHost;
        let out = move_path(
            &mut host,
            vec![
                Value::String(dir.join("link").to_string_lossy().into_owned()),
                Value::String(dst.to_string_lossy().into_owned()),
            ],
        )
        .unwrap();
        assert!(
            matches!(out.value, Value::Null),
            "MOVE must succeed, got {:?}",
            out.value
        );
        assert!(
            is_link(&dst),
            "same-device MOVE must preserve the link, not expand it into a real directory"
        );
        assert!(
            dst.join("inside.txt").exists(),
            "the preserved link must still resolve to its target"
        );

        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&dst);
    }

    // ── [D13-046] `copy_tree` 的防环守卫 ───────────────────────────────

    /// 成环 ⇒ **`ERR`** 而不是无限递归。
    ///
    /// ⚠ 这条**不需要真的建链接**:把源目录的 `canonicalize()` 预置进祖先链
    /// 就等价于「递归回到自己」的那个状态。⚠⚠ 但它**只守得住守卫本身** ——
    /// 「真实链接成环时整条路径的实际表现」当年没人跑过 ⇒ `D13-066` 补了
    /// 上面那条用 junction / symlink 的真用例,两条并存。
    #[test]
    fn copy_tree_refuses_a_cycle_instead_of_recursing_forever() {
        let dir = tmpfile("");
        std::fs::create_dir_all(&dir).unwrap();
        let dst = tmpfile("_copy");
        let canon = dir.canonicalize().unwrap();
        let mut chain = vec![canon];
        let r = copy_tree(&dir, &dst, &mut chain);
        let reason = match &r {
            Ok(()) => panic!("a revisited directory must be refused, got Ok(())"),
            Err(e) => e.clone(),
        };
        assert!(
            reason.contains("cycle"),
            "the reason must name the cycle: {reason}"
        );
        // 守卫**先于**建目录:环上的那一层不该在目标侧留痕迹。
        assert!(
            !dst.exists(),
            "the cycle guard must fire before create_dir_all"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
