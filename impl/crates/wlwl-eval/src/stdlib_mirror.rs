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
//! 层归属与引入版本登记在 `NAMESPACE_META`(私有 const —— 公开的模块级
//! 文档链不到私有项,故写代码而非 intra-doc 链接),与
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
    // [v0.11.3 M2] SHA256 / HMAC_SHA256 追加(沿 str 的先例:引入列记
    // 命名空间的引入版本,新成员的版本写成员表与 CHANGELOG)。
    (
        "wlwl:std.encode",
        "R2",
        "v0.11.2 / SHA256、HMAC_SHA256 于 v0.11.3",
    ),
    ("wlwl:std.text", "R2", "v0.11.2"),
    // [v0.11.3 M1] 输出安全(旗舰特色功能)。**全员 R2**:转义 / 解码 / 净化
    // 都要处理文档级输入,而解释器侧字符串与数组构建超线性(D11-012 /
    // D12-009);归层基准见 v0.11.3 计划 W-08 与 baseline.txt M1 段。
    // `HTML_SANITIZE` 随 M3 落地后本行成员数 2 → 3。
    ("wlwl:std.sanitize", "R2", "v0.11.3"),
    (
        "wlwl:std.collection",
        // [v0.11 M5] 16 成员 R1 + `RANGE` 归 R2。层归属变更不算破坏性变更
        // (ADR-0021 §0.2),故「引入」列仍写 v0.10 及以前(成员)——
        // 变的是实现语言,不是成员面。依据见 baseline.txt 的 M5 段与
        // 偏差 D11-012。
        // [v0.11.3 M5 / L0-A-2] 再加 `MAP` / `FILTER` / `CHUNK` / `WINDOW`
        // 归 R2(四档基准实测每元素成本随尺寸上涨,平方级坐实)。四个成员
        // 同样**留在 EXPORT 里**(门面改名导出),故成员面与本行的成员名册
        // 都不变,只有「层」这一列变。依据见 baseline.txt 的 L0 段。
        "混合(R1 门面 + R2 `RANGE`/`MAP`/`FILTER`/`CHUNK`/`WINDOW`)",
        "v0.10 及以前(成员)/ v0.11(R1 重写,M5 起 RANGE 沉 R2)/ v0.11.3(M5 L0-A-2 再加 MAP·FILTER·CHUNK·WINDOW 沉 R2)",
    ),
    (
        "wlwl:std.str",
        "R1",
        // [v0.11.2 M2] `INDEX_OF` / `CONTAINS_SUB` 追加。与 `std.collection`
        // 同款口径:层与成员面不变,「引入」列记的是**命名空间**的引入版本,
        // 新成员的版本写在成员表与 CHANGELOG 里。
        "v0.11 / INDEX_OF、CONTAINS_SUB 于 v0.11.2",
    ),
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
        // 哨兵**必须**是不会出现在任何成员名 / 命名空间名里的字符串。
        // 原文用 `"OLD"` —— v0.11.2 M3 加了 `FOLD_RIGHT`,而它**含子串
        // `OLD`**(F-O-L-D-R-I-G-H-T),于是这条守卫在成员表里读到自己的
        // 旧正文时判成了「替换失败」。守卫被一个**完全正确**的成员名打成
        // 红了,而失败的提示(`assertion failed: !out.contains("OLD")`)
        // 指向的方向完全相反。
        let spec = "head\n<!-- appendix-a:begin -->\nZZ-OLD-BODY-SENTINEL-XYZZ\n<!-- appendix-a:end -->\ntail\n";
        let out = splice_appendix_a(spec).expect("markers present");
        assert!(out.contains("head\n"));
        assert!(out.contains("<!-- appendix-a:begin -->"));
        assert!(out.contains("<!-- appendix-a:end -->"));
        assert!(
            !out.contains("ZZ-OLD-BODY-SENTINEL-XYZZ"),
            "the previous body must be fully replaced"
        );
        assert!(out.contains("| 命名空间 | 成员 | 层 | 引入 |"));
        assert!(out.contains("tail\n"));
    }

    #[test]
    fn splice_returns_none_on_missing_markers() {
        assert!(splice_appendix_a("no markers here").is_none());
    }

    // ---- R1 内核注入(M3-0)------------------------------------------------
    //
    // 「注入面不外泄」是 ADR-0021:90「混合模块以门面为对外契约层」的机械
    // 形式。断言各封一种泄漏:kernel 名字**跑进 EXPORT**(于是出现在附录 A
    // 与 IMPORT 面上)、kernel 名字**撞上全局内建**(门面里同名的 `LET` 会
    // 触发 W0030 遮蔽警告,语义上也多了一个不需要的入口)、以及共享表里躺着
    // **没人用的** kernel(死代码,且下次有人读它会以为它在生效)。
    //
    // **`kernels::KERNELS` 不是全部 kernel 的单源。** M3-0 写下这条守卫时
    // 断言的是「注入 ⊆ KERNELS」,M3-3 立刻被证伪:`std.test` 的三个 kernel
    // (`_TEST` / `_EXPECT_ERR` / `_RUN_TESTS`)住在**它自己的 R2 模块文件**
    // `test_native.rs` 里,不在 `kernels.rs`。这是对的 —— 内核代码该跟它服务
    // 的模块放在一起(规范 §0.2 说 R2 成员在 `wlwl-std`,没说在哪个文件)。
    // 于是 `KERNELS` 的真实身份是「**模块无关的共享** kernel 表」,模块
    // 还可以声明自己的。守卫因此改成要求**反方向**:KERNELS ⊆ 注入 ——
    // 那一条才是能防死代码的。

    /// `KERNELS` 里登记的共享 kernel 名字。
    fn shared_kernel_names() -> Vec<&'static str> {
        wlwl_std::kernels::KERNELS.iter().map(|(n, _)| *n).collect()
    }

    /// 全部已登记的 R2 kernel 名字(含各模块自带的)。
    fn all_kernel_names() -> Vec<&'static str> {
        wlwl_std::LANG_SOURCES
            .iter()
            .flat_map(|s| s.kernels.iter().map(|(n, _)| *n))
            .chain(shared_kernel_names())
            .collect()
    }

    #[test]
    fn r1_kernels_never_leak_to_the_export_surface() {
        for src in wlwl_std::LANG_SOURCES {
            let names = wlwl_std::lang_exports(src.source);
            let exports: std::collections::HashSet<&str> =
                names.iter().map(String::as_str).collect();
            for (name, _) in src.kernels {
                assert!(
                    name.starts_with('_'),
                    "{} injects `{name}` without the `_` prefix — it would read \
                     like a public member in the facade source",
                    src.path
                );
                assert!(
                    name.chars()
                        .all(|c| c.is_ascii_uppercase() || c == '_' || c.is_ascii_digit()),
                    "{} injects `{name}` which is not UPPER_SNAKE",
                    src.path
                );
                assert!(
                    !exports.contains(*name),
                    "kernel `{name}` is EXPORTed by {} — it must stay private to the \
                     facade, or it lands in appendix A and on the IMPORT surface",
                    src.path
                );
                assert!(
                    !crate::registry::resolved_builtin_names().contains(name),
                    "R2 kernel `{name}` collides with a global builtin — the facade's \
                     `LET({name}, ...)` would raise W0030 (shadowing) and the kernel \
                     would shadow a language-surface name"
                );
            }
        }
    }

    /// 共享表里的每个 kernel 都得有人用,否则是死代码。
    #[test]
    fn every_shared_kernel_is_actually_injected() {
        for (name, _) in wlwl_std::kernels::KERNELS {
            let used = wlwl_std::LANG_SOURCES
                .iter()
                .any(|s| s.kernels.iter().any(|(n, _)| n == name));
            assert!(
                used,
                "`{name}` is listed in `kernels::KERNELS` but no module injects it — \
                 delete it or wire it up. (That table holds only the *shared* kernels; \
                 module-local ones live in their own R2 file.)"
            );
        }
        assert!(
            !shared_kernel_names().is_empty(),
            "`kernels::KERNELS` is empty — the shared-kernel table was emptied out"
        );
    }

    /// [D12-002] 注入表里的每个 kernel,都必须在门面的 `.wll` 源码里真的
    /// 被用到 —— 否则是死注入。
    ///
    /// 为什么需要这条:上一条 `every_shared_kernel_is_actually_injected` 只锁
    /// **一个方向**(`KERNELS` ⊆ 注入)。而 `CHANGELOG.md` v0.11.1 声称修死注入
    /// 时「并补反向守卫」, 声称的正是本条。反方向一旦没人守, 有人把死内核
    /// **同时**加回 `KERNELS` 与注入表, 全部门禁照绿 —— 这正是 v0.11.1 里
    /// `_DIAG_E0038` 死注入当初能活下来的形状。
    #[test]
    fn every_injected_kernel_is_used_in_its_module_source() {
        for src in wlwl_std::LANG_SOURCES {
            let missing =
                kernels_without_a_use_site(src.source, src.kernels.iter().map(|(n, _)| *n));
            assert!(
                missing.is_empty(),
                "{} injects {} kernel(s) that the facade source never mentions: {missing:?} — \
                 a dead injection. Remove it from the injection table, or wire it up.",
                src.path,
                missing.len()
            );
        }
    }

    /// [D12-002] 上面那条的**负向自检**:守卫写反、或者匹配逻辑退化成
    /// 「什么都找得到」时,上面会变成恒真断言 —— 这正是 D11-006 踩过的坑
    /// (当时有个 `names_match_catalog` 恒等于左右两边, 从未锁住任何东西)。
    #[test]
    fn the_dead_injection_guard_actually_rejects_one() {
        let src = "LET(x, 0), SET(x, 1)";

        // 真的没被用到 ⇒ 必须报出来。
        assert_eq!(
            kernels_without_a_use_site(src, std::iter::once("_NOT_CALLED")),
            vec!["_NOT_CALLED".to_string()],
            "a kernel with no use site must be reported as dead"
        );
        // 正向对照:出现过的名字不该被报出来(顺带证明匹配不是「一律报」)。
        assert!(
            kernels_without_a_use_site(src, std::iter::once("x")).is_empty(),
            "a name that does appear must not be reported"
        );
        // 注释与字符串字面量里的名字**不算**使用点。
        let prose = "// _KIND(x) 见文件头\nLET(s, \"_KIND is documented here\")";
        assert_eq!(
            kernels_without_a_use_site(prose, std::iter::once("_KIND")),
            vec!["_KIND".to_string()],
            "comment / string mentions must not count as use sites — collection.wll's header \
             lists every injected kernel by name, and records the *deleted* `_DIAG_E0038` in \
             prose; a naive contains() would be fooled by the very text explaining the deletion"
        );
        // 词边界:名字是别的标识符的一部分时不匹配。
        assert_eq!(
            kernels_without_a_use_site("LET(_KIND_EXTRA, 1)", std::iter::once("_KIND")),
            vec!["_KIND".to_string()],
            "`_KIND` must not be found inside `_KIND_EXTRA`"
        );
    }

    /// 把 `.wll` 源码里的**注释与字符串字面量**抹成空格,只留可执行代码。
    ///
    /// 必须抹的理由见 `the_dead_injection_guard_actually_rejects_one`:门面文件
    /// 的头注释会**逐条列出本模块注入的每个 kernel 名**,其中还记着已被删除的
    /// `_DIAG_E0038`。不抹的话,那句解释删除原因的散文本身就会成为一个
    /// 「现存的调用点」。
    fn strip_comments_and_strings(src: &str) -> String {
        let chars: Vec<char> = src.chars().collect();
        let mut out = String::with_capacity(src.len());
        let mut i = 0;
        while i < chars.len() {
            match chars[i] {
                '/' if i + 1 < chars.len() && chars[i + 1] == '/' => {
                    while i < chars.len() && chars[i] != '\n' {
                        out.push(' ');
                        i += 1;
                    }
                }
                '"' => {
                    out.push(' ');
                    i += 1;
                    while i < chars.len() {
                        match chars[i] {
                            // `\"` 是字面量里的一个引号,两个字符都吞掉。
                            '\\' if i + 1 < chars.len() => {
                                out.push(' ');
                                out.push(' ');
                                i += 2;
                            }
                            '"' => {
                                out.push(' ');
                                i += 1;
                                break;
                            }
                            // 未闭合的串:退到行尾,别把后面整份文件都吃掉。
                            '\n' => {
                                out.push(' ');
                                i += 1;
                                break;
                            }
                            _ => {
                                out.push(' ');
                                i += 1;
                            }
                        }
                    }
                }
                c => {
                    out.push(c);
                    i += 1;
                }
            }
        }
        out
    }

    fn is_identifier_char(c: char) -> bool {
        c.is_alphanumeric() || c == '_'
    }

    /// 返回「注入了但源码里没有任何使用点」的 kernel 名。
    ///
    /// 匹配的是**词边界的出现**而不是 `name(`:门面有两种合法用法,`_RANGE`
    /// 与 `_EXPECT_ERR` 都是**裸标识符改名导出**(`LET(RANGE, _RANGE);`),
    /// 要求括号会把它们误判成死注入。
    fn kernels_without_a_use_site<'a>(
        source: &str,
        kernel_names: impl Iterator<Item = &'a str>,
    ) -> Vec<String> {
        let code = strip_comments_and_strings(source);
        let mut missing = Vec::new();
        for name in kernel_names {
            let used = code.match_indices(name).any(|(i, _)| {
                let before_ok =
                    i == 0 || !is_identifier_char(code[..i].chars().next_back().unwrap());
                let after = i + name.len();
                let after_ok = after >= code.len()
                    || !is_identifier_char(code[after..].chars().next().unwrap());
                before_ok && after_ok
            });
            if !used {
                missing.push(name.to_string());
            }
        }
        missing
    }

    /// 收集器坏掉时的反向守卫:「扫不到任何 kernel 就转绿」会让上面两条
    /// 断言退化成检查零件事的空断言(它们都以 `LANG_SOURCES` 的迭代结果
    /// 或 `KERNELS` 自身为输入)。
    #[test]
    fn kernel_collection_actually_finds_something() {
        let all = all_kernel_names();
        assert!(
            all.len() >= 7,
            "found only {} kernel(s): {all:?} — the M3-0 injection path looks dead \
             and its guards would pass vacuously",
            all.len()
        );
    }

    /// [D11-019] 行扫描器与 AST 权威口径必须看到**同一组**名字。
    ///
    /// 为什么需要这条:附录 A 镜像生成器、三份契约测试的「实际」侧、
    /// 以及 kernel 不外泄守卫,全都走 `wlwl_std::lang_exports` 这一个扫描器
    /// —— 它们彼此对账时是**同源**的,所以扫描器自身的偏差会自洽地全绿。
    /// 运行期真正生效的是 AST 的 `collect_exports`,故拿它当基准。
    ///
    /// 比较集合而非序列:顺序由「成员面 == 规范表格行序」那几条锁负责,
    /// 这里只管**成员是谁**。
    #[test]
    fn the_line_scanner_agrees_with_the_ast_export_walker() {
        for src in wlwl_std::LANG_SOURCES {
            let ast = wlwl_parser::parse(src.source, "std.wll")
                .unwrap_or_else(|e| panic!("{} must parse as a module: {e}", src.path));
            let from_ast: std::collections::HashSet<String> = crate::collect_exports(&ast);
            let from_scan: std::collections::HashSet<String> =
                wlwl_std::lang_exports(src.source).into_iter().collect();
            assert_eq!(
                from_scan, from_ast,
                "{}: the line scanner and the AST walker disagree — one of them is \
                 wrong, and the appendix-A mirror follows the scanner",
                src.path
            );
            assert!(
                !from_ast.is_empty(),
                "{}: no EXPORT found at all — this guard would pass vacuously",
                src.path
            );
        }
    }
}
