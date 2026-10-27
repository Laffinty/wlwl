//! [W-21] **宿主侧的原生动态库扩展加载器**。
//!
//! # 这条通道是干什么的
//!
//! 宿主能在**显式授权**下加载一个原生动态库（DLL / SO），把它声明的成员
//! 挂成可 `IMPORT` 的命名空间。
//!
//! # 这条通道**不是**什么
//!
//! - **不是沙箱**：加载一个原生库 = 把**整个进程**的权限交给它。
//!   ⇒ 加载前必有一行醒目告警（信任门控 T6）。
//! - **不是语言面的一部分**：**语言面绝不提供任何 `LOAD` 成员** ⇒
//!   脚本**不能**触发代码加载。只有 CLI `--ext` / 环境变量
//!   `WLWL_EXT_PATH` 能加载（T1）。
//! - **不是自动发现的**：本 crate **没有任何**目录遍历 / 清单读取 /
//!   文件名猜测的入口（T2）⇒ 加载路径只能由调用方逐字给出。
//!
//! # 依赖方向
//!
//! `wlwl-ext` → `wlwl-value`（值模型）+ `wlwl-ast`（SHA-256）+ `wlwl-std`
//! （std 命名空间清单）。`wlwl-eval` **不依赖**本 crate：它只认
//! [`wlwl_value::ExtRegistry`] 这张运行期表 ⇒ 语言面没有 FFI 的概念。
//!
//! # ABI
//!
//! v1 的契约见 `include/wlwl_ext.h`（C 头文件形态）与
//! [`abi`]（Rust 侧镜像）。扩展导出**唯一**一个符号 `wlwl_ext_v1`。
//!
//! # 失败怎么报
//!
//! - **宿主侧失败**（路径不对 / 哈希不对 / ABI 不对 / 命名空间占用…）
//!   ⇒ [`ExtLoadError`] ⇒ 调用方打 **stderr 一行** + **非零退出码**
//!   （**不新增诊断码**）。
//! - **语言面失败**（模块找不到 ⇒ `E0040`、元数错 ⇒ `E0022`、
//!   类型错 ⇒ `E0030`、扩展回传的 JSON 不合法 ⇒ `E0070`）
//!   ⇒ **既有**诊断码。

pub mod abi;
pub mod dynlib;
pub mod json;
pub mod registry;
pub mod trust;

pub use abi::{ABI_VERSION, SYMBOL_NAME};
pub use trust::{ExtLoadError, ExtLoadRequest};
// [W-21] 注册表类型**转出**一下:调用方(CLI)只依赖本 crate 就够,
// 不用再直接依赖 `wlwl-value`。
pub use wlwl_value::{ExtMember, ExtNamespace, ExtRegistry};

use std::path::PathBuf;
use std::rc::Rc;

/// 加载成功的结果。
pub struct LoadedExtension {
    /// 运行期注册表 —— 交给求值器(`Evaluator::with_ext_registry`)。
    pub registry: Rc<ExtRegistry>,
    /// 实际加载的路径。
    pub path: PathBuf,
    /// 文件的 SHA-256(小写 hex)。
    pub sha256: String,
    /// 扩展报告的 ABI 版本(必须 == 1)。
    pub abi_version: u32,
    /// 扩展报告的宿主版本文本(可空)。
    pub host_version: Option<String>,
    /// 宿主回调表 —— **必须活得比扩展久**(扩展在成员调用期间读它)。
    _host: Rc<abi::WlwlExtHost>,
}

impl std::fmt::Debug for LoadedExtension {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoadedExtension")
            .field("path", &self.path)
            .field("sha256", &self.sha256)
            .field("abi_version", &self.abi_version)
            .field("host_version", &self.host_version)
            .field("namespaces", &self.registry.namespace_paths())
            .finish()
    }
}

