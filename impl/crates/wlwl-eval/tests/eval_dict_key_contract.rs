//! 字典键的类型约束(语言规范 §2.1 / §4.5)与 `E0020` 的候选名(§3.2)——
//! 第二轮审计 R2-001 / R2-002 / R2-006 / R2-008 的契约。
//!
//! **判据全部来自语言标准** `docs/wlwl-spec-reference.md`(不可修改):
//!
//! - §2.1:「**键的约束**:字典的键**必须**是 `STRING`、`INTEGER` 或 `INSTANCE`。
//!   `INSTANCE` 作键按**对象身份**判定(2.4):同一实例命中,异实例不命中。
//!   其它类型作键**类型不符**,产生 `E0031`。NaN(2.2)**不得**作键(`E0031`)。」
//! - §4.5:「字典按键取值,**键类型不符产生 `E0031`**,键不存在产生 `E0037`。」
//! - §3.2:「读一个不存在的名字产生 `E0020`(**附最近候选名**)。」
//! - §13.4:「1. `value` 是当前线性 `THIS` → `E0032`(§15.3);**键为线性 `THIS`
//!   同样 → `E0032`;」

mod common;

use std::path::PathBuf;
use wlwl_eval::Evaluator;

struct Case {
    name: &'static str,
    src: &'static str,
    expect: &'static str,
}

/// 跑一段源码,返回「诊断形态」(`!CODE message`)或「值形态」(`Value::display`)。
fn actual(src: &str) -> String {
    let ast = match wlwl_parser::parse(src, "t.wll") {
        Ok(a) => a,
        Err(e) => return format!("!PARSE {}", e.diagnostic().message),
    };
    let mut ev = Evaluator::new().with_base_dir(scratch_dir());
    match ev.eval(&ast) {
        Ok(v) => v.display(),
        Err(e) => format!(
            "!{} {}",
            e.diagnostic().code.as_str(),
            e.diagnostic().message
        ),
    }
}

/// 跑一段源码,返回**整条诊断**(R2-006 要看 `hint` / `suggestion_code`)。
fn diagnostic(src: &str) -> wlwl_error::WlwlDiagnostic {
    let ast = wlwl_parser::parse(src, "t.wll").expect("src must parse");
    let mut ev = Evaluator::new().with_base_dir(scratch_dir());
    let err = ev.eval(&ast).expect_err("src must fail");
    err.diagnostic().clone()
}

fn scratch_dir() -> PathBuf {
    common::scratch("wlwl_dictkey")
}

