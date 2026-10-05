//! `std.collection` **归层回归表** —— v0.11.3 M5 / addendum-00 L0-A-4
//!
//! ## 这张表是什么
//!
//! L0-A-2 / L0-A-3a / L0-A-3b 把 **13 个成员**从 R1 归到了 R2。计划
//! §5 落点 4 要求一条「**归层前后可观察输出逐字节相同**」的差分测试,
//! 理由是「防『归层顺手改了语义』的唯一机械保障」。
//!
//! **但活的对拍在归层完成后就不存在了** —— R1 实现已被删掉,没法让两版同时
//! 跑。所以差分测试的「前」那一侧,只能是**在删码之前录下的输出**:归层当时
//! 逐条对拍过(R1 快照 vs R2 输出),把**R1 那一侧**冻进本文件,就得到一个永久
//! 可回归的等价性证据 —— 任何人日后改了这 13 个成员里的任何一个、而可观察
//! 输出变了,本表立刻变红。
//!
//! 这与 `collection_contract.rs` 的分工:
//!   * `collection_contract.rs` —— M3-1 交付的 **127 条**冻结契约,**逐条未动**
//!     (自追加批次开工的 `5e2f876` 起该文件字节级 `git diff` 为空);
//!   * 本文件 —— 只覆盖**被归层的那 13 个成员**,且期望值取自**归层前**的实测,
//!     专盯「归层有没有改语义」这**一件事**。
//!
//! ## 期望值的来源(不是实现输出,这一点很重要)
//!
//! 全部来自 **R1 实测**:每条都在删掉 R1 代码**之前**用 `wlwl run` 跑出来、
//! 逐字抄下来的。归层后的第一次对拍就是拿它们比 R2 输出(全绿),所以它们是
//! 「R1 当时就是这么做的」的事实,不是人手写的意图。
//!
//! ## 刻意保留的三处「不好看」的行为
//!
//! 本表有三条用例冻结的是**已知缺陷 / 反直觉口径**,它们在这里是为了
//! **让缺陷显形、让改动必须经过裁决**,不是为了赞美它们:
//!
//! 1. `sort_cannot_sort_booleans`(`D13-011`)—— `SORT` 排不了布尔,报
//!    `<cmp>: cannot compute less than for boolean and boolean`。**故意不修**:
//!    改成「布尔可比」是独立的语义裁决。
//! 2. `uniq_and_dedup_by_use_different_dedup_bases` —— `UNIQ` 按**值相等**
//!    去重、`DEDUP_BY` 按 **`STR` 渲染串**去重,故 `1` 与 `1.0` 在前者合并、
//!    在后者不合并。两者**不是同一族**。
//! 3. `key_by_keeps_first_position_takes_last_value` —— `KEY_BY` 同键后者覆盖
//!    值,但**位置留在首次**。
//!
//! ## 比对口径
//!
//! 与 `collection_contract.rs` 同款:**诊断码 + 消息逐字**,外加结果值的
//! `display()`。逐字比消息是有意的 —— 消息漂移必须在这里变红,由人决定改
//! 实现还是改表。

use std::path::PathBuf;
use wlwl_eval::Evaluator;
use wlwl_value::Value;

struct Case {
    name: &'static str,
    src: &'static str,
    expect: &'static str,
}

