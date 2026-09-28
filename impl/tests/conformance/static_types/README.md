# conformance:static_types —— 静态契约夹具(计划书 §10.4 / §12.1)

计划书 §10.4 规划的 `impl/tests/conformance/static_types/` 目录,内容是
**只有静态层能抓出**的问题:程序本身合法、能跑,门开着才报。

## 目录约定

**一份夹具一个子目录**,每个子目录里只有一份 `main.wll` 和**一份**契约:

```
static_types/
  README.md
  annotation_mismatch/main.wll            // expects: E0110
  annotation_and_arg_mismatch/main.wll    // expects: E0110
  arg_type_mismatch/main.wll              // expects: E0111
  return_type_mismatch/main.wll           // expects: E0112
  comparable_violation/main.wll           // expects: E0111
  bounded_var_violation/main.wll          // mode: both  (R10-025 哨兵)
  non_exhaustive_result/main.wll          // expects: E0116
  unreachable_clause/main.wll             // expects: W0117
  clean_baseline/main.wll                 // mode: both  (控制组 1)
  exhaustive_result/main.wll              // mode: both  (控制组 2)
```

契约用**两份注释**声明,照 [`../README.md`](../README.md) 的「第 1 行 = 期望诊断码」
惯例扩展:

| 字段 | 含义 |
|---|---|
| `expects` | `gradual_typing = "error"` 时**必须出现**的码(空格分隔多个) |
| `mode` | `both` = 默认档也必须干净(不写则按 `on` 处理) |

## [v0.10.1 / R10-011] 为什么要拆成子目录

以前这里是一份 `static_types.wll`,装着 8 个 `// expects:` 块和 2 个
`mode: both` 块。`contract_of()` 读到文件头的第一段就 `break`,于是:

- 实际参与断言的只有 `E0110` 一条;
- `E0111` / `E0112` / `E0116` / `W0117` 在夹具层面**零断言** ——
  实现里写错了也绿;
- `contract_of` 旁边的注释写着「不猜:解析不出契约就让测试失败」,但第一个块
  解析成功,守卫就通过了。**它防的是「没有契约」,防不住「契约只读到第一段」**。

拆开之后「读头部」与「读到全部」变成同一件事,本目录的约定才第一次真正成立。

拆分立刻暴露了三处断言与现实不符(都是**旧断言在借别的块的输出**):

| 夹具 | 原来声称 | 实际 | 处置 |
|---|---|---|---|
| `annotation_and_arg_mismatch` | `E0110 E0111` | 只有 `E0110` | 如实改成 `E0110`,并在注释里记下 R10-086 |
| `return_type_mismatch` | `E0112` | 先撞 `E0110`,`E0112` 被吞 | 去掉 `LET` 注解,让 `E0112` 真正可达 |
| `bounded_var_violation` | `E0110` | 一条诊断都没有 | 改成 `mode: both`,转成 R10-025 哨兵 |

## 三条守卫

`cargo test -p wlwl-cli --test conformance_static` 里有三条测试专门防
「夹具加了但没被验」:

1. `static_types_fixtures_diagnose_only_when_the_gate_is_on` —— 枚举**全部**
   子目录并断言目录数等于 `EXPECTED_STATIC_FIXTURE_DIRS`(10)。加了夹具
   目录却没让它进覆盖,这里就红。
2. `static_types_fixtures_cover_every_diagnostic_they_claim_to` —— 断言
   `E0110` / `E0111` / `E0112` / `E0116` / `W0117` 每个码都至少被一份夹具
   声称。这条直接封死上面那个「7 条死断言」。
3. `a_contract_written_after_the_code_is_not_a_contract` —— **负向**守卫:
   契约写在第一行代码之后,必须被判为「什么都没断言」。将来谁再把多段契约
   塞进一份夹具,这条立刻拦住。

## 为什么夹具必须解析干净

跑的是**真的二进制**(`wlwl check`),不是内部函数。所以「门开着才报」这条
断言有个前提:报出来的必须是**静态诊断**,不能是语法错冒充的。锁测试用
「默认档 `wlwl check` **通过**」来保证这一点 —— 默认档不做静态检查,于是
它通过就意味着夹具**解析干净**。

这条约定也决定了什么**不能**进本目录:文档里警告的
`ARRAY[INTEGER]: Comparable`(约束挂在类型之后)报的是 `E0010`,那是**解析错**,
程序根本不合法。它属于解析层的测试,不属于静态层。

**故意违例的夹具不要求 `wlwl run` 跑通**:静态类型违例在运行期多半**也**是
`E0030` 类型错误,那正是 `E0030` 存在的意义,不是夹具坏了。

`mode: both` 的控制组承担另一半责任:声明「两种模式都干净」的那几份
(`clean_baseline` / `exhaustive_result` / `bounded_var_violation`)**必须**
`wlwl run` 一次跑通 —— 否则这套夹具与这套断言都可能只是摆设。

## `mode: both` 也可以是「已知的漏」

`bounded_var_violation` 是 R10-025 的哨兵:规范文法
`BoundedVar = identifier ":" Type` 不允许右嵌套,可今天
`T: Comparable : Integer` 静默通过。

断言**不能声称还不存在的行为**,所以这条先钉成 `mode: both`(现状 = 干净),
而不是 `expects:`(声称会有诊断)。修 R10-025 的那天它会**自动变红** ——
那正是它该干的事:逼着改的人明确决定「现在该报什么码」。同理,把一条早就
写好的期望悄悄放任变成真话,是这套夹具最需要防的失败模式。

## 跑法

```bash
cargo test -p wlwl-cli --test conformance_static
```

锁测试还会逐条验证:默认档 `wlwl check` 通过(夹具解析干净)、门开着时报出
期望的码、`mode: both` 的控制组 `wlwl run` 一次跑通。
