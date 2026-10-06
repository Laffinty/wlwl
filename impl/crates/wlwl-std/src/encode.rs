//! [v0.11.2 M1] `wlwl:std.encode` — base64 / hex / percent-encoding (R2).
//!
//! ## 为什么是原生 R2 而不是 `wrap` 那条 serde_json 路
//!
//! io/fs/json/format/ai/agent 走 `crate::wrap`(ADR-0022 §4)是因为它们
//! 历史上以 serde_json 为中间表示。本模块从第一天起就只需要字节,而
//! serde_json 是 JSON 值模型 —— 绕一圈转换不产生任何好处。
//!
//! ## 三条边界契约(与 `kernels.rs` 的 `DomainError` 同款形状)
//!
//! - 元数错 → `E0022`;实参类型错 → `E0030`。二者都是**程序员错误**,
//!   按 §7 与 §5 的既有口径走原生诊断。
//! - **解码失败是域违例**,返 `ERR(["kind": "DecodeError", ...])` 值。
//!   非法 base64 字符、hex 奇数长度、非法 `%` 转义、解出来不是合法
//!   UTF-8 —— 全是这一类。
//! - **编码永不失败**:任意 `STRING` 都能编。`BASE64_ENCODE` 之类没有
//!   失败格,规范表里那一列是「—」而不是 `ERR`。
//!
//! ## UTF-8 严格性
//!
//! 三种编码都作用在 **UTF-8 字节**上:编码取 `STRING` 的 UTF-8 字节,
//! 解码后**必须**是合法 UTF-8 才交出去。wlwl 没有字节类型,解码出
//! 非法字节就没有能诚实表达的返回形态 —— 与其有损地替换成 `U+FFFD`
//! (`to_string_lossy`)骗调用方,不如明确报 `DecodeError`。
//!
//! ## 哈希的边界(v0.11.3 M2,W-09,否决记录)
//!
//! `SHA256` / `HMAC_SHA256` 提供的是**单向摘要**:
//! - 对称 / 非对称加密**不做**(误用重灾区 —— 标准库给了 AES 就会有人
//!   自己攒 TLS);
//! - 密钥生成**不做**(需要系统随机,撞 `std.rand` 的同族裁决 D12-006);
//! - `MD5` / `SHA-1` 已破,**不提供**;SHA-3 / BLAKE3 记演进方向;
//! - 实现不承诺常数时间;`HMAC_SHA256` 结果的比较由调用方负责。
//!
//! 实现复用 `wlwl-ast::sha256`(FIPS 180-4 纯 Rust,原为 AST 稳定 ID 而
//! 写;[v0.11.3 M2] 起字节级 `sha256` 公开)—— 单一实现两个消费者,
//! 杜绝副本漂移。依赖方向合法:`wlwl-std` 本就依赖 `wlwl-ast`。

use crate::{ModuleSpec, StdFn};
use argon2::{Algorithm, Argon2, ParamsBuilder, Version};
use wlwl_error::ErrorCode;
use wlwl_value::{Outcome, StdHost, Value};

pub static SPEC: ModuleSpec = ModuleSpec {
    path: "wlwl:std.encode",
    functions: &[
        ("BASE64_ENCODE", base64_encode as StdFn),
        ("BASE64_DECODE", base64_decode as StdFn),
        ("HEX_ENCODE", hex_encode as StdFn),
        ("HEX_DECODE", hex_decode as StdFn),
        ("URL_ENCODE", url_encode as StdFn),
        ("URL_DECODE", url_decode as StdFn),
        ("SHA256", sha256 as StdFn),
        ("HMAC_SHA256", hmac_sha256 as StdFn),
        ("PBKDF2_ITER", pbkdf2_iter as StdFn),
        ("ARGON2ID", argon2id as StdFn),
        ("RANDOM_BYTES", random_bytes as StdFn),
        ("RANDOM_HEX", random_hex as StdFn),
        ("TIMING_SAFE_EQ", timing_safe_eq as StdFn),
    ],
};

// ── 助手 ───────────────────────────────────────────────────────────────

fn arity(host: &mut dyn StdHost, name: &str, got: usize, want: usize) -> wlwl_error::WlwlError {
    host.diag(
        ErrorCode::E0022,
        format!("{name}: function expects {want} argument(s), got {got}"),
    )
}

fn type_err(
    host: &mut dyn StdHost,
    name: &str,
    expected: &str,
    got: &Value,
) -> wlwl_error::WlwlError {
    host.diag(
        ErrorCode::E0030,
        format!(
            "{name}: expected {expected}, got {}",
            crate::value_kind(got)
        ),
    )
}

/// 解码失败载荷。与 `kernels.rs::domain_error` 同构,只是 `kind` 不同:
/// 数学域违例是 `DomainError`,畸形编码是 `DecodeError` —— 把它塞进
/// `DomainError` 会让「`SQRT(-1)`」和「base64 串少了一个字符」长得一样。
fn decode_error(op: &str, reason: String) -> Value {
    Value::Err(Box::new(Value::Dict(vec![
        (
            Value::String("kind".into()),
            Value::String("DecodeError".into()),
        ),
        (Value::String("op".into()), Value::String(op.into())),
        (Value::String("reason".into()), Value::String(reason)),
    ])))
}

/// 字节 → `STRING`。不合法 UTF-8 走 `DecodeError` 而不是有损替换。
fn bytes_to_value(op: &str, bytes: Vec<u8>) -> Result<Value, Value> {
    match String::from_utf8(bytes) {
        Ok(s) => Ok(Value::String(s)),
        Err(_) => Err(decode_error(op, "decoded bytes are not valid UTF-8".into())),
    }
}

/// 解码成员的三段式收口:字节层失败与 UTF-8 层失败**都**是 ERR 值,
/// 所以逐个手写 `match` 会让 12 个分支重复三遍。调用方只需要知道
/// 「拿到 STRING,或者拿到一个 DecodeError」。
fn finish_decode(op: &str, decoded: Result<Vec<u8>, String>) -> Value {
    let as_value = match decoded {
        Ok(bytes) => bytes_to_value(op, bytes),
        Err(reason) => Err(decode_error(op, reason)),
    };
    match as_value {
        Ok(v) => v,
        Err(e) => e,
    }
}

fn str_arg<'a>(
    host: &mut dyn StdHost,
    name: &str,
    args: &'a [Value],
) -> Result<&'a str, wlwl_error::WlwlError> {
    match &args[0] {
        Value::String(s) => Ok(s.as_str()),
        other => Err(type_err(host, name, "string", other)),
    }
}

// ── base64 (RFC 4648 §4) ───────────────────────────────────────────────

const B64_STD: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
const B64_URL: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

fn b64_encode_bytes(bytes: &[u8], url_safe: bool) -> String {
    let alpha = if url_safe { B64_URL } else { B64_STD };
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(alpha[(n >> 18) as usize & 63] as char);
        out.push(alpha[(n >> 12) as usize & 63] as char);
        if chunk.len() > 1 {
            out.push(alpha[(n >> 6) as usize & 63] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(alpha[n as usize & 63] as char);
        } else {
            out.push('=');
        }
    }
    out
}

