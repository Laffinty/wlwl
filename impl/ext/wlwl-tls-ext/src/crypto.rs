//! `crypto` 命名空间（需求 A-4）。
//!
//! | 成员 | 形状 |
//! |---|---|
//! | `SHA384(bytes)` | `STRING`（小写 hex） |
//! | `SHA512(bytes)` | `STRING`（小写 hex） |
//! | `HMAC_SHA512(key, bytes)` | `STRING`（小写 hex） |
//! | `HKDF(ikm, salt, info, length, algorithm)` | `STRING`（小写 hex） |
//! | `AES_256_GCM(key, nonce, aad, plaintext)` | `{"ciphertext": …, "tag": …}`（hex） |
//! | `ED25519_VERIFY(pubkey, message, signature)` | `BOOLEAN` |
//! | `ECDSA_P256_VERIFY(pubkey, message, signature)` | `BOOLEAN` |
//! | `RSA_PSS_VERIFY(pubkey, message, signature)` | `BOOLEAN` |
//!
//! # ⚠ **只验签，不签发**
//!
//! 三个 `*_VERIFY` 只回答「这个签名对不对」。**不提供**签名生成 ——
//! 那需要私钥管理，而私钥管理是**调用方**（脚本）的责任，不是扩展的。
//!
//! # ⚠ 失败 = `ERR` **值**，不是诊断
//!
//! `kind` 恒为 `CryptoError`（**不新增语言诊断码**）。
//!
//! # 字节形态
//!
//! 入参：`ARRAY[INTEGER]` 或 `STRING`（取 UTF-8 字节）⇒ 见 [`crate::bytes`]。
//! 出参：**一律小写 hex 字符串**。
//!
//! # 公钥形态
//!
//! 三个 `*_VERIFY` 的 `pubkey` 都接受两种形态（自动判别）：
//!
//! 1. **裸公钥材料** —— Ed25519 的 32 字节；ECDSA P-256 的未压缩点
//!    `0x04 || X || Y`（65 字节）；RSA 的 **PKCS#1 `RSAPublicKey`** DER。
//! 2. **X.509 `subjectPublicKeyInfo`（SPKI）DER** —— 从里面把公钥材料抠出来。
//!
//! ⇒ 用户手里通常只有一张证书 ⇒ 直接喂 SPKI 就行。

use serde_json::Value as Json;

use crate::args::{bytes_arg, int_arg, str_arg};
use crate::bytes::hex;
use crate::envelope::{Ctx, MemberResult, KIND_CRYPTO};

fn err(op: &'static str, reason: impl Into<String>) -> crate::envelope::ExtError {
    crate::envelope::ExtError::new(KIND_CRYPTO, op, reason)
}

// ── 摘要 / MAC / KDF ──────────────────────────────────────────────────────

/// `SHA384(bytes) -> STRING`
pub fn sha384(_ctx: &Ctx, args: &[Json]) -> MemberResult {
    const OP: &str = "SHA384";
    let bytes = bytes_arg(args, 0, OP, 1).map_err(|e| err(OP, e))?;
    let digest = ring::digest::digest(&ring::digest::SHA384, &bytes);
    Ok(Json::String(hex(digest.as_ref())))
}

/// `SHA512(bytes) -> STRING`
pub fn sha512(_ctx: &Ctx, args: &[Json]) -> MemberResult {
    const OP: &str = "SHA512";
    let bytes = bytes_arg(args, 0, OP, 1).map_err(|e| err(OP, e))?;
    let digest = ring::digest::digest(&ring::digest::SHA512, &bytes);
    Ok(Json::String(hex(digest.as_ref())))
}

/// `HMAC_SHA512(key, bytes) -> STRING`
pub fn hmac_sha512(_ctx: &Ctx, args: &[Json]) -> MemberResult {
    const OP: &str = "HMAC_SHA512";
    let key = bytes_arg(args, 0, OP, 2).map_err(|e| err(OP, e))?;
    let bytes = bytes_arg(args, 1, OP, 2).map_err(|e| err(OP, e))?;
    let hmac_key = ring::hmac::Key::new(ring::hmac::HMAC_SHA512, &key);
    let tag = ring::hmac::sign(&hmac_key, &bytes);
    Ok(Json::String(hex(tag.as_ref())))
}

/// HKDF 的输出长度（`ring` 的 `KeyType` 只给「摘要长度」，我们要的是**任意**长度）。
struct OkmLength(usize);

impl ring::hkdf::KeyType for OkmLength {
    fn len(&self) -> usize {
        self.0
    }
}

