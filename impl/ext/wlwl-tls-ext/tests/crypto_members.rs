//! `crypto` 命名空间的成员测试（需求 A-4）。
//!
//! ⚠ 期望值全部来自 **Python 独立实现**算出来的向量（`tests/fixtures/vectors.json`）
//! ⇒ 与 Rust 那边的 `ring` 是**两套实现** ⇒ 能抓出「两边算法口径不同」。
//!
//! ⚠ 失败一律是 `ERR` **值**（`kind: "CryptoError"`）—— **不新增诊断码**。

mod common;

use common::{fixture, hex_to_json_array, json_array_to_hex, vectors, TestHost};
use serde_json::json;

/// 调一个 `crypto` 成员。
fn call(host: &TestHost, member: &str, args: &[serde_json::Value]) -> serde_json::Value {
    common::call(host, "crypto", member, args).unwrap_or_else(|e| panic!("{member} failed: {e}"))
}

/// 调一个 `crypto` 成员（要失败）。
fn call_err(host: &TestHost, member: &str, args: &[serde_json::Value]) -> serde_json::Value {
    common::call(host, "crypto", member, args)
        .expect_err(&format!("{member} was supposed to fail"))
}

#[test]
fn sha384_and_sha512_match_the_reference_values() {
    let host = TestHost::new();
    let v = vectors();
    let input = hex_to_json_array(v["digests"]["input"].as_str().unwrap());

    let got = call(&host, "SHA384", &[input.clone()]);
    assert_eq!(got.as_str().unwrap(), v["digests"]["sha384"].as_str().unwrap());

    let got = call(&host, "SHA512", &[input]);
    assert_eq!(got.as_str().unwrap(), v["digests"]["sha512"].as_str().unwrap());
}

#[test]
fn hmac_sha512_matches_the_reference_value() {
    let host = TestHost::new();
    let v = vectors();
    let key = hex_to_json_array(v["hmac_sha512"]["key"].as_str().unwrap());
    let data = hex_to_json_array(v["hmac_sha512"]["data"].as_str().unwrap());

    let got = call(&host, "HMAC_SHA512", &[key, data]);
    assert_eq!(got.as_str().unwrap(), v["hmac_sha512"]["tag"].as_str().unwrap());
}

#[test]
fn hkdf_matches_the_reference_value() {
    let host = TestHost::new();
    let v = vectors();
    let h = &v["hkdf_sha256"];
    let ikm = hex_to_json_array(h["ikm"].as_str().unwrap());
    let salt = hex_to_json_array(h["salt"].as_str().unwrap());
    let info = hex_to_json_array(h["info"].as_str().unwrap());
    let length = h["length"].as_i64().unwrap();

    let got = call(&host, "HKDF", &[ikm, salt, info, json!(length), json!("sha256")]);
    assert_eq!(got.as_str().unwrap(), h["okm"].as_str().unwrap());
}

#[test]
fn aes_256_gcm_matches_the_reference_ciphertext_and_tag() {
    let host = TestHost::new();
    let v = vectors();
    let a = &v["aes_256_gcm"];
    let key = hex_to_json_array(a["key"].as_str().unwrap());
    let nonce = hex_to_json_array(a["nonce"].as_str().unwrap());
    let aad = hex_to_json_array(a["aad"].as_str().unwrap());
    let plaintext = hex_to_json_array(a["plaintext"].as_str().unwrap());

    let got = call(&host, "AES_256_GCM", &[key, nonce, aad, plaintext]);
    assert_eq!(got["ciphertext"].as_str().unwrap(), a["ciphertext"].as_str().unwrap());
    assert_eq!(got["tag"].as_str().unwrap(), a["tag"].as_str().unwrap());
}

#[test]
fn ed25519_verification_accepts_a_good_signature_and_rejects_a_bad_one() {
    let host = TestHost::new();
    let v = vectors();
    let e = &v["ed25519"];
    let message = hex_to_json_array(v["message"].as_str().unwrap());
    let pubkey = hex_to_json_array(e["pubkey"].as_str().unwrap());
    let signature = hex_to_json_array(e["signature"].as_str().unwrap());
    let bad = hex_to_json_array(e["bad_signature"].as_str().unwrap());

    let got = call(&host, "ED25519_VERIFY", &[pubkey.clone(), message.clone(), signature]);
    assert_eq!(got, json!(true), "a good signature must verify");

    let got = call(&host, "ED25519_VERIFY", &[pubkey, message, bad]);
    assert_eq!(got, json!(false), "a tampered signature must not verify");
}

#[test]
fn ecdsa_p256_verification_accepts_a_good_signature_and_rejects_a_bad_one() {
    let host = TestHost::new();
    let v = vectors();
    let e = &v["ecdsa_p256"];
    let message = hex_to_json_array(v["message"].as_str().unwrap());
    let spki = hex_to_json_array(e["spki"].as_str().unwrap());
    let signature = hex_to_json_array(e["signature"].as_str().unwrap());

    // ① SPKI 形态（用户手里通常只有证书）。
    let got = call(&host, "ECDSA_P256_VERIFY", &[spki.clone(), message.clone(), signature.clone()]);
    assert_eq!(got, json!(true), "a good signature must verify");

    // ② 裸公钥形态（未压缩点）。
    let point = {
        // SPKI 的 BIT STRING 里就是未压缩点：这里直接从 SPKI 的字节里取。
        let der = e["spki"].as_str().unwrap();
        let bytes: Vec<u8> = (0..der.len() / 2)
            .map(|i| u8::from_str_radix(&der[i * 2..i * 2 + 2], 16).unwrap())
            .collect();
        // SPKI ::= SEQUENCE { AlgorithmIdentifier, BIT STRING } ⇒ 最后一段是公钥。
        let tail = &bytes[bytes.len() - 65..];
        assert_eq!(tail[0], 0x04, "the P-256 public key must be an uncompressed point");
        serde_json::Value::Array(tail.iter().map(|b| json!(b)).collect())
    };
    let got = call(&host, "ECDSA_P256_VERIFY", &[point, message.clone(), signature.clone()]);
    assert_eq!(got, json!(true), "the raw uncompressed point must verify too");

    // ③ 坏签名 ⇒ FALSE（不是错误）。
    let mut bad = signature.as_array().unwrap().clone();
    bad[0] = json!((bad[0].as_u64().unwrap() ^ 0xff) & 0xff);
    let got = call(&host, "ECDSA_P256_VERIFY", &[spki, message, json!(bad)]);
    assert_eq!(got, json!(false), "a tampered signature must not verify");
}

