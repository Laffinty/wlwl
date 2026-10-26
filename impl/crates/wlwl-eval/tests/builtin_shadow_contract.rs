//! [v0.12.0 / R-4] **遮蔽内建名**与**零参 `THIS`** 的契约。
//!
//! ## 背景(审计 F-06 / F-07,2026-10-26)
//!
//! 1. `IS_OK` / `IS_ERR` / `OR_DIE` 被实现成**关键字** ⇒ 规范 §3.5 的「遮蔽内建名」
//!    规则对它们**永久失效** ⇒ `LET(IS_OK, 1)` 报的是**语法错 `E0010`**
//!    (`expected identifier in LET, got IsOk`),而规范要求 `E0025`
//!    (`cannot shadow built-in 'IS_OK'`)⇒ 按码分流的工具(IDE / CI 门禁 /
//!    agent 自愈规则)会走错分支。
//!    规范 §1.4 的关键字表**只有 15 个**,这三个不在其中 ⇒ 它们是附录 G 的
//!    **全局内建**,不是关键字。
//! 2. 裸 `THIS`(零参引用)此前按**普通变量**查找 ⇒ `E0020: undefined name THIS`
//!    且附「did you mean one of: HAS, TRIM?」的**误导**提示;对象册 §15.1 明写
//!    `THIS()` 与零参引用 `THIS` **都**合法。
//!
//! ## 形状
//!
//! 一个用例 = 一个 `#[test]`,断言**诊断码 + 消息**或**返回值**。

use wlwl_eval::Evaluator;

/// 跑一段源码,返回 `Ok(显示形态)` 或 `Err(诊断码, 消息)`。
fn run(src: &str) -> Result<String, (String, String)> {
    let ast =
        wlwl_parser::parse(src, "t.wll").unwrap_or_else(|e| panic!("case must parse: {src}\n{e}"));
    let mut ev = Evaluator::new();
    match ev.eval(&ast) {
        Ok(v) => Ok(v.display()),
        Err(e) => Err((
            e.diagnostic().code.as_str().to_string(),
            e.diagnostic().message.clone(),
        )),
    }
}

// ── ① 遮蔽内建名(§3.5)──────────────────────────────────────────────────

#[test]
fn shadowing_is_ok_is_a_spec_level_e0025() {
    // 此前:解析期就拒了(`E0010: expected identifier in LET, got IsOk`)⇒
    // 遮蔽规则**根本到不了运行期** ⇒ 报的是**语法错**而不是 §3.5 的 `E0025`。
    let (code, msg) = run("LET(IS_OK, 1);").expect_err("shadowing a builtin must fail");
    assert_eq!(
        code, "E0025",
        "shadowing a global builtin must be E0025 (spec §3.5), got {code}: {msg}"
    );
    assert!(
        msg.contains("cannot shadow built-in 'IS_OK'"),
        "message must name the builtin: {msg}"
    );
}

#[test]
fn shadowing_is_err_is_a_spec_level_e0025() {
    let (code, msg) = run("LET(IS_ERR, 1);").expect_err("shadowing a builtin must fail");
    assert_eq!(code, "E0025", "got {code}: {msg}");
    assert!(
        msg.contains("cannot shadow built-in 'IS_ERR'"),
        "message must name the builtin: {msg}"
    );
}

#[test]
fn shadowing_or_die_is_a_spec_level_e0025() {
    let (code, msg) = run("LET(OR_DIE, 1);").expect_err("shadowing a builtin must fail");
    assert_eq!(code, "E0025", "got {code}: {msg}");
    assert!(
        msg.contains("cannot shadow built-in 'OR_DIE'"),
        "message must name the builtin: {msg}"
    );
}

#[test]
fn they_are_usable_as_parameter_names() {
    // 它们**不是**关键字 ⇒ 形参名合法(此前是解析错 `E0010: expected
    // parameter name, got IsOk`)。
    let v = run("FUN((IS_OK), 1);").expect("a parameter named IS_OK must parse");
    assert!(
        v.starts_with("<fun("),
        "the program value must be the function value, got {v}"
    );
}

// ── ② 调用语义**逐字未变** ───────────────────────────────────────────────

#[test]
fn the_call_semantics_are_unchanged() {
    assert_eq!(run("IS_OK(OK(1));").unwrap(), "TRUE");
    assert_eq!(run("IS_OK(ERR(\"x\"));").unwrap(), "FALSE");
    assert_eq!(run("IS_ERR(ERR(\"x\"));").unwrap(), "TRUE");
    assert_eq!(run("IS_ERR(OK(1));").unwrap(), "FALSE");
    // `OR_DIE` 是 `UNWRAP_OR` 的弃用别名:OK ⇒ 载荷,ERR ⇒ 默认值。
    assert_eq!(run("OR_DIE(OK(5), 0);").unwrap(), "5");
    assert_eq!(run("OR_DIE(ERR(\"boom\"), 42);").unwrap(), "42");
}

// ── ③ 裸 `THIS`(对象册 §15.1 的零参引用形态)─────────────────────────────

#[test]
fn a_bare_this_outside_a_method_body_is_e0032() {
    // 此前:「`E0020`: undefined name `THIS`」+ 一句「did you mean one of:
    // HAS, TRIM?」的**误导**提示 ⇒ agent 会去改**名字**,而真正要改的是**写法**。
    let (code, msg) = run("PRINT(THIS);").expect_err("bare THIS outside a method body must fail");
    assert_eq!(
        code, "E0032",
        "bare THIS outside a method body must be E0032 (objects booklet §15.1), got {code}: {msg}"
    );
    assert!(
        !msg.contains("undefined name"),
        "the message must not be a name-resolution error: {msg}"
    );
}

#[test]
fn a_bare_this_inside_a_method_body_is_the_receiver() {
    // 方法体**内部**用裸 `THIS` ⇒ 求值为当前活跃的接收者(对象册 §15.1)。
    // ⚠ 这里**不返回**它:线性 `THIS` 不得逃出方法体(那是另一条既有规则)。
    let src = r#"
LET(C, CLASS("C", NULL, [["get", FUN((self), (LET(x, THIS); 42))]]));
LET(o, NEW(C));
CALL_METHOD(o, "get")
"#;
    assert_eq!(
        run(src).expect("a bare THIS inside a method body must resolve"),
        "42"
    );
}
