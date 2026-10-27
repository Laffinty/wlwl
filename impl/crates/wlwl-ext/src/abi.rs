//! [W-21] 扩展 ABI v1 的 **Rust 侧镜像**。
//!
//! C 头文件形态见 `include/wlwl_ext.h`（同一份契约的两种写法）。
//! ⚠ **两侧必须逐字一致**：字段顺序、`#[repr(C)]` 的对齐/填充、函数指针
//! 的调用约定（`extern "system"`）任一处不一致 ⇒ 读到的是垃圾数据。
//! ⇒ 宿主侧在读任何字段**之前**先校验 `abi_version` 与 `struct_size`
//! （见 [`crate::load_extension`]），不匹配就**拒绝加载**。

use std::ffi::{c_char, c_void};

/// ABI 版本号。宿主只接受 `1`。
pub const ABI_VERSION: u32 = 1;

/// 扩展**唯一**允许导出的符号名（指向 [`WlwlExtV1`]）。
pub const SYMBOL_NAME: &str = "wlwl_ext_v1";

/// `arity_max` 的哨兵值：**变参**（不限上界）。
pub const ARITY_UNBOUNDED: u32 = u32::MAX;

/// 扩展成员的调用约定。
///
/// ⚠ `unsafe extern "system"`：宿主只保证**参数与返回约定**，
/// 不保证扩展的实现是安全的 —— 扩展拿到的是**宿主进程的���部权限**
/// （这正是信任门控 T6 要警告的那件事）。
pub type ExtCallFn = unsafe extern "system" fn(
    ctx: *mut WlwlExtCtx,
    args_json: *const c_char,
    out_json: *mut *mut c_char,
) -> i32;

/// 宿主分配器：`size` 字节 ⇒ 指针（失败返回 NULL）。
pub type ExtAllocFn = unsafe extern "system" fn(size: u32) -> *mut c_void;

/// 宿主释放器：释放 [`ExtAllocFn`] 交出来的指针。
pub type ExtFreeFn = unsafe extern "system" fn(ptr: *mut c_void);

/// 扩展向宿主报告一条信息（`kind` / `reason` 都是 UTF-8 文本）。
pub type ExtReportFn =
    unsafe extern "system" fn(ctx: *mut WlwlExtCtx, kind: *const c_char, reason: *const c_char);

/// 扩展询问某个句柄是否就绪。
///
/// ⚠ **v1 不支持**：宿主一律返回 `-1`（不支持）。理由见
/// [`crate::abi`] 的模块文档 —— 异步扩展成员需要调度器参与，
/// 而 ABI v1 的 `wlwl_ext_ctx_t` 里没有那条通道。
pub type ExtAwaitReadyFn =
    unsafe extern "system" fn(ctx: *mut WlwlExtCtx, handle: u32, timeout_ms: u32) -> i32;

/// 每次成员调用时交给扩展的上下文。
///
/// ⚠ **v1 的形状是本仓定的**（题面只给了 `wlwl_ext_ctx_t*` 这个指针），
/// 所以头文件里必须写死同一份定义。
#[repr(C)]
pub struct WlwlExtCtx {
    /// 宿主回调表（`alloc` / `free` / `report` / `await_ready`）。
    pub host: *const WlwlExtHost,
    /// 当前被调用的成员名（UTF-8，NUL 结尾）。
    pub member: *const c_char,
}

/// 一个扩展成员。
#[repr(C)]
pub struct WlwlExtMember {
    /// 成员名（UTF-8，NUL 结尾）；不得为空。
    pub name: *const c_char,
    /// 最少实参个数。
    pub arity_min: u32,
    /// 最多实参个数；[`ARITY_UNBOUNDED`] = 变参。
    pub arity_max: u32,
    /// 成员实现。
    pub call: Option<ExtCallFn>,
}

/// 一个扩展命名空间。
#[repr(C)]
pub struct WlwlExtNs {
    /// 命名空间路径（如 `"tls"`）；**不得**是 `wlwl:std.*`。
    pub path: *const c_char,
    /// 成员数。
    pub member_count: u32,
    /// 成员表。
    pub members: *const WlwlExtMember,
}

/// 宿主 → 扩展 的回调表。
#[repr(C)]
pub struct WlwlExtHost {
    /// 分配器。扩展**必须**用它分配要交回来的字符串 ——
    /// ⚠ Windows 跨 CRT：各用各的分配器 ⇒ 释放时跨堆 ⇒ 崩。
    pub alloc: Option<ExtAllocFn>,
    /// 释放器。扩展交回来的字符串由宿主用它释放（同一个分配器）。
    pub free: Option<ExtFreeFn>,
    /// 扩展报告一条信息。
    pub report: Option<ExtReportFn>,
    /// 询问句柄就绪（v1 恒为不支持）。
    pub await_ready: Option<ExtAwaitReadyFn>,
}

/// 扩展导出的**唯一**符号指向的结构。
#[repr(C)]
pub struct WlwlExtV1 {
    /// 必须 == [`ABI_VERSION`]。
    pub abi_version: u32,
    /// 本结构的大小（字节）；必须 >= 宿主看到的 `size_of::<WlwlExtV1>()`。
    pub struct_size: u32,
    /// 宿主版本文本（UTF-8，NUL 结尾；可为 NULL）。
    pub host_version: *const c_char,
    /// 命名空间个数。
    pub ns_count: u32,
    /// 命名空间表。
    pub namespaces: *const WlwlExtNs,
    /// 宿主回调表（**由宿主填写**，扩展只读）。
    pub host: *mut WlwlExtHost,
}
