# WLWL v0.11.3 构建计划 —— 输出安全 + 哈希

| | |
|---|---|
| **分支** | `main`(沿 v0.11.2 起的直接主干工作方式) |
| **基线** | v0.11.2 + 复核 N-6 修复(D12-013,`338dce6`),main@`89cfa14`;成员面 **99**(12 命名空间;`std.ai` 5 成员受 `real-ai` 门控,默认可见 94);probe 152;契约表 collection 125 / str_math 155 / format 39 / encode 39 / text 20 |
| **格式** | **执行契约**(沿 v0.11.2 首版范式,原文见 `docs/history/20261002.md` §1 的 §0):每条陈述自带地址或证伪命令,工作项自带依赖/验收/守卫;状态词表「未动工 / 进行中 / 已完成 / 已否决 / 已终止」全文件统一 |
| **立项** | 业主 2026-10-02/03 裁决:采纳「输出安全 + 哈希 + SQL 否决成文 + time/regex 只出 ADR 不出码」组合;命名空间名 `std.html` 被业主否决,改名为 **`std.sanitize`**(agent 提名,理由与落选者见附录 A)。**业主 2026-10-03 追加裁决:`std.sanitize` 为本语言的旗舰特色功能,性能是硬指标** —— 本计划 §2.2/§5/§7 据此重写(归层、架构、基准) |
| **行业依据** | ① OWASP XSS Prevention:防治正解是**按输出上下文转义**,净化只用于「必须接受富 HTML」;② SQL 转义被 OWASP 明确定性为 **last resort**,正解是参数化;③ 富 HTML 净化的现代标准形态是 ammonia/nh3 的**白名单 + 删除**(bleach 已于 2023 弃维);④ Trusted Types / Sanitizer API 是浏览器侧类型化收口范式,需宿主基建,记演进方向;⑤ **IEEE S&P 2024「Parse Me, Baby, One More Time」+ Sonar mXSS 研究:净化器的安全不变量是重解析幂等性,tokenizer 型净化器(不建树)已被结构性绕过**(sanitize-html 有公开 mXSS 先例),ammonia/DOMPurify 的抗混淆来自 WHATWG **树构建**;⑥ html5ever 为浏览器级性能(与 C 解析器可比),树构建不构成性能障碍(2026-10-03 检索) |

---

## §0 使用契约

沿 v0.11.2 构建计划 §0(已归档)的三条阅读规则、三条写作规则与状态词表,不复述。
本批特有的两条:

> **安全条款的措辞纪律**:凡涉及「防 X」的成员描述,必须写清**威胁模型与失效形态**
> (防什么、不防什么、被绕过时表现为什么),不得只写「清洗」或「过滤」。
> 本批最大的风险不是写错代码,而是让调用方**高估**这些成员提供的保证。
>
> **性能条款(业主 2026-10-03 追加)**:旗舰特色功能的性能是**契约的一部分**,不是
> 实现细节 —— ① 全员 O(n) 线性,任何成员在语料尺寸增大时**每 MB 成本不得增长**
> (超线性 = 缺陷);② 绝对吞吐目标见 §7.4,未达标须在检查点书面说明原因;③ 每个
> 性能敏感成员配 criterion 基准并记入 `baseline.txt`(沿 M5 先例);④ 归层决策
> **必须**有基准数据支撑(ADR-0021 §0.2 既有规则,本批对 `HTML_*` 全族执行)。

---

## §1 基线指纹

### 1.1 环境

- main@`89cfa14`(v0.11.2 + D12-013);`wlwl -V` = 0.11.2;fmt / clippy / 51 套件
  1917 passed / deny / doc / real-ai check 全绿(2026-10-02 实测)。
- 标准库规范章节:§0–§12 规范性(§11 `std.encode`、§12 `std.text`),§13 演进方向,
  附录 A 镜像(生成器产出)。

### 1.2 目标导出面(本批的改动基线)

| 命名空间 | 成员 | 层 | 状态 |
|---|---|---|---|
| `std.sanitize`(**新**) | `HTML_ESCAPE` `HTML_UNESCAPE` `HTML_SANITIZE` | **R2 全员**(直通 `Value`,沿 `std.encode`/`std.text` 形态) | 本批 M1 / M3 |
| `std.encode` | `SHA256` `HMAC_SHA256` | R2 | 本批 M2 |

完成后成员面 **99 → 104**,命名空间 **12 → 13**;`std.sanitize` 归属表登记为
**R2**。附录 G 不动(以上名字经查零冲突,2026-10-03 核对)。

> **归层裁决(沿 ADR-0021 §0.2 机械规则第 3 条:基准数据证明热点且解释器短期
> 无优化空间时下沉 R2)**:v0.11.2 计划原把转义两成员归 R1 纯 wlwl;**业主性能
> 裁决后推翻**。依据是已归档的基准证据:wll 解释器侧字符串不可变、`+(s, …)`
> 逐次全量拷贝、`PUSH` 数组逐次全量拷贝(D11-012 / D12-009),R1 实现的转义 /
> 解码在文档级输入上**必然超线性** —— 与 `RANGE` 沉 R2(M5,实测 10 000 元素
> 3.6 s、40 000 元素 86 s)同一根因。本批不重走「先 R1 再沉」的弯路,直接 R2;
> 但 W-08 仍要求做一次 **R1 原型对照基准** 把数据坐实进 `baseline.txt`(M5 先例:
> 证据链完整,裁决可审计)。净化器**从未考虑过 R1**(树构建在 wlwl 里既超线性
> 又表达不出,无争议)。

### 1.3 继承缺口与去向(不重开裁决)

