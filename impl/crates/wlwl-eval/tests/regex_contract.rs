//! `wlwl:std.regex` 成员契约 —— 标准库规范 §16(v0.11.3 M6 / addendum-02)。
//!
//! **期望值全部来自 Python `re`(独立第二实现),不是本实现输出。**
//! Python 的 `re` 是 leftmost-first —— 与 §16 选定的语义同族(Go / Python /
//! JS / Java / Rust `regex` 全是),所以它可以当本成员的交叉核对判据。
//! 两处**刻意不跟 Python 走**,都在用例注记里写明:
//!
//! 1. `re.ASCII` 是必需的 —— 否则 Python 的 `\w` / `\d` / `\s` 是 Unicode,
//!    而 §16 把它们钉成 **ASCII-only**;
//! 2. **空匹配的邻接规则**照 RE2/Go 的文档行为(紧贴前一个匹配则跳过),
//!    Python 的 `finditer` 在这条上**不同**(它会照报紧贴的空匹配)。
//!
//! 取数脚本见 `addendum-02` §3.7 的记录:左最earliest 的六个判别式
//! (`a|ab` / `ab|a` / `ab|b` / `b|ab` / `a|ab|abc` / `abc|ab|a`)逐条对过 Python。
//!
//! **病态复杂度那条是本成员存在的理由**,所以它进契约表而不是只进单测。

use std::path::PathBuf;
use wlwl_eval::Evaluator;

struct Case {
    name: &'static str,
    src: &'static str,
    expect: &'static str,
}

