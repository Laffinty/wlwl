# WLWL v0.11.2 构建计划

> **本文件是一份执行契约(execution contract),不是说明书。**
> 它的读者是**下一个接手这份工作的 AI agent**,而不是评审的人。
> 文件格式本身是本仓库的**首版范式**,§0 写明了格式契约与修改规则 —— 沿用它们,不要另起一套。

| | |
|---|---|
| 批次 | v0.11.2 |
| 语言规范 | [`../spec/wlwl-spec-v0.11.md`](../spec/wlwl-spec-v0.11.md)(v0.11 冻结,本批不改语言语义) |
| 标准库规范 | [`../stdlib/wlwl-stdlib-spec-v0.11.md`](../stdlib/wlwl-stdlib-spec-v0.11.md)(随本批**升版到 v0.12**) |
| 起草日 | 2026-10-02 |
| 起草基线 | `main@c7a3c88` · 二进制 `wlwl 0.11.1` |
| 上游依据 | [`../review/wlwl-v0.11.1-recheck.md`](../review/wlwl-v0.11.1-recheck.md)(v0.11.1 复核报告) |

---

## §0 使用契约

这一节决定本文件怎么被使用、怎么被修改。**先读它,再读任何工作项。**

### 0.1 三条阅读规则

**R1 — 代码现状以命令为准,不以本文档为准。**
凡陈述「代码现在是这样」的行,右列都配了一条能把它**推翻**的命令。命令红了 = 那一行已过期。
遇到冲突时,**信命令,不要信本文档**;并且顺手把本文档改过来(见 W3)。

**R2 — 「决策」行的唯一用途是阻止重新论证。**
标了 `已裁决` 的行,除非你能指出**新的反证**,否则不要重开讨论。
确有反证时的正确动作是:**改代码 + 登记偏差(D12-xxx)**,而不是只把文档改软。

**R3 — 工作项之间没有隐式顺序。**
`W-xx` 之间只通过各自的 `依赖` 字段关联。执行 `W-09` 不需要先读 `W-03`。
每个工作项自身就是完备的:它带着做这件事所需的全部事实。

### 0.2 三条写作规则(修改本文件时必须遵守)

| | 规则 | 反例 |
|---|---|---|
| **W1** | 新增任何陈述 → 必须同时给出**地址**(`file:line`)或**命令**。 | ❌「标准库的字符串支持较弱」 |
| **W2** | 新增任何「已完成」→ 必须给出产生它的 **commit SHA**。 | ❌「M1 已完成」 |
| **W3** | 新增任何「已核实」→ 必须给出**原始输出**,不得只写结论。 | ❌「实测确认 N-1 属实」 |

### 0.3 为什么是「可证伪优先」而不是「叙述优先」

这条格式不是审美选择,是被一次具体失败逼出来的:

> v0.11.1 复核报告共给出 5 条新发现(N-1~N-5)。本计划起草时逐条复核实测,
> 结果是 **4 条属实、1 条(N-4)不实**;同时,报告用于验证「三个 P1 已修」的
> **多条探针原样跑不起来**(缺 `IMPORT`、引用了不存在的全局名)。
> 也就是说:**那份报告里的「实测 ✓」有相当一部分是不可复现的断言,不是证据。**

一个 agent 读计划时会**默认相信**「已核实」标记。若那个标记底下是一次失败或未跑的探针,
它会基于一个假前提动手,产出第二个假结论。所以本文件把**「可被推翻」**当作一等公民:
每条事实自带证伪命令,每条待验证假设显式标注 `待验证`。

### 0.4 状态标记词表(全文件统一)

| 标记 | 含义 | 允许的动作 |
|---|---|---|
| `已核实` | 本计划起草时实跑过,附原始输出与日期 | 可直接依赖 |
| `待验证` | 合理推断,**未跑过** | 动手前必须先跑证伪命令 |
| `已裁决` | 有人做过决定,重启讨论需新反证 | 直接执行 |
| `挂起` | 明确推给后续批次 | 不要在本批实现 |

---

## §1 基线指纹

### 1.1 环境

| 项 | 值 | 复核命令 |
|---|---|---|
| 语言规范版本 | v0.11(冻结) | `Get-Content docs\spec\wlwl-spec-v0.11.md -TotalCount 5` |
| 编译器版本 | 0.11.1 | `impl\target\release\wlwl.exe --version` |
| 工具链 | cargo 1.96.0 | `cargo --version` |
| 工作区成员 | 10 个 crate | `Get-ChildItem impl\crates -Directory` |
| probe 用例目录数 | 146 | `(Get-ChildItem impl\tests\probe\cases -Directory).Count` |
| 附录 G 全局条目数 | 106 | `Get-Content docs\appendix_G.md \| Select-String '^\| \`'` |

> **`已核实`(2026-10-02)**:`cargo --version` → `cargo 1.96.0 (30a34c682 2026-09-25)`;
> `wlwl --version` → `wlwl 0.11.1`;probe 目录数 → `144`。

### 1.2 标准库导出面(全量,本批的改动基线)

> **`已核实`(2026-10-02)**,来源:`wlwl interface <file>` + 各 `src/*.rs` 的 `SPEC.functions`。
> 这是本批所有「加成员」工作的**起点**,不是从规范文档抄来的。

**R1 层(`.wll` 门面,`impl/crates/wlwl-std/wl/std/`)**

| 模块 | 成员数 | 成员 |
|---|---:|---|
| `wlwl:std.collection` | 17 | `MAP` `FILTER` `REDUCE` `SORT` `SORT_BY` `ZIP` `RANGE` `ANY` `ALL` `FIND` `ENUMERATE` `TAKE` `DROP` `FLAT` `UNIQ` `GROUP_BY` `JOIN` |
| `wlwl:std.math` | 11 | `ABS` `MIN` `MAX` `FLOOR` `CEIL` `ROUND` `SQRT` `POW` `CLAMP` + 常量 `PI` `E` |
| `wlwl:std.str` | 5 | `JOIN` `SPLIT_LINES` `CHAR_AT` `COUNT` `QUOTE` |
| `wlwl:std.test` | 6 | `TEST` `ASSERT` `ASSERT_EQ` `ASSERT_NEQ` `RUN_TESTS` + 值 `EXPECT_ERR` |

**R2 层(Rust 原生,`impl/crates/wlwl-std/src/`)**

| 模块 | 成员数 | 成员 | 门控 |
|---|---:|---|---|
| `wlwl:std.io` | 3 | `PRINT` `INPUT` `PRINT_ERR` | 默认 |
| `wlwl:std.format` | 1 | `FORMAT` | 默认 |
| `wlwl:std.json` | 2 | `PARSE` `STRINGIFY` | 默认 |
| `wlwl:std.fs` | 3 | `READ_FILE` `WRITE_FILE` `EXISTS` | 默认 |
| `wlwl:std.agent` | 5 | `TASK` `TOOL` `CALL_TOOL` `MODEL` `CONTEXT` | 默认 |
| `wlwl:std.ai` | 5 | `ASK` `EMBED` `COMPLETE` `ASK_STREAM` `ASK_ALL` | **`real-ai` feature** |

