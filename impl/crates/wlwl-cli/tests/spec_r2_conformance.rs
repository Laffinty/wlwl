//! v0.11.3 复核二的四条「实现不符合语言标准」缺陷的**端到端**契约测试。
//!
//! 判据全部来自 `docs/wlwl-spec-reference.md` 的原文条款,不是抄实现的
//! 输出 —— 抄实现的话,实现改了测试也跟着改,测试就再也抓不到任何东西。
//!
//! | 缺陷 | 规范条款 | 本文件钉住的东西 |
//! |---|---|---|
//! | R2-010 | §11.2 错误码表 | 输入文件不存在 → `E0061`(不是 `E0042`) |
//! | R2-011 | §11.1 | JSON 诊断的键名是 `errorCategory` |
//! | R2-012 | 附录 E.4 | `wlwl sig --format json` 的键名是 `exports[]` |
//! | R2-013 | §11.4 | 旁路签名文件解析失败 ⇒ `E0010` + 退出码 `3`,**无条件** |
//!
//! 走真二进制(`CARGO_BIN_EXE_wlwl`),与仓内既有的 CLI 契约测试同一套做法
//! —— 退出码与 stderr 的**组合**只有真跑一次才验得住。

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn wlwl_exe() -> PathBuf {
    // Cargo 为同名二进制设置 `CARGO_BIN_EXE_<name>`,指向刚构建出来的
    // 可执行文件 —— 这是跨平台定位测试二进制的规范做法。
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

fn run_cli(args: &[&str]) -> std::process::Output {
    Command::new(wlwl_exe())
        .args(args)
        .output()
        .expect("failed to spawn wlwl")
}

fn stderr_of(out: &std::process::Output) -> String {
    String::from_utf8_lossy(&out.stderr).to_string()
}

fn stdout_of(out: &std::process::Output) -> String {
    String::from_utf8_lossy(&out.stdout).to_string()
}

/// 每个用例一个**独立**目录:测试是并行的,共用一个目录会互相踩。
fn case_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("wlwl-cli-r2-spec").join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn write(dir: &Path, name: &str, content: &str) -> PathBuf {
    let p = dir.join(name);
    fs::write(&p, content).unwrap();
    p
}

// ── R2-010:「输入文件不存在」是 `E0061`,不是 `E0042` ─────────────
//
// ⚠ [v0.12.0] 原来的依据「规范 §11.2 错误码表:「| `E0061` | 文件 |
//   文件不存在 | 错误 |」」**已不存在** —— 规范仓 v1.0.2 把 `E0061` / `E0062`
//   移进了「本规范**不**定义其触发条件」的那张表（`std.fs` 的读 / 写失败
//   已改成 `ERR` 值 ⇒ 那两个码不再由 `std.fs` 产生）。
// ⇒ 下面的 `E0061` 现在是**本仓 CLI 自己的**用法（报「用户给的输入文件不存在」），
//   **不再有**规范条款为它背书 ⇒ 这里钉的是「**本仓选了哪个码**」，不是
//   「规范要求哪个码」。
//
// 而实现自己的码表把 `E0042` 定义成**清单/锁文件不一致**
// (`wlwl-error/src/lib.rs`),拿它报「文件读不到」是串码 —— 一个码号只对应
// 一种条件。
//
// ⚠ 这里钉的是**码**(`E0061`),不是退出码:`E0061` 不是解析失败,退出码
// 仍是 `1`(规范 §11.4 的「程序不对」)。把退出码一起钉住是为了防「靠改
// 退出码蒙混过关」。

#[test]
fn r2_010_run_on_missing_file_reports_e0061() {
    let dir = case_dir("r2_010_run");
    let missing = dir.join("nope.wll");
    let out = run_cli(&["run", missing.to_str().unwrap()]);
    let stderr = stderr_of(&out);
    assert!(
        stderr.contains("E0061"),
        "缺失输入文件必须报 E0061(规范 §11.2「文件不存在」),stderr={stderr}"
    );
    assert!(
        !stderr.contains("E0042"),
        "E0042 是「清单/锁文件不一致」,不能拿来报「文件不存在」: {stderr}"
    );
    assert_eq!(out.status.code(), Some(1), "stderr={stderr}");
}

#[test]
fn r2_010_ast_fmt_and_sig_on_missing_file_report_e0061() {
    let dir = case_dir("r2_010_others");
    let missing = dir.join("nope.wll");
    let m = missing.to_str().unwrap();

    for (sub, args) in [
        ("ast", vec!["ast", m]),
        ("fmt", vec!["fmt", m]),
        ("sig", vec!["sig", m]),
    ] {
        let out = run_cli(&args);
        let stderr = stderr_of(&out);
        assert!(
            stderr.contains("E0061"),
            "`wlwl {sub}` 缺失输入文件必须报 E0061,stderr={stderr}"
        );
        assert!(
            !stderr.contains("E0042"),
            "`wlwl {sub}` 不该用 E0042 报「文件不存在」: {stderr}"
        );
    }
}

// ── R2-011:JSON 诊断的键名是 `errorCategory` ───────────────────────
//
// 规范 §11.1:「`--format json`/`jsonl` 产生机器可读结构(含
// `error_schema_version`、**`errorCategory`**、`retryable`、`idempotent`、
// `suggestion_code`、`trace` 等字段)」。
//
// ⚠ **值**一直是对的(`E0020` 就是 `"name"`),错的只是**键**:按规范写的
// 工具读 `errorCategory` 读不到 ⇒ 分类信息等于不存在。

#[test]
fn r2_011_json_diagnostic_uses_the_spec_key_error_category() {
    let dir = case_dir("r2_011");
    let p = write(&dir, "undef.wll", "PRINT(NOPE);");
    let out = run_cli(&["run", "--format=json", p.to_str().unwrap()]);
    let stderr = stderr_of(&out);
    assert!(
        stderr.contains("\"errorCategory\""),
        "JSON 诊断必须带规范点名的 `errorCategory` 字段: {stderr}"
    );
    assert!(
        !stderr.contains("\"error_category\""),
        "旧键名 `error_category` 必须不再出现(规范 §11.1 写的是 `errorCategory`): {stderr}"
    );
    // 值也要对:`E0020` 是名字错,分类就是 `name`。
    let v: serde_json::Value =
        serde_json::from_str(stderr.trim()).expect("stderr is one JSON object");
    assert_eq!(v["errorCategory"], "name");
    assert_eq!(v["code"], "E0020");
}

#[test]
fn r2_011_jsonl_diagnostic_uses_the_spec_key_error_category() {
    let dir = case_dir("r2_011_jsonl");
    let p = write(&dir, "undef.wll", "PRINT(NOPE);");
    let out = run_cli(&["run", "--format=jsonl", p.to_str().unwrap()]);
    let stderr = stderr_of(&out);
    let line = stderr
        .lines()
        .find(|l| l.trim_start().starts_with('{'))
        .unwrap_or_else(|| panic!("no JSONL line in stderr: {stderr}"));
    let v: serde_json::Value = serde_json::from_str(line.trim()).expect("jsonl line parses");
    assert_eq!(v["errorCategory"], "name");
    assert!(v.get("error_category").is_none(), "旧键名不得残留: {v}");
}

// ── R2-012:`wlwl sig --format json` 的键名是 `exports[]` ────────────
//
// 规范附录 E.4:`| `exports[]` | 与 E.2 同构(见 E.2 的字段说明) |`。
// E.2(`wlwl interface`)用的就是 `exports` —— 同一份承诺在两个子命令里
// 键名必须一样。

#[test]
fn r2_012_sig_json_uses_exports_like_interface_does() {
    let dir = case_dir("r2_012");
    let p = write(
        &dir,
        "math.wll",
        "LET(add, FUN((a: INTEGER, b: INTEGER) : INTEGER, +(a, b)));\n\
         LET(PI, 3);\n\
         EXPORT([\"add\", \"PI\"]);\n",
    );
    let out = run_cli(&["sig", "--format=json", p.to_str().unwrap()]);
    let stdout = stdout_of(&out);
    assert!(
        out.status.success(),
        "sig --format json 应当成功,stderr={}",
        stderr_of(&out)
    );
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("stdout is one JSON object");
    let exports = v["exports"]
        .as_array()
        .expect("规范附录 E.4 要求 `exports[]` 数组");
    assert_eq!(exports.len(), 2);
    assert!(
        v.get("entries").is_none(),
        "旧键名 `entries` 必须不再出现(E.4 要求 `exports[]`): {v}"
    );
    // 条目形状与 E.2 同构:`name` + `kind`(+ `params`/`returns` 或 `type`)。
    let add = exports
        .iter()
        .find(|e| e["name"] == "add")
        .expect("`add` 在导出面里");
    assert_eq!(add["kind"], "function");
    assert_eq!(add["params"][0], "INTEGER");
    assert_eq!(add["returns"], "INTEGER");
}

#[test]
fn r2_012_sig_and_interface_agree_on_the_key_name() {
    // 「与 E.2 同构」是**跨子命令**的承诺:同一个模块,`sig` 与
    // `interface` 的导出数组必须用同一个键名。
    let dir = case_dir("r2_012_same_key");
    let p = write(&dir, "math.wll", "LET(PI, 3);\nEXPORT([\"PI\"]);\n");
    let sig = run_cli(&["sig", "--format=json", p.to_str().unwrap()]);
    let iface = run_cli(&["interface", p.to_str().unwrap()]);
    let sv: serde_json::Value = serde_json::from_str(&stdout_of(&sig)).expect("sig json");
    let iv: serde_json::Value = serde_json::from_str(&stdout_of(&iface)).expect("interface json");
    assert!(
        sv.get("exports").is_some() && iv.get("exports").is_some(),
        "两个子命令都必须用 `exports`: sig={sv} interface={iv}"
    );
    assert!(sv.get("entries").is_none(), "sig 不得再用 `entries`: {sv}");
}

// ── R2-013:旁路签名文件的语法失败 ⇒ `E0010` + 退出码 `3`,无条件 ──
//
// 规范 §11.4:「`3` = 解析失败」,并且「「读不出来的文本」出现在**旁路签名
// 文件**里同样是 `3`:签名写 `EXPORT add (ARRAY[INTEGER]: Comparable)` 报的
// 就是 `E0010`。**同一种故障不该因为走哪条路而拿到不同的退出码**」。
//
// ⚠ 关键在「**无条件**」:`gradual_typing` 缺省 `off`(规范 §9.4),此前
// 签名文件只在静态契约层启用时才被读取 ⇒ 开关全关时 `.wll.sig` 整体惰性,
// **解析失败一条诊断都没有**,`wlwl check` 拿着写坏了的签名文件照样 `rc=0`。

/// 摆一个工程:`main.wll` + 一份**写坏了**的旁路签名文件。
///
/// `gradual` 决定清单里 `[features] gradual_typing` 怎么写;
/// `None` = **完全没有** `[features]`(缺省 `off`,规范 §9.4)。
fn broken_sig_project(name: &str, gradual: Option<&str>) -> PathBuf {
    let dir = case_dir(name);
    let toml = match gradual {
        Some(g) => format!(
            "[package]\nname = \"p\"\nversion = \"0.0.1\"\nentry = \"main.wll\"\n\n\
             [features]\ngradual_typing = {g}\n"
        ),
        None => "[package]\nname = \"p\"\nversion = \"0.0.1\"\nentry = \"main.wll\"\n".to_string(),
    };
    fs::write(dir.join("wlwl.toml"), toml).unwrap();
    write(&dir, "main.wll", "LET(x, 1);\nPRINT(x);\n");
    // 规范 §11.4 点名的那个写法:签名文法不认它。
    write(
        &dir,
        "main.wll.sig",
        "EXPORT add (ARRAY[INTEGER]: Comparable)\n",
    );
    dir
}

#[test]
fn r2_013_broken_signature_file_is_a_parse_failure_even_with_typing_off() {
    // `gradual_typing` 缺省 `off` —— 门控关着,签名文件照样必须被读出来。
    let dir = broken_sig_project("r2_013_off", None);
    let out = run_cli(&["check", dir.join("main.wll").to_str().unwrap()]);
    let stderr = stderr_of(&out);
    assert_eq!(
        out.status.code(),
        Some(3),
        "旁路签名文件解析失败 ⇒ 退出码 3(规范 §11.4),stderr={stderr}"
    );
    assert!(
        stderr.contains("E0010"),
        "旁路签名文件的语法失败报 `E0010`,stderr={stderr}"
    );
}

#[test]
fn r2_013_broken_signature_file_is_a_parse_failure_in_every_typing_mode() {
    // 三个档位都要是 `3`:**同一种故障不该因为走哪条路而拿到不同的退出码**。
    for (name, gradual) in [
        ("r2_013_off_explicit", Some("\"off\"")),
        ("r2_013_warn", Some("\"warn\"")),
        ("r2_013_error", Some("\"error\"")),
    ] {
        let dir = broken_sig_project(name, gradual);
        let out = run_cli(&["check", dir.join("main.wll").to_str().unwrap()]);
        let stderr = stderr_of(&out);
        assert_eq!(
            out.status.code(),
            Some(3),
            "档位 {name:?} 下也必须是退出码 3,stderr={stderr}"
        );
        assert!(
            stderr.contains("E0010"),
            "档位 {name:?} 下必须报 E0010,stderr={stderr}"
        );
        // ⚠ **只报一次**。签名文件的语法失败有两条通路会看到它:无条件的
        // 语法体检,以及门控内的契约扫描。两处都报 = 同一条诊断出现两遍,
        // 用户会以为有两个不同的毛病。
        assert_eq!(
            stderr.matches("E0010").count(),
            1,
            "同一条签名解析失败只该报一次,stderr={stderr}"
        );
    }
}

#[test]
fn r2_013_run_and_check_agree_on_a_broken_signature_file() {
    // 规范 §11.4:「`check` 与 `run` 共用这张表:两者对解析失败的结论必须
    // 一致」。
    let dir = broken_sig_project("r2_013_run", None);
    let main = dir.join("main.wll");
    let check = run_cli(&["check", main.to_str().unwrap()]);
    let run = run_cli(&["run", main.to_str().unwrap()]);
    assert_eq!(
        check.status.code(),
        Some(3),
        "check ⇒ 3,stderr={}",
        stderr_of(&check)
    );
    assert_eq!(
        run.status.code(),
        Some(3),
        "run ⇒ 3,stderr={}",
        stderr_of(&run)
    );
}

#[test]
fn r2_013_a_valid_signature_file_is_not_reported() {
    // 反向守卫:这一层只抓**读不出来**的签名文件。一份**合法**的签名文件
    // 在任何档位下都不该产生诊断 —— 否则「无条件」就变成了「一律报错」。
    for (name, gradual) in [
        ("r2_013_valid_off", "\"off\""),
        ("r2_013_valid_error", "\"error\""),
    ] {
        let dir = case_dir(name);
        fs::write(
            dir.join("wlwl.toml"),
            format!(
                "[package]\nname = \"p\"\nversion = \"0.0.1\"\nentry = \"main.wll\"\n\n\
                 [features]\ngradual_typing = {gradual}\n"
            ),
        )
        .unwrap();
        write(
            &dir,
            "main.wll",
            "LET(x, 1);\nEXPORT([\"x\"]);\nPRINT(x);\n",
        );
        write(&dir, "main.wll.sig", "EXPORT x : INTEGER\n");
        let out = run_cli(&["check", dir.join("main.wll").to_str().unwrap()]);
        let stderr = stderr_of(&out);
        assert!(
            out.status.success(),
            "合法签名文件不该被报出来({name}),stderr={stderr}"
        );
        assert!(
            !stderr.contains("E0010"),
            "合法签名文件不该报 E0010({name}): {stderr}"
        );
    }
}

#[test]
fn r2_013_no_signature_file_at_all_is_still_clean() {
    // 没有签名文件 = 没有契约(那是 v0.9 行为),不是「读不出来」。
    let dir = case_dir("r2_013_absent");
    fs::write(
        dir.join("wlwl.toml"),
        "[package]\nname = \"p\"\nversion = \"0.0.1\"\nentry = \"main.wll\"\n",
    )
    .unwrap();
    write(&dir, "main.wll", "LET(x, 1);\nPRINT(x);\n");
    let out = run_cli(&["check", dir.join("main.wll").to_str().unwrap()]);
    assert!(
        out.status.success(),
        "没有签名文件时 `check` 必须干净通过,stderr={}",
        stderr_of(&out)
    );
}
