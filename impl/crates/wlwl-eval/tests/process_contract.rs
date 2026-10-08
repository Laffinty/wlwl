//! `wlwl:std.process` 成员契约 —— 标准库规范 §19(v0.11.3 M6 / addendum-06)。
//!
//! ## 本册最重的两条都关于**安全**,不是功能
//!
//! 1. **`argv` 只收 `ARRAY[STRING]`,规范显式禁止字符串形态** —— 拼一个字符串
//!    再让系统解析就是**命令注入**(`; rm -rf /`)。成员行的原文就是「这条是本
//!    成员存在的安全理由」,所以契约里**必须**有「元字符不被解释」那条。
//! 2. **超时真的杀掉子进程**(而不是把整个运行时挂住)—— 见
//!    `wlwl-std` 单测 `a_timeout_kills_the_child_instead_of_hanging`,它在本轮
//!    **抓到过一个真缺陷**(300 ms 的超时走了 29.26 s,根因是 Windows 上
//!    `child.kill()` 不杀孙进程,而孙进程握着管道句柄 ⇒ 读线程等不到 EOF)。
//!    契约这边钉的是**结果形态**:`ERR` 值 + `kind = "ProcessError"`。
//!
//! ## ⚠️ 阻塞代价必须在这里也写一遍
//!
//! 运行时是**单线程协作式** ⇒ `PROCESS_RUN` 等子进程期间**不调度任何其他
//! 任务**。与 `std.time::SLEEP` 的阻塞代价**同源**。契约能钉住「返回值是
//! `{code, stdout, stderr}` 这个字典」,但**钉不住也不该假装钉得住**「它阻塞」
//! —— 那是文档条款(`the_blocking_cost_is_documented` 那条测试守它)。

use std::path::PathBuf;
use wlwl_eval::Evaluator;

struct Case {
    name: &'static str,
    src: &'static str,
    expect: &'static str,
}

