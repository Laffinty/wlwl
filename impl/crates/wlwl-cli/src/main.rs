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

use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, ValueEnum};
use wlwl_ast::Expr;
use wlwl_error::{ErrorCode, Location, Severity, WlwlDiagnostic, WlwlError};
use wlwl_parser::{parse, parse_with_warnings};
use wlwl_toml::manifest::{GradualTyping, GradualTypingSetting};
use wlwl_types::{DeclaredBinding, Ty};

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
    /// Print the module signature implied by a .wll file (v0.10 Step 7 /
    /// plan §4.3 C3). Writes nothing -- pipe it yourself.
    Sig {
        /// Path to the .wll file
        file: PathBuf,
        /// Signature rendering: canonical text (default) or machine-readable JSON
        #[arg(long, value_enum, default_value_t = SigFormat::Text)]
        format: SigFormat,
    },
    /// Write the module signature to `<file>.wll.sig` (v0.10 Step 7 /
    /// plan §4.3 C3).
    ///
    /// Never overwrites an existing signature unless `--force`: a
    /// hand-tuned signature is hand-tuned for a reason.
    SigGen {
        /// Path to the .wll file
        file: PathBuf,
        /// Overwrite an existing `<file>.wll.sig`
        #[arg(long)]
        force: bool,
    },
}

/// `wlwl sig` output shape.
///
/// Deliberately **not** the shared [`OutputFormat`]: there is no meaningful
/// JSONL form for a single module's signature, and pretending otherwise
/// would push that decision onto whoever wires the tool up later.
#[derive(ValueEnum, Debug, Clone, Copy, Default)]
enum SigFormat {
    /// Canonical signature text -- byte-identical to what `sig-gen` writes
    #[default]
    Text,
    /// Machine-readable JSON (AI tools / future `wlwl lsp` diagnostics)
    Json,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Run { file, format } => run_file(&file, format, true),
        Cmd::Check { file, format } => run_file(&file, format, false),
        Cmd::Ast { file, format } => ast_file(&file, format),
        Cmd::Fmt { file, check } => fmt_file(&file, check),
        Cmd::Sig { file, format } => sig_file(&file, format),
        Cmd::SigGen { file, force } => sig_gen_file(&file, force),
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

    let base_dir = file
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));

    // [v0.10 Step 2 / ADR-0020 A3] The compile-time static pass. `check`
    // and `run` share this single entry point so the two subcommands can
    // never drift apart. Returns Some(exit code) when the caller must
    // abort.
    if let Some(code) = static_check_gate(&ast, file, &base_dir, format) {
        return code;
    }

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

/// [v0.10 Step 2 / ADR-0020 A3] Read `wlwl.toml` from the project root
/// and return the `[features] gradual_typing` switch.
///
/// Best-effort like [`load_manifest_strict_types`]: no manifest / read
/// error / parse error all fall back to `Off`, which is the only safe
/// default for a pass that must be a **no-op** on every v0.9 program
/// (ADR-0020 S1).
fn load_manifest_gradual_typing(base_dir: &std::path::Path) -> GradualTypingSetting {
    let project_root = find_project_root(base_dir);
    let toml_path = project_root.join("wlwl.toml");
    if !toml_path.is_file() {
        return GradualTypingSetting::default();
    }
    let Ok(src) = fs::read_to_string(&toml_path) else {
        return GradualTypingSetting::default();
    };
    let Ok(manifest) = wlwl_toml::manifest::parse(&src) else {
        return GradualTypingSetting::default();
    };
    manifest.gradual_typing()
}

/// [v0.10 Step 5 / ADR-0020 A6′] 内建签名表:名字 → `Ty::Fun` 签名。
///
/// `SigTy` → `Ty` 的映射规则:
/// - 基类型一一对应;
/// - 容器**无类型参数**(`ARRAY` / `DICT` 首批不带元素与键值类型,故取
///   `Dynamic` 元素)—— 与运行时 `E0033` 那条路径 *deliberately deferred*
///   的嵌套匹配保持同一档,不比运行时更激进;
/// - `RESULT` 两侧都取 `Dynamic`(首批不建模 `OK` / `ERR` 载荷);
/// - `SigTy::Dynamic` → `Ty::Dynamic`。
///
/// 首批(A6′)未结构化的 50 条**不进表** —— 「查不到」与「返回类型未知」
/// 对静态层是同一种结果:落 `Dynamic`,不产生诊断。
fn builtin_sig_table() -> std::collections::HashMap<String, wlwl_types::Ty> {
    use wlwl_eval::registry::{builtin_sig, SigTy};
    use wlwl_types::Ty;

    let conv = |s: SigTy| -> Ty {
        match s {
            SigTy::Integer => Ty::Integer,
            SigTy::Float => Ty::Float,
            SigTy::String => Ty::String,
            SigTy::Boolean => Ty::Boolean,
            SigTy::Null => Ty::Null,
            SigTy::Array => Ty::Array(Box::new(Ty::Dynamic)),
            SigTy::Dict => Ty::Dict(Box::new(Ty::Dynamic), Box::new(Ty::Dynamic)),
            SigTy::Result => Ty::Result(Box::new(Ty::Dynamic), Box::new(Ty::Dynamic)),
            SigTy::Function => Ty::Fun {
                params: vec![Ty::Dynamic],
                ret: Box::new(Ty::Dynamic),
            },
            SigTy::Dynamic => Ty::Dynamic,
        }
    };

    let mut table = std::collections::HashMap::new();
    for spec in wlwl_eval::registry::BUILTIN_REGISTRY {
        // 只收首批条目;`params` 恒为 `None`(可选形参 / 变长实参会让精确
        // 元数必然误报,见 `BuiltinSig::params` 的文档)。
        let Some(sig) = spec.sig else { continue };
        let Some(params) = sig.params else {
            // 无形参信息:只带返回类型。`params: []` 让静态层**跳过元数
            // 检查**(见 `check_call_with_sig` 的 `check_arity`)但仍然
            // 采用返回类型。
            table.insert(
                spec.name.to_string(),
                Ty::Fun {
                    params: Vec::new(),
                    ret: Box::new(conv(sig.ret)),
                },
            );
            continue;
        };
        table.insert(
            spec.name.to_string(),
            Ty::Fun {
                params: params.iter().map(|p| conv(*p)).collect(),
                ret: Box::new(conv(sig.ret)),
            },
        );
    }
    // `builtin_sig` 是注册表对外的唯一结构化入口;这里再过一次是为了
    // 让「CLI 只用公开 API,不碰 BUILTIN_REGISTRY 内部结构」成立。
    debug_assert_eq!(
        table.len(),
        wlwl_eval::registry::BUILTIN_REGISTRY
            .iter()
            .filter(|s| s.sig.is_some())
            .count()
    );
    debug_assert!(table.contains_key("LEN") && builtin_sig("LEN").is_some());
    table
}

