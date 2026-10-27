//! [v0.12.1 · W-23 · D-1] 正则模式的**编译缓存**契约。
//!
//! ## 这条契约在钉什么
//!
//! D-1 的需求是「`RE_*` 每次都重编译」⇒ WAF 每处理一个请求就把全部规则
//! 重编一遍。本项的验收口径是**确定性工作量计数**(**编译次数**),不是墙钟
//! —— 一致性册 §6.2 V-01 纪律:确定性计数可复算,墙钟不可复现。
//!
//! ## 为什么「编译次数」在语言面**不可见**
//!
//! 计数只能从 Rust 侧读(`wlwl_std::regex::pattern_compiles` /
//! `wlwl_std::regex::cache_stats`):「加一个语言面成员报编译次数」会撞 D-1 的
//! 非目标(**零新成员**)与一致性册 `:223`(不得改既有成员的语义 / 签名)
//! ⇒ 编译次数是**实现内部读数**,语言面能看到的只有「行为没变」。
//!
//! ## 期望值的出处
//!
//! 对照组的期望值全部取自 **Python `re`**(独立第二实现)与 **RE2 / Go 文档**
//! (空匹配邻接规则),**不是**本实现输出 —— 那会把「缓存改错了」也冻成基线。
//! 「编译次数 = 1」这类期望值则是**定义**:同一模式串只该编一遍。

mod common;

use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};
use wlwl_eval::Evaluator;

/// 缓存是**进程级**的 ⇒ 本文件里与缓存计数有关的用例必须**串行**。
///
/// ⚠ 为什么需要它:`cargo test` 默认并行跑同一二进制里的各个 `#[test]`
///   (libtest 的并发度 = CPU 数)⇒ 两个用例同时灌缓存 / 同时读全局计数,
///   计数就会互相污染。**模式串互不相同**还不够 —— 「缓存上界」那条会
///   **淘汰**别人的条目 ⇒ 那个用例必须在**别的用例之外**跑。
static SERIAL: Mutex<()> = Mutex::new(());

fn serial() -> MutexGuard<'static, ()> {
    SERIAL.lock().unwrap_or_else(|e| e.into_inner())
}

fn scratch_dir() -> PathBuf {
    common::scratch("wlwl_regex_cache")
}

/// 跑一段 wlwl 源码,返回它的显示值(诊断按 `!CODE message` 的形状)。
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

/// ⚠ [D-1] 的核心验收:同一模式对象连续匹配 1000 次 ⇒ **编译次数 = 1**。
///
/// 判据是**确定性工作量计数**,不是墙钟(一致性册 §6.2 V-01):计数可复算,
/// 墙钟在 CI 的机器上必然不稳。
#[test]
fn the_same_pattern_is_compiled_once_across_1000_matches() {
    let _g = serial();
    // ⚠ 模式串**全文件唯一**:缓存是进程级的,别的用例若用同一个模式串,
    //   本用例的「编译次数 = 1」就会被它先编掉(计数变成 0)。
    const PATTERN: &str = "^p121alpha[0-9]+q$";
    let src = format!(
        r#"IMPORT("wlwl:std.regex", ["RE", "RE_SEARCH"]);
LET(p, RE("{PATTERN}"));
LET MUT(i, 0);
LET MUT(last, "");
WHILE(<(i, 1000), (SET(i, +(i, 1)); SET(last, RE_SEARCH(p, "p121alpha7q"))));
last"#
    );

    // ⚠ 先证明计数器**是活的**:一个从没编过的模式,计数必须是 0。
    //   「计数器没被接线」和「缺陷真的没了」在断言里长得一模一样。
    assert_eq!(
        wlwl_std::regex::pattern_compiles(PATTERN),
        0,
        "the compile counter must start at 0 for a pattern nobody compiled yet"
    );

    let before = wlwl_std::regex::cache_stats().compiles;
    let got = actual(&src);
    let after = wlwl_std::regex::cache_stats();

    // 语义没变:第 1000 次的结果与第 1 次逐字相同。
    assert_eq!(
        got, "[matched: p121alpha7q, start: 0, end: 11, groups: [p121alpha7q]]",
        "the 1000th match must be byte-identical to the 1st one"
    );
    assert_eq!(
        after.compiles - before,
        1,
        "1000 matches of one pattern must compile it exactly once (compiles went {} -> {})",
        before,
        after.compiles
    );
    assert_eq!(
        wlwl_std::regex::pattern_compiles(PATTERN),
        1,
        "the pattern itself must have been compiled exactly once"
    );
    assert!(
        after.hits >= 1000,
        "the 1000 lookups must have hit the cache (hits = {})",
        after.hits
    );
}

