//! [X-05] `wlwl:std.io::INPUT` 与全局内建 `INPUT` —— **同名同语义**。
//!
//! ## 为什么要有这一条
//!
//! 规范(标准库·系统册 §1.1)写的是 `INPUT(prompt?) -> STRING`,§1.2 进一步
//! 要求「两条路径的可观察行为**必须**逐字相同」。而实现此前是:
//!
//! | | 元数 | prompt | EOF |
//! |---|---|---|---|
//! | 全局内建 `INPUT` | 0 或 1 | 有(`STRING`) | `ERR("EOF")` |
//! | `std.io::INPUT`(改前) | **只收 0** | **没有** | `""` |
//!
//! ⇒ 给 `std.io` 的 `INPUT` 传一个 prompt 会得到 `E0022`,而同样的程序
//! 不导入 `std.io` 却能跑 —— **加不加那行 `IMPORT` 改变了行为**。
//!
//! 修法在**实现侧**(规范本来就写对了):`std.io::INPUT` 收 0 或 1 个实参,
//! 给了 prompt 就先把它写到标准输出再读。
//!
//! ## 手法
//!
//! 同一段程序跑两遍:**带** `IMPORT("wlwl:std.io", ["INPUT"])` 与**不带**。
//! 两个版本喂**同一份 stdin**,stdout 必须**逐字节**相同 —— 这正是
//! 「加不加这行 IMPORT 不改变行为」的可执行形式(与 `io_print_parity.rs`
//! 同款手法)。
//!
//! ⚠ **EOF 不在这条的对拍范围内**:标准库·系统册 §1.2 把 EOF 钉死成
//! 「一个字节都没读到 ⇒ 返回 `""`」,而全局内建在 EOF 上返回
//! `ERR("EOF")` —— 那是**内建层**的口径(内建可以产 `ERR` 值,stdlib 成员
//! 不产)。该分歧已登记在规范仓一致性册 §5.4;本文件只对拍**读到一行**的
//! 那条路径。

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

/// 临时目录:唯一子目录 + 退出时清理(测试文件**不落仓库**)。
struct Tmp(PathBuf);

impl Tmp {
    fn new(tag: &str) -> Self {
        let p = std::env::temp_dir().join(format!(
            "wlwl_io_input_{}_{}_{}",
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

/// 被测程序体(两个版本共用,只有 IMPORT 行不同)。
///
/// prompt **不带换行** —— 换行由读到的内容后面那个 `PRINT` 补。
const BODY: &str = r#"
LET(line, INPUT("name: "));
PRINT(line);
"#;

/// 期望的 stdout:prompt 紧贴着读到的内容,然后是 `PRINT` 补的换行。
const EXPECTED_STDOUT: &str = "name: hello\n";

/// 写一份程序并跑它,stdin 喂 `input`,返回进程输出。
fn run_program(dir: &Path, file_name: &str, source: &str, input: &str) -> Output {
    let path = dir.join(file_name);
    fs::write(&path, source).expect("program written");
    let mut child = Command::new(wlwl_binary())
        .arg("run")
        .arg(&path)
        .current_dir(dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("wlwl starts");
    child
        .stdin
        .as_mut()
        .expect("stdin piped")
        .write_all(input.as_bytes())
        .expect("stdin written");
    child.wait_with_output().expect("wlwl finishes")
}

fn assert_ok(out: &Output, what: &str) {
    assert!(
        out.status.success(),
        "{what} must run clean:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// **主守卫**:带 `IMPORT("wlwl:std.io", ["INPUT"])` 与不带的两个版本,
/// 喂同一份 stdin,stdout 必须**逐字节**相同,且等于字面口径。
#[test]
fn std_io_input_is_byte_identical_to_the_global_builtin() {
    let tmp = Tmp::new("parity");
    let dir = tmp.path();

    let imported = format!("IMPORT(\"wlwl:std.io\", [\"INPUT\"]);\n{BODY}");
    let out_imported = run_program(dir, "with_import.wll", &imported, "hello\n");
    let out_plain = run_program(dir, "without_import.wll", BODY, "hello\n");

    assert_ok(&out_imported, "program with IMPORT");
    assert_ok(&out_plain, "program without IMPORT");

    assert_eq!(
        out_imported.stdout,
        out_plain.stdout,
        "std.io::INPUT stdout must be byte-identical to the global builtin INPUT\n\
         with IMPORT:\n{}\nwithout IMPORT:\n{}",
        String::from_utf8_lossy(&out_imported.stdout),
        String::from_utf8_lossy(&out_plain.stdout),
    );

    // 反向守卫:不只是「两个版本一致」,还得是**规范里那个形状** ——
    // prompt 紧贴着读到的内容(规范 §1.1:「先把 `prompt` 写到标准输出再读」)。
    assert_eq!(
        String::from_utf8_lossy(&out_imported.stdout),
        EXPECTED_STDOUT,
        "std.io::INPUT must write the prompt before reading"
    );

    assert!(
        out_imported.stderr.is_empty(),
        "unexpected stderr:\n{}",
        String::from_utf8_lossy(&out_imported.stderr)
    );
}

/// 0 个实参**合法**(元数下界是 0):`INPUT()` 与 `INPUT("name: ")` 都得跑通。
#[test]
fn std_io_input_accepts_zero_arguments() {
    let tmp = Tmp::new("zero");
    let dir = tmp.path();

    let src = "IMPORT(\"wlwl:std.io\", [\"INPUT\"]);\nLET(line, INPUT());\nPRINT(line);\n";
    let out = run_program(dir, "zero.wll", src, "plain\n");

    assert_ok(&out, "INPUT() with zero arguments");
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "plain\n",
        "INPUT() must read a line without a prompt"
    );
}

/// EOF **不是**失败:一个字节都没读到 ⇒ 返回 `""`(标准库·系统册 §1.2),
/// 不是 `E0060`,也不是 `ERR` 值。
#[test]
fn std_io_input_at_eof_returns_empty_string() {
    let tmp = Tmp::new("eof");
    let dir = tmp.path();

    let src = "IMPORT(\"wlwl:std.io\", [\"INPUT\"]);\nLET(line, INPUT());\nPRINT(line);\n";
    let out = run_program(dir, "eof.wll", src, "");

    assert_ok(&out, "INPUT() at EOF");
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "\n",
        "EOF must yield an empty string, not a diagnostic"
    );
}

/// >1 个实参 → `E0022`(标准库·系统册 §1.1 的失败列)。
#[test]
fn std_io_input_rejects_more_than_one_argument() {
    let tmp = Tmp::new("arity");
    let dir = tmp.path();

    let src = "IMPORT(\"wlwl:std.io\", [\"INPUT\"]);\nINPUT(\"a\", \"b\");\n";
    let out = run_program(dir, "arity.wll", src, "");

    assert!(!out.status.success(), "two arguments must fail");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("E0022"),
        "two arguments must report E0022, got:\n{stderr}"
    );
}

/// prompt 非 `STRING` → `E0030`(标准库·系统册 §1.1 的失败列)。
#[test]
fn std_io_input_rejects_non_string_prompt() {
    let tmp = Tmp::new("type");
    let dir = tmp.path();

    let src = "IMPORT(\"wlwl:std.io\", [\"INPUT\"]);\nINPUT(42);\n";
    let out = run_program(dir, "type.wll", src, "");

    assert!(!out.status.success(), "non-string prompt must fail");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("E0030"),
        "non-string prompt must report E0030, got:\n{stderr}"
    );
}
