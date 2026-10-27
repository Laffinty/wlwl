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
