//! 一个**够用就停**的 DER / X.509 解析器。
//!
//! # 为什么自己写
//!
//! 本 crate 的依赖面被钉死在 `rustls` / `ring` / `webpki-roots` / `serde_json`
//! 四个 —— `x509-parser` 不在其中。而 `CERT_INFO` / `CERT_FINGERPRINT`
//! （需求 A-3）需要证书的 **subject / issuer / SAN / 有效期 / SPKI / 序列号**
//! —— `rustls` 与 `ring` **都不提供**这些（`rustls` 只校验，不解析）。
//!
//! ⇒ 这里手写一个**只读、只取需要的字段**的解析器。它**不是**一个通用
//! X.509 库：不查 CRL、不查 OCSP、不验签、不处理证书路径。
//!
//! # 解析范围
//!
//! ```text
//! Certificate ::= SEQUENCE {
//!     tbsCertificate       TBSCertificate,
//!     signatureAlgorithm   AlgorithmIdentifier,
//!     signatureValue       BIT STRING }
//!
//! TBSCertificate ::= SEQUENCE {
//!     version         [0] EXPLICIT Version DEFAULT v1,
//!     serialNumber       CertificateSerialNumber,
//!     signature          AlgorithmIdentifier,
//!     issuer             Name,
//!     validity           Validity,
//!     subject            Name,
//!     subjectPublicKeyInfo SubjectPublicKeyInfo,
//!     issuerUniqueID  [1] IMPLICIT UniqueIdentifier OPTIONAL,
//!     subjectUniqueID [2] IMPLICIT UniqueIdentifier OPTIONAL,
//!     extensions      [3] EXPLICIT Extensions OPTIONAL }
//! ```
//!
//! # 名（Name）的写法
//!
//! 按 **RFC 4514** 写成 `CN=localhost,O=Example`（RDN 顺序**反过来**，
//! 即 DER 里最一般的在前、RFC 4514 里最具体的在前）。

use crate::bytes::hex;

/// 一张证书里我们要的字段。
#[derive(Debug, Clone, PartialEq)]
pub struct CertInfo {
    /// subject（RFC 4514 写法）。
    pub subject: String,
    /// issuer（RFC 4514 写法）。
    pub issuer: String,
    /// subjectAltName 里的名字（DNS 名 / IP / 邮箱 / URI）。
    pub san: Vec<String>,
    /// 生效时间（RFC 3339，`Z` 结尾）。
    pub not_before: String,
    /// 失效时间（RFC 3339，`Z` 结尾）。
    pub not_after: String,
    /// **SPKI**（`subjectPublicKeyInfo` 整个 DER）的 SHA-256，小写 hex。
    pub spki_sha256: String,
    /// 序列号（小写 hex，**去掉** DER INTEGER 的前导 `00`）。
    pub serial: String,
}

// ── DER 读原语 ────────────────────────────────────────────────────────────

/// 一个 DER TLV（tag / 内容 / 完整字节）。
#[derive(Debug, Clone, Copy)]
struct Tlv<'a> {
    tag: u8,
    content: &'a [u8],
    full: &'a [u8],
}

/// 读一个 TLV ⇒ `(tlv, 剩下的字节)`。
fn read_tlv(buf: &[u8]) -> Result<(Tlv<'_>, &[u8]), String> {
    if buf.is_empty() {
        return Err("unexpected end of the DER data".into());
    }
    let tag = buf[0];
    if buf.len() < 2 {
        return Err("the DER length is truncated".into());
    }
    let first = buf[1];
    let (len, header) = if first & 0x80 == 0 {
        (first as usize, 2usize)
    } else {
        let n = (first & 0x7f) as usize;
        if n == 0 {
            return Err("the indefinite length form is not valid DER".into());
        }
        if n > 4 {
            return Err("the DER length is unreasonably large".into());
        }
        if buf.len() < 2 + n {
            return Err("the DER length is truncated".into());
        }
        let mut len = 0usize;
        for byte in &buf[2..2 + n] {
            len = (len << 8) | *byte as usize;
        }
        (len, 2 + n)
    };
    if buf.len() < header + len {
        return Err("the DER content is truncated".into());
    }
    Ok((
        Tlv {
            tag,
            content: &buf[header..header + len],
            full: &buf[..header + len],
        },
        &buf[header + len..],
    ))
}

/// 期望一个 SEQUENCE（tag `0x30`）。
fn expect_seq<'a>(tlv: &Tlv<'a>, what: &str) -> Result<&'a [u8], String> {
    if tlv.tag != 0x30 {
        return Err(format!("{what} is not a DER SEQUENCE"));
    }
    Ok(tlv.content)
}

// ── OID ───────────────────────────────────────────────────────────────────

