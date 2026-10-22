//! [一致性册 §6.2 **V-02**] 资源释放责任:打开的句柄在操作后必须回到基线。
//!
//! 语言核心册 §19.4:「**程序终止时,全部资源必须被释放**:进程退出、文件描述符
//! 关闭、网络连接关闭、子进程被收割。实现**不得**因为某个失败路径而泄漏资源。」
//! §19.5:「**无泄漏**:程序终止时全部资源被释放」。
//!
//! ## 判据
//!
//! **进程内打开句柄的计数快照** —— 操作前后的两个快照必须**相等**。
//! ⚠ 用**确定性计数**而不是「跑 N 遍看有没有变慢」这类间接判据(规范 §18.5 的
//! 纪律:判定**必须**用确定性工作量计数)。
//!
//! **失败路径同样要守**:读一个不存在的文件会走错误分支 ⇒ 那条路径**不得**留下
//! 句柄 —— 这是 §19.4 那句「不得因为某个失败路径而泄漏资源」的直接可测形式。
//!
//! ## 平台
//!
//! * POSIX:`/proc/self/fd`(Linux)/ `/dev/fd`(macOS)—— 纯标准库。
//! * Windows:kernel32 的 `GetProcessHandleCount` —— **原始 FFI,不引依赖**
//!   (本仓的依赖表里没有 windows-sys;加一个只为了数句柄不值得)。
//!
//! ## ⚠️ 串行闸(不是可选的洁癖)
//!
//! 句柄计数是**进程级**的 ⇒ 同一条测试二进制里的用例**默认并行**跑 ⇒ 一条用例
//! 在数句柄时,另一条可能正在 spawn 子进程 ⇒ 差值里混进**别人的**句柄 ⇒ 偶发红。
//! ⇒ 三条用例共用一把互斥锁,**必须**串行。
//!
//! ## ⚠️ 子进程收割的判据分平台
//!
//! * POSIX:僵尸进程**仍然算子进程**(它还没被 `wait` 掉)⇒ 用 `pgrep -P` 数本进程
//!   的子进程数,`PROCESS_RUN` 返回前后必须**相等** ⇒ 没收割 ⇒ 多出一个僵尸 ⇒ 红。
//! * Windows:没有僵尸概念 ⇒ 等价形式是**进程句柄被关掉** ⇒ 句柄计数回到基线。
//!
//! ## 范围(如实说明)
//!
//! 本文件覆盖**文件类资源**、**子进程收割**与**网络连接**。
//! ⚠ **未**覆盖:线程 / 池化资源(那些路径的资源生命周期由 §19.2 的「无悬空」与
//! §19.4 的终止语义承担,句柄差里混着线程噪声,不是泄漏信号)。
//!
//! **网络那一半怎么验**:§21.5 明写「`HTTP_GET` / `HTTP_POST` / `HTTP_REQUEST`
//! **必须**在返回前关闭它打开的 TCP 连接与读写缓冲;失败路径**必须**同样关闭,
//! ⇒ 一次调用**不得**留下任何未关闭的 socket」。
//! ⇒ 判据 = **本地一次性服务器**:它接受一个连接、回一句响应、然后**再读一次**
//! —— 读到 **EOF** 才算客户端关干净了(读超时 = 客户端还留着 = 半开连接 ⇒ 红)。
//! ⇒ **不联网**:只连 `127.0.0.1` 上自己起的监听器。

mod common;

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::thread::JoinHandle;

use wlwl_eval::Evaluator;

/// 串行闸:三条用例共用(理由见文件头的「串行闸」一节)。
///
/// ⚠ `Mutex::new` 是 `const` ⇒ 可以做 `static`;`lock()` 的 `Err`(中毒)被
/// **吞掉** —— 中毒只说明**上一条**用例炸了,不该让本条也炸成"因为上一条炸了"。
static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// 当前进程打开的句柄数。
///
/// ⚠ 失败时返回 `usize::MAX`(哨兵)⇒ 任何断言都会**变红**而不是静默通过 ——
/// 「测不出来」不等于「没问题」。
fn open_handle_count() -> usize {
    #[cfg(windows)]
    {
        #[link(name = "kernel32")]
        extern "system" {
            fn GetProcessHandleCount(process: *mut core::ffi::c_void, count: *mut u32) -> i32;
            fn GetCurrentProcess() -> *mut core::ffi::c_void;
        }
        let mut count: u32 = 0;
        // SAFETY: 两个函数都是 kernel32 的 Win32 API,签名与调用约定正确;
        // `count` 指向一个合法的 u32。
        let ok = unsafe { GetProcessHandleCount(GetCurrentProcess(), &mut count) };
        if ok == 0 {
            return usize::MAX;
        }
        count as usize
    }
    #[cfg(unix)]
    {
        // Linux 有 /proc/self/fd;macOS 是 /dev/fd。⇒ 两个都试,先到先算。
        for dir in ["/proc/self/fd", "/dev/fd"] {
            if let Ok(rd) = std::fs::read_dir(dir) {
                return rd.count();
            }
        }
        usize::MAX
    }
}

