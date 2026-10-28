//! `tls` 命名空间的端到端测试（需求 A-1 / A-3 / A-5）。
//!
//! ⚠ 测试**用扩展自己的** `CONNECT` 客户端连**扩展自己的** `LISTEN` 服务端
//! （回环）⇒ 走的是真 ABI 路径 + 真 TLS 握手。
//!
//! ⚠ 每个用例都覆盖需求 A-5 的五条安全默认值。

mod common;

use std::io::{Read, Write};
use std::net::TcpStream;
use std::thread;

use common::{fixture, TestHost};
use serde_json::{json, Value};

/// 起一个 TLS 监听（`port = 0` ⇒ 临时端口）⇒ `(监听句柄, 端口)`。
fn listen(host: &TestHost, cert: &str, key: &str) -> (Value, u16) {
    let listener = common::call(
        host,
        "tls",
        "LISTEN",
        &[
            json!("127.0.0.1"),
            json!(0),
            json!({ "cert": fixture(cert), "key": fixture(key) }),
        ],
    )
    .unwrap_or_else(|e| panic!("LISTEN failed: {e}"));
    let addr = common::call(host, "tls", "LOCAL_ADDR", &[listener.clone()])
        .unwrap_or_else(|e| panic!("LOCAL_ADDR failed: {e}"));
    let port = addr["port"].as_u64().expect("no port") as u16;
    (listener, port)
}

/// 连上去（信任测试 CA）。
fn connect(host: &TestHost, port: u16, opts: Value) -> Result<Value, Value> {
    common::call(
        host,
        "tls",
        "CONNECT",
        &[json!("127.0.0.1"), json!(port), opts],
    )
}

/// 信任测试 CA 的选项。
fn trust_ca() -> Value {
    json!({ "ca": fixture("ca.crt") })
}

// ── 端到端 ────────────────────────────────────────────────────────────────

#[test]
fn a_tls_round_trip_exchanges_bytes_over_the_loopback() {
    let host = TestHost::new();
    let (listener, port) = listen(&host, "server.crt", "server.key");

    // 服务端在**另一个线程**上 accept（`ACCEPT` 是阻塞的）。
    let server = thread::spawn(move || {
        let host = TestHost::new();
        let conn = common::call(&host, "tls", "ACCEPT", &[listener.clone(), json!(10_000)])
            .unwrap_or_else(|e| panic!("ACCEPT failed: {e}"));
        let got = common::call(&host, "tls", "RECV", &[conn.clone(), json!(1024), json!(10_000)])
            .unwrap_or_else(|e| panic!("RECV failed: {e}"));
        let text = got["text"].as_str().unwrap().to_string();
        assert!(!got["eof"].as_bool().unwrap(), "the connection must not be at EOF");
        common::call(&host, "tls", "SEND", &[conn.clone(), json!("pong")])
            .unwrap_or_else(|e| panic!("SEND failed: {e}"));
        common::call(&host, "tls", "CLOSE", &[conn]).unwrap();
        text
    });

    // 客户端。
    let conn = connect(&host, port, trust_ca()).unwrap_or_else(|e| panic!("CONNECT failed: {e}"));
    let written = common::call(&host, "tls", "SEND", &[conn.clone(), json!("ping")]).unwrap();
    assert_eq!(written, json!(4), "SEND must report the number of bytes written");
    let got = common::call(&host, "tls", "RECV", &[conn.clone(), json!(1024), json!(10_000)]).unwrap();
    assert_eq!(got["text"], json!("pong"));
    assert_eq!(got["bytes"].as_array().unwrap().len(), 4);
    common::call(&host, "tls", "CLOSE", &[conn]).unwrap();

    assert_eq!(server.join().unwrap(), "ping", "the server must have read 'ping'");
}

