# wlwl-skill CHANGELOG

Skill-bundle changes (spec lives at `../docs/wlwl-spec-reference.md`
and is authoritative).

## [Unreleased] — 二期复测带回来的订正（v0.12.0-rc2 的实测打脸本 skill 两条纪律）

⚠ **本段是被第三方二期实测（`REPORT2.md`）推翻了前一段里的两条纪律** —— 不是新增功能，
是**订正**：那两条**教错了**。

### Fixed

- ⚠ **第 7 条纪律（`SCOPE` 与孙任务）原写「作用域不等孙任务 ⇒ 客户端看到 reset」——
  错的。** 实测（rc2）：作用域退出路径**先排干**（把挂着的任务等完）**再**强制取消
  仍非终态的那部分 ⇒ 挂在 socket 上的孙任务**被等完** ⇒ 客户端收到**完整响应**。
  ⇒ 现改为「**先等、后取消**」，并保留「别依赖 `SCOPE` 替你收尾，仍要自己数完成数」。
- ⚠ **第 8 条纪律（`AWAIT` 已结束句柄 ⇒ `E0053`）原写「已结束 ⇒ `E0053`—— 错的。**
  实测：对同一**已结束**句柄再次 `AWAIT` ⇒ **返回它的终值**；`E0053` 只在
  「实参不是句柄」或「句柄已失效（generation 不匹配）」时出现。
  ⇒ §17.1 的「过期句柄」= **失效**，不是**已结束**。
- **新增「Five writing traps」小节**（三条来自二期实测的踩坑记录）：
  ① `||` / `&&` **严格二元**（3 个实参 ⇒ `E0022`）；
  ② **`IF` 的第 3 个实参不能放多语句组**（`E0013`；同样形状放在**第 2** 个实参正常）；
  ③ **`PATH_NORMALIZE` 保留 `\`、`PATH_JOIN` 转 `/`** ⇒ 前缀比对必然失败 ⇒ 全部 `403`；
  ④ **`READ_FILE` 原样保留 UTF-8 BOM** ⇒ `Content-Length` 与浏览器看到的对不上；
  ⑤ ⚠ **`std.fs` 的 IO 失败是 `ERR` 值** ⇒ 不显式丢弃 ⇒ 收尾报 `E0103`
  ⇒ **整条连接无声死掉**（客户端只看到 reset）。
  ② 与 ⑤ 是**静默失败级**的坑：一个解析错、一个让整条连接无声死掉。

### Added

- `reference.md` **§28.6** —— 二期复测带回来的订正表（A4 / A5 / N5 / A3 四条）+
  一条通用教训：「凡是能实测的结论，不要只靠读规范定案」。

### Fixed（v0.12.0 发版审计 F-11…F-22 —— 技能包口径订正批次 R-7）

⚠ 本段是**发版审计**（`../docs/plan/wlwl-v0.12.0-release-audit-20261026.md`）
里**技能包修订批次（R-7）** 的落地 —— 全部是**口径订正**：不改示例源码、
不改语言面、不改任何成员契约。

- **`SKILL.md` 并发「硬规则」第 6 条**原写「**共享可变状态 = `LET MUT`**」⇒
  **与并发册 §17.0.1 正面冲突**（该条明令**不得**跨任务共享可变 cell，正确形态
  是 **actor + 通道**）。现改为「**绝不跨任务共享可变状态**」，并注明违反它
  **不被任何诊断拦截**（调度器只在段边界检查点）。
- **`X-06` / `X-07` 不再是「已知缺口」** —— 一致性册 §5.4 的两行状态均为
  「**已实现**」⇒ 技能包改写为「**已收敛：一条 `ERR` 都不会丢**」。
- **`std.fs` 段落的尾部**原写「三个冻结的 v0.3 成员仍用旧的原生 IO 诊断码」——
  **与同句前半、规范、实测三处冲突** ⇒ 删除；四个成员**全是** `ERR` 值。
- **消费者注册表补 `TAKE_ERR`**（§8.3 的第 **14** 条）⇒ 删掉「§8.3 不新增
  条目」这类过时说法（`SKILL.md` §8.3 / §8.6 与 `reference.md` §8.3 / §8.6 /
  §9.15）。
- **`reference.md` §28.3 第 3 条**按 §28.6 的 A4 订正行改写：`SCOPE` 是
  **先等后取消** ⇒ 客户端拿到**完整响应**，**不是 RST**。
- **成员面 `214` → `231`**、附录 A 由「generated」改称「**frozen snapshot**」
  （规范 §0.7 规则 1：该表**不得**随实现漂移）、差异登记 `X-01…X-15` →
  `X-01…X-25`（`SKILL.md` References / `reference.md` §2 版本表 / `README.md`）。
- **语义变更清单统一为 7 条（D-14…D-20）** —— 原先 `SKILL.md` 说「five」、
  `reference.md` §28.2 只列 5 条（**漏 D-16 / D-17**）、frontmatter 又是第四套。
- **`reference.md` §9.1 导语**原写「All eight members」⇒ 改为 **16 个成员**，
  并说明字节层三员（`UTF8_ENCODE` / `UTF8_DECODE` / `BYTE_LEN`）走
  `ARRAY[INTEGER]`（实测 `UTF8_ENCODE("héllo")` ⇒ `[104,195,169,108,108,111]`，
  `BYTE_LEN` ⇒ 6 而 `LEN` ⇒ 5）。
- **`examples/concurrency_cancel.wll` 的「Expected stdout」**原声明 4 行 ⇒ 改为
  **实际 3 行**（`kind` 只被比较、从未被 `PRINT`）—— **只改注释**。

### Added（技能包修订批次 R-7）

- **`SKILL.md` 新增一节「Task failures — the reporting obligation (§17.9)」** ——
  孤儿任务失败 ⇒ stderr **恰好一行**
  `[wlwl] unhandled task failure (task #N): …`、程序**继续**、**退出码 0**；
  被 `AWAIT` 认领 ⇒ 在**调用点**抛出、**不重复报告**（实质说明原在
  `reference.md` §28.4，本段把它提上来）。
- **`SKILL.md`「Top antipatterns」新增第 28 行**（孤儿任务失败那一行**不是噪声**）。
- **`impl/crates/wlwl-cli/tests/skill_package_guard.rs`** —— 技能包口径守卫：
  `214 members` / `X-01…X-15` / 旧的 fs 原生诊断码 / `Shared state = ` 四条过期
  口径在 `SKILL.md` / `reference.md` / `README.md` 里**零命中**，且 `SKILL.md`
  正文必须**能**搜到「孤儿 / orphan / unhandled task failure」。

## [v0.12.0] — 编译器 v0.12.0:传输层 / 字节层 / 宿主挂起 / `TAKE_ERR` / `INT` 规则

⚠ **规范面不动** —— 规范仍是 **v1.0**（本仓抄件 = 已发布的 `wlwl-spec-v1`
**v1.0.2**,tag `v1.0.2` ⇒ 提交 `294dc07`);本版动的是**编译器**的公共面与语义。

