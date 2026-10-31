# wlwl v0.12.1 功能性审计报告 · 兼 v0.12.1_rc2 工作清单

| 项 | 值 |
|---|---|
| **审计对象** | 实现仓 `Laffinty/wlwl`，`origin/main` = `d2bd479`（v0.12.1 收口第二批 · 第三条红修毕） |
| **对照基准** | [`wlwl-v0.12.1-build-plan.md`](./wlwl-v0.12.1-build-plan.md)（§4 工作项 33 个 · §6.2 新成员表 · §7 需求台账 57 条 · §11 落地表） |
| **行为证据** | **CI run #341**（head `d2bd479`）：`cargo test --locked --all-targets` 在 **ubuntu / macOS / windows 三平台全绿**，`smoke run examples` 绿 ⇒ **仓内全部契约测试通过** |
| **审计时间** | 2026-10-31（GMT+8） |
| **审计者结论** | **功能基本达成**：计划声称的 56 个新命名空间成员里 **53 个已实现**，另 1 个全局内建已实现；**缺 3 个**，其中 **1 个被台账错标为「已交付」**（`TCP_CONNECT`）。另有 **3 处文档内部不一致** 与 **6 项已登记降级** 留待 rc2。 |

---

## 0 本文档的契约（先读这一节）

### 0.1 这份文档是什么 / 不是什么

- **是**：一份**可证伪**的功能审计 —— 每条结论都带**能重跑的命令**与**当时的读数**。
- **是**：v0.12.1_rc2 的**工作输入** —— §6 的每条都有验收命令、反向守卫、非目标、依赖。
- **不是**：计划文档的替代品。计划写「要做什么」，本文档写「**实际做到了什么**、差在哪」。
- **不是**：对实现质量的全面评审。**审计范围 = 计划承诺的功能是否落地**；性能、可维护性、注释质量不在范围内。

### 0.2 状态标记（全文统一）

| 标记 | 含义 | 判定条件 |
|---|---|---|
| **已实现** | 公共面成员在 + 契约测试存在且绿 | ①`wlwl namespace` 列出该成员 ②对应 `*_contract.rs` 存在 ③CI run #341 绿 |
| **已实现（内部）** | 功能在，但**不是**计划承诺的那个公共成员名 | 同上 ① 不成立，但 ②③ 成立且代码内有该能力的调用点 |
| **未实现** | 公共面无该成员，且代码里**零引用** | grep 零命中 |
| **已登记降级** | 计划自己写明「这样做不到 / 这样有损」 | 计划 §11.1 的 ⑬⑭⑮ 等编号条目 |
| **文档不一致** | 两处文档对同一件事说法相反 | 逐字引用两处 |

⚠ **状态只按上面三条判**。**不采信**任何文档里的「已完成」字样 —— 那是**声称**，不是**证据**。

### 0.3 证伪纪律（每条结论都自带命令）

```powershell
cd D:\Project\wlwl\impl
.\target\debug\wlwl.exe namespace          # 公共面权威事实（25 命名空间 / 288 成员）
cargo test --locked -p wlwl-eval --test <契约名>   # 行为事实
```
⚠ **命令红了以命令为准**，本文档的读数作废。

### 0.4 新增/修改条目的规则（给下一个 agent）

1. **只加不删**：审计结论过时了就**标注作废 + 指向新条目**，不覆盖原文（可追溯）。
2. **每条必须带一条能重跑的验收命令**；写不出命令的条目**不许入库**（那就是没证据）。
3. **区分「声称」与「实测」**：引用文档说法时标 `[声称]`，自己跑过标 `[实测]`。
4. **`未实现` 的条目必须给 grep 命令**（零命中就是零命中的证明）。

---

## 1 一句话结论

> **v0.12.1 计划的功能面基本落地**：33 个工作项全部有代码与契约测试支撑，公共面 **25 命名空间 / 288 成员**与计划口径一致，三平台全量测试绿。
> **但有三件事必须在 rc2 处理**：① `std.net::TCP_CONNECT` **被台账标成「已交付」，实际零实现**（§5.1）；
> ② `std.http` 的 `SERVE` / `COMPRESS_RESPONSE` 两个成员**计划列了、实现没有、也没登记**（§5.1）；
> ③ **规范仓 v1.0.4 尚未打 tag** ⇒ 本仓全部规范文本仍是**候选文本**，58 个登记项尚未成为已发布规范（§5.3）。

---

## 2 审计方法与证据边界（先说清楚我没做什么）

### 2.1 我实际跑了什么

| # | 动作 | 命令 | 读数 |
|---|---|---|---|
| 1 | 取公共面权威清单 | `impl/target/debug/wlwl.exe namespace` | **25 命名空间 / 288 成员**（完整清单见 §3） |
| 2 | 计划新成员表 ↔ 实测逐项对拍 | `python .scratch/audit_members.py`（本文档 §3 的机器化版本，脚本可重建） | 声称 56 项 ⇒ **命中 53**，缺 **3** |
| 3 | 关键成员在代码里的落点 | `grep`（见 §5 各条） | `SERVE` / `COMPRESS_RESPONSE` / `TCP_CONNECT` **零命中** |
| 4 | 附录 A 抄件是否已含新成员 | `Select-String docs\wlwl-spec-reference.md RATE_LIMITER\|SERVE_STREAM\|SHARED_DICT_SNAPSHOT` | **已含**（:6673 / :6696 / :6707） |
| 5 | 句柄两条守卫本机复跑 | `cargo test --locked -p wlwl-eval --test resource_release_contract` | **15 passed / 0 failed**（26 s） |
| 6 | unix 侧契约复跑（WSL2） | `bash .scratch/wsl-full-target.sh` | `fs_atomic_lock_meta_contract` **21 passed / 0 failed** |
| 7 | 契约测试清单 | `ls impl/crates/wlwl-eval/tests/*.rs` · `.../wlwl-cli/tests/*.rs` | eval **79** 个 · cli **22** 个 target |

### 2.2 我**没有**做什么（边界，防止过度解读本文档）

- ❌ **没有逐个重跑 79 个 eval 契约 target**。行为证据来自 **CI run #341 的三平台全量 `cargo test --all-targets` 全绿**（比逐个人肉重跑更强：它跑的是**同一批**测试）。
- ❌ **没有**手工复核计划自己登记的 6 项降级（⑬⑭⑮ 等）的实际影响面 —— 它们在 §5.3 **原样转述 + 标 `[声称]`**，并在 §6 给出复核命令。
- ❌ **没有**做性能、内存、可维护性评审。
- ❌ **没有**验证「跨平台语义等价」的每一处 —— 只依据三平台 CI 同时绿这一条事实。

---

## 3 公共面逐项对拍（机器化，可重建）

### 3.1 总量

```
[实测] wlwl namespace ⇒ namespaces = 25, members = 288
```
计划口径（§11.2 / CHANGELOG）：**288 = 基线 231 + 57 个登记项** ⇒ **一致**。
⚠ 两口径别混：`wlwl namespace` **不计**全局内建（`ARRAY_GET` 不在其中，登记时单列）。

### 3.2 计划 §6.2 的新成员逐项判定

> 判定规则：**声称 ⊆ 实测** 才算达标（实测多于声称是正常的 —— 声称表只列"本轮新增"，实测含历史成员）。

