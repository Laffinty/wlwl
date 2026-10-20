//! `wlwl:std.result` 成员契约 —— 标准库规范 §22(v0.11.3 M7 · addendum-11 W-02)。
//!
//! 与 `str_math_contract.rs` 同款结构:「应然」侧是**规范表格**(从 markdown
//! 解析)+ 逐条用例(手写期望)。「应然」侧与实现不同源是这套锁的全部意义。
//!
//! ## 这份契约的**特殊**之处:主不变量是一条**否定**断言
//!
//! 规范 §22.2 规定这三条成员的 `ERR` 分支**不在成员体内执行** —— 语言规范 §8.2
//! 规定「实参为 `ERR` ⇒ 该调用不求函数体,直接以该 `ERR` 为调用结果」,判据是
//! **被调用者的名字**是否在 §8.3 注册表里,而 §8.3 明文「本表不新增条目」。
//!
//! 所以 `addendum-11` 原计划的核心不变���「`MAP(ERR(x), f)` ⇒ `f` 不被调用
//! (用计数器断言)」是**空证**:计数器不涨是因为**体压根没执行**,不是因为
//! `IF` 短路。本文件把三件事拆成**三条独立断言**,任何一条被破坏都要能定位:
//!
//! 1. **行为**:实参为 `ERR` 时,体内与实参无关的副作用计数为 `0`;
//! 2. **机制**:`dispatch_call` 的传播块位于 `user_fn` 分派之前,且三条成员名
//!    **不在** §8.3 注册表里(规范禁止新增);
//! 3. **前提**:`IF` 三参确实短路(规范 §22.3 的调用方惯用法依赖这条)。
//!
//! 反过来,这三条合成「组合子**接不到** `ERR` ⇒ **不可能**成为 §8.2 旁路」
//! —— `addendum-11` §6 的头号风险不是被绕开,是被证明无法发生。

mod common;

use std::path::{Path, PathBuf};
use wlwl_eval::Evaluator;

/// 尚未写期望的占位符(与 `str_math_contract.rs` 同款理由:空串是合法期望值)。
const UNFROZEN: &str = "~UNFROZEN";

/// §22.1 与 §22.5 的成员表合起来恰六行。改这个数必须同时改那两张表与
/// `result.wll` 的 `EXPORT`。
const MEMBER_COUNT: usize = 6;

struct Case {
    name: &'static str,
    src: &'static str,
    expect: &'static str,
}

