# WLWL 标准库规范 v0.11

> 本规范**独立于**语言规范([`../spec/wlwl-spec-v0.11.md`](../spec/wlwl-spec-v0.11.md)),
> 按文件名自带版本;v0.11 起与语言规范同号发布,1.0 后各自独立演进(届时头部登记
> 兼容配对)。依赖声明:本规范以语言规范 v0.11 的**值域**(§2.1)、**错误模型**(§8)、
> **模块机制**(§9)、**诊断码表**(§11)与**并发语义**(§17)为前提,不重复定义;
> 两册冲突时以语言规范为准。规范性用语(必须/不得/应当/可以)沿用语言规范 §0.3。
> 设计依据:ADR-0021(分层模型)、ADR-0022(值直通边界)、ADR-0023(稳定性政策)。

## 0 总则

### 0.1 调用面

标准库的调用面只有两种形式,与实现层无关:

- **全局内建**(R0 内建层):无须导入;契约见语言规范 §10.2–10.5、§10.7、
  §10.9、§10.12 与其附录 G。本规范不重复、不镜像全局内建。
- **命名空间成员**:`IMPORT("wlwl:std.X", [names])` 导入后调用;成员契约
  见本规范各章。

### 0.2 实现分层(ADR-0021)

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
| `std.collection` | 混合(R1 门面 + R2 `RANGE`) | §5 |
| `std.str` | R1 | §6 |
| `std.math` | 混合(R1 门面 + R2 浮点内核) | §7 |
| `std.test` | 混合(R1 门面 + R2 原生内核) | §8 |
| `std.ai` | R2 | §9 |
| `std.agent` | R2 | §10 |

**层间规则**:R1 可以调用 R0 与 R2;R2 默认禁止调用 R1(确需时走白名单并
登记偏差)。混合模块以门面(R1 侧)为对外契约层。层归属变更(下沉/上浮)
**不算**破坏性变更,但必须附基准数据并登记构建计划(ADR-0023)。

**R1→R2 私有注入通道(kernel)**:混合模块的 R2 实现经 `StdSource::kernels`
在 R1 源码求值**之前**注入到子求值器的局部环境;该通道是**私有**的 ——
**kernel 不外泄**:① kernel 名字必须带 `_` 前缀(全大写),② 不进 `EXPORT`
声明(故不进附录 A、不进 `IMPORT` 可见面),③ 不与任何全局内建同名。
门面若要把它变成成员,只能**改名导出**(`LET(RANGE, _RANGE);` 这类),不
改变 kernel 本身。机制说明见 ADR-0021 §层间规则;两条守卫分别是
「注入面不外泄」与反向的「每个 kernel 都得有人注入」(防死注入)。

### 0.3 治理四件套

每个命名空间成员,无论实现层,必须齐备:

1. **本规范条目**:成员表 + 语义 + 失败行为;
2. **附录 A 注册镜像**:由生成器从实现清单(`ModuleSpec` 绑定表 + R1 源码
   导出)提取,锁测试双向守护;
3. **与本规范成员表的外部对照**:测试从本规范的表格**解析**出成员面,与
   实现清单对拍(集合相等)。这一条是第 2 条的缺口补丁 —— 第 2 条两侧都读
   实现,规范被改坏而实现没动时它照样全绿;
4. **ERR 消费者注册**:R2 成员按 ADR-0009 在 init 时自注册;R1 成员的
   `ERR` 处理由语言语义(8.2/8.3)自然承担,不经消费者注册表。

> **`.wll.sig` 不在本清单内。** 语言规范 §9.6 写的是旁路签名文件「**可以**有」,
> 且三个方向都只在 `gradual_typing` 开启时被检查;而 std R1 模块的加载路径
> 是 `parse` + `eval_module`,**永不做类型检查**,所以给它们配 `.wll.sig` 只会
> 多出第三份**永不被校验**的副本 —— 那是要漂移的,不是治理。因此 stdlib 的
> 成员可审计性由上面第 1–3 条承担。偏差 D11-013。

> 状态:镜像生成器与锁测试已随 M1 落地(`gen-appendix-a` bin +
> `stdlib_appendix_a_sync` 锁测试)。附录 A 表体由生成器产出,手改无效。
> 第 3 条的外部对照已随 M3 落地(`collection_contract.rs` /
> `str_math_contract.rs` / `test_contract.rs`)。

### 0.4 稳定性政策(ADR-0023)

- **v0.x**:可见面变更(新增/改签名/改语义/移除/弃用)必须同批次完成
  四件套同步并留 CHANGELOG 条目;破坏性变更允许但必须明标 breaking;
  弃用走两步(警告码 + 至少保留一个次版本);**新增命名空间不得重导出
  全局内建同名成员**(既有 `std.io` / `std.format` 冻结为例外,不扩充)。
