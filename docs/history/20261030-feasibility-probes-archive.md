# 归档：迁移可行性探针（9 个 `.wll` + README 对照表）

> **本文件是归档件。** 2026-10-30 起，`docs/wlwl-feasibility/probes/`
> 这个目录已从本仓移除 —— 本文件按「原文照录，不精简合并」的体例，把
> 那个目录的**全部内容**（9 个探针 + `README.md` 对照表）并成**一份**。
>
> **为什么归档**：这 9 个 `.wll` 与门禁夹具**逐字节相同**
> （`impl/tests/probe/cases/P121_probe_*/main.wll`，SHA256 实测 9/9 一致），
> 门禁只读 `impl/tests/probe/cases/`（`probe.rs` 的 `cases_root()` 从
> `CARGO_MANIFEST_DIR` 解析）⇒ **`docs/` 下这份副本从不被任何测试或 CI 读取**，
> 留着只是两份会漂移的同源文件（**已漂移过**：见下「⚠ 归档时发现的漂移」）。
>
> **可重跑的证据仍在仓内**：`impl/tests/probe/cases/P121_probe_*/main.wll`
> （9 份，逐字相同）+ 同目录的 `expect.json`（断言）。
> 门禁：`cargo test --locked -p wlwl-cli --test probe`（`EXPECTED_CASE_COUNT = 196`）。
>
> **要追溯改动**：`git log --follow docs/wlwl-feasibility/probes/README.md`
> （归档前用该旧路径）。

---

## ⚠ 归档时发现的漂移（3 处 —— 原文与探针实测不符）

`README.md` 的对照表是**第一波（W-02 固化时）**写的；此后**第二波
（`86b3c4a`）** 与 **第四波（`969b220`）** 改了探针本体与 `expect.json`，
**README 没跟着改**。归档前实测（`impl/target/release/wlwl.exe run <探针>`）
的真相：

| # | 原文写的 | 实测（归档时） | 出处 |
|---|---|---|---|
| 1 | **p07**「handler 闭包里再定义并调用内层闭包 ⇒ **跑通了**：终值 **42**、exit 0」 | ❌ **exit 1** —— `error[E0014]: TCP_RECV: used outside a task body`（挂起点落在**内层**闭包 ⇒ 内层闭包不在任务体里 ⇒ 宿主挂起点成员在任务体外被调用 ⇒ `E0014`）。终值 **42 从未出现** | 第二波 `86b3c4a`（`W-38` 把「宿主挂起点在任务体外」诊断化） |
| 2 | **p09**「任务正常跑完、`AWAIT` 拿到 **42**、exit 0、stderr 空；可观察的是**段首重放 ⇒ 副作用重复**」 | ❌ **exit 1** —— 同一条 `E0014`（挂起点写进辅助闭包、由任务体调用 ⇒ 辅助闭包不在任务体里）。**stderr 不空**，终值 **42 从未出现** | 第二波 `86b3c4a` |
| 3 | **p12**「3 个 acceptor 抢 1 个连接 ⇒ **惊群坐实**：只有 **1** 个拿到连接、其余 **2** 个拿到 `NULL`；落选者是**被连接到达唤醒**的（≈155 ms）」 | ❌ **惊群已修** —— 实测 `winners= 2`、`losers= 1`、`losers-woke-immediately= FALSE`、`loser-elapsed= 2042`。**两个** acceptor 各拿到一条连接，落选者**等满自己的 2000 ms 超时**（不是被唤醒）。`expect.json` 的 `_why` 明写「本用例在 W-11 修掉惊群**之后已翻转** —— 它现在钉的是「**惊群已修**」」 | 第四波 `969b220`（`W-11` 唤醒一个） |

⚠ **第三条是「原文与实测相反」**（不是措辞过时）⇒ 归档件里的 README 原文
**照录不改**（体例如此），但**上面这张表是当前真相**，以
`impl/tests/probe/cases/P121_probe_*/expect.json` 为准。
⇒ **原文的 Δ1 / Δ2 两行同样作废**（Δ1 的 p09「exit 0 / 终值 42 / stderr 空」
与 Δ2 的 p12「1 胜 2 负 / 落选者被连接到达唤醒」都已被上表证伪）。

---

## 探针本体（9 份 · 原文照录）

⚠ 每份都**逐字等于** `impl/tests/probe/cases/P121_probe_*/main.wll`
（SHA256 实测）⇒ **要跑请跑门禁那份**（断言在同目录 `expect.json`）。

### `p01-capability-matrix.wll`

```wlwl
// ─────────────────────────────────────────────────────────────────────────────
// p01-capability-matrix.wll —— 需求清单附录 A 的「能力矩阵」探针
//
// 它证明什么（对应需求清单 §A / §C / §D / §H 组）：
//   (1) TLS 缺失：`HTTP_GET("https://…")` 在**任何网络 I/O 之前**就被拒
//       （scheme 判定先于拨号 ⇒ 本探针**不联网**）。
//   (2) 密码学原语可用：`SHA256` / `HMAC_SHA256` 给的是标准十六进制摘要。
//   (3) `HTML_SANITIZE*` 可用：`<script>` 被整段删掉，`<b>` 保留。
//   (4) `RE_TEST` 是**整串锚定**语义（两端锚定）⇒ 用它当「包含匹配」会**永远漏**。
//   (5) `PATH_NORMALIZE` 只认 `/`、`PATH_JOIN` 把 `\` 也当分隔符 ⇒ 二者不互逆。
//   (6) `READ_FILE` **保留** BOM（不吞）。
//   (7) `LEN`（码点）vs `BYTE_LEN`（字节）—— 多字节文本上两者不等。
//
// 期望读数（exit 0，stdout 见下）：
//   [p01] https-is-err=TRUE
//   [p01] https-reason=... scheme "https" is not supported — plaintext http: only (ADR-0028 §0)
//   [p01] sha256-len=64
//   [p01] sha256-abc=ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad
//   [p01] hmac-sha256-len=64
//   [p01] sanitize=<b>ok</b>
//   [p01] re-test-exact=TRUE
//   [p01] re-test-partial=FALSE
//   [p01] re-search-offset=1
//   [p01] path-normalize=C:\a\b
//   [p01] path-join=C:/a/b
//   [p01] read-file-keeps-bom=TRUE
//   [p01] bom-len=6
//   [p01] bom-byte-len=8
//   [p01] cjk-len=3
//   [p01] cjk-byte-len=9
//
// 与清单描述的差异：**无**（附录 A 对 p01 的七项描述逐条对上）。
//
// ⚠ 夹具副作用：本探针会**在当前工作目录**写一个临时文件再删掉
//   （`wlwl-p01-bom-probe.tmp`）—— 因为「`READ_FILE` 保留 BOM」必须真的读一个
//   带 BOM 的文件。probe 夹具跑在暂存副本里（不污染仓库）；手动跑本文件时
//   临时文件建在**你的当前目录**并在探针结束前删掉。
// ─────────────────────────────────────────────────────────────────────────────

// ⚠ 本文件与 `impl/tests/probe/cases/P121_probe_p01_capability_matrix/main.wll`
//   **逐字相同**（同一份程序的两处副本）⇒ 改动请同步两处。
// ─────────────────────────────────────────────────────────────────────────────

IMPORT("wlwl:std.io", ["PRINT"]);
IMPORT("wlwl:std.net", ["HTTP_GET"]);
IMPORT("wlwl:std.encode", ["SHA256", "HMAC_SHA256", "BYTE_LEN"]);
IMPORT("wlwl:std.sanitize", ["HTML_SANITIZE"]);
IMPORT("wlwl:std.regex", ["RE", "RE_TEST", "RE_SEARCH"]);
IMPORT("wlwl:std.fs", [
  "READ_FILE", "WRITE_FILE", "REMOVE", "PATH_NORMALIZE", "PATH_JOIN"
]);

// ── (1) TLS 缺失 ─────────────────────────────────────────────────────────────
// ⚠ 用 `https://127.0.0.1:9/` 而不是 `https://example.com/`：**scheme 判定在
//   拨号之前**（`impl/crates/wlwl-std/src/net.rs:830-841`）⇒ 根本不会碰网络；
//   但把 host 写成本机回环，万一将来判定顺序变了，代价也只是一次本地拒绝。
LET(https_err, HTTP_GET("https://127.0.0.1:9/"));
PRINT("[p01] https-is-err=", IS_ERR(https_err));
PRINT("[p01] https-reason=", ERR_PAYLOAD(https_err));

// ── (2) 密码学原语可用 ──────────────────────────────────────────────────────
// 期望值取自 **FIPS 180-4 / RFC 4231 的公开测试向量**，不是从本实现输出抄。
LET(sha_abc, SHA256("abc"));
PRINT("[p01] sha256-len=", LEN(sha_abc));
PRINT("[p01] sha256-abc=", sha_abc);
LET(hmac, HMAC_SHA256("key", "The quick brown fox jumps over the lazy dog"));
PRINT("[p01] hmac-sha256-len=", LEN(hmac));
PRINT("[p01] hmac-sha256=", hmac);

// ── (3) HTML_SANITIZE 可用 ─────────────────────────────────────────────────
PRINT("[p01] sanitize=", HTML_SANITIZE("<b>ok</b><script>alert(1)</script>"));

