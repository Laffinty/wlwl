# wlwl 侧需求清单 —— 支撑 TinyPage 由 Python 迁移至纯 wlwl

| | |
|---|---|
| **提给** | wlwl 团队（规范侧 `wlwl-spec-v1` + 实现侧 `wlwl`） |
| **提出方** | TinyPage 迁移项目 |
| **日期** | 2026-10-27 |
| **当前基线** | 编译器 `wlwl` **v0.12.0**（`x86_64-pc-windows-msvc`）／规范 `wlwl-spec-v1` **v1.1.0.3** |
| **证据来源** | 一次带实测的迁移可行性评估（9 个可运行探针，见文末附件）。**清单中每条「现状」都是实测结论，不是读文档得出的推测。** |

---

## 0. 背景（请先读这一节）

### 0.1 我们要迁的是什么

TinyPage 是一个**静态站点生成器 + 数字花园 + HTTP 服务 + 管理后台**，当前是 Python 实现（5738 行，含测试 691 行）。目标形态是**纯 wlwl 实现、零 Python 运行时、零前置 nginx**。

功能面拆解（迁移后必须全部在线）：

| 子系统 | 内容 | Python 侧现状 |
|---|---|---|
| 静态服务 | 预生成 HTML/CSS/JS + 字节流正确的 `Content-Length` + 缓存策略 | `frontend.py` + waitress WSGI |
| 内容渲染 | Markdown → HTML、目录（TOC）、脚注、`[[wikilink]]` 双向链接、反向链接、标签图谱、mermaid | mistune + `parsers/`（6 个模块 552 行） |
| 站点产物 | 首页、分页列表、分类页、RSS、Sitemap、robots、搜索索引（JSON） | `generator.py`（666 行） |
| 管理后台 | 12 个写操作、登录/会话、主题切换、文件上传、文章增删改、页面增删改、实时预览 | `admin/`（4 个模块 786 行） |
| 安全层 | CSRF（HMAC + double-submit cookie）、会话管理、滑动窗口限流、登录失败渐进锁定、审计日志、安全响应头 + CSP、路径穿越防护、XSS 转义 | `security.py`（465 行） |
| 部署形态 | **单进程、双端口**（公开 8080 + 管理 8081）、**自带 TLS**、可作为反向代理后端、可嵌入已有部署 | waitress + nginx（`nginx.conf`） |

### 0.2 我们已经验证到哪一步

在 v0.12.0 上，我们**已经实测跑通**了一个纯 wlwl 的 HTTP 服务：

- 8 个并发请求全部 `200`，正文与磁盘一致，`Content-Length` **字节数**正确；
- 5 个安全响应头随响应发出；
- 路径穿越请求 → `403`；`<script>` 载荷 → `403`；`union select` 载荷 → `403`；
- 单 acceptor 任务连续服务 4 个请求全部 `200`。

**⇒ 结论：wlwl v0.12.0 的语言与标准库已经足以承载这个应用的"服务层"。真正缺的是「TLS」「HTTP 协议层的现成原语」「并发模型里几个不安全的静默行为」三类。**

### 0.3 优先级定义

| 级别 | 含义 |
|---|---|
| **P0** | **阻塞项**。不做，迁移无法进行，或会产出**静默错误**（不报错、直接断连）。 |
| **P1** | 迁移可做，但工作量显著上升、风险显著变高，或长期会被反复踩。 |
| **P2** | 体验/长期演进。可排入后续版本，不阻塞本次迁移。 |

### 0.4 一句话请求

> **请优先交付 A 组（TLS）与 B 组（并发模型的可诊断性）。** A 组决定项目时间表（目前 wlwl 路线图里没有 TLS，周期不由我们决定）；B 组决定迁移期是否会陷入"偶发断连、无法定位"的消耗战。

---

## A 组 · TLS 与出站安全（P0，阻塞项）

### A-1 · TLS 服务端：`std.net` 支持 TLS 监听与握手 `P0`

- **现状（实测）**：`HTTP_GET("https://…")` → `ERR([kind: HttpError, op: HTTP_GET, reason: scheme "https" is not supported — plaintext http: only (ADR-0028 §0)])`。系统册 §21 写明「只支持明文 `http:`，**不做 TLS**」「明确不做：TLS / WebSocket / SSE / gRPC / 连接池 / 重试策略 / 自动重定向 / 代理 / cookie jar / 流式响应体」；安全册写明「**对称 / 非对称加密不做**」。`wlwl-v1.0-roadmap.md` 全文**无 TLS 条目**。
- **期望**：`std.net` 在传输族之上提供 TLS。建议二选一或都做：
  - **(a) 独立成员**：`TLS_LISTEN(host, port, opts)` / `TLS_ACCEPT(listener, timeout_ms)` / `TLS_SEND` / `TLS_RECV`；
  - **(b) 传输族加选项**：`TCP_LISTEN(host, port, ["tls": ["cert":…, "key":…, "ca":…]])`，后续 `TCP_ACCEPT` 自动升级为 TLS。
  - `opts` 至少支持：`cert`（PEM 证书链路径）、`key`（PEM 私钥路径）、`ca`（可选，客户端证书校验）、`min_version`（TLS1.2/1.3）、`alpn`、`sni`（多证书映射）。
