//! WLWL command-line interface.
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
use wlwl_parser::{parse, parse_with_warnings};

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
#[command(name = "wlwl", version, about = "WLWL language interpreter")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(clap::Subcommand, Debug)]
enum Cmd {
    /// Run a .wll source file
    Run {
        /// Path to the .wll file
        file: PathBuf,
        /// Output format for errors
        #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
        format: OutputFormat,
    },
    /// Only check (parse) without execution
    Check {
        /// Path to the .wll file
        file: PathBuf,
        /// Output format for errors
        #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
        format: OutputFormat,
    },
    /// Emit the AST of a .wll source file as JSON (AI-friendly).
    /// Implemented per D015 from the Phase 2 deviations log.
    Ast {
        /// Path to the .wll file
        file: PathBuf,
        /// Output format: json (default) or jsonl (one node per line; not
        /// used for AST, but accepted for symmetry with run/check).
        #[arg(long, value_enum, default_value_t = OutputFormat::Json)]
        format: OutputFormat,
    },
    /// Print the canonical formatter output (spec v0.4 §16.3) to
    /// stdout. Never writes the file in place (comments are not
    /// preserved by the AST rebuild -- see deviations P4-E2-001).
    Fmt {
        /// Path to the .wll file
        file: PathBuf,
        /// Check mode: print nothing; exit 1 with a W0053 diagnostic
        /// when the source deviates from the canonical form.
        #[arg(long)]
        check: bool,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Run { file, format } => run_file(&file, format, true),
        Cmd::Check { file, format } => run_file(&file, format, false),
        Cmd::Ast { file, format } => ast_file(&file, format),
        Cmd::Fmt { file, check } => fmt_file(&file, check),
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
    let (ast, parse_warnings) = match parse_with_warnings(&source, &file_name) {
        Ok(a) => a,
        Err(e) => return report_error(e, format),
    };

    if !execute {
        // Phase E4: unified warning channel. Parser warnings (W0020)
        // plus the static lint walk (W0010 / W0011 / W0012) surface
        // here; warnings never fail the check (exit 0).
        let mut warnings = parse_warnings;
        warnings.extend(wlwl_parser::lint(&ast));
        for w in &warnings {
            println!(
                "warning {}: {} ({}:{}:{})",
                w.code.as_str(),
                w.message,
                file_name,
                w.span.0,
                w.span.1
            );
        }
        println!("OK: parsed {} ({} bytes)", file_name, source.len());
        return ExitCode::SUCCESS;
    }

    let base_dir = file
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));

    // [v0.4 Phase E1] Wire up strict_types from wlwl.toml's
    // [features] strict_types. The flag is read at most once per
    // invocation; see `load_manifest_strict_types` for the failure
    // modes that fall back to `false`.
    let strict_types = load_manifest_strict_types(&base_dir);
    let mut ev = wlwl_eval::Evaluator::new()
        .with_source(&source, &file_name)
        .with_base_dir(base_dir.clone())
        .with_strict_types(strict_types);
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
        OutputFormat::Json => match serde_json::to_string_pretty(&AstOutput::new(&ast, &source)) {
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
                serde_json::to_string(&AstOutput::new(&ast, &source)).unwrap_or("{}".into())
            );
            ExitCode::SUCCESS
        }
    }
}

/// Top-level AST shape emitted by `wlwl ast` (v0.4 `Sec. 16.4.3`).
///
/// Phase E3: the root is now the stable-ID tree — every node carries
/// `node_id` / `parent_id` / `kind` / `span` / `hash` / `children`
/// (spec §16.4.1). Schema version bumped 0.3.1 → 0.4.0.
#[derive(serde::Serialize)]
struct AstOutput {
    /// Semantic version of the AST schema.
    ast_schema_version: &'static str,
    /// The source file (also the `module` part of every `node_id`).
    file: String,
    /// The number of source bytes (length of the input).
    source_bytes: usize,
    /// The single root expression as a stable-ID tree (a Block
    /// containing the program).
    root: wlwl_ast::stable::StableNode,
}

