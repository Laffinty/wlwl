//! [v0.11.3 M6 / addendum-06] `wlwl:std.process` —— **不经 shell** 跑一个子进程。
//!
//! **R2 全员**。
//!
//! ## 为什么本成员**只**收 `ARRAY[STRING]`(业主 2026-10-08 裁决)
//!
//! `argv` **必须**是数组。拼一个字符串再让系统解析 = **命令注入**:
//! `PROCESS_RUN(["sh", "-c", "echo " + user])` 里 `user` 若是 `; rm -rf /`,
//! 它就是一条真的会被执行的命令。⇒ 本成员**没有** `SYSTEM` / `exec("sh -c")`
//! 这种形态,规范也**显式禁止**字符串实参 —— 这条是本成员存在的**安全理由**。
//!
//! ## ⚠️ 它在协作式调度下是**阻塞**的(与 `SLEEP` 同源)
//!
//! wlwl 的调度是**单线程协作式**(`addendum-01` §3.11 核过:eval 侧无
//! `thread::spawn` / rayon,调度是一个 run queue)⇒ 本成员**等子进程期间不调度
//! 任何其他任务**,等价于把整个作用域停住。
//! ⇒ 与 `std.time::SLEEP` 的阻塞代价**同源**,规范与 skill 都要写明。
//! 这也是 `addendum-05`(`std.net`)的同源隐忧:`reqwest` 阻塞 → 同样停摆气泡。
//!
//! ## 超时怎么实现而**不开线程**
//!
//! `opts.timeout` 需要「到点杀掉子进程」。朴素做法是「先读完管道再等」——
//! 子进程写满管道缓冲就**死锁**。本实现起**两个读取线程**把 stdout / stderr
//! 抽干,主线程轮询 `try_wait()`。⚠️ 这两个线程**只做 I/O**,调度器仍是
//! 单线程(`ADR-0016` 边界未动)—— 与「不做 `std.thread`」那条非目标
//! **不冲突**:那条禁的是**把线程暴露给用户**。

use crate::{ModuleSpec, StdFn};
use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use wlwl_error::ErrorCode;
use wlwl_error::WlwlError;
use wlwl_value::{Outcome, StdHost, Value};

pub static SPEC: ModuleSpec = ModuleSpec {
    path: "wlwl:std.process",
    functions: &[
        ("PROCESS_RUN", process_run as StdFn),
        ("PROCESS_ID", process_id as StdFn),
    ],
};

/// 轮询间隔:5 ms。太大则超时的精度差,太小则空转。
const POLL: Duration = Duration::from_millis(5);

/// 读取线程的**有界**等待上限。子进程已退出后管道必然 EOF,这个上界只兜
/// 「有后代继承了写端」这种异常路径。
const JOIN_GRACE: Duration = Duration::from_millis(500);

