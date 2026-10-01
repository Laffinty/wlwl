# WLWL v0.11.0 多维度审查报告

| | |
|---|---|
| **审查对象** | 发布包 `wlwl-v0.11.0-x86_64-pc-windows-msvc`(wlwl.exe 报 `0.11.0`)+ 仓库 `D:\Project\wlwl`(commit `84d3f89`,main) |
| **审查标尺** | 语言规范 `docs/spec/wlwl-spec-v0.11.md` · 标准库规范 `docs/stdlib/wlwl-stdlib-spec-v0.11.md` · 构建计划 `docs/plan/plan-v0.11.md` · 偏差台账 `deviations-v0.11.md`(D11-001~018)· ADR-0021/0022/0023 · CHANGELOG v0.11.0 条目 |
| **审查方式** | ① 静态:四路并行源码审查(wlwl-std + R1 源码 / wlwl-value 值直通 / 双轨机制与 CLI / wlwl-skill 文本);② 动态:基于发布包 `wlwl.exe` 撰写并执行 **25 个 `.wll` 测试程序**(含错误路径、边界值、双轨覆盖、性能),全部测试文件置于发布包 `review-tests/` 目录;③ 依据动态使用体验评价 wlwl-skill |
| **日期** | 2026-10-01 |

---

## 0 总体结论

**v0.11.0 达到其设计目标。** 批次 A(M1 双轨机制、M2 值直通、M3 R1 首批、M5 基线)的每一项可观察承诺都在发布包上得到实测证实:成员面与规范逐一对齐、kernel 不外泄、覆盖轨漂移必须 `E0023` 且绝不静默回退、`ASSERT` 八假值 breaking 口径落地、`RANGE` 沉 R2 后 100 万次循环 0.575 s(预算 30 s)。静态审查未发现 P0。

但本轮审查发现 **1 处发布包实测的新功能性缺陷(P1-D1,`ABS(INT_MIN)` 静默回绕)**、**3 处 M2 引入且台账未登记的契约问题(P1)**,以及一批 P2 级文档/台账/死代码欠账。wlwl-skill 停在 v0.10.1,未收录 v0.11 全部用户可见增量,且有 2 条会被实测证伪的「实测结论」——**作为 v0.11 时代的 AI 技能包暂不推荐直接使用,需先做一轮对齐**。

| 维度 | 判定 |
|---|---|
| 设计目标实现度(批次 A) | **达成**(M1/M2/M3/M5 全部实测证实;M4 成文完成,但有两处文本未按台账承诺落地,见 S-P2-4/5) |
| 新发现功能缺陷 | **P1 × 4**(1 条动态实测、3 条静态);P2 × ~20 |
| 已知挂账项复核 | D11-004 SORT 比较器 ERR 不传播:**仍在**(机制已查清,见 D-4) |
| wlwl-skill | 6/10 —— 语言核心部分质量高,v0.11 增量零收录、2 条结论被 v0.11 实测证伪 |

---

## 第一部分 静态代码审查

### 1.1 设计目标达成项(四路审查交叉证实)

**M1 双轨实现机制**
- 单一清单双轨成立:`wlwl-std/src/lib.rs` 的 `StdSource/StdBackend/LANG_SOURCES/resolve` 与 ADR-0021 归属总表逐行对应;Lang 后端 `functions()` 返回空切片,名册统一走 `lang_exports`,口径单一。
- kernel 注入时序正确(`new_with_loader` 之后、`eval_module` 之前),模块 env 只由 `collect_exports` 的 EXPORT 名单构建;守卫三层齐备(不进 EXPORT / 反向防死代码 / 反空),无「声明了但没人注入」的 kernel。
- 覆盖轨优先级 flag > env(`main.rs:375-378`);覆盖目录缺文件 → `E0040` 全量替换、无逐模块回退;`with_base_dir` 与 `with_std_src` 装配次序无关(plan 声称属实,实测两条装配序都通)。
- 附录 A 生成器幂等 + 双向对账锁;`release.yml` staging 真的把 `stdlib/` 与 `docs/stdlib/` 打进产物(Desktop 包两目录俱在,且 `stdlib/*.wll` 与仓库 `wl/std/*.wll` **内容逐字节一致**,仅行尾形态不同);probe 计数 142 与目录实数一致。
- CLI `--std-src` 帮助文本如实标注「非稳定接口」。

