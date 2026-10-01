# v0.11 构建计划 —— 标准库底座

| | |
|---|---|
| **分支** | `wip0.11` |
| **基线** | 规范 [`../spec/wlwl-spec-v0.11.md`](../spec/wlwl-spec-v0.11.md)(v2 清洗版改号,内容同 v0.10)+ v0.10.4 实现(fmt/clippy 0、workspace 测试全绿、probe 136 用例目录) |
| **立项** | 标准库底座设计方案 v2(2026-10-01 与业主逐条裁决);裁决落点见 §2,论证见 ADR-0021 / 0022 / 0023 |
| **批次** | **A 标准库底座**(本计划主体,M1–M5)· **B `YIELD` 续体保存**(已立项,§5)· 承接项(§6,挂账待处置) |

## 0 基线棘轮(2026-10-01 立项时点;批次 A 任何 commit 不得降低)

| 指标 | 基线值 | 来源 |
|---|---|---|
| workspace 测试 | 全绿(v0.10.0 收口时 1736 passed;以门禁实跑输出为准) | `cargo test --workspace`;history/20260915-22.md |
| probe 用例 | 136(立项时实测)→ M1 后 **138**,只增不减 | `impl/tests/probe/cases/` 目录数 + `EXPECTED_CASE_COUNT` 守卫 |
| 全局内建注册表 | 106 条(86 builtin + 20 词法宏),spec 附录 G 与 `docs/appendix_G.md` 逐字节不变 | `docs/appendix_G.md` 头部 |
| `wlwl-eval/src/lib.rs` 体量 | ≈24,100 行(M1 后 ≈24,250;M2 拆分前的对照基线) | `wc -l` |
| fmt / clippy | 0 diff / 0 warning | CI 门禁 |

## 1 目标与非目标

### 1.1 目标

业主裁决要点(完整论证见 ADR):

1. **双轨实现机制**(M1):标准库按三层实现 —— R0 内建层(纯 Rust,语言自身
   表达不了的原语)、R1 语言层(纯 wlwl)、R2 原生层(纯 Rust);允许「R2 原生
   内核 + R1 wlwl 门面」的混合模块。调用者只见统一调用面(全局内建免导入 +
   `IMPORT("wlwl:std.X", [...])`),实现语言不可感知。
2. **值直通**(M2,**必须**):先拆 `wlwl-value`,再解除 `wlwl-std` 的
   `serde_json::Value` 边界,最后宿主归位(collection/test 迁出
   `wlwl-eval`)。`wlwl-value` 只承载值与句柄,函数调用经 trait/回调注入,
   保持 `eval → value ← std` 单向依赖。
3. **R1 首批**(M3):`std.collection` 以纯 wlwl 重写为终态;新增 `std.str`;
   新增 `std.math`(**混合**:R1 门面 + R2 浮点内核 —— `SQRT`/`POW` 依赖
   浮点指令,纯 wlwl 不可表达;首批仅基础函数,超越函数不进本版);
   `std.test` 允许混合实现。
4. **文档独立**:标准库文档与 `docs/spec/wlwl-spec-v0.11.md` 相互独立,落
   `docs/stdlib/wlwl-stdlib-spec-v0.11.md`,自含版本号。
5. **稳定性政策成文**(M4):v0.x 按 CHANGELOG + 规范改版执行;1.0 起冻结。
6. **基准基线**(M5):criterion 对 R1 模块建立基线,作为未来下沉/上浮的
   决策依据。

### 1.2 非目标(本版明确不做)

- 面向用户的 FFI;
- 预编译快照(启动解析 + 进程内缓存即可);
- 新 R2 大命名空间实体化(net/process/env/time 仅记演进方向);
- prelude 扩张(全局内建集合不动);
- `std.ai` / `std.agent` 降级为官方包(仅记演进方向,不实施);
- `std.math` 超越函数(sin/cos/exp/log 族)。

## 2 裁决落点(逐条)