const CASES: &[Case] = &[
    // ── RE:模式对象与编译期失败分界 ───────────────────────────────────
    Case {
        name: "re_returns_a_pattern_object",
        src: concat!(
            r#"IMPORT("wlwl:std.regex", ["RE"]); "#,
            r#"RE("(a)(b)")"#
        ),
        expect: "[pattern: (a)(b), groups: 2]",
    },
    Case {
        name: "re_group_count_is_zero_without_captures",
        src: concat!(
            r#"IMPORT("wlwl:std.regex", ["RE", "RE_GROUP_COUNT"]); "#,
            r#"RE_GROUP_COUNT(RE("abc"))"#
        ),
        expect: "0",
    },
    Case {
        name: "re_non_capturing_group_is_not_counted",
        src: concat!(
            r#"IMPORT("wlwl:std.regex", ["RE", "RE_GROUP_COUNT"]); "#,
            r#"RE_GROUP_COUNT(RE("(?:x)(y)"))"#
        ),
        expect: "1",
    },
    // 语法错 = **原生诊断 E0030 中止**,不是 ERR 值(§16 决策 2)。
    // 「带列号」是这条契约的价值所在:模式错在哪要一眼看见。
    Case {
        name: "re_syntax_error_is_e0030_with_a_column",
        src: concat!(
            r#"IMPORT("wlwl:std.regex", ["RE"]); "#,
            r#"RE("a(b")"#
        ),
        expect: "!E0030 RE: missing ')' at column 3 of the pattern",
    },
    Case {
        name: "re_unterminated_class_is_e0030",
        src: concat!(
            r#"IMPORT("wlwl:std.regex", ["RE"]); "#,
            r#"RE("[a-")"#
        ),
        expect: "!E0030 RE: unterminated character class at column 3 of the pattern",
    },
    // **必须拒**的三类(与线性时间互斥)。放进契约表是因为「拒得掉」和
    // 「能编译」一样是契约 —— 只测能编译的那种,回溯引擎也会全绿。
    Case {
        name: "re_rejects_lookahead",
        src: concat!(
            r#"IMPORT("wlwl:std.regex", ["RE"]); "#,
            r#"RE("(?=a)")"#
        ),
        expect: "!E0030 RE: lookahead is not supported (it would break linear time) at column 4 of the pattern",
    },
    Case {
        name: "re_rejects_backreference",
        src: concat!(
            r#"IMPORT("wlwl:std.regex", ["RE"]); "#,
            r#"RE("(a)\\1")"#
        ),
        expect: "!E0030 RE: backreferences are not supported (they would break linear time) at column 5 of the pattern",
    },
    Case {
        name: "re_rejects_named_group",
        src: concat!(
            r#"IMPORT("wlwl:std.regex", ["RE"]); "#,
            r#"RE("(?<name>a)")"#
        ),
        expect: "!E0030 RE: lookbehind and named groups are not supported in this batch at column 4 of the pattern",
    },
    Case {
        name: "re_rejects_an_enormous_repeat",
        src: concat!(
            r#"IMPORT("wlwl:std.regex", ["RE"]); "#,
            r#"RE("a{2000}")"#
        ),
        expect: "!E0030 RE: repeat count exceeds 1000 at column 7 of the pattern",
    },

    // ── RE_TEST:整串匹配 ─────────────────────────────────────────────
    Case {
        name: "re_test_full_match",
        src: concat!(
            r#"IMPORT("wlwl:std.regex", ["RE", "RE_TEST"]); "#,
            r#"RE_TEST(RE("abc"), "abc")"#
        ),
        expect: "TRUE",
    },
    Case {
        name: "re_test_is_anchored_at_both_ends",
        src: concat!(
            r#"IMPORT("wlwl:std.regex", ["RE", "RE_TEST"]); "#,
            r#"RE_TEST(RE("abc"), "abcd")"#
        ),
        expect: "FALSE",
    },
    Case {
        name: "re_test_anchor_does_not_span_lines_without_m_flag",
        src: concat!(
            r#"IMPORT("wlwl:std.regex", ["RE", "RE_TEST"]); "#,
            r#"RE_TEST(RE("^b$"), "a\nb")"#
        ),
        expect: "FALSE",
    },

    // ── RE_SEARCH:leftmost-first + 捕获 ──────────────────────────────
    Case {
        name: "re_search_reports_span_and_groups",
        src: concat!(
            r#"IMPORT("wlwl:std.regex", ["RE", "RE_SEARCH"]); "#,
            r#"RE_SEARCH(RE("(?i)([a-z]+)-([0-9]+)"), "xx abc-123 yy")"#
        ),
        // start / end 是**码点**下标(与 CHAR_AT / INDEX_OF 同一坐标系)。
        expect: "[matched: abc-123, start: 3, end: 10, groups: [abc-123, abc, 123]]",
    },
    Case {
        name: "re_search_no_match_is_null_not_an_error",
        src: concat!(
            r#"IMPORT("wlwl:std.regex", ["RE", "RE_SEARCH"]); "#,
            r#"RE_SEARCH(RE("abc"), "zzz")"#
        ),
        expect: "NULL",
    },
    // ⚠ **leftmost-first 的判别式**(Python `re` 逐条核对过):
    //    `a|ab` 在 "ab" 上取 "a"(分支 1 先试),`ab|a` 取 "ab"。
    //    左最长会答成相反的两条 —— 这是 §16 决策 1 的可观察形状。
    Case {
        name: "leftmost_first_prefers_the_earlier_alternative",
        src: concat!(
            r#"IMPORT("wlwl:std.regex", ["RE", "RE_SEARCH"]); "#,
            r#"RE_SEARCH(RE("a|ab"), "ab")"#
        ),
        expect: "[matched: a, start: 0, end: 1, groups: [a]]",
    },
    Case {
        name: "leftmost_first_prefers_the_longer_first_alternative",
        src: concat!(
            r#"IMPORT("wlwl:std.regex", ["RE", "RE_SEARCH"]); "#,
            r#"RE_SEARCH(RE("ab|a"), "ab")"#
        ),
        expect: "[matched: ab, start: 0, end: 2, groups: [ab]]",
    },
    Case {
        name: "lazy_quantifier_takes_the_shortest",
        src: concat!(
            r#"IMPORT("wlwl:std.regex", ["RE", "RE_SEARCH"]); "#,
            r#"RE_SEARCH(RE("a+?"), "aaa")"#
        ),
        expect: "[matched: a, start: 0, end: 1, groups: [a]]",
    },

    // ── RE_FIND_ALL ──────────────────────────────────────────────────
    Case {
        name: "find_all_returns_every_non_overlapping_match",
        src: concat!(
            r#"IMPORT("wlwl:std.regex", ["RE", "RE_FIND_ALL"]); "#,
            r#"LEN(RE_FIND_ALL(RE("[0-9]+"), "ab12cd345"))"#
        ),
        expect: "2",
    },
    Case {
        name: "find_all_empty_matches_terminate",
        src: concat!(
            r#"IMPORT("wlwl:std.regex", ["RE", "RE_FIND_ALL"]); "#,
            r#"LEN(RE_FIND_ALL(RE("a*"), "bab"))"#
        ),
        // 这条同时钉住「不会无限循环」。RE2/Go 的邻接规则:`a*` 在 "bab" 上
        // 报 3 个 —— 空(0)、"a"(1..2)、空(3);**紧贴 "a" 的那个空匹配被跳过**。
        // Python 的 finditer 在这里会给 4 个(它会照报 (2,2))—— 刻意不跟它。
        expect: "3",
    },

    // ── RE_REPLACE ───────────────────────────────────────────────────
    Case {
        name: "replace_with_numbered_group_refs",
        src: concat!(
            r#"IMPORT("wlwl:std.regex", ["RE", "RE_REPLACE"]); "#,
            r#"RE_REPLACE(RE("(a)(b)"), "ab", "$1-$2")"#
        ),
        expect: "a-b",
    },
    Case {
        name: "replace_swaps_capture_groups",
        src: concat!(
            r#"IMPORT("wlwl:std.regex", ["RE", "RE_REPLACE"]); "#,
            r#"RE_REPLACE(RE("(\\w+)@(\\w+)"), "user@host", "$2@$1")"#
        ),
        expect: "host@user",
    },
    Case {
        name: "replace_with_named_group_ref_is_rejected",
        src: concat!(
            r#"IMPORT("wlwl:std.regex", ["RE", "RE_REPLACE"]); "#,
            r#"RE_REPLACE(RE("(a)"), "a", +(+(+("$", "{"), "1"), "}"))"#
        ),
        // 命名组本批不做(§3.7.2-6)。见到 `${...}` **必须报**,
        // 不能静默原样输出 —— 那会让用户以为引用生效了。
        expect: concat!(
            "!E0030 RE_REPLACE: the replacement refers to a named group with ${...}, ",
            "but named groups are not supported in this batch"
        ),
    },

    // ── RE_SPLIT ─────────────────────────────────────────────────────
    Case {
        name: "split_keeps_empty_fields",
        src: concat!(
            r#"IMPORT("wlwl:std.regex", ["RE", "RE_SPLIT"]); "#,
            r#"RE_SPLIT(RE(","), "a,b,,c")"#
        ),
        expect: "[a, b, , c]",
    },
    Case {
        name: "split_on_space_runs_collapses",
        src: concat!(
            r#"IMPORT("wlwl:std.regex", ["RE", "RE_SPLIT"]); "#,
            r#"RE_SPLIT(RE("\\s+"), "a b  c")"#
        ),
        expect: "[a, b, c]",
    },

    // ── 元数 / 类型 ──────────────────────────────────────────────────
    Case {
        name: "re_arity_zero",
        src: concat!(
            r#"IMPORT("wlwl:std.regex", ["RE"]); "#,
            r#"RE()"#
        ),
        expect: "!E0022 RE: function expects 1 argument(s), got 0",
    },
    Case {
        name: "re_test_type_error_on_a_non_pattern_object",
        src: concat!(
            r#"IMPORT("wlwl:std.regex", ["RE_TEST"]); "#,
            r#"RE_TEST("abc", "abc")"#
        ),
        expect: concat!(
            "!E0030 RE_TEST: expected the dictionary returned by RE() for pattern object, ",
            "got string"
        ),
    },

    // ── **复杂度:本成员存在的理由** ───────────────────────────────────
    // `(a*)*b` 是回溯引擎的经典指数爆炸模式。没有 NFA 模拟的话,它在
    // 10 000 个 a 上根本跑不完 —— 所以这条用例「跑得完」本身就是断言。
    Case {
        name: "pathological_pattern_terminates_and_does_not_match",
        src: concat!(
            r#"IMPORT("wlwl:std.regex", ["RE", "RE_SEARCH"]); "#,
            r#"=(RE_SEARCH(RE("(a*)*b"), REPEAT("a", 10000)), NULL)"#
        ),
        expect: "TRUE",
    },
    Case {
        name: "pathological_pattern_still_matches_when_it_should",
        src: concat!(
            r#"IMPORT("wlwl:std.regex", ["RE", "RE_TEST"]); "#,
            r#"RE_TEST(RE("(a*)*b"), +(REPEAT("a", 10000), "b"))"#
        ),
        // 同一模式,输入末尾多个 b 就该匹配 —— 证明上一条不是「一律不匹配」。
        expect: "TRUE",
    },
];