### Added

- **`SKILL.md` 的「Network programming」整节**(v0.12.0 传输层专用):**八条
  非显然的规则**,每条都带**实测故障形态** —— 等待必须在任务里 / 等待必须是所在段
  的第一条语句且一段只放一次等待 / 跨挂起要活的状态绑在被重放段之外 /
  监听 socket 建在任务外 / **`SLEEP` 是阻塞原语** / 别 `SET` 不可变绑定 /
  **`SCOPE` 不等孙任务** / `AWAIT` 已结束句柄是 `E0053`。
  ⇒ 这节是本次更新的**核心**:没有它,AI agent 写出的服务端会踩中段首重放与
  阻塞 `SLEEP` 两个坑,而这两个坑的**症状**都不指向真因。
- **`reference.md` §28「v0.12.0 增量备忘」** —— 17 个新公共面成员的总表
  (§28.1)、五处语义变更 (§28.2)、传输层三条最易误诊的规则 (§28.3)、
  任务失败的报告义务 (§28.4)、本版实测读数 (§28.5)。
- **`reference.md` §9.14.1「传输层 `TCP_*`」** —— 9 个成员的签名表 + 超时形状
  + 「宿主挂起 vs 阻塞」的对照。
- **`reference.md` §9.2 / §9.3 / §9.1 / §9.11 / §9.12 / §9.15 补表行** ——
  `TO_INT` / `CAT` / `UTF8_ENCODE` `UTF8_DECODE` `BYTE_LEN` / `SLEEP_ASYNC` /
  `SCRIPT_PATH` / `TAKE_ERR`。
- **`examples/socket_server.wll`** —— 纯 WLWL 的最小 TCP 服务:演示「监听在任务外 /
  等待在段首 / 副作用在段外 / 字节数用 `BYTE_LEN`」的正确形状。

### Changed

- **`SKILL.md` 的 frontmatter `description`**:触发面从 v1.0 基线扩到
  **v0.12.0**(列出 17 个新成员与五处语义变更)⇒ AI agent 现在能被
  「写一个 WLWL 网络服务」这类请求**命中本 skill**。
- **`SKILL.md` 标题与「When to load」**:标明「spec v1.0 · compiler v0.12.0」,
  并在「Use when」里加一条**写服务端/客户端**的入口。
- **`reference.md` 头部 / 目录 / §2 版本表**:补 v0.12.0 行(214 → 231 成员)。

### Fixed

- ⚠ **`std.text::TO_INT` 之前根本没有登记**(§9.2 与 `SKILL.md` 的指针都缺)——
  而它与全局 `INT` 的分工是本版的规范要点(D-17):`TO_INT` **宽松**(裁剪 /
  十六进制 / 小数向零截断),`INT` **严格**(不裁剪 / 只十进制 / 小数形态拒 /
  失败是 `ERR` 值)。两处都补上了。
- ⚠ **「`PRINT` 在管道重定向时全缓冲」是错的** —— 实测 `PRINT` 是**行缓冲**
  (`println!` ⇒ Rust `LineWriter`),重定向到文件时进程存活期间即可见。
  本 skill 里**不**再暗示它是全缓冲。

## [Unreleased] — 规范基线切到 v1.0(合并册换代)

目标规范从 `wlwl-spec-v0.11` 切到 **v1.0**:权威处是独立仓
`Laffinty/wlwl-spec-v1`(11 册),仓内 `../docs/wlwl-spec-reference.md` 是把这 11 册
**合并成一份**的**次级载体**(机器守卫与 AI agent 读它)。

### Changed
- **当前基线的口径全部改成 v1.0**:`SKILL.md` 的 frontmatter `description`、
  标题、「何时加载」段、运算符速查表的标题;`reference.md` 的标题、卷首说明、
  §2 版本表、§24.5 引用表;`README.md` 的抬头 / Target version / Scope / See also。
  **历史沿革的表述原样保留**(v0.6 core / v0.7 §17 / v0.8 / v0.9 / v0.10 static
  contracts / v0.11 / v0.11.2 / v0.11.3 各批沿革)。
- **`description` 的触发面**:改前「anything targeting wlwl-spec-v0.11 (or
  v0.6/v0.7/v0.8/v0.9/v0.10 core)」,改后「anything targeting the wlwl-spec-v1
  baseline (or v0.6/v0.7/v0.8/v0.9/v0.10/v0.11 core)」⇒ **旧版本触发面一个没丢**
  (v0.11 仍可触发),只把「当前基线」从 v0.11 挪到 v1.0,并补了一句 v1.0 的错误
  模型(未消费 `ERR` 报 `E0103`)。
- **新增「章在哪」的说明**(`SKILL.md` 与 `reference.md` 卷首):合并册的二级标题
  带**册别前缀**(「语言规范 · 8 错误模型」「标准库规范 · 11 `std.encode`」
  「一致性册 · 5 …」)⇒ 跨册引用必须带册名(语言面 §5 与标准库面 §5 是两处不同的
  东西)。
- **`reference.md` §2「Version decisions」补 v1.0 一行**;§8 补 **§8.6 未消费
  `ERR`**:三条处置路径(消费者取走 / `LET(_, expr)` 显式丢弃 / 成为顶层程序值)、
  「观察型消费者不解除未消费状态」、以及 X-06 / X-07 两处已知缺口。