| 缺口 | 本批去向 |
|---|---|
| GAP-4 时间能力 | **ADR-0024 草案**(W-02),不实现 |
| 正则 | **调研备忘**(W-03 内,RE2 式线性模拟方向),不实现 |
| GAP-2 NFC/NFD | 维持 D12-004 已知限制(等 UCD 裁决) |
| D11-012 建数组平方级 | R0 解释器工作,维持(**本批成员因全员 R2 而不受此限**) |
| `std.rand` / `std.iter` | 维持 D12-006 / D12-005 |

---

## §2 上游判定

### 2.1 复核与提案判定

| 项 | 来源 | 判定与去向 |
|---|---|---|
| N-6 | v0.11.2 复核 | ✅ 已修(`338dce6`,D12-013);本批开工前复测两条严格 key 契约用例仍绿 |
| N-7 | v0.11.2 复核 | IS_SQRT 命名相悖 → **W-04**(规范 §7 加警示句) |
| N-8 | v0.11.2 复核 | skill README impl 标签停 0.11.0 → **W-05**(随 M4 对齐) |
| SQL 转义 | 业主提案 | **否决,成文**(W-01 内 §13.3):OWASP last-resort 定性 + 方言分裂 + 无 `std.db` 参数化收口;复活条件 = `std.db` 的 prepared statement |
| 加解密 | 研究结论 | **否决,成文**(W-09 内):单向哈希可做,对称/非对称加密与密钥生成不做(误用风险 + 密钥需系统随机,撞 D12-006) |

### 2.2 性能研究结论(2026-10-03 检索,驱动本批架构决策)

1. **tokenizer 型净化器已被学术界定性为结构性可绕过**:IEEE S&P 2024
   「Parse Me, Baby, One More Time」与 Sonar mXSS 研究表明,不建树的净化器
   (sanitize-html / htmlparser2 系)的解析模型与浏览器存在差分,攻击者构造
   「净化时一个树、重解析时另一个树」的输入即可绕过;**安全不变量是幂等性**
   —— `parse(serialize(parse(x)))` 必须是不动点。⇒ 本批净化器**必须树构建**,
   「轻量 tokenizer + 栈」方案就此否决(原计划草案曾列它为选项 A,作废)。
2. **树构建不构成性能障碍**:html5ever 是浏览器级解析器(与 C 实现可比,
   servo 仓库自述),ammonia 的抗混淆能力正来自 WHATWG 树构建;性能差距来自
   实现质量而非范式。⇒ 手写树构建器 + 白名单子集,性能目标见 §7.4。
3. **吞吐参考**:公开渠道无 html5ever / ammonia 的权威 MB/s 数字(基准仓库
   y21/rust-html-parser-benchmark 可作对照),故本批性能指标以**线性承诺 +
   自建基准存档**的形式立约(§0 性能条款 ②),不引用他方数字充数。

---

## §3 范围契约

### 3.1 目标

1. `std.sanitize`:把「不可信数据进入输出」的**上下文转义**与**富 HTML 白名单净化**
   做成标准库成员,成员描述自带威胁模型与失效形态(§0 措辞纪律)。
2. **性能作为契约**(业主追加):全员 R2 直通 `Value`、O(n) 线性、基准存档
   `baseline.txt`、无超线性承诺 —— 见 §0 性能条款与 §7.4 指标。
3. `std.encode`:补 `SHA256` / `HMAC_SHA256`,期望值**取自 FIPS 180-4 与 RFC 4231
   向量,不是实现输出**(沿 M1 encode 的纪律)。
4. SQL 转义、加解密两个「不做」以规范文本形式**留档**(否决 + 理由 + 复活条件)。
5. `std.time` 出 ADR-0024 草案、regex 出调研备忘 —— 只出文档,不出码。
6. 每个新成员走完「九处表」(v0.11.2 计划 §5.0,已归档),反向守卫齐全。

### 3.2 非目标(违反即视为范围事故)

- **不改语言语义**,不加全局内建(附录 G 逐字节不变)。
- **不做 SQL 转义成员**(见 §2;这是本批的**明示否决**,不是遗漏)。
- **不做加解密**、不做 `MD5` / `SHA1`(已破,不提供)、不提供 SHA-3 / BLAKE3(记演进)。
- **不实现 `std.time` / `std.rand` / `std.iter` / regex**(各自维持既有解锁条件)。
- **不承诺与浏览器解析器逐位一致**:树构建器实现 WHATWG 规则的**白名单子集**
  (策略允许的标签才需要完整的 tree-construction 规则),差异显式登记 ——
  安全性由幂等性测试守护,不由「与浏览器相同」守护(§7.3)。
- **不做流式 / 增量 API**(本批输入为整串;流式留演进方向)。
- 不修 D11-012(平方级成本,R0 工程)。
- `HTML_UNESCAPE` 的实体表口径见 W-07(本批为**全表**,经生成器产出)。

### 3.3 完成定义

```
cargo fmt --check                          # 0 diff
cargo clippy --all-targets -- -D warnings  # 0 warning
cargo test --locked --all-targets          # 0 failed
cargo deny check                           # exit 0
$env:RUSTDOCFLAGS='-D warnings'
cargo doc --locked --no-deps               # 0 error
cargo check --locked -p wlwl-std --features real-ai   # exit 0
```

外加:§4–§8 全部工作项验收通过;`sanitize_contract` 与 `encode_contract` 新用例
逐字冻结;**基准存档进 `benches/baseline.txt`(M5 先例),线性承诺经数据核对**;
probe 用例数只增不减(`EXPECTED_CASE_COUNT` 同步);附录 A 由生成器重生成且双向锁
绿;附录 G 逐字节不变;本文件 §10 检查点全部标记完成。

---

## §4 M0 文档先行(先规范与 ADR,后实现 —— 既有成文顺序)

### W-01 · stdlib 规范新章 §13「输出安全(`std.sanitize`)」+ 演进方向顺延为 §14

- **触点**:`docs/stdlib/wlwl-stdlib-spec-v0.11.md`;§13 演进方向 → §14(全文顺延,
  活文档侧引用同步;归档件不回写)。
