//! [v0.11 M1-3] 一键生成器:把 `crate::stdlib_mirror::generate_appendix_a_md`
//! 拼回 `docs/stdlib/wlwl-stdlib-spec-v0.11.md` 的附录 A 标记区。
//!
//! 用法(在 `impl/` 目录下):
//! ```bash
//! cargo run --bin gen-appendix-a
//! ```
//!
//! 默认目标路径:`../docs/stdlib/wlwl-stdlib-spec-v0.11.md`(从 `impl/`
//! 运行的视角);可传第二个参数覆盖。

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    let target = if args.len() > 1 {
        PathBuf::from(&args[1])
    } else {
        PathBuf::from("../docs/stdlib/wlwl-stdlib-spec-v0.11.md")
    };

    let spec = match fs::read_to_string(&target) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: cannot read {}: {}", target.display(), e);
            return ExitCode::from(2);
        }
    };
    let Some(spliced) = wlwl_eval::stdlib_mirror::splice_appendix_a(&spec) else {
        eprintln!(
            "error: {} has no appendix-a markers (<!-- appendix-a:begin --> / :end -->)",
            target.display()
        );
        return ExitCode::from(2);
    };
    match fs::write(&target, spliced) {
        Ok(()) => {
            println!("rewrote appendix A in {}", target.display());
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: write({:?}) failed: {}", target, e);
            ExitCode::from(1)
        }
    }
}
