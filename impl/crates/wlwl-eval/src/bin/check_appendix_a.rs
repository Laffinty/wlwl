//! [v0.11.4 / v1.0 规范 §0.7] 附录 A 一致性检查工具(**只读**)。
//!
//! v1.0 规范把标准库附录 A 定为**冻结快照**:单源真相是规范自身,表体
//! **不得**由实现清单自动生成,更**不得**为追平实现的当次改动而改写
//! (规范仓注册镜像册 §0.7 规则 1)。
//!
//! ⇒ 本工具**没有**写回路径。它**读**目标文件 → **取**附录 A 标记区 →
//! **比**「实现派生的表体」:
//! - 一致 ⇒ 打印 `OK` 并以 0 退出;
//! - 不一致 ⇒ 打印逐行差异并以**非 0** 退出。
//!
//! **本工具不写任何文件。** 附录 A 的抄件只应由规范仓的冻结快照更新
//! (人工同步 + `tests/stdlib_appendix_a_sync.rs` 的守卫 A 复核)。
//!
//! 用法(在 `impl/` 目录下):
//! ```bash
//! cargo run --bin check-appendix-a
//! ```
//!
//! 默认目标路径:`../docs/wlwl-spec-reference.md`(从 `impl/` 运行的视角);
//! 可传第一个参数覆盖。
//!
//! ⚠ 目标文件**必须**带 `<!-- appendix-a:begin -->` / `<!-- appendix-a:end -->`
//! 两个标记:缺标记时本工具报错退出(退出码 2),**不做猜测性改写**。

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

const BEGIN: &str = "<!-- appendix-a:begin -->";
const END: &str = "<!-- appendix-a:end -->";

/// 归一化行尾后再比对(与 `tests/stdlib_appendix_a_sync.rs` 同款)。
///
/// 这**不是**把比较放松:内容仍然是逐字比对,只是先把「本次 checkout 落成
/// CRLF 还是 LF」这个**平台产物**排除掉(Windows runner 的 git 默认
/// `core.autocrlf=true`)。
fn normalized(s: &str) -> String {
    s.replace("\r\n", "\n").trim().to_string()
}

/// 取两个标记之间的内容(标记本身保留在文件里,这里只取内容)。
fn section_between_markers(spec: &str) -> Option<&str> {
    let start = spec.find(BEGIN)? + BEGIN.len();
    let rel_end = spec[start..].find(END)?;
    Some(&spec[start..start + rel_end])
}

/// 逐行打印两份表体的差异。
fn print_diff(left: &str, right: &str) {
    let l: Vec<&str> = left.lines().collect();
    let r: Vec<&str> = right.lines().collect();
    for i in 0..l.len().max(r.len()) {
        let a = l.get(i).copied();
        let b = r.get(i).copied();
        if a == b {
            continue;
        }
        eprintln!("  line {}:", i + 1);
        if let Some(a) = a {
            eprintln!("    copy: {a}");
        }
        if let Some(b) = b {
            eprintln!("    impl: {b}");
        }
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    let target = if args.len() > 1 {
        PathBuf::from(&args[1])
    } else {
        PathBuf::from("../docs/wlwl-spec-reference.md")
    };

    let spec = match fs::read_to_string(&target) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: cannot read {}: {}", target.display(), e);
            return ExitCode::from(2);
        }
    };
    let Some(section) = section_between_markers(&spec) else {
        eprintln!(
            "error: {} has no appendix-a markers (<!-- appendix-a:begin --> / :end -->)",
            target.display()
        );
        return ExitCode::from(2);
    };

    let copy = normalized(section);
    let derived = normalized(&wlwl_eval::stdlib_mirror::generate_appendix_a_md());
    if copy == derived {
        println!(
            "OK: appendix A in {} matches the implementation-derived body",
            target.display()
        );
        return ExitCode::SUCCESS;
    }

    eprintln!(
        "MISMATCH: the appendix A in {} differs from the implementation-derived body.",
        target.display()
    );
    eprintln!(
        "  the frozen snapshot is the truth (v1.0 spec registry §0.7) — \
         this tool does NOT rewrite anything;"
    );
    eprintln!(
        "  register the difference per the v1.0 conformance booklet §5.4 \
         and converge the implementation side."
    );
    print_diff(&copy, &derived);
    ExitCode::from(1)
}
