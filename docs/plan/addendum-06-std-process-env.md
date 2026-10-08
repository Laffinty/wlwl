# 06 · `std.process` / `std.env` / 路径与目录(扩 `std.fs`)

> **层级** L1(可并行) · **前置** 无(与 05 边界见 §3.6)
> **状态** 未动工 · **规范登记** §14 候选命名空间

## 0. 契约摘要

- **目标**:把 `std.fs` 现有的 3 个成员(`READ_FILE` / `WRITE_FILE` / `EXISTS`)
  补成一个**能写真实程序的**文件系统面,并补上进程与环境两块。
- **非目标**(违反即范围事故):
  - **不做 shell 命令执行**(`SYSTEM` / `exec("sh -c")`)。理由:它等于给了
    任意命令执行,安全面与收益完全不成比例;真要跑外部程序,给一个
    `PROCESS_RUN(argv 数组)` —— **不经 shell**,参数不经二次解析。
  - 不做权限 / 属主 / 符号链接的完整语义(只做「跟随 / 不跟随」开关)。
  - 不做异步文件 I/O。
  - **不碰 `std.io` 的 `PRINT` / `INPUT` 形态**(既有行为,被契约钉死)。
- **为什么值得做**:`std.fs` 三个成员意味着现在**连列一个目录都做不到** ——
  凡是需要遍历的实用程序都写不出来。这是「最便宜的实用程序能力」。

## 1. 基线(可核验)

```powershell
cd docs
Select-String -Path stdlib/wlwl-stdlib-spec-v0.11.md -Pattern '## 2 `std.fs`' -Context 0,10
cd ..
Select-String -Path docs/stdlib/wlwl-stdlib-spec-v0.11.md -Pattern 'std.process|std.env' -Context 0,1
```

| 项 | 现值 | 出处 |
|---|---|---|
| `std.fs` 成员 | **3**(`READ_FILE` / `WRITE_FILE` / `EXISTS`) | 附录 A |
| `std.io` 成员 | **3**(`PRINT` / `INPUT` / `PRINT_ERR`) | 附录 A |
| 进程 / 环境成员 | **0** | 附录 A |
| 规范登记 | §14 新 R2 候选:`std.process`、`std.env` | 规范 §14 |
| 主流对照 | Go `os` / `io` / `os/exec`、Python `os` / `pathlib` / `subprocess`、Java `java.nio.file` / `ProcessBuilder`、Rust `std::fs` / `std::process` / `std::env` | — |

## 2. 成员面

### 2.1 `std.fs` 扩充(路径与目录,全 R2)

| 成员 | 语义要点 | 失败口径 |
|---|---|---|
| `PATH_JOIN(parts…)` | 路径拼接,**跨平台分隔符** | 永不失败 |
| `PATH_DIR(p)` / `PATH_BASE(p)` / `PATH_EXT(p)` | 拆解 | 永不失败 |
| `PATH_NORMALIZE(p)` | 消 `.` / `..` / 冗余分隔符。**不碰符号链接** | 永不失败 |
| `LIST_DIR(p)` | 列目录 → `ARRAY[STRING]`,**不递归** | 不存在 / 非目录 → `ERR([kind: FsError])` |
| `WALK_DIR(p, pattern?)` | 递归遍历 → `ARRAY[STRING]` | 同上 |
| `MKDIR(p)` / `MKDIRS(p)` | 建目录 / 建多级 | 已存在 → `ERR`(**幂等吗?要裁决**) |
| `REMOVE(p, recursive?)` | 删文件 / 删目录树 | 不存在 → `ERR` |
| `COPY(src, dst)` / `MOVE(src, dst)` | 复制 / 移动 | 任一失败 → `ERR` |
| `FILE_SIZE(p)` | 字节数 | 不存在 → `ERR` |
| `READ_BYTES(p)` / `WRITE_BYTES(p, data)` | **二进制**读写(现有成员是文本口径) | 同上 |

### 2.2 `std.env`(新命名空间,R2)

| 成员 | 语义要点 |
|---|---|
| `ENV_GET(name, default?)` | 缺省值可省;不存在且无缺省 → `NULL` |
| `ENV_SET(name, value)` | **仅本进程可见**,不写磁盘 |
| `ENV_KEYS()` | 全部键名,排序后返回(**确定性**) |
| `ARGS()` | 命令行参数 → `ARRAY[STRING]` |

### 2.3 `std.process`(新命名空间,R2)

| 成员 | 语义要点 |
|---|---|
| `PROCESS_RUN(argv, opts?)` | **不经 shell**。`opts` 可含 `{timeout, cwd, env}`;返 `{code, stdout, stderr}` |
| `PROCESS_EXIT_CODE` | 上一条 `PROCESS_RUN` 的退出码(省一次取字段) |
| `PROCESS_ID()` | 进程 id |

