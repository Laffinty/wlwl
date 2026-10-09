# 03 · Unicode 规范化(UCD 数据依赖成员合成一块做)

> **层级** L1(可并行) · **前置** 无(与 02 的 `\p{…}` 裁决共享生成器输入)
> **状态** 🚧 **W-01 / W-02 已结(2026-10-08)** —— W-03(`NFC`/`NFD`/`NFC_QC` 成员)起
> **全局裁决点** G1(Unicode 版本)/ G2(不做半张表)
> → 两者已由 [`ADR-0026`](../adr/0026-l1-global-decision-points.md)(2026-10-06,
> Status **Accepted**,六问全部采纳)定稿:**G1 钉 18.0.0**(2026-09-16 发布;18.0
> 明确「无新算法」故 NFC/NFD 语义零风险,而原建议的 17.0 已落后一版)、
> **G2 改为「要么全量,要么不做」**并**要求本份 W-01 之前先量「全量 UCD 生成」的
> 三个数**(数据体积 / 生成器自检耗时 / 产物体积)—— 不可接受就按 §12.1 老实
> **不做**,而不是缩成半张表。

## 0. 契约摘要

- **目标**:给 wlwl 第一条**规范化**能力,把「字符串相等」在组合字符上的
  静默错误结果消掉。
- **非目标**(违反即范围事故):
  - **不做半张表**。规范 §12.1 原话:「只覆盖 Latin-1 的 NFC 比没有 NFC 更糟
    —— 它会让 `==(NFC(a), NFC(b))` 在部分输入上返回 TRUE,调用方据此建索引,
    然后在真正的 CJK 或组合字符上炸」。
  - **不做 NFKC / NFKD**(兼容性分解会把全角/半角、①/1 折叠掉,是**另一个
    语义**,不能与 NFC 混在一处默认)。
  - 不做大小写折叠(case folding)—— `std.text` 已有 `TO_UPPER` / `TO_LOWER`,
    那是**映射**,折叠是**等价类**判定,不是一回事。
  - 不做 locale 相关的排序 / 比较(那是 02 与 `std.text` 的边界,见 §3.4)。
  - **不引入 Unicode 数据文件进仓** —— 与实体表同款纪律:**数据不入库,
    生成器入库**(见 §3.1)。
- **为什么合成一块做**:v0.11.2 M2 已裁决(规范 §12.1)—— `NFC` / `NFD` /
  `GRAPHEME_COUNT` / `WIDTH` **共用同一捆 UCD 数据**,分批做意味着生成器写
  三遍、审三遍、跟 Unicode 版本对齐三遍。

## 1. 基线(可核验)

```powershell
cd docs
Select-String -Path stdlib/wlwl-stdlib-spec-v0.11.md -Pattern 'UCD|NFC|GRAPHEME|半张表' -Context 0,3
cd ..\impl
Select-String -Path crates/wlwl-std/src/text.rs -Pattern 'fn |ASCII|ascii' | Select-Object -First 8
```

| 项 | 现值 | 出处 |
|---|---|---|
| `std.text` 成员 | **2**(`TO_UPPER` / `TO_LOWER`) | 附录 A |
| Unicode 数据文件 | **0**(两个成员走 Rust `char` 内置表,**零数据文件**) | `std.text` 契约注释 |
| 全局 `UPPER` / `LOWER` | **ASCII-only 且必须保持**(`UPPER("straße")` = `STRAßE`) | 契约表专门钉住 |
| 规范裁决 | 四成员合成一块;**明确不做半张表** | 规范 §12.1 / §14 |
| 主流对照 | JS `String.prototype.normalize`、Python `unicodedata.normalize`、Go `unicode/norm`、Java `java.text.Normalizer` —— **四家都有** | — |

> 这不是「少几个函数」:wlwl 现在**无法比较两个字符串是否规范化等价**,而
> `==(NFC(a), NFC(b))` 是索引、去重、缓存键的标准前置。

## 2. 成员面

**层:R2 全员**(规范化要查表 + 重组,语言层做不了)。

