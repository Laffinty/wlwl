//! `cargo test -p wlwl-cli --test probe`
//!
//! v0.10.0 动态 REVIEW 用的 97 份 probe 夹具,**整套搬进仓库、进 CI**。
//! 它们的价值只有一个:**防回归** —— 下面这些「与规范文本矛盾、但当前实现
//! 就是这么跑」的行为(`deviation: true`)被逐字钉死在 `expect.json` 里,
//! 下一次改动不会再把它们悄悄换掉。
//!
//! 一份 case 是 `impl/tests/probe/cases/<id>/` 下的一个目录:
//!
//! | 文件          | 含义                                              |
//! |---------------|---------------------------------------------------|
//! | `cmd`         | 可选。exe 之后的 argv,缺省 `run main.wll`          |
//! | `main.wll`    | 被测程序                                          |
//! | `wlwl.toml`   | 可选。清单                                        |
//! | `*.sig`       | 可选。签名伴随文件                                |
//! | `expect.json` | 断言(`exit` / `contains` / `not_contains` /
//! |               | `present` / `absent` / `deviation`)                |
//!
//! **`cmd` 不进暂存区**(它只是驱动脚本用来拼 argv 的),其余文件原样复制。
//!
//! 跑法与 `probe.py` 等价,两处容易踩的地方照抄:
//!
//! 1. **暂存**:整目录复制到 `temp_dir()/wlwl-probe/<id>` 再跑,会写文件的
//!    子命令(`sig-gen`)因此永远不污染仓库里的夹具。
//! 2. **`current_dir` 而不是绝对路径**:argv 里写的是 `main.wll`,`IMPORT("./math")`
//!    这类相对路径要靠 cwd 解析 —— 传绝对路径会挂掉。

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// `cases/` 下应当有的 case 数。
///
/// 这条常量是**元数据守卫**:加了夹具却忘了让它进 CI 覆盖,这里就会红。
///
/// 97 是 v0.10.0 动态 REVIEW 迁进来的原始套件;第 98 条
/// (`M5_features_only_manifest_clean_program`)是 v0.10.1 修完 R10-010
/// 后补的正面用例 —— 原套件只有「坏程序被拦」,没有「好程序照常跑通」。
/// 第 99 条(`P_r10_025_nested_type_constraint`)是修 R10-025 时补的:
/// 嵌套类型约束是**解析错**,static_types 夹具要求解析干净,住不了那里。
/// 最后两条(`P_r10_015_*`)是 R10-015 补的:`YIELD` 的已知限制是
/// **运行期**行为(规范 §17.1 那张表 5 行 5 行与实测相反),同样住不了
/// static_types 夹具。两条都标 `deviation` —— 它们断言的是当前实现的真实
/// 行为;v0.11 实现续体保存时它们会转红,那正是要看的信号。
///
/// 最后 35 条(`P_v10_2_*` / `P_v10_3_*`)是 v0.10.2 第三方合规审查修复批次
/// 与 v0.10.3 健壮性批次补的,一条缺陷至少一条 case,且**每条都配一条反向
/// 守卫**(对照组 / 误报守卫 / 仍须被拒的负例):只锁「修好了」而没有锁
/// 「没改坏别的」,等于把回归放进门禁之外。
///
/// 第 137 / 138 条(`M1_std_*_dual_track`)是 v0.11 标准库底座 M1 补的:
/// R1 语言层(`std.str` / `std.math`)嵌入加载的冒烟用例;开发覆盖
/// (`--std-src` / `WLWL_STD_SRC`)的导出面不变性由
/// `stdlib_dual_track.rs` 锁定。
///
/// 第 139–142 条(`M3_std_*`)是 M3 补的:三个 R1 模块(`std.collection`
/// 纯 wlwl 终态、`std.str` §6、`std.math` §7 混合)与 `std.test` 混合门面的
/// 端到端冒烟,外加 `E0038`(RANGE 步长为零)的存在性 —— 那条诊断在 M3 之前
/// 的唯一发射点是被删掉的 R2 实现,删掉之后只剩 M3-0 立起的注入通道。
///
/// 第 143–144 条(`P_d11_026_*`)是 D11-026 补的:文本输入**不得**因带 BOM 而被
/// 拒绝(工具约定,见 `docs/spec/wlwl-agent-spec-v0.11.md` §2 —— 语言规范的
/// BOM 条款只覆盖 `.wll`)。它们是头两条用 `bom` 键把 BOM 注入暂存副本的用例 ——
/// 驱动据此把不可见字节变成夹具里一个可审的声明。两条合起来覆盖那三类输入
/// 规范所辖的三个文件(源文件 / `wlwl.toml` / `main.wll.sig`);`wlwl.lock` 不在
/// 其中,理由见 §9.4(锁文件在规范外)。
/// 第 155–156 条(`M3_std_sanitize_*`)是 v0.11.3 补的:`std.sanitize` 的
/// 转义对(`HTML_ESCAPE` / `HTML_UNESCAPE`,含往返定理与「未知实体原样」)
/// 与净化器 `HTML_SANITIZE`(raw-text 整删不越界 / 拆壳留内容 / 逐属性
/// 判定 / 幂等 / 相对 URL 恒允许)。净化器那条的 `not_contains` 才是真
/// 断言 —— v0.11.3 收口时 D13-003 与 D13-005 两道「内容静默消失」的
/// 缺陷,正是靠 `contains` 里的 `<b>keep</b>` 与 `y` 钉住的。
// [v0.11.3 M5 / addendum-00 L0-A-2] 157 → 158:新增
// `M5_std_collection_l0a2_r2_scale`(`MAP` / `FILTER` / `CHUNK` / `WINDOW`
// 归 R2 之后才可能存在的大数组规模用例,5 000 元素,整条 346 ms / debug 构建)。
const EXPECTED_CASE_COUNT: usize = 164;

