# `wlwl-tls-ext` —— WLWL 的 **TLS / crypto 原生扩展**（W-22）

一个**独立编译**的原生动态库，导出扩展 ABI v1 的唯一符号 `wlwl_ext_v1` ⇒
宿主（`wlwl --ext <path>`）加载它之后，脚本就能 `IMPORT` 两个命名空间：
**`tls`** 与 **`crypto`**。

> ⚠⚠ **加载一个原生库 = 把整个进程的权限交给它**（读文件、连网络、改内存、
> 起进程）—— 它**不是沙箱**。宿主在加载前会打印一行醒目告警。

---

## 1. 它**不是**语言面的一部分

扩展命名空间**不进附录 A、不进公共面清单** —— 公共面取自
`wlwl_std::namespace_paths()` 那张静态表 ⇒ **`tls` / `crypto` 不在规范里**，
只有显式 `--ext` 加载了本库的脚本才能用。

## 2. 依赖面：主仓**一个字都没变**

本 crate **不在** wlwl 主 workspace 里 —— 它的 `Cargo.toml` 里有**空
`[workspace]` 表**（自己当自己的 workspace 根）⇒ cargo 不再向上找
`impl/Cargo.toml` ⇒ **主仓的 `Cargo.lock` 不会因为本 crate 多一行**。

第三方依赖只有四个，且**全部已在主仓的 `Cargo.lock` 里**：

| 依赖 | 版本 | 用途 |
|---|---|---|
| `rustls` | 0.23.46 | TLS 1.2 / 1.3（`default-features = false` + `ring`） |
| `ring` | 0.17.14 | 摘要 / MAC / KDF / AEAD / 验签 |
| `webpki-roots` | 1.0.11 | **默认**信任根（Mozilla 根库） |
| `serde_json` | 1.0.154 | 值编码（ABI 的值就是 JSON 文本） |

本 crate 自己出 `Cargo.lock`（**入库**）。

## 3. ABI：单一真相在宿主那边

本 crate **直接引用**宿主的 `impl/crates/wlwl-ext/src/abi.rs`
（`src/abi.rs` 里的 `#[path]`）⇒ **不复制、不重写** ⇒ 宿主改了 ABI，
这边自动跟上。

宿主那份 `abi.rs` 里**只有** `#[repr(C)]` 结构体与函数指针类型 ——
**不引用任何宿主侧的值类型** ⇒ 本 crate 把它当普通模块编进来即可，
**不产生 crate 依赖**。

⇒ 不需要本地副本；**布局守卫**仍然有（`tests/abi_layout.rs`）：它把本 crate
看到的**大小与字段偏移**与 C 头文件 `include/wlwl_ext.h` 里那套 `struct`
声明**逐字对照**（期望值是**从 C 声明手算**的，不是从 Rust 抄的）。

## 4. 成员面

### `tls`（10 个）

| 成员 | 形状 | 说明 |
|---|---|---|
| `LISTEN(host, port, opts)` | `INTEGER`（监听句柄） | `port = 0` ⇒ **临时端口** |
| `ACCEPT(listener, timeout_ms)` | `INTEGER`（连接句柄）/ `NULL` | **握手已完成**；超时 ⇒ `NULL`（**正常值**） |
| `CONNECT(host, port, opts)` | `INTEGER`（连接句柄） | 出站 TLS |
| `SEND(handle, bytes)` | `INTEGER`（写出的字节数） | `bytes` = `STRING` 或 `ARRAY[INTEGER]` |
| `RECV(handle, max_bytes, timeout_ms)` | `{"bytes": …, "text": …, "eof": …}` | 与 `TCP_RECV` **同形** |
| `CLOSE(handle)` | `NULL` | |
| `PEER(handle)` | `{"host": …, "port": …, "certificates": [ … ]}` | 与 `TCP_PEER` **同形** + 证书摘要 |
| `LOCAL_ADDR(handle)` | `{"host": …, "port": …}` | ⚠ **配套成员**，见 §4.1 |
| `CERT_FINGERPRINT(path)` | `STRING` | **SPKI** 的 SHA-256（小写 hex）⇒ 公钥固定用 |
| `CERT_INFO(path)` | `DICT` | `{subject, issuer, san[], not_before, not_after, spki_sha256, serial}` |

### `crypto`（8 个）

| 成员 | 形状 |
|---|---|
| `SHA384(bytes)` | `STRING`（小写 hex） |
| `SHA512(bytes)` | `STRING`（小写 hex） |
| `HMAC_SHA512(key, bytes)` | `STRING`（小写 hex） |
| `HKDF(ikm, salt, info, length, algorithm)` | `STRING`（小写 hex）；`algorithm` ∈ `sha256`/`sha384`/`sha512` |
| `AES_256_GCM(key, nonce, aad, plaintext)` | `{"ciphertext": …, "tag": …}`（hex） |
| `ED25519_VERIFY(pubkey, message, signature)` | `BOOLEAN` |
| `ECDSA_P256_VERIFY(pubkey, message, signature)` | `BOOLEAN` |
| `RSA_PSS_VERIFY(pubkey, message, signature)` | `BOOLEAN` |

⚠ **只验签，不签发** —— 签名生成需要私钥管理，那是**调用方**的责任。

