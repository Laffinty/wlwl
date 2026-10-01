//! [v0.11 M1-3] stdlib 规范附录 A ↔ 实现清单镜像锁。
//!
//! 单源真相在实现(见 `wlwl_eval::stdlib_mirror`):R2 成员取
//! `ModuleSpec` 绑定表(collection/test 名录取 eval 侧 BUILTINS),R1
//! 成员取嵌入源码的 `EXPORT` 声明。本文件把
//! `docs/stdlib/wlwl-stdlib-spec-v0.11.md` 附录 A 标记区与生成器输出
//! 逐字节对账 —— 漂移即文档撒谎,跑 `cargo run --bin gen-appendix-a`
//! 重新拼接。

use std::fs;
use std::path::Path;

const BEGIN: &str = "<!-- appendix-a:begin -->";
const END: &str = "<!-- appendix-a:end -->";

fn spec_path() -> PathBuf {
    // CARGO_MANIFEST_DIR = impl/crates/wlwl-eval;仓库根的 docs/ 在三级之上。
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../docs/stdlib/wlwl-stdlib-spec-v0.11.md")
}
use std::path::PathBuf;

#[test]
fn stdlib_spec_appendix_a_matches_the_implementation() {
    let md = fs::read_to_string(spec_path()).expect("stdlib spec is readable from the repo");
    let start = md
        .find(BEGIN)
        .expect("appendix-a begin marker missing from the stdlib spec");
    let rel_end = md[start..]
        .find(END)
        .expect("appendix-a end marker missing from the stdlib spec");
    let section = &md[start + BEGIN.len()..start + rel_end];
    let generated = wlwl_eval::stdlib_mirror::generate_appendix_a_md();
    assert_eq!(
        section.trim(),
        generated.trim(),
        "appendix A drifted from the implementation — \
         run `cargo run --bin gen-appendix-a` to resync"
    );
}

#[test]
fn every_namespace_in_the_spec_resolves_in_the_implementation() {
    let md = fs::read_to_string(spec_path()).unwrap();
    for ns in [
        "std.io",
        "std.fs",
        "std.json",
        "std.format",
        "std.collection",
        "std.str",
        "std.math",
        "std.test",
        "std.ai",
        "std.agent",
    ] {
        assert!(
            md.contains(&format!("`{ns}`")),
            "stdlib spec lost its `{ns}` namespace"
        );
        assert!(
            wlwl_std::resolve(&format!("wlwl:{ns}")).is_some(),
            "`{ns}` is documented in the spec but does not resolve"
        );
    }
}