/// 单个 case 的上限,与 `probe.py` 的 `timeout=60` 同义。
const CASE_TIMEOUT: Duration = Duration::from_secs(60);

/// `expect.json` 里驱动认识的键。多出来的键一定是拼错了,而拼错的键**什么
/// 都不断言** —— 那比没有夹具更糟,所以 `every_case_has_expect_json` 会拦。
///
/// **例外**:下划线开头的键(`_why` / `_note`)是纯文档,驱动跳过不读。
/// 这批 case 里有 13 条断言的是「实测行为与规范矛盾」,不写清楚原因,
/// 下一个人只会把它们当成随手写的期望值删掉。JSON 没有注释语法,只能借键。
const KNOWN_KEYS: [&str; 7] = [
    "exit",
    "contains",
    "not_contains",
    "present",
    "absent",
    "deviation",
    "bom",
];

// ---------------------------------------------------------------- 夹具定位

/// `impl/tests/probe/cases`
fn cases_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("CARGO_MANIFEST_DIR should be impl/crates/wlwl-cli")
        .join("tests")
        .join("probe")
        .join("cases")
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

/// `cases/` 下的全部 case id(排序,与 `probe.py` 的 `sorted(...)` 一致)。
fn case_ids() -> Vec<String> {
    let mut ids: Vec<String> = std::fs::read_dir(cases_root())
        .unwrap_or_else(|e| panic!("cannot list {}: {e}", cases_root().display()))
        .map(|entry| entry.expect("case dir entry"))
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    ids.sort();
    ids
}

