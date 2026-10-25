//! [G-23] `std.result::TAKE_ERR` —— **一步式「检查并消费」**的契约表。
//!
//! ## 它补的是哪一条
//!
//! 语言册 §8.2 的推论已经写死了唯一合规的写法：「检查并消费一个 `ERR` 值**必须**
//! 在拥有它的作用域内进行 —— 先以注册表消费者（`IS_ERR`、`ERR_PAYLOAD`、`UNWRAP_OR` 等）
//! 提取信息或兜底，再把**提取结果**（普通值）传给其他函数」。
//!
//! 但现成的注册表消费者**没有一个是「一步式」的**：
//!
//! - `IS_ERR` 是**观察型**（§8.6.1）⇒ 只看不带走 ⇒ 漏掉 `ERR_PAYLOAD` 就在作用域
//!   结束时撞 `E0103`；
//! - `UNWRAP_OR(x, d)` 确实消费，却只给**兜底值**、**丢掉载荷** ⇒ 想要「失败原因」
//!   就拿不到。
//!
//! ⇒ `TAKE_ERR` 把两件事合成**一步**：看一眼 + 取走载荷 + 消费掉那份未消费状态。
//!
//! ## 判据怎么写
//!
//! 「消费了没有」**不能**只看返回值 —— 那得靠**作用域结束时有没有 `E0103`**来判。
//! 本表的每条用例都跑在**顶层作用域**（顶层结束**一定**检查，见 `e0103_contract.rs`）
//! ⇒ 返回了预期值 **且** 程序没以 `E0103` 收场，两件事一起钉住。

use wlwl_eval::Evaluator;

/// 求值一段源，返回：
/// * 成功 ⇒ 值的 `display()`；
/// * 失败 ⇒ `!{码} {消息}`。
fn actual(src: &str) -> String {
    let ast = wlwl_parser::parse(src, "t.wll")
        .unwrap_or_else(|e| panic!("case source must parse: {src}\n{e}"));
    let mut ev = Evaluator::new();
    match ev.eval(&ast) {
        Ok(v) => v.display(),
        Err(e) => {
            let code = e.diagnostic().code.as_str();
            if e.diagnostic().message.is_empty() {
                format!("!{code}")
            } else {
                format!("!{code} {}", e.diagnostic().message)
            }
        }
    }
}

const IMPORT: &str = r#"IMPORT("wlwl:std.result", ["TAKE_ERR"]);
IMPORT("wlwl:std.encode", ["BASE64_DECODE"]);
"#;

#[test]
fn take_err_returns_the_payload_and_consumes_it() {
    // 载荷是 `ERR` 的**内层**值（`ERR([kind: ..., ...])` ⇒ 载荷是那个字典）。
    let out = actual(&format!(
        "{IMPORT}LET(p, TAKE_ERR(BASE64_DECODE(\"!!!\")));p"
    ));
    assert!(
        out.contains("DecodeError"),
        "TAKE_ERR must hand back the ERR payload, got {out}"
    );
    assert!(
        !out.starts_with('!'),
        "TAKE_ERR must consume the ERR (no E0103 at scope end), got {out}"
    );
}

#[test]
fn take_err_on_a_non_error_is_null() {
    let out = actual(&format!("{IMPORT}LET(p, TAKE_ERR(42));p"));
    assert_eq!(out, "NULL", "a non-ERR input must yield NULL");
}

#[test]
fn the_payload_is_an_ordinary_value_afterwards() {
    // §8.2 推论的后半段：「再把**提取结果**（普通值）传给其他函数」。
    // ⇒ 这一条钉的是：**取出来的载荷能当普通值传走**（不必再自己拆壳）。
    let out = actual(&format!(
        "{IMPORT}\
         LET(show, FUN((payload), +(\"kind=\", AT_K(payload, \"kind\", \"?\"))));\
         LET(p, TAKE_ERR(BASE64_DECODE(\"!!!\")));\
         show(p)"
    ));
    assert_eq!(
        out, "kind=DecodeError",
        "the payload must be an ordinary value that can be passed to a helper"
    );
}

#[test]
fn is_err_still_sees_the_original_value_after_take_err() {
    // `TAKE_ERR` 只**结清未消费状态**（§8.6.1 的取走型），**不**改写那个值
    // ⇒ `IS_ERR(r)` 之后仍为真 ⇒ 可以拿它当分支条件，且**不会**再撞 E0103。
    let out = actual(&format!(
        "{IMPORT}\
         LET(r, BASE64_DECODE(\"!!!\"));\
         LET(p, TAKE_ERR(r));\
         IS_ERR(r)"
    ));
    assert_eq!(
        out, "TRUE",
        "the value stays an ERR; only its unconsumed state is settled"
    );
}

#[test]
fn take_err_is_not_short_circuited_by_the_err_argument() {
    // §8.2 的墙：「实参为 `ERR` ⇒ 不求函数体」。`TAKE_ERR` 是**注册表里的取走型
    // 消费者** ⇒ 实参的 `ERR` 合法地进了它 ⇒ 载荷拿得到（而不是被原样返还）。
    let out = actual(&format!(
        "{IMPORT}\
         LET(p, TAKE_ERR(BASE64_DECODE(\"!!!\")));\
         TYPE(p)"
    ));
    assert_ne!(
        out, "RESULT",
        "TAKE_ERR must receive the ERR (a short-circuit would return the ERR itself)"
    );
    assert!(!out.starts_with('!'), "got {out}");
}

#[test]
fn without_take_err_the_scope_end_still_reports_e0103() {
    // **对照**：不加 `TAKE_ERR` ⇒ 那份未消费状态留在绑定里 ⇒ 作用域结束报 `E0103`
    // （§8.6.2 第 3 项）。⇒ 证明上面几条的「没报 E0103」是 `TAKE_ERR` 的功劳，
    // 不是别的什么顺带改了语义。
    let out = actual(
        r#"IMPORT("wlwl:std.encode", ["BASE64_DECODE"]);
LET(r, BASE64_DECODE("!!!"));
LET(p, IS_ERR(r));
p"#,
    );
    assert!(
        out.starts_with("!E0103"),
        "the unconsumed ERR must still be reported at scope end, got {out}"
    );
}
