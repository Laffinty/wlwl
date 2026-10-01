# WLWL v0.11.1 复核报告 —— v0.11.0 审查修复验证

| | |
|---|---|
| **复核对象** | 发布包 `wlwl-v0.11.1-x86_64-pc-windows-msvc`(wlwl.exe 报 `0.11.1`)+ 仓库 main@a96ddcf |
| **复核基线** | [v0.11.0 审查报告](wlwl-v0.11.0-review.md)(含其第五部分「修复前复核结论」)· CHANGELOG v0.11.1 · 台账 D11-019/020/021/023/026/037 |
| **复核方式** | 与上轮同标准:① 动态 —— 在 v0.11.1 发布包重建上轮回归套件(d01–d23)并新写 **15 个修复验证探针(f01–f15)**,全部实测;② 静态 —— 逐项抽查修复提交(`8b313fd` 行为修复 / `199280e` 卫生 / `1270c45` 台账与规范对齐 / `48ec525` skill)与现行文档。测试文件全部置于发布包 `review-tests/`,工作目录零过程文件 |
| **日期** | 2026-10-02 |

---

## 0 总体结论

**v0.11.0 审查的全部发现(4×P1、~12×P2、BOM 缺陷族)已妥善修复,无一回归。** 上轮 25 个动态用例重建后在 v0.11.1 上行为全部符合预期(修复点按新语义变化,其余逐字节一致);随包 11 个示例、skill 18 个示例全部通过。处置纪律值得肯定:每条发现都登记了偏差(D11-019/020/021)、D11-007 的错误前提在台账中正式更正、连上轮报告本身的两处误判(D-2 归因、S-P1-2 严重度)也被复核更正并留档。

本轮另发现 **1 个新问题(N-1,P1,skill 的 SUB 迁移公式修了个寂寞)** 与 4 个 P3 级小项。

| 上轮编号 | 内容 | 复核判定 |
|---|---|---|
| D-1 (P1) | `NEG/ABS(INT_MIN)` 静默回绕 | **已修,实测证实** |
| S-P1-1 (P1) | `real-ai` 特性编译必炸 | **已修**(代码 + CI 门禁;静态证实) |
| S-P1-2 (P1) | `call_callable` 吞 Yield → 假通过 | **已修,实测证实** |
| S-P1-3 (P1) | compat 边界三处语义漂移 | **已修,实测证实**(② 的运行时验证见 f14b) |
| D-2 (P2) | `SQRT(-0.0)` 丢符号 | **归因更正**:SQRT 不丢符号,是 `-0.0` 字面量在解析期不可达;规范 §7 已写明,真负零实测保号 |
| D-3 (P2) | `SORT(arr, cmp)` 未入成员表 | **已修**(§5 表已补,含 E0030 与 ERR 传播口径) |
| D-4 (P1) | SORT 比较器 ERR 零诊断原序返回 | **已修,实测证实**,载荷原样保真 |
| D-5 (P3) | W0051 文案「removed in v0.5」 | **已修** |
| D-6 (P2) | 「保留原序」口径不成立 | **已修**(§5 改为「各组内稳定,组间次序由择序策略决定」) |
| S-P2-2~10 | 死注入/台账失真/规范缺文/扫描器脆弱/env 门控/软失败等 | **全部处置**(逐项见 §2) |
| BOM 族 | `.wll` / `.sig` / `wlwl.toml` / `wlwl.lock` | **三处实测证实,一处以代码+单测为据**(见 §2.4) |
| skill 6/10 | v0.11 零收录、两条被证伪结论、示例失真 | **主体已对齐**,但 SUB 处方修复无效(N-1),退出码 2/3 仍未收录(N-4) |

---

## 第一部分 四个 P1 的修复验证(全部实测)

### 1.1 D-1:`NEG` / `ABS(INT_MIN)` → `E0034` ✓

