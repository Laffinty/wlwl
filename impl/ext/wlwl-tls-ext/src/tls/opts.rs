//! `tls` 命名空间的选项解析（`LISTEN` / `CONNECT` 的 `opts`）。
//!
//! # 口径
//!
//! - ⚠ **未知选项名 ⇒ 失败**（不静默忽略：拼错的上界会变成「没有上界」——
//!   与 std 的 `opts` 口径一致）。
//! - ⚠ `opts` 必须是 `DICT` 或 `NULL` —— WLWL 的 `[]` 是**空数组**，不是空字典。
//! - ⚠ **证书校验默认开**、**TLS 版本下限默认 1.2** —— 两个缺省值都是
//!   「安全的那一边」。

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde_json::{Map, Value as Json};

use crate::args::{opt_bool, opt_int, opt_str, reject_unknown};
use crate::bytes::type_name;

/// TLS 版本下限。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MinVersion {
    /// TLS 1.2（**缺省**）。
    V12,
    /// TLS 1.3。
    V13,
}

/// TLS 1.2 + 1.3 的版本列表（**偏好顺序**：1.3 在前）。
static VERSIONS_12: &[&rustls::SupportedProtocolVersion] =
    &[&rustls::version::TLS13, &rustls::version::TLS12];
/// 只有 TLS 1.3 的版本列表。
static VERSIONS_13: &[&rustls::SupportedProtocolVersion] = &[&rustls::version::TLS13];

impl MinVersion {
    /// 从文本解析（`"1.2"` / `"1.3"`）。
    pub fn parse(text: &str, op: &str) -> Result<MinVersion, String> {
        match text.trim() {
            "1.2" | "TLS1.2" | "tls1.2" => Ok(MinVersion::V12),
            "1.3" | "TLS1.3" | "tls1.3" => Ok(MinVersion::V13),
            other => Err(format!(
                "{op}: unknown min_version '{other}' (expected '1.2' or '1.3')"
            )),
        }
    }

    /// 这个下限对应的版本列表（**偏好顺序**：1.3 在前）。
    pub fn versions(self) -> &'static [&'static rustls::SupportedProtocolVersion] {
        match self {
            MinVersion::V12 => VERSIONS_12,
            MinVersion::V13 => VERSIONS_13,
        }
    }
}

/// SNI 条目的来源。
#[derive(Debug, Clone)]
pub enum SniSource {
    /// 一个 PEM 文件：里面可以有证书链，**也可以**有私钥。
    File(PathBuf),
    /// 证书与私钥分开两个文件。
    Pair { cert: PathBuf, key: PathBuf },
}

/// `LISTEN` 的选项。
#[derive(Debug, Clone)]
pub struct ListenOpts {
    /// 服务器证书链的 PEM 路径。
    pub cert: PathBuf,
    /// 服务器私钥的 PEM 路径。
    pub key: PathBuf,
    /// 客户端证书的信任根（给了 ⇒ **要求**客户端证书）。
    pub ca: Option<PathBuf>,
    /// TLS 版本下限（缺省 1.2）。
    pub min_version: MinVersion,
    /// ALPN 协议名列表。
    pub alpn: Vec<Vec<u8>>,
    /// 按 SNI 主机名选证书。
    pub sni: BTreeMap<String, SniSource>,
    /// 握手预算（毫秒；缺省 30 秒）。
    pub handshake_timeout_ms: u64,
}

/// `CONNECT` 的选项。
#[derive(Debug, Clone)]
pub struct ConnectOpts {
    /// 信任根（不给 ⇒ 用 `webpki-roots` 的 Mozilla 根库）。
    pub ca: Option<PathBuf>,
    /// SPKI SHA-256 指纹（hex）—— 公钥固定。
    pub pin: Option<String>,
    /// 客户端证书的 PEM 路径。
    pub client_cert: Option<PathBuf>,
    /// 客户端私钥的 PEM 路径。
    pub client_key: Option<PathBuf>,
    /// TLS 版本下限（缺省 1.2）。
    pub min_version: MinVersion,
    /// ⚠ **是否校验服务器证书**（缺省 **true** = 校验）。
    pub verify: bool,
    /// SNI / 证书校验用的名字（缺省 = `host`）。
    pub server_name: Option<String>,
    /// ALPN 协议名列表。
    pub alpn: Vec<Vec<u8>>,
    /// 连接预算（毫秒；缺省 30 秒）。
    pub connect_timeout_ms: u64,
}