const CASES: &[Case] = &[
    // ═══════════════════════════════════════════════════════════════
    // L0-A-2 归层第一批:`MAP` / `FILTER` / `CHUNK` / `WINDOW`
    // ═══════════════════════════════════════════════════════════════
    Case { name: "a2_map_plain", src: "IMPORT(\"wlwl:std.collection\", [\"MAP\"]); MAP([1, 2, 3], FUN((x), *(x, 2)))", expect: "[2, 4, 6]" },
    Case { name: "a2_filter_plain", src: "IMPORT(\"wlwl:std.collection\", [\"FILTER\"]); FILTER([1, 2, 3, 4], FUN((x), ==(%(x, 2), 0)))", expect: "[2, 4]" },
    Case { name: "a2_chunk_plain", src: "IMPORT(\"wlwl:std.collection\", [\"CHUNK\"]); CHUNK([1, 2, 3, 4, 5], 2)", expect: "[[1, 2], [3, 4], [5]]" },
    Case { name: "a2_window_plain", src: "IMPORT(\"wlwl:std.collection\", [\"WINDOW\"]); WINDOW([1, 2, 3, 4, 5], 2)", expect: "[[1, 2], [2, 3], [3, 4], [4, 5]]" },
    Case { name: "a2_chunk_n_gt_len", src: "IMPORT(\"wlwl:std.collection\", [\"CHUNK\"]); CHUNK([1, 2, 3], 5)", expect: "[[1, 2, 3]]" },
    // CHUNK / WINDOW 在 `n > LEN(arr)` 时**方向相反**:前者一块、后者零块。
    Case { name: "a2_window_n_gt_len", src: "IMPORT(\"wlwl:std.collection\", [\"WINDOW\"]); WINDOW([1, 2, 3], 5)", expect: "[]" },
    Case { name: "a2_chunk_empty", src: "IMPORT(\"wlwl:std.collection\", [\"CHUNK\"]); CHUNK([], 2)", expect: "[]" },
    // 回调里 `YIELD` ⇒ R1 于第一个挂起点中止、累加器全丢、结果 NULL。
    // R2「透传信号 + 丢弃累加器」与它**行为等价**,这条钉住的就是等价性。
    Case {
        name: "a2_map_yielding_callback_is_null",
        src: "IMPORT(\"wlwl:std.collection\", [\"MAP\"]); SCOPE(FUN(() , LET(t, SPAWN(FUN(() , MAP([1, 2, 3], FUN((x), (YIELD(); *(x, 2))))))); AWAIT(t)))",
        expect: "NULL",
    },
    // 元数 / 实参形态 / 回调可调用性 / 尺寸四类诊断,措辞逐字冻结。
    // 注意 `callback is not callable` 的码是 **E0020** 不是 E0030(最容易写错的一处)。
    Case { name: "a2_map_arity", src: "IMPORT(\"wlwl:std.collection\", [\"MAP\"]); MAP([1])", expect: "!E0022 MAP: function expects 2 argument(s), got 1" },
    Case { name: "a2_map_not_array", src: "IMPORT(\"wlwl:std.collection\", [\"MAP\"]); MAP(1, FUN((x), x))", expect: "!E0030 MAP: expected array, got integer" },
    Case { name: "a2_map_not_callable", src: "IMPORT(\"wlwl:std.collection\", [\"MAP\"]); MAP([1], 1)", expect: "!E0020 MAP: callback is not callable (got integer)" },
    Case { name: "a2_filter_not_array", src: "IMPORT(\"wlwl:std.collection\", [\"FILTER\"]); FILTER(\"x\", FUN((x), TRUE))", expect: "!E0030 FILTER: expected array, got string" },
    Case { name: "a2_filter_not_callable", src: "IMPORT(\"wlwl:std.collection\", [\"FILTER\"]); FILTER([1], 5)", expect: "!E0020 FILTER: callback is not callable (got integer)" },
    Case { name: "a2_filter_pred_not_boolean", src: "IMPORT(\"wlwl:std.collection\", [\"FILTER\"]); FILTER([1, 2], FUN((x), 42))", expect: "!E0030 FILTER: predicate must return BOOLEAN, got integer" },
    Case { name: "a2_chunk_not_array", src: "IMPORT(\"wlwl:std.collection\", [\"CHUNK\"]); CHUNK(1, 2)", expect: "!E0030 CHUNK: expected array, got integer" },
    Case { name: "a2_chunk_size_not_integer", src: "IMPORT(\"wlwl:std.collection\", [\"CHUNK\"]); CHUNK([1], \"a\")", expect: "!E0030 CHUNK: expected integer, got string" },
    Case { name: "a2_chunk_size_zero", src: "IMPORT(\"wlwl:std.collection\", [\"CHUNK\"]); CHUNK([1, 2], 0)", expect: "!E0030 CHUNK: size must be >= 1, got 0" },
    Case { name: "a2_window_not_array", src: "IMPORT(\"wlwl:std.collection\", [\"WINDOW\"]); WINDOW(1, 2)", expect: "!E0030 WINDOW: expected array, got integer" },
    Case { name: "a2_window_size_not_integer", src: "IMPORT(\"wlwl:std.collection\", [\"WINDOW\"]); WINDOW([1], \"a\")", expect: "!E0030 WINDOW: expected integer, got string" },
    Case { name: "a2_window_size_negative", src: "IMPORT(\"wlwl:std.collection\", [\"WINDOW\"]); WINDOW([1, 2], -1)", expect: "!E0030 WINDOW: size must be >= 1, got -1" },

    // ═══════════════════════════════════════════════════════════════
    // L0-A-3a 归层第二批(纯累加器):ENUMERATE / ZIP / UNIQ / FLAT / JOIN
    // ═══════════════════════════════════════════════════════════════
    Case { name: "a3_enumeration_plain", src: "IMPORT(\"wlwl:std.collection\", [\"ENUMERATE\"]); ENUMERATE([1, 2, 3, 4, 5])", expect: "[[0, 1], [1, 2], [2, 3], [3, 4], [4, 5]]" },
    Case { name: "a3_enumeration_empty", src: "IMPORT(\"wlwl:std.collection\", [\"ENUMERATE\"]); ENUMERATE([])", expect: "[]" },
    Case { name: "a3_enumeration_strings", src: "IMPORT(\"wlwl:std.collection\", [\"ENUMERATE\"]); ENUMERATE([\"x\", \"y\"])", expect: "[[0, x], [1, y]]" },
    Case { name: "a3_enumeration_nested_kept", src: "IMPORT(\"wlwl:std.collection\", [\"ENUMERATE\"]); ENUMERATE([[1, 2], [3]])", expect: "[[0, [1, 2]], [1, [3]]]" },
    Case { name: "a3_zip_pair", src: "IMPORT(\"wlwl:std.collection\", [\"ZIP\"]); ZIP([1, 2, 3], [10, 20, 30])", expect: "[[1, 10], [2, 20], [3, 30]]" },
    Case { name: "a3_zip_truncates_to_shorter", src: "IMPORT(\"wlwl:std.collection\", [\"ZIP\"]); ZIP([1, 2, 3], [10, 20])", expect: "[[1, 10], [2, 20]]" },
    Case { name: "a3_zip_three_columns", src: "IMPORT(\"wlwl:std.collection\", [\"ZIP\"]); ZIP([1, 2], [10, 20], [100, 200])", expect: "[[1, 10, 100], [2, 20, 200]]" },
    // 非数组实参按**单列**处理且**不报 E0030** —— R1 现状,照搬未「修正」。
    Case { name: "a3_zip_non_array_becomes_single_column", src: "IMPORT(\"wlwl:std.collection\", [\"ZIP\"]); ZIP([1, 2], 7)", expect: "[[1, 7]]" },
    Case { name: "a3_zip_empty", src: "IMPORT(\"wlwl:std.collection\", [\"ZIP\"]); ZIP([], [1, 2])", expect: "[]" },
    Case { name: "a3_uniq_basic", src: "IMPORT(\"wlwl:std.collection\", [\"UNIQ\"]); UNIQ([1, 1, 2, 3, 2, 1])", expect: "[1, 2, 3]" },
    // `UNIQ` 按**值相等**去重 ⇒ `1` 与 `1.0` 合并。
    Case { name: "a3_uniq_int_and_float_merge", src: "IMPORT(\"wlwl:std.collection\", [\"UNIQ\"]); UNIQ([1, 1.0, 2])", expect: "[1, 2]" },
    Case { name: "a3_uniq_strings", src: "IMPORT(\"wlwl:std.collection\", [\"UNIQ\"]); UNIQ([\"a\", \"a\", \"b\"])", expect: "[a, b]" },
    Case { name: "a3_uniq_nested", src: "IMPORT(\"wlwl:std.collection\", [\"UNIQ\"]); UNIQ([[1], [1], [2]])", expect: "[[1], [2]]" },
    Case { name: "a3_uniq_empty", src: "IMPORT(\"wlwl:std.collection\", [\"UNIQ\"]); UNIQ([])", expect: "[]" },
    // 布尔与整数**不**互等(`values_equal` 是类型严的),而 `FALSE` 与 `0` 也不互等。
    Case { name: "a3_uniq_bool_not_equal_int", src: "IMPORT(\"wlwl:std.collection\", [\"UNIQ\"]); UNIQ([TRUE, 1, 1.0, FALSE, 0])", expect: "[TRUE, 1, FALSE, 0]" },
    // `FLAT` 只展**一层**;内层数组原样保留。
    Case { name: "a3_flat_one_level_only", src: "IMPORT(\"wlwl:std.collection\", [\"FLAT\"]); FLAT([1, [2, 3], 4, [5, [6, 7]]])", expect: "[1, 2, 3, 4, 5, [6, 7]]" },
    Case { name: "a3_flat_empty", src: "IMPORT(\"wlwl:std.collection\", [\"FLAT\"]); FLAT([])", expect: "[]" },
    Case { name: "a3_flat_deep_stays_nested", src: "IMPORT(\"wlwl:std.collection\", [\"FLAT\"]); FLAT([[1, [2]], [[3]]])", expect: "[1, [2], [3]]" },
    Case { name: "a3_flat_all_scalars", src: "IMPORT(\"wlwl:std.collection\", [\"FLAT\"]); FLAT([1, 2, 3])", expect: "[1, 2, 3]" },
    Case { name: "a3_join_basic", src: "IMPORT(\"wlwl:std.collection\", [\"JOIN\"]); JOIN([1, 2, 3], \"-\")", expect: "1-2-3" },
    Case { name: "a3_join_empty_is_empty_string", src: "IMPORT(\"wlwl:std.collection\", [\"JOIN\"]); JOIN([], \"-\")", expect: "" },
    Case { name: "a3_join_single_has_no_sep", src: "IMPORT(\"wlwl:std.collection\", [\"JOIN\"]); JOIN([7], \"-\")", expect: "7" },
    // 元素经 `STR` 渲染后才连接 —— `NULL` 渲染成 `"NULL"`。
    Case { name: "a3_join_renders_via_str", src: "IMPORT(\"wlwl:std.collection\", [\"JOIN\"]); JOIN([1, \"a\", TRUE, NULL], \"|\")", expect: "1|a|TRUE|NULL" },
    Case { name: "a3_enumeration_arity", src: "IMPORT(\"wlwl:std.collection\", [\"ENUMERATE\"]); ENUMERATE([1], 2)", expect: "!E0022 ENUMERATE: function expects 1 argument(s), got 2" },
    Case { name: "a3_enumeration_not_array", src: "IMPORT(\"wlwl:std.collection\", [\"ENUMERATE\"]); ENUMERATE(1)", expect: "!E0030 ENUMERATE: expected array, got integer" },
    // `ZIP` 是 ≥1 元成员,报的是**下界 1**。
    Case { name: "a3_zip_arity_reports_lower_bound", src: "IMPORT(\"wlwl:std.collection\", [\"ZIP\"]); ZIP()", expect: "!E0022 ZIP: function expects 1 argument(s), got 0" },
    Case { name: "a3_uniq_arity", src: "IMPORT(\"wlwl:std.collection\", [\"UNIQ\"]); UNIQ()", expect: "!E0022 UNIQ: function expects 1 argument(s), got 0" },
    Case { name: "a3_uniq_not_array", src: "IMPORT(\"wlwl:std.collection\", [\"UNIQ\"]); UNIQ(\"x\")", expect: "!E0030 UNIQ: expected array, got string" },
    Case { name: "a3_flat_arity", src: "IMPORT(\"wlwl:std.collection\", [\"FLAT\"]); FLAT(1, 2)", expect: "!E0022 FLAT: function expects 1 argument(s), got 2" },
    Case { name: "a3_flat_not_array", src: "IMPORT(\"wlwl:std.collection\", [\"FLAT\"]); FLAT(5)", expect: "!E0030 FLAT: expected array, got integer" },
    Case { name: "a3_join_arity", src: "IMPORT(\"wlwl:std.collection\", [\"JOIN\"]); JOIN([1])", expect: "!E0022 JOIN: function expects 2 argument(s), got 1" },
    Case { name: "a3_join_not_array", src: "IMPORT(\"wlwl:std.collection\", [\"JOIN\"]); JOIN(1, \"-\")", expect: "!E0030 JOIN: expected array, got integer" },
    // `JOIN` 的分隔符诊断是 **bespoke 措辞**,不是 `_NEED` 那款 `expected <what>`。
    Case { name: "a3_join_separator_bespoke_wording", src: "IMPORT(\"wlwl:std.collection\", [\"JOIN\"]); JOIN([1, 2], 5)", expect: "!E0030 JOIN: expected string (separator), got integer" },

    // ═══════════════════════════════════════════════════════════════
    // L0-A-3b 归层第二批(下半):D 类 GROUP_BY / DEDUP_BY / KEY_BY
    // ═══════════════════════════════════════════════════════════════
    Case { name: "a3b_group_by_first_appearance_order", src: "IMPORT(\"wlwl:std.collection\", [\"GROUP_BY\"]); GROUP_BY([1, 2, 1, 3, 2], FUN((x), x))", expect: "[1: [1, 1], 2: [2, 2], 3: [3]]" },
    Case { name: "a3b_group_by_string_keys", src: "IMPORT(\"wlwl:std.collection\", [\"GROUP_BY\"]); GROUP_BY([\"aa\", \"ab\", \"b\", \"ba\"], FUN((s), SUB(s, 0, 1)))", expect: "[a: [aa, ab], b: [b, ba]]" },
    Case { name: "a3b_group_by_empty", src: "IMPORT(\"wlwl:std.collection\", [\"GROUP_BY\"]); GROUP_BY([], FUN((x), x))", expect: "[]" },
    // 键是 `STR(k)` **渲染串** ⇒ `TRUE` / `1` / `1.0` / `FALSE` 是**四个**组。
    Case { name: "a3b_group_by_key_is_rendered_string", src: "IMPORT(\"wlwl:std.collection\", [\"GROUP_BY\"]); GROUP_BY([TRUE, 1, 1.0, FALSE], FUN((x), x))", expect: "[TRUE: [TRUE], 1: [1], 1.0: [1.0], FALSE: [FALSE]]" },
    Case { name: "a3b_group_by_nested_keys", src: "IMPORT(\"wlwl:std.collection\", [\"GROUP_BY\"]); GROUP_BY([[1], [1], [2]], FUN((x), x))", expect: "[[1]: [[1], [1]], [2]: [[2]]]" },
    Case { name: "a3b_dedup_by_basic", src: "IMPORT(\"wlwl:std.collection\", [\"DEDUP_BY\"]); DEDUP_BY([1, 1, 2, 3, 2, 1], FUN((x), x))", expect: "[1, 2, 3]" },
    // ⚠️ 与上一条 `UNIQ` 那条**正好相反**:`DEDUP_BY` 按渲染串去重,`1` 与
    // `1.0` 的键是 `"1"` 与 `"1.0"` ⇒ **三个都留**。两者不是同一族。
    Case { name: "a3b_dedup_by_int_and_float_do_not_merge", src: "IMPORT(\"wlwl:std.collection\", [\"DEDUP_BY\"]); DEDUP_BY([1, 1.0, 2], FUN((x), x))", expect: "[1, 1.0, 2]" },
    Case { name: "a3b_dedup_by_string_keys", src: "IMPORT(\"wlwl:std.collection\", [\"DEDUP_BY\"]); DEDUP_BY([\"aa\", \"ab\", \"b\", \"ba\"], FUN((s), SUB(s, 0, 1)))", expect: "[aa, b]" },
    Case { name: "a3b_dedup_by_empty", src: "IMPORT(\"wlwl:std.collection\", [\"DEDUP_BY\"]); DEDUP_BY([], FUN((x), x))", expect: "[]" },
    Case { name: "a3b_dedup_by_nested", src: "IMPORT(\"wlwl:std.collection\", [\"DEDUP_BY\"]); DEDUP_BY([[1], [1], [2]], FUN((x), x))", expect: "[[1], [2]]" },
    Case { name: "a3b_key_by_later_overwrites_value", src: "IMPORT(\"wlwl:std.collection\", [\"KEY_BY\"]); KEY_BY([1, 2, 1, 3], FUN((x), x))", expect: "[1: 1, 2: 2, 3: 3]" },
    Case { name: "a3b_key_by_string_keys", src: "IMPORT(\"wlwl:std.collection\", [\"KEY_BY\"]); KEY_BY([\"a1\", \"b1\", \"a2\"], FUN((s), SUB(s, 0, 1)))", expect: "[a: a2, b: b1]" },
    Case { name: "a3b_key_by_empty", src: "IMPORT(\"wlwl:std.collection\", [\"KEY_BY\"]); KEY_BY([], FUN((x), x))", expect: "[]" },
    // ⚠️ **位置留首次、值取最后**:`z` 首次出现在下标 0,`z2` 是最后一次的值 ⇒
    // `z` 在**最前**但值是 `z2`。
    Case { name: "a3b_key_by_keeps_first_position_takes_last_value", src: "IMPORT(\"wlwl:std.collection\", [\"KEY_BY\"]); KEY_BY([\"z1\", \"y1\", \"z2\", \"x1\"], FUN((s), SUB(s, 0, 1)))", expect: "[z: z2, y: y1, x: x1]" },
    Case { name: "a3b_key_by_key_is_rendered_string", src: "IMPORT(\"wlwl:std.collection\", [\"KEY_BY\"]); KEY_BY([TRUE, 1, FALSE, 0], FUN((x), x))", expect: "[TRUE: TRUE, 1: 1, FALSE: FALSE, 0: 0]" },
    Case { name: "a3b_group_by_arity", src: "IMPORT(\"wlwl:std.collection\", [\"GROUP_BY\"]); GROUP_BY([1])", expect: "!E0022 GROUP_BY: function expects 2 argument(s), got 1" },
    Case { name: "a3b_group_by_not_array", src: "IMPORT(\"wlwl:std.collection\", [\"GROUP_BY\"]); GROUP_BY(1, FUN((x), x))", expect: "!E0030 GROUP_BY: expected array, got integer" },
    Case { name: "a3b_group_by_not_callable", src: "IMPORT(\"wlwl:std.collection\", [\"GROUP_BY\"]); GROUP_BY([1], 5)", expect: "!E0020 GROUP_BY: callback is not callable (got integer)" },
    Case { name: "a3b_dedup_by_arity", src: "IMPORT(\"wlwl:std.collection\", [\"DEDUP_BY\"]); DEDUP_BY([1])", expect: "!E0022 DEDUP_BY: function expects 2 argument(s), got 1" },
    Case { name: "a3b_dedup_by_not_array", src: "IMPORT(\"wlwl:std.collection\", [\"DEDUP_BY\"]); DEDUP_BY(1, FUN((x), x))", expect: "!E0030 DEDUP_BY: expected array, got integer" },
    Case { name: "a3b_dedup_by_not_callable", src: "IMPORT(\"wlwl:std.collection\", [\"DEDUP_BY\"]); DEDUP_BY([1], 5)", expect: "!E0020 DEDUP_BY: callback is not callable (got integer)" },
    Case { name: "a3b_key_by_arity", src: "IMPORT(\"wlwl:std.collection\", [\"KEY_BY\"]); KEY_BY([1])", expect: "!E0022 KEY_BY: function expects 2 argument(s), got 1" },
    Case { name: "a3b_key_by_not_array", src: "IMPORT(\"wlwl:std.collection\", [\"KEY_BY\"]); KEY_BY(1, FUN((x), x))", expect: "!E0030 KEY_BY: expected array, got integer" },
    Case { name: "a3b_key_by_not_callable", src: "IMPORT(\"wlwl:std.collection\", [\"KEY_BY\"]); KEY_BY([1], 5)", expect: "!E0020 KEY_BY: callback is not callable (got integer)" },
    // `key` 回调抛的 `ERR` 沿 8.2 传播**整个调用**,载荷逐字保留。
    Case { name: "a3b_group_by_key_err_propagates", src: "IMPORT(\"wlwl:std.collection\", [\"GROUP_BY\"]); IS_ERR(GROUP_BY([1], FUN((x), ERR(\"boom\"))))", expect: "TRUE" },
    Case { name: "a3b_group_by_key_err_payload_intact", src: "IMPORT(\"wlwl:std.collection\", [\"GROUP_BY\"]); ERR_PAYLOAD(GROUP_BY([1], FUN((x), ERR(\"boom\"))))", expect: "boom" },
    Case { name: "a3b_key_by_key_err_propagates", src: "IMPORT(\"wlwl:std.collection\", [\"KEY_BY\"]); IS_ERR(KEY_BY([1], FUN((x), ERR(\"boom\"))))", expect: "TRUE" },
    Case { name: "a3b_key_by_key_err_payload_intact", src: "IMPORT(\"wlwl:std.collection\", [\"KEY_BY\"]); ERR_PAYLOAD(KEY_BY([1], FUN((x), ERR(\"boom\"))))", expect: "boom" },

    // ═══════════════════════════════════════════════════════════════
    // 刻意冻结的「不好看」行为 —— 见文件头第 3 节
    // ═══════════════════════════════════════════════════════════════
    //
    // [D13-011] `SORT` 排不了布尔。`_SORT_LT` 判定两个布尔**可比**并调 `<(a,b)`,
    // 而语言的 `<` 只支持数值与字符串(`wlwl-eval` 的 `cmp_op` 对布尔直接 `Err`)。
    // **本批之前就存在**,127 条契约没有布尔排序用例所以一直没人碰到。
    // **故意不修**:改成「布尔可比」是一次独立的语义裁决,须单独立项。
    Case { name: "known_defect_sort_cannot_sort_booleans_d13_011", src: "IMPORT(\"wlwl:std.collection\", [\"SORT\"]); SORT([TRUE, FALSE])", expect: "!E0030 <cmp>: cannot compute less than for boolean and boolean" },
    // 给自定义比较器就绕开了上面那一支(`_SORT_LT` 的布尔判定只在缺省路径上)。
    Case { name: "known_defect_booleans_sortable_with_own_comparator", src: "IMPORT(\"wlwl:std.collection\", [\"SORT\"]); SORT([TRUE, FALSE], FUN((a, b), FALSE))", expect: "[TRUE, FALSE]" },
    // `UNIQ`(值相等)与 `DEDUP_BY`(`STR` 渲染串)是**两套去重口径**。
    Case { name: "uniq_and_dedup_by_use_different_dedup_bases", src: "IMPORT(\"wlwl:std.collection\", [\"UNIQ\", \"DEDUP_BY\"]); [UNIQ([1, 1.0, 2]), DEDUP_BY([1, 1.0, 2], FUN((x), x))]", expect: "[[1, 2], [1, 1.0, 2]]" },
];