**全局内建(无需 IMPORT)共 106 条**,与本批强相关的子集:

- 字符串 15 条:`UPPER` `LOWER` `SUB` `REPLACE` `SPLIT` `TRIM` `TRIM_START` `TRIM_END`
  `STARTS_WITH` `ENDS_WITH` `REPEAT` `PAD_START` `PAD_END` `CODEPOINTS` `FROM_CODEPOINTS`
- 数组 8 条:`PUSH` `SHIFT` `UNSHIFT` `SLICE` `CONCAT` `CONTAINS` `INDEX` `REVERSE`
- 类型 7 条:`LEN` `STR` `INT` `FLOAT` `TYPE` `BOOL` `CALL`
- 比较/运算以**函数形式**存在,无中缀:`==` `!=` `>` `<` `>=` `<=` `+` `-` `*` `/` `%` `&&` `||` `NEG`

> **⚠ 陷阱(本批反复踩到)**:`STRINGIFY` 属于 `wlwl:std.json`、`SUB`/`SORT`/`RANGE` 等
> 取决于所在模块,它们**不是全局**。附录 G 只列全局条目;模块成员要看上表。
> 复核报告的多条探针正是栽在这里。

### 1.3 已知能力缺口(实测,构成本批立项依据)

| 编号 | 缺口 | 证伪命令(在 temp 目录建 `.wll` 后 `wlwl run`) | 实测输出 |
|---|---|---|---|
| GAP-1 | **大小写映射仅 ASCII**。`UPPER`/`LOWER` 不做 Unicode 折叠 | `PRINT(UPPER("straße"), LEN(UPPER("straße")))` | `STRAßE 6`(期望 `STRASSE 5`) |
| GAP-2 | **无 Unicode 规范化**。NFC 与 NFD 判不等 | `LET(a,FROM_CODEPOINTS([233])); LET(b,FROM_CODEPOINTS([101,769])); PRINT(LEN(a),LEN(b),==(a,b))` | `1 2 FALSE` |
| GAP-3 | **无字符串检索内建**。`FIND` 只吃数组 | `PRINT(FIND("hello","el"))` | `error[E0020]: undefined name FIND` |
| GAP-4 | **无任何时间能力** | `PRINT(NOW())` | `error[E0020]` |
| GAP-5 | **无编码能力**(base64/hex/url 均无) | `PRINT(BASE64_ENCODE("x"))` | `error[E0020]` |
| GAP-6 | **`std.str` 只有 5 个成员**,而 15 条字符串操作是全局内建 —— 门面层几乎没有承载 Unicode 语义的地方 | 见 §1.2 | — |
| GAP-7 | **数组类成员平方级成本**(D11-012 未修):`PUSH` 每调用一次复制整个不可变数组 | 100 万次循环实测 0.575 s | 见 `docs/plan/README.md` 下一迭代栏 |

---

## §2 上游复核报告的逐条判定

对 [`wlwl-v0.11.1-recheck.md`](../review/wlwl-v0.11.1-recheck.md) 的每条新发现,本计划实跑复核。
**这是本批第一个里程碑的输入。**

| 编号 | 报告称 | 本计划判定 | 实测证据 |
|---|---|---|---|
| **N-1** (P1) | skill 的 `SUB` 迁移公式 `SUB(s, start, NEG(-(end_old, start)))` 产出负长度 | ✅ **属实** | `SUB("Hello, world", 7, -(12, 7))` → `world`;`SUB("Hello, world", 7, NEG(-(12, 7)))` → `""`;`NEG(-(12, 7))` → `-5` |
| **N-1′** | —— | 🔴 **报告漏掉的同源缺陷** | 报告推荐的替代写法 `SLICE(s, start, end_old)` 对字符串直接 `E0033`→ 实测 `error[E0030]: SLICE: expected ARRAY as first arg, got string` |
| **N-2** (P3) | 「防死注入的反向守卫」只有 `KERNELS ⊆ 注入` 方向,无「注入了但无调用点」检查 | ✅ **属实** | `stdlib_mirror.rs` 的测试集共 8 条,无一条读 `.wll` 源码文本找调用点 |
| **N-3** (P3) | 全局 `FORMAT(OK(1))` 渲染 `OK(1)`,`std.format` 成员渲染 `1` | ✅ **属实** | 全局路径 → `OK(1)`;`IMPORT("wlwl:std.format",["FORMAT"])` 后 → `1` |
| **N-4** (P3) | skill 退出码表只列 0/1/101,缺 2/3 | ❌ **不实** | `wlwl-skill\reference.md:277-291` 已完整列出 `0/1/2/3/101` 五档,附 2026-10-01 实测注记 |
| **N-5** (P3) | `E0014` 诊断 span 指向文件首而非 `YIELD` 调用点 | ✅ **属实** | `YIELD()` 在第 3 行,诊断报 `-->` 指向 `1:1` |

### 2.1 对报告「已修」判定的抽查(防止继承假结论)

| 上轮编号 | 报告判定 | 本计划实测 | 结论 |
|---|---|---|---|
| D-1 (P1) | `NEG`/`ABS(INT_MIN)` → `E0034` | 三条路径(门面 `ABS` / `NEG` 内建 / `-` 运算符)消息逐字一致 | ✅ 属实 |
| D-4 (P1) | `SORT` 比较器 `ERR` 传播 | `SORT([2,1],cmp)` → `IS_ERR=TRUE`、`TYPE=RESULT`、`ERR_PAYLOAD="cmp-boom"` | ✅ 属实 |
| S-P1-1 (P1) | `real-ai` 特性可编译(**报告未本地构建**) | `cargo check --locked -p wlwl-std --features real-ai` → `Finished dev profile in 4.58s`,`EXIT=0` | ✅ 属实(本计划补齐了动态证据) |
| S-P1-2 (P1) | 测试体内 `YIELD()` 不再假通过 | 顶层 `YIELD()` → `E0014` 整个运行终止;反向守卫存在于 `test_contract.rs:410-436` | ✅ 属实(任务内变体未复现,见 D12-003) |
| S-P1-3 (P1) | compat 三处改回 | 补 `IMPORT` 后 `STRINGIFY(OK(1))`→`1`、`FORMAT`→`1`、整数键 → `E0030` | ✅ 属实,**但报告的探针原样跑不起来** |

> **结论**:报告的判定**方向正确**,但其中 3 条探针缺前置(缺 `IMPORT` / 用了不存在的全局名)。
> 这正是 W-06 立项的原因。

