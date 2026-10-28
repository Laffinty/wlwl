//! 值 ⇄ JSON 的**边界编解码**与**信封**（envelope）。
//!
//! # 契约（与宿主 `impl/crates/wlwl-ext/include/wlwl_ext.h` 一字不差）
//!
//! - 入参：`args_json` 是宿主给的 **JSON 数组文本**（只读，宿主释放）。
//! - 出参：`out_json` 指向扩展交回来的字符串：
//!   - 成功 ⇒ `{"value": <任意 JSON>}`，返回码 `0`；
//!   - 失败 ⇒ `{"err": {"kind": …, "op": …, "reason": …}}`，返回码非 `0`。
//! - ⚠ **那块内存必须用 `ctx->host->alloc()` 分配** —— 宿主会用
//!   `ctx->host->free()` 释放它（Windows 跨 CRT：各用各的分配器 ⇒ 跨堆
//!   释放会崩）。
//!
//! # 失败**不新增诊断码**
//!
//! `err.kind` 是**开放字符串**（不是原生诊断码）⇒ 宿主把它变成 wlwl 的
//! `ERR` **值**，不是诊断。本 crate 只用两个 kind：
//! `TlsError`（`tls` 命名空间）与 `CryptoError`（`crypto` 命名空间）。

use std::ffi::{c_char, CStr, CString};

use serde_json::{json, Map, Value as Json};

use crate::abi::{WlwlExtCtx, WlwlExtHost};

/// `tls` 命名空间的失败 kind。
pub const KIND_TLS: &str = "TlsError";
/// `crypto` 命名空间的失败 kind。
pub const KIND_CRYPTO: &str = "CryptoError";

/// 一个成员的失败。
#[derive(Debug, Clone, PartialEq)]
pub struct ExtError {
    /// `TlsError` 或 `CryptoError`。
    pub kind: &'static str,
    /// 出错的成员名（与 std 的 `op:"TCP_SERVE"` 同口径：**裸成员名**）。
    pub op: &'static str,
    /// 人话原因（**不得**包含私钥、密钥材料）。
    pub reason: String,
}

impl ExtError {
    /// 造一个失败。
    pub fn new(kind: &'static str, op: &'static str, reason: impl Into<String>) -> ExtError {
        ExtError {
            kind,
            op,
            reason: reason.into(),
        }
    }
}

/// 成员的返回值：成功给一个 JSON 值，失败给一个 [`ExtError`]。
pub type MemberResult = Result<Json, ExtError>;

/// 成员实现的形状。
pub type MemberFn = fn(&Ctx, &[Json]) -> MemberResult;

/// 一次成员调用的上下文（宿主给的指针的**安全外壳**）。
///
/// ⚠ 里面只有**裸指针** ⇒ 本 crate 不给 `Ctx` 加 `Send`/`Sync`（宿主侧
/// 本来就是单线程解释器）。
pub struct Ctx {
    raw: *mut WlwlExtCtx,
}

impl Ctx {
    /// 包一层宿主给的上下文指针。
    ///
    /// # Safety
    /// `raw` 必须是宿主在调用期间有效的 `wlwl_ext_ctx_t*`（或 NULL ——
    /// 那时所有回调都变成「什么也不做」）。
    pub unsafe fn from_raw(raw: *mut WlwlExtCtx) -> Ctx {
        Ctx { raw }
    }

    /// 向宿主报告一条信息（宿主会打到 stderr，带 `wlwl: extension report`
    /// 前缀 ⇒ **不**混进程序输出）。
    ///
    /// ⚠ 本 crate 只用它报**告警**（例如「你显式关掉了证书校验」）。
    pub fn report(&self, kind: &str, reason: &str) {
        // SAFETY: `self.raw` 由宿主在调用期间保证有效。
        unsafe {
            if self.raw.is_null() {
                return;
            }
            let host: *const WlwlExtHost = (*self.raw).host;
            if host.is_null() {
                return;
            }
            let Some(report) = (*host).report else {
                return;
            };
            let Ok(k) = CString::new(kind) else {
                return;
            };
            let Ok(r) = CString::new(reason) else {
                return;
            };
            report(self.raw, k.as_ptr(), r.as_ptr());
        }
    }
}

