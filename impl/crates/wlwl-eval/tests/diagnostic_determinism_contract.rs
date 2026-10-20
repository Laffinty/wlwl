//! 诊断确定性契约 —— 审计缺陷 **F-07**(`E0020` 的「did you mean」候选非确定)。
//!
//! ## 这册要防的事
//!
//! 同一个未定义名,**每次跑**给出的候选提示都不一样:
//!
//! ```text
//! did you mean one of: NOT, POP, SCOPE?   ← 第 1 次
//! did you mean one of: SCOPE, NOT, POP?   ← 第 2 次
//! did you mean one of: TYPE, POP, NOT?    ← 第 3 次
//! ```
//!
//! ⚠ **变的不是「排列」,是「选哪几个」也在变** —— 候选池里有 4 个
//!   (`NOT` / `POP` / `SCOPE` / `TYPE`),而展示上限是 3 ⇒ 每跑一次都是
//!   **4 选 3 的不同子集**。第三方报告写的「候选集合恒为」与实测不符。
//!
//! **根因**(见 `wlwl-eval/src/lib.rs` 的 `similar_names`):候选池是
//! `HashSet<String>`,而 Rust `HashSet` 的**迭代序每进程随机** ⇒ `scored` 的
//! 初始序随机;`sort_by_key(|(d, _)| *d)` 是**稳定排序** ⇒ 同距离项保持随机
//! 初始序;`truncate(max)` 于是**既随机化「选哪几个」,也随机化「这 3 个的排列」**。
//!
//! ## 为什么这道缺陷能长期潜伏
//!
//! 既有单测(`wlwl-eval/src/lib.rs` 的 `p3_008_undefined_name_suggests_similar`)
//! 只断言「**有** hint」,不断言「hint **稳定****」⇒ 缺陷在测试全绿的树里躺着。
//!
//! 而本仓赖以立足的「逐字节冻结」机制 —— stdlib §0.8 的四件套、ADR-0025 的
//! 基准守卫、各 crate 的 insta 快照 —— 正是**逐字节比对 stderr** 的 ⇒ 一道
//! 随机诊断文本会直接让那些门禁**随机红**。
//!
//! ## 断言口径
//!
//! 同一个未定义名**重复产生诊断 ≥ 50 次**,断言 hint 字符串**逐字节相同**。
//! 这里取 **64** 次(审计要求 ≥ 50)。
//!
//! ## 两个 case 各打**缺陷的一半**
//!
//! | case | 候选来源 | 打的哪一半 |
//! |---|---|---|
//! | `builtin_tie` | 全局内建名(4 个候选,展示 3 个) | 「**选哪几个**」随机 |
//! | `scope_tie` | 作用域里的 3 个**同距离**名字 | 「**排列**」随机(集合恒定) |
//!
//! ⚠ `scope_tie` **必须**单独测:只测 `builtin_tie` 的话,一条「把排序改成全序、
//!   但排序键仍**不含名字**」的实现仍可能蒙混过关 —— 集合对了,排列还随机。

mod common;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use wlwl_eval::Evaluator;

/// 审计要求「重复 ≥ 50 次」⇒ 取 64。
const REPEATS: usize = 64;

/// 被测源:(case 名, 被拼错的名字, 源码)。
const CASES: &[(&str, &str, &str)] = &[
    // 候选全来自**全局内建名**:池里 4 个(`NOT` / `POP` / `SCOPE` / `TYPE`),
    // 展示上限 3 个 ⇒ 每跑一次都可能截到**不同的子集**。
    ("builtin_tie", "NOPE", "PRINT(NOPE);"),
    // 候选全来自**作用域**:3 个**同距离**(`abcX` → `abc1` / `abc2` / `abc3`)
    // 的名字 ⇒ 集合恒定,但**排列**随机。
    //
    // ⚠ 名字刻意用**小写**:全局内建名都是大写,`abcX` 与它们的编辑距离 ≥ 4
    //   ⇒ 这个 case 的候选池**只**含作用域里的三个 ⇒ 断言它不受内建名册变动
    //   影响(内建名册由 `spec_appendix_g_sync.rs` 单独守)。
    (
        "scope_tie",
        "abcX",
        "LET(abc1, 0);\nLET(abc2, 0);\nLET(abc3, 0);\nPRINT(abcX);",
    ),
];

fn scratch_dir() -> PathBuf {
    common::scratch("wlwl_diagdet")
}