const CASES: &[Case] = &[
    // ── argv 的形状:只收数组 ────────────────────────────────────────
    Case {
        name: "a_string_argv_is_e0030",
        src: concat!(r#"IMPORT("wlwl:std.process", ["PROCESS_RUN", "PROCESS_ID"]); "#, r#"PROCESS_RUN("cmd /C dir")"#),
        // 拼字符串再解析 = 命令注入。**规范显式禁止字符串形态**,所以这是原生诊断。
        expect: "!E0030 PROCESS_RUN: expected an argv array of strings, got string",
    },
    Case {
        name: "an_empty_argv_is_e0030",
        src: concat!(r#"IMPORT("wlwl:std.process", ["PROCESS_RUN", "PROCESS_ID"]); "#, r#"PROCESS_RUN([])"#),
        expect: "!E0030 PROCESS_RUN: argv must have at least one element (the program)",
    },
    Case {
        name: "a_non_string_argv_element_is_e0030",
        src: concat!(r#"IMPORT("wlwl:std.process", ["PROCESS_RUN", "PROCESS_ID"]); "#, r#"PROCESS_RUN(["cmd", 7])"#),
        expect: "!E0030 PROCESS_RUN: expected an argv array of strings, got integer",
    },
    Case {
        name: "process_run_arity",
        src: concat!(r#"IMPORT("wlwl:std.process", ["PROCESS_RUN", "PROCESS_ID"]); "#, r#"PROCESS_RUN()"#),
        expect: "!E0022 PROCESS_RUN: function expects 1 argument(s), got 0",
    },
    Case {
        name: "process_id_takes_no_arguments",
        src: concat!(r#"IMPORT("wlwl:std.process", ["PROCESS_RUN", "PROCESS_ID"]); "#, r#"PROCESS_ID(1)"#),
        expect: "!E0022 PROCESS_ID: function expects 0 argument(s), got 1",
    },
    // ── opts 的形状 ─────────────────────────────────────────────────
    Case {
        name: "an_unknown_opts_key_is_e0030",
        src: concat!(r#"IMPORT("wlwl:std.process", ["PROCESS_RUN", "PROCESS_ID"]); "#, r#"PROCESS_RUN(["cmd"], ["shell": TRUE])"#),
        // ⚠️ 尤其 `shell` 必须被**显式拒绝**而不是忽略 —— 「忽略」和「禁止」
        // 在安全上是两件不同的事。
        expect: "!E0030 PROCESS_RUN: unknown opts key `shell` (timeout / cwd / env)",
    },
    Case {
        name: "opts_must_be_a_dictionary",
        src: concat!(r#"IMPORT("wlwl:std.process", ["PROCESS_RUN", "PROCESS_ID"]); "#, r#"PROCESS_RUN(["cmd"], 5)"#),
        expect: "!E0030 PROCESS_RUN: expected an opts dictionary, got integer",
    },
    Case {
        name: "a_negative_timeout_is_e0030",
        src: concat!(r#"IMPORT("wlwl:std.process", ["PROCESS_RUN", "PROCESS_ID"]); "#, r#"PROCESS_RUN(["cmd"], ["timeout": -1])"#),
        expect: "!E0030 PROCESS_RUN: expected opts.timeout to be a non-negative integer, got integer",
    },
    // ── 失败形态:进程起不来是 ERR **值** ──────────────────────────────
    Case {
        name: "a_missing_program_is_an_err_value",
        src: concat!(r#"IMPORT("wlwl:std.process", ["PROCESS_RUN", "PROCESS_ID"]); "#, r#"IS_ERR(PROCESS_RUN(["wlwl_no_such_program_zzz_42"]))"#),
        // 「这个程序不存在」是**可预期的运行期结果**,调用方多半要 `IS_ERR`
        // 分支 ⇒ 用 `ERR` 值而不是终止运行的原生诊断。
        expect: "TRUE",
    },
    Case {
        name: "the_failure_payload_carries_kind_op_reason",
        src: concat!(
            r#"IMPORT("wlwl:std.process", ["PROCESS_RUN", "PROCESS_ID"]); "#,
            r#"ERR_PAYLOAD(PROCESS_RUN(["wlwl_no_such_program_zzz_42"]))"#
        ),
        expect: "[kind: ProcessError, op: PROCESS_RUN, reason: spawn wlwl_no_such_program_zzz_42: program not found]",
    },
];

fn scratch_dir() -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("wlwl_proc_contract_{nanos}"));
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
fn process_contract() {
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
        "wlwl:std.process contract drift:\n    {}",
        bad.join("\n    ")
    );
}

/// **安全守卫**:元字符必须作为**单个参数**原样到达,不被任何 shell 解释。
///
/// ⚠️ 本语言**没有中缀运算符**,所以字符串拼接是 `+(a, b)`。
/// 若实现哪天改成「拼一个字符串给 shell」,这条会红。
#[test]
fn argv_elements_are_never_shell_interpreted() {
    let src = if cfg!(windows) {
        r#"IMPORT("wlwl:std.process", ["PROCESS_RUN"]);
AT_K(PROCESS_RUN(["cmd", "/C", "echo", "a & echo INJECTED"]), "stdout", "")"#
    } else {
        r#"IMPORT("wlwl:std.process", ["PROCESS_RUN"]);
AT_K(PROCESS_RUN(["sh", "-c", "printf %s "$1"", "sh", "a & echo INJECTED"]), "stdout")"#
    };
    let out = actual(src);
    assert!(
        out.contains("a & echo INJECTED"),
        "the argv element was mangled or interpreted: {out}"
    );
    assert!(
        !out.contains("INJECTED\r\n") && !out.contains("\nINJECTED"),
        "`&` must not have run a second command: {out}"
    );
}

/// 真的跑一个命令,钉住返回字典的三个键。
#[test]
fn a_successful_run_returns_code_stdout_stderr() {
    let src = if cfg!(windows) {
        r#"IMPORT("wlwl:std.process", ["PROCESS_RUN"]);
LET(r, PROCESS_RUN(["cmd", "/C", "echo hello"]));
[AT_K(r, "code", 0), AT_K(r, "stdout", 0)] "#
    } else {
        r#"IMPORT("wlwl:std.process", ["PROCESS_RUN"]);
LET(r, PROCESS_RUN(["sh", "-c", "echo hello"]));
[AT_K(r, "code", 0), AT_K(r, "stdout", 0)] "#
    };
    let out = actual(src);
    // 跨平台行尾:Windows 的 `echo` 产出 `\r\n`,POSIX 是 `\n` ⇒ 断言前归一。
    let got = out.replace('\r', "");
    assert_eq!(got, "[0, hello\n]", "got {out}");
}

/// 非零退出码是**正常结果**,不是失败。
#[test]
fn a_nonzero_exit_code_is_a_normal_result() {
    let src = if cfg!(windows) {
        r#"IMPORT("wlwl:std.process", ["PROCESS_RUN"]);
AT_K(PROCESS_RUN(["cmd", "/C", "exit 3"]), "code", 0)"#
    } else {
        r#"IMPORT("wlwl:std.process", ["PROCESS_RUN"]);
AT_K(PROCESS_RUN(["sh", "-c", "exit 3"]), "code", 0)"#
    };
    let out = actual(src);
    assert_eq!(out, "3", "a nonzero exit is data, not an error — got {out}");
}

/// 超时杀掉子进程并返回 `ERR`(而不是把运行时挂住)。
///
/// ⏱ 这条会**真的等约 300 ms**,并且它同时钉「杀掉整棵进程树」——
/// 本轮在 `wlwl-std` 单测里抓到过 Windows 上只杀直接子进程、导致
/// 读线程等 29 s 的缺陷。
#[test]
fn a_timeout_yields_an_err_value() {
    let argv = if cfg!(windows) {
        r#"["cmd", "/C", "ping -n 6 127.0.0.1 > NUL"]"#
    } else {
        r#"["sleep", "5"]"#
    };
    let src = format!(
        r#"IMPORT("wlwl:std.process", ["PROCESS_RUN"]);
IS_ERR(PROCESS_RUN({argv}, ["timeout": 300]))"#
    );
    let started = std::time::Instant::now();
    let out = actual(&src);
    let elapsed = started.elapsed();
    assert_eq!(out, "TRUE", "a timeout must be an ERR value — got {out}");
    assert!(
        elapsed < std::time::Duration::from_secs(3),
        "a 300 ms timeout took {elapsed:?} — the child tree was not killed"
    );
}

/// `PROCESS_ID` 真的返回本进程 id(> 0)。
#[test]
fn process_id_is_positive() {
    let out = actual(concat!(
        r#"IMPORT("wlwl:std.process", ["PROCESS_RUN", "PROCESS_ID"]); "#,
        r#">(PROCESS_ID(), 0)"#
    ));
    assert_eq!(
        out, "TRUE",
        "PROCESS_ID must be the current pid — got {out}"
    );
}

#[test]
fn process_member_set_matches_the_spec_table() {
    let text = std::fs::read_to_string(spec_path()).expect("stdlib spec readable");
    let start = text.find("## 19 `std.process`").expect("§19 header exists");
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
    let impls: Vec<String> = wlwl_std::resolve("wlwl:std.process")
        .expect("wlwl:std.process resolves")
        .functions()
        .iter()
        .map(|(n, _)| (*n).to_string())
        .collect();
    assert_eq!(spec.len(), 2, "§19 table found {spec:?}");
    for n in &impls {
        assert!(spec.contains(n), "§19 table missed `{n}`: {spec:?}");
    }
    assert_eq!(impls.len(), 2, "wlwl:std.process exports 2 members");
}

/// ⚠️ 阻塞代价是**语言级事实**,不是实现细节 ⇒ 必须在规范里成文。
/// 契约钉不住「它阻塞」(那是时间性质),但能钉住「那句话还在」。
#[test]
fn the_blocking_cost_is_documented() {
    let text = std::fs::read_to_string(spec_path()).expect("stdlib spec readable");
    for needle in ["不经 shell", "阻塞", "命令注入"] {
        assert!(
            text.contains(needle),
            "stdlib spec lost `{needle}` — the no-shell / blocking-cost clauses of \
             std.process are language-level facts, not implementation notes"
        );
    }
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
