// [Phase H1] spec v0.4 section 16.5 conformance suite driver.
//
// Each `impl/tests/conformance/*.wll` file is the canonical fixture
// for one of the section 16.5 mandatory test categories. The harness
// shells out to the workspace-built `wlwl run` binary and asserts:
//   (a) the binary exists (built)
//   (b) exit status in {0, 1} (0 = happy, 1 = well-formed ERR path)
//   (c) JSONL output (if any) is one line per error and well-formed
//   (d) every JSONL field from schema 1.1.0 is present
//
// We use the binary built by the workspace test harness
// (`CARGO_BIN_EXE_wlwl`); if the env var is missing, we look
// for `target/debug/wlwl` and `target/release/wlwl` as a fallback.
//
// Fixture location: `impl/tests/conformance/` (workspace-level, not
// crate-local), so the fixtures are shared across all crates in the
// workspace and survive refactors of `wlwl-cli` itself.
//
// Deviations:
//   P4-H1-001  fixtures are spec v0.4 target snapshots; v0.4 release
//              implementation only partially covers section 16.5, so
//              fixtures exercise the *observable* surface and emit
//              well-formed ERRs as a substitute for as-yet-unimplemented
//              features.
//   P4-H1-002  harness accepts exit code 1 with well-formed JSONL as
//              a passing outcome; a future v0.4.x patch release will
//              tighten this back to exit 0 (happy path) per plan
//              section 1318-1330.
//   P4-H1-003  schema 1.1.0 mandatory field set is 12 (spec section
//              14.2 lists 13, but `cause` is deferred to v0.4.1 patch
//              per wlwl-error/src/lib.rs line 579, A1d/A1e). v0.4.0
//              emitter writes `cause: None` and serde skips the key.

use std::path::{Path, PathBuf};
use std::process::Command;

fn fixture_root() -> PathBuf {
    // CARGO_MANIFEST_DIR = impl/crates/wlwl-cli
    //   parent.parent -> impl/
    let here = Path::new(env!("CARGO_MANIFEST_DIR"));
    here.parent()
        .and_then(|p| p.parent())
        .expect("CARGO_MANIFEST_DIR should be impl/crates/wlwl-cli")
        .join("tests")
        .join("conformance")
}

fn wlwl_binary() -> PathBuf {
    if let Ok(p) = std::env::var("CARGO_BIN_EXE_wlwl") {
        return PathBuf::from(p);
    }
    let here = Path::new(env!("CARGO_MANIFEST_DIR"));
    for profile in ["debug", "release"] {
        let candidate =
            here.join("target")
                .join(profile)
                .join(if cfg!(windows) { "wlwl.exe" } else { "wlwl" });
        if candidate.exists() {
            return candidate;
        }
    }
    panic!("wlwl binary not found -- run `cargo build --bin wlwl` first");
}

fn fixture(name: &str) -> PathBuf {
    fixture_root().join(name)
}

const WLT_FILES: &[&str] = &[
    "core_subsets.wll",
    "err_propagation.wll",
    "index_bounds.wll",
    "numeric.wll",
    "match_patterns.wll",
    "destruct.wll",
    "closure_cell.wll",
    "module_paths.wll",
    "format_template.wll",
    "error_schema.wll",
    "stdlib_r1_collection.wll",
    "stdlib_r1_str.wll",
    "stdlib_r1_math.wll",
];

/// [v0.11 M3-4] stdlib R1 模块的**符合性自测脚本**。
///
/// 与上面 10 个「规范 §16.5 类别覆盖」脚本性质不同:那十个是「跑起来不
/// 崩就算过」(`all_conformance_fixtures_run_or_emit_error` 接受 exit 0
/// **或** exit 1 + 合法 JSONL)。这三个是**自测**:脚本内部用 `std.test`
/// 的 `ASSERT_EQ` 逐条断言成员行为,失败即 `PANIC`。
///
/// 所以它们**不能**享受上面那条宽松条款 —— 一个失败的 R1 自测恰好就是
/// 「exit 1 + 合法 JSONL」,会被 `all_conformance_fixtures_run_or_emit_error`
/// 当成通过。必须单独要求 exit 0,否则这套自测就是自欺欺人。
const R1_SELF_TEST_FILES: &[&str] = &[
    "stdlib_r1_collection.wll",
    "stdlib_r1_str.wll",
    "stdlib_r1_math.wll",
];

