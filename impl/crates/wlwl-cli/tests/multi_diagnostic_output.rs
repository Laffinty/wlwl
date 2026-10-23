//! `cargo test -p wlwl-cli --test multi_diagnostic_output`
//!
//! [v1.0 规范 §8.6]「丢弃时逐个报告」的**打印侧**护栏。
//!
//! ## 两层护栏,各管一件事
//!
//! | 层 | 钉住什么 | 在哪 |
//! |---|---|---|
//! | 函数级 | **一个错误携带 N 条诊断 ⇒ 输出里出现 N 条**(顺序 = 出现序) | `src/diag_output.rs` 的单元测试 |
//! | 进程级 | CLI **把拿到的每一条诊断都打印出来**;退出码按**第一条**的类别定 | 本文件 |
//!
//! ⚠ 为什么「一个错误携带 N 条诊断」这条只能是**函数级**:
//! `wlwl-cli` 是**纯二进制** crate(没有 lib target)⇒ `tests/` 下的集成测试
//! 只能驱动**进程**;而运行期此刻还**造不出**一个携带多条诊断的错误
//! (`WlwlError::Diagnostics` 还没有被构造出来)⇒ 进程级**无从触发**这条
//! 规则。规则住在模块里、测试住在模块旁边,才盯得住它。
//!
//! 进程级这一层盯的是**另一半**:诊断**收集**侧已经攒了 N 条时,打印侧
//! **不许**只打印第一条(那条就是 §8.6 违规的现场)。

use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;

fn wlwl_exe() -> PathBuf {
    // Cargo sets CARGO_BIN_EXE_<name> for the binary of the same crate,
    // pointing at the freshly-built executable. This is the canonical way
    // to locate the test binary across platforms and cargo working
    // directories.
    if let Ok(p) = std::env::var("CARGO_BIN_EXE_wlwl") {
        let p = PathBuf::from(p);
        if p.exists() {
            return p;
        }
    }
    // Fall back to relative path lookups.
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

fn run_cli(args: &[&str]) -> std::process::Output {
    Command::new(wlwl_exe())
        .args(args)
        .output()
        .expect("failed to spawn wlwl")
}

/// 摆一个独立工程(临时目录),返回入口文件路径。
///
/// 独立工程而不是就地跑:契约检查会沿 import 图找签名文件,夹具目录本身
/// 不该被跑测试的过程改动。
fn stage(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let dir = std::env::temp_dir()
        .join("wlwl-multi-diagnostic-output")
        .join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("staging dir");
    for (file, content) in files {
        let path = dir.join(file);
        let mut f = fs::File::create(&path).expect("file created");
        f.write_all(content.as_bytes()).expect("file written");
    }
    dir.join("main.wll")
}

/// stderr 里每条诊断的码(`error[E0110]: …` → `E0110`)。
fn diagnostic_codes(stderr: &str) -> Vec<String> {
    stderr
        .lines()
        .filter_map(|l| {
            let start = l.find('[')?;
            let end = l[start..].find(']')? + start;
            let code = &l[start + 1..end];
            if code.starts_with('E') || code.starts_with('W') {
                Some(code.to_string())
            } else {
                None
            }
        })
        .collect()
}

/// 同时踩两个静态头的程序:非穷尽 `MATCH`(该 `E0116`)+ 注解不匹配(该
/// `E0110`)。两个码在同一个输出里,才证明打印侧**没有**只取第一条。
const TWO_STATIC_DIAGNOSTICS: &str = "LET(r, ERR(\"boom\"));\n\
     LET(out, MATCH(r, [[OK(v), \"ok\"]]));\n\
     PRINT(out);\n\
     LET(flag: BOOLEAN, \"yes\");\n\
     PRINT(flag);\n";

const MANIFEST: &str = "[package]\nname = \"p\"\nversion = \"0.0.1\"\nentry = \"main.wll\"\n\n\
     [features]\ngradual_typing = \"error\"\n";

/// 静态层攒了**两条**诊断 ⇒ **两条**都打印出来(不许只打第一条),退出码 `1`。
#[test]
fn every_collected_diagnostic_is_printed_not_just_the_first_one() {
    let entry = stage(
        "two-static-diagnostics",
        &[
            ("wlwl.toml", MANIFEST),
            ("main.wll", TWO_STATIC_DIAGNOSTICS),
        ],
    );
    let out = run_cli(&["check", entry.to_str().unwrap()]);
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    let codes = diagnostic_codes(&stderr);
    assert!(
        codes.iter().any(|c| c == "E0110"),
        "E0110 该出现,stderr={stderr}"
    );
    assert!(
        codes.iter().any(|c| c == "E0116"),
        "E0116 该出现(只打第一条就会缺它),stderr={stderr}"
    );
    let mut uniq = codes.clone();
    uniq.sort();
    uniq.dedup();
    assert_eq!(
        uniq.len(),
        codes.len(),
        "同一条诊断不该被打印两遍,stderr={stderr}"
    );
    assert_eq!(
        out.status.code(),
        Some(1),
        "程序不对 ⇒ 退出码 1(规范 §11.4),stderr={stderr}"
    );
}

/// 一条**语法错** ⇒ 退出码 `3`(规范 §11.4),且**只**打印一条诊断。
///
/// 这条钉的是「打印侧改成逐条之后,单条诊断的行为一个字都没变」。
#[test]
fn a_parse_failure_prints_exactly_one_diagnostic_and_exits_3() {
    let entry = stage("parse-failure", &[("main.wll", "LET(x, 1\n")]);
    let out = run_cli(&["run", entry.to_str().unwrap()]);
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    assert_eq!(
        out.status.code(),
        Some(3),
        "源文件没能解析出来 ⇒ 退出码 3(规范 §11.4),stderr={stderr}"
    );
    let codes = diagnostic_codes(&stderr);
    assert_eq!(codes.len(), 1, "一条诊断 ⇒ 一段输出,stderr={stderr}");
    assert!(codes[0].starts_with('E'), "stderr={stderr}");
}

/// 运行期诊断(不是解析失败)⇒ 退出码 `1`,且只打印一条。
#[test]
fn a_runtime_error_prints_one_diagnostic_and_exits_1() {
    let entry = stage("runtime-error", &[("main.wll", "PRINT(no_such_name);\n")]);
    let out = run_cli(&["run", entry.to_str().unwrap()]);
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    assert_eq!(
        out.status.code(),
        Some(1),
        "能解析出来但不对 ⇒ 退出码 1(规范 §11.4),stderr={stderr}"
    );
    let codes = diagnostic_codes(&stderr);
    assert_eq!(codes.len(), 1, "一条诊断 ⇒ 一段输出,stderr={stderr}");
}

/// `--format=json` 下,多条诊断仍然是**一条一个 JSON 对象**(与既有逐条
/// 打印的形状一致)—— 打印侧改成逐条**不能**顺手把 JSON 形状改了。
#[test]
fn json_format_prints_one_object_per_diagnostic() {
    let entry = stage(
        "two-static-diagnostics-json",
        &[
            ("wlwl.toml", MANIFEST),
            ("main.wll", TWO_STATIC_DIAGNOSTICS),
        ],
    );
    let out = run_cli(&["check", "--format=json", entry.to_str().unwrap()]);
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    let objects = stderr.matches("\"code\"").count();
    let e0110 = stderr.matches("E0110").count();
    let e0116 = stderr.matches("E0116").count();
    assert!(e0110 >= 1 && e0116 >= 1, "两个码都该出现,stderr={stderr}");
    assert_eq!(
        objects,
        e0110 + e0116,
        "JSON 形态:一条诊断一个对象,stderr={stderr}"
    );
}