#[test]
fn rsa_pss_verification_accepts_a_good_signature_and_rejects_a_bad_one() {
    let host = TestHost::new();
    let v = vectors();
    let r = &v["rsa_pss"];
    let message = hex_to_json_array(v["message"].as_str().unwrap());
    let spki = hex_to_json_array(r["spki"].as_str().unwrap());
    let signature = hex_to_json_array(r["signature"].as_str().unwrap());

    let got = call(&host, "RSA_PSS_VERIFY", &[spki.clone(), message.clone(), signature.clone()]);
    assert_eq!(got, json!(true), "a good signature must verify");

    let mut bad = signature.as_array().unwrap().clone();
    let last = bad.len() - 1;
    bad[last] = json!((bad[last].as_u64().unwrap() ^ 0xff) & 0xff);
    let got = call(&host, "RSA_PSS_VERIFY", &[spki, message, json!(bad)]);
    assert_eq!(got, json!(false), "a tampered signature must not verify");
}

#[test]
fn a_malformed_public_key_is_an_error_not_a_false() {
    let host = TestHost::new();
    let v = vectors();
    let message = hex_to_json_array(v["message"].as_str().unwrap());
    let signature = hex_to_json_array(v["ed25519"]["signature"].as_str().unwrap());
    // Ed25519 的公钥必须是 32 字节 ⇒ 给 3 字节 ⇒ **错误**（不是 FALSE）。
    let err = call_err(
        &host,
        "ED25519_VERIFY",
        &[json!([1, 2, 3]), message.clone(), signature.clone()],
    );
    assert_eq!(err["kind"], json!("CryptoError"));
    assert_eq!(err["op"], json!("ED25519_VERIFY"));
    assert!(
        err["reason"].as_str().unwrap().contains("32 bytes"),
        "the reason must say what is wrong: {err}"
    );
}

#[test]
fn the_wrong_arity_is_an_error() {
    let host = TestHost::new();
    let err = call_err(&host, "SHA384", &[]);
    assert_eq!(err["kind"], json!("CryptoError"));
    assert_eq!(err["op"], json!("SHA384"));
    assert!(
        err["reason"].as_str().unwrap().contains("expects 1 argument"),
        "the reason must say the arity: {err}"
    );
}

#[test]
fn a_wrong_length_aes_key_is_an_error() {
    let host = TestHost::new();
    let v = vectors();
    let a = &v["aes_256_gcm"];
    let err = call_err(
        &host,
        "AES_256_GCM",
        &[
            json!([1, 2, 3]),
            hex_to_json_array(a["nonce"].as_str().unwrap()),
            hex_to_json_array(a["aad"].as_str().unwrap()),
            hex_to_json_array(a["plaintext"].as_str().unwrap()),
        ],
    );
    assert_eq!(err["kind"], json!("CryptoError"));
    assert!(
        err["reason"].as_str().unwrap().contains("32 bytes"),
        "the reason must say the key length: {err}"
    );
}

#[test]
fn bytes_may_be_given_as_a_string_or_as_an_array() {
    let host = TestHost::new();
    let v = vectors();
    // ⚠ `STRING` ⇒ 取它的 **UTF-8 字节**（**不是** hex 解码）。
    let as_string = call(&host, "SHA384", &[json!("wlwl")]);
    let as_array = call(
        &host,
        "SHA384",
        &[hex_to_json_array(v["digests"]["input"].as_str().unwrap())],
    );
    assert_eq!(as_string, as_array);
    // ⚠ 而且字符串形态**不**等于「把同一个字符串当 hex 解码」。
    let as_hex_decoded = call(
        &host,
        "SHA384",
        &[hex_to_json_array(
            &v["digests"]["sha384"]
                .as_str()
                .unwrap()
                .get(..8)
                .unwrap()
                .to_string(),
        )],
    );
    assert_ne!(as_string, as_hex_decoded);
}

#[test]
fn the_cert_fixtures_exist() {
    // 免得「夹具没生成 ⇒ 用例莫名其妙地红」。
    for name in [
        "ca.crt",
        "ca.key",
        "server.crt",
        "server.key",
        "expired.crt",
        "expired.key",
        "mismatch.crt",
        "mismatch.key",
        "vectors.json",
    ] {
        let path = fixture(name);
        assert!(
            std::path::Path::new(&path).exists(),
            "the fixture '{name}' is missing (run: python tests/gen_fixture_cert.py)"
        );
    }
    // 顺手确认 hex 工具本身没写错。
    assert_eq!(json_array_to_hex(&json!([0x00, 0x0f, 0xff])), "000fff");
    assert_eq!(
        hex_to_json_array("000fff").as_array().unwrap().len(),
        3,
        "hex decoding must produce one byte per pair"
    );
}