| 成员 | 层 | 语义要点 | 需要的 UCD 数据 |
|---|---|---|---|
| `NFC(s)` | R2 | 规范组合。`"e" + U+0301` → `"é"`(长度**变短**) | `UnicodeData`(canonical decomposition)+ `CompositionExclusions` |
| `NFD(s)` | R2 | 规范分解。`"é"` → `"e" + U+0301`(长度**变长**) | 同上 |
| `NFC_QC(s)` | R2 | 快速检查:**是否已规范化**。无变化时返回 `FALSE` 而不重排 —— 规范化热路径的成本闸门 | `Quick_Check` 属性 |
| `GRAPHEME_COUNT(s)` | R2 | 字素簇数(用户看到一个字符 = 1)。**`"👨‍👩‍👧"` 算 1** | `GraphemeBreakProperty` + `Extended_Pictographic` |
| `WIDTH(s)` | R2 | 显示宽度列数(终端对齐用):东亚全角 = 2,组合记号 = 0,emoji = 2 | `EastAsianWidth` |

**返回值与失败的统一口径**:永不失败(输入任意码点串都产出结果),不返回
`ERR`。元数 / 类型错是 `E0022` / `E0030`。

## 3. 语义裁决点(开写前必须逐条回答)

1. **Unicode 版本**(全局 G1)。实体表取的是 2026-10-03 的 WHATWG
   `entities.json`(它自带的 Unicode 版本与 UCD 的官方版本可能不同)。UCD
   **必须显式钉一个版本**并写进规范头 —— 否则「NFC 到底按哪版算」不可复现。
2. **数据入库形态**(与实体表同款):生成器 `src/bin/gen-ucd.rs` **入库**,
   生成的表文件**不入库**(或入库?)。实体表选了「生成器入库、数据不入库」;
   但 `NFC` 的数据量比实体表大一个量级,每次构建重新生成会拖慢编译 ——
   **需要裁决**。参照 `impl/crates/wlwl-std/src/bin/gen-entities.rs` 的现状。
3. **`NFC_QC` 是新增还是砍掉?** 它不在原四成员清单里。理由:规范化在热路径上,
   「已规范化就跳过」是唯一能把它变成可用的形态。→ 建议加,但要明写它是
   **性能闸门,不是语义糖**。
4. **字素簇 vs 码点的坐标系**:`CHAR_AT` / `INDEX_OF` 走**码点**。若
   `GRAPHEME_COUNT` 走字素簇,那 `std.str` 里就存在**两个坐标系** ——
   必须各自写明,否则调用方会把字素下标喂给码点成员。
5. **全局 `UPPER` / `LOWER` 仍不动**(ASCII-only 是被契约钉死的既有行为,
   替换是 breaking)。规范化**不**改变它们。
6. **与 02 正则的 `\w` / `\p{L}`**:若 02 决定做 Unicode 类别,两张表同源,
   必须共用生成器与同一 Unicode 版本(见 README §3 备注的编号撞车)。

## 3.1 ✅ **[2026-10-08] G2 要求的「三个数」已实测** —— 结论:**全量可行**

`ADR-0026` G2 规定的开工前置:「UCD 数据文件体积、生成器自检跑全量官方测试文件的
耗时、生成产物体积」量一次;「若全量生成不可接受,那就按 §12.1 老实**不做**,而不是缩成
半张表」。三个数都取自 **Unicode 18.0.0** 官方发布(`www.unicode.org/Public/18.0.0/ucd/`,
实拉实量)。

### (1) UCD 源数据体积

| 文件 | 体积 | 谁需要 |
|---|---|---|
| `UnicodeData.txt` | 2.14 MB | `NFC` / `NFD` 的规范分解 |
| `DerivedNormalizationProps.txt` | 1.33 MB | `NFC_QC` + `Full_Composition_Exclusion` |
| `DerivedCombiningClass.txt` | 0.18 MB | 规范排序 |
| `CompositionExclusions.txt` | 0.01 MB | 组合排除 |
| **NFC/NFD 小计** | **3.66 MB** | |
| `emoji-data.txt` | 0.10 MB | `GRAPHEME_COUNT`(`Extended_Pictographic`) |
| `EastAsianWidth.txt` | 0.20 MB | `WIDTH` |
| `GraphemeBreakProperty.txt` | 0.10 MB | `GRAPHEME_COUNT` |
| `NormalizationTest.txt` | 2.73 MB | **仅自检用,不进产物** |
| 九文件合计 | **7.05 MB** | |

⇒ 源数据**不入库**(与 `gen-entities` 同款政策),只需生成器可拉取。

### (2) 生成器自检跑全量官方测试文件的耗时