### 未改(有理由)
- **全部 `§N` 交叉引用原样保留** —— v1.0 的章号与 v0.11 对齐(语言面 §0–§12
  不变、对象面 §13–§16、并发面 §17;标准库面 §1–§22)⇒ 旧引用仍然有效。
- **成员表未重写** —— 成员面 21 个命名空间 / 214 个成员,v0.11 → v1.0 增删各 0。
- **沿革备忘 §23–§27 未动** —— 那是历史沿革,不是当前基线。

## [Unreleased] — 覆盖度收口(stdlib 全成员)

以标准库规范 `../docs/wlwl-spec-reference.md` 的**附录 A**(生成器
产出,与实现逐字节一致)为基准,本包三份文件的成员覆盖此前漏 **33 / 214** 个。
本批补齐到 **0 / 214**,外加修三处版本标签、补两批增量备忘。

### Added
- **`wlwl:std.math` 补 28 个成员**(`reference.md` §9.5 重写)。原 §9.5 是
  「the v0.11.2 twenty-one」的表(13 行),与 §9「每章成员不多」的结构冲突;
  现按规范 §7.1–§7.5 的五族分表:**A 基础与浮点内建 32**(v0.11 的 11 个与
  v0.11.2 的 21 个并成一表)/ **B `DECIMAL` 族 8** / **C `FRACTION` 族 8** /
  **D 近似比较 4** / **E 描述统计 9** = 61。⚠ 族内计数按规范的实际成员数写
  (8 / 8 / 4 / 9),不是先前备忘里说的 9 / 9 / 11。每族附「容易记反」条目:
  `scale` 与 `mode` 必填、分数是 `i64` 不是任意精度、统计族的「空」与
  「无定义」都返 `NULL`、`NaN` 不渗进统计量。
- **`wlwl:std.fs` 补 4 个成员**(`REMOVE` `COPY` `MOVE` `FILE_SIZE`)、
  **`wlwl:std.process` 补 1 个**(`PROCESS_ID`)(`reference.md` §9.12 新增
  「树形改动族」表,首列带签名)。`std.fs` 由此 18 个成员齐。
- **`reference.md` 补 §26 / §27 两节增量备忘**(此前只到 §25 v0.11)。
  §26 = v0.11.2(41 个新成员:`std.encode` 编码族 6 / `std.text` 大小写 2 /
  `std.str` 检索 2 / `std.collection` 10 / `std.math` 21,外加 `SUB` 迁移
  公式的修正)。§27 = v0.11.3(115 个新成员 + **一条可见面变更**)。
- **§27.1 写清 `ai` / `agent` 降级**:`IMPORT("ai:ask", …)` /
  `IMPORT("agent:task", …)`;`wlwl:std.ai` / `wlwl:std.agent` 不再解析
  (`E0040`);`wlwl:ai` 也不行(`wlwl:` 是 std 的保留前缀,会掉进裸模块名分支
  去找 `…\wlwl:ai.wll`);**没有命名空间限定调用语法**(冒号只出现在 `IMPORT`
  的路径串与改名处);要避开类型名冲突就在 `IMPORT` 时改名
  (`["TASK": "AGENT_TASK"]`)。成员名 / 签名 / 错误码一字未改,变的只是可见面。

### Fixed
- **`reference.md` §9.5 的小节标题过期**:原写「the v0.11.2 twenty-one」——
  那是 v0.11.2 的成员数,现在是 **61**(v0.11 的 11 + v0.11.2 的 21 +
  v0.11.3 的 29)。已改写为「the 61 members (stdlib §7, v0.11 → v0.11.3)」。
- **`SKILL.md` 的运算符速查表标题过期**:原写「(v0.6–v0.9)」。按
  `../docs/appendix_G.md`,**v0.7 之后没有新增任何内建**(106 条 = v0.2 的 70 +
  v0.4 的 15 + v0.6 的 4 + v0.7 的 17),所以这张表对**冻结在 v0.11** 的语言面
  是**完整的**;已改标题并在表首写明这一点,免得读者以为 v0.10 的类型注解与
  v0.11 的 `ASSERT` 变更漏收(前者不是运算符,见静态契约一节;后者见 §25.3)。
- **`reference.md` §2「Version decisions」表此前停在 v0.10**:补 v0.11 /
  v0.11.2 / v0.11.3 三行;§9 章首与 SKILL.md 的指针表改指 §26 / §27。
- `README.md` 的「Target version」此前说 v0.11.2 / v0.11.3 只加
  `wlwl:std.encode` 与 `wlwl:std.sanitize` —— 实际这两批加了 156 个成员与
  9 个新命名空间。已按实情改写,并补上 `ai` / `agent` 降级这一条。

### Added — examples
- **`examples/std_math.wll`**:v0.11.3 四个新族(`DECIMAL` / `FRACTION` /
  近似比较 / 描述统计)的可跑范本,断言用 `ASSERT_EQ` / `ASSERT`,退出码 0。

### 验证
- 覆盖度脚本 `impl/target/ci-verify-sandbox/check_skill_coverage.py` 以附录 A
  为基准,现报 **合计漏 0 / 214 个成员**(改前:33 / 214)。
- 所有新增的「实测」读数在 **wlwl 0.11.3(2026-10-20)** 上实跑取得
  (`impl/target/debug/wlwl.exe`):规范 §7.7 的整段示例、`DECIMAL` / `FRACTION`
  两族的边界值、统计族的「空」与「无定义」两态、`std.fs` 四成员与
  `std.process` 的 `PROCESS_ID`、以及 `ai:ask` / `agent:task` 的导入形态与
  `wlwl:std.ai` 的 `E0040`。
- `examples/` 下 20 个示例逐个实跑,**全部退出 0**(含新增的 `std_math.wll`)。

## [Unreleased] — v0.11.3 批次

语言规范仍冻结在 v0.11;本批只加标准库成员。**没有任何语言面改动** ——
`reference.md` 附录 G 逐字节不变。

