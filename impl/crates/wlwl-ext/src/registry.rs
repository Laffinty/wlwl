//! [W-21] 把 ABI 数据装配成**运行期注册表**（`wlwl_value::ExtRegistry`）。
//!
//! 这一层是「动态符号」与「语言面」之间唯一的桥：
//! - 读 ABI 数据 ⇒ 校验 ⇒ 造 [`ExtMember`]；
//! - 每个成员带一个**闭包**：闭包持有动态库句柄（`Rc<Library>`）与宿主
//!   回调表（`Rc<WlwlExtHost>`）⇒ 库只要还有成员能被调用，就**不会**被卸载。
//!
//! ⚠ **跨 CRT 的关键**：扩展交回来的字符串由**宿主**的 `free` 释放
//! （扩展用宿主的 `alloc` 分配）⇒ 两边共用同一个分配器 ⇒ Windows 上
//! 不会出现「一边是 ucrt、另一边是 msvcrt」的跨堆释放。

use std::collections::HashMap;
use std::ffi::{c_char, c_void, CStr, CString};
use std::rc::Rc;

use wlwl_error::{ErrorCode, Location, WlwlDiagnostic, WlwlError};
use wlwl_value::{DictInner, ExtMember, ExtMemberCall, ExtNamespace, ExtRegistry, Outcome, Value};

use crate::abi::{self, WlwlExtHost, WlwlExtV1};
use crate::dynlib::Library;
use crate::json;
use crate::trust::ExtLoadError;

/// 命名空间个数的上限（防「扩展说它有 40 亿个命名空间」）。
const MAX_NAMESPACES: usize = 4096;
/// 单个命名空间的成员数上限。
const MAX_MEMBERS: usize = 65536;

// ── 宿主分配器 ─────────────────────────────────────────────────────────

/// 分配器的**头部**（字节）：释放时要拿回同一个 `Layout` ⇒ 必须记下总长。
const ALLOC_HEADER: usize = 16;

/// 宿主分配器：扩展**必须**用它分配交回来的字符串。
///
/// ⚠ 头部里存的是「总长」而不是「载荷长」—— `free` 只拿到指针 ⇒ 长度
/// 必须由分配侧记下来。
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
    // 载荷起点仍在 16 字节边界上（头部本身就是 16 字节）。
    base.add(ALLOC_HEADER) as *mut c_void
}

/// 宿主释放器：只接受 [`host_alloc`] 交出来的指针。
///
/// # Safety
/// `ptr` 必须来自 [`host_alloc`]，且**只释放一次**。
pub unsafe extern "system" fn host_free(ptr: *mut c_void) {
    if ptr.is_null() {
        return;
    }
    // SAFETY: 调用方保证 `ptr` 来自 `host_alloc` ⇒ 前面有合法头部。
    let base = unsafe { (ptr as *mut u8).sub(ALLOC_HEADER) };
    // SAFETY: 同上。
    let total = unsafe { (base as *mut usize).read() };
    let layout = match std::alloc::Layout::from_size_align(total, 16) {
        Ok(l) => l,
        Err(_) => return,
    };
    // SAFETY: `base` 是 `host_alloc` 交出来的分配起点，`layout` 与分配时一致。
    unsafe { std::alloc::dealloc(base, layout) };
}

/// 扩展的报告通道：宿主把它打到 stderr（带前缀，**不**混进程序输出）。
///
/// # Safety
/// `kind` / `reason` 必须是 NUL 结尾的 C 串（或 NULL）。
pub unsafe extern "system" fn host_report(
    _ctx: *mut abi::WlwlExtCtx,
    kind: *const c_char,
    reason: *const c_char,
) {
    let kind = cstr_to_string(kind).unwrap_or_else(|| "?".to_string());
    let reason = cstr_to_string(reason).unwrap_or_default();
    eprintln!("wlwl: extension report [{}]: {}", kind, reason);
}

/// `await_ready`：**v1 不支持** ⇒ 恒返回 `-1`。
///
/// ⚠ ABI v1 的 `wlwl_ext_ctx_t` 里没有「就绪事件」通道 ⇒ 宿主没法诚实地
/// 回答这个问题 ⇒ 说「不支持」比假装支持诚实。
///
/// # Safety
/// 参数不被使用。
pub unsafe extern "system" fn host_await_ready(
    _ctx: *mut abi::WlwlExtCtx,
    _handle: u32,
    _timeout_ms: u32,
) -> i32 {
    -1
}

// ── 装配 ───────────────────────────────────────────────────────────────

