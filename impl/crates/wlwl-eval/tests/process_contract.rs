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
//! ## ⚠ 哪条断言只在**某个平台**上咬人
//!
//! 超时类的断言分**两层**,别混:「结果形态」(`ERR` 值)两个平台都咬;但**「耗时
//! 有界」**只有 **POSIX** 咬得到 —— Windows 的 `taskkill /T /F` 把整棵进程树
//! 都杀了,孙进程也跟着死 ⇒ 管道必然 EOF ⇒ 无界的 `join()` 也照样秒回。
//! ⚠ 那不是「Windows 更强」,是**同一条断言在 Windows 上失效**。
//! ⇒ 端到端那条(`a_timeout_with_a_living_grandchild_returns_promptly`)在
//! Windows 上**恒绿**;真正在 Windows 上守得住的那一半是 `wlwl-std` 里的
//! `drain_is_bounded_even_when_the_reader_never_finishes`(直接钉 `drain` 的
//! 有界性,与「杀不杀得掉进程树」无关)。
//!
//! ⚠ 同理,`a_timeout_leaves_no_living_grandchild`(`[N-037 项三]`:超时之后
//! **不得留下活着的孙进程**)**整条 `#[cfg(unix)]`** —— 它靠的也是「整棵树被杀」,
//! Windows 上由 `taskkill /T /F` 白送 ⇒ 在那儿恒绿。它是**唯一**直接钉「孤儿
//! 进程还在不在跑」的用例,证据只有 CI 的 ubuntu / macOS。Windows 上守得住的
//! 是 `wlwl-std` 里的 `a_group_is_only_killed_when_this_run_created_it`。
//!
//! ## ⚠️ **Windows 侧不钉「耗时」**（2026-10-30 收口夜）
//!
//! 上面两条 timeout 用例的「**耗时有界**」断言**只在 POSIX 上钉**。
//! **理由（实测，不是推测）**：Windows 的杀树走 `taskkill /T /F` —— 它是一个
//! **子进程 spawn** ⇒ 它的耗时**随机器负载在 1–3.5 s 之间随机波动**（实测：
//! 空载 0.9 s；同文件并行 + 其它用例起进程时，测到 1.4 s / 2.5 s / 3.4 s / 4.0 s）
//! ⇒ 任何**绝对**耗时上界都会被它撞穿（实测 **5 次里 3 次红**）;而**相对**判据
//! （两次测量之差）同样不可靠 —— 实测**两次测量各自**被抬 1–2.6 s ⇒ 差值也会超限。
//! ⇒ Windows 侧只能钉**结果形态**（`ERR` 值）。
//!
//! ⚠ **这与本文件头的既有结论一致**：「「耗时有界」**只有 POSIX 咬得到**」
//! ⇒ 现在是**代码对齐文件头**（原先两条在 Windows 上也钉耗时 ⇒ 与文件头矛盾）。
//! ⚠ **Windows 的这半由 `wlwl-std` 的单测守住**：
//! `drain_is_bounded_even_when_the_reader_never_finishes`（直接钉 `drain` 的有界性，
//! 与「杀不杀得掉进程树」无关）· `a_timeout_kills_the_child_instead_of_hanging`。
//! ⇒ **POSIX 上「耗时有界」仍然钉着**（`killpg` 是一次系统调用 ⇒ 不引入上面的波动）。
//!//! ## ⚠️ 阻塞代价必须在这里也写一遍
//!
//! 运行时是**单线程协作式** ⇒ `PROCESS_RUN` 等子进程期间**不调度任何其他
//! 任务**。与 `std.time::SLEEP` 的阻塞代价**同源**。契约能钉住「返回值是
//! `{code, stdout, stderr}` 这个字典」,但**钉不住也不该假装钉得住**「它阻塞」
//! —— 那是文档条款(`the_blocking_cost_is_documented` 那条测试守它)。

mod common;

use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};
use wlwl_eval::Evaluator;

