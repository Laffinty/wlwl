//! [W-21] 动态库的**原始 FFI**：打开、取符号、释放。
//!
//! ⚠ **零新增第三方依赖**（本仓的依赖纪律）：手写 `extern "system"` 声明，
//! 形状照 `wlwl-eval/tests/common/handle_guard.rs` 里的 kernel32 样板。
//!
//! - Windows：`LoadLibraryExW` + `GetProcAddress`（`#[link(name = "kernel32")]`）
//! - POSIX：`dlopen` / `dlsym`（`#[link(name = "dl")]`）

use std::ffi::{c_char, c_void, CString};
use std::path::Path;

/// 已打开的动态库句柄。**Drop 时释放**（`FreeLibrary` / `dlclose`）。
///
/// ⚠ 句柄的存活期必须覆盖**所有**对扩展成员的调用 —— 所以每个成员闭包
/// 都持有一份 `Rc<Library>`（见 `crate::registry`）。
pub struct Library {
    handle: *mut c_void,
    path: std::path::PathBuf,
}

// ⚠ 句柄里是裸指针 ⇒ `Library` 自动 **不是** `Send`/`Sync`
// （解释器是单线程的，§16.1 调度器边界）⇒ 跨线程移动它没有意义，
// 编译器也不允许。

impl Library {
    /// 打开动态库。失败时返回宿主侧错误（不是语言诊断）。
    pub fn open(path: &Path) -> Result<Library, String> {
        let handle = imp::open(path)?;
        if handle.is_null() {
            return Err(format!(
                "could not open dynamic library '{}'",
                path.display()
            ));
        }
        Ok(Library {
            handle,
            path: path.to_path_buf(),
        })
    }

    /// 取符号地址；不存在返回 `None`。
    pub fn symbol(&self, name: &str) -> Option<*mut c_void> {
        imp::symbol(self.handle, name)
    }

    /// 打开的路径（诊断用）。
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }
}

impl Drop for Library {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            imp::close(self.handle);
        }
    }
}

// ── Windows ────────────────────────────────────────────────────────────

#[cfg(windows)]
mod imp {
    use super::*;
    use std::os::windows::ffi::OsStrExt;

    #[link(name = "kernel32")]
    extern "system" {
        fn LoadLibraryExW(file: *const u16, reserved: *mut c_void, flags: u32) -> *mut c_void;
        fn GetProcAddress(module: *mut c_void, name: *const c_char) -> *mut c_void;
        fn FreeLibrary(module: *mut c_void) -> i32;
    }

    /// `LOAD_WITH_ALTERED_SEARCH_PATH`：依赖项从 **DLL 自己所在的目录**找，
    /// 而不是从当前工作目录找 ⇒ CWD 里放一个同名 DLL 劫持不了它。
    const LOAD_WITH_ALTERED_SEARCH_PATH: u32 = 0x0000_0008;

    pub fn open(path: &Path) -> Result<*mut c_void, String> {
        // ⚠ 信任门控 T3 保证路径是绝对的 ⇒ 这里不做相对路径补全。
        let wide: Vec<u16> = path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        // SAFETY: `wide` 是 NUL 结尾的 UTF-16 串，`flags` 为常量。
        let handle = unsafe {
            LoadLibraryExW(
                wide.as_ptr(),
                std::ptr::null_mut(),
                LOAD_WITH_ALTERED_SEARCH_PATH,
            )
        };
        if handle.is_null() {
            return Err(format!(
                "could not load dynamic library '{}' (Windows error {})",
                path.display(),
                last_error()
            ));
        }
        Ok(handle)
    }

    pub fn symbol(handle: *mut c_void, name: &str) -> Option<*mut c_void> {
        let c = CString::new(name).ok()?;
        // SAFETY: `c` 是 NUL 结尾的 C 串；`handle` 来自 `LoadLibraryExW`。
        let p = unsafe { GetProcAddress(handle, c.as_ptr()) };
        if p.is_null() {
            None
        } else {
            Some(p)
        }
    }

    pub fn close(handle: *mut c_void) {
        // SAFETY: `handle` 来自 `LoadLibraryExW`，只在这里释放一次。
        unsafe {
            FreeLibrary(handle);
        }
    }

    fn last_error() -> u32 {
        #[link(name = "kernel32")]
        extern "system" {
            fn GetLastError() -> u32;
        }
        // SAFETY: 无参数 Win32 API。
        unsafe { GetLastError() }
    }
}

// ── POSIX ──────────────────────────────────────────────────────────────

#[cfg(unix)]
mod imp {
    use super::*;

    // ⚠ **macOS 没有 libdl** —— `dlopen`/`dlsym` 在 **libSystem** 里
    // (Linux/BSD 才有独立的 `libdl.so`)。
    // ⇒ macOS 链 `System` framework,其它 POSIX 链 `dl`。
    // ⚠ 本仓 CI 有 macOS ⇒ 写死 `dl` 会让 macOS 那条构建**链不过**。
    #[cfg_attr(target_os = "macos", link(name = "System", kind = "framework"))]
    #[cfg_attr(not(target_os = "macos"), link(name = "dl"))]
    extern "system" {
        fn dlopen(filename: *const c_char, flag: i32) -> *mut c_void;
        fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
        fn dlclose(handle: *mut c_void) -> i32;
    }

    const RTLD_NOW: i32 = 2;
    const RTLD_LOCAL: i32 = 0;

    pub fn open(path: &Path) -> Result<*mut c_void, String> {
        let c = CString::new(path.to_string_lossy().as_bytes()).map_err(|e| e.to_string())?;
        // SAFETY: `c` 是 NUL 结尾的 C 串；flag 是常量。
        let handle = unsafe { dlopen(c.as_ptr(), RTLD_NOW | RTLD_LOCAL) };
        if handle.is_null() {
            return Err(format!(
                "could not load dynamic library '{}' (dlopen failed)",
                path.display()
            ));
        }
        Ok(handle)
    }

    pub fn symbol(handle: *mut c_void, name: &str) -> Option<*mut c_void> {
        let c = CString::new(name).ok()?;
        // SAFETY: `c` 是 NUL 结尾的 C 串；`handle` 来自 `dlopen`。
        let p = unsafe { dlsym(handle, c.as_ptr()) };
        if p.is_null() {
            None
        } else {
            Some(p)
        }
    }

    pub fn close(handle: *mut c_void) {
        // SAFETY: `handle` 来自 `dlopen`，只在这里释放一次。
        unsafe {
            dlclose(handle);
        }
    }
}