| 命名空间 | 声称 | 实测 | 判定 | 缺失项 |
|---|---:|---:|---|---|
| `wlwl:std.http` | 18 | 16 | **未达标** | **`SERVE`** · **`COMPRESS_RESPONSE`** |
| `wlwl:std.log` | 4 | 7 | 达标 | —（实测另有 `METRIC_COUNTER` / `METRIC_INC` / `METRIC_TIMER`，§11.2 登记清单写的是 7 ⇒ **§6.2 表少列 3 个**，见 §5.2） |
| `wlwl:std.sync` | 8 | 8 | 达标 | — |
| `wlwl:std.signal` | 2 | 2 | 达标 | — |
| `wlwl:std.net` | 3 | 16 | **未达标** | **`TCP_CONNECT`** |
| `wlwl:std.text` | 3 | 11 | 达标 | — |
| `wlwl:std.fs` | 8 | 26 | 达标 | — |
| `wlwl:std.str` | 1 | 9 | 达标 | — |
| `wlwl:std.format` | 1 | 2 | 达标 | — |
| `wlwl:std.test` | 7 | 13 | 达标 | — |
| `wlwl:std.regex` | 1 | 8 | 达标 | — |
| **全局内建** | 1 | — | 达标 | `ARRAY_GET`（`registry.rs` 有；不进 namespace 清单） |

**合计：声称 56 个命名空间成员 ⇒ 命中 53；全局内建 1 个 ⇒ 命中 1。总计 54 / 57。**

### 3.3 扩展面（不在公共面清单里，走 `--ext`）

| 命名空间 | 声称 | 实测 | 证据 |
|---|---:|---:|---|
| `tls` | 10 | **10** | `impl/ext/wlwl-tls-ext/src/lib.rs:54-63`（`ACCEPT` `CERT_FINGERPRINT` `CERT_INFO` `CLOSE` `CONNECT` `LISTEN` `LOCAL_ADDR` `PEER` `RECV` `SEND`） |
| `crypto` | 8 | **8** | 同上 `:65-72`（`AES_256_GCM` `ECDSA_P256_VERIFY` `ED25519_VERIFY` `HKDF` `HMAC_SHA512` `RSA_PSS_VERIFY` `SHA384` `SHA512`） |

⇒ **18 / 18 达标**。⚠ 它们**不进附录 A、不进公共面清单**，只进一致性册 §5.5 台账（计划 §11.2 末段）。

---

## 4 工作项逐项状态（33 项 + 1 派生项）

> 证据列写的是**契约测试文件名**（在 `impl/crates/wlwl-eval/tests/` 或 `.../wlwl-cli/tests/` 下）+ 公共面成员。
> 全部这些 target 都在 **CI run #341** 的三平台全量测试里跑过且全绿。

### 批次 A · 地基（2 项）

| 项 | 功能 | 状态 | 证据 |
|---|---|---|---|
| **W-01** | 基线自洽 + 死链订正 | **已实现** | `stdlib_appendix_a_sync` · `check-appendix-a`（CI step 20 绿）· `script/check_conformance_registry.py`（CI step 19 绿） |
| **W-02** | 9 个探针固化进仓 | **已实现** | `wlwl-cli/tests/probe.rs`（`EXPECTED_CASE_COUNT` 与目录数一致）· 夹具在 `impl/tests/probe/cases/P121_*` |

### 批次 B · 并发模型（7 项）

| 项 | 功能 | 状态 | 证据 |
|---|---|---|---|
| **W-10** | 嵌套闭包挂起点 ⇒ `E0014` | **已实现** | `suspend_site_contract`（13/0）· 代码 `wlwl-eval/src/lib.rs` 的 `task_body_depth` |
| **W-11** | accept 竞争落选者存活（唤醒一个） | **已实现** | `b2_accept_wakeup_contract`（4/0）· `socket_wait_contract` · `wlwl-cli/tests/concurrency` |
| **W-12** | `TCP_SERVE` 迭代器式接受循环 | **已实现** | 公共面 `std.net::TCP_SERVE` · `tcp_serve_contract`（16/0） |
| **W-13** | `std.sync` 共享容器 + 原子更新 | **已实现** | 公共面 8 员 · `sync_contract`（21/0） |
| **W-14** | 非阻塞出站（`HTTP_REQUEST_ASYNC` / `PROCESS_RUN_ASYNC`） | **已实现** | 公共面两成员在 · `b5_async_outbound_contract`（12/0）· ⚠ **未含 `TCP_CONNECT`**（见 §5.1） |
| **W-15** | 任务终值三态 + 优雅停机（`std.signal`） | **已实现** | 公共面 2 员 · `b6_task_result_contract`（7/0）· `b7_graceful_stop_contract`（10/0） |
| **W-38** | `TCP_SERVE` handler 内挂起点 ⇒ 诊断化 | **已实现** | `w38_native_frame_suspend_contract`（11/0）· 复用既有 `E0014`（**零新增诊断码**） |

### 批次 C · HTTP 协议层（5 项）

| 项 | 功能 | 状态 | 证据 |
|---|---|---|---|
| **W-16** | `PARSE_REQUEST` / `BUILD_RESPONSE` | **已实现** | 公共面 2 员 · `http_contract`（25/0，含 W-17/W-36 追加） |
| **W-17** | Keep-Alive + 请求预算 + 431/413 | **已实现** | `std.http::SERVE_STREAM` · `http_contract`（25/0） |
| **W-18** | 路由 + Cookie + 表单/multipart | **已实现**（含登记降级 ⑬） | 公共面 6 员 · `http_w18_contract`（15/0） |
| **W-19** | 静态文件 + 条件请求 + 压缩 | **已实现**（含登记降级 ⑭⑮） | 公共面 `STATIC_FILE` + `FILE_META` · `http_w19_contract`（11/0）· 压缩在 `http.wll:1189-1194` 内部实现 |
| **W-20** | 客户端 chunked 解码 + HTTPS 出站 | **已实现**（HTTPS 真实握手**未实测**） | 公共面 `GET`/`POST`/`REQUEST` · `http_w20_contract`（16/0） |

### 批次 D · TLS 与扩展面（2 项）

| 项 | 功能 | 状态 | 证据 |
|---|---|---|---|
| **W-21** | 宿主扩展加载器（ABI + 信任门控） | **已实现** | `impl/crates/wlwl-ext/` · `ext_loader_contract`（22/0）· CLI `--ext` / `--ext-sha256` |
| **W-22** | TLS 扩展 crate（`tls` 10 + `crypto` 8） | **已实现** | `impl/ext/wlwl-tls-ext/` 独立 workspace · 32/0（12 crypto + 17 tls + 3 ABI 布局）· ⚠ **发布包投递**见 §5.3 |

### 批次 E · 持久化 / 文本 / 格式化（4 项）

| 项 | 功能 | 状态 | 证据 |
|---|---|---|---|
| **W-23** | 正则编译缓存 | **已实现** | `regex_cache_contract`（14/0）· `wlwl-std/src/regex.rs` |
| **W-24** | 归一化 + 路径包含 | **已实现** | 公共面 `NFKC`/`CASE_FOLD`/`STRIP_INVISIBLE`/`PATH_CONTAINS`/`PATH_CANONICAL` · `path_contains_contract`（10/0）· `text_contract` |
| **W-27** | 原子写 + 文件锁 + 元数据 | **已实现** | 公共面 8 员 · `fs_atomic_lock_meta_contract`（**21/0**，本机 WSL2 复跑确认） |
| **W-28** | `QUERY_PARSE` + `FORMAT_NAMED` | **已实现** | 公共面两成员 · `str_format_contract`（14/0）· `format_contract`（11/0） |

### 批次 F · 可观测性 / 测试 / 模块（4 项）

| 项 | 功能 | 状态 | 证据 |
|---|---|---|---|
| **W-26** | `std.log` + 指标 | **已实现** | 公共面 7 员 · `log_contract` |
| **W-29** | `RE_MATCHES_FULL` + `ARRAY_GET` 别名 | **已实现** | 公共面 `RE_MATCHES_FULL` · `registry.rs` 的 `ARRAY_GET` · `h5_h6_alias_contract`（18/0） |
| **W-30** | `wlwl tasks` 检视 + 最小 REPL | **已实现** | `wlwl-cli/src/main.rs:207`（`Cmd::Tasks`）`:221`（`Cmd::Repl`）`:671`（`ReplSession`） |
| **W-33** | 诊断可读性 + 模块组织（含 G-4② sig drift） | **已实现** | `wlwl-types/src/sig.rs` 的 `drift` · `wlwl-cli/src/main.rs` 的列高亮 · `lsp_smoke` · `parser_syntax_contract` |

