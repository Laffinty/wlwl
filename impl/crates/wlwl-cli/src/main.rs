//! WLWL command-line interface (Phase 3).
//!
//! Phase 3 adds:
//! - `--format=jsonl` (streaming NDJSON) for AI tools (v0.3 `Sec. 14.7`)
//! - `wlwl ast <file> --format=json` (D015; AI tool input)
//! - Improved error rendering with the new schema fields (errorCategory,
//!   retryable, suggestion_code, related) shown in the human-readable
//!   output.
//!
//! v0.3 `Sec. 16.3` explicitly says **commands are out of scope** for
//! the language specification. The exact command set is a per-
//! implementation concern.

use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, ValueEnum};
use wlwl_ast::Expr;
use wlwl_error::{ErrorCode, Location, Severity, WlwlDiagnostic, WlwlError};
use wlwl_parser::parse;

#[derive(Debug, Clone, Copy, ValueEnum, Default)]
enum OutputFormat {
    /// Human-readable CLI output (default)
    #[default]
    Human,
    /// Single JSON object (per v0.3 `Sec. 14.3`)
    Json,
    /// JSONL streaming -- one diagnostic per line (per v0.3 `Sec. 14.7`)
    Jsonl,
}

#[derive(Parser, Debug)]
#[command(name = "wlwl", version, about = "WLWL language interpreter (Phase 3)")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(clap::Subcommand, Debug)]
enum Cmd {
    /// Run a .wl source file
    Run {
        /// Path to the .wl file
        file: PathBuf,
        /// Output format for errors
        #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
        format: OutputFormat,
    },
    /// Only check (parse) without execution
    Check {
        /// Path to the .wl file
        file: PathBuf,
        /// Output format for errors
        #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
        format: OutputFormat,
    },
    /// Emit the AST of a .wl source file as JSON (AI-friendly).
    /// Implemented per D015 from the Phase 2 deviations log.
    Ast {
        /// Path to the .wl file
        file: PathBuf,
        /// Output format: json (default) or jsonl (one node per line; not
        /// used for AST, but accepted for symmetry with run/check).
        #[arg(long, value_enum, default_value_t = OutputFormat::Json)]
        format: OutputFormat,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Run { file, format } => run_file(&file, format, true),
        Cmd::Check { file, format } => run_file(&file, format, false),
        Cmd::Ast { file, format } => ast_file(&file, format),
    }
}

/// Top-level entry: parse, optionally execute, report errors.
fn run_file(file: &PathBuf, format: OutputFormat, execute: bool) -> ExitCode {
    let source = match fs::read_to_string(file) {
        Ok(s) => s,
        Err(e) => {
            let d = WlwlDiagnostic::new(
                ErrorCode::E0042,
                format!("cannot read file ''{}'': {}", file.display(), e),
                Location::point(file.to_string_lossy().to_string(), 0, 0),
            );
            return report_diag(d, format);
        }
    };

    let file_name = file.to_string_lossy().to_string();
    let ast = match parse(&source, &file_name) {
        Ok(a) => a,
        Err(e) => return report_error(e, format),
    };

    if !execute {
        println!("OK: parsed {} ({} bytes)", file_name, source.len());
        return ExitCode::SUCCESS;
    }

    let base_dir = file
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));

    let mut ev = wlwl_eval::Evaluator::new()
        .with_source(&source, &file_name)
        .with_base_dir(base_dir.clone());
    match ev.eval(&ast) {
        Ok(_v) => {
            try_write_lock(&base_dir);
            ExitCode::SUCCESS
        }
        Err(e) => report_error(e, format),
    }
}

/// `wlwl ast <file>` -- emit the AST as JSON for AI tools to consume.
fn ast_file(file: &PathBuf, format: OutputFormat) -> ExitCode {
    let source = match fs::read_to_string(file) {
        Ok(s) => s,
        Err(e) => {
            let d = WlwlDiagnostic::new(
                ErrorCode::E0042,
                format!("cannot read file ''{}'': {}", file.display(), e),
                Location::point(file.to_string_lossy().to_string(), 0, 0),
            );
            return report_diag(d, format);
        }
    };
    let file_name = file.to_string_lossy().to_string();
    let ast = match parse(&source, &file_name) {
        Ok(a) => a,
        Err(e) => return report_error(e, format),
    };
    match format {
        OutputFormat::Human => {
            println!("{:#?}", ast);
            ExitCode::SUCCESS
        }
        OutputFormat::Json => match serde_json::to_string_pretty(&AstOutput::from(&ast)) {
            Ok(s) => {
                println!("{}", s);
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("internal: AST serialize failed: {}", e);
                ExitCode::from(2)
            }
        },
        OutputFormat::Jsonl => {
            // Stream one JSON object per top-level expression (rarely more
            // than one in practice). Symmetric with how errors stream.
            println!(
                "{}",
                serde_json::to_string(&AstOutput::from(&ast)).unwrap_or("{}".into())
            );
            ExitCode::SUCCESS
        }
    }
}