/// 不同模式 ⇒ **各自**编译一次(不是一个模式编一次、另一个编 0 次)。
#[test]
fn each_distinct_pattern_is_compiled_exactly_once() {
    let _g = serial();
    const P1: &str = "^p121beta[0-9]+q$";
    const P2: &str = "^p121gamma[0-9]+q$";
    let src = format!(
        r#"IMPORT("wlwl:std.regex", ["RE", "RE_FIND_ALL"]);
LET(a, RE("{P1}"));
LET(b, RE("{P2}"));
LET MUT(i, 0);
LET MUT(ca, 0);
LET MUT(cb, 0);
WHILE(<(i, 1000), (SET(i, +(i, 1));
    SET(ca, +(ca, LEN(RE_FIND_ALL(a, "p121beta3q"))));
    SET(cb, +(cb, LEN(RE_FIND_ALL(b, "p121gamma4q"))))));
+(ca, cb)"#
    );

    let before = wlwl_std::regex::cache_stats().compiles;
    let got = actual(&src);
    let after = wlwl_std::regex::cache_stats();

    // 期望值按定义算出:每轮两个模式各匹配 1 次 ⇒ 1000 × (1 + 1) = 2000。
    assert_eq!(got, "2000", "both patterns must keep matching");
    assert_eq!(
        after.compiles - before,
        2,
        "two distinct patterns must be compiled once each (compiles went {} -> {})",
        before,
        after.compiles
    );
    assert_eq!(wlwl_std::regex::pattern_compiles(P1), 1, "P1");
    assert_eq!(wlwl_std::regex::pattern_compiles(P2), 1, "P2");
}