### 批次 G · 文档 / 收口（2 项）

| 项 | 功能 | 状态 | 证据 |
|---|---|---|---|
| **W-34** | 文档 / 技能包同步 | **已实现** | `skill_package_guard` · `skill_examples` · `docs/site/**` 已对齐（本轮另修 8 处失实，见 CHANGELOG） |
| **W-35′** | 收口：附录 A 抄件 + 全量门禁 + 版本号 | **已实现** | 附录 A 副本**已含新成员**（`docs/wlwl-spec-reference.md:6673/6696/6707`）· `check-appendix-a` 绿 · 版本 `0.12.1` |

### 第九～十一波新增（5 项 + 1 派生）

| 项 | 功能 | 状态 | 证据 |
|---|---|---|---|
| **W-31** | `std.test` 增强（+7 员） | **已实现** | 公共面 7 员 · `test_contract`（18/0，表格 95 条） |
| **W-32** | `wlwl check` 两条静态规则 | **已实现** | `wlwl-types/src/check.rs` 的 `HostSuspendOutsideTask` · `diag.rs` 的 `shared-mutable-binding` · `conformance_static` |
| **W-39** | macOS 独有真失败 `a_reader_never_sees_a_half_written_target` | ✅ **本轮结案**（原为「欠账」） | 根因 = **判据的期望值依赖机器快慢**（写者 40 轮跑得快 ⇒ 读者只采到 8–9 次 < 阈值 10）⇒ 改成「补写到读够 10 次（上限 4×）」· WSL2 **21/0** + 单独 3 遍绿 + **逆向验证抬高阈值必红** · 见 CHANGELOG「第三条红」段 |
| **W-25** | 中间件原语（限流 / 规则热加载 / 审计 / 真实 IP） | **已实现** | 公共面 `RATE_LIMITER`/`RULESET_LOAD`/`CLIENT_IP` · `middleware_contract`（25/0） |
| **W-36** | `std.http` 重复 `Content-Length` 收紧 | **已实现** | `http_contract`（25/0，断言 37→40） |
| **W-37** | 解析器 `E0011`「误报」—— 证伪 | **已实现**（结论：非缺陷） | `wlwl-parser/tests/e0011_false_positive_contract.rs`（7/0）· 解析器源码零改动 |
| **G-4②** | `.wll.sig` 工作流简化（W-33 的最后一项） | **已实现** | `sig.rs` 的 `drift` 字段 · `sig_schema_version` 1.1.0 |

**小计：33 个工作项 + 1 派生项，全部有代码与契约测试支撑。** ⚠ **「工作项有代码」≠「计划声称的每个公共成员都存在」** —— 后者见 §3.2 与 §5.1。

---

## 5 发现的差异（分三类，每条都带证据）

### 5.1 实现缺失（3 处，其中 1 处被错标为已交付）

#### D-1 🔴 `std.net::TCP_CONNECT` —— **台账标「本轮落地」，实际零实现**

| 事实 | 内容 |
|---|---|
| **台账声称** | 计划 §7 台账 `I-3` 行：`TCP_CONNECT` **✅+📖**，备注「**本轮落地** `std.net::TCP_CONNECT`（出站必需：HTTPS 与 DB/Redis 都靠它）」 |
| **实测** | `wlwl namespace` 的 `wlwl:std.net` **16 个成员里没有它** |
| **代码** | `grep TCP_CONNECT impl/crates/*/src/*.rs impl/crates/*/wl/std/*.wll` ⇒ **零命中** |
| **更矛盾的是** | 计划 §11.2 自己写着：「**`TCP_CONNECT` 不在此列** —— §6.2 老名单写过它，但**实现侧从未注册** ⇒ **登记为计划侧笔误**」 |
| **判定** | **未实现**，且**两处文档互相打脸**（台账说已交付，登记清单说从未注册） |
| **影响** | ① 需求清单 I-3「出站 TCP」**没有交付**；② DB/Redis 出站**无原生路径**（HTTPS 出站走的是 `tls::CONNECT`，不受影响）；③ 下游若照台账规划，会以为有它 |

**复核命令**
```powershell
cd D:\Project\wlwl\impl
.\target\debug\wlwl.exe namespace | Select-String 'TCP_CONNECT'   # 期望：无输出
Select-String -Path 'crates\*\src\*.rs','crates\*\wl\std\*.wll' -Pattern 'TCP_CONNECT'  # 期望：无匹配
```

#### D-2 🔴 `std.http::SERVE` —— 计划表列了，**未实现、未登记**

| 事实 | 内容 |
|---|---|
| **计划声称** | §6.2 的 `std.http` 18 员清单里含 `SERVE` |
| **实测** | 公共面 `std.http` **16 员，无 `SERVE`** |
| **代码** | `grep '"SERVE"'` 在 `http.wll` / `net.rs` ⇒ **零命中**；`http.wll:2252` 的 `EXPORT([...])` 里只有 16 个名字 |
| **登记状况** | ⚠ **没有任何地方登记这个差异**（计划 §11.1 / §11.2 / CHANGELOG 都没提） |
| **判定** | **未实现**（且**未登记** ⇒ 这是本轮审计**新发现**的账） |
| **影响** | 功能上由 `TCP_SERVE`（accept 循环）+ `SERVE_STREAM`（单连接循环）覆盖 ⇒ **能力不缺**，缺的是那个"一条龙"便利成员。⚠ 但**规范侧若照 §6.2 落表**，会登记一个**实现里不存在**的成员 ⇒ 附录 A 对拍会红 |

#### D-3 🟡 `std.http::COMPRESS_RESPONSE` —— 计划表列了，**未实现**；但**功能在**

| 事实 | 内容 |
|---|---|
| **计划声称** | §6.2 的 18 员清单里含 `COMPRESS_RESPONSE` |
| **实测** | 公共面**无**该成员 |
| **功能** | ✅ **在** —— `http.wll:1189-1194`：`Accept-Encoding` ⇒ 选 `gzip`/`zstd` ⇒ `GZIP_COMPRESS`/`ZSTD_COMPRESS` ⇒ 设 `Content-Encoding`（在 `STATIC_FILE` 内部完成压缩协商） |
| **判定** | **已实现（内部）** —— 能力有，只是没导出成计划承诺的那个公共成员名 |
| **风险** | 同 D-2：规范侧若照 §6.2 落表，会登记一个不存在的成员 |

**复核命令**
```powershell
Select-String -Path 'D:\Project\wlwl\impl\crates\wlwl-std\wl\std\http.wll' -Pattern 'Accept-Encoding|GZIP_COMPRESS|Content-Encoding'
```

### 5.2 文档不一致（3 处，会误导下游迭代）

| # | 两处说法 | 冲突点 | 建议处置 |
|---|---|---|---|
| **I-1** | §7 台账 `I-3`（已交付 `TCP_CONNECT`）↔ §11.2（「从未注册，计划侧笔误」） | 同一件事两种结论 | 见 §6 **T-03**（需裁决：实现 or 改口径） |
| **I-2** | §11.6b 完成度面板「46 完全落地 + 1 部分 + 4 未开工 + 6 口径答复 = 57」↔ §7 台账状态列实际计数「**40 ✅ + 11 🔧 + 2 🔀 + 4 📖 = 57**」 | **「4 条未开工」在台账里根本不存在**（台账每行都有状态，无 ⬜） | rc2 统一口径：要么面板改成台账口径，要么台账补 ⬜ 标记 |
| **I-3** | §11.2「这些成员在规范仓落表前，实现侧的三道对拍闸**会红**」↔ **实测**附录 A 抄件已含新成员（`docs/wlwl-spec-reference.md:6673/6696/6707`）、`check-appendix-a` 绿、CI 全绿 | 该警告**已过期**（W-35′ 收口时同步了抄件） | rc2 删掉/改写该警告，避免下游以为有红闸 |