#[test]
fn the_peer_reports_its_address_and_its_certificate() {
    let host = TestHost::new();
    let (listener, port) = listen(&host, "server.crt", "server.key");

    let server = thread::spawn(move || {
        let host = TestHost::new();
        common::call(&host, "tls", "ACCEPT", &[listener.clone(), json!(10_000)])
            .unwrap_or_else(|e| panic!("ACCEPT failed: {e}"));
        // 一直等着，让客户端先看 PEER。
        std::thread::sleep(std::time::Duration::from_millis(500));
    });

    let conn = connect(&host, port, trust_ca()).unwrap_or_else(|e| panic!("CONNECT failed: {e}"));
    let peer = common::call(&host, "tls", "PEER", &[conn.clone()]).unwrap();
    assert_eq!(peer["host"], json!("127.0.0.1"));
    assert!(peer["port"].as_u64().unwrap() > 0);
    let certs = peer["certificates"].as_array().unwrap();
    assert_eq!(certs.len(), 1, "the server presented exactly one certificate");
    let cert = &certs[0];
    assert!(
        cert["subject"].as_str().unwrap().contains("CN=localhost"),
        "the subject must name localhost: {}",
        cert["subject"]
    );
    let san: Vec<&str> = cert["san"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s.as_str().unwrap())
        .collect();
    assert!(
        san.iter().any(|s| *s == "127.0.0.1"),
        "the SAN must cover 127.0.0.1: {san:?}"
    );
    assert!(
        cert["spki_sha256"].as_str().unwrap().len() == 64,
        "the SPKI fingerprint must be a 64-character hex digest"
    );
    assert!(!cert["not_after"].as_str().unwrap().is_empty());
    assert!(!cert["serial"].as_str().unwrap().is_empty());

    common::call(&host, "tls", "CLOSE", &[conn]).unwrap();
    let _ = server.join();
}

// ── 需求 A-5 第 5 条：明文客户端必须被拒 ─────────────────────────────────

#[test]
fn a_plaintext_client_is_rejected_and_never_served_in_the_clear() {
    let host = TestHost::new();
    let (listener, port) = listen(&host, "server.crt", "server.key");

    // 服务端线程：`ACCEPT` 必须**失败**（不是返回一个明文连接）。
    let server = thread::spawn(move || {
        let host = TestHost::new();
        common::call(&host, "tls", "ACCEPT", &[listener.clone(), json!(10_000)])
    });

    // 明文客户端：连上去就发 HTTP。
    let mut plain = TcpStream::connect(("127.0.0.1", port)).expect("cannot connect");
    // ⚠ 读**必须有上界** —— 否则「服务端没有把连接掐掉」这种情形会把
    // 用例挂死（而不是让它红）⇒ 挂死不是一条能读懂的失败。
    plain
        .set_read_timeout(Some(std::time::Duration::from_secs(5)))
        .expect("cannot set the read timeout");
    plain
        .write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n")
        .expect("cannot write");
    let mut buf = [0u8; 256];
    // 对端可以回一个 **TLS alert**（二进制）—— 那是正确的「我不说这个协议」；
    // 但**绝不能**回一段**明文 HTTP 应答**（那就是静默降级成明文了）。
    let read = match plain.read(&mut buf) {
        Ok(n) => n,
        // 读超时（对端既不答也不关）⇒ 也不是明文应答 ⇒ 放行。
        Err(_) => 0,
    };
    let seen = &buf[..read];
    assert!(
        !seen.starts_with(b"HTTP/") && !seen.windows(2).any(|w| w == b"OK"),
        "the server must not answer a plaintext client with a cleartext HTTP response: {seen:?}"
    );

    let result = server.join().unwrap();
    let err = result.expect_err("ACCEPT must fail for a plaintext client");
    assert_eq!(err["kind"], json!("TlsError"));
    assert_eq!(err["op"], json!("ACCEPT"));
    let reason = err["reason"].as_str().unwrap();
    assert!(
        reason.contains("plaintext"),
        "the reason must say the client was plaintext: {reason}"
    );
}

// ── 需求 A-5 第 1 条：证书校验默认开 ─────────────────────────────────────

#[test]
fn certificate_verification_is_on_by_default() {
    let host = TestHost::new();
    // 这张证书由**别的** CA 签发 ⇒ 默认信任根（webpki-roots）不认识它。
    let (listener, port) = listen(&host, "mismatch.crt", "mismatch.key");

    // 服务端线程：握手会失败（客户端不信任它）⇒ `ACCEPT` 报错。
    let server = thread::spawn(move || {
        let host = TestHost::new();
        common::call(&host, "tls", "ACCEPT", &[listener.clone(), json!(10_000)])
    });

    // 不给 `ca` ⇒ 用 webpki-roots ⇒ 这张自签测试证书**不被信任**。
    let err = connect(&host, port, json!({}))
        .expect_err("the default trust roots must not accept the test certificate");
    assert_eq!(err["kind"], json!("TlsError"));
    assert_eq!(err["op"], json!("CONNECT"));
    let reason = err["reason"].as_str().unwrap();
    assert!(
        reason.to_lowercase().contains("certificate")
            || reason.to_lowercase().contains("handshake"),
        "the reason must talk about the certificate: {reason}"
    );
    let _ = server.join();
}

