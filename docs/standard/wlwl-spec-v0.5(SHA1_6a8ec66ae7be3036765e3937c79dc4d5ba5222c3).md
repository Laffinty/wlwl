# WLWL 语言规范 v0.5

> 版本 0.5 · 2026-09-19
> 本规范以参考实现 wlwl(命令行可执行文件 `wlwl.exe`)的可观察行为为依据写成,是 WLWL 程序设计语言的完整定义。文中所有语义均经参考实现验证。

## 0 导言

### 0.1 本规范的构成

本规范定义 WLWL 程序设计语言:词法、语法、类型与值、求值规则、错误模型、模块系统、标准库与诊断。

WLWL 是一门**动态类型、前缀函数式**的语言:

- **一切都是表达式**。没有语句与表达式的二分;每个构造求值后产生一个值。
- **一切运算皆调用**。没有中缀运算符;算术、比较、逻辑都是具名函数,按 `name(arg, ...)` 前缀形式调用。
- **默认不可变**。绑定一旦建立即指向固定值;容器操作返回新容器而非修改原容器;状态突变只能经由被闭包捕获的单元格(cell)显式进行。
- **错误即值**。可恢复错误是 `RESULT` 值,沿调用链透明传播,由显式的消费者函数处理。

### 0.2 读者

本规范面向三类读者:WLWL 程序员、语言工具(格式化器、静态检查器、编辑器)的实现者,以及参考实现本身的维护者。

### 0.3 规范性用语

本规范中的关键词 **必须**(MUST)、**不得**(MUST NOT)、**应当**(SHOULD)、**可以**(MAY)按以下含义使用:

- **必须 / 不得**:实现的硬性约束。违反时实现必须产生一个诊断(第 11 章)并按第 11.4 节终止或报告。
- **应当**:强烈推荐的行为;实现可以偏离,但偏离必须在本规范或实现文档中说明。
- **可以**:真正的实现自由度。

标注为**非规范**(non-normative)的段落、注释与附录不构成一致性要求。

### 0.4 一致性

一个 WLWL 程序是符合本规范的,当且仅当它只使用本规范定义的语法形式与语义,且不依赖任何标注为未定义、保留或实现定义的行为。一个实现是一致的,当且仅当它接受所有一致的程序并产生本规范规定的结果。

### 0.5 记法

语法用扩展巴科斯范式(EBNF)书写,记法约定见附录 A。正文中 `code` 样式表示词法记号、字面代码或诊断码;`name` 斜体表示文法产生式。示例代码以 `//` 注释标出其求值结果。

---

## 1 词法结构

### 1.1 源文件

一个 WLWL 源文件是一个 **必须** 为 UTF-8 编码(带或不带 BOM)的字节序列。换行可以是 `\n`、`\r\n` 或 `\r`,三者等价。源文件是一个模块(第 9 章)。

### 1.2 注释

- `//` 开始一个**行注释**,延伸到行尾。
- `/*` 与 `*/` 界定一个**块注释**。块注释**必须**正确嵌套。
- `///` 开始一个**文档注释**,延伸到行尾。文档注释只在词法上与行注释相区别,**不得**影响程序语义。

注释中出现的任何字符序列(除上述定界规则外)都是注释的一部分。

### 1.3 标识符

```
identifier = letter { letter | decimal_digit | "_" } .
```

`letter` 含 Unicode 字母类别(含中文)。标识符区分大小写。非 ASCII 标识符是合法的;工具**应当**在诊断中如实回显。

以 `_` 单独出现的标识符是**通配符**,见 3.1 与 7.3。

### 1.4 关键字

```
TRUE    FALSE   NULL    LET     FUN     RETURN
IF      WHILE   FOR     BREAK   CONTINUE
CLASS   NEW     THIS    NOT
```

关键字不得用作标识符。`CLASS`、`NEW`、`THIS` 是**保留关键字**:当前实现中它们的求值产生诊断 `E0020`(第 12 章)。

### 1.5 运算符记号

以下记号是内建运算函数名,可用前缀调用形式书写(4.3、4.6):

```
+   -   *   /   %   ==  =   !=  <   >   <=  >=  &&  ||  !
```

`=` 与 `==` 在调用位置同义(都指相等函数);`!` 与 `NOT` 关键字(1.4)同义,但 `!` 形式在诊断流中附带弃用警告 `W0054`。`=` 另在形参表中作默认值分隔符(5.2),两种角色由文法位置区分,互不冲突。

### 1.6 整数字面量

```
int_lit        = [ "+" | "-" ] decimal_digit { decimal_digit } .
```

整数值是有符号 64 位整数。运算结果超出该范围的规则见 2.2。

### 1.7 浮点字面量

```
float_lit      = digits "." digits | digits exponent | digits "." digits exponent .
exponent       = ( "e" | "E" ) [ "+" | "-" ] digits .
```

浮点值是 IEEE 754 双精度数。整数与浮点字面量统称**数字字面量**。

### 1.8 字符串字面量

```
string_lit     = '"' { string_element } '"' .
string_element = any_char_except_quote_backslash_newline | escape .
escape         = "\" ( "n" | "t" | "r" | "\" | '"' | "/" | "0" | "b" | "f" ) .
```

字符串是 Unicode 码点的不可变序列。承认的转义:`\n` 换行、`\t` 水平制表、`\r` 回车、`\\` 反斜杠、`\"` 引号、`\/` 斜杠、`\0` 空、`\b` 退格、`\f` 换页。其他转义**不得**出现(`E0001` 或 `E0002`)。

WLWL **没有**字符串插值语法。构造含值的字符串用 `+` 拼接或 `FORMAT`(10.7)。

### 1.9 布尔与空字面量

`TRUE`、`FALSE` 产生布尔值;`NULL` 产生空值。

### 1.10 分号

处于"语句位置"的表达式**必须**以 `;` 结束。在括号序列(4.7)内部,最后一个表达式的 `;` **可以**省略;该表达式的值即括号序列的值。省略必需的 `;` 产生 `E0013`。

