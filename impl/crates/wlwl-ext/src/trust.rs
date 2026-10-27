//! [W-21] **信任门控**（七条中的 T1–T6；T7 是文档纪律，落在别处）。
//!
//! | 条 | 内容 | 落点 |
//! |---|---|---|
//! | T1 | 只有 CLI `--ext` / 环境变量 `WLWL_EXT_PATH` 能加载；语言面无 LOAD 成员 | 加载**只**接受 [`ExtLoadRequest`]（没有任何发现/扫描入口）；语言面根本没有 LOAD 成员 |
//! | T2 | 绝不从项目配置 / 项目目录 / `./ext/` 自动发现 | 本模块**没有任何目录遍历**；加载路径只能由调用方逐字给出 |
//! | T3 | 路径必须**绝对**且存在 | [`check_absolute`] / [`check_exists`] |
//! | T4 | `--ext-sha256` 给了就必须匹配 | [`check_sha256`] |
//! | T5 | 不得占用 `std.*`、不得覆盖既有命名空间 | [`check_namespace_allowed`] + [`std_reserved_namespaces`] |
//! | T6 | 加载前打印一行醒目告警 | [`trust_warning`]（由 `load_extension` 在 `dlopen` **之前**打印） |
//!
//! ⚠ **T1 的落点是「没有第二条路」**：本 crate 不导出任何「扫描目录 /
//! 读清单 / 猜文件名」的函数 ⇒ 想要加载扩展就只有 `--ext` /
//! `WLWL_EXT_PATH` 这两条**显式**通道。

use std::path::{Path, PathBuf};

use crate::abi::ABI_VERSION;

/// 加载请求 —— **唯一**能触发加载的东西（T1/T2 的落点）。
///
/// ⚠ 没有「默认路径」「搜索目录」「从清单读」这类字段：请求里有什么
/// 就加载什么，一个字节都不多。
#[derive(Debug, Clone, Default)]
pub struct ExtLoadRequest {
    /// 扩展动态库的路径。**必须绝对**（T3）。
    pub path: PathBuf,
    /// 期望的 SHA-256（小写 hex，64 个字符）。给了就**必须**匹配（T4）。
    pub expected_sha256: Option<String>,
    /// 不得被覆盖的既有命名空间名（项目清单里的命名空间等）。
    ///
    /// std 的命名空间名由本 crate **内部**补齐（[`std_reserved_namespaces`]），
    /// 免得这份清单变成会漂移的手写副本。
    pub reserved_namespaces: Vec<String>,
}

/// 宿主侧加载失败 —— **不是**语言诊断。
///
/// ⚠ 宿主侧失败一律是「stderr 一行 + 非零退出码」（禁新增诊断码）。
/// 语言面的失败（模块找不到 ⇒ `E0040`、元数错 ⇒ `E0022`、类型错 ⇒ `E0030`）
/// 走既有诊断码。
#[derive(Debug, Clone, PartialEq)]
pub enum ExtLoadError {
    /// T3：路径不是绝对路径。
    NotAbsolute(PathBuf),
    /// T3：路径不存在。
    Missing(PathBuf),
    /// T3：路径存在但不是普通文件。
    NotAFile(PathBuf),
    /// T4：SHA-256 不匹配。
    Sha256Mismatch { expected: String, actual: String },
    /// ABI 版本不是 1。
    AbiVersionMismatch { found: u32, expected: u32 },
    /// 扩展的结构体太小，撑不住宿主看到的字段。
    StructSizeTooSmall { found: u32, required: u32 },
    /// 扩展没有导出 `wlwl_ext_v1`。
    SymbolMissing { symbol: String },
    /// 动态库打不开。
    LibraryOpen { path: PathBuf, detail: String },
    /// T5：命名空间占用了既有命名空间（std.* 或调用方给的清单）。
    NamespaceReserved { path: String },
    /// T5：同一个扩展里声明了重名的命名空间。
    NamespaceDuplicate { path: String },
    /// 命名空间路径本身不合法。
    NamespacePathInvalid { path: String, reason: String },
    /// 成员名不合法。
    MemberNameInvalid { namespace: String, name: String },
    /// ABI 数据自相矛盾（成员表为空指针、计数离谱等）。
    Contract(String),
}

