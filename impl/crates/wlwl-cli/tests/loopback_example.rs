//! [v0.12.0 / G-09] **loopback 服务器基座**的冒烟测试。
//!
//! ## 它钉什么
//!
//! `impl/examples/loopback-server/` 是随仓发布的**可复制**的服务端骨架
//! （纯 WLWL 写的最小 HTTP 服务）。它的价值全在「读者能直接跑通」——
//! 一个跑不起来的示例比没有示例更糟（它教的是错误的知识）。
//!
//! ⇒ 这条测试**真的把它跑起来**：起子进程 → 读它打印的端口 → 连上去 →
//!   发一条 HTTP 请求 → 读响应 → 断言是那个 200 + body → 等子进程**自己退出**。
//!
//! ## 为什么它必须是**子进程**
//!
//! 示例要**监听端口** ⇒ 与测试进程同进程跑会占住端口、且端口不可预期
//! （示例用 `port = 0` 让宿主分配临时端口）⇒ 只能由外部客户端连它。
//!
//! ## 顺带钉住的**实现形态**（Q1 裁决后的等待模型）
//!
//! 「等待必须在任务里」「一次等待占一段」「等待是段首第一条」这三条
//! 是示例能跑通的前提 ⇒ 示例一旦写错，这条测试**当场红**（而不是
//! 「示例挂住了」—— 超时也是红）。

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// 示例目录（仓内相对 `impl/examples/loopback-server`）。
///
/// ⚠ `CARGO_MANIFEST_DIR` = `impl/crates/wlwl-cli` ⇒ **两级**上去是 `impl`
///   （三级上去就是仓根了 ⇒ 那样会找 `<仓根>/examples` ⇒ 不存在）。
fn example_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("examples")
        .join("loopback-server")
}

fn wlwl_binary() -> PathBuf {
    if let Ok(p) = std::env::var("CARGO_BIN_EXE_wlwl") {
        let p = PathBuf::from(p);
        if p.exists() {
            return p;
        }
    }
    panic!("wlwl binary not found; run `cargo build --bin wlwl` first");
}

#[test]
fn the_loopback_example_serves_one_request_and_exits() {
    let dir = example_dir();
    assert!(
        dir.join("wlwl.toml").exists(),
        "the example must ship its manifest: {}",
        dir.display()
    );

    let mut child = Command::new(wlwl_binary())
        .arg("run")
        .arg(&dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn wlwl run <example dir>");

    // ⚠ 端口是**示例自己打印**的（`port = 0` ⇒ 宿主分配临时端口）
    //   ⇒ 必须**读子进程的第一行**才能知道连哪儿。
    let mut stdout = BufReader::new(child.stdout.take().expect("piped stdout"));
    let mut first_line = String::new();
    let read_at = Instant::now();
    while read_at.elapsed() < Duration::from_secs(20) {
        first_line.clear();
        let n = stdout
            .read_line(&mut first_line)
            .expect("read the port line");
        if n == 0 {
            break;
        }
        if first_line.contains("127.0.0.1:") {
            break;
        }
    }
    let port: u16 = first_line
        .split_whitespace()
        .find_map(|w| w.trim_end_matches(':').parse::<u16>().ok())
        .unwrap_or_else(|| {
            let _ = child.kill();
            panic!("the example must print its port; got {first_line:?}")
        });

    // 连上去、发一条请求、读响应。
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect to the example");
    stream
        .set_read_timeout(Some(Duration::from_secs(20)))
        .expect("read timeout");
    stream
        .write_all(b"GET /probe HTTP/1.1\r\nHost: x\r\n\r\n")
        .expect("write request");
    stream.flush().expect("flush");

    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).expect("read response");
    let response = String::from_utf8_lossy(&buf).to_string();

    // ⇒ 示例必须**自己退出**（它只服务一个请求）⇒ 这条同时钉住「不会挂住终端」。
    let status = child.wait().expect("the example must exit on its own");

    assert!(
        response.starts_with("HTTP/1.1 200 OK"),
        "the example must answer with a 200, got {response:?}"
    );
    assert!(
        response.contains("hello-wlwl!"),
        "the example's body must come back, got {response:?}"
    );
    assert!(
        status.success(),
        "the example must exit cleanly after one request, got {status:?}"
    );
}