// P4-H1-003: v0.4.0 emitter writes 12 schema-1.1.0 fields; `cause` is
// deferred to v0.4.1 (see wlwl-error/src/lib.rs line 579).
const SCHEMA_110_FIELDS: &[&str] = &[
    "code",
    "error_category",
    "error_schema_version",
    "message",
    "location",
    "severity",
    "retryable",
    "retry_after",
    "related",
    "suggestion_code",
    "idempotent",
    "trace",
];

#[test]
fn all_conformance_fixtures_present() {
    // Spec section 16.5 mandates each of these 10 fixtures. The list
    // itself is the constraint; cargo test will fail if any
    // `impl/tests/conformance/*.wll` is removed.
    assert_eq!(WLT_FILES.len(), 13);
    for f in WLT_FILES {
        let p = fixture(f);
        assert!(p.exists(), "missing conformance fixture: {}", p.display());
    }
}

/// [v0.11 M3-4] R1 模块自测脚本必须**退出码 0**。
///
/// 这三个脚本用 `std.test` 逐条断言成员行为,失败即 `PANIC`;若某条断言
/// 不成立,进程退出码是 1 且 stderr 是合法的 JSONL 诊断 —— 正好落在
/// `all_conformance_fixtures_run_or_emit_error` 接受的「exit 1 + 合法
/// JSONL」分支里。所以那一条宽松测试**不能**用来判它们,必须有这条要求
/// exit 0 的独立门禁。
#[test]
fn r1_stdlib_self_tests_pass() {
    assert_eq!(
        R1_SELF_TEST_FILES.len(),
        3,
        "collection / str / math 三个 R1 模块各一份自测脚本"
    );
    for f in R1_SELF_TEST_FILES {
        let path = fixture(f);
        assert!(
            path.exists(),
            "missing R1 self-test fixture: {}",
            path.display()
        );
        let out = run_wlwl(&path);
        let code = out.status.code().unwrap_or(-1);
        let stdout = String::from_utf8_lossy(&out.stdout);
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert_eq!(
            code, 0,
            "R1 self-test `{f}` failed (exit {code}):\n  stdout={stdout}\n  stderr={stderr}"
        );
    }
}

/// 反向守卫:`r1_stdlib_self_tests_pass` 的输入是 `R1_SELF_TEST_FILES`。
/// 它被清空时上面那条会** vacuously 通过** —— 而这恰恰是最需要它报警的
/// 情况(自测脚本被误删)。所以名单必须恰好覆盖三个 R1 模块且都在
/// `WLT_FILES` 里。
#[test]
fn r1_self_test_list_is_not_vacuously_satisfied() {
    for f in R1_SELF_TEST_FILES {
        assert!(
            WLT_FILES.contains(f),
            "`{f}` must also be in WLT_FILES so the fixture-presence test covers it"
        );
    }
}

fn run_wlwl(path: &Path) -> std::process::Output {
    let bin = wlwl_binary();
    Command::new(&bin)
        .arg("run")
        .arg(path)
        .arg("--format=jsonl")
        .output()
        .unwrap_or_else(|e| panic!("failed to spawn `wlwl run {}`: {}", path.display(), e))
}

#[test]
fn all_conformance_fixtures_run_or_emit_error() {
    // Per P4-H1-002, a fixture passes if:
    //   (i)  exit code is 0 (happy path), OR
    //   (ii) exit code is 1 AND at least one stderr line is well-formed
    //        JSON with all schema 1.1.0 mandatory fields.
    //
    // This is the release-readiness probe: every category is exercised,
    // and unimplemented v0.4 features surface as ERRs rather than
    // silent passes.
    for f in WLT_FILES {
        let path = fixture(f);
        let out = run_wlwl(&path);
        let code = out.status.code().unwrap_or(-1);
        let stderr = String::from_utf8_lossy(&out.stderr);
        let stdout = String::from_utf8_lossy(&out.stdout);
        if code == 0 {
            continue;
        }
        // exit code 1: scan stderr (and stdout) for any well-formed JSONL line.
        let mut found = false;
        for stream in stderr.lines().chain(stdout.lines()) {
            let line = stream.trim();
            if line.is_empty() {
                continue;
            }
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
                let ok = SCHEMA_110_FIELDS.iter().all(|k| v.get(*k).is_some());
                if ok {
                    found = true;
                    break;
                }
            }
        }
        assert!(found,
            "`{}` failed without well-formed JSONL (exit {code}):\n  stdout={stdout}\n  stderr={stderr}",
            path.display());
    }
}