/// `LISTEN` 允许的选项名。
pub const LISTEN_OPTIONS: &[&str] = &[
    "cert",
    "key",
    "ca",
    "min_version",
    "alpn",
    "sni",
    "handshake_timeout_ms",
];

/// `CONNECT` 允许的选项名。
pub const CONNECT_OPTIONS: &[&str] = &[
    "ca",
    "pin",
    "client_cert",
    "client_key",
    "min_version",
    "verify",
    "server_name",
    "alpn",
    "connect_timeout_ms",
];

/// 缺省的握手 / 连接预算（毫秒）。
pub const DEFAULT_TIMEOUT_MS: u64 = 30_000;

/// 解析 `LISTEN` 的选项。
pub fn parse_listen(opts: &Map<String, Json>, op: &str) -> Result<ListenOpts, String> {
    reject_unknown(opts, LISTEN_OPTIONS, op)?;
    let cert = required_path(opts, "cert", op)?;
    let key = required_path(opts, "key", op)?;
    let ca = optional_path(opts, "ca", op)?;
    let min_version = match opt_str(opts, "min_version", op)? {
        Some(text) => MinVersion::parse(&text, op)?,
        None => MinVersion::V12,
    };
    let alpn = parse_alpn(opts, op)?;
    let sni = parse_sni(opts, op)?;
    let handshake_timeout_ms = match opt_int(opts, "handshake_timeout_ms", op)? {
        Some(ms) if ms < 0 => {
            return Err(format!(
                "{op}: option 'handshake_timeout_ms' must not be negative"
            ))
        }
        Some(ms) => ms as u64,
        None => DEFAULT_TIMEOUT_MS,
    };
    Ok(ListenOpts {
        cert,
        key,
        ca,
        min_version,
        alpn,
        sni,
        handshake_timeout_ms,
    })
}

/// 解析 `CONNECT` 的选项。
pub fn parse_connect(opts: &Map<String, Json>, op: &str) -> Result<ConnectOpts, String> {
    reject_unknown(opts, CONNECT_OPTIONS, op)?;
    let ca = optional_path(opts, "ca", op)?;
    let pin = opt_str(opts, "pin", op)?;
    if let Some(pin) = &pin {
        // 早失败：指纹格式不对就别去连了。
        crate::bytes::unhex(pin).map_err(|e| format!("{op}: option 'pin' is not a hex digest: {e}"))?;
    }
    let client_cert = optional_path(opts, "client_cert", op)?;
    let client_key = optional_path(opts, "client_key", op)?;
    if client_cert.is_some() && client_key.is_none() {
        return Err(format!(
            "{op}: option 'client_cert' was given without 'client_key' — a client \
             certificate needs its private key"
        ));
    }
    if client_key.is_some() && client_cert.is_none() {
        return Err(format!(
            "{op}: option 'client_key' was given without 'client_cert'"
        ));
    }
    let min_version = match opt_str(opts, "min_version", op)? {
        Some(text) => MinVersion::parse(&text, op)?,
        None => MinVersion::V12,
    };
    // ⚠ **缺省是「校验」**（安全的那一边）。
    let verify = opt_bool(opts, "verify", op)?.unwrap_or(true);
    let server_name = opt_str(opts, "server_name", op)?;
    let alpn = parse_alpn(opts, op)?;
    let connect_timeout_ms = match opt_int(opts, "connect_timeout_ms", op)? {
        Some(ms) if ms < 0 => {
            return Err(format!(
                "{op}: option 'connect_timeout_ms' must not be negative"
            ))
        }
        Some(ms) => ms as u64,
        None => DEFAULT_TIMEOUT_MS,
    };
    Ok(ConnectOpts {
        ca,
        pin,
        client_cert,
        client_key,
        min_version,
        verify,
        server_name,
        alpn,
        connect_timeout_ms,
    })
}

