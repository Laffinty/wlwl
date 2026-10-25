//! [G-11] **socket 族的句柄泄漏守卫** —— `common::handle_guard` 的**模板用法**。
//!
//! ## 为什么这个用例**独占一个测试二进制**
//!
//! 句柄计数是**进程级**的 ⇒ 同一条测试二进制里的**任何**用例开着句柄，
//! 都会把本用例的基线**污染**（哪怕只是「在基线那一刻还开着、关完就降了」
//! ⇒ 读数**下降** ⇒ 同样判红）。
//!
//! ⇒ 所以：**会数句柄的守卫，要么独占一个测试二进制，要么同文件里所有
//! 会开句柄的用例都进同一把闸**（上游 `resource_release_contract.rs` 走的是
//! 后一条：它文件里的每条用例都取那把 `SERIAL`）。
//!
//! ⚠ **本项目踩过**：这条用例最初放在 `tcp_contract.rs` 里 ⇒ 那条二进制里
//!   还有 4 条用例会开监听器/连接 ⇒ 并行跑时基线被污染 ⇒ **偶发红**
//!   （单独跑绿、全量跑红）⇒ 挪到独立文件才稳。
//!
//! ## 覆盖面：**监听 socket** 与**监听端口**
//!
//! 规范一致性册 §6.2 **V-02** 的覆盖边界原文写着「**未**覆盖：**监听端口**」，
//! 理由是「公开面上**没有**监听类成员」（那时 `std.net` 只有客户端那几个）。
//! ⇒ 现在 `std.net` 有 `TCP_LISTEN` / `TCP_CLOSE_LISTENER`（附录 A 已列）
//! ⇒ **监听端口这一半变成可覆盖的** ⇒ 本文件把它补上。
//!
//! ⚠ **两半都要验**（只看句柄数不够）：
//! ① **句柄**：监听 socket 的 OS 句柄在 `TCP_CLOSE_LISTENER` 之后必须消失
//!    ⇒ `common::handle_guard` 的判据（**回到基线**）。
//! ② **端口**：同一个端口必须**能被重新绑定** ⇒ 证明操作系统那边**真的**
//!    没有留着那个 LISTEN socket —— 「端口还占着」这种残留**只看句柄数
//!    看不出来**。
//!
//! ## 调用点就这 5 行（这就是 G-11 的验收判据：「十行内」）
//!
//! 句柄计数、串行闸、**异步释放**的等待 ⇒ 全在 `common::handle_guard` 里。

mod common;

use std::net::TcpListener;

use wlwl_eval::Evaluator;

fn scratch_dir() -> std::path::PathBuf {
    common::scratch("wlwl_socket_guard")
}

fn actual(src: &str) -> String {
    let ast =
        wlwl_parser::parse(src, "t.wll").unwrap_or_else(|e| panic!("case must parse: {src}\n{e}"));
    let mut ev = Evaluator::new().with_base_dir(scratch_dir());
    match ev.eval(&ast) {
        Ok(v) => v.display(),
        Err(e) => {
            let code = e.diagnostic().code.as_str();
            if e.diagnostic().message.is_empty() {
                format!("!{code}")
            } else {
                format!("!{code} {}", e.diagnostic().message)
            }
        }
    }
}

/// 起一个监听器（`TCP_LISTEN`）⇒ 返回它的句柄。
fn listen(host: &str, port: u16) -> i64 {
    actual(&format!(
        r#"IMPORT("wlwl:std.net", ["TCP_LISTEN"]); TCP_LISTEN("{host}", {port})"#
    ))
    .parse::<i64>()
    .expect("a listener handle")
}

/// 问监听器它占的端口（`TCP_LOCAL_ADDR`）。
///
/// ⚠ 拿不到 ⇒ 返 **0**（**不** panic）⇒ 由调用点据此判「监听器压根没起来」
///   ⇒ 免得「监听器没起来」被当成「端口释放成功」。
fn local_port(id: i64) -> u16 {
    actual(&format!(
        r#"IMPORT("wlwl:std.net", ["TCP_LOCAL_ADDR"]); AT_K(TCP_LOCAL_ADDR({id}), "port", 0)"#
    ))
    .parse::<u16>()
    .unwrap_or(0)
}