// ── (4) RE_TEST 是整串锚定 ──────────────────────────────────────────────────
LET(pat_exact, RE("abc"));
LET(pat_partial, RE("b"));
PRINT("[p01] re-test-exact=", RE_TEST(pat_exact, "abc"));
PRINT("[p01] re-test-partial=", RE_TEST(pat_partial, "abc"));
PRINT("[p01] re-search-offset=", AT_K(RE_SEARCH(pat_partial, "abc"), "start", -1));

// ── (5) PATH_NORMALIZE / PATH_JOIN 的分隔符口径不一致 ────────────────────────
PRINT("[p01] path-normalize=", PATH_NORMALIZE("C:\\a\\b"));
PRINT("[p01] path-join=", PATH_JOIN("C:\\a", "b"));

// ── (6) READ_FILE 保留 BOM ─────────────────────────────────────────────────
LET(bom, FROM_CODEPOINTS([65279]));
LET(bom_path, "wlwl-p01-bom-probe.tmp");
WRITE_FILE(bom_path, +(bom, "hello"));
LET(bom_content, READ_FILE(bom_path));
REMOVE(bom_path);
PRINT("[p01] read-file-keeps-bom=", ==(INDEX_GET(CODEPOINTS(bom_content), 0), 65279));
PRINT("[p01] bom-len=", LEN(bom_content));
PRINT("[p01] bom-byte-len=", BYTE_LEN(bom_content));

// ── (7) LEN vs BYTE_LEN ─────────────────────────────────────────────────────
LET(cjk, "中文字");
PRINT("[p01] cjk-len=", LEN(cjk));
PRINT("[p01] cjk-byte-len=", BYTE_LEN(cjk));

PRINT("[p01] DONE");

```

---

### `p02-handler-in-top-level-closure.wll`

```wlwl
// ─────────────────────────────────────────────────────────────────────────────
// p02-handler-in-top-level-closure.wll —— 「把 handler 放进闭包」反例 · 写法 ①
//
// 它证明什么（对应需求清单附录 A 的「`p02` / `p07` 反例：把 handler 放进嵌套
//   闭包的两种失败写法」之**第一种**）：
//   写法 ① = **handler 整体是一个模块顶层的闭包**，任务体只负责调用它。
//   挂起点（`TCP_ACCEPT` + `TCP_RECV`）因此落在**闭包体内**、被**任务体**间接调用。
//
// ⚠⚠ **与清单描述有差异（本探针按实测断言）**：
//   清单把这种写法列为「失败写法」，并说结果是「任务静默终止、`AWAIT` 拿到 NULL」。
//   **实测复现不出来**：当前实现里它**跑通了** —— 任务正常结束，终值是 **42**
//   （不是 NULL），**exit 0**，stderr 空。可观察的副作用是**段首重放**（挂起点
//   之前的 PRINT 跑了两遍）。
//   ⇒ 「写法 ① 会静默死掉」这个描述与实测**不符**；本探针把**实测**钉死。
//   （与计划 §2.3 的 **R-24** 一致。）
//
// 期望读数（exit 0）：
//   [p02] handler-head
//   [p02] client= CLIENT-NO-RESPONSE
//   [p02] handler-head          （第 2 次 —— 段首重放）
//   [p02] after-accept-is-null= FALSE
//   [p02] after-recv-is-null= FALSE
//   [p02] z = 42
//
// ⚠ 客户端看不到响应：`HTTP_*` 是阻塞原语（ADR-0028 D1）⇒ 断言落在服务端一侧。
// ─────────────────────────────────────────────────────────────────────────────

// ⚠ 本文件与 `impl/tests/probe/cases/P121_probe_p02_top_level_closure/main.wll`
//   **逐字相同**（同一份程序的两处副本）⇒ 改动请同步两处。
// ─────────────────────────────────────────────────────────────────────────────

IMPORT("wlwl:std.io", ["PRINT"]);
IMPORT("wlwl:std.str", ["CAT"]);
IMPORT("wlwl:std.result", ["TAKE_ERR"]);
IMPORT("wlwl:std.net", [
  "TCP_LISTEN", "TCP_ACCEPT", "TCP_RECV", "TCP_SEND", "TCP_CLOSE",
  "TCP_LOCAL_ADDR", "HTTP_GET"
]);

LET(http_client, FUN((port, path), (
  LET(r, HTTP_GET(CAT("http://127.0.0.1:", STR(port), path), 150));
  IF(==(TYPE(TAKE_ERR(r)), "NULL"), "CLIENT-GOT-RESPONSE", "CLIENT-NO-RESPONSE")
)));

LET(l, TCP_LISTEN("127.0.0.1", 0));
LET(port, AT_K(TCP_LOCAL_ADDR(l), "port", 0));

// ⚠ **写法 ①**：handler 是**模块顶层**的闭包；挂起点在它体内。
LET(handler, FUN(() , (
  PRINT("[p02] handler-head");
  LET(conn, TCP_ACCEPT(l, 3000));
  PRINT("[p02] after-accept-is-null=", ==(conn, NULL));
  LET(chunk, TCP_RECV(conn, 4096, 2000));
  PRINT("[p02] after-recv-is-null=", ==(chunk, NULL));
  TAKE_ERR(TCP_SEND(conn, "HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK"));
  TCP_CLOSE(conn);
  42
)));

SCOPE(FUN(() ,
  LET(t, SPAWN(handler));
  LET(tc, SPAWN(FUN(() , http_client(port, "/p02"))));
  PRINT("[p02] client=", AWAIT(tc));
  PRINT("[p02] z = ", AWAIT(t))
));

PRINT("[p02] DONE");

```

---

### `p03-accept-recv-inline.wll`

```wlwl
// ─────────────────────────────────────────────────────────────────────────────
// p03-accept-recv-inline.wll —— 「段首重放」探针
//
// 它证明什么（对应需求清单 §B-1 的前半段 / 并发册 §17.0 第 2 条）：
//   宿主挂起点（`TCP_ACCEPT` / `TCP_RECV`）是**不切段**的挂起点 ⇒ 恢复时
//   **段首重放** ⇒ **挂起点之前**的语句会**再跑一遍** ⇒ 副作用重复。
//
// 期望读数（exit 0）：
//   [p03] segment-head-run= 1
//   [p03] segment-head-run= 2          ← 段首重放：挂起点之前的 PRINT 跑了两次
//   [p03] request-line= GET /p03 HTTP/1.1
//   [p03] client= CLIENT-NO-RESPONSE
//   [p03] task= served
//   [p03] head-runs= 2
//
// 与清单描述的差异：**无**（附录 A 对 p03 的描述「段首重放（挂起点之前的 PRINT
//   跑了两次）」逐字对上）。
//
// ⚠ 为什么客户端看不到响应：`HTTP_*` 是**阻塞原语**，执行期间**整个 SCOPE 冻结**
//   （ADR-0028 D1 / 系统册 §21.4）⇒ 客户端在等响应时，服务端任务**跑不了**。
//   ⇒ 本探针的断言全部落在**服务端一侧**（请求确实到达了、handler 确实跑了）。
//   客户端那一侧只能断言「没拿到响应」—— 这**本身就是** ADR-0028 D1 的证据。
// ─────────────────────────────────────────────────────────────────────────────

// ⚠ 本文件与 `impl/tests/probe/cases/P121_probe_p03_accept_recv_inline/main.wll`
//   **逐字相同**（同一份程序的两处副本）⇒ 改动请同步两处。
// ─────────────────────────────────────────────────────────────────────────────

IMPORT("wlwl:std.io", ["PRINT"]);
IMPORT("wlwl:std.str", ["CAT"]);
IMPORT("wlwl:std.result", ["TAKE_ERR"]);
IMPORT("wlwl:std.net", [
  "TCP_LISTEN", "TCP_ACCEPT", "TCP_RECV", "TCP_SEND", "TCP_CLOSE",
  "TCP_LOCAL_ADDR", "HTTP_GET"
]);

// 客户端：连上、发请求、然后**必然**超时（服务端在客户端阻塞期间跑不了）。
LET(http_client, FUN((port, path), (
  LET(r, HTTP_GET(CAT("http://127.0.0.1:", STR(port), path), 150));
  IF(==(TYPE(TAKE_ERR(r)), "NULL"), "CLIENT-GOT-RESPONSE", "CLIENT-NO-RESPONSE")
)));

// ⚠ 计数器必须住在**重放段之外**（并发册 §17.0 第 3 条：段内的局部状态活不过
//   一次挂起）⇒ 它建在**模块顶层**，任务体只 `SET` 它。
LET MUT(head_runs, 0);

LET(l, TCP_LISTEN("127.0.0.1", 0));
LET(port, AT_K(TCP_LOCAL_ADDR(l), "port", 0));

// 服务端任务：**段首** = 挂起点之前的 PRINT + accept。
LET(server, FUN(() , (
  SET(head_runs, +(head_runs, 1));
  PRINT("[p03] segment-head-run=", head_runs);
  LET(conn, TCP_ACCEPT(l, 3000));
  YIELD();
  LET(chunk, TCP_RECV(conn, 4096, 2000));
  LET(lines, SPLIT(AT_K(chunk, "text", ""), "\r\n"));
  PRINT("[p03] request-line=", INDEX_GET(lines, 0));
  TAKE_ERR(TCP_SEND(conn, "HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK"));
  TCP_CLOSE(conn);
  "served"
)));

