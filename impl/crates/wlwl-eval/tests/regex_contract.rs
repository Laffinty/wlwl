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

mod common;

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
    // ⚠🔴 [D13-031 / N-022]**预定义类不能当字符区间的端点。**
    //
    // 期望值出处:Go `regexp/syntax` 与 Python 都在这里报
    // `invalid character class range` / `bad character range` —— 预定义类
    // (一整类码点)不是一个码点,`a-\d` 没有合法含义。
    //
    // ⚠ 修复前本条**编译成功**,而且给出的是一个与作者意图毫无关系的类:
    // `\d` 被 `unescape_literal` 取成字符 `'d'`(0x64),区间 `a-d` 顺利通过
    // 校验 ⇒ `RE_TEST(RE("[a-\\d]"), "c")` 答 TRUE。**没有任何诊断。**
    Case {
        name: "rejects_a_predefined_class_as_a_range_endpoint",
        src: concat!(
            r#"IMPORT("wlwl:std.regex", ["RE"]); "#,
            r#"RE("[a-\\d]")"#
        ),
        expect: concat!(
            "!E0030 RE: `\\d` is a predefined class and cannot be a character ",
            "range endpoint at column 6 of the pattern"
        ),
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
            r#"IMPORT("wlwl:std.regex", ["RE", "RE_SEARCH"]); "#,
            r#"RE_SEARCH(RE("a+"), REPEAT("a", 200)).end"#
        ),
        // ⚠⚠ **这条必须打在 `RE_SEARCH` 上,不能拿 `RE_REPLACE` 代替。**
        // 负向验证实测:升序(bug 形态)下 `RE_SEARCH(RE("a+"), REPEAT("a", 200))`
        // 只报 `end = 65`,而**同一构建**下 `RE_REPLACE` / `RE_FIND_ALL` 走
        // `find_all`,拿到的是**完整长度 200**。⇒ 症状只落在 `best_hit` 那条
        // 路径上。初版契约用的是 `RE_REPLACE`(读起来更顺、也更接近「贪婪量词」
        // 的直觉),结果在 bug 形态下**全绿** —— 是一条假守卫,直到变异验证
        // 才把它拆穿。教训写进 `Hit::better_than` 的注记里了。
        expect: "200",
    },
    Case {
        name: "greedy_repeat_is_unbounded_in_find_all_too",
        src: concat!(
            r#"IMPORT("wlwl:std.regex", ["RE", "RE_FIND_ALL"]); "#,
            r#"LEN(RE_FIND_ALL(RE("a+"), REPEAT("a", 100)))"#
        ),
        expect: "1",
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

    // ── RE_FIND_ALL 的**每个左端**只出一个候选 ────────────────────────────
    //
    // ⚠ 这几条是 [N-024]「按左端去重 + 不排序」那笔性能改动的**行为守卫**。
    // 性能判据(定序比较次数、候选表条数)是整数,埋在 `regex.rs` 的
    // `#[cfg(test)]` 埋点里,只能落单测;**对外行为**的守卫在这里。
    //
    // 期望值出处全部是 **Python `re`**(`re.finditer` / `re.findall` 的实际
    // 输出,逐条跑过),**不是**本实现跑出来抄的 —— 那会把「优化改错了」也
    // 冻成基线。
    Case {
        name: "find_all_reports_spans_in_left_end_order",
        src: concat!(
            r#"IMPORT("wlwl:std.regex", ["RE", "RE_FIND_ALL"]); "#,
            r#"RE_FIND_ALL(RE("[0-9]+"), "ab12cd345")"#
        ),
        // Python `re.finditer(r"[0-9]+", "ab12cd345")` 的 `span()` 逐条核对:
        // (2, 4) = "12"、(6, 9) = "345"。**顺序也是期望值的一部分** ——
        // 选取阶段按起点升序遍历,去重之后起点键退化成了下标。
        expect: concat!(
            "[[matched: 12, start: 2, end: 4, groups: [12]], ",
            "[matched: 345, start: 6, end: 9, groups: [345]]]"
        ),
    },
    Case {
        name: "find_all_keeps_the_winning_captures_at_a_left_end",
        src: concat!(
            r#"IMPORT("wlwl:std.regex", ["RE", "RE_FIND_ALL"]); "#,
            r#"RE_FIND_ALL(RE("(a)(b)?"), "abab")"#
        ),
        // Python `re.findall(r"(a)(b)?", "abab")` = [('a','b'), ('a','b')]。
        // ⚠ 这一条专门钉「留的是**最优**那个候选,不是**先到**那个」:
        // 左端 0 上先后抵达过 (0,1) 组 2 未参与 与 (0,2) 组 2 = "b" 两个候选,
        // 留错的话 `groups` 会变成 `[ab, a, NULL]`。
        expect: concat!(
            "[[matched: ab, start: 0, end: 2, groups: [ab, a, b]], ",
            "[matched: ab, start: 2, end: 4, groups: [ab, a, b]]]"
        ),
    },
    Case {
        name: "find_all_reports_two_left_ends_in_order",
        src: concat!(
            r#"IMPORT("wlwl:std.regex", ["RE", "RE_FIND_ALL"]); "#,
            r#"RE_FIND_ALL(RE("a+"), "aa b aa")"#
        ),
        // Python `re.finditer(r"a+", "aa b aa")` 的 span() = (0,2)、(5,7)。
        expect: concat!(
            "[[matched: aa, start: 0, end: 2, groups: [aa]], ",
            "[matched: aa, start: 5, end: 7, groups: [aa]]]"
        ),
    },
    // `RE_SPLIT` / `RE_REPLACE` 建在 `find_all` 上 ⇒ 同一个改动的另外两条通路。
    // 之前只有 `RE_REPLACE` 的一条 leftmost-first 判别式挂在 `find_all` 上,
    // 而它只覆盖**一个**左端;「多个左端按升序排」这一半当时没人钉。
    Case {
        name: "split_reports_every_left_end_boundary",
        src: concat!(
            r#"IMPORT("wlwl:std.regex", ["RE", "RE_SPLIT"]); "#,
            r#"RE_SPLIT(RE("[0-9]+"), "ab12cd345x")"#
        ),
        // Python `re.split(r"[0-9]+", "ab12cd345x")` = ['ab', 'cd', 'x']。
        expect: "[ab, cd, x]",
    },
    Case {
        name: "replace_reports_every_left_end_boundary",
        src: concat!(
            r#"IMPORT("wlwl:std.regex", ["RE", "RE_REPLACE"]); "#,
            r#"RE_REPLACE(RE("(a+)"), "aa b aa", "<$1>")"#
        ),
        // Python `re.sub(r"(a+)", r"<\1>", "aa b aa")` = '<aa> b <aa>'。
        expect: "<aa> b <aa>",
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
    common::scratch("wlwl_regex")
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
    let start = text.find("## 标准库规范 · 16 ").expect("§16 header exists");
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
        .join("wlwl-spec-reference.md")
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

/// [D13-063 · N-025] 标志作用域与 `$` 锚点的**成文**守卫。
///
/// ⚠ 本批对这两条**一个字行为都没改**（`(?i)` 作用域要「在 group 层共享标志」,
/// 属结构变更,已登记待立项）⇒ **契约测不出差异**,能测的只有「文档里有没有
/// 如实写」—— 所以这是一条**文档断言**守卫,不是行为守卫。
///
/// ⚠ 它守的是一条**具体的错误陈述**:本实现历史上在 `parse_concat` 里写着
/// 「作用于本分支的余下部分(**RE2 的口径**)」,而 RE2 `doc/syntax.txt` 原文是
/// `(?flags)` = "set flags **within current group**" ⇒ 那半句是**错的**。
/// `review-1` 把它列为「未写进文件头六条」是对的,但没说代码里那句依据本身
/// 写反了。
///
/// ⚠ 同时钉住 §16 的**标题**从「三条」变成「四条」—— 少一条会被当成笔误,
/// 多一条才是「确实加了条款」。
#[test]
fn the_flag_scope_and_anchor_semantics_are_documented_honestly() {
    let text = std::fs::read_to_string(spec_path()).expect("stdlib spec readable");
    for needle in [
        "四条容易踩的细节",
        "within current group",     // RE2 对 (?flags) 的原文措辞
        "until outer paren closes", // RE2 同一句的中文版
        r"like `\z` not `\Z`",      // RE2 对 $ 的原文措辞
        "(a(?i)b|c)",               // 两口径**具体分歧**的那个形状
        "比 RE2 / Go 窄",
    ] {
        assert!(
            text.contains(needle),
            "stdlib spec §16 lost `{needle}` — the `(?i)` scope / `$` anchor semantics are \
             documented divergences, not silent behaviour. Either the section was trimmed or \
             someone 'fixed' the docs back into the old wrong claim."
        );
    }

    // ⚠ **反向守卫**：那句错误的「RE2 的口径」不得复活。
    //   源码里曾经写着「作用于本分支的余下部分(RE2 的口径)」—— 它把本实现
    //   说成 RE2 同款,而实际**更窄**。这条断言直接读源码文件。
    let src = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../wlwl-std/src/regex.rs"),
    )
    .expect("regex.rs readable");
    assert!(
        // ⚠ needle 必须**逐字节对上**源码。首版我按记忆写成
        //   `余下部分(RE2 的口径)`,而源码里是两个 `**` 夹着的
        //   `余下部分**(RE2 的口径)` ⇒ **断言永远为真**(needle 不存在),
        //   守卫静默空转。⚠ 这是本仓踩过的第 N 次「检查器自己先错」——
        //   **负向验证的价值就在这里**:不放回那句错注,这条断言根本没被测过。
        !src.contains("的余下部分**(RE2 的口径)"),
        "regex.rs must not claim the current `(?i)` scope IS RE2's — RE2's scope is the \
         enclosing group, this one is the current branch (D13-063)."
    );
}
///
/// ⚠ 期望值出处：Go `regexp/syntax` 与 Python 都拒「预定义类当端点」；末尾那
///   四条**合法**形态的依据是 RFC / Python 的字符类文法（连字符在首尾是字面量）。
///   **不是**「跑一遍本实现看它报什么」—— 那会把「静默放行」冻成基线。
///
/// ⚠ **反向守卫与正向守卫一样重要**：`[\d-]` 里的 `-` 是**字面量**（`]` 收尾）,
///   不构成区间端点,必须仍然编译成功。只写「要拒」的那几条,很容易顺手把
///   合法形态一起拒掉 —— 那是另一种静默破坏。
#[test]
fn a_predefined_class_may_not_be_a_range_endpoint_but_adjacent_forms_still_work() {
    for name in ["d", "w", "s", "D", "W", "S"] {
        for pattern in [
            format!("[a-\\{name}]"), // 高端
            format!("[\\{name}-x]"), // 低端
        ] {
            let got = actual(&format!(
                r#"IMPORT("wlwl:std.regex", ["RE"]); RE("{}")"#,
                pattern.replace('\\', "\\\\")
            ));
            assert!(
                got.starts_with("!E0030") && got.contains("cannot be a character range endpoint"),
                "pattern {pattern:?} must be rejected, got: {got}"
            );
        }
    }

    // 零宽断言当端点走的是同一个兜底分支 ⇒ 一并拒(D13-030 同源)。
    for pattern in [r"[0-\b]", r"[\A-x]"] {
        let got = actual(&format!(
            r#"IMPORT("wlwl:std.regex", ["RE"]); RE("{}")"#,
            pattern.replace('\\', "\\\\")
        ));
        assert!(
            got.starts_with("!E0030"),
            "pattern {pattern:?} must be rejected, got: {got}"
        );
    }

    // ⚠ 正向守卫:这些**合法**,不能被上面那道闸误伤。
    for pattern in [
        r"[\d]",  // 预定义类单独在类里
        r"[^\d]", // 取反
        r"[a\d]", // 与字面量混排
        r"[\d-]", // 连字符在**末尾** ⇒ 字面量,不是区间
        r"[-a]",  // 连字符在**开头** ⇒ 字面量
        r"[a-]",  // 同上
        r"[a-c]", // 纯字面区间(全仓原有的那唯一一条取反用例的同类)
    ] {
        let got = actual(&format!(
            r#"IMPORT("wlwl:std.regex", ["RE"]); RE("{}")"#,
            pattern.replace('\\', "\\\\")
        ));
        assert!(
            !got.starts_with('!'),
            "pattern {pattern:?} is legal and must still compile, got: {got}"
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
fn greedy_repeat_reports_the_whole_run_at_every_size_past_the_cap() {
    // ⚠ 期望值不是「拍脑袋」也不是「跑一遍抄下来」:`n` 就是输入长度,
    // 「贪婪量词必须匹配整个输入」是**语义定义**。这正是这条守卫的价值 ——
    // 它不需要知道 `PRIO_MAX` 是几,也不需要知道 bug 形态下会报多少,
    // **只要报的不是 `n` 就红**。初版那条只钉了单点(200),现在扫一整排,
    // 免得「换个长度就绕过去」。
    for n in [32usize, 63, 64, 65, 66, 70, 100, 200, 1000] {
        let got = actual(&format!(
            r#"IMPORT("wlwl:std.regex", ["RE", "RE_SEARCH"]); RE_SEARCH(RE("a+"), REPEAT("a", {n})).end"#
        ));
        assert_eq!(
            got,
            n.to_string(),
            "greedy a+ on {n} a's must report end = {n}"
        );
    }
}

/// [N-024] `find_all` 在**封顶点之后**仍报整串长度。
///
/// ⚠ **必须打在 `RE_FIND_ALL` 上,不能拿 `RE_SEARCH` 代替** —— 同款理由见
/// 上方 `greedy_repeat_survives_the_priority_cap` 的注记:症状只落在走
/// `best_hit` 的那条路径上,拿对的那条写契约才有效。这里反过来:封顶区
/// (`PRIO_MAX` 之后 `prio` 塌成同一向量)裁决权全落到抵达顺序那一键,
/// 而 [N-024] 那笔改动**正好把这一键的比较从「排序时」挪到了「抵达时」**
/// ⇒ 方向要是搞反成「先到者胜」,这一条会答 `end = 0`(空匹配)而不是整串。
///
/// ⚠ 期望值不是抄实现:`n` 就是输入长度,「贪婪量词匹配整个输入」是**语义
/// 定义** —— Python `len(re.search("a+", "a"*n).group())` 同样是 `n`。
#[test]
fn find_all_reports_the_whole_run_past_the_priority_cap() {
    for n in [32usize, 63, 64, 65, 66, 100, 200, 1000] {
        let a = "a".repeat(n);
        let got = actual(&format!(
            r#"IMPORT("wlwl:std.regex", ["RE", "RE_FIND_ALL"]); RE_FIND_ALL(RE("a+"), REPEAT("a", {n}))"#
        ));
        assert_eq!(
            got,
            format!("[[matched: {a}, start: 0, end: {n}, groups: [{a}]]]"),
            "greedy a+ on {n} a's must report one match spanning all {n}"
        );
    }
}

/// [N-024 / N-026] 性能守卫住在 `wlwl-std` 那边(埋点是 `#[cfg(test)]` 项,
/// 跨 crate 读不到)。这边负责的是**行为不许变** —— 两条性能改动合起来覆盖
/// `find_all` 的候选筛选与 VM 热路径,而 `RE_FIND_ALL` / `RE_REPLACE` /
/// `RE_SPLIT` 三个成员建在它上面,所以行为守卫必须三个都打到。
///
/// ⚠ 这里**不钉任何绝对耗时**:耗时在 CI 的机器上必然不稳。可数的代理指标
/// (定序比较次数、候选表条数、每步克隆次数)在 `regex.rs` 的单测里。
#[test]
fn find_all_family_members_agree_with_python_on_multi_match_shapes() {
    // 期望值出处:Python `re`(逐条跑过 `finditer` / `findall` / `split` / `sub`),
    // 不是本实现输出。多左端 + 捕获 + 空匹配邻接这三种形状各来一遍。
    for (name, src, want) in [
        (
            "multi-start greedy",
            r#"RE_FIND_ALL(RE("a+"), "aa b aa")"#,
            "[[matched: aa, start: 0, end: 2, groups: [aa]], [matched: aa, start: 5, end: 7, groups: [aa]]]",
        ),
        (
            "captures per start",
            r#"RE_FIND_ALL(RE("(a)(b)?"), "abab")"#,
            "[[matched: ab, start: 0, end: 2, groups: [ab, a, b]], [matched: ab, start: 2, end: 4, groups: [ab, a, b]]]",
        ),
        (
            "empty-match adjacency",
            r#"RE_FIND_ALL(RE("a*"), "bab")"#,
            "[[matched: , start: 0, end: 0, groups: []], [matched: a, start: 1, end: 2, groups: [a]], [matched: , start: 3, end: 3, groups: []]]",
        ),
    ] {
        let got = actual(&format!(
            r#"IMPORT("wlwl:std.regex", ["RE", "RE_FIND_ALL"]); {src}"#
        ));
        assert_eq!(got, want, "[{name}] drifted:\n  want: {want}\n  got:  {got}");
    }
}

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