impl ExtLoadError {
    /// 一行式说明（宿主侧 stderr 用）。
    pub fn message(&self) -> String {
        match self {
            ExtLoadError::NotAbsolute(p) => {
                format!("extension path must be absolute (got '{}')", p.display())
            }
            ExtLoadError::Missing(p) => {
                format!("extension library not found: '{}'", p.display())
            }
            ExtLoadError::NotAFile(p) => {
                format!("extension path is not a regular file: '{}'", p.display())
            }
            ExtLoadError::Sha256Mismatch { expected, actual } => format!(
                "extension SHA-256 mismatch: expected {}, got {}",
                expected, actual
            ),
            ExtLoadError::AbiVersionMismatch { found, expected } => format!(
                "extension ABI version mismatch: extension reports {}, host requires {}",
                found, expected
            ),
            ExtLoadError::StructSizeTooSmall { found, required } => format!(
                "extension struct_size {} is smaller than the host's {} bytes",
                found, required
            ),
            ExtLoadError::SymbolMissing { symbol } => {
                format!("extension does not export the required symbol '{}'", symbol)
            }
            ExtLoadError::LibraryOpen { path, detail } => {
                format!("could not load extension '{}': {}", path.display(), detail)
            }
            ExtLoadError::NamespaceReserved { path } => format!(
                "extension namespace '{}' collides with an existing namespace \
                 (native extensions must not shadow std.* or a registered namespace)",
                path
            ),
            ExtLoadError::NamespaceDuplicate { path } => {
                format!("extension declares namespace '{}' more than once", path)
            }
            ExtLoadError::NamespacePathInvalid { path, reason } => {
                format!("extension namespace path '{}' is invalid: {}", path, reason)
            }
            ExtLoadError::MemberNameInvalid { namespace, name } => format!(
                "extension member name '{}' in namespace '{}' is invalid",
                name, namespace
            ),
            ExtLoadError::Contract(detail) => {
                format!("extension violates the ABI contract: {}", detail)
            }
        }
    }
}

impl std::fmt::Display for ExtLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message())
    }
}

impl std::error::Error for ExtLoadError {}

/// T3：路径必须**绝对**。
///
/// ⚠ 相对路径会让「加载哪个文件」取决于当前工作目录 ⇒ 那就是一条
/// **隐式**加载通道（T1/T2 要挡的正是这个）。
pub fn check_absolute(path: &Path) -> Result<(), ExtLoadError> {
    if path.is_absolute() {
        Ok(())
    } else {
        Err(ExtLoadError::NotAbsolute(path.to_path_buf()))
    }
}

/// T3：路径必须**存在**且是普通文件。
pub fn check_exists(path: &Path) -> Result<(), ExtLoadError> {
    if !path.exists() {
        return Err(ExtLoadError::Missing(path.to_path_buf()));
    }
    if !path.is_file() {
        return Err(ExtLoadError::NotAFile(path.to_path_buf()));
    }
    Ok(())
}

/// T4：给了期望摘要就**必须**匹配；返回文件实际的 SHA-256（小写 hex）。
///
/// ⚠ 比对**不区分大小写**（用户可能从 `sha256sum` 抄到大写），
/// 但**长度与字符集**必须合法（64 个 hex 字符）—— 写错的长度直接判失败，
/// 不当成「没给」。
pub fn check_sha256(path: &Path, expected: Option<&str>) -> Result<String, ExtLoadError> {
    let actual = sha256_file(path)?;
    let Some(expected) = expected else {
        return Ok(actual);
    };
    let expected = expected.trim();
    if !is_hex64(expected) {
        return Err(ExtLoadError::Sha256Mismatch {
            expected: expected.to_string(),
            actual,
        });
    }
    if !expected.eq_ignore_ascii_case(&actual) {
        return Err(ExtLoadError::Sha256Mismatch {
            expected: expected.to_string(),
            actual,
        });
    }
    Ok(actual)
}

