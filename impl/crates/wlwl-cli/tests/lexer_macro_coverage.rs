//! [v0.10.1 / R10-064 + 收尾] `dispatch: LexerMacro` 的条目**从来没有被任何
//! 锁测试确认过**。
//!
//! 现有的 `b11_registry_covers_resolve_builtin` 只对
//! `ResolvedBuiltin` / `ResolvedCompat` 断言「`resolve_builtin(name)` 能解析到」。
//! `LexerMacro` 的语义是「**lexer 产生一个记号,parser 把它降级成 `Expr::*`**」
//! —— 而**没有任何一条测试检查 lexer 真的会产生那个记号**。
//!
//! 代价是四个「三份副本一起说它能用、实现里没有」的条目一路走到发布:
//!
//! 历史上逮到 5 个,现在**一个都不剩** —— 它们都按实测从注册表与附录 G 移除了:
//!
//! | 条目 | 处置 |
//! |---|---|
//! | `ARRAY(items...)` / `DICT(pairs...)` | 签名里的带参形式不存在(零参可用)→ 签名改写(D10-013) |
//! | `AND` / `OR` | `E0020: undefined name` → **移除** |
//! | `MODULE` | `E0020: undefined name` → **移除**(文法里本就没有 `ModuleDecl` 那一支) |
//! | `EXPECT_ERR` | 全局那个 `E0020`;同名可用的只有 `wlwl:std.test` 的导出 → **移除** |
//!
//! 所以 `KNOWN_NOT_LOWERED` 现在是**空的**。这不是「暂时没有」,而是「下一个出现
//! 幽灵就该红」的起点。
//!
//! ## 本文件做什么
//!
//! **不**手抄一份「这些是幽灵」的名单 —— 那样第五个出现时还是绿的。
//! 这里对每个 `LexerMacro` 条目**实跑**一次 `wlwl check`,把「调不通的」集合
//! **算出来**,再与已知的幽灵名单比对:
//!
//! * 出现**新的**幽灵 → 红(这是本文件的主要目的);
//! * 修好一个(把它从名单里删掉)→ 绿,并提醒同步改附录 G。
//!
//! 这是一个**棘轮**:它把「注册表条目是否真的能用」从一次性的人工核对变成
//! 每次 `cargo test` 都会重跑的判据。

use std::path::PathBuf;
use std::process::Command;

/// 已知调不通、且**暂时豁免**的条目。
///
/// **现在是空的** —— 历史上那 5 个(`ARRAY` / `DICT` 的带参形式、`AND` / `OR`、
/// `MODULE`、全局 `EXPECT_ERR`)已全部按实测处置:要么改签名,要么从注册表与
/// 附录 G **整个移除**。
///
/// 保留这个常量(而不是删掉判据)是为了让「以后再出现一个」这件事**默认红**。
/// 若某条目确实需要豁免,在此登记并**同时**在附录 G 标注 —— 只登记豁免而不改文档,
/// 就是本文件要消灭的那种「三份副本各说各话」。
///
/// 探针用 `NAME();` 就够:名字解析不到的会报 `undefined name`,能解析到的
/// 报的是元数错(`E0022`)、解析错(`E0010` / `E0011`)。两者不会混。
const KNOWN_NOT_LOWERED: &[&str] = &[];

fn wlwl_binary() -> PathBuf {
    if let Ok(p) = std::env::var("CARGO_BIN_EXE_wlwl") {
        let p = PathBuf::from(p);
        if p.exists() {
            return p;
        }
    }
    panic!("wlwl binary not found; run `cargo build --bin wlwl` first");
}

fn lexermacro_names() -> Vec<&'static str> {
    wlwl_eval::registry::BUILTIN_REGISTRY
        .iter()
        .filter(|s| matches!(s.dispatch, wlwl_eval::registry::DispatchStatus::LexerMacro))
        .map(|s| s.name)
        .collect()
}