const RESULT_CASES: &[Case] = &[
    // ── RESULT_MAP:§22.1 第 1 行逐句 ──
    Case {
        name: "result_map_ok_wraps_the_projection",
        src: r#"IMPORT("wlwl:std.result", ["RESULT_MAP"]); RESULT_MAP(OK(5), FUN((x), +(x, 1)))"#,
        expect: "OK(6)",
    },
    Case {
        name: "result_map_ok_where_f_returns_err_yields_nested_ok",
        // §22.1 只说 `OK(x)` → `OK(f(x))`。`f` 返 `ERR` 时结果是**嵌套**的
        // `OK(ERR(..))`,不是展平 —— 这正是 `FLATTEN` 存在的理由。
        src: r#"IMPORT("wlwl:std.result", ["RESULT_MAP"]); RESULT_MAP(OK(1), FUN((x), ERR(["k": x])))"#,
        expect: "OK(ERR([k: 1]))",
    },
    Case {
        name: "result_map_err_passes_through_unchanged",
        src: r#"IMPORT("wlwl:std.result", ["RESULT_MAP"]); IS_ERR(RESULT_MAP(ERR(["why": "bad"]), FUN((x), x)))"#,
        expect: "TRUE",
    },
    Case {
        name: "result_map_err_payload_is_preserved_verbatim",
        src: r#"IMPORT("wlwl:std.result", ["RESULT_MAP"]); AT_K(ERR_PAYLOAD(RESULT_MAP(ERR(["why": "bad"]), FUN((x), x))), "why", "?")"#,
        expect: "bad",
    },
    Case {
        name: "result_map_rejects_non_result_with_e0030",
        // §22.1:「实参非 `RESULT` 且非 `ERR` 时由 `UNWRAP_OR` 报 `E0030`」。
        src: r#"IMPORT("wlwl:std.result", ["RESULT_MAP"]); RESULT_MAP(5, FUN((x), x))"#,
        expect: "!E0030",
    },
    // ── AND_THEN:§22.1 第 2 行逐句 ──
    Case {
        name: "and_then_ok_returns_f_directly_without_re_wrapping",
        src: r#"IMPORT("wlwl:std.result", ["AND_THEN"]); AND_THEN(OK(3), FUN((x), OK(+(x, 1))))"#,
        expect: "OK(4)",
    },
    Case {
        // ⚠ §22.1 写的是 `OK(x)` → `f(x)`,**逐字**如此:结果就是 `f` 的返回值,
        // 本成员**不补 `OK` 外壳**。所以 `f` 一旦返回裸值,链就断了 —— 这是
        // 调用方的责任(与 Rust `and_then` 要求 `F: FnOnce(T) -> Result<U, E>`
        // 同款)。规范 §22.1 有同款警示,这条把它钉住。
        name: "and_then_does_not_re_wrap_a_bare_value_returned_by_f",
        src: r#"IMPORT("wlwl:std.result", ["AND_THEN"]); AND_THEN(OK(3), FUN((x), +(x, 1)))"#,
        expect: "4",
    },
    Case {
        name: "and_then_ok_where_f_returns_err",
        src: r#"IMPORT("wlwl:std.result", ["AND_THEN"]); IS_ERR(AND_THEN(OK(3), FUN((x), ERR(["why": STR(x)]))))"#,
        expect: "TRUE",
    },
    Case {
        name: "and_then_err_passes_through_unchanged",
        src: r#"IMPORT("wlwl:std.result", ["AND_THEN"]); AT_K(ERR_PAYLOAD(AND_THEN(ERR(["why": "bad"]), FUN((x), x))), "why", "?")"#,
        expect: "bad",
    },
    Case {
        name: "and_then_rejects_non_result_with_e0030",
        src: r#"IMPORT("wlwl:std.result", ["AND_THEN"]); AND_THEN("nope", FUN((x), x))"#,
        expect: "!E0030",
    },
    // ── FLATTEN:§22.1 第 3 行 + §22.1 的「只剥一层」句 ──
    Case {
        name: "flatten_ok_ok_drops_one_layer",
        src: r#"IMPORT("wlwl:std.result", ["FLATTEN"]); FLATTEN(OK(OK(7)))"#,
        expect: "OK(7)",
    },
    Case {
        name: "flatten_ok_err_surfaces_the_inner_err",
        src: r#"IMPORT("wlwl:std.result", ["FLATTEN"]); IS_ERR(FLATTEN(OK(ERR(["why": "inner"]))))"#,
        expect: "TRUE",
    },
    Case {
        name: "flatten_err_passes_through_unchanged",
        src: r#"IMPORT("wlwl:std.result", ["FLATTEN"]); AT_K(ERR_PAYLOAD(FLATTEN(ERR(["why": "bad"]))), "why", "?")"#,
        expect: "bad",
    },
    Case {
        name: "flatten_ok_non_result_keeps_one_ok_layer",
        // 「只剥一层」:`FLATTEN(OK(5))` 得 `OK(5)` 而不是 `5`。
        src: r#"IMPORT("wlwl:std.result", ["FLATTEN"]); FLATTEN(OK(5))"#,
        expect: "OK(5)",
    },
    Case {
        name: "flatten_nested_ok_keeps_still_returning_result",
        src: r#"IMPORT("wlwl:std.result", ["FLATTEN"]); IS_OK(FLATTEN(OK(OK(OK(7)))))"#,
        expect: "TRUE",
    },
    // ── 组合子可用普通函数继续组合(ADR-0029 Consequences) ──
    Case {
        name: "combinators_compose_through_plain_closures",
        src: r#"IMPORT("wlwl:std.result", ["RESULT_MAP", "AND_THEN"]);
               AND_THEN(RESULT_MAP(OK(2), FUN((x), *(x, 10))), FUN((x), OK(+(x, 1))))"#,
        expect: "OK(21)",
    },
    Case {
        name: "flatten_after_result_map_does_not_nest",
        src: r#"IMPORT("wlwl:std.result", ["RESULT_MAP", "FLATTEN"]);
               IS_OK(FLATTEN(RESULT_MAP(OK(1), FUN((x), OK(+(x, 1))))))"#,
        expect: "TRUE",
    },
    // ── 元数错:§22.1「三条全是固定元数,元数错由解释器报无名前缀的 `E0022`」 ──
    Case {
        name: "result_map_wrong_arity_is_e0022_without_a_name_prefix",
        src: r#"IMPORT("wlwl:std.result", ["RESULT_MAP"]); RESULT_MAP(OK(1))"#,
        expect: "!E0022",
    },
    // ── NULL_OR:§22.5 逐句 ──
    Case {
        name: "null_or_maps_only_null_to_the_default",
        src: r#"IMPORT("wlwl:std.result", ["NULL_OR"]); NULL_OR(NULL, 5)"#,
        expect: "5",
    },
    Case {
        // §22.5「`NULL` / `""` / `[]` 三者互不相等」——最容易写成 `IF(v, …)` 而
        // 把后两者一起吞掉。这条钉住「不吞」。
        name: "null_or_does_not_treat_empty_string_as_null",
        src: r#"IMPORT("wlwl:std.result", ["NULL_OR"]); NULL_OR("", 5)"#,
        expect: "",
    },
    Case {
        name: "null_or_does_not_treat_empty_array_as_null",
        src: r#"IMPORT("wlwl:std.result", ["NULL_OR"]); NULL_OR([], 5)"#,
        expect: "[]",
    },
    Case {
        name: "null_or_does_not_treat_zero_or_false_as_null",
        src: r#"IMPORT("wlwl:std.result", ["NULL_OR"]); [NULL_OR(0, 5), NULL_OR(FALSE, 5)]"#,
        expect: "[0, FALSE]",
    },
    Case {
        name: "null_or_passes_ok_through",
        src: r#"IMPORT("wlwl:std.result", ["NULL_OR"]); NULL_OR(OK(1), 5)"#,
        expect: "OK(1)",
    },
    Case {
        // §22.5「`NULL_OR` 不是 `UNWRAP_OR`」:ERR 按 §8.2 原样返还,不是默认值。
        name: "null_or_propagates_an_err_instead_of_defaulting",
        src: r#"IMPORT("wlwl:std.result", ["NULL_OR"]); IS_ERR(NULL_OR(ERR("x"), 5))"#,
        expect: "TRUE",
    },
    // ── RESULT_ALL:§22.5 逐句 ──
    Case {
        name: "result_all_unwraps_every_ok_into_one_array",
        src: r#"IMPORT("wlwl:std.result", ["RESULT_ALL"]); RESULT_ALL([OK(1), OK(2), OK(3)])"#,
        expect: "OK([1, 2, 3])",
    },
    Case {
        name: "result_all_on_an_empty_array_is_ok_of_empty",
        // §22.5「空集真」,与 Rust `collect::<Result<Vec<_>, _>>()` 同款。
        src: r#"IMPORT("wlwl:std.result", ["RESULT_ALL"]); RESULT_ALL([])"#,
        expect: "OK([])",
    },
    Case {
        name: "result_all_returns_the_first_err_with_its_payload",
        src: r#"IMPORT("wlwl:std.result", ["RESULT_ALL"]); AT_K(ERR_PAYLOAD(RESULT_ALL([OK(1), ERR(["n": 7]), OK(3)])), "n", "?")"#,
        expect: "7",
    },
    Case {
        name: "result_all_first_err_wins_not_the_last",
        src: r#"IMPORT("wlwl:std.result", ["RESULT_ALL"]); AT_K(ERR_PAYLOAD(RESULT_ALL([OK(1), ERR(["n": 7]), ERR(["n": 8])])), "n", "?")"#,
        expect: "7",
    },
    Case {
        name: "result_all_rejects_a_non_result_element",
        src: r#"IMPORT("wlwl:std.result", ["RESULT_ALL"]); RESULT_ALL([OK(1), 5])"#,
        expect: "!E0030",
    },
    Case {
        name: "result_all_rejects_a_non_array",
        src: r#"IMPORT("wlwl:std.result", ["RESULT_ALL"]); RESULT_ALL(5)"#,
        expect: "!E0030",
    },
    // ── RESULT_ANY:§22.5 逐句 ──
    Case {
        name: "result_any_returns_the_first_ok_verbatim",
        // 「原样,不拆壳」:返回的是 `OK(7)` 而不是 `7`。
        src: r#"IMPORT("wlwl:std.result", ["RESULT_ANY"]); RESULT_ANY([ERR("a"), OK(7), OK(9)])"#,
        expect: "OK(7)",
    },
    Case {
        name: "result_any_on_all_err_returns_the_first_err",
        src: r#"IMPORT("wlwl:std.result", ["RESULT_ANY"]); ERR_PAYLOAD(RESULT_ANY([ERR("a"), ERR("b")]))"#,
        expect: "a",
    },
    Case {
        name: "result_any_on_an_empty_array_is_ok_null",
        src: r#"IMPORT("wlwl:std.result", ["RESULT_ANY"]); RESULT_ANY([])"#,
        expect: "OK(NULL)",
    },
    Case {
        name: "result_any_rejects_a_non_result_element",
        src: r#"IMPORT("wlwl:std.result", ["RESULT_ANY"]); RESULT_ANY([1, 2])"#,
        expect: "!E0030",
    },
    // ── 三条新成员与既有三条可续用 ──
    Case {
        name: "result_all_output_feeds_result_map",
        src: r#"IMPORT("wlwl:std.result", ["RESULT_ALL", "RESULT_MAP"]); RESULT_MAP(RESULT_ALL([OK(1), OK(2)]), FUN((a), LEN(a)))"#,
        expect: "OK(2)",
    },
];

