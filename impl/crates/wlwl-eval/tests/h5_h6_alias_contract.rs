//! [W-29 / v0.12.1 · 需求 H-5 / H-6] 两个**别名**的契约:
//! `wlwl:std.regex::RE_MATCHES_FULL`(= `RE_TEST`)与全局内建 `ARRAY_GET`(= `INDEX_GET`)。
//!
//! ## 两条需求各要什么(需求原文)
//!
//! - **H-5**:`RE_TEST` 是**整串锚定**(两端锚定、忽略 `(?m)`)⇒ 名字容易被读成
//!   「包含匹配」⇒ WAF 规则用它会**永远不命中**(静默漏防)。
//!   原文:「改名/**加别名**(如 `RE_MATCHES_FULL`),并在文档首行标注『整串锚定』」。
//!   ⇒ 本项**只加别名**:`RE_TEST` 不改名、不弃用、不改语义(一致性册 `:224` 禁删禁弃用)。
//! - **H-6**:`INDEX` 是 **1-based**、`INDEX_GET` 是 **0-based** —— 规范自认
//!   「最容易搞错的那个」。原文:「统一命名或提供 `ARRAY_GET` 别名,
//!   **并在错误信息里提示正确 API**」。
//!   ⇒ 本项**只加别名** + **错误信息里点名正确 API**。
//!
//! ## 「别名」的定义(本文件钉的就是这一条)
//!
//! 别名 = **同一个函数体** ⇒ 对同一批输入,**可观察行为逐字相同**:
//! 返回值逐字相同;诊断的**码**与**消息**也逐字相同 —— 别名**不另起一套消息**
//! (与仓里既有的 `POP`→`AT_K` / `DEL`→`REMOVE_KEY` 同款:诊断点名**规范名**)。
//!
//! ## 为什么「同一个函数体」是硬要求
//!
//! `INDEX_GET` 那一整套语义(0-based / `E0031` 非整数下标 / `E0036` 数组越界 /
//! `E0037` 字典缺键)若**重写一遍**,两份实现迟早漂移 ⇒ 别名必须**复用**
//! (分派表里两个名字指向**同一个函数指针**)。
//!
//! ## 反向守卫(本批逐条手工跑过,原始输出见交付记录)
//!
//! ① 从 `regex.rs` 的 `SPEC` 里去掉 `RE_MATCHES_FULL` ⇒ 本文件必红(未定义名)。
//! ② 从分派表里去掉 `ARRAY_GET` ⇒ 本文件必红,而 `INDEX_GET` 仍可用
//!    ⇒ 证明它是**别名**而不是**替代**。
//! ③ 把错误信息里那句提示去掉 ⇒ 本文件必红。

mod common;

use std::path::PathBuf;

use wlwl_eval::Evaluator;

/// 本文件要跑几十次求值 ⇒ 临时目录**建一次**就够(每次新建会白扔几十个目录)。
fn scratch_dir() -> PathBuf {
    static DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    DIR.get_or_init(|| common::scratch("wlwl_h5h6_alias"))
        .clone()
}

/// 求值一段 wlwl 源码,渲染成**可比对的字符串**(与 `regex_contract.rs` 同款)。
///
/// 诊断渲染成 `!CODE message` ⇒ 「码」与「消息」两样都能逐字比对。
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

// ── H-5:`RE_MATCHES_FULL` = `RE_TEST` 的别名 ─────────────────────────────────

