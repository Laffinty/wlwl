//! `wlwl:std.fs` + `wlwl:std.env` 成员契约 —— 标准库规范 §2 / §18
//! (v0.11.3 M6 / addendum-06)。
//!
//! ## 这册钉的是**跨平台形状**,不是「跑通就行」
//!
//! `PATH_*` 全部是**纯函数**,可以**全量表驱动逐字冻结** —— 而这是本册存在的
//! 主要理由:2026-10-08 核出的现状是「wlwl 的路径是什么」**从未被裁决过**,
//! 它只是「Rust 是什么」的影子(实现把 path 原样交给 `std::fs`)。裁决成
//! **规范分隔符恒为 `/`** 之后,`PATH_JOIN` / `PATH_NORMALIZE` 的返回值就
//! **与平台无关** ⇒ 可以逐字节冻结,也可以在 Windows 与 Linux 上**产出相同**。
//!
//! ⚠️ **边界用例比顺路用例重要**:`PATH_DIR("a")` 返 `""` 而不是 `"."`
//! (为了让 `PATH_JOIN(PATH_DIR(x), PATH_BASE(x))` 成立)、`PATH_EXT(".gitignore")`
//! 返 `""`(点文件**没有**扩展名)。这几条都是「看起来像小事、错了会让一串
//! 路径操作静默走偏」的地方。
//!
//! ## 目录 / 字节用例一律在**临时目录**内自建自清
//!
//! 依赖真实文件系统状态(固定路径、残留文件)会让契约在 CI 上偶发红。

mod common;

use std::path::PathBuf;
use wlwl_eval::Evaluator;

struct Case {
    name: &'static str,
    src: &'static str,
    expect: &'static str,
}

