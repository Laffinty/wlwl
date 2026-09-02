# WLWL 语言规范 v0.3

> **状态**:正式修订版
> **基于**:v0.2(2026-09-02)+ 第三轮审计意见(模块完整性、特性占位、形式化语义三方面)
> **核心约束**:保留 WLWL 原有审美风格(一切皆统一函数调用、显式分号、全大写关键字、宏函数式控制流)。仅修订设计标准,不涉及具体工程实现细节。
> **变更原则**:任何与本规范不一致的代码都不应当被接受,直到规范相应章节被更新。

---

## 0. 文档导读

### 章节表

| 章节 | 内容 | v0.3 状态 |
|------|------|----------|
| §1 | 设计目标与原则 | 不变 |
| §2 | 类型系统 | **新增 §2.6 渐进类型立场说明** |
| §3 | 词法 | 不变 |
| §4 | 字面量 | 不变 |
| §5 | 表达式 | 不变 |
| §6 | 变量与作用域 | 不变 |
| §7 | 控制流 | 不变 |
| §8 | 函数 | 不变 |
| §9 | 运算符 | 不变 |
| §10 | 数据结构 | 不变 |
| §11 | 面向对象 | 不变 |
| §12 | 错误处理 | 不变(§12.6 ERR 透明传播仍是核心) |
| §13 | 模块系统 | **整体重写**:AS 语法定型、跨目录、命名空间、`wlwl.toml` |
| §14 | 错误信息格式 | **整体重写+增强**:`retryable` / `errorCategory`、JSONL、错误码 23→33 |
| §15 | 标准库 | **`std.ai` 从占位变完整签名** |
| §16 | 工具设计原则 | 不变 |
| §18 | 未决问题 | **更新**(v0.3 解决了 7 项,新增 4 项) |
| §19 | 形式化语义(附录) | **新增**:附录式 small-step 操作语义,覆盖核心子集 |
| 附录 A/B/C/D | 示例与变更日志 | D 重写 |

### v0.2 → v0.3 变更摘要(高层)

| 类别 | 变更 |
|------|------|
| **模块系统** | AS 重命名定型为导入时重命名 `IMPORT("mod", ["name": "alias"])`;新增相对路径 `./` / `../`;新增命名空间路径 `wlwl:std`;新增 `wlwl.toml` 包清单 + `wlwl.lock` 锁定 |
| **错误信息** | 新增 `retryable: BOOLEAN` 字段;新增 `errorCategory` 字段(13 类);新增 JSONL 流式输出模式;新增 `error_schema_version` 字段(版本化) |
| **错误码** | 从 23 个扩展到 33 个:新增 IO 类 4 个、JSON 类 2 个、std.ai 类 4 个 |
| **std.ai** | 从占位变完整:`ASK` / `EMBED` / `COMPLETE` 同步签名,带超时、模型选择、错误码 |
| **类型系统** | 新增 §2.6 明确"为什么不引入监控(monitoring)"——基于 TypeScript 工业实践的 Transient 立场 |
| **形式化语义** | 新增 §19 附录:small-step 操作语义,CEK 风格,覆盖核心子集(表达式求值、ERR 透明传播、词法作用域、模块解析) |
| **未决问题** | v0.3 解决了 7 项(AS、跨目录、std.ai 签名、错误码覆盖、JSONL 等);新增 4 项(渐进类型监控、std.ai 流式、模块版本约束算法、形式化覆盖扩展) |

---

## 1. 设计目标与原则

### 1.1 一句话定位

**WLWL 是一门"一切皆统一函数调用"的纯动态类型通用编程语言,以 Rust 级别的结构化错误信息为核心,优先面向 AI Coding 场景。**

### 1.2 设计原则(按优先级排序)

1. **统一性(Unification)**:语言里的所有"动作"——变量赋值、运算、控制流、类定义、HTML 标签、SQL 语句——都是**对函数的调用**。这是最高原则。
2. **表达性(Expression-oriented)**:一切都是表达式,任何表达式都有值,包括 `IF`、`WHILE`、`LET`、`TRY`。
3. **显式优于隐式(Explicit over Implicit)**:必须用 `;` 结束语句;不依赖缩进;不依赖上下文推断;不引入隐式转换;不引入隐式覆盖。
4. **错误前置(Error-first)**:错误必须尽早暴露、必须位置精确、必须给出可操作的修复建议;ERR 默认透明传播,必须显式消费。这是面向 AI Coding 的硬要求。
5. **函数式核心 + OOP 上层(FP core, OOP surface)**:函数是一等公民;类、继承、封装作为组织大型代码的上层建筑。
6. **可读性优于简洁(Readability over Brevity)**:`+(a, b, c)` 比 `a+b+c` 啰嗦,但统一;真要简洁,语法糖永远可以后加。
7. **语法可预测性(Predictability)**:同一语义**仅**保留一种标准写法,不提供多种等价语法糖。语法糖仅作为**实现层**的可选扩展,**不得**进入核心规范。

### 1.3 非目标(Non-goals)

- WLWL **不强制**静态类型检查(注:可**可选**地声明类型注解供文档/AI 工具消费,见 §2.4、§2.6)。
- WLWL **不追求**极致运行性能(本规范不规定实现模型)。
- WLWL **不追求**成为系统编程语言(不需要替代 C/Rust 做底层)。
- WLWL **不追求**最短代码行数。
- **Web 子集技术边界**:Web 子集是**语义子集**,**不**承诺 Web 端运行时与原生运行时性能一致;**不**包含文件 IO、原生扩展、系统调用等能力。多目标编译时,目标间共享核心语义,平台相关能力显式标注。
- **形式化完备性**:本规范**不**追求形式化完备证明(见 §19);§19 附录是"无歧义参考",不是"数学证明"。

### 1.4 长期愿景(可分阶段实现)

- 同一份 WLWL 源码编译到 HTML/CSS/JS、SQL、华为/思科交换机配置、Nginx/Apache 配置(来自原始草稿)。
- **语言级 AI 原语**:在标准库提供 `ASK(model, prompt)`、`EMBED(text)`、`COMPLETE(code_context)` 等内置函数(§15.11)。
- 上述目标**不进入 v0.3 规范**的强制性条款,仅作为方向保留。

---

## 2. 类型系统

### 2.1 动态优先,渐进可选(Dynamic-first, Optionally Gradual)

v0.2 起的立场,延续到 v0.3:

- **默认行为**:纯动态类型。运行时不做类型检查(除显式的 `TYPE(x)` 查询)。
- **可选能力**:v0.2 起预留类型注解语法槽 `name: Type`(见 §2.4),**仅供文档、IDE、静态分析工具、AI 工具消费**。v0.3 运行时**完全忽略**类型注解。
- **未来兼容性**:此预留保证未来如需引入渐进类型检查时,无需破坏性变更现有代码。

### 2.2 内建类型(共 8 种)

| 类型 | 字面量示例 | 备注 |
|------|------------|------|
| `INTEGER` | `1`, `-42`, `0` | 64 位有符号整数 |
| `FLOAT` | `1.5`, `-3.14`, `0.0` | 64 位 IEEE 754 双精度 |
| `STRING` | `"hello"`, `""` | UTF-8 不可变字符串 |
| `BOOLEAN` | `TRUE`, `FALSE` | 关键字,大小写敏感 |
| `NULL` | `NULL` | 表示"无值",关键字 |
| `ARRAY` | `[1, 2, 3]` | 有序、可变、可嵌套 |
| `DICT` | `["k": 1, "k2": "v"]` | 键值映射 |
| `FUNCTION` | `FUN(...)` | 一等公民 |

### 2.3 DICT 键的相等性规则

- DICT 键使用**严格相等**比较。
- **不**做隐式类型转换:整数键 `1` 与字符串键 `"1"` 是不同的键。
- 仅以下类型可作为 DICT 键:`STRING`、`INTEGER`、以及实现了相等语义的不可变类型(初版仅前两种)。
- 违反(如以 ARRAY/DICT/FUNCTION 作键)→ `E0031`。

### 2.4 类型注解(预留,v0.3 不做检查)

- 函数参数、形参、变量绑定**可以**携带类型注解:`name: Type`。
- `Type` 可以是内建类型名(`INTEGER`、`STRING` 等)、自定义类名,或数组/字典的容器类型(具体语法待定)。
- **v0.3 运行时完全忽略注解**;不存在的类型注解不报错。
- 工具/AI 可以读取注解生成文档、提示、或(未来)进行静态检查。
- 任何具体类型注解的语义、形式化定义、模块契约集成,均在 v0.4 及之后讨论。

### 2.5 类型查询

通过 `TYPE(x)` 表达式获取值的类型名称(返回 STRING):

```wlwl
LET(t, TYPE(1));        // "INTEGER"
LET(t, TYPE("hi"));     // "STRING"
LET(t, TYPE([1, 2]));   // "ARRAY"
```

### 2.6 渐进类型的立场说明[v0.3 新增]

> **本节是设计决策的"自留地",向读者和未来贡献者解释 WLWL 为什么**目前**不做渐进类型的运行时检查。**

WLWL 的渐进类型立场是 **Transient**(瞬态),不引入 **Natural**(自然)监控(monitoring)。

#### 两种策略对比

- **Natural + monitoring**:在动态边界插入类型检查,确保类型注解被严格执行。形式化完备(graduality 定理可证),但运行时开销大,且会强制改变已工作代码的行为(常见类型注解错误会让本来跑得好好的程序崩溃)。
- **Transient**:不强制类型检查,类型注解仅作 IDE / AI 提示。运行时开销为零,不破坏现有程序行为,但失去形式化保证。

#### WLWL 选择 Transient 的理由

基于 **2025-2026 学术共识**与**工业实践**:

1. **TypeScript 经验**(Rastislav Bodik 等,2025):工业界**所有**大规模采用渐进类型的语言(TypeScript、Python annotations、PHP type hints)**全部**选择 Transient,放弃 soundness。原因是 Natural 会在大型代码库中产生"annotation debt",实际效果远低于理论。
2. **PLDI 2024 论文**(Igarashi 等,"Space-Efficient Polymorphic Gradual Typing, Mostly Parametric"):即使是放宽 parametricity 的 Natural,空间效率仍难保证。
3. **POPL 2025 论文**(Giovannini 等,Guarded Domain Theory):形式化 Natural 渐进类型需要重型数学(Guarded Domain Theory / step-indexed logical relations),远超 v0.3 阶段可承受的复杂度。
4. **AI Coding 场景**:TypeScript 在 GitHub 2025 Octoverse 超越 Python 成为最流行语言,**正是**因为它对 AI 友好——类型注解作为 AI 上下文,不强制执行。WLWL 的"面向 AI"定位与 Transient 一致。

#### 未来路径

- v0.4 可能**可选**引入 monitoring,**仅在 `strict_types: true` 模式下启用**(见 §13.8 `wlwl.toml` 字段预留)。
- 引入时应在 §2.6.1 进一步明确 monitoring 的具体行为(基于 transient cast insertion 思路,与 TypeScript 的 `--strict` 模式一致)。

#### 不引入 monitoring 的"坏处"明确化

- 类型注解**可能与运行时实际类型不一致**——`name: INTEGER` 不保证 `name` 一定是 INTEGER。
- AI 工具若依赖类型注解生成代码,**必须**同时插入运行时类型检查(类似 Python 的 `isinstance`)。
- 这跟 Python 3.5+ 的渐进类型行为**完全一致**,有大量先例可参考。

---

## 3. 词法

- 源文件为 UTF-8。
- 标识符允许字母、数字、下划线,首字符不能是数字。**允许中文标识符**,但建议在生产代码中使用 ASCII 标识符以提升 AI 编码效率。
- 区分大小写。

### 3.2 关键字(共 16 个)

```
TRUE  FALSE  NULL
LET   FUN    RETURN
IF    WHILE  FOR
BREAK CONTINUE
CLASS NEW    THIS
```

### 3.3 分隔符

| 符号 | 用途 |
|------|------|
| `(` `)` | 函数调用 / 表达式分组 |
| `,` | 参数分隔 |
| `;` | **语句结束符,必须** |
| `[` `]` | 数组、字典、下标访问 |
| `:` | 字典键值对分隔;**类型注解分隔(预留,见 §2.4)**;**导入重命名分隔(§13.4)** |
| `.` | 成员访问(对象属性 / 方法 / 链式调用) |
| `"` | 字符串字面量边界 |
| `//` | 单行注释起始 |
| `/*` `*/` | 块注释边界,**可嵌套** |

### 3.4 注释

```wlwl
// 这是单行注释
LET(x, 1);

/*
   这是块注释
   /* 可以嵌套 */
   结束 */
LET(y, 2);
```

> AI 友好约束:注释中以 `TODO(agent):` 开头的行,在严格模式下输出结构化警告(见 §14.5)。

### 3.5 空白

- 缩进不影响语义。
- 多个连续空白等同于一个空白。
- 换行不影响语义;语句必须用 `;` 显式结束。

---

## 4. 字面量

### 4.1 数值

```wlwl
LET(i, 42);
LET(f, 3.14);
LET(n, -1);
```

### 4.2 字符串

使用双引号。支持转义序列:`\n` `\t` `\r` `\\` `\"` `\0`。**v0.2 不支持字符串模板**;需要拼接请用 `+(...)`。

```wlwl
LET(s, "事屑");
LET(greet, +("hello, ", "world"));
```

### 4.3 布尔与空

```wlwl
LET(b, TRUE);
LET(n, NULL);
```

### 4.4 数组

用 `[` `]` 包裹,逗号分隔:

```wlwl
LET(i, [1, 1, 4, 5, 1, 4]);
LET(empty, []);
LET(mixed, [1, "two", TRUE, NULL]);
```

### 4.5 字典

用 `[` `]` 包裹,键值对用 `:` 分隔,各对之间用 `,` 分隔:

```wlwl
LET(j, ["cat": "nya", "dog": 2]);
LET(empty, []);
LET(mixed, [1: "one", "two": 2]);
```

> 注:数组和字典语法相同,根据内容消歧。**不允许**一个数组字面量中混用两种形式(违反 → `W0020`)。

### 4.6 函数字面量

见 §8.2。

---

---

## 5. 表达式

### 5.1 一切都是表达式

WLWL 中所有"语句"在求值后都产生一个值。下列形式都是表达式:

- 字面量(§4)
- 函数调用(§5.2)
- 变量引用(§6)
- 控制流(§7):`IF`、`WHILE`、`FOR`、`TRY`
- 块表达式(§5.3)

### 5.2 函数调用

**调用形式**:`NAME(arg1, arg2, ..., argN)`

```wlwl
LET(x, +(1, 2));          // 3
LET(y, PRINT("hi"));      // PRINT 返回 NULL
```

**方法调用**(对对象的链式访问):

```wlwl
LET(t, HTML(HEAD(TITLE("x")), BODY()));
LET(j, t.DOM.ID("j"));
j.APPEND(IMG("./1.jpg"));
```

### 5.3 块表达式

用 `(` `)` 包裹的语句序列,值为最后一条表达式的值:

```wlwl
LET(x, (
    LET(a, 1);
    LET(b, +(a, 2));
    +(a, b)
));  // x = 4
```

空块 `()` 的值为 `NULL`。

### 5.4 求值顺序

- 函数实参在传入前**完全求值**(无惰性求值)。
- 自左向右。
- 如需短路行为,使用 `AND`/`OR`(§9.3),它们是**宏函数**,提供短路语义。

---

---

## 6. 变量与作用域

### 6.1 变量绑定

**唯一方式**:`LET(name, value)` 表达式。**没有**`var`、`x = 1`、自增自减等语法。

```wlwl
LET(x, 1);
LET(y, +(x, 1));
LET(z, !(FALSE));
```

> `LET` 表达式本身返回 `NULL`;它的"作用"是副作用(命名绑定)。

### 6.2 重新绑定

- 同一作用域内对同一名字再次 `LET`,**默认报错**(`W0012: name 'x' already bound in this scope`)。
- 如需"重赋值"语义,使用 `SET(name, value)` 表达式:

```wlwl
LET(x, 1);
SET(x, +(x, 1));  // x = 2
```

> 初版规范明确:WLWL **没有**单独的"赋值语句",`SET` 是函数。

### 6.3 作用域规则

- **词法作用域(lexical scoping)**。
- 引入新作用域的构造:函数体、块表达式、类体、模块顶层。
- 子作用域可读父作用域的绑定,**不可**写(无 `nonlocal`/`global` 关键字)。
- 读未定义名字 → `W0001: undefined name 'x'(附最近似候选)`。

### 6.4 提升(hoisting)

- WLWL **不做**变量提升。
- 在 `LET` 之前引用一个名字 → `W0001` 错误,带行号。

---

---

## 7. 控制流

### 7.1 条件:`IF`

`IF(cond, then, else)` —— **三元表达式**。`else` 分支可省略,默认值为 `NULL`。

```wlwl
IF(=(x, TRUE),
    (PRINT("x是true")),
    (PRINT("x是false"))
);
```

> `IF` **不是**特殊语法,是宏函数。条件真假规则见 §9.4。

### 7.2 循环:`WHILE`

`WHILE(cond, body)` —— 条件为真时反复执行 body。`WHILE` 表达式的值是**最后依次循环体的值**,若从未执行则为 `NULL`。

```wlwl
LET(x, 2);
WHILE(>(x, 0),
    (PRINT(x), SET(x, -(x, 1)))
);
```

### 7.3 循环:`FOR`(遍历)

`FOR(var, iterable, body)` —— 对 iterable(ARRAY 或 DICT 的键)逐项执行 body,var 在每次迭代中绑定当前项。

```wlwl
FOR(item, [1, 2, 3],
    (PRINT(item))
);

FOR(key, ["a": 1, "b": 2],
    (PRINT(key))
);
```

**DICT 遍历顺序保证[v0.2 明确]**:`FOR` 遍历 DICT 时,键的出现顺序为**插入顺序**(与 Python 3.7+ 语义一致),不依赖实现细节。

`FOR` 表达式的值是 `NULL`。

### 7.4 提前退出

- `RETURN(value)` —— 立即从当前函数返回,值为 `value`(若省略则 `NULL`)。
- `BREAK()` —— 立即终止最近的 `WHILE`/`FOR` 循环。
- `CONTINUE()` —— 立即跳到最近的 `WHILE`/`FOR` 循环的下一次迭代。

```wlwl
FUN(first_positive(nums),
    FOR(n, nums,
        IF(>(n, 0), RETURN(n))
    );
    NULL
);
```

> `RETURN`/`BREAK`/`CONTINUE` 是宏函数,只能出现在 `FUN`/`WHILE`/`FOR` 体内,否则报错(见 §14.4)。

---

> **v0.3 备注**:DICT 遍历插入序已是 v0.2 明确

---

## 8. 函数

### 8.1 函数是一等公民

- 函数可作为参数传递、作为返回值、存放在变量/数组/字典中。
- 函数没有"方法重载",靠参数模式匹配区分(§8.4)。

### 8.2 函数定义

`FUN(name(params), body)`:

```wlwl
FUN(hello(str),
    (PRINT(+(+("hello ", str), "!")))
);
```

- `name` 可省略,产生**匿名函数**:`FUN((x), (*(x, x)))`。
- `params` 是参数列表,每个参数:
  - `name` —— 必填参数
  - `name = default` —— 默认参数
  - `name: Type` —— **类型注解(v0.2 起预留,运行时忽略,见 §2.4)**
  - `*rest` —— 剩余参数(收集为数组)
  - 三者可组合,形如 `name: Type = default`

```wlwl
FUN(greet(name, greeting = "hi"),
    (PRINT(+(+(greeting, ", "), name)))
);

greet("alice");                  // "hi, alice"
greet("alice", "hello");         // "hello, alice"
```

> 类型注解在 v0.2 仅为**元数据**,不参与运行时检查;任何语法上合法的注解都不报错。