/// 读 ABI 数据、校验、装配成运行期注册表。
pub(crate) fn build(
    v1: &mut WlwlExtV1,
    library: &Rc<Library>,
    host_table: &Rc<WlwlExtHost>,
    reserved: &[String],
) -> Result<ExtRegistry, ExtLoadError> {
    let mut registry = ExtRegistry::new();
    let ns_count = v1.ns_count as usize;
    if ns_count > MAX_NAMESPACES {
        return Err(ExtLoadError::Contract(format!(
            "extension declares {} namespaces (limit {})",
            ns_count, MAX_NAMESPACES
        )));
    }
    if ns_count > 0 && v1.namespaces.is_null() {
        return Err(ExtLoadError::Contract(
            "extension declares namespaces but the namespace table is NULL".into(),
        ));
    }
    for i in 0..ns_count {
        // SAFETY: `i < ns_count` 且命名空间表非空 ⇒ 读的是表内第 i 项。
        let ns = unsafe { &*v1.namespaces.add(i) };
        let path = cstr_to_string(ns.path).ok_or_else(|| {
            ExtLoadError::Contract("namespace path is not a valid UTF-8 C string".into())
        })?;
        crate::trust::check_namespace_allowed(&path, reserved)?;
        if registry.namespace(&path).is_some() {
            return Err(ExtLoadError::NamespaceDuplicate { path: path.clone() });
        }
        let member_count = ns.member_count as usize;
        if member_count > MAX_MEMBERS {
            return Err(ExtLoadError::Contract(format!(
                "namespace '{}' declares {} members (limit {})",
                path, member_count, MAX_MEMBERS
            )));
        }
        if member_count > 0 && ns.members.is_null() {
            return Err(ExtLoadError::Contract(format!(
                "namespace '{}' declares members but the member table is NULL",
                path
            )));
        }
        let mut members: HashMap<String, ExtMember> = HashMap::with_capacity(member_count);
        for j in 0..member_count {
            // SAFETY: `j < member_count` 且成员表非空 ⇒ 读的是表内第 j 项。
            let m = unsafe { &*ns.members.add(j) };
            let name = cstr_to_string(m.name).ok_or_else(|| {
                ExtLoadError::Contract(format!(
                    "member name in namespace '{}' is not a valid UTF-8 C string",
                    path
                ))
            })?;
            if name.is_empty() {
                return Err(ExtLoadError::MemberNameInvalid {
                    namespace: path.clone(),
                    name: name.clone(),
                });
            }
            let Some(call) = m.call else {
                return Err(ExtLoadError::Contract(format!(
                    "member '{}' in namespace '{}' has no call function",
                    name, path
                )));
            };
            let qualified = format!("{}:{}", path, name);
            members.insert(
                name.clone(),
                ExtMember {
                    name: name.clone(),
                    qualified: qualified.clone(),
                    arity_min: m.arity_min,
                    arity_max: m.arity_max,
                    call: make_member_call(
                        library.clone(),
                        host_table.clone(),
                        name.clone(),
                        qualified.clone(),
                        call,
                    ),
                },
            );
        }
        registry
            .insert_namespace(ExtNamespace {
                path: path.clone(),
                members,
            })
            .map_err(|conflict| ExtLoadError::NamespaceDuplicate { path: conflict })?;
    }
    Ok(registry)
}

/// 造一个成员的调用闭包。
fn make_member_call(
    library: Rc<Library>,
    host_table: Rc<WlwlExtHost>,
    member_name: String,
    qualified: String,
    call: abi::ExtCallFn,
) -> ExtMemberCall {
    // 成员名在 C 侧要能被扩展读到 ⇒ 存一份 NUL 结尾的副本。
    let member_c = CString::new(member_name).unwrap_or_default();
    Rc::new(move |args: Vec<Value>| {
        // ⚠ **动态库句柄必须活得比这个闭包久** —— 闭包持有它 ⇒ 只要成员
        // 还能被调用，库就不会被卸载。（`move` 闭包只捕获**用到的**变量
        // ⇒ 这里必须真的用一次，不能只写 `_library`。）
        let _keep_library_alive = &library;
        // ── 1. 实参 → JSON ────────────────────────────────────────────
        let payload = json::args_to_json(&args).map_err(|e| {
            ext_diag(
                ErrorCode::E0030,
                format!(
                    "cannot pass this value to extension member '{}': {}",
                    qualified, e
                ),
            )
        })?;
        let text = payload.to_string();
        // ── 2. 用宿主分配器分配 `args_json` ─────────────────────────────
        // ⚠ 扩展**只读**这个缓冲；调用返回后由宿主释放（头文件写死了这一点）。
        let args_ptr = unsafe { host_alloc(text.len() as u32 + 1) };
        if args_ptr.is_null() {
            return Err(ext_diag(
                ErrorCode::E0030,
                format!(
                    "out of memory while calling extension member '{}'",
                    qualified
                ),
            ));
        }
        // SAFETY: `host_alloc` 交的是 `len + 1` 字节。
        unsafe {
            std::ptr::copy_nonoverlapping(text.as_ptr(), args_ptr as *mut u8, text.len());
            *((args_ptr as *mut u8).add(text.len())) = 0;
        }
        // ── 3. 上下文 ──────────────────────────────────────────────────
        let mut ctx = abi::WlwlExtCtx {
            host: Rc::as_ptr(&host_table),
            member: member_c.as_ptr(),
        };
        // ── 4. 调用扩展 ────────────────────────────────────────────────
        let mut out: *mut c_char = std::ptr::null_mut();
        // SAFETY: 契约要求扩展把结果字符串用 `host->alloc` 分配、
        // 由宿主用 `host->free` 释放；`ctx` / `args_ptr` 在调用期间有效。
        let rc = unsafe { call(&mut ctx, args_ptr as *const c_char, &mut out) };
        // ── 5. 取回结果并释放缓冲 ──────────────────────────────────────
        let payload_text = if out.is_null() {
            None
        } else {
            // SAFETY: 扩展交回来的指针在释放前有效（NUL 结尾）。
            let s = unsafe { CStr::from_ptr(out) }
                .to_string_lossy()
                .into_owned();
            // SAFETY: `out` 由扩展用宿主的 `alloc` 分配 ⇒ 用宿主的 `free` 释放。
            unsafe { host_free(out as *mut c_void) };
            Some(s)
        };
        // SAFETY: `args_ptr` 是宿主自己分配的 ⇒ 宿主自己释放。
        unsafe { host_free(args_ptr) };
        // ── 6. 解释结果 ────────────────────────────────────────────────
        interpret(&qualified, rc, payload_text)
    })
}

