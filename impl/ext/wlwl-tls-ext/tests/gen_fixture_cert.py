#!/usr/bin/env python3
"""生成 **WLWL TLS 扩展的测试用证书与验签向量**。

⚠⚠⚠ **这些材料只能用于测试** ⚠⚠⚧

- **仅供测试**：它们是为了让 `cargo test` 有东西可测才生成的。
- **仅 127.0.0.1**：服务器证书的 SAN 只有 `localhost` / `127.0.0.1` / `::1`。
- **无 CA 信任**：这里的 CA 是**自己签自己**的测试 CA，**任何真实系统都不信它**。
- **不得用于任何真实服务**：私钥是明文存在仓里的，任何人都能拿到。

用法（**只需跑一次**；产物已经入库）：

```sh
python tests/gen_fixture_cert.py
```

产物：

| 文件 | 内容 |
|---|---|
| `tests/fixtures/ca.crt` / `ca.key` | 测试 CA（自签，`CA:TRUE`） |
| `tests/fixtures/server.crt` / `server.key` | 服务器叶证书（CN=localhost，10 年） |
| `tests/fixtures/expired.crt` / `expired.key` | **已过期**的叶证书 |
| `tests/fixtures/mismatch.crt` / `mismatch.key` | 主机名**不匹配**的叶证书（CN=other.example） |
| `tests/fixtures/vectors.json` | 验签向量（Ed25519 / ECDSA P-256 / RSA-PSS），全部 hex |

⚠ 本机**没有** `openssl` ⇒ 用 Python 的 `cryptography` 库生成（可复现）。
"""

from __future__ import annotations

import datetime
import hashlib
import hmac
import json
import pathlib

from cryptography import x509
from cryptography.hazmat.primitives import hashes, serialization
from cryptography.hazmat.primitives.asymmetric import ec, ed25519, padding, rsa
from cryptography.hazmat.primitives.ciphers.aead import AESGCM
from cryptography.hazmat.primitives.kdf.hkdf import HKDF
from cryptography.x509.oid import NameOID

HERE = pathlib.Path(__file__).resolve().parent
FIXTURES = HERE / "fixtures"

# 「10 年」—— 用 3650 天（闰日不算，够测试用）。
TEN_YEARS = datetime.timedelta(days=3650)
NOW = datetime.datetime.now(datetime.timezone.utc)


def _pem_key(key) -> bytes:
    """私钥 ⇒ PKCS#8 PEM（rustls 的 `PrivateKeyDer::from_pem_file` 认这个）。"""
    return key.private_bytes(
        encoding=serialization.Encoding.PEM,
        format=serialization.PrivateFormat.PKCS8,
        encryption_algorithm=serialization.NoEncryption(),
    )


def _pem_cert(cert) -> bytes:
    return cert.public_bytes(serialization.Encoding.PEM)


def _name(cn: str) -> x509.Name:
    return x509.Name(
        [
            x509.NameAttribute(NameOID.ORGANIZATION_NAME, "WLWL Test Only"),
            x509.NameAttribute(NameOID.COMMON_NAME, cn),
        ]
    )


def _make_ca() -> tuple[x509.Certificate, ec.EllipticCurvePrivateKey]:
    key = ec.generate_private_key(ec.SECP256R1())
    name = _name("WLWL Test Root CA")
    cert = (
        x509.CertificateBuilder()
        .subject_name(name)
        .issuer_name(name)
        .public_key(key.public_key())
        .serial_number(x509.random_serial_number())
        .not_valid_before(NOW - datetime.timedelta(days=365))
        .not_valid_after(NOW + TEN_YEARS)
        .add_extension(x509.BasicConstraints(ca=True, path_length=None), critical=True)
        .add_extension(
            x509.KeyUsage(
                digital_signature=False,
                content_commitment=False,
                key_encipherment=False,
                data_encipherment=False,
                key_agreement=False,
                key_cert_sign=True,
                crl_sign=True,
                encipher_only=False,
                decipher_only=False,
            ),
            critical=True,
        )
        .add_extension(
            x509.SubjectKeyIdentifier.from_public_key(key.public_key()), critical=False
        )
        .sign(key, hashes.SHA256())
    )
    return cert, key


