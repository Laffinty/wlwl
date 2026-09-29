# Changelog

All notable changes to WLWL (the implementation) are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

> **Note.** The compiler version is **independent of the language spec version**.
> The language spec lives in `docs/standard/` and is identified by version + name.
> This file tracks the **compiler / tooling** releases. The current spec is
> **v0.10** (`docs/standard/wlwl-spec-v0.10.md`); archived specs live under
> `docs/history/` (`wlwl-spec-v0.9.md`, `wlwl-spec-v0.8.md`,
> `wlwl-spec-v0.7.md`, `wlwl-spec-v0.6.md`).

## [v0.10.1] — 2026-09-29

Spec: **wlwl-spec-v0.10**(`docs/standard/wlwl-spec-v0.10.md`)——
本版**不改规范版本号**,改的是规范文本与实现对齐。

**本版是一次「让文档与实现说同一句话」的版本**:13 个 Step、60 余条修复,
其中最大的一块不是新功能,而是**发现三份副本里最不可能出错的那一份正在说假话**
(见 Fixed 的第一条)。零新增内建、零新增语法、零关键字变化。

门禁:`cargo test --workspace` **1809 passed / 0 failed**(v0.10.0 发布时 1744,
`+65`);`cargo fmt --check` 0 diff;`clippy -D warnings` 0;`cargo doc` 0 warning。

### Fixed

- **附录 G 的签名在 spec / 注册表 / mirror 三份副本间分了 37 条,其中
  `BUILTIN_REGISTRY` 有 12 处说的与实现相反**(D10-012)。

  「三份副本 + 只锁条目数」这个结构让人默认手维护的 spec 是漂移方。逐条对着
  `wlwl-eval` 的 dispatch 实现核实之后结论**相反**:

  | 条目 | 注册表原写法 | 实现实测 |
  |---|---|---|
  | `SUB` | `end?` | 第三参是**长度**(`SUB("hello world", 0, 3)` → `hel`) |
  | `INT` / `FLOAT` | `-> OK(...)` | 返回**裸**值(`INT("42")` → `42`) |
  | `INDEX_SET` | `-> NULL` | 返回新容器(`→ [99, 2, 3]`) |
  | `WHILE` | `-> v` | 恒返回 `NULL` |
  | `AT` | 2 参 | 强制 3 参,2 参报 `E0022 expects 3 argument(s)` |
  | `IMPORT` | 3 参 | 只接 2 参,3 参报 `E0011 expected ')' got Comma` |
  | `THIS` | 裸 `THIS` | 裸写报 `undefined name`,必须 `THIS()` |
  | `==` / `!=` | `=(a, b)` / `!(a, b)` | 签名列各掉了一个字符 |
  | `MODULE_REF` | `-> MODULE` | `MODULE` 不是类型;实测 `TYPE(...)` → `DICT` |
  | `+` | `INTEGER / FLOAT` | 漏了 STRING 拼接与 ARRAY 合并 |
  | `INDEX` | 缺 1-based | `INDEX([7,8,9], 8)` → `2` |
  | `INDEX_GET` | 只有 `E0031` | 实为三码 `E0031` / `E0036` / `E0037` |

  `SUB` 记的是 **v0.8.0** 的 end-index 语义,而 **v0.8.1 D8-010 已经推翻了它**;
  `MODULE_REF` 则是 R10-046 **只改了 spec、漏了注册表**,于是自动生成的 mirror
  一直在跟着说假话 —— 而 mirror 本该是最不可能出错的那一份。

  **本轮是先修注册表、再让 spec 跟上**,方向永远以实现为准。新增
  `tests/spec_appendix_g_sync.rs` 三条锁(签名逐条相等 / 条目数相等且无重名 /
  `||` 行必须切出恰好 8 列),四处变异全部确认转红。

- **`ARRAY(items...)` / `DICT(pairs...)` 是三份副本**共同**虚报的幻影 API**
  (D10-013)。实测两者都报 `E0020: undefined name`;零参形式由 `eval_call`
  直接拦截。三份**同样错**,于是彼此"一致",条目数锁照样绿。

- **`BUILTIN_REGISTRY` 的 markdown 生成器两个缺陷**:`generate_appendix_g_md()`
  声称注册表「已按 group 排序」——**但它没排**,于是分组表头在
  `docs/appendix_G.md` 里重复七八次且 ARRAY 表头下挂着 DICT 的条目;`||` 的
  竖线**没转义**,那一行对任何 markdown 表格解析器都是 9 列而不是 8 列
  (spec 自己那一份一直是转义过的,只有生成器忘了)。

- **`SEALED` 从来没走过 CLI**(只有 `wlwl-types` 的 Rust 单测)。新增
  `conformance/sealed_surface/`,做成**只有 SEALED 能逮到**的形状:签名里
  **声明了**被密封面漏掉的名字。四格矩阵 + 变异验证。

- **`match_exhaustiveness` 的独立性从未被验**。它缺省跟随 `gradual_typing`,
  而全部夹具走的都是「只写 `gradual_typing`」那一条路。新增六格矩阵,证明两个
  开关**双向都真正独立**。

- **`wlwl-skill`**:幻码 `W0014`(注册表里根本不存在)换成 `W0012`/`W0013`/`W0015`;
  章节号 §5.2.1 → **§2.6**;「两种括号形式都是 parse error」这句**错的**结论
  改成实测五行对照表。

- **`cargo doc` 两处告警清零**(`manifest.rs` / `main.rs` 的 doc 链接指向私有项)。
- **`wlwl fmt --check` 不再把 CRLF 判成规范偏离**(D10-018)。实测
  `PRINT("x")\n` → rc=0,而同样的内容写成 `PRINT("x")\r\n` → rc=1 + `W0053`。
  仓库的 `.gitattributes` 只给 `*.rs` 定了 `eol=lf`,`.wll` 夹具没有规则,
  于是 **Windows 检出的工作区里每一个 `.wll` 都过不了 `--check`**,Linux 上
  全过 —— 同一个 commit 在两台机器上结论相反。由 CI 的 Windows job 抓出
  (ubuntu / macOS 看不见)。「CRLF 文件偏离 §16.3」这句话本身是错的:规范
  规定的是 token 之间的空白与换行**布局**,没规定换行用哪个字节表示。
  修法是比较前把两侧 EOL 归一化成 LF,裸 CR 一并归一。
  `wlwl fmt <file>` 的 stdout 输出仍恒为 LF,未改。

### Changed