| 实现 | 用例 | 耗时 | 通过 |
|---|---|---|---|
| **权威**(`unicodedata`,Unicode **15.1.0**) | 20 171 | **28.68 s** | 19 943(98.87%) |
| 本次测量脚本**从零写**的 NFC/NFD | 20 171 | 41.20 s | 6 781(33.62%) |

**那 1.13% 的失败全是版本偏斜,零真失败**:已逐条查过 —— 每一条失败都涉及
CPython 尚未分配的码点(它停在 15.1.0,而测试文件含 16.0–18.0 新增的字符),
判据脚本 `D13-015` 记「疑似真失败 0」。

⚠ Rust 实现会快一个量级(`Vec` + 二分 vs Python 字典),但**全量自检不是「瞬间」
的事** —— 十几秒量级。

#### (2a) ✅ **[2026-10-08 上面那句预测被实测推翻]** —— 不是「十几秒」,是 **0.13 s**

W-02 落地后量了同一个用例集(Unicode **18.0.0** 官方文件全量 **20 171** 行):

| 实现 | 用例 | 耗时 | 通过 |
|---|---|---|---|
| **本实现 Rust**(release,`norm-check`) | 20 171 | **0.13 s** | **20 171(100.00%)** |
| 本实现 Rust(debug) | 20 171 | 0.39 s | 20 171(100.00%) |
| 对照:CPython `unicodedata`(15.1.0) | 20 171 | 28.68 s | 19 943(98.87%) |
| 对照:本次脚本从零写的 Python NFC/NFD | 20 171 | 41.20 s | 6 781(33.62%) |

⇒ 比 CPython 快 **约 220 倍**,不是「快一个量级」。
⚠ **「先估一个量级再实现」这个习惯在本条上又一次赔了**:估少了两个数量级。
代价不只是数字难看 —— 若照这个估计去做「自检要不要进 `cargo test`」的取舍,
会得出「十几秒太长,得缩样本」的错误结论,而真实值 0.13 s 完全放得下。
**耗时这类数,估算在实测出来之前一律不进文档。**

### (2b) ⚠️ **[2026-10-08 生成器落地时的更正]** —— 上一节的两个条数与一个体积**都不对**

写 `gen-ucd.rs` 时实测出**测量脚本自己有两个 bug**,顺带把三个数改了:

1. **`DerivedNormalizationProps.txt` 的字段顺序是 `码点 ; 属性[; 值]`** —— **码点在前**。
   测量脚本把第一段当属性名 ⇒ `Full_Composition_Exclusion` 与 `NFC_QC`
   **两个属性全被跳过**。后果:
   | | §3.1 原记 | 生成器实测 |
   |---|---|---|
   | 规范组合 | 965 | **961**(漏了 4 条排除) |
   | `NFC_QC` 快速路径表 | **0** | **1252** |
   ⇒ **这个 bug 很安静**:生成的表看着正常,只是少了几百条组合、少了一整张表。
2. ⚠️ **体积 186 208 B 这个数是错的,而且错法比原来的 92 KB 更隐蔽。**
   第一次落地用 PowerShell 的 `>` 重定向生成器 stdout,产物落盘是
   **UTF-16LE + CRLF** ⇒ 93 450 B 的文本变成了 186 208 B,恰好**翻倍**。
   症状有三条同时出现:`cargo build` 报 `stream did not contain valid
   UTF-8`、`git diff` 把文件标成 `Bin`、体积正好是真实值的两倍。
   ⇒ **真实产物 93 450 B**(UTF-8 / LF);加 `#[rustfmt::skip]` 后 **93 518 B**
   (四张表各加一行属性),与 `gen-ucd` 的 stdout **逐字节一致**
   (sha256 `667363d0…`,`python` 抓字节复现验过)。
   ⚠️ 原来那句「92 KB」**反而是对的** —— 我上轮据 186 208 B 去"更正"它,
   是把一个**工具假象**当成了更精确的测量。**这是本轮第四次栽在「报了数但
   没连口径一起报」**,而且这次是我**推翻自己**翻的。
   ⇒ 记体积必须连同三件事:**编码、行尾、生成命令**。

### (3) 生成产物(Rust 表)体积

| | 体积 |
|---|---|
| **`norm.rs`(真实生成器产出,ccc 1002 + 规范分解 2081 + 规范组合 961 + NFC_QC 1252)** | **93 518 B(0.09 MB)** |
| 源数据 : 产物 | **39 : 1** |
| 对照:`sanitize/entities.rs`(实体表,已入库) | 69 550 B |

