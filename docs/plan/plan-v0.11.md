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

### M3 R1 首批 + 混合 test(约 4 人日)

| # | 任务 | 验收 |
|---|---|---|
| M3-1 | `std.collection` 以纯 wlwl 重写(`wl/std/collection.wll`),成员契约与 stdlib 规范 §5 逐成员对拍 | 逐成员契约测试(含 ERR 回调传播、`E0038`);原生实现删除前先双实现差分 |
| M3-2 | `std.str`(§6 成员表)纯 wlwl 落地;`std.math`(§7 成员表)混合落地:纯 wlwl 门面 + R2 浮点内核(`SQRT`/`POW`) | 成员表逐条测试;域失败返回 `ERR(["kind": "DomainError", ...])` |
| M3-3 | `std.test` 混合:R1 门面(`wl/std/test.wll`)+ R2 原生内核(`test_native.rs`) | 契约同 stdlib 规范 §8;probe 增用例 |
| M3-4 | conformance / probe 扩展:R1 模块用 `.wll` 符合性脚本自测 | 类别对照表更新 |

### M5 基准基线(约 1 人日)

| # | 任务 | 验收 |
|---|---|---|
| M5-1 | criterion 基准:R1 模块热成员(collection `MAP`/`SORT`/`REDUCE`、str `JOIN` 等)+ 值直通迁移前后对比 | 基线存档;口径沿用语言规范 §17.7 性能口径(记录 CPU / rustc / 档位,≥ 20 次取中位数) |

### M4 稳定性政策成文(约 1 人日,收尾)

| # | 任务 | 验收 |
|---|---|---|
| M4-1 | ADR-0023 终稿核对 + stdlib 规范 §0.4 政策文本 + CHANGELOG 口径(v0.x 条目纪律) | 政策在 stdlib 规范可执行:有流程、有镜像、有锁测试 |

## 4 依赖与风险

| 风险 | 处置 |
|---|---|
| `wlwl-value` 拆分牵动 `wlwl-eval`(单文件约 2.4 万行) | 纯搬移纪律:M2-2 不夹带任何语义改动;workspace 锁测试全绿为闸 |
| R1 性能不足 | 数据驱动:M5 基线为准,热点下沉 R2;调用面不动,记演进台账 |
| R1 重写语义漂移 | 契约逐成员对拍 + probe;原生实现删除前先双实现差分 |
| 文档双轨失同步 | 镜像生成器 + 锁测试(M1-3);改面必须过四件套(ADR-0023) |
| serde_json 移除后的 JSON 解析 | 解析内核仍用 serde_json(实现自由),仅边界类型改为 Value |

## 5 批次 B:`YIELD` 的续体保存(已立项)

v0.10 收口时降级为已知限制(语言规范 §17.1/§17.4 的 YIELD「挂起透明」未兑现;
裁决 D-3 = 拆两半)。本批次与批次 A 相互独立,可并行或后置;细化计划在批次 A
的 M1 落地后补入本文件,不阻塞批次 A。

## 6 承接项(v0.10 遗留,挂账)

按 [README](README.md) 开工事项第 2 条,以下遗留项逐条确认状态;**均不在批次 A
范围内**,处置与否在批次 A 收口后裁决:

| 项 | 内容 | 来源 |
|---|---|---|
| P2-6 | 非末条顶层语句的 `ERR` 被静默丢弃(既有行为,规范↔实现背离) | v0.10 合规审查 |
| §5.2.1 | 部分实测事实过时,未改规范待裁决 | v0.10 收口 |
| D10-011 | cargo deny 配置卫生 | v0.10 偏差台账 |
| v0.10.1 其余 | R10-020/021/022/025~029/040/041/050 的核验状态逐条确认 | [`../history/20260915-22.md`](../history/20260915-22.md) |

## 7 文档落点

| 文件 | 动作 | 状态 |
|---|---|---|
| `docs/plan/plan-v0.11.md` | 新建(本文件) | 已完成 |
| `docs/plan/deviations-v0.11.md` | 新建偏差分册,编号 D11-001 起 | 已完成(空册) |
| `docs/adr/0021-stdlib-layering.md` | 新建:分层模型 | 已完成 |
| `docs/adr/0022-value-passthrough.md` | 新建:值直通边界 | 已完成 |
| `docs/adr/0023-stdlib-stability-policy.md` | 新建:稳定性政策 | 已完成 |
| `docs/spec/wlwl-spec-v0.11.md` | §0.1/§9.3/§10 机制化(10.6/10.8/10.10/10.11 成员契约外迁) | 已完成 |
| `docs/stdlib/wlwl-stdlib-spec-v0.11.md` | 新建:独立标准库规范 | 已完成 |
| `docs/plan/README.md` | 当前状态表更新 | 已完成 |
| `docs/appendix_G.md` | 不动(全局内建属语言表面) | — |

## 8 验收门禁(批次 A 收口)

- `cargo fmt --check` 0 diff;`clippy -D warnings` 0;`cargo test --workspace`
  全绿(锁测试只增不破);
- probe 用例数不低于基线 136,且新增双轨 / R1 用例;
- 语言规范附录 G 与 `docs/appendix_G.md` 逐字节不变(全局内建零改动);
- stdlib 规范附录 A 镜像与实现清单锁测试双向通过(M1-3 落地后);
- M5 基线数据存档;
- 本文件与 `deviations-v0.11.md` 状态同步收口。