```wlwl
PRINT(NEG(im));    // error[E0034]: NEG: cannot negate INTEGER_MIN (-9223372036854775808);
                   //              use -INTEGER_MIN+1 or special-case      rc=1  ✓
PRINT(ABS(im));    // 同一条 E0034,消息逐字一致                      rc=1  ✓
PRINT(-(0, im));   // 对照路径:E0034(与 v0.11.0 相同)               ✓
```
静态侧:`builtin_neg` 改走 `checked_neg`,与 `builtin_sub` 共用判定与消息;`str_math_contract.rs` 新增 4 条契约用例(+28 行)。上轮的静默回绕(返回 `INT_MIN`、rc=0)不再可达。

### 1.2 S-P1-2:测试体内 `YIELD()` 不再假通过 ✓

上轮的灾难形态(`YIELD(); ASSERT(FALSE)` 被记 `passed = TRUE`,断言一次不跑)已消除:
- **顶层调用 `RUN_TESTS`**(无调度器可交信号):`YIELD()` 执行点报 `E0014`(YIELD outside a step context),整个运行终止 —— 假通过不可能发生 ✓;
- **任务内调用**(规范 §8 新口径的场景):`SCOPE` 内 `SPAWN(RUN_TESTS())`,挂起信号原样穿透,`AWAIT` 得 `NULL`,**整个 `RUN_TESTS` 不产出任何记录**(含正常测试也不记)—— 与 stdlib §8「不产出记录并把挂起信号原样交回调度器」逐字吻合 ✓。
- 静态侧:`call_callable` 现在直通 `Outcome`(`lib.rs:272-279`);`RUN_TESTS` 显式处理挂起(D11-019),新增「挂起的测试体不得被记为通过」守卫 + 反向守卫。

### 1.3 D-4:SORT 比较器 ERR 传播 ✓(了结 D11-004 挂账)

```wlwl
LET(cmp, FUN((a, b), ERR("cmp-boom")));
LET(r, SORT([2, 1], cmp));
PRINT(IS_ERR(r), TYPE(r));    // TRUE RESULT      ✓(v0.11.0:FALSE ARRAY)
PRINT(ERR_PAYLOAD(r));        // cmp-boom         ✓ 载荷原样,无包装改写
PRINT(IS_ERR(SORT_BY(...)));  // TRUE             ✓ SORT_BY 不回归
```
对照路径不变:比较器返非布尔(`5`)仍 `E0030: SORT: comparator must return BOOLEAN, got integer` ✓。静态侧: `_SORT_LT` 先 `IS_ERR` 拦截原样交回 + `SORT` 加 err 累加器终止循环;首轮不调比较器的短路在同一处显式重写,并新增「比较器收到 NULL 前驱就抛 ERR」守卫钉住该短路。8 个回调成员的传播(d04)全部复测通过。

### 1.4 S-P1-1:`real-ai` 特性 ✓(静态证实)

修复提交实测该特性有 **3 个编译错误**(上轮记 2 个,漏了 `http_chat` 的 E0502 借用冲突),修法为进程内 `OnceLock` 缓存 client(不给 `wlwl-value` 引入 reqwest,符合 ADR-0022 的零依赖画像);`ci.yml:128-139` 新增 `cargo check --locked -p wlwl-std --features real-ai` 门禁。本复核未在本地构建(遵守零过程文件约束),以修复提交描述 + CI 门禁代码为据。

### 1.5 S-P1-3:compat 边界三处漂移全部改回 ✓

| 探针 | v0.11.1 实测 | 判定 |
|---|---|---|
| `STRINGIFY(OK(1))` | `1` | ✓ OK 解包恢复(M2 期是 `{"Ok":1}`) |
| `std.format` 成员 `FORMAT("{0}", OK(1))` | `1` | ✓ 恢复 |
| `STRINGIFY([2: "a"])`(整数键字面量) | `error[E0030]: std argument: expected string dict key, got integer` | ✓ 拒绝恢复 |
| `STRINGIFY(ERR("x"))` | 调用边界即短路,到不了转换层 | ✓(复核节认定「不可观察」,保留拒绝臂作兜底,认可) |

注:json.rs 的 4 条边界单测(含 `boundary_rejects_integer_dict_keys`)钉住 `values_to_json`;`wrap` 确实经由该函数(`lib.rs:295`)。