⚠ 口径:**UTF-8 / LF / 带 `#[rustfmt::skip]`**,与 `gen-ucd` stdout 逐字节一致。
上表的 186 208 B 是 UTF-16 假象,见 (2b) 第 2 条。

⇒ 产物**微不足道**,`include_str!` 进二进制没有压力(对比:`std.sanitize` 的实体表比这大)。

### 判定与两条顺带的结论

**三个数全部可接受 ⇒「全量」可行 ⇒ 本份可以开工,且不得缩成半张表。**

⚠ **顺带量出两条比数字本身更有用的东西**:

1. **从零写 NFC 很容易写错。** 本次脚本的 33.62% 失败里,代表性的一类是
   `1E0A 0323` 期望 `1E0C 0307` 而得到 `1E0A 0323` —— **渲染完全相同、码点序不同**
   ⇒ `canonical ordering` 的边界(组合记号与 starter 的相对位置)是错一个字符就
   静默出错的类型,人眼**看不出来**。⇒ 规范必须写死:「`NFC` / `NFD` 的正确性
   **只**由官方 `NormalizationTest.txt` **全量**自检证明,单测抽样**不足以**充当证据」。
2. **版本必须钉死,而且自检要知道它钉的是哪一版。** 上面 1.13% 的偏斜正是版本
   不匹配的可见形态 ⇒ 生成器与运行时表**必须同版**,否则自检会永远差那一截。

**待业主确认**:认可这三个数 ⇒ 本份按全量推进;不认可 ⇒ 按 §12.1 **不做**(不留半张表)。

### 3.2 ✅ **[2026-10-08] 全量自检已落地,并且抓到了一个真 bug** —— W-02 结项

**形态**:`impl/crates/wlwl-std/src/bin/norm-check.rs`(独立二进制,不是
`cargo test`)。理由见 §6:测试文件 2.73 MB **不入库**,CI 上根本不存在,
而 §6 早已预留「可把全量表作为 generator 的**自检**,不进契约表」这条路。

```
cargo run --release -p wlwl-std --bin norm-check -- <UCD 目录>
```

**读数**(Unicode 18.0.0 官方文件全量):

| 项 | 值 |
|---|---|
| 用例行数 | **20 171 / 20 171**(Part0 46 + Part1 17 154 + Part2 2 004 + Part3 194 + Part4 735 + Part5 38) |
| 断言数 | **100 855**(每行 5 条官方 CONFORMANCE 等式 + 1 条快检不变量) |
| 失败 | **0** |
| 耗时 | release **0.13 s** / debug 0.39 s |
| 快检保守侧 | **202 行**(说「不确定」而串本已是 NFC 形态 —— 官方 Part1 的 `Maybe` 单字符行) |

#### 它抓到的 bug:`NFC_QC` 快检少了一个判据 ⇒ **1 164 条静默放行**

第一版快检只查「串里有没有 `NFC_QC ∈ {No, Maybe}` 的码点」。全量自检立刻
报 **1 164 条失败**,而失败样本打印出来**渲染完全相同** —— 正是 §3.1 结论一
预警的那一类。

**根因**:`NFC_QC = Yes` 的定义只说「该码点自身已是 NFC 形态、且不与前一个
starter 组合」,**对码点之间的规范排序只字未提**。反例(U+05B8 QAMATS ccc=18 /
U+05B9 HOLAM ccc=19 / U+05B1 HATAF SEGOL ccc=11):三者 `NFC_QC` **全是 Yes**,
而 `05B8 05B9 05B1` 的 ccc 序列 18,19,11 **已乱序**,`NFC` 之后应当是
`05B1 05B8 05B9`。只查第一条的实现会把原串原样放回 —— **不报错、不崩溃、
渲染一模一样**。

**修法**:快检改成两条判据(缺一不可)——
① 无 `No`/`Maybe` 码点;② 组合类**非递减**(`ccc(prev) > ccc(cur) && ccc(cur) != 0`
即乱序,`ccc == 0` 的 starter 不参与)。

**由此得到的规范口径**:`NFC_QC` 是**保守**判定。文面写「**不保证**是不是
NFC 形态」,**不**写「不是 NFC 形态」—— 官方测试里 202 行反例。
`norm-check` 把这 202 行**打印成数字**而不是静默放过,免得日后有人
把保守方向误当成双向等价。

