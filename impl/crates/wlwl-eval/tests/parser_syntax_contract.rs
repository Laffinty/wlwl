//! v0.11.3 收口:4 条「实现不符合语言标准」的**语法契约**。
//!
//! 本册钉的是**语言标准**(`docs/wlwl-spec-reference.md`,不可修改)与实现
//! 之间的差距,修的一律是**实现侧**。判据一律取自规范原文,不是抄实现的输出。
//!
//! | 缺陷 | 规范条款 | 一句话 |
//! |---|---|---|
//! | R2-003 | §5.2 形参表(`spec:436`) | `*name`「只**可以**出现在末位」 |
//! | R2-004 | §1.6 整数字面量(`spec:100`) | `int_literal = [ "+" \| "-" ] int_lit .` |
//! | R2-005 | §1.8 字符串字面量(`spec:122-130`) | `${…}` 内的表达式按**完整表达式语法**求值 |
//! | R2-009 | §2.6 事实 ③(`spec:234-237`)/ 附录 A 注 ③(`spec:1898`) | 显式约束只跟在**裸标识符**后 |
//!
//! ## 三条贯穿全册的纪律
//!
//! 1. **只钉诊断码不够。** E0010 能由很多条路径报出来(形参名不对、类型约束
//!    位置不对、类型表达式缺项……),所以每条负向用例都额外钉**消息**里那句
//!    能唯一认出分支的话。
//! 2. **期望值不抄实现。** 每个期望值都能在规范原文里找到出处:形参末位规则
//!    出自 §5.2,`+x` 的展开形式出自 §1.6 的实现注,ERR 透传出自 §1.8 末句 +
//!    §8.2,显示形态出自 §2.5。
//! 3. **合法形态也要钉。** 只钉「非法形态被拒」不够:一个过严的实现会把
//!    规范明写合法的形态一起拒掉(那不是修,是换一种不合规)。所以每条缺陷
//!    都配一条**正例**,把「不许误伤」的边界钉死。

mod common;

use std::path::PathBuf;

use wlwl_eval::Evaluator;

/// 临时工作目录。本册的用例都不碰文件系统,建它只是为了不落在仓库里。
fn scratch_dir() -> PathBuf {
    common::scratch("wlwl_syntax_contract")
}

/// 解析 + 求值,返回可比较的文本。
///
/// 失败时返回 `!码 消息`(与 `text_contract.rs` 同一形状),这样负向用例能
/// 同时钉**码**与**文案**。
fn actual(src: &str) -> String {
    let ast = wlwl_parser::parse(src, "contract.wll")
        .unwrap_or_else(|e| panic!("case must parse: {src}\n{e}"));
    let mut ev = Evaluator::new().with_base_dir(scratch_dir());
    match ev.eval(&ast) {
        Ok(v) => v.display(),
        Err(e) => {
            let d = e.diagnostic();
            if d.message.is_empty() {
                format!("!{}", d.code.as_str())
            } else {
                format!("!{} {}", d.code.as_str(), d.message)
            }
        }
    }
}

/// 只走**解析**,返回 `(诊断码, 消息)`。解析成功时码是 `<parsed>`。
///
/// 解析失败类缺陷(R2-003 / R2-009)必须用这个入口:它们在**解析期**就该
/// 断掉,不该走到求值期再炸。
fn parse_diag(src: &str) -> (String, String) {
    match wlwl_parser::parse(src, "contract.wll") {
        Ok(_) => ("<parsed>".to_string(), String::new()),
        Err(e) => {
            let d = e.diagnostic();
            (d.code.as_str().to_string(), d.message.clone())
        }
    }
}

/// 断言一条**语法错**用例:码必须是 `E0010`,且消息里必须出现 `needle`
/// (那半句能唯一认出是哪条分支的话)。
fn assert_syntax_error(src: &str, needle: &str) {
    let (code, message) = parse_diag(src);
    assert_eq!(code, "E0010", "source: {src}\nmessage: {message}");
    assert!(
        message.contains(needle),
        "source: {src}\nexpected message to contain: {needle}\nactual message: {message}"
    );
}

// ── R2-003 §5.2:`*name` 只可以出现在末位 ────────────────────────────

/// §5.2 形参表:`*name`「剩余参数;收集多余的实参为**新数组**。只**可以**
/// 出现在末位」。
///
/// 缺陷形态:形参循环对**任意位置**的 `*` 一律接受,于是 `(*rest, a)` 里的
/// `*rest` 被求值器当成普通形参 —— 求值器的 `has_rest` 判据是「**最后一个**
/// 形参是 rest」,于是多出来的实参报 `E0022`(说的是元数,不是「形参表写错了」)。
#[test]
fn r2_003_a_rest_param_must_be_the_last_one() {
    // ⚠ 消息钉的是「rest 形参」那半句:E0010 由很多路径报出来,只钉码的话
    // 「别的分支恰好也报 E0010」会蒙混过关。
    assert_syntax_error("LET(f, FUN((*rest, a), [rest, a]));", "rest parameter");
}