- **实现指令**:新章含 —— ① 成员表(三个成员,层列 **R2**,失败列按 W-06/W-07/W-11
  落定);② **上下文分工口径**:URL 上下文用 `std.encode.URL_ENCODE`,标记上下文用
  `HTML_ESCAPE`,`SANITIZE_HTML` 只用于「业务必须接受富 HTML」—— 三者互不替代;
  ③ **§13.3 为什么没有 SQL 转义成员**(否决记录):OWASP last-resort 定性 + 方言分裂
  (反斜杠 vs 双写引号、`NO_BACKSLASH_ESCAPES`)+ 无 `std.db` 参数化收口 ⇒ 提供
  `SQL_ESCAPE` 只会制造假安全感;复活条件 = `std.db` 落地并以 prepared statement
  形态解决;标识符类需求(动态列名)的正解是白名单映射而非转义;
  ④ §14 演进方向补三条:类型化收口(Trusted Types / Sanitizer API,需宿主基建)、
  通用实体表已是全表(本批落地,无需演进)、CSS / JS 上下文转义(需求出现时按本批
  形态扩展)。
- **验收**:`grep -c "§13.3" docs/stdlib/wlwl-stdlib-spec-v0.11.md` ≥ 1;
  `grep -n "## 14 演进方向"` 命中;两册冲突时以语言规范为准的头部声明未动。
- **守卫**:无自动守卫(文档);由 M1/M3 的契约测试反向钉住成员表。

### W-02 · ADR-0024 草案:`std.time` 的假时钟气泡(状态 Proposed)

- **触点**:`docs/adr/0024-time-fake-clock-bubble.md`。
- **实现指令**:只写形态与裁决点,不做实现调研之外的事 —— 墙钟 vs 单调钟分离;
  气泡内假时钟(Go 1.24 `testing/synctest` 参照)、气泡外真实时钟;时间成员
  (`NOW` / `MONOTONIC` / `SLEEP`?)在气泡内的可复现语义;与 `SCOPE`/`STEP`
  调度器的关系。开放问题显式列出(气泡边界、真实 IO 的时钟推进)。
- **验收**:ADR 存在且状态为 Proposed;`std.time` 无任何实现代码。

### W-03 · regex 调研备忘(§5.6 同款,不立项)

- **触点**:本文件附录 B;stdlib §14 演进方向一行。
- **实现指令**:RE2 式**线性时间**模拟(NFA 模拟,零依赖手写)与确定性品牌的
  契合点;功能集窄化建议(字符类/锚点/量词,不含回溯引用 —— 回溯引用需要回溯
  引擎,与线性时间互斥);估算工程量级。**结论只有一条:是否值得立项,业主裁。**

### W-04 · `IS_SQRT` 命名警示(N-7)

- **触点**:stdlib §7 的 `IS_SQRT` 行。
- **实现指令**:行内加一句「返回**平方根本身**(向下取整),不是布尔谓词 ——
  与 `IS_OK`/`IS_ERR` 家族的 `IS_` 前缀无关;改名走 ADR-0023 弃用两步,本批不动」。
- **验收**:`grep -n "不是布尔谓词" docs/stdlib/wlwl-stdlib-spec-v0.11.md` 命中。

### W-05 · skill README 的 impl 标签(N-8)

- **触点**:`wlwl-skill/README.md:4`。
- **实现指令**:`(impl 0.11.0; …)` → `(impl 0.11.2; …)`;M4 收口时随版本号更新到
  0.11.3(与 W-14 合并执行,此处先记)。
- **验收**:`grep -n "impl 0.11.0" wlwl-skill/README.md` 0 命中。

**M0 验收**:以上五项全绿;规范两册与 ADR-0024 相互引用完整;**不写任何实现代码**。

---

## §5 M1 · 转义与实体(R2)+ 归层基准(W-06 ~ W-08)

> 三个成员全部 **R2 直通 `Value`**(`test_native.rs` 形态:`StdFn` 直接收发
> `Value::String`,**不经 `wrap()` / compat 层** —— 零转换开销,沿 ADR-0022)。
> 归层证据由 W-08 留档。

### W-06 · `HTML_ESCAPE(data) -> STRING`(R2)

- **语义(裁决,实现不得偏离)**:对 `& < > " '` 五个 ASCII 字符做实体转义,
  映射为 `&amp; &lt; &gt; &quot; &#39;`(`'` 用 `&#39;` 不用 `&apos;` —— 旧渲染器
  兼容口径;**非 ASCII 码点逐字节原样保留**,不做任何 Unicode 处理)。适用于
  HTML 文本上下文与**双引号属性**上下文(HTML5 同口径);规范明写「属性值必须带
  引号,未加引号的属性值不在本函数契约内」。
- **实现要点(性能)**:单趟字节扫描 + `String::with_capacity(输入字节长)` 预分配,
  命中转义字符才写实体 —— 输出分配一次;**无逐字符函数调用开销的查表分支**
  (256 项静态表或 `match`)。类型错 → `E0030` 带名;元数固定 1。
- **验收(证伪命令)**:`PRINT(HTML_ESCAPE("a<b>&\"'"))` → `a&lt;b&gt;&amp;&quot;&#39;`;
  `PRINT(HTML_ESCAPE(1))` → `!E0030 … HTML_ESCAPE:`;契约表含 roundtrip 用例
  (`UNESCAPE(ESCAPE(s)) == s`,样本含五字符 + 多字节 UTF-8 混排)。

### W-07 · `HTML_UNESCAPE(data) -> STRING`(R2)—— **全表实体解码**

