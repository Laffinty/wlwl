# ADR-0020 — Gradual Static Contracts (编译期 `gradual_typing` 静态 pass)

| | |---|
|---|---|
| **Status** | Accepted(2026-09-27;Step 0 定稿 → Step 1/2 落地后转正) |
| **Date** | 2026-09-27 |
| **Deciders** | Li (project lead) |
| **Related** | **ADR-0010**(本 ADR 有意推翻其「不做静态检查」方向,限缩范围)、ADR-0011(人日纪律先例)、`docs/history/20260915-22.md` §3 / §6.1、`docs/history/20260915-22.md` §4.1 P0-1、spec v0.9 §2.4 / §2.7 / §5.2 / §9.4 |

## Context and Problem Statement

WLWL 是动态类型语言:类型属于值而非名字(spec v0.9 §2.1)。类型注解从
v0.3 起就存在于语法里,但**长期没有语义**:

- `wlwl-ast/src/lib.rs:133` 已有 `TypeAnnotation { expr, span, text }` 结构,
  `lib.rs:181` / `lib.rs:360` 已有形参与返回值注解槽位 —— 静态层的挂载面
  一直存在,只是没人消费;
- `wlwl-parser/src/lib.rs:2366-2473` 已把注解解析成结构化
  `TypeExpr::{Ident, Array, Generic}`(`ARRAY[T]` → `Array`,
  `DICT[K,V]` / `OK[...]` → `Generic`),不是字符串;
- 唯一的消费者是运行时 `wlwl-eval/src/lib.rs:8586-8600`:按 `TYPE` 名做
  **顶层形状比对**,大小写不敏感,失配抛 `E0033`;并且代码注释明写
  *"Nested generic / array element matching is deliberately deferred"*;
- `wlwl-cli/src/main.rs:51-58` 的 `check` 子命令注释就是
  *"Only check (parse) without execution"* —— `check` **只 parse**,
  没有语义层。

ADR-0010 当时面对的是「v0.3 §2.7 要求全程序类型推断」这个诉求,结论是
**否决**,理由记为 *"requires a static type-checker; triple the work"*,
并选项了运行时 transient cast。该结论对「全程序推断 / HM」是成立的,
但它顺带把「静态检查」这个方向整体关掉了,而 v0.9 结束时的真实状态是:
**注解语法齐备、语义全空、`check` 名不副实**。

六方独立评估(《路线》§9)对「类型薄弱是最大缺口」给出 6/6 一致,
且《路线》§2.2 明确记载:类型要跨包验证时,「推论『必须配包管理』不成立
—— 类型契约在 `ModuleLoader` 边界即可验证」。因此重开静态方向**不需要**
触碰已被业主硬约束彻底否决的包管理议题。

## Decision Drivers

- **挂载点必须现成**。新建静态子系统若要侵入 `wlwl-eval`(单文件
  **21,765 行**、789 内嵌测试),风险与回归面都不可接受。新语义必须进新
  crate / 新 pass。
- **默认零破坏**(build plan §3 S1)。v0.10.0 的对外承诺是「任何 v0.9 程序
  在 v0.10.0 上默认可观察行为不变,例外:无」。静态 pass 必须默认关闭。
- **运行时路径不得回归**。`strict_types` / `E0033` 已有测试锁
  (build plan S3),运行时 transient cast 决策(ADR-0010 的一半)**继续有效**。
- **人日纪律**(ADR-0011 先例:1 天的 MVS 胜 1–2 周的 PubGrub)。写不出
  挂载点与人日的项一律降级,不立项。
- **词法/语法面尽量不动**。v0.9 lexer 57 个 `TokenKind` 已能表达全部 P0-1
  需求;静态检查是**语义层**工作,不应借机扩张语法面。
- **包管理零动作**。`wlwl-toml` 的 manifest/lock/MVS **冻结**,至多增加
  一个 `[features]` 只读键。

## Considered Options

### A. 维持 ADR-0010,不重开静态检查

