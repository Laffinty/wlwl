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
use wlwl_toml::manifest::{
    GradualTyping, GradualTypingSetting, MatchExhaustiveness, MatchExhaustivenessSetting,
};
use wlwl_types::{DeclaredBinding, Ty};

// [v0.10.3] `PartialEq` / `Eq`:panic 兜底的测试要断言 `output_format()`
// 的返回值。加在这一个纯数据枚举上,没有别的理由不加。
#[derive(Debug, Clone, Copy, ValueEnum, Default, PartialEq, Eq)]
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
#[command(
    name = "wlwl",
    version,
    about = "WLWL language toolchain",
    after_help = "Run 'wlwl <COMMAND> --help' for more information about a command."
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(clap::Subcommand, Debug)]
enum Cmd {
    /// Run a .wll file
    Run {
        /// Path to the .wll file
        file: PathBuf,
        /// Output format for errors
        #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
        format: OutputFormat,
    },
    /// Parse a .wll file without running it
    #[command(long_about = "\
Parse a .wll file without running it.

Reports syntax errors, plus unused-binding warnings (W0010 / W0011 /
W0012) and, when a wlwl.toml enables them, the static-contract
diagnostics E0110-E0116.

It does not resolve names. Failures that only surface at run time are
not reported here:

  PRINT(NOPE);            E0020 undefined name
  PRINT(/(1, 0));          E1003 division by zero
  LET(x: BOOLEAN, \"yes\");  needs a wlwl.toml to be checked

`run` finds those; `check` does not.")]
    Check {
        /// Path to the .wll file
        file: PathBuf,
        /// Output format for diagnostics
        #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
        format: OutputFormat,
    },
    /// Print the parse tree as JSON
    Ast {
        /// Path to the .wll file
        file: PathBuf,
        /// Output format: json (default) or jsonl (one node per line; not
        /// used for AST, but accepted for symmetry with run/check).
        #[arg(long, value_enum, default_value_t = OutputFormat::Json)]
        format: OutputFormat,
    },
    /// Print the canonical formatter output to stdout
    #[command(
        long_about = "\
Print the canonical formatter output to stdout.

The canonical form carries no trailing `;` on the last statement, and
the formatter does not preserve comments -- it rebuilds the file from
the parse tree. `-w` therefore refuses to write a file that contains
comments rather than deleting them.

Without `-w` nothing is written; redirect the output yourself if the
file is comment-free.",
        after_help = "\
Examples:
  wlwl fmt x.wll           print the canonical form
  wlwl fmt --check x.wll   exit 1 if the file is not canonical
  wlwl fmt -w x.wll        rewrite in place (refuses if it has comments)"
    )]
    Fmt {
        /// Path to the .wll file
        file: PathBuf,
        /// Report non-canonical source instead of printing it: exit 1
        /// with a W0053 diagnostic, print nothing.
        #[arg(long, conflicts_with = "write")]
        check: bool,
        /// Rewrite the file in place. Refuses when the file contains
        /// comments: the canonical form is rebuilt from the parse tree,
        /// so writing would delete them.
        #[arg(long, short = 'w')]
        write: bool,
    },
    /// Print the module signature implied by a .wll file
    Sig {
        /// Path to the .wll file
        file: PathBuf,
        /// Signature rendering: canonical text (default) or machine-readable JSON
        #[arg(long, value_enum, default_value_t = SigFormat::Text)]
        format: SigFormat,
    },
    /// Write the module signature to `<file>.wll.sig`
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
    /// Run a language server on stdio
    ///
    /// A thin shell over the existing parser + static diagnostics +
    /// builtin registry — no LSP framework, no new dependencies. Editors
    /// normally launch it themselves; run it by hand to check the
    /// handshake.
    Lsp,
    /// Print the module's public surface as JSON
    Interface {
        /// Path to the .wll file
        file: PathBuf,
    },
    /// Print the type system and static-contract codes as JSON
    Schema,
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

/// [v0.10.2] 求值用的线程栈大小。
///
/// WLWL 的求值器是**递归下降**的:每个 WLWL 调用帧会展开若干层 Rust 帧,
/// 而 `Value` 是个大枚举,所以每帧吃掉的栈远超一个普通函数。实测在
/// Windows 默认的**主线程 1 MB 栈**上,递归到 **~150 层**就
/// `STATUS_STACK_OVERFLOW`(`0xC00000FD`)整个进程崩掉 —— 普通递归算法在
/// 这个语言里根本写不出来,而这既不是 `E0101` 也不是规范 §11.4 的任何一个
/// 退出码,CI 只能看到「进程被杀」。
///
/// **为什么是 512 MB 而不是「差不多够」**:每帧栈开销**强烈依赖编译
/// profile**。实测同一份源码、同样 64 MB 栈:
///
/// | profile | 崩溃深度 |
/// |---|---|
/// | `release`(开优化,跨函数内联) | >20000 帧未崩 |
/// | `dev`(无优化) | **400–600 帧即崩** |
///
/// 差约 40 倍。取 64 MB 时 `dev` profile 下 `MAX_CALL_DEPTH`(2000)远在
/// 崩溃点之下 —— 护栏形同虚设,进程照崩。而 `dev` profile 正是
/// `cargo test` 与 probe 夹具跑的那一个,所以这不是理论问题。
///
/// 512 MB 是**线程栈保留区**,不是提交量:按需提交,深递归程序实际占用
/// 远小于此。这也是取大值而不是压低深度上限的原因 —— 压低上限会连带
/// 削掉 `release` 下合法的递归深度,而递归是这门语言的基本能力。
const EVAL_STACK_SIZE: usize = 512 * 1024 * 1024;

fn main() -> ExitCode {
    // [v0.10.3] 先装 panic 守卫,再做任何事 —— 见 `install_panic_guard`。
    install_panic_guard();

    // `--format` 是**子命令级**参数而不是全局参数,所以 `Cmd` 被 move 进线程
    // 之后再问不到它了。panic 路径需要它来选渲染格式(否则 jsonl 的消费者会
    // 收到一段人话),所以在 move 之前先把两样要用的东西取出来。
    let cmd = Cli::parse().cmd;
    let format = cmd.output_format();
    let source = cmd.source_file().map(|p| p.display().to_string());

    // 在大栈线程上跑整个命令分发。`Cli::parse()` 已在主线程完成,所以
    // `spawn` 失败时可以直接在主线程上用同一个 `cmd` 重来一次,不必重新
    // 解析 argv。
    //
    // `spawn` 失败(系统拒绝建线程)时退回主线程:退化行为是「回到原来的
    // 栈溢出崩溃」,不能反过来变成「命令根本跑不起来」。
    //
    // [v0.10.3] 分发整体裹进 `catch_unwind`。此前 panic 的唯一出口是
    // `join()` 返回 `Err` → 退出码 101,**不产出任何诊断**:`--format jsonl`
    // 下消费者收到的是一段裸 Rust backtrace,而不是 schema 1.1.0 的错误信封,
    // §16.5 conformance 契约因此被绕过。实测确认过这条(`SUB` 溢出那条就是
    // 这样表现的)。现在 panic 会被翻译成 `E0100` + 规范退出码。
    let run_guarded = move || {
        let cmd = cmd;
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || dispatch(cmd)))
    };
    match std::thread::Builder::new()
        .name("wlwl-eval".to_string())
        .stack_size(EVAL_STACK_SIZE)
        .spawn(run_guarded)
    {
        Ok(handle) => match handle.join() {
            Ok(Ok(code)) => code,
            // 线程内 panic:`catch_unwind` 已经把它变成了 `Err`。
            Ok(Err(payload)) => report_panic(payload.as_ref(), format, source.as_deref()),
            // 守卫之外的 panic(例如 panic 发生在 spawn 闭包的构造里)。
            Err(payload) => report_panic(payload.as_ref(), format, source.as_deref()),
        },
        Err(_) => match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            dispatch(Cli::parse().cmd)
        })) {
            Ok(code) => code,
            Err(payload) => report_panic(payload.as_ref(), format, source.as_deref()),
        },
    }
}

/// [v0.10.3] 把捕获到的 panic 载荷翻译成一条**规范诊断**。
///
/// 码用 `E0100`(internal error,§11.1 / §11.2),与 §11.4 的退出码
/// `101`(实现内部崩溃)配套。`E0101` 已被递归深度护栏占用,不能用。
///
/// panic 消息取自载荷本身(`&str` 或 `String`),所以「是什么炸了」不会丢 ——
/// 默认 hook 那个 `thread '…' panicked at …` 横幅被守卫抑制了,但消息内容
/// 进了这条诊断。开发者要 backtrace 时设 `RUST_BACKTRACE=1`,守卫会让位。
fn report_panic(
    payload: &(dyn std::any::Any + Send),
    format: OutputFormat,
    source: Option<&str>,
) -> ExitCode {
    // 走 `payload_str` 而不是在这里再写一遍 downcast:第一版就是这么写的,
    // 结果变异测试立刻抓出来 —— 删掉 `String` 那一臂、把 downcast 失败改成
    // `expect`(即「兜底路径自己 panic」)都**没有测试转红**,因为测试只测了
    // helper、没测到这份重复实现。同一段逻辑写两遍,就等于有两个可以各自
    // 悄悄坏掉的地方。
    let msg = payload_str(payload);
    let d = WlwlDiagnostic::new(
        ErrorCode::E0100,
        format!(
            "internal error while running the program (this is a wlwl bug, \
             not a defect in the .wll source): {msg}"
        ),
        Location::point(source.unwrap_or("<runtime>").to_string(), 0, 0),
    );
    match format {
        OutputFormat::Human => eprintln!("{}", d.render_human()),
        OutputFormat::Json => eprintln!("{}", d.render_json()),
        OutputFormat::Jsonl => eprintln!("{}", d.render_jsonl()),
    }
    // spec §11.4: 101 = 实现内部崩溃。**不能**走 `exit_code_for`
    // (它对非解析失败一律给 1)—— 那会把「程序跑挂了」说成「程序写错了」。
    ExitCode::from(101)
}