**M2 值直通**
- 依赖方向严格单向(`wlwl-value` 仅依赖 ast+error;eval → value ← std),workspace 依赖已带 `version`(D11-016 的坑已避开)。
- serde_json 边界转换层(`value_to_std_value`/`std_value_to_value`)确已删除,`invoke_std` 直通收发 `Value`;serde_json 残留均为 ADR-0022 §4 允许的模块内部表示。
- 「纯搬移不改语义」经与拆分前 commit(`30b70a7^`)逐段 diff 属实:`Value`/`display`/`PartialEq`/`values_equal`/`Env`/`Signal`/`Outcome`/句柄/协议等搬移体逐字一致。
- 回调注入闭环:`impl StdHost for Evaluator` 对闭包走真实 span + E0020 诊断;句柄校验含 generation 匹配,陈旧句柄一律 `E0053`。

**M3 R1 首批 + 混合 test**
- 四个 `.wll` 门面的 EXPORT 名单与规范 §5(17)/§6(5)/§7(11)/§8(6) 及附录 A **逐一对应,无多余无缺失**;str/math/test 的 EXPORT 顺序逐字跟随表格行序。
- str/math/test 契约测试从**规范 markdown 解析成员面**与实现对拍(§0.3 第 3 条外部对照落地),并各配反向守卫;D11-006 拆除的恒真式已换成真锁。
- `test.wll:83` 确用 `BOOL(cond)` —— D11-010 breaking 口径落地,契约表含 6 条探测器用例。
- str/math 各成员边界与 §6/§7 一致且有冻结用例钉住(COUNT 空串、CHAR_AT 负索引、QUOTE 转义、_LIFT2/_LIFT3 提升、±2^53 界、NaN 判据)。

### 1.2 静态发现的问题

> 编号 S-* 为静态发现;与动态复核的交叉验证状态随条目标注。P1-D* 见第二部分。

**S-P1-1|`real-ai` 特性编译必炸:`StdCtx.http_client` 字段在 M2 搬移中被静默删除**
- `wlwl-std/src/ai.rs:136-152` 读写 `ctx.http_client`,而 M2 后 `crate::StdCtx` 就是 `wlwl-value` 那份(仅 argv/env/warnings/source_file/tests 五字段);旧 `StdCtx` 的 `#[cfg(feature = "real-ai")] http_client` 字段被丢。默认构建不编译该 cfg 块,1824 项测试全绿也照不出来;CI 不跑 `--features wlwl-std/real-ai`。`Cargo.toml` 的 feature 与注释仍宣称该路径可用,偏差台账无此删除记录。
- 处置建议:补字段或删 feature,并记偏差。

**S-P1-2|`call_callable` 吞掉 `Signal::Yield`,违反 wlwl-value 白纸黑字的穿透契约**
- `wlwl-std/src/lib.rs:233-242` `let outcome = host.call(...)?; Ok(outcome.value)` —— signal 被丢弃;而 `wlwl-value/src/lib.rs:1005-1007` 明文「挂起必须原样穿透,不得消费」。
- 可达路径:`RUN_TESTS` 的测试体内 `YIELD()`,或任务内 `CHANNEL_RECV` 落空(该臂会先真实停泊任务再发信号)——任务已停泊但段运行器永远收不到 Yield,测试被记成 `passed = TRUE`。
- 定级说明:旧 R2 `collection::call_callable` 同样吞,非 M2 回归;但 M2 把穿透写成正式契约后,唯一在位的原生回调消费者恰是违约方。应修或记偏差。
- (注:动态测试未覆盖此路径——本条为静态认定,修复前建议先写一条「测试体内 YIELD」的探针用例钉住。)

**S-P1-3|compat 边界转换语义漂移(三处可观察差异),与「函数体与测试零改动」承诺不符且未记偏差**
- `wlwl-std/src/json.rs:76-119`(`value_to_json`,M2 新建)对照旧边界 `value_to_std_value`:① `OK(v)` 旧**解包**传内层,新包成 `{"Ok": v}`;② `ERR(v)` 旧一律 `E0030` 拒绝,新接受为 `{"Err": v}`;③ 字典整数键旧拒绝,新字符串化放行。波及经 `wrap` 的 io/fs/json/format/ai/agent 六模块;probe 无用例钉住。有意放宽应补偏差;无意则是「纯搬移」纪律的实质破口。