/// 本进程当前的**子进程**数(POSIX 才有意义)。
///
/// ⚠ **僵尸也算子进程** —— 它还没被 `wait` 掉 ⇒ 这正是「有没有被收割」的探针。
/// ⚠ 工具缺失 ⇒ 返回 `None` ⇒ 断言会红(「测不出来」不等于「没问题」)。
#[cfg(unix)]
fn child_process_count() -> Option<usize> {
    let out = std::process::Command::new("pgrep")
        .arg("-P")
        .arg(std::process::id().to_string())
        .output()
        .ok()?;
    match out.status.code() {
        // pgrep:0 = 有匹配,1 = 无匹配(不是错误),其它 = 出错了。
        Some(0) => Some(
            String::from_utf8_lossy(&out.stdout)
                .lines()
                .filter(|l| !l.trim().is_empty())
                .count(),
        ),
        Some(1) => Some(0),
        _ => None,
    }
}

fn scratch_dir() -> PathBuf {
    common::scratch("wlwl_resrelease")
}

/// 跑一段源码(必须成功)。
fn run(src: &str) {
    let ast = wlwl_parser::parse(src, "t.wll").expect("src must parse");
    let mut ev = Evaluator::new().with_base_dir(scratch_dir());
    ev.eval(&ast).expect("program must run");
}

/// **一次**操作的句柄快照差(操作后 − 操作前)。
///
/// ⚠ 做法:**先热身一次**再取基线 —— 第一次调用可能懒初始化(打开配置、建临时
/// 目录),那不是泄漏 ⇒ 把它排除在基线之外。
fn handle_delta(op: impl Fn()) -> i64 {
    op(); // 热身:把懒初始化的一次性句柄排除在基线之外
    let before = open_handle_count();
    op();
    let after = open_handle_count();
    if before == usize::MAX || after == usize::MAX {
        return i64::MAX; // 数不出来 ⇒ 哨兵 ⇒ 断言会红
    }
    after as i64 - before as i64
}

/// 文件类成员:一次往返之后,打开的句柄必须回到基线。
#[test]
fn file_members_leave_no_handle_behind() {
    let _guard = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let dir = scratch_dir();
    let path = dir.join("release-contract.txt");
    let p = path.to_string_lossy().replace('\\', "/");
    let delta = handle_delta(|| {
        run(&format!(
            r#"
            IMPORT("wlwl:std.fs", ["WRITE_FILE", "READ_FILE", "WRITE_BYTES", "READ_BYTES"]);
            WRITE_FILE("{p}", "hello");
            PRINT(READ_FILE("{p}"));
            WRITE_BYTES("{p}", [65, 66]);
            PRINT(READ_BYTES("{p}"));
            "#
        ));
    });
    assert_eq!(
        delta, 0,
        "a file round-trip must not leave a handle behind (delta = {delta})"
    );
}

/// **失败路径**同样不许泄漏:读一个不存在的文件会走错误分支。
///
/// ⇒ 这是 §19.4「实现**不得**因为某个失败路径而泄漏资源」的直接可测形式。
#[test]
fn a_failing_file_read_leaves_no_handle_behind() {
    let _guard = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let dir = scratch_dir();
    let missing = dir
        .join("no-such-file.txt")
        .to_string_lossy()
        .replace('\\', "/");
    let delta = handle_delta(|| {
        let ast = wlwl_parser::parse(
            &format!(
                r#"
                IMPORT("wlwl:std.fs", ["READ_FILE"]);
                PRINT(READ_FILE("{missing}"));
                "#
            ),
            "t.wll",
        )
        .expect("src must parse");
        let mut ev = Evaluator::new().with_base_dir(scratch_dir());
        // 这次**必须失败**(文件不存在)⇒ 失败本身是判据的一部分。
        let err = ev.eval(&ast).expect_err("reading a missing file must fail");
        assert!(
            err.diagnostic().message.contains("file not found"),
            "the read-side message must stay as it was, got {}",
            err.diagnostic().message
        );
    });
    assert_eq!(
        delta, 0,
        "a failing file read must not leave a handle behind (delta = {delta})"
    );
}