- **静态层一批「以前静默、现在响亮」的收紧**(D10-017):`R10-010` 清单缺
  `[package]` 时静态层不再整条静默失效(此前连 `W0001` 都没有)、`R10-020`
  `wlwl fmt` 不再产出不可重解析源码、`R10-021` `SEALED` 在非声明位回退成普通
  调用、`R10-025` 类型约束右嵌套 `T: A: B` 报 `E0010`、`R10-030` `.wll.sig`
  尾随垃圾不再被吞成类型名、`R10-086` `E0110` 不再掩盖下游 `E0111`/`E0112`。
- **静态层 12 条小改**:`E0110` 注解不符时绑定取实际类型、签名尾随垃圾、
  类型诊断补源码行、删 `TypeArityMismatch` 死变体并撤掉对应承诺、
  类型层 `is_ident` 改用 Unicode 属性(支持中文标识符)、约束右嵌套、
  泛型变量跨形参统一等。
- **规范文本 12 条按实测重写**(R10-040 ~ R10-052):§2.6 三条事实、§0.1 冻结
  范围声明与附录 D 申报表、四处断裂交叉引用、`NameItem` 两处定义不同与
  `ModuleDecl` 悬空、附录 G 计数分栏、§16.1 与 §17.4 直接矛盾、§17.1「性能」
  条款给出可判定口径、五条无法写成验收测试的条款逐条给可判定形式。
- **附录 A 的文法第一次被机器读过**(R10-050):新增五条锁测试(悬空非终结符 /
  重复定义 / §9.2↔附录 A 一致 / 关键字入口齐全 / `ModuleDecl` 可达),
  一上来就逮住两处漂移。读不到规范文件时 **panic 而不是 skip** —— 一个
  「文件不在就跳过」的校验器,正是这条测试要消灭的那种失败模式。

### Added

- **并发金标准夹具**:无缓冲通道「生产者先 `SPAWN`」的夹具 —— 正是它逃过了
  原来的 CI。
- **OOP 身份 conformance 夹具**(`conformance/oop/`):自反 / 对称 / 传递三律。
- **probe 套件进 CI**(101 条):v0.10.0 动态 REVIEW 用的 97 份 probe 整套搬进
  仓库并由 `cargo test` 驱动。它们的价值只有一个:**防回归** —— 下面
  「与规范文本矛盾、但实现就是这样跑」的行为被逐字钉死,下一次改动不会再把
  它们悄悄换掉。另有 3 条是本轮新加的判别用例。
- **静态契约夹具按 README 约定拆开**(9 个目录,一份夹具一个契约):修之前
  `contract_of` 读到第一段就 `break`,8 个 `// expects:` 块里有 **7 条是死的**,
  `E0111`/`E0112`/`E0116`/`W0117` 在夹具层面**零断言**。拆分后 7 条全部复活。
- **`impl/examples/` 接进 CI**:两个随包示例此前跑不起来(小写 `true`、
  `STRINGIFY` 给了两个参数、`build/` 目录不存在)。
- **新增错误码 `E0064`**(通道活锁护栏,见下)。

### Known limitations

- **`YIELD` 没有续体保存**(D10-015 之外的规范侧申报)。规范 §17.1 那张表
  **5 行 5 行与实测相反**,本版已如实降级并在附录 D 申报:

  | 形态 | 实测 |
  |---|---|
  | `LET(x, YIELD())` | 整条 `LET` **不完成**,`x` **根本没绑定** → `E0020` |
  | `IF(TRUE, YIELD(), 42)` | `NULL` |
  | `[1, YIELD(), 3]` | `NULL` |
  | `WHILE` / `FOR` **循环体内**的 `YIELD` | 循环**只跑一轮** |
  | `LET(y, YIELD); y()` | `E0020: undefined name 'YIELD'` |

  可用的规律只有两条:挂起点**之前**建立的绑定在恢复后可见;`YIELD()` 的值
  **被外层表达式消费**时,那个**最外层表达式**整体塌成 `NULL` —— 塌的是整条
  表达式,不只是 `YIELD` 所在位置。

  **本版不动实现**(裁决 D-3 = 拆两半)。兑现续体要改运行期栈模型,会直接推翻
  「运行期语义与 v0.9 逐条一致」,推 v0.11 单独立项。两条 probe 把实测钉死 ——
  **它们转红是预期信号**,意味着续体已实现。

- **无缓冲通道上同一对任务只能交接一次**:`E0064` 活锁护栏(按任务对计数,
  阈值 2,只对 `capacity == 0` 生效)。
- **缓冲通道上的「发 N 条收 N 条」循环挂死**:段重跑模型限制,**修复之前就挂**,
  非本轮引入;护栏刻意不覆盖它。需段内恢复才能根治,属 v0.11 范畴。
- 非末条顶层语句的 `ERR` 仍被静默丢弃(既有,与 v0.10 无关)。

### §0.4 Consistency (v0.10.0 → v0.10.1)

| 一致性项 | v0.10.0 | v0.10.1 |
|---|---|---|
| 错误码 | 74 激活 | **75 激活**(`+E0064` 通道活锁护栏;其余逐个不变) |
| 警告码 | 23 | **23 不变** |
| 内建数 | 110 | **110**(零新增、零改名;附录 G 表体 26 行按实测重写,见 Fixed) |
| 关键字表 | 14 | **14 不变** |
| spec 版本 | v0.10 | **v0.10 不变**(改的是规范**文本**与实现对齐,不是改版本) |
| 锁测试 | 1744 | **以 CI 的 `cargo test --workspace` 为准**;v0.10.1 收口实测 **1809 passed / 0 failed** |

**改变既有程序可观察行为的三项**(必须按破坏性变更对待,详见
`docs/history/deviations-v0.10.md`):

| 项 | 之前 | 现在 | 登记 |
|---|---|---|---|
| **`CLASS` / `INSTANCE` 相等** | `==` **恒为 `FALSE`**,包括 `x == x`(破坏自反律) | 按对象身份比较 | D10-014 |
| **无缓冲通道 rendezvous** | 生产者先 `SPAWN` **必假死锁** | 顺序无关;同一对任务第二次交接报 `E0064` | D10-015 |
| **`IMPORT` 的签名类型** | 只被解析展示,**从不参与判定**,错实参零诊断 rc=0 | 参与检查,报 `E0110` / `E0111`(**无清单时仍是 v0.10 行为**) | D10-016 |