**S-P2-1|D11-004 台账描述与当前代码脱节;缺陷仍在且无用例钉住**(动态复核 ✓,机制已修正,见 D-4)

**S-P2-2|`_DIAG_E0038` 成死注入**:`lib.rs:123` 仍向 collection 注入,但门面已无任何调用(RANGE 沉 R2 后 E0038 唯一发射点回到 Rust 侧 `collection.rs:98-101`);现有守卫只查「被注入」,查不出「注入了没人用」。四处文档随之过时(`collection.wll:12`「E0038 全仓唯一发射点就在这里」等)。

**S-P2-3|`collection.rs` 引用不存在的「§9.5 饱和约定」,与 D11-003/D11-012 台账矛盾未登记**:注释称溢出「按 §9.5 饱和」,语言规范 §9.5 实为「程序入口」,§11 明写溢出抛 `E0035` 无饱和路径;D11-012 宣称 RANGE 沉 R2「语义/诊断一字未变」,溢出行为实际从 E0035 回摆到饱和(仅 ~9.2×10^18 次迭代可达,实际不可达,故 P2)。

**S-P2-4|D11-001 处置承诺的规范文本未落地**:台账称「ADR-0021 §层间规则补机制说明 + stdlib §0.2 增『kernel 不外泄』一句」且标「已处置」,但两处文档均无任何 kernel 注入/不外泄文字——私有注入通道目前只活在 rustdoc 与台账里。

**S-P2-5|ADR-0021 归属总表与 M5 裁决矛盾**:`0021-stdlib-layering.md:82` 仍写 `std.collection | R1`(终态),与 M5 后的「混合(R1 门面 + R2 RANGE)」(stdlib §0.2/附录 A/语言规范 §10.6)不一致;D11-014 更正语言规范时漏了这处。

**S-P2-6|eval 侧四处已废止边界的陈旧注释**(`invoke_std` 文档仍描述 serde_json 转换层、`NativeInvoke::Builtin` 已删仍被引用等);其中一处原属已删除的 collection 模块的文档块**误挂在 `pub mod registry;`** 上,误导性最强。

**S-P2-7|`lang_exports` 行解析器对行尾注释中的引号脆弱**(`lib.rs:177-194`):`EXPORT([...]); // don't touch "C"` 会解析出幻影导出 "C"。当前四个 `.wll` 不触发,但它是附录 A 与全部守门测试的对账口径;且 `stdlib_appendix_a_sync` 两端同源(都走 `lang_exports`),扫描器自身偏差会自洽全绿——真正的兜底只有 contract 测试的 AST `collect_exports` 路径。建议在 `;`/`//` 处先截断,并补一条 `lang_exports(源码) == collect_exports(parse(源码))` 直测。

**S-P2-8|`WLWL_STD_SRC` 无 debug 门控**:ADR-0021/stdlib §0.5/main.rs 注释三处都写「debug 环境变量」,实现无条件生效——**发布版二进制可被环境变量重定向加载任意 R1 源码目录**(动态测试正是用发布包实测的覆盖轨)。与 §0.5「release 默认嵌入、运行时不读取外部文件」靠「默认」二字勉强自洽,且构成供应链面;release 行为零测试覆盖。若有意 release 生效,三处措辞需改。

**S-P2-9|release.yml 对 docs/stdlib 的复制是软失败**(`|| true`):规范手册缺席时打包静默成功;M1-4「发布产物含两目录」只剩一半是硬保证。

**S-P2-10|卫生类(一次列出)**:`wlwl-value/src/lib.rs:734` `Tag` 枚举文档只剩残句「/// the payload.」(D11-015 同批漏网);`StdCtx::warnings` 是无人排空的死汇(文档称「eval 侧在调用后排空」,全仓无排空点,W0052 发出后永不浮现);`wrap` 的 `_name` 死参;`lib.rs:12`/`io.rs:1` 目录注释漏 `PRINT_ERR`;`collection.wll:37` `_NEED_INT` 死代码、`:343-344` 注释引用已删除的 R2 NAMES 常量与已拆除的恒真锁;stdlib 规范附录 A 前言「collection/test 名录取 eval 侧 BUILTINS」过时;kernels.rs 注释称 KERNELS 是「StdSource 直接引用本表」实际是手抄清单;`stdlib_dual_track.rs:201` 注释混入字面 `` `n ``。

---

## 第二部分 动态运行测试

