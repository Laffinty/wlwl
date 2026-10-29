# WLWL 实现侧 v0.12.1 构建计划 —— 迁移需求清单 57 条全量落地

| | |
|---|---|
| **版本** | 实现侧 **v0.12.1**（对应规范仓 `wlwl-spec-v1` **v1.0.4**） |
| **输入** | [`wlwl-迁移需求清单.md`](./wlwl-迁移需求清单.md)（TinyPage 迁移项目提出，2026-10-27，**57 条编号**） |
| **基线** | 实现仓 `wlwl` **v0.12.0**（`9bd0f46`）／规范仓 `wlwl-spec-v1` **v1.0.3**（`eb5e7a8`） |
| **上一版** | `198e1ec` 的 `wlwl-v0.12.1-build-plan.md`（已删）—— 本文件**取代**它，三处方向变更见 §1.2 |
| **业主指令（2026-10-27）** | ① **全量实现**需求清单，不擅自延后；② **TLS 不进标准库**，改用 **Rust 原生 DLL/SO 动态库**交付；③ 规则侧**非必要勿改动**；④ 构建阶段**多 AGENT 作业**；⑤ **版本锁死 v0.12.1 / v1.0.4，不许漂到 v1.1.0** |
| **业主裁决（2026-10-27）** | §10 四项**全部已定**（点选确认）：Q1 动态库扩展 · Q2 并发三项走需求清单候选形态 · Q3 新成员落 `std.*` · Q4 G-2 规则 ② **进 v1.0.4**（故改成**不需要新诊断码**的形态）⇒ **本轮无待决项** |
| **业主裁决（2026-10-28）** | §11.4 四项**全部已定**（点选确认）：D1 `tls::LOCAL_ADDR` **留**（台账 10 员）· D2 `std.http` 重复 `Content-Length` **向 `std.net` 收紧**（W-36）· D3 三个探针**改写成 `TCP_SERVE` 形状**（W-35）· D4a 解析器 `E0011` 误报**本轮修**（W-37）· D4b NFKC 表**保持 17.0**、等 UCD 数据 ⇒ **零待决项** |
| **本轮裁决口径** | 规则侧只做「① 修规范自相矛盾 ② 为新成员/新命名空间走登记 ③ 给扩展一个合法落点」三类，**不动任何既有成员的签名与可观察语义** |
| **▶ 接替者从这里开始** | **§13「接替执行手册」** —— 仓库现状 / 门禁分类 / **下一批该开哪几个 agent（含可直接抄的规格要点与坐标）** / 集成者的固定动作 / 环境事实与 10 个坑。读完 §13 + §11 + §2 即可开工，**不需要重新勘察** |

---

## 0 本文档的契约

### 0.1 这份文档是什么 / 不是什么

**是**：一份**可被 agent 直接施工的施工图**——需求清单的 57 条逐条落到「哪个工作项 / 改哪些文件 / 验收命令是什么 / 怎么证明它真的生效」，外加一份**多 agent 并行作业的切分表**。

**不是**：① 不是需求清单的复述；② 不是「全部照做」的许诺——**本轮 57 条编号全部落地或给出明确口径答复**，其中 **E-6 / E-7 两条按需求方自己的建议归应用层**（§7 台账注明理由）；③ 不是规范文本——规范侧要改的条款在 §6 列成**待写条款**，由规范仓 v1.0.4 批次执行。

> ⚠ **条数订正**：清单正文与页眉都写「**60 条**」，但按编号实数是 **57 条**（A 5 + B 7 + C 12 + D 7 + E 7 + F 4 + G 4 + H 6 + I 5 = 57；H / I 两组是表格形态、不在 `###` 标题里）。本计划的台账按 **57 个编号**逐条落地 —— 上一版计划（`198e1ec`）沿用了「60 条」这个数，**台账本身也只有 57 行**，多出的 3 条并不存在。复算命令见 §2.3 R-23。

### 0.2 状态标记（全文统一）

| 标记 | 含义 |
|---|---|
| **已实测** | 用仓内二进制跑出来的读数，附**可重跑命令**；命令红了以命令为准 |
| **已核实** | 读过规范／源码原文的事实，附 `文件:行号` |
| **待实测** | 结论来自推理，**本轮未跑**——工作项必须补测后才可标「已实测」 |
| **待裁决** | 需要业主拍板；**不阻塞**其它工作项（清单见 §10） |

⚠ **每条「已实测」都带一条可重跑的命令**；⚠ **每条命令引用的夹具必须真实存在**（§2.3 的历史教训：需求清单引用的 9 个探针**从未入库**）。

### 0.3 怎么读、怎么改

- **执行者（agent）**：从 §5 的波次表进；每个工作项的「验收」是**可粘贴的命令**，「反向守卫」是**改回去必须变红**的验证，「归属文件」是**你独占的、别人不许碰的**。
- **业主**：§1 是结论，§6.4 是规则侧改动账，§10 是**要您拍板的 4 件事**。
- **改本文档**：新增工作项**追加**到对应批次末尾并编号；**不要**重排既有编号（§7 台账按编号引用）。

### 0.4 交付形态（收口时必须齐五件）

1. 实现仓 `impl/`：全部工作项落地 + 门禁全绿（§8）；
2. 规范仓 `wlwl-spec-v1`：**v1.0.4** 落地（§6 的条款级改动 + 总纲 §0.9 登记行 + 各册成员表 + 附录 A + §5.5 台账第一行）；
3. 两仓的 `docs/wlwl-spec-reference.md` **重建**（`python script/build_spec_reference.py`）；
4. **TLS 扩展产物**：`wlwl-tls` 动态库（Windows `.dll` / Linux `.so` / macOS `.dylib`）随发行包发布，并带 ABI 版本与哈希；
5. 需求清单的 **9 个探针固化进仓**（`docs/wlwl-feasibility/probes/` + `impl/tests/probe/cases/`），使清单里每条「现状」都有可重跑的证据。

---

## 1 结论摘要

### 1.1 一句话结论

> **57 条需求全部落地**：**50 条**变成实现侧工作项（**32** 个工作项：W-01…W-38 + 业主裁决派生的 W-35 / W-36 / W-37）、**5 条**是口径答复（I 组）、**2 条**按需求方自己的建议归应用层（E-6 / E-7）、**9 条**被需求清单自身的事实前提订正后并入上述工作项。
> **规则侧只动 3 类**：① **修一处规范自相矛盾**（语言册把 `E0014` 说成「只用于 `YIELD`」，与并发册「宿主挂起点成员在任务体外一律 `E0014`」正面矛盾）；② **给新增成员/命名空间走登记**（§3.2 允许修订版本新增**非破坏性**成员/命名空间，条件是总纲 §0.9 有登记行 + 发布说明显式列出）；③ **给 TLS 扩展一个合法落点**（一致性册 §5.5「实现侧扩展台账」本版为空，第一行留给本轮）。
> **TLS 走 Rust 原生动态库**：TLS **不进标准库**（`std.net` 明写「不做 TLS」、安全册明写「对称/非对称加密不做」、`ADR-0028 D3` 明写「零新增第三方依赖」），改为**独立编译的 `cdylib`**（`rustls` 自持），由宿主**显式授权**加载（CLI `--ext`），落进 §5.5 台账 ⇒ **不进公共面、不动附录 A**。

### 1.2 与上一版计划（`198e1ec`，已删）的三处方向变更

| # | 上一版 | 本版 | 依据 |
|---|---|---|---|
| **Δ1** | A 组 TLS「本轮不做」，走「外部 TLS 终结器 + wlwl 明文后端」 | **本轮做**，形态 = **Rust 原生 DLL/SO 动态库** + 宿主加载器 + §5.5 台账；**不进 `std.*`** | 业主指令（2026-10-27）；分析结论支持「不进标准库」这半句，但**否决**了「不做」 |
| **Δ2** | B-3 / B-4 / B-5「阻塞于 `ADR-0017` Step 3 ⇒ 移出 v0.12.1」 | **本轮做**，且**不需要 Step 3**：B-3 走「迭代器式」形态（需求清单候选 a）、B-4 走「共享容器 + 原子更新」形态（候选 a）、B-5 走「宿主挂起点式」新成员。三条都在需求清单自己列的候选清单里 | §4 B-3/B-4/B-5 的形态裁决；`ADR-0017 Step 3` 仍是「流式协程」路线的独立立项 |
| **Δ3** | 「本轮不做、进路线图」共 4 项（B-3 完整形态、G-3 调试器、F-2 指标、H-6 别名） | **全部落地**（G-3 按需求清单的「至少一个能跑表达式的 REPL」最小口径做） | 业主指令 |

⚠ **上一版计划的三条「实测更正」继续有效**（H-1 已修、H-2/H-3 已有全局内建、B-6 已被 §17.9 覆盖），本版沿用（§2.4）。

### 1.3 57 条 → 裁决分布

| 裁决 | 条数 | 编号 |
|---|---|---|
| **✅ 实现侧落地**（工作项 W-01…W-35） | **50** | A-1 A-2 A-3 A-4 A-5 B-1 B-2 B-3 B-4 B-5 B-6 B-7 C-1…C-12 D-1…D-7 E-1 E-2 E-3 E-4 E-5 F-1 F-2 F-3 F-4 G-1 G-2 G-3 G-4 H-1 H-2 H-3 H-4 H-5 H-6 |
| **📖 口径答复**（需求方明确只要排期口径） | **5** | I-1 I-2 I-4 I-5（+ I-3 兼落地 TCP_CONNECT） |
| **🔀 归应用层**（按需求方自己的建议） | **2** | E-6 E-7 |
| **🔧 事实前提订正后并入上面** | （已计入 50） | B-1 B-2 B-6 H-1 H-2 H-3 H-4 H-5 H-6 E-4 D-1 |

> 逐条台账见 **§7**（**57 行**）。**没有任何一条以「延后到下一版」结案**；口径答复的 5 条在 §7 里给出**可执行的排期口径**。

### 1.4 规则侧（v1.0.4）改动账

| # | 类别 | 改什么 | 依据 | 落在哪条例外 |
|---|---|---|---|---|
| **SP-1** | **修自相矛盾** | 语言册 `E0014` 触发条件：删掉「在并发下**只**用于一个位置：`YIELD` 在任务体外」这句，与并发册 §17.0 的「宿主挂起点成员在任务体**外任何位置**（含嵌套闭包内）⇒ `E0014`，且**指名成员** + **指向调用点**」对齐 | `wlwl-v1.0-language.md:1145` / `:1230` **vs** `wlwl-v1.0-concurrency.md:94` | 一致性册 §3.4 **例外 2**（规范缺陷修复） |
| **SP-2** | **补规范空白（条件项）** | ⚠ **仅当**实现仍保留「竞争落选 ⇒ `NULL`」语义时才需要：同一监听句柄上多个 `TCP_ACCEPT` 等待者时，**落选者返回 `NULL`（正常值，与超时同形）**，**不得**因此终止任务，**不得**连带关闭监听句柄。**若按 W-11 的「唤醒一个」实现 ⇒ 竞争不再产生 `NULL` ⇒ 本条作废、规则侧零改动** | `wlwl-v1.0-stdlib-system.md:813` 只规定了「超时 ⇒ `NULL`」，**未覆盖**竞争落选 ⇒ 一致性册 §3.5.1「规范文本缺口，必须清零」 | §3.4 **例外 2**（缺口清零，措辞**不引入新自由度**：只规定已发生竞争时的可观察结果） |
| **SP-3** | **为新成员让路** | 并发册 §17.0.1：把「`MUTEX` / `CONDITION` / `ONCE` / `SEMAPHORE` / `WAIT_GROUP` 不属于本语言面，**它们属标准库范畴**」这条**落成标准库册的成员**（`std.sync`），并把「在当前架构下锁不是一个可实现的成员」这句**限定为「语言级锁」**（标准库提供的原子共享容器不是语言级共享 cell） | `wlwl-v1.0-concurrency.md:153` / `:157` / `:147` | 措辞澄清 + **新增成员**（§3.2 修订版本通道，条件见下） |
| **SP-4** | **新成员 / 新命名空间登记** | 新命名空间 `std.http` / `std.log` / `std.sync` / `std.signal`；既有命名空间的新成员（`std.text` / `std.fs` / `std.str` / `std.test` / `std.format` / `std.net` / `std.regex` / `std.collection`）⇒ **附录 A 逐字改 + 各册成员表 + 总纲 §0.9 登记行 + 发布说明显式列出** | 一致性册 `:130`（修订版本可含新增非破坏性成员/命名空间，**条件**：§0.9 有登记行 + 发布说明列出） | §3.2 **修订版本通道**，先例 = 总纲 §0.9 的 **D-13**（17 个新公共面成员走 v1.0.2 修订版） |
| **SP-5** | **给扩展合法落点** | 一致性册 §5.5「实现侧扩展台账」**第一行**：`tls` / `crypto` 两个命名空间的成员（登记理由 / 效力声明 / 随哪个实现仓发布 / 收敛路线 / 拟进公共面的版本） | 一致性册 `:514-532`（本版台账为空，**第一个台账条目留给后续版本**） | 台账**不属公共面**（`:206`）⇒ **不进附录 A**、**不动公共面清单** |

⚠ **三条硬约束**（决定了 SP-1…SP-5 的写法）：

1. **不得新增诊断码**。一致性册 `:129` 把「新诊断码」列在**次版本**下 ⇒ 1.0.x **无通道**。⇒ 本轮所有新增失败一律走**既有诊断码**（`E0014`）或 **`ERR` 值**（`kind` 是开放字符串，仓内无注册表 ⇒ 结构上零成本）。**G-2 规则 ② 的静态产出走「无 E/W 码的规范符合性提示」**（§10 Q4，业主裁决「v1.0.4 包含」）⇒ 既不新增诊断码，也不动退出码表。
2. **不得修改任一既有成员的签名或可观察语义**（`:223`）。⇒ `std.net::HTTP_*` 的「`https://` 返回 `ERR`」**保持不变**（系统册 `:750`）——HTTPS 出站由**新成员**承担（§3.7）。
3. **不得删除或弃用既有成员**（`:224`）。⇒ `RE_TEST` **不改名**，加别名（`RE_MATCHES_FULL`）；`PATH_NORMALIZE` **不改语义**，加 `PATH_CANONICAL`；`AT_K` / `INDEX_GET` **不统一**，加别名 `ARRAY_GET`。

### 1.5 五个关键决策（每条一句话 + 理由）

| # | 决策 | 理由 |
|---|---|---|
| **D1** | **TLS = 独立 `cdylib` + 宿主显式授权加载 + §5.5 台账**；**不进 `std.*`** | `std.net` 明写「不做 TLS」（系统册 `:750` / `:779`）、安全册明写「对称/非对称加密不做」、`ADR-0028 D3` 明写「零新增第三方依赖」，且 `ADR-0028 §5` 把「顺手加 TLS」列为**明确排除**项 ⇒ 进标准库要同时破三条既有裁决；做成动态库则**发行二进制依赖面不变**，TLS 库可独立跟 CVE、独立升级 |
| **D2** | **加载面只给宿主，不给程序**：只有 `wlwl run --ext <abs-path>` / `WLWL_EXT_PATH` 能加载；**语言面不提供 `LOAD` 成员** | 程序能指定路径 ⇒ 脚本即任意代码执行（ast-grep `GHSA-cmvf-x8f7-rvp6` 的同款形状：项目配置里一条路径 ⇒ 任意代码执行，且构造器在符号校验前就执行）。**这条是 D1 的安全前提**，不可让渡 |
| **D3** | **B-3 / B-4 / B-5 走需求清单自己列的候选形态**，**不引入 `ADR-0017` Step 3** | 候选 a 三条都不需要「挂起后越过阻塞点继续」：B-3 = 原生迭代器成员（accept 循环在原生帧内推进，handler 不挂起）；B-4 = 原生共享容器 + 原子更新（无挂起）；B-5 = 宿主挂起点式新成员（复用 `TCP_*` 已有的 worker-线程 + resume-hint 幂等模式）。**Step 3 留给「流式协程」路线，独立立项** |
| **D3b** | **B-2 修根因**：把「唤醒全部等待者」改成「**唤醒一个**」 | 惊群是**结构性**的（一个 listener 一个 socket id ⇒ 一把等待者 `Vec`，R-28）⇒ 改成唤醒一个后，落选者**根本不会被唤醒**，也就不会拿到 `NULL` ⇒ 需求要的「落选者保持存活并能继续接受后续连接」**直接达成**，且比「返回可区分值」更强 ⇒ 规范侧大概率**零改动**（SP-2 降为条件项） |
| **D4** | **HTTP 协议层落在新命名空间 `std.http`（R1 纯 wlwl 源码）**，不动 `std.net` 既有成员 | 系统册 `:843` 已把「HTTP 的解析与响应构造」判为**调用方的事**；`:844` 明写「传输层不得把协议知识渗进原生层」。放新命名空间 ⇒ chunked 客户端解码、`Cookie`、`multipart`、Keep-Alive 等**全部是新增自由度**，不碰 `std.net::HTTP_*` 的冻结语义 ⇒ 不撞 `:223` |
| **D5** | **新增成员全部走「新增」而非「修改」**：别名 / 新命名空间 / 新选项，**不动既有成员一个字节** | `:223` / `:224`；先例 D-13。⇒ `RE_MATCHES_FULL`、`ARRAY_GET`、`PATH_CANONICAL`、`std.text::NFKC`、`std.fs::WRITE_FILE_ATOMIC`…全部是**新名** |

---

## 2 事实坐标与已核实证据

> 每条可复查。「已核实」附 `文件:行号`；「已实测」附命令。⚠ **规范侧坐标是本轮实测出来的**——早前那份勘察报告给的行号有偏差，本节坐标以 `grep` 实读为准。

### 2.1 基线

| 项 | 值 | 坐标 |
|---|---|---|
| 实现仓 HEAD | `9bd0f46`（`Delete wlwl-v0.12.1-build-plan.md`） | `git log --oneline -1` |
| 实现仓版本号 | `0.12.0` | `impl/Cargo.toml` `[workspace.package] version` |
| 规范仓 HEAD / tag | `eb5e7a8` / `v1.0.3` | 规范仓 `git log --oneline -1` / `git tag` |
| 规范仓 CI 钉死提交 | `408e1f03d77a25aa8eecbefacdde109af35f54e4` | `impl/../.github/workflows/ci.yml:121` |
| 公共面基线 | **21 个命名空间 / 231 个成员** | `docs/wlwl-spec-reference.md:6618-6642`（附录 A 抄件）；实现侧登记表 `impl/crates/wlwl-eval/src/stdlib_mirror.rs:84-282` |
| 实现侧 probe 用例 | **181** 个 case 目录 | `impl/tests/probe/cases/`；门禁常量 `impl/crates/wlwl-cli/tests/probe.rs:119` |
| 命名空间解析唯一入口 | `resolve(path)` | `impl/crates/wlwl-std/src/lib.rs:423-487` |
| R1（纯 wlwl 源码模块）登记表 | `LANG_SOURCES`（5 项） | `impl/crates/wlwl-std/src/lib.rs:175-366` |
| R1→R2 私有注入通道 | `StdSource::kernels` | `impl/crates/wlwl-std/src/lib.rs:142`；注入点 `impl/crates/wlwl-eval/src/lib.rs:1052-1060` |
| 原生成员函数签名 | `StdFn = fn(&mut dyn StdHost, Vec<Value>) -> Result<Outcome, WlwlError>` | `impl/crates/wlwl-value/src/lib.rs:2588` |

### 2.2 规范侧硬约束（本轮所有裁决的依据）

| 编号 | 约束 | 坐标 |
|---|---|---|
| **N-1** | **修订版本（1.0.x）可以**新增**非破坏性的成员 / 命名空间扩充**——**条件**：该扩充在总纲 §0.9 差异表有登记行，**且**发布说明显式列出 | 一致性册 `:130` |
| **N-2** | **次版本（1.1.0）** 用于「新增能力（新章、新成员、新诊断码）」 | 一致性册 `:129` |
| **N-3** | ⚠ **新诊断码在 1.0.x 无通道** ⇒ 本轮**不得**新增诊断码 | 一致性册 `:129`（对照 `:130`） |
| **N-3b** | ⚠ **`wlwl check` 的产出必须进诊断流**：「`lsp` 的 `diagnostics` 与 `wlwl check` **共用同一个收集函数**……编辑器里的划线**不会比命令行少**」⇒ **静态检查不能用「无码提示」绕开新诊断码** | 语言册 `:1681`（§11 工具链诊断口径） |
| **N-4** | **不得**修改任一既有成员的签名或可观察语义（§3.4 三条例外除外） | 一致性册 `:223` |
| **N-5** | **不得**移除或弃用既有成员（弃用走两步 + 主版本递增） | 一致性册 `:224` |
| **N-6** | §3.4 三条例外：① 安全修复 ② **规范缺陷修复**（且修复方向**不引入新的自由度**）③ 勘误（不改变任何可观察行为） | 一致性册 `:142-146` |
| **N-7** | 每次例外**必须**写明：受影响条款号 / 为什么落在例外内 / 迁移写法 | 一致性册 `:148` |
| **N-8** | ⚠ **§4.3 与 §3.2 正面不一致**：`:222` 写「新增成员、新增命名空间、新增诊断码：**允许**，**次版本递增**」；`:130` 写修订版本可含新增成员 ⇒ **新增成员必须显式引用 D-13 先例**（总纲 §0.9 的 D-13 行，17 个新公共面成员走 v1.0.2 修订版），否则审阅者会拿 `:222` 打回 | 一致性册 `:222` vs `:130`；总纲 §0.9 **D-13** |
| **N-9** | 发布前置四闸门：① 总纲 §0.9「实现状态」列**无**「未实现/部分」；② 第 6 章验证要求满足且可复现；③ 公共面清单与附录 A **逐项一致**；④ 一致性册/总纲/各册间**无悬空跨册引用** | 一致性册 `:152-157` |
| **N-10** | 每次规范变更**必须**更新总纲 §0.9 差异表；新增册**必须**同步总纲 §0.1 册表与 §0.6 规范矩阵 | 一致性册 `:694-696` |
| **N-11** | **§5.5 实现侧扩展台账**：① 不属公共面；② 效力低于附录 A；③ **必须随实现仓发布**，写明登记理由 / 收敛路线 / 拟进公共面的版本；④ 实现侧门禁按「**附录 A ∪ 台账**」判集合相等；⑤ 台账条目**不得**被表述为「规范已支持」 | 一致性册 `:514-532`（表体 `:528`，本版为空 `:530`） |
| **N-12** | 并发册 §17.0：宿主挂起点成员（`TCP_*` 族 / `SLEEP_ASYNC`）在**任务体外（模块顶层，或任何不在任务体里的位置）** ⇒ **必须** `E0014`，且诊断**必须**① **指名该成员** ② **位置指向调用点**，**不得**报成「`YIELD` 出现在任务体外」 | 并发册 `:94`；通道族另有 `E0053`（`:96`） |
| **N-13** | ⚠ **语言册与并发册正面矛盾**：语言册说 `E0014` 在并发下「**只**用于一个位置：`YIELD` 在任务体外」 | 语言册 `:1145` / `:1230` **vs** 并发册 `:94` |
| **N-14** | 并发册 §17.0.1：任务之间**不得**通过**共享的可变 cell** 传数据；正确形态是 **actor**；⚠ 「`MUTEX`/`CONDITION`/`ONCE`/`SEMAPHORE`/`WAIT_GROUP` **不**属于本语言面；**它们属标准库范畴**，见标准库册」；⚠ 「在当前架构下，『锁』不是一个可实现的成员」 | 并发册 `:147` / `:153` / `:157` |
| **N-15** | 系统册 §21.1/§21.4：**只支持明文 `http:`，不做 TLS**；`https://` 返回 `ERR`，不做「简易封装」；明确不做 TLS/WebSocket/SSE/gRPC/连接池/重试/自动重定向/代理/cookie jar/流式响应体 | 系统册 `:681` / `:750` / `:779` |
| **N-16** | 系统册 §21.6.5：原生层**只搬字节** ⇒ **HTTP 的解析与响应构造是「调用方」（WLWL 侧）的事**；且这条是**规范性**的——传输层**不得**把协议知识渗进原生层 | 系统册 `:843` / `:844` |
| **N-17** | 系统册 §21.4：⚠ **`HTTP_*` 是阻塞原语**（执行期间整个 `SCOPE` 冻结）；⚠ **阻塞形态是带条件冻结的**——冻结前提是「真 state-machine 续跑仍未落地」，前提一旦不成立**必须重新裁决** | 系统册 `:776` / `:777` |
| **N-18** | 系统册 §21.6.2 正常值表：`TCP_ACCEPT` 超时内无待接连接 ⇒ `NULL`；`TCP_RECV` 超时内无可读字节 ⇒ `{bytes: [], text: "", eof: FALSE}`。⚠ **竞争落选未被覆盖**（规范空白） | 系统册 `:813` / `:814` |
| **N-19** | 文本册 §16 **实现注**：正则模式**可以**缓存编译结果 ⇒ **缓存已被允许**，无需改规范 | 文本册 `:502` |
| **N-20** | 文本册 `RE_TEST` 契约 = **整串**匹配（两端锚定、忽略 `(?m)`） | 文本册 `:449` |
| **N-21** | 安全册：对称 / 非对称加密**不做**（理由：给了 AES 就会有人自己攒 TLS） | 安全册 `:306` |
| **N-22** | 附录 A 是**冻结快照**，不是活镜像；实现侧与镜像不一致 ⇒ 走 §5.4 差异登记，**不**由镜像跟随实现漂移；镜像与各册成员表**必须**集合相等 | 注册镜像册 `:69-78`；一致性册 `:206` |
| **N-23** | 每次 §5.4 新增一行 ⇒ `script/conformance_registry_map.json` **必须**同步加一条，否则门禁**当场红** | 一致性册 `:588-589` |

### 2.3 已实测读数（本轮）