**一批「以前静默、现在响亮」的收紧**(D10-017):`R10-010` / `020` / `021` /
`025` / `030` / `086`。没有任何一条是放宽 —— 但原先能编译通过的写法现在会
编译不过。



## [v0.10.0] — 2026-09-28

Spec: **wlwl-spec-v0.10** (`docs/standard/wlwl-spec-v0.10.md`).
v0.9 archived at `docs/history/wlwl-spec-v0.9.md`.

**本版是「静态契约」版:运行期语义与 v0.9 逐条一致。** 新增能力全部**默认关闭**
(`[features] gradual_typing` 缺省 `off`,`off` 时整条静态链路不执行);唯一的
**语法面**扩张是类型注解里的显式约束 `T: Comparable` 与 `SEALED([...])` 公开面
声明,两者都不改变既有程序的行为。下面每条都标了「默认关 / 常开」。

### Added

- **编译期静态契约层**(默认关):`[features] gradual_typing = "off" | "warn" |
  "error"`。注解失配 / 调用失配 / 返回失配分别发 `E0110`–`E0112`(warn 档
  `W0110`–`W0112`)。与运行期 `strict_types`(默认 `false`,`E0033`)**正交**:
  可同时开启,先静态拦、再运行时兜底。覆盖边界如实声明:**只**检查用户定义的、
  带注解的函数边界;未结构化内建的返回类型落 `DYNAMIC` 而不报。
- **模块契约**(默认无):可选旁路签名文件 `<module>.wll.sig` 记录对外承诺的
  名字与类型,三向检查 —— 导出 / 导入了签名没声明的名字(`E0113`)、签名声明了
  实现没导出的名字(`E0114`)、签名类型与实现注解冲突(`E0115`)。**无签名 =
  v0.9 行为**,且这是结构性保证:签名是旁路文件,语言里没有内嵌的「这是签名」
  标记语法,不存在「忘了写标记」这回事。
- **`SEALED([...])` 公开面声明**:前缀调用形式(与 `CLASS(...)` 同形),
  **不进入关键字表**,§12 保留集合仍为空。列出的名字就是全部对外可见的名字;
  多导出 / 声明却不导出分别报 `E0113` / `E0114`。运行期是 no-op —— 与 `EXPORT`
  一样,接受 / 拒绝判定不变,只是过期密封面会在编译期被报出来。
- **`wlwl sig` / `wlwl sig-gen`**:从模块反推签名骨架 / 打印签名。
  三处渲染(stdout、JSON 的 `text` 字段、写盘内容)**同源**;`sig-gen`
  **默认不覆盖**已有签名(`--force` 才覆盖);零导出的模块不写文件。
- **`MATCH` 穷尽性 / 可达性诊断**(默认跟随 `gradual_typing`,可单独关):
  非穷尽 `E0116`(warn 档 `W0116`)、不可达子句与永不执行的 default 臂
  `W0117`。`W0117` **恒为警告**且**故意没有** `E0117` 配对 —— 不可达子句通常
  出现在渐进重构的中间态,当硬错会拦下正在写的代码。非穷尽只在
  **default 臂被省略**时报:那种情况下漏掉的分支会**静默得到 `NULL`**。
  可枚举的构造子空间只有 `BOOLEAN` / `NULL` / `RESULT` 三种;整数 / 浮点 /
  字符串 / 数组 / 字典**不报**(报了就是误报)。
- **带显式约束的类型变量 `T: Comparable`**:调用点实例化,**运行期完全擦除**
  (`Value` 模型一个字节没动,变量在运行期不存在)。约束违反在**编译期**报
  `E0111` / `E0110`(复用既有码,本版不新增);运行期不拦 —— 没有
  monomorphization 就没有运行期类型检查器。`Comparable` 的成员集合对着运行期
  比较算子**量出来**:`INTEGER` / `FLOAT` / `STRING`,与 `cmp_op` 逐项一致。
  裸的未知类型名(`x: Foo`)**不是**变量而是不透明类型,所以拼错类型名的
  `E0110` 诊断没有被削弱。
- **`wlwl lsp`**(语言服务器薄壳,零新依赖):`diagnostics` / `definition` /
  `hover` + 由内建注册表驱动的补全。诊断口径与 `wlwl check` **共用同一个
  函数**,编辑器里的划线不会比命令行少。能力只承诺这几项 ——
  `initialize` 的 capabilities 里**没有** `renameProvider` /
  `documentFormattingProvider`。
- **`wlwl interface` / `wlwl schema`**:模块公开面与类型系统的机器可读 JSON
  (spec 附录 E.2 / E.3)。工具据此认识 WLWL 的类型与静态契约诊断码,不必把
  清单硬编码在自己代码里。
- **spec v0.10**:**类型注解文法第一次进入规范**(附录 A 的 `Type` /
  `TypeArg` / `BoundedVar` / `ModuleDecl` 产生式)—— 此前它只存在于实现。
  新增 §2.6 静态类型词汇、§5.2.1 注解的静态解释、§7.4 `MATCH` 静态诊断、
  §9.6 模块契约、附录 E 签名文件与 JSON 契约。
- 偏差登记 `D10-NNN`(`docs/history/deviations-v0.10.md`)。

### Changed

- `wlwl fmt` 不再把**省略的** `MATCH` default 臂补写成 `, NULL` —— 补出来会让
  「作者兜了底」与「作者漏了分支」在源码里长得一模一样,而后者正是
  `E0116` 要报的东西。语义不变(spec §7.1:省略时值为 `NULL`)。
- 签名文件里「`parse_braced` 之后残余记号中出现 `:`」现在报 `E0010`,
  不再被静默吸收成一个名字里含括号的不透明类型。

### Fixed

- **spec §11.3 警告表与实现的码注册表漂移**:表里漏列 6 个会真发出去的 W 码
  (`W0001` / `W0012` / `W0013` / `W0015` / `W0052` / `W0054`),并多列了一个
  注册表里**根本不存在**的 `W0014`(该码位早已被 `W0015` 取代);§10.3 对
  `W0014` 的交叉引用一并更正(实测 `UPPER` / `LOWER` 只转 ASCII,非 ASCII
  原样保留且不发诊断)。
- **规范 ↔ 注册表双向锁测试**:码清单**从规范正文抽取**而非在测试里手抄,
  任何一侧漏改都让 `cargo test` 失败。
- `docs/site/spec.md` 此前还写着「当前规范 = v0.7」(实际已是 v0.9),归档表
  也缺 v0.8 / v0.9。
