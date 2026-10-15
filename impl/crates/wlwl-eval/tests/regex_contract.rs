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
    // ⚠🔴 [D13-030 / N-021]**零宽断言转义必须编译期拒收**。
    //
    // 期望值出处:**本仓规范 §16.3**「`(?=…)` / `(?<=…)` / 回溯引用 / `(?<name>…)` /
    // `\p{…}` / 反向正则 / **零宽断言** —— 一律在编译期 `E0030`,不静默放行」,
    // 与 `regex.rs` 文件头同一句承诺。
    //
    // ⚠ 修复前本条答 `b`:`\b` 落到 `unescape_literal` 的兜底分支,被当成
    // **普通字面量**。从 Perl / Python / PCRE 过来的人写 `\bfoo\b` 期望
    // 「词边界」,拿到的是「字面量 bfoo 字面量 b」—— **静默给错结果**,
    // 而且模式**编译成功**、不报任何诊断。
    Case {
        name: "rejects_a_word_boundary_escape",
        src: concat!(
            r#"IMPORT("wlwl:std.regex", ["RE"]); "#,
            r#"RE("\\b")"#
        ),
        expect: concat!(
            "!E0030 RE: zero-width assertion `\\b` is not supported ",
            "(it would break linear time) at column 2 of the pattern"
        ),
        // 列号 2 = 那个 `b` 本身(1 基),即**被拒的那个转义字符**,不是 `\`。
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
    // ⚠ **封顶不得损语义**(D13-019 / N-003)。贪含量词每绕一圈 `Split`,
    //    `prio` 就长一格;长到 `PRIO_MAX`(64)之后不再增长,同一左端的多个
    //    候选 `prio` 塌成同一向量,裁决权落到第三键 `arrival`。
    //    100 个 a 远在封顶点之后 —— 修复前本条只匹配到 64/65 个。
    //    **期望值不是拍脑袋的**:贪心的定义就是「尽可能长」,Python
    //    `len(re.search("a+", "a"*100).group())` 同样是 100。
    Case {
        name: "greedy_repeat_survives_the_priority_cap",
        src: concat!(
            r#"IMPORT("wlwl:std.regex", ["RE", "RE_REPLACE"]); "#,
            r#"=(LEN(RE_REPLACE(RE("a+"), REPEAT("a", 100), "$0")), 100)"#
        ),
        // `$0` = 整体匹配。修复前本条只会还原出 64/65 个 a。
        expect: "TRUE",
    },
    Case {
        name: "greedy_repeat_is_unbounded_in_find_all_too",
        src: concat!(
            r#"IMPORT("wlwl:std.regex", ["RE", "RE_FIND_ALL", "RE_TEST"]); "#,
            r#"=(LEN(RE_FIND_ALL(RE("a+"), REPEAT("a", 100))), 1)"#
        ),
        expect: "TRUE",
    },

    // ── 取反字符类里的预定义类(D13-019 / N-002)────────────────────────
    // 整类的 `^` 在收尾处统一取补,类内**不得**再补一次。补两次 = 没补:
    // `[^\d]` 会等于 `\d`。这三条是判别式 —— 修复前它们全部答反。
    Case {
        name: "negated_class_over_a_predefined_class",
        src: concat!(
            r#"IMPORT("wlwl:std.regex", ["RE", "RE_TEST"]); "#,
            r#"=(RE_TEST(RE("[^\\d]"), " "), TRUE)"#
        ),
        expect: "TRUE",
    },
    Case {
        name: "negated_class_does_not_double_negate_word_chars",
        src: concat!(
            r#"IMPORT("wlwl:std.regex", ["RE", "RE_TEST"]); "#,
            r#"=(RE_TEST(RE("[^\\w]"), "a"), FALSE)"#
        ),
        // 修复前答 TRUE(等于把 `[^\w]` 当成了 `\w`)。
        expect: "TRUE",
    },
    Case {
        name: "negated_class_of_an_uppercase_predefined_class",
        src: concat!(
            r#"IMPORT("wlwl:std.regex", ["RE", "RE_TEST"]); "#,
            r#"=(RE_TEST(RE("[^\\D]"), "5"), TRUE)"#
        ),
        // `[^\D]` = 非「非数字」= 数字。修复前等于 `\D`,这条会答 FALSE。
        expect: "TRUE",
    },
    Case {
        name: "negated_class_mixes_literals_and_predefined",
        src: concat!(
            r#"IMPORT("wlwl:std.regex", ["RE", "RE_TEST"]); "#,
            r#"=(RE_TEST(RE("[^a-z\\d]"), "5"), FALSE)"#
        ),
        expect: "TRUE",
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
    // ⚠ `find_all` 的 leftmost-first(D13-019 / N-004)。上面的判别式全部走
    //    `RE_SEARCH`(经 `best_hit`),**照不到 `find_all` 这条路径** ——
    //    而 `RE_REPLACE` / `RE_SPLIT` 建在 `find_all` 上。
    //    修复前本条答 `["ab"]`:同一左端「后来的更长就取代」把分支 1 的 "a"
    //    顶掉了。Python `re.findall("a|ab", "ab")` 答 `['a']`。
    //    `RE_FIND_ALL` 返回的是**完整的捕获记录**(不是裸字符串),故期望值
    //    是 `[[...]]` 而非 `[a]`。
    Case {
        name: "find_all_honours_leftmost_first",
        src: concat!(
            r#"IMPORT("wlwl:std.regex", ["RE", "RE_FIND_ALL"]); "#,
            r#"RE_FIND_ALL(RE("a|ab"), "ab")"#
        ),
        expect: "[[matched: a, start: 0, end: 1, groups: [a]]]",
    },
    Case {
        name: "replace_honours_leftmost_first_too",
        src: concat!(
            r#"IMPORT("wlwl:std.regex", ["RE", "RE_REPLACE"]); "#,
            r#"RE_REPLACE(RE("(a)|ab"), "ab", "<$1>")"#
        ),
        // 建在 `find_all` 上 ⇒ 同一个 bug 的第二条通路。
        // 模式必须带捕获组:`a|ab` 的组号 1 不存在,`$1` 只会展开成空串,
        // 那就测不出「顶没顶掉」。
        expect: "<a>b",
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

/// [D13-030 / N-021]五个零宽断言转义**逐个**拒收,类内外一致。
///
/// ⚠ 期望值出处:**规范 §16.3**(零宽断言一律编译期 `E0030`)+ `regex.rs`
/// 文件头同一句承诺。**不是**「我跑一遍本实现看它报什么」—— 那会把
/// 「静默放行」也冻成基线,正是本条要防的。
///
/// ⚠ **类内也要拒**:`[\b]` 与类外 `\b` 是同一个 bug 的两个入口,只修一个
/// 会留下另一半。Go `regexp/syntax` 在类内同样报 `invalid escape sequence`。
#[test]
fn every_zero_width_escape_is_rejected_wherever_it_appears() {
    for name in ["b", "B", "A", "z", "Z"] {
        for pattern in [
            format!("\\{name}"),   // 类外
            format!("[\\{name}]"), // 字符类内
            format!("a\\{name}b"), // 嵌在别的内容中间
        ] {
            let got = actual(&format!(
                r#"IMPORT("wlwl:std.regex", ["RE"]); RE("{}")"#,
                pattern.replace('\\', "\\\\")
            ));
            assert!(
                got.starts_with("!E0030")
                    && got.contains(&format!("zero-width assertion `\\{name}` is not supported")),
                "pattern {pattern:?} must be rejected as a zero-width assertion, got: {got}"
            );
        }
    }
}

/// [D13-024 / N-005]括号嵌套必须有一道闸,且闸超限走 E0030 而不是爆栈。
///
/// ⚠ **这条守的是「失败分界」而不是「匹配结果」。** 栈溢出是 `abort`,不是
/// unwind —— `catch_unwind` 接不住,整个解释器进程被带走。所以判据是
/// 「诊断是 `!E0030`」而不是「没有崩」。
///
/// ⚠ **括号必须配平。** 只有开括号时,`parse_group` 会先撞上它自己的
/// 「pattern ends with an unclosed group」(`regex.rs` 里在递归**之前**就查
/// `peek().is_none()`),深度闸根本轮不到触发 —— 那样这条测试会在**闸完全
/// 没生效**的情况下变绿,正是它要防的那类假守卫。
///
/// 期望值的出处:**实现侧的 `MAX_NEST`**(`regex.rs`),不是本测试自己写的数字 ——
/// 改闸时两处必须同步。取值理由见 `MAX_NEST` 的文档注释:Go 的 1000 在本仓
/// 2 MiB 测试线程栈 + debug 构建下实测爆栈,故取 256。
///
/// 单独一条 `#[test]` 而不是进 `CASES` 表:几百个括号手写进 `&'static str`
/// 会撑爆可读性,照 `json_contract.rs` 的 `[`.repeat(200) 那个先例在 Rust 侧拼。
#[test]
fn deep_nesting_is_rejected_not_crashed() {
    const MAX_NEST: usize = 256; // regex.rs::MAX_NEST
    let nest = |n: usize| {
        format!(
            "IMPORT(\"wlwl:std.regex\", [\"RE\"]); RE(\"{}\")",
            "(".repeat(n) + &")".repeat(n)
        )
    };

    let got = actual(&nest(MAX_NEST + 1));
    assert!(
        got.starts_with("!E0030") && got.contains("pattern nests too deeply"),
        "nesting past {MAX_NEST} must be a graceful E0030, got: {got}"
    );

    // ⚠ 边界两侧都要断:证明那道闸是 {MAX_NEST} 而不是「一超过十几层就拒」
    // (与 `json_contract.rs` 那条 `ok_deep` 同一个道理)。
    let at_limit = actual(&nest(MAX_NEST));
    assert!(
        !at_limit.starts_with('!') && at_limit.ends_with(&format!(", groups: {MAX_NEST}]")),
        "nesting exactly at {MAX_NEST} must still compile all {MAX_NEST} groups, got: {at_limit}"
    );

    // 病态量级:修复前 20 000 层直接 abort。本测试进程若还能跑到这条断言,
    // 就说明闸起作用了 —— 它不能只在 257 层这种「温和」的输入上绿。
    let absurd = actual(&nest(20_000));
    assert!(
        absurd.starts_with("!E0030"),
        "20 000 nested groups must be E0030, got: {absurd}"
    );
}