fn scratch_dir() -> PathBuf {
    common::scratch("wlwl_result")
}

fn actual(src: &str) -> String {
    let ast = wlwl_parser::parse(src, "t.wll")
        .unwrap_or_else(|e| panic!("case source must parse: {src}\n{e}"));
    let mut ev = Evaluator::new().with_base_dir(scratch_dir());
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

/// 只锁码的期望:期望形如 `!EXXXX` 且长度为 6 时按前缀匹配。
fn matches(expect: &str, got: &str) -> bool {
    if expect.len() == 6 && expect.starts_with('!') && got.starts_with(expect) {
        return true;
    }
    expect == got
}

fn run_table(label: &str, cases: &[Case]) {
    let bad: Vec<String> = cases
        .iter()
        .filter_map(|c| {
            let got = actual(c.src);
            if matches(c.expect, &got) {
                None
            } else {
                Some(format!(
                    "{name}:\n      spec/expected: {want}\n      actual:         {got}",
                    name = c.name,
                    want = c.expect
                ))
            }
        })
        .collect();
    assert!(
        bad.is_empty(),
        "{label}: {}/{} case(s) diverged:\n  - {}",
        bad.len(),
        cases.len(),
        bad.join("\n  - ")
    );
}

#[test]
fn result_matches_section_22() {
    run_table("std.result §22", RESULT_CASES);
}

/// 反向守卫:表本身不能塌掉,否则上面那条会退化成检查零件事。
#[test]
fn the_case_table_is_not_empty() {
    assert!(
        RESULT_CASES.len() >= 15,
        "result table collapsed to {}",
        RESULT_CASES.len()
    );
    let mut names: Vec<&str> = RESULT_CASES.iter().map(|c| c.name).collect();
    names.sort_unstable();
    let before = names.len();
    names.dedup();
    assert_eq!(
        before,
        names.len(),
        "result table has a duplicate case name"
    );
    for c in RESULT_CASES {
        assert_ne!(
            c.expect, UNFROZEN,
            "result/{} has no expectation yet",
            c.name
        );
    }
}

// ── §22.2 的三条否定断言:组合子接不到 `ERR` ⇒ 不可能是旁路 ────────────

/// 断言 ①(**行为**):实参为 `ERR` 时,体内与实参无关的副作用**一次都不发生**。
///
/// 这条是 §22.2 的可观察核心。⚠ 它**不能**被写成「`f` 不被调用」—— 那在一个
/// 错误实现(比如给 `f` 传 `NULL` 然后照样调用)下也会绿,因为 §8.2 会在更早的
/// 地方就把调用掐掉;真正的区分点在体内那段**与实参无关**的计数器。
#[test]
fn an_err_argument_stops_the_closure_body_from_ever_running() {
    let src = r#"
         IMPORT("wlwl:std.result", ["RESULT_MAP", "AND_THEN", "FLATTEN"]);
         LET MUT(n, 0);
         LET(g, FUN((x), (SET(n, +(n, 1)); OK(1))));
         LET(a, IS_ERR(g(ERR(["why": "probe"]))));
         LET(b, IS_ERR(g(OK(5))));
         [n, a, b]"#;
    assert_eq!(
        actual(src),
        "[1, TRUE, FALSE]",
        "closure body must run exactly once (the OK call) and never for the ERR call"
    );
}

/// 断言 ②(**机制**):三条成员名**不在** §8.3 的 ERR 消费者注册表里。
///
/// §8.3 明文「本表不新增条目」。有人把 `RESULT_MAP` 加进注册表会让整条 §8.2
/// 传播制度对该成员失效 —— 那是规范禁止的变更,本测试据此转红。
#[test]
fn no_combinator_is_registered_as_an_err_consumer() {
    let src = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("lib.rs"),
    )
    .expect("wlwl-eval lib.rs is readable");
    let start = src
        .find("const ERR_CONSUMER_REGISTRY")
        .unwrap_or_else(|| panic!("the ERR consumer registry still exists"));
    let end = src[start..]
        .find("];")
        .map(|i| start + i)
        .expect("the registry array is closed");
    let table = &src[start..end];
    for n in ["RESULT_MAP", "AND_THEN", "FLATTEN"] {
        assert!(
            !table.contains(&format!("\"{n}\"")),
            "`{n}` is in the §8.3 ERR consumer registry — spec §8.3 says \
             \"本表不新增条目\", and §22.2 requires the ERR branch to stay \
             free. See ADR-0029 D6."
        );
    }
}