**Pros**:零回归风险;`E0033` 运行时检查已覆盖常见失配;不新增 crate。
**Cons**:`check` 永远只是 parse,`E0033` 只能在**运行到**该调用点时才报
——一个从未被调用的函数,它的签名错误永远不会被发现;嵌套泛型与数组元素
匹配被代码注释显式推迟且无 ADR 承接;六方一致的最大缺口原地不动。

### B. 重开为全程序类型推断(HM / let-多态)

**Pros**:诊断能力最强;业界共识路线(ML / TypeScript / Kotlin)。
**Cons**:**这正是 ADR-0010 已经否决的方案**,理由「triple the work」今天
依然成立;动态语言上做全程序推断需要大量逃逸口,`Dynamic` 回退的边界
反而更难解释;对人日预算(13–18)是数量级的超出。**本 ADR 不复活此项。**

### C. 重开静态方向,但范围限缩为「边界契约 + 期望类型向下传播」 ✅

新建独立 crate 承载编译期静态 pass,只做两件事:

1. **边界契约检查** —— 在已有注解槽位上,把「运行时顶层 `TYPE` 名比对」
   提升为**编译期**检查(嵌套泛型、数组元素、函数签名);
2. **期望类型向下传播** —— 局部、上下文驱动(字面量 / 容器字面量 /
   IF-MATCH 合流 / CALL 实参),**不做** HM、不做 let-多态,未标注处一律
   回退 `Dynamic`。

**Pros**:复用全部现成挂载面(AST 槽位 + `Check` 子命令 + 已有
`TypeExpr`);`Dynamic` 回退使「不误报」成为可验收目标;默认关闭即零破坏;
人日落在 13–18 的可核区间。
**Cons**:能力弱于 B —— 诊断覆盖率必然不全;需要显式承认「类型系统是
渐进的、默认不信任的」这一前提,写进 spec。

### D. 顺带把效果半落地、宏、Variant 一起收口

**Pros**:一次发版解决更多悬空承诺。
**Cons**:**范围失控**。v0.9 已经因为「四件套齐上」留下 Known limitations;
《路线》§5 明确否决宏 / Variant / 用户效果 / 类型收窄进本版。**否决。**

## Decision Outcome

Chosen option **C**。决策条目:

1. **新建独立 crate**(暂名 `wlwl-types`,名称可在 Step 1 微调),
   **不侵入** `wlwl-eval`。所有新语义进新 crate 的新 pass;`wlwl-eval`
   只保留既有运行时 `E0033` 路径不动。
2. **范围限缩**为「边界契约检查 + 期望类型向下传播」,**不是** ADR-0010
   当年否决的 HM / 全程序推断。A5 流敏感类型收窄**推迟**。
3. **`gradual_typing = off | warn | error`,默认 `off`**。落在
   `wlwl.toml` 的 `[features]` 内 —— 该表是不透明
   `BTreeMap<String, toml::Value>`(`wlwl-toml/src/manifest.rs:33-37`),
   加键**无需 schema 变更**、`wlwl-toml` 结构零改动(满足 S5 无包管理动作)。
   运行时 `strict_types` / `E0033` **保留**,两条通道并存且语义分离。
4. **人日预算:P0-1 = 13–18 人日**。超支时削减 A6′ 首批条目与 A2′ 传播
   深度,**不延期** `gradual_typing` 主开关本身。
5. **包管理零动作**。`wlwl-toml` manifest/lock/MVS 冻结;`E0041` /
   `E0045` 触发语义不变;不新增包管理类错误码。

### 与 ADR-0010 的边界(必须明文,防历史歧义)

- **被推翻的**:ADR-0010 中「不做编译期静态检查」这一**方向性结论**。
- **未被推翻、继续有效的**:ADR-0010 的**运行时 transient cast 决策**
  (`strict_types` 路径、`E0031` / `E0033`)完整保留,测试锁不动。
- **本 ADR 不授权**:全程序推断 / HM / let-多态、类型收窄与 guard、
  Variant / Open Row、ownership / lifetime / borrow 检查。

### 表面影响(词法 / 语法,经代码核验)

静态 pass 本身**不要求任何词法或语法改动**:

