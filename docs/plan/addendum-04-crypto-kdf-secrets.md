# 04 · KDF / CSPRNG / 常数时间比较(补 `std.encode` 的安全侧)

> **层级** L1(可并行) · **前置** 无(04 与 08 都涉及熵源,**须共用一个源**)
> **状态** 未动工 · **全局裁决点** G4(KDF 选型)
> → **G4 已由 [`ADR-0026`](../adr/0026-l1-global-decision-points.md)(2026-10-06,
> Status **Accepted**)定稿**:**Argon2id 为主 + PBKDF2 兼容**(两者皆单向,不触碰
> §11.5 对加解密 / 密钥生成的否决);**且新增一条原方案没有的硬约束 ——
> 迭代数与内存的上界必须钉死,超界报 `E0030`**(而非静默接受)。理由:可调参数 +
> 攻击者可调 = 迟早有人调到不安全值。本份 §3.5 的 `m_cost` 硬上界要求因此
> 扩展为**两个算法的全部参数**。

## 0. 契约摘要

- **目标**:把 `std.encode` 从「编码 + 两个哈希」补到**能安全处理口令与
  密钥比较**的最小闭环。
- **非目标**(违反即范围事故):
  - **不做对称加密、不做密钥生成** —— 规范 §11.5 **已否决**,理由撞 D12-006。
    本份**不重开**该裁决。
  - 不做 `MD5` / `SHA-1`(已破,不提供)、不做 SHA-3 / BLAKE3(§11.5 记为
    演进方向,不在本份)。
  - **不做口令哈希的「便捷封装」**(如「一步到位」成员)—— 调用方必须能
    看见并自己组合参数,否则参数就成了不可审计的魔法数。
  - 不做 KMS / 密钥轮换 / 密钥存储(那是宿主职责,不是字符串库能给的)。
- **为什么 KDF 优先于对称加密**:对称加密的否决理由是**没有密钥管理就不该
  给**;而 KDF **没有**这个负担 —— 它输出的就是一段派生字节,存不存、怎么存
  是调用方的事。补上它,wlwl 就有了「口令」这个真实场景的正解。

## 1. 基线(可核验)

```powershell
cd docs
Select-String -Path stdlib/wlwl-stdlib-spec-v0.11.md -Pattern '11.5|加解密|MD5|SHA-1' -Context 0,6
cd ..\impl
Select-String -Path crates/wlwl-eval/tests/encode_contract.rs -Pattern '^\s+Case \{|^\s+EncCase \{' | Measure-Object
```

| 项 | 现值 | 出处 |
|---|---|---|
| `std.encode` 成员 | **8**(`BASE64_*` `HEX_*` `URL_*` `SHA256` `HMAC_SHA256`) | 附录 A |
| 编码失败口径 | 解码失败是 `ERR` 值(`kind = "DecodeError"` + `op` + `reason`);元数 / 类型错是 `E0022` / `E0030` | 规范 §11.1 |
| 哈希边界 | 哈希是**摘要不是加密**;非常数时间口径已写明 | 规范 §11.5 |
| `sha256_10kb` 实测 | ≈ **255 MiB/s**(§7.4 指标 ≥ 100 ✅) | `impl/crates/wlwl-eval/benches/baseline.txt` M2 段 |
| `encode_contract` | **47 条** | 契约表 |
| 已有否决 | 加解密 / 密钥生成**不做**;`MD5` / `SHA-1` 不提供 | 规范 §11.5 |
| 主流对照 | Python `hashlib.pbkdf2_hmac` / `hashlib.scrypt` / `secrets`、Java `SecretKeyFactory`、Go `golang.org/x/crypto`(不在 std) | — |

## 2. 成员面

**层:R2 全员**。