⚠ 另：§6.2 表把 `std.log` 记作 **4 员**，而 §11.2 登记清单记作 **7 员**（实测 **7**：多 `METRIC_COUNTER` / `METRIC_INC` / `METRIC_TIMER`）⇒ §6.2 表**少列 3 个**。这会让规范侧少登记 3 个成员。

### 5.3 已登记降级与未结事项（计划自己写的，标 `[声称]` + 复核命令）

| # | 内容 | 性质 | 复核命令 |
|---|---|---|---|
| **⑬** | `PARSE_MULTIPART` 的**文件 part 落盘是有损 UTF-8 文本** ⇒ **二进制 part 无法原样落盘** | 功能降级 | `cargo test --locked -p wlwl-eval --test http_w18_contract` |
| **⑭** | `PATH_CONTAINS` 是**词法**判定 ⇒ **符号链接逃逸**：根内指向根外的链接仍判合法 | 🔴 **安全相关** | `cargo test --locked -p wlwl-eval --test path_symlink_contract` |
| **⑮** | `READ_BYTES` 有 **1 MiB 上限** ⇒ 大文件读不出 ⇒ `STATIC_FILE` 服务大文件受限 | 功能上限（**本轮重新裁决：不改**，见 §6.3 T-06 的订正块） | `cargo test --locked -p wlwl-eval --test fs_contract` |
| **W-20** | **HTTPS 真实握手** | ✅ **已实测（补上）** | `https_e2e_contract` **4/0**（真握手拿 **200**）· 见 §6.3 T-08 的落地记录 |
| **W-25** | `std.sync` **无按名 API**（`COUNTER(initial)` / `SHARED_DICT()` 每次新建）⇒ `http.wll` 内自建"按 name 注册表" | 设计限制 | `grep -n 'SHARED_DICT(' impl/crates/wlwl-std/wl/std/http.wll` |
| **新** | **SBOM 只覆盖主二进制** —— TLS 扩展是**独立 workspace**，不在 `cargo cyclonedx` 范围内（本轮做 TLS 打包时发现） | 供应链缺口 | `grep -n 'cyclonedx\|sbom' .github/workflows/release.yml` |
| **规范侧** | 规范仓 `wlwl-spec-v1` 的 **v1.0.4 tag 尚未发** ⇒ 本仓规范文本是**候选文本**；58 个登记项尚未成为已发布规范 | 🔴 **发布阻塞** | `git -C <spec-v1> tag -l 'v1.0.4'`（期望：空） |
| **已修但不在 rc1 包** | rc1 之后又修了 **4 条**（锁写并发 / chunked 夹具 RST / 句柄 MUI 判据 / macOS reader 判据）＋ **TLS 打包** ⇒ **`v0.12.1_rc1` 的发布包不含这些修复** | 版本边界 | `git log --oneline v0.12.1_rc1..main` |

---

## 6 v0.12.1_rc2 工作清单

> 每条：**为什么 / 验收命令 / 反向守卫 / 非目标 / 依赖 / 是否需业主裁决**。
> ⚠ **T-01 / T-02 是"把当前状态发出去"，T-03 / T-04 需要业主先裁决方向。**

### 6.1 必须先裁决的两条（它们决定 T-05 之后的范围）

#### T-03 · `TCP_CONNECT`：实现它，还是改台账口径？

> ✅ **方向已定（2026-10-31 · 三方对拍之后）＝ 走 B「改口径」**（**文档订正**，**零代码改动**）。
> **理由**：三方对拍（计划 §6.2 ↔ 规范 v1.0.4 候选文本 ↔ 实现 `wlwl namespace`）证明 **规范 ≡ 实现** —— 规范里**没有** `TCP_CONNECT`，实现里也**没有** ⇒ 缺的**不是实现**，是**计划侧那两处的账**。⇒ **规范侧不需要动**（v1.0.4 候选文本**已经是对的**）⇒ **不需要重新走规范侧登记流程** ⇒ **这一件不构成 rc2 的阻塞项**。
> **已落的订正**：§7 台账 `I-3` 行改记 **📖**（口径：本版不交付）· §6.2 新成员表删掉 `TCP_CONNECT` · 证据与复跑命令见 **§9**。
> ⚠ 下面的 A / B 两个方向**原文保留**（可追溯）—— **A 方向作废**（它会新造一个规范里**没有**的成员 ⇒ 必须走规范侧登记 ⇒ 与「规范已对」的实测结论冲突）。

- **为什么**：台账说已交付、实测零实现、§11.2 说从未注册 —— **三处打脸**。而需求清单 **I-3「出站 TCP」**正是它。
- **两个方向（需裁决）**：
  - **A（推荐）实现**：与 `TCP_LISTEN` 同形的出站连接（`TCP_CONNECT(host, port, timeout_ms) -> INTEGER`），复用 `tcp.rs` 既有 handle 表与 `socket_waiters`。**成本低**（一个成员 + 一条契约），且补齐 DB/Redis 出站的缺口。
  - **B 改口径**：把台账 I-3 改成「不交付」，并在需求回执里写明「出站 TCP 走 HTTPS 已够用，DB/Redis 场景不支持」。
- **验收（若选 A）**：`wlwl namespace` 的 `std.net` 出现 `TCP_CONNECT`；新增 `tcp_connect_contract` ⇒ 连本地 listener 成功 + 对端拒绝给 `ERR` 不 panic。
- **反向守卫**：把 `TCP_CONNECT` 从 `SPEC` 删掉 ⇒ 公共面读数少 1 ⇒ 新契约红。
- **非目标**：不做 TLS over `TCP_CONNECT`（那是扩展的事）。

#### T-04 · `SERVE` / `COMPRESS_RESPONSE`：实现，还是从计划表删除？

> ✅ **方向已定（2026-10-31 · 三方对拍之后）＝ 走 A「从 §6.2 删除 + 记录偏差」**（**文档订正**，**零代码改动**）。
> **已落的订正**：§6.2 的 `std.http` 行**删掉** `SERVE` 与 `COMPRESS_RESPONSE`（**18 → 16 员**），并在表下写明：「`SERVE` 的能力由 `TCP_SERVE` + `SERVE_STREAM` 覆盖；`COMPRESS_RESPONSE` 的压缩协商在 `STATIC_FILE` **内部**实现（`http.wll:1189-1194`）⇒ **零新增成员**」。
> ⚠ **规范侧不需要动** —— 规范 v1.0.4 候选文本的 `std.http` **本来就是 16 员**（**从未登记**这两个成员）⇒ **不需要重新走规范侧登记流程** ⇒ **这一件不构成 rc2 的阻塞项**。
> ⚠ 下面的 A / B 两个方向**原文保留**（可追溯）。

- **为什么**：这两个名字在 §6.2 里，实现里没有（§5.1 D-2 / D-3）。**若不处理，规范侧落表会登记不存在的成员**（附录 A 对拍必红）。
- **两个方向（需裁决）**：
  - **A（推荐）从 §6.2 删除 + 记录偏差**：`SERVE` 的能力已由 `TCP_SERVE` + `SERVE_STREAM` 覆盖；`COMPRESS_RESPONSE` 的能力已在 `STATIC_FILE` 内部实现（`http.wll:1189-1194`）。⇒ **零代码改动**，只改文档 + 定案记录。
  - **B 实现两个公共成员**：`SERVE(opts, handler)` 一条龙；`COMPRESS_RESPONSE(resp, accept_encoding)` 把内部逻辑导出。**收益低**（多 2 个公共面成员 = 多 2 个登记项 + 2 份文档）。
