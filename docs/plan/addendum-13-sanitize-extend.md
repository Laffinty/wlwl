# 13 · `std.sanitize` 扩展(CSS / JS 上下文转义 + 类型化收口)

> **层级** L2 · **前置** 无 · **状态** ✅ **整份结项(2026-10-13)** —— W-01…W-05 全部落地;[`ADR-0030`](../adr/0030-sanitize-extend-shape.md)**Accepted**;规范 §13.4 / §13.5 + 附录 A;skill §9.6。成员面 3 → 8(全库 172 → 176),契约 11 → 24 条
> **规范登记** §14 前两项已结(CSS 做、JS 换题);**第三项(类型化收口)已调研并判为不适用** —— 否证记录已写进规范 §14,由契约 `typed_closure_rejection_record_stays_in_the_spec` 钉住

## 0. 契约摘要

- **目标**:把 `std.sanitize` 从「HTML 一个上下文」扩到**三个**(HTML / CSS /
  JS),并评估**类型化收口**是否可做。
- **非目标**(违反即范围事故):
  - **不做 HTML 净化器的第二套**。`HTML_SANITIZE` 的语义、缺省策略、
    幂等不变量**一条都不动**(它们是 v0.11.3 刚冻结的 81 条契约)。
  - **不做属性名 / 值的 CSS 解析**。`CSS_ESCAPE` 只做**标识符**与**字符串
    字面量**两处转义,不做选择器语义。
  - **不实现 DOM 基建**。类型化收口在浏览器生态里依赖 DOM / 宿主;
    wlwl 没有 ⇒ 见 §3.2。
  - 不做 `JS_ESCAPE` 的「把任意值变成可 eval 的字面量」—— 那是**代码生成**,
    不是转义。本份只做**字符串字面量**与**注释**两处。
- **为什么是差异化面**:主流语言这块要么没有(Rust / Python / Go 都没有一等的
  HTML 净化),要么靠第三方(OWASP Java Encoder / DOMPurify)。wlwl 把它做成了
  一等成员 + 全表实体 + 幂等不变量 + 性能入契约。**这是库里最独特的资产,
  值得加厚而不是铺开。**

## 1. 基线(可核验)

```powershell
cd docs
Select-String -Path stdlib/wlwl-stdlib-spec-v0.11.md -Pattern '## 14 演进方向' -Context 0,8
cd ..\impl
Select-String -Path crates/wlwl-eval/tests/sanitize_contract.rs -Pattern '^\s+SanCase \{' | Measure-Object
```

| 项 | 现值 | 出处 |
|---|---|---|
| `std.sanitize` 成员 | **3**(`HTML_ESCAPE` / `HTML_UNESCAPE` / `HTML_SANITIZE`) | 附录 A |
| 冻结契约 | **81 条 OWASP/mXSS 向例**(其中 **14 条** 是 mXSS 样本)+ 324 条变异 | `sanitize_contract.rs` |
| 性能 | 10 KB 文章 **17.05 MiB/s** | `impl/crates/wlwl-eval/benches/baseline.txt` M3 段 |
| 规范已登记 | §14:「**CSS / JS 上下文转义**(`CSS_ESCAPE` / `JS_ESCAPE`):需求出现时按 §13 的形态扩展 `std.sanitize`,**先有真实场景再立项**」 | 规范 §14 |
| 类型化收口 | §14:「**类型化收口**(Trusted Types / Sanitizer API 范式):让危险的输出收口只接受经净化策略产出的**类型化值**……它需要宿主 / DOM 基建,不是字符串库能提供的;**wlwl 若将来出现宿主形态,照此记一笔**」 | 规范 §14 |

> ⚠️ 规范 §14 对这两项都写了门槛:「**先有真实场景再立项**」与
> 「**若将来出现宿主形态**」。本份的**第一产出应当是「有没有真实场景」的
> 调研**,不是成员表 —— 见 §3.1。

