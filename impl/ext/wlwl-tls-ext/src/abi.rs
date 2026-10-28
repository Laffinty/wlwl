//! 扩展 ABI 的**单一真相** = 宿主侧的定义。
//!
//! ⚠ 这里**直接引用**宿主 crate 的 `abi.rs`（`#[path]`），**不复制**、**不重写**
//! —— 宿主改了 ABI（加字段、改元数），本 crate 自动跟上，不会漂移。
//!
//! 宿主侧的那份文件：
//! `impl/crates/wlwl-ext/src/abi.rs`（Rust 镜像）
//! `impl/crates/wlwl-ext/include/wlwl_ext.h`（C 头文件形态）
//!
//! ## 为什么可以直接引用
//!
//! 宿主那份 `abi.rs` 里**只有** `#[repr(C)]` 结构体与 `extern "system"`
//! 函数指针类型 —— **不引用任何宿主侧的值类型**（没有 `wlwl_value`）⇒
//! 本 crate 把它当普通模块编进来即可，**不产生 crate 依赖**。
//!
//! ⇒ 本 crate 不需要 `src/abi_local.rs` 副本。但**布局守卫**仍然要有
//! （见 `tests/abi_layout.rs`）：它把本 crate 看到的结构体大小/字段偏移
//! 与 C 头文件里那套 `struct` 声明逐字对照 —— 哪天宿主改了字段顺序而
//! 忘了改头文件（或反之），那条用例会红。

#[path = "../../../crates/wlwl-ext/src/abi.rs"]
mod host_abi;

pub use host_abi::*;
