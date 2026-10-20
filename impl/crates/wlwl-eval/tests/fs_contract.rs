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
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("wlwl_fs_contract_{nanos}"));
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
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("wlwl_fs_io_{nanos}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
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
#[test]
fn the_move_link_semantics_are_written_down() {
    let text = std::fs::read_to_string(spec_path()).expect("stdlib spec readable");
    let start = text.find("## 2 `std.fs`").expect("§2 header exists");
    let end = text[start..]
        .find("\n## ")
        .map(|i| start + i)
        .expect("a following top-level heading exists");
    let body = &text[start..end];
    for needle in ["同设备", "跨设备", "按内容展开", "D13-051"] {
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
    let start = text.find("## 2 `std.fs`").expect("§2 header exists");
    let end = text[start..]
        .find("\n## ")
        .map(|i| start + i)
        .expect("a following top-level heading exists");
    let body = &text[start..end];
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
    let start = text.find("## 2 `std.fs`").expect("§2 header exists");
    let end = text[start..]
        .find("\n## ")
        .map(|i| start + i)
        .expect("a following top-level heading exists");
    // 提取范围收在 §2 内(散文里出现「以 `| ` 开头的行」会污染成员表提取器
    // —— 本轮在 §15 上踩过一次同样的坑)。
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
        18,
        "§2 table extractor found {} member(s): {spec:?} — the table's shape changed",
        spec.len()
    );
    for n in &impls {
        assert!(spec.contains(n), "§2 table missed `{n}`: {spec:?}");
    }
    assert_eq!(impls.len(), 18, "wlwl:std.fs exports 18 members");
}

/// `std.env` 的成员表同样双向锁。
#[test]
fn env_member_set_matches_the_spec_table() {
    let text = std::fs::read_to_string(spec_path()).expect("stdlib spec readable");
    let start = text.find("## 18 `std.env`").expect("§18 header exists");
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
    assert_eq!(spec.len(), 4, "§18 table found {spec:?}");
    for n in &impls {
        assert!(spec.contains(n), "§18 table missed `{n}`: {spec:?}");
    }
    assert_eq!(impls.len(), 4, "wlwl:std.env exports 4 members");
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

/// 代码侧的锁与文档侧的锁必须成对:分隔符那条是**跨平台契约**,它必须
/// 在规范里被写成明文,否则下一个人会以为平台原生是「允许的」。
#[test]
fn the_separator_decision_is_written_down() {
    let text = std::fs::read_to_string(spec_path()).expect("stdlib spec readable");
    for needle in ["`/`", "分隔符", "ADDENDUM-06", "不经 shell"] {
        assert!(
            text.contains(needle),
            "stdlib spec lost `{needle}` — the separator / argv decisions must be \
             written down, not left as an implementation artefact"
        );
    }
}