---

## §3 范围契约

### 3.1 目标

1. 清掉 §2 中 4 条已核实的缺陷(N-1、N-1′、N-2、N-3、N-5)。
2. 把标准库从「能跑通示例」推到「能写真实程序」:补齐**编码**、**Unicode 文本**、**集合**、**数值**四块。
3. 留下足够强的**反向守卫**,使每条修复与每个新成员都不可静默退化。

### 3.2 非目标(明确不做,违反即视为范围事故)

- **不改语言语义**。v0.11 语言规范冻结。本批只加**模块成员**与**文档**,不加内建、不改诊断码语义。
  唯一的例外是 W-08 的 span 修正(只改诊断定位,不改诊断本身)。
- **不复活 `std.web`**。已于 2026-10-01 宣告失败并回滚,复活前置条件见 `docs/history/20261001.md`。
- **不做 `std.iter` / 惰性视图**。它依赖「批次 B `YIELD` 续体保存」,而后者 **0 细化**。
  在依赖解除前实现惰性迭代,只会造出第二个 `std.web`。见 §5.6。
- **不引入第三方依赖**。沿用 ADR-0022 的零依赖画像。编码/哈希/随机数全部手写或用 Rust `std`。
- **不修 D11-012**(数组平方级成本)。那是 R0 解释器工作,不在本批。
- **不碰 `std.agent` / `std.ai`**。

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

外加:**§2 全部 5 条判定**复测通过、**§4 全部反向守卫**为绿、**§8 检查点**全部标记完成。

---

## §4 批次 0:缺陷修复(M0)

> 依赖:无。可与 M1 并行。
> 每个工作项的 `验收` 都是一条可复制的命令;`守卫` 是让该修复**永久不可退化**的那条测试。

---

### W-01 · 修正 skill 的 `SUB` 迁移公式

| 字段 | 内容 |
|---|---|
| 依赖 | 无 |
| 前置断言 | `wlwl-skill\reference.md:338` 与 `wlwl-skill\SKILL.md:631` 各有一处错误公式 —— `已核实` |
| 缺陷 | 公式 `SUB(s, start, NEG(-(end_old, start)))` 产出**负长度**,`SUB` 返回 `""`。散文自相矛盾:前半句说「length = `end_old − start`,所以 `-(end_old, start)`」(对),后半句说「所以用 `NEG` 取负」把刚算对的长度再取负 |
| 触点 | `wlwl-skill\reference.md:338`;`wlwl-skill\SKILL.md:631` |

**实现指令**

1. 两处统一改为:**`SUB(s, start, end_old)` → `SUB(s, start, -(end_old, start))`** —— 删掉 `NEG(...)` 包裹。
2. **同时删除 `SLICE` 替代方案**(N-1′):`SLICE` 首参必须是 `ARRAY`,对字符串报 `E0030`。
   在原句位置替换为:`SUB` 已经是全局内建(附录 G §10.5),字符串取 `[start, end)` 只能写 `SUB(s, start, -(end, start))`;
   `SLICE` 的 start/end 语义仅对数组成立。
3. `reference.md:339` 的实测注记同步更新(它现在描述的是被删掉的错误公式)。

**验收**

```powershell
$p = Join-Path $env:TEMP "w01.wll"
@'
PRINT(SUB("Hello, world", 7, 5));
PRINT(SUB("Hello, world", 7, -(12, 7)));
'@ | Set-Content $p -Encoding UTF8
impl\target\release\wlwl.exe run $p
# 期望:两行都是 world,rc=0
```

```powershell
# 注意:不要用裸模式 'NEG\(-\(end_old' 做验收 —— 它会把「不要这样写」的
# 警示句本身也算成命中。按**迁移箭头形式**匹配才有判别力。
Select-String -Path wlwl-skill\reference.md,wlwl-skill\SKILL.md -Pattern 'SUB\(s, start, NEG\('
# 期望:0 命中
Select-String -Path wlwl-skill\reference.md,wlwl-skill\SKILL.md -Pattern 'SLICE\(s, start, end_old\)'
# 期望:0 命中
Select-String -Path wlwl-skill\reference.md,wlwl-skill\SKILL.md -Pattern 'SUB\(s, start, -\(end_old, start\)\)'
# 期望:2 命中(reference.md + SKILL.md 各一)
```

```powershell
# 门禁本体:示例红了就是非零退出
& .\impl\target\release\wlwl.exe run wlwl-skill\examples\sub_migration.wll
echo "rc=$LASTEXITCODE"   # 期望 0
```

