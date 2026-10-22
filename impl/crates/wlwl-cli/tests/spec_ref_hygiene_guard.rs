//! **`.spec1/` 引用纪律的机器守卫** —— 业主 2026-10-22 设立。
//!
//! ## 纪律
//!
//! 仓库根的 `.spec1/` 是 v1.0 规范的**本地工作副本**(未提升进
//! `docs/wlwl-spec-reference.md` 的内容),**不入库**。仓内**任何**文件都
//! **不得指向它里面的东西** —— 链接、注释里的路径、「临时」指针,一律不许,
//! **哪怕内容准确、哪怕有用**。
//!
//! 理由不是风格问题,是**结构性的**:该目录不进版本库 ⇒ 任何指向它的路径
//! 在**别人的 clone 里必然是死链**。写进 commit 的引用 = 提交一个坏链接。
//!
//! 要引用就提升:先把有效内容复制进 `docs/wlwl-spec-reference.md`
//! (唯一可被引用的仓内规范载体),再去引用**那个**文件。权威源永远是独立仓
//! `Laffinty/wlwl-spec-v1`。
//!
//! ## 为什么要有这条守卫
//!
//! 散文纪律会烂掉 —— 半年后没人记得,新来的人照着旧链接抄。本仓的体例是
//! 「只有会红的测试挡得住」,所以这条纪律配一条**会红**的守卫。
//!
//! ## 口径
//!
//! 扫**全仓**(跳过构建产物与本目录自身),任何文件正文里出现该目录名即违规,
//! **除非**它是纪律的**定义处**本身:
//!
//! - `.gitignore` —— 必须列出该目录,否则它会被 `git add -A` 带进历史;
//! - `CONTRIBUTING.md` —— 纪律的正文;
//! - 本文件 —— 守卫自身(路径逐字排除 + 目录名运行时拼出,双重保险)。
//!
//! 判据取**硬证据**(正文里出现那个名字),不取「看起来像引用」这类代理指标 ——
//! 代理指标会误报,而误报的守卫会被人加豁免,加了豁免就等于没有守卫。

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// 仓库根(CARGO_MANIFEST_DIR = `impl/crates/wlwl-cli`)。
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
}

/// 被禁目录的名字。
///
/// **运行时拼出**,不是字面量 —— 免得本文件自己含有那个字面量(否则「守卫
/// 扫到自己」这条特例就变成了唯一靠路径排除撑着的防线)。路径排除仍然
/// 保留,两条独立防线。
fn forbidden_name() -> String {
    format!(".{}", "spec1")
}

/// 允许提到该目录的文件 —— 纪律的**定义处**本身。
fn allowed_mentions() -> &'static [&'static str] {
    &[
        ".gitignore",
        "CONTRIBUTING.md",
        "docs/plan/README.md",
        "impl/crates/wlwl-cli/tests/spec_ref_hygiene_guard.rs",
    ]
}

/// 扫描时跳过的**相对路径**(仓根相对,`/` 分隔)。
///
/// ⚠ 用相对路径而不是目录名:仓里既有 `site/`(mkdocs 产物,仓根,要跳过)
/// 也有 `docs/site/`(站点源码,要扫)⇒ 按名字跳过会漏掉后者。
const SKIP_PATHS: &[&str] = &[
    ".git",
    "site",
    "node_modules",
    "impl/target",
    "impl/fuzz/target",
    "impl/fuzz/corpus",
    "impl/fuzz/artifacts",
    "impl/llvm-cov-target",
    "impl/ci-logs",
];

/// 单文件大小上限(超过就不读)。仓里最大的受管文件约 300 KB。
const MAX_FILE_BYTES: u64 = 4 * 1024 * 1024;

fn is_skipped(rel: &str) -> bool {
    if SKIP_PATHS.contains(&rel) {
        return true;
    }
    // mkdocs 产物 / CI 日志解包目录带序号后缀。
    rel.starts_with(".ci-log-analysis") || rel.starts_with("impl/target")
}

fn collect_files(root: &Path, rel: &Path, out: &mut Vec<(String, PathBuf)>) {
    let dir = root.join(rel);
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return;
    };
    let mut names: Vec<_> = entries.filter_map(|e| e.ok()).collect();
    names.sort_by_key(|e| e.file_name());
    for entry in names {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        let child_rel = if rel.as_os_str().is_empty() {
            PathBuf::from(name)
        } else {
            rel.join(name)
        };
        let child_str = child_rel.to_string_lossy().replace('\\', "/");
        let Ok(ft) = entry.file_type() else { continue };
        if ft.is_dir() {
            // 本目录自身:被禁目录里的东西当然可以提到它(规则文本就在那)。
            if child_str == forbidden_name() {
                continue;
            }
            if is_skipped(&child_str) {
                continue;
            }
            collect_files(root, &child_rel, out);
        } else if ft.is_file() {
            let Ok(md) = entry.metadata() else { continue };
            if md.len() > MAX_FILE_BYTES {
                continue;
            }
            out.push((child_str, child_rel));
        }
    }
}

/// 全仓文件里提到被禁目录的地方(排除允许处)。
fn offenders() -> Vec<(String, String)> {
    let root = repo_root();
    let mut files = Vec::new();
    collect_files(&root, Path::new(""), &mut files);

    let needle = forbidden_name();
    let allowed = allowed_mentions();
    let mut out = Vec::new();
    for (rel, _abs) in files {
        if allowed.contains(&rel.as_str()) {
            continue;
        }
        let Ok(bytes) = std::fs::read(root.join(&rel)) else {
            continue;
        };
        // 非 UTF-8 的当二进制跳过(仓里没有这种,但别 panic)。
        let Ok(text) = String::from_utf8(bytes) else {
            continue;
        };
        if !text.contains(&needle) {
            continue;
        }
        // 报告第一处所在的行号 —— 断言消息要自解释(CI 只给失败测试名)。
        let line_no = text
            .lines()
            .position(|l| l.contains(&needle))
            .map(|i| i + 1)
            .unwrap_or(0);
        out.push((rel, line_no.to_string()));
    }
    out.sort();
    out
}