#### 顺带落定的三件事

1. **算法必须自己算 Hangul。** 实测 `UnicodeData.txt` 里 `AC00` 那行的分解
   字段是**空的**(Hangul 音节走 Unicode Standard §3.12 的**算法**分解),
   ⇒ 2081 条的 `DECOMP` 表**不覆盖** 11 172 个音节。`canon.rs` 里那段
   Hangul 常量不是可选优化,是缺了韩文就静默算错。
2. **W-02 / W-03 的边界按实际前移了。** 原计划把算法放 W-03,但自检必须测
   **发布路径** —— 自带参考实现的自检证明不了任何事(§3.1 结论一里那个
   33.62% 的 Python 版就是这么废的)。⇒ 算法随 W-02 落地在
   `unicode/canon.rs`,W-03 只剩三个成员的转发 + 契约表。
3. **`clippy` 的建议会把版本承诺变成谎言。** `manual_is_multiple_of` 让
   Hangul 那行 `s % T_COUNT == 0` 改成 `s.is_multiple_of(T_COUNT)`,而该 API
   是 **Rust 1.87.0** 才稳定的(官方 std 文档标注),工作区承诺
   `rust-version = "1.85"`(D13-013 实测下限)。**就地 `#[allow]` + 写明
   原因**,不照改 —— 四条 CI workflow 全用 `stable`,抓不到这种回归。

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

契约表 `tests/unicode_contract.rs`(新),**期望值取自 Unicode 官方
NormalizationTest.txt / GraphemeBreakTest.txt / EastAsianWidth.txt**:

| 组 | 用例 | 出处 |
|---|---|---|
| NFC | `NormalizationTest.txt` **全量**跑(canonical 部分),断言「NFC ∘ NFD == NFC」且「NFC 是幂等的」 | Unicode 官方测试文件 |
| NFD | 同上 | 同上 |
| 快检 | `NFC_QC` 与「NFD 后 `NFC_QC` 为真」一致;对未规范化串为假 | 自证 |
| 字素簇 | `GraphemeBreakTest.txt` **全量** | 官方测试文件 |
| 宽度 | `EastAsianWidth.txt` 全量比对 | 官方数据文件 |
| **不半张表** | 断言覆盖面:用官方测试文件里**全部**用例,一条跳过就红 | **这条防「只做 Latin-1」** |
| 回归 | `text_contract.rs` 20 条**逐条不动**(`TO_UPPER` / `TO_LOWER` 与全局 ASCII-only 行为) | 防顺手改坏 |

**性能断言**:1 MB 纯 ASCII 串 `NFC` ≤ 50 ms(快检命中时应为 O(n) 且**不分配**)。

## 5. 九处表落点

九处见 `docs/history/20261002.md` §5.0。**扩已有命名空间**,九处中 8 处:

| 落点 | 本份动作 |
|---|---|
| 1 规范成员表 | §12 补 5 行 + **Unicode 版本条款** + 「不做半张表」条款升级为规范性 |
| 2 附录 A | 跑 `gen-appendix-a`(`std.text` 2 → 7 成员) |
| 3 `SPEC.functions` | `text.rs` 的 `SPEC.functions` 追加 5 个 |
| 4 契约测试硬编码 | `text_contract` 的成员数与点名清单 |
| 5 模块登记 | 无新模块(`text` 已登记) |
| 6 probe | ≥ 2 条(NFC 往返 / 字素簇),`EXPECTED_CASE_COUNT` 同步 |
| 7 附录 G | **不动** |
| 8 skill 指针 | `SKILL.md` + `reference.md` §9.2,并加**反模式**条目:「用 `==` 比字符串而不规范化」是错的 |
| 9 CHANGELOG | 成员面 +5;Unicode 版本写明 |

**特例**:生成器 `src/bin/gen-ucd.rs` 与自检 `src/bin/norm-check.rs` 是
第 10、11 处(v0.11.2 的九处表没列它们,因为那时只加成员不生成数据)。
`src/unicode.rs` + `src/unicode/{norm,canon}.rs` 是第 12 处:它**不是命名空间**
(不持 `SPEC`,不进 `resolve()`),已在 `lib.rs` 的 `NOT_A_NAMESPACE` 豁免名单里
登记 —— 与 `collection.rs` / `test_native.rs` 同款。**数据规模裁决后可能还要
处理构建期成本** —— 已消:产物 93 518 B,直接编译进二进制。

