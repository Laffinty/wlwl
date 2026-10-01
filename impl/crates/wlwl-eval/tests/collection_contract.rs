//! `wlwl:std.collection` 成员契约表 —— 标准库规范 §5(标准库底座 v0.11 M3-1)。
//!
//! 这张表是 M3-1 的**交付物**,生成过程是它的出生证明:
//!
//! 1. R2 原生实现还在位时,跑一遍全部用例,把实际结果**冻结**进 `expect`
//!    —— 于是表里每一行都是「R2 当时就是这么做的」的事实,不是人手写的
//!    期望(人手写期望只会把实现现状和意图混为一谈);
//! 2. 删掉 R2、`collection` 改挂 `LANG_SOURCES` 之后,跑**同一张表**:
//!    全绿即证明纯 wlwl 重写与原生实现在码与值上等价。
//!
//! 比对口径:**诊断码 + 消息逐字**,外加结果值的 `display()`。逐字比消息
//! 是有意的 —— M3 只允许「码守恒 + 措辞登记偏差」,所以消息一旦漂移就必须
//! 在这里变红,由人去决定是改实现还是改表。
//!
//! `dump_actuals` 是第 1 步的引导器:`expect` 为空时它打印实际值,填完表
//! 之后就不再需要(它自己不会失败,所以留着不碍事,还能当回归时的对照窗)。

use std::path::PathBuf;
use wlwl_eval::Evaluator;
use wlwl_value::Value;

/// 尚未冻结的占位符。**不能**用空串当哨兵:`JOIN([], "-")` 的结果就是
/// 空串,拿空串当「未冻结」会让「已冻结且值为空」和「还没冻结」无法区分 ——
/// 而这正是冻结表最容易漏判的那类行。
const UNFROZEN: &str = "~UNFROZEN";

/// `expect` 以 `!E00xx <message>` 开头 → 期望该诊断(码 + 消息逐字);
/// 否则是结果的 `display()`。
struct Case {
    name: &'static str,
    src: &'static str,
    expect: &'static str,
}

const I: &str = r#"IMPORT("wlwl:std.collection", ["MAP"]);"#;

