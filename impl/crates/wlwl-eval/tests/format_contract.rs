//! `wlwl:std.format` 成员契约 —— 标准库规范 §4。
//!
//! 单独成册而不是并进 `str_math_contract.rs`:§4 只导出一个成员,但要锁的是
//! **它与全局内建 `FORMAT` 之间的差异** —— 而全局内建不在任何模块的成员表里,
//! 放进按模块组织的契约册会让「谁在断言它」变得不清楚。
//!
//! ## 这册要防的事:「好心统一」
//!
//! 规范 §4 原文说成员「导出与全局内建 `FORMAT` 同一函数」。这句话在 `RESULT`
//! 值上**不成立**,而两条路径分属 eval 侧与 std 边界两处实现:
//!
//! | 路径 | `FORMAT("{0}", OK(1))` |
//! |---|---|
//! | 全局内建 | `OK(1)` |
//! | `wlwl:std.format` 成员 | `1` |
//!
//! v0.11.2 裁决(D12-003):**保留差异,只写清口径**。于是这册的职能就不是
//! 「证明实现符合规范」,而是**钉住一个有意保留的不一致** —— 没有它,下一个
//! 看到「明明能统一」的 agent 会去统一,那是行为变更,且没有经过裁决。
//!
//! 口径写在 `docs/stdlib/wlwl-stdlib-spec-v0.11.md` §4.1,`the_divergence_is_documented`
//! 盯着那段文字还在不在 —— 免得代码锁住了、口径却被人删了。

use std::path::PathBuf;
use wlwl_eval::Evaluator;

const UNFROZEN: &str = "~UNFROZEN";

struct Case {
    name: &'static str,
    src: &'static str,
    expect: &'static str,
}

const CASES: &[Case] = &[
    // ── 两条路径的分歧本体(§4.1)──────────────────────────────────────
    Case {
        name: "global_format_renders_result_literally",
        src: r#"FORMAT("{0}", OK(1))"#,
        expect: "OK(1)",
    },
    Case {
        name: "module_format_unwraps_ok_at_the_boundary",
        src: r#"IMPORT("wlwl:std.format", ["FORMAT"]); FORMAT("{0}", OK(1))"#,
        expect: "1",
    },
    // ── 同一分歧的镜像:ERR 值两条路径都到不了转换层 ──────────────────
    // 调用边界即短路传播,所以「成员路径会怎么渲染 ERR」不可观察。
    // 保留这条是为了把「不可观察」这件事本身钉住 —— 若将来 ERR 能穿过
    // 边界了,这条会红,提示 §4.1 那段话该改。
    Case {
        name: "result_error_is_unobservable_on_both_paths",
        src: r#"PRINT(FORMAT("{0}", ERR("x")))"#,
        expect: "!E0102 unhandled ERR escaped to top level: x",
    },
    // ── 基线:模板文法本身(语言规范 §10.7)────────────────────────────
    Case {
        name: "positional_placeholder",
        src: r#"FORMAT("a{0}b", 1)"#,
        expect: "a1b",
    },
    Case {
        name: "named_placeholder_takes_first_dict",
        src: r#"FORMAT("{k}", ["k": 7])"#,
        expect: "7",
    },
    Case {
        name: "unmatched_placeholder_is_preserved_literally",
        src: r#"FORMAT("{9}", 1)"#,
        expect: "{9}",
    },
    // ── 基线:两条路径在非 RESULT 值上**一致** ────────────────────────
    // 「差异只发生在 RESULT 上」是 §4.1 的适用范围。这两条把它钉住:
    // 若哪天差异扩散到普通值,§4.1 的措辞就得跟着改。
    Case {
        name: "both_paths_agree_on_plain_values",
        src: r#"IMPORT("wlwl:std.format", ["FORMAT"]); FORMAT("{0}/{1}", 1, "a")"#,
        expect: "1/a",
    },
    Case {
        name: "global_path_on_plain_values",
        src: r#"FORMAT("{0}/{1}", 1, "a")"#,
        expect: "1/a",
    },
    // ── 基线:导出面 ──────────────────────────────────────────────────
    Case {
        name: "format_is_not_exported_by_std_str",
        src: r#"IMPORT("wlwl:std.str", ["FORMAT"])"#,
        expect: "!E0023 'FORMAT' is not exported by module 'wlwl:std.str'",
    },
];

fn scratch_dir() -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("wlwl_format_{nanos}"));
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
fn format_contract() {
    let bad: Vec<String> = CASES
        .iter()
        .filter_map(|c| {
            let got = actual(c.src);
            if c.expect == UNFROZEN || got == c.expect {
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
        "wlwl:std.format contract drift:\n    {}",
        bad.join("\n    ")
    );
}

fn spec_path() -> PathBuf {
    // CARGO_MANIFEST_DIR = impl/crates/wlwl-eval;仓库根的 docs/ 在三级之上。
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
        .join("docs")
        .join("stdlib")
        .join("wlwl-stdlib-spec-v0.11.md")
}

/// §4.1 那段口径必须还在。
///
/// 代码侧的锁(`format_contract`)与文档侧的锁必须成对:只锁代码,下一个人
/// 会看到「测试说这两条路径不同」却找不到为什么,于是「修文档」;只锁文档,
/// 下一个人会「统一实现」去迁就文档。
#[test]
fn the_divergence_is_documented() {
    let text = std::fs::read_to_string(spec_path()).expect("stdlib spec readable");
    assert!(
        text.contains("刻意不同"),
        "stdlib spec §4.1 no longer records the RESULT-rendering divergence. \
         Either the divergence was resolved on purpose (update this test and §4 \
         together) or the section was dropped by accident."
    );
    assert!(
        text.contains("D12-003"),
        "stdlib spec §4.1 lost its deviation-ledger reference (D12-003)"
    );
}