SCOPE(FUN(() ,
  // ⚠ 顺序要紧：**先** spawn 服务端任务（它会挂在 accept 上），**再** spawn 客户端
  //   任务 ⇒ 客户端连上时服务端**已经挂起** ⇒ 连接到达会把它唤醒 ⇒ 触发段首重放。
  LET(t, SPAWN(server));
  LET(tc, SPAWN(FUN(() , http_client(port, "/p03"))));
  PRINT("[p03] client=", AWAIT(tc));
  PRINT("[p03] task=", AWAIT(t));
  PRINT("[p03] head-runs=", head_runs)
));

PRINT("[p03] DONE");

```

---

### `p07-handler-in-nested-closure.wll`

```wlwl
// ─────────────────────────────────────────────────────────────────────────────
// p07-handler-in-nested-closure.wll —— 「把 handler 放进闭包」反例 · 写法 ②
//
// 它证明什么（对应需求清单附录 A 的「`p02` / `p07` 反例：把 handler 放进嵌套
//   闭包的两种失败写法」之**第二种**）：
//   写法 ② = handler 闭包内部**再定义并调用**一个内层闭包 ⇒ **两层嵌套**；
//   挂起点（`TCP_RECV`）落在**内层**闭包里。
//
// ⚠⚠ **[W-10 之后] 这个写法现在被**明确拒绝**（不再是「静默跑通」）：
//   并发册 §17.0「**宿主挂起点成员的调用点必须在任务体内**」⇒ 嵌套闭包里的
//   `TCP_RECV` **当场**报 `E0014`（**指名成员** + 位置指向**调用点**）。
//   ⇒ 任务失败 ⇒ `AWAIT` 把它重新抛到 `SCOPE` ⇒ 整个运行 **exit 1**。
//   ⇒ 这正是需求清单 §B-1 要的那条诊断（「要么跑通，要么明确报 E0014 指向此行」
//     ⇒ 现在走的是后一条）。
//
// 期望读数（**exit 1**）：
//   [p07] handler-head
//   [p07] client= CLIENT-NO-RESPONSE
//   [p07] handler-head          （第 2 次 —— 段首重放）
//   [p07] after-accept-is-null= FALSE
//   [p07] inner-entered
//   stderr: error[E0014]: TCP_RECV: used outside a task body — …
//           --> main.wll:<行>:<列>（那一行就是 `TCP_RECV` 本身）
//
// ⚠ 客户端看不到响应：`HTTP_*` 是阻塞原语（ADR-0028 D1）⇒ 断言落在服务端一侧。
// ─────────────────────────────────────────────────────────────────────────────

// ⚠ 本文件与 `impl/tests/probe/cases/P121_probe_p07_nested_closure/main.wll`
//   **逐字相同**（同一份程序的两处副本）⇒ 改动请同步两处。
// ─────────────────────────────────────────────────────────────────────────────

IMPORT("wlwl:std.io", ["PRINT"]);
IMPORT("wlwl:std.str", ["CAT"]);
IMPORT("wlwl:std.result", ["TAKE_ERR"]);
IMPORT("wlwl:std.net", [
  "TCP_LISTEN", "TCP_ACCEPT", "TCP_RECV", "TCP_SEND", "TCP_CLOSE",
  "TCP_LOCAL_ADDR", "HTTP_GET"
]);

LET(http_client, FUN((port, path), (
  LET(r, HTTP_GET(CAT("http://127.0.0.1:", STR(port), path), 150));
  IF(==(TYPE(TAKE_ERR(r)), "NULL"), "CLIENT-GOT-RESPONSE", "CLIENT-NO-RESPONSE")
)));

LET(l, TCP_LISTEN("127.0.0.1", 0));
LET(port, AT_K(TCP_LOCAL_ADDR(l), "port", 0));

// ⚠ **写法 ②**：handler 闭包里**再定义**一个内层闭包，挂起点在内层。
LET(handler, FUN(() , (
  PRINT("[p07] handler-head");
  LET(conn, TCP_ACCEPT(l, 3000));
  PRINT("[p07] after-accept-is-null=", ==(conn, NULL));
  LET(inner, FUN((c), (
    PRINT("[p07] inner-entered");
    TCP_RECV(c, 4096, 2000)
  )));
  LET(r, inner(conn));
  PRINT("[p07] after-inner-is-null=", ==(r, NULL));
  TAKE_ERR(TCP_SEND(conn, "HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK"));
  TCP_CLOSE(conn);
  42
)));

SCOPE(FUN(() ,
  LET(t, SPAWN(handler));
  LET(tc, SPAWN(FUN(() , http_client(port, "/p07"))));
  PRINT("[p07] client=", AWAIT(tc));
  PRINT("[p07] z = ", AWAIT(t))
));

PRINT("[p07] DONE");

```

---

### `p09-trace-nested-wait-death.wll`

```wlwl
// ─────────────────────────────────────────────────────────────────────────────
// p09-trace-nested-wait-death.wll —— 「嵌套闭包里的挂起点」探针
//
// 它证明什么（对应需求清单 §B-1）：
//   把宿主挂起点（`TCP_RECV`）写进**辅助闭包**、再由**任务体**调用它 ——
//   当前实现里到底发生了什么。
//
// ⚠⚠ **[W-10 之后] 这一族从「静默跑通」变成「明确报 `E0014`」**：
//   并发册 §17.0「**宿主挂起点成员的调用点必须在任务体内**」⇒ 辅助闭包里的
//   `TCP_RECV` **当场**报 `E0014`（**指名成员** + 位置指向**调用点**
//   `main.wll:<行>:<列>`）⇒ 任务失败 ⇒ `AWAIT` 把它重新抛到 `SCOPE`
//   ⇒ 整个运行 **exit 1**。
//   ⇒ 这正是需求清单 §B-1 要的那条诊断（「要么跑通，要么明确报 E0014 指向此行」
//     ⇒ 现在走的是后一条）。
//
// 期望读数（**exit 1**）：
//   [p09] before-accept              （第 1 次）
//   [p09] after-accept-is-null= FALSE
//   [p09] helper-entered
//   [p09] before-accept              （第 2 次 —— 段首重放 ⇒ 副作用重复）
//   [p09] after-accept-is-null= FALSE
//   [p09] helper-entered
//   [p09] client= CLIENT-NO-RESPONSE
//   stderr: error[E0014]: TCP_RECV: used outside a task body — …
//           --> main.wll:<行>:<列>（那一行就是 `TCP_RECV` 本身）
//   ⇒ 「z = 42」与「DONE」**不再出现**（任务失败 ⇒ 运行终止）。
//
// ⚠ 客户端看不到响应：`HTTP_*` 是阻塞原语，执行期间整个 SCOPE 冻结
//   （ADR-0028 D1）⇒ 客户端必然超时；断言落在服务端一侧。
// ─────────────────────────────────────────────────────────────────────────────

// ⚠ 本文件与 `impl/tests/probe/cases/P121_probe_p09_nested_wait_death/main.wll`
//   **逐字相同**（同一份程序的两处副本）⇒ 改动请同步两处。
// ─────────────────────────────────────────────────────────────────────────────

IMPORT("wlwl:std.io", ["PRINT"]);
IMPORT("wlwl:std.str", ["CAT"]);
IMPORT("wlwl:std.result", ["TAKE_ERR"]);
IMPORT("wlwl:std.net", [
  "TCP_LISTEN", "TCP_ACCEPT", "TCP_RECV", "TCP_SEND", "TCP_CLOSE",
  "TCP_LOCAL_ADDR", "HTTP_GET"
]);

LET(http_client, FUN((port, path), (
  LET(r, HTTP_GET(CAT("http://127.0.0.1:", STR(port), path), 150));
  IF(==(TYPE(TAKE_ERR(r)), "NULL"), "CLIENT-GOT-RESPONSE", "CLIENT-NO-RESPONSE")
)));

LET(l, TCP_LISTEN("127.0.0.1", 0));
LET(port, AT_K(TCP_LOCAL_ADDR(l), "port", 0));

// ⚠ **辅助闭包**：宿主挂起点 `TCP_RECV` 在**闭包体内**，由任务体调用。
LET(helper, FUN((c), (
  PRINT("[p09] helper-entered");
  TCP_RECV(c, 4096, 2000)
)));

LET(task_body, FUN(() , (
  PRINT("[p09] before-accept");
  LET(conn, TCP_ACCEPT(l, 3000));
  PRINT("[p09] after-accept-is-null=", ==(conn, NULL));
  LET(r, helper(conn));
  PRINT("[p09] after-helper-is-null=", ==(r, NULL));
  TAKE_ERR(TCP_SEND(conn, "HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK"));
  TCP_CLOSE(conn);
  42
)));

SCOPE(FUN(() ,
  LET(t, SPAWN(task_body));
  LET(tc, SPAWN(FUN(() , http_client(port, "/p09"))));
  PRINT("[p09] client=", AWAIT(tc));
  PRINT("[p09] z = ", AWAIT(t))
));

PRINT("[p09] DONE");