const CASES: &[Case] = &[
    // ── 1. MAP ──
    Case {
        name: "map_basic",
        src: r#"IMPORT("wlwl:std.collection", ["MAP"]); MAP([1, 2], FUN((x), *(x, 10)))"#,
        expect: "[10, 20]",
    },
    Case {
        name: "map_empty",
        src: r#"IMPORT("wlwl:std.collection", ["MAP"]); MAP([], FUN((x), x))"#,
        expect: "[]",
    },
    Case {
        name: "map_non_array",
        src: r#"IMPORT("wlwl:std.collection", ["MAP"]); MAP("no", FUN((x), x))"#,
        expect: "!E0030 MAP: expected array, got string",
    },
    Case {
        name: "map_non_callable",
        src: r#"IMPORT("wlwl:std.collection", ["MAP"]); MAP([1], 5)"#,
        expect: "!E0020 MAP: callback is not callable (got integer)",
    },
    Case {
        name: "map_arity",
        src: r#"IMPORT("wlwl:std.collection", ["MAP"]); MAP([1])"#,
        expect: "!E0022 MAP: function expects 2 argument(s), got 1",
    },
    Case {
        name: "map_arity_too_many",
        src: r#"IMPORT("wlwl:std.collection", ["MAP"]); MAP([1], FUN((x), x), 9)"#,
        expect: "!E0022 MAP: function expects 2 argument(s), got 3",
    },
    Case {
        name: "map_err_arg_transparent",
        src: r#"IMPORT("wlwl:std.collection", ["MAP"]); LET(r, MAP(ERR("boom"), FUN((x), x))); IS_ERR(r)"#,
        expect: "TRUE",
    },
    Case {
        name: "map_callback_err_transparent",
        src: r#"IMPORT("wlwl:std.collection", ["MAP"]); LET(g, FUN((x), IF(<(x, 0), ERR("neg"), x))); LET(r, MAP([1, -1], g)); UNWRAP_OR(r, "transparent")"#,
        expect: "transparent",
    },
    // ── 2. FILTER ──
    Case {
        name: "filter_basic",
        src: r#"IMPORT("wlwl:std.collection", ["FILTER"]); FILTER([1, 2, 3], FUN((x), >(x, 1)))"#,
        expect: "[2, 3]",
    },
    Case {
        name: "filter_all_out",
        src: r#"IMPORT("wlwl:std.collection", ["FILTER"]); FILTER([1, 2], FUN((x), FALSE))"#,
        expect: "[]",
    },
    Case {
        name: "filter_non_boolean_predicate",
        src: r#"IMPORT("wlwl:std.collection", ["FILTER"]); FILTER([1, 2], FUN((x), x))"#,
        expect: "!E0030 FILTER: predicate must return BOOLEAN, got integer",
    },
    Case {
        name: "filter_err_arg_transparent",
        src: r#"IMPORT("wlwl:std.collection", ["FILTER"]); LET(r, FILTER(ERR("boom"), FUN((x), TRUE))); IS_ERR(r)"#,
        expect: "TRUE",
    },
    // ── 3. REDUCE ──
    Case {
        name: "reduce_left_fold",
        src: r#"IMPORT("wlwl:std.collection", ["REDUCE"]); REDUCE([1, 2, 3], FUN((a, b), +(a, b)), 0)"#,
        expect: "6",
    },
    Case {
        name: "reduce_empty_returns_init",
        src: r#"IMPORT("wlwl:std.collection", ["REDUCE"]); REDUCE([], FUN((a, b), a), 42)"#,
        expect: "42",
    },
    Case {
        name: "reduce_arity",
        src: r#"IMPORT("wlwl:std.collection", ["REDUCE"]); REDUCE([1], FUN((a, b), a))"#,
        expect: "!E0022 REDUCE: function expects 3 argument(s), got 2",
    },
    Case {
        name: "reduce_callback_err",
        src: r#"IMPORT("wlwl:std.collection", ["REDUCE"]); LET(g, FUN((a, b), ERR("boom"))); LET(r, REDUCE([1, 2], g, 0)); IS_ERR(r)"#,
        expect: "TRUE",
    },
    // ── 4. SORT ──
    Case {
        name: "sort_ints",
        src: r#"IMPORT("wlwl:std.collection", ["SORT"]); SORT([3, 1, 2])"#,
        expect: "[1, 2, 3]",
    },
    Case {
        name: "sort_strings",
        src: r#"IMPORT("wlwl:std.collection", ["SORT"]); SORT(["b", "a"])"#,
        expect: "[a, b]",
    },
    Case {
        name: "sort_is_stable_for_incomparable",
        src: r#"IMPORT("wlwl:std.collection", ["SORT"]); SORT([1, "a", 2])"#,
        expect: "[1, a, 2]",
    },
    Case {
        name: "sort_mixed_numeric",
        src: r#"IMPORT("wlwl:std.collection", ["SORT"]); SORT([1.5, 1])"#,
        expect: "[1, 1.5]",
    },
    Case {
        name: "sort_custom_comparator",
        src: r#"IMPORT("wlwl:std.collection", ["SORT"]); SORT([1, 2, 3], FUN((a, b), >(a, b)))"#,
        expect: "[3, 2, 1]",
    },
    Case {
        name: "sort_comparator_non_boolean",
        src: r#"IMPORT("wlwl:std.collection", ["SORT"]); SORT([3, 1], FUN((a, b), 5))"#,
        expect: "!E0030 SORT: comparator must return BOOLEAN, got integer",
    },
    Case {
        name: "sort_comparator_non_boolean_string",
        src: r#"IMPORT("wlwl:std.collection", ["SORT"]); SORT([3, 1], FUN((a, b), "x"))"#,
        expect: "!E0030 SORT: comparator must return BOOLEAN, got string",
    },
    Case {
        name: "sort_arity_zero",
        src: r#"IMPORT("wlwl:std.collection", ["SORT"]); SORT()"#,
        expect: "!E0022 SORT: function expects 2 argument(s), got 0",
    },
    Case {
        name: "sort_arity_three",
        src: r#"IMPORT("wlwl:std.collection", ["SORT"]); SORT([1], FUN((a, b), b), 9)"#,
        expect: "!E0022 SORT: function expects 2 argument(s), got 3",
    },
    Case {
        name: "sort_non_array",
        src: r#"IMPORT("wlwl:std.collection", ["SORT"]); SORT(5)"#,
        expect: "!E0030 SORT: expected array, got integer",
    },
    Case {
        name: "sort_comparator_not_callable",
        src: r#"IMPORT("wlwl:std.collection", ["SORT"]); SORT([1], 5)"#,
        expect: "!E0020 SORT: callback is not callable (got integer)",
    },
    // ── 5. SORT_BY ──
    Case {
        name: "sort_by_derived",
        src: r#"IMPORT("wlwl:std.collection", ["SORT_BY"]); SORT_BY([[2, 1], [1, 2]], FUN((p), p[0]))"#,
        expect: "[[1, 2], [2, 1]]",
    },
    Case {
        name: "sort_by_key_err",
        src: r#"IMPORT("wlwl:std.collection", ["SORT_BY"]); LET(g, FUN((x), ERR("boom"))); LET(r, SORT_BY([1, 2], g)); IS_ERR(r)"#,
        expect: "TRUE",
    },
    Case {
        name: "sort_by_non_callable",
        src: r#"IMPORT("wlwl:std.collection", ["SORT_BY"]); SORT_BY([1], 5)"#,
        expect: "!E0020 SORT_BY: callback is not callable (got integer)",
    },
    // ── 6. ZIP ──
    Case {
        name: "zip_two",
        src: r#"IMPORT("wlwl:std.collection", ["ZIP"]); ZIP([1, 2], [3, 4, 5])"#,
        expect: "[[1, 3], [2, 4]]",
    },
    Case {
        name: "zip_non_array_is_single_column",
        src: r#"IMPORT("wlwl:std.collection", ["ZIP"]); ZIP(1, [1, 2])"#,
        expect: "[[1, 1]]",
    },
    Case {
        name: "zip_arity_zero",
        src: r#"IMPORT("wlwl:std.collection", ["ZIP"]); ZIP()"#,
        expect: "!E0022 ZIP: function expects 1 argument(s), got 0",
    },
    // ── 7. RANGE ──
    Case {
        name: "range_one_arg",
        src: r#"IMPORT("wlwl:std.collection", ["RANGE"]); RANGE(3)"#,
        expect: "[0, 1, 2]",
    },
    Case {
        name: "range_two_args",
        src: r#"IMPORT("wlwl:std.collection", ["RANGE"]); RANGE(2, 5)"#,
        expect: "[2, 3, 4]",
    },
    Case {
        name: "range_negative_step",
        src: r#"IMPORT("wlwl:std.collection", ["RANGE"]); RANGE(5, 0, -2)"#,
        expect: "[5, 3, 1]",
    },
    Case {
        name: "range_empty",
        src: r#"IMPORT("wlwl:std.collection", ["RANGE"]); RANGE(0, 0)"#,
        expect: "[]",
    },
    Case {
        name: "range_step_zero_is_e0038",
        src: r#"IMPORT("wlwl:std.collection", ["RANGE"]); RANGE(0, 0, 0)"#,
        expect: "!E0038 RANGE: step must be non-zero (got 0)",
    },
    Case {
        name: "range_non_integer",
        src: r#"IMPORT("wlwl:std.collection", ["RANGE"]); RANGE(1, 2.5)"#,
        expect: "!E0030 RANGE: expected integer, got float",
    },
    Case {
        name: "range_arity_four",
        src: r#"IMPORT("wlwl:std.collection", ["RANGE"]); RANGE(1, 2, 3, 4)"#,
        expect: "!E0022 RANGE: function expects 3 argument(s), got 4",
    },
    // ── 8/9. ANY / ALL ──
    Case {
        name: "any_truthiness",
        src: r#"IMPORT("wlwl:std.collection", ["ANY"]); ANY([FALSE, NULL, 1])"#,
        expect: "TRUE",
    },
    Case {
        name: "any_with_predicate",
        src: r#"IMPORT("wlwl:std.collection", ["ANY"]); ANY([1, 2], FUN((x), >(x, 1)))"#,
        expect: "TRUE",
    },
    Case {
        name: "any_empty",
        src: r#"IMPORT("wlwl:std.collection", ["ANY"]); ANY([])"#,
        expect: "FALSE",
    },
    Case {
        name: "all_truthiness",
        src: r#"IMPORT("wlwl:std.collection", ["ALL"]); ALL([TRUE, 1])"#,
        expect: "TRUE",
    },
    Case {
        name: "all_empty_is_true",
        src: r#"IMPORT("wlwl:std.collection", ["ALL"]); ALL([])"#,
        expect: "TRUE",
    },
    Case {
        name: "any_predicate_err",
        src: r#"IMPORT("wlwl:std.collection", ["ANY"]); LET(g, FUN((x), ERR("boom"))); LET(r, ANY([1, 2], g)); IS_ERR(r)"#,
        expect: "TRUE",
    },
    Case {
        name: "all_predicate_err",
        src: r#"IMPORT("wlwl:std.collection", ["ALL"]); LET(g, FUN((x), ERR("boom"))); LET(r, ALL([1, 2], g)); IS_ERR(r)"#,
        expect: "TRUE",
    },
    Case {
        name: "any_non_callable",
        src: r#"IMPORT("wlwl:std.collection", ["ANY"]); ANY([1], 5)"#,
        expect: "!E0020 ANY: callback is not callable (got integer)",
    },
    // ── 10. FIND ──
    Case {
        name: "find_first_match",
        src: r#"IMPORT("wlwl:std.collection", ["FIND"]); FIND([1, 2, 3], FUN((x), >(x, 2)))"#,
        expect: "3",
    },
    Case {
        name: "find_no_match_is_null",
        src: r#"IMPORT("wlwl:std.collection", ["FIND"]); FIND([1, 2], FUN((x), <(x, 0)))"#,
        expect: "NULL",
    },
    Case {
        name: "find_non_boolean_predicate",
        src: r#"IMPORT("wlwl:std.collection", ["FIND"]); FIND([1], FUN((x), x))"#,
        expect: "!E0030 FIND: predicate must return BOOLEAN, got integer",
    },
    Case {
        name: "find_predicate_err",
        src: r#"IMPORT("wlwl:std.collection", ["FIND"]); LET(g, FUN((x), ERR("boom"))); LET(r, FIND([1, 2], g)); IS_ERR(r)"#,
        expect: "TRUE",
    },
    // ── 11. ENUMERATE ──
    Case {
        name: "enumerate_pairs",
        src: r#"IMPORT("wlwl:std.collection", ["ENUMERATE"]); ENUMERATE(["a", "b"])"#,
        expect: "[[0, a], [1, b]]",
    },
    Case {
        name: "enumerate_empty",
        src: r#"IMPORT("wlwl:std.collection", ["ENUMERATE"]); ENUMERATE([])"#,
        expect: "[]",
    },
    // ── 12/13. TAKE / DROP ──
    Case {
        name: "take_basic",
        src: r#"IMPORT("wlwl:std.collection", ["TAKE"]); TAKE([1, 2, 3], 2)"#,
        expect: "[1, 2]",
    },
    Case {
        name: "take_over_len",
        src: r#"IMPORT("wlwl:std.collection", ["TAKE"]); TAKE([1, 2], 9)"#,
        expect: "[1, 2]",
    },
    Case {
        name: "take_negative_is_empty",
        src: r#"IMPORT("wlwl:std.collection", ["TAKE"]); TAKE([1, 2], -1)"#,
        expect: "[]",
    },
    Case {
        name: "take_non_integer",
        src: r#"IMPORT("wlwl:std.collection", ["TAKE"]); TAKE([1, 2], 1.5)"#,
        expect: "!E0030 TAKE: expected non-negative integer, got float",
    },
    Case {
        name: "drop_basic",
        src: r#"IMPORT("wlwl:std.collection", ["DROP"]); DROP([1, 2, 3], 1)"#,
        expect: "[2, 3]",
    },
    Case {
        name: "drop_negative_is_noop",
        src: r#"IMPORT("wlwl:std.collection", ["DROP"]); DROP([1, 2], -1)"#,
        expect: "[1, 2]",
    },
    Case {
        name: "drop_non_integer",
        src: r#"IMPORT("wlwl:std.collection", ["DROP"]); DROP([1, 2], "x")"#,
        expect: "!E0030 DROP: expected non-negative integer, got string",
    },
    // ── 14. FLAT ──
    Case {
        name: "flat_one_level",
        src: r#"IMPORT("wlwl:std.collection", ["FLAT"]); FLAT([1, [2, [3]]])"#,
        expect: "[1, 2, [3]]",
    },
    Case {
        name: "flat_empty",
        src: r#"IMPORT("wlwl:std.collection", ["FLAT"]); FLAT([])"#,
        expect: "[]",
    },
    // ── 15. UNIQ ──
    Case {
        name: "uniq_cross_type_numeric",
        src: r#"IMPORT("wlwl:std.collection", ["UNIQ"]); UNIQ([1, 1, 2, "1", 1.0])"#,
        expect: "[1, 2, 1]",
    },
    Case {
        name: "uniq_containers",
        src: r#"IMPORT("wlwl:std.collection", ["UNIQ"]); UNIQ([[1], [1], [2], ["a": 1], ["a": 1]])"#,
        expect: "[[1], [2], [a: 1]]",
    },
    Case {
        name: "uniq_non_array",
        src: r#"IMPORT("wlwl:std.collection", ["UNIQ"]); UNIQ(5)"#,
        expect: "!E0030 UNIQ: expected array, got integer",
    },
    // ── 16. GROUP_BY ──
    Case {
        name: "group_by_strings",
        src: r#"IMPORT("wlwl:std.collection", ["GROUP_BY"]); GROUP_BY(["b", "a", "bb"], FUN((x), x))"#,
        expect: "[b: [b], a: [a], bb: [bb]]",
    },
    Case {
        name: "group_by_integer_keys_are_stringified",
        src: r#"IMPORT("wlwl:std.collection", ["GROUP_BY"]); GROUP_BY([1, 2, 3], FUN((x), %(x, 2)))"#,
        expect: "[1: [1, 3], 0: [2]]",
    },
    Case {
        name: "group_by_key_err",
        src: r#"IMPORT("wlwl:std.collection", ["GROUP_BY"]); LET(g, FUN((x), ERR("boom"))); LET(r, GROUP_BY([1, 2], g)); IS_ERR(r)"#,
        expect: "TRUE",
    },
    Case {
        name: "group_by_non_callable",
        src: r#"IMPORT("wlwl:std.collection", ["GROUP_BY"]); GROUP_BY([1], 5)"#,
        expect: "!E0020 GROUP_BY: callback is not callable (got integer)",
    },
    // ── 17. JOIN ──
    Case {
        name: "join_renders_values",
        src: r#"IMPORT("wlwl:std.collection", ["JOIN"]); JOIN([1.5, TRUE, NULL, [1, 2], ["k": 1]], "|")"#,
        expect: "1.5|TRUE|NULL|[1, 2]|[k: 1]",
    },
    Case {
        name: "join_empty_array",
        src: r#"IMPORT("wlwl:std.collection", ["JOIN"]); JOIN([], "-")"#,
        expect: "",
    },
    Case {
        name: "join_non_string_separator",
        src: r#"IMPORT("wlwl:std.collection", ["JOIN"]); JOIN([1, 2], 5)"#,
        expect: "!E0030 JOIN: expected string (separator), got integer",
    },
    // ── ERR 透明性(§12.6)──
    Case {
        name: "err_arg_is_transparent",
        src: r#"IMPORT("wlwl:std.collection", ["SORT"]); LET(r, SORT(ERR("boom"), NULL)); IS_ERR(r)"#,
        expect: "TRUE",
    },
    // ── 成员面(§5 全表)──
    Case {
        name: "every_member_imports",
        src: r#"IMPORT("wlwl:std.collection", ["MAP", "FILTER", "REDUCE", "SORT", "SORT_BY", "ZIP", "RANGE", "ANY", "ALL", "FIND", "ENUMERATE", "TAKE", "DROP", "FLAT", "UNIQ", "GROUP_BY", "JOIN"]); LEN([MAP, FILTER, REDUCE, SORT, SORT_BY, ZIP, RANGE, ANY, ALL, FIND, ENUMERATE, TAKE, DROP, FLAT, UNIQ, GROUP_BY, JOIN])"#,
        expect: "17",
    },
];

