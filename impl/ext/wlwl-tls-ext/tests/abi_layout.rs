//! **ABI 布局守卫**。
//!
//! ⚠ 这条用例把本 crate 看到的 `#[repr(C)]` 结构体的**大小与字段偏移**
//! 与 C 头文件 `impl/crates/wlwl-ext/include/wlwl_ext.h` 里那套 `struct`
//! 声明**逐字对照**。
//!
//! ⇒ 哪天宿主改了字段顺序 / 加了字段而忘了改头文件（或反之），这条用例变红。
//!
//! 期望值是**从 C 声明手算**的（不是从 Rust 抄的）—— 所以它真的是一道
//! **独立的**对照，而不是「自己跟自己比」。

use std::mem::{offset_of, size_of};

use wlwl_tls::abi::{
    WlwlExtCtx, WlwlExtHost, WlwlExtMember, WlwlExtNs, WlwlExtV1, ABI_VERSION, SYMBOL_NAME,
};

/// 64 位（LP64）下的期望布局。
#[cfg(target_pointer_width = "64")]
mod expect {
    pub const CTX_SIZE: usize = 16;
    pub const CTX_HOST: usize = 0;
    pub const CTX_MEMBER: usize = 8;

    pub const HOST_SIZE: usize = 32;
    pub const HOST_ALLOC: usize = 0;
    pub const HOST_FREE: usize = 8;
    pub const HOST_REPORT: usize = 16;
    pub const HOST_AWAIT_READY: usize = 24;

    pub const MEMBER_SIZE: usize = 24;
    pub const MEMBER_NAME: usize = 0;
    pub const MEMBER_ARITY_MIN: usize = 8;
    pub const MEMBER_ARITY_MAX: usize = 12;
    pub const MEMBER_CALL: usize = 16;

    pub const NS_SIZE: usize = 24;
    pub const NS_PATH: usize = 0;
    pub const NS_MEMBER_COUNT: usize = 8;
    pub const NS_MEMBERS: usize = 16;

    pub const V1_SIZE: usize = 40;
    pub const V1_ABI_VERSION: usize = 0;
    pub const V1_STRUCT_SIZE: usize = 4;
    pub const V1_HOST_VERSION: usize = 8;
    pub const V1_NS_COUNT: usize = 16;
    pub const V1_NAMESPACES: usize = 24;
    pub const V1_HOST: usize = 32;
}

/// 32 位下的期望布局。
#[cfg(target_pointer_width = "32")]
mod expect {
    pub const CTX_SIZE: usize = 8;
    pub const CTX_HOST: usize = 0;
    pub const CTX_MEMBER: usize = 4;

    pub const HOST_SIZE: usize = 16;
    pub const HOST_ALLOC: usize = 0;
    pub const HOST_FREE: usize = 4;
    pub const HOST_REPORT: usize = 8;
    pub const HOST_AWAIT_READY: usize = 12;

    pub const MEMBER_SIZE: usize = 16;
    pub const MEMBER_NAME: usize = 0;
    pub const MEMBER_ARITY_MIN: usize = 4;
    pub const MEMBER_ARITY_MAX: usize = 8;
    pub const MEMBER_CALL: usize = 12;

    pub const NS_SIZE: usize = 12;
    pub const NS_PATH: usize = 0;
    pub const NS_MEMBER_COUNT: usize = 4;
    pub const NS_MEMBERS: usize = 8;

    pub const V1_SIZE: usize = 24;
    pub const V1_ABI_VERSION: usize = 0;
    pub const V1_STRUCT_SIZE: usize = 4;
    pub const V1_HOST_VERSION: usize = 8;
    pub const V1_NS_COUNT: usize = 12;
    pub const V1_NAMESPACES: usize = 16;
    pub const V1_HOST: usize = 20;
}

