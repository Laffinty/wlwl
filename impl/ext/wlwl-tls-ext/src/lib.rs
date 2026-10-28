//! [W-22] **TLS / crypto 扩展**（原生动态库，扩展 ABI v1）。
//!
//! 这个 crate 产出**一个独立的动态库**（`wlwl_tls.dll` / `libwlwl_tls.so`），
//! 宿主用 `wlwl --ext <path>` 加载它 ⇒ 它的成员挂成两个可 `IMPORT` 的
//! 命名空间：`tls` 与 `crypto`。
//!
//! # ⚠ 它**不是**语言面的一部分
//!
//! 扩展命名空间**不进附录 A、不进公共面清单** —— 公共面取自
//! `wlwl_std::namespace_paths()` 那张静态表 ⇒ TLS / crypto **不在**规范里，
//! 只有显式 `--ext` 加载了本库的脚本才能用。
//!
//! # 依赖面
//!
//! 本 crate **不在** wlwl 主 workspace 里（自己的 `Cargo.toml` 里有空
//! `[workspace]` 表）⇒ **编译器主仓的依赖面一个字都不变**。
//!
//! 第三方依赖只有四个，且**全部已在主仓的 `Cargo.lock` 里**：
//! `rustls` / `ring` / `webpki-roots` / `serde_json`。
//!
//! # ABI
//!
//! 契约见 `impl/crates/wlwl-ext/include/wlwl_ext.h`（C 形态）与
//! `impl/crates/wlwl-ext/src/abi.rs`（Rust 形态）—— 本 crate **直接引用**
//! 后者（`src/abi.rs` 里的 `#[path]`）⇒ **ABI 的单一真相在宿主那边**。
//!
//! # 失败怎么报
//!
//! 一律是 **`ERR` 值**（`{"kind": "TlsError" | "CryptoError", "op": …,
//! "reason": …}`）—— **不新增任何语言诊断码**。
//!
//! # 已知限制
//!
//! - **阻塞语义**：ABI v1 没有就绪事件通道 ⇒ 握手 / accept / 读 / 写**阻塞
//!   调用线程**（整个运行时随之冻结）。异步形态需要 **ABI v1.1**（另立）。
//! - **平台差异**：Windows 产出 `.dll`（`crate-type = ["cdylib"]`），
//!   Linux/macOS 产出 `.so` / `.dylib`；文件名由 crate 名决定。

pub mod abi;
mod args;
mod bytes;
pub mod crypto;
mod envelope;
pub mod tls;
pub mod x509;

pub use abi::*;
pub use envelope::Ctx;

use envelope::{member_entry, KIND_CRYPTO, KIND_TLS};

// ── 成员入口 ──────────────────────────────────────────────────────────────

member_entry!(accept_entry, KIND_TLS, "ACCEPT", tls::accept);
member_entry!(cert_fingerprint_entry, KIND_TLS, "CERT_FINGERPRINT", tls::cert_fingerprint);
member_entry!(cert_info_entry, KIND_TLS, "CERT_INFO", tls::cert_info);
member_entry!(close_entry, KIND_TLS, "CLOSE", tls::close);
member_entry!(connect_entry, KIND_TLS, "CONNECT", tls::connect);
member_entry!(listen_entry, KIND_TLS, "LISTEN", tls::listen);
member_entry!(local_addr_entry, KIND_TLS, "LOCAL_ADDR", tls::local_addr);
member_entry!(peer_entry, KIND_TLS, "PEER", tls::peer);
member_entry!(recv_entry, KIND_TLS, "RECV", tls::recv);
member_entry!(send_entry, KIND_TLS, "SEND", tls::send);

member_entry!(aes_256_gcm_entry, KIND_CRYPTO, "AES_256_GCM", crypto::aes_256_gcm);
member_entry!(ecdsa_p256_verify_entry, KIND_CRYPTO, "ECDSA_P256_VERIFY", crypto::ecdsa_p256_verify);
member_entry!(ed25519_verify_entry, KIND_CRYPTO, "ED25519_VERIFY", crypto::ed25519_verify);
member_entry!(hkdf_entry, KIND_CRYPTO, "HKDF", crypto::hkdf);
member_entry!(hmac_sha512_entry, KIND_CRYPTO, "HMAC_SHA512", crypto::hmac_sha512);
member_entry!(rsa_pss_verify_entry, KIND_CRYPTO, "RSA_PSS_VERIFY", crypto::rsa_pss_verify);
member_entry!(sha384_entry, KIND_CRYPTO, "SHA384", crypto::sha384);
member_entry!(sha512_entry, KIND_CRYPTO, "SHA512", crypto::sha512);

// ── 命名空间表 ────────────────────────────────────────────────────────────

/// 一张**只读**的表（里面全是裸指针）。
///
/// ⚠ 裸指针**不是** `Sync` ⇒ 直接 `static` 会被编译器拦下。这里显式声明
/// 「这张表只读、可以跨线程共享」—— 它在**装载之后**再也不会被改
/// （唯一的例外是 `wlwl_ext_v1.host`，那是**宿主**在装载时写的，而它本身
/// 就是 `static mut`）。
struct ReadOnlyTable<T>(T);

// SAFETY: `ReadOnlyTable` 里装的表在初始化之后**只读**（宿主只读它）⇒
// 跨线程共享是安全的。
unsafe impl<T> Sync for ReadOnlyTable<T> {}