- **验收判据**：
  ```wlwl
  LET(l, TLS_LISTEN("127.0.0.1", 0, ["cert": "server.pem", "key": "server.key"]));
  ```
  - `curl -k https://127.0.0.1:<port>/` → `200`；
  - 明文客户端连 TLS 端口 → **被拒且有明确错误**（不得静默降级为明文）；
  - 证书文件缺失/私钥不匹配 → 返回 `ERR([kind: "TlsError", …])`，**不得 panic、不得 abort**；
  - 证书链不被客户端信任时，客户端侧报的是**证书错误**而不是"连接被重置"。
- **为什么必须上游做**：传输层是"只搬字节"的原生层，且有机械守卫 `no_http_semantics_in_the_transport_layer` 保证它永远不认识协议；wlwl 语言层也没有任何对称/非对称加密原语。**⇒ TLS 只能在原生层实现，wlwl 程序层无法自行拼出。**

### A-2 · TLS 客户端：HTTPS 出站 `P0`

- **现状（实测）**：同 A-1；`HTTP_GET` / `HTTP_POST` / `HTTP_REQUEST` 一律拒绝 `https`。
- **期望**：`HTTP_*` 支持 `https://` scheme，且能配置：CA 信任（系统证书库 or 指定 `ca` 文件）、**证书固定**（pin SPKI 摘要）、客户端证书（mTLS）、TLS 版本下限、是否校验证书（**默认必须校验，不允许全局默认关闭**）。
- **验收判据**：
  - `HTTP_GET("https://example.com")` → 返回响应（或明确的非 TLS 原因）；
  - 对**过期证书**的自签名站点 → 返回 `ERR(kind:"TlsError", reason:"certificate expired")`，**不是**笼统的连接错误；
  - 关闭校验开关后行为可预测，且该开关**必须显式传入**（不能靠"环境变量没设就默认关"）。
- **关联场景**：管理端 AI 助手（OpenAI / DeepSeek 等）全部是 https 出站；`ai:ask` 官方包的真实传输也走同一条路（`real-ai` feature）。

### A-3 · 信任链与证书校验原语 `P0`

- **期望**：`std.encode` 或新的 `std.tls` 提供：X.509 解析（取 subject / SAN / 有效期 / 指纹）、SPKI 摘要（用于 pinning）、链校验结果可读。至少让调用方能实现"证书固定"而不必自己解析 ASN.1。
- **验收判据**：`std.encode::CERT_FINGERPRINT("server.pem")` 之类能返回稳定摘要；对同一证书两次调用结果一致。

### A-4 · 加密原语补全（TLS 的前置依赖）`P0`

- **现状（实测）**：`std.encode` 现有 16 个成员 —— `SHA256` / `HMAC_SHA256` / `PBKDF2_ITER` / `ARGON2ID` / `RANDOM_BYTES` / `RANDOM_HEX` / `TIMING_SAFE_EQ` / base64 / hex / URL 编解码 / UTF-8 字节层。**没有** SHA-384/512、HKDF、AES-GCM、Ed25519/X25519、RSA/ECDSA 验签。
- **期望**：补齐 TLS 实现与常见安全用途所需：SHA-384/SHA-512、HMAC-SHA512、HKDF、AES-256-GCM（可选）、Ed25519 与 ECDSA/RSA **验签**（不需要签名生成也行，但需要验签）。**保持"零新增第三方依赖"的既有约束，或在规范里明确这次破例。**
- **验收判据**：每个新成员有契约测试；`PBKDF2_ITER` / `ARGON2ID` 现有的 OWASP 下限校验行为不得回归。

### A-5 · TLS 相关的安全默认值 `P1`

- **期望**：证书校验**默认开启**；TLS 版本下限默认 1.2；不提供"一键关闭全部校验"的全局开关（若提供，必须是显式参数 + 运行时可见的告警）；私钥不得出现在错误消息或日志里。
- **验收判据**：错误消息与 `PRINT` 输出中不出现私钥字节；未传 `ca` 时使用系统信任库而非空信任。

---

## B 组 · 并发模型的可编程性与可诊断性（P0）

> 这一组全部来自实测。共同特征：**当前行为不报错，只是安静地做错事**。迁移期最贵的成本就是这类问题。

### B-1 · 宿主挂起点放进嵌套闭包时，必须报明确诊断（不得静默终止）`P0`

- **现状（实测）**：把 `TCP_RECV` 写进辅助函数、再由任务体调用 ⇒ **任务静默终止**：`AWAIT` 拿到 `NULL`、**stderr 全空**、客户端只见 RST。规范 §17.0 要求「宿主挂起点成员的调用点必须在任务体内」，实现**没有报 `E0014`**。
  实测输出（探针 `p09`）：
  ```
  [H] before accept
  [H] before accept          ← 段首重放，符合规范
  [H] after accept, is null? FALSE
    [SERVE] entered          ← 进入嵌套闭包
  scope done, z = NULL       ← 任务在这里无声死掉，没有任何报错
  ```
