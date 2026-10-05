# 13 · `std.sanitize` 扩展(CSS / JS 上下文转义 + 类型化收口)

> **层级** L2 · **前置** 无 · **状态** 未动工
> **规范登记** §14 已登记前两项;**第三项(类型化收口)被 §14 明确搁置** ——
> 本份把它的前置条件写清

## 0. 契约摘要

- **目标**:把 `std.sanitize` 从「HTML 一个上下文」扩到**三个**(HTML / CSS /
  JS),并评估**类型化收口**是否可做。
- **非目标**(违反即范围事故):
  - **不做 HTML 净化器的第二套**。`HTML_SANITIZE` 的语义、缺省策略、
    幂等不变量**一条都不动**(它们是 v0.11.3 刚冻结的 79 条契约)。
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
| 冻结契约 | **79 条 OWASP/mXSS 向例** + 324 条变异 + 14 条 mXSS 样本 | `sanitize_contract.rs` |
| 性能 | 10 KB 文章 **17.05 MiB/s** | `impl/crates/wlwl-eval/benches/baseline.txt` M3 段 |
| 规范已登记 | §14:「**CSS / JS 上下文转义**(`CSS_ESCAPE` / `JS_ESCAPE`):需求出现时按 §13 的形态扩展 `std.sanitize`,**先有真实场景再立项**」 | 规范 §14 |
| 类型化收口 | §14:「**类型化收口**(Trusted Types / Sanitizer API 范式):让危险的输出收口只接受经净化策略产出的**类型化值**……它需要宿主 / DOM 基建,不是字符串库能提供的;**wlwl 若将来出现宿主形态,照此记一笔**」 | 规范 §14 |

> ⚠️ 规范 §14 对这两项都写了门槛:「**先有真实场景再立项**」与
> 「**若将来出现宿主形态**」。本份的**第一产出应当是「有没有真实场景」的
> 调研**,不是成员表 —— 见 §3.1。

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
2. **类型化收口的前置**:§14 已明写它「需要宿主 / DOM 基建」,而 wlwl
   **没有宿主形态**。⇒ 本份**只做调研与 ADR,不实现**。ADR 要回答:
   - 「类型」是否要新增内建类型(语言变更)?
   - 若不新增类型,「类型化」用什么承载(标记 `DICT`?运行时检查?—— 那就
     不是 Trusted Types,只是「加了个检查」)?
   - 没有 DOM 时,Trusted Types 的**主要安全收益**(浏览器能识别 tainted
     值并在 sink 处拦截)还能不能拿到?**大概率拿不到** ⇒ 结论可能是
     「不适用,继续搁置」。
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
| `_CSS` / `_URL` | `expression(` / `url(javascript:` / `data:text/html` 逐条拒;相对 URL 恒允许 | OWASP |
| **同款判定** | 断言 `HTML_SANITIZE` 的 `href` 判定与 `HTML_SANITIZE_URL` **结论一致**(同一组 URL 喂两个入口) | **防分裂** |
| **回归** | 现有 **79 条 OWASP 向量 + 324 条变异** 逐条不动 | v0.11.3 的冻结契约 |
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
| W-01 | **真实场景调研**(§3.1)—— 决定 CSS / JS 各自做不做 | 未动工 | — |
| W-02 | CSS 转义族 + 契约(CSSOM / CSS Syntax 逐条) | 未动工 | — |
| W-03 | `HTML_SANITIZE_CSS` / `_URL` + **同款判定**断言 | 未动工 | — |
| W-04 | 类型化收口 **ADR**(含「不适用」的否证结论) | 未动工 | — |
| W-05 | skill 分工表扩 / 收口 | 未动工 | — |
