//! [W-21] 宿主侧原生动态库扩展加载器 —— **契约测试**。
//!
//! # 这个文件钉什么
//!
//! | 用例 | 钉的是 |
//! |---|---|
//! | `t1_*` | 正常加载 ⇒ 命名空间可 `IMPORT`、成员可调用 |
//! | `t2_*` | 宿主**绝不**自动发现扩展(项目目录里放着 DLL 也不算数) |
//! | `t3_*` | 相对路径被拒(T3) |
//! | `t4_*` | ABI 版本不匹配被拒 / SHA-256 不匹配被拒 |
//! | `t5_*` | `std.*` 影子被拒(T5) |
//! | `t6_*` | 未加载时该命名空间**不可见**(既有 `E0040`) |
//! | `t7_*` | **语言面没有 LOAD 成员**(脚本触发不了代码加载) |
//!
//! # 「真的加载」这件事怎么做到
//!
//! ⚠ 本文件**不**把二进制塞进仓里(那会立刻变成二进制制品的维护负担),
//! 而是在测试运行时用 `rustc` 把一份**最小扩展源码**编成 cdylib
//! (`--crate-type cdylib`)⇒ 需要 `rustc` 在 PATH 上(跑 `cargo test`
//! 的环境里本来就有)。
//!
//! 临时产物落在 `std::env::temp_dir()` ⇒ **仓内不留任何临时文件**。
//!
//! # 这份最小扩展长什么样
//!
//! 它按 `impl/crates/wlwl-ext/include/wlwl_ext.h` 的契约实现:
//! - 命名空间 `"tls"`;
//! - 成员 `hello`(0..1 个实参)、`fail`(恒失败)、`need2`(恰好 2 个实参);
//! - `hello` 把**实参 JSON 的字节数**回传 ⇒ 顺带证明实参确实编成了 JSON。
//!
//! ⚠ 「Rust 里带裸指针的 `static` 不是 `Sync`」⇒ 用 `#[repr(transparent)]`
//! 的 `SyncCell` 包一层(地址不变 ⇒ 导出的符号仍指向结构体本身)。

mod common;

use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::OnceLock;

use wlwl_eval::Evaluator;
use wlwl_ext::{ExtLoadError, ExtLoadRequest, ExtRegistry};

// ── 最小扩展的源码 ─────────────────────────────────────────────────────

/// 扩展源码模板。
///
/// ⚠ 占位符用 `@...@` 而不是 `{}`/`format!` —— 这段源码里**到处**是
/// 花括号(结构体字面量),用 `format!` 得处处转义 ⇒ 一个漏掉就是编译错。
const EXT_SOURCE: &str = r#"
use std::ffi::{c_char, c_void};

#[repr(C)]
pub struct Host {
    pub alloc: Option<unsafe extern "system" fn(u32) -> *mut c_void>,
    pub free: Option<unsafe extern "system" fn(*mut c_void)>,
    pub report: Option<unsafe extern "system" fn(*mut c_void, *const c_char, *const c_char)>,
    pub await_ready: Option<unsafe extern "system" fn(*mut c_void, u32, u32) -> i32>,
}

#[repr(C)]
pub struct Ctx {
    pub host: *const Host,
    pub member: *const c_char,
}

#[repr(C)]
pub struct Member {
    pub name: *const c_char,
    pub arity_min: u32,
    pub arity_max: u32,
    pub call: Option<unsafe extern "system" fn(*mut Ctx, *const c_char, *mut *mut c_char) -> i32>,
}

#[repr(C)]
pub struct Ns {
    pub path: *const c_char,
    pub member_count: u32,
    pub members: *const Member,
}

#[repr(C)]
pub struct V1 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub host_version: *const c_char,
    pub ns_count: u32,
    pub namespaces: *const Ns,
    pub host: *mut Host,
}

/// 交一块**用宿主分配器**分配的内存(跨 CRT 的关键:不能用扩展自己的分配器)。
unsafe fn emit(ctx: *mut Ctx, out_json: *mut *mut c_char, body: &str) -> i32 {
    let Some(alloc) = (*ctx).host.as_ref().and_then(|h| h.alloc) else {
        return 1;
    };
    let bytes = body.as_bytes();
    let buf = alloc(bytes.len() as u32 + 1);
    if buf.is_null() {
        return 1;
    }
    std::ptr::copy_nonoverlapping(bytes.as_ptr(), buf as *mut u8, bytes.len());
    *((buf as *mut u8).add(bytes.len())) = 0;
    *out_json = buf as *mut c_char;
    0
}

