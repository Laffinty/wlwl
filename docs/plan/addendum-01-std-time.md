# 01 · `std.time` —— 时间与假时钟气泡(参考 Go `testing/synctest`)

> **层级** L1(可并行) · **前置** 无 · **被依赖** 12(同步原语)
> **状态** 未动工 · **裁决点** G3(播种模型相关)/ G6(假时钟是否影响生产代码)

## 0. 契约摘要

- **目标**:给 wlwl 第一条时间能力,并把「测超时不用真等」做成**语言级形态**
  而不是各家测试框架里的一个 helper。
- **非目标**(违反即范围事故):
  - **不做时区数据库**(tzdata / IANA zoneinfo)。理由与实体表同款:半张时区表
    比没有更糟(跨 DST 边界算错,且没有任何报错)。见 §3.1。
  - **不做日历 / 日期算术的 R1 实现**。日历成员一律 R2,理由见 §3.2。
  - 不做夏令时推断、不做 `CRON` 表达式(那是调度器,不是时间)。
  - 不改语言的挂起语义(§17 真挂起是 v0.9 既有形态,本份只接入不改写)。
- **为什么排 L1 第一**:wlwl 现在**写不出任何带超时/定时/时间戳的程序**。
  `SCOPE` / `STEP` / `SPAWN` / 真挂起是 v0.9 的能力,却没有任何时间源可挂 ——
  这是「并发做完了、时间没做」的典型半截状态。

## 1. 基线(可核验)

```powershell
cd docs
Select-String -Path stdlib/wlwl-stdlib-spec-v0.11.md -Pattern 'std.time|synctest' -Context 0,2
```

