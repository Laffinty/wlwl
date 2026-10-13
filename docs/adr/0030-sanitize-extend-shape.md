# ADR-0030 — `std.sanitize` 扩展到什么形状(CSS 做 / JS 换题 / 类型化收口不适用)

| | |
|---|---|
| **Status** | **Proposed**(2026-10-13,`addendum-13` W-01 调研产出;待业主裁决 D2 / D3) |
| **Date** | 2026-10-13 |
| **Related** | ADR-0029(`std.result` 的形状,同日另一份「实测推翻计划」的 ADR)、`addendum-13-sanitize-extend.md` §3.1 / §3.2、`docs/stdlib/wlwl-stdlib-spec-v0.11.md` §13 / §14、`wlwl-std/src/sanitize.rs`、`wlwl-eval/tests/sanitize_contract.rs` |

## Context

`addendum-13` 的 §0 把「本份的第一产出应当是**有没有真实场景**的调研**,不是成员表」
写成了硬要求 —— 因为规范 §14 自己给 CSS / JS 转义设了门槛「**先有真实场景再立项**」,
并把「类型化收口」的条件写成「**若将来出现宿主形态**」。

本 ADR 是那份调研的产出。它**不新增成员面**,只回答三个问题:CSS 做不做、
JS 做不做(以及**做什么**)、类型化收口适不适用。

## 1. 基线(先数真值,再引用)

计划 §1 的数字经复核**仍然成立**(D11-024 吃过的坑:别照抄):

| 项 | 实测(2026-10-13) |
|---|---|
| `std.sanitize` 成员 | **3**(`HTML_ESCAPE` / `HTML_UNESCAPE` / `HTML_SANITIZE`) |
| `sanitize_contract` 的 `SanCase` 条目 | **81**(与计划一致) |
| 其中 `mxss_` 前缀 | **14**(是 81 的**子集**,不是并列的第三组) |
| 冻结契约 | 81 条 OWASP/mXSS 向例 —— 语义、缺省策略、幂等不变量**一条不动** |

## 2. JS 侧的真实场景:**成立,但计划问错了问题**

### 2.1 实测:`JSON_STRINGIFY` **不是**输出编码函数

OWASP DOM XSS Prevention Cheat Sheet 明写:

> JSON.stringify() is **not an output-encoding function**. Its output is valid
> JSON, but embedding it in HTML requires protection for the destination
> context. […] If JSON must be embedded in an inline script, use a serializer or
> encoder documented for that **exact placement**, protecting both JavaScript
> syntax **and the enclosing HTML script context**, including literal `<`.

本仓库实测确认(不是引 OWASP 了事):

```wlwl
STRINGIFY("</script><img src=x>")   // → "</script><img src=x>"   ← `<` `>` 原样
STRINGIFY("a & b")                   // → "a & b"                   ← `&` 原样
STRINGIFY("a<U+2028>b")              // → 长度 5(引号 2 + 输入 3)   ← U+2028 原样
```

最后一条是**按长度判的**,不是肉眼看字形:转成 `\u2028` 的话输出长度应是
**11**(1+1+6+1+2),实测 **5** ⇒ 原样。U+2029 同。

⇒ **OWASP 点名的五个字符(`<` `>` `&` U+2028 U+2029),wlwl 一个都没转。**
这是本仓库**自己**的缺口,不是转述别人的。

### 2.2 但「通用 `JS_ESCAPE_STRING`」这个成员**问错了**

OWASP 的**首选**不是转义,是**别内联**:用 `Content-Type: application/json` 单独响应,
或走 `<script type="application/json">` 这个 inert 容器。这两条路**都不需要**一个
「JS 字符串字面量转义器」。

`JS_ESCAPE_STRING` / `JS_ESCAPE_COMMENT` 针对的是**通用 JS 字面量**。而它的典型用武之地
——`onclick="f('…')"` 这类事件处理器属性 —— 在 wlwl 里**已被 `HTML_SANITIZE` 覆盖**:
净化器整条剥掉事件处理器属性,根本不会让它到需要转义的地方。

⇒ 真正缺的、且 OWASP **逐字符点名**的,是**「把一份 JSON 加固到能安全内联进 `<script>`」**
这一个窄动作。不是通用转义器。

### 2.3 形状:收整个元素,不收裸串

关键设计点:加固必须是**全有或全无**的。若只暴露一个「转义裸串」的成员,调用方可以
忘记调、可以只转一半、可以转完又拼回去 —— 那样它提供的安全感和裸拼字符串没区别。

OWASP 的首选形态(`<script type="application/json">`)本来就是**整个元素**,照它做:

```
HTML_SANITIZE_SCRIPT(value) -> "<script type=\"application/json\">…</script>"
```

