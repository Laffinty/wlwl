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

use crate::{ModuleSpec, StdFn};
use wlwl_error::ErrorCode;
use wlwl_value::{StdHost, Value};

pub static SPEC: ModuleSpec = ModuleSpec {
    path: "wlwl:std.encode",
    functions: &[
        ("BASE64_ENCODE", base64_encode as StdFn),
        ("BASE64_DECODE", base64_decode as StdFn),
        ("HEX_ENCODE", hex_encode as StdFn),
        ("HEX_DECODE", hex_decode as StdFn),
        ("URL_ENCODE", url_encode as StdFn),
        ("URL_DECODE", url_decode as StdFn),
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
    let mut out = String::with_capacity(s.len() * 2);
    for b in s.as_bytes() {
        out.push(char::from_digit((b >> 4) as u32, 16).unwrap());
        out.push(char::from_digit((b & 0x0F) as u32, 16).unwrap());
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
}