| 裁决项 | 结论 | 落点 |
|---|---|---|
| 值直通 | 必须做,纳入底座;先拆 wlwl-value 再迁 collection/test;「迁」= 解除 serde_json 边界 + 宿主归位;collection 终态 R1;test 允许混合;wlwl-value 只承载值与句柄,函数调用经 trait/回调注入 | ADR-0022,M2 |
| R1 分发 | release 默认嵌入二进制;开发覆盖仅显式 flag / debug 环境变量,不承诺稳定用户接口,不得改变导出与签名 | ADR-0021,M1 |
| std.ai / std.agent | 本版保留在 `wlwl:std.*`;降为官方包仅记演进方向 | ADR-0021;stdlib 规范 §11 |
| 新命名空间 | 批准 `std.str`、`std.math`;保留既有全局内建名,不移动、不重导出同名内建;`std.*` 只作可扩展库面 | ADR-0023(冻结规则);M3 |
| std.math 范围 | 首批仅基础函数,超越函数不进 v0.11;实现归**混合**(SQRT/POW 需浮点内核,成文时按 ADR-0021 判定准则修正归层) | stdlib 规范 §7 |
| 稳定性 | v0.x:CHANGELOG + 规范改版;1.0 起冻结 | ADR-0023 |
| 成文顺序 | 先 ADR / plan,再改规范,最后进实现 | 本计划执行序 |

## 3 批次 A:M1–M5 任务分解

执行顺序 **M1 → M2 → M3 → M5 → M4**(M4 收尾,其验收依赖全部成员面定稿)。

### M1 双轨解析与治理四件套(约 3 人日)—— **已完成(2026-10-01)**

立项单按「挂载点 → 变更面 → 预估人日 → 验收」登记(行号为落地后实测):

| # | 挂载点 | 变更面 | 验收 | 状态 |
|---|---|---|---|---|
| M1-1 | `wlwl-std/src/lib.rs:151-241`(StdSource / StdBackend / LANG_SOURCES / lang_exports / resolve);`wlwl-eval/src/lib.rs:834-840`(ResolvedSource::Std)、`:1037`(load_std 分派)、`:1125-1213`(load_std_lang:覆盖路由 + 子求值器 + collect_exports + 缓存) | `wlwl-std/src/lib.rs` 单一清单双轨化;新目录 `wlwl-std/wl/std/{str,math}.wll`(include_str! 嵌入);eval 模块加载区 | `IMPORT("wlwl:std.X")` 两轨行为一致(嵌入轨/覆盖轨/环境变量三通道实测通过);probe +2(M1_std_*_dual_track) | ✅ |
| M1-2 | `wlwl-eval/src/lib.rs:868`(ProjectContext.std_src)、`:7129-7148`(with_std_src;with_base_dir 保持 std_src,装配次序无关);`wlwl-cli/src/main.rs:60-71`(Run `--std-src`)、`:373-379`(flag > env 回退)、`:400-404 / :463-466`(run_file 装配) | CLI Run 子命令 + eval 装配;check/ast 不受影响(不解析名字) | `stdlib_dual_track` 锁测试:同导出面覆盖生效、漂移必须 `E0023`、绝不静默回退嵌入版 | ✅ |
| M1-3 | `wlwl-eval/src/stdlib_mirror.rs`(NAMESPACE_META + members + splice);`bin/gen_appendix_a.rs` + Cargo.toml `[[bin]]`;锁测试 `wlwl-eval/tests/stdlib_appendix_a_sync.rs`;spec 标记区 `<!-- appendix-a:begin/end -->` | stdlib 规范附录 A 改由生成器产出(手改无效) | 镜像逐字节对账锁测试双向通过;`cargo run --bin gen-appendix-a` 幂等 | ✅ |
| M1-4 | `.github/workflows/release.yml` staging 段 | release 产物增附 `stdlib/`(R1 源码参考副本)+ `docs/stdlib/` | 发布产物含两目录(本地 staging 逻辑同源) | ✅ |
| M1-5 | spec §0.1/§9.3/§10 机制化 | 上一批成文完成 | 规范交叉引用完整;附录 G 锁测试全绿 | ✅ |

### M2 值直通(约 4 人日)—— **已完成(2026-10-01;wlwl-value 全量落地,collection/test 宿主归位,名录特判与 serde_json 边界废止)**