| 项 | 现值 | 出处 |
|---|---|---|
| 时间相关成员 | **0** | 附录 A(13 命名空间 / 104 成员,无 `std.time`) |
| 调度器能力 | `SCOPE` / `STEP` / `SPAWN` / 通道 / 真挂起 | 附录 G(全局内建,106 条) |
| 测试面 | `std.test` 6 成员 | 附录 A |
| 规范已有的登记 | §14:`std.time` 的参考坐标是 Go 1.24 的 `testing/synctest`;「本语言最该抄的一个标准库形态 —— 但它需要虚拟时钟设计,**不是加几个函数**」 | 规范 §14 |
| **当期证据** | Go 1.25 把 `testing/synctest` 从 `GOEXPERIMENT=synctest` **毕业到 GA**:bubble 内时间虚拟化,全阻塞时时钟**瞬时**推进;旧 API 将在 1.26 移除 | [Go 1.25 Release Notes](https://golang.google.cn/doc/go1.25) |

> 这条当期证据的价值:它证明「假时钟 bubble」不是学术玩具,是主流在
> **2025 年刚刚 GA 化**的标准库形态。规范 §14 引的是 1.24 的实验形态,
> 落地时应按 1.25 的 GA 形态写。

## 2. 成员面

**层:R2 全员**(时间必然要宿主时钟,R1 拿不到)。

| 成员 | 层 | 语义要点 | 失败口径 |
|---|---|---|---|
| `NOW()` | R2 | 墙钟。**不保证单调**,会被 NTP 调整;测「经过多久」一律用 `MONOTONIC` | 永不失败 |
| `MONOTONIC()` | R2 | 单调钟,只在进程内有意义(重启归零);**不受墙钟调整影响** | 永不失败 |
| `SINCE(t)` | R2 | 从 `t` 到现在经过的秒数(f64)。**实测的差值走这个,不走 `NOW` 相减** | 永不失败 |
| `SLEEP(d)` | R2 | 挂起当前任务 `d` 秒。**在假时钟 bubble 内瞬时完成** | `d < 0` → `E0030` |
| `TIMEOUT(d, body)` | R2 | 跑 `body`,超过 `d` 秒则放弃并抛错。生产代码与测试**同一个函数** | — |
| `TEST_BUBBLE(body)` | R2 | 在**气泡**内跑 `body`:时间走假时钟,气泡内全部任务阻塞时时钟瞬时前进。生产路径与测试路径**同一份实现** | — |
| `ADVANCE(d)` | R2 | 仅气泡内有效:把假时钟拨快 `d` 秒,并唤醒到期的等待者 | 非气泡内 → `E0030` |
| `WAIT_BLOCKED()` | R2 | 等到**当前气泡内所有任务都阻塞**为止。对应 Go `synctest.Wait` | 非气泡内 → `E0030` |

**失败的统一口径**:`SLEEP` / `ADVANCE` / `WAIT_BLOCKED` 的误用是**元数 / 类型 /
上下文错**三类原生诊断(`E0022` / `E0030`),**不是** `ERR` 值 —— 理由同
`std.encode`:把「参数写错」和「运行期结果不好」混成一个 kind,调用方就没法分。

## 3. 语义裁决点(开写前必须逐条回答)

1. **时间戳的数据形状**:`NOW()` 返回什么?`STRING`(ISO-8601)?`DICT`?
   自定义 `DATETIME`?wlwl 的类型系统很小(`INTEGER` / `FLOAT` / `STRING` /
   `ARRAY` / `DICT` / `BOOLEAN` / `NULL` / `OK` / `ERR` / 8.3 消费者)——
   **新增一个内建类型是语言变更**,不是标准库扩张。→ 建议先用 `FLOAT`
   (Unix 秒)或 `STRING`,把「要不要 `DATETIME` 类型」单独立项。
2. **假时钟的实现落点**:它需要 eval 侧提供一个「可替换的时间源 + 阻塞检测
   钩子」。这是**宿主 / 调度器**的改动,不是 `wlwl-std` 一个 crate 能关起门
   做完的。→ 必须在开工前确认落点(eval 侧 `StdHost` 扩展?还是全局内建?),
   否则会做成「只能在 `std.test` 里用」的半形态。
3. **`TIMEOUT` 的失败形态**:超时是 `ERR([kind: Timeout])` 还是原生 `E00xx`?
   建议 **`ERR` 值**(它是可预期的运行期结果,调用方多半要处理它)——
   但这与 `std.test` 现有的 `EXPECT_ERR` 体例要对齐。
4. **`TEST_BUBBLE` 放 `std.time` 还是 `std.test`?** 它只在测试里有意义。
   建议**放 `std.time`**(与 Go 一致:气泡是时间形态,不是测试形态),
   `std.test` 不新增成员。
5. **与 `STEP` 的关系**:`STEP` 是确定性调度(§17),气泡是虚拟时间。两者组合
   会不会让「确定性」这个卖点变得不可测?→ 需要一条组合用例钉住。

## 4. 验收门禁

```powershell
cd impl
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --all-targets
cargo deny --locked --all-features check
$env:RUSTDOCFLAGS='-D warnings'; cargo doc --locked --no-deps
cargo check --locked -p wlwl-std --features real-ai
cargo bench -p wlwl-eval --bench sanitize -- --warm-up-time 1 --measurement-time 2
```

契约表 `tests/time_contract.rs`(新):

| 组 | 用例 | 期望值来源 |
|---|---|---|
| 形态 | 各成员元数错 / 类型错 | 逐字冻结,与 `sanitize_contract` 同体例 |
| 单调性 | `MONOTONIC` 在循环里单调不减(1000 次采样) | 实现不变量 |
| 墙钟不单调 | **不得**断言 `NOW` 单调 —— 文档写明它不是 | 规范语义 |
| **气泡** | 气泡内 `SLEEP(3600)` **墙钟耗时 < 1 s** | 机制本身(Go 同款断言) |
| **气泡** | 气泡内两个任务交错阻塞,`WAIT_BLOCKED` 后时钟跳到预期点 | 时序用例 |
| 逃逸 | 气泡内 spawn 的任务在气泡外存活吗?逃逸则挂起 | 需裁决(§3.5) |
| 确定性 | 同一程序跑两次,气泡内的时间序列**逐字节相同** | 本语言的卖点,必须钉 |

**性能断言**:`TIMEOUT` 在气泡内的额外开销 ≤ 1 µs/次(它是测试高频路径)。

## 5. 九处表落点

九处见 `docs/history/20261002.md` §5.0。**本份九处全中**(新命名空间):

| 落点 | 本份动作 |
|---|---|
| 1 规范成员表 | 新增 §15 `std.time` 章 + 成员表 8 行 |
| 2 附录 A | 跑 `gen-appendix-a`(新增 1 行命名空间) |
| 3 `SPEC.functions` | 新 `src/time.rs` + 8 个 `StdFn` 登记 |
| 4 契约测试硬编码 | `time_contract` 的成员数与点名清单 |
| 5 模块登记 | `ALL_SPECS` 登记 `time`;`src/` 文件数 == `ALL_SPECS` 长度 |
| 6 probe | 新增 ≥ 2 条(气泡一条 / 形态一条),`EXPECTED_CASE_COUNT` 同步 |
| 7 附录 G | **可能动**:若时间源要成为全局内建或 `StdHost` 方法,附录 G 与 `spec_appendix_g_sync` 要跟着改 —— 裁决点 §3.2 的直接后果 |
| 8 skill 指针 | `SKILL.md` 指针表 + `reference.md` 新增 §9.7 |
| 9 CHANGELOG | 成员面 +8 |

## 6. 风险与已知代价

- ⚠️ **气泡需要 eval / 调度器侧改动**,工作量可能超过「一个标准库命名空间」的
  常态。先落 `NOW` / `MONOTONIC` / `SLEEP` 三条**纯 R2 无依赖**的,再单独评估
  气泡 —— 免得气泡的复杂度把三条简单成员一起卡住。
- ⚠️ **不做时区**意味着 `NOW()` 出来的墙钟是 UTC 或本地,取决于决策。
  必须写明,否则调用方会默认本地时区然后算错。**半张时区表比没有更糟**。
- ⚠️ 气泡与 `STEP` 的组合语义未验证(§3.5),可能触碰「确定性」这个卖点 ——
  这条要在第一份用例里就试,不要留到最后。
- **明确不做**:时区库 / 日历 / `CRON` / 定时器对象 / 睡眠排序。

## 7. 检查点

| 子项 | 内容 | 状态 | commit |
|---|---|---|---|
| W-01 | 纯 R2 三成员(`NOW` / `MONOTONIC` / `SLEEP`)+ 契约 | 未动工 | — |
| W-02 | `SINCE` / `TIMEOUT` + 契约 | 未动工 | — |
| W-03 | **气泡落点裁决**(ADR)+ 落点确认 | 未动工 | — |
| W-04 | 气泡实现 + 时序契约 + 逃逸用例 | 未动工 | — |
| W-05 | skill / CHANGELOG / 附录 A 收口 | 未动工 | — |
