//! v0.10 Step 10(计划书 §5.3 P1-3):`wlwl lsp` 的**端到端**冒烟。
//!
//! `lsp.rs` 里的单测是在进程内驱动协议处理器;这里起**真的子进程**,
//! 把 JSON-RPC 帧写进它的 stdin、从 stdout 读回来 —— 覆盖那些只有真
//! 传输才暴露的东西:分帧、`shutdown` / `exit` 的收工、以及 stdout 没被
//! 别的输出污染(编辑器对这一点零容忍)。
//!
//! 不引任何测试框架依赖:用 `CARGO_BIN_EXE_wlwl`(cargo 为 bin 目标提供)
//! 定位刚编出来的可执行文件,与 `cli_subcommands.rs` 同一套做法。

use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn wlwl_exe() -> PathBuf {
    if let Ok(p) = std::env::var("CARGO_BIN_EXE_wlwl") {
        let p = PathBuf::from(p);
        if p.exists() {
            return p;
        }
    }
    panic!("CARGO_BIN_EXE_wlwl is not set; run this through `cargo test`");
}

fn frame(payload: &str) -> String {
    format!("Content-Length: {}\r\n\r\n{payload}", payload.len())
}

/// 读一帧;收尾行返回 `None`。
fn read_frame(r: &mut impl BufRead) -> Option<String> {
    let mut len: Option<usize> = None;
    loop {
        let mut line = String::new();
        if r.read_line(&mut line).ok()? == 0 {
            return None;
        }
        let t = line.trim_end_matches(['\r', '\n']);
        if t.is_empty() {
            break;
        }
        if let Some((k, v)) = t.split_once(':') {
            if k.trim().eq_ignore_ascii_case("content-length") {
                len = v.trim().parse().ok();
            }
        }
    }
    let mut buf = vec![0u8; len?];
    r.read_exact(&mut buf).ok()?;
    String::from_utf8(buf).ok()
}

/// 完整握手:`initialize` → `didOpen`(带一条语法错)→ `hover` → `shutdown`
/// → `exit`。断言三件事:能握手、诊断被推出来、进程在 `exit` 后干净退出。
#[test]
fn lsp_handshake_diagnostics_and_clean_exit() {
    let mut child = Command::new(wlwl_exe())
        .arg("lsp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("wlwl lsp must start");

    let mut stdin = child.stdin.take().expect("stdin");
    let mut stdout = BufReader::new(child.stdout.take().expect("stdout"));

    let script = [
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"capabilities":{}}}"#,
        r#"{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///tmp/lsp-smoke/main.wll","languageId":"wlwl","version":1,"text":"LET(x: INTEGER, \"nope\");"}}}"#,
        r#"{"jsonrpc":"2.0","id":2,"method":"textDocument/hover","params":{"textDocument":{"uri":"file:///tmp/lsp-smoke/main.wll"},"position":{"line":0,"character":4}}}"#,
        r#"{"jsonrpc":"2.0","id":3,"method":"shutdown","params":null}"#,
        r#"{"jsonrpc":"2.0","method":"exit"}"#,
    ];
    for msg in script {
        stdin
            .write_all(frame(msg).as_bytes())
            .expect("the server must accept frames");
        stdin.flush().expect("flush");
    }
    drop(stdin);

    // 1. initialize 的响应。
    let first = read_frame(&mut stdout).expect("initialize response");
    let v: serde_json::Value = serde_json::from_str(&first).expect("JSON");
    assert_eq!(v["id"], 1);
    assert_eq!(v["result"]["capabilities"]["hoverProvider"], true);

    // 2. didOpen 推出来的诊断。
    let second = read_frame(&mut stdout).expect("publishDiagnostics");
    let v: serde_json::Value = serde_json::from_str(&second).expect("JSON");
    assert_eq!(v["method"], "textDocument/publishDiagnostics");
    let diags = v["params"]["diagnostics"].as_array().expect("array");
    // 默认档下静态层不发 E0110,但 parser 的 W0010(未使用的绑定)该在。
    // 断言「有诊断被推出来」这个事实本身,而不是某一行的内容。
    assert!(!diags.is_empty(), "expected at least one diagnostic");

    // 3. hover 的响应(一个非 null 的结果,说明它在服务)。
    let third = read_frame(&mut stdout).expect("hover response");
    let v: serde_json::Value = serde_json::from_str(&third).expect("JSON");
    assert_eq!(v["id"], 2);

    // 4. shutdown 的响应。
    let fourth = read_frame(&mut stdout).expect("shutdown response");
    let v: serde_json::Value = serde_json::from_str(&fourth).expect("JSON");
    assert_eq!(v["id"], 3);
    assert!(v["result"].is_null());

    // 5. exit 之后进程收工,不再吐帧。
    assert!(read_frame(&mut stdout).is_none(), "no frames after exit");
    let status = child.wait().expect("the server must exit cleanly");
    assert!(status.success(), "exit status: {:?}", status);
}

/// 只有 `initialize` + `exit` 的最小会话也必须干净收场(编辑器探测
/// 可用性时就是这么发的)。
#[test]
fn a_minimal_session_exits_cleanly() {
    let mut child = Command::new(wlwl_exe())
        .arg("lsp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("wlwl lsp must start");
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    for msg in [
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
        r#"{"jsonrpc":"2.0","method":"exit"}"#,
    ] {
        stdin.write_all(frame(msg).as_bytes()).unwrap();
        stdin.flush().unwrap();
    }
    drop(stdin);
    let first = read_frame(&mut stdout).expect("initialize response");
    assert!(first.contains("\"capabilities\""));
    assert!(read_frame(&mut stdout).is_none());
    assert!(child.wait().unwrap().success());
}