---

## 2 类型与值

### 2.1 值域

WLWL 是动态类型语言:类型属于值而非名字。值的论域由以下**类型**构成:

| 类型 | 含义 |
|------|------|
| `NULL` | 空值,唯一成员 `NULL` |
| `BOOLEAN` | `TRUE` 或 `FALSE` |
| `INTEGER` | 有符号 64 位整数 |
| `FLOAT` | IEEE 754 双精度浮点数 |
| `STRING` | Unicode 码点的不可变序列 |
| `ARRAY` | 值的不可变有序序列 |
| `DICT` | 从键(STRING 或 INTEGER)到值的插入序映射,不可变 |
| `FUNCTION` | 闭包(5.5) |
| `RESULT` | `OK(v)` 或 `ERR(e)` 变体(第 8 章) |

数组与字典字面量见 4.8;`TYPE(x)` 返回上表名称的字符串(`RESULT` 两个变体都报 `"RESULT"`)。

**键的约束**:字典的键必须是 `STRING` 或 `INTEGER`。NaN(2.2)**不得**作键(`E0031`)。

### 2.2 算术性质

- 整数运算按二进制补码语义进行;`+`、`-`、`*` 溢出时**饱和**到 `INTEGER` 边界,并附带警告 `W0015`。`NEG` 于 `INTEGER` 下界取负产生 `E0034`。
- `/(a, b)` 当两操作数为整数时是**截断除法**(商向零取整):`/(7, 2)` 为 `3`,`/(-7, 2)` 为 `-3`。两操作数为浮点时是浮点除法,可产生 `Inf`/`NaN`。
- `b` 为零时 `/(a, b)` 产生 `E1003`(整数、浮点皆然,即使 IEEE 754 允许 `Inf`)。
- `%(a, b)` 只接受整数:`a - /(a, b) * b`,符号与被除数一致;`b` 为零产生 `E1003`。

### 2.3 真值

**假值**恰为两个:`FALSE` 与 `NULL`。**其余一切值为真**,包括 `0`、`0.0`、`""`、空数组、空字典、`NaN` 以及 `OK(FALSE)` 之类的 `RESULT` 值。

真值仅由 6.1 的 `IF`、6.2 的 `WHILE`、`&&`、`||`、`NOT`、`BOOL` 以及模式匹配以外的谓词位置消费。

### 2.4 相等

`==(a, b)`(别名 `=`,1.5)与 `!=(a, b)`:

- 数字跨类型比较:`==(1, 1.0)` 为 `TRUE`。
- 字符串按码点逐位比较;布尔、空按恒等。
- 数组按长度与逐元素深度相等;字典按键集与逐键值深度相等(受上一条 NaN 规则影响:含 NaN 的容器与任何值都不等)。
- 函数按闭包实例引用相等。
- `RESULT`:`OK(a) = OK(b)` 当且仅当 `a = b`;`ERR(e1) = ERR(e2)` 当且仅当 `e1 = e2`;`OK` 与 `ERR` 永不相等。
- 任一操作数为 `ERR` 变体时,比较函数**不**求布尔,而是透明传播该 `ERR`(8.2)。

### 2.5 显示

`PRINT`(10.2)与 `STR`(10.3)将值渲染为文本:布尔为 `TRUE`/`FALSE`,空为 `NULL`,数组为 `[e1, e2, ...]`,字典为 `[k: v, ...]`,函数为 `<fun(params)>`。`STR` 对字符串恒等;对浮点保留小数表示(整数产生的浮点显示一位小数,如 `2.0`)。

---

## 3 变量、作用域与单元格

### 3.1 绑定

`LET(name, value)` 在**当前作用域**为 `name` 建立一个新绑定,并将 `value` 存入为新创建的**单元格**(cell)。`LET` 表达式本身的值是 `NULL`。

```
LET(x, 1);
```

`LET` 的第一个实参**可以**是解构模式,此时按模式同时建立多个绑定;模式必须与值精确匹配,失配产生 `E0026`:

```
LET([a, b], [1, 2]);            // a=1, b=2
LET([head, *rest], [1, 2, 3]);  // head=1, rest=[2, 3]
LET(["k": v], ["k": 9]);        // v=9
LET([_, x, _], [1, 2, 3]);      // x=2
```

模式形态的完整定义见 7.3。

### 3.2 作用域与遮蔽

作用域由**函数体、括号序列(4.7)、`FOR` 循环体、`MATCH` 分支体**引入,以及每个模块的顶层。作用域按词法嵌套。

- 内层作用域中的 `LET` **总是**在内层新建单元格;若外层已有同名绑定,内层绑定**遮蔽**外层。
- 同一作用域中对同一名字再次 `LET` 是**重绑定**:旧绑定被替换。
- 读一个不存在的名字产生 `E0020`(附最近候选名)。
- **没有**变量提升:在 `LET` 之前读名字产生 `E0020`。

### 3.3 单元格可变性

每个绑定由一个单元格实现,单元格持有值与一个可变标志。可变标志初始为假。

当一个闭包捕获某单元格、且该闭包**被首次调用**时,其捕获环境中的全部单元格被升级为可变。此后任何持有该单元格的作用域都可以 `SET` 它。

```
LET(make_counter, FUN((),
    (LET(count, 0);
     FUN((), (SET(count, +(count, 1)); count)))
));
LET(c, make_counter());
c();   // 1
c();   // 2 — count 的单元格在闭包间共享
```

这是 WLWL 中唯一的原位状态突变机制。循环累加等命令式惯用法应当经由它或 `REDUCE`(10.6)表达。

### 3.4 SET

`SET(name, value)` 将 `name` 当前绑定的单元格改写为 `value`:

- 名字不存在 → `E0020`。
- 单元格不可变(3.3)→ `E0024`。典型情形:在子作用域中 `SET` 一个未被闭包捕获的外层绑定。
- `SET` 表达式的值是 `NULL`。

### 3.5 遮蔽内建