### 2.1 用例与结果总览

在发布包目录 `review-tests/` 下撰写 25 个测试程序,另实跑随包 examples 11 个、wlwl-skill examples 18 个。核心结果:

| 组 | 用例 | 结果 |
|---|---|---|
| 基线 | `-V` 报 0.11.0;11 个随包 examples 全跑通;`check/ast/fmt/sig/interface/schema` 子命令健全;`fmt` 幂等;`check` 不做名字解析(§11.4 ✓);parse 错误退出码 3(§11.4 ✓) | **全过** |
| §6 std.str | 五成员功能面 + 边界(`\r\n`、尾随换行、空串、负索引 `CHAR_AT("abc",-1)`→`"c"`、越界同 SUB 口径、COUNT 非重叠、`COUNT(s,"")`→`E0030` 带名消息、QUOTE 五种转义) | **全过** |
| §7 std.math | 11 成员 + 提升口径(`MIN(1,2.5)`→`1.0` FLOAT,CLAMP 看三参)+ ROUND 半数远离零(±2.5→±3.0)+ `FLOOR(1e20)`/`ROUND(1e20)` 原样直通 + `SQRT/POW` DomainError 是 ERR 值 + `PI/E` | **全过**(唯 `ABS(INT_MIN)`,见 D-1) |
| §5 std.collection | 17 成员功能面;RANGE 三种元数 + 负步长;`RANGE(1,10,0)`→`E0038`(注入 kernel,带名消息);混合类型排序;8 个回调成员的 ERR 传播 | **全过**(唯 SORT 比较器 ERR,见 D-4) |
| §8 std.test | `RUN_TESTS` 记录(name/passed/duration_ms/return_value|error);体逃逸 ERR→passed=FALSE;`E0046/E0047/E0048` | **全过** |
| D11-010 breaking | 八假值逐一探测:`ASSERT(0/0.0/""/[]/NULL/FALSE/空字典/NaN)` 全部失败,真值全部通过;NaN 经 `inf-inf` 可达(`POW(1e308,2.0)`→inf) | **全过** |
| kernel 治理 | `IMPORT("wlwl:std.math",["_SQRT"])`→`E0023`;`IMPORT("wlwl:std.str",["SUB"])`(重导出全局内建)→`E0023` | **全过** |
| 双轨 | `WLWL_STD_SRC` 指向随包 stdlib/:行为与嵌入轨一致;纯导出面漂移(EXPORT 删名)→ `E0023`,**绝不静默回退**;绑定改名漂移→ `E0020`(EXPORT 引用未绑定名),同样不回退 | **全过** |
| 性能 | `RANGE(1,1000001,1)` + 100 万次 FOR:**0.575 s**(§6.6 预算 30 s);MAP 缩放 2k/4k/8k = 338/845/2786 ms —— 超线性确认(D11-012 已知局限实测成立) | **符合已知结论** |
| 发布物一致性 | 随包 `stdlib/*.wll` 与仓库 `wl/std/*.wll` 内容逐字节一致(仅 EOL);随包 `docs/stdlib/` 与仓库规范一致 | **全过** |

### 2.2 动态新发现(偏差台账未登记)

**D-1(P1)|`ABS(INT_MIN)` 静默回绕,违反 stdlib §7 与语言规范 §2.2 —— 且 D11-007 的更正前提是错的**
- 实测(release 包):
  ```wlwl
  LET(m, -(0, 9223372036854775807));
  LET(im, -(m, 1));            // INT_MIN
  PRINT(ABS(im));              // → -9223372036854775808   rc=0   ← 应为 E0034
  PRINT(NEG(im));              // → -9223372036854775808   rc=0   ← NEG 内建静默回绕
  PRINT(-(0, im));             // → error[E0034]: NEG: cannot negate INTEGER_MIN ...  ✓
  ```