- **口径(裁决,取代原计划的「半表」提案)**:R2 形态使全表成本可控,**不做半张表**
  (D12-004 教训在安全语境下加倍成立:第三方 HTML 里 `&copy;` 被原样保留不是
  「 documented 局限」,是调用方拿到的**错误数据**)。落地方式:
  - **命名实体**:HTML5 全表(约 2 231 项,含无分号变体),数据由**生成器**从
    WHATWG `entities.json` 官方数据产出为 Rust 静态表(生成器脚本入库,数据文件
    不入库 —— 沿「数据文件不入库、生成器入库」的 D12 惯例);未知实体**原样保留**
    (HTML5 规范本身的容错行为,非本库私货)。
  - **数字引用**:`&#DDDD;` / `&#xHHHH;`,含 HTML5 的缺分号容错;码点越界 /
    代理区 / 控制字符按 WHATWG 数字引用解析规则映射(替换为 U+FFFD 等),
    **溢出有检查,不得 panic**。
- **实现要点(性能)**:命名实体匹配用**最长匹配 + 排序表二分**(静态排序数组,
  无运行时构建);输出 `with_capacity` 预分配单次分配;禁止递归。
- **验收(证伪命令)**:`PRINT(HTML_UNESCAPE("&lt;b&amp;&copy; &copy;&#65;&#x41;"))` →
  `<b&© ©AA`(`&copy;` 解码为 `©`,与半表提案的「原样保留」相反 —— 该差异就是
  本裁决的证伪点);`HTML_UNESCAPE("&#x110000;")`(越界)→ 按 WHATWG 规则产出
  U+FFFD 且 rc=0;契约表含 mXSS 常用实体混淆样本。

### W-08 · 归层基准:R1 原型 vs R2 对照,数据留档

- **触点**:`impl/crates/wlwl-eval/benches/`(沿 eval_hot_paths 形态)+
  `benches/baseline.txt` 新增 M6 段。
- **实现指令**:写 `HTML_ESCAPE` 的 **R1 纯 wlwl 原型**(仅为取数,不发布:
  用 `--std-src` 覆盖轨加载),与 R2 版在 **1 KB / 10 KB / 100 KB / 1 MB** 四档
  语料上对照;记录每 MB 成本随尺寸的增长曲线。R1 版预期超线性(每 MB 成本
  随尺寸增长),R2 版线性 —— **数据坐实 §1.2 的归层裁决**(ADR-0021 §0.2 要求
  归层变更附基准;M5 `RANGE` 沉 R2 的先例流程)。原型完成取数后**删除**,
  只留 baseline.txt 记录与 commit 注记。
- **验收**:`baseline.txt` 含 M6 段(环境口径沿 §17.7:CPU / rustc / 档位 /
  样本数)与结论;R1 曲线呈现超线性、R2 线性;原型代码不出现在 HEAD。

---

## §6 M2 · `std.encode` 哈希(W-09)

### W-09 · `SHA256(data) -> STRING` / `HMAC_SHA256(key, data) -> STRING`(R2)

- **语义(裁决)**:输入 STRING 按 **UTF-8 字节序列**取摘要(wlwl 的 STRING 不是
  字节缓冲 —— 沿 encode 章已立的口径);输出**小写十六进制**(64 字符)。
  `HMAC_SHA256` 同口径,`key` 亦按 UTF-8 字节。
- **实现**:复用 `wlwl-ast::sha256`(`pub fn sha256_hex` 已在,`wlwl-std` 依赖
  `wlwl-ast` 合法 —— 单一实现两个消费者,杜绝副本漂移);HMAC 按 RFC 2104 手写
  (~30 行,block size 64)。若 `sha256_hex` 的输出形态不符,做最小补面,不改
  `wlwl-ast` 现有调用方(稳定 ID)。**边界直通 `Value`**(不经 compat);实现为
  迭代式 compression(无递归)。
- **性能**:基准 +1(criterion,`sha256_10kb`),目标 ≥ 100 MB/s(手写 SHA-256 的
  常见量级;未达须书面说明);**非目标**:常数时间实现(本成员不承诺防侧信道 ——
  规范明写「不适用于 secrets 比较场景」?**否** —— HMAC 返回值交由调用方比较,
  本库不提供比较成员,规范写明即可)。
- **契约向量(期望值取自原文,不是实现输出)**:FIPS 180-4 三条标准向量
  (`"abc"` → `ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad`;
  空串;长串向量以 1000 条 `"a"` 拼接等价替代百万级,防 CI 超时并注明删减)、
  非 ASCII(`"世界"` 的 UTF-8 摘要)、HMAC 取 **RFC 4231** Test Case 1–3 逐字。
- **否决记录(写进 encode 章)**:不做对称/非对称加密、不做密钥生成(需系统随机,
  撞 D12-006)、不做 MD5 / SHA-1(已破)、SHA-3 / BLAKE3 记演进方向。
- **九处落点**:同 M1 模式;`encode_contract.rs` +N 条;probe +1
  (`M2_std_encode_sha256_hmac`)。
- **验收(证伪命令)**:`PRINT(SHA256("abc"))` → FIPS 向量逐字;
  `PRINT(HMAC_SHA256("key", "The quick brown fox jumps over the lazy dog"))` →
  RFC 4231 TC2 逐字;类型错 `!E0030` 带名;`SHA256("")` →
  `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`。

---

## §7 M3 · `HTML_SANITIZE`(R2,旗舰大件)

> **顺序纪律(std.web 教训)**:本里程碑**先补齐成员面归属表与策略 schema 文档,
> 经业主确认后再动第一个实现**。策略 schema 是本批唯一的开放设计面,不允许
> 「实现完了再定契约」。
>
> **架构裁决(2026-10-03,据 §2.2 研究)**:净化器为**树构建形态**(WHATWG 精神
> 的 tree-construction,范围 = 策略白名单子集),**tokenizer 直通方案已否决** ——
> IEEE S&P 2024 与 Sonar mXSS 研究证明不建树的净化器与浏览器存在解析差分,
> 可被 mXSS 结构性绕过;sanitize-html 的公开前例即此。安全性由 §7.3 的
> **幂等性测试**守护(这是我们与浏览器差异的兜底),不由「与浏览器相同」承诺。