- **1.0 起**:冻结 —— 不移除成员、不改签名、不改可观察语义;新增成员、
  新增命名空间与 R1↔R2 实现迁移不受限;例外仅安全修复与 bug 修复
  (以规范为准)。

### 0.5 分发

- release 二进制**默认嵌入**全部 R1 源码,运行时不读取外部文件;
- release 压缩包附 `stdlib/` 参考副本(R1 源码,运行时不读取);
- 开发期源码覆盖:显式 flag / debug 环境变量指向源码目录,仅覆盖 R1 加载
  路径;属非稳定实现细节,**不得**改变任何成员的名字、签名与语义(锁测试
  守护);
- **[v0.11.1 / D11-019] 「debug 环境变量」是硬门控**:`WLWL_STD_SRC` 只在
  debug 构建(`debug_assertions`)下被读取,release 构建**无条件忽略**它。
  此前的实现无条件生效,等于发布版二进制可被环境变量重定向去加载任意
  R1 源码目录 —— 与本节第一行「运行时不读取外部文件」直接冲突。
  release 下要用覆盖轨,只能显式敲 `--std-src`(`wlwl run --std-src DIR`),
  它需要人主动给出,且同样受上面的「不得改变成员契约」约束。

## 1 `std.io` — 基础 I/O(R2)

与全局内建同名同语义的三个成员(**冻结例外**,见 0.4;语义以语言规范 §10.2 为准):

| 签名 | 说明 |
|------|------|
| `PRINT(args...) -> NULL` | 同语言规范 §10.2 |
| `PRINT_ERR(args...) -> NULL` | 同上 |
| `INPUT(prompt?) -> STRING` | 同上 |

## 2 `std.fs` — 文件系统(R2)

| 签名 | 说明 |
|------|------|
| `WRITE_FILE(path, text) -> NULL` | 写 UTF-8 文本;父目录必须已存在 |
| `READ_FILE(path) -> STRING` | 读文件;不存在产生 `E0061` |
| `EXISTS(path) -> BOOLEAN` | 存在测试 |

## 3 `std.json` — JSON(R2)

| 签名 | 说明 |
|------|------|
| `STRINGIFY(x) -> STRING` | 序列化为 JSON;字典键按字典序输出;`NULL`/布尔/数字/字符串/数组/字典自然映射;失败产生 `E0071` |
| `PARSE(s)` | 解析 JSON;对象保持文档序的字典;失败产生 `E0070` |

## 4 `std.format` — 格式化(R2)

导出与全局内建 `FORMAT` 同一函数(**冻结例外**,见 0.4)。模板文法、
`{N}`/`{name}` 混用规则与 `E0039` 见语言规范 §10.7 —— 本成员是全局内建的
命名空间视图,语义以语言规范为准。

### 4.1 `RESULT` 的渲染:两条路径**刻意不同**(v0.11.2 / D12-003 已裁决)

「同一函数」这句话在 `RESULT` 值上**不成立**,且这不是缺陷:

| 路径 | `FORMAT("{0}", OK(1))` | 机制 |
|---|---|---|
| 全局内建 `FORMAT` | `OK(1)` | 走语言侧 display 路径,`RESULT` 按其字面构造形态渲染 |
| 成员 `wlwl:std.format` 的 `FORMAT` | `1` | 经 std 边界,`OK(v)` 在边界解包为 `v` |

两条路径本来就分属 eval 侧与 std 边界两处实现,统一它们是一次**行为变更**而非
文档债。v0.11.2 裁决:**保留差异,只把口径写清楚**。

选型口径:需要「把这个 `RESULT` 原样打出来给人看」用全局内建;需要「把 `RESULT`
里的值填进模板」用成员路径。`ERR` 值的渲染不可观察 —— 调用边界即短路传播,
到不了转换层(两种写法都一样),保留拒绝臂仅作兜底。

**锁定**:`impl/crates/wlwl-eval/tests/` 有一条契约用例分别钉住这两个输出,
目的是防止后续实现「好心统一」,把这个有意保留的差异改掉。

## 5 `std.collection` — 集合套件(混合:16 成员 R1 + `RANGE` 归 R2)

除注明外成员都是**非破坏性**的(语言规范 §10.1);成员回调抛出的 `ERR` 使
整个调用按 8.2 传播;`arr` 实参须为 `ARRAY`、回调 `f` 须为函数值(违者
`E0030`)。

**落地状态**:17 个成员全部落地。**16 个是纯 wlwl(R1)**;**`RANGE` 的实现
归 R2**,门面只改名导出 —— 层归属变更不算破坏性变更(ADR-0021 §0.2),故
导出面 / 签名 / 语义未变,75 条冻结行为用例(诊断码与消息逐字)全绿。