/// OID 的内容字节 ⇒ 点分十进制。
fn oid_to_string(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return String::new();
    }
    let first = bytes[0];
    let (a, b) = if first < 40 {
        (0u32, first as u32)
    } else if first < 80 {
        (1u32, (first - 40) as u32)
    } else {
        (2u32, (first - 80) as u32)
    };
    let mut out = format!("{a}.{b}");
    let mut acc: u64 = 0;
    for byte in &bytes[1..] {
        acc = (acc << 7) | u64::from(byte & 0x7f);
        if byte & 0x80 == 0 {
            out.push('.');
            out.push_str(&acc.to_string());
            acc = 0;
        }
    }
    out
}

/// 常见的属性短名（RFC 4514 用的那套）。
fn oid_short_name(oid: &str) -> Option<&'static str> {
    Some(match oid {
        "2.5.4.3" => "CN",
        "2.5.4.6" => "C",
        "2.5.4.7" => "L",
        "2.5.4.8" => "ST",
        "2.5.4.10" => "O",
        "2.5.4.11" => "OU",
        "2.5.4.5" => "serialNumber",
        "2.5.4.4" => "SN",
        "2.5.4.42" => "GN",
        "0.9.2342.19200300.100.1.25" => "DC",
        "0.9.2342.19200300.100.1.1" => "UID",
        "1.2.840.113549.1.9.1" => "emailAddress",
        _ => return None,
    })
}

// ── 字符串 / 时间 ─────────────────────────────────────────────────────────

/// 目录名里的属性值 ⇒ 文本。
fn string_value(tlv: &Tlv<'_>) -> String {
    match tlv.tag {
        // UTF8String
        0x0c => String::from_utf8_lossy(tlv.content).into_owned(),
        // PrintableString / IA5String / VisibleString / GeneralString / T61String
        0x13 | 0x16 | 0x1a | 0x1b | 0x14 => String::from_utf8_lossy(tlv.content).into_owned(),
        // BMPString = UTF-16BE
        0x1e => {
            let units: Vec<u16> = tlv
                .content
                .chunks_exact(2)
                .map(|c| u16::from_be_bytes([c[0], c[1]]))
                .collect();
            String::from_utf16_lossy(&units)
        }
        // UniversalString = UTF-32BE
        0x1c => {
            let units: Vec<u32> = tlv
                .content
                .chunks_exact(4)
                .map(|c| u32::from_be_bytes([c[0], c[1], c[2], c[3]]))
                .collect();
            units.into_iter().map(char::from_u32).flatten().collect()
        }
        // OctetString：按字节看（多半是 emailAddress 之类）
        0x04 => String::from_utf8_lossy(tlv.content).into_owned(),
        _ => String::from_utf8_lossy(tlv.content).into_owned(),
    }
}

/// 时间（UTCTime / GeneralizedTime）⇒ RFC 3339。
fn time_to_string(tlv: &Tlv<'_>) -> String {
    let text = String::from_utf8_lossy(tlv.content).into_owned();
    let digits = |s: &str| -> i64 { s.parse().unwrap_or(0) };
    match tlv.tag {
        // UTCTime: YYMMDDHHMMSSZ
        0x17 if text.len() >= 13 => {
            let yy = digits(&text[0..2]);
            let year = if yy >= 50 { 1900 + yy } else { 2000 + yy };
            format!(
                "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
                year,
                digits(&text[2..4]),
                digits(&text[4..6]),
                digits(&text[6..8]),
                digits(&text[8..10]),
                digits(&text[10..12])
            )
        }
        // GeneralizedTime: YYYYMMDDHHMMSSZ
        0x18 if text.len() >= 15 => format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
            digits(&text[0..4]),
            digits(&text[4..6]),
            digits(&text[6..8]),
            digits(&text[8..10]),
            digits(&text[10..12]),
            digits(&text[12..14])
        ),
        _ => text,
    }
}

// ── Name ──────────────────────────────────────────────────────────────────

/// `Name`（= RDNSequence）⇒ RFC 4514 字符串。
fn name_to_string(content: &[u8]) -> String {
    let mut rdns: Vec<String> = Vec::new();
    let mut rest = content;
    while !rest.is_empty() {
        let Ok((set, after)) = read_tlv(rest) else {
            break;
        };
        rest = after;
        let mut attrs: Vec<String> = Vec::new();
        let mut inner = set.content;
        while !inner.is_empty() {
            let Ok((attr, after)) = read_tlv(inner) else {
                break;
            };
            inner = after;
            if let Some(text) = attribute_to_string(&attr) {
                attrs.push(text);
            }
        }
        if !attrs.is_empty() {
            rdns.push(attrs.join("+"));
        }
    }
    // RFC 4514：RDN 顺序反过来写。
    rdns.reverse();
    rdns.join(", ")
}