const CASES: &[Case] = &[
    // ── R2-001:§4.5「键类型不符产生 `E0031`」──
    //
    // ⚠ 缺陷形态:字典分支只有「找到 / 没找到」两路,查不到一律 `E0037`
    //   ⇒ 同一个错误(键用错了)报的是「键不存在」那个码,调用方按码分派就会
    //   走错分支。数组分支反倒有正确的 `E0031` 检查 ⇒ 两条路径不一致。
    Case {
        name: "index_get_dict_key_type_must_be_e0031_not_e0037",
        src: r#"PRINT(INDEX_GET(["a": 1], TRUE));"#,
        expect: "!E0031 INDEX_GET: dict key must be STRING, INTEGER or INSTANCE, got boolean",
    },
    Case {
        name: "index_get_dict_key_float_is_a_type_error",
        src: r#"PRINT(INDEX_GET(["a": 1], 1.5));"#,
        expect: "!E0031 INDEX_GET: dict key must be STRING, INTEGER or INSTANCE, got float",
    },
    // ⚠ `1.0` 是 `FLOAT` **不是** `INTEGER` —— 规范写的是「必须是 `STRING`、
    //   `INTEGER` 或 `INSTANCE`」,所以它同样键类型不符。
    Case {
        name: "index_get_dict_key_integral_float_is_still_a_type_error",
        src: r#"PRINT(INDEX_GET(["a": 1], 1.0));"#,
        expect: "!E0031 INDEX_GET: dict key must be STRING, INTEGER or INSTANCE, got float",
    },
    Case {
        name: "index_get_dict_key_array_is_a_type_error",
        src: r#"PRINT(INDEX_GET(["a": 1], [1]));"#,
        expect: "!E0031 INDEX_GET: dict key must be STRING, INTEGER or INSTANCE, got array",
    },
    Case {
        name: "index_get_dict_key_null_is_a_type_error",
        src: r#"PRINT(INDEX_GET(["a": 1], NULL));"#,
        expect: "!E0031 INDEX_GET: dict key must be STRING, INTEGER or INSTANCE, got null",
    },
    // ── 合法键**不受影响**(§2.1 允许的三型)──
    Case {
        name: "index_get_dict_with_string_key_still_works",
        src: r#"INDEX_GET(["a": 1], "a")"#,
        expect: "1",
    },
    Case {
        name: "index_get_dict_with_integer_key_still_works",
        src: r#"INDEX_GET([1: 2], 1)"#,
        expect: "2",
    },
    // ── 缺键仍是 `E0037`(类型这一关过了,才是「不存在」)──
    Case {
        name: "index_get_missing_dict_key_is_still_e0037",
        src: r#"PRINT(INDEX_GET(["a": 1], "b"));"#,
        expect: "!E0037 INDEX_GET: dict key b not found",
    },
    // ── R2-002:`INDEX_SET` 也是**构造点**(§2.1)──
    Case {
        name: "index_set_dict_key_type_must_be_e0031",
        src: r#"PRINT(INDEX_SET(["a": 1], TRUE, 2));"#,
        expect: "!E0031 INDEX_SET: dict key must be STRING, INTEGER or INSTANCE, got boolean",
    },
    Case {
        name: "index_set_dict_with_string_key_still_works",
        src: r#"INDEX_SET(["a": 1], "b", 2)"#,
        expect: "[a: 1, b: 2]",
    },
    // ── R2-002:字典**字面量**是构造点 —— 错误必须在这里报,而不是拖到
    //    `wlwl:std.json` 的 `STRINGIFY` 才炸(那里报的还是 `E0030`)。──
    Case {
        name: "dict_literal_with_boolean_key_is_e0031",
        src: r#"LET(d, ["a": 1, TRUE: 2]); PRINT(KEYS(d));"#,
        expect: "!E0031 dict key must be STRING, INTEGER or INSTANCE, got boolean",
    },
    Case {
        name: "dict_literal_with_array_key_is_e0031",
        src: r#"LET(d, [[1]: "x"]); PRINT(d);"#,
        expect: "!E0031 dict key must be STRING, INTEGER or INSTANCE, got array",
    },
    Case {
        name: "dict_literal_with_null_key_is_e0031",
        src: r#"LET(d, [NULL: "x"]); PRINT(d);"#,
        expect: "!E0031 dict key must be STRING, INTEGER or INSTANCE, got null",
    },
    Case {
        name: "dict_literal_with_string_and_integer_keys_still_works",
        src: r#"["a": 1, 2: "b"]"#,
        expect: "[a: 1, 2: b]",
    },
    // ── ⚠ `INSTANCE` 当键**合法**(2026-10-20 业主裁决)──
    //
    // 规范 §2.1 的键类型集合**已补上** `INSTANCE`,按**对象身份**判定:
    // 「`DICT` 用实例当键时,同实例命中、异实例不命中」。
    // ⚠ 这条能力是仓内 v0.10.1 起就**刻意**做出来的:`R10-012` 有 4+ 条测试
    //   要求它,`impl/crates/wlwl-cli/tests/conformance.rs` 的 OOP 夹具
    //   (`oop_identity.wll`)也拿实例当字典键 ⇒ 禁掉它等于**删一个语言能力**。
    // ⚠ 这里断言的是**语义**(同实例命中)而不是键的显示形态 —— 后者依赖
    //   `Value::display` 对 `INSTANCE` 的渲染,不该被这条契约钉住。
    Case {
        name: "an_instance_may_still_be_used_as_a_dict_key",
        src: r#"LET(C, CLASS("C", NULL, [])); LET(a, NEW(C)); LET(d, INDEX_SET(DICT(), a, "hit")); AT_K(d, a, "miss")"#,
        expect: "hit",
    },
    // ── R2-008:§13.4 第 1 步 —— 键为线性 `THIS` ⇒ `E0032` ──
    //
    // ⚠ 缺陷形态:键的**类型**检查排在最前 ⇒ 键是线性 `THIS` 时报的是
    //   `E0030`(类型错),而规范要求 `E0032`(线性 capability 逃逸)。
    Case {
        name: "set_prop_with_a_linear_this_key_is_e0032",
        src: r#"LET(C, CLASS("C", NULL, [["m", FUN((self), SET_PROP(self, THIS(), 1))]])); CALL_METHOD(NEW(C), "m");"#,
        expect: "!E0032 linear THIS cannot escape the method body (SET_PROP key)",
    },
];