/// 断言 ③(**机制**):传播块必须**位于** `user_fn` 分派之前。
///
/// 这是 §22.2 那句「R1 门面与 R2 内核同样看不到 `ERR` 实参」的结构性根据。
/// 若有人把传播块挪到分派之后,`OR_ELSE` 之类的成员会「突然可用」—— 那是
/// `addendum-11` §3.5 的头号风险,必须转红。
#[test]
fn the_propagation_block_precedes_user_fn_dispatch() {
    let src = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("lib.rs"),
    )
    .expect("wlwl-eval lib.rs is readable");
    let body = &src[src.find("fn dispatch_call").expect("dispatch_call exists")..][..4000];
    let guard = body
        .find("if !whitelisted")
        .expect("dispatch_call still short-circuits on a pending ERR");
    let dispatch = body
        .find("if let Some(v) = user_fn")
        .expect("dispatch_call still dispatches user_fn");
    assert!(
        guard < dispatch,
        "the §8.2 propagation block moved after the user_fn dispatch — R1 \
         closures and R2 kernels would start seeing ERR arguments, turning \
         std.result into a §8.2 backdoor (addendum-11 §6, ADR-0029 §2)"
    );
}

/// 规范 §22.3 的调用方惯用法依赖 `IF` 三参短路:未被选中的一支**完全不求值**。
///
/// 少了这条,`OR_ELSE` 的惯用法会在 `r` 为 `OK` 时也调用 `fallback`。
#[test]
fn if_three_arg_short_circuits_the_branch_it_does_not_pick() {
    let src = r#"
         IMPORT("wlwl:std.result", ["RESULT_MAP", "AND_THEN", "FLATTEN"]);
         LET MUT(n, 0);
         LET(g, FUN((x), (SET(n, +(n, 1)); x)));
         LET(r, ERR(["why": "probe"]));
         LET(p, ERR_PAYLOAD(r));
         LET(y, IF(IS_ERR(r), OK(1), g(p)));
         [n, y]"#;
    assert_eq!(
        actual(src),
        "[0, OK(1)]",
        "the else branch must not be evaluated"
    );
}