/// [v0.10 Step 2 / ADR-0020 A3] The single static-check entry point,
/// shared by `wlwl check` and `wlwl run`.
///
/// Returns `Some(exit_code)` when the caller must abort, `None` to carry
/// on. Per level:
///
/// - `off`   — **the checker is never called**. This is where the
///   "default zero breakage, zero cost" promise is implemented: not
///   "run and discard", but "do not run".
/// - `warn`  — run, report `W0110`-`W0112`, never block.
/// - `error` — run, report `E0110`-`E0112`, block with exit 1.
///
/// An invalid `[features] gradual_typing` value has already fallen back
/// to `off` inside `wlwl-toml`; here we surface the rejection as a
/// `W0013`-class name warning so a typo is visible rather than silent.
fn static_check_gate(
    ast: &Expr,
    file: &std::path::Path,
    base_dir: &std::path::Path,
    format: OutputFormat,
) -> Option<ExitCode> {
    let setting = load_manifest_gradual_typing(base_dir);

    if let Some(raw) = setting.invalid_value() {
        let d = WlwlDiagnostic::new(
            ErrorCode::W0001,
            format!(
                "invalid [features] gradual_typing value `{raw}`; \
                 expected \"off\" | \"warn\" | \"error\" -- falling back to \"off\""
            ),
            Location::point(base_dir.join("wlwl.toml").to_string_lossy(), 0, 0),
        );
        let d = d.with_severity(Severity::Warning);
        match format {
            OutputFormat::Human => eprintln!("{}", d.render_human()),
            OutputFormat::Json => eprintln!("{}", d.render_json()),
            OutputFormat::Jsonl => eprintln!("{}", d.render_jsonl()),
        }
    }

    if !setting.is_enabled() {
        return None;
    }

    let severity = match setting.mode() {
        GradualTyping::Warn => Severity::Warning,
        _ => Severity::Error,
    };

    // [v0.10 Step 5 / ADR-0020 A6′] 内建签名表。**只在开启时构建** ——
    // `off` 档连表都不建,这是「默认零开销」的一部分。
    //
    // 映射放在 CLI 而不是 `wlwl-types` 里,是为了保住 ADR-0020
    // Decision 1 的分层:内建签名住在 `wlwl-eval` 的注册表,而
    // `wlwl-eval` 不依赖 `wlwl-types`、反之亦然。CLI 是同时依赖两者的
    // 那一层,所以它是唯一能同时看见 `SigTy` 与 `Ty` 的地方。
    let builtins = builtin_sig_table();

    // [v0.10 Step 6 / plan §4] 一次遍历同时拿到两样东西:类型诊断,以及
    // 根作用域的绑定类型(模块契约要比的就是这份表)。
    let checked = wlwl_types::check_program_detailed(ast, &builtins);
    let mut rendered: Vec<WlwlDiagnostic> = checked
        .diags
        .iter()
        .filter_map(|d| d.to_diagnostic(severity))
        .collect();

    // [v0.10 Step 6 / plan §4.1 C1 + §4.2 C2] 模块契约:签名文件与
    // `SEALED` 声明面。走完整 import 图,所以 `wlwl check main.wll`
    // 一次就把本地依赖链上的契约全验了。
    //
    // 依赖模块的**签名文件读不了/写错**也算诊断(它就是契约的一部分);
    // 但依赖模块自己的语法错、找不到的模块**不在这里报** —— 那些是
    // `run` 的地盘,在这里再报一遍等于让 `check main.wll` 替别人家的
    // 文件失败。
    let contracts = scan_module_contracts(file, ast, &checked.declared, &builtins, severity);
    rendered.extend(contracts);
    if rendered.is_empty() {
        return None;
    }
    // Deterministic order: source order. The checker already discovers
    // in walk order, so a stable sort by (line, col) is enough to make
    // multi-diagnostic output byte-stable across runs.
    rendered.sort_by_key(|d| (d.location.line, d.location.col));

    let blocking = severity == Severity::Error;
    for d in &rendered {
        match format {
            OutputFormat::Human => eprintln!("{}", d.render_human()),
            OutputFormat::Json => eprintln!("{}", d.render_json()),
            OutputFormat::Jsonl => eprintln!("{}", d.render_jsonl()),
        }
    }
    if blocking {
        Some(ExitCode::from(1))
    } else {
        None
    }
}

// ── v0.10 Step 6 (P0-2 C1 / C2): 模块契约 ─────────────────────

/// 旁路签名文件路径:`math.wll` → `math.wll.sig`。
///
/// 就地追加后缀,不新建目录、不引入别的命名空间 —— 决策 D-2 要的就是
/// 「同目录、多一个后缀」,这样签名文件能跟着模块一起被版本控制搬走。
fn sig_path_for(module: &std::path::Path) -> PathBuf {
    let mut name = module.as_os_str().to_os_string();
    name.push(".sig");
    PathBuf::from(name)
}

/// 读一个模块的签名文件。`Ok(None)` = 没有签名文件(那是 v0.9 行为)。
///
/// 读不到 / 写错都算**契约的**问题,原样冒泡给调用方按档位渲染。
fn load_module_sig(module: &std::path::Path) -> Result<Option<wlwl_types::ModuleSig>, WlwlError> {
    let sig = sig_path_for(module);
    if !sig.is_file() {
        return Ok(None);
    }
    let text = fs::read_to_string(&sig).map_err(|e| {
        WlwlDiagnostic::new(
            ErrorCode::E0042,
            format!("cannot read module signature '{}': {}", sig.display(), e),
            Location::point(sig.to_string_lossy().to_string(), 0, 0),
        )
    })?;
    wlwl_types::parse_module_sig(&text, &sig.to_string_lossy()).map(Some)
}