- 根因:`builtin_neg`(`wlwl-eval/src/lib.rs:2218-2226`)用 Rust 一元 `-i`,release 下溢出检查关闭 → **静默回绕**(debug 下会 panic 而非 E0034);`E0034` 只在 `-` 运算符路径(`-(0, x)` 等)实现。R1 门面 `math.wll:93` `ABS = IF(<(x,0), NEG(x), x)` 走的恰是会回绕的那条。
- 规范面:语言规范 §2.2 明写「`NEG` 于 `INTEGER` 下界取负产生 `E0034`」、§1.6 实现注称 NEG 与前导符号字面量「可观察行为等价」——两句都被实测否定;stdlib §7 的 `ABS` 失败格(`E0034`,D11-007 刚更正过)同样落空,且 **D11-007 的更正前提「实现走 NEG,一直是 E0034」对 NEG 内建而言不成立**(静态审查也在此点上被实测推翻)。`str_math_contract.rs` 无 `abs_int_min` 用例,故全绿照不出来。
- 处置建议:① `builtin_neg` 改走与 `-` 同一条 checked 路径(一行修);② 补 `abs_int_min` 契约用例;③ 登记偏差。

**D-2(P2)|`SQRT(-0.0)` 返回 `+0.0`,丢失符号**
- §7 明文「`-0.0` 得 `-0.0`」。实测 `PRINT(SQRT(-0.0))` → `0.0`;用显示层对照(`*(0.0, -(0,1.0))` 能打出 `-0.0`)证明显示层不吞符号,是值本身变成 `+0.0`。边界保真问题,不涉及数值正确性。

**D-3(P2)|`SORT(arr, cmp)` 二参比较器重载未见于成员表**:stdlib §5 只登记 `SORT(arr) / SORT_BY(arr, key)`,实现是 1–2 元(D11-004 的讨论也建立在二参形式上)。按 ADR-0023 的治理纪律,可观察面与规范表应集合相等——补文档或声明为非规范行为。

**D-4(P1,即 D11-004 挂账项的复核与机制更正)|SORT 比较器返 ERR:缺陷仍在,真实机制与两份记载都不同**
- 实测(嵌入轨与覆盖轨一致):
  ```wlwl
  LET(cmp, FUN((a, b), ERR("boom")));
  LET(r, SORT([2, 1], cmp));
  PRINT(IS_ERR(r), TYPE(r));   // → FALSE ARRAY;PRINT(r) → [2, 1];rc=0
  ```
  对照:比较器返非布尔非 ERR(如 `5`)→ `error[E0030]: SORT: comparator must return BOOLEAN, got integer` 如期浮出。§5「成员回调抛出的 ERR 使整个调用按 8.2 传播」对其余 8 个回调成员全部成立(逐条实测 `IS_ERR` = TRUE),**唯 SORT 不成立** —— 与 D11-004「挂账未修」的结论一致。
- **机制更正**(通过覆盖副本注入探针逐层实证,台账与静态审查的两种叙述都不准确):
  1. `_SORT_LT` 的比较器分支里 `lt = ERR`,`TYPE(lt)` = `"RESULT"`(TYPE 是 ERR 消费者,探针实测),故 `==(TYPE(lt),"BOOLEAN")` 为 FALSE,**else 分支确实被进入**;
  2. else 的消息构造 `_CAT("SORT", ": comparator ...", _KIND(lt))` 中,`_KIND` **不是** ERR 消费者 → `_KIND(ERR)` 短路成 ERR → `_CAT` 实参含 ERR → 整个 `_CAT` 短路成 ERR → `_DIAG_E0030` **调用从未发生**(实参求值已短路);
  3. 该 ERR 作为 `_SORT_LT` 的值返回 → 在 SORT 循环里经 `||`/`IF` 条件位继续作为**值**传播 → 在语句位被静默丢弃 → `best` 永不更新 → 选择排序退化为「每轮取剩余首个」→ **原序返回,零诊断**。
  - 探针证据:在 else 分支前插 `PRINT("ELSE-REACHED", _KIND(lt))` 后,`ELSE-REACHED` 一个字都没打出来(同一短路把探针 PRINT 也吞了);而在 `LET(lt,...)` 后插 `PRINT(IS_ERR(lt), TYPE(lt))` 正常打出 `TRUE RESULT`。
- 修复指引(比台账记载的「比较结果先落变量」更直接):在 `_SORT_LT` 比较器分支**最前面**加 `IF(IS_ERR(lt), lt, ...)`(IS_ERR 是消费者,能接住),或在 `CALL` 返回后先判 `IS_ERR` 原样返回;同时把 D11-004 的症状与机制描述更新为本节版本,并补一条 comparator-ERR 冻结用例——当前契约表只有 `sort_by_key_err`,这一行为完全未钉。