/// 解释扩展交回来的载荷。
fn interpret(qualified: &str, rc: i32, payload: Option<String>) -> Result<Outcome, WlwlError> {
    let Some(text) = payload else {
        // 扩展没交载荷：要么它失败了，要么它压根没写。
        return Ok(Outcome::normal(Value::Err(Box::new(Value::String(
            format!(
                "extension member '{}' returned no payload (code {})",
                qualified, rc
            ),
        )))));
    };
    let parsed: serde_json::Value = serde_json::from_str(&text).map_err(|e| {
        ext_diag(
            ErrorCode::E0070,
            format!(
                "extension member '{}' returned malformed JSON: {}",
                qualified, e
            ),
        )
    })?;
    let serde_json::Value::Object(mut map) = parsed else {
        return Err(ext_diag(
            ErrorCode::E0030,
            format!(
                "extension member '{}' returned a malformed envelope \
                 (expected an object with 'value' or 'err')",
                qualified
            ),
        ));
    };
    // 失败通道优先：`{"err": {...}}` ⇒ ERR 值。
    if let Some(err_obj) = map.remove("err") {
        return Ok(Outcome::normal(Value::Err(Box::new(err_payload(
            &err_obj, qualified,
        )?))));
    }
    if rc != 0 {
        // 扩展报失败,但没给 `err` 载荷 ⇒ 宿主自己造一个 ERR 值。
        let entries = vec![
            (
                Value::String("kind".into()),
                Value::String("extension_error".into()),
            ),
            (Value::String("op".into()), Value::String(qualified.into())),
            (
                Value::String("reason".into()),
                Value::String(format!(
                    "extension member returned code {} without an error payload",
                    rc
                )),
            ),
        ];
        return Ok(Outcome::normal(Value::Err(Box::new(Value::Dict(
            std::rc::Rc::new(DictInner::new(entries)),
        )))));
    }
    match map.remove("value") {
        Some(v) => Ok(Outcome::normal(json::json_to_value(&v))),
        None => Err(ext_diag(
            ErrorCode::E0030,
            format!(
                "extension member '{}' returned neither 'value' nor 'err'",
                qualified
            ),
        )),
    }
}

/// `{"err": {...}}` 的载荷 ⇒ ERR 值的内容（一个字典）。
fn err_payload(err: &serde_json::Value, qualified: &str) -> Result<Value, WlwlError> {
    let serde_json::Value::Object(map) = err else {
        return Err(ext_diag(
            ErrorCode::E0030,
            format!(
                "extension member '{}' returned an 'err' payload that is not an object",
                qualified
            ),
        ));
    };
    let entries: Vec<(Value, Value)> = map
        .iter()
        .map(|(k, v)| (Value::String(k.clone()), json::json_to_value(v)))
        .collect();
    Ok(Value::Dict(std::rc::Rc::new(DictInner::new(entries))))
}

/// 宿主侧的诊断（扩展边界上的失败 = 语言面失败 ⇒ **既有**诊断码）。
fn ext_diag(code: ErrorCode, message: String) -> WlwlError {
    WlwlDiagnostic::new(code, message, Location::point("<extension>", 0, 0)).into()
}

/// C 串 → Rust `String`（NULL 或非法 UTF-8 ⇒ `None`）。
fn cstr_to_string(p: *const c_char) -> Option<String> {
    if p.is_null() {
        return None;
    }
    // SAFETY: 调用方保证这是 NUL 结尾的 C 串。
    let bytes = unsafe { CStr::from_ptr(p) }.to_bytes();
    std::str::from_utf8(bytes).ok().map(|s| s.to_string())
}
