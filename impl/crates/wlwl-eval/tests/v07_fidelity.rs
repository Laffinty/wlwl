//! [v0.7 Phase B3] v0.6 conformance fidelity baseline.
//!
//! Runs every v0.6 conformance fixture through `wlwl run` and
//! captures a deterministic golden record per fixture:
//! - exit code
//! - stdout (UTF-8)
//! - stderr (UTF-8)
//!
//! The golden file lives next to this test in
//! `fixtures/v07_fidelity_v06_baseline.jsonl` and is checked in.
//!
//! **Until further Phase B commits land, the golden must remain
//! byte-for-byte stable** -- any drift indicates that something
//! about v0.6 behaviour moved. Subsequent B5 / C / D commits that
//! intentionally drift the golden must:
//!   1. update the JSONL to the new output
//!   2. explain the drift in the commit message
//!   3. link to the deviation entry that authorises the change
//!
//! This test does NOT depend on `wlwl-cli`'s integration harness
//! (which already runs its own subset of these fixtures). It runs
//! the full 10-fixture set from the wlwl-eval crate's test context
//! so a future `cargo test -p wlwl-eval` reports any v0.7 drift
//! even when the user only changed eval-side code.

use std::path::{Path, PathBuf};
use std::process::Command;

/// All v0.6 conformance fixtures (mirrors the list in
/// `wlwl-cli/tests/conformance.rs::WLT_FILES`).
const FIXTURES: &[&str] = &[
    "closure_cell.wll",
    "core_subsets.wll",
    "destruct.wll",
    "error_schema.wll",
    "err_propagation.wll",
    "format_template.wll",
    "index_bounds.wll",
    "match_patterns.wll",
    "module_paths.wll",
    "numeric.wll",
];

fn fixture_root() -> PathBuf {
    // CARGO_MANIFEST_DIR = impl/crates/wlwl-eval
    // parent.parent      = impl/
    // /tests             = impl/tests
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("CARGO_MANIFEST_DIR should be impl/crates/wlwl-eval")
        .join("tests")
}

fn fixture(name: &str) -> PathBuf {
    fixture_root().join("conformance").join(name)
}

fn golden_path() -> PathBuf {
    // Golden lives next to the test source (test fixtures are
    // bundled with the crate, not in workspace-level tests/).
    //
    // [v0.9 Step 13 / plan §9.1] baseline filename bumped from
    // `v07_fidelity_v06_baseline` to `v07_fidelity_v09_baseline`
    // because the v0.9 cycle is the first release after the v0.7
    // baseline was captured (v0.8.0 / v0.8.1 did not regenerate).
    //
    // **Content equality**: during the wip0.9 cycle (commits
    // `5997c93` Step 10 / `a2233ff` Step 11 / `dbf849d` Step 11
    // sub-phase 2 / `1ede6bb` Step 12 sub-phase 1 / `cc0b4dc`
    // deviations-v0.9) no v0.6 conformance path was altered —
    // the v0.9 file is byte-equivalent to the v0.6 file (verified
    // via `v07_fidelity_matches_v06_baseline` passing pre-rename
    // AND `v07_fidelity_matches_v09_baseline` passing post-rename).
    // When v0.9.0 impl lands (Step 12 sub-phase 2 — see D9-001) the
    // golden may need blessing again via `--ignored`
    // `v07_fidelity_bless_golden`.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("v07_fidelity_v09_baseline.jsonl")
}

