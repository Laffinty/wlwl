# WLWL v0.8.1 构建计划

> **状态**:草稿(本计划)
> **基线**:`docs/standard/wlwl-spec-v0.8.md`(v0.8.0)+ `impl/crates/`(已 v0.8 release tag)
> **不动**:`wlwl-spec-v0.8.md`(用户约束:v0.8.x 严格遵循 v0.8 spec,任何 spec 修订并入 v0.9)
> **驱动源**:`docs/AUDIT_REPORT.md`(2026-09-23,独立第三方实测)
> **立场**:**批判性吸收**。审计 26 条结论逐项核对源码;采纳与 spec 直接冲突的实现 bug;驳回与 spec 字面不矛盾的"误诊";搁置 spec 与实现均合理但文档含糊的灰色地带(进 deviations 登记,留待 v0.9)。

---

## 0 摘要

### 0.1 范围

v0.8.1 是 **patch 级修复**,相对 v0.8.0 严格向后兼容(以下两点均满足):

1. **无 spec 改动** — 全部修改在 impl 层;规范文本、附录 G、错误码注册表保持 v0.8 状态。
2. **无可观察行为扩展** — 修复的全是"实现偏离 spec"项,目标行为在 spec 中早已确定。

修复目标按优先级从高到低:

| # | 审计条目 | spec 引用 | impl 现状 | 优先级 |
|---|---------|----------|----------|------|
| F-A | §1.1 浮点指数字面量 | §1.7 EBNF `digits exponent` / `digits "." digits exponent` | `read_number` 不识别 `e`/`E[...]digits`,`1e2` 报 E0011 | **高** |
| F-B | §7.1 SUB 字符串下标"长度"语义 | §10.5 "自 start 起长 len 的子串" | `builtin_substr` 用 start/end 语义,`SUB(s, 7, 5)` 返回 `""` 而非 `"world"` | **高** |
| F-C | §1.2 `MUT` 作为绑定名 | §1.4 "其他位置可作普通标识符" | `LET(MUT, "x")` 在 binding 槽位报 E0010 | **高** |
| F-D | §4.4 字符串字面量下标 | §A.2 `PostfixExpr = Primary { Postfix }` + §4.5 prose | `parse_literal` 不调用 `apply_postfix_loop`,`"hi"[0]` 报 E0011 | 中 |
| F-E | §3.1 `closure_cell.wll` 示例与实现脱节 | §3.3 (LET 不可变标志固化) | 示例输出 `NULL` 而注释期望 `3` | 中 |
| F-F | §1.3 `${...("literal")...}` 内嵌字符串 | §A.2 grammar + 实测 | lexer `read_interp_body` 报 E0001;spec 未在 prose 显式禁止 | 低(进 deviations) |

**搁置**(审计结论与 spec 实测一致):§1.4 call-non-function 语法、§2.1 浮点除零 E1003、§2.4 `NaN` 不可达、§3.2 内建名作一等值、§5.1 类型注解默认开启(实际为默认忽略)、§6.1 整数溢出 `E0035`/除零 `E1003` 形式、§7.3 `EXPECT_ERR` 无法救回整数溢出、§8.1–8.4 wlwl-skill 路径(独立仓库)。

### 0.2 与 v0.8 的兼容承诺

v0.8.0 → v0.8.1 不应引入**新**的可观察行为,但会**纠正**若干偏离 spec 的实现 bug。这意味着:

- **正确性收益**:按 v0.8 spec 编写的代码,此前在 v0.8.0 上失败的 4 类输入(`1.5e2`、`SUB(s, n, len)`、`LET(MUT, …)`、`"hi"[0]`),在 v0.8.1 上将通过。
- **回归风险**:修复 `SUB` 语义后,凡是把 `SUB` 当作 start/end 使用的旧程序都会失配 — 但 spec §10.5 的字面承诺从 v0.6 一直是 start/len,v0.8.0 实测不一致是已知问题(闭包 capture 升级同一目录内的 §3.3 deviation),且 spec 已经在 v0.6 → v0.7 → v0.8 一路强化"SUB 是 len"。命中此回归面(如有)的用户应改用 `SUB(s, start, SUB(end, start, 0))` 或 `SLICE(s, start, start+len)`。
- **`read_number` 浮点路径冲击面**:只增不破 — 凡是用纯整数 / `digits "." digits` 字面量的 v0.8 程序继续按原状工作。

### 0.3 推进原则

按用户偏好:**一轮一修,本地 push,显式确认后才触发自动打包**(release workflow);不得新建分支;不写自动同步脚本。本计划只列修复目标 + deviation 登记,实际修复按 F-A → F-F 顺序逐项落地,每项独立的 commit + regression test。

---

## 1 现状盘点(逐项核对审计)

### 1.1 F-A · §1.1 浮点指数字面量

**审计原文**: `1.5e2`、`1e2`、`1.5e-2` 报 `E0011 expected ')', got Ident("e2")`。

**核实**:

```
$ wlwl.exe run test_audit/01_float_exp.wll
error[E0011]: expected ')', got Ident("e2")
  --> test_audit/01_float_exp.wll:2:12
```

