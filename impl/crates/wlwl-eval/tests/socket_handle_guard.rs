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
//! ## 全部代码就这 6 行（这就是 G-11 的验收判据：「十行内」）
//!
//! 句柄计数、串行闸、**异步释放**的等待 ⇒ 全在 `common::handle_guard` 里。

mod common;

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

#[test]
fn the_socket_family_leaves_no_handle_behind() {
    // ⚠ socket 族的释放是**异步**的（`TCP_CLOSE_LISTENER` 只是唤醒 acceptor
    //   线程，socket 句柄由那个线程 drop）⇒ 夹具会**等**它落定 ⇒ 判据稳。
    common::handle_guard::assert_no_handle_leak(
        "socket family",
        || {
            actual(r#"IMPORT("wlwl:std.net", ["TCP_LISTEN"]); TCP_LISTEN("127.0.0.1", 0)"#)
                .parse::<i64>()
                .expect("a listener handle")
        },
        |l| {
            actual(&format!(
                r#"IMPORT("wlwl:std.net", ["TCP_CLOSE_LISTENER"]); TCP_CLOSE_LISTENER({l})"#
            ));
        },
    );
}