```

---

### `p10-concurrent-server-inline-recv.wll`

```wlwl
// ─────────────────────────────────────────────────────────────────────────────
// p10-concurrent-server-inline-recv.wll —— 「纯 wlwl HTTP 服务」探针
//
// 它证明什么（对应需求清单附录 A / §0.2）：
//   用**纯 wlwl**（不借助任何外部 HTTP 库）写一个 HTTP 服务端：
//     * **8 个并发**连接全部被服务，状态码全是 **200**；
//     * **5 个安全响应头**随响应发出；
//     * `Content-Length` 按**字节数**算（不是码点数）—— 本探针的正文含中文，
//       `LEN`（码点）与 `BYTE_LEN`（字节）**不相等**，错用 `LEN` 就会发短。
//
// ✅ **[W-35 / 业主 2026-10-28 裁决 D3] 本探针已迁移到 `TCP_SERVE` 形状**
//   —— 下面这一段是**给需求方的迁移指引**。
//
// ⚠ **为什么必须迁移**（W-10 落地的那条词法判据）：
//   并发册 §17.0「**宿主挂起点成员的调用点必须在任务体内**」⇒ `TCP_RECV` /
//   `TCP_ACCEPT` 只能出现在**任务体自己的顶层语句**里。旧形状把「收一条连接」
//   写成**模块顶层**的 `handle` 闭包、再由 acceptor 任务体调用它 ⇒ `TCP_RECV`
//   落在**嵌套闭包**里 ⇒ **当场**报 `E0014`（指名成员 + 位置指向调用点
//   `main.wll:<行>:<列>`）⇒ 运行 **exit 1** ⇒ 「8 并发全 200」的正面读数
//   **全部丢失**。
//
// ✅ **`TCP_SERVE` 为什么合规**（这是迁移的**关键**）：
//   `TCP_SERVE(listener, timeout_ms, opts, handler)` 把
//   「accept → 读满一次请求 → 调 handler → 写回响应 → 下一条连接」
//   整个循环放在**一个原生成员内部**推进。
//   ⇒ §17.0 那条判据约束的是**宿主挂起点成员**（`TCP_RECV` / `TCP_ACCEPT` /
//     `SLEEP_ASYNC` —— 它们要**挂起**当前任务 ⇒ 必须有任务体可挂）。
//   ⇒ `TCP_SERVE` **不挂起**、它**阻塞**调用线程 ⇒ **不落进**那条判据。
//   ⇒ 合规形状 = **一个 `TCP_SERVE` + 一个 handler 闭包**；handler **只**返回
//     响应 DICT（`{"status", "headers", "body"}`），**不碰任何挂起点成员**。
//   ⇒ handler 的实参是**请求 DICT**
//     （`{"method", "path", "version", "headers", "body"}`），`path` 是**请求目标
//     原样**（可能带查询串）—— 本探针不需要它，但 p11 需要。
//
// ⚠ **代价（需求方须知）**：`TCP_SERVE` **阻塞整个运行时**（它在原生帧内等
//   accept / 读写 ⇒ 别的任务跑不了）⇒ 与 `HTTP_*` / `SLEEP` 同类。
//   ⇒ **同进程内的客户端拿不到响应**（客户端必然超时）。
//   ⇒ 「8 并发全 200」仍然是**服务端一侧**的断言 —— 这与本探针**原有**口径
//     一致：8 个**真实的 TCP 连接**都被服务端读到了请求、都被回了一份 200 响应。
//   ⇒ 与清单描述的差异只有这一处（清单的「8 并发全 200」是**客户端视角**）。
//
// ⚠ **客户端为什么还能算「并发」**：客户端**先**连上（连接进内核的 accept
//   队列），服务端**后**起 ⇒ 每条连接都被服务到。
//   ⇒ 这也让 8 个连接**不会**触发「惊群」（需求清单 §B-2 的现状）：这里是
//     **一个** `TCP_SERVE` **顺序**取连接，**不存在**「唤醒全部等待者」。
//     p12 探针专门复现那个惊群。
//
// 期望读数（**exit 0**）：
//   ⚠ 下面 `resp-head` 那一条里的 `\r\n` 是**记号**（线上真的是 CRLF），
//     `PRINT` 在标签与值之间插**一个空格**（`resp-head= HTTP/1.1 …`）。
//   [p10] resp-head= HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\n
//          Content-Length: 13\r\nX-Content-Type-Options: nosniff\r\n
//          X-Frame-Options: DENY\r\nX-XSS-Protection: 1; mode=block\r\n
//          Referrer-Policy: strict-origin-when-cross-origin\r\n
//          Content-Security-Policy: default-src 'self'\r\nConnection: close\r\n\r\n
//   [p10] body-len= 9  body-byte-len= 13
//   [p10] served-200-count= 8       ← `TCP_SERVE` 的返回值（成功服务的连接数）
//   [p10] served-request-count= 8    ← handler 实际被调了几次（真实到达的请求数）
//   [p10] client-no-response-count= 8  ← 客户端全部超时（见上面的「代价」）
//   [p10] DONE
//
// ⚠ `resp-head` 那一条是**从真正发上线的那份 headers 字典逐字渲染出来的**
//   （`KEYS` 保插入序 ⇒ 渲染顺序 = 线上顺序）⇒ 它与线上字节**同源**，不会漂移。
// ─────────────────────────────────────────────────────────────────────────────

// ⚠ 本文件与 `impl/tests/probe/cases/P121_probe_p10_concurrent_server/main.wll`
//   **逐字相同**（同一份程序的两处副本）⇒ 改动请同步两处。
// ─────────────────────────────────────────────────────────────────────────────

IMPORT("wlwl:std.io", ["PRINT"]);
IMPORT("wlwl:std.str", ["CAT"]);
IMPORT("wlwl:std.encode", ["BYTE_LEN"]);
IMPORT("wlwl:std.result", ["TAKE_ERR"]);
IMPORT("wlwl:std.net", [
  "TCP_LISTEN", "TCP_SERVE", "TCP_LOCAL_ADDR", "TCP_CLOSE_LISTENER", "HTTP_GET"
]);

LET(http_client, FUN((port, path), (
  LET(r, HTTP_GET(CAT("http://127.0.0.1:", STR(port), path), 150));
  IF(==(TYPE(TAKE_ERR(r)), "NULL"), "CLIENT-GOT-RESPONSE", "CLIENT-NO-RESPONSE")
)));

LET(l, TCP_LISTEN("127.0.0.1", 0));
LET(port, AT_K(TCP_LOCAL_ADDR(l), "port", 0));

// 正文含中文 ⇒ LEN（码点）≠ BYTE_LEN（字节）。
LET(body_text, "你好, wlwl!");
// ⚠ `Content-Length` 必须按**字节数**算（错用 `LEN` 就会发短 4 个字节）。
LET(content_len, STR(BYTE_LEN(body_text)));

// 响应头：5 个安全响应头 + Content-Type + Content-Length + Connection。
// ⚠ **这份字典就是发上线的那些头**（`TCP_SERVE` 按插入序渲染它们）。
LET(resp_headers, [
  "Content-Type": "text/html; charset=utf-8",
  "Content-Length": content_len,
  "X-Content-Type-Options": "nosniff",
  "X-Frame-Options": "DENY",
  "X-XSS-Protection": "1; mode=block",
  "Referrer-Policy": "strict-origin-when-cross-origin",
  "Content-Security-Policy": "default-src 'self'",
  "Connection": "close"
]);

// ⚠ `resp-head` **从上面那份字典渲染** ⇒ 与线上字节同源（`KEYS` 保插入序）。
LET MUT(head_text, "HTTP/1.1 200 OK\r\n");
FOR(k, KEYS(resp_headers), (
  SET(head_text, CAT(head_text, k, ": ", AT_K(resp_headers, k, ""), "\r\n"));
  0
));
SET(head_text, CAT(head_text, "\r\n"));

PRINT("[p10] resp-head=", head_text);
PRINT("[p10] body-len=", LEN(body_text), " body-byte-len=", BYTE_LEN(body_text));

// handler 被调了几次（= 真实到达服务端的请求数）。
LET MUT(handler_calls, 0);

// ⚠ handler：**只**返回响应 DICT，**不碰任何挂起点成员**（`TCP_RECV` /
//   `TCP_ACCEPT` / `SLEEP_ASYNC`）—— handler 跑在**原生帧**里，那里挂不了。
LET(server, FUN(() , (
  TCP_SERVE(l, 800, NULL, FUN((req), (
    SET(handler_calls, +(handler_calls, 1));
    ["status": 200, "headers": resp_headers, "body": body_text]
  )))
)));

SCOPE(FUN(() ,
  // ⚠ 服务端任务**先**挂上；客户端**后**连（连接排队）⇒ 服务端照样逐条服务。
  LET(t, SPAWN(server));

  LET(c1, http_client(port, "/p10/1"));
  LET(c2, http_client(port, "/p10/2"));
  LET(c3, http_client(port, "/p10/3"));
  LET(c4, http_client(port, "/p10/4"));
  LET(c5, http_client(port, "/p10/5"));
  LET(c6, http_client(port, "/p10/6"));
  LET(c7, http_client(port, "/p10/7"));
  LET(c8, http_client(port, "/p10/8"));

  // ⚠ 主任务在这里挂起 ⇒ 服务端任务开始跑（`TCP_SERVE` 逐条服务那 8 条连接；
  //   8 条都服务完后它再等到 accept 超时（800ms）才收工并返回计数）。
  LET(served, AWAIT(t));
  PRINT("[p10] served-200-count=", served);
  PRINT("[p10] served-request-count=", handler_calls);

  // ⚠ 客户端必然看不到响应（`TCP_SERVE` 阻塞整个运行时）⇒ 8 个全是超时。
  LET MUT(no_resp, 0);
  FOR(c, [c1, c2, c3, c4, c5, c6, c7, c8], (
    IF(==(c, "CLIENT-NO-RESPONSE"), SET(no_resp, +(no_resp, 1)), 0)
  ));
  PRINT("[p10] client-no-response-count=", no_resp);

  TCP_CLOSE_LISTENER(l)
));