**D-5(P3)|`W0051` 弃用警告文案过时**:随包 `interp.wll`/`phase2_demo.wll` 触发的警告仍写「will be removed in v0.5」——当前 v0.11,v0.5 从未发生(stdlib §11 的现行口径是「到期移除」记为演进方向)。另:该警告的位置显示 `0:0`(parser lint 无真实 span)。

**D-6(P2)|混合类型排序「保留原序」的文档口径不成立**:§5 写「混合类型不可比时保留原序」,实现源码注释的正典例子 `SORT([1,"a",2]) → [1, a, 2]` 成立,但一般情形不成立——`SORT([2,"a",1]) → [1, 2, a]`、`SORT([3,"a",1,"b",2]) → [1, 2, 3, a, b]`:不可比对("a" 与 1)的相对顺序被改变(可比组各自内排、组间按「不可比 = 不小于」的择序策略重排)。行为确定且无害,但「保留原序」按字面读是错的,建议 §5 改写为准确描述(如「不可比元素不报错;各组内稳定,组间次序由择序策略决定」)。

### 2.3 与规范逐条比对中确认无误的抽样

`SPLIT_LINES` 四边界、QUOTE 转义、`CHAR_AT` 越界=SUB(返回空串不报错)、`COUNT` 非重叠、`MIN/MAX/CLAMP` 提升含三参、`FLOOR/CEIL/ROUND` 非 `INT`(向零截断对比)、`±2^53` 外原样、`SQRT/POW/CLAMP` 域违例为 ERR 值、`E0038` 带名消息、`RUN_TESTS` 字典记录、`E0046–E0048` 载荷、`EXPECT_ERR` 捕 ERR 值不捕原生码、退出码 0/1/3 语义、`check` 不做名字解析、浮点除零 `E1003`(整数浮点皆然,§2.2:176 明文)。

---

## 第三部分 wlwl-skill 质量评价

### 3.1 评价方法

静态:将 SKILL.md / reference.md / examples 全部内容与两册 v0.11 规范逐条对照(审查代理执行);动态:实跑 skill 全部 18 个示例,并以「按 skill 写代码」的视角使用发布包。

### 3.2 结论:**6/10 —— 语言核心部分是高质量的,v0.11 增量零收录,且不能盲信**

**做得好的(值得保持)**
- 结构对 AI 友好:SKILL.md 前置「何时加载/不加载」、写作 checklist、24 条反模式表、「`wlwl run` 是唯一真相源」的验证闭环;reference.md 按需查表分工清晰。
- 示例可跑性极佳:18 个示例全部退出 0,并发/OOP/模块/签名/密封面示例的期望输出注释与实跑逐行吻合。
- 「实测 over 规范」的内容是全包最有价值的部分:YIELD 逐形态实测表、注解文法陷阱表(箭头形式被静默吸收等)、§8.3 十三消费者名单、`EXPECT_ERR` 捕不到 `E1003` 等,均与 v0.11 实况一致;v0.10 评审点名的幻码 `W0014`、失效的 stale 警告确已清理。
- 本次动态测试的语法基线(前缀调用、`FUN`、`LET MUT/SET`、`SCOPE/SPAWN/CHANNEL_NEW/AWAIT`、`CLASS/NEW/CALL_METHOD`)与 skill 示例一致,全部可用。

