# WLWL v0.8 独立第三方测评报告

> 测评对象: `wlwl.exe` v0.8.0 (Windows x64, build 2026-09-23)
> 参照规范: `docs/standard/wlwl-spec-v0.8.md`
> 测评侧重: **实现是否完整 / 是否与规范一致 / 是否存在偏差或隐性缺陷**
> 写作日期: 2026-09-23

本报告只列实际观察到的**问题与偏差**。实现大体能跑通基本程序（解构、模式匹配、并发、`std.collection`/`std.json`/`std.fs` 等都正常工作，`concurrency.wll`、`concurrency_cancel.wll`、`interp.wll`、complex 自测全部 exit 0），故而下面只摘录**实测不一致 / 反直觉 / 文档显式声称但实测不符**的点。

---

## 1. 词法 / 解析

### 1.1 浮点指数 `digits "." digits exponent` / `digits exponent` **未实现**
规范 §1.7 写明：
```
float_lit = digits "." digits | digits exponent | digits "." digits exponent .
exponent  = ( "e" | "E" ) [ "+" | "-" ] digits .
```
实测：

| 源码 | 实测结果 |
|------|---------|
| `1.5e2` | `E0011 expected ')', got Ident("e2")` |
| `1e2`   | 同上 |
| `1.5e-2`| 同上 |

整个 exponent 路径完全不接受。规范同时没有 fallback 说明，只有 `E0011`。这与 §1.7 字面冲突。

### 1.2 关键字 `MUT` 不能用作普通标识符
规范 §1.4 原话：
> `MUT` 是上下文关键字：仅在 `LET` 之后的位置具有特殊含义；其他位置可作普通标识符使用，不与关键字冲突。

实测：

| 源码 | 实测结果 |
|------|---------|
| `LET(MUT, "x")` | `E0010 expected identifier in LET, got Mut` |
| `LET MUT(MUT, "x")` | 同上 |

`MUT` 在任何 LET 路径内都不能用作 binding 名字——这与 §1.4 直接冲突。skill 的 SKILL.md 把它当作"上下文关键字"教给用户，但底层的 parser 在 binding name 槽位直接 `got Mut` 拒绝。

### 1.3 `${...}` 内的字面量嵌套字符串被硬性禁止
规范 §1.8 只字面规定：`nested string literal inside ${...} interpolation is not allowed` 的措辞并未出现于 §1.8，但实现每次都报 `E0001 nested string literal inside ${...} interpolation is not allowed`。

实际影响面非常广。任何 `${FUN_CALLED("literal arg")}`、`${ARR["key"]}`、`${ARR[0] = "x"}` 等长度在一行的内插表达式都不可写，必须拆出 LET 才能用。

这并不是真正的 bug，但**文档（§1.8 + 附录 A 词法）**应当显式承认这一限制或在 §1.8 中加 normative note。否则用户对照规范写出的代码会**大量触发 E0001**，skill 的 anti-pattern #8 也只把它作为"先 LET 再插值"小贴士给出，对照实现则这是默认的拒绝路径。

### 1.4 调用非函数值的运行时诊断被提前到 parse 阶段
规范 §4.2：
> 调用将实参自左向右全部求值后执行被调者。被调者不是函数值时产生 `E0020`。

实测 `(42)(1, 2)` 报 `E0011 expected ')'`，而非运行时 `E0020`。`42(1, 2)` 作为顶层 statement 时同样在 parser 阶段拒绝，没机会走到 §4.2 的"求值后判断被调者"。

这意味着 `E0020` "调用非函数值" 这一分支根本无法从源码出发触发——用户若想验证 §4.2 的运行时错误处理路径，唯一的方法是先把 `42` 放进 LET 再在下一行 `LET MUT(_f, 42); LET(_x, _f(1))`（这条路径会报 E0020，但写法极不自然）。

---

## 2. 类型与值

### 2.1 浮点除零没有 NaN 路径
规范 §2.1 / §2.2 / §2.3 把 `NaN` 列为第 8 个 falsy。但实测：

