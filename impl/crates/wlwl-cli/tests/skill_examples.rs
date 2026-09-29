//! [v0.10.1] `wlwl-skill/examples/` 的冒烟测试。
//!
//! **背景**:这个目录此前**不在任何测试的覆盖面里**。`examples.rs` 只跑
//! `impl/examples/`,skill 随包发布的那 10 份示例只有人手工跑过 —— 与
//! R10-010 修掉的 `impl/examples/` 是同一个洞,只是没被发现,因为
//! `showcase.wll` / `std_test.wll` 坏掉时也没人喊。
//!
//! skill 的示例是**给 agent 读的第一手材料**。一份跑不通的示例比没有示例
//! 更糟:它教的是错误的知识,而且读者(人或 agent)往往直接照抄。
//!
//! ## 为什么在 `examples/` 里跑而不是暂存到别处
//!
//! 目录里有三样东西依赖「当前工作目录」:
//!
//! - `wlwl.toml` —— `entry = "static_contracts.wll"`,清单按 CWD 向上找;
//! - `wlwl.lock` —— 同上;
//! - 相对 `IMPORT` —— `IMPORT("wlwl:std.json", ...)` 走内建命名空间,
//!   而 `import_stdlib.wll` 演示的 `fs` 路径解析按 CWD 走。
//!
//! 暂存会把这些语义从测试里抹掉,那不是让测试更严,是让测试测不到它声称
//! 要测的东西。`import_stdlib.wll` 里的 `EXISTS` 用的是**机器相关的绝对
//! 路径**(已在那份文件里写明),所以它不参与断言 —— 见下面的例外表。

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

/// `wlwl-skill/examples/`
///
/// `CARGO_MANIFEST_DIR` = `impl/crates/wlwl-cli`,往上三层才是仓库根
/// (`impl/crates` -> `impl` -> 根)。层数写错会静默跑到 `D:\Project` 那种
/// 地方去,所以这里用一次性的链式 `and_then`,少一层就 `None` 而不是指错。
fn examples_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent() // impl/crates
        .and_then(|p| p.parent()) // impl
        .and_then(|p| p.parent()) // repo root
        .and_then(|p| p.join("wlwl-skill").join("examples").canonicalize().ok())
        .unwrap_or_else(|| {
            panic!(
                "cannot locate wlwl-skill/examples from {}; expected the repo root two \
                 levels above `impl`",
                env!("CARGO_MANIFEST_DIR")
            )
        })
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

/// 直线程序的宽松上限。示例都是几十行,1 秒绰绰有余;给足余量是为了不让
/// 慢机器上的偶发调度延迟变成红灯。
const EXAMPLE_TIMEOUT: Duration = Duration::from_secs(60);

fn example_files() -> Vec<String> {
    let root = examples_root();
    let mut names: Vec<String> = std::fs::read_dir(&root)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", root.display()))
        .map(|e| e.expect("dir entry"))
        .filter(|e| e.path().is_file())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".wll"))
        .collect();
    names.sort();
    assert!(
        !names.is_empty(),
        "no examples found under {} — the smoke test would silently pass on zero",
        root.display()
    );
    names
}

fn run_example(bin: &Path, name: &str) -> (bool, String, bool) {
    let dir = examples_root();
    let mut child = Command::new(bin)
        .arg("run")
        .arg(name)
        .current_dir(&dir)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("wlwl spawns");

    // 管道读空线程 + 主线程轮询:只轮询会在管道写满时死锁(与 probe.rs 同一套)。
    let out_r = child.stdout.take().map(spawn_reader);
    let err_r = child.stderr.take().map(spawn_reader);

    let deadline = Instant::now() + EXAMPLE_TIMEOUT;
    let mut timed_out = false;
    let status = loop {
        match child.try_wait().expect("try_wait") {
            Some(s) => break s,
            None => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    timed_out = true;
                    break child.wait().expect("wait after kill");
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    };

    let mut out = String::new();
    if let Some(h) = out_r {
        out.push_str(&String::from_utf8_lossy(&h.join().unwrap_or_default()));
    }
    if let Some(h) = err_r {
        out.push_str(&String::from_utf8_lossy(&h.join().unwrap_or_default()));
    }
    (status.success() && !timed_out, out, timed_out)
}

fn spawn_reader<R: std::io::Read + Send + 'static>(mut r: R) -> std::thread::JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = r.read_to_end(&mut buf);
        buf
    })
}