/// 解析实参。
fn parse_args(args_json: *const c_char) -> Result<Vec<Json>, String> {
    if args_json.is_null() {
        return Ok(Vec::new());
    }
    // SAFETY: 宿主保证这是 NUL 结尾的 C 串，且在调用期间有效。
    let text = unsafe { CStr::from_ptr(args_json) }.to_str().map_err(|_| {
        "the arguments are not valid UTF-8 (this is a host-side encoding bug)".to_string()
    })?;
    let parsed: Json = serde_json::from_str(text)
        .map_err(|e| format!("the arguments are not valid JSON: {e}"))?;
    match parsed {
        Json::Array(items) => Ok(items),
        _ => Err("the arguments must be a JSON array".to_string()),
    }
}

/// 把结果写进 `*out_json`（用**宿主**的分配器）。
///
/// # Safety
/// `ctx` / `out_json` 必须是宿主在调用期间有效的指针。
unsafe fn write_out(ctx: *mut WlwlExtCtx, out_json: *mut *mut c_char, payload: &str) -> i32 {
    if ctx.is_null() || out_json.is_null() {
        return 1;
    }
    let host: *const WlwlExtHost = (*ctx).host;
    if host.is_null() {
        return 1;
    }
    let Some(alloc) = (*host).alloc else {
        return 1;
    };
    let bytes = payload.as_bytes();
    let size = bytes.len() + 1;
    if size > u32::MAX as usize {
        return 1;
    }
    let buf = alloc(size as u32);
    if buf.is_null() {
        return 1;
    }
    std::ptr::copy_nonoverlapping(bytes.as_ptr(), buf as *mut u8, bytes.len());
    *((buf as *mut u8).add(bytes.len())) = 0;
    *out_json = buf as *mut c_char;
    0
}

/// 一个成员的统一入口：解析实参 → 调实现 → 写信封。
///
/// # Safety
/// `ctx` / `args_json` / `out_json` 必须符合 ABI 契约。
pub unsafe fn run(
    ctx: *mut WlwlExtCtx,
    args_json: *const c_char,
    out_json: *mut *mut c_char,
    kind: &'static str,
    op: &'static str,
    handler: MemberFn,
) -> i32 {
    let reply = match parse_args(args_json) {
        Ok(args) => {
            let c = Ctx::from_raw(ctx);
            handler(&c, &args)
        }
        Err(reason) => Err(ExtError::new(kind, op, reason)),
    };
    let (payload, failed) = match reply {
        Ok(value) => (json!({ "value": value }), false),
        Err(e) => (
            json!({
                "err": { "kind": e.kind, "op": e.op, "reason": e.reason }
            }),
            true,
        ),
    };
    let rc = write_out(ctx, out_json, &payload.to_string());
    if rc != 0 {
        // 连载荷都写不出去 ⇒ 交回一个**失败**码（宿主会自己造 ERR 值）。
        return rc;
    }
    if failed {
        1
    } else {
        0
    }
}

/// 造一个成员的 `extern "system"` 入口。
///
/// ABI v1 的成员表里放的是**函数指针** ⇒ 每个成员都要有一个自己的入口
/// 符号（不能一个入口靠 `ctx->member` 分派 —— 那样宿主读到的元数就没法
/// 逐成员声明了）。
macro_rules! member_entry {
    ($fn_name:ident, $kind:expr, $op:expr, $handler:path) => {
        /// ABI v1 的成员入口。
        ///
        /// # Safety
        /// 由宿主按 ABI 契约调用。
        pub unsafe extern "system" fn $fn_name(
            ctx: *mut $crate::abi::WlwlExtCtx,
            args_json: *const ::std::ffi::c_char,
            out_json: *mut *mut ::std::ffi::c_char,
        ) -> i32 {
            // SAFETY: 契约保证三个指针在调用期间有效。
            unsafe { $crate::envelope::run(ctx, args_json, out_json, $kind, $op, $handler) }
        }
    };
}
pub(crate) use member_entry;

/// 一个 `DICT` 形状的返回值（键顺序 = 插入顺序，与 `TCP_RECV` 一致）。
pub fn dict(entries: Vec<(&str, Json)>) -> Json {
    let mut map = Map::new();
    for (k, v) in entries {
        map.insert(k.to_string(), v);
    }
    Json::Object(map)
}