- v0.9 lexer 全部 **57** 个 `TokenKind` 已足够;注解语法
  `x: TYPE`、`FUN(params): TYPE, body`、类型参数用**方括号**
  `ARRAY[T]` / `DICT[K, V]` 均已实现。
- `A4` 点名的 `OPTION` / `RESULT` 今天就能被解析成 `TypeExpr::Ident` ——
  它们是**静态层的名字白名单**问题,不是文法问题。
- 唯一潜在新增是函数类型 `FUN(...)`(AST 注释
  `wlwl-ast/src/lib.rs:83-84` 标为 "reserved for v0.4",至今未实现)。
  lexer 无 `Arrow` token,`->` 会被切成 `Minus` + `Gt`。建议 P1-2 按
  《路线》允许的「限形」把函数类型后置;若坚持引入,须为词法层新增一条
  `arrow` 终结符并同步 spec §1.5。
- **须在 spec v0.10 补写的事实**:v0.9 spec 全文**没有定义 `Type`
  产生式** —— §5.2 只写了 `name: Type` 这个元符号,§5.1 的 `Function`
  产生式也未收录返回值注解。即类型注解文法目前只存在于实现,不是规范。
  A4 是第一次必须把它写进 spec 的时机。**且必须照实写出三条不对称**:
  (a) 容器类型用方括号;(b) parser 对 `ARRAY` 特判强制要求方括号
  (`wlwl-parser/src/lib.rs:2407-2410`),裸 `ARRAY` 报 `E0010`、写不出来,
  而 `DICT` / `OPTION` / `RESULT` 裸写能解析成 `Ident`;(c) 函数类型
  `FUN(...) -> U` 本版**仍未进语法**。
- **D-5 拍板带来的第二处语法面扩张(2026-09-27 补记)**:决策 D-5 选
  「最小显式约束 `T: Comparable` 级」,这要求类型产生式接受 `:` 与约束名
  ——`parse_braced` 当前只接受 `,` 与 `]`。**这是 v0.10 唯一一处真正的
  语法面改动**,且它不在上面「静态 pass 不要求任何词法或语法改动」的
  原始预估里。因此 §1.4 运算符表 / 关键字表**除该处外零变化**,§12 保留
  形式(v0.9 为空表)仍为空 —— 这条已写进 build plan §12.4 出口条件第 7 项。


## Consequences

Positive:

- `check` 从「只 parse」升级为「parse → 可选静态 check」,挂载点现成
  (`wlwl-cli/src/main.rs:51-58`);
- 已有 AST 注解槽位(`wlwl-ast/src/lib.rs:133,181,360`)从死代码变为有
  语义的消费点,不必扩 AST 槽位;
- 运行时 `E0033` 通道与编译期通道并存,默认关时 v0.9 程序行为不变
  (S1 / S3);
- 注册表结构化签名(A6′)有了第一个真实消费者,不再是纯文档整理
  (`wlwl-eval/src/registry.rs:48` 的 `signature: &'static str` 升级为结构化,
  文档列保留);
- `Dynamic` 回退给出可验收的「不误报」目标,避免半成品类型系统制造噪音。

Negative:

- **诊断覆盖率必然不全**。这是渐进的、可关闭的检查,不是完备类型系统;
  spec 必须显式声明这一点,否则用户会误期望「开了就等于 TypeScript」。
- 新增一个 workspace 成员 crate,`impl/Cargo.toml` 需接线;`gen_appendix_g`
  需随 A6′ 同步(A6′ 为分批,不承诺一次做完 110 条)。
- 「编译期 `gradual_typing`」与「运行时 `strict_types`」是两个同名不同层的
  概念,spec §2.4 / §2.7 必须分开写清,否则成为长期歧义源。
- `wlwl-eval` 单文件 21,765 行的风险**不因本 ADR 缓解**;新 pass 只做到
  「不侵入」,存量单体化仍是已知负债。

## Ratification Status

**Status: Accepted**(2026-09-27)。build plan §10.1 Step 0 的两个交付物均已
落地:本 ADR 定稿(commit `c535818`),Step 1 `wlwl-types` crate(commit
`859400b`),Step 2 `gradual_typing` 开关与 `check` 接线。