/// 跑一遍源,回显诊断的 `hint`(人类可读形态里唯一会渲染的字段)。
///
/// 顺带把 `suggestion_code` 里的「did you mean」那条 `Note` **一起**取出来
/// —— 实现侧把**同一串**文本同时写进 `hint` 与 `suggestion_code`,而 JSON 形态
/// (`--format=json`)走的是后者 ⇒ 两条路都得逐字节钉住。
///
/// ⚠ `suggestion_code` 里**不止一条** `Note`:`E0020` 另有一条泛用的
///   「no binding for this name…」 ⇒ 这里按**内容**挑,不是按位置。
fn hint_of(src: &str, dir: &Path) -> String {
    let ast = wlwl_parser::parse(src, "t.wll").expect("src must parse");
    let mut ev = Evaluator::new().with_base_dir(dir.to_path_buf());
    let err = ev.eval(&ast).expect_err("undefined name must fail");
    let d = err.diagnostic();
    assert_eq!(
        d.code,
        wlwl_error::ErrorCode::E0020,
        "case must be an undefined-name error"
    );
    let hint = d.hint.clone().expect("E0020 must carry a `hint`");
    let note = d
        .suggestion_code
        .iter()
        .filter_map(|s| match s {
            wlwl_error::Suggestion::Note { description } => Some(description.as_str()),
            _ => None,
        })
        .find(|desc| desc.contains("did you mean"))
        .expect("E0020 must carry a `did you mean` Note suggestion");
    assert_eq!(
        hint, note,
        "`hint` and the `did you mean` Note description must be the same text"
    );
    hint
}

/// 从 hint 里取出候选名(实现侧的文案形状:`did you mean one of: A, B, C?`)。
fn candidates(hint: &str) -> Vec<&str> {
    hint.trim_start_matches("did you mean one of: ")
        .trim_end_matches('?')
        .split(", ")
        .collect()
}

/// 编辑距离(Levenshtein)—— 与实现侧同一套度量,这里**独立重写**一遍,
/// 免得测试与实现共用一个函数而一起错。
fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut curr = vec![0usize; b.len() + 1];
    for i in 1..=a.len() {
        curr[0] = i;
        for j in 1..=b.len() {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[b.len()]
}

/// **主断言**:同一个未定义名重复产生诊断,hint **逐字节相同**。
#[test]
fn undefined_name_hint_is_byte_identical_across_runs() {
    let dir = scratch_dir();
    for (name, _, src) in CASES {
        let mut seen: BTreeSet<String> = BTreeSet::new();
        for _ in 0..REPEATS {
            seen.insert(hint_of(src, &dir));
        }
        assert_eq!(
            seen.len(),
            1,
            "case `{name}`: {REPEATS} runs produced {} distinct hints — \
             the E0020 candidate list is not deterministic:\n{:#?}",
            seen.len(),
            seen
        );
    }
}

/// **排序口径**:候选按 **(编辑距离, 名字字典序)** 排成全序。
///
/// ⚠ 钉的是**规则**而不是**字面量** —— 候选池里有什么是内建名册说了算
///   (那本名册由 `spec_appendix_g_sync.rs` 单独守),本册只管「顺序」。
#[test]
fn candidates_are_ordered_by_distance_then_name() {
    let dir = scratch_dir();
    for (name, target, src) in CASES {
        let hint = hint_of(src, &dir);
        let cands = candidates(&hint);
        for pair in cands.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            let ka = (levenshtein(target, a), a);
            let kb = (levenshtein(target, b), b);
            assert!(
                ka < kb,
                "case `{name}`: candidates are not in (distance, name) order — \
                 {a:?} (d={}) comes before {b:?} (d={}) in {hint:?}",
                ka.0,
                kb.0
            );
        }
    }
}

/// **排列口径(精确)**:同距离候选按**名字字典序**排。
///
/// 钉在 `scope_tie` 上 —— 它的候选池**只**含作用域里的三个名字 ⇒ 期望值
/// 不受内建名册变动影响。
#[test]
fn same_distance_candidates_are_in_name_order() {
    let dir = scratch_dir();
    let (_, _, src) = CASES
        .iter()
        .find(|(name, _, _)| *name == "scope_tie")
        .expect("scope_tie case exists");
    assert_eq!(hint_of(src, &dir), "did you mean one of: abc1, abc2, abc3?");
}

/// **非空洞守卫**:两个 case 都必须真的产出**多个候选**。
///
/// 若某个 case 的候选只剩一个,「hint 恒定」就变成「**因为没得选**而恒定」
/// ⇒ 缺陷复发时这条仍然绿 ⇒ 白测。
#[test]
fn cases_really_have_several_candidates() {
    let dir = scratch_dir();
    for (name, _, src) in CASES {
        let hint = hint_of(src, &dir);
        let cands = candidates(&hint);
        assert!(
            cands.len() >= 2,
            "case `{name}` produced only {} candidate(s) ({hint:?}) — \
             the determinism contract would be vacuous",
            cands.len()
        );
    }
}
