//! 一个**测试用**的宿主替身。
//!
//! ⚠ 它**照着 ABI v1 的契约**驱动扩展：取 `wlwl_ext_v1` 的地址 ⇒ 读命名空间
//! 表 ⇒ 按成员表里的函数指针调用 ⇒ 用**宿主的分配器**收结果。
//!
//! ⇒ 测试走的是**真 ABI 路径**（不是直接调 Rust 函数）⇒ 契约破了测试就会红。
//!
//! 宿主侧的真实现见 `impl/crates/wlwl-ext/src/registry.rs`（`make_member_call`）。

// ⚠ 每个测试二进制只用得上本模块的一部分 ⇒ 别的地方会报「没用到」。
#![allow(dead_code)]

use std::ffi::{c_char, c_void, CStr, CString};
use std::path::PathBuf;
use std::sync::Mutex;

use serde_json::Value as Json;
use wlwl_tls::abi::{WlwlExtCtx, WlwlExtHost, WlwlExtMember, WlwlExtNs, WlwlExtV1};

// ── 宿主分配器（与宿主侧同款：16 字节头 + 总长）──────────────────────────

const ALLOC_HEADER: usize = 16;

/// 宿主分配器。
///
/// # Safety
/// 由扩展按 ABI 契约调用。
pub unsafe extern "system" fn host_alloc(size: u32) -> *mut c_void {
    if size == 0 {
        return std::ptr::null_mut();
    }
    let total = ALLOC_HEADER + size as usize;
    let layout = match std::alloc::Layout::from_size_align(total, 16) {
        Ok(l) => l,
        Err(_) => return std::ptr::null_mut(),
    };
    let base = std::alloc::alloc(layout);
    if base.is_null() {
        return std::ptr::null_mut();
    }
    // SAFETY: `base` 指向 `total` 字节、对齐 16 的分配 ⇒ 写 usize 合法。
    unsafe { (base as *mut usize).write(total) };
    base.add(ALLOC_HEADER) as *mut c_void
}

/// 宿主释放器。
///
/// # Safety
/// `ptr` 必须来自 [`host_alloc`]，且只释放一次。
pub unsafe extern "system" fn host_free(ptr: *mut c_void) {
    if ptr.is_null() {
        return;
    }
    // SAFETY: 调用方保证这是 `host_alloc` 交出来的指针 ⇒ 前面有合法头部。
    let base = unsafe { (ptr as *mut u8).sub(ALLOC_HEADER) };
    let total = unsafe { (base as *mut usize).read() };
    let layout = match std::alloc::Layout::from_size_align(total, 16) {
        Ok(l) => l,
        Err(_) => return,
    };
    // SAFETY: `base` 是 `host_alloc` 交出来的分配起点，`layout` 与分配时一致。
    unsafe { std::alloc::dealloc(base, layout) };
}

/// 宿主报告通道（收集起来给断言看）。
static REPORTS: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());

/// # Safety
/// 由扩展按 ABI 契约调用。
pub unsafe extern "system" fn host_report(
    _ctx: *mut WlwlExtCtx,
    kind: *const c_char,
    reason: *const c_char,
) {
    let kind = if kind.is_null() {
        "?".to_string()
    } else {
        // SAFETY: 契约保证是 NUL 结尾的 C 串。
        unsafe { CStr::from_ptr(kind) }.to_string_lossy().into_owned()
    };
    let reason = if reason.is_null() {
        String::new()
    } else {
        // SAFETY: 同上。
        unsafe { CStr::from_ptr(reason) }.to_string_lossy().into_owned()
    };
    REPORTS.lock().unwrap_or_else(|e| e.into_inner()).push((kind, reason));
}

/// ABI v1 **不支持**异步 ⇒ 恒返回 -1（与宿主侧同口径）。
///
/// # Safety
/// 由扩展按 ABI 契约调用。
pub unsafe extern "system" fn host_await_ready(
    _ctx: *mut WlwlExtCtx,
    _handle: u32,
    _timeout_ms: u32,
) -> i32 {
    -1
}

