# v0.11.1 构建计划 —— `std.web`:用 wlwl 统一描述 HTML / CSS / JS

> **状态**:初步方案(2026-10-01 立项)。本文是**设计裁决 + 里程碑骨架**,不是逐条
> 工单;每个里程碑落地前需按 v0.11 的格式补齐变更面清单。
>
> **配套分册**:偏差登记沿用 [`deviations-v0.11.md`](deviations-v0.11.md),
> 本计划的偏差从 **D11-020** 起续号(不新开分册 —— v0.11.1 未 tag,
> `Cargo.toml` 仍是 `0.11.0`,台账归期以「下一个 tag」为准)。

---

## 0 基线棘轮

本计划**不得降低**以下任一基线(取自 [`README.md`](README.md) 2026-10-01 状态表
与 plan-v0.11 §8):

| 项 | 棘轮值 |
|---|---|
| `cargo fmt --check` | 0 diff |
| `cargo clippy -D warnings` | 0 |
| `cargo test --locked --all-targets` | **37 套件 / 1860 passed / 0 failed** |
| probe 用例 | 142(不得减少) |
| 附录 G | 逐字节不变 |
| 既有 4 个 R1 std 模块契约 | `EXPORT` 面 / 签名 / 语义 / 诊断一字不改 |

**命名空间纪律**(标准库规范 §0.4,v0.x 段):`std.web` **不得重导出任何全局内建
同名成员**。`SPLIT` / `STR` / `FORMAT` / `LEN` / `TYPE` / `CONTAINS` / `AT` 等
一律只调用、不导出。

**治理四件套**(§0.3)必须**同批次**完成,缺一不可发布:规范条目 / 附录 A 镜像
(生成器 + 锁测试)/ 从规范 markdown 表格解析出成员面与实现对拍的外部对照测试 /
ERR 消费者注册(`std.web` 为纯 R1,第四条由语言语义 §8.2 天然承担,登记为豁免)。

> **版本与 tag 门禁(业主 2026-10-01 裁决)**:v0.11.1 **尚未完工,当前所有工作
> (含本计划)都属 v0.11.1**。`std.web` 落 v0.11.1,不另开版本号。
> **TAG 由业主下令后才能打** —— 在此之前 `impl/Cargo.toml` 的
> `[workspace.package] version` 可以按 release workflow 的 `version-check` job
> 要求随发布推进,但**不得 `git tag`、不得推 tag**。原「落 v0.11.2 还是 v0.12」
> 的选项作废(原 §9 Q1 已裁决,见 §9.1)。

---

## 1 目标与非目标

### 1.1 目标

1. **一份 wlwl 模板描述 → 三个正规产物**(`index.html` / `index.css` / `index.js`),
   三者由**同一棵节点树**导出。
2. **统一性落在语义层,不落在文件层**。真正的杠杆是三条**共推导**(co-derivation,
   §3.3),它们是「写三份文件」永远得不到的。
3. **JS 产物按构造最小化**。编译器逐个动态绑定判定「CSS 2025 能否表达」,
   能则**不进 JS 产物**。这是本计划最核心、也最前沿的一条。
4. **全部实现落在 R1**(ADR-0021 §0.1 规则 3:非系统调用、非性能敏感 → R1),
   驱动侧只留一个薄 CLI 子命令。
5. **产物可审计**:发光 JS 产物受**产物不变量**(§3.4)+ 锁测试守护,
   任何违反都让门禁变红。
6. **零第三方依赖**(业主裁决,规范性承诺):产物自带的最小运行时承载事件委托、
   handler 表、view-transition 命名与状态存取;不引任何外部库(不变量 **JS-5**)。
7. **组件状态在构建期即可验证可序列化**(§3.7 裁决 E-1)—— 这是类型级承诺,
   必须在第一个版本就定死,晚定即 breaking。

### 1.2 非目标(本版明确不做)

| 不做 | 理由 |
|---|---|
| **wlwl 语义后端**(把 wlwl 程序整体编译成跑在浏览器里的 JS) | §3.2 否决,六条结构性理由 |
| **完整 resumability**(Qwik 式:客户端态序列化 + 浏览器完全不重放) | §3.7 裁决 E-4 否决。构建期页面里静态部分占绝大多数,其核心机制冗余;而代价是**用户可见的类型级约束**,最难回收 |
| 虚拟 DOM / 客户端渲染器 / 状态管理 | 产物是**构建期**的;运行时归浏览器 |
| 服务端渲染、路由、HTTP、`std.net` | R2,且 §11 已列为独立候选 |
| 任意 CSS / JS 的解析与重写 | `<style>` / `<script>` 内容**逐字透传**,零解析 |
| 正则引擎 | §3.5 否决 |
| minify / 资源指纹 / autoprefixer | R2 + 成熟 crate(ADR-0021 §0.1 规则 1) |
| 响应式运行时(signal / effect 传播) | 留后续版本;§3.3 裁决 D 只**声明**效果面,本版不实现分析(§2.5) |
| 惰性 handler 块(`import()`) | §3.7 裁决 E-3,留后续版本;纯客户端工作,不阻塞构建期契约 |
| 拆分成 `std.html` / `std.css` / `std.template` 三个命名空间 | §9 Q2 已裁决为**单个 `std.web`** |

---

## 2 上游调研(具名)

按记忆纪律,本节具名全部上游来源;「证据强度」列区分**一手** / **多方** / **单一**。

### 2.1 CSS:平台在 2025 吸收了框架职责