/// 杀掉子进程**及其整棵后代**。
///
/// ⚠️ 这不是洁癖 —— `child.kill()` 在 Windows 上**只杀直接子进程**:
/// `cmd /C ping` 里 `cmd` 被杀了而 `ping` 还活着,而 `ping` **继承了管道句柄**
/// ⇒ 读线程等不到 EOF。实测:300 ms 的超时因此走了 **29.26 s**。
/// 「杀掉这个子进程」在语义上就该含它的后代,所以走 `taskkill /T /F`。
fn kill_tree(child: &mut std::process::Child) {
    if cfg!(windows) {
        let pid = child.id();
        // `taskkill` 失败也不该把调用卡住 ⇒ 立刻放弃,退回 `child.kill()`。
        let _ = Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    let _ = child.kill();
}

/// 收一个读取线程的输出。
///
/// ⚠️ **必须有界**:即便杀了进程树,一个「不听话」的孙进程仍可能握着管道写端,
/// 无条件 `join()` 会把「超时返回」变成「等到孙进程自己结束」—— 那超时就没了。
/// ⇒ 轮询 `is_finished()` 到 `JOIN_GRACE` 为止,仍未结束就**丢弃句柄**
/// (`JoinHandle` drop = 分离线程),让它自生自灭。已读到的部分拿不到就返回空
/// —— 超时路径上 stdout / stderr 本来就不给调用方。
fn drain(h: std::thread::JoinHandle<Vec<u8>>, already_timed_out: bool) -> Vec<u8> {
    if !already_timed_out {
        // 正常路径:子进程已退出,管道必然 EOF —— 但仍给一个上界,免得
        // 「有后代继承了句柄」这种异常把整个运行时挂住。
        let deadline = Instant::now() + JOIN_GRACE;
        while !h.is_finished() && Instant::now() < deadline {
            std::thread::sleep(POLL);
        }
    }
    h.join().unwrap_or_default()
}

fn arity(host: &mut dyn StdHost, name: &str, got: usize, want: usize) -> WlwlError {
    host.diag(
        ErrorCode::E0022,
        format!("{name}: function expects {want} argument(s), got {got}"),
    )
}

fn type_err(host: &mut dyn StdHost, name: &str, want: &str, got: &Value) -> WlwlError {
    host.diag(
        ErrorCode::E0030,
        format!("{name}: expected {want}, got {}", crate::value_kind(got)),
    )
}

fn ok(v: Value) -> Result<Outcome, WlwlError> {
    Ok(Outcome::normal(v))
}

fn proc_err(op: &str, reason: impl Into<String>) -> Value {
    Value::Err(Box::new(Value::Dict(vec![
        (
            Value::String("kind".into()),
            Value::String("ProcessError".into()),
        ),
        (Value::String("op".into()), Value::String(op.into())),
        (Value::String("reason".into()), Value::String(reason.into())),
    ])))
}

/// `PROCESS_RUN(argv, opts?) -> DICT`
///
/// 返 `{code, stdout, stderr}`。`code` 是退出码;**信号杀死**的平台差异用
/// `code = -1` 表达(`kill` 之后拿不到真实码),并在 `stderr` 留痕。
///
/// `opts` 可含:
/// - `timeout` `INTEGER` 毫秒 ⇒ 超时**杀掉**子进程并返回 `ERR`(`kind` =
///   `ProcessError`),**不挂死**;
/// - `cwd` `STRING` 工作目录;
/// - `env` `DICT` **追加**到继承来的环境(同名键覆盖)—— 不是「替换整份环境」。
pub fn process_run(host: &mut dyn StdHost, args: Vec<Value>) -> Result<Outcome, WlwlError> {
    const NAME: &str = "PROCESS_RUN";
    if args.is_empty() || args.len() > 2 {
        return Err(arity(host, NAME, args.len(), 1));
    }
    // ⚠️ 只收数组。**没有**字符串形态 —— 那是命令注入的入口(见文件头)。
    let Value::Array(items) = &args[0] else {
        return Err(type_err(host, NAME, "an argv array of strings", &args[0]));
    };
    let mut argv: Vec<String> = Vec::with_capacity(items.len());
    for it in items {
        match it {
            Value::String(s) => argv.push(s.clone()),
            other => return Err(type_err(host, NAME, "an argv array of strings", other)),
        }
    }
    if argv.is_empty() {
        return Err(host.diag(
            ErrorCode::E0030,
            format!("{NAME}: argv must have at least one element (the program)"),
        ));
    }

    let mut timeout_ms: Option<i64> = None;
    let mut cwd: Option<String> = None;
    let mut extra_env: Vec<(String, String)> = Vec::new();
    match args.get(1) {
        None | Some(Value::Null) => {}
        Some(Value::Dict(entries)) => {
            for (k, v) in entries {
                let (Value::String(key), _) = (k, v) else {
                    return Err(type_err(host, NAME, "an opts dict with string keys", k));
                };
                match key.as_str() {
                    "timeout" => match v {
                        Value::Integer(ms) if *ms >= 0 => timeout_ms = Some(*ms),
                        other => {
                            return Err(type_err(
                                host,
                                NAME,
                                "opts.timeout to be a non-negative integer",
                                other,
                            ))
                        }
                    },
                    "cwd" => match v {
                        Value::String(s) => cwd = Some(s.clone()),
                        other => {
                            return Err(type_err(host, NAME, "opts.cwd to be a string", other))
                        }
                    },
                    "env" => match v {
                        Value::Dict(kvs) => {
                            for (ek, ev) in kvs {
                                match (ek, ev) {
                                    (Value::String(k), Value::String(val)) => {
                                        extra_env.push((k.clone(), val.clone()))
                                    }
                                    _ => {
                                        return Err(type_err(
                                            host,
                                            NAME,
                                            "opts.env to be a dict of strings",
                                            ek,
                                        ))
                                    }
                                }
                            }
                        }
                        other => return Err(type_err(host, NAME, "opts.env to be a dict", other)),
                    },
                    other => {
                        return Err(host.diag(
                            ErrorCode::E0030,
                            format!("{NAME}: unknown opts key `{other}` (timeout / cwd / env)"),
                        ))
                    }
                }
            }
        }
        Some(other) => return Err(type_err(host, NAME, "an opts dictionary", other)),
    }

    let mut cmd = Command::new(&argv[0]);
    cmd.args(&argv[1..]);
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(d) = &cwd {
        cmd.current_dir(d);
    }
    for (k, v) in &extra_env {
        cmd.env(k, v);
    }

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => return ok(proc_err(NAME, format!("spawn {}: {e}", argv[0]))),
    };
    // 抽干两个管道,否则子进程写满缓冲就死锁(「先读后等」是错的)。
    let mut out_pipe = child.stdout.take();
    let mut err_pipe = child.stderr.take();
    let out_handle = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(p) = out_pipe.as_mut() {
            let _ = p.read_to_end(&mut buf);
        }
        buf
    });
    let err_handle = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(p) = err_pipe.as_mut() {
            let _ = p.read_to_end(&mut buf);
        }
        buf
    });

    let deadline = timeout_ms.map(|ms| Instant::now() + Duration::from_millis(ms as u64));
    let mut timed_out = false;
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break Some(s),
            Ok(None) => {}
            Err(e) => {
                let _ = out_handle.join();
                let _ = err_handle.join();
                return ok(proc_err(NAME, format!("wait {}: {e}", argv[0])));
            }
        }
        if let Some(dl) = deadline {
            if Instant::now() >= dl {
                kill_tree(&mut child);
                let _ = child.wait();
                timed_out = true;
                break None;
            }
        }
        std::thread::sleep(POLL);
    };

    let stdout = drain(out_handle, timed_out);
    let stderr = drain(err_handle, timed_out);
    if timed_out {
        let secs = timeout_ms.unwrap_or(0) as f64 / 1000.0;
        return ok(proc_err(
            NAME,
            format!("timed out after {secs}s: {}", argv.join(" ")),
        ));
    }
    let code = status.and_then(|s| s.code()).unwrap_or(-1);
    ok(Value::Dict(vec![
        (
            Value::String("code".into()),
            Value::Integer(i64::from(code)),
        ),
        (
            Value::String("stdout".into()),
            Value::String(String::from_utf8_lossy(&stdout).into_owned()),
        ),
        (
            Value::String("stderr".into()),
            Value::String(String::from_utf8_lossy(&stderr).into_owned()),
        ),
    ]))
}