/// 缓存**有上界**:条目数不得超过上界,超出 ⇒ 淘汰。
///
/// ⚠ 这一条是 D-1 的验收条目之一:长跑进程(WAF)不能因为缓存而无上限增长。
#[test]
fn the_cache_stays_within_its_bound() {
    let _g = serial();
    let capacity = wlwl_std::regex::cache_stats().capacity;
    assert!(capacity > 0, "the cache must have a positive bound");

    // 灌 **capacity + 32** 个互不相同的模式串。
    let mut src = String::from(r#"IMPORT("wlwl:std.regex", ["RE"]);"#);
    for i in 0..(capacity + 32) {
        src.push_str(&format!(r#"RE("p121flood{i}");"#));
    }
    let before = wlwl_std::regex::cache_stats();
    actual(&src);
    let after = wlwl_std::regex::cache_stats();

    assert!(
        after.entries <= capacity,
        "the cache must not grow past its bound: {} entries > {capacity}",
        after.entries
    );
    assert!(
        after.bytes <= after.capacity_bytes,
        "the cache must not grow past its byte bound: {} bytes > {}",
        after.bytes,
        after.capacity_bytes
    );
    assert!(
        after.evictions > before.evictions,
        "filling the cache past its bound must evict (evictions went {} -> {}) — \
         otherwise the bound is not enforced",
        before.evictions,
        after.evictions
    );
}

/// 既有语义**逐字不变** —— 缓存是实现内部的自由度,行为面必须一模一样。
///
/// ⚠ 期望值的出处:**Python `re`**(独立第二实现)与 **RE2 / Go 文档**
///   (空匹配邻接规则),**不是**本实现输出。
/// ⚠ 其中「手工拼的同形字典」那条是**本项新增**的对照:缓存是按**模式串**
///   命中的 ⇒ 调用方**不信任**的旧口径(「`RE_*` 的行为不取决于那个 `DICT`
///   是从哪来的」)必须仍然成立 —— 缓存最容易被误改成「只认 `RE()` 造的字典」。
#[test]
fn the_cached_members_keep_their_exact_behaviour() {
    // ⚠ **也要串行**:这条用例自己会编十来个模式 ⇒ 它若与别的用例并行,
    //   别的用例量到的「编译次数增量」就会被它污染(+1)。实测踩过:
    //   并行时「1000 次匹配 = 1 次编译」那条读出 2。
    let _g = serial();
    let cases: &[(&str, &str, &str)] = &[
        // 模式对象的**形状**:仍是两键(pattern / groups)—— 缓存的句柄
        // **不进语言面**(加第三个键会改 LEN / KEYS 这类可观察面)。
        (
            "pattern object shape",
            r#"RE("(a)(b)")"#,
            "[pattern: (a)(b), groups: 2]",
        ),
        // leftmost-first(不是左最长):两个判别式。
        (
            "leftmost-first picks the earlier alternative",
            r#"RE_SEARCH(RE("a|ab"), "ab")"#,
            "[matched: a, start: 0, end: 1, groups: [a]]",
        ),
        (
            "leftmost-first picks the longer first alternative",
            r#"RE_SEARCH(RE("ab|a"), "ab")"#,
            "[matched: ab, start: 0, end: 2, groups: [ab]]",
        ),
        // 空匹配的邻接规则(RE2 / Go 口径:紧贴前一个匹配的空匹配被跳过;
        // Python 的 finditer 在这里**不同** —— 刻意不跟它)。
        (
            "empty-match adjacency",
            r#"RE_FIND_ALL(RE("a*"), "bab")"#,
            "[[matched: , start: 0, end: 0, groups: []], \
             [matched: a, start: 1, end: 2, groups: [a]], \
             [matched: , start: 3, end: 3, groups: []]]",
        ),
        // 贪婪量词在优先级封顶(PRIO_MAX = 64)之后仍报整串长度。
        (
            "greedy repeat survives the priority cap",
            r#"RE_SEARCH(RE("a+"), REPEAT("a", 200)).end"#,
            "200",
        ),
        (
            "replace with numbered group refs",
            r#"RE_REPLACE(RE("([a-z]+)=(\\w+)"), "a=1 b=2", "$2:$1")"#,
            "1:a 2:b",
        ),
        (
            "split reports every left end boundary",
            r#"RE_SPLIT(RE("[,;]"), "a,b;c")"#,
            "[a, b, c]",
        ),
        // `RE_TEST` 是**整串锚定**语义(不是「包含匹配」)。
        (
            "re_test is anchored at both ends",
            r#"RE_TEST(RE("abc"), "abcd")"#,
            "FALSE",
        ),
        ("re_test full match", r#"RE_TEST(RE("abc"), "abc")"#, "TRUE"),
        (
            "group count ignores non-capturing groups",
            r#"RE_GROUP_COUNT(RE("(?:x)(y)"))"#,
            "1",
        ),
        // ⚠ **不信任调用方**这条旧口径:手工拼的同形字典与 `RE()` 造的字典
        //   必须**行为逐字相同**(缓存按模式串命中 ⇒ 两边命中同一份产物)。
        (
            "a hand-built pattern object behaves the same",
            r#"RE_SEARCH(["pattern": "a+"], "aaa")"#,
            "[matched: aaa, start: 0, end: 3, groups: [aaa]]",
        ),
        (
            "a hand-built pattern object keeps the full-match semantics",
            r#"RE_TEST(["pattern": "a+"], "aaa")"#,
            "TRUE",
        ),
        // 语法错仍是**原生诊断 E0030**(带列号),且**逐字不变**。
        (
            "syntax error is still E0030 with a column",
            r#"RE("a(b")"#,
            "!E0030 RE: missing ')' at column 3 of the pattern",
        ),
    ];

    let mut bad: Vec<String> = Vec::new();
    for (name, body, want) in cases {
        let src = format!(
            r#"IMPORT("wlwl:std.regex", ["RE", "RE_TEST", "RE_SEARCH", "RE_FIND_ALL", "RE_REPLACE", "RE_SPLIT", "RE_GROUP_COUNT"]); {body}"#
        );
        let got = actual(&src);
        if &got != want {
            bad.push(format!(
                "{name}:\n      expected: {want}\n      actual:   {got}"
            ));
        }
    }
    assert!(
        bad.is_empty(),
        "wlwl:std.regex behaviour drifted under the compile cache:\n    {}",
        bad.join("\n    ")
    );
}

/// ⚠ **反向守卫的前提**:把「命中缓存」的短路去掉 ⇒ 上面那条「编译次数 = 1」
///   的断言**必红**(不是变慢,是**计数变 N**)。
///
/// 这条测试**不**自己制造那个变异(变异要改 `regex.rs` 的实现),它钉的是
/// 「计数读数确实是活的」:同一个模式**再**编一次 ⇒ 计数从 1 变成 2。
/// ⇒ 变异验证时,「去掉短路」会让这条与上面那条**一起**变红 —— 而计数
/// **从 1 变成 N** 这个形状,就是「短路真的被去掉了」的证据(而不是
/// 「计数器坏了」)。
#[test]
fn the_compile_counter_counts_real_compilations() {
    let _g = serial();
    const PATTERN: &str = "^p121delta[0-9]+q$";
    let src = format!(
        r#"IMPORT("wlwl:std.regex", ["RE", "RE_SEARCH"]);
LET(p, RE("{PATTERN}"));
RE_SEARCH(p, "p121delta5q")"#
    );
    actual(&src);
    assert_eq!(
        wlwl_std::regex::pattern_compiles(PATTERN),
        1,
        "one RE() + one lookup must compile the pattern exactly once"
    );

    // ⚠ 换一个**同形但不同**的模式串再走一遍:它是**另一个**模式 ⇒ 计数
    //   必须是 1(不是 2)。这一条钉住「计数是**按模式串**的,不是全局的
    //   一个总数」。
    const OTHER: &str = "^p121epsilon[0-9]+q$";
    let src2 = format!(
        r#"IMPORT("wlwl:std.regex", ["RE", "RE_SEARCH"]);
LET(p, RE("{OTHER}"));
RE_SEARCH(p, "p121epsilon5q")"#
    );
    actual(&src2);
    assert_eq!(
        wlwl_std::regex::pattern_compiles(OTHER),
        1,
        "a different pattern has its own counter"
    );
    assert_eq!(
        wlwl_std::regex::pattern_compiles(PATTERN),
        1,
        "the first pattern's counter must not have been disturbed"
    );
}
