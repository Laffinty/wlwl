# WLWL v0.11.2 复核报告 —— 上轮复核发现处置验证 + 标准库扩展抽验

| | |
|---|---|
| **复核对象** | 发布包 `wlwl-v0.11.2-x86_64-pc-windows-msvc`(wlwl.exe 报 `0.11.2`)+ 仓库 main@24a9421 |
| **复核基线** | [v0.11.1 复核报告](wlwl-v0.11.1-recheck.md)(N-1~N-5)· CHANGELOG v0.11.2 · 构建计划 `docs/plan/wlwl-v0.11.2-build-plan.md`(W-01~W-08 工作项 + D12 台账)· 中途审查修复 `24a9421`(四处缺陷) |
| **复核方式** | 同等标准:① 动态 —— 重建上轮全部回归套件(d01–d23 + f01–f15)于 v0.11.2 二进制实跑,新写 **17 个探针**(g01–g08 系列)覆盖本版 41 个新成员的功能面 / 错误路径 / 规范特载口径;② 静态 —— 逐项核对 CHANGELOG、构建计划 W 工作项、D12 台账、两册规范、契约测试与守卫。测试文件全部置于发布包 `review-tests/`,工作目录零过程文件 |
| **日期** | 2026-10-02 |

---

## 0 总体结论

**上轮复核报告(v0.11.1)的五条发现全部得到妥善处置,其中一条(N-4)被诚实判定为我方错判并在我的报告顶部留下了更正块 —— 处置质量值得肯定。** v0.11.2 标准库成员面 58 → 99(新命名空间 `std.encode` / `std.text`,math 11 → 32,collection 17 → 27,str +2),抽验全部符合新规范条款,包括中途审查(`24a9421`)修掉的四处缺陷。上轮回归套件在本版二进制上零行为漂移。

**本轮新发现 1 个 P1(N-6:`MIN_BY` / `MAX_BY` 空数组违反规范明文,且契约用例因 key 函数恰好对 NULL 免疫而未能拦截)+ 2 个 P3。**

| 上轮编号 | 内容 | 复核判定 |
|---|---|---|
| N-1 (P1, skill) | SUB 迁移公式 `NEG(-(end_old, start))` 与旧错误代数等价 | **已修**:公式改为 `SUB(s, start, -(end_old, start))`,并新增「Do **not** wrap it in `NEG`」反例警告;旧公式里错误的 `SLICE` 替代建议(对字符串报 E0030)一并更正;新增 `examples/sub_migration.wll`,实测五条自检全 ok(含「NEG trap still yields empty」) |
| N-2 (P3) | 「防死注入反向守卫」声明与代码不符 | **已修**:`every_injected_kernel_is_used_in_its_module_source`(stdlib_mirror.rs:275),按 CHANGELOG 描述剥注释与字符串后匹配调用点,并带负向自检 `the_dead_injection_guard_actually_rejects_one`(:293,验证「注释里提到不算调用点」—— 正是当初 `_DIAG_E0038` 的存活形状) |
| N-3 (P3) | 全局 FORMAT 与 std.format 成员对 RESULT 渲染不一致,§4「同一函数」不成立 | **已修(裁决:保留差异,写明口径)**:规范新增 §4.1「两条路径刻意不同(D12-003)」+ 新建 `tests/format_contract.rs` 锁住两个输出,防将来被「好心统一」 |
| N-4 (P3) | skill 退出码 2/3 未收录 | **我方错判**:reference.md 在 v0.11.1 早已完整收录 0/1/2/3/101 五档(现 v0.11.2 位于 :446-447);我此前的 grep 模式没匹配到表格行。处置方在 **我的复核报告顶部** 追加「复核实测更正」块(D12-007)、正文一字未改 —— 正确做法,我接受该更正 |
| N-5 (P3) | `E0014` span 指向 IMPORT 行;`W0051` span `0:0` | **已修(动态证实)**:`E0014` 实测 `2:33`(精确指向 YIELD 表达式,比 CHANGELOG 自报的 `3:1` 更准);`W0051` 实测 `2:7`(`Warning` 结构体补了 span 字段,W0051/W0030/W0065/W0066 四码同受益) |

---

## 第一部分 上轮发现的处置细节(逐项实证)

### 1.1 N-1:SUB 迁移公式(skill)