def _make_leaf(
    ca_cert: x509.Certificate,
    ca_key: ec.EllipticCurvePrivateKey,
    cn: str,
    san: list,
    not_before: datetime.datetime,
    not_after: datetime.datetime,
) -> tuple[x509.Certificate, ec.EllipticCurvePrivateKey]:
    key = ec.generate_private_key(ec.SECP256R1())
    cert = (
        x509.CertificateBuilder()
        .subject_name(_name(cn))
        .issuer_name(ca_cert.subject)
        .public_key(key.public_key())
        .serial_number(x509.random_serial_number())
        .not_valid_before(not_before)
        .not_valid_after(not_after)
        .add_extension(x509.BasicConstraints(ca=False, path_length=None), critical=True)
        .add_extension(
            x509.SubjectAlternativeName(san), critical=False
        )
        .add_extension(
            x509.ExtendedKeyUsage([x509.oid.ExtendedKeyUsageOID.SERVER_AUTH]), critical=False
        )
        .add_extension(
            x509.SubjectKeyIdentifier.from_public_key(key.public_key()), critical=False
        )
        .add_extension(
            x509.AuthorityKeyIdentifier.from_issuer_public_key(ca_cert.public_key()),
            critical=False,
        )
        .sign(ca_key, hashes.SHA256())
    )
    return cert, key


def _der_to_fixed_ecdsa_signature(der: bytes) -> bytes:
    """ASN.1 DER 的 ECDSA 签名 ⇒ ring 要的 **定长** `r || s`（各 32 字节）。"""
    # SEQUENCE { INTEGER r, INTEGER s }
    assert der[0] == 0x30, "not a DER SEQUENCE"
    idx = 2 if der[1] < 0x80 else 2 + (der[1] & 0x7F)
    values = []
    for _ in range(2):
        assert der[idx] == 0x02, "not a DER INTEGER"
        length = der[idx + 1]
        raw = der[idx + 2 : idx + 2 + length]
        # 去掉符号位用的前导 00
        raw = raw.lstrip(b"\x00") or b"\x00"
        values.append(raw)
        idx += 2 + length
    r, s = values
    assert len(r) <= 32 and len(s) <= 32, "P-256 signature parts must fit in 32 bytes"
    return r.rjust(32, b"\x00") + s.rjust(32, b"\x00")


def _vectors() -> dict:
    """Ed25519 / ECDSA P-256 / RSA-PSS 的验签向量 + 摘要/AEAD 交叉向量（hex）。"""
    message = b"wlwl tls extension crypto test vector"

    # ── Ed25519 ──────────────────────────────────────────────────────
    ed_key = ed25519.Ed25519PrivateKey.generate()
    ed_pub = ed_key.public_key().public_bytes(
        encoding=serialization.Encoding.Raw,
        format=serialization.PublicFormat.Raw,
    )
    ed_sig = ed_key.sign(message)
    # 坏签名：把最后一个字节翻掉。
    ed_bad = bytearray(ed_sig)
    ed_bad[-1] ^= 0xFF

    # ── ECDSA P-256（SHA-256，定长 r||s）─────────────────────────────
    ec_key = ec.generate_private_key(ec.SECP256R1())
    ec_pub_der = ec_key.public_key().public_bytes(
        encoding=serialization.Encoding.DER,
        format=serialization.PublicFormat.SubjectPublicKeyInfo,
    )
    ec_sig_der = ec_key.sign(message, ec.ECDSA(hashes.SHA256()))
    ec_sig = _der_to_fixed_ecdsa_signature(ec_sig_der)

    # ── RSA-PSS（SHA-256，MGF1-SHA256，salt 长度 = 摘要长度）─────────
    # ⚠ ring 的 PSS **要求 salt 长度 = 摘要长度**（`s_len = h_len`）⇒ 32。
    rsa_key = rsa.generate_private_key(public_exponent=65537, key_size=2048)
    rsa_pub_der = rsa_key.public_key().public_bytes(
        encoding=serialization.Encoding.DER,
        format=serialization.PublicFormat.SubjectPublicKeyInfo,
    )
    rsa_sig = rsa_key.sign(
        message,
        padding.PSS(mgf=padding.MGF1(hashes.SHA256()), salt_length=32),
        hashes.SHA256(),
    )

    # ── 摘要 / MAC / KDF / AEAD（**独立实现**交叉向量）──────────────
    # ⚠ 这些值由 **Python**（hashlib / hmac / cryptography）算出来 ⇒ 与
    # Rust 那边的 `ring` 是**两套独立实现** ⇒ 能抓出「两边算法口径不同」。
    digest_input = b"wlwl"
    hmac_key = bytes(range(20))
    hmac_data = b"wlwl hmac test"
    hkdf_ikm = bytes(range(22))
    hkdf_salt = bytes(range(13))
    hkdf_info = bytes(range(10))
    hkdf_len = 42
    aes_key = bytes(range(32))
    aes_nonce = bytes(range(12))
    aes_aad = b"wlwl aad"
    aes_plaintext = b"wlwl aes-256-gcm plaintext"

    hkdf_okm = HKDF(
        algorithm=hashes.SHA256(), length=hkdf_len, salt=hkdf_salt, info=hkdf_info
    ).derive(hkdf_ikm)

    aesgcm = AESGCM(aes_key)
    aes_ct = aesgcm.encrypt(aes_nonce, aes_plaintext, aes_aad)

    return {
        "message": message.hex(),
        "ed25519": {
            "pubkey": ed_pub.hex(),
            "signature": ed_sig.hex(),
            "bad_signature": bytes(ed_bad).hex(),
        },
        "ecdsa_p256": {
            "spki": ec_pub_der.hex(),
            "signature": ec_sig.hex(),
        },
        "rsa_pss": {
            "spki": rsa_pub_der.hex(),
            "signature": rsa_sig.hex(),
        },
        "digests": {
            "input": digest_input.hex(),
            "sha384": hashlib.sha384(digest_input).hexdigest(),
            "sha512": hashlib.sha512(digest_input).hexdigest(),
        },
        "hmac_sha512": {
            "key": hmac_key.hex(),
            "data": hmac_data.hex(),
            "tag": hmac.new(hmac_key, hmac_data, hashlib.sha512).hexdigest(),
        },
        "hkdf_sha256": {
            "ikm": hkdf_ikm.hex(),
            "salt": hkdf_salt.hex(),
            "info": hkdf_info.hex(),
            "length": hkdf_len,
            "okm": hkdf_okm.hex(),
        },
        "aes_256_gcm": {
            "key": aes_key.hex(),
            "nonce": aes_nonce.hex(),
            "aad": aes_aad.hex(),
            "plaintext": aes_plaintext.hex(),
            "ciphertext": aes_ct[:-16].hex(),
            "tag": aes_ct[-16:].hex(),
        },
    }