/// 实跑 `NAME();`,看它是不是报 `undefined name`。
///
/// **必须用 `run` 而不是 `check`。** `E0020: undefined name` 是**运行期**
/// 诊断:`wlwl check` 只做解析与静态检查,`AND();` 在它那里是 `OK: parsed`。
/// 第一版这里写的是 `check`,于是主测试**假通过**(25 个条目一个都没报
/// `undefined name`,`newly_broken` 空得像全对),是下面那条反向棘轮把它
/// 逮住的 —— 名单里的三条「其实好了」全都是误报。
/// 教训:校验器**自己**也要被校验,否则「全绿」和「全瞎」长得一模一样。
///
/// 刻意**不**关心别的诊断:元数不对会给 `E0022`,但那恰恰说明名字**解析到了** ——
/// 我们要抓的只有「名字根本没被 lexer / parser 认出来」这一种。
fn is_not_lowered(bin: &PathBuf, name: &str) -> (bool, String) {
    // 临时目录**必须按名字分开**。两条测试是并行的,共用一个目录时
    // 一边刚写下的 `main.wll` 会被另一边读走 —— 表现为「AND 忽然不报
    // undefined name 了,附的却是 IS_OK 的输出」这种完全对不上的失败。
    let dir = std::env::temp_dir()
        .join("wlwl-lexermacro-probe")
        .join(format!("{}-{}", std::process::id(), name));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("staging dir");
    std::fs::write(dir.join("main.wll"), format!("{name}();\n")).expect("write probe");
    let out = Command::new(bin)
        .arg("run")
        .arg(dir.join("main.wll"))
        .output()
        .expect("wlwl run runs");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let _ = std::fs::remove_dir_all(&dir);
    (text.contains(&format!("undefined name `{name}`")), text)
}

/// 每个 `LexerMacro` 条目都必须真的能被 lexer 降级 —— 除非它在
/// [`KNOWN_NOT_LOWERED`] 里被显式登记为已知幽灵。
#[test]
fn every_lexer_macro_entry_is_actually_lowered() {
    let bin = wlwl_binary();
    let names = lexermacro_names();
    assert!(
        !names.is_empty(),
        "no LexerMacro entries found — the registry filter is broken, not the registry"
    );

    let mut newly_broken = Vec::new();
    for name in &names {
        let (not_lowered, out) = is_not_lowered(&bin, name);
        if not_lowered && !KNOWN_NOT_LOWERED.contains(name) {
            newly_broken.push(format!("{name}: {}", out.replace('\n', " | ").trim()));
        }
    }

    assert!(
        newly_broken.is_empty(),
        "{} LexerMacro entr(y/ies) are registered but the parser cannot lower them \
         (`E0020: undefined name`).\n\
         `dispatch: LexerMacro` claims the *lexer* produces a token for them; nothing \
         checked that, which is how AND / OR / MODULE got published.\n\
         Fix: either give the lexer the token, or move the entry to `ResolvedBuiltin`\n\
         (and register it in `resolve_builtin`), or drop it from the registry — and in\n\
         every case update Appendix G, whose signature column is locked against this\n\
         registry by `spec_appendix_g_sync`.\n  {}",
        newly_broken.len(),
        newly_broken.join("\n  ")
    );
}

/// 反向棘轮:已登记的幽灵里,若有哪个**其实已经能用了**,说明有人修了实现却
/// 忘了把它移出名单。留着它会让上面那条从「永久豁免」退化成「永久忽略」。
#[test]
fn the_known_not_lowered_list_is_not_stale() {
    let bin = wlwl_binary();
    let mut still_broken = Vec::new();
    for name in KNOWN_NOT_LOWERED {
        let (not_lowered, out) = is_not_lowered(&bin, name);
        if !not_lowered {
            still_broken.push(format!(
                "{name} no longer reports `undefined name` ({}) — remove it from \
                 KNOWN_NOT_LOWERED and fix Appendix G accordingly",
                out.replace('\n', " | ").trim()
            ));
        }
    }
    assert!(
        still_broken.is_empty(),
        "KNOWN_NOT_LOWERED is stale:\n  {}",
        still_broken.join("\n  ")
    );
}
