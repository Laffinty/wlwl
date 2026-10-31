//! [W-20] `std.http::GET("https://…")` 的 **HTTPS 真实握手** —— 端到端契约。
//!
//! ## 这条契约守的是什么
//!
//! 审计报告（`docs/plan/wlwl-v0.12.1-functional-audit.md` §5.3 的 **W-20** 行）登记的
//! 缺口是：**HTTPS 的真实握手从未被测过**。既有覆盖只有两条：
//!
//! 1. `https://` 且**没有加载扩展** ⇒ `ERR([kind:"TlsError", reason:"no TLS provider
//!    loaded"])`（在 `wlwl-eval` 的 `http_w20_contract.rs` 里，用**假**的 provider 注入）；
//! 2. 证书错误**逐字透传**（同上）。
//!
//! ⇒ 缺的是那一条**真**的：**真扩展、真证书、真握手、拿到 200**。本文件补的就是它。
//!
//! ## 为什么在 CLI 层（而不是 `wlwl-eval` 层）
//!
//! 真握手要经**宿主的 `--ext` 加载通道**（`wlwl --ext <绝对路径>`）⇒ 那条通道只在
//! CLI 里 ⇒ 用例驱动的是**真的 `wlwl` 二进制**（`CARGO_BIN_EXE_wlwl`），跑的是**真的
//! `.wll` 程序** ⇒ 覆盖的是「宿主 + 扩展 + 客户端」这条**完整**链路，而不是某一层。
//!
//! ## 为什么服务端与客户端是**两个进程**
//!
//! 扩展 ABI v1 **没有就绪事件通道**（`await_ready` 恒返回 `-1`）⇒ 握手 / accept /
//! 读 / 写**都在调用线程上阻塞**，**等待期间整个运行时是冻结的**（扩展 README §6）。
//!
//! ⇒ **同一个进程里**先 `tls::ACCEPT`（阻塞）再让客户端去连 ⇒ 死锁。
//! ⇒ 所以：**服务端一个进程**（`tls::LISTEN` + `tls::ACCEPT`），**客户端另一个进程**
//! （`std.http::GET`）—— 两边**各自加载同一份扩展**。
//!
//! ## ⚠ 绝不访问外网
//!
//! 服务端只绑 `127.0.0.1:0`（临时端口）⇒ **一个外部地址都不碰**（与仓里既有的
//! `the_contract_never_touches_an_external_address` 同款纪律）。
//!
//! ## 证书夹具
//!
//! `tests/fixtures/` 下的**测试专用**自签材料（CA + 叶证书，**100 年**有效期）——
//! 详见那个目录的 `README.md`。⚠ **仅 127.0.0.1**、**无 CA 信任**、**私钥是明文**、
//! **不得用于任何真实服务**。

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::JoinHandle;
use std::time::Duration;

// ── 标记（脚本与断言之间的契约）────────────────────────────────────────────

/// 服务端：监听成功后打印它拿到的临时端口。
const PORT_MARKER: &str = "WLWL_E2E_PORT=";
/// 服务端：`ACCEPT` 的结果（值是 `ERROR` = 握手失败 / `TIMEOUT` = 等不到连接）。
const ACCEPT_MARKER: &str = "WLWL_E2E_ACCEPT=";
/// 服务端：握手失败（错误的客户端 / 证书不被信任）。
const ACCEPT_ERROR: &str = "ERROR";
/// 服务端：等不到连接（超时）。
const ACCEPT_TIMEOUT: &str = "TIMEOUT";
/// 服务端：读到的请求字节数。
const REQUEST_MARKER: &str = "WLWL_E2E_REQUEST=";
/// 服务端：写出的响应字节数。
const SENT_MARKER: &str = "WLWL_E2E_SENT=";
/// 客户端：HTTP 状态码。
const STATUS_MARKER: &str = "WLWL_E2E_STATUS=";
/// 客户端：响应体。
const BODY_MARKER: &str = "WLWL_E2E_BODY=";
/// 客户端：`ERR` 载荷（`std.http::GET` 失败时）。
const ERROR_MARKER: &str = "WLWL_E2E_ERROR=";