### Added
- **`wlwl:std.sanitize` 收录**(`reference.md` §9.6 新章、`SKILL.md` 指针表):
  `HTML_ESCAPE` / `HTML_UNESCAPE` / `HTML_SANITIZE`。**开篇就是上下文分工表**
  —— URL 用 `URL_ENCODE`、标记用 `HTML_ESCAPE`、富 HTML 用 `HTML_SANITIZE`,
  三者**互不替代**,选错不是「保守」是错误。收录的要点:净化器是**输出点**工具
  (OWASP:输入侧清洗是 last resort)、非白名单标签**删标签留文本**、
  `script` / `style` / `iframe` **连内容整删**、**幂等但不往返**、
  「不清洗输入」这条威胁模型口径。
- **`std.encode` 补 `SHA256` / `HMAC_SHA256`**(`reference.md` §9.1):并写明
  **摘要不是加密、也不是口令哈希**(无盐、无工作因子、无拉伸;口令需要专用
  KDF,本库没有);`MD5` / `SHA-1` 已破不提供。

### Added (反模式)
- §20 补四条:**「清洗输入就安全了」是错的**、`HTML_SANITIZE` 不会「保留不危险
  的东西」(标签是删的)、**`HTML_ESCAPE` 双重转义是 bug**(定理只跑
  `UNESCAPE ∘ ESCAPE`)、`SHA256` 不是口令哈希。

### Fixed
- §9 章首的交叉引用此前写「`std.str` / `std.math` 的签名表在 **§25**」,而两章
  实际就在 §9.3 / §9.5 —— 按图索骥会扑空。已改指并补上 `std.sanitize` §9.6。
- `README.md` 的 impl 标签 0.11.2 → **0.11.3**;「本批只加标准库成员」那句
  补上 `wlwl:std.sanitize`。

## [0.11.2] — 2026-10-02

语言规范仍冻结在 v0.11;本批只加标准库成员与修正文案。

### Fixed
- **`SUB` 迁移公式的「修复」此前无效(N-1)**:0.11.0 那次把公式从
  `-(start, end_old)` 改成 `NEG(-(end_old, start))`,但 `NEG` 把刚算对的
  长度再取负 —— 与旧式**代数恒等**,产出负长度、`SUB` 返回 `""`。本批改为
  `SUB(s, start, -(end_old, start))`,并写明「不要包 `NEG`」。
- **删掉 `SLICE` 替代方案**:该建议在本批实测中被证伪 —— `SLICE` 首参必须是
  `ARRAY`,对字符串报 `E0030`;而 `E0030` 是原生诊断,任何 §8.3 消费者都拦不住,
  照此「迁移」的程序不是降级而是从此中止。
- 新增 `examples/sub_migration.wll`:上述两条各有断言,且门禁用 `PANIC` 而非
  `RUN_TESTS` —— 实测 `RUN_TESTS` 失败**退出码仍是 0**,拦不住 CI。

### Added
- **`wlwl:std.encode` 收录**(`reference.md` §9.1、`SKILL.md` 指针表):六个成员、
  失败口径(`DecodeError` 的 ERR 值 vs `E0022`/`E0030` 原生诊断)、`+` 不折成
  空格、base64 解码两张字母表都收 —— 四条都是容易记反的地方。
- **`wlwl:std.text` 收录**(`reference.md` §9.2):`TO_UPPER` / `TO_LOWER`,完整
  Unicode 简单大小写。**全局 `UPPER` / `LOWER` 仍是 ASCII-only 且本批不动**
  —— 替换全局内建是 breaking。`ß` → `SS` 会让长度变化;希腊 `Σ` 词尾得 `ς`。
- **`wlwl:std.str` 的字符串检索**(`reference.md` §9.3):`INDEX_OF(s, sub, from?)`
  与 `CONTAINS_SUB(s, sub)`。下标是**码点**,与 `SUB` 同一坐标系,拿到后可直接
  `SUB` 切回。**空 `sub` 报 `E0030`,不是恒匹配** —— 与同模块既有的 `COUNT`
  同一口径。
- **`wlwl:std.collection` 追加十个成员**(`reference.md` §9.4):`CHUNK` `WINDOW`
  `DEDUP_BY` `MIN_BY` `MAX_BY` `SUM` `PRODUCT` `FOLD_RIGHT` `POSITION`
  `KEY_BY`。附**规模警告**:建数组的成员在语言层是平方级,`WINDOW` 在 5000
  元素上已经要 20 秒 —— 调这类成员前先看 stdlib §5 的实测表。
- **`wlwl:std.math` 追加二十一个成员**(`reference.md` §9.5):超越函数族 + 整数
  数学。六条容易记反的已逐条写进该节:`TRUNC` 不是 `INT`(界外 `INT` 会中止
  运行)、`LN(0)` 是 `-inf` 不是错误、`DIV_CEIL` 只收整数且四个符号组合各有
  规律、`GCD`/`LCM` 取绝对值、`POW_MOD` 用欧几里得余数(负底给正余数)且
  走 kernel 所以支持 `2^61-1` 这种大模数、**`EXP` 的逆是 `LN` 不是 `LOG2`**。

### 纪律
- **未跑过的代码块就是假设**:引用模块成员的片段必须自带 `IMPORT`(名字是全局
  内建**只**以 `../docs/appendix_G.md` 为准),诊断消息不许凭记忆写。本包与
  `CONTRIBUTING.md` 同条规矩,起因见 v0.11.1 复核报告里三处缺前置的探针。

## [0.11.0] — 2026-10-01

对齐 v0.11.0 实现与 v0.11 规范。此前本包停在 0.10.1,**v0.11 的全部用户可见增量零收录**。

每条结论均以 `impl/target/release/wlwl.exe -V`(wlwl 0.11.0)实跑复核;标「实测」者附了观测日期。

### Fixed — 会让 agent 写出**跑不通或跑错**的程序