fn wlwl_binary() -> PathBuf {
    // Mirror the fallback used in wlwl-cli/tests/conformance.rs.
    // CARGO_BIN_EXE_wlwl is not set for wlwl-eval tests, so always
    // fall back to the target directory.
    let here = Path::new(env!("CARGO_MANIFEST_DIR"));
    for profile in ["debug", "release"] {
        let candidate = here
            .join("..")
            .join("..")
            .join("target")
            .join(profile)
            .join(if cfg!(windows) { "wlwl.exe" } else { "wlwl" });
        if candidate.exists() {
            return candidate;
        }
    }
    panic!(
        "wlwl binary not found -- run `cargo build -p wlwl-cli --bin wlwl` \
         first (looked under impl/target/{{debug,release}}/)"
    );
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct FixtureRecord {
    /// Fixture filename relative to `impl/tests/conformance/`.
    fixture: String,
    /// Process exit code (None if terminated by signal).
    exit_code: Option<i32>,
    /// Captured stdout as UTF-8 (lossy on invalid UTF-8).
    stdout: String,
    /// Captured stderr as UTF-8 (lossy on invalid UTF-8).
    stderr: String,
}

/// Run each fixture through `wlwl run <fixture>` and capture the
/// deterministic record. Used by both the test (to compare against
/// golden) and by the `--bless` mode (to regenerate the golden).
fn capture_all() -> Vec<FixtureRecord> {
    let bin = wlwl_binary();
    let mut out = Vec::with_capacity(FIXTURES.len());
    for f in FIXTURES {
        let path = fixture(f);
        assert!(
            path.exists(),
            "missing conformance fixture: {}",
            path.display()
        );
        let output = Command::new(&bin)
            .arg("run")
            .arg(&path)
            .output()
            .unwrap_or_else(|e| panic!("failed to spawn `wlwl run {}`: {}", path.display(), e));
        let stderr = normalize_paths(&String::from_utf8_lossy(&output.stderr));
        out.push(FixtureRecord {
            fixture: f.to_string(),
            exit_code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr,
        });
    }
    out
}

/// Strip the absolute project-root path from diagnostic strings so
/// the golden file is portable across CI hosts (Linux runners see
/// `/home/runner/...`, Windows hosts see `D:\...`). The `wlwl`
/// binary embeds the project root in error messages like
/// `error[E0040]: ... outside project root (<PATH>)`; replacing
/// `<PATH>` with a stable placeholder makes the golden independent
/// of where the test was captured. Both the OS-native path and its
/// JSON-escaped form are replaced (the latter because stderr is
/// serialised via serde_json before being compared). After the
/// absolute-path substitution we also canonicalise the remaining
/// path separator (Windows uses `\\` in the JSON-escaped form for
/// `\`, Linux uses `/`) so that the *relative* portion of the
/// diagnostic — `<ROOT>\conformance` vs `<ROOT>/conformance` —
/// also matches.
fn normalize_paths(s: &str) -> String {
    let root = fixture_root(); // impl/tests
    let native = root.display().to_string();
    let escaped = native.replace('\\', "\\\\");
    s.replace(&escaped, "<PROJECT_ROOT>")
        .replace(&native, "<PROJECT_ROOT>")
        .replace('\\', "/")
}

fn write_golden(records: &[FixtureRecord]) {
    let path = golden_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create fixtures dir");
    }
    // One JSON record per line (true JSONL) so any drift surfaces
    // as a clean per-fixture diff.
    let mut buf = String::new();
    for r in records {
        // Canonical serde_json output: no extra whitespace, one
        // record per line, trailing newline.
        buf.push_str(&serde_json::to_string(r).expect("serialize"));
        buf.push('\n');
    }
    std::fs::write(&path, buf).expect("write golden");
}

fn read_golden() -> Vec<FixtureRecord> {
    let path = golden_path();
    let text = std::fs::read_to_string(&path).expect("read golden");
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str::<FixtureRecord>(l).expect("parse golden line"))
        .collect()
}

#[test]
fn v07_fidelity_matches_v09_baseline() {
    // [v0.9 Step 13 / plan §9.1] baseline bumped to v09. Function
    // is renamed (was `v07_fidelity_matches_v06_baseline`) so the
    // log-filter invocation `cargo test v07_fidelity_matches_v09_baseline`
    // lands on the new golden. Old name retained as an alias
    // below for backwards-compatibility with any CI infra that
    // matches on the exact symbol name.
    //
    // First run: if the golden doesn't exist yet, write it and
    // return OK. This makes the test self-bootstrapping on first
    // checkout.
    if !golden_path().exists() {
        let records = capture_all();
        write_golden(&records);
        eprintln!(
            "v07_fidelity: bootstrapped golden at {}",
            golden_path().display()
        );
        return;
    }

    let records = capture_all();
    let golden = read_golden();

    assert_eq!(
        records.len(),
        golden.len(),
        "fixture count drift: {} new vs {} golden",
        records.len(),
        golden.len()
    );

    // Per-fixture byte-stable compare. We compare JSON strings of
    // the records themselves so that reformatting / field-order
    // changes in either side don't cause spurious diffs.
    for (got, exp) in records.iter().zip(golden.iter()) {
        assert_eq!(got.fixture, exp.fixture, "fixture list order drift");
        let got_json = serde_json::to_string(got).expect("re-serialize got");
        let exp_json = serde_json::to_string(exp).expect("re-serialize exp");
        assert_eq!(
            got_json, exp_json,
            "fidelity drift in fixture `{}`\n  got:  {}\n  exp:  {}",
            got.fixture, got_json, exp_json
        );
    }
}

// Backwards-compatible alias: the v0.6 → v0.9 baseline rename
// (D9-001 follow-up) is functionally a no-op on the underlying
// fixture outputs (wip0.9 did not alter any v0.6 conformance
// path), so the legacy `v07_fidelity_matches_v06_baseline`
// symbol is preserved as a thin wrapper around the v09 path.
// New test runs use the v09 name; CI scripts that grep for
// either name succeed.
#[test]
fn v07_fidelity_matches_v06_baseline() {
    v07_fidelity_matches_v09_baseline()
}

/// Regenerate the golden from the current v0.6 behaviour. Run
/// with `cargo test -p wlwl-eval --test v07_fidelity -- --ignored`
/// (the `#[ignore]` attribute makes cargo skip this by default;
/// the user must opt in explicitly so an accidental golden
/// rewrite never happens during routine development).
#[test]
#[ignore = "regenerate golden; run with --ignored"]
fn v07_fidelity_bless_golden() {
    let records = capture_all();
    write_golden(&records);
}