| 源码 | 结果 |
|------|------|
| `/(0.0, 0.0)` | `E1003 /: division by zero`（原生诊断） |
| `/(1.0, 0.0)` | 同上 |

即浮点 IEEE 754 行为本应产生 `NaN`，实现却直接抛原生 `E1003`。这导致 §2.1 描述的"含 NaN 的字典 key / 容器相等"行为（`E0031` NaN-as-key）永远无法从源码构造并验证——实际测试中此分支的 spec coverage 为 0。

deviations-v0.7.md / changelog v0.6.0 §B.8 都曾提到"div-by-zero 一律 E1003 不返回 IEEE NaN"，但 v0.8 §2.2 仍把 `NaN` 列在 falsy 列表里——文档与实现脱节。

### 2.2 `==` 在 `ERR` 上同时被 §2.4 / §8.3 描述为"做比较"和"透传"
规范 §2.4 明示 OK/ERR 自身需要遵循 `OK(a) = OK(b) iff a = b` 的相等规则。
规范 §8.3 表又写：`==/!=` "对 ERR 操作数透传而非消费"。

二者**直接互斥**——若 §8.3 生效（ERR 操作数透传），则任何 `==(ERR(...), ERR(...))` 都无法得到 `TRUE`；若 §2.4 生效，`==` 必须有 §8.3 消费者身份（否则会触发更一般的 §8.2）。实测 `08_errors_v2.wll` 中：

```
==(OK(1), ERR("e"))   → 透传（IS_ERR 返回 TRUE）
MATCH(r, [[OK(x),...],[ERR(e),...]]) 内部某 ERR 路径 → 函数体不执行，见 §7
```

→ 用户不能写 `==(ERR("a"), ERR("a"))` 也不能写 `IF(r == OK(0), ...)`. 这是 §7 / §8 三条规则同时启动造成的死角，规范文本本身需要把"ERR 实参位置的 `==`"统一为"先消费后比较"或"始终透传"，**两种行为之一**。

### 2.3 空字典与空数组显示同为 `[]`
规范 §2.5：
> 数组为 `[e1, e2, ...]`，字典为 `[k: v, ...]`

实测 `STR(DICT())` → `[]`。这两种值在 §2.4 已互相不等（深度比较先比类型/键集），但在 §2.5 的渲染层无法区分。规范没有显式给出空字典的显示约定，这是不一致。

### 2.4 `NaN` 是 falsy 但实际不可达
见 2.1。`v0.8 docs appendix` 与 `examples/truthiness.wll` 都把 `NaN` 列在 8 个 falsy 中，但当前实现在 `0.0/0.0` 上抛 E1003，`NaN` 字面量也没有构造路径，所以 falsy 表的"第 8 项"**形同虚设**。

---

## 3. 绑定 / 作用域 / 闭包

### 3.1 不可变绑定的闭包"升级"遗留行为与 §3.3 直接冲突
规范 §3.3：
> 单元格可变标志由绑定形式决定：`LET` 建立的为不可变，`LET MUT` 建立的为可变。该标志一经设定**不再改变**。

CHANGELOG v0.6.0 也明示 "Cell mutability flag is permanent"，并在 Known limits 中写道 "Captured-`LET` upgrade (legacy E-CloCap) still diverges from a strict reading of §3.3 — prefer `LET MUT` for shared mutation"。

但仓库根 `README.md` 的 "Quick example" 第 18-19 行直接给出：

```
PRINT(step());   // 1
PRINT(step());   // 2 — counter shared via closure
```

它对应的 `examples/closure_cell.wll` 实际输出却是**`NULL`**（不是 `3`）：

```wlwl
LET(n, 0);
LET(step, FUN((), LET(n, +(n, 1))));
LET(c, step()); LET(c, step()); LET(c, step());
PRINT(c);   // 实际输出 NULL；注释期望 3
```