/// `HKDF(ikm, salt, info, length, algorithm) -> STRING`
///
/// `algorithm` ∈ `"sha256"` / `"sha384"` / `"sha512"`（大小写不敏感）。
pub fn hkdf(_ctx: &Ctx, args: &[Json]) -> MemberResult {
    const OP: &str = "HKDF";
    let ikm = bytes_arg(args, 0, OP, 5).map_err(|e| err(OP, e))?;
    let salt = bytes_arg(args, 1, OP, 5).map_err(|e| err(OP, e))?;
    let info = bytes_arg(args, 2, OP, 5).map_err(|e| err(OP, e))?;
    let length = int_arg(args, 3, OP, 5).map_err(|e| err(OP, e))?;
    let algorithm = str_arg(args, 4, OP, 5).map_err(|e| err(OP, e))?;
    if length < 0 {
        return Err(err(OP, "the output length must not be negative"));
    }
    let length = length as usize;
    let alg = match algorithm.to_ascii_lowercase().as_str() {
        "sha256" => &ring::hkdf::HKDF_SHA256,
        "sha384" => &ring::hkdf::HKDF_SHA384,
        "sha512" => &ring::hkdf::HKDF_SHA512,
        other => {
            return Err(err(
                OP,
                format!(
                    "unknown algorithm '{other}' (expected 'sha256', 'sha384' or 'sha512')"
                ),
            ))
        }
    };
    // HKDF = Extract(salt, ikm) ⇒ PRK，再 Expand(info, len) ⇒ OKM。
    let salt = ring::hkdf::Salt::new(*alg, &salt);
    let prk = salt.extract(&ikm);
    let info_parts: [&[u8]; 1] = [&info];
    let okm = prk
        .expand(&info_parts, OkmLength(length))
        .map_err(|_| err(OP, "the requested output length is too large"))?;
    let mut out = vec![0u8; length];
    okm.fill(&mut out)
        .map_err(|_| err(OP, "the output could not be derived"))?;
    Ok(Json::String(hex(&out)))
}

// ── AEAD ──────────────────────────────────────────────────────────────────

/// `AES_256_GCM(key, nonce, aad, plaintext) -> {"ciphertext": …, "tag": …}`
///
/// ⚠ **GCM 的 nonce 绝不能重复** —— 用同一个 key + 同一个 nonce 加密两条
/// 不同消息 ⇒ 认证密钥可被恢复。扩展**不**替你管理 nonce（那要状态）。
pub fn aes_256_gcm(_ctx: &Ctx, args: &[Json]) -> MemberResult {
    const OP: &str = "AES_256_GCM";
    let key = bytes_arg(args, 0, OP, 4).map_err(|e| err(OP, e))?;
    let nonce = bytes_arg(args, 1, OP, 4).map_err(|e| err(OP, e))?;
    let aad = bytes_arg(args, 2, OP, 4).map_err(|e| err(OP, e))?;
    let plaintext = bytes_arg(args, 3, OP, 4).map_err(|e| err(OP, e))?;

    if key.len() != 32 {
        return Err(err(
            OP,
            format!("the key must be 32 bytes for AES-256-GCM, got {}", key.len()),
        ));
    }
    if nonce.len() != 12 {
        return Err(err(
            OP,
            format!(
                "the nonce must be 12 bytes for AES-256-GCM, got {}",
                nonce.len()
            ),
        ));
    }

    let unbound = ring::aead::UnboundKey::new(&ring::aead::AES_256_GCM, &key)
        .map_err(|_| err(OP, "the key was rejected"))?;
    let sealing = ring::aead::LessSafeKey::new(unbound);
    let nonce_bytes: [u8; 12] = nonce
        .try_into()
        .map_err(|_| err(OP, "the nonce must be 12 bytes"))?;
    let nonce = ring::aead::Nonce::assume_unique_for_key(nonce_bytes);

    let mut buffer = plaintext;
    sealing
        .seal_in_place_append_tag(nonce, ring::aead::Aad::from(aad.as_slice()), &mut buffer)
        .map_err(|_| err(OP, "the sealing failed"))?;
    // `seal_in_place_append_tag` 把 tag **接在**密文后面 ⇒ 拆开。
    let tag_len = 16;
    let split = buffer.len().saturating_sub(tag_len);
    let ciphertext = buffer[..split].to_vec();
    let tag = buffer[split..].to_vec();

    Ok(crate::envelope::dict(vec![
        ("ciphertext", Json::String(hex(&ciphertext))),
        ("tag", Json::String(hex(&tag))),
    ]))
}