PRINT("[p10] DONE");

```

---

### `p11-single-handler-waf-checks.wll`

```wlwl
// ─────────────────────────────────────────────────────────────────────────────
// p11-single-handler-waf-checks.wll —— 「单 handler 的 WAF 检查」探针
//
// 它证明什么（对应需求清单附录 A / §0.2）：
//   **一个** handler 处理 4 条真实 HTTP 请求，按三条规则判定：
//     * 路径穿越（`..`）⇒ **403**；
//     * `<script` ⇒ **403**；
//     * `union select` ⇒ **403**；
//     * 其它 ⇒ **200**。
//   ⚠ 并且证明：**必须先 URL 解码再匹配** —— 未解码的原始路径里没有
//     `<script`（它被百分号编码成 `%3Cscript%3E`）⇒ 只匹配原始路径会
//     **全部漏过**（静默漏防）。
//
// ✅ **[W-35 / 业主 2026-10-28 裁决 D3] 本探针已迁移到 `TCP_SERVE` 形状**
//   —— 下面这一段是**给需求方的迁移指引**。
//
// ⚠ **为什么必须迁移**（W-10 落地的那条词法判据）：
//   并发册 §17.0「**宿主挂起点成员的调用点必须在任务体内**」⇒ `TCP_RECV` /
//   `TCP_ACCEPT` 只能出现在**任务体自己的顶层语句**里。旧形状把 handler 写成
//   **模块顶层**的闭包、再由 acceptor 任务体调用它 ⇒ 宿主挂起点 `TCP_RECV`
//   落在**嵌套闭包**里 ⇒ **当场**报 `E0014`（指名成员 + 位置指向调用点
//   `main.wll:<行>:<列>`）⇒ 运行 **exit 1** ⇒ 4 条请求一条也没进 handler。
//
// ✅ **`TCP_SERVE` 为什么合规**（这是迁移的**关键**）：
//   `TCP_SERVE(listener, timeout_ms, opts, handler)` 把
//   「accept → 读满一次请求 → 调 handler → 写回响应 → 下一条连接」
//   整个循环放在**一个原生成员内部**推进。
//   ⇒ §17.0 那条判据约束的是**宿主挂起点成员**（`TCP_RECV` / `TCP_ACCEPT` /
//     `SLEEP_ASYNC` —— 它们要**挂起**当前任务 ⇒ 必须有任务体可挂）。
//   ⇒ `TCP_SERVE` **不挂起**、它**阻塞**调用线程 ⇒ **不落进**那条判据。
//   ⇒ 合规形状 = **一个 `TCP_SERVE` + 一个 handler 闭包**；handler **只**返回
//     响应 DICT（`{"status", "headers", "body"}`），**不碰任何挂起点成员**。
//
// ✅ **handler 的形状变了**（需求方注意）：
//   * 旧形状：handler 收**连接**（`conn`）、自己拼响应字符串、自己 `TCP_SEND`。
//   * 新形状：handler 收**请求 DICT**
//     （`{"method": STRING, "path": STRING, "version": STRING,
//       "headers": DICT, "body": STRING}`）⇒ **返回响应 DICT**
//     （`{"status": INTEGER, "headers": DICT, "body": STRING}`；
//     **三个键缺一即该连接作废**，循环继续）。
//   * `path` 是**请求目标原样**（可能带查询串）⇒ WAF 判定**必须先 URL 解码**。
//   * `headers` 的**键一律小写**（查表不用猜大小写）。
//   * `TCP_SERVE` **不关**监听器 ⇒ 调用方自己 `TCP_CLOSE_LISTENER`（幂等）。
//
// ⚠ **代价（需求方须知）**：`TCP_SERVE` **阻塞整个运行时** ⇒ 与 `HTTP_*` 同类
//   ⇒ **同进程内的客户端拿不到响应**（客户端必然超时）。
//   ⇒ 断言落在**服务端一侧** —— 这与本探针**原有**口径一致。
//
// 期望读数（**exit 0**）：
//   [p11] raw-path= /ok                      ← 请求目标**原样**
//   [p11] decoded-path= /ok                  ← 解码后
//   [p11] status= 200
//   [p11] raw-path= /%2e%2e%2fetc%2fpasswd
//   [p11] decoded-path= /../etc/passwd
//   [p11] status= 403
//   [p11] raw-path= /%3Cscript%3Ealert(1)
//   [p11] decoded-path= /<script>alert(1)
//   [p11] status= 403
//   [p11] raw-path= /search?q=union%20select
//   [p11] decoded-path= /search?q=union select
//   [p11] status= 403
//   [p11] status-count-403= 3
//   [p11] status-count-200= 1
//   [p11] served-count= 4          ← `TCP_SERVE` 的返回值（成功服务的连接数）
//   [p11] client-no-response-count= 4  ← 客户端全部超时（见上面的「代价」）
//   [p11] raw-hits-script= FALSE   ← 未解码时**匹配不到**
//   [p11] decoded-hits-script= TRUE
//   [p11] DONE
//
// ⚠ 上面那组 `raw-path=` / `decoded-path=` / `status=` 三行**每条请求各出现
//   一次**（4 条请求 ⇒ 12 行）⇒ 它们是「这条请求真的到了服务端、而且判定
//   是按解码后的路径做的」的**逐条证据**。
// ─────────────────────────────────────────────────────────────────────────────

// ⚠ 本文件与 `impl/tests/probe/cases/P121_probe_p11_waf_checks/main.wll`
//   **逐字相同**（同一份程序的两处副本）⇒ 改动请同步两处。
// ─────────────────────────────────────────────────────────────────────────────

IMPORT("wlwl:std.io", ["PRINT"]);
IMPORT("wlwl:std.str", ["CAT", "CONTAINS_SUB"]);
IMPORT("wlwl:std.encode", ["URL_DECODE"]);
IMPORT("wlwl:std.result", ["TAKE_ERR"]);
IMPORT("wlwl:std.net", [
  "TCP_LISTEN", "TCP_SERVE", "TCP_LOCAL_ADDR", "TCP_CLOSE_LISTENER", "HTTP_GET"
]);

LET(http_client, FUN((port, path), (
  LET(r, HTTP_GET(CAT("http://127.0.0.1:", STR(port), path), 150));
  IF(==(TYPE(TAKE_ERR(r)), "NULL"), "CLIENT-GOT-RESPONSE", "CLIENT-NO-RESPONSE")
)));

LET(l, TCP_LISTEN("127.0.0.1", 0));
LET(port, AT_K(TCP_LOCAL_ADDR(l), "port", 0));

// ⚠ WAF 判定：**先 URL 解码，再匹配**。
//   `path` 是**请求目标原样** ⇒ 未解码时 `<script` 是 `%3Cscript%3E`、
//   `union select` 是 `union%20select` ⇒ 只匹配原始路径 = **全部漏过**。
LET(waf_decide, FUN((raw_path), (
  LET(decoded, URL_DECODE(raw_path));
  LET(low, LOWER(decoded));
  LET(hit_traversal, CONTAINS_SUB(low, ".."));
  LET(hit_script, CONTAINS_SUB(low, "<script"));
  LET(hit_sqli, CONTAINS_SUB(low, "union select"));
  IF(||(hit_traversal, ||(hit_script, hit_sqli)), 403, 200)
)));

LET MUT(handler_calls, 0);
LET MUT(c403, 0);
LET MUT(c200, 0);

// ⚠ handler：拿**请求 DICT** → 取**原始**路径 → 判定 → 返回**响应 DICT**。
//   它**不碰任何挂起点成员**（`TCP_RECV` / `TCP_ACCEPT` / `SLEEP_ASYNC`）——
//   handler 跑在**原生帧**里，那里挂不了（真挂了 ⇒ 那一条连接作废）。
LET(server, FUN(() , (
  TCP_SERVE(l, 800, NULL, FUN((req), (
    SET(handler_calls, +(handler_calls, 1));
    LET(raw_path, AT_K(req, "path", ""));
    LET(status, waf_decide(raw_path));
    PRINT("[p11] raw-path=", raw_path);
    PRINT("[p11] decoded-path=", URL_DECODE(raw_path));
    PRINT("[p11] status=", status);
    IF(==(status, 403), SET(c403, +(c403, 1)), 0);
    IF(==(status, 200), SET(c200, +(c200, 1)), 0);
    LET(body_text, IF(==(status, 403), "Forbidden", "OK"));
    ["status": status,
     "headers": ["Content-Type": "text/plain; charset=utf-8", "Connection": "close"],
     "body": body_text]
  )))
)));