| # | 事实 | 复现命令 |
|---|---|---|
| **R-1** | `std.net::HTTP_GET("https://…")` ⇒ `ERR([kind:"HttpError", reason:"scheme \"https\" is not supported — plaintext http: only (ADR-0028 §0)"])` | `impl/crates/wlwl-std/src/net.rs:830-841`（`:836` 是那句文案） |
| **R-2** | chunked 响应被**拒绝**（不解码），错误文案含 "chunked is a smuggling vector, refused rather than decoded (ADR-0028 D3)" | `impl/crates/wlwl-std/src/net.rs:1082-1087` |
| **R-3** | `std.net` 共 **14** 员（2 URL + 3 HTTP + 9 TCP）；`std.net` 全部 R2、零第三方依赖 | `impl/crates/wlwl-std/src/net.rs:56-83`；`impl/crates/wlwl-std/src/lib.rs:563-566` |
| **R-4** | `HTTP_*` 返回裸 `DICT` `{status, headers, body}`，**无** Response/Request 构造器；`body` 形状随内容走（合法 UTF-8 ⇒ `STRING`，否则 `ARRAY[INTEGER]`） | `impl/crates/wlwl-std/src/net.rs:1211-1223`、`:1199-1209` |
| **R-5** | 全仓**没有** Response 构造器、cookie 助手、HTTP 请求解析器 | grep `cookie` 于 `impl/crates/wlwl-std/src` ⇒ 0 命中 |
| **R-6** | `std.fs` 共 **18** 员；**无** mtime / 原子写 / 文件锁；删除成员叫 `REMOVE` 不叫 `DELETE` | `impl/crates/wlwl-std/src/fs.rs:70-94`；`WRITE_FILE` 是裸 `std::fs::write`（`:153`） |
| **R-7** | `PATH_JOIN` 把 `\` **也**当分隔符（`fs.rs:277`）；`PATH_NORMALIZE` **只**认 `/`（`fs.rs:366`）⇒ 二者在 Windows 上**不互逆**（H-4 的根因） | `impl/crates/wlwl-std/src/fs.rs:277` / `:366` |
| **R-8** | `std.text` 8 员（**无** `SPLIT`、**无** NFKC）；`std.str` 8 员（**无** `SPLIT`）；但**全局内建** `STR`（`registry.rs:363-373`）与 `SPLIT`（`registry.rs:1096-1106`）**已存在** ⇒ H-2 / H-3 无需新增成员 | `impl/crates/wlwl-std/src/text.rs:45-59`；`impl/crates/wlwl-std/wl/std/str.wll:203`；`impl/crates/wlwl-eval/src/registry.rs:363` / `:1097` |
| **R-9** | `+` **不做**隐式数值→字符串强制转换 ⇒ `+("Content-Length: ", 8579)` ⇒ `E0030: +: cannot add string and integer`；变长拼接只能用 `std.str::CAT` | `impl/crates/wlwl-eval/src/lib.rs:6784-6789`（`E0030` 发射点 `:6638-6648`） |
| **R-10** | `std.process` 仅 2 员（`PROCESS_RUN` / `PROCESS_ID`），**阻塞**（等子进程期间不调度任何其它任务）；全仓**无任何信号处理**（`SIGINT`/`SIGTERM`/`ctrlc`/`SetConsoleCtrlHandler` ⇒ 0 命中）；`libc` 只用于 `getpgrp` / `killpg`（`cfg(unix)`），Windows 侧用 `taskkill /T /F` | `impl/crates/wlwl-std/src/process.rs:36-42`、`:173-199`、`:223-231` |
| **R-11** | `E0092` = 「TLS handshake / certificate error」**已注册**为原生诊断码，但仅 `real-ai` 路径可达 | `impl/crates/wlwl-error/src/lib.rs:191`；`impl/crates/wlwl-std/src/ai.rs:284-292` |
| **R-12** | `ERR` 的 `kind` 是**开放字符串**（无枚举 / 无注册表）⇒ 新增 `kind` 结构上零成本，但**也没有任何自动校验**提示是否与规范冲突 | 构造器样板：`net.rs:167-176`（`HttpError`）、`tcp.rs:200-209`（`SocketError`） |
| **R-13** | `rustls 0.23.46` / `ring 0.17.14` / `reqwest 0.12.28` / `tokio 1.54.1` **已在** `impl/Cargo.lock`（经 `wlwl-std` 的 `real-ai` 可选 feature 引入）⇒ TLS 库**不需要新增第三方 crate 之外的东西** | `impl/Cargo.lock`；`impl/crates/wlwl-std/Cargo.toml:31`（`reqwest` 特性 `rustls-tls`） |
| **R-14** | ⚠ **全仓没有任何动态库 / FFI / 插件机制**：`dlopen` / `LoadLibrary` / `libloading` / `cdylib` / `#[no_mangle]` ⇒ **0 命中**；`libloading` 不在 `Cargo.lock` ⇒ 扩展面**必须从零建** | grep 全仓（`impl/crates/**`）；`impl/crates/wlwl-std/src/tcp.rs:6` 注释亦自陈「语言侧没有 socket、没有 FFI、没有插件加载机制」 |
| **R-15** | 原生成员是**编译期静态函数指针**（`StdFn`），`ModuleSpec.functions` 是 `&'static` ⇒ **动态符号装不进现有登记表** ⇒ 需要一层「运行期成员注册表」 | `impl/crates/wlwl-value/src/lib.rs:2588`；`impl/crates/wlwl-std/src/lib.rs:113-116` |
| **R-16** | 命名空间**注册要改三处静态表**（`LANG_SOURCES` / `resolve()` / `NATIVE_SPECS`），且 `stdlib_mirror::NAMESPACE_META` 与实现真相**双向**对账（漏登记 / 多登记都红） | `impl/crates/wlwl-std/src/lib.rs:175-366` / `:423-487` / `:542-567`；`impl/crates/wlwl-eval/src/stdlib_mirror.rs:84-282`、`:384-416` |
| **R-17** | 附录 A 门禁是**逐字**比对（不是集合比对），含**成员顺序**与「层」「v1.0 册」两列 ⇒ 新成员必须插到规范表里的位置 | `impl/crates/wlwl-eval/src/bin/check_appendix_a.rs:39-41`、`:94`、`:103-116` |
| **R-18** | probe 夹具数是**硬门禁常量** = 181；`expect.json` 的键必须在 `KNOWN_KEYS`（7 个）内 | `impl/crates/wlwl-cli/tests/probe.rs:119`、`:130-138` |
| **R-19** | CI 门禁：fmt / clippy / doc / cargo-deny / conformance / 全量测试（flaky **不放行**，`FLAKY_EXIT_CODE=1`）/ §5.4 双向门禁 / **附录 A 逐字对拍** / `real-ai` feature 编译 / bench 冒烟 | `.github/workflows/ci.yml:223` / `:232` / `:249` / `:270` / `:287` / `:329`（`:350`）/ `:582` / `:607` / `:620` / `:631` |
| **R-20** | ⚠ **需求清单引用的 9 个探针不在仓内**：`docs/wlwl-feasibility/` **不存在**（`Test-Path` ⇒ `False`）；清单 §J-3 声称「已开源在目标仓库」⇒ **不成立** | `Test-Path D:\Project\wlwl\docs\wlwl-feasibility`；清单 `:418` |
| **R-21** | ⚠ **死链三处**：`CHANGELOG.md:4`、`docs/plan/README.md:7`、`docs/plan/README.md:52` 仍指向被删的 `wlwl-v0.12.1-build-plan.md` | grep `wlwl-v0.12.1-build-plan` 于两文件 |
| **R-22** | 规范侧坐标校正：并发册 `:94`（`E0014` 条款原文）**已经**覆盖「嵌套闭包内」；语言册 `:1145` / `:1230` 才是那句「只用于 `YIELD`」 | grep `E0014` 于 `wlwl-v1.0-language.md` / `wlwl-v1.0-concurrency.md` |
| **R-23** | 需求编号实数 = **57**（A 5 + B 7 + C 12 + D 7 + E 7 + F 4 + G 4 + H 6 + I 5）；清单页眉的「60 条」是**多计** | `Select-String -Path docs/plan/wlwl-迁移需求清单.md -Pattern '^### ([A-I]-\d+)'` ⇒ 46 条（`###` 形态）+ H/I 两组表格 11 条 |
| **R-24** | ⚠ **B-1 的原始前提在当前代码上复现不出来**：任务体内调用**嵌套闭包**里的挂起点，**不**静默终止（`AWAIT` 拿到 `42`，exit 0）；可观察到的是**段首重放**（handler 体被重跑 3 次 ⇒ **副作用重复**） | 用仓内已构建的 `impl/target/release/wlwl.exe` 跑 5 个重构探针（M-1…M-5）；探针写在 `%TEMP%\wlwl-probe-b1\`，**仓库零改动** |
| **R-25** | 「裸 `NULL` + stderr 全空」的真实来源是**兜底臂**：`run_one_task` 把「非四种 park 信号」的 outcome 直接结成 `TaskResult::Ok(o.value)`，而原生成员挂起时带的 value 恰是 `Value::Null` ⇒ **丢失的挂起信号会伪装成「正常返回 NULL」** | `impl/crates/wlwl-eval/src/lib.rs:10651-10684`（兜底臂）；挂起 value 见 `impl/crates/wlwl-std/src/tcp.rs:620-623` |
| **R-26** | `E0014` **只有一条产生路径**：「挂起信号逃到模块顶层」（`eval` 收尾 `:8591-8600` / `eval_module` `:8881-8890`）⇒ **任务体内**的挂起点永远到不了 `E0014` | `impl/crates/wlwl-eval/src/lib.rs:8591-8600`、`:8881-8890`；文案合成 `yield_outside_task` `:10235` |
| **R-27** | 「我在任务体内吗」**只有一个布尔标志** `Evaluator::current_task: Option<TaskId>`（置位 `:10484` / 还原 `:10501`）⇒ **没有嵌套深度、没有词法位置** ⇒ 并发册 `:94` 要求的「调用点必须在任务体内」这条检查**当前根本没写** | `impl/crates/wlwl-eval/src/lib.rs:7990`、`:10484`、`:10501` |
| **R-28** | **B-2 的惊群是结构性的**：一个 listener 只有**一个 socket id** ⇒ 所有 acceptor 共享同一把等待者 `Vec` ⇒ `wake_socket_waiter` **唤醒全部**；落选者拿到与超时**同形**的 `NULL` | `impl/crates/wlwl-eval/src/runtime.rs:319`（`socket_waiters: HashMap<i64, Vec<TaskId>>`）、`:973-1006`（唤醒全部）；`impl/crates/wlwl-std/src/tcp.rs:600`（超时）vs `:606`（落选） |
| **R-29** | 落选者的**真实死法**不是「静默 NULL」，而是调用方拿 `NULL` 去 `TCP_RECV` ⇒ **`E0030: TCP_RECV: expected integer, got null`** ⇒ 任务失败 ⇒ `SCOPE` 提前退出 ⇒ 监听句柄随进程消失 ⇒ 客户端 RST | `impl/crates/wlwl-std/src/tcp.rs:697`（`int_arg`）；`impl/crates/wlwl-eval/src/lib.rs:11131`（`pop_scope`） |
| **R-30** | 并发夹具里**没有任何 socket/TCP 用例**，也**没有**「嵌套闭包挂起点」「多 acceptor」用例 ⇒ B-1/B-2 的验收用例**必须新建**；socket 用例受 `impl/crates/wlwl-eval/tests/common/handle_guard.rs` 的**串行闸**约束 | `impl/tests/concurrency/`（15 个 `.wll`）；`impl/crates/wlwl-cli/tests/concurrency.rs:55`（`WLT_FILES`）；`handle_guard.rs:63-78` |
| **R-31** | 语言面**今天没有任何同步原语**：全仓 `Mutex`/`RwLock`/`Condvar`/`Atomic` 命中**全部是 Rust 内部细节**（`tcp.rs` 的 socket 状态、`net.rs:1397` 的 cookie jar）或**仅测试** ⇒ 新增 `std.sync` 是从零开始 | `impl/crates/wlwl-std/src/lib.rs:542-567`（`NATIVE_SPECS` 里**没有** `sync`） |
| **R-32** | `Task` **没有启动时刻字段** ⇒ F-3 的「已运行时长」需要**新加字段**（`resume_hint` / `deadline_ms` 已有） | `impl/crates/wlwl-eval/src/task.rs:50`（`Task`）、`:184`（`resume_hint`）、`:166`（`deadline_ms`） |
| **R-33** | 就绪集合 `READY` 是**进程级全局**（`static`）⇒ 同进程两个求值器互见事件；⚠ `pending_ready` 这条「事件先于 park 到达」的补偿在当前 pump 路径下**不可达**（读码判读，**未定向实测**） | `impl/crates/wlwl-std/src/tcp.rs:125`；`impl/crates/wlwl-eval/src/runtime.rs:329`、`:974-977`、`:10908-10916` |

### 2.4 需求清单的 11 条事实前提已被订正（照单全收会做无用功或做错方向）

| 需求 | 清单说的 | 实测 | 处置 |
|---|---|---|---|
| H-1 | `IF` 第三实参不支持多语句组 | **已支持** | 只补回归守护（W-34） |
| H-2 | 没有数字→字符串 | **全局内建 `STR` 已有** | 不新增成员，改文档 / 技能包（W-34） |
| H-3 | 没有通用 `SPLIT` | **全局内建 `SPLIT` 已有** | 同上 |
| H-4 | 路径分隔符不统一 | 属实（R-7） | **不改** `PATH_NORMALIZE`（撞 `:223`）⇒ **新增** `PATH_CANONICAL`（W-24） |
| H-5 | `RE_TEST` 名字误导 | 属实（N-20） | **不改名**（撞 `:224`）⇒ **新增** `RE_MATCHES_FULL`（W-29） |
| H-6 | 容器取值 API 不统一 | 属实 | **不统一既有** ⇒ **新增** `ARRAY_GET` + 错误提示（W-29） |
| B-1 | 嵌套闭包挂起点 ⇒ 任务静默终止 | ⚠ **前提部分不成立**（R-24）：当前代码**不静默终止**，可观察的是**段首重放 + 副作用重复**；但并发册 `:94` 要求的「调用点必须在任务体内 ⇒ `E0014`」这条检查**根本没写**（R-27）⇒ **实现侧今天不合规** | W-10 改成「**新增词法检查**」而非「补一条丢失的诊断」；验收按**新语义**写 |
| B-2 | 多 acceptor 落选者拿 `NULL` 并直接死 | ⚠ **死法说错了**（R-29）：不是「静默 NULL」，是调用方 `TCP_RECV(NULL)` ⇒ `E0030`；惊群本身**坐实**（R-28） | W-11 改用「**唤醒一个**」修根因 ⇒ 落选者**不再拿到 NULL**，规范侧大概率**零改动**（§6.1 SP-2 降为条件项） |
| B-6 | 任务终值不可区分 | ⚠ 规范 §17.9 **已覆盖**；实现的**三态**（`Ok`/`ErrValue`/`Failed`/`Cancelled`）**已具备**（`lib.rs:4734-4763`）⇒ 需求描述的「裸 NULL 不可区分」不成立；**但**兜底臂（`lib.rs:10651-10684`）会把**丢失的挂起信号伪装成正常 NULL**（R-25）⇒ 那是**真缺陷** | W-15：修兜底臂（**不改** `AWAIT` 既有语义，撞 `:223`）+ 补三态契约用例 |
| D-1 | `RE_*` 每次重编译 | 属实 | **实现侧内部缓存**（零新成员、零规范改动，N-19）⇒ W-23 |

---

## 3 架构决策：TLS 以 Rust 原生动态库交付

> 这是本轮最大的架构判断，故单独成章。**这一章的每个结论都带坐标**；执行者不需要回头翻需求清单。

### 3.1 为什么 TLS 不能进标准库（六条证据）

| # | 证据 | 坐标 |
|---|---|---|
| 1 | `std.net` 明写「**只支持明文 `http:`，不做 TLS**」，`https://` 返回 `ERR`，「不做简易封装」 | 系统册 `:750`、`:681` |
| 2 | 系统册「明确不做」清单第一条就是 **TLS**（与 WebSocket / SSE / gRPC / cookie jar 并列） | 系统册 `:779` |
| 3 | 安全册明写「**对称 / 非对称加密不做**」（理由：给了 AES 就会有人自己攒 TLS）⇒ A-4 的加密原语同理不能进语言面 | 安全册 `:306` |
| 4 | `ADR-0028 D3`：「明文 `http:` + 手写 HTTP/1.1 最小子集，**零新增依赖**」；`ADR-0028 §5` 把「**顺手加 TLS**」列为**明确排除**的选项，理由是「TLS 实现不是能顺手写对的东西」 | `docs/adr/0028-std-net-shape.md:93-97`、`:208` |
| 5 | `ADR-0022` 的**零第三方依赖画像**是 `std.net` 的设计依据；引入 `rustls` ⇒ 发行二进制依赖面变化 ⇒ 破 ADR | `docs/adr/0028-std-net-shape.md:203` |
| 6 | TLS 所需的对称/非对称原语（X.509、RSA/ECDSA/Ed25519 验签、AES-GCM、HKDF、SHA-2 家族）在 `std.encode` 里**一个都没有**（现有 16 员只有 SHA256 / HMAC_SHA256 / PBKDF2_ITER / ARGON2ID / RANDOM_* / TIMING_SAFE_EQ / 各种编解码）⇒ TLS **不可能**用现有语言面拼出来 | `impl/crates/wlwl-std/src/encode.rs:44-73` |

⇒ **结论**：TLS 既不能进 `std.net`（1、4、5），也不能进 `std.encode`（3、6），更不能由 wlwl 程序自行拼出（6）。
⇒ **唯一既满足需求、又不破任何既有裁决的形态**：**独立编译的 Rust `cdylib`**，由宿主在**运行时显式授权**加载，其成员落在**一致性册 §5.5 实现侧扩展台账**（不进公共面 ⇒ 不动附录 A ⇒ 规则侧改动最小）。

### 3.2 交付形态（三段式）

```
┌─ 发行包 ────────────────────────────────────────────────────────────┐
│  wlwl.exe / wlwl            ← 依赖面**不变**（零新增第三方依赖）      │
│  wlwl-tls.dll / .so         ← **新增**：Rust cdylib，自持 rustls      │
│    └─ 实现 tls / crypto 两个命名空间的成员（§3.6）                    │
└──────────────────────────────────────────────────────────────────────┘
        ▲ 运行时显式加载（CLI --ext / WLWL_EXT_PATH，§3.4）
        │
┌─ 宿主侧（wlwl 实现仓内） ────────────────────────────────────────────┐
│  加载器：raw FFI（Windows LoadLibraryExW / POSIX dlopen）            │
│          + ABI 版本校验 + 路径门控 + 哈希钉扎                        │
│  运行期成员注册表：把扩展声明的成员挂成可 IMPORT 的命名空间           │
│  注入通道：扩展成员可作为 kernel 注入 R1 模块（复用现有 kernels 机制） │
└──────────────────────────────────────────────────────────────────────┘
```

**为什么加载器自己手写 raw FFI、而不引 `libloading`**：仓内对依赖面收窄有明确纪律（`impl/Cargo.toml` 里 `argon2` / `getrandom` / `zstd` / `flate2` 每条都写了收窄理由），`libloading` 是新第三方 crate ⇒ 用 `extern "system" { fn LoadLibraryExW(...) }`（`#[link(name = "kernel32")]`）+ `extern "C" { fn dlopen(...) }`（`#[link(name = "dl")]`）**零新增依赖**（与 `impl/crates/wlwl-eval/tests/common/handle_guard.rs:96-100` 的既有手写 FFI 做法同款）。

### 3.3 ABI 契约（v1）

**导出符号**：扩展导出**唯一**一个符号 `wlwl_ext_v1`（一个 `#[repr(C)]` vtable 指针）。

```c
/* 值编码：JSON 文本（宿主与扩展各用各的 serde_json 均可） */
typedef struct wlwl_ext_v1 {
    uint32_t abi_version;      /* 必须 == 1；不匹配 ⇒ 宿主拒绝加载 */
    uint32_t struct_size;      /* 本结构大小，宿主按它读，前向兼容 */
    const char* host_version;   /* 宿主版本串，扩展可据此拒绝 */
    /* 命名空间声明（一个扩展可声明多个） */
    uint32_t ns_count;
    const wlwl_ext_ns_t* namespaces;   /* { path, member_count, members } */
    /* 宿主提供的回调（扩展调用宿主） */
    wlwl_ext_host_t* host;
} wlwl_ext_v1_t;

typedef struct wlwl_ext_member {
    const char* name;
    uint32_t arity_min;
    uint32_t arity_max;         /* 0xFFFFFFFF = 变参 */
    /* 调用约定：args_json（入）→ out_json（出）；返回码 0 = OK */
    int32_t (*call)(wlwl_ext_ctx_t* ctx, const char* args_json, char** out_json);
} wlwl_ext_member_t;

/* 宿主回调表（扩展 → 宿主） */
typedef struct wlwl_ext_host {
    /* 宿主分配/释放：扩展返回的字符串一律用宿主给的分配器，
       否则 Windows 上跨 CRT 释放会炸 —— 这条不可省 */
    void*  (*alloc)(uint32_t size);
    void   (*free)(void* ptr);
    /* 扩展 → 宿主的错误上报（可失败；失败不致命） */
    void   (*report)(wlwl_ext_ctx_t* ctx, const char* kind, const char* reason);
    /* 扩展需要宿主调度器挂起时用（可选，见 §3.3 的就绪模型） */
    int32_t (*await_ready)(wlwl_ext_ctx_t* ctx, uint32_t handle, uint32_t timeout_ms);
} wlwl_ext_host_t;
```

**就绪模型（关键）**：TLS 的握手与读写是**阻塞 I/O**，而 wlwl 是单线程协作式调度 ⇒ 扩展**必须**像 `std.net` 的 TCP 族那样做：每连接一个后台线程做阻塞 I/O，宿主用**就绪查询**驱动挂起/唤醒。⇒ 扩展成员需提供 `poll(handle) -> READY|BLOCKED|CLOSED`，宿主把它接到现有的挂起点机制上（`Signal::Yield(YieldReason::SocketReady(..))` 的同款）。

**错误通道（与仓内约定一致）**：
- **运行期可预期失败**（证书过期 / 私钥不匹配 / 握手失败 / 对端关闭）⇒ 扩展返回 `{"err": {"kind": "TlsError", "op": "...", "reason": "..."}}` ⇒ 宿主转成 **`ERR` 值**（**不**是原生诊断 ⇒ **不中止运行**、**不新增诊断码**）。
- **程序员错误**（元数 / 类型错）⇒ 宿主在调用前按 vtable 的 `arity_min/arity_max` 校验 ⇒ 报**既有** `E0022` / `E0030`。
- **扩展 ABI 违规**（版本不匹配 / 导出符号缺失 / 命名空间试图占用 `std.*`）⇒ **宿主侧 CLI 级失败**：stderr 一行 + 非零退出码（**不是语言诊断** ⇒ 不占诊断码）。

### 3.4 信任模型（七条硬规则；**不可让渡**）

| # | 规则 | 理由 |
|---|---|---|
| **T1** | **加载只能由宿主发起**：CLI `--ext <path>` 或环境变量 `WLWL_EXT_PATH`。**语言面不提供任何 `LOAD` 成员** | 程序能指定路径 ⇒ 脚本即任意代码执行（ast-grep `GHSA-cmvf-x8f7-rvp6` 的同款形状：项目配置里一条路径 ⇒ 任意代码执行，且 `dlopen` 的**构造器在符号校验之前就执行**） |
| **T2** | **绝不从项目配置 / 项目目录自动发现扩展**：`wlwl.toml`、程序同目录、`./ext/` 一律**不**扫描 | 同 T1 |
| **T3** | **路径必须是绝对路径**，且必须存在、是普通文件 | 相对路径 = 依赖 CWD = 可被工作目录劫持 |
| **T4** | **可选哈希钉扎**：`--ext-sha256 <hex>`；给了就必须匹配（发行包自带扩展时，发行说明给出期望哈希） | 防「替换文件」 |
| **T5** | **扩展不得占用 `std.*` 命名空间**，也不得覆盖任何既有命名空间 ⇒ 宿主侧拒绝并报 CLI 级错误 | 防「劫持标准库」 |
| **T6** | **加载 = 交出全部进程权限**。宿主在加载前打印一行醒目告警；扩展文档必须写明这一点；**不得**宣称沙箱 | 事实陈述，不是缓解 |
| **T7** | **扩展成员在语言面只以「运行期注册」出现**：不进附录 A、不进公共面清单、只进 §5.5 台账；`wlwl namespace` 之类的工具**不得**把它列进「公共面」（§5.5 规则 5） | 规则侧改动最小化 |

### 3.5 命名空间解析顺序与防影子

```
resolve(path) 的最终顺序：
  1. wlwl:std.*            → 静态表（永远第一，扩展无法影子）
  2. 官方包（ai / agent / …）→ 静态表
  3. wlwl.toml 声明的包      → 磁盘 R1 模块
  4. ★ 运行期扩展注册表      → 扩展声明的命名空间（新增层）
  5. 都不中 → E0040 module 'X' not found
```

⇒ 扩展**永远无法**影子标准库成员（T5 的运行时兑现）。

### 3.6 TLS / crypto 成员面（A-1…A-5 的落点）

**`tls` 命名空间**（流原语 + 证书）：

| 成员 | 语义 | 失败口径 |
|---|---|---|
| `tls::LISTEN(host, port, opts) -> INTEGER` | 起 TLS 监听；`opts` = `["cert": path, "key": path, "ca": path?, "min_version": "1.2"|"1.3", "alpn": [..]?, "sni": {host: cert}?]`；`port=0` ⇒ 临时端口 | 证书缺失 / 私钥不匹配 ⇒ `ERR(kind:"TlsError")` |
| `tls::ACCEPT(listener, timeout_ms) -> INTEGER` | 接受一条 **TLS** 连接（内部完成握手）；超时 ⇒ `NULL`（正常值，与 `TCP_ACCEPT` 同形） | 同上 |
| `tls::CONNECT(host, port, opts) -> INTEGER` | **出站** TLS 连接（客户端）；`opts` 含 `ca` / `pin`（SPKI 摘要）/ `client_cert` / `min_version` | 证书不受信 / 过期 / 固定不匹配 ⇒ `ERR(kind:"TlsError", reason:"certificate expired" 等)` |
| `tls::SEND(handle, bytes) -> INTEGER` | 写 | `ERR(kind:"TlsError")` |
| `tls::RECV(handle, max_bytes, timeout_ms) -> DICT` | 读 ⇒ `{bytes, text, eof}`（与 `TCP_RECV` 同形） | 同上 |
| `tls::CLOSE(handle) -> NULL` | 关 | — |
| `tls::PEER(handle) -> DICT` | 对端地址 + 证书摘要 | — |
| `tls::CERT_FINGERPRINT(path) -> STRING` | **SPKI** SHA-256 十六进制（用于 pinning）⇒ **A-3** | 读不到 ⇒ `ERR` |
| `tls::CERT_INFO(path) -> DICT` | `{subject, issuer, san[], not_before, not_after, spki_sha256, serial}` ⇒ **A-3** | 同上 |