### 8.3 函数调用

直接用 `name(args...)`,不区分"方法调用"和"普通调用"——但 `a.b(args)` 是对 `a` 的 `"b"` 方法的语法糖,见 §11.4。

### 8.4 函数应用与元数[v0.2 补全]

- 函数对**元数(arity)**敏感:`FUN((x, y), ...)` 与 `FUN((x), ...)` 是不同函数。
- **元数检查规则**[v0.2 明确]:
  - 剩余参数 `*rest` **不计入**必填参数数量。
  - 带有默认值的参数,若调用方未提供,则使用默认值;若调用方提供了,则按位置/名字覆盖。
  - 调用参数数量必须满足:**`必填参数数量 ≤ 实际参数数量 ≤ 必填参数数量 + 1`(若存在剩余参数)**。
  - 违反 → `E0022: function 'foo' expects N..M args, got K`(带签名、行号、调用点)。

### 8.5 闭包

函数捕获其定义时的词法环境:

```wlwl
FUN(make_counter(),
    LET(count, 0);
    FUN((),
        SET(count, +(count, 1));
        count
    )
);

LET(c, make_counter());
CALL(c);  // 1
CALL(c);  // 2
```

> 闭包变量存放在堆上;具体 GC 策略由实现决定,**不在本规范范围**。

---

---

## 9. 运算符

**核心原则**:一切运算符都是函数,大写命名,中缀仅在语法糖中后加(且**不进入核心规范**,见 §1.2 第 7 条)。

### 9.1 算术

| 函数 | 说明 |
|------|------|
| `+(a, b, ...)` | 至少 2 个参数 |
| `-(a, b)` | 二元 |
| `*(a, b, ...)` | 至少 2 个参数 |
| `/(a, b)` | 二元 |
| `%(a, b)` | 二元 |
| `NEG(a)` | 一元取负 |

#### `+` 的混合类型规则[v0.2 明确]

- 所有参数类型一致时:
  - `INTEGER` / `FLOAT` 做算术加。
  - `STRING` 做字符串拼接。
  - `ARRAY` 做数组合并(返回新数组,非破坏性)。
- **类型不一致时**:**抛出 `E0030` 类型错误**,**不**做隐式转换。
- 如需将元素追加到数组,使用 `PUSH` 而非 `+`(语义清晰、避免歧义)。
- 例外:`INTEGER` 与 `FLOAT` 混合时,允许结果提升为 `FLOAT`,不视为类型不一致(算术常规)。

### 9.2 比较

| 函数 | 含义 |
|------|------|
| `=(a, b)` | 等于 |
| `!=(a, b)` | 不等于 |
| `>(a, b)` | 大于 |
| `<(a, b)` | 小于 |
| `>=(a, b)` | 大于等于 |
| `<=(a, b)` | 小于等于 |

> 比较运算符的返回类型**总是** `BOOLEAN`。

### 9.3 逻辑

| 函数 | 短路 | 说明 |
|------|------|------|
| `AND(a, b)` | 是 | 仅当 a 真才求值 b |
| `OR(a, b)` | 是 | 仅当 a 假才求值 b |
| `!(a)` | 否 | 逻辑非,可接受多个参数(全部真才真) |

### 9.4 真值规则[v0.2 明确]

以下值视为**假**:

- `FALSE`
- `NULL`
- `INTEGER` 值 `0`
- `FLOAT` 值 `0.0`
- `STRING` 值 `""`(空字符串)
- `ARRAY` 值 `[]`(空数组)
- `DICT` 值 `[]`(空字典)

**非空**的 `ARRAY` / `DICT` **无论内部元素值是什么,一律视为真**(空数组 `[]` 和非空数组 `[NULL]` 是不同的真值)。

其他值视为**真**。`BOOL(x)` 函数显式化转换规则。

---

> **v0.3 备注**:`+` 混合类型规则已是 v0.2 明确

---

## 10. 数据结构

### 10.1 数组(ARRAY)

| 操作 | 函数 | 说明 |
|------|------|------|
| 取长度 | `LEN(arr)` | 嵌套数组不递归 |
| 追加 | `PUSH(arr, val)` | 原地,返回 arr |
| 弹出 | `POP(arr)` | 移除末尾,返回该值 |
| 头部插入 | `UNSHIFT(arr, val)` | 原地,返回 arr |
| 头部删除 | `SHIFT(arr)` | 移除首位,返回该值 |
| 切片 | `SLICE(arr, start, end = -1)` | 返回新数组,负数从末尾数 |
| 连接 | `CONCAT(a, b, ...)` | 返回新数组,非破坏性 |
| 包含 | `CONTAINS(arr, val)` | 返回 BOOLEAN |
| 索引 | `INDEX(arr, val)` | 返回第一次出现的索引,无则 -1 |
| 反转 | `REVERSE(arr)` | 原地,返回 arr |

### 10.2 字典(DICT)

| 操作 | 函数 | 说明 |
|------|------|------|
| 取长度 | `LEN(d)` | 键的数量 |
| 取键 | `KEYS(d)` | 返回 ARRAY(键列表,按插入顺序) |
| 取值 | `VALUES(d)` | 返回 ARRAY(值列表,按插入顺序) |
| 包含键 | `HAS(d, key)` | 返回 BOOLEAN |
| 删除键 | `DEL(d, key)` | 原地,返回 d |
| 合并 | `MERGE(a, b, ...)` | b 覆盖 a,返回新字典 |
| 遍历顺序 | `FOR(k, d, ...)` | 按**插入顺序**遍历键(见 §7.3) |

### 10.3 字符串(STRING)

| 操作 | 函数 | 说明 |
|------|------|------|
| 长度 | `LEN(s)` | 字符数(按 Unicode 码点) |
| 拼接 | `+(s1, s2, ...)` | 与算术 `+` 共用,过载 |
| 子串 | `SUB(s, start, len = -1)` | 负数从末尾 |
| 包含 | `CONTAINS(s, substr)` | 字符串版本 |
| 分割 | `SPLIT(s, sep)` | 返回 ARRAY |
| 替换 | `REPLACE(s, old, new)` | 全部替换,返回新串 |
| 大小写 | `UPPER(s)`, `LOWER(s)` | ASCII;非 ASCII 行为由实现定义 |
| 整数/字符串转换 | `STR(n)`, `INT(s)` | INT 失败抛 `ERR` |

> 字符串**不可变**;任何"修改"返回新串。

### 10.4 相等性

- `=(a, b)`:对 INTEGER/FLOAT/STRING/BOOLEAN/NULL 按值比较;对 ARRAY/DICT 递归比较;对 FUNCTION 按引用比较(同一闭包实例)。
- DICT 键的相等性遵循 §2.3 严格相等规则。

---

> **v0.3 备注**:DICT 遍历插入序已是 v0.2 明确

---

## 11. 面向对象

### 11.1 设计取向

OOP 是函数式之上的"组织代码"机制,**不是**核心。类的内部表示就是 DICT + 函数。

### 11.2 类定义

`CLASS(name, parent, members)`:

- `name`:STRING,类名(可省略,产生匿名类)
- `parent`:父类(可为 `NULL` 表示无父类)
- `members`:DICT,键为属性/方法名,值为:
  - 默认值(属性)
  - `FUN(...)` 函数字面量(方法)

```wlwl
CLASS("Rectangle", NULL, [
    "width": 0,
    "height": 0,
    "getArea": FUN((self),
        *(GET_PROP(self, "width"), GET_PROP(self, "height"))
    ),
    "scale": FUN((self, factor),
        SET_PROP(self, "width", *(GET_PROP(self, "width"), factor));
        SET_PROP(self, "height", *(GET_PROP(self, "height"), factor))
    )
]);
```

### 11.3 实例化与属性

```wlwl
LET(rect, NEW("Rectangle"));
SET_PROP(rect, "width", 10);
SET_PROP(rect, "height", 20);
LET(area, CALL_METHOD(rect, "getArea"));  // 200
```

### 11.4 方法调用的链式语法糖

`a.b.c(args)` 在词法上等价于:

```
CALL_METHOD(CALL_METHOD(GET_PROP(a, "b"), "c"), ???, args)
```

但 `a.b` 应当是属性访问;只有当末尾带 `(args)` 时才触发方法调用。具体规则:

- `a.b` —— 属性访问,等价 `GET_PROP(a, "b")`
- `a.b(args)` —— 方法调用,等价 `CALL_METHOD(a, "b", args...)`
- `a.b.c(args)` —— 链式:先 `a.b`,再对结果做 `.c(args)`。
- `a.b.c.d(args)` —— 同理递归。

### 11.5 继承

- `CLASS(name, parent, members)` 中 `parent` 可以是父类的类名字符串,或父类对象本身。
- 方法解析顺序(MRO):v0.2 用**单链**:子类 → 父类 → NULL。
- 多继承**不支持**。

### 11.6 封装

- v0.2 **不**做访问控制。所有属性公开。
- 命名约定:以下划线 `_` 开头的属性/方法视为"私有",由文档和工具约定,语言层不强制。

### 11.7 静态属性/方法

v0.2 不做。`CLASS` 体里只能是属性默认值和方法。需要在类层面共享数据,使用一个外部 DICT。

---

---

## 12. 错误处理

### 12.1 范式

参考 Rust 的 `Result<T, E>`,但在动态类型下表示为两个构造器函数:

- `OK(value)` —— 成功,值可以是任意类型
- `ERR(error)` —— 失败,值必须是 STRING 或 DICT(描述错误的结构化信息)

### 12.2 触发与传播

| 操作 | 函数 | 说明 |
|------|------|------|
| 不可恢复错误 | `PANIC(msg)` | 立即终止执行,msg 必须是 STRING 或 DICT |
| 错误传播 | `TRY(expr)` | 若 expr 求值为 `ERR(...)`,提前从当前函数返回该 ERR;否则取其值 |
| 默认值兜底 | `OR_DIE(expr, default)` | 若 expr 是 `ERR`,返回 default;否则取 expr 的值 |
| 主动判断 | `IS_OK(x)`, `IS_ERR(x)` | BOOLEAN |