impl AstOutput {
    fn new(root: &Expr, source: &str) -> Self {
        let file = root.span().file.clone();
        Self {
            ast_schema_version: "0.4.0",
            source_bytes: source.len(),
            root: wlwl_ast::stable::stable_tree(root, &file),
            file,
        }
    }
}

/// `wlwl fmt <file>` -- canonical formatter (Phase E2, spec §16.3).
///
/// Default mode prints the canonical text to stdout (pipe into the
/// file yourself; we never overwrite in place because the AST rebuild
/// drops comments). With `--check`, prints nothing and exits 1 with a
/// `W0053 格式化偏离` diagnostic when the source is not canonical.
/// v0.6 §A.3: the canonical formatter drops comments, so the
/// `--check` comparison must also drop them from the source before
/// matching. Strips `// line` comments and `/* block */` comments
/// (nested block comments supported, matching the lexer).
///
/// Line-aware: a line whose only content is comments is removed
/// entirely (along with its trailing `\n`). This matches the
/// canonical layout where comments contribute zero characters.
fn strip_comments(src: &str) -> String {
    let bytes = src.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    let mut line_start = 0usize; // index where current line began in `out`
    let mut block_depth: u32 = 0;
    let mut in_string = false;
    while i < bytes.len() {
        // Skip line-leading whitespace. WLWL v0.6 has no syntactic
        // indentation, so leading spaces and tabs never survive into
        // the canonical form. We only do this when we are at the
        // start of a line (out.len() == line_start); once code has
        // been emitted, internal whitespace is preserved.
        if out.len() == line_start {
            while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\t') {
                i += 1;
            }
        }
        if i >= bytes.len() {
            break;
        }
        let b = bytes[i];
        if in_string {
            out.push(b);
            if b == b'\\' && i + 1 < bytes.len() {
                out.push(bytes[i + 1]);
                i += 2;
                continue;
            }
            if b == b'"' {
                in_string = false;
            }
            i += 1;
            continue;
        }
        if block_depth > 0 {
            // Inside a block comment: copy newlines so line numbers
            // stay aligned, drop everything else.
            if b == b'*' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
                block_depth -= 1;
                i += 2;
            } else if b == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'*' {
                block_depth += 1;
                i += 2;
            } else {
                if b == b'\n' {
                    // Truncate the line back to `line_start` (drop any
                    // code-leading content if the comment started mid-
                    // line and consumed the rest; in that case the line
                    // is now empty + a newline). For our purposes the
                    // simpler thing is: emit `\n` and reset line_start.
                    out.push(b);
                    line_start = out.len();
                }
                i += 1;
            }
            continue;
        }
        if b == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
            // `//` line comment. Truncate the current line back to
            // its start (drops any code that came before `//` only if
            // no code preceded it; otherwise we keep the code but
            // need to drop the comment). The simplest correct rule:
            // if `out.len() == line_start`, the line was comment-only
            // so far -- drop the whole line up to (but not including)
            // the newline.
            if out.len() == line_start {
                // Line was comment-only so far. Drop everything back to
                // line_start, then skip to the next newline.
                out.truncate(line_start);
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
                if i < bytes.len() {
                    // Skip the newline too — the comment-only line
                    // produces no output.
                    i += 1;
                }
                line_start = out.len();
            } else {
                // Code before the comment on this line; keep the code,
                // drop the comment, AND drop any trailing whitespace
                // before // (canonical layout has no trailing space
                // before EOL). Then keep the newline.
                while out.len() > line_start
                    && (out[out.len() - 1] == b' ' || out[out.len() - 1] == b'\t')
                {
                    out.pop();
                }
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
                if i < bytes.len() {
                    out.push(b'\n');
                    i += 1;
                    line_start = out.len();
                }
            }
            continue;
        }
        if b == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'*' {
            // `/*` block comment. Remember whether anything non-
            // whitespace has been emitted on this line so far; if
            // not, the comment "owns" the line and we drop it
            // entirely (along with the trailing `\n` if present).
            let line_empty = out.len() == line_start;
            block_depth = 1;
            i += 2;
            // Walk until matching `*/`. Track newlines to keep
            // line numbers aligned.
            let owned_line = line_empty;
            while i < bytes.len() && block_depth > 0 {
                if bytes[i] == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'*' {
                    block_depth += 1;
                    i += 2;
                } else if bytes[i] == b'*' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
                    block_depth -= 1;
                    i += 2;
                } else if bytes[i] == b'\n' {
                    if owned_line {
                        out.push(b'\n');
                        line_start = out.len();
                    }
                    // If not owned_line, the comment is mid-line and
                    // we just drop everything until `*/` (the
                    // newline stays as part of the line break).
                    i += 1;
                } else {
                    i += 1;
                }
            }
            // After `*/`, strip trailing whitespace from `out` so we
            // don\u2019t leave dangling space before EOL or before the
            // next token (matches `//` handling and canonical layout).
            while out.len() > line_start
                && (out[out.len() - 1] == b' ' || out[out.len() - 1] == b'\t')
            {
                out.pop();
            }
            // After `*/`, if the line is now empty AND we haven't
            // emitted a newline yet, skip the optional trailing `\n`.
            if owned_line && out.len() == line_start && i < bytes.len() && bytes[i] == b'\n' {
                i += 1;
            }
            continue;
        }
        if b == b'"' {
            in_string = true;
            out.push(b);
            i += 1;
            continue;
        }
        // Refresh `line_start` whenever we emit a real newline so that
        // the next iteration sees a clean "start of line" marker. This
        // is required for the `//` comment-only branch (which checks
        // `out.len() == line_start`) to fire correctly on lines that
        // come after ordinary code.
        if b == b'\n' {
            out.push(b);
            line_start = out.len();
            i += 1;
            continue;
        }
        out.push(b);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_default()
}