实现级决策点(build plan §10.2)已由用户拍板 4 个:

| 决策 | 裁决 | 落地位置 |
|---|---|---|
| **D-1** 静态诊断码段 | 新建 `E0110+` / `W0110+` 段 | Step 2:`E0110`-`E0112` / `W0110`-`W0112` 已注册 |
| **D-2** 签名文件载体 | 旁路 `*.wll.sig`,与源同目录 | Step 6(未开工) |
| **D-3** `SEALED` 语法 | 前缀调用 / 模块头 `SEALED(...)` | Step 6(未开工) |
| **D-5** 泛型约束深度 | 最小显式约束 `T: Comparable` 级 | Step 9(未开工) |

**D-4**(Effect 半落地收口)与 **D-6**(LSP 薄壳范围)仍未拍板,分别只卡
Step 11(余力项)与 Step 10,不阻塞 P0 主线。

### Step 1 / Step 2 的实测结论(回填本 ADR)

1. **`INTEGER -> FLOAT` 必须算可赋值**。ADR-0010 明写这是 *silent upcast,
   even under strict*;静态层若不认这条安全方向,`gradual_typing = "error"`
   会拒掉运行时认为合法的程序,直接违反「不误报」验收项。反方向
   `FLOAT -> INTEGER` 仍然拒绝。
2. **函数返回值要查「块尾表达式」,不只查显式 `RETURN`**。spec §4.4 规定
   括号序列的值即最后一个表达式的值,`FUN((a): STRING, 42)` 这种最常见
   的写法根本不含 `RETURN`,只查 `Return` 节点会整类漏报。
3. **`Type` 产生式在 v0.9 语法里比想象的更窄**:parser 对 `ARRAY` 特判,
   强制要求方括号(`wlwl-parser/src/lib.rs:2407-2410`),裸 `ARRAY` 注解
   报 `E0010`、写不出来;`DICT` / `OPTION` / `RESULT` 裸写却能解析成
   `Ident`。spec v0.10 补写 `Type` 产生式时必须照实写出这条不对称。
4. **A3 抓不到内建调用**。注册表结构化签名归 A6′(Step 5),在那之前内建
   返回类型不可知、一律 `Dynamic`。所以 A3 只能抓**用户定义的带注解
   函数**边界 —— 这是范围裁剪,不是缺陷,但 spec 与 CHANGELOG 必须如实
   说明,不能让用户误以为「开了就等于 TypeScript」。

> build plan §10.1 的 Step 0 正文写「决策点 D-1..D-4」,而 §10.2 表格
> 实列 D-1..D-6。以表格为准,共 6 个。


## References

- `docs/history/20260915-22.md` §3(P0-1 立项单)/ §6.1(本 ADR 草案)/ §10.1(Step 0–15)/ §10.2(决策点)/ §11.1(人日)
- `docs/history/20260915-22.md` §2.2(真缺口)/ §4.1 P0-1 / §5(明确不做)/ §9(六方对照)
- ADR-0010(`strict_types` runtime check via transient cast)—— 方向被本 ADR 限缩推翻,运行时决策继续有效
- ADR-0011(MVS over PubGrub)—— 人日否决纪律先例
- spec v0.9 §2.1(值域)/ §2.4(类型注解)/ §2.7(`strict_types`)/ §5.2(形参注解)/ §9.4(清单特性表)
- 代码锚点:`wlwl-ast/src/lib.rs:83-100,133,181,360`;`wlwl-parser/src/lib.rs:1126-1176,2366-2473`;`wlwl-eval/src/lib.rs:8586-8600`;`wlwl-eval/src/registry.rs:46-48`;`wlwl-cli/src/main.rs:51-58`;`wlwl-toml/src/manifest.rs:33-37,361`;`wlwl-error/src/lib.rs:164-166`(现有码止于 E0102)
- 基线实测(2026-09-27,`wip0.10` @ `635a768`):`cargo test --workspace` = **1517 passed / 0 failed**