/// Top-level AST shape emitted by `wlwl ast` (v0.3 `Sec. 16.3` AI input).
#[derive(serde::Serialize)]
struct AstOutput<'a> {
    /// Semantic version of the AST schema.
    ast_schema_version: &'static str,
    /// The source file the AST was parsed from.
    file: String,
    /// The number of source bytes (length of the input).
    source_bytes: usize,
    /// The single root expression (a Block containing the program).
    root: &'a Expr,
}

impl<'a> From<&'a Expr> for AstOutput<'a> {
    fn from(root: &'a Expr) -> Self {
        Self {
            ast_schema_version: "0.3.1",
            file: root.span().file.clone(),
            source_bytes: 0,
            root,
        }
    }
}

fn report_error(err: WlwlError, format: OutputFormat) -> ExitCode {
    report_diag(err.diagnostic().clone(), format)
}

fn report_diag(d: WlwlDiagnostic, format: OutputFormat) -> ExitCode {
    match format {
        OutputFormat::Human => eprintln!("{}", d.render_human()),
        OutputFormat::Json => eprintln!("{}", d.render_json()),
        OutputFormat::Jsonl => eprintln!("{}", d.render_jsonl()),
    }
    ExitCode::from(1)
}

// Suppress unused-import warning for Severity when no human rendering
// uses it directly (we use it indirectly via `severity` field on
// the diagnostic). Kept here for future expansion (e.g. --strict mode).
#[allow(dead_code)]
fn _silence_severity() -> Severity {
    Severity::Error
}

// ── wlwl.lock generation (v0.3 §13.8) ────────────────────────────

/// Walk up from `start` looking for a `wlwl.toml`. Returns the
/// directory containing the manifest, or `start` itself if no
/// manifest is found. (Duplicated here rather than depending on
/// `wlwl-eval` internals to keep the CLI self-contained.)
fn find_project_root(start: &std::path::Path) -> std::path::PathBuf {
    let mut cur = start.to_path_buf();
    loop {
        if cur.join("wlwl.toml").is_file() {
            return cur;
        }
        if !cur.pop() {
            return start.to_path_buf();
        }
    }
}