/// `AttributeTypeAndValue` ⇒ `CN=localhost`。
fn attribute_to_string(attr: &Tlv<'_>) -> Option<String> {
    let (oid_tlv, rest) = read_tlv(attr.content).ok()?;
    let oid = oid_to_string(oid_tlv.content);
    let (value_tlv, _) = read_tlv(rest).ok()?;
    let name = oid_short_name(&oid).unwrap_or(oid.as_str());
    Some(format!("{}={}", name, string_value(&value_tlv)))
}

// ── SAN ───────────────────────────────────────────────────────────────────

/// `GeneralNames` ⇒ 名字列表。
fn general_names(content: &[u8]) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = content;
    while !rest.is_empty() {
        let Ok((gn, after)) = read_tlv(rest) else {
            break;
        };
        rest = after;
        match gn.tag {
            // [2] dNSName
            0x82 => out.push(String::from_utf8_lossy(gn.content).into_owned()),
            // [1] rfc822Name
            0x81 => out.push(format!(
                "mailto:{}",
                String::from_utf8_lossy(gn.content)
            )),
            // [6] uniformResourceIdentifier
            0x86 => out.push(String::from_utf8_lossy(gn.content).into_owned()),
            // [7] iPAddress
            0x87 => out.push(format_ip(gn.content)),
            _ => {}
        }
    }
    out
}

/// iPAddress 字节 ⇒ 文本。
fn format_ip(bytes: &[u8]) -> String {
    match bytes.len() {
        4 => format!("{}.{}.{}.{}", bytes[0], bytes[1], bytes[2], bytes[3]),
        16 => {
            let groups: Vec<String> = bytes
                .chunks(2)
                .map(|c| format!("{:x}", u16::from_be_bytes([c[0], c[1]])))
                .collect();
            groups.join(":")
        }
        _ => hex(bytes),
    }
}

/// `Extensions` ⇒ SAN 名字列表（只找 `2.5.29.17`）。
fn san_from_extensions(content: &[u8]) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = content;
    while !rest.is_empty() {
        let Ok((ext, after)) = read_tlv(rest) else {
            break;
        };
        rest = after;
        // Extension ::= SEQUENCE { extnID OID, critical BOOLEAN DEFAULT FALSE,
        //                           extnValue OCTET STRING }
        let Ok((oid_tlv, after_oid)) = read_tlv(ext.content) else {
            continue;
        };
        let oid = oid_to_string(oid_tlv.content);
        let Ok((next, after_next)) = read_tlv(after_oid) else {
            continue;
        };
        // `critical` 是 BOOLEAN（tag 0x01）⇒ 跳过它。
        let value_tlv = if next.tag == 0x01 {
            match read_tlv(after_next) {
                Ok((t, _)) => t,
                Err(_) => continue,
            }
        } else {
            next
        };
        if oid == "2.5.29.17" && value_tlv.tag == 0x04 {
            // extnValue 里装的是 `GeneralNames` 的 DER。
            if let Ok((names, _)) = read_tlv(value_tlv.content) {
                out.extend(general_names(names.content));
            }
        }
    }
    out
}

// ── 顶层 ──────────────────────────────────────────────────────────────────

/// 解析一张 DER 证书。
pub fn parse_certificate(der: &[u8]) -> Result<CertInfo, String> {
    let (cert, rest) = read_tlv(der)?;
    expect_seq(&cert, "the certificate")?;
    if !rest.is_empty() {
        return Err("there are trailing bytes after the certificate".into());
    }
    let mut inner = cert.content;
    let (tbs, after_tbs) = read_tlv(inner)?;
    inner = after_tbs;
    let (_sig_alg, after_alg) = read_tlv(inner)?;
    inner = after_alg;
    let (_sig_value, after_value) = read_tlv(inner)?;
    if !after_value.is_empty() {
        return Err("there are trailing bytes inside the certificate".into());
    }
    parse_tbs(expect_seq(&tbs, "tbsCertificate")?)
}