| # | 挂载点 | 变更面 | 验收 |
|---|---|---|---|
| M2-1 | `wlwl-eval/src/lib.rs:51`(`pub enum Value` 十三类)、`:66-70`(NativeFn 携带名与调用标签)—— 布局**纯搬移** | 新 crate `wlwl-value`:`Value` + `Handle`(宿主侧对象不透明句柄)+ `Callable` trait(回调注入点);不依赖 eval/std | 独立编译;cargo-deny 通过 |
| M2-2 | `wlwl-eval/src/lib.rs` 全文件对 Value 构造/匹配的引用;`value_to_std_value`(`:1520` 起,serde_json 边界实况) | eval 改依赖 `wlwl-value`,为 wlwl 闭包实现 `Callable`(句柄 → 求值器注册表) | **纯搬移不改语义**;workspace 锁测试全绿 |
| M2-3 | `wlwl-eval/src/lib.rs:1520-1610` 的 `value_to_std_value` / `std_value_to_value` 双向转换层;`wlwl-std/src/lib.rs` StdCtx(`:105-135`) | 原生函数直接收发 `Value`;StdCtx 增 Callable 注入槽;实现内部中间表示自选(JSON 解析仍用 serde_json),**跨界类型只有 Value** | 边界转换代码删除;std 各模块测试全绿 |
| M2-4 | `wlwl-eval/src/collection.rs` / `test.rs` 全文;`load_std_native` 的名录特判(`:1064-1089`);`wlwl-std/src/lib.rs:11-17` 既有契约注释 | collection/test 迁至 `wlwl-std`(test 原生内核落 `test_native.rs`);名录特判与「std 边界拒绝闭包值」契约废止 | eval 不再含 std 实现;混合实现可持闭包 |

### M3 R1 首批 + 混合 test(约 5 人日;原估 4,差额见 M3-0 接线机制)

侦察于 2026-10-01(commit `30b70a7`),行号为**抵达时实测**;立项单格式同 M1/M2。

**M3-0 前置裁决(ADR-0021 §层间规则的落地机制,不是新裁决)**

ADR-0021:89 已定「R1 可以调用 R0 与 R2」,M1 只落了 R1→R0 一半(闭包调全局
内建)。M3 的三个模块里有两类东西**纯 wlwl 表达不出来**,必须在混合门面上
留一条到 R2 的私有通道:

1. **浮点内核**(`SQRT` / `POW`)—— ADR-0021:84 已预告;
2. **指定码的原生诊断**—— 侦察实测(临时脚本跑 `target/debug/wlwl`):
   `PANIC(msg)` → `E0100`;`LEN(1)` → `E0030` 但消息由 `LEN` 固定;
   wlwl 闭包元数错 → `E0022` 但消息**无函数名前缀**
   (`function expects 1..1 argument(s), got 2`)。
   而 `E0038`(`RANGE` 步长为零,规范 §5 / 语言规范 §11)目前**全仓唯一发射点
   就是待删的 `wlwl-std/src/collection.rs:528`** —— 删掉 R2 实现后 wlwl 侧
   无路可走,除非注入发射器。

**机制**:`StdSource` 增私有 `kernels: &'static [(&'static str, StdFn)]`
注入槽;`load_std_lang` 在 `eval_module` **之前**把 kernel 以局部绑定注入子
求值器;只有 `EXPORT` 名单进入返回的模块 env(`wlwl-eval/src/lib.rs:484-497`),
故 kernel **不外泄**到 `IMPORT` 方 —— 门面仍是唯一对外契约层
(ADR-0021:90 / 规范 §0.2)。kernel 只在 Rust 侧固定,`--std-src` 覆盖轨同样
注入,故覆盖不可能改变导出面(M1 的 E0023 漂移守卫不受影响)。

**被否决的替代方案**:让 R1 靠「误撞」可发的原生码(如 `LEN(1)`)来报类型错 ——
码对了但消息指向 `LEN` 而非 `MAP`,诊断反而更糟。

