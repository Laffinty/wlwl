# WLWL v0.12.0 正式发版前审计报告（第三份 · 静态 + 动态 + 技能包）

> **审计时间**：2026-10-26
> **实现仓**：`D:\Project\wlwl` @ `e78934561a3cff96bc0bf8116f4c8946dc325aa0`（`impl/Cargo.toml` 版本 `0.12.0`）
> **规范仓**：`D:\Project\wlwl-spec-v1` @ `294dc07b356efd39ac3e8be06334d076dac17325`（= tag `v1.0.2`）
> **编译器**：`impl\target\release\wlwl.exe` —— `wlwl --version` ⇒ `wlwl 0.12.0`；工具链 `1.99.0`（与 `rust-toolchain.toml` 钉死值一致）
> **平台**：Windows x64 / PowerShell 5.1
> **过程临时文件**：`C:\Users\ikrx2\Desktop\111\audit3\`（探针程序、驱动脚本、日志；**仓库零写入**，两仓 `git status --porcelain` 收尾为空）
> **本文件是第三份**：`docs/plan/` 已有 `v1.0-alignment-audit-20261024.md`（v1.0 对齐）与 `wlwl-v0.12.0-rc1-audit-and-fixes.md`（rc1 第三方报告核实）；本份是**发版前**的第三次，覆盖前两份未覆盖的三个面。

---

## 0 本文的格式契约

### 0.1 四种判定（每条事实只归一档）

| 判定 | 含义 | 发版影响 |
|---|---|---|
| **缺陷** | 实现行为与规范条款冲突，或仓内门禁/元数据自相矛盾 | **阻断** |
| **规范文本缺口** | 规范两处自相矛盾或未登记某一失败模式，实现侧的选择无规范依据 | **阻断**（需业主裁决） |
| **未登记差异** | 实现与规范的事实分叉存在，但未被登记进一致性册 §5.4 / §5.5 ⇒ 无人负责收敛 | **阻断**（登记即可） |
| **非缺陷** | 看着像问题、实测或读码确认无问题 | 不阻断（记入 §7 防重复劳动） |

### 0.2 状态标记（每条事实自带）

| 标记 | 含义 |
|---|---|
| **已核实** | 附**原始读数**或可复现命令，命令红了以命令为准 |
| **待验证** | 结论来自读码推导，**未实测** |
| **待裁决** | 需要业主或规范侧拍板 |
| **已裁决** | 业主已定，照办 |

### 0.3 本文的命令约定

- 所有复现命令在 `D:\Project\wlwl` 下运行；`wlwl.exe` 用 `impl\target\release\wlwl.exe`。
- 探针程序（`.wll`）与驱动脚本（`.ps1`）都在 `C:\Users\ikrx2\Desktop\111\audit3\probes\`；**不写进仓库**。
- `check-appendix-a` 必须**在 `impl/` 目录内**运行（它按 `../docs/wlwl-spec-reference.md` 相对路径取抄件）。

### 0.4 审计方法（三个维度 + 交叉确认）

| 维度 | 做法 | 覆盖 |
|---|---|---|
| **① 静态 REVIEW** | 规范文本 → 实现源码 → 定位坐标 → 实测复现 | 词法/语法（附录 A 全部 50 条产生式）、标准库成员面（21 命名空间 / 231 成员逐项对拍）、错误模型（§8.6 丢弃点矩阵）、并发（§17）、技能包（`SKILL.md` 871 行 + `reference.md` 2401 行 + 22 个示例） |
| **② 动态 REVIEW** | 自写**遍历最新特性的 WLWL 范例程序**跑在 v0.12.0 上，反推实现缺陷 | 17 个新成员逐条、D-14/D-15/D-16/D-17/D-18/D-19 语义、§8.6.2 的 12 格丢弃点矩阵、TCP 传输层端到端（真客户端）、诊断位置抽样 29 例 |
| **③ 交叉确认** | 每条结论由**独立通道**复现；静态读数与动态读数对撞 | 三路并行审查（词法语法 / 标准库 / 技能包）＋ 主审动态探针；不一致处逐条回查 |

⚠ **一条纪律**：结论若只来自读规范文本而未实测，标注「待验证」，不得写进「缺陷」。本报告的每条「缺陷」都带**可重跑的证伪命令**。

---

## 1 结论摘要与发版闸门判定

### 1.1 一句话结论

**核心语义面已经达标，可以发版；但「门禁面」有 4 条必须先处理的红**——其中**一条是仓内唯一的逐字对拍工具在 HEAD 上就是红的，且从未接进 CI**；另有一条**规范缺口会让「服务端每连接一个任务」的失败无声消失**。

### 1.2 发版闸门判定表

| 闸门 | 判定 | 依据 |
|---|---|---|
| 公共面 17 员齐备且语义合规 | ✅ **绿** | 21 命名空间 / 231 成员，附录 A ≡ 各册成员表 ≡ 实现，**逐命名空间集合相等、零缺零多**；17 员逐条实测 |
| D-14 `std.fs` 改 `ERR` 值 | ✅ **绿** | 12 条失败路径逐条实测，**全部 `ERR` 值、零原生诊断** |
| D-15 `TAKE_ERR` + 切段 `E0103` | ✅ **绿** | 三种作用域形状（顶层 / 普通闭包 / **切段任务体**）读数**逐字相同** |
| D-16 任务失败的报告义务 | ✅ **绿**（有 1 条规范缺口） | 孤儿诊断失败 ⇒ stderr 恰好一行、程序继续、退出码 0；被 `AWAIT` 认领 ⇒ 调用点抛出、**不重复报告** |
| D-17 `INT` 严格 / `TO_INT` 宽松 | ✅ **绿** | 7 个输入上两者结论相反，逐行兑现 §12.4 对比表 |
| D-18 传输层等待归宿主挂起 | ⚠ **黄** | 挂起语义正确（实测别的任务照跑）；但**模块顶层不可用且规范未登记**（F-03） |
| §5.4 差异登记门禁（25 行） | ✅ **绿** | 25 行逐行比对，**零错配**；`X-14` = `no-contract` ✓（D-20 要求） |
| **附录 A 逐字对拍门禁** | ❌ **红** | **`check-appendix-a` 在 HEAD 判红（3 行差异）且不在 CI**（F-01） |
| **诊断位置可定位性** | ❌ **红** | 核心内建实参校验诊断的 span **系统性地退化为 `<runtime>:0:0`**（29 例抽样 17/17 命中）（F-04） |
| **技能包可作为 agent 的唯一依据** | ❌ **红** | 3 条 P1：把规范明令禁止的共享可变状态写成「Hard rule 6」（F-11）；把已收敛的 X-06/X-07 写成已知缺口（F-12）；`std.fs` 尾部与自身前半冲突（F-13） |
| 完整测试套件 | ✅ **绿** | `cargo test --locked --workspace --all-targets --no-fail-fast` ⇒ **102 个测试目标 / 3014 条通过 / 0 失败 / 2 忽略 / exit 0** |

### 1.3 问题分布

| 严重级 | 条数 | 编号 |
|---|---|---|
| **P0 阻断** | 4 | F-01、F-02、F-03、F-04 |
| **P1 语义 / 可用性** | 9 | F-05 … F-13 |
| **P2 文档 / 文案** | 9 | F-14 … F-22 |
| **规范侧待裁决** | 4 | F-23a … F-23d |
| **正面确认** | 12 组 | §6 |

---

## 2 逐条事实

### 2.1 P0 —— 门禁与元数据

---

#### F-01 `check-appendix-a` 在 HEAD 判红（3 行差异），且**从未接进 CI**

- **判定**：**缺陷**（门禁失效 + 实现侧元数据未随 D-13 同步）
- **状态**：**已核实**（直接跑仓内二进制得到的原始输出）

**规范条款**

- 标准库·注册镜像册 **§0.7 规则 2**：「实现侧与本镜像不一致时，走一致性册 §5 的**差异登记**」；**规则 1**：「表体**不得**由实现清单自动生成，**不得**为追平实现的当次改动而改写」。
- 标准库·注册镜像册 **§0.5**：「层 | 该成员所在的实现层:R0 内建层 / R1 语言层 / R2 原生层」——「层」是**事实性登记**。
- 一致性册 **§5.4**：25 行差异登记（全部「已实现」），**无本条**。

**实现坐标**

| 位置 | 内容 | 与快照的差 |
|---|---|---|
| `impl\crates\wlwl-eval\src\stdlib_mirror.rs:201` | `layer: "R1",`（`wlwl:std.str`） | 快照是 `混合(R1 … + R2 \`CAT\`)` |
| `impl\crates\wlwl-eval\src\stdlib_mirror.rs:271` | `layer: "混合(R1 \`RESULT_MAP\`/… + R2 \`RESULT_ALL\`/\`RESULT_ANY\`)",`（`wlwl:std.result`） | 快照多一段 `+ R2 \`TAKE_ERR\`` |
| `impl\crates\wlwl-std\src\time.rs:53` | `("SLEEP_ASYNC", sleep_async as StdFn),` | 派生表体里 `SLEEP_ASYNC` 排在第 4 位 ⇒ **顺序**与快照不同 |

**实测读数**（在 `impl/` 目录下直接跑仓内二进制）

```powershell
cd D:\Project\wlwl\impl
.\target\release\check-appendix-a.exe
```

```
error: MISMATCH: the appendix A in ../docs/wlwl-spec-reference.md differs from the implementation-derived body.
  the frozen snapshot is the truth (v1.0 spec registry §0.7) — this tool does NOT rewrite anything;
  register the difference per the v1.0 conformance booklet §5.4 and converge the implementation side.
  line 8:
    copy: | `std.time` | `NOW` `MONOTONIC` `SLEEP` `TEST_BUBBLE` `ADVANCE` `TIMEOUT` `SLEEP_ASYNC` | R2 | 系统 |
    impl: | `std.time` | `NOW` `MONOTONIC` `SLEEP` `SLEEP_ASYNC` `TEST_BUBBLE` `ADVANCE` `TIMEOUT` | R2 | 系统 |
  line 16:
    copy: | `std.str` | … `CAT` | 混合(R1 `JOIN`/… + R2 `CAT`) | 核心 |
    impl: | `std.str` | … `CAT` | R1 | 核心 |
  line 23:
    copy: | `std.result` | … `TAKE_ERR` | 混合(R1 … + R2 `RESULT_ALL`/`RESULT_ANY`/`TAKE_ERR`) | 核心 |
    impl: | `std.result` | … `TAKE_ERR` | 混合(R1 … + R2 `RESULT_ALL`/`RESULT_ANY`) | 核心 |
```

⇒ **exit code 1**。

**为什么没人发现**：`impl\crates\wlwl-eval\tests\stdlib_appendix_a_sync.rs` 的 6 个用例只覆盖两件事——① 抄件 vs 冻结快照**逐字节**；② 实现成员**集合** vs 附录 A ∪ 台账。该文件**不比「层」列、不比顺序**。⇒ 总纲 §0.9 的 D-13 状态里那句「附录 A 同步守卫 6/6 绿」**为真但不充分**。

**CI 现状**（`.github\workflows\ci.yml`）：`grep -n "check-appendix\|appendix" .github\workflows\*.yml` ⇒ **零命中**。CI 只跑 `cargo fmt` / `clippy` / `cargo doc` / `cargo deny` / `cargo test` / `cargo build --release --bin wlwl` / `cargo check --features real-ai` 等。⇒ **这个只读工具从未被任何 job 调用过**。

**复现步骤**（第三方可独立重跑）

```powershell
cd D:\Project\wlwl\impl
.\target\release\check-appendix-a.exe ; Write-Output "exit=$LASTEXITCODE"   # ⇒ exit=1
Select-String -Path D:\Project\wlwl\.github\workflows\ci.yml -Pattern 'appendix'   # ⇒ 0 命中
```

**影响**

1. 仓内**唯一**做「镜像 vs 实现派生表体」**逐字**对拍的只读工具，从 D-13 落地起就是红的，而 CI 不跑它 ⇒ 这条分叉无人发现。
2. `std.result` 那条是**纯实现侧 bug**（`TAKE_ERR` 确实是 R2 内核，见 `impl\crates\wlwl-std\src\result.rs:146` `pub fn kernel_take_err`）⇒ 元数据漏登记。
3. `std.str` 那条是 F-02 的**元数据侧表现**。
4. `std.time` 那条**成员集合相等**，只是绑定表顺序不同 ⇒ 对拍工具按**行**比 ⇒ 判红。
5. ⚠ 旧读数误导：`docs\plan\v1.0-alignment-audit-20261024.md:40` 记 `check-appendix-a ⇒ ✅` —— 那是 **2026-10-24、D-13 落地之前**的读数；`CHANGELOG.md` 里那几处 `check-appendix-a ⇒ OK` 同理，都是 X-01/X-02b/X-24 批次的旧读数。

**修法**（见 §4 批次 R-1）

---

#### F-02 `std.str::CAT` 的实现层是 R1，规范两处写死 R2，且**未登记**

- **判定**：**未登记差异**（事实分叉存在，但一致性册 §5.4 无此行、§5.5 台账为空 ⇒ 按 §0.7 规则 2 **无人负责收敛**）
- **状态**：**已核实**（读码 + 实测行为）

**规范条款**

- 标准库·核心册 **§6 导语**（`:336`）：「除 `CAT` 外全部是纯 wlwl 实现(R1);**变长**拼接的 `CAT` 归 **R2 原生层**。」
- 标准库·核心册 **§6 成员表**（`:349`）：`| \`CAT(parts...) -> STRING\` | … | — | \`O(m)\` | **R2** |`
- 标准库·注册镜像册 **附录 A**（`:119`）：`| \`std.str\` | … \`CAT\` | 混合(R1 … + **R2 \`CAT\`**) | 核心 |`

**实现坐标**

- `impl\crates\wlwl-std\wl\std\str.wll:16` ⇒ `LET(_CAT, FUN((*parts), (LET MUT(s, ""); FOR(p, parts, SET(s, +(s, p))); s)));`
  —— **纯 wlwl 闭包（R1）**，不是 R2 内核。
- `impl\crates\wlwl-std\src\kernels.rs` ⇒ **无 `CAT` kernel**（`grep -rn "\bCAT\b" impl\crates` 只命中 `str.wll` 与测试）。
- `impl\crates\wlwl-eval\src\stdlib_mirror.rs:201` ⇒ `layer: "R1"`（与实现相符，与快照不符）。

**实测读数**（行为层**完全合规**，与层归属无关）

```
cat1 zero:                       # CAT() ⇒ ""（§6「零段 ⇒ ""」✓）
cat2 one: a                      # CAT("a") ⇒ "a" ✓
cat3 many: abc                   # CAT("a","b","c") ⇒ "abc" ✓
cat4 render: x1TRUENULL[1, 2][a: 1]   # 非字符串段按 STR 渲染 ✓（§6:361）
cat5 propagates: TRUE            # CAT("a", ERR("boom")) ⇒ ERR 透传，**未被渲染** ✓（§6:363）
cat6 payload: boom               # 载荷仍是 "boom"
```

**影响**

1. 可观察行为**零偏差** ⇒ 不是行为缺陷。
2. 但冻结快照的「层」列此刻**陈述了一件假事**（说 `CAT` 在 R2，实现是 R1）⇒ 规范文本层面的事实错误。
3. 该分叉**未**登记进一致性册 §5.4（25 行全「已实现」，无相关行），**§5.5 台账为空**（`wlwl-v1.0-conformance.md:530`：「(本版无条目)」）⇒ 按 §0.7 规则 2，**没有人在负责收敛**。
4. 收敛方向**需要裁决**（两条路都动东西）：
   - **甲案**：把 `CAT` 改写成 R2 内核（`impl\crates\wlwl-std\src\kernels.rs` 加 kernel）⇒ 实现对齐规范，冻结快照不动；
   - **乙案**：按一致性册 §3.4 **例外 2（规范缺陷修复）** 修规范文本（核心册 §6 两处 + 附录 A 一处）⇒ **要动冻结快照**，且需走一致性册 §5 的登记流程。

⚠ **本报告的建议**：走**甲案**。理由：`CAT` 的语义已经是「变长 + 按 `STR` 渲染 + `ERR` 不渲染」——这三条在 R1 闭包里是自然写成的，但要正确实现「`ERR` 实参不渲染」需要**在 `FOR` 之前判一次实参**；R2 内核能一次性把「渲染 / 不渲染」两条规则写死。乙案要改冻结快照，而 §0.7 规则 1 明写快照**不得**为追平实现的当次改动而改写 ⇒ 乙案的成本更高、风险更大。

---

#### F-03 宿主挂起点成员在**模块顶层**报 `E0014`，规范未登记这一失败模式；诊断消息指名 `YIELD` 且泄露 Rust 内部实现

- **判定**：**规范文本缺口**（+ 诊断措辞缺陷）
- **状态**：**已核实**

**规范条款**

- 标准库·系统册 **§21.6.2**（`:811-816`）：「**『暂时没有』是正常值,不是错误**」；表内「`TCP_ACCEPT` 在超时内**没有**待接的连接 | `NULL`」—— **无条件表述**，未提任务体前置条件。
- 标准库·系统册 **§15.1**（`:315`）：`SLEEP_ASYNC(ms) -> NULL` 的「失败」列只有「元数错 `E0022`;非整数 `E0030`;`ms < 0` `E0030`」——**没有**「任务体外 ⇒ `E0014`」。
- 语言核心册 **§11.2**：`E0014` 的触发条件只列「`RETURN`/`BREAK`/`CONTINUE` 出现在函数或循环体之外;或 `YIELD` 出现在调度驱动的上下文(任务体)之外」——**不含**宿主挂起。
- 并发册 **§17.0** 挂起点表：**只**对 `YIELD` 规定了「任务体之外 ⇒ `E0014`」（`:227`）。

**实现坐标**

- `impl\crates\wlwl-eval\src\lib.rs:8715-8717` ⇒
  `Signal::Yield(_) => Err(self.diag(ErrorCode::E0014, "YIELD used outside a step context (yield is only valid inside a Scheduler::step loop, not at a top-level module load)".to_string(), …))`
  —— 宿主挂起在模块顶层走的是**与 `YIELD` 同一条**信号路径，于是消息原样沿用。
- 同族：`:8428-8430` 是模块顶层 eval 路径。

**实测读数**

```powershell
# 顶层 TCP_ACCEPT，带 2000ms 超时
cd C:\Users\ikrx2\Desktop\111\audit3\probes
& 'D:\Project\wlwl\impl\target\release\wlwl.exe' run p04a1_toplevel_accept.wll
```

```
listening
error[E0014]: YIELD used outside a step context (yield is only valid inside a Scheduler::step loop, not at a
 top-level eval call)
  --> p04a1_toplevel_accept.wll:3:1
exit=1     ELAPSED_MS=101
```

⚠ **两个要点**：

1. **它根本没等**：声明的超时是 **2000 ms**，实测 **101 ms** 就失败了 ⇒ §21.6.2 的「超时 ⇒ `NULL`」在模块顶层**不可达**。
2. **span 指错位置**：`TCP_ACCEPT` 在第 **5** 行，诊断指向第 **3** 行的 `IMPORT`（另见 F-05）。

同一形态的第二个成员（`SLEEP_ASYNC` 顶层）：

```
marker
error[E0014]: YIELD used outside a step context (…)
  --> p04a3_sleep_async_toplevel.wll:2:1
```

**影响**

1. §21.6.2 的「超时 ⇒ `NULL`」契约在模块顶层不可达 ⇒ **单任务程序写不了顶层轮询式服务器**（这是传输层族最自然的用法之一）。
2. **同族成员的顶层可用性不一致且规范没写**：`SLEEP` 明确「顶层如此」（§15.5），`SLEEP_ASYNC` 顶层却直接报错。
3. **诊断消息误导**：用户写的是 `SLEEP_ASYNC` / `TCP_ACCEPT`，消息却指名 `YIELD`，并把 `Scheduler::step loop` 这个 Rust 内部实现细节暴露给使用者。

**修法**（§4 批次 R-2）

---

#### F-04 核心内建的**实参校验诊断**系统性地丢失源位置（`<runtime>:0:0`）

- **判定**：**缺陷**（诊断结构不合 §11.1）
- **状态**：**已核实**（29 例抽样，读数见下）

**规范条款**

语言核心册 **§11.1 结构**：

> 「诊断是错误(`E`)或警告(`W`),带四位数字码。**错误形式的 CLI 输出包含码、消息、位置(文件、行列)与源行摘录**;`--format json` / `jsonl` 产生机器可读结构(含 `error_schema_version`、`error_category`、`retryable`、`idempotent`、`suggestion_code`、`trace` 等字段)。」

**实测读数**（`run_diag_spans.ps1` / `run_diag_spans2.ps1`，两轮共 29 例）

**丢位置的一类（核心内建的实参校验 —— 17/17 命中）**

| 源码 | 诊断码 | span |
|---|---|---|
| `LEN(1)` | E0030 | `<runtime>:0:0` |
| `+(1, "a")` | E0030 | `<runtime>:0:0` |
| `INDEX_GET([1], 5)` | E0036 | `<runtime>:0:0` |
| `INDEX_GET("a", 9)` | E0036 | `<runtime>:0:0` |
| `+(1, 2, 3)` | E0022 | `<runtime>:0:0` |
| `LEN()` | E0022 | `<runtime>:0:0` |
| `-(1)` | E0022 | `<runtime>:0:0` |
| `*(1, 2, 3)` | E0022 | `<runtime>:0:0` |
| `STR()` / `TYPE()` / `BOOL(1,2)` | E0022 | `<runtime>:0:0` |
| `UPPER(1)` / `LOWER(1)` | E0030 | `<runtime>:0:0` |
| `AT_K([1],"k",0)` / `AT_K(1,"k",0)` | E0030 | `<runtime>:0:0` |
| `INDEX_SET("a",0,1)` | E0030 | `<runtime>:0:0` |
| `NEW(1,2)` | E0030 | `<runtime>:0:0` |
| `CONCAT(1,2)` | E0030 | `<runtime>:0:0` |
| `CONTAINS([1],1,2)` | E0022 | `<runtime>:0:0` |
| `/(1,0)` / `%(1,0)` | E1003 | `<runtime>:0:0` |

**不丢位置的一类（求值器层的分派期检查 —— 全部带真位置）**

| 源码 | 诊断码 | span |
|---|---|---|
| `UNDEFINED_NAME` | E0020 | `…/diag_06.wll:2:1` ✓ |
| `CALL(1, 2)` | E0020 | `…/diag_09.wll:2:1` ✓ |
| `CHANNEL_RECV(1)` | E0053 | `…/diag_11.wll:2:1` ✓ |
| `SCOPE(FUN(() , YIELD()))` | E0014 | `…/diag_12.wll:2:16` ✓ |
| `SPAWN(1)` | E0058 | `…/diag_13.wll:2:1` ✓ |
| `RETURN(1)` | E0014 | `…/diag_120.wll:2:1` ✓ |

**对照组（stdlib 成员层 —— 也带位置）**

```
error[E0030]: UTF8_DECODE: byte out of range: 300
  --> p05b2_bytes_err.wll:5:7        # ← stdlib 成员的实参校验有位置
```

⇒ **分界线很清楚**：诊断在**求值器的分派期**产生 ⇒ 有位置；诊断在**核心内建 / 成员体内部**产生 ⇒ 丢位置。

**JSON 形态的实测**（`--format json`）

```json
"location": { "file": "<runtime>", "line": 0, "col_start": 0, "line_end": 0, "col_end": 0 },
"source_line": null,
"trace": [ { "frame": "<toplevel>", "location": { "file": "<runtime>", "line": 0, … } } ]
```

⇒ §11.1 要求的「位置(文件、行列)」与「源行摘录」**双双缺失**（`source_line` 为 `null`）。

**影响**

1. 违 §11.1 的明文要求 ⇒ **所有**核心内建的实参校验错误对使用者**不可定位**。
2. 这类错误恰是**新手最高频**的一类（类型错、元数错、越界）⇒ 报错等于没报。
3. 对 agent 更糟：agent 拿到的诊断没有位置 ⇒ 只能靠猜。

**修法**（§4 批次 R-3）

---

### 2.2 P1 —— 诊断可定位性与语义

---

#### F-05 顶层宿主挂起的 `E0014` 的 span 指向**模块第一条语句**，不是挂起调用点

- **判定**：**缺陷**（诊断位置错误）
- **状态**：**已核实**

**实测读数**

```wlwl
// p04a2_span.wll —— 挂起调用在**第 10 行**
 1  // P4a-2 — E0014 的 span 指向：调用点还是别的？
 2  IMPORT("wlwl:std.net", ["TCP_LISTEN", "TCP_ACCEPT", "TCP_LOCAL_ADDR"]);
 3  LET(a, 1);
 4  LET(b, 2);
 5  LET(c, 3);
 6  LET(listener, TCP_LISTEN("127.0.0.1", 0));
 7  LET(addr, TCP_LOCAL_ADDR(listener));
 8  PRINT("marker-line-before-call");
 9  LET(nope, TCP_ACCEPT(listener, 100));
10  PRINT("unreachable");
```

```
marker-line-before-call
error[E0014]: YIELD used outside a step context (…)
  --> p04a2_span.wll:2:1          ← 第 2 行 = IMPORT；调用点在第 9 行
```

⇒ span = **模块的第一条语句**（`IMPORT` 所在行），与挂起调用点无关。

**影响**：与 F-04 同族但成因不同 —— F-04 是「span 从未建立」，本条是「span 建错了地方」。使用者按提示跳到 `IMPORT` 行，完全找不到问题。

**修法**（§4 批次 R-2，与 F-03 合并处理）

---

#### F-06 `IS_OK` / `IS_ERR` / `OR_DIE` 被实现成**关键字** ⇒ §3.5 的「遮蔽内建」规则对它们永久失效

- **判定**：**缺陷**
- **状态**：**已核实**（主审复现 + 词法审查独立定位）

**规范条款**

- 语言核心册 **§1.4** 关键字表只列 **15** 个（`TRUE FALSE NULL LET FUN RETURN IF WHILE FOR BREAK CONTINUE CLASS NEW THIS NOT`），并明写「关键字**不得**用作标识符」。
- 语言核心册 **§3.5**：「`LET` 或命名函数定义(§5.1)遮蔽第 10 章的内建名时:默认产生 **`E0025`**;当模块清单声明 `[features] allow_builtin_shadow = true` 时允许并产生警告 **`W0030`**」。
- 附录 G：`IS_OK` / `IS_ERR` / `OR_DIE` 是**全局内建**（`OR_DIE` 标注为「弃用别名 (`W0051`)」），在附录 A.2 的任何产生式里都**不出现**。

**实现坐标**

`impl\crates\wlwl-lexer\src\lib.rs:633-635`

```rust
"IS_OK" => TokenKind::IsOk,
"IS_ERR" => TokenKind::IsErr,
"OR_DIE" => TokenKind::OrDie,
```

这三个 token 在 parser 里都不是 `Ident` 分支（`impl\crates\wlwl-parser\src\lib.rs:882-888` 的 `parse_expr`、`:1546-1559` 的形参名只收 `Ident` / `Mut`）。

**实测读数**

```
x01: LET(IS_OK, 1);
  ⇒ error[E0010]: expected identifier in LET, got IsOk        (x01_isok_keyword.wll:2:5)

对照: LET(LEN, 1);
  ⇒ error[E0025]: cannot shadow built-in 'LEN'

FUN((IS_OK), 1);   ⇒ error[E0010]: expected parameter name, got IsOk
对照 FUN((LEN), 1); ⇒ 解析通过（仅 W0011 未用形参）

[features] allow_builtin_shadow = true 之下:
  LET(LEN, 1);     ⇒ 运行成功 + warning[W0030]: shadowing global builtin 'LEN'
  LET(IS_OK, 1);   ⇒ 仍 error[E0010]（许可开关对它无效）
```

**影响**

1. 用户拿 `IS_OK` 当变量名得到的是**语法错 `E0010`**而不是规范 §3.5 规定的「遮蔽内建 `E0025`」⇒ 按码分流的工具（IDE / CI 门禁 / agent 自愈规则）会走错分支。
2. `allow_builtin_shadow = true` 这条**唯一的合法遮蔽通道**对这三个名字**永久失效**。
3. `OR_DIE` 是 `UNWRAP_OR` 的**弃用别名**；把别名做成关键字，使 §11.3 的 `W0051`（弃用别名警告）只能靠 `parse_or_die` 特判维持，与「它只是个内建名」的规范定位不一致。

**修法**（§4 批次 R-4）

---

#### F-07 裸 `THIS`（零参引用）**未实现** —— 报 `E0020 undefined name` 且附带误导性提示

- **判定**：**缺陷**
- **状态**：**已核实**（主审复现 + 对象册审查独立定位）

**规范条款**

- 语言核心册 **§4.2**：「`CLASS`、`NEW`、`THIS` 以关键字身份出现在调用位置(§1.4),文法上按具名调用解析;**`THIS` 也可单独作为零参引用出现**(对象册 §15.1)。」
- 对象册 **§15.1**：「`THIS()`(**或零参引用 `THIS`**)求值为当前活跃的 `INSTANCE` 接收者。」且「当前没有活跃方法接收者(方法体之外)→ **`E0032`**。」

**实现坐标**

- `impl\crates\wlwl-parser\src\lib.rs:2049` ⇒ `TokenKind::This => "THIS".to_string(),`
- `impl\crates\wlwl-parser\src\lib.rs:2171-2183` ⇒ 在没有 `(` 时把它降级成 `Expr::Var("THIS")`（普通变量引用）⇒ 运行期按名字查找 ⇒ `E0020`。
- 佐证这是**遗漏**而非取舍：`impl\crates\wlwl-eval\src\lib.rs:75-79` 的逃逸检查明确识别 `E::Var(name) if name == "THIS"` ⇒ 实现侧认为 `Var("THIS")` 是有语义的形态，只是运行期没有对应解析。

**实测读数**

```wlwl
LET(C, CLASS("C", NULL, [["get", FUN((self), THIS)]]));
LET(o, NEW(C));
PRINT(CALL_METHOD(o, "get"));
```

```
error[E0020]: undefined name `THIS`
  --> x02_this_bare.wll:2:46
  = hint: did you mean one of: HAS, TRIM?
```

对照：`THIS()` 在方法体内 ⇒ `5`（正常）；`THIS()` 在方法体外 ⇒ `error[E0032]: THIS used outside a method body (no active receiver)`（正常）。

⇒ **两处错**：① 零参形态未实现；② 规范要求方法体外报 `E0032`，实现报 `E0020`。

**影响**

1. 对象册 §15.1 明写两种写法都合法；实现只支持 `THIS()`。
2. 错误信息把它引向「**名字拼错**」（hint 给的是 `HAS` / `TRIM`），而不是「零参形式未实现」⇒ agent 会改名字而不是改写法。
3. 规范要求的 `E0032` 在裸 `THIS` 形态下**永远发不出**。

**修法**（§4 批次 R-4）

---

#### F-08 嵌套块内省略必需的 `;` 报 `E0011`，规范要求 `E0013`

- **判定**：**缺陷**
- **状态**：**已核实**

**规范条款**

语言核心册 **§1.10**：

> 「处于『语句位置』的表达式**必须**以 `;` 结束。在括号序列(§4.4)内部,最后一个表达式的 `;` **可以**省略;该表达式的值即括号序列的值。**省略必需的 `;` 产生 `E0013`**。」

**实现坐标**

`impl\crates\wlwl-parser\src\lib.rs:709-722`

```rust
} else {
    if !in_paren {                 // ← 只有模块顶层才走 E0013
        if self.at_eof() { break; }
        return Err(self.err_at(EC::E0013, "expected ';' after expression", …));
    }
    break;                        // ← 嵌套块：跳出循环，由调用方 expect(')') ⇒ E0011
}
```

调用方（`parse_paren_block` / `parse_fun` / `parse_if` / `parse_while` / `parse_for`，分别在 `lib.rs:2632 / 1605 / 1392+ / 1418 / 1455`）统一 `expect_specific(EC::E0011, "')'")`。

**实测读数**

| 源码 | 实测码 | 规范要求 |
|---|---|---|
| `PRINT(1)`↵`PRINT(2);`（模块顶层） | **E0013** | E0013 ✓ |
| `PRINT((PRINT(1) PRINT(2)));` | **E0011** | E0013 ✘ |
| `FUN((x), (PRINT(1) PRINT(2)));` | **E0011** | E0013 ✘ |
| `IF(TRUE, (PRINT(1) PRINT(2)));` | **E0011** | E0013 ✘ |
| `WHILE(FALSE, (PRINT(1) PRINT(2)));` | **E0011** | E0013 ✘ |
| `FOR(x, [], (PRINT(1) PRINT(2)));` | **E0011** | E0013 ✘ |

**影响**

同一个错误（漏写 `;`）在顶层与嵌套块里给出**不同诊断码**。按码分流的工具在嵌套块里拿不到 `E0013`，无法按「缺分号」做定向修复，只能回退到「期望 `)`」这种**误导定位**的文案。

**修法**（§4 批次 R-5）

---

#### F-09 嵌套 `EXPORT` 被**静默吞掉**（声明了导出却不导出，且零诊断）

- **判定**：**缺陷**（+ 规范文本缺口）
- **状态**：**已核实**

**规范条款**

- 附录 A.2：「**`ModuleDecl`(§9.1 / §9.2)只出现在模块顶层,不作为 `Expression`。**」
- 语言核心册 §9.1：「模块顶层**可以**有 `EXPORT(name_array)`;未导出的绑定对外不可见。」
- ⚠ 规范自身矛盾：附录 A.2 的 `Block = "(" { ExprStmt | ModuleDecl } [ Expression ] ")"` **字面上允许** `ModuleDecl` 出现在嵌套块里（见 F-23）。

**实现坐标**

- `impl\crates\wlwl-parser\src\lib.rs:892-893` ⇒ `TokenKind::Import => self.parse_import(),` / `TokenKind::Export => self.parse_export(),` —— **不做位置判断**。
- `impl\crates\wlwl-parser\src\lib.rs:2127-2141` ⇒ `SEALED` **有**专门的模块顶层闸门（`self.module_top && name == "SEALED" && …`）。
⇒ 三个 `ModuleDecl` 的处理彼此不一致：**`SEALED` 报错（可诊断）、`IMPORT` 被接受并真的执行、`EXPORT` 被接受但静默无效**。

**实测读数**

```wlwl
// x04_nested_export.wll
LET(b, 111);
LET(x, (EXPORT(["b"])));
```

```
⇒ run 退出码 0，零诊断
⇒ wlwl interface x04_nested_export.wll:
   { "exports": [], … "sealed": null }
⇒ 消费方 IMPORT("./x04_nested_export.wll", ["b"]) ⇒ error[E0023]: 'b' is not exported by module '…'
对照（EXPORT 移到顶层）⇒ 一切正常
```

**影响**

作者写了 `EXPORT(["b"])`、程序**不报错**、模块对外**什么都没导出**，跨模块导入时才炸 `E0023` —— 而错误信息指向的是**消费方**。这是典型的「失败不可见」陷阱，与本语言 §8「失败可见」的原则相悖。

**修法**（§4 批次 R-5）

---

#### F-10 孤儿任务的「`ERR` **终值**」被**静默丢弃**（无人 `AWAIT` 时）

- **判定**：**规范文本缺口**（§17.9 把该情形指向 §8.6 的丢弃点规则，但 §8.6.2 的三处丢弃点**都不覆盖**「任务终值」⇒ 实现侧的选择没有规范依据，且实测是**静默**）
- **状态**：**已核实**

**规范条款**

- 并发册 **§17.9 末段**： 「**用户 `ERR` 值不是失败。** 任务以用户 `ERR` 值结束时,那是**结果**而不是失败:被 `AWAIT` 时按 §17.1 **返回该值**(不自动消费);**无人 `AWAIT` 时按语言核心册 §8.6 的丢弃点规则处置** ⇒ 本节只管**宿主诊断**型失败。」
- 语言核心册 **§8.6.2** 的丢弃点**恰好三处**：① 语句值丢弃;② 绑定被覆盖;③ **作用域结束时仍有绑定**持有未消费 `ERR`。

⇒ 「任务终值」**不是绑定**、**不是语句值** ⇒ 三处丢弃点**都不覆盖它** ⇒ 按字面读，§17.9 的指向落空。

**实测读数**（`p03f_orphan_err_terminal.wll`）

```wlwl
SCOPE(FUN(() ,
  LET(t, SPAWN(FUN(() , ERR("orphan-terminal-err"))));
  0
));
PRINT("P3F-DONE");
```

```
stdout: P3F-DONE
stderr: （空）
exit=0
```

⇒ **那个 `ERR` 完全消失**：无 `E0103`、无孤儿报告行、退出码 0。

**对照组 1 —— 孤儿任务里的「绑定」持有 `ERR`**（`p03g_orphan_bound_err.wll`）

```wlwl
SCOPE(FUN(() ,
  LET(t, SPAWN(FUN(() ,
    LET(x, ERR("orphan-bound-err"));
    0
  )));
  0
));
```

```
stderr: [wlwl] unhandled task failure (task #0): E0103 unhandled ERR was discarded: orphan-bound-err
exit=0
```

⇒ **绑定**形态会被 §8.6.2 第 3 项抓住 ⇒ 走孤儿报告行（F-06 的 §17.9 报告路径正常工作）。

**对照组 2 —— 被 `AWAIT` 的 `ERR` 终值**（`p03b_awaited_failure.wll`）

```
stderr: error[E0103]: unhandled ERR was discarded: awaited-boom
  --> p03b_awaited_failure.wll:4:10
exit=1
```

**影响**

1. §17.9 明写「无人 `AWAIT` 的失败**不得**完全静默:症状会与『网络卡了』**无法区分**」—— 这条动机**同样适用于 `ERR` 终值**：服务端「每连接一个任务」模式下，一个以 `ERR` 结束的连接任务**不留任何痕迹**。
2. 同一段代码，三种归属（绑定 / 终值 / 被 `AWAIT`）走出**三种**结果（报 E0103 / 静默 / 报 E0103）⇒ 行为面不成体系。

**修法**（§4 批次 R-6 —— **需业主裁决**：规范侧补第四条丢弃点，还是实现侧在孤儿任务的 `ERR` 终值上补报）

---

#### F-11 技能包：并发「Hard rule 6」把规范**明令禁止**的共享可变状态写成推荐做法

- **判定**：**缺陷（P1）**
- **状态**：**已核实**（技能包审查独立定位；主审复核了行号与自相矛盾）

**技能包坐标**

`wlwl-skill\SKILL.md:510`

> `6. **Shared state = `LET MUT`.** Prefer `LET MUT` for cells written from children. `SET` on a `LET` binding is `E0024` (single-task and cross-task alike).`

**规范依据**

并发册 **§17.0.1**（规范性，`:143`）：

> 「**规范约束**:任务之间**不得**通过**共享的可变 cell**(即 `LET MUT` 绑定的、被 `SCOPE` 内多个任务读写的 cell)来传递数据。正确形态是 **actor**:每个任务独占状态、只经**通道**(`CHANNEL_*`)传消息」

同册 `:145`：「违反本条的**不**构成符合本规范的程序(一致性册 §1.1)」。

**技能包内部自相矛盾**

| 位置 | 说法 |
|---|---|
| `wlwl-skill\SKILL.md:55`（writing flow 第 3 步） | 「⚠ **Never share mutable state between tasks** (spec §17.0.1)」 |
| `wlwl-skill\reference.md:1801` | 「⚠ **Do not use `LET MUT` to share mutable state between tasks.**」 |
| `wlwl-skill\SKILL.md:510`（**唯一以「Hard rules」标题列出的位置**） | 「**Shared state = `LET MUT`.** Prefer …」 |

**影响**

agent 读「Hard rules」（并发部分权威度最高的位置）会按 `LET MUT` 共享 cell 写并发程序 —— 这正是规范判为**不符合规范的程序**的形态；且违反它的程序**不被任何诊断拦截**（规范明说这不是运行期错误）⇒ 静默错误。

**修法**（§4 批次 R-7）

---

#### F-12 技能包：把**已收敛**的 X-06 / X-07 写成「已知缺口」

- **判定**：**缺陷（P1）**
- **状态**：**已核实**

**技能包坐标**

`wlwl-skill\SKILL.md:739-743`（`reference.md:243-246` 是同一句的第二份拷贝）：

> 「⚠ Two known gaps are registered in the spec's conformance volume (X-06 / X-07): a binding holds only **one** unconsumed token, so two unconsumed `ERR`s in one container lose one; and a standard-library callback's argument channel carries no token, so an unconsumed `ERR` inside a callback is not reported.」

**规范依据**

一致性册 **§5.4**：`X-06` 与 `X-07` 两行的状态都是「**已实现**」（X-06：「实测读数 **2 条** `E0103`」；X-07：「实测读数 `E0103: unhandled ERR was discarded: boom`」）。

**实测读数**

```
X-06: LET(a, [ERR("one"), ERR("two")]); PRINT(a); 0
  ⇒ 两条 E0103（不是一条）；exit=1
X-07: MAP([1,2], FUN((x), ERR("boom")))
  ⇒ error[E0103]: unhandled ERR was discarded: boom；exit=1
```

**影响**

agent 被告知「这两种情形会静默丢错」，于是（a）不写 `LET(_, …)` 也不取载荷，程序实际会以 `E0103` + 退出码 1 终止；（b）在回调里忽略错误时以为安全，实则会整程序失败。**最容易让人误判「是不是编译器坏了」的一类描述。**

**修法**（§4 批次 R-7）

---

#### F-13 技能包：`std.fs` 段落的**尾部**仍说三个冻结成员用 `E0060`–`E0062`（与同一句前半、规范、实测三处冲突）

- **判定**：**缺陷（P1）**
- **状态**：**已核实**

**技能包坐标**

`wlwl-skill\SKILL.md:761`（该行超长）—— **同一行**前半已写对：

> 「⚠ **v0.12.0 changes the failure semantics of `std.fs`**: the IO failures of the frozen `READ_FILE` / `WRITE_FILE` are now **`ERR([kind:"FsError", op, reason])` values**, not the native `E0060`–`E0062` diagnostics」

**同一行的尾部**却写：

> 「All four fail with an **`ERR` value** (…), not a diagnostic — only the three frozen v0.3 members still use `E0060`–`E0062`.」

**规范依据**

语言核心册 **§11.2**：`E0061`（`std.fs` 读失败）与 `E0062`（`std.fs` 写失败）自 v1.0.2 起**退役**；标准库·系统册 §2.3.1 同款。

**实测读数**

```
READ_FILE("…\__no_such_file__.txt")   ⇒ IS_ERR=TRUE / kind=FsError / op=READ_FILE；进程**不**中止
WRITE_FILE("…\__no_such_dir__\x.txt")  ⇒ IS_ERR=TRUE / kind=FsError / op=WRITE_FILE
LIST_DIR("…\__no_such_dir__")          ⇒ IS_ERR=TRUE / kind=FsError
MKDIR(<已存在>)                         ⇒ IS_ERR=TRUE / kind=FsError
⇒ 全程 **无 E0061 / E0062**，退出码 0
```

**影响**

agent 读到尾部会为 `READ_FILE` / `WRITE_FILE` 写「捕获 `E0061`/`E0062`」的分支 —— 这两个码**永不触发**，分支是死代码；真正的失败以 `ERR` 值返回，需要 `IS_ERR` / `ERR_PAYLOAD` 判定。

**修法**（§4 批次 R-7）

---

### 2.3 P2 —— 文档、诊断文案与规范侧

---

#### F-14 诊断建议文案承诺了**不存在**的语言特性

- **判定**：**缺陷**（假修复指引）
- **状态**：**已核实**

`impl\crates\wlwl-lexer\src\lib.rs:302-306`（`E0002` 的建议）：

> `"add a closing \`\"\` before end of line, or split into two adjacent strings (WLWL concatenates them at parse time)"`

⚠ **WLWL 没有相邻字符串拼接**：附录 A.2 的 `Primary` 无「串接」分支，`string_lit` 也不可组合。

```
PRINT("a" "b");   ⇒ error[E0011]: expected ')', got StringLit("b")
```

⇒ 建议把用户从「补一个引号」引向「拆成两个字符串」，而那条路**根本走不通**。

`impl\crates\wlwl-lexer\src\lib.rs:294-301`（`E0001` 的建议）：

> `"valid identifier characters: a-z, A-Z, 0-9, _ ; valid string escapes: \`\" \\ / \u{8} \u{c} \n \r \t \0\` ; (numbers must be ASCII digits, optionally with one '.')"`

三处失真（实测对照）：

| 建议文本说 | 实测 |
|---|---|
| 转义只有 9 个（**漏 `\$`**） | `PRINT("a\$b");` ⇒ 正常输出 `a$b` ⇒ `\$` **合法** |
| 数字「ASCII digits, optionally with one `.`」 | `1e2`⇒`100.0`；`1.5e-2`⇒`0.015`；`0x1F`⇒`31` ⇒ **指数与十六进制都合法** |
| 标识符「a-z, A-Z, 0-9, _」 | `LET(变量, 1);` ⇒ 合法；`LET(a。b, 1);` ⇒ **也合法**（见 F-15） |

**影响**：`E0001` 是词法层最高频的诊断，这条文案事实上是「词法面说明书」，与实现脱节说明它已长期未随实现更新。诊断建议是 agent 与人**最常直接照做**的文本。

---

#### F-15 标识符接受**任意**非 ASCII 码点（不止 Unicode 字母类别）

- **判定**：**缺陷**（宽松方向）
- **状态**：**已核实**

**规范条款**

语言核心册 **§1.3**：

```
identifier = letter { letter | decimal_digit | "_" } .
```

「`letter` 含 Unicode 字母类别(含中文)。」

**实现坐标**

`impl\crates\wlwl-lexer\src\lib.rs:575-609`（`read_ident_or_keyword`）—— 只判断「是不是多字节序列」，**不判断码点的 Unicode 类别**。

**实测读数**

```
LET(a。b, 1); PRINT(a。b);   # 。 = U+3002，类别 Po ⇒ 成功，输出 1
LET(a€b, 1); PRINT(a€b);     # € = U+20AC，类别 Sc ⇒ 成功，输出 1
LET(a😀b, 1); PRINT(a😀b);   # 😀 = U+1F600，类别 So ⇒ 成功，输出 1
```

**影响**

① 按 v1.0.2 写的第三方实现会**拒绝**这些程序，而本实现接受 ⇒ 同一份源码在两套实现上行为不同；② `a。b` 在词法上与 `a` `。` `b` 不可区分 ⇒ 给格式化器 / LSP / 代码分割器带来边界歧义。

---

#### F-16 实现侧的词法扩展（`0x1F` 十六进制、`b"…"` 字节串）不在 v1.0.2 词法文法内

- **判定**：**待裁决**（规范侧登记与否）
- **状态**：**已核实**

**规范条款**：§1.6 的 `int_lit = decimal_digit { decimal_digit } .`（**无**十六进制形式）；§1.7 / §1.8 同款；§12「本规范的保留集合为空」。

**实现坐标**（源码注释**自称**「[项目扩展]」）

- `impl\crates\wlwl-lexer\src\lib.rs:329-367`（`0x` / `0X` 分支）
- `impl\crates\wlwl-lexer\src\lib.rs:469-562`（`read_bytes`，`b"…"` 字面量）

**实测读数**

```
PRINT(0x1F);   ⇒ 31
PRINT(b"AB");  ⇒ [65, 66]      # 脱糖成 ARRAY[INTEGER]，未引入新类型
PRINT(b"\x41") ⇒ [65]
```

**影响**：不引入新类型、不产生保留名 ⇒ 对 §12 不构成违规；风险只在——按 v1.0.2 实现或做一致性审计的第三方会**拒绝**这些写法（`0x1F` 会被切成 `0` + 标识符 `x1F`）⇒ 跨实现移植时会炸。建议规范侧裁一次：要么在 §1.6/§1.8 登记，要么声明「本实现的词法面是 v1.0.2 的**超集**」。

---

#### F-17 技能包：`reference.md` §28.3 第 3 条仍写着 §28.6 已明文推翻的旧结论

- **判定**：**缺陷（P2）**
- **状态**：**已核实**

**技能包坐标**

`wlwl-skill\reference.md:2366-2367`：

> `- ⚠ **\`SCOPE\` 不等孙任务**(并发册 §17.3 第 6 条):作用域退出时**仍非终态**的子任务被**强制取消** ⇒ 客户端只看到 RST ⇒ 服务端要**自己数完成数**。`

**对撞面**

`wlwl-skill\SKILL.md:630-638`（规则 7）：

> `7. **\`SCOPE\` waits first, then force-cancels what is left.** … ⇒ A grandchild parked on a socket normally **finishes** ⇒ the client gets the **complete response**, not a reset.`

而 `reference.md:2392`（§28.6 的 A4 订正行）**自己指出** §28.3 那条是错的。

**实测读数**（skill 审查做的 socket 实测）：子任务 `TCP_ACCEPT` + `TCP_RECV` 停在 socket 上时，`SCOPE` 在**同一毫秒**（415 ms，数据到达时刻）返回，客户端收到**完整响应**（62 字节，`Content-Length: 23`），**不是 RST**。

**影响**：§28.3 是「最容易误诊的三条」之一 ⇒ agent 会认定「客户端只会看到 reset」，从而为不存在的问题加完成数计数 / 提前放弃逻辑。

---

#### F-18 技能包：孤儿任务失败报告**正文零覆盖**

- **判定**：**缺陷（P2 · 覆盖缺口）**
- **状态**：**已核实**

`wlwl-skill\SKILL.md` 正文（871 行）里 `grep -i "orphan|unhandled task failure|task failure"` ⇒ **零命中**。唯一提及在 frontmatter 的一长句里（`SKILL.md:3`），没有格式、没有「程序继续 / 退出码 0」的信息。实质说明在 `reference.md:2369-2377`（§28.4）。

**影响**：技能包的设计是 `SKILL.md` 常驻、`reference.md` 按需加载 ⇒ 只读 `SKILL.md` 的 agent 对「无人 `AWAIT` 的任务失败会怎样」只有一句话 ⇒ 会误以为子任务失败会拖垮整个程序，或把 stderr 那一行当噪声忽略。

---

#### F-19 技能包：「五处语义变更」有**三套互不相同**的口径，且漏掉 D-16 / D-17 / D-19

- **判定**：**缺陷（P2）**
- **状态**：**已核实**

| 位置 | 口径 |
|---|---|
| `SKILL.md:16-17` | 「**five semantic changes** listed in `reference.md` §28」 |
| `reference.md:2327-2348`（§28.2「五处语义变更」） | D-18 / D-14 / D-15 / D-19 / D-20 |
| `SKILL.md:3`（frontmatter） | D-18 / D-14 / D-17 / D-16-孤儿报告 |

总纲 §0.9 实有 **8** 条决策（D-13…D-20），除 D-13（成员扩充）外 **7 条**是语义变更。

**影响**：**D-19**（气泡内 `SLEEP_ASYNC` ≡ `SLEEP`，**breaking**）只在 `SKILL.md:762` 的 `std.time` 长段里出现、**不在**任何清单里；D-17（严格 `INT`）同理。⇒ agent 按「五处」清单读，会漏掉一个会改变已有程序行为的破坏性变更。

---

#### F-20 技能包：成员面数字 `214` 过期（应 **231**），且把「冻结快照」说成「generated」

- **判定**：**缺陷（P2）**
- **状态**：**已核实**

**技能包坐标**

- `wlwl-skill\SKILL.md:865`：「Its **Appendix A** is the *generated* member registry — **214 members across 21 namespaces** …」
- `wlwl-skill\reference.md:88`：「**Member surface unchanged**: 21 namespaces / 214 members, zero added and zero removed … 它的 §5 registers the spec-vs-implementation differences **X-01…X-15**」

**实测读数**

- `wlwl.exe namespace` 的 `namespaces[].member_count` 求和 ⇒ **231**（21 命名空间）。
- 合并抄件 `docs\wlwl-spec-reference.md` 的附录 A 区间逐行计数 ⇒ **231**（`agent` 行的 `` `[stub]` `` 是属性、不是成员）。
- 一致性册 §5.4 实际登记 **X-01…X-25**。

**影响**：agent 按 `214` 判断「这 17 个新成员不在规范附录 A 里」，会转而只依赖技能包二手转述；`generated` 的措辞还会让它以为该表会随实现漂移（规范 §0.7 规则 1 恰好明令它**不得**漂移）。`X-01…X-15` 同理会让 agent 忽略 X-16…X-25。

---

#### F-21 技能包：`examples\concurrency_cancel.wll` 的「Expected stdout」与实际输出不符

- **判定**：**缺陷（P3）**
- **状态**：**已核实**

`wlwl-skill\examples\concurrency_cancel.wll:3-7` 声明期望 **4 行**：

```
// Expected stdout:
//   [TRUE, TRUE]
//   PASS:concurrency_cancel
//   ChannelClosed
//   PASS:channel_closed_kind
```

实际输出 **3 行**（`kind` 只被 `==(kind,"ChannelClosed")` **比较**，从未被 `PRINT`）：

```
[TRUE, TRUE]
PASS:concurrency_cancel
PASS:channel_closed_kind
```

**影响**：抄示例的 agent 会以为应输出一行 `ChannelClosed` 文本，从而自己加一条 `PRINT`，或误判示例跑错了。

---

#### F-22 技能包：`reference.md` §9.1 的导语仍按 8 个成员写（表内 11 个）

- **判定**：**缺陷（P3）**
- **状态**：**已核实**

`wlwl-skill\reference.md:656`：「All eight members take/return `STRING` and work on its **UTF-8 bytes**.」

而同节表格（`:670-672`）已列出 `UTF8_ENCODE`（→ `ARRAY[INTEGER]`）、`UTF8_DECODE`（`ARRAY[INTEGER]` → `STRING`）、`BYTE_LEN`（→ 整数）。标准库·注册镜像册 §附录 A 记 `std.encode` 共 **16** 个成员。

**实测读数**：`UTF8_ENCODE("héllo")` ⇒ `[104, 195, 169, 108, 108, 111]`（**数组**，不是 `STRING`）；`BYTE_LEN("héllo")` ⇒ 6 而 `LEN` ⇒ 5。

**影响**：agent 读到导语会以为 `std.encode` 全是文本变换，写出 `UTF8_ENCODE(s)` 后拿它当字符串用。

---

#### F-23 规范侧待裁决（4 条）

| # | 事项 | 冲突点 | 实测/证据 |
|---|---|---|---|
| **F-23a** | 语言册 **§8.6.2 的实现注**仍说「参考实现当前在**切段**任务体路径上不报 `E0103`」 | 与**同一册** §0.9 的 **D-15**（「已实现：切段任务体路径的 E0103，`e0103_contract` 28/28 绿」）自相矛盾 | 三形状读数**逐字相同**：顶层 / 普通闭包 / 切段任务体 ⇒ 全部 `E0103` + exit 1（`e0103_c_segmented_task.wll`） |
| **F-23b** | 附录 **A.2 注②** vs **§2.6 第 2 条** | A.2 要「尖括号与箭头形式**直接报错**(`E0010`/`E0012`)」；§2.6 说 `FUN(INTEGER) -> STRING` 与 `DICT<STRING>` 是「**静默吸收**」 | 实现跟的是 §2.6：`DICT<STRING,INTEGER>` ⇒ `E0011: expected ')', got Gt`；`DICT<STRING>` 与 `FUN(INTEGER) -> STRING` ⇒ **无诊断**、运行成功 |
| **F-23c** | §1.3 的 `identifier` EBNF vs「`_` 是通配符」；§1.4/§12「保留集为空」 vs 附录 A.2 的 8 个引号终结符 | ① EBNF 要求首字符是 `letter`，与「`_` 是标识符」互斥；② 附录 A 把 `OK`/`ERR`/`TRY`/`PANIC`/`MATCH`/`IMPORT`/`EXPORT`/`SEALED` 写成**引号终结符**，而 §12 说保留集为空 | `LET(MUT,1)` ⇒ 成功 ✓（上下文关键字）；`LET(OK,1)` ⇒ `E0010: expected identifier in LET, got Ok`（⇒ §3.5 的 `E0025`/`W0030` 对它们**永久不可达**）；`LET(_x,1)` ⇒ 成功（与散文一致、与 EBNF 相反） |
| **F-23d** | §17.9 把「无人 `AWAIT` 的 `ERR` 终值」指向 §8.6 的丢弃点规则，但 §8.6.2 的三处丢弃点**不覆盖**它 | 见 F-10 | 实测：**静默丢弃**（stderr 空、exit 0） |

---

## 3 交叉确认矩阵

每条结论的**独立通道**与**复核方式**。「主审」= 本次动态探针轨；「A/B/C」= 三路并行静态审查。

| # | 主审动态 | A（词法/语法） | B（标准库） | C（技能包） | 最终判定 |
|---|---|---|---|---|---|
| F-01 | ✅ 直接跑仓内二进制 ⇒ exit 1 | — | ✅ 独立复刻同一 3 行差异 | — | **缺陷**（两路一致） |
| F-02 | ✅ 实测 `CAT` 四形态 | — | ✅ 读码定位 `str.wll:16` 无 R2 kernel | — | **未登记差异**（两路一致） |
| F-03 | ✅ 实测顶层 `TCP_ACCEPT` 101 ms 即失败 | — | ✅ 实测同形态 + 定位 `lib.rs:8715` | — | **规范文本缺口**（两路一致） |
| F-04 | ✅ 29 例抽样 | — | — | — | **缺陷**（主审独占） |
| F-05 | ✅ 实测 span 指向第 2 行（调用在第 9 行） | — | — | — | **缺陷**（主审独占） |
| F-06 | ✅ 复现 `E0010` vs 对照 `E0025` | ✅ 定位 `lexer:633-635` | — | — | **缺陷**（两路一致） |
| F-07 | ✅ 复现 `E0020` + 误导 hint | ✅ 定位 `parser:2049` | — | — | **缺陷**（两路一致） |
| F-08 | ✅ 复现顶层 `E0013` / 嵌套 `E0011` | ✅ 定位 `parser:709-722` | — | — | **缺陷**（两路一致） |
| F-09 | ✅ 复现 `exports: []` + 零诊断 | ✅ 定位 `parser:892-893` | — | — | **缺陷**（两路一致） |
| F-10 | ✅ 实测静默丢弃 + 两组对照 | — | — | — | **规范文本缺口**（主审独占） |
| F-11 | — | — | — | ✅ 定位 `SKILL.md:510` 与 `:55` 对撞 | **缺陷**（C 独占） |
| F-12 | ✅ 实测 X-06 两条 E0103 | — | ✅ 核 X 表 25/25 | ✅ 实测 X-07 报 E0103 | **缺陷**（三路一致） |
| F-13 | ✅ 实测 fs 全部 `ERR` 值、零 `E0061` | — | ✅ 12 条失败路径实测 | ✅ 定位 `SKILL.md:761` 尾部 | **缺陷**（三路一致） |
| F-14 | ✅ 实测 `"a" "b"` ⇒ `E0011` | ✅ 定位 `lexer:302-306` | — | — | **缺陷**（两路一致） |
| F-15 | — | ✅ 实测 `a。b` 合法 | — | — | **缺陷**（A 独占） |
| F-16 | ✅ 实测 `0x1F` / `b"AB"` | ✅ 定位扩展分支 | — | — | **待裁决**（两路一致） |
| F-17 | — | — | — | ✅ 实测 415 ms 等完 | **缺陷**（C 独占） |
| F-18 | ✅ 实测孤儿报告行格式 | — | — | ✅ 定位正文零覆盖 | **缺陷**（两路一致） |
| F-19 | — | — | — | ✅ 三套口径对撞 | **缺陷**（C 独占） |
| F-20 | ✅ 独立数出 231 | — | ✅ 独立数出 231 | ✅ 定位 `214` / `generated` | **缺陷**（三路一致） |
| F-21 | — | — | — | ✅ 实测 3 行 vs 注释 4 行 | **缺陷**（C 独占） |
| F-22 | — | — | — | ✅ 定位 `reference.md:656` | **缺陷**（C 独占） |
| F-23a | ✅ 实测切段报 E0103 | — | ✅ 定位 `lib.rs:10485` | ✅ 指出规范注过时 | **规范缺口**（三路一致） |
| F-23b | — | ✅ 实测实现跟 §2.6 | — | — | **规范缺口**（A 独占） |
| F-23c | ✅ 实测 `LET(MUT,1)` / `LET(OK,1)` | ✅ 定位四处互斥 | — | — | **规范缺口**（两路一致） |
| F-23d | ✅ 实测静默丢弃 | — | — | — | **规范缺口**（主审独占） |

### 3.1 跨通道一致的关键读数（同一事实，三路各自独立测出同值）

| 事实 | 主审 | A | B | C |
|---|---|---|---|---|
| 成员面 = 21 命名空间 / **231** 成员 | ✅ 逐 `member_count` 求和 | — | ✅ 脚本双向对拍 | ✅ 数附录 A |
| 附录 A 逐字对拍在 HEAD 判红 | ✅ 直接跑二进制 exit 1 | — | ✅ 复刻同一 3 行 | — |
| 切段任务体**报** `E0103` | ✅ 三形状读数相同 | — | ✅ 定位 `scope_check` | ✅ 实测 exit 1 |
| 孤儿诊断失败 ⇒ stderr 一行 + exit 0 | ✅ `[wlwl] unhandled task failure (task #0): E0030 …` | — | ✅ | ✅ 同格式 |
| `std.fs` 零 `E0061`/`E0062` | ✅ 4 条路径 | — | ✅ 12 条路径 | ✅ |
| `INT` 严格 / `TO_INT` 宽松 | ✅ 7 组输入 | — | ✅ 7 组输入 | ✅ |

---

## 4 修复批次设计（可执行）

> **纪律**：每批自带**依赖 / 验收命令 / 反向守卫 / 非目标**。⚠ **不要先做再补裁决**（同 v0.12.0 计划 §0.3）：F-10 / F-02 的收敛方向需先拍板。

### 批次 R-1 —— 门禁与元数据（P0，**发版前必须**）

| 项 | 内容 |
|---|---|
| **依赖** | 无 |
| **改动位置** | ① `impl\crates\wlwl-eval\src\stdlib_mirror.rs:201`（`std.str` 层 → `混合(R1 … + R2 \`CAT\`)` 或按 R-2 的裁决改实现）；② 同文件 `:271`（`std.result` 层 → 补 `+ R2 \`TAKE_ERR\``）；③ `impl\crates\wlwl-std\src\time.rs:53`（`SLEEP_ASYNC` 的绑定表位置 → 与快照顺序一致） |
| **验收命令** | `cd impl && .\target\release\check-appendix-a.exe` ⇒ **exit 0** |
| **反向守卫** | 在 `stdlib_appendix_a_sync.rs` 增设一条**逐字**对拍用例（覆盖「层」列与顺序），并把它改造成能红的守卫：故意改一处层字符串 ⇒ 该用例必红 |
| **非目标** | 不动附录 A 的冻结快照内容；不改任何成员的语义 |
| **附带** | 把 `cargo run --bin check-appendix-a` 接进 `.github\workflows\ci.yml`（建议放在 §5.4 门禁 job 之后），**并在 job 名里写明「逐字对拍」**，避免与 6/6 绿的那条混淆 |

### 批次 R-2 —— 宿主挂起的顶层失败模式与诊断（P0）

| 项 | 内容 |
|---|---|
| **依赖** | 与 R-1 无耦合，可并行 |
| **改动位置** | ① `impl\crates\wlwl-eval\src\lib.rs:8715-8717`（`E0014` 的消息与 span：区分「`YIELD` 在任务体外」与「宿主挂起点成员在任务体外」，后者消息应指名**成员**并带上**调用点 span**）；② 同文件 `:8428-8430`（顶层 eval 路径） |
| **验收命令** | ① 顶层 `TCP_ACCEPT(l, 2000)` ⇒ 诊断**指名 `TCP_ACCEPT`** 且 span 指向调用行；② 顶层 `SLEEP_ASYNC(50)` ⇒ 同款；③ 任务体内 `TCP_ACCEPT(l, 50)` ⇒ 仍返回 `NULL`（§21.6.2 不回归） |
| **反向守卫** | 负向验证：把 span 改回模块首行 ⇒ 守卫必红 |
| **规范侧同步** | ① 标准库·系统册 §21.5 的 `TCP_ACCEPT` 行与 §15.1 的 `SLEEP_ASYNC` 行的「失败」列**补上**「调用点不在任务体内 ⇒ `E0014`」；② §21.6.2 的表加一句前置条件；③ 并发册 §17.0 的挂起点表加一行「宿主挂起点：任务体外 ⇒ `E0014`」 |

### 批次 R-3 —— 核心内建诊断的源位置（P0）

| 项 | 内容 |
|---|---|
| **依赖** | 无 |
| **改动位置** | 核心内建的实参校验路径（`impl\crates\wlwl-eval\src\lib.rs` 中 `LEN` / `+` / `INDEX_GET` / `INDEX_SET` / `AT_K` / `NEW` / `UPPER` / `LOWER` / `STR` / `TYPE` / `BOOL` / `CONCAT` / `CONTAINS` 等的**元数与类型**检查点）—— 让诊断带上**调用点**的 span（与 stdlib 成员层已有的做法一致） |
| **验收命令** | `LEN(1)` ⇒ `--> <file>:<line>:<col>` 且 `source_line` 非 `null`；`--format json` 的 `location.file` 不是 `<runtime>` |
| **反向守卫** | 新增契约用例：至少 8 条核心内建的实参校验诊断，逐条断言 `location.line > 0` 且 `source_line` 非空；负向验证（把 span 改回 `<runtime>`）⇒ 必红 |
| **非目标** | 不改诊断码；不改诊断消息的措辞（除 R-2 指定的宿主挂起那条） |

### 批次 R-4 —— 关键字边界与对象模型的零参形态（P1）

| 项 | 内容 |
|---|---|
| **依赖** | 无 |
| **改动位置** | ① `impl\crates\wlwl-lexer\src\lib.rs:633-635` ⇒ `IS_OK` / `IS_ERR` / `OR_DIE` **降级为标识符**，由求值器在**调用位置**解析（与 `OK`/`ERR`/`MATCH` 等保持不同处理）；② `impl\crates\wlwl-parser\src\lib.rs:2049` + `:2171-2183` ⇒ 裸 `THIS` 解析为**零参引用**（方法体外 ⇒ `E0032`） |
| **验收命令** | ① `LET(IS_OK,1)` ⇒ `E0025: cannot shadow built-in 'IS_OK'`；开 `allow_builtin_shadow` ⇒ `W0030` + 运行成功；② `FUN((IS_OK),1)` ⇒ 解析通过；③ 裸 `THIS` 在方法体内 ⇒ 接收者；方法体外 ⇒ `E0032` |
| **反向守卫** | 契约用例：上述 5 条读数逐条断言；负向验证（把 `IS_OK` 改回关键字、把裸 `THIS` 改回 `Var`）⇒ 必红 |
| **非目标** | 不动 `OR_DIE` 的 `W0051` 弃用警告行为 |

### 批次 R-5 —— 诊断码精度与 `ModuleDecl` 位置（P1）

| 项 | 内容 |
|---|---|
| **依赖** | F-23c（规范侧对 `_` / 保留集的口径）需先拍板 ⇒ **本批可先做实现侧**，规范侧随后对齐 |
| **改动位置** | ① `impl\crates\wlwl-parser\src\lib.rs:709-722` ⇒ 嵌套块里省略必需的 `;` 报 **`E0013`**（与顶层一致）；② 同文件 `:892-893` ⇒ `IMPORT` / `EXPORT` 在**非模块顶层**时报诊断（建议复用 `SEALED` 的 `module_top` 闸门，`:2127-2141`），**不留静默吞掉** |
| **验收命令** | ① `PRINT((PRINT(1) PRINT(2)));` ⇒ `E0013`；② `LET(x, (EXPORT(["b"])));` ⇒ **非零退出 + 诊断**（不再是 exit 0 + `exports: []`） |
| **反向守卫** | 契约用例：5 种嵌套形状（`Block` / `FUN` / `IF` / `WHILE` / `FOR`）都断言 `E0013`；负向验证（把闸门去掉）⇒ 必红 |
| **非目标** | 不改顶层行为（顶层 `E0013` 不动） |

### 批次 R-6 —— 孤儿任务的 `ERR` 终值（**需业主裁决**）

| 项 | 内容 |
|---|---|
| **依赖** | ⚠ **先裁决**：① 规范侧补「任务终值」是第**四**处丢弃点（语言册 §8.6.2）；还是 ② 实现侧在孤儿任务的 `ERR` 终值上补一行报告（对齐 §17.9 的报告行形态） |
| **推荐** | **②**（实现侧补报）。理由：① 要动语言册 §8.6.2 的丢弃点清单 ⇒ 影响面比一条报告行大得多；② §17.9 的报告行形态已经存在且可复用（`[wlwl] unhandled task failure (task #N): …`） |
| **验收命令** | 孤儿任务终值是 `ERR` ⇒ stderr 出现一行、程序继续、**退出码 0**；被 `AWAIT` 的同一形态 ⇒ 仍按现状（调用点 `E0103`） |
| **反向守卫** | 契约用例：孤儿 + 绑定持有 `ERR` ⇒ 仍是 `E0103` + 孤儿报告行（**不得**双报） |

### 批次 R-7 —— 技能包修订（P1，**发版前必须**）

| 项 | 内容 |
|---|---|
| **依赖** | 无（纯文档） |
| **改动位置** | ① `wlwl-skill\SKILL.md:510` ⇒ 删掉「Hard rule 6」，改为与 `:55` 一致的「**不得**跨任务共享可变 cell，正确形态是 actor + 通道」；② `SKILL.md:739-743` + `reference.md:243-246` ⇒ 删除「X-06 / X-07 是已知缺口」（已收敛），改为「v0.11.4 已收敛，v0.12.0 起一条 `ERR` 都不会丢」；③ `SKILL.md:761` 尾部 ⇒ 删掉「the three frozen v0.3 members still use `E0060`–`E0062`」；④ `SKILL.md:712` + `:729-730` + `reference.md:198` / `:1036` ⇒ 消费者注册表补 `TAKE_ERR`，删掉「§8.3 takes no new entries」；⑤ `reference.md:2366-2367` ⇒ 按 §28.6 的 A4 订正改写（`SCOPE` **先等后取消**）；⑥ `SKILL.md:865` + `reference.md:88` ⇒ `214` → **231**；`generated` → `frozen snapshot`；`X-01…X-15` → `X-01…X-25`；⑦ `SKILL.md` 正文补一节「任务失败的报告义务」（把 `reference.md:2369-2377` 提上来）；⑧ `SKILL.md:16-17` + `reference.md:2327-2348` ⇒ 语义变更清单统一为 **7 条**（D-14…D-20）；⑨ `reference.md:656` ⇒ 「All eight members」改成 16 个成员、说明字节层 3 员走 `ARRAY[INTEGER]`；⑩ `examples\concurrency_cancel.wll:3-7` ⇒ Expected stdout 改成实际的 3 行 |
| **验收命令** | 重跑技能包全部 22 个示例 ⇒ **全部 exit 0**；`grep -n "214\|X-01…X-15\|E0060–E0062\|Shared state = " wlwl-skill\SKILL.md wlwl-skill\reference.md` ⇒ **零命中** |
| **反向守卫** | 给技能包加一条 CI 检查：上述关键字符串若重新出现 ⇒ 红 |
| **非目标** | 不改技能包的示例**源码**（只改 `concurrency_cancel.wll` 的注释） |

### 批次 R-8 —— 诊断文案与文档小项（P2，可与 R-1…R-7 同批）

| 项 | 内容 |
|---|---|
| **改动位置** | ① `impl\crates\wlwl-lexer\src\lib.rs:302-306`（`E0002` 建议：**删掉**「split into two adjacent strings」这半句——WLWL 没有相邻串拼接）；② 同文件 `:294-301`（`E0001` 建议：补 `\$`、指数、十六进制；标识符说明改为「Unicode 字母类别 + 数字 + `_`」）；③ `impl\crates\wlwl-lexer\src\lib.rs:575-609`（标识符的 Unicode 类别收窄 ⇒ 与 §1.3 对齐）—— ⚠ 第 ③ 项是**行为变更**，会拒掉 `a。b` 这类标识符 ⇒ 需按 §3.4 判断是否属破坏性变更并写进 CHANGELOG |
| **验收命令** | `PRINT("abc);` ⇒ 建议文本不再含「adjacent strings」；`PRINT(@);` ⇒ 建议文本含 `\$` 与指数形态 |
| **反向守卫** | 契约用例：断言 `E0001`/`E0002` 的建议文本**逐字**匹配期望串 |

---

## 5 规范侧待裁决项（跨仓，需业主拍板）

| # | 事项 | 影响仓 | 本报告的建议 |
|---|---|---|---|
| **S-1** | F-02：`CAT` 的层归属（改实现为 R2 内核 **vs** 修规范文本） | 规范仓 + 实现仓 | **改实现为 R2 内核**（不动冻结快照；§0.7 规则 1 明令快照不得为追平实现而改写） |
| **S-2** | F-10 / F-23d：孤儿任务 `ERR` 终值的处置 | 规范仓（语言册 §8.6.2 / 并发册 §17.9） | **实现侧补报**（复用 §17.9 的报告行形态），规范侧在 §17.9 末段把「按 §8.6 的丢弃点规则处置」改写为「按 §17.9 的报告路径处置」 |
| **S-3** | F-23b：附录 A.2 注② vs §2.6 第 2 条（尖括号 / 箭头形式） | 规范仓 | 以 **§2.6** 为准（实现已跟它），把 A.2 注②改成「**不承认但不报错**」 |
| **S-4** | F-16：十六进制字面量 / 字节串字面量是否登记进 §1.6 / §1.8 | 规范仓 | 登记（它们已是实现的事实面，且 `wlwl-skill` 与实现文档都在用）；或在 §12 声明「本实现的词法面是 v1.0.2 的超集」 |
| **S-5** | F-23a：语言册 §8.6.2 的实现注过时 | 规范仓 | 删掉该实现注（D-15 已翻「已实现」） |
| **S-6** | F-23c：`identifier` EBNF vs `_` 通配符；§1.4/§12 保留集 vs A.2 引号终结符 | 规范仓 | EBNF 补 `_`；§12 的「保留集为空」改写为「保留集 = A.2 的 8 个引号终结符 + §1.4 的 15 个关键字」 |
| **S-7** | F-03：宿主挂起点成员的「任务体外 ⇒ `E0014`」写入规范失败列 | 规范仓 | 写入（§21.5 / §15.1 的失败列 + 并发册 §17.0 挂起点表） |

---

## 6 正面确认清单（发版可以依赖的部分）

### 6.1 完整测试套件

```powershell
cd D:\Project\wlwl\impl
cargo test --locked --workspace --all-targets --no-fail-fast
```

**读数**：**102 个测试目标 / 3014 条通过 / 0 失败 / 2 忽略 / exit 0**（2026-10-26，`e789345`，Windows x64，debug profile）。

- 全部 `test result: ok`；最大的两个目标是 `914` 与 `348` 条。
- ⚠ 与仓内既有口径一致：`--all-targets` 下另有 11 个 bench 目标跑在测试模式、**不报结果行** ⇒ 「测试目标数」与「`test result:` 行数」**不是**同一个口径（仓内 README 已有此提醒）。
- ⚠ **本读数为「全绿」，但它没有覆盖本报告的三条 P0**：仓内契约表**不含**「附录 A 逐字对拍」（那是独立二进制，不在 CI）、**不含**「核心内建实参校验诊断的源位置」、**不含**「顶层宿主挂起的失败模式与 span」。⇒ **测试全绿 ≠ 发版闸门全绿**，这三条正是靠本报告的独立实测抓出来的。

### 6.2 公共面与成员面

| 项 | 读数 |
|---|---|
| 17 个新成员**全部存在**且语义合规 | `std.net` 14（5+9）/ `std.encode` 16（13+3）/ `std.text` 8（+1）/ `std.str` 8（+1）/ `std.env` 5（+1）/ `std.result` 7（+1）/ `std.time` 7（+1） |
| 成员面 = 附录 A ≡ 各册成员表 ≡ 实现 | **21 命名空间 / 231 成员**（std 221 + 官方包 10），**逐命名空间集合相等、零缺零多** |
| 无「规范外扩展」泄漏 | `IMPORT` 试探 kernel 名（`_CAT` / `_SQRT` / `_KIND` / `kernel_take_err`）⇒ 全部 `E0023` |
| §5.4 差异登记门禁 | **25 行逐行一致、零错配**；`X-14` = `no-contract` ✓（D-20 要求） |

### 6.3 错误模型（§8.6 丢弃点矩阵 —— 12/12 合规）

| 丢弃点 | 期望 | 实测 |
|---|---|---|
| 模块顶层作用域结束 | E0103 | ✅ exit 1 |
| 普通闭包体作用域结束 | E0103 | ✅ exit 1 |
| **切段任务体**作用域结束 | E0103 | ✅ exit 1（**D-15 已落地**） |
| 非切段任务体作用域结束 | E0103 | ✅ exit 1 |
| 语句值丢弃 | E0103 | ✅ |
| 显式丢弃 `LET(_, expr)` | **无** | ✅ exit 0 |
| 绑定被覆盖 | E0103 | ✅ |
| 只观察（`IS_ERR`）不消费 | E0103 | ✅ |
| `TAKE_ERR` 消费 | **无** | ✅ exit 0（且 `IS_ERR` 仍为真） |
| 容器装盛（转发） | E0103 | ✅（与 §8.6.3 的示例**逐字**一致） |
| 两个未消费 `ERR` | **两条** E0103，按出现序 | ✅ 两条，顺序正确 |
| 程序值（顶层末表达式） | **E0102**（非 E0103） | ✅ |

### 6.4 并发（§17）

| 项 | 实测 |
|---|---|
| §17.9 孤儿**诊断**失败 | stderr **恰好一行**：`[wlwl] unhandled task failure (task #0): E0030 LEN: …`；程序**继续**；**退出码 0** ✓ |
| §17.9 被 `AWAIT` 认领的失败 | 在**调用点**抛出；**不**另写报告行；退出码 1 ✓（两条路径互斥 ✓） |
| §17.1 `SCOPE` 返回前等子任务 | 停在通道上的子任务被排干；**不挂住程序**；退出码 0 ✓ |
| D-18 宿主挂起只挂当前任务 | 等待期间**别的任务照常跑完**（`OTHER-TASK-RAN` 先于 accept 完成打印）✓ |
| D-19 气泡内 `SLEEP_ASYNC` ≡ `SLEEP` | 气泡内立即返回 ✓ |
| 段首重放 | 宿主挂起从**段首重放**（等待前的语句重跑）；`YIELD` 切段不重放 ✓ |

### 6.5 传输层（`TCP_*` 九员 —— 端到端，真客户端）

| 项 | 实测 |
|---|---|
| `TCP_LISTEN` + `TCP_LOCAL_ADDR` | 端口由宿主分配（`port=0` ⇒ 实际端口 > 0）；`{host, port}` ✓ |
| `TCP_ACCEPT` 超时 | ⇒ **`NULL`**（正常值，`IS_ERR` 为假）✓ §21.6.2 |
| `TCP_RECV` 超时 | ⇒ `{bytes: [], text: "", eof: FALSE}` ✓ §21.6.2 |
| `TCP_RECV` 读到数据 | ⇒ `bytes` 逐字节无损（`"ping"` ⇒ 4 字节）；`text` 同一批字节的 UTF-8 解读 ✓ |
| `eof` | 对端关闭且缓冲排空后 ⇒ `eof: TRUE` ✓ |
| `TCP_SEND` | 返回**写入字节数**（`[65,66,67]` ⇒ 3）✓ |
| `TCP_SET_TIMEOUT` | 返回 `NULL`；**两个预算先到者为准**（连接读超时 300 ms 胜过调用点 2000 ms ⇒ 该次读返回空）✓ §21.6.4 |
| `TCP_CLOSE` / `TCP_CLOSE_LISTENER` | **幂等**；关闭**从未存在**的句柄也返回 `NULL`、**零诊断** ✓ §21.6.6 |
| 句柄种类错 / 句柄不存在 | ⇒ `ERR([kind: "SocketError", op, reason])`（**不是**诊断）✓ |
| 元数 / 类型 / 端口越界 | ⇒ `E0022` / `E0030` ✓ |

### 6.6 词法与语法

- 附录 A.2 的 **50 条产生式**中 **44 条完全对齐**（含 §1.8 全部 10 个转义、§2.6 三条类型事实、§7.2 全部模式形态、§5.2 全部形参形态、16 个 `OpToken`、A.3 格式化器幂等与 `--check`）。
- 词法面细节全部实测吻合：`=` ≡ `==`、`!` 无警告、`MUT` 是上下文关键字、指数 / 十六进制字面量、块注释正确嵌套、BOM、`\r` 单换行、中文标识符、`MATCH` 的 7 种模式形态、`*rest` 形参末位约束。

### 6.7 其它

| 项 | 读数 |
|---|---|
| `std.fs` 的 12 条 IO 失败路径 | 全部 `ERR([kind:"FsError", op, reason])`，**零原生诊断**；`E0061`/`E0062` 在全仓只剩码表定义与 CLI 自有故障 ✓ D-14 |
| `INT` 严格 / `TO_INT` 宽松 | 7 个输入上结论相反，逐行兑现 §12.4 对比表 ✓ D-17 |
| 技能包 22 个示例 | **全部 exit 0**，无一报错 ✓ |
| `wlwl namespace` / `schema` / `interface` / `sig` / `ast` / `fmt` / `lsp` | 全部可用，JSON 形态符合附录 E ✓ |

---

## 7 覆盖说明与已排除的怀疑

### 7.1 覆盖说明

**逐行读完**：规范侧 语言核心册 §1、§2、§4.2、§8.3、§8.6、§10.3.1、§11.1–§11.2、§12、§18（抽样）、附录 A、附录 G（抽样）；并发册 §17.0、§17.1、§17.3、§17.9、附录 H（抽样）；对象册 §13–§16；一致性册 §5.4、§5.5、§6.1–§6.2；标准库六册的成员表（核心 §6/§22、系统 §2.3/§15/§18/§21、安全 §11.2/§11.3、文本 §12.4、注册镜像册附录 A）。

**脚本化全量对拍**：附录 A ↔ 各册成员表（15 个命名空间双向）；附录 A ↔ 实现（21 个命名空间双向）；§5.4 ↔ `script/conformance_registry_map.json`（25 行逐行）；诊断位置抽样 29 例。

**明确未覆盖**

1. **仅 Windows 平台** —— `std.fs` 的 POSIX 分支（`\` 不替换、符号链接展开）、`std.process` 的 POSIX 路径**未实测**。
2. `std.json` / `std.math` / `std.collection` / `std.sanitize` / `std.regex` / `std.compress` / `std.format` / `std.test` / `ai` / `agent` 这 10 个命名空间的**成员语义**未逐个实测（只核对存在性与成员面）。
3. `HTTP_*` 传输层（阻塞式 IO）未实测 —— 它与宿主挂起不同款（§21.4），且需要外部服务。
4. 静态类型层（`gradual_typing` = `warn` / `error`）与 `MATCH` 穷尽性层未开关实测。
5. 附录 E.1 的 `.wll.sig` 签名文件文法未逐条核对。
6. Unicode 标识符的**完整码点类别**覆盖（只测了 4 个非 ASCII 样例）。
7. **未跑** POSIX / macOS 平台。

### 7.2 已排除的怀疑（实测或读码确认无问题）

| 怀疑 | 排除依据 |
|---|---|
| 「`INT` 的严格口径靠 Rust `parse` 顺带做了」 | Rust `i64::from_str` **不**裁剪空白 ⇒ 实测 `INT(" 42 ")` ⇒ `ERR:ParseError` ⇒ 严格口径**继承** Rust 语义恰好合规，不是另写的规则 |
| 「`TO_INT` 与 `INT` 其实是同一个东西」 | 7 个输入上结论相反（空白 / 十六进制 / 小数三点）⇒ §12.4 对比表逐行兑现，**故意**不同 |
| 「`std.fs` 还有别的路径发 `E0061`/`E0062`」 | `fs.rs` 全文只有 `E0022`/`E0030`；两码在全仓只剩 `wlwl-error` 的**码表定义**与 `wlwl-cli` 的 **CLI 自有**文件故障（CLI 侧注释已自陈「不再有规范条款为它背书」） |
| 「`TAKE_ERR` 不在 §8.3 消费者注册表」 | 实现侧 `lib.rs:6319-6323` 已放进 `ERR_CONSUMER_REGISTRY`、`:6385-6387` 放进 `ERR_TAKEAWAY_CONSUMERS`；规范侧 §8.3 表与 §8.6.1 取走型表也都已登记 ⇒ 两侧一致（**技能包**才是漏的那个，见 F-14） |
| 「`ERR(NULL)` 的歧义分支不可达」 | `LET(r, ERR(NULL));` ⇒ `E0030: ERR payload must be STRING or DICT, got null` ⇒ **构造不出来** ⇒ 规范与实现文档里的这条歧义描述**不可达**（文档瑕疵，非行为缺陷） |
| 「`TAKE_ERR` 会剥 `OK` 外壳」 | 实测 `TAKE_ERR(OK(7))` ⇒ `NULL`，**不剥壳** ✓ §22.6 第 2 条 |
| 「`CAT` 会把 `ERR` 渲染成文本」 | 实测 `CAT("a", ERR("boom"))` ⇒ `IS_ERR` 为真、载荷仍是 `"boom"` ⇒ **未渲染** ✓ §6:363 |
| 「`AND` / `OR` 被实现成了内建」 | `PRINT(AND(TRUE, FALSE));` ⇒ `E0020: undefined name 'AND'` ✓ §4.3 明写「`AND` 与 `OR` **不是**内建」 |
| 「`!` 会发 `W0054` 弃用警告」 | `PRINT(!TRUE);` ⇒ 无任何警告 ✓ §1.5 |
| 「块注释不支持嵌套」 | `/* a /* b */ c */` ⇒ 解析成功 ✓ §1.2 |
| 「浮点指数形式缺失」 | `1e2` / `1.5e-2` / `1E3` 全部正确；`1e` ⇒ `E0001` ✓ §1.7 |
| 「`f(x)(y)` 应当合法」 | 附录 A.2 的 `Postfix` 只有「`.` identifier」才带调用 ⇒ `mk(10)(5)` 报 `E0011` **是正确**的 |
| 「`d["a"] = 2` 没改到 `d`」 | §4.5 明写 `INDEX_SET` 是值语义、「接收者不变(§2.1)」 ⇒ 非缺陷 |
| 「`(PRINT(1); PRINT(2))` 的值应是 NULL」 | 那是 `PRINT` 的返回值（`NULL`），不是块的值；`PRINT((1; 2))` ⇒ `2` ⇒ 非缺陷 |
| 「`MATCH` 子句形如 `[[1]: "one"]`」 | 那是数组/字典字面量；正确形态是 `Clause = "[" Pattern "," Expression "]"` ⇒ 实现与 A.2 一致 |
| 「`E0002` 的建议文案只是措辞问题」 | 建议**承诺了不存在的语言特性**（相邻串拼接）⇒ 属假修复指引，已列 F-14 |
| 「§5.4 门禁有 stale 条目」 | 25 行逐行比对**零错配**；`X-14` = `no-contract` 正是 D-20 要求的值 ⇒ 门禁本身没问题 |
| 「`wlwl namespace` 少列了成员」 | 其输出即 `stdlib_mirror::members()` 的数据源，与 `stdlib_appendix_a_sync.rs` 的守卫同源；另用 `IMPORT` 试探 kernel 名全部 `E0023` ⇒ 无面泄漏 |
| 「技能包 22 个示例里有跑挂的」 | **全部 exit 0**；唯一「输出与文本相抵触」的是 `concurrency_cancel.wll`（F-21） |

---

## 8 复现材料索引

### 8.1 过程临时文件（`C:\Users\ikrx2\Desktop\111\audit3\`）

| 路径 | 内容 |
|---|---|
| `probes\p01_core.wll` | 语言核心遍历（算术 / 真值 / 字面量 / 字符串 / 下标 / 控制流 / 模式 / 函数 / `LET MUT` / `FORMAT` / 括号序列）—— **全绿** |
| `probes\p05a_int.wll` | D-17：`INT` 严格 vs `TO_INT` 宽松（14 组读数）—— **全绿** |
| `probes\p05b1_bytes.wll` / `p05b2_bytes_err.wll` | D-13 字节层三员 + 越界 `E0030` —— **全绿** |
| `probes\p04a_tcp_basic.wll` | 传输层无需客户端的路径（监听 / 本地地址 / 超时 ⇒ `NULL` / 关闭幂等 / 句柄错）—— **全绿** |
| `probes\p04a1_toplevel_accept.wll` / `p04a2_span.wll` / `p04a3_sleep_async_toplevel.wll` | **F-03 / F-05** 的最小复现 |
| `probes\p04b_tcp_server.wll` + `run_p04b.ps1` | 传输层端到端（PowerShell `TcpClient` 真客户端）—— **全绿** |
| `probes\p04c_tcp_timeouts.wll` + `run_p04c.ps1` | 读超时 / eof / 字节发送 / 两个预算先到者为准 —— **全绿** |
| `probes\e0103_a…l_*.wll` + `run_matrix.ps1` | §8.6.2 的 **12 格丢弃点矩阵** —— **全绿** |
| `probes\p03a/p03d/p03e/p03f/p03g_*.wll` + `run_split.ps1` | §17.9 的两条互斥路径 + **F-10** 的三组对照 |
| `probes\p06_fs.wll` | D-14：`std.fs` 的 4 条失败路径 + 成功路径 —— **全绿** |
| `probes\p08_new_members.wll` | D-13 的 `CAT` / `SCRIPT_PATH` / `TAKE_ERR` 边界 —— **全绿** |
| `probes\x01…x04_*.wll` | F-06 / F-07 / F-08 / F-09 的最小复现 |
| `probes\run_diag_spans.ps1` / `run_diag_spans2.ps1` | 诊断位置抽样 **29 例**（**F-04**） |
| `static\` | 三路静态审查各自的临时产物与最小复现集 |

⚠ 复现命令统一形如：

```powershell
cd C:\Users\ikrx2\Desktop\111\audit3\probes
& 'D:\Project\wlwl\impl\target\release\wlwl.exe' run <file>.wll ; Write-Output "exit=$LASTEXITCODE"
```

### 8.2 仓内可重跑的门禁

```powershell
cd D:\Project\wlwl\impl
.\target\release\check-appendix-a.exe ; Write-Output "exit=$LASTEXITCODE"   # F-01 ⇒ exit=1
cargo test --locked --workspace --all-targets --no-fail-fast
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
$env:RUSTDOCFLAGS='-D warnings'; cargo doc --workspace --no-deps
```

---

## 9 给业主的三句话

1. **能发的部分**：公共面 17 员齐备、成员面 231 ≡ 231 ≡ 231、D-14/D-15/D-16/D-17 的行为层全部实测合规、§8.6 的 12 格丢弃点矩阵全绿、TCP 九员端到端合规、附录 A.2 的 50 条产生式 44 条完全对齐、**完整测试套件 102 目标 / 3014 条全绿**——**这一轮的核心语义面是扎实的**。
2. **发版前必须先做 4 件事**：R-1（把仓内唯一的逐字对拍工具从红变绿**并接进 CI**——它从 D-13 落地起就是红的，且从未跑过）、R-2（顶层宿主挂起的诊断与规范失败列）、R-3（核心内建诊断的源位置——现在 17/17 的实参校验错误都不可定位）、R-7（技能包的 3 条 P1，尤其是把规范**禁止**的共享可变状态写成「Hard rule 6」）。
3. **需要你拍板的 3 件事**：S-1（`CAT` 的层归属改实现还是改规范）、S-2（孤儿任务的 `ERR` 终值：补报告行还是补丢弃点）、S-4（十六进制 / 字节串字面量是否登记进规范）——**我建议的方案都写在 §5 那一列**，可直接照做。

⚠ **最后一句提醒**：测试全绿**不等于**发版闸门全绿。本报告的三条 P0 里，有两条（附录 A 逐字对拍、核心内建诊断源位置）**根本不在任何 CI job 里**，所以「CI 全绿」这个信号对它们**无效**。发版前请按 §4 的验收命令**逐条跑一遍**，而不是只看 CI 的绿灯。