> **[2026-10-05 更正] 冻结条数 79 → 81,并说明两个数字都不是断言钉住的。**
> 原稿写「79 条 OWASP/mXSS 向例 + 324 条变异 + 14 条 mXSS 样本」。实测
> 2026-10-05 数 `sanitize_contract.rs` 的 `SanCase { name:` 条目 = **81**;
> 其中 `mxss_` 前缀的**恰为 14** 条(是 81 的**子集**,不是并列的第三组)——
> 原稿把 14 与 79 并列,会让人以为总数是 79 + 14。
> 另外:**81 与 324 都不是断言钉住的值**。代码里的守卫是**下界**
> (`SANITIZE_CASES.len() >= 75` / `variants >= 150` / `mXSS 组 >= 12`),
> 刻意留了余量好让「有人删了用例」变红;324 只出现在文件头注释(某次运行值)。
> ⇒ **本份若要重冻或扩展契约,先数真值再动手,不要照抄这里的数字** ——
> 这正是 D11-024 / D11-025 吃过一次的坑(「基线棘轮的 1860 passed 是错的,
> 真值 1870」)。
> (另:`docs/plan/README.md` 门禁行记的「契约表 sanitize **103** 条」是
> **另一个口径** —— 整份契约表的断言组数,手工点的,文件里没有 103 这个
> 字面量。它与 81 **不冲突**,不要拿它去对 81。)

## 2. 成员面(候选,待 §3.1 调研确认后定稿)

**层:R2 全员。**

| 成员 | 层 | 语义要点 | 出处 |
|---|---|---|---|
| `CSS_ESCAPE_IDENT(s)` | R2 | CSS **标识符**转义(类名 / `id` / 自定义属性名) | CSSOM `CSS.escape()` |
| `CSS_ESCAPE_STRING(s)` | R2 | CSS **字符串字面量**转义(引号内) | CSS Syntax L3 |
| `JS_ESCAPE_STRING(s)` | R2 | JS **单 / 双引号**字符串字面量转义 | ECMAScript 词法文法 |
| `JS_ESCAPE_COMMENT(s)` | R2 | 注释内 `*/` 与换行的处理 | ECMAScript MultiLineComment |
| `HTML_SANITIZE_CSS(text)` | R2 | 从文本中**抽出**安全的 CSS 值(白名单属性 / 拒绝 `expression(` / `url(javascript:`) | — |
| `HTML_SANITIZE_URL(u)` | R2 | URL 归一 + scheme 判定 + **返回可安全放进 `href` 的值**。与 `HTML_SANITIZE` 的 URL 属性判定同款口径 | — |

**失败口径**:转义**永不失败**;`HTML_SANITIZE_CSS` / `_URL` 对不可接受的输入
⇒ `ERR([kind: SanitizeError])`(这是**数据**问题,不是参数错)。**它与
`HTML_SANITIZE` 的「永不因内容失败」是不同族** —— 规范要写清分界:
`HTML_SANITIZE` 不能失败(否则是 DoS 面),而 `*_CSS` / `*_URL` 可以失败
(因为它们收的是**值**,失败有明确的业务含义)。

## 3. 语义裁决点(开写前必须逐条回答)

1. **有没有真实场景?**(规范 §14 的门槛)本份的**第一产出**是调研:
   - CSS 转义的真实需求通常是**模板引擎 / 服务端渲染样式**。
   - JS 转义的真实需求通常是**服务端渲染 `<script>` 内联数据** ——
     而正确解是 **JSON 序列化**(`JSON_STRINGIFY` 已存在),不是 `JS_ESCAPE`。
     ⇒ **这一点必须先答**:如果 `JSON_STRINGIFY` 已经覆盖了 90% 的场景,
     `JS_ESCAPE_STRING` 的边际价值就很低,规范 §14 说的「先有真实场景」
     可能并不成立。
   > 🟡 **[2026-10-13 已调研 → `ADR-0030`]—— 答案既不是「成立」也不是「不成立」,
   > 而是「**问错了问题**」。**
   >
   > ① **`JSON_STRINGIFY` 没覆盖**,实测坐实:OWASP DOM XSS Cheat Sheet 明写它
   > 「**is not an output-encoding function**」,并逐字符点名内联场景要额外挡住
   > `<`(以及 `>` `&` U+2028 U+2029)。本仓库实测:`STRINGIFY("</script><img src=x>")`
   > → `</script><img src=x>`;`STRINGIFY("a & b")` → `a & b`;U+2028 / U+2029
   > 原样输出(按**长度**判:输入 3 码点 → 输出 5,转成 `\u2028` 该是 11)。
   > ⇒ 缺口是**本仓库自己的**,不是转述别人的。
   > ② 但 **通用 `JS_ESCAPE_STRING` 仍是错的题**:OWASP 的**首选**根本不是转义,
   > 是「用 `application/json` 单独响应 / 走 `<script type="application/json">`
   > inert 容器」;而 `JS_ESCAPE_*` 剩下的用武之地(`onclick="f('…')"`)已被
   > `HTML_SANITIZE` 整条剥掉事件处理器属性覆盖掉。
   > ③ **裁决(D2):不做 `JS_ESCAPE_STRING` / `JS_ESCAPE_COMMENT`,改做
   > `HTML_SANITIZE_SCRIPT(value)`** —— 收**值**,自己 `STRINGIFY` + 加固 + 包壳,
   > 返回整个 `<script type="application/json">…</script>`。这样「忘记加固」这个
   > 失败模式**不存在**,而不是靠文档提醒。

