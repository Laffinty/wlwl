# 05 · `std.net` —— 网络(零第三方依赖的最小面)

> **层级** L1(可并行) · **前置** 无(05 与 06 都要碰宿主能力,**须确认边界不重叠**)
> **状态** 🚧 **W-01 已结(2026-10-09)** —— [`ADR-0028`](../adr/0028-std-net-shape.md)
> **Accepted**(业主采纳 **A 引入 / 阻塞原语**):D1 `HTTP_*` 阻塞原语(与 `SLEEP`
> 同族)、D3 手写明文 HTTP/1.1 **零新增依赖**、以及被它们逼出来的 D2(超时必填,
> 缺省 30 s)/ D4(不自动跟随重定向)/ D5(05 与 06 的边界)。
> ⚠ **D1 是带条件冻结的**:前提是「`ADR-0017` Step 3 仍未落地」,守卫测试
> `impl/crates/wlwl-eval/tests/adr_conditional_guard.rs` 会在前提不成立时转红。
> 规范登记 §14 候选命名空间

## 0. 契约摘要

- **目标**:给 wlwl 第一条网络能力,且**形态足够小** —— 只做「发请求 /
  收响应」,不做框架。
- **非目标**(违反即范围事故):
  - **不做 TLS**。理由:零第三方依赖纪律 + TLS 实现不是能顺手写对的东西。
    明写「明文 HTTP only」并要求调用方自备隧道 / 反代。
  - **不做 WebSocket / SSE / gRPC**、不做连接池、不做重试策略、不做
    代理 / DNS 解析的定制。
  - **不引入 async 语法**。wlwl 的并发形态是 `SCOPE` / `STEP` / `SPAWN` +
    通道(真挂起,v0.9),本份**只做阻塞式**并把阻塞挂到那个形态上。
  - 不做 TLS 相关的任何「简易封装」。
- **为什么最小面可行**:wlwl 现在连一次 HTTP 都发不出去,`std.ai` 的
  `real-ai` 路径是靠 `reqwest`(可选依赖)绕过这件事的 —— 那是**实证**:
  仓里已经有一个成员在用第三方网络栈了(门禁里有
  `cargo check -p wlwl-std --features real-ai`)。

## 1. 基线(可核验)

```powershell
cd impl
Select-String -Path crates/wlwl-std/Cargo.toml -Pattern 'reqwest|optional' -Context 0,3
Select-String -Path crates/wlwl-std/src/ai.rs -Pattern 'reqwest|feature' | Select-Object -First 6
cd ..
Select-String -Path docs/stdlib/wlwl-stdlib-spec-v0.11.md -Pattern 'std.net'
```

| 项 | 现值 | 出处 |
|---|---|---|
| 网络成员 | **0** | 附录 A(104 成员) |
| 已存在的网络依赖 | `reqwest` = **optional**,受 `real-ai` 门控,默认构建不含 | `wlwl-std/Cargo.toml` |
| 门禁 | `cargo check --locked -p wlwl-std --features real-ai` 是 §3.3 的一条 | 计划 §3.3 |
| 规范登记 | §14 新 R2 候选:`std.net` | 规范 §14 |
| 主流对照 | Go `net/http`、Python `http` + `asyncio`、JS `fetch`、Java `java.net.http` | — |

## 2. 成员面

**层:R2 全员**。

| 成员 | 层 | 语义要点 | 失败口径 |
|---|---|---|---|
| `HTTP_GET(url, headers?, timeout?)` | R2 | 返回 `DICT`:`{status, headers, body}` | 连接 / 超时 / 非 2xx → `ERR([kind: HttpError, op, reason])` |
| `HTTP_POST(url, body, headers?, timeout?)` | R2 | 同上 | 同上 |
| `HTTP_REQUEST(method, url, body?, headers?, timeout?)` | R2 | 其余动词的通用形态 | 同上 |
| `URL_PARSE(url)` | R2 | 拆成 `{scheme, host, port, path, query, fragment}`。**纯解析,不发请求** —— 这是最常被单独需要的那个 | 非法 URL → `E0030` |
| `URL_JOIN(base, rel)` | R2 | 相对 → 绝对 | 非法 → `E0030` |

**失败口径的归属**:网络失败是**运行期可预期结果**(会重试、会降级、
会记日志),所以是 `ERR` **值**并带 `kind = "HttpError"` + `op` + `reason` ——
理由与 `std.encode` 的 `DecodeError` 同款(把「参数错」和「网络不通」分成
两种 kind,调用方才分得清)。**它绝不是原生诊断**。

