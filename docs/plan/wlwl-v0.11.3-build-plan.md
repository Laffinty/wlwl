# WLWL v0.11.3 构建计划 —— 输出安全 + 哈希

| | |
|---|---|
| **分支** | `main`(沿 v0.11.2 起的直接主干工作方式) |
| **基线** | v0.11.2 + 复核 N-6 修复(D12-013,`338dce6`),main@`89cfa14`;成员面 **99**(12 命名空间;`std.ai` 5 成员受 `real-ai` 门控,默认可见 94);probe 152;契约表 collection 125 / str_math 155 / format 39 / encode 39 / text 20 |
| **格式** | **执行契约**(沿 v0.11.2 首版范式,原文见 `docs/history/20261002.md` §1 的 §0):每条陈述自带地址或证伪命令,工作项自带依赖/验收/守卫;状态词表「未动工 / 进行中 / 已完成 / 已否决 / 已终止」全文件统一 |
| **立项** | 业主 2026-10-02/03 裁决:采纳「输出安全(R1 转义 + R2 白名单净化)+ 哈希 + SQL 否决成文 + time/regex 只出 ADR 不出码」组合;命名空间名 `std.html` 被业主否决,改名为 **`std.sanitize`**(agent 提名,理由与落选者见附录 A) |
| **行业依据** | ① OWASP XSS Prevention:防治正解是**按输出上下文转义**,净化只用于「必须接受富 HTML」;② SQL 转义被 OWASP 明确定性为 **last resort**,正解是参数化;③ 富 HTML 净化的现代标准形态是 ammonia/nh3 的**白名单 + 删除**(bleach 已于 2023 弃维);④ Trusted Types / Sanitizer API 是浏览器侧类型化收口范式,需宿主基建,记演进方向(2026-10-02 检索) |

---

## §0 使用契约

沿 v0.11.2 构建计划 §0(已归档)的三条阅读规则、三条写作规则与状态词表,不复述。
补充一条本批特有的:

> **安全条款的措辞纪律**:凡涉及「防 X」的成员描述,必须写清**威胁模型与失效形态**
> (防什么、不防什么、被绕过时表现为什么),不得只写「清洗」或「过滤」。
> 本批最大的风险不是写错代码,而是让调用方**高估**这些成员提供的保证。

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
| `std.sanitize`(**新**) | `HTML_ESCAPE` `HTML_UNESCAPE` | R1 纯 wlwl | 本批 M1 |
| `std.sanitize`(**新**) | `HTML_SANITIZE` | R2(容错解析 + 白名单过滤) | 本批 M3 |
| `std.encode` | `SHA256` `HMAC_SHA256` | R2 | 本批 M2 |

完成后成员面 **99 → 104**,命名空间 **12 → 13**;`std.sanitize` 归属表登记为
**混合(转义 R1 + 净化 R2)**。附录 G 不动(以上名字经查零冲突,2026-10-03 核对)。

### 1.3 继承缺口与去向(不重开裁决)

| 缺口 | 本批去向 |
|---|---|
| GAP-4 时间能力 | **ADR-0024 草案**(W-02),不实现 |
| 正则 | **调研备忘**(W-03 内,RE2 式线性模拟方向),不实现 |
| GAP-2 NFC/NFD | 维持 D12-004 已知限制(等 UCD 裁决) |
| D11-012 建数组平方级 | R0 解释器工作,维持 |
| `std.rand` / `std.iter` | 维持 D12-006 / D12-005 |

---

## §2 上游判定

| 项 | 来源 | 判定与去向 |
|---|---|---|
| N-6 | v0.11.2 复核 | ✅ 已修(`338dce6`,D12-013);本批开工前复测两条严格 key 契约用例仍绿 |
| N-7 | v0.11.2 复核 | IS_SQRT 命名相悖 → **W-04**(规范 §7 加警示句) |
| N-8 | v0.11.2 复核 | skill README impl 标签停 0.11.0 → **W-05**(随 M4 对齐) |
| SQL 转义 | 业主提案 | **否决,成文**(W-01 内 §13.3):OWASP last-resort 定性 + 方言分裂 + 无 `std.db` 参数化收口;复活条件 = `std.db` 的 prepared statement |
| 加解密 | 研究结论 | **否决,成文**(W-08 内):单向哈希可做,对称/非对称加密与密钥生成不做(误用风险 + 密钥需系统随机,撞 D12-006) |

---

## §3 范围契约

### 3.1 目标

1. `std.sanitize`:把「不可信数据进入输出」的**上下文转义**与**富 HTML 白名单净化**
   做成标准库成员,成员描述自带威胁模型与失效形态(§0 措辞纪律)。