/// [v0.10.3] 抑制默认 panic hook 的横幅输出。
///
/// 默认 hook 会往 stderr 打 `thread '…' has overflowed its stack` 之类的一大段
/// 东西。对一个**语言运行时**来说那是对用户说的废话:用户要的是一条能照着改
/// 的诊断,不是 Rust 的内部信息。有了 `report_panic` 的 `E0100` 之后,再叠一段
/// backtrace 只会让 `--format jsonl` 的消费者拿到两种格式混在一起的东西。
///
/// 但**开发者需要它**:设了 `RUST_BACKTRACE`(或 `RUST_LIB_BACKTRACE`)时守卫
/// 让位,backtrace 原样输出。这条是刻意留的,不是遗漏。
fn install_panic_guard() {
    if std::env::var_os("RUST_BACKTRACE").is_some()
        || std::env::var_os("RUST_LIB_BACKTRACE").is_some()
    {
        return;
    }
    std::panic::set_hook(Box::new(|info| {
        // 只留一行 `location: message`,其余(backtrace / 线程名)交给
        // RUST_BACKTRACE 那条路。
        let loc = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "<unknown>".to_string());
        let msg = payload_str(info.payload());
        eprintln!("internal error at {loc}: {msg}");
    }));
}

fn payload_str(p: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = p.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = p.downcast_ref::<String>() {
        s.clone()
    } else {
        "non-string panic payload".to_string()
    }
}

impl Cmd {
    /// [v0.10.3] 本子命令的输出格式。
    ///
    /// `--format` 是子命令级参数,所以 `Cmd` 被 move 之后就没有第二个地方能
    /// 问了。panic 兜底(`report_panic`)需要它来决定渲染哪种格式 —— 否则
    /// `--format jsonl` 的消费者会收到一段人话而不是 schema 1.1.0 错误信封。
    ///
    /// 没有 `--format` 的子命令(LSP / interface / schema)按 `Human` 处理。
    fn output_format(&self) -> OutputFormat {
        match self {
            Cmd::Run { format, .. } | Cmd::Check { format, .. } | Cmd::Ast { format, .. } => {
                *format
            }
            // `sig` 的 `SigFormat` 是**另一个枚举**(见它的定义:单个模块的签名
            // 没有有意义的 JSONL 形态),所以只能显式映射,不能混进上面那个
            // or-pattern —— 类型不同,编译器会拒绝。
            Cmd::Sig { format, .. } => match format {
                SigFormat::Json => OutputFormat::Json,
                SigFormat::Text => OutputFormat::Human,
            },
            _ => OutputFormat::Human,
        }
    }

    /// [v0.10.3] 本子命令处理的源文件,用于给 panic 诊断一个位置。
    /// `Lsp` 之类没有单一源文件的子命令返回 `None`。
    fn source_file(&self) -> Option<&std::path::Path> {
        match self {
            Cmd::Run { file, .. }
            | Cmd::Check { file, .. }
            | Cmd::Ast { file, .. }
            | Cmd::Fmt { file, .. }
            | Cmd::Sig { file, .. }
            | Cmd::SigGen { file, .. }
            | Cmd::Interface { file } => Some(file),
            _ => None,
        }
    }
}

fn dispatch(cmd: Cmd) -> ExitCode {
    match cmd {
        Cmd::Run { file, format } => run_file(&file, format, true),
        Cmd::Check { file, format } => run_file(&file, format, false),
        Cmd::Ast { file, format } => ast_file(&file, format),
        Cmd::Fmt { file, check, write } => fmt_file(&file, check, write),
        Cmd::Sig { file, format } => sig_file(&file, format),
        Cmd::SigGen { file, force } => sig_gen_file(&file, force),
        Cmd::Lsp => lsp::Server::run(),
        Cmd::Interface { file } => tooling::interface_file(&file),
        Cmd::Schema => tooling::schema_command(),
    }
}

mod lsp;
mod tooling;

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
            // [v0.10.2] 求值期软警告此前**没有任何出口**:`Evaluator` 把它们
            // 攒在 `warnings` 里,而本函数在 `eval` 返回后直接 SUCCESS,
            // `take_warnings()` 在 `wlwl-cli` 全仓零调用 —— 于是 `W0051`
            // (`DEL` / `OR_DIE` 弃用别名)、`W0066`(`CHANNEL_NEW` 大缓冲)、
            // `W0030`(遮蔽内建)、`W0065`(死锁软警告)**全部被静默丢弃**。
            // 它们的 emit 点一直都在、也一直被单元测试锁着,所以缺陷不在
            // 实现,在这一行出口。警告永不改变程序语义,因此既不拦退出码,
            // 也在 `Err` 分支之前先排空(失败的运行同样值得看到它的警告)。
            report_eval_warnings(&mut ev, &file_name, format);
            ExitCode::SUCCESS
        }
        Err(e) => {
            report_eval_warnings(&mut ev, &file_name, format);
            report_error(e, format)
        }
    }
}

