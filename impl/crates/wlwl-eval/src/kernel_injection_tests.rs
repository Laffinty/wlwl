//! [v0.11 M3-0 / ADR-0021 §层间规则] R1 门面的 R2 内核注入 —— 端到端证明。
//!
//! 这个文件在 lib.rs **文件尾**声明(同 `collection_tests` /
//! `test_native_tests`):`wlwl-error` 的实现面扫描按文件中首个 `#[cfg(test)]`
//! 截断,新增测试模块声明必须落在尾部,否则实现侧的 W 码引用会被扫漏。
//!
//! 覆盖三件事:
//!
//! 1. kernel **真的注进去了** —— R1 门面源码能调到它并拿到结果(走
//!    `--std-src` 覆盖轨的公共 API,顺带证明覆盖轨同样注入);
//! 2. kernel **不外泄** —— 门面 env 里没有它,`IMPORT` 也拿不到;
//! 3. M1 行为**零回归** —— 没有 kernel 的 `std.str` 与 M1 逐字节等价。

#[cfg(test)]
mod tests {
    use crate::{Evaluator, LoadedModule, ModuleLoader, Value};
    use std::path::{Path, PathBuf};
    use wlwl_error::ErrorCode;

    fn unique_dir(name: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!("wlwl_kernel_{name}_{nanos}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn run(base: &Path, std_src: Option<&Path>, src: &str) -> wlwl_error::WlwlResult<Value> {
        let ast = wlwl_parser::parse(src, "t.wll").expect("test program parses");
        let mut ev = Evaluator::new().with_base_dir(base.to_path_buf());
        if let Some(dir) = std_src {
            ev = ev.with_std_src(dir.to_path_buf());
        }
        ev.eval(&ast)
    }

    /// 直接问 loader 要 `wlwl:std.math` 的模块 env —— 「kernel 不外泄」
    /// 最直接的证据(不经过 `IMPORT` 那一层)。
    fn load_math_module() -> LoadedModule {
        let src = wlwl_std::LANG_SOURCES
            .iter()
            .find(|s| s.path == "wlwl:std.math")
            .expect("std.math is an R1 module");
        let loader = ModuleLoader::new(std::env::temp_dir());
        let mut loader = loader;
        loader
            .load_std_lang(src, "wlwl:std.math")
            .expect("std.math loads")
    }

    // ── 1. 注入真的发生 ──────────────────────────────────────────────

    #[test]
    fn injected_kernel_is_callable_from_the_r1_facade() {
        // 覆盖轨:同一个 `math.wll` 换成会调 kernel 的实现。kernel 注入
        // 发生在 Rust 侧,与源码来自嵌入还是磁盘无关 —— 覆盖轨能调到
        // `_SQRT`,正是 M1「覆盖不得改变导出面」与 M3-0 注入机制能共存
        // 的证明。
        let std_src = unique_dir("inject");
        std::fs::write(
            std_src.join("math.wll"),
            "LET(TENTH, FUN((x), /(_SQRT(x), 10)));\nEXPORT([\"TENTH\"]);\n",
        )
        .unwrap();
        let base = unique_dir("inject_main");

        let v = run(
            &base,
            Some(&std_src),
            "IMPORT(\"wlwl:std.math\", [\"TENTH\"]); TENTH(100)",
        )
        .expect("the facade must reach the injected kernel");
        assert_eq!(v, Value::Float(1.0), "SQRT(100)/10 must be 1.0");
    }

    #[test]
    fn injected_kernel_receives_real_arguments_and_diagnostics() {
        // 内核不只是「能调到」,还要真的按实参算:域违例走 §7 规定的
        // `ERR(["kind": "DomainError", ...])` **值**,不是原生诊断。
        // 用 `IS_ERR` / `ERR_PAYLOAD` 消费掉它 —— 逃到顶层的 ERR 会按
        // §8.2 变成 `E0102`,那是语言行为,不是内核行为。
        let std_src = unique_dir("domain");
        std::fs::write(
            std_src.join("math.wll"),
            "LET(R, FUN((x), _SQRT(x)));\nEXPORT([\"R\"]);\n",
        )
        .unwrap();
        let base = unique_dir("domain_main");

        let v = run(
            &base,
            Some(&std_src),
            "IMPORT(\"wlwl:std.math\", [\"R\"]); R(4)",
        )
        .expect("no diagnostic");
        assert_eq!(v, Value::Float(2.0));

        let v = run(
            &base,
            Some(&std_src),
            r#"IMPORT("wlwl:std.math", ["R"]);
               LET(r, R(-1));
               IF(IS_ERR(r), ERR_PAYLOAD(r), "NOT_AN_ERR")"#,
        )
        .expect("a domain violation is a value, not a diagnostic");
        let Value::Dict(entries) = v else {
            panic!("SQRT(-1) must yield a DICT payload, got {v:?}")
        };
        assert!(
            entries.iter().any(|(k, v)| {
                matches!((k, v), (Value::String(k), Value::String(v)) if k == "kind" && v == "DomainError")
            }),
            "stdlib spec §7 requires kind=DomainError, got {entries:?}"
        );
    }

    // ── 2. 注入面不外泄 ──────────────────────────────────────────────

    #[test]
    fn injected_kernel_never_appears_on_the_import_surface() {
        let m = load_math_module();
        for (name, _) in wlwl_std::LANG_SOURCES
            .iter()
            .find(|s| s.path == "wlwl:std.math")
            .expect("std.math is an R1 module")
            .kernels
        {
            assert!(
                m.env.get(name).is_none(),
                "kernel `{name}` leaked into the module env: {:?}",
                m.env.get(name)
            );
            assert!(
                !m.exports.contains(*name),
                "kernel `{name}` leaked into the EXPORT set: {:?}",
                m.exports
            );
        }
    }

    #[test]
    fn importing_a_kernel_by_name_is_rejected() {
        // 门面的 `EXPORT` 就是契约面;`IMPORT` 一个 kernel 名必须 E0023,
        // 而不是「碰巧能调到」。
        let base = unique_dir("leak");
        let err = run(&base, None, "IMPORT(\"wlwl:std.math\", [\"_SQRT\"]); NULL")
            .expect_err("kernels are not importable");
        assert_eq!(
            err.diagnostic().code,
            ErrorCode::E0023,
            "importing a private kernel must be an export-surface error: {:?}",
            err.diagnostic()
        );
    }

    // ── 3. M1 零回归 ────────────────────────────────────────────────

    #[test]
    fn a_module_without_float_kernels_is_unchanged() {
        // M1 零回归:`std.str` 的 `QUOTE` 行为与 M1 落地时逐字一致。
        // (这条原先还断言 `std.str` 的 kernel 表为空 —— M3-2 给它加了诊断
        // 发射器之后就不成立了。改成钉规范真正承诺的不变量:**纯 R1 的模块
        // 不得拿到浮点内核**。§6 说 std.str 是纯 wlwl 实现,一旦有人给它
        // 挂上 `_SQRT` / `_POW`,那就已经不是纯 R1 了。)
        let str_src = wlwl_std::LANG_SOURCES
            .iter()
            .find(|s| s.path == "wlwl:std.str")
            .expect("std.str is an R1 module");
        for (name, _) in str_src.kernels {
            assert!(
                !matches!(*name, "_SQRT" | "_POW"),
                "stdlib spec §6 declares std.str a pure R1 module, but it now \
                 borrows the float kernel `{name}`"
            );
        }
        let base = unique_dir("m1");
        let v = run(
            &base,
            None,
            "IMPORT(\"wlwl:std.str\", [\"QUOTE\"]); QUOTE(\"a\\\"b\")",
        )
        .expect("M1 std.str must still work");
        assert_eq!(v, Value::String("\"a\\\"b\"".into()));
    }

    #[test]
    fn the_math_module_actually_declares_the_float_kernels() {
        // 反向守卫:上面那些以 kernel 列表为输入的断言,在列表被清空时会退化成
        // 检查零件事。这条钉住标准库规范 §7 承诺的那两个。
        //
        // **刻意不钉整个列表**:那份清单是手写的「应然」侧,每加一个诊断发射
        // 器就得改它一次,而这类改动本身完全无害 —— 钉死只会制造无谓的
        // 红灯。规范承诺的是「SQRT/POW 走 R2 内核」,那就只钉这个。
        let math_src = wlwl_std::LANG_SOURCES
            .iter()
            .find(|s| s.path == "wlwl:std.math")
            .expect("std.math is an R1 module");
        let names: Vec<&str> = math_src.kernels.iter().map(|(n, _)| *n).collect();
        for required in ["_SQRT", "_POW"] {
            assert!(
                names.contains(&required),
                "stdlib spec §7 pins std.math to the `{required}` float kernel; \
                 declared: {names:?}"
            );
        }
        let table: std::collections::HashSet<&str> =
            wlwl_std::kernels::KERNELS.iter().map(|(n, _)| *n).collect();
        for n in &names {
            assert!(
                table.contains(n),
                "`{n}` is injected but missing from `kernels::KERNELS`"
            );
        }
    }
}