> **「诊断一字未变」有一条例外,在此明写**(v0.11.1 / D11-003 补记):`RANGE`
> 的整数计数溢出行为随层归属**回摆**过一次。R1 版走 `+(i, step)`,按语言
> 规范 §2.2 抛 `E0035`;R2 版用 `i64::checked_add`,`None` 即停(饱和)。
> 即 D11-003 当时判为「实现层私货」的饱和行为,在 M5 把 `RANGE` 沉回 R2 时
> 一并回来了。可达性:需循环约 9.2×10^18 次,实际不可达(在此之前已耗尽
> 内存/时间),故不改行为;此处只把「一字未变」的表述改回与代码一致。
> 另:语言规范 §9.5 是「程序入口」,**没有**任何饱和约定 —— 旧注释里那句
> 「按 §9.5 饱和」是编号变更前留下的悬空引用。

**为什么 `RANGE` 单独沉回 R2**(M5 基准实测,release 档、单线程):R1 版
`RANGE(0, 10 000)` 3 623 ms、`RANGE(0, 40 000)` 86 036 ms,每元素成本
**超线性**;对照 R2 时代 `RANGE(1, 1 000 001, 1)` 加上 100 万次 `FOR`
整体只要 536–551 ms。根因不是解释器慢,而是语言层**无法线性建数组**:
wlwl 数组不可变,`PUSH` 每次复制整个数组。于是语言规范 §6.6 的
「100 万次简单循环 < 30 s」符合性负载跑不完,且时间全部消耗在**循环开始
之前**。数据见 `impl/crates/wlwl-eval/benches/baseline.txt` 的 M5 段,
偏差 D11-012。

**局限(明写,不掩盖)**:同一成本剖面适用于**所有**建数组的成员 —— `MAP` /
`FILTER` / `FLAT` / `UNIQ` / `ENUMERATE` / `GROUP_BY` / `JOIN` 都走
`PUSH`,故它们在 1000+ 元素上仍是平方级。只沉 `RANGE` 是**治标**;根因
修复(解释器侧写时复制 / 结构共享)超出本版范围,记为演进项。

| 签名 | 说明 |
|------|------|
| `MAP(arr, f)` | `[f(v), ...]` |
| `FILTER(arr, f)` | 保留 `f(v)` 为真的元素 |
| `REDUCE(arr, f, init)` | 左折叠;空数组返回 `init` |
| `SORT(arr)` / `SORT(arr, cmp)` / `SORT_BY(arr, key)` | 升序排序;`cmp(a, b)` 返回 `BOOLEAN` 表示 `a` 排在 `b` 前(降序传 `FUN((a, b), >(a, b))`);`SORT_BY` 以 `key(v)` 为排序键。`cmp` 返非 `BOOLEAN` → `E0030`;`cmp` 抛出的 `ERR` 按 8.2 传播整个调用。混合类型**不可比时不报错**:各组内稳定,组间次序由择序策略决定(见下方注) |
| `RANGE(n)` / `RANGE(start, end)` / `RANGE(start, end, step)` | 整数区间 `[start, end)`,步长 `step`;`step = 0` 产生 `E0038`。**实现归 R2**(见本章落地状态)|
| `ZIP(a, b) -> ARRAY` | 配对至较短者:`[[a0, b0], ...]` |
| `ENUMERATE(arr) -> ARRAY` | `[[0, v0], [1, v1], ...]` |
| `TAKE(arr, n)` / `DROP(arr, n)` | 前 n / 去前 n |
| `FLAT(arr) -> ARRAY` | 展平一层 |
| `UNIQ(arr) -> ARRAY` | 按 `==` 去重,保留首现 |
| `GROUP_BY(arr, key) -> DICT` | 按 `key(v)` 分组,组为按首现序的字典 |
| `ANY(arr, f?)` / `ALL(arr, f?)` | 任一/全部为真(缺省 `f` 时按真值) |
| `FIND(arr, f)` | 首个满足者,无则 `NULL` |
| `JOIN(arr, sep) -> STRING` | 字符串化后以 `sep` 连接 |

> **混合类型排序的准确口径**(v0.11.1 更正,原写「不可比时保留原序」按字面
> 读是错的)。不可比的元素**不报错**;排序分「可比组」进行,**各组内稳定**,
> 而**组与组之间的相对次序由择序策略决定,不保证与原序一致**。
> 实测:`SORT([1, "a", 2])` → `[1, a, 2]`(恰好与原序同形,故容易被当成
> 「保留原序」的例子),但 `SORT([2, "a", 1])` → `[1, 2, a]`、
> `SORT([3, "a", 1, "b", 2])` → `[1, 2, 3, a, b]` —— `"a"` 相对 `1` 的
> 位置变了。行为确定且无害(实现是选择排序 + 「不可比即不小于」),但规范
> 不该承诺它没承诺的东西。