**spec 引用** (`docs/standard/wlwl-spec-v0.8.md:112-119`):

```
float_lit      = digits "." digits | digits exponent | digits "." digits exponent .
exponent       = ( "e" | "E" ) [ "+" | "-" ] digits .
```

spec 显式接受三种 float 字面量形态,其中两种含 exponent。

**impl 引用** (`impl/crates/wlwl-lexer/src/lib.rs:289-310`):

```rust
// 读裸数字
while let Some(b) = self.peek() {
    if b.is_ascii_digit() { self.bump(); } else { break; }
}
// 可选的小数部分
if self.peek() == Some(b'.') && matches!(self.peek_at(1), Some(b) if b.is_ascii_digit()) {
    is_float = true;
    self.bump();
    while /* digits */ { ... }
}
```

`read_number` 只识别 `digits "." digits`,**完全跳过** exponent 段。`e`/`E` 被随后的 `read_ident_or_keyword` 当作标识符首字符读出,与数字字面量分离。

**结论**:**真 bug** — impl 不实现 spec 显式承诺。

**修复方案**:

- 在小数部分读完后,检查 `( 'e' | 'E' ) [ '+' | '-' ] digits`。
- 通过 lexer 一次性产出 `Float(v)` token(现有 token kind),不再把 `e2` 当作独立 ident 弹出。
- 边界:`1e` / `1e+` 后无 digits → lexer 仍按裸整数 `1` 处理 + emit diagnostic(`E0001 invalid float`)或维持 `1 ; e` 两 token(参考 `digits` 字面量在非数字后停下行为)— 选 E0001 更符合 "float 字面量不完整" 语义,但需文案清晰。
- 测试增补:`lexer/tests/` 加四例 `1e2 → 100.0`、`1.5e2 → 150.0`、`1.5e-2 → 0.015`、`1E3 → 1000.0`;`eval/src/lib.rs` 端到端 `LET(x, 1.5e2); PRINT(x);` 输出 `150.0`。

### 1.2 F-B · §7.1 SUB 字符串下标"长度"语义

**审计原文**:

```
SUB("Hello, world", 7, 1)  → ""          BUG (expect "w")
SUB("Hello, world", 7, 3)  → ""          BUG
SUB("Hello, world", 7, 5)  → ""          BUG
```

**核实**:

```
$ wlwl.exe run test_audit/02_sub_bug.wll
Hello
ello

                ← (空行;SUB("Hello, world", 7, *) 全空)
Hello, world
```

**spec 引用** (`docs/standard/wlwl-spec-v0.8.md:649-650`):

```
| `SUB(s, start, len?) -> STRING` | 自 `start` 起长 `len` 的子串;缺省 `len` 到末尾;按码点 |
```

**impl 引用** (`impl/crates/wlwl-eval/src/lib.rs:2067-2125`):

```rust
let norm_end = if end_raw < 0 {
    (end_raw + len).max(0)
} else {
    end_raw.min(len)
};
if norm_end <= norm_start {
    return Ok(Outcome::normal(Value::String(String::new())));
}
// chars: Vec<char> = s.chars().collect();
Ok(Outcome::normal(Value::String(chars[start..end].iter().collect())))
```

实现将第三个参数当作"end"(而非"len"),当 `end < start` 时直接返回空串。doc 注释 (`lib.rs:2067-2068`) 也写"start/end INTEGER; end 缺省切到末尾; 负数从尾数 (类似 SLICE)" — 与 spec §10.5 不一致。

**结论**:**真 bug** — impl 与 spec 直接冲突。

**修复方案**:

- 把第二个参数从 "end" 改读为 "len":
  - `len_raw < 0` → 钳到 0。
  - `len_raw == 0` → 直接返回空串(无需访问 chars 数组)。
  - `len_raw > 0` → 起点 `start`,终点 `start + len`,但要先按 SLICE 的范围裁剪再应用长度 — 与 SLICE 并存的语义是 `SUB(s, start, len) = SLICE(s, start, start+len)`,负 `len` 等价于 `0`。
  - 缺省(2 元) → `len = len_total - start`(到末尾)。
  - 越界 `start` → 钳到 `[0, len_total]`(与现有 `start.min(len)` 一致)。
- 更新 doc 注释为"start/len INTEGER; len 缺省切到末尾;负 len 视为 0"。
- 测试增补:`SUB("Hello", 0, 5) = "Hello"`、`SUB("Hello", 1, 3) = "ell"`、`SUB("Hello", 7, 5) = ""`(原 len 越界)、`SUB("Hello", -1, 1) = ""`(负 len)、`SUB("Hello", 0) = "Hello"`。注册表 `BuiltinSpec.signature` `SUB` 条目已写 `SUB(s, start, len?)`,metadata 正确;dispatch 路径不变。
- **回归风险**:无 — 这是把 impl 对齐 spec,不是反过来。

### 1.3 F-C · §1.2 `MUT` 作为绑定名

**审计原文**:

| 源码 | 实测结果 |
|------|---------|
| `LET(MUT, "x")` | `E0010 expected identifier in LET, got Mut` |
| `LET MUT(MUT, "x")` | 同上 |

**核实**:

```
$ wlwl.exe run test_audit/03_mut_ident.wll
error[E0010]: expected identifier in LET, got Mut
  --> test_audit/03_mut_ident.wll:2:5
  2 | LET(MUT, 99);
```

**spec 引用** (`docs/standard/wlwl-spec-v0.8.md:81`):

> `MUT` 是**上下文关键字**:仅在 `LET` 之后的位置具有特殊含义(3.1);其他位置可作普通标识符使用,不与关键字冲突。

**impl 引用** (`impl/crates/wlwl-parser/src/lib.rs:818-825, 827-841`):

```rust
// v0.6 §3.1: optional `MUT` keyword between `LET` and `(`.
let is_mut = if let TokenKind::Mut = self.peek() {
    self.advance();
    true
} else {
    false
};
self.expect_specific(EC::E0011, "'('")?;
match self.peek().clone() {
    TokenKind::LBracket | TokenKind::Ident(_) => { /* pattern form */ }
    _ => return Err("expected identifier in LET, got {:?}", ...)
}
```

`is_mut` 消耗 modifier slot 后,`LBracket | Ident(_)` 才接受 binding-name;`TokenKind::Mut` 不在白名单 → 报 E0010。

**结论**:**真 bug** — spec §1.4 说"其他位置可作普通标识符",**LET 内的 binding slot 正是"非 modifier slot"的位置**,应该接受 `TokenKind::Mut` 当作 `Ident("MUT")`。

**修复方案**:

- 在 `parse_pattern` 或 `parse_let` 的 binding-name 槽位额外接受 `TokenKind::Mut`,把它作为 `Ident("MUT")` 进入 pattern(不消耗 modifier slot 的 `MUT` 已被前置消费 — 真正绑定名位置的 `MUT` 是第二个)。
- 文案调整:`got Mut` → `got MUT`(`TokenKind::Mut` 的 `Debug` 默认输出 `Mut`,改用 token display)。
- 测试增补:
  - `LET(MUT, 99)` → `MUT = 99`(then `PRINT(MUT)` → `99`)
  - `LET MUT(MUT, 1); LET(MUT, +(MUT, 1)); PRINT(MUT)` → `2`
  - 反向保险:`LET MUT([MUT], 99)` 仍报 `LET MUT` 不接受解构模式(原逻辑不变)。

### 1.4 F-D · §4.4 字符串字面量下标

**审计原文**: `LET(_x, "hi"[5]);` 报 `E0011 expected ')', got LBracket`。

**核实**:

```
$ wlwl.exe run test_audit/10_str_subscript_in_let.wll
error[E0011]: expected ')', got LBracket
  --> test_audit/10_str_subscript_in_let.wll:2:13
  2 | LET(_x, "hi"[0]);
```

顶层 `[1, 2, 3][0]` 工作(D8-004 修复),但 `LET(_x, "hi"[0])` 不工作。

**spec 引用**:

- §4.5 (line 313-323) prose: "下标链可挂在变量、调用、属性访问、**或数组/字典字面量之后**" — 字面要求只挂数组/字典 literal,没列 string literal。
- §A.2 grammar (line 1030-1036):

```
PostfixExpr = Primary { Postfix } .
Primary     = Literal | identifier | ...
Postfix     = "." identifier | "[" Expression "]" | ...
```

`Literal` 是 Primary,理应进入 PostfixExpr,但 prose 把"具体哪些 Literal"限制到了 array/dict。

- §4.5 (line 321): "字符串接受整数索引,按码点返回单字符字符串,越界产生 E0036" — INDEX_GET 接受 string receiver,在 eval 端语义完整。

**impl 引用** (`impl/crates/wlwl-parser/src/lib.rs:2055-2066`):

```rust
let lit = match t.kind {
    TokenKind::Integer(v) => Literal::Integer(v),
    TokenKind::Float(v) => Literal::Float(v),
    TokenKind::StringLit(s) => Literal::String(s),  // ← 直接 return,无 apply_postfix_loop
    /* ... */
};
Ok(Expr::Literal(lit, ...))  // 没有 postfix loop
```

`parse_literal` 对所有非数组/字典/插值字符串字面量直接 `Expr::Literal(...)` 返回,**没有调用 `apply_postfix_loop`**(后者只接 array/dict 字面量 — `parse_array_or_dict` line 2264 — 与 identifier/orop 头)。string 字面量也是 `Literal`,按 §A.2 grammar 应能接 Postfix。

**结论**:**真 bug**(按 §A.2 grammar 字面读法,string Literal 是 Primary,可挂 postfix)。

**修复方案**:

- 在 `parse_literal` 返回前判断:`is_float / is_int / is_bool / is_null` 不允许 postfix(语义无意义);`is_string` 允许 postfix(eval 端 INDEX_GET 已实现)。
- 实现:把 `parse_literal` 末尾的 `Ok(Expr::Literal(...))` 替换为对 `apply_postfix_loop` 的调用(仅当字面量是 StringLit 时,或更宽松:任何字面量,parse-time 过滤;但只 StringLit 真允许)。
- 测试增补:`"hi"[0] = "h"`(端到端)、`PRINT("hi"[1]) = "i"`、`"hello"[5] = ""`(越界给 E0036,不被 parse 拒绝)。

**注意**:此修复扩大字面 postfix 语义,超出当前 §4.5 prose(只列 array/dict)。但 §A.2 grammar 与 §4.5 string INDEX_GET 联合支持,且与 README/示例的"自然字符串下标"习惯一致。如果 spec 作者更愿意 prose "字面"跟踪,本项应在 v0.9 spec 增补;deviation 仍登记 D8-010。

### 1.5 F-E · §3.1 `examples/closure_cell.wll` 与 §3.3 不一致

**审计原文**: 示例注释期望 `3`,实测输出 `NULL`。

**核实**:

```
$ wlwl.exe run examples/closure_cell.wll
NULL
```

**spec 引用** (`docs/standard/wlwl-spec-v0.8.md:238-254`):

> 单元格可变标志由绑定形式决定:`LET` 建立的为不可变,`LET MUT` 建立的为可变。该标志一经设定**不再改变**。

**impl 文件** (`impl/examples/closure_cell.wll:5-10`):

```wlwl
LET(n, 0);
LET(step, FUN((), LET(n, +(n, 1))));
LET(c, step()); LET(c, step()); LET(c, step());
PRINT(c);   // 3
```

按 §3.3:`LET(n, 0)` 不可变;函数体内 `LET(n, +(n, 1))` 是局部 shadow,而非 mutate;函数 return 是 `LET(...)` 的值,即 `NULL`(§3.1: "LET 表达式本身的值是 NULL")。与 §3.3 "标志不再改变" 的 strict 读法一致;实测 `NULL`。注释期望 `3` 与当前 v0.7/v0.8 语义不兼容。

**README 对照** (`README.md:13-19`):

```wlwl
LET MUT(counter, 0);
LET(step, FUN((), (
    SET(counter, +(counter, 1));
    counter
)));
PRINT(step());   // 1
PRINT(step());   // 2
```

README 用 `LET MUT + SET`,与 §3.3 一致,实测下确认输出 `1\n2\n`。**这个示例是正确的;`closure_cell.wll` 应改写。**

**结论**:示例与 spec + impl 脱节。

**修复方案**:

- 把 `closure_cell.wll` 全文改写为 `LET MUT + SET` 版本,与 README 同步。函数体改成 `(SET(n, +(n, 1)); n)` 块表达,以 `n` 作为返回值。
- 同步更新该示例的注释,引用 §3.3 + 3.4,指明 `LET MUT` 是 v0.6+ 显式可变性的唯一路径。
- 测试增补:`impl/examples/conformance_smoke/` 加 `closure_cell_mut_smoke.wll`,断言 `step()` 调用 3 次后 `c = 3`。

**注意**:这是 documentation/example 修复,**不是 impl 修复**;不会引入回归。但 `cargo test --workspace` 跑示例没有强制断言(impl/examples 没有 CI 验证),需要补一条 conformance 测试。

### 1.6 F-F · §1.3 `${...(... "..." ...)}` 内嵌字符串字面量

**审计原文**: lexer 在 `${...}` 体内遇到 `"` 即报 `E0001 nested string literal inside ${...} interpolation is not allowed`。

**核实**(lexer lib.rs:633-643):

```rust
b'"' => {
    return Err(self.err(
        ErrorCode::E0001,
        "nested string literal inside `${...}` interpolation is not allowed",
        self.line, self.col,
    ));
}
```

**spec 引用**: §1.8 与 §A.2 grammar **均未显式禁止**内嵌 string literal,只是 §A.2 末尾注 "为避免歧义,实现通常在词法阶段以括号配对定位插值边界" — 这是 implementation hint,不是 normative 禁止。

**结论**:**灰色地带** — 审计意见(spec 应补禁止)与"实现以 brace 配对简化"的现实假设均合理。修复成本:lexer 改为递归 lex `${...("hi")...}` 内的字符串需要重写 `read_interp_body` 的 scope 跟踪,工程量大且需新增锁测试验证嵌套深度匹配。短中期建议:**不进 v0.8.1**,登记 D8-011 进 deviations,留待 v0.9 spec 加 normative note 后再实现。

**修复方案**:无(留 deviations)。

---

## 2 设计优化(impl / parser / lexer / docs/examples)

### 2.1 修复 F-A · 浮点指数字面量

**改动**:`impl/crates/wlwl-lexer/src/lib.rs::read_number` (line 278-336) 在小数部分消费完后,加入 exponent 读路径:

```rust
let mut is_float = false;
if self.peek() == Some(b'.') && matches!(self.peek_at(1), Some(b) if b.is_ascii_digit()) {
    is_float = true;
    /* ... digits after '.' ... */
}
// NEW (F-A): exponent segment
if is_float && matches!(self.peek(), Some(b'e') | Some(b'E')) {
    is_float = true;
    self.bump(); // 'e' | 'E'
    if matches!(self.peek(), Some(b'+') | Some(b'-')) {
        self.bump();
    }
    let exp_start = self.pos;
    while let Some(b) = self.peek() {
        if b.is_ascii_digit() { self.bump(); } else { break; }
    }
    if self.pos == exp_start {
        // no digits after exponent marker — emit E0001
        return Err(self.err(
            ErrorCode::E0001,
            format!("invalid float exponent in '{}'", /* ... */),
            line, col,
        ));
    }
}
```

随后 `text.parse::<f64>()` 已经能解析 `"1.5e2"`、`"1e-2"`、`"1E3"` — 现有 `Float(v)` token 不变。

**锁测试影响**:无新增。已有 `lexer/tests.rs` 数字部分需保持兼容。
**deviation 登记**:`D8-009 · 浮点指数字面量按 spec §1.7 启用(v0.8.0 漏实现)`。
**测试增补**:

- `lexer/tests/lex_numbers.rs`(或同名模块):
  - `lex_1e2_yields_float_100`: `1e2` → `[(Float(100.0),)]`
  - `lex_1_5e2_yields_float_150`: `1.5e2` → `[(Float(150.0),)]`
  - `lex_1_5e_neg_2_yields_float_0_015`: `1.5e-2` → `[(Float(0.015),)]`
  - `lex_1E3_uppercase_e`: 同 1e3 路径
  - `lex_bare_e_is_ident_when_not_after_digits`: `LET(e, 1)`(e 是 ident,沿用)
  - `lex_1e_without_digits_errors_e0001`: `1e ;` → `E0001 invalid float`
- `eval/src/lib.rs::tests`:
  - `eval_floating_exponent_literals`: `LET(_x, 1.5e2); PRINT(_x)` → `150.0`
  - `eval_negative_exponent`: `LET(_x, 1e-3); PRINT(_x)` → `0.001`

### 2.2 修复 F-B · SUB 第三参数为长度

**改动**:`impl/crates/wlwl-eval/src/lib.rs::builtin_substr` (line 2067-2125) 将 `end_raw` 改为 `len_raw`:

```rust
let start_raw: i64 = /* as before */;
let len_raw: i64 = if args.len() == 3 {
    /* type-check as before, but against `len` */
    /* 语义:
       - len_raw < 0       → 0
       - len_raw == 0      → ""
       - len_raw > 0       → chars[norm_start .. min(norm_start + len_raw, len_total)]
       - start 越界         → 与 SLICE 一致,clamp 到 [0, len_total]
    */
} else {
    len - norm_start   // 缺省 = 到末尾
};
```

具体行为(对应测试矩阵):

| `SUB(s, start, len)` | 旧实现(当前) | 新实现(目标) | spec 期望 |
|---------------------|-------------|-------------|----------|
| `("Hello, world", 0, 5)` | `"Hello"` | `"Hello"` | `"Hello"` |
| `("Hello, world", 1, 5)` | `"ello"` | `"ello,"`(start=1, len=5→chars[1..6], 5 个码点) | `"ello,"`(len semantics) |
| `("Hello, world", 7, 1)` | `""` | `"w"` | `"w"` |
| `("Hello, world", 7, 3)` | `""` | `"wor"` | `"wor"` |
| `("Hello, world", 7, 5)` | `""` | `"world"` | `"world"` |
| `("Hello, world", 0)` | `"Hello, world"` | `"Hello, world"` | `"Hello, world"` |
| `("Hello", -1, 1)` | `""`(end=1 ≤ start norm 4 → 空;新:start=4, len=1 → chars[4..5]="o") | `"o"` | `"o"` |
| `("Hello", 0, -1)` | n/a(旧把 -1 当 end;新:负 len clamp 到 0) | `""` | `""` |

**锁测试影响**:既有 `substr_*` 测试如用 2 参调用,继续通过;3 参调用如隐含 end-语义,需同步改测试(以 spec §10.5 为准)。
**deviation 登记**:`D8-010 · SUB(s, start, len?) 第三参数从 end 改读为 length,符合 spec §10.5`。
**测试增补**:

- 改写并补全 `substr_*` 系列测试覆盖上述 8 行表。
- 端到端 smoke:`impl/examples/conformance_smoke/substr_length_smoke.wll` 中跑标准 8 行。
- 与 `SLICE` 比对:`SUB(s, start, len) = SLICE(s, start, start + len)`(前提 `start + len ≤ LEN(s)`,否则 SLICE 钳到末尾);可加 `test_substr_matches_slice_truncated` 双路径一致性测试。

**注意**:这是 **唯一会改变 v0.8 程序可观察行为** 的修复。理由清单:

- spec §10.5 在 v0.6→v0.7→v0.8 一路是 "len" 措辞。
- 当前 impl 在 v0.8.0 例外是"实现落后于 spec",不是 spec 变更。
- 用户代码如依赖当前"end"语义,迁移路径:`SUB(s, start, end_old)` → `SUB(s, start, -(start, end_old))` 或 `SLICE(s, start, end_old)`。

CHANGELOG v0.8.1 中标注:`"§10.5 SUB 第三参数严格按 length 实现,旧 end 语义用户需改用 SLICE"`。

### 2.3 修复 F-C · `MUT` 作为绑定名

**改动**:`impl/crates/wlwl-parser/src/lib.rs::parse_let` (line 818-892) 在 `TokenKind::Mut` modifier slot 消费后,把 binding-name 槽位的 `TokenKind::LBracket | TokenKind::Ident(_)` 改写为接受 `TokenKind::Mut` 也接受;在 `Pattern` 构造时把 `TokenKind::Mut` 当作 `Ident("MUT")` 处理。

两种实现位置任选其一:

(A) `parse_let` 内联:在 `match self.peek().clone()` 的 arm 里加入 `TokenKind::Mut`,把它转为 `Ident("MUT")` 后进入 `parse_pattern`。

(B) `parse_pattern` (line 911) 把 `Ident(s)` arm 扩为:`if matches!(self.peek(), TokenKind::Ident(s) | TokenKind::Mut) → Pattern::Ident(Ident_or_Mut, ...)`,把 Mut 当 Ident 用。

(A) 更窄(只影响 LET),(B) 更广(影响所有 pattern)。**推荐 (A)**,因为这是 spec §1.4 的"上下文关键字"语义 — pattern position 是另一种上下文,不该一并放宽。

**锁测试影响**:既有 `let_mut_*` 测试不受影响(MUT 仍可作 modifier)。
**deviation 登记**:`D8-011 · LET binding 槽位接受 MUT 作普通标识符,符合 spec §1.4 上文关键字语义`。
**测试增补**:

- `parser/tests/spec_v3_alignment.rs`:
  - `let_paren_let_mut_as_binding_name_roundtrips`: `LET(MUT, 99)` → `Expr::Let { name: "MUT", mut_: false, ... }`
  - `let_double_mut_first_modifier_second_binding`: `LET MUT(MUT, 99)` → `Expr::Let { name: "MUT", mut_: true, ... }`
  - `let_mut_as_binding_does_not_shadow_built_in`: 修改后 `MUT` 在显式 LET 中作为 binding 名是合法的;`E0025 shadowing 内建名`规则是否触发 — 见 spec §3.5,`MUT` 不在内建注册表的 110 条之列(prose 强调"MUT 不与关键字冲突"),所以不触发 E0025。
- 端到端 smoke:`impl/examples/conformance_smoke/mut_as_binding_smoke.wll`:
  - `LET(MUT, "x"); PRINT(MUT);` → `x`
  - `LET MUT(MUT, 1); PRINT(MUT);` → `1`

### 2.4 修复 F-D · 字符串字面量下标

**改动**:`impl/crates/wlwl-parser/src/lib.rs::parse_literal` (line 2029-2066) 末尾,当字面量是 `TokenKind::StringLit(s)` 时,把生成的 `Expr::Literal(Literal::String(s), ...)` 通过 `apply_postfix_loop` 一次。

```rust
let mut base: Expr = Expr::Literal(
    Literal::String(s.clone()),
    Span { /* ... */ }
);
// v0.8.1 §F-D (D8-012): string literal subscripts allowed per §A.2 grammar.
base = self.apply_postfix_loop(base, line, col)?;
Ok(base)
```

其他字面量(Integer / Float / Boolean / Null)目前不允许 postfix(数字下标无意义) — 维持原状。

**注意**:array/dict literal 已经在 `parse_array_or_dict` 末尾接入 `apply_postfix_loop`(line 2264),本修复与之平行。

**锁测试影响**:无新增。
**deviation 登记**:`D8-012 · 字符串字面量允许下标链 postfix,符合 §A.2 grammar;prose §4.5 仅列 array/dict 时举例,非穷举`。
**测试增补**:

- `parser/tests/spec_v3_alignment.rs`:
  - `parser_string_literal_subscript_roundtrip`: `"hi"[0]` → `Expr::Index { base: Literal("hi"), index: Literal(0) }`
  - `parser_string_literal_subscript_in_let_slot`: `LET(_x, "hi"[0])` 不报 E0011
- `eval/src/lib.rs::tests`:
  - `eval_string_literal_subscript_h`: `PRINT("hi"[0])` → `h`
  - `eval_string_literal_subscript_out_of_range`: `PRINT("hi"[5])` → E0036(由 INDEX_GET 触发,不在 parser)
- smoke:`impl/examples/conformance_smoke/string_subscript_smoke.wll`

### 2.5 修复 F-E · `closure_cell.wll` 与 spec 对齐

**改动**:`impl/examples/closure_cell.wll` 全文改写:

```wlwl
// examples/closure_cell.wll — closure cell-capture semantics (§3.3, §3.4)
// Demonstrates: a closure capturing a `LET MUT` cell and accumulating
// state across invocations.

LET MUT(n, 0);
LET(step, FUN((), (SET(n, +(n, 1)); n)));
LET(c, step()); LET(c, step()); LET(c, step());
PRINT(c);   // 3
```

更新顶部 doc 注释引用 §3.3 (cell mutability flag is permanent) 与 §3.4 (SET)。

**这是 example 修复,不是 impl 修复。**

**deviation 登记**:`D8-013 · examples/closure_cell.wll 对齐 spec §3.3(原样按 §3.3 strict 读法输出 NULL,与 README 冲突;v0.8.1 改写为 LET MUT 版本,与 §3.3 + README + 实测 1/2/3 一致)`。
**测试增补**:在 `impl/tests/conformance/` 或新建 `impl/examples/conformance_smoke/closure_cell_mut_smoke.wll`,assert `step()` 三次后 `c = 3`。`cargo test --workspace` 中加入此 smoke 路径。

---

## 3 文档优化

### 3.1 deviations 同步登记

`docs/plan/deviations.md`(待新建,或沿用现有 `docs/history/deviations-v0.8.md` 作为 v0.8.1 增补分册)新增五条:

- **D8-009 · 浮点指数字面量启用 (§1.7)**(impl 修复 commit `xxxxxxx`)
- **D8-010 · SUB(s, start, len?) 第三参数严格按 length 实现 (§10.5)**(impl 修复 commit `xxxxxxx`,**唯一会改变可观察行为**)
- **D8-011 · MUT 作为 LET binding 名允许 (§1.4)**(impl 修复 commit `xxxxxxx`)
- **D8-012 · 字符串字面量允许下标 postfix (§A.2 grammar;§4.5 prose 跟进留 v0.9)**(impl 修复 commit `xxxxxxx`)
- **D8-013 · examples/closure_cell.wll 与 README + spec §3.3 对齐**(docs 修复 commit `xxxxxxx`)
- **D8-014 · §1.8 内嵌字符串字面量在 `${...}` 内不允许 — 当前 impl 与 spec prose 同处理,补 deviations 备忘**(留 v0.9 spec 增补 normative note)

### 3.2 CHANGELOG v0.8.1 节

```markdown
## [v0.8.1] — 2026-09-XX

Spec: **wlwl-spec-v0.8**(无 spec 变更)。

### Fixed (per third-party AUDIT_REPORT.md 2026-09-23)

- **§1.7 浮点指数字面量启用**(D8-009):`1e2`、`1.5e2`、`1.5e-2`、`1E3` 等符合
  spec §1.7 EBNF `digits exponent` / `digits "." digits exponent` 三态的字面量现可识别。
- **§10.5 SUB 严格 length 语义**(D8-010,**observable change**):
  `SUB(s, start, len?)` 第三参数严格按"长度"读,与 spec §10.5 自 v0.6 一致;
  旧 v0.8.0 实现把第三参数当 end-index,本修复纠正。
  迁移:`SUB(s, start, end_old)` 改 `SUB(s, start, -(start, end_old))` 或
  `SLICE(s, start, end_old)`。
- **§1.4 MUT 作 LET binding 名**(D8-011):`LET(MUT, "x")` 现允许,
  `LET MUT(MUT, x)` 第一 MUT 作 modifier、第二 MUT 作 binding 名。
- **§A.2 grammar 字符串字面量下标**(D8-012):`"hi"[0]`、`LET(x, "hi"[1])`
  现可 parse,通过现有 INDEX_GET 求值。
- **examples/closure_cell.wll 对齐 §3.3 + README**(D8-013):示例改写为
  `LET MUT + SET` 版本,与实测输出 1/2/3 一致。

### NOT changed (audit findings reviewed and rejected as misdiagnosis)

按 spec v0.8 字面文本:
- §1.4 `(42)(1, 2)` 在 parser 阶段拒绝 — spec §4.2 grammar 仅允许
  `identifier "(" args ")"` 形态,`expr(args)` 无对应产生式。
- §2.1 浮点除零返 E1003 — spec §2.2 显式 "整数、浮点皆然,即使 IEEE 754 允许 Inf"。
- §5.1 类型注解默认严格 — impl 默认 strict_types = false(`wlwl-toml/src/manifest.rs:361`),
  §11.2 确认 E0030 与 strict_types(E0033)是两个独立码,用户的 E0030 实测
  来源是 `*` 体内的运行时类型错,不是边界注解触发。
- §6.1 / §7.3 整数溢出 / 除零 / NEG(MIN) 形式为原生诊断 E0035 / E1003,
  而非 ERR — spec §11.2 与 §2.2 措辞"抛错"指 native code(11.4 退出码 1),
  不指 ERR 包装;不放入 §8.3 消费者接住路径。
- §3.2 内建(`+`, `PRINT` 等)不可作一等值 — spec §5.4 措辞适用 user-defined function;
  内建由 §4.3 "运算符即函数" grammar 规约(operator-token call form),无对应 retrieve-as-value 路径。

### Adds (consistency tests)

- 5 个 lexer exponent round-trip test(`lex_1e2_yields_float_100` 等)。
- 5 个 eval exponent end-to-end test。
- 8 个 builtin_substr length-semantics test(对照表行 1-8)。
- 3 个 parser MUT-as-binding-name round-trip test。
- 2 个 eval MUT-as-binding-name end-to-end。
- 4 个 string literal subscript(round-trip + eval smoke)。
- 2 个 closure_cell_mut smoke test。
- `cargo test --workspace` ~1366 + ~25 项全绿。

### Compatible with v0.8.0 except

唯一 observable-change 项:§10.5 SUB 语义(D8-010)。其余四项补全 spec
已显式承诺但 impl 未达成的形态,不破任何已工作代码。
```

### 3.3 wlwl-skill 同步(独立维度)

`wlwl-skill/SKILL.md` frontmatter 当前指 `wlwl-spec-v0.7`。审计 §8.1 指出 skill 与本版 spec 错位。本计划**不在 v0.8.1 范围内强修** — skill 是独立仓库,由其自身节奏升级。`wlwl-skill/CHANGELOG.md` 在 skill 仓侧补一段 "v0.8 + v0.8.1 spec 字面对应关系"备忘,与本仓解耦。

### 3.4 不动 spec(`wlwl-spec-v0.8.md`)

按用户约束,v0.8.x 严格遵循 v0.8 spec。下列在 audit 中浮现但与 spec 实测一致的灰色地带留给 v0.9:

- §1.8 / §A.2: `${...("...")}` 内嵌 string literal 规范化(d-D8-014)
- §2.4 / §8.3 边角:ERR 操作数位置的 `==` 一致性承诺(优先 §2.4 或 §8.3?)
- §2.5 空 DICT 显示形态(空 dict vs 空 array)
- §10.11 `wlwl:std.ai.TASK` 全限定写法支持
- §10.7 FORMAT `{N}` / `*arr` 显式 splat 可读性改进

---

## 4 验收

### 4.1 锁测试矩阵(提交前必跑)

| 测试名 | 守住什么 | v0.8.1 改动后影响 |
|--------|---------|------------------|
| `b11_registry_count_matches_spec_table` | 总条目数 = 110 | 不变 |
| `b11_registry_covers_resolve_builtin` | 每个 resolve 名都在 registry | 不变 |
| `b11_resolve_builtin_covers_registry` | 反向 | 不变 |
| `b11_err_consumer_registry_consistent` | ERR consumer 集合一致 | 不变(SUB 不是 consumer) |
| `b11_macro_fn_attribute_matches_dispatch` | macro_fn 一致 | 不变 |
| `b9_*` 系列 | E 码归属 | 不变 |
| `pop_dict_*` 七件套 | POP 三元 DICT 语义 | 不变 |
| `literal_subscript_*` 八件套(D8-004) | 字面下标 | 加 string 字面量(D8-012) |
| **新增 `substr_length_*` 八件套** | SUB length 语义 | D8-010 |
| **新增 `lex_floating_exponent_*` 五件套** | lexer 指数 | D8-009 |
| **新增 `let_mut_as_binding_*` 五件套** | MUT 作 identifier | D8-011 |
| **新增 `closure_cell_mut_smoke_*`** | example | D8-013 |

### 4.2 一致性快照比对

跑 `cargo run --bin gen-appendix-g -- ../docs/appendix_G.md`(不动 entry,只重生成),diff 与上次 v0.8.0 release 应**零字节差异**。改 registry 没动 spec。

### 4.3 运行时差分基线

`cargo run --bin wlwl -- run` 跑 `impl/examples/` 全 12 + `impl/tests/conformance/` 全套 + 新加的 4 个 smoke,全绿。SUB 修复后,凡用 3-参 SUB 的旧 conformance test 必须以新语义重写(归 2.2 节)。

### 4.4 CHANGELOG 与 deviations 同步

本次六条偏差(D8-009..D8-014)入 `deviations.md`(或并入 `deviations-v0.8.md` 增补),CHANGELOG v0.8.1 节落定。

---

## 5 实施排期与原则

按用户"一轮一修,本地 push,显式确认后才触发打包"的工作约束:

| 步 | 内容 | commit | regression test | 用户确认闸 |
|----|------|--------|----------------|----------|
| 1 | F-A 浮点指数字面量 | D8-009 commit | `lex_*e2*` + `eval_*exponent*` | 是 |
| 2 | F-B SUB length 语义(**唯一 observable change**) | D8-010 commit | `substr_length_*` 8 件 | 是 |
| 3 | F-C MUT 作 binding | D8-011 commit | `let_mut_as_binding_*` | 是 |
| 4 | F-D 字符串字面量下标 | D8-012 commit | `parser_string_literal_subscript_*` + `eval_string_literal_subscript_*` | 是 |
| 5 | F-E closure_cell.wll 改写 | D8-013 commit | `closure_cell_mut_smoke_*` | 是 |
| 6 | F-F documentation-only deviation 记录 | D8-014 commit | 无(纯备忘) | 是 |

每步独立 commit + regression test 全绿后等用户视觉确认 badge / conformance 输出 → 才允许下一步。

---

## 6 不在范围(明确剔除)

- v0.8 spec 任何章节修订 — 用户约束。
- wlwl-skill 升级 — 独立仓库。
- §10.11 全限定 TASK 写法支持、§10.7 FORMAT splat、§2.5 空 DICT 显示 — 留 v0.9 议程。
- §6.1 / §7.3 整数溢出改 ERR 形式 — 与 spec §11.2 native-code 定义直接冲突,**任何改法均需先 spec 修订**;不放 v0.8.1。
- 新增自动化辅助(脚本生成器)— 用户偏好手动维护。
- 新建分支(用户明确约束,GitHub revert 兜底)。

---

> 本计划由独立审计 `docs/AUDIT_REPORT.md`(2026-09-23)驱动,**批判性吸收**:采纳实现 bug,驳回 spec-ambiguous 误诊,灰色地带进 deviations 留 v0.9。spec v0.8 字面文本作为唯一真相源,impl 向其对齐。