/// 文件的 SHA-256（小写 hex）。
///
/// ⚠ 复用 `wlwl_ast::sha256` —— 与 `std.encode::SHA256` **同一个实现**
/// （那本仓没有第二份摘要代码，也不会漂移）。
pub fn sha256_file(path: &Path) -> Result<String, ExtLoadError> {
    let bytes = std::fs::read(path).map_err(|e| {
        ExtLoadError::Contract(format!("could not read '{}': {}", path.display(), e))
    })?;
    Ok(hex(&wlwl_ast::sha256::sha256(&bytes)))
}

/// 小写 hex 编码。
pub fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(DIGITS[(b >> 4) as usize] as char);
        out.push(DIGITS[(b & 0x0f) as usize] as char);
    }
    out
}

/// 是不是 64 个 hex 字符。
fn is_hex64(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

/// std 的**全部**命名空间短名（`"io"` / `"str"` / …）。
///
/// ⚠ 取自 `wlwl_std::namespace_paths()`（实现真相），**不是**手写副本 ——
/// 手写清单会随 std 增删成员而漂移 ⇒ 门控会漏。
pub fn std_reserved_namespaces() -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for path in wlwl_std::namespace_paths() {
        // "wlwl:std.io" -> "io"
        let short = path
            .rsplit('.')
            .next()
            .filter(|s| !s.is_empty())
            .unwrap_or(path);
        out.push(short.to_string());
    }
    // "std" 本身与 "wlwl" 前缀也不许被占用。
    out.push("std".to_string());
    out.push("wlwl".to_string());
    out.sort();
    out.dedup();
    out
}

/// T5：命名空间路径**合法**且**不占用**既有命名空间。
///
/// 合法性：
/// - 非空；
/// - 不含 `:`（运行期派发键是 `<ns>:<member>`，`:` 会让键有歧义）；
/// - 不以 `wlwl` 开头（那是 std 的前缀，ABI 头文件明写「不得是 `wlwl:std.*`」）；
/// - 不以 `.` / `/` 开头（那会让它看起来像模块路径而不是命名空间）。
pub fn check_namespace_allowed(ns: &str, reserved: &[String]) -> Result<(), ExtLoadError> {
    if ns.is_empty() {
        return Err(ExtLoadError::NamespacePathInvalid {
            path: ns.to_string(),
            reason: "namespace path must not be empty".into(),
        });
    }
    if ns.contains(':') {
        return Err(ExtLoadError::NamespacePathInvalid {
            path: ns.to_string(),
            reason: "namespace path must not contain ':'".into(),
        });
    }
    if ns.starts_with("wlwl") {
        return Err(ExtLoadError::NamespacePathInvalid {
            path: ns.to_string(),
            reason: "namespace path must not start with 'wlwl' (that prefix belongs to std)".into(),
        });
    }
    if ns.starts_with('.') || ns.starts_with('/') {
        return Err(ExtLoadError::NamespacePathInvalid {
            path: ns.to_string(),
            reason: "namespace path must not start with '.' or '/'".into(),
        });
    }
    if reserved.iter().any(|r| r == ns) {
        return Err(ExtLoadError::NamespaceReserved {
            path: ns.to_string(),
        });
    }
    Ok(())
}

/// T6：加载前的**醒目告警**那一行。
///
/// ⚠ 这句话是**事实陈述**，不是免责声明：加载一个原生库 = 把**整个进程**
/// 的权限交给它（读文件、连网络、改内存、起进程…）—— 它**不是**沙箱。
pub fn trust_warning(path: &Path) -> String {
    format!(
        "wlwl: WARNING: loading native extension '{}' -- this hands the \
         extension FULL PROCESS PRIVILEGES (file system, network, memory, \
         processes). It is NOT a sandbox.",
        path.display()
    )
}

/// 本 crate 支持的 ABI 版本（诊断用）。
pub fn supported_abi_version() -> u32 {
    ABI_VERSION
}