fn scratch_dir() -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("wlwl_regex_{nanos}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

fn actual(src: &str) -> String {
    let ast =
        wlwl_parser::parse(src, "t.wll").unwrap_or_else(|e| panic!("case must parse: {src}\n{e}"));
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

#[test]
fn regex_contract() {
    let bad: Vec<String> = CASES
        .iter()
        .filter_map(|c| {
            let got = actual(c.src);
            if got == c.expect {
                None
            } else {
                Some(format!(
                    "{name}:\n      expected: {want}\n      actual:   {got}",
                    name = c.name,
                    want = c.expect
                ))
            }
        })
        .collect();
    assert!(
        bad.is_empty(),
        "wlwl:std.regex contract drift:\n    {}",
        bad.join("\n    ")
    );
}

/// 规范 §16 的成员表 ↔ `SPEC.functions`。**反向守卫**:表格形状变了而
/// 提取器抽不出成员时,`[] == []` 会绿 —— 所以正面锁住「正好 7 个」。
#[test]
fn regex_member_set_matches_the_spec_table() {
    let text = std::fs::read_to_string(spec_path()).expect("stdlib spec readable");
    let start = text.find("## 16 ").expect("§16 header exists");
    let end = text[start..]
        .find("\n## ")
        .map(|i| start + i)
        .expect("a following top-level heading exists");
    let body = &text[start..end];

    let mut spec: Vec<String> = Vec::new();
    for line in body.lines() {
        let Some(rest) = line.strip_prefix("| `") else {
            continue;
        };
        let name: String = rest
            .chars()
            .take_while(|c| *c != '`' && *c != '(')
            .collect();
        if !name.is_empty()
            && name
                .chars()
                .all(|c| c.is_ascii_uppercase() || c == '_' || c.is_ascii_digit())
        {
            spec.push(name);
        }
    }

    let impls: Vec<String> = wlwl_std::resolve("wlwl:std.regex")
        .expect("wlwl:std.regex resolves")
        .functions()
        .iter()
        .map(|(n, _)| (*n).to_string())
        .collect();

    assert_eq!(
        spec.len(),
        7,
        "§16 table extractor found {} member(s): {spec:?} — the table's shape changed \
         and the extractor needs updating",
        spec.len()
    );
    for n in [
        "RE",
        "RE_TEST",
        "RE_SEARCH",
        "RE_FIND_ALL",
        "RE_REPLACE",
        "RE_SPLIT",
        "RE_GROUP_COUNT",
    ] {
        assert!(
            spec.iter().any(|m| m == n),
            "§16 table missed `{n}`: {spec:?}"
        );
        assert!(
            impls.iter().any(|m| m == n),
            "implementation does not export `{n}`: {impls:?}"
        );
    }
    assert_eq!(impls.len(), 7, "wlwl:std.regex exports 7 members");
}

fn spec_path() -> PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
        .join("docs")
        .join("stdlib")
        .join("wlwl-stdlib-spec-v0.11.md")
}

/// 代码侧的锁与文档侧的锁必须成对 —— §16 里的「拒绝不支持的构造」与
/// 「左最早而非左最长」是最容易被下一个人「修实现去迁就文档」的两条。
#[test]
fn the_linearity_promise_is_documented() {
    let text = std::fs::read_to_string(spec_path()).expect("stdlib spec readable");
    for needle in [
        "leftmost-first",
        "leftmost-longest",
        "lookahead",
        "回溯引用",
        "ASCII",
        "addendum-02",
        "Pike",
    ] {
        assert!(
            text.contains(needle),
            "stdlib spec §16 lost `{needle}` — either the design changed on purpose (update \
             this test and §16 together) or the section was trimmed by accident"
        );
    }
}