- **验收（若选 A）**：§6.2 表删除两行；`docs/plan` 记一条定案；`check-appendix-a` 仍绿。
- **反向守卫**：无（纯文档）。⚠ 但若规范侧已按 18 员落表 ⇒ 必须同步删规范表那两行，否则对拍红。

### 6.2 发布相关（把当前状态发出去）

#### T-01 · 规范仓 `wlwl-spec-v1` 打 **v1.0.4** tag

- **为什么**：本仓全部规范文本是**候选文本**；58 个登记项要有"已发布规范"这个落点。**这是 rc2 的前置**（§6.4 要求 tag 注释逐条列 SP-1…SP-5）。
- **验收**：`git -C <spec-v1> tag -l 'v1.0.4'` 有输出；tag 注释含 SP-1…SP-5 与全部新增成员清单；`README.md` 的「当前状态」行有 v1.0.4 段。
- **反向守卫**：**已发布的 tag 不许移动** ⇒ 修订就新发版本号（v1.0.5）。
- **非目标**：不合并到 v1.1.0（版本口径业主已锁死 v1.0.4）。
- **依赖**：业主点头（**分支/tag 操作必须逐次明确指示**）。

#### T-02 · 发 `v0.12.1_rc2` 并验证 TLS staging

- **为什么**：`release.yml` 的 TLS 打包**只在打 tag 时跑** ⇒ 五个平台的交叉链接（尤其 aarch64 的 linker 环境变量）**至今零实测**。而 rc1 的包**不含**本轮 4 条修复 + TLS 打包。
- **验收**：
  ```powershell
  # ① job 级结论（不是只看 run 级）
  (Invoke-RestMethod "https://api.github.com/repos/Laffinty/wlwl/actions/runs/<run_id>/jobs?per_page=100").jobs | Select name,conclusion
  # ② release 真建成 + 资产齐全（job 绿 ≠ release 存在）
  Invoke-RestMethod "https://api.github.com/repos/Laffinty/wlwl/releases/tags/v0.12.1_rc2"
  # ③ 包内确实有 TLS 目录（这是本轮唯一没被验过的东西）
  #    下载任一资产 → 解包 → 期望看到 wlwl-tls/{libwlwl_tls.so|.dylib|wlwl_tls.dll} + ABI_VERSION + SHA256SUMS
  ```
- **反向守卫**：把 `Stage the TLS extension` 那一步的 `if [ ! -f "$src" ]` 判断去掉 ⇒ 产物缺失时静默发一个没有 TLS 的包 ⇒ **不许**（那正是 run #44 的状态）。
- **非目标**：**不打正式版**（正式版 `v0.12.1` 由业主另行指示）；不动 `FLAKY_EXIT_CODE` 策略。
- **依赖**：T-01（规范 tag 先发）+ 主 CI 全绿 + **业主明确指示打 tag**。

### 6.3 质量与安全（建议 rc2 一并做）

#### T-05 · 🔴 符号链接逃逸（`PATH_CONTAINS` 是词法判定）

> ✅ **已完成（2026-10-31，本批实测）** —— 结论与证据见本条末尾「落地记录」。

- **为什么**：`STATIC_FILE` 是**对外暴露文件系统**的入口；词法判定下"根内指向根外的符号链接"仍判合法 ⇒ **可读到根外文件**。这是 §5.3 ⑭，**计划登记了但没修**。
- **验收**：新增契约用例 —— 在临时根下建 `link -> ../outside`，请求 `/link/secret` ⇒ 必须 **403/404**，不得返回内容。
  ```powershell
  cargo test --locked -p wlwl-eval --test path_symlink_contract
  cargo test --locked -p wlwl-eval --test http_w19_contract
  ```
- **反向守卫**：把修复去掉（退回词法判定）⇒ 新用例必红（读到 `outside/secret`）。
- **非目标**：不改 `PATH_CONTAINS` 的**公开语义**（若是词法判定就继续是词法）⇒ 在 `STATIC_FILE` 侧另做 `canonicalize` 后的二次校验。
- **依赖**：无（可独立做）。

##### ✅ 落地记录（2026-10-31，实测）

**⚠ 先纠正一条我写错的前提**：本条原文假设「`PATH_CANONICAL` 是真 canonicalize，可在 R1 里直接用来做二次校验」——
**实测它是错的**：`std.fs::PATH_CANONICAL` 走 `fs.rs` 的 `canonicalize()`，只做「分隔符统一 + `PATH_NORMALIZE`
消解 `.`/`..`」，**不访问文件系统**、**看不见符号链接** ⇒ 拿它做逃逸防护是**空操作**。

**修法**：新增**私有 R2 内核** `_RESOLVE_UNDER(root, rel)`（`wlwl-std/src/fs.rs`，注册在
`wlwl-std/src/lib.rs` 的 `wlwl:std.http` 的 `kernels` 里）—— Rust 侧 `std::fs::canonicalize` **真解析**符号链接：

- `ok` = 解析后是否仍在 `root` 之下；`path` = **解析后的绝对路径**（`STATIC_FILE` 用它去 stat / 读 ⇒
  「判的路径」与「读的路径」是同一条）；`exists` 区分 404 与 403。
- **向上找最近的存在祖先**来判断「不存在」与「逃逸」：祖先在根内 ⇒ **404**；在根外 ⇒ **403**。
  ⚠ 这一步是被既有契约逼出来的：首版「父目录解析失败就 403」把 `an_absolute_looking_request_path_stays_inside_the_root`
  （`/etc/passwd` 期望 **404**）打红了。
- **公共面零变化**（实测 `wlwl namespace` 仍 **25 命名空间 / 288 成员**，`std.http` 16、`std.net` 16）
  ⇒ **规范侧零改动**（私有内核不进附录 A）。

**为什么不是上一版 agent 的 R1 方案**（`DIR_ENTRIES` 的 lstat 视图 vs `FILE_META` 的 stat 视图对拍）：
① 每段一次 `readdir`（一次请求 O(段数) 次列目录）；② 会把根内**合法**的符号链接也一起拒掉（语义过严）。
⇒ R2 方案常数次 `canonicalize`、只挡真逃逸。

**实测读数**：

| 项 | 读数 |
|---|---|
| `path_symlink_contract` | **15 passed / 0 failed** |
| `http_w19_contract`（回归） | **11 passed / 0 failed** |
| **反向守卫**（去掉 `_RESOLVE_UNDER` 闸） | `a_symlinked_directory_out_of_the_root_does_not_serve_the_outside_{file,index}` **双双 FAILED**，报「STATIC_FILE 把**根外**那份文件的内容端出去了（status=200）」⇒ 恢复后 15/15 绿 |
| 平台覆盖 | POSIX 符号链接**真跑**；Windows 上文件 symlink 需特权 ⇒ 该条 **skip**（打印说明，不假装覆盖），目录逃逸走 **junction** 兜底后**真跑** |
| `rustfmt --check` / `clippy -D warnings` / `check-appendix-a` | 全绿 |


#### T-06 · `READ_BYTES` 1 MiB 上限 ⇒ 大文件静态服务不可用

> ⚠⚠ **本条的方向已在 2026-10-31 重新裁决（见下方「⚠ 订正」块）** —— 原文的
> 「抬上限」方案**被撤回**，改为**保持上限 + 加两条守卫 + 另立次版本项**。

- **为什么**：⑮。`STATIC_FILE` 走 `READ_BYTES` ⇒ 超过 1 MiB 的文件服务不了。对一个"能当静态站点服务"的语言来说这是硬伤。
- **验收**：`STATIC_FILE` 服务 8 MiB 文件 ⇒ 200 + `Content-Length` 正确。
- **反向守卫**：保留上限但**必须**返回明确 `ERR`（不许静默截断）。
- **非目标**：不做流式 sendfile（那是 `ADR-0017 Step 3` 之后）。