/// [H-5] `RE_MATCHES_FULL` 与 `RE_TEST` 对**同一批输入**必须给出**逐字相同**的结果。
///
/// ⚠ 这批输入刻意覆盖三种最容易分家的形态:
/// **含 `\n` 的串** · **`(?m)` 模式** · **非整串的串**。
/// 期望值那一列钉的是「整串锚定」本身的语义 —— 两个名字**都**必须答它。
#[test]
fn re_matches_full_is_byte_identical_to_re_test() {
    // (模式, 主串, 期望)
    const BATCH: &[(&str, &str, &str)] = &[
        ("abc", "abc", "TRUE"),   // 整串命中
        ("abc", "abcd", "FALSE"), // 尾部多字符 ⇒ 非整串
        ("abc", "xabc", "FALSE"), // 头部多字符 ⇒ 非整串
        ("b", "abc", "FALSE"),    // ← WAF 那个「永远不命中」的形态
        ("a+", "aaa", "TRUE"),
        ("a+", r"aaa\n", "FALSE"),     // 整串 ⇒ 整个输入必须被消费完
        ("a.c", r"a\nc", "FALSE"),     // `.` 默认不含换行
        ("(?s)a.c", r"a\nc", "TRUE"),  // `(?s)` 打开
        ("^b$", r"a\nb", "FALSE"),     // 锚不跨行
        ("(?m)^b$", r"a\nb", "FALSE"), // `(?m)` 切行锚点,但**整串**仍要消费完整个输入
        ("[a-z]+", "abc", "TRUE"),
        ("[a-z]+", "abc1", "FALSE"),
    ];
    for (pattern, subject, want) in BATCH {
        let via_test = actual(&format!(
            r#"IMPORT("wlwl:std.regex", ["RE", "RE_TEST"]); RE_TEST(RE("{pattern}"), "{subject}")"#
        ));
        let via_alias = actual(&format!(
            r#"IMPORT("wlwl:std.regex", ["RE", "RE_MATCHES_FULL"]); RE_MATCHES_FULL(RE("{pattern}"), "{subject}")"#
        ));
        assert_eq!(
            via_test, via_alias,
            "RE_MATCHES_FULL must be a byte-identical alias of RE_TEST \
             (pattern /{pattern}/, subject {subject:?})"
        );
        assert_eq!(
            via_test, *want,
            "RE_TEST's own semantics drifted (pattern /{pattern}/, subject {subject:?})"
        );
    }
}

