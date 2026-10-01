//! WLWL standard library (v0.11 §10) — stdlib foundation, direct-value
//! boundary (ADR-0022).
//!
//! 自 M2 起本 crate 与 eval 之间**不再有 serde_json 转换边界**:原生函数
//! 直接收发真 [`Value`](含闭包),回调经 [`StdHost`] 注入,挂起
//! ([`Outcome::signal`] 携带 `Signal::Yield`)原样穿透。依赖方向单向:
//! `wlwl-eval → wlwl-value ← wlwl-std`。
//!
//! Modules exposed (dual-track, ADR-0021):
//!
//! R2 原生层(Rust 绑定表):
//!   - `wlwl:std.io`     — `PRINT`, `INPUT` (§15.1)
//!   - `wlwl:std.fs`     — `READ_FILE`, `WRITE_FILE`, `EXISTS` (§15.3)
//!   - `wlwl:std.json`   — `PARSE`, `STRINGIFY` (§15.3 + E0070/E0071)
//!   - `wlwl:std.ai`     — ASK / ASK_STREAM stubs (§15.13, Phase 4 batch 3)
//!   - `wlwl:std.agent`  — agent-shaped helpers over `std.ai`
//!     (`TASK`, `TOOL`, `CALL_TOOL`, `MODEL`, `CONTEXT`; §15.14, Phase D3)
//!   - `wlwl:std.format` — `FORMAT` + the shared template grammar (§15.8 / §10.6, Phase B5)
//!   - `wlwl:std.test`   — 进程内测试框架内核(`TEST`/`ASSERT` 族/
//!     `RUN_TESTS`;注册表在 [`StdCtx::tests`])
//!
//! R1 语言层(纯 wlwl,`include_str!` 嵌入,eval 侧求值并缓存):
//!   - `wlwl:std.collection` — 17 个高阶集合函数(M3-1 起纯 wlwl 终态)
//!   - `wlwl:std.str`    — string extensions (stdlib spec §6; M1 placeholder
//!     member `QUOTE`, full member set lands in M3)
//!   - `wlwl:std.math`   — math basics (stdlib spec §7; M1 placeholder member
//!     `ABS`; `SQRT`/`POW` will be R2 kernels behind the facade)
//!
//! This list is **locked by a test** against [`resolve`] — a std module
//! that is reachable but unlisted is a documentation bug, and the test
//! says so out loud.

pub mod agent;
pub mod ai;
pub(crate) mod compat;
pub mod format;
pub mod fs;
pub mod io;
pub mod json;
pub mod kernels;
pub mod test_native;

use wlwl_error::{WlwlError, WlwlResult};
use wlwl_value::type_name;

// [v0.11 M2 / ADR-0022] 调用契约单源在 wlwl-value;这里再导出保持
// `wlwl_std::StdCtx` / `StdFn` 等既有路径对 eval 与测试不变。
pub use wlwl_value::{Outcome, Signal, StdCtx, StdFn, StdHost, TestEntry, Value};

/// A std module: a stable path + a static list of (name, function)
/// pairs. The list is the contract with the eval side: the IMPORT
/// `names` field is checked against this list, and each requested
/// name is bound as a `Value::NativeFn`.
pub struct ModuleSpec {
    pub path: &'static str,
    pub functions: &'static [(&'static str, StdFn)],
}

/// R1 语言层(纯 wlwl)源码模块 —— 标准库底座 v0.11(ADR-0021)。
///
/// 源码经 `include_str!` 嵌入二进制(release 默认形态,运行时不读外部
/// 文件)。开发期可经 `--std-src <dir>` / 环境变量 `WLWL_STD_SRC` 指向
/// 源码目录覆盖加载;该通道**不得**改变任何成员的名字与语义(锁测试
/// `wlwl-cli/tests/stdlib_dual_track.rs` 守护)。
pub struct StdSource {
    pub path: &'static str,
    pub source: &'static str,
    /// 注入本模块门面的 R2 内核(标准库底座 v0.11 M3-0,ADR-0021 §层间规则)。
    ///
    /// eval 侧在求值本模块源码**之前**把它们绑进子求值器的局部环境;返回
    /// 给 `IMPORT` 方的模块 env 只收 `EXPORT` 名单,所以这些名字**不外泄**
    /// —— 门面仍是唯一对外契约层。取值见 [`kernels::KERNELS`]。
    ///
    /// 为什么需要这条通道:浮点指令(`SQRT` / `POW`)纯 wlwl 表达不出来;
    /// 诊断码表同理 —— wlwl 源码无法指定原生诊断码(实测 `PANIC` → `E0100`),
    /// 而 `E0038`(`RANGE` 步长为零)在 M3 之前的唯一发射点是被删掉的 R2
    /// `RANGE` 实现。详见 [`kernels`] 的模块文档。
    pub kernels: &'static [(&'static str, StdFn)],
}