unsafe extern "system" fn hello_call(
    ctx: *mut Ctx,
    args_json: *const c_char,
    out_json: *mut *mut c_char,
) -> i32 {
    let n = if args_json.is_null() {
        0usize
    } else {
        std::ffi::CStr::from_ptr(args_json).to_bytes().len()
    };
    let body = format!("{{\"value\": {{\"greeting\": \"hello from ext\", \"argc_bytes\": {n}}}}}");
    unsafe { emit(ctx, out_json, &body) }
}

unsafe extern "system" fn fail_call(
    ctx: *mut Ctx,
    _args_json: *const c_char,
    out_json: *mut *mut c_char,
) -> i32 {
    unsafe {
        emit(
            ctx,
            out_json,
            "{\"err\": {\"kind\": \"demo\", \"op\": \"fail\", \"reason\": \"always fails\"}}",
        )
    }
}

unsafe extern "system" fn need2_call(
    ctx: *mut Ctx,
    _args_json: *const c_char,
    out_json: *mut *mut c_char,
) -> i32 {
    unsafe { emit(ctx, out_json, "{\"value\": \"two\"}") }
}

/// 裸指针 ⇒ 不是 `Sync` ⇒ 不能直接当 `static`。
/// `#[repr(transparent)]` 保证**地址**与内层一致 ⇒ 导出的符号仍指向结构体。
#[repr(transparent)]
pub struct SyncCell<T>(std::cell::UnsafeCell<T>);
unsafe impl<T> Sync for SyncCell<T> {}

static MEMBERS: SyncCell<[Member; 3]> = SyncCell(std::cell::UnsafeCell::new([
    Member {
        name: c"hello".as_ptr(),
        arity_min: 0,
        arity_max: 1,
        call: Some(hello_call),
    },
    Member {
        name: c"fail".as_ptr(),
        arity_min: 0,
        arity_max: 0,
        call: Some(fail_call),
    },
    Member {
        name: c"need2".as_ptr(),
        arity_min: 2,
        arity_max: 2,
        call: Some(need2_call),
    },
]));

static NAMESPACES: SyncCell<[Ns; 1]> = SyncCell(std::cell::UnsafeCell::new([Ns {
    path: c"@NS_PATH@".as_ptr(),
    member_count: 3,
    members: &MEMBERS as *const SyncCell<[Member; 3]> as *const [Member; 3] as *const Member,
}]));

#[no_mangle]
pub static wlwl_ext_v1: SyncCell<V1> = SyncCell(std::cell::UnsafeCell::new(V1 {
    abi_version: @ABI_VERSION@,
    struct_size: std::mem::size_of::<V1>() as u32,
    host_version: c"wlwl-ext-contract 1.0".as_ptr(),
    ns_count: 1,
    namespaces: &NAMESPACES as *const SyncCell<[Ns; 1]> as *const [Ns; 1] as *const Ns,
    host: std::ptr::null_mut(),
}));
"#;

/// 要编的扩展变体。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// 正常扩展:命名空间 `tls`、ABI 版本 1。
    Normal,
    /// ABI 版本报 2 ⇒ 宿主必须拒绝加载。
    WrongAbi,
    /// 命名空间叫 `io`(std 的短名)⇒ 宿主必须拒绝加载(T5)。
    StdShadow,
}

impl Kind {
    fn source(self) -> String {
        EXT_SOURCE
            .replace(
                "@ABI_VERSION@",
                match self {
                    Kind::WrongAbi => "2",
                    _ => "1",
                },
            )
            .replace(
                "@NS_PATH@",
                match self {
                    Kind::StdShadow => "io",
                    _ => "tls",
                },
            )
    }

    fn file_stem(self) -> &'static str {
        match self {
            Kind::Normal => "wlwl_ext_normal",
            Kind::WrongAbi => "wlwl_ext_wrong_abi",
            Kind::StdShadow => "wlwl_ext_std_shadow",
        }
    }
}

/// 动态库的文件名(平台差异)。
fn dll_file_name(kind: Kind) -> String {
    #[cfg(windows)]
    {
        format!("{}.dll", kind.file_stem())
    }
    #[cfg(unix)]
    {
        format!("lib{}.so", kind.file_stem())
    }
}

