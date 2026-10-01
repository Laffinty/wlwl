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