/// 子进程必须被**收割**:`PROCESS_RUN` 返回时,子进程已经退出**且被 wait 掉**。
///
/// ⇒ §19.4「子进程被收割」的可测形式:
/// * POSIX:不收割 ⇒ 子进程变僵尸 ⇒ 它**仍然是**本进程的子进程 ⇒ `pgrep -P` 看得见。
/// * Windows:没有僵尸 ⇒ 等价形式是**进程句柄被关掉** ⇒ 句柄计数回到基线。
///
/// ⚠ 成员名是 `PROCESS_RUN(argv, opts?)`(不是 `EXEC`)—— `std.process` 的成员表
/// 只有 `PROCESS_RUN` / `PROCESS_ID` 两员(标准库册 §19)。
#[test]
fn a_spawned_child_is_reaped() {
    let _guard = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    // 跨平台:`echo` 在 Windows 上不是可执行文件 ⇒ 用 `cmd /C`。
    let argv = if cfg!(windows) {
        r#"["cmd", "/C", "echo release-contract"]"#
    } else {
        r#"["sh", "-c", "echo release-contract"]"#
    };
    let src = format!(
        r#"
        IMPORT("wlwl:std.process", ["PROCESS_RUN"]);
        LET(r, PROCESS_RUN({argv}));
        [AT_K(r, "code", -1), AT_K(r, "stdout", "")]
        "#
    );
    let ast = wlwl_parser::parse(&src, "t.wll").expect("src must parse");

    // 热身一次:把懒初始化的一次性句柄排除在基线之外(同 `handle_delta`)。
    {
        let mut ev = Evaluator::new().with_base_dir(scratch_dir());
        ev.eval(&ast).expect("program must run");
    }

    #[cfg(unix)]
    let children_before = child_process_count();

    let before = open_handle_count();
    let out = {
        let mut ev = Evaluator::new().with_base_dir(scratch_dir());
        let v = ev.eval(&ast).expect("program must run");
        v.display()
    };
    let after = open_handle_count();

    // ⓿ 子进程真的跑完了:`code` 只在**退出之后**才有值(`status` 来自 wait)⇒
    // `code == 0` ⇒ 它退出且**被收割**了;stdout 里有标记 ⇒ 它确实执行了。
    let got = out.replace('\r', "");
    assert!(
        got.starts_with("[0,"),
        "the child must have exited and been waited on — got {out}"
    );
    assert!(
        got.contains("release-contract"),
        "the child's stdout must come back — got {out}"
    );

    // ① 句柄回到基线(Windows 上这是「进程句柄被关掉」的唯一可测形式)。
    assert_ne!(
        before,
        usize::MAX,
        "the handle counter must work on this platform"
    );
    assert_eq!(
        after as i64 - before as i64,
        0,
        "spawning a child must not leave handles behind"
    );

    // ② 僵尸探针:没收割 ⇒ 子进程数 +1。
    #[cfg(unix)]
    {
        let children_before = children_before.expect(
            "pgrep must be available for the zombie check — \
             a missing probe is not a passing probe",
        );
        let children_after = child_process_count().expect("pgrep must stay available");
        assert_eq!(
            children_after, children_before,
            "a spawned child must be reaped — {children_after} children vs {children_before} before"
        );

        // ②-探针自检(阴性对照):故意 spawn 一个**不收割**的子进程 ⇒ 探针必须
        // 看得见它 ⇒ 证明上面那条断言不是「探针坏了所以永远相等」。
        let base = child_process_count().expect("pgrep");
        let mut orphan = std::process::Command::new("sh")
            .arg("-c")
            .arg("exit 0")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("the self-check must be able to spawn");
        let with_orphan = child_process_count().expect("pgrep");
        assert_eq!(
            with_orphan,
            base + 1,
            "the probe must see an unreaped child — otherwise the reaping check is vacuous"
        );
        orphan
            .wait()
            .expect("the self-check child must be waitable");
        let after_reap = child_process_count().expect("pgrep");
        assert_eq!(
            after_reap, base,
            "waiting on the self-check child must remove it from the probe"
        );
    }
}