---

## 第二部分 P2 / BOM 族 / 发布物 复核

### 2.1 文档与台账(静态逐项)

- **S-P2-3** RANGE 溢出回摆:stdlib §5 落地状态补「唯一例外……约 9.2×10^18 次迭代,实际不可达」✓;
- **S-P2-4** D11-001 承诺的规范文本:stdlib §0.2 新增「R1→R2 私有注入通道(kernel)」段,三条不外泄约束 + 改名导出形态 + 两条守卫名 ✓;ADR-0021 同步 ✓;
- **S-P2-5** ADR-0021 归属总表 `std.collection` → 混合(注明 M5 裁决与基准)✓;
- **S-P2-6** 陈旧注释:`wrap` 的 `_name` 死参删除(附说明)、`_DIAG_E0038` 相关注释全部改写 ✓(抽查);
- **S-P2-7** `lang_exports` 在 `;`/`//` 处先截断 + 新增交叉校验测试 **`the_line_scanner_agrees_with_the_ast_export_walker`**(行扫描器 vs AST 权威口径,恰是上轮建议的「同源自洽」缺口补丁)✓;上轮举例被复核更正(仅注释中有引号不触发,需同时含 `[]` 与 `"`)—— 复核节与实现一致 ✓;
- **S-P2-8** `WLWL_STD_SRC` 加 `debug_assertions` 门控,release 下**实测被忽略**(f06a:env 指向漂移副本,程序用嵌入版正常输出);`--std-src` flag 仍生效(漂移副本 → `E0020` 拒载,同导出面副本 → 正常切换)✓;
- **S-P2-9** release.yml 三处 `|| true` 移除,`docs/stdlib` 复制为硬失败 ✓;
- 台账:D11-019/020/021 登记 ✓;D11-001/003/004/007 四处失真更正,D11-007 附「前提后来被证伪」补记 ✓;台账整体归档 `docs/history/20261001.md` ✓。

### 2.2 死注入(S-P2-2)

`_DIAG_E0038` 从注入表与 `KERNELS` 表双双移除(kernels.rs:128-134 注释交代三处同删);`E0038` 唯一发射点回到 Rust 侧 `kernel_range`,实测 `RANGE(1,10,0)` 仍 `E0038` 带名消息 ✓。

> **N-2(P3)|「补反向守卫防注入了没人用」的声明与代码不符。** CHANGELOG 称「(移除死注入)并补反向守卫,防止再出现『注入了没人用』」,但现有守卫面只有:`kernel_table_is_well_formed`(命名)、`every_shared_kernel_is_actually_injected`(KERNELS ⊆ 注入,D11-011)、非空反空守卫。**没有任何测试检查「注入了但 .wll 源码无调用点」** —— 将来有人把死内核同时加回 KERNELS 与注入表,全部门禁照绿。本次删除本身正确彻底,但该防线声明不成立。建议:补一条「注入表中每个 kernel 名必须出现在对应模块的 `include_str!` 源码文本里」的守卫(与 `the_line_scanner_agrees_with_the_ast_export_walker` 同风格)。

### 2.3 D-2 / D-3 / D-5 / D-6

- **D-2**:`SQRT(真负零)`(经 `*(0.0, -(0, 1.0))` 构造)实测返回 `-0.0`,符号保真 ✓;stdlib §7 该格改写为「源码写不出负零字面量……此格按字面读不可达」并给绕法,归因更正准确 ✓;
- **D-3**:§5 成员表补 `SORT(arr, cmp)`,含 `cmp` 返非布尔的 `E0030` 与抛 `ERR` 的传播口径 ✓;
- **D-5**:W0051 文案改为「deprecated alias — removal is an open evolution item, no version is scheduled」✓(span 仍 `0:0`,见 N-5);
- **D-6**:§5 改为「不可比时不报错:各组内稳定,组间次序由择序策略决定」,并加注「不保证与原序一致」—— 与实测行为(`[3,"a",1,"b",2]` → `[1,2,3,a,b]`)一致 ✓。

### 2.4 BOM 缺陷族(D11-023 / D11-026)—— 动态三处证实