/// 解码。**自动识别字母表**:标准表的 `+/` 与 URL 表的 `-_` 除了这两个
/// 字符之外完全相同,所以两者都收是确定性的。混用也能解(Go 的两个
/// `Encoding` 都会拒)—— 这是有意的放宽,已在规范 §11.3-1 写明。
fn b64_decode_bytes(s: &str) -> Result<Vec<u8>, String> {
    let mut acc: u32 = 0;
    let mut bits: u32 = 0;
    let mut out = Vec::with_capacity(s.len() / 4 * 3);
    let mut padding = 0usize;
    let mut data_chars = 0usize;
    for b in s.bytes() {
        // Go 的解码器忽略 CR/LF,base64 常被折行嵌入,沿用同一口径。
        if b == b'\r' || b == b'\n' {
            continue;
        }
        if b == b'=' {
            padding += 1;
            continue;
        }
        if padding > 0 {
            return Err("data after padding".into());
        }
        let v = match b {
            b'A'..=b'Z' => b - b'A',
            b'a'..=b'z' => b - b'a' + 26,
            b'0'..=b'9' => b - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            _ => return Err(format!("illegal base64 character {:?}", b as char)),
        };
        acc = (acc << 6) | v as u32;
        data_chars += 1;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(((acc >> bits) & 0xFF) as u8);
        }
    }
    if padding > 2 {
        return Err("more than two padding characters".into());
    }
    // **一旦出现补位,总字符数就必须是 4 的倍数。**
    //
    // 不加这条的话 `"Zg="`(2 数据字符 + 1 补位)会被当成 `"Zg"` 收下,解出
    // 一个字节的 `"f"` —— 比输入**短**,而且看起来完全合法。截断或损坏的
    // base64 因此不会报 `DecodeError`,而是悄悄给出一个错的值。
    //
    // 省略补位是允许的(RFC 4648 §3.2 的可选项,下面 `bits >= 6` 那条管
    // 它的合法性),但「省略」与「补了一半」是两种不同的形态,不能混。
    let total = data_chars + padding;
    if padding > 0 && !total.is_multiple_of(4) {
        return Err(format!(
            "base64 with padding must be a multiple of 4 characters, got {total}"
        ));
    }
    if bits >= 6 {
        return Err("input length is not a valid base64 length".into());
    }
    if bits > 0 && (acc & ((1u32 << bits) - 1)) != 0 {
        return Err("non-zero bits after the final quantum".into());
    }
    Ok(out)
}

pub fn base64_encode(
    host: &mut dyn StdHost,
    args: Vec<Value>,
) -> wlwl_error::WlwlResult<wlwl_value::Outcome> {
    if args.is_empty() || args.len() > 2 {
        return Err(arity(host, "BASE64_ENCODE", args.len(), 1));
    }
    let s = str_arg(host, "BASE64_ENCODE", &args)?;
    let url_safe = match args.get(1) {
        None => false,
        Some(Value::Boolean(b)) => *b,
        Some(other) => return Err(type_err(host, "BASE64_ENCODE", "boolean", other)),
    };
    Ok(wlwl_value::Outcome::normal(Value::String(
        b64_encode_bytes(s.as_bytes(), url_safe),
    )))
}

pub fn base64_decode(
    host: &mut dyn StdHost,
    args: Vec<Value>,
) -> wlwl_error::WlwlResult<wlwl_value::Outcome> {
    if args.len() != 1 {
        return Err(arity(host, "BASE64_DECODE", args.len(), 1));
    }
    let s = str_arg(host, "BASE64_DECODE", &args)?;
    Ok(wlwl_value::Outcome::normal(finish_decode(
        "BASE64_DECODE",
        b64_decode_bytes(s),
    )))
}

// ── hex ────────────────────────────────────────────────────────────────

fn hex_encode_bytes(s: &str) -> String {
    bytes_to_hex(s.as_bytes())
}

/// 字节 → 小写十六进制(无分隔符)。`HEX_ENCODE` / `HMAC_SHA256` / 两个
/// KDF 共用**同一个**编码器。
///
/// ⚠ 摘要字节**不能**过 `sha256_hex` —— 那是「哈希 + 编码」的复合 API,
/// 对摘要再用一次就是双重哈希(RFC 4231 TC2 的向量在 `encode_contract`
/// 变红时抓到的)。
fn bytes_to_hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(char::from(DIGITS[usize::from(b >> 4)]));
        out.push(char::from(DIGITS[usize::from(b & 0x0F)]));
    }
    out
}

fn hex_decode_bytes(s: &str) -> Result<Vec<u8>, String> {
    let bytes = s.as_bytes();
    if !bytes.len().is_multiple_of(2) {
        return Err(format!("hex string has odd length {}", bytes.len()));
    }
    let mut out = Vec::with_capacity(bytes.len() / 2);
    for pair in bytes.chunks(2) {
        let (Some(hi), Some(lo)) = (hex_nibble(pair[0]), hex_nibble(pair[1])) else {
            return Err(format!(
                "illegal hex digit in {:?}",
                pair.iter().map(|&b| b as char).collect::<String>()
            ));
        };
        out.push((hi << 4) | lo);
    }
    Ok(out)
}

fn hex_nibble(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

pub fn hex_encode(
    host: &mut dyn StdHost,
    args: Vec<Value>,
) -> wlwl_error::WlwlResult<wlwl_value::Outcome> {
    if args.len() != 1 {
        return Err(arity(host, "HEX_ENCODE", args.len(), 1));
    }
    let s = str_arg(host, "HEX_ENCODE", &args)?;
    Ok(wlwl_value::Outcome::normal(Value::String(
        hex_encode_bytes(s),
    )))
}

pub fn hex_decode(
    host: &mut dyn StdHost,
    args: Vec<Value>,
) -> wlwl_error::WlwlResult<wlwl_value::Outcome> {
    if args.len() != 1 {
        return Err(arity(host, "HEX_DECODE", args.len(), 1));
    }
    let s = str_arg(host, "HEX_DECODE", &args)?;
    Ok(wlwl_value::Outcome::normal(finish_decode(
        "HEX_DECODE",
        hex_decode_bytes(s),
    )))
}

// ── percent-encoding (RFC 3986 §2) ─────────────────────────────────────

/// RFC 3986 的 `unreserved`:`ALPHA / DIGIT / "-" / "." / "_" / "~"`。
/// 刻意**不含** `*`(那是 RFC 1738 的旧表)与 `+`。
fn is_unreserved(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~')
}

fn url_encode_bytes(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.as_bytes() {
        if is_unreserved(*b) {
            out.push(*b as char);
        } else {
            out.push('%');
            out.push(
                char::from_digit((b >> 4) as u32, 16)
                    .unwrap()
                    .to_ascii_uppercase(),
            );
            out.push(
                char::from_digit((b & 0x0F) as u32, 16)
                    .unwrap()
                    .to_ascii_uppercase(),
            );
        }
    }
    out
}

fn url_decode_bytes(s: &str) -> Result<Vec<u8>, String> {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            if i + 2 >= bytes.len() {
                return Err("truncated percent-escape at end of input".into());
            }
            let (Some(hi), Some(lo)) = (hex_nibble(bytes[i + 1]), hex_nibble(bytes[i + 2])) else {
                return Err(format!("illegal percent-escape %{}", &s[i + 1..i + 3]));
            };
            out.push((hi << 4) | lo);
            i += 3;
        } else {
            // `+` **不**折成空格:那是 application/x-www-form-urlencoded 的
            // 规则,不是 RFC 3986 的。Go 的 `url.QueryUnescape` 才做这个
            // 替换,`url.PathUnescape` 不做 —— 本成员按 RFC 3986 读。
            out.push(bytes[i]);
            i += 1;
        }
    }
    Ok(out)
}

pub fn url_encode(
    host: &mut dyn StdHost,
    args: Vec<Value>,
) -> wlwl_error::WlwlResult<wlwl_value::Outcome> {
    if args.len() != 1 {
        return Err(arity(host, "URL_ENCODE", args.len(), 1));
    }
    let s = str_arg(host, "URL_ENCODE", &args)?;
    Ok(wlwl_value::Outcome::normal(Value::String(
        url_encode_bytes(s),
    )))
}