### W-10 · 策略 schema 文档(先于实现)

- **触点**:stdlib §13 的 `SANITIZE_HTML` 行 + 策略小节(M3 内)。
- **策略形态(提议,业主确认后冻结)**:`policy` 为 `DICT`,键:
  - `"tags"`:ARRAY[STRING] —— 允许保留的标签白名单(小写);
  - `"attributes"`:DICT —— 标签名 → 属性白名单;键 `"*"` 表示全局属性;
  - `"url_schemes"`:ARRAY[STRING] —— `href` / `src` 类属性允许的 scheme
    (默认 `["http", "https", "mailto"]`;相对 URL 恒允许;大小写不敏感、
    控制字符剥离后再判 —— 防 `jav&#x09;ascript:` 类绕过);
  - `"strip_comments"`:BOOLEAN,默认 TRUE。
  - `policy` 实参可省,缺省 = 内置保守策略(段落 / 链接 / 强调 / 列表级,
    具体清单在 W-09 文档里逐个列出);policy 形态不符 → `E0030` 带名。
- **语义(裁决,沿 ammonia/nh3 现代口径)**:违规标签**删除**(不是转义);
  `script` / `style` 及其**内容**整体删除;属性按逐标签白名单过滤,URL 属性
  过 scheme 白名单;容错解析(WHATWG 精神:畸形输入不报错,按恢复规则处理)
  ⇒ **本成员永不因输入内容失败**,失败只有类型 / 元数 / policy 形态三类原生诊断。
  嵌套矫正(mXSS 面)与实体在解析期的处理(用 W-07 的全表),在 W-10 文档里
  逐条写明。
- **验收**:W-10 文档存在且策略 DICT 各键的类型与缺省值成表;业主确认记录
  (§10 检查点)。

### W-11 · 树构建解析器(R2 实现)

- **触点**:`wlwl-std/src/sanitize/`(新模块目录:`tokenize.rs` / `tree.rs` /
  `filter.rs` / `serialize.rs` / `entities.rs` 五个 `pub(crate)` 文件各配单测)+
  `lib.rs` ALL_SPECS 登记。
- **实现指令**:两段式 —— ① tokenizer(标签 / 属性 / 文本 / 注释 / CDATA /
  doctype,零第三方依赖,字节扫描 + 预分配);② **树构建**(W-10 白名单子集的
  tree-construction 规则:`<p>` 自动闭合、`<li>` 作用域、`<table>` 系 foster
  parenting 按 W-10 文档登记的子集实现;**构建栈为迭代堆内存,禁止递归**)。
  过滤在**树上**进行(ammonia 形态),随后序列化。
- **性能要点**:输入单次扫描 + 单次树构建 + 单次序列化,三段各自 O(n);
  节点用索引池(`Vec<Node>` + `u32` 句柄,非 `Rc`/`Box` 链)—— 分配一次
  容量、缓存友好;输出串 `with_capacity` 预分配。
- **病态输入契约(安全面,与功能同权)**:深嵌套(≥ 10 万层)不栈溢出(迭代栈)、
  数字实体越界不 panic(W-07)、属性数 / 属性长度不设人为上限但内存 O(n)、
  **任何线性输入不得触发超线性时间**(用 §7.4 基准的 pathological 语料断言)。

### W-12 · 幂等性测试(mXSS 兜底,安全不变量)

- **实现指令**:属性测试与固定样本双轨 —— ① **固定差分集**:对契约语料的每条,
  断言 `SANITIZE_HTML(SANITIZE_HTML(x, p), p) == SANITIZE_HTML(x, p)`(字节级);
  ② **变异集**:对 OWASP 向量做实体 / 大小写 / 拆行变异后同断言;③ mXSS 教科书
  样本(Sonar cheatsheet 的 namespace 混淆族中**作用于白名单子集**的部分)。
  幂等性被打破 = **缺陷**(不是文档说明事项)。
- **验收**:`sanitize_contract` 含幂等性用例组;变异集规模与覆盖在用例注释里
  写明(变异脚本入库)。

### W-13 · 契约:OWASP 规避向量 + 性能断言

- **实现指令**:契约表含 **OWASP XSS Filter Evasion** 经典向量
  (`<IMG SRC="javascript:alert('XSS')">`、`<SCRIPT>` 系、属性拆行 / 编码变体、
  `<A HREF="jav&#x09;ascript:…">`)**逐条**,期望值 = OWASP 原向量 + 人工裁定
  (裁定依据随用例注明,沿「期望值取自原文」纪律);`javascript:` 属性注入
  在默认策略下必被删除;**往返不承诺**(`SANITIZE_HTML` 的输出不保证与输入
  逐位相关 —— 嵌套矫正的固有属性,规范明写)。
- **性能断言(§7.4)**:基准 +3(`sanitize_10kb_article` / `sanitize_1kb_comment` /
  `sanitize_pathological_nesting`),目标见 §7.4;**线性承诺**以
  「1 KB → 100 KB 每字节成本不增长」为断言写入检查点(人工核对 baseline.txt,
  注明数值)。

### §7.4 性能指标(目标值,未达须在检查点书面说明)

| 成员 | 语料 | 目标吞吐 | 依据 |
|---|---|---:|---|
| `HTML_ESCAPE` | 10 KB 文章 | ≥ 200 MB/s | 单趟字节扫描 + 查表,无分配放大 |
| `HTML_UNESCAPE` | 10 KB 文章 | ≥ 100 MB/s | 二分实体表 + 数字解码 |
| `SANITIZE_HTML` | 10 KB 文章 | ≥ 10 MB/s | 树构建 + 过滤 + 序列化三趟;html5ever 同范式为浏览器级,白名单子集远小于全量 HTML5 |
| `SANITIZE_HTML` | 病态深嵌套 | **线性**,不栈溢出 | W-11 病态契约 |
| `SHA256` / `HMAC_SHA256` | 10 KB | ≥ 100 MB/s | 手写 SHA-256 常见量级 |