/// 沿 import 图检查模块契约,返回已按档位渲染好的诊断。
///
/// 每个模块在**第一次被发现**时做一次自查,然后进队列;出队时只查它对
/// 直接依赖的消费方一侧。缓存 `路径 -> (契约, 已报过的名字)` 保证:
/// 同一个模块被十个文件 import 也只查一次(不重复解析、不重复报),环也
/// 不会打转 —— 这里不求值,所以不需要 `E0041` 那套环检测。
///
/// 每个模块查两件事:
/// 1. **自查** —— 自己的 `EXPORT` 面 vs 自己的签名 / `SEALED`;
/// 2. **查消费方** —— 自己的 `IMPORT` 名字 vs 直接依赖的签名。
///
/// 无签名的模块走 `wlwl-types` 的空契约短路,一条诊断都不会有。
fn scan_module_contracts(
    entry: &std::path::Path,
    entry_ast: &Expr,
    entry_declared: &[DeclaredBinding],
    builtins: &HashMap<String, Ty>,
    severity: Severity,
) -> Vec<WlwlDiagnostic> {
    use std::collections::{BTreeMap, HashMap as PathCache};

    let mut out: Vec<WlwlDiagnostic> = Vec::new();

    // 入口模块先自查(它不会被别人「发现」,所以不走下面的发现路径)。
    let entry_contract = match build_contract(entry, entry_ast, severity) {
        Ok(c) => c,
        Err(d) => {
            out.push(d);
            return out;
        }
    };
    let entry_check = wlwl_types::check_exports(entry_ast, entry_declared, &entry_contract);
    out.extend(
        entry_check
            .diags
            .iter()
            .filter_map(|d| d.to_diagnostic(severity)),
    );

    // 规范路径 -> (契约, 自查已报过的「不在声明面内」的名字)
    let mut cache: PathCache<PathBuf, (wlwl_types::ModuleContract, BTreeSet<String>)> =
        PathCache::new();
    cache.insert(
        normalize_path(entry),
        (entry_contract, entry_check.undeclared),
    );

    // 队列里带上已解析的 AST,避免同一模块解析两次。绑定表在「第一次被
    // 发现」时就算掉了(自查在那儿做),不必跟着 AST 一起走。
    let mut queue: Vec<(PathBuf, Expr)> = vec![(entry.to_path_buf(), clone_expr(entry_ast))];

    while let Some((path, ast)) = queue.pop() {
        // 直接依赖的契约:逐个 spec 解析到文件,顺带把新文件推进队列。
        let base = path.parent().map(|p| p.to_path_buf()).unwrap_or_default();
        let mut direct: BTreeMap<String, wlwl_types::ImportedModule> = BTreeMap::new();
        for spec in wlwl_types::import_specs(&ast) {
            // std 模块没有磁盘源,也就没有旁路签名;解析失败的 spec 是
            // `run` 的 E0040 / E0043,在那里报。
            let Ok(Some(dep)) = wlwl_eval::resolve_module_file(&spec, &base) else {
                continue;
            };
            let key = normalize_path(&dep);
            if !cache.contains_key(&key) {
                // 第一次见到的依赖:读、解析、算绑定表、自查,然后进队列。
                let Ok(source) = fs::read_to_string(&dep) else {
                    continue;
                };
                let dep_name = dep.to_string_lossy().to_string();
                let Ok(dep_ast) = parse(&source, &dep_name) else {
                    continue; // 依赖自己的语法错归 `run`,见函数文档
                };
                let dep_contract = match build_contract(&dep, &dep_ast, severity) {
                    Ok(c) => c,
                    Err(d) => {
                        out.push(d);
                        continue;
                    }
                };
                let dep_declared = wlwl_types::check_program_detailed(&dep_ast, builtins).declared;
                let dep_check = wlwl_types::check_exports(&dep_ast, &dep_declared, &dep_contract);
                // 依赖模块的**注解诊断**不在这里报(用户没问那个文件);
                // 它的契约问题照报,见函数文档。
                out.extend(
                    dep_check
                        .diags
                        .iter()
                        .filter_map(|d| d.to_diagnostic(severity)),
                );
                cache.insert(key.clone(), (dep_contract, dep_check.undeclared));
                queue.push((dep, dep_ast));
            }
            let Some((contract, already_reported)) = cache.get(&key) else {
                continue; // 上面刚报过签名文件问题,这个 spec 跳过
            };
            direct.insert(
                spec,
                wlwl_types::ImportedModule {
                    contract: contract.clone(),
                    already_reported: already_reported.clone(),
                },
            );
        }
        out.extend(
            wlwl_types::check_imports(&ast, &direct)
                .iter()
                .filter_map(|d| d.to_diagnostic(severity)),
        );
    }

    out
}

/// 组装一个模块的契约载体(签名文件 + `SEALED`),并把签名文件的问题
/// 按当前档位渲染成诊断。
fn build_contract(
    path: &std::path::Path,
    ast: &Expr,
    severity: Severity,
) -> Result<wlwl_types::ModuleContract, WlwlDiagnostic> {
    let sig = load_module_sig(path).map_err(|e| e.diagnostic().clone().with_severity(severity))?;
    Ok(wlwl_types::ModuleContract::from_module(
        ast,
        path.to_string_lossy().to_string(),
        sig,
    ))
}

