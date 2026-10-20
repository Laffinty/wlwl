//! [F-01] `wlwl:std.io::PRINT` / `PRINT_ERR` 与全局内建**逐字节同形**。
//!
//! ## 为什么要有这一条
//!
//! 这道缺陷是**静默**的:同一份程序,只要加一行
//! `IMPORT("wlwl:std.io", ["PRINT"])`,数组 / 字典 / `RESULT` 的输出形态
//! 就整体改变 ——
//!
//! | 值 | 全局内建 `PRINT` | 改前的 `std.io::PRINT` |
//! |---|---|---|
//! | `[1, 2, 3]` | `[1, 2, 3]` | `[1,2,3]` |
//! | `["a": 1, "b": "x"]` | `[a: 1, b: x]` | `{"a":1,"b":"x"}` |
//! | `["b": 1, "a": 2]` | `[b: 1, a: 2]` | `{"a":2,"b":1}`(键序被改写成字典序) |
//! | `OK(1)` | `OK(1)` | `1` |
//!
//! 根因是 `std.io` 仍走 `crate::wrap` 的 serde_json 中间表示,而全局内建
//! 走 `Value::display()`(语言规范 §2.5)。
//!
//! crate 内的单测(`crates/wlwl-std/src/io.rs`)能守住**渲染函数**;这一条
//! 守的是**跨进程的真实 stdout / stderr 字节** —— 也就是用户真正看见的东西。
//! 两条都要有:渲染函数对了但调用点没走它,进程级这条会红;调用点对了但
//! 渲染函数又漂了,crate 内那条会红。
//!
//! ## 手法
//!
//! 同一段程序跑两遍:**带** `IMPORT("wlwl:std.io", […])` 与**不带**。
//! 两个版本必须**逐字节**相同 —— 这正是「加不加这行 IMPORT 不改变行为」
//! 这条承诺的可执行形式。

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

/// 临时目录:唯一子目录 + 退出时清理(测试文件**不落仓库**)。
struct Tmp(PathBuf);