/// 用 `rustc` 把最小扩展编成动态库,返回**绝对**路径。
fn build_extension(kind: Kind) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "wlwl_ext_contract_{}_{}",
        std::process::id(),
        kind.file_stem()
    ));
    std::fs::create_dir_all(&dir).expect("scratch dir for the test extension");
    let src = dir.join("ext.rs");
    std::fs::write(&src, kind.source()).expect("write the test extension source");
    let out = dir.join(dll_file_name(kind));
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".to_string());
    let status = std::process::Command::new(&rustc)
        .args(["--edition", "2021", "--crate-type", "cdylib", "-O"])
        .arg("-o")
        .arg(&out)
        .arg(&src)
        .status()
        .unwrap_or_else(|e| {
            panic!(
                "cannot run `{}` to build the test extension (needed by this contract): {e}",
                rustc
            )
        });
    assert!(
        status.success(),
        "the test extension ({kind:?}) must compile with `{rustc}`"
    );
    assert!(
        out.is_file(),
        "the test extension must exist: {}",
        out.display()
    );
    out
}

/// 各变体只编一次(测试是并行跑的 ⇒ 用 `OnceLock` 收口)。
fn extension(kind: Kind) -> PathBuf {
    static NORMAL: OnceLock<PathBuf> = OnceLock::new();
    static WRONG_ABI: OnceLock<PathBuf> = OnceLock::new();
    static STD_SHADOW: OnceLock<PathBuf> = OnceLock::new();
    match kind {
        Kind::Normal => NORMAL.get_or_init(|| build_extension(kind)).clone(),
        Kind::WrongAbi => WRONG_ABI.get_or_init(|| build_extension(kind)).clone(),
        Kind::StdShadow => STD_SHADOW.get_or_init(|| build_extension(kind)).clone(),
    }
}

// ── 跑 wlwl 源 ─────────────────────────────────────────────────────────

/// 跑一段源,回显示值 / `!CODE message`。
fn eval_in(base_dir: &Path, src: &str, registry: Option<Rc<ExtRegistry>>) -> String {
    let ast = wlwl_parser::parse(src, "t.wll")
        .unwrap_or_else(|e| panic!("case source must parse: {src}\n{e}"));
    let mut ev = Evaluator::new().with_base_dir(base_dir.to_path_buf());
    if let Some(registry) = registry {
        ev = ev.with_ext_registry(registry);
    }
    match ev.eval(&ast) {
        Ok(v) => v.display(),
        Err(e) => format!(
            "!{} {}",
            e.diagnostic().code.as_str(),
            e.diagnostic().message
        ),
    }
}

/// 跑一段源(临时工程目录 + 无扩展)。
fn out(src: &str) -> String {
    eval_in(&common::scratch("wlwl_ext_contract"), src, None)
}

/// 加载一个扩展,失败就把错误**原文**打出来(便于定位)。
fn load(path: &Path, sha256: Option<String>) -> Result<wlwl_ext::LoadedExtension, ExtLoadError> {
    let req = ExtLoadRequest {
        path: path.to_path_buf(),
        expected_sha256: sha256,
        reserved_namespaces: Vec::new(),
    };
    wlwl_ext::load_extension(&req)
}

// ── T1:正常加载 ────────────────────────────────────────────────────────

/// 正常加载 ⇒ 命名空间可 `IMPORT`、成员可调用、值能过 JSON 边界。
#[test]
fn t1_a_loaded_extension_becomes_an_importable_namespace() {
    let loaded = load(&extension(Kind::Normal), None).expect("a conforming extension must load");
    assert_eq!(loaded.abi_version, 1);
    assert!(loaded.registry.namespace("tls").is_some());
    assert!(loaded.registry.member("tls", "hello").is_some());

    let registry = loaded.registry.clone();
    let got = eval_in(
        &common::scratch("wlwl_ext_contract"),
        r#"IMPORT("tls", ["hello"]); hello("x");"#,
        Some(registry),
    );
    assert!(
        got.contains("hello from ext"),
        "the extension member must be callable and its value must cross the JSON boundary: {got}"
    );
    // `hello` 把**实参 JSON 的字节数**回传 ⇒ 实参确实编成了 JSON 数组。
    assert!(
        got.contains("argc_bytes"),
        "the extension must receive the encoded arguments: {got}"
    );
}