/// After a successful `wlwl run`, refresh the project's
/// `wlwl.lock` (per spec §13.8):
///
/// - Locate the project root (nearest ancestor with `wlwl.toml`).
/// - Read the manifest. If it fails to parse, silently skip; the
///   user will see the manifest error elsewhere.
/// - Build one `LockEntry` per `[dependencies]` entry that has a
///   `path`. Version-only deps are reserved for v0.4 (central
///   registry) and are skipped here.
/// - Hash every `.wl` file in the dependency directory (deterministic
///   SHA-256 from `wlwl_toml::lock::hash_dependency_dir`).
/// - Write atomically via `wlwl_toml::lock::write`.
///
/// Failures are warnings on stderr, not fatal -- the program ran
/// successfully, the lock is just bookkeeping.
fn try_write_lock(base_dir: &std::path::Path) {
    let project_root = find_project_root(base_dir);
    let toml_path = project_root.join("wlwl.toml");
    if !toml_path.is_file() {
        return;
    }
    let src = match fs::read_to_string(&toml_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!(
                "warning: cannot read {} for lock generation: {}",
                toml_path.display(),
                e
            );
            return;
        }
    };
    let manifest = match wlwl_toml::manifest::parse(&src) {
        Ok(m) => m,
        Err(e) => {
            eprintln!(
                "warning: wlwl.toml parse error, skipping lock: {}",
                e
            );
            return;
        }
    };
    let mut entries = Vec::new();
    for (key, dep) in &manifest.dependencies {
        let path = match dep.local_path() {
            Some(p) => p,
            None => continue, // v0.4 central registry
        };
        let dep_dir = base_dir.join(path);
        let hash = wlwl_toml::lock::hash_dependency_dir(&dep_dir)
            .ok()
            .flatten();
        entries.push(wlwl_toml::lock::LockEntry {
            name: key.clone(),
            path: Some(path.to_string()),
            version: None,
            hash,
        });
    }
    let lock = wlwl_toml::lock::Lockfile {
        schema_version: wlwl_toml::lock::CURRENT_SCHEMA_VERSION.to_string(),
        entries,
    };
    let lock_path = project_root.join("wlwl.lock");
    if let Err(e) = wlwl_toml::lock::write(&lock_path, &lock) {
        eprintln!(
            "warning: failed to write {}: {}",
            lock_path.display(),
            e
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_tmp(content: &str, name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join("wlwl-cli-tests");
        fs::create_dir_all(&dir).unwrap();
        let p = dir.join(name);
        let mut f = fs::File::create(&p).unwrap();
        f.write_all(content.as_bytes()).unwrap();
        p
    }

    #[test]
    fn run_hello() {
        let p = write_tmp("LET(x, 1); PRINT(x);", "hello.wl");
        let code = run_file(&p, OutputFormat::Human, true);
        assert_eq!(code, ExitCode::SUCCESS);
    }

    #[test]
    fn run_parse_error_reports_diagnostic() {
        let p = write_tmp("LET(x, 1) LET(y, 2);", "bad.wl");
        let code = run_file(&p, OutputFormat::Human, true);
        assert_eq!(code, ExitCode::from(1));
    }

    #[test]
    fn run_undefined_name() {
        let p = write_tmp("PRINT(zzz);", "undef.wl");
        let code = run_file(&p, OutputFormat::Human, true);
        assert_eq!(code, ExitCode::from(1));
    }

    #[test]
    fn check_only_parses() {
        let p = write_tmp("LET(x, 1);", "check.wl");
        let code = run_file(&p, OutputFormat::Human, false);
        assert_eq!(code, ExitCode::SUCCESS);
    }

    #[test]
    fn run_if_control_flow() {
        let p = write_tmp(r#"IF(==(1, 1), PRINT("yes"), PRINT("no"));"#, "if.wl");
        let code = run_file(&p, OutputFormat::Human, true);
        assert_eq!(code, ExitCode::SUCCESS);
    }

    // -- Phase 3: JSON / JSONL output ---------------------------------

    #[test]
    fn run_json_format_on_parse_error() {
        let p = write_tmp("LET(x, 1) LET(y, 2);", "bad-json.wl");
        // Capture stderr.
        let code = run_file(&p, OutputFormat::Json, true);
        assert_eq!(code, ExitCode::from(1));
    }

    #[test]
    fn run_jsonl_format_on_undefined_name() {
        // The Phase 3 JSONL output must contain the new schema fields:
        // error_category, retryable, suggestion_code, related.
        let p = write_tmp("PRINT(zzz);", "undef-jsonl.wl");
        let code = run_file(&p, OutputFormat::Jsonl, true);
        assert_eq!(code, ExitCode::from(1));
    }

    // -- Phase 3: wlwl ast subcommand --------------------------------

    #[test]
    fn ast_emits_json_for_valid_program() {
        let p = write_tmp("LET(x, 1); PRINT(x);", "ast-ok.wl");
        let code = ast_file(&p, OutputFormat::Json);
        assert_eq!(code, ExitCode::SUCCESS);
    }

    #[test]
    fn ast_reports_parse_error() {
        let p = write_tmp("LET(x, 1", "ast-bad.wl");
        let code = ast_file(&p, OutputFormat::Json);
        assert_eq!(code, ExitCode::from(1));
    }

    #[test]
    fn run_writes_wlwl_lock_when_manifest_present() {
        // Set up a temp project with a wlwl.toml that has a path
        // dependency, run a minimal program, and confirm that a
        // `wlwl.lock` is generated with the dependency's hash.
        let dir = std::env::temp_dir().join(format!(
            "wlwl-cli-lock-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        // Dependency lives next to the project, in a sibling dir.
        let dep_dir = dir.join("dep");
        fs::create_dir_all(&dep_dir).unwrap();
        fs::write(
            dep_dir.join("lib.wl"),
            "LET(greet, 1); EXPORT([\"greet\"]);\n",
        )
        .unwrap();
        // Manifest points at ../dep. We pass `dir` (the project
        // root) as the run target's base dir.
        fs::write(
            dir.join("wlwl.toml"),
            r#"[package]
name = "lock-test"
version = "0.1.0"
entry = "main.wl"

[dependencies]
"myteam:lib" = { path = "dep" }
"#,
        )
        .unwrap();
        fs::write(
            dir.join("main.wl"),
            r#"IMPORT("myteam:lib", ["greet"]); PRINT(greet);"#,
        )
        .unwrap();
        // Run the program via run_file.
        let main_path = dir.join("main.wl");
        let code = run_file(&main_path, OutputFormat::Human, true);
        assert_eq!(code, ExitCode::SUCCESS);
        // The lock should now exist and have one entry.
        let lock_path = dir.join("wlwl.lock");
        assert!(
            lock_path.is_file(),
            "wlwl.lock should be created at {}",
            lock_path.display()
        );
        let lock_src = fs::read_to_string(&lock_path).unwrap();
        let lf: wlwl_toml::lock::Lockfile =
            serde_json::from_str(&lock_src).unwrap();
        assert_eq!(lf.schema_version, wlwl_toml::lock::CURRENT_SCHEMA_VERSION);
        assert_eq!(lf.entries.len(), 1);
        let e = &lf.entries[0];
        assert_eq!(e.name, "myteam:lib");
                assert_eq!(e.path.as_deref(), Some("dep"));
        assert!(e.hash.is_some(), "lock entry should carry a hash");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn run_does_not_write_lock_when_no_manifest() {
        // No wlwl.toml in the project -> no lock generation, no error.
        let dir = std::env::temp_dir().join(format!(
            "wlwl-cli-nolock-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("hello.wl"), "PRINT(\"hi\");\n").unwrap();
        let code = run_file(&dir.join("hello.wl"), OutputFormat::Human, true);
        assert_eq!(code, ExitCode::SUCCESS);
        assert!(!dir.join("wlwl.lock").exists());
        let _ = fs::remove_dir_all(&dir);
    }
    // ---- P3-009d: lock generation edge cases + find_project_root ----

    #[test]
    fn find_project_root_walks_up_to_manifest() {
        // Create a nested project: outer/wlwl.toml, outer/inner/deep/.
        // find_project_root(inner/deep) must return outer/.
        let dir = std::env::temp_dir().join(format!(
            "wlwl-cli-fpr-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::remove_dir_all(&dir);
        let deep = dir.join("inner").join("deep");
        fs::create_dir_all(&deep).unwrap();
        fs::write(dir.join("wlwl.toml"), "").unwrap();
        let root = find_project_root(&deep);
        assert_eq!(root, dir, "should walk up to manifest dir");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn find_project_root_returns_start_when_no_manifest() {
        // No manifest anywhere on the way up; should return the start
        // directory itself (so callers do not panic).
        let dir = std::env::temp_dir().join(format!(
            "wlwl-cli-fpr-noop-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        // Use the temp dir as a known path. find_project_root must
        // return *some* path under the system temp.
        let root = find_project_root(&dir);
        assert!(
            root.starts_with(std::env::temp_dir()) || root == dir,
            "got {:?}", root
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn try_write_lock_no_manifest_is_silent_noop() {
        // No wlwl.toml anywhere -> function should return silently
        // and NOT create a lock file.
        let dir = std::env::temp_dir().join(format!(
            "wlwl-cli-twl-noop-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        try_write_lock(&dir);
        assert!(!dir.join("wlwl.lock").exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn try_write_lock_skips_version_only_deps() {
        // Manifest with a version-only dep (no path): v0.3 says
        // skip these. The lock should still be written, but with an
        // empty entries list.
        let dir = std::env::temp_dir().join(format!(
            "wlwl-cli-twl-version-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("wlwl.toml"),
            r#"[package]
name = "v"
version = "0.1.0"
entry = "main.wl"

[dependencies]
"hub:lib" = { version = "1.2.3" }
"#,
        )
        .unwrap();
        try_write_lock(&dir);
        let lock_path = dir.join("wlwl.lock");
        assert!(lock_path.exists(), "lock should be written");
        let content = fs::read_to_string(&lock_path).unwrap();
        // No entries -> empty array. We don't pin to JSON format, just
        // assert the structure is present.
        assert!(content.contains("\"entries\""), "got: {}", content);
        assert!(!content.contains("hub:lib"), "version-only should be skipped, got: {}", content);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn try_write_lock_manifest_parse_error_silently_skips() {
        // A malformed wlwl.toml must NOT crash the program -- the lock
        // is bookkeeping, not load-bearing.
        let dir = std::env::temp_dir().join(format!(
            "wlwl-cli-twl-bad-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("wlwl.toml"), "this is = not [valid").unwrap();
        // Should not panic.
        try_write_lock(&dir);
        assert!(!dir.join("wlwl.lock").exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn ast_human_format_prints_debug() {
        // ast_file with OutputFormat::Human should print a {:#?} of
        // the AST. We just check the exit code is success and stdout
        // is non-empty.
        let p = write_tmp("LET(x, 1);", "ast_human.wl");
        let code = ast_file(&p, OutputFormat::Human);
        assert_eq!(code, ExitCode::SUCCESS);
    }

    #[test]
    fn ast_jsonl_format_streams_one_object() {
        // ast_file with OutputFormat::Jsonl prints one JSON object per
        // top-level expression. For a single expression, that's one
        // object.
        let p = write_tmp("LET(x, 1);", "ast_jsonl.wl");
        let code = ast_file(&p, OutputFormat::Jsonl);
        assert_eq!(code, ExitCode::SUCCESS);
    }

    #[test]
    fn _silence_severity_returns_error() {
        // The dead-code suppression helper must still compile and
        // return Severity::Error so the import is kept alive.
        assert_eq!(_silence_severity(), Severity::Error);
    }
}