```wlwl
FUN(parse_int(s),
    LET(n, INT(s));
    IF(IS_ERR(n), RETURN(ERR(["kind": "ParseError", "input": s])));
    OK(GET_PROP(n, "value"))
);
```

### 12.3 `TRY` 的关键语义

`TRY(expr)` 在语义上等价于:

```
LET(_tmp, expr);
IF(IS_ERR(_tmp), RETURN(_tmp));
UNWRAP(_tmp)  // 取出 OK 里的值
```

因此它**只能**出现在期望"接收一个值"的位置,比如赋值、函数实参、另一层 `TRY` 的内部。

```wlwl
LET(x, TRY(some_risky_op()));
LET(y, TRY(other_op()));
```

### 12.4 `PANIC` vs `ERR`

- `ERR`:**可恢复**,沿调用栈向上传播,通过 `TRY` 或调用方处理。
- `PANIC`:**不可恢复**,打印结构化错误后立即终止(默认 exit code 1)。用于真正不可处理的内部错误。

### 12.5 错误信息结构

所有 `ERR` / `PANIC` 的 `error` 字段必须是 STRING 或 DICT。DICT 形式推荐字段见 §14.2。

### 12.6 ERR 透明传播规则[v0.2 新增,核心语义]

> 这是 v0.2 对错误处理的关键语义补全。**缺失此规则会导致错误被意外吞掉,是最严重的语义漏洞之一。**

**规则**:`ERR` 值在所有普通函数调用中**透明传播**。

- 任何**普通函数**(白名单外的函数)接收到 `ERR` 类型的实参时,**必须**直接返回该 `ERR`,**不**执行函数体,亦不报"参数类型错误"。
- 传播过程**不消耗** `ERR`;原 `ERR` 值原封不动地向上冒泡。
- 仅以下 4 个函数**可以消费** `ERR` 值:
  - `IS_OK(x)` / `IS_ERR(x)` —— 显式判断
  - `OR_DIE(expr, default)` —— 提供默认值
  - `TRY(expr)` —— 显式传播
- 其他函数(算术、字符串、数据结构、I/O 等)对 `ERR` 输入一律透明传播。

**示例**:

```wlwl
LET(r, +(1, ERR("oops")));     // r = ERR("oops"),+ 本身不被调用
LET(r, LEN(ERR("oops")));      // r = ERR("oops")
LET(r, IF(TRUE, ERR("e"), 2)); // r = ERR("e")
LET(r, OK(+(1, 2)));           // r = OK(3)
LET(r, IS_OK(OK(1)));          // r = TRUE
LET(r, OR_DIE(ERR("e"), 0));   // r = 0
LET(r, TRY(ERR("e")));         // r = ERR("e")(传播到当前函数)
```

**为何如此设计**:
- 默认传播、显式处理,杜绝错误被静默吞掉。
- 调用方不显式 `TRY`/`OR_DIE`,错误就一直往上冒,直到最外层被 `PANIC` 兜底。
- 强制 AI 生成代码时考虑错误路径(否则测试会立即暴露)。

**与 `PANIC` 的区别**:`ERR` 透明传播是**软失败**;`PANIC` 是**硬终止**。`PANIC` 不参与透明传播规则。

---

> **v0.3 备注**:**§19.6 给出 ERR 透明传播的形式化定义**

---

## 13. 模块系统[v0.3 整体重写]

> **本章节是 v0.3 的核心修订之一**。v0.2 阶段只有"单目录、显式 EXPORT/IMPORT、AS 语法未决、跨目录空白、包管理空白"。v0.3 全部解决。

### 13.1 文件即模块

- 每个 `.wl` 源文件是一个模块。
- **模块名解析规则**(优先级从高到低):
  1. 文件内的 `MODULE("explicit_name", ...)` 声明(若存在)
  2. `wlwl.toml` 中 `package.name`(若存在)
  3. 文件名(不含扩展名)
- 跨目录引用见 §13.5、§13.6。

### 13.2 显式 `EXPORT` 名单

模块必须通过 `EXPORT(names)` 显式声明哪些名字对外可见,否则一律私有。

```wlwl
// file: math.wl
FUN(add(a, b), +(a, b));
FUN(sub(a, b), -(a, b));
LET(PI, 3.14159);

EXPORT(["add", "PI"]);
```

- `EXPORT` 必须出现在文件**顶层**。
- `EXPORT` 可多次出现,实现应取并集(便于条件导出)。
- 引用一个**未导出**的名字 → `E0023: 'sub' is not exported by module 'math'`。

### 13.3 显式 `IMPORT`(重写)

`IMPORT(path, names, opts = [])` 的三参数形式:

```wlwl
IMPORT(path, names)              // 标准形式
IMPORT(path, names, opts)        // 带选项(预留)
```

其中:
- `path`:STRING,**模块路径**(见 §13.5 路径形式)
- `names`:ARRAY,可以有两种形式:
  - 简单形式:`["add", "PI"]` —— 直接引入同名绑定
  - 重命名形式:`["add": "math_add", "PI": "MATH_PI"]` —— 引入并重命名(见 §13.4)
- `opts`:DICT(可选,目前预留,无字段)

#### 重复导入规则

> 沿用 v0.2 修订 7,**重复导入直接报错**,不允许隐式覆盖。

- 同一作用域内,**重复导入同名符号** → `E0021: name 'x' already bound by previous IMPORT`。
- **不**允许隐式覆盖(无论两次 `IMPORT` 的是同一模块还是不同模块)。
- 如需不同模块的同名符号,使用**导入时重命名**(§13.4)。

### 13.4 导入时重命名(取代 v0.2 的 `AS` 函数)[v0.3 新增定型]

```wlwl
IMPORT("math", ["add": "math_add", "PI": "MATH_PI"]);
// 现在:add 已重命名为 math_add,PI 已重命名为 MATH_PI
// 原名 "add"、"PI" 在当前作用域中**不**被绑定
```

#### 形式

`IMPORT(path, ["original_name": "alias_name", ...])`

- `alias_name` 可以与 `original_name` 相同(无意义的别名,但语法合法)。
- 多个名字可以一次性重命名,使用 ARRAY 语法。

#### 为什么不单独保留 `AS` 函数?

基于 2025-2026 工业实践调研:
- **Go / Rust / TypeScript** 等主流语言**全部**采用"导入时重命名"或"alias 关键字",**没有**单独 `AS` 函数。
- 单独 `AS` 函数会带来"已绑定名字的二次重命名"语义,**难以静态分析**(`AI 友好`目标要求导入行为可静态预测)。
- 导入时重命名**与 IMPORT 紧耦合**,AI 工具解析 IMPORT 即可拿到完整绑定,无需二次扫描。

#### 例外情况:v0.3 仍保留单参数 `AS` 函数(可选)

为**对称性**与**运行时动态重命名**场景保留:

```wlwl
// 单参数 AS(已绑定名字的本地别名)—— 标记为不推荐
LET(x, 1);
AS("y", "x");  // 绑定 y = x
```

**v0.3 强烈推荐**使用 §13.4 的导入时重命名,而非单参数 `AS`。`AS` 函数在 v0.4 考虑是否移除。

### 13.5 跨目录引用[v0.3 新增]

支持三种路径形式(优先级从高到低):

| 形式 | 示例 | 适用场景 |
|------|------|----------|
| 相对路径 | `IMPORT("./other/foo", ...)`、`IMPORT("../sibling/bar", ...)` | 库内引用、monorepo 内部包 |
| 命名空间路径 | `IMPORT("wlwl:std.io", ...)`、`IMPORT("myteam:utils", ...)` | 标准库、已注册包(基于 `wlwl.toml` 索引) |
| 简单名(同目录) | `IMPORT("math", ...)` | 单文件项目(与 v0.2 兼容) |

#### 路径解析规则

1. **以 `./` 或 `../` 开头**:相对路径,以当前模块所在目录为基准。
2. **以 `xxx:` 开头**(其中 `xxx` 是命名空间前缀):命名空间路径,根据 `wlwl.toml` 的 `[dependencies]` 解析;标准库的命名空间固定为 `wlwl:`。
3. **其他**:先尝试同目录,失败则尝试 `wlwl.toml` 中所有依赖包的根。

#### 跨目录的边界规则

- 跨目录导入时,`IMPORT` 链式递归解析,但**禁止目录外的搜索**(类似 Rust 的 `mod.rs` 边界)。
- 项目根目录(包含 `wlwl.toml` 的目录)是搜索的最高边界。
- 试图越界 → `E0040: module 'foo' not found outside project root`。

### 13.6 命名空间路径(标准库与第三方包)[v0.3 新增]

#### 标准库命名空间

所有标准库模块位于 `wlwl:` 命名空间下:

```wlwl
IMPORT("wlwl:std.io", ["PRINT"]);
IMPORT("wlwl:std.json", ["PARSE", "STRINGIFY"]);
IMPORT("wlwl:std.fs", ["READ_FILE"]);
```

#### 第三方包命名空间

第三方包在 `wlwl.toml` 中通过 `[dependencies]` 注册,使用 `<namespace>:<name>` 形式引用:

```toml
# wlwl.toml
[package]
name = "myapp"
version = "0.1.0"

[dependencies]
"myteam:utils" = { path = "../utils" }
"huggingface:client" = "^0.5.0"
```

```wlwl
IMPORT("myteam:utils", ["format_date"]);
IMPORT("huggingface:client", ["Client"]);
```

#### 命名空间冲突规则

- 同一项目内**禁止**注册同名命名空间(可在 `wlwl.toml` 中通过 `[namespaces]` 显式声明)。
- 命名空间命名规则:小写字母、数字、连字符(`-`),首字符必须为字母。
- 冲突 → `E0043: namespace 'myteam' is already registered with different path`。

### 13.7 循环导入(从 v0.2 §13.6 提升)