2. `std.encode`:补 `SHA256` / `HMAC_SHA256`,期望值**取自 FIPS 180-4 与 RFC 4231
   向量,不是实现输出**(沿 M1 encode 的纪律)。
3. SQL 转义、加解密两个「不做」以规范文本形式**留档**(否决 + 理由 + 复活条件)。
4. `std.time` 出 ADR-0024 草案、regex 出调研备忘 —— 只出文档,不出码。
5. 每个新成员走完「九处表」(v0.11.2 计划 §5.0,已归档),反向守卫齐全。

### 3.2 非目标(违反即视为范围事故)

- **不改语言语义**,不加全局内建(附录 G 逐字节不变)。
- **不做 SQL 转义成员**(见 §2;这是本批的**明示否决**,不是遗漏)。
- **不做加解密**、不做 `MD5` / `SHA1`(已破,不提供)、不提供 SHA-3 / BLAKE3(记演进)。
- **不实现 `std.time` / `std.rand` / `std.iter` / regex**(各自维持既有解锁条件)。
- **不碰 `std.ai` / `std.agent`**,不复活 `std.web`。
- 不修 D11-012(平方级成本,R0 工程)。
- `HTML_UNESCAPE` **不是**通用 HTML 实体解码器(口径见 W-07,防「半张表」)。

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
逐字冻结;probe 用例数只增不减(`EXPECTED_CASE_COUNT` 同步);附录 A 由生成器
重生成且双向锁绿;附录 G 逐字节不变;本文件 §10 检查点全部标记完成。

---

## §4 M0 文档先行(先规范与 ADR,后实现 —— 既有成文顺序)

### W-01 · stdlib 规范新章 §13「输出安全(`std.sanitize`)」+ 演进方向顺延为 §14

- **触点**:`docs/stdlib/wlwl-stdlib-spec-v0.11.md`;§13 演进方向 → §14(全文顺延,
  活文档侧引用同步;归档件不回写)。
- **实现指令**:新章含 —— ① 成员表(§5 的三个成员,失败列按 W-06/W-07/W-11 落定);
  ② **上下文分工口径**:URL 上下文用 `std.encode.URL_ENCODE`,标记上下文用
  `HTML_ESCAPE`,`SANITIZE_HTML` 只用于「业务必须接受富 HTML」—— 三者互不替代;
  ③ **§13.3 为什么没有 SQL 转义成员**(否决记录):OWASP last-resort 定性 + 方言分裂
  (反斜杠 vs 双写引号、`NO_BACKSLASH_ESCAPES`)+ 无 `std.db` 参数化收口 ⇒ 提供
  `SQL_ESCAPE` 只会制造假安全感;复活条件 = `std.db` 落地并以 prepared statement
  形态解决;标识符类需求(动态列名)的正解是白名单映射而非转义;
  ④ §14 演进方向补三条:类型化收口(Trusted Types / Sanitizer API,需宿主基建)、
  通用实体表(UCD 级数据)、CSS / JS 上下文转义(需求出现时按本批形态扩展)。
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

## §5 M1 · `std.sanitize` R1 转义(W-06 / W-07)

### W-06 · `HTML_ESCAPE(s) -> STRING`(R1)

- **语义(裁决,实现不得偏离)**:对 `& < > " '` 五个字符做实体转义,
  映射为 `&amp; &lt; &gt; &quot; &#39;`(`'` 用 `&#39;` 不用 `&apos;` —— 旧渲染器
  兼容口径;其余码点原样保留,含全部 Unicode)。适用于 HTML 文本上下文与
  **双引号属性**上下文(HTML5 同口径);规范明写「属性值必须带引号,未加引号的
  属性值不在本函数契约内」。
- **诊断**:类型错 → 注入 kernel `_DIAG_E0030` 带名(`HTML_ESCAPE: expected string,
  got …`);元数固定 1,由解释器报 `E0022`(无名前缀,与 `str`/`math` 固定元数成员
  同款)。注入面:`_DIAG_E0030` 一条即可,W-02(v0.11.2)的调用点守卫自动覆盖。
- **九处落点**:规范表(W-01)→ 附录 A 生成 → `EXPORT` → `sanitize_contract` →
  probe 目录(+`EXPECTED_CASE_COUNT`)→ skill 指针 → CHANGELOG(M4)。
- **验收(证伪命令)**:`PRINT(HTML_ESCAPE("a<b>&\"'"))` → `a&lt;b&gt;&amp;&quot;&#39;`;
  `PRINT(HTML_ESCAPE(1))` → `!E0030 … HTML_ESCAPE:`;契约表含
  `unescape_escape_roundtrip`(`UNESCAPE(ESCAPE(s)) == s` 对含全部五字符与
  Unicode 混排的 s 成立)。

