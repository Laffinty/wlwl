//! `cargo test -p wlwl-cli --test conformance_static`
//!
//! 计划书 §12.1 点名的验收命令。它跑的是**真的二进制**(`wlwl check`),
//! 不是内部函数 —— 于是它同时验到三件事:
//!
//! 1. **S2 开启可抓错**:门开着时,每份夹具的期望码**必须**出现;
//! 2. **S1 默认零破坏**:同一批夹具在默认档(无清单)下**一条静态诊断都没有**
//!    且 `wlwl check` 通过(所以夹具一定**解析干净** —— 静态诊断不可能是
//!    语法错冒充的);
//! 3. `mode: both` 的控制组还必须 `wlwl run` 跑通,证明这套夹具与这套
//!    断言都不是摆设。故意违例的夹具**不**要求能跑:静态类型违例在运行期
//!    多半**也**是 `E0030`,那是 E0030 存在的意义。
//!
//! 夹具的期望写在**夹具文件的注释里**(见 `impl/tests/conformance/
//! static_types/README.md`),所以增删夹具不需要改本文件。
//!
//! 三条断言里有一条是**按需**的:只有 `mode: both` 的控制组要求程序
//! `wlwl run` 跑得通。故意违例的夹具不要求 —— 静态类型违例在运行期多半
//! **也**是 `E0030`,那是 E0030 存在的意义,不是夹具坏了。

use std::path::{Path, PathBuf};
use std::process::Command;

/// `impl/tests/conformance/`
fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("CARGO_MANIFEST_DIR should be impl/crates/wlwl-cli")
        .join("tests")
        .join("conformance")
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

/// 一份夹具的契约。
struct Contract {
    /// 门开着时必须出现的码
    expects: Vec<String>,
    /// 默认档也必须干净
    also_clean_when_off: bool,
}

/// 读夹具头部的两份注释。**不猜**:解析不出契约就让测试失败,免得夹具
/// 悄悄退化成「什么都不断言」。
fn contract_of(source: &str) -> Contract {
    let mut expects = Vec::new();
    let mut also_clean_when_off = false;
    for line in source.lines() {
        let line = line.trim();
        if !line.starts_with("//") {
            // 契约必须在文件开头的注释区。
            if !line.is_empty() {
                break;
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix("// expects:") {
            expects = rest.split_whitespace().map(|s| s.to_string()).collect();
        }
        if line.trim_end() == "// mode: both" {
            also_clean_when_off = true;
        }
    }
    Contract {
        expects,
        also_clean_when_off,
    }
}

/// 把一份夹具复制成独立工程(`gradual_typing` 可选),返回入口路径。
///
/// 独立工程而不是就地跑:契约检查会沿 import 图找签名文件,夹具目录本身
/// 不该被跑测试的过程改动。
fn stage(fixture: &Path, name: &str, gradual: Option<&str>) -> PathBuf {
    let dir = std::env::temp_dir()
        .join("wlwl-conformance-static")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("staging dir");
    for entry in std::fs::read_dir(fixture).expect("fixture dir") {
        let entry = entry.expect("dir entry");
        let path = entry.path();
        // `README.md` 不是夹具。
        if path.extension().and_then(|e| e.to_str()) == Some("md") {
            continue;
        }
        let target = dir.join(entry.file_name());
        if path.is_dir() {
            copy_tree(&path, &target);
        } else {
            std::fs::copy(&path, &target).expect("fixture file copied");
        }
    }
    if let Some(level) = gradual {
        std::fs::write(
            dir.join("wlwl.toml"),
            format!(
                "[package]\nname = \"conformance\"\nversion = \"0.0.1\"\nentry = \"main.wll\"\n\n[features]\ngradual_typing = \"{level}\"\n"
            ),
        )
        .expect("manifest written");
    }
    dir
}

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("nested dir");
    for entry in std::fs::read_dir(from).expect("nested read") {
        let entry = entry.expect("nested entry");
        let src = entry.path();
        let dst = to.join(entry.file_name());
        if src.is_dir() {
            copy_tree(&src, &dst);
        } else {
            std::fs::copy(&src, &dst).expect("nested file copied");
        }
    }
}

fn run_check(bin: &Path, dir: &Path, entry: &str) -> (bool, String) {
    let out = Command::new(bin)
        .arg("check")
        .arg(dir.join(entry))
        .output()
        .expect("wlwl check runs");
    (
        out.status.success(),
        format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ),
    )
}

fn run_program(bin: &Path, dir: &Path, entry: &str) -> (bool, String) {
    let out = Command::new(bin)
        .arg("run")
        .arg(dir.join(entry))
        .output()
        .expect("wlwl run runs");
    (
        out.status.success(),
        format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ),
    )
}