/// `tls` 命名空间的成员表。
static TLS_MEMBERS: ReadOnlyTable<[WlwlExtMember; 10]> = ReadOnlyTable([
    WlwlExtMember {
        name: c"LISTEN".as_ptr(),
        // `opts` 可省 ⇒ 2..=3。
        arity_min: 2,
        arity_max: 3,
        call: Some(listen_entry),
    },
    WlwlExtMember {
        name: c"ACCEPT".as_ptr(),
        arity_min: 2,
        arity_max: 2,
        call: Some(accept_entry),
    },
    WlwlExtMember {
        name: c"CONNECT".as_ptr(),
        arity_min: 2,
        arity_max: 3,
        call: Some(connect_entry),
    },
    WlwlExtMember {
        name: c"SEND".as_ptr(),
        arity_min: 2,
        arity_max: 2,
        call: Some(send_entry),
    },
    WlwlExtMember {
        name: c"RECV".as_ptr(),
        arity_min: 3,
        arity_max: 3,
        call: Some(recv_entry),
    },
    WlwlExtMember {
        name: c"CLOSE".as_ptr(),
        arity_min: 1,
        arity_max: 1,
        call: Some(close_entry),
    },
    WlwlExtMember {
        name: c"PEER".as_ptr(),
        arity_min: 1,
        arity_max: 1,
        call: Some(peer_entry),
    },
    WlwlExtMember {
        name: c"LOCAL_ADDR".as_ptr(),
        arity_min: 1,
        arity_max: 1,
        call: Some(local_addr_entry),
    },
    WlwlExtMember {
        name: c"CERT_FINGERPRINT".as_ptr(),
        arity_min: 1,
        arity_max: 1,
        call: Some(cert_fingerprint_entry),
    },
    WlwlExtMember {
        name: c"CERT_INFO".as_ptr(),
        arity_min: 1,
        arity_max: 1,
        call: Some(cert_info_entry),
    },
]);

/// `crypto` 命名空间的成员表。
static CRYPTO_MEMBERS: ReadOnlyTable<[WlwlExtMember; 8]> = ReadOnlyTable([
    WlwlExtMember {
        name: c"SHA384".as_ptr(),
        arity_min: 1,
        arity_max: 1,
        call: Some(sha384_entry),
    },
    WlwlExtMember {
        name: c"SHA512".as_ptr(),
        arity_min: 1,
        arity_max: 1,
        call: Some(sha512_entry),
    },
    WlwlExtMember {
        name: c"HMAC_SHA512".as_ptr(),
        arity_min: 2,
        arity_max: 2,
        call: Some(hmac_sha512_entry),
    },
    WlwlExtMember {
        name: c"HKDF".as_ptr(),
        arity_min: 5,
        arity_max: 5,
        call: Some(hkdf_entry),
    },
    WlwlExtMember {
        name: c"AES_256_GCM".as_ptr(),
        arity_min: 4,
        arity_max: 4,
        call: Some(aes_256_gcm_entry),
    },
    WlwlExtMember {
        name: c"ED25519_VERIFY".as_ptr(),
        arity_min: 3,
        arity_max: 3,
        call: Some(ed25519_verify_entry),
    },
    WlwlExtMember {
        name: c"ECDSA_P256_VERIFY".as_ptr(),
        arity_min: 3,
        arity_max: 3,
        call: Some(ecdsa_p256_verify_entry),
    },
    WlwlExtMember {
        name: c"RSA_PSS_VERIFY".as_ptr(),
        arity_min: 3,
        arity_max: 3,
        call: Some(rsa_pss_verify_entry),
    },
]);

/// 命名空间表。
static NAMESPACES: ReadOnlyTable<[WlwlExtNs; 2]> = ReadOnlyTable([
    WlwlExtNs {
        path: c"tls".as_ptr(),
        member_count: TLS_MEMBERS.0.len() as u32,
        members: TLS_MEMBERS.0.as_ptr(),
    },
    WlwlExtNs {
        path: c"crypto".as_ptr(),
        member_count: CRYPTO_MEMBERS.0.len() as u32,
        members: CRYPTO_MEMBERS.0.as_ptr(),
    },
]);

// ── 导出符号 ──────────────────────────────────────────────────────────────

/// 扩展**唯一**允许导出的符号（指向 [`WlwlExtV1`]）。
///
/// ⚠ 这是一个**数据符号**（不是函数）—— 宿主拿到的就是这个符号的**地址**，
/// 再把它当成 `wlwl_ext_v1_t*` 读（见宿主的 `load_extension`）。
/// ⚠ 它的 `host` 字段由**宿主**填写（扩展只读）。
#[no_mangle]
#[allow(non_upper_case_globals)]
pub static mut wlwl_ext_v1: WlwlExtV1 = WlwlExtV1 {
    abi_version: ABI_VERSION,
    struct_size: std::mem::size_of::<WlwlExtV1>() as u32,
    host_version: c"wlwl-tls-ext 0.1.0".as_ptr(),
    ns_count: NAMESPACES.0.len() as u32,
    namespaces: NAMESPACES.0.as_ptr(),
    host: std::ptr::null_mut(),
};

/// 扩展入口结构体的地址（**只取地址**，不读内容）。
///
/// ⚠ 这是给**测试**用的：宿主侧拿的是 `GetProcAddress` / `dlsym` 返回的
/// 地址 ⇒ 与 [`wlwl_ext_v1`] 是同一个东西。
pub fn extension_v1_ptr() -> *mut WlwlExtV1 {
    // ⚠ `&raw const` **不产生**对 `static mut` 的引用 ⇒ 不触发
    // `static_mut_refs`（也不需要 `unsafe`）。
    &raw const wlwl_ext_v1 as *mut WlwlExtV1
}