- **`gradual_typing` 的码段仍然说窄了(0.10.1 的修正本身是错的)**。本包两处
  坚持「只管 `E0110`–`E0112`,`E0113`–`E0115` 与开关无关」,并援引
  `main.rs` 的注释声称「已对实现验证」。**实测三个模块契约码全部受该开关管辖**:
  签名声明了实现没有导出的名字 → 无 `[features]` 时**退出 0、零输出**,
  `gradual_typing = "error"` 时 `E0114` 退出 1;签名与注解类型冲突 → 无开关 `E0115`
  同样不报;导入签名未声明的名字 → 无开关得到的是**另一个码** `E0023`(运行期),
  有开关才是 `E0113`。规范的 §11.2 明文「`gradual_typing` 管 `E0110`–`E0115`」。
  真正的实现口径在 `wlwl-types/src/diag.rs`(「`E0110`-`E0112` **+ Step 6 的模块契约**」);
  本包当年引的是 `main.rs` 里那句少算了模块契约的旧注释。**照旧文档写
  「签名文件存在就一定报」会误判,照更早的「只管到 112」会漏判。**
- **`wlwl check` 被说成做名字解析**(`reference.md` §24.3)。规范 §11.4 明文
  **不做**。实测:`check` 一个 `PRINT(NOPE)` 的文件 → **退出 0**,而 `run`
  同一文件 → `E0020` 退出 1。写「check 通过了所以名字都对」是错的。
- **`SUB` 迁移公式写反了**。原写 `SUB(s, start, end_old)` →
  `SUB(s, start, -(start, end_old))`。长度是 `end_old − start`,**实参顺序反了**:
  实测 `NEG(-(12, 7))` = `-5` → `SUB("Hello, world", 7, -5)` = `""`(空串),
  `NEG(-(7, 12))` = `5` → `"world"`。照抄必产出空串。附带一条实测:一元 `-`
  不是合法调用(`-(5)` 是 `E0022`),取负要用 `NEG`。
- **`INDEX` 的 1-based 口径只字未提**。`INDEX` 是**值 → 位置**查找且返回
  **1-based**,而 `xs[i]` / `INDEX_GET(xs, i)` 是 **0-based** —— 基数故意相反。
  实测 `INDEX([10,20,30], 10)`=`1`、`INDEX([10,20,30], 20)`=`2`、
  `INDEX([10,20,30], 99)`=`-1`、`INDEX_GET([10,20,30], 0)`=`10`、`xs[0]`=`10`。
  规范 §4.5 称这是本节「最容易踩的坑」,本包原来连基数都没写。
- **`E0012` 被当成返回类型失配码**。返回类型失配是 **`E0112`**(静态契约);
  `E0012` 是**语法**码(`expected ','`,见 0.10.1 条目里那条合法的
  `E0012` 记录)。本包两处把二者混为一谈。
- **引用了 v0.11 已不存在的 spec §16.3**。v0.11 把格式化器移进**附录 A.3**,
  「§16.3」现在解析不到,两处已改指 §A.3。
- **退出码表缺 2 和 3**。补全为 `0` / `1` / `2`(命令行用法错)/ `3`(输入文件
  解析不出来,恰为 7 个 parser 码)/ `101`(实现崩溃)。实测 `wlwl bogus` → `2`、
  `wlwl check` 缺 `<FILE>` → `2`。

### Added — v0.11 增量(**此前完全缺席**)

- **`wlwl:std.str`**(stdlib §6,新命名空间):`JOIN` / `SPLIT_LINES` / `CHAR_AT` /
  `COUNT` / `QUOTE`。码点索引,负下标自尾部计数(`CHAR_AT("abc", -1)` = `"c"`)。
- **`wlwl:std.math`**(stdlib §7,新命名空间):`ABS` / `MIN` / `MAX` / `FLOOR` /
  `CEIL` / `ROUND` / `SQRT` / `POW` / `CLAMP` / `PI` / `E`。域违例返回
  `ERR(["kind": "DomainError"])`;`FLOOR`/`CEIL`/`ROUND` **不是** `INT`。
  两个命名空间都必须 `IMPORT`,**`MIN`/`MAX` 不是全局内建**(裸调用是 `E0020`)。
- **⚠ BREAKING:`ASSERT` 按 §2.3 的八个假值判**。此前只把 `BOOLEAN(false)` 与
  `NULL` 当假,于是 `ASSERT(0)` / `ASSERT("")` / `ASSERT([])` / `ASSERT(DICT())`
  **会通过**;v0.11 起全部失败 `E0046`。实测逐个确认(每个单独一份文件):
  六个假值 → `E0046` 退出 1,`ASSERT(42)` → `OK(TRUE)` 退出 0。
- `reference.md` **§25**(v0.11 增量备忘,含上三张带签名的表)与 SKILL.md 的
  标准库指针同步。

### Fixed — 示例与时效性

- **`interp.wll` 命中了本包自己的反模式 #9**。`FORMAT("{0}: falsy", [label])`
  把 `label` 包成单元素数组,`{0}` 于是渲染**整个列表**:7 条输出注释写的
  `FALSE: falsy` / `NULL: falsy` / … 实跑全是 `[FALSE]: falsy` / `[NULL]: falsy` / …。
  已改为变参形态 `FORMAT("{0}: falsy", label)`,实跑与注释逐行吻合。
- **`match.wll` / `interp.wll` 的 `ERR` 变体臂整行静默消失**。
  `PRINT(classify(ERR("boom")))` 里 `classify` 返回的 `ERR` 经 §8.2 传播,
  在顶层被 §8.5 丢弃 —— **没有输出、没有诊断、退出码仍是 0**,教学点只演示了
  一半。已改为先 `LET` 绑定再用 `IS_ERR` / `ERR_PAYLOAD` 显式消费,两臂都可见。
- **`static_contracts.wll` 里名为 `max` 的函数返回的是较小者**
  (`IF(<(a, b), a, b)`)。输出注释 `// 3` 本身与实跑一致(所以不是注释失真),
  但函数名会教错 —— 已改名 `pick_smaller` 并注明 `MIN`/`MAX` 在
  `wlwl:std.math`。
- **幻码 `W0015` / `W0040` 仍在警告表里当活码**。规范 §11.2 明文**不定义**
  这两个:整数溢出是抛 `E0035`,没有饱和路径可警告;词法层不保留注释文本,
  `TODO(agent):` 无输入可查。已从表中移除并写明「不存在」。