/// 规范 §22.3 给了三条惯用法。契约表**逐条**跑它们,免得规范里的示例腐化。
#[test]
fn the_section_22_3_idioms_behave_as_documented() {
    let cases = [
        // OR_ELSE 的惯用法:`OK` 时 fallback 不被调用
        (
            "or_else_idiom_on_ok",
            r#"LET MUT(n, 0);
               LET(r, OK(7));
               LET(p, ERR_PAYLOAD(ERR(["why": "x"])));
               LET(fb, FUN((e), (SET(n, +(n, 1)); OK(0))));
               [IF(IS_ERR(r), fb(p), r), n]"#,
            "[OK(7), 0]",
        ),
        // OR_ELSE 的惯用法:`ERR` 时用载荷调 fallback
        (
            "or_else_idiom_on_err",
            r#"LET(r, ERR(["why": "bad"]));
               LET(p, ERR_PAYLOAD(r));
               LET(fb, FUN((e), OK(e)));
               IF(IS_ERR(r), fb(p), r)"#,
            "OK([why: bad])",
        ),
        // ERR_MAP 的惯用法:只改载荷
        (
            "err_map_idiom",
            r#"LET(r, ERR("bad"));
               LET(p, ERR_PAYLOAD(r));
               LET(y, IF(IS_ERR(r), ERR(+(p, "!")), r));
               ERR_PAYLOAD(y)"#,
            "bad!",
        ),
        // ERR_OR_ELSE 的惯用法:替换载荷
        (
            "err_or_else_idiom",
            r#"LET(r, ERR(["why": "bad"]));
               LET(y, IF(IS_ERR(r), ERR(["why": "replaced"]), r));
               AT_K(ERR_PAYLOAD(y), "why", "?")"#,
            "replaced",
        ),
    ];
    for (name, src, expect) in cases {
        assert_eq!(actual(src), expect, "{name} (§22.3)");
    }
}