// ── 验签 ──────────────────────────────────────────────────────────────────

/// `ED25519_VERIFY(pubkey, message, signature) -> BOOLEAN`
pub fn ed25519_verify(_ctx: &Ctx, args: &[Json]) -> MemberResult {
    const OP: &str = "ED25519_VERIFY";
    let pubkey = bytes_arg(args, 0, OP, 3).map_err(|e| err(OP, e))?;
    let message = bytes_arg(args, 1, OP, 3).map_err(|e| err(OP, e))?;
    let signature = bytes_arg(args, 2, OP, 3).map_err(|e| err(OP, e))?;

    let key = normalize_public_key(&pubkey, |k| k.len() == 32).map_err(|e| err(OP, e))?;
    if key.len() != 32 {
        return Err(err(
            OP,
            format!(
                "the Ed25519 public key must be 32 bytes, got {}",
                key.len()
            ),
        ));
    }
    if signature.len() != 64 {
        return Err(err(
            OP,
            format!(
                "the Ed25519 signature must be 64 bytes, got {}",
                signature.len()
            ),
        ));
    }
    let verifier = ring::signature::UnparsedPublicKey::new(&ring::signature::ED25519, &key);
    Ok(Json::Bool(verifier.verify(&message, &signature).is_ok()))
}

/// `ECDSA_P256_VERIFY(pubkey, message, signature) -> BOOLEAN`
///
/// ⚠ 用 **SHA-256** 摘要（`ECDSA_P256_SHA256_FIXED`），签名是 **64 字节定长**
/// （`r || s`），不是 ASN.1 DER 形态。
pub fn ecdsa_p256_verify(_ctx: &Ctx, args: &[Json]) -> MemberResult {
    const OP: &str = "ECDSA_P256_VERIFY";
    let pubkey = bytes_arg(args, 0, OP, 3).map_err(|e| err(OP, e))?;
    let message = bytes_arg(args, 1, OP, 3).map_err(|e| err(OP, e))?;
    let signature = bytes_arg(args, 2, OP, 3).map_err(|e| err(OP, e))?;

    let key = normalize_public_key(&pubkey, |k| k.len() == 65 && k[0] == 0x04).map_err(|e| err(OP, e))?;
    if key.len() != 65 || key[0] != 0x04 {
        return Err(err(
            OP,
            "the ECDSA P-256 public key must be the 65-byte uncompressed point \
             (0x04 || X || Y)",
        ));
    }
    if signature.len() != 64 {
        return Err(err(
            OP,
            format!(
                "the ECDSA P-256 signature must be 64 bytes (r || s), got {}",
                signature.len()
            ),
        ));
    }
    let verifier =
        ring::signature::UnparsedPublicKey::new(&ring::signature::ECDSA_P256_SHA256_FIXED, &key);
    Ok(Json::Bool(verifier.verify(&message, &signature).is_ok()))
}

/// `RSA_PSS_VERIFY(pubkey, message, signature) -> BOOLEAN`
///
/// ⚠ 用 **RSA-PSS** 填充 + **SHA-256** 摘要（`RSA_PSS_2048_8192_SHA256`）。
/// ⚠ 签名长度必须等于**模长**（2048 位 ⇒ 256 字节）。
pub fn rsa_pss_verify(_ctx: &Ctx, args: &[Json]) -> MemberResult {
    const OP: &str = "RSA_PSS_VERIFY";
    let pubkey = bytes_arg(args, 0, OP, 3).map_err(|e| err(OP, e))?;
    let message = bytes_arg(args, 1, OP, 3).map_err(|e| err(OP, e))?;
    let signature = bytes_arg(args, 2, OP, 3).map_err(|e| err(OP, e))?;

    // 公钥：裸 PKCS#1 `RSAPublicKey` DER 或 SPKI DER。
    let key = match pkcs1_public_key(&pubkey) {
        Some(k) => k,
        None => {
            return Err(err(
                OP,
                "the RSA public key must be a PKCS#1 RSAPublicKey DER or an X.509 \
                 subjectPublicKeyInfo DER",
            ))
        }
    };
    let modulus_len = pkcs1_modulus_len(&key).ok_or_else(|| {
        err(
            OP,
            "the RSA public key does not look like a PKCS#1 RSAPublicKey \
             (SEQUENCE { INTEGER, INTEGER })",
        )
    })?;
    if !(256..=1024).contains(&modulus_len) {
        return Err(err(
            OP,
            format!(
                "the RSA modulus must be 2048..=8192 bits, got {} bits",
                modulus_len * 8
            ),
        ));
    }
    if signature.len() != modulus_len {
        return Err(err(
            OP,
            format!(
                "the RSA-PSS signature must be {modulus_len} bytes (the modulus \
                 length), got {}",
                signature.len()
            ),
        ));
    }
    let verifier = ring::signature::UnparsedPublicKey::new(
        &ring::signature::RSA_PSS_2048_8192_SHA256,
        &key,
    );
    Ok(Json::Bool(verifier.verify(&message, &signature).is_ok()))
}