**文档/示例与实现/规范三者**打架：
- 规范 §3.3：`LET n` 不可变 → 闭包内不能 `SET(n, ...)` → 注释期望的累积行为本就不合法
- 实现：当前正确按 §3.3 走（函数体只 `LET(n, +(n, 1))`，shadow 出新单元；返回值 NULL），结果是 NULL
- README + examples/closure_cell.wll：声称输出 1 / 2 / 3

`README.md` 与 `closure_cell.wll` 都应当改写为 `LET MUT(n, 0)` 的版本，与 §3.3 对齐。这是一处 README/示例与规范+实现直接背离的高优先级不一致。

### 3.2 内建名不可作为一等值
规范 §5.4：
> 函数值可以存储、传递、返回、放入容器，没有名与值的区别。

实测：

| 源码 | 结果 |
|------|------|
| `LET(f, +); LET(_x, f(1, 2))` | `E0020 undefined name +` |
| `LET(P, "P bound")` 然后 `SET(...P...)` | OK（用户标识符可绑定） |
| `LET(f, PRINT)` | `E0020 undefined name PRINT` |

也就是说 `+`, `-`, `PRINT`, `LEN`, `PUSH`, `MAP`, `FILTER` 等内建函数都是 lexer-macro 级别或在 dispatch 表里注册的**不可被引用的值**。这与 §5.4 "函数是一等值" 大相径庭——根本没有 `+` 这种"函数值"可以放进容器、可以当 callback、可以作为 `MAP(arr, +)` 的实参；skill 反复把 `+` 写成 `+(a, b)` 也是这个原因，否则 curry / partial application / 容器内塞函数 等一等值用法全部失效。

### 3.3 `LET(name, value)` 在实现中等价于"先求值 name 再绑定"
设：

```wlwl
LET(y, y);        // E0020 undefined name y
```

规范 §3.1 / §5.1 都把 LET 的第一参数描述为 binding pattern，不应作为表达式求值。impl 实际用 `LET(name, value)` 形式时**先把 name 当表达式求一遍**——如果 name 是个全新名字（首次出现），则触发 E0020。`(name = expr)` 这种用法一旦名字未绑定就会撞出红诊断；要绕过只能写成 `LET(x, 99); LET(x, x+1)` 或在 LET 出现前 LET MUT 占位。这是 lexer-macro / parse 阶段没有把 LET 当特殊形式处理。

能跑通的大多数例子只是因为 `name` 是 `$var` 这种局部名，第一次 LET 之前没人用它；但这个隐式契约在功能上完全堵死了 spec §3.1 列举的 `LET([a, b], arr)` 之外的容错空间。

---

## 4. 表达式 / 控制流

### 4.1 INDEX_SET 对字符串接收者报 `E0030` 是规范行为，但 message 不与 spec 措辞一致
规范只说"不接受字符串"，实际报错文案是 `E0030 INDEX_SET: STRING is read-only; use SUB/SPLIT/CODEPOINTS for writes`。spec §11.2 的 `E0030` 没列这分支。这只是用户体验层面的小问题，但与规范保持 verbose / terse 的统一原则有偏差。

### 4.2 IF 在 spec §6.1 的 ERR 处理有时不符直觉

```wlwl
LET(classify, FUN((r),
    MATCH(r, [[OK(x), "ok"], [ERR(e), "err"]])   // MATCH 不是 §8.3 消费者
));
LET(c, classify(err_val));
```

实测：`err_val` 是 `ERR("boom")`，调用 `classify(err_val)` 时 impl 整体进 §8.2 透明传播——**函数体根本没执行**，"enter classify" 只在 ok_val 调用时打印一次。这正是规范 §8.2 的"arg 是 ERR → 不进入 body"的字面行为，但配合 §7 的 MATCH（不在 §8.3 注册表里）让"对 ERR 做模式分发"成为不可能——必须用 trampoline / 解构函数形式，不能让 ERR 直接流过 MATCH 隔离层。规范 §7 与 §8.3 在这一点上是相互掣肘的，要么 MATCH 进 §8.3，要么 `==`/`MATCH` 等都必须明示。