### W-07 · `HTML_UNESCAPE(s) -> STRING`(R1)—— **半张表裁决点**

- **口径(提议,开工前业主可否决)**:只识别**本库 `HTML_ESCAPE` 会发出的五个
  命名实体**(另接受 `&apos;` 作 `&#39;` 的别名)+ **数字引用** `&#DDDD;` /
  `&#xHHHH;`;**其余命名实体原样保留**(逐字返回,不报错)。规范明写:
  这不是通用 HTML 实体解码器 —— 「半张表」教训(D12-004)在这里的适用形式是
  **roundtrip 定理只对本库输出成立**,消费第三方 HTML 的完整解码属
  `SANITIZE_HTML` 解析器(R2)的内部职责,不由本成员承担。
- **验收**:`PRINT(HTML_UNESCAPE("a&lt;b &amp;&amp;c &copy; &#65; &#x41;"))` →
  `a<b &&c © A A`?**否** —— `&copy;` 不在五实体内 ⇒ 原样 `&copy;`;
  实际期望 `a<b &&c &copy; A A`(逐字钉进契约表,防实现顺手「贴心」扩展);
  `PRINT(HTML_UNESCAPE(HTML_ESCAPE(S)))` 对随机 Unicode 串成立(属性测试,
  契约表内联十条固定样本代替随机)。
- **守卫**:契约表加**负向用例**「`&copy;` 原样保留」—— 防止将来有人把它当
  通用解码器扩展而静默改契约(要扩先改规范)。

### M1 其余落点

- `wl/std/sanitize.wll` 新建(`include_str!` 嵌入;`lang_sources_resolve_and_export_
  upper_snake` / `every_module_file_documents_and_registers_its_own_path` 等既有守卫
  自动覆盖);`ALL_SPECS` 登记;`sanitize_contract.rs` 新建(**从规范 §13 表格解析
  成员面**对拍 + 逐字冻结用例 + 反向守卫,沿三份既有契约的形态);附录 A 由
  `gen-appendix-a` 重生成;probe +2(`M1_std_sanitize_escape_roundtrip` /
  `M1_std_sanitize_unescape_scoping`)。
- **验收**:`cargo test --locked -p wlwl-eval --test sanitize_contract` 全绿;
  `--std-src` 覆盖轨对 `std.sanitize` 生效(输出切换 + 漂移 E0023)。

---

## §6 M2 · `std.encode` 哈希(W-08)

### W-08 · `SHA256(data) -> STRING` / `HMAC_SHA256(key, data) -> STRING`(R2)

- **语义(裁决)**:输入 STRING 按 **UTF-8 字节序列**取摘要(wlwl 的 STRING 不是
  字节缓冲 —— 沿 encode 章已立的口径);输出**小写十六进制**(64 字符)。
  `HMAC_SHA256` 同口径,`key` 亦按 UTF-8 字节。
- **实现**:复用 `wlwl-ast::sha256`(`pub fn sha256_hex` 已在,`wlwl-std` 依赖
  `wlwl-ast` 合法 —— 单一实现两个消费者,杜绝副本漂移);HMAC 按 RFC 2104 手写
  (~30 行,block size 64)。若 `sha256_hex` 的输出形态不符,做最小补面,不改
  `wlwl-ast` 现有调用方(稳定 ID)。
- **契约向量(期望值取自原文,不是实现输出)**:FIPS 180-4 三条标准向量
  (`"abc"` → `ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad`;
  空串;百万 `"a"` 改为 1000 条 `"a"` 拼接的等价短样本,防 CI 超时并注明删减)、
  非 ASCII(`"世界"` 的 UTF-8 摘要)、HMAC 取 **RFC 4231** Test Case 1–3 逐字。
- **否决记录(写进 encode 章)**:不做对称/非对称加密、不做密钥生成(需系统随机,
  撞 D12-006)、不做 MD5 / SHA-1(已破)、SHA-3 / BLAKE3 记演进方向。
- **九处落点**:同 M1 模式;`encode_contract.rs` +N 条;probe +1
  (`M2_std_encode_sha256_hmac`)。
- **验收(证伪命令)**:`PRINT(SHA256("abc"))` → FIPS 向量逐字;
  `PRINT(HMAC_SHA256("key", "The quick brown fox jumps over the lazy dog"))` →
  RFC 4231 TC2 逐字(`f7bc83f430538424b13298e6aa6fb143ef4d59a14946175997479dbc
  2d1a3cd8`);类型错 `!E0030` 带名;`SHA256("")` →
  `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`。

---

## §7 M3 · `HTML_SANITIZE`(R2,旗舰大件)

> **顺序纪律(std.web 教训)**:本里程碑**先补齐成员面归属表与策略 schema 文档,
> 经业主确认后再动第一个实现**。策略 schema 是本批唯一的开放设计面,不允许
> 「实现完了再定契约」。

