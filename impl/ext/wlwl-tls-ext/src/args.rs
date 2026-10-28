//! 实参的小工具（取字符串 / 整数 / 字节 / 选项字典）。
//!
//! ⚠ 这些函数返回 `Result<_, String>`（只是**原因**）—— 由调用方决定用哪个
//! kind（`TlsError` / `CryptoError`）包成失败。

use serde_json::{Map, Value as Json};

use crate::bytes::bytes_from_json;

/// 实参个数不对。
pub fn arity(op: &str, got: usize, want: usize) -> String {
    format!("{op} expects {want} argument(s), got {got}")
}

/// 第 `i` 个实参（缺 ⇒ 报「实参个数不对」）。
pub fn arg<'a>(args: &'a [Json], i: usize, op: &str, want: usize) -> Result<&'a Json, String> {
    args.get(i)
        .ok_or_else(|| arity(op, args.len(), want))
}

/// 字符串实参。
pub fn str_arg(args: &[Json], i: usize, op: &str, want: usize) -> Result<String, String> {
    let v = arg(args, i, op, want)?;
    match v {
        Json::String(s) => Ok(s.clone()),
        other => Err(format!(
            "{op}: argument {} must be a string, got {}",
            i + 1,
            crate::bytes::type_name(other)
        )),
    }
}

/// 整数实参。
pub fn int_arg(args: &[Json], i: usize, op: &str, want: usize) -> Result<i64, String> {
    let v = arg(args, i, op, want)?;
    match v {
        Json::Number(n) => n
            .as_i64()
            .ok_or_else(|| format!("{op}: argument {} must be an integer", i + 1)),
        other => Err(format!(
            "{op}: argument {} must be an integer, got {}",
            i + 1,
            crate::bytes::type_name(other)
        )),
    }
}

/// 字节实参（`ARRAY[INTEGER]` 或 `STRING`）。
pub fn bytes_arg(args: &[Json], i: usize, op: &str, want: usize) -> Result<Vec<u8>, String> {
    let v = arg(args, i, op, want)?;
    bytes_from_json(v, &format!("{op}: argument {}", i + 1))
}

/// 可选的**选项字典**（`DICT` 或 `NULL`/缺省）。
///
/// ⚠ WLWL 的 `[]` 是**空数组**，不是空字典 ⇒ 传 `[]` 会被拒（这是刻意的：
/// 与 std 的 `opts` 口径一致）。
pub fn opts_arg(
    args: &[Json],
    i: usize,
    op: &str,
    want: usize,
) -> Result<Map<String, Json>, String> {
    if args.len() < want - 1 {
        return Err(arity(op, args.len(), want - 1));
    }
    match args.get(i) {
        None => Ok(Map::new()),
        Some(Json::Object(map)) => Ok(map.clone()),
        Some(Json::Null) => Ok(Map::new()),
        Some(other) => Err(format!(
            "{op}: argument {} must be a DICT or NULL, got {}",
            i + 1,
            crate::bytes::type_name(other)
        )),
    }
}

/// 从选项字典里取一个字符串选项。
pub fn opt_str(
    opts: &Map<String, Json>,
    key: &str,
    op: &str,
) -> Result<Option<String>, String> {
    match opts.get(key) {
        None => Ok(None),
        Some(Json::String(s)) => Ok(Some(s.clone())),
        Some(other) => Err(format!(
            "{op}: option '{key}' must be a string, got {}",
            crate::bytes::type_name(other)
        )),
    }
}

/// 从选项字典里取一个布尔选项。
pub fn opt_bool(
    opts: &Map<String, Json>,
    key: &str,
    op: &str,
) -> Result<Option<bool>, String> {
    match opts.get(key) {
        None => Ok(None),
        Some(Json::Bool(b)) => Ok(Some(*b)),
        Some(other) => Err(format!(
            "{op}: option '{key}' must be a boolean, got {}",
            crate::bytes::type_name(other)
        )),
    }
}

/// 从选项字典里取一个整数选项。
pub fn opt_int(opts: &Map<String, Json>, key: &str, op: &str) -> Result<Option<i64>, String> {
    match opts.get(key) {
        None => Ok(None),
        Some(Json::Number(n)) => Ok(Some(
            n.as_i64()
                .ok_or_else(|| format!("{op}: option '{key}' must be an integer"))?,
        )),
        Some(other) => Err(format!(
            "{op}: option '{key}' must be an integer, got {}",
            crate::bytes::type_name(other)
        )),
    }
}

/// 拒绝**未知**的选项名。
///
/// ⚠ 拼错的上界会变成「没有上界」⇒ 不静默忽略（与 std 的 `opts` 口径一致）。
pub fn reject_unknown(opts: &Map<String, Json>, allowed: &[&str], op: &str) -> Result<(), String> {
    for key in opts.keys() {
        if !allowed.contains(&key.as_str()) {
            return Err(format!(
                "{op}: unknown option '{key}' (allowed: {})",
                allowed.join(", ")
            ));
        }
    }
    Ok(())
}