/// 同一条约束的**另一半**:两个 rest 形参。
///
/// 缺陷形态:求值器的绑定循环碰到第一个 rest 就 `break`,第二个 `*b` **永远
/// 绑不上** —— `b` 在函数体里成了未定义名,错误一路推迟到**运行期**才炸成
/// `E0020`(说的是「名字没定义」,不是「形参表写错了」)。
#[test]
fn r2_003_two_rest_params_are_rejected() {
    assert_syntax_error("LET(f, FUN((*a, *b), [a, b]));", "rest parameter");
}

/// **正例**:末位的 rest 形参照常收集多余实参(§5.3:「有剩余参数:`实参数 ≥ R`;
/// 超出固定形参的实参收集为剩余数组」)。
///
/// 这条钉的是「不许误伤」:末位 rest 是规范**明写**的形态,修 bug 修掉它
/// 就是把合法程序判死。
#[test]
fn r2_003_a_trailing_rest_param_still_collects_surplus_args() {
    assert_eq!(
        actual("LET(f, FUN((a, *rest), [a, rest])); f(1, 2, 3);"),
        "[1, [2, 3]]"
    );
    // 只有 rest 的形参表:全部实参都进剩余数组。
    assert_eq!(
        actual("LET(f, FUN((*rest), rest)); f(1, 2, 3);"),
        "[1, 2, 3]"
    );
}

// ── R2-004 §1.6:前导 `+` 的整数字面量 ──────────────────────────────

/// §1.6 的 EBNF 是**规范性**的:`int_literal = [ "+" | "-" ] int_lit .` —— 前导
/// `+` 与前导 `-` **同形**。§1.6 的实现注逐字写着「前导符号字面量 `+x` / `-x`
/// 在语法分析阶段被改写为 `+(0, x)` / `-(0, x)`」。
///
/// 缺陷形态:一元 sugar 分支只判 `Minus`,`+x` 落进「变量引用」分支产出
/// `Var("+")` 并把操作数留给下一条语句 ⇒ `PRINT(+5);` 报
/// `E0011: expected ')', got Integer(5)`。同一形态的两种符号行为不一致。
#[test]
fn r2_004_a_leading_plus_is_the_same_shape_as_a_leading_minus() {
    // 期望值来自 §1.6:前导 `+` 的展开形式是 `+(0, x)`,而 `+(0, 5)` 就是
    // 0 + 5 = 5(§4.3 的加法)。
    assert_eq!(actual("+5"), "5");
    assert_eq!(actual("+(0, 5)"), "5");
    // 负向侧:前导 `-` 一直是对的,别把它带坏。
    assert_eq!(actual("-5"), "-5");
}

/// **正例**:`+` 的**二元调用形态**不受影响。
///
/// 判据是 §1.6 那个「前导符号」的限定:只有 `+` **不接** `(` 时才是前导符号,
/// `+(a, b)` 继续是「调用 `+`」—— 与 `-` 分支的判定完全一致。
///
/// ⚠ `+(-3)` 报 `E0022`(`+` 是 2 元函数)与 `-(-3)` 报 `E0022`(`-` 是 2 元
/// 函数)必须**同形**:这正是 §1.6 把两种符号并排写进文法的理由。
#[test]
fn r2_004_a_leading_plus_keeps_the_binary_call_form() {
    assert_eq!(actual("+(1, 2)"), "3");
    let plus = actual("+(-3)");
    let minus = actual("-(-3)");
    assert!(
        plus.starts_with("!E0022") && minus.starts_with("!E0022"),
        "both signs must keep the binary-call form: `+(-3)` -> {plus} ; `-(-3)` -> {minus}"
    );
    // 消息形态由 §5.3 规定:「违反范围产生 `E0022`,消息形如
    // `expects R..N argument(s), got A`」。
    assert!(
        plus.contains("expects 2 argument(s)"),
        "unexpected message: {plus}"
    );
}

// ── R2-005 §1.8:`${…}` 内按完整表达式语法求值 ───────────────────────