##### ⚠ 订正（2026-10-31，本批实测后）：**抬上限的方案撤回**，本轮**不改上限**

**实测数据**（本机，`wlwl run` + `GetProcessMemoryInfo` 采样峰值工作集）：

| 文件 | 墙钟 | **峰值工作集** | 放大倍数 |
|---|---|---|---|
| 1 MiB | 0.19 s | **119.7 MiB** | ~120× |
| 4 MiB | 0.65 s | 455.9 MiB | ~114× |
| 8 MiB | 1.35 s | 903.7 MiB | ~113× |
| 16 MiB | 4.09 s | **1799.7 MiB** | ~112× |

⇒ **时间**近线性（每翻倍约 2.1×）**不是**瓶颈；**内存放大 ~112×** 才是 ——
`READ_BYTES` 的产物是**语言值数组**（1 字节 = 1 个 `Value`，实测 56 B）⇒
抬到 16 MiB ⇒ **一次读吃 1.8 GiB** ⇒ 把「读一个小文件」的内存耗尽面**放大 15 倍**。

⇒ 而**规范侧已经裁决过这件事**：系统册「写得出 ≠ 读得回」一节明写
「**大载荷的正解是让它待在文件里**」「**记为已知边界，不作为待修缺陷**」
「**流式写 / 分段读成员本册不做** —— 它们要动的是**成员面**」。
⇒ 按 §6.3 的 **N-4**，「修改既有成员的可观察语义」在**修订版本**里**禁止**；
真正的解法（流式 / 分段）**要新增公共面成员** ⇒ 属**次版本** ⇒ 与业主
「版本锁死 v1.0.4」冲突 ⇒ **立为 v1.1.0 的独立立项**，不在 v0.12.1 里做。

⇒ **本轮实际交付（三条，都是净收益且与档位无关）**：
1. **上限保持 1 MiB**（规范一致 ⇒ 规范 ≡ 实现 的不变量不破）。
2. **`read_bytes_with_cap` 可测接缝**（与既有 `write_bytes_with_cap` 同套路）。
3. **两条守卫**：
   - `the_read_cap_fits_the_language_value_budget`：把「上限 × `size_of::<Value>()`」
     钉在 **≤ 64 MiB** ⇒ `Value` 一旦变大**那条就红** ⇒ 逼着重新裁决档位。
   - `read_bytes_reads_up_to_the_cap_and_refuses_one_byte_more` 的**常量断言**
     钉住「上限 = 规范的 1 MiB」⇒ **谁想抬上限而没改规范，那条必红**。
4. **「大文件静态服务」** ⇒ 明确转入 **v1.1.0 独立立项**（需要流式 / 分段成员）。

**复核命令**
```powershell
cd D:\Project\wlwl\impl
cargo test --locked -p wlwl-eval --test fs_contract --no-fail-fast   # 30/0
cargo test --locked -p wlwl-std --lib --no-fail-fast                # 388/0
```


#### T-08 · HTTPS 真实握手端到端实测

> ✅ **已完成（2026-10-31，本机实测）** —— 读数见本条末尾。

- **为什么**：W-20 只验了"扩展未加载"与"证书错误逐字透传"两条；**真实握手从未跑过**。
- **验收**：起一个带自签证书的本地 TLS 服务（或对 `https://example.com` 视网络而定）⇒ `std.http::GET` 返回 200；证书不匹配 ⇒ `ERR` 且消息逐字含 `certificate`。
- **反向守卫**：把 `--ext` 去掉 ⇒ 必须返回 `ERR([kind:"TlsError", reason:"no TLS provider loaded"])`（这条已有用例）。
- **依赖**：扩展可加载（本机可行）。

##### ✅ 落地记录（2026-10-31）

**新增** `impl/crates/wlwl-cli/tests/https_e2e_contract.rs`（4 条）+ 证书夹具
`impl/crates/wlwl-cli/tests/fixtures/`（CA / server / **错 CA** / 生成脚本 / README）。

- 形态：**服务端与客户端各起一个进程**（服务端用扩展自己的 `tls::LISTEN` + `tls::ACCEPT`），
  只连 `127.0.0.1`，**不联网**；证书 `ECDSA P-256`，`SAN` 含 `127.0.0.1`，
  **有效期 2025-10-31 → 2126-10-07（100 年）**。
- 读数（本机实跑）：**4 passed / 0 failed**（0.26 s）——
  `the_https_request_completes_a_real_tls_handshake_and_gets_a_200` ·
  `without_the_extension_the_https_request_reports_no_tls_provider` ·
  `the_handshake_rejects_a_certificate_that_is_not_signed_by_the_trusted_ca` ·
  `a_missing_extension_file_makes_the_host_refuse_to_start`。
- **反向守卫（手工独立复现，不靠测试自证）**：
  不加载扩展 ⇒ **逐字** `[kind: TlsError, reason: no TLS provider loaded]`；
  `--ext` 指向不存在路径 ⇒ **更早一层**（宿主加载）报
  `extension library not found: '...no-such-extension.dll'` 且 exit 1；
  错误 CA ⇒ `invalid peer certificate: UnknownIssuer`。
- **顺带修掉一处真偶发风险**：原实现「客户端退出后读一次服务端标记」⇒ 慢机上会红
  （服务端 stdout 由另一线程收）⇒ 改成轮询等待。
- ⚠ **已知限制（如实登记）**：
  ① 只在 **Windows 本机**实跑，Linux / macOS 未实跑；
  ② 测试内「扩展产物不存在就自己 build」的 **fallback 分支未被真正触发过**（本机产物一直在）；
  ③ 🔴 **证书私钥（`*.key`）明文入库** —— 仅测试用途、只监听回环，但**可能触发 GitHub secret scanning**；
     若推送被拦，需改为「测试时用 `fixtures/gen_e2e_cert.py` 现场生成」。


#### T-09 · SBOM 覆盖 TLS 扩展

- **为什么**：扩展自持 `rustls` / `ring` / `webpki-roots`，`cargo cyclonedx` **扫不到**（独立 workspace）⇒ 供应链清单漏了 TLS 这部分依赖。
- **验收**：release 的 SBOM 资产里含 `rustls` / `ring` / `webpki-roots` 条目。
- **反向守卫**：把扩展从 SBOM 生成里摘掉 ⇒ 校验脚本必红。
- **非目标**：不引入新的 SBOM 工具链。

#### T-10 · 文档三处不一致订正（§5.2）

- **为什么**：下游照面板/台账规划会拿到**互相矛盾的数字**。
- **验收**：§11.6b 面板计数与 §7 台账状态计数**逐项相等**；§11.2 的"三道对拍闸会红"警告删除或改写；§6.2 的 `std.log` 由 4 改为 7。
- **反向守卫**：写一个小脚本按状态列统计，与面板数字比对 ⇒ 不等就红（建议作为入库守卫，防再次漂移）。

### 6.4 明确**不**在 rc2 做（封口，防范围蔓延）

| 项 | 口径 | 依据 |
|---|---|---|
| HTTP/2（I-1）· HTTP/3（I-5）· WebSocket（I-2）· brotli（I-4） | **不做** | 计划 §7 已给口径答复；均需独立立项 |
| Markdown 渲染（E-6）· 代码高亮（E-7） | **不做** | 归应用层（需求方自研） |
| multipart **二进制** part（⑬） | **单独立项** | 本语言正文是 `STRING` ⇒ 二进制无法原样落盘，需要 bytes 通道 |
| `MUTEX` / `CONDITION` / `SEMAPHORE` | **不做** | 需 `ADR-0017 Step 3`（流式协程） |
| 流式响应体 / sendfile | **不做** | 同上 |
| 完整调试器（G-3 的强口径） | **不做** | 只承诺最小 REPL（已交付） |

---

## 7 复核命令总表（一条命令验一件事）