2. **类型化收口的前置**:§14 已明写它「需要宿主 / DOM 基建」,而 wlwl
   **没有宿主形态**。⇒ 本份**只做调研与 ADR,不实现**。ADR 要回答:
   - 「类型」是否要新增内建类型(语言变更)?
   - 若不新增类型,「类型化」用什么承载(标记 `DICT`?运行时检查?—— 那就
     不是 Trusted Types,只是「加了个检查」)?
   - 没有 DOM 时,Trusted Types 的**主要安全收益**(浏览器能识别 tainted
     值并在 sink 处拦截)还能不能拿到?**大概率拿不到** ⇒ 结论可能是
     「不适用,继续搁置」。
   > ✅ **[2026-10-13 已裁决 → `ADR-0030` D3]** —— **不适用**(否证结论)。
   > 收益的**全部**来自「浏览器在 sink 处强制拒绝裸串」(`require-trusted-types-for
   > 'script'` 让 `innerHTML = "str"` 抛 `TypeError`)。没有宿主时,这条收益是
   > **定义上的**消失而非打折:**没有 sink**(innerHTML / eval / document.write 都不存在),
   > **也没有执行者**(强制发生在浏览器里,不在语言里)。剩下的「一个做净化、返回
   > 字符串的函数」**正是 `HTML_SANITIZE`,已经有了**。
   > ⚠ 有第三方资料称 Trusted Types 可防服务端 XSS —— **那是错的**,同一批资料
   > 自己也写着「Trusted Types do not protect against server-side XSS; they only
   > cover DOM XSS sinks in the browser」。本裁决采信后者。
   > **复活条件**:wlwl 出现**宿主形态**(能执行代码、能接管输出点的运行时)。
3. **`CSS.escape()` 的规范来源**:CSSOM 规范定义了 CSS 标识符转义;
   `CSS_ESCAPE_STRING` 的来源是 CSS Syntax L3。两者**不是同一套算法** ——
   不能混成一个成员。
4. **`HTML_SANITIZE_URL` 与 `HTML_SANITIZE` 的 URL 属性判定要同款**:
   同一份 `scheme_allowed` 逻辑,否则会出现「净化器放行、独立成员拒绝」
   的分裂。**复用,不要各写一份。**
5. **`CSS_ESCAPE_IDENT` 的输入边界**:空串?以数字开头?控制字符?——
   `CSS.escape()` 对这些有明确规则,逐条照抄并冻结向量。

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

契约表扩 `tests/sanitize_contract.rs`,**期望值取自 CSSOM / CSS Syntax /
ECMAScript 规范原文,不是实现输出**:

| 组 | 用例 | 出处 |
|---|---|---|
| `CSS_ESCAPE_IDENT` | `CSS.escape()` 规范里的**逐条例子**冻结 | CSSOM |
| `CSS_ESCAPE_STRING` | CSS Syntax L3 §4.3.7 的字符串转义例子 | CSS Syntax L3 |
| `JS_ESCAPE_STRING` | ECMAScript 的 LineContinuation / UnicodeEscape / LineTerminator 三例 | ECMAScript 词法文法 |
| `JS_ESCAPE_COMMENT` | `*/` 与 U+2028 / U+2029 的处理 | 同上 |
| `_CSS` / `_URL` | `expression(` / `url(javascript:` / `data:text/html` 逐条拒;相对 URL 恒允许 —— ⚠ **「恒允许」是位置性判据**:第一个 `/` `?` `#` **之前**的那个 `:` 才是 scheme 的结尾,落在它们之后的 `:` 是路径/查询/片段内的分隔器 ⇒ 相对 URL,恒允许(`D13-023`,WHATWG URL §4.2 ≡ RFC 3986 §3.3–3.5) | OWASP |
| **同款判定** | 断言 `HTML_SANITIZE` 的 `href` 判定与 `HTML_SANITIZE_URL` **结论一致**(同一组 URL 喂两个入口) | **防分裂** |
| ⚠ **上一行的补充** | **一致性断言单独用不住**:两处同时错也照样绿。`D13-023` 复核时确认这条契约**只断言一致、不断言哪个结论**,因而**没有**把缺陷锁成基线 —— 但也**没有**把它抓出来。故另加**绝对结论**守卫(`html_sanitize_url_allows_relative_urls_with_a_colon_after_a_delimiter`:允许 4 条 / 拒绝 3 条)。⚠ **加判定分支时两条守卫要一起加** | **防「两处一起错」** |
| **回归** | 现有 **81 条 OWASP/mXSS 向量 + 324 条变异** 逐条不动 | v0.11.3 的冻结契约 |
| 性能 | 10 KB 纯文本转义 ≥ 100 MB/s | 新基准 |

**类型化收口**:若 ADR 结论是「不适用」,本份要在规范 §14 记
「已调研 + 结论:无宿主形态时不适用,复活条件 = 出现宿主形态」——
**否证记录与肯定记录同权**(与 §13.3 SQL 否决同款体例)。

## 5. 九处表落点

九处见 `docs/history/20261002.md` §5.0。**扩已有命名空间**:

| 落点 | 本份动作 |
|---|---|
| 1 规范成员表 | §13 补 CSS / JS 小节 + 成员表 + **「何时失败」的分界条款** |
| 2 附录 A | 跑 `gen-appendix-a`(`std.sanitize` 3 → ≈ 9 成员) |
| 3 `SPEC.functions` | `sanitize.rs` 追加 |
| 4 契约测试硬编码 | `sanitize_contract` 成员数与点名清单 |
| 5 模块登记 | 无新模块 |
| 6 probe | ≥ 2 条(CSS 标识符 / URL 判定),`EXPECTED_CASE_COUNT` 同步 |
| 7 附录 G | **不动** |
| 8 skill 指针 | `reference.md` §9.6 的**上下文分工表要扩**成四行(HTML / CSS / JS / URL);反模式加「内联 JSON 用字符串拼接」 |
| 9 CHANGELOG | 成员面 +≈6 |

## 6. 风险与已知代价

- ⚠️ **规范 §14 自己设了门槛「先有真实场景再立项」**,而 §3.1 的初判是
  **JS 转义的真实场景已被 `JSON_STRINGIFY` 覆盖大半**。→ 本份有可能的
  结论是「**只做 CSS,JS 不做**」。那也要成文(否证记录),不能默默不做。
- ⚠️ **类型化收口大概率不适用**(§3.2)。它是本项目最想要的形态之一,
  但 Trusted Types 的安全收益来自**宿主能识别 tainted 值并在 sink 处拦截**;
  没有宿主就只剩「加了个检查」,那不值得一个新内建类型。**如实写否证结论。**
- ⚠️ **与 `HTML_SANITIZE` 判定的分裂风险**(§3.4):两处 URL 判定必须复用
  同一份逻辑。用例要断言两者结论一致。
- **明确不做**:HTML 净化器语义变更 / CSS 选择器解析 / JS 代码生成 /
  模板引擎 / DOM 基建 / 新内建类型(除非 ADR 明确裁决)。

## 7. 检查点