**守卫**:`wlwl-skill\examples\` 下新增 `sub_migration.wll`,含上面两条断言;
并在 skill 的示例自检里跑它(随包 18 示例的既有机制)。**这是「迁移式产出预期子串」的可执行断言** ——
报告建议但没做的那一步。

**非目标**:不改 `SUB` 本身的实现;不动附录 G。

---

### W-02 · 补「注入了但没有调用点」的反向守卫

| 字段 | 内容 |
|---|---|
| 依赖 | 无 |
| 前置断言 | `CHANGELOG.md` v0.11.1 声称「已移除死注入**并补反向守卫**」;现有守卫只有 `KERNELS ⊆ 注入` 方向 —— `已核实` |
| 缺陷 | 声明与代码不符。若有人把死内核同时加回 `KERNELS` 与注入表,全部门禁照绿 |
| 触点 | `impl/crates/wlwl-eval\src\stdlib_mirror.rs`(紧邻 `every_shared_kernel_is_actually_injected`) |

**实现指令**

新增一条测试 `every_injected_kernel_is_called_in_its_module_source`:

- 对每个 `StdSource { path, source, kernels }`,断言 `kernels` 里的每个名字**在 `source` 文本里出现过**(允许 `_` 前缀形式,如注入 `_KIND` 时源码写 `_KIND(...)`)。
- 失败信息要指名:哪个模块注入了哪个 kernel、源码里没有。
- 与既有 `the_line_scanner_agrees_with_the_ast_export_walker` 同风格(该测试是仓库已有的「交叉校验」范式)。

**验收**

```powershell
cargo test --locked -p wlwl-eval every_injected_kernel_is_called_in_its_module_source
# 期望:1 passed
```

**守卫**:这条测试**本身**就是守卫。额外加一条负向自检:在测试内构造一个虚拟
`StdSource`(`kernels: [("_NOT_CALLED", ...)]`,`source: "LET MUT(x, 0), SET(x, 1)"`),
断言它**被拒** —— 证明这条守卫不是恒真断言(仓库在 D11-006 踩过「恒真锁」的坑)。

**非目标**:不删除任何现存 kernel;不改 `KERNELS` 表。

---

### W-03 · 裁定并处置 `FORMAT` 对 `RESULT` 的双路径

| 字段 | 内容 |
|---|---|
| 依赖 | 无 |
| 前置断言 | 全局 `FORMAT("{0}", OK(1))` → `OK(1)`;`std.format` 成员 → `1` —— `已核实` |
| 缺陷 | stdlib 规范 §4 称「同一函数」,在 `RESULT` 值上不成立 |
| 触点 | `docs/stdlib/wlwl-stdlib-spec-v0.11.md` §4 |

**已裁决**:**不做行为统一,只补口径。**
理由:全局 `FORMAT` 与成员 `FORMAT` 分属两条不同实现路径,统一要动 eval 侧与 std 边界两处,
属于行为变更而非文档债;而 `OK(1)` 这类字面输出对用户并不罕见。**本批只把事实写清楚**,
行为是否统一留给后续单独裁决。

**实现指令**:stdlib 规范 §4 增补一句:全局 `FORMAT` 走 display 路径,`RESULT` 按其
`OK(v)` 字面形态渲染;`wlwl:std.format` 成员经 std 边界,`OK(v)` 解包为 `v`。
**明写「这是刻意的差异,不是 bug」**,并给出各自的适用场景。

**验收**:`cargo test --locked -p wlwl-eval` 全绿,且规范文本含该句。

**守卫**:`impl/crates/wlwl-eval\tests\` 新增一条契约用例,分别锁住两条路径的当前输出
(`OK(1)` 与 `1`)。**目的是防止后续 agent「好心统一」**,把一个有意保留的差异改掉。

**非目标**:不改 `format.rs` / eval 侧任何代码。

---

### W-04 · 修正 `E0014` 的诊断 span

| 字段 | 内容 |
|---|---|
| 依赖 | 无 |
| 前置断言 | `YIELD()` 位于第 3 行,诊断 `-->` 指向 `1:1` —— `已核实` |
| 缺陷 | span 指向文件首,用户无法从诊断定位到出错调用点 |
| 触点 | `YIELD` 的求值点(在 eval 侧);`W0051` 的 span 仍为 `0:0`(同批一并处理) |

**实现指令**

- `YIELD` 的 `E0014` 携带**当前表达式**的 span,而非文件首 span。
- 同步修 `W0051`(deprecated alias 提示)的 `0:0` span。
- 若实现层拿不到调用点 span,**不要伪造**:改为把 `E0014` 的 span 定义为「所在语句起点」,
  并在本行注释里写清这是降级口径。

**验收**

```powershell
$p = Join-Path $env:TEMP "w04.wll"
@'
PRINT("before");
YIELD();
PRINT("after");
'@ | Set-Content $p -Encoding UTF8
impl\target\release\wlwl.exe run $p 2>&1 | Select-String '\-\->'
# 期望:指向 w04.wll:3:1(而非 1:1)
```

**守卫**:契约用例锁住 span 的**行号**,不锁消息文本(消息易改,行号是要修的东西)。

**非目标**:不改 `E0014` 的消息文本与退出码(退出码 1 已正确)。

---

### W-05 · 更正复核报告的失真记录

| 字段 | 内容 |
|---|---|
| 依赖 | 无 |
| 触点 | `docs/review/wlwl-v0.11.1-recheck.md` |

**实现指令**:在复核报告**顶部**加一段「2026-10-02 复核实测更正」,逐条写明:

- **N-4 不实** —— `reference.md:277-291` 早已收录 2/3 两档退出码;
- **N-1 不完整** —— 报告推荐的 `SLICE` 替代方案对字符串报 `E0030`;
- **报告的探针有 3 处缺前置**(`STRINGIFY` 缺 `IMPORT`;`SORT`/`INT_MIN` 被当成全局)。

> **理由**:报告已被写进 `docs/`,下一个 agent 会读到它。把已知的失真留在原地 = 下一个 agent
> 会重做一遍核实(本批已经花掉了这份时间)。**更正记录必须留在原文件,不是新开一份。**

**验收**:`docs\review\wlwl-v0.11.1-recheck.md` 含「复核实测更正」小节,且 §3.2 的 N-4 条目被交叉引用。

**非目标**:不改报告的原有正文(历史记录不改写)。

---

### W-06 · 给复核探针立一条规矩

| 字段 | 内容 |
|---|---|
| 依赖 | W-01, W-05 |
| 触点 | `wlwl-skill\`(证据块书写规范)、`CONTRIBUTING.md` |

**实现指令**:新增一条约定并写进 `CONTRIBUTING.md` 与 skill 的验证章节:

> **任何进入文档的探针,必须能在 v0.11.1 发布二进制上原样跑通并附 rc。**
> 引用模块成员的探针必须自带 `IMPORT` 行;不确定某名字是不是全局内建时,
> 先查 `docs/appendix_G.md`,而不是假设。

**验收**:`Select-String CONTRIBUTING.md -Pattern 'IMPORT'` 命中。

**守卫**:无自动守卫;这是流程约定。**若将来 probe 体系支持「原样执行文档中的代码块」,
应把这条变成 CI 检查。**

---

### W-07 · 复核报告未复现项的处置

| 字段 | 内容 |
|---|---|
| 依赖 | 无 |

**已裁决**:报告 §1.2 声称的「任务内调用 `RUN_TESTS` 挂起 ⇒ 不产出记录」这一变体,
本计划起草时**未能复现**(卡在本语言的多语句块语法上,非结论性)。
该判定**部分依赖静态守卫**(`test_contract.rs:410-436` 的反向断言确实存在)。

**实现指令**:用 `SCOPE` + `STEP` + `SPAWN` 写一个最小可运行复现脚本放进
`impl/tests/probe/cases/`,把这条从「静态可信」升格为「动态已证」。若发现行为与报告不符,
按缺陷处理并登记 `D12-xxx`。

**验收**:`cargo test --locked -p wlwl-cli --test probe` 全绿,且新用例目录存在。

---

## §5 批次 1:标准库扩展(M1–M5)

### 5.0 「加一个成员」要碰的九处 —— 本节是全文件复用价值最高的部分

> **任何**新增标准库成员(不论 R1 还是 R2)都必须走完这张表。漏掉任何一处,
> 对应的门禁会红,红的位置就是那张表的第几行。

| # | 落点 | 怎么改 | 漏掉的后果 |
|---:|---|---|---|
| 1 | `docs/stdlib/wlwl-stdlib-spec-v0.11.md` 的成员表(§5/§6/§7/§8) | 加一行 | 契约测试「规范表 vs 实现」直接红 |
| 2 | 同文件附录 A(区间 `<!-- appendix-a:begin -->` … `:end`) | **生成**,勿手写 | `stdlib_appendix_a_sync` 红 |
| 3 | R1:`wl/std/*.wll` 的 `EXPORT([...])`;R2:`src/*.rs` 的 `SPEC.functions` | 加名字 | 同上 |
| 4 | `impl/crates/wlwl-eval/tests/{collection,str_math,test}_contract.rs` 的**硬编码成员数与点名清单** | 改数字 + 补名字 | `assert_eq!(impls.len(), N)` 红 |
| 5 | 新模块才需要:`wlwl-std/src/<name>.rs` + `lib.rs` 的 `ALL_SPECS` 登记 | 加文件 + 登记 | `lib.rs:558` 的「`src/` 文件数 == `ALL_SPECS` 长度」红 |
| 6 | `impl/tests/probe/cases/<case>/`(目录 + `main.wll` + `expect.json`) | 加用例目录 | `probe_case_count_matches_inventory` 红(现为 146,同步 `probe.rs` 的 `EXPECTED_CASE_COUNT`) |
| 7 | **仅当加的是全局内建**(不是模块成员) | `docs/appendix_G.md` | `spec_appendix_g_sync` 红 |
| 8 | `wlwl-skill\reference.md` 的成员指针 | 加提及 | 无自动守卫,但 skill 会教错 |
| 9 | `CHANGELOG.md` | 加条目 | 无人拦,但发版流程依赖它 |

> **第 4 行的现存硬编码值(2026-10-02 `已核实`)**:
> `collection_contract.rs:638` → `17`;`:654` → `17`;
> `str_math_contract.rs:720` → `5`;`:734` → `11`;`test_contract.rs:561` → `6`。

### 5.1 M1 · `wlwl:std.encode`(新模块,R2)

**为什么先做它**:完全自包含、无外部依赖、语义无歧义(RFC 逐字定义)、
测试可对照 RFC 向量。**这是本批风险最低、收益最直接的一块。**

出处:Go `encoding/base64`、`encoding/hex`、`net/url`(RFC 4648 / RFC 3986)。

| 成员 | 签名 | 语义 | 失败口径 |
|---|---|---|---|
| `BASE64_ENCODE` | `(s, url_safe?) -> STRING` | RFC 4648;`url_safe=TRUE` 用 `-_` 字母表 | 非字符串 → `E0030` |
| `BASE64_DECODE` | `(s) -> STRING` | 解码;忽略内嵌换行 | 非法字符/长度 → `ERR`(`E0071` 同族) |
| `HEX_ENCODE` | `(s) -> STRING` | 小写十六进制,无分隔 | 非字符串 → `E0030` |
| `HEX_DECODE` | `(s) -> STRING` | 奇数长度或非 hex → `ERR` | 同上 |
| `URL_ENCODE` | `(s) -> STRING` | RFC 3986 percent-encoding | — |
| `URL_DECODE` | `(s) -> STRING` | `%XX` 解码;`+` **不**折成空格(RFC 3986 与 form 编码的差异) | 非法 `%` → `ERR` |

**实现指令**

1. 新建 `impl/crates/wlwl-std/src/encode.rs`,照 `json.rs` 的骨架写 `SPEC`(header 里写清自己的 `path`)。
2. 在 `lib.rs:433` 的 `ALL_SPECS` 登记 `&encode::SPEC`。
3. 挂到 `format.rs` 的分层归属:按 ADR-0021 归 **R2**,无 R1 门面。
4. **零依赖**:base64/hex/url 手写(合计约 120 行)。

**验收**

```powershell
$p = Join-Path $env:TEMP "m1.wll"
@'
IMPORT("wlwl:std.encode", ["BASE64_ENCODE","BASE64_DECODE","HEX_ENCODE","URL_ENCODE"]);
PRINT(BASE64_ENCODE("Hello, 世界"));
PRINT(BASE64_DECODE("SGVsbG8sIOS4lueVjA=="));
PRINT(HEX_ENCODE("Hi"));
PRINT(URL_ENCODE("a b&c=d"));
PRINT(BASE64_DECODE("!!!not base64!!!"));
'@ | Set-Content $p -Encoding UTF8
impl\target\release\wlwl.exe run $p
# 期望: SGVsbG8sIOS4lueVjA== / Hello, 世界 / 4869 / a%20b%26c%3Dd / IS_ERR 路径
```

前四条取自 RFC 4648 §10 与 RFC 3986 §2.5 的官方测试向量 —— **不要自造期望值**。

**守卫**:每个成员至少 2 条契约用例(1 正 1 负),**期望值必须引 RFC 的向量**。
另加一条:大输入往返一致性(`encode(decode(x)) == x`,`x` 取随机字节经 base64)。

**非目标**:不做 MIME 换行、不做 base32、不做 zstd/Python 3.14 `compression.zstd`。

---

### 5.2 M2 · `wlwl:std.text`(新模块,R2)+ `std.str` 补检索

**为什么**:GAP-1/GAP-2 是**已实测**的能力缺口,不是想象的需求。
NFC vs NFD 判不相等(GAP-2)会直接咬到 macOS 文件名(底层是 NFD)与网页内容(多为 NFC)。

| 成员 | 签名 | 语义 | 实现成本 |
|---|---|---|---|
| `TO_UPPER` | `(s) -> STRING` | **完整 Unicode** 简单大小写映射 | **近乎零** |
| `TO_LOWER` | `(s) -> STRING` | 同上 | **近乎零** |
| `NFC` / `NFD` | `(s) -> STRING` | 规范化 | **需要表 —— 见下方裁决** |
| `GRAPHEME_COUNT` | `(s) -> INTEGER` | 字素簇数(用户感知的字符数) | 中 |
| `WIDTH` | `(s) -> INTEGER` | 终端显示宽度(CJK 全角 = 2) | 中 |

> **关键实现事实**:`TO_UPPER` / `TO_LOWER` **不需要任何 Unicode 表**。
> wlwl 是 Rust 实现,`str::to_uppercase()` / `to_lowercase()` 直接就是完整 Unicode
> 大小写映射(含 `ß`→`SS`、希腊 final sigma、土耳其无点 i)。**零依赖、零数据、一次到位。**

**已裁决 · NFC/NFD**:本批**只交付 `TO_UPPER`/`TO_LOWER`/`GRAPHEME_COUNT`**。
NFC/NFD 需要 Unicode 规范化表(约 2000 条 canonical decomposition + composition),
在零依赖约束下要自建并逐版本跟随 —— **代价与本批其余全部工作不相称**。
改为:在 §5.6 登记为后续批次的 spike,并在 GAP-2 处留一条显式的「已知限制」说明。
**不要用一张只覆盖 Latin-1 的半表冒充 NFC。**

**`std.str` 补齐(GAP-3)**

| 成员 | 签名 | 语义 |
|---|---|---|
| `INDEX_OF` | `(s, sub, from?) -> INTEGER` | 首次出现的**码点下标**;无则 `-1`(与全局 `INDEX` 的 1-based / `-1` 口径一致) |
| `COUNT_SUB` | `(s, sub) -> INTEGER` | 非重叠出现次数 |
| `CONTAINS_SUB` | `(s, sub) -> BOOLEAN` | 子串判定(全局 `CONTAINS` 只吃数组) |

**实现指令**

1. `text.rs` 挂 R2;`INDEX_OF`/`COUNT_SUB`/`CONTAINS_SUB` 作为 **R1 门面**加进 `wl/std/str.wll`,
   门面内用 `CODEPOINTS` + 全局 `INDEX` 组合实现(**不新增 kernel**)——
   这样它们不占 R2 槽位,也不用改 `kernels.rs`。
2. 走 §5.0 九处表(`str` 的硬编码 `5` → `8`)。

**验收**

```powershell
$p = Join-Path $env:TEMP "m2.wll"
@'
IMPORT("wlwl:std.text", ["TO_UPPER"]);
IMPORT("wlwl:std.str", ["INDEX_OF"]);
PRINT(TO_UPPER("straße"));                    # STRASSE
PRINT(TO_UPPER("héllo"), TO_UPPER("ÉÀÜ"));      # HÉLLO ÉÀÜ
PRINT(INDEX_OF("hello", "ll"));                # 3
PRINT(INDEX_OF("hello", "zz"));                # -1
'@ | Set-Content $p -Encoding UTF8
impl\target\release\wlwl.exe run $p
```

**守卫**:契约用例必须覆盖 **至少 5 个非 ASCII 脚本**(Latin-1 补充、Latin 扩展 A、
希腊、西里尔、土耳其),外加 `ß` 与 final sigma 两条特例 —— 只测 ASCII 等于没测。

**非目标**:不做 NFC/NFD(已裁决);不做正则;不改全局 `UPPER`/`LOWER` 的现有行为
(**新增而不替换**,避免 breaking)。

---

### 5.3 M3 · `wlwl:std.collection` 扩展(R1)

出处:C++23 `views`(chunk / slide / zip / fold_right)、Go `slices`/`maps`、Rust `Iterator`。

| 成员 | 签名 | 语义 | 出处 |
|---|---|---|---|
| `CHUNK` | `(arr, n) -> ARRAY` | 定长分块,尾块可短 | C++23 `views::chunk` |
| `WINDOW` | `(arr, n) -> ARRAY` | 滑动窗口,步长 1 | C++23 `views::slide` |
| `DEDUP_BY` | `(arr, key) -> ARRAY` | 按 `key` 去重,**保序** | Rust `iter::unique_by` |
| `MIN_BY` / `MAX_BY` | `(arr, key) -> v` | 按投影取极值 | C++23 ranges 投影 |
| `SUM` / `PRODUCT` | `(arr) -> num` | 求和/求积 | `fold_left` 特化 |
| `FOLD_RIGHT` | `(arr, init, f) -> v` | 右折叠 | C++23 `ranges::fold_right` |
| `POSITION` | `(arr, pred?) -> INTEGER` | 首个满足者的下标 | `iter::position` |
| `KEY_BY` | `(arr, key) -> DICT` | 键控归并,后者覆盖前者 | Go `maps.Collect` |

**实现指令**

1. 全部写在 `wl/std/collection.wll`(R1),**不新增 kernel** —— 它们都由既有内建
   (`FOR`/`INDEX`/`LEN`/`SORT_BY`/回调)组合得出。
2. `EXPORT([...])` 顺序**跟随规范 §5 表格的行序**(该约定由 `collection_contract.rs` 锁)。
3. 走 §5.0 九处表(`17` → `25`)。

> ⚠ **D11-012 警告**:`PUSH` 每次复制整个数组 ⇒ `CHUNK` / `WINDOW` / `FLAT` 这类
> 「产出新数组且结果 O(n²)」的成员**在大输入上会退化**。
> **必须**在 `CHUNK`/`WINDOW` 的实现里用「预分配 + `INDEX_SET`」而不是反复 `CONCAT`/`PUSH`,
> 并为两者各加一条 **10 万元素基准用例**(`CHUNK` 与 `WINDOW` 的结果是 O(n) 个元素,
> 平方级实现会直接跑不完 —— 这条用例就是防回摆的)。

**验收**

```powershell
$p = Join-Path $env:TEMP "m3.wll"
@'
IMPORT("wlwl:std.collection", ["CHUNK","WINDOW","DEDUP_BY","MIN_BY","FOLD_RIGHT","KEY_BY"]);
PRINT(CHUNK([1,2,3,4,5], 2));
PRINT(WINDOW([1,2,3,4], 2));
PRINT(DEDUP_BY([1,1,2,2,3], FUN((x), x)));
PRINT(FOLD_RIGHT(["a","b","c"], "", FUN((acc, x), CONCAT(x, acc))));
PRINT(KEY_BY([1,2], FUN((x), x)));
'@ | Set-Content $p -Encoding UTF8
impl\target\release\wlwl.exe run $p
# 期望: [[1,2],[3,4],[5]] / [[1,2],[2,3],[3,4]] / [1,2,3] / "cba" / {1:1, 2:2}
```

**守卫**:每个成员**至少 2 条**契约用例,含**空数组**与**单元素**两个边界;
`WINDOW` 额外加 `n > LEN(arr)` 时返回空数组的断言;10 万规模基准用例见上。

**非目标**:不做惰性视图(依赖未解);不做 `SORT_UNSTABLE`(见 §5.5 裁决)。

---

### 5.4 M4 · `wlwl:std.math` 扩展

出处:Rust `f64` 稳定面、Go `math`、C++23 `fold`/`div_ceil`。

| 成员 | 签名 | 语义 | 层 |
|---|---|---|---|
| `LN` / `LOG2` / `LOG10` | `(x) -> FLOAT` | 对数 | R2 kernel |
| `EXP` | `(x) -> FLOAT` | 自然指数 | R2 kernel |
| `TRUNC` | `(x) -> INTEGER/FLOAT` | 向零取整(区别于 `FLOOR`) | R2 kernel |
| `SIN` `COS` `TAN` | `(x) -> FLOAT` | 三角 | R2 kernel |
| `ASIN` `ACOS` `ATAN` `ATAN2` | `(y) / (y, x)` | 反三角 | R2 kernel |
| `SINH` `COSH` `TANH` | `(x) -> FLOAT` | 双曲 | R2 kernel |
| `SIGN` | `(x) -> INTEGER` | 符号(-1/0/1) | R1 |
| `DIV_CEIL` | `(a, b) -> INTEGER` | 向上取整除 | R1 |
| `GCD` / `LCM` | `(a, b) -> INTEGER` | 最大公约/公倍 | R1 |
| `IS_SQRT` | `(n) -> INTEGER` | 整数平方根 | R1 |
| `POW_MOD` | `(base, exp, mod) -> INTEGER` | 模幂(快幂) | R1 |

**实现指令**

1. 浮点成员走 `kernels.rs`(与 `_SQRT`/`_POW` 同列),注入进 `math.wll` 的门面
   —— 与 D11-001 建立的「R1→R2 私有注入通道」一致,**kernel 不进 `EXPORT`,不外泄**。
2. 整数成员(`DIV_CEIL`/`GCD`/`IS_SQRT`/`POW_MOD`)纯 wlwl 可表达,**放 R1**,不占 kernel。
3. **必须补 `kernels::KERNELS` 的登记 + `every_shared_kernel_is_actually_injected` 的通过**
   (W-02 的新守卫会一并验证调用点存在)。
4. 走 §5.0 九处表(`11` → `24`)。

> **精度契约(易被忽略,务必写进规范)**:浮点成员与现有 `FLOOR`/`CEIL`/`ROUND` 同口径 ——
> 大值经 `INT` 转换会 `E0035`(D11-009 记录的 ±2^53 界)。新成员**沿用同一条界**,
> 不要让 `TRUNC` 成为第一个绕过它的成员。

**验收**

```powershell
$p = Join-Path $env:TEMP "m4.wll"
@'
IMPORT("wlwl:std.math", ["LN","EXP","TRUNC","SIGN","DIV_CEIL","GCD","IS_SQRT","POW_MOD"]);
PRINT(LN(1.0), EXP(0.0), TRUNC(-1.7));
PRINT(SIGN(-5), DIV_CEIL(7,2), GCD(12,18), IS_SQRT(17));
PRINT(POW_MOD(2, 10, 1000));
PRINT(LN(0.0));
'@ | Set-Content $p -Encoding UTF8
impl\target\release\wlwl.exe run $p
```

**守卫**:每个浮点成员 ≥1 条契约用例 + **域错误用例**(`LN(0)` / `LN(-1)` / `SQRT(-1)`
必须是明确的 `ERR`/`E00xx`,不是 `NaN` 静默返回 —— 现有 `SQRT` 的口径要照抄)。

**非目标**:不做三角函数的弧度/角度切换;不做任意精度;不做复数。

---

### 5.5 M5 · 排序与随机数

**已裁决 · 排序**:本批**不加 `SORT_UNSTABLE`/`SORT_SELECT`**。
理由:现有 `SORT` 的稳定性语义是规范承诺(D-6 刚把它写清),而引入不稳定排序要同时处理
比较器一致性检测与「择序策略决定组间次序」的新口径 —— 这是一次**语义变更**,
和本批「不改语言语义」的非目标冲突。收益(大数组常数因子)不足以换一次 breaking。
登记为后续批次。

**已裁决 · 随机数**:本批**不做 `std.rand`**。
理由:wlwl 的卖点包含可复现(确定性并发、规范化的格式化器、契约门禁)。
一个**默认播种**的 RNG 会削弱这个卖点,而一个**要求显式播种**的 RNG 需要先裁决
「全局状态 vs 显式传递」——这是设计决策,不是加几个函数。
登记为后续批次,**但要在 §6 单独记一条,因为它是「有意推迟」而不是「不重要」。**

---

### 5.6 明确推迟到后续批次(spike 清单)

| 项 | 阻塞原因 | 解锁条件 |
|---|---|---|
| NFC / NFD 规范化 | Unicode 表成本 vs 收益(GAP-2 已记录为已知限制) | 评估半表方案的诚实表述,或接受一张受限域表并写明覆盖范围 |
| `std.iter` 惰性迭代 / 视图 | 依赖批次 B `YIELD` 续体保存(0 细化) | 批次 B 的三件事定完 |
| `std.rand` | 全局状态 vs 显式传递未裁决 | 先出 ADR |
| `std.time` | 平台时钟 API + **可复现性冲突**(真实时钟进测试) | 参照 Go 1.24 `testing/synctest` 的「假时钟气泡」思路出 ADR |
| `SORT_UNSTABLE` | 属语义变更 | 单独提案 |
| NFC 之外的 `std.text` 全量 Unicode 表 | 同 NFC | 同 NFC |

> **`std.time` 的参考坐标**:Go 1.24 的 `testing/synctest` 把并发测试放进一个
> 「bubble」,bubble 内 `time` 包走**假时钟**,`synctest.Wait()` 等所有协程阻塞 ——
> 于是「测 10 秒超时」不用真等 10 秒。wlwl 已有 `SCOPE`/`STEP`/`SPAWN` 调度器与 `std.test`,
> **这是本语言最该抄的一个标准库形态**,但它需要虚拟时钟设计,不是加函数。

---

## §6 偏差登记(D12)

> **登记规则**:状态列必须引用证据(命令 / commit SHA / 文件行)。
> 本仓库的历史教训(D11-006)表明:**状态列比描述列更容易失真**,因为描述至少有代码可对照,
> 状态是纯断言。空状态一律写 `未登记`,不要留空。

| 编号 | 来源 | 内容 | 状态 |
|---|---|---|---|
| **D12-001** | N-1 | skill 的 `SUB` 迁移公式产出负长度;报告推荐的 `SLICE` 替代方案对字符串报 `E0030` | 未登记 → 见 W-01 |
| **D12-002** | N-2 | 「防死注入的反向守卫」声明与代码不符 | 未登记 → 见 W-02 |
| **D12-003** | 复核 §1.2 | 「任务内 `RUN_TESTS` 挂起 ⇒ 不产出记录」未能动态复现,现仅静态可信 | 未登记 → 见 W-07 |
| **D12-004** | GAP-2 | `NFC` 与 `NFD` 判不等;本批不做规范化,登记为**已知限制** | 未登记 → 见 M2 |
| **D12-005** | §3.2 | 批次 B(`YIELD` 续体保存)0 细化,阻塞 `std.iter` | 挂起(继承自 D11 批次) |
| **D12-006** | §5.5 | `std.rand` 因「可复现性 vs 全局状态」未裁决而推迟 | 挂起 |
| **D12-007** | 报告失真 | 复核报告 N-4 不实、3 处探针缺前置 | 未登记 → 见 W-05 |

---

## §7 门禁与收口

### 7.1 逐里程碑门禁

每个里程碑结束时跑一遍,**全绿才进下一个**:

```powershell
cd impl
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
cargo deny check
# `cargo doc` **不接受** `-D warnings` —— 它是 rustdoc 的选项,必须走
# RUSTDOCFLAGS。写成 `cargo doc --no-deps -D warnings` 会得到
# `error: unexpected argument '-D' found`,而且**退出码是 1**,
# 看起来像门禁红,其实是命令写错。本条已在 2026-10-02 实测更正。
$env:RUSTDOCFLAGS='-D warnings'
cargo doc --locked --no-deps
cargo check --locked -p wlwl-std --features real-ai
```

### 7.2 新增成员的专项自检(每个里程碑追加)

```powershell
# 1. 规范表 ↔ 实现 一致
cargo test --locked -p wlwl-eval --test collection_contract --test str_math_contract --test test_contract
# 2. 附录 A 同步
cargo test --locked -p wlwl-eval --test stdlib_appendix_a_sync
# 3. 全局内建同步(仅当动了附录 G)
cargo test --locked -p wlwl-eval --test spec_appendix_g_sync
# 4. probe 清单(用例目录在 impl/tests/probe/cases/,驱动在 wlwl-cli crate)
cargo test --locked -p wlwl-cli --test probe
# 5. 新增成员在规范表里能查到、且抽取器抽得到
cargo test --locked -p wlwl-eval the_spec_table_extractor_actually_finds_the_members
```

### 7.3 收口清单

- [ ] `CHANGELOG.md` 加 `v0.11.2` 条目(§5.0 第 9 行)
- [ ] `docs/plan/README.md` 的「当前状态 / 下一迭代 / 挂账项」三表更新
- [ ] 标准库规范**升版到 v0.12**(成员面变了),同步改四份契约测试里指向
      `wlwl-stdlib-spec-v0.11.md` 的路径常量(3 处:`collection_contract.rs:560`、
      `str_math_contract.rs`、`stdlib_appendix_a_sync.rs:18`)
- [ ] `docs/review/` 归档本批的复核报告
- [ ] 版本号 `0.11.1` → `0.11.2`(三处联动,见 D11-016 的台账记载)
- [ ] 发 tag **前**先本地跑一遍 §7.1 + §7.2

---

## §8 检查点

> **恢复工作时先读这张表。** 状态写在文档里,不在 agent 的上下文里 ——
> 上下文会丢,文件不会。

| 里程碑 | 内容 | 状态 | commit |
|---|---|---|---|
| M0 | W-01 ~ W-07(缺陷修复) | 🟡 **代码完成,门禁待确认** | — |
| M1 | `std.encode`(6 成员) | ☐ 未开始 | — |
| M2 | `std.text`(Unicode 大小写)+ `std.str` 检索(3 成员) | ☐ 未开始 | — |
| M3 | `std.collection` 扩展(8 成员) | ☐ 未开始 | — |
| M4 | `std.math` 扩展(13 成员) | ☐ 未开始 | — |
| M5 | 排序/随机数裁决 + 登记 | ☐ 未开始 | — |
| M6 | 收口(§7.3) | ☐ 未开始 | — |

### 8.1 里程碑内的细粒度进度

| 工作项 | 状态 | 实际产出 | 备注 |
|---|---|---|---|
| **W-01** | ✅ 完成 | `reference.md:338-346`、`SKILL.md:631`、`examples/sub_migration.wll`(新)、`README.md` 示例表 | 门禁用 `PANIC` 而非 `RUN_TESTS` —— 实测 `RUN_TESTS` 失败**退出码仍是 0**,拦不住 CI。计划里原写的验收模式是错的,已改 |
| **W-02** | ✅ 完成 | `stdlib_mirror.rs` 新增 2 条测试 | 匹配必须**剥注释**:门面头注释逐条列 kernel 名,`collection.wll` 注释里还记着已删的 `_DIAG_E0038`;朴素 `contains()` 恰好会掩盖要防的回归。**不能要求 `(`**:`_RANGE` / `_EXPECT_ERR` 是裸标识符改名导出 |
| **W-03** | ✅ 完成 | stdlib 规范 §4.1、新建 `tests/format_contract.rs` | 我最初把 `E0102` 消息猜成 `top-level ERR reached the top level uncaught`,实测是 `unhandled ERR escaped to top level: x` —— 又一次印证「期望值不许凭记忆写」 |
| **W-04** | ✅ 完成 | `Evaluator::last_yield_span`、`Warning::span`、`main.rs::report_eval_warnings`、2 条守卫 | `Warning` 结构体原本**没有 span 字段**,`main.rs:519` 硬编码 `0:0`。给 `Signal` 加 span 是大改(几十处构造点),改走 `current_span` 旁路。实测 `E0014` `1:1`→`3:1`,`W0051` `0:0`→`3:1` |
| **W-05** | ✅ 完成 | `wlwl-v0.11.1-recheck.md` 顶部新增更正节 | 正文未改(历史记录不改写) |
| **W-06** | ✅ 完成 | `CONTRIBUTING.md`「Coding conventions」、`SKILL.md` 验证窗口条目 | — |
| **W-07** | ✅ 完成 | 2 个 probe 用例 + `EXPECTED_CASE_COUNT` 144→146 | 任务内实测 `in-task result: NULL` ⇒ 报告 §1.2 的判定**成立**。顶层那条的 span 是 `4:24` 不是 `9:24` —— `YIELD()` 在 TEST 注册行,且**只有 RUN_TESTS 真跑起来才传播** |

---

## 附录 A · 本计划引用的调研出处

标准库扩展的每一组成员都对照了具体语言的现行标准库形态,出处如下
(均为 2026-10-02 检索;**学术/前沿路线优先**是选型时的一贯取舍):

| 来源 | 借鉴点 | 落在本计划哪一节 |
|---|---|---|
| Go 1.23 `iter` + 1.24 `weak`/`synctest` | 迭代器协议、假时钟气泡测试 | §5.5 / §5.6 |
| Go `encoding/{base64,hex}`、`net/url` | RFC 4648 / RFC 3986 编码形态 | §5.1 |
| C++23 `views::{chunk, slide, zip}`、`ranges::fold_right`、投影式 `min`/`max` | 集合成员签名与惰性/急切的分界 | §5.3 |
| C++23 `ranges::to`、`std::expected`、monadic `and_then`/`transform` | 值或错误返回、链式组合 | 现有 `RESULT` 语义(本批不改) |
| Python 3.14 `compression.zstd`、PEP 750 t-strings | 压缩进入标准库的趋势 | §5.1 非目标 |
| Rust `driftsort`/`ipnsort`、`slice::isqrt`、`NonZero::div_ceil` | 排序实现演进、整数数学 | §5.4 / §5.5 |
| Koka 效应系统(`yield` 效应做迭代器) | 迭代作为效应而非数据结构 | §5.6 阻塞说明 |
| Rust `str::to_uppercase` 全 Unicode 映射 | **零表实现 Unicode 大小写** | §5.2 关键事实 |

> **选型说明**:本批刻意**没有**采纳「惰性视图/管道」这一主流形态(C++23 ranges、Go iter)。
> 理由不是不认同,而是它在 wlwl 里有一个**未解的前置依赖**(批次 B 续体保存),
> 现在做就是重演 `std.web`:门禁全绿地推进三周,到第四周才发现前提没成立。
> **这个教训已写进 `docs/history/20261001.md`,本计划继承它。**
