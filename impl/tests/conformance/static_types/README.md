# conformance:static_types —— 静态契约夹具(计划书 §10.4 / §12.1)

计划书 §10.4 规划的 `impl/tests/conformance/static_types/` 目录,内容是
**只有静态层能抓出**的问题:程序本身合法、能跑,门开着才报。

## 目录约定

`static_types.wll` 用**两份注释**声明期望,照
[`../README.md`](../README.md) 的「第 1 行 = 期望诊断码」惯例扩展:

```
// expects: E0110
// mode: both
```

| 字段 | 含义 |
|---|---|
| `expects` | `gradual_typing = "error"` 时**必须出现**的码(空格分隔多个) |
| `mode` | `both` = 默认档也必须干净(不写则按 `on` 处理) |

`mode: both` 的两条是**控制组**:`clean_baseline`(完全合规)与
`exhaustive_result`(补齐 `ERR` 分支的 `RESULT` 匹配)。零破坏不是靠断言,
是靠控制组 —— 证明这套夹具在默认档下不会给仓库添乱。

## 为什么夹具必须解析干净

跑的是**真的二进制**(`wlwl check`),不是内部函数。所以「门开着才报」这条
断言有个前提:报出来的必须是**静态诊断**,不能是语法错冒充的。锁测试用
「默认档 `wlwl check` **通过**」来保证这一点 —— 默认档不做静态检查,于是
它通过就意味着夹具**解析干净**。

**故意违例的夹具不要求 `wlwl run` 跑通**:静态类型违例在运行期多半**也**是
`E0030` 类型错误,那正是 `E0030` 存在的意义,不是夹具坏了。

`mode: both` 的控制组承担另一半责任:声明「两种模式都干净」的那两份
(`clean_baseline` / `exhaustive_result`)**必须** `wlwl run` 一次跑通 ——
否则这套夹具与这套断言都可能只是摆设。

## 跑法

```bash
cargo test -p wlwl-cli --test conformance_static
```

锁测试还会逐条验证:默认档 `wlwl check` 通过(夹具解析干净)、门开着时报出
期望的码、`mode: both` 的控制组 `wlwl run` 一次跑通。