/// **两条 timeout 用例互斥**（计划登记 ⑦ 的修法方向之一：「标为需串行」）。
///
/// ⚠ **实测**（2026-10-30 收口夜）：这两条**并行**跑时 **5 次里 3 次红**
///   （两条的耗时上界都被撞：3.59 s / 4.13 s，而上界是 2.5 s / 3 s）;
///   **单跑绿**（1.46 s）⇒ 是**并行干扰**，不是回归。
///   ⚠ 本仓其它「会真的起进程 / 会真的等」的用例也是这么串的
///   （见 `middleware_contract.rs` / `sync_contract.rs` 的 `gate()`）。
static TIMEOUT_GATE: Mutex<()> = Mutex::new(());

fn timeout_gate() -> MutexGuard<'static, ()> {
    TIMEOUT_GATE.lock().unwrap_or_else(|e| e.into_inner())
}

struct Case {
    name: &'static str,
    src: &'static str,
    expect: &'static str,
}

const CASES: &[Case] = &[
    // ── argv 的形状:只收数组 ────────────────────────────────────────
    Case {
        name: "a_string_argv_is_e0030",
        src: concat!(
            r#"IMPORT("wlwl:std.process", ["PROCESS_RUN", "PROCESS_ID"]); "#,
            r#"PROCESS_RUN("cmd /C dir")"#
        ),
        // 拼字符串再解析 = 命令注入。**规范显式禁止字符串形态**,所以这是原生诊断。
        expect: "!E0030 PROCESS_RUN: expected an argv array of strings, got string",
    },
    Case {
        name: "an_empty_argv_is_e0030",
        src: concat!(
            r#"IMPORT("wlwl:std.process", ["PROCESS_RUN", "PROCESS_ID"]); "#,
            r#"PROCESS_RUN([])"#
        ),
        expect: "!E0030 PROCESS_RUN: argv must have at least one element (the program)",
    },
    Case {
        name: "a_non_string_argv_element_is_e0030",
        src: concat!(
            r#"IMPORT("wlwl:std.process", ["PROCESS_RUN", "PROCESS_ID"]); "#,
            r#"PROCESS_RUN(["cmd", 7])"#
        ),
        expect: "!E0030 PROCESS_RUN: expected an argv array of strings, got integer",
    },
    Case {
        name: "process_run_arity",
        src: concat!(
            r#"IMPORT("wlwl:std.process", ["PROCESS_RUN", "PROCESS_ID"]); "#,
            r#"PROCESS_RUN()"#
        ),
        expect: "!E0022 PROCESS_RUN: function expects 1 argument(s), got 0",
    },
    Case {
        name: "process_id_takes_no_arguments",
        src: concat!(
            r#"IMPORT("wlwl:std.process", ["PROCESS_RUN", "PROCESS_ID"]); "#,
            r#"PROCESS_ID(1)"#
        ),
        expect: "!E0022 PROCESS_ID: function expects 0 argument(s), got 1",
    },
    // ── opts 的形状 ─────────────────────────────────────────────────
    Case {
        name: "an_unknown_opts_key_is_e0030",
        src: concat!(
            r#"IMPORT("wlwl:std.process", ["PROCESS_RUN", "PROCESS_ID"]); "#,
            r#"PROCESS_RUN(["cmd"], ["shell": TRUE])"#
        ),
        // ⚠️ 尤其 `shell` 必须被**显式拒绝**而不是忽略 —— 「忽略」和「禁止」
        // 在安全上是两件不同的事。
        expect: "!E0030 PROCESS_RUN: unknown opts key `shell` (timeout / cwd / env)",
    },
    Case {
        name: "opts_must_be_a_dictionary",
        src: concat!(
            r#"IMPORT("wlwl:std.process", ["PROCESS_RUN", "PROCESS_ID"]); "#,
            r#"PROCESS_RUN(["cmd"], 5)"#
        ),
        expect: "!E0030 PROCESS_RUN: expected an opts dictionary, got integer",
    },
    Case {
        name: "a_negative_timeout_is_e0030",
        src: concat!(
            r#"IMPORT("wlwl:std.process", ["PROCESS_RUN", "PROCESS_ID"]); "#,
            r#"PROCESS_RUN(["cmd"], ["timeout": -1])"#
        ),
        expect:
            "!E0030 PROCESS_RUN: expected opts.timeout to be a non-negative integer, got integer",
    },
    // ── 失败形态:进程起不来是 ERR **值** ──────────────────────────────
    Case {
        name: "a_missing_program_is_an_err_value",
        src: concat!(
            r#"IMPORT("wlwl:std.process", ["PROCESS_RUN", "PROCESS_ID"]); "#,
            r#"IS_ERR(PROCESS_RUN(["wlwl_no_such_program_zzz_42"]))"#
        ),
        // 「这个程序不存在」是**可预期的运行期结果**,调用方多半要 `IS_ERR`
        // 分支 ⇒ 用 `ERR` 值而不是终止运行的原生诊断。
        expect: "TRUE",
    },
];