---

## §8 M4 · 收口(W-14)

- skill 对齐:`reference.md` §9 成员指针 + `HTML_ESCAPE` 用法入反模式表
  (「拼接 HTML 前先转义;净化只用于富 HTML;URL 上下文用 URL_ENCODE」);
  README 标签 → 0.11.3(W-05 收尾)。
- CHANGELOG `v0.11.3` 条目(成员面 104/13;SQL 否决与加解密否决明标;性能条款
  与基准存档明标)。
- `docs/plan/README.md` 三表;版本号 `0.11.2` → `0.11.3`(workspace version +
  10 条 path 依赖 version)。
- **发 tag 由维护者执行,agent 不代劳**(既有约定)。
- 验收:§3.3 全部门禁 + probe / 附录 A / 附录 G 锁全绿 + baseline.txt M6 段齐备。

> **两处计划文本与现实不符,M4 按现实执行并在此登记**(留给下一个读者,别照抄
> 上一段):
> ① 「workspace version + **10 条** path 依赖 version」—— 实测 `[workspace.dependencies]`
>    里正是 **10 条** `wlwl-* = { version = …, path = … }`,加上 `[workspace.package]`
>    的 1 条,**共 11 处**;各 crate 自身用 `version.workspace = true`,不含字面量。
>    `impl/Cargo.toml` 带 UTF-8 BOM,改动须字节级进行(用会重写整文件的命令会
>    顺手改行尾 / 去 BOM,产生无关 diff)。
> ② 「baseline.txt **M6** 段」—— 本批只有 M0–M4,无 M6;该处指 **M3 段**
>    (`HTML_SANITIZE` 基准与线性数据),已写入。

---

## §9 偏差登记(D13)

> 沿 v0.11.2 §6 的登记规则:状态必须引用证据(commit / 文件行 / 门禁结果);
> 空状态一律写 `未登记`,不要留空。状态口径:`✅ 已处置` = 落地 + 有门禁钉住;
> `⚠️ 已知限制` = 有意不做且已写明。