/// 关掉监听器（`TCP_CLOSE_LISTENER`）。
fn close_listener(id: i64) {
    let got = actual(&format!(
        r#"IMPORT("wlwl:std.net", ["TCP_CLOSE_LISTENER"]); TCP_CLOSE_LISTENER({id})"#
    ));
    assert_eq!(
        got, "NULL",
        "closing a listener must return NULL, got {got:?}"
    );
}

/// 端口能不能被**重新绑定**（⇒ 操作系统那边已经没有那个 LISTEN socket 了）。
///
/// ⚠ **轮询**：监听 socket 的释放是**异步**的（`TCP_CLOSE_LISTENER` 只置标志，
///   真正 drop 掉 socket 的是接收线程）⇒ 刚关完的那一瞬端口可能还被占着
///   ⇒ 有界轮询到能绑为止。
///
/// ⚠ **必须在串行闸之内调用**：重绑会**临时**开一个监听 socket ⇒ 若在闸外
///   做，它的句柄会混进**别的**用例的句柄读数 ⇒ 偶发红。
fn rebindable(port: u16) -> bool {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    loop {
        match TcpListener::bind(("127.0.0.1", port)) {
            Ok(l) => {
                drop(l);
                return true;
            }
            Err(_) => {
                if std::time::Instant::now() >= deadline {
                    return false;
                }
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
        }
    }
}

#[test]
fn the_socket_family_leaves_no_handle_behind() {
    // ⚠ socket 族的释放是**异步**的（`TCP_CLOSE_LISTENER` 只是唤醒 acceptor
    //   线程，socket 句柄由那个线程 drop）⇒ 夹具会**等**它落定 ⇒ 判据稳。
    common::handle_guard::assert_no_handle_leak(
        "socket family",
        || listen("127.0.0.1", 0),
        close_listener,
    );
}

/// **监听 socket** 与它占的那个**端口**，都必须被释放。
///
/// ⚠ 判据分两半：
/// ① **句柄**：`common::handle_guard` 的声明式判据（建 → 关 → 回到基线）。
/// ② **端口**：关掉之后**同一个端口必须能被重新绑定** ⇒ 操作系统那边
///    **没有**残留的 LISTEN socket。
///
/// ⚠ **清理纪律**：监听 socket 是**真的 OS 资源** ⇒ 无论断言成不成功它都
///   必须被关掉 ⇒ 关的动作放在守卫的 `release` 闭包里（它**一定**跑），
///   端口的断言放在守卫**之后** ⇒ 失败也不会留下占用的端口。
#[test]
fn a_listening_socket_and_its_port_are_released() {
    // 端口从监听器**自己**问出来（`TCP_LOCAL_ADDR`）⇒ 顺带证明「监听器真的
    // 占了一个端口」⇒ 免得这条用例在「监听器压根没起来」的情况下也变绿。
    let bound_port = std::cell::Cell::new(0u16);
    // 端口那一半的判据（**在闸内**做，见 `rebindable` 的说明）。
    let port_released = std::cell::Cell::new(false);

    common::handle_guard::assert_no_handle_leak(
        "listening socket",
        || {
            let id = listen("127.0.0.1", 0);
            bound_port.set(local_port(id));
            id
        },
        |id| {
            close_listener(id);
            // 端口这一半：**关掉之后**同一个端口必须**能再绑一次**。
            // ⚠ 这一步**故意放在 `release` 闭包里** ⇒ 它在串行闸**之内**
            //   ⇒ 它临时开的那一个监听 socket 不会污染别的用例的句柄读数；
            //   而且闸内**没有别的**监听器 ⇒ 端口不会被别人占着。
            port_released.set(rebindable(bound_port.get()));
        },
    );

    // 守卫绿 ⇒ 句柄数已回到基线 ⇒ 监听 socket 的 OS 句柄已被释放。
    let port = bound_port.get();
    assert!(
        port > 0,
        "the listener must have reported a real port — otherwise this case would \
         pass without ever having bound one"
    );
    assert!(
        port_released.get(),
        "the listening port {port} must be released — the OS still holds the LISTEN socket"
    );
}