| 成员 | 层 | 语义要点 | 失败口径 |
|---|---|---|---|
| `PBKDF2_ITER(password, salt, iter, len, algo)` | R2 | 标准 PBKDF2。`algo` ∈ `sha256` / `sha512`。**参数全部显式**,无默认值 —— 少一个参数就不该有默认 | `iter < 1` / `len < 1` / 未知 `algo` → `E0030` |
| `ARGON2ID(password, salt, t_cost, m_cost, p_cost, len)` | R2 | RFC 9106。内存硬,抗 GPU/ASIC | 同上;`m_cost` 过大 → `E0030`(要有内存上限,否则一个参数就能耗尽机器) |
| `RANDOM_BYTES(n)` | R2 | **CSPRNG**,操作系统熵源。**不是** `RAND()` —— 名字要让人看出它是密码学安全的 | `n < 0` → `E0030` |
| `RANDOM_HEX(n)` | R2 | 便利层:`RANDOM_BYTES` 的十六进制形态(同 `std.encode` 既有风格) | 同上 |
| `TIMING_SAFE_EQ(a, b)` | R2 | **常数时间**比较,长度不等也走完整比较(不提前返回) | 永不失败 |

> ⚠️ `TIMING_SAFE_EQ` 的**长度不等**处理是设计要点:朴素实现
> `a.len() == b.len() && ...` 会在长度不等时**提前返回**,泄漏长度。
> 规范必须写明「长度不泄漏」,并用一条测试钉住(见 §4)。

## 3. 语义裁决点(开写前必须逐条回答)

1. **G4 选型**(全局):Argon2id 为主 + PBKDF2 兼容,还是只做一个?两个都做
   意味着两份参数语义要各自写进规范,且**参数名不能同名不同义**。
2. **熵源归属**:本份的 `RANDOM_BYTES` 与 08 的 `std.rand` **都要熵** ——
   **必须是同一个源**,否则会出现「一个安全一个不安全」的分裂面。裁决:
   `std.rand` 的播种从 `RANDOM_BYTES` 取,还是从 `std.rand` 反向供 04?
   (建议 08 单向依赖 04。)
3. **参数要不要默认值?** §0.2 立场是「规范明写、不靠隐式」。但口令哈希的
   `iter` / `m_cost` **没有默认值就是没法用**。→ 裁决:给**下界**(安全底线)
   而不是默认值,并在规范里写明「下界不是推荐值」。
4. **`RANDOM_BYTES` 的熵质量**:`RANDOM_BYTES(0)` 返回什么?空串还是错?
   (长度 0 是合法请求,返空串;`n < 0` 才错。)
5. **参数量的上限**:Argon2 的 `m_cost` 没有上限,一个调用就能把机器的内存
   打满。规范要不要给硬上界?→ 建议给,并让越界是 `E0030` 而不是 OOM。
6. **非常数时间口径的归属**:`SHA256` / `HMAC_SHA256` 已有口径(§11.5)。
   `TIMING_SAFE_EQ` 是**比较**不是哈希,口径要另写一句。

### 3.1 W-01 裁决(2026-10-06):参数上下界

**下界取 OWASP Password Storage Cheat Sheet(2025)的最小配置**,多源一致:

| 算法 | 参数 | 下界 | 出处 |
|---|---|---|---|
| Argon2id | `m_cost` | **19,456 KiB**(19 MiB) | OWASP 2025 |
| Argon2id | `t_cost` | **2** | OWASP 2025 |
| Argon2id | `p_cost` | **1** | OWASP 2025 |
| PBKDF2 | `iter`(SHA-256) | **600,000** | OWASP 2025 |
| PBKDF2 | `iter`(SHA-512) | **220,000** | OWASP 2025 |
| PBKDF2 | `len` | **1**(短输出无意义) | RFC 8018 |

**上界(记录者建议,锚点是「OWASP 说抬到单次 0.5–1 s」+「不得让一个参数打满机器」)**
——**越界一律 `E0030`,不静默接受、不 OOM**:

| 参数 | 建议上界 | 怎么定的 |
|---|---|---|
| PBKDF2 `iter` | **10,000,000** | OWASP 底线 600k ≈ 0.3 s ⇒ 10M ≈ 5 s,单次调用有界,而任何合理配置都远低于它 |
| PBKDF2 `len` | **1,024 字节** | 任何派生用途(X25519 密钥 32 B、口令 32 B)都用不到 1 KiB;上界只为防「一个参数吃掉内存」 |
| Argon2id `m_cost` | **1,048,576 KiB(1 GiB)** | 刻意**略低于** RFC 9106 §4 的 first-recommended(2 GiB):上界的职责是防 DoS,而 OWASP 底线 19 MiB 之上已有 **54 倍**余量 |
| Argon2id `t_cost` | **32** | 成本是 `m_cost × t_cost` 的内存流量;1 GiB × 32 已是数十秒级,足够封顶 |
| Argon2id `p_cost` | **16** | `p` 会开线程;16 已高于常规服务器核数,防线程爆炸 |