/// 一份夹具:**门开着报出期望的码** + **默认档一条都没有** + **程序本身能跑**。
fn assert_fixture(dir: &str, entry: &str) {
    let bin = wlwl_binary();
    let src = std::fs::read_to_string(fixture_root().join(dir).join(entry))
        .unwrap_or_else(|e| panic!("cannot read fixture {dir}/{entry}: {e}"));
    let contract = contract_of(&src);
    assert!(
        !contract.expects.is_empty() || contract.also_clean_when_off,
        "{dir}/{entry} declares neither `expects:` nor `mode: both` — \
         a fixture that asserts nothing is worse than no fixture"
    );

    // 1. 默认档(无清单):一条静态诊断都不能有,且必须通过。
    let off = stage(&fixture_root().join(dir), &format!("{dir}-off"), None);
    let (ok, out) = run_check(&bin, &off, entry);
    assert!(
        ok,
        "{dir}/{entry} must pass `wlwl check` by default:\n{out}"
    );
    for code in &contract.expects {
        assert!(
            !out.contains(code.as_str()),
            "{dir}/{entry} leaked {} with the static layer OFF:\n{out}",
            code
        );
    }

    // 2. 门开着:期望的码必须出现。
    let on = stage(
        &fixture_root().join(dir),
        &format!("{dir}-on"),
        Some("error"),
    );
    let (ok, out) = run_check(&bin, &on, entry);
    for code in &contract.expects {
        assert!(
            out.contains(code.as_str()),
            "{dir}/{entry} must report {} with gradual_typing=error:\n{out}",
            code
        );
    }
    // 声明 `mode: both` 的夹具:门开着也必须干净。
    if contract.also_clean_when_off {
        assert!(
            ok,
            "{dir}/{entry} declares `mode: both` but `check` failed with the layer on:\n{out}"
        );
    }

    // 3. 控制组(`mode: both`)还必须**真的能跑**。
    //
    //    故意违例的那几份**不**要求能跑:静态类型违例在运行期多半**也**是
    //    `E0030` 类型错误(那正是 E0030 存在的意义),要求它们 run 通过会
    //    把「静态层抓的类型错误」和「程序本身能跑」混成一件事。
    //    控制组承担另一半责任:它证明这套夹具与这套断言不是摆设 ——
    //    声明「两种模式都干净」的那两份,必须一次跑通。
    if contract.also_clean_when_off {
        let run = stage(&fixture_root().join(dir), &format!("{dir}-run"), None);
        let (ok, out) = run_program(&bin, &run, entry);
        assert!(
            ok,
            "{dir}/{entry} declares `mode: both`, so it must also RUN clean in \
             the default configuration:\n{out}"
        );
    }
}

/// §10.4 规划的 `static_types/` 目录。
#[test]
fn static_types_fixtures_diagnose_only_when_the_gate_is_on() {
    assert_fixture("static_types", "static_types.wll");
}

/// §10.4 规划的 `module_sig/` 目录:签名与实现**一致**时零诊断。
#[test]
fn a_module_signature_that_matches_its_implementation_is_silent() {
    let bin = wlwl_binary();
    let on = stage(
        &fixture_root().join("module_sig"),
        "module_sig-on",
        Some("error"),
    );
    let (ok, out) = run_check(&bin, &on, "main.wll");
    assert!(
        ok,
        "a consistent module signature must not produce any diagnostic:\n{out}"
    );
    let run = stage(&fixture_root().join("module_sig"), "module_sig-run", None);
    let (ok, out) = run_program(&bin, &run, "main.wll");
    assert!(ok, "the module_sig fixture must run:\n{out}");
}

/// 破坏一致性时**确实**报出来(逐条)—— 否则上面那条「一致时静默」可能
/// 只是因为契约检查根本没跑。改动在**临时副本**上做,夹具目录不动。
#[test]
fn breaking_the_signature_is_reported_in_both_directions() {
    let bin = wlwl_binary();
    let dir = stage(
        &fixture_root().join("module_sig"),
        "module_sig-broken",
        Some("error"),
    );

    // E0113:实现多导出了签名没声明的名字。
    std::fs::write(
        dir.join("math.wll"),
        "LET(add, FUN((a: INTEGER, b: INTEGER) : INTEGER, +(a, b)));\n\
         LET(PI, 3);\n\
         LET(secret, 4);\n\
         EXPORT([\"add\", \"PI\", \"secret\"]);\n",
    )
    .expect("patch math.wll");
    let (_, out) = run_check(&bin, &dir, "main.wll");
    assert!(
        out.contains("E0113"),
        "an export the signature does not declare must be E0113:\n{out}"
    );

    // E0114:签名声明了实现没导出的名字。
    std::fs::write(
        dir.join("math.wll.sig"),
        "EXPORT add (INTEGER, INTEGER) : INTEGER\nEXPORT PI : INTEGER\nEXPORT ghost : INTEGER\n",
    )
    .expect("patch signature");
    let (_, out) = run_check(&bin, &dir, "main.wll");
    assert!(
        out.contains("E0114"),
        "a declared-but-unexported name must be E0114:\n{out}"
    );

    // E0115:签名类型与实现注解冲突。
    std::fs::write(
        dir.join("math.wll.sig"),
        "EXPORT add (INTEGER, INTEGER) : STRING\nEXPORT PI : INTEGER\n",
    )
    .expect("patch signature");
    let (_, out) = run_check(&bin, &dir, "main.wll");
    assert!(
        out.contains("E0115"),
        "a signature type that conflicts with the annotation must be E0115:\n{out}"
    );
}