fn scratch_dir() -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("wlwl_regression_{nanos}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

fn run(src: &str) -> Result<Value, String> {
    let ast = wlwl_parser::parse(src, "t.wll").expect("case source parses");
    let mut ev = Evaluator::new().with_base_dir(scratch_dir());
    match ev.eval(&ast) {
        Ok(v) => Ok(v),
        Err(e) => Err(format!(
            "!{} {}",
            e.diagnostic().code.as_str(),
            e.diagnostic().message
        )),
    }
}

fn actual(case: &Case) -> String {
    match run(case.src) {
        Ok(v) => v.display(),
        Err(s) => s,
    }
}

#[test]
fn re_layered_members_keep_their_pre_re_layering_behaviour() {
    let mut bad = Vec::new();
    for case in CASES {
        let got = actual(case);
        if case.expect != got {
            bad.push(format!(
                "{name}:\n      R1 冻结: {want}\n      现在:    {got}",
                name = case.name,
                want = case.expect
            ));
        }
    }
    assert!(
        bad.is_empty(),
        "{} / {} case(s) diverged from the pre-re-layering (R1) behaviour:\n  - {}\n\
         这说明有人改了这 13 个已归层成员的可观察语义 —— 归层的契约是不改语义,\
         要改须先走契约表评审并在此处同步冻结值。",
        bad.len(),
        CASES.len(),
        bad.join("\n  - ")
    );
}

/// 反向守卫:表本身不能塌缩,否则上面那条会退化成检查零件事。
/// 下限取**当前实际条数**(L0-A-2 / 3a / 3b 各 20 / 33 / 28,外加 3 条
/// 「已知缺陷与陷阱」),所以**误删任何一条都会在这里变红**,而不会静默地
/// 少守一个成员。
#[test]
fn the_regression_table_covers_every_re_layered_batch() {
    assert_eq!(
        CASES.len(),
        84,
        "regression table changed size (expected 84, got {}) — adding a case is fine \
         (bump this number and say why), silently dropping one is not",
        CASES.len()
    );
    for (batch, want) in [("a2_", 20), ("a3_", 33), ("a3b_", 28)] {
        let n = CASES.iter().filter(|c| c.name.starts_with(batch)).count();
        assert_eq!(n, want, "batch `{batch}` has {n} case(s), expected {want}");
    }
    // 13 个被归层的成员每个至少有一条用例钉住。
    for m in [
        "MAP",
        "FILTER",
        "CHUNK",
        "WINDOW",
        "ENUMERATE",
        "ZIP",
        "UNIQ",
        "FLAT",
        "JOIN",
        "GROUP_BY",
        "DEDUP_BY",
        "KEY_BY",
    ] {
        assert!(
            CASES.iter().any(|c| c.src.contains(&format!("\"{m}\""))),
            "no regression case mentions `{m}` — a re-layered member lost its pin"
        );
    }
    // 三条「刻意冻结的不好看行为」各在场 —— 它们是让缺陷显形的探针,
    // 删掉等于把缺陷藏回「没人注意」的状态。
    for probe in [
        "known_defect_sort_cannot_sort_booleans_d13_011",
        "known_defect_booleans_sortable_with_own_comparator",
        "uniq_and_dedup_by_use_different_dedup_bases",
    ] {
        assert!(
            CASES.iter().any(|c| c.name == probe),
            "the deliberate-trap probe `{probe}` is gone"
        );
    }
}

/// 引导器:打印全部用例的实际结果,用来复核或重冻结。跑法
/// `cargo test -p wlwl-eval --test collection_regression -- --nocapture`。
#[test]
fn dump_actuals() {
    for case in CASES {
        println!(
            "CASE|{name}|{expect}|{actual}",
            name = case.name,
            expect = case.expect,
            actual = actual(case)
        );
    }
}