/// §1.8 的文法:`interpolation = "${" Expression "}"`;§1.8 正文逐字写着
/// 「`${...}` 内的表达式按**完整表达式语法**求值」—— 字符串字面量是
/// `Expression` 的合法组成。
///
/// 缺陷形态:限制长在**词法层**,绕不开 —— 插值体里见 `"` 即报 E0001,
/// `"${"x"}"`、`"${ERR("boom")}"` 一律在词法期被拒。
#[test]
fn r2_005_an_interpolation_may_contain_a_nested_string_literal() {
    // 期望值出自 §1.8:插值结果以 STR 渲染后**嵌入**字符串。
    assert_eq!(actual(r#""${"x"}""#), "x");
    assert_eq!(actual(r#""a${"b"}c""#), "abc");
}

/// 跳过嵌套字符串时,边界必须数对:嵌套字符串里的 `}` 不是插值体的括号,
/// 转义引号 `\"` 也不是字符串的结束符。
///
/// 少处理一条,源码就会被截成半截表达式 —— 症状是「表达式莫名其妙少一截」,
/// 而不是一条像样的诊断。
#[test]
fn r2_005_a_nested_string_does_not_truncate_the_interpolation() {
    // 嵌套字符串里的 `}`:整串被完整取出(ERR 的载荷是 `a}b`)。
    assert_eq!(
        actual(r#""${ERR("a}b")}""#),
        "!E0102 unhandled ERR escaped to top level: a}b"
    );
    // 转义引号:载荷是 `a"b`(反斜杠被 §1.8 的转义规则吃掉)。
    assert_eq!(
        actual(r#""${ERR("a\"b")}""#),
        r#"!E0102 unhandled ERR escaped to top level: a"b"#
    );
}

/// §1.8 末句:「求值若产生 `ERR` 则按 8.2 透传,整个字符串字面量表达式的
/// 结果为该 `ERR`」。
///
/// 也就是说插值体里的 `ERR` **不是**词法错、也不是运行期崩溃,而是**值**:
/// 它一路透传到顶层,顶层再按 §19.6 的推论报 E0102「未处理的 ERR 逃到顶层」。
///
/// ⚠ 缺陷形态下这条用例在**词法期**就断了(E0001),根本走不到求值 ——
/// 这正是「限制长在词法层」的可怕之处:换写法也不行。
#[test]
fn r2_005_an_err_inside_an_interpolation_propagates_as_a_value() {
    assert_eq!(
        actual(r#""${ERR("x")}""#),
        "!E0102 unhandled ERR escaped to top level: x"
    );
}

/// **正例**:一条字符串里的**多次**插值照旧拼接(既有行为,不许被新扫描器
/// 破坏)。
#[test]
fn r2_005_two_interpolations_in_one_string_still_concatenate() {
    assert_eq!(actual(r#""${1}${2}""#), "12");
}

// ── R2-009 §2.6 事实 ③:约束只跟在裸标识符后 ─────────────────────────

/// §2.6 事实 ③:「**显式约束只跟在裸标识符后,且不可嵌套。**`T: Comparable`
/// 合法;`ARRAY[INTEGER]: Comparable` 报 **`E0010`**,消息是
/// `a type constraint may only follow a bare type variable`」;附录 A 注 ③
/// 重复了同一条。
///
/// 两种「给容器加约束」的写法都必须报这一条:
///   ① **括号外**:`ARRAY[INTEGER] : Comparable` —— 约束跟在复合类型后;
///   ② **括号内**:`ARRAY[ARRAY[INTEGER] : Comparable]` —— 同一个缺陷形态,
///      只是它长在方括号里。
///
/// ⚠ ② 在修之前报的是 **E0012**(`expected `,` or `]` in type expression,
/// got `:``)—— 语法错是错的,但**码与文案都不是规范说的那条**。同一个缺陷
/// 形态,顶层那条报 E0010、括号里那条报 E0012 ⇒ 诊断码分叉,下游按码分流
/// 会漏掉一半。
#[test]
fn r2_009_a_constraint_on_a_container_is_e0010() {
    // ① 括号外(修之前就是对的,不许被改动弄坏)。
    assert_syntax_error(
        "LET(x: ARRAY[INTEGER] : Comparable, [1]);",
        "a type constraint may only follow a bare type variable",
    );
    // ② 括号内(修之前报 E0012 ⇒ 这次下沉检查把它拉到 E0010)。
    assert_syntax_error(
        "LET(x: ARRAY[ARRAY[INTEGER] : Comparable], [1]);",
        "a type constraint may only follow a bare type variable",
    );
    assert_syntax_error(
        r#"LET(x: DICT[STRING, ARRAY[INTEGER] : Comparable], ["k": 1]);"#,
        "a type constraint may only follow a bare type variable",
    );
}

/// **正例**:「裸标识符 + 显式约束」在**方括号内的类型参数**位置是**合法**的。
///
/// 判据是规范三处一致的写法,不是实现的取舍:
///   - §2.6 事实 ③:约束只跟在**裸标识符**后 —— `T` 就是裸标识符;
///   - §5.2.1(`spec:460`):「语法是**裸标识符 + 显式约束**,在**顶层注解与
///     方括号内的类型参数**两处都认(附录 A 的 `BoundedVar`)」;
///   - 附录 A 的文法:`TypeArg = Type | BoundedVar .` —— `BoundedVar` 明确
///     是类型实参的合法成分。
///
/// ⚠ 这条正例是**防过严**的:把它改成 E0010 会让实现比规范**严**,而规范把
/// 这种形态写成合法 —— 那不是修缺陷,是换一个不合规(还会连带废掉附录 E.1
/// 的签名文件文法:签名里的 `Type` 就是附录 A 的 `Type`)。
#[test]
fn r2_009_a_bounded_type_variable_inside_brackets_stays_legal() {
    // 顶层注解位置(§5.2.1:「顶层注解…那处也认」)。
    assert_eq!(actual("LET(x: T: Comparable, 1); x"), "1");
    // 方括号内的类型参数位置(§5.2.1:「方括号内的类型参数…那处也认」)。
    assert_eq!(actual("LET(x: ARRAY[T: Comparable], [1]); x"), "[1]");
    assert_eq!(
        actual(r#"LET(x: DICT[STRING, T: Comparable], ["k": 1]); x"#),
        "[k: 1]"
    );
}