fn scratch_dir() -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("wlwl_contract_{nanos}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

fn run(src: &str) -> Result<Value, String> {
    let ast = wlwl_parser::parse(src, "t.wll").expect("case source parses");
    let mut ev = Evaluator::new().with_base_dir(scratch_dir());
    match ev.eval(&ast) {
        Ok(v) => Ok(v),
        Err(e) => Err(format!(
            "!{} {}",
            e.diagnostic().code.as_str(),
            e.diagnostic().message
        )),
    }
}

fn actual(case: &Case) -> String {
    match run(case.src) {
        Ok(v) => v.display(),
        Err(s) => s,
    }
}

#[test]
fn collection_matches_the_frozen_r2_contract() {
    let mut bad = Vec::new();
    for case in CASES {
        let got = actual(case);
        if case.expect == UNFROZEN {
            bad.push(format!(
                "{name}: NOT FROZEN yet (actual = {got})",
                name = case.name
            ));
        } else if case.expect != got {
            bad.push(format!(
                "{name}:\n      frozen: {want}\n      actual: {got}",
                name = case.name,
                want = case.expect
            ));
        }
    }
    assert!(
        bad.is_empty(),
        "{} / {} case(s) diverged from the frozen R2 contract:\n  - {}",
        bad.len(),
        CASES.len(),
        bad.join("\n  - ")
    );
}

/// 引导器:打印全部用例的实际结果,用来冻结 `expect`。跑法
/// `cargo test -p wlwl-eval --test collection_contract -- --nocapture`。
#[test]
fn dump_actuals() {
    for case in CASES {
        println!(
            "CASE|{name}|{expect}|{actual}",
            name = case.name,
            expect = case.expect,
            actual = actual(case)
        );
    }
}

/// 反向守卫:表本身不能是空的,否则上面那条断言会退化成检查零件事。
#[test]
fn the_contract_table_is_not_empty() {
    assert!(CASES.len() >= 17, "case table collapsed");
    let frozen = CASES.iter().filter(|c| c.expect != UNFROZEN).count();
    assert_eq!(
        frozen,
        CASES.len(),
        "{}/{} rows are unfrozen — run `dump_actuals` and fill `expect` in",
        CASES.len() - frozen,
        CASES.len()
    );
    let mut names: Vec<&str> = CASES.iter().map(|c| c.name).collect();
    names.sort_unstable();
    let before = names.len();
    names.dedup();
    assert_eq!(before, names.len(), "duplicate case name");
}

/// 成员面 ↔ **标准库规范 §5 表格**的外部对照。
///
/// 这条是整个 M3-1 里唯一一条「应然」侧**不是** Rust 常量的断言,因而
/// 是防「两侧同源改坏、互相抵消」的那道闸:R2 时代的 `names_match_catalog`
/// 拿 `NAMES` 跟另一份 `NAMES` 比,恒真。
///
/// 口径:**集合**相等,不比顺序 —— §5 的表是一张成员清单,没有规定次序;
/// 而附录 A 的行内顺序是**生成的**,已由 `stdlib_appendix_a_sync` 锁定,
/// 不该由这里反向规定。
fn spec_path() -> std::path::PathBuf {
    // CARGO_MANIFEST_DIR = impl/crates/wlwl-eval
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
        .join("docs")
        .join("stdlib")
        .join("wlwl-stdlib-spec-v0.11.md")
}

/// 从 §5 的表格首列抽出成员名。
///
/// §5 有**合并单元格**的行:`SORT(arr)` / `SORT_BY(arr, key)`、
/// `TAKE(arr, n)` / `DROP(arr, n)`、`ANY(arr, f?)` / `ALL(arr, f?)`,
/// 以及 `RANGE` 的三种形态写在一格里。所以按 ` / ` 切分 —— 这个分隔符带
/// 两侧空格,签名内部不会出现(真出现的话 `SORT(arr, n)` 那种就切错了,
/// 断言会红);`RANGE` 切出三条同名,按首现去重。
fn spec_section5_members() -> Vec<String> {
    let text = std::fs::read_to_string(spec_path()).expect("stdlib spec is readable");
    let start = text
        .find("## 5 `std.collection`")
        .expect("§5 header exists");
    let end = text[start..]
        .find("\n## 6 ")
        .map(|i| start + i)
        .expect("§6 header follows §5");
    let mut out: Vec<String> = Vec::new();
    for line in text[start..end].lines() {
        let t = line.trim();
        if !t.starts_with('|') || t.contains("---") {
            continue;
        }
        let first = t.trim_start_matches('|').split('|').next().unwrap_or("");
        for cell in first.split(" / ") {
            let name: String = cell
                .trim()
                .trim_start_matches('`')
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
    }
    out
}

fn r1_collection_exports() -> Vec<String> {
    let src = wlwl_std::LANG_SOURCES
        .iter()
        .find(|s| s.path == "wlwl:std.collection")
        .expect("collection is an R1 module");
    wlwl_std::lang_exports(src.source)
}

#[test]
fn member_set_matches_the_spec_section_5_table() {
    let spec = spec_section5_members();
    let impls = r1_collection_exports();
    let mut a = spec.clone();
    let mut b = impls.clone();
    a.sort();
    b.sort();
    assert_eq!(
        a, b,
        "§5 table and the R1 EXPORT list disagree.\n  spec: {spec:?}\n  impl: {impls:?}"
    );
    assert_eq!(impls.len(), 17, "the §5 table declares 17 members");
    let mut seen = std::collections::HashSet::new();
    for n in &impls {
        assert!(seen.insert(n.as_str()), "{n} exported twice");
    }
}

/// 反向守卫:上面那条以「§5 解析出的成员集」为输入。§5 表格一旦被改成别的
/// 形状(比如列数变了、签名写法变了),解析器可能一个成员都抽不出来 ——
/// 那时断言会拿 `[]` 比 `[]`… 不,`impls` 侧非空所以仍会红;但若两侧同时
/// 为空就绿了。这条把「§5 至少解析出 17 个」钉住。
#[test]
fn the_spec_table_extractor_actually_finds_the_members() {
    let spec = spec_section5_members();
    assert_eq!(
        spec.len(),
        17,
        "§5 extractor found {} member(s): {spec:?} — the table's shape changed and the \
         extractor needs updating",
        spec.len()
    );
    for n in [
        "MAP",
        "FILTER",
        "REDUCE",
        "SORT",
        "SORT_BY",
        "RANGE",
        "ZIP",
        "ANY",
        "ALL",
        "FIND",
        "ENUMERATE",
        "TAKE",
        "DROP",
        "FLAT",
        "UNIQ",
        "GROUP_BY",
        "JOIN",
    ] {
        assert!(spec.iter().any(|m| m == n), "§5 extractor missed `{n}`");
    }
}

/// `IMPORT` 报未知名必须是 E0023 —— 与「kernel 不外泄」同一条契约面
/// 纪律的 collection 版本。
#[test]
fn unknown_collection_member_is_e0023() {
    let ast = wlwl_parser::parse(r#"IMPORT("wlwl:std.collection", ["NOPE"]); NULL"#, "t.wll")
        .expect("parse");
    let mut ev = Evaluator::new().with_base_dir(scratch_dir());
    let err = ev.eval(&ast).expect_err("unknown member must be rejected");
    assert_eq!(err.diagnostic().code, wlwl_error::ErrorCode::E0023);
}

/// `wlwl:std.collection` 不是全局内建:必须走 IMPORT 才可见(§10.6)。
#[test]
fn collection_is_not_a_global_builtin() {
    let ast = wlwl_parser::parse("MAP([1], 1)", "t.wll").expect("parse");
    let mut ev = Evaluator::new().with_base_dir(scratch_dir());
    let err = ev.eval(&ast).expect_err("bare MAP must be undefined");
    assert_eq!(err.diagnostic().code, wlwl_error::ErrorCode::E0020);
}

#[allow(dead_code)]
const _UNUSED_I: &str = I;