**问题(按危害排序)**
1. **会直接导致错误代码的两条「实测结论」已被 v0.11 实测证伪**:① skill 称 `gradual_typing` 只管 `E0110`–`E0112`、`E0113`–`E0115` 与开关无关且「已对实现验证」——规范 §11.2/§9.6 明文管辖 `E0110`–`E0115`,发布包实测:无 `[features]` 时签名漂移零诊断,加 `gradual_typing="error"` 后 `E0114` 触发;② skill 称 `wlwl check` 做名字解析——规范 §11.4 明文不做,实测 `check PRINT(NOPE)` 退出 0。skill 的可信度模型恰恰建立在「实测结论」上,这两条失效危害倍增。
2. **v0.11 全部用户可见增量零收录**:`std.str`/`std.math` 两个新命名空间(16 个新成员)在 SKILL.md 与 reference.md 中完全缺席;`ASSERT` 八假值 breaking(D11-010)无任何提示——AI 若按旧知识写 `ASSERT(LEN(xs))` 这类代码,v0.11 下会静默变失败。这正是「skill 是否让我掌握了语言」的最直接扣分项:本次动态测试中,标准库部分我完全依赖 stdlib 规范与随包 `.wll` 源码,skill 提供不了任何帮助;而若我先读了 skill,反而会带着两条错误结论进场。
3. **六处「注释声称的输出与实跑不符」**:如 `static_contracts.wll` 的 `max` 实为 min(`max(3,9)`→`3`,注释还把错值固化);`truthiness.wll`/`interp.wll` 的 `FORMAT("{0}: …", [label])` 踩进自家反模式 #9(输出 `[FALSE]: falsy` 而注释称 `FALSE: falsy`);`match.wll:60-67` 的 ERR 变体模式演示因 §8.2 传播 + §8.5 静默丢弃而**整行静默消失**,教学点实际只演示了一半。对「注释即文档」的教学包,这比跑不起来更危险。
4. **处方级错误**:`SUB` 迁移公式写成 `-(start, end_old)`(应为 `-(end_old, start)`),照抄必产出错程序;`INDEX` 的 1-based 口径只字未提(规范明言这是「最容易踩的坑」)。
5. **时效性**:CHANGELOG 停在 [0.10.1];README 开头自称 v0.10、L31 又说 v0.11,自相矛盾;幻码 `W0015`/`W0040` 仍列为活警告;附录 G 条数说 110(实际 106);`wlwl.lock` 全包零解释;SKILL.md:571 承诺的「~70 名成员总表」在包内不存在(与 reference.md §9 互相踢皮球)。
6. 残留幻码级小项:引用失效的「spec §16.3」(v0.11 重编号后格式化器在附录 A.3)、`E0012` 被误作返回类型失配码(实为 `E0112`,`E0012` 是语法错)、退出码表缺 2/3。

### 3.3 回答「这套 skill 是否让我很好地掌握了 wlwl」

**部分掌握,且带毒。** 对 v0.9–v0.10 时代的语言核心(真值、控制流、MATCH、错误模型、OOP、并发、模块),skill 是我见过的同类产物中相当优秀的一份:速查准确、反模式实用、实测表可信。但 (a) 它在 v0.11 发布日原地踏步,本版全部增量(标准库底座)无法经由它学会;(b) 它的两条招牌「实测结论」已经在 v0.11 变错,而包内没有任何机制提示「这条结论有版本有效期」;(c) 示例注释的六处失真会以「验证过的输出」的形式被 AI 当真理背下来。**建议**:升级到 v0.11 后再使用,升级清单按危害排序为——两条被证伪结论、v0.11 标准库增量 + ASSERT breaking、六处示例注释、SUB 迁移公式与 INDEX 口径;并在 SKILL.md 头部加入「实测结论附验证日期与 spec 版本」的标注纪律,避免下一版重演。

---

## 第四部分 修复优先级建议

| 批次 | 条目 | 建议去向 |
|---|---|---|
| **A(行为修复,建议 v0.11.1)** | D-1 `builtin_neg` 溢出检查(一行)+ 契约用例;S-P1-1 real-ai feature 二选一;S-P1-2 `call_callable` Yield 穿透(修或记偏差)+ 探针用例;D-4 SORT 比较器 ERR(修复指引见 2.2,顺带了结 D11-004 挂账) | 代码 + 偏差登记 + 契约用例 |
| **B(契约/台账对齐,随 A)** | S-P1-3 compat 三处行为漂移(补偏差或回退);S-P2-3 溢出语义回摆登记;D-3 SORT 二参重载入表;D-6 「保留原序」改写;D11-004/001/012/007 相关台账文本更新(007 的前提修正) | 文档/台账 |
| **C(文档卫生,一个清扫 commit)** | S-P2-2/4/5/6/7/10、D-2(规范表态)、D-5 警告文案、S-P2-8(debug 门控或改措辞)、S-P2-9(去 `|| true`) | 文档/CI |
| **D(skill 对齐,独立批次)** | 3.2 节清单按序 | `wlwl-skill/` |

---

## 附:审查产物清单

- 动态测试:发布包 `review-tests/d01–d25`(25 个 `.wll`,含 3 个纯探针),测试过程零写入工作目录;
- 静态审查:四路并行只读审查(wlwl-std/R1、wlwl-value/M2、双轨/CLI/发布管线、wlwl-skill),全部结论经主审交叉复核;静态与动态结论冲突处(ABS 的 E0034、SORT 比较器 ERR 的机制)一律以实测为准,并在正文标注。
