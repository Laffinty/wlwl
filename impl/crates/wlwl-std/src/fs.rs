//! `wlwl:std.fs` — 文本读写 + **路径 / 目录 / 字节**(v0.3 §15.3 · v0.11.3 M6 扩)。
//!
//! Error code mapping (matches spec §14.4) for the **three original** members:
//!   - E0061 — file not found
//!   - E0062 — permission denied
//!   - E0060 — other I/O error (retryable)
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
        Err(e) => Err(io_error_to_std("READ_FILE", e, path)),
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
        Err(e) => Err(io_error_to_std("WRITE_FILE", e, path)),
    }
}

pub(super) fn std_exists_inner(
    _ctx: &mut StdCtx,
    args: Vec<StdValue>,
) -> Result<StdValue, StdError> {
    let path = expect_string("EXISTS", &args, 0, 1)?;
    Ok(StdValue::Bool(std::fs::metadata(path).is_ok()))
}

/// Map a `std::io::Error` to the v0.3 §14.4 IO error codes. `E0060`
/// is the catch-all; `E0061` for NotFound; `E0062` for PermissionDenied.
fn io_error_to_std(fn_name: &str, e: std::io::Error, path: &str) -> StdError {
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
fn spec(p: &std::path::Path) -> String {
    p.to_string_lossy().replace('\\', "/")
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
///   递归永不终止)。置 `TRUE` 时记 **visited 集合**(按 `canonicalize` 的目录)
///   防环。
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
    walk(
        &base,
        "",
        pattern.as_deref(),
        follow,
        &mut out,
        &mut visited,
    );
    out.sort();
    ok(strings(out))
}

fn walk(
    dir: &std::path::Path,
    rel: &str,
    pattern: Option<&str>,
    follow: bool,
    out: &mut Vec<String>,
    visited: &mut Vec<std::path::PathBuf>,
) {
    // 跟随时防环:同一个真实目录只展开一次。
    if follow {
        if let Ok(canon) = dir.canonicalize() {
            if visited.contains(&canon) {
                return;
            }
            visited.push(canon);
        }
    }
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
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
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        if glob_hit(pattern, &child_rel) {
            out.push(child_rel.clone());
        }
        if is_dir && (!is_link || follow) {
            walk(&path, &child_rel, pattern, follow, out, visited);
        }
    }
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
        match copy_tree(&s, &d) {
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

fn copy_tree(src: &std::path::Path, dst: &std::path::Path) -> Result<(), String> {
    std::fs::create_dir_all(dst).map_err(|e| e.to_string())?;
    for entry in std::fs::read_dir(src).map_err(|e| e.to_string())?.flatten() {
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if from.is_dir() {
            copy_tree(&from, &to)?;
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

/// `READ_BYTES(p) -> ARRAY[INTEGER]` —— 二进制读,元素是 `0..=255`(业主 2026-10-08 裁决)。
///
/// 形状与 `std.encode::RANDOM_BYTES` **同款** —— 它已被规范定为「本语言唯一
/// 无损字节载体」:wlwl 的 `STRING` 是 UTF-8,装不了任意字节,硬塞只能**有损
/// 替换**成 `U+FFFD`(规范 §11.3-3 已把「不做有损替换」立成条款)。
///
/// ⚠️ **范围**:这是给**小**二进制用的(配置、图标、密钥材料片段)。
/// **大载荷的正解是让它待在文件里** —— 压缩 / 拷贝都在 R2 侧走字节流;绕语言值
/// 搬一趟会重新踩上解释器侧数组构建的超线性证据(D11-012 / D12-009)。
pub fn read_bytes(host: &mut dyn StdHost, args: Vec<Value>) -> Result<Outcome, WlwlError> {
    const NAME: &str = "READ_BYTES";
    if args.len() != 1 {
        return Err(fs_arity(host, NAME, args.len(), 1));
    }
    let p = str_arg(host, NAME, &args[0])?;
    match std::fs::read(native(&p)) {
        Ok(bytes) => ok(Value::Array(
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
pub fn write_bytes(host: &mut dyn StdHost, args: Vec<Value>) -> Result<Outcome, WlwlError> {
    const NAME: &str = "WRITE_BYTES";
    if args.len() != 2 {
        return Err(fs_arity(host, NAME, args.len(), 2));
    }
    let p = str_arg(host, NAME, &args[0])?;
    let Value::Array(items) = &args[1] else {
        return Err(fs_type(host, NAME, "a byte array of integers", &args[1]));
    };
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
}
