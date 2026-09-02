//! WLWL command-line interface (Phase 2: `wlwl run <file>`, `wlwl check <file>`).
//!
//! v0.3 §16.3 explicitly says **commands are out of scope** for the language
//! specification. The exact command set is a per-implementation concern.
//! This file implements the Phase 2 minimum: `run` and `check`, with
//! IMPORT-path resolution against the file's parent directory.

use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, ValueEnum};
use wlwl_eval::Evaluator;
use wlwl_error::{ErrorCode, Location, WlwlDiagnostic, WlwlError};
use wlwl_parser::parse;

#[derive(Debug, Clone, Copy, ValueEnum, Default)]
enum OutputFormat {
    /// Human-readable CLI output (default)
    #[default]
    Human,
    /// Single JSON object (per v0.3 §14.3)
    Json,
}

#[derive(Parser, Debug)]
#[command(name = "wlwl", version, about = "WLWL language interpreter (Phase 2)")]
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
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Run { file, format } => run_file(&file, format, true),
        Cmd::Check { file, format } => run_file(&file, format, false),
    }
}

fn run_file(file: &PathBuf, format: OutputFormat, execute: bool) -> ExitCode {
    let source = match fs::read_to_string(file) {
        Ok(s) => s,
        Err(e) => {
            // File I/O failures get an E0042 diagnostic (module file
            // IO error from §14.4) with a synthetic location.
            let d = WlwlDiagnostic::new(
                ErrorCode::E0042,
                format!("cannot read file '{}': {}", file.display(), e),
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

    // Phase 2: IMPORT paths are resolved relative to the file's parent
    // directory. If the file has no parent (e.g. stdin), fall back to
    // the current working directory.
    let base_dir = file
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));

    let mut ev = Evaluator::new()
        .with_source(&source, &file_name)
        .with_base_dir(base_dir);
    match ev.eval(&ast) {
        Ok(_v) => ExitCode::SUCCESS,
        Err(e) => report_error(e, format),
    }
}

fn report_error(err: WlwlError, format: OutputFormat) -> ExitCode {
    report_diag(err.diagnostic().clone(), format)
}

fn report_diag(d: WlwlDiagnostic, format: OutputFormat) -> ExitCode {
    match format {
        OutputFormat::Human => eprintln!("{}", d.render_human()),
        OutputFormat::Json => eprintln!("{}", d.render_json()),
    }
    ExitCode::from(1)
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
        // Phase 2: IF.
        let p = write_tmp(r#"IF(==(1, 1), PRINT("yes"), PRINT("no"));"#, "if.wl");
        let code = run_file(&p, OutputFormat::Human, true);
        assert_eq!(code, ExitCode::SUCCESS);
    }
}