成员收**值**,自己 `STRINGIFY` 再加固再包壳 ⇒ 调用方**没有机会**做错。这一条与
`HTML_SANITIZE` 收整段标记、`HTML_SANITIZE_URL` 收整个 URL 是同一个族。

## 3. CSS 侧:**做**

OWASP 的 CSS 上下文指引:

> CSS: Do not place untrusted data in stylesheets. **If a value must be
> dynamic, allowlist it** (for example a fixed set of color tokens) **and
> CSS-escape it**.

⇒ 真实场景有两个且都成立,而且**优先级不同**:

| 成员 | 场景 | 优先级 |
|---|---|---|
| `HTML_SANITIZE_CSS(text)` | 「值必须动态」时的**白名单**抽取(allowlist 是 OWASP 的第一句) | **高** —— 它挡的是 `expression(` / `url(javascript:)` |
| `CSS_ESCAPE_IDENT(s)` | 模板引擎生成的类名 / `id` / 自定义属性名 | 中 |
| `CSS_ESCAPE_STRING(s)` | `content:` 等属性里的字符串字面量 | 中 |

⚠ 计划 §2 把它们平铺成六行,但 OWASP 的口径里**白名单高于转义**。实现顺序照上表。

## 4. 类型化收口:**不适用**(否证结论)

Trusted Types 的安全收益来自**浏览器在 sink 处强制拒绝裸串**:开
`require-trusted-types-for 'script'` 后,`innerHTML = "str"` 直接抛 `TypeError`,
只有策略产出的 `TrustedHTML` 才被接受。

没有宿主时,这条收益**整体消失**,而且是**定义上的**消失,不是打折:

- 没有 **sink** —— `innerHTML` / `eval` / `document.write` 全部不存在;
- 没有**执行者** —— 强制发生在浏览器里,不在语言里。

剩下的只有「一个做净化、返回字符串的函数」—— **那正是 `HTML_SANITIZE`,已经有了**。
套一层类型标记买不到任何它没有的东西:wlwl 没有浏览器去**识别** tainted 值并在
sink 处**拦截**,而那正是 Trusted Types 相对于「大家记得调用净化器」的**全部**增量。

⚠ 有第三方资料把 Trusted Types 描述成「浏览器可执行策略」甚至称其可防止服务端 XSS ——
**那是错的**;同一批资料自己也写着「Trusted Types do not protect against server-side
XSS; they only cover DOM XSS sinks in the browser」。本 ADR 采信后者,并把这条
**否证记录**写进规范 §14(否证记录与肯定记录同权)。

**复活条件**:wlwl 出现**宿主形态**(能执行代码、能接管输出点的运行时)。
在那之前,「类型化收口」只是一个新内建类型 + 一个运行时检查,不值得。

## Decision

> **Proposed**,待裁决:

- **D1 · CSS 侧做。** `HTML_SANITIZE_CSS` + `CSS_ESCAPE_IDENT` + `CSS_ESCAPE_STRING`,
  实现顺序按 §3 表(白名单优先于转义)。
- **D2 · JS 侧**换题**:不做通用 `JS_ESCAPE_STRING` / `JS_ESCAPE_COMMENT`,
  改做 `HTML_SANITIZE_SCRIPT(value)`**(§2.3)。这是本 ADR 唯一**推翻计划成员表**的裁决。
- **D3 · 类型化收口不实现**,只留否证记录 + 复活条件(§4),写进规范 §14。
- **D4 · 现有 81 条冻结契约一条不动。**

## Consequences

**能拿到的**:把 `std.sanitize` 从「HTML 一个上下文」变成「HTML / CSS / URL /
inline-script 四处」,且每一处都对着 OWASP 的**具体**指引,不是对着「上下文转义」
这个抽象名词。`HTML_SANITIZE_SCRIPT` 收整个元素这一点尤其重要:它让「忘记加固」
这个失败模式**不存在**,而不是靠文档提醒。

**要付出的**:

- ⚠ **D2 是偏离 `addendum-13` §2 的**(它列了 `JS_ESCAPE_STRING` / `JS_ESCAPE_COMMENT`)。
  计划里那两条**不做**的理由要成文,不能默默消失(§3.2 同款体例)。
- ⚠ **`HTML_SANITIZE_SCRIPT` 与 `HTML_SANITIZE` 的失败口径必须不同**:后者**永不因内容
  失败**(否则是 DoS 面),前者失败就说明数据里有连 `<script type="application/json">`
  都装不下的东西 —— 那是**数据**问题,报 `ERR([kind: SanitizeError])`,不是参数错。
- ⚠ **契约期望值必须取自规范原文**(CSSOM / CSS Syntax L3 / OWASP),不是实现输出。
  OWASP 点名的那五个字符是本 ADR 的独立佐证,可直接作为契约向量。
- **明确不做**:HTML 净化器语义变更 / CSS 选择器解析 / JS 代码生成 / DOM 基建 /
  新内建类型。