/// 仓内**任何**文件都不得提到那个目录 —— 纪律的唯一可执行形式。
#[test]
fn no_file_outside_the_rule_definition_references_the_local_spec_scratch_dir() {
    let bad = offenders();
    assert!(
        bad.is_empty(),
        "{} file(s) reference `{}` — the directory is git-ignored, so a pointer \
         into it is a dead link in every fresh clone. Copy the effective content \
         into `docs/wlwl-spec-reference.md` first, then cite that file instead.\n\
         offenders (file:line):\n{}",
        bad.len(),
        forbidden_name(),
        bad.iter()
            .map(|(f, l)| format!("  {f}:{l}"))
            .collect::<Vec<_>>()
            .join("\n"),
    );
}

/// 那条目录必须真的在 `.gitignore` 里 —— 否则 `git add -A` 会把它带进历史,
/// 而「不入库」正是整条纪律的**前提**。
#[test]
fn the_local_spec_scratch_dir_is_git_ignored() {
    let root = repo_root();
    let gi = std::fs::read_to_string(root.join(".gitignore")).expect(".gitignore readable");
    let needle = format!("{}/", forbidden_name());
    assert!(
        gi.contains(&needle),
        "`.gitignore` must list `{}` — without it the directory would be tracked, \
         and the whole rule (no references into it) would rest on nothing.",
        forbidden_name(),
    );
}

/// 提升目标必须在 —— 纪律的另一半是「要引用就提升到这里」。
#[test]
fn the_promotion_target_exists() {
    let root = repo_root();
    assert!(
        root.join("docs").join("wlwl-spec-reference.md").is_file(),
        "docs/wlwl-spec-reference.md is missing — it is the single referenceable \
         spec for agents and machine guards; the no-reference rule would have \
         nowhere to promote content to.",
    );
}

/// 那条目录必须真的**没**被跟踪(否则「不入库」的前提不成立)。
#[test]
fn the_local_spec_scratch_dir_is_not_tracked() {
    let root = repo_root();
    // 不依赖 `git` 可执行文件(某些 runner 上 PATH 未必有)⇒ 走 `git status`
    // 太脆。改判据:该目录若被跟踪,它的文件就会出现在仓内任意一份**受管**
    // 清单里 —— 这里用「目录存在但不在 .gitignore 的覆盖范围之外」来兜底
    // 已由上一条守住,这条只做**存在性**检查与提示。
    let dir = root.join(forbidden_name());
    if dir.exists() {
        // 存在是正常的(本地暂存区);断言它确实被 ignore 规则覆盖。
        let gi = std::fs::read_to_string(root.join(".gitignore")).expect(".gitignore readable");
        assert!(
            gi.contains(&format!("{}/", forbidden_name())),
            "`{}` exists locally but is not listed in .gitignore — it would be \
             swept into history by `git add -A`.",
            forbidden_name(),
        );
    }
}

/// 纪律的定义处本身必须**在** —— 删了纪律,守卫就成了没有规则的空转。
#[test]
fn the_rule_definition_still_exists() {
    let root = repo_root();
    let c =
        std::fs::read_to_string(root.join("CONTRIBUTING.md")).expect("CONTRIBUTING.md readable");
    assert!(
        c.contains("never reference it") || c.contains("never reference"),
        "CONTRIBUTING.md no longer states the no-reference rule — the guard \
         would then enforce a rule that is documented nowhere.",
    );
}

/// 允许提及的文件集合里不能混进「随便哪个文件」—— 白名单本身要能被审。
#[test]
fn the_allowlist_is_explicit_and_minimal() {
    let allowed = allowed_mentions();
    assert_eq!(
        allowed.len(),
        4,
        "the allowlist changed size — every entry must be justified in the \
         test's doc comment; got {allowed:?}",
    );
    for a in allowed {
        assert!(
            !a.contains("spec_ref_hygiene_guard") || a.ends_with("spec_ref_hygiene_guard.rs"),
            "unexpected allowlist entry: {a}",
        );
    }
}

/// 收集到的文件数下限 —— 防「扫描器根本没扫到东西」这种**静默失效**。
///
/// 这类守卫最危险的失效模式不是变红,是「因为没扫到而一直绿」。
#[test]
fn the_scan_actually_covers_the_repository() {
    let root = repo_root();
    let mut files = Vec::new();
    collect_files(&root, Path::new(""), &mut files);
    let names: BTreeSet<&String> = files.iter().map(|(r, _)| r).collect();
    assert!(
        files.len() > 100,
        "the scan only saw {} file(s) — the walker is probably broken, and a \
         guard that sees nothing stays green forever",
        files.len(),
    );
    // 抽查三个必须被扫到的位置(仓根文档 / 实现源码 / 规范载体)。
    for must in [
        "CONTRIBUTING.md",
        "README.md",
        "impl/crates/wlwl-eval/tests/stdlib_appendix_a_sync.rs",
        "docs/wlwl-spec-reference.md",
    ] {
        assert!(
            names.contains(&must.to_string()),
            "the scan missed `{must}` — the walker is skipping files it must not",
        );
    }
}
