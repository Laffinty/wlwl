//! 字节 ⇄ JSON 的编解码。
//!
//! ⚠ WLWL **没有原生字节类型** ⇒ 字节一律用两种形态承载：
//!
//! | 形态 | 含义 |
//! |---|---|
//! | `ARRAY[INTEGER]` | 二进制的规范形态；每个元素必须是 `0..=255` |
//! | `STRING` | 取其 **UTF-8 字节**（⚠ **不是** hex 串！） |
//!
//! ⚠ **输出**侧统一用**小写 hex 字符串**（`crypto` 命名空间的摘要/密文/
//! 签名/密钥指纹）—— 那是唯一「人能复制粘贴」的二进制形态。
//! 唯一的例外是 `tls.RECV` 的 `bytes`：它沿用 `TCP_RECV` 的形状
//! （`ARRAY[INTEGER]`），因为那个成员要与 std 同形。

use serde_json::Value as Json;

/// JSON 值 → 字节。
///
/// * `ARRAY[INTEGER]` ⇒ 每个元素 0..=255；
/// * `STRING` ⇒ 它的 UTF-8 字节。
pub fn bytes_from_json(v: &Json, what: &str) -> Result<Vec<u8>, String> {
    match v {
        Json::Array(items) => {
            let mut out = Vec::with_capacity(items.len());
            for (i, item) in items.iter().enumerate() {
                let Some(n) = item.as_i64() else {
                    return Err(format!("{what}[{i}] is not an integer"));
                };
                if !(0..=255).contains(&n) {
                    return Err(format!(
                        "{what}[{i}] is {n}, which is outside the byte range 0..=255"
                    ));
                }
                out.push(n as u8);
            }
            Ok(out)
        }
        Json::String(s) => Ok(s.as_bytes().to_vec()),
        other => Err(format!(
            "{what} must be an ARRAY[INTEGER] or a STRING, got {}",
            type_name(other)
        )),
    }
}

/// 字节 → `ARRAY[INTEGER]`。
pub fn bytes_to_json(bytes: &[u8]) -> Json {
    Json::Array(bytes.iter().map(|b| Json::from(*b)).collect())
}

/// JSON 值的类型名（给错误消息用）。
pub fn type_name(v: &Json) -> &'static str {
    match v {
        Json::Null => "null",
        Json::Bool(_) => "boolean",
        Json::Number(_) => "number",
        Json::String(_) => "string",
        Json::Array(_) => "array",
        Json::Object(_) => "dict",
    }
}

/// 小写 hex 编码。
pub fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(DIGITS[(b >> 4) as usize] as char);
        out.push(DIGITS[(b & 0x0f) as usize] as char);
    }
    out
}

/// hex 解码（大小写都认；**不接受** `0x` 前缀与分隔符）。
pub fn unhex(s: &str) -> Result<Vec<u8>, String> {
    let s = s.trim();
    if s.len() % 2 != 0 {
        return Err(format!(
            "the hex string has an odd length ({}), so it cannot be decoded",
            s.len()
        ));
    }
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(s.len() / 2);
    for pair in bytes.chunks(2) {
        let hi = hex_digit(pair[0])?;
        let lo = hex_digit(pair[1])?;
        out.push((hi << 4) | lo);
    }
    Ok(out)
}

fn hex_digit(b: u8) -> Result<u8, String> {
    match b {
        b'0'..=b'9' => Ok(b - b'0'),
        b'a'..=b'f' => Ok(b - b'a' + 10),
        b'A'..=b'F' => Ok(b - b'A' + 10),
        other => Err(format!(
            "'{}' is not a hex digit",
            char::from(other)
        )),
    }
}