- 检测到循环导入(`A` 导入 `B`,`B` 导入 `A`,形成环)时,抛出 `E0041: circular import detected: A -> B -> A`,**终止加载**。
- v0.3 **不**支持部分加载或延迟加载解决循环依赖。
- 设计动机:循环导入在 AI 生成代码中常见,显式失败比"运行时找不到名字"更易诊断。
- **v0.3 增强**:错误信息列出完整环路路径(而非仅首尾),便于 AI 工具直接定位。

### 13.8 `wlwl.toml` 包清单[v0.3 新增]

#### 基本格式

```toml
[package]
name = "myapp"                    # 必填,小写+连字符
version = "0.1.0"                 # 必填,SemVer
entry = "src/main.wl"             # 必填,入口文件相对路径
description = "A WLWL app"        # 可选
license = "MIT"                   # 可选

[dependencies]
# 路径依赖(本地开发)
"myteam:utils" = { path = "../utils" }

# 版本依赖(中央仓库,v0.4 启用)
"huggingface:client" = "^0.5.0"
"json:parser" = "1.2.3"

# 详细约束
"strict:math" = { version = ">=1.0.0, <2.0.0", optional = true }

[namespaces]
# 显式命名空间映射(可选,通常自动推断)
"myteam" = "./vendor/myteam"

[features]
strict_types = false              # 见 §2.6 渐进类型开关
default_encoding = "utf-8"        # 标准库相关
```

#### 字段说明

- **`[package]`** —— 必填段,描述本包。
- **`[dependencies]`** —— 依赖列表。键是 `<namespace>:<name>`,值是约束表达式。
- **`[namespaces]`** —— 显式命名空间到本地路径的映射(可选,默认按 `[dependencies]` 自动推断)。
- **`[features]`** —— 特性开关(预留,目前仅 `strict_types` 见 §2.6)。

#### `wlwl.lock`

`wlwl.lock` 是**自动生成**的依赖锁定文件,记录每个依赖的实际版本/路径/哈希,确保可重现构建。

- 提交到版本控制(类似 `package-lock.json` / `Cargo.lock`)。
- 禁止手动编辑。
- `wlwl build` / `wlwl run` 优先读 lock,缺失时按 toml 解析并生成 lock。

### 13.9 入口文件

```bash
wlwl run main.wl                  # 直接指定入口(无 wlwl.toml)
wlwl run                          # 使用 wlwl.toml 中的 entry 字段
```

### 13.10 模块接口契约(从 v0.2 §13.7 提升)

v0.3 进一步明确契约语法的具体形式:

```wlwl
EXPORT([
    "add": ["params": ["INTEGER", "INTEGER"], "returns": "INTEGER"]
]);
```

#### 字段

- `params`:ARRAY of STRING,每个元素是参数类型名(类型注解形式,见 §2.4)
- `returns`:STRING,返回类型名
- `errors`:ARRAY of STRING(可选),可能抛出的错误码列表(预留)

#### 契约执行时点

- **v0.3 不执行契约**(与 §2.4 类型注解一致,Transient 立场)
- 工具/AI 读取契约,生成文档、提示、测试用例
- 未来 v0.4 引入 `strict_types: true` 时,**可能**在 IMPORT 边界做运行时检查

---

## 14. 错误信息格式(AI 友好核心)[v0.3 整体重写+增强]

> **v0.3 在 v0.2 基础上增加 4 项关键增强**:
> 1. `errorCategory` 字段(13 类,便于 AI 工具分类处理)
> 2. `retryable` 字段(基于 2026 错误设计最佳实践)
> 3. JSONL 流式输出模式(基于 2026 行业共识)
> 4. `error_schema_version` 字段(规范演进时的兼容性保证)
> 5. 错误码从 23 个扩展到 33 个

### 14.1 总则

**所有** WLWL 错误,无论来自词法、语法、运行时还是用户主动 `ERR`/`PANIC`,**必须**符合以下结构化格式。实现**不得**只输出不可解析的自由文本。

### 14.2 标准错误对象[v0.3 增强]

```wlwl
ERR([
    "error_schema_version": "1.0.0",                  // [v0.3 新增]schema 版本
    "code": "E1003",                                  // 错误码
    "severity": "error",                              // "error" | "warning" | "note"
    "errorCategory": "runtime",                       // [v0.3 新增]13 类之一
    "retryable": FALSE,                                // [v0.3 新增]TRUE/FALSE
    "message": "division by zero",                    // 一句话人类描述
    "location": ["main.wl", 42, 5, 42, 12],           // [file, line, col_start, line_end, col_end]
    "source_line": "LET(x, /(y, z));",                // 出错行的源码
    "hint": "检查除数 z 是否可能为 0",                // 自然语言修复建议
    "suggestion_code": [                               // 机器可执行的修复(来自 v0.2)
        ["kind": "patch", "description": "...", "patch": [...]]
    ],
    "related": []                                      // 相关位置
]);
```

#### `errorCategory` 字段[v0.3 新增]

13 类之一:

| 值 | 含义 | 典型错误码 |
|----|------|------------|
| `lexical` | 词法错误 | E0001-E0003 |
| `syntax` | 语法错误 | E0010-E0014 |
| `name` | 名字解析错误 | E0020-E0023 |
| `type` | 类型错误 | E0030-E0032 |
| `module` | 模块解析错误 | E0040-E0043 |
| `oop` | OOP 错误 | E0050-E0051 |
| `io` | 文件系统错误 | E0060-E0063(新) |
| `json` | JSON 解析错误 | E0070-E0071(新) |
| `ai` | std.ai 错误 | E0080-E0083(新) |
| `network` | 网络错误 | E0090(预留) |
| `runtime` | 通用运行时错误 | E1003 等 |
| `user` | 用户主动抛错 | E0099 |
| `internal` | 内部错误 | E0100-E0102 |

#### `retryable` 字段[v0.3 新增]

- `TRUE` —— 操作**可能**在下一次重试时成功(如网络超时、文件锁等待)
- `FALSE` —— 重试**不会**改变结果(如参数类型错误、未定义名字)
- AI 工具应根据 `retryable` 决定是否自动重试,而不是盲目重试所有错误

#### `error_schema_version` 字段[v0.3 新增]

- 字符串,SemVer 格式
- 当前固定为 `"1.0.0"`
- v0.4 之后如增加/删除字段,**必须** bump major 版本
- AI 工具应**先检查此字段**再解析,确保兼容性

#### `suggestion_code` 字段(沿用 v0.2)

- ARRAY,每条建议一个 DICT,字段 `kind` / `description` / `patch`(或 `ast_rewrite`)
- AI 工具**优先**用 `suggestion_code` 而非 `hint`

### 14.3 顶层输出格式(CLI)

#### 人类可读(默认)

```
error[E1003]: division by zero
 --> main.wl:42:5
  |
42 | LET(x, /(y, z));
  |          ^^^^^^^
  |
  = hint: 检查除数 z 是否可能为 0
  = fix:  在除法前增加零值守卫(可自动应用)
  = category: runtime
  = retryable: false
  = note: y 在此声明
   --> main.wl:41:5
```

#### JSON(单条,默认 AI 模式)

通过 `--format=json` 输出**单个** JSON 数组(包含该次运行的所有错误),与 v0.2 一致。

#### JSONL(流式,v0.3 新增)

通过 `--format=jsonl` 输出**JSONL** 格式——**每行一条**错误,**无外层数组**。

```jsonl
{"error_schema_version":"1.0.0","code":"E1003","severity":"error","errorCategory":"runtime","retryable":false,"message":"division by zero","location":{"file":"main.wl","line":42,"col":5,"line_end":42,"col_end":12},"source_line":"LET(x, /(y, z));","hint":"...","suggestion_code":[],"related":[]}
{"error_schema_version":"1.0.0","code":"E0022","severity":"error","errorCategory":"name","retryable":false,"message":"function 'foo' expects 2 args, got 1","location":{...},"source_line":"foo(1);","hint":"...","suggestion_code":[],"related":[]}
```

**优势**:
- AI 工具**流式消费**,拿到第一条错误就开始修复,不必等编译器完成
- 不需要解析整个数组,逐行 `json.loads()` 即可
- 适合大型项目的快速迭代

### 14.4 错误码[v0.3 扩展:23 → 33]

#### 词法 / 语法 / 名字 / 类型 / 模块 / OOP(沿用 v0.2 23 个)

| 码 | 类别 | 含义 |
|----|------|------|
| `E0001` | lexical | 非法字符 |
| `E0002` | lexical | 未闭合的字符串 |
| `E0003` | lexical | 未闭合的块注释 |
| `E0010` | syntax | 期望表达式 |
| `E0011` | syntax | 期望 `)` |
| `E0012` | syntax | 期望 `,` |
| `E0013` | syntax | 期望 `;` |
| `E0014` | syntax | `RETURN`/`BREAK`/`CONTINUE` 出现在非法位置 |
| `E0020` | name | 未定义名字 |
| `E0021` | name | 重复定义 / 重复 IMPORT |
| `E0022` | name | 函数元数不匹配 |
| `E0023` | name | 未导出的模块成员 |
| `E0030` | type | 算术运算类型错误(含 `+` 混合类型) |
| `E0031` | type | 下标/键类型错误 |
| `E0032` | type | 属性/方法不存在 |
| `E0040` | module | 找不到模块 |
| `E0041` | module | 循环导入 |
| `E0050` | oop | 类继承链错误 |
| `E0051` | oop | `NEW` 元数与 `INIT` 不匹配 |

#### IO / JSON / std.ai / 网络[v0.3 新增 10 个]

| 码 | 类别 | 含义 | retryable |
|----|------|------|-----------|
| `E0060` | io | 文件不存在 | FALSE |
| `E0061` | io | 文件权限拒绝 | FALSE |
| `E0062` | io | 读取失败 | **TRUE** |
| `E0063` | io | 写入失败(磁盘满、锁) | **TRUE** |
| `E0070` | json | 解析失败 | FALSE |
| `E0071` | json | 类型不匹配(预期 vs 实际) | FALSE |
| `E0080` | ai | LLM 调用超时 | **TRUE** |
| `E0081` | ai | LLM 响应内容超长 | FALSE |
| `E0082` | ai | 凭据缺失(API key 未配置) | FALSE |
| `E0083` | ai | 模型未找到 | FALSE |
| `E0090` | network | 网络不可达(预留) | **TRUE** |

