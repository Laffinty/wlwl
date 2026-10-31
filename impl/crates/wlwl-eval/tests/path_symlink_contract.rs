//! [T-05 · 审计 §5.3 ⑭] **符号链接逃逸**的专项契约 —— `std.http::STATIC_FILE`。
//!
//! ## 这张契约表守的是什么
//!
//! `STATIC_FILE(root, path, opts)` 用 `std.fs::PATH_CONTAINS` 做**词法**穿越
//! 防护 ⇒ 根内一个**指向根外**的链接（`root/link -> ../outside`）**仍判合法**
//! ⇒ 请求 `/link/secret.txt` 会把**根外**的文件端出去（审计 §5.3 ⑭ 登记的
//! 那条边界；§6.3 T-05）。
//!
//! ⇒ 本文件钉的是「`STATIC_FILE` **不再**把根外的文件端出去」：
//! **状态码必须是 403 / 404**，且**响应体不得**是根外那份文件的字节。
//!
//! ## ⚠ 为什么**不用** `PATH_CANONICAL` 当积木
//!
//! `std.fs::PATH_CANONICAL` 是**纯词法**归一（`fs.rs` 的 `canonicalize()` 只做
//! 「分隔符统一 + `.`/`..` 消解」，**不访问文件系统**）⇒ 它**看不见**符号链接。
//! ⇒ 「先 canonicalize 再判包含」在这个问题上是**空操作**。
//!
//! ## ⚠ 符号链接**建不起来**时怎么办（skip 的口径）
//!
//! · **POSIX**：`symlink(2)` 不需要特权 ⇒ **真的建** ⇒ unix 上这条契约**必跑**。
//! · **Windows**：`symlink_dir` / `symlink_file` 要特权（开发者模式 / 管理员）
//!   ⇒ 建不起来就**退回 junction**（`mklink /J`，**不需要**特权）⇒ junction
//!   也是「指向根外的目录链接」⇒ 逃逸形状**同款** ⇒ 契约照样跑。
//! · **两者都建不起来** ⇒ **skip**：打印一行说明后 `return`（测试绿，但**没
//!   覆盖**）⇒ 读数里必须看得见那行 skip —— **不许**把 skip 当成「验过了」。
//!
//! ## ⚠ 反向守卫（**本表的核心证据**）
//!
//! 把 `http.wll` 里 `STATIC_FILE` 的**第二道闸**（`_STATIC_NO_ESCAPE` 那两次
//! 调用）去掉 ⇒ 本文件**必须红**（读到 `outside/secret.txt` 的内容）⇒
//! 恢复 ⇒ 再跑一遍绿。⇒ 「去掉守卫会红」这件事**必须实测**，不能只写在注释里。

mod common;

use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use wlwl_eval::Evaluator;

/// 仓库里 `wlwl-std` 的 R1 源目录（`wl/std`）。
fn repo_std_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../wlwl-std/wl/std")
}

/// 覆盖轨的目录：仓库的 `wl/std/*.wll`（**逐字照抄** ⇒ 测的就是真正生效的那份）。
fn std_src_dir() -> &'static Path {
    static CELL: OnceLock<PathBuf> = OnceLock::new();
    CELL.get_or_init(|| {
        let dir = common::scratch("wlwl_path_symlink_src");
        let repo = repo_std_dir();
        for entry in std::fs::read_dir(&repo).expect("read the repo std dir") {
            let p = entry.expect("dir entry").path();
            if p.extension().and_then(|e| e.to_str()) == Some("wll") {
                let name = p.file_name().expect("file name");
                std::fs::copy(&p, dir.join(name)).expect("copy a std source");
            }
        }
        dir
    })
}

/// 跑一段程序 ⇒ 渲染串（失败 ⇒ `!EXXXX 消息`）。
fn actual(src: &str) -> String {
    let ast = wlwl_parser::parse(src, "t.wll")
        .unwrap_or_else(|e| panic!("case source must parse: {src}\n{e}"));
    let mut ev = Evaluator::new()
        .with_base_dir(common::scratch("wlwl_path_symlink_case"))
        .with_std_src(std_src_dir().to_path_buf());
    match ev.eval(&ast) {
        Ok(v) => v.display(),
        Err(e) => {
            let d = e.diagnostic();
            let code = d.code.as_str();
            let loc = d.location.clone();
            if d.message.is_empty() {
                format!("!{code} at {loc:?}")
            } else {
                format!("!{code} {} at {loc:?}", d.message)
            }
        }
    }
}

