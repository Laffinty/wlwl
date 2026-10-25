//! [G-19] IO 失败是 **`ERR` 值**，不是**终止运行的诊断**。
//!
//! ## 这条契约在钉什么
//!
//! v0.3 以来 `std.fs::READ_FILE` / `WRITE_FILE` 的 IO 失败是**原生诊断**
//! （`E0061` 文件不存在 / `E0062` 写失败 / `E0060` 其它）⇒ **终止运行**。
//! 在「每连接一个任务」的服务器里，这意味着**一次读文件失败会杀掉整个进程** ——
//! 症状是「端口还在听，但服务已经没了」。
//!
//! 现在 IO 失败是 `ERR([kind:"FsError", op, reason])` **值** ⇒ 调用方
//! `IS_ERR` 分支处理，**程序继续跑**。
//!
//! ⚠ **元数 / 类型错仍是诊断**（`E0022` / `E0030`）—— 那是程序员错误，
//!   与「文件不在」这种可预期的运行期结果不同类。

use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Output};

fn wlwl_exe() -> PathBuf {
    if let Ok(p) = std::env::var("CARGO_BIN_EXE_wlwl") {
        let p = PathBuf::from(p);
        if p.exists() {
            return p;
        }
    }
    let exe_name = if cfg!(windows) { "wlwl.exe" } else { "wlwl" };
    let candidates = [
        std::env::current_dir()
            .unwrap()
            .join("target/debug")
            .join(exe_name),
        PathBuf::from(format!("target/debug/{exe_name}")),
        PathBuf::from(exe_name),
    ];
    candidates
        .into_iter()
        .find(|c| c.exists())
        .unwrap_or_else(|| panic!("wlwl binary not found; build it first with `cargo build`"))
}

fn scratch_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("wlwl_g19_{tag}"));
    fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

fn write_source(dir: &std::path::Path, name: &str, src: &str) -> PathBuf {
    let p = dir.join(name);
    let mut f = fs::File::create(&p).expect("create source");
    f.write_all(src.as_bytes()).expect("write source");
    p
}

fn run_cli(args: &[&str]) -> Output {
    Command::new(wlwl_exe())
        .args(args)
        .output()
        .expect("failed to spawn wlwl")
}

#[test]
fn a_failing_read_file_does_not_kill_the_program() {
    let dir = scratch_dir("read");
    let src = r#"
IMPORT("wlwl:std.fs", ["READ_FILE"]);
LET(r, READ_FILE("Z:/__wlwl_definitely_missing__/abc_xyz_123"));
IF(IS_ERR(r), (LET(p, ERR_PAYLOAD(r)); PRINT("caught-err:", AT_K(p, "reason", "?"))), (PRINT("not-err")));
PRINT("still-running");
"#;
    let p = write_source(&dir, "read.wll", src);
    let out = run_cli(&["run", p.to_str().unwrap()]);
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();

    assert!(
        out.status.success(),
        "an IO failure must NOT abort the program; status={:?} stderr={stderr}",
        out.status
    );
    assert!(
        stdout.contains("caught-err"),
        "the failure must be observable as a value; stdout={stdout}"
    );
    assert!(
        stdout.contains("still-running"),
        "the program must run to completion; stdout={stdout}"
    );
}

#[test]
fn a_failing_write_file_does_not_kill_the_program() {
    let dir = scratch_dir("write");
    let missing_parent = dir.join("no_such_dir_zzz").join("x.txt");
    let src = format!(
        r#"
IMPORT("wlwl:std.fs", ["WRITE_FILE"]);
LET(r, WRITE_FILE("{}", "hello"));
IF(IS_ERR(r), (LET(p, ERR_PAYLOAD(r)); PRINT("caught-err:", AT_K(p, "reason", "?"))), (PRINT("not-err")));
PRINT("still-running");
"#,
        missing_parent.to_string_lossy().replace('\\', "/")
    );
    let p = write_source(&dir, "write.wll", &src);
    let out = run_cli(&["run", p.to_str().unwrap()]);
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();

    assert!(
        out.status.success(),
        "an IO failure must NOT abort the program; status={:?} stderr={stderr}",
        out.status
    );
    assert!(
        stdout.contains("caught-err"),
        "the failure must be observable as a value; stdout={stdout}"
    );
    assert!(
        stdout.contains("still-running"),
        "the program must run to completion; stdout={stdout}"
    );
}