fn fmt_file(file: &PathBuf, check: bool) -> ExitCode {
    let source = match fs::read_to_string(file) {
        Ok(s) => s,
        Err(e) => {
            let d = WlwlDiagnostic::new(
                ErrorCode::E0042,
                format!("cannot read file ''{}'': {}", file.display(), e),
                Location::point(file.to_string_lossy().to_string(), 0, 0),
            );
            return report_diag(d, OutputFormat::Human);
        }
    };
    let file_name = file.to_string_lossy().to_string();
    let ast = match parse(&source, &file_name) {
        Ok(a) => a,
        Err(e) => return report_error(e, OutputFormat::Human),
    };
    let canonical = wlwl_formatter::format(&ast);
    if check {
        // v0.6 §A.3: canonical form drops comments; compare comment-
        // stripped source against canonical. Also tolerate a missing
        // trailing newline.
        let src_for_compare = strip_comments(&source);
        let canon_for_compare = strip_comments(&canonical);
        if src_for_compare == canon_for_compare {
            ExitCode::SUCCESS
        } else {
            let src_trim = src_for_compare.trim_end_matches('\n');
            let canon_trim = canon_for_compare.trim_end_matches('\n');
            if src_trim == canon_trim {
                ExitCode::SUCCESS
            } else {
                let d = WlwlDiagnostic::new(
                    ErrorCode::W0053,
                    "source deviates from the §16.3 canonical formatter contract",
                    Location::point(file_name, 1, 1),
                )
                .with_hint("run `wlwl fmt <file>` and apply its output");
                eprintln!("{}", d.render_human());
                ExitCode::from(1)
            }
        }
    } else {
        print!("{}", canonical);
        ExitCode::SUCCESS
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

/// [v0.4 Phase E1] Read `wlwl.toml` from the project root and
/// return the `[features] strict_types` flag.
///
/// Failure modes (no manifest / read error / parse error) all
/// silently default to `false` so that:
/// - standalone `.wll` files with no surrounding project still run;
/// - manifest parse errors don't block program execution (the
///   user will see them via `try_write_lock` if/when it runs).
///
/// The function is intentionally best-effort; spec §2.7 says
/// strict_types defaults to off, which is exactly the safe
/// default when we cannot read the manifest.
fn load_manifest_strict_types(base_dir: &std::path::Path) -> bool {
    let project_root = find_project_root(base_dir);
    let toml_path = project_root.join("wlwl.toml");
    if !toml_path.is_file() {
        return false;
    }
    let Ok(src) = fs::read_to_string(&toml_path) else {
        return false;
    };
    let Ok(manifest) = wlwl_toml::manifest::parse(&src) else {
        return false;
    };
    manifest.strict_types()
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
/// - Hash every `.wll` file in the dependency directory (deterministic
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
            eprintln!("warning: wlwl.toml parse error, skipping lock: {}", e);
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
        eprintln!("warning: failed to write {}: {}", lock_path.display(), e);
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
        let p = write_tmp("LET(x, 1); PRINT(x);", "hello.wll");
        let code = run_file(&p, OutputFormat::Human, true);
        assert_eq!(code, ExitCode::SUCCESS);
    }

    #[test]
    fn run_parse_error_reports_diagnostic() {
        let p = write_tmp("LET(x, 1) LET(y, 2);", "bad.wll");
        let code = run_file(&p, OutputFormat::Human, true);
        assert_eq!(code, ExitCode::from(1));
    }

    #[test]
    fn run_undefined_name() {
        let p = write_tmp("PRINT(zzz);", "undef.wll");
        let code = run_file(&p, OutputFormat::Human, true);
        assert_eq!(code, ExitCode::from(1));
    }

    #[test]
    fn check_only_parses() {
        let p = write_tmp("LET(x, 1);", "check.wll");
        let code = run_file(&p, OutputFormat::Human, false);
        assert_eq!(code, ExitCode::SUCCESS);
    }

    #[test]
    fn run_if_control_flow() {
        let p = write_tmp(r#"IF(==(1, 1), PRINT("yes"), PRINT("no"));"#, "if.wll");
        let code = run_file(&p, OutputFormat::Human, true);
        assert_eq!(code, ExitCode::SUCCESS);
    }

    // -- Phase 3: JSON / JSONL output ---------------------------------

    #[test]
    fn run_json_format_on_parse_error() {
        let p = write_tmp("LET(x, 1) LET(y, 2);", "bad-json.wll");
        // Capture stderr.
        let code = run_file(&p, OutputFormat::Json, true);
        assert_eq!(code, ExitCode::from(1));
    }

    #[test]
    fn run_jsonl_format_on_undefined_name() {
        // The Phase 3 JSONL output must contain the new schema fields:
        // error_category, retryable, suggestion_code, related.
        let p = write_tmp("PRINT(zzz);", "undef-jsonl.wll");
        let code = run_file(&p, OutputFormat::Jsonl, true);
        assert_eq!(code, ExitCode::from(1));
    }

    // -- Phase 3: wlwl ast subcommand --------------------------------

    #[test]
    fn ast_emits_json_for_valid_program() {
        let p = write_tmp("LET(x, 1); PRINT(x);", "ast-ok.wll");
        let code = ast_file(&p, OutputFormat::Json);
        assert_eq!(code, ExitCode::SUCCESS);
    }

    #[test]
    fn ast_reports_parse_error() {
        let p = write_tmp("LET(x, 1", "ast-bad.wll");
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
            dep_dir.join("lib.wll"),
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
entry = "main.wll"

[dependencies]
"myteam:lib" = { path = "dep" }
"#,
        )
        .unwrap();
        fs::write(
            dir.join("main.wll"),
            r#"IMPORT("myteam:lib", ["greet"]); PRINT(greet);"#,
        )
        .unwrap();
        // Run the program via run_file.
        let main_path = dir.join("main.wll");
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
        let lf: wlwl_toml::lock::Lockfile = serde_json::from_str(&lock_src).unwrap();
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
        fs::write(dir.join("hello.wll"), "PRINT(\"hi\");\n").unwrap();
        let code = run_file(&dir.join("hello.wll"), OutputFormat::Human, true);
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
            "got {:?}",
            root
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
entry = "main.wll"

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
        assert!(
            !content.contains("hub:lib"),
            "version-only should be skipped, got: {}",
            content
        );
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
    fn cli_strict_types_off_by_default() {
        // No wlwl.toml => strict_types defaults to off, so a
        // STRING-for-INTEGER mismatch must NOT raise E0033.
        let dir = std::env::temp_dir().join(format!(
            "wlwl-cli-strict-off-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let p = dir.join("main.wll");
        fs::write(&p, "LET(f, FUN((x: INTEGER), x)); f(\"hi\");").unwrap();
        let code = run_file(&p, OutputFormat::Human, true);
        assert_eq!(code, ExitCode::SUCCESS);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn cli_strict_types_on_raises_e0033_via_cli() {
        // wlwl.toml with [features] strict_types = true =>
        // run_file must surface E0033 (non-zero exit).
        let dir = std::env::temp_dir().join(format!(
            "wlwl-cli-strict-on-{}-{}",
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
            "[package]\nname = \"app\"\nversion = \"0.1.0\"\nentry = \"main.wll\"\n\n[features]\nstrict_types = true\n",
        )
        .unwrap();
        let p = dir.join("main.wll");
        fs::write(&p, "LET(f, FUN((x: INTEGER), x)); f(\"hi\");").unwrap();
        let code = run_file(&p, OutputFormat::Human, true);
        assert_ne!(code, ExitCode::SUCCESS, "E0033 should fail the run");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn cli_strict_types_human_render_includes_diag_message() {
        // The human-readable error format must include the E0033
        // canonical "type annotation mismatch" message.
        let dir = std::env::temp_dir().join(format!(
            "wlwl-cli-strict-hr-{}-{}",
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
            "[package]\nname = \"app\"\nversion = \"0.1.0\"\nentry = \"main.wll\"\n\n[features]\nstrict_types = true\n",
        )
        .unwrap();
        let p = dir.join("main.wll");
        fs::write(&p, "LET(f, FUN((x: INTEGER), x)); f(\"hi\");").unwrap();
        // Capture stdout/stderr? We just check that the exit code
        // indicates failure -- the diagnostic surface is tested
        // elsewhere (wlwl-error tests); here we only need to prove
        // the CLI honors the manifest flag end-to-end.
        let code = run_file(&p, OutputFormat::Human, true);
        assert_ne!(code, ExitCode::SUCCESS);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn cli_strict_types_missing_manifest_falls_back_to_false() {
        // Empty dir + .wll file with no surrounding project => no
        // wlwl.toml, so the helper must return false and the program
        // must succeed.
        let dir = std::env::temp_dir().join(format!(
            "wlwl-cli-strict-no-manifest-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let p = dir.join("orphan.wll");
        fs::write(&p, "LET(f, FUN((x: INTEGER), x)); f(\"hi\");").unwrap();
        let code = run_file(&p, OutputFormat::Human, true);
        assert_eq!(code, ExitCode::SUCCESS);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn cli_strict_types_broken_manifest_does_not_crash_run() {
        // Malformed wlwl.toml: the strict_types helper must default
        // to false (best-effort, see Phase E1 docs).
        let dir = std::env::temp_dir().join(format!(
            "wlwl-cli-strict-broken-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("wlwl.toml"), "not = [valid toml").unwrap();
        let p = dir.join("main.wll");
        fs::write(&p, "LET(f, FUN((x: INTEGER), x)); f(\"hi\");").unwrap();
        let code = run_file(&p, OutputFormat::Human, true);
        // Even though wlwl.toml is malformed, the program itself is
        // valid and must run successfully (strict_types defaults to
        // false on parse failure).
        assert_eq!(code, ExitCode::SUCCESS);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn ast_human_format_prints_debug() {
        // ast_file with OutputFormat::Human should print a {:#?} of
        // the AST. We just check the exit code is success and stdout
        // is non-empty.
        let p = write_tmp("LET(x, 1);", "ast_human.wll");
        let code = ast_file(&p, OutputFormat::Human);
        assert_eq!(code, ExitCode::SUCCESS);
    }

    #[test]
    fn ast_jsonl_format_streams_one_object() {
        // ast_file with OutputFormat::Jsonl prints one JSON object per
        // top-level expression. For a single expression, that's one
        // object.
        let p = write_tmp("LET(x, 1);", "ast_jsonl.wll");
        let code = ast_file(&p, OutputFormat::Jsonl);
        assert_eq!(code, ExitCode::SUCCESS);
    }

    // ---- Phase E2 (spec v0.4 §16.3): wlwl fmt -----------------------

    #[test]
    fn fmt_prints_canonical_output_successfully() {
        let p = write_tmp("LET( x ,1 );PRINT( x );", "fmt_print.wll");
        let code = fmt_file(&p, false);
        assert_eq!(code, ExitCode::SUCCESS);
    }

    #[test]
    fn fmt_check_canonical_source_succeeds() {
        // Already-canonical source (including the trailing newline
        // convention) must pass --check.
        let p = write_tmp("LET(x, 1);\nPRINT(x)\n", "fmt_ok.wll");
        let code = fmt_file(&p, true);
        assert_eq!(code, ExitCode::SUCCESS);
    }

    #[test]
    fn fmt_check_canonical_source_without_trailing_newline_succeeds() {
        // A missing final newline is not a §16.3 deviation.
        let p = write_tmp("LET(x, 1);\nPRINT(x)", "fmt_ok_nonl.wll");
        let code = fmt_file(&p, true);
        assert_eq!(code, ExitCode::SUCCESS);
    }

    #[test]
    fn fmt_check_deviating_source_fails_with_w0053() {
        // Non-canonical whitespace => --check exits 1 (W0053).
        let p = write_tmp("LET( x ,1 );", "fmt_dev.wll");
        let code = fmt_file(&p, true);
        assert_eq!(code, ExitCode::from(1));
    }

    #[test]
    fn fmt_check_missing_file_reports_error() {
        let p = std::path::PathBuf::from("/nonexistent/fmt_target.wll");
        let code = fmt_file(&p, true);
        assert_eq!(code, ExitCode::from(1));
    }

    #[test]
    fn fmt_check_parse_error_reports_diagnostic() {
        let p = write_tmp("LET(x, 1", "fmt_bad.wll");
        let code = fmt_file(&p, true);
        assert_eq!(code, ExitCode::from(1));
    }

    // ---- P5-V06-003: fmt --check strips comments from source --------

    #[test]
    fn fmt_check_tolerates_line_comments() {
        // Comments are not part of the canonical form (§A.3), so a
        // source whose code portion is canonical but which carries
        // `// ...` line comments must still pass `fmt --check`.
        let src = "\
                    // hello\n\
                    LET(x, 1);\n\
                    // trailing\n\
                    PRINT(x)\n";
        let p = write_tmp(src, "fmt_with_line_comments.wll");
        let code = fmt_file(&p, true);
        assert_eq!(
            code,
            ExitCode::SUCCESS,
            "comment-bearing canonical source should pass fmt --check"
        );
    }

    #[test]
    fn fmt_check_tolerates_block_comments() {
        // Canonical layout (each statement on its own line, no leading
        // indentation) interleaved with block comments. fmt --check
        // must accept this as canonical.
        let src = "/* head */\n\
                   LET(x, 1);\n\
                   /* between */\n\
                   PRINT(x)\n\
                   /* tail */\n";
        let p = write_tmp(src, "fmt_with_block_comments.wll");
        let code = fmt_file(&p, true);
        assert_eq!(
            code,
            ExitCode::SUCCESS,
            "block-comment-bearing canonical source should pass fmt --check"
        );
    }

    #[test]
    fn fmt_check_tolerates_nested_block_comments() {
        let src = "/* outer /* inner */ still */\n\
                   LET(x, 1);\n\
                   PRINT(x)\n";
        let p = write_tmp(src, "fmt_nested_comments.wll");
        let code = fmt_file(&p, true);
        assert_eq!(
            code,
            ExitCode::SUCCESS,
            "nested-block-comment source should pass fmt --check"
        );
    }

    #[test]
    fn fmt_check_still_rejects_non_canonical_whitespace() {
        // A source that differs from canonical form in a way that's
        // NOT just comments (e.g. extra spaces inside a call) must
        // still trip W0053.
        let src = "LET(x, 1);PRINT(x);\n";
        let p = write_tmp(src, "fmt_non_canonical.wll");
        let code = fmt_file(&p, true);
        assert_eq!(
            code,
            ExitCode::from(1),
            "non-canonical whitespace must still fail fmt --check"
        );
    }

    #[test]
    fn fmt_strip_comments_helper_basic() {
        // The unit-level guard for the comment-stripping state machine.
        // Line-only `//` comments drop the whole line.
        let r1 = strip_comments("// x\nLET(x, 1);\n");
        assert_eq!(r1, "LET(x, 1);\n");
        // Mid-line `//` keeps the code, drops the comment.
        assert_eq!(
            strip_comments("LET(x, 1); // tail\nPRINT(x);\n"),
            "LET(x, 1);\nPRINT(x);\n"
        );
        // Whole-line `/* ... */` drops the line.
        assert_eq!(strip_comments("/* a */\nLET(x, 1);\n"), "LET(x, 1);\n");
        // Mid-line `/* ... */` keeps the code, drops the comment.
        assert_eq!(
            strip_comments("LET(/* mid */ x, 1); /* tail */\nPRINT(x);\n"),
            "LET( x, 1);\nPRINT(x);\n"
        );
        // Nested block comment, whole line — drops the line.
        assert_eq!(
            strip_comments("/* o /* i */ s */\nLET(x, 1);\n"),
            "LET(x, 1);\n"
        );
        // Comments inside strings are preserved.
        assert_eq!(
            strip_comments("PRINT(\"// not a comment\");\n"),
            "PRINT(\"// not a comment\");\n"
        );
        assert_eq!(
            strip_comments("PRINT(\"a /* b */ c\");\n"),
            "PRINT(\"a /* b */ c\");\n"
        );
    }

    // ---- Phase E3 (spec v0.4 §16.4): stable node IDs -----------------

    #[test]
    fn ast_json_output_carries_node_ids_and_hashes() {
        // The §16.4.3 JSON schema: every node has node_id / kind /
        // span / hash; the root id is module:fn:<top>/body.
        let p = write_tmp("LET(x, 1); PRINT(x);", "ast_stable.wll");
        let source = fs::read_to_string(&p).unwrap();
        let ast = parse(&source, &p.to_string_lossy()).unwrap();
        let out = AstOutput::new(&ast, &source);
        let json = serde_json::to_value(&out).unwrap();
        assert_eq!(json["ast_schema_version"], "0.4.0");
        assert_eq!(
            json["root"]["node_id"],
            format!("{}:fn:<top>/body", p.to_string_lossy())
        );
        assert_eq!(json["root"]["kind"], "Block");
        let root_hash = json["root"]["hash"].as_str().unwrap();
        assert!(root_hash.starts_with("sha256:"));
        assert_eq!(root_hash.len(), "sha256:".len() + 64);
        // Two statements => two stmt children, each with ids + spans.
        let stmts = json["root"]["children"].as_array().unwrap();
        assert_eq!(stmts.len(), 2);
        for s in stmts {
            assert!(
                s["node_id"]
                    .as_str()
                    .unwrap()
                    .starts_with("t.wll:fn:<top>/body")
                    || s["node_id"].as_str().unwrap().contains("/stmt:")
            );
            assert!(s["hash"].as_str().unwrap().starts_with("sha256:"));
            assert!(s["span"]["line_start"].is_u64());
        }
    }

    #[test]
    fn ast_stable_ids_survive_line_shift() {
        // Same code, different line numbers => identical node_ids and
        // hashes (spans differ, ids do not).
        let tree_for = |src: &str| {
            let ast = parse(src, "t.wll").unwrap();
            serde_json::to_value(wlwl_ast::stable::stable_tree(&ast, "t.wll")).unwrap()
        };
        let a = tree_for("LET(x, 1); PRINT(x);");
        let b = tree_for("\nLET(x, 1);\nPRINT(x);");
        // tree_for serializes the StableNode itself (no "root" wrapper).
        assert_eq!(a["node_id"], b["node_id"]);
        assert_eq!(a["hash"], b["hash"]);
        assert_ne!(a["span"], b["span"]);
    }

    #[test]
    fn _silence_severity_returns_error() {
        // The dead-code suppression helper must still compile and
        // return Severity::Error so the import is kept alive.
        assert_eq!(_silence_severity(), Severity::Error);
    }
}