- **内建条数说 110,实际 106**(`docs/appendix_G.md`)。已改。
- **`wlwl.lock` 全包零解释**。已在 README 的文件清单里说明它属于
  `examples/wlwl.toml` 的锁文件,以及何时才需要动它。
- **README 开头自称 v0.10、L31 又说 v0.11**。统一为 v0.11;并删掉
  「v0.11 只是重编号、语义不变」的说法 —— 那正是 `ASSERT` breaking 被漏掉的原因。
- **SKILL.md 承诺的「~70 名成员总表」在包内不存在**(它指向的 `reference.md` §9
  原本只有 6 行,写着「See SKILL.md pointers」,两边互踢皮球)。§9 改为指向
  SKILL.md 的分类表与新增的 §25,SKILL.md 删掉那个无法兑现的「~70」。

### Discipline

- 新增「实测结论必须标注验证版本」的纪律(SKILL.md References 与 README
  "Maintaining the skill")。本包两条招牌「实测结论」正是**因为没有版本标注**
  才在 v0.11 静默失效,而本包的可信度模型恰恰建立在「实测」之上。
- `wlwl-skill/examples/wlwl.toml` 的 `version` 字段由 `0.10.1` 升到 `0.11.0`。

## [0.10.1] — 2026-09-30

对齐 v0.10.1 实现。此前本包停在 0.10.0,而规范与实现已走了 13 个 Step。

### Fixed — 这些会让 agent 写出**跑不通或跑错**的程序

- **`YIELD` 5 条形态全部与实测相反**(最高危)。规范 §17.1 已在 R10-015 逐条改正,
  本包此前仍称「挂起后继续 / `[1, YIELD(), 3]` → `[1, NULL, 3]` / 循环恢复嵌套体
  剩余 / 间接调用也挂起」。实测:`LET(x, YIELD())` 里 **`x` 根本没绑定**(`E0020`)、
  `IF(TRUE, YIELD(), 42)` → `NULL`、数组字面量 → `NULL`、`WHILE` / `FOR`
  体内**只跑一轮**、`LET(y, YIELD); y()` → `E0020`。**全部零诊断。**
  补上可用的规律:塌掉的是**整个最外层表达式**;挂起点**之前**的绑定仍可见。
- **把中缀运算写成了合法示例**。本包两处把 `FUN((MUT), MUT + 1)` 列为
  "all valid / all parse" —— 实现**根本没有中缀解析路径**,`LET(x, a + b)` 是
  `E0011`。补上「一切运算皆前缀调用」这条根本规则(此前全包零命中)。
- **`match_exhaustiveness` 默认值说反**。此前两处说 default-off,实际是
  **缺省跟随 `gradual_typing`**。本包自己的 `examples/wlwl.toml` 只写了
  `gradual_typing = "error"`,按旧文档理解 `E0116` 是关的,实际开着。
- **`gradual_typing` 的码段说宽了**。此前说管到 `…116`,实际只管 `…112`。
  > ⚠ **本条在 0.11.0 被推翻**:模块契约码 `E0113`–`E0115` **也**受
  > `gradual_typing` 管辖,正确口径是 `E0110`–`E0115`。当年据以收窄的
  > `main.rs` 注释本身少算了模块契约。详见 0.11.0 条目。
- **裸 `THIS` 说成「零参引用」**。实际 `E0020: undefined name`,必须 `THIS()` ——
  且本包另一处还说它不是 `self` 的别名,自相矛盾。
- **`fmt --check` 被授权「一律忽略」**。补上可判定规则:**末句不带 `;`**(内层语句
  保留)、注释与缩进已被处理、**行尾自 v0.10.1 起无关**。并写明
  `examples/` 下 10 个 `.wll` 全部不过 `--check`,是手写教学材料。
- **`[package]` 三件套与 `W0001` 零覆盖**。R10-010 的教训只躺在示例注释里,
  `W0001` 也不在 W 码表。agent 照文档只写 `[features]` 会拿到看不懂的诊断;
  在 v0.10 上更是**静默拿到绿构建**。
- **`AND` / `OR` / `MODULE` / `EXPECT_ERR`(全局)已从注册表与附录 G 移除**
  (D10-019)—— 它们实测 `E0020`,从来就调不通。`EXPECT_ERR` 可用的是
  `wlwl:std.test` 的导出,**1 个参数**,且捕不到原生码。

### Added

- **示例接进 CI**:此前 `wlwl-skill/examples/` **不在任何测试覆盖面里**,
  与 R10-070 修掉的 `impl/examples/` 同一个洞。新增
  `crates/wlwl-cli/tests/skill_examples.rs`(3 条:全跑 / 主题守卫 / 清单三件套)。
- **五份新示例**,每份都实跑验证:`module_decl`(+依赖)、`sealed`(+svc+sig)、
  `module_signature`(+math+sig)、`match_exhaustiveness`、`object_identity`。
  覆盖的全是零示例特性:`MODULE` 形态、`SEALED` 面、`.wll.sig` 三向检查、
  `E0116`、对象身份 `==` 与 `obj.m()` 语法糖。

### Changed

- 查表补齐:`E0010`–`E0013` / `E0020` / `E0022` / `E0025` / `E0064` / `W0001`
  (此前被引用多次却查不到);§8.3 消费者 14 → **13 全局**(减去已移除的)。
- 补 `;` 规则、**没有 `{}` 块**、容器不可变与 `MERGE`(此前 `MERGE` 全包零命中)。
- 修正 v0.9 / v0.10 两处都被称作「the current standard」的自述矛盾。

## [0.10.0] — 2026-09-28

### Changed

- **Target spec** bumped from `wlwl-spec-v0.9.md` (archived at
  `docs/history/`) to **`wlwl-spec-v0.10.md`** (current). v0.10 is a
  *static-contracts* release: its **runtime is identical to v0.9's**, and every
  new feature is **default-off**. A program that adds no annotation and ships
  no `.sig` behaves exactly as before.

### Added

- `SKILL.md`: new section **"Static contracts — spec §2.6, §5.2, §5.2.1, §7.4,
  §9.1, §9.6 (v0.10)"** covering type annotations, container/function types,
  bounded type variables, the `gradual_typing` switch, module signature
  sidecars + `SEALED`, and the v0.10 diagnostic table.