pub fn url_decode(
    host: &mut dyn StdHost,
    args: Vec<Value>,
) -> wlwl_error::WlwlResult<wlwl_value::Outcome> {
    if args.len() != 1 {
        return Err(arity(host, "URL_DECODE", args.len(), 1));
    }
    let s = str_arg(host, "URL_DECODE", &args)?;
    Ok(wlwl_value::Outcome::normal(finish_decode(
        "URL_DECODE",
        url_decode_bytes(s),
    )))
}

// ── 哈希(v0.11.3 M2,W-09)──────────────────────────────────────────────

/// HMAC-SHA256(RFC 2104):block size 64;密钥超过块长先哈希再补零。
/// 内 / 外层哈希走字节级 `wlwl_ast::sha256`(公开于本批 —— hex 往返
/// 做内层哈希是浪费)。
fn hmac_sha256_bytes(key: &[u8], data: &[u8]) -> [u8; 32] {
    let mut k = [0u8; 64];
    if key.len() > 64 {
        k[..32].copy_from_slice(&wlwl_ast::sha256::sha256(key));
    } else {
        k[..key.len()].copy_from_slice(key);
    }
    let mut inner: Vec<u8> = k.iter().map(|b| b ^ 0x36).collect();
    inner.extend_from_slice(data);
    let mut outer: Vec<u8> = k.iter().map(|b| b ^ 0x5C).collect();
    outer.extend_from_slice(&wlwl_ast::sha256::sha256(&inner));
    wlwl_ast::sha256::sha256(&outer)
}

/// `SHA256(data) -> STRING`:SHA-256(FIPS 180-4);输入按 UTF-8 字节,
/// 输出**小写十六进制**(64 字符)。
pub fn sha256(host: &mut dyn StdHost, args: Vec<Value>) -> wlwl_error::WlwlResult<Outcome> {
    if args.len() != 1 {
        return Err(arity(host, "SHA256", args.len(), 1));
    }
    let Value::String(s) = &args[0] else {
        return Err(type_err(host, "SHA256", "string", &args[0]));
    };
    Ok(Outcome::normal(Value::String(
        wlwl_ast::sha256::sha256_hex(s.as_bytes()),
    )))
}

/// `HMAC_SHA256(key, data) -> STRING`:HMAC-SHA256(RFC 2104);密钥与
/// 数据都按 UTF-8 字节,输出小写十六进制。
pub fn hmac_sha256(host: &mut dyn StdHost, args: Vec<Value>) -> wlwl_error::WlwlResult<Outcome> {
    if args.len() != 2 {
        return Err(arity(host, "HMAC_SHA256", args.len(), 2));
    }
    let Value::String(key) = &args[0] else {
        return Err(type_err(host, "HMAC_SHA256", "string", &args[0]));
    };
    let Value::String(data) = &args[1] else {
        return Err(type_err(host, "HMAC_SHA256", "string", &args[1]));
    };
    let hex: String = bytes_to_hex(&hmac_sha256_bytes(key.as_bytes(), data.as_bytes()));
    Ok(Outcome::normal(Value::String(hex)))
}

// ── KDF(v0.11.3 M6,W-03)──────────────────────────────────────────────
//
// `PBKDF2_ITER` 与 `ARGON2ID` 解决的是同一个场景:**口令**。它们与
// `SHA256` / `HMAC_SHA256` 的差别不在「强度」,而在**成本可调** ——
// 单向摘要是快的,快到 GPU 上每秒几十亿次;KDF 的全部意义就是把它压到
// 每秒几千次。
//
// 三条硬约束(addendum-04 §3.1):
//
// 1. **参数全部显式,无默认值**。少一个参数就不该有默认 —— 调用方必须
//    能看见并自己组合,否则参数就成了不可审计的魔法数。
// 2. **上下界都是硬边界,越界 `E0030`**。下界取 OWASP 2025 的最小配置,
//    上界防「一个参数打满机器」。不设下界的话「参数全显式」等于
//    「调用方可以传 `iter=1`」,那比不给这个成员更危险。
// 3. **`salt` / `secret` / `assoc` 收十六进制文本**。RFC 8018 / RFC 9106
//    的它们都是**任意字节串**,而 wlwl 的 `STRING` 是 UTF-8 文本 ——
//    同 D13-001 的二进制密钥问题。**盐的自然来源是 `RANDOM_HEX`**。
//
// ⚠ **`algo` 暂为单值不是设计缺陷,是留位。** 签名保留 `algo` 免得日后
// 加值改签名,但当前**只接受 `sha256`**:仓里没有 SHA-512(要支持得手写
// 约 100 行 + 另取 FIPS 180-4 向量),而 OWASP 把 PBKDF2-HMAC-SHA-512 的
// 适用场景写成「FIPS-140 合规时」—— wlwl 提供不了经 FIPS 校验的实现,
// 那条互操作路径对本语言不成立。与模块对 `MD5` / `SHA-1` 的既有立场一致。

/// PBKDF2 迭代数下界:OWASP 2025 对 HMAC-SHA-256 的最小配置。
const PBKDF2_ITER_MIN: i64 = 600_000;
/// PBKDF2 迭代数上界:600 k ≈ 0.3 s ⇒ 10 M ≈ 5 s,单次调用有界。
const PBKDF2_ITER_MAX: i64 = 10_000_000;
/// 派生字节数下界。短输出无意义(RFC 8018)。
const KDF_LEN_MIN: i64 = 1;
/// 派生字节数上界:任何派生用途都用不到 1 KiB,上界只为防「一个参数吃掉内存」。
const KDF_LEN_MAX: i64 = 1_024;
/// Argon2id 内存下界:OWASP 2025(19 MiB)。
const ARGON2_M_MIN: i64 = 19_456;
/// Argon2id 内存上界:1 GiB。刻意**略低于** RFC 9106 §4 的 first-recommended
/// (2 GiB)—— 上界的职责是防 DoS,而 OWASP 底线之上已有 54 倍余量。
const ARGON2_M_MAX: i64 = 1_048_576;
/// Argon2id 轮数下界:OWASP 2025。
const ARGON2_T_MIN: i64 = 2;
/// Argon2id 轮数上界:成本是 `m_cost × t_cost` 的内存流量,1 GiB × 32
/// 已是数十秒级。
const ARGON2_T_MAX: i64 = 32;
/// Argon2id 并行度下界。
const ARGON2_P_MIN: i64 = 1;
/// Argon2id 并行度上界:`p` 会开线程,16 已高于常规服务器核数。
const ARGON2_P_MAX: i64 = 16;
/// Argon2id 标签长度下界:RFC 9106 §3.1 要求 `T` ∈ [4, 2^32-1]。
const ARGON2_TAG_MIN: i64 = 4;
/// Argon2id 盐长度下界:`argon2::MIN_SALT_LEN`。RFC 9106 只「RECOMMENDED 16」,
/// 没有硬下界,但底层实现拒收更短的,越界必须由**我们**报,不能变成内部错误。
const ARGON2_SALT_MIN: usize = 8;

/// 实参错(类型 / 参数越界 / 未知 `algo` / 非法 hex 文本)→ `E0030`。
///
/// **为什么是诊断而不是 `DecodeError` 值**:这几类都是**程序员错误**
/// (把参数配到不安全值、把十六进制写错),不是数据违例。沿用 §11.1
/// 「程序员错误走原生诊断」那条 —— 编码侧才是数据违例。
fn param_err(host: &mut dyn StdHost, detail: String) -> wlwl_error::WlwlError {
    host.diag(ErrorCode::E0030, detail)
}

fn str_arg_at<'a>(
    host: &mut dyn StdHost,
    name: &str,
    label: &str,
    args: &'a [Value],
    idx: usize,
) -> Result<&'a str, wlwl_error::WlwlError> {
    match &args[idx] {
        Value::String(s) => Ok(s.as_str()),
        other => Err(param_err(
            host,
            format!(
                "{name}: expected string for {label}, got {}",
                crate::value_kind(other)
            ),
        )),
    }
}