### 4.3 `%` 浮点类型检查实际触发
规范 §2.2：
> 任一实参是 FLOAT 时产生 `E0030`（类型错；v0.8 起列入 `%` 的触发列表）

实测 `%` 不接受 FLOAT，与规范一致 ✓。但 §11.2 错误码表 `E0030` 行的描述仅有"类型错误（含 ERR_PAYLOAD/WRAP/CALL 的误用; INDEX_SET 接收字符串）"，**没有**单独列出 `%` FLOAT 触发——这部分规范/实现已对齐但 spec 表没补。

### 4.4 字符串下标 / 字面量下标在 LET 直接内联报 E0011

```wlwl
LET(_x, "hi"[5]);    // E0011 expected ')', got LBracket
```

规范 §4.5 明确允许 `Expression "[" Expression "]"`。`"hi"[5]` 在源代码里是合法表达式，但放在 LET 第二槽位时 LET 的 lexer-macro 没把这个 `[...]` 当 postfix 处理——必须先 `LET(s, "hi"); LET(_x, s[5])` 才行。这与 "operators are functions, postfix is expression" 的全局设计是相互独立的死穴。

`01_lexer.wll` 的实测量证明 `[1, 2, 3][0]` 作为**顶层表达式**求值能跑通（`v0.8 关键放宽点`），但**夹在 LET/IF/任何带 `(` 的调用内部**时一律失败。

---

## 5. 函数

### 5.1 类型注解 `name: TYPE` 在 strict_types 未开启时仍然触发 `E0030`
规范 §5.2：
> 运行时忽略，除非模块清单开启 `strict_types`(9.6)。

实测：

```wlwl
LET(anno, FUN((n: INTEGER), *(n, 2)));
LET(_r, anno("hello"));
```

→ `E0030 *: cannot multiply string and integer`。在没有 `wlwl.toml` 启用 strict_types 的情况下，仍按强类型抛 `E0030`，且用的是 `E0030` 而非 `E0033`（规范在 §11.2 明确把 strict_types 违例分为 `E0033`）。这意味着 §5.2 "默认忽略类型注解" 这一条的 normative 承诺在实现里被默认 ON——实际"开启 strict_types"的差异不是"是否启用"，而是"是否允许运行时再放一次 catch"。

### 5.2 默认参数行为
未实测深入，但 §5.2 的 "默认表达式在被调函数的词法帧内求值" 没有验证工具（spec 里没有 deterministic test 用例）。`05_functions.wll` 仅默认参数 + *rest 的最小测试通过；动态语义需要更复杂的回归覆盖。

---

## 6. 错误模型 / TOP-LEVEL

### 6.1 整数溢出 / 除零 / `NEG(i64::MIN)` 报**原生诊断**，不是 `ERR(...)` 沿 §8.2 透明传播
规范 §2.2：
> `+`、`-`、`*` 溢出时**抛错** `E0035`（INTEGER 与 FLOAT 越界共用此码）。`NEG` 于 INTEGER 下界取负产生 `E0034`。

实测：都报原生诊断 exit 1。**§B.8（v0.6）** 写："Integer overflow → ERR(E0035)；was saturate + W0015 in v0.5"，意思应该是**变成 ERR 可以沿链路处理**。但实现层面：
- `+(MAX, 1)` 不是 ERR——是原生 E0035 退出
- `/(1, 0)` 不是 ERR——是原生 E1003
- `EXPECT_ERR(/(1, 0))` 在 `10c_json_test_fs.wll` 直接报 "uncaught diagnostic" 让 TEST 失败

deviations-v0.7.md 第 H 项也承认这一点。但 v0.8 §2.2 措辞不变，用户照 spec 写的 TRY/UNWRAP_OR 救不回来——这是数据建模上的失信。

CHANGELOG 同步声明：
> "Integer overflow: spec says ERR(E0035) consumable via UNWRAP_OR; current impl surfaces a runtime diagnostic (exit 1)."

也就是说作者知道，但没在 spec §2.2 / §11.2 中显式注记 "**warn** ERR-as-value 路径当前由原生诊断替代"——这一行应**进规范**。