// ──────────────────────────────────────────────────────────────────────
// 自测
// ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// 本 crate 自己的源文件(绝对路径,拿来当「被哈希的文件」)。
    fn self_source() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/trust.rs")
    }

    /// T3:相对路径被拒。
    ///
    /// ⚠ **反向守卫**:去掉 `check_absolute` 里的绝对性判据 ⇒ 本用例变红。
    #[test]
    fn a_relative_path_is_rejected() {
        let err = check_absolute(Path::new("ext.dll")).expect_err("relative must be rejected");
        assert!(matches!(err, ExtLoadError::NotAbsolute(_)), "got {err:?}");
        assert!(check_absolute(&self_source()).is_ok());
    }

    /// T3:不存在的路径被拒。
    #[test]
    fn a_missing_path_is_rejected() {
        let missing = std::env::temp_dir().join("wlwl_ext_definitely_missing_98765.dll");
        let err = check_exists(&missing).expect_err("missing must be rejected");
        assert!(matches!(err, ExtLoadError::Missing(_)), "got {err:?}");
        assert!(check_exists(&self_source()).is_ok());
    }

    /// T4:摘要不匹配被拒;匹配则通过(并回传实际摘要)。
    #[test]
    fn a_sha256_mismatch_is_rejected_and_a_match_passes() {
        let path = self_source();
        let wrong = "0".repeat(64);
        let err = check_sha256(&path, Some(&wrong)).expect_err("wrong hash must be rejected");
        assert!(
            matches!(err, ExtLoadError::Sha256Mismatch { .. }),
            "got {err:?}"
        );
        let actual = check_sha256(&path, None).expect("hash without expectation");
        assert_eq!(actual.len(), 64);
        assert!(check_sha256(&path, Some(&actual)).is_ok());
        // 大写也认(用户多半是从 `sha256sum` 抄的)。
        assert!(check_sha256(&path, Some(&actual.to_ascii_uppercase())).is_ok());
    }

    /// T5:命名空间路径的合法性与「不占用既有命名空间」。
    ///
    /// ⚠ **反向守卫**:去掉「`std.*` 影子拒绝」⇒ 本用例变红。
    #[test]
    fn a_namespace_may_not_shadow_std_or_an_existing_namespace() {
        let reserved = std_reserved_namespaces();
        assert!(
            reserved.iter().any(|n| n == "io"),
            "std 的短名清单里必须有 io(否则这条用例测的是别的): {reserved:?}"
        );
        // std 的短名 ⇒ 拒。
        let err =
            check_namespace_allowed("io", &reserved).expect_err("std shadow must be rejected");
        assert!(
            matches!(err, ExtLoadError::NamespaceReserved { .. }),
            "got {err:?}"
        );
        // 调用方给的「既有命名空间」⇒ 拒。
        let err = check_namespace_allowed("mine", &["mine".to_string()])
            .expect_err("an existing namespace must not be taken over");
        assert!(
            matches!(err, ExtLoadError::NamespaceReserved { .. }),
            "got {err:?}"
        );
        // 路径本身不合法 ⇒ 拒(且**不是**「占用」那一种理由)。
        for bad in ["", "a:b", "wlwl:std.io", "wlwlfoo", "./x", "/x"] {
            let err = check_namespace_allowed(bad, &reserved)
                .expect_err("an invalid namespace path must be rejected");
            assert!(
                matches!(err, ExtLoadError::NamespacePathInvalid { .. }),
                "namespace path {bad:?} must be rejected as invalid, got {err:?}"
            );
        }
        // 正常名字 ⇒ 过。
        assert!(check_namespace_allowed("tls", &reserved).is_ok());
    }

    /// T6:告警那一行必须说清「加载 ≠ 沙箱」。
    #[test]
    fn the_warning_line_states_the_privileges_handed_over() {
        let warning = trust_warning(Path::new("/tmp/example.dll"));
        assert!(warning.contains("NOT a sandbox"), "{warning}");
        assert!(warning.contains("FULL PROCESS PRIVILEGES"), "{warning}");
        assert!(warning.contains("/tmp/example.dll"), "{warning}");
    }
}
