//! 句柄表（`LISTEN` / `CONNECT` 交出来的 `INTEGER` 句柄）。
//!
//! # 为什么是全局表
//!
//! ABI v1 的值编码是 **JSON 文本** ⇒ **句柄过不了边界**（JSON 没有句柄
//! 形态）⇒ 只能由扩展自己把 `INTEGER` ↔ 真实对象映射起来。
//!
//! # 线程安全
//!
//! 表本身用 `Mutex`（宿主是单线程解释器，但扩展可能被别的线程调进来 ——
//! 用 `Mutex` 比用 `RefCell` 便宜不了多少，却不会在跨线程时炸）。
//! ⚠ **连接对象**（`Conn`）里的流用**自己一把锁** ⇒ 一次阻塞的读/写
//! **不会**把整张表卡住（表锁只在取/放句柄时持有）。

use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpListener};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::Duration;

use rustls::ServerConfig;

use crate::x509::CertInfo;

/// 一个「能读能写」的字节流（TLS 记录层之下）。
///
/// ⚠ 额外带两个**控制**方法：读超时与关闭 —— 它们作用在**底下的 socket**
/// 上（TLS 记录层把它藏起来了 ⇒ 只能通过 socket 的克隆句柄去设）。
pub trait Stream: Read + Write + Send {
    /// 设读超时（`None` = 不限时）。
    fn set_read_timeout(&self, timeout: Option<Duration>) -> io::Result<()>;
    /// 关掉这条连接（尽力而为）。
    fn shutdown(&self);
}

/// 对端信息（`PEER` 交出去的那份）。
#[derive(Debug, Clone)]
pub struct PeerInfo {
    /// 对端 IP（文本）。
    pub host: String,
    /// 对端端口。
    pub port: u16,
    /// 对端证书链（从叶证书开始）。
    pub certificates: Vec<CertInfo>,
}

/// 一个监听器。
pub struct Listener {
    /// 真正的 socket。
    pub listener: TcpListener,
    /// 这个监听用的服务器配置（证书 / SNI / ALPN / 客户端证书策略）。
    pub config: Arc<ServerConfig>,
    /// 本地绑定地址。
    pub local: SocketAddr,
}

/// 一条已建立的连接。
pub struct Conn {
    /// 字节流（TLS 记录层）。
    pub stream: Mutex<Box<dyn Stream>>,
    /// 对端信息。
    pub peer: PeerInfo,
}

/// 表里的一项。
pub enum Entry {
    /// 监听器。
    Listener(Arc<Listener>),
    /// 连接。
    Conn(Arc<Conn>),
}

/// 句柄表。
struct Registry {
    next: u32,
    entries: HashMap<u32, Entry>,
}

/// 全局句柄表。
///
/// ⚠ 用 `LazyLock` 而不是直接 `static Mutex<…>`：`HashMap::new()` **不是**
/// const ⇒ 直接放进 `static` 编不过。
static REGISTRY: LazyLock<Mutex<Registry>> = LazyLock::new(|| {
    Mutex::new(Registry {
        next: 1,
        entries: HashMap::new(),
    })
});

/// 放一个对象进表 ⇒ 句柄。
pub fn insert(entry: Entry) -> u32 {
    let mut guard = REGISTRY.lock().unwrap_or_else(|e| e.into_inner());
    let id = guard.next;
    guard.next = guard.next.wrapping_add(1).max(1);
    guard.entries.insert(id, entry);
    id
}

/// 取一个对象（**克隆 Arc**，不长时间持锁）。
pub fn get(id: u32) -> Option<Entry> {
    let guard = REGISTRY.lock().unwrap_or_else(|e| e.into_inner());
    match guard.entries.get(&id)? {
        Entry::Listener(l) => Some(Entry::Listener(l.clone())),
        Entry::Conn(c) => Some(Entry::Conn(c.clone())),
    }
}

/// 从表里**取走**一个对象（`CLOSE` 用）。
pub fn remove(id: u32) -> Option<Entry> {
    let mut guard = REGISTRY.lock().unwrap_or_else(|e| e.into_inner());
    guard.entries.remove(&id)
}

/// 表里现在有几项（自测用）。
pub fn len() -> usize {
    let guard = REGISTRY.lock().unwrap_or_else(|e| e.into_inner());
    guard.entries.len()
}