/// 服务端 `ACCEPT` 的等待预算（毫秒）。
const ACCEPT_TIMEOUT_MS: i64 = 30_000;
/// 服务端读请求的预算（毫秒）。
const RECV_TIMEOUT_MS: i64 = 15_000;
/// 测试**这边**等服务端报标记的预算（见 `Server::wait_marker`）。
const MARKER_WAIT: Duration = Duration::from_secs(10);

// ── 仓内路径 ───────────────────────────────────────────────────────────────

/// `impl/` 仓根（`crates/wlwl-cli` 的上两级）。
fn impl_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("the wlwl-cli manifest must live in <impl>/crates/wlwl-cli")
        .to_path_buf()
}

/// 证书夹具目录。
fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
}

/// 一个夹具文件的**绝对**路径。
fn fixture(name: &str) -> PathBuf {
    fixtures_dir().join(name)
}

/// 宿主二进制（cargo 为同包的集成测试设的 `CARGO_BIN_EXE_<name>`）。
fn wlwl_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_wlwl"))
}

/// 一个路径 ⇒ **WLWL 字符串字面量**。
///
/// ⚠ WLWL 的字符串里 `\` 是**转义符** ⇒ 一律换成正斜杠（Windows 也认正斜杠）；
/// `"` 要转义（路径里几乎不会出现，但字面量坏了就是整条用例坏）。
fn wlwl_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "/")
        .replace('"', "\\\"")
}

// ── 扩展产物 ───────────────────────────────────────────────────────────────

/// 扩展动态库可能的文件名（按平台）。
///
/// ⚠⚠ **Linux / macOS 的 cdylib 带 `lib` 前缀**（`libwlwl_tls.so` /
/// `libwlwl_tls.dylib`）⇒ **不能**只写 `wlwl_tls.so` ⇒ 两种都找。
/// ⚠ Windows 的 DLL **不带**前缀（`wlwl_tls.dll`）。
fn ext_file_names() -> &'static [&'static str] {
    if cfg!(target_os = "windows") {
        &["wlwl_tls.dll"]
    } else if cfg!(target_os = "macos") {
        &["libwlwl_tls.dylib", "wlwl_tls.dylib"]
    } else {
        &["libwlwl_tls.so", "wlwl_tls.so"]
    }
}