`LET` 或命名函数定义(5.1)遮蔽第 10 章的内建名时:默认产生 `E0025`;当模块清单声明 `[features] allow_builtin_shadow = true` 时允许并产生警告 `W0030`。遮蔽保留关键字(1.4)产生 `W0030`。

---

## 4 表达式

### 4.1 操作数

操作数是字面量(第 1 章)、标识符、括号序列(4.7)、数组与字典字面量(4.8)、或下文定义的各构造。

### 4.2 调用

```
Call = identifier "(" [ Expression { "," Expression } ] ")" .
```

调用将实参自左向右全部求值后执行被调者。被调者不是函数值时产生 `E0020`。

### 4.3 运算符即函数

所有运算符都是具名函数,以 1.5 的记号作前缀调用:

| 调用 | 语义 | 结果 |
|------|------|------|
| `+(a, b)` | 数字相加(整数与浮点混合提升为浮点);字符串拼接;数组拼接 | 同类 |
| `-(a, b)`、`*(a, b)` | 减、乘;类型规则同上 | 数字 |
| `/(a, b)` | 除法,见 2.2 | 数字 |
| `%(a, b)` | 整数取余,见 2.2 | 整数 |
| `NEG(a)` | 取负 | 数字 |
| `==(a, b)`、`!=(a, b)` | 相等、不等(`=` 是 `==` 的别名) | 布尔 / ERR 透传 |
| `<(a, b)`、`>(a, b)`、`<=(a, b)`、`>=(a, b)` | 比较;数字按数值,字符串按码点序 | 布尔 / ERR 透传 |
| `&&(a, b)`、`||(a, b)` | 合取、析取;**两实参都求值**(不短路),结果按真值(2.3)合成 | 布尔 |
| `NOT(a)` | 真值取反 | 布尔 |

- `+`、`-`、`*`、`/`、`%`、`CONCAT` 是**严格二元**的;实参数不符产生 `E0022`。
- 任一操作数为 `ERR` 时,除 `NOT` 外全部透明传播(8.2)。
- 需要条件求值(短路)时使用 `IF`(6.1)。

### 4.4 括号序列(块)

```
Block = "(" [ Expression ";" ] { Expression ";" } [ Expression ] ")" .
```

括号序列按顺序求值其中的表达式;其值是最后一个表达式的值。空括号序列 `()` 的值是 `NULL`。

### 4.5 下标

```
Index = Expression "[" Expression "]" | Expression "[" Expression "]" "=" Expression .
```

`a[i]` 是 `INDEX_GET(a, i)` 的语法糖;`a[i] = v` 是 `INDEX_SET(a, i, v)` 的语法糖。两者可以与 4.6 的属性链自由组合:`a[0].b`、`a.b[0][1]`。

- `INDEX_GET(coll, k)`:数组接受整数索引(负数自尾部计数,`-1` 为末元素),越界产生 `E0036`;字典按键取值,键类型不符产生 `E0031`,键不存在产生 `E0037`。字符串不是下标容器(用 `SUB`/`SPLIT`)。
- `INDEX_SET(coll, k, v)` 返回**插入或更新后的新容器**;接收者不变(见 2.1 不可变性与附录 B)。
- 下标链挂在变量、调用或属性访问之后;对数组、字典字面量直接施加下标不在本规范内。

### 4.6 属性与方法

```
Selector = Expression "." identifier [ "(" [ Expression { "," Expression } ] ")" ] .
```

`a.b` 是 `GET_PROP(a, "b")` 的语法糖;`a.b(args)` 是 `CALL_METHOD(a, "b", args...)` 的语法糖。接收者通常是字典(模块也是字典,9.4):

- `GET_PROP(obj, k)`:键不存在产生 `E0037`。
- `a.b(args)` 调用 `a` 的 `"b"` 成员;若该成员是函数值,`a` 本身作为**首个实参**(惯用名 `self`)自动注入,即等价于调用 `f(self=a, 其余实参...)`。`CALL_METHOD` 亦总是注入接收者,**不得**再显式传入。
- 成员存在但不可调用时产生 `E0030`。

### 4.7 求值顺序

实参、括号序列各元素、数组与字典字面量的各元素,都按源代码顺序自左向右求值;除 6.1、6.2 与 `&&`/`||` 的真值语义外,不存在条件求值。`&&`/`||` 也不短路——需要短路语义时使用 `IF`。

### 4.8 数组与字典字面量

```
ArrayLit = "[" [ Expression { "," Expression } ] "]" .
DictLit  = "[" Key ":" Expression { "," Key ":" Expression } "]" .
Key      = Expression .
```

- 第一项无 `:` 时为**数组字面量**;第一项为 `k: v` 时为**字典字面量**;空字面量 `[]` 恒为空数组。空字典只有一种写法:`DICT()`(10.9)。
- 同一字面量中混用裸值与键值对产生警告 `W0020`。

---

## 5 函数

### 5.1 定义

```
Function = "FUN" [ identifier ] "(" ParameterList ")" Expression .
```

- `FUN((params), body)` 产生一个**匿名函数值**。
- `FUN(name(params), body)` 产生同样的函数值,**并**在当前作用域将 `name` 绑定到它——与 `LET(name, FUN(...))` 等价。两种形式的函数值相同,表达式的值都是该函数值。

### 5.2 形参

形参列表由以下形态组成(按位置排列):

| 形态 | 含义 |
|------|------|
| `name` | 必需参数 |
| `name = Expression` | 默认参数;缺省实参时求值该表达式。默认表达式在**被调函数的词法帧**内求值,因此可以引用更外层的捕获与排在其前的形参 |
| `*name` | 剩余参数;收集多余的实参为**新数组**。只**可以**出现在末位 |
| `name: Type`、`name: Type = Expression` | 类型注解。运行时忽略,除非模块清单开启 `strict_types`(9.6),此时在调用边界按 `TYPE` 名做顶层形状检查,失配产生 `E0033` |

### 5.3 调用与元数

设必需参数数为 `R`、形参总数为 `N`(含默认与剩余):