fn int_arg(
    host: &mut dyn StdHost,
    name: &str,
    label: &str,
    got: &Value,
) -> Result<i64, wlwl_error::WlwlError> {
    match got {
        Value::Integer(i) => Ok(*i),
        other => Err(param_err(
            host,
            format!(
                "{name}: expected integer for {label}, got {}",
                crate::value_kind(other)
            ),
        )),
    }
}

/// 十六进制文本实参。**偶长 + 全 `[0-9a-fA-F]`**,否则 `E0030`
/// (不静默截断、不猜测);空串 = 空字节串(合法)。
fn hex_text_arg(
    host: &mut dyn StdHost,
    name: &str,
    label: &str,
    s: &str,
) -> Result<Vec<u8>, wlwl_error::WlwlError> {
    hex_decode_bytes(s).map_err(|reason| {
        param_err(
            host,
            format!("{name}: {label} must be an even-length hex string, but {reason}"),
        )
    })
}

/// 越界一律 `E0030` —— 不静默接受、不 OOM。
fn bounded(
    host: &mut dyn StdHost,
    name: &str,
    label: &str,
    got: i64,
    lo: i64,
    hi: i64,
    unit: &str,
) -> Result<u32, wlwl_error::WlwlError> {
    if got < lo || got > hi {
        return Err(param_err(
            host,
            format!("{name}: {label} = {got}{unit} is out of range; the allowed range is [{lo}, {hi}]{unit}"),
        ));
    }
    u32::try_from(got).map_err(|_| {
        param_err(
            host,
            format!("{name}: {label} = {got}{unit} does not fit in 32 bits"),
        )
    })
}

/// PBKDF2-HMAC-SHA256(RFC 8018 §5.2):
/// `T_i = U_1 xor U_2 xor ... xor U_c`,`U_1 = PRF(P, S || INT_BE32(i))`、
/// `U_j = PRF(P, U_{j-1})`,`i` 从 **1** 起,输出按 `i` 顺序拼接后取前
/// `out.len()` 字节。
///
/// **刻意手写**:PRF 就是本文件的 `hmac_sha256_bytes`(M2 为 `HMAC_SHA256`
/// 写的,现在有第二个消费者 —— 单一实现,杜绝副本漂移)。RFC 8018 的
/// PBKDF2 本身约 30 行,不值得为它引一个依赖。
fn pbkdf2_hmac_sha256(password: &[u8], salt: &[u8], iter: u32, out: &mut [u8]) {
    const HLEN: usize = 32;
    let blocks = out.len().div_ceil(HLEN);
    for block in 1..=blocks {
        // 末块可能不满 32 字节(输出长度由调用方定,不是 32 的倍数)。
        let start = (block - 1) * HLEN;
        let end = (start + HLEN).min(out.len());
        let chunk = &mut out[start..end];
        let mut prf_input = Vec::with_capacity(salt.len() + 4);
        prf_input.extend_from_slice(salt);
        prf_input.extend_from_slice(&u32::try_from(block).unwrap_or(u32::MAX).to_be_bytes());
        let mut u = hmac_sha256_bytes(password, &prf_input);
        let mut acc = u;
        for _ in 1..iter {
            u = hmac_sha256_bytes(password, &u);
            for (a, b) in acc.iter_mut().zip(u.iter()) {
                *a ^= b;
            }
        }
        chunk.copy_from_slice(&acc[..chunk.len()]);
    }
}

/// `PBKDF2_ITER(password, salt, iter, len, algo) -> STRING`
///
/// 标准 PBKDF2,输出**小写十六进制**。参数全部显式,`algo` 当前只接受
/// `"sha256"`(见本节 ⚠)。`salt` 是十六进制文本。
pub fn pbkdf2_iter(host: &mut dyn StdHost, args: Vec<Value>) -> wlwl_error::WlwlResult<Outcome> {
    const NAME: &str = "PBKDF2_ITER";
    if args.len() != 5 {
        return Err(arity(host, NAME, args.len(), 5));
    }
    let password = str_arg_at(host, NAME, "password", &args, 0)?;
    let salt_text = str_arg_at(host, NAME, "salt", &args, 1)?;
    let iter = int_arg(host, NAME, "iter", &args[2])?;
    let len = int_arg(host, NAME, "len", &args[3])?;
    let algo = str_arg_at(host, NAME, "algo", &args, 4)?;

    if algo != "sha256" {
        return Err(param_err(
            host,
            format!(
                "{NAME}: algo = {algo:?} is not provided; only \"sha256\" is (SHA-512 is not \
                 implemented, and the FIPS-140 interop path it exists for does not apply here)"
            ),
        ));
    }
    let iter = bounded(
        host,
        NAME,
        "iter",
        iter,
        PBKDF2_ITER_MIN,
        PBKDF2_ITER_MAX,
        "",
    )?;
    let len = bounded(host, NAME, "len", len, KDF_LEN_MIN, KDF_LEN_MAX, " bytes")?;
    let salt = hex_text_arg(host, NAME, "salt", salt_text)?;

    let mut out = vec![0u8; len as usize];
    pbkdf2_hmac_sha256(password.as_bytes(), &salt, iter, &mut out);
    Ok(Outcome::normal(Value::String(bytes_to_hex(&out))))
}

/// `ARGON2ID(password, salt, t_cost, m_cost, p_cost, len) -> STRING`
///
/// RFC 9106 Argon2id(v = 19),内存硬、抗 GPU/ASIC。输出**小写十六进制**。
/// `m_cost` 单位 KiB,`salt` 是十六进制文本。
///
/// **无 `secret` / `assoc` 形参** —— 这不是省事,是实测结论:唯一的官方向量
/// (RFC 9106 §5.3,带 `Secret[8]` 与 `Associated data[12]`)在
/// `argon2` 0.6.0 上**复现不出来**(得 `58a04dad…`,权威值是 `0d640df5…`),
/// 而同一 crate 的**无 K/X 路径三方一致**(见本文件 `#[cfg(test)]` 里的
/// `argon2id_matches_an_independent_implementation`)。一个与其他实现
/// 互不认的 `secret` / `assoc`,存下来的哈希**任何别的实现都验不过** ——
/// 那比没有这个参数更危险。详见 `addendum-04` §3.4。
pub fn argon2id(host: &mut dyn StdHost, args: Vec<Value>) -> wlwl_error::WlwlResult<Outcome> {
    const NAME: &str = "ARGON2ID";
    if args.len() != 6 {
        return Err(arity(host, NAME, args.len(), 6));
    }
    let password = str_arg_at(host, NAME, "password", &args, 0)?;
    let salt_text = str_arg_at(host, NAME, "salt", &args, 1)?;
    let t_cost = int_arg(host, NAME, "t_cost", &args[2])?;
    let m_cost = int_arg(host, NAME, "m_cost", &args[3])?;
    let p_cost = int_arg(host, NAME, "p_cost", &args[4])?;
    let len = int_arg(host, NAME, "len", &args[5])?;

    let t_cost = bounded(host, NAME, "t_cost", t_cost, ARGON2_T_MIN, ARGON2_T_MAX, "")?;
    let m_cost = bounded(
        host,
        NAME,
        "m_cost",
        m_cost,
        ARGON2_M_MIN,
        ARGON2_M_MAX,
        " KiB",
    )?;
    let p_cost = bounded(host, NAME, "p_cost", p_cost, ARGON2_P_MIN, ARGON2_P_MAX, "")?;
    let len = bounded(
        host,
        NAME,
        "len",
        len,
        ARGON2_TAG_MIN,
        KDF_LEN_MAX,
        " bytes",
    )?;
    let salt = hex_text_arg(host, NAME, "salt", salt_text)?;
    if salt.len() < ARGON2_SALT_MIN {
        return Err(param_err(
            host,
            format!(
                "{NAME}: salt decodes to {} byte(s), but Argon2id requires at least {ARGON2_SALT_MIN} \
                 (RFC 9106 recommends 16; the natural source is RANDOM_HEX)",
                salt.len()
            ),
        ));
    }

    argon2id_bytes(
        password.as_bytes(),
        &salt,
        t_cost,
        m_cost,
        p_cost,
        len as usize,
    )
    .map(|bytes| Outcome::normal(Value::String(bytes_to_hex(&bytes))))
    .map_err(|e| param_err(host, format!("{NAME}: {e}")))
}