## 6 `std.str` — 字符串扩展(R1,v0.11 新增)

纯 wlwl 实现。只补全局内建(语言规范 §10.5)没有的能力;**不重导出**任何
全局内建。索引口径与 `SUB` 一致(码点;起点同 `SUB` 的 `start` —— 含
**负索引自尾部计数**,所以 `CHAR_AT("abc", -1)` 是 `"c"`,不是越界)。
落地状态:本章成员已随 M3-2 全部落地;`INDEX_OF` / `CONTAINS_SUB` 随
v0.11.2 M2 追加(§6 表格即当前成员面;附录 A 镜像由 `gen-appendix-a` 从实现
生成并由 `stdlib_appendix_a_sync` 锁)。
`SPLIT_LINES` 的四条边界逐条见下表;`COUNT` 的非重叠口径用一次
`SPLIT(s, sub)` 实现 —— 切成 n+1 段则非重叠出现次数正是 n。

| 签名 | 说明 | 失败 |
|------|------|------|
| `JOIN(arr, sep) -> STRING` | 元素经 `STR` 渲染后以 `sep` 连接;空数组得 `""` | — |
| `SPLIT_LINES(s) -> ARRAY` | 按 `\n` 切分并去除行尾 `\r`;尾随换行不产生空尾元素;空串得 `[]`;无换行得 `[s]` | — |
| `CHAR_AT(s, i) -> STRING` | 取第 `i` 个码点,等价于 `SUB(s, i, 1)` | 越界同 `SUB` |
| `COUNT(s, sub) -> INTEGER` | `sub` 的非重叠出现次数 | `sub` 为空串:`E0030` |
| `QUOTE(s) -> STRING` | 双引号包裹;内部 `"` 与 `\` 转义为 `\"`、`\\`;`\n`、`\t`、`\r` 分别转义;其余字符原样 | — |
| `INDEX_OF(s, sub, from?) -> INTEGER` | `sub` 首次出现的**码点**下标(0 起),无则 `-1`;`from` 把起点推到后面(负值钳到 0),默认 0 | `sub` 为空串:`E0030` |
| `CONTAINS_SUB(s, sub) -> BOOLEAN` | `sub` 是否出现 | `sub` 为空串:`E0030` |

> **`COUNT` 早就在,别再加一个 `COUNT_SUB`**(v0.11.2 M2 实测更正)。起草
> v0.11.2 计划时把「标准库无字符串检索」记成 GAP-3,依据是探针
> `PRINT(FIND("hello","el"))` 报 `E0020` —— 但 `FIND` 是 `std.collection`
> 的**数组**成员,探的是错的名字。实测 `std.str::COUNT` 早已给出非重叠计数
> (`COUNT("aaa","a")` = 3、`COUNT("aaaa","aa")` = 2),而且**空 `sub` 报
> `E0030`**。真正缺的只有**位置**与**布尔判定**两件事,故本表只加两个成员。
>
> 空 `sub` 的口径因此统一成「报错」:`INDEX_OF` 与 `CONTAINS_SUB` 沿用
> `COUNT` 的既有规则,而不是各自发明一条(比如「空串恒匹配」)。同一模块里
> 三个成员对空串给三种答案,调用方只能靠「读过哪份文档」来判断。
>
> 下标是**码点**不是字节,与 `CHAR_AT` / `SUB` / `CODEPOINTS` 同口径,
> 拿到后可直接 `SUB(s, i, n)` 切出那段。`from` 存在的理由:没有它就只能从 0
> 开始反复扫全文找后续出现,那是 O(n·m);`INDEX_OF` 是本模块 v0.11.2 之前
> 唯一真正缺的能力(全局 `INDEX` 只吃数组)。

## 7 `std.math` — 数学基础(混合,v0.11 新增)

**纯 wlwl 门面 + R2 浮点内核**:`ABS` / `MIN` / `MAX` / `FLOOR` / `CEIL` /
`ROUND` / `CLAMP` 与常量可用 wlwl 算术与比较表达,归门面;`SQRT` / `POW`
依赖浮点指令,纯 wlwl 不可表达,落 R2 内核(经门面导出)。首批仅基础函数;
超越函数(sin/cos/exp/log 族)不进本版(§11)。

