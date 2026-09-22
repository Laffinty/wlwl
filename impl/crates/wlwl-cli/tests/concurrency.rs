// [v0.7 Phase B5a-3 Path B] concurrency conformance suite driver.
//
// Each `impl/tests/concurrency/*.wll` file is a v0.7 WIP conformance
// fixture for one of the plan §6.2 concurrency categories. The
// harness shells out to the workspace-built `wlwl run` binary and
// asserts the exit code is 0 (happy path) and that stdout contains
// the expected deterministic text.
//
// The fixture list itself is the constraint: removing a `.wll`
// file without updating WLT_FILES makes the test fail. This is the
// same contract as the spec v0.4 §16.5 conformance driver in
// `conformance.rs`.
//
// Deviations:
//   P7-B5a3-001  Path B chosen 2026-09-22 as the mid-body suspend
//                strategy (over Path A's full stack-machine rewrite
//                of `eval_expr` and Path C's re-run + yield-counter
//                hack). See `docs/plan/deviations.md` for rationale.

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
        .join("concurrency")
}

fn wlwl_binary() -> PathBuf {
    if let Ok(p) = std::env::var("CARGO_BIN_EXE_wlwl") {
        return PathBuf::from(p);
    }
    let here = Path::new(env!("CARGO_MANIFEST_DIR"));
    for profile in ["debug", "release"] {
        let candidate = here
            .join("target")
            .join(profile)
            .join(if cfg!(windows) { "wlwl.exe" } else { "wlwl" });
        if candidate.exists() {
            return candidate;
        }
    }
    panic!("wlwl binary not found -- run `cargo build -p wlwl-cli --bin wlwl` first");
}

fn fixture(name: &str) -> PathBuf {
    fixture_root().join(name)
}

const WLT_FILES: &[&str] = &[
    // Phase B5a-3 Path B: mid-body YIELD actually suspends the
    // task body, the scheduler runs a sibling between the
    // pre-yield and post-yield segments, and the post-yield
    // mutation is observable.
    "yield_midbody.wll",
];

#[test]
fn all_concurrency_fixtures_present() {
    assert!(!WLT_FILES.is_empty(), "spec says at least one concurrent fixture");
    for f in WLT_FILES {
        let p = fixture(f);
        assert!(p.exists(), "missing concurrency fixture: {}", p.display());
    }
}

fn run_wlwl(path: &Path) -> std::process::Output {
    let bin = wlwl_binary();
    Command::new(&bin)
        .arg("run")
        .arg(path)
        .output()
        .unwrap_or_else(|e| panic!("failed to spawn `wlwl run {}`: {}", path.display(), e))
}

/// Phase B5a-3 Path B: the body that yields once, then mutates a
/// shared cell, must produce the deterministic post-yield value (42)
/// after the scheduler has interleaved the other task between the
/// two segments. If mid-body YIELD were a no-op (the v0.6
/// workaround), the test would either hang on the AWAIT or emit
/// a different non-deterministic ordering.
#[test]
fn yield_midbody_runs_to_completion_with_expected_stdout() {
    let path = fixture("yield_midbody.wll");
    let out = run_wlwl(&path);
    let code = out.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    assert_eq!(code, 0, "expected exit 0, got {code}:\n  stderr={stderr}");
    let expected = "before-yield\nafter-resume\n42\ndone\n";
    assert_eq!(
        stdout, expected,
        "stdout mismatch:\n  got:      {stdout:?}\n  expected: {expected:?}"
    );
}