#### 用户 / 内部(沿用 v0.2 5 个)

| 码 | 类别 | 含义 |
|----|------|------|
| `E0099` | user | 用户抛出的 `ERR`/`PANIC` |
| `E0100` | internal | 实现内部错误(`PANIC` 默认值) |
| `E0101` | internal | 栈溢出 |
| `E0102` | internal | 未处理的 `ERR` 逃逸到顶层 |

> 错误码**必须稳定**。任何变更都需在规范更新中说明,新增时分配连续编号,删除时保留为"已弃用"。

### 14.5 警告码(沿用 v0.2)

| 码 | 含义 |
|----|------|
| `W0001` | 名字未定义 |
| `W0010` | 未使用的 `LET` 绑定 |
| `W0011` | 未使用的函数参数(标注 `_` 前缀忽略) |
| `W0012` | 同名重复 `LET` |
| `W0013` | `IF` 的 then/else 分支类型不一致(可能 bug) |
| `W0020` | 数组/字典字面量中混用裸值和键值对 |
| `W0030` | `IMPORT` 了一个名字但未使用 |
| `W0040` | `TODO(agent):` 注释未处理 |

#### 未使用警告的 AI 语义约定(沿用 v0.2)

- AI 工具在严格模式下,**应将**未使用变量/参数视为高概率 bug,优先检查是否存在拼写错误或遗漏逻辑。
- 人类开发者可通过 `_` 前缀显式忽略。

### 14.6 严重程度与退出码

| 严重度 | CLI 退出码 |
|--------|------------|
| error | 1 |
| warning | 0(在 `strict` 模式下变 1) |
| note | 0 |

### 14.7 AI 工具约定

- AI 编码工具**应总是**用 `--format=jsonl` 流式消费(优先)或 `--format=json` 批量消费。
- AI 工具**应优先执行** `suggestion_code` 字段的修复,而非仅依赖 `hint` 自然语言。
- AI 工具**应**根据 `hint` 字段生成自然语言建议,作为 `suggestion_code` 不可用时的兜底。
- AI 工具**应**把 `related` 字段内的所有位置都纳入上下文,而不是只看主错误位置。
- AI 工具**应**尊重 `severity`:`warning` 在常规模式下不应阻塞,但在 `strict` 模式下应阻塞。
- AI 工具**应**根据 `retryable` 决定是否自动重试:**仅当 `retryable: TRUE` 时**才重试,否则直接报告失败。
- AI 工具**应**根据 `errorCategory` 选择修复策略(如 `io` 类先检查路径与权限,`ai` 类先检查凭据与超时)。
- AI 工具**应**先检查 `error_schema_version` 字段,确保与本规范兼容。

### 14.8 语法错误恢复策略(沿用 v0.2)

- 词法/语法错误采用**单错误终止模式**:检测到**第一个**错误后,输出错误信息并终止解析,**不**进行级联错误报告。
- 错误的 `suggestion_code` 字段可提供最多 3 个按置信度排序的修复方案。
- 运行时错误(非词法/语法)**不**受单错误终止约束。

### 14.9 JSONL 流式输出[v0.3 新增]

#### 启用

```bash
wlwl check main.wl --format=jsonl
```

#### 协议

- 每行一条完整的 JSON 对象
- 字段顺序不限
- 末尾**不**加换行外的其他字符
- 错误对象**不**用数组包裹
- 跨错误之间的字段集**完全一致**(schema 稳定性)

#### AI 工具消费模式

```python
import subprocess, json
proc = subprocess.Popen(["wlwl", "check", "main.wl", "--format=jsonl"], stdout=subprocess.PIPE, text=True)
for line in proc.stdout:
    err = json.loads(line)
    if err["retryable"]:
        # 重试
    else:
        # 应用 suggestion_code
        ...
```

#### 与 `--format=json` 的对比

| 维度 | JSON | JSONL |
|------|------|-------|
| 流式 | ❌(整体数组) | ✅(逐行) |
| 解析复杂度 | 一次性 `json.load` | 逐行 `json.loads` |
| 适合场景 | 批量分析 | 实时修复(AI) |
| 工具友好度 | 中 | 高 |

**v0.3 强烈推荐 AI 工具使用 JSONL**。

### 14.10 错误信息 schema 版本化[v0.3 新增]

#### 版本号规则

- 字段:`error_schema_version`,SemVer 格式
- 当前:`"1.0.0"`
- **MAJOR** 变化:增/删字段、字段类型变化
- **MINOR** 变化:新增可选字段
- **PATCH** 变化:错误信息文案微调

#### 兼容性策略

- AI 工具解析错误对象时,**应先检查 MAJOR 版本**,若不兼容则降级到自然语言提示
- v0.4 引入新字段时,MINOR bump,旧工具仍可工作(忽略未知字段)
- 任何破坏性变更必须 MAJOR bump,且**至少**经过 2 个 minor 版本的"过渡期"

---

## 15. 标准库[v0.3 `std.ai` 完整化]

### 15.1-15.10 简表(与 v0.2 一致)

| 模块 | 暴露 | 用途 | v0.3 状态 |
|------|------|------|-----------|
| `wlwl:std.io` | `PRINT`, `INPUT` | 控制台 I/O | 承诺 |
| `wlwl:std.fs` | `READ_FILE`, `WRITE_FILE`, `EXISTS` | 文件系统 | 承诺 |
| `wlwl:std.json` | `PARSE`, `STRINGIFY` | JSON | 承诺 + 错误码 E0070/E0071 |
| `wlwl:std.math` | `ABS`, `MIN`, `MAX`, `POW`, `SQRT` | 数学 | 大纲 |
| `wlwl:std.string` | 字符串函数(§10.3) | 已在全局 | 承诺 |
| `wlwl:std.web` | `HTML`, `HEAD`, `BODY`, ... | 来自原始草稿 | 大纲 |
| `wlwl:std.web.dom` | `DOM`, `ID`, `APPEND` | 来自 1.txt | 大纲 |
| `wlwl:std.web.css` | `STYLE`, `EDIT`, `DEL` | 来自 1.txt | 大纲 |
| `wlwl:std.time` | `NOW`, `SLEEP` | 时间 | 大纲 |
| `wlwl:std.os` | `EXIT`, `ENV`, `ARGS` | 进程与环境 | 大纲 |
| `wlwl:std.ai` | `ASK`, `EMBED`, `COMPLETE` | LLM 调用 | **v0.3 完整签名** |

### 15.11 `std.ai` 完整签名[v0.3 完整化]

#### 模块导入

```wlwl
IMPORT("wlwl:std.ai", ["ASK", "EMBED", "COMPLETE"]);
```

#### 通用约定

- 所有函数**同步调用**(`v0.3 不支持 async/await`)
- 所有函数返回 `OK(value)` 或 `ERR(...)` 形式
- 错误对象遵循 §14.2 schema,`errorCategory = "ai"`
- 超时与凭据通过环境变量配置:
  - `WLWL_AI_DEFAULT_MODEL` —— 默认模型名
  - `WLWL_AI_API_KEY` —— API 凭据
  - `WLWL_AI_ENDPOINT` —— API 端点(可选)

#### 15.11.1 `ASK` —— 调用 LLM

**签名**(注解仅作文档,运行时忽略):

```wlwl
FUN(ASK(model: STRING, prompt: STRING, opts: DICT = []),
    // 返回 OK(STRING) 或 ERR(...)
)
```

**参数**:
- `model`:STRING,模型标识(如 `"gpt-4"`, `"claude-3"`)
- `prompt`:STRING,提示词
- `opts`:DICT,可选配置:
  - `timeout`:INTEGER,毫秒,默认 `30000`
  - `max_tokens`:INTEGER,默认 `4096`
  - `temperature`:FLOAT,默认 `0.7`
  - `system`:STRING,系统提示(可选)

**返回**:
- `OK(content: STRING)` —— LLM 响应内容
- `ERR(...)` —— 见错误码

**错误码**:
- `E0080`:超时(`retryable: TRUE`)
- `E0081`:响应内容超长
- `E0082`:凭据缺失
- `E0083`:模型未找到

**示例**:

```wlwl
LET(r, ASK("gpt-4", "解释 WLWL 的 ERR 透明传播", ["timeout": 60000]));
IF(IS_OK(r),
    (PRINT(UNWRAP(r))),
    (PRINT(["ASK failed:", r]))
);
```

#### 15.11.2 `EMBED` —— 计算文本向量

**签名**:

```wlwl
FUN(EMBED(text: STRING, model: STRING = "default"),
    // 返回 OK(ARRAY<FLOAT>) 或 ERR(...)
)
```

**参数**:
- `text`:STRING,待嵌入文本
- `model`:STRING,嵌入模型名(默认 `"default"`,由环境变量决定)

**返回**:
- `OK(vector: ARRAY<FLOAT>)` —— 嵌入向量
- `ERR(...)` —— 同 `ASK`

#### 15.11.3 `COMPLETE` —— 代码补全

**签名**:

```wlwl
FUN(COMPLETE(context: STRING, language: STRING = "wlwl", opts: DICT = []),
    // 返回 OK(STRING) 或 ERR(...)
)
```

**参数**:
- `context`:STRING,代码上下文(通常是编辑器提供的 surrounding code)
- `language`:STRING,目标语言,默认 `"wlwl"`
- `opts`:DICT,同 `ASK` 加:
  - `max_suggestions`:INTEGER,默认 `3`

**返回**:
- `OK(suggestion: STRING)` —— 补全建议(单个;多建议用 ARRAY 的扩展形式)
- `ERR(...)` —— 同 `ASK`

#### 15.11.4 流式与并发(预留)

v0.3 同步调用。v0.4 议程:
- 流式响应:`ASK_STREAM(model, prompt, callback)` —— 边收边处理
- 并发调用:`ASK_ALL(prompts: ARRAY)` —— 批量调用
- 详细 API 见 §18.3 未决问题。