**`crypto` 命名空间**（**A-4** 的加密原语，全部自持在扩展里）：

| 成员 | 语义 |
|---|---|
| `crypto::SHA384(bytes)` / `crypto::SHA512(bytes)` | 摘要 |
| `crypto::HMAC_SHA512(key, bytes)` | MAC |
| `crypto::HKDF(ikm, salt, info, length, algorithm)` | 密钥派生（TLS 1.3 用） |
| `crypto::AES_256_GCM(key, nonce, aad, plaintext) -> DICT{ciphertext, tag}` | AEAD |
| `crypto::ED25519_VERIFY(pubkey, message, signature) -> BOOLEAN` | 验签 |
| `crypto::ECDSA_P256_VERIFY(pubkey, message, signature) -> BOOLEAN` | 验签 |
| `crypto::RSA_PSS_VERIFY(pubkey, message, signature) -> BOOLEAN` | 验签 |
| `crypto::RANDOM_BYTES(n)` | CSPRNG（宿主已有 `std.encode::RANDOM_BYTES`，扩展侧不重复暴露） |

⚠ **A-5 的安全默认值**（证书校验**默认开**、TLS 下限 1.2、无「一键全关」开关、私钥不得出现在错误消息 / 日志里）落在**扩展实现**内，并有守卫用例（W-14 的反向守卫）。

### 3.7 HTTPS 出站与 `std.net` 的边界（A-2 的偏差说明）

**问题**：需求 A-2 的验收判据写的是「`HTTP_GET("https://example.com")` → 返回响应」。但系统册 `:750` 明写「`https://` 返回 `ERR`」，而 `:223` **禁止**在 1.0.x 修改既有成员的可观察语义 ⇒ **让 `std.net::HTTP_GET` 支持 https 会让实现不合规**。

**处置**（**能力达成、判据改名**）：

| 需求写法 | 本轮交付 | 为什么不是 `std.net::HTTP_GET` |
|---|---|---|
| `HTTP_GET("https://example.com")` | **`std.http::GET("https://example.com")`**（新命名空间 §4 W-16） | `std.net::HTTP_GET` 的 `https://` ⇒ `ERR` 是**规范要求**（系统册 `:750`），改它违反 `:223` |
| 「对过期证书的自签名站点 ⇒ `ERR(kind:"TlsError", reason:"certificate expired")`」 | 逐字达成（`std.http::GET` 透传扩展的 `TlsError`） | — |
| 「关闭校验开关必须显式传入」 | 达成：`std.http::GET(url, ["tls": ["verify": FALSE]])`，**且**每次关闭都打一行告警到 stderr（A-5） | — |

⚠ 这条偏差**必须**在交付回执里对需求方写明，并给出「若要 `std.net::HTTP_GET` 原生支持 https ⇒ 需 v1.1.0（改既有成员可观察语义）」的排期口径。

---

## 4 工作项（28 个；按波次）

> 每个工作项：**目标 / 依赖 / 归属文件 / 改动要点 / 验收 / 反向守卫 / 非目标 / 规则侧**。
> ⚠ **归属文件 = 该工作项独占、其它 agent 不得触碰**（§5 的切分依据）。
> ⚠ 每条「验收」都是**可粘贴的命令**。

### 批次 A · 地基与夹具（串行，2 个工作项）

#### W-01 · 基线自洽与死链订正
- **目标**：把「附录 A 抄件 ↔ 规范仓快照 ↔ 实现派生表体」三方拉直；修掉 3 处死链；**核实需求清单里 8 条「已实测」到底还成不成立**。
- **依赖**：无
- **归属文件**：`docs/plan/README.md`、`CHANGELOG.md`、`docs/plan/wlwl-迁移需求清单.md`（只加「订正」批注）
- **改动要点**：① 跑 §8.1 的三条读数命令，**红了先修红**；② 修 `CHANGELOG.md:4` / `docs/plan/README.md:7` / `:52` 的死链（本文件同路径 ⇒ 自动复活）；③ 在需求清单里给 9 个探针补一句「**原文声称已开源，实际未入库；v0.12.1 已固化（见 W-02）**」。
- **验收**：`cargo run --locked --bin check-appendix-a -- ../docs/wlwl-spec-reference.md` ⇒ exit 0；`cargo test --locked -p wlwl-eval --test stdlib_appendix_a_sync` ⇒ 绿；`python ../script/check_conformance_registry.py` ⇒ exit 0
- **反向守卫**：任一命令红了 ⇒ 本工作项不算完
- **非目标**：不改任何实现代码
- **规则侧**：无

#### W-02 · 需求清单的 9 个探针固化进仓
- **目标**：让需求清单里每条「现状」都有**仓内可重跑**的证据（记忆纪律：文档里的「可重跑命令」必须先核实夹具真实存在）。
- **依赖**：无
- **归属文件**：`docs/wlwl-feasibility/probes/**`（新建）、`impl/tests/probe/cases/P121_*`（新建）、`impl/crates/wlwl-cli/tests/probe.rs`（**只改 `EXPECTED_CASE_COUNT` 一行**）
- **改动要点**：把 `p01` / `p03` / `p09` / `p10` / `p11` / `p12` / `p13` / `p02` / `p07` 九个探针按清单附录 A 的语义写成 `.wll` + `expect.json`；探针本体落在 `docs/wlwl-feasibility/probes/`，同款用例落在 `impl/tests/probe/cases/`。
- **验收**：`cargo test --locked -p wlwl-cli --test probe` ⇒ 绿；`EXPECTED_CASE_COUNT` 与目录数一致
- **反向守卫**：把 `EXPECTED_CASE_COUNT` 改回 181 ⇒ 必红
- **非目标**：不改探针的**语义**（只把「文档里的程序」变成「仓里的程序」）
- **规则侧**：无

### 批次 B · 并发模型（6 个工作项 + 第三波新增 W-38）

> ⚠ **依赖**：`ADR-0017 Step 3` **不在本轮**（§1.5 D3）—— 本批六项**都不需要**「挂起后越过阻塞点继续」。

#### W-10 · B-1：宿主挂起点在嵌套闭包内 ⇒ `E0014`（指名成员 + 指向调用点）
- **目标**：消灭「任务静默终止 + stderr 全空 + 客户端只见 RST」。
- **依赖**：无
- **归属文件**：`impl/crates/wlwl-eval/src/runtime.rs`（挂起点上下文的判定）、`impl/crates/wlwl-eval/src/lib.rs`（`E0014` 的抛出点）、`impl/crates/wlwl-std/src/tcp.rs`（`TCP_RECV`/`TCP_ACCEPT` 的调用点捕获）
- **改动要点**：① **新增词法判据**（当前只有布尔标志 `current_task`，R-27）：记录「当前调用栈是否还在 `SPAWN` 闭包的**顶层语句**里」——需要栈帧 + 语句位置；② 当挂起点成员被调用而**当前帧不是任务体顶层** ⇒ **主动造诊断**（不能指望信号上浮：`E0014` 只有「信号逃到模块顶层」这一条产生路径，R-26）；③ 诊断消息**必须含成员名**（`TCP_RECV`），**位置指向调用点**（行+列），**不得**写成「`YIELD` 出现在任务体外」。
- **⚠ 事实订正（必须写进用例注释）**：当前代码里嵌套闭包的挂起点**不静默终止**（R-24），可观察症状是**段首重放 ⇒ 副作用重复**；本项交付后症状从「重复副作用」变成「**明确的 `E0014`**」。**不要**在用例里断言「现在会静默死」。
- **验收**：新增 probe 用例 `P121_b1_nested_suspend_point` ⇒ 期望 `exit 1` + stderr 含 `E0014` + 含 `TCP_RECV` + 位置指向**调用点**；并发套件 `cargo test --locked -p wlwl-cli --test concurrency` ⇒ 绿；⚠ socket 类用例必须走 `handle_guard.rs` 的串行闸（R-30）
- **反向守卫**：把「当前帧不是任务体顶层」的判定去掉 ⇒ 新用例必红（回到段首重放 + 副作用重复）
- **非目标**：不实现「嵌套闭包内挂起点**能**跑通」——本轮选需求清单的 (b)「明确拒绝」
- **规则侧**：**SP-1**（只改语言册两行；并发册 `:94` 已经写对了）

#### W-11 · B-2：accept 竞争落选者**存活**
- **目标**：落选者拿 `NULL` 后**不得**终止任务，**不得**连带关闭监听句柄（今天会 ⇒ `SCOPE` 提前退出 ⇒ 后续连接被拒）。
- **依赖**：无
- **归属文件**：`impl/crates/wlwl-std/src/tcp.rs`（就绪集合与 acceptor 交接）、`impl/crates/wlwl-eval/src/runtime.rs`（任务不因 `NULL` 而终止）
- **改动要点**：**修根因而不是记录症状** —— 惊群的结构性原因是「一个 listener 只有一个 socket id ⇒ 所有 acceptor 共享一把等待者 `Vec` ⇒ 唤醒全部」（R-28）⇒ 改成 **`wake_socket_waiter` 唤醒一个**（FIFO 取第一个，其余留在 `Vec` 里）⇒ **落选者根本不会被唤醒、也就不会拿到 `NULL`** ⇒ 下一条连接到达时（acceptor 线程每次 `push_ready`，`tcp.rs:352`）由**下一个**等待者接手。
  另需保证：① 等待者任务**继续存活**（不得因「没抢到」而终止）；② 监听句柄的生命周期**只**由 `TCP_CLOSE_LISTENER` 决定；③ 调用方拿到 `NULL`（**仅超时**路径）时的失败形态可预期。
- **验收**：新增 probe 用例 `P121_b2_accept_race` ⇒ 3 acceptor + 1 连接 ⇒ **恰好一个** acceptor 拿到连接、其余**仍在等待**（不是 `NULL`）、监听句柄仍在；随后**再连 2 条** ⇒ 另外两个 acceptor 依次接手，三条连接全部正常
- **反向守卫**：把 `wake_socket_waiter` 改回「唤醒全部」⇒ 新用例必红（落选者拿 `NULL` ⇒ 调用方 `TCP_RECV(NULL)` ⇒ `E0030`）
- **非目标**：**不新增** `TCP_ACCEPT_SHARED`（新增成员要动附录 A + 各册成员表 + §0.9 登记，收益不抵成本）；需求清单要的「落选者**保持存活**并能继续接受后续连接」由「唤醒一个」**直接达成**（比「返回可区分值」更强）
- **规则侧**：**SP-2（条件项）** —— 若实现按「唤醒一个」落地，竞争**不再产生 `NULL`** ⇒ 规范侧**零改动**；只有当实现仍保留「落选者返回 `NULL`」的语义时，才需要 §6.1 那句澄清

#### W-12 · B-3：可推进的接受循环（迭代器形态）
- **目标**：20 个连接连续接受，**代码里不出现展开的重复段**。
- **依赖**：无
- **归属文件**：`impl/crates/wlwl-std/src/net.rs`（新成员）、`impl/crates/wlwl-std/src/tcp.rs`（accept 循环下沉到原生帧）、`impl/crates/wlwl-std/src/lib.rs`（`SPEC` 登记）
- **改动要点**：新增 **`TCP_SERVE(listener, timeout_ms, opts, handler) -> INTEGER`**（返回处理连接数）：
  - 原生帧内循环：`accept` → 读满一次请求（阻塞）→ 调 `handler(request_dict) -> response_dict` → 写回 → 继续；
  - `handler` 契约：**内部不得调用宿主挂起点成员**（文档写明，违反 ⇒ `E0014`）；
  - 单条连接异常（对端断开）⇒ 记一笔、继续下一条，**不影响**后续。
  - ⚠ 命名与落地形态登记进 §6 的新成员清单（附录 A + 系统册成员表）。
- **验收**：新增 probe 用例 `P121_b3_serve_loop` ⇒ 连续 20 个连接全 200、中途断开一个不影响后续；契约测试 `net_contract` 增 2 例
- **反向守卫**：把「读满请求再回调」改回「回调后逐条读」⇒ 用例必红
- **非目标**：**不做**「wlwl 层的 `FOR`/`WHILE` 循环体可挂起」——那是 `ADR-0017 Step 3` 的路线；本项是需求清单候选 (a) 的**迭代器式**落地
- **规则侧**：新成员 ⇒ **SP-4**

#### W-13 · B-4：跨任务共享可变状态（共享容器 + 原子更新）
- **目标**：两个并发 handler 共享一个计数器，最终计数**精确等于**请求总数。
- **依赖**：W-12（`handler` 不挂起 ⇒ 原生帧内更新无需跨帧恢复）
- **归属文件**：`impl/crates/wlwl-std/src/sync.rs`（**新建**）、`impl/crates/wlwl-std/src/lib.rs`（`SPEC` + `resolve()` + `NATIVE_SPECS` 登记）、`impl/crates/wlwl-eval/src/stdlib_mirror.rs`（`NAMESPACE_META` 登记）
- **改动要点**：新命名空间 **`std.sync`**：
  - `std.sync::COUNTER(0) -> INTEGER`（句柄）
  - `std.sync::COUNTER_INC(c, delta) -> INTEGER`（**原子**自增，返回新值）
  - `std.sync::COUNTER_ADD(c, delta) -> INTEGER`
  - `std.sync::COUNTER_GET(c) -> INTEGER`
  - `std.sync::SHARED_DICT() -> INTEGER` / `SHARED_DICT_SET(d, k, v)` / `SHARED_DICT_GET(d, k)`（**单键原子写**；整体读出用 `SHARED_DICT_SNAPSHOT`）
  - ⚠ 句柄是**不透明 `INTEGER`**，不透明状态放进程级表，**表里不放任何 `Value`**（与 `tcp.rs` 同一纪律：`Value` 是 `Rc`，不是 `Send`）
- **验收**：新增 probe 用例 `P121_b4_shared_counter` ⇒ 两个 handler 各 500 次 ⇒ 计数 = 1000（**精确**）；契约测试 `sync_contract` ≥ 4 例
- **反向守卫**：把原子自增改成非原子（先读后写）⇒ 用例必红
- **非目标**：**不做** `MUTEX` / `CONDITION` / `SEMAPHORE`（它们需要「挂起后越过阻塞点」⇒ Step 3）；**不引入任何语言级共享 cell**（撞并发册 `:147`）
- **规则侧**：**SP-3** + **SP-4**

#### W-14 · B-5：非阻塞出站（`HTTP_REQUEST_ASYNC` / `PROCESS_RUN_ASYNC`）
- **目标**：一个 handler 在等 10 s 的慢速 HTTP 响应时，**其它 handler 仍能正常响应**。
- **依赖**：W-10（挂起点幂等模式要与 `E0014` 判定共存）
- **归属文件**：`impl/crates/wlwl-std/src/net.rs`、`impl/crates/wlwl-std/src/process.rs`、`impl/crates/wlwl-value/src/lib.rs`（`YieldReason` 新变体）、`impl/crates/wlwl-eval/src/runtime.rs`（挂起点 → 唤醒通道）
- **改动要点**：① 新成员 `HTTP_REQUEST_ASYNC(method, url, body?, headers?, timeout_ms?) -> INTEGER`（返回**任务句柄**）；② 新成员 `PROCESS_RUN_ASYNC(argv, opts?) -> INTEGER`；③ 顺带落地 **I-3 的 `TCP_CONNECT`**（出站 TCP 句柄：`TCP_CONNECT(host, port, timeout_ms) -> INTEGER`，与 `TCP_LISTEN` 同形；HTTPS 与 DB/Redis 出站都靠它）；④ 阻塞工作搬到**工作线程**，成员在挂起点上 `Yield`，就绪后由调度器唤醒；⑤ **幂等**：重入时用 `resume_hint` / `current_task_key` 判定「这次是重放」⇒ **绝不重复发起请求**（照抄 `tcp.rs:539` / `:584` 的既有模式）。
- **验收**：新增 probe 用例 `P121_b5_async_outbound` ⇒ 一个 handler 等 10 s 慢响应，另一个 handler 在 1 s 内拿到 200
- **反向守卫**：去掉幂等判定 ⇒ 用例必红（重复发起 ⇒ 计数翻倍）
- **非目标**：**不改** `HTTP_*` / `PROCESS_RUN` 的阻塞语义（撞 `:223`）；**不实现**流式响应体
- **规则侧**：新成员 ⇒ **SP-4**；⚠ 系统册 `:777` 的「阻塞形态带条件冻结」**在本轮**前提**仍成立**（Step 3 未落地）⇒ 规范侧**不动**那一句，只在实现文档里说明新成员是推荐路径

#### W-15 · B-6 + B-7：任务终值可区分 + 优雅停机
- **目标**：B-6 —— `AWAIT` 能区分「成功 / 失败 / 被取消」；B-7 —— 收到 SIGTERM / Ctrl-C 后，进程在上限时间内退出且在途请求正常收尾。
- **依赖**：W-14（停机要能打断挂起点上的等待）
- **归属文件**：`impl/crates/wlwl-eval/src/runtime.rs`（任务终值）、`impl/crates/wlwl-eval/src/lib.rs`（`AWAIT`）、`impl/crates/wlwl-std/src/signal.rs`（**新建**）、`impl/crates/wlwl-std/src/lib.rs`（`SPEC` 登记）
- **改动要点**：① **B-6 修兜底臂**：`run_one_task` 的兜底臂（`lib.rs:10651-10684`）把「非四种 park 信号」的 outcome 结成 `TaskResult::Ok(o.value)`，而原生成员挂起时带的 value 恰是 `Value::Null` ⇒ **丢失的挂起信号会伪装成「正常返回 NULL」**（R-25）⇒ 改成「任务失败 + 按 §17.9 上报」；**不改** `AWAIT` 的既有语义（成功/失败/取消三态**已具备**：`Awaited::{Ok, ErrValue, Failed, Cancelled}`，`lib.rs:4734-4763`）——需求清单描述的「裸 NULL 不可区分」**不成立**；② 补**契约用例**（失败被 `AWAIT` 认领 ⇒ 调用点重抛；无人认领 ⇒ stderr 一行、退出码仍 0）；⚠ 失败上报的「挂起—认领—收尾」三段式**不得**破坏（不要在失败点当场 `eprintln!`，否则双报）；③ **B-7** 新命名空间 **`std.signal`**：`ON_SIGNAL(sig, handler)`（`SIGINT`/`SIGTERM` / Windows `CTRL_C_EVENT` / console close）、`STOP()`（请求优雅停机）；Unix 用 `libc::signal`（`wlwl-std` 已有 `cfg(unix)` 的 `libc` 依赖 ⇒ 零新增依赖），Windows 用手写 `extern "system" { SetConsoleCtrlHandler }`；停机序列：**停止 accept → 等在途请求（可配上限时）→ 落盘 → 退出**。
- **验收**：新增 probe 用例 `P121_b6_task_result` + `P121_b7_graceful_stop`；`cargo test --locked -p wlwl-cli --test concurrency` ⇒ 绿
- **反向守卫**：把终值统一回裸 `NULL` ⇒ `P121_b6` 必红
- **非目标**：**不引入新诊断码**（B-6 的区分靠**既有** `E0100` 家族 + `ERR` 值）；B-7 不承诺「零数据丢失」之外的强保证
- **规则侧**：新命名空间 ⇒ **SP-4**

#### W-38 · `TCP_SERVE` 的 handler 里调宿主挂起点成员 ⇒ 该连接**静默作废**（第三波带出的缺口）
- **目标**：让「handler 里写了 `TCP_RECV` / `TCP_ACCEPT` / `SLEEP_ASYNC`」这件事**不再无声无息** —— 现在它既不报错也不计数（§13.4 的实测读数）。
- **来源**：W-35（第三波）把 `p10`/`p11`/`p13` 迁到 `TCP_SERVE` 形状时，做反向守卫（把 handler 改回旧形状）**实测**到：handler 被调用 **1** 次，但 `TCP_SERVE` 返回 **0**、程序 **exit 0**、**stderr 一个字都没有** ⇒ 那条连接**既没被服务、也没有任何诊断**（失败只落进宿主告警槽的软告警，**不打印**）。
- **归属文件**：`impl/crates/wlwl-std/src/net.rs`（`TCP_SERVE` 的 handler 调用处）+ `impl/crates/wlwl-eval/tests/tcp_serve_contract.rs`
- **改动要点（待裁决后定）**：① **诊断化** —— 在 handler 调用点判「原生帧内是否出现宿主挂起点成员」⇒ 报**既有**诊断码（不得新增）；② **拒绝** —— 直接把该条连接作废但**报一行**；③ **文档化** —— 只在实现文档写明「handler 不得碰挂起点成员」，实现不动。⚠ 判定「原生帧内算不算任务体」要动 W-10 的 `task_body_depth` 语义 ⇒ 与 W-10 的 `E0014` 判据**耦合**（并发册 `:94` 的措辞是「调用点必须在任务体内」，原生帧**不是**任务体 ⇒ 按字面读法**应当**报错 ⇒ 倾向 ①）。
- **验收**：新增契约用例 —— handler 里调 `TCP_RECV` ⇒ 该用例**必须看到诊断或明确的失败信号**（不再是「静默少一条」）
- **反向守卫**：去掉判定 ⇒ 用例必红（回到「静默作废」）
- **非目标**：⚠ **不新增诊断码**（1.0.x 无通道）；不碰 `TCP_SERVE` 的既有可观察语义（成功服务的连接数、失败只记一笔继续下一条）
- **规则侧**：若走 ① ⇒ 需在并发册 §17.0 的措辞里点明「原生帧内**同样**不是任务体」⇒ 属**措辞澄清**（一致性册 §3.4 例外 2），**不动既有成员语义**

### 批次 C · HTTP 协议层（5 个工作项，落 `std.http`）

> **`std.http` 的形态**：**R1（纯 wlwl 源码）** + 少量 R2 内核（`impl/crates/wlwl-std/wl/std/http.wll` + `impl/crates/wlwl-std/src/net.rs` 的新内核）。理由：ADR-0021 的分层规则（纯逻辑 ⇒ R1；触系统调用 ⇒ R2），且 R1 模块本身**就是语言符合性测试**。

#### W-16 · `std.http` 骨架：请求解析 + 响应构造（C-1 / C-2）
- **目标**：`std.http::PARSE_REQUEST(bytes) -> DICT`（方法 / 路径 / 版本 / header 字典 / body）、`std.http::BUILD_RESPONSE(status, headers, body) -> DICT`。
- **依赖**：W-02（夹具）
- **归属文件**：`impl/crates/wlwl-std/wl/std/http.wll`（**新建**）、`impl/crates/wlwl-std/src/lib.rs`（`LANG_SOURCES` + `resolve()`）、`impl/crates/wlwl-eval/src/stdlib_mirror.rs`（`NAMESPACE_META`）
- **改动要点**：① header 名**大小写不敏感**、重复 header **可取全部值**（`DICT` 值允许数组）；② body 支持 `Content-Length` 与 **`chunked`**（**服务端**仍按 ADR-0028 D3 **拒绝** chunked 请求体 ⇒ 返 `ERR`）；③ `BUILD_RESPONSE` **内部只认字节数**（用 `BYTE_LEN`，不认 `LEN`）⇒ 结构上杜绝错长；④ `HEAD` 只发头不发体。
- **验收**：新增 probe 用例 `P121_c1_c2_http_parse_build`；契约测试 `http_contract` ≥ 6 例
- **反向守卫**：把 `BYTE_LEN` 换成 `LEN` ⇒ 用例必红（多字节正文 ⇒ `Content-Length` 错）
- **非目标**：不碰 `std.net::HTTP_*` 的任何语义
- **规则侧**：新命名空间 ⇒ **SP-4**

#### W-17 · `std.http` 连接管理：Keep-Alive + 预算 + 大小限制（C-3 / C-8 / C-9）
- **目标**：一个连接连续处理 3 个请求；空闲超时后服务端主动关；首字节 / 头 / 体 / 写出分别可配时限；超限 ⇒ 413 / 431。
- **依赖**：W-16
- **归属文件**：`impl/crates/wlwl-std/wl/std/http.wll`、`impl/crates/wlwl-std/src/net.rs`（新内核：读超时 / 大小上限）
- **改动要点**：① `std.http::SERVE_STREAM(conn, opts, handler)`（在一条连接上循环处理请求，直到对端关闭 / 空闲超时 / `Connection: close`）；② 请求级预算 = {first_byte_ms, headers_ms, body_ms, response_ms}；③ 头块上限 / 体上限 ⇒ 431 / 413。
- **验收**：新增 probe 用例 `P121_c3_c8_c9_keepalive_budget`
- **反向守卫**：把上限设为无限 ⇒ 超限用例必红
- **非目标**：不做 HTTP/2、不做 WebSocket（I 组口径答复）
- **规则侧**：新成员 ⇒ **SP-4**

#### W-18 · `std.http` 路由 + Cookie + 表单/multipart（C-4 / C-5 / C-6）
- **目标**：路径模式匹配（`/article/{slug}.html`）+ 参数提取 + 方法区分 + 404/405；Cookie 解析 / 生成（`HttpOnly`/`Secure`/`SameSite`/`Max-Age`/`Path`/`Domain`）；`x-www-form-urlencoded` 与 `multipart/form-data`（含文件 part）解析。
- **依赖**：W-16
- **归属文件**：`impl/crates/wlwl-std/wl/std/http.wll`
- **改动要点**：① 路由表以 `DICT` 表达（`"/article/{slug}.html"` ⇒ 参数抽取）；② Cookie 的 `Set-Cookie` 序列化；③ multipart 边界解析（按 `boundary` 切分，文件 part 落临时目录并给路径）。
- **验收**：新增 probe 用例 `P121_c4_c5_c6_router_cookie_multipart`
- **反向守卫**：把 multipart 的边界匹配改成前缀匹配 ⇒ 用例必红
- **非目标**：不做 WebSocket / SSE
- **规则侧**：新成员 ⇒ **SP-4**