> ⚠️ 这与 `std.sanitize` / `std.test` 的「永不因内容失败」是**不同族**,
> 规范要把这条分界写明:`sanitize` 不失败是因为失败会让净化器变成 DoS 面;
> `net` 失败是因为失败是业务的正常分支。

## 3. 语义裁决点(开写前必须逐条回答)

1. **阻塞 vs 挂起**:阻塞调用会占住一个任务。wlwl 有真挂起(§17)——
   阻塞 I/O 能不能被调度器挂起?若不能,`HTTP_GET` 在 30 s 超时下会**冻住**
   整个 `SCOPE`。→ 裁决:阻塞 I/O 的挂起点(需要一个「等待 fd 可读」的
   调度器支持)还是「明写会阻塞且写进非目标」。**这是本份最大的技术风险。**

   > ✅ **[2026-10-09 已裁决 → `ADR-0028` D1]** —— **不能挂起,`HTTP_*` 是阻塞
   > 原语**,与 `SLEEP`(2026-10-06 业主方案 E)同族。四条实测依据见 ADR §1。
   > 关键一条常被低估:**阻塞在本仓不是新问题** —— `std.ai` 的 `real-ai` 路径
   > 本来就用 `reqwest::blocking::Client` 在运行时线程上内联调用,已受 feature
   > 门控 + CI 隔离。⚠ **代价必须原样写进规范**:`HTTP_*` 执行期间不调度任何
   > 其他任务;且「在同一个 `SCOPE` 里并发发多个 HTTP 请求」是**错的写法**
   > (`SPAWN` 了也还是排队),不是低效的写法。

2. **零依赖怎么做到 HTTP?** 手写 HTTP/1.1 解析(含 chunked、Content-Length、
   压缩响应头)。这是**真工作量**且容易出安全洞(请求走私 / 响应拆分)。
   → 建议裁决:本批只支持 **`http:` 明文 + 简单响应**,并在规范里明写
   「不支持 chunked / 不做响应头折叠」—— 或者引入 `ureq` 一类小型零依赖
   crate(但那破了「零第三方依赖」纪律,须显式裁决)。

   > ✅ **[2026-10-09 已裁决 → `ADR-0028` D3]** —— **手写,零新增依赖**。
   > 上一条担心的「真工作量」比预想的小:TCP 与 DNS 都在 **std** 里
   > (`std::net::TcpStream` / `ToSocketAddrs`),仓内当前**没有任何一处**用到
   > `std::net`,CI 只构建原生目标。⇒ 不引 `ureq`(那只会**换到依赖、换不到
   > 能力**)。安全洞按「**不支持的写法一律 `ERR`,绝不尽力解析**」正面回答:
   > `chunked` 不解码(走私向量)、框架有歧义即 `ERR`、头块与响应体各有上限。
3. **默认超时**:`timeout` 参数有默认值吗?没有 ⇒ 调用方忘写就永久挂住。
   建议给一个**保守默认**(如 30 s)并在规范写明它是可覆盖的。
4. **响应体大小上限**:一个 `Content-Length: 10 GB` 的响应要不要全读进内存?
   → 必须有上限,越限是 `ERR` 不是 OOM。
5. **重定向跟随吗?** 跟随 ⇒ 可能把请求带到非预期 host(SSRF 面)。→ 建议
   **不自动跟随**,把 3xx 原样返回,由调用方决定。
6. **与 06 的边界**:DNS 解析、连接池、代理 —— 归 05 还是 06?建议归 05,
   06 只管进程 / 文件 / 环境。

## 4. 验收门禁

```powershell
cd impl
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --all-targets
cargo deny --locked --all-features check
$env:RUSTDOCFLAGS='-D warnings'; cargo doc --locked --no-deps
cargo check --locked -p wlwl-std --features real-ai
```

契约表 `tests/net_contract.rs`(新)。**网络测试不得依赖外网**(CI 不稳定 +
违反可复现)⇒ 用一个**仓内起的本地 HTTP 夹具服务器**:

| 组 | 用例 | 备注 |
|---|---|---|
| `URL_PARSE` / `URL_JOIN` | RFC 3986 §5.4 的参考解析器用例**逐字冻结** | 纯函数,可全量跑 |
| 形态 | 元数 / 类型 / 非法 URL → `E0030` | 逐字 |
| **集成** | 对本地夹具:200 / 404 / 500 / 响应超时 / 连接拒绝 | 每条一个用例 |
| **不跟随重定向** | 夹具返 302 → 断言**没有**跟到目标 | 防 SSRF |
| 响应体上限 | 夹具声明超大 `Content-Length` → `ERR` 而非 OOM | 机制断言 |
| 门禁内跑 | 测试自身**不得**访问 `example.com` 之类外部地址 | 可用 `NET_TEST_OFFLINE=1` 环境变量硬门控 |