约定:算术提升与溢出行为沿语言规范 §2.2;域违例返回
`ERR(["kind": "DomainError", ...])`;实参类型错按语言规范报 `E0030`。
`MIN` / `MAX` / `CLAMP` 的**返回值类型**也按 §2.2 提升:实参中有任一
`FLOAT` 则返回 `FLOAT`(`CLAMP` 看**三个**实参)。
落地状态:本章成员已随 M3-2 全部落地(§7 表格即当前成员面;附录 A 镜像
由 `gen-appendix-a` 从实现生成并由 `stdlib_appendix_a_sync` 锁)。
`FLOOR` / `CEIL` / `ROUND` 归门面但**不是** `INT`:`INT` 按 §2.2 向零截断,
而下/上取整与「半数远离零」都不同于它,故实现走 `INT` + 符号修正,超出
`±2^53` 的 `FLOAT`(本身已是整数)与 `NaN` / `±inf` 原样返回。

| 签名 | 说明 | 失败 |
|------|------|------|
| `ABS(x)` | 绝对值,返回与实参同类型 | `INTEGER` 下界(`-ABS(INT_MIN)` 形):`E0034` |
| `MIN(a, b)` / `MAX(a, b)` | 二元最值;混合整浮按 §2.2 提升 | — |
| `FLOOR(x)` / `CEIL(x)` | 下/上取整;`INTEGER` 恒等;`FLOAT` 返回 `FLOAT`;`NaN`/`±inf` 原样 | — |
| `ROUND(x)` | 四舍五入(半数远离零);其余同上 | — |
| `SQRT(x) -> FLOAT` | 平方根;`-0.0` 得 `-0.0`(见下方注) | `x < 0`:`ERR(["kind": "DomainError"])` |
| `POW(a, b) -> FLOAT` | 幂;两参提升为 `FLOAT` | `0` 的负次幂、负底的非整数指数:`ERR(["kind": "DomainError"])` |
| `CLAMP(x, lo, hi)` | 夹取到 `[lo, hi]`;类型提升同 `MIN`/`MAX` | `lo > hi`:`ERR(["kind": "DomainError"])` |
| `PI` / `E` | `FLOAT` 常量 | — |

> **`-0.0` 这一格怎么写出来**(v0.11.1 补记,实测)。`SQRT` 的实现**没有**
> 丢符号:喂一个真负零,返回的就是负零(`*(0.0, -(0, 1.0))` → `SQRT` →
> `-0.0`,且 `/(1.0, 负零)` 报 `E1003` 证明值确实是零)。但**源码里写
> 不出负零字面量**:语言规范 §1.6 的 `-x` 语法糖在解析期被改写成
> `-(0, x)`,而 IEEE 754 下 `0 - 0.0 = +0.0` ⇒ `SQRT(-0.0)` 里的 `-0.0`
> 其实是 `+0.0`,返回 `0.0` 是正确的。要拿到真负零只能绕开语法糖
> (`*(0.0, -(0, 1.0))`)。此格按字面读不可达,故在此写明口径;
> 解析期保留符号属于语言层议题(见演进方向),不在本册范围。

## 8 `std.test` — 测试(混合)

R1 门面 + R2 原生内核(注册与计时)。

| 签名 | 说明 |
|------|------|
| `TEST(name, body) -> NULL` | 注册零参测试体 |
| `ASSERT(cond, msg?)` | `cond` 按语言规范 §2.3 的**八个**假值判(`FALSE` / `NULL` / `0` / `0.0` / `""` / 空数组 / 空字典 / `NaN`);为真 → `OK(TRUE)`,否则 `ERR(E0046)`。**自 v0.11 变更**:此前只把 `BOOLEAN(false)` 与 `NULL` 当假,`ASSERT(0)` / `ASSERT("")` / `ASSERT([])` 会通过(按 ADR-0023 §v0.x 第 2 条申报的 breaking) |
| `ASSERT_EQ(a, b)` / `ASSERT_NEQ(a, b)` | 相等/不等断言,失败为 `E0047`/`E0048` |
| `EXPECT_ERR(x)` | `x` 为 `ERR` → `OK(载荷)`;否则 `ERR(E0049)` |
| `RUN_TESTS() -> ARRAY` | 调用所有已注册体,返回记录数组;每条记录是字典,至少含 `name`(STRING)、`passed`(BOOLEAN)、`duration_ms`(INTEGER),并按结果附 `return_value` 或 `error` |

**约定**:测试体应当以 `ASSERT` 族断言收尾并以 `TRUE` 结束;体求值期间
逃逸的 `ERR` 使该项 `passed = FALSE`。