| 落点 | 探针 | 结果 |
|---|---|---|
| `.wll` | BOM + `PRINT("bom-ok")` | 正常输出,rc=0 ✓(v0.11.0:E0020) |
| `wlwl.toml` | BOM + 完整清单(`[package]`+`[features] gradual_typing="error"`) | **无 W0001**,清单正常加载 ✓ |
| `.wll.sig` | BOM + 故意错误的返回类型声明 | 签名被解析并触发 **`E0115` 硬错** —— 同时证明 sig 的 BOM 被剥、且 `gradual_typing=error` 从 BOM 清单正确生效(无开关时此处应为静默)✓ |
| `wlwl.lock` | 未动态探(需依赖解析场景) | 代码在 `wlwl-toml/src/lib.rs:45`(`strip_prefix('\u{FEFF}')`,恰好剥一个)+ 单测,以代码为据 |

复核要点:「恰好剥一个」语义有文档(`lib.rs:28-39`),`trim_start_matches` 的坑被显式避开 ✓。当初探针里看到的 `W0001` 实为清单缺 `[package]` 所致(serde 缺字段报「line 1 column 1」),与 BOM 无关 —— 记录于此以免误导。

### 2.5 std.web 终止(D11-037)

二进制确认无 `std.web`:`IMPORT("wlwl:std.web", ["PARSE_TEMPLATE"])` → `E0040: module not found` ✓。失败记录、回滚 SHA、复活前置条件留档 `docs/history/20261001.md`,台账 D11-037 完整(含「门禁测不出计划前提失效」的教训)✓。处置干净,本版确不含任何 std.web 代码。

---

## 第三部分 wlwl-skill 复核

### 3.1 上轮问题的处置(逐条)

| 上轮问题 | 判定 |
|---|---|
| `gradual_typing` 管辖范围被证伪 | **已修**:SKILL.md:248-254 改为「governs `E0110`–`E0115`」✓ |
| `wlwl check` 做名字解析被证伪 | **已修**:reference.md:684「does NOT do name resolution… exits 0」并补「walks the import graph」的准确描述 ✓ |
| 幻码 `W0015`/`W0040` | **已修**:改为「Two codes that do not exist」专段 ✓ |
| v0.11 增量零收录 | **已修**:reference.md §25(v0.11 增量)、SKILL.md:594(std.test + ASSERT breaking 指针)、std.str/std.math 共 9 处提及 ✓ |
| ASSERT breaking | **已修**:reference.md §25 收录八假值新口径 ✓ |
| SUB 迁移公式 | **改了,但仍错 —— 见 N-1** |
| INDEX 1-based | **已修**:SKILL.md:69 + reference.md:311-314,含「single easiest trap」警示 ✓ |
| 六处示例注释失真 | **已修**(复核节认定实为 4 处):`static_contracts` 改名 `pick_smaller`(实测 `pick_smaller(3,10)` → 3)、`truthiness` 输出 `FALSE: falsy` ✓、`match.wll` 实测出现 `ERR arm: is_err=TRUE payload=boom` ✓ |
| CHANGELOG/README 版本 | **已修**:skill CHANGELOG 有 0.11 对齐条目(自陈「此前全部用户可见增量零收录」),README 统一 v0.11 ✓ |
| `wlwl.lock` 零解释 | **已修**:README:104 ✓ |
| 附录 G 106 / §8.3 13 | **已修**:reference.md:209「106 entries」、SKILL.md:567「13 global entries」✓ |
| 验证版本标注纪律(建议项) | **已采纳**:SKILL.md:686「Validity window for measured claims」—— 超出建议范围的好改动 ✓ |

18 个示例在 v0.11.1 上全部 rc=0。

### 3.2 新发现

