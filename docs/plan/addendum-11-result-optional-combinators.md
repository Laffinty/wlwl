# 11 · `RESULT` 组合子(把 §8 的消费者名单变成可链的形态)

> **层级** L2 · **硬前置** 00(R0 治本)
> **状态** 未动工 · **规范登记** 本批**不在** §14 清单里 —— 由本方案首次提出

## 0. 契约摘要

- **目标**:给 `OK` / `ERR` 一套**组合子**,让「先算一步、错了就提前退出、
  成功就继续」这类控制流不必每次都手写 `ERR_PAYLOAD` + 分支。
- **非目标**(违反即范围事故):
  - **不改 §8.2 的传播语义**。「非消费者拿到 `ERR` 就在调用边界抛出」这条
    是语言语义,组合子**不得**成为绕过它的后门。
  - **不新增 `OPTIONAL` 类型**。wlwl 用 `NULL` 表示「没有」,不引入 Rust 的
    `Option` —— 那需要语言层的「缺失」与「空」的区分,见 §3.2。
  - **不引入 `?` / `unwrap` 语法糖**(语言变更)。
  - **不把组合子做成宏**。它们是普通 `StdFn`,可被普通函数组合。
  - 不改 14 个 §8.3 消费者的**任何**一个(它们是语言面,不是标准库面)。
- **为什么值得做**:`std.encode` 的规范里已经写着调用方要用
  `AT_K(ERR_PAYLOAD(BASE64_DECODE(s)), "kind", "?")` 来查错误载荷 ——
  **每次都要这么写一遍**,说明缺一层组合。

## 1. 基线(可核验)

```powershell
cd docs
Select-String -Path spec/wlwl-spec-v0.11.md -Pattern '8.3' -Context 0,4
cd ..\impl
Select-String -Path crates/wlwl-std/src/*.rs -Pattern 'ERR_PAYLOAD' | Select-Object -First 6
```

| 项 | 现值 | 出处 |
|---|---|---|
| §8.3 消费者 | **14 个**入口 | 语言规范 §8.3 |
| `OK` / `ERR` 值 | 存在;`ERR_PAYLOAD` 是**全局内建** | 附录 G |
| 组合子 | **0** | 附录 A(104 成员,无 `std.result`) |
| 现有调用样例 | `AT_K(ERR_PAYLOAD(BASE64_DECODE(s)), "kind", "?")` | `wlwl-skill/reference.md` §9.1 |
| 索引失败口径 | 数组越界**抛 `E0036`**,**不**返 `NIL` | skill §20 反模式条目 |
| 主流对照 | Rust `Option` / `Result` 的 `map` / `and_then` / `unwrap_or`;Scala / Kotlin 的 `map` / `flatMap`;Python 无内建(靠异常) | — |

## 2. 成员面

**命名空间裁决建议:新建 `std.result`,不塞进 `std.test`。**
理由:`std.test` 是测试面(6 成员,`TEST` / `ASSERT` / …),组合子是**生产代码**
的通用控制流工具。混在一起会让「测试专用」与「语言通用」两个性质不同的面
纠缠。

**层:R1 门面 or R2?** 组合子只在 `OK` / `ERR` 上做投影,**不构建新容器** ——
理论上 R1 可行。但 §0.2 的 R2 立场是「直通 `Value` 不经 `wrap()` / compat」,
而 `OK` / `ERR` 在 eval 侧有特殊表示 ⇒ **R2**(否则每次投影都要过 compat)。

| 成员 | 层 | 语义要点 |
|---|---|---|
| `IS_OK(v)` | R2 | 判别式 |
| `IS_ERR(v)` | R2 | 判别式 |
| `MAP(r, f)` | R2 | `OK(x)` → `OK(f(x))`;`ERR` **原样透传**(不碰 `f`) |
| `AND_THEN(r, f)` | R2 | `OK(x)` → `f(x)`(可返回 `OK` **或** `ERR`);`ERR` 原样透传 |
| `FLATTEN(r)` | R2 | `OK(OK(x))` → `OK(x)`;`OK(ERR(e))` → `ERR(e)` |
| `OR_ELSE(r, fallback)` | R2 | `OK` → 原样;`ERR` → `fallback(e)` |
| `UNWRAP_OR(r, default)` | R2 | `OK(x)` → `x`;`ERR` → `default` |
| `UNWRAP(r)` | R2 | `ERR` → **中止**(抛原生诊断)。名字要让人知道它会炸 |
| `ERR_MAP(r, f)` | R2 | 只改 `ERR` 的载荷 |
| `ERR_OR_ELSE(r, e)` | R2 | `OK(x)` → `OK(x)`;`ERR(_)` → `ERR(e)` |
| `RESULT_ALL(rs)` | R2 | 全部 `OK` → `OK(ARRAY)`;任一 `ERR` → **第一个** `ERR` |
| `RESULT_ANY(rs)` | R2 | 任一 `OK` → 第一个 `OK`;全 `ERR` → 第一个 `ERR` |
| `NULL_OR(v, default)` | R2 | `NULL` → `default`。**明确不是 `UNWRAP_OR`** —— `NULL` 不是 `ERR` |

## 3. 语义裁决点(开写前必须逐条回答)