### 6.2 `CALL(...)` 在 v0.8 registry 已登记但**作为值表达式不能调用**
规范 §12 v0.8 重写后清单里没有保留形式列；§10.3 又写 `CALL(fn, args...)` 是"保留形式(第 12 章)"。实测 `CALL(PRINT, "x")` 与 `CALL(FUN(...) , 1)` 均无法被识别为可调用构造（在 LET 槽位以外的"裸"调用位置上不报错，能 parse，但行为不可预期）。规范 §10.3 与 §12 自相矛盾，且 v0.8 §12 取消了清单。

### 6.3 `PANIC` 后没继续，没有 sanitizer / cleanup hook
`PANIC` 后 exit 1，spec §11.4 写退出码 1 实测一致。但测试中 `15i_e0100_panic.wll` 的 `PANIC` 在 `LET MUT(_z, ...)` 内被求值后整进程退出——没有 RUN_TESTS 报告该测试"fail"这种"软失败"通道。本语言没有 try/catch（TRY 不接 PANIC），`UNWRAP(ERR)` 也走 PANIC——用户写测试时无法将 PANIC 隔离成 passed=FALSE。

---

## 7. 标准库 / 并发

### 7.1 `SUB(s, start, len)` 在 start ≥ 7 + 短串上结果错误
实测：

```text
SUB("Hello, world", 0, 5)  → "Hello"     OK
SUB("Hello, world", 1, 5)  → "ello"      OK
SUB("Hello, world", 7, 1)  → ""          BUG (expect "w")
SUB("Hello, world", 7, 3)  → ""          BUG
SUB("Hello, world", 7, 5)  → ""          BUG
SUB("Hello, world", 0)     → "Hello, world"   OK
```

这是 §10.5 SUB 的内部 bug。spec §10.5 只承诺"按码点"，但 7+5 vs "Hello, world" 应该是 "world"。可能实现把超过某个 offset 之后的 start 当作 "0" 处理，或与 ±1 边界混了。

### 7.2 FORMAT 的 `{0}` 解析在 §10.7 上限清晰但实际是**全列表而不是序列**

```wlwl
LET(_v, [10, 20, [30, 40]]);
PRINT(FORMAT("head={0},{1} tail={2}", a, b, rest));
```

`FORMAT("head={0},{1} tail={2}", a, b, rest)` 中 `{0}=a, {1}=b, {2}=rest` → 工作正常。
但 `FORMAT("head={0},{1} tail={2}", [a, b, rest])` 时 `{0}` 渲染为完整列表 `[a, b, rest]`，后续 `{1}`、`{2}` 因没有 args 而原样保留。

这与 skill 的 anti-pattern #9 一致，也与 §10.7 的"`{N}` 是 args 第 N 项"一致，所以**实现无误**；但**这条规则非常反直觉**——FORMAT 用户期望 variadic，第一个参数是模板后续是若干值而不是"列表里的位置 N 项"。建议 spec 改写为"FORMAT 接受 variadic，第 0 个非模板实参对应 `{0}`"或者同时支持 splat `*[arr]`，否则无数初学者会踩。

### 7.3 `EXPECT_ERR(...)` 实际不工作
v0.8 §8.3 把它加入 consumer 表；实测 `EXPECT_ERR(/(1, 0))` 撞到原生 E1003 终止，而不是 OK(payload)。`ASSERT(IS_OK(EXPECT_ERR(/(1, 0))))` 在 `10c_json_test_fs.wll` 直接让整个 RUN_TESTS 报一个 failure。`EXPECT_ERR` / `TRY` / `UNWRAP_OR` 三者面对数字溢出与除零全部失效——见 §6.1。

### 7.4 `TYPE` 没有把 v0.7 的 `TASK` / `CHANNEL` 句柄 typo 风险堵住
实测 `TYPE(handles)` 均返回 `"TASK"` / `"CHANNEL"` ✓。但 §10.11 给出的"建议"：

