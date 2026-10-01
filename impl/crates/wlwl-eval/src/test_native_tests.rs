//! [v0.11 M3-1] `wlwl:std.test` 成员面的锁。
//!
//! 上一版这里叫 `names_match_catalog`,断言写成
//! `NAMES == collection::NAMES.chunks(2).next().map(|_| NAMES)` —— 右边
//! 恒等于 `NAMES`,所以它**永远绿**,一次也没锁住任何东西。R2 collection
//! 实现随 M3-1 删除后这个恒等式连编译都过不了,正好把它拆掉换成真锁。

#[cfg(test)]
mod tests {
    use wlwl_std::test_native::{NAMES, SPEC};

    /// `NAMES` 常量与 `SPEC` 绑定表必须逐位同序 —— 两份名单在同一个文件里
    /// 写两遍,少一处同步就是 `IMPORT` 少绑一个名字。
    #[test]
    fn names_and_spec_are_in_lockstep() {
        assert_eq!(
            SPEC.functions.len(),
            NAMES.len(),
            "SPEC has {} members but NAMES lists {}",
            SPEC.functions.len(),
            NAMES.len()
        );
        for (i, (name, _)) in SPEC.functions.iter().enumerate() {
            assert_eq!(*name, NAMES[i], "member #{i}: SPEC order must match NAMES");
        }
    }

    /// 成员面本身:标准库规范 §8 的 6 个成员,不多不少。**顺序**按 §8 表格
    /// 顺序,不是按字母 —— 附录 A 镜像跟随它。
    ///
    /// 这份手写清单是「应然」侧,所以配一条反向守卫
    /// (`member_set_is_not_trivially_satisfied`):上面那条锁的是 SPEC 与
    /// NAMES 的**自洽**,若两份一起被改坏照样全绿;这里补上与规范文本
    /// 的外部对照由 `tests/collection_contract.rs` 承担(M3-3 之后 `std.test`
    /// 的规范对照会一并搬过去)。
    #[test]
    fn member_set_matches_the_spec_section_8_table() {
        assert_eq!(
            NAMES,
            &[
                "TEST",
                "ASSERT",
                "ASSERT_EQ",
                "ASSERT_NEQ",
                "EXPECT_ERR",
                "RUN_TESTS"
            ]
        );
    }

    /// 反向守卫:上一条若因为某种原因退化成比较空集合,必须转红。
    #[test]
    fn member_set_is_not_trivially_satisfied() {
        assert_eq!(NAMES.len(), 6, "the §8 member count is 6, not 0");
        let mut seen = std::collections::HashSet::new();
        for n in NAMES {
            assert!(seen.insert(*n), "{n} listed twice");
        }
    }

    /// `wlwl:std.collection` 已经不是 R2 了,但两条 std 模块的成员名不得
    /// 互相撞名 —— `IMPORT` 的名字校验按模块各自的名单走,一旦撞上,
    /// 错误信息会指向另一个模块。
    #[test]
    fn no_member_name_collides_with_the_r1_collection() {
        let collection: Vec<String> = wlwl_std::LANG_SOURCES
            .iter()
            .find(|s| s.path == "wlwl:std.collection")
            .map(|s| wlwl_std::lang_exports(s.source))
            .unwrap_or_default();
        assert!(
            !collection.is_empty(),
            "collection must be an R1 module with 17 exports; found none"
        );
        for n in NAMES {
            assert!(
                !collection.iter().any(|c| c == n),
                "std.test exports `{n}`, which std.collection also exports"
            );
        }
    }
}
