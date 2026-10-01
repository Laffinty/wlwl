//! [v0.11 M1-2 / ADR-0021] R1 标准库双轨加载的开发覆盖锁。
//!
//! 规则(stdlib 规范 §0.5):`--std-src <dir>` / 环境变量 `WLWL_STD_SRC`
//! 仅覆盖 R1 语言层的加载路径,**不得**改变任何成员的名字与语义。
//! 本文件锁四件事:
//!
//! 1. 默认轨(嵌入源码)行为不变;
//! 2. 同导出面的覆盖轨可加载,且**真实生效**(输出切换为覆盖实现,
//!    证明加载路径确实走了覆盖目录,不是静默回退嵌入版);
//! 3. 环境变量通道与 flag 通道等价;
//! 4. 导出面漂移(覆盖模块缺成员)必须 `E0023` 拒绝 —— 覆盖不完整
//!    是开发者的错,绝不能悄悄拿嵌入版兜底。

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

struct Tmp(PathBuf);

impl Tmp {
    fn new(tag: &str) -> Self {
        let p = std::env::temp_dir().join(format!(
            "wlwl_dual_{}_{}_{}",
            tag,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).expect("tmp project dir created");
        Tmp(p)
    }
    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// `bin args` → (exit code, 合并的 stdout+stderr)。断言只对合并输出做,
/// 因为诊断走 stderr、PRINT 走 stdout,合并口径最稳。
fn run(args: &[&str], std_src_env: Option<&Path>, cwd: &Path) -> (i32, String) {
    let mut c = Command::new(env!("CARGO_BIN_EXE_wlwl"));
    c.current_dir(cwd);
    // 清掉宿主可能带进来的覆盖变量,默认轨断言才是确定性的。
    c.env_remove("WLWL_STD_SRC");
    if let Some(dir) = std_src_env {
        c.env("WLWL_STD_SRC", dir);
    }
    let out = c.args(args).output().expect("wlwl binary runs");
    (
        out.status.code().unwrap_or(-1),
        format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ),
    )
}

#[test]
fn std_lang_dual_track_override_never_changes_the_export_surface() {
    let tmp = Tmp::new("main");
    let proj = tmp.path().join("proj");
    fs::create_dir_all(&proj).unwrap();
    fs::write(
        proj.join("main.wll"),
        "IMPORT(\"wlwl:std.str\", [\"QUOTE\"]);\nPRINT(QUOTE(\"ab\"));\n",
    )
    .unwrap();

    // 1. 默认轨:嵌入 QUOTE 正常,输出带引号。
    let (code, out) = run(&["run", "main.wll"], None, &proj);
    assert_eq!(code, 0, "embedded track must run:\n{out}");
    assert_eq!(
        out.trim_end(),
        "\"ab\"",
        "embedded QUOTE output must be the quoted string:\n{out}"
    );

    // 2. 覆盖轨(同导出面、不同实现):可加载且真实生效 —— 恒等实现
    //    不加引号,输出切成了 `ab`。
    let ovr = tmp.path().join("ovr");
    fs::create_dir_all(&ovr).unwrap();
    fs::write(
        ovr.join("str.wll"),
        "LET(QUOTE, FUN((s), s));\nEXPORT([\"QUOTE\"]);\n",
    )
    .unwrap();
    let (code, out) = run(
        &["run", "--std-src", ovr.to_str().unwrap(), "main.wll"],
        None,
        &proj,
    );
    assert_eq!(code, 0, "override track must run:\n{out}");
    assert_eq!(
        out.trim_end(),
        "ab",
        "override QUOTE did not take effect (still the embedded impl?):\n{out}"
    );

    // 3. 环境变量通道与 flag 通道等价。
    let (code, out) = run(&["run", "main.wll"], Some(&ovr), &proj);
    assert_eq!(code, 0, "env override track must run:\n{out}");
    assert_eq!(
        out.trim_end(),
        "ab",
        "env override did not take effect:\n{out}"
    );

    // 4. 漂移守卫:覆盖模块丢了 QUOTE → E0023,绝不静默回退嵌入版。
    let bad = tmp.path().join("bad");
    fs::create_dir_all(&bad).unwrap();
    fs::write(bad.join("str.wll"), "// override module with no exports\n").unwrap();
    let (code, out) = run(
        &["run", "--std-src", bad.to_str().unwrap(), "main.wll"],
        None,
        &proj,
    );
    assert_ne!(code, 0, "drifted override must fail:\n{out}");
    assert!(
        out.contains("E0023"),
        "expected E0023 on export drift:\n{out}"
    );
}

/// Copy every embedded R1 sibling into an override dir.
///
/// The `--std-src` channel is **全量替换**, not a per-module fallback: a
/// module missing from the dir is E0040, not "use the embedded copy". So
/// any override fixture must carry the whole set — M1-2's test got away with
/// one file because `std.str` was the only R1 module at the time.
fn seed_override_dir(dir: &Path) {
    for name in ["str.wll", "math.wll", "collection.wll", "test.wll"] {
        let embedded = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("wlwl-std")
            .join("wl")
            .join("std")
            .join(name);
        fs::copy(&embedded, dir.join(name))
            .unwrap_or_else(|e| panic!("copy {} into {}: {e}", embedded.display(), dir.display()));
    }
}

/// [v0.11 M3-4] 混合模块的覆盖轨:门面换掉,导出面**不得**变。
///
/// 比 M1-2 多覆盖的那一块正是 M3 立起来的机制:**kernel 注入**。覆盖轨换掉
/// 的是 `.wll` 门面源码,而 kernel 注入发生在 **Rust 侧**(`load_std_lang`
/// 在 `eval_module` 之前绑定),两轨都得注入 —— 所以这一条同时验证:
///
/// 1. 覆盖轨真的生效(输出切换,不是静默回退嵌入版);
/// 2. 覆盖轨的门面照样能调到注入的 R2 浮点内核;
/// 3. 导出面漂移(成员改名)仍必须 E0023,绝不静默回退嵌入版。
#[test]
fn std_math_mixed_override_keeps_the_export_surface_and_reaches_kernels() {
    let tmp = Tmp::new("math");
    let proj = tmp.path().join("proj");
    fs::create_dir_all(&proj).unwrap();
    fs::write(
        proj.join("main.wll"),
        "IMPORT(\"wlwl:std.math\", [\"ROOT4\"]); PRINT(ROOT4(16));\n",
    )
    .unwrap();

    let ovr = tmp.path().join("ovr");
    fs::create_dir_all(&ovr).unwrap();
    seed_override_dir(&ovr);
    fs::write(
        ovr.join("math.wll"),
        "LET(ROOT4, FUN((x), *(_SQRT(x), 2)));\nEXPORT([\"ROOT4\"]);\n",
    )
    .unwrap();

    let (code, out) = run(
        &["run", "--std-src", ovr.to_str().unwrap(), "main.wll"],
        None,
        &proj,
    );
    assert_eq!(code, 0, "override track must run:\n{out}");
    assert_eq!(
        out.trim_end(),
        "8.0",
        "the override facade must reach the injected `_SQRT` kernel, and the \
         override must actually take effect (if this printed the embedded \
         facade's answer, the output would differ):\n{out}"
    );

    // 漂移守卫:覆盖模块把成员改名 → E0023。
    // 覆盖文件用 raw string:原文里混进了字面量 `` `n ``(反引号 n),
    // 拼进 .wll 源里会让「注释跨两行」这件事只存在于人的想象里。
    let bad = tmp.path().join("bad");
    fs::create_dir_all(&bad).unwrap();
    seed_override_dir(&bad);
    fs::write(
        bad.join("math.wll"),
        "// override module with no exports at all: the importer asks for ROOT4,\n\
         //   the module exports nothing -> E0023 (never a silent fallback).\n",
    )
    .unwrap();
    let (code, out) = run(
        &["run", "--std-src", bad.to_str().unwrap(), "main.wll"],
        None,
        &proj,
    );
    assert_ne!(code, 0, "drifted override must fail:\n{out}");
    assert!(
        out.contains("E0023"),
        "expected E0023 on export drift:\n{out}"
    );
}

/// [v0.11 M3-4] `std.collection` 覆盖轨:纯 R1 模块同样能被换掉。
#[test]
fn std_collection_override_keeps_the_export_surface() {
    let tmp = Tmp::new("coll");
    let proj = tmp.path().join("proj");
    fs::create_dir_all(&proj).unwrap();
    fs::write(
        proj.join("main.wll"),
        "IMPORT(\"wlwl:std.collection\", [\"MAP\"]); PRINT(MAP([1, 2], FUN((x), *(x, 10))));\n",
    )
    .unwrap();

    let (code, out) = run(&["run", "main.wll"], None, &proj);
    assert_eq!(code, 0, "default track must run:\n{out}");
    assert_eq!(out.trim_end(), "[10, 20]");

    let ovr = tmp.path().join("ovr");
    fs::create_dir_all(&ovr).unwrap();
    seed_override_dir(&ovr);
    fs::write(
        ovr.join("collection.wll"),
        "LET(MAP, FUN((arr, f), [\"OVERRIDE-MAP\"]));\nEXPORT([\"MAP\"]);\n",
    )
    .unwrap();
    let (code, out) = run(
        &["run", "--std-src", ovr.to_str().unwrap(), "main.wll"],
        None,
        &proj,
    );
    assert_eq!(code, 0, "override track must run:\n{out}");
    assert_eq!(
        out.trim_end(),
        "[OVERRIDE-MAP]",
        "override MAP did not take effect (still the embedded impl?):\n{out}"
    );
}