| # | 挂载点 | 变更面 | 验收 |
|---|---|---|---|
| M3-0 | `wlwl-std/src/lib.rs:66-69`(`StdSource`)、`:101-110`(`LANG_SOURCES`)、`:289-298`(`ALL_SPECS`)、`:391-421`(命名空间守卫);`wlwl-eval/src/lib.rs:435-506`(`load_std_lang`,注入点落在 `:478` `new_with_loader` 之后、`eval_module` 之前) | 新增 `wlwl-std/src/kernels.rs`:R1 注入的 R2 内核表(**非命名空间** —— 无 `SPEC`、不进 `resolve()`、不进附录 A;命名空间守卫的豁免名单扩到它并写明理由)。首批 kernel:`_KIND(x)`(保 R2 消息措辞)、`_DIAG_E0020/_E0030/_E0038(msg)`、`_SQRT/_POW` | 注入面**不外泄**:一条断言「kernel 名不得出现在任一 std 模块的 `EXPORT` 里」;kernel 表为空时 `load_std_lang` 行为与 M1 等价(M1 既有两条门禁测试不改动即绿) |

| # | 挂载点 | 变更面 | 验收 |
|---|---|---|---|
| M3-1 | 新建 `wlwl-std/wl/std/collection.wll`(17 成员,`EXPORT` 顺序对齐原 R2 `NAMES`);`wlwl-std/src/collection.rs:1-925`(**删**);`wlwl-std/src/lib.rs:36`(`pub mod collection`)、`:152`(`resolve` 键)、`:19-20`(crate 目录注释)、`:296`(`ALL_SPECS`);`wlwl-eval/src/collection_tests.rs:4-5,53-65,67-85,88-108,110-135`;`wlwl-std/src/test_native.rs:280`(`collection::call_callable` 调用点) | 纯 wlwl 实现 §5 全部 17 成员;`call_callable` 上移到 `wlwl-std/src/lib.rs`(与既有 `value_kind` / `wrap` 同处)—— **`arity` / `type_err` / `not_callable` / `short_circuit_err` 不上移**:它们的唯一调用方是 R2 实现,已随 R2 删除,搬上来即死代码(见 [D11-005](deviations-v0.11.md));`value_kind` 收敛到 `wlwl_value::type_name` 单一来源([D11-002](deviations-v0.11.md));`names_match_catalog` 改为对拍 **标准库规范 §5 表格** | ① `wlwl-eval/tests/collection_contract.rs`:75 条冻结用例逐条对拍(§5 全表 + 回调 `ERR` 传播 + `RANGE` step=0 → `E0038` + 元数/类型/非布尔谓词);**冻结顺序**:R2 仍在位时跑一遍把实际值写进 `expect` 并验证绿,再翻路由删 R2 跑同一张表 —— 差分就是这么做的,产物是表本身;② eval 侧既有 `b6_*` 约 40 条测试不改动即通过(它们自动改跑 R1);③ 与规范 §5 的外部对照(集合相等,不比顺序 —— §5 未规定次序) | ✅ |
| M3-2 | `wlwl-std/wl/std/str.wll:1-21`(补 `JOIN`/`SPLIT_LINES`/`CHAR_AT`/`COUNT`);`wlwl-std/wl/std/math.wll:1-5`(补 `MIN`/`MAX`/`FLOOR`/`CEIL`/`ROUND`/`CLAMP`/`PI`/`E` + `SQRT`/`POW` 门面);`wlwl-std/src/kernels.rs`(M3-0,新增 `_SQRT`/`_POW`);`wlwl-eval/src/stdlib_mirror.rs:27-42`(`NAMESPACE_META` 的「引入」列) | §6/§7 成员表逐条落地;`cargo run --bin gen-appendix-a` 重生成附录 A;规范 §6/§7「落地状态」行改写 | §6/§7 成员表逐条测试;`SQRT(x<0)` / `POW(0, 负)` / `POW(负底, 非整数)` / `CLAMP(lo>hi)` 返 `ERR(["kind": "DomainError", ...])`;`ABS(INT_MIN)` → `E0035`;`CHAR_AT` 越界口径 = `SUB`;`COUNT(s, "")` → `E0030`;附录 A 锁测试(`wlwl-eval/tests/stdlib_appendix_a_sync.rs:23`)绿,生成器幂等 |
| M3-3 | 新建 `wlwl-std/wl/std/test.wll`;`wlwl-std/src/test_native.rs:50-57`(`NAMES`)、`:114-133`(`TEST`)、`:257-384`(`RUN_TESTS`)、`:428-438`(`SPEC`);`wlwl-eval/src/test_native_tests.rs:5-17`(`names_match_catalog`) | R1 门面导出 6 成员;`ASSERT` 族载荷 `DICT`(`{code, kind, cond?, msg?}` / `{code, kind, actual, expected, msg?}`)的**构造**移到门面(纯 wlwl 可表达),注册与计时留 R2(`StdCtx::tests` + `Instant`);`resolve` 改挂 `LANG_SOURCES`;`names_match_catalog` 改为对拍 R1 `EXPORT` | 契约同规范 §8:6 成员签名、`E0046`–`E0049`、`RUN_TESTS` 记录字典(至少 `name`/`passed`/`duration_ms`,按结果附 `return_value` 或 `error`)、体逃逸 `ERR` → `passed=FALSE`;eval 侧既有 `b7_*` 约 30 条测试经解释器不改动即通过;probe 增用例 |
| M3-4 | `impl/tests/conformance/`(新增 `stdlib_r1_collection.wll` / `stdlib_r1_str.wll` / `stdlib_r1_math.wll`);`impl/crates/wlwl-cli/tests/conformance.rs:61-72`(`WLT_FILES`);`impl/crates/wlwl-cli/tests/probe.rs:51-55`(`EXPECTED_CASE_COUNT`)、`:127-135`(计数守卫);`impl/crates/wlwl-cli/tests/stdlib_dual_track.rs:67-131`(覆盖轨现只测 `std.str`) | R1 模块用 `.wll` 符合性脚本自测;probe 计数守卫同步递增;双轨锁测试扩到 `collection` / `math`(证明覆盖轨不因 kernel 注入而漂移) | 符合性脚本进 `WLT_FILES` 且跑绿;`EXPECTED_CASE_COUNT` 与目录数一致;覆盖轨对 `collection` / `math` 同导出面生效(输出切换证明真走覆盖目录) |

