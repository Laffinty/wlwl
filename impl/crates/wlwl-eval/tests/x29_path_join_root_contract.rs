//! [X-29] `PATH_JOIN` 必须**保留首部的根** —— 标准库·系统册 §2 路径族。
//!
//! ## 它补的是什么
//!
//! 标准库·系统册 §2 的路径族表只写「用 `/` 连接,**合并重复分隔符**、
//! **跳过空片段**」—— **没有**任何「丢弃前导 `/`」的条款。
//!
//! 缺陷形态:前导 `/` 被「跳过空片段」这条逻辑**吃掉**
//! ⇒ `PATH_JOIN("/a", "b")` = `"a/b"`、`PATH_JOIN("/", "b")` = `"b"`
//! ⇒ **拼绝对路径会静默变成相对路径**(同一个输入在不同平台/不同基准目录下
//! 结果不同)。同族的 `PATH_NORMALIZE` / `PATH_CANONICAL` / `PATH_DIR`
//! **都保留根** ⇒ 本成员是这族里唯一的例外。
//!
//! ## 只对**首部**判根
//!
//! 中间段的前导 `/` 仍然被吃掉(`PATH_JOIN("a/", "/b")` = `"a/b"`)——
//! 那是「合并重复分隔符 / 跳过空片段」本身的语义,与根的问题**不是**一件事。
//!
//! ## 负向验证
//!
//! 把根标记去掉 ⇒ **本表至少 6 条变红**(`path_join.root.*` /
//! `path_join.root_only.*` / `path_join.absolute.*`)。

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

fn scratch_dir() -> PathBuf {
    std::env::temp_dir().join("wlwl_x29_path_join_root")
}

const CASES: &[Case] = &[
    // ── 首部的根**保留** ─────────────────────────────────────────────
    Case {
        name: "path_join.root_is_kept",
        src: concat!(
            r#"IMPORT("wlwl:std.fs", ["PATH_JOIN", "PATH_NORMALIZE"]); "#,
            r#"PATH_JOIN("/a", "b")"#
        ),
        expect: "/a/b",
    },
    Case {
        name: "path_join.root_only",
        src: concat!(
            r#"IMPORT("wlwl:std.fs", ["PATH_JOIN", "PATH_NORMALIZE"]); "#,
            r#"PATH_JOIN("/", "b")"#
        ),
        expect: "/b",
    },
    Case {
        name: "path_join.root_with_trailing_separators",
        src: concat!(
            r#"IMPORT("wlwl:std.fs", ["PATH_JOIN", "PATH_NORMALIZE"]); "#,
            r#"PATH_JOIN("/a/", "/b/")"#
        ),
        expect: "/a/b",
    },
    Case {
        name: "path_join.absolute_with_three_parts",
        src: concat!(
            r#"IMPORT("wlwl:std.fs", ["PATH_JOIN", "PATH_NORMALIZE"]); "#,
            r#"PATH_JOIN("/a", "b", "c")"#
        ),
        expect: "/a/b/c",
    },
    Case {
        name: "path_join.root_is_the_only_part",
        src: concat!(
            r#"IMPORT("wlwl:std.fs", ["PATH_JOIN", "PATH_NORMALIZE"]); "#,
            r#"PATH_JOIN("/")"#
        ),
        expect: "/",
    },
    // ── 相对路径**逐字不变**(回归守卫) ───────────────────────────────
    Case {
        name: "path_join.relative_is_unchanged",
        src: concat!(
            r#"IMPORT("wlwl:std.fs", ["PATH_JOIN", "PATH_NORMALIZE"]); "#,
            r#"PATH_JOIN("a", "b")"#
        ),
        expect: "a/b",
    },
    Case {
        name: "path_join.relative_with_empty_parts_is_unchanged",
        src: concat!(
            r#"IMPORT("wlwl:std.fs", ["PATH_JOIN", "PATH_NORMALIZE"]); "#,
            r#"PATH_JOIN("a", "", "b")"#
        ),
        expect: "a/b",
    },
    // ⚠ **中间段**的前导 `/` 仍然被吃掉(那是「跳过空片段」本身的语义)。
    Case {
        name: "path_join.middle_segment_separator_is_still_eaten",
        src: concat!(
            r#"IMPORT("wlwl:std.fs", ["PATH_JOIN", "PATH_NORMALIZE"]); "#,
            r#"PATH_JOIN("a/", "/b")"#
        ),
        expect: "a/b",
    },
    Case {
        name: "path_join.repeated_separators_are_still_collapsed",
        src: concat!(
            r#"IMPORT("wlwl:std.fs", ["PATH_JOIN", "PATH_NORMALIZE"]); "#,
            r#"PATH_JOIN("a/", "/b", "c")"#
        ),
        expect: "a/b/c",
    },
    // ⚠ Windows 盘符路径**逐字不变**(本成员不做盘符识别)。
    Case {
        name: "path_join.drive_letter_is_unchanged",
        src: concat!(
            r#"IMPORT("wlwl:std.fs", ["PATH_JOIN", "PATH_NORMALIZE"]); "#,
            r#"PATH_JOIN("C:/x", "b")"#
        ),
        expect: "C:/x/b",
    },
    // ── 与同族的 `PATH_NORMALIZE` **逐字一致**(同族口径守卫) ──────────
    Case {
        name: "path_join.agrees_with_path_normalize",
        src: concat!(
            r#"IMPORT("wlwl:std.fs", ["PATH_JOIN", "PATH_NORMALIZE"]); "#,
            r#"=(PATH_JOIN("/a", "b"), PATH_NORMALIZE(PATH_JOIN("/a", "b")))"#
        ),
        expect: "TRUE",
    },
];

#[test]
fn path_join_keeps_the_leading_root() {
    for c in CASES {
        let got = actual(c.src);
        assert_eq!(got, c.expect, "case {}: {}", c.name, c.src);
    }
}