**测试体挂起时(v0.11.1 / D11-019 新增)**:测试体若在求值中挂起(体是
`YIELD()` 所在的任务体,且挂起点在 `RUN_TESTS` 调用之内),`RUN_TESTS`
**不产出记录**并把挂起信号原样交回调度器 —— 挂起不是「测试通过」也不是
「测试失败」,因此**不得**被折算成其中任一个。判据是 `ERR` 与挂起的区别:
前者是**值**,`RUN_TESTS` 可以捕获并记账(§8 约定);后者是**控制流信号**,
按 `wlwl_value::StdFn` 契约(std 层不得消费)必须穿透。
> 修订记录:此前实现把挂起信号丢弃,挂起点传回的 `NULL` 被按「非 `ERR` 即
> 通过」记账,于是 `TEST("t", FUN(() → YIELD(); ASSERT(FALSE)))` 判为通过
> —— 体里的断言一次都没跑。

## 9 `std.ai` — AI 推理(R2)

下列最小契约是规范性的;各函数依赖外部服务,传输与供应商细节属实现文档,
不在规范内。

| 签名 | 说明 | 失败 |
|------|------|------|
| `ASK(model, prompt, opts?) -> STRING` | 单次推理 | `E0080`–`E0083` |
| `ASK_STREAM(model, prompt, callback, opts?) -> ARRAY` | 流式推理(实现侧当前整收集为单块) | `E0080`–`E0083` |
| `ASK_ALL(models, prompt, opts?) -> ARRAY` | 多模型批量 | `E0080`–`E0083` |
| `EMBED(model, text) -> ARRAY` | 嵌入向量(四维,实现侧固定) | `E0080` |
| `COMPLETE(model, context) -> STRING` | 代码补全 | `E0080` |

错误码语义:`E0080` 请求失败;`E0081` 协议错误;`E0082` 工具错误;`E0083`
上下文错误。模型名缺少 `provider/` 前缀产生 `W0052`(语言规范 §11.3)。

## 10 `std.agent` — 代理(R2)

| 签名 | 说明 | 失败 |
|------|------|------|
| `MODEL(name) -> DICT` | 选择模型,返回模型描述字典 | `E0080` |
| `TASK(name, prompt, ...) -> TASK` | 构造异步代理任务,返回任务句柄 | `E0090`–`E0094` |
| `TOOL(name, schema, fn) -> NULL` | 注册工具 | `E0081`(重复注册或协议不符) |
| `CALL_TOOL(name, args) -> v` | 同步调用已注册工具 | `E0082` |
| `CONTEXT(set, get, ...) -> v` | 上下文存取 | `E0083` |

网络错误码语义:`E0090` 网络不可达、`E0091` DNS 失败、`E0092` TLS 错误、
`E0093` HTTP 4xx、`E0094` HTTP 5xx。

返回的 `TASK` 句柄按语言规范 §17.1 经 `AWAIT` 取值;已取消任务的 `AWAIT`
按 §17.3 返回 `ERR(kind="Cancelled", reason: ...)`。

**示例**(推荐全限定以避免与类型名 `TASK` 混淆):

```wlwl
IMPORT("wlwl:std.agent", ["TASK", "MODEL"]);
LET(handle, wlwl:std.agent.TASK("summarize", "long text..."));
```

用户作用域内裸名 `TASK` 也合法(若 `IMPORT` 引入了同名函子);全限定写法
仅为阅读清晰度,与同名类型名 `TASK` 不构成运行时冲突(§2.1)。

## 11 `std.encode` — 编码(R2,v0.11.2 新增)

base64(RFC 4648 §4)、hex、percent-encoding(RFC 3986 §2)。三者都作用在
**字节**上,而本语言的 `STRING` 是 UTF-8:编码取实参的 UTF-8 字节,解码后
**必须**是合法 UTF-8 才交出去。

### 11.1 失败口径

- 元数错 `E0022`、实参类型错 `E0030`(与 §5 / §7 同款:程序员错误走原生诊断)。
- **解码失败是域违例**,返回 `ERR(["kind": "DecodeError", "op": ..., "reason": ...])`
  **值**。它不叫 `DomainError` —— 把「`SQRT(-1)`」和「base64 串少了一个字符」
  归成同一个 kind 会让调用方没法区分。
- **编码永不失败**,下表的失败列对三个 `*_ENCODE` 是 `—`。

### 11.2 成员

