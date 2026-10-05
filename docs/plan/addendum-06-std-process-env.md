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
| W-01 | 路径语义裁决(分隔符 / 字节形状 / 符号链接) | 未动工 | — |
| W-02 | `PATH_*` 纯函数族 + 契约(先落,无平台依赖) | 未动工 | — |
| W-03 | 目录与文件族 + 契约(含符号链接防环) | 未动工 | — |
| W-04 | `std.env` + 契约 | 未动工 | — |
| W-05 | `std.process` + 契约(含注入守卫) | 未动工 | — |
| W-06 | skill / CHANGELOG / 附录 A 收口 | 未动工 | — |
