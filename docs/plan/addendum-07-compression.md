# 07 · 压缩(zstd / DEFLATE / gzip —— 补上字节侧的最后一块)

> **层级** L1(可并行) · **前置** 无 · **共享裁决** 二进制数据形状(见 06 §3.4)
> **状态** 未动工 · **裁决点** 命名空间归属 + 二进制形状(复用 06)

## 0. 契约摘要

- **目标**:给 wlwl 第一条压缩能力,覆盖**两个当代必需格式**(Zstandard 与
  DEFLATE/gzip),让「产物落盘」与「网络载荷」两个真实场景不再无路可走。
- **非目标**(违反即范围事故):
  - **不做归档格式**(ZIP / TAR / 7z)。归档是「多个条目 + 目录结构 + 元数据」,
    与压缩算法是**两件事**;混在一个命名空间里会让两者的失败口径纠缠。
  - 不做 BZip2 / LZMA / LZ4 / Snappy(可后续按同款体例加,但**不混在本批**)。
  - **不做流式 API**(与 `std.json` 同款立场:本批输入为整串,流式留演进)。
  - **不做「压缩即安全」的任何暗示** —— 压缩不是加密,必须在成员表写明。
- **为什么值得做**:Python 3.14 刚把 Zstandard 收进标准库
  ([PEP 784](https://peps.python.org/pep-0784/),`compression.zstd`),
  Java 有 `java.util.zip`,Go 有 `compress/*` —— 这是**当代主流的基线**,
  不是老派需求。

## 1. 基线(可核验)

```powershell
cd docs
Select-String -Path stdlib/wlwl-stdlib-spec-v0.11.md -Pattern 'std.encode|11.2' -Context 0,3
cd ..
Select-String -Path impl/crates/wlwl-eval/tests/encode_contract.rs -Pattern '^\s+Case \{' | Measure-Object
```

| 项 | 现值 | 出处 |
|---|---|---|
| 压缩成员 | **0** | 附录 A(104 成员) |
| `std.encode` 成员 | 8 | 附录 A |
| 编码的二进制立场 | 「wlwl 的 `STRING` **不是**字节缓冲」,跨语言传二进制要「这边编码、那边解回文本」 | 规范 §11.3 / skill §9.1 |
| `encode_contract` | **47 条** | 契约表 |
| 主流对照 | Python `compression.zstd`(3.14 新增)/ `zlib` / `gzip`、Java `java.util.zip`、Go `compress/*`、Rust(**不在 std**) | — |

## 2. 成员面

**命名空间裁决建议:新建 `std.compress`,不塞进 `std.encode`。**
理由:`std.encode` 现在是「文本编解码 + 摘要」,输入输出都是**文本口径**;
压缩是**字节流**,数据形状与失败口径都不同(见 §3.1)。塞进去会让
`std.encode` 的「成员都是 STRING 进 STRING 出」这条隐含契约破掉。

**层:R2 全员。**

| 成员 | 层 | 语义要点 |
|---|---|---|
| `ZSTD_COMPRESS(data, level?)` | R2 | RFC 8878。默认级别取 Zstandard 的**库默认**,不自行发明 |
| `ZSTD_DECOMPRESS(data)` | R2 | 帧格式校验;**未知帧 / 校验和不符 → `ERR`** |
| `ZLIB_COMPRESS(data, level?)` | R2 | RFC 1950(zlib 包装) |
| `ZLIB_DECOMPRESS(data)` | R2 | |
| `GZIP_COMPRESS(data, level?)` / `GZIP_DECOMPRESS(data)` | R2 | RFC 1952 |
| `DEFLATE_RAW_COMPRESS` / `_DECOMPRESS` | R2 | RFC 1951 裸流,给自定义容器用 |

**失败口径统一**:**数据错**(校验和不符 / 帧非法)→ `ERR` 值,带
`kind = "CompressError"` + `op` + `reason`;**元数 / 类型错** → `E0022` /
`E0030`。

## 3. 语义裁决点(开写前必须逐条回答)

1. **命名空间归属**(§2):新建 `std.compress` vs 扩 `std.encode`。本份建议前者。
2. **二进制数据形状 —— 复用 06 §3.4,本份不重复裁决。** 压缩的输入输出
   **必然**是非 UTF-8 字节,若 06 选了 `ARRAY[INTEGER]`,本份就必须同款;
   若选了 hex 串,本份就是双份 hex 开销。**两份必须同款**,否则 std 会出现
   两种字节表示。
3. **压缩炸弹防护**:解压输出可能远大于输入。**必须有输出上限**,
   越限 → `ERR` 而非 OOM。这条是安全面不是性能面。
4. **级别参数有默认值吗?** `level` 给不给默认?给了要写明「默认不是推荐值」
   (同 04 的 KDF 参数立场)。给了多少?是否与 zstd 库默认一致?
5. **零依赖怎么做到?** zstd 与 DEFLATE 都是**有专利史**的格式(RFC 8878 的
   zstd 专利已于 2019 年过期;DEFLATE 的专利也早已到期)。手写 DEFLATE 的
   **压缩端**尚可,**解压端**要处理动态 Huffman,容易出安全洞。
   → 裁决:手写还是引入小型零依赖 crate(须显式打破「零第三方依赖」纪律)。
6. **压缩不是加密** —— 规范要有一句明写(同 §11.5「哈希不是加密」的体例),
   避免调用方把 gzip 当「藏起来」。

## 3.1 只读调查(2026-10-08):两条硬事实,一条推翻「零依赖」直觉

#### (a) 依赖树里**一个压缩库都没有**

已核 `Cargo.lock`(220 个 crate):`zstd` / `flate2` / `libz` / `miniz` / `brotli` /
`lz4` / `snap` / `crc32` / `crc32fast` / `async-compression` —— **全部不在**。
⇒ 本份是压缩方向的**第一次引入依赖**。

#### (b) ⚠️ 但「引入 C 依赖」**不是先例破坏** —— `ring` 已在树里

已核:`ring`(C + 汇编)经 `rustls` ← `reqwest` ← `std.ai` **已经在依赖树里**。
⇒ 「本工作区是纯 Rust、不能引入 C 依赖」这个前提**本来就不成立**。
`zstd` 的绑定 `zstd-sys` 属同一类,不是新的一类成本。

#### (c) W-02 的真取舍:手写的**总**成本不比引 crate 小,而风险高一个量级

| | 手写 | 引 crate |
|---|---|---|
| **zstd 压缩端** | **不可行** —— FSE / Huffman / 块划分是周级工作量,且正确性极难自证 | `zstd`(成熟实现) |
| DEFLATE / gzip / zlib | 可行(解压 ≈ puff 级别 300 行 + LZ77/Huffman 压缩 ≈ 400 行) | `flate2`(纯 Rust 后端 `miniz_oxide`,零 C) |
| **自研解压器的长期风险** | ⚠️ **压缩炸弹 / OOB / 无界分配** —— 计划 §6 自己写了「解压端要处理动态 Huffman,**容易出安全洞**」 | 上游维护 |
| 逐字向量测试 | 每个格式都要自己攒 RFC + 官方向量 | 仍要写,但对**已验证的实现**写 |

⚠️ 注意计划 §6 写反了一处:它说「手写 DEFLATE 的**压缩端**尚可,**解压端**……容易出
安全洞」。实际上**解压端**(动态 Huffman + 长度/距离码)是最容易出安全洞的一侧,
而**压缩端**(要真出压缩比)也不轻。⇒ **两半都不该自己写。**

一旦为了 zstd 破例,引第二个的**边际成本 ≈ 0**;而手写 DEFLATE 的 700 行 +
逐字向量 + 长期安全维护,**总工作量并不比引 crate 小**。

#### (d) 记录者的建议(待业主拍板)

**引 `zstd` + `flate2`,不手写。** 四条理由:
1. zstd 压缩端手写**不可行**(见 (c));
2. `ring` 已在树里 ⇒ 引入 C 依赖**不是新先例**(见 (b));
3. 自研解压器的安全风险(zip bomb / OOB / 无界分配)是**最不该自担**的一类 ——
   而「压缩炸弹上限」这条规范条款本身就说明这面攻击是真的;
4. 许可证**大概率兼容**:`flate2` = `MIT OR Apache-2.0`(两个都在
   `deny.toml` 白名单);`zstd` = `BSD-3-Clause OR GPL-2.0`(双许可,`BSD-3-Clause`
   在白名单)。⚠️ **这只是静态读白名单的推断,必须加进去跑 `cargo deny` 实测**
   —— 不先声称绿。

**W-01 同时定案**:新建 `std.compress`(**不**扩 `std.encode`)—— 后者是
「文本编解码 + 摘要」,输入输出都是**文本口径**;压缩是**字节流**,塞进去会破掉
`std.encode` 那条「成员都是 STRING 进 STRING 出」的隐含契约。**字节形状继承
`addendum-06` §3.1(b) 的裁决**(`ARRAY[INTEGER]` + 「小载荷」范围条款),
本份**不重复裁决** —— 两份各定一次 = std 里两种字节表示,那是最难还的债。

### 3.2 加依赖后的实测(2026-10-08):许可证**放行**,但撞出一条**既有的** MSRV 谎言

**先说承诺要消掉的那条**:`zstd`(`BSD-3-Clause OR GPL-2.0` 双许可)与 `flate2`
(`MIT OR Apache-2.0`,走纯 Rust 后端 `miniz_oxide`,零 C)**直接通过**
`cargo deny --locked --all-features check`,判定 `advisories ok, bans ok,
licenses ok, sources ok`,**`deny.toml` 一个字没改**。⇒ 调查里那条「静态读白名单
的推断,必须实测」已兑现。

**但实测顺带撞出一条与压缩无关的既有问题**:`[workspace.package]` 声明
`rust-version = "1.75"`,而依赖图里有两个 **`rust-version = "1.85"`** 的 crate:

| crate | MSRV | 位置 | 归属 |
|---|---|---|---|
| `getrandom 0.4.3` | **1.85** | **dev 链**(`insta` → `tempfile` → …) | ⚠️ **既有的** —— 把 `addendum-07` 的 Cargo.toml 改动 stash 掉、从零 `cargo generate-lockfile`,它**仍然在** |
| `jobserver 0.1.35` | **1.85** | **build 链**(`zstd-sys` → `cc` 的 parallel) | ⚠️ **本批带来的** |

**定性方法**(值得记,因为结论反直觉):`impl/Cargo.lock` 是 **gitignored** 的
(`.gitignore:3`,且注释写明是刻意的「不入库」政策)⇒ 没有「HEAD 的锁」可比,
只能**回到 HEAD 重新解一次**。实测:`cc` 两种状态下都是 `1.6.0`,而 `jobserver`
**只在有 zstd 时出现** ⇒ 它来自 `zstd-sys`,不是 `cc`。

⇒ **`rust-version = "1.75"` 这个承诺在本批开工前就已经不成立了**,本批又加了
第二个 1.85 的 **build 依赖**。两者都只落在 build / dev 位置、CI 用 `stable`,
所以**门禁一直是绿的** —— 绿的原因不是承诺成立,而是**没人真的用 1.75 构建过**。

**待业主裁决**(记录者建议见 `build-plan` 的 D13-013):把 `rust-version` 改成
**实测下限**并注明它是 build/dev 依赖带来的,而不是继续留一个假的数字;或者
把承诺改成「CI 工具链」而非「MSRV」。

```powershell
cd impl
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --all-targets
cargo deny --locked --all-features check
$env:RUSTDOCFLAGS='-D warnings'; cargo doc --locked --no-deps
cargo check --locked -p wlwl-std --features real-ai
```

契约表 `tests/compress_contract.rs`(新),**期望值取自 RFC 与官方测试向量**:

| 组 | 用例 | 出处 |
|---|---|---|
| zstd | RFC 8878 附录的**逐字压缩字节**冻结 | RFC 8878 |
| zlib | RFC 1950 §B 的示例字节流**逐字** | RFC 1950 |
| gzip | RFC 1952 §B 的示例字节流**逐字** | RFC 1952 |
| deflate | RFC 1951 §3.2.5 的固定 Huffman 示例 | RFC 1951 |
| 往返 | 各格式的往返,含**空输入**与**高压缩比输入** | 自证 |
| **压缩炸弹** | 构造一个 1 MB 输入 → 极小输出 → 解压出 GB 级;断言 **≤ 上限** 且是 `ERR` 不是 OOM | **安全断言** |
| 损坏输入 | 翻转一个字节 ⇒ `ERR`,**不得 panic** | 逐条 |
| 确定性 | 同输入两次压缩产出**逐字节相同** | 本语言卖点;若实现的 zstd 用了随机化则需说明 |

## 5. 九处表落点

九处见 `docs/history/20261002.md` §5.0。**本份九处全中**(新命名空间):

| 落点 | 本份动作 |
|---|---|
| 1 规范成员表 | 新增 §20 `std.compress` 章 + 8 行 + 「压缩不是加密」条款 + **输出上限条款** |
| 2 附录 A | 跑 `gen-appendix-a` |
| 3 `SPEC.functions` | 新 `src/compress.rs` + 8 个 `StdFn` |
| 4 契约测试硬编码 | `compress_contract` 成员数与点名清单 |
| 5 模块登记 | `ALL_SPECS` 登记 `compress`;`src/` 文件数守卫同步 |
| 6 probe | ≥ 1 条(往返),`EXPECTED_CASE_COUNT` 同步 |
| 7 附录 G | **不动** |
| 8 skill 指针 | `SKILL.md` + `reference.md` §9.12;**反模式**加「gzip 当加密」 |
| 9 CHANGELOG | 成员面 +8;命名空间 +1 |

## 6. 风险与已知代价

- ⚠️ **二进制形状必须与 06 同款**(§3.2)。两份各定一次 = std 里两种字节
  表示,那是最难还的技术债。**裁决归 06,本份只继承。**
- ⚠️ **手写 DEFLATE 解压端的正确性**(动态 Huffman + 长度/距离码)是本份的
  主要工程风险。字节级 RFC 向量测试是唯一可靠防线,不能省。
- ⚠️ **压缩炸弹**是真实攻击面(解压炸弹打满内存/磁盘)。上限必须是
  **规范性条款**,不是实现里的一个 `const`。
- **明确不做**:归档格式 / 流式 / BZip2 / LZMA / LZ4 / Snappy / 「压缩即安全」
  的任何暗示。

## 7. 检查点

| 子项 | 内容 | 状态 | commit |
|---|---|---|---|
| W-01 | 命名空间归属裁决 + **继承 06 的字节形状裁决** | 未动工 | — |
| W-02 | 零依赖形态裁决(手写 vs crate) | 未动工 | — |
| W-03 | zstd(按 RFC 8878 逐字向量) | 未动工 | — |
| W-04 | zlib / gzip / deflate(按 RFC 逐字向量) | 未动工 | — |
| W-05 | 压缩炸弹上限 + 损坏输入契约 | 未动工 | — |
| W-06 | skill / CHANGELOG / 收口 | 未动工 | — |