**N-1(P1,skill)|SUB 迁移公式「修复」无效,与旧错误代数等价。**
reference.md:338 与 SKILL.md:631(反模式 #18)现写:

> `SUB(s, start, end_old)` → `SUB(s, start, NEG(-(end_old, start)))`

实测(f13b):

```wlwl
PRINT(SUB("Hello", 1, 3));             // ell     ← 第三参 = 长度
PRINT(SUB("Hello", 1, -(4, 1)));       // ell     ← 正确迁移式(无需取负)
PRINT(SUB("Hello", 1, NEG(-(4, 1))));  // ""(空) ← skill 现行公式
PRINT(NEG(-(4, 1)));                   // -3
```

`NEG(-(end_old, start))` = `start − end_old`,与旧错误公式 `-(start, end_old)` **代数恒等** —— 都是负长度,`SUB` 返回 `""`。更糟的是公式与它自己的散文矛盾:同一句里先说「length is `end_old − start`, so `-(end_old, start)`」(这一步是对的),紧接着又说「so negate with `NEG`」把刚算对的长度再取负;而下一句「the latter yields a negative length and SUB returns `""`」描述的恰是自己公式的输出。修复提交 `48ec525` 声称「修正处方级错误」,实际只是把同一个错值换了个写法。**修法:删去 `NEG(...)` 包裹,迁移式应为 `SUB(s, start, -(end_old, start))`**(或按其自荐 `SLICE(s, start, end_old)`)。

**N-4(P3)|退出码 2 / 3 仍未收录**(上轮完整性清单第 7 条的未处理项;`reference.md` 退出码表仍只有 0/1/101)。parse 失败退出码 3 是排错的第一线索,建议补。

### 3.3 结论

skill 从 6/10 升到 **8/10**。两条被证伪的「实测结论」已更正并建立了防复发机制(验证版本标注);v0.11 增量、breaking、成员指针全部补齐;示例注释失真清零。剩余扣分:N-1 的处方仍会产出错误代码(这是 skill 里危害等级最高的一类错误)、退出码表不全。

---

## 第四部分 本轮新发现汇总与回归

| 编号 | 级别 | 内容 | 建议去向 |
|---|---|---|---|
| N-1 | **P1(skill)** | SUB 迁移公式 `NEG(-(end_old, start))` 与旧错误代数等价,产出负长度 → `""`;散文自相矛盾 | 删 `NEG` 包裹,reference.md:338 + SKILL.md:631 两处;补一条「迁移式产出预期子串」的示例验证 |
| N-2 | P3 | 「防死注入的反向守卫」声明与代码不符,无「注入但无调用点」检查 | 补源码文本包含性守卫,或更正 CHANGELOG 措辞 |
| N-3 | P3 | 全局 `FORMAT(OK(1))` 渲染 `OK(1)`,`std.format` 成员渲染 `1` —— §4「同一函数」在 RESULT 值上不成立(M2 前即如此,非回归) | stdlib §4 补一句口径(成员路径经边界,OK 解包;全局走 display) |
| N-4 | P3 | skill 退出码 2/3 未收录 | 补 reference.md |
| N-5 | P3(cosmetic) | `E0014`(YIELD outside step)的诊断 span 指向 IMPORT 行;W0051 span 仍 `0:0` | 随下次 span 治理一并处理 |

**回归验证**:上轮 d01–d23 重建后在 v0.11.1 上全部符合预期 —— str/math/collection/test 功能面逐字节一致;kernel 不外泄(E0023)、不重导出(E0023)、`E0038`、真值八假值、NaN 可达性、混合排序行为、SUB 越界钳制、`TYPE(ERR)` 消费者语义全部不变;100 万次循环 0.575 s 无回退;随包 11 示例 + skill 18 示例全绿。**未发现修复引入的任何行为回归。**

---

## 附:复核产物清单

- 动态:发布包 `review-tests/` 下 d01–d23(回归重建)+ f01–f15(修复探针,含 bom_project 三个最小工程),全部在 v0.11.1 二进制上实跑;
- 静态:抽查 `8b313fd` / `199280e` / `1270c45` / `48ec525` 四个提交与现行 `docs/stdlib`、ADR-0021、`ci.yml`、`release.yml`、`wlwl-toml`、`kernels.rs`、`stdlib_mirror.rs`、`test_native.rs`、`json.rs`、skill 全部文本;
- 工作目录零过程文件;报告本身是本目录唯一新增。