/// 加载一个扩展动态库。
///
/// 顺序(= 信任门控的落点):
/// 1. **T3** 路径必须绝对、必须存在;
/// 2. **T4** 给了期望摘要就必须匹配;
/// 3. **T6** 打印告警(**在加载之前**);
/// 4. 打开动态库、取 `wlwl_ext_v1`;
/// 5. 校验 `abi_version` / `struct_size`;
/// 6. **T5** 逐个校验声明的命名空间(不得占用既有命名空间)。
pub fn load_extension(req: &ExtLoadRequest) -> Result<LoadedExtension, ExtLoadError> {
    // ── T3:路径 ────────────────────────────────────────────────────
    trust::check_absolute(&req.path)?;
    trust::check_exists(&req.path)?;

    // ── T4:摘要 ────────────────────────────────────────────────────
    let sha256 = trust::check_sha256(&req.path, req.expected_sha256.as_deref())?;

    // ── T6:告警(**加载之前**)────────────────────────────────────────
    eprintln!("{}", trust::trust_warning(&req.path));

    // ── 打开动态库 ─────────────────────────────────────────────────
    let library =
        Rc::new(
            dynlib::Library::open(&req.path).map_err(|detail| ExtLoadError::LibraryOpen {
                path: req.path.clone(),
                detail,
            })?,
        );

    // ── 取符号 ─────────────────────────────────────────────────────
    let symbol = library
        .symbol(SYMBOL_NAME)
        .ok_or_else(|| ExtLoadError::SymbolMissing {
            symbol: SYMBOL_NAME.to_string(),
        })?;

    // ⚠ 读任何字段**之前**先校验版本与结构大小:前两个字段都是 `u32`,
    // 任何 conforming 扩展都有 ⇒ 先读这 8 个字节是安全的。
    let v1_ptr = symbol as *mut abi::WlwlExtV1;
    let (abi_version, struct_size) = unsafe {
        let head = v1_ptr as *const u32;
        ((*head).clone(), (*head.add(1)).clone())
    };
    if abi_version != abi::ABI_VERSION {
        return Err(ExtLoadError::AbiVersionMismatch {
            found: abi_version,
            expected: abi::ABI_VERSION,
        });
    }
    let required = std::mem::size_of::<abi::WlwlExtV1>() as u32;
    if struct_size < required {
        return Err(ExtLoadError::StructSizeTooSmall {
            found: struct_size,
            required,
        });
    }

    // SAFETY: 版本与结构大小都已校验 ⇒ 下面的字段都真实存在。
    let (host_version, ns_count, namespaces) = unsafe {
        let v = &*v1_ptr;
        (v.host_version, v.ns_count, v.namespaces)
    };
    let host_version = if host_version.is_null() {
        None
    } else {
        // SAFETY: 契约要求它是 NUL 结尾的 C 串(或 NULL)。
        let bytes = unsafe { std::ffi::CStr::from_ptr(host_version) }.to_bytes();
        std::str::from_utf8(bytes).ok().map(|s| s.to_string())
    };
    let _ = (ns_count, namespaces); // 装配时再读(要可变借用)。

    // ── 宿主回调表(**由宿主填写**)─────────────────────────────────
    let host_table = Rc::new(abi::WlwlExtHost {
        alloc: Some(registry::host_alloc),
        free: Some(registry::host_free),
        report: Some(registry::host_report),
        await_ready: Some(registry::host_await_ready),
    });
    // SAFETY: 版本/大小已校验 ⇒ `host` 字段存在且可写。
    unsafe {
        (*v1_ptr).host = Rc::as_ptr(&host_table) as *mut abi::WlwlExtHost;
    }

    // ── 装配注册表(含 T5 的命名空间门控)────────────────────────────
    let mut reserved = trust::std_reserved_namespaces();
    reserved.extend(req.reserved_namespaces.iter().cloned());
    // SAFETY: 同上 —— 结构已校验。
    let ext_registry = unsafe { registry::build(&mut *v1_ptr, &library, &host_table, &reserved)? };

    Ok(LoadedExtension {
        registry: Rc::new(ext_registry),
        path: req.path.clone(),
        sha256,
        abi_version,
        host_version,
        _host: host_table,
    })
}