**为什么下界也要硬(`E0030`)而不只是写进文档**:若不设下界,「参数全显式」就等于
「调用方可以传 `iter=1`」,那这个成员成了**能配出弱哈希的接口**,比不给更危险。

> ⚠️ **一处必须写进规范的连带事实:本语言的 `salt` 只能是文本。**
> `RANDOM_BYTES` 返回**字节**,而 wlwl 的 `STRING` 是 UTF-8 文本,所以
> - `PBKDF2_ITER` / `ARGON2ID` 的 `salt` 实参**只能收 `STRING`**;
> - 因此**盐的自然来源是 `RANDOM_HEX`**(十六进制文本),不是 `RANDOM_BYTES`;
> - 盐长度的下界也只能在**文本域**表达(例如「≥ 16 字符」,对应 128 bit 盐需要
>   32 个 hex 字符)。
> 这与 **D13-001** 同源(RFC 4231 的二进制密钥在文本语言里表达不了,当时改用
> Python 交叉核对)。**别在实现时才发现盐传不进去。**

### 3.2 W-02 裁决(2026-10-06):熵源归属

**`std.encode` 是源,`std.rand` 单向消费它。**

- `RANDOM_BYTES(n)` / `RANDOM_HEX(n)` 直接读操作系统熵源,是**唯一的**「安全随机」入口;
- `std.rand` 的播种从 `RANDOM_BYTES` 取字节(种子本身只是字节,之后由 RNG 展开),
  **不得**反向依赖 04;
- 依据:总纲 §1 已定「`RANDOM_BYTES` 是熵源(读 OS),`std.rand` 消费它做播种,
  **单向依赖**」,且 [`ADR-0026`](../../adr/0026-l1-global-decision-points.md) G3
  已定「显式 seed + 可注入确定性源」—— 单向依赖正是那个形态的必然结果。
- 命名面无冲突:`RANDOM_BYTES` 在 `std.encode`,`std.rand` 的成员名不同 ⇒ 不违反
  ADR-0023 §v0.x 4(禁同名重导出)。

**待 08 落地时确认的一条**:两个命名空间的成员**用法**必须写清
「要密钥材料 → `RANDOM_BYTES`;要可复现的伪随机序列 → `std.rand` 显式播种」,
否则会出现「哪个才是安全的随机」这个本可避免的问题(§6 风险表的关切)。

## 4. 验收门禁

```powershell
cd impl
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --all-targets
cargo deny --locked --all-features check
$env:RUSTDOCFLAGS='-D warnings'; cargo doc --locked --no-deps
cargo check --locked -p wlwl-std --features real-ai
```

契约表扩 `tests/encode_contract.rs`(47 → 约 60),**期望值取自 RFC 原文与
官方测试向量,不是实现输出**:

| 组 | 用例 | 出处 |
|---|---|---|
| PBKDF2 | RFC 6070 的 SHA-1 向例**不可用**(SHA-1 不提供)⇒ 改用 RFC 7914 §11 的 **SHA-256** 向量**逐字冻结** | RFC 7914 |
| Argon2id | RFC 9106 官方测试向量**逐字冻结** | RFC 9106 |
| 交叉核对 | Python `hashlib.pbkdf2_hmac` / `argon2-cffi` 独立跑一遍并注明出处(D13-001 建立的纪律) | Python |
| CSPRNG | 100 000 次抽样**不得重复**;统计检验(卡方粗检) | 自证 |
| **常数时间** | 长度不等时**不得提前返回** —— 钉住「长度不泄漏」;性能上两路径耗时差 < 10% | 机制断言 |
| 回归 | `SHA256` / `HMAC_SHA256` 47 条**逐条不动** | 防顺手改坏 |