- `reference.md:501`:迁移式 `SUB(s, start, -(end_old, start))`,散文正确(「length is `end_old − start`」);`:505` 专设一条 **「Do not wrap the result in `NEG`. `NEG(-(end_old, start))` re-negates the length you just computed」** —— 把我报告指出的「NEG 包裹 = 代数等价于旧错误」写成了显式反例;
- `SKILL.md:633`(反模式 #18)同步,并删掉了旧的 `SLICE(s, start, end_old)` 建议 —— 静态审查确认 `SLICE` 对字符串首参报 `E0030`,旧建议本身就是第二个处方错误;
- 新示例 `examples/sub_migration.wll` 实跑五条自检全 ok(length 语义 / end-index 迁移 / NEG 陷阱 / 负长度钳 0 / 负起点自尾计数)。

### 1.2 N-2:守卫形态(与声明一致)

`every_injected_kernel_is_used_in_its_module_source`:对注入表中每个 (module, kernel),要求剥掉注释与字符串字面量后的门面源码里出现该名字(不要求左括号 —— `_RANGE` / `_EXPECT_ERR` 是 `LET(RANGE, _RANGE);` 式裸改名导出);负向自检证明「文件头散文提到 kernel 名」不算调用点。`str.wll` 本版新增的 `_DIAG_E0022` 注入恰被该守卫验到确有调用点 —— 守卫已实战运转。

### 1.3 N-3 / N-5

- §4.1(D12-003)全文写明两路径的机制差异(eval 侧 display vs std 边界解包)与「裁决保留差异」;`format_contract.rs` 双向锁。我的 v0.11.2 探针复测:全局 `FORMAT("{0}", OK(1))` → `OK(1)`,成员 → `1`,与规范一致;
- span 修复实测见 §0 表。

### 1.4 过程项(W-05 / W-06)

- W-05:我的复核报告顶部已出现「⚠ 2026-10-02 复核实测更正」块(N-4 判不实、N-1 补充 SLICE、三处探针缺前置),正文未改写 —— 符合「更正留原文件」的验收;
- W-06:`CONTRIBUTING.md:91` 已立「探针必须自带 IMPORT / 查附录 G 而非假设」的成文规矩。

---

## 第二部分 v0.11.2 新增标准库抽验(41 个新成员)

### 2.1 `std.encode`(6 成员,R2)—— 全部符合 §11

| 探针 | 实测 | 判定 |
|---|---|---|
| `BASE64_ENCODE("foobar")` | `Zm9vYmFy` | ✓ RFC 4648 §10 向量 |
| `BASE64_DECODE(BASE64_ENCODE("hello, world"))` | 原文往返 | ✓ |
| `BASE64_DECODE("Zg==")` / `("Zg=")` | `"f"` / **DecodeError** | ✓ 后者是 `24a9421` 修复项:补位数量校验(此前 `"Zg="` 静默返回 `"f"`) |
| `BASE64_DECODE("ab!c")`、`URL_DECODE("%ZZ")` | DecodeError | ✓ 非法字符拒绝 |
| `HEX_DECODE("414")`(奇数长度) | DecodeError,载荷 `[kind: DecodeError, op: HEX_DECODE, reason: hex string has odd length 3]` | ✓ 与 §11 逐字同口径(带 op/reason) |
| `URL_ENCODE("a b/c?")` / `URL_DECODE("a+b")` | `a%20b%2Fc%3F` / `a+b` | ✓ RFC 3986 口径,`+` 不折空格 |

### 2.2 `std.text`(2 成员,R2)—— 全部符合 §12

`TO_UPPER("straße")` → `STRASSE`(ß→SS,长度变化)、`TO_LOWER("SIGMA Σ")` → `sigma σ`(词尾变体)、`TO_LOWER("ΟΔΟΣ")` → `οδος` ✓;全局 `UPPER` 实测仍 ASCII-only(`"héllo"` → `"HéLLO"`)—— 与 §12「两个并存」口径一致,契约测试钉住全局不动 ✓。

### 2.3 `std.math` 新增 21 成员 —— 抽验全部符合 §7

- `TRUNC`:界内与截断一致(2.7→2.0、-2.7→-2.0),`TRUNC(1e20)` 原样直通;对照全局 `INT(1e20)` → `E0035` 中止 ✓(CHANGELOG 特载的差异逐字成立);
- `LN(0)` → `-inf`、`EXP(1000)` → `inf`(与「±inf 原样」既有口径一致);`LN(-1)` / `ASIN(2)` → DomainError ERR 值 ✓;
- `POW_MOD(-7, 3, 5)` → `2`(欧几里得余数,非 Rust `%`)、`POW_MOD(2, 61, 2305843009213693951)` → `1`(i128 中间积,大模数)✓;
- `GCD(12,18)=6`、`LCM(4,6)=12`;`GCD(INT_MIN, 3)` → `E0034` 中止 —— §7 失败列已如实登记该口径(24a9421 #3)✓;
- `DIV_CEIL(7,2)=4`、`(-7,2)=-3`、`(7,-2)=-3`,`DIV_CEIL(7.0,2)` → `E0030` 带名(不提升)✓;
- `SIGN(-5)` 恒 `INTEGER` ✓;`IS_SQRT(-1)` → DomainError ✓;
- `LOG` 确认**不是**成员(`IMPORT` → `E0023`)✓(契约册的专门断言与实测一致)。

### 2.4 `std.str` + `std.collection` 新增成员 —— 功能与错误路径

- `INDEX_OF`:码点下标(`"héllo"` 找 `"l"` = 2 ✓)、`from` 参、缺失 → `-1`、空 sub → `E0030` 带名(三成员统一口径 ✓)、变长元数错 → `E0022` **带函数名前缀**(新注入 `_DIAG_E0022` 生效,补上了 v0.11.1 时代「闭包元数错无名前缀」的缺口)✓;`CONTAINS_SUB` ✓;
- `CHUNK` / `WINDOW`:`n>LEN` 方向相反(一块 vs 零块)✓、`n=0` → `E0030: size must be >= 1` ✓;
- `DEDUP_BY`:`[1,"1",2,1]` 按 STR 渲染键 → `[1, 2]`(撞键规则与 GROUP_BY 一致)✓;
- `SUM`/`PRODUCT`:提升(`[1,2,3.5]`→6.5)与单位元(`0` / `1`)✓;
- `FOLD_RIGHT([1,2,3], FUN((x,acc), -(x,acc)), 0)` → `2`(= 1-(2-(3-0)),参数序 = 元素在前,与 REDUCE 一致)✓;
- `POSITION`:0 起下标、非布尔谓词 → `E0030` 带名 ✓;`KEY_BY` 后者覆盖 ✓;
- `MIN_BY`/`MAX_BY`:与 `SORT_BY` 首尾等价 ✓、key 抛 ERR 按 8.2 传播(实测 TRUE)✓ —— **但空数组行为违反规范,见 N-6**。

### 2.5 成员面与镜像

附录 A 已重生成:`std.encode` / `std.text` 行俱在,§0.2 登记成员面 58 → 99(12 命名空间;`std.ai` 5 成员受 real-ai 门控,默认可见 94)✓;规范不升版的理由(D12-011:纯新增不触发分册升版)在 CHANGELOG 有完整论证。

---

## 第三部分 回归验证

上轮全部套件重建后于 v0.11.2 实跑:**零行为漂移** ——
- d01–d23:str/math/collection/test 功能面逐字节一致;kernel 不外泄(E0023)、不重导出(E0023)、`E0038`、混合排序、SUB 钳制、`TYPE(ERR)` 消费者语义、真值八假值、NaN 可达性全部不变;
- f01/f01b/f10:`NEG`/`ABS(INT_MIN)` → `E0034`(消息逐字不变);f03/f15:SORT 比较器 ERR 传播 + 载荷保真;f14b:整数键过界 `E0030`;f02:真负零进出保号;f05c:std.format 成员 `OK(1)` → `1`;f06:release 下 `WLWL_STD_SRC` 仍被忽略;f08:`std.web` 仍缺席;BOM `.wll` 仍可运行;
- 随包示例与 skill 示例全绿(v0.11.2 skill 新增 `sub_migration.wll` 等示例一并通过)。

---

## 第四部分 本轮新发现

**N-6(P1)|`MIN_BY` / `MAX_BY` 空数组中止运行,违反 stdlib §5 明文「空数组得 `NULL`」。**

```wlwl
MIN_BY([], FUN((s), LEN(s)));   // error[E0030]: LEN: expected string/array/dict, got null   rc=1
MAX_BY([], FUN((x), LEN(x)));   // 同上
```

- 根因(`stdlib/collection.wll` `_EXTREME_BY`):空数组守卫写的是 **`IF(<(LEN(arr), 0), NULL, ...)`** —— `LEN` 恒非负,该分支**永不可达**;空数组照常走到 `CALL(key, AT(arr, 0, NULL))`,把 `NULL` 喂给用户 key 函数,`LEN(NULL)` 即中止。规范 §5 主表(24a9421 刚并表的那行)明文「空数组得 `NULL`」。
- **门禁为何没拦住**:`collection_contract.rs:554/559` 存在 `min_by_empty_array_is_null` / `max_by_empty_array_is_null` 两条冻结用例,但二者的 key 恰好是**恒等函数** `FUN((x), x)` —— 对 `NULL` 免疫,`key(NULL)` 返回 `NULL`,断言「得 NULL」照样绿。**这是「契约用例的输入选取掩盖实现缺陷」的实证**:换个普通 key(如 `LEN`)缺陷立现。这与 std.web 终止时的教训同构 —— 门禁测「写出来的用例」,测不出「用例的输入是否踩在缺陷面上」。
- 修法:守卫改 `==(LEN(arr), 0)`(或 `< 1`);契约表**补一条非 NULL 容忍 key 的空数组用例**(如 `MIN_BY([], FUN((s), LEN(s)))` → `NULL`),否则现有两条用例对回归零防护力。

**N-7(P3)|`IS_SQRT` 命名与返回类型相悖。** 规范 §7 明文 `IS_SQRT(n) -> INTEGER`(整数平方根,实测 `IS_SQRT(16)=4`、`IS_SQRT(15)=3`),契约一致;但语言内 `IS_` 前缀家族(`IS_OK` / `IS_ERR` / `IS_EMPTY`…)全部是谓词,这个名字会被当成布尔判断用。建议在规范该行加一句「返回平方根本身,不是布尔」的显式警示,或在未来大版本评估改名(按 ADR-0023 走弃用两步)。

**N-8(P3)|skill README 的 impl 版本标签停在 0.11.0。** `wlwl-skill/README.md:4` 仍写 `(impl 0.11.0; spec wlwl-spec-v0.11)`,而本版 impl 为 0.11.2、标准库成员面已翻近一倍;v0.11.1 复核后 skill 对齐时未同步该行。(`reference.md` 的 scale warning 等新内容倒是标注了「Measured on wlwl 0.11.2」—— 同包内版本标注不一致。)

---

## 第五部分 结论与建议

1. **上轮五条发现处置完毕,无遗留。** 处置方对我报告错判(N-4)的更正方式(更正块置顶、原文不改写、登记 D12-007)是我见过的对「审查报告本身也需要审查」最干净的实践;
2. **v0.11.2 的 41 个新成员抽验全部符合规范**,包括两条特载口径(TRUNC vs INT 的界外差异、POW_MOD 的欧几里得余数)与中途审查修掉的 BASE64 补位校验(动态复现修复前后差异:半补位现在正确报 DecodeError);
3. **待修清单**:N-6(P1,一行守卫 + 一条契约用例)→ N-7 / N-8(P3,文档卫生)。建议 N-6 随下一个补丁版处置;
4. 本轮维持上轮的方法论结论:动态实测仍是唯一权威 —— 本轮两个「虚惊」(HEX_DECODE `"41"` 是偶数长度、`std.text` 成员名是 `TO_UPPER`)与一个「门禁盲区」(N-6 的恒等 key)都只有跑真实程序才暴露。

## 附:复核产物清单

- 动态:发布包 `review-tests/` 下 d01–d23 / f01–f15(回归重建)+ g01–g08(新成员探针,含 7 个错误路径小文件),全部在 v0.11.2 二进制实跑;
- 静态:CHANGELOG v0.11.2、`wlwl-v0.11.2-build-plan.md`(W-01~W-08 + D12 台账)、两册规范新增章节(§4.1/§5/§6/§7/§11/§12)、`collection_contract.rs` / `encode_contract.rs` / `format_contract.rs`、`stdlib_mirror.rs`(W-02 守卫)、`24a9421` 提交全文;
- 我方报告的更正块(v0.11.1 复核报告顶部)已核读并接受;工作目录零过程文件,本报告是 docs/review 唯一新增。