const CASES: &[Case] = &[
    // ── PATH_* :纯函数,全量表驱动 ───────────────────────────────────
    Case {
        name: "path_join_uses_the_spec_separator_always",
        src: concat!(
            r#"IMPORT("wlwl:std.fs", ["PATH_JOIN", "PATH_DIR", "PATH_BASE", "PATH_EXT", "PATH_NORMALIZE"]); "#,
            r#"PATH_JOIN("a", "b", "c")"#
        ),
        // ⚠️ 跨平台**逐字**冻结:平台原生分隔符会让这条在 Windows 上产出
        // `a\b\c` ⇒ 契约表无法成立。这条就是「Q1 裁决成甲」的守卫。
        expect: "a/b/c",
    },
    Case {
        name: "path_join_collapses_repeated_separators",
        src: concat!(
            r#"IMPORT("wlwl:std.fs", ["PATH_JOIN", "PATH_DIR", "PATH_BASE", "PATH_EXT", "PATH_NORMALIZE"]); "#,
            r#"PATH_JOIN("a/", "/b", "c")"#
        ),
        expect: "a/b/c",
    },
    Case {
        name: "path_join_skips_empty_segments",
        src: concat!(
            r#"IMPORT("wlwl:std.fs", ["PATH_JOIN", "PATH_DIR", "PATH_BASE", "PATH_EXT", "PATH_NORMALIZE"]); "#,
            r#"PATH_JOIN("a", "", "b")"#
        ),
        // 让 `FILTER(MAP(...), 空则丢弃)` 的结果能直接拼。
        expect: "a/b",
    },
    Case {
        name: "path_join_needs_at_least_one_argument",
        src: concat!(
            r#"IMPORT("wlwl:std.fs", ["PATH_JOIN", "PATH_DIR", "PATH_BASE", "PATH_EXT", "PATH_NORMALIZE"]); "#,
            r#"PATH_JOIN()"#
        ),
        expect: "!E0022 PATH_JOIN: function expects 1 argument(s), got 0",
    },
    Case {
        name: "path_dir_of_a_nested_path",
        src: concat!(
            r#"IMPORT("wlwl:std.fs", ["PATH_JOIN", "PATH_DIR", "PATH_BASE", "PATH_EXT", "PATH_NORMALIZE"]); "#,
            r#"PATH_DIR("a/b/c.txt")"#
        ),
        expect: "a/b",
    },
    Case {
        name: "path_dir_of_a_bare_name_is_empty",
        src: concat!(
            r#"IMPORT("wlwl:std.fs", ["PATH_JOIN", "PATH_DIR", "PATH_BASE", "PATH_EXT", "PATH_NORMALIZE"]); "#,
            r#"PATH_DIR("a.txt")"#
        ),
        // 选 `""` 而不是 Go 的 `"."`:本语言里
        // `PATH_JOIN(PATH_DIR(x), PATH_BASE(x))` 是取回 `x` 的惯用写法。
        expect: "",
    },
    Case {
        name: "path_dir_of_an_absolute_path",
        src: concat!(
            r#"IMPORT("wlwl:std.fs", ["PATH_JOIN", "PATH_DIR", "PATH_BASE", "PATH_EXT", "PATH_NORMALIZE"]); "#,
            r#"PATH_DIR("/a/b")"#
        ),
        expect: "/a",
    },
    Case {
        name: "path_base_does_not_strip_the_extension",
        src: concat!(
            r#"IMPORT("wlwl:std.fs", ["PATH_JOIN", "PATH_DIR", "PATH_BASE", "PATH_EXT", "PATH_NORMALIZE"]); "#,
            r#"PATH_BASE("a/b/c.txt")"#
        ),
        expect: "c.txt",
    },
    Case {
        name: "path_base_of_a_trailing_slash",
        src: concat!(
            r#"IMPORT("wlwl:std.fs", ["PATH_JOIN", "PATH_DIR", "PATH_BASE", "PATH_EXT", "PATH_NORMALIZE"]); "#,
            r#"PATH_BASE("a/b/")"#
        ),
        expect: "b",
    },
    Case {
        name: "path_ext_excludes_the_dot",
        src: concat!(
            r#"IMPORT("wlwl:std.fs", ["PATH_JOIN", "PATH_DIR", "PATH_BASE", "PATH_EXT", "PATH_NORMALIZE"]); "#,
            r#"PATH_EXT("a/b/c.txt")"#
        ),
        // 裁决:`PATH_EXT` **不含前导点**(与 `PATH_BASE` 不剥扩展名对称)。
        expect: "txt",
    },
    Case {
        name: "path_ext_takes_the_last_dot",
        src: concat!(
            r#"IMPORT("wlwl:std.fs", ["PATH_JOIN", "PATH_DIR", "PATH_BASE", "PATH_EXT", "PATH_NORMALIZE"]); "#,
            r#"PATH_EXT("a/b/c.tar.gz")"#
        ),
        expect: "gz",
    },
    Case {
        name: "path_ext_of_a_dotfile_is_empty",
        src: concat!(
            r#"IMPORT("wlwl:std.fs", ["PATH_JOIN", "PATH_DIR", "PATH_BASE", "PATH_EXT", "PATH_NORMALIZE"]); "#,
            r#"PATH_EXT(".gitignore")"#
        ),
        // 点必须在文件名里**不是第一个字符**才算扩展名的起点,否则
        // `PATH_BASE` + `PATH_EXT` 拼不回原名。
        expect: "",
    },
    Case {
        name: "path_ext_of_no_extension_is_empty",
        src: concat!(
            r#"IMPORT("wlwl:std.fs", ["PATH_JOIN", "PATH_DIR", "PATH_BASE", "PATH_EXT", "PATH_NORMALIZE"]); "#,
            r#"PATH_EXT("a/b/README")"#
        ),
        // 「没有」是 `""`,不是 `NULL` —— 与 `ENV_GET` 缺省口径一致。
        expect: "",
    },
    Case {
        name: "path_normalize_resolves_dot_and_dotdot",
        src: concat!(
            r#"IMPORT("wlwl:std.fs", ["PATH_JOIN", "PATH_DIR", "PATH_BASE", "PATH_EXT", "PATH_NORMALIZE"]); "#,
            r#"PATH_NORMALIZE("a/./b/../c")"#
        ),
        expect: "a/c",
    },
    Case {
        name: "path_normalize_on_an_absolute_path_never_escapes_root",
        src: concat!(
            r#"IMPORT("wlwl:std.fs", ["PATH_JOIN", "PATH_DIR", "PATH_BASE", "PATH_EXT", "PATH_NORMALIZE"]); "#,
            r#"PATH_NORMALIZE("/a/../../b")"#
        ),
        expect: "/b",
    },
    Case {
        name: "path_normalize_keeps_leading_dotdot_on_a_relative_path",
        src: concat!(
            r#"IMPORT("wlwl:std.fs", ["PATH_JOIN", "PATH_DIR", "PATH_BASE", "PATH_EXT", "PATH_NORMALIZE"]); "#,
            r#"PATH_NORMALIZE("../a")"#
        ),
        // 绝对路径消 `..` 是安全的,相对路径**消不掉** —— 消了就改了语义。
        expect: "../a",
    },
    Case {
        name: "path_normalize_keeps_the_drive_prefix",
        src: concat!(
            r#"IMPORT("wlwl:std.fs", ["PATH_JOIN", "PATH_DIR", "PATH_BASE", "PATH_EXT", "PATH_NORMALIZE"]); "#,
            r#"PATH_NORMALIZE("C:/a/./b")"#
        ),
        // 盘符是路径的**一部分**,不能当普通片段消掉。跨平台逐字冻结。
        expect: "C:/a/b",
    },
    Case {
        name: "path_normalize_drops_a_trailing_slash",
        src: concat!(
            r#"IMPORT("wlwl:std.fs", ["PATH_JOIN", "PATH_DIR", "PATH_BASE", "PATH_EXT", "PATH_NORMALIZE"]); "#,
            r#"PATH_NORMALIZE("a/b/")"#
        ),
        expect: "a/b",
    },
    Case {
        name: "path_functions_reject_non_strings",
        src: concat!(
            r#"IMPORT("wlwl:std.fs", ["PATH_JOIN", "PATH_DIR", "PATH_BASE", "PATH_EXT", "PATH_NORMALIZE"]); "#,
            r#"PATH_DIR(42)"#
        ),
        expect: "!E0030 PATH_DIR: expected a string, got integer",
    },
    Case {
        name: "path_functions_check_arity",
        src: concat!(
            r#"IMPORT("wlwl:std.fs", ["PATH_JOIN", "PATH_DIR", "PATH_BASE", "PATH_EXT", "PATH_NORMALIZE"]); "#,
            r#"PATH_BASE("a", "b")"#
        ),
        expect: "!E0022 PATH_BASE: function expects 1 argument(s), got 2",
    },
    // ── std.env ─────────────────────────────────────────────────────
    Case {
        name: "env_get_missing_is_null",
        src: concat!(
            r#"IMPORT("wlwl:std.env", ["ENV_GET", "ENV_SET", "ENV_KEYS", "ARGS"]); "#,
            r#"ENV_GET("WLWL_DEFINITELY_UNSET_zzz_42")"#
        ),
        // 「没设」与「设为空串」是两件事,分不开就没法读 `.env` 覆盖后的真实状态。
        expect: "NULL",
    },
    Case {
        name: "env_get_missing_with_default",
        src: concat!(
            r#"IMPORT("wlwl:std.env", ["ENV_GET", "ENV_SET", "ENV_KEYS", "ARGS"]); "#,
            r#"ENV_GET("WLWL_DEFINITELY_UNSET_zzz_42", "fallback")"#
        ),
        expect: "fallback",
    },
    Case {
        name: "env_set_then_get_round_trips",
        src: concat!(
            r#"IMPORT("wlwl:std.env", ["ENV_GET", "ENV_SET", "ENV_KEYS", "ARGS"]); "#,
            r#"ENV_SET("WLWL_PROBE_KEY_42", "v"); ENV_GET("WLWL_PROBE_KEY_42")"#
        ),
        expect: "v",
    },
    Case {
        name: "env_set_is_process_local_and_does_not_leak",
        src: concat!(
            r#"IMPORT("wlwl:std.env", ["ENV_GET", "ENV_SET", "ENV_KEYS", "ARGS"]); "#,
            r#"ENV_SET("WLWL_PROBE_KEY_42", "v"); ENV_GET("WLWL_SENTINEL_UNSET_42")"#
        ),
        expect: "NULL",
    },
    Case {
        name: "env_keys_is_deterministic_across_calls",
        src: concat!(
            r#"IMPORT("wlwl:std.env", ["ENV_GET", "ENV_SET", "ENV_KEYS", "ARGS"]); "#,
            r#"&&(=(ENV_KEYS(), ENV_KEYS()), =(LEN(ENV_KEYS()), LEN(ENV_KEYS())))"#
        ),
        // 排序是**规范性要求**:`std::env::vars()` 的迭代序逐次可能不同,
        // 而可复现是本语言卖点第一条。两次调用必须**逐字节相同**。
        expect: "TRUE",
    },
    Case {
        name: "env_keys_contains_what_we_just_set",
        src: concat!(
            r#"IMPORT("wlwl:std.env", ["ENV_GET", "ENV_SET", "ENV_KEYS", "ARGS"]); "#,
            // `HAS` 只收 DICT(规范 §10.4),而 `ENV_KEYS()` 是数组 ⇒ 用全局内建
            // `INDEX`(1-based,未命中返 -1)。踩过一次:`HAS(数组)` → E0030。
            r#"ENV_SET("WLWL_PROBE_KEY_42", "v"); >(INDEX(ENV_KEYS(), "WLWL_PROBE_KEY_42"), 0)"#
        ),
        expect: "TRUE",
    },
    Case {
        name: "env_members_check_arity",
        src: concat!(
            r#"IMPORT("wlwl:std.env", ["ENV_GET", "ENV_SET", "ENV_KEYS", "ARGS"]); "#,
            r#"ENV_SET("A")"#
        ),
        expect: "!E0022 ENV_SET: function expects 2 argument(s), got 1",
    },
    Case {
        name: "args_takes_no_arguments",
        src: concat!(
            r#"IMPORT("wlwl:std.env", ["ENV_GET", "ENV_SET", "ENV_KEYS", "ARGS"]); "#,
            r#"ARGS(1)"#
        ),
        expect: "!E0022 ARGS: function expects 0 argument(s), got 1",
    },
];