SCOPE(FUN(() ,
  // ⚠ 服务端任务**先**挂上；客户端**后**连（连接排队）⇒ 服务端照样逐条服务。
  LET(t, SPAWN(server));

  LET(c1, http_client(port, "/ok"));
  LET(c2, http_client(port, "/%2e%2e%2fetc%2fpasswd"));
  LET(c3, http_client(port, "/%3Cscript%3Ealert(1)"));
  LET(c4, http_client(port, "/search?q=union%20select"));

  // ⚠ 主任务在这里挂起 ⇒ 服务端任务开始跑（`TCP_SERVE` 逐条服务那 4 条连接）。
  LET(served, AWAIT(t));
  PRINT("[p11] status-count-403=", c403);
  PRINT("[p11] status-count-200=", c200);
  PRINT("[p11] served-count=", served);

  // ⚠ 客户端必然看不到响应（`TCP_SERVE` 阻塞整个运行时）⇒ 4 个全是超时。
  LET MUT(no_resp, 0);
  FOR(c, [c1, c2, c3, c4], (
    IF(==(c, "CLIENT-NO-RESPONSE"), SET(no_resp, +(no_resp, 1)), 0)
  ));
  PRINT("[p11] client-no-response-count=", no_resp);

  TCP_CLOSE_LISTENER(l)
));

// ⚠ 「必须先 URL 解码再匹配」的**反例**证据：同一段路径，原始形态匹配不到
//   `<script`，解码后才匹配得到 ⇒ 只匹配原始路径 = 静默漏防。
LET(encoded_script, "/%3Cscript%3Ealert(1)");
PRINT("[p11] raw-hits-script=", CONTAINS_SUB(LOWER(encoded_script), "<script"));
PRINT("[p11] decoded-hits-script=", CONTAINS_SUB(LOWER(URL_DECODE(encoded_script)), "<script"));

PRINT("[p11] DONE");

```

---

### `p12-acceptor-pool-wakeall.wll`

```wlwl
// ─────────────────────────────────────────────────────────────────────────────
// p12-acceptor-pool-wakeall.wll —— 「多 acceptor 的惊群」探针
//
// ⚠ **文件名里的 `wakeall` 是历史名**（旧的「唤醒全部等待者」形状已废）——
//   与 `p13-single-acceptor-chain.wll` 同款：文件名保留，含义以本文为准。
//
// ✅ **[第三波 / W-11] 本探针已翻转为「惊群**已修**」的正面证据**
//   —— 下面这一段是**给需求方的迁移指引**。
//
// 它证明什么（对应需求清单附录 A / §B-2）：
//   **3 个 acceptor 等同一个监听句柄，2 条连接先后到达** ⇒
//     * **第一个**就绪事件只唤醒**一个** acceptor（**FIFO 队首**）⇒ 它拿到第一条连接；
//     * **其余等待者不被唤醒**（**不**拿到与超时同形的 `NULL`）⇒ 它们**继续等**
//       ⇒ **第二条连接到达时，其中一个接住了它** ⇒ **两个 acceptor 各拿到一条连接**；
//     * 剩下的那一个**等满自己的超时**才拿到 `NULL`（那是**超时**，不是**竞争失败**）。
//   ⇒ 这正是需求清单 §B-2 要的「**落选者保持存活并能继续接受后续连接**」。
//
// ⚠⚠ **修复前（唤醒全部等待者）** 这个探针的读数**完全相反**：
//   一个连接到达 ⇒ **唤醒全部** 3 个等待者 ⇒ 只有 **1 个**拿到连接，其余 **2 个**
//   拿到 `NULL`（与超时**同形**）⇒ 调用方拿 `NULL` 去 `TCP_RECV` ⇒
//   **`E0030: TCP_RECV: expected integer, got null`** ⇒ 任务失败 ⇒ `SCOPE` 提前退出。
//   ⇒ **「唤醒全部」本身就是缺陷**，不是「多个等待者共享一条连接」那条合法语义。
//
// ⚠ **两个读数各自钉住什么**（这才是本用例的价值）：
//   * `winners= 2` ⇒ **落选者能继续接受后续连接**（需求 B-2 的**正面**证据）。
//     ⚠ 若哪天惊群复发（改回唤醒全部）⇒ `winners` 变回 **1** ⇒ 本用例必红。
//   * `losers-woke-immediately= FALSE` ⇒ 落选者**不是**被连接到达唤醒的（惊群的**指纹**）。
//     ⚠ 惊群复发时它会变回 `TRUE`（≈150 ms）⇒ 本用例必红。
//   * 末尾那次 `TCP_RECV(loser_conn)` ⇒ `E0030` ⇒ **exit 1 是期望值**：它钉的是
//     **规范空白 SP-2** —— 「超时产生的 `NULL`」与「竞争失败产生的 `NULL`」**同形**，
//     调用方**分不出来** ⇒ 这是留给规范侧的那条缺口（W-11 修的是「不再产生竞争失败」，
//     **没有**消除「两种 NULL 同形」这件事）。
//
// 期望读数（**exit 1** —— 探针刻意让落选者真的去 `TCP_RECV(NULL)`）：
//   [p12] accept-timeout-ms= 2000
//   [p12] client1-reason= read head      ← 客户端**连上了**（不是 connect refused）
//   [p12] client2-reason= read head
//   [p12] winners= 2                       ← 两个 acceptor 各拿到一条连接 ★
//   [p12] losers= 1                        ← 第三个等满**自己的超时**
//   [p12] loser-elapsed= <≈2000>           ← 是**超时**，不是被连接唤醒
//   [p12] losers-woke-immediately= FALSE    ← 惊群已修
//   …随后 stderr 报：error[E0030]: TCP_RECV: expected integer, got null
//
// ⚠ 客户端看不到响应：`HTTP_*` 是**阻塞原语**（ADR-0028 D1）⇒ 断言落在服务端一侧。
// ─────────────────────────────────────────────────────────────────────────────

// ⚠ 本文件与 `impl/tests/probe/cases/P121_probe_p12_acceptor_pool/main.wll`
//   **逐字相同**（同一份程序的两处副本）⇒ 改动请同步两处。
// ─────────────────────────────────────────────────────────────────────────────

IMPORT("wlwl:std.io", ["PRINT"]);
IMPORT("wlwl:std.str", ["CAT"]);
IMPORT("wlwl:std.result", ["TAKE_ERR"]);
IMPORT("wlwl:std.time", ["MONOTONIC"]);
IMPORT("wlwl:std.net", [
  "TCP_LISTEN", "TCP_ACCEPT", "TCP_RECV", "TCP_SEND", "TCP_CLOSE",
  "TCP_LOCAL_ADDR", "HTTP_GET"
]);

LET(http_client, FUN((port, path), (
  LET(r, HTTP_GET(CAT("http://127.0.0.1:", STR(port), path), 150));
  LET(payload, TAKE_ERR(r));
  IF(==(TYPE(payload), "NULL"), "CLIENT-GOT-RESPONSE", payload)
)));

LET(l, TCP_LISTEN("127.0.0.1", 0));
LET(port, AT_K(TCP_LOCAL_ADDR(l), "port", 0));

LET MUT(winners, 0);
LET MUT(losers, 0);
LET MUT(loser_conn, NULL);
LET MUT(loser_elapsed, -1);
LET(t0, MONOTONIC());

// 一个 acceptor：accept 单独占一段（等待必须是所在段的第一条语句）。
// ⚠ 拿到连接 ⇒ 计入 `winners`（**它服务了这一条**）；超时 ⇒ 拿到 `NULL` ⇒ 计入 `losers`。
LET(acceptor, FUN(() , (
  LET(conn, TCP_ACCEPT(l, 2000));
  YIELD();
  LET(elapsed, -(MONOTONIC(), t0));
  IF(==(conn, NULL),
    (SET(losers, +(losers, 1)); SET(loser_conn, conn); SET(loser_elapsed, elapsed); "loser"),
    (SET(winners, +(winners, 1)); "winner"))
)));

SCOPE(FUN(() ,
  // ⚠ 顺序要紧：**先** spawn 3 个 acceptor（它们会挂在 accept 上），**再** spawn
  //   客户端任务 ⇒ 连接到达时 acceptor **已经挂起**。
  LET(t1, SPAWN(acceptor));
  LET(t2, SPAWN(acceptor));
  LET(t3, SPAWN(acceptor));
  // ⚠ **两个**客户端 ⇒ **两条**连接 ⇒ 证明「落选者继续等、接住了第二条」。
  LET(tc1, SPAWN(FUN(() , http_client(port, "/p12/a"))));
  LET(tc2, SPAWN(FUN(() , http_client(port, "/p12/b"))));
  PRINT("[p12] accept-timeout-ms= 2000");
  PRINT("[p12] client1-reason=", SUB(AT_K(AWAIT(tc1), "reason", ""), 0, 9));
  PRINT("[p12] client2-reason=", SUB(AT_K(AWAIT(tc2), "reason", ""), 0, 9));
  AWAIT(t1);
  AWAIT(t2);
  AWAIT(t3);
  PRINT("[p12] winners=", winners);
  PRINT("[p12] losers=", losers);
  PRINT("[p12] loser-elapsed=", loser_elapsed);
  // ⚠ 惊群的指纹：落选者若是被**连接到达**唤醒的 ⇒ elapsed ≈ 客户端的等待预算（≈150 ms）；
  //   若是**自己等满超时** ⇒ elapsed ≈ accept 的超时（2000 ms）⇒ 这里是 FALSE。
  PRINT("[p12] losers-woke-immediately=", <(loser_elapsed, 1500))
));

// ⚠ 落选者的**真实死法**：拿到 NULL 的调用方去 `TCP_RECV` ⇒ E0030。
//   ⚠ **这条尾巴钉的是规范空白 SP-2**（两种 NULL 同形），不是惊群本身。
PRINT("[p12] now calling TCP_RECV on the loser's NULL handle");
TCP_RECV(loser_conn, 4096, 100);

PRINT("[p12] DONE");