### 4.1 为什么多了 `LOCAL_ADDR`

需求写明「`port = 0` ⇒ 临时端口」，但 `LISTEN` 的返回值是**句柄**（不是
端口）⇒ 临时端口**只能**从别处问出来 ⇒ 加了 `LOCAL_ADDR`（与仓里既有的
`TCP_LOCAL_ADDR` **同名同形**）。它是**加法**，不影响那 9 个成员的任何语义。

### 4.2 字节形态

- **入参**：`ARRAY[INTEGER]`（元素 0..=255）**或** `STRING`（取它的
  **UTF-8 字节** —— ⚠ **不是** hex 解码）。
- **出参**：**一律小写 hex 字符串**；唯一的例外是 `tls.RECV` 的 `bytes`
  （沿用 `TCP_RECV` 的 `ARRAY[INTEGER]` 形状）。

### 4.3 公钥形态

三个 `*_VERIFY` 的 `pubkey` 都接受两种形态（自动判别）：

1. **裸公钥材料** —— Ed25519 的 32 字节；ECDSA P-256 的未压缩点
   `0x04 || X || Y`（65 字节）；RSA 的 **PKCS#1 `RSAPublicKey`** DER；
2. **X.509 `subjectPublicKeyInfo`（SPKI）DER** —— 从里面把公钥材料抠出来。

⇒ 用户手里通常只有一张证书 ⇒ 直接喂 SPKI 就行。

## 5. 安全默认值（需求 A-5）

| # | 规则 | 落点 |
|---|---|---|
| ① | **证书校验默认开** | `CONNECT` 不给 `verify` ⇒ 校验；信任根 = `ca` 指定的文件，**没给就用 `webpki-roots`（Mozilla 根库），不是空信任库** |
| ② | **TLS 版本下限默认 1.2** | `min_version` 缺省 `"1.2"` |
| ③ | **没有「一键关掉全部校验」的开关** | 要关必须**显式**传 `{"verify": false}`，而那会**同时**打一条告警（走宿主的 `report` 通道 ⇒ stderr） |
| ④ | **私钥不出现在任何错误消息 / 告警 / 返回值里** | 私钥的失败消息是**固定文本**（只有路径），**不**把底层错误原文带出来 |
| ⑤ | **明文客户端连 TLS 端口 ⇒ 被拒且给出明确错误** | 握手失败 ⇒ `ERR`（`kind:"TlsError"`），reason 里点明「plaintext」；**绝不**静默降级成明文 |

另：**未知选项名 ⇒ 失败**（不静默忽略 —— 拼错的上界会变成「没有上界」）。

## 6. ⚠ 阻塞语义（ABI v1 的硬限制）

扩展 ABI v1 **没有就绪事件通道**（宿主的 `await_ready` 恒返回 `-1`）⇒
握手 / accept / 读 / 写**都在调用线程上阻塞**，与 std 的 `HTTP_*` / `SLEEP`
同族 ⇒ **等待期间整个运行时是冻结的**。

⇒ 要「等待期间别的任务照常运行」，需要 **ABI v1.1**（**另立**）：那条版本要给
`wlwl_ext_ctx_t` 加一条就绪事件通道 —— **不是**本 crate 能自己解决的。

## 7. 构建与加载

```sh
cd impl/ext/wlwl-tls-ext
cargo build --locked --release        # => target/release/wlwl_tls.dll（Windows）
cargo test  --locked                  # 32 个用例

cd ../..
cargo build --release --bin wlwl      # 宿主
./target/release/wlwl run \
    --ext D:/…/impl/ext/wlwl-tls-ext/target/release/wlwl_tls.dll \
    D:/…/impl/ext/wlwl-tls-ext/tests/host_smoke.wll
```

⚠ `--ext` 的路径必须**绝对**（信任门控 T3）。

## 8. 测试材料

⚠⚠⚠ **仅供测试** —— 详见 [`tests/fixtures/README.md`](tests/fixtures/README.md)。
**仅 127.0.0.1**、**无 CA 信任**、**私钥是明文**、**不得用于任何真实服务**。

生成脚本：[`tests/gen_fixture_cert.py`](tests/gen_fixture_cert.py)（本机没有
`openssl` ⇒ 用 Python 的 `cryptography`）。

## 9. 已知限制

- **阻塞语义**（§6）。
- **平台差异**：Windows 产出 `.dll`，Linux/macOS 产出 `.so` / `.dylib`。
- **X.509 解析是「够用就停」**：本 crate 手写了一个只取需要的字段的 DER /
  X.509 解析器（`src/x509.rs`）—— 它**不**查 CRL / OCSP、不验签、不做路径
  构建。`CERT_INFO` 的 `san` 只收 `dNSName` / `iPAddress` / `rfc822Name` /
  `URI` 四种 `GeneralName`。
- **`name` 按 RFC 4514 写**（RDN 顺序反过来）。
- **RSA-PSS 用 SHA-256 + salt 长度 = 摘要长度**（`ring` 的硬要求）。
- **ECDSA P-256 用 SHA-256 + 定长 `r || s` 签名**（不是 ASN.1 DER）。
- **签名生成不做**（见 §4）。