/// std 命名空间的实现后端(单一清单的两轨,ADR-0021 §分发)。
#[derive(Clone, Copy)]
pub enum StdBackend {
    /// R2 原生层:Rust 绑定表。
    Native(&'static ModuleSpec),
    /// R1 语言层:嵌入的纯 wlwl 源码,由 eval 侧解析、求值并缓存模块值。
    Lang(&'static StdSource),
}

impl StdBackend {
    pub fn path(&self) -> &'static str {
        match self {
            StdBackend::Native(spec) => spec.path,
            StdBackend::Lang(src) => src.path,
        }
    }

    /// 导出绑定表。语言层模块的成员由 `.wll` 源码声明、eval 侧在加载
    /// 时绑定,这里返回空切片;名册经 [`lang_exports`] 提取(镜像生成
    /// 器与锁测试的对账口径)。
    pub fn functions(&self) -> &'static [(&'static str, StdFn)] {
        match self {
            StdBackend::Native(spec) => spec.functions,
            StdBackend::Lang(_) => &[],
        }
    }
}

/// R1 源码模块登记表(单一清单的语言层半边)。新模块:在此登记 +
/// 更新 crate 目录注释 + 通过守门测试。
pub static LANG_SOURCES: &[StdSource] = &[
    StdSource {
        path: "wlwl:std.collection",
        source: include_str!("../wl/std/collection.wll"),
        // 诊断发射器 + 值种类措辞;算法部分纯 wlwl(§5 是「非敏感库的
        // 默认归宿」R1,归属见 ADR-0021 判定准则 3)。
        kernels: &[
            ("_KIND", kernels::kernel_kind as StdFn),
            ("_DIAG_E0020", kernels::kernel_diag_e0020 as StdFn),
            ("_DIAG_E0022", kernels::kernel_diag_e0022 as StdFn),
            ("_DIAG_E0030", kernels::kernel_diag_e0030 as StdFn),
            ("_DIAG_E0038", kernels::kernel_diag_e0038 as StdFn),
        ],
    },
    StdSource {
        path: "wlwl:std.str",
        source: include_str!("../wl/std/str.wll"),
        kernels: &[],
    },
    StdSource {
        path: "wlwl:std.math",
        source: include_str!("../wl/std/math.wll"),
        // 混合模块(规范 §7):门面在 wlwl 侧,浮点内核在这里。
        kernels: &[
            ("_SQRT", kernels::kernel_sqrt as StdFn),
            ("_POW", kernels::kernel_pow as StdFn),
        ],
    },
];

/// 从 R1 源码提取导出名:扫描 `EXPORT([...])` 声明中的字符串字面量。
///
/// 这是镜像生成器(附录 A)与测试的对账口径;运行期加载不走这里 ——
/// eval 侧由 AST 的 `collect_exports` 提取(权威口径),两边不一致会
/// 被 `stdlib_appendix_a_sync` 锁测试抓出来。只对随本 crate 分发的
/// 受控格式负责:每条声明一行、双引号名字。
pub fn lang_exports(source: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in source.lines() {
        let t = line.trim();
        let Some(rest) = t.strip_prefix("EXPORT(") else {
            continue;
        };
        let (Some(a), Some(b)) = (rest.find('['), rest.rfind(']')) else {
            continue;
        };
        for name in rest[a + 1..b].split('"').skip(1).step_by(2) {
            if !name.is_empty() {
                out.push(name.to_string());
            }
        }
    }
    out
}