- `SKILL.md`: writing-flow step 7 for opting into static contracts.
- `SKILL.md`: "When to load" now lists v0.10; the frontmatter `description`
  covers the v0.10 surface.
- `reference.md`: **§24 v0.10 增量备忘 — 静态契约** with the 附录 D delta, the
  annotation-grammar traps, the two new `[features]` keys, and the five new
  CLI subcommands (`sig`, `sig-gen`, `interface`, `schema`, `lsp`).
- `reference.md` §10: `E0110`–`E0116` added to the error table; `W0110`–`W0117`
  added to the warning list, with the "there is deliberately no `E0117`" note.
- `reference.md` §2: v0.10 row in the version-decisions table.
- `README.md`: target version, scope, contents, and a new **"Static contracts
  quick rules (v0.10)"** section.
- `examples/static_contracts.wll` — runnable demo of annotations, container
  types, and `T: Comparable`. Verified output: `3 10 / 10 / 42 / TRUE / TRUE /
  3 / a`.

### Fixed

- **Annotation-grammar traps now state measured behaviour, not the spec's stale
  claim.** Spec §5.2.1's "三条实测事实" facts #2 and #3 no longer match the
  reference implementation; the bundle documents what the compiler actually
  does and flags the spec text as stale:
  - `DICT<STRING, INTEGER>` → **`E0011` expected `')'`, got `Gt`** — a parse
    error. The spec says it parses silently into an opaque type name.
  - `FUN(INTEGER) -> STRING` → **`E0012` expected `','`, got `Minus`** — a
    parse error. Same stale claim.
  - `ARRAY[INTEGER]: Comparable` → **`E0010`** ("a type constraint may only
    follow a bare type variable"). The spec says `E0012`, which is the
    return-type-mismatch code and unrelated.
  - Still accurate, and kept: bare `ARRAY` is `E0010`; bare `DICT` / `OPTION` /
    `RESULT` parse as opaque named types with no error.

## [0.9.0] — 2026-09-25

### Changed

- **Target spec** bumped from `wlwl-spec-v0.8.md` (archived at
  `docs/history/`) to **`wlwl-spec-v0.9.md`** (current). v0.9 is a
  *language upgrade* over v0.8: true-suspension concurrency, structured
  cancellation, deadlock L1, and OOP with session-type protocols.
- **SKILL.md description + title** rewritten: "Writing WLWL (v0.9)"
  covers v0.9 concurrency semantics, OOP (§13–§16), linear THIS, and
  the error-code delta. Frontmatter description lists the v0.9 surface.
- **`wlwl-spec-v0.8.md` references** updated throughout SKILL.md /
  reference.md / README.md to point at v0.9; `docs/history/` archive
  paths preserved.
- **Concurrency section rewritten** (SKILL.md + reference.md §21):
  - `YIELD()` legal at **any** expression position inside a task body
    (v0.7/v0.8 "Block direct child" restriction removed).
  - Blocking `CHANNEL_SEND` / `RECV` **suspend** when full/empty with
    no peer; `ERR(ChannelWouldBlock)` path **removed**.
  - `TASK_CANCEL(task, reason?)` / `TASK_CANCEL_PARENT(reason?)` carry
    an optional DICT reason (default `{}`); non-DICT → `E0066`.
  - `AWAIT` of a cancelled task returns
    `ERR(kind="Cancelled", reason: <dict>)`.
  - Close wakes parked receivers with `ERR(ChannelClosed)` and parked
    senders with `E0054`.
- **OOP section added** (SKILL.md + reference.md §14–§16):
  `CLASS` / `NEW` / `GET_PROP` / `SET_PROP` / `CALL_METHOD` / `THIS`;
  session-type protocols (sequence + ⊕ + μ); linear `THIS` discipline.
- **Error-code table** (reference.md §10): `E0055` / `E0057` marked
  removed; `E0065` / `E0066` / `W0065` / `W0066` added; `E0014` /
  `E0032` / `E0050` / `E0051` redefined per v0.9 §11.2.
- **`wlwl.toml` features** (reference.md §19): added
  `strict_deadlock_detect`, `native_channel_close`,
  `channel_large_buf_threshold`.
- **Anti-patterns expanded to 24 rows**: new rows cover linear `THIS`
  escapes, external `SET_PROP`, protocol-order violations, non-DICT
  cancel reason, `ChannelWouldBlock` expectation, and deadlocks.

### Added

- **`examples/oop.wll`** — CLASS / NEW / methods with `self` injection,
  session-protocol sequence, and linear-`THIS` usage.
- **`reference.md` §23 v0.9 增量备忘** — compact index of every v0.9
  item affecting `.wll` writing (concurrency, OOP, error codes, types,
  features).
- **Types `CLASS` / `INSTANCE`** in the type/display/equality tables
  (reference.md §1, §11, §21).
- **`MODULE_REF(path)`** documented (reference.md §7) — loads module
  dict without binding names.

### Verified

- All `examples/*.wll` and `interp.wll` run with exit 0 against the
  v0.9 reference implementation.

---

## [0.8.1] — 2026-09-23

### Changed

- **Target spec** bumped from `wlwl-spec-v0.7.md` (archived at
  `docs/history/`) to **`wlwl-spec-v0.8.md`** (current). v0.8 is a
  *clarification / documentation / alignment* patch over v0.7 — v0.7
  core + §17 concurrency still required knowledge.
- **SKILL.md description + title** rewritten: "Writing WLWL (v0.8.1)"
  covers v0.8 字面增改 + v0.8.1 patch 修复. Frontmatter description
  lists every v0.8 字面增改 (12 items) and v0.8.1 patch 修复 (5 items).
- **`wlwl-spec-v0.7.md` references** updated throughout SKILL.md /
  reference.md / README.md to point at v0.8; `docs/history/` archive path
  preserved.
- **§12 reserved forms** rewritten in SKILL.md and reference.md:
  `CLASS` / `NEW` / `THIS` / `MODULE_REF` / `CALL` /
  (`AND` / `OR` / `MODULE` have since been **removed** as unreachable)
  `BuiltinSpec` records in `BUILTIN_REGISTRY`. The registry
  (`docs/appendix_G.md`) is the single source of truth; section §12 now
  points to it. Audit-driven (see `docs/history/audit-report-v0.8.1.md`
  §3.2 + spec §12 rewrite D8-005).

### Added

- **4 new anti-patterns (rows 16–19)**, audit §8.2 reproducer-driven:
  - **#16** Builtin name as value (`LET(f, +)` → E0020; `LET(f, PRINT)` → E0020; spec §4.3 op-tokens + §5.4 user-defined function only)
  - **#17** Float integer overflow / divide-by-zero NOT catchable (these are native codes E0035 / E1003 per §11.2 — not ERR; `EXPECT_ERR(/(1, 0))` does NOT save; spec §11.2 + §2.2)
  - **#18** Style guide rebalance: SUB 3rd arg is length, not end-index (v0.8.1 D8-010 observable change; spec §10.5; `SUB("Hello", 7, 5)` now returns `"world"`)
  - **#19** Float exponent literals work in v0.8.1 (`1.5e2`, `1e-3`, `1E3` per spec §1.7 EBNF `digits exponent`; pre-D8-01 raised E0011).
- **`YIELD()` placement note** softened: v0.7 / v0.8 spec §17.1 frames this
  as "理想语义 + 实现路径" (static task-body segmentation in
  `yield_split.rs`), not a hard rule. Future suspension-based schedulers
  may lift the limitation without breaking change (D8-007).
- **`NOT(ERR(x))` note**: spec §4.3 prose corrected (D8-006) — all
  operators including NOT propagate ERR transparently. Safe idiom:
  `NOT(BOOL(ERR(x)))` for safe coercion.
- **String literal subscript** (§4.5 prose + §A.2 grammar, v0.8.1
  D8-012): `"hi"[0]` parses and evaluates to `"h"`. INT / FLOAT /
  Boolean / NULL literal postfix remains rejected at parser (no
  `INDEX_GET` semantics).
- **`MUT` as binding name** (§1.4, v0.8.1 D8-011): `LET(MUT, "x")` is
  valid; MUT is a context keyword only after LET-modifier slot.
- **`=` triple-identity** (§1.5, D8-008): `=` is `==` in call position
  AND a separator for INDEX_SET sugar AND for default-param — three
  roles, no lookahead needed.
- **`%` float E0030** (§2.2 + §11.2, D8-008): `%` with any FLOAT arg
  raises E0030 (was previously listed in only spec prose; E0030 row
  updated).
- **`EXPECT_ERR`** (§8.3, D8-008): added to the §8.3 consumer table
  (was in prose only — explicit table entry for lookup).
- **`SHIELD` / `SCOPE(ERR)` / `AWAIT` host diagnostic clarifications**
  (§17.1 / §17.5, D8-008): user ERR vs host diagnostic now distinct;
  `SCOPE(ERR("x"))` propagates transparently (E0052 not fired).
- **`TASK` name-collision** (§2.1 / §10.11, D8-008): type name `TASK`
  vs `wlwl:std.agent.TASK` / `wlwl:std.ai.TASK` — same name, no
  conflict; recommended full-qualified write style.

### Verified

- All `examples/*.wll` in the bundle still parse under v0.8.1 spec
  (mechanical re-check; no source change to examples required).
- `interp.wll` (gold) extends with a `SUB` length example
  (`SUB("Hello, world", 7, 5)` → `"world"`) demonstrating D8-010 fix.

---

## [0.7.0] — 2026-09-22

### Added

- **Concurrency (spec §17)** section in `SKILL.md`: SCOPE / SPAWN /
  AWAIT / YIELD / TASK_* / SHIELD / CHANNEL_* with hard rules
  (explicit SCOPE, YIELD placement, ChannelClosed kind, TRY_* preference).
- **5 new antipatterns** (rows 11–15): top-level SPAWN, bare-branch
  YIELD, NULL-as-close, non-suspending SEND/RECV, unhandled Cancelled
  AWAIT.
- `examples/concurrency.wll` — SCOPE + two SPAWN + YIELD + channel
  fan-in (TRY_*), self-printing PASS lines.
- `examples/concurrency_cancel.wll` — TASK_CANCEL_PARENT inside SHIELD,
  deferred `TASK_IS_CANCELLED`, ChannelClosed kind check.
- `reference.md` §12 concurrency reference: builtin table, ERR kinds,
  `E0052`–`E0058`, type names `TASK`/`CHANNEL`, equality/display notes.

### Changed

- Target spec **wlwl-spec-v0.7** (additive over v0.6). Authoritative
  path: `../docs/history/wlwl-spec-v0.7.md`; v0.6 archived at
  `../docs/history/wlwl-spec-v0.6.md`.
- Deviations pointer → `../docs/history/deviations-v0.7.md`
  (former `docs/history/deviations-v0.10.md`).
- `SKILL.md` description + title cover v0.7 concurrency; writing flow
  checklist gains a concurrency step.
- Equality note: `==(f, f)` / `==(h, h)` are TRUE (handle identity).

## [0.6.1] — 2026-09-20

### Changed

- Extension notes for `.wll` (was `.wl`).
- Authoritative spec path → `../docs/history/wlwl-spec-v0.6.md`.

### Known impl/spec deviations (not skill bugs)

- Integer overflow: spec says `ERR(E0035)` consumable via `UNWRAP_OR`;
  current impl surfaces a runtime diagnostic (exit 1). Recorded in
  `docs/history/deviations-v0.7.md`.
- Captured-`LET` upgrade (legacy E-CloCap) vs §3.3 strict reading —
  prefer `LET MUT` for shared mutation (skill antipattern #1).

## [0.6.0] — 2026-09-20

### Added

- Nine v0.6 decisions documented (truthy overhaul, short-circuit,
  IF-ERR routing, `!` canonical, `AT_K`, string subscript, `LET MUT`,
  overflow `E0035`, `${}` interpolation).
- Gold examples through `N` blocks in `interp.wll`.
