//! [G-20] `std.text::TO_INT` · [G-21] `std.str::CAT` · [G-22] `std.env::SCRIPT_PATH`
//! —— 三个**本次实践补的参考实现**的契约表。
//!
//! 它们补的是「上游实现库里**没有**」的能力：
//!   * `TO_INT`：全局内建 `INT` 的字符串解析太窄（不 trim 空白 / 字符串只收纯整数 /
//!     不支持十六进制文本 / 类型错与解析错两套失败语义）；
//!   * `CAT`：语言里的 `+` 是**二元**的（`+(a,b,c)` ⇒ E0022）⇒ 缺变长拼接
//!     （⚠ 标准库自己就在绕这个缺口：`wl/std/str.wll` 里有个私有的 `_CAT`）；
//!   * `SCRIPT_PATH`：`ARGS()[0]` 是**解释器**自己的路径 ⇒ 应用定位不到自己的资源目录。

use wlwl_eval::Evaluator;

/// 跑一段源码，返回它的**显示**形态（ERR 值会显示成 `ERR([...])`）。
fn run(src: &str) -> String {
    let ast = wlwl_parser::parse(src, "t.wll").expect("case must parse");
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

/// 同 [`run`]，但**带入口文件名**（`with_source`）—— 宿主正是靠它把
/// 「入口源文件名」写进 `StdCtx.source_file`。
fn run_with_file(src: &str, file: Option<&str>) -> String {
    let ast = wlwl_parser::parse(src, "t.wll").expect("case must parse");
    let mut ev = Evaluator::new();
    if let Some(f) = file {
        ev = ev.with_source(src, f);
    }
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

// ── G-20：TO_INT ──────────────────────────────────────────────────────────

#[test]
fn to_int_parses_the_shapes_int_refuses() {
    let src = r#"
IMPORT("wlwl:std.text", ["TO_INT"]);
LET(a, TO_INT("42"));
LET(b, TO_INT("  7  "));
LET(c, TO_INT("3.9"));
LET(d, TO_INT("0x1F"));
LET(e, TO_INT("-5"));
LET(f, TO_INT("+5"));
[a, b, c, d, e, f]
"#;
    assert_eq!(
        run(src),
        "[42, 7, 3, 31, -5, 5]",
        "TO_INT must trim, truncate, and accept hex"
    );
}

#[test]
fn to_int_reports_a_parse_error_value() {
    let src = r#"
IMPORT("wlwl:std.text", ["TO_INT"]);
LET(r, TO_INT("abc"));
IF(IS_ERR(r), (LET(p, ERR_PAYLOAD(r)); AT_K(p, "kind", "?")), "NOT-ERR")
"#;
    assert_eq!(
        run(src),
        "ParseError",
        "a failed parse must be a ParseError value (not a diagnostic)"
    );
}

#[test]
fn to_int_on_an_empty_string_is_a_parse_error_value() {
    let src = r#"
IMPORT("wlwl:std.text", ["TO_INT"]);
LET(r, TO_INT(""));
IF(IS_ERR(r), (LET(p, ERR_PAYLOAD(r)); AT_K(p, "kind", "?")), "NOT-ERR")
"#;
    assert_eq!(run(src), "ParseError");
}

// ── G-21：CAT ──────────────────────────────────────────────────────────────

#[test]
fn cat_joins_an_arbitrary_number_of_parts() {
    let src = r#"
IMPORT("wlwl:std.str", ["CAT"]);
LET(a, CAT());
LET(b, CAT("x", "y"));
LET(c, CAT("a", 1, TRUE));
[a, b, c]
"#;
    assert_eq!(
        run(src),
        "[, xy, a1TRUE]",
        "CAT must render each part with STR (same rule as JOIN)"
    );
}

/// [v0.12.0 / S-1 甲案] `CAT` 归 **R2 原生内核**后的行为锁。
///
/// 「实参为 `ERR` ⇒ **不渲染**、结果就是那个 `ERR`」这条规则写在**语言层**
/// (§12.6 透明传播:`CAT` 不在 §8.3 消费者注册表里 ⇒ 调用边界短路,函数体
/// 根本不被求值)⇒ **换层不换语义**。
///
/// ⚠ 这条以前**没有**测试钉住(只有审计的实测读数)⇒ 换层时它最容易被
///   悄悄改掉 ⇒ 这里把它钉成契约。
#[test]
fn cat_propagates_an_err_argument_instead_of_rendering_it() {
    let src = r#"
IMPORT("wlwl:std.str", ["CAT"]);
LET(f, CAT("a", ERR("boom")));
[IS_ERR(f), ERR_PAYLOAD(f)]
"#;
    assert_eq!(
        run(src),
        "[TRUE, boom]",
        "CAT must propagate an ERR argument, never render it (stdlib §6 / §12.6)"
    );
}

/// 同款契约的**嵌套**形态:`ERR` 装在数组里 ⇒ 它不是实参本身 ⇒ 不短路 ⇒
/// 数组被**渲染**成文本(`ERR(...)`),而那份 `ERR` 的未消费状态留在数组里。
#[test]
fn cat_renders_a_container_holding_an_err() {
    let src = r#"
IMPORT("wlwl:std.str", ["CAT"]);
CAT([ERR("nested")])
"#;
    assert_eq!(
        run(src),
        "[ERR(nested)]",
        "CAT must render a container with STR semantics, ERR element included"
    );
}

// ── G-22：SCRIPT_PATH ──────────────────────────────────────────────────────

const SCRIPT_PATH_SRC: &str = r#"
IMPORT("wlwl:std.env", ["SCRIPT_PATH"]);
SCRIPT_PATH()
"#;

#[test]
fn script_path_is_null_when_there_is_no_entry_script() {
    // 嵌入式求值 / REPL / 库里直接 eval ⇒ **没有入口脚本**。
    // ⚠ 宿主那时的哨兵是 `<runtime>`（诊断定位用的占位名，**不是路径**）
    //   ⇒ 成员必须把它翻译成 `NULL`，不能把内部约定泄漏到成员面。
    assert_eq!(run_with_file(SCRIPT_PATH_SRC, None), "NULL");
}

#[test]
fn script_path_is_absolute_even_when_the_host_gives_a_relative_path() {
    // 宿主给的是「命令行原样」的路径（可能是相对的）⇒ 成员必须绝对化。
    let out = run_with_file(SCRIPT_PATH_SRC, Some("t.wll"));
    let cwd = std::env::current_dir().expect("cwd must be readable");
    assert_eq!(
        out,
        cwd.join("t.wll").to_string_lossy().to_string(),
        "SCRIPT_PATH must absolutize a relative entry path"
    );
}

#[test]
fn script_path_keeps_an_absolute_path_unchanged() {
    let abs = std::env::current_dir()
        .expect("cwd must be readable")
        .join("pkg")
        .join("x.wll");
    let out = run_with_file(SCRIPT_PATH_SRC, Some(abs.to_str().expect("utf-8 path")));
    assert_eq!(
        out,
        abs.to_string_lossy().to_string(),
        "SCRIPT_PATH must not rewrite an absolute entry path"
    );
}