### M5 基准基线(约 1 人日)

| # | 挂载点 | 变更面 | 验收 |
|---|---|---|---|
| M5-1 | `wlwl-eval/benches/eval_hot_paths.rs:59-68`(`simple_loop_1m`,驱动是 `RANGE(1, 1000001, 1)`)、`:129-143`(`array_higher_order`)、criterion 组;`wlwl-eval/benches/baseline.txt`(新增 M5 段) | 新增 5 个 R1 热成员 bench(`r1_range_build` / `r1_collection_map` / `r1_collection_sort` / `r1_str_join` / `r1_math_facade`);`baseline.txt` 增 M5 段,记 §17.7 口径(CPU / rustc / 档位 / LTO / 样本数)与**结论** | ⚠️ **已实测出决定性结论,待业主裁决**:`RANGE` 的 R1 化在规模上不可用(10k 元素 3.6 s、40k 元素 86 s,每元素成本超线性),`simple_loop_1m` 的 100 万元素工作负载跑不完 —— 时间全消耗在**循环开始之前**。根因:R1 用 `PUSH` 建数组而 wlwl 数组不可变,每次 `PUSH` 复制整个数组。详见 `baseline.txt` M5 段与偏差 D11-012。**业主 2026-10-01 裁决:`RANGE` 单独沉回 R2,其余 16 成员仍 R1**(已落地,75 条冻结用例复跑全绿) | ✅ |

### M4 稳定性政策成文(约 1 人日,收尾)

| # | 挂载点 | 变更面 | 验收 |
|---|---|---|---|
| M4-1 | `docs/adr/0023-stdlib-stability-policy.md`;`docs/stdlib/wlwl-stdlib-spec-v0.11.md:69-77`(§0.4);`CHANGELOG.md` `[Unreleased]` | ADR-0023 终稿核对 + §0.4 政策文本复核(v0.x 四件套同步 / breaking 明标 / 弃用两步 / 不重导出同名全局内建)+ M3 各批 breaking 口径 | 政策在 stdlib 规范可执行:有流程、有镜像、有锁测试;批次 A 收口按 §8 门禁逐条过(fmt 0 diff / clippy `-D warnings` 0 / workspace 全绿 / probe ≥138 / 附录 G 逐字节不变 / M5 基线存档) |

## 4 依赖与风险