/// 公钥归一化：能当 **SPKI** 解析就抠出公钥材料，否则按**裸材料**用。
///
/// `plausible` 用来避免把一段随机字节**误判**成 DER。
fn normalize_public_key(
    raw: &[u8],
    plausible: impl Fn(&[u8]) -> bool,
) -> Result<Vec<u8>, String> {
    if let Some(bits) = spki_public_key_bits(raw) {
        if plausible(&bits) {
            return Ok(bits);
        }
    }
    Ok(raw.to_vec())
}

/// 从一段 DER 里抠出 SPKI 的公钥材料（不是 SPKI ⇒ `None`）。
fn spki_public_key_bits(der: &[u8]) -> Option<Vec<u8>> {
    // SPKI ::= SEQUENCE { algorithm AlgorithmIdentifier, subjectPublicKey BIT STRING }
    // AlgorithmIdentifier ::= SEQUENCE { ... }
    let (seq_tag, seq_content, rest) = read_tlv(der).ok()?;
    if !rest.is_empty() || seq_tag != 0x30 {
        return None;
    }
    let (alg_tag, _alg_content, after_alg) = read_tlv(seq_content).ok()?;
    if alg_tag != 0x30 {
        return None;
    }
    let (bits_tag, bits_content, after_bits) = read_tlv(after_alg).ok()?;
    if !after_bits.is_empty() || bits_tag != 0x03 || bits_content.is_empty() {
        return None;
    }
    Some(bits_content[1..].to_vec())
}

/// 从一段 DER 里抠出 **PKCS#1 `RSAPublicKey`**（不是 ⇒ `None`）。
fn pkcs1_public_key(der: &[u8]) -> Option<Vec<u8>> {
    // 先试「已经是 PKCS#1」：SEQUENCE { INTEGER, INTEGER }
    if let Some(len) = pkcs1_modulus_len(der) {
        if (256..=1024).contains(&len) {
            return Some(der.to_vec());
        }
    }
    // 再试「是 SPKI」⇒ 从 BIT STRING 里抠出 PKCS#1。
    let bits = spki_public_key_bits(der)?;
    pkcs1_modulus_len(&bits).map(|_| bits)
}

/// PKCS#1 `RSAPublicKey` ⇒ 模长（字节）。
fn pkcs1_modulus_len(der: &[u8]) -> Option<usize> {
    let (seq_tag, seq_content, rest) = read_tlv(der).ok()?;
    if !rest.is_empty() || seq_tag != 0x30 {
        return None;
    }
    let (modulus_tag, modulus_content, after_modulus) = read_tlv(seq_content).ok()?;
    if modulus_tag != 0x02 {
        return None;
    }
    let (_exponent_tag, _exponent_content, after_exponent) = read_tlv(after_modulus).ok()?;
    if !after_exponent.is_empty() {
        return None;
    }
    let mut bytes = modulus_content;
    // DER INTEGER 的前导 `00` 是符号位 ⇒ 不算模长。
    if bytes.first() == Some(&0) {
        bytes = &bytes[1..];
    }
    Some(bytes.len())
}

/// 读一个 DER TLV ⇒ `(tag, 内容, 剩余)`。
fn read_tlv(buf: &[u8]) -> Result<(u8, &[u8], &[u8]), ()> {
    if buf.len() < 2 {
        return Err(());
    }
    let tag = buf[0];
    let first = buf[1];
    let (len, header) = if first & 0x80 == 0 {
        (first as usize, 2usize)
    } else {
        let n = (first & 0x7f) as usize;
        if n == 0 || n > 4 || buf.len() < 2 + n {
            return Err(());
        }
        let mut len = 0usize;
        for byte in &buf[2..2 + n] {
            len = (len << 8) | *byte as usize;
        }
        (len, 2 + n)
    };
    if buf.len() < header + len {
        return Err(());
    }
    Ok((tag, &buf[header..header + len], &buf[header + len..]))
}