#[test]
fn error_schema_jsonl_is_one_line_per_error() {
    // Per spec section 16.5 #10 -- JSONL output must be one line per
    // error and stable across runs. error_schema.wll emits >= 2 ERRs.
    let path = fixture("error_schema.wll");
    let out = run_wlwl(&path);
    let stderr = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let mut n_lines = 0;
    for line in stderr.lines().chain(stdout.lines()) {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        // Only count well-formed JSONL lines.
        if serde_json::from_str::<serde_json::Value>(line).is_err() {
            continue;
        }
        let v: serde_json::Value =
            serde_json::from_str(line).unwrap_or_else(|e| panic!("bad JSONL line `{line}`: {e}"));
        n_lines += 1;
        // Schema 1.1.0 mandatory fields per section 14.2 (minus `cause`,
        // see P4-H1-003)
        for k in SCHEMA_110_FIELDS {
            assert!(
                v.get(*k).is_some(),
                "JSONL line missing field `{k}`: {line}"
            );
        }
    }
    assert!(
        n_lines >= 1,
        "expected >= 1 JSONL error line, got {n_lines}"
    );
}

/// [v0.10.1 / R10-071] `oop_identity.wll` 的逐行期望输出。
///
/// 规范 §2.4(v0.9 起规范性)「`CLASS`/`INSTANCE` 按对象身份恒等」。
const EXPECTED_OOP_IDENTITY: &[(&str, &str)] = &[
    ("self:", "TRUE"),
    ("alias:", "TRUE"),
    ("two NEW:", "FALSE"),
    ("class self:", "TRUE"),
    ("two CLASS:", "FALSE"),
    ("dict alias:", "hit"),
    ("dict other:", "miss"),
    ("contains alias:", "TRUE"),
    ("contains other:", "FALSE"),
];

/// R10-012 的 conformance 夹具:断言**实际输出**,不只是退出码。
///
/// 为什么这条必须单独写而不并进上面的 `all_conformance_fixtures_run_or_emit_error`:
/// 那个测试只验「退出码 0 或报规范错误」。而 R10-012 的 bug 恰恰是
/// `==(a, a)` 恒 FALSE —— **退出码一直是 0**,程序看起来完全正常。
/// 一条只验「跑得通」的断言永远抓不到它。
///
/// 所以这里逐行比对 `label: 结果`,任何一条身份规则退化都会立刻红。
#[test]
fn oop_identity_matches_the_normative_rule() {
    let path = fixture("oop_identity.wll");
    let out = run_wlwl(&path);
    assert_eq!(
        out.status.code(),
        Some(0),
        "oop_identity.wll must run clean:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);

    for (label, expected) in EXPECTED_OOP_IDENTITY {
        let wanted = format!("{label} {expected}");
        assert!(
            stdout.lines().any(|l| l.trim() == wanted),
            "spec §2.4 identity rule violated: expected a line `{wanted}`, got:\n{stdout}"
        );
    }
    // 行数也要对上:少印说明有分支没走到,多印说明夹具改了但期望没改。
    let printed = stdout.lines().filter(|l| !l.trim().is_empty()).count();
    assert_eq!(
        printed,
        EXPECTED_OOP_IDENTITY.len(),
        "oop_identity.wll printed {printed} line(s), expected {} — \
         a new assertion was added to the fixture but not to \
         EXPECTED_OOP_IDENTITY:\n{stdout}",
        EXPECTED_OOP_IDENTITY.len()
    );
}

// (spec_sha_anchored removed: the SHA-1-in-filename convention was
// dropped along with the rename to `wlwl-spec-v0.6.md` -- see
// CHANGELOG "Note on the spec filename". The plain filename is now
// the version identifier; git log -p --follow is the content history.)