```wlwl
IMPORT("wlwl:std.ai", TASK, MODEL);
LET(handle, wlwl:std.ai.TASK("summarize", "long text..."));
```

全限定写法需要 `wlwl:std.ai.TASK(...)` 解析。impl `IMPORT` 不支持这种 `module.member` 形式的 namespace member 解析——实测 `IMPORT("wlwl:std.ai", ["TASK"])` 可以，但 `wlwl:std.ai.TASK` 这种全限定调用不可用。spec §10.11 把这一用法列为推荐，但实现并不支持。

### 7.5 并发章节大部分通过；但有两处偏差
- **§17.4 noted dev 偏差保留**：captured-LET 升级在跨任务路径仍生效，规范 §3.3 文字层面要求严格，但 impl 仍走 deviation D-something。CHANGELOG 已知，本报告不重复。
- **`AWAIT` re-await of same handle**: 实测 `12a_conc_basic.wll` 中没有 re-await 测试，但同名示例 `concurrency.wll` 也没测——这是一个 silent 偏差：第一次 AWAIT 拿到值后，handle 是否还有效？

---

## 8. wlwl-skill 的可用性评估（独立维度）

### 8.1 skill 与实现主版本错位
- skill 的 `SKILL.md` frontmatter 明确写 `"wlwl-spec-v0.7"` 并写 "Do NOT use for v0.5 or earlier"
- 但 **v0.8 已经发布**（spec 2026-09-23），CHANGELOG 列出的 v0.8 改动（字面量下标、§12 重写、`EXPECT_ERR` 入表、`%` 浮点 E0030 列入、`LET`/`FUN` 不对称 normative 等）**没有一条** 写进 skill 的 SKILL.md / reference.md。
- skill 的 README 仍然指 `docs/standard/wlwl-spec-v0.7.md`，且 .wll 引用也是 v0.7 路径。结论：skill 名字与内容与最新版不匹配；用来写 v0.8 程序时**对 v0.8 新引入的字面量下标、`EXPECT_ERR` 进入 §8.3、`%` 对 FLOAT 触发 E0030、`§12` 已删空 等都需要自学**。

### 8.2 skill 没有覆盖若干关键陷阱
- skill 提到 `EXPECT_ERR` 在 anti-pattern 里**完全没出现**——v0.6.1 / v0.7 changelog 都把它移到 consumer 表，但 SKILL.md 顶部加粗强调的"§8.3 ERR consumers"13 条枚举里只有 12 条。
- 没有 anti-pattern 提示：**内建名不可作为值**（见本报告 §3.2），用户想 `LET(f, +)` 直接撞死。
- 没有 anti-pattern 提示：**字符串字面量直接下标**在 LET 第二参数槽位不能 parse（见 §4.4）。
- 没有 anti-pattern 提示：**integer overflow / div-by-zero / NEG(MIN) 不会被任何 consumer 接住**（见 §6.1）——这一点 changelog / deviations 都承认，但 skill 仍以"UNWRAP_OR 兜底"的措辞默认引导用户使用 TRY / UNWRAP_OR。
- 没有 anti-pattern 提示：内置的 PRINT 不输出回车 / 没有 println，但用 PRINT 不能像 println 一样换行——只有"PRINT 实参间空格分隔"被提到，对"自带一个 \\n"还是"无 \\n"没明说。实测 "PRINT('a'); PRINT('b')" 实际输出 `a\nb\n`——`\n` 是单独 print 自身的分隔，而不是换行处理。这点算 OK，但用户写 "PRINT('a')" 期望后跟换行的就反直觉。
- skill 的 anti-pattern #15 "AWAIT of cancelled task left unhandled → E0102 at top level" 给出的解法是 `IS_ERR / UNWRAP_OR / ERR_PAYLOAD the AWAIT result`，**但实测**因为 cancel 是 ad-hoc + AWAIT 才返回 `ERR(kind="Cancelled")`，这条 anti-pattern 在 sync run 模型下大部分时候并不可重现。这是 §17.7 限制里写明的"在飞兄弟取消可能不可观测"的复现，但 skill 没写明。

