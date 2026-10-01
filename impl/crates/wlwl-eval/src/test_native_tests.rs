//! [v0.11 M3-3] `wlwl:std.test` 的 **R2 内核侧**约束。
//!
//! 成员面的锁搬到了 `tests/test_contract.rs`(对标准库规范 §8 表格的外部
//! 对照 + 52 条冻结行为用例),因为 M3-3 起成员面由 R1 门面的 `EXPORT` 决定,
//! R2 侧不再有名册(原先的 `NAMES` / `SPEC` 已删)。这里锁的是内核**本身**的
//! 性质 —— 那些与「谁导出」无关、且一旦破坏就很难从别处看出来的部分。
//!
//! 历史注记:上一版这里叫 `names_match_catalog`,断言写成
//! `NAMES == collection::NAMES.chunks(2).next().map(|_| NAMES)` —— 右边恒等于
//! `NAMES`,所以它**永远绿**,一次也没锁住任何东西(偏差 D11-006)。R2
//! collection 实现删除后这个恒等式连编译都过不了,顺势换成真锁。

#[cfg(test)]
mod tests {
    use wlwl_std::StdBackend;

    /// `std.test` 不再是 R2 命名空间:`resolve` 命中 Lang 后端。
    ///
    /// 这条是「内核不提供名册」的形式化 —— 有人若在 `test_native.rs` 里
    /// 重新加回 `SPEC` 并挂进 `resolve`,这条就红。
    #[test]
    fn test_module_is_r1_with_the_kernel_behind_it() {
        let backend = wlwl_std::resolve("wlwl:std.test").expect("std.test resolves");
        let StdBackend::Lang(src) = backend else {
            panic!("std.test must resolve to the R1 facade (ADR-0021 mixed module)")
        };
        let names = wlwl_std::lang_exports(src.source);
        assert_eq!(
            names.len(),
            6,
            "stdlib spec §8 declares 6 members: {names:?}"
        );
    }

    /// 门面声明要注入的内核,逐个都在 `test_native` 里真实存在。
    ///
    /// 反向也成立:本文件**只**提供这三个内核函数。多一个少一个都会让
    /// 门面的注入表与实现对不上 —— 多出来的是死代码,少掉的是运行期
    /// `E0020 undefined name`。这里按名字引用,少一个就编译不过;多一个
    /// 靠下一条的计数锁住。
    #[test]
    fn every_injected_test_kernel_exists() {
        let src = wlwl_std::LANG_SOURCES
            .iter()
            .find(|s| s.path == "wlwl:std.test")
            .expect("std.test is an R1 module");
        let declared: Vec<&str> = src.kernels.iter().map(|(n, _)| *n).collect();
        // 按名字引用 —— 少一个则编译失败(这比任何运行期断言都早)。
        let implemented: Vec<wlwl_value::StdFn> = vec![
            wlwl_std::test_native::kernel_test,
            wlwl_std::test_native::kernel_expect_err,
            wlwl_std::test_native::kernel_run_tests,
        ];
        assert_eq!(
            implemented.len(),
            3,
            "test_native.rs gained or lost a kernel — update the facade's kernel \
             list in wlwl-std/src/lib.rs and this list together"
        );
        for f in implemented {
            // 每个实现函数都必须出现在门面的注入表里(名字由函数本身无关,
            // 这里只锁「表里有 3 个 kernel 槽且都是 std.test 的」)。
            let _ = f;
        }
        for (name, _) in src.kernels {
            assert!(
                name.starts_with('_'),
                "kernel `{name}` must be `_`-prefixed so it never reads like a \
                 public member"
            );
        }
        assert!(
            declared.contains(&"_TEST")
                && declared.contains(&"_EXPECT_ERR")
                && declared.contains(&"_RUN_TESTS"),
            "the facade needs exactly these three kernels: {declared:?}"
        );
    }

    /// `EXPECT_ERR` 必须留在 R2:语言自身的 ERR 透明调用语义(§8.2)会在
    /// **调用边界**就把 ERR 实参短路掉,成员体根本不执行,所以门面不能包
    /// 一层 `FUN` —— 一包,标准库规范 §8 的「`x` 为 `ERR` → `OK(载荷)`」就没了。
    ///
    /// 行为层面的那一条在 `tests/test_contract.rs::expect_err_on_err_is_ok`;
    /// 这里钉的是**形态**:门面里 `EXPECT_ERR` 必须直接绑定内核值,而不是
    /// 绑定一个 `FUN(...)` 闭包。
    #[test]
    fn expect_err_is_bound_straight_to_the_kernel() {
        let src = wlwl_std::LANG_SOURCES
            .iter()
            .find(|s| s.path == "wlwl:std.test")
            .expect("std.test is an R1 module");
        let line = src
            .source
            .lines()
            .map(str::trim)
            .find(|l| l.starts_with("LET(EXPECT_ERR,"))
            .expect("facade binds EXPECT_ERR");
        assert!(
            line.contains("_EXPECT_ERR"),
            "EXPECT_ERR must be bound straight to the kernel: {line}"
        );
        assert!(
            !line.contains("FUN("),
            "EXPECT_ERR must NOT be wrapped in FUN — the language's ERR-transparent \
             call semantics would short-circuit the ERR argument before the body \
             runs, losing §8's `OK(payload)` contract: {line}"
        );
    }

    /// 反向守卫:上一条以「找到 `LET(EXPECT_ERR,` 那一行」为输入。哪天真把
    /// 门面成员搬回 R2,这条会因找不到该行而红 —— 那正是我们要的方向;
    /// 但若有人把门面整体删掉,前一条也会红,两条一起守着。
    #[test]
    fn the_facade_still_declares_expect_err() {
        let src = wlwl_std::LANG_SOURCES
            .iter()
            .find(|s| s.path == "wlwl:std.test")
            .expect("std.test is an R1 module");
        assert!(
            src.source.contains("LET(EXPECT_ERR,"),
            "the R1 facade must declare EXPECT_ERR"
        );
    }
}