fn scratch_dir() -> PathBuf {
    common::scratch("wlwl_fs_contract")
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
fn fs_and_env_contract() {
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
        "wlwl:std.fs / std.env contract drift:\n    {}",
        bad.join("\n    ")
    );
}

// ── 触及真实文件系统的用例:不进上表,各自自建自清 ───────────────────
//
// 理由:它们需要**目录状态**,而纯函数表驱动不了;而把它们塞进上表就会让
// 「同一份契约表同时冻结纯函数值和 IO 副作用」,失败时看不出是哪一类坏了。

fn io_scratch() -> PathBuf {
    common::scratch("wlwl_fs_io")
}

fn run_with_dir(src: &str, dir: &std::path::Path) -> String {
    let ast = wlwl_parser::parse(src, "t.wll").expect("case must parse");
    let mut ev = Evaluator::new().with_base_dir(dir.to_path_buf());
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

/// IO 用例共用的 IMPORT 头。
const IO: &str = concat!(
    r#"IMPORT("wlwl:std.fs", ["MKDIR", "MKDIRS", "LIST_DIR", "WALK_DIR", "#,
    r#""REMOVE", "COPY", "MOVE", "FILE_SIZE", "READ_BYTES", "WRITE_BYTES", "READ_FILE"]); "#,
);

/// ⚠️ **IO 用例必须用绝对路径。**
///
/// 实测:fs 成员的**相对路径按进程 CWD 解析** —— `with_base_dir` 只影响 `IMPORT`
/// 的模块解析,不影响 `LIST_DIR(".")` 之类的文件操作。⇒ 一版用相对路径的测试把
/// 文件写进了**仓库里**(产出 `impl/crates/wlwl-eval/a.bin`)。这条语义本身是对的
/// (CLI 的常规预期),但**契约必须用绝对路径**,否则测试会污染工作树。
fn abs(dir: &std::path::Path, rel: &str) -> String {
    dir.join(rel).to_string_lossy().replace('\\', "/")
}

#[test]
fn mkdir_is_not_idempotent_but_mkdirs_is() {
    let dir = io_scratch();
    let first = run_with_dir(&format!("{IO}MKDIR(\"{}\")", abs(&dir, "d")), &dir);
    assert_eq!(first, "NULL", "MKDIR on a fresh path must succeed");
    let second = run_with_dir(&format!("{IO}MKDIR(\"{}\")", abs(&dir, "d")), &dir);
    // 静默吞掉「已存在」会掩盖竞态(另一个进程刚建了它)⇒ 必须响。
    assert!(
        second.contains("FsError"),
        "MKDIR on an existing path must be an ERR value, got {second}"
    );
    // `mkdir -p` 语义:幂等成功。
    let nested = abs(&dir, "x/y/z");
    assert_eq!(
        run_with_dir(&format!("{IO}MKDIRS(\"{nested}\")"), &dir),
        "NULL"
    );
    assert_eq!(
        run_with_dir(&format!("{IO}MKDIRS(\"{nested}\")"), &dir),
        "NULL",
        "MKDIRS must be idempotent"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// ⚠️ `PRINT` 观察不到 `ERR` 值(既有缺陷:整条语句被静默吞掉),所以这里用
/// `IS_ERR` 与 `ERR_PAYLOAD`。
#[test]
fn a_failing_fs_call_is_an_err_value_with_kind_op_reason() {
    let dir = io_scratch();
    let hdr = r#"IMPORT("wlwl:std.fs", ["FILE_SIZE"]); "#;
    let out = run_with_dir(&format!(r#"{hdr}IS_ERR(FILE_SIZE("nope_zzz_42"))"#), &dir);
    assert_eq!(
        out, "TRUE",
        "a missing path must be an ERR value, not a diagnostic"
    );
    let payload = run_with_dir(
        &format!(r#"{hdr}ERR_PAYLOAD(FILE_SIZE("nope_zzz_42"))"#),
        &dir,
    );
    assert!(payload.contains("kind: FsError"), "payload = {payload}");
    assert!(payload.contains("op: FILE_SIZE"), "payload = {payload}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn list_dir_is_sorted() {
    let dir = io_scratch();
    for n in ["zeta", "alpha", "mid"] {
        std::fs::write(dir.join(n), b"x").unwrap();
    }
    let out = run_with_dir(&format!("{IO}LIST_DIR(\"{}\")", abs(&dir, "")), &dir);
    let names: Vec<&str> = out
        .split(',')
        .map(|s| s.trim().trim_end_matches(']'))
        .filter_map(|s| s.rsplit('/').next())
        .collect();
    assert_eq!(
        names,
        vec!["alpha", "mid", "zeta"],
        "LIST_DIR must be sorted, got {out}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn walk_dir_recurses_and_glob_star_does_not_cross_separators() {
    let dir = io_scratch();
    std::fs::create_dir_all(dir.join("sub/deep")).unwrap();
    std::fs::write(dir.join("top.md"), b"x").unwrap();
    std::fs::write(dir.join("sub/mid.md"), b"x").unwrap();
    std::fs::write(dir.join("sub/deep/low.md"), b"x").unwrap();
    std::fs::write(dir.join("sub/other.txt"), b"x").unwrap();
    let root = abs(&dir, "");

    let all = run_with_dir(&format!("{IO}WALK_DIR(\"{root}\")"), &dir);
    assert!(all.contains("top.md"), "{all}");
    assert!(
        all.contains("sub/deep/low.md"),
        "WALK_DIR must recurse: {all}"
    );

    // pattern **不含** `/` ⇒ 只匹配文件名 ⇒ `*.md` 命中各层任何深度。
    let md = run_with_dir(&format!("{IO}WALK_DIR(\"{root}\", \"*.md\")"), &dir);
    assert!(
        md.contains("sub/deep/low.md"),
        "filename glob must match at depth: {md}"
    );
    assert!(!md.contains("other.txt"), "{md}");

    // pattern **含** `/` ⇒ 匹配整条相对路径 ⇒ `sub/*.md` **不跨分隔符**。
    let scoped = run_with_dir(&format!("{IO}WALK_DIR(\"{root}\", \"sub/*.md\")"), &dir);
    assert!(scoped.contains("sub/mid.md"), "{scoped}");
    assert!(
        !scoped.contains("sub/deep/low.md"),
        "`*` must not cross the separator: {scoped}"
    );
    // `**` 跨目录
    let deep = run_with_dir(
        &format!("{IO}WALK_DIR(\"{root}\", \"sub/**/low.md\")"),
        &dir,
    );
    assert!(
        deep.contains("sub/deep/low.md"),
        "`**` must cross directories: {deep}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// ⚠🔴 **[N-010 / D13-060] `follow` 在 POSIX 上不是死参数。**
///
/// `WALK_DIR(root, "*.txt", TRUE)` 必须**下降进「符号链接 → 目录」**;
/// `follow` 缺省 / `FALSE` 时**不得**下降。
///
/// ⚠ **期望值出处**(不抄本实现输出):
/// - 规范 §2 `std.fs` 散文条款:**「`WALK_DIR` **默认不下降进链接目录**
///   (`follow = FALSE`),因为**成环的链接目录会让递归永不终止**。置
///   `follow = TRUE` 时记 **visited 集合**(按 `canonicalize`)防环。」**
///   ⇒ 「`FALSE` 不下降」是**成文**的;而「`TRUE` 会下降」在规范里是**隐含**的
///   (为什么要 visited 防环,只可能是会下降)—— ⚠ 这条已提请父代理在规范里
///   补成显式条款,补之前本条断言的依据就是上面这段原文。
/// - 成员行写明返回值是**相对 `p` 的路径、用 `/`** ⇒ 断言只写 `link/inside.txt`
///   这样的规范形状,不碰平台分隔符。
///
/// ⚠ **为什么是 `#[cfg(unix)]`**:`DirEntry::file_type()` **不跟随**符号链接,
/// 所以「链接 → 目录」在 **POSIX** 上 `is_dir()` 恒 `false` ⇒ 缺陷形态下
/// `follow = TRUE` **完全不下降**。而 **Windows** 上目录符号链接带
/// `FILE_ATTRIBUTE_DIRECTORY` 位 ⇒ `file_type().is_dir()` 是 true ⇒
/// **`follow` 在 Windows 上一直是好的** ⇒ 这条契约**在 Windows 上恒绿,
/// 即便实现退回缺陷形态也不红**。
/// ⚠⚠ 本机(Windows)**也建不了符号链接**(实测:`New-Item -ItemType
/// SymbolicLink` 报「此操作需要管理员权限」,未开开发者模式)⇒ 这条**在任何
/// 平台都跑不到本机**,只能交给 CI 的 `ubuntu-latest` / `macos-latest`。
#[cfg(unix)]
#[test]
fn walk_dir_follow_descends_into_a_symlinked_directory_only_when_follow_is_true() {
    use std::os::unix::fs::symlink;
    let dir = io_scratch();
    std::fs::create_dir_all(dir.join("real")).unwrap();
    std::fs::write(dir.join("real/inside.txt"), b"x").unwrap();
    symlink(dir.join("real"), dir.join("link")).unwrap();
    let root = abs(&dir, "");

    // `follow` 缺省 = FALSE ⇒ 列出 `link` 这一项,但**不下降**进它。
    let nofollow = run_with_dir(&format!("{IO}WALK_DIR(\"{root}\", \"*.txt\")"), &dir);
    assert!(
        nofollow.contains("real/inside.txt"),
        "sanity: a real directory must be recursed — {nofollow}"
    );
    assert!(
        !nofollow.contains("link/inside.txt"),
        "follow defaults to FALSE: a symlinked directory must NOT be descended — {nofollow}"
    );

    // ⚠ 显式 `FALSE` 与缺省**必须同口径**(这两条是同一个承诺的两种写法)。
    let explicit = run_with_dir(&format!("{IO}WALK_DIR(\"{root}\", \"*.txt\", FALSE)"), &dir);
    assert!(
        !explicit.contains("link/inside.txt"),
        "follow = FALSE must behave like the default — {explicit}"
    );

    // ⚠🔴 这条在缺陷形态下**恒红**:POSIX 上 `follow = TRUE` 根本到不了
    //   「链接目录也下降」那条分支。
    let followed = run_with_dir(&format!("{IO}WALK_DIR(\"{root}\", \"*.txt\", TRUE)"), &dir);
    assert!(
        followed.contains("link/inside.txt"),
        "follow = TRUE must descend into a symlinked directory — {followed}"
    );
    // ⚠⚠ **`real` 与 `link` 两条都必须出现**(`find -L` 同款:跟随链接不等于
    //   只走一条路)。这一条是 **D13-065** 加的 —— 此前只断言 `link` 出现,
    //   于是**「`real` 被吃掉」这个方向照不出来**。
    // ⚠ 而旧实现(`visited` = 全局已见目标集合)恰恰是**反过来的**那个方向坏:
    //   `read_dir` 顺序未定义,`real` 先被遍历到时 `link` 就被当成「已访问」跳过
    //   —— CI 上 ubuntu 碰巧先返回 `link` 所以绿、macOS 先返回 `real` 所以红
    //   (run 236/237/238)。⇒ **单平台绿灯证明不了顺序相关的性质**。
    assert!(
        followed.contains("real/inside.txt"),
        "follow = TRUE must ALSO keep the real directory's path — {followed}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// ⚠ **N-010 修好之后, `visited` 防环守卫在 POSIX 上才第一次真的被执行到。**
///
/// 修之前 `follow` 是死参数 ⇒ 链接目录从不被下降 ⇒ 成环**不可能发生**
/// ⇒ 防环守卫在 POSIX 上是**从未被执行过的死代码**。修好之后「自环 / 指回祖先」
/// 就成了真会发生的输入,守卫必须真的挡住。
///
/// ⚠⚠ 断言写成「**必须终止**」而不是「必须返回某个精确数组」: 防环的具体口径
/// (哪些层被跳过)不是对外承诺;「不挂死 + 不重复」才是。这条断言形状与
/// `copy_tree_refuses_a_cycle_instead_of_recursing_forever` 同源。
#[cfg(unix)]
#[test]
fn walk_dir_follow_terminates_on_a_self_referential_symlink() {
    use std::os::unix::fs::symlink;
    let dir = io_scratch();
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    std::fs::write(dir.join("sub/leaf.txt"), b"x").unwrap();
    // `loop` 指回**根自己** ⇒ 跟到底就是无限下降。
    symlink(&dir, dir.join("sub/loop")).unwrap();
    let root = abs(&dir, "");

    let out = run_with_dir(&format!("{IO}WALK_DIR(\"{root}\", \"*.txt\", TRUE)"), &dir);
    assert!(
        out.contains("sub/leaf.txt"),
        "sanity: the real subtree must still be walked — {out}"
    );
    // ⚠ 环上的层被 `visited` 挡住 ⇒ `leaf.txt` **只出现一次**。
    assert_eq!(
        out.matches("sub/leaf.txt").count(),
        1,
        "the cycle guard must stop the walk before re-walking a directory — {out}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn remove_refuses_a_non_empty_directory_without_recursive() {
    let dir = io_scratch();
    std::fs::create_dir_all(dir.join("d")).unwrap();
    std::fs::write(dir.join("d/f"), b"x").unwrap();
    let target = abs(&dir, "d");
    let refused = run_with_dir(&format!("{IO}REMOVE(\"{target}\")"), &dir);
    assert!(
        refused.contains("ERR"),
        "a non-empty dir must not be silently removed: {refused}"
    );
    assert!(dir.join("d/f").exists(), "the file must still be there");
    assert_eq!(
        run_with_dir(&format!("{IO}REMOVE(\"{target}\", TRUE)"), &dir),
        "NULL"
    );
    assert!(!dir.join("d").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn copy_and_move_are_for_files() {
    let dir = io_scratch();
    std::fs::write(dir.join("a.bin"), b"hello").unwrap();
    assert_eq!(
        run_with_dir(
            &format!(
                "{IO}COPY(\"{}\", \"{}\")",
                abs(&dir, "a.bin"),
                abs(&dir, "b.bin")
            ),
            &dir
        ),
        "NULL"
    );
    assert_eq!(std::fs::read(dir.join("b.bin")).unwrap(), b"hello");
    assert_eq!(
        run_with_dir(
            &format!(
                "{IO}MOVE(\"{}\", \"{}\")",
                abs(&dir, "b.bin"),
                abs(&dir, "c.bin")
            ),
            &dir
        ),
        "NULL"
    );
    assert!(!dir.join("b.bin").exists());
    assert_eq!(std::fs::read(dir.join("c.bin")).unwrap(), b"hello");
    // 目录 COPY 明确不做(不静默做一半)。
    std::fs::create_dir_all(dir.join("d")).unwrap();
    let refused = run_with_dir(
        &format!("{IO}COPY(\"{}\", \"{}\")", abs(&dir, "d"), abs(&dir, "e")),
        &dir,
    );
    assert!(refused.contains("ERR"), "{refused}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn file_size_counts_bytes() {
    let dir = io_scratch();
    std::fs::write(dir.join("a.bin"), b"0123456789").unwrap();
    assert_eq!(
        run_with_dir(&format!("{IO}FILE_SIZE(\"{}\")", abs(&dir, "a.bin")), &dir),
        "10"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// **本批的核心用例**:非 UTF-8 字节经 `WRITE_BYTES` → `READ_BYTES` **逐字节相同**。
///
/// 形状裁决成 `ARRAY[INTEGER]`(与 `std.encode::RANDOM_BYTES` 同款)—— `STRING`
/// 装不了任意字节,硬塞只能有损替换成 `U+FFFD`(规范 §11.3-3「不做有损替换」)。
#[test]
fn binary_bytes_round_trip_exactly() {
    let dir = io_scratch();
    // 含 NUL、0x80(非法 UTF-8 续字节)、0xFF(永不是合法 UTF-8)
    let payload = "[0, 128, 255, 65, 0]";
    let target = abs(&dir, "a.bin");
    let w = run_with_dir(&format!("{IO}WRITE_BYTES(\"{target}\", {payload})"), &dir);
    assert_eq!(w, "NULL", "WRITE_BYTES failed: {w}");
    let r = run_with_dir(&format!("{IO}READ_BYTES(\"{target}\")"), &dir);
    assert_eq!(r, payload, "bytes must round-trip exactly");
    // 同一份字节**不能**用文本成员读回来(那正是有损替换发生的地方)。
    let bad = abs(&dir, "bad.bin");
    std::fs::write(dir.join("bad.bin"), [0xFFu8, 0xFE]).unwrap();
    let via_text = run_with_dir(&format!("{IO}READ_FILE(\"{bad}\")"), &dir);
    assert!(
        via_text.starts_with("!"),
        "READ_FILE on non-UTF-8 must be a diagnostic, not silent U+FFFD: {via_text}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn write_bytes_rejects_out_of_range_bytes() {
    let dir = io_scratch();
    let target = abs(&dir, "a.bin");
    let out = run_with_dir(&format!("{IO}WRITE_BYTES(\"{target}\", [0, 256])"), &dir);
    // 静默截断会让「同一个文件」有两种解释,而字节 IO 最不能容忍二义性。
    assert!(out.contains("ERR"), "byte 256 must be rejected, got {out}");
    assert!(out.contains("0..=255"), "{out}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// ⚠🔴 **[D13-045 / N-031] 根目录打不开必须与 `LIST_DIR` 同口径。**
///
/// 顶层此前忽略 `walk` 的全部产出 ⇒ **根不存在**与**根是个空目录**返回
/// 完全相同的 `[]`,调用方无从分辨。这条把两件事**同时**钉住:缺失的根要响,
/// 而**存在但为空**的根仍然是空数组(别把守卫做成「一律响」)。
#[test]
fn walk_dir_on_a_missing_root_is_an_err_value_but_an_empty_root_is_not() {
    let dir = io_scratch();
    let missing = abs(&dir, "no_such_dir_zzz");
    assert_eq!(
        run_with_dir(&format!("{IO}IS_ERR(WALK_DIR(\"{missing}\"))"), &dir),
        "TRUE",
        "a missing root must be an ERR value, not an empty list"
    );
    // 口径对齐:同一条路径在 `LIST_DIR` 上本来就响(两个成员不该有两种答案)。
    assert_eq!(
        run_with_dir(&format!("{IO}IS_ERR(LIST_DIR(\"{missing}\"))"), &dir),
        "TRUE",
        "WALK_DIR and LIST_DIR must agree on a missing directory"
    );
    std::fs::create_dir_all(dir.join("empty")).unwrap();
    let empty = abs(&dir, "empty");
    assert_eq!(
        run_with_dir(&format!("{IO}WALK_DIR(\"{empty}\")"), &dir),
        "[]",
        "an existing but empty root must stay an empty array"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// ⚠🔴 **[D13-047 / N-038] `READ_BYTES` 的大小护栏。**
///
/// 文件在 **Rust 侧**写:用 `WRITE_BYTES` 造一个 1 MiB+ 的 `ARRAY[INTEGER]`
/// 需要在解释器里物化上百万个值 —— 那正是这条护栏要防的形状,不能拿被测对象
/// 造测试数据。
///
/// 判据是**语义**:超上限返 `ERR` **值**(不是诊断、不是截断),而且**文件本身
/// 毫发无损**(`FILE_SIZE` 仍报真实大小)—— 拒绝读取不该有任何副作用。
#[test]
fn read_bytes_refuses_a_file_over_the_cap_without_touching_it() {
    let dir = io_scratch();
    let big = abs(&dir, "big.bin");
    std::fs::write(dir.join("big.bin"), vec![65u8; 2 * 1024 * 1024]).unwrap();
    assert_eq!(
        run_with_dir(&format!("{IO}IS_ERR(READ_BYTES(\"{big}\"))"), &dir),
        "TRUE",
        "a file over the READ_BYTES cap must be an ERR value"
    );
    let payload = run_with_dir(&format!("{IO}ERR_PAYLOAD(READ_BYTES(\"{big}\"))"), &dir);
    assert!(payload.contains("kind: FsError"), "payload = {payload}");
    assert!(payload.contains("op: READ_BYTES"), "payload = {payload}");
    assert_eq!(
        run_with_dir(&format!("{IO}FILE_SIZE(\"{big}\")"), &dir),
        (2 * 1024 * 1024).to_string(),
        "a refused read must not truncate, move or delete the file"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// [D13-051] **`MOVE` 的链接语义随「源与目标是否同一设备」而变**这件事,
/// **必须写在规范里** —— 它是**实测**结论(同设备 `rename` 保留链接 /
/// 跨设备回退按内容展开),而此前**任何一处规范文本都没写**它 ⇒ 读规范的人
/// 会以为「链接总是被展开」。⚠ 写下来之后,「换个设备行为就变」就从
/// 「环境事实」变成「**成文的契约**」—— 契约可以复核,环境事实只能撞见。
///
/// ⚠ 范围收在 **§2 内**:散文里出现「以 `| ` 开头的行」会污染成员表提取器。
///
/// ⚠ **判据里不能放本仓的台账号**:规范仓不会带本仓的偏差台账编号 ⇒ 拿
/// `D13-051` 当 needle 等于判「规范必须提我们自己的内部编号」,规范**永远不会**
/// 满足。改钉**语义表述**:同设备**链接原样保留**、跨设备**按内容展开**、
/// 回退路径叫**复制 + 删除** —— 三者合起来才等于「MOVE 的链接语义成文」。
#[test]
fn the_move_link_semantics_are_written_down() {
    let text = std::fs::read_to_string(spec_path()).expect("stdlib spec readable");
    // 范围收在 §2 内 ⇒ 取该章正文(结束于下一个 `## `)。
    let body = common::chapter_body(&text, "## 标准库规范 · 2 `std.fs`");
    for needle in [
        "同设备",
        "跨设备",
        "按内容展开",
        "链接原样保留",
        "复制 + 删除",
    ] {
        assert!(
            body.contains(needle),
            "stdlib spec §2 lost `{needle}` — the MOVE link semantics must be \
             written down, not left as an environment-dependent artefact"
        );
    }
}

/// [D13-052] `WRITE_BYTES` 的上限必须写在规范里 —— 旧版写着「**本成员尚无
/// 上限**,与 `READ_BYTES` 不对称」,而实现**已经有**上限 ⇒ 规范与实现对不上,
/// 下一个人会照旧文案以为没有上限。
#[test]
fn the_write_bytes_cap_is_written_down() {
    let text = std::fs::read_to_string(spec_path()).expect("stdlib spec readable");
    // 范围收在 §2 内 ⇒ 取该章正文(结束于下一个 `## `)。
    let body = common::chapter_body(&text, "## 标准库规范 · 2 `std.fs`");
    for needle in ["MAX_WRITE_BYTES", "64 MiB", "原封不动"] {
        assert!(
            body.contains(needle),
            "stdlib spec §2 lost `{needle}` — the WRITE_BYTES cap must be \
             written down, not left as an implementation artefact"
        );
    }
    assert!(
        !body.contains("本成员尚无上限"),
        "stdlib spec §2 still says WRITE_BYTES has no cap — the implementation \
         does have one, and the spec must not contradict it"
    );
}

/// 规范 §2 的成员表 ↔ `SPEC.functions`。
#[test]
fn fs_member_set_matches_the_spec_table() {
    let text = std::fs::read_to_string(spec_path()).expect("stdlib spec readable");
    // 提取范围收在 §2 内(散文里出现「以 `| ` 开头的行」会污染成员表提取器
    // —— 本轮在 §15 上踩过一次同样的坑)。
    let body = common::chapter_body(&text, "## 标准库规范 · 2 `std.fs`");

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
            && !spec.contains(&name)
        {
            spec.push(name);
        }
    }

    let impls: Vec<String> = wlwl_std::resolve("wlwl:std.fs")
        .expect("wlwl:std.fs resolves")
        .functions()
        .iter()
        .map(|(n, _)| (*n).to_string())
        .collect();

    assert_eq!(
        spec.len(),
        26,
        "§2 table extractor found {} member(s): {spec:?} — the table's shape changed",
        spec.len()
    );
    for n in &impls {
        assert!(spec.contains(n), "§2 table missed `{n}`: {spec:?}");
    }
    assert_eq!(impls.len(), 26, "wlwl:std.fs exports 26 members");
}

/// `std.env` 的成员表同样双向锁。
#[test]
fn env_member_set_matches_the_spec_table() {
    let text = std::fs::read_to_string(spec_path()).expect("stdlib spec readable");
    // 范围收在 §18 内 ⇒ 取该章正文(结束于下一个 `## `)。
    let body = common::chapter_body(&text, "## 标准库规范 · 18 `std.env`");

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
            && !spec.contains(&name)
        {
            spec.push(name);
        }
    }
    let impls: Vec<String> = wlwl_std::resolve("wlwl:std.env")
        .expect("wlwl:std.env resolves")
        .functions()
        .iter()
        .map(|(n, _)| (*n).to_string())
        .collect();
    assert_eq!(spec.len(), 5, "§18 table found {spec:?}");
    for n in &impls {
        assert!(spec.contains(n), "§18 table missed `{n}`: {spec:?}");
    }
    assert_eq!(
        impls.len(),
        5,
        "wlwl:std.env exports 5 members (4 + SCRIPT_PATH)"
    );
}

fn spec_path() -> PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
        .join("docs")
        .join("wlwl-spec-reference.md")
}

/// 代码侧的锁与文档侧的锁必须成对:分隔符那条是**跨平台契约**,它必须
/// 在规范里被写成明文,否则下一个人会以为平台原生是「允许的」。
///
/// ⚠ 判据**分两段、每段收在自己的章里**。落在整个文件上时,别的章里的同名
///   文字会让这条一直绿 —— 修之前实测把 §2 的「规范分隔符恒为 `/`」与 §19 的
///   三处「不经 shell」**全部删掉**,本条**仍然绿**。
#[test]
fn the_separator_decision_is_written_down() {
    let text = std::fs::read_to_string(spec_path()).expect("stdlib spec readable");
    // ① std.fs §2:分隔符恒为 `/`(跨平台契约)+ 那次裁决的出处。
    let fs_body = common::chapter_body(&text, "## 标准库规范 · 2 `std.fs`");
    // ⚠ 三个锚点在 §2 内**各自唯一**(整册也唯一):分隔符的**裁决句**(§2 导语)、
    //   成员表里的**操作口径**(`PATH_JOIN`)、以及那次裁决的**出处**。
    //   只钉「§2 里出现过 `/`」不够 —— §2 的成员表里到处是 `/`,删掉裁决句也照样绿。
    for needle in ["规范分隔符恒为 `/`", "合并重复分隔符", "ADDENDUM-06"] {
        assert!(
            fs_body.contains(needle),
            "stdlib spec §2 lost `{needle}` — the separator decision must be \
             written down, not left as an implementation artefact"
        );
    }
    // ② std.process §19:`argv` 不经 shell 解析(不经 shell 跑子进程)。
    let proc_body = common::chapter_body(&text, "## 标准库规范 · 19 `std.process`");
    // ⚠ 同上:两个锚点在 §19 内各自唯一 —— 章题(声明这件事是本章的主题)与
    //   §19.6 的 `argv` 语义句。整册的「不经 shell」在系统册总纲里也出现过,
    //   所以不能拿「不经 shell」四个字本身当判据。
    for needle in ["不经 shell 跑子进程", "不经 shell 解析"] {
        assert!(
            proc_body.contains(needle),
            "stdlib spec §19 lost `{needle}` — the argv decision (no shell) must be \
             written down, not left as an implementation artefact"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════
// [F-03] 读写两侧的**诊断码分流** —— `WRITE_FILE` 写失败 = `E0062`,
// `READ_FILE` 读不到 = `E0061`
// ═══════════════════════════════════════════════════════════════════════
//
// 规范依据:stdlib §2.0「`WRITE_FILE(path, text) -> NULL` | 写 UTF-8 文本;
// **父目录必须已存在** | **写失败 → `E0062`**(原生诊断)」。
//
// 缺陷形态:实现与 `READ_FILE` **共用**一个 IO 映射,而「父目录不存在」在
// OS 层同样落进 `ErrorKind::NotFound` ⇒ 写失败被报成
// `error[E0061]: WRITE_FILE: file not found (…)` —— 码错(把「写不进」说成
// 「文件不存在」)、消息也误导。⇒ 写侧一律 `E0062`,读侧一字未动。

/// [F-03] 读写两成员的 IMPORT 头。
///
/// ⚠ 上面的 `IO` 那份**不含 `WRITE_FILE`** —— 本组用例自带一份,不去改
/// 既有常量(避免牵动其它用例)。
const RW: &str = r#"IMPORT("wlwl:std.fs", ["READ_FILE", "WRITE_FILE"]); "#;

/// 把 `run_with_dir` 的 `!CODE message` 形状拆成 (码, 消息)。
///
/// ⚠ [G-19] 读写两个成员的 IO 失败**不再是诊断** ⇒ 这条只对**仍是诊断**的
///   失败形状（arity / type / 未改口径的成员）有用。
#[allow(dead_code)]
fn split_diag(out: &str) -> (String, String) {
    let rest = out.strip_prefix('!').unwrap_or(out);
    match rest.split_once(' ') {
        Some((code, msg)) => (code.to_string(), msg.to_string()),
        None => (rest.to_string(), String::new()),
    }
}

/// [G-19] 跑一个「会失败的 IO 调用」并把 **`ERR` 载荷取出来**。
///
/// ⚠ **为什么要 `ERR_PAYLOAD` 消费掉**：失败现在是 **`ERR` 值**；若让它当程序的
///   **最后一个表达式**，按 §8.6 它会**逃逸到顶层**并报 `E0102`
///   （「unhandled ERR escaped to top level」）⇒ 看到的是 E0102 而不是载荷本身。
///   消费掉之后，程序的返回值就是载荷的字段 ⇒ 断言可以直接看字符串。
///
/// 返回形如 `[kind: FsError, reason: ...]` 的 display。
fn io_failure_payload(src: &str, dir: &std::path::Path) -> String {
    let prog = format!(
        r#"{RW}LET(r, {src}); LET(p, ERR_PAYLOAD(r)); ["kind": AT_K(p, "kind", "?"), "reason": AT_K(p, "reason", "?")]"#
    );
    run_with_dir(&prog, dir)
}

/// ⚠🔴 **[G-19] 写失败现在是 `ERR` **值**（不是诊断）**。
///
/// 写向**不存在的父目录**是最常见的一种写失败（写侧规范明写「父目录必须
/// 已存在」），它必须**可分支**（`IS_ERR`）而不是**终止进程**。
///
/// ⚠ **改口径的理由**：诊断会终止运行 ⇒ 在「每连接一个任务」的服务器里，
///   一次写失败就会让**整个服务**消失。
/// ⚠ **改口径的代价（breaking）**：stdlib §2.0 明写「写失败 → `E0062`（原生诊断）」，
///   现在那个码**不再产生** ⇒ 上游采纳时必须改规范。
/// ⚠ **必须用绝对路径**（stdlib §2.3：相对路径按进程 CWD 解析）⇒ 文件不会落进工作树。
#[test]
fn write_file_into_a_missing_parent_directory_is_an_fs_error_value() {
    let dir = io_scratch();
    let target = abs(&dir, "no_such_dir_zzz/x.txt");
    let out = io_failure_payload(&format!("WRITE_FILE(\"{target}\", \"hello\")"), &dir);
    assert!(
        !out.starts_with('!'),
        "a write failure must NOT be a diagnostic (it would kill the process), got {out}"
    );
    assert!(
        out.contains("FsError"),
        "the failure must carry the FsError family, got {out}"
    );
    assert!(
        out.contains("parent directory"),
        "the reason must name the missing parent directory, got {out}"
    );
    assert!(
        !out.contains("file not found"),
        "the read-side wording must not leak into the write side, got {out}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// ⚠ **[G-19] 读不到现在是 `ERR` **值**（不是 `E0061` 诊断）**。
///
/// 这条是 F-03 的**反向守卫**的续集：读写两侧的**原因文本**仍要**分得开**
/// —— 读侧说「file not found」，写侧说「parent directory does not exist」。
#[test]
fn read_file_of_a_missing_file_is_an_fs_error_value() {
    let dir = io_scratch();
    let target = abs(&dir, "no_such_file_zzz.txt");
    let out = io_failure_payload(&format!("READ_FILE(\"{target}\")"), &dir);
    assert!(
        !out.starts_with('!'),
        "a missing file must NOT be a diagnostic (it would kill the process), got {out}"
    );
    assert!(
        out.contains("FsError"),
        "the failure must carry the FsError family, got {out}"
    );
    assert!(
        out.contains("file not found"),
        "the read-side message must stay as it was, got {out}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// ⚠ **[G-19] 只读目标 ⇒ 写失败 ⇒ `ERR` 值**（原因文本要能区分权限不足）。
///
/// ⚠ **以 root 跑时**（部分容器 CI）只读位对 root 无效 ⇒ 先探一次可写性，
/// 探得过就跳过这条，而不是让测试在那种环境里假红。
#[test]
fn write_file_to_a_read_only_target_is_an_fs_error_value() {
    let dir = io_scratch();
    let target = dir.join("ro.txt");
    std::fs::write(&target, b"x").unwrap();
    // 跨平台置只读:Unix 上清掉写位,Windows 上置 `FILE_ATTRIBUTE_READONLY`。
    let mut perms = std::fs::metadata(&target).unwrap().permissions();
    perms.set_readonly(true);
    std::fs::set_permissions(&target, perms).unwrap();
    if std::fs::OpenOptions::new()
        .write(true)
        .open(&target)
        .is_ok()
    {
        // 权限模型对本进程无效(root)⇒ 这条在本环境里测不了。
        let _ = std::fs::remove_dir_all(&dir);
        return;
    }
    let out = io_failure_payload(
        &format!("WRITE_FILE(\"{}\", \"hello\")", abs(&dir, "ro.txt")),
        &dir,
    );
    assert!(
        !out.starts_with('!'),
        "a write failure must NOT be a diagnostic, got {out}"
    );
    assert!(
        out.contains("FsError"),
        "the failure must carry the FsError family, got {out}"
    );
    assert!(
        out.contains("permission denied"),
        "the reason must distinguish a permission failure, got {out}"
    );
    // 只读位会让 Windows 上的删除失败 ⇒ 清掉再清目录。
    //
    // ⚠ **必须按平台分治**:`Permissions::set_readonly(false)` 在 **Unix 上是空操作**
    // ——Unix 上 `set_readonly` 改的是**写位**,那个位本身跟「只读」不是一个东西 ⇒
    // Unix 上要恢复可写只能 `set_mode(0o644)`;Windows 上则只能用 `set_readonly(false)`。
    // clippy 的 `permissions_set_readonly_false` 建议一律改 `set_mode`,那在 Windows 上
    // 编译不过(`std::os::unix` 只在 Unix 存在)⇒ 非 Unix 分支只能局部豁免。
    let mut perms = std::fs::metadata(&target).unwrap().permissions();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        perms.set_mode(0o644);
    }
    #[cfg(not(unix))]
    {
        #[allow(clippy::permissions_set_readonly_false)]
        perms.set_readonly(false);
    }
    let _ = std::fs::set_permissions(&target, perms);
    let _ = std::fs::remove_dir_all(&dir);
}