- 无剩余参数:`R ≤ 实参数 ≤ N`。省略的尾部实参由默认表达式填充;无默认而缺位产生 `E0022`。
- 有剩余参数:`实参数 ≥ R`;超出固定形参的实参收集为剩余数组。
- 违反范围产生 `E0022`,消息形如 `expects R..N argument(s), got A`。

### 5.4 函数是一等值

函数值可以存储、传递、返回、放入容器,没有名与值的区别。递归经由顶层绑定即可实现(调用时解析名字):

```
LET(fib, FUN((n), IF(<(n, 2), n, +(fib(-(n, 1)), fib(-(n, 2))))));
```

---

## 6 控制流

### 6.1 IF

```
If = "IF" "(" Expression "," Expression [ "," Expression ] ")" .
```

先求条件;条件为真(2.3)求第二实参并以其值为结果,否则求第三实参(若有)并以其值为结果,缺省第三实参时结果为 `NULL`。**未求值的分支不产生任何效果。** `IF` 不消费 `ERR`(8.3):条件为 `ERR` 时按真值规则取真分支;分支表达式自身产生的 `ERR` 正常传播。

### 6.2 WHILE

```
While = "WHILE" "(" Expression "," Expression ")" .
```

条件为真时反复求值体。`WHILE` 表达式的值是 `NULL`。体产生的 `ERR` 终止循环并传播。

### 6.3 FOR

```
For = "FOR" "(" identifier "," Expression "," Expression ")" .
```

对可迭代对象逐项求值体。可迭代对象是:数组(逐元素)、字典(按**插入序**逐键)、字符串(逐码点,绑定单字符字符串)。循环变量在每次迭代是**新绑定**,体结束后即不可见。`FOR` 表达式的值是 `NULL`。

### 6.4 提前退出

- `RETURN(expr?)`:从最内层函数返回,值为 `expr`(缺省 `NULL`)。
- `BREAK()`、`CONTINUE()`:终止/跳过最内层 `WHILE` 或 `FOR`。

三者出现在函数或循环体之外时产生 `E0014`。

---

## 7 模式匹配

### 7.1 语法

```
Match  = "MATCH" "(" Expression "," ClauseArray [ "," Expression ] ")" .
ClauseArray = "[" [ Clause { "," Clause } ] "]" .
Clause = "[" Pattern "," Expression "]" .
```

`MATCH` 按字面顺序试各子句的模式;首个命中的子句的右值被求值并作为 `MATCH` 的值。无命中且省略默认表达式时,值为 `NULL`。

### 7.2 模式

```
Pattern = identifier | wildcard | Literal | ArrayPattern | DictPattern | VariantPattern .
wildcard       = "_" .
ArrayPattern   = "[" [ Pattern { "," Pattern } [ "," "*" identifier ] ] "]" .
DictPattern    = "[" Key ":" Pattern { "," Key ":" Pattern } "]" .
VariantPattern = "OK" "(" Pattern ")" | "ERR" "(" Pattern ")" .
```

- 标识符模式绑定该名;通配符 `_` 匹配任意值不绑定。
- 字面量模式按 2.4 相等匹配。
- `OK(p)`/`ERR(p)` 匹配 `RESULT` 的对应变体并继续匹配载荷;对非 `RESULT` 值失配。
- 数组、字典模式按结构匹配;数组可捕余(`*rest`),字典可部分匹配(值必须包含全部模式键)。

### 7.3 语义

- 模式匹配必须是**穷尽形状**的:子句或默认表达式终将被选中。
- 模式绑定的作用域**仅为**对应分支;分支体是一个新作用域,绑定在其中遮蔽外层同名绑定,分支结束后不可见。
- 模式与值形状失配不算错误,只是不命中;`LET` 解构(3.1)失配则是错误 `E0026`。

---

## 8 错误模型

### 8.1 RESULT

`OK(v)` 把任意值 `v` 包装为 `RESULT` 的成功变体;`ERR(e)` 把载荷 `e` 包装为失败变体。`e` **必须**是字符串或字典,否则产生 `E0030`。

推荐以字典载荷携带结构化字段(`kind`、`input` 等),便于程序化消费。

### 8.2 透明传播

**任何不在消费者注册表(8.3)内的函数调用**——内建或用户定义——若任一实参求值为 `ERR`,则该调用不求函数体,直接以该 `ERR` 为调用结果。传播不消耗、不改写 `ERR`。于是未显式处理的错误沿调用链上浮,直到被消费者处理或逃逸到顶层(8.5)。

**推论(错误处理惯用法)**:检查并消费一个 `ERR` 值必须**在拥有它的作用域内**进行——先以注册表消费者(`IS_ERR`、`ERR_PAYLOAD`、`UNWRAP_OR` 等)提取信息或兜底,再把**提取结果**(字符串、字典等普通值)传给其他函数:

```wlwl
LET(r, risky());                          // r 是 ERR
LET(p, ERR_PAYLOAD(r));                   // 在此处消费,p 是普通值
PRINT(FORMAT("failed: {0}", p));          // 传普通值,安全
```

把 `ERR` 值原样作为实参传给普通函数(包括"渲染函数""检查函数")只会触发传播,函数体永不执行:

```wlwl
LET(show, FUN((r), IF(IS_ERR(r), "E", "O")));
LET(r, ERR("x"));
show(r);       // = ERR("x") — show 的体永不执行
show(r);       // 该 ERR 在顶层无消费者 → E0102,程序终止
show(OK(r));   // FALSE — OK 包装后不再是 ERR 实参,可安全传递
```

**注意**:`OK(r)` 包装可以阻断传播(此后按 2.4,`OK` 与 `ERR` 永不相等,且 `IS_OK(OK(...))` 为真)。将 `ERR` 装入数组或字典能通过**当前**求值,但取出元素再作实参时会再次触发传播——装盛不是消费。唯一合法的消费路径是 8.3 的注册构造。

### 8.3 消费者注册表

以下构造消费 `ERR` 而不触发 8.2 的传播(它们的**实参位置**是注册的 `ERR` 合法入口;返回值是普通值,可自由传递):