/// [H-5] `RE_MATCHES_FULL` 是 **`std.regex` 的模块成员**(不是全局内建):
/// 要 `IMPORT` 才能用,且必须出现在模块自己的成员表里。
#[test]
fn re_matches_full_is_a_regex_module_member() {
    let members: Vec<&str> = wlwl_std::resolve("wlwl:std.regex")
        .expect("wlwl:std.regex resolves")
        .functions()
        .iter()
        .map(|(n, _)| *n)
        .collect();
    assert!(
        members.contains(&"RE_MATCHES_FULL"),
        "RE_MATCHES_FULL must be registered in the std.regex SPEC: {members:?}"
    );
    // `RE_TEST` 本体**不许**消失(只加别名、不改名、不弃用)。
    assert!(
        members.contains(&"RE_TEST"),
        "RE_TEST must stay in the std.regex SPEC: {members:?}"
    );
    // 模块成员**免 IMPORT 不可用** —— 它不是全局内建。
    let got = actual(r#"IMPORT("wlwl:std.regex", ["RE"]); RE_MATCHES_FULL(RE("abc"), "abc")"#);
    assert!(
        got.starts_with('!') && got.contains("RE_MATCHES_FULL"),
        "RE_MATCHES_FULL is a module member: without importing it the name must be \
         undefined, got {got}"
    );
}

/// [H-5] `RE_MATCHES_FULL` 是**整串**语义 —— **不是**「包含匹配」。
///
/// 这一条钉的是需求本身(WAF 规则拿它当包含匹配会**永远不命中**)。
#[test]
fn re_matches_full_is_anchored_not_a_contains_match() {
    assert_eq!(
        actual(
            r#"IMPORT("wlwl:std.regex", ["RE", "RE_MATCHES_FULL"]); RE_MATCHES_FULL(RE("b"), "abc")"#
        ),
        "FALSE",
        "RE_MATCHES_FULL must be whole-string anchored, not a contains-match"
    );
    assert_eq!(
        actual(
            r#"IMPORT("wlwl:std.regex", ["RE", "RE_MATCHES_FULL"]); RE_MATCHES_FULL(RE("abc"), "abc")"#
        ),
        "TRUE",
        "RE_MATCHES_FULL must match a whole string"
    );
}

// ── H-6:`ARRAY_GET` = `INDEX_GET` 的别名 ─────────────────────────────────────

/// [H-6] `ARRAY_GET` 与 `INDEX_GET` 对**同一批实参**必须给出**逐字相同**的结果。
///
/// ⚠ 批次里既有**值**也有**四种失败形态**(非整数下标 / 数组越界 / 字典缺键 /
/// 类型错)—— 别名必须把**失败形态**也一起带上,而不只是成功路径。
#[test]
fn array_get_is_byte_identical_to_index_get() {
    // (实参, 期望) —— 期望值是「值」或「诊断码前缀」。
    const BATCH: &[(&str, &str)] = &[
        ("[10, 20, 30], 0", "10"),
        ("[10, 20, 30], 1", "20"),
        ("[10, 20, 30], 2", "30"),
        ("[10, 20, 30], -1", "30"),
        ("[\"k\": 7], \"k\"", "7"),
        ("\"abc\", 1", "b"),
        ("[10, 20, 30], 5", "!E0036"),
        ("[10, 20, 30], \"x\"", "!E0031"),
        ("[\"k\": 7], \"nope\"", "!E0037"),
        ("5, 0", "!E0030"),
    ];
    for (args, want) in BATCH {
        let via_index_get = actual(&format!("INDEX_GET({args})"));
        let via_array_get = actual(&format!("ARRAY_GET({args})"));
        assert_eq!(
            via_index_get, via_array_get,
            "ARRAY_GET must be a byte-identical alias of INDEX_GET (args: {args})"
        );
        assert!(
            via_index_get.starts_with(want),
            "unexpected result for ({args}): want {want}..., got {via_index_get}"
        );
    }
}

/// [H-6] `ARRAY_GET` 是**别名**,不是**替代** —— `INDEX_GET` 必须照旧可用。
#[test]
fn array_get_does_not_replace_index_get() {
    assert_eq!(actual("INDEX_GET([10, 20, 30], 1)"), "20");
    assert_eq!(actual("ARRAY_GET([10, 20, 30], 1)"), "20");
}

/// [H-6] `ARRAY_GET` 必须在**全局内建注册表**里登记 —— 分派表与元数据**成对**
/// (仓里的两条锁测试 `b11_registry_covers_resolve_builtin` /
/// `b11_resolve_builtin_covers_registry` 正是钉这一对)。
#[test]
fn array_get_is_registered_as_a_global_builtin() {
    let spec = wlwl_eval::registry::lookup("ARRAY_GET")
        .expect("ARRAY_GET must be in BUILTIN_REGISTRY (dispatch + metadata come in pairs)");
    assert_eq!(
        spec.signature,
        "ARRAY_GET(coll, k) -> v / E0031, E0036, E0037"
    );
    assert_eq!(spec.section, "§4.5");
    assert_eq!(spec.group, wlwl_eval::registry::BuiltinGroup::Subscript);
    assert_eq!(
        spec.err_consumer,
        wlwl_eval::registry::ErrConsumerStatus::No
    );
    assert_eq!(
        spec.dispatch,
        wlwl_eval::registry::DispatchStatus::ResolvedBuiltin
    );
    assert!(
        !spec.macro_fn,
        "ARRAY_GET is an ordinary builtin, not a macro"
    );
    // `INDEX_GET` 本体**不许**消失。
    assert!(wlwl_eval::registry::lookup("INDEX_GET").is_some());
}

// ── H-6:错误信息里提示正确 API ───────────────────────────────────────────────

/// [H-6] `AT_K` 只吃 DICT ⇒ 拿它取数组元素的人多半是把它当「安全下标」用了
/// ⇒ 错误信息必须**点名正确 API**。
#[test]
fn at_k_error_points_at_array_get() {
    let got = actual(r#"AT_K([1, 2], "k", "d")"#);
    assert!(
        got.starts_with("!E0030"),
        "AT_K on an ARRAY must stay E0030, got {got}"
    );
    assert!(
        got.contains("ARRAY_GET"),
        "AT_K's type error must name the correct API (`ARRAY_GET`): {got}"
    );
}

/// [H-6] 基号用混(拿 1-based 的 `INDEX` 返回值当 0-based 下标)是规范自认
/// 「最容易搞错的那个」⇒ 越界消息必须点明**基号**。
#[test]
fn array_oob_error_states_the_base() {
    let got = actual("ARRAY_GET([1, 2, 3], 3)");
    assert!(
        got.starts_with("!E0036"),
        "an out-of-range index must stay E0036, got {got}"
    );
    assert!(
        got.contains("0-based"),
        "the out-of-bounds message must state the base: {got}"
    );
    assert!(
        got.contains("1-based"),
        "the out-of-bounds message must contrast with INDEX's 1-based position: {got}"
    );
}

/// [H-6] 两个基号各自的形状(混用的两个来源)。
#[test]
fn index_is_one_based_and_array_get_is_zero_based() {
    assert_eq!(actual("INDEX([10, 20, 30], 20)"), "2", "INDEX is 1-based");
    assert_eq!(
        actual("ARRAY_GET([10, 20, 30], 1)"),
        "20",
        "ARRAY_GET is 0-based"
    );
    assert_eq!(
        actual("INDEX_GET([10, 20, 30], 1)"),
        "20",
        "INDEX_GET is 0-based"
    );
}
