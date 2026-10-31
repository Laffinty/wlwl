#!/usr/bin/env python3
"""生成 **HTTPS 端到端契约用的测试证书**（`wlwl-cli` 的 `https_e2e_contract.rs`）。

⚠⚠⚠ **这些材料只能用于测试** ⚠⚠⚧

- **仅供测试**：它们是为了让 `cargo test -p wlwl-cli --test https_e2e_contract`
  能跑一次**真 TLS 握手**才生成的。
- **仅 127.0.0.1**：服务器叶证书的 SAN 只有 `localhost` / `127.0.0.1` / `::1`。
- **无 CA 信任**：这里的 CA 是**自己签自己**的测试 CA —— **任何真实系统都不信它**。
- **私钥是明文**：任何拿到这个仓的人都能冒充这些证书。
- **不得用于任何真实服务**。

## 为什么是 **100 年**

夹具里的证书有**有效期** ⇒ 过期那天这条用例会突然变红，而且**红的原因与代码
无关**（是时间到了）。⇒ 这里给 **100 年**（36500 天）：

- 覆盖到本仓的整个可预见寿命（不必每十年重签一次）；
- 100 年 > X.509 的 `UTCTime` 上限（2050）⇒ 落进 `GeneralizedTime` ⇒
  rustls / webpki **认**（webpki 两种都解析）。

## 用法（**只需跑一次**；产物已经入库）

```sh
python impl/crates/wlwl-cli/tests/fixtures/gen_e2e_cert.py
```

产物：

| 文件 | 内容 |
|---|---|
| `ca.crt` / `ca.key` | 测试 CA（自签，`CA:TRUE`）—— 客户端的**信任根** |
| `server.crt` / `server.key` | 服务器叶证书（CN=localhost，SAN 含 127.0.0.1） |
| `other-ca.crt` / `other-ca.key` | **另一个**测试 CA（**什么都不签**）—— 只用来当「错误的信任根」 |

⚠ 客户端的信任根是 **`ca.crt`**（不是 `server.crt`）—— `server.crt` 是**叶**证书。
⚠ **重新生成会换掉全部密钥** ⇒ 已入库的证书与私钥**必须一起**重新生成。
"""

from __future__ import annotations

import datetime
import ipaddress
import pathlib

from cryptography import x509
from cryptography.hazmat.primitives import hashes, serialization
from cryptography.hazmat.primitives.asymmetric import ec
from cryptography.x509.oid import NameOID

HERE = pathlib.Path(__file__).resolve().parent

# 「100 年」—— 36500 天（闰日不算；够用到本仓寿命结束之后）。
ONE_HUNDRED_YEARS = datetime.timedelta(days=36500)
NOW = datetime.datetime.now(datetime.timezone.utc)


def _pem_key(key: ec.EllipticCurvePrivateKey) -> bytes:
    """私钥 ⇒ PKCS#8 PEM（rustls 的 `PrivateKeyDer::from_pem_file` 认这个）。"""
    return key.private_bytes(
        encoding=serialization.Encoding.PEM,
        format=serialization.PrivateFormat.PKCS8,
        encryption_algorithm=serialization.NoEncryption(),
    )


def _pem_cert(cert: x509.Certificate) -> bytes:
    return cert.public_bytes(serialization.Encoding.PEM)


def _name(cn: str) -> x509.Name:
    return x509.Name(
        [
            x509.NameAttribute(NameOID.ORGANIZATION_NAME, "WLWL Test Only"),
            x509.NameAttribute(NameOID.COMMON_NAME, cn),
        ]
    )


def _make_ca(name: str = "WLWL HTTPS E2E Test Root CA") -> tuple[x509.Certificate, ec.EllipticCurvePrivateKey]:
    key = ec.generate_private_key(ec.SECP256R1())
    subject = _name(name)
    cert = (
        x509.CertificateBuilder()
        .subject_name(subject)
        .issuer_name(subject)
        .public_key(key.public_key())
        .serial_number(x509.random_serial_number())
        # ⚠ 起始比「现在」早一年 ⇒ 机器时钟略不准也不会一上来就「证书还没生效」。
        .not_valid_before(NOW - datetime.timedelta(days=365))
        .not_valid_after(NOW + ONE_HUNDRED_YEARS)
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
) -> tuple[x509.Certificate, ec.EllipticCurvePrivateKey]:
    key = ec.generate_private_key(ec.SECP256R1())
    cert = (
        x509.CertificateBuilder()
        .subject_name(_name("localhost"))
        .issuer_name(ca_cert.subject)
        .public_key(key.public_key())
        .serial_number(x509.random_serial_number())
        .not_valid_before(NOW - datetime.timedelta(days=365))
        .not_valid_after(NOW + ONE_HUNDRED_YEARS)
        .add_extension(x509.BasicConstraints(ca=False, path_length=None), critical=True)
        .add_extension(
            x509.SubjectAlternativeName(
                [
                    x509.DNSName("localhost"),
                    x509.IPAddress(ipaddress.ip_address("127.0.0.1")),
                    x509.IPAddress(ipaddress.ip_address("::1")),
                ]
            ),
            critical=False,
        )
        .add_extension(
            x509.ExtendedKeyUsage([x509.oid.ExtendedKeyUsageOID.SERVER_AUTH]),
            critical=False,
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


def main() -> None:
    ca_cert, ca_key = _make_ca()
    server_cert, server_key = _make_leaf(ca_cert, ca_key)

    # ⚠ **第二个** CA：它**什么都不签** ⇒ 只用来当「错误的信任根」⇒ 客户端拿它当
    # `ca` 时，服务端那张叶证书的签发者**不在**信任库里 ⇒ 握手必须失败。
    other_cert, other_key = _make_ca(name="WLWL HTTPS E2E Other Root CA")

    files = {
        "ca.crt": _pem_cert(ca_cert),
        "ca.key": _pem_key(ca_key),
        "other-ca.crt": _pem_cert(other_cert),
        "other-ca.key": _pem_key(other_key),
        "server.crt": _pem_cert(server_cert),
        "server.key": _pem_key(server_key),
    }
    for name, content in files.items():
        (HERE / name).write_bytes(content)
        print(f"wrote {name} ({len(content)} bytes)")


if __name__ == "__main__":
    main()