/// 解析 `TBSCertificate` 的内容。
fn parse_tbs(content: &[u8]) -> Result<CertInfo, String> {
    let mut rest = content;

    // version [0] EXPLICIT（可省）
    let (first, after_first) = read_tlv(rest)?;
    if first.tag == 0xa0 {
        rest = after_first;
    } else {
        rest = content;
    }

    // serialNumber
    let (serial, after) = read_tlv(rest)?;
    if serial.tag != 0x02 {
        return Err("the serial number is not a DER INTEGER".into());
    }
    let serial_hex = hex(strip_leading_zeros(serial.content));
    rest = after;

    // signature（AlgorithmIdentifier）—— 不关心，跳过。
    let (_sig, after) = read_tlv(rest)?;
    rest = after;

    // issuer
    let (issuer, after) = read_tlv(rest)?;
    let issuer_text = name_to_string(expect_seq(&issuer, "the issuer")?);
    rest = after;

    // validity
    let (validity, after) = read_tlv(rest)?;
    let validity_content = expect_seq(&validity, "the validity")?;
    let (not_before_tlv, after_nb) = read_tlv(validity_content)?;
    let (not_after_tlv, _) = read_tlv(after_nb)?;
    let not_before = time_to_string(&not_before_tlv);
    let not_after = time_to_string(&not_after_tlv);
    rest = after;

    // subject
    let (subject, after) = read_tlv(rest)?;
    let subject_text = name_to_string(expect_seq(&subject, "the subject")?);
    rest = after;

    // subjectPublicKeyInfo（**完整字节**就是 SPKI —— 指纹算在它身上）
    let (spki, after) = read_tlv(rest)?;
    rest = after;

    // 可选的 [1] / [2] / [3]
    let mut extensions: Option<&[u8]> = None;
    while !rest.is_empty() {
        let Ok((t, after)) = read_tlv(rest) else {
            break;
        };
        rest = after;
        if t.tag == 0xa3 {
            // [3] EXPLICIT Extensions ::= SEQUENCE OF Extension
            if let Ok((seq, _)) = read_tlv(t.content) {
                extensions = Some(seq.content);
            }
        }
    }

    let san = extensions.map(san_from_extensions).unwrap_or_default();

    Ok(CertInfo {
        subject: subject_text,
        issuer: issuer_text,
        san,
        not_before,
        not_after,
        spki_sha256: spki_sha256_hex(spki.full),
        serial: serial_hex,
    })
}

/// 去掉 DER INTEGER 的前导 `00`（那是给符号位用的，不是序列号的一部分）。
fn strip_leading_zeros(bytes: &[u8]) -> &[u8] {
    let mut i = 0;
    while i + 1 < bytes.len() && bytes[i] == 0 {
        i += 1;
    }
    &bytes[i..]
}

/// **SPKI**（`subjectPublicKeyInfo` 整个 DER）的 SHA-256，小写 hex。
pub fn spki_sha256_hex(spki_der: &[u8]) -> String {
    let digest = ring::digest::digest(&ring::digest::SHA256, spki_der);
    hex(digest.as_ref())
}

/// 从一张 DER 证书里取出 **SPKI** 的完整 DER 字节。
///
/// （`ECDSA_P256_VERIFY` / `RSA_PSS_VERIFY` 要用它 —— 见 `crypto` 模块。）
pub fn spki_der(der: &[u8]) -> Result<Vec<u8>, String> {
    let (cert, _) = read_tlv(der)?;
    let mut inner = cert.content;
    let (tbs, after_tbs) = read_tlv(inner)?;
    inner = after_tbs;
    let (_sig_alg, after_alg) = read_tlv(inner)?;
    inner = after_alg;
    let (_sig_value, _) = read_tlv(inner)?;
    let mut rest = expect_seq(&tbs, "tbsCertificate")?;
    let (first, after_first) = read_tlv(rest)?;
    if first.tag == 0xa0 {
        rest = after_first;
    } else {
        rest = expect_seq(&tbs, "tbsCertificate")?;
    }
    // serialNumber / signature / issuer / validity / subject —— 逐个跳过。
    let mut cursor = rest;
    for _ in 0..5 {
        let (_t, after) = read_tlv(cursor)?;
        cursor = after;
    }
    let (spki, _) = read_tlv(cursor)?;
    Ok(spki.full.to_vec())
}

/// 从一张 DER 证书里取出 **SPKI 里的公钥比特**（`BIT STRING` 的内容）。
///
/// * ECDSA P-256 ⇒ **未压缩点** `0x04 || X || Y`（65 字节）；
/// * RSA ⇒ **PKCS#1 `RSAPublicKey`** DER（`SEQUENCE { n, e }`）。
///
/// （`ring` 的验签接口要的就是这两种形状。）
pub fn spki_public_key_bits(der: &[u8]) -> Result<Vec<u8>, String> {
    let spki = spki_der(der)?;
    let (seq, _) = read_tlv(&spki)?;
    let content = expect_seq(&seq, "the subjectPublicKeyInfo")?;
    let (_alg_id, after_alg) = read_tlv(content)?;
    let (bits, _) = read_tlv(after_alg)?;
    if bits.tag != 0x03 {
        return Err("the subjectPublicKeyInfo does not contain a BIT STRING".into());
    }
    // BIT STRING 的第一个字节是「未用位数」⇒ 跳过它。
    if bits.content.is_empty() {
        return Err("the public key BIT STRING is empty".into());
    }
    Ok(bits.content[1..].to_vec())
}