## 6. 风险与已知代价

- ⚠️ **数据体量**:`UnicodeData` + `GraphemeBreakProperty` +
  `Extended_Pictographic` + `EastAsianWidth` + `CompositionExclusions` 合起来
  远大于实体表。入库与否直接决定构建时间(§3.2 未裁决前**不要开写生成器**)。
- ✅ **规范测试文件是权威**:全量跑意味着几万个用例 —— **已裁决(2026-10-08)**:
  测试文件 2.73 MB **不入库**(与实体表 / UCD 表同款纪律),CI 上拿不到,
  所以全量表**不进 `cargo test`**,而是做成独立自检二进制 `bin/norm-check.rs`
  (§6 原先预留的那条路)。代价是它**不在 CI 里自动跑**,靠 §3.2 记的读数与
  生成步骤保证有人跑;收益是 `cargo test` 零负担、构建期零下载。耗时实测
  **0.13 s**,不是负担。
- ⚠️ NFC 会**改变字符串长度** ⇒ 任何拿 `LEN` / `INDEX_OF` 下标做缓存键的
  调用方,规范化后下标全变。这是**正确行为**但会打破调用方假设 ——
  要在 skill 反模式里写明。
- **明确不做**:NFKC / NFKD / case folding / locale 排序 / 半张表 / 把数据
  文件塞进仓而不同步生成器。

## 7. 检查点

| 子项 | 内容 | 状态 | commit |
|---|---|---|---|
| W-01 | Unicode 版本裁决(G1)+ 数据入库形态裁决 | ✅ **完成(2026-10-08)** —— **版本**:Unicode **18.0.0**(G1 已钉)。**入库形态**:先例 `gen-entities.rs`(4.6 KB)+ `sanitize/entities.rs`(69.5 KB)**两者都入库** ⇒ 本份同款:**生成器 + 产物入库,3.66 MB 源数据不入库,构建期零下载**。⚠️ §3.2 原先担心「`NFC` 数据量比实体表**大一个量级**,每次构建重新生成会拖慢编译」—— **实测证伪**:产物 **186 KB** vs 实体表 69.5 KB,**2.7 倍不是 10 倍**,直接 `include!` 进二进制毫无压力 | 待回填 |
| W-02 | 生成器 + 自检(全量官方测试文件) | ✅ **完成(2026-10-08)** —— 生成器 `bin/gen-ucd.rs` 落地(吃本地 UCD 目录、写 stdout、不联网),产出 `src/unicode/norm.rs` **93 518 B**(UTF-8/LF,与 stdout 逐字节一致),四张表 ccc **1002** / 规范分解 **2081** / 规范组合 **961** / NFC_QC **1252**。自检 `bin/norm-check.rs` 跑官方 `NormalizationTest.txt` **全量 20 171 行 × 100 855 条断言,零失败**(release 0.13 s)。⚠️ **落地上抓到/修掉的四个问题**:① 我上轮提交 `bedb8ed` 里混入 0 字节残留 `unicode_tmp.rs`,**把 `every_module_file_documents_and_registers_its_own_path` 守卫打红了**(16 vs 15)—— 那笔提交**没跑过门禁**;② `norm.rs` 被 PowerShell 的 `>` 写成 **UTF-16LE + CRLF**,编译报「stream did not contain valid UTF-8」、git 当二进制、体积虚高一倍(186 208 B ⇒ 真实 **93 450 B**);③ **全量自检抓到一个真 bug**:`NFC_QC` 快检只查「有没有 `No`/`Maybe` 码点」,漏了**规范排序**判据 ⇒ **1 164 条静默放行**(渲染完全相同,肉眼与截图都看不出来),修法与规范口径见 §3.2;④ `clippy` 的 `manual_is_multiple_of` 建议要用 **1.87** 才有的 API,而工作区承诺 1.85 ⇒ 就地豁免并写明原因 | `bedb8ed` → `934f9b5` |
| W-03 | `NFC` / `NFD` / `NFC_QC` + 契约 | 未动工 | — |
| W-04 | `GRAPHEME_COUNT` / `WIDTH` + 契约 | 未动工 | — |
| W-05 | 规范条款升级 / skill 反模式 / 收口 | 未动工 | — |