#### W-19 · `std.http` 静态文件 + 条件请求 + 压缩（C-7 / C-10 / C-12）
- **目标**：静态文件服务（**内建且跨平台正确**的路径穿越防护、MIME、`Content-Length`、`ETag`/`Last-Modified`/`304`、`Range`/`206`、`Cache-Control`、目录 ⇒ `index.html`）+ `Accept-Encoding` 协商与 `Content-Encoding`。
- **依赖**：W-16、W-28（`PATH_CONTAINS`）
- **归属文件**：`impl/crates/wlwl-std/wl/std/http.wll`、`impl/crates/wlwl-std/src/fs.rs`（新内核：mtime）
- **改动要点**：① 路径防护**不用字符串前缀**：一律走 `std.fs::PATH_CONTAINS`（W-28）；② `ETag` 用 `mtime + size`；③ `Range` 支持单区间；④ 压缩协商 `gzip` / `zstd`（`std.compress` 已有 8 员）。
- **验收**：新增 probe 用例 `P121_c7_c10_c12_static_file`
- **反向守卫**：把防护改成「字符串前缀」⇒ `D:/root-evil` 用例必红
- **非目标**：不做 brotli（I-4 口径答复）
- **规则侧**：新成员 ⇒ **SP-4**

#### W-20 · `std.http` 客户端：chunked 解码 + HTTPS 出站（C-11 / A-2）
- **目标**：`std.http::GET(url, opts?) -> DICT` 能取回 **chunked** 响应正文；`https://` 走 TLS 扩展。
- **依赖**：W-14（异步出站）、W-21（TLS 扩展）
- **归属文件**：`impl/crates/wlwl-std/wl/std/http.wll`、`impl/crates/wlwl-std/src/net.rs`（新内核：chunked 解码）
- **改动要点**：① **客户端**解码 chunked（服务端继续拒绝 —— 需求清单 C-11 明写「服务端继续拒绝」）；② `https://` ⇒ 调 `tls::CONNECT`（扩展注入的 kernel）；③ 扩展未加载 ⇒ `ERR(kind:"TlsError", reason:"no TLS provider loaded")`；④ 证书错误逐字透传（`certificate expired` 等）。
- **验收**：新增 probe 用例 `P121_c11_chunked_client`（对本地 chunked 服务端）；A-2 判据由 W-21 的端到端用例覆盖
- **反向守卫**：把 chunked 解码去掉 ⇒ 用例必红
- **非目标**：**不改** `std.net::HTTP_GET`（`:750` 要求 `https://` ⇒ `ERR`；改它撞 `:223`）
- **规则侧**：新成员 ⇒ **SP-4**；`std.net` **零改动**

### 批次 D · TLS 与扩展面（5 个工作项）

#### W-21 · 宿主扩展加载器（ABI + 信任门控 + 运行期注册表）
- **目标**：让宿主能在**显式授权**下加载一个原生扩展，把它的成员挂成可 `IMPORT` 的命名空间。
- **依赖**：无
- **归属文件**：`impl/crates/wlwl-ext/**`（**新建 crate**）、`impl/crates/wlwl-value/src/lib.rs`（`ExtRegistry` 类型与 `StdHost` 的扩展回调）、`impl/crates/wlwl-eval/src/lib.rs`（`resolve_source` 的第 4 层）、`impl/crates/wlwl-cli/src/main.rs`（`--ext` / `--ext-sha256`）
- **改动要点**：① raw FFI 加载（Windows `LoadLibraryExW` + `LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR`；POSIX `dlopen(RTLD_NOW|RTLD_LOCAL)`）⇒ **零新增第三方依赖**；② ABI 校验（`abi_version` / `struct_size`）；③ 路径门控（绝对路径 / 存在 / 哈希钉扎）；④ `std.*` 影子拒绝；⑤ 加载前打印告警；⑥ 运行期注册表 + `resolve()` 的第 4 层；⑦ 扩展成员可作为 **kernel** 注入 R1 模块（复用 `StdSource::kernels` 的注入点 `wlwl-eval/src/lib.rs:1052-1060`）。
- **验收**：新增 crate 自带测试 ≥ 6 例（含「相对路径被拒」「`std.*` 影子被拒」「ABI 不匹配被拒」「哈希不匹配被拒」）；集成 probe 用例 `P121_ext_loader`
- **反向守卫**：把「绝对路径校验」去掉 ⇒ 自测必红
- **非目标**：**不提供**语言级 `LOAD` 成员（T1）；**不做**扩展的签名校验（超出本轮；文档写明「加载 = 交出全部权限」）
- **规则侧**：**SP-5**（台账第一行）

#### W-22 · TLS 扩展 crate（`cdylib`，自持 rustls）
- **目标**：产出 `wlwl-tls` 动态库，实现 `tls` / `crypto` 两个命名空间（§3.6 的成员表）。
- **依赖**：W-21（ABI 头文件）
- **归属文件**：`impl/ext/wlwl-tls-ext/**`（**新建 crate**，`crate-type = ["cdylib"]`）、`impl/ext/wlwl_ext.h`（ABI 头文件）
- **改动要点**：① `rustls` + `ring` 走**扩展自己的**依赖（`rustls 0.23` / `ring 0.17`，**已在 `Cargo.lock` 里**⇒ 版本不新增）；② 每连接一个后台线程做阻塞 I/O，暴露 `poll` 供宿主挂起/唤醒；③ 证书与私钥**只**从文件系统读，**不得**出现在错误消息 / 日志里（A-5）；④ 证书校验**默认开**、TLS 下限 **1.2**、**无**「一键全关」开关（关闭必须显式传参且宿主打告警）。
- **验收**：扩展 crate 自测 ≥ 10 例（含握手成功 / 证书过期 / 私钥不匹配 / 明文客户端被拒 / SPKI 指纹稳定）；端到端：本地起 TLS 服务 ⇒ `curl -k` 得 200；**明文客户端连 TLS 端口 ⇒ 被拒且有明确错误**
- **反向守卫**：把「明文客户端降级为明文」打开 ⇒ 用例必红（需求 A-1 明写「不得静默降级」）
- **非目标**：**不做** HTTP 语义（HTTP 归 `std.http`）；**不做** mTLS 的客户端证书签发链
- **规则侧**：**SP-5**

#### W-23 · 正则模式缓存（D-1）
- **目标**：同一模式对象连续匹配 1000 次，**第二次起不再重新编译**。
- **依赖**：无
- **归属文件**：`impl/crates/wlwl-std/src/regex.rs`（模式缓存 + 句柄）
- **改动要点**：① `RE(pattern)` 的返回值携带**缓存句柄**（进程级表：模式串 ⇒ 编译产物）；② `RE_SEARCH` / `RE_TEST` / `RE_REPLACE` / `RE_SPLIT` / `RE_FIND_ALL` 复用句柄；③ 缓存**不改变任何可观察语义**（同一模式串的匹配结果逐字不变）。
- **验收**：新增 probe 用例 `P121_d1_regex_cache` ⇒ **用确定性工作量计数**（不是墙钟！一致性册 §6.2 V-01 纪律）断言编译次数 = 1
- **反向守卫**：把缓存去掉 ⇒ 用例必红
- **非目标**：**不新增成员**（N-19：文本册 `:502` 已说「实现**可以**缓存」⇒ 缓存**已被允许**，规范**零改动**）；**不**把 `RE_GROUP_COUNT` 的复杂度从 O(p) 升格为 O(1)（那是规范性改动 ⇒ 撞 `:223`）
- **规则侧**：**无**

#### W-24 · 归一化 + 路径包含判断（D-2 / D-4）
- **目标**：WAF 可用的 Unicode 归一化；跨平台正确、**非字符串前缀**的路径包含判断。
- **依赖**：无
- **归属文件**：`impl/crates/wlwl-std/src/text.rs`（`NFKC` / `CASE_FOLD` / `STRIP_INVISIBLE`）、`impl/crates/wlwl-std/src/fs.rs`（`PATH_CONTAINS` / `PATH_CANONICAL`）
- **改动要点**：① `std.text::NFKC(s)`（**当前只有 NFC/NFD，缺 NFKC**）；② `std.text::CASE_FOLD(s)`（Unicode 感知大小写折叠）；③ `std.text::STRIP_INVISIBLE(s)`（零宽 / 双向控制字符）；④ `std.fs::PATH_CONTAINS(root, target) -> BOOLEAN`：**先 `PATH_CANONICAL` 归一**（统一分隔符 + 消 `..`）再做**真实路径**判定，**不是**字符串前缀；⑤ `std.fs::PATH_CANONICAL(p)`：**新增**成员（**不改** `PATH_NORMALIZE` 的语义 —— 撞 `:223`）。
- **验收**：新增 probe 用例 `P121_d2_d4_normalize_path_contains` ⇒ `PATH_CONTAINS("D:/root","D:/root-evil/x")` = `FALSE`；`PATH_CONTAINS("D:/root","D:/root/../root2/x")` = `FALSE`；`SELECT` 规则命中 `ｓｅｌｅｃｔ`（先 NFKC）
- **反向守卫**：把 `PATH_CONTAINS` 改回前缀比较 ⇒ 用例必红
- **非目标**：不做完整 Unicode 双向算法（只做 WAF 需要的归一化子集）
- **规则侧**：新成员 ⇒ **SP-4**

#### W-25 · 中间件原语：限流 / 规则热加载 / 审计 / 真实 IP（D-3 / D-5 / D-6 / D-7）
- **目标**：滑动窗口 / 令牌桶；WAF 规则集从文件加载 + 文件变更后热更新（原子切换，不影响在途请求）；机器可读审计日志（JSON 行：时间 / 事件 / 客户端 IP / 路径 / 结果 / 请求 ID）；可信代理列表 ⇒ 真实 IP。
- **依赖**：W-16（`std.http`）、W-26（`std.log`）
- **归属文件**：`impl/crates/wlwl-std/wl/std/http.wll`（限流 / 规则 / 真实 IP）、`impl/crates/wlwl-std/src/fs.rs`（规则文件 mtime）
- **改动要点**：① 限流器基于 W-13 的 `std.sync` 共享容器（**原子**）⇒ 计数精确；② 规则集：读文件 ⇒ 编译成规则数组 ⇒ 记录 mtime；变更 ⇒ **原子切换**（换引用，不改在途请求正在用的旧数组）；③ 审计：`std.log` 的结构化字段；④ 真实 IP：`X-Forwarded-For` 只在**可信代理列表**内采信。
- **验收**：新增 probe 用例 `P121_d3_d5_d6_d7_middleware` ⇒ 60 次/60 秒窗口命中 61 次拒绝；规则热切换不影响在途请求；审计行是合法 JSON
- **反向守卫**：把限流计数改成非原子 ⇒ 用例必红
- **非目标**：不提供分布式限流（多进程协调）
- **规则侧**：新成员 ⇒ **SP-4**

### 批次 E · 持久化 / 文本 / 格式化（4 个工作项）

#### W-26 · `std.log` 结构化日志 + 运行时指标（F-1 / F-2）
- **目标**：级别（TRACE/DEBUG/INFO/WARN/ERROR）、级别过滤、输出到文件、结构化字段（JSON 行）、可从环境变量配级别；请求计数 / 状态码分布 / 延迟直方图 / 当前连接数与任务数。
- **依赖**：无
- **归属文件**：`impl/crates/wlwl-std/src/log.rs`（**新建**）、`impl/crates/wlwl-std/src/lib.rs`（`SPEC` + `resolve()` + `NATIVE_SPECS`）、`impl/crates/wlwl-eval/src/stdlib_mirror.rs`（`NAMESPACE_META`）
- **改动要点**：① 新命名空间 **`std.log`**：`LOG(level, message, fields?)`、`LOG_SET_LEVEL(l)`、`LOG_FILE(path)`、`LOG_METRICS() -> DICT`；② 级别默认值取环境变量 `WLWL_LOG_LEVEL`（缺省 `INFO`）；③ 指标用 W-13 的共享容器（原子累加）⇒ 与审计日志**分文件**（修需求方指出的「混在一个文件」缺陷）。
- **验收**：新增 probe 用例 `P121_f1_f2_log_metrics`
- **反向守卫**：把级别过滤去掉 ⇒ 用例必红
- **非目标**：不做远程日志投递
- **规则侧**：新命名空间 ⇒ **SP-4**

#### W-27 · 文件系统补齐：原子写 / 文件锁 / 元数据（E-1 / E-2 / E-3）
- **目标**：`WRITE_FILE_ATOMIC`（临时文件 + rename + fsync）；文件锁（`LOCK_FILE` / `UNLOCK_FILE`）；目录项元数据（mtime / 大小 / 类型）。
- **依赖**：无
- **归属文件**：`impl/crates/wlwl-std/src/fs.rs`
- **改动要点**：① `WRITE_FILE_ATOMIC(path, content)`：同目录临时文件 ⇒ 写 ⇒ fsync ⇒ rename ⇒ 失败清理临时文件；② `LOCK_FILE(path) -> INTEGER` / `UNLOCK_FILE(handle)`：跨进程文件锁（Windows `LockFileEx` / POSIX `flock`，**手写 raw FFI**，零新增依赖）；③ `FILE_MTIME(path) -> INTEGER`（Unix 毫秒 / Windows FILETIME 转换）、`DIR_ENTRIES(path) -> ARRAY[DICT]`（`{name, size, mtime, is_dir}`）。
- **验收**：新增 probe 用例 `P121_e1_e2_e3_fs_atomic_lock_meta`
- **反向守卫**：把 `WRITE_FILE_ATOMIC` 改回直接 `write` ⇒ 用例必红（中断后不留半文件）
- **非目标**：不做网络文件系统上的锁保证
- **规则侧**：新成员 ⇒ **SP-4**

#### W-28 · 文本 / 格式化补齐（E-4 / E-5 + H-2 / H-3）
- **目标**：`QUERY_PARSE(query_string) -> DICT`（支持重复键）；`std.format::FORMAT` 支持具名参数、宽度 / 对齐 / 填充。
- **依赖**：无
- **归属文件**：`impl/crates/wlwl-std/wl/std/str.wll`（`QUERY_PARSE`）、`impl/crates/wlwl-std/src/format.rs`（`FORMAT` 增强）
- **改动要点**：① `QUERY_PARSE` 走**全局内建 `SPLIT`**（R-8）⇒ 不新增 `SPLIT`；② `FORMAT` 的 `{name}` 具名参数**扩展到全部 DICT 参数**（当前只有第一个），并支持 `{0:>10}` 宽度 / 对齐 / 填充；③ **不动** `+` 的强转规则（撞 `:223`）—— 数字转字符串走**全局内建 `STR`**（R-8 / R-9），文档与技能包同步订正。
- **验收**：新增 probe 用例 `P121_e4_e5_text_format`
- **反向守卫**：把 `FORMAT` 的具名参数解析改回「只认第一个 DICT」⇒ 用例必红
- **非目标**：不做模板引擎（条件 / 循环留应用层）
- **规则侧**：新成员 + **既有成员 `FORMAT` 的行为扩展** ⇒ ⚠ **`FORMAT` 既有语义**（`{name}` 只认第一个 DICT）**是既有成员的可观察语义** ⇒ 按 `:223` **不得**扩展！⇒ **处置**：`FORMAT` 的增强**另起新成员** `std.format::FORMAT_NAMED(template, dict)`，原 `FORMAT` **一字不改**（⇒ SP-4）
- **规则侧**：新成员 ⇒ **SP-4**

#### W-29 · 别名与易用性小修（H-4 / H-5 / H-6）
- **目标**：`RE_MATCHES_FULL`（别名 `RE_TEST`，文档首行标注「整串锚定」）、`ARRAY_GET`（别名 `INDEX_GET`，错误信息提示正确 API）、`PATH_CANONICAL`（见 W-24）。
- **依赖**：W-24
- **归属文件**：`impl/crates/wlwl-std/src/regex.rs`、`impl/crates/wlwl-std/src/collection.rs`、`impl/crates/wlwl-eval/src/registry.rs`（全局内建 `ARRAY_GET` ⇒ 附录 G 需重生成）
- **改动要点**：① 别名**同名同义**，既有成员**一字不改**；② `INDEX_GET` 的类型错误消息里加一句「也可用 `ARRAY_GET`」；③ `ARRAY_GET` 若落全局内建 ⇒ `docs/appendix_G.md` 用 `cargo run --bin gen-appendix-g` 重生成（附录 G 有逐字对拍守卫）。
- **验收**：新增 probe 用例 `P121_h5_h6_aliases`；`cargo run --locked --bin gen-appendix-g -- ../docs/appendix_G.md` 后附录 G 对拍绿
- **反向守卫**：把别名删掉 ⇒ 用例必红；把 `RE_TEST` 的语义改成包含匹配 ⇒ 老用例必红
- **非目标**：**不重命名**任何既有成员（撞 `:224`）
- **规则侧**：新成员 ⇒ **SP-4**

### 批次 F · 可观测性 / 测试 / 模块（4 个工作项）

#### W-30 · `wlwl tasks` 任务检视 + 最小 REPL（F-3 / G-3）
- **目标**：运行期能看到「任务挂在哪个挂起点 / 已运行时长 / 任务树」；至少一个能跑表达式的 REPL。
- **依赖**：W-15（任务终值与停机）、W-21（若复用扩展机制传指标则需 W-21；否则走本地 socket）
- **归属文件**：`impl/crates/wlwl-eval/src/runtime.rs`（任务快照）、`impl/crates/wlwl-cli/src/main.rs`（`tasks` / `repl` 子命令）、`impl/crates/wlwl-cli/src/tooling.rs`（JSON 输出，风格对齐 `interface` / `namespace` / `schema`）
- **改动要点**：① `wlwl run --inspect <addr>` 在 `127.0.0.1` 起一个**只读**检视端点（JSON 行），`wlwl tasks --addr <addr>` 拉取；⚠ `Task` **没有启动时刻字段**（R-32）⇒ 「已运行时长」需要**新加字段**（`task.rs:50`）；调度器的 `TaskState::Suspended{tag,reason}` / `deadline_ms` / `resume_hint` 已有现成读数；② 端点**只绑本机**、只读、无执行能力；③ `wlwl repl` 起一个 REPL，能跑表达式、打印当前任务挂起点（**只读**语义：不支持改状态）。
- **验收**：新增 probe 用例 `P121_f3_g3_inspect_repl`（起服务 ⇒ 查 tasks ⇒ JSON 可解析）；`cargo test --locked -p wlwl-cli` ⇒ 绿
- **反向守卫**：把端点改成可执行 ⇒ 守卫必红（本轮只读）
- **非目标**：不做完整调试器（断点 / 单步 / 变量改写）
- **规则侧**：**无**（CLI 子命令不是公共面）

#### W-31 · `std.test` 增强（G-1）
- **目标**：断言带自定义消息、捕获原生诊断码、跳过、参数化、临时目录夹具、异步测试、等任务；结果输出可被 CI 解析。
- **依赖**：W-13（异步测试要能等任务）
- **归属文件**：`impl/crates/wlwl-std/wl/std/test.wll`（门面）、`impl/crates/wlwl-std/src/test_native.rs`（R2 内核）
- **改动要点**：① 新成员：`ASSERT_MSG(cond, message)`、`EXPECT_ERR_CODE(fn, code)`（捕获 `E0100` 这类**原生**诊断码 —— 当前 `EXPECT_ERR` 只有一个参数且捕不到原生码）、`SKIP(reason)`、`TEST_CASES(...)`（参数化）、`TEMP_DIR()`（临时目录夹具）、`RUN_TESTS_ASYNC(...)`；② `ASSERT(0)` / `ASSERT("")` / `ASSERT([])` **不该判失败**（现判失败）⇒ 这是**既有成员语义** ⇒ 按 `:223` **不改** `ASSERT`，而是新增 `ASSERT_TRUE`（语义 = 「值为真」），并在文档里说明差异；③ 结果输出加 `--format json` 形态。
- **验收**：新增 probe 用例 `P121_g1_std_test`；契约测试 `test_contract` 增 ≥ 6 例
- **反向守卫**：把 `EXPECT_ERR_CODE` 改回捕不到原生码 ⇒ 用例必红
- **非目标**：不改 `ASSERT` / `ASSERT_EQ` / `EXPECT_ERR` / `RUN_TESTS` 的既有语义
- **规则侧**：新成员 ⇒ **SP-4**

#### W-32 · `wlwl check` 静态检查新增两条规则（G-2）
- **目标**：① 静态发现「宿主挂起点成员出现在任务体之外（含嵌套闭包内）」；② 静态发现「跨任务共享可变绑定」。
- **依赖**：W-10、W-13
- **归属文件**：`impl/crates/wlwl-cli/src/main.rs`（`check` 的 lint 段）、`impl/crates/wlwl-types/src/**`（静态分析层）
- **改动要点**：① 规则 ① **复用 `E0014`**（既有码，语义正好是「宿主挂起点成员在任务体外」⇒ N-12）；② 规则 ② 走**「规范符合性提示」**形态（**不带 E/W 码**，带稳定名称 `shared-mutable-binding`）—— ✅ **业主 2026-10-27 裁决「v1.0.4 包含」**（§10 Q4）：① 把 `wlwl check` 与 LSP **共用的收集函数**从「返回诊断列表」扩成「返回 `(诊断, 提示)`」⇒ 两端拿同一份提示，不产生工具分裂（语言册 `:1681`）；② 提示**不改变退出码**（与警告同款：`:1262`）⇒ 退出码表不动；③ LSP 侧映射为 `severity: Warning`（LSP 的 `code` 字段本就可选）；④ **同轮**落运行期拦截 `ERR([kind: "SharedMutableViolation"])` 作兜底 ⇒ 规则侧**零改动**、**零新码** ⇒ v1.0.4 原样成立。
- **验收**：新增 probe 用例 `P121_g2_static_rules`（`wlwl check` 对违规样本 ⇒ 规则 ① 报 `E0014`；规则 ② 报提示 `shared-mutable-binding`；**两端（CLI 与 LSP）拿同一份**）
- **反向守卫**：把规则 ① 的判定去掉 ⇒ 用例必红；把规则 ② 的判定去掉 ⇒ 提示消失 ⇒ 用例必红；把「收集函数」拆成两份 ⇒ 用例必红（守住「编辑器不报、CI 报」的分裂）
- **非目标**：**不新增诊断码**（N-3 的硬约束）；规则 ② 的提示**不阻塞退出码**
- **规则侧**：**无**（规则 ① 复用既有码；规则 ② 无码提示 ⇒ 不进附录 A、不进 §0.9）

#### W-33 · 诊断可读性 + 模块组织（F-4 / G-4）
- **目标**：诊断带**源码行片段 + 列号高亮**，`wlwl check` / `wlwl run` 都给，并在可能时给「你是不是想写……」提示；模块组织补齐（模块级常量与初始化代码、循环导入的清晰诊断、模块私有成员、`.wll.sig` 工作流简化）。
- **依赖**：无
- **归属文件**：`impl/crates/wlwl-cli/src/diag_output.rs`、`impl/crates/wlwl-error/src/lib.rs`、`impl/crates/wlwl-eval/src/lib.rs`（模块加载 / 循环导入诊断）
- **改动要点**：① 诊断输出补**源码行 + 列号高亮**（`diag_output.rs` 已有位置块，补内容）；② 提示语：例如「`IF` 的第三实参不支持多语句组」—— ⚠ 注意 **H-1 已修**（R-?/§2.4）⇒ 提示语必须**按当前实现的事实**写，不能照抄需求清单；③ 模块私有成员：`.wll.sig` 的导出面收紧（**不新增语言概念**）；④ 循环导入：`IMPORT` 成环 ⇒ **既有诊断**（`E004x` 家族）里补明确文案。
- **验收**：新增 probe 用例 `P121_f4_g4_diag_module`（诊断含源码行与列号）
- **反向守卫**：把高亮输出关掉 ⇒ 用例必红
- **非目标**：不改任何既有诊断码的**含义**（撞 `:223`）
- **规则侧**：**无**

### 批次 G · 文档 / 技能包 / 收口（2 个工作项）

#### W-34 · 文档与技能包同步（H-1 / H-2 / H-3 口径 + 本轮新成员 + TLS 扩展用法）
- **目标**：`wlwl-skill/SKILL.md`、`docs/site/**`、`CHANGELOG.md` 与本轮新成员 / 新形态对齐（**含**「`STR` / `SPLIT` 已存在」这条订正）。
- **依赖**：W-12…W-33
- **归属文件**：`wlwl-skill/**`、`docs/site/**`、`CHANGELOG.md`、`docs/plan/README.md`
- **改动要点**：① 明确「`STR` / `SPLIT` 是全局内建」；② 明确 `HTTP_*` 仍阻塞、推荐 `HTTP_REQUEST_ASYNC`；③ 明确 TLS **不在标准库**、须 `--ext` 加载、**加载 = 交出全部权限**；④ 明确 `TCP_SERVE` 的「handler 内不得挂起」契约。
- **验收**：`impl/crates/wlwl-cli/tests/skill_pack_*.rs` 全绿；`docs/site/**` 与实际成员面一致（`wlwl namespace` 输出对照）
- **反向守卫**：把技能包里的「`+` 会把数字转成字符串」写回去 ⇒ 文档一致性守卫必红
- **非目标**：不重写整套文档
- **规则侧**：**无**

#### W-35′ · 收口：附录 A 抄件同步 + 全量门禁 + 发行包

> ⚠ **编号消歧**：业主 2026-10-28 12:36 的裁决 D3 把「三个探针改写成 `TCP_SERVE` 形状」也编成了 **W-35**（第三波已落地，见 §11.1 / §13.4）⇒ 本收口项改记 **W-35′**（§13.4 的「收口批」原本就写作 `W-35'`）⇒ **两个 W-35 不再混淆**。
- **目标**：规范侧 v1.0.4 落地后，**重生成**本仓附录 A 抄件并让**全部**门禁绿。
- **依赖**：全部前置 + 规范仓 v1.0.4 已发布
- **归属文件**：`docs/wlwl-spec-reference.md`（附录 A 标记区）、`impl/Cargo.toml`（版本号 0.12.0 ⇒ **0.12.1**，11 个 crate 同步）、`CHANGELOG.md`
- **改动要点**：① 附录 A 抄件**逐字**改为 v1.0.4 的表体（**不**自动生成改规范！N-22）；② `impl/Cargo.toml` 与 11 个 crate 的版本号一起改到 `0.12.1`；③ 发行包带上 `wlwl-tls` 动态库与哈希；④ `ci.yml:121` 的规范仓钉死提交**前移**到 v1.0.4 的提交。
- **验收**：§8.1 全部门禁绿；`cargo test --locked --test conformance -p wlwl-cli` ⇒ 绿
- **反向守卫**：把附录 A 抄件改一个字 ⇒ `check-appendix-a` 必红
- **非目标**：**不打 tag**（发版由业主发起）
- **规则侧**：落地 SP-1…SP-5 的**最后一步**