---

## 16. 工具设计原则(沿用 v0.2)

> **本章不规定具体命令、具体实现技术栈,只规定 WLWL 工具链必须遵守的设计契约。**

### 16.1 工具的 AI 契约

任何与 WLWL 交互的工具(AI 代理、LSP 客户端、CI、IDE 插件)若要"AI 友好",**必须**:

1. **优先用 `--format=jsonl`** 流式消费错误信息
2. **总是读取 `suggestion_code` 字段**并尝试自动应用
3. **总是把 `related` 字段全部纳入上下文**
4. **总是尊重 `severity` 字段**
5. **总是根据 `retryable` 字段**决定是否重试
6. **总是根据 `errorCategory` 字段**选择修复策略
7. **总是优先使用 §14.4 错误码表**做错误分类
8. **总是先检查 `error_schema_version` 字段**

### 16.2 工具不应做的事

- 不应在用户未授权时自动修改源码
- 不应忽略 `code` 字段而用自然语言推断错误类型
- 不应假设错误信息是自然语言——必须按结构化字段解析
- 不应同时输出多个 `severity="error"` 级别的错误
- 不应对 `retryable: FALSE` 的错误盲目重试

### 16.3 不在本规范范围的内容

- 具体命令(`wlwl run`、`wlwl check`、`wlwl fmt` 等)
- 具体实现技术栈
- REPL、调试器、性能分析器的具体形式
- LSP、formatter、syntax highlighting 的具体协议
- `wlwl.lock` 的具体算法(只规定**存在性**与**生成时机**)

---

## 17. ~~实现参考~~(已删除)

> v0.2 已删除,不在规范范围。

---

## 18. 未决问题(更新)

### 18.1 v0.3 已解决(从 v0.2 移出)

下列问题在 v0.3 中已解决,不再列为未决:

1. ~~AS 语法的最终形式~~ → §13.4 定型(导入时重命名)
2. ~~跨目录模块引用~~ → §13.5 三种路径形式
3. ~~包管理完全空白~~ → §13.8 `wlwl.toml` + `wlwl.lock`
4. ~~`std.ai` 的具体签名~~ → §15.11 完整签名
5. ~~错误码覆盖场景不足~~ → §14.4 扩展到 33 个
6. ~~错误输出流式支持~~ → §14.9 JSONL
7. ~~自然语言语义过度歧义~~ → §19 形式化语义附录(覆盖核心子集)

### 18.2 v0.4 短期(可能在 v0.4 决定)

1. **渐进类型监控(monitoring)**:§2.6 提到 v0.4 可能引入,需明确 Natural vs Transient 的具体行为、`strict_types: true` 的开关语义。
2. **`std.ai` 流式与并发**:§15.11.4 预留,需要 `ASK_STREAM` / `ASK_ALL` 等异步 API。
3. **模块版本约束算法**:`wlwl.toml` 中版本约束(目前仅 `^0.5.0`、`>=1.0.0, <2.0.0` 等 SemVer 表达式),需明确求解算法(Cargo-style 还是 npm-style)。
4. **错误码 v2 扩展**:预留 `E0090`(网络)等,需补充网络类错误码(连接拒绝、DNS 失败、TLS 错误等)。
5. **`AS` 函数的去留**:§13.4 中保留的单参数 `AS` 函数,v0.4 决定是否移除。

### 18.3 中长期

6. **SYMBOL 类型**:v0.2 §18.2 已拒绝,维持拒绝。
7. **原型委托**:v0.2 §18.2 已拒绝,维持拒绝。
8. **`error sets` 借鉴 Zig**:v0.2 §18.2 已拒绝,维持拒绝。
9. **形式化契约** `requires`/`ensures`/`invariant`:v0.2 §18.2 已拒绝,维持拒绝。
10. **元编程 / 宏**:语言级宏,继续留给 v0.5+。
11. **async/await**:v0.3 仍未引入;`std.ai` 流式是异步的"局部解法"。
12. **多目标编译**:HTML/SQL/交换机配置编译器的目标 AST 形状,远期 TODO。
13. **包管理中央仓库协议**:v0.3 仅有 `wlwl.toml` 与本地路径,中央仓库协议在 v0.5+。
14. **Unicode 标识符规范化**、**正则字面量**、**字符串 interning** 等小决策。

---

## 19. 形式化语义(附录式)[v0.3 新增]

> **本章是附录,不是规范主体**。本规范的主体(§1–§18)是**自然语言**,是 AI 工具和人类开发者的**主要阅读对象**。本章是**无歧义参考**,供实现者交叉验证、供未来形式化证明打基础。
>
> **形式化范围限制**:
> - ✅ 形式化:**核心子集**(LET / IF / WHILE / FOR / FUN / RETURN / BREAK / CONTINUE / 表达式 / 函数调用 / ERR 传播 / 词法作用域 / 模块解析)
> - ❌ 不形式化:完整 OOP、继承链、std.io / fs / json / ai、std.web 编译目标
>
> **风格选择**:small-step 操作语义,CEK 抽象机风格(参考 Delaët, Blazy, Merigoux, PPDP 2025)。

### 19.1 目标

1. 为关键语义(尤其 **ERR 透明传播**)提供无歧义参考
2. 让不同实现之间可交叉验证
3. 给未来证明(如类型安全、模块隔离)打基础

### 19.2 语法(精简)

仅核心子集,完整语法见 §3-§5。

```
e ::= n                    // 数字字面量
    | s                    // 字符串字面量
    | TRUE | FALSE | NULL  // 常量
    | x                    // 变量引用
    | FN(args, e)          // 函数字面量
    | CALL(e, e*)          // 函数调用(变长参数)
    | LET(x, e)            // 变量绑定
    | SET(x, e)            // 变量重赋值
    | IF(e, e, e)          // 条件
    | WHILE(e, e)          // 循环
    | FOR(x, e, e)         // 遍历
    | RETURN(e?)           // 返回
    | BREAK() | CONTINUE() // 循环控制
    | BLOCK(e*)            // 块表达式
    | OK(e) | ERR(e)       // 错误值构造
    | TRY(e)               // 错误传播
    | IS_OK(e) | IS_ERR(e) // 错误判断
    | OR_DIE(e, e)         // 错误兜底
    | PANIC(e)             // 不可恢复错误
```

### 19.3 状态(CEK 风格)

#### 三元组 `<e, σ, κ>`

- `e`:表达式(待求值)
- `σ`:环境(env),`Name → Value × Mutability`
- `κ`:控制栈(continuation),记录**剩余**计算

#### 值(Value)

```
v ::= n | s | TRUE | FALSE | NULL
    | [v, v, ...]                  // ARRAY
    | {k: v, k: v, ...}            // DICT
    | CLOSURE(args, body, σ')      // 闭包:捕获环境
    | OK(v) | ERR(v)               // 错误值
```

#### 控制栈元素(κ)

```
κ ::= []                          // 结束(HALT)
    | ARG(v, κ)                   // 已求值的实参(累积)
    | ARGS(v*, e*, σ, κ)          // 未求值的实参列表
    | IF-THEN(e, σ, κ)            // 等待 cond 结果
    | IF-ELSE(σ, κ)               // 等待 cond 与 then 结果
    | WHILE-COND(e, σ, κ)         // 等待循环条件
    | WHILE-BODY(e, σ, κ, v)      // 等待循环体结果
    | FOR-NEXT(x, iterable, e, σ, κ, idx)
    | LET-CONT(x, σ, κ)           // 等待 LET 右值
    | SET-CONT(x, σ, κ)           // 等待 SET 右值
    | TRY-CONT(σ, κ)              // 等待 TRY 内表达式
    | RETURN-CONT(σ)              // 函数返回
    | HANDLE-ERR(σ, κ)            // 等待 ERR 处理(TRY/OR_DIE)
```

### 19.4 求值关系:小步规约

`⟨e, σ, κ⟩ → ⟨e', σ', κ'⟩`:一个求值步。

#### 基本规则

```
E-Lit    ⟨n, σ, κ⟩ → ⟨n, σ, κ⟩                  // 字面量是值,不消耗步(可视为"已终止")
E-Var    ⟨x, σ, κ⟩ → ⟨σ(x).value, σ, κ⟩        // 查表
```

#### LET

```
E-Let    ⟨LET(x, e), σ, κ⟩ → ⟨e, σ, LET-CONT(x, σ, κ)⟩
E-LetVal ⟨v, σ, LET-CONT(x, σ', κ')⟩ → ⟨NULL, σ'[x ↦ (v, MUTABLE)], κ'⟩
```

#### IF

```
E-If     ⟨IF(c, t, e), σ, κ⟩ → ⟨c, σ, IF-THEN(t, σ, e, κ)⟩
E-IfT    ⟨TRUE, σ, IF-THEN(t, σ, e, κ)⟩ → ⟨t, σ, κ⟩
E-IfF    ⟨FALSE, σ, IF-THEN(t, σ, e, κ)⟩ → ⟨e, σ, κ⟩
```

#### 函数调用与 ERR 透明传播(关键!)

```
E-Call   ⟨CALL(f, e_1, ..., e_n), σ, κ⟩ → ⟨f, σ, ARGS([e_1, ..., e_n], σ, κ)⟩
E-CallF  ⟨CLOSURE(args, body, σ'), σ, ARGS([e_1, ..., e_n], σ', κ)⟩ →
         ⟨e_1, σ, ARG(v_0, ARGS([e_2, ..., e_n], σ, κ))⟩  // 假设 e_1 正常求值...
```

**ERR 透明传播**:

```
E-CallErr  // 实参是 ERR
⟨ERR(v), σ, ARG(_, ARGS(rest, σ', κ))⟩ →
⟨ERR(v), σ, ARGS(rest, σ', κ)⟩              // ERR 不被消费,继续作为下一个实参

// 更一般地:任何函数接收到 ERR 入参 → 直接返回 ERR,不进入函数体
E-TransparentErr
⟨CALL(f, e_1, ..., ERR(v), ..., e_n), σ, κ⟩ →
⟨CALL(f, e_1, ..., e_i), σ, κ⟩              // 其中 e_i 求值为 ERR(v) 后的中间态
... → ⟨ERR(v), σ, κ⟩                         // 最终直接返回 ERR
```