/// 在扩展自己的 `target/` 里找产物（`release` 优先）。
fn find_ext_artifact() -> Option<PathBuf> {
    let target = impl_root().join("ext").join("wlwl-tls-ext").join("target");
    for profile in ["release", "debug"] {
        for name in ext_file_names() {
            let candidate = target.join(profile).join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

/// 构建扩展（**独立 workspace**，所以要单独 `cargo build`）。
///
/// ⚠ `--target-dir` **显式**给 ⇒ 不受环境里的 `CARGO_TARGET_DIR` 影响 ⇒
/// 产物位置是确定的。
/// ⚠ 扩展**不在**主 workspace 里（它自己的 `Cargo.toml` 有空 `[workspace]` 表）
/// ⇒ 主仓的 `Cargo.lock` **不会**因为它多一行。
fn build_ext_artifact() {
    let ext_dir = impl_root().join("ext").join("wlwl-tls-ext");
    let manifest = ext_dir.join("Cargo.toml");
    let target_dir = ext_dir.join("target");
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let out = Command::new(&cargo)
        .arg("build")
        .arg("--release")
        .arg("--locked")
        .arg("--manifest-path")
        .arg(&manifest)
        .arg("--target-dir")
        .arg(&target_dir)
        .output()
        .unwrap_or_else(|e| panic!("cannot run '{cargo} build' for the TLS extension: {e}"));
    assert!(
        out.status.success(),
        "building the TLS extension failed ({}); build it by hand with:\n    \
         cargo build --release --locked --manifest-path {}\n--- stderr ---\n{}",
        out.status,
        manifest.display(),
        String::from_utf8_lossy(&out.stderr)
    );
}

/// 扩展动态库的**绝对**路径 —— 找不到就**自己构建**。
///
/// ⚠ 找不到就构建（而不是跳过）：这条契约的价值全在「真握手」上 ⇒ 缺了扩展就
/// **响亮地失败**，附上手工构建的命令 —— 绝不静默 skip（skip ⇒ 读数是绿的，
/// 但什么也没测到）。
fn ext_artifact() -> PathBuf {
    static ARTIFACT: OnceLock<PathBuf> = OnceLock::new();
    ARTIFACT
        .get_or_init(|| {
            if let Some(found) = find_ext_artifact() {
                return found;
            }
            build_ext_artifact();
            find_ext_artifact().unwrap_or_else(|| {
                panic!(
                    "the TLS extension was built but its artifact is still missing; expected it \
                     under {}",
                    impl_root()
                        .join("ext")
                        .join("wlwl-tls-ext")
                        .join("target")
                        .display()
                )
            })
        })
        .clone()
}

// ── 临时目录 ───────────────────────────────────────────────────────────────

/// 一个用完即删的临时目录（脚本写在这儿）。
struct Scratch {
    path: PathBuf,
}

impl Scratch {
    fn new(tag: &str) -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("the system clock must be after the Unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "wlwl-https-e2e-{tag}-{}-{nanos}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).expect("cannot create the scratch directory");
        Scratch { path }
    }

    fn join(&self, name: &str) -> PathBuf {
        self.path.join(name)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

// ── 脚本 ───────────────────────────────────────────────────────────────────

/// 服务端程序：用扩展自己的 `tls::LISTEN` / `tls::ACCEPT` 起一个**真的** TLS 服务端。
///
/// ⚠ 它回**一个** HTTP/1.1 200 响应（体是 `hello`）⇒ 客户端能读到 body。
/// ⚠ **先把请求读完再回响应**（仓里既有的 Linux `RST` 教训：`http_w20_contract.rs`
///   的 `serve_pieces` 文档注释里有实测证据）—— 关连接前不读 ⇒ Linux 发 `RST`
///   而不是 `FIN` ⇒ 客户端偶发读不到完整响应。
fn server_script(cert: &Path, key: &Path) -> String {
    let response = "HTTP/1.1 200 OK\\r\\nContent-Type: text/plain\\r\\nContent-Length: 5\\r\\nConnection: close\\r\\n\\r\\nhello";
    format!(
        "// [W-20] HTTPS 真实握手契约的**服务端**（由 https_e2e_contract.rs 生成）。\n\
         //\n\
         // 它用扩展自己的 `tls::LISTEN` / `tls::ACCEPT` 起一个**真的** TLS 服务端 ⇒\n\
         // 客户端那条 `std.http::GET(\"https://…\")` 走的是**真的**握手。\n\
         IMPORT(\"tls\", [\"LISTEN\", \"LOCAL_ADDR\", \"ACCEPT\", \"RECV\", \"SEND\", \"CLOSE\"]);\n\
         \n\
         LET(listener, LISTEN(\"127.0.0.1\", 0, [\"cert\": \"{cert}\", \"key\": \"{key}\"]));\n\
         PRINT(\"{port_marker}\", AT(LOCAL_ADDR(listener), \"port\", 0));\n\
         \n\
         LET(conn, ACCEPT(listener, {accept_timeout_ms}));\n\
         IF(IS_ERR(conn),\n\
         \x20  (LET(_, conn);\n\
         \x20   PRINT(\"{accept_marker}{accept_error}\")),\n\
         \x20  (IF(==(TYPE(conn), \"NULL\"),\n\
         \x20     PRINT(\"{accept_marker}{accept_timeout}\"),\n\
         \x20     (LET(chunk, RECV(conn, 65536, {recv_timeout}));\n\
         \x20      PRINT(\"{request_marker}\", LEN(AT(chunk, \"text\", \"\")));\n\
         \x20      LET(sent, SEND(conn, \"{response}\"));\n\
         \x20      PRINT(\"{sent_marker}\", sent);\n\
         \x20      CLOSE(conn)))));\n\
         \n\
         CLOSE(listener);\n",
        cert = wlwl_path(cert),
        key = wlwl_path(key),
        port_marker = PORT_MARKER,
        accept_marker = ACCEPT_MARKER,
        accept_error = ACCEPT_ERROR,
        accept_timeout = ACCEPT_TIMEOUT,
        request_marker = REQUEST_MARKER,
        sent_marker = SENT_MARKER,
        accept_timeout_ms = ACCEPT_TIMEOUT_MS,
        recv_timeout = RECV_TIMEOUT_MS,
        response = response,
    )
}

/// 客户端程序：用**公共面**的 `std.http::GET` 走 `https://`。
///
/// ⚠ `opts[\"tls\"]` 是**逐字转交**给 `tls::CONNECT` 的（内核不解释、不改写）
/// ⇒ 这里给的是扩展自己的 `ca`（信任根）选项。
fn client_script(url: &str, ca: &Path) -> String {
    format!(
        "// [W-20] HTTPS 真实握手契约的**客户端**（由 https_e2e_contract.rs 生成）。\n\
         //\n\
         // 它走**公共面**的 `std.http::GET` ⇒ `https://` ⇒ `tls::CONNECT` ⇒ 真握手。\n\
         IMPORT(\"wlwl:std.http\", [\"GET\"]);\n\
         \n\
         LET(r, GET(\"{url}\", [\"tls\": [\"ca\": \"{ca}\"]]));\n\
         LET(failed, IS_ERR(r));\n\
         IF(failed,\n\
         \x20  (LET(payload, ERR_PAYLOAD(r));\n\
         \x20   LET(_, r);\n\
         \x20   PRINT(\"{error_marker}\", payload)),\n\
         \x20  (PRINT(\"{status_marker}\", AT(r, \"status\", 0));\n\
         \x20   PRINT(\"{body_marker}\", AT(r, \"body\", \"\"))));\n",
        url = url,
        ca = wlwl_path(ca),
        error_marker = ERROR_MARKER,
        status_marker = STATUS_MARKER,
        body_marker = BODY_MARKER,
    )
}

// ── 跑起来 ─────────────────────────────────────────────────────────────────

/// 一次子进程运行的读数。
struct Run {
    code: Option<i32>,
    stdout: String,
    stderr: String,
    /// 跑的那个程序（**失败时一并报出来** ⇒ 不用再去猜脚本长什么样）。
    script: String,
}

impl Run {
    /// 某个标记后面的值（`PRINT` 用空格连接实参 ⇒ 标记与值之间**有**空格）。
    fn marker(&self, marker: &str) -> Option<&str> {
        self.stdout
            .lines()
            .find_map(|line| line.trim().strip_prefix(marker))
            .map(str::trim)
    }

    /// 失败时的**完整**读数（含程序本身 ⇒ 不用再去猜脚本长什么样）。
    fn report(&self) -> String {
        format!(
            "--- exit ---\n{:?}\n--- stdout ---\n{}\n--- stderr ---\n{}\n--- program ---\n{}",
            self.code, self.stdout, self.stderr, self.script
        )
    }
}

/// 起一个**真的** TLS 服务端进程，并等它报出自己的临时端口。
struct Server {
    child: Child,
    port: u16,
    stdout_lines: Arc<Mutex<Vec<String>>>,
    stderr_lines: Arc<Mutex<Vec<String>>>,
    readers: Vec<JoinHandle<()>>,
}

impl Server {
    /// 客户端该连的 URL。
    fn url(&self, path: &str) -> String {
        format!("https://127.0.0.1:{}{path}", self.port)
    }

    /// 服务端 stdout 里的某个标记值。
    fn marker(&self, marker: &str) -> Option<String> {
        self.stdout_lines
            .lock()
            .expect("the server stdout lock must not be poisoned")
            .iter()
            .find_map(|line| line.trim().strip_prefix(marker))
            .map(|v| v.trim().to_string())
    }

    /// 服务端 stderr 目前的全部内容。
    fn stderr_text(&self) -> String {
        self.stderr_lines
            .lock()
            .expect("the server stderr lock must not be poisoned")
            .join("\n")
    }

    /// **等**服务端报出某个标记（最多 `timeout`）。
    ///
    /// ⚠ 为什么不能「读一次就断言」：服务端 stdout 是**另一个线程**边读边攒的 ⇒
    /// 客户端进程刚退出时，服务端那几行**可能还没**进缓冲区（Windows / macOS 的
    /// CI 机器在负载下尤其明显）⇒ 读一次会变成**偶发红**，而本仓对 flaky 是
    /// 「重跑仍判红」（`FLAKY_EXIT_CODE=1`）⇒ 必须**轮询等**，不靠运气。
    fn wait_marker(&self, marker: &str, timeout: Duration) -> Option<String> {
        let deadline = std::time::Instant::now() + timeout;
        loop {
            if let Some(value) = self.marker(marker) {
                return Some(value);
            }
            if std::time::Instant::now() >= deadline {
                return None;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// **等**服务端 stderr 里出现某段文本（最多 `timeout`）。同上的理由。
    fn wait_stderr_contains(&self, needle: &str, timeout: Duration) -> bool {
        let deadline = std::time::Instant::now() + timeout;
        loop {
            if self.stderr_text().contains(needle) {
                return true;
            }
            if std::time::Instant::now() >= deadline {
                return false;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        for reader in self.readers.drain(..) {
            let _ = reader.join();
        }
    }
}

/// 起服务端（`tls::LISTEN` + `tls::ACCEPT`），返回它的临时端口。
fn spawn_server(scratch: &Scratch) -> Server {
    let ext = ext_artifact();
    let script_path = scratch.join("server.wll");
    let script = server_script(&fixture("server.crt"), &fixture("server.key"));
    std::fs::write(&script_path, &script).expect("cannot write the TLS server script");

    let mut child = Command::new(wlwl_bin())
        .arg("run")
        .arg("--ext")
        .arg(&ext)
        .arg(&script_path)
        // ⚠ 环境变量那条通道**必须掐掉** ⇒ 这条用例只由 `--ext` 决定扩展从哪来。
        .env_remove("WLWL_EXT_PATH")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("cannot spawn the wlwl host for the TLS server");

    let stdout = child
        .stdout
        .take()
        .expect("the server stdout must be piped");
    let stderr = child
        .stderr
        .take()
        .expect("the server stderr must be piped");
    let stdout_lines = Arc::new(Mutex::new(Vec::new()));
    let stderr_lines = Arc::new(Mutex::new(Vec::new()));

    // ⚠ 端口要**边跑边取** ⇒ 用一个线程读 stdout，主线程等它报端口。
    let (tx, rx): (mpsc::Sender<String>, Receiver<String>) = mpsc::channel();
    let sink = Arc::clone(&stdout_lines);
    let stdout_reader = std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if line.trim().starts_with(PORT_MARKER) {
                let _ = tx.send(line.clone());
            }
            sink.lock()
                .expect("the server stdout lock must not be poisoned")
                .push(line);
        }
    });
    // ⚠ stderr 也得有人读 ⇒ 否则管道满了会把服务端**卡住**。
    let sink = Arc::clone(&stderr_lines);
    let stderr_reader = std::thread::spawn(move || {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            sink.lock()
                .expect("the server stderr lock must not be poisoned")
                .push(line);
        }
    });

    let port_line = rx
        .recv_timeout(Duration::from_secs(120))
        .unwrap_or_else(|e| {
            panic!(
                "the TLS server did not report its port within 120s: {e}\n\
             --- server stdout ---\n{}\n--- server stderr ---\n{}\n--- program ---\n{}",
                stdout_lines
                    .lock()
                    .expect("the server stdout lock must not be poisoned")
                    .join("\n"),
                stderr_lines
                    .lock()
                    .expect("the server stderr lock must not be poisoned")
                    .join("\n"),
                script,
            )
        });
    let port: u16 = port_line[PORT_MARKER.len()..]
        .trim()
        .parse()
        .unwrap_or_else(|e| {
            panic!("the TLS server reported an unparsable port {port_line:?}: {e}")
        });

    Server {
        child,
        port,
        stdout_lines,
        stderr_lines,
        readers: vec![stdout_reader, stderr_reader],
    }
}

/// 跑一次客户端程序。
///
/// ⚠ `ext` = `None` ⇒ **不加载任何扩展**（⇒ `https://` 必然撞「no TLS provider
/// loaded」）—— 那就是反向守卫要的那条路。
fn run_client(scratch: &Scratch, ext: Option<&Path>, url: &str, ca: &Path) -> Run {
    let script_path = scratch.join("client.wll");
    let script = client_script(url, ca);
    std::fs::write(&script_path, &script).expect("cannot write the HTTPS client script");

    let mut cmd = Command::new(wlwl_bin());
    cmd.arg("run");
    if let Some(ext) = ext {
        cmd.arg("--ext").arg(ext);
    }
    let out = cmd
        .arg(&script_path)
        // ⚠ 同上：环境变量那条通道必须掐掉。
        .env_remove("WLWL_EXT_PATH")
        .output()
        .expect("cannot spawn the wlwl host for the HTTPS client");

    Run {
        code: out.status.code(),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        script,
    }
}

// ── 用例 ───────────────────────────────────────────────────────────────────

/// **主用例**：真扩展 + 真证书 + 真握手 ⇒ 拿到 200 与响应体。
#[test]
fn the_https_request_completes_a_real_tls_handshake_and_gets_a_200() {
    let scratch = Scratch::new("green");
    let server = spawn_server(&scratch);
    let ext = ext_artifact();

    // ⚠ 服务端那侧**也**加载了扩展（⇒ 它是**真的** TLS 服务端，不是替身）。
    assert!(
        server.wait_stderr_contains("loading native extension", MARKER_WAIT),
        "the TLS server must have loaded the extension too\n--- server stderr ---\n{}",
        server.stderr_text()
    );

    let run = run_client(&scratch, Some(&ext), &server.url("/"), &fixture("ca.crt"));

    assert_eq!(
        run.code,
        Some(0),
        "the HTTPS client must succeed\n{}",
        run.report()
    );
    // ⚠ 扩展**真的**被加载了（宿主在加载前会打那条醒目告警 ⇒ 它是加载的证据）。
    assert!(
        run.stderr.contains("loading native extension"),
        "the host must have loaded the extension\n{}",
        run.report()
    );
    // ⚠ 走的是**成功**那条路（不是 `ERR` 值）。
    assert!(
        run.marker(ERROR_MARKER).is_none(),
        "the HTTPS request must not fail\n{}",
        run.report()
    );
    assert_eq!(
        run.marker(STATUS_MARKER),
        Some("200"),
        "the HTTPS response status must be 200\n{}",
        run.report()
    );
    assert_eq!(
        run.marker(BODY_MARKER),
        Some("hello"),
        "the HTTPS response body must be the one the server sent\n{}",
        run.report()
    );

    // ⚠ 服务端那侧也要有读数 ⇒ 证明那**不是**客户端自己编出来的 200。
    let request_bytes = server
        .wait_marker(REQUEST_MARKER, MARKER_WAIT)
        .expect("the TLS server must report the request it read");
    let request_bytes: usize = request_bytes.parse().unwrap_or_else(|e| {
        panic!("the server reported an unparsable request size {request_bytes:?}: {e}")
    });
    assert!(
        request_bytes > 0,
        "the TLS server must have read a real HTTP request"
    );
    let sent_bytes = server
        .wait_marker(SENT_MARKER, MARKER_WAIT)
        .expect("the TLS server must report the response it sent");
    let sent_bytes: usize = sent_bytes.parse().unwrap_or_else(|e| {
        panic!("the server reported an unparsable sent size {sent_bytes:?}: {e}")
    });
    assert!(
        sent_bytes > 0,
        "the TLS server must have sent a real response"
    );
}

/// **反向守卫 ①**：**没有加载扩展** ⇒ `https://` 必须是那句逐字的
/// `no TLS provider loaded`（⇒ 证明上面那条 200 是**经扩展**拿到的，不是白捡的）。
#[test]
fn without_the_extension_the_https_request_reports_no_tls_provider() {
    let scratch = Scratch::new("noext");
    // ⚠ 端口随便填 ⇒ 这条路在 `tls:CONNECT` 之前就断了（压根没到网络）。
    let run = run_client(&scratch, None, "https://127.0.0.1:1/", &fixture("ca.crt"));

    let error = run
        .marker(ERROR_MARKER)
        .unwrap_or_else(|| panic!("the client must report an ERR value\n{}", run.report()));
    assert!(
        error.contains("no TLS provider loaded"),
        "the ERR payload must carry the verbatim reason\n{}",
        run.report()
    );
    assert!(
        run.marker(STATUS_MARKER).is_none(),
        "without the extension there must be no status at all\n{}",
        run.report()
    );
}

/// **反向守卫 ③**：客户端信任**另一个** CA（`other-ca.crt` —— 它什么都不签）
/// ⇒ 握手**必须失败** ⇒ 证明上面那条 200 是**过了证书校验**才拿到的
/// （而不是某条「不校验」的旁路）。
#[test]
fn the_handshake_rejects_a_certificate_that_is_not_signed_by_the_trusted_ca() {
    let scratch = Scratch::new("wrongca");
    let server = spawn_server(&scratch);
    let ext = ext_artifact();

    let run = run_client(
        &scratch,
        Some(&ext),
        &server.url("/"),
        &fixture("other-ca.crt"),
    );

    assert!(
        run.marker(STATUS_MARKER).is_none(),
        "the handshake must not produce a status\n{}",
        run.report()
    );
    let error = run
        .marker(ERROR_MARKER)
        .unwrap_or_else(|| panic!("the client must report an ERR value\n{}", run.report()));
    assert!(
        error.contains("TlsError"),
        "the failure must be a TLS failure\n{}",
        run.report()
    );
    // ⚠ 服务端那侧也**握手失败**了（⇒ 失败发生在**握手**里，不是在 HTTP 层）。
    assert_eq!(
        server.wait_marker(ACCEPT_MARKER, MARKER_WAIT),
        Some(ACCEPT_ERROR.to_string()),
        "the TLS server must report the failed handshake"
    );
}

/// **反向守卫 ②**：`--ext` 指向一个**不存在**的路径 ⇒ 宿主必须在**加载**这一步
/// 就拒绝启动（而不是带着「没有扩展」继续跑 ⇒ 那样上面那条 200 就可能来自
/// 某条不经扩展的旁路）。
#[test]
fn a_missing_extension_file_makes_the_host_refuse_to_start() {
    let scratch = Scratch::new("missing-ext");
    let missing = scratch.join("there-is-no-such-extension.dll");
    let run = run_client(
        &scratch,
        Some(&missing),
        "https://127.0.0.1:1/",
        &fixture("ca.crt"),
    );

    assert_ne!(
        run.code,
        Some(0),
        "the host must refuse to run with a missing extension\n{}",
        run.report()
    );
    assert!(
        run.stderr.contains("error:"),
        "the host must report why it refused\n{}",
        run.report()
    );
    assert!(
        run.stderr.contains("there-is-no-such-extension.dll"),
        "the host must name the extension file it could not load\n{}",
        run.report()
    );
    assert!(
        run.marker(STATUS_MARKER).is_none(),
        "a refused load must not produce a response\n{}",
        run.report()
    );
}