**性能断言**:`PBKDF2_ITER` 100 000 轮 ≤ 100 ms(否则调用方会用更小的
`iter`,安全性就崩了);`RANDOM_BYTES(4096)` ≤ 50 µs。

## 5. 九处表落点

九处见 `docs/history/20261002.md` §5.0。**扩已有命名空间**:

| 落点 | 本份动作 |
|---|---|
| 1 规范成员表 | §11.2 补 5 行 + §11.5 补 KDF / CSPRNG 边界 + 参数上界条款 |
| 2 附录 A | 跑 `gen-appendix-a`(`std.encode` 8 → 13 成员) |
| 3 `SPEC.functions` | `encode.rs` 追加 5 个 `StdFn` |
| 4 契约测试硬编码 | `encode_contract` 成员数与点名清单 |
| 5 模块登记 | 无新模块 |
| 6 probe | ≥ 2 条(常数时间 / 参数越界),`EXPECTED_CASE_COUNT` 同步 |
| 7 附录 G | **不动** |
| 8 skill 指针 | `SKILL.md` + `reference.md` §9.1;**必加反模式**:「`SHA256` 是口令哈希」是错的(已有一条,补 KDF 后要指向新成员) |
| 9 CHANGELOG | 成员面 +5;**重申加解密仍否决**(防读者以为本份开了那个口子) |

## 6. 风险与已知代价

- ⚠️ **两个 KDF 的参数语义容易互相污染**:`m_cost` / `t_cost` 的含义、默认值
  建议、内存占用估算,三者要分别写进规范。抄 RFC 原文不够,调用方需要的是
  「我这个配置大概要多少内存」。
- ⚠️ **`RANDOM_BYTES` 一旦引入,就必须和 08 的 `std.rand` 划清界限** ——
  否则会出现「哪个才是安全的随机」这个问题,而答案会是「两个都是,但用法
  不同」。裁决点 §3.2 未定之前不要开写。
- ⚠️ 加解密**仍未做**:这意味着本份之后,wlwl 能安全地处理口令与比较密钥,
  但**不能加密数据**。规范 §11.5 的否决理由要保持可见,别让「有 KDF 了」
  被读成「密码学这块齐了」。
- **明确不做**:对称加密 / 密钥生成 / KDF 便捷封装 / KMS / SHA-3 / BLAKE3 /
  `MD5` / `SHA-1`。

## 7. 检查点

| 子项 | 内容 | 状态 | commit |
|---|---|---|---|
| W-01 | G4 选型裁决 + 参数上界裁决 | ✅ **完成(2026-10-06)** —— 选型由 `ADR-0026` G4 定(Argon2id 为主 + PBKDF2 兼容);**上下界**见 §3.1:下界取 OWASP 2025(Argon2id 19 456 KiB / t=2 / p=1;PBKDF2-SHA256 600k、SHA-512 220k),上界建议 `iter ≤ 10M` / `len ≤ 1 024 B` / `m_cost ≤ 1 GiB` / `t_cost ≤ 32` / `p_cost ≤ 16`,**越界一律 `E0030`**。**附一条连带事实**:本语言 `salt` 只能是文本 ⇒ 盐的自然来源是 `RANDOM_HEX`,盐长下界只能在文本域表达(同 D13-001 的二进制密钥问题) | `5924f65` |
| W-02 | 熵源归属裁决(与 08 共同) | ✅ **完成(2026-10-06)** —— **`std.encode` 是源,`std.rand` 单向消费**:`RANDOM_BYTES`/`RANDOM_HEX` 读 OS 熵源,是唯一「安全随机」入口;`std.rand` 播种从它取,不得反向依赖。依据:总纲 §1 + `ADR-0026` G3(显式 seed + 可注入源 ⇒ 单向依赖是其必然结果)。命名面无冲突(不违反 §v0.x 4) | `5924f65` |
| W-03 | PBKDF2 + Argon2id + 契约(RFC 向量 + Python 交叉核对) | 未动工 | — |
| W-04 | CSPRNG + 常数时间比较 + 契约 | 未动工 | — |
| W-05 | skill / CHANGELOG / 收口 | 未动工 | — |