**白名单:4 个函数可消费 ERR**

```
E-IsOk    ⟨IS_OK(OK(v)), σ, κ⟩ → ⟨TRUE, σ, κ⟩
E-IsErr   ⟨IS_OK(ERR(v)), σ, κ⟩ → ⟨FALSE, σ, κ⟩
E-OrDieOk ⟨OR_DIE(OK(v), d), σ, κ⟩ → ⟨v, σ, κ⟩
E-OrDieEr ⟨OR_DIE(ERR(v), d), σ, κ⟩ → ⟨d, σ, κ⟩
E-TryOk   ⟨TRY(OK(v)), σ, κ⟩ → ⟨v, σ, κ⟩
E-TryErr  ⟨TRY(ERR(v)), σ, HANDLE-ERR(σ, κ)⟩ → ⟨ERR(v), σ, RETURN-CONT(σ)⟩  // 传播到当前函数
```

**§19.4 的覆盖范围总结**:
- ✅ 普通函数接收 ERR:透明传播,具体规则在 E-TransparentErr 系列
- ✅ 4 个白名单函数:精确规则
- ❌ OOP 方法、模块函数、std.ai 调用:实现可自由实现 ERR 处理(但建议一致)

### 19.5 模块解析(摘要)

```
// 模块加载
⟨IMPORT(p, names), σ_top, κ_top⟩ →
// 1. 解析路径 p(相对/命名空间/简单名)
// 2. 加载模块 m,获取其 EXPORT 列表
// 3. 验证 names ⊆ m.EXPORT,否则 E0023
// 4. 将 names 注入当前作用域 σ_top
// 5. 递归检查循环导入(拓扑序),违反则 E0041
```

### 19.6 ERR 透明传播的形式化(关键)

这是 WLWL v0.2 引入的**核心语义**,在 v0.3 形式化附录中给完整定义。

#### 定义(透明传播)

> **定义 19.1(ERR 透明传播)**:函数 `f` 称为**对 ERR 透明**,若对任意 `σ, κ, v_err, e_1, ..., e_n`,当 `e_i` 求值为 `ERR(v_err)` 时,`CALL(f, e_1, ..., e_n)` 不进入 `f` 的函数体,直接规约为 `ERR(v_err)`。

#### 定理(白名单)

> **定理 19.1(白名单穷尽)**:WLWL 标准库中,仅有 4 个函数**不**对 ERR 透明:`IS_OK`、`IS_ERR`、`OR_DIE`、`TRY`。其他函数(包括所有用户函数、所有 std.io/fs/json/math/web 函数)对 ERR 透明。

#### 推论(无错误吞掉)

> **推论 19.1**:若程序执行到顶层仍未被 `TRY`/`OR_DIE`/`IS_OK`/`IS_ERR` 显式消费,`ERR` 逃逸到顶层 → `E0102`(未处理的 ERR 逃逸到顶层)。

#### 形式化收益

- **可证明**:ERR 不会在传播过程中被意外吞掉(除非显式调用白名单函数)
- **可验证**:两个不同实现(比如 Rust 解释器 vs Python 字节码)可在同一测试套件下验证 ERR 传播行为
- **可扩展**:新增函数时,只需在白名单中显式登记"消费 ERR",其余默认透明

### 19.7 范围与限制(诚实声明)

- 本附录是**非形式化证明**(no mechanized proof)。虽然规则用 CEK 风格书写,**未**在 Coq/Rocq/Agda 中机械化。
- §19.4 仅覆盖核心子集;OOP、std.io/fs/json/ai 等**未**形式化,**仍是**自然语言规范。
- 形式化与自然语言规范**冲突时**,以**自然语言为准**——本附录是参考,不是 source of truth。
- **未来工作**:v0.4 考虑将本附录机械化(用 Coq / Lean / Rocq)。

---

## 附录 A:Hello World(沿用 v0.2)

```wlwl
IMPORT("wlwl:std.io", ["PRINT"]);

FUN(hello(name),
    (PRINT(+(+("hello, ", name), "!")))
);

hello("world");
```

运行:

```
$ wlwl run
hello, world!
```

## 附录 B:完整程序示例(更新)

### B.1 矩形类(沿用 v0.2)

```wlwl
// rectangle.wl
CLASS("Rectangle", NULL, [
    "width": 0,
    "height": 0,
    "init": FUN((self, w, h),
        SET_PROP(self, "width", w);
        SET_PROP(self, "height", h)
    ),
    "getArea": FUN((self),
        *(GET_PROP(self, "width"), GET_PROP(self, "height"))
    )
]);

EXPORT(["Rectangle"]);
```

### B.2 std.ai 完整示例[v0.3 新增]

```wlwl
// ai_demo.wl
IMPORT("wlwl:std.io", ["PRINT"]);
IMPORT("wlwl:std.ai", ["ASK", "EMBED"]);

FUN(summarize(text),
    LET(r, ASK("gpt-4", +("请用一句话总结:", text), ["max_tokens": 100]));
    IF(IS_OK(r),
        (PRINT(UNWRAP(r))),
        (LET(err, r);
         IF(=(GET_PROP(err, "code"), "E0080"),
            (PRINT("LLM 超时,稍后重试")),
            (PRINT(["LLM 失败:", err]))
         )
        )
    )
);

FUN(similarity(a, b),
    LET(va, EMBED(a));
    LET(vb, EMBED(b));
    IF(AND(IS_OK(va), IS_OK(vb)),
        (cosine_sim(UNWRAP(va), UNWRAP(vb))),
        NULL
    )
);

EXPORT(["summarize", "similarity"]);
```

### B.3 跨目录导入[v0.3 新增]

```wlwl
// src/main.wl
IMPORT("./utils/helpers", ["format_date"]);    // 同目录子目录
IMPORT("../shared/config", ["load"]);          // 父目录
IMPORT("wlwl:std.io", ["PRINT"]);              // 标准库

LET(today, format_date(NOW()));
PRINT(today);
```

## 附录 C:面向 AI 的错误信息示例(更新)

输入:

```wlwl
LET(x, /(10, 0));
```

输出(`--format=jsonl`):

```jsonl
{"error_schema_version":"1.0.0","code":"E1003","severity":"error","errorCategory":"runtime","retryable":false,"message":"division by zero","location":{"file":"main.wl","line":1,"col":9,"line_end":1,"col_end":18},"source_line":"LET(x, /(10, 0));","hint":"检查除数 0 是否应该避免。考虑在除法前用 IF(=(y, 0), ...) 守卫。","suggestion_code":[{"kind":"patch","description":"在除法前增加零值守卫","patch":[["replace",1,1,1,20,"IF(=(y, 0), RETURN(ERR([\"kind\": \"ZeroDiv\"])), /(y, z))"]]}],"related":[]}
```

## 附录 D:v0.2 → v0.3 变更日志

| 类别 | 变更 | 来源/依据 |
|------|------|-----------|
| §2.6 渐进类型立场 | 新增 Transient 立场说明,引用 TypeScript / POPL 2025 / PLDI 2024 学术依据 | 第三方审计"核心特性占位" |
| §13.1 模块名解析 | 新增显式 `MODULE()` 声明 + toml package.name + 文件名 三级 fallback | 跨目录规则需要 |
| §13.3 IMPORT 三参数 | 形式定型,加 opts 预留 | 跨目录 + 命名空间需要 |
| §13.4 导入时重命名 | **取代**单独的 `AS` 函数,Go/Rust 工业实践 | 第三方审计"AS 语法未定型" |
| §13.4 AS 函数 | 保留单参数 `AS` 但标记不推荐 | 对称性,v0.4 决定去留 |
| §13.5 跨目录 | 新增相对路径 `./` / `../`,项目根边界 | 第三方审计"跨目录规则缺失" |
| §13.6 命名空间路径 | 新增 `wlkl:std` / `myteam:utils` 形式,含冲突规则 E0043 | 第三方审计"包管理空白" |
| §13.7 循环导入 | 错误信息增强,列出完整环路路径 | v0.2 已存在,v0.3 增强 |
| §13.8 wlwl.toml | 新增完整包清单格式(path 依赖、版本约束、命名空间映射、features) | 第三方审计"包管理空白" |
| §13.10 模块契约 | 形式细化,`errors` 字段预留 | v0.2 已有,v0.3 增强 |
| §14.2 errorCategory | 新增 13 类错误分类 | 2026 行业共识(MCP/AI 错误设计) |
| §14.2 retryable | 新增 TRUE/FALSE 字段 | 2026 错误设计最佳实践 |
| §14.2 error_schema_version | 新增 SemVer 版本号 | 规范演进兼容性 |
| §14.4 错误码 | 23 → 33,新增 IO/JSON/AI/Network 10 个 | 第三方审计"错误码覆盖不足" |
| §14.9 JSONL 流式输出 | 新增 `--format=jsonl` 模式 | 2026 行业共识(LLM 流式消费) |
| §14.10 schema 版本化 | 错误对象 schema 版本号规则 | 规范演进兼容性 |
| §15.11 std.ai | 从占位变完整签名(ASK/EMBED/COMPLETE 同步调用) | 第三方审计"核心特性占位" |
| §18.1 已解决 | 移除 v0.2 的 7 个未决问题(已解决) | 增量管理 |
| §19 形式化语义 | **新增**附录,small-step CEK 风格,覆盖核心子集 | 第三方审计"语义描述仍为自然语言" |
| §19.6 ERR 传播定理 | 形式化 §12.6 的 ERR 透明传播,白名单 + 推论 | v0.2 核心语义的形式化 |

---

> 该归档文件同时包含 v0.1 时代的草稿(1.txt/2.txt 因不在 standard 未归档)+ 三方审计(3.txt/4.txt/5.txt)+ v0.1 完整原文 + v0.2 完整原文(本节指向),形成版本演进的完整时间线。

**文档结束。生成时间:2026-09-02。**