```

---

### `p13-single-acceptor-chain.wll`

```wlwl
// ─────────────────────────────────────────────────────────────────────────────
// p13-single-acceptor-chain.wll —— 「单接受循环」探针
//
// 它证明什么（对应需求清单附录 A / §0.2）：
//   **一个**接受循环（**一个** `TCP_SERVE` + **一个** handler）**连续服务 4 条
//   连接，全部 200**。
//   这正是需求清单 §B-3 要的「**可推进的 accept 循环**」正解 —— 旧的「静态展开
//   的 4 段 accept 链」（连接数写死在源码里）已经**不再需要**，也不**再成立**。
//
// ✅ **[W-35 / 业主 2026-10-28 裁决 D3] 本探针已迁移到 `TCP_SERVE` 形状**
//   —— 下面这一段是**给需求方的迁移指引**。
//
// ⚠ **旧形状为什么彻底不成立**（W-10 落地后的发现，本项最要紧的一条）：
//   并发册 §17.0「**宿主挂起点成员的调用点必须在任务体内**」⇒ `TCP_RECV` /
//   `TCP_ACCEPT` 只能出现在**任务体自己的顶层语句**里。
//   ⇒ 旧形状「每条连接一个 `serve_one` 辅助闭包」**结构性违规**：宿主挂起点
//     `TCP_RECV` 落在**嵌套闭包**里 ⇒ **当场**报 `E0014`（指名成员 + 位置
//     指向调用点 `main.wll:<行>:<列>`）⇒ 运行 **exit 1**，4 条连接一条也没
//     服务完。
//   ⇒ 需求清单 §B-3 当时说「当前只能用静态展开的 accept 链绕过去」，而 §17.0
//     的这条规则让**那个绕法也不成立**（accept 与 recv 必须分处两段 ⇒ recv
//     若是段首那条语句，它就必须**直接写在任务体里**）。
//
// ✅ **裁决 D3 落地的正解 = `TCP_SERVE`**（运行期托管的迭代器）：
//   `TCP_SERVE(listener, timeout_ms, opts, handler)` 把
//   「accept → 读满一次请求 → 调 handler → 写回响应 → 下一条连接」
//   整个循环放在**一个原生成员内部**推进。
//   ⇒ §17.0 那条判据约束的是**宿主挂起点成员**（`TCP_RECV` / `TCP_ACCEPT` /
//     `SLEEP_ASYNC` —— 它们要**挂起**当前任务 ⇒ 必须有任务体可挂）。
//   ⇒ `TCP_SERVE` **不挂起**、它**阻塞**调用线程 ⇒ **不落进**那条判据。
//   ⇒ 「等下一条连接」这件事**不再需要**语言层的位置 ⇒ 代码**不再随连接数
//     线性膨胀** ⇒ 这就是 §B-3 要的「可推进的接受循环」。
//
// ✅ **handler 的形状**（需求方注意）：
//   * handler 收**请求 DICT**
//     （`{"method": STRING, "path": STRING, "version": STRING,
//       "headers": DICT, "body": STRING}`）⇒ **返回响应 DICT**
//     （`{"status": INTEGER, "headers": DICT, "body": STRING}`；
//     **三个键缺一即该连接作废**，循环继续）。
//   * handler **只**返回 DICT，**不碰任何挂起点成员**（handler 跑在**原生帧**
//     里，那里挂不了；真挂了 ⇒ 那一条连接作废）。
//   * `TCP_SERVE` **不关**监听器 ⇒ 调用方自己 `TCP_CLOSE_LISTENER`（幂等）。
//   * accept 超时 ⇒ **退出循环并返回计数**（不是错误）；单条连接失败 ⇒ 只记
//     一笔、**继续下一条**。
//
// ⚠ **代价（需求方须知）**：`TCP_SERVE` **阻塞整个运行时** ⇒ 与 `HTTP_*` 同类
//   ⇒ **同进程内的客户端拿不到响应**（客户端必然超时）。
//   ⇒ 「连续 4 个请求全 200」是**服务端一侧**的断言 —— 这与本探针**原有**
//     口径一致（4 个真实 TCP 连接都被回了 200）。
//
// 期望读数（**exit 0**）：
//   [p13] served-request-count= 4   ← handler 实际被调了几次（真实到达的请求数）
//   [p13] served-count-200= 4       ← 回了 200 的连接数
//   [p13] chain-done= 4             ← 接受循环推进了几次（`TCP_SERVE` 的返回值）
//   [p13] client-no-response-count= 4  ← 客户端全部超时（见上面的「代价」）
//   [p13] DONE
//
// ⚠ 文件名里的 `chain` 是**历史名**（旧的「静态展开的 accept 链」形状已废）；
//   `chain-done` 这个输出标签是**需求清单在认的**，所以**保留** —— 它的含义
//   从今往后是「接受循环推进了几次」。
// ─────────────────────────────────────────────────────────────────────────────

// ⚠ 本文件与 `impl/tests/probe/cases/P121_probe_p13_acceptor_chain/main.wll`
//   **逐字相同**（同一份程序的两处副本）⇒ 改动请同步两处。
// ─────────────────────────────────────────────────────────────────────────────

IMPORT("wlwl:std.io", ["PRINT"]);
IMPORT("wlwl:std.str", ["CAT"]);
IMPORT("wlwl:std.encode", ["BYTE_LEN"]);
IMPORT("wlwl:std.result", ["TAKE_ERR"]);
IMPORT("wlwl:std.net", [
  "TCP_LISTEN", "TCP_SERVE", "TCP_LOCAL_ADDR", "TCP_CLOSE_LISTENER", "HTTP_GET"
]);

LET(http_client, FUN((port, path), (
  LET(r, HTTP_GET(CAT("http://127.0.0.1:", STR(port), path), 150));
  IF(==(TYPE(TAKE_ERR(r)), "NULL"), "CLIENT-GOT-RESPONSE", "CLIENT-NO-RESPONSE")
)));

LET(l, TCP_LISTEN("127.0.0.1", 0));
LET(port, AT_K(TCP_LOCAL_ADDR(l), "port", 0));

LET(body_text, "hello-wlwl");
LET(resp_headers, [
  "Content-Type": "text/plain; charset=utf-8",
  "Content-Length": STR(BYTE_LEN(body_text)),
  "Connection": "close"
]);

LET MUT(handler_calls, 0);
LET MUT(c200, 0);

// ⚠ handler：**只**返回响应 DICT，**不碰任何挂起点成员**。
LET(server, FUN(() , (
  TCP_SERVE(l, 800, NULL, FUN((req), (
    SET(handler_calls, +(handler_calls, 1));
    SET(c200, +(c200, 1));
    ["status": 200, "headers": resp_headers, "body": body_text]
  )))
)));

SCOPE(FUN(() ,
  // ⚠ 服务端任务**先**挂上；客户端**后**连（连接排队）⇒ 服务端照样逐条服务。
  LET(t, SPAWN(server));

  LET(c1, http_client(port, "/p13/1"));
  LET(c2, http_client(port, "/p13/2"));
  LET(c3, http_client(port, "/p13/3"));
  LET(c4, http_client(port, "/p13/4"));

  // ⚠ 主任务在这里挂起 ⇒ 服务端任务开始跑（`TCP_SERVE` 连续服务那 4 条连接；
  //   4 条都服务完后它再等到 accept 超时（800ms）才收工并返回计数）。
  LET(served, AWAIT(t));
  PRINT("[p13] served-request-count=", handler_calls);
  PRINT("[p13] served-count-200=", c200);
  PRINT("[p13] chain-done=", served);

  // ⚠ 客户端必然看不到响应（`TCP_SERVE` 阻塞整个运行时）⇒ 4 个全是超时。
  LET MUT(no_resp, 0);
  FOR(c, [c1, c2, c3, c4], (
    IF(==(c, "CLIENT-NO-RESPONSE"), SET(no_resp, +(no_resp, 1)), 0)
  ));
  PRINT("[p13] client-no-response-count=", no_resp);

  TCP_CLOSE_LISTENER(l)
));

PRINT("[p13] DONE");

```

---

---

## 原 `README.md`（原文照录 · 含上述 3 处漂移）

# wlwl 迁移可行性探针（需求清单 §J-3 的 9 个探针）

> **本目录是需求清单 §J-3 第 3 条承诺的兑现**：清单称「本清单附带的 9 个探针
> （`p01`–`p13`）已开源在目标仓库 `docs/wlwl-feasibility/probes/`」——**该目录原先
> 根本不存在**（见计划 §2.3 的 **R-20**）。本工作项（**W-02**）把这 9 个探针
> **重写进本仓**，使清单里每条「现状（实测）」都有**仓内可重跑**的证据。
>
> **原件不在本仓** ⇒ 这 9 个探针是**按需求清单正文的描述重写的等价探针**，不是
> 原件的拷贝。凡清单描述与实测不符的，**按实测断言**，并在探针文件头写明差异。

## 怎么跑

```powershell
# 逐个跑（仓库根）
impl\target\release\wlwl.exe run docs\wlwl-feasibility\probes\p01-capability-matrix.wll
# ...