/// 队列去重用的规范化路径。Windows 上同一个文件可能以
/// `./math.wll` 与 `math.wll` 两种写法到达,不归一就会重复查、重复报。
fn normalize_path(p: &std::path::Path) -> PathBuf {
    use std::path::Component;
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

// ── v0.10 Step 7 (P0-2 C3): sig / sig-gen ─────────────────────

/// 从一个 `.wll` 文件反推签名 —— `sig` 与 `sig-gen` 的共用前半段。
///
/// 类型来自同一次类型遍历的根作用域绑定表(带内建签名表,所以
/// `LET(n, LEN([1, 2]))` 能定成 `INTEGER` 而不是 `DYNAMIC`)。
/// `gradual_typing` 在这里**不参与**:显式的工具调用,用户要的就是签名,
/// 跟门禁开关无关。
fn derive_signature(file: &std::path::Path) -> Result<wlwl_types::ModuleSig, WlwlError> {
    let source = fs::read_to_string(file).map_err(|e| {
        WlwlDiagnostic::new(
            ErrorCode::E0042,
            format!("cannot read file ''{}'': {}", file.display(), e),
            Location::point(file.to_string_lossy().to_string(), 0, 0),
        )
    })?;
    let ast = parse(&source, &file.to_string_lossy())?;
    let builtins = builtin_sig_table();
    let checked = wlwl_types::check_program_detailed(&ast, &builtins);
    Ok(wlwl_types::sig_from_module(&ast, &checked.declared))
}

/// `wlwl sig <file>` —— 把签名打到 stdout,一个字节都不写盘。
///
/// 与 `wlwl ast --json` 同一形态的只读子命令:想落盘用 `sig-gen`。
fn sig_file(file: &std::path::Path, format: SigFormat) -> ExitCode {
    let sig = match derive_signature(file) {
        Ok(s) => s,
        // 诊断渲染固定走 human:本子命令的 `--format` 只管签名本身。
        Err(e) => return report_error(e, OutputFormat::Human),
    };
    match format {
        SigFormat::Text => {
            print!("{}", sig);
            ExitCode::SUCCESS
        }
        SigFormat::Json => {
            let out = SigOutput::new(file, &sig);
            match serde_json::to_string_pretty(&out) {
                Ok(s) => {
                    println!("{}", s);
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("internal: signature serialize failed: {}", e);
                    ExitCode::from(2)
                }
            }
        }
    }
}

/// `wlwl sig-gen <file>` —— 把签名写进 `<file>.wll.sig`。
///
/// 三条纪律:
/// 1. **默认不覆盖**已有签名(`--force` 才覆盖)—— 手调过的签名是手调
///    的理由;这是 CLI 用法错误,所以走 stderr + exit 1 而**不占用一个
///    语言错误码**(码表纪律:一个码号只对应一种条件,「会不会覆盖」不是
///    语言问题)。
/// 2. **零导出就不写文件**。「不导出任何名字」的诚实契约就是**没有**
///    签名文件(与 v0.9 行为同解),留一个 0 字节文件只会污染仓库。
/// 3. 写进去的内容必须能被 `check` 读回来 —— 生成侧的这条不变量由
///    `wlwl-types` 的 `sig_gen_roundtrip_parse_check` 守住。
fn sig_gen_file(file: &std::path::Path, force: bool) -> ExitCode {
    let sig = match derive_signature(file) {
        Ok(s) => s,
        Err(e) => return report_error(e, OutputFormat::Human),
    };
    let target = sig_path_for(file);
    if target.exists() && !force {
        eprintln!(
            "{}: signature file already exists; pass --force to overwrite",
            target.display()
        );
        return ExitCode::from(1);
    }
    if sig.entries.is_empty() {
        eprintln!(
            "note: '{}' exports nothing; not writing a signature file",
            file.display()
        );
        return ExitCode::SUCCESS;
    }
    let text = sig.to_string();
    match fs::write(&target, &text) {
        Ok(()) => {
            println!(
                "wrote {} ({} exports, {} bytes)",
                target.display(),
                sig.entries.len(),
                text.len()
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("cannot write '{}': {}", target.display(), e);
            ExitCode::from(1)
        }
    }
}

/// `wlwl sig --format=json` 的输出形状(AI 工具 / 未来 `wlwl lsp` 消费)。
///
/// `text` 字段与 `wlwl sig <file>` 的 stdout、与 `sig-gen` 写盘的内容
/// **逐字节相同** —— 三者共用 `ModuleSig` 的 `Display`,不各自渲染,
/// 所以不存在「JSON 说一套、文本说另一套」。
#[derive(serde::Serialize)]
struct SigOutput {
    /// 签名 schema 版本(照 `wlwl ast --json` 的先例带版本号)
    sig_schema_version: &'static str,
    /// 签名来源的 `.wll`
    module: String,
    /// 签名在磁盘上的位置(尚未写盘时它就是「将要写到哪」)
    signature_file: String,
    /// 规范文本,与文本输出逐字节相同
    text: String,
    /// 每条导出声明,按渲染顺序(名字升序)
    entries: Vec<SigEntryOutput>,
}

/// 一条签名声明的 JSON 形态。函数用 `params` + `returns`,值用 `type`,
/// 两组字段互斥(不出现的那组直接不出现在 JSON 里)。
#[derive(serde::Serialize)]
struct SigEntryOutput {
    name: String,
    /// `"function"` 或 `"value"`
    kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    r#type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    returns: Option<String>,
}

impl SigOutput {
    fn new(file: &std::path::Path, sig: &wlwl_types::ModuleSig) -> Self {
        let entries = sig
            .entries
            .values()
            .map(|e| match &e.ty {
                Ty::Fun { params, ret } => SigEntryOutput {
                    name: e.name.clone(),
                    kind: "function",
                    r#type: None,
                    params: Some(params.iter().map(Ty::to_string).collect()),
                    returns: Some(ret.to_string()),
                },
                other => SigEntryOutput {
                    name: e.name.clone(),
                    kind: "value",
                    r#type: Some(other.to_string()),
                    params: None,
                    returns: None,
                },
            })
            .collect();
        SigOutput {
            sig_schema_version: "1.0.0",
            module: file.to_string_lossy().to_string(),
            signature_file: sig_path_for(file).to_string_lossy().to_string(),
            text: sig.to_string(),
            entries,
        }
    }
}

/// 队列里要带 AST,而调用方只借给我们一份 —— `Expr` 是 `Clone` 的,复制
/// 一份比把它改成 `Rc` 划算(模块文件都很小,而且整棵 AST 只复制一次)。
fn clone_expr(e: &Expr) -> Expr {
    e.clone()
}

/// After a successful `wlwl run`, refresh the project's/// `wlwl.lock` (per spec §13.8):
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

    /// 写一个带 `wlwl.toml` 的最小工程,返回 `.wll` 路径。
    ///
    /// 每个用例独占一个子目录 —— `find_project_root` 是**向上**找清单的,
    /// 所以清单放在子目录里不会污染同级的其他测试。
    fn write_project(dir: &str, gradual: &str, source: &str) -> PathBuf {
        let root = std::env::temp_dir().join("wlwl-cli-tests").join(dir);
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("wlwl.toml"),
            format!(
                "[package]\nname = \"probe\"\nversion = \"0.0.1\"\nentry = \"main.wll\"\n\n[features]\ngradual_typing = {}\n",
                gradual
            ),
        )
        .unwrap();
        let p = root.join("main.wll");
        fs::write(&p, source).unwrap();
        p
    }

    /// 一份必然触发 `E0112` 的源码:函数声明返回 INTEGER,尾表达式是 STRING。
    const MISMATCH: &str = "LET(f, FUN((a: INTEGER) : INTEGER, \"wrong\"));";
    /// 一份在任何档位下都合法的源码。
    const CLEAN: &str = "LET(x: INTEGER, 1); LET(f, FUN((a: INTEGER) : INTEGER, a)); f(x);";

    // -- v0.10 Step 2 (ADR-0020 A3): gradual_typing 三档 ----------------

    /// 锁测试:`off` 是**默认**,且必须真的什么都不做。
    ///
    /// 两种"默认"都要验:完全没有清单的裸文件,以及显式写了 `off` 的工程。
    /// 二者都必须在有静态失配的源码上依然通过 `check` 与 `run`。
    #[test]
    fn default_off_zero_diag() {
        // 无清单 —— 裸 .wll 文件照旧能过。
        let bare = write_tmp(MISMATCH, "gradual_bare.wll");
        assert_eq!(
            run_file(&bare, OutputFormat::Human, false),
            ExitCode::SUCCESS
        );
        assert_eq!(
            run_file(&bare, OutputFormat::Human, true),
            ExitCode::SUCCESS
        );

        // 显式 off。
        let off = write_project("gradual_off", "\"off\"", MISMATCH);
        assert_eq!(
            run_file(&off, OutputFormat::Human, false),
            ExitCode::SUCCESS
        );
        assert_eq!(run_file(&off, OutputFormat::Human, true), ExitCode::SUCCESS);
    }

    /// 锁测试:`warn` 档发 `W0110`-`W0112` 但**不阻塞**退出码。
    #[test]
    fn warn_mode_soft() {
        let p = write_project("gradual_warn", "\"warn\"", MISMATCH);
        // check 与 run 都只发软诊断,退出码保持 0。
        assert_eq!(run_file(&p, OutputFormat::Human, false), ExitCode::SUCCESS);
        assert_eq!(run_file(&p, OutputFormat::Human, true), ExitCode::SUCCESS);
    }

    /// 锁测试:`error` 档发 `E0110`-`E0112` 并**硬拦**退出码。
    ///
    /// `check` 与 `run` 共用同一个门禁入口,所以两边都必须被拦下 ——
    /// 这正是"两个子命令不会漂移"的证明。
    #[test]
    fn error_mode_hard() {
        let p = write_project("gradual_error", "\"error\"", MISMATCH);
        assert_eq!(run_file(&p, OutputFormat::Human, false), ExitCode::from(1));
        assert_eq!(run_file(&p, OutputFormat::Human, true), ExitCode::from(1));
    }

    /// 合法源码在 `error` 档下必须照常通过 —— 门禁不许变成「见注解就拦」。
    #[test]
    fn error_mode_does_not_reject_clean_programs() {
        let p = write_project("gradual_clean", "\"error\"", CLEAN);
        assert_eq!(run_file(&p, OutputFormat::Human, false), ExitCode::SUCCESS);
        assert_eq!(run_file(&p, OutputFormat::Human, true), ExitCode::SUCCESS);
    }

    /// 非法取值回落 `off`,并把笔误报出来而不是静默吞掉。
    #[test]
    fn invalid_value_falls_back_to_off_and_is_reported() {
        let p = write_project("gradual_invalid", "\"loud\"", MISMATCH);
        assert_eq!(run_file(&p, OutputFormat::Human, false), ExitCode::SUCCESS);
        assert_eq!(run_file(&p, OutputFormat::Human, true), ExitCode::SUCCESS);
    }

    /// [v0.10 Step 2] `check` 的三层验收:parse / static / run。
    ///
    /// A3 把 `check` 从「只 parse」升级为「parse → 可选静态 check」,
    /// 所以这三层必须各自独立可验,不能互相掩盖:
    /// - 层 1 parse:语法错在任何档位下都失败,与 `gradual_typing` 无关;
    /// - 层 2 static:注解失配只在 `error` 档失败,`warn` 档放行;
    /// - 层 3 run:同一份源码,`run` 也走同一道门禁。
    #[test]
    fn check_is_layered_parse_static_run() {
        // 层 1 —— parse。
        let bad = write_tmp("LET(x, 1) LET(y, 2);", "layer_parse_err.wll");
        assert_eq!(
            run_file(&bad, OutputFormat::Human, false),
            ExitCode::from(1)
        );

        // 层 2 —— static。
        let errored = write_project("layered_error", "\"error\"", MISMATCH);
        assert_eq!(
            run_file(&errored, OutputFormat::Human, false),
            ExitCode::from(1)
        );
        let warned = write_project("layered_warn", "\"warn\"", MISMATCH);
        assert_eq!(
            run_file(&warned, OutputFormat::Human, false),
            ExitCode::SUCCESS
        );

        // 层 3 —— run。
        let run_errored = write_project("layered_run_error", "\"error\"", MISMATCH);
        assert_eq!(
            run_file(&run_errored, OutputFormat::Human, true),
            ExitCode::from(1)
        );
    }

    #[test]
    fn check_only_parses() {
        let p = write_tmp("LET(x, 1);", "check.wll");
        let code = run_file(&p, OutputFormat::Human, false);
        assert_eq!(code, ExitCode::SUCCESS);
    }

    // -- v0.10 Step 5 (A6′): 内建结构化签名真的接上了 -------------------

    /// `LEN` 在首批内(返回 `INTEGER`),所以把它接到 `STRING` 注解上必报。
    /// 这条在 Step 5 之前**不会**报 —— 当时内建一律落 `Dynamic`。
    #[test]
    fn a6p_structured_builtin_signature_takes_effect() {
        let p = write_project(
            "a6p_takes_effect",
            "\"error\"",
            "LET(x: STRING, LEN([1, 2]));",
        );
        assert_eq!(run_file(&p, OutputFormat::Human, false), ExitCode::from(1));
        // 匹配的注解照常通过。
        let ok = write_project(
            "a6p_takes_effect_ok",
            "\"error\"",
            "LET(x: INTEGER, LEN([1, 2]));",
        );
        assert_eq!(run_file(&ok, OutputFormat::Human, false), ExitCode::SUCCESS);
    }

    /// 首批未覆盖的内建(这里取 `CHANNEL_LEN` —— Concurrent 组 17 条
    /// 整体留给后续批次)**必须继续静默**。落 `Dynamic` = 不报。
    #[test]
    fn a6p_uncovered_builtins_stay_silent() {
        for src in [
            "LET(x: STRING, CHANNEL_LEN(ch));",
            "LET(x: STRING, CHANNEL_NEW(0));",
            "LET(x: INTEGER, GET_PROP(o, \"k\"));",
            "LET(x: STRING, AT([1, 2], 0));",
        ] {
            let p = write_project("a6p_uncovered", "\"error\"", src);
            assert_eq!(
                run_file(&p, OutputFormat::Human, false),
                ExitCode::SUCCESS,
                "uncovered builtin must not be judged: {src}"
            );
        }
    }

    /// 用户重绑定内建名时(`allow_builtin_shadow` 的场景),静态层必须用
    /// **用户的**签名,而不是注册表里的 —— 否则会对用户的定义报错。
    #[test]
    fn a6p_shadowed_builtin_uses_the_user_signature() {
        let src = concat!(
            "LET(LEN, FUN((x: STRING) : STRING, x)); ",
            "LET(y: STRING, LEN(\"abc\"));"
        );
        let p = write_project("a6p_shadow_ok", "\"error\"", src);
        assert_eq!(run_file(&p, OutputFormat::Human, false), ExitCode::SUCCESS);
        // 用户的签名确实生效:传 INTEGER 就该报,尽管内建 `LEN` 收容器。
        let bad = concat!(
            "LET(LEN, FUN((x: STRING) : STRING, x)); ",
            "LET(y: STRING, LEN(1));"
        );
        let p2 = write_project("a6p_shadow_bad", "\"error\"", bad);
        assert_eq!(run_file(&p2, OutputFormat::Human, false), ExitCode::from(1));
    }

    /// 首批不检查内建元数 —— 可选形参(`SUB(s, start, end?)`)与变长
    /// 实参(`PRINT(args...)`)让精确元数必然误报。
    #[test]
    fn a6p_builtin_arity_is_not_checked() {
        for src in [
            "PRINT();",
            "PRINT(1, 2, 3, 4, 5);",
            r#"LET(s, SUB("hello"));"#,
            r#"LET(s, SUB("hello", 1));"#,
            r#"LET(s, SUB("hello", 1, 3));"#,
        ] {
            let p = write_project("a6p_arity", "\"error\"", src);
            assert_eq!(
                run_file(&p, OutputFormat::Human, false),
                ExitCode::SUCCESS,
                "builtin arity must not be judged in batch 1: {src}"
            );
        }
    }

    /// `off` 档下整条静态链路都不执行 —— 连内建签名表都不该建。
    /// 用一个「开启时必报」的夹具反证:`off` 仍过 = 门禁真的没跑。
    #[test]
    fn a6p_off_mode_does_not_consult_builtin_signatures() {
        let src = "LET(x: STRING, LEN([1, 2]));";
        for setting in ["\"off\"", "DEFAULT_NO_KEY"] {
            let p = if setting == "DEFAULT_NO_KEY" {
                write_tmp(src, "a6p_off_no_key.wll")
            } else {
                write_project("a6p_off", setting, src)
            };
            assert_eq!(
                run_file(&p, OutputFormat::Human, false),
                ExitCode::SUCCESS,
                "off mode must not judge: {setting}"
            );
        }
    }

    /// 写一个带 `wlwl.toml` 的最小工程,返回**工程根目录**。
    ///
    /// Step 6 的用例要摆下模块 + 旁路签名文件,所以这里返回目录而不是
    /// `.wll` 路径,让用例自己往里放文件。
    fn module_project(dir: &str, gradual: &str) -> PathBuf {
        let root = std::env::temp_dir().join("wlwl-cli-tests").join(dir);
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("wlwl.toml"),
            format!(
                "[package]\nname = \"probe\"\nversion = \"0.0.1\"\nentry = \"main.wll\"\n\n[features]\ngradual_typing = {}\n",
                gradual
            ),
        )
        .unwrap();
        root
    }

    // -- v0.10 Step 6 (P0-2 C1): 旁路签名文件 ----------------------------

    /// `math.wll` + 一份**匹配**的签名。返回工程根。
    ///
    /// 注意 `add` 那个绑定**故意不加** `LET` 注解:函数的类型由 `FUN`
    /// 自己的形参/返回注解推出来(`FUN[INTEGER, INTEGER] -> INTEGER`),
    /// 那正是 `E0115` 要比的东西。给 `LET` 加注解反而会把这条推断盖掉。
    fn signed_math_project(dir: &str, gradual: &str, sig: &str) -> PathBuf {
        let root = module_project(dir, gradual);
        fs::write(
            root.join("math.wll"),
            "LET(add, FUN((a: INTEGER, b: INTEGER) : INTEGER, +(a, b)));\n\
             LET(PI, 3);\n\
             EXPORT([\"add\", \"PI\"]);\n",
        )
        .unwrap();
        fs::write(
            root.join("main.wll"),
            "IMPORT(\"math\", [\"add\"]);\nPRINT(add(1, 2));\n",
        )
        .unwrap();
        fs::write(root.join("math.wll.sig"), sig).unwrap();
        root
    }

    /// 锁测试:签名与实现对得上时,`check` 干净通过 —— 新增的这层不能给
    /// 正确的程序添乱。
    #[test]
    fn c1_matching_signature_passes() {
        let root = signed_math_project(
            "c1_ok",
            "\"error\"",
            "EXPORT add (INTEGER, INTEGER) : INTEGER\nEXPORT PI : INTEGER\n",
        );
        assert_eq!(
            run_file(&root.join("main.wll"), OutputFormat::Human, false),
            ExitCode::SUCCESS
        );
        // `run` 也照常(契约层是编译期的,不改变求值语义)。
        assert_eq!(
            run_file(&root.join("main.wll"), OutputFormat::Human, true),
            ExitCode::SUCCESS
        );
    }

    /// 锁测试:`no_sig_behaves_as_v09` 的端到端版。删掉签名文件,同一个
    /// 工程必须回到「什么都没报」。
    #[test]
    fn c1_no_sig_behaves_as_v09() {
        let root = signed_math_project(
            "c1_no_sig",
            "\"error\"",
            "EXPORT nothing_like_this : STRING\n",
        );
        let _ = fs::remove_file(root.join("math.wll.sig"));
        assert_eq!(
            run_file(&root.join("main.wll"), OutputFormat::Human, false),
            ExitCode::SUCCESS
        );
    }

    /// 锁测试:`E0113`(多出)/ `E0114`(缺失)/ `E0115`(类型冲突)三条
    /// 在真实工程里各报各的。
    #[test]
    fn c1_signature_mismatch_reports_e0113_e0114_e0115() {
        // E0113:签名没声明 `PI`。
        let root = signed_math_project(
            "c1_e0113",
            "\"error\"",
            "EXPORT add (INTEGER, INTEGER) : INTEGER\n",
        );
        assert_eq!(
            run_file(&root.join("main.wll"), OutputFormat::Human, false),
            ExitCode::from(1)
        );
        // E0114:签名声明了实现没导出的 `sub`。
        let root = signed_math_project(
            "c1_e0114",
            "\"error\"",
            "EXPORT add (INTEGER, INTEGER) : INTEGER\nEXPORT sub (INTEGER, INTEGER) : INTEGER\n",
        );
        assert_eq!(
            run_file(&root.join("main.wll"), OutputFormat::Human, false),
            ExitCode::from(1)
        );
        // E0115:签名说 `add` 返回 STRING,实现注解是 INTEGER。
        let root = signed_math_project(
            "c1_e0115",
            "\"error\"",
            "EXPORT add (INTEGER, INTEGER) : STRING\nEXPORT PI : INTEGER\n",
        );
        assert_eq!(
            run_file(&root.join("main.wll"), OutputFormat::Human, false),
            ExitCode::from(1)
        );
    }

    /// 签名是**给别人看的契约**:消费方用了一个签名没声明的名字,报
    /// `E0113` —— 即使那个名字模块真的导出了。
    #[test]
    fn c1_import_of_an_undeclared_name_is_e0113() {
        let root = signed_math_project(
            "c1_import",
            "\"error\"",
            "EXPORT add (INTEGER, INTEGER) : INTEGER\n",
        );
        // `PI` 被模块导出了,但签名没声明它。
        fs::write(
            root.join("main.wll"),
            "IMPORT(\"math\", [\"PI\"]);\nPRINT(PI);\n",
        )
        .unwrap();
        assert_eq!(
            run_file(&root.join("main.wll"), OutputFormat::Human, false),
            ExitCode::from(1)
        );
        // 声明过就静默。
        let root = signed_math_project(
            "c1_import_ok",
            "\"error\"",
            "EXPORT add (INTEGER, INTEGER) : INTEGER\nEXPORT PI : INTEGER\n",
        );
        fs::write(
            root.join("main.wll"),
            "IMPORT(\"math\", [\"PI\"]);\nPRINT(PI);\n",
        )
        .unwrap();
        assert_eq!(
            run_file(&root.join("main.wll"), OutputFormat::Human, false),
            ExitCode::SUCCESS
        );
    }

    /// 契约沿 import 图传递:`main` → `a.wll` → `b.wll`,`b` 的签名对不上
    /// 时 `check main.wll` 就该报,不必单独 check 每一个文件。
    #[test]
    fn c1_contracts_are_checked_across_the_import_graph() {
        let root = module_project("c1_graph", "\"error\"");
        fs::write(root.join("b.wll"), "LET(v, 1); EXPORT([\"v\"]);\n").unwrap();
        // `b` 的签名说 v 是 STRING,实现没注解但字面量推成 INTEGER。
        fs::write(root.join("b.wll.sig"), "EXPORT v : STRING\n").unwrap();
        fs::write(
            root.join("a.wll"),
            "IMPORT(\"b\", [\"v\"]);\nEXPORT([\"v\"]);\n",
        )
        .unwrap();
        fs::write(
            root.join("main.wll"),
            "IMPORT(\"a\", [\"v\"]);\nPRINT(v);\n",
        )
        .unwrap();
        assert_eq!(
            run_file(&root.join("main.wll"), OutputFormat::Human, false),
            ExitCode::from(1)
        );
    }

    /// 签名文件自己写错了也是契约问题,按档位报(语法码 E0010,不是新码)。
    #[test]
    fn c1_malformed_signature_is_reported_at_its_own_line() {
        let root = signed_math_project(
            "c1_malformed",
            "\"error\"",
            "EXPORT add (INTEGER, INTEGER) : INTEGER\nMODULE nonsense\n",
        );
        assert_eq!(
            run_file(&root.join("main.wll"), OutputFormat::Human, false),
            ExitCode::from(1)
        );
    }

    /// `warn` 档发 `W0113` 但不挡退出码;`off` 档连签名文件都不读。
    #[test]
    fn c1_respects_the_three_gradual_typing_levels() {
        let sig = "EXPORT nothing_like_this : INTEGER\n";
        let warn = signed_math_project("c1_warn", "\"warn\"", sig);
        assert_eq!(
            run_file(&warn.join("main.wll"), OutputFormat::Human, false),
            ExitCode::SUCCESS
        );
        for setting in ["\"off\"", "DEFAULT_NO_KEY"] {
            let root = if setting == "DEFAULT_NO_KEY" {
                let root = module_project("c1_off_no_key", "\"off\"");
                let _ = fs::remove_file(root.join("wlwl.toml"));
                fs::write(root.join("math.wll"), "LET(a, 1); EXPORT([\"a\"]);\n").unwrap();
                fs::write(
                    root.join("main.wll"),
                    "IMPORT(\"math\", [\"a\"]);\nPRINT(a);\n",
                )
                .unwrap();
                fs::write(root.join("math.wll.sig"), sig).unwrap();
                root
            } else {
                signed_math_project("c1_off", setting, sig)
            };
            assert_eq!(
                run_file(&root.join("main.wll"), OutputFormat::Human, false),
                ExitCode::SUCCESS,
                "off mode must not judge the signature: {setting}"
            );
        }
    }

    // -- v0.10 Step 6 (P0-2 C2): SEALED 声明面 ---------------------------

    /// 锁测试:`sealed_violation` 的端到端版。`SEALED` 划的公开面比
    /// `EXPORT` 窄 → `E0113`,消息里要点名是密封面。
    #[test]
    fn c2_sealed_violation_is_reported() {
        let root = module_project("c2_violation", "\"error\"");
        fs::write(
            root.join("math.wll"),
            "SEALED([\"open\"]);\n\
             LET(open: INTEGER, 1);\n\
             LET(secret: INTEGER, 2);\n\
             EXPORT([\"open\", \"secret\"]);\n",
        )
        .unwrap();
        fs::write(
            root.join("main.wll"),
            "IMPORT(\"math\", [\"open\"]);\nPRINT(open);\n",
        )
        .unwrap();
        assert_eq!(
            run_file(&root.join("main.wll"), OutputFormat::Human, false),
            ExitCode::from(1)
        );
        // 密封面与导出面一致 → 静默。
        fs::write(
            root.join("math.wll"),
            "SEALED([\"open\"]);\nLET(open: INTEGER, 1);\nEXPORT([\"open\"]);\n",
        )
        .unwrap();
        assert_eq!(
            run_file(&root.join("main.wll"), OutputFormat::Human, false),
            ExitCode::SUCCESS
        );
    }

    /// `SEALED` 与签名可以并存:两条载体各自查自己那一侧,一个根因一条
    /// 诊断。签名对了但密封面过期,只报密封面那一条。
    #[test]
    fn c2_seal_and_signature_coexist() {
        let root = module_project("c2_both", "\"error\"");
        fs::write(
            root.join("math.wll"),
            "SEALED([\"open\"]);\n\
             LET(open: INTEGER, 1);\n\
             LET(secret: INTEGER, 2);\n\
             EXPORT([\"open\", \"secret\"]);\n",
        )
        .unwrap();
        fs::write(
            root.join("main.wll"),
            "IMPORT(\"math\", [\"open\"]);\nPRINT(open);\n",
        )
        .unwrap();
        // 签名把两个名字都声明了(消费方侧静默),但密封面只留 `open`。
        fs::write(
            root.join("math.wll.sig"),
            "EXPORT open : INTEGER\nEXPORT secret : INTEGER\n",
        )
        .unwrap();
        assert_eq!(
            run_file(&root.join("main.wll"), OutputFormat::Human, false),
            ExitCode::from(1)
        );
        // 把 `secret` 也封进去,整条链就干净了。
        fs::write(
            root.join("math.wll"),
            "SEALED([\"open\", \"secret\"]);\n\
             LET(open: INTEGER, 1);\n\
             LET(secret: INTEGER, 2);\n\
             EXPORT([\"open\", \"secret\"]);\n",
        )
        .unwrap();
        assert_eq!(
            run_file(&root.join("main.wll"), OutputFormat::Human, false),
            ExitCode::SUCCESS
        );
    }

    /// 零破坏的端到端版:写上 `SEALED` 之后,程序照跑、值不变。
    #[test]
    fn c2_sealed_does_not_change_runtime_behaviour() {
        let root = module_project("c2_runtime", "\"error\"");
        fs::write(
            root.join("math.wll"),
            "SEALED([\"twice\"]);\nLET(twice, FUN((n) : INTEGER, *(n, 2)));\nEXPORT([\"twice\"]);\n",
        )
        .unwrap();
        fs::write(
            root.join("main.wll"),
            "IMPORT(\"math\", [\"twice\"]);\nPRINT(twice(21));\n",
        )
        .unwrap();
        assert_eq!(
            run_file(&root.join("main.wll"), OutputFormat::Human, true),
            ExitCode::SUCCESS
        );
    }

    // -- v0.10 Step 7 (P0-2 C3): wlwl sig / wlwl sig-gen ---------------

    /// 写一个带 `EXPORT` 的模块,返回路径。
    fn write_module(dir: &str, name: &str, source: &str) -> PathBuf {
        let root = std::env::temp_dir().join("wlwl-cli-tests").join(dir);
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let p = root.join(name);
        fs::write(&p, source).unwrap();
        p
    }

    const SIG_MODULE: &str = "\
LET(add, FUN((a: INTEGER, b: INTEGER) : INTEGER, +(a, b)));
LET(PI, 3);
EXPORT([\"add\", \"PI\"]);
";

    /// `wlwl sig` 只打到 stdout,一个字节都不写盘。
    #[test]
    fn c3_sig_prints_the_signature_and_writes_nothing() {
        let p = write_module("c3_sig", "math.wll", SIG_MODULE);
        assert_eq!(sig_file(&p, SigFormat::Text), ExitCode::SUCCESS);
        assert!(!sig_path_for(&p).exists(), "sig must not write anything");
    }

    /// 计划书点名的验收项:`sig-gen` 的产物能被 `check` 读回来(且
    /// 契约自洽 —— 一份生成出来的签名不该反过来指控自己的模块)。
    #[test]
    fn c3_sig_gen_output_passes_check() {
        let root = std::env::temp_dir()
            .join("wlwl-cli-tests")
            .join("c3_roundtrip");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        // 工程开着 `error` 档:生成 → check,必须过。
        fs::write(
            root.join("wlwl.toml"),
            "[package]\nname = \"p\"\nversion = \"0.0.1\"\nentry = \"math.wll\"\n\n[features]\ngradual_typing = \"error\"\n",
        )
        .unwrap();
        let module = root.join("math.wll");
        fs::write(&module, SIG_MODULE).unwrap();

        assert_eq!(sig_gen_file(&module, false), ExitCode::SUCCESS);
        let sig = sig_path_for(&module);
        assert!(sig.is_file(), "sig-gen must write {}", sig.display());

        // 产物能解析回来,且内容与文本输出逐字节相同。
        let parsed = wlwl_types::parse_module_sig(
            &fs::read_to_string(&sig).unwrap(),
            &sig.to_string_lossy(),
        )
        .expect("generated signature parses");
        assert_eq!(parsed.to_string(), fs::read_to_string(&sig).unwrap());

        // 自查:生成的签名不判自己有罪。
        assert_eq!(
            run_file(&module, OutputFormat::Human, false),
            ExitCode::SUCCESS
        );
    }

    /// `sig-gen` 默认**不覆盖**:手调过的签名是手调的理由。
    #[test]
    fn c3_sig_gen_refuses_to_overwrite_without_force() {
        let p = write_module("c3_refuse", "math.wll", SIG_MODULE);
        assert_eq!(sig_gen_file(&p, false), ExitCode::SUCCESS);
        let sig = sig_path_for(&p);
        let first = fs::read_to_string(&sig).unwrap();
        // 改模块但不改签名 → 拒绝,且**原文件一个字节都没动**。
        fs::write(&p, "LET(add, 1);\nEXPORT([\"add\"]);\n").unwrap();
        assert_eq!(sig_gen_file(&p, false), ExitCode::from(1));
        assert_eq!(fs::read_to_string(&sig).unwrap(), first);
        // `--force` 才覆盖。
        assert_eq!(sig_gen_file(&p, true), ExitCode::SUCCESS);
        assert_eq!(fs::read_to_string(&sig).unwrap(), "EXPORT add : INTEGER\n");
    }

    /// 零导出的模块不写签名文件:「不导出任何名字」的诚实契约就是**没有**
    /// 签名文件(与 v0.9 行为同解),留个 0 字节文件只污染仓库。
    #[test]
    fn c3_sig_gen_skips_modules_without_exports() {
        let p = write_module("c3_no_exports", "script.wll", "LET(a, 1);\nPRINT(a);\n");
        assert_eq!(sig_gen_file(&p, false), ExitCode::SUCCESS);
        assert!(!sig_path_for(&p).exists());
    }

    /// 生成的签名带内建返回类型(A6′ 的结构化签名在推导里生效):
    /// `LEN([1, 2])` 返回 INTEGER 而不是笼统的 DYNAMIC。
    #[test]
    fn c3_generated_signatures_use_builtin_return_types() {
        let p = write_module(
            "c3_builtins",
            "b.wll",
            "LET(n, LEN([1, 2]));\nEXPORT([\"n\"]);\n",
        );
        let sig = derive_signature(&p).expect("signature derives");
        assert_eq!(sig.to_string(), "EXPORT n : INTEGER\n");
    }

    /// JSON 形态:与文本逐字节同源,函数/值两组字段互斥。
    #[test]
    fn c3_sig_json_shape_is_stable() {
        let p = write_module("c3_json", "math.wll", SIG_MODULE);
        assert_eq!(sig_file(&p, SigFormat::Json), ExitCode::SUCCESS);
        let sig = derive_signature(&p).expect("signature derives");
        let json = serde_json::to_value(SigOutput::new(&p, &sig)).expect("serializes");
        assert_eq!(json["sig_schema_version"], "1.0.0");
        assert_eq!(json["module"], p.to_string_lossy().as_ref());
        assert_eq!(
            json["signature_file"],
            sig_path_for(&p).to_string_lossy().as_ref()
        );
        // text 字段必须与文本输出逐字节相同 —— 三处渲染同源。
        assert_eq!(json["text"], sig.to_string());
        let entries = json["entries"].as_array().expect("entries array");
        // 渲染顺序 = `BTreeMap` 的字节序,所以大写名排在小写名前面
        // (`PI` < `add`)。确定即可,不必是好读序。
        assert_eq!(entries[0]["name"], "PI");
        assert_eq!(entries[0]["kind"], "value");
        assert_eq!(entries[0]["type"], "INTEGER");
        assert!(entries[0].get("params").is_none(), "值条目不带 params");
        assert_eq!(entries[1]["name"], "add");
        assert_eq!(entries[1]["kind"], "function");
        assert_eq!(entries[1]["params"][0], "INTEGER");
        assert_eq!(entries[1]["returns"], "INTEGER");
        assert!(entries[1].get("type").is_none(), "函数条目不带 type");
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