- **期望**（二选一，**都不接受"静默"**）：
  - **(a) 支持**：宿主挂起点出现在任务体调用的闭包内时，能正确挂起与恢复（按调用栈恢复帧）；
  - **(b) 明确拒绝**：产生 `E0014`，消息里**指名成员名**（如 `TCP_RECV`）、**位置指向调用点**（行+列），**不得**报成"YIELD 出现在任务体外"（用户写的是 `TCP_RECV`，报 YIELD 会把他引向一个他没写的构造）。
- **验收判据**：
  ```wlwl
  LET(helper, FUN((c), (TCP_RECV(c, 4096, 1000))));   // 期望：要么跑通，要么明确报 E0014 指向此行
  ```
  - 绝不允许出现「任务静默结束 + 无诊断 + 客户端只见 RST」。

### B-2 · 多任务在同一 listener 上 accept 的语义 `P0`

- **现状（实测）**：一个连接到达会**唤醒全部**等待者；没抢到连接的 acceptor 拿到 `NULL`（语义等同超时）并**直接死掉**。3 个 acceptor + 1 个连接的实测：
  ```
  h1 conn null? FALSE
  h2 conn null? TRUE
  h3 conn null? TRUE
  ```
  连带后果：所有 handler 提前结束 → `SCOPE` 提前退出 → **监听套接字被关** → 后续请求连接被拒（客户端收到 `000` / RST）。
- **期望**：明确并文档化 accept 的并发语义。可选方案：
  - **(a)** 明确"**同一 listener 只允许一个 acceptor**"，把当前行为变成受控诊断（第二个 acceptor 启动时立刻报明确错误）；
  - **(b)** 提供 `TCP_ACCEPT_SHARED(listener, timeout_ms)`，多等待者安全竞争，落选者返回可区分的"未获得"值（不是 `NULL`）。
- **验收判据**：
  - 若选 (a)：起第二个 acceptor 必须**立刻**得到明确诊断，而不是安静地拿 `NULL`；
  - 若选 (b)：3 个 acceptor + 1 个连接时，落选者必须**保持存活**并能继续接受后续连接。

### B-3 · 可推进的循环：循环体内的挂起点 `P0`

- **现状**：规范 §17.0 写明「循环体内的宿主挂起点必须是循环体的第一条语句」，并指出「段内的局部状态活不过一次挂起 ⇒ **循环推不动**」。当前没有可用形态 —— 我们只能用**静态展开的 accept 链**代替 `WHILE`（这会让 accept 段代码随连接数线性膨胀，代码生成器能缓解，但语义上仍是 workaround）。
- **期望**：提供一种**循环体可挂起且循环状态能正确推进**的形态。候选：
  - `FOR_EACH_SOCKET(listener, timeout_ms, FUN((conn), …))` 迭代器式（挂起点在迭代器内部，循环状态由运行时托管）；
  - 或让 `WHILE`/`FOR` 的循环变量在**段重放**后保持递增（放宽 §17.0 第 3 条对循环变量的限制）。
- **验收判据**：一个能连续接受 20 个连接的 acceptor 循环，**代码里不出现展开的重复段**；中途断开一个连接不影响后续。

### B-4 · 跨任务共享可变状态的安全原语 `P0`

- **现状**：规范 §17.0.1（normative）**禁止**任务间通过共享可变单元传数据。而限流计数、会话表、审计计数器、缓存**本质上就是跨请求共享状态**。当前只能靠"属主任务 + 通道"自建，成本很高且易错。
- **期望**：提供一种**被规范认可**的共享状态设施，二选一：
  - **(a) 共享容器**：`SHARED_DICT()` / `SHARED_LIST()`，提供原子更新成员（`SHARED_UPDATE(sd, key, fn)` 之类），并在规范 §17.0.1 中**明确豁免**这类用法；
  - **(b) actor 模式**：`ACTOR`/`MAILBOX` —— 一个属主任务 + 邮箱，提供 request/response 往返原语（"发消息并等回复"），让限流/会话这类状态有归属。
- **验收判据**：两个并发 handler 共享一个计数器，最终计数**精确等于**请求总数（无丢失更新）；且该用法**不被 §17.0.1 判违规**。

### B-5 · 非阻塞的出站调用 `P0`