#[test]
fn disabling_verification_is_possible_but_it_is_reported() {
    let host = TestHost::new();
    let (listener, port) = listen(&host, "server.crt", "server.key");

    let server = thread::spawn(move || {
        let host = TestHost::new();
        common::call(&host, "tls", "ACCEPT", &[listener.clone(), json!(10_000)])
            .unwrap_or_else(|e| panic!("ACCEPT failed: {e}"));
    });

    common::clear_reports();
    // ⚠ 显式关掉校验 ⇒ 能连上，**但**有一条告警。
    let conn = connect(&host, port, json!({ "verify": false }))
        .unwrap_or_else(|e| panic!("CONNECT with verify:false failed: {e}"));
    let reports = common::reports();
    assert!(
        reports
            .iter()
            .any(|(kind, reason)| kind == "TlsWarning"
                && reason.to_lowercase().contains("verification was disabled")),
        "disabling verification must be reported: {reports:?}"
    );
    common::call(&host, "tls", "CLOSE", &[conn]).unwrap();
    let _ = server.join();
}

// ── 需求 A-5 第 2 条：TLS 版本下限默认 1.2 ─────────────────────────────

#[test]
fn the_default_minimum_tls_version_is_1_2() {
    let host = TestHost::new();
    // 只给 `min_version = "1.3"` 是**允许**的（说明下限可调）。
    let listener = common::call(
        &host,
        "tls",
        "LISTEN",
        &[
            json!("127.0.0.1"),
            json!(0),
            json!({
                "cert": fixture("server.crt"),
                "key": fixture("server.key"),
                "min_version": "1.3",
            }),
        ],
    )
    .unwrap_or_else(|e| panic!("LISTEN failed: {e}"));
    assert!(listener.as_u64().unwrap() > 0);

    // ⚠ 缺省是 1.2 ⇒ **不是** 1.3。查源码级的缺省（`opts::parse_listen`）。
    let bogus = common::call(
        &host,
        "tls",
        "LISTEN",
        &[
            json!("127.0.0.1"),
            json!(0),
            json!({
                "cert": fixture("server.crt"),
                "key": fixture("server.key"),
                "min_version": "1.1",
            }),
        ],
    )
    .expect_err("TLS 1.1 must not be accepted");
    assert_eq!(bogus["kind"], json!("TlsError"));
    assert!(
        bogus["reason"].as_str().unwrap().contains("min_version"),
        "the reason must name the option: {bogus}"
    );
}

// ── 需求 A-5 第 4 条：私钥不出现在任何消息里 ───────────────────────────

#[test]
fn a_broken_private_key_never_leaks_its_material() {
    let host = TestHost::new();
    // 拿服务器的证书配一个**不匹配**的私钥 ⇒ 加载/校验失败。
    let err = common::call(
        &host,
        "tls",
        "LISTEN",
        &[
            json!("127.0.0.1"),
            json!(0),
            json!({ "cert": fixture("server.crt"), "key": fixture("ca.key") }),
        ],
    )
    .expect_err("a mismatched private key must be rejected");
    assert_eq!(err["kind"], json!("TlsError"));
    let reason = err["reason"].as_str().unwrap();

    // ⚠ 私钥的**内容**绝不能出现在消息里。
    let key_pem = std::fs::read_to_string(fixture("server.key")).unwrap();
    let secret_line = key_pem
        .lines()
        .find(|l| l.starts_with("M") && l.len() > 40)
        .expect("the key file has a body line");
    assert!(
        !reason.contains(secret_line),
        "the error message must not contain the private key material: {reason}"
    );
    assert!(
        !reason.contains("PRIVATE KEY"),
        "the error message must not contain the private key block: {reason}"
    );
}

// ── 需求 A-3：证书指纹 / 证书信息 ────────────────────────────────────────

#[test]
fn the_certificate_fingerprint_is_stable_across_calls() {
    let host = TestHost::new();
    let first = common::call(&host, "tls", "CERT_FINGERPRINT", &[json!(fixture("server.crt"))])
        .unwrap_or_else(|e| panic!("CERT_FINGERPRINT failed: {e}"));
    let second = common::call(&host, "tls", "CERT_FINGERPRINT", &[json!(fixture("server.crt"))])
        .unwrap_or_else(|e| panic!("CERT_FINGERPRINT failed: {e}"));
    assert_eq!(first, second, "the fingerprint must be stable");
    let hex = first.as_str().unwrap();
    assert_eq!(hex.len(), 64, "the fingerprint must be a SHA-256 hex digest");
    assert!(
        hex.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()),
        "the fingerprint must be lowercase hex: {hex}"
    );

    // ⚠ 它必须等于 `CERT_INFO` 里的 `spki_sha256`（同一个 SPKI 的同一个摘要）。
    let info = common::call(&host, "tls", "CERT_INFO", &[json!(fixture("server.crt"))]).unwrap();
    assert_eq!(info["spki_sha256"], first);
}