/// **主测试**:每份示例都必须 `wlwl run` 退出码 0。
///
/// 一次性汇总全部失败,而不是在第一个就断 —— 修了示例 A 却不知道 B 坏着,
/// 是这类批量维护最常见的来回。
#[test]
fn every_shipped_skill_example_runs_clean() {
    let bin = wlwl_binary();
    let mut failures = Vec::new();
    for name in example_files() {
        let (ok, out, timed_out) = run_example(&bin, &name);
        if !ok {
            failures.push(if timed_out {
                format!("{name}: TIMEOUT after {}s", EXAMPLE_TIMEOUT.as_secs())
            } else {
                format!("{name}: {}", out.replace('\n', " | ").trim())
            });
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} shipped skill example(s) fail to run:\n  {}",
        failures.len(),
        example_files().len(),
        failures.join("\n  ")
    );
}

/// **元数据守卫**:示例目录不能悄悄变空,也不能悄悄少掉关键主题。
///
/// `EXPECTED_TOPICS` 是**主题**而不是文件名 —— 改名不该让这条测试红,
/// 但「OOP 示例被删了」必须红。
#[test]
fn the_skill_examples_cover_the_documented_topics() {
    let names = example_files();
    assert!(
        names.len() >= 10,
        "expected the shipped skill example set to be non-trivial, found {names:?}"
    );

    // 主题 -> 至少要有一份文件名含这个子串的例子
    const EXPECTED_TOPICS: &[(&str, &str)] = &[
        ("truthiness", "truthiness"),
        ("control flow", "control_flow"),
        ("match", "match"),
        ("interpolation", "interpolation"),
        ("error propagation", "error_propagation"),
        ("import + stdlib", "import_stdlib"),
        ("concurrency", "concurrency"),
        ("cancellation", "concurrency_cancel"),
        ("OOP", "oop"),
        ("static contracts", "static_contracts"),
        ("module declaration", "module_decl"),
        ("SEALED surface", "sealed"),
        ("module signature sidecar", "module_signature"),
        ("MATCH exhaustiveness (E0116)", "match_exhaustiveness"),
        ("object identity", "object_identity"),
    ];

    for (topic, needle) in EXPECTED_TOPICS {
        assert!(
            names.iter().any(|n| n.contains(needle)),
            "no skill example covers `{topic}` (expected a file whose name contains \
             `{needle}`). Current set: {names:?}"
        );
    }
}

/// `wlwl.toml` 必须含完整的 `[package]` 三件套。
///
/// 这是 R10-062 的原教训:示例随包发布、清单却缺 `[package]`,静态层**整条
/// 静默失效**(`static_contracts.wll` 于是从没真的跑过检查)。缺一个键就会
/// 退回 v0.10 的静默失败。
#[test]
fn the_skill_examples_manifest_declares_a_complete_package() {
    let text = std::fs::read_to_string(examples_root().join("wlwl.toml"))
        .expect("examples/wlwl.toml must exist — the static-contracts example needs it");
    for key in ["name", "version", "entry"] {
        assert!(
            text.contains(&format!("{key} =")),
            "examples/wlwl.toml is missing `{key}` in [package]; a manifest without \
             all three is not a manifest and the static layer goes quiet (R10-010) \
             rather than reporting:\n{text}"
        );
    }
    assert!(
        text.contains("[features]"),
        "examples/wlwl.toml has no [features] table, so static_contracts.wll is \
         not being checked at all"
    );
}