---

## 5 构建期多 AGENT 编排

### 5.1 切分原则（先说清楚，避免并行时打架）

1. **按文件切，不按任务切**：一个工作项 = 一个 owner = 一组**独占**文件。
2. **共享文件由「集成者」独占**：下列文件**任何并行 agent 都不得改**，只在集成阶段由集成者统一改 —— `impl/crates/wlwl-std/src/lib.rs`（三张静态表）、`impl/crates/wlwl-eval/src/stdlib_mirror.rs`（`NAMESPACE_META`）、`docs/wlwl-spec-reference.md`（附录 A 抄件）、`impl/crates/wlwl-cli/tests/probe.rs`（`EXPECTED_CASE_COUNT`）、`impl/crates/wlwl-eval/src/registry.rs`（全局内建表）。
   ⇒ **因此**：各 agent 只产出「成员实现 + 契约测试 + 登记清单」，**登记动作由集成者按清单统一执行**（这是并行能不能真的并行的关键）。
3. **禁 `cargo fmt --all`**（会重排别人的文件）——统一在集成阶段跑；**禁 `git commit` / `git push`**（由集成者统一提交）。
4. **每个 agent 必须自证**：交付时给出「验收命令的原始输出」（不是「我跑过了」）。

### 5.2 波次与并行度

| 波次 | 内容 | 并行度 | 依赖 | 关键路径 |
|---|---|---|---|---|
| **W0** | W-01 基线 / W-02 夹具 | 1（串行） | — | ~0.5 天 |
| **W1** | W-21 加载器 · W-10 B-1 · W-12 B-3 · W-23 D-1 · W-24 D-2/D-4 · W-26 F-1/F-2 · W-27 E-1..3 · W-33 F-4/G-4 | **8 并行** | W0 | ~1 周 |
| **W2** | W-22 TLS 扩展 · W-11 B-2 · W-13 B-4 · W-14 B-5 · W-16 C-1/C-2 · W-28 E-4/E-5 · W-29 别名 · W-30 F-3/G-3 · W-32 G-2 | **9 并行** | W1 各自的接口件 | ~1.5 周 |
| **W3** | W-17 C-3/C-8/C-9 · W-18 C-4..6 · W-19 C-7/C-10/C-12 · W-20 C-11/A-2 · W-25 D-3..7 · W-15 B-6/B-7 · W-31 G-1 | **7 并行** | W2 | ~1 周 |
| **W4** | W-34 文档/技能包 · W-35 收口（登记 + 附录 A + 门禁）· 规范仓 v1.0.4 批次 | 3（规范侧与实现侧并行，规范侧**不含**实现状态登记行） | W3 | ~0.5 周 |

⚠ **W4 的跨仓顺序**（一致性册 `:154` 闸门 1）：§0.9 登记行的「实现状态」列必须是「已实现」才能打 tag ⇒ **实现侧 W-35 先落地 → 再推规范仓 v1.0.4**。规范仓的**条款级改动**（SP-1/SP-2/SP-3/SP-5）可以与实现侧并行起草。

### 5.2b ⚠ W1 的**真并行度**（按文件冲突实测后修正，2026-10-27）

上表的「8 并行」是**按工作项**数的；按 §5.1 的「按文件切」纪律落成**真并行**时，同一波里**文件会撞车**。实测冲突矩阵（`同一文件 ⇒ 不可并行`）：

| 文件 | 争用者 |
|---|---|
| `wlwl-eval/src/lib.rs` | W-10 · W-21 · W-33 |
| `wlwl-eval/src/runtime.rs` | W-10 · W-11 · W-14 · W-15 · W-30 |
| `wlwl-std/src/tcp.rs` | W-10 · W-11 · W-12 |
| `wlwl-std/src/net.rs` | W-12 · W-14 · W-17 · W-19 · W-20 |
| `wlwl-std/src/fs.rs` | W-24 · W-27 · W-19 · W-25 |
| `wlwl-std/src/regex.rs` | W-23 · W-29 |
| `wlwl-cli/src/main.rs` | W-21 · W-30 · W-32 |
| `wlwl-value/src/lib.rs` | W-21 · W-14 |
| `wlwl-std/src/lib.rs`（集成者专属） | W-12 · W-13 · W-15 · W-16 · W-26 |

⇒ **W1 拆成两批**：

| 批次 | 工作项 | 真并行度 | 文件 |
|---|---|---|---|
| **W1a**（先跑） | W-01（串行前置）· **W-02** · **W-21** · **W-23** · **W-24** | **4 并行**（W-01 单独先做） | 探针目录 / 新 crate `wlwl-ext` + `wlwl-value` + `wlwl-eval/lib.rs` + `wlwl-cli/main.rs` / `wlwl-std/regex.rs` / `wlwl-std/text.rs` + `fs.rs` |
| **W1b**（W1a 合完后） | **W-10** ∪ **W-12** ∪ **W-27** ∪ **W-33** | **3 并行**（W-10 与 W-33 抢 `wlwl-eval/lib.rs` ⇒ 二选一先做） | `runtime.rs`+`eval/lib.rs`+`tcp.rs` / `net.rs`+`tcp.rs` / `fs.rs` / `diag_output.rs`+`error/lib.rs`+`eval/lib.rs` |

⚠ **W-26（`std.log`）在 W1a 之后单独跑**：它是**新命名空间** ⇒ 登记要动 `wlwl-std/src/lib.rs`（集成者专属）⇒ 只能由集成者统一登记 ⇒ agent 只实现 `wlwl-std/src/log.rs` + 契约测试 + 登记清单。
⚠ **W-22（TLS 扩展 crate）在 W1a 之后**：它依赖 **W-21** 的 ABI 头文件 ⇒ 底座必须先合。

### 5.3 派工模板（每个 agent 收到的 briefing 必须含这七项）

```
目标：<一句话，可验收>
范围：<只做哪几条需求 / 哪个工作项>
事实：<已核实的坐标与读数，直接可用，不要重新调查>
产出：<哪些文件（独占）/ 契约测试 / 登记清单（不改共享表）>
验收：<可粘贴命令>
反向守卫：<改回去必须变红的那一条>
禁项：不改共享表、不跑 cargo fmt --all、不 commit、不 push、不新增诊断码、不改既有成员语义
```

### 5.4 合并顺序与冲突规则

1. **先合底座**：W-21（加载器 + `ExtRegistry` + `YieldReason` 新变体）必须**先**合 —— W-13/W-14/W-20/W-22 都依赖它的接口。
2. **再合能力**：每个能力项自带契约测试，合并时**测试必须同时进**（测试与实现同 PR）。
3. **登记统一做**：所有「新成员 / 新命名空间」的登记（`SPEC` / `LANG_SOURCES` / `resolve()` / `NATIVE_SPECS` / `NAMESPACE_META` / 附录 A 抄件 / `EXPECTED_CASE_COUNT`）由集成者在 W-35 一次性做完，**逐字对拍必须绿**。
4. **冲突判据**：同一文件被两个 agent 改 ⇒ 立即停手，由集成者裁决；**不允许**「谁后合谁覆盖」。

### 5.5 失败处置（不许悄悄降级）

| 情况 | 处置 |
|---|---|
| 某工作项超时 | 拆成「最小可交付」+「余项」两半，**余项必须在 §4 里留条目**，不许从台账上消失 |
| 某工作项被证实不可行 | **写进 §4 的「非目标」并给出实测证据**，同时在 §10 提请业主裁决；**不许**改成「已做」 |
| 规范侧某条 SP 被拒 | 该条对应的能力**降级为 §5.5 台账条目**（不进公共面），**不许**改既有成员绕过去 |

---

## 6 规则侧 v1.0.4 变更清单（交给规范仓批次执行）

### 6.1 条款级改动（两条 + 一条条件项 + 一条登记）

| # | 册 | 坐标 | 改什么 | 性质 |
|---|---|---|---|---|
| **SP-1** | 语言核心册 | `wlwl-v1.0-language.md:1145` + `:1230` | 删掉「`E0014` 在并发下**只**用于一个位置：`YIELD` 出现在任务体外」；把 `E0014` 的触发条件补上「**宿主挂起点成员**在任务体**外任何位置**（含嵌套闭包内）」并注明「诊断**必须**指名该成员、位置指向调用点」（与并发册 `:94` 逐字对齐） | 例外 2（规范缺陷修复） |
| **SP-2**（**条件项**） | 标准库·系统册 | `wlwl-v1.0-stdlib-system.md:813` 附近（§21.6.2 正常值表） | **仅当**实现仍保留「竞争落选 ⇒ `NULL`」语义时才需要：补一行「落选者返回 `NULL`（正常值，与超时同形），**不得**因此终止任务，**不得**连带关闭监听句柄」。⚠ **若按 W-11 的「唤醒一个」实现 ⇒ 竞争不再产生 `NULL` ⇒ 本条作废、规则侧零改动** | 例外 2（缺口清零，措辞**不引入新自由度**：只规定**已发生竞争时**的可观察结果，**不**授权任何新用法） |
| **SP-3** | 标准库·并发册 + 系统册 | `wlwl-v1.0-concurrency.md:147` / `:153` / `:157` | ① `:153` 的「在当前架构下『锁』不是一个可实现的成员」**限定为「语言级锁」**；② `:157` 的「`MUTEX`/`CONDITION`/… 属标准库范畴」**落成标准库册的新成员** `std.sync`（**原子共享容器**形态：计数器 / 单键原子写）；③ `:147` 的「不得通过共享的可变 cell」**明确不涵盖**标准库提供的原子共享容器（那不是语言级 cell） | 措辞澄清 + 新增成员（§3.2 修订版本通道，先例 D-13） |
| **SP-5** | 一致性册 | `wlwl-v1.0-conformance.md:514-532`（§5.5 台账） | **台账第一行**：`tls` / `crypto` 两个命名空间的成员（登记理由 / 效力声明 / 随哪个实现仓发布 / 收敛路线 / 拟进公共面的版本） | 台账**不属公共面**（`:206`）⇒ 不进附录 A |

### 6.2 新成员 / 新命名空间清单（SP-4 的全部内容）

| 命名空间 | 新成员 | 册 |
|---|---|---|
| **`std.http`**（新） | `PARSE_REQUEST` `BUILD_RESPONSE` `SERVE_STREAM` `SERVE` `ROUTE` `MATCH_ROUTE` `PARSE_COOKIES` `SET_COOKIE` `PARSE_FORM` `PARSE_MULTIPART` `STATIC_FILE` `GET` `POST` `REQUEST` `COMPRESS_RESPONSE` `RATE_LIMITER` `RULESET_LOAD` `CLIENT_IP` | 系统册 |
| **`std.log`**（新） | `LOG` `LOG_SET_LEVEL` `LOG_FILE` `LOG_METRICS` | 系统册 |
| **`std.sync`**（新） | `COUNTER` `COUNTER_INC` `COUNTER_ADD` `COUNTER_GET` `SHARED_DICT` `SHARED_DICT_SET` `SHARED_DICT_GET` `SHARED_DICT_SNAPSHOT` | 并发册 |
| **`std.signal`**（新） | `ON_SIGNAL` `STOP` | 系统册 |
| `std.net` | `TCP_SERVE` `HTTP_REQUEST_ASYNC` `TCP_CONNECT` | 系统册 |
| `std.text` | `NFKC` `CASE_FOLD` `STRIP_INVISIBLE` | 文本册 |
| `std.fs` | `PATH_CONTAINS` `PATH_CANONICAL` `WRITE_FILE_ATOMIC` `LOCK_FILE` `UNLOCK_FILE` `FILE_MTIME` `DIR_ENTRIES` | 系统册 |
| `std.str` | `QUERY_PARSE` | 文本册 |
| `std.format` | `FORMAT_NAMED` | 文本册 |
| `std.test` | `ASSERT_MSG` `ASSERT_TRUE` `EXPECT_ERR_CODE` `SKIP` `TEST_CASES` `TEMP_DIR` `RUN_TESTS_ASYNC` | 核心册 |
| `std.regex` | `RE_MATCHES_FULL` | 文本册 |
| `std.collection` | `ARRAY_GET` | 核心册 |

⚠ **附录 A 的改法**：**逐字**改规范仓注册镜像册附录 A 的对应行（成员**按规范表里的顺序**插入，不是追加到末尾）⇒ 再逐字抄到本仓 `docs/wlwl-spec-reference.md:6618-6642` ⇒ `check-appendix-a` 必须绿。
⚠ **总纲 §0.9 登记行**：按 D-13 体例写**一行**（覆盖上表全部新增），条件是 §0.9 有登记行 + 发布说明显式列出（N-1）。
⚠ **不得新增诊断码**（N-3）⇒ 上表所有失败口径走 `ERR` 值或既有码。

### 6.3 版本落点判定（为什么是 v1.0.4 而不是 v1.1.0）

| 判据 | 本轮结论 |
|---|---|
| 新增成员 / 新命名空间 | ✅ 允许于修订版本（N-1 + D-13 先例：17 个新公共面成员走 v1.0.2） |
| 新增诊断码 | ❌ 允许于修订版本（N-3）⇒ **本轮不新增任何诊断码** |
| 修改既有成员的签名 / 可观察语义 | ❌ 禁止（N-4）⇒ **本轮零处修改** |
| 移除 / 弃用既有成员 | ❌ 禁止（N-5）⇒ 本轮零处 |
| 新增章 | ❌ 次版本 ⇒ **本轮不新增册**（全部落进既有册） |
| 破坏性变更 | ❌ 禁止（N-6）⇒ 本轮零处（SP-1 是**修矛盾**，SP-2 是**清缺口**，SP-3 是**澄清 + 新成员**） |

⇒ **v1.0.4 成立**。⚠ 但 `:222`（§4.3）写着「新增成员…次版本递增」⇒ **发布说明必须显式引用 D-13 先例**，否则审阅者会拿 `:222` 打回（§6.4 的变更记录里必须写这一句）。

### 6.4 v1.0.4 的「变更记录」落点（规范仓没有 CHANGELOG）

规范仓 v1.0.3 的发布点写法是 **annotated tag message + `README.md` 的「当前状态」行** ⇒ v1.0.4 照此办理：

1. **tag 注释**：逐条列出 SP-1…SP-5，每条写明「受影响的条款号 / 为什么落在 §3.4 的例外之内 / 迁移写法」（N-7）；
2. **`README.md`「当前状态」行**追加 v1.0.4 段落；
3. **发布说明**显式列出上表全部新增成员与命名空间（N-1 的第二个条件）。

---

## 7 需求清单逐条台账（57 条）

> **状态列**：✅ 做 / 🔧 改口径后做 / 📖 口径答复 / 🔀 归应用层。
> **规则侧列**：`—` = 不动规范；`SP-x` = §6 对应条款。

| # | 需求 | 状态 | 工作项 | 规则侧 | 关键偏差 / 备注 |
|---|---|---|---|---|---|
| A-1 | TLS 服务端 | ✅ | W-21 W-22 | SP-5 | **不进标准库**，走 DLL/SO 扩展；「明文连 TLS 端口 ⇒ 被拒且有明确错误」「证书缺失 ⇒ `ERR` 不 panic」逐条达成 |
| A-2 | HTTPS 出站 | 🔧 | W-20 W-22 | SP-4 | **偏差**：入口是 `std.http::GET`（新成员）而非 `std.net::HTTP_GET` —— 后者的 `https://` ⇒ `ERR` 是规范要求（系统册 `:750`），改它违反 `:223`（§3.7） |
| A-3 | 证书校验原语 | ✅ | W-22 | SP-5 | `tls::CERT_FINGERPRINT`（SPKI）/ `tls::CERT_INFO` |
| A-4 | 加密原语补全 | ✅ | W-22 | SP-5 | 落在扩展的 `crypto` 命名空间（N-21 安全册「对称/非对称加密不做」） |
| A-5 | TLS 安全默认值 | ✅ | W-22 | SP-5 | 校验默认开 / 下限 1.2 / 无「一键全关」/ 私钥不入日志 |
| B-1 | 嵌套闭包挂起点 ⇒ 诊断 | ✅ | W-10 | SP-1 | 规范并发册 `:94` **已经**写对了（要求「任务体外任何位置 ⇒ `E0014`」）⇒ **实现侧今天不合规**（这条检查根本没写，R-27）⇒ W-10 = **新增词法检查**；⚠ 需求描述的「静默终止」复现不出来（R-24）⇒ 用例按**新语义**写。规则侧只修语言册那半边的矛盾 |
| B-2 | 多 acceptor 语义 | 🔧 | W-11 | SP-2（条件） | 惊群**坐实**（R-28）⇒ W-11 改成「**唤醒一个**」修根因 ⇒ 落选者**不再拿 `NULL`**（比需求要的「返回可区分值」更强）；⚠ 真实死法是调用方 `TCP_RECV(NULL)` ⇒ `E0030`（R-29），不是「静默 NULL」 |
| B-3 | 可推进的循环 | ✅ | W-12 | SP-4 | 走需求清单候选 (a) 的**迭代器式**：`TCP_SERVE`（accept 循环在原生帧内推进）。**不需要** `ADR-0017 Step 3` |
| B-4 | 跨任务共享可变状态 | ✅ | W-13 | SP-3 SP-4 | 走候选 (a) 的**共享容器 + 原子更新**（`std.sync`）。**不引入**语言级共享 cell（并发册 `:147`）；`MUTEX`/`CONDITION` 需 Step 3 ⇒ 本轮不做并说明 |
| B-5 | 非阻塞出站 | ✅ | W-14 | SP-4 | 新成员 `HTTP_REQUEST_ASYNC` / `PROCESS_RUN_ASYNC`（宿主挂起点 + 幂等重放）。**不改**既有 `HTTP_*` 的阻塞语义 |
| B-6 | 任务终值可区分 | 🔧 | W-15 | — | 规范 §17.9 **已覆盖**，实现的**三态已具备**（`lib.rs:4734-4763`）⇒ 需求描述的「裸 NULL 不可区分」**不成立**；**但**兜底臂（`lib.rs:10651-10684`）会把丢失的挂起信号伪装成正常 NULL（R-25）⇒ 那是**真缺陷**，本轮修它（**不改** `AWAIT` 语义，撞 `:223`） |
| B-7 | 优雅停机 | ✅ | W-15 | SP-4 | 新命名空间 `std.signal` |
| C-1 | HTTP 请求解析 | ✅ | W-16 | SP-4 | `std.http::PARSE_REQUEST`（含 chunked 请求体**拒绝**） |
| C-2 | HTTP 响应构造 | ✅ | W-16 | SP-4 | `std.http::BUILD_RESPONSE`（内部只认字节数） |
| C-3 | Keep-Alive | ✅ | W-17 | SP-4 | `std.http::SERVE_STREAM` |
| C-4 | 路由 | ✅ | W-18 | SP-4 | `std.http::ROUTE` / `MATCH_ROUTE` |
| C-5 | Cookie | ✅ | W-18 | SP-4 | `PARSE_COOKIES` / `SET_COOKIE` |
| C-6 | 表单 / multipart | ✅ | W-18 | SP-4 | `PARSE_FORM` / `PARSE_MULTIPART` |
| C-7 | 静态文件服务 | ✅ | W-19 | SP-4 | 内建且跨平台正确的穿越防护（走 `PATH_CONTAINS`） |
| C-8 | 请求级预算 | ✅ | W-17 | SP-4 | 四段分别可配 |
| C-9 | 头 / 体大小限制 | ✅ | W-17 | SP-4 | 431 / 413 |
| C-10 | 压缩集成 | ✅ | W-19 | SP-4 | `gzip` / `zstd` 协商 |
| C-11 | chunked 客户端解码 | ✅ | W-20 | SP-4 | **客户端解码**；**服务端继续拒绝**（N-17 / ADR-0028 D3） |
| C-12 | 条件请求 / 部分响应 | ✅ | W-19 | SP-4 | `HEAD` / `304` / `206` |
| D-1 | 正则编译缓存 | 🔧 | W-23 | — | **零新成员、零规范改动**（文本册 `:502` 已允许缓存）⇒ 实现侧内部缓存 + 确定性计数断言 |
| D-2 | 匹配前归一化 | ✅ | W-24 | SP-4 | `NFKC` / `CASE_FOLD` / `STRIP_INVISIBLE` |
| D-3 | 限流与计数 | ✅ | W-25 | SP-4 | 滑动窗口 + 令牌桶（基于 `std.sync` 原子计数） |
| D-4 | 安全路径包含判断 | ✅ | W-24 | SP-4 | `PATH_CONTAINS`（真实路径判定，非前缀） |
| D-5 | 规则集加载 / 热更新 | ✅ | W-25 | SP-4 | 原子切换，不影响在途请求 |
| D-6 | 结构化审计日志 | ✅ | W-25 W-26 | SP-4 | JSON 行 + 请求 ID |
| D-7 | 客户端 IP / 可信代理 | ✅ | W-25 | SP-4 | 可信代理列表内才采信 `X-Forwarded-For` |
| E-1 | 原子写 | ✅ | W-27 | SP-4 | `WRITE_FILE_ATOMIC` |
| E-2 | 文件锁 | ✅ | W-27 | SP-4 | `LOCK_FILE` / `UNLOCK_FILE`（raw FFI，零新增依赖） |
| E-3 | 目录与元数据 | ✅ | W-27 | SP-4 | `FILE_MTIME` / `DIR_ENTRIES` |
| E-4 | 文本补齐 | 🔧 | W-28 | SP-4 | `SPLIT` / `STR` **已存在**（全局内建）⇒ 只补 `QUERY_PARSE` |
| E-5 | 模板 / 格式化 | ✅ | W-28 | SP-4 | **偏差**：既有 `FORMAT` 的语义**不可扩展**（`:223`）⇒ 另起 `FORMAT_NAMED` |
| E-6 | Markdown 渲染 | 🔀 | — | — | **按需求方自己的建议归应用层**（清单 `:321`「**不建议** wlwl 提供 Markdown」+ `:322`「我们的方案：TinyPage 侧自研」）⇒ 口径：**wlwl 不提供**；需求方自研 |
| E-7 | 代码高亮 | 🔀 | — | — | 同 E-6（清单 `:325`「建议明确不做」+ `:327`）⇒ 口径：**wlwl 不提供** |
| F-1 | 结构化日志 | ✅ | W-26 | SP-4 | 新命名空间 `std.log` |
| F-2 | 运行时指标 | ✅ | W-26 | SP-4 | `LOG_METRICS` |
| F-3 | 任务状态检视 | ✅ | W-30 | — | `wlwl tasks`（只读检视端点） |
| F-4 | 错误信息可读性 | ✅ | W-33 | — | 源码行片段 + 列号高亮 + 提示语 |
| G-1 | `std.test` 增强 | ✅ | W-31 | SP-4 | 新增成员；**不改** `ASSERT` 既有语义 |
| G-2 | 静态检查新增规则 | ✅ | W-32 | — | 规则 ① 复用 `E0014`；规则 ② 走**「规范符合性提示」**（**无 E/W 码**，稳定名 `shared-mutable-binding`，两端共用同一收集函数、**不阻塞退出码**）+ 运行期 `ERR` 兜底 ⇒ ✅ **业主 2026-10-27 裁决「v1.0.4 包含」**（§10 Q4）⇒ 规则侧**零改动、零新码** |
| G-3 | 调试器 / REPL | ✅ | W-30 | — | 按需求清单的**最小口径**（`wlwl repl` + 挂起点查看） |
| G-4 | 模块组织 | ✅ | W-33 | SP-4 | 模块私有成员 / 循环导入诊断 / `.wll.sig` 工作流 |
| H-1 | `IF` 第三实参多语句组 | 🔧 | W-34 | — | **已支持** ⇒ 只补回归守护 + 文档口径 |
| H-2 | 数字 → 字符串 | 🔧 | W-34 | — | 全局内建 `STR` 已有 ⇒ 文档 / 技能包订正 |
| H-3 | 通用 `SPLIT` | 🔧 | W-34 | — | 全局内建 `SPLIT` 已有 ⇒ 同上 |
| H-4 | 路径分隔符统一 | 🔧 | W-24 W-34 | SP-4 | **不改** `PATH_NORMALIZE` ⇒ 新增 `PATH_CANONICAL` |
| H-5 | `RE_TEST` 命名 | 🔧 | W-29 | SP-4 | **不改名** ⇒ 新增 `RE_MATCHES_FULL` + 文档首行标注 |
| H-6 | 容器取值 API 统一 | 🔧 | W-29 | SP-4 | **不统一既有** ⇒ 新增 `ARRAY_GET` + 错误提示 |
| I-1 | HTTP/2 | 📖 | — | — | **口径**：v1.0.x 不做。HTTP/1.1 的三个已知短板（chunked 服务端拒绝 / 无推送 / 无多路复用）由 `std.http` 的 Keep-Alive + 206 覆盖到需求方的场景；若需求方必须要 HTTP/2 ⇒ 独立立项（需要 `std.http` 的流式升级支持） |
| I-2 | WebSocket | 📖 | — | — | **口径**：不做。需求方原 Python 版的实时预览**本来就因 P0-1 不可用**（清单 `:325`）⇒ 不构成迁移阻塞；要做的前提是「流式响应体」（`std.net` 明确不做，N-15）⇒ 属 `ADR-0017 Step 3` 之后 |
| I-3 | `TCP_CONNECT` | ✅+📖 | W-14 | SP-4 | **本轮落地** `std.net::TCP_CONNECT`（出站必需：HTTPS 与 DB/Redis 都靠它）⇒ 口径：**v0.12.1 交付** |
| I-4 | brotli | 📖 | — | — | **口径**：不做。`std.compress` 现有 zstd / gzip / zlib / raw deflate；brotli 需要新增第三方依赖（破 `ADR-0022` 画像）⇒ 需求方用 `gzip` / `zstd` 协商即可 |
| I-5 | HTTP/3 | 📖 | — | — | **口径**：不做。前提是 HTTP/2（I-1）+ UDP 传输族（`std.net` 只有 TCP）⇒ 属独立立项 |

---

## 8 门禁与验收

### 8.1 实现仓（本轮验收命令）

