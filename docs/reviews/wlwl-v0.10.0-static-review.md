# WLWL v0.10.0 静态 REVIEW 综合分析报告

| | |
|---|---|
| **审查对象** | WLWL v0.10.0("Static Contracts" 里程碑) |
| **基线** | `v0.9.0`(tag)→ `306532b`(分支 `wip0.10`,19 commits) |
| **审查日期** | 2026-09-29 |
| **方法** | 三段式:①以 v0.9 规范为基准 ②以 v0.10 构建计划为宏观标尺 ③静态阅读实际变更的源码(词法 / 语法 / 静态层 / 运行期 / 工具链) |
| **手段** | 4 路并行只读审查 + 主审逐条复核;**头条结论一律实测复现**(不靠推理下结论) |
| **变更规模** | `impl` 净增 11,661 行 / 删 557 行;新增 crate `wlwl-types`(5,416 行);规范 v0.9 1,346 行 → v0.10 1,363 行 |
| **修复状态** | **3 个 P0 全部已修复并实测复验(2026-09-29)**,详见 [§0.1 修复记录](#01-p0-修复记录2026-09-29);其余 P1/P2 未动 |

---

## 0. 审查纪律

本报告区分三种证据等级,全文标注:

| 标记 | 含义 |
|---|---|
| **【实测】** | 已用 `wlwl` 真二进制复现,命令与输出见 §13 |
| **【静态】** | 读代码得出的结论,未运行验证 |
| **【存疑】** | 子审查提出的推断,主审未能核实或实测后**推翻了** |

**本次实测推翻了两条子审查的推断**(见 P0-3 与 §7.4),已在正文明确更正,不做「看起来一致」的模糊表述。

---

## 0.1 P0 修复记录(2026-09-29)

三个阻塞缺陷已全部修复,每项都配了锁测试并用真二进制复验。**修复只动实现,没有改任何规范文本** —— P1-1 / P1-2 那些规范错误仍按 §11.4 挂着,等用户裁决。

### 0.1.1 修复总览

| # | 缺陷 | 改动文件 | 新增测试 | 复验 |
|---|---|---|---|---|
| P0-1 | `wlwl fmt` 折叠路径物化 `NULL` default 臂,抑制 `E0116` | `wlwl-formatter/src/lib.rs` | 2 | ✅ 三步链路全绿 |
| P0-2 | 默认形参 / `*rest` 形参误报 `E0111` | `wlwl-types/src/{check,diag}.rs` | 3 | ✅ rc 1→0 |
| P0-3 | `SEALED` 劫持用户自定义函数 | `wlwl-parser/src/lib.rs` | 3(+1 改写) | ✅ 标量 / 数组两种实参均恢复 |

门禁:`cargo test --workspace` **1744 passed / 0 failed**(修复前 1736,净增 8);`cargo fmt --check` 0 diff;`clippy -D warnings` 0;`cargo doc` 0 warning。

### 0.1.2 P0-1 修复

**根因**:`render`(折叠)路径的 `Expr::Match` 臂用 `..` 吞掉了 `default_synthetic`,`render_match` 无条件渲染 default;而 parser 省略 default 臂时物化的是 `NULL` 字面量。

**改动**:

1. `wlwl-formatter/src/lib.rs` 折叠路径的 match 臂解构出 `default_synthetic` 并透传;
2. `render_match` 签名新增 `default_synthetic: &bool`,末尾的 `, {default}` 渲染改为 `if !*default_synthetic` 才做。

inline 路径本来就正确,未动。两条路径现在共用同一个判据,**同一条 AST 不会再有两种 canonical 形态**。

**锁测试**(`wlwl-formatter/tests/formatter_tests.rs`):

- `a_folded_match_without_a_default_arm_never_gains_a_null_default` —— 先断言夹具**真的走了折叠路径**(多行),再断言产物不含 `NULL`,最后跑幂等;
- `a_folded_match_with_an_explicit_default_arm_keeps_it` —— 对照组:显式写了 default 臂时折叠必须保留它。这条防的是「修成折叠时永不渲染 default」这种反向过头。

> 该文件 403 行测试在 v0.10 **零改动**、新臂零覆盖 —— 这正是本缺陷能进 release 的原因,现在补上了。

**实测复验**(fix 前 → fix 后):

| 步骤 | fix 前 | fix 后 |
|---|---|---|
| ① 原始非穷尽 `MATCH` + 省略 default 臂 | `E0116: missing ERR(_)` rc=1 | `E0116: missing ERR(_)` rc=1 |
| ② `wlwl fmt` 产物 | **含 `NULL`** | **不含 `NULL`** |
| ③ 产物落盘后再 `check` | **rc=0,`E0116` 消失** | **rc=1,`E0116` 仍在** |

### 0.1.3 P0-2 修复

**根因**:`check.rs` 的 arity 判定是 `params.len() != args.len()`;而运行期按「必填 = 非 `*rest` 且无默认值 / 有 `*rest` 则不限上界」处理。`Ty::Fun` 只带 `Vec<Ty>`,装不下这两个信息。

**改动**:

1. `Checker` 新增 `arity: HashMap<String, (usize, usize)>` 字段;
2. 新增 `fn arity_shape(&[FunParam]) -> (usize, usize)` —— 与运行期同一套规则;
3. **注册点在 `Expr::Let` 臂**,不是 `Expr::Fun` 臂。这是修复过程中踩到的一个真实陷阱:绝大多数函数是**匿名**的(`LET(f, FUN(…))`),名字来自 `LET` 而非 `FUN` 的可选 `name` 字段。最初挂在 `Expr::Fun` 的 name 上,结果一条都记不上,arity 查表恒空,连既有的 `call_arity_mismatch_is_reported` 都一起挂了;
4. `check_call_with_sig` 的 `check_arity: bool` 改为 `arity: Option<(usize, usize)>`,判定从 `!=` 改为区间 `found < required || found > max`;
5. `TypeDiagKind::CallArityMismatch` 新增 `max` 字段;消息在 `max == expected` 时**保持原措辞**不变,只有区间真的存在时才说「expected at least N」/「expected N to M」。既有锁测试逐字不变。

> 单独一张表而**不改 `Ty::Fun`**:`Ty` 还要渲染进 `*.wll.sig` 与 `wlwl schema` 的 JSON,往里塞 arity 会污染那两个对外契约。

**锁测试**(`wlwl-types/src/check.rs`,3 条):

- `a_default_parameter_may_be_omitted` —— `g()` 与 `g("bye")` 都干净;`g(1, 2)` 仍报 `E0111`;
- `a_rest_parameter_may_take_any_number_of_arguments` —— `f(1)` / `f(1,2,3,4)` 都干净;`f()` 仍报(必填形参没给);
- `plain_parameter_tables_keep_the_exact_arity` —— 对照组:普通形参表少报多报,与修复前**逐字等价**。

> 这三条夹具一律用**裸调用语句**而不是 `PRINT(f(1))`。原因值得记一笔:`PRINT` 属于 A6′ 未结构化的 50 条内建,拿不到签名就整条提前返回 `Dynamic`,**参数列表根本不会被遍历** —— 包一层会让元数测试永远测不到东西(我第一版夹具就栽在这,四个测试全红)。

**实测复验**:

| 程序 | fix 前 `check` | fix 后 `check` |
|---|---|---|
| `LET(g, FUN((name = "hi"), name)); PRINT(g());` | `E0111: expected 1, found 0` rc=1 | `OK: parsed` **rc=0** |
| `LET(f, FUN((a, *rest), a)); PRINT(f(1));` | `E0111: expected 2, found 1` rc=1 | `OK: parsed` **rc=0** |

两条的 `wlwl run` 全程都是 rc=0(打印 `hi`/`bye`、`1`/`1`),运行期从未受影响 —— 这也确认了缺陷只在编译期。

### 0.1.4 P0-3 修复

**根因**:`SEALED` 是**标识符**而非关键字,`parse_call_or_ident` 按名字无条件拦截。`IMPORT` / `EXPORT` 没这个问题 —— 它们是词法关键字,用户压根不能定义同名函数。

**改动**:两段式判别,而不是单点收紧。

1. **三词前瞻** —— `SealedDecl = "SEALED" "(" name_array ")"`,而 `name_array` 恒以 `[` 开头(附录 A;`parse_import_name_list` 第一件事就是 `expect_specific(E0011, "'['")`)。所以 `SEALED` `(` 后面**不是** `[` 的,一定不是声明,直接落到普通调用路径 —— 无错误、无回溯;
2. **失败回退** —— `SEALED([1, 2, 3])` 过了前瞻但不是名字列表,声明解析失败时**回退游标**重当普通调用。

**取舍要说清楚**:修复后**没有任何 `SEALED` 形态会让 parser 硬失败** —— 要么是合法声明,要么退回普通调用。代价是写错的声明(`SEALED([1])`)不再得到「名字列表必须是字符串」这种就地的语法错,而是变成一次对 `SEALED` 的调用,到运行期报「未定义的名字」。这是有意接受的降级:反过来硬判错,就直接破坏「任何 v0.9 程序默认可观察行为不变」这条本版的立身承诺。

**锁测试**(`wlwl-parser/src/lib.rs`):

- `a_user_defined_sealed_function_is_still_callable` —— 标量实参;
- `sealed_called_with_an_array_literal_is_not_a_declaration` —— 数组字面量实参(靠回退救回);
- `a_real_sealed_declaration_still_parses_as_a_declaration` —— 对照组,防止把声明语法一并放跑;
- `sealed_always_resolves_to_a_declaration_or_an_ordinary_call` —— 把上面的取舍写成契约:合法声明是 `Expr::Sealed`,其余五种形态**全部**必须 parse 成功且不是声明。

**一条既有测试被改写**:`parse_sealed_rejects_a_missing_name_list` 原本断言 `SEALED();` / `SEALED("add");` 是错误 —— 它编码的正是 P0-3 证伪的那个假设。已改写为 `sealed_always_resolves_to_a_declaration_or_an_ordinary_call`,并在 doc comment 里写明为什么契约要变。**这是本次修复唯一一处「让既有测试变绿」的改动,不是放宽标准,而是把一个错误前提换成正确前提。**

**实测复验**:

| 程序 | fix 前 | fix 后 |
|---|---|---|
| `LET(SEALED, FUN((x), x)); PRINT(SEALED(41));` | `E0011: expected '[', got Integer(41)` rc=1 | 打印 `41`,**rc=0** |
| `LET(SEALED, FUN((xs), INDEX_GET(xs, 1))); PRINT(SEALED([1,2,3]));` | `E0010: expected identifier or string in name list` rc=1 | 打印 `2`,**rc=0** |

### 0.1.5 修复未覆盖的部分

- **§11.4 P1-1** 规范 §2.6 码号错(`E0012` vs `E0010`)、**P1-2** 兼容承诺自述不实、**P1-3** formatter 可产出不可解析源码、**P1-4 / P1-5** —— 均未动,属规范文本或需额外裁决;
- **§11.5 P2 全部**未动;
- `SEALED` 固有的歧义(`SEALED(<name-list>)` 既是合法声明又长得像一次调用)已被 parser 侧注释和锁测试**记录在案**,但**尚未写进 spec §9.1** —— 那是 P1 级的规范改动,等用户拍板。

---

## 1. 结论摘要

### 1.1 总体判定

**v0.10 达到了它的设计目标,但发布前有 3 个必须修复的缺陷。**

设计目标达成情况:该做的都做了 —— 新增 `wlwl-types` 静态层 5.4k 行、模块契约(`*.wll.sig` + `SEALED`)、MATCH 穷尽性、5 个新 CLI 子命令、LSP 薄壳,全部落地且有锁测试;`wlwl-eval` 运行期语义**逐 hunk 核对后确无变更**;包管理器面**确未扩展**;关键字表与保留形式**确未变化**。

但「不误报优先」这条全局纪律被破了:**两处对完全合法的 v0.9 程序产出硬错误或静默改变语义**,且其中一处会**静默关掉 v0.10 自己新增的诊断**。两者都不是设计取舍,是实现漏洞。

### 1.2 门禁实测

**本表是 v0.10.0 审查当时的快照**,数字保留为历史记录;**当前值以 CI 的
`cargo test --workspace` 为准**(与 R10-049 对规范表格的处理同一条纪律 ——
逐 Step 记死一个数,只会在最后一次改动后忘记回头改)。

| 命令 | 结果(2026-09-29 审查时 / v0.10.0 基线) |
|---|---|
| `cargo test --workspace` | **1736 passed / 0 failed** |
| `cargo fmt --check` | 0 diff |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | 0 warning |
| `cargo doc --workspace --no-deps` | 0 warning |
| `conformance_static` 连续 3 轮 | 均 3 passed / 0 failed |
| `cargo deny --locked --all-features check` | ❌ 失败(**D10-011,既有**,干净树同样失败) |
| `wlwl-skill/examples/*.wll` | 10/10 exit 0 |

**v0.10.1 收口时的实测**(2026-09-29,本轮 13 个 Step 全部落地后):
`cargo test --workspace` **1807 passed / 0 failed**;
`cargo fmt --check` 0 diff;`clippy -D warnings` 0;`cargo doc` 0 warning。
基线 1744(见 §0.1)→ 1807,净增 63。

**无测试被删除**:各文件 `#[test]` 数只增不减(`wlwl-eval` 789→798、parser 63→72、error 48→52、registry 7→15、runtime 25→27;lexer 29→29 未变,与其「词法层零变化」自述一致)。

### 1.3 必须处理的问题(按严重度)

> **2026-09-29 更新(v0.10.1 收口):P0 三条 + P1 五条 + P2 的 P2-1~P2-5 / P2-7
> 已全部处置,逐条对应见「现状」列。唯一仍挂的是 P2-6(非末条顶层语句的 `ERR`
> 被静默丢弃),它是**既有行为、与 v0.10 无关**。**
>
> P0 三条是本报告审查当时修的(§0.1);P1 / P2 是 v0.10.1 修复计划
> (`docs/history/wlwl-v0.10.1-修复计划.md`)接着修的,编号见「对应」列。

| # | 问题 | 等级 | 证据 | 现状 | 对应 |
|---|---|---|---|---|---|
| **P0-1** | `wlwl fmt` 在折叠路径写出合成的 `NULL` default 臂,**静默关掉 v0.10 新增的 `E0116` 穷尽性检查** | **阻塞** | 【实测】 | ✅ 已修复 | §0.1.2 |
| **P0-2** | 默认形参 / `*rest` 形参的调用被误报 `E0111`,**合法程序在开门后被判死** | **阻塞** | 【实测】 | ✅ 已修复 | §0.1.3 |
| **P0-3** | parser 把用户自定义的 `SEALED` 函数调用当成模块声明,**v0.9 合法程序变成硬解析错** | **阻塞** | 【实测】 | ✅ 已修复 | §0.1.4 |
| P1-1 | 规范 §2.6 称约束错报 `E0012`,实现发 `E0010`,且**零测试覆盖码号** | 高 | 【静态】+【实测】 | ✅ 已修复(§2.6 三条事实按实测重写) | R10-040 |
| P1-2 | 规范自述「§1–§17 逐条未改」**不实**(4 处未申报的就地改写,其中 2 处是新增 `[规范性]`) | 高 | 【静态】 | ✅ 已修复(§0.1 冻结范围改写 + 附录 D 7 行申报表) | R10-041 |
| P1-3 | `wlwl fmt` 可把能解析的源码改写成**不能解析**的源码 | 中 | 【静态】 | ✅ 已修复(parser 兜底吸收现在保留整个类型头) | R10-020 |
| P1-4 | `SEALED` 可出现在任意表达式位置,规范禁止「只出现在模块顶层」 | 中 | 【静态】 | ✅ 已修复(原为「部分缓解」) | R10-021 |
| P1-5 | 密封分支的 `E0023` 绕过 `self.diag()`,缺 `source_line` | 中 | 【静态】 | ✅ 已修复(改走 `self.diag()`) | R10-022 |
| P2-1 | `BoundedVar` 可任意右嵌套(`T: A: B`) | 低 | 【静态】 | ✅ 已修复(→ `E0010`) | R10-025 |
| P2-2 | 类型层 `is_ident` 只认 ASCII,`LET(x: 整数, 1)` 报 `E0010` | 低 | 【静态】 | ✅ 已修复(改用 Unicode 属性判定) | R10-026 |
| P2-3 | `INTEGER: Comparable` 落成通配符而非 `Ty::Integer` | 低 | 【静态】 | ✅ 已修复(该改文档:约束本就被丢弃) | R10-027 |
| P2-4 | 类型错误绕过 `err_at` ⇒ 无源码行 | 低 | 【静态】 | ✅ 已修复(`collect_static_diagnostics` 统一补) | R10-028 |
| P2-5 | `TypeArityMismatch` 死代码 + 未兑现承诺 | 低 | 【静态】 | ✅ 已修复(删变体,承诺一并撤掉) | R10-029 |
| **P2-6** | 非末条顶层语句的 `ERR` 被静默丢弃 | 低 | 【静态】 | ⬜ **未动(既有,与 v0.10 无关)** | — |
| P2-7 | 附录 A 的文法**零锁测试** | 低 | 【静态】 | ✅ 已修复(五条锁测试) | R10-050 |

---

## 2. 基准面:v0.9 承诺了什么不能变

v0.10 规范第 9 节与构建计划 §9 的兼容性承诺是一切审查的标尺:

> **任何 v0.9 程序,在 v0.10.0 上默认可观察行为不变。例外:无。**

拆成可验证的不变式:

| # | 不变式 | 核验结果 |
|---|---|---|
| I1 | 运行期语义逐条不变 | ✅ `wlwl-eval` diff 逐 hunk 分类,**无 (c) 级变更**(§8.1) |
| I2 | 关键字表不变 | ✅ §1.4 逐行比对,仅新增一句 `SEALED` 非关键字的说明 |
| I3 | 保留形式集合仍为空 | ✅ §12 表体仍为 `(无)` |
| I4 | `wlwl-toml` 功能面零扩展 | ✅ `manifest.rs` +411 纯新增两个只读键;`lock.rs` / `mvs.rs` 零 diff |
| I5 | 静态层默认整条不执行 | ✅ 门禁在 `main.rs:810-814`,早于签名表构建与 checker |
| I6 | `strict_types` / `E0033` 路径不变 | ✅ 未触碰 |
| I7 | `E0055` / `E0057` 继续不出现 | ✅ |
| I8 | **v0.9 可解析的源码,v0.10 仍可解析** | ❌ P0-3 破 → ✅ **已修复**(§0.1.4) |
| I9 | **`wlwl fmt` 的可观察输出不变** | ❌ P0-1 破 → ✅ **已修复**(§0.1.2) |

I1–I7 成立;I8、I9 曾被本次审查实测推翻,**现已修复并复验**。这三条破坏的正是「v0.9 程序在 v0.10 默认行为不变」这条对外承诺,而它正是本版的立身之本 —— 所以优先在发版前修掉,而不是留给 v0.10.1。

---

## 3. 宏观面:v0.10 的意图与验收标尺

### 3.1 计划书的成功标准达成

| 标准 | 内容 | 结论 |
|---|---|---|
| S1 | 开启可抓错 | ✅ `E0110`–`E0116` / `W0117` 逐条由 `conformance_static` 验到 |
| S2 | 注解失配 / 模块签名违例 / MATCH 非穷尽可稳定诊断 | ✅ 三个方向都验到;模块契约**两个破坏方向**都验到 |
| S3 | 运行时兼容 | ✅ 但见 I8/P0-3 |
| S4 | 悬空承诺清零 | ✅ D-4 裁决 B′,枚举 tag 面 + 规范明文,两条锁测试双向钉住 |
| S5 | 无包管理动作 | ✅ 逐条核实 |
| S6 | 人日可核 | ✅ 立项单完整 |

### 3.2 出口条件(§12.4)逐条

| # | 条件 | 结论 |
|---|---|---|
| 1 | S1–S6 全过 | ✅ |
| 2 | 无包管理 diff | ✅ |
| 3 | 默认模式全绿 | ✅ |
| 4 | 开启模式 conformance 连续 3 次一致 | ✅ 实测三轮 |
| 5 | §8「明确不做」零触碰 | ✅ 无后端 crate、无 ownership、无用户宏、110 条注册表未一次全结构化 |
| 6 | 人日偏差 > +50% 须补 D10-NNN | ✅ |
| 7 | 唯一语法面扩张已单列,关键字/运算符表零变化 | ✅ 实测 §1.4 / §1.5 逐行相同 |

**出口条件形式上全部满足。** 但条件本身没有覆盖「合法程序误报」与「fmt 行为变更」两类,这是验收设计的盲区,不是执行者的疏忽。

---

## 4. 语言面增量(规范层)

### 4.1 结构差异:小而干净

`docs/standard/wlwl-spec-v0.10.md` 相对 `docs/history/wlwl-spec-v0.9.md` 的章节级差异:

**新增 4 个小节**:`§2.6 静态类型词汇(编译期)`、`§5.2.1 类型注解的静态解释`、`§7.4 静态诊断(编译期)`、`§9.6 模块契约`;
**重写附录 E** 为「模块契约文件与工具接口(规范性)」,含 `E.1` 签名文件文法 / `E.2` `wlwl interface` JSON / `E.3` `wlwl schema` JSON。

净增仅 **17 行**。这是一个刻意做小的语言增量,符合「纯加法 + 默认关」的定位。

### 4.2 诊断码增量

| 类别 | 新增 | 说明 |
|---|---|---|
| 错误码 | `E0110`–`E0116`(7) | 注解失配 / 调用失配 / 返回失配 / 模块契约三向 / MATCH 非穷尽 |
| 警告码 | `W0110`–`W0117`(8) | `E0110`–`E0116` 的 warn 档 + `W0117` |
| **故意不存在** | `E0117` | 不可达子句恒为警告,不留 error 档 |

注册表实测 **97 个变体 = 74 E + 23 W**。双向锁测试真实存在且是真双向:
`every_code_the_spec_lists_exists_in_the_registry` / `every_registered_code_is_covered_by_the_spec`,码清单**从规范正文抽取**并展开 `` `A`–`B` `` 区间,不手抄;豁免表 `DELIBERATELY_UNREGISTERED = ["E0055","E0057","E0117"]`,且豁免码必须**确实未注册**否则断言失败。

v0.10 顺带补齐了 v0.9 漏登记的码:`W0001` / `W0012` / `W0013` / `W0015` / `W0052` / `W0054` 六个警告码首次入表,并**删除**了注册表中本就不存在的 `W0014`。这是修 drift,不是扩张。

### 4.3 语法增量

```
TypePos     = Type | BoundedVar .                        // 附录 A
Type        = TypeHead [ "[" TypeArg { "," TypeArg } "]" ] .
TypeHead    = identifier .
TypeArg     = Type | BoundedVar .
BoundedVar  = identifier ":" Type .
SealedDecl  = "SEALED" "(" name_array ")" .
```

`LET` / `FUN` 形参 / `FUN` 返回三处新增可选 `[ ":" TypePos ]`。`Expression` / `Literal` / `OpToken` / `Clause*` / `Pattern*` 全部逐字未变。`SEALED` 只以 `ModuleDecl` 成员出现,**不进 `KeywordCall`** —— 与「词法面零扩张」自洽。

### 4.4 规范自述与实际的偏差

| # | 规范说 | 实际 | 影响 |
|---|---|---|---|
| D1 | 「§1–§17 逐条未改,只有 §5.2 与 §7.3 被扩写」 | §5.2 表体与 §7.3 **逐字未变**(新内容在 §5.2.1 / §7.4,是新小节不是扩写);另有 §9.1 / §10.5 / §16.4 / §17.4 **4 处未申报的就地改写**,其中 §16.4 / §17.4 是新增的 `[规范性]` 段落,落在冻结章 | 兼容承诺的**字面表述不成立**。语义上无害(4 处都只是描述既有行为),但这是规范性文本的自述失准 |
| D2 | §2.6 事实③:`ARRAY[INTEGER]: Comparable` 报 **`E0012`** | 实现发 **`E0010`**(`parser/src/lib.rs:2442`),且 `E0012` 在 parser 里的既有语义是「方括号内缺逗号」 | 规范错误。**注**:`docs/history/deviations-v0.10.md:86` 登记的也是 `E0010`,即仓库自己知道实现是 `E0010`,只有规范没跟上 |
| D3 | §2.6 事实②:尖括号与箭头形式「**都不会报错**」 | 箭头形式在**类型注解位**确实静默(`LET(g: FUN(INTEGER) -> STRING, "s")` 通过且能跑,`D10-005` 准确);尖括号在注解位是**响亮的 `E0011`**,且报错位置指向注解之后那个无辜的 `)`。**同一句里两个形式的行为不同** | 措辞过宽。实测见 §13.3 |
| D4 | `TypeExpr` 的不透明类型叫「具名类型」 | 注释与偏差登记写作 `Named{...}`,但 AST 实际只有 `Ident` / `Array` / `Generic` / `Bounded`,**无 `Named` 变体** | 术语漂移,易误导后续实现 |
| D5 | `NameItem` 定义 | 附录 A 含 `identifier` 分支,§9.2 不含 —— **两份规范给出不同答案** | 自相矛盾 |
| D6 | `ModuleDecl` | 附录 A 定义了,但 `Program` 产生式**未引用它** —— 悬空产生式 | 规范性文法无入口路径 |

---

## 5. 词法层:**零变化**(硬证据)

【静态】

```
git diff --stat v0.9.0..HEAD -- impl/crates/wlwl-lexer   →  (empty)
```

整个 `wlwl-lexer` crate 在 v0.9.0→HEAD 之间**一个字节都没改**。

- 关键字枚举在 `wlwl-lexer/src/lib.rs:435-474` 的单个 match 里,24 个字面拼写 + `_ => Ident(text)` 兜底;`SEALED` 不在其中,走兜底成为普通标识符。
- `is_reserved_keyword`(`:130-145`)只列 10 个,零 diff。
- v0.10 的类型文法**没有引入任何新终结符**:`:` / `[` / `]` 全是既有 token。唯一的词法层新事实是 `:` 在类型注解位获得第二种含义 —— 那是上下文判定,不是新记号。

**这一条是本版最干净的设计决策**,兑现了「唯一语法面扩张」且不碰词法。

---

## 6. 语法层

### 6.1 类型表达式:两趟解析

【静态】

不是递归下降直接作用在 token 流上,而是两趟:

1. **平铺收集** `parse_type_annotation`(`parser/src/lib.rs:1135-1185`):遇 `:` 后,只把 `(` / `[` 计入 `depth`,遇到**顶层** `,` / `)` / `]` / EOF 停止,把 token 拍平成 `Vec<String>`。
2. **结构化解析** `parse_type_expr_from_pieces`(`:2421-2463`)→ `TypeExprParser::parse_expr`(`:2475-2511`):先看 `"ARRAY"` 字面量特判,再看下一个元素是 `[`(容器)、`:`(带约束裸变量)、其它(无约束裸标识符)。**无回溯,两步前瞻。**

`T: Comparable` 的识别点**唯一**(`:2498-2506`),因为 `parse_braced` 的每个参数也调 `parse_expr`,同一处同时覆盖顶层注解与方括号内类型参数两条路径。

四个调用点,无遗漏:`LET` 模式(`:863`)、`LET MUT`(`:910`)、`FUN` 形参(`:1413`)、`FUN` 返回(`:1444`)。

### 6.2 `SEALED`:不是关键字,但有代价

【静态】识别点在通用表达式解析器 `parse_call_or_ident`(`:1927-1929`):

```rust
if name == "SEALED" && matches!(self.peek(), TokenKind::LParen) {
    return self.parse_sealed(line, col);
}
```

`parse_sealed`(`:1792-1807`)复用 `parse_import_name_list`,与 `EXPORT` 逐行同构,支持 `["a","b"]` / `[a, b]` / `["a":"b"]` 三种名字形态 —— 与规范的 `NameItem` 一致。

**代价见 P0-3。**

### 6.3 AST 增量

| 变更 | 位置 | 说明 |
|---|---|---|
| `TypeExpr::Bounded` **新变体** | `ast/src/lib.rs:111-117` | `name` / `bound: Box<TypeExpr>` / `span`;唯一构造点 `parser:2501` |
| `Expr::Sealed` **新变体** | `ast/src/lib.rs:467-470` | 复用 `ImportName` 列表 |
| `Expr::Match.default_synthetic` **新字段** | `ast/src/lib.rs:441-442` | `#[serde(default, skip_serializing_if = …)]`,区分「作者写了兜底」与「作者漏了分支」 |

`default_synthetic` 是 Step 8 的地基:parser 在省略 default 臂时物化一个 `NULL` 字面量并标记 `true`(`parser:1597-1614`),`matchx` 据此决定是否报 `E0116`(`matchx.rs:583-587`)。设计正确 —— **但 P0-1 恰好从 formatter 侧把它破坏了。**

### 6.4 格式化器

【静态】v0.10 改了 3 处(`formatter/src/lib.rs`):`Match` 的 `default_synthetic`(inline 路径)、`Expr::Sealed`、`TypeExpr::Bounded`。两个新节点**都渲染了**,往返成立。

但 **folded(多行)路径漏掉了 `default_synthetic`** —— 见 P0-1。`formatter/tests/formatter_tests.rs` 403 行**在 v0.10 中零改动**,新臂零测试覆盖,也无幂等/往返测试。

---

## 7. 静态层 `wlwl-types`

### 7.1 架构

5,416 行 / 7 文件,依赖方向单向:`wlwl-types → {wlwl-ast, wlwl-error, wlwl-parser}`。

| 模块 | 行 | 职责 |
|---|---|---|
| `ty.rs` | 965 | 类型 IR(13 变体)、`lub`、`satisfies_annotation`、可赋值、泛型实例化 |
| `check.rs` | 1,589 | AST 单趟遍历、期望类型下推、边界检查 |
| `sig.rs` | 1,269 | `.wll.sig` 解析/渲染/生成、`SEALED` 面、契约检查 |
| `matchx.rs` | 838 | MATCH 穷尽性 / 不可达(三值覆盖递归) |
| `diag.rs` | 525 | 12 种诊断类型 + 码映射 + 消息 |
| `env.rs` | 140 | 词法作用域链 |
| `lib.rs` | 90 | 门面 + 2 个集成锁测试 |

**分层纪律良好**:`wlwl-eval` 的 `wlwl-*` 依赖**不含** `wlwl-types` —— 运行期在**依赖图层面**就看不见 `Ty`,擦除不是靠约定而是靠架构。

### 7.2 默认关闭保证(逐行可证)

【静态】门禁在 `wlwl-cli/src/main.rs:810-814`:

```rust
if !setting.is_enabled() && !match_setting.is_enabled() {
    return out;
}
```

它在 `builtin_sig_table()`(`:832`)与 `check_program_with_options`(`:836`)**之前**返回 ⇒ 门全关时**连内建签名表都不构建**。crate 内还有第二道闸(`check.rs:157-159` 构造器 `match_exhaustiveness: false`)。

默认值 = `Off` 有四重保证:枚举 `#[default] Off`、`Setting::default()`、键缺失回落、`is_enabled() { mode != Off }`。

`wlwl check` / `wlwl run` 共用 `static_check_gate`,LSP 共用 `collect_static_diagnostics` —— **三条路径同一口径**。

**唯一例外**:`gradual_typing = "loud"`(非法值)时,`main.rs:783-808` 会先吐一条 `W0001`,**然后**才在 `:812` 提前返回。严格讲「`off` 档零诊断」在非法值场景下不成立(默认无键不受影响)。

### 7.3 类型系统的关键判断

【静态】

- **`Dynamic` 同时是顶类型和底类型**(`ty.rs:336-338`):任一侧含 `Dynamic` 则赋值恒真,`FUN` 的逆变/协变判定**退化为恒真**,代码自己承认(`check.rs:1217-1218`)。这是「不误报优先」的代价。
- **`lub` 遇 `Dynamic` 一律返回 `Dynamic`**(`ty.rs:209-211`),规则位置早于任何可赋值判定 ⇒ **只放宽不收窄**,有锁测试。
- **形参逆变是真实现了的**(`ty.rs:356-370` `b.is_assignable_to(a)`),锁测试用数值 ladder 避开 `Dynamic` 退化做双向验证。但 `Ty::Fun` 的来源极窄(只有 `from_head("FUN")` 与 `Expr::Fun` 合成),逆变路径**实际可达但窄**。
- **`OPTION[T]` 是独立变体,不是 `RESULT[T, NULL]` 的糖**(`ty.rs:45`),糖只落在注解边界的 `satisfies_annotation`(`:257-269`)。有反向保护:裸 `T` **不**满足 `OPTION[T]`。
- **格是良基的**:所有递归规则严格下降到子 `Ty`;`Ty::Var` 的 bound **不递归**(`satisfies_bound` 是纯平铺 match)。

### 7.4 泛型擦除:证据链完整

【静态】

1. 依赖图层面 `wlwl-eval` 看不见 `wlwl-types`;
2. 本 crate 唯一的 `From` 实现是 `ty.rs:405`,全 crate 对 `Value` 的引用数为 **0**;
3. 运行期锁测试三条:`generic_parameters_are_erased_at_runtime`(`eval:16388`,`TYPE(ident(42)) == "INTEGER"`)、`a_bounded_generic_behaves_exactly_like_a_plain_function`(`:16398`,与同形无注解函数**逐值相等**)、`a_bound_violation_is_not_caught_at_runtime`(`:16411`,违反约束得到 `E0030` 而非「约束错」)。

**「转回运行期」这一步不存在** —— `Ty` 只被诊断消息、`.sig` 渲染与 JSON 消费,没有任何路径把它送回 `Value`。**Step 9 的「擦除」承诺完全兑现。**

### 7.5 MATCH 穷尽性:不是 Maranget

【静态】实际是**三值覆盖查询递归**(`Verdict::{Covered, Uncovered, Unknown}`),核心是 `Unknown → 不报`(`matchx.rs:38, :361`)。没有 usefulness matrix,没有 `Σ`/`P` 列向量。

刻意**不**检查的:数组/字典穷尽性(构造子按元数无限展开)、整数/浮点/字符串穷尽性(无限域)、`Ty::Option`(显式注释「故意不枚举」)、`Dynamic`/`Fun`/`Named` 无构造子模式(全静默)。

**收敛点**:`E0116` 仅在「缺构造子 **且** default 臂被省略」时报(`:583-587`)—— 作者写了 default 就是有意兜底,符合规范 §7.6。

### 7.6 健全性:误报与漏报

**误报(对合法程序报错):**

| # | 问题 | 位置 |
|---|---|---|
| **FP-1** | **默认形参 / `*rest` 形参的调用必报 `E0111`** —— 运行期规则在 `eval:8679-8689` 正确处理了 `default_expr` 与 `is_rest`,静态层的 `check.rs:627` 只比 `params.len() != args.len()`。**已实测坐实,见 P0-2** | `check.rs:627` |
| FP-2 | `Ty::Named` 比较大小写敏感(`ty.rs:371-375`),但文档称「大小写不敏感」(`:96-98`);手写 `.sig` 配自造类型名易踩 | `ty.rs:371` |
| FP-3 | `pending_substitutions` 是单共享字段,嵌套调用可污染外层绑定 | `check.rs:146/620/671` |
| FP-5 | `.wll.sig` 尾随垃圾被静默吞成 `Named`(`EXPORT PI : STRING oops`) | `parser:2450` |

**漏报(合法但没抓到):**

| # | 问题 | 影响面 |
|---|---|---|
| **FN-1** | **`IMPORT` 的名字永不参与类型检查**(`check.rs:524-528` 一律落 `Dynamic`);`check_imports` 只比名字集(`sig.rs:679`),**签名里的类型在消费方根本没有比对路径** | **最大面积** —— 跨模块调用零检查 |
| **FN-2** | 泛型变量跨形参不做统一(`check.rs:658` 循环内新建 `bindings`) | `FUN((a: T, b: T), …)` 被异类型实参调用不报;同名 `T` 可同时是 `String` 和 `Integer` |
| FN-4 | 50/110 内建无结构化签名 ⇒ 恒 `Dynamic`;首批**只给返回类型**(`BuiltinSig::ret_only` 硬编码 `params: None`) | 内建元数与实参零检查 |
| FN-7/8 | `FOR` 循环变量、`WHILE` 恒 `Dynamic` | 循环体内无类型信息 |
| FN-9 | `TypeArityMismatch` 诊断**从未被发射** —— `ty.rs:100-103` 承诺「让 A4 能报元数不对」,但 `from_head` 的两条兜底分支产出完全相同,`is_known_head` 的判别零作用 | **承诺未兑现的死代码**;`DICT[INTEGER]` 静默通过 |

**panic 可达性**:生产代码(`#[cfg(test)]` 之外)`unwrap/expect/panic!/unreachable!` 命中数为 **0**;`Ty::from_type_expr` 是全函数(不 panic 不返回 Result);`parse_type_text` 调真 parser 但用 `?` 传播。**无用户输入可达的 panic。**

---

## 8. 运行期与模块契约

### 8.1 `wlwl-eval` diff 逐 hunk 分类

【静态】`lib.rs` +341 / −27,共 18 个 hunk。分类结果:

| 类别 | hunk | 说明 |
|---|---|---|
| (a) 纯提取 / 接线 | 4 个 | `resolve_source` 抽出、`load` 分派改写、公开 `resolve_module_file` |
| (b) 新增声明形态 | 7 个 | `LoadedModule.sealed`、`collect_sealed`、`Expr::Sealed => NULL`、`Expr::Sealed` 出现在 `expr_mentions_this` / `contains_yield` 的穷尽 match、注册表 `sig` 字段 |
| (c) **运行期语义变更** | **0 个** | — |
| (c′) 新可观察输出 | 1 个 | `eval_import` 的 `E0023` 密封分支,**仅当模块源码含 `SEALED` 时可达**;v0.9 文法产生不了该 token ⇒ **无 v0.9 程序可触达** |
| (d) 注释 / 测试 | 6 个 | 含 D-4 的 tag 说明与 2 条锁测试 |

> **判定:`wlwl-eval` 内没有任何一处运行期语义变更。I1 成立。**

(c′) 那条有一处**渲染细节不一致**:密封分支绕过 `self.diag()`,因此不带 `source_line`、`trace` 不在构造时捕获,而同文件的 `EXPORT` 未绑定名诊断走 `self.diag`。两个 v0.10 测试只断言 `.code` 与 `.message`,**不会抓到**。

### 8.2 `.wll.sig` 完全不进运行期

【静态】`wlwl-eval` 全文没有任何 `fs::read` / `is_file()` 触碰 `.sig` —— 读取侧全在 CLI(`sig_path_for` / `load_module_sig`),且唯一调用点被 `if setting.is_enabled()` 包住。**sidecar 签名是纯编译期产物。**

### 8.3 「无 sig 无 SEALED ⇒ 与 v0.9 等价」可以证明

【静态】三条支撑:

1. `sealed` 恒 `false` ⇒ `eval_import` 取 `else` 分支,该分支是 v0.9 那句的**逐字复制**;
2. 有专门反证测试 `unsealed_out_of_bounds_message_is_unchanged`,断言消息**整串相等**;
3. `env` / `exports` 的构造逻辑逐行未改(diff 只在字面量处插入 `sealed:` 字段)。

**但「零新增工作」不能证明**:每次 `load` 多一次 `collect_sealed` 顶层遍历;`run` / `check` 现在恒做两次 `wlwl.toml` 读 + 解析。语义是空操作,**I/O 不是零** —— 与「零开销」措辞有张力。

### 8.4 A6′ 首批:60/110,只给返回类型

【静态】`sig: Some(…)` 出现 **60** 次、`sig: None` **50** 次(60+50=110)。全部 60 条用 `BuiltinSig::ret_only`,该函数硬编码 `params: None`;`params: Some(…)` 在 `registry.rs` 全文**零出现**。

理由写在代码里:`SUB(s, start, end?)` / `POP(d, k, default)` / `PRINT(args...)` 的可选形参与变长实参会必然误报,与「不误报优先」冲突。

`None` 的名单被硬锁在 `unstructured_entries_stay_none_for_a_known_reason`:19 个 parser 降级名 + 30 个后续批次名逐一断言。附录 G 生成器**逐字节不变**,由 `appendix_g_regen_is_stable_and_ignores_the_structured_field` 守住。

### 8.5 `Effect::MethodCall` / `ProtocolViolation`:确认不产生

【静态】全仓库构造点只有 4 处,**全部在 `mod tests` 内**(`runtime.rs:1295/1298/1783/1787`,`#[cfg(test)] mod tests` 起于 `:997`);`lib.rs` 内无任何构造。确认发布版不产生。两条锁测试双向钉住「变体还在」与「规范明文不产生」。

> 附带修正:`protocol.rs` 的 `ProtocolCursor::Done` 文档注释原写「终态后调用 → E0051」,与实现 `lib.rs:3280-3284` 的 `E0050` 矛盾(终态算状态机不匹配,与 step-order slip 分开)。已在 D-4 收口时改正。

### 8.6 `wlwl-std` +124 行:纯增量

【静态】全部落在 crate 文档(C5′ 命名约定表)+ 3 个新测试(目录登记、每文件自报路径、命名约定)。`resolve()` / `ModuleSpec` / `SPEC` 定义零 diff,**无任何现有内建的行为、名字、绑定被修改**。

---

## 9. CLI 与工具链

### 9.1 新增 5 个子命令

| 子命令 | 分派 | 实现 | 是否只读 |
|---|---|---|---|
| `sig <file> [--format text\|json]` | `main.rs:149` | `:1079-1104` | ✅ |
| `sig-gen <file> [--force]` | `main.rs:150` | `:1117-1153` | ⚠️ 写盘(默认不覆盖已有文件) |
| `lsp` | `main.rs:151` | `lsp.rs:75` | ✅ |
| `interface <file>` | `main.rs:152` | `tooling.rs:25-51` | ✅ |
| `schema` | `main.rs:153` | `tooling.rs:107-118` | ✅ |

`SigFormat` 刻意不复用 `OutputFormat` —— 单模块签名没有有意义的 JSONL 形态。

### 9.2 LSP 能力:严格 A 档

【静态】`lsp.rs:116-123` 的 `capabilities` 逐项:`textDocumentSync: 1` / `hoverProvider: true` / `definitionProvider: true` / `completionProvider: {resolveProvider: false}` / `positionEncoding: "utf-8"`。

**`renameProvider` 与 `documentFormattingProvider` 均未广告** —— 与 D-6 的 A 档裁决一致,代码里显式注明「B 档与 C 档没被本文件关掉」。

请求分派只处理 7 个方法,其余 `_ => Vec::new()` **静默忽略**(`:155`)。因此对未广告的 `rename` / `formatting`,服务器**既不回 `-32601` 也不回结果**,请求方只会超时。刻意为之,但协议上不理想。

### 9.3 `interface` / `schema`:同一套推导,不另建

【静态】两者都走 `check_program_detailed` + `sig_from_module` + `ModuleContract::from_module`,与 `sig` / `sig-gen` / `check` **同一口径**。

`schema` 的类型清单是**手写不反射**(`tooling.rs:122-126` 自述),代价是静态层加类型头要同步改(有锁测试 `schema_type_names_match_the_static_layer` 兜底)。

---

## 10. 达成度评估(逐条对构建计划)

| 立项项 | 计划要求 | 实际 | 达成 |
|---|---|---|---|
| **A1** 类型 IR / 环境 | `TypeExpr` → IR,`Dynamic` 回退 | 13 变体 IR,`from_type_expr` 全函数 | ✅ |
| **A3** `gradual_typing` 三档 | 默认 `off` 零诊断 | 门禁早于签名表构建;两路独立开关 | ✅ |
| **A4** 容器 / 函数类型 | 逆变、OPTION 糖 | 逆变真实现;OPTION 是独立变体+边界糖;**元数不符诊断未兑现** | ⚠️ 部分 |
| **A2′** 向下传播 | 逐元素 / 逐实参 / `lub` | 容器逐元素、dict **只推 value**、CALL 逐位、`lub` 只放宽 | ✅ |
| **A6′** 注册表结构化 | 首批 60/110,只给返回类型 | 精确 60/50,`ret_only` 硬编码 | ✅ |
| **C1** 模块签名 | 旁路 `.wll.sig` | 纯编译期,真 parser 解析类型 | ✅ |
| **C2** `SEALED` | 前缀调用,非关键字 | 确非关键字;**但引入 P0-3** | ⚠️ 有缺陷 |
| **C3** 校验 + `sig-gen` | 子命令 | 5 子命令齐备 | ✅ |
| **C5′** std 命名 | 仅命名 / 文档 | 纯增量 + 3 锁测试 | ✅ |
| **P1-1** MATCH 穷尽 | 穷尽 + 不可达 | 三值覆盖;`W0117` 恒警告无 `E0117` | ✅ |
| **P1-2** 泛型限形 | 擦除,`Value` 不变 | 依赖图 + 零转换 + 3 运行期锁测试 | ✅ |
| **P1-3** 工具链薄壳 | A 档三项 | 严格 A 档,rename/format 未广告 | ✅ |
| **D-4** 效果收口 | B/B′ | 保留 tag 面 + 规范明文,双向锁测试 | ✅ |

---

## 11. 问题清单(详版)

### 11.1 P0-1 `wlwl fmt` 静默关掉 `E0116`【实测 · 阻塞】

**根因**:`formatter/src/lib.rs:211-224` 的 **inline** 路径尊重 `default_synthetic`;`:491-496` → `render_match:678-680` 的 **folded(多行)** 路径**无条件渲染 default**,而 parser 省略 default 臂时物化的是 `NULL` 字面量。

**完整复现链**(见 §13.2):

| 步骤 | 结果 |
|---|---|
| ① 非穷尽 `MATCH` + 省略 default 臂 + `gradual_typing=error` | `E0116: non-exhaustive match: missing ERR(_)` rc=1 ✅ |
| ② `wlwl fmt <file>`(folded 路径) | 产物含 `NULL` |
| ③ 把产物落盘后再 `wlwl check` | **rc=0,`E0116` 消失** |

**危害**:静态层是 opt-in 的,但**一旦开启,跑一次 formatter 就把 v0.10 自己新增的诊断关掉**。而且这恰好摧毁 Step 8 的立身之本 —— `default_synthetic` 存在的唯一理由就是区分「作者写了兜底」与「作者漏了分支」,formatter 一旦把前者物化成后者,这个区分在源码层面永久丢失。

**同时破坏 I9**:`wlwl fmt` 的可观察输出对存量 MATCH 发生了变化(省略 default 臂的 canonical 形态在 v0.10 与 v0.9 不同),而这不是「新增内容默认关闭」能解释的 —— 它是 formatter 输出的**默认变更**。

**修法**(单点):`render` 路径的 `Expr::Match` 也要解构并传递 `default_synthetic`,`render_match` 在其为真时跳过 default 渲染。**必须同时补 formatter 幂等 / 往返测试** —— 该文件 403 行测试在 v0.10 零改动,新臂零覆盖。

### 11.2 P0-2 默认形参 / `*rest` 形参误报 `E0111`【实测 · 阻塞】

**根因**:`wlwl-types/src/check.rs:627` 只比 `params.len() != arg_tys.len()`;而运行期规则 `wlwl-eval/src/lib.rs:8679-8689` 正确处理了 `default_expr` 与 `is_rest`:

```rust
let required = params.iter().filter(|p| !p.is_rest && p.default_expr.is_none()).count();
let in_range = if has_rest { arg_values.len() >= required } else { … };
```

静态层 `check.rs:766-770 param_type` 完全忽略这两个字段。

**实测**(见 §13.1):两个**运行期完全合法**的程序,在 `gradual_typing="error"` 下被判死:

| 程序 | `wlwl run` | `wlwl check`(门开) |
|---|---|---|
| `LET(g, FUN((name = "hi"), name)); g();` | ✅ 打印 `hi` / `bye` | ❌ `E0111: call arity mismatch: expected 1, found 0` |
| `LET(f, FUN((a, *rest), a)); f(1);` | ✅ 打印 `1` / `1` | ❌ `E0111: call arity mismatch: expected 2, found 1` |

**危害**:直接违反计划 A3 的「不误报」验收标准。任何用默认形参或变长形参的用户,一打开静态层就编译不过。这是本 crate **唯一一条对合法程序必报硬错的路径**。

**为什么 CI 没抓到**:全 crate 零 `default_expr` / `is_rest` 字样 ⇒ **零测试覆盖**;仓库 46 个 `.wll` 目前都没用到这两种形参。

**修法**:`check.rs:627` 的 arity 判定改用与运行期同一套规则(抽共享 helper,或至少对齐 `required` / `has_rest` 两个量)。

### 11.3 P0-3 `SEALED` 吞掉用户自定义函数【实测 · 阻塞】

**根因**:`parser/src/lib.rs:1927-1929` 在**通用表达式解析器**里按名字拦截,且**不检查位置**。v0.9 的 parser 完全没有这段(`git show v0.9.0:…` grep `SEALED|parse_sealed` 零命中)。

**实测更正**:子审查推断「静默变成 `NULL`」,**实测不成立** —— 实际是**硬解析错**,因为 `SEALED(...)` 的实参被当作名字列表解析:

| v0.9 合法程序 | v0.10 实际 |
|---|---|
| `LET(SEALED, FUN((x), x)); SEALED(41);` | `E0011: expected '[', got Integer(41)` |
| `LET(SEALED, FUN((xs), LENGTH(xs))); SEALED([1,2,3]);` | `E0010: expected identifier or string in name list, got Integer(1)` |

**危害不变且更重**:这不是静默降级,是**硬解析失败** —— 编译期直接过不去。**破坏 I8**。词法面零扩张(不新增 `SEALED` 关键字)换来的正是这个歧义,而规范 §12 只声明了「`SEALED` 不是保留名」,**没有声明这是破坏性变更**,也没有 conformance 用例覆盖。

**同时是 P1-4 的根因**:识别点在表达式层 ⇒ `PRINT(SEALED(["x"]))`、`[SEALED(["x"])]`、`FUN((): INTEGER, SEALED(["x"]))` 全部被接受为模块声明,而规范明写「只出现在模块顶层,不作为 `Expression`」。

**修法**二选一:(a) 收紧判别条件 —— 仅当处于模块顶层 Block 时才走 `parse_sealed`,否则降级为普通调用;(b) 规范 §9.1 明写这是破坏性变更 + 补 conformance 用例。**推荐 (a)**,因为 (b) 仍会让用户代码编译不过。

### 11.4 P1 级

> **2026-09-29(v0.10.1 收口):P1-1 ~ P1-5 全部已处置**,逐条对应见 §1.3。
> 下表保留为问题原文。

| # | 问题 | 证据 |
|---|---|---|
|---|---|---|
| P1-1 | 规范 §2.6 事实③码号错(`E0012` vs 实现 `E0010`);**parser 自带测试只断言消息含 "type constraint",不断言码号**,所以 CI 不会发现 | 【静态】+【实测】 |
| P1-2 | 规范「§1–§17 逐条未改」自述不实(4 处未申报就地改写,2 处新增 `[规范性]` 且落在冻结章 §16) | 【静态】 |
| P1-3 | `wlwl fmt` 可把能解析的代码改成**不能解析**的代码:`DICT<STRING>` 解析成 `Generic{name:"< STRING >", args:[]}`,formatter 渲染成 `< STRING >[]`,再解析即 `E0010`。`Generic` 空 args 的渲染形是**输出非法输入的唯一路径** | 【静态】 |
| P1-4 | `SEALED` 位置不校验(见 P0-3) | 【静态】 |
| P1-5 | 密封分支的 `E0023` 绕过 `self.diag()`,缺 `source_line`、trace 不在构造时捕获;两个测试只断言 code+message,抓不到 | 【静态】 |

### 11.5 P2 级

> **2026-09-29(v0.10.1 收口)**:P2-1 ~ P2-5 与 P2-7 已处置,逐条对应见 §1.3;
> **P2-6 仍挂**(既有行为,与 v0.10 无关)。下表保留为问题原文。

| # | 问题 |
|---|---|
| P2-1 | `BoundedVar` 可任意右嵌套(`T: A: B`),规范文法不允许 —— `parse_expr` 自递归(`parser:2500`) |
| P2-2 | 类型层的 `is_ident` 只认 ASCII(`parser:2582-2589`),与 lexer 支持非 ASCII 标识符不一致 ⇒ `LET(x: 整数, 1)` 报 `E0010` |
| P2-3 | `INTEGER: Comparable` 落成 `Ty::Var` 而非 `Ty::Integer`(`ty.rs:571-589` 的 `is_known_head` 不含基类型),且 `ty.rs:113-117` 注释声称「parser 层已挡」而实测未挡 |
| P2-4 | 类型错误(T2–T5)绕过 `err_at` ⇒ 无源码行、无 suggestion,位置指向注解起点而非出错 token |
| P2-5 | `TypeArityMismatch` 诊断从未被发射(死代码 + 未兑现承诺);`schema` 类型清单手写不反射 |
| P2-6 | 非末条顶层语句的 `ERR` 被静默丢弃(既有,与 v0.10 无关,收口期已发现) |
| P2-7 | 附录 A 的文法**零锁测试**(§11.2/§11.3 有双向锁测试,文法没有) |

---

## 12. 已登记偏差

`docs/history/deviations-v0.10.md` + `docs/history/deviations-v0.10.md` 现有 D10-001…D10-011,除 D10-011(`cargo deny`,既有)与 D10-010(已修复)外均为「接受保留」。**D10-005(箭头形式不可达且不报错)在类型注解位经实测仍然准确** —— 这一点在 §4.4 D3 中做了区分。

**建议新登记**(待用户裁决,本次未改任何代码或规范):

> **2026-09-29 更新(R10-063 回填):下表的「建议号」**全部作废** —— 编号与实际
> 登记撞了。`D10-012` / `D10-013` 在 v0.10.1 里实际登记的是
> **附录 G 签名分叉**与 **`ARRAY(items...)` 幻影 API**两条(见
> `docs/history/deviations-v0.10.md`),不是本表建议的内容。
>
> 本报告的六条建议**一条都没有按建议号登记**,原因是:它们在 v0.10.1 里被
> **逐条处置**了 —— 三条 P0 修在 `c0d8652`,P1 / P2 修在 v0.10.1 修复计划的
> 各 Step(对应关系见 §1.3 的「对应」列)。**修掉的缺陷不需要登记为偏差**,
> 偏差台账记的是「已知但保留」或「按计划偏离」;真正需要登记的是行为变更,
> 那几条(R10-012 实例身份 / R10-023 IMPORT 签名类型参与检查 / R10-014 通道
> 唤醒顺序)按 v0.10.1 计划书附录 A 的要求进 `CHANGELOG.md` §0.4
> 兼容性承诺表,在 Step 13 收口。
>
> 下表保留为**当时的建议记录**,内容本身仍然成立,只是号与去向都变了。

| 当时的建议号 | 内容 | 依据 | 实际去向 |
|---|---|---|---|
| ~~D10-012~~ | 默认形参 / `*rest` 形参的静态 arity 误报 | P0-2,实测 | ✅ 已修,未登记(`c0d8652`) |
| ~~D10-013~~ | `wlwl fmt` folded 路径物化 default 臂,抑制 `E0116` | P0-1,实测 | ✅ 已修,未登记(`c0d8652`) |
| ~~D10-014~~ | `SEALED` 与用户自定义函数名冲突,v0.9 合法程序变解析错 | P0-3,实测 | ✅ 已修,未登记(`c0d8652`) |
| ~~D10-015~~ | 规范 §2.6 事实②③ 措辞/码号过时于实现 | P1-1,实测 | ✅ 已修(R10-040),未登记 |
| ~~D10-016~~ | 规范兼容承诺自述(§1–§17 逐条未改)不实 | P1-2,静态 | ✅ 已修(R10-041),未登记 |
| ~~D10-017~~ | `Generic{name, args:[]}` 渲染形产出不可重解析源码 | P1-3,静态 | ✅ 已修(R10-020),未登记 |

---

## 13. 实测复现方法

全部在 `impl/target/debug/wlwl.exe` 上执行,临时目录已清理。

### 13.1 P0-2:默认形参 / `*rest` 误报

```powershell
# 目录内 wlwl.toml(注意必须含 [package] 段,否则清单不被读取)
#   [package] name="fp1" version="0.0.1" entry="main.wll"
#   [features] gradual_typing = "error"

# default_param.wll
LET(g, FUN((name = "hi"), name));
PRINT(g());
PRINT(g("bye"));

# rest_param.wll
LET(f, FUN((a, *rest), a));
PRINT(f(1));
PRINT(f(1, 2, 3, 4));
```

| 命令 | 结果 |
|---|---|
| `wlwl run default_param.wll` | `hi` / `bye`,rc=0 |
| `wlwl run rest_param.wll` | `1` / `1`,rc=0 |
| `wlwl check default_param.wll` | `E0111: call arity mismatch: expected 1 argument(s), found 0`,rc=1 |
| `wlwl check rest_param.wll` | `E0111: call arity mismatch: expected 2 argument(s), found 1`,rc=1 |

### 13.2 P0-1:`wlwl fmt` 抑制 `E0116`

```powershell
# wlwl.toml:gradual_typing="error" + match_exhaustiveness="error"
# e0116_folded.wll
LET(r, ERR("x"));
PRINT(MATCH(r, [
  [OK("aaaa…(够长以逼出 folded 路径)"), "ok"]
]));
```

| 步骤 | 命令 | 结果 |
|---|---|---|
| ① | `wlwl check e0116_folded.wll` | `E0116: non-exhaustive match: missing ERR(_); the omitted default arm yields NULL for those values`,rc=1 |
| ② | `wlwl fmt e0116_folded.wll` | 产物含 `NULL` |
| ③ | 产物落盘 → `wlwl check after_fmt.wll` | `OK: parsed`,**rc=0** |

### 13.3 P0-3 与 D3 精确化:`SEALED` 碰撞 + 箭头/尖括号分位

| 输入 | 结果 |
|---|---|
| `LET(SEALED, FUN((x), x)); SEALED(41);` | `E0011: expected '[', got Integer(41)` |
| `LET(SEALED, FUN((xs), LENGTH(xs))); SEALED([1,2,3]);` | `E0010: expected identifier or string in name list, got Integer(1)` |
| `LET(g: FUN(INTEGER) -> STRING, "s"); PRINT(g);` | **静默通过**,`run` 打印 `s` ← D10-005 准确 |
| `LET(g, FUN((x: INTEGER) -> STRING, "s"));` | `E0012: expected ',', got Minus` |
| `LET(d: DICT<STRING, INTEGER>, 1);` | `E0011: expected ')', got Gt`(位置指向注解**之后**的 `)`) |
| `LET(xs: ARRAY, [1]);` | `E0010: expected '[' after 'ARRAY' in type expression` |
| `LET(d: DICT, ["a":1]); PRINT(d);` | 静默通过(不透明具名类型) |
| `LET(f, FUN((a: ARRAY[INTEGER]: Comparable), a));` | `E0010`(规范称 `E0012`) |

### 13.4 关键字表 / 保留形式零变化

```powershell
git show v0.9.0:docs/standard/wlwl-spec-v0.9.md   # vs 现 docs/standard/wlwl-spec-v0.10.md
```
逐行比对 §1.4 / §1.5 / §12:**仅新增说明性文字,无表格行增删**。

---

## 14. 未能核实

1. ~~子审查对 `DICT<STRING, INTEGER>` 的静态追踪(报在哪个 token)与 formatter 往返后果 —— **仅静态推演**,未用 fixture 实跑 `wlwl fmt` 复现 P1-3~~
   → **已消除**:v0.10.1 的 R10-020 把它复现并修掉了(parser 兜底吸收现在保留
   整个类型头,`wlwl fmt` 往返可重解析),并补了锁测试。
2. 密封分支 `E0023` 的 `trace` 是否在更上层 `enrich_with_trace` 回填 —— 未逐行追完顶层 `eval` 的 Err 包装链;`source_line` 缺失可确定。
   → **部分消除**:`source_line` 已由 R10-022 补上(改走 `self.diag()`);
   `trace` 的回填路径仍未逐行追完。
3. `default_synthetic` 的 `skip_serializing_if` 是否真保住 `stable.rs` 内容哈希 —— 静态读来成立,需跑 `stable_tests`;
4. FP-3(`pending_substitutions` 跨调用污染)未能构造确定性触发用例;
   → **已消除**:R10-028 一并清掉了跨调用残留(每次调用先 `clear()`)。
5. `E0027` / `E0095` / `E0096` 的「无触发路径」是规范自述,未在 impl 中找到对应断言。
6. ~~附录 A 文法与 parser 的对应关系**目前无机器校验**,只靠人工维护~~
   → **已消除**:v0.10.1 的 R10-050 加了
   `crates/wlwl-parser/tests/spec_grammar_consistency.rs` 五条锁测试
   (悬空 / 重复定义 / §9.2↔附录 A 一致 / 关键字入口齐全 / `ModuleDecl` 可达),
   并已用它逮住 D5 / D6 两处漂移。

---

## 15. 建议处置顺序

> **2026-09-29:第 1–3 项(P0)已全部完成**,见 §0.1;第 4–7 项(P1 / P2)在
> v0.10.1 修复计划里也已全部完成,逐条对应见 §1.3。下表保留为处置记录,
> 「状态」列按 v0.10.1 收口时的实况回填。

| 顺序 | 项 | 理由 | 状态 |
|---|---|---|---|
| 1 | **P0-1** | 唯一会**静默丢失**用户已经获得的检查能力,且破坏 I9;单点修复 + 必须补幂等测试 | ✅ 已修 |
| 2 | **P0-2** | 唯一「对合法程序必报硬错」的路径,直接违反 A3 验收;单点修复 + 补两种形参的夹具 | ✅ 已修 |
| 3 | **P0-3** | 破坏 I8(硬解析错);需在「收紧判别」与「申报破坏性变更」之间裁决,建议前者 | ✅ 已修 |
| 4 | P1-1 / P1-2 | 可证伪的规范错误,改动量小(各一处措辞 + 一处码号 + 一处落位清单) | ✅ 已修(R10-040 / R10-041) |
| 5 | P1-3 / P1-4 | 需与 P0-3 一并考虑(同在 parser 的类型/声明解析面) | ✅ 已修(R10-020 / R10-021) |
| 6 | P2-7(附录 A 文法锁测试) | 把 P1-1 / D5 / D6 这类「规范↔实现」漂移从人工核对变成机器守住,收益最高的一项预防性投入 | ✅ 已修(R10-050) |
| 7 | P2 其余 | 逐条,可不阻塞 v0.10.1 | ✅ P2-1~P2-5 已修(R10-025~029);⬜ **P2-6 仍挂**(既有,与 v0.10 无关) |

**发版结论**:三个 P0 均为单点修复,已全部完成并复验,**v0.10.0 可以发**。
P1 / P2 也已在 v0.10.1 全部处置完毕(唯一遗留 P2-6),都不破坏
「v0.9 程序默认可观察行为不变」。

> **本节的后续教训(R10-064 记录)**:v0.10.1 修 R10-064 时发现,附录 G 的
> 签名在 spec / 注册表 / mirror 三份副本之间分了 37 条,其中**注册表这一侧
> 有 12 处说的与实现相反**。本报告 §12 建议的「D10-012 起连续编号」之所以
> 全部作废,也属于同一类问题 —— **编号是引用,一旦发出去就有人按它找**。
> 台账的「连续递增」约定只在**实际登记**时成立,建议号不算。