/// 扩展报上来的所有告警/信息。
pub fn reports() -> Vec<(String, String)> {
    REPORTS.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// 清掉已收集的告警。
pub fn clear_reports() {
    REPORTS.lock().unwrap_or_else(|e| e.into_inner()).clear();
}

// ── 测试宿主 ──────────────────────────────────────────────────────────────

/// 一个测试宿主（持有回调表）。
pub struct TestHost {
    /// 回调表（扩展只读）。
    pub table: WlwlExtHost,
}

impl TestHost {
    /// 造一个测试宿主。
    pub fn new() -> TestHost {
        TestHost {
            table: WlwlExtHost {
                alloc: Some(host_alloc),
                free: Some(host_free),
                report: Some(host_report),
                await_ready: Some(host_await_ready),
            },
        }
    }
}

impl Default for TestHost {
    fn default() -> Self {
        TestHost::new()
    }
}

/// 找命名空间。
fn find_ns(v1: *mut WlwlExtV1, path: &str) -> Option<&'static WlwlExtNs> {
    if v1.is_null() {
        return None;
    }
    // SAFETY: `v1` 来自 `extension_v1_ptr()` ⇒ 指向扩展自己的静态结构
    // （它**永远**活着）⇒ 借出来的引用也是 `'static`。
    let v1: &'static WlwlExtV1 = unsafe { &*v1 };
    let ns_count = v1.ns_count as usize;
    if ns_count == 0 || v1.namespaces.is_null() {
        return None;
    }
    for i in 0..ns_count {
        // SAFETY: `i < ns_count` 且命名空间表非空。
        let ns: &'static WlwlExtNs = unsafe { &*v1.namespaces.add(i) };
        let p = unsafe { CStr::from_ptr(ns.path) }.to_string_lossy().into_owned();
        if p == path {
            return Some(ns);
        }
    }
    None
}

/// 找成员。
fn find_member(ns: &WlwlExtNs, name: &str) -> Option<&'static WlwlExtMember> {
    if ns.members.is_null() {
        return None;
    }
    for j in 0..ns.member_count as usize {
        // SAFETY: `j < member_count` 且成员表非空。
        let m: &'static WlwlExtMember = unsafe { &*ns.members.add(j) };
        let n = unsafe { CStr::from_ptr(m.name) }.to_string_lossy().into_owned();
        if n == name {
            return Some(m);
        }
    }
    None
}

/// 调一个成员。
///
/// 成功 ⇒ `Ok(value)`；失败 ⇒ `Err(err 载荷)`。
pub fn call(
    host: &TestHost,
    ns_path: &str,
    member: &str,
    args: &[Json],
) -> Result<Json, Json> {
    let v1 = wlwl_tls::extension_v1_ptr();
    let ns = find_ns(v1, ns_path).unwrap_or_else(|| panic!("no namespace '{ns_path}'"));
    let m = find_member(&ns, member).unwrap_or_else(|| panic!("no member '{ns_path}:{member}'"));
    let call = m.call.expect("the member has no call function");

    let args_text = serde_json::to_string(args).expect("the arguments are serializable");
    let args_c = CString::new(args_text).expect("no NUL in the arguments");
    let member_c = CString::new(member).expect("no NUL in the member name");

    let mut ctx = WlwlExtCtx {
        host: &host.table as *const WlwlExtHost,
        member: member_c.as_ptr(),
    };
    let mut out: *mut c_char = std::ptr::null_mut();
    // SAFETY: 三个指针都按 ABI 契约准备。
    let rc = unsafe { call(&mut ctx, args_c.as_ptr(), &mut out) };

    let payload = if out.is_null() {
        panic!("the member '{ns_path}:{member}' returned no payload (code {rc})");
    } else {
        // SAFETY: 扩展用宿主的 `alloc` 分配的、NUL 结尾的 C 串。
        let text = unsafe { CStr::from_ptr(out) }.to_string_lossy().into_owned();
        // SAFETY: 由宿主的 `free` 释放。
        unsafe { host_free(out as *mut c_void) };
        text
    };
    let parsed: Json = serde_json::from_str(&payload)
        .unwrap_or_else(|e| panic!("the member returned malformed JSON: {e} ({payload})"));

    if let Some(err) = parsed.get("err") {
        return Err(err.clone());
    }
    if rc != 0 {
        return Err(serde_json::json!({
            "kind": "extension_error",
            "op": format!("{ns_path}:{member}"),
            "reason": format!("the member returned code {rc} without an error payload"),
        }));
    }
    Ok(parsed.get("value").cloned().unwrap_or(Json::Null))
}

// ── 夹具路径 ──────────────────────────────────────────────────────────────

/// `tests/fixtures/` 目录。
pub fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("fixtures")
}

/// 夹具文件的路径。
pub fn fixture(name: &str) -> String {
    fixtures_dir().join(name).to_string_lossy().into_owned()
}

/// 读 `vectors.json`。
pub fn vectors() -> Json {
    let text = std::fs::read_to_string(fixtures_dir().join("vectors.json"))
        .expect("vectors.json is missing (run: python tests/gen_fixture_cert.py)");
    serde_json::from_str(&text).expect("vectors.json is not valid JSON")
}

/// hex ⇒ 字节数组（JSON 形态）。
pub fn hex_to_json_array(hex: &str) -> Json {
    Json::Array(
        (0..hex.len() / 2)
            .map(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap().into())
            .collect(),
    )
}

/// 字节数组（JSON 形态）⇒ hex。
pub fn json_array_to_hex(value: &Json) -> String {
    value
        .as_array()
        .expect("not an array")
        .iter()
        .map(|n| format!("{:02x}", n.as_u64().expect("not a number")))
        .collect()
}