| 构造 | 对 `ERR` 的行为 |
|------|-----------------|
| `IS_OK(x)` / `IS_ERR(x)` | 返回布尔,不消耗载荷 |
| `UNWRAP_OR(x, d)` / `OR_DIE(x, d)` | `ERR` 时返回 `d`;`OK(v)` 时返回 `v`(`OR_DIE` 是弃用别名) |
| `TRY(x)` | `ERR` 时等价于 `RETURN(err)`,`OK(v)` 时取 `v`;只能在函数体内使用 |
| `UNWRAP(x)` | `OK(v)` 取 `v`;`ERR` 产生 `E0100` PANIC |
| `ERR_PAYLOAD(x)` | `ERR(e)` 取 `e`;对 `OK` 或非 `RESULT` 产生 `E0030` |
| `WRAP(x, ctx)` | `ERR(e)` 包装为 `ERR(["original": e, "context": ctx])`;`OK(v)` 原样返回 |
| `TYPE(x)` | 返回 `"RESULT"` |
| `==` / `!=` | 按 2.4,对 `ERR` 操作数透传而非消费 |
| `IF` / `&&` / `||` | 作为真值判断器(2.3) |

`EXPECT_ERR(x)`:供测试使用,`x` 为 `ERR` 时返回 `OK(载荷)`,否则返回 `ERR(E0049)`。

### 8.4 PANIC

`PANIC(msg)`,`msg` 必须是字符串或字典,立即以诊断 `E0100` 终止程序(11.4)。PANIC 不参与透明传播。`UNWRAP` 于 `ERR`、以及 `INTEGER` 下界取负(`E0034`)同样以 PANIC 终止。

### 8.5 顶层逃逸

`ERR` 到达程序顶层而未被消费时,实现以诊断 `E0102` 报告该载荷并以退出码 1 终止。

---

## 9 模块与程序

### 9.1 文件即模块

每个源文件是一个模块。模块顶层可以有 `EXPORT(name_array)`;未导出的绑定对外不可见。

### 9.2 IMPORT

```
Import = 'IMPORT' "(" string_lit "," name_array ")" .
name_array = "[" [ NameItem { "," NameItem } ] "]" .
NameItem = string_lit | string_lit ":" string_lit .
```

- `IMPORT(path, ["a", "b"])` 将模块 `path` 的导出名 `a`、`b` 绑定到当前作用域。
- `["orig": "alias"]` 将导出名 `orig` 绑定为本地名 `alias`(重命名)。
- 路径以 `./`、`../` 开头时按文件系统相对解析(后缀 `.wl` 可省);形如 `wlwl:std.*` 的路径指向标准库命名空间(第 10 章)。
- 模块不存在产生 `E0040`;导出名不存在产生 `E0023`;循环导入产生 `E0041`。
- 同一作用域内重复导入同一名字产生 `E0021`。

模块本身是字典值;经由 `IMPORT` 之外获得模块字典的机制(`MODULE_REF`)是保留形式(第 12 章)。

### 9.3 标准库命名空间

`wlwl:std.` 前缀的路径由实现直接提供,不经文件系统:`wlwl:std.io`、`wlwl:std.fs`、`wlwl:std.json`、`wlwl:std.collection`、`wlwl:std.format`、`wlwl:std.test`、`wlwl:std.ai`、`wlwl:std.agent`。各命名空间的成员见第 10 章。

### 9.4 清单文件

模块目录**可以**包含 `wlwl.toml` 清单,声明包名、语言版本与特性开关。本规范定义以下特性:

| 特性 | 默认 | 含义 |
|------|------|------|
| `strict_types` | `false` | 开启 5.2 类型注解的调用边界检查(`E0033`) |
| `allow_builtin_shadow` | `false` | 允许遮蔽内建名(3.5),以 `W0030` 代替 `E0025` |

清单的其余字段(依赖、版本求解、锁文件)属于包管理器职责,不在本规范内。

### 9.5 程序入口

被运行的文件即入口模块;其顶层表达式按顺序求值,最后一个表达式的值被丢弃。顶层求值产生的 `ERR` 按 8.5 处理。

---

## 10 标准库

### 10.1 约定

- **全局内建**:下表 10.2–10.5 的名字无须导入即可调用。
- **命名空间成员**:10.6–10.10 的名字**必须**经 `IMPORT` 导入。
- 除注明外,容器操作都是**非破坏性**的:返回新容器,接收者不变;更新绑定用 `SET`(3.4)。
- 对 `ERR` 实参:全局内建按 8.2 透明传播;集合套件(10.6)的回调抛出的 `ERR` 同样使整个调用传播。

### 10.2 I/O

| 签名 | 说明 |
|------|------|
| `PRINT(args...) -> NULL` | 依序渲染实参(2.5),以空格分隔输出到标准输出 |
| `PRINT_ERR(args...) -> NULL` | 同上,输出到标准错误 |
| `INPUT(prompt?) -> STRING` | 读一行标准输入 |

### 10.3 类型与转换

| 签名 | 说明 |
|------|------|
| `LEN(coll) -> INTEGER` | 数组元素数、字典键数、字符串码点数 |
| `TYPE(x) -> STRING` | 2.1 类型名;`RESULT` 报 `"RESULT"` |
| `STR(x) -> STRING` | 渲染为文本(2.5) |
| `INT(x)` | 整数→恒等;浮点→向零截断;字符串→解析整数,失败返回 `ERR(["kind": "ParseError", "input": s])` |
| `FLOAT(x)` | 浮点→恒等;整数→提升;字符串→解析,失败同上 |
| `BOOL(x) -> BOOLEAN` | 真值(2.3) |
| `CALL(fn, args...)` | 保留形式(第 12 章) |

### 10.4 容器操作(全局)