/// Resolve a `wlwl:std.X` path to its implementation backend. Returns
/// `None` for anything that doesn't match — the eval side will then
/// surface `E0040 module 'X' not found` (treating the namespace path
/// as a module name).
pub fn resolve(path: &str) -> Option<StdBackend> {
    if let Some(src) = LANG_SOURCES.iter().find(|s| s.path == path) {
        return Some(StdBackend::Lang(src));
    }
    let spec = match path {
        "wlwl:std.io" => &io::SPEC,
        "wlwl:std.fs" => &fs::SPEC,
        "wlwl:std.json" => &json::SPEC,
        "wlwl:std.ai" => &ai::SPEC,
        "wlwl:std.agent" => &agent::SPEC,
        "wlwl:std.format" => &format::SPEC,
        "wlwl:std.test" => &test_native::SPEC,
        _ => return None,
    };
    Some(StdBackend::Native(spec))
}

// ── 诊断助手(边界直通后错误直接构造为 WlwlError,定位取入口文件)──

/// 值的种类名(诊断措辞用)。
pub fn value_kind(v: &Value) -> &'static str {
    type_name(v)
}

/// 调用任意可调用值(闭包 / `NativeFn`)并取回 `Outcome::value`。
///
/// [v0.11 M3-1] 原住在 `collection.rs`,随该文件的 R2 实现一起删除后上移
/// 到这里 —— `std.test` 的 `RUN_TESTS` 调测试体走的是同一条路。挂起
/// (`Signal::Yield`)由更上层消费,这里只取值。
///
/// 曾经同处 `collection.rs` 的 `arity` / `type_err` / `not_callable` /
/// `short_circuit_err` **没有**上移:R1 门面把元数/类型/回调检查改由注入
/// kernel 发射(见 [`kernels`]),这些助手随 R2 实现一起消失,搬上来就是死代码。
pub(crate) fn call_callable(
    host: &mut dyn StdHost,
    fn_name: &str,
    callable: &Value,
    args: Vec<Value>,
) -> WlwlResult<Value> {
    let outcome = host.call(callable, args, fn_name)?;
    Ok(outcome.value)
}

// ── 直通边界包装(内部表示模块共用,ADR-0022 §4)──