1. **命名空间归属**(§2)。新建 `std.result` vs 塞进 `std.collection`。
2. **`OPTIONAL` 要不要?** §0 非目标说「不引入」。但组合子族一旦成型,
   `NULL` 的「可能没有值」与「空字符串 / 空数组」在 `MAP` 一族里就撞上了。
   → 裁决:本份**只做 `RESULT`**,`NULL` 侧只给 `NULL_OR` 一个最小成员;
   「要不要 `OPTIONAL`」单独立项(它需要语言层区分「缺失」与「空」)。
3. **`MAP(r, f)` 收什么形态的 `f`?** wlwl 能不能传函数值?
   (§15 提到 OOP 的 `CALL_METHOD`,语言有闭包 `FUN`。) → 需确认 `StdFn`
   能不能接闭包 —— 若不能,组合子必须写成 R1 形态(调用点展开)。
4. **`UNWRAP` 失败用什么诊断?** 建议 `E00xx` 专用码(如 `E00xx: UNWRAP on
   ERR`)。**不能复用 `E0102`**(那是「ERR 逃逸到顶」)—— 两者含义不同,
   混用会让调用方分不出「我没接住」和「我没消费」。
5. **§8.2 传播会不会被绕过?**(§0 非目标第 1 条)关键:组合子**必须**接收
   非消费者拿到的 `ERR` 吗?—— 建议**必须**,否则 `MAP` 就成了 §8.2 的旁路,
   整条消费者注册制度就漏了。**这条要有专门用例。**
6. **`RESULT_ALL` 遇到 `ERR` 时要不要把其余结果也求值完?** 短路还是全部求值
   —— 求值顺序与副作用(例如 `ASK` 这类有副作用的成员)有关。→ 裁决短路。

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

契约表 `tests/result_contract.rs`(新):

| 组 | 用例 | 备注 |
|---|---|---|
| 判别 | `IS_OK` / `IS_ERR` 对 `OK` / `ERR` / 普通值 | 平凡但必须有 |
| 透传 | `MAP(ERR(x), f)` ⇒ **`f` 不被调用**(用计数器断言) | **核心不变量** |
| 透传 | `AND_THEN` / `OR_ELSE` / `ERR_MAP` 同款 | |
| 展平 | `FLATTEN` 的 4 种组合 | |
| **§8.2 旁路** | 组合子**必须**能接非消费者拿到的 `ERR`;接了就**不得**抛 `E0102` | 防制度漏 |
| `UNWRAP` | `ERR` 上 `UNWRAP` ⇒ 专用诊断码(**断言不是 `E0102`**) | 裁决 §3.4 |
| `RESULT_ALL` | 短路:**第二个是 `ERR` 时,第三个副作用不发生** | 裁决 §3.6 |
| `NULL_OR` | `NULL` vs `""` vs `[]` **三者区分**(别被当成同一个) | |
| 性能 | `MAP` / `AND_THEN` 100 万次 ≥ 200 MB/s(**依赖 00**) | |

## 5. 九处表落点

九处见 `docs/history/20261002.md` §5.0。**本份九处全中**(新命名空间):

| 落点 | 本份动作 |
|---|---|
| 1 规范成员表 | 新增 §22 `std.result` 章 + 13 行 + **「不绕过 §8.2」条款** + `UNWRAP` 诊断条款 |
| 2 附录 A | 跑 `gen-appendix-a` |
| 3 `SPEC.functions` | 新 `src/result.rs` + 13 个 `StdFn` |
| 4 契约测试硬编码 | `result_contract` 成员数与点名清单 |
| 5 模块登记 | `ALL_SPECS` 登记 `result`;`src/` 文件数守卫 |
| 6 probe | ≥ 2 条(链式 / 短路),`EXPECTED_CASE_COUNT` 同步 |
| 7 附录 G | **不动**(`ERR_PAYLOAD` 是既有内建) |
| 8 skill 指针 | `SKILL.md` + `reference.md` §9.14;**并把 §9.1 里那句 `AT_K(ERR_PAYLOAD(...))` 改写成用组合子的新写法** |
| 9 CHANGELOG | 成员面 +13;命名空间 +1 |

## 6. 风险与已知代价

- ⚠️ **组合子可能成为 §8.2 的旁路**(裁决 §3.5)。这是本份**最需要盯**的一条 ——
  消费者注册制度是语言级的设计,标准库给它开后门是结构性损伤。用例必须钉住。
- ⚠️ **`MAP` 收什么形态的 `f`**(裁决 §3.3)决定了这批成员是 R2 还是 R1,
  也决定了它们能不能被组合。若接不了闭包,组合子就只是语法糖的另一种写法,
  价值大幅缩水。**先测这个。**
- ⚠️ **`UNWRAP` 会炸**。它必须**显眼**(名字 + 文档 + 专用诊断),
  且 skill 反模式要写「生产路径不用 `UNWRAP`」。
- **明确不做**:`OPTIONAL` 类型 / `?` 语法糖 / 宏 / 改 §8.3 的 14 个消费者 /
  改 §8.2 传播语义。

## 7. 检查点

| 子项 | 内容 | 状态 | commit |
|---|---|---|---|
| W-01 | 命名空间归属 + 闭包形态实测(裁决 §3.3)+ §8.2 旁路裁决 | 未动工 | — |
| W-02 | 判别 / 透传 / 展平族 + 契约(**不依赖 00**) | 未动工 | — |
| W-03 | `UNWRAP` 专用诊断 + 契约 | 未动工 | — |
| W-04 | `RESULT_ALL` / `RESULT_ANY` / `NULL_OR` + 契约 | 未动工 | — |
| W-05 | §9.1 文档改写 / 性能 / 收口 | 未动工 | — |