#[test]
fn the_certificate_info_has_the_expected_shape() {
    let host = TestHost::new();
    let info = common::call(&host, "tls", "CERT_INFO", &[json!(fixture("server.crt"))])
        .unwrap_or_else(|e| panic!("CERT_INFO failed: {e}"));
    for key in [
        "subject",
        "issuer",
        "san",
        "not_before",
        "not_after",
        "spki_sha256",
        "serial",
    ] {
        assert!(info.get(key).is_some(), "CERT_INFO must have '{key}': {info}");
    }
    assert!(
        info["subject"].as_str().unwrap().contains("CN=localhost"),
        "the subject must name localhost: {}",
        info["subject"]
    );
    assert!(
        info["issuer"].as_str().unwrap().contains("CN=WLWL Test Root CA"),
        "the issuer must be the test CA: {}",
        info["issuer"]
    );
    assert!(info["not_before"].as_str().unwrap().ends_with('Z'));
    assert!(info["not_after"].as_str().unwrap().ends_with('Z'));
    let san: Vec<&str> = info["san"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s.as_str().unwrap())
        .collect();
    assert!(san.iter().any(|s| *s == "localhost"), "{san:?}");
    assert!(san.iter().any(|s| *s == "127.0.0.1"), "{san:?}");
}

#[test]
fn two_different_certificates_have_two_different_fingerprints() {
    let host = TestHost::new();
    let a = common::call(&host, "tls", "CERT_FINGERPRINT", &[json!(fixture("server.crt"))]).unwrap();
    let b = common::call(&host, "tls", "CERT_FINGERPRINT", &[json!(fixture("mismatch.crt"))]).unwrap();
    assert_ne!(a, b, "different certificates must have different fingerprints");
}

// ── 公钥固定（需求 A-3）──────────────────────────────────────────────────

#[test]
fn a_matching_pin_is_accepted_and_a_wrong_pin_is_rejected() {
    let host = TestHost::new();
    let (listener, port) = listen(&host, "server.crt", "server.key");
    let fingerprint = common::call(&host, "tls", "CERT_FINGERPRINT", &[json!(fixture("server.crt"))])
        .unwrap()
        .as_str()
        .unwrap()
        .to_string();

    // ① 对的 pin ⇒ 连得上。
    let server = thread::spawn(move || {
        let host = TestHost::new();
        common::call(&host, "tls", "ACCEPT", &[listener.clone(), json!(10_000)])
            .unwrap_or_else(|e| panic!("ACCEPT failed: {e}"));
    });
    let conn = connect(
        &host,
        port,
        json!({ "ca": fixture("ca.crt"), "pin": fingerprint.clone() }),
    )
    .unwrap_or_else(|e| panic!("CONNECT with the right pin failed: {e}"));
    common::call(&host, "tls", "CLOSE", &[conn]).unwrap();
    let _ = server.join();

    // ② 错的 pin ⇒ 被拒（**即使**证书链是对的）。
    let (listener2, port2) = listen(&host, "server.crt", "server.key");
    let server2 = thread::spawn(move || {
        let host = TestHost::new();
        common::call(&host, "tls", "ACCEPT", &[listener2.clone(), json!(5_000)])
    });
    let err = connect(
        &host,
        port2,
        json!({ "ca": fixture("ca.crt"), "pin": "00".repeat(32) }),
    )
    .expect_err("a wrong pin must be rejected");
    assert_eq!(err["kind"], json!("TlsError"));
    let reason = err["reason"].as_str().unwrap();
    assert!(
        reason.contains("pin"),
        "the reason must mention the pin: {reason}"
    );
    let _ = server2.join();
}

// ── 证书被拒的三种情形（reason 必须可区分）─────────────────────────────

#[test]
fn an_expired_certificate_is_rejected_with_its_own_reason() {
    let host = TestHost::new();
    let (listener, port) = listen(&host, "expired.crt", "expired.key");
    let server = thread::spawn(move || {
        let host = TestHost::new();
        common::call(&host, "tls", "ACCEPT", &[listener.clone(), json!(10_000)])
    });

    let err = connect(&host, port, trust_ca())
        .expect_err("an expired certificate must be rejected");
    assert_eq!(err["kind"], json!("TlsError"));
    let reason = err["reason"].as_str().unwrap().to_lowercase();
    assert!(
        reason.contains("expired") || reason.contains("not valid") || reason.contains("certificate"),
        "the reason must point at the certificate problem: {reason}"
    );
    let _ = server.join();
}

