# WLWL 标准库规范 v0.11

WLWL 标准库的成员契约:21 个命名空间、214 个成员的名字、签名、语义、失败口径与边界。

- **与语言规范的分工**:语言规范 [`docs/spec/wlwl-spec-v0.11.md`](../spec/wlwl-spec-v0.11.md) 定义语言本身(词法、语法、类型与值、错误模型、模块系统、面向对象、并发);本规范定义标准库**成员**的契约。语言规范 §10 只保留语言表面(全局内建)与命名空间的机制性条目,成员面一律以本规范为准。
- **依赖声明**:本规范以语言规范 v0.11 的**值域**(§2.1)、**错误模型**(§8)、**模块机制**(§9)、**诊断码表**(§11)与**并发语义**(§17)为前提,不重复定义。两册冲突时以语言规范为准;唯一的例外登记在 §0.10。
- **规范性用语**(必须 / 不得 / 应当 / 可以)沿用语言规范 §0.3。

## 0 导言与总则

### 0.1 本规范的构成

本规范定义 WLWL 标准库的成员面:21 个命名空间、214 个成员(完整注册镜像见附录 A)。第 1 章到第 22 章各对应一个命名空间,每章含成员表(签名 / 语义 / 失败 / 层)、共享失败口径、值形状与边界、示例与该章特有条款;第 14 章是**非规范性**的演进方向;附录 A 是**规范性**的成员注册镜像。

全局内建(`PRINT` / `LEN` / `FORMAT` 等)**不在**本册重复定义,见语言规范 §10.2–§10.5、§10.7、§10.9、§10.12 与其附录 G。

### 0.2 读者

本规范面向三类读者:使用标准库的 WLWL 程序员、标准库实现者,以及需要判断某个成员行为是否合规的工具作者。

### 0.3 规范性用语

本规范中的关键词 **必须**(MUST)、**不得**(MUST NOT)、**应当**(SHOULD)、**可以**(MAY)按以下含义使用:

- **必须 / 不得**:实现的硬性约束。违反时实现必须产生一个诊断(语言规范 §11)并按 §11.4 终止或报告。
- **应当**:强烈推荐的行为;实现可以偏离,但偏离必须在本规范或实现文档中说明。
- **可以**:真正的实现自由度。

标注为**非规范**(non-normative)的段落、注释与附录不构成一致性要求。

### 0.4 一致性

一个 WLWL 程序是符合本规范的,当且仅当它只使用本规范定义的成员面,并按各章的语义与失败口径使用它们。一个实现是一致的,当且仅当它提供本规范第 1 章到第 22 章列出的全部成员,对每个成员给出规定的签名、语义、失败行为与边界,且不依赖任何标注为未定义、保留或实现定义的行为。

成员面以**附录 A**为准:附录 A 由生成器从实现清单产出,与本册各章的成员表逐项对拍。

### 0.5 记法

成员表统一为四列:

| 列 | 含义 |
|---|---|
| 签名 | 成员名与形参;`?` 表示可选形参;`...` 表示变参 |
| 语义 | 正常路径的可观察行为 |
| 失败 | 失败时的可观察行为;`—` 表示永不失败 |
| 层 | 实现分层(§0.7);R0 内建层 / R1 语言层 / R2 原生层 |

正文中的 `code` 样式表示词法记号、字面代码或诊断码。示例代码以 `//` 注释标出其求值结果。值形状用 `json` 标记的代码块给出。

**规范性标注**:本册除 §14 外全部为**规范性**;§14 是**非规范性**的演进方向,各章的「设计依据」小节是**非规范性**的追溯材料,附录 A 是**规范性**的成员注册镜像。

### 0.6 调用面

标准库的调用面只有两种形式,与实现层无关:

- **全局内建**(R0 内建层):无须导入;契约见语言规范 §10.2–§10.5、§10.7、§10.9、§10.12 与其附录 G。本规范不重复、不镜像全局内建。
- **命名空间成员**:`IMPORT("wlwl:std.X", [names])` 导入后调用;成员契约见本规范各章。官方包 `ai` / `agent` 的导入路径分别是 `ai:ask` 与 `agent:task`(第 9 章、第 10 章)。

**本语言没有「命名空间限定调用」表达式**:文法里没有 `ns.MEMBER(...)` 这种形式,冒号只出现在 `IMPORT` 的路径字符串与改名处。成员只能经 `IMPORT` 取裸名;需要避开与类型名同名时,在 `IMPORT` 时改名。

### 0.7 实现分层

| 层 | 实现语言 | 内容 |
|---|---|---|
| R0 内建层 | 纯 Rust(编译器/运行时内部) | 语言自身无法表达或与编译器强耦合的原语;即语言规范附录 G 全部条目 |
| R1 语言层 | 纯 wlwl(`.wll` 源码,`include_str!` 嵌入二进制) | 不触系统调用且非性能敏感的库;**非敏感库的默认归宿** |
| R2 原生层 | 纯 Rust(`wlwl-std`) | 触系统调用、性能敏感、或需复用成熟 Rust crate 的库 |

**归属判定**(机械执行):

1. 触系统调用 / OS 资源 / 进程·网络·时间·随机 → R2;
2. 语言核心语义 → R0;
3. 其余 → R1;仅当基准数据证明热点且解释器短期无优化空间时下沉 R2;
4. R2 成员必须有不依赖实现细节的语义描述,保证可被 R1 等价替换。

**归属总表**

| 命名空间 | 层 | 章 |
|---|---|---|
| `std.io` | R2 | §1 |
| `std.fs` | R2 | §2 |
| `std.json` | R2 | §3 |
| `std.format` | R2 | §4 |
| `std.collection` | 混合(R1 门面 + R2 内核) | §5 |
| `std.str` | R1 | §6 |
| `std.math` | 混合(R1 门面 + R2 内核) | §7 |
| `std.test` | 混合(R1 门面 + R2 内生内核) | §8 |
| `ai`(官方包) | R2 | §9 |
| `agent`(官方包) | R2 | §10 |
| `std.encode` | R2 | §11 |
| `std.text` | R2 | §12 |
| `std.sanitize` | R2 | §13 |
| `std.time` | R2 | §15 |
| `std.regex` | R2 | §16 |
| `std.rand` | R2 | §17 |
| `std.env` | R2 | §18 |
| `std.process` | R2 | §19 |
| `std.compress` | R2 | §20 |
| `std.net` | R2 | §21 |
| `std.result` | 混合(R1 + R2) | §22 |

**层间规则**:R1 可以调用 R0 与 R2;R2 **不得**调用 R1(确需时走白名单并登记偏差)。混合模块以门面(R1 侧)为对外契约层。层归属变更(下沉/上浮)**不算**破坏性变更,但必须附基准数据并登记(§0.8)。

**R1→R2 私有注入通道(kernel)**:混合模块的 R2 实现经 `StdSource::kernels` 在 R1 源码求值**之前**注入到子求值器的局部环境;该通道是**私有**的 —— **kernel 不外泄**:① kernel 名字必须带 `_` 前缀(全大写),② 不进 `EXPORT` 声明(故不进附录 A、不进 `IMPORT` 可见面),③ 不与任何全局内建同名。门面若要把它变成成员,只能**改名导出**(`LET(RANGE, _RANGE);` 这类),不改变 kernel 本身。

### 0.8 稳定性政策

- **v0.x**:可见面变更(新增 / 改签名 / 改语义 / 移除 / 弃用)必须同批次完成四件套同步(本册条目、附录 A 镜像、与本册成员表的外部对照、CHANGELOG 条目)并留 CHANGELOG 条目;破坏性变更允许但必须明标 breaking;弃用走两步(警告码 + 至少保留一个次版本);**新增命名空间不得重导出全局内建同名成员**(既有 `std.io` / `std.format` 冻结为例外,不扩充)。
- **1.0 起**:冻结 —— 不移除成员、不改签名、不改可观察语义;新增成员、新增命名空间与 R1↔R2 实现迁移不受限;例外仅安全修复与 bug 修复(以规范为准)。

**四件套**的每一项对每个命名空间成员都**必须**齐备:

1. **本规范条目**:成员表 + 语义 + 失败行为;
2. **附录 A 注册镜像**:由生成器从实现清单(R2 取 `wlwl-std` 的 `ModuleSpec` 绑定表;R1 取嵌入源码的 `EXPORT` 声明)提取,锁测试双向守护;
3. **与本规范成员表的外部对照**:契约测试从本规范的表格**解析**出成员面,与实现清单对拍(集合相等)。第 2 条两侧都读实现,规范被改坏而实现没动时它照样全绿;第 3 条补的就是这个缺口;
4. **ERR 消费者注册**:R2 成员按 ADR-0009 在 init 时自注册;R1 成员的 `ERR` 处理由语言语义(语言规范 §8.2 / §8.3)自然承担,不经消费者注册表。

**`.wll.sig` 不在本清单内**:语言规范 §9.6 写的是旁路签名文件「**可以**有」,且三个方向都只在 `gradual_typing` 开启时被检查;而 std R1 模块的加载路径是 `parse` + `eval_module`,**永不做类型检查**,给它配 `.wll.sig` 只会多出一份**永不被校验**的副本。偏差 D11-013。

### 0.9 分发

- release 二进制**默认嵌入**全部 R1 源码,运行时不读取外部文件;
- release 压缩包附 `stdlib/` 参考副本(R1 源码,运行时不读取);
- 开发期源码覆盖:显式 flag / debug 环境变量指向源码目录,仅覆盖 R1 加载路径;属非稳定实现细节,**不得**改变任何成员的名字、签名与语义(锁测试守护);
- **「debug 环境变量」是硬门控**:`WLWL_STD_SRC` 只在 debug 构建(`debug_assertions`)下被读取,release 构建**无条件忽略**它。release 下要用覆盖轨,只能显式敲 `--std-src`(`wlwl run --std-src DIR`),它需要人主动给出,且同样受上面的「不得改变成员契约」约束。偏差 D11-019。

### 0.10 与语言规范的跨册差异登记

语言规范 §10.6 对 `wlwl:std.collection` 的描述写的是「混合实现(R1 门面 + R2 `RANGE`;**其余 16 成员为纯 wlwl**)」并列出 17 个成员名。本规范的 §5 写的是 **27 个成员**,其中 **13 个**归 R2 内核(`RANGE` / `MAP` / `FILTER` / `CHUNK` / `WINDOW` / `ENUMERATE` / `ZIP` / `UNIQ` / `FLAT` / `JOIN` / `GROUP_BY` / `DEDUP_BY` / `KEY_BY`),其余 14 个为纯 wlwl。

**本规范以本册为准**:语言规范 §10.6 那段是 v0.11 早期批次的快照,成员面在后续批次中扩充过,而语言规范的 §10 只承担「机制性条目」,不承担成员清单。偏差 D11-012。语言规范本身不在本轮的修改范围内。

## 1 `std.io` — 基础 I/O

与全局内建同名同语义的三个成员(**冻结例外**,见 §0.8;语义以语言规范 §10.2 为准)。

| 签名 | 语义 | 失败 | 层 |
|---|---|---|---|
| `PRINT(args...) -> NULL` | 依序渲染实参(语言规范 §2.5),以空格分隔输出到标准输出 | — | R2 |
| `PRINT_ERR(args...) -> NULL` | 同上,输出到标准错误 | — | R2 |
| `INPUT(prompt?) -> STRING` | 读一行标准输入 | 读失败按宿主语义 | R2 |

```wlwl
PRINT("hello, ", "world");   // hello, world
```

## 2 `std.fs` — 文件系统

路径、目录与字节读写的成员面。**规范分隔符恒为 `/`**(§2.1);字节形状恒为 `ARRAY[INTEGER]`(§2.2);失败分「原生诊断」与「`ERR` 值」两套口径(§2.3)。

### 2.0 v0.3 的三个冻结成员

| 签名 | 语义 | 失败 | 层 |
|---|---|---|---|
| `READ_FILE(path) -> STRING` | 读 UTF-8 文本;父目录必须已存在 | 文件不存在 → `E0061`(原生诊断,终止运行) | R2 |
| `WRITE_FILE(path, text) -> NULL` | 写 UTF-8 文本;父目录必须已存在 | 写失败 → `E0062`(原生诊断) | R2 |
| `EXISTS(path) -> BOOLEAN` | 存在测试 | — | R2 |

这三个成员自 v0.3 起**契约冻结**,语义一字未动。

### 2.1 路径成员

| 签名 | 语义 | 失败 | 层 |
|---|---|---|---|
| `PATH_JOIN(parts...) -> STRING` | 变参(≥1,全 `STRING`);用 `/` 连接,**合并重复分隔符**、**跳过空片段** | 元数 < 1 或类型错 → `E0022` / `E0030` | R2 |
| `PATH_DIR(p) -> STRING` | 父目录。**无父目录时返回 `""`**(不是 `"."`) | 同上 | R2 |
| `PATH_BASE(p) -> STRING` | 最后一段。**不剥扩展名**(`a.tar.gz` → `a.tar.gz`) | 同上 | R2 |
| `PATH_EXT(p) -> STRING` | 扩展名,**不含前导点**;无扩展名 → `""`;**点文件没有扩展名**(`.gitignore` → `""`) | 同上 | R2 |
| `PATH_NORMALIZE(p) -> STRING` | 消 `.` / `..` / 冗余分隔符。**纯词法,绝不碰符号链接** | 同上 | R2 |

