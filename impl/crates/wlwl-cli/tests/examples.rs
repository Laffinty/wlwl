//! [v0.10.1 / R10-070] 随包示例的冒烟测试。
//!
//! 之前 `impl/examples/` **不在任何测试的覆盖面里** —— CI 跑
//! `cargo test --all-targets`,而没有任何一条测试跑 `examples/*.wll`。
//! 后果是两个示例从 v0.6 起就一直坏着,没人发现:
//!
//! | 文件 | 坏了多久 | 症状 |
//! |---|---|---|
//! | `showcase.wll` | v0.6 → v0.10 | `E0020: undefined name 'true'`(小写布尔字面量) |
//! | `std_test.wll` | v0.4 → v0.10 | `E1003: division by zero`(`EXPECT_ERR` 捕原生码) |
//!
//! 两者都随包发布。示例的价值全在于「读者能直接跑通」,一个跑不起来的示例
//! 比没有示例更糟 —— 它教的是错误的知识。
//!
//! ## 暂存而不是就地跑
//!
//! 每份示例先复制到独立临时目录再执行。`showcase.wll` 会 `WRITE_FILE`
//! 写一个 JSON 产物,就地跑会把文件落在仓库里。暂存让仓库保持干净,
//! 也让示例的相对路径行为与用户在自己项目里跑时一致。

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

/// `impl/examples/`
fn examples_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("CARGO_MANIFEST_DIR should be impl/crates/wlwl-cli")
        .join("examples")
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

/// 示例逐个跑的上限。示例都是几十行的直线程序,1 秒绰绰有余;给足余量是为了
/// 不让慢机器上的偶发调度延迟变成红灯。
const EXAMPLE_TIMEOUT: Duration = Duration::from_secs(60);

/// 列出全部示例(按名字排序,失败信息才可复现)。
fn example_files() -> Vec<String> {
    let root = examples_root();
    let mut names: Vec<String> = std::fs::read_dir(&root)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", root.display()))
        .filter_map(|e| e.ok())
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

/// 把整份示例目录复制到独立临时目录,返回运行目录。
///
/// **复制整个目录而不是单个文件**:`phase2_demo.wll` 会 `IMPORT("math")`
/// 去加载同目录的 `math.wll`,而「跨目录模块」正是 showcase 明确要演示的
/// 特性之一。只复制入口文件会把这条能力本身从测试里抹掉 —— 那不是让测试
/// 更严,那是让测试测不到它声称要测的东西。
fn stage(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("wlwl-examples").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("staging dir");
    let root = examples_root();
    for sibling in example_files() {
        std::fs::copy(root.join(&sibling), dir.join(&sibling)).expect("example copied");
    }
    dir
}

/// 跑一份示例,返回 `(是否成功, 输出, 是否超时)`。
fn run_example(bin: &Path, name: &str) -> (bool, String, bool) {
    let dir = stage(name);
    let mut child = Command::new(bin)
        .arg("run")
        .arg(name)
        // cwd 而非绝对路径:示例里的相对 `IMPORT` 靠它解析。
        .current_dir(&dir)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("wlwl spawns");

    // 管道读空线程 + 主线程轮询,和 `probe.rs` 同一套:只轮询会在管道写满时死锁。
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
fn every_shipped_example_runs_clean() {
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
        "{} of {} shipped example(s) fail to run:\n  {}",
        failures.len(),
        example_files().len(),
        failures.join("\n  ")
    );
}

/// **元数据守卫**:示例目录不能悄悄变空。
///
/// 没有这条,有人把示例全删了会让上面那条「全绿」变成一句空话。
#[test]
fn the_examples_directory_is_not_empty() {
    let names = example_files();
    assert!(
        names.len() >= 2,
        "expected the shipped example set to be non-trivial, found {names:?}"
    );
}

/// 两个曾经坏掉的文件各有专门一条断言,把「为什么当初会坏」钉在测试里。
///
/// 只有「跑通了」是不够的 —— 修完之后没人记得 `true` 为什么不行、`EXPECT_ERR`
/// 为什么捕不到除零。写下来,下一个改示例的人不用重新踩一遍。
#[test]
fn the_two_examples_that_used_to_be_broken_stay_fixed() {
    let bin = wlwl_binary();

    // showcase.wll:曾用小写 `true`(E0020),曾给 STRINGIFY 传两个参数(E0022),
    // 曾往一个不存在的 `build/` 目录写文件(E0061)。三处都是 v0.4 之后没收掉的。
    let (ok, out, _) = run_example(&bin, "showcase.wll");
    assert!(ok, "showcase.wll must run:\n{out}");
    assert!(
        out.contains("round-trip"),
        "showcase.wll must still demonstrate the fs+json round-trip:\n{out}"
    );

    // std_test.wll:曾写 `EXPECT_ERR(/(1, 0))`,指望它捕除零。除零是**原生码**
    // E1003 而不是 ERR 值,任何 catch 都来不及 —— 整份文件直接挂。
    let (ok, out, _) = run_example(&bin, "std_test.wll");
    assert!(ok, "std_test.wll must run:\n{out}");
    assert!(
        out.contains("TRUE TRUE TRUE"),
        "std_test.wll must still demonstrate ASSERT / ASSERT_EQ / EXPECT_ERR:\n{out}"
    );
}