| 编号 | 来源 | 内容 | 处置 |
|---|---|---|---|
| D13-001 | M2 实测 | 计划 W-09 的「RFC 4231 TC1–TC3 逐字冻结」不可达:TC1 / TC3 的密钥是**二进制**(0x0b×20 / 0xaa×20),wlwl 的 `STRING` 是 UTF-8 文本,表达不了;TC7 的提示文本回抄也不可靠(两次手打两次错) | ✅ 已处置 —— TC2(ASCII 键)逐字冻结;UTF-8 与超长键向量以 **Python hashlib 独立交叉核对**并注明出处;TC7 以字节级单元测试(真二进制密钥)落地。`本提交` |
| D13-002 | M2 实测 | `HMAC_SHA256` 成员包装层把 HMAC 摘要**又过了一次 `sha256_hex`**(双重哈希)—— 单元级直接调 `hmac_sha256_bytes` 是对的,包装层错;单元测试与成员测试分居两层,恰好只剩后者能抓住 | ✅ 已处置 —— 摘要手工十六进制编码,encode.rs 补 ⚠ 注记;encode_contract 的 TC2 向量当场抓住并验红转绿。`本提交` |
| D13-003 | M3 W-11 收尾实测 | `tree.rs` 收到 `Token::RawText` 时**不弹栈**:tokenizer 吞掉 `</script>` 本体且不发 `End`,开标签永不出栈,其后内容全挂进这个「必被整删」的元素里 —— `HTML_SANITIZE("<b>x</b><script>evil</script>after")` 返回 `"<b>x</b>"`,`after` 静默消失 | ✅ 已处置 —— `tree.rs` 收到 RawText 即弹栈(tokenizer 保证此刻栈顶就是配对开标签);`sanitize_keeps_siblings_after_a_dropped_raw_text` 三条断言钉住(含多 raw-text 交替)。`本提交` |
| D13-004 | M3 W-11 收尾实测 | `DROP_WITH_CONTENT` **漏了 `noembed`**,而规范 §13.2 明写它是 raw-text 整删集成员 —— 实现与已冻规范相悖,`noembed` 的 raw-text 正文会被当标记解析(mXSS 面) | ✅ 已处置 —— 补入整删集;`drop_set_covers_raw_text_elements`(以 `tokenize::RAWTEXT` 为循环源的守卫)当场抓住;另加 `sanitize_drops_noembed_with_content` 端到端钉住。`本提交` |
| D13-005 | M3 W-11 收尾实测 | `filter_tree` 的挂载点 `target[id]` 判据用错了对象:按**自己**的动作分支,Keep 节点被挂到 `parent` 上;而 Unwrap 父节点自身不输出、那棵子树无人遍历 —— **非白名单元素内的内容整段消失**(`<div onclick="x">y</div>` → `""`、`<table><td>x</td></table>` → `""`)。注释写的是「进父的挂载点」,代码写的是 `parent`,二者矛盾 | ✅ 已处置 —— 判据改为**父**的动作(`actions[parent]`):父 Keep 挂父本身、父 Unwrap 沿链上溯;`sanitize_keeps_content_of_unwrapped_elements` 五条断言钉住。⚠ 中途试过「一律 `target[parent]`」,同样错(Keep 的孩子被踢出元素,`<a>x</a>` → `<a></a>x`)—— 两种错法都表现为「静默丢内容」,不报错。`本提交` |
| D13-006 | M3 W-11 收尾实测 | `serialize` 以 `Item::Enter(tree.root)` 起栈,而 root 的 `NodeKind` 是 `Text("")` 占位 → 匹配 Text 分支只写空串、**从不遍历孩子** —— **`HTML_SANITIZE` 对任何输入都返回空串**,旗舰成员整体不可用 | ✅ 已处置 —— 以 root 的**孩子**为栈起点(root 透明);`serialize.rs` 留注记说明该坑,三条 sanitize 端到端测试各自覆盖一个非空输出。`本提交` |
| D13-007 | M3 W-11 收尾实测 | 承接上四条的**共同根因**:W-11 只交了模块内单测,**端到端零覆盖**,于是上述四道静默缺陷(其中两道使成员完全不产出内容)全部存活且门禁全绿 —— 单测测的是各段自身,没有一条从 `HTML_SANITIZE` 入口走到出口 | ⚠️ 已知限制(**本检查点已解除**):W-12 幂等性套件 + W-13 契约表已落地(79 条冻结用例 / 324 条变异 / 14 条 mXSS 样本 / 病态深度 10 万层断言),「入口到出口」的路已被钉住。**残余**:变异集是**确定性枚举**而非属性测试的随机生成,覆盖受 `mutation_variants` 的 9 类变形所限;要更强的覆盖需引入 proptest 一类依赖(本批非目标)。`本提交` |
| D13-008 | M3 W-13 契约表判读 | `plaintext` 被放进 `DROP_WITH_CONTENT`(整删集),而规范 §13.2 的整删枚举里**没有它**,且明写「`<plaintext>` 后的全文文本化不实现(按普通文本处理)」⇒ 规范要求它走**拆壳留内容**。实现比规范更狠,把用户内容整段丢掉(`<plaintext>hello` → `""`) | ✅ 已处置 —— 按规范移出整删集,改为普通非白名单元素:`<plaintext>hello` → `hello`,`<plaintext><b>x</b>` → `<b>x</b>`,而内层 `script` / `img` / `javascript:` 照常按各自规则处置(拆壳不放宽内层判定);规范 §13.2 同步补写「整删集恰好 8 个 raw-text 元素,`plaintext` 不在其中」,把原本可能被读成「整删」的措辞钉死;契约表 `mxss_plaintext` + wlwl-std 单元测试 `plaintext_is_unwrapped_not_dropped` 各钉一道。`本提交` |
| D13-009 | M3 W-13 契约表判读 | `policy` 传非 `DICT` 时复用了 `data` 的类型错模板,报 `HTML_SANITIZE: expected string, got array` —— 对着一个**本来就合法**的 string 参数说「要 string」,把错误指到了错的参数上(调用方会照着去改 `data`)。`filter::parse_policy` 里其实已有 `policy must be a DICT` 的 `perr`,只是调用点在进 `parse_policy` 之前就把它截住了 | ✅ 已处置 —— `sanitize.rs` 的 policy 分支改用专属诊断 `HTML_SANITIZE: policy must be a DICT, got <kind>`;契约表 `policy_non_dict` 逐字冻结,消息与键名类诊断(`policy "tags" must be an ARRAY`)形态一致。`本提交` |
| D13-010 | M3 W-13 契约表判读 | `HTML_SANITIZE()` 的元数错复用了共享 `arity()` 模板,报「expects **2** argument(s)」—— 而 `policy` 可省、**1 个实参才合法**,等于对着合法调用报错。三个成员共用一个 `want` 固定为 2 的模板,只有本成员是变参 | ✅ 已处置 —— `sanitize.rs` 改用本成员专属消息「expects 1 or 2 argument(s), got N」;契约组 `sanitize_diagnostics_are_frozen` 逐字冻结下界(0)与上界(3)两端。`本提交` |

---

## §10 检查点

> 恢复工作时先读这张表。状态写在文档里,不在 agent 的上下文里 ——
> 上下文会丢,文件不会。