| 签名 | 说明 |
|------|------|
| `PUSH(arr, x) -> ARRAY` | 尾部追加的新数组 |
| `UNSHIFT(arr, x) -> ARRAY` | 头部插入的新数组 |
| `SHIFT(arr) -> ARRAY` | 去除首元素的新数组 |
| `POP(d, k, default) -> v` | 字典取值:`k` 存在返回其值,否则返回 `default`;不修改 `d` |
| `SLICE(arr, start, end?) -> ARRAY` | 区间 `[start, end)`;负索引自尾部计数;缺省 `end` 到末尾;越界部分截为空 |
| `CONCAT(a, b) -> ARRAY` | 两数组连接 |
| `CONTAINS(coll, x) -> BOOLEAN` | 成员测试;亦接受字符串(子串) |
| `INDEX(arr, x) -> INTEGER` | 首次出现索引,无则 `-1` |
| `REVERSE(arr) -> ARRAY` | 逆序副本 |
| `INDEX_GET(coll, k) -> v` | 见 4.5 |
| `INDEX_SET(coll, k, v) -> ARRAY/DICT` | 见 4.5;返回更新后的新容器 |
| `AT(coll, k, default) -> v` | 安全下标;越界/缺键返回 `default` |
| `KEYS(d) -> ARRAY`、`VALUES(d) -> ARRAY` | 按**插入序** |
| `HAS(d, k) -> BOOLEAN` | 键存在测试 |
| `MERGE(a, b) -> DICT` | `b` 覆盖 `a` 的新字典 |
| `REMOVE_KEY(d, k) -> DICT` | 去键的新字典;键不存在则等价于 `d` |
| `DEL(d, k) -> DICT` | `REMOVE_KEY` 的弃用别名(警告 `W0051`) |

> 注:数组没有原位弹出;移除字典键用 `REMOVE_KEY`,取值用 `AT`/`POP`。

### 10.5 字符串操作(全局)

| 签名 | 说明 |
|------|------|
| `SUB(s, start, len?) -> STRING` | 自 `start` 起长 `len` 的子串;缺省 `len` 到末尾;按码点 |
| `SPLIT(s, sep) -> ARRAY` | 按 `sep` 分割 |
| `REPLACE(s, old, new) -> STRING` | 全部替换 |
| `UPPER(s)` / `LOWER(s)` | ASCII 大小写转换;非 ASCII 行为实现定义(可能附带 `W0014`) |
| `TRIM(s)` / `TRIM_START(s)` / `TRIM_END(s)` | 去两端/前端/尾端空白 |
| `STARTS_WITH(s, pre)` / `ENDS_WITH(s, suf)` | 前后缀测试 |
| `REPEAT(s, n) -> STRING` | 重复 `n` 次 |
| `PAD_START(s, n, c)` / `PAD_END(s, n, c)` | 用单字符 `c` 填充到长度 `n` |
| `CODEPOINTS(s) -> ARRAY` / `FROM_CODEPOINTS(arr) -> STRING` | 码点整数数组与字符串互转 |

### 10.6 集合套件 — `wlwl:std.collection`

| 签名 | 说明 |
|------|------|
| `MAP(arr, f)` | `[f(v), ...]`;`f(v)` 抛 `ERR` 则整体传播 |
| `FILTER(arr, f)` | 保留 `f(v)` 为真的元素 |
| `REDUCE(arr, f, init)` | 左折叠;空数组返回 `init` |
| `SORT(arr)` / `SORT_BY(arr, key)` | 升序排序;`SORT_BY` 以 `key(v)` 为排序键 |
| `RANGE(n)` / `RANGE(start, end)` / `RANGE(start, end, step)` | 整数区间 `[start, end)`,步长 `step`;`step = 0` 产生 `E0038` |
| `ZIP(a, b) -> ARRAY` | 配对至较短者:`[[a0, b0], ...]` |
| `ENUMERATE(arr) -> ARRAY` | `[[0, v0], [1, v1], ...]` |
| `TAKE(arr, n)` / `DROP(arr, n)` | 前 n / 去前 n |
| `FLAT(arr) -> ARRAY` | 展平一层 |
| `UNIQ(arr) -> ARRAY` | 按 `==` 去重,保留首现 |
| `GROUP_BY(arr, key) -> DICT` | 按 `key(v)` 分组,组为按首现序的字典 |
| `ANY(arr, f?)` / `ALL(arr, f?)` | 任一/全部为真(缺省 `f` 时按真值) |
| `FIND(arr, f)` | 首个满足者,无则 `NULL` |
| `JOIN(arr, sep) -> STRING` | 字符串化后以 `sep` 连接 |

### 10.7 格式化 — `FORMAT`(全局)

`FORMAT(template, args...) -> STRING`:模板中 `{N}` 以第 `N` 个实参的 `STR` 渲染替换;`{name}` 以首个实参为字典按键取值;两类占位符可混用。未匹配的 `{...}` 原样保留;模板畸形(孤立的 `{`)产生 `E0039`。`wlwl:std.format` 命名空间导出同一函数。

### 10.8 JSON 与文件系统

`wlwl:std.json`:

| 签名 | 说明 |
|------|------|
| `STRINGIFY(x) -> STRING` | 序列化为 JSON;字典键按字典序输出;`NULL`/布尔/数字/字符串/数组/字典自然映射 |
| `PARSE(s)` | 解析 JSON;对象保持文档序的字典 |

`wlwl:std.fs`:

| 签名 | 说明 |
|------|------|
| `WRITE_FILE(path, text) -> NULL` | 写 UTF-8 文本;父目录必须已存在 |
| `READ_FILE(path) -> STRING` | 读文件;不存在产生 `E0061` |
| `EXISTS(path) -> BOOLEAN` | 存在测试 |

### 10.9 构造器

`DICT() -> DICT`:空字典(唯一写法)。`ARRAY()`、`ARRAY(items...)` 是保留形式(第 12 章);非空数组用字面量(4.8)。

### 10.10 测试 — `wlwl:std.test`