// ── 成员面:与标准库规范 §22.1 表格的外部对照 ────────────────────────────

fn spec_text() -> String {
    let p: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
        .join("docs")
        .join("stdlib")
        .join("wlwl-stdlib-spec-v0.11.md");
    std::fs::read_to_string(&p).expect("stdlib spec is readable")
}

/// §22.1 与 §22.5 的成员表(§22.3 那张表的单元格长成
/// `` `ERR(e)` → `fallback(e)` ``,会被首列抽取误当成成员名,所以**不**取它)。
fn member_tables() -> String {
    let text = spec_text();
    let grab = |start_marker: &str, end_marker: Option<&str>| {
        let start = text
            .find(start_marker)
            .unwrap_or_else(|| panic!("{start_marker} exists in the stdlib spec"));
        let end = match end_marker {
            Some(m) => text[start..]
                .find(m)
                .map(|i| start + i)
                .unwrap_or_else(|| panic!("{m} follows {start_marker}")),
            None => text.len(),
        };
        text[start..end].to_string()
    };
    format!(
        "{}\n{}",
        grab("### 22.1", Some("### 22.2")),
        grab("### 22.5", None)
    )
}

fn members_of(section_body: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for line in section_body.lines() {
        let t = line.trim();
        if !t.starts_with('|') || t.contains("---") {
            continue;
        }
        let first = t.trim_start_matches('|').split('|').next().unwrap_or("");
        if first.trim() == "成员" {
            continue;
        }
        let name: String = first
            .trim()
            .trim_matches('`')
            .split('(')
            .next()
            .unwrap_or("")
            .trim()
            .to_string();
        if name.is_empty()
            || !name
                .chars()
                .all(|c| c.is_ascii_uppercase() || c == '_' || c.is_ascii_digit())
        {
            continue;
        }
        if !out.contains(&name) {
            out.push(name);
        }
    }
    out
}

fn r1_exports(path: &str) -> Vec<String> {
    let src = wlwl_std::LANG_SOURCES
        .iter()
        .find(|s| s.path == path)
        .unwrap_or_else(|| panic!("{path} is an R1 module"));
    wlwl_std::lang_exports(src.source)
}

#[test]
fn result_member_set_matches_the_spec_table() {
    let spec = members_of(&member_tables());
    let impls = r1_exports("wlwl:std.result");
    let mut a = spec.clone();
    let mut b = impls.clone();
    a.sort();
    b.sort();
    assert_eq!(
        a, b,
        "§22.1 and the R1 EXPORT list disagree.\n  spec: {spec:?}\n  impl: {impls:?}"
    );
    assert_eq!(
        impls.len(),
        MEMBER_COUNT,
        "§22.1 declares {MEMBER_COUNT} members"
    );
    let mut seen = std::collections::HashSet::new();
    for n in &impls {
        assert!(seen.insert(n.as_str()), "std.result exports `{n}` twice");
    }
}

/// `ADR-0029` D5:`MAP` 与 `std.collection::MAP` 硬撞 `E0021`,所以本成员叫
/// `RESULT_MAP`。本测试钉住「改回去会撞车」这件事 —— 否则下一个人会「顺手统一」。
#[test]
fn result_map_does_not_collide_with_collection_map() {
    let src = r#"
         IMPORT("wlwl:std.collection", ["MAP"]);
         IMPORT("wlwl:std.result", ["RESULT_MAP", "AND_THEN", "FLATTEN"]);
         [MAP([1, 2, 3], FUN((x), +(x, 10))), RESULT_MAP(OK(1), FUN((x), +(x, 1)))]"#;
    assert_eq!(actual(src), "[[11, 12, 13], OK(2)]");
}