- **现状（实测 / 规范）**：`HTTP_*`、`PROCESS_RUN`、`SLEEP` 都是**阻塞原语**，会**冻结整个 `SCOPE`**（只有 `TCP_*` 是"只挂起调用任务"的宿主挂起）。⇒ 在请求处理链路里做任何出站调用（AI 助手、回调外部服务、跑外部命令），**整站停摆**。
- **期望**：提供不冻结其它任务的变体：`HTTP_REQUEST_ASYNC`（宿主挂起式）/ `PROCESS_RUN_ASYNC` / `SLEEP_ASYNC`（`SLEEP_ASYNC` 已有，HTTP/PROCESS 没有）。或者至少把 `HTTP_*` 改为"只挂起调用任务"。
- **验收判据**：一个 handler 正在等待一个慢速 HTTP 响应（10s）时，**其它 handler 仍能正常响应**（当前做不到 —— 整站会卡 10s）。

### B-6 · 任务终值必须可区分「正常结束」与「异常死亡」 `P0`

- **现状（实测）**：B-1 的案例里 `AWAIT(h)` 返回 `NULL`，与"任务正常返回 `NULL`"**无法区分**。`z = NULL` 这一个值让排障彻底失去抓手。
- **期望**：任务因错误/取消/异常结束时，`AWAIT` 必须返回**带 `kind` 的 `ERR`**（如 `ERR(kind:"TaskFailed", reason: …)`），而不是裸 `NULL`。正常的 `NULL` 终值保持 `NULL`。
- **验收判据**：`AWAIT` 返回值能被写代码区分「成功 / 失败 / 被取消」三种情况。

### B-7 · 优雅停机（信号处理）`P1`

- **期望**：`std.process` 或 `std.signal` 提供 `ON_SIGNAL(sig, handler)`（至少覆盖 SIGINT / SIGTERM，Windows 上覆盖 CTRL_C_EVENT / console close）。停机语义应为：停止 accept → 等在途请求完成（可配上限时）→ 落盘 → 退出。
- **验收判据**：收到 SIGTERM 后，进程在**上限时间内**退出，且在途请求被正常响应完毕（不是 RST）。

---

## C 组 · HTTP 协议层（现在全要手写）`P0`/`P1`

> 传输层是"只搬字节"的原生层，**没有** HTTP 解析、没有 `Content-Length` 处理、没有状态码 —— 这些"live in WLWL code"。对迁移项目而言，这意味着每个项目都要把 HTTP 解析器重写一遍。

### C-1 · HTTP 请求解析原语 `P0`

- **期望**：`std.net`（或新 `std.http`）提供：解析请求行（方法/路径/版本）、解析 header（大小写不敏感、支持重复 header）、解析 body（`Content-Length` 与 `chunked` **都要支持**）、URL 路径与 query 拆分、百分号解码（**注意与 `+` 的语义**）。
- **验收判据**：能解析出方法/路径/版本/header 字典/body 字节；`chunked` 请求体可正确还原；header 名大小写不敏感；重复 header 可取到全部值。

### C-2 · HTTP 响应构造原语 `P0`

- **期望**：`Response(status, headers, body)` 或等价构造器，自动按**字节数**算 `Content-Length`（当前需手写 `FORMAT("Content-Length: {0}", BYTE_LEN(body))`），支持多值 header、任意 header 顺序、`HEAD`（只发头不发体）。
- **验收判据**：构造出的响应头顺序与 body 长度**始终一致**；`BYTE_LEN` 与 `LEN` 混用导致的错长不可能发生（构造器内部只认字节数）。

### C-3 · Keep-Alive / 连接复用 `P1`

- **现状**：我们目前只能 `Connection: close` —— **每个请求一条 TCP 连接**。生产环境（尤其有 PWA / Service Worker 的站点）代价明显。
- **期望**：handler 能选择"读下一条请求"而不是关闭；提供连接空闲超时。
- **验收判据**：一个连接上连续处理 3 个请求，客户端与服务端均正常；空闲超时后服务端主动关闭。

### C-4 · 路由原语 `P1`

- **期望**：路径模式匹配（`/article/{slug}.html`）、参数提取、方法区分（GET/POST）、404/405 语义。
- **TinyPage 场景**：管理后台 12 个写操作 + 静态资源 + API 路由。

### C-5 · Cookie 解析与生成 `P1`

- **期望**：`ParseCookies(header)` / `SetCookie(name, value, opts)`，支持 `HttpOnly` / `Secure` / `SameSite` / `Max-Age` / `Path` / `Domain`。
- **TinyPage 场景**：CSRF double-submit cookie、会话 cookie。

### C-6 · 表单与 multipart 解析 `P1`

- **期望**：`application/x-www-form-urlencoded` 与 `multipart/form-data`（含文件 part）解析。
- **TinyPage 场景**：管理后台的文章/页面表单、文件上传。

### C-7 · 静态文件服务原语 `P1`