# 仓内门禁（CI 用的同一套语义）
cd impl; cargo test --locked -p wlwl-cli --test probe
```

- 每个探针的**同款副本**在 `impl/tests/probe/cases/P121_probe_*/main.wll`
  （**逐字相同**）⇒ 断言写在同目录的 `expect.json`。
- ⚠ **这些探针都是单进程自包含的**：客户端用 `std.net::HTTP_GET` 连自己的
  listener。`HTTP_*` 是**阻塞原语**（执行期间整个 `SCOPE` 冻结，ADR-0028 D1）
  ⇒ **客户端看不到响应**（必然超时）⇒ **断言全部落在服务端一侧**。这是与清单
  描述唯一的系统性差异（清单是客户端视角）。

---

## 一、探针 ↔ 需求清单条目 对照表

| # | 探针文件 | probe case | 清单条目 | 组 | 这条探针证明的「现状」 | 与清单描述的差异 |
|---|---|---|---|---|---|---|
| 1 | `p01-capability-matrix.wll` | `P121_probe_p01_capability_matrix` | **A-1 / A-2**（TLS 缺失）、**A-4**（密码学原语）、**H-5**（`RE_TEST` 整串锚定）、**H-4**（路径分隔符不统一）、**E-4**（`LEN` vs `BYTE_LEN`）、**C-7**（路径穿越防护的前提） | A / D / H / E | ① `https:` 在**任何网络 I/O 之前**被拒（scheme 判定先于拨号）；② `SHA256` / `HMAC_SHA256` 给出标准摘要；③ `HTML_SANITIZE` 可用；④ `RE_TEST` **整串锚定**（当「包含匹配」会永远漏）；⑤ `PATH_NORMALIZE` 只认 `/`、`PATH_JOIN` 认 `\` ⇒ 二者不互逆；⑥ `READ_FILE` **保留** BOM；⑦ `LEN`（码点）≠ `BYTE_LEN`（字节） | **无** |
| 2 | `p02-handler-in-top-level-closure.wll` | `P121_probe_p02_top_level_closure` | **B-1**（嵌套闭包里的挂起点）+ 附录 A「`p02` / `p07` 反例」**写法 ①** | B | handler 整体是**模块顶层**闭包、任务体只调它 ⇒ 挂起点落在闭包体内、被任务体间接调用 ⇒ **跑通了**：终值 **42**、exit 0 | ⚠ **有**：清单称这种写法「失败、静默终止、`AWAIT` 拿到 `NULL`」⇒ **实测复现不出来** |
| 3 | `p03-accept-recv-inline.wll` | `P121_probe_p03_accept_recv_inline` | **B-1** 前半 / **B-3** 的前提（并发册 §17.0 第 2 条） | B | 宿主挂起点是**不切段**的挂起点 ⇒ 恢复时**段首重放** ⇒ **挂起点之前**的 `PRINT` 跑了**两次** ⇒ 副作用重复 | **无** |
| 4 | `p07-handler-in-nested-closure.wll` | `P121_probe_p07_nested_closure` | **B-1** + 附录 A「`p02` / `p07` 反例」**写法 ②** | B | handler 闭包里**再定义并调用**内层闭包（两层嵌套）、挂起点落在**内层** ⇒ **跑通了**：终值 **42**、exit 0 | ⚠ **有**：同 p02 |
| 5 | `p09-trace-nested-wait-death.wll` | `P121_probe_p09_nested_wait_death` | **B-1**（主证据）、**B-6**（终值可区分性） | B | 挂起点写进辅助闭包、由任务体调用 ⇒ **不静默终止**：任务正常跑完、`AWAIT` 拿到 **42**、exit 0、stderr 空；可观察的是**段首重放 ⇒ 副作用重复** | ⚠ **有**（清单的核心描述）：清单称「任务静默终止、`AWAIT` 拿到 `NULL`、stderr 全空」⇒ **实测复现不出来** |
| 6 | `p10-concurrent-server-inline-recv.wll` | `P121_probe_p10_concurrent_server` | **§0.2**「8 个并发请求全部 `200`、正文与磁盘一致、`Content-Length` **字节数**正确；5 个安全响应头随响应发出」、**C-2**（响应构造）、**D 组**（安全响应头） | §0.2 / C / D | 纯 wlwl HTTP 服务：8 个**真实 TCP 连接**全部被服务、状态码全 **200**、5 个安全响应头齐全、`Content-Length` 按**字节数**算（正文含中文 ⇒ `LEN`=9 / `BYTE_LEN`=13） | ⚠ 视角差异（客户端 vs 服务端），见上文「怎么跑」 |
| 7 | `p11-single-handler-waf-checks.wll` | `P121_probe_p11_waf_checks` | **§0.2**「路径穿越 → `403`；`<script>` 载荷 → `403`；`union select` 载荷 → `403`」、**D-2**（归一化）、**D-4**（路径包含判断） | §0.2 / D | **一个** handler 处理 4 条真实请求：穿越 / `<script` / `union select` ⇒ **403**（3 条），其它 ⇒ **200**（1 条）；⚠ 并且**必须先 URL 解码再匹配**——未解码的原始路径里**没有** `<script`（`%3Cscript%3E`）⇒ 只匹配原始路径 = **静默漏防** | ⚠ 视角差异（客户端 vs 服务端） |
| 8 | `p12-acceptor-pool-wakeall.wll` | `P121_probe_p12_acceptor_pool` | **B-2**（多任务在同一 listener 上 accept 的语义） | B | 3 个 acceptor 抢 1 个连接 ⇒ **惊群坐实**：只有 **1** 个拿到连接、其余 **2** 个拿到 `NULL`；⚠ 落选者是**被连接到达唤醒**的（≈155 ms，而非等满 8000 ms 超时）；⚠ 落选者的**真实死法** = 拿 `NULL` 去 `TCP_RECV` ⇒ **`E0030`** ⇒ `SCOPE` 提前退出 | ⚠ **有**：清单说「直接死掉」方向对，但机制不是「静默 NULL」，而是 `TCP_RECV(NULL)` ⇒ `E0030` |
| 9 | `p13-single-acceptor-chain.wll` | `P121_probe_p13_acceptor_chain` | **§0.2**「单 acceptor 任务连续服务 4 个请求全部 `200`」、**B-3**（可推进的循环 —— 只能用静态展开绕过去） | §0.2 / B | **一个** acceptor 任务用**静态展开的 4 段**（accept → recv → 回 200 → 关连接）连续服务 4 个请求、全部 200 ⇒ 这是清单给出的**可用架构样板** | ⚠ 视角差异（客户端 vs 服务端） |

---

## 二、与清单描述不一致的地方（逐条）

| 编号 | 清单原文 | 实测 | 处置 |
|---|---|---|---|
| **Δ1** | §B-1：「任务**静默终止**、`AWAIT` 拿到 `NULL`、stderr 全空、客户端只见 RST」（p09） | **复现不出来**：嵌套闭包里的挂起点**不静默终止** —— 任务正常跑完、`AWAIT` 拿到 **42**、**exit 0**；可观察的症状是**段首重放 ⇒ 副作用重复** | p09 / p02 / p07 **按实测断言**；文件头写明差异。与计划 §2.3 的 **R-24** 一致 |
| **Δ2** | §B-2：「没抢到连接的 acceptor 拿到 `NULL`（语义等同超时）并**直接死掉**」（p12） | 惊群**坐实**（1 胜 2 负、落选者被**连接到达**唤醒而非等超时），但**死法机制不同**：不是「静默 NULL」，而是调用方 `TCP_RECV(NULL)` ⇒ **`E0030: TCP_RECV: expected integer, got null`** | p12 **按实测断言**；`expect.json` 的 `exit: 1` 是**期望值**（探针刻意触发那个 `E0030`）。与计划 §2.3 的 **R-29** 一致 |
| **Δ3** | §J-3：「本清单附带的 9 个探针……**已开源在目标仓库** `docs/wlwl-feasibility/probes/`」 | **该目录原先根本不存在** ⇒ 清单的每条「现状（实测）」当时**没有仓内可重跑证据** | 本目录即该目录（**W-02** 兑现）；清单侧由 **W-01** 加「订正」批注（不在本工作项范围） |
| **Δ4** | §0.2 / 附录 A：p10 / p11 / p13 的「全 200」「403」是**客户端视角** | 单进程的 wlwl 程序**无法**既服务又当客户端收到响应：`HTTP_*` 是阻塞原语，执行期间整个 `SCOPE` 冻结（ADR-0028 D1 / 系统册 §21.4）⇒ 客户端必然超时 | 9 个探针的断言**全部落在服务端一侧**（真实 TCP 连接 + 服务端回的状态码）；每个探针文件头都写明了这一点 |

## 三、复现命令（逐个）

```powershell
$exe = 'impl\target\release\wlwl.exe'
& $exe run docs\wlwl-feasibility\probes\p01-capability-matrix.wll
& $exe run docs\wlwl-feasibility\probes\p02-handler-in-top-level-closure.wll
& $exe run docs\wlwl-feasibility\probes\p03-accept-recv-inline.wll
& $exe run docs\wlwl-feasibility\probes\p07-handler-in-nested-closure.wll
& $exe run docs\wlwl-feasibility\probes\p09-trace-nested-wait-death.wll
& $exe run docs\wlwl-feasibility\probes\p10-concurrent-server-inline-recv.wll
& $exe run docs\wlwl-feasibility\probes\p11-single-handler-waf-checks.wll
& $exe run docs\wlwl-feasibility\probes\p12-acceptor-pool-wakeall.wll    # exit 1（预期）
& $exe run docs\wlwl-feasibility\probes\p13-single-acceptor-chain.wll
```