/// 而 `std.collection` 那个 `MAP` 单独用时不受影响(改名不能是单向的)。
#[test]
fn collection_map_still_works_on_its_own() {
    let src = r#"IMPORT("wlwl:std.collection", ["MAP"]); MAP([1, 2, 3], FUN((x), *(x, 2)))"#;
    assert_eq!(actual(src), "[2, 4, 6]");
}

/// `ADR-0029` D6 / §4b:三条**不可实现**的成员**不得**出现在 `EXPORT` 里;
/// 四个全局内建**不得**被复刻。两组都用「不该在」的否定断言表达。
///
/// ⚠ W-04 落地后本测试**只**保留 `OR_ELSE` / `ERR_MAP` / `ERR_OR_ELSE` 三条 ——
/// `RESULT_ALL` / `RESULT_ANY` / `NULL_OR` 已在 §22.5 落地,不再属于本组。
#[test]
fn no_unimplementable_member_and_no_reexported_builtin() {
    let impls = r1_exports("wlwl:std.result");
    for n in ["OR_ELSE", "ERR_MAP", "ERR_OR_ELSE"] {
        assert!(
            !impls.iter().any(|m| m == n),
            "`{n}` must not be exported — its ERR branch needs to do work, and \
             §8.2 guarantees the body never runs. See ADR-0029 D6 and stdlib \
             spec §22.3 for the caller-side idiom."
        );
    }
    let builtins = wlwl_eval::registry::resolved_builtin_names();
    for n in &impls {
        assert!(
            !builtins.iter().any(|b| b == n),
            "std.result exports `{n}`, which is also a global builtin — \
             stdlib spec §0.4 forbids re-exporting one (ADR-0029 §4b)"
        );
    }
}

// ── 规范 §22.2 的「装盛 vs 消费」分界线 ────────────────────────────────

/// 第一行(**不是旁路**):成员收数组、在自己作用域内消费元素 `ERR` —— 调用方
/// 自己写循环也能做到,成员未新增任何能力。`RESULT_ALL` 返回该 `ERR` 即是证据。
#[test]
fn a_member_may_consume_an_err_that_starts_out_inside_a_container() {
    let src = r#"
         IMPORT("wlwl:std.result", ["RESULT_ALL"]);
         LET(r, RESULT_ALL([OK(1), ERR(["why": "inner"])]));
         [IS_ERR(r), AT_K(ERR_PAYLOAD(r), "why", "?")]"#;
    assert_eq!(actual(src), "[TRUE, inner]");
}

/// 第二行(**撞墙**):把容器里的 `ERR` **元素**喂给任何闭包都会触发 §8.2。
///
/// 这条正是 `RESULT_ALL` / `RESULT_ANY` **刻意不接受谓词参数**的原因 ——
/// 逐元素调用本身就是一次带 `ERR` 实参的调用。
#[test]
fn handing_a_container_element_to_a_closure_hits_the_wall() {
    let src = r#"
         IMPORT("wlwl:std.collection", ["ANY"]);
         LET(rs, [OK(1), ERR("boom")]);
         ANY(rs, FUN((x), IS_ERR(x)))"#;
    assert!(
        matches("!E0102", &actual(src)),
        "a per-element closure call is itself a call with an ERR argument; got: {}",
        actual(src)
    );
}

/// 第三行(**是旁路**):把**裸** `ERR` 藏进 `DICT` 交给别的函数,让调用方凭空
/// 获得消费它的能力 —— 规范明文排除。钉住标准库没提供这种形状:
/// `OR_ELSE` 始终不可导入。
#[test]
fn no_member_smuggles_a_bare_err_across_a_call_boundary() {
    let src = r#"IMPORT("wlwl:std.result", ["OR_ELSE"])"#;
    assert!(
        matches("!E0023", &actual(src)),
        "OR_ELSE must stay unimportable — see ADR-0029 D6"
    );
}

/// 附录 A 镜像里也得有这一行(与 `stdlib_appendix_a_sync` 互补:那条对账
/// 镜像与实现,这条钉住**命名空间本身**进了镜像)。
#[test]
fn appendix_a_carries_the_std_result_row() {
    let text = spec_text();
    let appendix = &text[text
        .find("<!-- appendix-a:begin -->")
        .expect("appendix A begin marker")
        ..text
            .find("<!-- appendix-a:end -->")
            .expect("appendix A end marker")];
    assert!(
        appendix.contains("| `std.result` |"),
        "appendix A has no std.result row"
    );
}