- 附录 G 生成器里硬编码的规范指针停在 v0.8(改**生成器**,不改产物)。
- `wlwl-std` 的模块目录漏了 `wlwl:std.agent`;并落定 `## Naming` 四条约定 +
  守门测试。

### Known limitations

- **`Effect::MethodCall` / `Effect::ProtocolViolation` 是保留 tag,本版不
  产生**(决策 **D-4** = 规范明文化)。方法调用是**同步直落**的;协议顺序
  违规以 `E0051`、协议已结束仍被调用以 `E0050` **当场**报告(已实装)。
  两个 tag 作为**代数效果后端迁移的预留 tag 面**保留(spec §17.4 末段 /
  ADR-0019 决策 #5),使后端迁移时每个 tag 映射一条 `suspend` 指令时不必
  回头改 tag 面。**实现不得依赖它们产生任何可观察行为**。
- 类型注解的**箭头形式 `FUN(...) -> U` 与尖括号形式都不可达**:会被解析成
  一个名字里含括号的不透明类型,不报错也不生效。spec §2.6 已如实写明
  「本规范不承认这两种形式」—— `D10-005`。
- 数组 / 字典的**穷尽性不判定**:`[1, _]` 与 `[_ , 2]` 的并集能盖住
  `[_, _]`,而任一行单独都盖不住,需要完整的元数分解 —— `D10-006`。
- 泛型**没有量词语法**,同一个变量在每个出现位置都要重写 `T: Comparable`
  —— `D10-008`。
- `wlwl fmt --check` 与句末分号:规范形式对单语句程序不输出 `;`,所以任何带
  `;` 的单语句文件都会报 `W0053`(既有行为,无泛型的普通程序同样)。
  —— `D10-009`。
- 静态层抓不到未结构化内建的返回类型(首批覆盖 60 / 110 条),这些调用
  落 `DYNAMIC` 因而不报 —— 范围裁剪,不是缺陷。

### §0.4 Consistency (v0.9.0 → v0.10.0)

| 一致性项 | v0.9.0 | v0.10.0 |
|---|---|---|
| 错误码 | 67 激活 | **74 激活**(`+E0110`–`E0116`;运行时码**逐个不变**,包管理码零新增) |
| 警告码 | 15 | **23**(`+W0110`–`W0117`;`W0117` 恒警告,无 `E0117`) |
| 内建数 | 110 | **110**(零新增;签名结构化不改内建集合,附录 G 表体逐字节不变) |
| `wlwl-toml` | manifest / lock / MVS | **冻结**;至多 `[features]` 下的**只读**键 `gradual_typing` / `match_exhaustiveness`,零 schema 变更 |
| spec 章节 | §1–§17 + 附录 A–G | §2.6 / §5.2.1 / §7.4 / §9.1 / §9.6 增补 + **新附录 E**;附录 G 重生成 |
| 锁测试 | 1517 | 见下方说明(不在表内写死,以 CI 为准) |
| 关键词表 | 14 | **14 不变**(`SEALED` 走前缀声明,词法面零扩张) |

> 计划书 §12.2 预估的是「67 激活 + 18 警告」;实测 **74 + 23** —— Step 6 / Step 8
> 在其后又分配了 `W0113`–`W0117` 与 `E0116`。数字变化本身**不违反**兼容承诺:
> 新增码对 v0.9 程序**不可观察**(默认 `off`,`off` 档不调用 checker)。
> 锁测试总数:本表**不在这里写死数字**,以 CI 的
> `cargo test --workspace` 为准。原因是 v0.10 一节里先后出现过三个数
> (1656 / 1731 / 1736),而它们分别对应 Step 10 / 12 / 15 的**中途**状态 ——
> 逐 Step 记数很容易在最后一次改动后忘记回头改,读者也就无从判断哪个是真值。
>
> [v0.10.1 / R10-049] v0.10.0 发布时的实测全量是 **1744 passed / 0 failed**
> (基线 1517,`+227`);上面的 `+139` 只算到 Step 10,后面四步的增量没进这张表。
> 需要逐 Step 明细请看 `docs/history/wlwl-build-plan-v0.10-COMPLETED.md` 的收尾表。
> v0.10.1 的数字见本版本自己的小节。

---

## [v0.9.0] — 2026-09-25

Spec: **wlwl-spec-v0.9** (`docs/history/wlwl-spec-v0.9.md`).
v0.8 archived at `docs/history/wlwl-spec-v0.8.md`.

### Added

- **Algebraic-effect runtime 模型**:runtime 把控制流统一为
  `Effect { tag: Tag, payload }`,`Tag ∈ { Yield, ChannelOp, Cancelled }`;
  `Scheduler::step` 为 effect handler 循环。impl 内部命名
  `TaskState::Suspended { tag, reason: YieldReason }` 取代旧 tuple variant。
- **结构化并发死锁检测 L1**:顶层错误码 `E0065`,软警告 `W0065`;
  严格同 scope 检测,显式 `Yield` 互让不触发。默认严格(`E0065`);
  `[features] strict_deadlock_detect = false` 降级为 `W0065` + `E0053`。
- **Task 取消 `reason` 字段(tag/payload cancellation)**:
  `TASK_CANCEL(task, reason?)` / `TASK_CANCEL_PARENT(reason?)` 签名扩展;
  reason 非法类型 → `E0066`;旧 `TASK_CANCEL(task)` 隐式 `{}` 兼容。
- **OOP 真实实现**:`CLASS` / `NEW` / `THIS` / `GET_PROP` / `SET_PROP` /
  `CALL_METHOD` 启用;spec §13 / §14 / §15 / §16 章节号在 v0.9.0 上冻结。
- **行为类型与会话类型协议**(顺序 + `⊕` 内部选择 + μ 递归骨架):
  `CALL_METHOD` 走协议状态机;协议已终止 → `E0050`,协议 step 错位 → `E0051`。
  外部选择 `&` / 命名协议 / 并行 `par` 留后续版本。
- **`THIS` 线性 capability**:不可变 `SET_PROP` + `THIS` 跨容器 / call /
  `AWAIT` / SPAWN / return 五类边界越界 → `E0032`。
- **`wlwl.toml [features]` 并发开关**:
  `strict_deadlock_detect`(默认 true)、`native_channel_close`(默认 false,
  开启后关闭通道 RECV 硬抛 `E0054`)、`channel_large_buf_threshold`
  (默认 1024,超阈值发 `W0066`)。

### Changed

- **同步通道真挂起**:`buf=0` 同步通道 SEND / RECV 在满/空时挂起当前 task,
  加入 channel 的 `sender_waiters` / `receiver_waiters`;
  `TRY_SEND` / `TRY_RECV` 保持非阻塞。
- **`YIELD` 位置解除**:v0.7 / v0.8 的 `E0014` 在 Block 直接子项之外的
  YIELD 触发路径删除;`LET(x, YIELD())` / `IF(cond, YIELD(), 42)` /
  数组字面量内部 / 间接调用 `LET(y, YIELD); y()` 均合法,自动挂起。
- **嵌套 YIELD 解除限制**:嵌套构造(`WHILE` / `FOR` / `IF` 分支)
  内 YIELD 恢复后继续执行嵌套体剩余部分。
- **`ChannelWouldBlock` 载荷路径删除**:同步通道真挂起后该 ERR 载荷
  无触发路径;`TRY_*` 仍非阻塞。
- **`E0055` / `E0057` 错误码移除**:关闭后 RECV 仍走
  `ERR(kind="ChannelClosed")` 载荷;跨任务 / 单任务不可变单元格统一走 `E0024`。

### Spec

- 语言规范 v0.9 定稿(`docs/history/wlwl-spec-v0.9.md`):§17 整体重写,
  §11.2 / §11.3 字面修订,§13 / §14 / §15 / §16 OOP 与会话类型落地;
  v0.8 规范归档至 `docs/history/wlwl-spec-v0.8.md`。
- 附录 G 镜像(`docs/appendix_G.md`)由 `gen-appendix-g` 重生成;
  OOP 实现位置章节号修正为 `§13` / `§15`;
  签名列同步 v0.9(无 `ChannelWouldBlock`,`TASK_CANCEL(task, reason?)`)。
- `wlwl-skill` 更新至 v0.9(真挂起并发、OOP / 会话协议 / 线性 `THIS`、
  错误码增删、`wlwl.toml` 新特性;新增 `examples/oop.wll`)。
- 负 `buf` 错误码定案为 **`E0031`**(§17.2)。

### Tests

- 锁测试总计数:v0.8.1 baseline `1409 passed` → v0.9 `1517 passed`(+108 项)。
- `cargo clippy --locked --workspace --all-targets -- -D warnings`:clean。
- `cargo fmt --check`:clean。

### Known limitations

- `Effect::MethodCall` / `Effect::ProtocolViolation` 已作为 enum 表面落地
  (spec §16.4 命名对齐);evaluator 尚未在 CALL_METHOD 路径上实际 raise
  这两个 effect(留后续版本接 effect-handler 调度)。

---

## [v0.8.1] — 2026-09-23

Spec: **wlwl-spec-v0.8** (unchanged from v0.8.0; v0.8.1 is a **patch**
release that ships zero spec text changes). Build plan, audit report,
and deviations: `docs/history/wlwl-build-plan-v0.8.1-COMPLETED.md`,
`docs/history/audit-report-v0.8.1.md`,
`docs/history/deviations-v0.8.md` (entries **D8-009** through
**D8-014**).

### Compatibility commitment (per plan §0.2)

Any v0.8.0 program yields identical results in v0.8.1, **with one
documented observable change**:

1. **`SUB` third argument is now length, not end-index** (D8-010 /
   `3b4db19`). Pre-v0.8.1: `SUB(s, start, end_idx)` — third arg was an
   end-index (`SUB("Hello, world", 7, 12)` → `"world"`). v0.8.1:
   third arg is a length (`SUB("Hello, world", 7, 5)` → `"world"`).
   Migration: `SUB(s, start, end_old)` → `SUB(s, start, -(start,
   end_old))` or `SLICE(s, start, end_old)`. Locked by three new tests
   (`substr_length_semantics_8_cases`,
   `substr_len_overflow_clamps_to_string_end`,
   `substr_len_zero_returns_empty`).
2. **No other program-visible change.** The other five patch items are
   either new *lexical* / *grammar* forms (D8-009 float exponents,
   D8-011 `MUT` as ordinary identifier, D8-012 string-literal
   subscript), example alignment (D8-013 `closure_cell.wll`), or
   pure deviation notes (D8-014) — none affect behavior of any
   pre-existing v0.8.0 program.

### Changed (non-breaking, additive)

- **§1.7 Float exponent literals** (D8-009 / `939c59a`). Forms
  `1e2`, `1.5e2`, `1.5e-2`, `1E3`, `2.5e+1` are now tokenized as a
  single `Float` literal per the §1.7 EBNF; pre-v0.8.1 the `e` /
  `E` would terminate the number and parse as an identifier
  (`E0011 expected ')', got Ident("e2")`). Six lexer lock tests
  (`lex_float_exponent_*`) + six eval lock tests
  (`eval_float_exponent_*`) gate regressions.
- **§1.4 `MUT` as ordinary identifier** (D8-011 / `9c1ab6b`).
  `MUT` can now appear as a binding name, expression identifier,
  call name, pattern identifier, or function parameter — five
  dispatch points (`parse_let` binding slot, `parse_expr`,
  `parse_call_or_ident`, `parse_pattern`, `parse_fun`). Spec §1.4
  mandates "其他位置可作普通标识符"; pre-v0.8.1 the parser
  swallowed `MUT` as a keyword in all five positions, breaking
  valid programs like `LET(MUT, "x")`. Eight lock tests gate
  regressions.
- **§A.2 String-literal subscript** (D8-012 / `29f3130`).
  `"hello"[0]` is now allowed as a parse form per §A.2 grammar;
  pre-v0.8.1 the postfix loop only fired on `Int` / `Float` /
  `Bool` / `Null` literals. Behavior of the resulting expression
  (subscript on a `String`) was already implemented — only the
  parser surface was missing. Nine lock tests gate regressions.
- **§3.3 `closure_cell.wll` example alignment** (D8-013 /
  `8ee8c77`). The example was rewritten from a `LET` then
  `SET`-on-immutable pattern (which depended on the legacy
  E-CloCap upgrade that v0.6 deleted) to the v0.8 idiom
  (`LET MUT` + `SET`); the v0.7 fidelity baseline line 1 was
  corrected from `NULL NULL NULL` to `1 2 3` to match current
  observed output. Two lock tests gate regressions.

### Pure deviation notes (no impl / spec change)

- **§1.8 / §A.2 nested-string inside `${...}`** (D8-014 /
  `a7392e7`). Documents the parser rejection of `"outer
  ${"inner"}"` as a deviation from naïve spec reading; recommends
  v0.9 spec option B (explicit forbid + §1.8 normative note) over
  option A (放开). Zero code change; the lock tests added during
  v0.8 already gate the current rejection behavior.

### Test counts

- `cargo test --workspace`: **1409 passed**, 0 failed, 2 ignored
  (was 1366 passing at v0.8.0; +43 new lock tests across the six
  D8-NNN items).
- eval crate: 754 → 767 (+13).
- parser crate: 80 → 89 (+9).
- v07_fidelity suite: 0 → 1 pass (`closure_cell` baseline aligned).

### Migration (only `SUB`)

```diff
- SUB(s, start, end_idx)
+ SUB(s, start, length)
+ # or, to keep pre-v0.8.1 semantics:
+ SLICE(s, start, end_idx)
```

No other source changes are required to upgrade v0.8.0 → v0.8.1.

## [v0.8.0] — 2026-09-23

Spec: **wlwl-spec-v0.8** (released 2026-09-23; v0.7 archived at
`docs/history/wlwl-spec-v0.7.md`). Build plan + deviations
(archived post-release):
`docs/history/wlwl-build-plan-v0.8-COMPLETED.md`,
`docs/history/deviations-v0.8.md`.

### Compatibility commitment (per plan §4.4)

Any v0.7 program that does **not** depend on the §12 "保留形式" old
table (which is rewritten away in v0.8 — see below) yields identical
results in v0.8. Confirmed observable-behavior changes:

1. **Literal subscripts now allowed**: `[1, 2, 3][0]` was a parse
   error (`E0010` / `E0011`) in v0.7 and earlier; in v0.8 it parses
   and evaluates to `1`. (`["a": 1]["a"]`, `[[1,2][0], 3]`, etc. —
   all allowed for the first time.) See `2bd11b3` and deviation D8-004.
2. **No other program-visible change.** Everything else is either
   prose-only clarification, metadata-only (registry fields,
   appendix_G anchors), or aligns spec text with behavior the
   implementation has had all along.

### Changed (non-breaking)

- **§12 保留形式重写 (D8-005)**: the "reserved but undefined" list
  (CLASS / NEW / THIS / MODULE / MODULE_REF / CALL / ARRAY / AND / OR)
  was implementation-stale — all entries have working `BuiltinSpec`
  records in `BUILTIN_REGISTRY` (LexerMacro / ResolvedBuiltin /
  ResolvedCompat). §12 now points to the registry as the single source
  of truth; the `b11_*` lock tests + the new
  `registry::tests::appendix_g_anchors_match_v07_section_numbers`
  (§2.9 / `061b0fd`) gate consistency. See `b266700`.
- **§4.3 NOT 透明传播 prose 反向 (D8-006)**: v0.7 prose wrongly
  framed NOT as the sole exception to §8.2 transparent propagation.
  Implementation has always been E0102-on-`NOT(ERR(...))`; prose now
  matches. `b9_not_with_err_arg_is_e0034` and
  `b11_err_consumer_registry_consistent` were already enforcing the
  corrected behavior. Recommended idiom: `NOT(BOOL(ERR(...)))` for
  safe coercion. See `32c9ada`.
- **§17.1 YIELD 位置限制 demoted (D8-007)**: the "YIELD() 必须
  出现在 Block 直接子项" rule was normative prose over what is
  actually an implementation choice (static task-body segmentation
  in `yield_split.rs`). §17.1 now frames it as "理想语义 + v0.7 /
  v0.8 实现路径" so future suspension-based schedulers can lift
  the limitation without claiming a breaking change. See `f028860`.
- **Spec / registry alignment (D8-008, 12-item batch)**:
  - 85 `BuiltinSpec.section` fields updated to v0.7 chapter numbers
    (registry was full of v0.4 / v0.6 stale numbers like §13.x /
    §15.x / §12.x / §7.x / §9.1 / §10.6 / §11.4) — `849dc3c`.
  - `POP` entry signature / group / version corrected
    (`POP(d, k, default) -> v` / `Dict` / `V06`) — `f8a5908`.
  - Column header in `docs/appendix_G.md` fixed
    (`ERR 消费者 (§12.7)` → `(§8.3)`, `宏函数 (§3.4)` → `(§1.4)`) —
    folded into `849dc3c`.
  - Spec §4.5 prose on literal subscripts rewritten to ALLOW; §A.2
    grammar was already permissive — `2bd11b3`.
  - Spec §1.6 split into `int_lit` (bare digits) + `int_literal`
    (optionally signed); parser / lexer source comments updated to
    document the two-step lexer-bare-digits + parser-desugar
    reality — `9c8bc08`.
  - `EXPECT_ERR` folded into the §8.3 consumer table; §2.4 %
    float `E0030` listed; SHIELD / SCOPE(ERR) / AWAIT host
    diagnostic clarifications; §1.5 `=` triple-identity
    disambiguation; §5.1 LET/FUN asymmetry normative; §2.1 / §10.11
    `TASK` name-collision normative — `2bf9093`.
  - `wlwl:std.agent` and `wlwl:std.ai` module docstrings updated with
    a "与类型名 TASK 的同名问题" section — `d1239e7`.

### Marked RESERVED (no trigger path; documentation only)

- **E0055** "CHANNEL_RECV after close native code": signal
  surfaces as `ERR(kind="ChannelClosed")` dict payload (§8.1 /
  §17.2), not as native code. `e0055_channel_recv_after_close_does_not_raise_native_code`
  test gates future regressions — `1b150db` + D8-003.
- **E0057** "cross-task immutable cell native code": same condition
  triggers `E0024` (unified immutable-cell code, §17.4).
  `e0057_immutable_cell_set_raises_e0024_not_e0057` gates it — same
  commit / deviation.

### Added (consistency tests)

- `pop_registry_entry_matches_dispatch` (D8-001 regression) — `f8a5908`
- 4 parser literal-subscript round-trip tests
  (`parser_array_literal_subscript_roundtrip`,
  `parser_dict_literal_subscript_roundtrip`,
  `parser_mixed_literal_subscript_chain`,
  `parser_nested_literal_subscript_in_array`) — `2bd11b3`
- 4 eval literal-subscript end-to-end tests
  (`eval_array_literal_subscript`, `eval_dict_literal_subscript`,
  `eval_chained_literal_subscript`,
  `eval_literal_subscript_with_set_sugar`) — `2bd11b3`
- 4 parser unary-minus round-trip tests
  (`unary_minus_integer_literal_desugars`,
  `minus_call_with_paren_is_not_sugar`,
  `unary_minus_variable_desugars`,
  `minus_call_three_args_is_not_sugar`) — `9c8bc08`
- `eval_unary_minus_integer_min_throws_e0034_via_desugar` (locks
  full sugar path → `E0034`) — `9c8bc08`
- `eval_negative_literal_minus_one` (`-1 → -1`, `--1 → 1`,
  `LET(x, 7); -x → -7`) — `061b0fd`
- `appendix_g_anchors_match_v07_section_numbers` (locks §-anchor
  whitelist in `docs/appendix_G.md`) — `061b0fd`
- `e0055_channel_recv_after_close_does_not_raise_native_code`
- `e0057_immutable_cell_set_raises_e0024_not_e0057`

`cargo test --workspace`: ~1366 tests passing, 0 FAILED.

### Known limits (unchanged from v0.7)

Documented in spec §17.7:

- Single-thread cooperative scheduling.
- `CHANNEL_SEND` / `CHANNEL_RECV` do not suspend (path B); use
  `TRY_*` or pre-sized buffers.
- No deadlock detector.
- Nested `YIELD` inside `WHILE` / `FOR` / `IF` does not resume the
  nested construct remainder (path B).
- Captured-`LET` upgrade (legacy E-CloCap) still diverges from a strict
  reading of §3.3.

### Phase B summary

Spec prose alignment batch (commit `2bf9093`) rolled 12 plan items
into a single doc-only commit; see the commit message for the per-section
delta. Spec §0.1 adds a "规范版本约定" paragraph establishing the
`major.minor` notation; historical `[v0.7.0]` rows in §17.7 are
preserved as version snapshots.

## [v0.7.0] — 2026-09-22

First concurrency release. Spec: **wlwl-spec-v0.7** (additive over v0.6).
Build plan + deviations: `docs/history/wlwl-build-plan-v0.7-COMPLETED.md`,
`docs/history/deviations-v0.7.md`.

### Added (language)

- **Structured concurrency + channels** (spec §17). New global builtins
  (17), none of which are ERR consumers — `ERR` arguments still
  transparently propagate per §8.2:
  - Scope / task: `SCOPE(fn)`, `SPAWN(fn)`, `AWAIT(task)`, `YIELD()`
  - Cancel: `TASK_CURRENT()`, `TASK_IS_CANCELLED()`, `TASK_CANCEL(task)`,
    `TASK_CANCEL_PARENT()`, `SHIELD(fn)`
  - Channel: `CHANNEL_NEW(buf)`, `CHANNEL_SEND`, `CHANNEL_RECV`,
    `CHANNEL_TRY_SEND`, `CHANNEL_TRY_RECV`, `CHANNEL_CLOSE`,
    `CHANNEL_LEN`, `CHANNEL_CAP`
- **New value types** `TASK` and `CHANNEL` (TYPE strings). Handle
  identity equality (`==(h, h)` is TRUE); display
  `<task handle id=N gen=M>` / `<channel handle id=N gen=M>`.
- **New error codes** (category `Concurrent`):
  - `E0052` SCOPE/SPAWN/SHIELD arg is not a function
  - `E0053` invalid task/channel handle (`TASK_CURRENT` outside a task)
  - `E0054` SEND/TRY_SEND on a closed channel
  - `E0055` reserved (close-on-read surfaces as `ERR(kind="ChannelClosed")`)
  - `E0056` SPAWN arity (including non-zero-param `fn`)
  - `E0057` reserved (cross-task immutable cell currently reuses `E0024`)
  - `E0058` top-level `SPAWN` without an active `SCOPE` (no implicit runtime scope)
- **Structured ERR kinds** (dict payloads, §8.1):
  `"ChannelClosed"` | `"ChannelWouldBlock"` | `"Cancelled"`.
  `NULL` is **not** a close signal — detect via `IS_ERR` + `ERR_PAYLOAD`.
- **Runtime modules** in `wlwl-eval`: `runtime` (scheduler / handles),
  `task`, `channel`, `yield_split` (path-B body segmentation).
- **BUILTIN_REGISTRY** 93 → **110** (17 concurrent entries + `Version::V07`
  + `BuiltinGroup::Concurrent`). Regenerate the table with
  `cargo run --bin gen-appendix-g -- ../docs/appendix_G.md`.
- **Conformance fixtures** `impl/tests/concurrency/*.wll` (yield, channel,
  SCOPE ERR surfacing, ERR consumers cross-task, 3× SHIELD).
- **Bench suite** `concurrency` (single-task fidelity, scope spawn/exit,
  channel throughput, close→RECV latency).

### Changed (non-breaking)

- **Function / handle equality** (`==`, `!=`): implementations now match
  v0.6 §2.4 instance identity for closures and add identity for
  `TASK`/`CHANNEL` handles (previously both always returned FALSE).
- **Trailing `YIELD()`** on the last body segment completes the task
  with `NULL` instead of aborting the process (path-B regression fix).
- **Spec lock** (`b11_registry_count_matches_spec_table`) pinned at 110.

### Known limits (v0.7.0)

Documented in spec §17.7 and `deviations-v0.7.md` — not silent gaps:

- Single-thread cooperative scheduling (no CPU parallel speedup).
- `CHANNEL_SEND`/`CHANNEL_RECV` do **not** suspend: full/empty yields
  `ERR(kind="ChannelWouldBlock")` (`P7-D2-001`). Use `TRY_*` or buffer.
- No bounded-channel deadlock detector (`P7-D8-001`).
- Nested `YIELD` inside `WHILE`/`FOR`/`IF` does not resume the nested
  construct remainder (path-B segmentation).
- In-flight sibling cancel may be unobservable under sync run
  (`P7-E3-001`); `SCOPE` still surfaces the first uncaught child `ERR`.
- Captured-`LET` upgrade (legacy E-CloCap) still diverges from a strict
  reading of §3.3 — prefer `LET MUT` for shared mutation.

## [v0.6.0] — 2026-09-20

### Changed (breaking)

The implementation now matches the **v0.6 specification** (the v0.5 spec
was never published; this is the first versioned release). Nine breaking
semantic changes (per the spec's Appendix B) are now in force:

- **A · Truthiness**: `0`, `0.0`, `""`, empty ARRAY, empty DICT, `NaN` are
  now falsy. Previously only `FALSE` and `NULL` were. (Matches Python /
  JS / Ruby conventions; the v0.5 design was deemed too unusual.)
- **B · `&&` / `||` short-circuit**: left operand's truthiness decides
  whether the right is evaluated; ERR on the left propagates without
  touching the right. Previously both sides were always evaluated.
- **C · `IF` consumes `ERR`**: an `ERR` condition routes to the `else`
  branch (or propagates `ERR` if no else). Previously ERR was treated
  as truthy and took the then-branch.
- **D · `!` and `NOT` accepted without warning**: `W0054` (the
  `!`-is-deprecated warning) was removed. Both forms are now equally
  canonical.
- **E · `POP` renamed `AT_K`**: the dict lookup-with-default helper
  is now `AT_K(d, k, default)`. `POP` remains as a back-compat alias
  but emits no warning. `REMOVE_KEY` is the dedicated removal function.
- **F · String subscript read**: `s[i]` returns a single-codepoint
  string; negative indexes count from the end. `s[i] = ...` is
  rejected (`E0030`).
- **G · Explicit `LET MUT`**: mutable bindings must be declared
  `LET MUT(name, value)`. The v0.5 "first closure call upgrades cell
  to mutable" rule was deemed too magical and removed. `SET` on a
  `LET` (immutable) binding raises `E0024`.
- **H · Integer overflow throws `E0035`**: replaced the v0.5
  "saturate to i64::MAX/MIN + emit `W0015`" behavior with an
  explicit error. `E0035` is now shared with float-to-int overflow.
- **J · String interpolation**: `"hi ${name}!"` form, with `\$`,
  `\n`, `\t`, `\\`, `\"`, `\/`, `\0`, `\b`, `\f` escapes. Inner
  expressions are evaluated and `STR`-rendered; an `ERR` inside
  `${...}` propagates without producing partial output.

### Added

- **String interpolation lexer**: 3-token form `StrStart`,
  `StrText(String)`, `StrEnd`; recursive sub-lex for the inner
  expression. The AST adds `Literal::Interpolated(Vec<StrPart>)` and
  a `StrPart` enum (`Text` / `Expr`).
- **`MUT` keyword** (lexer-level contextual): `TokenKind::Mut`,
  consumed by the parser only between `LET` and `(`. Other positions
  treat it as a regular identifier.
- **`BOOL` registered as ERR consumer**: `BOOL(ERR(...))` returns a
  boolean instead of triggering §8.2 transparent propagation.
- **`&&`, `||` registered as ERR consumers**: short-circuit paths
  live in a new `eval_logical_short_circuit` helper, intercepted in
  `eval_call` before the generic ERR-propagation block.
- **`IF` registered as ERR consumer**: documented in the registry
  for completeness; the actual handling lives in `eval_if`.
- **String subscript AST path**: `INDEX_GET` accepts `(String,
  Integer)`; `INDEX_SET` rejects `String` first arg with `E0030`.

### Changed (non-breaking)

- **Cell mutability flag is permanent**: `Binding.mutable` is set
  at binding creation; no "closure-capture upgrade" mechanism
  remains. The dead `Env::upgrade_all_to_mutable` was removed.
- **Error message updated**: `SET` on an immutable binding now
  suggests `LET MUT` in its message.
- **Registry**: `Version::V06` added; `&&`/`||`/`AT_K` added as
  `ResolvedBuiltin`; `POP` demoted to `ResolvedCompat`. Total
  entries: 90 → 93.
- **`BOOL` is now an ERR consumer in the registry**, matching its
  runtime behavior.
- **Removed `W0015`** (integer saturate warning) — now `E0035` on
  overflow.
- **Removed `W0054`** (`!` deprecation) — `!` is canonical.

### Fixed

- **Parser**: `LET MUT(name, value)` now correctly accepts the
  `MUT` keyword between `LET` and `(`. (Earlier draft had the
  syntax as `LET(MUT name, value)`, which was off-spec.)
- **Formatter**: `Literal::Interpolated` segments are rendered
  with proper escape sequences (`escape_str_text` helper); `MUT`
  keyword preserved when re-formatting `LET MUT(...)` expressions.

## [Unreleased — v0.5.0]

### Added

- **wlwl-spec-v0.6** — supersedes v0.5 with the nine breaking
  semantic changes above. The v0.3 and v0.4 specs have been moved
  to the trash (recoverable).

> **Note.** v0.5 was never published — the spec was drafted but not
> tagged, and the implementation never claimed compliance with it.
> The v0.5 spec is in `docs/standard/` was moved to trash on
> 2026-09-20 along with v0.3 and v0.4.

## [Unreleased — v0.4.0]

### Added

## [Unreleased -- v0.6.1] (extension rename)

### Changed (breaking)

- **File extension renamed from .wl to .wll**: Wolfram
  Language (Mathematica) has long claimed .wl, which causes
  editors, GitHub Linguist, Shiki/Rouge/Prism highlighters, and the
  Wolfram VSCode extension to mis-identify WLWL source files. The
  .wll extension has no prior claim from any other language, so
  adopting it costs zero ecosystem work. This is the first breaking
  change at the *file-format* level (the spec semantics are unchanged).

### Migration

- All tracked .wl files renamed to .wll (12 in impl/examples/ + 1
  in wlwl-skill/). git log --follow preserves history.
- Test fixtures, error messages, doc paths, CI commands, and skill
  bundle references updated.
- examples/showcase.wll still contains a pre-existing bug (lowercase
  	rue keyword); unrelated to this rename.
- Conformance test fixtures (impl/tests/conformance/*.wll) were left
  untouched; their internal references to .wll source files were
  updated.


### Note on the spec filename

- Renamed docs/standard/wlwl-spec-v0.6(SHA1_cdb548cb5161e61d836aad2208fd33adc0917861).md to docs/history/wlwl-spec-v0.6.md. The SHA1 in the old filename never matched the file's content (the v0.5 spec had the same issue), so the content-address fiction is dropped entirely. Specs are now identified by version + name only; git history (git log -p --follow) is the source of truth for content changes.

## [Docs archival] — 2026-09-22

- `docs/history/wlwl-spec-v0.6.md` → `docs/history/wlwl-spec-v0.6.md`
- `docs/plan/*` → `docs/history/` (`wlwl-build-plan-v0.7-COMPLETED.md`,
  `wlwl-phase-b-implementation-plan.md`, `deviations-v0.7.md`)
- `docs/plan/` now holds only a README pointing at the archive;
  next iteration starts a fresh plan + `deviations.md` there.