/// 一次性的**本地** HTTP 服务端:接受**一个**连接、回一句响应,然后**再读一次**。
///
/// 返回 `(port, handle)`。handle 的结果:
/// * `Ok(true)`  ⇒ 客户端**关掉**了连接(读到 EOF)⇒ 符合标准库册 §21.5。
/// * `Ok(false)` ⇒ 客户端**还留着**连接(读超时)⇒ 半开的连接 ⇒ 违约。
/// * `Err(_)`    ⇒ 服务端自己出错(那不是客户端的判据)。
///
/// ⚠ 响应**故意不带** `Connection: close` —— 要看的正是**客户端**关不关。
fn one_shot_server(response: &'static [u8]) -> (u16, JoinHandle<std::io::Result<bool>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("the local server must bind");
    let port = listener
        .local_addr()
        .expect("the bound socket must have an address")
        .port();
    let handle = std::thread::spawn(move || -> std::io::Result<bool> {
        let (mut sock, _) = listener.accept()?;
        // 读超时:客户端不关的话,第二次读必须**超时**而不是永远挂着。
        sock.set_read_timeout(Some(std::time::Duration::from_secs(5)))?;
        let mut buf = [0u8; 4096];
        // ① 读请求(只关心「客户端确实发了东西」)。
        let n = sock.read(&mut buf)?;
        if n == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "the client sent no request",
            ));
        }
        // ② 回一句响应。
        sock.write_all(response)?;
        sock.flush()?;
        // ③ 关键观察:客户端**必须**已经关掉 ⇒ 这次读拿到 EOF。
        match sock.read(&mut buf) {
            Ok(0) => Ok(true),
            Ok(_) => Ok(false),
            Err(e) => match e.kind() {
                // 客户端「带着未读数据关闭」⇒ 内核回 RST ⇒ 同样算关了。
                std::io::ErrorKind::ConnectionReset => Ok(true),
                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut => Ok(false),
                _ => Err(e),
            },
        }
    });
    (port, handle)
}

/// 成功路径:`HTTP_GET` 返回前**必须**关掉它打开的 TCP 连接(标准库册 §21.5)。
#[test]
fn an_http_call_leaves_no_socket_open() {
    let _guard = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let (port, server) = one_shot_server(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nhi");
    let src = format!(
        r#"
        IMPORT("wlwl:std.net", ["HTTP_GET"]);
        LET(r, HTTP_GET("http://127.0.0.1:{port}/release-contract"));
        AT_K(r, "status", -1)
        "#
    );
    let ast = wlwl_parser::parse(&src, "t.wll").expect("src must parse");
    let mut ev = Evaluator::new().with_base_dir(scratch_dir());
    let out = ev.eval(&ast).expect("program must run").display();
    assert_eq!(
        out.trim(),
        "200",
        "the local server must have answered — got {out}"
    );
    let closed = server
        .join()
        .expect("the server thread must not panic")
        .expect("the server must not error");
    assert!(
        closed,
        "HTTP_GET returned, but the TCP connection was still open — a half-open socket"
    );
}

/// 失败路径:协议越界 ⇒ `ERR` 值 ⇒ 连接**同样**必须关掉(标准库册 §21.5)。
#[test]
fn a_failing_http_call_leaves_no_socket_open() {
    let _guard = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    // 不是 HTTP 响应 ⇒ 客户端必然报协议错。
    let (port, server) = one_shot_server(b"NOT-HTTP\r\n\r\n");
    let src = format!(
        r#"
        IMPORT("wlwl:std.net", ["HTTP_GET"]);
        IS_ERR(HTTP_GET("http://127.0.0.1:{port}/release-contract"))
        "#
    );
    let ast = wlwl_parser::parse(&src, "t.wll").expect("src must parse");
    let mut ev = Evaluator::new().with_base_dir(scratch_dir());
    let out = ev.eval(&ast).expect("program must run").display();
    assert_eq!(
        out.trim(),
        "TRUE",
        "a malformed response must be an ERR value — got {out}"
    );
    let closed = server
        .join()
        .expect("the server thread must not panic")
        .expect("the server must not error");
    assert!(
        closed,
        "the failing HTTP_GET left the TCP connection open — a half-open socket"
    );
}
