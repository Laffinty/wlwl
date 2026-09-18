//! B11 一键生成器:跑这个 binary 把 `crate::registry::BUILTIN_REGISTRY`
//! 渲染到 `docs/appendix_G.md`。
//!
//! 用法 (在 `impl/` 目录下):
//! ```bash
//! cargo run --bin gen-appendix-g -- ../docs/appendix_G.md
//! ```
//!
//! 默认目标路径:`../docs/appendix_G.md`(假设从 `impl/` 运行)。

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    let target = if args.len() > 1 {
        PathBuf::from(&args[1])
    } else {
        // 默认写到 ../docs/appendix_G.md (impl/Cargo.toml 视角)
        PathBuf::from("../docs/appendix_G.md")
    };

    let md = wlwl_eval::registry::generate_appendix_g_md();
    if let Some(parent) = target.parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            eprintln!("error: create_dir_all({:?}) failed: {}", parent, e);
            return ExitCode::from(2);
        }
    }
    match fs::write(&target, &md) {
        Ok(()) => {
            println!("wrote {} bytes to {}", md.len(), target.display());
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: write({:?}) failed: {}", target, e);
            ExitCode::from(1)
        }
    }
}