/// `ns:name` 形态(`IMPORT("tls:hello", …)`)同样能绑上。
#[test]
fn t1_the_ns_name_form_also_binds_an_extension_member() {
    let loaded = load(&extension(Kind::Normal), None).expect("load");
    let got = eval_in(
        &common::scratch("wlwl_ext_contract"),
        r#"IMPORT("tls:hello", ["hello"]); hello();"#,
        Some(loaded.registry.clone()),
    );
    assert!(
        got.contains("hello from ext"),
        "the `ns:name` import form must bind the extension member: {got}"
    );
}

/// 扩展返回失败 ⇒ 宿主把它变成 **ERR 值**(不是诊断)。
///
/// ⚠ 「ERR 值」与「诊断」是两件事:wlwl 里 `ERR` **是值**,要用 `IS_ERR` /
/// `ERR_PAYLOAD` 消费;没被消费的 `ERR` 逃到顶层会报既有的 `E0102`
/// (那是语言的既有规矩,与扩展无关)。
#[test]
fn t1_an_extension_failure_becomes_an_err_value() {
    let loaded = load(&extension(Kind::Normal), None).expect("load");
    let registry = loaded.registry.clone();
    let got = eval_in(
        &common::scratch("wlwl_ext_contract"),
        r#"IMPORT("tls", ["fail"]); IS_ERR(fail());"#,
        Some(registry.clone()),
    );
    assert_eq!(
        got, "TRUE",
        "an extension failure must surface as an ERR value (consumed by IS_ERR): {got}"
    );
    // ERR 的载荷带着扩展给的 kind / op / reason。
    let got = eval_in(
        &common::scratch("wlwl_ext_contract"),
        r#"IMPORT("tls", ["fail"]); ERR_PAYLOAD(fail());"#,
        Some(registry),
    );
    assert!(
        got.contains("always fails") && got.contains("demo"),
        "the ERR payload must carry the extension's kind / op / reason: {got}"
    );
}

/// 元数由**宿主先**校验 ⇒ 不合规报既有 `E0022`,扩展根本不会被调用。
#[test]
fn t1_an_arity_mismatch_is_reported_as_e0022() {
    let loaded = load(&extension(Kind::Normal), None).expect("load");
    let got = eval_in(
        &common::scratch("wlwl_ext_contract"),
        r#"IMPORT("tls", ["need2"]); need2(1)"#,
        Some(loaded.registry.clone()),
    );
    assert!(
        got.starts_with("!E0022"),
        "the host must check arity before calling the extension (existing code E0022): {got}"
    );
}

// ── T2:绝不自动发现 ────────────────────────────────────────────────────