/// 平台路径 ⇒ wlwl 路径字面量（分隔符恒 `/`）。
fn wlwl_path(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/").replace('"', "\\\"")
}

/// 建一个**指向目录**的链接 ⇒ 建成了吗？
///
/// ⚠ Windows 上 `symlink_dir` 要特权（开发者模式 / 管理员）⇒ 建不起来就
///   **退回 junction**（`mklink /J`，**不需要**特权）⇒ junction 也是
///   「指向根外的目录链接」⇒ 逃逸形状**同款**。
/// ⚠ 两者都建不起来 ⇒ `false`（调用方 **skip**，不假装覆盖过）。
#[cfg(unix)]
fn make_dir_link(target: &Path, link: &Path) -> bool {
    std::os::unix::fs::symlink(target, link).is_ok()
}

#[cfg(windows)]
fn make_dir_link(target: &Path, link: &Path) -> bool {
    if std::os::windows::fs::symlink_dir(target, link).is_ok() {
        return true;
    }
    // ⚠ junction（目录联接）—— **不需要特权**（`mklink /J`）。
    let out = std::process::Command::new("cmd")
        .args(["/c", "mklink", "/J"])
        .arg(link)
        .arg(target)
        .output();
    matches!(out, Ok(o) if o.status.success())
}

/// 建一个**指向文件**的链接 ⇒ 建成了吗？
///
/// ⚠ Windows 上 `symlink_file` 要特权 ⇒ 建不起来就 `false`（调用方 **skip**）。
/// ⚠ **不做硬链接**兜底：硬链接与原文件**不可区分**（`lstat` 与 `stat` 完全
///   相同）⇒ 任何「链接逃逸」的判定都挡不住它 ⇒ 拿它当契约只会钉一条**假的**。
#[cfg(unix)]
fn make_file_link(target: &Path, link: &Path) -> bool {
    std::os::unix::fs::symlink(target, link).is_ok()
}

#[cfg(windows)]
fn make_file_link(target: &Path, link: &Path) -> bool {
    std::os::windows::fs::symlink_file(target, link).is_ok()
}

/// 夹具：文档根 + **根外**的 `outside/` + 根内**指向根外**的链接。
struct Fixture {
    root: PathBuf,
    /// 根外那份**不许**被端出去的文件。
    secret: PathBuf,
    /// 根外那份 `index.html`（`/link` 这一路要拿它当「不该被端出去的内容」）。
    outside_index: PathBuf,
    /// 「指向根外的**目录**链接」建成了吗？
    dir_link: bool,
    /// 「指向根外的**文件**链接」建成了吗？
    file_link: bool,
}

fn fixture() -> &'static Fixture {
    static CELL: OnceLock<Fixture> = OnceLock::new();
    CELL.get_or_init(|| {
        let base = common::scratch("wlwl_path_symlink");
        let root = base.join("root");
        let outside = base.join("outside");
        std::fs::create_dir_all(root.join("sub")).expect("create the doc root");
        std::fs::create_dir_all(&outside).expect("create the outside dir");
        std::fs::write(root.join("index.html"), b"<h1>root</h1>\n").expect("index.html");
        std::fs::write(root.join("a.txt"), b"hello static file\n").expect("a.txt");
        std::fs::write(root.join("sub/index.html"), b"<h1>sub</h1>\n").expect("sub/index.html");
        let secret = outside.join("secret.txt");
        std::fs::write(&secret, b"TOP SECRET\n").expect("secret.txt");
        let outside_index = outside.join("index.html");
        std::fs::write(&outside_index, b"<h1>OUTSIDE</h1>\n").expect("outside index.html");
        let dir_link = make_dir_link(&outside, &root.join("link"));
        let file_link = make_file_link(&secret, &root.join("flink"));
        Fixture {
            root,
            secret,
            outside_index,
            dir_link,
            file_link,
        }
    })
}

/// 用例程序的 IMPORT 头。
///
/// ⚠ `CAT` 是 `std.str` 的**命名空间成员**（不是全局内建 —— 全局内建是 `STR`
/// 与 `SPLIT`）⇒ 必须显式 `IMPORT`，否则用例在**渲染阶段**就撞
/// `E0020 undefined name CAT`，而那不是被测行为（会伪装成「STATIC_FILE 出错」）。
const IMP: &str = concat!(
    r#"IMPORT("wlwl:std.http", ["STATIC_FILE"]);"#,
    "\n",
    r#"IMPORT("wlwl:std.fs", ["READ_BYTES"]);"#,
    "\n",
    r#"IMPORT("wlwl:std.str", ["CAT"]);"#,
    "\n",
);