### 8.3 skill 给出的 anti-pattern #3 写在插值 ERR
> "Interpolating ERR(x) — the whole literal becomes ERR (§1.8)" —建议先 `ERR_PAYLOAD` 提取

实测 `08_errors_v2.wll` 的 `PRINT("payload = ${ERR_PAYLOAD(out)}")` 工作（payload = oops），但 `PRINT("passthru IS_ERR = ${IS_ERR(out)}")` 由于 IS_ERR(out) 是消费者正常工作，所以这个 anti-pattern 可被妥善化解；但**只要写成 `PRINT("... ${out}")`**（其中 out 是 ERR），整个字符串字面量就会因为插值 ERR 而透传出诊断。这是 v0.8 spec §1.8 的字面行为，没有问题。但 skill 的提示只在 anti-pattern #3 草草带过。

### 8.4 反正面结论（必要的对照）
- 对非并发、纯算法代码：用 skill 教法在 v0.8 上大多数时候能跑——`MAP`/`FILTER`/`REDUCE`、解构、模式匹配、闭包 + `LET MUT` 全部如预期。`
- 但 skill **绝不会教用户**以下几件事：v0.8 字面量下标是合法表达式、`SUB` 越界 bug、MUT 不能用作 binding name、内建名不能 bind、整数/除零/PANIC 不能被 consumer 接住。

---

## 9. 容易撞到的"代码差异"总览

下表是实测发现的最容易在"按 spec 写 → runtime 翻车"的几个点，供后续修复优先级排序：

| 类别 | 现象 | spec 期望 | 实际 | 严重度 |
|------|------|----------|------|--------|
| 词法 | `1.5e2` 等指数浮点 | §1.7 接受 | E0011 | 高 |
| 词法 | `MUT` 作绑定名 | §1.4 context OK | E0010 | 高 |
| 词法 | `${...(... "..."...)}` 内嵌字面量字符串 | 没明示禁止 | E0001 | 中 |
| 算术 | `+(MAX,1)`、`/(x,0)`、NEG(MIN) | §2.2 ERR 形式 | 原生诊断 | 高 |
| 类型 | 非 OK/ERR 给 UNWRAP_OR | §8.3 E0030 | E0030 ✓ 但很严苛 | 低 |
| 类型 | 强制 strict_types 默认开 | §5.2 默认关 | 默认开（type 检查生效） | 高 |
| 类型 | 类型注解 `: INTEGER` 调用 string | §5.2 忽略 | E0030 | 中 |
| 一等值 | `LET(f, PRINT)` 等 | §5.4 一等值 | E0020 | 中 |
| 闭包 | `examples/closure_cell.wll` 注释期望 1/2/3 | README.md 示例 | 输出 NULL | 高 |
| 容器 | SUB 字符串下标越长越诡异 | §10.5 按码点 | start ≥ 7 返回 "" | 高 |
| 容器 | FORMAT `{0}` 不显式 splat | 多数语言期望 variadic | 把列表当一个 arg | 中 |
| 模块 | `wlwl:std.ai.TASK(...)` 全限定 | §10.11 推荐 | 不支持 | 中 |
| 一致 | spec `==`/MATCH 与 ERR 行为交叉 | §2.4 + §8.3 自洽 | 实测两种都触达 | 高 |
| 一致 | `RESULT` equality & §8.3 consumer list | 不能互相矛盾 | 矛盾 | 高 |
| 显示 | 空 DICT 显示 | 应区别 ARRAY | `[]` | 低 |
| 错误 | `PANIC` / UNWRAP / 过载：可恢复 | §8.4 仅 E0100 | 是 | 低（spec 一致） |
| 错误 | `EXPECT_ERR(/(1,0))` 救回 | §8.3 consumer | 否，原生诊断退出 | 高 |

---

## 10. 实施层面的建议（按优先级降序，不属于本报告结论）

> 仅作为第三方测评的"哪里最值得修"提示。

1. **修复浮点指数字面量** —— spec 明确承诺 `1.5e2` 应被识别，是一致性问题。
2. **修复 SUB 字符串下标的 ~7+ offset 返回值** —— 现行实现低于 spec 的 §10.5 字面行为。
3. **README 的 `closure_cell.wll` 例子应改写为 `LET MUT`** —— 与 §3.3 对齐；移除 "counter shared via closure" 的诱人注释——它和当前的 v0.8 严格语义不符。
4. **整数溢出 / 除零 / NEG(MIN) 的 §8.2 ERR 表达** —— spec v0.6 起就承诺，结果一路未实现，至少在 spec §11.2 + §2.2 中显式注明"诊断而非 ERR"。
5. **移除默认 strict_types 类型检查** —— spec §5.2 显式说"运行时忽略"，且 `E0030` 不是 §11.2 中 strict_types 违例的 `E0033` 码。
6. **统一 §2.4 RESULT 相等与 §8.3 ERR 透传**：在二者之一上明示另一者之优先级。
7. **空 DICT 显示为 `[=:]` 或类似以区别 ARRAY**——给规范补一行。
8. **wlwl-skill 应同步到 v0.8**：SKILL.md frontmatter 改 v0.8，加入 v0.8 字面量下标 anti-pattern、`%` 与 FLOAT、`EXPECT_ERR` 进 §8.3、整数溢出不能被消费者接住等。
9. **MUT 作 binding name 应允许**（按 §1.4 字面要求）。
10. **`PANIC` / `UNWRAP` 应可在 std.test 框架中被"软失败"捕获**——目前测试框架一遇到 PANIC 整个 RUN_TESTS 中止。

---

## 11. 实现整体一致性打分（仅作者自查用）

| 维度 | 期望 | 观察 | 评 |
|------|------|------|----|
| §1 词法 | 完整 EBNF 字面遵守 | 浮点指数 / MUT 关键字错 | 7/10 |
| §2 类型与值 | NaN 在 falsy 表 | NaN 不可达 / OK | 7/10 |
| §3 绑定 / 单元 | 永久不可变标志 | captured-upgrade 在某路径残留；与规范同 known deviation | 7/10 |
| §4 表达式 | 一切皆调 | 内建名不可 bind；call-non-fn parse 而不是 E0020 | 6/10 |
| §5 函数 | 一等值 + 默认参数 + 类型注解 | 一等值残缺；类型注解默认开 | 7/10 |
| §6 控制流 | 完整可用 | 通过 | 9/10 |
| §7 模式 | 完全自洽 | OK；但 MATCH 非 §8.3 consumer，ERR 实参位置的 MATCH 等无法透传 | 7/10 |
| §8 错误模型 | 全部 ERR 可处理 | 数字 overflow / div-by-zero 不是 ERR 而是原生诊断 | 5/10 |
| §9 模块 | IMPORT 全可用 | 相对路径 OK；`wlwl:std.ai.TASK` 全限定不支持 | 8/10 |
| §10 标准库 | 全套实现 | SUB bug；FORMAT ergonomics 待优化；其余通过 | 7/10 |
| §11 诊断 | 全部 E 码齐 | E0033 / E0042-44 / E0050-51 等保留码未触发但定义存在；E0034 不可触达 | 7/10 |
| §12 保留形式 | v0.8 已重写不留清单 | 通过；但 `CALL` 仍然在 §10.3 表里 | 8/10 |
| §17 并发 | 结构化 + 单线程 | 通过；与 §3.3 残留问题相同 | 8/10 |
| 文档/示例一致 | README 与实现一致 | `closure_cell.wll` 与 README 完全相反 | 4/10 |
| **总体** | | 中等偏上：基本能力完整、生态自洽；但细节坑多、文档/实现/示例在多处打架 | **7/10** |

---

> 本报告由独立第三方编写，仅依据 `docs/standard/wlwl-spec-v0.8.md` 与 `wlwl.exe 0.8.0` 在 Windows 11 x64 的实测。所有引用的源码可在 `/test_audit/*.wll` 中复现。