| 能力 | 状态(2025–2026) | 对本计划的意义 | 来源 | 强度 |
|---|---|---|---|---|
| `@scope` | 2025-12 进入 Baseline;Chrome 118+ / Safari 17.4+ / **Firefox 146+** | **组件样式隔离的原生解法,不需要 shadow root**。这是本计划的旗舰裁决 | **W3C CSS Cascade 6 §2.5**([w3.org/TR/css-cascade-6](https://www.w3.org/TR/2021/WD-css-cascade-6-20211221))· [lambdatest](https://www.lambdatest.com/web-technologies/css-cascade-scope-safari) · [MDN @scope](https://developer.mozilla.org/en-us/docs/Web/CSS/@scope) | **一手 + 多方** |
| `@layer` | 稳定(Chrome 99+/Safari 15.4+/FF 97+) | 层叠顺序显式化,消灭 specificity 战争 | [frontend-hero](https://frontend-hero.com/modern-css-features) | 多方 |
| `@property` | Chromium + Safari,部分支持 | 自定义属性**带类型** ⇒ 可过渡 / 可动画。设计 token 的原生载体 | [codeboxr](https://codeboxr.com/ultimate-guide-to-modern-css-tricks-additions/) | 单一 |
| View Transitions + `view-transition-class` | 2025-10 进入 Baseline | 列表重排动画可自动生成 transition name | [plainenglish](https://javascript.plainenglish.io/baseline-quietly-fixed-the-hardest-parts-of-css-and-most-devs-missed-it-464dbe93e5b7) | 单一 |
| Popover API | 2025-01 进入 Baseline | 顶层渲染,免 z-index | [plainenglish](同上) | 单一 |
| container query / `:has()` / 原生嵌套 | 全主流稳定 | 组件优先响应式 + 父状态选择 ⇒ **减少对 JS 的需求**(§3.3 裁决 C) | [frontend-hero](https://frontend-hero.com/modern-css-features) | 多方 |
| `color-mix()` / `oklch()` / 相对颜色 / `light-dark()` | 稳定 | 主题派生在 CSS 内完成 | [naeemnur](https://naeemnur.com/modern-css-features-you-should-know-in-2025) | 单一 |
| Declarative Shadow DOM | 全 evergreen 落地,支持 SSR | 可选的强封装路径(**不作为默认**,§3.3 裁决 B) | [datafmt](https://datafmt.com/de/blog/en-web-components-state-2025) | 单一 |

> ⚠ **实施前须复核**:上表 Baseline 日期来自二手技术博客,非 `webstatus.dev`
> 权威源。W1 仍需逐条核对 `@scope` / `@property` / `light-dark()` 的当前状态。

### 2.1.1 `@scope` 语义定案(原 Q3,已查实)

**已用一手规范来源结清**,不再作为 W1 的未验证假设。一手来源:
**W3C CSS Cascading and Inheritance Level 6 §2.5「Scoped Styles」**
([WD-css-cascade-6-20211221](https://www.w3.org/TR/2021/WD-css-cascade-6-20211221)),
辅以 [MDN `@scope`](https://developer.mozilla.org/en-us/docs/Web/CSS/@scope) 与
[MDN `:scope`](https://developer.mozilla.org/docs/Web/CSS/Reference/Selectors/:scope)。

四条定案事实:

| # | 事实 | 规范依据 |
|---|---|---|
| S-1 | **scope 根在作用域内**。「Each resulting scope includes a scoping root and all its descendants」;上界 **inclusive** | CSS Cascade 6 §2.5 |
| S-2 | **但裸选择器仍匹配不到根**。块内裸选择器与 `&` 行为等价于前置 `:where(:scope)`,即**后代**关系,而 `:scope` 不是自身的后代 ⇒ 要命中根必须显式写 `:scope`(或 `&`) | MDN `@scope`「both bare selectors and `&` behave as if `:where(:scope)` were prepended」 |
| S-3 | **`@scope` 前置选择器不增加内层特异性**。「Unlike Nesting, selectors within an `@scope` rule do not acquire the specificity of any parent selector(s) in the `@scope` prelude」——`@scope (#hero) { img {} }` 与 `:where(#hero) img` 同为 `(0,0,1)` | CSS Cascade 6 §2.5.1(**一手,无争议**) |
| S-4 | **无前置子的 `@scope { }` 只在 `<style>` 元素内生效**,绑定到该 `<style>` 的父元素;放进外部 CSS 文件则无根可绑、什么都不匹配 | [lambdatest](https://www.lambdatest.com/web-technologies/css-cascade-scope-safari) · [MDN `@scope` prelude-less] |

> 🔺 **来源冲突(已记录,不静默取舍)**:`&` 的特异性,一手规范说
> `&` 表示「the selector representing the scoping root」⇒ 应脱糖为
> `:is(.card)`,携带该选择器的特异性 `(0,1,0)`(seo-guider 的实测描述与规范一致);
> 而 MDN 写「`&` behaves as if `:where(:scope)` were prepended」⇒ 特异性为 0。
> **两者不能同时成立。** 裁决:**`std.web` 的发射器不使用 `&`,也不依赖 `&` 的
> 特异性语义**;面向用户的根元素样式一律用 `:scope`(其行为 S-2 已定,且
> `:scope` 作为伪类 `(0,1,0)` 在规范与 MDN 上一致)。**W4 门禁里加一条负向用例:
> 模板中出现 `&` 时给构建期警告**,把它挡在语义分歧之外。

**S-1 + S-2 + S-4 合起来给出一个比「属性哈希」更好的方案** —— 见 §3.3 裁决 B。
关键洞察:**无前置子的 `@scope` 绑定到 `<style>` 的父元素**,所以只要把组件的
`<style>` 放进组件根元素内部,**作用域根就是组件根本身**,既不需要注入
`data-wc` 属性,也不需要知道根元素是什么标签。

### 2.2 组件与产物形态:islands

- 「islands architecture」由 Katie Sylor-Miller 提出、Jason Miller 推广;
  Astro 把它做进主流框架。**默认零 JS,只有显式标记的组件才 hydrate。**
  来源:[Astro 官方文档](https://docs.astro.build/en/core-concepts/component-hydration/) ·
  [patterns.dev](https://www.patterns.dev/vanilla/islands-architecture/) · 强度:多方
- hydration 指令是**人写的标注**(`client:load` / `client:visible` / `client:idle`)。
  → **本计划的差异化点**:让**编译器**决定岛,而不是让人标注(§3.3 裁决 C)。
- Qwik 的 resumability:把服务端状态序列化到 HTML,客户端**不重放**。
  与本计划同源但更激进;列入 §9 Q4。

### 2.3 模板表面语法:三条路,一条已被实测否掉

| 方案 | 代表 | 关键性质 | 本计划裁决 |
|---|---|---|---|
| **组合子** | Gleam + Lustre:`html.div([attr.class(...)], [...])` | 无需编译器,但**啰嗦** | ❌ 否决(§3.1) |
| **宿主字符串插值** | 各种模板字符串 | 零成本,但**静态/动态不可分** | ❌ 否决(§3.1) |
| **惰性模板 + 宿主内解析** | htm(**700 字节,无编译器**)、lit-html | 静态片段与动态值**分离**;模板是**值** | ✅ **采纳** |

来源:[htm 讨论](https://www.mo4tech.com/?p=28372) · [lit-html](https://github.com/shutdown256/lit-html) ·
[Gleam/Lustre 前端实践](https://blog.arda.tr/blog/2026-04-01-rescript-vs-gleam-is-it-worth-to-write-your-frontend-in-a-typesafe-functional-language/) · 强度:多方(组合子啰嗦的具体数字为**单一来源**的从业者自测,见下)

**组合子被实测否掉的证据**(单一来源,需在 W2 复核):同一份前端项目用 Gleam
重写,Hero 组件从 TS 的 82 行涨到 Gleam 的 138 行;图标因无法复用 `lucide-react`
而**手写 314 行 SVG 路径**。作者结论是「JSX 式语法在人体工学上优势太大,不能放弃」。
⇒ 方向可信(ReScript 靠 JSX 也确实把渲染代码压到 TS 的 70%),但**具体行数是
一个人的项目**,不作为硬数据引用,只作为「组合子不适合做主表面语法」的倾向性证据。

lit-html 另有一条对本设计直接相关的性质:**模板字面量是值,不是函数调用**。
它可以被存进变量、传参、被库二次处理 —— JSX 做不到(JSX 编译后就是函数调用)。
本计划的组件因此天然可组合。

### 2.4 编译到 JS 的语言学:i64 与 f64 的鸿沟

**Gleam v1.6.0(v1.6.0 released,2024-11-18)新增 unsafe number 警告**,一手来源:

> When compiling to JavaScript Gleam uses JavaScript's number types, to ensure
> good interop... This means that it inherits some shortcomings that do not exist
> on the BEAM, namely a maximum and minimum safe number size. When targeting
> JavaScript the compiler now emits a warning for integer literals and constants
> that lie outside JavaScript's safe integer range...
> ```
> warning: Int is outside the safe range on JavaScript
> 1 │ pub const i = 9_007_199_254_740_992
>   │               ^^^^^^^^^^^^^^^^^^^^^ This is not a safe integer on JavaScript
> ```
> 来源:[gleam.run/news/context-aware-compilation](https://gleam.run/news/context-aware-compilation/)
> —— **已抓取原页核实,非二手转述。强度:一手。**

⇒ Gleam 的选择是「用宿主 number 换互操作 + 超界**警告**」。本计划取**更严**的
一条(§3.4 产物不变量 JS-2),理由是页面描述里的整数只可能是索引 / 计数 / 像素值,
没有任何一种正当需求会超过 2^53 —— 所以这里是**构建错误**而不是警告。

Elm 侧的对照:无运行时异常保证 + 激进 DCE,但**不 minify 时中��应用 bundle
可超 100 KB**,且自 0.19.1(2019-10)起核心停滞。
来源:[grokipedia/Elm](https://grokipedia.com/page/Elm_(programming_language)) · 强度:单一

### 2.5 前沿语境:代数效果与后端迁移

- 语言规范 §17.4 已把 `MethodCall` / `ProtocolViolation` 两个 tag 标为
  「**为代数效果后端迁移预留**」,并明写**不由运行期产生**、**实现不得依赖它们
  产生任何可观察行为**。本计划的所有设计**只读这个预留面,不写它**。
- 学术侧近期工作(OPSLA 2024/2025、ICFP 2025)集中在 lexical effect handler
  的零开销编译与 dynamic-wind 语义,与本计划**无直接依赖**,仅作为「wlwl 后端
  迁移有持续学术进展」的方向性佐证。
  来源:[Tübingen SE 论文列表](https://se.cs.uni-tuebingen.de/publications/peer-reviewed/) ·
  [OOPSLA 2025 Dynamic Wind for Effect Handlers](https://2025.splashcon.org/details/OOPSLA/179/Dynamic-Wind-for-Effect-Handlers) ·
  强度:一手(会议程序)

**结论:本计划不触碰代数效果。** 它只保证 §3.3 裁决 D 声明的接口形状**不堵死**
未来的效果分析,不在 v0.11.1 实现它。

---

## 3 设计裁决(逐条)

### 3.1 表面语法:惰性模板,不用组合子,不用宿主插值

**裁决 A**:页面源文件扩展名 `.wlwt`。它是**惰性文本** —— wlwl 求值器从不解释它;
R1 侧一个递归下降解析器拥有它。CLI(R2 侧)读文件,把字符串交给 R1 模块。

**为什么不用宿主插值 `${}`** —— 这条最容易被想当然:

wlwl 的 `${...}` 是**立即求值**的(语言规范 §1.2:插值内表达式按完整表达式语法
求值,结果以 `STR` 渲染后嵌入;产生 `ERR` 则整个字面量结果为该 `ERR`)。因此:

1. **静态片段与动态值不可分**。htm / lit-html 的全部价值建立在
   `tag(strings, ...values)` 这个**分离**上 —— 靠它才能只重渲染动态部分。
   wlwl 插值产出一个已经拼好的单一字符串,分离信息**已经丢失且不可恢复**。
2. **无法生成 JS**。值在模板求值那一刻就已经渲染成文本了;到发射器手里只剩
   死字符串,**没有任何东西可以翻译成 JS**。
3. 即便把 `\$` 转义当逃生阀,也只是「在字符串里藏模板语法」,并不比 `.wlwt`
   更省事,却丢掉了惰性。

**为什么不用组合子** —— §2.3 的实测方向 + 治理成本:组合子会把每个元素变成一次
`FUN` 调用,产物与源码的距离最远(最坏情况要手写 SVG 路径)。

**裁决 A 的形态**(示意,非终稿语法):

```
{{#component "card"}}
  <style>
    /* 逐字透传 —— R1 不解析 CSS */
    :scope { padding: 1rem; border-radius: .5rem; }
    h2 { margin: 0; }
  </style>
  <section class="card">
    <h2>{{ title }}</h2>
    <p>{{ body }}</p>
    <button w:on="click->toggle" w:id="{{ id }}">展开</button>
  </section>
{{/component}}
```

四条语法约定:

| 约定 | 内容 |
|---|---|
| `{{ expr }}` | 表达式在**构建期**由 R1 求值(wlwl 表达式,非字符串) |
| `{{#component}}` / `{{/component}}` | 打开组件作用域 → 触发裁决 B 的共推导 |
| `<style>` / `<script>` 体 | **逐字透传**,零解析 |
| `w:` 前缀属性 | 语义标记,wlwl 求值器不关心,只有发射器读 |

### 3.2 明确否决:wlwl 语义后端

「把 wlwl 程序整体编译成 JS,让 wlwl 语义在浏览器里跑」——**六条结构性理由**:

1. **R1 看不见 AST**。后端要编译程序自身,AST 是 Rust 类型 ⇒ 需新增 R0 原语。
2. **i64 vs f64**(§2.4,一手证据)。`E0035` 是可观察诊断,在 `number` 上无法表达。
3. **码点 vs UTF-16**。§4.5 / §10.5 的 `SUB` / `INDEX_GET` 是码点语义,JS 字符串
   是 UTF-16 码元,代理对上分叉。
4. **`ERR` 传播需重新设计**。§8.2 规定 `ERR` 穿透每次调用**含条件位** ——
   D11-004 至今挂账的根因正是这条,说明它在解释器内部都难做对。
5. **结构化并发需整个运行时**。`SCOPE`/`SPAWN`/`AWAIT`/channel 是单线程协作调度器
   (ADR-0016 / 0017),发射到 JS 等于写一个调度器,不是文本变换。
6. **`UPPER`/`LOWER` 仅 ASCII**(§10.5,非 ASCII 是实现定义),JS `toUpperCase()`
   是全 Unicode。

补充:规范 §17.4 自己就把后端迁移定位成**未来阶段**并预留了 tag 面。
⇒ 本计划跳到 v0.11.1 是跳阶段,否决。

### 3.3 三条共推导(本计划的核心价值)

「统一」不指「一个文件写三种语言」(那是普通 HTML)。统一指**三条推导** ——
它们需要三份独立源码里不存在的全局视野。

**裁决 B:作用域共推导(依 §2.1.1 的 S-1/S-2/S-4 定案)**

`{{#component "card"}}` 同时驱动两个发射器,但**机制比"注入属性 + 带前置子的
`@scope`"更省**:

1. **HTML 发射器**把组件的 `<style>` 块**移入组件根元素内部**(作为其第一个子节点)。
   若用户原本就把它写在组件块内,这一步是恒等变换。
2. **CSS/HTML 发射器**把该 `<style>` 体包进**无前置子的** `@scope { ... }`。

由 **S-4**,无前置子的 `@scope` 绑定到**该 `<style>` 的父元素** —— 也就是组件
根本身。⇒ **作用域根自动就是组件根**:

- **不需要**注入 `data-wc` / `data-scope` 之类属性;
- **不需要**知道组件根是哪个标签或哪个类(所以**零 CSS 解析**成立);
- **不需要**类名哈希(这是 Svelte / Vue 至今仍在做的事)。

用户写:

```html
{{#component}}
  <style>
    :scope { padding: 1rem; border-radius: .5rem; }   /* 根元素(S-2:必须显式) */
    h2 { margin: 0; }                                 /* 后代 */
  </style>
  <section>
    <h2>{{ title }}</h2>
  </section>
{{/component}}
```

发射成:

```html
<section>
  <style>
    @scope {
      :scope { padding: 1rem; border-radius: .5rem; }
      h2 { margin: 0; }
    }
  </style>
  <h2>…</h2>
</section>
```

**特异性是平的**(S-3,一手规范):块内 `h2` 仍是 `(0,0,1)`,与
`@scope (#hero) { img {} }` 同权。单个工具类仍能覆盖组件样式,不需要 specificity
升级竞赛。

**渐进增强(S-4 的直接推论)**:外部 CSS 文件里放无前置子的 `@scope` 会**完全失效**。
所以组件样式**必须**是 HTML 内的行内 `<style>`。发射器同时输出一份
`@supports (at-rule: @scope)` 守卫的未作用域副本,老浏览器拿到的是**不隔离但
能看**的页面 —— 而不是哈希类名那种需要解析 CSS 才能生成的降级方案。**这消掉了
原风险 R-1 的整个降级分支。**

**代价(须写进规范)**:组件样式进 HTML ⇒ 无法独立缓存。这是与 Svelte / Vue 组件
样式相同的取舍,可接受;外部 CSS 文件只承载 token / reset / 页面级规则。

默认**不**用 Shadow DOM(它有「CSS 不穿透边界」的坑,且 Declarative Shadow DOM
只在真需要强封装时才是对的答案);作为**可选发射目标**保留 —— 开启时改发
`@scope ([data-wc~="card"])` 并回到属性注入路径,与本裁决共用同一棵节点树。

**裁决 C:行为最小化共推导(最前沿的一条)**

发射器遍历节点树,对**每一个动态绑定**问:「CSS 2025 能否表达?」

| 绑定形态 | 判定 | 产物 |
|---|---|---|
| 父状态影响子样式 | `:has()` | 纯 CSS,**不进 JS** |
| 容器尺寸响应 | `@container` | 纯 CSS,**不进 JS** |
| 组件内样式隔离 | `@scope` | 纯 CSS,**不进 JS** |
| 原生控件状态 | `:checked` / `:valid` / `:popover-open` / `:target` | 纯 CSS,**不进 JS** |
| 纯展示态切换 | `[data-wc-state~="..."]` + 属性选择器 | 纯 CSS,**不进 JS** |
| 列表重排动画 | `view-transition-name` 自动生成 | 纯 CSS,**不进 JS** |
| 其余 | 不可归约 | 进 JS 产物 |

⇒ JS 产物**按构造最小**,而不是靠人写 `client:*` 标注去裁剪(§2.2 的差异化点)。
判定表是**固定模式匹配**,纯函数,R1 可写。

**裁决 D:token 共推导 + 效果声明**

`TOKEN("accent", ...)` 发射成
`@property --w-accent { syntax: "<color>"; inherits: true; initial-value: ...; }`
⇒ token 变得**带类型、可过渡、可继承**,这是 §2.1 里 `@property` 的直接兑现。

组件可声明效果面 `{{#component "x" effects=(READ_FS, HTTP)}}`。
**本版只把这个声明面设计出来并原样透传到产物注释,不做效果分析**
(§2.5 理由)。这样未来做岛选择时**接口不堵死**,而 v0.11.1 不背这个复杂度。

**上下文敏感转义**(裁决 A 的附带红利,值得单列):

`{{ }}` 的插入点决定转义规则,**四个上下文各自不同**:

| 上下文 | 规则 |
|---|---|
| HTML 文本 | `& < >` 转义 |
| HTML 属性 | 加 `"` `'` |
| CSS 值 | 按 CSS 标识符 / 字符串上下文 |
| JS 字符串 | 按目标语言字符串字面量转义 |

⇒ 一处写错上下文就产出坏页面,而且**坏在别人看不见的地方**。
wlwl 的**码点精确字符串**(§1.2 / §10.5)让这件事可以被可靠实现和可靠测试 ——
这是语言选型带来的真实红利,不是修辞。

### 3.4 产物不变量(JS 产物)+ 锁测试

四条不变量,每条配一个**对产物本身**的锁测试(不是对发射器代码的断言):

| 编号 | 不变量 | 检查方式 |
|---|---|---|
| **JS-1** | JS 产物**不含位置化字符串索引**(`s[i]` / `.slice(i)` / `.charCodeAt(i)`) | 正则扫产物;命中即红 |
| **JS-2** | 每个跨边界的 `INTEGER` 满足 \|v\| ≤ 2^53−1 | 发射时校验,越界抛**构建诊断**(非警告) |
| **JS-3** | 产物**不用** `.toUpperCase()` / `.toLowerCase()` 处理数据派生串(§10.5 仅 ASCII) | 正则扫产物 |
| **JS-4** | 产物**不含** wlwl 算术的重新实现 | 形态锁:产物中算术运算符只出现在字面量常量里 |
| **JS-5** | 产物**零第三方依赖**(业主 2026-10-01 裁决,写为**规范性承诺**)。事件委托、handler 表、view-transition 命名、状态存取全部为 `std.web` 自带运行时;不得出现任何外部 `import` / CDN 引用 / `node_modules` 路径 | 扫产物:除自身运行时外无任何 `import` / `src=` 外链 |

JS-1 / JS-3 的处理方式是把 codepoint 敏感的计算**在 R1 里做完**,结果作为
**不透明值**跨边界。⇒ 码点/UTF-16 分叉从「运行期危险」变成「构建期围栏」。

**JS-2 比 Gleam 更严的理由**(§2.4):Gleam 选警告是因为它要跟整个 JS 生态互操作。
页面描述里的整数只可能是索引 / 计数 / 像素值,没有正当需求超过 2^53 ⇒ 构建错误。

**JS-2 的围栏延伸(§3.7 裁决 E-1)**:该围栏**管到运行时状态**,不只管发射出的
字面量。wlwl `INTEGER` 是 i64,JSON / JS 只有 f64 ⇒ **客户端态按 f64 有界**。
字面量越界在构建期报错;**fetch 结果无法再诊断**,只能在规范里事先声明这条边界。

### 3.5 明确否决:通用正则引擎

手写正则引擎在 R1 里规模与性能双杀(参照 D11-012 的同源成本剖面)。
`<style>` / `<script>` **逐字透传**已经把 95% 的「需要正则」需求消掉了。
若 wlwl 特有语法确实需要超出 `STARTS_WITH` / `SPLIT` / `CHAR_AT` / `INDEX` 的匹配,
走 kernel 逃生口(`_RE`,ADR-0021 §0.2 允许;先例 `_KIND` / `_RANGE`),
且必须满足 §0.2 的三条 kernel 纪律。

### 3.6 层归属

| 组件 | 层 | 依据 |
|---|---|---|
| 模板词法 / 语法分析 | **R1** | 纯字符串变换 |
| 节点树 → HTML / CSS / JS 三发射器 | **R1** | 纯字符串变换,构建期,非性能敏感 |
| 行为最小化分析(裁决 C) | **R1** | 固定模式匹配,纯函数 |
| 上下文敏感转义 | **R1** | 纯函数 |
| token / `@property` 发射 | **R1** | 纯字符串变换 |
| 读 `.wlwt` / 写三产物 | **R2** | `READ_FILE` / `WRITE_FILE` 已是 R2 |
| `wlwl web build` 子命令 | R2 侧薄驱动 | 与现有 CLI 边界一致 |
| minify / 指纹 / autoprefixer | R2(未来) | ADR-0021 §0.1 规则 1,复用成熟 crate |

**实测支撑**(2026-10-01,release 档,单线程,`wlwl` 二进制直跑,探针写在
`%TEMP%` 未入库):

| 形态 | 节点数 | 产物 | 耗时 |
|---|---|---|---|
| 平铺追加 `s = s + x` | 5,000 | 134 KB | 164 ms |
| 平铺追加 | 20,000 | 549 KB | 1,434 ms |
| 平铺追加 | 50,000 | 1.39 MB | **22,448 ms** |
| 递归二分构造 | 50,000 | 1.39 MB | **4,239 ms** |
| 逐码点循环(转义形态) | 9,000 字符 | 24 KB | ~19 ms(≈2 µs/码点) |

⇒ **真实页面(1–5k 节点)两种写法都在 200 ms 内,性能不是本计划的风险**。
设计约束只有一条:**不要用大数组做累积**(`PUSH` 是二次的,16,000 元素即 3.9 s,
与 D11-012 同源);字符串拼接随便用。

### 3.7 JS 产物的装载策略(原 Q4,已分析定案)

**先修正原措辞。** 计划 v1 把裁决 C 与 resumability 写成「两套不同的裁剪哲学,
二选一」——**这个框架是错的**。二者**串行且正交**:

- **裁决 C 按能力裁**:决定**哪些绑定需要 JS 存在**。
- **resumability 按状态裁**:决定**已保留的绑定如何少执行、少下载**。

后者作用在前者的输出上,不是替代品。

**关键洞察(这是本裁决的全部依据)**:裁决 C 配合**构建期渲染**,意味着
**DOM 本身就是序列化状态**。一个构建期页面里,`count = 0` 已经在 HTML 里,
点击要改什么、要调哪个 handler,都可以由构建期写进 `data-*` 属性。
⇒ **resumability 最贵的那一块(把组件状态序列化进 HTML)对静态部分是冗余的。**

真正缺的只有三样,按代价排序:

| 缺什么 | 代价 | 收益 |
|---|---|---|
| ① **单根事件委托**(一个 `document` 级监听 + `data-wc-h` 指向 handler id) | 十几行 | 不重放组件树;初始载荷里没有 per-element 监听器。**这是 resumability 收益的绝大部分** |
| ② **惰性 handler 块**(按需 `import()`) | 中 | handler 代码不进初始包。纯客户端运行时工作,**不改任何构建期约束** |
| ③ **客户端态序列化**(用户输入 / fetch 结果) + 不重放 | 大 | 只对「跨组件共享的客户端态」有意义;且引入**用户可见的类型级约束** |

**裁决 E(三分层)**:

| 层 | 内容 | 落点 |
|---|---|---|
| **E-1** | **状态可序列化性检查**(构建期)。组件声明的状态必须是 JSON 可表示的:`NULL`/`BOOLEAN`/`INTEGER`/`FLOAT`/`STRING`/`ARRAY`/`DICT`(键为 STRING)/`RESULT`;**`FUNCTION` / `CLASS` / `INSTANCE` / `CHANNEL` / `TASK` 一律拒绝** | **v0.11.1** |
| **E-2** | **单根事件委托** + 最小运行时 | **v0.11.1** |
| **E-3** | **惰性 handler 块**(`import()`) | 后续版本 |
| **E-4** | **完整 resumability**(客户端态序列化 + 不重放) | 后续版本单独立项,**仅当实测显示初始 JS 仍是瓶颈才做** |

**为什么 E-1 必须在 v0.11.1,哪怕 E-4 不做** —— 三条理由,第一条是决定性的:

1. **它是类型级承诺,晚做就是 breaking change。** 「组件状态必须可序列化」会写进
   规范与用户的心智模型。v0.12 再加,已经写了不可序列化状态的组件全部要改。
2. **它让裁决 C 的「按构造最小」从口号变成可验证的命题**:被 CSS 归约掉的绑定
   不需要状态;需要 JS 的绑定,其状态必须可重建。**没有 E-1,这个断言无法成立。**
3. **它是纯 R1**:一个类型谓词,零 IO。

**E-1 的两个复用点**(不重造轮子):

- 序列化**直接调用已有的 `std.json.STRINGIFY`**(已是 R2、已在附录 A),不自造编码。
- 与**裁决 D 的效果声明**合流:组件同时声明 `effects=` 与 `state=`,构建期一次性
  校验「这个组件允许做什么 + 它能记住什么」。

**必须写进规范的窄化(不是 bug,是明示取舍)**:wlwl `INTEGER` 是 i64,而 JSON /
JS 只有 f64(§2.4 一手证据)。⇒ **客户端态按 f64 有界**,且这条围栏**从 JS-2
延伸到运行时状态**,不只管发射出的字面量。超出者:构建期对**字面量**报错(JS-2),
对 **fetch 结果**则在规范里明示「客户端态整数受 `Number.MAX_SAFE_INTEGER` 约束」
—— 运行时无法再诊断,只能**事先声明**。

**否决**:把完整 resumability(E-4)放进 v0.11.1。理由是收益/代价比 —— 目标产物是
**构建期页面描述**,静态部分占绝大多数,E-4 的核心机制在那里是冗余的;而它的代价
(用户可见的类型约束 + 客户端态协议)是**最难回收**的一类成本。

---

## 4 里程碑

> 人日为粗估。W1–W3 是**纯 R1 + 文档**,不碰 Rust 语义,因此与批次 B(`YIELD`
> 续体)可并行。

### W1 平台状态复核 + 契约骨架(约 1.5 人日)

- 逐条复核 §2.1 表里 `@property` / `light-dark()` / View Transitions 的 Baseline
  状态,改用 `webstatus.dev` 等权威源;修正任何二手来源带来的偏差
  (**先修文档再写码** —— 避免照着二手博客的过期状态实现)。
  (`@scope` 的**语义**已由 §2.1.1 用一手规范结清,不再复核;这里只复核其
  Baseline **日期**。)
- 写 `docs/stdlib/wlwl-stdlib-spec-v0.11.md` 的 `std.web` 条目骨架
  (成员表 + 语义 + 失败行为)。**此时不写实现** —— 规范先行是本仓既有纪律。
- 在规范里写死 §3.3 裁决 B 的**产物形状契约**:组件样式为行内 `<style>` +
  无前置子 `@scope` + `@supports` 守卫副本;根元素样式必须显式 `:scope`。
- 在规范里写死 §3.7 裁决 E-1 的**状态可序列化类型契约** + §3.4 的
  **JS-1..JS-5 五条产物不变量**(含零第三方依赖的规范性承诺)。
  **这两条是本计划里唯一的类型级承诺,晚写即 breaking。**

### W2 词法 + 语法 + 节点树(约 3 人日)

- `.wlwt` 递归下降解析器(纯 R1),产出节点树 DICT 形状。
- `<style>` / `<script>` 体作为**不透明字符串**原样挂在节点上,零解析。
- 治理四件套的第 2、3 条此时落地(附录 A 镜像 + 外部对照测试),
  因为成员面一旦开始长就难改。
- 门禁:节点树形状的冻结用例 + 解析器幂等锁。

### W3 HTML 发射器 + 上下文敏感转义(约 2.5 人日)

- HTML 发射器 + 四上下文转义。
- 裁决 B 的**样式重定位**:把组件 `<style>` 移入组件根元素内部(裁决 B 第 1 步)。
- 门禁:转义的四上下文各一组用例,含码点边界(代理对、组合字符、裸 `&`)。

### W4 CSS 发射器 + token + 行为最小化(约 4 人日)

- 无前置子 `@scope { }` 包裹、`@supports` 守卫副本、`@layer` 分层、
  `@property` token 发射。
- 裁决 C 的判定表 + 降级路径(判不了就走 JS,并在构建期计数上报)。
- **负向门禁**:模板组件 `<style>` 里出现 `&` ⇒ 构建期警告
  (§2.1.1 的来源冲突;发射器不依赖 `&` 的特异性语义)。
- 门禁:判定表的**表驱动用例**,每条 CSS 原语一行;
  「判不了时必须降级」的负向用例。

### W5 JS 发射器 + 产物不变量锁(约 4.5 人日)

- handler 收集与发射;产出 **JS-1..JS-5 五条**锁测试。
- **§3.7 裁决 E-1**:状态可序列化性检查(纯 R1 类型谓词),序列化复用
  `std.json.STRINGIFY`;与裁决 D 的 `state=` 声明合流。
- **§3.7 裁决 E-2**:单根事件委托 + 最小运行时(零第三方依赖,JS-5)。
- 门禁:**产物级**锁(扫产物文本,不是扫发射器代码)——
  理由:锁测试必须对**产物**有判别力,对发射器断言等于自证。
- 门禁:E-1 的**负向用例**:组件状态含 `FUNCTION` / `INSTANCE` / `CHANNEL` 时
  必须构建期拒绝,且错误信息指名具体字段路径。

### W6 CLI 驱动 + 端到端 + 收口(约 2 人日)

- `wlwl web build page.wlwt` 子命令(读文件 / 调 R1 / 写三产物)。
- 3 个端到端页面(静态页 / 含交互组件 / 含列表重排动画)。
- 四件套第 1、4 条 + CHANGELOG + `docs/plan/README.md` 状态表同步。

**合计约 17.5 人日。** 相对批次 A(标准库底座)的体量是同一量级,不是小改动。

---

## 5 风险与预置裁决

| # | 风险 | 触发条件 | **预置裁决**(业主可推翻) |
|---|---|---|---|
| R-1 | `@scope` 的 Baseline **日期**与 §2.1 不符(**语义已定案,不再是风险**) | W1 复核发现 | 语义侧无分支可走(S-1..S-4 有一手规范支撑);仅需调整 §2.1 表格与兼容性说明。`@supports` 守卫副本已覆盖老浏览器 |
| R-2 | 行为最小化判定表爆炸式增长(判定逻辑比写 JS 还复杂) | W4 判定表超过约 12 条 | 收缩到 Top-N 原语,其余一律进 JS。**宁可 JS 多,不要编译器复杂** |
| R-3 | `.wlwt` 语法与 wlwl 表达式求值脱节(模板里写不了 wlwl 就算) | W2 出现 | 引入 `{{#let}}` 局部绑定块;不改求值器 |
| R-4 | 纯 R1 解释器在真实页面上超时 | 端到端 > 5 s | 先查数组累积(§3.6);根因是解释器写时复制则登记为新偏差,**不在本计划内修解释器** |
| R-5 | 治理四件套成为主要成本 | W2 实测 > 4 人日 | 砍 W4/W5 范围到最小可演示集,保住 W1–W3 + 四件套完整 |
| R-6 | 状态可序列化约束(裁决 E-1)**过严**,把正当用法挡在门外 | W5 出现真实页面写不出 | 先放宽**具体类型**(如允许 `NULL` 键 / 嵌套 `RESULT`);**不放宽「`FUNCTION`/`CLASS`/`INSTANCE` 不可序列化」这条底线** —— 它是 JS-4 与「产物不含 wlwl 语义」的同一件事 |
| R-7 | 零第三方依赖(JS-5)与惰性 handler 块(裁决 E-3)冲突 | 后续版本实现 E-3 时 | E-3 的 `import()` **指向自身产物内的相对路径**,不算第三方依赖;若必须引外部 loader 则该条承诺需业主重新裁决 |

---

## 6 文档落点

| 落点 | 内容 | 里程碑 |
|---|---|---|
| `docs/stdlib/wlwl-stdlib-spec-v0.11.md` | §11 之后新增 `std.web` 章节(成员表 / 语义 / 失败行为 / 落地状态) | W1 起,逐里程碑增量 |
| 同上 附录 A | 由 `gen-appendix-a` 生成,**手改无效** | W2 |
| `docs/plan/deviations-v0.11.md` | D11-020 起 | 全程 |
| `docs/plan/README.md` | 状态表新增行 | W6 |
| `CHANGELOG.md` | 按 Q1 裁决落到 `[Unreleased]` 或 `[v0.12]` | W6 |
| `docs/site/examples.md` | 3 个端到端示例 | W6 |

**不在本计划范围**:不动语言规范、不动 ADR、不新增 R0 内建、不改
`wlwl-lexer` / `wlwl-parser` / `wlwl-ast`(R0 变更需独立立项)。

---

## 7 验收门禁

在 §0 棘轮之上追加:

1. **既有 37 套件 / 1860 passed / 0 failed 保持不变**,probe 142 不减。
2. `std.web` 治理四件套齐备,且附录 A 由生成器产出(锁测试双向守护)。
3. **产物不变量 JS-1..JS-5 五条锁测试全绿**;每条配**反向守卫**
   (故意让发射器产出违规产物 → 必须变红 → 还原后全绿)。
4. **裁决 B 产物形状锁**:组件 `<style>` 落在组件根元素内、体被无前置子
   `@scope { }` 包裹、含 `@supports` 守卫副本 —— 以 **golden file** 逐字节对拍;
   并配一条负向守卫(把 `<style>` 移到根元素外 ⇒ 必须变红)。
5. **`&` 负向门禁**:组件 `<style>` 中出现 `&` 时给构建期警告
   (§2.1.1 记录了 `&` 特异性的来源冲突,发射器不依赖它)。
6. **JS-5 零第三方依赖锁**:扫产物确认除自带运行时外无任何 `import` /
   CDN / `node_modules` 引用,并配一条负向守卫(塞一个外部 import ⇒ 必须变红)。
7. **裁决 E-1 可序列化锁**:每个可序列化类型一条**正向**用例;
   `FUNCTION` / `CLASS` / `INSTANCE` / `CHANNEL` / `TASK` 各一条**负向**用例,
   且错误信息必须指名字段路径。
8. 3 个端到端页面产物经**人工审阅**并把审阅结论写进 `docs/site/examples.md`。
9. §3.6 的性能实测在真实端到端页面上复跑一次,数字写进规范落地状态段。
10. `cargo doc` 无断链(v0.11.0 首次实跑 `cargo doc` 曾红,见 D11-015)。
11. **不新增**任何未经单独立项的 R0 内建。

---

## 8 与既有决策的关系

| 既有 | 关系 |
|---|---|
| ADR-0021 §0.1 分层 | 本计划全部实现落 R1,是规则的**直接适用**而非例外 |
| ADR-0021 §0.2 kernel 私有通道 | 仅在 §3.5 逃生口可能用到;三条 kernel 纪律照旧 |
| ADR-0023 稳定性政策 | v0.x 段:可见面变更必须同批次完成四件套 + CHANGELOG |
| D11-004(`SORT` 回调 `ERR`)挂账 | `std.web` 若用 `SORT` 排序 CSS 声明,会撞上这条。**已知且可绕**:W4 的判定表用固定顺序,不依赖 `SORT` 的比较器 |
| D11-012(R1 数组二次) | §3.6 的设计约束直接来自这条实测 |
| 语言规范 §17.4 预留 tag | **只读不写**(§2.5) |
| `std.json`(R2,附录 A 已有) | **复用**:裁决 E-1 的序列化直接调 `STRINGIFY`,不自造编码 |
| 批次 B(`YIELD` 续体保存) | 无冲突,W1–W3 可并行 |

---

## 9 未决问题 —— **全部已裁决**

五个问题在 2026-10-01 全部落定。本节保留裁决内容以便追溯,详细设计见括注章节。

**Q1 —— `std.web` 落哪个版本号?** ✅ 落 **v0.11.1**;v0.11.1 未完工,
**TAG 由业主下令后才能打**(§0 版本与 tag 门禁)。原「v0.12 / 预览」三选项作废。

**Q2 —— 命名空间是一个还是几个?** ✅ **单个 `std.web`**。

理由:三条共推导(§3.3)要求 HTML / CSS / JS **同属一个模块才能互相看见**;
拆开就退化成几个互不知情的库,旗舰特性无法演示。成员名按关注点区分
(`ELEMENT` / `COMPONENT` / `TOKEN` / `RULES` / `PAGE` / `EMIT_HTML` /
`EMIT_CSS` / `EMIT_JS` / `RENDER`),且不与全局内建重名(§0 纪律)。
若成员面后续膨胀,再按 §0.4 的 1.0 段规则拆分(「新增命名空间不受限」)。

**Q3 —— `@scope` 的 scope root 自身是否被其内部样式匹配?** ✅ 已用 W3C 一手
规范结清:**在作用域内,但裸选择器匹配不到它**,必须显式写 `:scope`(§2.1.1)。
据此 §3.3 裁决 B 改为「`<style>` 行内重定位 + 无前置子 `@scope`」。

**Q4 —— 是否要做 resumability(Qwik 路线)?** ✅ **分层做,不做完整的**;
完整分析见 **§3.7 裁决 E**。三句话版本:

1. 原计划把裁决 C 与 resumability 写成「二选一的两套裁剪哲学」是**错的** ——
   二者**串行且正交**:前者按能力裁(哪些绑定需要 JS),后者按状态裁(已保留的
   绑定如何少执行)。
2. **关键洞察**:裁决 C 配构建期渲染 ⇒ **DOM 本身就是序列化状态**。
   resumability 最贵的那块(组件状态序列化进 HTML)对静态部分**冗余**。
3. 所以 v0.11.1 只做 **E-1 状态可序列化检查** + **E-2 单根事件委托**;
   E-3 惰性 handler 块、E-4 完整 resumability 留后续。**E-1 必须现在做** ——
   它是类型级承诺,晚定即 breaking(§3.7 给了三条理由)。

**Q5 —— 产物是否要求零运行时依赖?** ✅ **写成规范性承诺**,落为产物不变量
**JS-5**(§3.4)。措辞须精确:是**零第三方依赖**,不是零运行时 —— 产物**自带**
最小运行时(事件委托 / handler 表 / view-transition 命名 / 状态存取)。
门禁扫产物确认无任何外部 `import` / CDN / `node_modules`(§7 第 6 条)。

---

## 9.1 已裁决事项(留档)

| 原编号 | 裁决 | 日期 | 落点 |
|---|---|---|---|
| Q1 | `std.web` 落 **v0.11.1**;**TAG 由业主下令后才能打** | 2026-10-01 | §0 版本与 tag 门禁 |
| Q2 | **单个 `std.web` 命名空间**,不拆 | 2026-10-01 | §1.2 + §9 Q2 |
| Q3 | 按最优解执行,用 W3C CSS Cascade 6 §2.5 一手规范结清 | 2026-10-01 | §2.1.1 + §3.3 裁决 B |
| Q4 | **分层**:v0.11.1 做 E-1 + E-2;E-3 / E-4 留后续;完整 resumability **否决** | 2026-10-01 | §3.7 裁决 E + §1.2 |
| Q5 | **零第三方依赖**写成规范性承诺(产物不变量 JS-5) | 2026-10-01 | §1.1 + §3.4 + §7 |

---

## 10 一句话总结

**不要造框架。** 2025 年的 CSS 平台(`@scope` / `@layer` / `@property` /
`:has()` / container query / view transitions)已经吸收了框架的大部分职责。
`std.web` 的价值不在「能生成 HTML/CSS/JS」,而在**用一棵节点树同时驱动三个发射器,
让编译器决定每一处动态该由 CSS 还是 JS 承担** —— 从而让 JS 产物按构造最小。
这正好落在 R1 的能力范围内,也正好避开 i64/码点/ERR 三条保真鸿沟。