#[test]
fn dict_key_type_constraints_match_the_spec() {
    let bad: Vec<String> = CASES
        .iter()
        .filter_map(|c| {
            let got = actual(c.src);
            if got == c.expect {
                None
            } else {
                Some(format!(
                    "{}\n      expected: {}\n      actual:   {got}",
                    c.name, c.expect
                ))
            }
        })
        .collect();
    assert!(
        bad.is_empty(),
        "dict key type contract drift:\n    {}",
        bad.join("\n    ")
    );
}

/// R2-006:§3.2「读一个不存在的名字产生 `E0020`(**附最近候选名**)」。
///
/// ⚠ 这一条要查**两处**:
/// 1. **候选池必须含全局内建名** —— 缺陷形态只取作用域里的名字,于是「把内建名
///    打错」时一个候选都给不出来(`LENGTH` 少打 3 个字符,`LEN` 就在旁边)。
/// 2. **候选名必须进 `hint`** —— `hint` 是**人类可读形态**里唯一会渲染的字段
///    (`render_human` 打成 `= hint: …`);`suggestion_code` **只**出现在 JSON 形态
///    ⇒ 只写 `suggestion_code` 的话,终端里的用户看不到这个候选名。
#[test]
fn undefined_name_carries_the_nearest_candidate() {
    // ── 拼错**内建**名:候选必须来自全局内建名册 ──
    let d = diagnostic(r#"PRINT(LENGTH);"#);
    let hint = d
        .hint
        .as_deref()
        .unwrap_or_else(|| panic!("E0020 must carry a `hint` with the nearest candidate: {d:?}"));
    assert!(
        hint.contains("did you mean"),
        "hint must name the candidate: {hint:?}"
    );
    assert!(
        hint.contains("LEN"),
        "hint must offer the nearby builtin `LEN` (distance 3): {hint:?}"
    );
    // JSON 形态那份也不能少 —— 机器读的那一侧同样要带候选名。
    assert!(
        d.suggestion_code.iter().any(|s| match s {
            wlwl_error::Suggestion::Note { description } => description.contains("did you mean"),
            _ => false,
        }),
        "suggestion_code must carry the candidate note: {:?}",
        d.suggestion_code
    );

    // ── 拼错**作用域**里的名字:候选来自作用域(既有行为不许退化)──
    let d = diagnostic("LET(FOO, 1); PRINT(FOOO);");
    let hint = d
        .hint
        .as_deref()
        .unwrap_or_else(|| panic!("E0020 must carry a `hint` with the nearest candidate: {d:?}"));
    assert!(hint.contains("FOO"), "hint must offer `FOO`: {hint:?}");
}

/// 语言标准 §2.1 的路径(与 `spec_appendix_g_sync.rs` 同一写法)。
fn language_spec_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
        .join("docs")
        .join("wlwl-spec-reference.md")
}

/// §2.1 的「键的约束」那**一句** —— 不是整章。
///
/// ⚠ 整章里 `INSTANCE` 本来就是**值域的一员**(§2.1 的类型表列了它),
///   拿整章 `contains("INSTANCE")` 等于**没守** ⇒ 必须只取那一句。
fn spec_key_constraint_sentence() -> String {
    let text = std::fs::read_to_string(language_spec_path()).expect("language spec readable");
    let start = text
        .find("**键的约束**")
        .expect("§2.1 has a 键的约束 clause");
    text[start..].lines().next().unwrap_or("").to_string()
}