**分隔符口径**:规范形态恒为 `/`。**输入端**在 Windows 上额外容忍 `\` —— 那是 v0.3 三个成员**既有**的行为(它们把 path 原样交给 OS),收掉就是破坏性变更。**POSIX 上不做 `\` → `/` 的替换**:`\` 在 POSIX 里是**合法文件名字符**,把它换成 `/` 会让一个真实存在的文件「找不到」。

**出参端同理**:`LIST_DIR` / `WALK_DIR` 返回的相对路径**只在 Windows 上**做 `\` → `/` 归一。在 POSIX 上也替换,形如 `a\b.txt` 的**合法文件名**会被吐成 `a/b.txt`,而**把出参原样喂回 `READ_FILE` / `MOVE` 会指向另一个路径** —— 这是**不可逆的往返破坏**。Windows 上替换**无损**(`\` 不可能出现在 Windows 文件名里)。偏差 D13-044。

**为什么分隔符要写死**:平台原生形态下 `PATH_JOIN("a","b")` 在 Windows 返 `a\b`、Linux 返 `a/b` ⇒ **同一份程序跨平台产出不同字符串** ⇒ 契约表**无法逐字节冻结**。而逐字节冻结是本规范所有契约表的立足点。

**`PATH_DIR` 选 `""` 而不是 `"."`**:本语言里 `PATH_JOIN(PATH_DIR(x), PATH_BASE(x))` 是取回 `x` 的惯用写法,`""` 让它成立而不必特判。

**`PATH_NORMALIZE` 是纯词法的**:它**不知道** `a/../b` 里的 `a` 是不是符号链接,所以「消 `..`」可能与真实文件系统不一致。这是刻意的 —— 一个会去 `canonicalize` 的 `NORMALIZE` 就有 I/O 副作用、且路径不存在时行为突变,那不是「归一化」。要真实解析用 `EXISTS` / `LIST_DIR`。

### 2.2 目录与字节成员

| 签名 | 语义 | 失败 | 层 |
|---|---|---|---|
| `LIST_DIR(p) -> ARRAY[STRING]` | 列目录(不递归),**结果排序** | 不存在 / 非目录 → `ERR([kind: "FsError", op, reason])` | R2 |
| `WALK_DIR(p, pattern?, follow?) -> ARRAY[STRING]` | 递归,返回**相对 `p`** 的路径(用 `/`),**排序**。⚠ `p` 本身打不开 ⇒ `ERR`,与 `LIST_DIR` 同口径;递归途中的**子目录**打不开(权限 / 竞态删除)静默跳过 | 同上 | R2 |
| `MKDIR(p) -> NULL` | 建**单层**;**已存在 → `ERR`**(不静默) | 同上 | R2 |
| `MKDIRS(p) -> NULL` | 建**多级**(`mkdir -p`);**已存在即成功** | 同上 | R2 |
| `REMOVE(p, recursive?) -> NULL` | 删文件 / 删目录;目录**未**带 `recursive = TRUE` 时非空即失败 | 同上 | R2 |
| `COPY(src, dst) -> NULL` | 复制**文件**;目录**不做** | 同上 | R2 |
| `MOVE(src, dst) -> NULL` | 移动 / 重命名;跨设备自动回退为「复制 + 删除」 | 同上 | R2 |
| `FILE_SIZE(p) -> INTEGER` | 字节数 | 同上 | R2 |
| `READ_BYTES(p) -> ARRAY[INTEGER]` | 二进制读,元素 `0..=255`。⚠ 单次超过 `MAX_READ_BYTES` = **1 MiB** ⇒ `ERR`,**不是截断、不是 OOM** | 同上 | R2 |
| `WRITE_BYTES(p, data) -> NULL` | 二进制写;`data` 是 `ARRAY[INTEGER]`,**逐元素校验** `0..=255`。⚠ 单次超过 `MAX_WRITE_BYTES` = **64 MiB** ⇒ `ERR`,**不是截断、不是 OOM**;⚠ **超限时目标文件必须原封不动**(上限在**落盘之前**判 —— 落盘会截断目标文件,判晚了就是把原文件清空) | 同上 | R2 |

**符号链接**:`LIST_DIR` **列出**链接条目(你有权知道它存在)但**不跟随**;`WALK_DIR` **默认不下降进链接目录**(`follow = FALSE`),因为**成环的链接目录会让递归永不终止**。`follow = TRUE` 时**下降进**链接目录,并记 **visited 集合**(按 `canonicalize`)防环;**此语义在 POSIX 与 Windows 上相同**。偏差 D13-060。

**`MOVE` 的跨设备回退**(复制 + 删除)按 `WALK_DIR(follow = TRUE)` 的纪律展开符号链接并记 `visited` 防环;**命中环返 `ERR` 而非跳过**。返 `ERR` 是刻意的:跳过会让随后的 `remove_dir_all(src)` **把源侧链接删掉** —— 搬完少一个链接,比直接报错更坏。偏差 D13-046。

🔴 **`MOVE` 对符号链接的处理**随**源与目标是否同一设备**而变,这是**实测**结论,不是设计意图:

| 场景 | 走的路径 | 链接在目的地的结果 |
|---|---|---|
| **同设备** | `rename` | **链接原样保留**(符号链接仍是符号链接) |
| **跨设备**(回退) | 复制 + 删除 | **按内容展开** —— 目录链接变成实体目录,文件链接变成目标文件的内容 |

⚠ **为什么跨设备是「按内容展开」而不是「原样重建链接」**:「原样重建」在 POSIX 上要 `symlink`,在 Windows 上要**特权或开发者模式**;没有那些条件时它**根本没有定义**(退化成展开,或直接失败)⇒ `MOVE` 的结果**不应该取决于「这台机器能不能建符号链接」这种环境事实**。「跳过并报错」则是功能倒退(带链接的目录就搬不动)。⇒ 本规范**规定现状**:跨设备回退按内容展开,同设备保持链接。⚠ 调用方若依赖「搬过去的东西仍是链接」,**必须自己确认同设备**。偏差 D13-051。

**`WALK_DIR` 的 `pattern` 是 glob**:`*` **不跨分隔符**、`**` 跨目录、`?` 匹配单个字符。pattern **含 `/`** 时匹配整条相对路径,**否则只匹配文件名**。缺省 = 全中。

**字节形状**取 **`ARRAY[INTEGER]`**,与 `std.encode::RANDOM_BYTES` **同款** —— 它已被定为「本语言唯一无损字节载体」(§11.4):`STRING` 是 UTF-8,装不了任意字节,硬塞只能**有损替换**成 `U+FFFD`,而本规范不提供有损替换。**范围**:这是给**小**二进制用的(配置、图标、密钥材料片段);**大载荷的正解是让它待在文件里**。

⚠⚠ **写得出 ≠ 读得回** —— 两个成员的档位**不同**:`WRITE_BYTES` 一次最多写 **64 MiB**,而 `READ_BYTES` 一次最多读 **1 MiB** ⇒ **大于 1 MiB 的文件是 `WRITE_BYTES` 写得出来、`READ_BYTES` 读不回来的**。⚠ 这**不是缺陷**,是两个成员各自的策略上限;**任何 > 1 MiB 的写上限都无法建立「写出来的读得回来」**(该方向性契约只有把写上限压到 ≤ 1 MiB 才成立,而那会**删掉「写大文件」这个能力** —— `WRITE_BYTES` 是本语言**唯一**能写出文件内容的成员)。⇒ **大载荷的正解是让它待在文件里**:压缩 / 拷贝都在 R2 侧走字节流(`std.compress`),绕语言值搬一趟会重新踩上解释器侧数组构建的超线性证据。⚠ **流式写 / 分段读成员本册不做** —— 它们要动的是成员面,属另立项。

### 2.3 失败口径

本章有两套失败口径,这是有意的:

- v0.3 的三个原生成员(`READ_FILE` / `WRITE_FILE` / `EXISTS`)用**原生诊断**口径(终止运行,`E0060`–`E0062`),它们**冻结不许动**。
- 路径族与目录 / 字节族用 **`ERR` 值**口径:载荷是 `[kind: "FsError", op: <成员名>, reason: <路径与 OS 原因>]`。理由是「目录不存在」「已存在」是**可预期的运行期结果**,调用方多半要 `IS_ERR` 分支;做成终止整个运行的诊断,调用方连分支都写不了。
- **元数 / 类型错**两套口径一致,都是 `E0022` / `E0030`。

**相对路径按进程 CWD 解析**:程序的 base dir(用于 `IMPORT` 解析相对模块路径)**不影响**文件操作 —— `LIST_DIR(".")` 列的是**当前工作目录**。推论:**测试里做文件操作必须用绝对路径**,否则会把文件写进工作树。

### 2.4 示例

```wlwl
IMPORT("wlwl:std.fs", ["PATH_JOIN", "PATH_DIR", "PATH_BASE", "PATH_EXT", "PATH_NORMALIZE"]);
PRINT(PATH_JOIN("a", "b", "c"));          // a/b/c
PRINT(PATH_DIR("a/b/c.txt"));             // a/b
PRINT(PATH_BASE("a/b/c.txt"));            // c.txt
PRINT(PATH_EXT("a/b/c.tar.gz"));          // gz
PRINT(PATH_NORMALIZE("a/./b/../c//d"));   // a/c/d
```

### 2.5 设计依据

本章的路径族、目录族与字节族来自 **ADDENDUM-06**(2026-10-08 裁决);字节形状与 `addendum-07` §3.2 **复用**同一条裁决,不自定第二套表示。缺陷登记:D13-044(出参分隔符)、D13-045(递归途中子目录打不开)、D13-046(`MOVE` 跨设备回退的环)、D13-047(`READ_BYTES` 上限)、D13-060(`follow = TRUE` 语义)、D13-019(debug 环境变量门控)。

## 3 `std.json` — JSON

解析、序列化、低层词法层、路径族与 schema 校验。**本章内部存在两套失败口径**(§3.3),这是规格的一部分,不是待清理的债。

### 3.1 成员表

| 签名 | 语义 | 失败 | 层 |
|---|---|---|---|
| `STRINGIFY(x) -> STRING` | 序列化为 JSON;字典键按**字典序**输出;`NULL` / 布尔 / 数字 / 字符串 / 数组 / 字典自然映射 | 失败产生 `E0071` | R2 |
| `PARSE(s) -> v` | 解析 JSON;对象得到**键按字典序排列**的字典(见 §3.2) | 失败产生 `E0070` | R2 |
| `JSON_TOKENIZE(s) -> ARRAY[DICT]` | **纯词法层**:切出 token 流,每个是 `{kind, start, end, text}`。`kind` ∈ `lbrace` `rbrace` `lbracket` `rbracket` `colon` `comma` `string` `number` `true` `false` `null`。**不看结构** —— 故 `[}` 在这里合法(它不是词法错误),结构由 `JSON_VALIDATE` 管 | 元数 / 类型错 → `E0022` / `E0030` | R2 |
| `JSON_LOCATE(s, offset) -> DICT` | 由**码点偏移** `offset` 求 `{line, column, token_start}`。`token_start` 是**包含该偏移的 token** 的起点;空输入返 `NULL`。**越界不报错**,返该位置附近的行列 | 元数 / 类型错 → `E0022` / `E0030`;负偏移 → `E0030` | R2 |
| `JSON_VALIDATE(s) -> OK \| ERR` | 只验**语法**(词法 + 结构 + 嵌套深度 ≤ **128** + 数值须在 `f64` 范围内),**不看 schema** | 语法错 → `ERR([kind: "JsonError", op, offset, line, column, reason])` | R2 |
| `JSON_GET(doc, path) -> v` | 按**路径数组**取值;**缺失返 `NULL`,不报错** | 路径形状错 → `E0030` | R2 |
| `JSON_SET(doc, path, value) -> v` | 返回**新**文档(不可变语义);缺失的中间容器**就地建出来**;数组下标越界 ⇒ **追加** | 路径形状错 → `E0030`;穿过标量的类型冲突 → `ERR([kind: "JsonError", op, path, reason])` | R2 |
| `JSON_DELETE(doc, path) -> v` | 删**整段**(对象键 / 数组元素);路径不存在 ⇒ **幂等返原文档** | 路径形状错 → `E0030`;空路径 → `ERR` | R2 |
| `JSON_MERGE(a, b) -> v` | 对象**递归深合并**,数组**整体替换**;`b` 侧是标量而 `a` 侧是容器时 `b` 赢 | — | R2 |
| `JSON_CHUNK_ITEMS(doc, size?) -> ARRAY[ARRAY]` | 把**顶层数组**切块;`size` 缺省 **1** | 非数组 / `size` < 1 → `E0030` | R2 |
| `JSON_CHECK(doc, schema) -> OK \| ERR` | schema 闭合子集校验;**`doc` 收 `STRING`,不是已解析的值**(见 §3.4) | schema 形状错 → `E0030`;数据不合 schema → `ERR([kind: "JsonError", op, path, offset, line, column, reason])` | R2 |

### 3.2 值形状与坐标系

**路径形状**是 `ARRAY[INTEGER | STRING]`:字符串段是对象键,整数段是数组下标(负数从尾部计数,与 `SLICE` 同口径)。**元素只接受 `STRING` 与 `INTEGER`**;别的(`FLOAT` / 布尔 / `NULL` / 嵌套容器)一律 `E0030` —— 路径是**寻址**,不是数据。

**`PARSE` 的字典键序是字典序,不是文档序**。解析器的对象表示是有序映射,键按**字典序**排列;`STRINGIFY` 同样按键的字典序输出。⇒ **JSON 对象在本语言里不是有序映射**:需要保留原始键序的调用方**必须**自己承担(在 `ARRAY` 里带序)。`JSON_SET` / `JSON_DELETE` / `JSON_MERGE` 在这个有序映射上操作,新键追加在末尾。

**`JSON_MERGE` 的两条口径逐字写死**(含糊的 merge 是配置事故的常见源):

- **对象深合并** ⇒ `{"a":{"x":1}}` 合 `{"a":{"y":2}}` 得 `{"a":{"x":1,"y":2}}`,**不是**顶层替换(那样会把 `x` 悄悄丢掉);
- **数组整体替换** ⇒ `{"a":[1]}` 合 `{"a":[2,3]}` 得 `{"a":[2,3]}`,**不拼接**(拼接不可逆,且与「merge 是覆盖语义」的直觉相反)。

**`JSON_CHUNK_ITEMS` 不是流式解析**:块直接从**已解析**的数组里切,不经过 `STRINGIFY` / `PARSE`。能保证的是**不重复解析**;总拷贝量仍是 O(n)。

### 3.3 失败口径

- 旧两成员(`PARSE` / `STRINGIFY`)失败产生 **`E0070` / `E0071` 诊断**(中止整个运行),它们是 v0.3 时代的遗留且**冻结不许动**。
- 低层词法三成员(`JSON_TOKENIZE` / `JSON_LOCATE` / `JSON_VALIDATE`)与路径族失败返 **`ERR` 值**,载荷按 `[kind, op, reason]` **加**位置三件套或 `path`。
- 照 Go `encoding/json/v2` 的体例(新 API 落地、**旧行为原样保留**)。
- **元数 / 类型错仍是诊断**(`E0022` / `E0030`),不是 `ERR` 值 —— 调用方式错是程序员错误。

### 3.4 低层词法层的可观察约定

1. **偏移一律码点,不是字节。** `start` / `end` / `offset` / `token_start` 是**码点 0 基**;`line` / `column` 是 **1 基**,`column` 也是**码点列**。依据:本语言唯一的字符串切分原语 `SUB(s, start, len?)` 按**码点**(语言规范 §10.5),字节偏移用户根本用不了。
2. **`ERR` 载荷必须带 `offset`。** `JSON_VALIDATE` 给行列而 `JSON_LOCATE` 收偏移 —— 没有 `offset`,两个成员之间**转不过弯**,而本册不提供行列反算回偏移的函数。`line` / `column` 给人看,`offset` 给机器接下一棒。
3. **`offset` 指向最小「出错构造」的起点**,不是它内部某个字符。坏转义 `"\uZZZZ"` 报**反斜杠**的位置,前导零 `01` 报**整个数字**的起点。
4. **`path` 在低层三成员里不出现。** 它们只做语法,不看结构 ⇒ 没有路径可言;`path` 从 `JSON_CHECK` 与路径族成员开始才有。塞一个恒为 `NULL` 的 `path` 会让人误以为「路径是空的」而不是「此处无路径」。
5. **三处比 RFC / Python 更严**(与 `PARSE` 对齐,不与它们对齐):孤立代理(`"\uD800"` / `"\uDC00"` / 换序的代理对)一律拒;嵌套深度上限 **128**;`1e400` 这类溢出 `f64` 的数字拒。

### 3.5 `JSON_CHECK` 的 schema 闭合子集

**`doc` 收 `STRING`,不是已解析的值** —— 这是有意偏离,理由必须写清楚:同一个值可以由任何一段文本产生,值里**没有位置可指**。当位置是伪的,比没有位置更坏。

| 关键字 | 含义 |
|---|---|
| `type` | `object` / `array` / `string` / `number` / `integer` / `boolean` / `null`;**可写多个**(任一匹配即过);缺省不约束 |
| `required` | 字符串数组;每个都必须存在 |
| `properties` | 字典,键 → 子 schema |
| `items` | 子 schema;对**数组的每个元素**生效 |
| `enum` | 允许值的数组 |
| `minimum` / `maximum` | 数值边界(含端点) |
| `minLength` / `maxLength` | 字符串长度边界(**按码点数**,不是字节数) |

**schema 侧的嵌套深度上限也是 ≤ 128**,与 `JSON_VALIDATE` 的文档侧同一口径。两侧必须同口径,另立一个数会让两者对「同一份深层文档」的接受集**分叉**。超深的形态是**原生诊断 `E0030`**(schema 本身写错了),与「值不合 schema → `ERR` 值」不是同一条路。偏差 D13-021。

**不支持的关键字一律报 `E0030`**,且报错的**消息会把全部合法键一并列出来** —— 半张表比没有更糟。

**数据不符 schema → `ERR([kind: "JsonError", op, path, offset, line, column, reason])`**。这里**既有 `path` 又有 `line` / `column`** —— 与低层三成员(有行列、无 path)和路径族(有 path、无行列)都不同。违规只报**第一条**(走文档序,类型不对时优先报类型)。

**一条有意分歧**:`type: "integer"` 只认 wlwl 的 **`INTEGER`**,**不认 `FLOAT`** —— `PARSE("2.0")` 给的是 `FLOAT`。JSON Schema 的 `integer` 指「没有小数部分的值」,本册取**严**的一支:本语言的类型系统**记得**原来是不是整数,抹平就丢掉了本仓比 Python 多出来的那份信息。

### 3.6 示例

```wlwl
IMPORT("wlwl:std.json", ["PARSE", "STRINGIFY", "JSON_GET", "JSON_SET", "JSON_MERGE", "JSON_CHUNK_ITEMS"]);
LET(doc, PARSE("{\"a\":{\"b\":1},\"list\":[10,20,30]}"));
PRINT(JSON_GET(doc, ["a", "b"]));            // 1
PRINT(JSON_GET(doc, ["a", "missing"]));      // NULL
PRINT(JSON_SET(doc, ["a", "c"], 9));         // [a: [b: 1, c: 9], list: [10, 20, 30]]
PRINT(JSON_MERGE(PARSE("{\"a\":{\"x\":1}}"), PARSE("{\"a\":{\"y\":2}}")));  // [a: [x: 1, y: 2]]
PRINT(JSON_CHUNK_ITEMS(PARSE("[1,2,3,4,5]"), 2));  // [[1, 2], [3, 4], [5]]
```

### 3.7 设计依据

低层词法层与路径族来自 **addendum-09**(W-02 词法层 / W-04 路径族);`JSON_CHECK` 的 schema 闭合子集见同批 §2.3;缺陷登记 D13-021(schema 侧深度上限)。

## 4 `std.format` — 格式化

导出与全局内建 `FORMAT` 同一函数(**冻结例外**,见 §0.8)。模板文法、`{N}` / `{name}` 混用规则与 `E0039` 见语言规范 §10.7 —— 本成员是全局内建的命名空间视图,语义以语言规范为准。

| 签名 | 语义 | 失败 | 层 |
|---|---|---|---|
| `FORMAT(template, args...) -> STRING` | `{N}` 以第 `N` 个实参的 `STR` 渲染替换;`{name}` 以首个实参为字典按键取值;两类占位符可混用。未匹配的 `{...}` 原样保留 | 模板畸形(孤立的 `{`)→ `E0039` | R2 |

### 4.1 `RESULT` 的渲染:两条路径**刻意不同**

「同一函数」这句话在 `RESULT` 值上**不成立**,且这不是缺陷:

| 路径 | `FORMAT("{0}", OK(1))` | 机制 |
|---|---|---|
| 全局内建 `FORMAT` | `OK(1)` | 走语言侧 display 路径,`RESULT` 按其字面构造形态渲染 |
| 成员 `wlwl:std.format` 的 `FORMAT` | `1` | 经 std 边界,`OK(v)` 在边界解包为 `v` |

两条路径分属 eval 侧与 std 边界两处实现,统一它们是一次**行为变更**而非文档债。裁决:**保留差异,只把口径写清楚**。

选型口径:需要「把这个 `RESULT` 原样打出来给人看」用全局内建;需要「把 `RESULT` 里的值填进模板」用成员路径。`ERR` 值的渲染不可观察 —— 调用边界即短路传播,到不了转换层。

### 4.2 示例

```wlwl
IMPORT("wlwl:std.format", ["FORMAT"]);
PRINT(FORMAT("{0}-{1}", 1, "a"));            // 1-a
PRINT(FORMAT("{name}={v}", ["name": "x", "v": 2]));  // x=2
```

### 4.3 设计依据

`RESULT` 渲染的两条路径来自 **D12-003** 裁决:保留差异、只写清口径,并由契约用例分别钉住两个输出,防止后续实现「好心统一」。

## 5 `std.collection` — 集合套件

**27 个成员**:14 个纯 wlwl(R1),13 个走 R2 内核。除注明外成员都是**非破坏性**的(语言规范 §10.1);成员回调抛出的 `ERR` 使整个调用按语言规范 §8.2 传播;`arr` 实参须为 `ARRAY`、回调 `f` 须为函数值(违者 `E0030`)。

| 签名 | 语义 | 失败 | 层 |
|---|---|---|---|
| `MAP(arr, f) -> ARRAY` | `[f(v), ...]` | 回调 `ERR` 按 §8.2 传播 | R2 |
| `FILTER(arr, f) -> ARRAY` | 保留 `f(v)` 为真的元素 | 同上 | R2 |
| `REDUCE(arr, f, init) -> v` | 左折叠;空数组返回 `init` | 同上 | R1 |
| `SORT(arr)` / `SORT(arr, cmp)` / `SORT_BY(arr, key) -> ARRAY` | 升序排序;`cmp(a, b)` 返回 `BOOLEAN` 表示 `a` 排在 `b` 前(降序传 `FUN((a, b), >(a, b))`);`SORT_BY` 以 `key(v)` 为排序键 | `cmp` 返非 `BOOLEAN` → `E0030` | R1 |
| `RANGE(n)` / `RANGE(start, end)` / `RANGE(start, end, step) -> ARRAY` | 整数区间 `[start, end)`,步长 `step` | `step = 0` → `E0038`;非整数 → `E0030` | R2 |
| `ANY(arr, f?)` / `ALL(arr, f?) -> BOOLEAN` | 任一 / 全部为真(缺省 `f` 时按真值);`ALL([])` 为 **TRUE**(空集真) | 回调 `ERR` 按 §8.2 传播 | R1 |
| `FIND(arr, f) -> v` | 首个满足者,无则 `NULL` | 回调 `ERR` 按 §8.2 传播 | R1 |
| `ZIP(a, b) -> ARRAY` | 配对至较短者:`[[a0, b0], ...]` | — | R2 |
| `ENUMERATE(arr) -> ARRAY` | `[[0, v0], [1, v1], ...]` | — | R2 |
| `TAKE(arr, n)` / `DROP(arr, n) -> ARRAY` | 前 n / 去前 n | `n` 非非负整数 → `E0030` | R1 |
| `FLAT(arr) -> ARRAY` | 展平一层 | — | R2 |
| `UNIQ(arr) -> ARRAY` | 按 `==` 去重,保留首现 | — | R2 |
| `GROUP_BY(arr, key) -> DICT` | 按 `key(v)` 分组,组为按首现序的字典 | 回调 `ERR` 按 §8.2 传播 | R2 |
| `JOIN(arr, sep) -> STRING` | 字符串化后以 `sep` 连接 | `sep` 非 `STRING` → `E0030` | R2 |
| `CHUNK(arr, n) -> ARRAY` | 定长分块,**尾块可短**;`n > LEN(arr)` 时得一块;空数组得 `[]` | `n < 1` → `E0030` | R2 |
| `WINDOW(arr, n) -> ARRAY` | 步长 1 的滑动窗口,共 `LEN(arr) - n + 1` 个;`n > LEN(arr)` 时得 `[]`(与 `CHUNK` 方向相反) | `n < 1` → `E0030` | R2 |
| `DEDUP_BY(arr, key) -> ARRAY` | 按 `key(v)` 去重,**保序**(留首现);键按 `STR` 渲染,故 `1` 与 `"1"` 撞键 | 回调 `ERR` 按 §8.2 传播 | R2 |
| `MIN_BY(arr, key)` / `MAX_BY(arr, key) -> v` | 键最小 / 最大对应的**元素**(不是键);空数组得 `NULL` | 回调 `ERR` 按 §8.2 传播 | R1 |
| `SUM(arr)` / `PRODUCT(arr) -> v` | 求和 / 求积;整数 / 浮点按语言规范 §2.2 提升;空数组分别得 `0` / `1`(单位元) | 非数值元素 → `E0030` | R1 |
| `FOLD_RIGHT(arr, f, init) -> v` | 右折叠 `f(a0, f(a1, ... f(an, init)))`;**参数顺序与 `REDUCE` 一致** | 回调 `ERR` 按 §8.2 传播 | R1 |
| `POSITION(arr, pred) -> INTEGER` | 首个满足者的 **0 起**下标,无则 `-1`(与 `FIND` 的「给元素」是两件事) | `pred` 返非 `BOOLEAN` → `E0030` | R1 |
| `KEY_BY(arr, key) -> DICT` | 同键**后者覆盖前者**(`GROUP_BY` 则是收成数组);键按 `STR` 渲染 | 回调 `ERR` 按 §8.2 传播 | R2 |

### 5.1 排序口径

**混合类型排序的准确口径**:不可比的元素**不报错**;排序分「可比组」进行,**各组内稳定**,而**组与组之间的相对次序由择序策略决定,不保证与原序一致**。实测:`SORT([1, "a", 2])` → `[1, a, 2]`(恰好与原序同形,故容易被当成「保留原序」的例子),但 `SORT([2, "a", 1])` → `[1, 2, a]`、`SORT([3, "a", 1, "b", 2])` → `[1, 2, 3, a, b]` —— `"a"` 相对 `1` 的位置变了。行为确定且无害(实现是选择排序 +「不可比即不小于」),但本规范**不**承诺它没承诺的东西。

### 5.2 十个容易记反的取舍

1. **`MIN_BY` / `MAX_BY` 用 `SORT_BY` 的比较原语做单遍扫描,不是去排序。** 排序口径与 `SORT_BY` **逐字相同** —— 因为用的是同一个函数,而不是「照着它的规则再写一遍」;不可比时该原语返 `FALSE`,与 `SORT_BY` 一致。同一输入上 `MIN_BY` 与 `SORT_BY` 的首元素相等,这条由契约用例钉住。
2. **尺寸实参 `n < 1` 报 `E0030`,不照 `TAKE` 那套「负数给 `[]`」。** `TAKE` 的负数语义是「取空」,本身说得通;而 `CHUNK` / `WINDOW` 的 `0` 会让循环**不终止**,负数几乎总是调用方算错了。悄悄给 `[]` 的话,「我的数组怎么空了」比一条诊断难查。
3. **`CHUNK` 与 `WINDOW` 在 `n > LEN(arr)` 时方向相反**:前者给一块、后者给零块。这是定义决定的,不是笔误。
4. **`DEDUP_BY` 的键按 `STR` 渲染**,与同模块 `GROUP_BY` / `KEY_BY` 同一口径 ⇒ 整数 `1` 与字符串 `"1"` 撞键。
5. **`FOLD_RIGHT` 是迭代的,不是递归的。** 递归深度 = 数组长度,十万元素就是十万层,直接爆栈;从尾往头累加一遍即可。
6. **`SORT` / `SORT_BY` 仍留 R1。** 归层要换 `Vec::sort_by`,而那会改**比较器调用次数**(R1 实测恰好 C(n,2) 次),对带副作用的比较器是可观察行为。
7. **`RANGE` 的整数计数溢出行为是饱和**(`i64::checked_add`,`None` 即停),不是语言规范 §2.2 的 `E0035` 抛错。该形态随层归属回摆过一次,现行为以本条为准;可达性需循环约 9.2×10^18 次,实际不可达。

### 5.3 性能口径

建数组类成员(`MAP` / `FILTER` / `FLAT` / `UNIQ` / `ENUMERATE` / `GROUP_BY` / `JOIN` 等)曾因 R1 的 `PUSH` 复制而呈平方级;归 R2 后每元素成本随规模**下降**,平方级消除。仍呈平方级的是 `SORT` / `SORT_BY`,以及走 `DICT` 的 `DEDUP_BY` / `GROUP_BY` / `KEY_BY` —— 后者的来源是 `DICT` 的表示是线性扫描的 `Vec<(Value, Value)>`(逐项深比较),与 `PUSH` 无关。`UNIQ` 归层后仍平方级:它另有一笔线性成员查找的成本,`PUSH` 重建那笔消不掉。根因修复(解释器侧写时复制 / 结构共享)记为演进项。

`WINDOW` 的输出本身就有 `LEN(arr) - n + 1` 个窗口 × 每个 `k` 个元素 = Θ(n·k);它不是「实现没写好」,是**这个成员的定义本身**要产出 Θ(n·k) 个元素。要线性窗口就得惰性化,那是演进方向。

### 5.4 示例

```wlwl
IMPORT("wlwl:std.collection", ["MAP", "FILTER", "RANGE", "CHUNK", "FOLD_RIGHT", "POSITION", "KEY_BY", "GROUP_BY"]);
PRINT(MAP([1, 2, 3], FUN((x), *(x, 10))));     // [10, 20, 30]
PRINT(FILTER([1, 2, 3], FUN((x), >(x, 1))));    // [2, 3]
PRINT(RANGE(0, 5));                            // [0, 1, 2, 3, 4]
PRINT(CHUNK([1, 2, 3, 4, 5], 2));              // [[1, 2], [3, 4], [5]]
PRINT(FOLD_RIGHT([1, 2, 3], FUN((a, b), +(a, b)), 0));  // 6
PRINT(POSITION([1, 2, 3], FUN((x), >(x, 2)))); // 2
PRINT(KEY_BY([1, 2], FUN((x), %(x, 2))));       // [1: 1, 0: 2]
PRINT(GROUP_BY([1, 2, 3], FUN((x), %(x, 2))));  // [1: [1, 3], 0: [2]]
```

### 5.5 设计依据

`RANGE` 归 R2 是 v0.11 M5 的**实测裁决**:R1 版每元素成本超线性(10 000 元素 3.6 s、40 000 元素 86 s),使语言规范 §6.6 的「100 万次简单循环 < 30 s」符合性负载跑不完。层归属变更不算破坏性变更(ADR-0021 §0.2)—— 成员面、签名、语义、诊断一字未变。偏差 D11-012。其余十个成员随 v0.11.2 M3 追加,当时**同为 R1**。平方级成本的分型证据与裁决点见 [`ADR-0025`](../adr/0025-array-quadratic-cost-routes.md) Q1。

## 6 `std.str` — 字符串扩展

纯 wlwl 实现。只补全局内建(语言规范 §10.5)没有的能力;**不重导出**任何全局内建。索引口径与 `SUB` 一致(码点;起点同 `SUB` 的 `start` —— 含**负索引自尾部计数**,所以 `CHAR_AT("abc", -1)` 是 `"c"`,不是越界)。

| 签名 | 语义 | 失败 | 层 |
|---|---|---|---|
| `JOIN(arr, sep) -> STRING` | 元素经 `STR` 渲染后以 `sep` 连接;空数组得 `""` | — | R1 |
| `SPLIT_LINES(s) -> ARRAY` | 按 `\n` 切分并去除行尾 `\r`;尾随换行不产生空尾元素;空串得 `[]`;无换行得 `[s]` | — | R1 |
| `CHAR_AT(s, i) -> STRING` | 取第 `i` 个码点,等价于 `SUB(s, i, 1)` | 越界同 `SUB` | R1 |
| `COUNT(s, sub) -> INTEGER` | `sub` 的**非重叠**出现次数(切成 n+1 段则非重叠出现次数正是 n) | `sub` 为空串 → `E0030` | R1 |
| `QUOTE(s) -> STRING` | 双引号包裹;内部 `"` 与 `\` 转义为 `\"`、`\\`;`\n`、`\t`、`\r` 分别转义;其余字符原样 | — | R1 |
| `INDEX_OF(s, sub, from?) -> INTEGER` | `sub` 首次出现的**码点**下标(0 起),无则 `-1`;`from` 把起点推到后面(负值钳到 0),默认 0 | `sub` 为空串 → `E0030` | R1 |
| `CONTAINS_SUB(s, sub) -> BOOLEAN` | `sub` 是否出现 | `sub` 为空串 → `E0030` | R1 |

**空 `sub` 的口径统一成「报错」**:`INDEX_OF` 与 `CONTAINS_SUB` 沿用 `COUNT` 的既有规则,而不是各自发明一条(比如「空串恒匹配」)。同一模块里三个成员对空串给三种答案,调用方只能靠「读过哪份文档」来判断。

**下标是码点不是字节**,与 `CHAR_AT` / `SUB` / `CODEPOINTS` 同口径,拿到后可直接 `SUB(s, i, n)` 切出那段。`from` 存在的理由:没有它就只能从 0 开始反复扫全文找后续出现,那是 O(n·m)。

### 6.1 示例

```wlwl
IMPORT("wlwl:std.str", ["JOIN", "SPLIT_LINES", "CHAR_AT", "COUNT", "QUOTE", "INDEX_OF", "CONTAINS_SUB"]);
PRINT(JOIN(["a", "b"], "-"));          // a-b
PRINT(SPLIT_LINES("a\nb\r\nc"));       // [a, b, c]
PRINT(CHAR_AT("abc", 1));              // b
PRINT(COUNT("aaaa", "aa"));            // 2
PRINT(QUOTE("a\"b\n"));                // "a\"b\n"
PRINT(INDEX_OF("hello", "ll"));        // 2
PRINT(CONTAINS_SUB("hello", "ell"));   // TRUE
```

### 6.2 设计依据

`INDEX_OF` / `CONTAINS_SUB` 随 v0.11.2 M2 追加,补的是**位置**与**布尔判定**两件事:`std.str::COUNT` 早已给出非重叠计数,缺的只有这两项。⚠ 定位缺口时曾用探针 `PRINT(FIND("hello","el"))` 报 `E0020` 作依据 —— 但 `FIND` 是 `std.collection` 的**数组**成员,探的是错的名字。

## 7 `std.math` — 数学基础

**61 个成员**:R1 门面 + R2 内核(浮点内核、任意精度十进制、精确分数、近似比较、描述统计)。

**约定**:算术提升与溢出行为沿语言规范 §2.2;域违例返回 `ERR(["kind": "DomainError", ...])`;实参类型错按语言规范报 `E0030`。`MIN` / `MAX` / `CLAMP` 的**返回值类型**也按 §2.2 提升:实参中有任一 `FLOAT` 则返回 `FLOAT`(`CLAMP` 看**三个**实参)。

`FLOOR` / `CEIL` / `ROUND` / `TRUNC` 归门面但**不是** `INT`:`INT` 按 §2.2 向零截断,而下 / 上取整与「半数远离零」都不同于它,故实现走 `INT` + 符号修正;超出 `±2^53` 的 `FLOAT`(本身已是整数)与 `NaN` / `±inf` 原样返回。

### 7.1 基础与浮点成员

| 签名 | 语义 | 失败 | 层 |
|---|---|---|---|
| `ABS(x) -> v` | 绝对值,返回与实参同类型 | `INTEGER` 下界 → `E0034` | R1 |
| `MIN(a, b)` / `MAX(a, b) -> v` | 二元最值;混合整浮按 §2.2 提升 | — | R1 |
| `FLOOR(x)` / `CEIL(x) -> v` | 下 / 上取整;`INTEGER` 恒等;`FLOAT` 返回 `FLOAT`;`NaN` / `±inf` 原样 | — | R1 |
| `ROUND(x) -> v` | 四舍五入(半数远离零);其余同上 | — | R1 |
| `TRUNC(x) -> v` | 向零取整;形态同 `FLOOR` / `CEIL`(`INTEGER` 恒等、`FLOAT` 返回 `FLOAT`) | 非数值 → `E0030` | R1 门面 + R2 浮点内核 |
| `CLAMP(x, lo, hi) -> v` | 夹取到 `[lo, hi]`;类型提升同 `MIN` / `MAX` | `lo > hi` → `ERR([kind: "DomainError"])` | R1 |
| `SQRT(x) -> FLOAT` | 平方根;`-0.0` 得 `-0.0` | `x < 0` → `ERR([kind: "DomainError"])` | R2 |
| `POW(a, b) -> FLOAT` | 幂;两参提升为 `FLOAT` | `0` 的负次幂、负底的非整数指数 → `ERR([kind: "DomainError"])` | R2 |
| `PI` / `E` | `FLOAT` 常量 | — | R1 |
| `LN(x)` / `LOG2(x)` / `LOG10(x) -> FLOAT` | 对数。**`x = 0` 不报错,返回 `-inf`**(数学上正确,与 C / Python / Go 一致) | `x < 0` → `ERR([kind: "DomainError"])` | R2 |
| `EXP(x) -> FLOAT` | 自然指数;全定义域;上溢给 `+inf` | — | R2 |
| `SIN` / `COS` / `TAN(x) -> FLOAT` | 三角(弧度制);全定义域 | — | R2 |
| `ASIN(x)` / `ACOS(x) -> FLOAT` | 反三角;定义域 `[-1, 1]` | `ABS(x) > 1` → `ERR([kind: "DomainError"])` | R2 |
| `ATAN(x) -> FLOAT` | 反正切;全定义域 | — | R2 |
| `ATAN2(y, x) -> FLOAT` | 按象限定义的反正切;**全定义域**,`(0, 0)` 得 `0.0` | — | R2 |
| `SINH` / `COSH` / `TANH(x) -> FLOAT` | 双曲;全定义域;上溢给 `±inf` | — | R2 |
| `SIGN(x) -> INTEGER` | 符号 `-1` / `0` / `1`。**返回类型恒为 `INTEGER`**,不随实参提升 | — | R1 |
| `DIV_CEIL(a, b) -> INTEGER` | 向上取整的整数除法;**只收整数**;`b = 0` 是域违例 | `b = 0` → `ERR([kind: "DomainError"])` | R1 |
| `GCD(a, b)` / `LCM(a, b) -> INTEGER` | 辗转相除;**只取绝对值**(`GCD(-4, 6) = 2`);任一为 0 时 `LCM` 得 0;`LCM` 先除后乘以避溢出 | 取绝对值时实参为 `INTEGER` 下界 → `E0034`(中止整个运行);`LCM` 的中间乘积溢出 i64 → `E0035` | R1 |
| `IS_SQRT(n) -> INTEGER` | 整数平方根(向下取整)—— **返回平方根本身,不是布尔谓词**,与 `IS_OK` / `IS_ERR` 家族的 `IS_` 前缀无关 | `n < 0` → `ERR([kind: "DomainError"])` | R1 |
| `POW_MOD(base, exp, mod) -> INTEGER` | 模幂(平方-乘,`O(log exp)`);**`mod` 可以是 `2^61-1` 这样的密码学素数**;余数按**欧几里得余数**(与模数同号),不是 Rust `%` 的「跟左操作数走」 | `mod = 0` 或 `exp < 0` → `ERR([kind: "DomainError"])` | R2 |

### 7.2 任意精度十进制(`DECIMAL` 族)

**值形状是公开契约,字段名变更即 breaking。**

```json
{"sign": INTEGER, "digits": STRING, "scale": INTEGER}
```

表示 `sign × digits × 10^-scale`。规范形式:

- `digits` **无前导零**(零为 `"0"`),只含 ASCII 数字;
- `scale >= 0`;
- `sign ∈ {-1, 1}`,且**零恒为 `{1, "0", 0}`** —— 消灭 `-0` 那种两个表示同一数的状态。

**`scale` 是有意义的**,不是规范化掉的:`DECIMAL("1.00")` 渲染回 `"1.00"`。金额场景靠它保留「几位」这个业务事实。

**本族不接受语言层不存在的类型**:`INTEGER` / `FLOAT` 是 `i64` / `f64`,金额大到 `f64` 的 `2^53` 之后就会被静默丢精度 —— 实测 `+(9007199254740993.0, 1.0)` 得 `9007199254740992.0`,而 `DECIMAL` 给 `9007199254740994`。这正是本族存在的理由。

| 签名 | 语义 | 失败 | 层 |
|---|---|---|---|
| `DECIMAL(literal) -> DICT` | 从字面量构造。接受 `[+-]?digits[.digits]`,含 `".5"` 与 `"-0"` | 非法字面量 → `E0030` | R2 |
| `DECIMAL_ADD(a, b)` / `DECIMAL_SUB(a, b)` / `DECIMAL_MUL(a, b) -> DICT` | 精确算术;加减结果的 `scale` = 两者的较大者,乘法结果 `scale` = 两者之和 | 非法 `DICT` → `E0030` | R2 |
| `DECIMAL_DIV(a, b, scale) -> DICT` | 精确到 `scale` 位,舍入恒 `HALF_EVEN` | **`scale` 必填**;`b = 0` → `ERR([kind: "DomainError", "op": "DIV"])` | R2 |
| `DECIMAL_CMP(a, b) -> INTEGER` | `-1` / `0` / `1`,**按数值**比(`1.0` 与 `1.00` 等) | 永不失败 | R2 |
| `DECIMAL_TO_STRING(a) -> STRING` | 按 `scale` 渲染(`"1.50"` / `"-0.25"` / `"3"`) | 永不失败 | R2 |
| `DECIMAL_ROUND(a, scale, mode) -> DICT` | 按 `mode` 舍入到 `scale` 位 | **`mode` 无默认**;未知 → `E0030` | R2 |

**`scale` 必填是刻意的**:静默截断是精度谎言。调用方必须显式声明要几位。

**`mode` 无默认是刻意的**:银行家舍入与四舍五入是**业务决定**,给默认等于替业务方选了一个。`mode` 的取值是 `HALF_EVEN`(银行家)、`HALF_UP`(四舍五入)、`DOWN`(向零)、`UP`(远离零)、`FLOOR`(向 `-inf`)、`CEILING`(向 `+inf`)。**方向模式的差别只在负数上**:`FLOOR(-2.1) = -3` 而 `DOWN(-2.1) = -2`。

**非法 `DICT` → `E0030`,不修复、不报错成别的形状。** 用户能手工写出 `{"sign": 0, "digits": "abc", "scale": 0}` 这种非法状态 —— 这是**明码标价的妥协**,不是干净的形状。

**本族不含 `MEAN`** —— `std.collection::SUM` 加 `/` 已覆盖。

### 7.3 精确分数(`FRACTION` 族)

**值形状是公开契约,字段名变更即 breaking。**

```json
{"num": INTEGER, "den": INTEGER}
```

规范形式:`den > 0`、`gcd(|num|, den) = 1`(**约分到最简**)、**零恒为 `{0, 1}`**。

| 签名 | 语义 | 失败 | 层 |
|---|---|---|---|
| `FRACTION(n, d) -> DICT` | 构造并约分;分母负时**分子分母同时取反** | `d = 0` → `ERR([kind: "DomainError"])` | R2 |
| `FRACTION_ADD(a, b)` / `FRACTION_SUB(a, b)` / `FRACTION_MUL(a, b) -> DICT` | 精确算术,结果自动约分 | 溢出 → `ERR([kind: "OverflowError"])`;非法 `DICT` → `E0030` | R2 |
| `FRACTION_DIV(a, b) -> DICT` | 精确算术 | 除数为零 / 溢出 → `ERR` | R2 |
| `FRACTION_CMP(a, b) -> INTEGER` | `-1` / `0` / `1`。**永不失败**(交叉相乘用 `i128` 中转,不该因中间量溢出给出错误答案) | — | R2 |
| `FRACTION_TO_STRING(f) -> STRING` | `"3/4"` / `"-1/2"` / `"2"`(分母为 1 时不写分母) | — | R2 |
| `FRACTION_TO_DECIMAL(f, scale) -> DICT` | 转十进制,舍入恒 `HALF_EVEN` | **`scale` 必填**;超出 `0..=30` → `E0030` | R2 |

**分子分母是 `i64`,不是任意精度。** 分数连乘很容易撞 `i64` 上界,本族**一律在溢出发生前**检查并报 `ERR([kind: "OverflowError"])`,**绝不**让语言层的 `E0035` 冒出来 —— 那个是**中止整个运行**的,放进标准库成员里不可接受。若将来需要任意精度分数,正确做法是复用 §7.2 的数位编码而不是另起一套。

**未约分 / `den <= 0` / 零不是 `0/1` 的 `DICT` 一律 `E0030`,不修复。** 与 §7.2 同款:`DICT` 承载的固有缺陷,明码标价。

**`scale` 必填**:`1/3` 的十进制展开**不终止**,静默给一个截断值就是精度谎言。

### 7.4 近似比较

| 签名 | 语义 | 失败 | 层 |
|---|---|---|---|
| `APPROX_EQ(a, b, rel_tol, abs_tol) -> BOOLEAN` | `\|a - b\| <= max(rel_tol × max(\|a\|, \|b\|), abs_tol)` | 元数 4;非数值实参 → `E0030` | R2 |
| `IS_NAN(x) -> BOOLEAN` | `x` 是 `NaN` | 非数值实参 → `E0030` | R2 |
| `IS_INF(x) -> BOOLEAN` | `x` 是 `±inf` | 同上 | R2 |
| `SIGNIFICANT_DIGITS(x, n) -> BOOLEAN` | `x` 是否**恰好等于**它保留 `n` 位有效数字后的值 | `n` 超出 `1..=17` → `E0030` | R2 |

**两个容差都不给默认。** `0.1` 与 `0.1000000001` 该不该相等是**业务决定**;猜一个默认值等于替调用方选语义。

**`IS_INF` 补的是缺口,不是便利。** `==(x, x)` 对 `±inf` 为 **TRUE** —— 自等判据抓不到它,而本库引入本族之前既没有 `IS_INF` 也没有 `IS_NAN`。`NaN` 至少有 `x != x` 这个经典绕法。

**特殊值语义**(三者都必须是总函数,不能因特殊值崩掉):

| 情形 | `APPROX_EQ` | `SIGNIFICANT_DIGITS` |
|---|---|---|
| `NaN` 与任何值(含自己) | `FALSE` | `FALSE` |
| `±inf` 与**同号** `±inf` | `TRUE` | `FALSE` |
| `±inf` 与 `-∞` / 有限值 | `FALSE` | `FALSE` |
| `0.0` | 正常判据 | `TRUE`(零对任意 `n ≥ 1` 都「已经就位」) |

**`SIGNIFICANT_DIGITS` 取谓词读法**(与「比容差更直觉的**判定**」一致);返回「有效数字的**个数**」是另一个成员,且那不是判定。

### 7.5 描述统计

| 签名 | 语义 | 失败 | 层 |
|---|---|---|---|
| `MEDIAN(arr) -> v` | 排序后取中位;**偶数长度取中间两个的平均**(走 `f64`,整数 `/` 会截断) | `[]` → `NULL` | R2 |
| `MODE(arr) -> v` | 出现次数最多的值;**并列取先出现者**;全部只出现一次 ⇒ 无众数 | `[]` → `NULL` | R2 |
| `VARIANCE_S(arr)` / `VARIANCE_P(arr) -> v` | 样本(`n-1`) / 总体(`n`)方差,**显式分成两个成员** | `[]` → `NULL` | R2 |
| `STDDEV_S(arr)` / `STDDEV_P(arr) -> v` | 同上,取平方根 | `[]` → `NULL` | R2 |
| `PERCENTILE(arr, p) -> v` | `p ∈ [0, 100]`,**线性插值**(含端点) | `p` 越界 → `NULL` | R2 |
| `CORRELATION(xs, ys) -> v` | Pearson 相关系数;两数组须**等长** | 任一方向方差为零 → `NULL` | R2 |
| `LINEAR_FIT(xs, ys) -> DICT` | 最小二乘直线 ⇒ `{"slope": FLOAT, "intercept": FLOAT}` | 任一方向方差为零 → `NULL` | R2 |

**混装静默提升**:`ARRAY[INTEGER]` 与 `ARRAY[FLOAT]` 混装**不**报 `E0030`,与既有 `SUM` 同口径。收紧就会与 `SUM` 分裂,而分裂意味着其中一处是错的。

**「空」与「无定义」都返回 `NULL`,不用错误** —— 这条容易被读反:

| 输入 | 结果 | 为什么 |
|---|---|---|
| `[]` | `NULL` | 没有样本,与 `std.collection` §5 一致 |
| `[x]` 的**样本**方差 / 标准差 | `NULL` | 分母 `n-1 = 0`,**数学上无定义** |
| `[x]` 的**总体**方差 / 标准差 | `0.0` | 分母 `n = 1`,有定义 |
| 任一元素是 `NaN` / `±inf` | `NULL` | 见下 |
| `CORRELATION` / `LINEAR_FIT` 任一方向方差为零 | `NULL` | 相关系数 / 斜率无定义,**不是 0** |
| `PERCENTILE` 的 `p` 越界 | `NULL` | `p` 是**数据**不是参数错 |

**`NaN` 不渗进统计量。** 理由是 wlwl 没有浮点异常的传播约定(`/(0.0, 0.0)` 直接 `E1003`),让 `NaN` 渗进统计量会让调用方**看不出**结果不可信,而 `NULL` 是看得见的。

**`PERCENTILE` 的插值方式是可观察的**,故必须写明:同样一组数据,`p` 落在两个观测点之间时,线性 / 较低 / 较高 / 中点 / 哈德利五种方法给出**不同**的值。本规范取线性(含端点),等价于 NumPy `method="linear"` / Python `statistics.quantiles(method="inclusive")`。

**`CORRELATION` 在 `n = 2` 时恒为 `±1`** —— 两点永远共线,这是数学事实不是缺陷。

**本族不含 `MEAN`** —— `SUM` 加 `/` 已覆盖。

### 7.6 整数数学与模幂的四条取舍

1. **`TRUNC` 不是全局 `INT` 的别名,但差别只在界外。** 界内两者逐字相同(都是向零截断);`|x| >= 2^53` 时 `TRUNC` 原样返回,而 `INT(1e20)` 抛 **E0035 并中止整个运行**。一个取整函数因为边界中止程序,比返回边界值难用得多。非数值实参两者也分属两类:`TRUNC` 经门面的类型检查报 **E0030 诊断**,`INT("abc")` 返的是 `ERR([kind: ParseError])` **值**。
2. **`SIGN` 的返回类型恒为 `INTEGER`**,不按 §2.2 随实参提升。`ABS` 之所以「返回与实参同类型」,是因为它的取值范围要跟随实参;符号只有三个取值,给 `SIGN(1.0)` 返 `1.0` 只会让每个调用方多一次类型判断。
3. **`DIV_CEIL` 只收整数。** 浮点的向上取整是 `CEIL(/(a, b))`,语言自足;若 `DIV_CEIL` 悄悄接受 `7.0` 并提升,等于替调用方做了一个它没要求的取整决定。四个符号组合都必须对:`DIV_CEIL(7,2)=4`、`(-7,2)=-3`、`(7,-2)=-3`、`(-7,-2)=4` —— 规律是「有余数**且两数同号**才 +1」。
4. **`POW_MOD` 归 R2 而不是门面**,因为门面**算不出中间积会不会溢出**。平方-乘的中间值最大是 `m²`,而 `m` 可能是 `2^61 - 1`;内核用 `i128` 算中间积,于是大模数照常工作:实测 `POW_MOD(2, 61, 2305843009213693951)` = `1`。`IS_SQRT` 的二分上界取 `MIN(n, 3037000499)` 而不是 `n`,因为 `mid * mid` 在 `n = 2^62` 时会溢出 i64。

**`-0.0` 这一格怎么写出来**:`SQRT` 的实现**没有**丢符号,喂一个真负零返回的就是负零。但**源码里写不出负零字面量**:语言规范 §1.6 的 `-x` 语法糖在解析期被改写成 `-(0, x)`,而 IEEE 754 下 `0 - 0.0 = +0.0` ⇒ `SQRT(-0.0)` 里的 `-0.0` 其实是 `+0.0`,返回 `0.0` 是正确的。要拿到真负零只能绕开语法糖(`*(0.0, -(0, 1.0))`)。解析期保留符号属语言层议题,不在本册范围。

### 7.7 示例

```wlwl
IMPORT("wlwl:std.math", ["SQRT", "ROUND", "TRUNC", "CLAMP", "SIGN", "DIV_CEIL", "GCD", "IS_SQRT", "POW_MOD", "DECIMAL", "DECIMAL_ADD", "DECIMAL_TO_STRING", "FRACTION", "FRACTION_TO_STRING", "APPROX_EQ", "MEDIAN", "PERCENTILE", "LINEAR_FIT"]);
PRINT(SQRT(2.0));                 // 1.4142135623730951
PRINT(ROUND(2.5));                // 3.0
PRINT(TRUNC(-2.7));               // -2.0
PRINT(CLAMP(5, 0, 3));            // 3
PRINT(SIGN(-3));                  // -1
PRINT(DIV_CEIL(7, 2));            // 4
PRINT(GCD(-4, 6));                // 2
PRINT(IS_SQRT(17));               // 4
PRINT(POW_MOD(2, 61, 2305843009213693951));  // 1
PRINT(DECIMAL_TO_STRING(DECIMAL_ADD(DECIMAL("1.25"), DECIMAL("0.5"))));  // 1.75
PRINT(FRACTION_TO_STRING(FRACTION(3, 6)));    // 1/2
PRINT(APPROX_EQ(0.1, 0.1000000001, 0.001, 0.0));  // TRUE
PRINT(MEDIAN([1, 3, 2]));         // 2.0
PRINT(PERCENTILE([1, 2, 3, 4], 50.0));  // 2.5
PRINT(LINEAR_FIT([1, 2, 3], [2, 4, 6]));  // [slope: 2.0, intercept: 0.0]
```

### 7.8 设计依据

`DECIMAL` / `FRACTION` / 近似比较 / 描述统计四族随 v0.11.3 M7 追加(`ADR-0031` D1–D4);`DECIMAL` 的 `scale` 必填是 D1,`MEAN` 不做是 D2,混装静默提升是 D3,`SIGNIFICANT_DIGITS` 取谓词是 D4。**正确性证据**:契约期望值取自 **Python 的 `decimal` / `fractions` / `statistics` 模块**(默认精度 28、舍入 `ROUND_HALF_EVEN`),**不是实现输出** —— 由脚本在 Python 侧独立算一遍再落表。一处**有意分歧**:Python 的 `statistics.stdev([7])` 抛 `StatisticsError`,本册的 `STDDEV_S([7])` 返回 `NULL`。

## 8 `std.test` — 测试

R1 门面 + R2 原生内核(注册与计时)。

| 签名 | 语义 | 失败 | 层 |
|---|---|---|---|
| `TEST(name, body) -> NULL` | 注册零参测试体 | — | R1 门面 + R2 内核 |
| `ASSERT(cond, msg?)` | `cond` 按语言规范 §2.3 的**八个**假值判(`FALSE` / `NULL` / `0` / `0.0` / `""` / 空数组 / 空字典 / `NaN`);为真 → `OK(TRUE)`,否则 `ERR(E0046)` | — | R1 门面 + R2 内核 |
| `ASSERT_EQ(a, b)` / `ASSERT_NEQ(a, b)` | 相等 / 不等断言;成功 → `OK(TRUE)` | 失败为 `E0047` / `E0048` | R1 门面 + R2 内核 |
| `EXPECT_ERR(x)` | `x` 为 `ERR` → `OK(载荷)`;否则 `ERR(E0049)` | — | R1 门面 + R2 内核 |
| `RUN_TESTS() -> ARRAY` | 调用所有已注册体,返回记录数组;每条记录是字典,至少含 `name`(STRING)、`passed`(BOOLEAN)、`duration_ms`(INTEGER),并按结果附 `return_value` 或 `error` | — | R1 门面 + R2 内核 |

**约定**:测试体**应当**以 `ASSERT` 族断言收尾并以 `TRUE` 结束;体求值期间逃逸的 `ERR` 使该项 `passed = FALSE`。

**`ASSERT` 的假值集是八个**,不是两个 —— v0.10 及以前只把 `BOOLEAN(false)` 与 `NULL` 当假,`ASSERT(0)` / `ASSERT("")` / `ASSERT([])` 会通过(按 §0.8 申报的 breaking 变更)。

**测试体挂起时**:测试体若在求值中挂起(体是 `YIELD()` 所在的任务体,且挂起点在 `RUN_TESTS` 调用之内),`RUN_TESTS` **不产出记录**并把挂起信号原样交回调度器 —— 挂起不是「测试通过」也不是「测试失败」,因此**不得**被折算成其中任一个。判据是 `ERR` 与挂起的区别:前者是**值**,`RUN_TESTS` 可以捕获并记账;后者是**控制流信号**,按 `wlwl_value::StdFn` 契约(std 层不得消费)必须穿透。

### 8.1 示例

```wlwl
IMPORT("wlwl:std.test", ["TEST", "ASSERT", "ASSERT_EQ", "RUN_TESTS"]);
TEST("adds", FUN((), ASSERT_EQ(+(1, 2), 3)));
TEST("fails", FUN((), ASSERT(FALSE, "boom")));
LET(results, RUN_TESTS());
PRINT(LEN(results));                  // 2
PRINT(results[0].name, results[0].passed);  // adds TRUE
PRINT(results[1].name, results[1].passed);  // fails FALSE
```

### 8.2 设计依据

`ASSERT` 的八假值集是 v0.11 的**破坏性变更**(按 ADR-0023 §v0.x 第 2 条申报);挂起不折算成通过 / 失败的判据来自 D11-019。`RUN_TESTS` 的记录形状与语言规范 §10.10 一致。

## 9 `ai` — AI 推理(官方包)

**导入路径是 `ai:ask`**(不是 `wlwl:std.ai`)。`wlwl:std.ai` **不可解析** —— 不是空壳,空壳会让「搬没搬」看不出来。`wlwl:ai` 结构上不可行:`wlwl:` 是 std 的保留前缀,解析器对它开头的路径直接返回 `None`,于是它落进「裸模块名」分支、被当成文件名去找 `…\wlwl:ai.wll`,最终报 `E0040`。前缀只有 `ai:` / `agent:` 两种形态可用。

| 签名 | 语义 | 失败 | 层 |
|---|---|---|---|
| `ASK(model, prompt, opts?) -> STRING` | 单次推理;`opts` 是 `DICT` 或 `NULL` | 元数错 → `E0022`;类型错 → `E0030`;`E0080`–`E0083` | R2 |
| `EMBED(text, model?) -> ARRAY` | 嵌入向量(**四维**,实现侧固定);`model` 缺省取环境变量 `WLWL_AI_DEFAULT_MODEL` | 同上;`E0080` | R2 |
| `COMPLETE(context, language?, opts?) -> STRING` | 代码补全;`language` 缺省取 `WLWL_AI_DEFAULT_MODEL` | 同上;`E0080` | R2 |
| `ASK_STREAM(model, prompt, callback, opts?) -> STRING` | 流式推理(实现侧当前**整收集为单块**,返回整段文本) | 同上 | R2 |
| `ASK_ALL(prompts, opts?) -> ARRAY` | 多提示批量;`prompts` 是 `ARRAY[STRING]` | 同上 | R2 |

**错误码语义**:`E0080` 请求失败;`E0081` 协议错误;`E0082` 工具错误;`E0083` 上下文错误。模型名缺少 `provider/` 前缀产生 `W0052`(语言规范 §11.3)。

**实现形态的差距**:本包**仍以 `wlwl-std` 的 `real-ai` feature 编译**,只是不再出现在 `wlwl:std.*` 的可见面里;它**没有**独立的 crate 边界与分发产物。`real-ai` 门控的是 **HTTP 传输层**(mock ↔ real),**不是成员** —— 默认构建下这 5 个成员**全部可见可导入**。

**传输与供应商细节属实现文档,不在本规范内。** 默认构建下的可观察结果由 mock 后端给出(见示例)。

### 9.1 示例

```wlwl
IMPORT("ai:ask", ["ASK", "EMBED", "COMPLETE", "ASK_ALL"]);
PRINT(EMBED("hello"));        // [0.17448691538872357, 0.31102464332036317, 0.35886167696650645, 0.5751277943083848]
PRINT(COMPLETE("fn main() {}"));  // // mock completion for (wlwl): fn main() {}…
PRINT(ASK_ALL(["a", "b"]));   // [[mock-batch:0] echo (h=0xe40c292c) :: a, [mock-batch:1] echo (h=0xe70c2de5) :: b]
```

### 9.2 设计依据

本包由 `wlwl:std.ai` **降级为官方包**,前缀取 `ai:`,成员面(名字 / 语义 / 错误码)**一字未改**,变的是**可见面** —— 属须申报的可见面变更(§0.8)。形态上的差距(仍在 `wlwl-std` 内编译、无独立分发产物)记为**部分完成**。

## 10 `agent` — 代理(官方包)

**导入路径是 `agent:task`**(或包名 `agent`)。`wlwl:std.agent` **不再可解析**,成员面一字未改,形态上的差距见 §9。

| 签名 | 语义 | 失败 | 层 |
|---|---|---|---|
| `TASK(name, prompt, opts?) -> STRING` | 构造代理任务并返回一段描述文本;`name` 命中内置任务名时用对应的系统提示,否则用通用提示并给 `W0052` | 元数错 → `E0022`;类型错 → `E0030`;`E0090`–`E0094` | R2 |
| `TOOL(name, description, params_schema?, returns_schema?) -> DICT` | 描述一个工具,返回工具描述字典(`name` / `description` / `params_schema` / `returns_schema` / `v`);两个 schema 形参必须是 `DICT` 或 `NULL` | 元数错 → `E0022`;类型错 → `E0030`;`E0081` | R2 |
| `CALL_TOOL(tool, params) -> DICT` | 同步调用已注册工具;`tool` 是 `TOOL` 的返回值,`params` 是 `DICT` 或 `NULL` | 元数错 → `E0022`;类型错 → `E0030`;`E0082` | R2 |
| `MODEL(name) -> DICT` | 选择模型,返回模型描述字典(`name` / `provider` / `model` / `capabilities` / `v`) | 元数错 → `E0022`;类型错 → `E0030`;`E0080` | R2 |
| `CONTEXT(key, value?) -> v` | 上下文存取:给 `value` 则**写入并返回 `NULL`**;不给则**读取**,不存在时返回 `NULL` | 元数错 → `E0022`;类型错 → `E0030`;`E0083` | R2 |

**网络错误码语义**:`E0090` 网络不可达、`E0091` DNS 失败、`E0092` TLS 错误、`E0093` HTTP 4xx、`E0094` HTTP 5xx。

**`MODEL` 的 provider 来自名字里的 `provider/` 前缀**;没有前缀时 provider 为 `"unknown"`,并给 `W0052`。`capabilities` 按模型名前缀推导(`gpt-4` / `gpt-3.5` → 聊天 / 函数调用 / JSON 模式;`claude*` → 聊天 / 视觉 / 工具使用;`text-embed*` → 嵌入;其余 → 聊天)。

**`CONTEXT` 是「取或存」,不是「存」与「取」两个成员**:一个形参写、一个形参读。**「没设」返回 `NULL`**,与「设为空串」是两件事。

**裸名 `TASK` 本身不与类型名 `TASK` 构成运行时冲突**(语言规范 §2.1):类型名活在类型位置,函子活在值位置。改名是为了**阅读清晰**,不是为了消歧义。

### 10.1 示例

```wlwl
IMPORT("agent:task", ["TASK": "AGENT_TASK", "TOOL", "MODEL", "CONTEXT"]);
PRINT(TOOL("t1", "desc"));    // [description: desc, name: t1, params_schema: NULL, returns_schema: NULL, v: 0.4.0]
PRINT(MODEL("gpt-4"));        // [capabilities: [chat, function_call, json_mode], model: gpt-4, name: gpt-4, provider: unknown, v: 0.4.0]
CONTEXT("k", "v");
PRINT(CONTEXT("k"));          // v
```

### 10.2 设计依据

本包由 `wlwl:std.agent` **降级为官方包**,前缀取 `agent:`,与 §9 同一裁决。

## 11 `std.encode` — 编码与哈希

base64(RFC 4648 §4)、hex、percent-encoding(RFC 3986 §2)与单向摘要。编码类成员都作用在**字节**上,而本语言的 `STRING` 是 UTF-8:编码取实参的 UTF-8 字节,解码后**必须**是合法 UTF-8 才交出去。摘要类成员同口径取 UTF-8 字节。

### 11.1 失败口径

- 元数错 `E0022`、实参类型错 `E0030`(程序员错误走原生诊断)。
- **解码失败是域违例**,返回 `ERR(["kind": "DecodeError", "op": ..., "reason": ...])` **值**。它不叫 `DomainError` —— 把「`SQRT(-1)`」和「base64 串少了一个字符」归成同一个 kind 会让调用方没法区分。
- **编码永不失败**,下表的失败列对三个 `*_ENCODE` 是 `—`。

### 11.2 成员表

| 签名 | 语义 | 失败 | 层 |
|---|---|---|---|
| `BASE64_ENCODE(s, url_safe?) -> STRING` | RFC 4648 标准字母表 + `=` 补齐;`url_safe=TRUE` 换用 URL 字母表(`+/` → `-_`) | — | R2 |
| `BASE64_DECODE(s) -> STRING` | 忽略 `CR` / `LF`(base64 常被折行嵌入);**两张字母表都收**;**补位可省略**(RFC 4648 §3.2),但**一旦出现 `=`,总字符数必须是 4 的倍数** | 非法字符、补位后仍有数据、补位数 > 2、**补位后总长非 4 的倍数**、未补位时长度 ≡ 1 (mod 4)、末组残留非零位、解出非 UTF-8 → `ERR([... "DecodeError" ...])` | R2 |
| `HEX_ENCODE(s) -> STRING` | 小写、无分隔符 | — | R2 |
| `HEX_DECODE(s) -> STRING` | 大小写都收;不忽略任何字符 | 奇数长度、非 hex 字符、解出非 UTF-8 → `ERR([... "DecodeError" ...])` | R2 |
| `URL_ENCODE(s) -> STRING` | percent-encoding;`unreserved = ALPHA / DIGIT / "-" / "." / "_" / "~"`,其余 `%XX`(大写十六进制) | — | R2 |
| `URL_DECODE(s) -> STRING` | 解 `%XX`;**`+` 不折成空格** | 截断或非法的 `%` 转义、解出非 UTF-8 → `ERR([... "DecodeError" ...])` | R2 |
| `SHA256(data) -> STRING` | SHA-256(FIPS 180-4);输入按 UTF-8 字节,输出**小写十六进制**(64 字符) | 类型错 `E0030`;元数错 `E0022` | R2 |
| `HMAC_SHA256(key, data) -> STRING` | HMAC-SHA256(RFC 2104);`key` / `data` 均按 UTF-8 字节;密钥超过块长(64 字节)先哈希再补零 | 同上 | R2 |
| `PBKDF2_ITER(password, salt, iter, len, algo) -> STRING` | PBKDF2-HMAC-SHA256(RFC 8018 §5.2);`salt` 是**十六进制文本**;输出**小写十六进制**。`algo` 当前**只接受 `"sha256"`** —— 签名保留该形参是留位,SHA-512 不提供(见 §11.5) | `iter` ∉ [600 000, 10 000 000]、`len` ∉ [1, 1024]、未知 `algo`、`salt` 非偶长 hex → `E0030`;类型错 `E0030`;元数错 `E0022` | R2 |
| `ARGON2ID(password, salt, t_cost, m_cost, p_cost, len) -> STRING` | Argon2id(RFC 9106,v = 19);`m_cost` 单位 KiB;`salt` 是**十六进制文本**;输出**小写十六进制**。**参数全部显式,无默认值** | `t_cost` ∉ [2, 32]、`m_cost` ∉ [19 456, 1 048 576] KiB、`p_cost` ∉ [1, 16]、`len` ∉ [4, 1024]、解码后 `salt` < 8 字节、`salt` 非偶长 hex → `E0030`;类型错 `E0030`;元数错 `E0022` | R2 |
| `RANDOM_BYTES(n) -> ARRAY` | **操作系统熵源(CSPRNG)**;返回 `n` 个 `INTEGER`(0–255)的**数组**,不是 `STRING` | `n < 0` → `E0030`;类型错 `E0030`;元数错 `E0022`;**熵源读失败 → `E0060`** | R2 |
| `RANDOM_HEX(n) -> STRING` | `RANDOM_BYTES` 的十六进制文本形态,`2n` 个**小写**字符。**本语言里生成 KDF 盐的正确方式** | 同上 | R2 |
| `TIMING_SAFE_EQ(a, b) -> BOOLEAN` | **常数时间**比较两段 `STRING`(按 UTF-8 字节);**永不失败** —— 长度不等是 `FALSE` 而非错误 | 元数错 `E0022`;类型错 `E0030`。**无其它失败** | R2 |

**`RANDOM_BYTES` 返回整数数组,不是 `STRING` —— 这是本语言唯一能无损承载任意字节串的形态。** 名字说的是「字节」,而 wlwl 没有字节类型;硬塞进 `STRING` 只能把非法 UTF-8 有损替换成 `U+FFFD`,而本规范不提供有损替换。**要文本就用 `RANDOM_HEX`。**

**`RANDOM_BYTES` / `RANDOM_HEX` 刻意不设长度上界。** 两个 KDF 设上界是因为「一个参数能打满机器」是**安全**问题;随机缓冲区调大只是多花时间与内存,而一个 wlwl 程序本来就能建更大的数组。`n = 0` **合法**,返回空数组 / 空串。

**熵源读失败是 `E0060`(IO error)而不是 `E0030`。** 前者是**运行环境**出问题(内核没给熵、沙箱封了系统调用),后者是程序员把参数写错了。两者的处置完全不同 —— 这与 §11.1 的三分类(程序员错误 / 数据违例 / 环境失败)是同一条线。

**`TIMING_SAFE_EQ` 的「长度不泄漏」是设计要点,不是实现细节。** 朴素写法 `a.len() == b.len() && …` 在长度不等时**提前返回**,于是「两值长度不同」可从耗时上被观察。本成员走 `max(len)` 步并把长度差折进同一个累加器,**没有任何一处按比较结果分支**。**诚实边界:耗时仍依赖长度**(比较多少字节就走多少步),这无法避免也不必避免 —— 要隐藏的是「哪几个字节不同」与「是否相等」,不是长度。该性质由 `wlwl-std` 的机制断言守着。

**两个 KDF 的上下界不是「推荐值」,是硬边界。** 下界取 OWASP 2025 的最小配置(Argon2id `m` = 19 MiB / `t` = 2 / `p` = 1;PBKDF2-SHA256 `c` = 600 000),上界防「一个参数打满机器」。不设下界的话「参数全显式」等于「调用方可以传 `iter = 1`」—— 那比不给这个成员更危险。**代价也要写明:越界会拒掉一些合法的历史参数。** RFC 7914 §11 的 `c` = 1 / 80 000 与 RFC 9106 §5.3 的 `m` = 32 KiB **在本成员上都是 `E0030`** —— 它们仍作为**原语层**向量冻结在 `wlwl-std` 的单元测试里。

**`salt` 收十六进制文本,不是原样字符串。** RFC 8018 / RFC 9106 的 `salt` 是**任意字节串**,而本语言的 `STRING` 是 UTF-8 文本。三者一律十六进制:偶数长度 + 全 `[0-9a-fA-F]`,否则 `E0030`(不静默截断、不猜测);**空串 = 空字节串**是合法的。⇒ **盐的自然来源是 `RANDOM_HEX`**,不是 `RANDOM_BYTES`。

**`ARGON2ID` 没有 `secret`(pepper)/ `assoc`(关联数据)形参。** 曾提议加上,理由是「没有它们就一个 RFC 官方向量都验不了」。**实测结论是不能加**:唯一的官方向量(RFC 9106 §5.3,带 `Secret[8]` + `Associated data[12]`)在 `argon2` 0.6.0 上**复现不出来**,而同一 crate 的无 K/X 路径三方一致。一个与其他实现**互不认**的 `secret` / `assoc`,存下来的哈希任何别的实现都验不过 —— 那比没有这个参数更危险。

### 11.3 三条需要明写的设计取舍

1. **解码自动识别 base64 字母表。** 标准表的 `+/` 与 URL 表的 `-_` 除这两个字符外完全相同,所以「两张都收」是确定性的。Go 的 `StdEncoding.Decode` 会拒 `-`、`URLEncoding.Decode` 会拒 `+`;本成员比它们宽。混用 `+/` 与 `-_` 的串也能解,这是有意的放宽。
2. **`+` 按 RFC 3986 读,不折成空格。** `+` → 空格是 `application/x-www-form-urlencoded` 的规则。Go 的 `url.QueryUnescape` 做这个替换,`url.PathUnescape` 不做;本成员对标后者。表单编码请自己处理。
3. **解码不做有损替换。** wlwl 没有字节类型,解出的非法字节没有能诚实表达的返回形态。与其 `to_string_lossy` 换成 `U+FFFD` 骗调用方,不如报 `DecodeError`。这同时意味着**不能通过 `STRING` 携带任意二进制** —— 要传二进制得先 base64 / hex,再在对面解回 `STRING`。

### 11.4 RFC 测试向量

base64 编码的期望值取自 RFC 4648 §10 的表,**不复制实现输出**:

| 输入 | 标准 | URL-safe |
|---|---|---|
| `""` | `""` | `""` |
| `f` | `Zg==` | `Zg==` |
| `fo` | `Zm8=` | `Zm8=` |
| `foo` | `Zm9v` | `Zm9v` |
| `foob` | `Zm9vYg==` | `Zm9vYg==` |
| `fooba` | `Zm9vYmE=` | `Zm9vYmE=` |
| `foobar` | `Zm9vYmFy` | `Zm9vYmFy` |

最后一行的「URL-safe 与标准不同」用 0xFB 0xFF 演示(标准 `+/8=` / URL `-_8=`)—— 但那**不是合法的 UTF-8**,而 wlwl 的 `STRING` 是 UTF-8,所以这个向量只存在于 `wlwl-std` 的 Rust 侧单测,不在语言层可达。语言层能观察到的 URL-safe 分歧用 `U+083F`(UTF-8 字节 `E0 A0 BF`):标准 `4KC/`,URL-safe `4KC_`。

### 11.5 哈希的边界

- 只提供**单向摘要**:对称 / 非对称加密不做(标准库给了 AES 就会有人自己攒 TLS —— 误用重灾区);密钥生成不做(需要系统随机,与 `std.rand` 的裁决同族);
- `MD5` / `SHA-1` 已破,**不提供**;SHA-3 / BLAKE3 记演进方向;
- 实现不承诺常数时间;`HMAC_SHA256` 结果的比较由调用方负责;
- 实现复用 `wlwl-ast::sha256`(FIPS 180-4 纯 Rust,初为 AST 稳定 ID 而写)—— 单一实现两个消费者,杜绝副本漂移。

**有 KDF ≠ 密码学这块齐了。下面三条否决依然成立,且不因 `PBKDF2_ITER` / `ARGON2ID` 的加入而松动。**

- **加解密仍未做,密钥生成仍未做。** 本命名空间现在能安全地处理**口令**(慢哈希 + 参数显式 + 上下界硬),也能生成**不可预测**的随机原料与做**常数时间**比较,但**不能加密数据、不能凭空生成密钥**。§11.5 第一条的否决理由原样保留:没有密钥管理就不该给。
- **`algo` 只有 `"sha256"`,不是漏实现。** 签名保留该形参是**留位**(免得日后加值改签名)。SHA-512 若要支持得手写一份实现并另取 FIPS 180-4 向量冻结;而 OWASP 把 PBKDF2-HMAC-SHA-512 的适用场景写成「FIPS-140 合规时」—— 本语言提供不了经 FIPS 校验的实现(没有硬件边界),那条互操作路径不成立。**记为已知限制**,不是设计缺陷。
- **选哪个 KDF(调用方最该看的一段)**:本机 release 实测 `ARGON2ID`(m = 19 456 KiB / t = 2 / p = 1,OWASP 下界)= **20.3 ms**,而 `PBKDF2_ITER`(c = 600 000,同为 OWASP 下界)= **833 ms** —— **Argon2id 便宜 41 倍,且抗 GPU/ASIC 强得多。** ⇒ **新写的口令存储应当默认 `ARGON2ID`。** `PBKDF2_ITER` 存在的理由是**互操作**:别的系统只能验 PBKDF2 时,它是那条下限,不是「二选一时差一点」。两边的下界都会让调用方**明显变慢**(0.8 s / 20 ms 量级)—— 这正是 KDF 的意义,但要写进你的超时与 UX 预期里。`ARGON2ID` 每次调用还分配约 19 MiB。
- **`SHA256` / `HMAC_SHA256` 不是口令哈希。** 它们是**快的**单向摘要:GPU 上每秒几十亿次。口令必须走 `PBKDF2_ITER` / `ARGON2ID`。
- **`RANDOM_BYTES` / `RANDOM_HEX` 与 `std.rand` 的分工**:要**密钥材料 / 盐 / token** → `std.encode` 的这两个;要**可复现的伪随机序列**(测试用、模拟用)→ `std.rand` 显式播种。**`std.rand` 单向消费** `RANDOM_HEX`,不得反向依赖 —— 否则会出现「一个安全一个不安全」的分裂面。
- **`ARGON2ID` 引入了 RustCrypto 的 `argon2` 0.6.0**(MIT OR Apache-2.0,feature 收窄为 `["alloc"]`);`RANDOM_*` 引 **`getrandom` 0.3.4**(MIT OR Apache-2.0,`default-features = false` —— 关掉 `sys_rng` 那个**非密码学**的线程局部回退);`PBKDF2_ITER` **手写**(约 30 行,PRF 复用 `HMAC_SHA256` 已有的字节级实现,不引依赖)。**不做**对称加密 / 密钥生成 / KDF 便捷封装 / KMS / SHA-3 / BLAKE3 / `MD5` / `SHA-1`。

### 11.6 口令处理的标准写法

下面这段是跑过的(不是示意代码;两条输出在括号里):

```wlwl
IMPORT("wlwl:std.encode", ["RANDOM_HEX", "ARGON2ID", "TIMING_SAFE_EQ"]);

LET(salt, RANDOM_HEX(16));
LET(stored, ARGON2ID("correct horse battery staple", salt, 2, 19456, 1, 32));

// 存库:把 salt 与 stored 一起落盘(盐不是秘密,但必须**每次不同**)。

// 验签:重新派生一次,再常数时间比较。
PRINT(TIMING_SAFE_EQ(stored, ARGON2ID("correct horse battery staple", salt, 2, 19456, 1, 32)));  // TRUE
PRINT(TIMING_SAFE_EQ(stored, ARGON2ID("wrong horse battery staple", salt, 2, 19456, 1, 32)));      // FALSE
```

三条要点,每条都有代价:

- **盐来自 `RANDOM_HEX(16)`**,不是 `RANDOM_BYTES` —— KDF 的 `salt` 实参收的是**十六进制文本**。盐不保密,但**必须每次不同**(相同盐 ⇒ 相同口令的哈希可直接互相比对,也让彩虹表能一次命中所有人)。
- **参数照抄,不要自己调小**。`2, 19456, 1` 是 OWASP 2025 的下界。想更快就把 `m_cost` 调大(成本随 `m_cost` 线性涨),**不要**把 `t_cost` 调到 1 —— 那正是下界要拦的东西(会被自己的 `E0030` 拒掉)。
- **比较必须用 `TIMING_SAFE_EQ`**,不要用 `=`。朴素比较会在首个不同的字节处提前返回,验签耗时本身就是一条侧信道。

### 11.7 设计依据

编码族随 v0.11.2 M1 落地;摘要与 KDF 随 v0.11.3 M2 / M6 落地。KDF 的上下界取 OWASP 2025 最小配置;`ARGON2ID` 不收 `secret` / `assoc` 的实测证据见 `docs/plan/addendum-04-crypto-kdf-secrets.md` §3.4。KDF 的性能读数取自 `impl/crates/wlwl-eval/benches/baseline.txt` 的 KDF 段。

## 12 `std.text` — Unicode 大小写与规范规范化

**全局内建 `UPPER` / `LOWER` 只做 ASCII 映射**(语言规范 §10.5),实测 `UPPER("straße")` = `STRAßE`、`UPPER("héllo")` = `HéLLO` —— 非 ASCII 字符原样穿过。本命名空间提供**完整 Unicode 简单大小写映射**与**规范形式**。

| 签名 | 语义 | 失败 | 层 |
|---|---|---|---|
| `TO_UPPER(s) -> STRING` | 完整 Unicode 大写映射;**长度可能变**(`ß` → `SS`) | — | R2 |
| `TO_LOWER(s) -> STRING` | 完整 Unicode 小写映射;希腊 `Σ` 词尾得 `ς`、词中得 `σ` | — | R2 |
| `NFC(s) -> STRING` | 规范组合:`"e"` + U+0301 → `"é"`,**长度变短** | — | R2 |
| `NFD(s) -> STRING` | 规范分解:`"é"` → `"e"` + U+0301,**长度变长** | — | R2 |
| `NFC_QC(s) -> BOOLEAN` | 规范快速检查(性能闸门,见 §12.2) | — | R2 |
| `GRAPHEME_COUNT(s) -> INTEGER` | 字素簇数(UAX #29)。**空串是 `0`**;`"👨‍👩‍👧"` 是 **1** | — | R2 |
| `WIDTH(s) -> INTEGER` | 终端显示宽度(列数),**簇折叠**。三条口径见 §12.3 | — | R2 |

**大小写部分零 Unicode 数据文件**:Rust 的 `str::to_uppercase` / `to_lowercase` 直接走 `char` 实现内置的 `Uppercase` / `Lowercase` 查表(含 `ß` → `SS`、希腊 final sigma、土耳其无点 i / 点上 i 这类特殊映射)。ADR-0022 的零第三方依赖画像因此不变。

**规范化部分要表**:Unicode 规范形式按 **UCD 18.0.0** 定义,表由生成器 `bin/gen-ucd` 从 UCD 源数据产出并入库(源数据 3.66 MB **不入库**,构建期零下载、零生成)。**换 Unicode 版本必须重跑生成器 + 重跑 `bin/norm-check`**,两件事一起做。

**与全局 `UPPER` / `LOWER` 的关系**:两个并存,本册**不替换**全局内建。替换是 breaking(既有程序里非 ASCII 字符的大小写行为会变)。调用方按需要显式选:`UPPER` 快且只管 ASCII,`TO_UPPER` 全 Unicode。

**`NFC` / `NFD` 会改变字符串长度**,因此任何拿 `LEN` / `INDEX_OF` 下标做缓存键或行号的调用方,规范化之后**下标全部失效**。这是正确行为,但会打破调用方的既有假设。

**明确不做**:`NFKC` / `NFKD`(兼容性分解是**另一个语义**,会把全角 / 半角、`①` / `1` 折叠掉,不能与 NFC 混在一处默认)、case folding(等价类判定,与 `TO_UPPER` / `TO_LOWER` 的映射不是一回事)、locale 相关排序 / 比较。

### 12.1 数据源与版本:合成一块做

UCD 依赖的成员**共用同一捆数据**:

| 成员 | 需要的数据 |
|---|---|
| `NFC` / `NFD` | `UnicodeData`(canonical decomposition)+ `CompositionExclusions` |
| `GRAPHEME_COUNT` | `GraphemeBreakProperty` + `Extended_Pictographic` |
| `WIDTH` | `EastAsianWidth` |

分批做意味着生成器写三遍、审三遍、跟 Unicode 版本对齐三遍;合成一块做是**一次**决定。

**明确不做半张表**:只覆盖 Latin-1 的 NFC 比没有 NFC 更糟 —— 它会让 `==(NFC(a), NFC(b))` 在部分输入上返回 TRUE,调用方据此建索引,然后在真正的 CJK 或组合字符上炸。**「正确性只由全量自检证明」这条纪律**的适用范围因此必须一次定清:`GRAPHEME_COUNT` 由 `bin/grapheme-check` 对官方 `GraphemeBreakTest.txt` **全量 853 行**自检,`WIDTH` 由 `bin/width-check` 对 `EastAsianWidth.txt` **全表 1 114 112 个码点逐条比对**;两者都不在 `cargo test` 里(源数据不入库)。

### 12.2 `NFC_QC` 的单方向语义

`NFC_QC(s)` 返回 `TRUE` **当且仅当**该串**必定**已是 NFC 形态,此时可安全跳过 `NFC`。返回 `FALSE` **只表示「不保证」** —— 该串**可能**已经是 NFC 形态。

**反向不成立,这是规范条款而不是实现瑕疵**:官方测试文件里有 202 行的串本来就是 NFC 形态(`Maybe` 类单码点行,例如单独一个 U+0301 —— 它经 `NFC` 之后原样不动,却有 `NFC_QC = Maybe`),快检一律返回 `FALSE`。文面**不得**写成「返回 `FALSE` 即表示不是 NFC 形态」—— 那会教调用方一个假事实,进而写出「不是 NFC 就去 `TO_UPPER`」这类错误推理。

### 12.3 `GRAPHEME_COUNT` / `WIDTH` 的坐标系与口径

**坐标系有两个,不可混用**:`LEN` 给**码点数**,`GRAPHEME_COUNT` 给**字素簇数**。`"👨‍👩‍👧"` 的 `LEN` 是 **5**,`GRAPHEME_COUNT` 是 **1**。

`GRAPHEME_COUNT` 按 **UAX #29 扩展字素簇**(Unicode **18.0.0**,含 15.1 新增的 **GB9.3** Indic conjunct break)。

`WIDTH` 是**簇感知**的:切簇后**每簇取簇内码点列宽的最大值**再求和。三条口径:

| 判据 | 列宽 | 为什么 |
|---|---|---|
| `EAW` = `W` / `F` | **2** | 全角 |
| `EAW` = **`A`**(Ambiguous) | **1** | Unicode 自己定位为「西方 1 列、东亚 2 列」;`unicode-width` / `wcwidth` / `runewidth` 一律默认窄,CJK 是显式 opt-in。`A` 类占全表 **38.9%** |
| `EAW` = `N` / `Na` / `H`,**且**默认 emoji 呈现 | **2** | 实测只影响 **26** 个码点,全是 U+1F1E6–1F1FF 区域指示符 |
| 一般类别 `Mn` / `Me` / `Cf` | **0** | 零宽。`Cf` 含 U+200B / U+200D / U+2060 / U+FEFF —— 按 EAW 它们各得 1,会让每个零宽字符**占一整列** |
| 一般类别 **`Mc`** | **1**(不置 0) | 带间距的组合记号(天城文 visarga 等)在真实字体里占宽度,`wcwidth` 同款 |
| 其余 | **1** | |

**不做任何规范化折叠。** U+2122 OHM SIGN 与 U+00A5 YEN SIGN 是同一字符,本成员下两者都恰好得 1 —— 但那是**各自独立查表**的结果,**不是**识别出了等价关系。要按等价比较,先用 `NFC`。

### 12.4 示例

```wlwl
IMPORT("wlwl:std.text", ["TO_UPPER", "TO_LOWER", "NFC", "NFD", "NFC_QC", "GRAPHEME_COUNT", "WIDTH"]);
PRINT(TO_UPPER("straße"));      // STRASSE
PRINT(TO_LOWER("ÄÖÜ"));         // äöü
PRINT(NFC_QC("abc"));           // TRUE
PRINT(GRAPHEME_COUNT("👨‍👩‍👧"));  // 1
PRINT(WIDTH("日本"));            // 4
```

### 12.5 设计依据

`TO_UPPER` / `TO_LOWER` 随 v0.11.2 M2 落地;`NFC` / `NFD` / `NFC_QC` 随 addendum-03 落地;`GRAPHEME_COUNT` / `WIDTH` 随 v0.11.3 M7 落地,与前三个**合成一块**(见 §12.1)。Unicode 版本固定为 **UCD 18.0.0**,换版本必须同时重跑 `bin/gen-ucd` 与 `bin/norm-check`。

## 13 `std.sanitize` — 输出安全

**威胁模型总述**:本命名空间防的是「不可信数据在**输出点**被解释为代码或结构」。它**不清洗输入**(OWASP 现行口径:输入侧「清洗」不是 XSS / 注入的防治手段),不做语义校验(URL 是否可达、邮箱是否真实),不防逻辑漏洞;每个成员各自写明防什么、不防什么、被绕过时表现为什么。

**上下文分工口径(规范性,三者互不替代)**:不可信数据进入输出的安全手段**由输出上下文决定** ——

- **URL 上下文**(`href` / `src` 等属性的**值**):`std.encode.URL_ENCODE`(RFC 3986,§11);
- **标记上下文**(HTML 文本节点与双引号属性值):`HTML_ESCAPE`(本章);
- **业务必须接受富 HTML** 时:`HTML_SANITIZE`(本章)—— 唯一能保留标记结构的手段。选错上下文的转义(如拿 URL 转义去填 HTML 文本)不是保守,是**错误**。

| 签名 | 语义 | 失败 | 层 |
|---|---|---|---|
| `HTML_ESCAPE(data) -> STRING` | 标记上下文转义,见 §13.1 | 类型错 `E0030`(带名);元数错 `E0022` | R2 |
| `HTML_UNESCAPE(data) -> STRING` | HTML 实体解码(全表),见 §13.1 | 同上 | R2 |
| `HTML_SANITIZE(data, policy?) -> STRING` | 富 HTML 白名单净化,见 §13.2 | 同上;`policy` 形态错 `E0030`(带名) | R2 |
| `HTML_SANITIZE_SCRIPT(value) -> STRING` | 把值加固成可安全内联的 `<script type="application/json">…</script>`,见 §13.4 | 同上 | R2 |
| `CSS_ESCAPE_IDENT(s) -> STRING` | 把字符串序列化成**合法 CSS 标识符**(类名 / `id` / 自定义属性名),见 §13.5 | 同上 | R2 |
| `CSS_ESCAPE_STRING(s) -> STRING` | 把字符串序列化成 **CSS 字符串字面量**(两侧带 `"`),见 §13.5 | 同上 | R2 |
| `HTML_SANITIZE_CSS(text) -> STRING` | 从**声明列表**里按白名单抽出安全声明,见 §13.5 | 命中拒绝构造 → `ERR([kind: "SanitizeError"])` | R2 |
| `HTML_SANITIZE_URL(u) -> STRING` | 归一 + scheme 判定,返回可放进 `href` 的值,见 §13.5 | 不接受 → `ERR([kind: "SanitizeError"])` | R2 |

**SQL 注入不在本命名空间的防治范围** —— 不是遗漏,是裁决:字符串转义被 OWASP 定性为 last resort 且方言分裂,而 wlwl 没有 `std.db`,不存在可参数化的收口;提供 `SQL_ESCAPE` 只会制造假安全感。

### 13.1 `HTML_ESCAPE` / `HTML_UNESCAPE` 的精确口径

**`HTML_ESCAPE`**:对 `& < > " '` 五个 ASCII 字符做实体转义,映射为 `&amp;` / `&lt;` / `&gt;` / `&quot;` / `&#39;`(`'` 用 `&#39;` 不用 `&apos;` —— 旧渲染器兼容口径);**非 ASCII 码点逐字节原样保留**,不做任何 Unicode 处理。适用于 HTML 文本节点与**双引号属性**值两种上下文(HTML5 同口径);**属性值必须带引号**,未加引号的属性值不在本函数契约内。

**`HTML_UNESCAPE`**:HTML 实体解码,**全表** ——

- **命名实体**:HTML5 全表(约 2 231 项,含无分号变体),由生成器从 WHATWG `entities.json` 官方数据产出为静态表(生成器入库,数据文件不入库);**未知命名实体原样保留**(HTML5 自身的容错行为,不是本库私货)—— 这是全表口径,`&copy;` 一类实体**必须**正确解码,不允许「只认本库转义集」的半表行为;
- **数字引用**:`&#DDDD;` / `&#xHHHH;`,按 WHATWG 数字引用解析规则处理(码点越界 / 代理区映射为 U+FFFD 等);**溢出有检查,不得 panic**。

**roundtrip 定理**:`HTML_UNESCAPE(HTML_ESCAPE(s)) == s` 对一切 `s` 成立(本库转义集闭合)。本成员**就是**消费第三方 HTML 时的实体解码器 —— `HTML_SANITIZE` 的解析器复用同一张表。

### 13.2 `HTML_SANITIZE` 与净化策略

**形态(沿 ammonia / nh3 的现代口径)**:容错解析(WHATWG 精神,畸形输入按恢复规则处理,**不报错**)→ **白名单过滤** → 序列化。三段决定:

- 违规标签**删除**,不是转义(nh3 与 bleach 的关键行为差异,bleach 已弃维);
- `script` / `style` 及其**内容**整体删除;
- 属性按逐标签白名单过滤;`href` / `src` 类 URL 属性过 **scheme 白名单**(大小写不敏感、控制字符剥离后再判,防 `jav&#x09;ascript:` 类绕过);
- 解析与过滤**永不因输入内容失败** —— 失败只有类型错 / 元数错 / `policy` 形态错三类原生诊断。

**策略(`policy`,可省)**:`DICT`,四键(全部可省,省略的键取**缺省策略**;出现的键**整体替换**缺省值,不是合并):

| 键 | 类型 | 缺省值 | 说明 |
|---|---|---|---|
| `"tags"` | ARRAY[STRING] | `a` `b` `blockquote` `br` `code` `em` `h1`–`h6` `hr` `i` `li` `ol` `p` `pre` `s` `strong` `u` `ul`(22 个,小写) | 允许保留的标签白名单;不在表内的标签**拆壳留内容**(unwrap),下列例外整棵删除 |
| `"attributes"` | DICT(键:标签名 → ARRAY[STRING];键 `"*"` 为全局属性) | `{"a": ["href", "title"]}`(`"*"` 缺省为空) | 逐标签属性白名单;不在表内的属性**删除** |
| `"url_schemes"` | ARRAY[STRING](小写) | `["http", "https", "mailto"]` | URL 类属性(`href` / `src` / `action` / `formaction` / `poster` / `cite` / `background` / `data` / `srcset` / `longdesc` / `usemap` / `manifest` / `codebase` / `archive` / `dynsrc` / `lowsrc` / `icon` / `xlink:href`)的 scheme 白名单;**相对 URL 恒允许**;判定前先剥首尾空白与控制字符再取 `:` 前缀小写比较(实体已由解析解码) |
| `"strip_comments"` | BOOLEAN | TRUE | TRUE → 注释删除;FALSE → 保留,但注释内容中的 `--` 全部移除(防 `-->` 提前闭合) |

`policy` 形态不符(键值类型错、tags 元素非小写标识符等)→ `E0030` 带名。

**树构建与解析的子集(与浏览器解析器的差异,显式登记)**:

- 解析为**树**(非 token 流):维护开标签栈,实现白名单相关子集的 tree-construction 规则 —— `<p>` 在新块级标签开始时隐式闭合、`<li>` 闭合 `<li>`、`<dt>` / `<dd>` 闭合彼此、`<tr>` / `<td>` / `<th>` / `<thead>` / `<tbody>` / `<tfoot>` 互相按 HTML5 位次闭合;结束标签在栈内则弹到匹配处、不在栈内则忽略。
- **不实现**:foster parenting(表格外内容的搬移)、命名空间 / 外来内容(svg / math 整棵删除)、`<plaintext>` 后的全文文本化(按普通文本处理)、`</p>` 无对应开标签时的空元素生成(直接忽略)。**安全性由幂等性守护,不由「与浏览器相同」守护** —— 本成员的输出再次输入本成员必须逐字节不动。
- **raw-text 元素**(`script` / `style` / `textarea` / `title` / `xmp` / `noembed` / `noframes` / `iframe` 内文):内容不作为标记解析,整棵删除 —— 这些元素的删除集身份同时封掉了「raw-text 内容再解析」这一整类 mXSS 面。**整删集恰好就是这 8 个 raw-text 元素,不多不少**(整删集还含 `object` / `embed` / `noscript` / `template` / `svg` / `math` / `head` 等危险嵌入与外来内容)。**`<plaintext>` 不在整删集** —— 它不是 raw-text 元素,本成员不实现「其后全文文本化」,故按**普通非白名单元素**处理(拆壳留内容)。拆壳是就内容可恢复性而言更宽松,**不放宽内层任何判定**。
- **void 元素**(`br` / `hr` / `img` / `input` 等 14 个):无子节点,序列化为 `<name …>` 无结束标签。
- 未知 / 拆壳的标签,其**属性一并消失**(只有保留标签才谈属性白名单);同名属性重复时**首个生效**。
- 性能契约:解析 / 过滤 / 序列化各一趟 O(n);深嵌套不栈溢出(迭代栈 + 节点索引池)。三条使「O(n)」成立的实现细节:① **结束标签按标签名查「栈内最内层位置」**,判负是一次查表、**与栈深无关**,位置索引必须留**位置串**并随截断**自栈顶向下**清理;② **raw-text 的关闭扫描是原地滑窗**、**不物化整个余文**;③ **属性同名去重是哈希集合**,判负 / 判重都是 O(1)。

**安全不变量(规范性,打破即缺陷)**:**重解析幂等** —— `HTML_SANITIZE(HTML_SANITIZE(x, p), p) == HTML_SANITIZE(x, p)`(字节级)。依据:IEEE S&P 2024 与 Sonar mXSS 研究证明,解析模型与浏览器存在差分的净化器可被 mXSS 结构性绕过。**不承诺与浏览器解析逐位一致**:树构建实现的是 WHATWG 规则的**白名单子集**,差异显式登记。**往返不承诺**:输出不保证与输入逐位相关(嵌套矫正的固有属性)。

### 13.3 为什么没有 SQL 转义成员

**本命名空间不为 SQL 注入提供任何字符串转义成员。** 这不是遗漏,是裁决:

1. **OWASP 的定性是 last resort**:SQL 注入防治的位次是参数化查询(prepared statement)→ 存储过程 → 白名单校验 → **转义垫底** —— 转义易碎且方言分裂(MySQL 的反斜杠转义 vs 标准的双写引号、`NO_BACKSLASH_ESCAPES` 模式、LIKE 通配符另有一套),做错比不做更危险。
2. **wlwl 没有参数化收口**:`std.db` 不存在,调用方拿到的任何 `SQL_ESCAPE` 都只能手工拼串 —— 正是把 OWASP 排在最后的姿势当成第一防线。提供它等于**制造假安全感**。
3. **动态标识符(列名 / 表名)的正解是白名单映射**:把用户提供的名字映射到服务端固定值,而不是转义。这与 `HTML_SANITIZE` 的白名单哲学同源,同样不需要转义函数。
4. **复活条件**:`std.db`(或任何 SQL 互操作命名空间)落地时,防治手段以 prepared statement 形态提供;届时本记录由该命名空间的规范接手。

### 13.4 `HTML_SANITIZE_SCRIPT`

**为什么不是 `JS_ESCAPE_STRING`。** OWASP DOM XSS Prevention Cheat Sheet 明写 `JSON.stringify()`「**is not an output-encoding function**」,并逐字符点名内联场景还要额外挡住 `<`(及 `>` `&` U+2028 U+2029)。本库实测坐实:`STRINGIFY` 把这五个字符**全部原样输出**。**缺口是本库自己的。**

但 OWASP 的**首选**根本不是转义,是「用 `Content-Type: application/json` 单独响应」或「走 `<script type="application/json">` 这个 inert 容器」;而通用 JS 字符串字面量转义剩下的用武之地(`onclick="f('…')"`)已被 `HTML_SANITIZE` 整条剥掉事件处理器属性覆盖掉。⇒ 裁决:**不做通用 `JS_ESCAPE_*`**,做本成员。

**加固动作**:对 `STRINGIFY(value)` 的输出,把 `<` `>` `&` U+2028 U+2029 五个字符逐个换成**大写十六进制**的 `\u003C` / `\u003E` / `\u0026` / `\u2028` / `\u2029`。**只转这五个** —— JSON 自身的 `\"` `\\` 与控制字符转义已由 `STRINGIFY` 负责。

**为什么收整个元素而不是裸串**(这是本成员的关键设计):若只暴露一个「转义裸串」的成员,调用方可以忘记调、可以只转一半、可以转完又拼回去 —— 那它提供的安全感和裸拼字符串没有区别。收**值**、由成员自己 `STRINGIFY` + 加固 + 包壳,「忘记加固」这个失败模式就**不存在**,而不是靠文档提醒。这与 `HTML_SANITIZE` 收整段标记、`HTML_SANITIZE_URL` 收整个 URL 是同一个族。

**失败口径:全函数,且不因内容失败。** 加固把 `<` 全部消掉之后,输出里唯一的 `<` 是本成员自己写的开标签,任何输入都无法终止 script data 状态 ⇒ **没有任何值能让它出错**。⚠ 与 §13.3 同体例:**「什么情况下它不够用」要成文** —— 需要把数据**执行**而不是**读取**时(本成员产出的 `application/json` 块必须由 `JSON.parse` 读取),以及需要跨域读取时(同源策略,与本成员无关)。

**明确不做**:`JS_ESCAPE_STRING` / `JS_ESCAPE_COMMENT`(通用字面量与注释转义)、JS 代码生成、把裸 JSON 交给 `eval` 的任何便利化。

### 13.5 CSS 上下文与 URL 上下文

**实现顺序按 OWASP 口径**:CSS 上下文指引的第一句是「**allowlist it**」,CSS-escape 是**补充** ⇒ `HTML_SANITIZE_CSS` 先于两个 `CSS_ESCAPE_*`。这与 §13.3 否掉 SQL 转义是同一条纪律:转义垫底、白名单当第一防线。

**`CSS_ESCAPE_IDENT` / `CSS_ESCAPE_STRING` 逐条照抄 CSSOM §"Common Serializing Idioms"**,不是自创规则。要点:

- 「转义一个字符」= `\` + 该字符;「按码点转义」= `\` + **最小位数小写十六进制** + **一个空格**。尾随空格是消歧符,漏了它 `\31 23` 会与 `\3123` 混。
- 标识符:`U+0000` → `U+FFFD`;`[U+0001-U+001F]` 与 `U+007F` 按码点转义;**首字符**数字按码点转义;**第二字符**数字且首字符是 `-` 按码点转义;单独的 `-` 转义为 `\-`;`>= U+0080` 与 `- _ 0-9 A-Z a-z` 原样;其余转义。
- 字符串:`"` + 内容 + `"`;`U+0000` → `U+FFFD`;控制字符按码点转义;`"` 与 `\` 转义;**其余原样**。⚠ **`'` 不转义** —— CSSOM 原文注明字符串**总是**用 `"` 序列化,单引号没有转义必要。
- ⚠ **版本差异(契约冻结现行版)**:CSSOM 的这两条算法改过 —— 2013 WD 版对 `U+0000` 是**抛 `InvalidCharacterError`**,控制字符范围也更宽(`[U+007F-U+009F]`);**现行版**(W3C REC cssom-1)是 `U+0000` → **`U+FFFD`**、取 `[U+0001-U+001F]` + `U+007F`。本规范跟**现行版**。

**`HTML_SANITIZE_CSS` 的输出形态**:每条存活声明写成 `属性: 值`,**声明之间以 `;` 分隔**(尾随 `;` 保留)。⚠ 分隔符**不可省** —— `color: red font-weight: bold` **不是合法 CSS**;属性名一律小写归一。**没有任何声明存活时返回空串 `""`**,不是一个孤零零的 `;`。

**`HTML_SANITIZE_CSS` 的两条分开失败口径**:

| 情形 | 结果 |
|---|---|
| 属性**不在白名单** | **静默丢弃**,产出里看不到它 —— 那是白名单**正常工作**的样子 |
| 命中**拒绝构造** | 整个输入判为攻击 ⇒ `ERR([kind: SanitizeError])` |

拒绝构造(文本级,**先于**解析扫描):`@import`、`expression(`、`javascript:`、`vbscript:`、`behavior:`、`-moz-binding`、**任意 `url(`**。

⚠ **任意 `url(` 一律拒**,不做「URL 看着安全就放行」—— 那需要解析 `url()` 的完整语法 + `data:` 的 MIME 白名单,是**另一个成员**的事。能取 `url()` 的属性(`background` / `cursor` / `content` / `filter` / `src` / `list-style-image` / `border-image`)**也一律不在白名单** —— 不靠单点防御。⚠ 解析范围只到**声明级**(`prop: value` 对):**不做选择器语义、不做值语法解析**;但拒绝扫描是**文本级**的(先扫原文再解析),分号与冒号都**不在引号内、不在圆括号内**才算分隔符。

🔴 **拒绝扫描扫的是「原文 ∪ 按 CSS 转义解码后的原文」**。⚠ 只扫原文是可绕过的:`e\78 pression(…)` 在字节层面根本不含 `expression(`,而浏览器**会**把它解码回去 ⇒ 扫描必须按 **CSS Syntax Module Level 3 §4.3.7 *Consume an escaped code point*** 解码后再比:反斜杠后**尽量多取十六进制、至多 6 位**,零 / 代理区 / 越界一律返 `U+FFFD`,遇到 EOF 同样 `U+FFFD`(解析错误),**非十六进制则该字符字面**;那个「顺带消费一个尾随空白」**只在十六进制分支**。⚠ 解码结果**只喂扫描、不进输出**。⚠ **两份原文都要扫**:反斜杠在 token **之前**的写法(`\url(`)字面已含 token,只扫解码遍反而会漏。

⚠ **本成员提供的保证值多少 —— 一条必须写明的限制**:`expression()` 是 **IE5–IE9 的专有特性**,IE10 起被移除,其它引擎从未实现 ⇒ **现代浏览器不执行它**。因此本成员的真实防线是**两道**,而不是「堵住一个能打的洞」:① **白名单**(只有列出的属性与值能进);② **拒绝扫描**(把 `expression(` / `@import` / `javascript:` / `vbscript:` / `behavior:` / `-moz-binding` / 任意 `url(` 挡在外面)。其中 `@import` 与 `url(` **即便被绕过也无出口** —— 能取 `url()` 的属性(`background` / `background-image` / `cursor` / `content` / `filter` / `src` / `list-style-image` / `border-image`)**一律不在白名单**。⚠ 「拒绝扫描可被反斜杠转义绕过」这个**静态性质**已经为假(本节的双遍扫描把它关掉了);但**不要**据此以为本成员是「浏览器沙箱」—— 它是**上下文转义 + 白名单**的组合,不是同源策略或 Trusted Types 的替代品。

🔴 **`HTML_SANITIZE_CSS` 的返回值是「CSS 源码片段」,不是 CSS 数据值**。声明值出函数前按 CSSOM §"Common Serializing Idioms" 的 *escape a character as code point*(`\` + 小写十六进制 + **一个空格**)逐字符加固,至少覆盖 `<` `>` `"` `'` `*` `;` `{` `}` `\` 与裸 CR / LF / FF。

⚠ **不能照抄 `HTML_SANITIZE_SCRIPT` 的 `\u003C`**:JS 字符串字面量的转义形状是 `\uXXXX`,CSS **没有这个形状** —— 照抄过去在 CSS 里只是标识符文本 `u003C`,`<` **原样留在输出里**,破壳照旧。两边形似而义不同,是本节最容易照错的一处。

⚠ **反斜杠必须与本文插入的转义一起加固**:CSS 里 `\` 后跟十六进制才是转义,调用方自带的 `\` 会与插入的 `\3c ` 连成别的序列,转义就不再是「值的逐字映射」。副作用是**有意的**:调用方自己写的嵌套 CSS 转义(中文字体名 `\5FAE\8BA4` 一族)被当数据处理,字体名退化为不匹配 → 回退默认字体。对**不可信数据**而言,嵌套转义语法正是拒绝扫描的绕过面,不能留给它。

**因此,与 §13.4 同体例 —— 「本成员在什么情况下不够用」成文**:

1. **返回值必须原样内联**。它已经是可安全放进 `<style>` / `style="…"` 的源码片段;调用方若把它当字符串**数据**再拼进 CSS 上下文,会得到双份转义。
2. **不接受需要嵌套转义的字体名**。要安全地放进字体名,先过 `CSS_ESCAPE_STRING`。
3. 仍然**不做**:把返回值当 HTML 用(那是 `HTML_ESCAPE` 的活)、跨域读取(同源策略,与本成员无关)。

**`HTML_SANITIZE_URL` 与 `HTML_SANITIZE` 的 URL 属性判定是同一份逻辑**,默认 scheme 列表同为 `http` / `https` / `mailto`。**复用,不要各写一份** —— 否则会出现「净化器放行、独立成员拒绝」的分裂,而分裂意味着其中一处是错的。契约测试对同一组 URL 喂两个入口并断言结论一致。

⚠ **但那条「一致」契约只断言一致、不断言哪个结论** —— 两处**同时**错也照样绿。它守的是「别分裂」,不是「别都错」。因此契约里另有**绝对结论**的守卫。

**相对 URL 的判定是位置性的**:剥首尾控制字符与空白后,**第一个** `/` `?` `#` **之前**出现的那个 `:` 才是 scheme 的结尾;落在它们**之后**的 `:` 是路径段 / 查询 / 片段内部的分隔符 ⇒ 该值是**相对 URL**,**恒允许**,不进 scheme 白名单比对。依据 WHATWG URL §4.2 *scheme start state* ≡ RFC 3986 §3.3 / §3.4 / §3.5。⚠ **豁免必须排在「scheme 形态合法性」判定之前** —— 顺序反了会把 `/wiki/special` 当成非法 scheme 而拒掉。

⚠ **不做大小写归一、不做百分号编码**:路径大小写敏感,小写化会**改错资源**;百分号编码是 URL 上下文自己的事(`std.encode.URL_ENCODE`)。本成员只做**安全判定需要**的那一步归一(剥首尾控制字符与空白)。

### 13.6 示例

```wlwl
IMPORT("wlwl:std.sanitize", ["HTML_ESCAPE", "HTML_UNESCAPE", "HTML_SANITIZE", "HTML_SANITIZE_SCRIPT", "CSS_ESCAPE_IDENT", "CSS_ESCAPE_STRING", "HTML_SANITIZE_CSS", "HTML_SANITIZE_URL"]);
PRINT(HTML_ESCAPE("<a href=\"x\">&</a>"));  // &lt;a href=&quot;x&quot;&gt;&amp;&lt;/a&gt;
PRINT(HTML_UNESCAPE("&lt;b&gt;"));           // <b>
PRINT(HTML_SANITIZE("<p onclick=\"x\">hi</p><script>bad</script>"));  // <p>hi</p>
PRINT(HTML_SANITIZE_SCRIPT(["a": "<b>"]));  // <script type="application/json">{"a":"\u003Cb\u003E"}</script>
PRINT(CSS_ESCAPE_IDENT("a b"));             // a\ b
PRINT(CSS_ESCAPE_STRING("a\"b"));           // "a\"b"
PRINT(HTML_SANITIZE_CSS("color: red; font-weight: bold"));  // color: red; font-weight: bold;
PRINT(HTML_SANITIZE_URL("https://example.com/a"));  // https://example.com/a
```

### 13.7 设计依据

本命名空间随 v0.11.3 M1 / M3 / M7 落地;转义 / 解码 / 净化全员 R2 —— 它们都要处理**文档级输入**,而解释器侧字符串与数组构建是超线性的(与 `RANGE` 沉回 R2 同一根因)。O(n) 线性承诺与吞吐目标以基准存档立约,超线性 = 缺陷。`JS_ESCAPE_*` 的否决与 `HTML_SANITIZE_SCRIPT` 的换题见 `ADR-0030` D2;Trusted Types 范式的否证记录见 `ADR-0030` D3(复活条件:wlwl 出现**宿主形态**)。缺陷登记:D13-009(policy 合法键的报错形状)、D13-022(`HTML_SANITIZE_CSS` 返回值形状)、D13-023(相对 URL 判定)、D13-058 / D13-059 / D13-062(三条 O(n) 实现细节)、D13-070(拒绝扫描的解码遍)。

## 14 演进方向

本章**非规范性**。它记录已经裁决的方向、已否决的方向与尚未立项的候选,不构成一致性要求。

### 14.1 已裁定的方向

- **类型化收口(Trusted Types / Sanitizer API 范式)** —— 否证记录(否证记录与肯定记录同权):Trusted Types 的**全部**安全收益来自「**浏览器在 sink 处强制拒绝裸串**」。wlwl **没有宿主形态**时,这条收益是**定义上的**消失,不是打折:**没有 sink**(`innerHTML` / `eval` / `document.write` / `location.href` 全部不存在),**也没有执行者**(强制发生在浏览器里,不在语言里)。剩下的只有「一个做净化、返回字符串的函数」—— **那正是 `HTML_SANITIZE`,已经有了**。⚠ 有第三方资料称 Trusted Types 可防服务端 XSS —— **那是错的**;同一批资料自己也写着「Trusted Types do not protect against server-side XSS; they only cover DOM XSS sinks in the browser」。本条采信后者。**复活条件**:wlwl 出现**宿主形态**。
- **CSS / JS 上下文转义** —— CSS **做**(白名单高于转义 ⇒ `HTML_SANITIZE_CSS` 先于 `CSS_ESCAPE_*`),JS **换题**:`JS_ESCAPE_STRING` / `JS_ESCAPE_COMMENT` **不做**,改做 `HTML_SANITIZE_SCRIPT`(已落地)。
- **`std.ai` / `std.agent` 降级为官方包** —— **已落地**(前缀 `ai:` / `agent:`,成员面一字未改,章节见 §9 / §10)。⚠ **形态上的差距**:包**仍以 `wlwl-std` 的 `real-ai` feature 编译**、**无独立 crate 边界与分发产物** ⇒ 记为**部分完成**。⚠ `std.rand` 适用同一套逻辑(见 §17),**尚未搬**。
- **新 R2 候选命名空间 `std.net` / `std.process` / `std.env` / `std.time`** —— **已全部落地**:`std.process` → §19、`std.env` → §18、`std.time` → §15、`std.net` → §21。⚠ `std.net` 的**阻塞**形态是**带条件冻结**的 —— 冻结前提是「真 state-machine 续跑仍未落地」;前提一旦不成立,**必须重新裁决**(那时 `HTTP_*` 可以直接做挂起形态,连 30 s 超时兜底都不必承担)。守卫测试会在前提不成立时转红。

### 14.2 两条**已否决**的方向

| 方向 | 裁决 | 理由 |
|---|---|---|
| `SORT_UNSTABLE` / 选择类原语 | **不加** | 现有 `SORT` 的稳定性语义是规范承诺,而引入不稳定排序要同时处理比较器一致性检测与「择序策略决定组间次序」的新口径 —— 那是**语义变更**。收益(大数组常数因子)不足以换一次 breaking |
| `wlwl:std.rand` 早期形态 | **不做默认播种** | wlwl 的卖点包含**可复现**(确定性并发、规范化的格式化器、契约门禁)。一个**默认播种**的 RNG 会削弱这个卖点,而一个**要求显式播种**的 RNG 需要先裁决「全局状态 vs 显式传递」—— 那是**设计决策**,不是加几个函数。参照先例:Rust 把 `rand` 放在 `std` 之外正是这个原因 |

两条都不是「不重要」而是「**没想清楚**」。要动它们,先出 ADR。

### 14.3 尚未立项的候选

- **正则**:RE2 式线性时间模拟(NFA,零依赖手写)—— 主流 stdlib 全员配备,而本语言在 `std.regex` 落地之前零模式匹配能力。**已落地**为 `std.regex`(§16)。
- **惰性迭代**:`WINDOW` 之类成员要线性就得惰性化,那是 `std.iter` 的事。
- **R1 预编译快照**(替代启动解析);
- **全局内建弃用别名**(`DEL` / `POP` / `OR_DIE`)到期移除;
- **SHA-3 / BLAKE3** 摘要;
- **对称加密 / 密钥生成 / KMS**。

## 15 `std.time` — 墙钟与单调钟

时间能力分三片推进:**A1 纯读** + **A2 阻塞睡眠**(本章)、**B 假时钟气泡**。范围与裁决见 [`ADR-0024`](../adr/0024-time-fake-clock-bubble.md)。**别把本章读成「时间能力齐了」**。

### 15.1 成员

| 签名 | 语义 | 失败 | 层 |
|---|---|---|---|
| `NOW() -> INTEGER` | **墙钟**:Unix 纪元起的**毫秒**。**可能因系统时间调整而向后跳**(NTP 校时 / 手动改表 / 夏令时),所以**不要**用它测时间差 | 元数错 `E0022`。**无其它失败** —— 读时钟不会失败 | R2 |
| `MONOTONIC() -> INTEGER` | **单调钟**:**进程启动起**的毫秒。单调不减,且**不受系统时间调整影响**。⚠️ **绝对值无意义、跨进程不可比 —— 只有差值有效** | 同上 | R2 |
| `SLEEP(ms) -> NULL` | **阻塞原语**:调用线程停住 `ms` 毫秒,期间**不调度任何其他任务**。`ms` 必须是非负整数(`0` 合法 = 让出一次时间片)。⚠️ **气泡内它只拨钟不阻塞**(见 §15.6) | 元数错 `E0022`;非整数 `E0030`;`ms < 0` `E0030` | R2 |
| `TEST_BUBBLE(body) -> v` | 在**假时钟气泡**内跑 `body`:期间 `NOW` / `MONOTONIC` 走虚拟钟、`SLEEP` 只拨钟。body 的返回值原样传出。**气泡不再嵌套**;气泡内 `SPAWN` 的任务**归气泡且无法逃逸** | 元数错 `E0022`;非函数 `E0030`;已在气泡内 `E0030` | R2 |
| `ADVANCE(ms) -> NULL` | **仅气泡内**:把虚拟钟拨快 `ms` | 气泡外 `E0030`;元数错 `E0022`;非整数 / 负值 `E0030` | R2 |
| `TIMEOUT(d, body) -> v \| ERR` | 跑 `body`,虚拟钟走过 `d` 毫秒仍未返回就**放弃**它,返回 `ERR([kind: "Timeout"])`。**只在气泡内可用** | 元数错 `E0022`;非函数 / 负预算 `E0030`;气泡外 `E0030` | R2 |

**为什么是两个而不是一个**:「现在几点」要问 `NOW`,「过了多久」要问 `MONOTONIC`。后者用 `Instant`(Rust 标准库的 `Instant`),系统时间怎么改都动不了它;前者用 `SystemTime` 才能给出绝对时刻。**两者单位相同(毫秒)但起点差着四十多年** —— 把它们混用是最容易犯的错,契约表有一条专门钉这件事。

### 15.2 三条口径

1. **单位是毫秒,不是秒。** 取秒会带来 ±1 s 的量化误差,直接污染「缓存 5 秒过期」这类真实语义。两个成员单位统一为 ms 也是刻意的:这样 `SLEEP(ms)` 与时钟差值能直接对齐,不必在每个调用点换算。
2. **本册不做 ISO-8601 字符串。** 要写人类可读的时间戳是**格式化层**的事;`std.format` 尚无日期能力,本册不扩。`NOW()` 给的是整数,不是字符串。
3. **不支持亚毫秒测量。** 记为**已知限制**,不是疏漏。

### 15.3 可复现性:使用 `NOW()` 会让程序不可复现

本语言的卖点是「同一输入 ⇒ 同一行为」。**用了 `NOW()` 的程序不再满足它** —— 而且是**静默地**不再满足(没有诊断、没有警告)。

⇒ **规范不检测这件事**(运行时检测不到「你有没有用它」),只在成员表上声明。**测试代码尤其要用 `MONOTONIC` 而不要用 `NOW`** —— 测试的可复现性比断言的直观更重要。假时钟气泡正是为了把这件事在**测试里**变回可复现(而生产代码里用真实时间是使用者的明确选择)。

### 15.4 层与来源

R2(触系统资源,§0.7 机械规则第 1 条)。**成员在 `wlwl-std` 内直接读系统时钟,不经宿主** —— 与 `std.encode` 的 `RANDOM_BYTES` 直读 `getrandom` 同款理由:`std.time` 是**时间的源**。

⚠️ **绝不能给本命名空间的成员加「可注入时钟」形参。** 那等于让调用方能把它降级成可预测源 —— 与「给 CSPRNG 加后门」是同一条错误。虚拟时钟**只走宿主**(`StdHost` 加方法),且必须如此:全局可变状态会让测试之间互相污染、结果顺序相关(违反「无隐藏全局状态」纪律)。

### 15.5 `SLEEP` 是**阻塞原语** —— 这条代价写明

**`SLEEP` 期间不调度任何其他任务。** 顶层如此,**任务体内调用同样如此**。

这不是「实现图省事」,而是一条**语言级语义**,理由与代价都写在这里:

- **原本的设计是「任务内挂起、到点唤醒」。** 实现时撞上一个**运行时能力缺口**:wlwl 的「中途挂起后**原地续跑**」能力**只有 `YIELD()` 有** —— 它由分段 + 保存运行环境两件事共同实现,而**分段器只认 `YIELD()` 这一个名字**。任何**非分段**任务体一旦被挂起再唤醒,重入走整段重跑(绑定从头重来、副作用重放)。⇒ 挂起路线下,唤醒会**再次执行同一个 `SLEEP`**,**无限循环**(实测 8 秒 79 次仍不终止)。
- **因此改为阻塞。** 这条代价**早已被接受**:候选方案的原话就是「**`SLEEP` 期间不调度其他任务**」,当时选它就是为了它**可被文档化成一条确定性规则**。改为阻塞只是把它从「运行队列排空后才阻塞」提前到「调用点即刻阻塞」,**没有新增一类代价**,反而省掉了挂起 / 唤醒两套机制。
- **⚠️ 与阻塞式 IO 的冲突照旧**:本语言有阻塞式 IO(`ai` / `agent` 包的 HTTP 后端),`SLEEP` 与它**不可同时依赖其及时性** —— 这是语言级取舍,不是实现细节。
- **契约表有一条专门的反向守卫**:任务体里 `SLEEP` 之后必须继续执行并交出结果。它红了的形式是**测试超时** —— 而超时恰恰就是「挂起路线被重新引入」的特征读数。

### 15.6 假时钟气泡:能给什么、给不了什么

`TEST_BUBBLE(body)` 内:虚拟单调钟**从 0 起算**,虚拟墙钟 = **写死的起点** `1_700_000_000_000`(2023-11-14T22:13:20Z)+ 已流逝毫秒。起点写死是「可复现」这条承诺的**全部** —— 起点一旦含真实时间,同一程序两次运行读到的 `NOW` 就不同,气泡的意义当场消失。⚠️ 气泡内的 `NOW()` **不是真实时刻**;要真实时刻用气泡外的 `NOW()`。

| 气泡**能给** | 气泡**给不了** |
|---|---|
| `SLEEP(3600000)` 墙钟耗时 **< 1 s** | 「虚拟时钟**只在全员阻塞时**推进」 |
| 同一程序跑两次,气泡内时间序列**逐字节相同** | 「A 睡 1 s,B 能看到 +500 时刻」 |
| 超时(`TIMEOUT`)是一个**确定性事件** | `SLEEP` 期间让别的任务插进来(任务间交错) |

**为什么给不了后两条**:那需要**挂起**睡着的任务、让别的任务跑完再推进时钟。本语言的中途续跑能力**只有 `YIELD()` 有**(分段器只认 `YIELD()` 这一个名字,非分段任务体被唤醒后**整段重跑**),而运行时是**单线程协作式**的(没有工作线程,调度是一个 run queue)⇒ 「`SLEEP` 真阻塞线程」也不可能:阻塞了就没有别的代码去推进虚拟时钟。⇒ 气泡内 `SLEEP` 只能「**立即推进虚拟时钟并返回**」,时钟在 `SLEEP` 的**调用点**就跳。

**要拿到那两条,必须先补「任意挂起原因的中断续跑」这一运行时能力** —— 那是独立立项,不是本成员的待办。

**`TIMEOUT` 的中断粒度**:超时触发时,`SLEEP` 置「已放弃」标志并返回 `ERR`。实测粒度:**从最内层闭包返回**,且不逸出 `TEST_BUBBLE` / 顶层。⇒

- `SLEEP` **直写在** `TIMEOUT` 的 body 里 ⇒ body 被**完全放弃**;
- `SLEEP` 藏在 body 内**更深的函数调用**里 ⇒ 只中断到那一层,外层继续跑完。

⚠️ 但 **`TIMEOUT` 的返回值永远正确**:它查那个标志,只要置过就返回 `ERR([kind: "Timeout"])`,哪怕 body 最终跑完了。⇒「超时了没有」这个问题永远有确定答案,「省了多少时间」取决于 `SLEEP` 位于多深的调用里。

**嵌套**:气泡**不再嵌套**(已在气泡内 → `E0030`),但 **`TIMEOUT` 可以嵌套**;此时取**最紧的未放弃** deadline(不是栈顶)。被放弃过的那一项会退出计算,否则它会永久触发后续每一次 `SLEEP`。

### 15.7 示例

```wlwl
IMPORT("wlwl:std.time", ["NOW", "MONOTONIC", "SLEEP", "TEST_BUBBLE", "ADVANCE", "TIMEOUT"]);
PRINT(TEST_BUBBLE(FUN((), ADVANCE(1000))));   // NULL
PRINT(TEST_BUBBLE(FUN((), MONOTONIC())));      // 0
PRINT(IS_ERR(TEST_BUBBLE(FUN((), TIMEOUT(10, FUN((), SLEEP(100)))))));  // TRUE
```

### 15.8 设计依据

本命名空间随 v0.11.3 M6 落地。范围与裁决见 [`ADR-0024`](../adr/0024-time-fake-clock-bubble.md) §6 与 `docs/plan/addendum-01-std-time.md` §3.7;`SLEEP` 的阻塞代价来自「中途挂起后原地续跑」只有 `YIELD()` 一条路这个**运行时能力缺口**。

## 16 `std.regex` — 线性时间正则

**立身之本是「执行时间对输入线性」。** 实现走 Thompson 构造 + Pike VM,**没有回溯**。下面的「必须拒绝」那一组不是功能取舍,是这条承诺的**前提**:一旦允许回溯引用或 lookahead,指数爆炸就成了可能,那这个成员等于没做。

### 16.1 成员

| 签名 | 语义 | 失败 | 层 |
|---|---|---|---|
| `RE(pattern) -> DICT` | 编译成**模式对象** `[pattern, groups]` | 语法错 → 原生诊断 `E0030` 中止(带**列号**);类型错 → `E0030`;元数错 → `E0022` | R2 |
| `RE_TEST(re, s) -> BOOLEAN` | **整串**是否匹配(两端锚定,且**忽略** `(?m)` —— 整串的语义是「整个输入被消费完」) | 元数 / 类型错同上。**永不因内容失败** | R2 |
| `RE_SEARCH(re, s) -> DICT \| NULL` | 第一个匹配:`[matched, start, end, groups]`。`start` / `end` 是**码点**下标(与 `CHAR_AT` / `INDEX_OF` 同一坐标系),**不是字节偏移** | 同上 | R2 |
| `RE_FIND_ALL(re, s) -> ARRAY of DICT` | 全部**非重叠**匹配,逐项形状同 `RE_SEARCH` | 同上 | R2 |
| `RE_REPLACE(re, s, repl) -> STRING` | 替换全部非重叠匹配;`repl` 里 `$0` 是整体、`$1`… 是数字捕获组 | 同上;`${…}` 命名组 → `E0030` | R2 |
| `RE_SPLIT(re, s) -> ARRAY of STRING` | 按匹配切分(匹配本身不保留);**空匹配不做切分点** | 同上 | R2 |
| `RE_GROUP_COUNT(re) -> INTEGER` | 捕获组个数(不含整体)。供调用方在解码侧索引,免得每个成员各返回一种形状 | 同上 | R2 |

**不匹配不是错误。** `RE_*` 返回成功值或 `NULL`;唯一的原生诊断是元数 / 类型 / 编译错。正则不匹配是**正常的业务分支**,做成 `ERR` 会逼调用方到处 `ERR_PAYLOAD`。

### 16.2 语义:左最早,不是左最长

多个分支都可能匹配时,wlwl 取**左最早(leftmost-first)** —— 也就是「一个回溯引擎最先找到的那个」,**与 Perl / Python / JS / Java / Rust `regex` 一致**。

⚠️ **这里刻意不跟 RE2 的默认。** RE2 的默认也是 leftmost-first,而**左最长(leftmost-longest)是它的 POSIX 模式**;选它会让 wlwl 成为唯一异类,而正则跨语言移植是日常操作。另一条理由:Go 的文档承认 POSIX 的子选择规则「计算上不可承受甚至没有良定义」,它自己的 `CompilePOSIX` 也没完全遵守。

⇒ **线性时间来自 NFA 模拟,与选哪条语义无关** —— 所以没有性能理由偏向任何一方。

### 16.3 必须拒绝的构造(与线性时间互斥)

`(?=…)` / `(?!…)`(lookahead)、`(?<=…)` / `(?<!…)`(lookbehind)、`\1` / `(?P=…)`(**回溯引用**)、`(?<name>…)`(命名组)、`\p{…}`(Unicode 类别)、反向正则、零宽断言 —— **一律在编译期 `E0030`**,不静默放行。**它们不是「以后再加」**:允许其中任何一条,线性承诺当场破。

⚠ **零宽断言含转义形式 `\b` `\B` `\A` `\z` `\Z`**。这五个在 Perl / Python / PCRE / RE2 里都是**正宗的零宽锚点**,从那些生态过来的人几乎必然会写到 ⇒ **必须给出指名报错的 `E0030`,而不是静默当字面量**。⚠ **字符类内同样拒**(`[\b]`):类外与类内是同一个缺口的两个入口,只堵一个会留下另一半。口径与 Go `regexp/syntax` 一致(它在类内也报 `invalid escape sequence`)。

⚠ **为什么不能实现**:零宽断言在 Thompson NFA 上要**分支**而不是消费字符 —— 那会让一次抵达同时对应「锚点前」与「锚点后」两条路径,§16.2 的 leftmost-first 裁决键随之不再是良定义的。

反向引用的实测后果(本仓的单测钉着):`(a*)*b` 在 10 000 个 `a` 上,回溯引擎跑不完,而本成员**立刻返回「不匹配」**。

### 16.4 四条容易踩的细节

1. **`.` 默认不含换行**,`(?s)` 让它匹配一切(依据 RE2 `re2.h` 的 `dot_nl (false)`)。
2. **`^` / `$` 默认锚整个输入**,`(?m)` 切到行锚点。⚠ **刻意不抄 RE2 的 `m` 命名** —— RE2 的 `(?m)` 语义与 Perl **相反**(它的 `one_line` 才是「只在文本首尾」),而从 Perl / JS / Python 过来的人对 `m` 的直觉是「多行 = 行锚点」。跟直觉走。🔴 **`$` 是 `\z` 语义、不是 `\Z`**:RE2 `doc/syntax.txt` 原文 "text end (**like `\z` not `\Z`**)" ⇒ **不匹配结尾换行之前**。这与 RE2 / Go **一致**;与 **Perl / Python** 的 `$` 不同 —— 后者在结尾换行**前**也成立,而 RE2 把那个形态叫 `\Z` 并标注 **NOT SUPPORTED**。⇒ 从 Perl / Python 过来的人若要「换行前也成立」,**本成员给不了**,且**不会假装给**。
3. **`\w` `\d` `\s` 与 `(?i)` 都是 ASCII-only**。Unicode 大小写折叠走 `std.text` —— 两套口径混在一起会让「为什么 `\w` 不匹配 `é`」变成一件说不清的事。
4. 🔴 **`(?i)` / `(?s)` / `(?m)` 的作用域比 RE2 / Go 窄**。本成员是「到**本分支**结束」;RE2 `doc/syntax.txt` 原文 `(?flags)` = "set flags **within current group**"(中文版「until outer paren closes」)⇒ RE2 / Go 是「到**所在 group 的右括号**」;Go `regexp/syntax` 的 `parse.go` 同证(标志在闭合括号处才恢复,`|` **不恢复**)。⇒ **`(a(?i)b|c)` 上两者结果不同**:RE2 让 `c` 也忽略大小写,本实现不。⚠ 改成 RE2 口径要在**整个 group 层**共享标志而非每分支拷贝,属**结构变更**,本版**不修**。

### 16.5 已知限制

- **`TRY_RE` 本册不做**:模式来自用户输入时**无法「试编译不中止」**。理由是不对称 —— 加成员是**附加式**(日后加不破坏任何东西),而**改失败形态是破坏式**,所以先把破坏的那条定死。
- **无命名组**(见 §16.3);`RE_REPLACE` 见到 `${…}` 报 `E0030` 而不是原样输出。
- **量词计数上限 1000、编译后指令数上限 64 K**:超出的模式报 `E0030`。这防的是「模式把**编译期**打爆」,与**输入长度**无关 —— 线性承诺说的是输入。
- **括号嵌套深度上限 256**:超出的模式报 `E0030`「pattern nests too deeply」。⚠ **这道闸不能省,也不能等编译期闸兜底**:2 万层 `((((…))))` 只产出 2 万条指令,「指令数上限」过得去,但解析递归、编译递归与 `AST` 析构递归**全部爆栈**;而栈溢出是 `abort` 不是 unwind,`catch_unwind` 接不住 —— 纯输入会带走整个解释器进程,而不是本节承诺的「编译期 `E0030` 优雅中止」。取 256 是**量出来的**:Go `regexp/syntax` 的 `maxNestDepth` 是 1000,但每层嵌套要占四个栈帧,而调用方线程栈不保证有 8 MiB(本仓门禁的测试线程就是 2 MiB,debug 构建的帧又比 release 大一个量级)。⚠ 这条是**可证的边界**,改闸时必须同步改契约测试的 `deep_nesting_is_rejected_not_crashed`,它钉住闸的两侧。
- **码点坐标系**:一切位置与长度都是码点,不是字节。
- ⚠ **内存形状:本册承诺的是「执行时间对输入线性」,不是「内存对输入 O(1)」。** `RE_FIND_ALL` / `RE_REPLACE` / `RE_SPLIT` 三条路径会为候选表分配**每输入起点 4 字节**的稠密索引 ⇒ 该路径的辅助内存是 **O(输入长度)**。⚠ 这**不改变渐近阶**(输入本身就要 O(n) 的字符数组),它只改变常数;而候选集合本身是 O(不同起点数) ⇒ 把稠密索引换成哈希表**不改变渐近阶、且丢掉 O(1) 查表**,净收益为负 ⇒ 本册**不规定**更小的内存上限,也不承诺「`find_all` 是 O(1) 内存」。⚠ `RE_TEST` / `RE_SEARCH` 两条路径**不分配**该表。
- ⚠ **`RE_REPLACE` 对「越界组号」静默展开成空串**:`$5` 在模式只有 1 个组时**不报任何错**,直接展开成空串。依据:「组存在但没参与这次匹配」与「组号越界」**不可区分** —— 两者都是空串,后者是 Python / Go 的既定行为。对照:`${name}` 见到就 `E0030`;`$999…9`(超出 `usize`)解析失败也 `E0030`。⚠ **Python 在这里报 `invalid group reference`、Go 原样输出 `$5`** —— 三家三种行为,本成员跟的是「空串」那一档。改成报错属**可观察行为变更**,须单独立项,本版**不修**。

### 16.6 示例

```wlwl
IMPORT("wlwl:std.regex", ["RE", "RE_TEST", "RE_SEARCH", "RE_FIND_ALL", "RE_REPLACE", "RE_SPLIT", "RE_GROUP_COUNT"]);
PRINT(RE_TEST(RE("a+"), "aaa"));              // TRUE
PRINT(RE_SEARCH(RE("(a)(b)"), "ab"));         // [matched: ab, start: 0, end: 2, groups: [ab, a, b]]
PRINT(RE_FIND_ALL(RE("a"), "banana"));        // [[matched: a, start: 1, end: 2, groups: [a]], [matched: a, start: 3, end: 4, groups: [a]], [matched: a, start: 5, end: 6, groups: [a]]]
PRINT(RE_REPLACE(RE("(a)(b)"), "ab", "$2$1")); // ba
PRINT(RE_SPLIT(RE(","), "a,b,c"));            // [a, b, c]
PRINT(RE_GROUP_COUNT(RE("(a)(b)")));          // 2
```

### 16.7 设计依据

本命名空间随 v0.11.3 M6 落地,裁决过程见 `docs/plan/addendum-02-regex.md` §3.7。实现走 Thompson 构造 + **Pike VM**,零第三方依赖。

## 17 `std.rand` — 显式播种的随机数

本语言在 `std.rand` 落地之前**零随机数能力**,而这从来不是「不重要」,是 §14.2 记下的「**没想清楚**」:「一个**默认播种**的 RNG 会削弱可复现这个卖点,而一个**要求显式播种**的 RNG 需要先裁决『全局状态 vs 显式传递』—— 那是**设计决策**,不是加几个函数」。[`ADR-0027`](../adr/0027-std-rand-shape.md) 已 **Accepted**,那个争点随之结项。

⚠️ **过渡形态**:本命名空间仍放在 `wlwl:std.*` 里,而 §14 已登记一条方向 —— `ai` / `agent` **降级为官方包,移出 `wlwl:std.*`**。同一条逻辑适用于 `std.rand`,**但前提是「官方包」机制存在,而它当前不存在** ⇒ 现实选择只有「先放 std」。**官方包机制一旦存在,`std.rand` 按 §14 降级。** 这一句必须留着:**不写它,现状就变成永久事实**。

### 17.1 成员

| 签名 | 语义 | 失败 | 层 |
|---|---|---|---|
| `SEED(entropy) -> DICT` | 从**显式熵**造一个 RNG 状态:`["s0": …, "s1": …, "s2": …, "s3": …]`。熵收 `STRING`(按 **UTF-8 字节**)或 `ARRAY`(字节数组,即 `RANDOM_BYTES(n)` 的返回) | 元数错 `E0022`;熵类型错 / 字节超 `0..=255` → `E0030` | R2 |
| `UNIFORM(rng) -> [FLOAT, DICT]` | `[0, 1)` 上的均匀浮点 | 元数**必须是 1 或 3**(2 个实参是 `E0022`,**不是**「`a` 取默认」);`a >= b` 空区间 → `E0030` | R2 |
| `UNIFORM(rng, a, b) -> [FLOAT, DICT]` | **`[a, b)`**(左闭右开)。端点收 `FLOAT` 或 `INTEGER`(内部提升) | 同上 | R2 |
| `INT_RANGE(rng, lo, hi) -> [INTEGER, DICT]` | 整数区间 `[lo, hi)`,**无偏**。可跨负数 | 元数错 `E0022`;`lo >= hi` → `E0030` | R2 |
| `CODEPOINT(rng) -> [STRING, DICT]` | 一个 Unicode **标量值**,返**恰好一个字符**的字符串。排除 UTF-16 代理区 `0xD800..=0xDFFF` 与**永久非字符**(`0xFDD0..=0xFDEF`、尾数位非零的 `U+xFFFE` / `U+xFFFF`) | 元数错 `E0022` | R2 |

**成员数是承诺**:恰好 4 个。

**非目标(成员数守卫把这条钉住)**:**不提供** `NORMAL` / 泊松 / 指数等**分布族** —— 需要别的分布的调用方自己从均匀分布构造(极坐标法 / 逆变换法),那是调用方的算法选择,不是标准库该替他定的。**没有 `SET_SEED()`,没有「全局当前 RNG」,没有任何隐藏状态。** `INT_RANGE` 不是「一种分布」,它是均匀分布在整数域上的成员。

### 17.2 为什么抽签成员返回 `[值, 新状态]`

**这一节是本成员最容易踩空的地方**,所以它写进规范而不只是 ADR。

**`DICT` 是不可变的**(语言规范 §2.1),而 `SET_PROP` / `INDEX_SET` 对字典也是「返回**新**字典、**接收者不变**」。⇒ 若抽签成员只返回标量,**状态永不前进**:同一个 `rng` 值反复调用拿到的是**同一个数**(实测 2000 次全部相同)。

所以状态与值**一起**返回,用语言**已有**的数组模式绑定消费:

```wlwl
IMPORT("wlwl:std.rand", ["SEED", "INT_RANGE"]);
LET(r0, SEED("fixed-seed"));
LET([v1, r1], INT_RANGE(r0, 0, 100));
LET([v2, r2], INT_RANGE(r1, 0, 100));
PRINT(v1, v2);        // 15 99
```

**收益是「忘了推进状态」不再是可能的惯用写法** —— 拿不到值,除非同时拿到新状态。契约表 `two_draws_advance_the_state` 是这条的守卫。

### 17.3 算法(第三方必须能复现冻结向量)

- **播种**:熵字节 → **FNV-1a-64** 折叠成一个 `u64` → **SplitMix64** 展开成 **256 bit 状态**(4 × `u64`)。⚠️ 展开后**四字全零**则按固定常数 `0x9E3779B97F4A7C15` 再展开一次 —— xoshiro 的**全零态是吸收态**(恒输出 0),不守卫就会**静默**退化成一条无意义序列,而冻结向量照样绿。读状态时**再挡一次**(用户可以手写那个字典),报 `E0030`。
- **生成**:**xoshiro256++**。
- **浮点**:`UNIFORM` 取 `(next_u64() >> 11) as f64 * 2^-53`。右移 11 位拿到**恰好 53 bit** 尾数 ⇒ 结果**可被保证**落在 `[0, 1)`,而不是「恰好不会返 1.0」。`[a, b)` 的右端不包含也是**被保证**的:实现夹一次(当 `a` 远大于区间宽度时,`(b - a)` 的量级会低于半个 ulp,浮点舍入会把结果顶到 `b`)。
- **整数**:走**无偏取模拒绝**(截断点 `2^64 - (2^64 mod range)`,抽到以上就重抽),**绝不经过 `FLOAT`** —— 浮点只有 53 bit 尾数,中转会引入舍入偏差,且对大区间**静默收窄上界**(不报错、只给偏斜序列)。
- ⚠️ **状态字按 `i64` 位模式存放**:xoshiro 状态是 `u64`,而 wlwl 的 `INTEGER` 是 **`i64`** ⇒ 显示为**有符号数**(多数是负数),读回时按**位**重解释。逐字节往返无损,用户手写正数状态(`42` → `42`)同样合法。这是「不加内建类型」唯一的代价。

### 17.4 可复现性与熵源归属

- **熵源是操作系统,但依赖必须写在程序文本里**:`SEED` **内部不读 OS 熵源**。要真随机就去 `std.encode` 的 `RANDOM_BYTES` / `RANDOM_HEX` 取,显式传进来 —— `std.encode` 是源、`std.rand` 单向消费它。**对一个把可复现当卖点的语言,「依赖可见」本身就是功能。**
- **`std.rand` 不做密码学随机**:那是 `RANDOM_BYTES` 的活。两个命名空间、两套用途,**不合并**。口令 / 密钥材料一律走 `std.encode`。
- **换算法是 breaking**:同一熵的前若干个输出**逐字节**由契约表冻结(期望值取自**独立的第二实现**,不是从本实现导出)。算法换掉那条就红 ⇒ 必须同批更新向量并在 CHANGELOG 显式记录,**不能悄悄换**。

### 17.5 已知限制

- **亚毫秒 / 非 64 位状态**:状态固定 4 × `u64`,不可配置。
- **`CODEPOINT` 返回 `STRING` 而不是码点数值**:语言层没有「码点 → 字符串」的成员,返回数值形态要用户自己处理,而调用方几乎总要用它拼字符串。
- **`SEED` 的熵用 FNV-1a-64 折叠变长熵**:熵**已经均匀**之后的一次归一化,不承担安全职责(真熵来自 OS)。它让「同一个熵」在 `STRING` 与 `ARRAY` 两种字面量下**各有一种确定解释**,而不是取决于偶然。
- **分布统计不在契约表里**:它们是多项分布,写进 `cargo test` 只会得到一条必然偶尔变红的假门禁;统计断言在 `wlwl-std` 单测里用 4σ 容差,「无偏」的**确定性**证明(`2^64 mod 7 = 2` ⇒ 最高两个 `u64` 必然被拒)也在那里。

### 17.6 设计依据

本命名空间随 v0.11.3 M6 落地;形状与「显式 seed + 可注入熵源」的裁决见 [`ADR-0027`](../adr/0027-std-rand-shape.md)(`ADR-0026` G3 是它的前置条件)。算法为手写(哈希 / SplitMix64 / xoshiro256++),零第三方依赖。

## 18 `std.env` — 进程环境与命令行参数

| 签名 | 语义 | 失败 | 层 |
|---|---|---|---|
| `ENV_GET(name, default?) -> STRING \| NULL` | 取环境变量。**不存在且无缺省 → `NULL`** | 元数 > 2 / 类型错 → `E0022` / `E0030` | R2 |
| `ENV_SET(name, value) -> NULL` | **仅本进程可见,不写磁盘** | 同上 | R2 |
| `ENV_KEYS() -> ARRAY[STRING]` | 全部键名,**排序后返回** | 元数 ≠ 0 → `E0022` | R2 |
| `ARGS() -> ARRAY[STRING]` | 命令行参数,**含 `argv[0]`** | 元数 ≠ 0 → `E0022` | R2 |

**`ENV_KEYS()` 排序是规范性要求,不是实现偏好**:宿主环境表的迭代序**逐次可能不同**,而**可复现是本语言卖点第一条** ⇒ 不排序就等于把一个不稳定值暴露给调用方。契约表钉死「两次调用逐字节相同」。

**「没设」与「设为空串」是两件事** ⇒ `ENV_GET` 缺省返 `NULL` 而不是 `""`;分不开就没法读 `.env` 覆盖后的真实状态。

🔴 **`ENV_SET` 的写入不会到达 `PROCESS_RUN` 的子进程**。本命名空间的环境是**启动时由宿主交给程序的那份快照**,而子进程继承的是**操作系统那份真环境**(§19 的「追加到继承来的环境」说的就是后者)。⇒ `ENV_SET("FOO", "bar")` 之后 `PROCESS_RUN([…, "printenv", "FOO"])` **读不到** `FOO`;要把值传下去只能走 `PROCESS_RUN` 的 `opts.env`。⚠ **这是「如实成文」而不是「实现继承」**:让子进程继承本节的环境是**行为变更** —— 既有程序的子进程环境会突然多出变量,且子进程的环境会变成「**此前 `ENV_SET` 了多少次**」的**隐式函数**;而本语言明写「**显式值传递、无隐藏状态**」,`opts.env` 正是那条**显式**通道。⇒ 是否改为继承,留作独立的产品裁决。缺陷 D13-067。

⚠ **一条要订正的旧论证**:此前本节把「不继承」的理由写成「子进程环境将取决于**语句执行顺序**,与可复现冲突」。**这一条其实不成立** —— 确定性程序的语句顺序是固定的,跨次运行仍然可复现。**真正的反对理由是上一段的「隐式状态」**,不是语句顺序。⚠ 保留这个订正是因为:论证一旦站不住,将来有人会用「我的程序是确定性的」把它辩过去,而那恰好是**不成立**的反驳。

`ARGS()` **含程序自身路径**(`argv[0]`),与 Go `os.Args` / Python `sys.argv` 同款;要「用户传进来的参数」就 `SLICE(ARGS(), 1)`。

与 `std.time` 的立场差异(写明免得下一个人「顺手统一」):`std.time` **直读系统时钟**,因为本语言**就是**时间的源;而环境的源是**启动时由宿主交给程序的** —— 它已经是「传进来的值」,所以本命名空间只读宿主的一个字段,不碰 OS API。

### 18.1 示例

```wlwl
IMPORT("wlwl:std.env", ["ENV_GET", "ENV_SET", "ENV_KEYS", "ARGS"]);
ENV_SET("WLWL_PROBE_KEY", "v1");
PRINT(ENV_GET("WLWL_PROBE_KEY"));     // v1
PRINT(ENV_GET("WLWL_PROBE_MISSING"));  // NULL
PRINT(LEN(ENV_KEYS()));                // 非 0(个数随宿主环境而变)
PRINT(LEN(ARGS()));                    // ≥ 1(含 argv[0])
```

### 18.2 设计依据

本命名空间随 v0.11.3 M6(ADDENDUM-06)落地。缺陷 D13-067(`ENV_SET` 不被子进程继承)。

## 19 `std.process` — 不经 shell 跑子进程

| 签名 | 语义 | 失败 | 层 |
|---|---|---|---|
| `PROCESS_RUN(argv, opts?) -> DICT` | 返 `[code: INTEGER, stdout: STRING, stderr: STRING]`。🔴 **`stdout` / `stderr` 是 `from_utf8_lossy` 的结果 —— 非 UTF-8 字节被换成 `U+FFFD`,不可逆**。⚠ 这与本册的「不做有损替换」**冲突**:成员表把这两个键定为 `STRING`,改成 `ARRAY[INTEGER]` 并集属**破坏性变更**。**要拿无损字节就走文件**:`WRITE_BYTES` + `READ_BYTES`。**不经 shell** | 元数 / 类型错 → `E0022` / `E0030`;起不来 / 超时 → `ERR([kind: "ProcessError", op, reason])` | R2 |
| `PROCESS_ID() -> INTEGER` | 本进程 id | 元数 ≠ 0 → `E0022` | R2 |

`opts` 可含 `timeout`(毫秒)、`cwd`、`env`(**追加**到继承来的环境,同名键覆盖 —— 不是「替换整份环境」)。未知键(如 `shell`)→ `E0030`,**显式拒绝而不是忽略**:「忽略」与「禁止」在安全上是两件不同的事。

🔴 **`timeout` 的两半必须分开说**:

- **杀掉谁**:Windows 上杀**整棵后代树**(`taskkill /T /F`);**POSIX 上只发信号给直接子进程**,**后代不被杀**。本规范**不承诺**「杀掉整棵进程树」。
- **多久返回**:**有界** —— 杀掉子进程后**至多等一个宽限期**,读取线程仍未结束就**丢弃句柄**(分离线程)并返回 `ERR`。⚠ 这一半**两个平台都兑现**,而且**恰恰因为它不兑现,超时在 POSIX 上就等于「等到孙进程自己结束」**:孙进程握着管道写端 ⇒ 管道不 EOF。⚠ Windows 上这条曾被 `taskkill /T /F` **掩盖**,所以既有的两个超时契约在 Windows 上对该缺陷完全无感。缺陷 D13-061 / D13-037。

⚠️ **`argv` 必须且只接受 `ARRAY[STRING]`。规范显式禁止字符串形态。** 拼一个字符串再让系统解析就是**命令注入**(`; rm -rf /` 会真的被执行)⇒ 本规范**没有** `SYSTEM` / `exec("sh -c")` 这种形态。这条是本成员存在的**安全理由**,契约表有一条「元字符不被解释」的守卫钉它。

⚠️ **非零退出码是正常结果,不是失败** —— 它就是 `code` 字段里的一个整数。

⚠️ **它是阻塞的(与 `std.time::SLEEP` 同源)。** 运行时是**单线程协作式**(eval 侧无 `thread::spawn`,调度是一个 run queue)⇒ `PROCESS_RUN` 等子进程期间**不调度任何其他任务**,等价于把整个作用域停住。这条**语言级事实**必须成文,因为调用方若在气泡 / `SCOPE` 里用它,会以为别的任务还能跑。`std.net` 的阻塞式 HTTP 是**同源隐忧**。

**没有 `PROCESS_EXIT_CODE` 成员**:它需要「上一条 `PROCESS_RUN` 的退出码」—— 那是**跨调用隐藏可变状态**,与本语言「显式值传递、无隐藏状态」的纪律冲突;而 `code` 字段**已经在返回值里**。

### 19.1 示例

```wlwl
IMPORT("wlwl:std.process", ["PROCESS_RUN", "PROCESS_ID"]);
PRINT(PROCESS_ID());   // 本进程 id(具体值随进程变化)
```

### 19.2 设计依据

本命名空间随 v0.11.3 M6(ADDENDUM-06)落地;阻塞形态来自「单线程协作式调度」这一运行时事实。缺陷 D13-037(POSIX 超时不杀后代)、D13-061(timeout 两半)、D13-064(`stdout` / `stderr` 的有损替换)。

## 20 `std.compress` — Zstandard 与 DEFLATE 家族

| 签名 | 语义 | 失败 | 层 |
|---|---|---|---|
| `ZSTD_COMPRESS(data, level?) -> ARRAY[INTEGER]` | RFC 8878。`level` 缺省取库默认(3),范围 `-131072..=22`(负数 = 更快、压缩比更低) | 元数错 `E0022`;非字节数组 / `level` 越界 `E0030` | R2 |
| `ZSTD_DECOMPRESS(data) -> ARRAY[INTEGER]` | ⚠ 输出上限 `MAX_DECOMPRESSED` = **64 MiB** | 帧非法 / 截断 / **超上限** → `ERR` | R2 |
| `ZLIB_COMPRESS(data, level?) -> ARRAY[INTEGER]` | RFC 1950(zlib 包装,Adler-32)。`level` 缺省库默认(6),范围 `0..=9` | 同上 | R2 |
| `ZLIB_DECOMPRESS(data) -> ARRAY[INTEGER]` | 校验和不符 → `ERR` | 同上 | R2 |
| `GZIP_COMPRESS(data, level?) -> ARRAY[INTEGER]` | RFC 1952(CRC32)。`level` 同上 | 同上 | R2 |
| `GZIP_DECOMPRESS(data) -> ARRAY[INTEGER]` | CRC32 不符 → `ERR`。🔴 **多成员全解 + 流后尾随字节 `ERR`**(RFC 1952 §2.2) | 同上 | R2 |
| `DEFLATE_RAW_COMPRESS(data, level?) -> ARRAY[INTEGER]` | RFC 1951 **裸流**(无容器、无校验和),给自定义容器用 | 同上 | R2 |
| `DEFLATE_RAW_DECOMPRESS(data) -> ARRAY[INTEGER]` | ⚠ **没有校验和 ⇒ 数据损坏不可检测**,截断输入可能解出一段前缀正确的垃圾而不报错 | 同上 | R2 |

`data` 的形状是 **`ARRAY[INTEGER]`**(元素 `0..=255`)—— **继承 `std.fs` §2.2 的裁决,本节不重新裁决**。两份各定一次就是 std 里两种字节表示,那是最难还的债。

⚠ **`level` 越界是 `E0030`,绝不静默 clamp** —— clamp 会让「我传了 9」和「我传了 999」产出同一个结果,那是在骗调用方。

### 20.1 压缩不是加密

规范**必须**明写这一条(与 §11.5「哈希不是加密」同款体例):压缩**不提供机密性**,任何拿到输入的人都能解开。调用方若把 gzip 当「藏起来」用,那是**安全缺陷**而不是用法。机密性走传输层加密。

### 20.2 解压输出上限是硬边界

压缩炸弹(几十字节压缩出几 GB)是**真实攻击面**:不设上限等于把调用方的内存交给攻击者。⇒ 解压成员输出超过 **`MAX_DECOMPRESSED` = 64 MiB** 时返回 `ERR([kind: "CompressError", op, reason])`,**不是** OOM、**不是** panic。

⚠ **它同时是一条已知限制**:本册明确**不做流式 API**,所以 > 64 MiB 的单个载荷**没有解压路径** —— 要处理它得先解压到文件,或等流式成员落地。

🔴 **闸封的是 `len` 不是 `capacity`**。合法载荷压在 64 MiB 上限时读取的 `Vec` 仍会翻倍 ⇒ 实测峰值约为文档上限的 **2×**。⚠ **没有改**:修法(读前预分配)会让**每一次**解压都先占 64 MiB,比现在更坏。已登记待独立立项。缺陷 D13-049。

🔴 **`GZIP_DECOMPRESS` 吃多成员,`ZLIB_DECOMPRESS` / `DEFLATE_RAW_DECOMPRESS` 不**(缺陷 D13-048)。gzip 格式本身支持多个成员串联(RFC 1952 §2.2),故全部解开;**流后尾随字节 ⇒ `ERR`**。⚠ zlib(RFC 1950)**没有成员概念**,Python 的对应行为叫 `unused_data`(实测 `zlib.decompress(stream + b"JUNK")` **正常返回**)⇒ 对它做同样的拒绝会是**发明需求**,有意不做。

### 20.3 确定性口径:往返与交叉核对,而**不是**冻结输出字节

**压缩输出的字节是实现定义的** —— 同一份输入,zlib / miniz_oxide / zlib-ng 的 DEFLATE 流**互不相同**(块划分、lazy matching、熵编码选择都是实现自由)。⇒ 规范与契约表**不**冻结本实现的输出字节:那样只会冻结某个后端的内部选择,且**没有任何独立实现能产出同样的字节**。

真正成立、且被契约表钉住的是三条:

1. **往返**:`COMPRESS` → `DECOMPRESS` 恒等于原输入(逐字节);
2. **独立向量交叉核对**:契约表用 **Python 标准库**(`gzip.compress(mtime=0)` / `zlib.compress`)产出的向量喂给我们的解码器 —— 证明解码器是**按 RFC 实现**的,而不是「只会解开自己写的东西」(后者是一种常见的自证循环缺陷:用本成员压再解,永远「通过」);
3. **确定性**:同 `(data, level)` 两次压缩产出**逐字节相同**。

### 20.4 示例

```wlwl
IMPORT("wlwl:std.compress", ["ZSTD_COMPRESS", "ZSTD_DECOMPRESS", "ZLIB_COMPRESS", "ZLIB_DECOMPRESS", "GZIP_COMPRESS", "GZIP_DECOMPRESS", "DEFLATE_RAW_COMPRESS", "DEFLATE_RAW_DECOMPRESS"]);
LET(data, [104, 101, 108, 108, 111]);
PRINT(LEN(ZSTD_COMPRESS(data)));        // 14
PRINT(ZSTD_DECOMPRESS(ZSTD_COMPRESS(data)));  // [104, 101, 108, 108, 111]
PRINT(ZLIB_DECOMPRESS(ZLIB_COMPRESS(data)));  // [104, 101, 108, 108, 111]
PRINT(GZIP_DECOMPRESS(GZIP_COMPRESS(data)));  // [104, 101, 108, 108, 111]
PRINT(DEFLATE_RAW_DECOMPRESS(DEFLATE_RAW_COMPRESS(data)));  // [104, 101, 108, 108, 111]
```

### 20.5 层与依赖

R2 全员。**这是本工作区第一次引入压缩依赖**(`zstd` + `flate2`),取舍推导见 `addendum-07` §3.1;许可证已实测放行(`deny.toml` 未改)。

## 21 `std.net` — 网络

**只支持明文 `http:`**,**不做 TLS** —— 调用方自备隧道 / 反代。**零新增第三方依赖**:分组件走 RFC 3986 通用语法,解析与引用求解全部手写。

| 签名 | 语义 | 失败 | 层 |
|---|---|---|---|
| `URL_PARSE(url) -> DICT` | 拆成 `{scheme, userinfo, host, port, path, query, fragment}`。**纯解析,不发请求** | 不满足 `URI-reference` 的 ABNF → `E0030` | R2 |
| `URL_JOIN(base, rel) -> STRING` | 相对引用 → 绝对引用(RFC 3986 §5,**strict 模式**) | 同上 | R2 |
| `HTTP_GET(url, headers?, timeout?) -> DICT` | `{status, headers, body}`。**明文 `http:` only**。🔴 **`body` 的形状随内容走**:合法 UTF-8 ⇒ `STRING`;否则 ⇒ `ARRAY[INTEGER]`(元素 `0..=255`,逐字节无损,继承 §2.2 / §11.2 的字节裁决)。⚠ 8 MiB 的非 UTF-8 体会展开成约 840 万元素(与 `READ_BYTES` 同族放大)。**大载荷的正解仍是让它待在文件里** | 网络失败 → `ERR([kind: "HttpError", op, reason])` | R2 |
| `HTTP_POST(url, body, headers?, timeout?) -> DICT` | 同上 | 同上 | R2 |
| `HTTP_REQUEST(method, url, body?, headers?, timeout?) -> DICT` | 其余动词的通用形态 | 同上 | R2 |

⚠ **`HTTP_*` 是阻塞原语**:执行期间**不调度任何其他任务**,整个 `SCOPE` 冻结。**不要在同一个 `SCOPE` 里并发发多个 HTTP 请求** —— `SPAWN` 了也还是排队,那是**错的写法**不是低效的写法。

### 21.1 `URL_PARSE` 的返回值

- **七个键恒定存在**。某组件**未出现**时取值为 `NULL`,**出现但为空**时取值为 `""` —— 这个区别是 RFC 3986 §5.3 末段的**规范性**要求:分隔符没出现是 undefined,分隔符出现但后面立刻是下一个分隔符或结尾是 empty。混同两者就分不开 `query` 与 `path`,`remove_dot_segments` 会去动 `query` 里的 `/./`(RFC §5.2.4 末段的 Note 明写这个坑)。
- `port` 是 **INTEGER**(便于直接比较),`NULL` 表示未指定或为空端口。
- `host` 对 IPv6 literal **保留方括号**(`"[::1]"`),好让调用方能无损拼回。

⚠ **`userinfo` 必须单独拆出,不得丢弃。** RFC 3986 §7.6 用 `ftp://cnn.example.com&story=breaking_news@10.0.0.1/` 证明:丢掉这一段会让人以为 host 是 `cnn.example.com`,而它实际是 `10.0.0.1`。解析器替调用方吃掉 userinfo,等于**藏起一个已知的语义攻击面**。

### 21.2 「非法 URL」的边界

RFC 3986 的解析是**宽容**的 —— 绝大多数字符串都能解析成某种引用。所以「非法 → `E0030`」必须有能写进契约表的边界。判据是 **不满足 `URI-reference` 的 ABNF(RFC 3986 Appendix A)即非法**:

| 情形 | 结果 |
|---|---|
| 任何 ASCII 控制字符或空格 | `E0030` |
| `:` 出现在首个 `/?#` 之前,但其前缀不是合法 `scheme` | `E0030` |
| 裸 `%`(后面不是两个十六进制数字) | `E0030` |
| 组件里有超出该组件字符集的字符 | `E0030` |
| `[` / `]` 不成对,或出现在 authority 之外 | `E0030` |
| `port` 非 `*DIGIT`,或数值 > 65535 | `E0030` |
| **空 host**(如 `http://`) | **接受** |
| **无 scheme**(如 `//g`、`a/b`) | **接受** |

最后两行是**刻意宽松**的:判成非法会让 `URL_JOIN` 无法处理 RFC §5.4 里 `//g`、`""` 这类正常输入。

**明确不校验 IPv6 地址的完整文法** —— RFC 3986 §3.2.2 自己说 IPv6 文法「很难指定」并把它委派给 RFC 3513。本实现只做保守检查(方括号内非空且字符集合理),不解析成二进制地址。**这是已知限制,不是遗漏。**

### 21.3 `URL_JOIN` 的 strict 模式

RFC 3986 §5.4.2 的 `http:g` 一行给了**两个**结果:strict 解析器得 `http:g`,向后兼容的解析器得 `http://a/b/c/g`。本实现取 **strict** —— RFC 自己把前者排在前面。⚠ **WHATWG URL 取的是另一个**,两者**不等价**,跨规范对照时不要互相套用。

`URL_JOIN` **不做**百分号解码、host 小写化、scheme 规范化 —— 那些是 RFC §6.2 的**语法 / scheme 规范化**,是**另一个语义**,不在本成员内。

### 21.4 `HTTP_*` 的形态

- **只支持明文 `http:`**,**不做 TLS**。`https://` 返回 `ERR`,不做「简易封装」。
- **超时必填且有保守默认**:缺省 **30 000 ms**,调用方可覆盖。⚠ 这条**不是建议而是契约** —— 阻塞时长由墙钟决定(不像 `SLEEP` 有虚拟时钟可测),忘写就是永久挂住整个程序。
- 🔴 **`timeout` 是「整条请求」的预算上限,不是每次 I/O 的时限**。拨号、重定向轮次、每一次 `read` 都从**同一份剩余预算**里扣,不再各拿一份完整的 `dur` —— 否则「多地址连接 + 慢速滴流响应」可以把总耗时放大成 `N × dur + Σ 每次读`。⚠ **DNS 解析不在闸内,且这是已知缺口不是疏忽**:域名解析是**不可中断**的阻塞系统调用(标准库无取消手段;另起线程解决不了「线程仍会挂住」,还会泄漏线程)。⇒ 闸**从 DNS 返回之后**才开始计;若 DNS 真的挂住,本函数仍会挂住。要真正覆盖只能换可取消的解析器,**那会引入第三方依赖**,与「零新增依赖」冲突 —— **需要一次独立裁决**。⚠ 实现在预算耗尽时**返回超时错误而不是把 socket 超时设成 0** —— 零值会让系统调用报 `EINVAL`,而调用点的 `let _ =` 会把失败静默吞掉,「超时」就退化成「无超时」。
- **不自动跟随重定向**。3xx 原样返回,`Location` 在 `headers` 里,由调用方决定。自动跟随可能把请求带到非预期 host(SSRF 面)。
- **不支持的写法一律 `ERR`,绝不「尽力解析」**。请求走私 / 响应拆分的根因都是「宽容地解析有歧义的框架」:

| 情形 | 结果 |
|---|---|
| `Transfer-Encoding`(**含 `chunked`**) | `ERR` —— **不解码**,这是走私向量 |
| HTTP/1.1 既无 `Content-Length` 又无 `Connection: close` | `ERR`(不猜 body 长度)。**HTTP/1.0 不适用此条** —— 见下 |
| 重复的 `Content-Length`(即便值相同) | `ERR` |
| 响应头块 > **64 KiB** | `ERR`(**不是**无界分配) |
| 响应体 > **8 MiB** | `ERR`(**不是** OOM) |
| obs-fold(头行以空白开头) | `ERR`,**不合并** |
| 头值含裸 CR / LF | `ERR` |

🔴 **HTTP/1.0 的 close 定界是例外**。RFC 9112 §6.3 第 8 条其实**不分版本**,本规范**刻意只给 HTTP/1.0 开口子**:HTTP/1.0 没有别的定界手段,在那里拒绝等于零安全收益;而 **HTTP/1.1 无长度无 close 是真正的歧义**(请求走私向量),收紧是刻意的。⇒ 契约里 `/nolength`(1.1)与 `/close10`(1.0)**只差版本号**,这一条边界被配对夹具直接钉住。缺陷 D13-041。

⚠ 另有一处**已知缺口、不在本版裁决范围**:实现未特判 **1xx / 204 / 304**,而按 RFC 9112 §6.3 第 1 条这些响应**本就无 body**(无论头里写了什么)。⇒ HTTP/1.1 的 `204` 若不带 `Content-Length` 现在会 `ERR`。已登记待独立立项。

- **失败是 `ERR` 值不是原生诊断**:网络不通是**运行期可预期的业务分支**(会重试、会降级、会记日志),做成原生诊断会让整个运行**终止**,调用方连重试都做不到。载荷是 `{kind: "HttpError", op, reason}`,与 `std.encode` 的 `DecodeError` 同款(把「参数错」与「网络不通」分成两种 kind,调用方才分得清)。
- **元数错 `E0022` / 实参类型错 `E0030`** —— 那两条是**程序员错误**,走原生诊断。头字段名不是合法 token、或头值含 CR/LF,也是 `E0030`(后者是头注入)。
- ⚠ **阻塞形态是带条件冻结的**:冻结前提是「真 state-machine 续跑仍未落地」。前提一旦不成立,阻塞的代价就没有必要再承担,**必须重新裁决**(以及随它而来的 30 s 超时兜底)。守卫测试会在前提不成立时转红。

**明确不做**:TLS / WebSocket / SSE / gRPC / 连接池 / 重试策略 / 自动重定向 / 代理 / cookie jar / 流式响应体。

### 21.5 示例

```wlwl
IMPORT("wlwl:std.net", ["URL_PARSE", "URL_JOIN"]);
PRINT(URL_PARSE("http://u:p@example.com:8080/a/b?q=1#f"));  // [scheme: http, userinfo: u:p, host: example.com, port: 8080, path: /a/b, query: q=1, fragment: f]
PRINT(URL_JOIN("http://example.com/a/b", "../c"));           // http://example.com/c
```

### 21.6 设计依据

本命名空间随 v0.11.3 M6(ADDENDUM-05)落地;裁决全文见 [`ADR-0028`](../adr/0028-std-net-shape.md)(**Accepted**)。零新增第三方依赖:明文 HTTP/1.1 走 `std::net::TcpStream` + 系统解析器。缺陷 D13-040(`timeout` 预算)、D13-041(HTTP/1.0 的 close 定界)、D13-043(`body` 形状随内容走)。

## 22 `std.result` — `RESULT` 组合子

`OK` / `ERR` 的控制流组合子。**R1 门面 + R2 内核**,零 kernel 泄露。

⚠ 本章的头号条款是 §22.2:**这三个成员的 `ERR` 分支不在成员体内执行**,正确性由语言规范 §8.2 提供。读成员语义前必须先读 §22.2。

### 22.1 成员表

| 签名 | 语义 | 失败 | 层 |
|---|---|---|---|
| `RESULT_MAP(r, f) -> RESULT` | `OK(x)` → `OK(f(x))`;`ERR` → 原样透传(`f` 一次都不被调用) | 实参非 `RESULT` → `E0030` | R1 |
| `AND_THEN(r, f) -> v` | `OK(x)` → `f(x)`(`f` 可返回 `OK` **或** `ERR`);`ERR` → 原样透传 | 同上 | R1 |
| `FLATTEN(r) -> RESULT` | `OK(OK(x))` → `OK(x)`;`OK(ERR(e))` → `ERR(e)`;`ERR(e)` → 原样透传;`OK(x)`(非 `RESULT`)→ `OK(x)` | 同上 | R1 |

三条**全是固定元数**,元数错由解释器报无名前缀的 `E0022`。

`FLATTEN` 只剥**一层**:`FLATTEN(OK(5))` 得 `OK(5)` 而不是 `5` —— 返回的仍然是 `RESULT`,只是不再是嵌套的。

⚠ **`AND_THEN` 不补 `OK` 外壳,这是字面语义。** `OK(x)` → `f(x)`,结果**就是** `f` 的返回值。`f` 一旦返回**裸值**,链就断了(`AND_THEN(OK(3), FUN((x), +(x, 1)))` 得 `4`,不是 `OK(4)`)。调用方有责任让 `f` 返回 `RESULT`(与 Rust `and_then` 要求 `F: FnOnce(T) -> Result<U, E>` 同款)。契约测试对此有**专门一条**。

⚠ **`RESULT_MAP` 在 `f` 返回 `ERR` 时产出**嵌套**的 `OK(ERR(..))`,不展平** —— 这正是 `FLATTEN` 存在的理由。

⚠ **命名裁决:`MAP` 不可用。** `std.collection` 自 v0.10 起即导出 `MAP`,两者同作用域 `IMPORT` 会硬撞 **`E0021`**(不遮蔽、直接报错)。故本成员取 `RESULT_MAP`,与本命名空间后续的 `RESULT_ALL` / `RESULT_ANY` 成族。`AND_THEN` / `FLATTEN` 对附录 A 全表无冲突。

### 22.2 三条成员的 `ERR` 分支由 §8.2 提供,不在成员体内

语言规范 §8.2:**任何不在消费者注册表(§8.3)内的函数调用——内建或用户定义——若任一实参求值为 `ERR`,则该调用不求函数体,直接以该 `ERR` 为调用结果。** §8.3 明文「**本表不新增条目**」。

⇒ 实参为 `ERR` 时,本节任何成员的**体都不执行**,§8.2 已把那个 `ERR` 原样返还。而「原样返还」恰好就是这三条在 `ERR` 上应有的行为 —— 所以 `ERR` 分支是**免费的**,不是被实现的。**本节不得**以任何形式绕过这条:不新增注册表条目、不把 `ERR` 藏进容器走私、不改 §8.2 传播语义。

实现侧证据:eval 侧的 `dispatch_call` 里 §8.2 的传播块位于 `user_fn` 分派**之前**,判据是 `is_err_consumer(name)`——**被调用者的名字**。故 R2 原生内核与 R1 门面**同样**看不到 `ERR` 实参。

⚠ 因此三条成员的源码里**没有** `IF(IS_ERR(r), r, …)` 外层护栏 —— 写上它是永久不可达的死代码,且会让人误以为 `ERR` 分支是成员体内实现的。`FLATTEN` 的**内层** `IS_ERR(i)` 是活的:`FLATTEN(OK(ERR(e)))` 的外层实参是 `OK`,体照常执行,内层才可能取出 `ERR`。

**§8.2 旁路用例**:组合子**接不到**非消费者拿到的 `ERR`,所以它们**不可能**成为旁路。

**「装盛」与「消费」的分界线(规范性)**:标准库成员**从容器里**读出 `ERR` 并在**自己作用域内**用注册表消费者处理它,**不构成旁路**;把一个 §8.2 **本该传播**的**裸 `ERR`** 藏进 `DICT` / `ARRAY` 交给别的函数,**构成旁路**。判据:

| 做法 | 判定 | 为什么 |
|---|---|---|
| 成员收到数组(数组不是 `ERR`),用 `IS_ERR` / `IS_OK` 逐元素判定 | **不是旁路** | `ERR` 全程在容器里(§8.5 明文「装盛不是消费」),**从未跨越调用边界**;调用方自己写循环也能做到,成员未新增任何能力 |
| 闭包收到容器里的某个 `ERR` **元素** | **撞墙**(得 `E0102`) | 逐元素调用本身就是一次带 `ERR` 实参的调用 |
| `f(容器)` 传入的是裸 `ERR` 却被藏进容器绕过 | **是旁路** | 调用方自己做不到这件事,成员凭空给了它消费裸 `ERR` 的能力 |

⇒ 结论:`RESULT_ALL` / `RESULT_ANY` **刻意不接受谓词参数**,正是为了不踩第二行。

### 22.3 三条**不可实现**的成员与调用方惯用法

下列成员的 `ERR` 分支要**做事**(取载荷 / 调 fallback / 换载荷),而 §8.2 保证体不执行 ⇒ **任何实现层都做不到**,R1 门面与 R2 内核 alike。**本规范不提供它们**,调用方按 §8.2 推论「消费 `ERR` 必须在拥有它的作用域内进行」改写:

| 想要 | 不可用成员 | 调用方惯用法 |
|---|---|---|
| `ERR(e)` → `fallback(e)` | `OR_ELSE(r, fb)` | `LET(y, IF(IS_ERR(r), fb(ERR_PAYLOAD(r)), r));` |
| `ERR(e)` → `ERR(f(e))` | `ERR_MAP(r, f)` | `LET(y, IF(IS_ERR(r), ERR(f(ERR_PAYLOAD(r))), r));` |
| `ERR(_)` → `ERR(e)` | `ERR_OR_ELSE(r, e)` | `LET(y, IF(IS_ERR(r), ERR(e), r));` |

三条惯用法成立的前提是 **`IF` 三参短路:未被选中的一支完全不求值**(含副作用)。实测:`IF(IS_ERR(<ERR>), OK(1), g(p))` 的 `g` 调用计数为 `0`。若 `IF` 求值了 else 支,`OR_ELSE` 的惯用法就会在 `r` 为 `OK` 时也调用 `fb`。

`ERR_PAYLOAD` 对 `OK` 或非 `RESULT` 产生 `E0030`,故惯用法须放在 `IS_ERR` 判定的 then 支内(短路保证 `OK` 时不触发)。

### 22.4 `UNWRAP` 的诊断条款

`UNWRAP` 是**全局内建**,本命名空间不复刻。它的失败诊断是**专用码 `E0100` PANIC**(`cause` = 载荷),**不是** `E0102`(后者是「`ERR` 逃逸到顶层」)。两者含义不同:调用方要能分清「我没接住」与「我没消费」。契约测试对这条做**否定断言**。

**明确不做**:`OPTIONAL` 类型 / `?` 语法糖 / 宏 / 改 §8.3 注册表 / 改 §8.2 传播语义。

### 22.5 `NULL_OR` / `RESULT_ALL` / `RESULT_ANY`

| 签名 | 语义 | 失败 | 层 |
|---|---|---|---|
| `NULL_OR(v, default) -> v` | `NULL` → `default`;**其余一切原样返回**,含 `""` / `[]` / `0` / `FALSE` / `OK(..)` | 非 `RESULT` 实参 → `E0030` | R1 |
| `RESULT_ALL(rs) -> RESULT` | 全部 `OK` → `OK(解包后的数组)`;任一 `ERR` → **第一个** `ERR`(短路,不看后续) | 非数组 / 非 `RESULT` 元素 → `E0030` | R2 |
| `RESULT_ANY(rs) -> RESULT` | 任一 `OK` → **第一个** `OK`(**原样,不拆壳**,短路);全 `ERR` → **第一个** `ERR`;**空数组** → `OK(NULL)` | 同上 | R2 |

**空数组口径**(两条相反,各有惯例依据):

- `RESULT_ALL([])` = `OK([])` —— 空集**真**(与 Rust `collect::<Result<Vec<_>, _>>()` 对空迭代器同款)。
- `RESULT_ANY([])` = `OK(NULL)` —— 「**没有候选**」不是错误,故不编造 `ERR`;载荷是 `NULL` 而不是 `OK([])`,因为这里要的是**一个**结果而不是一批。

⚠ **`NULL_OR` 不是 `UNWRAP_OR`。** `NULL` 不是 `ERR`,所以:

- `NULL_OR("", 5)` = `""`、`NULL_OR([], 5)` = `[]`、`NULL_OR(0, 5)` = `0` —— 三者**互不相等**,契约测试逐个点名;
- `NULL_OR(ERR(e), 5)` = `ERR(e)` —— §8.2 让本成员的体不执行并原样返还。这是**对的**:拿默认值去吞一个错误才是错的。

⚠ **判别器只能写 `==(TYPE(v), "NULL")`** —— 语言**没有 `IS_NULL`** 这个构造(全局只有 `IS_OK` / `IS_ERR` / `BOOL` / `TYPE`)。用 `IF(v, …)` 的真值化会把 `""` / `[]` / `0` 一起吞掉,那是**错的**。

⚠ **归层理由(实测,不是偏好)**:`RESULT_ALL` / `RESULT_ANY` 都要把 n 个载荷收集成新数组,而 wlwl 数组不可变,R1 的累加每次复制整个数组。实测该累加链 n 每翻一倍耗时约 **×3**(n=250→4000:27→765 ms),标准库不该带这条曲线。`NULL_OR` 只二选一、不建数组,故留 R1。与 `std.collection` 归层那十个成员同款理由、同款形态(kernel 注入 + 门面改名导出,`EXPORT` 里是用户可见的名字)。

### 22.6 示例

```wlwl
IMPORT("wlwl:std.result", ["RESULT_MAP", "AND_THEN", "FLATTEN", "NULL_OR", "RESULT_ALL", "RESULT_ANY"]);
PRINT(RESULT_MAP(OK(2), FUN((x), *(x, 10))));   // OK(20)
PRINT(AND_THEN(OK(2), FUN((x), OK(+(x, 1)))));  // OK(3)
PRINT(FLATTEN(OK(OK(5))));                      // OK(5)
PRINT(NULL_OR(NULL, 5));                        // 5
PRINT(NULL_OR(0, 5));                           // 0
PRINT(RESULT_ALL([OK(1), OK(2)]));              // OK([1, 2])
PRINT(RESULT_ANY([ERR("e"), OK(2)]));           // OK(2)
```

### 22.7 设计依据

本命名空间随 v0.11.3 M7(ADDENDUM-11)落地。`RESULT_MAP` / `AND_THEN` / `FLATTEN` / `NULL_OR` 是 R1 门面;`RESULT_ALL` / `RESULT_ANY` 因累加成本曲线归 R2。`UNWRAP` 不复刻的裁决见 `ADR-0029` §4b。

## 附录 A 成员注册镜像

本章**规范性**。

**表体由生成器产出,手改无效。** 单源真相是实现:R2 成员取 `wlwl-std` 的 `ModuleSpec` 绑定表;R1 成员取嵌入源码的 `EXPORT` 声明(含 `std.collection` 与 `std.result`)。漂移时跑 `cargo run --bin gen-appendix-a` 重新拼接,`stdlib_appendix_a_sync` 锁测试会拒绝。本册各章的成员表与本表**双向对拍**。

<!-- appendix-a:begin -->
| 命名空间 | 成员 | 层 | 引入 |
|---|---|---|---|
| `std.io` | `PRINT` `INPUT` `PRINT_ERR` | R2 | v0.10 及以前 |
| `std.fs` | `READ_FILE` `WRITE_FILE` `EXISTS` `PATH_JOIN` `PATH_DIR` `PATH_BASE` `PATH_EXT` `PATH_NORMALIZE` `LIST_DIR` `WALK_DIR` `MKDIR` `MKDIRS` `REMOVE` `COPY` `MOVE` `FILE_SIZE` `READ_BYTES` `WRITE_BYTES` | R2 | v0.10 及以前 |
| `std.json` | `PARSE` `STRINGIFY` `JSON_TOKENIZE` `JSON_LOCATE` `JSON_VALIDATE` `JSON_GET` `JSON_SET` `JSON_DELETE` `JSON_MERGE` `JSON_CHUNK_ITEMS` `JSON_CHECK` | R2 | v0.10 及以前 |
| `std.format` | `FORMAT` | R2 | v0.10 及以前 |
| `std.encode` | `BASE64_ENCODE` `BASE64_DECODE` `HEX_ENCODE` `HEX_DECODE` `URL_ENCODE` `URL_DECODE` `SHA256` `HMAC_SHA256` `PBKDF2_ITER` `ARGON2ID` `RANDOM_BYTES` `RANDOM_HEX` `TIMING_SAFE_EQ` | R2 | v0.11.2 / SHA256、HMAC_SHA256 于 v0.11.3 |
| `std.time` | `NOW` `MONOTONIC` `SLEEP` `TEST_BUBBLE` `ADVANCE` `TIMEOUT` | R2 | v0.11.3 |
| `std.regex` | `RE` `RE_TEST` `RE_SEARCH` `RE_FIND_ALL` `RE_REPLACE` `RE_SPLIT` `RE_GROUP_COUNT` | R2 | v0.11.3 |
| `std.rand` | `SEED` `UNIFORM` `INT_RANGE` `CODEPOINT` | R2 | v0.11.3(过渡形态,见 ADR-0027 A7) |
| `std.env` | `ENV_GET` `ENV_SET` `ENV_KEYS` `ARGS` | R2 | v0.11.3 |
| `std.process` | `PROCESS_RUN` `PROCESS_ID` | R2 | v0.11.3 |
| `std.text` | `TO_UPPER` `TO_LOWER` `NFC` `NFD` `NFC_QC` `GRAPHEME_COUNT` `WIDTH` | R2 | v0.11.2 |
| `std.sanitize` | `HTML_ESCAPE` `HTML_UNESCAPE` `HTML_SANITIZE` `HTML_SANITIZE_SCRIPT` `CSS_ESCAPE_IDENT` `CSS_ESCAPE_STRING` `HTML_SANITIZE_CSS` `HTML_SANITIZE_URL` | R2 | v0.11.3 |
| `std.collection` | `MAP` `FILTER` `REDUCE` `SORT` `SORT_BY` `ZIP` `RANGE` `ANY` `ALL` `FIND` `ENUMERATE` `TAKE` `DROP` `FLAT` `UNIQ` `GROUP_BY` `JOIN` `CHUNK` `WINDOW` `DEDUP_BY` `MIN_BY` `MAX_BY` `SUM` `PRODUCT` `FOLD_RIGHT` `POSITION` `KEY_BY` | 混合(R1 门面 + R2 `RANGE`/`MAP`/`FILTER`/`CHUNK`/`WINDOW`/`ENUMERATE`/`ZIP`/`UNIQ`/`FLAT`/`JOIN`/`GROUP_BY`/`DEDUP_BY`/`KEY_BY`) | v0.10 及以前(成员)/ v0.11(R1 重写,M5 起 RANGE 沉 R2)/ v0.11.3(M5 L0-A-2 加 MAP·FILTER·CHUNK·WINDOW,L0-A-3a 加 ENUMERATE·ZIP·UNIQ·FLAT·JOIN,L0-A-3b 加 GROUP_BY·DEDUP_BY·KEY_BY 沉 R2) |
| `std.str` | `JOIN` `SPLIT_LINES` `CHAR_AT` `COUNT` `QUOTE` `INDEX_OF` `CONTAINS_SUB` | R1 | v0.11 / INDEX_OF、CONTAINS_SUB 于 v0.11.2 |
| `std.math` | `ABS` `MIN` `MAX` `FLOOR` `CEIL` `ROUND` `SQRT` `POW` `CLAMP` `PI` `E` `LN` `LOG2` `LOG10` `EXP` `TRUNC` `SIN` `COS` `TAN` `ASIN` `ACOS` `ATAN` `ATAN2` `SINH` `COSH` `TANH` `SIGN` `DIV_CEIL` `GCD` `LCM` `IS_SQRT` `POW_MOD` `DECIMAL` `DECIMAL_ADD` `DECIMAL_SUB` `DECIMAL_MUL` `DECIMAL_DIV` `DECIMAL_CMP` `DECIMAL_TO_STRING` `DECIMAL_ROUND` `FRACTION` `FRACTION_ADD` `FRACTION_SUB` `FRACTION_MUL` `FRACTION_DIV` `FRACTION_CMP` `FRACTION_TO_STRING` `FRACTION_TO_DECIMAL` `APPROX_EQ` `IS_NAN` `IS_INF` `SIGNIFICANT_DIGITS` `MEDIAN` `MODE` `VARIANCE_S` `VARIANCE_P` `STDDEV_S` `STDDEV_P` `PERCENTILE` `CORRELATION` `LINEAR_FIT` | 混合 | v0.11 |
| `std.test` | `TEST` `ASSERT` `ASSERT_EQ` `ASSERT_NEQ` `EXPECT_ERR` `RUN_TESTS` | 混合 | v0.10 及以前(成员)/ v0.11(混合化) |
| `ai` | `ASK` `EMBED` `COMPLETE` `ASK_STREAM` `ASK_ALL` | R2 | v0.10 及以前(以 wlwl:std.ai 名义)/ v0.11.3 M8 降级为官方包(ADR-0034) |
| `agent` | `TASK` `TOOL` `CALL_TOOL` `MODEL` `CONTEXT` | R2 | v0.10 及以前(以 wlwl:std.agent 名义)/ v0.11.3 M8 降级为官方包(ADR-0034) |
| `std.compress` | `ZSTD_COMPRESS` `ZSTD_DECOMPRESS` `ZLIB_COMPRESS` `ZLIB_DECOMPRESS` `GZIP_COMPRESS` `GZIP_DECOMPRESS` `DEFLATE_RAW_COMPRESS` `DEFLATE_RAW_DECOMPRESS` | R2 | v0.11.3 |
| `std.net` | `URL_PARSE` `URL_JOIN` `HTTP_GET` `HTTP_POST` `HTTP_REQUEST` | R2 | v0.11.3 |
| `std.result` | `RESULT_MAP` `AND_THEN` `FLATTEN` `NULL_OR` `RESULT_ALL` `RESULT_ANY` | 混合(R1 `RESULT_MAP`/`AND_THEN`/`FLATTEN`/`NULL_OR` + R2 `RESULT_ALL`/`RESULT_ANY`) | v0.11.3 |
<!-- appendix-a:end -->