**失败口径**:文件 / 环境 / 进程失败一律是 `ERR` **值**并带 `kind`
(`FsError` / `EnvError` / `ProcessError`)+ `op` + `reason`;
**元数 / 类型错**是 `E0022` / `E0030`。

## 3. 语义裁决点(开写前必须逐条回答)

1. **路径分隔符**:规范现有 `std.fs` 用的是 `/` 还是 `\`?跨平台形态要写死,
   否则 Windows 与 Linux 上同一份程序行为不同(这正是附录 A 锁测试当年踩过的
   CRLF 坑的同源问题)。
2. **`MKDIR` 幂等吗?** POSIX `mkdir` 幂等失败(EEXIST),`mkdir -p` 幂等成功。
   两个都要 ⇒ `MKDIR` / `MKDIRS` 分开(本表已分),但**各自语义要写明**。
3. **符号链接**:`LIST_DIR` / `WALK_DIR` 遇符号链接跟随吗?跟随则可能成环
   (`WALK_DIR` 无限递归)。→ 建议**默认不跟随**,提供显式开关,且跟随时
   **记visited 集合防环**。
4. **`READ_BYTES` 的数据形状**:返回什么?`STRING` 装不了任意字节
   (wlwl 的 `STRING` 是 UTF-8,与 `std.encode` 的 `BASE64_DECODE` 失败口径
   同款)。→ 建议:字节数据用 **`ARRAY[INTEGER]`** 或 **hex 串**,
   两种都试过再定。**这是个真设计问题**,与 `std.encode` §11.1 的口径相连。
5. **`PROCESS_RUN` 的 `argv` 是 `ARRAY[STRING]` 还是 `STRING`?** 必须是
   数组 —— 拼字符串再解析就是命令注入。规范要**显式禁止**字符串形态。
6. **与 05 的边界**:DNS / 代理归 05(§5 有裁决建议)。本份不碰 socket。
7. **`ENV_KEYS()` 的排序**:确定性是这个语言的卖点 ⇒ 必须排序后返回,
   不能是 `OS` 的迭代序。

### 3.1 只读调查(2026-10-08):七条裁决点的证据与候选

开工前把七条逐条核过。**四条有仓内先例可直接定案,三条是真岔路。**

#### (a) Q1 路径分隔符 —— **真岔路,且现状是个未被记录的隐式决定**

**核到的现状**:规范 §2 `std.fs` 的三个成员对分隔符**只字未提**;实现
(`fs.rs:76/81/86`)把 `path` 原样交给 Rust 的 `std::fs::read_to_string` /
`write` / `metadata` ⇒ **平台原生口径**(Windows 上 `/` 与 `\` 都收,
POSIX 上只收 `/`)。

⚠️ 这意味着**「wlwl 的路径是什么」从来没有被裁决过**,它只是「Rust 是什么」的
影子。而它有个**直接后果**:若 `PATH_JOIN` 也用平台原生分隔符,
`PATH_JOIN("a", "b")` 在 Windows 上返回 `a\b`、在 Linux 上返回 `a/b` ⇒
**同一份程序跨平台产出不同字符串** ⇒ 契约表**无法逐字冻结**跨平台值,
§4 的「跨平台矩阵 job 产出相同」那条验收**直接不可能满足**。

| 候选 | 形态 | 代价 |
|---|---|---|
| **甲** | **规范分隔符恒为 `/`**;输入端 Windows 额外容忍 `\`(**既有行为,不能收**),输出 / `PATH_JOIN` / `PATH_NORMALIZE` 一律 `/` | 需在规范写死;`std.fs` 现有行为**不变**(只是被文档化) |
| 乙 | 平台原生(现状) | 零改动,但**跨平台契约不可冻结**;`PATH_*` 的期望值要按平台分支写,等于放弃「同一份程序同一结果」这条卖点 |
| 丙 | 平台原生 + 另给 `PATH_JOIN_POSIX` | 两个真相源,下一个人不知道该用哪个 |

**记录者推荐甲** —— 理由不是「统一好看」,是**乙让契约表无法逐字冻结**,
而逐字节冻结是本仓所有契约表的立足点。

#### (b) Q4 `READ_BYTES` 数据形状 —— **仓内已有先例,可直接定案**

`std.encode::RANDOM_BYTES(n)` **已经返回 `ARRAY[INTEGER]`**,当时写进规范的理由
(§11.6 + CHANGELOG)原话是:「wlwl 没有字节类型,硬塞进 `STRING` 只能把非法
UTF-8 **有损替换**成 `U+FFFD` —— §11.3-3 已把『不做有损替换』立成规范条款」,
并注明它是**本语言唯一无损字节载体**。

⇒ **候选甲:`READ_BYTES` / `WRITE_BYTES` 用 `ARRAY[INTEGER]`,与 `RANDOM_BYTES`
同款**。候选乙(hex 串)有**双份 hex 开销**(压缩后的字节要先 hex 才能进语言、
出来又要 hex 才能用),且等于让 `std` 出现**两种字节表示**。

⚠️ **但要补一条范围条款**(本条 `addendum-07` §3.2 会**复用**它,必须一次写清):
`ARRAY[INTEGER]` 适合**小**二进制(配置、图标、密钥材料片段);
**大**载荷的正解是**让它待在文件里**(压缩 / 拷贝都在 R2 侧用字节流做),
而不是绕语言值搬一趟 —— 解释器侧的数组 / 字符串构建有已归档的超线性证据
(D11-012 / D12-009),拿它当大载荷通道会把那条坑重新踩一遍。

#### (c) Q2 / Q3 / Q5 / Q6 / Q7 —— 候选与理由

| # | 候选 | 理由 |
|---|---|---|
| Q2 `MKDIR` 幂等 | **甲**:`MKDIR` 跟随 POSIX —— 已存在 → `ERR`(不静默);`MKDIRS`(=`mkdir -p`)幂等成功 | 拆成两个成员正是因为语义不同(计划 §2.1 已拆);静默吞掉「已存在」会掩盖竞态 |
| Q3 符号链接 | **甲**:`LIST_DIR` / `WALK_DIR` **默认不跟随**;`WALK_DIR` 提供显式跟随开关,且**跟随时记 visited 集合防环** | 成环的符号链接目录会让 `WALK_DIR` **无限递归**;§4 的「成环 ⇒ 必须终止」那条用例正是在钉它 |
| Q5 `PROCESS_RUN` 的 `argv` | **甲**:**只接受 `ARRAY[STRING]`**,规范**显式禁止**字符串形态 | 拼字符串再解析就是**命令注入**;§4 那条「`; rm -rf /` 不被 shell 解释」是本成员存在的**安全理由** |
| Q6 与 05 的边界 | **甲**:DNS / 代理 / socket 归 `05`,本份**只做** `PROCESS_RUN` 跑进程 | 两份都在 L1 可并行,边界不清会两头落空 |
| Q7 `ENV_KEYS()` | **甲**:**排序后返回** | 确定性是本语言卖点第一条;`OS` 的迭代序**逐次可能不同** |

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

契约表 `tests/fs_contract.rs` + `tests/process_contract.rs`(新)。**测试不得
依赖真实文件系统状态** ⇒ 用 `std.test` 的临时目录约定(既有 `scratch_dir()`
体例,见 `sanitize_contract.rs` 的同款写法):

| 组 | 用例 | 备注 |
|---|---|---|
| `PATH_*` | **纯函数,全量表驱动**;含 Windows / POSIX 两套分隔符样例 | 跨平台形态裁决的守卫 |
| 目录 | 建 / 列 / 递归列 / 删,各自在临时目录内 | 每条自建自清 |
| 符号链接 | 成环的链接目录 ⇒ `WALK_DIR` **必须终止** | 防无限递归 |
| 二进制 | `WRITE_BYTES` 写入非 UTF-8 字节 ⇒ `READ_BYTES` 逐字节相同 | 钉住数据形状裁决 |
| 权限失败 | 对不存在路径操作 → `ERR` 而非 panic / OOM | |
| `ENV_KEYS` | 两次调用顺序**相同**(确定性) | |
| **`PROCESS_RUN` 注入** | `argv` 里有 `; rm -rf /` 之类 ⇒ **不被 shell 解释** | **这条是本成员存在的安全理由** |
| `PROCESS_RUN` 超时 | 超时 → `ERR`,不挂死 | 与 05 同款形态 |
| 跨平台 | 同一组用例在 Windows 与 Linux 上**产出相同** | 矩阵 job 已覆盖 |

## 5. 九处表落点

九处见 `docs/history/20261002.md` §5.0。**扩一个 + 新增两个**:

| 落点 | 本份动作 |
|---|---|
| 1 规范成员表 | §2 扩 `std.fs`(+10)+ 新增 §18 `std.env`(4)/ §19 `std.process`(3) |
| 2 附录 A | 跑 `gen-appendix-a` |
| 3 `EXPORT` / `SPEC.functions` | `fs.rs` 扩;`env.rs` / `process.rs` 新增 |
| 4 契约测试硬编码 | 三份契约表的成员数与点名清单 |
| 5 模块登记 | `ALL_SPECS` 登记两个新模块;`src/` 文件数守卫 |
| 6 probe | ≥ 2 条(路径拼接 / 字节往返),`EXPECTED_CASE_COUNT` 同步 |
| 7 附录 G | **不动** |
| 8 skill 指针 | `SKILL.md` + `reference.md` §9.10 / §9.11;`std.fs` 的 §9.x 也要扩 |
| 9 CHANGELOG | 成员面 +17;命名空间 +2 |

## 6. 风险与已知代价

- ⚠️ **`READ_BYTES` 的数据形状是硬裁决**(§3.4)。`STRING` 装不了任意字节,
  沿用 `std.encode` 的「不做有损替换」纪律就得另选形状 —— 而这会影响
  之后所有二进制成员的形态。**先定,再写。**
- ⚠️ **文件系统成员天然有平台差异**:权限位、符号链接、大小写敏感
  (Windows 不敏感 / Linux 敏感)。契约用例必须**显式写清哪些是平台相关的**,
  否则 CI 三平台矩阵会出现「只有 macOS 红」的疑难 case。
- ⚠️ **`PROCESS_RUN` 是本份里安全面最重的一个**:`argv` 必须数组、不经 shell
  这两条要在**成员表第一行**写死,并用注入用例钉住。
- **明确不做**:shell 字符串命令 / 权限属主完整语义 / 异步 I/O / 符号链接跟随
  默认开 / `std.io` 形态变更。

## 7. 检查点

| 子项 | 内容 | 状态 | commit |
|---|---|---|---|
| W-01 | 路径语义裁决(分隔符 / 字节形状 / 符号链接) | ✅ **完成(2026-10-08)** —— 只读调查核出**「wlwl 的路径是什么从未被裁决过」**(规范 §2 对分隔符只字未提,实现把 path 原样交给 `std::fs`)⇒ 四条裁决**全按推荐采纳**:分隔符恒 `/` 且 Windows 输入容忍 `\`、符号链接默认不跟随 + visited 防环、字节形状 `ARRAY[INTEGER]` + 「小载荷」范围条款、`MKDIR` 响 / `MKDIRS` 幂等。⚠️ 另裁掉 `PROCESS_EXIT_CODE`(需跨调用隐藏状态,与 `ADR-0027` A1 同款理由)。推导见 §3.1 | 待回填 |
| W-02 | `PATH_*` 纯函数族 + 契约(先落,无平台依赖) | ✅ **完成(2026-10-08)** —— `PATH_JOIN` / `PATH_DIR` / `PATH_BASE` / `PATH_EXT` / `PATH_NORMALIZE` 五个**纯函数** + `WALK_DIR` 的 glob 匹配器(段级 `**` + 段内 `*` / `?`)落地。契约 `fs_contract` 里 23 条表驱动用例**逐字冻结**跨平台值 —— 这正是「恒为 `/`」这条裁决存在的理由 | 待回填 |
| W-03 | 目录与文件族 + 契约(含符号链接防环) | ✅ **完成(2026-10-08)** —— `LIST_DIR` / `WALK_DIR` / `MKDIR` / `MKDIRS` / `REMOVE` / `COPY` / `MOVE` / `FILE_SIZE` / `READ_BYTES` / `WRITE_BYTES` 落地。⚠️ **IO 契约用例必须用绝对路径**:fs 成员的相对路径**按进程 CWD 解析**(`with_base_dir` 只影响 `IMPORT`)⇒ 一版用相对路径的测试把 `a.bin` 写进了**工作树**,已清理并把该语义写进规范 §2.3 | 待回填 |
| W-04 | `std.env` + 契约 | ✅ **完成(2026-10-08)** —— 新命名空间 `wlwl:std.env`(4 成员)。`ENV_KEYS()` **排序**是规范性要求(宿主环境表迭代序逐次可能不同,而可复现是卖点第一条);`ENV_GET` 缺省返 `NULL` 而非 `""`(「没设」与「设为空串」是两件事);`ARGS()` 含 `argv[0]`(同 Go / Python) | 待回填 |
| W-05 | `std.process` + 契约(含注入守卫) | ✅ **完成(2026-10-08)** —— 新命名空间 `wlwl:std.process`(2 成员)。⚠️ **本轮抓到两个真缺陷**:① 超时**没兜住** —— 300 ms 走了 **29.26 s**,根因是 Windows 上 `child.kill()` **不杀孙进程**而它握着管道句柄 ⇒ 读线程等不到 EOF;修法「**杀进程树** + **join 必须有界**」。② 测试**污染工作树**(见 W-03)。`argv` **只收 `ARRAY[STRING]`**、字符串形态 `E0030`,契约与 probe 各有一条「元字符不被解释」守卫 | 待回填 |
| W-06 | skill / CHANGELOG / 附录 A 收口 | ✅ **完成(2026-10-08)** —— 附录 A 重生成;规范 §0.2 计数 **18 命名空间 / 147 成员**、`§2` 重写成三节、**新增 §18 / §19**;probe **167 → 168**(`M6_std_fs_paths_and_process_safety`);skill + CHANGELOG 同步 | 待回填 |