| 风险 | 处置 |
|---|---|
| `wlwl-value` 拆分牵动 `wlwl-eval`(单文件约 2.4 万行) | 纯搬移纪律:M2-2 不夹带任何语义改动;workspace 锁测试全绿为闸 |
| R1 性能不足 | 数据驱动:M5 基线为准,热点下沉 R2;调用面不动,记演进台账 |
| R1 重写语义漂移 | 契约逐成员对拍 + probe;原生实现删除前先双实现差分 |
| **`RANGE` 是 R1 化的性能放大器**(M3 新增实测风险) | `simple_loop_1m` 基准的驱动就是 `RANGE(1, 1000001, 1)`(bench `:59-68`)。R2 版一次原生循环产出 100 万元素;R1 版要由解释器跑 100 万次 `WHILE` + `PUSH`。**处置:M5 实测记账**;若越过语言规范 §6.6 的「100 万次简单循环 < 30 s」预算,按上一行「热点下沉 R2」把 `RANGE` 单独留 R2(其余 16 成员仍 R1),登记偏差 —— 归属变更按 ADR-0021 §0.2 不算破坏性变更 |
| 诊断**码**不变但**措辞**变(M3 新增) | R1 闭包的元数检查由解释器发出,消息无函数名前缀(`function expects 1..1 argument(s), got 2` vs R2 的 `MAP: function expects 2 argument(s), got 1`)。处置:码严格守恒(差分对拍**码**与**值**),措辞变化登记偏差 + CHANGELOG 明标;不追求逐字节复刻 R2 文案 |
| `value_kind` 措辞分叉(M3 新增实测) | 现有两套:`collection::value_kind`(`"function closure"`/`"native fn"`/`"RESULT err"`)与 `crate::value_kind` = `wlwl_value::type_name`(`"function"`/`"native-function"`/`"err"`),同仓两套 E0030 措辞。处置:M3-1 收敛为单一来源,选边登记偏差 |
| 文档双轨失同步 | 镜像生成器 + 锁测试(M1-3);改面必须过四件套(ADR-0023) |
| serde_json 移除后的 JSON 解析 | 解析内核仍用 serde_json(实现自由),仅边界类型改为 Value |

## 5 批次 B:`YIELD` 的续体保存(已立项,未细化)

v0.10 收口时降级为已知限制(语言规范 §17.1/§17.4 的 YIELD「挂起透明」未兑现;
裁决 D-3 = 拆两半)。本批次与批次 A 相互独立,批次 A 已于 2026-10-01 收口。

> **原承诺已作废并在此更正**:本节原先写着「细化计划在批次 A 的 M1 落地后补入
> 本文件」—— M1 早已落地(`d56cf08`)而细化计划始终未补。**批次 B 至今 0
> 细化**,故不写成「已立项」以免读成「已就绪」。
>
> **开细化前必须先定的三件事**(缺一都会让计划写不下去):
>
> 1. **「续体保存」的数据形态**:续体是解释器状态(调用栈 + 局部绑定 + 挂起的
>    `Outcome`/`Signal`),落盘要么序列化整个帧,要么改写成可重放的显式栈 ——
>    这两条路的复杂度差一个数量级,且决定是否需要 R0 解释器改动。
> 2. **与批次 A 遗留的 `MAP` 平方级成本是否合并**:两者都指向「语言层缺少
>    某些原语」这一根(前者缺续体存储,后者缺线性建数组)。合并成一个「R0 原语
>    补齐」批次,还是分两批 —— 属范围决策,未裁。
> 3. **是否需要 `Serialize` 依赖进 `wlwl-value`**:当前值层刻意零依赖
>    (ADR-0022 的单源设计),引入序列化会改变它的依赖画像。
>
> 这三条都需要侦察(`wlwl-eval` 的 `Outcome`/`Signal`/`SPAWN` 路径)与业主
> 裁决,不在本文件凭猜测补写。

## 6 承接项(v0.10 遗留)

按 [README](README.md) 开工事项第 2 条,以下遗留项在**批次 A 收口后(2026-10-01)逐条裁决**:

| 项 | 内容 | 裁决(2026-10-01) |
|---|---|---|
| P2-6 | 非末条顶层语句的 `ERR` 被静默丢弃(既有行为,规范↔实现背离) | **维持挂账,且已不是「未登记的背离」。** 实测复现:`ERR("dropped"); PRINT("after")` → 只打 `after`,rc=0。但语言规范 **§8.5 已把它逐条写成已知限制**(「首条 / 中间一条是 `ERR(...)`,末条是 `1` → rc=0,`ERR` 静默丢弃,继续执行」),并给了风险面说明。所以它是**规范承认的现状**,不是待补的漏洞;要改语义属语言层裁决,不在 stdlib 批次。 |
| §5.2.1 | 部分实测事实过时,未改规范待裁决 | **本次未裁决 —— 缺可核的标的。** 该条在 v0.10 收口时写的是「部分实测事实过时」,**没有点名是哪几条**;现状 §5.2.1(类型注解的静态解释)经查与 `strict_types` / `gradual_typing` 两个开关的当前实现一致,未发现过时处。若确有具体条目,需由提出者点名后另立。 |
| D10-011 | cargo deny 配置卫生 | **已根治(偏差 D11-016)。** CI run #167 证实它在三平台全红。本地复现后查明是两层根因:`impl/Cargo.toml` 里 10 条内部依赖只写 `path` 不写 `version`(cargo 解析成通配 `*`,而 `deny.toml` 的 `wildcards = "deny"` 判红),外加 4 处**直接** `path =` 的依赖(`allow-workspace = true` 覆盖不到)。按业主裁决走根治而非加 skip:补 `version` + 那 4 处改走 `workspace = true`,并**清空 `deny.toml` 的 10 条 skip**(删掉后又暴露出被它掩盖的另外 4 处 wildcard,一并改掉)。现 `cargo deny check` exit 0、四项全 ok。原表是 [v0.10] Step 1 为绕同一个错建的,代价是「每加一个内部 crate 记得补一行」,M2 加 `wlwl-value` 就忘了 —— 通配要求消失后这个人为陷阱也随之消除。 |
| v0.10.1 其余 | R10-020/021/022/025~029/040/041/050 的核验状态逐条确认 | **本次未裁决 —— 逐条记录不在当前文档内。** `docs/history/` 只保留两份合并纪要,其中与 R10 相关的 6 处讲的是**评审过程**而非逐条核验状态;逐条记录在 v0.10.0 时期的审查报告里,已按 README 的归档规则并入 git 历史(`git log --follow`)。要逐条裁决需先取回那些报告 —— 属独立的一轮文档考古,不夹进本批次。 |

**小结**:4 条里 1 条确认维持挂账(且已被规范承认为已知限制)、1 条从「待办」升级为「在红的活项」(D10-011)、2 条因**标的缺失**明确本次不裁决并写明了缺什么。前者不拖批次,后者要么等 CI 实跑、要么需先取回归档材料。

## 7 文档落点

| 文件 | 动作 | 状态 |
|---|---|---|
| `docs/plan/plan-v0.11.md` | 新建(本文件) | 已完成 |
| `docs/plan/deviations-v0.11.md` | 新建偏差分册,编号 D11-001 起 | **已完成(D11-001 – D11-013,仅 D11-004 挂账)** |
| `docs/adr/0021-stdlib-layering.md` | 新建:分层模型 | 已完成 |
| `docs/adr/0022-value-passthrough.md` | 新建:值直通边界 | 已完成 |
| `docs/adr/0023-stdlib-stability-policy.md` | 新建:稳定性政策;M4-1 加「终稿核对」表,逐条映射到强制它的机制 | **已完成**(D11-013 更正了一处不准确的前提) |
| `docs/spec/wlwl-spec-v0.11.md` | §0.1/§9.3/§10 机制化(10.6/10.8/10.10/10.11 成员契约外迁) | 已完成 |
| `docs/stdlib/wlwl-stdlib-spec-v0.11.md` | 新建:独立标准库规范 | 已完成 |
| `docs/plan/README.md` | 当前状态表更新 | 已完成 |
| `docs/appendix_G.md` | 不动(全局内建属语言表面) | — |
| `docs/stdlib/wlwl-stdlib-spec-v0.11.md` §0.3/§5/§6/§7/§8 | 各章「落地状态」行改写;§5 补两处**已实现未文档**的过载与「混合类型不可比保留原序」;§8 `ASSERT` 标「自 v0.11 变更」;§0.3 把 `.wll.sig` 移出四件套并补第三条外部对照;附录 A 由 `gen-appendix-a` 重生成 | **已完成**(D11-007 – D11-010 / D11-013) |
| `docs/adr/0021-stdlib-layering.md` §层间规则 | M3-0 的 kernel 注入是 ADR-0021:89「R1 可以调用 R0 与 R2」的落地,不新增裁决 —— 仅在 §层间规则下补一段机制说明(注入槽 + 不外泄) | **已完成** |
| `CHANGELOG.md` `[Unreleased]` | M3 / M5 收口补条目;两条 breaking(`ASSERT` 真值口径、`RANGE` 层变更)明标 | **已完成** |