/// 2026-10-20 业主裁决:「`INSTANCE` 能不能当字典键」⇒ **能**,且**按身份**。
///
/// ⚠ 这一条守的是**规范文本**:§2.1 的键类型集合必须含 `INSTANCE`,而且
///   必须写明**按对象身份判定** —— 只写「`INSTANCE` 是合法键」而不写身份语义,
///   实现就会滑回「按结构相等」(两次 `NEW` 字段相同就撞键)⇒ 那正是
///   `R10-012` 当年修掉的缺陷。
#[test]
fn the_spec_lists_instance_as_a_legal_dict_key() {
    let sentence = spec_key_constraint_sentence();
    assert!(
        sentence.contains("`INSTANCE`"),
        "语言规范 §2.1 的「键的约束」必须把 `INSTANCE` 列进合法键类型: {sentence}"
    );
    assert!(
        sentence.contains("对象身份"),
        "语言规范 §2.1 的「键的约束」必须写明 `INSTANCE` 按**对象身份**判定: {sentence}"
    );
}

/// 键类型的**完整矩阵**:合法三型放行、其余一律 `E0031`。
///
/// ⚠ 为什么要有这条:上面那批固定文案只覆盖了布尔 / 浮点 / 数组 / null
///   **四型**,而**字典 / 类对象 / 函数** 这几型当键**没人钉** —— 它们能悄悄
///   溜进容器(闸门漏了),直到某个下游(`wlwl:std.json` 的 `STRINGIFY`)才炸,
///   报的还是**别的码** ⇒ 正是 R2-002 当初的缺陷形态。
/// ⚠ 合法集取**规范 §2.1 的三型**(`STRING` / `INTEGER` / `INSTANCE`),
///   不是抄实现 —— 规范改了、实现没跟上时这条会红。
/// ⚠ 全部走 `INDEX_SET` 这条路径:它**必过**闸门;而 `AT` / `AT_K` / `HAS`
///   遇错类型只「查不到」不报错 ⇒ 用它们验不出闸门。
/// ⚠ `TASK` / `CHANNEL` 两型**不在**矩阵里:构造它们要 `SPAWN` /
///   `CHANNEL_NEW`,会把「造得出来吗」和「能当键吗」两件事搅在一起
///   (前者要起运行时)⇒ 留作已知缺口,不在这里假装覆盖。
#[test]
fn every_key_type_is_either_legal_or_e0031() {
    // ── 合法三型(§2.1)──
    for (label, src) in [
        ("STRING", r#"INDEX_SET(DICT(), "a", 1)"#),
        ("INTEGER", r#"INDEX_SET(DICT(), 1, 1)"#),
        (
            "INSTANCE",
            r#"LET(C, CLASS("C", NULL, [])); LET(a, NEW(C)); INDEX_SET(DICT(), a, 1)"#,
        ),
    ] {
        let got = actual(src);
        assert!(
            !got.starts_with("!E"),
            "{label} 是 §2.1 允许的键类型,实现却报错了: {got}"
        );
    }
    // ── 其余一律 `E0031` ──
    for (label, src) in [
        ("BOOLEAN", r#"INDEX_SET(DICT(), TRUE, 1)"#),
        ("FLOAT", r#"INDEX_SET(DICT(), 1.0, 1)"#),
        ("ARRAY", r#"INDEX_SET(DICT(), [1], 1)"#),
        ("DICT", r#"INDEX_SET(DICT(), ["x": 1], 1)"#),
        ("NULL", r#"INDEX_SET(DICT(), NULL, 1)"#),
        (
            "CLASS",
            r#"LET(C, CLASS("C", NULL, [])); INDEX_SET(DICT(), C, 1)"#,
        ),
        ("FUNCTION", r#"INDEX_SET(DICT(), FUN((x), x), 1)"#),
    ] {
        let got = actual(src);
        assert!(
            got.starts_with("!E0031 INDEX_SET: dict key must be STRING, INTEGER or INSTANCE, got "),
            "{label} 不是合法键类型,必须报 E0031(且文案要列出 §2.1 的合法集): {got}"
        );
    }
}