#[test]
fn the_structures_match_the_c_header_layout() {
    assert_eq!(size_of::<WlwlExtCtx>(), expect::CTX_SIZE, "WlwlExtCtx size");
    assert_eq!(offset_of!(WlwlExtCtx, host), expect::CTX_HOST, "WlwlExtCtx.host");
    assert_eq!(
        offset_of!(WlwlExtCtx, member),
        expect::CTX_MEMBER,
        "WlwlExtCtx.member"
    );

    assert_eq!(size_of::<WlwlExtHost>(), expect::HOST_SIZE, "WlwlExtHost size");
    assert_eq!(offset_of!(WlwlExtHost, alloc), expect::HOST_ALLOC, "WlwlExtHost.alloc");
    assert_eq!(offset_of!(WlwlExtHost, free), expect::HOST_FREE, "WlwlExtHost.free");
    assert_eq!(
        offset_of!(WlwlExtHost, report),
        expect::HOST_REPORT,
        "WlwlExtHost.report"
    );
    assert_eq!(
        offset_of!(WlwlExtHost, await_ready),
        expect::HOST_AWAIT_READY,
        "WlwlExtHost.await_ready"
    );

    assert_eq!(
        size_of::<WlwlExtMember>(),
        expect::MEMBER_SIZE,
        "WlwlExtMember size"
    );
    assert_eq!(
        offset_of!(WlwlExtMember, name),
        expect::MEMBER_NAME,
        "WlwlExtMember.name"
    );
    assert_eq!(
        offset_of!(WlwlExtMember, arity_min),
        expect::MEMBER_ARITY_MIN,
        "WlwlExtMember.arity_min"
    );
    assert_eq!(
        offset_of!(WlwlExtMember, arity_max),
        expect::MEMBER_ARITY_MAX,
        "WlwlExtMember.arity_max"
    );
    assert_eq!(
        offset_of!(WlwlExtMember, call),
        expect::MEMBER_CALL,
        "WlwlExtMember.call"
    );

    assert_eq!(size_of::<WlwlExtNs>(), expect::NS_SIZE, "WlwlExtNs size");
    assert_eq!(offset_of!(WlwlExtNs, path), expect::NS_PATH, "WlwlExtNs.path");
    assert_eq!(
        offset_of!(WlwlExtNs, member_count),
        expect::NS_MEMBER_COUNT,
        "WlwlExtNs.member_count"
    );
    assert_eq!(
        offset_of!(WlwlExtNs, members),
        expect::NS_MEMBERS,
        "WlwlExtNs.members"
    );

    assert_eq!(size_of::<WlwlExtV1>(), expect::V1_SIZE, "WlwlExtV1 size");
    assert_eq!(
        offset_of!(WlwlExtV1, abi_version),
        expect::V1_ABI_VERSION,
        "WlwlExtV1.abi_version"
    );
    assert_eq!(
        offset_of!(WlwlExtV1, struct_size),
        expect::V1_STRUCT_SIZE,
        "WlwlExtV1.struct_size"
    );
    assert_eq!(
        offset_of!(WlwlExtV1, host_version),
        expect::V1_HOST_VERSION,
        "WlwlExtV1.host_version"
    );
    assert_eq!(
        offset_of!(WlwlExtV1, ns_count),
        expect::V1_NS_COUNT,
        "WlwlExtV1.ns_count"
    );
    assert_eq!(
        offset_of!(WlwlExtV1, namespaces),
        expect::V1_NAMESPACES,
        "WlwlExtV1.namespaces"
    );
    assert_eq!(offset_of!(WlwlExtV1, host), expect::V1_HOST, "WlwlExtV1.host");
}

/// 导出符号的**契约字段**必须对。
#[test]
fn the_exported_entry_point_declares_the_contract() {
    assert_eq!(ABI_VERSION, 1, "the ABI version must be 1");
    assert_eq!(SYMBOL_NAME, "wlwl_ext_v1", "the symbol name is fixed");

    let v1 = wlwl_tls::extension_v1_ptr();
    assert!(!v1.is_null());
    // SAFETY: `v1` 指向扩展自己的静态结构。
    let v1 = unsafe { &*v1 };
    assert_eq!(v1.abi_version, ABI_VERSION, "abi_version");
    assert_eq!(
        v1.struct_size as usize,
        size_of::<WlwlExtV1>(),
        "struct_size must cover every field the host reads"
    );
    assert_eq!(v1.ns_count, 2, "two namespaces: tls and crypto");
    assert!(!v1.namespaces.is_null());
    // SAFETY: `host_version` 是 NUL 结尾的 C 串（或 NULL）。
    let version = if v1.host_version.is_null() {
        String::new()
    } else {
        // SAFETY: 契约保证是 NUL 结尾的 C 串。
        unsafe { std::ffi::CStr::from_ptr(v1.host_version) }
            .to_string_lossy()
            .into_owned()
    };
    assert!(
        version.starts_with("wlwl-tls-ext"),
        "the host version text must identify the extension, got {version:?}"
    );
}

/// 命名空间表里的**每个成员**都必须有可调用的入口。
#[test]
fn every_declared_member_has_a_call_function() {
    let v1 = wlwl_tls::extension_v1_ptr();
    // SAFETY: `v1` 指向扩展自己的静态结构。
    let v1 = unsafe { &*v1 };
    for i in 0..v1.ns_count as usize {
        // SAFETY: `i < ns_count` 且命名空间表非空。
        let ns = unsafe { &*v1.namespaces.add(i) };
        // SAFETY: 命名空间路径是 NUL 结尾的 C 串。
        let path = unsafe { std::ffi::CStr::from_ptr(ns.path) }
            .to_string_lossy()
            .into_owned();
        assert!(
            !path.contains(':'),
            "a namespace path must not contain ':' (the dispatch key is '<ns>:<member>')"
        );
        assert!(
            !path.starts_with("wlwl"),
            "a namespace path must not start with 'wlwl' (that prefix belongs to std)"
        );
        assert!(ns.member_count > 0, "namespace '{path}' has no members");
        for j in 0..ns.member_count as usize {
            // SAFETY: `j < member_count` 且成员表非空。
            let m = unsafe { &*ns.members.add(j) };
            assert!(
                m.call.is_some(),
                "member {j} of '{path}' has no call function"
            );
            assert!(m.arity_min <= m.arity_max, "member {j} of '{path}' has a reversed arity");
            // SAFETY: 成员名是 NUL 结尾的 C 串。
            let name = unsafe { std::ffi::CStr::from_ptr(m.name) }
                .to_string_lossy()
                .into_owned();
            assert!(!name.is_empty(), "member {j} of '{path}' has an empty name");
        }
    }
}