### W-09 · 策略 schema 文档(先于实现)

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
  嵌套矫正(mXSS 面)与实体在解析期的处理,在 W-09 文档里逐条写明。
- **验收**:W-09 文档存在且策略 DICT 各键的类型与缺省值成表;业主确认记录
  (§10 检查点)。

### W-10 · 容错解析 + 过滤 + 重序列化(R2 实现)

- **触点**:`wlwl-std/src/sanitize.rs`(新)+ `lib.rs` ALL_SPECS + 注入(若需
  带名诊断 kernel)。
- **实现指令**:手写 WHATWG 精神 tokenizer(开启 / 关闭标签 / 自闭合 / 属性 /
  文本 / 注释 / CDATA),零第三方依赖;解析 → 白名单过滤 → 重序列化三段式
  (bleach 架构,ammonia 行为);~600–900 行量级,拆 `tokenize` / `filter` /
  `serialize` 三个 `pub(crate)` 模块文件各配单测。
- **验收**:`sanitize_contract.rs` 含 **OWASP XSS Filter Evasion** 经典向量
  (`<IMG SRC="javascript:alert('XSS')">`、`<SCRIPT>` 系、属性拆行 / 编码变体、
  `<A HREF="jav&#x09;ascript:…">`)**逐条**,期望值 = OWASP 原向量 + 人工裁定
  (裁定依据随用例注明,沿「期望值取自原文」纪律);`javascript:` 属性注入
  在默认策略下必被删除;**往返不承诺**(`SANITIZE_HTML` 的输出不保证再解析
  等价 —— 嵌套矫正的固有属性,规范明写)。
- **守卫**:契约表含反向守卫(默认策略删除 `<script>` 但保留其纯文本子内容?
  —— **否**,script 连内容删,用例钉死这个分歧点);probe +2
  (`M3_std_sanitize_sanitize_html_default` / `M3_std_sanitize_sanitize_html_policy`)。

### M3 其余落点

`sanitize.wll` 门面 `EXPORT(["HTML_ESCAPE", "HTML_UNESCAPE", "HTML_SANITIZE"])`
(命名空间成员面增长);附录 A 重生成;契约硬编码计数同步;probe 计数同步;
skill 指针(M4)。

---

## §8 M4 · 收口(W-14)

- skill 对齐:`reference.md` §9 成员指针 + `HTML_ESCAPE` 用法入反模式表
  (「拼接 HTML 前先转义」);README 标签 → 0.11.3(W-05 收尾)。
- CHANGELOG `v0.11.3` 条目(成员面 104/13;SQL 否决与加解密否决明标)。
- `docs/plan/README.md` 三表;版本号 `0.11.2` → `0.11.3`(workspace version +
  10 条 path 依赖 version)。
- **发 tag 由维护者执行,agent 不代劳**(既有约定)。
- 验收:§3.3 全部门禁 + probe / 附录 A / 附录 G 锁全绿。

---

## §9 偏差登记(D13)

> 沿 v0.11.2 §6 的登记规则:状态必须引用证据;空状态写 `未登记`,不留空;
> `✅ 已处置` = 落地 + 有门禁钉住;`⚠️ 已知限制` = 有意不做且已写明。

| 编号 | 来源 | 内容 | 处置 |
|---|---|---|---|
| (空册,开工即填) | | | |

---

## §10 检查点

> 恢复工作时先读这张表。状态写在文档里,不在 agent 的上下文里。

| 里程碑 | 内容 | 状态 | commit |
|---|---|---|---|
| M0 | W-01 ~ W-05(规范新章 / ADR-0024 / 调研 / N-7 / N-8) | 未动工 | — |
| M1 | `std.sanitize` R1 转义(`HTML_ESCAPE` / `HTML_UNESCAPE`) | 未动工 | — |
| M2 | `std.encode` 哈希(`SHA256` / `HMAC_SHA256`) | 未动工 | — |
| M3 | `HTML_SANITIZE`(策略 schema → R2 实现) | 未动工 | — |
| M4 | 收口(skill / CHANGELOG / 版本号 / tag 归维护者) | 未动工 | — |

> 裁决点备忘(开工前业主可否决,否则视为接受):① W-07 的 `HTML_UNESCAPE`
> 半表口径(五实体 + 数字引用,未知原样保留);② W-09 的默认策略内容;
③ `HMAC_SHA256` 是否随 `SHA256` 同批(计划内:同批,增量 ~30 行)。

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
- **依赖关系**:无前置(不依赖批次 B);工程量级 ≥ 本批 `SANITIZE_HTML`,
  建议独立批次,先出独立调研文档再立项。