## 5. 九处表落点

九处见 `docs/history/20261002.md` §5.0。**本份九处全中**(新命名空间):

| 落点 | 本份动作 |
|---|---|
| 1 规范成员表 | 新增 §17 `std.net` 章 + 6 行 + **「明文 only」条款** |
| 2 附录 A | 跑 `gen-appendix-a` |
| 3 `SPEC.functions` | 新 `src/net.rs` + 6 个 `StdFn` |
| 4 契约测试硬编码 | `net_contract` 成员数与点名清单 |
| 5 模块登记 | `ALL_SPECS` 登记 `net`;`src/` 文件数守卫同步 |
| 6 probe | ≥ 1 条(**只能用本地夹具**,probe 不得联网),`EXPECTED_CASE_COUNT` 同步 |
| 7 附录 G | **可能动**:若阻塞 I/O 需要新的调度器形态 ⇒ 附录 G 与 `spec_appendix_g_sync` 跟着改(裁决点 §3.1 的后果) |
| 8 skill 指针 | `SKILL.md` + `reference.md` §9.9,**必写明「明文 only」** |
| 9 CHANGELOG | 成员面 +6;命名空间 +1 |

## 6. 风险与已知代价

- ⚠️ **阻塞 I/O 与真挂起的相容性未定**(§3.1)。这是本份的**头号风险**:
  做不成挂起,`HTTP_GET` 就会在超时场景冻住整个 `SCOPE`,那比没有网络更糟
  (它会让人误以为并发可用)。**先裁决,再开写。**
- ⚠️ **手写 HTTP 解析容易出安全洞**(请求走私 / 响应拆分 / 头注入)。若裁决
  走「零依赖手写」,建议**先只支持最小子集**并把不支持的显式写成 `ERR`,
  而不是「尽量解析」。
- ⚠️ **明文 only** 是一个会被真实用户抱怨的限制。规范必须把它写在**成员表
  的第一行**,而不是藏在小节里。
- **明确不做**:TLS / WebSocket / SSE / gRPC / 连接池 / 重试策略 / 自动重定向
  / 代理 / DNS 定制 / 流式响应体 / cookie jar。

## 7. 检查点

| 子项 | 内容 | 状态 | commit |
|---|---|---|---|
| W-01 | 阻塞 / 挂起裁决 + 零依赖形态裁决(ADR) | ✅ **完成(2026-10-09,业主采纳 A)** —— [`ADR-0028`](../adr/0028-std-net-shape.md)**Accepted**:**D1** `HTTP_*` 阻塞原语(与 `SLEEP` 同族)、**D3** 手写明文 HTTP/1.1 **零新增依赖**、**D2** 超时必填缺省 30 s / **D4** 不自动跟随重定向 / **D5** 05 与 06 的边界。⚠ **D1 带条件冻结**:前提是「`ADR-0017` Step 3 仍未落地」,守卫测试 `impl/crates/wlwl-eval/tests/adr_conditional_guard.rs` 在前提不成立时**转红**并写明该重裁什么(见 ADR §6)。⚠ 查证时发现**两条把问题改写的事实**:① 阻塞在本仓**不是新问题** —— `real-ai` 那条路径本来就在阻塞(`reqwest::blocking::Client` 内联调用),已被 feature 门控与 CI 隔离;② 「等真挂起」**不是空话** —— `ADR-0017` 2026-09-25 已 **Accepted**、Step 1–2 已落地(通道是真挂起的),只差 Step 3,而业主 **2026-10-08 已裁决把 Step 3 与 `addendum-12` 一并往后放**。**顺带更正一处过时数字**:`addendum-12` §0.1 记的「`yield_split.rs` 338 行」实测已 **365 行** | `7581471` / `29b9dae` |
| W-02 | `URL_PARSE` / `URL_JOIN`(纯函数,先落) | 未动工 | — |
| W-03 | 夹具服务器 + `HTTP_*` 实现 | 未动工 | — |
| W-04 | 契约表(含离线门控 / 上限 / 不跟随重定向) | 未动工 | — |
| W-05 | skill / CHANGELOG / 收口 | 未动工 | — |