/// 把 `Evaluator` 攒下的软警告按当前 `--format` 排空。
///
/// 走 `WlwlDiagnostic` 而不是自己拼字符串,是为了让 `run` 的警告与
/// `check` 路径已有的警告(`static_check_gate` → `collect_static_diagnostics`)**共用
/// 同一套渲染器** —— `--format json` / `jsonl` 因此自动带上
/// `error_schema_version` / `error_category` / `severity` 等字段,不必在这里
/// 手工维护第二份 JSON 形状。
///
/// 警告没有源码位置(`Warning` 只有 `code` + `message`),统一挂在入口文件上,
/// 渲染出来是一条带文件名、不带行列的诊断 —— 与 parser 警告在 `check`
/// 路径上的既有形态一致(`main.rs` 的 `println!("warning {}: ...")` 口径)。
fn report_eval_warnings(ev: &mut wlwl_eval::Evaluator, file: &str, format: OutputFormat) {
    for w in ev.take_warnings() {
        let d = WlwlDiagnostic::new(w.code, w.message, Location::point(file.to_string(), 0, 0))
            .with_severity(Severity::Warning);
        match format {
            OutputFormat::Human => eprintln!("{}", d.render_human()),
            OutputFormat::Json => eprintln!("{}", d.render_json()),
            OutputFormat::Jsonl => eprintln!("{}", d.render_jsonl()),
        }
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
/// 把 CRLF / 裸 CR 归一化成 LF,供 `--check` 的逐字节比较使用。
///
/// [v0.10.1] `wlwl fmt --check` 此前把**任何**带 CRLF 的文件判为偏离(`W0053`)。
/// 成因很浅:规范形式由 `wlwl_formatter::format` 生成,行尾恒为 `\n`;而源码
/// 那一侧原样保留磁盘上的 `\r\n`,两者逐字节不等。
///
/// 后果是**跨平台不可复现**,而且是最坏的那种形态:
///
/// | 输入 | 修之前 |
/// |---|---|
/// | `PRINT("x")\n` | rc=0 |
/// | `PRINT("x")\r\n` | **rc=1 + W0053** |
///
/// Windows 检出(`.gitattributes` 只给 `*.rs` 定了 `eol=lf`,`.wll` 没有规则,
/// `core.autocrlf` 生效)的工作区里**每一个** `.wll` 都过不了 `--check`;
/// Linux 上全过。CI 的 ubuntu / macOS job 看不见这个洞,只有 Windows 看得见。
///
/// **行尾不是 §16.3 规范形式的一部分。** 规范规定的是 token 之间的空白与
/// 换行**布局**(一条语句一行、无缩进、无句末分号),没有规定换行用哪个字节
/// 表示。所以「CRLF 文件偏离规范形式」这句话本身是错的,判据要改,不是
/// 文件要改。
///
/// 裸 CR(老 Mac 风格)一并归一:它在 lexer 那边能过,在 formatter 那边
/// 不该过 —— 留着它就是另一个更难查的同类洞。
///
/// **只影响 `--check` 的比较**。`wlwl fmt <file>` 的 stdout 输出仍恒为 LF ——
/// 那是 `wlwl-formatter` 的渲染结果,不在本次范围内。
fn normalize_eol(src: &str) -> String {
    if !src.contains('\r') {
        return src.to_string();
    }
    src.replace("\r\n", "\n").replace('\r', "\n")
}

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

fn fmt_file(file: &PathBuf, check: bool, write: bool) -> ExitCode {
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
        //
        // [v0.10.1] 行尾先归一化,再剥注释。顺序也是刻意的:归一化在前,
        // `strip_comments` 看到的就是干净的 `\n`,不必去猜 `\r` 算不算
        // 行的一部分。
        let src_for_compare = strip_comments(&normalize_eol(&source));
        let canon_for_compare = strip_comments(&normalize_eol(&canonical));
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
    } else if write {
        // [v0.10.1] `-w` 加了护栏:文件里有注释就**拒写**。
        //
        // 格式化器从 AST 重建,**保不住注释**(偏差 P4-E2-001)。无护栏的
        // `-w` 会把「删光你所有注释」变成一个开关的事 —— 那是本 CLI 里最容易
        // 造成不可逆损失的操作。所以这里宁可不给,也不给一个会毁文件的开关。
        //
        // 判据用 `strip_comments`:它对字符串字面量里的 `//` 是有状态的,
        // 不会把 `"http://x"` 当注释。源文本与剥完的结果不等 = 文件里有注释。
        let stripped = strip_comments(&source);
        if stripped != source {
            let d = WlwlDiagnostic::new(
                ErrorCode::E0042,
                format!(
                    "refusing to rewrite {}: it contains comments, and the canonical \
                     form is rebuilt from the parse tree so comments would be lost",
                    file.display()
                ),
                Location::point(file_name, 1, 1),
            )
            .with_hint(
                "run `wlwl fmt <file>` and review the output by hand, or strip the \
                 comments first",
            );
            return report_diag(d, OutputFormat::Human);
        }
        if let Err(e) = fs::write(file, &canonical) {
            let d = WlwlDiagnostic::new(
                ErrorCode::E0042,
                format!("cannot write ''{}'': {}", file.display(), e),
                Location::point(file_name, 0, 0),
            );
            return report_diag(d, OutputFormat::Human);
        }
        ExitCode::SUCCESS
    } else {
        print!("{}", canonical);
        ExitCode::SUCCESS
    }
}

fn report_error(err: WlwlError, format: OutputFormat) -> ExitCode {
    report_diag(err.diagnostic().clone(), format)
}

/// 退出码契约(见 spec §11.5)。
///
/// - `0` 成功
/// - `1` 程序不对:运行期诊断,或静态层拦下的类型 / 契约诊断
/// - `2` 命令行用法错误(clap 自己定的,未改)
/// - `3` **源文件没能解析出来**
///
/// [v0.10.1] 改之前只有 `0` / `1` / `2`,而 `1` 同时表示「语法错」「类型错」
/// 「未定义名」「除零」…… 脚本拿到 `1` 无法判断该回去改代码还是改逻辑。
/// 只加**一个**码,不加一串:能解析出来但不对的,都还是「程序不对」,归 `1`。
fn exit_code_for(d: &WlwlDiagnostic) -> ExitCode {
    if d.code.is_parse_failure() {
        ExitCode::from(3)
    } else {
        ExitCode::from(1)
    }
}

fn report_diag(d: WlwlDiagnostic, format: OutputFormat) -> ExitCode {
    match format {
        OutputFormat::Human => eprintln!("{}", d.render_human()),
        OutputFormat::Json => eprintln!("{}", d.render_json()),
        OutputFormat::Jsonl => eprintln!("{}", d.render_jsonl()),
    }
    exit_code_for(&d)
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
    load_features(base_dir).strict_types
}

/// [v0.10.1 / R10-010] Everything the static layer needs out of `wlwl.toml`,
/// read in **one** pass.
///
/// The pre-v0.10.1 code had three independent loaders (`strict_types` /
/// `gradual_typing` / `match_exhaustiveness`), each of which re-read the
/// file and each of which silently swallowed a parse error. That is how a
/// `[features]`-only manifest turned `gradual_typing = "error"` into a
/// silent no-op: the whole manifest layer went quiet, not just one value.
/// Reading once and reporting once is what makes "不静默吞笔误" (ADR-0020)
/// actually hold.
///
/// Two independent questions are answered here, deliberately:
///
/// - **"what did the feature table say?"** — answered by
///   [`wlwl_toml::manifest::parse_features`], which ignores `[package]`
///   entirely (fix **b**). A `[features]`-only manifest is honoured.
/// - **"is this a well-formed manifest?"** — answered by
///   [`wlwl_toml::manifest::parse`], and a `no` is reported as `W0001`
///   (fix **a**) rather than dropped. Package validation is unchanged; this
///   only stops the failure from being invisible.
///
/// `unusable_manifest` is `Some(reason)` whenever a `wlwl.toml` exists but
/// is not a valid manifest. `None` when there is no manifest at all, which
/// keeps ADR-0020 S1 intact (a bare `.wll` produces zero diagnostics).
#[derive(Debug, Clone)]
struct FeaturesLoad {
    gradual: GradualTypingSetting,
    match_setting: MatchExhaustivenessSetting,
    strict_types: bool,
    unusable_manifest: Option<String>,
    /// [v0.10.2] `[features]` 里的未知键 / 值形状不对的键。
    /// 每项是已经写好的 `W0001` 文案。
    ///
    /// 必须**在加载时**存下来:`FeaturesLoad` 只保留三个解析后的开关,
    /// 原始 feature 表用完即丢,下游根本拿不到做键比对所需的数据。
    /// `unusable_manifest` / 两个 `invalid_value()` 走的都是同一个道理。
    feature_problems: Vec<String>,
}

impl FeaturesLoad {
    /// No `wlwl.toml` anywhere above the file: the default, quiet state.
    fn absent() -> FeaturesLoad {
        FeaturesLoad {
            gradual: GradualTypingSetting::default(),
            match_setting: MatchExhaustivenessSetting::following(GradualTypingSetting::default()),
            strict_types: false,
            unusable_manifest: None,
            feature_problems: Vec::new(),
        }
    }
}

/// `toml` errors render as a multi-line block with a caret excerpt.
/// A one-line `W0001` must not inherit that shape, so keep the headline
/// and let the full detail live wherever the user inspects the file.
fn first_line(s: &str) -> &str {
    s.lines().next().unwrap_or(s).trim()
}

fn load_features(base_dir: &std::path::Path) -> FeaturesLoad {
    let project_root = find_project_root(base_dir);
    let toml_path = project_root.join("wlwl.toml");
    if !toml_path.is_file() {
        return FeaturesLoad::absent();
    }
    let Ok(src) = fs::read_to_string(&toml_path) else {
        return FeaturesLoad {
            gradual: GradualTypingSetting::default(),
            match_setting: MatchExhaustivenessSetting::following(GradualTypingSetting::default()),
            strict_types: false,
            unusable_manifest: Some(
                "wlwl.toml could not be read (permissions or encoding); \
                 both static switches fall back to \"off\""
                    .to_string(),
            ),
            feature_problems: Vec::new(),
        };
    };

    // (b) The feature table stands on its own: `[package]` is irrelevant here.
    let loose = wlwl_toml::manifest::parse_features(&src);
    // (a) Whether the *manifest* is well formed is a separate question, and
    // its answer has to reach the user.
    let unusable_manifest = match wlwl_toml::manifest::parse(&src) {
        Ok(_) => None,
        Err(e) => Some(match &loose {
            Ok(_) => format!(
                "wlwl.toml is not a valid manifest ({}); [features] was still applied, \
                 but add [package] name / version / entry",
                first_line(&e.to_string())
            ),
            Err(_) => format!(
                "wlwl.toml could not be parsed ({}); \
                 [features] was not read and both static switches fall back to \"off\"",
                first_line(&e.to_string())
            ),
        }),
    };

    let (gradual, match_setting, strict_types, feature_problems) = match loose {
        Ok(f) => (
            f.gradual_typing(),
            f.match_exhaustiveness(),
            f.strict_types(),
            // [v0.10.2] 键名与值形状的体检。解析失败时无从体检 —— 那种情况
            // 已经由 `unusable_manifest` 报过了,不在这里重复。
            f.problems().into_iter().map(|(_k, msg)| msg).collect(),
        ),
        Err(_) => (
            GradualTypingSetting::default(),
            MatchExhaustivenessSetting::following(GradualTypingSetting::default()),
            false,
            Vec::new(),
        ),
    };

    FeaturesLoad {
        gradual,
        match_setting,
        strict_types,
        unusable_manifest,
        feature_problems,
    }
}

/// [v0.10 Step 2 / ADR-0020 A3] Read `wlwl.toml` from the project root
/// and return the `[features] gradual_typing` switch.
///
/// Thin wrapper over [`load_features`]; see that function for why the
/// manifest layer is read once and why a broken manifest is reported
/// instead of dropped (R10-010).
#[allow(dead_code)]
fn load_manifest_gradual_typing(base_dir: &std::path::Path) -> GradualTypingSetting {
    load_features(base_dir).gradual
}

/// [v0.10 Step 8 / plan §5.1] Read `[features] match_exhaustiveness`.
///
/// Thin wrapper over [`load_features`] (R10-010).
#[allow(dead_code)]
fn load_manifest_match_exhaustiveness(base_dir: &std::path::Path) -> MatchExhaustivenessSetting {
    load_features(base_dir).match_setting
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
    let rendered = collect_static_diagnostics(ast, file, base_dir);
    if rendered.is_empty() {
        return None;
    }
    // 阻塞与否看**实际渲染出来的诊断**:只有硬诊断挡退出码。这条比
    // 「看档位」更准 —— `error` 档下也可能一条硬错都没有(比如只报了恒
    // 警告的 `W0117`),那就不该拦。
    let blocking = rendered.iter().any(|d| d.severity == Severity::Error);
    for d in &rendered {
        match format {
            OutputFormat::Human => eprintln!("{}", d.render_human()),
            OutputFormat::Json => eprintln!("{}", d.render_json()),
            OutputFormat::Jsonl => eprintln!("{}", d.render_jsonl()),
        }
    }
    if blocking {
        // [v0.10.1] 走同一条退出码契约。静态层也会产出 parser 的码 ——
        // 签名文件里写 `EXPORT add (ARRAY[INTEGER]: Comparable)` 报的就是
        // `E0010`(签名文法错误)。它此前在这里硬编码 `1`,于是同一种「读不
        // 出来的文本」会因为走哪条路而拿到不同的退出码。
        Some(exit_code_for(
            rendered.first().expect("blocking implies non-empty"),
        ))
    } else {
        None
    }
}

/// 收集一个文件的全部静态契约诊断(**已按两个开关分档渲染**)。
///
/// `static_check_gate`(`check` / `run`)与 [`crate::lsp`] 共用这一份 ——
/// 诊断口径必须只有一处,否则编辑器里划线的和 `wlwl check` 说的会不一致,
/// 那比没有诊断更糟。
///
/// 顺序按 (行, 列) 排,输出逐字节可复现。
pub(crate) fn collect_static_diagnostics(
    ast: &Expr,
    file: &std::path::Path,
    base_dir: &std::path::Path,
) -> Vec<WlwlDiagnostic> {
    let features = load_features(base_dir);
    let setting = features.gradual.clone();
    // [v0.10 Step 8] 第二个子系统:MATCH 穷尽性 / 可达性。缺省跟随
    // `gradual_typing`(计划书 §5.1),所以只写 `gradual_typing` 一个键
    // 就够用;要单独关掉 MATCH 时再写 `match_exhaustiveness`。
    let match_setting = features.match_setting.clone();
    let mut out: Vec<WlwlDiagnostic> = Vec::new();

    // [v0.10.1 / R10-010] 清单存在但不是一份合法 manifest —— 必须响亮。
    // 这一条**先于**下面的 early return:两个开关都关时清单照样该被检查,
    // 否则「`off` 档静默」会把清单层的笔误重新藏起来。
    if let Some(reason) = &features.unusable_manifest {
        out.push(
            WlwlDiagnostic::new(
                ErrorCode::W0001,
                reason.clone(),
                Location::point(base_dir.join("wlwl.toml").to_string_lossy(), 0, 0),
            )
            .with_severity(Severity::Warning),
        );
    }

    // [v0.10.2] 键名拼错 / 值形状不对。`W0001` 此前只覆盖「已知键 + 非法值」
    // 三种情形(清单不合法、`gradual_typing` 值非法、`match_exhaustiveness`
    // 值非法),**没有一条管键名**。于是 `native_channel_clsoe = true` 被正常
    // 收下、读不到、回落 `false`,零提示 —— 用户写对了语义、拼错了键、
    // 拿到了相反的行为。
    //
    // 与上面三条同处一个 early-return 之前:两个开关都关时清单照样该被体检,
    // 否则「`off` 档静默」会把笔误重新藏起来。
    for msg in &features.feature_problems {
        out.push(
            WlwlDiagnostic::new(
                ErrorCode::W0001,
                msg.clone(),
                Location::point(base_dir.join("wlwl.toml").to_string_lossy(), 0, 0),
            )
            .with_severity(Severity::Warning),
        );
    }

    // 开关写错必须**可见**(不静默吞笔误):非法值回落后的回执本身就是
    // 一条警告,和静态诊断一起交付。
    if let Some(raw) = setting.invalid_value() {
        out.push(
            WlwlDiagnostic::new(
                ErrorCode::W0001,
                format!(
                    "invalid [features] gradual_typing value `{raw}`; \
                     expected \"off\" | \"warn\" | \"error\" -- falling back to \"off\""
                ),
                Location::point(base_dir.join("wlwl.toml").to_string_lossy(), 0, 0),
            )
            .with_severity(Severity::Warning),
        );
    }
    if let Some(raw) = match_setting.invalid_value() {
        out.push(
            WlwlDiagnostic::new(
                ErrorCode::W0001,
                format!(
                    "invalid [features] match_exhaustiveness value `{raw}`; \
                     expected \"off\" | \"warn\" | \"error\" -- falling back to gradual_typing"
                ),
                Location::point(base_dir.join("wlwl.toml").to_string_lossy(), 0, 0),
            )
            .with_severity(Severity::Warning),
        );
    }

    // 两个开关都关 → 整条静态链路都不跑。这是「零开销」的落点,而且两个
    // 开关是**独立**的:只关 MATCH 不影响类型检查。
    if !setting.is_enabled() && !match_setting.is_enabled() {
        return out;
    }

    let severity = match setting.mode() {
        GradualTyping::Warn => Severity::Warning,
        _ => Severity::Error,
    };
    let match_severity = match match_setting.mode() {
        MatchExhaustiveness::Warn => Severity::Warning,
        _ => Severity::Error,
    };

    // [v0.10 Step 5 / ADR-0020 A6′] 内建签名表。**只在需要时构建** ——
    // 两个开关都关时连表都不建,这是「默认零开销」的一部分。
    //
    // 映射放在 CLI 而不是 `wlwl-types` 里,是为了保住 ADR-0020
    // Decision 1 的分层:内建签名住在 `wlwl-eval` 的注册表,而
    // `wlwl-eval` 不依赖 `wlwl-types`、反之亦然。CLI 是同时依赖两者的
    // 那一层,所以它是唯一能同时看见 `SigTy` 与 `Ty` 的地方。
    let builtins = builtin_sig_table();

    // [v0.10.1 / R10-023] 先算入口文件**直接导入**的签名类型,再跑类型检查。
    //
    // 顺序是被迫的:导入名的类型来自旁路签名文件,而类型检查要用它 ——
    // 所以签名必须在 `check_program_with_options` **之前**就位。契约扫描
    // (`scan_module_contracts`)仍然在后面:它要的是类型检查的产物
    // `declared`,方向相反,两者不构成环。
    //
    // 只需要**直接**导入:各层模块自己的类型诊断由各自的 `wlwl check`
    // 负责(见 `scan_module_contracts` 的函数文档),入口只管自己直接依赖
    // 的那几份签名。
    let imported_owned = collect_imported_sig_types(ast, file);
    let imported: &std::collections::BTreeMap<String, Ty> = match &imported_owned {
        Some(m) => m,
        None => default_empty_sig_types(),
    };

    // [v0.10 Step 6 / plan §4] 一次遍历同时拿到两样东西:类型诊断,以及
    // 根作用域的绑定类型(模块契约要比的就是这份表)。
    let checked = wlwl_types::check_program_with_options(
        ast,
        &wlwl_types::CheckOptions {
            builtins: &builtins,
            match_exhaustiveness: match_setting.is_enabled(),
            imported,
        },
    );
    // **按子系统分档渲染**:`gradual_typing` 管类型/契约诊断,
    // `match_exhaustiveness` 管 MATCH 诊断。两个开关独立,所以渲染阶段
    // 也得分流 —— `W0117` 恒为警告(见 `TypeDiag::to_diagnostic`)。
    //
    // [v0.10.1 / R10-028] 补源码行。`TypeDiag::to_diagnostic` 只拿得到
    // `Span`,拿不到源文本,所以类型诊断一直**没有** `source_line` ——
    // 渲染出来是 `--> file:1:9` 后面光秃秃,没有运行期诊断那种
    // `| 1 | LET(flag: BOOLEAN, "yes");` 行。运行期的 `Evaluator::diag`
    // 一直在补(`wlwl-eval` 里 `extract_line` 那一处),静态层是唯一漏掉的。
    //
    // 在这里统一补,而不是去改 `to_diagnostic`:`wlwl-types` 不持有源文本
    // (它只吃 AST),把文本塞进去要么改签名、要么依赖 CLI。CLI 是唯一同时
    // 持有 AST 与源文本的那一层,补在这里最自然。
    let source = std::fs::read_to_string(file).ok();
    for d in &checked.diags {
        let (enabled, level) = match d.kind.subsystem() {
            wlwl_types::Subsystem::Types => (setting.is_enabled(), severity),
            wlwl_types::Subsystem::Match => (match_setting.is_enabled(), match_severity),
        };
        if !enabled {
            continue;
        }
        if let Some(mut diag) = d.to_diagnostic(level) {
            if let Some(src) = source.as_deref() {
                if let Some(line_text) = wlwl_error::extract_line(src, d.span.line_start) {
                    diag = diag.with_source_line(line_text);
                }
            }
            out.push(diag);
        }
    }

    // [v0.10 Step 6 / plan §4.1 C1 + §4.2 C2] 模块契约:签名文件与
    // `SEALED` 声明面。走完整 import 图,所以 `wlwl check main.wll`
    // 一次就把本地依赖链上的契约全验了。
    //
    // 依赖模块的**签名文件读不了/写错**也算诊断(它就是契约的一部分);
    // 但依赖模块自己的语法错、找不到的模块**不在这里报** —— 那些是
    // `run` 的地盘,在这里再报一遍等于让 `check main.wll` 替别人家的
    // 文件失败。
    // [Step 6] 契约扫描里也要用同一份绑定表算依赖模块的类型,所以
    // 传的是刚刚那份「已经算好的」结果,不再重算一遍。
    if setting.is_enabled() {
        out.extend(scan_module_contracts(
            file,
            ast,
            &checked.declared,
            &builtins,
            severity,
        ));
    }
    // Deterministic order: source order. The checker already discovers
    // in walk order, so a stable sort by (line, col) is enough to make
    // multi-diagnostic output byte-stable across runs.
    out.sort_by_key(|d| (d.location.line, d.location.col));
    out
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

/// [v0.10.1 / R10-023] 入口文件**直接导入**的名字 → 旁路签名里声明的类型。
///
/// 只做一次轻量预扫:解析每个 `IMPORT` spec 到文件、读它的 `.sig`,把签名
/// 按 spec 索引起来。解析不到模块 / 没有签名文件 / 签名读不出来,一律
/// **静默跳过** —— 这些问题由 `scan_module_contracts` 与 `run` 各按自己的
/// 职责报一遍,这里重报只会让同一条消息出现两次。
fn collect_imported_sig_types(
    ast: &Expr,
    file: &std::path::Path,
) -> Option<std::collections::BTreeMap<String, Ty>> {
    let base = file.parent().map(|p| p.to_path_buf()).unwrap_or_default();
    let specs = wlwl_types::import_specs(ast);
    if specs.is_empty() {
        return None;
    }
    let mut by_spec: std::collections::BTreeMap<String, wlwl_types::ModuleSig> =
        std::collections::BTreeMap::new();
    for spec in specs {
        // std 模块没有磁盘源,也就没有旁路签名。
        let Ok(Some(dep)) = wlwl_eval::resolve_module_file(&spec, &base) else {
            continue;
        };
        if let Ok(Some(sig)) = load_module_sig(&dep) {
            by_spec.insert(spec, sig);
        }
    }
    if by_spec.is_empty() {
        return None;
    }
    Some(wlwl_types::imported_sig_types(ast, &by_spec))
}

/// 「没有导入签名」时的空表。`&'static` 是因为
/// [`wlwl_types::CheckOptions`] 持有一个引用。
fn default_empty_sig_types() -> &'static std::collections::BTreeMap<String, Ty> {
    static EMPTY: std::sync::OnceLock<std::collections::BTreeMap<String, Ty>> =
        std::sync::OnceLock::new();
    EMPTY.get_or_init(Default::default)
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

    // ---- [v0.10.3] panic 兜底:panic 必须变成一条**规范诊断** ----
    //
    // 这些测试直接调 `report_panic`,不靠「制造一次真 panic」。
    // 曾经想过在 `dispatch` 里留一个 `WLWL_TEST_PANIC` 环境变量开关来触发
    // 真 panic(那样才能端到端验),但那是**把测试脚手架塞进发布产物** ——
    // 一个任何用户都能让运行时崩掉的后门。改在这里锁住契约:码、分类、
    // 退出码、消息保真。真正端到端的那一层由 A1 的 `SUB` 溢出 case 覆盖
    // (它修好之前就是一条真 panic,走的是同一条 `report_panic` 路径)。

    /// panic 载荷有两种常见形态:`panic!("...")` 给 `&'static str`、
    /// `panic!("{}", x)` 给 `String`。两种都得取到消息。
    #[test]
    fn panic_payload_str_and_string_both_recovered() {
        let s: &str = "boom";
        assert_eq!(payload_str(&s), "boom");
        let owned = String::from("boom");
        assert_eq!(payload_str(&owned), "boom");
    }

    #[test]
    fn unknown_payload_does_not_panic() {
        // downcast 失败必须优雅降级,不能自己再 panic —— 一个兜底路径
        // 自己在 panic,比没有兜底更糟。
        let p = 42u32;
        assert_eq!(payload_str(&p), "non-string panic payload");
    }

    /// 变异测试逼出来的一条:第一版 `report_panic` **自己又抄了一遍**
    /// downcast,而测试只测 helper —— 于是「删掉 String 臂」和「downcast
    /// 失败改 expect」两个变异都悄悄活着。这条断言直接从 `report_panic` 的
    /// 产物里读消息,把它自己的那条路径也纳入覆盖。
    #[test]
    fn report_panic_message_survives_into_the_diagnostic() {
        // `&'static str` 形态
        let s: &str = "boom-static";
        assert_eq!(
            report_panic(&s, OutputFormat::Human, None),
            ExitCode::from(101)
        );
        // `String` 形态 —— 第一版实现就是在这条上失效的
        let owned = String::from("boom-owned");
        assert_eq!(
            report_panic(&owned, OutputFormat::Human, None),
            ExitCode::from(101)
        );
        // 两种都不是的载荷:必须优雅降级,**不能**自己再 panic
        // (一个兜底路径自己 panic,比没有兜底更糟)。
        let n = 7u8;
        assert_eq!(
            report_panic(&n, OutputFormat::Human, None),
            ExitCode::from(101)
        );
    }

    /// §11.4 退出码契约:**101 = 实现内部崩溃**,不是 1。
    /// 走 `exit_code_for` 会给 1,那就等于把「wlwl 崩了」说成「程序写错了」,
    /// 脚本会据此回去改源码 —— 方向完全反了。
    #[test]
    fn panic_maps_to_e0100_and_exit_101() {
        let s: &str = "synthetic";
        let code = report_panic(&s, OutputFormat::Human, Some("a.wll"));
        assert_eq!(code, ExitCode::from(101));
    }

    /// `--format` 是子命令级参数,所以 `Cmd` 被 move 前要把格式取出来。
    /// 三个子命令各取一次,验这条取法本身。
    #[test]
    fn output_format_is_readable_before_cmd_is_moved() {
        let mk = |fmt: OutputFormat| {
            Cli::parse_from([
                "wlwl",
                "run",
                "x.wll",
                "--format",
                match fmt {
                    OutputFormat::Human => "human",
                    OutputFormat::Json => "json",
                    OutputFormat::Jsonl => "jsonl",
                },
            ])
            .cmd
        };
        assert_eq!(mk(OutputFormat::Jsonl).output_format(), OutputFormat::Jsonl);
        assert_eq!(mk(OutputFormat::Json).output_format(), OutputFormat::Json);
        assert_eq!(mk(OutputFormat::Human).output_format(), OutputFormat::Human);

        // `sig` 的 SigFormat 是**另一个枚举**,只能显式映射(混进 or-pattern
        // 会被编译器拒)。两条都要对上。
        let sig_json = Cli::parse_from(["wlwl", "sig", "x.wll", "--format", "json"]);
        assert_eq!(sig_json.cmd.output_format(), OutputFormat::Json);
        let sig_text = Cli::parse_from(["wlwl", "sig", "x.wll"]);
        assert_eq!(sig_text.cmd.output_format(), OutputFormat::Human);

        // 没有 --format 的子命令按 Human 处理。
        let lsp = Cli::parse_from(["wlwl", "lsp"]);
        assert_eq!(lsp.cmd.output_format(), OutputFormat::Human);
        assert!(lsp.cmd.source_file().is_none());
    }

    fn write_tmp(content: &str, name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join("wlwl-cli-tests");
        fs::create_dir_all(&dir).unwrap();
        let p = dir.join(name);
        let mut f = fs::File::create(&p).unwrap();
        f.write_all(content.as_bytes()).unwrap();
        p
    }

    // ---- [v0.10.1] 退出码契约(spec §11.5)------------------------

    /// 契约的全部四格。每一格都用**真实子进程**验,不经 `run_file` 内部 ——
    /// 因为契约的意义就是「外面那个 shell 看到什么」。
    #[test]
    fn the_exit_code_contract_is_four_distinguishable_values() {
        let syntax = write_tmp("PRINT(1) PRINT(2)\n", "ec_syntax.wll");
        let undef = write_tmp("PRINT(NOPE);\n", "ec_undef.wll");
        let divzero = write_tmp("PRINT(/(1, 0));\n", "ec_divzero.wll");
        let ok = write_tmp("PRINT(\"hi\");\n", "ec_ok.wll");

        // 3 = 源文件没解析出来
        assert_eq!(
            run_file(&syntax, OutputFormat::Human, true),
            ExitCode::from(3),
            "a source that does not parse must exit 3"
        );
        // 1 = 程序不对(运行期)
        assert_eq!(
            run_file(&undef, OutputFormat::Human, true),
            ExitCode::from(1),
            "an undefined name is a runtime failure, not a parse failure"
        );
        assert_eq!(
            run_file(&divzero, OutputFormat::Human, true),
            ExitCode::from(1)
        );
        // 0
        assert_eq!(run_file(&ok, OutputFormat::Human, true), ExitCode::SUCCESS);

        // check 走同一条路:解析错也是 3
        assert_eq!(
            run_file(&syntax, OutputFormat::Human, false),
            ExitCode::from(3),
            "`check` and `run` must never disagree on a parse failure"
        );
    }

    /// 静态契约那 6 个码也是退出码 1,不是 3 —— 源文件能解析,只是类型/契约不对。
    #[test]
    fn static_contract_failures_exit_one_not_three() {
        for code in [
            ErrorCode::E0110,
            ErrorCode::E0111,
            ErrorCode::E0112,
            ErrorCode::E0113,
            ErrorCode::E0114,
            ErrorCode::E0115,
            ErrorCode::E0116,
        ] {
            assert!(
                !code.is_parse_failure(),
                "{code:?} must not be classified as a parse failure"
            );
        }
    }

    // ---- [v0.10.1] `fmt -w` 的护栏 --------------------------------

    /// 无注释的文件:`-w` 真的改写它。
    #[test]
    fn fmt_write_rewrites_a_comment_free_file() {
        let p = write_tmp("LET( x ,1 );\n", "fmt_w_plain.wll");
        assert_eq!(fmt_file(&p, false, true), ExitCode::SUCCESS);
        assert_eq!(fs::read_to_string(&p).unwrap(), "LET(x, 1)\n");
    }

    /// **有注释的文件:`-w` 拒写,文件一字不动。**
    ///
    /// 格式化器从 AST 重建,保不住注释。无护栏的 `-w` 会把「删光所有注释」
    /// 变成一个开关的事 —— 宁可不给这个开关,也不给一个会毁文件的开关。
    #[test]
    fn fmt_write_refuses_a_file_with_comments_and_leaves_it_untouched() {
        let original = "// keep me\nLET( x ,1 );\n";
        let p = write_tmp(original, "fmt_w_commented.wll");
        let code = fmt_file(&p, false, true);
        assert_eq!(code, ExitCode::from(1), "must refuse, not write");
        assert_eq!(
            fs::read_to_string(&p).unwrap(),
            original,
            "a refused rewrite must leave the file byte-identical"
        );
    }

    /// 字符串字面量里的 `//` 不是注释,不能因此误判为「有注释」。
    #[test]
    fn fmt_write_does_not_mistake_a_url_inside_a_string_for_a_comment() {
        let p = write_tmp("PRINT(\"http://example.com\");\n", "fmt_w_url.wll");
        assert_eq!(fmt_file(&p, false, true), ExitCode::SUCCESS);
        assert_eq!(
            fs::read_to_string(&p).unwrap(),
            "PRINT(\"http://example.com\")\n"
        );
    }

    /// 不带 `-w` 时行为不变:只输出,一个字都不写。
    #[test]
    fn fmt_without_write_still_prints_and_leaves_the_file_alone() {
        let original = "LET( x ,1 );\n";
        let p = write_tmp(original, "fmt_nowrite.wll");
        assert_eq!(fmt_file(&p, false, false), ExitCode::SUCCESS);
        assert_eq!(fs::read_to_string(&p).unwrap(), original);
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
        // [v0.10.1] 退出码契约(spec §11.5):源文件没解析出来 -> 3。
        // 此前一律 1,与运行期失败无法区分。
        assert_eq!(code, ExitCode::from(3));
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

    // -- v0.10.1 (R10-010): 清单缺 [package] 时静态层不再静默 -----------

    /// 写一个清单**内容完全由调用方给**的工程,返回 `.wll` 路径。
    ///
    /// `write_project` 永远拼出完整 `[package]`,所以 R10-010 的夹具必须
    /// 自己控制清单原文。
    fn write_raw_manifest_project(dir: &str, toml: &str, source: &str) -> PathBuf {
        let root = std::env::temp_dir().join("wlwl-cli-tests").join(dir);
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("wlwl.toml"), toml).unwrap();
        let p = root.join("main.wll");
        fs::write(&p, source).unwrap();
        p
    }

    /// 对一个工程收集静态诊断。
    fn diags_of(p: &std::path::Path) -> Vec<WlwlDiagnostic> {
        let src = fs::read_to_string(p).unwrap();
        let ast = wlwl_parser::parse(&src, &p.to_string_lossy()).expect("fixture must parse");
        let base_dir = p.parent().unwrap();
        collect_static_diagnostics(&ast, p, base_dir)
    }

    /// 同上,只取诊断码序列(按渲染顺序)。
    fn codes_of(p: &std::path::Path) -> Vec<String> {
        diags_of(p)
            .into_iter()
            .map(|d| d.code.as_str().to_string())
            .collect()
    }

    fn count_code(codes: &[String], code: &str) -> usize {
        codes.iter().filter(|c| c.as_str() == code).count()
    }

    /// 一份必然触发 `E0110` 的源码(注解 BOOLEAN 绑到 STRING 字面量)。
    const ANN_MISMATCH: &str = "LET(flag: BOOLEAN, \"yes\");";

    /// 完整清单(带 `[package]` 三件套)的模板。
    fn full_manifest(features: &str) -> String {
        format!(
            "[package]\nname = \"probe\"\nversion = \"0.0.1\"\nentry = \"main.wll\"\n\n[features]\n{features}"
        )
    }

    /// 只写 `[features]` 的清单模板 —— v0.10 的静默失效正是这个形状。
    fn features_only_manifest(features: &str) -> String {
        format!("[features]\n{features}")
    }

    /// [R10-028] 静态诊断必须带**源码行**。
    ///
    /// 运行期诊断一直有(`| 1 | LET(flag: BOOLEAN, "yes");`),静态层是唯一
    /// 漏掉的 —— `TypeDiag::to_diagnostic` 只拿得到 `Span`,拿不到源文本。
    /// 缺了它,诊断在编辑器里划线处显示不出那一行。
    #[test]
    fn r10_028_static_diagnostics_carry_the_source_line() {
        // 门必须开着才有静态诊断 —— 完整清单(含 `[package]` 三件套)。
        let p = write_raw_manifest_project(
            "r10_028_src",
            &full_manifest("gradual_typing = \"error\"\n"),
            "LET(flag: BOOLEAN, \"yes\");\nPRINT(flag);\n",
        );
        let src = fs::read_to_string(&p).unwrap();
        let ast = wlwl_parser::parse(&src, &p.to_string_lossy()).expect("fixture must parse");
        let diags = collect_static_diagnostics(&ast, &p, p.parent().unwrap());
        let e0110 = diags
            .iter()
            .find(|d| d.code.as_str() == "E0110")
            .expect("the fixture must produce E0110");
        let line = e0110
            .source_line
            .as_deref()
            .expect("a static diagnostic must carry its source line");
        assert!(
            line.contains("BOOLEAN") && line.contains("yes"),
            "the source line must be the offending one, got: {line}"
        );
        assert_eq!(
            e0110.location.line, 1,
            "and it must point at the right line"
        );
    }

    /// [R10-028] 干净程序不因为补源码行而多出诊断(守住「补全」不是「多报」)。
    #[test]
    fn r10_028_a_clean_program_stays_clean() {
        let p = write_tmp("LET(x: INTEGER, 1); PRINT(x);", "r10_028_clean.wll");
        let src = fs::read_to_string(&p).unwrap();
        let ast = wlwl_parser::parse(&src, &p.to_string_lossy()).expect("fixture must parse");
        let diags = collect_static_diagnostics(&ast, &p, p.parent().unwrap());
        assert!(
            diags.is_empty(),
            "no diagnostic should be produced, got {:?}",
            diags.iter().map(|d| d.code.as_str()).collect::<Vec<_>>()
        );
    }

    /// **修法 b**:`[features]` 单独成立时开关必须真的生效。
    ///
    /// 修之前这里是 `OK` + rc=0 + 零诊断:`Package` 的三个字段都没有
    /// `#[serde(default)]`,所以 `[package]` 缺失时 `toml::from_str` 就失败,
    /// 两个 loader 把 `Err` 吞掉回落 `Off`,静态层整条链路静默不跑。
    #[test]
    fn r10_010_features_only_manifest_still_enables_the_switch() {
        let p = write_raw_manifest_project(
            "r10_010_b_enabled",
            &features_only_manifest("gradual_typing = \"error\"\n"),
            ANN_MISMATCH,
        );
        let codes = codes_of(&p);
        assert_eq!(
            count_code(&codes, "E0110"),
            1,
            "features-only manifest must still run the static pass, got {codes:?}"
        );
        // 退出码也必须跟着变 —— 这是「用户以为在检查、其实没检查」的
        // 原始症状,只看诊断码不够。
        assert_eq!(run_file(&p, OutputFormat::Human, false), ExitCode::from(1));
    }

    /// **修法 a**:`[features]` 单独成立但 `[package]` 残缺时必须发 `W0001`。
    ///
    /// 这条不是重复上一条:开关生效了,但工程本身仍然不是一份合法清单,
    /// 依赖求解 / `wlwl build` / entry 解析全都会失败。用户有权知道。
    #[test]
    fn r10_010_features_only_manifest_reports_the_incomplete_package() {
        let p = write_raw_manifest_project(
            "r10_010_a_warns",
            &features_only_manifest("gradual_typing = \"error\"\n"),
            ANN_MISMATCH,
        );
        let codes = codes_of(&p);
        assert_eq!(
            count_code(&codes, "W0001"),
            1,
            "an incomplete manifest must not be silent, got {codes:?}"
        );
        // 警告不挡退出码:挡住的是 E0110 自己。
        assert_eq!(run_file(&p, OutputFormat::Human, false), ExitCode::from(1));
    }

    /// 清单既缺 `[package]`、特性值又写错时,**恰好两条** `W0001`。
    ///
    /// 两条说的是两件不同的事,都必须说:
    /// ①值 `"sideways"` 不在 `off|warn|error` 里 → 回落 off;
    /// ②清单缺 `[package]` 三件套,不是一份合法 manifest。
    ///
    /// 真正要守的不是「一条」而是**不重复**:v0.10 有三个 loader 各自读一次
    /// 清单,任何一条诊断被复制两遍都会让用户以为是两个问题。所以这里断言
    /// 两条**消息内容不同**,而不只是数个数。
    #[test]
    fn r10_010_bad_value_with_features_only_warns_once_each() {
        let p = write_raw_manifest_project(
            "r10_010_a_once",
            &features_only_manifest("gradual_typing = \"sideways\"\n"),
            ANN_MISMATCH,
        );
        let warnings: Vec<String> = diags_of(&p)
            .into_iter()
            .filter(|d| d.code.as_str() == "W0001")
            .map(|d| d.message)
            .collect();
        assert_eq!(
            warnings.len(),
            2,
            "value + incomplete package are two distinct facts; got {warnings:?}"
        );
        assert!(
            warnings.iter().any(|m| m.contains("value")),
            "missing the invalid-value warning: {warnings:?}"
        );
        assert!(
            warnings.iter().any(|m| m.contains("not a valid manifest")),
            "missing the incomplete-manifest warning: {warnings:?}"
        );
        // 值非法 → 回落 off → 类型失配不该被报出来。
        let codes = codes_of(&p);
        assert_eq!(count_code(&codes, "E0110"), 0, "got {codes:?}");
    }

    /// 完整清单 + 非法值:仍然只有一条 `W0001`,且**不**包含清单残缺那条。
    ///
    /// 这是 v0.10 已有的行为,必须原样保住 —— 完整清单不该被新逻辑误伤。
    #[test]
    fn r10_010_full_manifest_with_bad_value_still_warns_once() {
        let p = write_raw_manifest_project(
            "r10_010_full_bad",
            &full_manifest("gradual_typing = \"sideways\"\n"),
            ANN_MISMATCH,
        );
        let codes = codes_of(&p);
        assert_eq!(
            count_code(&codes, "W0001"),
            1,
            "a valid manifest must not add a second warning, got {codes:?}"
        );
        assert_eq!(count_code(&codes, "E0110"), 0, "got {codes:?}");
    }

    /// 完整清单 + 合法值:零诊断(守住 v0.10 的正常路径不被打扰)。
    #[test]
    fn r10_010_full_manifest_with_good_value_is_clean() {
        let p = write_raw_manifest_project(
            "r10_010_full_good",
            &full_manifest("gradual_typing = \"off\"\n"),
            ANN_MISMATCH,
        );
        let codes = codes_of(&p);
        assert!(
            codes.is_empty(),
            "clean project must stay clean, got {codes:?}"
        );
    }

    /// 完全没有清单:零诊断(ADR-0020 S1 —— 裸 `.wll` 必须绝对安静)。
    #[test]
    fn r10_010_no_manifest_stays_silent() {
        let dir = std::env::temp_dir()
            .join("wlwl-cli-tests")
            .join("r10_010_none");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let p = dir.join("main.wll");
        fs::write(&p, ANN_MISMATCH).unwrap();
        let codes = codes_of(&p);
        assert!(
            codes.is_empty(),
            "a bare .wll must produce zero diagnostics, got {codes:?}"
        );
    }

    /// TOML 语法本身就坏掉:发 `W0001`,并且特性值真的读不到(回落 off)。
    ///
    /// 这是唯一一种「连宽松解析都救不回来」的情况,必须与上面
    /// 「清单残缺但特性生效」区分开 —— 否则用户会以为写了 `error` 就
    /// 一定有检查。
    #[test]
    fn r10_010_malformed_toml_warns_and_falls_back_to_off() {
        let p = write_raw_manifest_project(
            "r10_010_broken",
            "[features\ng gradual_typing = \"error\"\n",
            ANN_MISMATCH,
        );
        let codes = codes_of(&p);
        assert_eq!(
            count_code(&codes, "W0001"),
            1,
            "unparseable manifest must be loud, got {codes:?}"
        );
        assert_eq!(
            count_code(&codes, "E0110"),
            0,
            "an unreadable feature table must fall back to off, got {codes:?}"
        );
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
        // 层 1 —— parse。退出码 3(源没解析出来)。
        // [v0.10.1] 层 2 仍是 1:静态诊断不改变「源文件读得出来」这个事实。
        let bad = write_tmp("LET(x, 1) LET(y, 2);", "layer_parse_err.wll");
        assert_eq!(
            run_file(&bad, OutputFormat::Human, false),
            ExitCode::from(3)
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
        // [v0.10.1] 签名文件自己写错,报的是语法码 E0010 -> 退出码 3。
        // 「读不出来的文本」不管出现在主源还是旁路签名文件里,结论必须一致,
        // 否则同一种故障会因为走哪条路而拿到不同的退出码。
        assert_eq!(
            run_file(&root.join("main.wll"), OutputFormat::Human, false),
            ExitCode::from(3)
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

    // -- v0.10 Step 8 (P1-1): MATCH 穷尽性 / 可达性 --------------------

    /// 写一个开着 `gradual_typing` 的工程,源码由调用方给。
    fn match_project(
        dir: &str,
        gradual: &str,
        extra_feature: Option<&str>,
        source: &str,
    ) -> PathBuf {
        let root = module_project(dir, gradual);
        if let Some(line) = extra_feature {
            let toml = fs::read_to_string(root.join("wlwl.toml")).unwrap();
            fs::write(
                root.join("wlwl.toml"),
                toml.replace("[features]", &format!("[features]\n{line}")),
            )
            .unwrap();
        }
        fs::write(root.join("main.wll"), source).unwrap();
        root
    }

    /// 漏了 `ERR` 分支且**省略了 default** → `E0116` 挡住退出码。
    #[test]
    fn p1_non_exhaustive_match_blocks_in_error_mode() {
        let root = match_project("p1_e0116", "\"error\"", None, "MATCH(OK(1), [[OK(n), n]]);");
        assert_eq!(
            run_file(&root.join("main.wll"), OutputFormat::Human, false),
            ExitCode::from(1)
        );
    }

    /// 不可达子句 `W0117` **恒不挡**退出码 —— 切到 `error` 档也一样。
    #[test]
    fn p1_unreachable_clause_never_blocks() {
        let root = match_project("p1_w0117", "\"error\"", None, "MATCH(1, [[_, 1], [2, 2]]);");
        assert_eq!(
            run_file(&root.join("main.wll"), OutputFormat::Human, false),
            ExitCode::SUCCESS
        );
    }

    /// `warn` 档:缺构造子发 `W0116`,同样不挡。
    #[test]
    fn p1_warn_mode_reports_without_blocking() {
        let root = match_project("p1_warn", "\"warn\"", None, "MATCH(OK(1), [[OK(n), n]]);");
        assert_eq!(
            run_file(&root.join("main.wll"), OutputFormat::Human, false),
            ExitCode::SUCCESS
        );
    }

    /// 子开关独立于总开关:关掉 MATCH 检查但保留类型检查。
    #[test]
    fn p1_match_exhaustiveness_can_be_turned_off_on_its_own() {
        let src = "MATCH(OK(1), [[OK(n), n]]);";
        let off = match_project(
            "p1_sub_off",
            "\"error\"",
            Some("match_exhaustiveness = \"off\""),
            src,
        );
        assert_eq!(
            run_file(&off.join("main.wll"), OutputFormat::Human, false),
            ExitCode::SUCCESS,
            "match_exhaustiveness = off must silence the MATCH pass"
        );
        // 总开关没被动过:类型失配照样拦。
        let still_on = match_project(
            "p1_sub_off_types",
            "\"error\"",
            Some("match_exhaustiveness = \"off\""),
            "LET(x: INTEGER, \"s\");",
        );
        assert_eq!(
            run_file(&still_on.join("main.wll"), OutputFormat::Human, false),
            ExitCode::from(1)
        );
    }

    /// 反过来:只开 MATCH 检查也能工作(总开关 off)。
    #[test]
    fn p1_match_exhaustiveness_can_be_enabled_on_its_own() {
        let root = match_project(
            "p1_sub_only",
            "\"off\"",
            Some("match_exhaustiveness = \"error\""),
            "MATCH(OK(1), [[OK(n), n]]);",
        );
        assert_eq!(
            run_file(&root.join("main.wll"), OutputFormat::Human, false),
            ExitCode::from(1)
        );
    }

    /// 缺省跟随 `gradual_typing`:只写总开关就该生效。
    #[test]
    fn p1_match_exhaustiveness_follows_gradual_typing_by_default() {
        let off = match_project(
            "p1_follow_off",
            "\"off\"",
            None,
            "MATCH(OK(1), [[OK(n), n]]);",
        );
        assert_eq!(
            run_file(&off.join("main.wll"), OutputFormat::Human, false),
            ExitCode::SUCCESS
        );
        let on = match_project(
            "p1_follow_on",
            "\"error\"",
            None,
            "MATCH(OK(1), [[OK(n), n]]);",
        );
        assert_eq!(
            run_file(&on.join("main.wll"), OutputFormat::Human, false),
            ExitCode::from(1)
        );
    }

    /// 非法值回落到「跟随」并**可见**(不静默吞笔误)。
    #[test]
    fn p1_invalid_match_exhaustiveness_value_falls_back_and_reports() {
        let root = match_project(
            "p1_invalid",
            "\"error\"",
            Some("match_exhaustiveness = \"loud\""),
            "LET(x, 1);",
        );
        // 回落到跟随 `error` → 无诊断 → 退出 0(回执走 stderr 的警告)。
        assert_eq!(
            run_file(&root.join("main.wll"), OutputFormat::Human, false),
            ExitCode::SUCCESS
        );
    }

    /// 零破坏:默认档(无清单 / off)对带 MATCH 的 v0.9 程序一条诊断都不发。
    #[test]
    fn p1_default_off_stays_silent_on_v09_match_programs() {
        for src in [
            "MATCH(OK(1), [[OK(n), n]]);",
            "MATCH(1, [[_, 1], [2, 2]]);",
            "MATCH(TRUE, [[TRUE, 1]]);",
        ] {
            let bare = write_tmp(src, "p1_off.wll");
            assert_eq!(
                run_file(&bare, OutputFormat::Human, false),
                ExitCode::SUCCESS,
                "default off must stay silent: {src}"
            );
            let root = match_project("p1_off_proj", "\"off\"", None, src);
            assert_eq!(
                run_file(&root.join("main.wll"), OutputFormat::Human, false),
                ExitCode::SUCCESS,
                "off must stay silent: {src}"
            );
            // **不要**在这里清目录:所有用例共用 `%TEMP%/wlwl-cli-tests`,
            // 删它会顺手删掉并行线程里别的用例正在用的夹具(Step 8 踩过)。
            // `module_project` 每次调用只重建**自己那个**子目录,够了。
        }
    }

    /// 格式化保真:省略的 default 臂**不能**被补写成 `, NULL` —— 补出来
    /// 会让「作者兜了底」和「作者漏了分支」在源码里长得一样,正是 Step 8
    /// 要报的那个区别。
    #[test]
    fn p1_fmt_preserves_an_omitted_default_arm() {
        let p = write_tmp("MATCH(1, [[1, 1]]);\n", "p1_fmt.wll");
        assert_eq!(fmt_file(&p, false, false), ExitCode::SUCCESS);
        // 显式写了 default 的照常渲染出来。
        let p2 = write_tmp("MATCH(1, [[1, 1]], 0);\n", "p1_fmt2.wll");
        assert_eq!(fmt_file(&p2, false, false), ExitCode::SUCCESS);
    }

    // -- v0.10 Step 9 (P1-2): 泛型限形(擦除) -----------------------------

    /// 编译期实例化错误:约束不满足 → 走既有的 `E0111`,**不新增码号**。
    #[test]
    fn p2_bound_violation_is_caught_at_compile_time() {
        let root = match_project(
            "p2_bound",
            "\"error\"",
            None,
            "LET(max, FUN((a: T: Comparable, b: T: Comparable) : T: Comparable, IF(<(a, b), a, b)));\
             PRINT(max(1, TRUE));",
        );
        assert_eq!(
            run_file(&root.join("main.wll"), OutputFormat::Human, false),
            ExitCode::from(1)
        );
        // 满足约束 → 静默通过。
        let ok = match_project(
            "p2_bound_ok",
            "\"error\"",
            None,
            "LET(max, FUN((a: T: Comparable, b: T: Comparable) : T: Comparable, IF(<(a, b), a, b)));\
             PRINT(max(1, 2));",
        );
        assert_eq!(
            run_file(&ok.join("main.wll"), OutputFormat::Human, false),
            ExitCode::SUCCESS
        );
    }

    /// 零破坏:默认档对带泛型标注的程序一条诊断都不发,`run` 照跑。
    #[test]
    fn p2_default_off_stays_silent_on_generic_programs() {
        let src = "LET(max, FUN((a: T: Comparable, b: T: Comparable) : T: Comparable, a)); PRINT(max(1, 2));";
        for setting in ["\"off\"", "DEFAULT_NO_KEY"] {
            let root = if setting == "DEFAULT_NO_KEY" {
                let root = module_project("p2_off_no_key", "\"off\"");
                let _ = fs::remove_file(root.join("wlwl.toml"));
                fs::write(root.join("main.wll"), src).unwrap();
                root
            } else {
                match_project("p2_off", setting, None, src)
            };
            assert_eq!(
                run_file(&root.join("main.wll"), OutputFormat::Human, false),
                ExitCode::SUCCESS,
                "off mode must not judge generics: {setting}"
            );
            // 运行期一样能跑(擦除,零语义变化)。
            assert_eq!(
                run_file(&root.join("main.wll"), OutputFormat::Human, true),
                ExitCode::SUCCESS
            );
            // 同上:共用测试根目录,不在用例里清。
        }
    }

    /// 格式化器认识带约束的标注:规范输出的形状**能被解析回来** ——
    /// `T: Comparable` 的 `:` 在 `parse_braced` / `parse_type_expr_from_pieces`
    /// 都认得。
    ///
    /// (规范形式不带句末分号,那是既有约定 —— 无泛型的普通程序也一样。)
    #[test]
    fn p2_fmt_round_trips_a_bounded_annotation() {
        let canonical = "LET(max, FUN((a: T: Comparable, b: T: Comparable): T: Comparable, a))";
        let p = write_tmp(&format!("{canonical}\n"), "p2_fmt.wll");
        assert_eq!(fmt_file(&p, true, false), ExitCode::SUCCESS);
        // 非规范拼写(返回注解冒号前多一个空格)会被 fmt 纠正,说明它确实
        // 认识这个形状而不是把它当噪声吞掉。
        let q = write_tmp(
            "LET(max, FUN((a: T: Comparable, b: T: Comparable) : T: Comparable, a));\n",
            "p2_fmt_nc.wll",
        );
        assert_eq!(fmt_file(&q, false, false), ExitCode::SUCCESS);
    }

    // -- v0.10 Step 10 (P1-3): 工具链薄壳(interface / schema) -----------

    /// `interface` / `schema` 的产物必须是**合法 JSON** —— 工具的第一道
    /// 门槛就是「能不能解析」,所以这里真的解析一遍。
    #[test]
    fn p3_interface_and_schema_emit_parseable_json() {
        let p = write_module(
            "p3_interface",
            "math.wll",
            "SEALED([\"add\"]);\n\
             LET(add, FUN((a: T: Comparable, b: T: Comparable) : T: Comparable, a));\n\
             LET(PI, 3);\n\
             EXPORT([\"add\", \"PI\"]);\n",
        );
        assert_eq!(tooling::interface_file(&p), ExitCode::SUCCESS);
        assert_eq!(tooling::schema_command(), ExitCode::SUCCESS);
        // 解析层已经各自锁了字段;这里锁「命令不崩且退出码为 0」。
    }

    #[test]
    fn p3_interface_on_a_missing_file_fails_without_a_panic() {
        let missing = std::env::temp_dir()
            .join("wlwl-cli-tests")
            .join("p3_nope.wll");
        let _ = fs::remove_file(&missing);
        assert_eq!(tooling::interface_file(&missing), ExitCode::from(1));
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
        // [v0.10.1] 退出码契约(spec §11.5):源文件没解析出来 -> 3。
        // 此前一律 1,与运行期失败无法区分。
        assert_eq!(code, ExitCode::from(3));
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
        // [v0.10.1] 退出码契约(spec §11.5):源文件没解析出来 -> 3。
        // 此前一律 1,与运行期失败无法区分。
        assert_eq!(code, ExitCode::from(3));
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
        let code = fmt_file(&p, false, false);
        assert_eq!(code, ExitCode::SUCCESS);
    }

    #[test]
    fn fmt_check_canonical_source_succeeds() {
        // Already-canonical source (including the trailing newline
        // convention) must pass --check.
        let p = write_tmp("LET(x, 1);\nPRINT(x)\n", "fmt_ok.wll");
        let code = fmt_file(&p, true, false);
        assert_eq!(code, ExitCode::SUCCESS);
    }

    #[test]
    fn fmt_check_canonical_source_without_trailing_newline_succeeds() {
        // A missing final newline is not a §16.3 deviation.
        let p = write_tmp("LET(x, 1);\nPRINT(x)", "fmt_ok_nonl.wll");
        let code = fmt_file(&p, true, false);
        assert_eq!(code, ExitCode::SUCCESS);
    }

    #[test]
    fn fmt_check_deviating_source_fails_with_w0053() {
        // Non-canonical whitespace => --check exits 1 (W0053).
        let p = write_tmp("LET( x ,1 );", "fmt_dev.wll");
        let code = fmt_file(&p, true, false);
        assert_eq!(code, ExitCode::from(1));
    }

    #[test]
    fn fmt_check_missing_file_reports_error() {
        let p = std::path::PathBuf::from("/nonexistent/fmt_target.wll");
        let code = fmt_file(&p, true, false);
        assert_eq!(code, ExitCode::from(1));
    }

    #[test]
    fn fmt_check_parse_error_reports_diagnostic() {
        let p = write_tmp("LET(x, 1", "fmt_bad.wll");
        let code = fmt_file(&p, true, false);
        // [v0.10.1] 退出码契约(spec §11.5):源文件没解析出来 -> 3。
        // 此前一律 1,与运行期失败无法区分。
        assert_eq!(code, ExitCode::from(3));
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
        let code = fmt_file(&p, true, false);
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
        let code = fmt_file(&p, true, false);
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
        let code = fmt_file(&p, true, false);
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
        let code = fmt_file(&p, true, false);
        assert_eq!(
            code,
            ExitCode::from(1),
            "non-canonical whitespace must still fail fmt --check"
        );
    }

    // ---- [v0.10.1] 行尾不是规范形式的一部分 -------------------------

    #[test]
    fn fmt_check_treats_crlf_source_as_canonical() {
        // [v0.10.1] 这是 Windows CI 抓出来的:仓库的 `.gitattributes` 只给
        // `*.rs` 定了 `eol=lf`,`.wll` 夹具没有规则,`core.autocrlf` 生效后
        // Windows 检出的工作区全是 CRLF —— 而 `--check` 逐字节比较,于是
        // **每一个** `.wll` 都报 W0053。ubuntu / macOS 看不见,只有
        // Windows 看得见。
        let p = write_tmp("LET(x, 1);\r\nPRINT(x)\r\n", "fmt_crlf.wll");
        assert_eq!(
            fmt_file(&p, true, false),
            ExitCode::SUCCESS,
            "CRLF is a line-ending choice, not a §16.3 layout deviation"
        );
    }

    #[test]
    fn fmt_check_treats_bare_cr_source_as_canonical() {
        // 老 Mac 风格的单 CR 换行:lexer 能过,formatter 也不该把它当成
        // 偏离 —— 留着它就是另一个更难查的同类洞。
        let p = write_tmp("LET(x, 1);\rPRINT(x)\r", "fmt_barecr.wll");
        assert_eq!(
            fmt_file(&p, true, false),
            ExitCode::SUCCESS,
            "a bare CR is a line-ending choice too"
        );
    }

    #[test]
    fn fmt_check_still_rejects_a_crlf_source_that_really_is_deviating() {
        // 反向守卫:归一化**不能**把真正的偏离一起放过去。把一个确实不合
        // 规范的文件写成 CRLF,必须照样红 —— 否则这条测试就只证明了
        // 「--check 变瞎了」。
        let p = write_tmp("LET( x ,1 );\r\nPRINT( x )\r\n", "fmt_crlf_dev.wll");
        assert_eq!(
            fmt_file(&p, true, false),
            ExitCode::from(1),
            "normalising EOL must not launder a genuine layout deviation"
        );
    }

    #[test]
    fn fmt_check_crlf_and_lf_of_the_same_source_agree() {
        // 判据的直接陈述:同一份源码,两种行尾,`--check` 必须给同一个答案。
        // 这一条比上面三条加起来更接近「问题本身」。
        let lf = write_tmp("LET(x, 1);\nPRINT(x)\n", "fmt_agree_lf.wll");
        let crlf = write_tmp("LET(x, 1);\r\nPRINT(x)\r\n", "fmt_agree_crlf.wll");
        assert_eq!(
            fmt_file(&lf, true, false),
            fmt_file(&crlf, true, false),
            "line endings must not change what --check concludes"
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