- **期望**：`StaticFile(root, opts)` 一次性给出：路径穿越防护（**内建、正确实现**）、MIME 猜测、`Content-Length`、`ETag` / `Last-Modified` / `304`、`Range`、`Cache-Control`、目录 → `index.html`。
- **特别强调**：路径穿越防护必须是**内建且跨平台正确**的 —— 我们实测踩到 `PATH_NORMALIZE` 保留 `\`、`PATH_JOIN` 产出 `/` 的坑，导致前缀比对在 Windows 上**必然失败**、全站 403。

### C-8 · 请求级总预算与分级超时 `P1`

- **现状**：`TCP_SET_TIMEOUT` 已有（连接级读超时），但没有"整条请求预算"概念（Python 侧 waitress 有 `channel_timeout`）。
- **期望**：请求级总时限（首字节 / 头完成 / body 读取 / 响应写出 分别可配）。

### C-9 · 请求头与请求体大小限制 `P1`

- **期望**：最大 header 尺寸、最大 body 尺寸（超限 → 413 / 431，而不是无限读进内存）。
- **TinyPage 场景**：原 Python 版对上传有 `client_max_body_size 10m` 的限制（见 `nginx.conf`）。

### C-10 · 压缩与 HTTP 集成 `P1`

- **现状**：`std.compress` 有 `GZIP_COMPRESS` / `ZSTD_COMPRESS` 等 8 个成员，但与 HTTP 层没有集成。
- **期望**：`Content-Encoding` + 协商（`Accept-Encoding`）的一体化，或至少提供 `CompressResponse(resp, encoding)`。

### C-11 · chunked 响应体的客户端解码 `P1`

- **现状（实测）**：`HTTP_GET("http://example.com")` → `ERR(… Transfer-Encoding "chunked" is not supported — chunked is a smuggling vector, refused rather than decoded (ADR-0028 D3))`。
- **说明**：**服务端**拒绝 chunked 是正确的防走私设计，**请保留**；但**客户端**也拒绝 ⇒ 调不了大量现代 API（大量站点默认 chunked）。建议：**客户端**支持解码 chunked；**服务端**继续拒绝（并明确说明这是有意的）。
- **验收判据**：`HTTP_GET` 能取回 chunked 响应正文。

### C-12 · 条件请求与部分响应 `P2`

- **期望**：`HEAD`（头同 GET、体为空）、`304 Not Modified`（配 `ETag` / `Last-Modified`）、`Range`（206）。
- **TinyPage 场景**：静态资源缓存（Python 版目前每次全量传，无 `Last-Modified`/`ETag`/`304`，是已知 P3 缺陷）。

---

## D 组 · 中间件与 WAF 需要的原语 `P0`/`P1`

### D-1 · 正则模式的编译缓存 `P0`

- **现状（规范明示）**：「参考实现对 `RE_*` 的每次调用都重新编译模式（不信任调用方传回的 `DICT`）」⇒ **WAF 每处理一个请求就要重编译全部规则**。
- **期望**：模式对象可被安全缓存复用（或提供 `RE_COMPILE` + 显式缓存容器），使"编译一次、多次匹配"成为常规用法。
- **验收判据**：同一模式对象连续 `RE_SEARCH` 1000 次，第二次起不再重新编译（可用耗时或实现注证明）。

### D-2 · 匹配前的归一化能力 `P0`

- **现状**：`(?i)` 是 **ASCII-only**（规范明示）⇒ Unicode 混淆（如 `ſ` vs `s`、`K` vs `K`）绕过规则。
- **期望**：`std.text` 提供可用于 WAF 的归一化：大小写折叠（Unicode 感知）、NFKC（**注意当前只有 NFC/NFD，缺 NFKC** —— NFKC 正是兼容性字符攻击的克星）、零宽/双向控制字符剥离。
- **验收判据**：一条针对 `SELECT` 的规则能命中 `ｓｅｌｅｃｔ`（全角）等变体（前提是先做 NFKC 归一）。

### D-3 · 限流与计数的原语 `P1`

- **期望**：滑动窗口 / 令牌桶原语（可直接用 B-4 的共享容器实现，但**希望由标准库提供经过验证的实现**，避免每个项目各写一遍并各写错一遍）。
- **TinyPage 场景**：滑动窗口限流（60 次/60 秒）、登录失败渐进锁定（5 次失败锁 300 秒）、审计计数。

### D-4 · 安全的路径包含判断 `P1`

- **现状**：我们自己实现时踩了两个坑（分隔符不一致、前缀匹配会误接受同前缀的兄弟目录，如 `pages-evil` 对 `pages`）。
- **期望**：`PATH_CONTAINS(root, target) -> BOOLEAN`，**跨平台正确**、内部做真实路径解析（不是字符串前缀）、处理 `..`、符号链接、Windows 大小写与分隔符。
- **验收判据**：`PATH_CONTAINS("D:/root", "D:/root-evil/x")` → `FALSE`；`PATH_CONTAINS("D:/root", "D:/root/../root2/x")` → `FALSE`。

### D-5 · 规则集的加载与热更新 `P1`

- **期望**：从文件读取 WAF 规则集、编译成规则数组、检测文件变更后重载（原子切换，不影响在途请求）。
- **TinyPage 场景**：规则需要能由站点管理员调整，而不必改代码重编译。

### D-6 · 结构化审计日志原语 `P1`

- **期望**：能输出**机器可读**的审计记录（JSON 行），字段：时间、事件类型、客户端 IP、请求路径、结果、请求 ID。
- **TinyPage 场景**：现有 `security_audit.log` 混装了全部 INFO 级日志（构建、文章写入等），不是结构化审计 —— 迁移时应一次做对。

### D-7 · 客户端 IP 与可信代理判定 `P2`

- **现状**：`TCP_PEER` ✔ 已有（对端地址）。但 `X-Forwarded-For` 的可信代理判定需自己实现。
- **期望**：可选的辅助原语（可信代理列表 → 真实 IP）。

---

## E 组 · 持久化与内容处理 `P1`

### E-1 · 原子写 `P1`

- **期望**：`WRITE_FILE_ATOMIC(path, content)`（临时文件 + rename + 必要的 fsync），或明确说明 `WRITE_FILE` + `MOVE` 的组合是否原子。
- **TinyPage 场景**：文章写盘、配置写盘。现有 Python 版用 `.tmp` 中转 + `.backup`（已知 P3 缺陷：中途崩溃会残留 `.backup`）。

### E-2 · 文件锁 / 并发写协调 `P1`

- **期望**：文件锁原语（`LOCK_FILE` / `UNLOCK_FILE`），或明确说明"同进程多任务写同一文件"的官方推荐做法。
- **TinyPage 场景**：管理端写文章的同时，构建任务在跑。

### E-3 · 目录与元数据 `P1`

- **期望**：目录项的修改时间 / 大小 / 类型（`LIST_DIR` 当前只给名字？）、`MTime` 之类的成员。
- **TinyPage 场景**：增量构建靠 mtime 判定"是否需要重建"。

### E-4 · 文本处理补齐（解析 HTTP 头与 query 的前提）`P0`

- **现状**：`std.str` 只有 `JOIN` / `SPLIT_LINES` / `CHAR_AT` / `COUNT` / `QUOTE` / `INDEX_OF` / `CONTAINS_SUB` / `CAT` —— **没有通用 `SPLIT`**。拆 HTTP 头、query string 只能用 `RE_SPLIT`（正则做 split，很慢且易错）。
- **期望**：
  - `std.str::SPLIT(s, sep)`；
  - `std.net::QUERY_PARSE(query_string)`（或 `std.str`）→ 字典，支持重复键；
  - `std.text::TO_STRING`（当前 `+` 不做数字→字符串强转，`+( "Content-Length: ", 8579)` → `E0030: cannot add string and integer`，必须绕道 `FORMAT`）。

### E-5 · 模板 / 格式化增强 `P2`

- **现状**：`std.format::FORMAT` 只有 `{0}` 位置占位 + 第一个 DICT 参数的 `{name}`。
- **期望**：具名参数、宽度/对齐/填充、条件与循环（否则模板只能手写字符串拼接）。
- **TinyPage 场景**：管理后台 7 个 HTML 模板（`admin_templates/`），Python 侧用 `string.Template`。

### E-6 · Markdown 渲染 `P2（建议明确不做，或降级为 P2 长期项）`

- **现状**：wlwl 无任何 Markdown 能力；Python 侧用 mistune（可选依赖）。
- **建议**：**不建议 wlwl 提供 Markdown**（属于应用层而非语言层），但若提供，至少需要：转义安全（防存储型 XSS，Python 侧正是因此修过一个 P0）、标题 id 生成、目录、脚注、`[[wikilink]]`。
- **我们的方案**：TinyPage 侧自研一个子集渲染器（与 HTTP 无关，可独立推进、可单测）。**请 wlwl 团队明确答复"是否提供"，以便我们确定归属。**

### E-7 · 代码高亮 `P2（建议明确不做）`

- **现状**：wlwl 无；Python 侧用 Pygments。
- **建议**：与 E-6 同理，属于应用层。建议 TinyPage 侧自研最小子集或砍掉该特性。**请明确答复归属。**

---

## F 组 · 可观测性与运维 `P1`

### F-1 · 结构化日志 `P1`

- **现状**：`std.io` 只有 `PRINT` / `PRINT_ERR` / `INPUT`。
- **期望**：`std.log`：级别（TRACE/DEBUG/INFO/WARN/ERROR）、级别过滤、输出到文件、结构化字段（JSON 行）、可从环境变量配置级别。
- **TinyPage 场景**：审计日志 + 运行日志分离（现在混在一个文件里）。

### F-2 · 运行时指标 `P2`

- **期望**：请求计数、状态码分布、延迟直方图、当前连接数/任务数；可被管理后台读取。
- **TinyPage 场景**：管理后台面板展示（Python 侧目前没有，但这是"中间件"定位的自然期望）。

### F-3 · 任务状态检视（排障神器）`P1`

- **期望**：`wlwl tasks`（或 `wlwl inspect`）子命令，列出存活任务、各自挂在哪个挂起点、已运行时长；配合 `wlwl` 的 `interface` / `namespace` / `schema` 风格输出 JSON。
- **理由**：本次评估中，"任务为什么静默死了"这个问题**消耗了大量排查时间**，因为运行时是黑盒。若能在运行期看到"任务挂在哪个挂起点"，这类问题 10 分钟内可定位。

### F-4 · 错误信息可读性 `P1`

- **现状**：部分诊断只有一行（如 `E0011: expected ')', got Semicolon`），行号需要看 stderr 的位置块。迁移期我们会大量与解析错误打交道。
- **期望**：诊断信息带上**源码行片段 + 列号高亮**（`wlwl check` / `wlwl run` 都给），并在可能时给出"你是不是想写……"的提示（例如 `IF` 第三实参放多语句组）。

---

## G 组 · 开发体验与测试 `P1`

### G-1 · `std.test` 增强 `P1`

- **现状**：`std.test` 只有 `TEST` / `ASSERT` / `ASSERT_EQ` / `ASSERT_NEQ` / `EXPECT_ERR` / `RUN_TESTS` 六个成员；且 `EXPECT_ERR` 只有一个参数、不捕获原生诊断码（`E1003` 除零仍会逃逸）、`ASSERT(0)` / `ASSERT("")` / `ASSERT([])` 会被判失败。
- **期望**：断言可带自定义消息、捕获原生诊断码、跳过（skip）、参数化、fixture/临时目录、异步测试（等任务）、测试结果输出格式（便于 CI 解析）。
- **TinyPage 场景**：现有 125 个 pytest 用例要迁移（`tests/` 5 个文件 691 行），加上必须新增的 e2e 冒烟测试（"建文章 → 首页可见 → 删文章"）。**测试能力不足会让迁移期的回归全靠手工。**

### G-2 · 静态检查新增规则 `P1`

- **期望**：`wlwl check` 能静态发现本次实测到的两类错误：
  1. 宿主挂起点成员出现在**任务体之外**（含嵌套闭包内）；
  2. 跨任务共享可变绑定（§17.0.1 违规）。
- **理由**：这两类都是**静默失败**，静态检查是成本最低的拦截手段。

### G-3 · 调试器 / REPL `P2`

- **期望**：单步、断点、查看任务与变量；或至少一个能跑表达式并查看当前任务挂起点的 REPL。
- **TinyPage 场景**：调试并发与段重放行为时非常需要。

### G-4 · 模块组织 `P1`

- **期望**：多文件模块的 `IMPORT`（相对路径已有）、**模块级常量与初始化代码**、循环导入的清晰诊断、模块私有成员（不导出）机制、`.wll.sig` 契约的工作流简化。
- **TinyPage 场景**：迁移后代码量约 4000+ 行 wlwl，必须有清晰的模块划分（http / router / security / waf / render / admin）。

---

## H 组 · 易用性小修（零成本、高收益）`P1`

> 这几条都是我们在 4 小时评估里**各自踩了至少一次**的坑。单条实现成本很低，但反复踩的代价不小。

| ID | 需求 | 现状（实测） | 期望 |
|---|---|---|---|
| **H-1** | `IF` 的**第三**实参支持多语句组 | `IF(c, x, (LET(a,1); LET(b,2); a))` → `E0013: expected ';' after expression`；**同样的形状放第二实参却正常** ⇒ 不一致且反直觉 | 三个实参一视同仁；或至少在诊断里明确说明"第三实参不支持多语句组" |
| **H-2** | 数字 → 字符串内建 | `+( "Content-Length: ", 8579)` → `E0030: cannot add string and integer` | 加 `std.text::TO_STRING`（或让 `+` 在字符串语境下强转） |
| **H-3** | 通用 `SPLIT` 与 query 解析 | 只有 `SPLIT_LINES`；拆 HTTP 头/query 只能靠 `RE_SPLIT` | 见 E-4 |
| **H-4** | 路径分隔符统一 | `PATH_NORMALIZE("C:\\a\\b")` 保留 `\`，`PATH_JOIN` 产出 `/` ⇒ 跨平台行为不一致，是前缀比对失败的直接原因 | 统一为 `/`（或提供 `PATH_CANONICAL`） |
| **H-5** | `RE_TEST` 命名 | `RE_TEST` 是**整串锚定**语义（两端锚定），名字容易被当成"包含匹配" ⇒ WAF 规则用它会**永远不命中**（静默漏防） | 改名/加别名（如 `RE_MATCHES_FULL`），并在文档首行标注"整串锚定" |
| **H-6** | 容器取值 API 命名统一 | `AT_K` 只接受 DICT，取数组元素要用 `INDEX_GET`；`INDEX` 是 1-based 而 `INDEX_GET` 是 0-based（规范自认"最容易搞错的那个"） | 统一命名或提供 `ARRAY_GET` 别名，并在错误信息里提示正确 API |

---

## I 组 · 本次迁移不需要，但请给出排期口径 `P2`

> 这几条我们**不阻塞**，但作为"中间件"定位的长期期望，希望知道方向（避免将来再评估一轮）。

| ID | 需求 | 说明 |
|---|---|---|
| **I-1** | **HTTP/2** | 企业环境常见期望；至少给一个"是否在规划内"的答复 |
| **I-2** | **WebSocket** | 管理端实时预览（Python 版该功能已因 P0-1 不可用） |
| **I-3** | **`TCP_CONNECT`** | 传输族能 listen/serve 但**不能主动连出**（出站只有阻塞式 `HTTP_*`）。若将来要连数据库/Redis 等，需要 |
| **I-4** | **brotli 压缩** | `std.compress` 目前有 zstd / gzip / zlib / raw deflate，无 brotli |
| **I-5** | **HTTP/3** | 仅作方向性询问 |

---

## J 组 · 我们承诺配合的事项

为降低团队成本，我们可以承担：

1. **回归用例迁移**：把现有 125 个 pytest 用例逐条转写成 wlwl 侧 `std.test` 用例，作为上游 `std.test` 增强的验收材料。
2. **性能基线**：提供 Python 版 waitress 的 QPS / 延迟基线，作为 wlwl HTTP 服务的对比目标。
3. **探针共享**：本清单附带的 9 个探针（`p01`–`p13`）已开源在目标仓库 `docs/wlwl-feasibility/probes/`，可直接作为新成员的上手材料与回归基线。
4. **上游 issue 复现**：任何 P0 项的复现步骤，我们会写成可一键重跑的脚本。

---

## 附录 A · 实测证据索引

所有「现状」结论的出处（探针源码见 `docs/wlwl-feasibility/probes/`）：

| 探针 | 证明的事 |
|---|---|
| `p01-capability-matrix.wll` | TLS 缺失（`https` → ERR）、密码学原语可用、`HTML_SANITIZE*` 可用、`RE_TEST` 整串锚定、`PATH_NORMALIZE` 行为、`READ_FILE` 保留 BOM、`LEN` vs `BYTE_LEN` |
| `p03-accept-recv-inline.wll` | 段首重放（挂起点之前的 `PRINT` 跑了两次） |
| `p09-trace-nested-wait-death.wll` | **嵌套闭包里的 `TCP_RECV` ⇒ 任务静默终止** |
| `p10-concurrent-server-inline-recv.wll` | 纯 wlwl HTTP 服务：8 并发全 200、安全头齐全、`Content-Length` 字节数正确 |
| `p11-single-handler-waf-checks.wll` | 路径穿越 → 403；`<script>` / `union select` → 403；**必须先 URL 解码再匹配**（未解码时全部漏过） |
| `p12-acceptor-pool-wakeall.wll` | **3 acceptor + 1 连接 ⇒ 落选者拿 `NULL` 并死掉** |
| `p13-single-acceptor-chain.wll` | 单 acceptor 展开链连续服务 4 个请求全 200（可用架构样板） |
| `p02` / `p07` | 反例：把 handler 放进嵌套闭包的两种失败写法 |

## 附录 B · 复现命令

```powershell
$exe = 'D:\Project\wlwl_runtime\wlwl-v0.12.0\wlwl.exe'
& $exe run docs\wlwl-feasibility\probes\p01-capability-matrix.wll   # TLS 缺失 / 密码学 / regex 语义
& $exe run docs\wlwl-feasibility\probes\p09-trace-nested-wait-death.wll  # 静默死亡
& $exe run docs\wlwl-feasibility\probes\p12-acceptor-pool-wakeall.wll   # acceptor 池语义
& $exe run docs\wlwl-feasibility\probes\p13-single-acceptor-chain.wll   # 可用架构
```

## 附录 C · 一页速查

| 立刻要做 | 尽快做 | 顺手做 |
|---|---|---|
| A-1 TLS 服务端<br>A-2 HTTPS 出站<br>B-1 挂起点位置诊断<br>B-2 acceptor 语义<br>B-4 共享状态<br>B-6 终值可区分<br>C-1 请求解析<br>C-2 响应构造<br>D-1 编译缓存<br>E-4 文本补齐 | B-3 可推进循环<br>B-5 非阻塞出站<br>C-5 Cookie<br>C-6 表单/multipart<br>C-7 静态文件服务<br>C-11 chunked 客户端<br>D-2 归一化<br>D-3 限流<br>D-4 路径包含判断<br>D-6 审计日志<br>E-1 原子写<br>E-3 mtime<br>F-1 结构化日志<br>F-3 任务检视<br>F-4 错误可读性<br>G-1 std.test<br>G-2 静态检查<br>G-4 模块组织<br>H-1…H-6 易用性小修 | C-3 Keep-Alive<br>C-8 请求预算<br>C-9 大小限制<br>C-10 压缩集成<br>C-12 条件请求<br>D-5 规则热加载<br>D-7 真实 IP<br>E-2 文件锁<br>E-5 模板<br>E-6 Markdown（建议不做）<br>E-7 高亮（建议不做）<br>F-2 指标<br>G-3 调试器<br>I-1…I-5 排期口径 |