impl Tmp {
    fn new(tag: &str) -> Self {
        let p = std::env::temp_dir().join(format!(
            "wlwl_io_parity_{}_{}_{}",
            tag,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).expect("tmp dir created");
        Tmp(p)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
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

/// 被打印的值。**两个版本共用这一段**,只有 IMPORT 行不同。
///
/// ⚠ `ERR` 那一行是 `[ERR("e")]` 而不是 `ERR("e")`:语言规范 §12.6 的
/// **ERR 透明传播**在**调用边界**就把 ERR 实参短路掉 —— `PRINT(ERR("e"))`
/// 根本不会进 `PRINT` 的函数体(顶层会变成 E0102)。所以 ERR 值的渲染只能
/// 在**容器里**观察:`[ERR(e)]`。这不是 std.io 的取舍,是语言规则。
const BODY: &str = r#"
PRINT([1, 2, 3]);
PRINT(["a": 1, "b": "x"]);
PRINT(["b": 1, "a": 2]);
PRINT(OK(1));
PRINT([ERR("e")]);
PRINT(TRUE, FALSE, NULL);
PRINT(1.0);
PRINT([[1, 2], ["k": [3]]]);
"#;

/// 期望的 stdout(语言规范 §2.5 的口径)。
const EXPECTED_STDOUT: &str = concat!(
    "[1, 2, 3]\n",
    "[a: 1, b: x]\n",
    "[b: 1, a: 2]\n",
    "OK(1)\n",
    "[ERR(e)]\n",
    "TRUE FALSE NULL\n",
    "1.0\n",
    "[[1, 2], [k: [3]]]\n",
);

/// 把 `PRINT` 换成 `PRINT_ERR` 的同一段程序(走 stderr)。
const BODY_ERR: &str = r#"
PRINT_ERR([1, 2, 3]);
PRINT_ERR(["a": 1, "b": "x"]);
PRINT_ERR(["b": 1, "a": 2]);
PRINT_ERR(OK(1));
PRINT_ERR([ERR("e")]);
PRINT_ERR(TRUE, FALSE, NULL);
PRINT_ERR(1.0);
PRINT_ERR([[1, 2], ["k": [3]]]);
"#;

/// 期望的 stderr(与 [`EXPECTED_STDOUT`] 同形)。
const EXPECTED_STDERR: &str = EXPECTED_STDOUT;

/// 写一份程序并跑它,返回进程输出。
fn run_program(dir: &Path, file_name: &str, source: &str) -> Output {
    let path = dir.join(file_name);
    fs::write(&path, source).expect("program written");
    Command::new(wlwl_binary())
        .arg("run")
        .arg(&path)
        .current_dir(dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("wlwl runs")
}

/// 断言程序**跑成功**(两个版本都要)。
fn assert_ok(out: &Output, what: &str) {
    assert!(
        out.status.success(),
        "{what} must run clean:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// **主守卫**:带 `IMPORT("wlwl:std.io", ["PRINT"])` 与不带的两个版本,
/// stdout 必须**逐字节相同**,且等于语言规范 §2.5 的字面口径。
#[test]
fn std_io_print_is_byte_identical_to_the_global_builtin() {
    let tmp = Tmp::new("print");
    let dir = tmp.path();

    let imported = format!("IMPORT(\"wlwl:std.io\", [\"PRINT\"]);\n{BODY}");
    let out_imported = run_program(dir, "with_import.wll", &imported);
    let out_plain = run_program(dir, "without_import.wll", BODY);

    assert_ok(&out_imported, "program with IMPORT");
    assert_ok(&out_plain, "program without IMPORT");

    assert_eq!(
        out_imported.stdout,
        out_plain.stdout,
        "std.io::PRINT stdout must be byte-identical to the global builtin PRINT\n\
         with IMPORT:\n{}\nwithout IMPORT:\n{}",
        String::from_utf8_lossy(&out_imported.stdout),
        String::from_utf8_lossy(&out_plain.stdout),
    );

    // 反向守卫:不只是「两个版本一致」,还得是**规范里那个形状** ——
    // 否则两边一起漂成 JSON 风格,这条断言仍然全绿。
    assert_eq!(
        String::from_utf8_lossy(&out_imported.stdout),
        EXPECTED_STDOUT,
        "std.io::PRINT must render per language spec §2.5"
    );

    // 这两个程序都不该产生任何诊断。
    assert!(
        out_imported.stderr.is_empty(),
        "unexpected stderr:\n{}",
        String::from_utf8_lossy(&out_imported.stderr)
    );
    assert!(
        out_plain.stderr.is_empty(),
        "unexpected stderr:\n{}",
        String::from_utf8_lossy(&out_plain.stderr)
    );
}

/// `PRINT_ERR` 同款:stderr 逐字节相同,且 stdout 为空。
#[test]
fn std_io_print_err_is_byte_identical_to_the_global_builtin() {
    let tmp = Tmp::new("print_err");
    let dir = tmp.path();

    let imported = format!("IMPORT(\"wlwl:std.io\", [\"PRINT_ERR\"]);\n{BODY_ERR}");
    let out_imported = run_program(dir, "with_import.wll", &imported);
    let out_plain = run_program(dir, "without_import.wll", BODY_ERR);

    assert_ok(&out_imported, "program with IMPORT");
    assert_ok(&out_plain, "program without IMPORT");

    assert_eq!(
        out_imported.stderr,
        out_plain.stderr,
        "std.io::PRINT_ERR stderr must be byte-identical to the global builtin PRINT_ERR\n\
         with IMPORT:\n{}\nwithout IMPORT:\n{}",
        String::from_utf8_lossy(&out_imported.stderr),
        String::from_utf8_lossy(&out_plain.stderr),
    );

    assert_eq!(
        String::from_utf8_lossy(&out_imported.stderr),
        EXPECTED_STDERR,
        "std.io::PRINT_ERR must render per language spec §2.5"
    );

    // `PRINT_ERR` 只走 stderr —— stdout 必须**空**(逐字节)。
    assert!(
        out_imported.stdout.is_empty(),
        "PRINT_ERR must not write to stdout, got:\n{}",
        String::from_utf8_lossy(&out_imported.stdout)
    );
    assert!(
        out_plain.stdout.is_empty(),
        "PRINT_ERR must not write to stdout, got:\n{}",
        String::from_utf8_lossy(&out_plain.stdout)
    );
}