def main() -> None:
    FIXTURES.mkdir(parents=True, exist_ok=True)

    ca_cert, ca_key = _make_ca()

    # 服务器叶证书：CN=localhost，SAN = localhost / 127.0.0.1 / ::1，10 年。
    server_cert, server_key = _make_leaf(
        ca_cert,
        ca_key,
        cn="localhost",
        san=[
            x509.DNSName("localhost"),
            x509.IPAddress(__import__("ipaddress").ip_address("127.0.0.1")),
            x509.IPAddress(__import__("ipaddress").ip_address("::1")),
        ],
        not_before=NOW - datetime.timedelta(days=1),
        not_after=NOW + TEN_YEARS,
    )

    # 已过期的叶证书（有效期在**过去**）。
    expired_cert, expired_key = _make_leaf(
        ca_cert,
        ca_key,
        cn="localhost",
        san=[
            x509.DNSName("localhost"),
            x509.IPAddress(__import__("ipaddress").ip_address("127.0.0.1")),
        ],
        not_before=NOW - datetime.timedelta(days=730),
        not_after=NOW - datetime.timedelta(days=365),
    )

    # 主机名**不匹配**的叶证书（链是好的 ⇒ 只有名字对不上）。
    mismatch_cert, mismatch_key = _make_leaf(
        ca_cert,
        ca_key,
        cn="other.example",
        san=[x509.DNSName("other.example")],
        not_before=NOW - datetime.timedelta(days=1),
        not_after=NOW + TEN_YEARS,
    )

    files = {
        "ca.crt": _pem_cert(ca_cert),
        "ca.key": _pem_key(ca_key),
        "server.crt": _pem_cert(server_cert),
        "server.key": _pem_key(server_key),
        "expired.crt": _pem_cert(expired_cert),
        "expired.key": _pem_key(expired_key),
        "mismatch.crt": _pem_cert(mismatch_cert),
        "mismatch.key": _pem_key(mismatch_key),
    }
    for name, content in files.items():
        (FIXTURES / name).write_bytes(content)
        print(f"wrote {name} ({len(content)} bytes)")

    vectors = _vectors()
    (FIXTURES / "vectors.json").write_text(
        json.dumps(vectors, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    print("wrote vectors.json")


if __name__ == "__main__":
    main()