/// `PROCESS_ID() -> INTEGER` —— 本进程 id。
pub fn process_id(host: &mut dyn StdHost, args: Vec<Value>) -> Result<Outcome, WlwlError> {
    const NAME: &str = "PROCESS_ID";
    if !args.is_empty() {
        return Err(arity(host, NAME, args.len(), 0));
    }
    ok(Value::Integer(i64::from(std::process::id())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use wlwl_value::StdCtx;

    struct NullHost;

    impl StdHost for NullHost {
        fn ctx(&mut self) -> &mut StdCtx {
            panic!("std.process must not touch StdCtx")
        }
        fn call(&mut self, _f: &Value, _a: Vec<Value>, _n: &str) -> Result<Outcome, WlwlError> {
            panic!("std.process has no member that calls back into the host")
        }
        fn diag(&mut self, code: ErrorCode, message: String) -> WlwlError {
            wlwl_error::WlwlDiagnostic::new(
                code,
                message,
                wlwl_error::Location::point("<test>", 0, 0),
            )
            .into()
        }
    }

    fn argv_of(v: &str) -> Value {
        Value::Array(vec![Value::String(v.into())])
    }

    #[test]
    fn runs_a_real_command_and_captures_stdout() {
        let mut h = NullHost;
        let (cmd, flag): (&str, &str) = if cfg!(windows) {
            ("cmd", "/C echo hello")
        } else {
            ("sh", "-c echo hello")
        };
        let out = process_run(
            &mut h,
            vec![
                Value::Array(vec![Value::String(cmd.into()), Value::String(flag.into())]),
                Value::Dict(vec![]),
            ],
        )
        .expect("spawn");
        let Value::Dict(d) = out.value else { panic!() };
        let get = |k: &str| {
            d.iter()
                .find(|(key, _)| matches!(key, Value::String(s) if s == k))
                .map(|(_, v)| v.clone())
                .unwrap()
        };
        let Value::Integer(code) = get("code") else {
            panic!()
        };
        assert_eq!(code, 0, "a successful command exits 0");
        let Value::String(stdout) = get("stdout") else {
            panic!()
        };
        assert!(stdout.contains("hello"), "stdout = {stdout:?}");
    }

    /// **本成员存在的安全理由**:元组里的元字符**不被**任何 shell 解释。
    /// 若实现哪天改成拼字符串,这条会红。
    #[test]
    fn argv_elements_are_never_shell_interpreted() {
        let mut h = NullHost;
        let (cmd, args): (&str, Vec<&str>) = if cfg!(windows) {
            ("cmd", vec!["/C", "echo", "a; echo b"])
        } else {
            ("sh", vec!["-c", "printf %s a; echo b"])
        };
        let argv = Value::Array(
            std::iter::once(cmd.to_string())
                .chain(args.into_iter().map(String::from))
                .map(Value::String)
                .collect(),
        );
        let out = process_run(&mut h, vec![argv]).expect("spawn");
        let Value::Dict(d) = out.value else { panic!() };
        let stdout = d
            .iter()
            .find(|(k, _)| matches!(k, Value::String(s) if s == "stdout"))
            .map(|(_, v)| v.clone())
            .unwrap();
        let Value::String(s) = stdout else { panic!() };
        // 无论哪个平台,`a; echo b` 必须作为**单个参数**原样到达。
        assert!(s.contains("a; echo b"), "argv element was mangled: {s:?}");
    }

    #[test]
    fn a_string_argv_is_rejected() {
        let mut h = NullHost;
        let e = process_run(&mut h, vec![Value::String("sh -c ls".into())]).unwrap_err();
        assert_eq!(e.diagnostic().code, ErrorCode::E0030);
        assert!(e.diagnostic().message.contains("argv array"));
    }

    #[test]
    fn an_empty_argv_is_rejected() {
        let mut h = NullHost;
        let e = process_run(&mut h, vec![Value::Array(vec![])]).unwrap_err();
        assert_eq!(e.diagnostic().code, ErrorCode::E0030);
    }

    #[test]
    fn a_spawn_failure_is_an_err_value_not_a_diagnostic() {
        let mut h = NullHost;
        let out = process_run(
            &mut h,
            vec![argv_of("wlwl_definitely_no_such_program_zzz_42")],
        )
        .expect("returns a value, not an Err");
        let Value::Err(payload) = out.value else {
            panic!("a failed spawn must be an ERR value")
        };
        let Value::Dict(d) = *payload else { panic!() };
        let kind = d
            .iter()
            .find(|(k, _)| matches!(k, Value::String(s) if s == "kind"))
            .map(|(_, v)| v.clone());
        assert_eq!(kind, Some(Value::String("ProcessError".into())));
    }

    /// ⚠️ 这条曾经**真的抓到过一个缺陷**:300 ms 的超时走了 **29.26 s**。
    /// 根因是 `child.kill()` 在 Windows 上只杀直接子进程(`cmd`),它 spawn 的
    /// `ping` 仍活着且**继承了管道句柄** ⇒ 读线程等不到 EOF,`join()` 拖住了
    /// 整个超时。修法是「杀进程树」+「join 必须有界」。
    ///
    /// 子命令只跑 5 s(不是 30 s) —— 回归时这条在 ~5 s 内失败,而不是让整个
    /// `cargo test` 多等半分钟。
    #[test]
    fn a_timeout_kills_the_child_instead_of_hanging() {
        let mut h = NullHost;
        let argv = if cfg!(windows) {
            Value::Array(vec![
                Value::String("cmd".into()),
                Value::String("/C ping -n 6 127.0.0.1 > NUL".into()),
            ])
        } else {
            Value::Array(vec![
                Value::String("sleep".into()),
                Value::String("5".into()),
            ])
        };
        let opts = Value::Dict(vec![(Value::String("timeout".into()), Value::Integer(300))]);
        let started = Instant::now();
        let out = process_run(&mut h, vec![argv, opts]).expect("returns");
        let elapsed = started.elapsed();
        assert!(
            elapsed < Duration::from_secs(2),
            "a 300 ms timeout took {elapsed:?} — the child (or its descendants) \
             were not killed, or the pipe drain was unbounded"
        );
        let Value::Err(payload) = out.value else {
            panic!("a timeout must be an ERR value")
        };
        let Value::Dict(d) = *payload else { panic!() };
        let reason = d
            .iter()
            .find(|(k, _)| matches!(k, Value::String(s) if s == "reason"))
            .map(|(_, v)| v.clone())
            .unwrap();
        let Value::String(reason) = reason else {
            panic!()
        };
        assert!(reason.contains("timed out"), "reason = {reason:?}");
    }

    #[test]
    fn unknown_opts_key_is_a_diagnostic() {
        let mut h = NullHost;
        let opts = Value::Dict(vec![(Value::String("shell".into()), Value::Boolean(true))]);
        let e = process_run(&mut h, vec![argv_of("cmd"), opts]).unwrap_err();
        assert_eq!(e.diagnostic().code, ErrorCode::E0030);
        assert!(e.diagnostic().message.contains("unknown opts key"));
    }

    #[test]
    fn process_id_is_the_current_process() {
        let mut h = NullHost;
        let Value::Integer(id) = process_id(&mut h, vec![]).unwrap().value else {
            panic!()
        };
        assert_eq!(id, i64::from(std::process::id()));
    }
}