| 签名 | 说明 |
|------|------|
| `TEST(name, body) -> NULL` | 注册零参测试体 |
| `ASSERT(cond, msg?)` | `cond` 为真 → `OK(TRUE)`;否则 `ERR(E0046)` |
| `ASSERT_EQ(a, b)` / `ASSERT_NEQ(a, b)` | 相等/不等断言,失败为 `E0047`/`E0048` |
| `EXPECT_ERR(x)` | `x` 为 `ERR` → `OK(载荷)`;否则 `ERR(E0049)` |
| `RUN_TESTS() -> ARRAY` | 调用所有已注册体,返回记录数组 `["name": ..., "passed": ..., "duration_ms": ..., "return_value"/"error": ...]` |

**约定**:测试体应当以 `ASSERT` 族断言收尾并以 `TRUE` 结束;体求值期间逃逸的 `ERR` 使该项 `passed = FALSE`。

### 10.11 AI 与代理

`wlwl:std.ai`(`TASK`、`TOOL`、`CALL_TOOL`、`MODEL`、`CONTEXT`)与 `wlwl:std.agent`(`TASK`)向语言模型与代理运行时提供入口。它们依赖外部服务,失败产生 `E0080`–`E0083`/`E0090`–`E0094`。其协议细节属于实现文档,不在本规范内。

---

## 11 诊断

### 11.1 结构

诊断是错误(`E`)或警告(`W`),带四位数字码。错误形式的 CLI 输出包含码、消息、位置(文件、行列)与源行摘录;`--format json`/`jsonl` 产生机器可读结构(含 `error_schema_version`、`errorCategory`、`retryable`、`idempotent`、`suggestion_code`、`trace` 等字段)。

### 11.2 错误码

| 码 | 含义 |
|------|------|
| `E0001` / `E0002` / `E0003` | 非法字符 / 未终止字符串 / 未终止块注释 |
| `E0010`–`E0013` | 语法错误(期望表达式、`)`、`,`、`;`) |
| `E0014` | `RETURN`/`BREAK`/`CONTINUE` 出现在非法位置 |
| `E0020` | 未定义名字;调用非函数;求值保留关键字 |
| `E0021` | 同作用域重复导入 |
| `E0022` | 函数元数不符 |
| `E0023` | 名字未被模块导出 |
| `E0024` | 对未捕获绑定 `SET` |
| `E0025` | 遮蔽内建名(未开启 `allow_builtin_shadow`) |
| `E0026` | 解构模式失配 |
| `E0030` | 类型错误(含 `ERR_PAYLOAD`/`WRAP`/`CALL` 的误用) |
| `E0031` | 下标/键类型错误;NaN 作键 |
| `E0032` | 属性/方法不存在于该类接收者 |
| `E0033` | `strict_types` 违例 |
| `E0034` | 对 `INTEGER` 下界取负 |
| `E0035` | 浮点转整数越界 |
| `E0036` | 数组索引越界 |
| `E0037` | 字典键不存在 |
| `E0038` | `RANGE` 步长为零 |
| `E0039` | `FORMAT` 模板畸形 |
| `E0040` / `E0041` | 模块不存在 / 循环导入 |
| `E0045` | 依赖约束不可满足(包管理) |
| `E0046`–`E0049` | 测试断言失败(`ASSERT`/`ASSERT_EQ`/`ASSERT_NEQ`/`EXPECT_ERR`) |
| `E0061` | 文件不存在 |
| `E0070` / `E0071` | JSON 解析 / 序列化失败 |
| `E0100` | PANIC(含对 `ERR` 调用 `UNWRAP`) |
| `E0102` | `ERR` 逃逸到顶层 |
| `E1003` | 除零/模零 |
| 其余注册码 | `E0032`/`E0050`/`E0051`(保留,面向对象)、`E0042`–`E0044`(清单与依赖)、`E0060`/`E0062`/`E0063`、`E0090`–`E0094`、`E0099`(IO/网络/AI 子系统)、`E0101`(栈溢出)随对应子系统定义 |

### 11.3 警告码

| 码 | 含义 |
|------|------|
| `W0010` | 未使用的 `LET` 绑定 |
| `W0011` | 未使用的函数形参( `_` 前缀可静默) |
| `W0014` | 非 ASCII 大小写转换行为未定 |
| `W0015` | 整数运算饱和 |
| `W0020` | 字面量混用裸值与键值对 |
| `W0030` | 遮蔽保留关键字 / 经许可遮蔽内建 |
| `W0040` | 注释中未处理的 `TODO(agent):` |
| `W0051` | 使用弃用别名(`DEL` 等) |
| `W0053` | 源文件偏离规范格式化器(§附录 A.3)输出 |
| `W0054` | 使用 `!` 弃用形式(应当用 `NOT`) |

警告**不得**改变程序语义;`check` 子命令与解析期诊断流会输出警告,运行期警告进入诊断流,由实现决定呈现时机。

### 11.4 退出码

| 退出码 | 含义 |
|--------|------|
| `0` | 成功 |
| `1` | 以诊断终止(语法错误、`E0100` PANIC、`E0102` 顶层 `ERR` 等) |
| `101` | 实现内部崩溃(非规范性;出现即属实现缺陷) |

---

## 12 保留形式

以下名字已被词法或注册表层保留,但本规范**不**定义其语义。对它们的调用产生 `E0020`(或按注所指的特定诊断),程序不得依赖其存在:

| 形式 | 现状 |
|------|------|
| `CLASS`、`NEW`、`THIS` | 面向对象构造保留;求值产生 `E0020` |
| `MODULE(name?, body)` | 显式模块名声明保留;求值产生 `E0020` |
| `MODULE_REF(path)` | 动态取模块字典保留 |
| `CALL(fn, args...)` | 保留;对用户闭包调用产生 `E0030`(动态分发专用路径) |
| `ARRAY(items...)` | 非空数组构造器保留;空数组用 `[]`,其余用字面量 |
| `AND`、`OR` | 逻辑函数的旧名保留;调用产生 `E0020`,用 `&&`、`||`(4.3) |

未来版本为这些形式赋予语义时**必须**整体修订本规范。

---

## 附录 A 文法(规范性)