/// **项目目录里放着扩展 DLL 也算数** —— 没显式授权就不可见。
///
/// ⚠ 这条钉的是「宿主**绝不**从项目目录 / `./ext/` 自动发现扩展」。
#[test]
fn t2_an_extension_lying_in_the_project_directory_is_not_discovered() {
    let dir = common::scratch("wlwl_ext_contract");
    let dll = dir.join(dll_file_name(Kind::Normal));
    std::fs::copy(extension(Kind::Normal), &dll).expect("copy the extension next to the program");
    assert!(dll.is_file());
    let got = eval_in(&dir, r#"IMPORT("tls", ["hello"]);"#, None);
    assert!(
        got.starts_with("!E0040"),
        "the host must never auto-discover an extension from the project directory: {got}"
    );
}

// ── T3:绝对路径 ────────────────────────────────────────────────────────

/// 相对路径被拒(T3):相对路径会让「加载哪个文件」取决于 CWD。
#[test]
fn t3_a_relative_path_is_rejected() {
    let err = load(Path::new("wlwl_ext.dll"), None).expect_err("a relative path must be rejected");
    assert!(
        matches!(err, ExtLoadError::NotAbsolute(_)),
        "a relative path must be rejected as NotAbsolute, got: {err:?}"
    );
}

/// 不存在的路径被拒(T3)。
#[test]
fn t3_a_missing_path_is_rejected() {
    let missing = std::env::temp_dir().join("wlwl_ext_definitely_missing_1234.dll");
    let err = load(&missing, None).expect_err("a missing path must be rejected");
    assert!(
        matches!(err, ExtLoadError::Missing(_)),
        "a missing path must be rejected as Missing, got: {err:?}"
    );
}

// ── T4:ABI 版本 / SHA-256 ──────────────────────────────────────────────

/// ABI 版本不匹配 ⇒ 宿主**拒绝加载**。
#[test]
fn t4_an_abi_version_mismatch_is_rejected() {
    let err =
        load(&extension(Kind::WrongAbi), None).expect_err("a wrong ABI version must be rejected");
    assert!(
        matches!(
            err,
            ExtLoadError::AbiVersionMismatch {
                found: 2,
                expected: 1
            }
        ),
        "a wrong ABI version must be rejected, got: {err:?}"
    );
}

/// `--ext-sha256` 给了就必须匹配;匹配时正常加载。
#[test]
fn t4_a_sha256_mismatch_is_rejected_and_a_match_is_accepted() {
    let path = extension(Kind::Normal);
    let wrong = "0".repeat(64);
    let err = load(&path, Some(wrong.clone())).expect_err("a wrong sha256 must be rejected");
    assert!(
        matches!(err, ExtLoadError::Sha256Mismatch { .. }),
        "a wrong sha256 must be rejected, got: {err:?}"
    );
    // 正确的摘要 ⇒ 正常加载。
    let actual = wlwl_ext::trust::sha256_file(&path).expect("hash the test extension");
    assert_eq!(actual.len(), 64);
    let loaded = load(&path, Some(actual.clone())).expect("a matching sha256 must load");
    assert_eq!(loaded.sha256, actual);
}

// ── T5:`std.*` 影子 ────────────────────────────────────────────────────

/// 扩展声明的命名空间占用了 std 的短名 ⇒ **拒绝加载**(T5)。
///
/// ⚠ 这条是**反向守卫**:去掉「`std.*` 影子拒绝」⇒ 本用例必红。
#[test]
fn t5_an_extension_may_not_shadow_a_std_namespace() {
    // 前提:被影子的名字确实是 std 的命名空间短名(否则本用例测的是别的)。
    let reserved = wlwl_ext::trust::std_reserved_namespaces();
    assert!(
        reserved.iter().any(|n| n == "io"),
        "the std namespace short names must include `io` (got {reserved:?})"
    );
    let err = load(&extension(Kind::StdShadow), None)
        .expect_err("an extension shadowing a std namespace must be rejected");
    assert!(
        matches!(err, ExtLoadError::NamespaceReserved { .. }),
        "shadowing a std namespace must be rejected as NamespaceReserved, got: {err:?}"
    );
}

// ── T6:未加载 ⇒ 不可见 ─────────────────────────────────────────────────

/// 没有加载扩展 ⇒ 该命名空间**不可见** ⇒ `IMPORT` 报既有 `E0040`。
#[test]
fn t6_an_unloaded_namespace_is_invisible() {
    let got = out(r#"IMPORT("tls", ["hello"]);"#);
    assert!(
        got.starts_with("!E0040"),
        "an extension namespace must not be visible unless it was loaded: {got}"
    );
}

/// 加载前的那行告警必须说清「这不是沙箱」(T6)。
#[test]
fn t6_the_load_warning_says_it_is_not_a_sandbox() {
    let warning = wlwl_ext::trust::trust_warning(Path::new("/tmp/example.dll"));
    assert!(
        warning.contains("NOT a sandbox"),
        "the warning must say the extension is not a sandbox: {warning}"
    );
    assert!(
        warning.contains("FULL PROCESS PRIVILEGES"),
        "the warning must say what is actually being handed over: {warning}"
    );
    assert!(
        warning.contains("/tmp/example.dll"),
        "the warning must name the library being loaded: {warning}"
    );
}

// ── T7:语言面没有 LOAD 成员 ────────────────────────────────────────────

/// **语言面绝不提供任何 LOAD 成员** ⇒ 脚本触发不了代码加载(T1/T7)。
#[test]
fn t7_the_language_surface_has_no_load_member() {
    let got = out(r#"PRINT(LOAD("evil.dll"));"#);
    assert!(
        got.starts_with("!E0020"),
        "the language surface must not have a LOAD member: {got}"
    );
    assert!(
        got.contains("LOAD"),
        "the diagnostic must name the missing member: {got}"
    );
}