| 里程碑 | 内容 | 状态 | commit |
|---|---|---|---|
| M0 | W-01 ~ W-05(规范新章 / ADR-0024 / 调研 / N-7 / N-8) | ✅ 完成(验收:§13.3 / §14 / 警示句 / 标签残留全部核实;`stdlib_appendix_a_sync` 2/2 绿;零实现代码) | `601e76b` |
| M1 | 转义与实体 R2(`HTML_ESCAPE` / `HTML_UNESCAPE` 全表 / 归层基准 W-08) | ✅ 完成(criterion:escape 442 MiB/s / unescape 115 MiB/s,均达标;W-08 归层对照:R1 每码点 2.7→15.4 µs 超线性、917K 码点档 >15 分钟未完成,R2 全档平坦 —— 数据入 baseline.txt M1 段;契约 22 条 + probe 154) | `613f13e` |
| M2 | `std.encode` 哈希(`SHA256` / `HMAC_SHA256`) | ✅ 完成(FIPS 180-4 / RFC 4231 TC2 逐字 + Python 交叉核对;criterion sha256_10kb ≈ 255 MiB/s,指标 ≥ 100 ✅;D13-001 / D13-002 登记) | `本提交` |
| M3 | `HTML_SANITIZE`(策略 schema → 树构建 → 幂等性 → 契约与基准) | ✅ **W-10 ~ W-13 全部落地** —— W-10 规范 §13.2 策略四键表 + 解析子集差异登记;W-11 五文件(W-11 收尾修掉 D13-003~006 四道静默缺陷);**W-12 幂等性三组**:契约语料逐条 / **324 条变异**(9 类变形,逐类在场守卫)/ 14 条 mXSS 样本,字节级断言;**W-13 契约 + 性能**:**79 条 OWASP 向量逐条冻结并注明裁定依据**、病态契约断言(10 万层不栈溢出 / 1 MB 文本 / 2 万属性 / 10 万实体)、criterion 五条新基准。判读中又抓出三条与规范/契约相悖的实现缺陷(D13-008 `plaintext` 整删、 D13-009 policy 形态错指错参数、D13-010 元数下界报错),**已改实现而非冻结错误输出**。性能:§7.4 三条指标全达标(escape 454.8 / unescape 100.9 / sanitize 17.05 MiB/s);线性承诺成立(每字节 114.0 → 55.9 → 41.6 ns/B 随尺寸**下降**,不增);病态 700 KB / 10 万层 60.87 ms。门禁:`fmt` / `clippy --all-targets -D warnings` / `doc -D warnings` / `deny` / `test --all-targets`(42 套件 1955 绿)/ `conformance` 全绿 | `本提交` |
| M4 | 收口(skill / CHANGELOG / 版本号 / tag 归维护者) | ✅ **完成** —— skill:`reference.md` 新增 **§9.6 `wlwl:std.sanitize`**(开篇即上下文分工表:URL→`URL_ENCODE` / 标记→`HTML_ESCAPE` / 富 HTML→`HTML_SANITIZE`,并写明净化器是**输出点**工具、删标签留文本、`script`/`style`/`iframe` 连内容整删、**幂等但不往返**)、§9.1 补 `SHA256`/`HMAC_SHA256`(**摘要不是加密也不是口令哈希**)、§20 补四条反模式(含「清洗输入就安全了」是错的、双重转义是 bug)、`SKILL.md` 指针、skill 自身 CHANGELOG 补 v0.11.3 批;**顺带修 §9 章首指向 §25 的失效交叉引用**(表实际在 §9.3/§9.5)。CHANGELOG `v0.11.3` 条目(成员面 **99 → 104**、命名空间 **12 → 13** 已对着生成器锁定的附录 A 逐行点过)。版本号 11 处 → `0.11.3`(`impl/Cargo.toml` 带 BOM,字节级改)。probe **155 → 157**。`docs/plan/README.md` 三表回填,并修掉三处陈旧计数(probe 152 → 157、契约表 125/155/39 → 127/156/47、v0.11.2「待发 tag」→ 已发布)。**tag 仍归维护者,agent 未代劳** | `本提交` |

> 裁决点备忘(开工前业主可否决,否则视为接受):
> ① W-10 默认净化策略的具体内容(文档先行,业主确认后才动 M3 实现);
> ② `HMAC_SHA256` 是否随 `SHA256` 同批(计划内:同批,增量 ~30 行);
> ③ W-08 归层对照是否真做 R1 原型(计划内:真做 —— ADR-0021 要求归层附基准,
> M5 先例;数据只有一条才有说服力)。

> ✅ **裁决点 ① 已闭合(业主 2026-10-05)**:W-10 缺省净化策略(22 个排版级标签 /
> `{"a": ["href","title"]}` / 三个 URL scheme / 删注释)**按现状冻结**。背景如实
> 记录:该表由第三方 agent 在**未等确认**的情况下实现(§7 的顺序纪律要求「策略
> schema 经业主确认后再动第一个实现」),M3 的 W-12 / W-13 又是在该表现状之上推进;
> 业主在被两次明确提示该缺口后指示「继续 M4 收口」—— 按收口语境读作**对该表
> 现状接受**,而非要求改表。**日后若要改这张表,连带作废的是 W-13 的 79 条冻结
> 期望值与 probe 的净化器夹具**,不是只改规范一行。
> 裁决点 ②(`HMAC_SHA256` 同批)与 ③(W-08 归层对照真做)按计划内默认执行,无异议。

---

## 附录 A · 命名空间命名裁决记录

**提名:`std.sanitize`**。理由:① 行业伞形术语 —— OWASP / NIST 语境下
sanitization 就是「让不可信数据安全进入特定输出上下文」的统称,转义与白名单净化
都是它的下位概念,命名空间保持伞形、格式前缀放成员名
(`std.sanitize.HTML_ESCAPE`),将来 CSS / JS 上下文转义(`CSS_ESCAPE` / `JS_ESCAPE`)
有处安放;② 与仓库既有命名风格一致(io / fs / json / format / encode / text:
短小、语义、小写);③ 成员名组合可读,无自我重复(`std.sanitize.HTML_SANITIZE`
是唯一轻微冗余处,可接受)。

**落选者**:`std.html`(业主否决 —— 绑定具体格式,非 HTML 上下文无处安放);
`std.markup`(同病,且对该词汇的通识度差);`std.guard` / `std.sec` / `std.safety`
(过泛,暗示密码学或访问控制,承诺了本命名空间不打算给的东西);`std.esc`
(只覆盖转义半边,SANITIZE 进来就名不副实);`std.clean` / `std.waf`
(语义错位 —— 「清洗」暗示输入侧处理,`waf` 是流量侧设备,均与成员实际语义相反)。

## 附录 B · regex 调研备忘(W-03 附件)

- **为什么值得做**:主流 stdlib 全员标配(Go `regexp` / Python `re` / JS 字面量);
  wlwl 现状是零模式匹配能力,校验类需求(邮箱 / URL / 词法模式)无处落脚。
- **形态建议**:RE2 式 **NFA 模拟**而非回溯引擎 —— 线性时间、无指数爆炸、
  可复现(执行步数与输入确定相关),契合本语言确定性品牌;零第三方依赖手写
  可行(Thompson 构造 ~500–800 行量级)。
- **功能集必须窄**:字符类 / 锚点 / 量词 / 分组(非捕获)/ 交替;**不含回溯引用**
  `(?P=…)`(需回溯引擎,与线性时间互斥)、**不含 lookahead / lookbehind**
  (同因);Unicode 码点级语义(不做字素簇,沿 std.str 坐标系)。
- **依赖关系**:无前置(不依赖批次 B);工程量级 ≥ 本批 `HTML_SANITIZE`,
  建议独立批次,先出独立调研文档再立项。