### A.1 记法

产生式用 EBNF:`=` 定义;`|` 选择;`[ x ]` 可选;`{ x }` 重复;`"..."` 终结符。终结符与 1.4–1.5 的记号一致。

### A.2 语法

```
Program     = { ExprStmt } [ Expression ] .
Block       = "(" { ExprStmt } [ Expression ] ")" .
ExprStmt    = Expression ";" .

Expression  = LetExpr | FunExpr | IfExpr | WhileExpr | ForExpr | MatchExpr
            | TryExpr | ReturnExpr | BreakExpr | ContinueExpr
            | OkExpr | ErrExpr | PanicExpr
            | PostfixExpr .

LetExpr     = "LET" ( identifier | Pattern ) "," Expression .
FunExpr     = "FUN" [ identifier ] "(" [ Param { "," Param } ] ")" Expression .
IfExpr      = "IF" "(" Expression "," Expression [ "," Expression ] ")" .
WhileExpr   = "WHILE" "(" Expression "," Expression ")" .
ForExpr     = "FOR" "(" identifier "," Expression "," Expression ")" .
MatchExpr   = "MATCH" "(" Expression "," ClauseArray [ "," Expression ] ")" .
TryExpr     = "TRY" "(" Expression ")" .
ReturnExpr  = "RETURN" [ Expression ] .
BreakExpr   = "BREAK" "(" ")" .
ContinueExpr= "CONTINUE" "(" ")" .
OkExpr      = "OK" "(" Expression ")" .
ErrExpr     = "ERR" "(" Expression ")" .
PanicExpr   = "PANIC" "(" Expression ")" .

Param       = identifier [ ":" TypeRef ] [ "=" Expression ] | "*" identifier .
TypeRef     = identifier .

PostfixExpr = Primary { Postfix } .
Primary     = Literal | identifier | OperatorCall | identifier "(" [ Args ] ")"
            | "(" Block ")" | ArrayLit | DictLit .
OperatorCall= OpToken "(" [ Args ] ")" .
Postfix     = "." identifier [ "(" [ Args ] ")" ]
            | "[" Expression "]"
            | "[" Expression "]" "=" Expression .
Args        = Expression { "," Expression } .

ArrayLit    = "[" [ Expression { "," Expression } ] "]" .
DictLit     = "[" Key ":" Expression { "," Key ":" Expression } "]" .
Key         = Expression .

Literal     = int_lit | float_lit | string_lit | "TRUE" | "FALSE" | "NULL" .
OpToken     = "+" | "-" | "*" | "/" | "%" | "==" | "=" | "!=" | "<" | ">"
            | "<=" | ">=" | "&&" | "||" | "!" .

ClauseArray = "[" [ Clause { "," Clause } ] "]" .
Clause      = "[" Pattern "," Expression "]" .

Pattern     = identifier | "_" | Literal
            | ArrayPattern | DictPattern
            | "OK" "(" Pattern ")" | "ERR" "(" Pattern ")" .
ArrayPattern= "[" [ Pattern { "," Pattern } [ "," "*" identifier ] ] "]" .
DictPattern = "[" Expression ":" Pattern { "," Expression ":" Pattern } "]" .
```

**注**:`Block` 允许以一个无分号的表达式收尾,其值为括号序列的值;顶层 `Program` 通常写作若干 `ExprStmt`。`Postfix` 的三种形式可任意交错。

### A.3 规范格式化器

实现随附一个把程序重排为唯一规范形式的格式化器(不保留注释)。`--check` 模式对偏离规范形式者产生 `W0053`。规范形式的确切空白策略属于实现文档,不构成一致性要求。

---

## 附录 B 与早期草案的语义差异(非规范)

本规范以参考实现为准全新定义语言。与此前草案(v0.4 文本)相比,以下行为差异是**有意的**,以本规范为准:

1. **真值**:`0`、`0.0`、`""`、空容器、`NaN` 都是**真值**;假值仅 `FALSE` 与 `NULL`。
2. **逻辑不短路**:`&&`/`||` 两实参都求值;条件求值一律用 `IF`;`AND`/`OR` 名字是保留形式而非可调用函数。
3. **运算严格二元**:`+`、`-`、`*`、`/`、`%`、`CONCAT` 恰取两实参。
4. **容器非破坏性**:`PUSH`/`INDEX_SET`/`REMOVE_KEY`/`REVERSE` 等返回新容器,**不**修改接收者;更新绑定必须 `SET`(第 12 章之外的突变不存在;`POP` 是字典取值而非数组弹出)。
5. **`LET` 恒新建绑定**:内层 `LET` 遮蔽而非覆盖外层绑定(§3.2);循环累加须用捕获单元格或 `REDUCE`。
6. **`MATCH` 无默认且无命中**时值为 `NULL`(不报错)。
7. **`WHILE`/`FOR` 的值为 `NULL`**。
8. **字符串无下标**:`INDEX_GET` 不接受字符串,用 `SUB`/`SPLIT`/`CODEPOINTS`。
9. **相等跨数值类型**:`=(1, 1.0)` 为真。
10. **默认参数在词法帧内求值**、`*rest` 收集余参;元数检查为范围式。
11. **命名函数定义绑定名字**;`LET` 与命名定义可并存。
12. **`=`、`!` 为兼容拼写**:`NOT`/`==` 是正字,兼容形式分别携带 `W0054`/无警告。
13. **面向对象构造(`CLASS`/`NEW`/`THIS`)与显式模块名(`MODULE`)未定义**,列为保留形式(第 12 章)。

## 附录 C 参考实现

本规范的可执行参考实现随仓库提供:

```
wlwl.exe run <file>        # 求值
wlwl.exe check <file>      # 仅解析,输出诊断
wlwl.exe ast <file>        # 输出 AST JSON
wlwl.exe fmt <file>        # 输出规范形式;--check 仅校验(§A.3)
```

实现行为与本规范的任何出入,以实现仓库的偏差登记文档为准,并应在本规范的下一版本中消解。