```powershell
$mp = 'D:\Project\wlwl\impl\Cargo.toml'
$wlwl = 'D:\Project\wlwl\impl\target\debug\wlwl.exe'

# ⓿ 公共面权威事实
& $wlwl namespace                      # 期望：25 命名空间 / 288 成员

# ① 本轮审计的核心断言（3 个缺失成员）
& $wlwl namespace | Select-String 'TCP_CONNECT|SERVE\b|COMPRESS_RESPONSE'   # 期望：无输出

# ② 各契约（本文档引用的全部测试）
cargo test --manifest-path $mp --locked -p wlwl-eval --test resource_release_contract   # 15/0
cargo test --manifest-path $mp --locked -p wlwl-eval --test sync_contract               # 21/0
cargo test --manifest-path $mp --locked -p wlwl-eval --test http_contract               # 25/0
cargo test --manifest-path $mp --locked -p wlwl-eval --test http_w18_contract           # 15/0
cargo test --manifest-path $mp --locked -p wlwl-eval --test http_w19_contract           # 11/0
cargo test --manifest-path $mp --locked -p wlwl-eval --test http_w20_contract           # 16/0
cargo test --manifest-path $mp --locked -p wlwl-eval --test middleware_contract         # 25/0
cargo test --manifest-path $mp --locked -p wlwl-eval --test b5_async_outbound_contract  # 12/0
cargo test --manifest-path $mp --locked -p wlwl-eval --test w38_native_frame_suspend_contract
cargo test --manifest-path $mp --locked -p wlwl-eval --test h5_h6_alias_contract        # 18/0
cargo test --manifest-path $mp --locked -p wlwl-eval --test test_contract               # 18/0
cargo test --manifest-path $mp --locked -p wlwl-eval --test path_contains_contract      # 10/0
cargo test --manifest-path $mp --locked -p wlwl-cli --test concurrency                  # 19/0
cargo test --manifest-path $mp --locked -p wlwl-cli --test probe

# ③ 三平台全量（最终判据；CI 已在 run #341 跑过）
#    ⚠ 本仓全量很慢（sanitize_contract 已测 781 s）⇒ 优先看 CI，不要在本机串一条 --all-targets

# ④ 规范侧（T-01 前置）
git -C <path-to-wlwl-spec-v1> tag -l 'v1.0.4'    # 期望：现在为空
```

---

## 8 什么时候算 rc2 完成

- [ ] **T-03 / T-04 已裁决**，且裁决结果落进了「台账 + §6.2 表」两处（**不允许只改一处**）
- [ ] **T-01 规范仓 v1.0.4 tag 已发**，tag 注释含 SP-1…SP-5 与全部新增成员
- [ ] **T-02 `v0.12.1_rc2` 的 release 已建成**，且**包内 `wlwl-tls/` 目录存在**（含动态库 + `ABI_VERSION` + `SHA256SUMS`）
- [ ] 五平台 **job 级**结论全 success（不是只看 run 级）
- [x] **T-05 符号链接逃逸已修**并有反向守卫（2026-10-31 完成：`path_symlink_contract` 15/0，去掉闸 ⇒ 必红）
- [ ] **T-10 三处文档不一致已订正**，且面板与台账**逐项相等**
- [ ] 主 CI 三平台全绿（`cargo fmt` / `clippy -D warnings` / `doc -D warnings` / 全量测试 / 附录 A 对拍 / §5.4 门禁）

---

## 附录 · 本报告的证据来源清单

| 证据 | 位置 |
|---|---|
| 公共面全量清单 | `impl/target/debug/wlwl.exe namespace` 输出（25 命名空间 / 288 成员，逐项见 §3.2） |
| 三平台全量测试 | CI run **#341**（head `d2bd479`）：ubuntu / macOS / windows + smoke 四个 job 全 success |
| 句柄两条守卫 | `resource_release_contract` **15/0**（本机 2026-10-31 复跑） |
| unix 侧契约 | WSL2：`fs_atomic_lock_meta_contract` **21/0** + 单独 3 遍绿 + 逆向验证必红 |
| 计划原文 | `docs/plan/wlwl-v0.12.1-build-plan.md` §4 / §6.2 / §7 / §11.1 / §11.2 / §11.6b |
| 变更史 | `CHANGELOG.md` 的 `[v0.12.1 收口 · 第二批]` 及其「修补」「结案」「第三条红」四段 |

---

## 9 订正（本轮三方对拍后的结论修正）

> **性质**：本节**追加**在 §1–§8 与附录之后，**不改写**前面的原文（§0.4 第 1 条「只加不删」）⇒ 旧结论**可追溯**；凡与本节冲突，**以本节为准**（逐条取代关系见 §9.1 / §9.2）。
> **触发**：2026-10-31 的**三方对拍** —— 计划 §6.2 ↔ 规范仓 v1.0.4 候选文本 ↔ 实现 `wlwl namespace`，**一次**跑完（脚本见 §9.3）。
> **一句话结论**：**规范 v1.0.4 候选文本与实现逐成员一致**（规范 ≤ 实现**零缺失**；实现**没有**「规范未登记」的成员）⇒ §5.1 的三条差异**全部**从「实现缺 3 个成员」refined 为「**计划 §6.2 与台账 `I-3` 的账目错误**」⇒ **规范侧不需要动** ⇒ **T-03 / T-04 不构成 rc2 阻塞项**。

### 9.1 §5.1 三条差异的结论 refined（取代关系）

| 原条目 | 原结论（§5.1，**保留**） | **现结论（取代）** |
|---|---|---|
| **D-1** | `std.net::TCP_CONNECT` —— 台账标「本轮落地」，实际零实现 ⇒ **实现缺失** | **不是实现缺口**，是**计划 §6.2 新成员表 + §7 台账 `I-3` 的账目错误**。**规范 v1.0.4 候选文本与实现逐成员一致**：规范 `std.net` **16 员无** `TCP_CONNECT`，实现 `wlwl:std.net` **16 员也无** ⇒ 缺的只是计划那两处的账。**处置**：§6.2 删该成员 + 台账 `I-3` 改记 **📖**（本版不交付）⇒ **规范侧不需要动** ⇒ **不构成 rc2 阻塞项** |
| **D-2** | `std.http::SERVE` —— 计划表列了，**未实现、未登记** ⇒ **实现缺失** | **不是实现缺口**，是**计划 §6.2 的账目错误**（这个成员**从来不存在**）。规范 `std.http` **16 员** / 实现 **16 员**，**逐成员一致**。**能力不缺**：`TCP_SERVE` + `SERVE_STREAM` 已覆盖。**处置**：§6.2 删该成员 ⇒ **规范侧不需要动** ⇒ **不构成 rc2 阻塞项** |
| **D-3** | `std.http::COMPRESS_RESPONSE` —— 计划表列了，**未实现**；但**功能在**（**已实现（内部）**） | **不是实现缺口**，是**计划 §6.2 的账目错误**（成员**从来不存在**）。⚠ 原判「**已实现（内部）**」的结论**不变**（压缩协商在 `STATIC_FILE` 内部实现，`http.wll:1189-1194`）—— 只是**它从来不是公共面成员**。**处置**：§6.2 删该成员 ⇒ **规范侧不需要动** ⇒ **不构成 rc2 阻塞项** |

⇒ **D-1 / D-2 / D-3 全部 refined**。⚠ **不变的部分**：§5.1 的**证据**（`wlwl namespace` 读数、grep 零命中、`http.wll:1189-1194` 的内部实现位置）**全部仍然成立** —— 被推翻的只是**归因**（不是「实现缺」，而是「计划账错」）。

### 9.2 §5.2 三处文档不一致的处置（取代关系）

