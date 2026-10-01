//! [v0.11 M1-3 / ADR-0021] stdlib 规范附录 A 镜像生成器。
//!
//! 单源真相:
//! - R2 原生层成员来自 `wlwl_std` 的 `ModuleSpec` 绑定表;collection /
//!   test 两个名录模块的 SPEC.functions 为空(既有契约),真名册取本
//!   crate 的 `collection::BUILTINS` / `test::BUILTINS`;
//! - R1 语言层成员来自嵌入 `.wll` 源码的 `EXPORT([...])` 声明(经
//!   `wlwl_std::lang_exports` 提取;运行期加载由 AST 的
//!   `collect_exports` 权威提取,两者不一致会被锁测试抓出来)。
//!
//! 层归属与引入版本登记在 [`NAMESPACE_META`],与
//! `docs/stdlib/wlwl-stdlib-spec-v0.11.md` §0.2 归属总表一致。
//!
//! 产出写进 stdlib 规范的附录 A 标记区(`<!-- appendix-a:begin -->`
//! 到 `<!-- appendix-a:end -->` 之间,标记保留),锁测试
//! `tests/stdlib_appendix_a_sync.rs` 双向对账。
//!
//! 用法(在 `impl/` 目录下):
//! ```bash
//! cargo run --bin gen-appendix-a
//! ```

use wlwl_std::StdBackend;

/// (完整路径, 层, 引入)。层的取值与 stdlib 规范 §0.2 一致:
/// R2 / R1 / 混合。
const NAMESPACE_META: &[(&str, &str, &str)] = &[
    ("wlwl:std.io", "R2", "v0.10 及以前"),
    ("wlwl:std.fs", "R2", "v0.10 及以前"),
    ("wlwl:std.json", "R2", "v0.10 及以前"),
    ("wlwl:std.format", "R2", "v0.10 及以前"),
    (
        "wlwl:std.collection",
        "R1",
        "v0.10 及以前(成员)/ v0.11(R1 重写)",
    ),
    ("wlwl:std.str", "R1", "v0.11"),
    ("wlwl:std.math", "混合", "v0.11"),
    ("wlwl:std.test", "混合", "v0.10 及以前(成员)/ v0.11(混合化)"),
    ("wlwl:std.ai", "R2", "v0.10 及以前"),
    ("wlwl:std.agent", "R2", "v0.10 及以前"),
];

/// 一个命名空间的当前成员名册(实现真相)。
fn members(path: &str) -> Vec<String> {
    match wlwl_std::resolve(path) {
        Some(StdBackend::Lang(src)) => wlwl_std::lang_exports(src.source),
        Some(StdBackend::Native(spec)) => {
            // [v0.11 M2 / ADR-0022] 名录特判废止,成员直接来自 SPEC。
            spec.functions.iter().map(|(n, _)| n.to_string()).collect()
        }
        None => Vec::new(),
    }
}

/// 生成附录 A 的表体(仅表格本身;标记与前置说明留在规范文件里)。
pub fn generate_appendix_a_md() -> String {
    let mut out = String::new();
    out.push_str("| 命名空间 | 成员 | 层 | 引入 |\n");
    out.push_str("|---|---|---|---|\n");
    for (path, layer, introduced) in NAMESPACE_META {
        let short = path.trim_start_matches("wlwl:std.");
        let names = members(path);
        let members = if names.is_empty() {
            "—".to_string()
        } else {
            names
                .iter()
                .map(|n| format!("`{n}`"))
                .collect::<Vec<_>>()
                .join(" ")
        };
        out.push_str(&format!(
            "| `std.{short}` | {members} | {layer} | {introduced} |\n"
        ));
    }
    out
}

/// 把生成的表体拼回 stdlib 规范文本:替换两个标记之间的内容,标记
/// 本身保留。规范文件缺少标记或标记失序时返回 `None`(由 bin 报错,
/// 不做猜测性改写)。
pub fn splice_appendix_a(spec_md: &str) -> Option<String> {
    const BEGIN: &str = "<!-- appendix-a:begin -->";
    const END: &str = "<!-- appendix-a:end -->";
    let start = spec_md.find(BEGIN)? + BEGIN.len();
    let end = spec_md[start..].find(END)? + start;
    let mut out = String::with_capacity(spec_md.len() + 256);
    out.push_str(&spec_md[..start]);
    out.push('\n');
    out.push_str(&generate_appendix_a_md());
    out.push_str(&spec_md[end..]);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_namespace_resolves_with_members() {
        for (path, _, _) in NAMESPACE_META {
            let names = members(path);
            assert!(
                !names.is_empty(),
                "{path} resolves but its member list came out empty — \
                 the mirror would silently hide a whole namespace"
            );
        }
    }

    #[test]
    fn layer_meta_covers_exactly_the_resolvable_namespaces() {
        for (path, _, _) in NAMESPACE_META {
            assert!(
                wlwl_std::resolve(path).is_some(),
                "{path} is listed in NAMESPACE_META but does not resolve"
            );
        }
        // resolve 可达而 META 缺登记 = 文档撒谎。
        for path in [
            "wlwl:std.io",
            "wlwl:std.fs",
            "wlwl:std.json",
            "wlwl:std.format",
            "wlwl:std.collection",
            "wlwl:std.str",
            "wlwl:std.math",
            "wlwl:std.test",
            "wlwl:std.ai",
            "wlwl:std.agent",
        ] {
            assert!(
                NAMESPACE_META.iter().any(|(p, _, _)| *p == path),
                "{path} resolves but is missing from NAMESPACE_META"
            );
        }
    }

    #[test]
    fn splice_replaces_between_markers_and_keeps_them() {
        let spec = "head\n<!-- appendix-a:begin -->\nOLD\n<!-- appendix-a:end -->\ntail\n";
        let out = splice_appendix_a(spec).expect("markers present");
        assert!(out.contains("head\n"));
        assert!(out.contains("<!-- appendix-a:begin -->"));
        assert!(out.contains("<!-- appendix-a:end -->"));
        assert!(!out.contains("OLD"));
        assert!(out.contains("| 命名空间 | 成员 | 层 | 引入 |"));
        assert!(out.contains("tail\n"));
    }

    #[test]
    fn splice_returns_none_on_missing_markers() {
        assert!(splice_appendix_a("no markers here").is_none());
    }
}