## 8 验收门禁(批次 A 收口)

- `cargo fmt --check` 0 diff;`clippy -D warnings` 0;`cargo test --workspace`
  全绿(锁测试只增不破);
- probe 用例数不低于基线 136,且新增双轨 / R1 用例;
- 语言规范附录 G 与 `docs/appendix_G.md` 逐字节不变(全局内建零改动);
- stdlib 规范附录 A 镜像与实现清单锁测试双向通过(M1-3 落地后);
- M5 基线数据存档;
- 本文件与 `deviations-v0.11.md` 状态同步收口。

### 门禁自检(2026-10-01,批次 A 收口)

| 门禁 | 结果 |
|---|---|
| `cargo fmt --check` | 0 diff |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 warning |
| `cargo doc --no-deps -D warnings` | 0 error(**2026-10-01 CI 首次实跑后才补进本地门禁**;此前漏跑,4 处 rustdoc 断裂潜伏了两个里程碑 —— 偏差 D11-015) |
| `cargo test --workspace` | 47 套件 **1860 passed / 0 failed** |
| `cargo deny check` | **exit 0**(此前 exit 2 —— 内部 path 依赖只写 `path` 不写 `version` 被判通配;2026-10-01 根治,`deny.toml` 的 10 条 skip 同时清空。偏差 D11-016) |
| probe 用例数 | 136(立项)→ 138(M1)→ **142**(M3-4),只增不减 |
| 附录 G 逐字节不变 | 未触碰 `registry.rs` 与 `docs/appendix_G.md`;`spec_appendix_g_sync` 绿 |
| 附录 A 镜像双向锁 | `stdlib_appendix_a_sync` 2/2 绿 |
| M5 基线存档 | `benches/baseline.txt` 的 M5 段(§17.7 口径 + 结论 + 局限) |
| 偏差台账 | D11-001 – D11-013,**仅 D11-004 挂账**(`SORT` 比较器返 `ERR` 时返回值被拆成载荷 —— R2 既有行为,原样保留,修复需独立裁决) |
| 成员面外部对照 | collection 17 / str 5 / math 11 / test 6,与规范 §5/§6/§7/§8 表格集合相等 |
| 冻结行为契约 | collection 75 / str+math 82 / test 58 条,诊断码与消息逐字 |

**未完成 / 已知局限**(不掩盖):

1. **D11-004 挂账**:`SORT` 的自定义比较器返回 `ERR` 时,返回值被拆成**载荷**
   而非 `ERR` 包装。这是 R2 既有行为,M3 按「规范沉默处保留既有实现」原样
   搬过来了;要修属独立的语义裁决,不在批次 A。
2. **建数组类成员的平方级成本未解**(D11-012):`RANGE` 已按业主裁决沉回
   R2,但 `MAP` / `FILTER` / `FLAT` / `UNIQ` / `ENUMERATE` / `GROUP_BY` /
   `JOIN` 在 1000+ 元素上仍是平方级 —— 根因是语言层的不可变数组
   (`PUSH` 每次复制),修复属 R0 解释器工作,超批次 A 范围。
3. **`criterion` 20 样本的完整行待补**:R1 新 bench 的单次迭代在秒级到
   十秒级,20 样本要几十分钟;且 M5 裁决改了工作量,现在跑会作废。待
   §1.1「批次 B」或下一个迭代按稳定工作量补齐。