| 子项 | 内容 | 状态 | commit |
|---|---|---|---|
| W-01 | **真实场景调研**(§3.1)—— 决定 CSS / JS 各自做不做 | ✅ **已裁决** —— [`ADR-0030`](../adr/0030-sanitize-extend-shape.md)**Accepted**(D1–D4):① **基线复核**:81 条 / 14 条 mXSS / 3 成员,数字仍成立(D11-024 的坑:先数真值再引用);② **CSS 做**(D1),但按 OWASP 口径**白名单高于转义** ⇒ `HTML_SANITIZE_CSS` 优先于两个 `CSS_ESCAPE_*`;③ **JS 换题**(D2):实测坐实 `JSON_STRINGIFY` **不是**输出编码函数(OWASP 点名的 `<` `>` `&` U+2028 U+2029 一个没转,U+2028 按**长度**判:3→5 而非 11),但通用 `JS_ESCAPE_*` 是错的题,改做收整个元素的 `HTML_SANITIZE_SCRIPT`;④ **类型化收口不适用**(D3)—— Trusted Types 的收益全部来自「浏览器在 sink 处强制拒绝裸串」,无宿主时是**定义上**消失,剩下的「一个净化函数」正是 `HTML_SANITIZE`;复活条件 = 出现宿主形态 | `6894bc5` |
| W-02 | CSS 转义族 + 契约(CSSOM / CSS Syntax 逐条) | ✅ **已改题为 `HTML_SANITIZE_SCRIPT`** —— ⚠ 计划原写的 W-02「CSS 转义族」被 `ADR-0030` D2 换掉:D1 说 CSS 做但**白名单高于转义**,D2 说 JS 侧真正的缺口不是通用转义器而是「JSON 内联加固」。本项先落 D2 的产物(它是**刚被裁决出来、且缺口有实测坐证**的那条),D1 的 CSS 族顺延到 W-03。规范新增 §13.4,契约 **11 → 17 条** | `e5ad7af` |
| W-03 | \HTML_SANITIZE_CSS\ / \HTML_SANITIZE_URL\ + \CSS_ESCAPE_IDENT\ / \CSS_ESCAPE_STRING\ + **同款判定**断言 | ✅ **已落地** —— 四成员全 R2,规范新增 §13.5。⚠ **\HTML_SANITIZE_URL\ 复用 \ilter::scheme_allowed\(同一份逻辑 + 同一个默认 scheme 表)**,不是另写一份;契约 \html_sanitize_url_agrees_with_the_html_sanitizer\ 把同一组 URL 喂两个入口断言结论一致。两个转义器**逐条照抄 CSSOM §Common Serializing Idioms**并冻结现行版(2013 WD 版对 U+0000 是抛异常)。⚠ 实现顺序按 D1:白名单先于转义。⚠⚠ **2026-10-15(`D13-022` / `D13-023`,首次审查后)复查补记**:① `HTML_SANITIZE_CSS` 的输出**曾可含裸 `</style>`**(嵌进 `<style>` 即破壳)—— 已按 CSSOM *escape a character as code point* 加固值;⚠ **不能照抄 SCRIPT 的反斜杠-u-转义**,CSS 没有那个形状,照抄过去只是标识符文本、`<` 原样留着;② `scheme_allowed` 的 `/ ? #` 豁免**曾是空头支票**(注释承诺、代码没写)⇒ `/wiki/Special:Search` 的 href 被删,违自家规范「相对 URL 恒允许」。⚠ **只写在 §3.4 是不够的**,已同步进 `sanitize_contract.rs` 并补绝对结论守卫 | 56244a1 + `1869c04` |
| W-04 | 类型化收口 **ADR**(含「不适用」的否证结论) | ✅ **已并入 W-01** —— `ADR-0030` D3 判不适用,否证记录已写进规范 §14,契约 `typed_closure_rejection_record_stays_in_the_spec` 钉住它不被删 | — |
| W-05 | skill 分工表扩 / 收口 | ✅ **已落地** —— `reference.md` §9.6 的分工表从三行扩到**八行**(markup / rich HTML / URL / CSS 标识符 / CSS 字符串 / 动态 CSS / 进 `href` 的 URL / 内联 `<script>` 状态),成员表扩到 8 条;§20 反模式加三条:**「内联 JSON 用字符串拼接」**(OWASP 原话 + 本库实测 `STRINGIFY` 原样输出 `<` `>` `&` U+2028 U+2029)、**「`URL_ENCODE` 让 URL 对 `href` 安全」**(它百分号编码但**不判 scheme**)、**「CSS 转义让动态 CSS 安全」**(OWASP 口径是白名单优先);`SKILL.md` 指针同步。⚠ 原 §9.1「三行」的说法一并改成「**都不是彼此可替代的**」 | 0a3eebf |