fn required_path(opts: &Map<String, Json>, key: &str, op: &str) -> Result<PathBuf, String> {
    match opt_str(opts, key, op)? {
        Some(text) => Ok(PathBuf::from(text)),
        None => Err(format!("{op}: option '{key}' is required")),
    }
}

fn optional_path(opts: &Map<String, Json>, key: &str, op: &str) -> Result<Option<PathBuf>, String> {
    match opts.get(key) {
        None => Ok(None),
        Some(Json::String(text)) => Ok(Some(PathBuf::from(text))),
        Some(other) => Err(format!(
            "{op}: option '{key}' must be a string path, got {}",
            type_name(other)
        )),
    }
}

/// `alpn` ⇒ 协议名列表。
fn parse_alpn(opts: &Map<String, Json>, op: &str) -> Result<Vec<Vec<u8>>, String> {
    match opts.get("alpn") {
        None => Ok(Vec::new()),
        Some(Json::Array(items)) => {
            let mut out = Vec::with_capacity(items.len());
            for (i, item) in items.iter().enumerate() {
                match item {
                    Json::String(s) => out.push(s.as_bytes().to_vec()),
                    other => {
                        return Err(format!(
                            "{op}: option 'alpn'[{i}] must be a string, got {}",
                            type_name(other)
                        ))
                    }
                }
            }
            Ok(out)
        }
        Some(other) => Err(format!(
            "{op}: option 'alpn' must be an array of strings, got {}",
            type_name(other)
        )),
    }
}

/// `sni` ⇒ `{host: cert}`。
///
/// 值可以是：
/// - 一个**字符串** = 一个 PEM 文件的路径（里面可以有证书链，**也可以**有私钥）；
/// - 一个**字典** = `{"cert": <路径>, "key": <路径>}`。
fn parse_sni(opts: &Map<String, Json>, op: &str) -> Result<BTreeMap<String, SniSource>, String> {
    let mut out = BTreeMap::new();
    let Some(value) = opts.get("sni") else {
        return Ok(out);
    };
    let Json::Object(map) = value else {
        return Err(format!(
            "{op}: option 'sni' must be a dict of {{host: cert}}, got {}",
            type_name(value)
        ));
    };
    for (host, entry) in map {
        let source = match entry {
            Json::String(path) => SniSource::File(PathBuf::from(path)),
            Json::Object(fields) => {
                let cert = match fields.get("cert") {
                    Some(Json::String(p)) => PathBuf::from(p),
                    Some(other) => {
                        return Err(format!(
                            "{op}: option 'sni'['{host}']['cert'] must be a string path, got {}",
                            type_name(other)
                        ))
                    }
                    None => {
                        return Err(format!(
                            "{op}: option 'sni'['{host}'] must have a 'cert' entry"
                        ))
                    }
                };
                let key = match fields.get("key") {
                    Some(Json::String(p)) => PathBuf::from(p),
                    Some(other) => {
                        return Err(format!(
                            "{op}: option 'sni'['{host}']['key'] must be a string path, got {}",
                            type_name(other)
                        ))
                    }
                    None => {
                        return Err(format!(
                            "{op}: option 'sni'['{host}'] must have a 'key' entry \
                             (a per-host certificate needs its own private key)"
                        ))
                    }
                };
                SniSource::Pair { cert, key }
            }
            other => {
                return Err(format!(
                    "{op}: option 'sni'['{host}'] must be a string path or a \
                     {{cert, key}} dict, got {}",
                    type_name(other)
                ))
            }
        };
        out.insert(host.clone(), source);
    }
    Ok(out)
}