/// 请求一次 ⇒ `(状态码, 是否把 `forbidden` 那份内容端出去了)`。
fn serve(root: &Path, path: &str, forbidden: &Path) -> (i64, bool) {
    let src = format!(
        "{IMP} LET(r, STATIC_FILE(\"{}\", \"{}\", NULL)); \
         CAT(STR(r[\"status\"]), \" \", STR(==(r[\"body\"], READ_BYTES(\"{}\"))))",
        wlwl_path(root),
        path,
        wlwl_path(forbidden)
    );
    let got = actual(&src);
    let mut parts = got.split(' ').map(str::to_string);
    let status = parts
        .next()
        .expect("the render must have a status")
        .parse::<i64>()
        .unwrap_or_else(|_| panic!("unexpected render: {got}"));
    let leaked = parts.next().unwrap_or_default() == "TRUE";
    (status, leaked)
}

/// 「状态码是 403 / 404」＋「**没有**端出根外那份内容」—— 两条一起断言。
fn assert_refused(what: &str, status: i64, leaked: bool) {
    assert!(
        !leaked,
        "{what}: STATIC_FILE 把**根外**那份文件的内容端出去了（status={status}）"
    );
    assert!(
        status == 403 || status == 404,
        "{what}: 期望 403 / 404，实际 {status}"
    );
}

// ── 逃逸用例（**必须**被挡住）──────────────────────────────────────────

/// 根内一个**指向根外的目录链接** ⇒ `/link/secret.txt` **不得**被端出去。
#[test]
fn a_symlinked_directory_out_of_the_root_does_not_serve_the_outside_file() {
    let fx = fixture();
    if !fx.dir_link {
        eprintln!(
            "skip: 这个平台上建不出「指向根外的目录链接」（symlink 要特权，junction 也不可用）"
        );
        return;
    }
    let (status, leaked) = serve(&fx.root, "/link/secret.txt", &fx.secret);
    assert_refused("目录链接 ⇒ /link/secret.txt", status, leaked);
}

/// 同一条链接，但请求的是**它自己**（⇒ 服务的是链接**里面**的 `index.html`）。
#[test]
fn a_symlinked_directory_out_of_the_root_does_not_serve_the_outside_index() {
    let fx = fixture();
    if !fx.dir_link {
        eprintln!("skip: 这个平台上建不出「指向根外的目录链接」");
        return;
    }
    let (status, leaked) = serve(&fx.root, "/link", &fx.outside_index);
    assert_refused("目录链接 ⇒ /link", status, leaked);
}

/// 根内一个**指向根外的文件链接** ⇒ `/flink` **不得**被端出去。
#[test]
fn a_symlinked_file_out_of_the_root_does_not_serve_the_outside_file() {
    let fx = fixture();
    if !fx.file_link {
        eprintln!("skip: 这个平台上建不出文件符号链接（Windows 上 `symlink_file` 要特权）");
        return;
    }
    let (status, leaked) = serve(&fx.root, "/flink", &fx.secret);
    assert_refused("文件链接 ⇒ /flink", status, leaked);
}

// ── 回归守卫（新闸**不许**误伤正常路径）──────────────────────────────

/// 根下的**普通文件**照旧 200。
#[test]
fn a_normal_file_is_still_served() {
    let fx = fixture();
    let (status, leaked) = serve(&fx.root, "/a.txt", &fx.secret);
    assert_eq!(status, 200, "普通文件必须照旧被服务");
    assert!(!leaked);
}

/// 根下的**真子目录**照旧 200（逐段下探要能走进真目录）。
#[test]
fn a_real_subdirectory_is_still_served() {
    let fx = fixture();
    let (status, leaked) = serve(&fx.root, "/sub/", &fx.secret);
    assert_eq!(status, 200, "真子目录必须照旧被服务");
    assert!(!leaked);
}

/// 根目录本身（`/` ⇒ `index.html`）照旧 200。
#[test]
fn the_root_itself_is_still_served() {
    let fx = fixture();
    let (status, leaked) = serve(&fx.root, "/", &fx.secret);
    assert_eq!(status, 200, "根目录必须照旧被服务");
    assert!(!leaked);
}