/// 把「serde_json 内部表示」的旧式函数包成直通 StdFn:
/// Value →(values_to_json)→ 内部表示 →(f)→ json_to_value → Value。
/// 转换失败(闭包/NaN 等不可表示)与旧边界一致地报 E0030。
pub(crate) fn wrap(
    host: &mut dyn StdHost,
    _name: &str,
    f: fn(&mut StdCtx, Vec<serde_json::Value>) -> Result<serde_json::Value, compat::StdError>,
    args: Vec<Value>,
) -> Result<Outcome, WlwlError> {
    let jargs = match crate::json::values_to_json(&args) {
        Ok(v) => v,
        Err((code, msg)) => return Err(host.ctx().err(code, msg)),
    };
    match f(host.ctx(), jargs) {
        Ok(v) => Ok(Outcome::normal(crate::json::json_to_value(v))),
        Err(e) => Err(host.ctx().err(e.code, e.message)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn std_ctx_default_is_empty() {
        let ctx = StdCtx::default();
        assert!(ctx.argv.is_empty());
        assert!(ctx.env.is_empty());
        assert!(ctx.tests.is_empty());
    }

    #[test]
    fn std_ctx_from_process_sees_argv() {
        let ctx = StdCtx::from_process();
        assert!(!ctx.argv.is_empty());
    }

    // ---- resolve ----

    #[test]
    fn resolve_io() {
        let s = resolve("wlwl:std.io").expect("io resolves");
        assert_eq!(s.path(), "wlwl:std.io");
    }
    #[test]
    fn resolve_fs() {
        let s = resolve("wlwl:std.fs").expect("fs resolves");
        assert_eq!(s.path(), "wlwl:std.fs");
    }
    #[test]
    fn resolve_json() {
        let s = resolve("wlwl:std.json").expect("json resolves");
        assert_eq!(s.path(), "wlwl:std.json");
    }
    #[test]
    fn resolve_ai() {
        let s = resolve("wlwl:std.ai").expect("ai resolves");
        assert_eq!(s.path(), "wlwl:std.ai");
    }
    #[test]
    fn resolve_agent() {
        let s = resolve("wlwl:std.agent").expect("agent resolves");
        assert_eq!(s.path(), "wlwl:std.agent");
        let names: Vec<&str> = s.functions().iter().map(|(n, _)| *n).collect();
        assert_eq!(names, vec!["TASK", "TOOL", "CALL_TOOL", "MODEL", "CONTEXT"]);
    }

    #[test]
    fn resolve_format() {
        let s = resolve("wlwl:std.format").expect("format resolves");
        assert_eq!(s.path(), "wlwl:std.format");
        let names: Vec<&str> = s.functions().iter().map(|(n, _)| *n).collect();
        assert_eq!(names, vec!["FORMAT"]);
    }
    #[test]
    fn resolve_collection() {
        // [v0.11 M3-1] collection 以纯 wlwl 重写为终态(ADR-0021 判定准则
        // 3:非敏感库默认归宿 R1)。R2 原生实现已删,成员面改由嵌入源码的
        // `EXPORT` 声明;17 条的**内容**对拍由
        // `wlwl-eval/tests/collection_contract.rs` 对标准库规范 §5 逐条做,
        // 这里只锁「解析到 Lang 后端 + 成员数」。
        let s = resolve("wlwl:std.collection").expect("collection resolves");
        let StdBackend::Lang(src) = s else {
            panic!("collection must be R1 now, got a native backend")
        };
        let names = lang_exports(src.source);
        assert_eq!(names.len(), 17, "collection member set: {names:?}");
    }
    #[test]
    fn resolve_test() {
        // [v0.11 M2] std.test 内核迁入(6 成员),注册表走 StdCtx::tests。
        let s = resolve("wlwl:std.test").expect("test resolves");
        assert_eq!(s.path(), "wlwl:std.test");
        let names: Vec<&str> = s.functions().iter().map(|(n, _)| *n).collect();
        assert_eq!(
            names,
            vec![
                "TEST",
                "ASSERT",
                "ASSERT_EQ",
                "ASSERT_NEQ",
                "EXPECT_ERR",
                "RUN_TESTS"
            ]
        );
    }
    #[test]
    fn resolve_unknown_returns_none() {
        assert!(resolve("wlwl:std.unknown").is_none());
        assert!(resolve("wlwl:std.ioo").is_none());
        assert!(resolve("").is_none());
        assert!(resolve("std.io").is_none()); // missing namespace
    }

    // ---- lang_exports ----

    #[test]
    fn lang_exports_scan_handles_multi_member_lines() {
        let md = lang_exports("LET(A, 1);\nEXPORT([\"A\", \"B_2\"]);\n");
        assert_eq!(md, vec!["A".to_string(), "B_2".to_string()]);
        assert!(lang_exports("LET(A, 1);\n").is_empty());
    }

    // ---- 文档与命名约定的守门测试(C5′)----

    /// 每个 std 模块的 `SPEC`。R2 侧引用 SPEC 本体。
    const ALL_SPECS: &[&ModuleSpec] = &[
        &io::SPEC,
        &fs::SPEC,
        &json::SPEC,
        &ai::SPEC,
        &agent::SPEC,
        &format::SPEC,
        &test_native::SPEC,
    ];

    #[test]
    fn every_module_is_listed_in_the_crate_catalog() {
        let catalog = include_str!("lib.rs");
        for spec in ALL_SPECS {
            assert!(
                catalog.contains(spec.path),
                "`{}` is a std module but missing from the crate-level \
                 module catalog in lib.rs",
                spec.path
            );
        }
        for src in LANG_SOURCES {
            assert!(
                catalog.contains(src.path),
                "`{}` is an R1 std module but missing from the crate-level \
                 module catalog in lib.rs",
                src.path
            );
        }
    }

    /// R1 语言层守门:resolve 命中 Lang 后端、导出 UPPER_SNAKE 且有 LET 绑定。
    #[test]
    fn lang_sources_resolve_and_export_upper_snake() {
        for src in LANG_SOURCES {
            match resolve(src.path) {
                Some(StdBackend::Lang(s)) => assert_eq!(s.path, src.path),
                other => panic!(
                    "{} must resolve to the Lang backend, got {:?}",
                    src.path,
                    other.map(|b| b.path())
                ),
            }
            let exports = lang_exports(src.source);
            assert!(
                !exports.is_empty(),
                "{} must declare EXPORT members",
                src.path
            );
            let mut seen = std::collections::HashSet::new();
            for name in &exports {
                assert!(
                    name.chars()
                        .all(|c| c.is_ascii_uppercase() || c == '_' || c.is_ascii_digit()),
                    "{} exports `{name}`, which is not UPPER_SNAKE",
                    src.path
                );
                assert!(
                    seen.insert(name.as_str()),
                    "{} exports `{name}` twice",
                    src.path
                );
                assert!(
                    src.source.contains(&format!("LET({name},")),
                    "{} exports `{name}` but no `LET({name}, ...)` binding exists",
                    src.path
                );
            }
        }
    }

    /// 命名约定(C5′):`SPEC.path` 与 `resolve()` 的键一致,导出名
    /// 全是 UPPER_SNAKE,且没有重复。
    #[test]
    fn module_paths_and_export_names_follow_the_naming_convention() {
        for spec in ALL_SPECS {
            let resolved = resolve(spec.path)
                .unwrap_or_else(|| panic!("{} must resolve to a SPEC", spec.path));
            assert_eq!(
                resolved.path(),
                spec.path,
                "SPEC.path must match its resolve() key"
            );
            let mut seen = std::collections::HashSet::new();
            for (name, _) in spec.functions {
                assert!(
                    name.chars()
                        .all(|c| c.is_ascii_uppercase() || c == '_' || c.is_ascii_digit()),
                    "{} exports `{name}`, which is not UPPER_SNAKE",
                    spec.path
                );
                assert!(!name.is_empty(), "{} exports an empty name", spec.path);
                assert!(seen.insert(*name), "{} exports `{name}` twice", spec.path);
            }
        }
    }

    /// `src/` 下每个**命名空间**文件都必须在自己的文档头里写清自己的路径,
    /// 且**必须已登记进 `ALL_SPECS`**(含 test_native.rs 的改名)。
    ///
    /// 豁免名单(`NOT_A_NAMESPACE`)是两个**不是命名空间**的内部文件:
    /// `compat.rs` 是 serde_json 内部表示的兼容层,`kernels.rs` 是注入
    /// R1 门面的 R2 内核表(M3-0)—— 两者都不该有 `SPEC`,进了
    /// `ALL_SPECS` 反而会让 `resolve()` 认得一个规范里不存在的命名空间。
    /// 豁免是**显式列举**的:新增文件默认要被这条守卫拦住。
    #[test]
    fn every_module_file_documents_and_registers_its_own_path() {
        const NOT_A_NAMESPACE: &[&str] = &["lib.rs", "compat.rs", "kernels.rs"];
        let mut files: Vec<String> = std::fs::read_dir("src")
            .expect("unit tests run with the package root as cwd")
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| n.ends_with(".rs") && !NOT_A_NAMESPACE.contains(&n.as_str()))
            .collect();
        files.sort();
        assert_eq!(
            files.len(),
            ALL_SPECS.len(),
            "src/ has {} namespace file(s) but ALL_SPECS registers {}; \
             a new src/*.rs must either register a SPEC here or be added to \
             NOT_A_NAMESPACE with a reason",
            files.len(),
            ALL_SPECS.len()
        );
        for name in &files {
            let source =
                std::fs::read_to_string(format!("src/{name}")).expect("module source readable");
            let spec = ALL_SPECS
                .iter()
                .find(|s| source.contains(&format!("path: \"{}\"", s.path)))
                .unwrap_or_else(|| {
                    panic!("{name}: no registered SPEC.path matches — add its SPEC to ALL_SPECS")
                });
            assert!(
                source.contains(spec.path),
                "{name} must name its own module path `{}` in its doc header",
                spec.path
            );
        }
    }
}