/// 元数据守卫的共用实现:目录数必须正好是 97,且不能混进别的东西
/// (一个落单的 `README.md` 会让「97」和实际夹具数悄悄对不上)。
fn assert_case_inventory(ids: &[String]) {
    let entries: Vec<_> = std::fs::read_dir(cases_root())
        .expect("cases/ readable")
        .map(|e| {
            e.expect("cases/ entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert_eq!(
        entries.len(),
        ids.len(),
        "cases/ holds entries that are not directories; only case dirs are driven"
    );
    assert_eq!(
        ids.len(),
        EXPECTED_CASE_COUNT,
        "impl/tests/probe/cases must hold exactly {EXPECTED_CASE_COUNT} case dirs \
         (found {}). Add the fixture AND make sure this harness drives it: the \
         runner below iterates case_ids(), so a new dir is picked up for free — \
         but then bump EXPECTED_CASE_COUNT so the count stays honest.",
        ids.len()
    );
}

// ---------------------------------------------------------------- 暂存

/// 把一份 case 整目录复制到 `temp_dir()/wlwl-probe/<id>`(先清后建)。
///
/// `bom_files` 里的每个文件名会在**暂存副本**上前置一个 UTF-8 BOM
/// (见 [`Expect::bom`])。前置只发生在暂存副本上,仓库里的
/// 夹具字节不变 —— 所以 `git diff` 看不到的字节不会藏在夹具里。
fn stage(case_id: &str, bom_files: &[String]) -> PathBuf {
    let dir = std::env::temp_dir().join("wlwl-probe").join(case_id);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("staging dir");
    for entry in std::fs::read_dir(cases_root().join(case_id)).expect("case dir") {
        let entry = entry.expect("case entry");
        let path = entry.path();
        // `cmd` 只用来拼 argv,不是被测输入。
        if entry.file_name() == "cmd" {
            continue;
        }
        let target = dir.join(entry.file_name());
        if path.is_dir() {
            copy_tree(&path, &target);
        } else {
            std::fs::copy(&path, &target).expect("case file staged");
        }
    }
    for name in bom_files {
        let target = dir.join(name);
        assert!(
            target.is_file(),
            "expect.json 的 `bom` 列了 `{name}`,但 case 目录里没有这个文件"
        );
        let body = std::fs::read(&target).expect("staged file readable");
        assert!(
            !body.starts_with(&[0xEF, 0xBB, 0xBF]),
            "`{name}` 夹具已经自带 BOM,会与 `bom` 键重复前置"
        );
        let mut with_bom = vec![0xEFu8, 0xBB, 0xBF];
        with_bom.extend_from_slice(&body);
        std::fs::write(&target, with_bom).expect("BOM written to staged copy");
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
            std::fs::copy(&src, &dst).expect("nested file staged");
        }
    }
}

// ---------------------------------------------------------------- 跑

enum RunEnd {
    /// 正常退出,值是退出码。
    Exited(i32),
    /// 60 秒到点,已 `kill()`。
    TimedOut,
}

struct Capture {
    end: RunEnd,
    /// stdout + stderr 合并(与 `probe.py` 的 `(proc.stdout or "") + (proc.stderr or "")`
    /// 同一口径)。
    out: String,
}

/// 跑一个子命令,60 秒上限。
///
/// 两个坑都在这里解决:
///
/// * **超时**:`Command::output()` 只有「跑完才有输出」,没有超时,而
///   `wait-timeout` 不在依赖里(`Cargo.toml` 不许改)。于是 spawn 之后
///   主线程轮询 `try_wait()`,到点 `kill()` + `wait()` 收尸。
/// * **管道死锁**:光轮询 `try_wait()` 是不够的 —— 子进程往写满的管道里
///   写,父进程在等它退出,谁也不动。所以**stdout / stderr 各起一个读取
///   线程**(`read_to_end`)先把管道读空,主线程只管等退出码。这样既不会
///   卡在满管道上,也不会无限挂起。
fn run_capture(bin: &Path, args: &[String], cwd: &Path) -> Capture {
    let mut child = Command::new(bin)
        .args(args)
        // cwd,不是绝对路径:argv 里的 `main.wll` 与 `IMPORT("./math")` 靠它解析。
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("wlwl spawns");

    let stdout = child.stdout.take().expect("stdout piped");
    let stderr = child.stderr.take().expect("stderr piped");
    let out_reader = std::thread::spawn(move || read_all(stdout));
    let err_reader = std::thread::spawn(move || read_all(stderr));

    let deadline = Instant::now() + CASE_TIMEOUT;
    let end = loop {
        match child.try_wait().expect("try_wait") {
            Some(status) => break RunEnd::Exited(status.code().unwrap_or(-1)),
            None => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    break RunEnd::TimedOut;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    };

    // 进程已经没了(退出或被杀),管道随之关闭,两个读取线程会自行结束。
    let stdout = out_reader.join().unwrap_or_default();
    let stderr = err_reader.join().unwrap_or_default();
    Capture {
        end,
        // `from_utf8_lossy` 对应 probe.py 的 `errors="replace"`。
        out: format!(
            "{}{}",
            String::from_utf8_lossy(&stdout),
            String::from_utf8_lossy(&stderr)
        ),
    }
}

fn read_all<R: std::io::Read>(mut r: R) -> Vec<u8> {
    let mut buf = Vec::new();
    let _ = r.read_to_end(&mut buf);
    buf
}

// ---------------------------------------------------------------- 断言

#[derive(Default)]
struct Expect {
    exit: Option<i32>,
    contains: Vec<String>,
    not_contains: Vec<String>,
    present: Vec<String>,
    absent: Vec<String>,
    /// **仅标记**:不改断言。失败消息里加 `[deviation]` 提示。
    deviation: bool,
    /// 暂存时给这些文件前置一个 UTF-8 BOM。
    ///
    /// 为什么在夹具里做而不是提交一个带 BOM 的源文件:`.gitattributes` 的
    /// `*.wll text eol=lf` 会把提交的 CRLF 规范化,而 BOM 在 code review
    /// 里是**不可见字节** —— 提交它等于提交一个没人看得见的东西。写成
    /// 期望里的一个显式键,这个字节就变成可审、可 diff 的声明。
    bom: Vec<String>,
}

fn load_expect(case_id: &str) -> Expect {
    let path = cases_root().join(case_id).join("expect.json");
    let Ok(text) = std::fs::read_to_string(&path) else {
        // probe.py 同样容忍缺失:此时只要求 rc == 0。
        return Expect::default();
    };
    let value: serde_json::Value = serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("{case_id}/expect.json is not valid JSON: {e}"));
    Expect {
        exit: value.get("exit").and_then(|v| v.as_i64()).map(|v| v as i32),
        contains: strings_of(case_id, &value, "contains"),
        not_contains: strings_of(case_id, &value, "not_contains"),
        present: strings_of(case_id, &value, "present"),
        absent: strings_of(case_id, &value, "absent"),
        deviation: value
            .get("deviation")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        bom: strings_of(case_id, &value, "bom"),
    }
}

fn strings_of(case_id: &str, value: &serde_json::Value, key: &str) -> Vec<String> {
    match value.get(key) {
        None | Some(serde_json::Value::Null) => Vec::new(),
        Some(serde_json::Value::Array(items)) => items
            .iter()
            .map(|item| {
                item.as_str()
                    .unwrap_or_else(|| {
                        panic!("{case_id}/expect.json: `{key}` must hold only strings")
                    })
                    .to_string()
            })
            .collect(),
        Some(other) => panic!("{case_id}/expect.json: `{key}` must be an array, got {other}"),
    }
}

struct CaseOutcome {
    id: String,
    cmd: String,
    deviation: bool,
    failures: Vec<String>,
    /// 失败时带出去的输出前几行,便于定位。
    output_head: Vec<String>,
}

/// 一份 case 的完整判定。语义与 `probe.py` 的 `run_case()` 等价。
fn run_case(id: &str) -> CaseOutcome {
    let case = cases_root().join(id);

    // argv:`cmd` 文件按空白切分,缺省 `run main.wll`。
    let cmd_file = case.join("cmd");
    let tail = match std::fs::read_to_string(&cmd_file) {
        Ok(text) => text,
        Err(_) => "run main.wll".to_string(),
    };
    let argv: Vec<String> = tail.split_whitespace().map(str::to_string).collect();

    let expect = load_expect(id);
    let staged = stage(id, &expect.bom);
    let capture = run_capture(&wlwl_binary(), &argv, &staged);

    let code = match capture.end {
        // probe.py 超时也只报这一条,不再看其它断言。
        RunEnd::TimedOut => {
            let _ = std::fs::remove_dir_all(&staged);
            return CaseOutcome {
                id: id.to_string(),
                cmd: tail.trim().to_string(),
                deviation: expect.deviation,
                failures: vec!["TIMEOUT".to_string()],
                output_head: Vec::new(),
            };
        }
        RunEnd::Exited(code) => code,
    };

    let mut failures = Vec::new();

    if let Some(want) = expect.exit {
        if code != want {
            failures.push(format!("exit={code} want={want}"));
        }
    }
    for needle in &expect.contains {
        if !capture.out.contains(needle.as_str()) {
            failures.push(format!("missing:{needle:?}"));
        }
    }
    for needle in &expect.not_contains {
        if capture.out.contains(needle.as_str()) {
            failures.push(format!("unexpected:{needle:?}"));
        }
    }
    for needle in &expect.present {
        if !staged.join(needle).exists() {
            failures.push(format!("file-missing:{needle}"));
        }
    }
    for needle in &expect.absent {
        if staged.join(needle).exists() {
            failures.push(format!("file-should-not-exist:{needle}"));
        }
    }
    // 暂存区最后才清:`present` / `absent` 查的就是它。
    let _ = std::fs::remove_dir_all(&staged);

    let output_head = capture
        .out
        .lines()
        .filter(|l| !l.trim().is_empty())
        .take(4)
        .map(str::to_string)
        .collect();
    CaseOutcome {
        id: id.to_string(),
        cmd: tail.trim().to_string(),
        deviation: expect.deviation,
        failures,
        output_head,
    }
}

/// 失败行的统一格式:`<id>: [deviation] <原因>`。
///
/// `[deviation]` 是**提示而不是判据** —— 这条 case 断言的正是当前实现的真实
/// 行为(与规范文本矛盾),它红了一样是真回归,只是「回归」的对象是那个
/// 已知偏差。
fn failure_line(outcome: &CaseOutcome, reason: &str) -> String {
    if outcome.deviation {
        format!("{}: [deviation] {reason}", outcome.id)
    } else {
        format!("{}: {reason}", outcome.id)
    }
}

// ---------------------------------------------------------------- 测试

/// 一个 `#[test]` 跑完全部 97 份 case,把失败**一次性**报出来。
///
/// 故意不 `assert_eq!` 到第一个失败就停:一次 `--nocapture` 就能看到全部
/// 红的 case,而不是修一个跑一次。
#[test]
fn probe_suite() {
    let ids = case_ids();
    assert_case_inventory(&ids);

    let mut ran = 0usize;
    let mut failed = Vec::new();
    let mut failed_cases = 0usize;
    let mut deviation = 0usize;
    for id in &ids {
        ran += 1;
        let outcome = run_case(id);
        if outcome.failures.is_empty() {
            if outcome.deviation {
                deviation += 1;
                println!("[DEVIATION] {:<42} $ wlwl {}", outcome.id, outcome.cmd);
            } else {
                println!("[PASS]      {:<42} $ wlwl {}", outcome.id, outcome.cmd);
            }
        } else {
            failed_cases += 1;
            println!("[FAIL]      {:<42} $ wlwl {}", outcome.id, outcome.cmd);
            for reason in &outcome.failures {
                let line = failure_line(&outcome, reason);
                println!("        {line}");
                failed.push(line);
            }
            for line in &outcome.output_head {
                println!("        | {line}");
            }
        }
    }

    // 计数口径与 `probe.py` 相同:一份 case 至多算一次。
    println!(
        "\nSUMMARY pass={} deviation={} fail={} total={}",
        ran - failed_cases - deviation,
        deviation,
        failed_cases,
        ran
    );
    assert_eq!(
        ran,
        ids.len(),
        "the loop above must drive every listed case"
    );
    assert!(
        failed.is_empty(),
        "{} of {} probe cases failed:\n  {}",
        failed_cases,
        ran,
        failed.join("\n  ")
    );
}

/// 元数据守卫 1:`cases/` 必须正好 97 份夹具。
///
/// 改了夹具数却没让 CI 覆盖新夹具,是这套套件唯一的失守方式,所以钉死。
#[test]
fn probe_case_count_matches_inventory() {
    assert_case_inventory(&case_ids());
}

/// 元数据守卫 2:每份 case 都有可解析的 `expect.json`,且键都在驱动认识的范围里。
///
/// 一份断言写坏(键名拼错、`JSON` 语法错)的夹具不会**红**,只会**悄悄什么都不
/// 断言** —— 那比没有夹具更糟。
#[test]
fn every_case_has_expect_json() {
    let mut problems = Vec::new();
    for id in case_ids() {
        let path = cases_root().join(&id).join("expect.json");
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) => {
                problems.push(format!("{id}: expect.json unreadable ({e})"));
                continue;
            }
        };
        let value: serde_json::Value = match serde_json::from_str(&text) {
            Ok(value) => value,
            Err(e) => {
                problems.push(format!("{id}: expect.json is not valid JSON ({e})"));
                continue;
            }
        };
        let Some(object) = value.as_object() else {
            problems.push(format!("{id}: expect.json is not a JSON object"));
            continue;
        };
        for key in object.keys() {
            if key.starts_with('_') {
                continue; // 文档键,见 KNOWN_KEYS 的说明。
            }
            if !KNOWN_KEYS.contains(&key.as_str()) {
                problems.push(format!(
                    "{id}: expect.json has unknown key `{key}` — the driver would \
                     assert nothing about it"
                ));
            }
        }
    }
    assert!(
        problems.is_empty(),
        "{} probe case(s) have an unusable expect.json:\n  {}",
        problems.len(),
        problems.join("\n  ")
    );
}