| 签名 | 说明 | 失败 |
|------|------|------|
| `BASE64_ENCODE(s, url_safe?)` | RFC 4648 标准字母表 + `=` 补齐;`url_safe=TRUE` 换用 URL 字母表(`+/` → `-_`) | — |
| `BASE64_DECODE(s)` | 忽略 `CR` / `LF`(base64 常被折行嵌入);**两张字母表都收** | 非法字符、补位错位、末组残留非零位、长度不合法 → `ERR([... "DecodeError" ...])`;解出非 UTF-8 同 |
| `HEX_ENCODE(s)` | 小写、无分隔符 | — |
| `HEX_DECODE(s)` | 大小写都收;不忽略任何字符 | 奇数长度、非 hex 字符、解出非 UTF-8 → `ERR([... "DecodeError" ...])` |
| `URL_ENCODE(s)` | percent-encoding;`unreserved = ALPHA / DIGIT / "-" / "." / "_" / "~"`,其余 `%XX`(大写十六进制) | — |
| `URL_DECODE(s)` | 解 `%XX`;**`+` 不折成空格** | 截断或非法的 `%` 转义、解出非 UTF-8 → `ERR([... "DecodeError" ...])` |

### 11.3 三条需要明写的设计取舍

1. **解码自动识别 base64 字母表。** 标准表的 `+/` 与 URL 表的 `-_` 除这两个
   字符外完全相同,所以「两张都收」是确定性的。Go 的 `StdEncoding.Decode` 会
   拒 `-`、`URLEncoding.Decode` 会拒 `+`;本成员比它们宽。混用 `+/` 与 `-_`
   的串也能解,这是有意的放宽。
2. **`+` 按 RFC 3986 读,不折成空格。** `+` → 空格是
   `application/x-www-form-urlencoded` 的规则。Go 的 `url.QueryUnescape` 做这个
   替换,`url.PathUnescape` 不做;本成员对标后者。表单编码请自己处理。
3. **解码不做有损替换。** wlwl 没有字节类型,解出的非法字节没有能诚实表达的
   返回形态。与其 `to_string_lossy` 换成 `U+FFFD` 骗调用方,不如报
   `DecodeError`。这同时意味着**不能通过 `STRING` 携带任意二进制** ——
   要传二进制得先 base64/hex,再在对面解回 `STRING`。

### 11.4 RFC 测试向量(规范正文的一部分,不是实现细节)

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

最后一行的「URL-safe 与标准不同」用 0xFB 0xFF 演示(标准 `+/8=` / URL `-_8=`)——
但那**不是合法的 UTF-8**,而 wlwl 的 `STRING` 是 UTF-8,所以这个向量只存在于
`wlwl-std` 的 Rust 侧单测,不在语言层可达。语言层能观察到的 URL-safe 分歧用
`U+083F`(UTF-8 字节 `E0 A0 BF`):标准 `4KC/`,URL-safe `4KC_`。

> 取码点的方法(留给下一个要加向量的人):**不要手算**。用参考实现枚举 ——
> `for cp in 0x80..0xFFFF` 取每个码点的 UTF-8 字节,喂
> `[Convert]::ToBase64String`,挑第一个结果里含 `+/` 的。2 字节 UTF-8
> (U+0080–U+07FF)**永远**产生不出 `+/`(首 6 位恒为 `110xxx`,落在索引
> 48–55),必须从 3 字节起找。起草本节时手算过一次,算错了,以参考实现为准。

## 12 `std.text` — Unicode 大小写(R2,v0.11.2 新增)

**全局内建 `UPPER` / `LOWER` 只做 ASCII 映射**(语言规范 §10.5),实测
`UPPER("straße")` = `STRAßE`、`UPPER("héllo")` = `HéLLO` —— 非 ASCII 字符
原样穿过。本命名空间提供**完整 Unicode 简单大小写映射**。

零 Unicode 数据文件:Rust 的 `str::to_uppercase` / `to_lowercase` 直接走
`char` 实现内置的 `Uppercase` / `Lowercase` 查表(含 `ß` → `SS`、希腊
final sigma、土耳其无点 i / 点上 i 这类特殊映射)。ADR-0022 的零第三方依赖
画像因此不变。

| 签名 | 说明 | 失败 |
|------|------|------|
| `TO_UPPER(s) -> STRING` | 完整 Unicode 大写映射;**长度可能变**(`ß` → `SS`) | — |
| `TO_LOWER(s) -> STRING` | 完整 Unicode 小写映射;希腊 `Σ` 词尾得 `ς`、词中得 `σ` | — |

**与全局 `UPPER` / `LOWER` 的关系**:两个并存,本批**不替换**全局内建。
替换是 breaking(既有程序里非 ASCII 字符的大小写行为会变),不在本批范围。
调用方按需要显式选:`UPPER` 快且只管 ASCII,`TO_UPPER` 全 Unicode。

### 12.1 为什么只有两个成员(其余推迟的理由)

原计划还有 `NFC` / `NFD` / `GRAPHEME_COUNT` / `WIDTH`,**全部推迟**。它们
**共用同一捆 UCD 数据**:

| 成员 | 需要的数据 |
|---|---|
| `NFC` / `NFD` | `UnicodeData`(canonical decomposition)+ `CompositionExclusions` |
| `GRAPHEME_COUNT` | `GraphemeBreakProperty` + `Extended_Pictographic` |
| `WIDTH` | `EastAsianWidth` |

分批做意味着生成器写三遍、审三遍、跟 Unicode 版本对齐三遍。合成一块做是
**一次**决定。§13 的演进方向里已把它们合并成一条。

**明确不做半张表**:只覆盖 Latin-1 的 NFC 比没有 NFC 更糟 —— 它会让
`==(NFC(a), NFC(b))` 在部分输入上返回 TRUE,调用方据此建索引,然后在真正的
CJK 或组合字符上炸。

## 13 演进方向(非规范性)

- `std.ai` / `std.agent` 降级为官方包,移出 `wlwl:std.*`(业界先例:Rust /
  Julia 的 std 小核心原则);
- 新 R2 候选命名空间:`std.net`、`std.process`、`std.env`、`std.time`;
- `std.math` 超越函数族(sin/cos/tan/exp/log);
- **UCD 数据依赖的成员合成一块做**(v0.11.2 M2 裁决):`std.text` 的
  `NFC` / `NFD` / `GRAPHEME_COUNT` / `WIDTH` 共用同一捆
  `UnicodeData` + `CompositionExclusions` + `GraphemeBreakProperty` +
  `Extended_Pictographic` + `EastAsianWidth`,分批做要写三遍生成器。
  随附:决定做的时候必须同时定「跟哪个 Unicode 版本」与「半张表怎么办」;
- R1 预编译快照(替代启动解析);
- 全局内建弃用别名(`DEL` / `POP` / `OR_DIE`)到期移除。

## 附录 A 成员注册镜像(规范性)

> 状态:表体由生成器产出 —— 单源真相是实现(R2 取 `wlwl-std` 的
> `ModuleSpec` 绑定表;R1 取嵌入源码的 `EXPORT` 声明,**含 `std.collection`
> 与 `std.test`** —— v0.11 起这两者的成员面也由 `EXPORT` 声明决定,
> eval 侧的 `BUILTINS` 名录随 R2 实现删除后已不存在)。
> 手改无效,`stdlib_appendix_a_sync` 锁测试会拒绝;
> 漂移时跑 `cargo run --bin gen-appendix-a` 重新拼接。
> R2 成员的 ERR 消费者按 ADR-0009 在 init 时自注册。

<!-- appendix-a:begin -->
| 命名空间 | 成员 | 层 | 引入 |
|---|---|---|---|
| `std.io` | `PRINT` `INPUT` `PRINT_ERR` | R2 | v0.10 及以前 |
| `std.fs` | `READ_FILE` `WRITE_FILE` `EXISTS` | R2 | v0.10 及以前 |
| `std.json` | `PARSE` `STRINGIFY` | R2 | v0.10 及以前 |
| `std.format` | `FORMAT` | R2 | v0.10 及以前 |
| `std.encode` | `BASE64_ENCODE` `BASE64_DECODE` `HEX_ENCODE` `HEX_DECODE` `URL_ENCODE` `URL_DECODE` | R2 | v0.11.2 |
| `std.text` | `TO_UPPER` `TO_LOWER` | R2 | v0.11.2 |
| `std.collection` | `MAP` `FILTER` `REDUCE` `SORT` `SORT_BY` `ZIP` `RANGE` `ANY` `ALL` `FIND` `ENUMERATE` `TAKE` `DROP` `FLAT` `UNIQ` `GROUP_BY` `JOIN` | 混合(R1 门面 + R2 `RANGE`) | v0.10 及以前(成员)/ v0.11(R1 重写,M5 起 RANGE 沉 R2) |
| `std.str` | `JOIN` `SPLIT_LINES` `CHAR_AT` `COUNT` `QUOTE` `INDEX_OF` `CONTAINS_SUB` | R1 | v0.11 / INDEX_OF、CONTAINS_SUB 于 v0.11.2 |
| `std.math` | `ABS` `MIN` `MAX` `FLOOR` `CEIL` `ROUND` `SQRT` `POW` `CLAMP` `PI` `E` | 混合 | v0.11 |
| `std.test` | `TEST` `ASSERT` `ASSERT_EQ` `ASSERT_NEQ` `EXPECT_ERR` `RUN_TESTS` | 混合 | v0.10 及以前(成员)/ v0.11(混合化) |
| `std.ai` | `ASK` `EMBED` `COMPLETE` `ASK_STREAM` `ASK_ALL` | R2 | v0.10 及以前 |
| `std.agent` | `TASK` `TOOL` `CALL_TOOL` `MODEL` `CONTEXT` | R2 | v0.10 及以前 |
<!-- appendix-a:end -->