/// Argon2id 原语:字节进、字节出。**成员与单测共用** —— 参数装配只写一遍,
/// 于是「单测里过、成员里不过」这种偏差在结构上就不可能发生。
///
/// 刻意不带上下界检查:原语层要能跑成员层**够不着**的参数
/// (RFC 9106 §5.3 的 `m = 32 KiB` 就是这种),那正是原语层存在的意义。
fn argon2id_bytes(
    password: &[u8],
    salt: &[u8],
    t_cost: u32,
    m_cost: u32,
    p_cost: u32,
    len: usize,
) -> Result<Vec<u8>, argon2::Error> {
    let mut builder = ParamsBuilder::new();
    builder
        .m_cost(m_cost)
        .t_cost(t_cost)
        .p_cost(p_cost)
        .output_len(len);
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, builder.build()?);
    let mut out = vec![0u8; len];
    argon2.hash_password_into(password, salt, &mut out)?;
    Ok(out)
}

// ── CSPRNG 与常数时间比较(v0.11.3 M6,W-04)────────────────────────────
//
// 有了 KDF 还差两件东西才够「安全处理口令」:一块**不可预测**的原料
// (否则盐是常量、token 可预测),和一个**不泄漏比较结果**的比较
// (否则验签时间本身就是侧信道)。这两件就是本节。
//
// ⚠ **熵源只有一个,单向。** `RANDOM_BYTES` / `RANDOM_HEX` 直接读操作
// 系统熵源(`getrandom(2)` / Windows `RtlGenRandom`);`std.rand` 的播种
// 从这里取字节,**不得**反向依赖(addendum-04 §3.3 的 W-02 裁决)。名字
// 要让人看出它是**密码学安全**的 —— 这正是它不叫 `RAND()` 的原因。

/// 读操作系统熵源。**熵源读失败是 `E0060`(IO error)而不是 `E0030`**:
/// 前者是**运行环境**出问题(内核没给熵、沙箱封了系统调用),后者是程序员
/// 把参数写错了。两者的处置完全不同,不该混成一个码。
fn fill_entropy(
    host: &mut dyn StdHost,
    name: &str,
    buf: &mut [u8],
) -> Result<(), wlwl_error::WlwlError> {
    getrandom::fill(buf).map_err(|e| {
        host.diag(
            ErrorCode::E0060,
            format!("{name}: the operating system entropy source failed ({e})"),
        )
    })
}

/// 随机缓冲区的长度实参。`n < 0` 报 `E0030`;`n = 0` **合法**且返回空 ——
/// 长度 0 是一个正常请求,不是错误。
///
/// **刻意不设上界**:两个 KDF 成员设上界是因为「一个参数能打满机器」
/// 是**安全**问题(把 `iter` 配小);而随机缓冲区调大只是多花点时间和内存,
/// 一个 wlwl 程序本来就能建更大的数组。这里设上界只会凭空多一个陷阱。
fn random_len(
    host: &mut dyn StdHost,
    name: &str,
    got: i64,
) -> Result<usize, wlwl_error::WlwlError> {
    if got < 0 {
        return Err(param_err(
            host,
            format!("{name}: n = {got} is negative; a buffer length cannot be negative"),
        ));
    }
    usize::try_from(got).map_err(|_| {
        param_err(
            host,
            format!("{name}: n = {got} does not fit in this platform's address space"),
        )
    })
}

/// `RANDOM_BYTES(n) -> ARRAY`(元素是 `INTEGER` 0–255)
///
/// **为什么返回整数数组而不是 `STRING`**:名字说的是「字节」,而 wlwl
/// 没有字节类型。硬塞进 `STRING` 只能把非法 UTF-8 有损替换成 `U+FFFD` ——
/// §11.3-3 已经把「不做有损替换」立成规范条款,§11.3-3 的注释写得很直白:
/// 「与其有损地替换骗调用方,不如报 `DecodeError`」。整数数组是本语言
/// **唯一**能无损承载任意字节串的形态。
/// ⇒ **要文本就用 `RANDOM_HEX`** —— 那才是 KDF 盐的来源。
///
/// **无上界**(见 `random_len`);`n = 0` 返回空数组。
pub fn random_bytes(host: &mut dyn StdHost, args: Vec<Value>) -> wlwl_error::WlwlResult<Outcome> {
    const NAME: &str = "RANDOM_BYTES";
    if args.len() != 1 {
        return Err(arity(host, NAME, args.len(), 1));
    }
    let n = int_arg(host, NAME, "n", &args[0])?;
    let n = random_len(host, NAME, n)?;
    let mut buf = vec![0u8; n];
    fill_entropy(host, NAME, &mut buf)?;
    Ok(Outcome::normal(Value::Array(
        buf.into_iter()
            .map(|b| Value::Integer(i64::from(b)))
            .collect(),
    )))
}

/// `RANDOM_HEX(n) -> STRING`
///
/// `RANDOM_BYTES` 的十六进制文本形态(2 `n` 个小写字符)。**这是本语言
/// 里生成 KDF 盐的正确方式** —— 两个 KDF 的 `salt` 实参收十六进制文本,
/// 原因是 RFC 的盐是任意字节串而 `STRING` 是 UTF-8 文本(§11.2 注)。
pub fn random_hex(host: &mut dyn StdHost, args: Vec<Value>) -> wlwl_error::WlwlResult<Outcome> {
    const NAME: &str = "RANDOM_HEX";
    if args.len() != 1 {
        return Err(arity(host, NAME, args.len(), 1));
    }
    let n = int_arg(host, NAME, "n", &args[0])?;
    let n = random_len(host, NAME, n)?;
    let mut buf = vec![0u8; n];
    fill_entropy(host, NAME, &mut buf)?;
    Ok(Outcome::normal(Value::String(bytes_to_hex(&buf))))
}

/// `TIMING_SAFE_EQ(a, b) -> BOOLEAN`
///
/// **常数时间**比较两段 `STRING`(按 UTF-8 字节)。**永不失败** ——
/// 它的返回值是 `BOOLEAN`,不是「相等 / 不相等」两种错误。
///
/// **长度不等时也走完整比较**,这是本成员存在的全部理由:朴素写法
/// `a.len() == b.len() && ...` 在长度不等时**提前返回**,于是「两个值长度
/// 不同」这件事可以从耗时上被观察到 —— 攻击者据此逐字节爆破长度,再
/// 爆破内容。本实现走 `max(len_a, len_b)` 步并把长度差折进同一个累加器,
/// **没有任何一处按比较结果分支**。
///
/// 诚实边界:**耗时仍然依赖长度**(比较多少字节就要走多少步),这无法避免
/// 也不需要避免 —— 要隐藏的是「哪几个字节不同」和「是否相等」,不是长度。
pub fn timing_safe_eq(host: &mut dyn StdHost, args: Vec<Value>) -> wlwl_error::WlwlResult<Outcome> {
    const NAME: &str = "TIMING_SAFE_EQ";
    if args.len() != 2 {
        return Err(arity(host, NAME, args.len(), 2));
    }
    let a = str_arg_at(host, NAME, "a", &args, 0)?;
    let b = str_arg_at(host, NAME, "b", &args, 1)?;
    Ok(Outcome::normal(Value::Boolean(timing_safe_eq_bytes(
        a.as_bytes(),
        b.as_bytes(),
    ))))
}