```powershell
cd impl
cargo fmt --all -- --check                                    # 期望:0 diff
cargo clippy --locked --workspace --all-targets -- -D warnings # 期望:0
cargo test --locked --all-targets                             # 期望:0 failed
cargo test --locked -p wlwl-cli --test probe                  # 期望:181+N 绿
cargo test --locked --test conformance -p wlwl-cli            # 期望:绿
cargo deny --locked --all-features check                      # 期望:exit 0
$env:RUSTDOCFLAGS='-D warnings'; cargo doc --workspace --no-deps # 期望:0 error
python ../script/check_conformance_registry.py "$env:WLWL_SPEC_V1_DIR"   # 期望:exit 0
cargo run --locked --bin check-appendix-a -- ../docs/wlwl-spec-reference.md  # 期望:exit 0
cargo test --locked -p wlwl-eval --test stdlib_appendix_a_sync             # 期望:绿
cargo check --locked -p wlwl-std --features real-ai              # 期望:exit 0
cargo test --locked -p wlwl-tls-ext                              # 期望:绿（TLS 扩展自测）
```

⚠ **`impl/Cargo.lock` 不入库**（`.gitignore`）⇒ 依赖集读数**不可复现** ⇒ 本轮**新增依赖面**（TLS 扩展的 `rustls` / `ring`）⇒ **必须在 W-35 登记锁文件策略**（要么入库，要么在发行说明里钉死版本）。
⚠ **`wlwl-tls-ext` 不在主 workspace**（它是 `cdylib`，独立构建）⇒ 上面两条命令要单独在 `impl/ext/wlwl-tls-ext` 目录跑。

### 8.2 规范仓（v1.0.4 的验收命令）

```powershell
cd D:\Project\wlwl-spec-v1
python script/check_spec.py                     # 附录 A ↔ 各册成员表 集合相等
# 另需人工确认:总纲 §0.9 新增登记行存在且「实现状态」= 已实现、发布说明显式列出全部新增成员、§5.5 台账第一行存在
```

### 8.3 反向守卫（每条都要做负向验证）

1. **W-10**：去掉「当前帧不是任务体顶层 ⇒ `E0014`」判定 ⇒ 新用例**必红**（回到段首重放 + 副作用重复）；
2. **W-11**：把 `wake_socket_waiter` 改回「唤醒全部」⇒ 新用例**必红**（落选者拿 `NULL` ⇒ `TCP_RECV(NULL)` ⇒ `E0030`）；
3. **W-12**：把 `TCP_SERVE` 改回「逐条读」⇒ 新用例**必红**；
4. **W-13**：把原子自增改回「先读后写」⇒ 计数**必错**；
5. **W-14**：去掉幂等判定 ⇒ 出站请求**必重复**；
6. **W-21**：去掉绝对路径校验 / 去掉 `std.*` 影子拒绝 ⇒ 自测**必红**；
7. **W-22**：打开「明文降级」⇒ 用例**必红**；
8. **W-23**：去掉模式缓存 ⇒ 编译计数**必红**（**用确定性计数，不用墙钟** —— 一致性册 §6.2 V-01）；
9. **W-24**：把 `PATH_CONTAINS` 改回前缀比较 ⇒ `D:/root-evil` 用例**必红**；
10. **W-27**：把原子写改回直接写 ⇒ 中断后**必留半文件**；
11. **W-28**：把 `FORMAT_NAMED` 改回「只认第一个 DICT」⇒ 用例**必红**；
12. **W-35**：把附录 A 抄件改一个字 ⇒ `check-appendix-a` **必红**；
13. **SP-1…SP-5**：规范侧改动 ⇒ **附录 A 逐字对拍门禁必须仍绿**（证明没偷加公共面成员）。

---

## 9 风险

| 项 | 定性 | 处置 |
|---|---|---|
| **扩展面从零建** | 全仓无任何 FFI / 动态库机制（R-14），且 `StdFn` 是静态函数指针（R-15）⇒ 需要「运行期成员注册表」这一层新抽象 | W-21 单独一个工作项、先合；ABI 版本化 + `struct_size` 前向兼容 |
| **扩展 = 任意代码执行** | 加载原生库即交出全部权限 | 信任模型七条（T1…T7）；**语言面不提供 `LOAD`**；文档明说「不是沙箱」 |
| **B 组的三条形态选择** | 需求清单给的是「候选清单」，本轮选了**不需要 Step 3** 的那三条 ⇒ 形态与需求方的原始想象有差（见 §7 的偏差列） | 已逐条写明偏差与理由；需求方确认口径见 §10 |
| **`std.http` 的性能** | R1（解释执行）⇒ 比 R2 慢 | 以基准（bench 冒烟已在 CI）+ 需求方的 QPS 基线验收；热点下沉 R2 是既有纪律（ADR-0021） |
| **`HTTP_*` 的 `https://` 仍是 `ERR`** | 与需求清单 A-2 的字面判据不同 | §3.7 已写明偏差与「要 v1.1.0」的排期口径 |
| **附录 A 冻结快照** | 任何「顺手改一下既有成员」都会破 `:223` | 全部走**新增**；守卫见 §8.3 第 12 条 |
| **不得新增诊断码** | G-2 规则 ② 因此无法在 1.0.x 做静态版 | 运行期 `ERR` 先落地；静态那条见 §10 Q4 |
| **`impl/Cargo.lock` 不入库** | 依赖集读数不可复现 | W-35 登记锁文件策略 |
| **B-1 的需求前提被实测推翻** | 「嵌套闭包 ⇒ 静默终止」在当前代码上**复现不出来**（R-24）⇒ 若照抄需求清单写用例，会得到一个「永远绿」的假验收 | 用例按**新语义**写（要么跑通、要么 `E0014` 指名成员 + 指向调用点）；§2.3 R-24 记录了实测读数与其边界（用的是仓内已构建的 release 二进制，**未核对它与 HEAD 的构建来源**） |
| **`pending_ready` 疑似死代码** | 「事件先于 park 到达」的补偿在当前 pump 路径下**不可达**（R-33，**读码判读，未定向实测**）⇒ 潜在的丢事件 | 随 W-11 一并复核：若坐实 ⇒ 让 `park_for_socket` 直接查 `READY` 集合 |
| **规模** | **32 个工作项**（⚠ 原写 28 —— 后续新增 W-35(D3) / W-36 / W-37 / W-38 四项）、6 个波次、约 4 周的并行工作量（§5.2） | 波次内并行、波次间串行；§5.5 规定失败不许悄悄降级 |

---

## 10 业主裁决记录（2026-10-27，四项**全部已定**，本轮无待决项）

> 业主以**点选**方式逐项确认，以下为回执。**四项全部按推荐项落地**；Q4 业主另加约束「**不许漂到 v1.1.0**」，据此把规则 ② 改成**不需要新诊断码**的交付形态。

| # | 问题 | 业主裁决 | 落地动作 |
|---|---|---|---|
| **Q1** | TLS 交付形态 | ✅ **动态库扩展**（独立 Rust `cdylib` + 宿主 `--ext` 显式授权加载 + 一致性册 §5.5 台账第一行） | §3 全章按此写；W-21（加载器）+ W-22（TLS crate） |
| **Q2** | 并发三项（B-3/B-4/B-5） | ✅ **按需求清单候选形态做**，**不引入 `ADR-0017` Step 3** | W-12（`TCP_SERVE` 迭代器式）/ W-13（`std.sync` 原子共享容器）/ W-14（`HTTP_REQUEST_ASYNC` 等宿主挂起点式） |
| **Q3** | 新成员落哪 | ✅ **放 `std.*`**（动附录 A + 各册成员表 + 总纲 §0.9 一行登记，先例 D-13） | §6.2 的成员清单；W-35 统一登记 |
| **Q4** | G-2 规则 ② 静态检查 | ✅ **v1.0.4 包含**（业主原话：「不要自作主张，我说 v1.0.4 包含就按我说的」） | 交付形态改为**「规范符合性提示」**（**无 E/W 码**、稳定名 `shared-mutable-binding`、两端共用同一收集函数、**不阻塞退出码**）+ 运行期 `ERR` 兜底 ⇒ **规则侧零改动、零新码、版本不动**（W-32） |

⚠ **Q4 的代价（业主已知悉）**：规则 ② 的静态提示**不改变退出码** ⇒ CI 里靠 grep/日志发现它；真正挡住违规用法的是同轮的**运行期拦截**（`ERR(kind:"SharedMutableViolation")`）。若日后要求它**阻塞 CI**，就必须新增诊断码 ⇒ 那时再单独裁决版本落点。

### Q1 · TLS 的交付形态：动态库扩展 + 宿主显式加载（**业主已确认**）
**我的倾向**：按业主 2026-10-27 的指示执行 —— **TLS 不进标准库**，做成独立 `cdylib`，宿主用 `--ext` 显式加载，落 §5.5 台账。
**理由**：① 六条证据（§3.1）证明进标准库要同时破 `std.net` 的「不做 TLS」、安全册的「不做加密」、`ADR-0028 D3` 的零依赖纪律；② 做成动态库后**发行二进制的依赖面不变**，TLS 库可独立跟 CVE、独立升级；③ 台账机制（一致性册 `:514-532`）**本来就是为这种扩展准备的**，本轮给它第一行。
**若您要别的形态**：① 把 TLS 直接链进 `std.net`（则要破三条裁决 + 改附录 A + 版本可能要 v1.1.0）；② 用进程外 TLS 终结器（则 A-1/A-2 不算「实现」，需求方的 P0 仍悬着）。

### Q2 · B-3 / B-4 / B-5 的形态（**业主已确认：不走 `ADR-0017 Step 3`**）
**我的倾向**：按需求清单**自己列的候选**落地 —— B-3 迭代器式（`TCP_SERVE`）、B-4 共享容器 + 原子更新（`std.sync`）、B-5 宿主挂起点式新成员。**不引入 Step 3**。
**理由**：① 三条都在需求清单的候选清单里，**验收判据全部可达**；② Step 3 是「流式协程」路线的独立立项（需求清单的 I-2 WebSocket / SSE 也依赖它），把它塞进 v0.12.1 会让本批的交付确定性崩掉；③ 本轮形态的**代价**要说清：`TCP_SERVE` 的 handler 内**不得**使用宿主挂起点（否则段重放会重复处理）—— 这是一个真实限制。
**若您要 Step 3**：它会变成 v0.12.1 的关键路径，且按既有实测规模（`runtime.rs` 1726 行 vs ADR 估 718）需要单独排期。

### Q3 · 新成员落 `std.*`（动附录 A）还是落参考实现（**业主已确认：落 `std.*`**）
**我的倾向**：**落 `std.*`**（`std.http` / `std.log` / `std.sync` / `std.signal` + 既有命名空间的新成员），按 D-13 先例走 **v1.0.4 修订版本**。
**理由**：① 需求清单**逐条点名了 `std.*` 命名空间**（`std.net` 或新 `std.http`、`std.encode` 或新 `std.tls`、`std.test` 增强、`std.log`、`std.process` 或 `std.signal`）；② 「参考实现」路线等于把 57 条的工作量推给每个使用者各写一遍 —— 需求清单 §0 的整篇论证就是为了避免这个；③ 附录 A 的改法是**机械的**（逐字改 12 行 + 抄件 + 对拍）。
**风险**：`:222` 与 `:130` 的版本口径打架 ⇒ 发布说明**必须显式引用 D-13 先例**（§6.3）。

### Q4 · G-2 规则 ② 的静态检查 —— ✅ **业主已裁决（2026-10-27）：v1.0.4 包含**
**业主原话**：「不要自作主张，我说 v1.0.4 包含就按我说的」⇒ **本项进 v0.12.1 / 规范 v1.0.4，不许漂到 v1.1.0**。

**交付形态（采纳「不需要新诊断码」的那条路 ⇒ 规则侧零改动、零新码、v1.0.4 原样成立）**：

| 维度 | 做法 | 为什么合规 |
|---|---|---|
| **产出物** | **「规范符合性提示」**（named advisory）：带**稳定名称** `shared-mutable-binding` + 位置 + 说明，**不带 E/W 诊断码** | 「诊断码」有专门定义（语言册 §11 的码表）⇒ 提示**不是**诊断码 ⇒ 一致性册 `:129` 的「新诊断码 ⇒ 次版本」**不触发** |
| **工具一致性** | `wlwl check` 与 LSP **共用同一个收集函数**（把它从「返回诊断列表」扩成「返回 `(诊断, 提示)`」）⇒ 两者拿到**同一份**提示 | 语言册 `:1681`「编辑器里的划线**不会比命令行少**」被满足；**不**制造「编辑器不报、CI 报」的分裂 |
| **退出码** | 提示**不改变退出码**（与警告同款语义：语言册 `:1262`「警告**不得**改变程序语义」）⇒ 退出码表（§11.4）**一字不动** | 退出码表按诊断码映射 ⇒ 没有码就没有映射项 |
| **严重级** | LSP 侧映射为 `severity: Warning`；CLI 侧单列一节「规范符合性提示」 | LSP 的 `Diagnostic.code` 本就可选（协议允许任意串）⇒ 不必借用既有 E/W 码 |
| **兜底** | 运行期拦截同轮落地：`ERR([kind: "SharedMutableViolation", reason: …])` ⇒ 违规用法在运行期照样被拦下 | `kind` 是开放字符串 ⇒ 零规范成本（§2.3 R-12） |

⇒ **规则侧净改动 = 0**：本项**不进** SP-1…SP-5 任何一条，**不进**附录 A，**不进**总纲 §0.9，**不动**语言册的码表。
⚠ **代价要说清**：这条提示**不阻塞**退出码（CI 里要靠 grep/日志发现，或用运行期拦截兜底）—— 这是「不新增诊断码」前提下必然的取舍。若业主后续要求它**阻塞 CI**，那就必须新增诊断码 ⇒ **必须**升 v1.1.0 ⇒ 那时再单独裁决。

---

## 11 收尾交接（六波落地完毕：22 个工作项）

### 11.1 六波已落地（22 个工作项）

| 工作项 | 落地内容 | 验收读数 |
|---|---|---|
| **W-01** 基线自洽（我做） | 三条门禁基线 + 死链订正 | `check-appendix-a` OK · `stdlib_appendix_a_sync` 8/0 · `§5.4 状态列与契约表一致（25 行）` · probe 181 = 常量 |
| **W-02** 探针固化 | `docs/wlwl-feasibility/probes/`（9 个探针 + README 对照表）+ 9 个 `P121_probe_*` 夹具 | 9/9 绿（debug + release 双二进制）· 反向守卫验过 |
| **W-21** 扩展加载器 | 新 crate `impl/crates/wlwl-ext/`（`abi.rs` / `dynlib.rs` / `trust.rs` / `json.rs` / `registry.rs` / `include/wlwl_ext.h`）+ `wlwl-value` 的 `ExtRegistry` + eval 侧第 4 层解析 + CLI `--ext` / `--ext-sha256` | `wlwl-ext` 5/0 · `ext_loader_contract` 22/0 · 两条反向守卫都红过 |
| **W-23** 正则编译缓存 | `regex.rs` 进程级编译缓存（条数 256 + 字节 4 MiB 双上界、LRU） | `regex_cache_contract` **14/0**（我复核）· 反向守卫把编译次数 1 → 1001 |
| **W-24** 归一化 + 路径包含 | `text.rs`（`NFKC`/`CASE_FOLD`/`STRIP_INVISIBLE`）+ `fs.rs`（`PATH_CANONICAL`/`PATH_CONTAINS`）+ `unicode/waf.rs`（**生成物**，仓内**没有** NFKC 数据底座）+ `script/gen_unicode_waf_tables.py` | `path_contains_contract` **10/0**（我复核）· 反向守卫 8 条断言红 |
| **W-26** `std.log` | `log.rs`（`LOG`/`LOG_SET_LEVEL`/`LOG_FILE`/`LOG_METRICS`/`METRIC_COUNTER`/`METRIC_INC`/`METRIC_TIMER`）+ 契约测试 | agent 21/0 · 两条反向守卫（原子自增 / 级别过滤）都红过 |
| **W-27** 原子写 + 文件锁 + 元数据 | `fs.rs`（`WRITE_FILE_ATOMIC`/`LOCK_FILE`/`UNLOCK_FILE`/`FILE_MTIME`/`DIR_ENTRIES`）+ 契约测试 | `fs_atomic_lock_meta_contract` **20/0**（集成者修掉 agent 的一处自递归后） |
| **W-22** TLS 扩展 crate | `impl/ext/wlwl-tls-ext/**`（独立 crate + 自带 `Cargo.lock`；`tls` 10 员 + `crypto` 8 员；`abi.rs` 用 `#[path]` 引用宿主定义） | **32/0**（12 crypto + 17 tls + 3 ABI 布局）· 宿主联调通过（`--ext` 加载 + 两种 IMPORT 形态）· 三条反向守卫都红过 · **主仓依赖面零变化** |
| **W-10** B-1 嵌套挂起点诊断 | `wlwl-eval/src/lib.rs`：`task_body_depth` + 四个闭包调用点的帧维护 + **调用时**判定（不是「真挂起才报」）+ 复用既有文案/位置合成 | `suspend_site_contract` **13/0** · 反向守卫「去掉判定 ⇒ 恰好那 6 条红、7 条合法用例仍绿」 · probe 套件 `pass=180 deviation=12 fail=0` |
| **W-16** `std.http` 骨架 | `wl/std/http.wll`（494 行，22 个助手；R1 纯 wlwl）—— ⚠ **集成者误删后由修复 agent 重写** | `http_contract` **15/0** · 两条反向守卫（`BYTE_LEN`→`LEN` ⇒ 5≠6；framing 改回 ⇒ 3 条红） |
| **W-13** `std.sync` 原子容器 | `wlwl-std/src/sync.rs`（467 行，8 员；句柄表**不存 `Value`**，用 `Send` 安全的镜像树） | `sync_contract` **21/0** · 反向守卫 `3998 ≠ 4000` |
| **W-28** `QUERY_PARSE` + `FORMAT_NAMED` | `wl/std/str.wll` 加 `QUERY_PARSE`（复用 `URL_DECODE`）· `src/format.rs` 加 `FORMAT_NAMED`（**既有 `FORMAT` 消息逐字未变**） | `str_format_contract` **14/0** · `format_contract` **11/0** · 反向守卫 `1/{b}` |
| **W-12** `TCP_SERVE`（B-3 迭代器形态） | `net.rs` 新成员 + `tcp.rs` 下沉原生帧阻塞原语（只搬字节、不认协议） | `tcp_serve_contract` 16/0 · `tcp_contract` 23/0 · `socket_handle_guard` 11/0 · 反向守卫 2 条红 |
| **W-36** `std.http` 重复 `Content-Length` 收紧（D2 · 第三波） | `wl/std/http.wll` 的 `_CL_INT` 改为「同名多值一律拒（含值相同）」+ 返回形由整数哨兵改 DICT（**内部助手，未导出** ⇒ 成员面不变）;`http_contract.rs` 新增 3 条 | `http_contract` **15/0**（断言 37 → 40）· 反向守卫「改回『值相同也接受』⇒ 新用例必红」· `std.net` **零改动** |
| **W-35** `p10`/`p11`/`p13` 改写成 `TCP_SERVE` 形状（D3 · 第三波） | 三个探针 + 三份夹具副本（**成对逐字相同**，SHA256 已核）;头部注释重写为**给需求方的迁移指引** | probe 套件 **3/3（192 用例）全绿** · 正面读数恢复（8 并发全 200 / 403×3 + 200×1 / 4 条全 200）· 反向守卫「handler 改回旧形状 ⇒ case 红」 |
| **W-37** 解析器 `E0011`「误报」—— **证伪完成**（D4a · 第三波） | ⚠ **解析器源码零改动**;新增 `impl/crates/wlwl-parser/tests/e0011_false_positive_contract.rs` **7 条**契约 | `wlwl-parser` **208/0** 全绿 · **79 个合法形态 0 误拒** · 反向守卫「少一个 `)` ⇒ 仍报 `E0011`」 |
| **W-11** B-2 惊群修根因（**唤醒一个** · 第四波） | `runtime.rs`：`wake_socket_waiter` 只唤醒 **FIFO 队首一个** + 其余留在名单里;`park_for_socket` 去重;`tcp.rs`：就绪电平**重新推高**（否则剩下的连接永远等不到唤醒）;新契约 `b2_accept_wakeup_contract.rs` + 新并发夹具 `tcp_accept_single_wakeup.wll` | `b2_accept_wakeup_contract` **4/0** · 并发 **19/0** · `socket_wait_contract` 16/0 · `tcp_contract` 23/0 · `suspend_site_contract` 13/0 · ⚠ **规范侧零改动**（**SP-2 因此作废**） |
| **W-14** B-5 非阻塞出站（**第四波**） | 新成员 `HTTP_REQUEST_ASYNC` / `PROCESS_RUN_ASYNC`（`net.rs` + `process.rs`）;`wlwl-value` 加 `YieldReason::AsyncReady` + `Tag::AsyncReady`;求值器侧加**独立的** `async_waiters` 表 + 就绪泵（**不碰** `socket_waiters`） | `b5_async_outbound_contract` **12/0** · 事件驱动：快 handler **2 ms** / 慢请求 **10002 ms** / 慢请求只发 **1 次** · 反向守卫（去幂等 ⇒ 计数 **2**）· ⚠ **新增 2 个公共面成员** ⇒ 待规范侧登记 |
| **W-15** B-6 修兜底臂 + B-7 优雅停机（+ W-38 诊断化 · **第五波**） | 兜底臂改成按「**是不是 `Signal::Yield(_)`**」判（不按变体枚举）+ 判据抽成 `runtime::classify_task_completion`;**新命名空间 `std.signal`**（`wlwl-std/src/signal.rs` 408 行，原子待处理位 + 平台映射，**零新增依赖**）+ 两处登记;原生帧回调时清掉「在任务体内」标志 ⇒ W-38 报**既有 `E0014`** | `b6_task_result_contract` **7/0** · `b7_graceful_stop_contract` **10/0** · `w38_native_frame_suspend_contract` **11/0** · ⚠ **新增 2 个公共面成员 + 1 个新命名空间** |
| **W-38** `TCP_SERVE` 的 handler 里调宿主挂起点成员 ⇒ 诊断化（**第五波** · 由 W-15 一并落地） | 原生帧回调进 WLWL 代码时清掉「在任务体内」的标志 ⇒ 走 W-10 的**既有**判据 ⇒ 报**既有 `E0014`**（指名成员 + 指向调用点）;`TCP_SERVE` 的既有可观察语义**未变**（误用的连接仍不算被服务、失败仍只记一笔继续下一条） | `w38_native_frame_suspend_contract` **11/0** · ⚠ **反向守卫**：去掉「清标志」⇒ 必红（回到静默作废）· ⚠ agent 原版的测试有占位符 bug（`replace("PORT", …)` 把 `IMPORT` 也替换了 ⇒ 端口从未 listen ⇒ 挂死）⇒ 已改 `__PORT__` |
| **W-32** `wlwl check` 静态检查新增两条规则（G-2 · **第七波**） | `wlwl-types/src/{check.rs,diag.rs,lib.rs}`：规则① 静态发现「宿主挂起点成员在任务体外（含嵌套闭包内）」⇒ 复用**既有 `E0014`**（新增 `TypeDiagKind::HostSuspendOutsideTask` + `fixed_code()`，因码表里**无 `W0014`**），消息**逐字抄运行期**;规则② 静态发现「跨任务共享可变绑定」⇒ **无 E/W 码**提示 `shared-mutable-binding`;`cli/src/main.rs`：`collect_static_diagnostics` 返回类型扩成 `StaticDiagnostics{diags,notices}` ⇒ **7 处调用点一次改完**（LSP ≡ CLI）;`cli/src/lsp.rs`：提示映射 `severity: Warning` | `wlwl-types` **144/0** · `lsp_smoke` 2/0 · `suspend_site` 13/0 · `spec_r2_conformance` 11/0 · probe **3/3（195 用例）** · **三条反向守卫全红过**（含「拆成两份收集函数」）· ⚠ **取舍**：`check` 拦（exit 1）、`run` 不报不拦（既有 `p07`/`p09` 钉死「跑一半 + 运行期报同一条」）· ⚠ **两条静态/动态已知差异**（顶层漏报 · 「成员跑完才判」的先后不存在） |
| **W-29** H-5 / H-6 别名（**第五波**） | `std.regex::RE_MATCHES_FULL`（= `RE_TEST`，**整串锚定**语义逐字复用）;全局内建 `ARRAY_GET`（= `INDEX_GET`，**同一函数指针** ⇒ 0-based / E0031 / E0036 / E0037 不重实现）;`AT_K` 的 `E0030` 与 `INDEX_GET` 的 `E0036` **消息文本**加「正确 API」提示（**码不动、不新增码**）;附录 G **重生成** | `h5_h6_alias_contract` **18/0** · `wlwl-eval --lib` **915/0**（含 registry 双向锁）· ⚠ **新增 2 个公共面成员** + 附录 G 变化 ⇒ 待规范侧登记 |
| **W-33** 诊断可读性 + 模块组织（F-4 / G-4 的一部分 · **第六波**） | `cli/src/diag_output.rs`：源码行 + **列号高亮**（⚠ 列号是**字节列** ⇒ 需字节→字符折算）;把 `suggestion_code` 里的修法**上浮**为人形态 `= hint:` 行（措辞沿用实现侧原文）;`eval/src/lib.rs`：模块内诊断补源码行 + `E0041` 循环导入的位置改指**闭合环那条 IMPORT** | `P121_f4_g4_diag_module` 通过 · `wlwl-error` **65/0** · `parser_syntax_contract` **20/0** · 反向守卫（关高亮 ⇒ 必红）已做 · ⚠ **json/jsonl 逐字节不变**（除循环导入那条）· ⚠ **G-4 另三项做不了**（文件归属在别人处 ⇒ 已登记）· ⚠ **第七波补做「模块私有成员」⇒ 证伪结案**（见下） |
| **W-30** `wlwl tasks` 任务检视 + 最小 REPL（F-3 / G-3 的一部分 · **第六波**） | `eval/src/task.rs` 新增 `started_at_ms`（**绝对单调毫秒**，填在唯一构造器里 ⇒ 不可能漏填）;`eval/src/runtime.rs`：任务快照（挂起点**穷举 7 个 `YieldReason` 变体** / 时长 / 任务树，纯数据可跨线程）+ **11 个状态迁移点**发布活快照;`cli/src/tooling.rs`：只读检视端点（**只绑回环 / 只读 / 无执行能力**，零新增依赖）;`cli/src/main.rs`：`Cmd::Tasks` / `Cmd::Repl` + `ReplSession`（共用一个 `Evaluator` ⇒ **变量跨次保留**） | `P121_f3_g3_inspect_repl` 通过 · `wlwl-eval --lib` **920/0** · 反向守卫**两层**（单测 + probe）都红过 · ⚠ **`SPAWN` 必须在 `SCOPE` 内** ⇒ 跑完看不到 `suspended` ⇒ 端点是**活读数** · ⚠ Windows 二次 bind 不报错 ⇒ 加了**绑前探测** |