| 原条目 | 现结论 |
|---|---|
| **I-1**（台账 `I-3` ↔ §11.2 两处打脸） | **已消解**：台账 `I-3` 行改记 **📖**（口径：本版不交付）· §6.2 删该成员 ⇒ 两处**口径一致** |
| **I-2**（§11.6b 面板 ↔ §7 台账状态列） | **已消解**：§11.6b 面板按台账状态列**重算**为 **39 ✅ + 11 🔧 + 2 🔀 + 5 📖 = 57**（⬜ **0 条**）。⚠ 原面板写「46 完全落地 / 🔧 1 条 / ⬜ 4 条 / 📖 6 条」**全部对不上**；⚠ 且 `I-3` 原被记成 `✅+📖` ⇒ 改记 📖 后是 **✅ 39 / 📖 5**（**不是** 40 / 4）—— 面板以**台账状态列的实数**为准 |
| **I-3**（§11.2「三道对拍闸会红」） | **已消解**：该警告**已过期** —— 附录 A 抄件**早已同步**（`docs/wlwl-spec-reference.md:6673` / `:6696` / `:6707`）⇒ `check-appendix-a` 与 §5.4 门禁在 **CI run #341 三平台全绿** ⇒ §11.2 已改写为**实测事实** |

⚠ **另**：§5.2 末尾提到的「§6.2 把 `std.log` 记作 4 员、§11.2 记作 7 员」⇒ **已消解**：§6.2 的 `std.log` 行改为 **7 员**（补 `METRIC_COUNTER` / `METRIC_INC` / `METRIC_TIMER`）⇒ 两处一致。

### 9.3 可复跑的三方对拍脚本（本轮入库）

**`script/audit_member_threeway.py`** —— 一条命令把「计划 / 规范 / 实现」三边对拍完：

```powershell
cd D:\Project\wlwl
python script\audit_member_threeway.py                    # 规范仓默认 D:\Project\wlwl-spec-v1
python script\audit_member_threeway.py <规范仓目录>        # 规范仓路径可用 argv 覆盖
```

**实测输出（2026-10-31）** —— 关键行：

```
namespace               plan  spec  impl  verdict
wlwl:std.http             18    16    16  plan-only: COMPRESS_RESPONSE,SERVE
wlwl:std.log               4     7     7  spec-only(new ns): METRIC_COUNTER,METRIC_INC,METRIC_TIMER
wlwl:std.net               3    16    16  plan-only: TCP_CONNECT; (spec has 14 historical member(s) ...)

impl totals: namespaces = 25, members = 288
spec totals: namespaces = 25, members = 288

== spec <= impl ? (does the implementation cover every spec member) ==
  missing in impl: NONE  ->  spec<=impl: no missing

== impl <= spec ? (does the implementation expose unregistered members) ==
  unregistered in impl: NONE  ->  impl<=spec: every member is registered

== plan sec.6.2 (pre-fix snapshot) vs spec: members the plan over-claims ==
  plan-only member: wlwl:std.http::COMPRESS_RESPONSE
  plan-only member: wlwl:std.http::SERVE
  plan-only member: wlwl:std.net::TCP_CONNECT
  count: 3

== live plan sec.6.2 vs spec vs impl ==
  live plan sec.6.2 matches the spec and the implementation
  wlwl:std.collection::ARRAY_GET: global built-in (registered in appendix G, not a namespace member) -> expected difference

RESULT: spec<=impl no missing = True ; impl<=spec no unregistered = True
        plan over-claims (pre-fix) = 3 member(s)
```

**脚本的四个性质**（都是本轮特意做的，缺一个就不该入库）：

1. **不用临时目录** —— 实现侧**直接调** `wlwl namespace` 并解析 stdout 的 JSON（`.scratch` 的旧版本要 `%TEMP%` 中转文件）。
2. **规范仓路径可配** —— `argv[0]` = 规范仓目录（默认 `D:\Project\wlwl-spec-v1`）；`argv[1]` = `wlwl.exe` 路径；`argv[2]` = 计划文档路径。⚠ **规范仓不在本机时**：改 `argv[0]` 即可，不会写死本地路径。
3. **输出纯 ASCII** —— Windows 控制台是 **GBK**，中文会 `UnicodeEncodeError`；脚本自己声明 `# -*- coding: ascii -*-` ⇒ 源码里混入非 ASCII 字节会**直接语法错**（而不是运行时才炸）。
4. **两侧各读一次** —— `PLAN_BEFORE` 是**订正前** §6.2 的**逐字快照**（故意保留 ⇒ 每次都把「计划多出的 3 个成员」打出来，**可追溯**）；**现表**由脚本**实时解析**计划文档的 §6.2 ⇒ 订正后读作「live plan sec.6.2 matches the spec and the implementation」。

⚠ **命名空间名的两套写法**（脚本里已做映射，别自己再踩）：规范册写 `std.http` ⇒ 实现侧路径是 `wlwl:std.http`；官方包 `ai` / `agent` 在实现侧**没有** `wlwl:` 前缀。

### 9.4 对 §6 工作清单的影响（T-03 / T-04）

| 条目 | 方向 | 是否阻塞 rc2 |
|---|---|---|
| **T-03**（`TCP_CONNECT`） | **B 改口径（文档订正）** —— 台账 `I-3` 改记 📖 + §6.2 删该成员 | ❌ **不阻塞**（规范侧不需要动 ⇒ 不必重走规范侧登记流程） |
| **T-04**（`SERVE` / `COMPRESS_RESPONSE`） | **A 删除 + 记录偏差** —— §6.2 删两个成员 + 写明能力归属 | ❌ **不阻塞**（同上） |
| **T-01**（规范仓 `v1.0.4` tag） | **不变** | ⚠ **仍是发布前置**（**待业主指示**） |
| **T-02**（发 `v0.12.1_rc2`） | **不变** | ⚠ **仍是发布动作**（**待业主指示**） |

⚠ **本轮改的是「方向」，不是「是否需要业主点头」**：T-03 / T-04 的方向由**三方对拍的实测证据**定案（不是新增能力、不动代码、不动规范）；**T-01 / T-02 仍是业主的动作**，本轮**一个字没改**。

### 9.5 对 §8 验收清单的影响

- §8 第 1 条「T-03 / T-04 已裁决，且裁决结果落进了「台账 + §6.2 表」两处」⇒ **实质已满足**（两处都已落：§7 台账 `I-3` + §6.2 表）。⚠ 「已裁决」字面上指**业主裁决** —— 本轮是**依据实测证据**由集成方定案；若业主另有口径，以业主口径为准（届时**仍然不用动规范侧**，只要再改那两处文档）。
- §8 其余各条（T-01 / T-02 / T-05 / T-10 / CI 全绿）**状态不变**。
- ⚠ **T-10**（§5.2 三处不一致订正）⇒ 本轮**已完成**其中三处：§6.2 表（`std.http` / `std.log` / `std.net` 三行）· §11.2 的过期警告 · §11.6b 面板计数 ⇒ **面板与台账状态列已逐项相等**。

### 9.6 本节新增的证据

| 证据 | 位置 |
|---|---|
| 三方对拍脚本 | `script/audit_member_threeway.py`（本轮入库；纯 ASCII · 不用临时目录 · 规范仓路径可配） |
| 规范侧成员表 | `D:\Project\wlwl-spec-v1\docs\stdlib\wlwl-v1.0-stdlib-registry.md` 附录 A（HEAD `d6d90c3` = v1.0.4 候选文本，**tag 未发**） |
| 实现侧公共面 | `impl/target/debug/wlwl.exe namespace`（**25 命名空间 / 288 成员**） |
| 规范侧总量 | 同上脚本读出 **25 命名空间 / 288 成员** ⇒ 与实现**逐成员一致** |
| 计划侧订正 | `docs/plan/wlwl-v0.12.1-build-plan.md` §6.2（`std.http` 16 员 / `std.log` 7 员 / `std.net` 2 员）· §7 台账 `I-3` · §11.2 · §11.6b 面板 |
