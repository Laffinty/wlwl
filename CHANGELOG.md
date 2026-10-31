## [v0.12.1 收口 · 第二批 · 修补] — 2026-10-31（上一笔自己引入的两条 CI 红：非 Windows 侧 clippy 未使用变量 + 探针在 runner 上挂死 2.4 小时）

⚠ **本段是**我自己**在上一笔（`7b6321d`）里造成的两条 CI 红**，如实登记。
⇒ 上一笔的**功能没有回退**（TLS staging、打包排除、锁写修复、夹具修复都还在），
红的是**新加的探针文件**。

### ① run #337 的 ubuntu ✗ / macOS ✗：`cargo clippy -D warnings` 红（非 Windows 侧未使用变量）

`handle_dump_probe.rs` 里基线快照分两臂：

```rust
#[cfg(windows)]      let base_snapshot = scan_handles();
#[cfg(not(windows))] let base_snapshot: Vec<...> = Vec::new();
```

⇒ 非 Windows 侧那个 `Vec::new()` **只被 `#[cfg(windows)]` 块用到** ⇒ 在 ubuntu /
macOS 上它是**未使用变量** ⇒ `-D warnings` ⇒ **直接红**。
⚠ **为什么本机没发现**：我在 **Windows** 上跑 `clippy` ⇒ 走的是 `#[cfg(windows)]` 那一臂
⇒ 另一臂**根本不参与编译** ⇒ 本机全绿而 CI 红。⚠ **这正是「只在另一个平台编译的分支，
本机 lint 永远看不到」** 这个形态。

**修法**：`let _ = &base_snapshot;`（显式吞掉）＋ 注释写明原因。
**验法（本机能做的那一半）**：把文件里的 `#[cfg(windows)]` / `#[cfg(not(windows))]`
**互换**成一份临时的 `*_linux_sim.rs` ⇒ 用 `cargo clippy -D warnings` 编那一份
⇒ **`unused variable` 消失**（只剩模拟本身造成的 `non_minimal_cfg` 噪音）⇒ 删掉临时件。

### ② run #337 的 windows job：探针步骤**挂死 2.4 小时**（`NtQueryObject` 死锁）

**症状**：`Handle-leak naming probe` 步骤**从 `02:06` 一直 `in_progress` 到 `04:30`
仍未结束**（其余步骤都已完成）⇒ 白烧两个多小时 runner。

**根因（公开资料坐实）**：第一版探针用 `NtQueryObject(ObjectTypeInformation)` 取
**对象类型名**（`File` / `Event` / `Key` …）。⚠ 该 API 在**同步管道句柄 + 有挂起读操作**
时**会永久挂死**，而且**那个线程杀不掉**（强杀可能蓝屏）；连「先判断它是不是管道」的
替代尝试（`GetFileType` 等）**同样会死锁**。
⇒ GitHub runner 上有大量这类管道句柄（runner 与它派的子进程通信）⇒
⇒ **任何「逐个句柄查类型名」的写法在 CI 上都是定时炸弹**。
⚠ **为什么本机没发现**：本机扫 212 个句柄、**7.7 s 全读出类型名**（那时它还「能用」）⇒
本机的句柄构成里没有那种「同步管道 + 挂起读」的组合。

**修法（两道保险）**：
1. **探针不再调 `NtQueryObject`** ⇒ 只用两个安全 API：`DuplicateHandle`（判有效）与
   `GetFinalPathNameByHandleW`（取路径）。⚠ **代价**：拿不到类型名 ⇒ 但「多出来的是
   **文件**（带路径）还是**非文件内核对象**」这个二分对定位已经足够决定性。
2. **扫描跑进可放弃的线程**（`thread::spawn` + `recv_timeout(120 s)`）⇒ 万一再踩到什么
   会阻塞的 API，探针只是**拿不到证据**，不会挂死。
3. **`ci.yml` 那一步加 `timeout-minutes: 10`** ⇒ **硬上限** ⇒ 无论以后谁再动它，
   **都不可能**再烧几小时。

**顺带的可用性改进（本轮加的）**：探针把结论**压成一行发 `::warning` 注解** ——
因为 GitHub 的**作业日志需要登录**才能读（`GET /actions/jobs/<id>/logs` = 403、
artifact = 401），而**注解是匿名可读的**（`check-runs/<id>/annotations` 无需 token）
⇒ 一次 API 调用就能拿到「命中没有 / 多出来的句柄是什么」。

**实测（本机，改后）**：探针 **8.1 s** 跑完、基线 **212 个**有效句柄（**8 个带路径**）、
20000 次 eval **未命中**、注解正常输出
（`NO-HIT evals=20000 baseline=72 transient=0 snapshot=212 with_path=8`）。

### ③ 一条与该事故无关的好消息：上一笔的两条 flaky 修复**在 CI 上验住了**

**run #336（head `372b790`）4 个 job 全绿** —— `test (ubuntu-latest)` ✓ ·
`test (macos-latest)` ✓ · `test (windows-latest)` ✓ · `smoke run examples` ✓。
⇒ ⚠ **ubuntu 那两条红（锁写并发竞争 / chunked 夹具 RST）在 run #336 上都没有再出现**。
⚠ 但**不能据此判「已彻底修好」**：这两条本来就是**概率性**缺陷，单轮绿只是**一个**
读数（锁写那条的病根已由**确定性**守卫钉住；RST 那条由 WSL2 上 500 轮的 A/B 对照钉住）。

### ④ 修补批的结果：**run #338（head `fbf3189`）4 个 job 全绿**

`test (ubuntu-latest)` ✓ · `test (macos-latest)` ✓ · `test (windows-latest)` ✓ ·
`smoke run examples` ✓ ⇒ ① 的 clippy 红消失（ubuntu / macOS 的
`cargo clippy -D warnings` 都 **success**）；② 的挂死消失
（探针步骤 **`04:39:19 → 04:43:30` ＝ 4 分 11 秒**，不再几小时）。

### ⑤ ⚠⚠ **句柄泄漏的关键新证据：它在 runner 上不是「1/20000」，而是「第 0 次就命中」**

探针注解（**匿名可读**，`check-runs/124442753863/annotations`）原文：

```
LEAK-HIT eval=0 count=87 baseline=86 settled=87 :: SCAN-TIMEOUT (guarded scan did not finish in 120s)
```

两条都极重要：

1. **`eval=0` ⇒ 泄漏在 runner 上第 0 次 eval 就出现了**（`86 → 87`，且**落定后仍是 87**
   ⇒ 不是噪声）。⇒ 本机 30000 次 eval 零命中、runner 上第 0 次就命中 ⇒
   **这是一个「按环境切换」的缺陷，不是概率问题** ⇒ 之前把它当「1/600 的偶发」是本轮
   该纠正的判断。
2. **`SCAN-TIMEOUT` ⇒ 连「只用 `DuplicateHandle` + `GetFinalPathNameByHandleW`」的
   扫描也扫不完（120 s）** ⇒ `GetFinalPathNameByHandleW` 在 runner 那类句柄上**同样
   阻塞**（它内部查 `FileNameInformation`，与 `NtQueryObject` 同一类阻塞面）
   ⇒ **在 runner 上「逐个句柄查询」这条路根本走不通**，不是换个 API 就能绕过的。

⇒ **探针改成「变体矩阵」（版 3）**：只依赖 `GetProcessHandleCount` 这一个绝对安全的
调用 ⇒ 拿不到「那个句柄是什么」，但能拿到「**哪一段操作**让它多出来一个」——
而后者才直接指向代码。**六个变体**：`import_only` · `read_missing_print`（ERR 逃逸到
顶层）· `read_missing_let`（ERR 被丢弃）· `read_existing`（成功路径）·
`write_bad_dir`（另一条 IO 失败路径）· `reuse_evaluator`（复用同一个 `Evaluator`
⇒ 分「构造/析构」与「每次 eval」）。每个变体自带**落定基线** ⇒ 互不污染。

**本机实测（版 3）**：`1000 次 eval × 6 变体 ＝ 2.3 s`，六个 `d=0`，
`dup_only=ok(212)` ⇒ 本机不触发（与既往一致），但枚举本身在本机可用
（⇒ 阻塞只发生在 runner 的句柄构成上）。

---

## [v0.12.1 收口 · 第二批] — 2026-10-31（CI run #334/#335 两条 flaky 已修 · TLS 扩展产物随包发布 · CI 专用句柄点名探针）

⚠ **本段是 `v0.12.1_rc1` tag 之后**的工作 ⇒ 它**不在**那个 RC 包里（tag ＝ 快照）
⇒ **下一个发布（正式版 `v0.12.1` 或更新的 RC）**才会有它。

### 事件

- ✅ **CI run #334 / #335 的两条 flaky 已定性并修掉**（一条真实现缺陷、一条 Linux-only
  夹具缺陷；根因、修法与实测读数见下方两段）。
- ✅ **TLS 扩展产物随发行包发布**（计划 §0 第 4 件）—— 此前 staging 只放编译器二进制
  ⇒ **实测 run #44 的 5 个资产里没有任何一个是 TLS 动态库**；现已补上。
- ⚠ **加了一条 CI 专用临时探针**，把 Windows 那条「句柄漂移」的**持有者点名**。

### ① run #335 ubuntu：锁写并发竞争（真实现缺陷，已修）

`wlwl_toml::lock::write` 的临时文件名**固定** ⇒ 并发写同一个 `wlwl.lock` 时它们写
**同一个** `.tmp` ⇒ 先完成者把 `.tmp` rename 走 ⇒ 后完成者 `rename` 拿到 `ENOENT`
⇒ 那行警告混进示例的 stdout+stderr ⇒ 断言红。**修法**：临时名唯一化
`<name>.tmp.<pid>.<seq>`。**实测 8 线程 × 40 轮 ＝ 320 次并发写 ⇒ 320/320 成功、
0 次 ENOENT**。

⚠ **顺带订正两处「此前登记过、但实测不成立」的说法**：① 原先那条反向守卫是
「跑 320 次并发写、断言零 ENOENT」—— ⚠ **实测守不住**（把临时名改回固定名，它在
空闲机器上照样全绿 ⇒ 命中与否取决于线程调度）⇒ 已换成**确定性**守卫
`each_concurrent_write_uses_its_own_temp_name`（并发写时每次用的临时路径必须
两两不同 ⇒ 固定名必红，已实测验红/验绿；副作用是更快：0.44 s → 0.02 s）；
② 原实现里「rename 前先删正式路径」**前提是错的**（Rust 的 `fs::rename` 用
`MoveFileExW` ＋ `MOVEFILE_REPLACE_EXISTING` ⇒ **能覆盖**）⇒ 已去掉（那一步还会
造出「lock 文件在删除与 rename 之间**不存在**」的窗口）。

### ② run #334 ubuntu：chunked 夹具 `close()` 发 RST（Linux-only flaky，已修）

夹具**不读**客户端请求 ⇒ 接收缓冲区留着未读数据 ⇒ **Linux close() 发 RST 而非 FIN**
⇒ 客户端丢尾块 ⇒ 拿到 `ERR` 值。**只改夹具**（写响应前把请求读到头块结束）⇒
**实现侧零改动**。**实测（WSL2 Ubuntu A/B）**：修前 500 轮 2 次 RST ⇒ 修后零 RST。

### ③ TLS 扩展产物随包发布（计划 §0 第 4 件，已补）

- `release.yml` 新增两步：**构建** TLS 扩展 cdylib（**同一个 `--target`** ⇒ 不混架构）
  ＋ **装进 `staging/wlwl-tls/`**（动态库 ＋ `ABI_VERSION` ＋ `SHA256SUMS`）。
  ⚠ ABI 版本从**单一真相** `impl/crates/wlwl-ext/src/abi.rs` 里**读出来**（不在
  workflow 里写死 ⇒ 宿主升 ABI 时不会悄悄过期）；⚠ **产物缺失就响亮失败**（不静默
  发一个「没有 TLS 动态库」的包 —— 那正是 run #44 的状态）。
- `script/package_source.py` 的排除清单加 `impl/ext/wlwl-tls-ext/target` ——
  ⚠ 否则 release.yml 先编扩展、再打源码包时，那几百 MB 的 rustls / ring 构建产物
  会被打进 `wlwl-src.tar.gz`。
- `docs/site/install.md` 的包内容清单补上 `wlwl-tls/`（含加载方式 `--ext`）。

**实测**：本地编出 `wlwl_tls.dll`（**2078208 B**）⇒ staging 预演 ⇒ `ABI_VERSION = 1`
＋ `SHA256SUMS`（`749f6ef4e6e56f8839978ef7b671abaf8e71e7e31c1c7ea66fe530f4c527b98d`）
与 PowerShell `Get-FileHash` **独立核对一致**；打包脚本跑通 ⇒ **1236 个文件**、
两次运行**树哈希一致**（`b5c0f204…`）、tarball 里 `ext/target` **0 条目**而扩展
**源码 30 条**（含它自己的 `Cargo.lock`）。

⚠ **本条尚未被真正的 release run 验过** —— `release.yml` 只在**打 tag** 时跑 ⇒
五个平台的交叉链接（尤其 aarch64 的 linker 环境变量）**要等下一次发布**才有读数。

### ④ CI 专用句柄点名探针（**临时**，拿到结论即删）

Windows 那条 `resource_release_contract::a_failing_file_read_leaves_no_handle_behind`
（`delta = 1`）已定性为**真泄漏**（读数不回落），但 ⚠ **本机复现不出**（本机 5 个
变体各 6000 次 eval ⇒ 零命中）⇒ 决定性的一步只能在 **CI 的 windows runner** 上做。

新增 `impl/crates/wlwl-eval/tests/handle_dump_probe.rs` ＋ `ci.yml` 里一个
**只在 windows 跑**（`if: always()` ⇒ 主测试红时也要取证）的步骤：复刻那条用例的
形状，**每次 eval 后**读句柄数，一旦偏离且**落定后仍偏离** ⇒ 把**句柄逐个点名**
（类型名 ＋ 文件路径）⇒ 日志里直接看得见多出来的是什么、指向哪里。

⚠ **为什么不用 `NtQuerySystemInformation`**：实测 `SystemExtendedHandleInformation`
给的是**全系统**句柄表（本机 9 万多条）且**没有 PID 字段**；`SystemHandleInformation`
实测三种 stride 都**匹配不到**本进程 PID ⇒ 都用不得 ⇒ 改走**用户态暴力扫描**：
`DuplicateHandle` 试 `0..4096` 的句柄值（成功 ⇔ 有效）＋ `NtQueryObject` 取类型名
＋ `GetFinalPathNameByHandleW` 取路径 ⇒ 与基线快照**做差**。

**实测**（本机）：基线快照 **212 个**可识别句柄（类型分布 `File` 76 · `Event` 28 ·
`WaitCompletionPacket` 24 · `IRTimer` 16 · `Key` 16 …，其中 **8 个带路径** ⇒ 路径
读取可用）；20000 次 eval **未命中**（与「本机复现不出」一致）。
⚠ 它**不会让 CI 红**（命中也只是打印）⇒ 真正的守卫仍是 `resource_release_contract`
那条断言；⚠ **拿到结论后这一步与那个探针文件一起删**。

---

## [v0.12.1_rc1] — 2026-10-31（预览版 · tag 已发 · Release run #44 全绿 · Release 已建成）

**本段记录的是 `v0.12.1_rc1` 那个快照** —— RC 与正式版**在 RC 发布时**是**同一份代码**
（`impl/Cargo.toml` 的 `version = "0.12.1"` 不因 RC 而改）。

⚠ **这是 RC,不是正式版** —— 本快照的二进制报的是 `wlwl 0.12.1`，而发布 tag 是
`v0.12.1_rc1` ⇒ GitHub 上标记为 **pre-release**，不会被工具链当成 production ready
抓走。正式版 `v0.12.1` 的 tag 尚未发。

⚠ **版本守卫已实测过**：`.github/workflows/release.yml` 的 `version guard (tag == Cargo
version)` job **结论 success**（run #44 里 3 个实际步骤全 success：checkout → 比对 →
收尾）。它比的是**基版本** —— `v0.12.1_rc1` ⇒ 基版本 `v0.12.1` ⇒ 与 `Cargo.toml`
的 `0.12.1` **逐字相等** ⇒ 过。
（下划线**不是合法 semver** ⇒ `Cargo.toml` 里写 `0.12.1_rc1` 会让**全仓每一条 cargo
命令都崩** ⇒ RC 只能在 **tag 名**上带后缀。）

### 事件

- ✅ **tag 已发**：`v0.12.1_rc1`（**annotated tag**，对象 `6c5ae26c`）⇒ 剥壳指向
  **`e1f65f8`**（＝ 本 tag 所在快照）⇒ 与 `origin/main` 同一提交。
- ✅ **打 tag 前的主 CI 已绿**：**run #333**（head `e1f65f8`，= 本 tag 所在快照）
  **结论 success**；另有 #332（`e3dacae`）**绿** · #331（`fe2befc`）**绿**
  （三平台 + smoke **4 个 job 全绿**）。
  ⚠ **发版纪律**：「先修到远端 CI 全绿，再打 tag」—— tag 一推就对外可见且撤不回，
  且 **CI 绿 ≠ release workflow 绿**（release 有自己的门禁，不重跑主 CI 的测试）。
- ✅ **Release workflow 已跑完 —— run #44 十个 job 全绿**：**run #44**
  （`head_branch = v0.12.1_rc1`，`head_sha = e1f65f8`）**结论 = success**，
  **10 个 job 逐个 `conclusion: success`** ——
  `version guard (tag == Cargo version)` ✓ · `build (x86_64-unknown-linux-gnu)` ✓ ·
  `build (x86_64-apple-darwin)` ✓ · `build (aarch64-apple-darwin)` ✓ ·
  `build (x86_64-pc-windows-msvc)` ✓ · `build (aarch64-unknown-linux-gnu)` ✓ ·
  `generate SBOM (CycloneDX)` ✓ · `spec conformance suite` ✓ ·
  `cosign keyless sign binaries + SBOM` ✓ · `github release` ✓
  ⇒ **没有红项**。（`spec conformance suite` 这一格是 release workflow **自己**的
  门禁 `cargo test --locked --test conformance -p wlwl-cli`，**不是**主 CI。）
- ✅ **Release 真的建成了**（不是「job 绿就当发布成功」）：`GET
  /releases/tags/v0.12.1_rc1` ⇒ **id `429579021`**、`tag_name = v0.12.1_rc1`、
  **`draft: false`**、**`prerelease: true`**、`published_at 2026-10-30T23:21:34Z`
  ⇒ **5 个资产齐全**（4 个 `.tar.xz` + 1 个 `.zip`，每个平台一个）：
  `wlwl-v0.12.1_rc1-aarch64-apple-darwin.tar.xz`（5360004 B）·
  `wlwl-v0.12.1_rc1-aarch64-unknown-linux-gnu.tar.xz`（5450008 B）·
  `wlwl-v0.12.1_rc1-x86_64-apple-darwin.tar.xz`（5650776 B）·
  `wlwl-v0.12.1_rc1-x86_64-unknown-linux-gnu.tar.xz`（5715584 B）·
  `wlwl-v0.12.1_rc1-x86_64-pc-windows-msvc.zip`（6612854 B），
  **每个都带 `sha256:` 摘要**。
  ⚠ **`prerelease: true` 是对的设计**（RC 按 tag 名里的 `_rc` 判 pre-release）
  ⇒ 不会被工具链当成 production ready 抓走；**正式版 `v0.12.1` 的 tag 尚未发**。
  ⚠ **资产里没有 TLS 动态库**（5 个包全是主二进制 + 源码树）⇒ 见下「两处已知未完」①。
- ⚠ **规范侧本版**：`wlwl-spec-v1` 的 **v1.0.4 文本**已进本仓
  （`docs/wlwl-spec-reference.md` ＝ 从规范仓 v1.0.4 重建的合并册，**头部已对齐
  v1.0.4 / 25 个命名空间 / 288 个成员 / 30 处守卫**），但**规范侧 v1.0.4 的 tag 尚未发**
  ⇒ 本仓里的规范文本是**候选文本**，不是已发布的规范版本。
  ⚠ **发布包不含该册**（规范文本不随包分发）⇒ 取规范文本请到 `Laffinty/wlwl-spec-v1`。

### 本批实测数字

- 公共面 **288 members / 25 namespaces**（23 个 `std.*` = 278 成员 + 官方包
  `ai` 5 + `agent` 5）—— `wlwl namespace` 实测
- probe 用例 **196**（`EXPECTED_CASE_COUNT`）
- `wlwl-std` **385/0** · `wlwl-eval --lib` **920/0** · `wlwl-cli` **30 个 target
  全绿 0 failed**（含 `probe` 3/3）· 其余 9 个 crate 全绿 · `wlwl-tls-ext` **17/0**
- `cargo fmt --check` **0 diff** · `cargo clippy -D warnings` **0 error** ·
  `cargo doc -D warnings` **0 error**
- 一致性册 §5.4 双向门禁 **25 行全部一致** · `check-appendix-a` **exit 0**
  （合并册附录 A 与规范仓注册镜像册**逐字节相同**，4495 bytes）
- 合并册重建**幂等**（重跑生成器 ⇒ diff 恒为头部那 3 行，603310 bytes 不变）

### 本批新东西（相对 v0.12.0）

- **公共面 231 → 288**（净增 **57** 个；规范侧 D-21 登记 58 项，含 1 个全局内建
  `ARRAY_GET`，故实现面读数为 288）—— 新增命名空间 `std.http` 16 · `std.log` 7 ·
  `std.sync` 8 · `std.signal` 2，加上既有命名空间的补齐
- **一道新的机器门禁**：§5.4 状态列 ↔ 契约表的双向核对
- **TLS 扩展**（`wlwl-tls` cdylib，宿主 `--ext` 显式授权加载）—— 语言面**不提供**
  `LOAD` ⇒ 脚本不能触发代码加载
- **中间件原语**：`RATE_LIMITER` / `RULESET_LOAD` / `CLIENT_IP` + `std.sync` 原子容器

⚠ 其中 **D-18 是 breaking**：传输层的等待由「阻塞冻结 `SCOPE`」改为「宿主挂起」；
`timeout_ms` ＝ 那一次等待的截止时间，`0` ＝ 无截止时间；`TCP_SET_TIMEOUT` 与
调用点预算**先到者为准**。

### ⚠ 本批的未完与订正（① **不是**本 tag 引入的，如实登记；② 本轮已修）

① **TLS 扩展产物未随发行包发布** —— `release.yml` 的 staging 步骤**没有**把
`wlwl-tls` 的 `.dll` / `.so` / `.dylib` 装进包（计划 §0.4 第 4 件）。
⇒ **本 RC 的发布包里没有 TLS 动态库**；要用得自己按 `--ext` 加载自建的。
⇒ ⚠ **该件已在 `main` 上补齐**（见下方「TLS 扩展产物随包发布」段）⇒ **下一个
发布（正式版 `v0.12.1` 或更新的 RC）的包里就有它了**；⚠ 但 **`v0.12.1_rc1`
这个已发布的 RC 包本身仍然没有**（tag 打在那之前 ⇒ 不会追溯重建）。
⚠ **这一条已由真实发布包核实**（不是只看 workflow 文本）：run #44 建成的 release
**5 个资产逐一列出**（4 个 `.tar.xz` + 1 个 `.zip`）⇒ **没有任何一个是 TLS 动态库**
⇒ 与 staging 步骤的读数一致。
② **`docs/site/` 下三处版本残留** —— ✅ **已订正**（业主 2026-10-31 裁决：改指
**v0.12.1_rc1**）：`install.md` 的 4 个下载 URL 与 `contributing.md` 的
「`main` is …」一句**均已改指 `v0.12.1_rc1`**（**不是** v0.12.0 —— 业主选的是
与当前工作树版本一致的那个）。⚠ **顺带订正了同批查出的另外 8 处失实**（全部
实测验证，非推断）：
- `install.md` 声称包里有「`sbom.json` + `<archive>.bundle`」⇒ **实测发布包里
  都没有**（真实内容：`wlwl` · `LICENSE` · `README.md` · `CHANGELOG.md` ·
  `examples/` · `stdlib/` · `wlwl-skill/` · `wlwl-src.tar.gz` · `SOURCE_HASH`，
  共 61 个条目）⇒ 清单改为真实内容，并注明 `.bundle`/`.sig` **不是 release 资产**
- `install.md`「Requires Rust ≥ 1.75」⇒ 实测 `rust-version = "1.85"`；且
  「toolchain pinning is planned for v0.5」早已过时 ⇒ 工具链**已钉死 1.99.0**
- `contributing.md`「≈30 KLOC across 9 crates」⇒ 实测 **≈117 KLOC（src 口径，
  91 个 .rs）· 12 crates**
- `contributing.md`「the 1100+ unit + integration tests」⇒ 数量已随版本漂移，
  **改为不给具体数字**（避免再次过期）
- `contributing.md`「10 files in `impl/examples/`」⇒ 实测 **11**
- `contributing.md`「meta-test fails CI if the union drops below 58 codes」⇒
  实测判据是 **`total >= 59`**
- `contributing.md`「`[docs/adr/ ADR absent]` and `deviations.md`」⇒
  `docs/adr/` 实有 **35 份 ADR**；`deviations.md` **不存在**
- `contributing.md`「The 5 baseline measurements」⇒ `benches/baseline.txt`
  实有 **1321 行**测量记录 ⇒ 改为「the baseline measurements」
⇒ **对照项已清零**（`docs/site/` 下已无 v0.4.0 残留；`changelog.md` 里的 v0.4.0
是**历史记录**，正确，不动）。

### ⚠ CI run #334 的红项：**已定性并修掉**（flaky 的根因，不是回归）

⚠ **run #334（head `15a150e`，纯文档改动）的 ubuntu job 红在 step 15 `cargo test`**—— ⚠ **CI 自己标了**：`FLAKY (cargo test)` ——「pass 1 FAILED, pass 2 PASSED → flaky, **NOT auto-cleared**」⇒ 本仓策略是「重跑仍判红」⇒ **整条 run 判 failure**。

⇒ **上一轮登记过的「未能定性的 flake」这次有名字了**：`http_w20_contract::the_chunk_boundaries_do_not_have_to_match_the_tcp_segments`（panic 在 `http_w20_contract.rs:252` 的 `status_of` ⇒ 客户端拿到的是 **`ERR` 值**）。

**根因（Linux 上实测坐实）**：夹具 `serve_pieces` **不读**客户端发来的请求 ⇒ socket 的**接收缓冲区里留着未读数据** ⇒ 夹具写完响应直接 `close()` ⇒ **Linux 在「接收缓冲区有未读数据」时 close() 发 `RST` 而不是 `FIN`** ⇒ 客户端正在读的那块数据被**丢弃** ⇒ 客户端报 `ECONNRESET`（⇒ `ERR` 值）。
⚠ **Windows 上不发 RST** ⇒ **本地怎么跑都不红**（本机实测：单跑 **80 轮** + **12 路并发 × 60 轮**全绿）⇒ **只在 Linux CI 上暴露**。

**实测证据（WSL2 Ubuntu，A/B 对照，纯 Python 复刻同一时序）**：修前（不读请求就 close）**500 轮 2 次 RST** · 修后（先读完请求再 close）**500 轮零 RST**。

**修法**：**只改夹具**（写响应前把请求读到头块结束）⇒ `close()` 发 `FIN`。⚠ **实现侧零改动** —— chunked 解码器**没有缺陷**，它只是**正确地**把对端的 `ECONNRESET` 报成了 `ERR` 值。⚠ **同类形态已全仓排查**：只有 `serve_pieces` 这一处。

### ⚠ CI run #335 的红项：**两条独立的 flaky，各有各的成因**（ubuntu ✗ · windows ✗ · macOS ✓）

⚠ **run #335（head `b468bac`，纯文档改动）结论 = failure**：**ubuntu ✗ · windows ✗ · macOS ✓** ⇒ 两条**互相独立**的红项。⚠ **两条都不是文档改动引入的**（run #334 / #335 的提交都是纯文档）。

**① ubuntu：`skill_examples::h1_if_else_third_arg_holds_a_multi_statement_group`（真实现缺陷，已修）**

**症状**：期望 5 行输出，实际多一行 —— `warning: failed to write ./wlwl.lock: wlwl.lock I/O error: No such file or directory (os error 2)`。

**根因**：`wlwl_toml::lock::write` 的**临时文件名是固定的** ⇒ **并发**写同一个 `wlwl.lock` 时它们写的是**同一个** `.tmp` ⇒ 先完成的那个把 `.tmp` rename 走了 ⇒ 后完成的那个 `fs::rename` 时 `.tmp` **已经不在** ⇒ `ENOENT`。⇒ 后果不止「一条警告」：`wlwl run` 的示例把 **stdout 与 stderr 拼在一起**比对 ⇒ 多出的那行警告让「逐行钉住输出」的断言**红**。

**修法（实现侧）**：临时名唯一化 ⇒ `<name>.tmp.<pid>.<seq>`（**pid** ⇒ 跨进程不撞；**seq** ⇒ 同一进程内的并发写不撞）。

⚠ **实测订正一：反向守卫换成了「确定性」的那一条**。先前写的守卫是「跑 320 次并发写、断言零 `ENOENT`」—— ⚠ **实测：把临时名改回固定名，这条守卫在空闲机器上照样全绿** ⇒ **命中与否完全取决于线程调度** ⇒ 那不是能守住回归的守卫（它「看着在管事、实际管不住」）。⇒ 改为 `each_concurrent_write_uses_its_own_temp_name`：**并发写时每次用的临时路径必须两两不同** ⇒ 临时名固定 ⇒ 8 个线程拿到**同一个**路径 ⇒ **必红**（已实测验红 / 验绿）。⚠ 副作用是这条守卫**更快**：整个 `wlwl-toml` 用例从 **0.44 s** 降到 **0.02 s**。

⚠ **实测订正二：去掉了「rename 前先删正式路径」那一步**。先前的理由是「Windows 的 `rename` 不能覆盖已存在的文件」—— ⚠ **这个前提是错的**：Rust 的 `fs::rename` 用的是 `MoveFileExW` + `MOVEFILE_REPLACE_EXISTING` ⇒ **能覆盖**（探针读数：目标原内容 `A`、源内容 `B` ⇒ rename 成功后目标是 `B`）。⇒ 那一步不但多余，还会**造出一个窗口**：正式 lock 在「删除」与「rename」之间**不存在** ⇒ 并发读者可能读到「lock 文件不见了」。⇒ **已去掉**。⚠ 去掉后重测：**8 线程 × 40 轮 = 320 次并发写 ⇒ 320/320 成功、0 次 `ENOENT`、0 次其它错误**（299 ms）。

**② windows：`resource_release_contract::a_failing_file_read_leaves_no_handle_behind`（真泄漏，持有者尚未点名）**

**症状**：`delta = 1`（多了一个句柄）。⚠ **本机（Windows）复现不出**：单跑 60 轮全绿 ⇒ 只在 CI 的 windows-latest 上偶发。

**已定性（探针分解）**：漂在 **`eval()` 内部**（`parse` / `new Evaluator` / `drop(ev)` / 什么都不做都不漂）；**纯 IO 读失败 0/600 · 只 `IMPORT` 0/600 · `IMPORT` + 读失败 1/600**；**不回落**（settled 读数仍高于 before）⇒ **不是测量噪声，是真泄漏**。⇒ 已排除 `scratch_dir()` / 纯 IO / 只 `IMPORT` ⇒ 锁定「`IMPORT` + 读失败」这个组合。

⚠ **本轮新增的收敛**：被测的 `READ_FILE` 走 `std::fs::read_to_string` ⇒ **读失败时连句柄都不会建** ⇒ **泄漏点不在「读」本身** ⇒ 候选收敛到两条：**「ERR 值被丢弃」的错误路径**（`E0103`）与**导入/模块加载**。⇒ 已用**变体探针**切开：`V1` `IMPORT`+ERR 逃逸 · `V2` `IMPORT`+ERR 被 `LET` 丢弃 · `V3` 不碰文件的纯 `E0103` · `V4` 读成功 · `V5` 只 `IMPORT`（每个变体 **6000 次 eval**、**每次 eval 后立刻读句柄数**、偏离即等它落定复核）。⚠ **读数：5 个变体各 6000 次 eval ⇒ 零命中、零瞬时偏离**（句柄数恒为 72）⇒ **本机复现不出** ⇒ 该泄漏是 **CI windows runner 特有**（本机单跑 60 轮全绿、CI 上偶发 `delta = 1`）。

⚠ **探针顺带查实的一条事实**（决定了下一步该怎么查）：ERR 的两条路径**分得很清** —— `PRINT(READ_FILE(missing))` ⇒ **`E0102`「unhandled ERR escaped to top level」**；而 `LET(e, READ_FILE(missing))` ⇒ **`E0103`「unhandled ERR was discarded」**（**ERR 在 `LET` 处就被判丢弃** ⇒ 后面的 `IS_ERR(e)` 根本没机会消费它）⇒ 「ERR 被消费」这条路在当前实现下**走不到** ⇒ 变体探针的 `V2` 实际测的仍是 `E0103` 路径。

⚠ **本轮不改那条断言**（改了就没有守卫了）；⚠ **本轮也不动 `release.yml` 的 `FLAKY_EXIT_CODE`**（那是**策略裁决**，须业主明确指示）。⚠ **下一步的形状（待业主拍板）**：既然本机复现不出，**决定性的一步是「在 CI 上把泄漏的持有者点名」** —— 即加一条**只在 CI 跑**的临时探针：当 `delta != 0` 时用 `NtQuerySystemInformation(SystemExtendedHandleInformation)` **把本进程的句柄表连类型带路径 dump 出来** ⇒ CI 日志里直接写出那个多出来的句柄是什么（文件？事件？子进程？）、指向哪里。

完整变更清单见本文件「[v0.12.1 收口 · 文档同步 + 探针归档]」与「[v0.12.1 收口]」两段；
构建方案与逐项读数见仓内 `docs/plan/wlwl-v0.12.1-build-plan.md`（随源码 tarball 走）。

---

## [v0.12.1 收口 · 文档同步 + 探针归档] — 2026-10-30（CI run #331 全绿 · README 四处误述订正 · 规范参考载体头部对齐 v1.0.4 · 9 个探针整目录归档为单 md）

**本段 = 收口批之后的**文档同步 + 归档**（**零代码改动**）。计划与坐标见 `docs/plan/wlwl-v0.12.1-build-plan.md` §13.2 / §13.7。

### 事件

- ✅ **CI run #331 全绿**（上一笔提交留的待办「待补 run #331 结论」**已补**）：head `fe2befc`，check_suite `111168057717`，**结论 = success**，**4 个 job 全绿** —— `test (ubuntu-latest)` ✓ · `test (macos-latest)` ✓ · `test (windows-latest)` ✓ · `smoke run examples` ✓。⇒ **run #330 的 macOS 唯一红**（`http_contract::the_handler_can_choose_to_stop_the_stream`）**随 `fe2befc` 转绿** ⇒ 计划登记 **⑳ 结案**。⚠ 每个 job 的 **step 1–23 逐条 success**（含 step 15 全量测试 / **附录 A 逐字对拍** / TLS extension / bench smoke）⇒ **首次在 CI 上看到全量门禁一次跑完且全绿**。
- ✅ **规范仓 v1.0.4（`d6d90c3`）已 push** ⇒ 实现仓 CI 检的就是那个 ref ⇒ 「**规范仓先推、实现仓后推**」的顺序已执行完毕。
- ⚠ **README 有四处误述**（本轮**逐条实测**后订正 ⇒ 见下表）。
- ⚠ **规范参考载体的**头部**落后于它的内容**（正文早已与规范仓 v1.0.4 一致，**头部**还写着旧基线）。
- ⚠ **`docs/wlwl-feasibility/probes/` 是纯副本**（与门禁夹具逐字节相同，却从不被任何测试或 CI 读取）⇒ **整目录归档为单 md**。
- ⚠⚠ **归档时实测发现探针 README 的三行与探针实测相反**（详见「探针漂移」一节）—— 这是**新发现**，不是本轮引入的。

### README 四处误述的订正（全部实测验证）

| # | 原文写的 | 实测 | 订正后 |
|---|---|---|---|
| 1 | `19 std.*` namespaces with `204` members | `wlwl namespace` ⇒ **23 `std.*` = 278 成员** + 官方包 `ai` 5 + `agent` 5 ⇒ **288 members / 25 namespaces** | `23 std.*` / `278`（并注明全层 **288 / 25**）；标准库表**补上 `std.http` `std.sync` `std.log` `std.signal`** 四个新命名空间 |
| 2 | 「**There are no synchronisation primitives.**」 | `std.sync` **已存在**（8 个成员：`COUNTER` / `COUNTER_INC` / `COUNTER_ADD` / `COUNTER_GET` / `SHARED_DICT` / `SHARED_DICT_SET` / `SHARED_DICT_GET` / `SHARED_DICT_SNAPSHOT`）；`std.fs` 另有 `LOCK_FILE` / `UNLOCK_FILE` | 改为「**没有阻塞式同步原语**（无 mutex / 无 condition variable —— 那些仍被 ADR-0017 Step 3 卡着），但 `std.sync` 提供**不需要挂起的原子计数器与共享字典**」 |
| 3 | 「**An unhandled error is only reported when it is the program's last expression; earlier ones are dropped silently.**」 | 实测 `LET(a, ERR("x")); PRINT("after");` ⇒ **`error[E0103]: unhandled ERR was discarded`**（中途丢弃**也报**）⇒ `E0102`（顶层逃逸）与 `E0103`（丢弃）是**两个码** | 改为「`E0102`（逃逸到顶层）或 `E0103`（被丢弃且未消费）」，并从 Status 段删掉那句错的 |
| 4 | 「**The most recent published release is a preview (RC) build.**」 | GitHub Releases：**v0.12.0 是正式版**（`prerelease: false`，2026-10-26 发布，5 个资产）⇒ RC 只是历史（`v0.12.0-rc2` / `-rc3`） | 改为「最新已发布是 **v0.12.0**；工作树在 **v0.12.1**（未打 tag）」 |

顺带：工具链表**补三个真实子命令**（`--help` 实测存在）—— `wlwl namespace`（列命名空间与成员，JSON）· `wlwl repl`（最小 REPL）· `wlwl tasks`（运行期任务视图，JSON）；技能包示例数 **20 → 22**（`wlwl-skill/examples/` 实测 22 份 `.wll`）。

### 规范参考载体的头部对齐（正文一字未动）

`docs/wlwl-spec-reference.md` 的**正文早已与规范仓 v1.0.4 一致**（本轮跑生成器 ⇒ **diff 仅 3 行**、**输出 603310 bytes 不变** ⇒ 合并版的**身体一个字都没动**）；**只有头部落后**：

| 头部字段 | 原值 | 现值 |
|---|---|---|
| 来源基线 | `v1.0.0`（11 册） | **`v1.0.4`**（11 册 —— 册数没变） |
| 标准库读数 | 21 个命名空间、214 个成员 | **25 个命名空间、288 个成员** |
| 守卫处数 | 23 处 | **30 处**（实测：`impl/` 下按 `## N` 文本定位章节的 `#[test]` 守卫数） |

⚠ **生成器同步改了**：`script/build_spec_reference.py` 的 `HEADER` 常量**与生成结果同改** —— 否则下次重建会把头部**覆回去**。⇒ 重跑生成器**幂等**（连续两次，diff 恒为那 3 行）。
⚠ 附录 A **逐字不动**：`check-appendix-a` **exit 0**（抄件与实现派生表体一致）· 合并册的附录 A 与规范仓注册镜像册**逐字节相同**（实测 4495 bytes 全等）。

### 探针整目录归档为单 md

`docs/wlwl-feasibility/probes/`（9 个 `.wll` + README 对照表）**已从本仓移除**，内容按**原文照录、不精简合并**并入单 md：[`docs/history/20261030-feasibility-probes-archive.md`](docs/history/20261030-feasibility-probes-archive.md)（体例同 `20261022-spec-archive.md`）。

**归档理由（实测，不是推测）**：
1. 9 个 `.wll` 与门禁夹具 `impl/tests/probe/cases/P121_probe_*/main.wll` **SHA256 逐字节相同（9/9）**；
2. 门禁**只读** `impl/tests/probe/cases/`（`probe.rs` 的 `cases_root()` 从 `CARGO_MANIFEST_DIR` 解析）⇒ **`docs/` 下那份副本从不被任何测试或 CI 读取**；
3. ⇒ 留着只是**两份会漂移的同源文件** —— **而它已经漂移过了**（下一节）。

⇒ **可重跑的证据仍在仓内**：`impl/tests/probe/cases/P121_probe_*/main.wll`（9 份，逐字相同）+ 同目录 `expect.json`（断言）⇒ 门禁 `cargo test --locked -p wlwl-cli --test probe`（`EXPECTED_CASE_COUNT = 196`）。
⇒ **需求清单的承诺同步改**（`docs/plan/wlwl-迁移需求清单.md`）：§J-3 第 3 条与附录 A 的路径指向**归档件 + 可跑夹具**两处，附录 B 的**复现命令改成指向 `impl\tests\probe\cases\`**（新命令**已实测跑通**：p01 / p13 ⇒ exit 0）⇒ 「可直接作为新成员的上手材料与回归基线」这条承诺**仍然成立**，只是路径换了。
⚠ 需求清单是**外部提出**的原文 ⇒ 本轮**只加「订正」批注**（文首一节 + §J-3 + 附录 A/B），**不重写 TinyPage 侧的措辞**。

### ⚠ 探针漂移（新发现 · 归档时实测）

归档件的 README 对照表有**三行**写的是**第二波（`86b3c4a`）与第四波（`969b220`）之前**的行为，与现在的探针**实测相反**：

| 探针 | README 写的 | 实测（现在） | 出处 |
|---|---|---|---|
| **p07** | 「handler 闭包里再定义并调用内层闭包 ⇒ **跑通了**：终值 **42**、exit 0」 | **exit 1** —— `error[E0014]: TCP_RECV: used outside a task body`（挂起点落在**任务体之外**的内层闭包 ⇒ 宿主挂起点成员在任务体外被调用）⇒ **终值 42 从未出现** | 第二波 `86b3c4a`（W-38 把「宿主挂起点在任务体外」诊断化） |
| **p09** | 「任务正常跑完、`AWAIT` 拿到 **42**、exit 0、stderr 空」 | **exit 1** —— 同一条 `E0014` ⇒ **stderr 不空**，终值 **42 从未出现** | 第二波 `86b3c4a` |
| **p12** | 「**惊群坐实**：只有 **1** 个拿到连接、其余 **2** 个拿到 `NULL`；落选者是**被连接到达唤醒**的（≈155 ms）」 | **惊群已修** —— `winners= 2` · `losers= 1` · `losers-woke-immediately= FALSE` · `loser-elapsed= 2042`（落选者**等满自己的 2000 ms 超时**） | 第四波 `969b220`（W-11「唤醒一个」） |

⇒ `expect.json` 的 `_why` 本来就写着「本用例在 W-11 修掉惊群**之后已翻转** —— 它现在钉的是『**惊群已修**』」⇒ **门禁断言是对的，README 落后了**。
⇒ **归档件里的 README 原文照录不改**（体例如此），但**归档件里另附一张「漂移表」**标出以上实测真相；需求清单也加了对应订正。⚠ 原文的 **Δ1 / Δ2** 两行同样作废。

### 门禁（本轮实测）

**文档改动**（`README.md` · `CHANGELOG.md` · `docs/wlwl-spec-reference.md` · `script/build_spec_reference.py` · `docs/plan/**` · `docs/history/**`）⇒ **零代码改动** ⇒ **不触发** Rust 侧**语义**门禁。⇒ 但与本次改动直接相关的门禁**已逐道实测跑过**：

| 检查 | 读数 |
|---|---|
| `cargo test --locked -p wlwl-cli --no-fail-fast` | **全部 target 全绿**（**0 failed**）—— 含 `probe` **3/3**（278 s）· `skill_package_guard` · `spec_ref_hygiene_guard` **7/7** · `spec_needle_scope_guard` **7/7** · `conformance` **6/6** · `spec_r2_conformance` **11/11** · `stdlib_dual_track` · `skill_examples` · `orphan_report` |
| `cargo test --locked --no-fail-fast -p wlwl-eval` | **全绿 · exit 0** —— **0 failed**（`--lib` **920/0** · `fs_contract` **29/0** · `http_contract` **25/0** · `middleware_contract` **26/0** · `json_contract` **49/0** · `sanitize_contract` **39/0**（781 s，**既有慢用例**）· `str_math_contract` **54/0** · `result_contract` **24/0** · `stdlib_appendix_a_sync` **9/0** · `spec_appendix_g_sync` **3/0** · `v07_fidelity` **2/0**） |
| `cargo test --locked --no-fail-fast -p wlwl-std` | **385/0** |
| `cargo test --locked --no-fail-fast -p wlwl-ast -p wlwl-error -p wlwl-ext -p wlwl-formatter -p wlwl-lexer -p wlwl-parser -p wlwl-toml -p wlwl-types -p wlwl-value` | **全绿 · exit 0**（**0 failed**，含 `wlwl-types` **144/0** · `wlwl-parser` **88/0** · `wlwl-toml` **89/0**） |
| `cargo fmt --all -- --check` | **0 diff** |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | **0 error** |
| `cargo doc --workspace --no-deps`（`RUSTDOCFLAGS=-D warnings`） | **0 error** |
| `python script/check_conformance_registry.py` | **exit 0**（§5.4 状态列与契约表一致（25 行）） |
| `cargo run --locked --bin check-appendix-a -- ../docs/wlwl-spec-reference.md` | **exit 0**（附录 A 逐字对拍） |
| `python script/build_spec_reference.py`（重跑） | **幂等** —— 连续两次，diff 恒为头部那 3 行，**603310 bytes 不变**（SHA256 两次相同） |
| 合并册附录 A ↔ 规范仓注册镜像册 | **逐字节相同**（4495 bytes 全等） |
| 9 个探针（`impl/tests/probe/cases/` 那份） | **9/9 逐个单独跑**，退出码与各自 `expect.json` **一致**（p01/p02/p03/p10/p11/p13 ⇒ 0；p07/p09/p12 ⇒ 1，为**期望值**） |
| 仓内 md 相对链接 | **新增 0 条断链**（全仓扫描；剩余 12 条均为**改动前就存在**的历史断链，非本轮引入） |

### 公共面变化

**零** —— 本轮**无代码改动** ⇒ 公共面仍是 **288 members / 25 namespaces**（`wlwl namespace` 实测），**零新增诊断码**，版本口径仍是 实现 **v0.12.1** ↔ 规范 **v1.0.4**。

### 下一步

- ⚠ **本轮零代码改动** ⇒ 本段写下时 **v0.12.1 仍未打 tag**（工作树 `0.12.1`，最新已发布是 **v0.12.0**）。⇒ ⚠ **后续已发 `v0.12.1_rc1`**（RC，非正式版）—— 见本文件顶部的 **[v0.12.1_rc1]** 段。
- ⇒ **发版还剩一件**：**TLS 扩展产物**（Windows `.dll` / Linux `.so` / macOS `.dylib` + ABI 版本 + 哈希）随发行包发布，见计划 §0.4 第 4 件。⚠ **该件在 `v0.12.1_rc1` 的发布包里也没有**（`release.yml` 的 staging 步骤未含它）—— ⚠ **已在 `main` 上补齐**（见下方「TLS 扩展产物随包发布」段）⇒ **下一个发布包**里就有它。

---

## [v0.12.1 收口] — 2026-10-30（文档 / 技能包 · 规范仓 v1.0.4 落表 · 附录 A 抄回 · 全量门禁 · 三条实测缺陷修复 + 一条基线重生成）

**本段 = 收口批**：**W-34**（文档 / 技能包）+ **W-35′**（登记 + 附录 A 抄件 + 全量门禁）+ 规范仓 **v1.0.4**（D-21 公共面 58 项 + D-22 `E0014` 触发条件补全）。计划与坐标见 `docs/plan/wlwl-v0.12.1-build-plan.md` §4 / §11.1 / §11.2 五步 / §13。

### 事件

- ✅ **规范仓 v1.0.4 已落表**（`d6d90c3`，**本地已提交、尚未 push**）：附录 A 收了 **58 个登记项**（57 个命名空间成员 + 全局内建 `ARRAY_GET`）· 各册成员表补行（`std.text` / `std.fs` / `std.net` / `std.format` / `std.str` / `std.test` / `std.regex` 加成员 + `std.log` / `std.sync` / `std.signal` / `std.http` **四个新命名空间**）· 总纲 §0.9 新增登记行 **D-21** · 一致性册 §5.5 台账**第一行**（`tls` 10 员 / `crypto` 8 员）· 语言册 `E0014` 触发条件补全（**D-22**，对齐并发册 §17.0）。
- ⚠ **CI 的规范仓钉死提交前移**：`ci.yml` 由 `408e1f0` → **`d6d90c3`** ⇒ ⚠ **该提交必须先 push 规范仓**，否则实现仓 CI 检不出这个 ref ⇒ **顺序：规范仓先推、实现仓后推**。
- ⚠⚠ **收口夜抓出三条实测缺陷 + 一条过期基线**（详见下面「修掉的三条」与「④ 基线重生成」）—— 其中一条**潜伏了一周**才暴露。

### 修掉的三条（收口夜全量门禁抓出）

| # | 缺陷 | 症状 | 根因 | 修法 |
|---|---|---|---|---|
| **A** | **`middleware_contract` 的并发用例期望值依赖墙钟** | 用例 `concurrent_counting_is_exact_under_threads` 期望 `979998`、实得 `980510`（**少 512 次**）⇒ 机器一慢就红 | 滑窗 `remaining` 是「最近 60 秒的窗口和」⇒ 20002 次调用跑了 **61.5 s** ⇒ 最早的 ~1.5 s 的调用**滑出窗口** ⇒ 违反 §8.3 第 8 条「用确定性计数，不用墙钟」⇒ **期望值本身就是错的** | 改用**令牌桶**（`refill = 0` ⇒ 全部计数进同一个计数器 ⇒ 总量与墙钟无关）⇒ 期望值不变;**另加**一条滑窗并发用例（只断言与墙钟无关的事实）⇒ 该用例**只剩一种**变红理由 |
| **B** | ⚠⚠ **`STATIC_FILE` 的 `If-Modified-Since` 比较是错的** | 用例 `an_if_modified_since_older_than_the_file_gives_200` 期望 **200**、实得 **304** ⇒ **客户端会拿到错误 304 ⇒ 缓存住旧内容** | 旧实现拿**整串**比**字典序**，而 IMF-fixdate 的字段顺序**不是时间序**（**星期名在最前** · **日在月年之前**）⇒ ① 星期名支配结果（`"Thu" >= "Fri"` ⇒ 1970 年也被判成「没变」）;② `"01 Jan 2038"` 字典序 **小于** `"30 Oct 2026"` | `_MATCH_IMS` 改成**逐字段解析**成 `YYYYMMDDHHMMSS` **整数键**再比大小（定宽十进制 ⇒ 数值比较 == 时间比较）⇒ 数字解析复用既有 `_PARSE_DEC`（**不用 `INT`** —— 它失败返回 `ERR` 值 ⇒ 透明传播）⇒ 认不出的月份 / 非数字字段 ⇒ **不可比 ⇒ 走 200** |
| **C** | ⚠ **`process_contract` 两条 timeout 用例的「耗时」断言在 Windows 上不可靠** | 并行跑 **5 次里 3 次红**（耗时 3.59 s / 4.13 s，上界 2.5 s / 3 s）；单跑绿 | Windows 的杀树走 `taskkill /T /F` —— 它是一个**子进程 spawn** ⇒ **耗时随负载在 1–3.5 s 间随机波动** ⇒ 任何**绝对**上界都会被撞穿;**相对**判据也不行（两次测量**各自**被抬 1–2.6 s）⇒ ⚠ **根子在本文件头早就写着**：「「耗时有界」**只有 POSIX 咬得到**」⇒ 是**代码没对齐文件头** | 两条用例的**耗时断言改成 `#[cfg(unix)]`**（Windows 侧只钉 `ERR` 值这个**结果形态**）+ 给两条加一把**互斥门**（`timeout_gate`）⇒ Windows 侧那半由 `wlwl-std` 的 `drain_is_bounded_even_when_the_reader_never_finishes` 守住;**POSIX 上「耗时有界」仍然钉着** ⇒ **负向验证**（WSL 上退回「不按组杀 + 无界 drain」的历史缺陷形态 ⇒ 用例挂住 > 4 min）⇒ 复原后 `process.rs` **SHA256 逐字节一致** |

⚠ **B 的潜伏期**：W-19 落地那天（2026-10-29）是**周四** ⇒ 星期名相等 ⇒ 字典序恰好落到日期段 ⇒ 用例是绿的 ⇒ **缺陷到 2026-10-30（周五）才暴露**。⇒ **教训：别只看「用例绿不绿」，要看它「在什么条件下绿」。**
⚠ **C 不是本轮引入的**：它是第四波登记 ⑦ 的那条 flake（**登记为待办**）⇒ 收口批的**全量门禁**把它撞了出来 ⇒ 本轮**按实测**改了根因（不是按原来的猜测「两条互相误杀」—— 那部分只解释了一半）。
⚠ **第四条（run #330 的 macOS 红）**：CI 这才**第一次真跑到** `http_contract`（它此前被 **fail-fast 盲区**整条藏着 ⇒ `fs_contract` 一直红 ⇒ 后面的 target 从没执行过）⇒ macOS 唯一红：`the_handler_can_choose_to_stop_the_stream`（**pass 1 与 pass 2 都红** ⇒ 真失败）⇒ 根因：客户端在服务端关闭**之后**又写一条请求 ⇒ 服务端那个 socket 的收队列里有未读数据 ⇒ TCP 栈回 **RST**（不是 FIN）⇒ 客户端读响应头拿到 `ECONNRESET` ⇒ ⚠ 这是 **TCP 的规定行为**（RFC 9110 §9.6：客户端不得复用已被服务端关闭的连接）⇒ **不是实现缺陷** ⇒ **修法**：客户端把「连接被重置」与「读到 0 字节」**同义**看待（新增 `is_peer_gone()`）⇒ **实现侧零改动** ⇒ 本机 3 轮 + Linux(WSL2) 侧均绿。
⚠ **三条反向守卫都真红过**（A：Rust 层 `fetch_add` → 先读后写 ⇒ `979999 ≠ 979998`;中间件层原子自增 → 非原子读-改-写 ⇒ `982049 ≠ 979998`（丢 2051 次）;B：换回旧比较 ⇒ `Wed, 01 Jan 1970 …` ⇒ 304 ≠ 200 ⇒ **且与当天是星期几无关**;C：WSL 上退回历史缺陷形态 ⇒ 用例挂住 > 4 min）⇒ 复原后 `sync.rs` / `http.wll` / `process.rs` **SHA256 逐字节一致**。

### 落地清单（2 项 + 规范仓 1 项）

| 工作项 | 内容 | 验收读数 |
|---|---|---|
| **W-34** | 文档 / 技能包同步：**H-1 回归守护**（`IF` 第三实参**支持**多语句组 ⇒ 新增示例 `wlwl-skill/examples/if_else_multistmt.wll` + 守卫 `skill_examples.rs::h1_if_else_third_arg_holds_a_multi_statement_group`）· **H-2 / H-3 口径订正**（`STR` / `SPLIT` 是**全局内建**，不是 `std.str` 成员;`+` **不做任何隐式强转**）· 成员面旧读数（231 / 21 命名空间）全部作废 ⇒ 当前 **288 / 25**（`wlwl namespace` 实测）· 本轮新成员（`std.http` 16 员 / `std.sync` 8 / `std.log` 7 / `std.signal` 2）全部写进 `SKILL.md` / `reference.md` / `docs/site/*` | `skill_package_guard` 的 `FORBIDDEN` 表从 3 条扩到 **18 条**（每条都带**为什么**）· `skill_examples` 的 `EXPECTED_TOPICS` 加一条 ⇒ **漏写就红** |
| **W-35′** | 附录 A 抄件 + 各册成员表**抄回**实现仓 + 期望名单同步 + 规范仓钉死提交前移 + **失败口径对拍**（V-05 钉子表，新增 `impl/crates/wlwl-eval/tests/failure_surface_pins_contract.rs`）+ 版本号 0.12.0 → **0.12.1** | `check-appendix-a` **exit 0** · `stdlib_appendix_a_sync` **绿** · `net_contract`（14 → **16**）· `fs_contract` / `str_math_contract` / `text_contract` 成员表**全绿** · `check_conformance_registry.py` **exit 0** · 抄件重建**幂等**（跑一次 SHA256 不变） |
| **规范仓 v1.0.4** | D-21 公共面 58 项 + D-22 `E0014` 触发条件补全 | `python script/check_spec.py` **exit 0**（错误 0 · 失败口径对拍**缺口 0**）· 17 册 / 519 已定义章节 |

### 集成者做的四件事

1. **锁文件策略登记**（§8.1 要求的收口项）：`impl/Cargo.lock` 与 `impl/ext/wlwl-tls-ext/Cargo.lock` **都不入库**（`.gitignore`）⇒ 依赖集读数**不可复现** ⇒ **发行说明钉死版本**：`rustls 0.23.46` · `ring 0.17.14` · `rustls-webpki 0.103.16` · `webpki-roots 1.0.11`（发行包**随包走**锁文件 ⇒ 重建出来的是同一个依赖集）。
2. **失败口径对拍钉子表**（新文件 `impl/crates/wlwl-eval/tests/failure_surface_pins_contract.rs`）：规范册成员表「失败」列声明的每个诊断码，实现侧**必须有断言位**钉住 ⇒ `check_spec.py` 的「失败口径对拍」**缺口 0**（扫 `src:` / `expect:` 行算已钉面）⇒ 且这些钉子**全部真的跑**（只给扫描器看、不跑的「钉子」是假绿）。
3. **三条实测缺陷的定性 + 修复**（上面那张表）—— ⚠ 三条都不是「上一波没做完」，是**上一波的实现 / 用例里一直存在的错**，被收口批的**全量门禁**抓出来。
4. **④ 基线重生成**（`v07_fidelity` 的黄金基线过期 ⇒ 被 **fail-fast 盲区**（登记 ⑫）藏着 ⇒ **从来没被报过**）：漂移**逐字可归因** —— **3 个** fixture，全部是 §13.2 早就登记过的「**人形态新增两行**」（`^^^^` 高亮行 ×2 + `= hint:` 行 ×1）⇒ **无意外漂移** ⇒ 那次改动**是有意的** ⇒ 基线本就该重生成 ⇒ **走仓里规定的路径**（`cargo test -p wlwl-eval --test v07_fidelity -- --ignored` 的 `v07_fidelity_bless_golden`）⇒ **不是手改快照** ⇒ 重生成后两条对拍**绿**。

### 门禁（收口实测）

**Windows（13 道全绿）**：`cargo fmt --all -- --check` **0 diff** · `cargo clippy --locked --workspace --all-targets -- -D warnings` **0 error** · `cargo doc --workspace --no-deps`（`-D warnings`）**0 error** · `cargo test --locked --all-targets --no-fail-fast` **0 failed** · probe **3/3** · `conformance` **绿** · `cargo deny --locked --all-features check` **exit 0** · `cargo check --locked -p wlwl-std --features real-ai` **exit 0** · `cargo test --locked -p wlwl-tls-ext` **绿** · `check-appendix-a` **exit 0** · `stdlib_appendix_a_sync` **绿** · `check_conformance_registry.py` **exit 0** · 规范仓 `check_spec.py` **exit 0**（错误 0 · 失败口径对拍**缺口 0**）。
**Linux（WSL2）**：clippy 全 workspace **0 error** · `http_w19_contract` **11/0** · `middleware_contract` **26/0** · `fs_atomic_lock_meta_contract` **21/0** · `http_contract` **25/0** · probe **3/3**。

⚠ **本地门禁这一轮特意加了 `--no-fail-fast`**（CI 命令没有 ⇒ 登记 ⑫ 的盲区仍在）⇒ 把「所有 target 一次跑完」在本地做实了 —— 正是这样才把**被盲区藏住的** `v07_fidelity` 基线过期（登记 ⑲）挖出来。

### 公共面变化（**已落规范仓 v1.0.4**）

- 公共面读数 **288 members / 25 namespaces**（`wlwl namespace` 实测）⇒ 58 个登记项（57 个命名空间成员 + 全局内建 `ARRAY_GET`）已进附录 A 与各册成员表。
- ⚠ 既有成员**零改动** · **零新增诊断码** · 版本口径仍是 实现 **v0.12.1** ↔ 规范 **v1.0.4**。

### 下一步

- ⚠ **push 顺序**：**规范仓 `d6d90c3` 先推** ⇒ 再推实现仓 `main`（实现仓 CI 会 checkout 规范仓那个 ref ⇒ 顺序反了 CI 直接红）。
- ⚠ **发版前的最后一件**（§0.4 第 4 件）：**TLS 扩展产物**（Windows `.dll` / Linux `.so` / macOS `.dylib` + ABI 版本 + 哈希）随发行包发布。

---

## [v0.12.1 开工 · 第十一波] — 2026-10-29（W-25 中间件原语：限流 / 规则热加载 / 审计 / 真实 IP;公共面 +3 成员）

**本段 = 第十一波**：**W-25**（D-3 / D-5 / D-6 / D-7）+ CI run #328 结论落档。计划与坐标见 `docs/plan/wlwl-v0.12.1-build-plan.md` §4（工作项）/ §11.1（落地清单）/ §13（接替手册）。

### 事件

- **CI run #328**（check_suite `110809776912`，head_sha `89e1f30`）⇒ 三平台 fmt/clippy/doc/deny/conformance **全绿**;step 15 唯一失败 = `fs_member_set_matches_the_spec_table`（`§2 table missed 'PATH_CANONICAL'` ⇒ **规范侧 `std.fs` 成员表未跟上**，不是缺陷）;**`a_reader_never_sees_a_half_written_target` macOS/ubuntu 各两轮 ok** ⇒ **EINTR 假设成立**，登记 ⑨ **复核结案**（run #326 + #328 连续两跑）;「注解带 panic 原因」通道实测有效 ⇒ 归因不必捞日志。
- ⚠ **CI fail-fast 盲区仍在**（登记 ⑫）⇒ 其余规范侧欠账只在本地全量跑可见。

### 落地清单（1 项）

| 工作项 | 内容 | 验收读数 |
|---|---|---|
| **W-25** | `std.http` 新增 **3 个**成员：`RATE_LIMITER`（滑动窗口 + 令牌桶，计数走 `std.sync::COUNTER_INC` 原子）· `RULESET_LOAD`（mtime 变更热更新，**一次 `SHARED_DICT_SET` 原子换引用** ⇒ 在途旧数组不受影响）· `CLIENT_IP`（仅可信代理采信 XFF，取最右非可信跳;**不可信远端无视 XFF**）;D-6 审计经**不导出** `_AUDIT` 走 `std.log` 六字段 JSON 行（时间/事件/客户端 IP/路径/结果/请求 ID）⇒ 零新增导出成员 | `middleware_contract` **25/0** · probe 新增 `P121_d3_d5_d6_d7_middleware`（三条验收读数全钉）· **两条反向守卫真红过**（计数改非原子 ⇒ 丢 280 次更新 `980278≠979998`;热切换改原地改旧数组 ⇒ 并发读者见撕裂条目）⇒ 复原 SHA256 逐字节一致 |

### 集成者做的三件事

1. **共享表登记**：`EXPECTED_CASE_COUNT` 195 → **196**;`http_contract` 冻结成员面 13 → **16**（断言改名 `the_member_set_is_exactly_sixteen_members` —— 名字跟着数量走，不许撒谎）。
2. **口径核对**：三条新成员的语义逐条对照计划 W-25 规格（滑窗 60/60s 第 61 次拒 · 原子切换 · 可信代理）✓;既有 13 个成员一字未改 ✓;零新增诊断码 ✓;`fs.rs` 零改动（mtime 用既有 `FILE_META`）✓。
3. ⚠ **一处 briefing 事实订正（已文档化）**：`std.sync` 的 `COUNTER(initial)` / `SHARED_DICT()` **每次调用新建容器、无按名查回**（与 §11.1 W-13 行的「按名」印象不符）⇒ http.wll 内补按 name 注册表层（缺省挂 `StdCtx.env`，跨求值器经 `opts.state` 显式传句柄）。

### 门禁（第十一波集成后实测）

Windows：`cargo fmt --all -- --check` **0 diff** · `cargo clippy --locked --workspace --all-targets -- -D warnings` **0 error** · `cargo doc --workspace --no-deps`（`-D warnings`）**0 error** · `wlwl-std` **385/0** · `http_contract` **25/0** · `middleware_contract` **25/0** · probe **3/3（196 用例）** · conformance **6/0**。
**Linux 侧**（WSL2）：clippy 全 workspace **0 error** · `fs_atomic_lock_meta_contract` **21/0** · `http_contract` **25/0** · probe **3/3** · ⚠ `middleware_contract` **首跑 24/1 偶发**（用例名未抓到 ⇒ **登记 ⑰**，教训同 ⑪）⇒ 其后 9 连跑含 CPU 加压全绿 ⇒ 定性负载相关 flake。
**红项仍是那两处已知欠账**（`stdlib_appendix_a_sync` 4 failed · `fs_contract` 28/1）⇒ 全部是「规范侧未跟上」，**零新增**。

### 公共面变化（待规范仓落表：**55 → 58 个成员**）

- `std.http` **+3**：`RATE_LIMITER` · `RULESET_LOAD` · `CLIENT_IP`
- ⚠ 既有成员**零改动** · **零新增诊断码** · 版本口径仍是 实现 **v0.12.1** ↔ 规范 **v1.0.4**

### 下一步

实现侧只剩**收口批**：**W-34**（文档 / 技能包）· **W-35′**（登记 + 附录 A 抄件 + 全量门禁）+ 规范仓 **v1.0.4 五步**（业主那边，§11.2）。

---

## [v0.12.1 开工 · 第十波] — 2026-10-29（`std.http` 四节之三：路由 / 静态文件 / 客户端;公共面 +11 成员）

**本段 = 第十波**：`std.http` 的剩余三节（W-18 / W-19 / W-20）。计划与坐标见 `docs/plan/wlwl-v0.12.1-build-plan.md` §4（工作项）/ §11.1（落地清单）。

### 事件

- **CI run #326**（head_sha `84390a0`）⇒ 三平台 **fmt / clippy / doc / cargo-deny / conformance 全绿**；唯一出现的失败用例是 `fs_member_set_matches_the_spec_table`（`§2 table missed PATH_CANONICAL` ⇒ **规范侧成员表未跟上**，不是实现缺陷）。
- **✅ 第九波 W-39 结案**：macOS 上 `test a_reader_never_sees_a_half_written_target ... ok`（**pass 1 与 pass 2 都 ok**；此前 run #325 是两轮都红）⇒ **EINTR 修复生效**。
- **✅ 新注解机制实测有效**：workflow annotation 现在带 panic 原因全文 ⇒ 归因**不再需要**捞日志（workflow 日志匿名 403 / artifact 401）。
- ⚠⚠ **新发现（登记 ⑫）**：`cargo test --locked --all-targets` **不带 `--no-fail-fast`** ⇒ **第一个失败目标之后的目标不再执行** ⇒ 实测有四个用例名在 CI 日志里**一次都没出现**（本地跑它们是红的）⇒ **那几笔规范侧欠账在 CI 里从来没被执行过**。⚠ 这是 `ci.yml:319-323` 里**刻意**写下的决定 ⇒ 本轮不动 ⇒ **待业主裁决**。

### 落地清单（3 项 · 三个 agent 并行 · `http.wll` 的写入串行接线）

| 工作项 | 内容 | 验收读数 |
|---|---|---|
| **W-18** | `std.http` 新增 **6 个**成员：`ROUTE` · `MATCH_ROUTE`（命中 / **405** / **404**）· `PARSE_COOKIES` · `SET_COOKIE` · `PARSE_FORM` · `PARSE_MULTIPART`（**文件 part 落盘**） | `http_w18_contract` **15/0** · 反向守卫（multipart 边界改前缀匹配 ⇒ 恰好 2 条红） |
| **W-19** | `std.fs` 新增 **`FILE_META`**（**一次 `stat` 的快照**）⇒ 理由：`EXISTS`+`FILE_SIZE`+`FILE_MTIME` 是**三次独立 stat** ⇒ 文件被改写时三读数互相矛盾；`std.http` 新增 **`STATIC_FILE`**（穿越防护**一律走 `PATH_CONTAINS`**） | `http_w19_contract` **11/0**（**28 条**行为用例）· `wlwl-std` **385/0**（连跑三次）· 反向守卫（防护改字符串前缀 ⇒ **穿越成功** ⇒ 必红） |
| **W-20** | `net.rs` 新增三个**私有 R2 内核** `http_client_get` / `http_client_post` / `http_client_request`（**+1206 行**，既有成员**一个字节未动** ⇒ `std.net` 公共面零变化）：增量 **chunked 解码状态机**（块边界 ≠ TCP 段边界 · 支持 chunk extensions 与 trailer）· 响应定界（TE+CL 并存⇒ERR · 重复 CL⇒ERR 含值相同）· 请求侧（重复 CL⇒ERR · `Transfer-Encoding`⇒ERR ⇒ **内容错先于联网**）· TLS 走 `tls:CONNECT/SEND/RECV/CLOSE` | `http_w20_contract` **16/0**（含「证书错误逐字透传」）· 反向守卫（抽掉 chunked 解码 ⇒ 恰好 4 条红） |

### 集成者做的四件事

1. **串行接线 `http.wll`**：三个 agent 都往同一个文件加东西 ⇒ 按 **W-18 → W-19 → W-20** 的顺序接；W-19/W-20 的段落先落在**仓库外**的段文件里并附接线说明，接线由 agent 自己完成（带着上下文更可靠）。
2. **登记内核**（`wlwl-std/src/lib.rs` 的 `wlwl:std.http` → `kernels`）：`_SERVE_STREAM`（第九波）· `_HTTP_GET` / `_HTTP_POST` / `_HTTP_REQUEST`（第十波）⇒ **公共面只长在 `std.http` 上，`std.net` 一个字节没动**。
3. **改 `http_contract.rs` 的冻结成员面**：3 → **13**（`the_member_set_is_exactly_thirteen_members`）⇒ 这条测试同时钉住「`EXPORT` 必须写在一行」。
4. **修两处 clippy**（`http_w18_contract.rs`）：`&PathBuf` ⇒ `&Path`（`clippy::ptr_arg`）与 `read_to_string(expected.as_path())`。

### ⚠⚠ Linux 侧当场抓出**两个跨平台 bug**（Windows 上全绿 ⇒ 本机门禁看不见）

集成者把 **unix 侧变成本地可验**（WSL2 + rustc 1.99.0 + 无 root gcc）⇒ 在 Linux 上跑，**两条红**：

| 症状 | 根因 | 修法 |
|---|---|---|
| `PARSE_MULTIPART` 的文件 part 落盘报 `parent directory does not exist` | ⚠ **`std.fs::PATH_JOIN` 会把 POSIX 的前导 `/` 吃掉**（实测 `PATH_JOIN("/tmp/x","f")` ⇒ `"tmp/x/f"` ⇒ **相对路径**）⇒ Linux 上落进进程 CWD；Windows 上 `C:\` 不受影响 ⇒ **同一输入两个平台两个结果** | 落盘路径**自己拼**（`CAT(dir, "/", 名)`）⇒ 根得以保留;W-18 另加断言「落盘路径必须是绝对路径且在临时目录之下」⇒ 复发当场红 |
| `STATIC_FILE` 的「绝对形请求路径」用例：Windows 得 404、Linux 得 **403** | 同一根因：`PATH_JOIN(root, rel)` 在 Linux 上掉了根 ⇒ `target0` 变相对 ⇒ `PATH_CONTAINS(root, target0)` 必然判假 ⇒ 403 | 同样**自己拼**（`CAT(root, "/", rel)`，并把 `index.html` 那处也改掉）⇒ **URL 路径永远相对 root** ⇒ 两平台同结果 |

⇒ ⚠ **`std.fs::PATH_JOIN` 自身没动** —— 它是**既有成员**，改它的可观察行为撞规范 `:223` ⇒ 已**单独登记**（§13.4 ⑯）⇒ **需规范侧裁决**：若规范上 `PATH_JOIN` 的语义是「拼接两段路径」，则吃掉根是**实现缺陷** ⇒ 修它属「修缺陷」而非「扩语义」。

### 三条降级/边界（已显式登记，§13.4 ⑬⑭⑮）

- **⑬ multipart 的文件 part 落盘是「有损 UTF-8 文本」** —— 本语言正文就是 `STRING` ⇒ 二进制 part 无法原样落盘 ⇒ 单独立项。
- **⑭ `STATIC_FILE` 的符号链接逃逸**（**安全相关**）：`PATH_CONTAINS` 是**词法**判定 ⇒ 根内指向根外的符号链接仍判合法 ⇒ 要堵住得改既有成员（撞规范 `:223`）⇒ 单独立项。
- **⑮ `STATIC_FILE` 读不出大于 1 MiB 的文件**（`READ_BYTES` 的既有上限）⇒ 单独立项。

### 门禁（第十波集成后实测）

`cargo fmt --all -- --check` **0 diff** · `cargo clippy --locked --workspace --all-targets -- -D warnings` **0 error** · `cargo doc --workspace --no-deps`（`-D warnings`）**0 error** · `wlwl-std` **385/0** · `http_contract` **25/0** · `http_w18_contract` **15/0** · `http_w19_contract` **11/0** · `http_w20_contract` **16/0** · probe **3/3** · conformance **6/0** · **Linux 侧**（WSL2）：`fs_atomic_lock_meta_contract` **21/0**（含那条 `#[cfg(unix)]` 用例 ok）· `http_contract` **25/0** · clippy 全 workspace **0 error**。

**红项仍是那两处已知欠账**（`stdlib_appendix_a_sync` 4 failed · `fs_contract` 28/1）⇒ 全部是「规范侧未跟上」，**零新增**。

### 公共面变化（待规范仓落表：**44 → 55 个成员**）

- `std.http` **+10**：`ROUTE` · `MATCH_ROUTE` · `PARSE_COOKIES` · `SET_COOKIE` · `PARSE_FORM` · `PARSE_MULTIPART` · `STATIC_FILE` · `GET` · `POST` · `REQUEST`
- `std.fs` **+1**：`FILE_META`
- ⚠ 既有成员**零改动** · **零新增诊断码** · 版本口径仍是 实现 **v0.12.1** ↔ 规范 **v1.0.4**

---

## [v0.12.1 开工 · 第九波] — 2026-10-29（3 个 PHASE + 1 项欠账单独处理;公共面 +8 个成员）

**本段 = 第九波**：**接管后的第一波**（4 个 agent 并行，文件面零重叠）。计划与坐标见 `docs/plan/wlwl-v0.12.1-build-plan.md` §4（工作项）/ §11.1（落地清单）/ §13（接替手册）。

### 事件

- **CI run #325**（head_sha `d9549a7`，`main`）⇒ 三平台 **fmt / clippy / doc / cargo-deny / conformance 全绿**（第八波两轮的修复全部生效 ⇒ 旧的那张「三平台分叉」表已清空）⇒ 三平台全红在 **step 15 `cargo test --locked --all-targets`**，且 **pass 1 与 pass 2 都红 ⇒ REAL FAILURE（非 flaky）**。
- 失败用例名（取自**匿名可读**的 workflow annotation —— `GET /repos/Laffinty/wlwl/check-runs/<id>/annotations` **无需 token**）：**windows / ubuntu = `fs_member_set_matches_the_spec_table`**（**已知欠账** = 规范侧 `std.fs` 成员表未跟上）· **macOS = `a_reader_never_sees_a_half_written_target`**（**新欠账，本轮首次登记 ⇒ W-39**）。
- ⚠ run #325 是这批提交**第一次**真的跑到 `cargo test --all-targets`（前七波的提交全推在 `wip0121`，该分支**不在 `ci.yml` 的 push 触发列表**里）⇒ **不是回归，是首次暴露**。

### 落地清单（4 项）

| 工作项 | 内容 | 验收读数 |
|---|---|---|
| **W-17** | `std.http::SERVE_STREAM` —— Keep-Alive 循环 + 分级请求预算 {首字节/头/体/响应/总} + 头块上限 64 KiB ⇒ **431** + 体上限 8 MiB ⇒ **413**（**在读取过程中就判**，不读完再判） | `http_contract` **25/0**（新增 **10 例**：一条连接连发 3 个请求 · 空闲超时服务端主动关 · 431 · 413 · 四档预算各触发 408 · handler 自选收尾）· **两条反向守卫都真红过**（上限改无限 ⇒ 431/413 用例红;Keep-Alive 改「处理一条就关」⇒ 3 连发用例红）· `net.rs` **纯追加 751 行 / 零删除** |
| **W-31** | `std.test` 新增 **7 个**成员：`ASSERT_MSG`（带必需消息）· `ASSERT_TRUE`（宽松口径真值断言）· `EXPECT_ERR_CODE`（捕获**原生**诊断码 ⇒ 原生码不逃逸）· `SKIP` · `TEST_CASES` · `TEMP_DIR`（**绝对路径**）· `RUN_TESTS_ASYNC` | `test_contract` **18/0**（表格 52 → **95 条**）· `wlwl-std` **381/0** · 反向守卫（`EXPECT_ERR_CODE` 改回捕不到原生码 ⇒ **恰好那 3 条红**、其余 92 条仍绿） |
| **G-4②** | `.wll.sig` 契约的工作流简化：`sig-gen` 写盘后**立刻回读自检** + `sig --format json` 的 `drift` 漂移面（`status` / `extra_in_declared` / `missing_in_declared` / `type_conflicts`）+ 人形态 `= hint:` 漂移行 | `wlwl-cli` **147/0**（新增 **9** 例）· 反向守卫（摘掉回读自检 ⇒ **恰好那 2 条红**）· `sig_schema_version` **1.0.0 → 1.1.0**（附录 E.4 明写「字段增删时递增版本号」） |
| **W-39** | macOS 独有的原子写守卫真失败：① `ci.yml` 的 workflow annotation 现在**带 panic 原因**（原先只带用例名）② `fs.rs` 的 `EINTR` **有界重试**（`retry_on_interrupt` + `open_new`〔中断后先删可能已建出的孤儿文件〕+ unix 的 `atomic_replace`）③ 该测试的 reader **只**放行 `ErrorKind::Interrupted`（`NotFound` 与其它错误**仍算中间态** ⇒ 退化实现照样被抓住） | ⚠ **macOS 侧根因未实测**（候选 = POSIX `EINTR`）⇒ **下一次 CI 会由新注解读出确切原因** · ✅ **Linux 侧已实测**（WSL2 Ubuntu）：`fs_atomic_lock_meta_contract` **21/0**（含那条 `#[cfg(unix)]` 用例 **ok**）· 反向守卫在副本里把 unix `atomic_replace` 改成「先删后写」⇒ **必红** ⇒ 复原后逐字节相同 |

### 集成者做的三处口径订正（agent 交付 ⇒ 集成者改）

1. **`ASSERT_TRUE` 的真值口径**（W-31）：agent 原做成「值**恰好**是布尔 `TRUE`」⇒ 计划 §4 W-31 点名的「`ASSERT(0)` / `ASSERT("")` / `ASSERT([])` **不该判失败**」**一件都没解决** ⇒ 回派改成**宽松口径**（只有 `FALSE` / `NULL` 为假）⇒ `ASSERT` 的 §2.3 八假值口径**未动**（`ASSERT(0)` 仍失败，已加契约钉住）。
2. **坏旁路签名文件的退出码**（G-4②）：agent 原做成「状态值 + 退出码 0」⇒ 回到审计 **R2-013** 的裁决（「**同一种故障不该因为走哪条路而拿到不同的退出码**」）⇒ `wlwl sig` 与 `wlwl check` 现在**同码同形**（`E0010` + 退出码 **3**）。
3. **`SERVE_STREAM` 的形态**（W-17）：agent 原做成 `std.net::TCP_SERVE_STREAM` 的**公共成员**（⇒ 公共面 +2、诊断指名一个**用户没调过**的名字）⇒ 改成 `std.http::SERVE_STREAM` 的**私有 R2 内核**（`_SERVE_STREAM`，登记在 `lib.rs` 的 `wlwl:std.http` 段）⇒ 公共面**只 +1**（`std.net` 不增）且**诊断指名用户真调过的成员** ⇒ 与 §6.2 的新成员表逐字对齐。

### 集成者另修的两处「红消息在撒谎」

- `net_contract` / `text_contract` 的成员数断言**写死了字面量**（`14` / `8`）⇒ 规范侧还没跟上时**断言文本说「exports 14 members」而实现侧其实是 16 / 11** ⇒ 把「诚实」换成「误导」⇒ 改成**对着规范表比** ⇒ 两条**仍然红**（那正是那笔欠账该有的红），但消息现在说的是真话。
- `stdlib_mirror.rs`：`std.http` 的「层」列写的是 `R1` ⇒ W-17 给了它 R2 内核 ⇒ 模块已是**混合** ⇒ 附录 A 派生体改成「混合(R1 `PARSE_REQUEST`/`BUILD_RESPONSE` + R2 `SERVE_STREAM`)」。

### 公共面变化（待规范仓落表：**36 → 44 个成员**）

- `std.test` **+7**：`ASSERT_MSG` · `ASSERT_TRUE` · `EXPECT_ERR_CODE` · `SKIP` · `TEST_CASES` · `TEMP_DIR` · `RUN_TESTS_ASYNC`
- `std.http` **+1**：`SERVE_STREAM`
- ⚠ **附录 E.4 要补一行**：`wlwl sig --format json` 的 `drift` 字段 + `sig_schema_version` 记 **1.1.0**（G-4②）
- ⚠ 既有成员**零改动** · **零新增诊断码** · 版本口径仍是 实现 **v0.12.1** ↔ 规范 **v1.0.4**

### 门禁（第九波集成后实测）

`cargo fmt --all -- --check` **0 diff** · `cargo clippy --locked --workspace --all-targets -- -D warnings` **0 error** · `cargo doc --workspace --no-deps`（`-D warnings`）**0 error**（修了 **2 处** `rustdoc::private-intra-doc-links`，与第八波**同款修法**：链接改纯代码字面量）· `wlwl-std` **381/0** · `http_contract` **25/0** · `test_contract` **18/0** · `wlwl-cli` **147/0** · probe **3/3** · conformance **6/0** · **Linux 侧**（WSL2）：`fs_atomic_lock_meta_contract` **21/0**（含那条 `#[cfg(unix)]` 用例 **ok**）· `http_contract` **25/0** · **clippy 全 workspace 0 error** ⇒ **unix 段第一次在本地被 lint、也被跑**。

**红项仍是那 8 处**（`check-appendix-a` exit 1 · `stdlib_appendix_a_sync` 4 failed · `spec_appendix_g_sync` 2 failed · `fs_contract` 28/1 · `net_contract` 14/1 · `str_math_contract` 53/1 · `text_contract` 12/1 · `process_contract` 1~3 failed）⇒ **零新增**，全部是「规范侧未跟上」（§11.2 五步）。

### 附带产出：unix 侧本地可验了

- 本机 WSL2 Ubuntu 装了 **rustc 1.99.0**（**与 CI 钉死的 toolchain 同版本**）+ 一份**无 root** 的 C 工具链（`apt-get download` + `dpkg -x` 解到 `~/ccsys`，**不碰 sudo**）⇒ **`#[cfg(unix)]` 的代码能本地编译、能本地跑测试、能本地 lint** ⇒ 计划 §13.6 第 14 行的「unix 侧本机不可验」**补上了一半**（macOS 独有行为仍只能靠 CI）。
- CI 的 workflow annotation 现在**带 panic 原因** ⇒ 下一次 CI 迭代就能**一次匿名 API 调用**读到 macOS 那条的**确切原因**，不必再靠推理。

---

## [v0.12.1 开工 · 第八波] — 2026-10-29(门禁补跑 —— 修 CI run #323 的 clippy 红;零语义改动)

**本段 = 第八波**：不是新工作项，而是**把 CI 跑红的 13 处 clippy lint 修掉**（**零语义改动**）**并把 clippy 补进本地门禁清单**。事件与坐标见 `docs/plan/wlwl-v0.12.1-build-plan.md` **§13.2「CI 实况」**。

### 事件

- **CI run #323**（head_sha `9a2dbda`，`main`）⇒ **failure**：三个 job（**windows / ubuntu / macos**）**红点完全一致** = `cargo clippy -D warnings` ⇒ step 11–23（`cargo doc` · `cargo-deny` · `cargo test` · **附录 A 对拍** · TLS · bench）**全部 skipped** ⇒ ⚠ **那 8 处「规范侧未跟上」的红，CI 这一次一次都没跑到** ⇒ 「CI 红」这次**不是**规范侧欠账，是**新回归**。
- 对照 run **#322**（head_sha `9bd0f46`）= **success** ⇒ 红是 `9bd0f46..9a2dbda` 这 **19 个提交**引入的 ⇒ **不是上游旧账**。
- **根因是流程缺口，不是代码缺陷**：前七波的 19 个提交**全推在 `wip0121` 分支**，而该分支**不在 `ci.yml` 的 push 触发列表**里 ⇒ clippy / doc / deny / test / conformance **一次都没跑过** ⇒ **本地门禁清单与 CI 步骤不对齐**（本地清单只有 fmt + 各 crate 测试，**漏了 clippy**）⇒「本地全绿」是**假绿**。

### 修法（13 处 · **全部零语义改动**）

| # | 位置 | lint | 修法 |
|---|---|---|---|
| 1–2 | `wlwl-types/src/check.rs`（`collect_defs` / `walk`） | `collapsible_match` | `Expr::Return { value, .. }` + `if let Some(v)` ⇒ 拆成 `value: Some(v)` / `value: None` 两条 arm（`None` 那条是空体 ⇒ 与原 `if let` 不匹配时的行为**逐字相同**） |
| 3 | `wlwl-std/src/fs.rs`（`acquire_lock`） | `suspicious_open_options` | `.create(true)` 之后**显式** `.truncate(false)` —— 与原默认行为**逐字一致**（锁文件只当会合点，内容无意义） |
| 4 | `wlwl-std/src/fs.rs`（原子写失败用例） | `io_other_error` | `Error::new(ErrorKind::Other, ..)` ⇒ `Error::other(..)`（MSRV 1.85 ≥ 1.74 ✔） |
| 5 | `wlwl-ext/src/registry.rs`（`host_alloc`） | `missing_safety_doc` | 补 `# Safety` 段（**只加文档**，与相邻 `host_free` / `host_report` 风格对齐） |
| 6–7 | `wlwl-ext/src/lib.rs`（读 ABI 头） | `clone_on_copy` | `u32` 是 `Copy` ⇒ 去掉 `.clone()` |
| 8 | `wlwl-eval/tests/log_contract.rs` | `redundant_redefinition` | 删 `let handle = handle;`（`i64` 是 `Copy` ⇒ `move` 闭包自动捕获） |
| 9–12 | `wlwl-eval/tests/sync_contract.rs` | `redundant_redefinition` | 同上（4 处） |
| 13 | `wlwl-eval/tests/http_contract.rs` | `doc_lazy_continuation` | 文档列表项续行缩进 2 → 4 |

- 引入提交（`git blame` 实测）：**第一波 `b0649cc` 6 处** · **第二波 `86b3c4a` 5 处** · **第七波 `64d9bbb` 2 处** ⇒ 全部是这 19 个提交带进来的新债。

### 集成者动作

- **本地门禁清单补齐**（根因消除）：§13.5 加第 **2b** 步（clippy，命令与 `ci.yml` 的步骤**逐字一致**）· §13.6 加第 **13** 行（门禁清单必须与 CI 步骤对齐）· §13.3「第一分钟」加 clippy 一行。
- **验收**：`cargo fmt --all -- --check` **0 diff** · `cargo clippy --locked --workspace --all-targets -- -D warnings` **0 error**（本地 clippy = **1.99.0** = CI 钉死的 toolchain ⇒ **本地结论即 CI 结论**）· `wlwl-types` **144/0** · `wlwl-error` 65/0 · `wlwl-ext` 5/0 · `wlwl-eval --lib` **920/0** · probe **3/3（195 用例）** · `conformance` 6/0 · `lsp_smoke` 2/0 · `spec_r2_conformance` 11/0 · **改过的文件必须重跑**：`ext_loader_contract` **22/0** · `http_contract` **15/0** · `log_contract` **21/0** · `sync_contract` **21/0**;**红项仍是那 8 处**（`check-appendix-a` exit 1 · `stdlib_appendix_a_sync` 4 failed · `spec_appendix_g_sync` 2 failed · `fs_contract` 28/1 · `net_contract` 14/1 · `str_math_contract` 53/1 · `text_contract` 12/1 · `process_contract` 1~3 failed）⇒ **零新增**。
- 本波**未改任何公共面成员** ⇒ 待规范侧落表总数仍是 **36 个成员 + 1 个新命名空间（`std.signal`）+ 附录 G 一行**。
- 本波**未动规范仓**（一行都没动）· **未发 tag** · 实现侧版本口径仍是 **v0.12.1**。

---

### 第二轮：run #324 暴露的**三平台分叉**（同一波的第二笔提交）

- **run #324**（head_sha `84fbe56`，`main`）⇒ **failure**，但**三个 job 的红点不一致**：

  | job | `cargo clippy`（step 10） | `cargo doc`（step 11） |
  |---|---|---|
  | **windows** | ✅ **success**（第一轮 13 处修完 ⇒ 本机可验的那部分已转绿） | ❌ **failure** |
  | **ubuntu** | ❌ **failure** | skipped |
  | **macos** | ❌ **failure** | skipped |

- **windows 的 doc 红** ⇒ **6 处 `rustdoc::private-intra-doc-links`**（public 文档用 intra-doc link 指向 private 项）: `wlwl-types/src/check.rs`（`ConcurrencyAnalyzer::is_outside_task`）· `wlwl-std/src/fs.rs`（`mtime_millis`）· `wlwl-std/src/net.rs`（`record_serve_failure`）· `wlwl-std/src/regex.rs`（`MAX_TRACKED_PATTERNS`）· `wlwl-std/src/sync.rs`（`SyncValue` ×2）⇒ **修法 = intra-doc link 改成纯代码字面量**（去方括号）⇒ **零语义改动**、可观察行为不变。
- **ubuntu / macOS 的 clippy 仍红** ⇒ ⚠ **根因比第一轮更深一层**：**`#[cfg(unix)]` 的代码在 Windows 的门禁里根本不编译** ⇒ 本机 clippy **永远看不见它**（仓里早就有这条警告：`wlwl-std/src/process.rs:66`）。这是**同形的第二次**（上一次是 2026-10-19 的 `clippy::useless_conversion`）。
  - **命中**：`wlwl-std/src/signal.rs`（**第五波新增的整个文件**）的 unix 段 —— `os_signal_handler as libc::sighandler_t`。`libc` 仓 `src/unix/mod.rs:21` 是 `pub type sighandler_t = size_t;` ⇒ **函数项直接转整数** ⇒ 触发 clippy 的 **`function_casts_as_integer`**（默认 warn）⇒ **只在 unix 编译** ⇒ ubuntu / macOS 双双红、Windows 绿。
  - **修法**：**先转指针再转整数**（地址逐字相同 ⇒ **行为不变**）⇒ `os_signal_handler as *const () as libc::sighandler_t`。
  - **取证不靠猜**（三段式，本机可复现）：① 读 cargo registry 的 `libc-0.2.191/src/unix/mod.rs` 确认 `sighandler_t = size_t`;② 用 **`clippy-driver` 单文件探针**在本机复现该 lint（`clippy-driver --edition 2021 -W clippy::all probe.rs`）并**顺手验证修法**;③ `git diff <CI 绿的基线>..HEAD` 圈定「这批提交动过的 `#[cfg(unix)]` 段」，逐个审 ⇒ **命中面只剩 `signal.rs` 一处**。
- ⚠ **本机做不了 linux-target clippy**：`x86_64-unknown-linux-gnu` **已装**，但 `zstd`（`wlwl-std` 的依赖）带 `zstd-sys` ⇒ 需要 `x86_64-linux-gnu-gcc`（**本机无**、**无网络装不了**）⇒ **unix 侧的唯一权威判据是 CI**。
- **集成者动作**：§13.2 加「run #324 三平台分叉」小节（读数 + 取证方式 + 排除清单）· §13.5 加第 **2c** 步（`cargo doc` 门禁）并给第 2b 步补「unix 侧本机不可验」的限制 · §13.6 加第 **14** 行（`#[cfg(unix)]` 的代码本机看不见 + 三段式做法），并**订正**第 13 行「本地结论即 CI 结论」这句 —— 它**只对当前平台成立**。
- 本波**未改任何公共面成员** ⇒ 待规范侧落表总数仍是 **36 个成员 + 1 个新命名空间（`std.signal`）+ 附录 G 一行**。
- 本波**未动规范仓**（一行都没动）· **未发 tag** · 实现侧版本口径仍是 **v0.12.1**。

---

## [v0.12.1 开工 · 第七波] — 2026-10-29(1 个 PHASE + 1 项补做;多 agent 并行)

**本段 = 第七波**：**W-32**（`wlwl check` 两条静态规则 · G-2）+ **G-4①**（模块私有成员 · 实为**证伪结案**），两个 agent 文件面零重叠。计划原文见 `docs/plan/wlwl-v0.12.1-build-plan.md`;本波的地位与交接见该文档 **§13「接替执行手册」**。

### 落地清单

- **W-32 `wlwl check` 静态检查新增两条规则（G-2）**：
  - **规则① 静态发现「宿主挂起点成员出现在任务体之外（含嵌套闭包内）」** ⇒ 复用**既有 `E0014`**（新增 `TypeDiagKind::HostSuspendOutsideTask` + `fixed_code()`，因为码表里**没有 `W0014`**）;消息**逐字抄运行期那段合成逻辑** ⇒ 静态与运行期**逐字相同** ⇒ 规范仓的「失败口径对拍」两边都认得出。
    ⚠ 静态层用 `WalkCtx.depth` **逐条镜像**运行期 `task_body_depth` 的三态（`None` 粘住 / `Some(0)` 合法 / `Some(k>=1)` 报）⇒ **`SCOPE` 体在任务体内时深度为 1 ⇒ 报 `E0014`**（与运行期一致）。
    ⚠ **时钟气泡豁免已实现**：静态层引入 `WalkCtx::in_bubble`，在 `TEST_BUBBLE` 的体里置位并**沿调用链传递** ⇒ 气泡内的 `SLEEP_ASYNC` **不误报**（两条单元测试双向钉住）。
  - **规则② 静态发现「跨任务共享可变绑定」** ⇒ **无 E/W 码**的规范符合性提示，稳定名 `shared-mutable-binding`，**不改退出码**，LSP 侧 `severity: Warning` ⇒ **规则侧零改动、零新码**。
    ⚠ **单一收集函数**（语言册 `:1681` 的硬约束）：`collect_static_diagnostics` 的返回类型扩成 `StaticDiagnostics { diags, notices }` ⇒ **7 处调用点一次改完**（`lsp.rs` 两处 + `main.rs` 五处）⇒ **编辑器里的划线不会比命令行少**。
  - 验收：`wlwl-types` **144/0** · `lsp_smoke` 2/0 · `suspend_site_contract` 13/0 · `spec_r2_conformance` 11/0 · probe **3/3（195 用例）**;**三条反向守卫全部如实变红**（① 去掉规则①判定 ② 去掉规则②判定 ③ **把收集函数拆成两份** ⇒ LSP 守卫单测红 ⇒ 守住「编辑器不报、CI 报」的分裂）。
  - ⚠ **一处取舍（已按 agent 建议采纳）**：静态 `E0014` 在 **`wlwl check` 拦退出码**，在 **`wlwl run` 不报不拦** —— 理由：既有探针 `p07`/`p09` 钉死了「跑了一半 + 运行期报同一条 E0014 + exit 1」⇒ 若静态层在 `run` 里拦，程序根本不会跑 ⇒ 那两条必红;而运行期本来就会报**逐字相同**的那条。
- **G-4「模块私有成员」—— 证伪结案，语义零改动**：⚠ **计划书里这一项的描述已过期**。实测 6 个场景（有无 `.wll.sig` / 有无 `SEALED` / 消费方越界导入）证明：**私有成员**（既不 `EXPORT`、也不在签名里声明的根作用域绑定）**本来就允许且零诊断** —— `check_exports` 的两个方向都只拿 `EXPORT` 名单与声明面对照，私有成员**两边都不在** ⇒ 不触发。⇒ **不需要 `private` 修饰符、不需要模块级 `CONST`、不需要新诊断码**。
  ⇒ 交付改为：**把这条语义写进模块文档**（此前全仓没有任何地方写下来，读代码的人只能反推）+ **2 条回归守卫**（`sig.rs` +90 行，**零语义变化**）;并用两次临时 mutation 证明新用例**咬得住**。
  ⇒ **未新增 probe 用例**（既有的 `D1_sig_match` / `D5_no_sig_is_v09` / `E1_sealed_clean` / `D3_sig_extra_export` 已整面覆盖）⇒ 不占 `EXPECTED_CASE_COUNT` 名额。

### 集成者动作（本波）

- `EXPECTED_CASE_COUNT` **194 → 195**（W-32 的 `P121_g2_static_rules`）⇒ probe 套件恢复全绿（**195 用例**）。
- **诊断文案登记（规范侧）**：规则①**不新增措辞**（逐字沿用运行期文本）但**新增了一个发射点（静态层）**;规则②是**新增措辞**（无码提示）⇒ 不进附录 A、不进总纲 §0.9。
- 本波**未新增任何公共面成员** ⇒ 待规范侧落表总数仍是 **36**。

---

## [v0.12.1 开工 · 第六波] — 2026-10-28(2 个 PHASE 落地;多 agent 并行)

**本段 = 第六波的 2 个 PHASE**（计划 §13.4「再随后」里的 **W-33 / W-30**，文件面零重叠 ⇒ 真并行 2）。计划原文见 `docs/plan/wlwl-v0.12.1-build-plan.md`;本波的地位与交接见该文档 **§13「接替执行手册」**。

### 落地清单（2 项）

- **W-33 诊断可读性 + 模块组织（F-4 / G-4 的一部分）**：
  - **诊断输出补「源码行 + 列号高亮」**（`cli/src/diag_output.rs`）：在既有形状**之上**插入一行 `^`;⚠ **列号是字节列不是字符列**（词法器每字节 `col += 1`）⇒ 高亮先做字节→字符折算，中文行里指针才不会指偏;`col_end` 是**开区间**右端 ⇒ 宽度 = `col_end - col`。
  - **把「修法提示」提到人形态**：实现侧早就为每个码写好了修法（`suggestion_code`），但**人形态一直不渲染它** ⇒ 终端里一条都看不到 ⇒ 现在上浮为 `= hint:` 行，**措辞沿用实现侧原文**，不另造词。
  - **模块内诊断此前没有源码行** ⇒ 补上 ⇒ 模块内运行期诊断与 `IMPORT` 也有行号 + 高亮（原先只有 `<module>:0:0`）。
  - **循环导入诊断**：`E0041` 的**位置**从 `<module>:0:0` 改成**闭合环那条 `IMPORT`** 的 `file:line:col`，并补明确文案与拆环建议 ⇒ **没有新增任何诊断码**，也没改任何既有码的含义。
  - ⚠ **json / jsonl 两行未动**（机器读路径）⇒ 除循环导入那一条（文案与位置按要求变了）外**逐字节相同**;退出码表一字未动。
  - 验收：`P121_f4_g4_diag_module` 通过;`wlwl-cli` 全绿（除计数守卫）;`wlwl-error` 65/0;`parser_syntax_contract` 20/0;反向守卫（关掉高亮 ⇒ 新用例必红）已做。
  - ⚠ **G-4 的另外三项本轮做不了**（**文件归属在别人处**，已登记为待办）：模块私有成员（`wlwl-types/src/sig.rs`）· `.wll.sig` 工作流简化（`cli/tooling.rs` + `cli/main.rs`）· 模块级常量与初始化代码（**新语言特性** ⇒ 要动 `wlwl-parser` / `wlwl-ast` + 规范侧条款）。
- **W-30 `wlwl tasks` 任务检视 + 最小 REPL（F-3 / G-3 的一部分）**：
  - **任务快照**（`eval/src/runtime.rs`）：挂起点（**穷举 `YieldReason` 全部 7 个变体**）· 已运行时长 · 任务树;快照类型是**纯数据**（无 `Rc`、无求值器句柄）⇒ **结构上**跨线程可传。
  - **「已运行时长」需要新加字段**：`Task` 此前**没有**启动时刻字段 ⇒ 新加 `started_at_ms`（**绝对单调毫秒**，与 `deadline_ms` 同域 ⇒ **不用墙钟**）;填在**唯一构造器** `new_pending` 里 ⇒ **不可能漏填**。
  - **只读检视端点**：`wlwl run --inspect` / `wlwl repl --serve` 在 `127.0.0.1` 起一个 JSON 行端点，`wlwl tasks --addr` 拉取。**安全边界三条**：**只绑回环**（`0.0.0.0` 实测被拒）· **只读**（op 白名单只含 `tasks`）· **无执行能力**（端点线程**拿不到求值器** ⇒ 结构性保证，不靠约定）。**零新增依赖**（手写 `std::net::TcpListener`）。
  - **最小 REPL**：`wlwl repl` 能跑表达式、打印当前任务挂起点（**只读**：不支持改状态）;会话**共用一个 `Evaluator`** ⇒ **变量跨次保留**。
  - ⚠ **两条实测发现（改写了原计划的前提）**：① **`SPAWN` 必须在 `SCOPE` 内**（`E0058`）⇒ 作用域退出会**硬取消**未结束的子任务 ⇒ **程序跑完后不可能观察到 `suspended`** ⇒ 「任务挂在哪个挂起点」本质是**运行期活读数** ⇒ 端点因此在 **11 个状态迁移点**发布活快照;② **Windows 上第二次 bind 同一端口不报错**（`SO_REUSEADDR` 语义 = 后绑者**抢**端口）⇒ 只看 bind 错误**抓不到**端口占用 ⇒ 加了**绑前探测**。
  - 验收：`P121_f3_g3_inspect_repl` 通过;`wlwl-cli` 全绿（除计数守卫）;`wlwl-eval --lib` 920/0;反向守卫两条（单测层 + probe 层）都红过。

### 集成者动作（本波）

- `EXPECTED_CASE_COUNT` **192 → 194**（两个 agent 各 +1）⇒ probe 套件恢复全绿（**194 用例**）。
- **诊断文案差异 1 条**（规范侧语言册措辞登记）：`E0041` 的消息补了「模块加载中不能再被加载」的说明（**含义未变**，条件仍是「IMPORT 成环」）;位置从 `<module>:0:0` 改成闭合环那条 `IMPORT` 的 `file:line:col`。
- 本波**未新增任何公共面成员**（CLI 子命令不是公共面）⇒ 待规范侧落表总数仍是 **36**。

---

## [v0.12.1 开工 · 第五波] — 2026-10-28(2 个 PHASE 落地;多 agent 并行)

**本段 = 第五波的 2 个 PHASE**（计划 §13.4「随后批」里本轮做的两项 —— **W-15 / W-29**，文件面零重叠 ⇒ 真并行 2）。计划原文见 `docs/plan/wlwl-v0.12.1-build-plan.md`;本波的地位与交接见该文档 **§13「接替执行手册」**。

### 落地清单（2 项）

- **W-15 B-6 修兜底臂 + B-7 优雅停机（+ W-38 诊断化）**：
  - **B-6**：`wlwl-eval/src/lib.rs` 的 `run_one_task` **兜底臂**改成按「**是不是 `Signal::Yield(_)`**」判（**不按具体变体枚举** ⇒ 将来新增 `YieldReason` 变体也会被抓住），判据与处置抽成 `runtime::classify_task_completion` ⇒ **丢失的挂起信号不再伪装成「正常返回 NULL」**。⚠ 计划的原始描述（「非四种 park 信号」）**已过期** —— 现在有**五**种 park 信号分支（第四波新增 `AsyncReady`），这正是本项要防的那类隐患。
  - **B-7**：**新命名空间 `std.signal`**（`impl/crates/wlwl-std/src/signal.rs`，408 行）—— `ON_SIGNAL(sig, handler)` + `STOP()`;Unix 用 `libc::signal`（`libc` 已是直接依赖 ⇒ **零新增依赖**）、Windows 手写 `extern "system" { SetConsoleCtrlHandler }`;⚠ **信号处理器里只碰原子量**（`Value` 是 `Rc` 不是 `Send`）⇒ OS 事件先落成待处理位，由**主线程**在安全点取走跑用户 handler;停机序列 = **停止 accept → 等在途请求（可配上限时）→ 落盘 → 退出**。
  - **W-38**：`TCP_SERVE` 的 handler 里调宿主挂起点成员 ⇒ 原先是**静默作废**（无诊断、stderr 全空、exit 0、不计入返回计数）⇒ 现在**原生帧回调进 WLWL 代码时清掉「在任务体内」的标志** ⇒ 走 W-10 的**既有**判据 ⇒ 报**既有 `E0014`**（指名成员 + 指向调用点）;`TCP_SERVE` 的既有可观察语义**未变**（误用的连接仍不算被服务、失败仍只记一笔继续下一条）。
  - 验收 `b6_task_result_contract` **7/0** · `b7_graceful_stop_contract` **10/0** · `w38_native_frame_suspend_contract` **11/0**。
- **W-29 H-5 / H-6 别名**：
  - `std.regex::RE_MATCHES_FULL` = `RE_TEST` 的别名（**整串锚定**语义逐字复用 ⇒ WAF 规则不会再因名字误解而静默漏防）;`RE_TEST` 本体**一字未改**。
  - 全局内建 `ARRAY_GET` = `INDEX_GET` 的别名（**同一个函数指针** ⇒ 0-based / `E0031` / `E0036` / `E0037` 整套语义**不重实现**、不漂移）。
  - **错误信息里提示正确 API**（H-6 的另一半）：`AT_K` 的 `E0030` 文案加「取数组元素请用 `ARRAY_GET`」;`INDEX_GET` 的 `E0036` 文案加「下标是 0-based，`INDEX(arr, x)` 返回 1-based 位置」⇒ **只改消息文本，诊断码一律不动、不新增码**。
  - 验收 `h5_h6_alias_contract` **18/0**;`wlwl-eval --lib` **915/0**（含 registry 的双向锁测试）;附录 G 已**重生成**（`docs/appendix_G.md`）。

### ⚠ 本波的两处「集成者修正」（agent 交付的洞，已修并记录）

- **`w38_native_frame_suspend_contract` 曾挂死**：agent 用 `SERVER.replace("PORT", &port)` ⇒ 把 **`IMPORT` 里的 "PORT"** 也替换了 ⇒ 服务端程序第一行就 `E0020: undefined name 'IM<port>'` ⇒ **端口从未被 listen** ⇒ 客户端永远连不上 ⇒ 测试**挂死**（不是断言失败）。⇒ 改用 `__PORT__` 这种源码里不会出现的占位符 ⇒ 现在 **11/0、5 秒**。
- **W-15 的两个 probe 夹具（`P121_b6_task_result` / `P121_b7_graceful_stop`）未做** ⇒ 本波**未新增** probe case 目录 ⇒ `EXPECTED_CASE_COUNT` 保持 192;同等语义已由两个**契约测试**覆盖（7/0 + 10/0）⇒ 已登记为待办（下一批补）。

### 集成者动作（本波）

- **新增 4 个公共面成员**（`std.regex::RE_MATCHES_FULL` · 全局内建 `ARRAY_GET` · `std.signal::ON_SIGNAL` · `std.signal::STOP`）+ **1 个新命名空间**（`std.signal`）⇒ **待规范侧落表总数 32 → 36**。
- **附录 G 已重生成**（`docs/appendix_G.md`）;⚠ 它锁的是**规范抄件** ⇒ `spec_appendix_g_sync` 仍红（新增的 `ARRAY_GET` 不在抄件里）⇒ **「规范侧未跟上」**。
- ⚠ **两个 agent 都动了 `wlwl-eval/src/lib.rs`** ⇒ 派工时做了**严格分区**（W-15：run_one_task 区 + 原生帧区；W-29：builtin 区 + 分派表）⇒ 集成后已**逐 hunk 核验无越界**。

---

## [v0.12.1 开工 · 第四波] — 2026-10-28(2 个 PHASE 落地;多 agent 并行)

**本段 = 第四波的 2 个 PHASE**（计划 §13.4「随后批」的前两项 —— **W-11 / W-14**，文件面零重叠 ⇒ 真并行 2）。计划原文见 `docs/plan/wlwl-v0.12.1-build-plan.md`;本波的地位与交接见该文档 **§13「接替执行手册」**。

### 落地清单（2 项）

- **W-11 B-2 惊群修根因：唤醒一个而不是唤醒全部**：`wlwl-eval/src/runtime.rs` 的 `wake_socket_waiter` 改成**只唤醒 FIFO 队首一个**，其余等待者**留在名单里**继续等下一次就绪事件;`park_for_socket` 加**去重**（同一任务在同一 socket 上至多登记一次 ⇒ 否则「唤醒一个」会变成「同一任务被唤醒两次」）;`wlwl-std/src/tcp.rs` 侧**把就绪电平重新推高**（⚠ 解释器侧取走就绪事件时已经 `retire_ready` 把电平抹掉了 ⇒ 不推高的话队列里剩下的连接永远等不到它的等待者）。新增 `impl/crates/wlwl-eval/tests/b2_accept_wakeup_contract.rs`（4 条）+ 新并发夹具 `impl/tests/concurrency/tcp_accept_single_wakeup.wll`。验收 **4/0**;并发套件 **19/0**;`socket_wait_contract` 16/0 · `tcp_contract` 23/0 · `suspend_site_contract` 13/0 全绿。
  ⚠ **既有语义一条未改**：`TCP_ACCEPT` 的超时仍是 `NULL`;**「同一个 socket 可以有多个等待者」**（审计 N4：两个任务读同一条连接是合法的）**不变** —— 变的是**唤醒几个**，不是**允许挂几个**。
  ✅ **规范侧零改动**（计划 §1.4 的 **SP-2 因此作废**：竞争不再产生 `NULL` ⇒ 规范文本没覆盖的那处不再发生）。
- **W-14 B-5 非阻塞出站：新增 `HTTP_REQUEST_ASYNC` / `PROCESS_RUN_ASYNC`**：`wlwl-value` 加 `YieldReason::AsyncReady(i64)` + `Tag::AsyncReady`;`wlwl-std/src/net.rs` 与 `process.rs` 各加一个 async 成员（**阻塞的 `HTTP_*` / `PROCESS_RUN` 一个字节未改**）;求值器侧加**独立的** `async_waiters` 表（不碰 `socket_waiters`）⇒ `park_for_async` / `wake_async_waiter` / 就绪泵 / `expire_deadlines` 与 `cancel_suspended_task` 的 `AsyncReady` 分支。验收 **12/0**。
  ⚠ **两个成员**都走一致性册 §3.2 修订版本通道登记（引用 D-13 先例）⇒ **规范仓落表前，附录 A 对拍会红**（见下）。

### ⚠ 一个必须翻转的探针（`p12`）

`p12-acceptor-pool-wakeall.wll` 原本是「**钉住惊群存在**」的（B-2 的主证据，断言 `losers-woke-immediately= TRUE`）。W-11 修掉惊群后 ⇒ **该断言翻转**（`winners` 从 1 变 2、`loser-elapsed` 从 ≈150 ms 变 ≈2000 ms ⇒ 落选者**等满自己的超时**而不是被连接唤醒）⇒ 探针**已改写为「钉住惊群已修 + 落选者能继续接受后续连接」的正面证据**（3 个 acceptor + **2 条**连接 ⇒ **两个 acceptor 各拿到一条** ⇒ `winners= 2`）。两条读数各自钉住：B-2 的**正面**证据 + 惊群的**指纹** ⇒ **任何一条变回去都必红**。⚠ 文件名里的 `wakeall` 是**历史名**（同 `p13` 的 `chain`）;探针耗时从 8.5 s 降到 2.1 s。
⚠ 末尾那条 `TCP_RECV(NULL)` ⇒ `E0030` ⇒ **exit 1 仍是期望值** —— 它钉的**不是**惊群，而是**规范空白 SP-2**（「超时产生的 NULL」与「竞争失败产生的 NULL」同形，调用方分不出来）。

### 集成者动作（本波）

- 本波**新增 2 个公共面成员**（`HTTP_REQUEST_ASYNC` / `PROCESS_RUN_ASYNC`）⇒ **登记清单**里的 ①`wlwl-std/src/lib.rs`（**无需改**：新成员随模块 `SPEC` 自动登记）· ②`stdlib_mirror.rs`（**无需改**）· ③各 `*_contract.rs` 的期望名单 · ④`docs/wlwl-spec-reference.md` 的附录 A 抄件 —— **③④ 本轮不做**，因为它们**必须与规范侧落表成对**（§11.2 的顺序：规范仓 ①②③ → 实现仓 ④）;③ 现在改反而会把红消息从「§19 table missed `PROCESS_RUN_ASYNC`」（**诚实**）换成「§19 table found …」（**误导**）。
- 红项：**7 处**（6 个 cargo test 目标 + `check-appendix-a`），**全部是「规范侧未跟上」这一类** —— 第三波是 6 处，本波 **+1**（`process_contract` 因新增成员转红）;⚠ 另有 `process_contract` 的**两条超时用例是负载敏感的既有 flake**（同一份代码两次跑出 1 红 / 2 红）⇒ **不是回归**。

---

## [v0.12.1 开工 · 第三波] — 2026-10-28(3 个 PHASE 落地;多 agent 并行)

**本段 = 第三波的 3 个 PHASE**（计划 §13.4「优先批」三项 —— 业主 2026-10-28 12:36 裁决 **D2 / D3 / D4a** 的落地）。计划原文见 `docs/plan/wlwl-v0.12.1-build-plan.md`;本波的地位与交接见该文档 **§13「接替执行手册」**。

### 落地清单（3 项）

- **W-36 `std.http` 重复 `Content-Length` 收紧（裁决 D2）**：`wl/std/http.wll` 的 `_CL_INT` 改为「**同名多值一律拒**（即便值相同）」，与 `std.net` 同口径（`net.rs` 的客户端与服务端都是「即便值相同也拒」——「同一个头出现两次」本身就是走私信号，不同实现取第一个或最后一个会分叉）。验收 **15/0**（断言 37 → 40）;反向守卫「改回『值相同也接受』⇒ 新增用例必红」。
  ⚠ **只改新模块** ⇒ `std.net` 既有成员的语义**一个字节未动** ⇒ v1.0.4 仍成立（零版本风险）。
  ⚠ **唯一可观察变化**：`std.http` 解析失败时的 `reason` 文案 —— 原「`invalid or conflicting content-length`」拆成两条：`duplicate Content-Length header`（重复）/ `invalid content-length value`（值非法）。`_CL_INT` 的返回形由整数哨兵改为 DICT（**内部助手，未导出** ⇒ 成员面不变）。
- **W-35 `p10` / `p11` / `p13` 改写成 `TCP_SERVE` 形状（裁决 D3）**：三个探针从「手写 accept 链 / 嵌套闭包」迁到 `TCP_SERVE`，**正面读数恢复** —— p10「8 并发全 200 + 5 个安全响应头 + `Content-Length` 按字节」· p11「403×3 + 200×1 + 先解码再匹配的反例证据」· p13「**一个**接受循环连续服务 4 条全 200」（= 需求清单 B-3 要的「可推进的 accept 循环」正解，不再是静态展开的 4 段链）。探针套件 **3/3（192 用例）全绿**;三对副本 SHA256 逐字一致。
  ⚠ **代价（需求方须知）**：`TCP_SERVE` 阻塞整个运行时 ⇒ 同进程客户端必然超时 ⇒ 「全 200」是**服务端一侧**的断言（三个探针各新增一条 `client-no-response-count=` 把这条偏差钉成机器可验读数）。
  ⚠ 三个探针的头部注释已重写为**给需求方的迁移指引**（含「handler 只返回 DICT、不得碰挂起点成员」的形状说明）。
- **W-37 解析器 `E0011`「误报」—— **证伪完成**（裁决 D4a）**：**缺陷不存在**。计划里记的「最小复现」（多行 `LET(f, FUN((conn), (FOO(conn, CAT(head, body)); "200")));`）**多行与单行都解析通过**;计划记的 `E0011: expected ')', got Semicolon` 可由**同一份程序少一个右括号**精确复现 ⇒ 那是**正确诊断**，不是误报。扫了 **79 个合法形态**（排版 / 块位置 / 块终结符 / 嵌套层数 / `;` 省略 / 控制流关键字位置 / 模块声明位置 / `LET` 变体）⇒ **0 个被误拒**;反向侧扫「逐个删掉任意一个 `)`」× 37 个落点 ⇒ 全部正确分类（33 处 `E0011` / 4 处 `E0010`）。
  ⚠ **解析器源码零改动**;新增 `impl/crates/wlwl-parser/tests/e0011_false_positive_contract.rs` **7 条**契约（合法形态必须通过 + 少一个括号仍必须报 `E0011`）⇒ `wlwl-parser` **208/0** 全绿。**该工作项以「证伪 + 钉回归」结案**（计划 §5.5：被证实不可行 ⇒ 写明实测证据，不许改成「已修」）。

### ⚠ 两条工程发现（下一轮照抄）

- **过期二进制会给出完全误导的读数**：本波开工时 `target/release/wlwl.exe` 构建于 07:49，而 `wl/std/str.wll` 于 08:05 被 W-28 改过 ⇒ **旧二进制读新模块源码** ⇒ 报 `E0013` 且位置指向 `wlwl:std.str:281:45` ⇒ **合法程序被误拒**（差一点被当成解析器缺陷）。⇒ **任何解析器 / 诊断面结论，先确认二进制新鲜度**。
- **`TCP_SERVE` 的 handler 里调宿主挂起点成员 ⇒ 那一条连接被**静默作废**（无诊断、stderr 全空、程序 exit 0、该连接不计入 `TCP_SERVE` 的返回计数）**。已登记为 **W-38**（本波**不修** —— 它要动的是「原生帧内是否算任务体」这条语义判据，需要单独裁决）。

### 集成者动作（本波）

- 本波**未新增**任何公共面成员 / 命名空间 / 诊断码 ⇒ **无需登记**（`LANG_SOURCES` / `NATIVE_SPECS` / `resolve()` / `NAMESPACE_META` 全部未动）。
- `EXPECTED_CASE_COUNT` 保持 **192**（三个 case 是**改写**，不是新增）。
- 门禁红项**全部**落在「规范侧未跟上」这一类（`check-appendix-a` / `stdlib_appendix_a_sync` / `fs_contract` 等成员表）⇒ 修法见计划 §11.2 五步 —— **不是本波引入**。

---

## [v0.12.1 开工 · 第二波] — 2026-10-28(5 个 PHASE 落地;多 agent 并行)

**本段 = 第二波的 5 个 PHASE。** 计划原文见 `docs/plan/wlwl-v0.12.1-build-plan.md`;本波的地位与交接见该文档 **§11「收尾交接」**。

### 落地清单（5 项）

- **W-22 TLS 扩展 crate**：`impl/ext/wlwl-tls-ext/`（**独立 crate + 自带 `Cargo.lock`**;主仓依赖面**零变化**）。`tls` 10 员 + `crypto` 8 员;手写 DER/X.509 解析;默认信任根用 `webpki-roots`（真 CA 校验，不是空信任）。验收 **32/0**;宿主联调通过（`--ext` 加载 + 两种 IMPORT 形态）;三条反向守卫都红过。
  ⚠ 已知限制：**阻塞语义**（ABI v1 无就绪通道 ⇒ 握手/accept/读写阻塞调用线程;异步形态需 **ABI v1.1**）。
- **W-10 B-1 嵌套挂起点诊断**：`wlwl-eval/src/lib.rs` 新增 `task_body_depth` + 四个闭包调用点的帧维护;判定是**「被调用时」**（不是「真挂起了才报」）—— 后者会让探针时绿时红（竞态）。验收 **13/0**;反向守卫「去掉判定 ⇒ 恰好那 6 条红、7 条合法用例仍绿」。
  ⚠ 通道族不归 `E0014`（仍是 `E0053`）;仅「会等」的那几个成员被判。
- **W-16 `std.http` 骨架**：`wl/std/http.wll`（R1 纯 wlwl,494 行,22 个助手）。验收 **15/0**;反向守卫两条。
  ⚠ **framing 规则已按 RFC 9112 §6.3 改正**（请求既无 TE 也无 CL ⇒ **体长为 0**，不是拒绝）—— 原实现把**响应侧**规则套到了请求侧。
- **W-13 `std.sync` 原子共享容器**：句柄表**不存 `Value`**（用 `Send` 安全的镜像树）。验收 **21/0**;反向守卫 `3998 ≠ 4000`。
- **W-28 `QUERY_PARSE` + `FORMAT_NAMED`**：`QUERY_PARSE` 复用既有 `URL_DECODE`;新增 `FORMAT_NAMED`，**既有 `FORMAT` 的错误消息逐字未变**。验收 **14/0 + 11/0**;反向守卫 `1/{b}`。

### 后续裁决（2026-10-28 12:36，四项**全部已定**；**本轮只修订文档，未执行**）

- **D1** `tls::LOCAL_ADDR` **保留** ⇒ 扩展的 `tls` 命名空间按 **10** 个成员登记（只进一致性册 §5.5 台账，**不进附录 A**）。
- **D2** 重复 `Content-Length`（值相同）：**`std.http` 向 `std.net` 收紧**（值相同也拒）⇒ 只改**新**模块 ⇒ **零版本风险**（v1.0.4 仍成立）；「放宽」的口径留到 v1.1.0 再议。⇒ 工作项 **W-36**。
- **D3** `p10` / `p11` / `p13` 三个探针**改写成 `TCP_SERVE` 形状** ⇒ 恢复「8 并发全 200 / 403×3 / 4 请求全 200」的**正面证据**，并顺带成为需求方的迁移指引。⇒ 工作项 **W-35**。
- **D4a** 解析器的 `E0011` 误报（合法程序被误拒）**本轮修** ⇒ 工作项 **W-37**（静默错误级，优先级高于其它余项）。
- **D4b** NFKC 字符表**保持 Unicode 17.0**，`unicode/waf.rs` 文件头已写明版本与 53 码点差集；`--ucd` 正路已实现 ⇒ **等有 UCD 源数据时重生成**，不占用 v0.12.1。

---

### 集成者动作

- 登记 `std.sync` / `std.http` 两个**新命名空间**（三处静态表 + `NAMESPACE_META` 两行）+ `resolve_format` 期望名单加 `FORMAT_NAMED`。
- `EXPECTED_CASE_COUNT` 保持 192;新增 CI 步骤**构建 TLS 扩展 cdylib**（它在 workspace 之外 ⇒ 默认门禁碰不到它）。
- 新增 30 个公共面成员（见计划 §11.2）⇒ **规范仓落表前**，实现侧的三道对拍闸会红 —— 那是**「规范侧未跟上」**，不是实现缺陷。

### ⚠ 一个必须让业主知道的连带后果

**W-10 把「宿主挂起点成员的调用点必须在任务体内」真正执行了** ⇒ 需求清单里那个「可用架构样板」（模块级 helper 闭包里调 `TCP_ACCEPT`/`TCP_RECV`）现在**结构性违规** ⇒ `p10`/`p11`/`p13` 三个探针的读数从「全 200 / 403×3 / 4 请求全 200」变成了「`E0014`」。
**合规的形状是 `TCP_SERVE`**（accept/recv 都在**原生帧**里，handler 闭包也跑在原生帧内 ⇒ 不受本判据约束）⇒ 需求方**迁移到 `TCP_SERVE`** 即可。

---

## [v0.12.1 开工 · 第一波] — 2026-10-27(7 个工作项落地;多 agent 并行)

**本段 = 第一波实现落地的记录(实现侧)。** 计划原文见
`docs/plan/wlwl-v0.12.1-build-plan.md`;本波的组织方式与未完项见该文档
**§11「收尾交接」**。

### 落地清单(7 项 + W-01 基线)

- **W-01 基线自洽**:三条门禁基线全绿(`check-appendix-a` OK /`stdlib_appendix_a_sync` 8/0 /
  `§5.4 状态列与契约表一致(25 行)`),probe 夹具数 181 = 常量。
- **W-02 探针固化**:`docs/wlwl-feasibility/probes/` 9 个探针 + README 对照表 +
  9 个 `P121_probe_*` 夹具。⚠ **实测把需求清单的 4 处描述订正了**:B-1「静默终止」
  复现不出来(可观察的是**段首重放**)、B-2 死法是 `TCP_RECV(NULL)` ⇒ `E0030`、
  「9 个探针已开源」**不成立**、清单的「全 200」是客户端视角(单进程 wlwl 无法既服务
  又当客户端 ⇒ 断言落在服务端一侧)。⚠ **`docs/wlwl-feasibility/probes/` 已于
  2026-10-30 整目录归档**（见本文件顶部「文档同步 + 探针归档」段）⇒ 原文路径
  **已不存在**，现行位置见 `docs/history/20261030-feasibility-probes-archive.md`
  与 `impl/tests/probe/cases/P121_probe_*/`。
- **W-21 扩展加载器**:新 crate `impl/crates/wlwl-ext/`(ABI v1 + raw FFI 加载器 +
  信任门控 T1–T6 + 运行期注册表)+ `wlwl-value` 的 `ExtRegistry` + eval 侧第 4 层解析 +
  CLI `--ext` / `--ext-sha256`。⚠ **T7 结构上成立**:`wlwl namespace` 与附录 A 都取自
  `wlwl_std::namespace_paths()` 这张**静态**表 ⇒ 运行期扩展**进不去**公共面。
  ⚠ macOS **没有 `libdl`** ⇒ 走 `System` framework(不偏离则 macOS 构建直接断)。
- **W-23 正则编译缓存**:进程级缓存(条数 256 + 字节 4 MiB 双上界、LRU);
  同一模式 1000 次匹配 ⇒ **编译次数 = 1**。**零新成员、零规范改动**(文本册 §16 的
  实现注本就允许缓存)。
- **W-24 归一化 + 路径包含**:`std.text::NFKC`/`CASE_FOLD`/`STRIP_INVISIBLE` +
  `std.fs::PATH_CANONICAL`/`PATH_CONTAINS`(逐段判定,不是字符串前缀)。
  ⚠ **仓内本来没有 NFKC 的数据底座**(`gen-ucd.rs` 跳过了兼容性分解)⇒ 补了生成物
  `unicode/waf.rs` + 生成器 `script/gen_unicode_waf_tables.py`;无网络时用离线权威
  oracle(Node ICU / CPython)交叉校验生成。⚠ 表停在 **Unicode 17.0**(`norm.rs` 在
  18.0.0)⇒ 53 个码点有差异,**待业主裁决是否等 UCD**。
- **W-26 `std.log`**:结构化日志(级别过滤 / JSON 行 / stderr⇄文件)+ 运行时指标
  (原子计数器 / 延迟直方图 / 快照),7 个成员。
- **W-27 原子写 + 文件锁 + 元数据**:`WRITE_FILE_ATOMIC`/`LOCK_FILE`/`UNLOCK_FILE`/
  `FILE_MTIME`/`DIR_ENTRIES`;Windows 侧原子替换走 `MoveFileExW`(⚠ `std::fs::rename`
  在 Windows 上不能覆盖已存在的目标)。
- **W-12 `TCP_SERVE`**(B-3 迭代器形态):accept 循环在**原生帧内**推进
  (accept → 读满一次请求 → 调 handler → 写回 → 下一条);单条连接异常只废那一条。
  ⚠ 已知代价:原生帧里的等待**阻塞整个运行时**(与 `HTTP_*`/`SLEEP` 同类)。

### 门禁状态(收口时)

- ✅ `cargo fmt --all -- --check` 0 diff · probe 套件 3/3(192 个用例)· `wlwl-std` 全绿。
- ⚠ **红的是「规范侧未跟上」,不是实现缺陷**:新成员共 18 个(`std.text` +3 · `std.fs` +7 ·
  `std.net` +1 · `std.log` 新命名空间 7)⇒ 附录 A 抄件 / `stdlib_appendix_a_sync` 的
  成员集合 / `net_contract` 与 `fs_contract` 的成员表**在规范仓落表前必然红**。
  正解顺序见计划 §11.2(规范仓 §5.5 台账行 → 附录 A → 各册成员表 → 抄回实现仓 →
  **最后**才前移 `ci.yml:121` 的规范仓钉死提交)。

### Changed(文档)

- 计划新增 **§11「收尾交接」**:本波落地清单 + 规范侧待做五步 + 下一批 agent 切分 +
  两个待业主裁决项 + 三条工程教训。

---

## [v0.12.1 立项（重做）] — 2026-10-27(迁移需求清单 60 条**全量落地**;**计划阶段**,未动代码)

**本段 = 对外需求清单的逐条裁决结果,不是实现落地。** 计划原文见
`docs/plan/wlwl-v0.12.1-build-plan.md`(57 条需求编号 → **28 个工作项 / 7 个批次 / 4 个波次**);
需求来源见 `docs/plan/wlwl-迁移需求清单.md`(TinyPage 迁移项目提出)。

⚠ **本段取代同日 08:13 的那一版**(commit `198e1ec`,已被 `9bd0f46` 删除)。三处方向变更:

- **Δ1 TLS 从「本轮不做」改为「本轮做」** —— 交付形态 = **Rust 原生 `cdylib`(DLL/SO)**,
  由宿主用 `--ext` **显式授权**加载,落一致性册 §5.5「实现侧扩展台账」(不进公共面 ⇒ 不动附录 A)。
- **Δ2 B-3 / B-4 / B-5 从「移出本轮」改为「本轮做」**,且**不需要 `ADR-0017` Step 3**:
  三条都走需求清单**自己列的候选形态**(迭代器式 / 共享容器+原子更新 / 宿主挂起点式新成员)。
- **Δ3 「进路线图」的 4 项全部落地**(G-3 调试器、F-2 指标、B-3 完整形态、H-6 别名)。

### 裁决分布

- **实现侧落地 44 条** · **口径答复 5 条**(I 组)· **归应用层 2 条**(E-6 Markdown / E-7 高亮
  —— **按需求方自己的建议**:清单原文即「**建议明确不做**」)。
- ⚠ **没有任何一条以「延后到下一版」结案**。

### 规则侧（v1.0.4）三类改动

- **SP-1 修规范自相矛盾**:并发册 §17.0 说宿主挂起点成员在任务体外 ⇒ `E0014` 且**指名该成员**;
  语言册 §11.2 却说 `E0014`「**只**」用于 `YIELD` ⇒ 两册正面矛盾 ⇒ **只修语言册那半边**。
- **SP-2 补规范空白**:多 acceptor 在同一 listener 上的竞争语义无条款覆盖 ⇒ 补「落选者返回 `NULL`
  但**不得**终止任务、**不得**连带关闭监听句柄」。
- **SP-3 + SP-4 + SP-5**:为新成员/新命名空间走登记(§3.2 修订版本通道,**引用 D-13 先例**)+
  给扩展一个合法落点(§5.5 台账第一行)。
- ⚠ **硬约束三条**:**不得新增诊断码**(一致性册 `:129` 把新诊断码放在次版本下 ⇒ 1.0.x 无通道)、
  **不得修改既有成员的可观察语义**(`:223`)、**不得移除或弃用既有成员**(`:224`)
  ⇒ 本轮**零处修改、零新码** ⇒ 规范侧可做 **v1.0.4**（修订版本）而非 v1.1.0。

### 实测校正（照单全收会做无用功）

- **B-6 不成立** ✅ 并发册 §17.9 已定义、且实现**正确** ⇒ 只补契约用例。
- **H-1 已修** ✅ `IF` 第三实参的多语句组**已正常执行** ⇒ 不排批次。
- **H-2 / H-3 已有等价物** ✅ 全局内建 `STR(x)` / `SPLIT(s, sep)` **本就存在** ⇒ 文档/技能点明。
- **H-4 / H-5 / H-6 属实但只能「加别名」** ⇒ `PATH_CANONICAL` / `RE_MATCHES_FULL` / `ARRAY_GET`,
  **不改**既有成员语义(否则撞 `:223` / `:224`)。
- **D-1 零规范改动** ✅ 文本册 `:502` 已写「实现**可以**缓存编译结果」⇒ 缓存**已被允许**
  ⇒ 只需实现侧内部缓存 + 确定性工作量计数断言。
- ⚠ **条数订正** ✅ 清单页眉写「60 条」,按编号实数是 **57 条**(A 5 + B 7 + C 12 + D 7 + E 7 +
  F 4 + G 4 + H 6 + I 5;H / I 两组是表格形态、不在 `###` 标题里)⇒ 上一版计划的台账也只有 57 行。
- ⚠ **B-1 的需求前提被实测推翻** ✅ 用仓内已构建的 release 二进制跑 5 个重构探针:**嵌套闭包里的
  挂起点不静默终止**(可观察的是**段首重放 ⇒ 副作用重复**);但并发册 `:94` 要求的那条检查
  (「调用点必须在任务体内 ⇒ `E0014`」)**根本没写** ⇒ **实现侧今天不合规** ⇒ 工作项改成
  「**新增词法检查**」,验收按**新语义**写(不写「现在会静默死」)。
- ⚠ **B-2 的死法说错了** ✅ 落选者的真实死法不是「静默 NULL」,是调用方 `TCP_RECV(NULL)` ⇒
  **`E0030`**;惊群本身坐实(一个 listener 一个 socket id ⇒ 一把等待者 `Vec` ⇒ 唤醒全部)
  ⇒ 改成「**唤醒一个**」修根因 ⇒ 落选者**不再拿 `NULL`** ⇒ 规范侧大概率**零改动**。
- **B-6** ✅ 规范 §17.9 已覆盖、实现的**三态已具备** ⇒ 需求描述的「裸 NULL 不可区分」不成立;
  **但**兜底臂会把**丢失的挂起信号伪装成正常 NULL** ⇒ 那是**真缺陷**,本轮修它。
- ⚠ **需求清单引用的 9 个探针从未入库**(`docs/wlwl-feasibility/` 不存在)⇒ W-02 固化为仓内夹具。
  ⚠ **2026-10-30 订正**：`docs/wlwl-feasibility/probes/` 那一轮**确实建过**，但因与门禁夹具
  逐字节相同、而门禁只读 `impl/tests/probe/cases/` ⇒ **已整目录归档**为
  `docs/history/20261030-feasibility-probes-archive.md`（**可跑的证据仍在**
  `impl/tests/probe/cases/P121_probe_*/`）。

### 已知偏差（对需求方口径）

- **A-2**:HTTPS 出站的入口是**新成员** `std.http::GET`,**不是** `std.net::HTTP_GET`
  —— 后者的 `https://` ⇒ `ERR` 是**规范要求**(系统册 `:750`),改它违反 `:223`。
- **B-2**:落选者返回 `NULL`(与超时同形),**不新增** `TCP_ACCEPT_SHARED`;
  需求要的「落选者保持存活并能继续接受」**达成**。
- **E-5**:`std.format::FORMAT` 的既有语义**不可扩展** ⇒ 另起 `FORMAT_NAMED`。
- **G-2** ✅ 规则 ① 复用既有 `E0014`;规则 ② 走**「规范符合性提示」**形态(**无 E/W 码**,
  稳定名 `shared-mutable-binding`;`wlwl check` 与 LSP **共用同一个收集函数**、**不阻塞退出码**)
  + 运行期 `ERR(kind:"SharedMutableViolation")` 兜底 ⇒ **规则侧零改动、零新诊断码** ⇒
  **版本纹丝不动**(业主 2026-10-27 裁决「我说 v1.0.4 包含就按我说的」)。
  ⚠ 代价:该提示**不阻塞退出码** ⇒ CI 靠 grep/日志发现,真正挡住违规用法的是运行期拦截。

### 业主裁决（2026-10-27,四项**全部已定**）

- **Q1 TLS** ✅ **动态库扩展**:独立 Rust `cdylib`(DLL/SO)+ 宿主 `--ext` 显式授权加载
  + 一致性册 §5.5 台账第一行 ⇒ 不进 `std.*`、不动附录 A、发行二进制依赖面不变。
  ⚠ 安全前提不可让渡:**语言面不提供 `LOAD` 成员**(否则脚本即任意代码执行)。
- **Q2 并发三项** ✅ 按需求清单**自己列的候选**形态,**不引入 `ADR-0017` Step 3**。
- **Q3 新成员** ✅ 落 `std.*`(动附录 A + 各册成员表 + 总纲 §0.9 一行登记,先例 D-13)。
- **Q4 G-2 规则 ②** ✅ **进 v1.0.4** ⇒ 为此改成**不需要新诊断码**的交付形态(见上)。
- ⇒ **本轮无待决项**,版本口径固定为 **实现 v0.12.1 ↔ 规范 v1.0.4**。

### Changed（文档）

- **新建** `docs/plan/wlwl-v0.12.1-build-plan.md`(本轮计划全文,含 TLS 扩展架构章、
  多 agent 并行编排章、规则侧条款级清单、60 条逐条台账)。
- **`docs/plan/` 的 v0.12.0 三件 + v1.0 对齐审计**整份原文 `git mv` 归档至 `docs/history/`;
  `docs/plan/README.md` 同步更新索引与「文件去哪了」表。
- ⚠ **本段不发 tag** —— v0.12.1 尚未开工,无实现变更。

---

## [v0.12.0 发版前整改批] — 2026-10-26(第三方审计整改;tag 待业主发起)

**本段 = 发版前第三方审计报告的整改落地。** 审计原文见
`docs/history/wlwl-v0.12.0-release-audit-20261026.md`(22 条事实 / 8 个修复批次 /
7 项规范侧待裁决)。⚠ **本轮不发 tag** —— 正式版 tag 由业主发起。

### Fixed — 三条发版阻断

- **附录 A 的逐字对拍门禁从红转绿，并接进 CI**（审计 F-01）。仓内唯一的
  「镜像 vs 实现派生表体」**逐字**对拍工具 `check-appendix-a` **从 D-13 落地起
  就是红的**（3 行差异），而 CI 从未调用过它 ⇒ 这条分叉无人发现。
  三处差异的成因：`std.result` 的层列漏登记 R2 的 `TAKE_ERR`；`std.time` 的成员
  **顺序**与快照不同；`std.str` 的层列与快照不符（见下条）。
  ⇒ 另加一条**逐字**守恒测试（比**全文**，含「层」列与顺序；既有守卫只比成员
  **集合** ⇒ 这正是它能长期潜伏的原因），并把 `check-appendix-a` 接进
  `.github/workflows/ci.yml`（step 名写明「逐字对拍」，与既有的 §5.4 门禁区分）。
  验收：`cd impl && cargo run --bin check-appendix-a` ⇒ **exit 0**。
- **核心内建的实参校验诊断补回调用点位置**（审计 F-04）。`LEN(1)` /
  `INDEX_GET([1], 5)` / `+(1, "a")` 这一族的诊断此前**全部**落在
  `<runtime>:0:0`（实测 17/17 命中）⇒ 语言核心册 §11.1 要求的「位置(文件、行列)
  与源行摘录」双双缺失 ⇒ 新手最高频的一类错误**不可定位**。
  成因：`arity_error` / `type_error` / `builtin_error` 的签名只有 `(name, msg)`，
  拿不到 AST span ⇒ 一律用占位位置。
  ⇒ 改法在**分派点**统一换掉占位位置（分派点**有**调用点 span）⇒ 一处改动覆盖
  全部核心内建，且**已有**真实位置的诊断原样保留。配套契约 26 条
  （`builtin_diag_span_contract.rs`）。
  ⚠ **保真黄金已授权重录**：`v07_fidelity_v0{6,9}_baseline.jsonl` 里 `E1003` 的
  位置从 `<runtime>:0:0` 变成真实调用点 ⇒ 行为面（退出码 / stdout / 码 / 消息）
  **逐字未变**，变的只有「位置」与「多出来的源行摘录」。
- **宿主挂起点成员在任务体外的失败模式**：诊断**指名成员**并**指向调用点**
  （审计 F-03 / F-05）。此前顶层调用 `TCP_ACCEPT` / `SLEEP_ASYNC` 报的是
  「`YIELD` used outside a step context」（用户写的是 `TCP_ACCEPT`，报的是
  `YIELD`），位置还落到**模块第一条语句**；而且它**根本没等**（声明 2000 ms、
  实测 101 ms 就失败）⇒ 系统册 §21.6.2 的「超时 ⇒ `NULL`」在模块顶层不可达。
  ⇒ 消息改成仓内既有的 `MEMBER: 说明` 约定（`TCP_ACCEPT: used outside a task
  body — …`），位置指向调用点；**显式 `YIELD()` 的既有行为一字未动**。

### Fixed — 其余批次

- **嵌套块里省略必需的 `;` ⇒ `E0013`**（审计 F-08）：此前顶层报 `E0013`、嵌套报
  `E0011` ⇒ 同一个错误两处不同码，按码分流的工具拿不到「缺分号」这一档。
- **非模块顶层的 `IMPORT` / `EXPORT` 不再静默**（审计 F-09）：嵌套 `EXPORT`
  此前是「零诊断 + 退出码 0 + 什么都没导出」，跨模块导入时才炸 `E0023`，而错误
  指向**消费方**；嵌套 `IMPORT` 此前则是**被接受并真的执行**（导入 + 绑定）
  ⇒ 同一个 `ModuleDecl` 家族三种行为。⇒ 现在 `IMPORT` / `EXPORT` / `SEALED`
  口径**一致**：都只在模块顶层合法（规范附录 A.2：「`ModuleDecl` 只出现在模块
  顶层，不作为 `Expression`」），诊断同码同措辞（`E0010` + 「only allowed at
  module top level」）。
  ⚠ 随此改动，`wlwl-types` 里那条断言「嵌套 `IMPORT` 能解析」的用例（它的载体
  是**非法程序**）已改写为断言**解析期被拒**。
- **孤儿任务以 `ERR` 值收尾不再无声消失**（审计 F-10 / 规范侧 §17.9）：§8.6.2 的
  三处丢弃点**都不覆盖**「任务终值」⇒ 一个以 `ERR` 结束的 fire-and-forget 任务
  **不留任何痕迹**（服务端「每连接一个任务」正是这种）⇒ 症状与「网络卡了」无法
  区分。⇒ 收尾时按并发册 §17.9 的**报告行形态**报一行（程序继续、退出码仍 0），
  被 `AWAIT` 认领的那一份**摘掉** ⇒ 两条路径互斥、**不报两次**。
- **`CAT` 归 R2 原生内核**（审计 F-02 / 规范侧 S-1 甲案）：冻结快照说它在 R2，
  实现却是 R1 闭包 ⇒ 快照陈述了一件**假事**。⇒ 改为 kernel 注入 + 门面直接改名
  导出（形态与 `std.result` 的 `RESULT_ALL` / `TAKE_ERR` 同款），并把 O(n²) 的
  累加链换成 O(n) 缓冲。⚠ **行为面逐字未变**（实测 R1/R2 读数相同：零段/一段/
  多段/逐段 `STR` 渲染/`ERR` 实参透传/`ERR` 装在容器里）。
- **词法诊断建议文案订正**（审计 F-14）：`E0002` 曾承诺「相邻字符串拼接」这一
  **不存在的语言特性**；`E0001` 的转义表漏 `\$`、数字形态漏指数与十六进制。
- **`zstd` 0.13.3 ⇒ 0.14.3**（`zstd-safe` 7.3.0 ⇒ 8.2.0）：0.13.3 触发两条新
  advisory —— `RUSTSEC-2026-0391`（复用 context 时上一个 stream 的数据泄进下一
  个）与 `RUSTSEC-2026-0398`（`Seekable::decompress` 把 `Vec` 长度写越界）。
  ⇒ 两条的修复版都在 `zstd >= 0.14.3` / `zstd-safe >= 8.2.0` ⇒ **升版本**而不是
  在 `deny.toml` 里加豁免（那份豁免清单只留给「无可升修复版」的情形，见其中
  rustls 的四条）。本仓对 zstd 的 **API 面只有两处**（`zstd::bulk::compress` 与
  `zstd::stream::read::Decoder::new`）⇒ 升级面极小，压缩契约 16/16 全绿。
- **`IS_OK` / `IS_ERR` / `OR_DIE` 不再是关键字**（审计 F-06）：规范 §1.4 的
  关键字表只有 15 个，这三个不在其中 ⇒ 遮蔽内建名本该按 §3.5 报 `E0025`
  （或 `allow_builtin_shadow` 下的 `W0030`），此前却是**语法错 `E0010`** ⇒
  按码分流的工具（IDE / CI 门禁 / agent 自愈规则）会走错分支。
  ⇒ 改走**通用调用路径**（`Expr::Call`），由求值器**按名字**在调用位置解析；
  读模式（§8.6.1 的观察型 / 取走型）与 ERR 令牌语义**逐字未变**
  （`IS_OK` / `IS_ERR` 仍是观察型；`OR_DIE` 仍是「剥 `OK` 外壳 + 携带」那一个）。
  ⚠ **一处行为对齐**：`OR_DIE` 现在与它的规范别名 `UNWRAP_OR` **完全同款**
  —— 默认值在**调用点**求值（此前是 `Expr::OrDie` 分支里的**惰性**求值）⇒
  `OR_DIE(OK(1), ERR("x"))` 这类写法会多出一条 `E0103`（默认值里那份 `ERR`
  随结果转出）—— 这正是 `UNWRAP_OR` 一直以来的行为 ⇒ 别名与本体终于同款。
- **裸 `THIS`（零参引用）实现**（审计 F-07）：对象册 §15.1 明写两种写法都合法，
  此前只支持 `THIS()`，裸写法报 `E0020 undefined name THIS` 且附「did you mean
  one of: HAS, TRIM?」的**误导**提示 ⇒ 现在与 `THIS()` **同款**（方法体内取当前
  接收者、方法体外 `E0032`）。

### Changed — `wlwl-skill`（审计 F-11 … F-22）

- ⚠ **并发「Hard rule 6」原写反了**：它把规范**明令禁止**的共享可变状态
  （跨任务共享 `LET MUT` cell）写成推荐做法，而 agent 读「Hard rules」正是
  把它当权威 ⇒ 改成与 §17.0.1 一致的口径（**不得**跨任务共享可变 cell；正确形态
  是 actor + 通道）。
- ⚠ **把已收敛的 X-06 / X-07 写成「已知缺口」**：一致性册 §5.4 里这两行状态都是
  **已实现** ⇒ agent 照着「会静默丢错」的描述写代码，反而制造误判。
- ⚠ **`std.fs` 段落的尾部**仍说三个冻结成员用 `E0060`–`E0062`（该码早已退役）
  ⇒ 照它写的捕获分支是**死代码**。
- ⚠ `SCOPE` 的「不等孙任务」旧结论（§28.6 已推翻）、孤儿任务失败报告正文零覆盖、
  「五处语义变更」三套口径且漏 D-16/D-17、成员面 `214` 应为 **231**、
  `concurrency_cancel.wll` 的期望输出多写一行、`std.encode` 导语按 8 个成员写
  （实为 16）—— 逐条订正，并加一条**防回归守卫**（关键字符串重新出现即红）。

### 规范侧同步（`Laffinty/wlwl-spec-v1`）

- **§8.6.2** 删掉「切段任务体路径不报 `E0103`」这条**已过时**的实现注（D-15 已落地）。
- **§1.3** 标识符 EBNF 补 `_`（首字符可以是 `_`，与「`_` 是通配符」的散文一致）。
- **§1.6 / §1.8 / 附录 A.2** 登记**十六进制字面量**（`0x1F`）与**字节串字面量**
  （`b"AB"` / `b"\x41"`）—— 它们已是实现的事实面 ⇒ 登记进规范而不是只留在实现里。
- **§12** 澄清「保留集合为空」与附录 A.2 的 8 个引号终结符**不矛盾**（它们全都有
  定义语义 ⇒ 是「被定义」而不是「保留但未定义」）。
- **附录 A.2 注②** 改成与 §2.6 一致（尖括号与箭头形式的**命运不同**：多参数形式是
  响亮的语法错，单参数形式与箭头形式被**静默吸收**）。
- **并发册 §17.0** 补「宿主挂起点成员的调用点必须在任务体内」；**§17.9** 把「无人
  `AWAIT` 的用户 `ERR` 值」从「交给 §8.6 的丢弃点」改成「走本节的报告路径」。
- **系统册 §21.5 / §15.1** 的失败列补「调用点不在任务体内 ⇒ `E0014`」，§21.6.2
  补上这条前置条件（⇒ 「超时 ⇒ 正常值」在任务体外不可达，是**规则的后果**而不是
  成员缺陷）。

⇒ 规范侧检查器读数：`python script/check_spec.py --impl <wlwl.exe>` ⇒
**错误 0 / 提示 0 / 失败口径对拍缺口 0**（223 成员、23 条 (成员,码) 对**全部钉住**）。

## [v0.12.0-rc3] — 2026-10-25(预览版)

**v0.12.0 的第三个预览版。** 内容 = `[0.12.0]` 段 **+ rc2 的四条传输层等待修复**
**+ 本段的一条补漏**（恢复提示覆盖「每一次从挂起恢复」）**+ `wlwl-skill` 的两处订正
与五条新写法约束**。

### Fixed

- **恢复提示覆盖「每一次从挂起恢复」** —— rc2 的恢复提示**只在 socket 就绪唤醒时**
  置位，而**截止时间到期**、**定时器到点**、**通道对端**这三种唤醒**不置**
  ⇒ 段首重放时 `TCP_ACCEPT` 不查它的备忘 ⇒ 弹出空队列 ⇒ 返回 `NULL` ⇒
  `TCP_RECV(NULL)` ⇒ **`E0030`** ⇒ 任务死掉、连接句柄丢失、客户端收到 reset
  ⇒ **规范要求的超时形态 `{bytes: [], text: "", eof: FALSE}` 在该形态下不可达**。
  ⇒ 现改为每一种恢复都置提示（挂起点不是 socket 时用哨兵 `RESUME_NON_SOCKET`）。
  ⇒ **第三方二期复测实测发现**（`REPORT2.md` §4.3）。

### Changed — `wlwl-skill`

- ⚠ **第 7 条纪律（`SCOPE` 与孙任务）原写错了**：实测证明作用域退出是**先排干
  （等完）再强制取消** ⇒ 挂在 socket 上的孙任务**被等完** ⇒ 客户端收到完整响应。
- ⚠ **第 8 条纪律（`AWAIT` 已结束句柄）原写错了**：实测证明它**返回终值**，
  `E0053` 只在「不是句柄」或「句柄已失效（generation 不匹配）」时出现。
- **新增「五条写法约束」**：其中两条是**静默失败级**的坑。

⚠ **本轮不发正式版 tag** —— 正式版的 tag 由业主发起。

基版本 `v0.12.0`（与 `impl/Cargo.toml` 一致）⇒ RC 与正式版同一份代码，
GitHub 上标 **pre-release**。

## [v0.12.0-rc2] — 2026-10-25(预览版)

**v0.12.0 的第二个预览版。** 内容 = 下方 `[0.12.0]` 段 **+ 本轮的四条传输层等待
修复**（重放中的 `TCP_ACCEPT` 不再再次挂起 · 就绪等待改事件驱动 · 一个 socket 可有
多个等待者 · `TCP_RECV`/`TCP_ACCEPT` 先看货再问超时）**+ `wlwl-skill` 补上 v0.12.0**
（17 个新公共面成员的登记、`reference.md` §28 增量备忘、`SKILL.md` 的「Network
programming」八条纪律、`examples/socket_server.wll`）。

⚠ **RC 与正式版是同一份代码** —— 基版本 `v0.12.0` ＝ `impl/Cargo.toml` 的
`[workspace.package] version`（`version-check` job 按「基版本」比，对，
`v0.12.0-rc2` 与 `v0.12.0_rc1` 同一口径）⇒ 二进制报的是 `wlwl 0.12.0`，
而 tag 是 `v0.12.0-rc2` ⇒ 这是**预览版**，GitHub 上标为 **pre-release**，
不会被工具链当成 production ready 抓走。

⚠ **本轮不发正式版 tag** —— 正式版的 tag 由业主发起。

本轮读数：全量 `cargo test --workspace` **0 failed**；`cargo fmt --check`
**0 diff**；`clippy -D warnings` **0**；第三方那份 HTTP 服务器在新构建上
**6/6 路由 200/404**、`/utf8` = 45 字节。

⇒ 逐条判据（每条带证伪命令）见 `docs/history/wlwl-v0.12.0-rc1-audit-and-fixes.md`。

## [v0.12.0_rc1] — 2026-10-25(预览版)

**本段记录的是 `v0.12.0_rc1` 那个快照** —— RC 与正式版**在 RC 发布时**是**同一份代码**
（基版本相同即合法；见 `.github/workflows/release.yml` 的 `version-check` job：
RC tag 比的是**基版本**，`v0.12.0_rc1` ⇒ 基版本 `v0.12.0` ＝ `Cargo.toml` 的版本）。
⚠ **此后（2026-10-25 收尾批）又动了**：规范侧 **v1.0.2 的 tag 已发**；实现侧把
§5.4 接线表的 **X-14** 转成 `no-contract`、补了 **4 条** V-04「宿主挂起」契约用例
（`scheduling_semantics.rs` 17 ⇒ **21** 条）、重建了合并册抄件、CI 的规范仓钉死提交
前移到 `294dc07` ⇒ ⇒ **正式版的读数见下方 `[0.12.0]` 段**（契约测试 **2995 例全绿**）。

⚠ **这是 RC,不是正式版**：本快照的二进制报的是 `wlwl 0.12.0`,而发布 tag 是
`v0.12.0_rc1` ⇒ GitHub 上标记为 **pre-release**,不会被工具链当成 production
ready 抓走。**正式版 `v0.12.0` 的 tag 尚未发。**

✅ **规范侧本版**：`wlwl-spec-v1` 的 **v1.0.2 文本**已进本仓
（`docs/wlwl-spec-reference.md` ＝ 从规范仓 v1.0.2 重建的合并册；
⚠ **发布包不含该册** —— 规范文本不随包分发，取规范文本请到 `Laffinty/wlwl-spec-v1`），
**且规范侧 v1.0.2 的 tag 已发**（`refs/tags/v1.0.2` ⇒ 提交 `294dc07`）——
一致性册 §3.5 的四道发布闸门**全部已过**：第 1 / 3 / 4 条已过；第 2 条（§6.2 的
V-01…V-06）**已清零** —— V-01 随 **X-14 的消解**（总纲 §0.9 的 **D-20**）清零，
V-04 的**宿主挂起**那一格随本批的 **4 条 `h3_10_*` 契约用例**取证。
⇒ **本仓里的规范文本就是已发布的规范版本。**

本批实测数字（**`v0.12.0_rc1` 那个快照**）：命名空间 **21**（`std.*` 19 ＋ 官方包 `ai` / `agent` 2）——
标准库成员面 **231**（v0.11.4 的 214 ＋ **17** 个新成员）；
probe 用例 **181**；契约测试 **2991 例全绿**；`clippy -D warnings` **0**；
`cargo fmt --check` **0 diff**；一致性册 §5.4 双向门禁 **25 行全部一致**。
⚠ **收尾批之后是 2995 例**（+4 条 V-04 契约用例）⇒ 正式版的读数见下方 `[0.12.0]` 段。

本批新东西：**17 个新公共面成员** ＋ **五处语义变更**（D-14 / D-15 / D-16 / D-18 / D-19）
＋ 一道**新的机器门禁**（§5.4 状态列 ↔ 契约表的双向核对）—— 完整清单见下方的 `[0.12.0]` 段。

## [0.12.0] — 公共面扩充 17 员 + 传输层等待归宿主挂起（正式版 tag `v0.12.0`）

**对应规范侧 `wlwl-spec-v1` v1.0.3。** 本轮把 17 个新公共面成员落进实现，
并把**五处**语义变更收敛到规范要求的形状（**D-20** 是规范侧**消解一项承诺** ——
实现侧**无行为改动**，只同步接线表与抄件）。

**规范基线**：`wlwl-spec-v1` 的 **v1.0.3 已发布**（tag `v1.0.3` ⇒ 提交 `408e1f0`）
⇒ 一致性册 §3.5 的**四道发布闸门全部已过**：第 1 条（§0.9 差异表的「实现状态」列无
「未实现」/「部分」）、第 3 条（公共面清单 ↔ 附录 A 逐项一致）、第 4 条（无悬空跨册
引用）已过；第 2 条（§6.2 的 V-01…V-06）**已清零** —— V-01 随 **X-14 的消解**
（总纲 §0.9 的 **D-20**）清零，V-04 的**宿主挂起**那一格随本批的 **4 条 `h3_10_*`
契约用例**取证。
⇒ 本仓的 `docs/wlwl-spec-reference.md` ＝ **已发布**规范版本 `408e1f0` 的抄件
（⚠ **发布包不含该册** —— 规范文本不随包分发）。

**本版实测读数**：契约测试 **2995 例全绿** / 0 failed / 2 ignored（102 个 target）；
`cargo fmt --check` **0 diff**；`clippy -D warnings` **0**；
一致性册 §5.4 双向门禁 **25 行全部一致**（**0 红**）。

⚠ **本轮不发 tag**：`impl/Cargo.toml` 的版本号已落 `0.12.0`（workspace + 10 条 path 依赖
共 11 处），**打 tag 由业主发起**。
⚠ **历史段说明**：仓内另有一段 `## [v0.12.0(未发布的历史痕迹)] — 2026-10-22` ——
那是**先前 agent 私自打出、未经业主同意**、已撤回的同名 tag 所对应的段落
（内容是 E0103 批次那批改动，与本版**无关**）⇒ 本段才是 **v0.12.0 的正式段**。

### Added — 17 个新公共面成员（规范 §0.9 的 **D-13**）

**`std.net`（+9）** —— 传输层族（§21.6）：

- `TCP_LISTEN(host, port) -> INTEGER` —— 绑定并监听，返回**监听句柄**；
  `port = 0` ⇒ 宿主分配临时端口（实际端口用 `TCP_LOCAL_ADDR` 读回）。
- `TCP_ACCEPT(listener, timeout_ms) -> INTEGER` —— 接受一个连接；**超时 ⇒ `NULL`**
  （那是**正常值**，不是 `ERR`）。
- `TCP_RECV(conn, max_bytes, timeout_ms) -> DICT` —— 读至多 `max_bytes` 个字节，
  返回 `{bytes, text, eof}`；**超时 ⇒ 空字节且 `eof = FALSE`**（**不得**当 EOF）。
- `TCP_SEND(conn, payload) -> INTEGER` —— 写 `payload`（`STRING` 按 UTF-8 /
  `ARRAY[INTEGER]` 原样），返回写入字节数。
- `TCP_PEER(conn) -> DICT` —— 对端地址 `{host, port}`。
- `TCP_LOCAL_ADDR(listener) -> DICT` —— 本地绑定地址 `{host, port}`。
- `TCP_SET_TIMEOUT(conn, ms) -> NULL` —— 设该连接的**读超时**（`0` = 无限）。
- `TCP_CLOSE(conn) -> NULL` / `TCP_CLOSE_LISTENER(listener) -> NULL` —— 关闭；**幂等**
  （重复关 / 关从未存在的句柄都返回 `NULL` 且**不**产生诊断）。

**`std.encode`（+3）** —— 字节层（§11.2）：

- `UTF8_ENCODE(s) -> ARRAY[INTEGER]` —— 字符串 → UTF-8 字节（元素 `0..=255`）。
- `UTF8_DECODE(bytes) -> STRING` —— UTF-8 字节 → 字符串；**不做有损替换**
  ⇒ 非法字节 ⇒ `ERR([kind:"DecodeError", ...])`。
- `BYTE_LEN(s) -> INTEGER` —— UTF-8 的**字节数**（`LEN` 给的是码点数）。

**`std.text`（+1）**：`TO_INT(s) -> INTEGER` —— **宽松**的文本 → 整数
（裁剪首尾 / 接受十六进制 / 小数向零截断）；解析失败 ⇒ `ERR([kind:"ParseError", ...])`。
⚠ 与全局内建 `INT` **故意不同**：`INT` 是**严格**内部转换（不裁剪 / 只十进制 / 失败是 `ERR` 值）。

**`std.str`（+1）**：`CAT(parts...) -> STRING` —— **变长**拼接（语言的 `+` 是二元的）。

**`std.env`（+1）**：`SCRIPT_PATH() -> STRING | NULL` —— 入口脚本的路径。

**`std.result`（+1）**：`TAKE_ERR(x) -> payload | NULL` —— **一步式「检查并消费」**：
实参是 `ERR` ⇒ 取走其载荷**并**解除那份未消费状态；不是 `ERR` ⇒ 返回 `NULL`。
⚠ 它是**取走型消费者**（语言核心册 §8.3）⇒ 只观察（`IS_ERR`）**不**解除未消费状态。

**`std.time`（+1）**：`SLEEP_ASYNC(ms) -> NULL` —— **可挂起**的睡眠：挂起当前任务、
让出调度，到期醒来继续；等待期间**其它任务可以运行**。
⚠ 它**不是** `SLEEP` 的另一种形态 —— `SLEEP` 仍是阻塞原语。
⚠ **气泡内等同 `SLEEP`**（拨快虚拟钟、立即返回）—— 规范 §15.6（**D-19**）。

### Changed — 五处语义变更（规范 §0.9 的 **D-14…D-20**）

- **传输层的等待由「阻塞冻结 `SCOPE`」改为「宿主挂起」**（**D-18** · **breaking**）：
  等待期间**只挂起当前任务**，**别的任务照跑**；`timeout_ms` 是**那一次等待**的截止时间，
  `0` = **无截止时间**（挂起直到条件满足）。
  ⇒ **迁移写法**：原先指望「等待期间别的任务不跑」的程序**要改**；
  同一个 `SCOPE` 里**并发发多个连接**从「错的写法」变成「对的写法」。
- **`std.fs` 的 IO 失败从原生诊断改为 `ERR` 值**（**D-14** · **breaking**）：
  读 / 写失败 ⇒ `ERR([kind:"FsError", op, reason])` 值，**不再终止运行**；
  `E0061` / `E0062` 自此**不再由 `std.fs` 产生**（**两个码本身不删**）。
  ⇒ **迁移写法**：原先依赖诊断终止的代码改用 `IS_ERR` / `ERR_PAYLOAD` 判定并自行处置。
- **`ERR` 处置新增一步式构造 `TAKE_ERR`**（**D-15**），
  并补齐语言核心册 §8.6.2 第 3 项在**切段任务体路径**上的判定
  —— 顶层 / 普通闭包 / **切段任务体**三者行为**一致**（切段**不构成豁免**）。
- **气泡内 `SLEEP_ASYNC` 等同 `SLEEP`**（**D-19** · **breaking**）——
  气泡的虚拟钟**只在 `SLEEP` 时前进** ⇒ 气泡内挂起者**永醒不来**。
  ⇒ **迁移写法**：原先指望「气泡内 `SLEEP_ASYNC` 会挂起 / 会让别人插进来」的程序**要改**。
- **「别名在写序列内部每轮重建」不再有摊还承诺**（**D-20** · **规范侧 breaking**）——
  该形态由语言核心册 §19.1 的值语义决定（每轮的快照都让旧值可观察 ⇒ 写**必须**
  产出新值 ⇒ 每轮拷贝整份）⇒ 与 §18.3.1 第 2 条**自相矛盾** ⇒ 规范侧已把它
  **明确排除**在摊还承诺之外。**实现侧本就不排期该项**（收敛路线 = 惰性 CoW /
  结构共享，属表示层的另一项决策）⇒ **实现侧无行为改动**，只同步接线表与抄件。
  ⇒ **迁移写法**：别把别名**放在写序列内部**建立 —— 把它**提到写序列之外**建立
  一次（§18.3.1 的 **X-12** 形态）⇒ 只在首次写拷贝一次、之后原地写 ⇒ 序列级摊还
  成立；或改用累加器形态。
- **字符串累加就地化**（X-25 收敛）：循环内 `SET(acc, +(acc, t))` 的总拷贝从
  Σ(1..n) ⇒ **n 字节**（n = 16 ⇒ 16 字节 / n = 64 ⇒ 64 字节，**Θ(n)**），
  而**可观察面一字未变**。

### Fixed

- **孤儿任务失败不再无声无息**（**D-16**）：无人 `AWAIT` 的任务失败**必须**报 stderr 一行
  （含任务身份 / 诊断码 / 诊断消息），程序**继续**、**退出码不变**；
  被 `AWAIT` 的失败仍在**调用点**抛出且**不重复报告**。
  ⇒ 判定**延后**到程序收尾（失败发生的那一刻判不准「有没有人等」）。
- **取消时清空宿主等待登记**（并发册 附录 H `H.4.3`）—— 被取消的 socket 等待者
  不再留在等待表里让调度循环**空等**。
- **截止时间在两条驱动循环里都被服务** —— 此前只有 `AWAIT` 驱动的那条处理
  ⇒「截止时间可打断挂起」**只在有 `AWAIT` 时成立**，现已两条都成立。
- **`SLEEP_ASYNC` 改按调用点记账** ⇒ 一个任务体可以**睡多次**，
  循环里**每轮都真的睡**；顺带删掉了一条**规范里没有**的诊断
  （原形态：「一个任务体至多一次睡眠」⇒ 第二次报 `E0030`）。
- **句柄泄漏守卫的假绿** —— 读不到句柄数时守卫**判红**（此前哨兵相等 ⇒ 恒真 ⇒ 报绿）。
- **`[namespaces]` 的路径基准** = 清单所在目录（与 `wlwl.toml` 文档一致）。

**传输层等待的四条修复**（2026-10-25，第三方测试报告核实 ⇒ 详见
`docs/history/wlwl-v0.12.0-rc1-audit-and-fixes.md`）：

- **重放中的 `TCP_ACCEPT` 不再再次挂起** —— 宿主挂起后恢复时挂起点被**再次
  求值**（并发册 §17.0 第 2 条：「等待条件既已满足，它**必须**立即完成；实现
  **不得**让它再次挂起」）。而 `TCP_ACCEPT` 的等待条件是「队列里有连接」——
  连接一旦被取走，条件**不再成立** ⇒ 重放时它又挂一次 ⇒ 任务原地活锁：
  实测 accept 烧光自己的 5000 ms 预算 ⇒ 返回 `NULL` ⇒ 调用方拿 `NULL` 去
  `TCP_RECV` ⇒ **`E0030`**，且第一次 accept 到的连接**被覆盖、再也无法关闭**。
  ⇒ 现记「本任务 accept 到的连接」，重放时**返回同一个**。
  **同一用例：5345 ms + `E0030` ⇒ 230 ms + 拿到数据。**
- **就绪等待改为事件驱动** —— 解释器侧原本是「睡 50 ms 再看一眼」⇒ 一次「就绪」
  最多白等 50 ms ⇒ 现给就绪队列加**条件变量**（I/O 线程一推进就唤醒）。
  **同一用例：51 ms ⇒ 0 ms。** 另：就绪事件在「没有等待者」时不再丢弃。
- **一个 socket 可以有多个等待者** —— 此前第二个任务挂上来会**静默顶掉**第一个
  ⇒ 那个任务永远等不到唤醒，只能耗到自己的截止时间。
- **`TCP_RECV` / `TCP_ACCEPT` 先看货再问超时** —— 此前顺序相反 ⇒ 数据在截止
  时间**之前**就到了，却因截止时间先到而被**丢弃**。
- **就绪集合从「一次性队列」改为「电平」** —— 解释器侧此前是**排干**就绪队列
  ⇒ **同一进程里跑两个求值器**（测试套件、嵌入 wlwl 的宿主程序）时，一个求值器
  会把**另一个**的就绪事件吃掉 ⇒ 那个求值器的任务**永远等不到唤醒**
  ⇒ 实测：`TCP_ACCEPT` 白等满 5 s 预算才超时（CI 上两条平台同时抓到）。
  ⇒ 现在解释器侧**只取走自己有等待者的**那些事件，别人的原样留给主人；
  条件被消费时（读走了字节 / 取走了连接）标记作废。
- **恢复提示覆盖「每一次从挂起恢复」** —— 二期复测发现：恢复提示原先**只在
  socket 就绪唤醒时**置位，而**截止时间到期**、**定时器到点**、**通道对端**这三种
  唤醒**不置** ⇒ 重放时 `TCP_ACCEPT` 不查它的备忘 ⇒ 弹出空队列 ⇒ 返回 `NULL` ⇒
  `TCP_RECV(NULL)` ⇒ **`E0030`** ⇒ 任务死掉、连接句柄丢失、客户端收到 reset
  ⇒ **规范要求的超时形态 `{bytes: [], text: "", eof: FALSE}` 在该形态下不可达**。
  ⇒ 现改为每一种恢复都置提示（挂起点不是 socket 时用哨兵值 `RESUME_NON_SOCKET`）。

⚠ **这四条不是语义变更** —— 它们落地的是并发册 §17.0 第 2 条与第 3 条，以及
标准库·系统册 §21.6.4 的「那一次等待的截止时间」口径。

### Added — 工具链与门禁

- **`wlwl namespace [path]`** —— 列出某命名空间的成员（JSON）。
  ⚠ 它给的是**实现的成员面**，**不是**规范的公共面（输出带 `"surface": "implementation"`）——
  **不得**用它生成或改写规范的附录 A（注册镜像册 §0.7 规则 1）。
- **`wlwl run <dir>`** —— 跑一个工程目录（入口 = `<dir>/wlwl.toml` 的 `[package].entry`）。
- **一致性册 §5.4「状态」列 ↔ 契约表的双向门禁**（`script/check_conformance_registry.py`）
  —— 行里写着「未实现」而契约表全绿 ⇒ 门禁红；反向也查（实现回退 ⇒ 红）。
  ⇒ 规范仓**每新增一行**差异，接线表 `script/conformance_registry_map.json` 必须同时加一条。
- **发布包随包发编译器源码**（`script/package_source.py`）＋ 可复算的树哈希 `SOURCE_HASH`；
  **依赖集随源码冻结**（`impl/Cargo.lock` 随包走 ⇒ 重建出来的**是同一个编译器**）。
- **`impl/examples/loopback-server/`** —— 纯 WLWL 写的最小 HTTP 服务基座（可直接跑）。

### ⚠ 仍未收敛的差异（**必须**随发布说明列出）

**本版为空** —— 一致性册 §5.4 的 **25 行全部「已实现」** ⇒ 没有「实现待收敛」的差异
⇒ 这张表**没有行可列**。
⚠ 此前**唯一**的一行 **X-14**（别名在**写序列内部**每轮重建 ⇒ 每轮写产出新值 ⇒
总工作量二次）已由**规范侧消解**（总纲 §0.9 的 **D-20**）：该形态是语言核心册 §19.1
值语义的内在推论 ⇒ 与 §18.3.1 第 2 条自相矛盾 ⇒ 已**排除**在摊还承诺之外 ⇒
规范侧 §5.4 的该行转「已消解」（类别「—」/ 状态「已实现」）。
⇒ **它不再是一条「实现待收敛」的差异**，故不列于此表；但**迁移写法**见上方
「Changed」段的 **D-20** 条（⇒ 别名**提到写序列之外**建立一次，或改用累加器形态）。
⚠ 该形态的**可核对锚点仍在仓内**（`impl/crates/wlwl-eval/tests/x14_alias_in_loop_cost_contract.rs`
的 `#[ignore]` 目标测试，**红**）⇒ 将来真要收敛（惰性 CoW / 结构共享）时用得上。

⚠ **规范外扩展台账**（一致性册 §5.5）本版**为空** ⇒ 实现侧当前**没有**任何
「规范未认可」的扩展成员；台账条目**不得**被表述为「规范已支持」。

## [0.11.4] — 规范基线切换:v0.11 → v1.0(未发 tag)

**规范基线从 v0.11 切到 v1.0。** 权威处在独立仓
[`Laffinty/wlwl-spec-v1`](https://github.com/Laffinty/wlwl-spec-v1)(v1.0,11 册);
本仓 [`docs/wlwl-spec-reference.md`](docs/wlwl-spec-reference.md) 是把这 11 册
**合并成一份**的**次级规范载体** —— 机器守卫与 AI agent 读它。
⚠ **本轮不发 tag**:实现侧这条对齐线的版本号 = **v0.11.4**(业主 2026-10-24 定)⇒
`impl/Cargo.toml` 的 `[workspace.package] version` **未动**(仍是**已发布**的 `v0.12.0`
—— 改成 `0.11.4` 是版本号**回退**,且本轮不打 tag ⇒ 无版本守卫需求)⇒
⚠ **2026-10-24 发版**：业主指令改为 **`0.11.4` 打 tag 发布** ⇒ `Cargo.toml` 已落
`0.11.4`、tag **`v0.11.4` 已推**（Release 全绿）。
⚠ **`v0.12.0` 不是既定发布点**——该 tag 系**先前 agent 私自打出、未经业主同意**；
版本线上以 **`v0.11.4` 为准**（`v0.12.0` 的 tag/Release **不移动**，仅作历史痕迹）。
⚠ **规范侧**已同步打 `v1.0.1` 标记规范侧发布点(`v1.0.0` 已发布,不移动)。

### Changed

- **合并册换成 v1.0 的十一册**:总纲 / 语言核心 / 对象与行为类型 / 并发 / 一致性,
  外加标准库六册(核心 / 文本 / 系统 / 安全 / 官方包 / 注册镜像)—— 此前是 v0.11 的
  三册(语言规范 / 标准库规范 / 代理工作说明)。
  ⚠ **章号与 v0.11 对齐**(语言面 §0–§12 不变,对象面仍 §13–§16,并发面仍 §17;
  标准库面 §1–§22 不变)⇒ 文档里原有的 `§N` 交叉引用**仍然有效**,不必改。
  ⚠ **两套编号空间彼此独立**:语言面与标准库面**都有「§5」** ⇒ 跨面引用必须带册名
  (「语言核心册 §5.2」与「标准库册 §5.2」是两处不同的东西)。
  ⚠ **标准库面的 §14 空缺**:v1.0 拆掉了旧的 stdlib §14(演进方向)——「复活条件」
  分散到各章、「已否决」移到总纲 ⇒ 引用旧 stdlib §14 的文案会扑空(语言面的 §14
  是另一回事:行为类型与会话类型协议)。
- **实现侧零行为变更**:成员面 **21 个命名空间 / 214 个成员**,v0.11 → v1.0
  **增删各 0** ⇒ 本仓实现**不需要新增任何成员**。v1.0 新增的是**规范章节**
  (§18 复杂度与性能保证、§19 内存与资源模型、一致性册 §5「规范与实现的差异登记」
  X-01…X-15、并发册 附录 H 操作语义),不是成员。
- **契约测试的判据与 v1.0 的章节结构对齐**(不是行为变更):marker 换成 v1.0 的
  等价语义表述,成员表抽取范围收窄到 `### N.x 成员表` 子节 ⇒ 抽到的仍是同一批成员。
- **新增 `script/build_spec_reference.py`** —— 合并册的**重建工具**:从规范仓的
  11 册机械重建 `docs/wlwl-spec-reference.md`(**整体覆盖**;日常维护直接编辑
  合并册,只在需要从规范仓整体重建时才跑它)。

### 附录 A 的治理模型改为「冻结快照 + 抄件」(业主 2026-10-22 裁决)

v1.0 规范仓的**注册镜像册 §0.7** 规定:「**本册附录 A 是 v1.0 的冻结快照,不是
随实现更新的活镜像。本册的单源真相是本规范自身**」,规则 1:「表体**不得**由
实现清单自动生成,**不得**为追平实现的当次改动而改写」。

⇒ 本仓此前的「生成器从实现派生表体 + 逐字节锁」与该规定**方向相反**:实现侧
一改成员,跑一次生成器就把冻结快照改写了。

**本轮改动**:
- **生成器退役** —— `gen-appendix-a` 删除,改为**只读**的 `check-appendix-a`
  (只比对、只打印差异、**不写文件**)。⇒ 实现侧加成员**不能**悄悄改写快照。
- **守卫改为两条**,合计覆盖**不弱于**原来那一条:
  ① **抄件忠实度** —— 合并册的附录 A **逐字节** == 规范仓注册镜像册的附录 A
     (规范仓找不到时**响亮失败**,不回落 —— 回落会让这条检查静默失效);
  ② **实现与快照一致** —— 实现的成员集合 == 快照的成员集合(逐命名空间比集合)。
  ① 是旧守卫**没有**的维度(旧守卫只管「实现派生的表体 == 合并册里的表体」,
  抄件是否走样无人管)。
- CI 增加 `WLWL_SPEC_V1_DIR`(指向规范仓**目录**,不是某一册文件)。

⚠ **语义不变**:成员面仍是 21 命名空间 / 214 成员,实现侧**零行为变更**;
变的只是「谁有权改附录 A」——从「实现派生」改成「规范仓说了算」。

### 契约判据的「作用域」收口(静默假绿的最后一类)

上一段把**红**的判据对齐了;但还有一类更隐蔽的:判据落在**整本合并册**上,
它能守住那件事**只因为「全册别处碰巧没有同样的字」** ⇒ 规范将来新增一章
写出同样的字,这条守卫就**当场失效**,而读数是绿的。

⇒ 本轮把 **11 处**整册型判据收窄到**自己的章**(§4 / §11 / §12 / §15 / §16 /
§17 / §19 / §20),needle 换成**章内唯一**的锚点;并**新增机器守卫**
`spec_needle_scope_guard.rs`(词法级判定 + 显式白名单,白名单**只减不增**),
让「新增整册作用域的规范 needle」从此**过不了 CI**。

⚠ **已实测出并修掉 2 处真的静默假绿**(用「删掉目标条款 ⇒ 测试仍绿」证明,
非推测):`text_contract` 的「半张表」(v1.0 改写成「数据覆盖必须完整」,而
「半张表」只剩 `std.json` §3.5 的同义比喻)与 `fs_contract` 的整册分隔符判据。
⇒ 收窄后的证据形式是**新判据 RED、旧整册判据仍 GREEN**。

### 其它

- `ci.yml` 的两个 job 名(`test (...)` / `smoke run examples`)旁加注:它们是
  **main 分支保护规则里的 required status check**,改名会让 PR 永久卡在
  「Expected — Waiting for status to be reported」。
- **「取章正文」的样板收进共用助手**:`tests/common/mod.rs` 新增
  `chapter_body(text, title)`(结束于下一个 `## `)与
  `chapter_body_until(text, title, next_title)`(结束于**指定的**下一个章标题),
  替掉散在 **13 个契约测试文件里的 27 处**重复代码。⚠ 两种结束口径
  **不等价**:规范将来新增中间章时,通用口径会**静默截断**(新章被排除、
  测试照绿),而具体口径会把它**包进来** ⇒ 成员表对拍失败 ⇒ **变红**
  ⇒ 「设计变了就一起改测试」那道闸。⇒ **原用具体章号的必须继续用具体口径**。
  两者都**找不到就 panic**(附实际章标题清单)—— 安静返回空会让「章结构变了」
  变成静默漂移,而集合相等型断言在空集上会**假绿**。

### X-07 收敛:标准库回调的实参令牌通道(v1.0 一致性册 §5.4)

v1.0 登记的**未实现**项 X-07:「标准库回调的实参通道不带令牌 ⇒ 回调内未消费
的 `ERR` 不报(假阴性)」。

**根因**:`eval_call` 早已算出每个实参的令牌(`tokens`),但 `invoke_std` 只把
`args` 交给 stdlib 成员 ⇒ 成员不知道「我的实参携带未消费的 `ERR`」⇒ 按 §8.6.3
「读入其它任何位置 → 由接收方承担」的转发链在**第一跳**就断了。

**修法**(两条通道,`StdHost` 上各带**默认实现** ⇒ 仓内 8 处手写 `NullHost`
零改动):
- **求值器 → 成员**:`StdHost::arg_tokens()` 返回本次调用每个实参的令牌;
  令牌存 `Evaluator.current_arg_tokens`,`invoke_std` 里存旧值 → 填新值 →
  调用 → 还原(与 `source_file` / `current_span` 同族)。
- **成员 → 回调**:`StdHost::call_with_tokens()` 把令牌带进回调的**形参绑定**
  ⇒ 回调作用域结束时按 §8.6.2 第 3 项报 `E0103`。

**先只做 5 个迭代型成员**(`MAP` / `FILTER` / `GROUP_BY` / `DEDUP_BY` /
`KEY_BY`)—— 一上来全铺会改变一大批既有行为。

**语义核对**(实测):
- 回调**消费**那个 `ERR`(`ERR_PAYLOAD(x)`)⇒ **不报**(无假阳性);
- 回调**观察**那个 `ERR`(`IS_ERR(x)`)⇒ **报**(§8.6.1 观察型不解除状态);
- 容器里**候选不唯一**时**不猜**(猜错会把早已被消费的 `ERR` 报出来 = 假阳性)
  ⇒ 归 X-06。

⚠ **同时暴露了三个尚未收敛的通道**(本轮未动,需另行登记):
内建 `CALL(f,…)` 丢令牌、`*args` rest 形参不带令牌、语句值丢弃点只认 `Err`
形态的值(容器令牌在语句处被丢)。

### X-06 收敛第一步:未消费 `ERR` 的表示改成**集合**(v1.0 一致性册 §5.4)

v1.0 登记的**未实现**项 X-06 的前半:「未消费状态是一个标志,不是集合」⇒
「汇入同一接收位置时取并集」「丢弃时逐个报告」两条都做不到。

**改法**:
- **`wlwl-value`**:令牌从 `bool` 改成有序集合 `UnconsumedErrs = Vec<Value>`,
  新增 `union_unconsumed`(**并集,不去重** —— 多重集口径)与
  `find_unconsumed_index`;六个传播点各按 §8.6.1 / §8.6.2 / §8.6.3 落位。
- **`wlwl-eval`**:三个丢弃点改成**逐个报告** —— 判据从「值本身是不是
  `ERR`」改成「**集合非空**」⇒ **顺带修掉 X-17**(持有未消费 `ERR` 的容器 /
  字典被丢弃时也报 `E0103`)。
- **`wlwl-error`**:新增 `WlwlError::Diagnostics(Vec<WlwlDiagnostic>)` 变体 +
  `diagnostics()`;`diagnostic()` 在多条时返回**第一条** ⇒ 仓内 390+ 处
  `.diagnostic()` 调用点**零改动**。
- **`wlwl-cli`**:新增 `diag_output.rs`,把一批诊断**逐条**打印(stderr 逐条 /
  JSON 一诊断一对象 / LSP 一次全量发布);退出码按**第一条**的类别定(未改
  退出码表本身)。

⚠ **`E0103` 的措辞报的是「被处置的那个值」,不是「里面那个 `ERR` 的载荷」**
—— 依据 §8.6.3「容器装盛……该 `ERR` 继续到**容器**;容器本身在丢弃点被处置」
+ 冻结契约自带注释 ⇒ 与 `b48a442` 的旧口径**逐字一致**,不是新行为。

⚠ **一处语义回归已修**:`Env::read_binding` 的 `Consume` 分支曾被收窄成「只在
读到的值**本身**是 `Value::Err(_)` 时才从源集合里移除」⇒ 读到的值是
`OK(ERR(x))` 时移不掉 ⇒ 源绑定与结果值各留一份 ⇒ **同一个 `ERR` 被报两次**
(违 §8.6.5),且 `std.result` 的 `FLATTEN`(wlwl 级闭包)形参作用域结束时误报
⇒ 冻结期望 `result_contract::flatten_ok_err_surfaces_the_inner_err` 转红。
**修法**:`Consume` 分支加「值是 `OK(ERR(..))` ⇒ 移除**内层**那个 `ERR`」这一支
(容器等形态不动)⇒ 语义**只留一处定义**(eval 侧那份同义补偿已删)。

**契约**:`x06_unconsumed_set_contract.rs`(21 例)、`multi_diagnostic_output.rs`
(4 例);负向验证 7 组 + 6 组,全部「改坏即红」。

⚠ **剩余缺口**(详见 `docs/active/x06-x07-unconsumed-token-batch.md` §6):
~~`StdHost` 两条通道仍是 `bool`(集合跨不过 eval ↔ std 通道)~~ ✅ **已收敛**
(步 ①:两条通道改成集合本体)、~~X-20(标准库成员的实参边界)~~ ✅ **已收敛**
(步 ②:见下节)、~~X-06 的**跨通道逐元素归属**~~ ✅ **已收敛**(通道是集合 +
元素分发按元素各带各的 ⇒ 逐元素归属成立);
**仍在**:`TRY(OK(ERR(x)))` 的假阴性(缺口 **G-2**,与本批无关)。

✅ **两处规范缺口已由业主裁决(2026-10-23),并落进规范册**(规范仓提交 `7d3cfdd`):

- **身份语义** ⇒ 取**甲「值语义即新身份」** ⇒ 落进语言核心册 **§8.6.5** ⇒ 深拷贝
  产生的 `ERR` 是**新生的** ⇒ 两个内容相同但来源不同的 `ERR` 是**两个** ⇒ 被分别
  处置时**各报一次**。⇒ **本实现按内容判定身份、集合又是多重集(不去重)⇒ 恰好
  与该口径一致** ⇒ ⇒ **不需要**改 `Value::Err` 的表示(111 处构造点一处不用动)
  ⇒ **X-06 的身份半步到此收口**。
- **实参进成员** ⇒ 取**甲「转给该调用的结果」** ⇒ 落进语言核心册 **§8.6.3** ⇒
  成员是接收方、结果是接收方的产物;并据此明确「成员**不是**取走型消费者」
  (§8.6.1 的名单封闭)⇒ 成员**结清不了**那份状态。⇒ **实现侧已落地**(见下节
  「X-20 收敛」):分账规则按处置册 §6.1 写死(成员转给回调的那部分**按内容**从
  「成员手上」**移走**,不是复制)⇒ 回调与结果**各持各的、不重叠**。
  ⚠ 该落地是 **breaking**(今天 `PRINT(a)` 不报,按新条它会在该语句处报)⇒ 已进
  发布说明 + 重跑全部契约测试。

### X-16 收敛:内建 `CALL` 的实参通道带未消费集合(v1.0 一致性册 §5.4)

v1.0 登记的**未实现**项 X-16:「内建 `CALL(f, …)` 的实参通道不带令牌 ⇒
`LET(f, FUN((x), 0)); CALL(f, [ERR("boom")]);` 里的未消费 `ERR` **不报** `E0103`
(假阴性)」。

**根因**:内建函数的签名是 `fn(&mut Evaluator, Vec<Value>)` ⇒ **没有**传未消费
集合的参数位 ⇒ 集合在**进入内建**时被丢掉 ⇒ 被调方的形参绑定拿不到。

⚠ 与 **X-07** 是**两条不同的通道**:X-07 修的是「标准库成员回调用户函数」(成员
侧那条通道由 `StdHost` 承载),X-16 修的是「语言内建 `CALL` 调任意可调用值」。

**修法**(三处,一处语义):① `dispatch_call` 的内建分支在调用前后投递本次调用的
每参集合(`Evaluator::current_arg_unconsumed` + 布尔投影,与 `invoke_std` 同族
机制)⇒ 内建**能读到**;② `builtin_call` 取集合并**跳过第 0 个实参**(那是**被调者**
自己,不是 `CALL` 的实参);③ `call_value_with_receiver` 新增 `arg_sets` 参数 ⇒
闭包分支传给形参绑定、原生成员分支传给成员侧通道;⚠ receiver 注入时**同步**在
集合表里插一个**空集**(否则形参绑定会**错位**拿到下一个实参的集合)。

**语义核对**(实测):被调方**消费**那个 `ERR` ⇒ **不报**(无假阳性);被调方
**显式丢弃**(`LET(_, x)`,§8.6.4)⇒ **不报**;实参没有 `ERR` ⇒ 不报;被调方
**返回**该实参 ⇒ 状态**转给调用方** ⇒ 由调用方持有并处置 ⇒ 报;**逐参对齐**
(两个实参、只有第二个持有状态)⇒ 集合落在**第二个**形参上。

**契约**:`x16_builtin_call_token_contract.rs`(9 例);负向验证 3 组「改坏即红」:
内建分派不投递集合 ⇒ 5 红;集合表不跳过被调者(对齐偏移)⇒ 5 红;集合到不了形参
绑定 ⇒ 4 红。

⚠ **同时暴露并登记了三条同族缺口**(实测,见下节):`CALL_METHOD` / `NEW` 的
实参通道同样不带令牌(X-18 / X-19),以及**标准库成员的实参边界**把集合吞掉
(X-20)。

### X-18 / X-19 收敛:内建 `CALL_METHOD` / `NEW` 的实参通道带未消费集合

本批实测新发现并登记的**未实现**项 X-18 / X-19:内建 `CALL_METHOD` / `NEW` 的实参
通道**不携带**未消费集合 ⇒ 方法 / 初始化器的形参拿不到 ⇒
`CALL_METHOD(o, "m", [ERR("boom")])` 与 `NEW(C, [ERR("boom")])` **不报** `E0103`
(假阴性)。

**根因与修法**:与 **X-16** 同款 —— 内建函数的签名 `fn(&mut Evaluator, Vec<Value>)`
**没有**传集合的参数位 ⇒ ① `dispatch_call` 的内建分支投递本次调用的每参集合
(与 `invoke_std` 同族机制)⇒ 内建**能读到**;② `builtin_call` / `builtin_call_method` /
`builtin_new` 各自取集合并**跳过前置实参**(`CALL` 跳过**被调者**、`CALL_METHOD`
跳过**接收者**与**方法名**、`NEW` 跳过**类**本身);③ `call_value_with_receiver` 把
集合传到形参绑定。

⇒ **内建 → 用户函数的三条通道至此全覆盖**(`CALL` / `CALL_METHOD` / `NEW`);
**属性调用**形态 `o.m(…)` 路由到 `CALL_METHOD` ⇒ 同款同报。

**契约**:`x18_call_method_token_contract.rs`(7 例)、`x19_new_token_contract.rs`
(6 例);**负向验证 3 组**「改坏即红」:`CALL_METHOD` 不跳过前置实参 ⇒ 4 红 /
`NEW` 不跳过类 ⇒ 3 红 / 通道不投递集合 ⇒ 4 红 ⇒ **每次只打红对应那一张表**
(变异是局部的)。

### X-20 收敛:标准库成员的实参边界 —— 那份状态**转给该调用的结果**

规范侧已裁决(2026-10-23,业主取**甲**「转给成员的**结果**」)并落进语言核心册
§8.6.3 第三条款 ⇒ 实现侧按甲落地。

**缺口本体**:标准库**成员**的实参位**不建立绑定**(实参不是绑定)、它的结果也不是被
传进去的那个值 ⇒ 实参持有的未消费集合**跨过成员边界后无处可落** ⇒
`LET(a, [ERR("boom")]); PRINT(a);` **不报** `E0103`(假阴性);而同一份状态经直接绑定
(`LET(b, a);`)**会**报 ⇒ 洞精确地在**成员边界**。

**修法**(三处,一处语义):① `StdHost` 两条通道从 `bool` 改成**集合本体** ⇒ 集合
**跨得过** eval ↔ std 这条通道(步 ①,行为零变化);② 成员把**一部分**未消费状态转给
自己的回调时,那部分从「成员手上」**移走**(**按内容**做多重集差集,不是复制)⇒ 回调
与结果**各持各的、不重叠**;③ 成员返回时把「**没转出去的那部分**」并进**结果的集合**
⇒ 那份状态随结果继续,直到在下一个丢弃点被处置。

⚠ **breaking**:`PRINT(a)` 这类**开始报** `E0103`(今天 0 条)⇒ 已在发布说明里列出。

**契约**:新表 `x20_member_boundary_contract.rs`(10 例)—— 形态一(转发给回调 ⇒ 结果
**不携带**)/ 形态二(不转发 ⇒ 结果承担)/ **红线**(同一个 `ERR` **至多**一条 `E0103`)/
按元素分发 / 反向守卫(消费、显式丢弃)/ 既有口径不得改变。
⚠ **每条用例都断言 `E0103` 的条数**,不只是第一条诊断 —— 只看第一条**看不见**
「同一个 `ERR` 报两条」(那两条落在**同一个**报告点上 ⇒ 第一条消息一模一样)。

**负向验证 4 组**:内建 `CALL` 改回复制 ⇒ `x16` 2 红 + `x20_c` 红 / `CALL_METHOD`
改回复制 ⇒ `x18` 2 红 / `NEW` 改回复制 ⇒ `x19` 2 红 / 元素分发改回「候选唯一就不转」
⇒ `x20_d` + `x20_e` 红 ⇒ **每次只打红对应那一张表**。

⚠ **落地时额外抓出两处**(在途版本自带 ⇒ 必须一并修,否则这批落地**本身**会带进
**假阳性** —— 本仓纪律:一个会误报的修法比不修更糟):

① **内建帧把集合复制**给被调方、自己那份**不放手** ⇒ 被调方消费过了、结果还带着 ⇒
**误报**(`x16` / `x18` / `x19` 各 2 条);且被调方**返回**该实参时 ⇒ 结果已带一份 +
承接再挂一份 ⇒ 多重集**不去重** ⇒ **同一个 `ERR` 报两条**(实测 `LET(f, FUN((x), x));
LET(r, CALL(f, [ERR("boom")])); 0` ⇒ **2 条**)⇒ 违 §8.6.5「一个 `ERR` 至多报告一次」
⇒ 修法:那三处改成**取走**(`std::mem::take`)⇒ 内建帧不再持有已交给形参绑定的那份。
② **元素分发只给「唯一承载者」** ⇒ 数组里同时装**两个**未消费 `ERR` 时,那两个
`ERR` **从没进过任何回调** ⇒ 回调**消费**或**显式丢弃**了它们之后**仍被报** ⇒
**假阳性**(实测 `MAP([ERR("a"), ERR("b")], FUN((x), (LET(_, x); 0)))` ⇒ **2 条**,应当
**0 条**)⇒ 修法:`element_token` 改成**每个元素各带各的**(求值器侧按元素收窄)⇒
差集恒为空 ⇒ 回调处置了它就**不再被报**。

### X-01 收敛:通道关闭**释放**停泊的发送方 ⇒ 其 `SEND` 以 `E0054` 失败

v1.0 登记的**未实现**项 X-01:「通道关闭**不**唤醒停泊的**发送方** ⇒ 它停在通道操作上,
`AWAIT` 得到 `E0053`(无对端唤醒)」(一致性册 §5.4;并发册 §17.2.3 / §17.7 / 附录 H.3.7)。

**规范要求**(并发册 §17.2.3):「关闭**必须**释放该通道上全部挂起的等待方」—— 停泊的
**接收方**按「关闭且读空」完成(返回 `ERR(kind="ChannelClosed")`);停泊的**发送方**
重跑后其 `SEND` 以 **`E0054`** 失败。附录 H.3.7 的参考转移是同一句。

**根因**:`wlwl-eval` 的 `Channel::close()` 只 `swap` 出 `receiver_waiters` ⇒ 停泊的
发送方**永远不被唤醒** ⇒ 对它 `AWAIT` ⇒ `E0053`(「无对端唤醒」)⇒ **不**是规范要的
`E0054`。

**修法**(三处,一处语义):
① `Channel::close()` 改为**同时**换出**发送方**停泊队列 ⇒ 返回值改成带两组列表的
`ClosedWaiters { receivers, senders }`;⚠ `sender_waiter_values` 必须**与**
`sender_waiters` **一起**换出 —— 两者是**平行数组** ⇒ 只换一个会让长度对不上 ⇒
`pop_sender_waiter` 会取走**别人**的值;
② `CHANNEL_CLOSE` 把**发送方**也唤醒(`wake_channel_op_waiter` 本来就同时认
`ReceivingOn` 与 `SendingOn`)⇒ 被唤醒的发送方**重跑段首** ⇒ 再次 `SEND` ⇒ 通道已关闭
⇒ `try_send` 返回 `Closed` ⇒ 抛 **`E0054`**;
③ **作用域退出**的强制关闭(`pop_scope`)同样释放停泊的发送方(同一缺口的两条路径)。

⚠ **顺带收掉一个**误报:两个发送方都停泊且**没有**接收方时,旧读数是 **`E0065`**
(死锁检测误报)—— 死锁检测看到「两个任务都停在通道操作上」⇒ 修好之后是 `E0054` ⇒
**死锁误报也一并消失**。

⚠ **实现里另有一条**部分释放**的路径**:`CHANNEL_RECV` 在**关闭后**读到 `Closed` 时会
`pop_sender_waiter` 唤醒**一个**停泊的发送方 ⇒ ⇒ 它只释放**一个**、且**只在有接收方
时** ⇒ ⇒ 关闭本身**必须**释放**全部**(契约表 `x01_c` 特意**不放任何接收方** ⇒ 唯一
的释放来源就是关闭)。

**契约**:新表 `x01_channel_close_contract.rs`(6 例)—— 停泊的发送方以 `E0054` 失败 /
**两个**停泊的发送方**都**被释放 / 停泊的接收方仍按原样拿到 `ChannelClosed` / 关闭
**幂等**(不重复释放 ⇒ 诊断**恰好一条**)/ 无停泊方时关闭**干净** / 作用域退出的强制
关闭**不外泄**诊断。⚠ 每条用例都断言诊断的**条数**。
**负向验证**:把 `close()` 改回旧行为(停泊的发送方原样留着)⇒ `x01_a` / `x01_c` /
`x01_d` 三条变红 ⇒ 探针读数从 `E0054` 变回 `E0053`(两个发送方那一条则从 `E0054`
变回 `E0065` 死锁误报)⇒ 还原后 6 例全绿。

### X-15 收敛:字典深度相等走哈希索引 ⇒ 键比较 **2n**(满足 §18.3 的 O(n) 最坏)

v1.0 一致性册 §5.4 的 **X-15**:「**深度相等(字典)**是 **Θ(n²)**(键比较 =
n(n+1)/2)⇒ 不满足规范 §18.3 的 O(n) 最坏」。

**根因**:`wlwl-value` 的 `dict_entries_equal` 对 x 的每个条目在 y 里**线性扫描**
⇒ 键比较 = Σ(1..n) = n(n+1)/2。⚠ **数组那一半一直达标**(访问元素对 = n)⇒ 只差字典。

**修法**(值层):相等判定改走 `DictInner` 已有的**键→位置哈希索引** —— 每个 x 的条目
按哈希在 y 里定位**候选**,再在候选内做**集合口径**核对。⚠ **三条语义红线全部守住**:

* **集合口径**(§2.4,不看顺序)—— `{"a":1,"b":2}` 与 `{"b":2,"a":1}` 仍然相等;
* **重复键** —— 索引存的是**位置列表**(不是单个位置)⇒ 同一键的**全部**候选位都被
  取用 ⇒ 集合口径不会被「只取首个」悄悄改掉(`DictInner::lookup` 仍返回**首次出现**,
  与旧的线性扫描逐字一致);
* **域外键** —— 任一侧含域外键(`FLOAT` / 布尔)⇒ 整条比较**退回旧的线性扫描**
  ⇒ 语义与修前逐字一致(§2.1 的键域封闭 ⇒ O(n) 的承诺只对**合法程序**成立)。

**读数**(确定性工作量计数 `equality_key_comparisons`,不用墙钟):

| | n = 16 | n = 64 |
|---|---|---|
| 修前 | **136** | **2080** |
| 修后 | **32** | **128** |

⇒ 2n(线性)。门禁 `complexity_cost_contract` 里钉着 §18.3 **规范性上界**的那条
(原带 `#[ignore]`,注释写明「⚠ 当前红」)**去掉 `#[ignore]`** 且保持绿(c = 4 未动);
另一条钉着「键比较 = n(n+1)/2」的**期望已过时**(规范要求 O(n) ⇒ 那条期望是**偏差**
的记录,不是规范)⇒ 改成新的达标期望,并在测试文件头写明**为何变**。

**契约**:新表 `wlwl-value/tests/x15_dict_equality_cost.rs` 11 例(顺序无关 / 值不等 /
长度不等 / 域外键 / `INSTANCE` 身份键 / 重复键三形态 / 嵌套 / `DictCell` 交叉)。
**负向验证**:① 退回线性扫描 ⇒ 门禁两条 + 新表 8 条红;② 把集合口径改成**同序逐位
比较** ⇒ 新表的「顺序无关」守卫红(⑥ 仍绿 ⇒ 语义破坏只有那条守卫抓得到)⇒ 还原后全绿。

⚠ **不产生可见面变更**:判定结果一个都没变,变的只是**代价**。

### G-02 / X-22 收敛:`TRY` / `OR_DIE` 的 `OK(ERR(..))` 剥壳形态 ⇒ 那一个 `ERR` 随结果继续

**规范侧先裁决**(2026-10-23,语言核心册 §8.6.1 + §8.6.3):§8.6.1 的取走型名单只说
「该 `ERR` 被取走」,**没区分**消费者读到的是「`ERR` 本身」还是「`OK(ERR(..))`」
⇒ 裁决:值**本身**是 `ERR` 时取走它(已被处置);值是 **`OK(ERR(..))`** 且消费者
**剥壳交还**时,交还的是**新生的** `ERR`(§8.6.5「值语义即新身份」)⇒ **它自己未消费**
⇒ 它的未消费状态**必须随该结果继续**,在下一个丢弃点报 `E0103`。

**根因**:值层的取走型读法把内层 `ERR` 从源绑定**取走**后,**在途值不带集合**
⇒ 那个 `ERR` **无处可寻** ⇒ `LET(a, OK(ERR("boom"))); LET(b, TRY(a));` 一条
`E0103` 都不报(假阴性)。

**修法**:值层新增「取走但**携带**」的读模式 `ReadMode::ConsumeCarry` —— 取走口径与
`Consume` **逐字相同**,区别**只在** `OK(ERR(..))` 这一种形态上**携带**那一个被取走的
`ERR`;值**本身**是 `ERR` 时**不携带**(消费者要么处理它、要么让它逃逸)⇒ 否则
`UNWRAP_OR(ERR(x), d)` 的默认值 `d` 会白背一个未消费 `ERR` ⇒ **误报**。`TRY` 与
`OR_DIE` 的 `OK` 支把那份集合**挂回结果**。

⚠ **`Consume` 的语义一个字节未改** ⇒ 内建 `UNWRAP` / `UNWRAP_OR` 那条路径(它有自己
的结果承接机制)**逐字不变** ⇒ 不会出现同一个 `ERR` 报两条。

⚠ **可见面变更(breaking)**:**只有** `TRY` / `OR_DIE` 的实参是 **`OK(ERR(..))` 形态**
那一支 ⇒ 修前 **0 条** ⇒ 修后**恰好 1 条** `E0103`(在下一个丢弃点:绑定被覆盖 /
语句值丢弃 / 作用域结束)。**不变**:`TRY(ERR(x))` 仍按 §8.5 报 `E0102`;`UNWRAP` /
`UNWRAP_OR` / `IS_ERR` 等逐字不变。

**契约**:新表 `g02_try_ok_err_contract.rs` 13 例(每例都断言诊断的**条数**,按错误码
分列 `E0103` / `E0102` / `W0051`)。**负向验证**:退回 `Consume` + 去掉 carry ⇒ 6 条红;
只回退报告点 ⇒ 位置断言红(条数仍 1)⇒ 还原后全绿。

### X-21 收敛:`*args` rest 形参带未消费令牌(⚠ breaking)

**根因**:`invoke_closure` 给 rest 形参用 `set_local`(**不带令牌**)⇒ **富余实参持有
的未消费 `ERR` 被整个丢弃** ⇒ 被调方作用域结束时**一条 `E0103` 都不报**(假阴性)。

**修法**:rest 形参改用 `set_local_with_token`,**富余实参**的集合取**并集**后转给
rest 绑定(与固定形参**同一口径**)。⚠ 并集是**必需的** —— rest 位在 `n_fixed` ⇒
照抄形参那一位的 `arg_tokens.get(i)` 会**只取最后一个**富余实参的集合 ⇒ 前面的漏掉。

⚠ **可见面变更(breaking)**:**只有**「富余实参**不是** `ERR` 形态、却**持有**未消费
`ERR`」那一支 ⇒ 修前 **0 条** ⇒ 修后**恰好 1 条** `E0103`(处置点是**被调方的作用域
结束**,§8.6.2 第 3 项)。⚠ 富余实参**是** `ERR` 形态时走 §12.6 透明传播的**短路**
(被调方**根本没跑**)⇒ rest 绑定**没参与** ⇒ 那一支**条数与报告位置完全不变**。

⚠ **报告位置不变**:作用域结束处的 `E0103` 仍报在**调用点**那一行(与固定形参路径
**同一口径**)⇒ 本项是**纯增量**。⚠ 把 span 改成被调方的函数体(更贴近「处置点」)
会**全局**挪动所有闭包的 scope-end 报告位置 ⇒ **超出本项范围** ⇒ 留作单独提案。

**契约**:新表 `x21_rest_param_token_contract.rs` 11 例(逐条断言**条数**与**报告行号**
—— 只断言条数对「短路路径」那几条是非判别性的)。**负向验证**:退回 `set_local` ⇒
5 条红;只回退报告点 ⇒ 位置断言红 ⇒ 还原后全绿。

### X-02a 前置:`SHIELD` 的屏蔽状态改 **per-task**(跨任务不再互相遮挡)

⚠ **这不是 X-02 的收敛** —— X-02(判据从「取消的**来源**」改成「取消**到达时点**」)
**仍未收敛**;本项只把它**判据所依赖的状态**修对。

**根因**:`shield_depth` 与待发取消槽(`shield_pending_cancel` / `shield_pending_reason`)
挂在**共享 `Evaluator`** 上 ⇒ 一个任务在 `SHIELD` 块内 `AWAIT`(调度器会把别的任务
拉进来跑)时,别的任务被**误遮挡**:① 它的 `TASK_CANCEL_PARENT()` 被**误缓冲**;
② 它退出自己的屏蔽块时共享深度**回不到 0** ⇒ 那次待发取消**永不触发**(被吞掉)。

**修法**:两类状态改 **per-task**(落在 `Task` 上:`Task::shield_depth` /
`Task::shield_pending_cancel` / `Task::shield_pending_reason`);顶层(`current_task`
为 `None`)沿用求值器上的三个字段作**兜底**。`SHIELD` 的**归属任务**在**进入时**捕获
⇒ 退出时减的是同一个任务。⚠ **判据一个字没改**(仍是来源判据:`TASK_CANCEL_PARENT()`
在自己的 `SHIELD` 块内被推迟;`TASK_CANCEL(handle)` 仍立即生效)。

**读数**(同一批程序,改动前 / 改动后两个二进制**实测**):

| 程序 | 观察点 | 修前 | 修后 |
|---|---|---|---|
| V1 | 别的任务在**无屏蔽**时自取消 ⇒ 取消是否**立即**生效 | **0**(被邻任务的屏蔽深度遮住) | **1** |
| V2 | 别的任务在**自己的**屏蔽块退出后 ⇒ 它的待发取消是否生效 | **0**(被吞掉) | **1** |
| V3 | 本任务在**自己的**屏蔽块退出后 ⇒ 它的待发取消是否生效 | 1 | 1(**回归守卫**,两边同值) |

⚠ **可见面变更(breaking)**:只有「**跨任务**的 `SHIELD` 与 `TASK_CANCEL_PARENT()`
交错」这一支 ⇒ 修前那次取消**被吞**(假阴性)⇒ 修后**生效**。**单任务**形态逐字不变
(既有 H.4.4 金标准 `FALSE` / `TRUE` / `1` 不动)。

**契约**:新表 `x02a_shield_per_task_contract.rs` **9 例**(跨任务遮挡 / 待发取消不被吞 /
两个任务各自生效 / 单任务金标准 / 嵌套屏蔽 / `TASK_CANCEL(handle)` 不被推迟 / 顶层兜底)。
**负向验证**:把 per-task 改动**回退**(共享深度 + 共享待发取消槽)⇒ **4 条红**
(`a` 读数 `0` / `b` 读数 `0` / `d` 读数 `[1, 0]` / `h` 读数 `0`),其余 **5 条保持绿**
(单任务金标准 / 嵌套屏蔽 / `TASK_CANCEL(handle)` / 顶层兜底 / A 自己的待发取消)
⇒ 断言确实钉住了 per-task 化、不是恒真 ⇒ 还原后 9 例全绿。

**全量门禁**(`impl`):`cargo test --locked --workspace --all-targets --no-fail-fast` ⇒
**89 个测试目标 / 2894 例 / 0 失败**;`cargo clippy --locked --workspace --all-targets -- -D warnings`
⇒ **0**;`cargo fmt --all -- --check` ⇒ **0 diff**;`cargo doc --no-deps`(`RUSTDOCFLAGS='-D warnings'`)
⇒ **0 error**;规范守卫(hygiene / needle scope)⇒ 绿;`check-appendix-a` ⇒ **OK**;
`python script/build_spec_reference.py` ⇒ **幂等**(哈希不变)。

### X-02b 收敛:取消请求按「**到达时点**」判定 ⇒ `SHIELD` 挡住跨任务取消(⚠ breaking)

⚠ **三条语义裁决已于 2026-10-23 落定**(业主拍板,三条全取推荐项):**Q1 按目标**判定 /
**Q2 只有作用域退出豁免**、兄弟失败连带受屏蔽保护 / **Q3 帧口径**、挂起点即结束。
⇒ 可实现规格(S1–S7)与规范侧清单见
[`docs/active/x02-shield-cancel-arrival-time.md`](docs/active/x02-shield-cancel-arrival-time.md) §6.7 / §6.8。

**旧口径**(来源判据):只有**屏蔽块内发起**的 `TASK_CANCEL_PARENT()` 被推迟;**别的任务**
用 `TASK_CANCEL(handle)` 发起的取消**立即**生效。
**新口径**(到达时点 + 按目标):取消请求**到达目标**时,若该目标正处在 `SHIELD` 块内 ⇒
**推迟**到它退出自己的屏蔽块;否则立即生效。`TASK_CANCEL_PARENT()` 取消「自己 + 同域兄弟
+ 子树」时,**每个目标各自**按自己的状态判定。

**实现**(S1–S7):
* **S1** 投递点(`request_task_cancel_with_reason_policy`)逐目标判定 ⇒ 新增
  `ShieldPolicy{Deferrable, Bypass}`;命中屏蔽 ⇒ 记进**该目标**的待发槽、**不**置
  `cancel_requested`。
* **S2** `builtin_task_cancel_parent` **不再提前返回** ⇒「推迟自己」与「取消子树」拆开。
* **S3** 兄弟失败连带改走逐目标投递 ⇒ **受屏蔽保护**。
* **S4** 作用域退出的**硬清理**(`force_cancel_scope_children`)⇒ 豁免屏蔽。
* **S5** ⚠ `TASK_IS_CANCELLED()` 的第三条路径(父作用域 `cancelled`)**必须尊重本任务的
  屏蔽状态** ⇒ 不改这里,被屏蔽的任务照样读出「已取消」⇒ 屏蔽形同虚设。
* **S6** 释放点 = 屏蔽块退出(X-02a 已就位);**S7** **不新增任何取消检查点**。

**读数**(改动后二进制实跑):

| 形态 | 观察点 | 改动后 |
|---|---|---|
| 跨任务 `TASK_CANCEL(handle)` 命中**被屏蔽**的目标 | 屏蔽块内 / 退出后 | **`no` / `yes`** ⇒ 推迟 |
| 跨任务 `TASK_CANCEL(handle)` 命中**未屏蔽**的目标 | 目标读数 | **`yes`** ⇒ 立即生效(不变) |
| 被屏蔽的任务发 `TASK_CANCEL_PARENT()` | 同域未屏蔽的兄弟 | **立即被取消** ⇒ `AWAIT(兄弟)` 得到 `ERR(kind="Cancelled")` |

⚠ **可见面变更(breaking)**:① 跨任务取消**不再**绕过被屏蔽的目标 ⇒ 改为推迟;
② 被屏蔽的任务发 `TASK_CANCEL_PARENT()` ⇒ **同域未屏蔽的兄弟立即被取消**(今天整棵子树
一起推迟)—— ⚠ 兄弟来不及跑起来 ⇒ `AWAIT(兄弟)` 得到 `ERR(kind="Cancelled")`;
③ 兄弟失败连带 ⇒ 被屏蔽的兄弟**受保护**(今天立即被杀)⇒ 它的清理代码能跑完。
⚠ **单任务**形态**逐字不变**(H.4.4 金标准 `FALSE` / `TRUE` / `1`)。

⚠ **三条裁决里只有 Q1 有可观察效果**:Q2 在 S4 存在的前提下与「兄弟连带也豁免」
**观察等价**(兄弟失败连带只在作用域退出那一刻触发,那时无论如何都会被 S4 收尾)
⇒ Q2 甲案是**防御性**口径;Q3 表现为**不改 `YIELD` 那一支**。

**契约**:新表 `x02b_arrival_time_contract.rs` 3 例(命中被屏蔽目标 ⇒ 推迟 / 命中未屏蔽
目标 ⇒ 立即 / 被屏蔽的发起方 ⇒ 同域未屏蔽兄弟立即被取消);⚠ 既有 `x02a_shield_per_task_contract.rs`
有 **3 条**因判据切换而失效 ⇒ 逐条判定「期望还代表规范吗」⇒ 均为**过时**(规范侧口径
变了,不是判据被放宽)⇒ 改期望/改程序 + 写明**为何变**(`b` 程序改写、`d` 改为钉新口径、
`g` 期望 `1` → `0`);另补 **2 条 S4 白盒守卫**(`wlwl-eval` 的 `runtime.rs` 单测)——
⚠ `SHIELD` 那一支在用户程序里**不可达**(`AWAIT` 不停泊调用方;阻塞型通道 park 会展开
屏蔽帧)⇒ 只有「已被请求取消 + 仍停泊」这一支能被观测 ⇒ 在调度器层直接构造状态来钉。

**负向验证**(回退本批的实现改动 ⇒ 共享深度 + 来源判据):
`x02b` 表 **2 条红**(`a` 读数 `[1, 1]`、`c` 读数 `0`;`b` 非判别 ⇒ 绿);
`x02a` 表 **2 条红**(`d` 读数 `0`、`g` 读数 `1`)⇒ 断言确实钉住了新判据、**不是恒真**
⇒ 还原后全绿。

**全量门禁**(`impl`):`cargo test --locked --workspace --all-targets --no-fail-fast` ⇒
**90 个测试目标 / 2899 例 / 0 失败**;clippy `-D warnings` ⇒ **0**;fmt `--check` ⇒ **0 diff**;
rustdoc `-D warnings` ⇒ **0 error**;规范守卫(hygiene / needle scope)⇒ 绿;`check-appendix-a`
⇒ **OK**;`python script/build_spec_reference.py` ⇒ **幂等**(哈希随规范仓改动而更新,连续两次相同)。

### X-24 收敛:停泊子任务**不**活过词法 `SCOPE`(⚠ breaking)

**缺口**(一致性册 §5.4 的 X-24):停泊在通道上的子任务在作用域退出时**不**被终止 ⇒
`SCOPE` **在子任务仍存活时**就返回了 ⇒ 违并发册 §17.1「`SCOPE` 返回前**必须**等待其
子任务结束」。

**根因三处**(实测,`%TEMP%` 探针 + 契约表回退读数):
1. `pop_scope` 强关通道后只把停泊方 `enqueue` ⇒ 任务仍 `Suspended` ⇒ `begin_run`
   拒绝运行 ⇒ **从未真正重跑**(§17.2.3 的 E0054 / ChannelClosed **观察不到**);
2. 强关后**不**排干 ⇒ 即便唤醒也在 `SCOPE` 返回**之后**才跑;
3. 硬清理只覆盖「屏蔽块内 / 已被请求取消」⇒ 停在**外层作用域**通道上的子任务
   永远醒不过来 ⇒ **泄漏**。

**修法(甲案「先放行、再兜底」——§17.2.3 与 §17.1 同时成立,不新增规范豁免)**:
1. 强关后改走 `wake_channel_op_waiter`(`Suspended → Pending` + 入队);
2. **再排干一次** ⇒ 停泊方在 `SCOPE` 返回**之前**重跑完(发送方 `SEND` → `E0054`、
   接收方 `RECV` → `ERR(kind="ChannelClosed")`);
3. `force_cancel_scope_children` 放宽到**所有**仍非终态子任务(与 X-23 硬清理同一条
   路径)⇒ 外层通道停泊 / 仍被屏蔽等收不到唤醒的形态一律 `Suspended → Cancelled`。

⚠ **可见面变更(breaking)**:① 本作用域通道上的停泊子任务从「SCOPE 返回**后**才
(有机会)重跑失败」变成「返回**前**已失败」;② 外层通道上的停泊子任务从「泄漏到
外层」变成「**被终止**」⇒ `AWAIT` 从 `E0053` 变成 `ERR(kind="Cancelled")`;
③ `AWAIT` 停泊发送方从 `E0053` 变成 **`E0054`**(§17.2.3 本来要求的码)。

**契约**:新表 `x24_scope_exit_child_lifetime_contract.rs` **4 例**:
`a` 体重放计数(修前 `1` / 修后 `2`)、`b` 外层通道 `AWAIT` 的 `kind`(修前 `E0053` /
修后 `Cancelled`)、`c` 回归守卫(不带崩主程序)、`d` 停泊发送方的 `E0054`(修前 `E0053` /
修后 `E0054`)。⚠ 白盒 `force_cancel_scope_children_leaves_an_uncancelled_parked_child_alone`
的期望**已过时**(旧范围「不动它」不再代表 §17.1)⇒ 改为
`force_cancel_scope_children_terminates_a_plain_parked_child`,注释写明**为何变**。

**负向验证**(回退实现 ⇒ `pop_scope` 只 enqueue + 硬清理收窄):
`x24` 表 **3 条红**(`a` `1`、`b` `E0053…`、`d` `E0053…`);`c` 非判别 ⇒ 绿
⇒ 断言钉得住、**不是恒真** ⇒ 还原后全绿。

**全量门禁**(`impl`,X-24 落地后):
`cargo test --locked --workspace --all-targets --no-fail-fast` ⇒ **91 个测试目标 / 2903 例 / 0 失败**;
clippy `-D warnings` ⇒ **0**;fmt `--check` ⇒ **0 diff**;rustdoc `-D warnings` ⇒ **0 error**;
规范守卫(hygiene **7+7** / needle scope **7+7**)⇒ 绿;`check-appendix-a` ⇒ **OK**;
`python script/build_spec_reference.py` ⇒ **幂等**。

### 审计 F1 / F2 修复:E0103 报告位置 + 插值错误提示(2026-10-24)

来源:`docs/history/v1.0-alignment-audit-20261024.md` 的交叉验证发现。

**F1 — `E0103` 报告 span 在「绑定 + 裸实参」形态下退化为块首句**:
作用域结束的丢弃点统一用「作用域块的 span」(总是落在**首句**)⇒ 当持有未消费
`ERR` 的绑定来自**后面的** `LET` 时,诊断指到无关语句(甚至 `1:1` dummy)。
⇒ 修法:`Binding` 新增 **`origin_span`**(值表达式位置 / 形参对应的调用点),
`report_pending_unconsumed` **优先钉它**。条数与丢弃点语义**逐字不变**。
⚠ 既有契约 `x21_a_callee_that_reads_the_rest_binding_moves_the_token_with_the_value`
的期望从「模块作用域结束 = 第 1 行」改为「绑定建立处 = 第 2 行」——旧期望
**不再代表**「可定位到出事绑定」这一目标(注释写明为何变)。
新契约表 `e0103_span_contract.rs` **7 例**(四象限:裸/容器 × 语句/绑定 +
块首无关定义 + 首句回归 + 不许多报)。**负向验证**:退回块 span ⇒ **4 条红**
(全部绑定形态),语句/首句回归 **3 条绿** ⇒ 判别力成立。

**F2 — 插值内 `\"` 的 `E0002` 无提示**:`${...}` 体内是**表达式**上下文,
字符串实参应写 `"` 而不是 `\"`(后者是外层字符串的转义,会打乱括号配对)。
⇒ 三条 `E0002` 消息补上修复提示。语义未改。

**全量门禁**(F1/F2 落地后):**92 个测试目标 / 2910 例 / 0 失败**;
clippy 0;fmt 0;rustdoc 0;守卫 7+7;附录 A OK。

### 发布说明:v1.0 规范下**未收敛**的差异(截至本批)

规范仓一致性册 §5.4 登记的「实现待收敛」项**必须**在发布说明里**显式列出**,并给出
收敛路线(规范仓 README 的同款要求)。v1.0 发布时,以下各项**允许存在** —— 它们
**不**构成 v1.0 相对 v0.11 的**语义变更** ⇒ 按 §3.5.1 登记在 §5.4 即可,**不进**
总纲 §0.9 的差异表(那一列只放语义变更)。

⚠ **相对 v0.12.0 的发布说明,本批的变化**:`X-01`、`X-06`(集合化那半步)、`X-07`、
`X-16`、`X-17`、`X-18`、`X-19`、**`X-20`** **已收敛**;`X-06` 的**身份**那半步也
**已闭合** —— ⚠ **已裁决**(2026-10-23,业主取**甲**「**值语义即新身份**」)⇒ 规范册
§8.6.5 已落该口径 ⇒ 参考实现按**内容**判定身份、集合又是**多重集**(`union_unconsumed`
**不去重**)⇒ 两个内容相同、来源不同的 `ERR` 占**两个**元素 ⇒ 逐个报告时**就是两条**
⇒ **恰好与甲一致** ⇒ **不需要**给 `Value::Err` 加身份(`Value::Err(` 那 111 处
**一处不用动**)⇒ ⇒ `X-06` **全步收敛**;**本批新发现并已收敛** `X-20` 一条(标准库
成员的实参边界)。
⚠ **本批(续)又收敛三条**:`X-15`(字典深度相等的 **O(n) 最坏** —— 纯代价,判定结果不变)
+ **新登记并收敛** `X-21`(`*args` rest 形参的令牌通道)与 `X-22`(`TRY` / `OR_DIE` 的
`OK(ERR(..))` 剥壳形态 —— **先在规范册裁决**,再落实现)⇒ ⇒ **未收敛清单只剩
`X-02` 与 `X-14` 两条**。
⚠ **本批(再续)收敛 `X-02`** —— 判据从「取消的**来源**」改成「取消**到达时点**是否在
屏蔽块内」(**按目标**判定)。⚠ **随后新登记 `X-24`**(停泊子任务活过 `SCOPE`)⇒
当时的未收敛清单 = `X-14` + `X-24`。
⚠ **本批(X-24)收敛 `X-24`** —— 作用域退出时停泊子任务被真正收尾(甲案:先放行
§17.2.3 的重跑、再兜底硬清理)⇒ ⇒ **未收敛清单只剩 `X-14` 一条**(业主已定不排期)。

⚠ **七条**因本批而**不再**出现在本表里的项:
① **`X-20`** —— 落地前只产生**假阴性**(漏报);落地后按甲它会让 `PRINT(a)` 这类
**开始报** ⇒ 这是**可见面变更**(breaking)⇒ 已在上节与本节的「可见面变更」处列出;
② **`X-01`** —— 落地前只产生**假阴性**(停泊的发送方永远不被唤醒 ⇒ `AWAIT` 报
`E0053`);落地后它的 `SEND` 以 **`E0054`** 失败(并发册 §17.2.3 规定的那个码)⇒
⇒ 也是**可见面变更**;
③ **`X-15`** —— 落地前只影响**代价**(判定结果一直是对的)⇒ **不是**可见面变更;
④ **`X-21`** —— 落地前**假阴性**(富余实参持有的未消费 `ERR` 一条都不报);落地后
**开始报** ⇒ **可见面变更**(breaking);
⑤ **`X-22`** —— 落地前**假阴性**;落地后**开始报** ⇒ **可见面变更**(breaking);
⑥ ⚠ **`X-02`** —— 落地前跨任务取消**直接生效**(被屏蔽的目标**挡不住**)⇒ 落地后
**改为推迟** ⇒ **可见面变更**(breaking);且被屏蔽的发起方发 `TASK_CANCEL_PARENT()`
⇒ **同域未屏蔽的兄弟立即被取消**(今天整棵子树一起推迟)⇒ 也是 breaking
⇒ 已在「X-02b 收敛」一节逐条列出;
⑦ ⚠ **`X-24`** —— 落地前停泊子任务活过 `SCOPE`(假阴性/泄漏);落地后被真正收尾
⇒ **可见面变更**(breaking,见「X-24 收敛」一节)。

| 编号 | 差异 | 收敛路线 |
|---|---|---|
| **X-14** | 别名在**写序列内部**每轮重建的形态(`LET(e, d)` 写在循环体里)⇒ 每次写仍 O(k) ⇒ 总工作量二次 | 需要**结构共享**(持久化映射)⇒ 属另一个决策(见规范仓路线图册 §3.3 的候选)⇒ **业主已定不排期** |

⚠ **X-24 已在本批收敛**(2026-10-24,甲案「先放行、再兜底」)⇒ 见上节;本表**只剩
X-14**。⚠ **X-14 只产生假阴性**(漏报)—— 它是**性能**形态的偏差(不产生误报,只是那条写
序列达不到 §18.3.1 第 2 条的序列级摊还)⇒ 同一形态的**别名在写之前建立**已达标
(确定性计数:拷贝 = k0 = 1,之后原地写 ⇒ c = 2)。
⚠ **X-15 已在本批收敛**
(键比较 2n;判定结果**未变** ⇒ 纯代价)⇒ 见上节。⚠ **X-02 已在本批收敛**
(判据改成「到达时点 + 按目标」⇒ **可见面变更**,见上节)。⚠ **X-23 已于 2026-10-24
消解**(豁免写进并发册 §17.3 的 SHIELD 语义**第 6 条** ⇒ §3.5.1 的「缺口要清零」闸门
清零)。

⚠ **一处「推荐上界」未达标(按规范 §18.4.1 披露,不是规范性差距)**:循环内
`SET(acc, +(acc, t))` **累积字符串**(规范 §18.4 的推荐上界 O(n) 摊还)在参考实现
上仍是**二次**的 —— `+` 每次都产出**新串**(没有字符串累加 cell)。§18.4.1 明确
允许「**可以**选择不识别(则该惯用法退化为 O(n²)),但**必须**在实现文档中说明」
⇒ 此处即为该说明。⇒ 相对地,**数组**的累加形态**已达标**(可变累加器 cell ⇒
原地累积)。⇒ 想要字符串也达标,需一个**字符串累加器 cell**(与数组版同款)。

## [v0.12.0(未发布的历史痕迹)] — 2026-10-22(⚠ **未经业主同意的 tag**，勿作发布基线)

> ⚠ **本段 tag `v0.12.0` 系先前 agent 私自打出，业主 2026-10-24 明确表示未同意。**
> 版本线以 **`v0.11.4`** 为准；本段内容仍作**语义变更的编年史**保留，但**不是**
> 有效发布点。tag/Release **不移动**（历史痕迹）。
> ⚠ **本段不是 v0.12.0 的正式段** —— 正式段在本文件**顶部**
> （`## [0.12.0] — 公共面扩充 17 员 + 传输层等待归宿主挂起`）。
> 2026-10-25：**v0.12.0 由业主重新决定发布**，内容见顶部正式段；
> 本段**只**保留当初那批 E0103 改动的编年史（与本版内容无关）。

**v1.0 语义落地(第一批)**:实现 v1.0 规范的「未消费 `ERR` 的报告」(`E0103`)。
规范在**独立仓** `Laffinty/wlwl-spec-v1`(基线 `46b153b`),本仓实现的语义以它为准 ——
本仓的码表守卫(`every_registered_code_is_covered_by_the_spec`)现在**直接读规范文本**,
CI 把规范仓按钉死的提交取下来(见 `.github/workflows/ci.yml`)。

### ⛔ 本批的可见面变更(breaking,按 ADR-0023 §v0.x 2 申报)

| # | 变更 | 之前 | 之后 | 依据 |
|---|---|---|---|---|
| **B-1** | **未消费的 `ERR` 被丢弃** | **静默丢弃**:作为表达式语句被求值并丢弃的 `ERR`,**只有当它是最后一条语句时**才走顶层逃逸(`E0102`);更早出现的**不产生任何诊断** | 产生 **`E0103`**(退出码 1)。丢弃点恰好三处:① 语句值丢弃 ② 绑定被覆盖(重绑定 / `SET`)③ 作用域结束仍未消费 | 语言核心册 §8.5、§8.6.2 |

⚠ **这一条改变既有程序的可观察行为**,不走弃用两步。

**为什么**:v0.11 §8.5 的「静默丢弃」不是理论洁癖 —— `D11-019` 就是它造成的真实缺陷:
`SORT` 的比较器返回 `ERR` 时,错误处理代码把 `ERR` 直接放在 `||` / `IF` 的**条件位**上;
按 §8.2 条件位不消费 `ERR`,于是它落进静默丢弃路径 ⇒ **零诊断 + 原序返回** ——
比较器的错误凭空消失。v1.0 删掉这条限制(规范仓总纲 D-05),代之以可判定规则。

**怎么改**:一个 `ERR` 现在**必须**被三者之一接住 ——

1. **取走型消费者**取走它:`ERR_PAYLOAD` / `UNWRAP_OR` / `OR_DIE` / `TRY` / `UNWRAP` / `WRAP` / `EXPECT_ERR`;
2. **显式丢弃**:`LET(_, expr)` —— 本语言**唯一**的显式丢弃构造;
3. 走到**顶层**成为程序值(走 `E0102`)。

**只观察不处置不够**:`IS_ERR(x)` / `IS_OK(x)` / `BOOL(x)` / `TYPE(x)` 是**观察型**消费者,
**不解除**未消费状态 —— 观察之后要么取走、要么显式丢弃。这是与 v0.11 差别最大的一处:
过去「看一眼然后不管」是合法的,现在它会在作用域结束时报 `E0103`。

**已知缺口**(只产生**假阴性**即漏报,不产生假阳性即误报;已登记在规范仓一致性册
§5.4 的 X-06 / X-07):① 一个绑定只持有一个未消费令牌 ⇒ 容器里同时装两个未消费 `ERR`
时只记得住一个;② 标准库回调的实参通道不带令牌 ⇒ 回调内未消费的 `ERR` 不报 `E0103`。

### X-05 收敛:`std.io::INPUT` 接受可选 prompt(非 breaking)

| # | 变更 | 之前 | 之后 | 依据 |
|---|---|---|---|---|
| **B-2** | `wlwl:std.io::INPUT` 的元数 | **只收 0 个实参**(给 1 个即 `E0022`) | 收 **0 或 1** 个实参;给了 prompt 就**必须**是 `STRING`(否则 `E0030`),且**必须**先把它写到标准输出再读 | 标准库·系统册 §1.1、§1.2 |

**为什么**:规范仓一致性册 §5.4 登记的 **X-05**。规范(标准库·系统册 §1.1)写的是
`INPUT(prompt?) -> STRING`,§1.2 要求「两条路径的可观察行为**必须**逐字相同」——
而实现只收 0 个实参 ⇒ **加一行 `IMPORT` 就改变了程序的行为**(同样的调用,不导入时
能跑,导入后报 `E0022`)。第三方二次审计把它列成「API 分叉」;核验后确认
**分叉在实现侧,不在规范侧**(规范三处本来就一致)⇒ 改的是**实现**。

⚠ **非 breaking**:元数下界仍是 **0**,0 个实参的调用**照旧**;此前报 `E0022` 的
1 个实参调用现在**能跑**。⇒ 纯扩宽,既有程序不受影响。

**EOF 的口径不变**:一个字节都没读到 ⇒ 返回 `""`;只有 I/O 错误才报 `E0060`
(标准库·系统册 §1.2「读到 EOF 与读失败是两件事」)。

### EOF 口径收敛:内建 `INPUT` 在 EOF 上返回空串(breaking)

| # | 变更 | 之前 | 之后 | 依据 |
|---|---|---|---|---|
| **B-3** | 全局内建 `INPUT` 在 **EOF**(一个字节都没读到)时的返回值 | `ERR("EOF")` —— 一个 `ERR` 值,顶层逃逸为 `E0102` | **`""`**(空串)—— 与 `std.io::INPUT` **逐字相同**,不是失败 | 标准库·系统册 §1.2 |

**为什么**:规范仓一致性册 §5.4 的 **X-08 / X-09**。标准库·系统册 §1.2 要求
「两条路径的可观察行为**必须**逐字相同」,而 EOF 上内建产 `ERR` 值、成员产空串
⇒ 分叉;语言核心册 §10.2 又**只字未提**内建 `INPUT` 的 EOF 口径(那是 X-08 的
规范文本缺口)⇒ 两处一起补:规范补一句成文,实现收敛成 `""`。

⚠ **breaking**:此前靠 `IS_ERR(INPUT())` 判 EOF 的程序,现在拿到的是**空串**
⇒ **EOF 与「用户输入了空行」不可区分**。这是选 `""` 的代价(另一条路是让
成员侧产 `ERR("EOF")`,那要改标准库册 §1.1 的「本章不产 `ERR` 值」与 §1.2 的
EOF 条款 —— 改两册条款比改一处实现重)。

⚠ **读失败不变**:内建层的读失败仍产 `ERR("NoInputAvailable")`(内建层的失败
模型:内建产 `ERR` 值);`std.io::INPUT` 的读失败仍是原生诊断 `E0060`(成员
契约)。两层的失败模型本就不同,**不是**待收敛的分叉。

⚠ **守卫从单元层搬到 CLI 层**:原单测 `b15_input_no_stdin_returns_err` 断言
「EOF → `ERR` 值」,随本次收敛**作废**且被**删除** —— 它在单元层根本测不了
(`builtin_input` 直接读进程 stdin,单测喂不了它,本地终端上还会挂住等输入)。
EOF 的守卫搬到 `crates/wlwl-cli/tests/io_input_parity.rs`:那里能真的喂 stdin,
两条路径在 EOF 上都被钉住。

**验证**:crate 内单测 3 条(元数错 / 类型错 / 元数下界是 0)+ CLI 层
`io_input_parity.rs` 5 条 —— 其中主守卫是**与全局内建逐字节对拍**(同一份程序,
带 `IMPORT` 与不带,喂同一份 stdin,stdout 必须逐字节相同)。

### 可见面变更:不可变绑定的循环写恢复报 `E0024`(修 v0.11.3 的一个缺陷)

| # | 变更 | 之前 | 之后 | 依据 |
|---|---|---|---|---|
| **B-4** | `LET(a, [])`(不可变)+ 累加器形状的循环里 `SET(a, …)` | **不报错**(程序跑通) | 报 **`E0024`**(`cannot SET immutable binding`) | 语言核心册 §3.1 / §3.4 |

**为什么**:v0.11.3 的可变累加器优化(`ADR-0032`)在**包装**累加器时用了
`set_local_mut` ⇒ 它的语义是「在**最内层作用域插入一个新绑定**」且那个绑定是
**可变**的 ⇒ ⇒ 不可变绑定被**静默改成了可变** ⇒ 规范 §3.1「可变性建绑定时定、
**永不变**」被破坏 ⇒ 程序从「报错」变成「不报错」。

⚠ **这是修缺陷,不是改语义**:规范一直说该报 `E0024` ⇒ 修的是实现与规范的不符。
⚠ 但它**改变已发布行为** ⇒ 此前靠「不报错」跑通的程序现在会报 `E0024` ⇒ 修法是
把那些程序的累加器声明改成 `LET MUT`(与 §3.1 一致)。
⇒ 回归守卫:两个累加器形态各一条契约用例(不可变绑定 ⇒ 必须 `E0024`);
**负向验证**:把优化退回缺陷形态 ⇒ 守卫必红(实测输出 `Array([1, 1, 1])` = 程序跑通
了,正是缺陷形态)。

### 非 breaking 的性能形态:字典累加器(§18.3 的写上界)

循环里逐次 `SET(d, INDEX_SET(d, k, v))` 写 n 个键,现在被识别成**累加器形态** ⇒
写**原地**进行 ⇒ 确定性计数实测:n = 16 ⇒ 比较 30 / **拷贝 0**;n = 64 ⇒ 比较 126 /
**拷贝 0** ⇒ §18.3.1 的序列级判据「n 次写 ⇒ 总工作量 ≤ c·n」在累加器形态下成立。

⚠ **可观察面零变化**:`TYPE` 报 `"DICT"`、渲染逐字节相同(含插入序)、`==` 的两种
口径都覆盖、JSON 同形。⚠ **别名形态仍未达标**(`LET(e, d)` 后再写 `d` 每次仍是 O(n))
⇒ 那是**值语义的推论**(§19.1:旧值可观察 ⇒ 写必拷贝)⇒ 已登记在规范仓一致性册
§5.4,待裁决。

### 可见面变更:`ai` 包真实后端的失败码按规范重新归类(breaking)

| # | 变更 | 之前 | 之后 | 依据 |
|---|---|---|---|---|
| **B-5** | `ai` 包(`ASK` / `EMBED` / `COMPLETE` / `ASK_STREAM` / `ASK_ALL`)在**真实后端**下的失败诊断码 | 401 / 403 → `E0081`;404 → `E0083`;响应体解析失败(不是合法 JSON / 缺 `choices[0].message.content`)→ `E0083` | 401 / 403 → **`E0080`**;404 与其余 4xx → **`E0093`**;响应体解析失败 → **`E0081`** | 标准库·官方包册 §9.2 |

**为什么**:规范仓一致性册 §5.4 的 **X-11**。官方包册 §9.2 的失败口径表写「推理请求
失败 → `E0080`」「响应协议不符 → `E0081`」,而实现把 `E0081` 用作**认证失败**、
把 `E0083` 用作**响应解析失败** ⇒ 与规范**错位一格**。同期规范侧把 401 / 403 的
归类**钉死**为 `E0080`(此前两解并存:既是「HTTP 4xx」`E0093`、又是「推理请求失败」
`E0080`;那是 **X-10** 的规范文本缺口,已补)⇒ 本笔把实现对齐到规范。

⚠ **breaking**:按状态码分诊的程序会看到不同的码 —— 401 / 403 不再是 `E0081`,
而是 `E0080`。

⚠ **证据的边界(如实登记)**:这三处在 `real-ai` feature 之内 ⇒ **默认构建与 CI
跑不到** ⇒ 本笔只有**编译检查**作证(`cargo check -p wlwl-std --features real-ai`),
**行为证据缺**。⚠ 诊断**消息文本逐字未改**(只改码)。



### 发布说明:v1.0 规范下**未收敛**的差异(一致性册 §3.5.1 第 ③ 条)

规范仓一致性册 §5.4 登记的「实现待收敛」项**必须**在发布说明里**显式列出**,并给出
收敛路线。v1.0 发布时,以下五项**允许存在** —— 它们**不**构成 v1.0 相对 v0.11 的
**语义变更** ⇒ 按 §3.5.1 登记在 §5.4 即可,**不进**总纲 §0.9 的差异表(那一列
只放语义变更)。

| 编号 | 差异 | 收敛路线 |
|---|---|---|
| **X-01** | 通道关闭**不**唤醒停泊的**发送方** ⇒ 它停在通道操作上,`AWAIT` 得到 `E0053`(无对端唤醒) | `channel.rs` 的 `close()` 改为同时 swap 出**发送方**停泊队列 ⇒ 它们以 `E0054` 失败(与接收方的 `ChannelClosed` 对称) |
| **X-02** | `SHIELD` 只推迟 `TASK_CANCEL_PARENT()` 发起的取消;另一任务 `TASK_CANCEL(handle)` 发起的取消**不被**推迟 | 判定从「取消的**来源**」改成「取消**到达时点**是否在屏蔽块内」 |
| **X-06** | 一个绑定只持有一个未消费 `ERR` 令牌 ⇒ 容器里同时装两个未消费 `ERR` 时只记得住一个,第二个**静默丢失** | 值层的未消费状态从「每绑定一个布尔令牌」改成「每绑定一个**令牌列表**」 |
| **X-07** | 标准库回调(如 `MAP` 的 `f`)的实参通道**不携带**令牌 ⇒ 回调内未消费的 `ERR` **不报** `E0103` | 回调调用点把 `Outcome` 的未消费标记**一起**传下去 |
| **X-14** | 别名在**写序列内部**每轮重建的形态(`LET(e, d)` 写在循环体里)⇒ 每次写仍 O(k) ⇒ 总工作量二次 | 需要**结构共享**(持久化映射)⇒ 属另一个决策(见规范仓路线图册 §3.3 的候选) |
| **X-15** | **深度相等(字典)**是 **Θ(n²)**(键比较 = n(n+1)/2)⇒ 不满足规范 §18.3 的 O(n) 最坏 | 让 `DictInner` 已有的**键→位置**哈希索引参与相等判定(哈希命中把候选缩到 O(1) 条,再在候选内做**集合口径**核对)⇒ 期望 O(n);⚠ **语义红线**:相等是**集合口径**(不看顺序)⇒ **不得**简化成「同序逐位比较」,否则 `{"a":1,"b":2}` 与 `{"b":2,"a":1}` 会被判不等 |

⚠ **前四项只产生假阴性**(漏报),**不产生假阳性**(误报)⇒ 已发布的程序不会因为
它们而改变行为。**X-14** 是**性能**形态的偏差(不产生误报,只是那条写序列达不到
§18.3.1 第 2 条的序列级摊还)⇒ 同一形态的**别名在写之前建立**已达标(确定性
计数:拷贝 = k0 = 1,之后原地写 ⇒ c = 2)。**X-15** 同样只影响**代价**,不改变
判定结果(两个字典仍然按**集合口径**判等,只是判等的代价是二次的)。

⚠ **一处「推荐上界」未达标(按规范 §18.4.1 披露,不是规范性差距)**:循环内
`SET(acc, +(acc, t))` **累积字符串**(规范 §18.4 的推荐上界 O(n) 摊还)在参考实现
上仍是**二次**的 —— `+` 每次都产出**新串**(没有字符串累加 cell)。§18.4.1 明确
允许「**可以**选择不识别(则该惯用法退化为 O(n²)),但**必须**在实现文档中说明」
⇒ 此处即为该说明。⇒ 相对地,**数组**的累加形态**已达标**(可变累加器 cell ⇒
原地累积)。⇒ 想要字符串也达标,需一个**字符串累加器 cell**(与数组版同款)。

## [v0.11.3] — 2026-10-20(正式版)

⚠ **本版 = 同日预览版 `v0.11.3_rc1` + 一个追加批次**(第三方功能性审计的 11 条落地,见下)。
预览版自己的记录在本文件末尾的 `v0.11.3_rc1` 段 —— **两份都在,别只读一段**。

**本批来源**:第三方**功能性审计**(`docs/history/wlwl-v0.11.3-functional-audit.md`,2026-10-20)。
第三方交来 11 条(7 条功能缺陷 F-01~F-07 + 4 条文档不一致 D-01~D-04);业主逐条复核
**0 条否证、4 条修正定性**(F-04 降为文档缺口、F-05 不是违规范、F-06 属钉文档不改实现、
F-07 事实有误)、**3 条否决其建议的某一支**(F-02 的「改规范」、F-04 的「放行 FUNCTION」、
F-06 的「改实现」)⇒ 本批落地 **5 条 breaking 行为变更 + 1 条确定性修复 + 5 条纯文档条目**
(规范文本修订:标准库规范 **8** 处 + 语言规范 **2** 处 + **4** 册 ADR)。

⚠ **这一批的判定纪律**:第三方报告是**黑盒**的(只读外部可观察行为),复核时补读了实现
源码以定位根因,但**每条现象判定仍然只用外部可观察行为**;且**期望值一律手写自规范**,
不从实现输出反推。完整核对表见审计报告 §0.2。

### ⛔ 本批的可见面变更(breaking,按 ADR-0023 §v0.x 2 申报)

⚠ **这一段是本批最需要读的部分。** 五条都**改变既有程序的可观察行为**,其中三条是
**码/类型/文案对位错了**(实现与规范写的不是一条),两条是**规范本来就这么写而实现没照做**
(修的是实现,不是语义)。它们**不走弃用两步**。

| # | 变更 | 之前 | 之后 | 依据 |
|---|---|---|---|---|
| **B-1** | `wlwl:std.io::PRINT` / `PRINT_ERR` 的**容器渲染** | JSON 风格:`[1,2,3]` / `{"a":1,"b":"x"}` / `OK(1)` → `1`;**字典键序被改写为字典序** | 语言规范 §2.5 风格:`[1, 2, 3]` / `[a: 1, b: x]` / `OK(1)`;**字典键序 = 插入序** | 语言规范 §2.5 + 标准库规范 §1(语义列**点名引用** §2.5) |
| **B-2** | `WRITE_FILE` **写失败**的诊断码 | `E0061` + 消息 `WRITE_FILE: file not found (…): <os error>` | **`E0062`** + 消息 `WRITE_FILE: cannot write (…): parent directory does not exist` / `permission denied` / `<os error>` | 标准库规范 §2.0(「写失败 → `E0062`」) |
| **B-3** | `std.math::MODE` 的**返回类型** | 恒 `FLOAT`(`MODE([1,1,2])` → `1.0`) | **原样保留元素类型**(纯 `INTEGER` 数组 → `INTEGER`);**混装按语言规范 §2.2 提升为 `FLOAT`** | 标准库规范 §7.5(本批补口径) |
| **B-4** | `JSON_CHECK` 的 schema `type` 写成**数组** | `E0030`「schema "type" must be a STRING」 | **接受 `ARRAY[STRING]`**(任一匹配即过);畸形 schema 的文案改为「must be a STRING or an ARRAY of STRING」 | 标准库规范 §3.5(「**可写多个**(任一匹配即过)」) |
| **B-5** | `agent::CALL_TOOL` 的 **mock 回包** | `{ok: false, error_code: "E0093", message: "CALL_TOOL(t1) is mocked in v0.4. Process-level tool executor lands in v0.5 (plan §5.4)", params}` | `{ok: false, **mocked: true**, error_code: **NULL**, message: "CALL_TOOL(t1) is mocked: no process-level tool executor is wired yet. …", params}` | 标准库规范 §10(本批补形状) |

⚠ **B-1 的连带**:随包示例 `impl/examples/showcase.wll` 的输出从
`round-trip: {"name":"wlwl","ok":true}` 变成 `round-trip: [name: wlwl, ok: TRUE]` ——
JSON 风格**且**小写 `true`,而语言规范 §2.5 的口径是 `TRUE`/`FALSE`。
⚠ **B-2 的连带**:`E0062` 在**错误注册表**里的语义注释同步改成「写侧失败(父目录不存在 /
权限不足 / 其他)」—— 此前它写的是「file permission denied」,而标准库规范 §2.0 把它定义为
「写失败」⇒ **两册对同一个码的口径不一致**,只改实现不改注册表等于改完仍然对不上。
⚠ **B-3 的连带**:数值本身不变(§2.4 `==(1, 1.0)` 仍为 `TRUE`),**只有 `TYPE` 可见差异**;
但按 `TYPE` 分支或做类型标注序列化的调用方会受影响。
⚠ **B-4 的连带**:**文案**也是可观察面 —— 按错误文案做字符串匹配的下游会踩坑。
⚠ **B-5 的连带**:`E0093` 在标准库规范 §10 里是「HTTP 4xx」的**网络码**,而成员表写
`CALL_TOOL` 的失败是 `E0082`(工具错误)⇒ 旧回包**连成员表自己的口径都对不上**。

### Fixed

- **`E0020` 的「did you mean」候选提示现在是确定的** —— 之前同一份未定义名**连跑 8 次
  出 8 种不同的候选集合与排列**(不只排列随机,**选哪几个**也随机);现在按
  **(编辑距离, 名字字典序)** 全序排序后再截取 ⇒ 集合与排列都确定。
  - ⚠ 根因是排序键**只按距离**:候选池是 `HashSet`(迭代序每进程随机)⇒ 同距离项的相对
    顺序随机,再 `truncate` ⇒ 随机化「选哪几个」。修法是**把名字纳入排序键**,不换
    `HashSet`(换了会给每次诊断加 O(n log n) 开销)。
  - 守卫是**两条**:一条断言 hint 逐字节相同(64 次),一条断言**规则**(距离 → 字典序)而不
    钉字面量;另有一条**非空洞守卫**(候选必须 ≥2 个,否则「恒定」只是没得选)。
    ⚠ 两个 case 分别打「选哪几个」与「排列」—— 只测前者的话,「排序键不含名字」的实现
    仍能蒙混过关。
  - 排序口径**同时写进了语言规范 §11.1**(诊断的顺序也是结构的一部分),免得下一个改
    `similar_names` 的人改回随机序。
- **`std.io` 整模块移出 serde_json 中间表示** —— 三个成员直接收发真 `Value`,渲染走
  `Value::display()`,**与全局内建 `PRINT` 用的是同一个函数**(ADR-0022 方向:`io` 是六个
  仍走 `wrap` 的模块之一)。

### 文档(规范文本 —— 实现本身没变)

- **标准库规范 §1**:`PRINT("hello, ", "world")` 的示例与本节自己的「以空格分隔」规则冲突
  (规则推出两个空格)⇒ 改示例 + 补「分隔符**无条件**插入」的口径。
- **标准库规范 §3.4**:补第 6 条 —— `text` 是**原文切片**(字符串 token 含两侧引号;转义
  序列**不解码**),`start`/`end` 是**半开区间 `[start, end)`** ⇒ 顺带给出复原公式
  `SUB(s, start, end - start)`。§3.1 成员表加指针。
- **标准库规范 §2.2**:补 `LIST_DIR` 的**返回形状**(以 `p` 为前缀的完整路径、含目录条目
  本身、排序),并写明它与 `WALK_DIR` 的**相对路径坐标系不同** ⇒ 两个兄弟成员的坐标系
  终于在规范里写明了(**只钉文档,不改实现**)。
- **标准库规范 §7.5**:补 `MODE` 的**返回类型**口径(返回数组中那个元素的原值)⇒ 消掉
  `MODE` / `MEDIAN`(明写走 `f64`)/ `SUM`(§2.2 推导链)三者之间的不对称。
- **标准库规范 §9**:`ASK_STREAM` 的 `callback` 形参补约束(当前只接受 `NULL`,传函数值
  → `E0030`)+ §9.2 写明「该形参是为真流式预留的占位,真流式随 P4-D1b-001 立项」。
- **标准库规范 §10**:补 `CALL_TOOL` 的 mock 回包形状(与 B-5 同批落地)。
- **语言规范 §10.6 / §11.1** 与 **ADR-0010 / ADR-0021 / ADR-0025 / ADR-0032**:
  「语言规范 §6.6」的**悬空引用**改指 **ADR-0025 的『100 万次简单循环 < 30 s』符合性负载**
  (v0.11 的语言规范 §6 只到 §6.4,§6.6 已不存在)⇒ 另加脚注标明旧条款原为 v0.4 版。
  ⚠ **写死段号会把读者引到错的段**(ADR-0025 的 Q1 是 `DICT` 表示裁决,负载原句在 Q2),
  故**一律不带段号**。
- **审计报告**:`docs/history/wlwl-v0.11.3-functional-audit.md` 补业主复核结论、逐条根因、修复方案、
  验收命令与待裁决清单。⚠ **已归档**(`docs/plan/` 按本仓体例只留索引与状态)。

### 不做(留档,「不做」与「做」同权)

- **CLI 顶层读侧仍把「权限不足」报成 `E0062`** —— B-2 之后,`E0062` 在标准库规范 §2.0 里
  的定义是「**写侧失败**」;而 CLI 读用户给的程序文件撞上权限不足也报 `E0062` ⇒ 同一个码
  承担了两种条件,与本仓「一个码号只对应一种条件」的纪律相抵。
  ⚠ **本轮未改行为**:上一版(`v0.11.3_rc1` 段的 **B-6**)曾**显式申报**过这个选择,
  改它要回撤一条已申报的变更 ⇒ **需业主裁决**(建议:CLI 侧改用 `E0060` 通用 IO 错并标
  breaking)。已把口径分叉写进 `wlwl-cli/src/main.rs` 的文档注释,别让下一个人误读。
- **`TOOL` / `MODEL` 回包里的 `v: 0.4.0` 未动** —— 实现侧与标准库规范 §10.1 的示例**同源
  一致**(不是分叉),且 `v` 更像「**信封格式版本**」(该格式 v0.4 定下、此后未变)而不是产品
  版本 ⇒ 改它可能是改错。**需业主核实**。
- **`impl/` 源码里 5 处「spec §6.6」注释未动** —— 那指的是 **v0.4 的 `LET` 遮蔽条款**
  (v0.11 里是 §3.5),**与本次的性能负载不是同一条**;且这是**全仓性的 v0.4→v0.11 段号重编**
  问题(同一批注释里还有指 v0.4 §6.4 的),点着修一处不成体系 ⇒ **单独立项**。
- **`MODE` 的分组仍走 `f64` 桶键** —— `|n| > 2^53` 的不同大整数会被并进同一桶(**分组层面,
  非返回类型层面**)。修它必须动 `mode_key` 的键语义,而键语义有契约钉着
  (`mode_treats_signed_zeros_as_one_group_and_keeps_the_first_sign`)⇒ 不在本批范围。

### 门禁

`cargo fmt --all -- --check` **0 diff**;`cargo clippy --locked --workspace --all-targets -- -D warnings` **0**;
`cargo doc --workspace --no-deps`(`RUSTDOCFLAGS=-D warnings`)**0 error**;
`cargo deny --locked --all-features check` **exit 0**;
`cargo test --locked --test conformance -p wlwl-cli` **全绿**;
`cargo check --locked -p wlwl-std --features real-ai` **exit 0**;
`cargo test --locked --all-targets` **0 failed**(2345 条断言;**66** 条 `test result:` 行
= 48 集成 + 8 lib 单测 + 1 main 单测 + 9 doctest,另 **11** 个 bench 目标跑在测试模式、
不报结果行 ⇒ **77** 个测试二进制,上一版记的 75 是「64 + 11 bench」,本批新增 **2** 个);
probe **175** 用例(**未动** —— 本批**没有**新增探针夹具,`probe_case_count_matches_inventory` 守卫原样);
附录 A 由 `gen-appendix-a` 双向锁绿(本批**逐字节未变**);附录 G 逐字节不变;
契约表 `json` **40**(38 → 40,F-02 新增两条)/ `fs` 28(F-03 新增 3 条 `#[test]`,`Case` 计数不变)/
`str_math` 156(F-05 新增 4 条 `#[test]`,`Case` 计数不变)/ `ai_agent` 18(D-04 新增 4 条 `#[test]`,
`Case` 计数不变);**新建** `io_print_parity` 2 条(进程级 stdout 逐字节守卫)/
`diagnostic_determinism_contract` 4 条(hint 确定性守卫)。

⚠ **门禁本身抓出一处真错**:F-03 的新测试里 `set_readonly(false)` 触发 clippy 的
`permissions_set_readonly_false`(`-D warnings` 下是 **error**,本地 `cargo test` 看不见)⇒
已按平台分治修掉(Unix `set_mode(0o644)` / 非 Unix `set_readonly(false)` 局部豁免)。
⚠ 而 clippy 建议的「一律改 `set_mode`」在 Windows 上**编译不过**,且
`set_readonly(false)` 在 **Unix 上是空操作**(Unix 上它清的是**写位**)⇒ 那条清理逻辑本来就是坏的
—— **门禁抓到的不只是风格问题,是一条 Unix 上真会失效的清理路径**。

## [v0.11.3_rc1] — 2026-10-20(预览版)

**本批实测数字**:命名空间 **19**(不含官方包 `ai` / `agent`)/ **21**(含)——
标准库成员面 **214**;probe 用例 **175**;全局内建(附录 G)**逐字节不变**。

Spec: **wlwl-spec-v0.11 与 wlwl-stdlib-spec-v0.11 留在 v0.11**(同号发布,
ADR-0023 §v0.x 1)。⚠ **本批动了四处语言规范文本** —— 其中两处是**澄清既有条文**
(§4.5 的挂载点枚举补上「括号序列」;附录 A 注 ② 的机制描述改准),**不改任何
规范性行为条款**; ⚠ 第三处是**实质性的规范性变更**:§2.1 的字典键类型集合
**补上 `INSTANCE`**(按对象身份判定)—— 它**放宽**了一条约束(既有程序不受影响,
原先「规范未定义」的行为现在成文), 详见下面的 **B-3**; ⚠ 第四处同样落在附录 A:
`ReturnExpr` 产生式由 `"RETURN" [ Expression ]` 改为
`"RETURN" "(" [ Expression ] ")"` —— 它**收窄**了一条**声明**,但**零可观察行为
变化**(实现本来就只认带括号的形式,裸用 `RETURN 5` 从一开始就是 `E0011`;
§6.4 与附录 A 成员表一直写的是带括号那一派)⇒ **不是 breaking**,不进上面的
可见面变更表;机器守卫见 `spec_grammar_consistency.rs` 的
`appendix_a_return_expr_agrees_with_the_parser`。

### ⛔ 本批的可见面变更(breaking,按 ADR-0023 §v0.x 2 申报)

⚠ **这一段是本批最需要读的部分。** 下面每一条都**改变既有程序的可观察行为**。
它们**不走弃用两步**—— 因为它们要么是**码对位错了**(实现报的码与规范码表不是
一条),要么是**规范本来就这么写**而实现没照做(修的是实现,不是语义)。

| # | 变更 | 之前 | 之后 | 依据 |
|---|---|---|---|---|
| **B-1** | JSON / JSONL 诊断的字段名 | `error_category` | **`errorCategory`** | 语言规范 §11.1(逐字点名) |
| **B-2** | `wlwl sig --format json` 的数组键 | `entries[]` | **`exports[]`** | 语言规范 附录 E.4(要求与 E.2 同构) |
| **B-3** | 字典键的类型闸门 | 任意值都能当键 | 布尔 / 浮点 / 数组 / 字典 / null 的键 → **`E0031`**;**`INSTANCE` 仍然合法**(按对象身份) | 语言规范 §2.1 + §4.5(「键类型不符产生 `E0031`」) |
| **B-4** | `SET_PROP` 的**键**是线性 `THIS` | `E0030`(类型错) | **`E0032`**(线性 capability 逃逸) | 语言规范 §13.4 校验顺序第 1 步 + §15.3 |
| **B-5** | `HTTP_*` 的响应体定界 | 完全不看状态码 | `1xx` **跳过**并继续读下一个响应;`204` / `304` / `HEAD` 的 body **恒空**且**忽略 `Content-Length`** | RFC 9112 §6.3 第 1 条 |
| **B-6** | CLI 顶层「输入文件不存在」 | `E0042`(清单 / 锁文件族) | **`E0061`**(文件不存在);权限不足 → `E0062` | 语言规范 §11.2 的码表 |
| **B-7** | 旁路签名文件**读不出来** | `gradual_typing` 关着时**静默**(rc=0) | **无条件** `E0010` + rc=**3** | 语言规范 §11.4(「同一种故障不该因为走哪条路而拿到不同的退出码」) |
| **B-8** | `E0020` 诊断 | 只说「未定义名」 | **附最近候选名**(作用域可见名 + 全局内建名),并写进 `hint` | 语言规范 §3.2(「附最近候选名」) |
| **B-9** | `E0031` 的**诊断文案** | `dict key must be STRING or INTEGER, got …` | **`dict key must be STRING, INTEGER or INSTANCE, got …`** | 语言规范 §2.1(合法键类型集合含 `INSTANCE`) |

⚠ **B-1 / B-2 影响按规范写的外部工具**:JSON 诊断的键名与 `sig` 的数组键都**变了**
⇒ 读旧键名的消费者会读不到字段。B-2 只影响 `wlwl sig --format json`
(`wlwl interface` 本来就是 `exports[]` ⇒ 这次是**两个子命令终于一致**)。
⚠ **B-3 的例外**:`INSTANCE` **仍然可以**当字典键 —— 规范 §2.1 的键类型集合
**已补上它**,按**对象身份**判定(同一实例命中、异实例不命中),与 §2.4 的
身份语义对齐; 被拦的是**错类型**(布尔 / 浮点 / 数组 / 字典 / null / 类对象 /
函数)。⚠ 这条「规范侧」问题**已于 2026-10-20 裁决**:规范**补一句**(§2.1),
不是把实现改窄 —— 仓内 `R10-012` 有 4+ 条**刻意**的测试与 OOP conformance
夹具依赖「实例按身份当键」,禁掉它等于**删一个语言能力**。
⚠ **B-9 是 B-3 的连带**:文案必须列出 §2.1 的**全部**合法键类型,否则它会
**否定规范允许的类型**(用户会以为「实例不能当键」而绕路)。
⚠ **B-5 的连带**:原先 `204` / `304` / `HEAD` 的响应会因「无 `Content-Length` 且无
`Connection: close`」而 `ERR`;现在**可读且 body 为空**。`204` + `Content-Length: 5`
原先会把那 5 字节**真的读出来**(RFC 说这些响应本就无 body)⇒ 现在恒空。

### 本批的语法面变更(此前报语法错,现在合法;或此前 panic,现在报码)

| 变更 | 之前 | 之后 | 依据 |
|---|---|---|---|
| `*name` 剩余形参**必须在末位** | `(*rest, a)` 报 `E0022`(说元数);`(*a, *b)` 里的 `b` **静默未绑定** | 两者都报 **`E0010`** | 语言规范 §5.2 形参表(「只**可以**出现在末位」) |
| 前导 `+` 的整数字面量 | `+5` 报 `E0011` | `+5` **合法**(与 `-5` 同形) | 语言规范 §1.6 的 EBNF(`int_literal = [ "+" \| "-" ] int_lit .`) |
| `${…}` 插值内的嵌套字符串 | `"${ERR("x")}"` 报 `E0001` | **合法**(插值内按完整表达式语法求值) | 语言规范 §1.8 |
| 括号序列之后接后缀链 | `(d).a` / `(d)["a"]` 报 `E0011` | **合法** | 附录 A 的 `PostfixExpr = Primary { Postfix }` + §0.4 一致性条款 |
| `ai::ASK_STREAM` 给 2 个实参 | **panic**(索引越界) | **`E0022`** | 语言规范 §9 的成员表(3 个必填) |
| 容器类型上的约束 | `ARRAY[ARRAY[INTEGER] : Comparable]` 报 `E0012` | **`E0010`**(规范逐字文案) | 语言规范 附录 A 注 ③ + §2.6 |



### Added

- **`wlwl:std.sanitize`(新命名空间,R2 全员,3 成员)** —— `HTML_ESCAPE` /
  `HTML_UNESCAPE` / `HTML_SANITIZE`。输出安全的上下文分工,三者**互不替代**:
  URL → `URL_ENCODE`,标记 → `HTML_ESCAPE`,富 HTML → `HTML_SANITIZE`。
  - **威胁模型写进成员描述**:防的是「不可信数据在**输出点**被解释为代码或
    结构」;**不清洗输入**(OWASP 现行口径:输入侧「清洗」不是 XSS 防治手段),
    不做语义校验,不防逻辑漏洞。
  - **实体表是全表,不是半表**:由生成器从 WHATWG `entities.json` 官方数据产出
    2125 项分号项 + 106 项 legacy 项(数据文件不入库,生成器入库)。半张表在
    安全语境下不是「已知局限」而是**调用方拿到的错误数据**(`&copy;` 原样穿过)。
    数字引用按 WHATWG 规则映射(越界 / 代理区 → `U+FFFD`,溢出有检查,不得 panic)。
  - **净化器是树构建形态,不是 token 流**:IEEE S&P 2024 与 Sonar mXSS 研究
    证明不建树的净化器与浏览器存在解析差分,可被 mXSS 结构性绕过。实现的是
    WHATWG 规则的**白名单子集**(树构建用开标签栈 + 隐式闭合表),**不承诺与
    浏览器逐位一致** —— 差异显式登记在规范 §13.2。
  - **安全不变量是「重解析幂等」**:`SANITIZE(SANITIZE(x, p), p) == SANITIZE(x, p)`
    字节级,规范性、打破即缺陷。**安全性由幂等性守护,不由「与浏览器相同」守护**。
  - **缺省策略保守**:22 个排版级标签(`a` `b` `blockquote` `br` `code` `em`
    `h1`–`h6` `hr` `i` `li` `ol` `p` `pre` `s` `strong` `u` `ul`)、
    `{"a": ["href", "title"]}`、三个 URL scheme(`http` / `https` / `mailto`,
    相对 URL 恒允许)、删注释。策略 `DICT` 的四个键**整体替换**缺省值而非合并。
  - **违规标签删除而非转义**;`script` / `style` / `iframe` 等 8 个 raw-text
    元素**连内容整删** —— 删除集身份同时封掉「raw-text 内容再解析」整类 mXSS 面;
    `svg` / `math` 作为外来内容整删。非白名单的其余标签**拆壳留内容**。
  - **性能是契约的一部分**(业主 2026-10-03 裁决「旗舰特色功能,性能是硬指标」):
    全员 R2 直通 `Value`,单趟扫描 + 预分配,树构建用节点索引池 + 迭代栈
    (**禁止递归**)。实测 10 KB 文章 **17.05 MiB/s**(§7.4 指标 ≥ 10);
    每字节成本 114.0 → 55.9 → **41.6 ns/B**(1 KB → 10 KB → 100 KB)
    随尺寸**下降**,线性承诺成立;10 万层嵌套不栈溢出(700 KB / 60.87 ms)。
  - 契约表 `tests/sanitize_contract.rs` **103 条**(22 转义 + 81 净化/诊断),
    其中 **79 条 OWASP XSS Filter Evasion 经典向量 + 14 条 mXSS 教科书样本**
    逐条冻结并**注明裁定依据**,幂等性三组(契约语料 / **324 条变异** / mXSS),
    病态契约断言(10 万层 / 1 MB 文本 / 2 万属性 / 10 万实体)。probe **155 → 157**。
- **`wlwl:std.encode` +`SHA256` / `HMAC_SHA256`(6 → 8 成员)** —— FIPS 180-4
  摘要与 RFC 4231 HMAC,手写零第三方依赖;复用 `wlwl-ast` 的字节级 SHA-256
  (单一实现两个消费者)。
  - **向量期望值取自 FIPS 180-4 与 RFC 4231 TC2,不是实现输出**;TC1 / TC3 的
    密钥是**二进制**,wlwl 的 `STRING` 是 UTF-8 文本表达不了 ⇒ 以 **Python
    hashlib 独立交叉核对**并注明出处(D13-001)。
  - TC7(超长密钥)以**字节级单元测试**落地(真二进制密钥),提示文本回抄不可靠。
  - 实测 `sha256_10kb` ≈ 255 MiB/s(§7.4 指标 ≥ 100)。
- **`wlwl:std.encode` +`PBKDF2_ITER` / `ARGON2ID`(8 → 10 成员)** —— 口令哈希
  闭环(追加批次 M6 的 W-03)。`PBKDF2_ITER` **手写**(RFC 8018 §5.2,PRF 复用
  `HMAC_SHA256` 已有的字节级 HMAC,约 30 行,不引依赖);`ARGON2ID` 引
  **`argon2` 0.6.0**(RustCrypto 官方,MIT OR Apache-2.0,feature 收窄为
  `["alloc"]`)。**加解密与密钥生成仍否决** —— 有 KDF **不改变**那条裁决。
  - **参数上下界是硬边界,越界 `E0030`**:下界取 OWASP 2025 最小配置
    (Argon2id `m` ≥ 19 456 KiB / `t` ≥ 2 / `p` ≥ 1;PBKDF2-SHA256 `c` ≥
    600 000),上界防「一个参数打满机器」(`c` ≤ 10M / `len` ≤ 1 024 B /
    `m_cost` ≤ 1 GiB / `t_cost` ≤ 32 / `p_cost` ≤ 16)。不设下界的话
    「参数全显式」等于「调用方可以传 `iter = 1`」—— 比不给这个成员更危险。
  - **参数全部显式、无默认值**;`salt` 收**十六进制文本**(RFC 的盐是任意
    字节串,而 wlwl 的 `STRING` 是 UTF-8 文本,同 D13-001),盐的自然来源是
    W-04 的 `RANDOM_HEX`。
  - **向量期望值不是实现输出**:PBKDF2 原语层冻结 RFC 7914 §11 两条;成员层
    5 条正例的期望值来自**独立第二实现**(Python `hashlib` 与一份照 RFC 9106
    + 参考实现 C 源码写的 Argon2id,后者先**逐块复现了参考实现自己的 KAT**
    才被采信)。契约 47 → **67** 条,probe 158 → **160**。
  - ⚠️ **实测推翻了计划里的一条**:`ARGON2ID` 原拟增 `secret` / `assoc`
    (pepper / 关联数据)两形参,理由是「没它们就一个 RFC 9106 §5.3 官方向量
    都验不了」。**该向量在 `argon2` 0.6.0 上复现不出来**(`58a04dad…` vs
    权威值 `0d640df5…`,而同一 crate 的**无 K/X 路径三方一致**)⇒ **形参
    撤掉**:一个与其他实现互不认的 `secret` / `assoc`,存下的哈希任何别的
    实现都验不过,那比没有这个参数更危险。完整证据链见
    `docs/history/addendum-04-crypto-kdf-secrets.md` §3.5。
  - **已知限制**:`algo` 只接受 `"sha256"`(仓里无 SHA-512;签名保留该形参是
    留位,`sha512` 报 `E0030` 而非静默接受)。
- **`wlwl:std.encode` +`RANDOM_BYTES` / `RANDOM_HEX` / `TIMING_SAFE_EQ`
  (10 → 13 成员)** —— 口令处理的另外两半(追加批次 M6 的 W-04)。
  **口令闭环至此完整**:不可预测的原料(`RANDOM_*`)+ 慢哈希(`PBKDF2_ITER` /
  `ARGON2ID`)+ 不泄漏的比较(`TIMING_SAFE_EQ`)。**加解密与密钥生成仍否决。**
  - **熵源是操作系统**(`getrandom` 0.3.4,MIT OR Apache-2.0,
    `default-features = false` —— 关掉 `sys_rng` 那个**非密码学**的线程局部
    回退)。**刻意钉 0.3 而不是 0.4**:0.4 声明 `rust-version = "1.85"` /
    edition 2024,而本工作区在 `[workspace.package]` 里承诺 1.75 —— 取最新版
    会把那个承诺变成谎言(CI 用 `stable`,抓不到)。两者 API 相同
    (`fill()`),升级要连同 `rust-version` 一起改。
  - **`RANDOM_BYTES` 返回整数数组而不是 `STRING`**:名字说的是「字节」,而
    wlwl 没有字节类型,硬塞进 `STRING` 只能有损替换成 `U+FFFD`,而规范
    §11.3-3 已经把「不做有损替换」立成条款。要文本用 `RANDOM_HEX`。
  - **熵源读失败是 `E0060`(IO error)而不是 `E0030`**:前者是**运行环境**
    出问题(内核没给熵、沙箱封了系统调用),后者是程序员把参数写错了。
  - **随机缓冲区刻意不设长度上界**:两个 KDF 设上界是「一个参数能打满机器」
    是**安全**问题;随机缓冲区调大只是多花时间与内存,而程序本来就能建更大的
    数组。`n = 0` 合法,返空。
  - **`TIMING_SAFE_EQ` 永不失败**,长度不等是 `FALSE` 而非错误;实现走
    `max(len)` 步并把长度差折进同一个累加器,**没有任何一处按比较结果分支**
    —— 朴素写法在长度不等时提前返回,等于把「长度」泄给时钟。这条性质由两条
    **机制断言**守着并**做过变异验证**:把实现换成 `a.len() == b.len() && a == b`
    之后两条立刻变红(比值 0.000 对门槛 0.2)。**诚实边界**:耗时仍依赖长度,
    这无法避免也不必避免 —— 要藏的是「哪几个字节不同」与「是否相等」。
  - CSPRNG 由**不变量**而非固定值断言:10 万次 128 bit 抽样**一条不撞**、
    首字节卡方粗检(阈值 400;真缺陷在 10⁷ 量级,健康源约 255 —— 两者差四个
    数量级,阈值不是赌运气)。契约 **67 → 83** 条,probe 160 → **161**。
  - **性能基线**(`benches/kdf.rs` + `baseline.txt` KDF 段,release 直调
    `StdFn`):`ARGON2ID` 在 OWASP 下界 **20.3 ms**,`PBKDF2_ITER` 在同为
    OWASP 下界处 **833 ms** —— **Argon2id 便宜 41 倍**。⇒ **新写的口令存储
    默认 `ARGON2ID`**;`PBKDF2_ITER` 的存在理由是**互操作**(别的系统只认
    PBKDF2 时它是那条下限)。熵源 4 KiB 在 10–18 µs,`RANDOM_HEX` 比
    `RANDOM_BYTES` 快 1.8×(后者慢在把 4096 个整数装进数组,不是熵源)。
  - ⚠️ **本项实测否掉了计划里的一条性能断言**:原「`PBKDF2_ITER` 100 000 轮
    ≤ 100 ms」实测 **138.8 ms**。原阈值源自「600 k ≈ 0.3 s」这个对可移植纯
    Rust SHA-256 偏乐观的估计。**没有为凑读数去改实现** —— 0.69 µs/次
    HMAC-SHA256 落在正常量级,差距来自「纯 Rust 通用实现 vs OpenSSL 汇编」。
    断言改成**回归闸**(600 k 轮 ≤ 1.0 s,余量 20%),与全表「> 110% 基线均值
    即回归」同口径 —— 它要防的是「病态变慢」,不是「比某个估计慢」。
- **新命名空间 `wlwl:std.regex`(7 成员)** —— 补上「功能清单里唯一的全员标配
  空白」(此前本语言**零模式匹配能力`),命名空间 14 → **15**、成员面 111 → **118**。
  **执行时间对输入线性**:Thompson 构造 + Pike VM,零第三方依赖、零 UCD 数据。
  - 语义取 **leftmost-first(左最早)**,与 Perl / Python / JS / Java / Rust `regex`
    一致。⚠️ 这里的依据是**实测更正了计划**:计划原写「RE2 的语义是左最长」,而
    `re2.h` 的 `longest_match (false) search for longest match, **not first
    match**` 与 Go `regexp` 的文档注释都证明 **RE2 默认就是左最早**,左最长只是它的
    **POSIX 模式**(且 Go 文档承认 POSIX 的子选择规则「计算上不可承受甚至没有良定义」,
    它自己的 `CompilePOSIX` 也没完全遵守)。另:**线性时间来自 NFA 模拟,与选哪条语义
    无关** —— 所以没有性能理由偏向任何一方。
  - **必须拒的构造**(与线性时间互斥,不是「以后再加」):反向引用 `\1` / `(?P=…)`、
    lookahead / lookbehind、命名组、`\p{…}` —— 一律**编译期 `E0030`**。只测能编译的
    用例,一个回溯引擎也会全绿;而 `(a*)*b` 在 10 000 个 a 上回溯引擎跑不完,本成员
    **立刻返回「不匹配」**。
  - 三条容易踩的细节都写进规范:`.` 默认**不含**换行(`(?s)` 打开);`^`/`$` 锚**整个
    输入**(`(?m)` 才切行锚点,**刻意不抄 RE2 的 `m` 命名** —— 它与 Perl 相反);
    `\w` `\d` `\s` 与 `(?i)` 全是 **ASCII-only**(Unicode 折叠走 `std.text`)。
  - **期望值来自 Python `re` 独立核对**,不是实现输出;两处刻意不跟它:`re.ASCII`
    (否则 `\w`/`\d` 是 Unicode)、空匹配邻接规则(照 RE2/Go —— Python 的
    `finditer` 会照报紧贴的空匹配)。契约 `regex_contract` 新建 **28 条**,
    `wlwl-std` 单测 **10 条**,probe **162 → 164**。
- **新命名空间 `wlwl:std.time`(A1 + A2 片,3 成员)** —— 命名空间 **13 → 14**,
  成员面 **109 → 112**。GAP-4 的第一刀:墙钟、单调钟与睡眠。
  - `NOW() -> INTEGER`(Unix **毫秒**)/ `MONOTONIC() -> INTEGER`(进程启动起的
    毫秒)。**单位取毫秒不取秒**:秒的 ±1 s 量化误差会直接污染「缓存 5 秒过期」。
    **不返回 ISO-8601 字符串**(那是格式化层的事);**亚毫秒测量不支持**(已知限制)。
  - ⚠️ **`MONOTONIC` 的绝对值无意义、跨进程不可比 —— 只有差值有效**(`Instant`
    契约);它与 `NOW` 是**两个独立的源**,契约表专门钉了「两者差着五十多年」这件事。
  - **`NOW` 不单调**(NTP 校时 / 手动改表都能让它回跳),所以**测时间差要用
    `MONOTONIC`**。规范 §15.3 另有一条:用了 `NOW` 的程序**不再可复现**,而
    运行时**检测不到**这件事,所以只在成员表上声明 —— **测试代码尤其要用
    `MONOTONIC`**。
  - 成员**在 `wlwl-std` 内直读系统时钟,不经宿主** —— 与 `RANDOM_BYTES` 直读
    `getrandom` 同款理由(「`std.time` 是时间的源」)。⚠️ **绝不**给本命名空间的
    成员加「可注入时钟」形参:那等于让调用方能把它降级成可预测源,与「给 CSPRNG
    加后门」同一条错误。
  - `SLEEP(ms) -> NULL` —— ⚠️ **阻塞原语**:调用线程停住 `ms` 毫秒,期间**不调度
    任何其他任务**,**任务体内调用同样如此**(规范 §15.5)。`ms` 必须是非负整数,
    `0` 合法(让出一次时间片),负值 `E0030`。
    - **原本设计是「任务内挂起、到点唤醒」,实施时撞上运行时能力缺口并被否**:
      wlwl 的「中途挂起后**原地续跑**」能力**只有 `YIELD()` 有** —— 分段器
      `split_body_for_yield` 只认 `YIELD()` 这一个名字,而非分段任务体被唤醒后走
      `invoke_closure` **整段重跑**。⇒ 挂起路线会让 `SLEEP` **无限循环**
      (实测 8 秒 79 次不终止)。⇒ 业主 2026-10-06 裁决**方案 E**,`StdCtx.task_depth`
      **不开**、`wlwl-eval` **零改动**。
    - **代价没有新增**:「`SLEEP` 期间不调度其他任务」本就是候选甲**早已接受**的
      那句(选甲就是为了让它可被文档化成一条确定性规则),方案 E 只是把它提前到调用点。
    - **附带收益**:B 片(假时钟气泡)不再需要「队列空 ⇒ 判全阻塞 ⇒ 推进虚拟时钟」
      那处调度器分支 —— 气泡内 `SLEEP` 直接推进虚拟时钟返回即可。
    - 契约表与 probe 各有一条**反向守卫**:任务体里 `SLEEP` 之后必须继续执行并交出
      结果。它红了的形式是**超时**(挂起路线的特征读数),不是断言失败。
  - **`ADR-0024` 由 Proposed 改 Accepted**,并在 §6 重切成三片:A1 纯读、
    A2 `SLEEP`(本批,阻塞语义)、B 假时钟气泡。⚠️ **本批不是「时间能力齐了」** ——
    气泡还没做,读规范 §15 的人要先看到那句切片声明。
  - 契约表 `time_contract` **9 → 16** 条,probe **161 → 162**(另 `std.regex`
  使其到 164、`SLEEP` 使其到 **165**)。
- **`wlwl:std.time` 加三个气泡成员(3 → 6 成员,addendum-01 B 片)** ——
  `TEST_BUBBLE` / `ADVANCE` / `TIMEOUT`。命名空间与成员面计数见规范 §0.2。
  - **`TEST_BUBBLE(body)`** —— 假时钟气泡。虚拟单调钟**从 0 起算**、虚拟墙钟 =
    **写死的起点** `1_700_000_000_000` + 已流逝毫秒 ⇒ `SLEEP(3600000)` **瞬时完成**,
    同一程序跑两次时间序列**逐字节相同**。**气泡不再嵌套**(`E0030`);气泡内
    `SPAWN` 的任务**归气泡且无法逃逸**(已核:`SPAWN` 无 `SCOPE` 即 `E0058`,
    结构化并发保证它们在 body 返回前终止)。
  - **时钟接缝兑现**:B 片按 `ADR-0024` §6.3-2 候选甲给 **`StdHost` 加方法**
    (读虚拟钟 / 是否在气泡内 / 拨钟 / deadline 栈),`wlwl-std` 的成员改问宿主。
    ⇒ **A1 留的 `_host` 形参此刻才用上:签名与调用点一行都没改。**
    ⚠️ 与 §3.10 否掉的 `StdCtx.task_depth` **形态相反**:那次是「逐次调用的上下文
    事实」放 `StdCtx`,这次是「调度器状态」放 `StdHost` —— **一个判据,两次相反的
    落点**。所有默认实现都是「真实时钟 + 无气泡」,所以每个手写 `StdHost`
    (测试与基准里的 `NullHost`)**一行都不用改**。
  - **`TIMEOUT(d, body)`** —— 虚拟钟走过 `d` 未返回就**放弃** body,返回
    `ERR([kind: "Timeout"])`(§3.7 口径 2 定的 `ERR` **值**形态,不是原生诊断)。
    **只在气泡内可用**,气泡外 `E0030` 且诊断里带「为什么做不到」的完整解释。
    - ⚠️ **中断粒度是实测出来的**(`addendum-01` §3.12):超时触发时 `SLEEP` 置
      「已放弃」标志并返回 `Signal::Return(ERR)`;**实测粒度 = 最近的函数边界,
      且不逸出 `TEST_BUBBLE` / 顶层**。⇒ `SLEEP` 直写在 body 里 ⇒ body 被完全
      放弃;藏在更深的调用里 ⇒ 只中断到那一层。
      **但 `TIMEOUT` 的返回值永远正确**(查标志),所以「超时了没有」永远有确定答案。
    - **`TIMEOUT` 可嵌套,取最紧的未放弃 deadline**(不是栈顶);被放弃过的那项
      会退出计算,否则会永久触发后续每一次 `SLEEP`。
  - **⚠️ 气泡的已知限制写进规范 §15.6,不藏**:它给的是**确定性时间**,**不是**
    Go `synctest` 的**任务间交错** —— 时钟在 `SLEEP` **调用点**就跳,要拿到
    交错必须先补「任意挂起原因的中断续跑」这一运行时能力(独立立项)。
    同源的两条:运行时**单线程协作式**(无工作线程,调度是一个 run queue)⇒
    「`SLEEP` 真阻塞」也不可能(阻塞了就没有代码去推进虚拟钟)。
  - **三条成员按裁决砍掉并登记**:`SINCE`(**不配** —— 与「单位统一毫秒」自相矛盾,
    且功能就是 `-(MONOTONIC(), t)`,调用方现在就能写)、`WAIT_BLOCKED`(**机制
    不存在** —— B-简下无任务因时钟挂起,永远立即返回)、`ADVANCE` 原设想里
    「唤醒到期的等待者」那半(**没有等待者**,删掉)。
  - 契约 `time_contract` 16 → **31** 条,probe **166 → 167**
    (`M6_std_time_bubble`)。⚠️ 成员表守卫的**提取范围被收在 §15.1 成员表内**:
    原先扫完整个 §15,而 §15.6 那张「能给 / 给不了」对照表里有一行
    `| \`SLEEP(3600000)\` …` 被当成了一个叫 `SLEEP` 的成员行(6 变 7)。
    **不是把守卫放宽,是把它对准被测对象。**
- **新命名空间 `wlwl:std.compress`(8 成员)** —— Zstandard(RFC 8878)+ DEFLATE 家族
  (RFC 1950 / 1951 / 1952)。成员面 147 → **155**、命名空间 18 → **19**。
  - **这是本工作区第一次引入压缩依赖**。取舍核过一遍:`zstd` 压缩端(FSE / Huffman /
    块划分)手写是周级工作量且正确性极难自证;而自研 DEFLATE **解压端**是压缩炸弹 /
    OOB / 无界分配的第一现场 —— 本批自己那条「解压输出上限」规范条款就说明这面攻击
    是真的 ⇒ **两半都引 crate**。⚠️ 顺带更正计划 §6 的一处写反:它说「压缩端尚可、
    解压端容易出安全洞」,实际**解压端才是最危险的一侧**,压缩端也不轻。
  - **`ring`(C + 汇编)早已在依赖树里**(经 `rustls` ← `reqwest` ← `std.ai`)⇒
    「本工作区纯 Rust、不能引入 C 依赖」这个前提**本来就不成立**。
    `flate2` 走**纯 Rust** 后端 `miniz_oxide`,DEFLATE 三条线零 C。许可证**实测放行**
    (`advisories / bans / licenses / sources` 全 ok,`deny.toml` 一个字未改)。
  - ⚠️ **`MAX_DECOMPRESSED` = 64 MiB 是规范性条款**,不是实现里的一个可调 `const`:
    越限返回 `ERR`(**不是** OOM、**不是** panic)。它同时是一条**已知限制** ——
    本批不做流式,所以 > 64 MiB 的单个载荷**没有解压路径**。
  - **确定性口径:冻往返 + 冻交叉核对,而**不**冻输出字节。** ⚠️ 实测:同一份 104 字节
    载荷,Python 的 `zlib` 压到 **66**,而 `miniz_oxide` 压到 **115**(比原文还大)
    ⇒ **压缩比与输出字节都是实现定义的**(块划分 / lazy matching / 熵编码选择自由),
    冻它只会冻结某个后端的内部选择,而且**没有任何独立实现能产出同样的字节**。
    契约表改为用 **Python 标准库产出的 gzip / zlib 流**喂给我们的解码器 ⇒ 证明解码器
    是**按 RFC 实现**的,而不是「只会解开自己写的东西」(后者是常见的自证循环缺陷)。
  - `DEFLATE_RAW_*` **无校验和 ⇒ 损坏不可检测**:截断输入可能解出前缀正确的垃圾而
    不报错。要完整性保证就用 zlib(Adler-32)或 gzip(CRC32)。
  - `level` 越界 `E0030` **绝不静默 clamp** ——clamp 会让「我传了 9」和「我传了 999」
    产出同一个结果。
  - 规范明写 **「压缩不是加密」**(与 §11.5「哈希不是加密」同款体例):压缩**不提供
    机密性**,把它当「藏起来」是安全缺陷。
  - ⚠️ 字节形状**继承 `addendum-06` 的裁决**,本批**不重新裁决** —— 两份各定一次就是
    std 里两种字节表示,那是最难还的债。
  - 契约 `compress_contract` **6 个测试**新建(含 **Python 独立向量交叉核对**);
    probe **168 → 169**(`M6_std_compress_roundtrip`)。
- **`std.fs` 扩到 18 成员 + 新命名空间 `std.env`(4)/ `std.process`(2)** ——
  成员面 126 → **147**、命名空间 16 → **18**。「现在**连列一个目录都做不到**」这条
  补上了。规范新增 **§18 `std.env`** / **§19 `std.process`**,`§2 std.fs` 重写成三节。
  - **路径分隔符定案(甲)**:规范分隔符**恒为 `/`**;输入端 Windows 额外容忍 `\`
    (那是 `std.fs` 三个原有成员**既有**的行为,收掉就是破坏性变更),**POSIX 上不做
    替换**(那里的 `\` 是合法文件名字符)。⚠️ 理由不是「统一好看」——现状核出来是
    「**wlwl 的路径是什么从未被裁决过**,它只是『Rust 是什么』的影子」(实现把 path
    原样交给 `std::fs`),而平台原生形态会让 `PATH_JOIN("a","b")` 跨平台产出不同
    字符串 ⇒ **契约表无法逐字节冻结**。逐字节冻结是本仓所有契约表的立足点。
  - `PATH_DIR` 无父目录返 `""`(不是 Go 的 `"."`)——让 `PATH_JOIN(PATH_DIR(x),
    PATH_BASE(x))` 成立;`PATH_EXT` **不含前导点**、**点文件没有扩展名**
    (`.gitignore` → `""`);`PATH_NORMALIZE` 是**纯词法**的(会去 `canonicalize` 的
    归一化有 IO 副作用、路径不存在时行为突变)。
  - **符号链接**:`LIST_DIR` 列出链接但不跟随;`WALK_DIR` **默认不下降进链接目录**
    (成环会让递归永不终止),`follow = TRUE` 时记 visited 防环。`WALK_DIR` 的
    `pattern` 是 **glob**(`*` 不跨分隔符、`**` 跨目录、`?` 单字符;含 `/` 时匹配
    整条相对路径否则只匹配文件名)—— 与 `std.regex` 互补。
  - **`MKDIR` 已存在 → `ERR`,`MKDIRS` 幂等成功**:静默吞掉「已存在」会掩盖竞态。
  - **字节形状定案(甲)**:`READ_BYTES` / `WRITE_BYTES` 用 **`ARRAY[INTEGER]`**,与
    `std.encode::RANDOM_BYTES` 同款(它已被定为「本语言唯一无损字节载体」),
    避开 hex 的双份开销。⚠️ 另补**范围条款**:给**小**二进制用;大载荷的正解是让它
    **待在文件里** —— 绕语言值搬一趟会重新踩上解释器侧数组构建的超线性证据
    (D11-012 / D12-009)。`addendum-07` §3.2 **复用**本条。
  - **失败口径新增一条**:新成员用 `ERR` 值(`[kind: "FsError", op, reason]`)而不是
    旧三个的 `E0060`–`E0062` 原生诊断 —— 「目录不存在」是**可预期的运行期结果**,
    调用方多半要 `IS_ERR` 分支。**两套口径并存**,元数 / 类型错一致。
  - **`std.process` 只收 `ARRAY[STRING]` 的 `argv`,规范显式禁止字符串形态** ——
    拼串再解析就是**命令注入**;契约与 probe 各有一条「元字符不被解释」的守卫。
    未知 `opts` 键(如 `shell`)**显式拒绝**而不是忽略:「忽略」与「禁止」在安全上是
    两件不同的事。⚠️ 它是**阻塞的**(与 `std.time::SLEEP` 同源):单线程协作式调度下
    等子进程期间**不调度任何其他任务**。
  - **砍掉 `PROCESS_EXIT_CODE`**:它需要「上一条 `PROCESS_RUN` 的退出码」——那是
    **跨调用隐藏可变状态**,与「显式值传递、无隐藏状态」的纪律冲突(`ADR-0027` A1
    同款理由);而 `code` 字段已经在返回值里,它存在的全部价值是少写一个字段名。
  - ⚠️ **本轮抓到两个真缺陷**:
    ① `PROCESS_RUN` 的超时**没兜住** —— 300 ms 的超时走了 **29.26 s**。根因是
    `child.kill()` 在 Windows 上**只杀直接子进程**(`cmd`),它 spawn 的 `ping`
    仍活着且**继承了管道句柄** ⇒ 读线程等不到 EOF,`join()` 拖住整个超时。修法是
    「**杀进程树**(`taskkill /T /F`)+ **join 必须有界**」。
    ② **测试污染了工作树**:fs 成员的**相对路径按进程 CWD 解析**(`with_base_dir`
    只影响 `IMPORT`)⇒ 一版用相对路径的契约测试把 `a.bin` 写到了
    `impl/crates/wlwl-eval/`。⇒ 契约里的 IO 用例全改绝对路径,probe **刻意不碰
    文件系统**,并把这条语义写进规范 §2.3。
  - 契约 `fs_contract`(13 个测试)+ `process_contract`(8 个测试)新建;probe
    **167 → 168**(`M6_std_fs_paths_and_process_safety`)。 ⚠ **2026-10-16 更正**(`D13-054`):该用例原含 `argv_intact` / `one_line_only` 两条**只覆盖 Windows** 的断言(探 `cmd /C`,而 `cmd` 不存在于 Linux/macOS),已移出 —— 同一安全性质由 `process_contract.rs::argv_elements_are_never_shell_interpreted` 两平台钉住;用例 id 保留不改(它在历史记录里)。
- **新命名空间 `wlwl:std.rand`(4 成员)** —— 消掉规范 §14.1 那条否决(它是
  「**没想清楚**」,不是「不重要」)。命名空间 **15 → 16**、成员面 119 → **123**。
  形状由 [`ADR-0027`](docs/adr/0027-std-rand-shape.md) 逐条裁决(A1–A7 + 四处补强)。
  - **状态是显式值,不是隐藏状态**:`SEED(entropy)` 造值,成员第一个实参收它,
    **没有 `SET_SEED()`、没有全局当前 RNG**。D12-006 由此结项。
  - ⚠️ **抽签成员返回 `[值, 新状态]` 而不是标量**(ADR 补强 4)。根因是语言的一条
    既有事实:**`DICT` 不可变**(语言规范 §2.1),`SET_PROP` / `INDEX_SET` 对字典
    也是「返回**新**字典、接收者不变」(§13.4)⇒ **值传递 + 不可变状态 = 状态永不
    前进**(实测 2000 次返回同一个数)。用语言**已有**的数组模式绑定消费:
    `LET([v, r1], INT_RANGE(r0, 0, 100));` —— 好处是**「忘了推进状态」不再是
    可能的惯用写法**(拿不到值,除非同时拿到新状态),而「加一个 `NEXT(rng)`」那种
    形态下忘了就是**静默拿到同一个数**。
  - **熵只从显式实参进来**:`SEED` 内部**不读 OS 熵源**。要真随机就去
    `std.encode` 的 `RANDOM_BYTES` / `RANDOM_HEX` 取 —— `std.encode` 是源、
    `std.rand` 单向消费它,**依赖写在程序文本里**。
  - 算法 **xoshiro256++**,播种走 **SplitMix64** + **零状态守卫**(xoshiro 的全零态
    是**吸收态**,不守卫会静默退化成恒 0 序列而冻结向量照样绿);读状态时**再挡一次**。
    熵字节经 **FNV-1a-64** 折叠(熵已经均匀之后的一次归一化,不承担安全职责)。
  - `UNIFORM` 取 `(next_u64() >> 11) as f64 * 2^-53` ⇒ 恰好 53 bit 尾数,
    **可被保证**落在 `[0,1)` 而不是「恰好不会返 1.0」;`[a, b)` 的右端不包含也是
    **被保证**的(实现夹一次)。`INT_RANGE` 走**无偏取模拒绝**、**绝不经过 `FLOAT`**
    —— 浮点只有 53 bit 尾数,中转会引入舍入偏差且对大区间**静默收窄上界**。
  - `CODEPOINT(rng)` 显式存在:码点**不是**均匀分布(要排除 UTF-16 代理区
    `0xD800..=0xDFFF` 与**永久非字符**),不能拿 `UNIFORM` 复用。
  - **契约 `rand_contract` 新建**(4 个测试 / 24 条用例),`wlwl-std` 单测 **15 条**,
    probe **165 → 166**。冻结向量的**期望值取自一份独立的 Python 第二实现**,
    不是从本实现导出 —— 否则「冻结」只会冻住「当前实现恰好长什么样」。
    ⚠️ 统计分布**刻意不进契约表**(多项分布,写进 `cargo test` 只会得到一条必然
    偶尔变红的假门禁:实测 70 000 次抽 7 值,最大偏差 1.7σ,完全健康)⇒ 用 4σ 容差;
    而「无偏」的**确定性**证明(`2^64 mod 7 = 2` ⇒ 最高两个 `u64` 必然被拒)在单测里。
  - ⚠️ **基准量到了 ADR-0027 A2 要求的那把尺子**(`benches/rand.rs` + `baseline.txt`
    段九):`DICT` 状态表示的代价是 **636 ns/调用**,而**同一序列绕过 `DICT`** 是
    **1.77 ns** ⇒ **约 360 倍**,吞吐 1.57 M vs 565 M 抽/s。**成本在表示方式,
    不在算法**(xoshiro 一步本身 1.77 ns;每次调用要新构造出参字典 = 4 次键字符串
    堆分配 + 8 次 `Value` 装箱 + 4 次键查找)。按 `addendum-08` §6 的「**先测再争**」
    **只登记数字,不判决**:加内建类型能把 636 ns 降到个位数,但那是**语言变更**
    (ADR-0023),而仓内**没有**已知的 10M 级需求,「不可接受」这个前提不成立。
    `std.rand` 的真契约是**逐字节可复现**,不是吞吐。
- **`ADR-0027`(`std.rand` 的形状)由 Proposed 改 Accepted** —— 逐条采纳 A1–A7 并加
  **四处补强**。本批**决策与实现同期落地**。
  - **A1 显式值传递、无任何隐藏状态**(`SEED(entropy)` 造值、成员第一个实参收它、
    **没有 `SET_SEED()`**)—— D12-006 由「同一 seed 建两个 RNG 序列完全相同」这条
    契约守住。**A2** 状态用 `DICT` 不加内建类型,键名定为 **`s0`…`s3`**。
    **A3** 算法 **xoshiro256++**、向量逐字节冻结并记版本。**A4** 熵**只**从显式实参
    进来(`std.encode` 是源这条依赖**写在程序文本里**)。**A5** 浮点只给 `UNIFORM`,
    区间 `[a, b)`、53 bit 且 **≥ 2^53 重抽**。**A6** 显式 `CODEPOINT`。
    **A7** 本批先落 `std.rand`,「降级为官方包」记为**过渡形态且必须写进规范**。
  - **补强 1** —— 播种走 **SplitMix64** 并加**零状态守卫**:xoshiro 的全零态是
    **吸收态**(恒输出 0),不守卫会静默退化成一条无意义序列而冻结向量照样绿。
  - **补强 2** —— 整数区间是**独立成员 `INT_RANGE(rng, lo, hi)`**,**不让调用方拿
    `FLOOR(UNIFORM(...))` 拼**:`FLOAT` 只有 53 bit 尾数,浮点中转会引入舍入偏差,
    且对大区间**静默收窄上界**(不报错、只给偏斜序列)。
  - **补强 3** —— A7 的过渡句**现在就写进规范**,列为硬检查项(不写它就默认长期
    住在 `std`,与规范 §14 已登记的方向互相矛盾)。规范 §17 章头一句 + 契约里一条
    专门的 `the_transitional_status_of_std_rand_is_still_written_down` 守它。
  - **补强 4** —— 抽签成员返回 `[值, 新状态]`,理由见上面那条。
  - **实测两条实现约束**:`StdFn` 签名层**没有可选形参构造**(裸函数指针,元数由每个
    成员自己 `args.len()` 判)⇒ `UNIFORM` 的单参形态是「一个成员内部判 `{1, 3}`」,
    且 **2 参必须是 `E0022` 错误**而不是「`a` 取默认」;`DICT` 的键是 `Value` 而非
    `String` ⇒ 状态字必须用**字符串键**。
  - **成员面就此冻结为 4**(`SEED` / `UNIFORM` / `INT_RANGE` / `CODEPOINT`)。
- **ADR-0024 草案**(`std.time` 的假时钟气泡,状态 Proposed)与 **regex 调研备忘**
  (RE2 式线性模拟,不立项)—— **只出文档,不出码**。
- **`wlwl:std.text` 增 `GRAPHEME_COUNT` / `WIDTH`——addendum-03 W-04(v0.11.3)**
  成员面 **5 → 7**,规范§12 补两行 + **§12.3**。现在 `std.text` 覆盖大小写、**规范归一**、**字符簇**、**显示宽度** 三个方向。
  - ⚠ **坐标系有两个**:`LEN` 给码点数、`GRAPHEME_COUNT` 给**字符簇数**——`"👨‍👩‍👧"` 的 `LEN` 是 **5**而 `GRAPHEME_COUNT` 是 **1**。
  - ⚠ **没有规范化就比不比错**:`é` 与 `"e" + U+0301` 渲染完全一致而 `==` 给 FALSE。
  - ⚠ **`WIDTH` 是簇感知的**:三个 emoji + 两个 ZWJ 逐码点是 **8**,取簇内最大得 **2**。
  - 口径是 **实测定的**(§3.3):D-W1 `A`类(Ambiguous)=**1**(占全表 **38.9%**) · D-W2 默认 emoji 呈现的 `N`=**2**(实测只影响 **26** 个 U+1F1E6..1F1FF) · D-W3 `Mn`/`Me`/`Cf`=**0**(`Mc` 不算)。⚠ **计划§2原话「emoji = 2」照字面实现是错的**。
  - **正确性证据**:`bin/grapheme-check` 对官方 `GraphemeBreakTest.txt` **全量 853 行零失败**;`bin/width-check` 对 `EastAsianWidth.txt` **全表 1 114 112 个码点逐条比对**,并单独**逐条点名** 26 个 D-W2 生效点。两者均不在 `cargo test` 里。
  - 契约 `text_contract` **20 → 44 条**,probe **172 → 173**(不联网,纯函数)。
- **新命名空间 `wlwl:std.net`(5 成员,R2 全员)** —— 命名空间 **15 → 16**、
  成员面 **118 → 123**。`URL_PARSE` / `URL_JOIN` 两个**纯函数** + `HTTP_GET` /
  `HTTP_POST` / `HTTP_REQUEST` 三个明文 HTTP 成员。规范新增 **§21 `std.net`**,
  附录 A 重生成,契约 `net_contract` 5 个测试、probe **171 → 172**。
  - **`ADR-0028`(Accepted)定死形状**:明文 `http:` only、**零新增第三方依赖**
    (走 `std::net::TcpStream` + 系统解析器)、`HTTP_*` 是**阻塞原语**、超时缺省
    **30 000 ms**、**不自动跟随重定向**、**不解 chunked**。
  - ⚠ **`HTTP_*` 是阻塞原语**:执行期间不调度任何其他任务,整个 `SCOPE` 冻结
    —— 与 `std.time::SLEEP` / `std.process::PROCESS_RUN` 同源。**不要在同一个
    `SCOPE` 里并发发多个 HTTP 请求**,`SPAWN` 了也还是排队:那是**错的写法**,
    不是低效的写法。
  - ⚠ **D1(`HTTP_*` 阻塞)是带条件冻结的**:冻结前提是「`ADR-0017` Step 3(真
    state-machine 续跑)仍未落地」;前提不成立则阻塞的代价就没有必要承担,
    **必须重新裁决**。守卫 `impl/crates/wlwl-eval/tests/adr_conditional_guard.rs`
    会在前提不成立时**转红**。
  - ⚠ **不支持的写法一律 `ERR`,绝不「尽力解析」**:`chunked` 不解码(走私向量)、
    既无 `Content-Length` 又无 `Connection: close`、重复的 `Content-Length`、
    obs-fold、头值含裸 CR/LF,全部拒绝;头块 **64 KiB** / 体 **8 MiB** 上限越限
    是 `ERR` 而**不是** OOM。
  - ⚠ **失败是 `ERR([kind:"HttpError", op, reason])` 值不是原生诊断** —— 网络不通
    是可预期的业务分支,做成诊断会让整个运行终止、调用方连重试都做不到。
  - ⚠ **`URL_PARSE` 返回七个键**(计划原列六个):多出的 `userinfo` **不得丢弃**
    —— RFC 3986 §7.6 证明丢掉它会让人以为 host 是 `cnn.example.com` 而实际是
    `10.0.0.1`,解析器替调用方吃掉它等于**藏起一个已知的语义攻击面**。
  - ⚠ **`URL_JOIN` 是 strict 模式**:RFC §5.4.2 的 `http:g` 给了两个结果,本实现
    取 RFC 排在**前面**的那个;**WHATWG URL 取的是另一个**,两者**不等价**,
    跨规范对照时不要互相套用。
  - **正确性证据**:契约把 **RFC 3986 §5.4 的 42 条引用求解用例逐字冻结** ——
    由脚本从 `rfc3986.txt` 原文**解析产出**,不是手抄也不是从实现输出抄。
- **新命名空间 `wlwl:std.result`(6 成员,4 R1 + 2 R2)** —— 命名空间
  **20 → 21**、成员面 **165 → 171**。`RESULT_MAP` / `AND_THEN` / `FLATTEN` /
  `NULL_OR`(R1,零 kernel)+ `RESULT_ALL` / `RESULT_ANY`(R2)。规范新增
  **§22 `std.result`**,附录 A 重生成,契约 `result_contract` 15 个测试。
  - ⚠ **计划原列 6 个组合子,实测只能做 3 个** —— 另一三个
    (`OR_ELSE` / `ERR_MAP` / `ERR_OR_ELSE`)**在任何实现层都做不到**:
    语言规范 §8.2 规定**实参为 `ERR` ⇒ 该调用不求函数体**,`ERR` 直接成为调用结果;
    判据是**被调用者的名字**是否在 §8.3 注册表里,而 §8.3 明文「本表不新增条目」。
    传播块位于 `user_fn` 分派**之前**,所以 **R1 门面与 R2 内核同样接不到** `ERR`。
    这三个成员的 `ERR` 分支要**做事**(取载荷 / 调 fallback / 换载荷),故**不可实现**,
    改写成规范 §22.3 的**调用方惯用法**。依据 `ADR-0029` D6。
  - 留下的三个之所以成立,是因为它们的 `ERR` 分支恰好就是「原样返还」—— §8.2
    **免费**提供了这件事。所以 §22.2 规定这三个成员**源码里没有** `IF(IS_ERR(r), …)`
    外层护栏:写上它是永久不可达的死代码,且会让人误以为 `ERR` 分支在成员体内。
    `FLATTEN` 的**内层** `IS_ERR(i)` 是活的(`FLATTEN(OK(ERR(e)))` 的外层实参是 `OK`)。
  - ⚠ **`addendum-11` 原计划的核心不变量的断言口径是错的**:「`MAP(ERR(x), f)` ⇒
    `f` 不被调用(用计数器断言)」是**空证** —— 计数器不涨是因为**体压根没执行**,
    不是因为 `IF` 短路。契约测试改钉**三条独立断言**(体内与实参无关的副作用计数
    为 0 / 三条成员名不在 §8.3 注册表 / 传播块在 `user_fn` 分派之前),合成
    「组合子**接不到** `ERR` ⇒ **不可能**成为 §8.2 旁路」。`addendum-11` §6 的
    头号风险不是被绕开,是被证明**无法发生**。
  - ⚠ **成员叫 `RESULT_MAP` 不是 `MAP`**(`ADR-0029` D5):`std.collection` 自 v0.10
    起就导出 `MAP`,两者同作用域 `IMPORT` **硬撞 `E0021`**(不遮蔽、直接报错)。
  - ⚠ **`AND_THEN` 不补 `OK` 外壳,这是字面语义**:`f` 一旦返回裸值,链就断了
    (`AND_THEN(OK(3), FUN((x), +(x, 1)))` 得 `4`)。调用方有责任让 `f` 返回 `RESULT`。
  - **`ADR-0029`(Accepted,同日修订)** 定死形状:R1 门面零 kernel / 新命名空间 /
    不扩 §8.3 注册表 / 不复刻四个全局内建(`IS_OK` / `IS_ERR` / `UNWRAP_OR` /
    `UNWRAP`)/ `MAP` 改名 / 三条判为不可实现。
  - **`RESULT_ALL` / `RESULT_ANY` 归 R2**(`ADR-0029` D7):两者要把 n 个载荷收成
    新数组,而 R1 累加链 `SET(acc, +(acc, [v]))` 每次复制整个数组 —— 实测 n 每翻
    一倍耗时约 **×3**(n=250→4000:27→765 ms)。标准库不该带这条曲线。形态与
    `std.collection` 归层那十个成员同款(kernel 注入 + 门面改名导出)。
  - ⚠ **§8.2 那堵墙对容器里的 `ERR` **元素**同样成立**:`ANY(rs, FUN((x), IS_ERR(x)))`
    与 `MAP(rs, FUN((x), x))` 都得 `E0102` —— 逐元素调用本身就是一次带 `ERR`
    实参的调用。所以这两个成员**刻意不接受谓词参数**,改为收数组后在内核里逐个读
    `Value::Err`(全程不调任何闭包)。规范 §22.2 把「装盛 vs 消费」的分界线写成
    规范性条款:从容器里读 `ERR` 并在本作用域消费**不是**旁路(调用方自己也能写
    循环);把**裸** `ERR` 藏进容器骗过边界**才是**。
  - ⚠ **`NULL_OR` 不是 `UNWRAP_OR`**:`NULL` → `default`,而 `""` / `[]` / `0` /
    `FALSE` 原样返回(三者互不相等);遇 `ERR` 时按 §8.2 **传播**而非取默认值 ——
    拿默认值吞错误才是错的。⚠ 语言**没有 `IS_NULL`**,判别器只能写
    `==(TYPE(v), "NULL")`;用 `IF(v, …)` 的真值化会把 `""` / `[]` / `0` 一起吞掉。
  - **空数组口径两条相反**:`RESULT_ALL([])` = `OK([])`(空集真,与 Rust
    `collect::<Result<Vec<_>,_>>()` 同款);`RESULT_ANY([])` = `OK(NULL)`
    (「没有候选」不是错误,不编造 `ERR`)。
- **`wlwl:std.sanitize` 加 `HTML_SANITIZE_SCRIPT`(v0.11.3 M7 / addendum-13)** ——
  成员面 3 → 4(全库 171 → 172)。把一个值序列化成**可安全内联进 `<script>` 的
  inert 数据块**,返回完整 `<script type="application/json">…</script>`。
  - ⚠ **补的是本库自己的缺口,不是转述别人的**:OWASP DOM XSS Cheat Sheet 明写
    `JSON.stringify()` 「**is not an output-encoding function**」并逐字符点名内联场景
    还要额外挡 `<`(及 `>` `&` U+2028 U+2029);实测 `STRINGIFY` 把这五个**全部原样
    输出**(U+2028 按**长度**判:输入 3 码点 → 输出 5,转成 `\u2028` 该是 11)。
  - **收整个元素而不是裸串**:只暴露「转义裸串」的成员时,调用方可以忘记调、可以
    只转一半、可以转完又拼回去 —— 那它提供的安全感和裸拼字符串没有区别。收**值**、
    由成员自己 `STRINGIFY` + 加固 + 包壳,「忘记加固」这个失败模式就**不存在**。
  - **全函数**:加固把 `<` 全部消掉之后,输出里唯一的 `<` 是它自己写的开标签,任何
    值都无法终止 script data 状态 ⇒ 内容层面**没有任何值能让它出错**。
  - ⚠ **不做 `JS_ESCAPE_STRING` / `JS_ESCAPE_COMMENT`**(`ADR-0030` D2):OWASP 的首选
    根本不是转义,是 `application/json` 这个 inert 容器本身;而通用 JS 字面量转义剩下
    的用武之地(`onclick="f('…')"`)已被 `HTML_SANITIZE` 整条剥掉事件处理器属性覆盖。
  - **类型化收口判为不适用**(`ADR-0030` D3):Trusted Types 的**全部**收益来自
    「浏览器在 sink 处强制拒绝裸串」;wlwl 没有宿主形态时它是**定义上**的消失 ——
    没有 sink、没有执行者,剩下的「一个净化函数」正是 `HTML_SANITIZE`。否证记录
    已写进规范 §14,并由契约测试钉住不被删。复活条件 = 出现宿主形态。
  - 契约 11 → 17 条;⚠ 其中一条是**反向断言**(输出里除自身开标签外一个 `<` 都没有),
    它对任何输入成立,比逐条比对样例更有价值。
- **`std.sanitize` 扩到 CSS / URL 上下文(4 → 8 成员,v0.11.3 M7 / addendum-13)** ——
  全库 172 → 176。`CSS_ESCAPE_IDENT` / `CSS_ESCAPE_STRING` / `HTML_SANITIZE_CSS` /
  `HTML_SANITIZE_URL`。
  - **实现顺序按 OWASP 口径**(`ADR-0030` D1):CSS 上下文指引第一句是「**allowlist
    it**」,CSS-escape 只是补充 ⇒ `HTML_SANITIZE_CSS` 先于两个 `CSS_ESCAPE_*`。
    这与 §13.3 否掉 SQL 转义是同一条纪律:转义垫底、白名单当第一防线。
  - **两个转义器逐条照抄 CSSOM §"Common Serializing Idioms"**,不是自创规则。
    ⚠ 「按码点转义」= `\` + **最小位数小写十六进制** + **一个空格** —— 尾随空格
    是消歧符,漏了它 `\31 23` 会与 `\3123` 混。⚠ **单引号不转义**(CSSOM 明注:
    字符串**总是**用 `"` 序列化),这条最容易凭直觉写成「引号都转」。
    ⚠ **版本差异**:2013 WD 版对 `U+0000` 是抛 `InvalidCharacterError`,现行版
    (W3C REC cssom-1)是 → `U+FFFD`;契约冻结**现行版**。
  - **`HTML_SANITIZE_CSS` 的两条分开失败口径**:属性不在白名单 ⇒ **静默丢弃**
    (白名单正常工作);命中拒绝构造(`@import` / `expression(` / `javascript:` /
    `behavior:` / `-moz-binding` / **任意 `url(`**)⇒ 整个输入判为攻击,报 `ERR`。
    ⚠ 任意 `url(` 一律拒 —— 「看着安全就放行」需要解析 `url()` 完整语法 +
    `data:` MIME 白名单,那是**另一个成员**的事,放在这里只会变成一个看起来能用的
    漏洞;能取 `url()` 的属性也一律不在白名单,不靠单点防御。
  - **`HTML_SANITIZE_URL` 与 `HTML_SANITIZE` 的 URL 判定是同一份逻辑**(默认
    `http`/`https`/`mailto`)。**复用,不要各写一份** —— 分裂意味着其中一处是错的。
    契约 `html_sanitize_url_agrees_with_the_html_sanitizer` 把同一组 URL 喂两个
    入口并断言结论一致。
  - ⚠ **不做大小写归一、不做百分号编码**:路径大小写敏感,小写化会改错资源。
- **`wlwl:std.math` 加任意精度十进制 `DECIMAL` 族(8 成员,v0.11.3 M7 / addendum-10)** ——
  成员面 32 → 40(全库 176 → 184)。`DECIMAL` / `DECIMAL_ADD` / `_SUB` / `_MUL` /
  `_DIV` / `_CMP` / `_TO_STRING` / `_ROUND`,**R2 归层**(每次运算都要构造一个新
  `DICT`,与 D11-012 的数组平方级同族)。形状见 `ADR-0031` D1。
  - **值形状是公开契约**:`{sign: INTEGER, digits: STRING, scale: INTEGER}`,字段名
    变更即 breaking。规范形式:`digits` 无前导零、`scale ≥ 0`、`sign ∈ {-1,1}`,
    **零恒为 `{1, "0", 0}`**。⚠ `scale` **有意义**:`DECIMAL("1.00")` 渲染回
    `"1.00"` —— 金额场景靠它保留「几位」。
  - ⚠ **本族存在的理由是 `f64` 的 `2^53`**:实测 `+(9007199254740993.0, 1.0)` 得
    `9007199254740992.0`(静默丢掉 +1),而 `DECIMAL_ADD` 给 `9007199254740994`。
  - ⚠ `DECIMAL_DIV` 的 **`scale` 必填**(静默截断是精度谎言),舍入恒 `HALF_EVEN`;
    `b = 0` ⇒ `ERR([kind: DomainError, op: DIV])`。⚠ `DECIMAL_ROUND` 的 **`mode`
    无默认**(银行家舍入与四舍五入是业务决定);`FLOOR` 与 `DOWN` **只在负数上**分岔。
  - ⚠ **不含 `MEAN`**(`ADR-0031` D2):`std.collection::SUM` 加 `/` 已覆盖。
  - ⚠ **非法 `DICT` → `E0030`,不修复** —— 用户能手工写出
    `{sign: 0, digits: "abc"}`,这是**明码标价的妥协**,不是干净的形状。
  - **正确性证据**:契约期望值取自 **Python `decimal` 模块**(默认精度 28、
    `ROUND_HALF_EVEN`),由脚本独立跑一遍再落表,**不是实现输出** —— 否则它对
    「准不准」这件事什么都不能证明。实现期由交叉核对抓出三个真 bug:大端下标的
    乘法权重写错(`125 × 5` 算成 5062)、`at_scale` 把补零**前置**、以及用
    `Vec::cmp` 做字典序比较(位数不同时 `[1,0,0] < [9,5]`,数值却 100 > 95)。
- **`wlwl:std.math` 加精确分数 `FRACTION` 族(8 成员,W-04)** —— 成员面 40 → 48
  (全库 184 → 192)。`FRACTION` / `_ADD` / `_SUB` / `_MUL` / `_DIV` / `_CMP` /
  `_TO_STRING` / `_TO_DECIMAL`,形状 `{num: INTEGER, den: INTEGER}`(规范 §7.3)。
  - **分子分母是 `i64`,不是任意精度** —— 计划 §0 把「任意精度」明确划给十进制。
    代价说清:分数连乘很容易撞上界,本族**一律在溢出发生前**检查并报
    `ERR([kind: OverflowError])`,**绝不**让语言层的 `E0035` 冒出来(那个会**中止
    整个运行**,放进标准库成员里不可接受)。契约钉住边界:`(1/3)^39` 分母
    `4052555153018976267` 仍成立,`(1/3)^40` 分母 `12157665459056928801` 越界。
  - `FRACTION_CMP` 用 `i128` 中转做交叉相乘 —— 比较**不该**因中间量溢出给错答案。
  - `FRACTION_TO_DECIMAL` 的 **`scale` 必填**(`1/3` 的十进制展开不终止),舍入恒
    `HALF_EVEN`;未约分 / `den <= 0` / 零不是 `0/1` 的 `DICT` ⇒ `E0030` **不修复**。
- **`wlwl:std.math` 加近似比较四成员(W-06)** —— 成员面 48 → 52(全库 192 → 196)。
  `APPROX_EQ` / `IS_NAN` / `IS_INF` / `SIGNIFICANT_DIGITS`(规范 §7.4)。
  - ⚠ **`IS_INF` 补的是缺口不是便利**:`==(x, x)` 对 `±inf` 为 **TRUE**(实测),
    自等绕法抓不到它,而本库此前既没有 `IS_INF` 也没有 `IS_NAN`。
    (`IS_NAN` 至少有 `x != x` 可用 —— `==(NaN, NaN)` 实测为 FALSE。)
  - ⚠ `APPROX_EQ` 的两个容差**不给默认**:`0.1` 与 `0.1000000001` 该不该相等是**业务
    决定**。实测 `+(0.1, 0.2)` 得 `0.30000000000000004` 且 `==` 为 FALSE —— 经典
    陷阱在本库同样成立,`APPROX_EQ` 是 `ASSERT_EQ` 在浮点上唯一能用的形态。
  - ⚠ `SIGNIFICANT_DIGITS(x, n)` 取**谓词**读法(x 是否恰好等于它保留 n 位有效
    数字后的值);计划原文没说返回什么,「返回有效数字**个数**」是另一个成员。零对任意
    `n ≥ 1` 为 TRUE;`NaN` / `±inf` 恒 FALSE。
- **`wlwl:std.math` 加描述统计九成员(W-05)** —— 成员面 52 → 61(全库 196 → 205)。
  `MEDIAN` / `MODE` / `VARIANCE_S` / `VARIANCE_P` / `STDDEV_S` / `STDDEV_P` /
  `PERCENTILE` / `CORRELATION` / `LINEAR_FIT`(规范 §7.5)。
  - ⚠ **不含 `MEAN`**(`ADR-0031` D2)。⚠ **混装静默提升**,与既有 `SUM` 同口径
    (D3)—— 收紧就会与 `SUM` 分裂,而分裂意味着其中一处是错的。
  - ⚠ **「空」与「无定义」都返回 `NULL`,不用错误**:样本标准差在 `n = 1` 时分母是 0;
    `CORRELATION` / `LINEAR_FIT` 在任一方向方差为零时同理;含 `NaN` / `±inf` 时
    同理。**`NaN` 不渗进统计量** —— wlwl 没有浮点异常传播约定
    (`/(0.0, 0.0)` 直接 `E1003`),让 `NaN` 渗进结果会让调用方**看不出**它不可信,
    而 `NULL` 是看得见的。
  - **样本 / 总体显式分成两个成员**,不给默认。
  - ⚠ `PERCENTILE` 的插值方式写死为**线性(含端点)**,因为它是**可观察的**:同样数据,
    线性 / 较低 / 较高 / 中点 / 哈德利五种方法给出不同的值。
  - 正确性证据:契约期望值取自 **Python `statistics`**(脚本独立跑一遍),逐值吻合。
    ⚠ 一处**有意分歧**并被契约显式钉住:Python 的 `statistics.stdev([7])` 抛
    `StatisticsError`,本成员的 `STDDEV_S([7])` 返回 `NULL`。

### Changed

- **`wlwl:std.collection` 的 `MAP` / `FILTER` / `CHUNK` / `WINDOW` 归 R2**
  (追加批次 M5 的 L0-A-2,`ADR-0025`)。**层归属变更,按 ADR-0023 §v0.x 5 不算
  破坏性变更** —— 调用面、签名、语义、诊断**一字未改**,成员面仍是 27
  (`EXPORT` 未动,门面 `LET(X, _X);` 改名导出,沿 `RANGE` 的既有形态)。
  **这一项无 breaking**(层归属变更不算破坏性变更)。
  - **为什么**:这四个是建数组类成员里规模成本最高的四个。四档实测
    (bench profile / criterion / 10 样本)每元素成本随尺寸**线性上涨**,
    平方级坐实 —— 根因是 `PUSH` 每次深拷贝整个不可变数组
    (`wlwl-eval/src/lib.rs:1032`)。
  - **效果**(8 000 元素,时间均值):`MAP` 2 033 → **26.4 ms**(77×)、
    `FILTER` 1 436 → **31.0 ms**(46×)、`CHUNK` 370 → **3.2 ms**(115×)、
    `WINDOW` 24 472 → **6.2 ms**(3 969×)。四组的每元素成本 1 k → 8 k
    **持平或下降**,平方级消除。`WINDOW` 的输出本身是 Θ(n·k),所以它归层后
    仍是 Θ(n·k) —— **贵是定义决定的,不是实现问题**(规范 §5.1 原话)。
  - **没变的**:`FLAT` / `UNIQ` / `ENUMERATE` / `GROUP_BY` / `JOIN` / `SORT` /
    `SORT_BY` / `ZIP` / `DEDUP_BY` / `KEY_BY` **仍在 R1、仍平方级**(L0-A-3);
    用户代码里的累加器链(`SET(arr, +(arr, [v]))`)**归层管不到**,那是
    L0-B(可变累加器,已立项、独立于 M5)的面。
  - 逐条数据与口径见 `impl/crates/wlwl-eval/benches/baseline.txt` **L0 段**。

- **`wlwl:std.collection` 的 `ENUMERATE` / `ZIP` / `UNIQ` / `FLAT` / `JOIN` 归 R2**
  (追加批次 M5 的 L0-A-3a)。同 L0-A-2 的形态与不破契约的理由(层归属变更,
  ADR-0023 §v0.x 5,非 breaking,`EXPORT` 未动,成员面仍是 27)。
  - **效果**(8 000 元素):`ENUMERATE` 7 437 → **3.2 ms**、`ZIP` 9 341 → **3.7 ms**、
    `FLAT` 9 814 → **3.7 ms**(2 300~2 650×,每元素成本由涨转降);
    `JOIN` 75.7 → **2.6 ms**;`UNIQ` 1 611 → **191 ms**(8.4×)。
  - **`UNIQ` 归层后仍平方级,如实记**:R1 有两笔平方成本 —— `PUSH` 重建
    (归层消掉)与 `INDEX(out, v)` 的线性成员查找(消不掉)。后者不能顺手改
    哈希:`values_equal` 要求 `1` 与 `1.0` 判等、`TRUE` 与 `1` **不**判等、
    `Dict` 判等无序、闭包判等是结构性的,而 `Value` 没有 `Hash`;且
    `Float(NaN)` 在 `values_equal` 下**与自身不等**(所以 `UNIQ([NaN, NaN])`
    留两个),按位哈希却会合并 —— 那是可观察的行为差异。**把 `UNIQ` 做成线性
    是一次独立的设计裁决**,不是归层顺手能带的事。
  - **仍留 R1 / 仍平方级**:`SORT` / `SORT_BY`(L0-A-3b,涉排序稳定性与比较器
    调用次数)、`DEDUP_BY` / `GROUP_BY` / `KEY_BY`(D 类)。
  - `ZIP` 的非数组实参按**单列**处理且**不报 `E0030`**(`ZIP(1, 2)` → `[[1, 2]]`)
    —— R1 现状,照搬未「修正」。
  - 逐条数据与口径见 `benches/baseline.txt` **L0-A-3a 段**。

- **`wlwl:std.collection` 的 `GROUP_BY` / `DEDUP_BY` / `KEY_BY` 归 R2**
  (追加批次 M5 的 L0-A-3b 下半)。同款形态与非破坏性契约。
  - **为什么**:`Value::Dict` 的表示是 `Vec<(Value,Value)>`,`dict_lookup` 是
    `iter().position(...)` 线性扫描,而 `INDEX_SET` 每次还要**克隆整个 entries**
    ⇒ 键互异时 n 次插入是 O(n²) 的克隆 + O(n²) 的扫描。这是与 `PUSH` **无关的
    第二笔平方级**。归层后内部 `HashMap<String, usize>` 索引 + 末次物化,
    查找 O(n) → O(1)、克隆 O(n) → O(0)。实测 `GROUP_BY` 4 000 档
    4 512 → **14.4 ms**(**314×**),每元素成本由涨(4.9×)转降。
  - **`Value::Dict` 的表示不动** —— 插入序就是可观察序(JSON 输出、错误载荷、
    契约里冻结的 `display()`),换 `HashMap` 会让这些乱序 = breaking。
  - **键是 `STR(k)` 渲染串,不是值相等。** 实测对照:
    `DEDUP_BY([1, 1.0, 2], id)` 得 `[1, 1.0, 2]`(三个都留),
    而 `UNIQ([1, 1.0, 2])` 得 `[1, 2]`。**两者不是同一族。**
    正因为键是渲染串,`HashMap<String, usize>` 才是天然键,不必给 `Value`
    实现 `Hash`。
  - `KEY_BY` **位置留首次、值取最后**(实测 `["z1","y1","z2","x1"]` → `[z: z2, y: y1, x: x1]`)。
  - 差分:值 15/15、诊断 11/11;127 条冻结契约全绿。
  - ⚠️ `DEDUP_BY` / `KEY_BY` 的**归层前**数据**未测**(R1 的 `group_by/8000`
    单档 criterion 自报 214 秒,三个 8 k 档会让本轮跑到十几分钟),故本条**不
    声称它们各自的倍数**,结论支撑是机制 + 同族已实测的 `GROUP_BY`。见
    `benches/baseline.txt` **L0-A-3b 段**。
  - **`SORT` / `SORT_BY` 仍留 R1** —— 归层必须换 `Vec::sort_by`(保留选择排序
    只值 0.03%:实测 n=500 净 45 889 ms 里数组拷贝仅占 15.7 ms),而那会改
    **比较器调用次数**(R1 实测恰好 C(n,2):n=4/8/16/32/64 → 6/28/120/496/2016),
    对带副作用的比较器是可观察行为。实测 R1 `SORT` 在 n=2 000 上要
    **9.4 分钟**,`SORT_BY` 直接依赖它。待业主裁。
  - ⚠️ 顺带更正:原注释称选择排序「总拷贝量 O(n) 而不是 O(n²)」,**与代码矛盾**
    —— 代码每轮做 `SLICE`+`SLICE`+`CONCAT`(合计 `len(rest)` 个元素),拷贝
    **也是 Θ(n²)**。注释已改。

- **新增 `collection_regression.rs`(84 条)—— 归层等价性的永久证据**。L0-A-2 /
  3a / 3b 把 13 个成员从 R1 归到 R2,而**活的对拍在归层完成后就不存在了**
  (R1 实现已删)。故把**删码之前逐条录下的 R1 输出**冻结成常驻回归表:任何人
  日后改了这 13 个成员里的任何一个、而可观察输出变了,这张表立刻变红。
  与 `collection_contract.rs` 分工:后者是 M3-1 交付的 127 条冻结契约
  (**本批逐条未动**,自开工起该文件字节级 `git diff` 为空),本表只盯
  「归层有没有改语义」这一件事。
  表里有**三条刻意冻结的「不好看」行为**——它们是让缺陷显形的探针,不是
  在赞美缺陷:`SORT` 排不了布尔(D13-011)、`UNIQ` 与 `DEDUP_BY` 去重口径
  不同(值相等 vs `STR` 渲染串)、`KEY_BY` 位置留首次而值取最后。

### Fixed (规范文本 / 注释 —— 实现本身没变)

- **`SORT` 排不了布尔(偏差 D13-011,本批之前就存在)**。`std.collection` 的
  `_SORT_LT` 里 `&&(==(TYPE(a), "BOOLEAN"), ==(TYPE(b), "BOOLEAN"))` 判定两个
  布尔**可比**并调 `<(a, b)`,而语言的 `<` 只支持数值与字符串
  (`wlwl-eval` 的 `cmp_op` 对布尔直接 `Err`)。实测
  `SORT([TRUE, FALSE])` → `E0030: <cmp>: cannot compute less than for boolean
  and boolean` —— **布尔根本排不了**。**故意不修**:把「布尔可比」化是一次独立的
  语义裁决,不该夹在一次归层里带走;已在 `_SORT_LT` 注释处标注。给自定义比较器
  可绕开:`SORT([TRUE, FALSE], FUN((a, b), FALSE))` 正常返回。

### Fixed

- **`MIN_BY` / `MAX_BY` 空数组中止运行,违反 stdlib §5「空数组得 `NULL`」**(v0.11.2
  复核 N-6,偏差 D12-013)。`_EXTREME_BY` 的空数组守卫写成了 `<(LEN(arr), 0)` ——
  恒假,空数组照样把 `NULL` 喂给 key 函数,普通 key(如 `LEN`)直接
  `E0030` 中止;只有恒等 key 这类「对 NULL 免疫」的调用方才碰巧拿到 NULL。
  已改 `==(LEN(arr), 0)`,返回先于碰 key;契约表新增两条**严格 key** 的空数组
  用例(原有恒等 key 用例对本缺陷零防护,保留),probe 对照行同步换用严格 key。
- **净化器七道缺陷**(v0.11.3 收口时实测揪出,偏差 D13-003 ~ D13-010)。前四道
  是**静默**的 —— 不报错、只丢东西,且当时全部门禁全绿:
  - **D13-006**:`serialize` 以 `Enter(root)` 起栈,而 root 的 `NodeKind` 是
    `Text("")` 占位 ⇒ 匹配 Text 分支只写空串、**从不遍历孩子**。
    `HTML_SANITIZE` **对任何输入都返回空串**,旗舰成员整体不可用。
  - **D13-005**:过滤器挂载点 `target[id]` 的判据用错了对象(看**自己**的动作
    而非父的)⇒ Keep 节点被挂到不输出的 Unwrap 父节点下,那棵子树无人遍历 ——
    **任何非白名单元素里的内容整段消失**(`<div>y</div>` → `""`)。注释写的是
    「进父的挂载点」、代码写的是 `parent`,自相矛盾。
  - **D13-003**:树构建收到 `Token::RawText` 不弹栈。tokenizer 吞掉 `</script>`
    本体且不发 `End`,开标签永不出栈,其后内容全挂进这个「必被整删」的元素里
    跟着消失(`<script>evil</script>after` 的 `after` 全丢)。
  - **D13-004**:`DROP_WITH_CONTENT` 漏了 `noembed`,而规范 §13.2 明写它是
    raw-text 整删集成员 —— 其内文会被当标记解析(mXSS 面)。
  - **D13-008**:`plaintext` 被放进整删集,而规范 §13.2 的整删枚举里**没有它**、
    且明写「按普通文本处理」⇒ 实现比规范更狠,`<plaintext>hello` 丢成空串。
    已按规范移出整删集改走拆壳(内层 `script` / `javascript:` 照常处置 ——
    拆壳不放宽内层判定),规范同步补写「整删集恰好 8 个 raw-text 元素」。
  - **D13-009**:`policy` 传非 `DICT` 时复用了 `data` 的类型错模板,报
    `expected string, got array` —— 对着一个**本来就合法**的 string 参数说
    「要 string」,把错误指到了错的参数(调用方会照着去改 `data`)。
  - **D13-010**:`HTML_SANITIZE()` 的元数错复用共享 `arity()` 模板报
    「expects 2 argument(s)」,而 `policy` 可省、**1 个实参才合法**。
  - **D13-007** 记录共同根因:W-11 只交了模块内单测,**端到端零覆盖** ——
    上述四道静默缺陷因此全部存活。W-12 / W-13 补上「从入口走到出口」那条路
    后才封住。
- **clippy 8 处 / rustdoc 2 处**(含一条「断言恒真式」的假测试
  `scheme_check_normalization`,以及 M1 契约表里「`SANITIZE_HTML` 不得出现」
  的断言在 M3 落地后失效)—— CI 是 `RUSTFLAGS: -D warnings` + `clippy -D warnings`,
  这些会直接让流水线红。
- **门禁误红一处**(D13-092,发版前最后一处红)—— macOS job 的
  `sanitize::filter::tests::distinct_attribute_dedup_does_not_scale_quadratically`
  红,读数是「4× 的互异属性花了 **12.8×** 的时间(小=1ms 大=12.8ms)」。⚠ **红的
  不是被测代码**:同名属性去重早已是哈希集合(O(k)),这条守卫只是**判据形式**
  不可靠 —— 它用「两档规模(2 千 / 8 千属性)的**墙钟耗时比** < 12 倍」判「没
  退化成平方」,而**同一份实现**本地 debug 档量到 **4.0×**、CI 上量到
  **12.8×** ⇒ 越过阈值 ⇒ 误红(两档规模的内存占位不同 ⇒ 比值里混着缓存命中率,
  那部分随机器与并发负载漂移 ⇒ 缺陷形态与正常形态在 CI 上**没有稳定间隔**)。
  ⇒ 判据换成**确定性计数**(名字查找次数;两段式:校准 + 线性上界,与
  `tree.rs` 的 N-006 契约同体例)⇒ 同一份代码在本地与 CI **同红同绿**;
  负向验证 3/3 转红(平方形态校准段红 / 上界段红 / 绕过计量点红)。
  ⚠ **零行为变化**:去重语义(§13.2「同名属性重复时首个生效」)一字未改。
  同批另修 `ci.yml` 的 rust-cache 工作区指错(仓根**没有** `Cargo.toml` ⇒
  它的 `cargo metadata` exit 101 ⇒ 依赖集解析为空 ⇒ 缓存键**不含** manifest
  哈希 ⇒ 依赖变了缓存不失效)。

### 不做(留档,「不做」与「做」同权)

- **SQL 转义成员** —— 明示否决。OWASP 定性为 last resort 且方言分裂,而 wlwl
  没有 `std.db`,不存在需要它的场景;复活条件 = 出现 `std.db` 形态的绑定层。
  记录在标准库规范 §13.3,并由契约测试 `sql_escape_rejection_record_stays_in_the_spec`
  钉住(删掉那段规范会变红)。
- **加解密 / 密钥生成** —— 不做。撞 D12-006;`MD5` / `SHA-1` 已破不提供;
  SHA-3 / BLAKE3 记演进方向。记录在规范 §11.5。
- **不做流式 / 增量 API**(本批输入为整串)、**不承诺与浏览器解析器逐位一致**、
  **不修 D11-012**(平方级成本属 R0 解释器治本工程)。

### 门禁

`cargo fmt --check` 0 diff;`clippy --all-targets -D warnings` 0;`cargo doc --no-deps
-D warnings` 0 error;`cargo deny check` exit 0;`cargo check -p wlwl-std --features
real-ai` exit 0;`cargo test --locked --all-targets` **0 failed**(75 个测试二进制);
`--test conformance` 全绿;probe **175** 用例(与本版段开头「本批实测数字」里的
「probe 用例 175」一致 —— 旧数字 157 是 M3 批次的快照,已随 M5~M8 的新增用例
过期);附录 A 由
`gen-appendix-a` 重生成且双向锁绿;附录 G 逐字节不变;`sanitize_contract` 103 /
`encode_contract` 47 / `collection` 127 / `str_math` 156 条逐条冻结。基准存档进
`impl/crates/wlwl-eval/benches/baseline.txt`(M1 / M2 / M3 三段)。
⚠ **门禁本身也修了一处**(D13-092,见上条「门禁误红一处」):一条复杂度守卫的
**判据形式**不可靠,在 macOS CI 上误红;已换成**确定性计数**判据(本地与 CI
同红同绿),负向验证 3/3 转红,去重语义零变化。

### 追加批次(M8 · L3)—— `std.ai` / `std.agent` 移出 `wlwl:std.*`(可见面变更)

⚠ **这是需要申报的可见面变更(ADR-0023 §v0.x 1),但不是「移除成员」。**
成员的**名字、签名、语义一条未改** —— 变的只有**在哪个前缀下可达**。
因此**不走弃用两步**(不设警告码、不保留一个次版本)。

**迁移写法**(改 `IMPORT` 那一行即可,函数体一个字不用动):

```wlwl
// 之前
IMPORT("wlwl:std.ai",    ["ASK", "EMBED", "COMPLETE", "ASK_STREAM", "ASK_ALL"]);
IMPORT("wlwl:std.agent", ["TASK", "MODEL", "TOOL", "CALL_TOOL", "CONTEXT"]);

// 之后
IMPORT("ai:ask",    ["ASK", "EMBED", "COMPLETE", "ASK_STREAM", "ASK_ALL"]);
IMPORT("agent:task", ["TASK", "MODEL", "TOOL", "CALL_TOOL", "CONTEXT"]);
```

| 项 | 之前 | 之后 |
|---|---|---|
| `wlwl:std.ai` | 可解析 | **`E0043` / 不再解析** |
| `wlwl:std.agent` | 可解析 | **不再解析** |
| `wlwl:ai` | — | ⚠ **不可行**,见下 |
| 成员面 | 5 + 5 | **5 + 5,一字未改** |

⚠ **前缀不能带 `wlwl:`** —— `wlwl:` 是标准库的**保留前缀**,`wlwl:ai` 会落进
「裸模块名」分支、被当成文件名去找 `…\wlwl:ai.wll`,最终 `E0040`。
**只有 `ai:` / `agent:` 两种形态可用。**

⚠ **没有「全限定调用」这种语法** —— `ai:ask.ASK(…)`、`ai.ASK(…)` 以及旧式的
`wlwl:std.ai.ASK(…)` **一律解析失败**(`E0011 expected ')' got Colon`)。
成员只能经 `IMPORT` 取裸名。**旧文档里「推荐全限定写法以避免与类型名 `TASK`
混淆」的建议是错的,已废止**;要避开阅读混淆请在 `IMPORT` 时改名:
`IMPORT("agent:task", ["TASK": "AGENT_TASK"])`。裸名 `TASK` 与类型名 `TASK`
本来就不冲突(§2.1)。

⚠ **更正一条更早的错误陈述**:`v0.11.3` 段(以及标准库规范 §0)**原先**写的
「`std.ai` 的 5 个成员受 `real-ai` feature 门控,默认构建可见 99」**从未成立**。
`real-ai = ["dep:reqwest"]` 门控的是 **HTTP 传输层**(mock ↔ real),
成员函数本身一直编译 —— 默认构建下这 5 个成员**全部可见**。

⚠ **形态上的差距(如实记下)**:`ai` / `agent` 目前**仍以 `wlwl-std` 的
`real-ai` feature 编译**,只是不再出现在 `wlwl:std.*` 的可见面里;它们
**没有**独立的 crate 边界与分发产物 ⇒ 按 `addendum-14` §6,本批记为
**部分完成**,不是搬完。裁决见 `ADR-0034`(D1 已撤回、D2 已定)。

**规范与文档同步**:标准库规范 §9 / §10 改写为官方包正文(章号不变);
§14 的演进方向条目标记为已落地;语言规范 §9.3 的前缀枚举与 §10.11 标题同步;
skill 的指针表与失效交叉引用一并修正。

### 追加批次(M7 · L2)—— `std.json` 深度面

**`wlwl:std.json` 成员面 2 → 11**。⚠ **`PARSE` / `STRINGIFY` 的可观察行为
**一条未改**(照 Go `encoding/json/v2` 保留旧行为的体例),新者全部是增量。

| 组 | 新成员 | 用途 |
|---|---|---|
| 词法层 | `JSON_TOKENIZE` `JSON_LOCATE` `JSON_VALIDATE` | **位置**。从这里开始的每个错误都说 **哪里** |
| 路径族 | `JSON_GET` `JSON_SET` `JSON_DELETE` `JSON_MERGE` `JSON_CHUNK_ITEMS` | 寻址与不可变编辑 |
| schema | `JSON_CHECK` | JSON Schema 的**闭合子集** |

⚠ **同一命名空间里存在两套失败口径**,且都是规格:旧两成员报
**`E0070` / `E0071` 诊断**(会中止运行),新九个返**`ERR` 值**(用 `IS_ERR` /
`ERR_PAYLOAD` / `UNWRAP_OR` 消费)。

**新登记的缺陷(本批不修,见 `docs/history/$1` 缺陷表)**:

- **D13-016** —— `PARSE` 对 **i64 之外**的整数静默丢精度
  (`-9223372036854775809` 变成 `-9223372036854775808`,值被改了)。
- **D13-017** —— `PARSE` **不保文档序**,键被排序(规范 §3 明写保序)。
  根因:`serde_json` 未开 `preserve_order`。同一根因还导致 `PARSE` 的每字节成本
  从 1 KB 到 1 MB 涨 **2.74 倍** ⇒ `addendum-09` §4 的**规模契约**未达成。

⚠ **工具链**—— `impl/deny.toml` 登记 `rustls 0.23.45` 的四条
RUSTSEC(2026-0346 ~ 0349)为**已知未修复**:锁文件未动、`0.23.45` 已是最新兼容版
(无可升目标),依赖链仅在 `--features real-ai` 的 HTTP 路径上。

