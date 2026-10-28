# ⚠⚠⚠ 测试专用材料 —— **不得用于任何真实服务** ⚠⚠⚧

本目录里的**每一个**文件（证书、私钥、验签向量）都**只为 `cargo test`
服务**而存在。

| 事实 | 说明 |
|---|---|
| **仅供测试** | 它们是为了让 `impl/ext/wlwl-tls-ext` 的测试有东西可测才生成的。 |
| **仅 127.0.0.1** | 服务器叶证书的 SAN 只有 `localhost` / `127.0.0.1` / `::1`；测试也只在回环上跑。 |
| **无 CA 信任** | 这里的 CA 是**自己签自己**的测试 CA（`CN=WLWL Test Root CA`）—— **任何真实系统都不信它**，浏览器也不信。 |
| **私钥是明文** | `*.key` 就是完整的私钥。任何拿到这个仓的人都能冒充这些证书。 |
| **不得用于任何真实服务** | 用它们跑真实流量 ⇒ 等于把身份公开。 |

## 文件

| 文件 | 用途 |
|---|---|
| `ca.crt` / `ca.key` | **测试 CA**（自签，`CA:TRUE`）—— 客户端的信任根（`{"ca": "<ca.crt>"}`）。 |
| `server.crt` / `server.key` | **服务器叶证书**（CN=localhost，SAN=`localhost`/`127.0.0.1`/`::1`，有效期 10 年）—— `LISTEN` 的 `cert` / `key`。 |
| `expired.crt` / `expired.key` | **已过期**的叶证书（有效期在**过去**）⇒ 用来测「过期证书被拒」。 |
| `mismatch.crt` / `mismatch.key` | **主机名不匹配**的叶证书（CN=`other.example`，但**由同一个 CA 签发**）⇒ 链是好的、只有名字对不上 ⇒ 用来测「不匹配证书被拒」。 |
| `vectors.json` | 验签/摘要/AEAD 的**交叉向量**（Ed25519 / ECDSA P-256 / RSA-PSS / SHA-384 / SHA-512 / HMAC-SHA-512 / HKDF / AES-256-GCM），全部 hex 编码。 |

⚠ 客户端的信任根是 **`ca.crt`**（不是 `server.crt`）—— `server.crt` 是**叶**证书。

## 重新生成

生成脚本在仓里（便于复现）：

```sh
cd impl/ext/wlwl-tls-ext
python tests/gen_fixture_cert.py
```

⚠ 本机**没有** `openssl` ⇒ 用 Python 的 `cryptography` 库（本机版本 50.0.2）。
⚠ **重新生成会换掉全部密钥** ⇒ 已入库的向量与证书**必须一起**重新生成
（脚本一次全写）。