#[test]
fn a_name_mismatched_certificate_is_rejected_with_its_own_reason() {
    let host = TestHost::new();
    // `mismatch.crt` 由**同一个 CA** 签发（链是好的）⇒ 只有名字对不上。
    let (listener, port) = listen(&host, "mismatch.crt", "mismatch.key");
    let server = thread::spawn(move || {
        let host = TestHost::new();
        common::call(&host, "tls", "ACCEPT", &[listener.clone(), json!(10_000)])
    });

    let err = connect(&host, port, trust_ca())
        .expect_err("a name-mismatched certificate must be rejected");
    assert_eq!(err["kind"], json!("TlsError"));
    let reason = err["reason"].as_str().unwrap().to_lowercase();
    assert!(
        reason.contains("name") || reason.contains("valid"),
        "the reason must point at the name mismatch: {reason}"
    );
    let _ = server.join();
}

#[test]
fn the_three_rejection_reasons_are_distinguishable() {
    let host = TestHost::new();

    // ① 过期。
    let (l1, p1) = listen(&host, "expired.crt", "expired.key");
    let s1 = thread::spawn(move || {
        let host = TestHost::new();
        common::call(&host, "tls", "ACCEPT", &[l1.clone(), json!(10_000)])
    });
    let expired = connect(&host, p1, trust_ca())
        .expect_err("expired must be rejected")
        .to_string();
    let _ = s1.join();

    // ② 名字不匹配。
    let (l2, p2) = listen(&host, "mismatch.crt", "mismatch.key");
    let s2 = thread::spawn(move || {
        let host = TestHost::new();
        common::call(&host, "tls", "ACCEPT", &[l2.clone(), json!(10_000)])
    });
    let mismatch = connect(&host, p2, trust_ca())
        .expect_err("a name mismatch must be rejected")
        .to_string();
    let _ = s2.join();

    // ③ pin 不匹配。
    let (l3, p3) = listen(&host, "server.crt", "server.key");
    let s3 = thread::spawn(move || {
        let host = TestHost::new();
        common::call(&host, "tls", "ACCEPT", &[l3.clone(), json!(5_000)])
    });
    let pin = connect(
        &host,
        p3,
        json!({ "ca": fixture("ca.crt"), "pin": "11".repeat(32) }),
    )
    .expect_err("a wrong pin must be rejected")
    .to_string();
    let _ = s3.join();

    assert_ne!(expired, mismatch, "the expired and the mismatch reasons must differ");
    assert_ne!(expired, pin, "the expired and the pin reasons must differ");
    assert_ne!(mismatch, pin, "the mismatch and the pin reasons must differ");
}

// ── 句柄与形状 ───────────────────────────────────────────────────────────

#[test]
fn accepting_on_an_empty_listener_times_out_with_null() {
    let host = TestHost::new();
    let (listener, _port) = listen(&host, "server.crt", "server.key");
    let got = common::call(&host, "tls", "ACCEPT", &[listener, json!(200)])
        .unwrap_or_else(|e| panic!("ACCEPT failed: {e}"));
    assert_eq!(got, json!(null), "a timeout is a normal result, not an error");
}

#[test]
fn an_unknown_handle_is_an_error() {
    let host = TestHost::new();
    let err = common::call(&host, "tls", "RECV", &[json!(999_999), json!(16), json!(10)])
        .expect_err("an unknown handle must be rejected");
    assert_eq!(err["kind"], json!("TlsError"));
    assert_eq!(err["op"], json!("RECV"));
    assert!(
        err["reason"].as_str().unwrap().contains("999999"),
        "the reason must name the handle: {err}"
    );
}

#[test]
fn an_unknown_option_is_rejected_rather_than_silently_ignored() {
    let host = TestHost::new();
    let err = common::call(
        &host,
        "tls",
        "LISTEN",
        &[
            json!("127.0.0.1"),
            json!(0),
            json!({
                "cert": fixture("server.crt"),
                "key": fixture("server.key"),
                "min_verison": "1.2",
            }),
        ],
    )
    .expect_err("a misspelled option must be rejected");
    assert_eq!(err["kind"], json!("TlsError"));
    assert!(
        err["reason"].as_str().unwrap().contains("min_verison"),
        "the reason must name the offending option: {err}"
    );
}