### 11.2 未做的登记（**规范侧**，下一轮按此顺序做）

新成员累计 **30 个**（`std.text` +3 · `std.fs` +7 · `std.net` +1 · `std.format` +1 · `std.str` +1 · `std.log` 7 · `std.sync` 8 · `std.http` 2）。
⚠ **这些成员在规范仓落表前，实现侧的三道对拍闸会红**（`check-appendix-a` / `stdlib_appendix_a_sync` 的成员集合 / `net_contract`·`fs_contract`·`str_math_contract`·`text_contract` 的成员表）⇒ **那是「规范侧未跟上」，不是实现缺陷**。

⚠ **扩展命名空间不在这张表里**（`tls` 10 员 + `crypto` 8 员）⇒ 它们**不进附录 A、不进公共面清单**，只进**一致性册 §5.5 台账**（步骤 ①）—— 业主 2026-10-28 裁决 **D1：`tls::LOCAL_ADDR` 保留** ⇒ 台账按 **10** 个成员写。

| 步骤 | 在哪个仓 | 做什么 | 顺序 |
|---|---|---|---|
| ① | **规范仓** | 一致性册 `§5.5`「实现侧扩展台账」**加行**（新成员的「预期差异」只能挂那里；TCP 那 9 个成员当年就是这么挂的 ⇒ `read_ledger()` 直接从规范仓读） | 1 |
| ② | **规范仓** | 注册镜像册**附录 A** 补表体行（`std.text` / `std.fs` / `std.net` / `std.format` / `std.str` 加成员 + `std.log` / `std.sync` / `std.http` 三行新命名空间） | 2 |
| ③ | **规范仓** | 各册成员表补行（`std.text` §6 · `std.fs` §2 · `std.net` §21 · `std.format` §4 · `std.str` §6 · `std.log`/`std.sync`/`std.http` 新节）+ 总纲 §0.9 登记行（引用 D-13 先例）+ 并发册 §17.0.1 补「标准库原子容器不是语言级共享 cell」那句澄清 | 3 |
| ④ | **实现仓** | 抄回 `docs/wlwl-spec-reference.md`（附录 A + 各册成员表）+ 改 `net_contract.rs` / `fs_contract.rs` / `str_math_contract.rs` / `text_contract.rs` 的期望名单 | 4 |
| ⑤ | **实现仓** | `.github/workflows/ci.yml:121` 的规范仓钉死提交**前移**到 ①–③ 的提交 ⇒ ⚠ **必须等规范仓先 push**，否则实现仓 CI 读到旧规范提交而红 | 5（最后） |

### 11.3 下一批 agent 名单与切分（第二波之后刷新）

| 批次 | 工作项 | 并行度 | 文件冲突提示 |
|---|---|---|---|
| **下一批（业主裁决新增的三项优先）** ✅ **第三波已落地（2026-10-28 下午）** | **W-37**（D4a 解析器 `E0011` 误报：`wlwl-parser` + `wlwl-eval` 的诊断面）· **W-36**（D2 `std.http` 重复 `Content-Length` 收紧：`wl/std/http.wll` + 契约测试）· **W-35**（D3 三个探针改写成 `TCP_SERVE` 形状：`docs/wlwl-feasibility/probes/p1{0,1,3}-*.wll` + `impl/tests/probe/cases/P121_probe_p1{0,1,3}_*/`） | **3 并行**（三个文件面互不重叠 ✓ —— 第三波**实测确认**，真并行 3） | ⚠ W-36 与 W-35 都碰 `http.wll`？**不碰** —— W-36 只改 `_CL_INT` 口径，W-35 只改探针程序 ⇒ 但两者都要跑 `http_contract` ⇒ 合并顺序：W-36 先合，W-35 后合。**第三波实测**：真并行、无一处同文件争用 |
| 下一批 | **W-11**（B-2 唤醒一个：`std/tcp.rs` + `eval/runtime.rs`）· **W-14**（B-5 异步出站：`std/net.rs` + `std/process.rs` + `wlwl-value`）· **W-15**（B-6/B-7 终值+停机：`std/signal.rs` 新建 + `eval/runtime.rs`）· **W-29**（别名：`std/regex.rs` + `std/collection.rs` + `eval/registry.rs`）· **W-31**（`std.test` 增强：`wl/std/test.wll` + `std/test_native.rs`） | **4 并行**（W-11 与 W-15 抢 `eval/runtime.rs` ⇒ 二选一先做） | ⚠ `eval/runtime.rs`：W-11 · W-15 · W-30 撞 |
| 随后 | **W-33**（诊断可读性 + 模块组织：`cli/diag_output.rs` + `error/lib.rs` + `eval/lib.rs`）· **W-30**（`wlwl tasks` + REPL：`eval/runtime.rs` + `cli/main.rs` + `cli/tooling.rs`）· **W-32**（静态检查两条规则：`cli/main.rs` + `wlwl-types/**`） | **3 并行**（W-33 与 W-30 抢 `eval/lib.rs`/`cli/main.rs` ⇒ 二选一先做） | |
| 再随后 | **W-17 / W-18 / W-19 / W-20**（C-3…C-12 的 `std.http` 扩展）· **W-25**（D-3…D-7 中间件） | **2–3 并行** | ⚠ 四项都改 `wl/std/http.wll` ⇒ **必须串行**（一节一节做） |
| 收口 | **W-34**（文档 / 技能包：含本轮三项裁决的对外口径 + `TCP_SERVE` 迁移指引）· **W-35'**（收口登记 + 附录 A 抄件） | 2 | 规范侧 v1.0.4 批次按 §11.2 五步走 |

### 11.4 业主裁决记录（2026-10-28 12:36，四项**全部已定**）

> 业主以**点选**方式逐项确认，**四项全部采纳推荐项**。本轮**只修订文档，不执行**（代码 / 规范侧一律不动）⇒ 每条决策下面都附「落到哪一项工作项」。

| # | 问题 | 裁决 | 落地去向（本轮**未执行**） |
|---|---|---|---|
| **D1** | `tls` 里多出的 `LOCAL_ADDR`（问监听器实际占的端口）留不留？ | ✅ **留** ⇒ `tls` 登记成 **10** 个成员 | 规范侧 §5.5 台账行 + 各册成员表按 10 个写（§11.2 步骤 ①②③）；扩展代码**不动** |
| **D2** | 重复 `Content-Length`（值相同）两处口径不一致 | ✅ **新模块向老的收紧** ⇒ `std.http` 与 `std.net` 一致：**值相同也拒** | **W-36**（§11.3）：只改 `wl/std/http.wll` 的 `_CL_INT` 口径 + 契约测试；**不动** `std.net` ⇒ **零版本风险**；「放宽」的口径留到 v1.1.0 再议 · ✅ **第三波已落地**（`http_contract` 15/0） |
| **D3** | `p10` / `p11` / `p13` 要不要改写成 `TCP_SERVE` 形状以恢复正面证据？ | ✅ **改写** | **W-35**（§11.3）：三个探针 + 镜像副本改成 `TCP_SERVE` 形状 ⇒ 恢复「8 并发全 200 / 403×3 / 4 请求全 200」，**并顺带成为需求方的迁移指引** · ✅ **第三波已落地**（probe 192/192） |
| **D4a** | 解析器的 `E0011` 误报（合法程序被误拒） | ✅ **本轮修**，并入 v0.12.1 | **W-37**（§11.3）：解析器最小复现 + 修复 + 回归；**静默错误级**（报错方向误导）⇒ 优先级高于其它余项 · ✅ **第三波已结案（证伪 + 钉回归）** —— 见下方「D4a 的实测更正」 |
| **D4b** | NFKC 字符表停在 Unicode 17.0 | ✅ **保持 17.0**，等数据源 | **不排期**；`unicode/waf.rs` 文件头已写明版本与 53 码点差集；`--ucd` 通路已实现并用合成 UCD 端到端验证过 ⇒ **有 UCD 源数据时重生成即可**，不占用 v0.12.1 |

⚠ **D2 的方向说明**：业主选的是「**新模块收紧**」而不是「老模块放宽」⇒ **不需要动任何既有成员** ⇒ **不破版本纪律**（v1.0.4 仍成立）；代价是 `std.http` 比 RFC 允许的更严（多拒一种报文）⇒ 安全上不吃亏。
⚠ **D4b 的「不排期」不等于「不做」**：Unicode 18.0 的新增**拿不到**（实测 unicode.org 超时、仓内无 vendored UCD）⇒ 等数据比等一个版本号划算；`--ucd` 正路已就绪。

### 11.4b ⚠ D4a 的实测更正（第三波，2026-10-28 下午）—— **W-37 证伪结案**

**结论：计划里记的「解析器 `E0011` 误报」不成立；W-37 以「证伪 + 钉回归」结案，解析器源码零改动。**

| 项 | 计划 §13.4 的记载 | 实测 |
|---|---|---|
| 最小复现 | 多行 `LET(f, FUN((conn), (FOO(conn, CAT(head, body)); "200")));` ⇒ `E0011: expected ')', got Semicolon`；单行正常 | **多行与单行都解析通过**（exit 0、零输出）⇒ 那串代码是**平衡的合法程序** |
| 症状的真正来源 | —— | 同一份程序**少一个右括号** ⇒ **逐字复现** `E0011: expected ')', got Semicolon`（line 4 col 3）⇒ 那是**正确诊断** |
| 覆盖面 | —— | **79 个合法形态**（排版 / 块位置 / 块终结符 / 嵌套层数 / `;` 省略 / 控制流关键字位置 / 模块声明位置 / `LET` 变体）⇒ **0 个被误拒**;反向侧扫「逐个删掉任意一个 `)`」× 37 个落点 ⇒ 全部正确分类（33 处 `E0011` / 4 处 `E0010`） |

⚠ **差点踩进去的坑（工程教训）**：本波开工时 `target/release/wlwl.exe` 构建于 07:49，而 `wl/std/str.wll` 于 08:05 被 W-28 改过 ⇒ **旧二进制读新模块源码** ⇒ 报 `E0013` 且位置指向 `wlwl:std.str:281:45` ⇒ **合法程序被误拒**。⇒ 「过期二进制」能造出与解析器缺陷**完全同形**的现象 ⇒ **任何解析器 / 诊断面结论，先确认二进制新鲜度**（§13.6 第 11 条）。

⇒ **W-37 的产出**：新增 `impl/crates/wlwl-parser/tests/e0011_false_positive_contract.rs` **7 条**契约（① 计划记的那串多行形态**必须解析通过** ② 单行形态必须通过 ③ CRLF 必须通过 ④ 24 个代表形态一张表全覆盖 ⑤ 钉住块值语义 ⑥ 少一个 `)` ⇒ **仍报 `E0011`** ⑦ 删任意一个 `)` ⇒ 绝不悄悄放过）⇒ `wlwl-parser` **208/0** 全绿。

⇒ **计划本文档的两处措辞需据此订正**（本波已改）：§13.4 的 W-37 行「最小复现」栏、与 §11.4 的 D4a 行。**根因不在实现，在记录** —— 记录里的「最小复现」是一个**合法**程序，而失败的那个 artifact 必然是**丢了一个 `)`** 的变体。



### 11.5 本波的三条工程教训（供下一轮照抄）