fn scratch_dir() -> PathBuf {
    common::scratch("wlwl_proc_contract")
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
///
/// ⚠⚠ **两条断言缺一不可**(2026-10-16 从 probe 搬过来时补记,`D13-054`):
/// ① 「参数原样到达」② 「`&` 没有真的执行第二条命令」。
/// **只用 `INDEX(...) >= 0` 证明不了 ②** —— 参数原样到达时它必然命中,而 `&`
/// 真的被执行时它**也**会命中(输出里同样含那段字面量)。② 靠「stdout 只有
/// 一行」(`!\nINJECTED`)才真正区分两者。这段论证原先只写在 probe 的 `_why` 里,
/// 用例被移除时会跟着丢,所以搬到这里。
///
/// ⚠ **本用例是 `std.process` 注入防线的唯一跨平台钉法** —— probe 用例
/// `M6_std_fs_paths_and_process_safety` 原本也探这条,但它探的是 `cmd /C`,
/// 而 `cmd` 只存在于 Windows ⇒ macOS / ubuntu 恒红(那个缺口自 2026-10-08 起就在,
/// 因 CI 此前死在 clippy、**测试步骤从来没跑过**)。probe 侧已移除并留了指路。
#[test]
fn argv_elements_are_never_shell_interpreted() {
    let src = if cfg!(windows) {
        r#"IMPORT("wlwl:std.process", ["PROCESS_RUN"]);
AT_K(PROCESS_RUN(["cmd", "/C", "echo", "a & echo INJECTED"]), "stdout", "")"#
    } else {
        // ⚠ 三个转义 / 元数坑,都在这里踩过(`D13-055` / `D13-056`):
        //   ① wlwl 字符串里**必须有 `\"`** —— lexer(`wlwl-lexer/src/lib.rs`
        //      的 `Some(b'"') => push(b'"')`)**支持** `\"`,不写就会让内层引号
        //      提前闭合并让 `$1` 落到字符串外 ⇒ `E0001 illegal character '$'`。
        //      ⚠ 本文件用 Rust **原始字符串** `r#"…"#`,所以里面的 `\"` 原样
        //      就是 wlwl 源码里的 `\"`,**不需要**在 Rust 这层再转义。
        //   ② `$1` 里的 `$` **不需要**转义:lexer 只在 `$` 后面紧跟 `{` 时
        //      才当插值(`Some(b'$') if peek_at(1) == Some(b'{')`),`$1` 落回
        //      普通字符。写成 `\$1` 反而会让 wlwl 交出**字面** `$1`,sh 就
        //      不会展开它 —— 那测到的是另一件事。
        //   ③ `AT_K` 是**三元**的 `(dict, key, 缺省)`。原先这个分支只写了
        //      两个 ⇒ `E0022 function AT_K expects 3 argument(s), got 2`。
        //      ⚠ **它被 ① 挡住了**:词法错误先炸,元数错误根本没机会报出来 ——
        //      所以「只验解析通过」这种局部验证会漏掉下一层。修 ① 之后 ③ 才
        //      浮出来。⇒ 验证要**跑**整个程序,不能只 `check`。
        // ⚠ 被测参数放进 `$1` 让它**只展开一次**;若直接写进 `-c` 的脚本体,
        //   `&` 会被 sh 在**第一轮**就当操作符 —— 那测的是「sh 的引号处理」
        //   而不是「argv 有没有被拼接」。
        r#"IMPORT("wlwl:std.process", ["PROCESS_RUN"]);
AT_K(PROCESS_RUN(["sh", "-c", "printf %s \"$1\"", "sh", "a & echo INJECTED"]), "stdout", "")"#
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

/// 失败载荷必须带 `kind` / `op` / `reason`,且 **reason 里含失败的程序名**。
///
/// ⚠⚠ **只钉前缀,不钉操作系统那句原文**(`D13-055`,2026-10-16):原文是
/// **Rust `std::io::Error` 的 Display** —— 那是标准库的措辞,不是本库的可观察
/// 契约。冻结它等于把「换了 libc / 换了 OS」也变成一条契约,没有任何语义理由。
/// 实测:Windows `program not found` / Linux `No such file or directory
/// (os error 2)` —— **本条在 Linux / macOS 上从未通过过**,它此前以 Windows
/// 的措辞整串冻结在 `CASES` 表里。⇒ 移出表、改钉本库自己负责的那段前缀。
#[test]
fn the_failure_payload_carries_kind_op_reason() {
    let out = actual(
        r#"IMPORT("wlwl:std.process", ["PROCESS_RUN", "PROCESS_ID"]);
ERR_PAYLOAD(PROCESS_RUN(["wlwl_no_such_program_zzz_42"]))"#,
    );
    assert!(
        out.starts_with(
            "[kind: ProcessError, op: PROCESS_RUN, reason: spawn wlwl_no_such_program_zzz_42: "
        ),
        "kind / op / the program name must survive into the reason — got: {out}"
    );
    // ⚠ 反向:reason 至少得有内容(空 reason 会让调用方无从判断发生了什么)。
    assert!(out.trim_end().ends_with(']'), "got: {out}");
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
    let _g = timeout_gate();
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
    // ⚠ **「耗时有界」这条只在 POSIX 上钉**（理由见文件头「Windows 侧不钉耗时」）。
    #[cfg(unix)]
    assert!(
        elapsed < std::time::Duration::from_secs(3),
        "a 300 ms timeout took {elapsed:?} — the child tree was not killed"
    );
    #[cfg(not(unix))]
    let _ = elapsed;
}

/// ⚠🔴 **[N-011 / D13-061] 超时 + **活着的孙进程** ⇒ 耗时有界。**
///
/// 这条与 `a_timeout_yields_an_err_value` 的差别**不在断言,在被测形态**:
/// 那条只杀一个**叶子**子进程(`sleep 5` / `ping`),管道写端随它一起消失 ⇒
/// 读线程必然 EOF ⇒ **`drain` 的上界有没有,这条都绿**。
/// ⚠ 实测(本机 Windows、`taskkill` 正常):把 `drain` 退回缺陷形态后,
/// `wlwl-std` 的 `a_timeout_kills_the_child_instead_of_hanging` **仍然绿**
/// —— 它打在没有孙进程的路径上。
/// ⚠ 本条让子进程**再生一个孙进程**并让它握住 stdout/stderr:
/// 直接子进程被杀掉后,孙进程**仍活着、仍持有管道写端** ⇒ 管道不 EOF ⇒
/// **只有有界的 `drain` 能让这里返回**。这正是 `[D13-061]` 的真形态。
///
/// ⚠⚠ **平台无关的写法,但两个平台走的是**不同**路径**:
/// - **Windows**:祖先 → 孙的链是 `cmd` → `ping`,`taskkill /T /F` 把**整棵树**
///   杀掉 ⇒ 孙进程也死 ⇒ 管道 EOF ⇒ 这条**恒绿**,**与 `drain` 修没修无关**。
/// - **POSIX**:`child.kill()` 只给直接子进程发信号 ⇒ 孙进程**存活** ⇒ 旧实现
///   在这里无限期 `join()` ⇒ 这条**才真的验得到** `[D13-061]`。
///
/// 所以**本机 Windows 上它恒绿**,POSIX 才是它的有效平台。
/// ⚠ 在 Windows 上钉住这半的方法:**临时让 `kill_tree` 跳过 `taskkill`**
/// (等价于模拟 POSIX 的 `child.kill()`)⇒ 缺陷形态下本条实测 **7.13 s**(红),
/// 加上本修复后同条件下 **1.34 s**(绿)⇒ **是 `drain` 在起作用,不是
/// `taskkill`**。
#[test]
fn a_timeout_with_a_living_grandchild_returns_promptly() {
    let _g = timeout_gate();
    let argv = if cfg!(windows) {
        // `cmd` 是子进程、`ping` 是孙进程;`> NUL` 让它别把管道写爆。
        r#"["cmd", "/C", "ping -n 31 127.0.0.1 > NUL"]"#
    } else {
        // ⚠ 形状:`sh -c <脚本>` 必须是**三个** argv 元素(`D13-057` 同款坑)。
        //   脚本里的 `sleep 30 &` 起一个**后台孙进程**(继承 stdout/stderr),
        //   紧跟的 `sleep 30` 是前台子进程 —— 超时被杀的正是它。
        r#"["sh", "-c", "sleep 30 & sleep 30"]"#
    };
    let src = format!(
        r#"IMPORT("wlwl:std.process", ["PROCESS_RUN"]);
IS_ERR(PROCESS_RUN({argv}, ["timeout": 300]))"#
    );
    let started = std::time::Instant::now();
    let out = actual(&src);
    let elapsed = started.elapsed();
    assert_eq!(out, "TRUE", "a timeout must be an ERR value — got {out}");
    // ⚠ **「耗时有界」这条只在 POSIX 上钉**（理由见文件头「Windows 侧不钉耗时」）。
    #[cfg(unix)]
    assert!(
        elapsed < std::time::Duration::from_millis(2500),
        "a 300 ms timeout took {elapsed:?} — a surviving grandchild still holds the \
         pipe write end, so the drain was unbounded"
    );
    #[cfg(not(unix))]
    let _ = elapsed;
}

/// 🔴 **[N-037 项三 / D13-068] 超时之后**不得留下活着的孙进程**。
///
/// ⚠ 这条钉的不是「返回得快」(快慢由 `wlwl-std` 里 `drain` 那两条守卫负责),而是
///   **进程还在不在跑**。`[D13-061]` 把「我们的线程挂死」救出来了,但**孤儿本身
///   还在跑**:它占着 CPU 到自己结束,在 wlwl 退出之后也照样占着那些管道。
///
/// **可数出的代理指标**:孙进程每秒往一个文件追加一行;`PROCESS_RUN` 返回之后数
/// 两次行数 —— **不再增长**才算它真的死了。
/// ⚠ 刻意**不用耗时**做判据:修好时起作用的是「管道早 EOF」,与快慢无关;而快慢
///   会被 CI 机器的负载搅动(仓库里已有 `D13-062` 用两档耗时比的先例,那条量的
///   是复杂度,不是这里这种「有没有人还在动」)。
///
/// ⚠ **两侧余量是算出来的,不是抄某次运行**:
/// - 心跳周期 1 s、观测窗 **2500 ms** ⇒ 一个**活着**的孙进程在这个窗里**至少**写
///   `⌊2500 / 1000⌋ = 2` 行。阈值取「0 行增长」⇒ **红侧余量 ≥ 2 行**,不可能是
///   噪声;反过来,组被杀时**恒定**是 0 行增长 ⇒ 绿侧没有边界在晃。
/// - 防「空转恒绿」靠 `count1 >= 1`:孙进程**一上来就写第一行**(循环体里没有先
///   sleep),而超时给足 800 ms ⇒ 「文件一行都没有」只可能是夹具没跑起来,那时
///   这条会**因无意义而失败**,而不是悄悄通过。
///
/// ⚠⚠ **本用例 `#[cfg(unix)]` —— 本机(Windows)连编译都到不了。**
///   `taskkill /T /F` 在 Windows 上已经把整棵树杀了,同一个夹具在那边**恒绿**,
///   在那儿跑也不构成任何证据(与本文件既有那条同款理由)。⇒ 它的验证通路**只有
///   CI 的 ubuntu / macOS**,这一段**没有本机实测证据**,如实登记。
#[cfg(unix)]
#[test]
fn a_timeout_leaves_no_living_grandchild() {
    // ⚠ 名字必须唯一(D13-078):旧写法只拿时钟当唯一性来源,而时钟的分辨粒度
    // 在 macOS 上只有 1 微秒 ⇒ 并行取名会撞 ⇒ 撞了就互相覆盖心跳文件。
    // ⇒ 走共享取名(进程 id + 原子计数 + 纳秒),文件放在那个唯一目录里。
    let beat = common::scratch("wlwl_proc_beat").join("beat");
    let _ = std::fs::remove_file(&beat);
    let path = beat.to_string_lossy().into_owned();
    // 路径被塞进 shell 脚本的**单引号**里(wlwl 字符串里 `'` 是普通字符,只有
    // `\"` / `\` / `${` 需要转义)。路径本身带单引号的场合本仓**造不出来**,
    // 真出现了就明说,而不是悄悄生成一个错的脚本。
    assert!(
        !path.contains('\''),
        "temp path would break the quoting: {path}"
    );

    // `sh -c <脚本>` 必须是**三个** argv 元素(`D13-057` 同款坑)。脚本把心跳循环
    // 放**后台**、前台 `sleep 30` —— 超时被杀的正是前台那个;后台那个就是
    // ⚠ **握着 stdout/stderr 写端的活着的孙进程**(非交互 shell 默认不开 job
    // control ⇒ 后台孙进程留在**同一个进程组**里,正是 `killpg` 覆盖得到的)。
    let src = format!(
        r#"IMPORT("wlwl:std.process", ["PROCESS_RUN"]);
IS_ERR(PROCESS_RUN(["sh", "-c", "while :; do echo x >> '{path}'; sleep 1; done & sleep 30"], ["timeout": 800]))"#
    );
    assert_eq!(actual(&src), "TRUE", "a timeout must be an ERR value");

    let lines = || {
        std::fs::read_to_string(&beat)
            .map(|s| s.lines().count())
            .unwrap_or(0)
    };
    let first = lines();
    assert!(
        first >= 1,
        "the grandchild never wrote a heartbeat line — the fixture did not run, so \
         the growth check below would be vacuously true"
    );
    std::thread::sleep(std::time::Duration::from_millis(2500));
    let second = lines();
    let _ = std::fs::remove_file(&beat);
    assert_eq!(
        second, first,
        "the heartbeat kept growing ({first} -> {second} lines): a grandchild of the \
         timed-out child is STILL RUNNING — on POSIX `Child::kill()` only signals the \
         direct child, so the whole group must be killed instead (N-037 item 3)"
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
    let body = common::chapter_body(&text, "## 标准库规范 · 19 `std.process`");
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
    assert_eq!(spec.len(), 3, "§19 table found {spec:?}");
    for n in &impls {
        assert!(spec.contains(n), "§19 table missed `{n}`: {spec:?}");
    }
    assert_eq!(impls.len(), 3, "wlwl:std.process exports 3 members");
}

/// ⚠️ 阻塞代价是**语言级事实**,不是实现细节 ⇒ 必须在规范里成文。
/// 契约钉不住「它阻塞」(那是时间性质),但能钉住「那句话还在」。
#[test]
fn the_blocking_cost_is_documented() {
    let text = std::fs::read_to_string(spec_path()).expect("stdlib spec readable");
    // ⚠ 判据**收窄到 §19**。落在整册上时,别处的同名文字会让这条一直绿 ——
    //   修之前实测把 §19.2 里「**它是阻塞的**」那句改掉,整册判据**仍然绿**
    //   (整册里 `阻塞` 出现 58 次,§19 里的「不经 shell」也另有 2 处)。
    let body = common::chapter_body(&text, "## 标准库规范 · 19 `std.process`");
    // ⚠ 三个锚点在 §19 内**各自唯一**:`不经 shell` 在 §19 里出现 3 次(章标题 /
    //   成员表 / §19.5 示例)、`阻塞` 出现 4 次 —— 拿它们当 needle 等于判
    //   「§19 里提到过这些词」⇒ 条款被删光也照样绿。改钉**条款所在的那一句**。
    for needle in ["不经 shell 解析", "它是阻塞的", "命令注入"] {
        assert!(
            body.contains(needle),
            "stdlib spec §19 lost `{needle}` — the no-shell / blocking-cost clauses of \
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
        .join("wlwl-spec-reference.md")
}