/// 常数时间相等。**逐字节累积差异,不按结果分支**。
///
/// 缺失的一侧按 `0` 补齐(而不是跳过)——跳过就是提前返回,那正是要避免的。
/// 长度差最后折进同一个 `u8` 累加器,于是函数里没有一个 `if`。
fn timing_safe_eq_bytes(a: &[u8], b: &[u8]) -> bool {
    let mut acc: u8 = 0;
    for i in 0..a.len().max(b.len()) {
        acc |= a.get(i).copied().unwrap_or(0) ^ b.get(i).copied().unwrap_or(0);
    }
    acc |= u8::from((a.len() ^ b.len()) != 0);
    acc == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    /// RFC 4648 §10 的官方表,逐条照抄,不自造。
    #[test]
    fn rfc4648_10_vectors() {
        let cases: &[(&str, &str)] = &[
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ];
        for (plain, encoded) in cases {
            assert_eq!(b64_encode_bytes(plain.as_bytes(), false), *encoded);
            assert_eq!(
                b64_decode_bytes(encoded).unwrap(),
                plain.as_bytes(),
                "round trip failed for {plain:?}"
            );
        }
    }

    /// RFC 4648 §10 表的最后两行 —— 唯一一对会用到 `+/` 与 `-_` 的输入。
    /// 0xfb 0xff = `111110 111111 1111(pad)` ⇒ 索引 62 / 63 / 60。
    ///
    /// 这个向量**在语言层不可达**:0xFB 0xFF 不是合法 UTF-8,而 wlwl 的
    /// `STRING` 是 UTF-8。语言层对应的可观察向量是 U+083F(E0 A0 BF →
    /// `4KC/` / `4KC_`),那条锁在 `encode_contract.rs` 里。
    #[test]
    fn rfc4648_10_url_safe_alphabet() {
        assert_eq!(b64_encode_bytes(b"\xfb\xff", false), "+/8=");
        assert_eq!(b64_encode_bytes(b"\xfb\xff", true), "-_8=");
        assert_eq!(b64_decode_bytes("+/8=").unwrap(), vec![0xfb, 0xff]);
        assert_eq!(b64_decode_bytes("-_8=").unwrap(), vec![0xfb, 0xff]);
    }

    #[test]
    fn decode_rejects_malformed() {
        assert!(b64_decode_bytes("!!!").is_err());
        assert!(b64_decode_bytes("Zm9v=Yg==").is_err(), "data after padding");
        assert!(b64_decode_bytes("Z===").is_err(), "too much padding");
        assert!(b64_decode_bytes("Zh").is_err(), "1 leftover char");
        assert!(b64_decode_bytes("Zm9vYmFyZ").is_err(), "5 leftover bits");
    }

    /// 补位数量必须与末组长度匹配。少了这条,`"Zg="` 会被当成 `"Zg"` 收下,
    /// 解出比输入短一个字节的 `"f"` —— 截断/损坏的 base64 静默给错值,
    /// 而不是报 `DecodeError`。
    #[test]
    fn half_padded_input_is_rejected() {
        assert_eq!(b64_decode_bytes("Zg==").unwrap(), b"f".to_vec());
        assert_eq!(b64_decode_bytes("Zm8=").unwrap(), b"fo".to_vec());
        assert!(
            b64_decode_bytes("Zg=").is_err(),
            "2 data chars + 1 pad is neither padded nor unpadded"
        );
        assert!(
            b64_decode_bytes("Zm8==").is_err(),
            "3 data chars + 2 pad is 5 characters, not a multiple of 4"
        );
        assert!(b64_decode_bytes("Z").is_err(), "1 data char, no pad");
    }

    /// 省略补位是允许的(RFC 4648 §3.2 的可选项),与「补了一半」是两种形态。
    #[test]
    fn unpadded_input_is_accepted() {
        assert_eq!(b64_decode_bytes("Zg").unwrap(), b"f".to_vec());
        assert_eq!(b64_decode_bytes("Zm8").unwrap(), b"fo".to_vec());
        assert_eq!(b64_decode_bytes("Zm9vYmFy").unwrap(), b"foobar".to_vec());
    }

    #[test]
    fn decode_ignores_line_breaks() {
        assert_eq!(
            b64_decode_bytes("Zm9v\r\nYmFy").unwrap(),
            b"foobar".to_vec()
        );
    }

    #[test]
    fn hex_is_lowercase_and_case_insensitive_on_the_way_back() {
        assert_eq!(hex_encode_bytes("Hi"), "4869");
        assert_eq!(hex_decode_bytes("4869").unwrap(), b"Hi".to_vec());
        assert_eq!(hex_decode_bytes("4a4F").unwrap(), b"JO".to_vec());
    }

    #[test]
    fn hex_decode_rejects_bad_input() {
        assert!(hex_decode_bytes("abc").is_err(), "odd length");
        assert!(hex_decode_bytes("zz").is_err(), "not hex");
    }

    /// RFC 3986 §2.1:`unreserved = ALPHA / DIGIT / "-" / "." / "_" / "~"`。
    #[test]
    fn url_encoding_follows_rfc3986_unreserved_set() {
        assert_eq!(url_encode_bytes("aZ09-._~"), "aZ09-._~");
        assert_eq!(url_encode_bytes(" "), "%20");
        assert_eq!(url_encode_bytes("+"), "%2B");
        assert_eq!(url_encode_bytes("&="), "%26%3D");
        assert_eq!(url_encode_bytes("%"), "%25");
    }

    /// `+` 不是空格 —— 那是 form 编码,不是 RFC 3986(§11.3-2)。
    #[test]
    fn plus_is_not_a_space() {
        assert_eq!(url_decode_bytes("a+b").unwrap(), b"a+b".to_vec());
    }

    #[test]
    fn url_decode_rejects_bad_escapes() {
        assert!(url_decode_bytes("a%2").is_err(), "truncated");
        assert!(url_decode_bytes("a%zz").is_err(), "non-hex digits");
    }

    /// 解码出非 UTF-8 必须走 `DecodeError`,不能有损替换(§11.3-3)。
    #[test]
    fn non_utf8_decode_becomes_a_decode_error() {
        let v = finish_decode("BASE64_DECODE", b64_decode_bytes("/w=="));
        let Value::Err(payload) = v else {
            panic!("0xFF is not valid UTF-8 and must not be a STRING")
        };
        let Value::Dict(entries) = *payload else {
            panic!("payload must be a DICT")
        };
        let kind = entries
            .iter()
            .find(|(k, _)| matches!(k, Value::String(s) if s == "kind"))
            .map(|(_, v)| v.clone());
        assert_eq!(kind, Some(Value::String("DecodeError".into())));
    }

    // ── 哈希向量(v0.11.3 M2)—— 期望值取自 FIPS 180-4 / RFC 4231 原文,
    //    UTF-8 与超长键向量取自 Python hashlib 的独立交叉核对(D13-001),
    //    不是实现输出。

    #[test]
    fn sha256_fips_vectors() {
        assert_eq!(
            wlwl_ast::sha256::sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            wlwl_ast::sha256::sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            wlwl_ast::sha256::sha256_hex(
                b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"
            ),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
    }

    #[test]
    fn sha256_input_is_utf8_bytes() {
        assert_eq!(
            wlwl_ast::sha256::sha256_hex("世界".as_bytes()),
            "33650a369521ec29f2e26c43d25967535bcb26436755f536735d1ef6e84a1ec5"
        );
    }

    #[test]
    fn hmac_rfc4231_tc2_ascii() {
        // TC1 / TC3 的密钥是二进制(0x0b×20 / 0xaa×20),wlwl 的 STRING 是
        // UTF-8 文本,表达不了 —— 见 D13-001。
        assert_eq!(
            hmac_sha256_bytes(b"Jefe", b"what do ya want for nothing?"),
            [
                0x5b, 0xdc, 0xc1, 0x46, 0xbf, 0x60, 0x75, 0x4e, 0x6a, 0x04, 0x24, 0x26, 0x08, 0x95,
                0x75, 0xc7, 0x5a, 0x00, 0x3f, 0x08, 0x9d, 0x27, 0x39, 0x83, 0x9d, 0xec, 0x58, 0xb9,
                0x64, 0xec, 0x38, 0x43,
            ]
        );
    }

    #[test]
    fn hmac_rfc4231_tc7_key_over_block_size_is_hashed_first() {
        // RFC 4231 TC7:131 字节 0xAA 密钥(> 块长 64,先哈希再补零)+
        // ASCII 数据;摘要 = RFC 逐字。**字节级单元测试** —— wlwl 的
        // STRING 表达不了二进制密钥,wlwl 层的对应口径见 D13-001。
        let key = [0xAAu8; 131];
        let data: &[u8] = b"This is a test using a larger than block-size key and a larger than block-size data. The key needs to be hashed before being used by the HMAC algorithm.";
        // ⚠ 摘要字节用 `bytes_to_hex` 即可;**不能**再过一次 `sha256_hex` ——
        // 那是「哈希 + 编码」的复合 API,对摘要再用一次就是双重哈希(本测试
        // 的第一次提交就栽在这里,左值 cc53540e… 正是 sha256(9b09ffa7… 的字节))。
        assert_eq!(
            bytes_to_hex(&hmac_sha256_bytes(&key, data)),
            "9b09ffa71b942fcb27635fbcd5b0e944bfdc63644f0713938a7f51535c3a35e2"
        );
    }

    // ── KDF 向量(v0.11.3 M6 / W-03)────────────────────────────────────
    //
    // 期望值**一律不是本实现的输出**:
    //   · PBKDF2 前两条 = RFC 7914 §11 原文,另经 Python `hashlib.pbkdf2_hmac`
    //     逐字节复算;
    //   · PBKDF2 第三条与 Argon2id 三条 = 独立第二实现交叉核对(下注)。

    /// RFC 7914 §11 的两个 PBKDF2-HMAC-SHA-256 向量,**逐字冻结**。
    ///
    /// ⚠ 它们的 `c` = 1 与 80 000 **低于** OWASP 下界 600 000 ⇒ **成员层
    /// 走这两个参数会被自己的 `E0030` 拒掉**(那正是 `encode_contract` 里的
    /// 两条反例)。它们住在原语层证明**算法实现**对,与成员参数界无关 ——
    /// 与 M3-1 处理 `SHA256` 的二进制向量同款做法。
    #[test]
    fn pbkdf2_rfc7914_11_vectors() {
        let mut out = [0u8; 64];
        pbkdf2_hmac_sha256(b"passwd", b"salt", 1, &mut out);
        assert_eq!(
            bytes_to_hex(&out),
            concat!(
                "55ac046e56e3089fec1691c22544b605f94185216dde0465e68b9d57c20dacbc",
                "49ca9cccf179b645991664b39d77ef317c71b845b1e30bd509112041d3a19783",
            )
        );

        let mut out = [0u8; 64];
        pbkdf2_hmac_sha256(b"Password", b"NaCl", 80_000, &mut out);
        assert_eq!(
            bytes_to_hex(&out),
            concat!(
                "4ddcd8f60b98be21830cee5ef22701f9641a4418d04c0414aeff08876b34ab5",
                "6a1d425a1225833549adb841b51c9b3176a272bdebba1d078478f62b397f33c8d",
            )
        );
    }

    /// 输出长度跨过 32 字节边界时,第二块要用 `S || INT_BE32(2)` **重新起链**,
    /// 而不是接着第一块算(RFC 8018 §5.2 的块索引从 1 起)。
    ///
    /// 这条锁的是「前 32 字节与 `len = 32` 时相同、且后 32 字节与之不同」——
    /// 一个把 `i` 写成从 0 起或忘记换块的实现会在这里现形。
    #[test]
    fn pbkdf2_block_counter_starts_at_one() {
        let mut short = [0u8; 32];
        pbkdf2_hmac_sha256(b"passwd", b"salt", 1, &mut short);
        let mut long = [0u8; 64];
        pbkdf2_hmac_sha256(b"passwd", b"salt", 1, &mut long);
        assert_eq!(
            &long[..32],
            &short[..],
            "block 1 must not depend on the requested output length"
        );
        assert_ne!(
            &long[32..],
            &short[..],
            "block 2 must be a different chain, not a repeat of block 1"
        );
    }

    /// 落在我们上下界内的 PBKDF2 向量(`c = 600 000` = OWASP 2025 下界),
    /// 期望值来自 Python `hashlib.pbkdf2_hmac` 的独立复算。
    ///
    /// ⚠ **实测更正**:`addendum-04` §3.2(a) 曾把这条冻结成
    /// `aaf96b2b…`,那是**错的** —— 同一组参数经 `hashlib` 复算是
    /// `1074be24…`(本实现与之逐字节一致)。salt 在这里是**十六进制解码后**
    /// 的 4 字节 `73 61 6c 74`,不是 ASCII 串 `73616c74`;把未解码的文本
    /// 当盐就会得到另一个值。计划文档已改。
    #[test]
    fn pbkdf2_owasp_floor_vector() {
        let mut out = [0u8; 32];
        pbkdf2_hmac_sha256(
            b"passwd",
            &hex_decode_bytes("73616c74").unwrap(),
            600_000,
            &mut out,
        );
        assert_eq!(
            bytes_to_hex(&out),
            "1074be241b7be078a90369fae10cdc0394cf64a6780904421bd79c51fd372db0"
        );
    }

    /// Argon2id 与**独立第二实现**逐字节一致。
    ///
    /// 期望值由一份照 **RFC 9106 + 参考实现 C 源码**
    /// (`P-H-C/phc-winner-argon2` 的 `core.c` / `ref.c`)独立写成的 Python
    /// 产生;而那份实现先**逐块复现了参考实现自己的 KAT**
    /// (`kats/argon2id`,即 RFC 9106 §5.3,含三轮全部中间分块与最终 Tag)。
    /// 这就是 D13-001 那条纪律在 Argon2 上的形状:**第二个实现**,不是自己
    /// 的输出。推导与全部读数见 `addendum-04` §3.4。
    #[test]
    fn argon2id_matches_an_independent_implementation() {
        // A:m = 19 456 KiB(OWASP 下界)、t = 2、p = 1、tag 32 B、盐 8 B
        //    —— 成员层的**下界参数**本身(OWASP 最小配置),最该被钉住的一组。
        assert_eq!(
            bytes_to_hex(
                &argon2id_bytes(
                    b"passwd",
                    &hex_decode_bytes("0001020304050607").unwrap(),
                    2,
                    19_456,
                    1,
                    32
                )
                .unwrap()
            ),
            "b95d51b0625a6b6013d3e7b024d8f34efb064f93996fea539965c3852ccbafa3"
        );

        // B:t = 3、p = 2(多 lane)—— `p_cost` 是显式参数,得有向量钉住它生效
        assert_eq!(
            bytes_to_hex(
                &argon2id_bytes(
                    b"passwd",
                    &hex_decode_bytes("0102030405060708").unwrap(),
                    3,
                    19_456,
                    2,
                    32
                )
                .unwrap()
            ),
            "4deaf7554232beb810f790184b3f0051cd30a867106611088f85a9966390455d"
        );

        // C:口令与盐都是非 ASCII / 全零,证明文本域走的是 UTF-8 字节
        assert_eq!(
            bytes_to_hex(&argon2id_bytes("世界".as_bytes(), &[0u8; 16], 2, 19_456, 1, 32).unwrap()),
            "a5e4dd3d628296d67d7aa747d30d956b99387c6a0e6fa4af14a0b63cd5da417d"
        );
    }

    /// `p_cost` 必须真的改变结果 —— 参数是**显式**的,不是摆设。
    /// (无 `parallel` feature 时 `p_cost` 仍走多 lane 语义,只是不并发。)
    #[test]
    fn argon2id_p_cost_changes_the_tag() {
        let salt = [0u8; 16];
        let a = argon2id_bytes(b"passwd", &salt, 2, 19_456, 1, 32).unwrap();
        let b = argon2id_bytes(b"passwd", &salt, 2, 19_456, 2, 32).unwrap();
        assert_ne!(a, b, "p_cost had no effect on the tag");
    }

    // ── CSPRNG 与常数时间比较(v0.11.3 M6 / W-04)─────────────────────
    //
    // 这两组断言**测不了「值」只能测「分布」** —— 期望一个随机值等于某个
    // 常数,是把随机源钉死。所以它们断言的是**不变量**:不重复、分布接近
    // 均匀、长度不等不提前返回。

    /// 10 万次抽样**不得重复**。
    ///
    /// 这是能对 CSPRNG 提的最强单条断言:128 bit 抽样撞车的概率约
    /// 2⁻⁸⁰,所以「一条都不撞」几乎必然成立;而**任何**退化成计数器、
    /// 时间戳、定长 PRNG 未换种子的实现都会立刻撞出来。
    #[test]
    fn csprng_samples_never_repeat() {
        let mut seen = std::collections::HashSet::with_capacity(100_000);
        for _ in 0..100_000 {
            let mut buf = [0u8; 16];
            getrandom::fill(&mut buf).expect("OS entropy");
            assert!(
                seen.insert(buf),
                "the entropy source produced a duplicate 128-bit sample"
            );
        }
    }

    /// 卡方粗检:10 万个样本的**首字节**在 256 个桶里应接近均匀。
    ///
    /// 阈值为什么敢取 400:均匀源的期望卡方 ≈ 255(自由度),99.9 分位约
    /// 310 ⇒ 400 的误报率远低于万分之一;而**任何**真缺陷(常量源、
    /// 只有两个取值、双峰)算出来的卡方在 10⁷ 量级 —— 两者差四个数量级,
    /// 所以这个阈值不是在赌运气,是在给「缺陷」留出无法跨越的量级差。
    #[test]
    fn csprng_first_byte_is_roughly_uniform() {
        const N: usize = 100_000;
        let mut buckets = [0u32; 256];
        for _ in 0..N {
            let mut buf = [0u8; 1];
            getrandom::fill(&mut buf).expect("OS entropy");
            buckets[usize::from(buf[0])] += 1;
        }
        let expected = N as f64 / 256.0;
        let chi2: f64 = buckets
            .iter()
            .map(|&obs| {
                let d = f64::from(obs) - expected;
                d * d / expected
            })
            .sum();
        assert!(
            chi2 < 400.0,
            "first-byte distribution looks biased: chi2 = {chi2:.1} over 255 dof \
             (a real defect lands around 1e7; a healthy source around 255)"
        );
    }

    /// `RANDOM_HEX` 的长度必须是 `2n`,且只含小写 hex —— 契约表从语言层
    /// 钉形状,这里从原语层钉编码。
    #[test]
    fn random_hex_is_lowercase_and_twice_as_long() {
        for n in [0usize, 1, 16, 32] {
            let mut buf = vec![0u8; n];
            getrandom::fill(&mut buf).expect("OS entropy");
            let hex = bytes_to_hex(&buf);
            assert_eq!(hex.len(), n * 2, "n = {n}");
            assert!(
                hex.bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
                "n = {n}: hex must be lowercase, got {hex:?}"
            );
        }
    }

    #[test]
    fn timing_safe_eq_semantics() {
        assert!(timing_safe_eq_bytes(b"", b""));
        assert!(timing_safe_eq_bytes(b"a", b"a"));
        assert!(!timing_safe_eq_bytes(b"a", b"b"));
        assert!(!timing_safe_eq_bytes(b"a", b"ab"));
        assert!(!timing_safe_eq_bytes(b"ab", b"a"));
        assert!(!timing_safe_eq_bytes(b"", b"\0"));
        // 差在**首字节**与差在**末字节**结果相同,且都不提前返回。
        let base = b"abcdefgh".as_slice();
        let mut first = base.to_vec();
        first[0] = b'z';
        let mut last = base.to_vec();
        last[7] = b'z';
        assert!(!timing_safe_eq_bytes(base, &first));
        assert!(!timing_safe_eq_bytes(base, &last));
        // 高位字节不能因为「符号位」被当成相等。
        assert!(!timing_safe_eq_bytes(&[0x80], &[0x00]));
    }

    /// **长度不等时不得提前返回** —— 本成员存在的理由,机制断言。
    ///
    /// 断言的是**下界**而不是「两路径耗时相近」:朴素实现
    /// `a.len() == b.len() && …` 在长度不等时立刻返回,耗时是这个循环的
    /// 百万分之一量级,于是「短 vs 长」远低于 20% 的门槛而变红;而本实现
    /// 走 `max(len)` 步,必然过线。**下界断言在负载不均的 CI 上也稳** ——
    /// 机器慢只会让两边一起慢。
    #[test]
    fn length_mismatch_does_not_short_circuit() {
        const N: usize = 1 << 20;
        let long = vec![b'a'; N];
        let warmup = 3;
        let runs = 5;
        let mut short_long = f64::INFINITY;
        let mut same_length = f64::INFINITY;
        for _ in 0..warmup {
            std::hint::black_box(timing_safe_eq_bytes(&long, &long));
        }
        for _ in 0..runs {
            let t0 = std::time::Instant::now();
            std::hint::black_box(timing_safe_eq_bytes(b"", &long));
            short_long = short_long.min(t0.elapsed().as_secs_f64());
            let t0 = std::time::Instant::now();
            std::hint::black_box(timing_safe_eq_bytes(&long, &long));
            same_length = same_length.min(t0.elapsed().as_secs_f64());
        }
        let ratio = short_long / same_length;
        assert!(
            ratio > 0.2,
            "length mismatch returned early: {short_long:.6?}s vs {same_length:.6?}s \
             for the same max length (ratio {ratio:.3}; an early return gives ~0.001)"
        );
    }

    /// **内容不同不得改变耗时** —— 比较结果本身不能从时钟上看出来。
    /// 同样是下界断言(取 min、门槛宽松),只要求「差在首字节」与「完全相等」
    /// 的耗时落在同一量级。
    #[test]
    fn content_does_not_change_the_timing() {
        const N: usize = 1 << 20;
        let long = vec![b'a'; N];
        let mut differs_at_front = long.clone();
        differs_at_front[0] = b'z';
        let warmup = 3;
        let runs = 5;
        let mut equal = f64::INFINITY;
        let mut different = f64::INFINITY;
        for _ in 0..warmup {
            std::hint::black_box(timing_safe_eq_bytes(&long, &long));
        }
        for _ in 0..runs {
            let t0 = std::time::Instant::now();
            std::hint::black_box(timing_safe_eq_bytes(&long, &long));
            equal = equal.min(t0.elapsed().as_secs_f64());
            let t0 = std::time::Instant::now();
            std::hint::black_box(timing_safe_eq_bytes(&long, &differs_at_front));
            different = different.min(t0.elapsed().as_secs_f64());
        }
        let ratio = different / equal;
        assert!(
            (0.2..5.0).contains(&ratio),
            "content changed the timing: equal {equal:.6?}s vs different {different:.6?}s \
             (ratio {ratio:.3}; a per-byte early exit would be far below 0.2)"
        );
    }
}