1. **派工 briefing 必须自带规格与坐标**，不要只丢「见计划文档」——第一个 W-21 agent 把预算烧在通读计划 + 全仓勘察上，**零产出**；把 ABI 契约整段抄进 prompt 后才正常开工。
2. **改 Rust 源文件用精确替换，别用正则式批量替换**——我一度用脚本把 `lib.rs` 注释里的字面 `` `n `` 替成真换行，**弄坏文件**（编译直接断），已回退重做。
3. **`.ps1` 脚本里别写中文**——PowerShell 5.1 按 ANSI/GBK 读无 BOM 的脚本 ⇒ 中文字符串的引号被吃掉 ⇒ 脚本直接语法错；今天的脚本全部改成纯 ASCII。

---

### 11.6 完成度面板（2026-10-28 09:00 快照 · 第二波落地后）

| 口径 | 数值 | 说明 |
|---|---|---|
| **工作项** | **13 / 28（46.4%）** | 第一波 8 项（W-01 W-02 W-12 W-21 W-23 W-24 W-26 W-27）+ 第二波 5 项（W-22 W-10 W-16 W-13 W-28） |
| **需求条目** | **19 / 57 完全落地** | 🔧 部分 **2 条**（D-6 差 W-25 中间件侧 · H-4 差 W-34 文档口径）· ⬜ 未开工 **29 条** · 📖 口径答复 **6 条**（E-6 E-7 I-1 I-2 I-4 I-5） |
| **新增公共面成员** | **30 个**（全部待规范仓落表） | `std.text` +3 · `std.fs` +7 · `std.net` +1 · `std.format` +1 · `std.str` +1 · `std.log` 7 · `std.sync` 8 · `std.http` 2 —— 后三个是**新命名空间**，走一致性册 §5.5 台账 |
| **门禁** | 见 §11.7 | 第二波集成后的实测分类 |
| **关键路径** | **W-21 + W-22 已就位** | TLS 扩展端到端可跑（宿主 `--ext` 加载 → `tls`/`crypto` 命名空间可 IMPORT） |
| **版本口径** | 实现 **v0.12.1** ↔ 规范 **v1.0.4** | 未动；零处修改既有成员、零处新增诊断码 |
| **待业主裁决** | **0 件** | ✅ 四项**全部已裁决**（2026-10-28 12:36，见 §11.4）：D1 `tls::LOCAL_ADDR` **留**（台账 10 员）· D2 `std.http` 的重复 `Content-Length` **向 `std.net` 收紧**（W-36）· D3 三个探针**改写成 `TCP_SERVE` 形状**（W-35）· D4a 解析器 `E0011` 误报**本轮修**（W-37）· D4b NFKC 表**保持 17.0**，等 UCD 源数据 |

### 11.6b 完成度面板（2026-10-29 早间快照 · **第七波**落地后）

| 口径 | 数值 | 说明 |
|---|---|---|
| **工作项** | **24 / 32（75.0%）** | 第一波 8 + 第二波 5 + 第三波 3 + 第四波 2 + 第五波 2 + 第六波 2 + **第七波 1（W-32）**;⚠ 分母由 28 订正为 **32**（新增 W-35(D3) / W-36 / W-37 / W-38 四项） |
| **需求条目** | **28 / 57 完全落地**（+1） | 第七波落地 **G-2**（`wlwl check` 两条静态规则）⇒ 完全落地。🔧 部分 **4 条**（**G-3** 只做最小 REPL、调试器未做 · **G-4** 只做循环导入诊断 + 模块私有成员（**语义本已满足**，第七波补做）+ `.wll.sig` 工作流简化未做 · D-6 差 W-25 中间件侧 · H-4 差 W-34 文档口径）· ⬜ 未开工 **19 条** · 📖 口径答复 **6 条**（E-6 E-7 I-1 I-2 I-4 I-5）⇒ 28+4+19+6 = 57 ✓ |
| **业主裁决项** | **3 / 4 已执行**（D2 D3 D4a） | D2 → W-36 ✅ · D3 → W-35 ✅ · D4a → W-37 ✅（**证伪结案**，见 §11.4b）；D1 随规范侧 §5.5 台账执行；D4b **不排期** |
| **新增公共面成员** | **36 个**（仍全部待规范仓落表） | 第六、七波**均未新增**（CLI 子命令与静态提示都不是公共面；规则②无 E/W 码 ⇒ 不进附录 A、不进 §0.9）⇒ 仍是第三波 30 + 第四波 2 + 第五波 4 = **36**;⚠ 另加 **1 个新命名空间**（`std.signal`）+ **附录 G 一行**（`ARRAY_GET`） |
| **门禁** | 见 §13.2（第七波集成后实测） | 绿 —— fmt 0 diff · probe **3/3（195 用例）** · `wlwl-cli` **全 crate 绿**（138 / 23 / 19 / 6 / 9 / 3 / 2 / 9 / 5 / 2 / 2 / 1 / 2 / 4 / 6 …）· `wlwl-types` **144/0** · `lsp_smoke` 2/0 · `suspend_site` 13/0 · `spec_r2_conformance` 11/0;红 —— 仍是「规范侧未跟上」那一类（详见 §13.2） |
| **关键路径** | **W-21 + W-22 已就位**；**W-11 修掉 B-2 根因**；**W-15 修掉 B-6 真缺陷**；**W-30 给出运行期活读数**；**W-32 补上静态预检** | TLS 扩展端到端可跑；惊群不再发生 ⇒ **SP-2 作废**;「丢失的挂起信号伪装成正常 NULL」已修;任务树/挂起点/时长**运行期可见**;⚠ **第七波补上静态这一环** —— 宿主挂起点成员写错位置**在 `check` 阶段就被拦**（复用既有 `E0014`） |
| **版本口径** | 实现 **v0.12.1** ↔ 规范 **v1.0.4** | 未动；零处修改既有成员、零处新增诊断码（W-33 只改**消息文本**与位置;W-30 只加内部字段与 CLI 子命令;W-32 规则①**复用** `E0014`、规则②**无码**） |
| **待业主裁决** | **0 件**（本波无新增待决） | ⚠ **本波登记 3 项**（见 §13.4）：W-32 的两条「静态 vs 动态」差异 · `SCOPE` 体的文案-判据不一致 · **「顶层也静态拦」的口径决定**（建议单独立项） |

### 11.7 第二波的门禁分类与**一个必须让业主知道的连带后果**

**门禁**（第二波集成后实测）：绿 —— `cargo fmt --all -- --check` 0 diff · `wlwl-std` 全绿 · probe 套件 `pass=180 deviation=12 fail=0`（192 用例）· `suspend_site_contract` 13/0 · `sync_contract` 21/0 · `http_contract` 15/0 · `str_format_contract` 14/0 · `format_contract` 11/0 · `ext_loader_contract` 22/0 · `wlwl-ext` 32/0 · conformance 6/6。
红 —— 全部落在**「规范侧未跟上」**这一类（附录 A 抄件 4 行差异 + `stdlib_appendix_a_sync` 成员集合 + 各 `*_contract` 的成员表），修法见 §11.2 五步。

⚠⚠ **连带后果（W-10 带来的，必须让业主知道）**：
新判据把「宿主挂起点成员的调用点必须在任务体内」**真正执行**了 ⇒ **需求清单里那个「可用架构样板」现在结构性违规**：
需求清单附录 A 的 `p13`（单 acceptor + 每条连接一个 `serve_one` 辅助闭包 + 静态展开链）**在 `TCP_RECV` 处被判 `E0014`** ⇒ 需求方 §0.2 说的「已经实测跑通的那个纯 wlwl HTTP 服务」的**代码形状**现在不再合规 ⇒ `p10` / `p11` / `p13` 三个探针的读数从「8 并发全 200 / 403×3 / 4 请求全 200」变成了「`E0014`」。
⇒ **合规的形状是 `TCP_SERVE`**（W-12 落的那个）：accept/recv 都在**原生帧**里，handler 闭包跑在原生帧内 ⇒ 不受本判据约束 ⇒ 需求方**迁移到 `TCP_SERVE` 即可**，功能面不受影响。
⇒ ✅ **业主已裁决（2026-10-28，D3）：把这 3 个探针改写成 `TCP_SERVE` 形状**，以**恢复正面证据**（并顺带成为需求方的迁移指引）⇒ 落 **W-35**（§11.3）⇒ **本轮只修订文档，未执行**。

## 12 给业主的摘要

1. **57 条全部落地**：50 条变成 28 个实现侧工作项（W-01…W-35，按波次分 4 波、最多 9 个 agent 并行），5 条是口径答复（I 组），2 条按需求方**自己的建议**归应用层（E-6 Markdown / E-7 高亮 —— 清单原文就是「**建议明确不做**」）。**没有任何一条以「延后」结案**。
2. **规则侧只动三类**：① 修语言册与并发册关于 `E0014` 的**正面矛盾**（并发册本来就写对了）；② 给新增成员 / 命名空间走登记（§3.2 修订版本通道，**引用 D-13 先例**）；③ 给 TLS 扩展一个合法落点（一致性册 §5.5 台账第一行）。**零处修改既有成员、零处新增诊断码、零处新增册** ⇒ **v1.0.4 成立**。
3. **TLS 按您的指示走 Rust 原生动态库**：`wlwl-tls.dll/.so`（cdylib，自持 rustls）+ 宿主 `--ext` 显式加载 + 一致性册 §5.5 台账第一行 ⇒ **不进 `std.*`、不动附录 A、发行二进制依赖面不变**。安全前提只有一条但不可让渡：**语言面不提供 `LOAD` 成员**（否则脚本即任意代码执行）。
4. ⚠ **实测把需求清单的两条前提推翻了**（§2.3 R-24…R-29）：**B-1** 的「嵌套闭包 ⇒ 静默终止」在当前代码上**复现不出来**（可观察的是段首重放 + 副作用重复）——但并发册 `:94` 要求的那条检查**根本没写** ⇒ **实现侧今天不合规**，W-10 改成「新增词法检查」；**B-2** 的死法不是「静默 NULL」而是调用方 `TCP_RECV(NULL)` ⇒ `E0030`，惊群本身坐实 ⇒ W-11 改成「**唤醒一个**」修根因，落选者**不再拿 `NULL`**，规范侧因此**大概率零改动**（SP-2 降为条件项）。
5. **四项已全部拍板**（§10，2026-10-27 点选确认）：**Q1** TLS 走动态库扩展（与您的指令一致）· **Q2** 并发三项按需求清单候选形态、不引入 Step 3 · **Q3** 新成员落 `std.*`（动附录 A）· **Q4** G-2 规则 ② **进 v1.0.4** —— 为此把它改成**不需要新诊断码**的形态（无 E/W 码的「规范符合性提示」+ 运行期拦截兜底）⇒ **规则侧零改动、零新码、版本纹丝不动**。
6. **第二波后又裁决四项**（§11.4，2026-10-28 12:36 点选确认，**全部采纳推荐项**）：**D1** `tls::LOCAL_ADDR` **留**（台账按 10 员）· **D2** `std.http` 的重复 `Content-Length` **向 `std.net` 收紧**（只改新模块 ⇒ 零版本风险，W-36）· **D3** `p10`/`p11`/`p13` **改写成 `TCP_SERVE` 形状**以恢复正面证据（W-35）· **D4a** 解析器 `E0011` 误报**本轮修**（W-37）· **D4b** NFKC 表**保持 17.0**、等 UCD 源数据再重生成（**不排期**）。
   ⇒ **第三波已执行 D2 / D3 / D4a**（§11.6b）;⚠ **D4a 的结论是「缺陷不存在」**（§11.4b）—— 计划记的「最小复现」是一个**平衡的合法程序**，多行与单行**都**解析通过;计划记的 `E0011: expected ')', got Semicolon` 可由**同一份程序少一个右括号**精确复现 ⇒ 那是**正确诊断**。⇒ **解析器源码零改动**，改为钉 **7 条**回归契约（合法形态必须通过 + 少一个括号仍必须报 `E0011`）。
7. **六波共落地 23 / 32 个工作项、27 / 57 条需求完全落地**（§11.6b 面板）；未开工 20 条、口径答复 6 条。**第三波**落地了业主裁决的 **D2 / D3 / D4a** 三项（其中 **D4a 证伪结案** —— 解析器 `E0011`「误报」不存在，计划的最小复现是一个**合法**程序，真实症状源是**少一个右括号** ⇒ 解析器源码零改动，改为钉 7 条回归契约）。**第四波**修掉 B-2 的惊群**根因**（唤醒一个 ⇒ 规范侧 SP-2 作废、零改动）与 B-5 的非阻塞出站;**第五波**修掉 B-6 的真缺陷（丢失的挂起信号不再伪装成正常 NULL）、落地 B-7 的 `std.signal`、W-38 诊断化，以及 H-5 / H-6 的两个别名;**第六波**落地 F-3 任务检视 + F-4 诊断可读性（G-3 / G-4 **部分**）。**版本口径始终是实现 v0.12.1 ↔ 规范 v1.0.4**；**规范仓一行未动** —— **36 个新成员 + `std.signal` 新命名空间 + 附录 G 一行**要进公共面，得按 §11.2 的五步走（且**必须等规范仓先 push**）。

---

## 13 接替执行手册（2026-10-28 13:05 收尾版）

> **这份手册的目的**：下一次开工的人（或 agent）**只读本文件 + §11 + §2**，就能直接派出下一批 agent 并完成集成，**不需要重新勘察**。
> ⚠ 本文件里的**规格要点与坐标都已核实过**（2026-10-28 上午实测），可直接抄进 briefing。

### 13.1 现在在哪（仓库与版本现状）

| 项 | 现状 |
|---|---|
| 仓库 / 分支 | `D:\Project\wlwl`（实现侧） / **`wip0121`**（远端 `origin/wip0121` 已同步）;规范仓 `D:\Project\wlwl-spec-v1`（**本轮一行未动**） |
| HEAD | `64d9bbb` feat(impl): v0.12.1 第七波 —— W-32（wlwl check 两条静态规则）+ G-4① 证伪结案 |
| 提交序列 | `40b5af6` **第六波 2 个 PHASE** → `1987aa0` 第五波面板 → `832839e` **第五波 2 个 PHASE** → `a8a431a` 第四波面板 → `969b220` **第四波 2 个 PHASE** → `78a826f` sanitize 定性更正 → `7f05d80` 第三波面板 → `0d5f410` **第三波 3 个 PHASE** → `d3f7b6c` §13 接替执行手册 → `20fa75f` 裁决记录 → `86b3c4a` **第二波 5 个 PHASE** → `df90382` 完成度面板 → `b0649cc` **第一波 7 个工作项** → `87a473b` §5.2b 真并行度 |
| 工作树 | **干净**（`git status` 空） |
| 版本口径 | 实现 **v0.12.1** ↔ 规范 **v1.0.4**（业主两次确认锁死；**零处修改既有成员、零处新增诊断码**） |
| 完成度 | **工作项 24 / 32** · **需求 28 / 57 完全落地**（部分 4 · 未开工 19 · 口径答复 6）⇒ 面板见 **§11.6b** |
| 已落地清单 | §11.1（**24 行**，每行带验收读数） |
| 业主裁决 | §10（Q1–Q4）+ §11.4（D1–D4b，**全部已定**）—— **D2 / D3 / D4a 已执行**（第三波）;D1 随规范侧 §5.5 台账执行;D4b 不排期 |

### 13.2 门禁现状（**第七波**集成后实测）

**绿**：`cargo fmt --all -- --check` **0 diff** · probe 套件 **3/3**（**195 用例**，常量 194 → **195**）· `wlwl-cli` **全 crate 绿**（138 / 23 / 19 / 6 / 9 / 3 / 2 / 9 / 5 / 2 / 2 / 1 / 2 / 4 / 6 …）· `wlwl-types` **144/0**（含 W-32 的 14 条新契约）· `wlwl-error` 65/0 · `wlwl-eval --lib` **920/0**（含 W-30 的 4 条 runtime 守卫）· `lsp_smoke` 2/0 · `spec_r2_conformance` 11/0 · 第五波契约仍绿（`b6` 7/0 · `b7` 10/0 · `w38` 11/0 · `h5_h6` 18/0）· 回归守护全绿（`b2` 4/0 · `b5` 12/0 · `socket_wait` 16/0 · `tcp` 23/0 · `suspend_site` 13/0）。

**红（8 处，与第六波完全相同 ⇒ 本波零新增，全部是「规范侧未跟上」）**：`check-appendix-a`（抄件 vs 实现派生表体）· `stdlib_appendix_a_sync`（**4 failed**）· `spec_appendix_g_sync`（**2 failed** —— 附录 G 的抄件缺 `ARRAY_GET` 一行）· `fs_contract` / `net_contract` / `str_math_contract` / `text_contract` 的**成员表**（各 1 failed）· `process_contract`（**1~3 failed** —— 其一为**既有的成员表红**（规范侧未跟上），另两条 `a_timeout_yields_an_err_value` / `a_timeout_with_a_living_grandchild_returns_promptly` 是**并行干扰型 flake** ⇒ **不是回归**：**各自单跑绿**（0.89 s / 0.86 s）· **同文件串行跑绿**（`--test-threads=1` ⇒ 只剩成员表那条红）· **并行跑则红** ⇒ 两个用例各自 `taskkill /T /F` 杀整棵进程树 ⇒ 并行时互相误杀 ⇒ 起因指向第四波 W-14 新增的 `PROCESS_RUN_ASYNC` 进程组管理 ⇒ 已登记为待办）。
⇒ 根因：**36 个公共面成员 + 1 个新命名空间（`std.signal`）+ 附录 G 一行** 待规范仓落表 ⇒ 修法见 **§11.2 五步**（**必须规范仓先落表，再抄回抄件**）。

⚠ **本波新增的诊断文案差异（规范侧语言册措辞登记）**：**只有 `E0041` 一条** —— 消息补了「模块加载中不能再被加载」的说明（**含义未变**，条件仍是「IMPORT 成环」;`a -> b -> a` 段**逐字保留**）;**位置**从 `<module>:0:0` 改成**闭合环那条 `IMPORT`** 的 `file:line:col` 并新增 `source_line` ⇒ 这三个字段值会变。
⚠ **人形态新增两行**（`^` 高亮行 + `= hint:` 行）**无诊断码、无规范条款** ⇒ 不进附录 A、不进 §0.9;`json` / `jsonl` **逐字节不变**（除循环导入那一条）。

⚠ **登记项 ③ 仍然不做**（理由同前四波）：它与 ④（抄回 `docs/wlwl-spec-reference.md`）**必须成对**，而 ④ 的内容来自规范仓 ⇒ 现在改 ③ 反而会把红消息从「诚实」换成「误导」。

### 13.3 接替者的第一分钟（照做，别重新调查）

```powershell
cd D:\Project\wlwl\impl
cargo run --locked --bin check-appendix-a -- ../docs/wlwl-spec-reference.md   # 预期 MISMATCH（规范侧未跟上）
cargo test --locked -p wlwl-eval --test stdlib_appendix_a_sync                # 预期 4 failed（同上）
cargo test --locked -p wlwl-cli --test probe                                 # 预期 3 passed（195 用例）
cargo test --locked -p wlwl-parser                                           # 预期 208 passed（含 e0011 契约 7 条）
cargo test --locked -p wlwl-types                                           # 预期 144 passed（含 W-32 契约 14 条）
cargo test --locked -p wlwl-cli --test lsp_smoke                         # 预期 2 passed（LSP ≡ CLI 同一份）
cd D:\Project\wlwl; git log --oneline -4; git status --short                  # 预期干净
```
⇒ 读本文件 **§13**（接替手册）→ **§11**（交接与下一批切分）→ **§2**（事实坐标）。

### 13.4 下一轮该开的 agent（**按优先级；规格要点可直接抄进 briefing**）

#### 优先批（3 并行，文件面零重叠）—— 业主裁决直接产生的三项 · ✅ **第三波已落地（2026-10-28 下午）**

| 工作项 | 目标 | 独占文件 | 规格要点（已核实） | 第三波结果 |
|---|---|---|---|---|
| **W-37** | 解析器 `E0011`「误报」（D4a） | `impl/crates/wlwl-parser/**` + 诊断面 | ⚠ **原记的「最小复现」不成立**（多行/单行**都**解析通过）；真实症状源 = **同一份程序少一个右括号** ⇒ 那是正确诊断。⇒ 改为「**证伪 + 钉回归**」 | ✅ **证伪结案**：解析器源码**零改动**;新增 7 条契约（合法形态必须通过 + 少一个括号仍报 `E0011`）⇒ `wlwl-parser` **208/0**。详见 **§11.4b** |
| **W-36** | `std.http` 的重复 `Content-Length` **收紧**（D2） | `impl/crates/wlwl-std/wl/std/http.wll` + `impl/crates/wlwl-eval/tests/http_contract.rs` | 与 `std.net` 对齐：**同名多值 ⇒ 一律拒**（含值相同）；**只改新模块** ⇒ 零版本风险。⚠ **不要**动 `std/net.rs`（改既有成员语义撞 `:223`）。⚠ R1 模块的 `EXPORT` 必须**单行**（`lang_exports` 逐行扫） | ✅ `http_contract` **15/0**（断言 37 → 40）;反向守卫红过;`std.net` 零改动 |
| **W-35** | `p10`/`p11`/`p13` 改写成 `TCP_SERVE` 形状（D3） | `docs/wlwl-feasibility/probes/p1{0,1,3}-*.wll` + `impl/tests/probe/cases/P121_probe_p1{0,1,3}_*/` | 目标读数恢复为「8 并发全 200 / 403×3 / 4 请求全 200」;**`TCP_SERVE` 的 handler 不得调用宿主挂起点**（它跑在原生帧内 ⇒ 不受 W-10 判据约束）。⚠ 两份副本（探针本体 + 夹具）必须**逐字相同**（用 SHA256 核对） | ✅ probe **3/3（192 用例）全绿**;三对副本 SHA256 一致;头部注释已重写为**迁移指引** |

**合并顺序**：W-36 先合（W-35 的新探针要跑 `http_contract`）⇒ 再合 W-35 ⇒ W-37 独立，随时可合。· **第三波实测**：三者文件面确实零重叠 ⇒ **真并行 3**（无一处同文件争用）。

⚠ **第三波带出的新缺口（已登记，本波不修）**：

| 工作项 | 目标 | 独占文件 | 规格要点（已实测） |
|---|---|---|---|
| **W-38** | `TCP_SERVE` 的 handler 里调宿主挂起点成员 ⇒ 该连接**静默作废**（无诊断） | `impl/crates/wlwl-std/src/net.rs`（`TCP_SERVE` 的 handler 调用处）+ 契约测试 | **实测**：handler 被调用 **1** 次（`handler-calls= 1`），但 `TCP_SERVE` 返回 **0**、程序 **exit 0**、**stderr 一个字都没有** ⇒ 那条连接既没被服务、也没有任何诊断（只落进宿主告警槽的软告警，**不打印**）。⇒ 属「**静默失败级**」：照旧形状写 handler 的开发者只会看到「少服务了一条连接」。⚠ 修法要动「**原生帧内算不算任务体**」这条语义判据 ⇒ **需单独裁决**（诊断？拒绝？文档化？），**本波不动** |

#### 随后批（4 并行）—— ✅ **第五波已做 W-15 / W-29**（W-11 / W-14 第四波已做）· **全部 4 项已落地**

| 工作项 | 目标 | 独占文件 | 规格要点 | 状态 |
|---|---|---|---|---|
| **W-11** | B-2 修根因：**唤醒一个**而不是唤醒全部 | `impl/crates/wlwl-std/src/tcp.rs` + `impl/crates/wlwl-eval/src/runtime.rs` | 惊群的结构性原因：一个 listener 只有一个 socket id ⇒ 所有 acceptor 共享一把等待者 `Vec`（`runtime.rs:319`）⇒ `wake_socket_waiter`（`runtime.rs:973-1006`）**唤醒全部**。改成 **FIFO 唤醒一个** ⇒ 落选者**不再拿到 `NULL`**（需求要的「保持存活并能继续接受」直接达成）⇒ 规范侧大概率**零改动**（SP-2 降为条件项）。⚠ 落选者的真实死法是调用方 `TCP_RECV(NULL)` ⇒ `E0030`（不是「静默 NULL」） | ✅ **第四波已落地**（4/0；规范侧**零改动**） |
| **W-14** | B-5 非阻塞出站 | `impl/crates/wlwl-std/src/{net.rs,process.rs}` + `impl/crates/wlwl-value/src/lib.rs` | 新成员 `HTTP_REQUEST_ASYNC` / `PROCESS_RUN_ASYNC`（**新增**，不改既有 `HTTP_*`/`PROCESS_RUN` 的阻塞语义）；复用 `TCP_*` 已有的「工作线程 + 就绪集合 + resume-hint 幂等」模式 ⇒ **重入时不得重复发起** | ✅ **第四波已落地**（12/0；**新增 2 个公共面成员** ⇒ 待规范侧登记） |
| **W-15** | B-6 修兜底臂 + B-7 优雅停机 | `impl/crates/wlwl-eval/src/{runtime.rs,lib.rs}` + `impl/crates/wlwl-std/src/signal.rs`（新建） | ① 兜底臂 `lib.rs:10651-10684` 把「非四种 park 信号」的 outcome 结成 `TaskResult::Ok(o.value)` ⇒ **丢失的挂起信号会伪装成正常 NULL**（R-25）⇒ 改成「任务失败 + 按 §17.9 上报」；② `std.signal::ON_SIGNAL` / `STOP`（Unix `libc::signal`；Windows 手写 `extern "system" { SetConsoleCtrlHandler }`；**零新增依赖**） | ✅ **第五波已落地**（7/0 + 10/0 + W-38 11/0）；⚠ **两个 probe 夹具未做**（见下） |
| **W-29** | H-5 / H-6 别名 | `impl/crates/wlwl-std/src/{regex.rs,collection.rs}` + `impl/crates/wlwl-eval/src/registry.rs` | `RE_MATCHES_FULL`（别名 `RE_TEST`，**不改 `RE_TEST` 语义**）· `ARRAY_GET`（别名 `INDEX_GET`）· 全局内建改动 ⇒ 附录 G 需 `cargo run --bin gen-appendix-g -- ../docs/appendix_G.md` 重生成 | ✅ **第五波已落地**（18/0 + `--lib` 915/0）；⚠ 附录 G 的变化**待规范侧落表** |

⚠ **W-15 的两个 probe 夹具未做**（`P121_b6_task_result` / `P121_b7_graceful_stop`）⇒ **本波未新增 probe case 目录** ⇒ `EXPECTED_CASE_COUNT` 保持 **192**（probe 套件 3/3 全绿）⇒ 同等语义已由 `b6_task_result_contract`（7/0）+ `b7_graceful_stop_contract`（10/0）覆盖 ⇒ **登记为待办，下一批补**（§5.5：不许悄悄降级 ⇒ 此处显式登记）。

⚠ **第四波的文件冲突处置（下一批照抄）**：`runtime.rs` 被 W-11 与 W-14 **都要动**（W-11 改唤醒策略、W-14 要接新的 `AsyncReady` 通道）⇒ 派工时判给 W-11 独占，W-14 改为输出**接线登记清单**（8 个站点）⇒ W-11 完成后**用 `task_append` 让 W-14 带着自己的配方补完接线**（它有全部上下文，比集成者照抄可靠）。⇒ 两条通道**故意不同**：socket 是「一次事件只够一个消费者」⇒ 唤醒**一个**；异步操作的产物**存在槽里、谁都能取** ⇒ 唤醒**全部**不会产生「落选者拿到空值」。

#### 再随后（`std.http` 的四节扩展 —— **必须串行**，都改 `wl/std/http.wll`）

**W-17**（C-3/C-8/C-9：Keep-Alive + 请求预算 + 头/体上限）→ **W-18**（C-4/C-5/C-6：路由 + Cookie + 表单/multipart）→ **W-19**（C-7/C-10/C-12：静态文件 + 压缩 + 条件请求）→ **W-20**（C-11/A-2：chunked 客户端解码 + HTTPS 出站，走 `tls::CONNECT`）· **W-25**（D-3…D-7：限流 / 规则热更新 / 审计 / 真实 IP）。
⇒ **W-16 已把 `http.wll` 的骨架与 22 个助手立起来**（494 行）⇒ 扩展这四节时**沿用现有助手**，别重写。

#### ⚠ 第六波带出的**降级登记**（§5.5：不许悄悄降级 ⇒ 显式登记）

| # | 项 | 状态 | 原因与去向 |
|---|---|---|---|
| ① | **G-3「调试器」** | **部分** | W-30 只做了计划给的**最小口径**（「至少一个能跑表达式的 REPL」）⇒ **断点 / 单步 / 变量改写**没做 ⇒ 非目标已明写「不做完整调试器」⇒ 若要做 ⇒ 需另立专项 |
| ② | **G-4「模块组织」的三项** | **部分**（第七波补做了一项） | W-33 做了「循环导入的清晰诊断」;**第七波补做了「模块私有成员」⇒ 证伪结案**：实测 6 个场景（有无 `.wll.sig` / 有无 `SEALED` / 消费方越界导入）证明**私有成员（既不 `EXPORT`、也不在签名里声明的根作用域绑定）本来就允许且零诊断** ⇒ `check_exports` 的两个方向都只拿 `EXPORT` 名单与声明面对照 ⇒ 私有成员**两边都不在** ⇒ 不触发 ⇒ **不需要 `private` 修饰符、不需要模块级 `CONST`、不需要新诊断码** ⇒ 交付改为「把这条语义写进模块文档（此前全仓无任何地方写下来）+ 2 条回归守卫」（`sig.rs` +90 行，**语义零改动**）;**仍未做**：**`.wll.sig` 工作流简化**（在 `cli/tooling.rs` + `cli/main.rs` ⇒ 与 W-32 同文件面 ⇒ **下一批**）· **模块级常量与初始化代码**（**新语言特性** ⇒ 要动 `wlwl-parser` / `wlwl-ast` **+ 规范侧条款** ⇒ 单独立项） |
| ③ | **「运行期安全点」钩子** | **未做** | W-30 指出：要让**不依赖端点**也能在运行期看到挂起点 ⇒ 需要在 `wlwl-eval` 的求值循环里加一个安全点钩子 ⇒ 该文件本轮归 W-33 ⇒ 未做 ⇒ 登记为待办 |
| ④ | **「跑完打印快照就能看到挂起点」** | ⚠ **原计划前提不成立** | 实测 **`SPAWN` 必须在 `SCOPE` 内**（`E0058`）⇒ 作用域退出会**硬取消**未结束的子任务 ⇒ 程序跑完后**不可能**观察到 `suspended` ⇒ 该读数本质是**运行期活读数** ⇒ 已改为「端点活快照 + 11 个状态迁移点发布」⇒ **计划措辞需按此订正** |
| ⑤ | **W-32 的两条「静态 vs 动态」已知差异** | **登记**（不修） | ① **模块顶层漏报**：静态层按 `host_suspend_outside_task` 的判据（`None` ⇒ 不动）⇒ 顶层形状静态层不报，但**运行期在顶层同样会报 `E0014`**（走「挂起信号逃到顶层」通路，文案逐字相同）⇒ 若要「顶层也静态拦」⇒ 是把判据扩到 `None` 的一行改动，**会改掉既有 `check` 的退出码语义** ⇒ **不夹带进 W-32**，单独立项;② **「成员跑完之后才判」的先后在静态层不存在** ⇒ 同一处静态可能报 `E0014` 而运行期报 `E0030`（如 `SLEEP_ASYNC(-1)` 在嵌套闭包里）⇒ 登记 |
| ⑦ | **`process_contract` 两条 timeout 用例的并行干扰** | **登记**（不修） | 两条用例各自 `taskkill /T /F` 杀整棵进程树 ⇒ **同进程并行时互相误杀** ⇒ 断言 `elapsed < 3 s` 被撞 ⇒ **各自单跑绿 / 串行跑绿 / 并行跑红** ⇒ **不是回归** ⇒ 起因指向第四波 W-14 新增的 `PROCESS_RUN_ASYNC` 进程组管理 ⇒ 修法方向：让超时杀树**只作用于自己起的那个进程组**（隔离），或把这两条用例标为需串行 ⇒ 登记为待办 |
| ⑥ | **`SCOPE` 体是不是合法落点** | **登记** | `E0014` 的**文案**说「(e.g. within SCOPE(...) / SPAWN(...))」，但**判据**上 `SCOPE` 的体在任务体内时深度为 1 ⇒ **报 `E0014`** ⇒ **文案与判据不一致** ⇒ 文案归运行期文件 ⇒ 登记为独立项 |

#### 收口批

**W-34**（文档 / 技能包：含 D1–D4b 的对外口径 + **`TCP_SERVE` 迁移指引**）· **W-35′**（登记 + 附录 A 抄件 + 全量门禁）· 规范仓 v1.0.4 批次（§11.2 五步）。

⚠ **收口批另有两项待办**（第三波实测带出，**不阻塞**）：

| # | 事 | 证据 | 归属 |
|---|---|---|---|
| ① | **W-38**：`TCP_SERVE` 的 handler 里调宿主挂起点成员 ⇒ 该连接**静默作废** | 实测：handler 调用 1 次 · `TCP_SERVE` 返回 0 · exit 0 · stderr 全空 | `std/net.rs` + `tcp_serve_contract.rs`；**需先裁决**（诊断？拒绝？文档化？） |
| ② | **`sanitize_contract::sanitize_is_idempotent_on_mutations` 极慢**（单跑 **381.33 s 通过**） | 该目标跑的代码与 HEAD **完全相同**（第三波对 `wlwl-parser/src` · `wlwl-eval/src` · `wlwl-std/src` · `sanitize.wll` · `wlwl-value` **零改动**）⇒ **既有问题，非第三波引入** | `impl/crates/wlwl-eval/tests/sanitize_contract.rs:1616`;形态 = 对 `SANITIZE_CASES` × 变异组合逐个**新起求值器**跑两遍 ⇒ debug + 机器负载下约 **6.4 分钟**。⚠ **它会过、只是慢**（本轮实测 `1 passed`）⇒ 下一轮给它设预算 / 拆用例即可，**不是缺陷** |

### 13.5 集成者的固定动作（每波结束都做这几件，顺序不变）

1. 收 agent 的**「登记清单」** ⇒ 统一改：`impl/crates/wlwl-std/src/lib.rs`（`pub mod X;` + `resolve()` 分派 + `NATIVE_SPECS` + `LANG_SOURCES`）· `impl/crates/wlwl-eval/src/stdlib_mirror.rs`（`NAMESPACE_META`）· 各 `*_contract.rs` 里的**期望名单**。
2. `cargo fmt --all` ⇒ `cargo fmt --all -- --check` 必须 0 diff。
3. 跑门禁 ⇒ **把红分类**：①「规范侧未跟上」（改抄件 / 规范仓）②「真缺陷」（当场修）。
4. `git add -A` + `git commit`（**不 push**；push 由业主发起）。
5. 更新本文件 §13.1（HEAD/提交序列/门禁/完成度）⇒ **下一个接替者只读 §13 就能知道进度**。

### 13.6 环境事实（接替者别再踩一遍）

| # | 事实 | 影响 |
|---|---|---|
| 1 | **本机无网络**（实测 unicode.org 超时） | 不能新增需要下载的依赖；仓内 cargo 缓存已有 `rustls 0.23.46` / `ring 0.17.14` / `webpki-roots` / `serde_json` ⇒ TLS 扩展可离线构建 |
| 2 | **无 `openssl` CLI**，但 **Python `cryptography` 50.0.2 可用** | 测试自签证书用 Python 生成（`impl/ext/wlwl-tls-ext/tests/gen_fixture_cert.py`），不要去找 openssl |
| 3 | `impl/Cargo.lock` **不入库**；**TLS 扩展的 `Cargo.lock` 入库**（它自带 crate-local `.gitignore` 只忽略 `target/`） | 扩展的版本可复现；主仓依赖面不变 |
| 4 | **agent 并发上限 4** | 一次最多 4 个；本轮的「真并行度」按**文件不重叠**算（§5.2b） |
| 5 | 所有 agent 共享 `impl/target` ⇒ cargo 会 `Blocking waiting for file lock` | **正常**，耐心等；**不要**改 `CARGO_TARGET_DIR`（会重复编译 5 份） |
| 6 | ⚠ **改 agent 产出的未跟踪文件前先备份** | 我用脚本回溯删除 `http.wll` 的死代码，删过了头（删掉文档头 + 10 个助手）；该文件**未跟踪** ⇒ `git checkout` 救不回 ⇒ 只能重写。**先 `git add -N <file>` 或复制到 scratch** |
| 7 | **`.ps1` 脚本里别写中文** | PowerShell 5.1 按 ANSI/GBK 读无 BOM 脚本 ⇒ 中文引号被吃掉 ⇒ 语法错。脚本一律纯 ASCII |
| 8 | **wlwl 语言的三个坑** | `( … )` 是**块**不是分组括号（块内用 `;` 分隔）；`\|\|` / `&&` 是**二元**的（3 个条件要嵌套）；`+` **不做**隐式强转；`LEN` 是**码点**、`BYTE_LEN` 是**字节**、`SUB` 按**码点**切 |
| 9 | **R1 模块的 `EXPORT` 必须单行** | `lang_exports` 逐行扫 ⇒ 拆多行会让附录 A 镜像扫出**零个成员** |
| 10 | **扩展命名空间不进附录 A / 不进公共面清单** | 公共面取自 `wlwl_std::namespace_paths()` 这张**静态**表 ⇒ 运行期扩展结构上进不去 ⇒ 只进一致性册 §5.5 台账 |
| 11 | ⚠ **过期二进制会给出与源码缺陷完全同形的读数** | 第三波开工时 `target/release/wlwl.exe` 构建于 07:49，而 `wl/std/str.wll` 于 08:05 被 W-28 改过 ⇒ **旧二进制读新模块源码** ⇒ 报 `E0013` 且位置指向 `wlwl:std.str:281:45` ⇒ **合法程序被误拒**（差点被当成解析器缺陷）⇒ **任何解析器 / 诊断面结论，先确认二进制新鲜度**（`cargo build --release --locked --bin wlwl`）;探针与契约测试走 `CARGO_BIN_EXE_wlwl` ⇒ **自带新鲜度**，不受影响 |
| 12 | `TCP_SERVE` 的 handler **跑在原生帧内** ⇒ 那里**挂不了** | handler 里调宿主挂起点成员 ⇒ 该连接**静默作废**（无诊断、stderr 全空、exit 0、不计入返回计数）⇒ 已登记 **W-38**;⇒ 写 handler 时**只返回 DICT**，别碰 `TCP_*` / `SLEEP_ASYNC` |

### 13.7 一句话状态

> **24 / 32 个工作项、28 / 57 条需求已落地并复核**（第七波：**W-32** —— `wlwl check` 两条静态规则；规则①复用既有 `E0014`、规则②是**无码**提示 `shared-mutable-binding`，**LSP ≡ CLI 同一份收集函数**;**G-4① 证伪结案** —— 「模块私有成员」**语义本就满足**，只补了模块文档 + 2 条回归守卫，**语义零改动**;第六波：W-33（F-4 诊断可读性 + G-4 循环导入诊断）· W-30（F-3 任务检视 + G-3 最小 REPL）；第五波：W-15（B-6 兜底臂真缺陷 + B-7 `std.signal` + W-38 诊断化）· W-29（H-5/H-6 别名）；第四波：W-11（B-2 惊群根因）· W-14（B-5 非阻塞出站）；第三波：W-36 / W-35 / W-37 —— **W-37 是证伪结案**，见 §11.4b）；规则侧**只差规范仓落表**（§11.2 五步，**必须等业主先 push 规范仓**）—— 待落表总数仍是 **36 个成员 + 1 个新命名空间（`std.signal`）+ 附录 G 一行**;实现侧下一批是 **G-4②「`.wll.sig` 工作流简化」**（动 `cli/tooling.rs` + `cli/main.rs` ⇒ 正好独占这两个文件面 ⇒ 单 agent 即可）⇒ 之后 **`std.http` 四节串行**（W-17 → W-18 → W-19 → W-20）⇒ 再后 **W-31 / W-25** ⇒ 收口 **W-34 / W-35′** + 规范仓 v1.0.4。
> ⚠ **本波登记 3 项**（**不阻塞**，见 §13.4）：W-32 的两条「静态 vs 动态」已知差异（顶层漏报 · 「成员跑完才判」的先后不存在）· `SCOPE` 体的**文案-判据不一致**（文案说 SCOPE 合法，判据说它报 E0014）· **「顶层也静态拦」的口径决定**（建议单独立项，勿夹带）。
> ⚠ 另有一条**已记在案的取舍**：`wlwl check` 拦静态 `E0014`（exit 1），`wlwl run` **不报不拦** —— 因为既有探针 `p07`/`p09` 钉死了「跑一半 + 运行期报**逐字相同**的那条 E0014 + exit 1」⇒ 静态层若在 `run` 里拦，那两条必红。
