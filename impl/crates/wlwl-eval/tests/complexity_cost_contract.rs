//! [V-01] 语言核心册 §18.3 的**四项**「规范性」复杂度上界的**确定性工作量
//! 计数守卫**。
//!
//! §18.3 那张表的「档」列里,下面四项写的都是**规范性** ⇒ 实现**必须**在界内
//! 完成 ⇒ 一致性册 §6.2 的 **V-01**(发布闸门第 2 条)要求它们各自有可复算的证据。
//! 字典那四条已由 `dict_cost_contract.rs` 取证;**本文件补齐其余四项**。
//!
//! | 项 | 规范上界(§18.3) | 本文件的守卫 | 计量点(在被测结构**内部**) |
//! |---|---|---|---|
//! | 数组追加 | `PUSH(arr, x)` O(n) 最坏 | `push_copies_n_plus_one_elements` | `wlwl_value::copy_array_elements` / `copy_array_element` |
//! | 字符串拼接 | `CONCAT(s, t)` O(n+m) 最坏 | `concat_copies_n_plus_m_bytes` | `wlwl_value::concat_strings` |
//! | 深度相等 | `==(a, b)`(数组 / 字典) O(n) 最坏 | `equality_visits_array_elements_linearly` / `equality_dict_deep_comparison_is_linear` / `equality_dict_must_be_linear` | `wlwl_value::values_equal`(数组分支 / 字典分支) |
//! | `SORT` 比较次数 | `SORT(arr)` 的比较器调用次数 O(n²) 最坏 | `sort_comparator_calls_are_at_most_n_squared` | 闭包调用落点 + 值比较原语 |
//!
//! ## 测法(规范 §18.5.2 的三条硬要求,逐条照做)
//!
//! ① **不用墙钟比值** —— 同一份线性实现在不同机器上可差 3 倍,比值里混着缓存
//!    命中率与调度噪声 ⇒ 全部读数是**确定性工作量计数**:拷贝的元素个数、展开的
//!    字符个数、访问的元素个数、键比较次数、比较器调用次数。
//! ② **计量点收在被测结构内部** —— 计数点**不在** `PUSH(...)` / `==(a, b)` /
//!    `SORT(...)` 这些**调用点**上(在调用点计数,内部换成线性扫也看不出来),
//!    而在**拷贝原语 / 相等判定 / 比较调用**这些结构内部的原语上。
//! ③ **带下界断言** —— 每条守卫都断言计数 **> 0**(确认计量点真被走到),
//!    否则「计数恒为 0」也会绿。
//!
//! ## 期望值手写自规范
//!
//! 每条期望值都由**规范上界 + 语义**推出来,**不是**先跑一遍再抄实现输出:
//!
//! * `PUSH`:规范 §18.3.2 说「产出 n+1 个元素的新数组,拷贝量下界是 Ω(n)」⇒
//!   拷贝 n 个旧元素 + 追加 1 个 ⇒ **n + 1**。
//! * 字符串拼接:产出 n+m 个字符 ⇒ **n + m**。
//! * 数组深度相等:逐对比较 ⇒ 访问 **n** 对元素。
//! * 字典深度相等:每个条目在对方那边**按键定位**一次(哈希索引,规范 §18.3.1)
//!   ⇒ 1 次探测 + 每个候选 1 次核对 ⇒ 键互异时 **2n** ⇒ **线性**(⇒ 达标,见下)。
//! * `SORT`:每轮扫「剩余」,本轮首个不比较 ⇒ Σ(len(rest) − 1) = **n(n−1)/2**。
//!
//! ## ✅ 字典的深度相等已**达标**(X-15 收敛,2026-10-23)
//!
//! **原先是 Θ(n²)**:每个 `x` 的条目都要在 `y` 的 n 个条目里**线性**找一遍 ⇒
//! 键比较 n(n+1)/2 次 ⇒ 不满足 §18.3 的 O(n) 最坏(**规范性**)。收敛之后:
//!
//! * ① `equality_dict_deep_comparison_is_linear`(原名
//!   `equality_scans_dict_keys_quadratically`)锁住**达标后的读数**:键互异时
//!   键比较次数**恰好 2n**;
//! * ② `equality_dict_must_be_linear` 是**规范性上界本身**的守卫(≤ c·n,
//!   c = 4)—— **`#[ignore]` 已去掉**,现在是**绿**的。
//!
//! ⚠ **这两条期望为什么变过**(下一个人别再纠结):实现收敛前,① 钉的是**二次
//!   读数 n(n+1)/2**,注释里明写「这是**偏差**,不是达标」—— 那条期望钉的是
//!   **当时的实现偏差**,**不是规范**。规范侧**已裁决**(§18.3 的 O(n) 上界是
//!   **规范性**的)⇒ 期望改成规范要求的那一档,而**不是**留一条红门禁。
//!   ⚠ **判据本身没有放宽**:② 的 c·n(c = 4)一字未动,只去了 `#[ignore]`。
//!
//! ⚠ **相等口径没变**:仍是**集合口径**(规范 §2.4,**不看顺序**);域外键
//!   (`FLOAT` / 布尔)仍走**线性回退**,判定与收敛前**逐字一致**。值层那份
//!   契约(复杂度读数 + 重复键 / 域外键 / `INSTANCE` 键的语义)在
//!   `wlwl-value` 的 `tests/x15_dict_equality_cost.rs`。

mod common;

use std::path::PathBuf;
use wlwl_eval::Evaluator;
use wlwl_value::{reset_value_work_counters, value_work_counters, ValueWorkCounters};

fn scratch_dir() -> PathBuf {
    common::scratch("wlwl_complexitycost")
}

/// 跑一段源码,返回 `(末条语句的值, 被测结构内部的工作量)`。
///
/// ⚠ 末条语句的**值**也要拿到:工作量守卫必须同时证明「这次拷贝是**合法**的」
///   (规范 §19.1:旧值可观察 ⇒ 写必拷贝)—— 只看计数不看值,守卫就没有语义。
fn value_and_work_of(src: &str) -> (String, ValueWorkCounters) {
    let ast = wlwl_parser::parse(src, "t.wll").expect("src must parse");
    reset_value_work_counters();
    let mut ev = Evaluator::new().with_base_dir(scratch_dir());
    let v = ev.eval(&ast).expect("program must run");
    (v.display(), value_work_counters())
}

/// 造一个 n 元素的**数组字面量**源码(元素 0..n-1)。
fn int_array_literal(n: usize) -> String {
    let mut s = String::from("[");
    for i in 0..n {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&format!("{i}"));
    }
    s.push(']');
    s
}

/// 造一个 n 元素的**字符串数组字面量**源码(**降序** ⇒ 排序的最坏形态)。
fn string_array_literal(n: usize) -> String {
    let mut s = String::from("[");
    for i in 0..n {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&format!("\"s{:02}\"", n - 1 - i));
    }
    s.push(']');
    s
}

/// 造一个 n 条目的**字典字面量**源码(键 `"k0".."k{n-1}"`,值 = 序号)。
///
/// ⚠ 键在封闭域内(规范 §2.1:键域 = STRING / INTEGER / INSTANCE)⇒ 字典走哈希
///   索引那条路 ⇒ **构造**字典不产生键比较(计数点只在相等判定里)⇒ 读数干净。
fn dict_literal(n: usize) -> String {
    let mut s = String::from("[");
    for i in 0..n {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&format!("\"k{i}\": {i}"));
    }
    s.push(']');
    s
}

/// 造一段 n 个字符的**字符串字面量**源码。
fn string_literal(n: usize) -> String {
    let mut s = String::from("\"");
    s.push_str(&"a".repeat(n));
    s.push('"');
    s
}

/// `array_elements_copied` 的守卫读数:数组 0..n-1 再追加 `last` 之后的显示串。
fn expected_int_array_display(n: usize, last: i64) -> String {
    let mut parts: Vec<String> = (0..n).map(|i| i.to_string()).collect();
    parts.push(last.to_string());
    format!("[{}]", parts.join(", "))
}

/// ① **数组追加 · `PUSH(arr, x)` O(n) 最坏**(规范 §18.3,规范性)。
///
/// ## 判据(期望值**手写自规范**)
///
/// * **拷贝的元素个数 = n + 1** —— 规范 §18.3.2:「`PUSH` 返回**新**数组……
///   产出 n+1 个元素的新数组,拷贝量下界是 Ω(n)」⇒ 旧数组的 n 个元素被搬进新
///   数组,追加的元素再拷 1 个 ⇒ 精确地 n + 1 ⇒ **线性** ⇒ 满足 O(n) 最坏,
///   且**恰好**达到规范写的 Ω(n) 下界。
/// * **不随 n 超线性** —— n = 16 ⇒ 17;n = 64 ⇒ 65(比值 3.82 ≈ 4 = 64/16 ⇒ 线性)。
/// * **下界断言** —— 计数必须 **> 0**(否则「计量点从没被走到」也会绿)。
/// * **语义** —— 新数组必须是 n+1 个元素、原数组原样(规范 §3.3 / §10.4)。
///
/// ⚠ **这条守卫钉住的是「达标」**:一个「就地追加、不产出新数组」的实现(拷贝
///   恒 0)会让它红 —— 那正是规范 §18.3.2 明文禁止的形态。
#[test]
fn push_copies_n_plus_one_elements() {
    const SMALL: usize = 16;
    const LARGE: usize = 64;
    let small_src = format!("PUSH({}, 999)", int_array_literal(SMALL));
    let large_src = format!("PUSH({}, 999)", int_array_literal(LARGE));
    let (small_val, small) = value_and_work_of(&small_src);
    let (large_val, large) = value_and_work_of(&large_src);

    // 语义:新数组 = 旧数组 + 追加的元素。
    assert_eq!(
        small_val,
        expected_int_array_display(SMALL, 999),
        "PUSH must produce a new array with the appended element"
    );
    assert_eq!(
        large_val,
        expected_int_array_display(LARGE, 999),
        "PUSH must produce a new array with the appended element"
    );
    // 拷贝量 = n + 1(规范 §18.3.2 的 Ω(n) 恰好达到)。
    assert_eq!(
        small.array_elements_copied,
        (SMALL + 1) as u64,
        "one PUSH over {SMALL} elements must copy exactly n+1 elements (copied {})",
        small.array_elements_copied
    );
    assert_eq!(
        large.array_elements_copied,
        (LARGE + 1) as u64,
        "one PUSH over {LARGE} elements must copy exactly n+1 elements (copied {})",
        large.array_elements_copied
    );
    // 下界断言(§18.5.2 的纪律):计量点真被走到。
    assert!(
        small.array_elements_copied > 0,
        "the array copy path must be counted at all (copied = {})",
        small.array_elements_copied
    );
    // 线性:两档读数之比 ≈ 规模之比(不是二次)。
    assert!(
        large.array_elements_copied > small.array_elements_copied,
        "the counted work must grow with n (small={}, large={})",
        small.array_elements_copied,
        large.array_elements_copied
    );
    assert!(
        large.array_elements_copied <= (LARGE as u64) * (LARGE as u64),
        "PUSH must stay within the O(n) worst-case bound (copied {})",
        large.array_elements_copied
    );
}

/// ② **字符串拼接 · O(n+m) 最坏**(规范 §18.3 的 `CONCAT(s, t)`,规范性)。
///
/// ## 判据(期望值**手写自规范**)
///
/// * **拷贝的字节数 = n + m** —— 产出 n+m 个字符的字符串 ⇒ 拷贝 n+m 个字节 ⇒
///   **线性于 n+m** ⇒ 满足 O(n+m) 最坏。
/// * **是「和」不是「最大」** —— 第三组用**不对称**的两个操作数(16 与 64):
///   和 = 80,而「2·n」= 32、「2·m」= 128 ⇒ 只有「和」给出 80。
/// * **下界断言** —— 计数必须 **> 0**。
/// * **语义** —— 拼接结果必须是那 n+m 个字符(逐字)。
///
/// ⚠ 本仓的**字符串**拼接是 `+`(`CONCAT` 的字符串支返回**码点数组**,见下一条
///   守卫)。两者都测,因为规范 §18.3 的那一行写的是 `CONCAT(s, t)`。
#[test]
fn concat_copies_n_plus_m_bytes() {
    const SMALL: usize = 16;
    const LARGE: usize = 64;
    let small_src = format!(
        "LET(r, +({}, {})); r",
        string_literal(SMALL),
        string_literal(SMALL)
    );
    let large_src = format!(
        "LET(r, +({}, {})); r",
        string_literal(LARGE),
        string_literal(LARGE)
    );
    let asymmetric_src = format!(
        "LET(r, +({}, {})); r",
        string_literal(SMALL),
        string_literal(LARGE)
    );
    let (small_val, small) = value_and_work_of(&small_src);
    let (large_val, large) = value_and_work_of(&large_src);
    let (asymmetric_val, asymmetric) = value_and_work_of(&asymmetric_src);

    // 语义:拼接结果逐字是 n+m 个字符。
    assert_eq!(
        small_val,
        "a".repeat(SMALL + SMALL),
        "the concatenated string must be the two operands back to back"
    );
    assert_eq!(
        large_val,
        "a".repeat(LARGE + LARGE),
        "the concatenated string must be the two operands back to back"
    );
    assert_eq!(
        asymmetric_val,
        "a".repeat(SMALL + LARGE),
        "the concatenated string must be the two operands back to back"
    );
    // 拷贝量 = n + m。
    assert_eq!(
        small.string_bytes_copied,
        (SMALL + SMALL) as u64,
        "concatenating two {SMALL}-byte strings must copy exactly n+m bytes (copied {})",
        small.string_bytes_copied
    );
    assert_eq!(
        large.string_bytes_copied,
        (LARGE + LARGE) as u64,
        "concatenating two {LARGE}-byte strings must copy exactly n+m bytes (copied {})",
        large.string_bytes_copied
    );
    // 不对称的一档 ⇒ 读数是「和」,不是「最大」也不是「2·某一个」。
    assert_eq!(
        asymmetric.string_bytes_copied,
        (SMALL + LARGE) as u64,
        "the copied byte count must be n+m, not max(n,m) nor 2n (copied {})",
        asymmetric.string_bytes_copied
    );
    // 下界断言(§18.5.2 的纪律)。
    assert!(
        small.string_bytes_copied > 0,
        "the string copy path must be counted at all (copied = {})",
        small.string_bytes_copied
    );
    // 线性于 n+m:两档读数之比 ≈ 4(= 规模之比),不是二次。
    assert!(
        large.string_bytes_copied > small.string_bytes_copied,
        "the counted work must grow with n+m (small={}, large={})",
        small.string_bytes_copied,
        large.string_bytes_copied
    );
    assert!(
        large.string_bytes_copied <= ((LARGE + LARGE) * (LARGE + LARGE)) as u64,
        "the concatenation must stay within the O(n+m) worst-case bound (copied {})",
        large.string_bytes_copied
    );
}

/// ③ **字符串拼接 · `CONCAT` 的字符串支**(规范 §18.3 那一行字面写的是
/// `CONCAT(s, t)`)。
///
/// 本仓的 `CONCAT` 遇两个 STRING 会返回**码点数组**(语言核心册 §10.4 把
/// `CONCAT` 定义成「两数组连接」;字符串支是宽松路径)⇒ 那条支的代价是
/// **展开 n+m 个码点**。
///
/// ## 判据
///
/// * **展开的码点数 = n + m** ⇒ 线性于 n+m ⇒ 满足 O(n+m) 最坏。
/// * **下界断言** —— 计数必须 **> 0**。
/// * **语义** —— 结果是 n+m 个码点,逐个等于操作数的码点。
#[test]
fn concat_expands_n_plus_m_codepoints() {
    const SMALL: usize = 16;
    const LARGE: usize = 64;
    let small_src = format!(
        "LET(r, CONCAT({}, {})); r",
        string_literal(SMALL),
        string_literal(SMALL)
    );
    let large_src = format!(
        "LET(r, CONCAT({}, {})); r",
        string_literal(LARGE),
        string_literal(LARGE)
    );
    let (small_val, small) = value_and_work_of(&small_src);
    let (large_val, large) = value_and_work_of(&large_src);

    // 语义:结果是一个 n+m 个码点的数组(全是 'a' = 97)。
    assert_eq!(
        small_val,
        format!("[{}]", vec!["97"; SMALL + SMALL].join(", ")),
        "CONCAT over two strings must yield n+m codepoints"
    );
    assert_eq!(
        large_val,
        format!("[{}]", vec!["97"; LARGE + LARGE].join(", ")),
        "CONCAT over two strings must yield n+m codepoints"
    );
    // 展开量 = n + m。
    assert_eq!(
        small.string_codepoints_expanded,
        (SMALL + SMALL) as u64,
        "CONCAT over two {SMALL}-char strings must expand exactly n+m codepoints (expanded {})",
        small.string_codepoints_expanded
    );
    assert_eq!(
        large.string_codepoints_expanded,
        (LARGE + LARGE) as u64,
        "CONCAT over two {LARGE}-char strings must expand exactly n+m codepoints (expanded {})",
        large.string_codepoints_expanded
    );
    // 下界断言(§18.5.2 的纪律)。
    assert!(
        small.string_codepoints_expanded > 0,
        "the codepoint expansion must be counted at all (expanded = {})",
        small.string_codepoints_expanded
    );
    // 线性于 n+m。
    assert!(
        large.string_codepoints_expanded > small.string_codepoints_expanded,
        "the counted work must grow with n+m (small={}, large={})",
        small.string_codepoints_expanded,
        large.string_codepoints_expanded
    );
}

/// ④ **深度相等 · 数组 O(n) 最坏**(规范 §18.3 的 `==(a, b)`,规范性)。
///
/// ## 判据(期望值**手写自规范**)
///
/// * **访问的元素对数 = n** —— 深度相等逐对比较 ⇒ 访问 n 对 ⇒ **线性** ⇒ 满足
///   O(n) 最坏。
/// * **短路是真的短路** —— 首个元素就不等时只访问 **1** 对 ⇒ 计数点确实记的是
///   「**实际访问过**的对数」,不是「长度」。
/// * **下界断言** —— 计数必须 **> 0**。
/// * **语义** —— 相等的数组判 TRUE,不相等的判 FALSE。
#[test]
fn equality_visits_array_elements_linearly() {
    const SMALL: usize = 16;
    const LARGE: usize = 64;
    let small_src = format!(
        "==({}, {})",
        int_array_literal(SMALL),
        int_array_literal(SMALL)
    );
    let large_src = format!(
        "==({}, {})",
        int_array_literal(LARGE),
        int_array_literal(LARGE)
    );
    let (small_val, small) = value_and_work_of(&small_src);
    let (large_val, large) = value_and_work_of(&large_src);
    // 首个元素就不等 ⇒ 只访问 1 对。
    let mismatch_src = "==([0, 1], [9, 1])".to_string();
    let (mismatch_val, mismatch) = value_and_work_of(&mismatch_src);

    // 语义。
    assert_eq!(small_val, "TRUE", "equal arrays must compare equal");
    assert_eq!(large_val, "TRUE", "equal arrays must compare equal");
    assert_eq!(mismatch_val, "FALSE", "unequal arrays must compare unequal");
    // 访问量 = n(逐对比较 ⇒ 线性)。
    assert_eq!(
        small.equality_element_visits, SMALL as u64,
        "deep equality over {SMALL} elements must visit exactly n element pairs (visited {})",
        small.equality_element_visits
    );
    assert_eq!(
        large.equality_element_visits, LARGE as u64,
        "deep equality over {LARGE} elements must visit exactly n element pairs (visited {})",
        large.equality_element_visits
    );
    // 短路:首个就不等 ⇒ 只访问 1 对。
    assert_eq!(
        mismatch.equality_element_visits, 1,
        "an early mismatch must short-circuit the element scan (visited {})",
        mismatch.equality_element_visits
    );
    // 下界断言(§18.5.2 的纪律)。
    assert!(
        small.equality_element_visits > 0,
        "the equality path must be counted at all (visited = {})",
        small.equality_element_visits
    );
    // 线性:两档读数之比 = 规模之比(= 4),不是二次。
    assert!(
        large.equality_element_visits == small.equality_element_visits * (LARGE / SMALL) as u64,
        "the visited-pair count must grow linearly with n (small={}, large={})",
        small.equality_element_visits,
        large.equality_element_visits
    );
    assert!(
        large.equality_element_visits <= (LARGE as u64) * (LARGE as u64),
        "deep equality must stay within the O(n) worst-case bound (visited {})",
        large.equality_element_visits
    );
}

/// ⑤ **深度相等 · 字典**:✅ **O(n) 最坏**(规范 §18.3,**规范性**)。
///
/// 这条是**特征化守卫**:锁住**达标后**的读数 —— 字典的深度相等现在按**键**
/// 在对方的哈希索引里定位候选(规范 §18.3.1 的哈希索引),不再是「每个 `x` 的
/// 条目都在 `y` 的 n 个条目里线性找一遍」。
///
/// ## 判据(期望值**手写自规范 + 语义**)
///
/// * **键比较次数 = 2n** —— 每个 `x` 的条目:① **1 次哈希探测**(按键定位到 `y`
///   里的候选位置;与 `DictInner::lookup` 的哈希路径同口径:一次查找 = 1 次键
///   比较)② **1 次候选核对**(键按 §2.4 复核 + 值比较)⇒ 键互异时**恰好 2n**
///   ⇒ **线性** ⇒ 满足 §18.3 的 O(n) 最坏。
///   (⚠ 重复键时同一键有多个候选 ⇒ 核对次数可能多于 1 —— 那一形态由
///   `wlwl-value` 的 `tests/x15_dict_equality_cost.rs` 覆盖。)
/// * **两档读数之比 = 规模之比(= 4)** —— 线性,不是二次(= 16)。
/// * **下界断言** —— 计数必须 **> 0**。
/// * **语义** —— 相等的字典判 TRUE。
///
/// ⚠ **这条期望为什么变过**(别再纠结):实现收敛前,本条钉的是**二次读数**
///   n(n+1)/2,注释里明写「这是**偏差**,不是达标」—— 那钉的是**实现偏差**,
///   不是规范。规范侧**已裁决**(§18.3 的 O(n) 上界是**规范性**的)⇒ 期望改成
///   规范要求的那一档。**判据没有放宽**:下一条(⑥)的 c·n(c = 4)一字未动。
#[test]
fn equality_dict_deep_comparison_is_linear() {
    const SMALL: usize = 16;
    const LARGE: usize = 64;
    let small_src = format!("==({}, {})", dict_literal(SMALL), dict_literal(SMALL));
    let large_src = format!("==({}, {})", dict_literal(LARGE), dict_literal(LARGE));
    let (small_val, small) = value_and_work_of(&small_src);
    let (large_val, large) = value_and_work_of(&large_src);

    // 语义。
    assert_eq!(small_val, "TRUE", "equal dicts must compare equal");
    assert_eq!(large_val, "TRUE", "equal dicts must compare equal");
    // 键比较次数 = 2n(1 次探测 + 1 次候选核对,每个条目)。
    assert_eq!(
        small.equality_key_comparisons,
        2 * SMALL as u64,
        "dict deep equality over {SMALL} entries must cost exactly 2n key comparisons (scanned {})",
        small.equality_key_comparisons
    );
    assert_eq!(
        large.equality_key_comparisons,
        2 * LARGE as u64,
        "dict deep equality over {LARGE} entries must cost exactly 2n key comparisons (scanned {})",
        large.equality_key_comparisons
    );
    // 下界断言(§18.5.2 的纪律)。
    assert!(
        small.equality_key_comparisons > 0,
        "the dict equality path must be counted at all (scanned = {})",
        small.equality_key_comparisons
    );
    // 线性:两档读数之比 = 规模之比(= 4),不是二次(= 16)。
    assert!(
        large.equality_key_comparisons == small.equality_key_comparisons * (LARGE / SMALL) as u64,
        "the scanned-key count must grow linearly with n (small={}, large={})",
        small.equality_key_comparisons,
        large.equality_key_comparisons
    );
}

/// ⑥ **深度相等 · 字典的规范性上界本身**:✅ **已达标**(绿)。
///
/// 这条断言的就是规范 §18.3 那一行的**规范性上界**:字典的深度相等必须
/// **O(n) 最坏** ⇒ 键比较次数 ≤ c·n。
///
/// ⚠ **`#[ignore]` 已去掉**(X-15 收敛,2026-10-23):实现原先是 Θ(n²)(见上一条
///   守卫的旧读数 n(n+1)/2)⇒ 这条曾**必红**;现在收敛为哈希索引那条路 ⇒
///   读数落在 c·n 内。**判据本身一字未动**(c = 4 **不许**放宽)。
///
/// ⚠ 这条**不是**「改规范」或「改实现来对上」:规范 §18.3 那一行从头到尾都是
///   **规范性**的 ⇒ 实现收敛 + 去掉 `#[ignore]` 是唯一正确的消法。
#[test]
fn equality_dict_must_be_linear() {
    const SMALL: usize = 16;
    const LARGE: usize = 64;
    let small_src = format!("==({}, {})", dict_literal(SMALL), dict_literal(SMALL));
    let large_src = format!("==({}, {})", dict_literal(LARGE), dict_literal(LARGE));
    let (_, small) = value_and_work_of(&small_src);
    let (_, large) = value_and_work_of(&large_src);
    let c = 4u64;
    assert!(
        small.equality_key_comparisons <= c * SMALL as u64,
        "dict deep equality must stay within c·n key comparisons (n={SMALL}, scanned={}, c={c})",
        small.equality_key_comparisons
    );
    assert!(
        large.equality_key_comparisons <= c * LARGE as u64,
        "dict deep equality must stay within c·n key comparisons (n={LARGE}, scanned={}, c={c})",
        large.equality_key_comparisons
    );
    assert!(
        small.equality_key_comparisons > 0,
        "the dict equality path must be counted at all (scanned = {})",
        small.equality_key_comparisons
    );
}

/// ⑦ **`SORT` 的比较器调用次数 O(n²) 最坏**(规范 §18.3 的 `SORT` 行,规范性)。
///
/// ## 判据(期望值**手写自规范 + 语义**)
///
/// * **比较器调用次数 = n(n−1)/2** —— 排序每轮扫「剩余」,本轮的**首个**元素
///   没有前驱可比(不调比较器),其余每个都要比一次 ⇒ 第 i 轮比 len(rest) − 1 次
///   ⇒ Σ = n(n−1)/2。
/// * **规范性上界** —— O(n²) 最坏 ⇒ 读数 ≤ n²(c = 1)。
/// * **下界断言** —— 计数必须 **> 0**。
/// * **语义** —— 结果是升序排列的同一批元素。
///
/// ## 计量点(在被测结构**内部**)
///
/// * `comparator_calls` —— **闭包被真正调用**的次数(落在
///   `Evaluator::call_value_with_receiver` 的 `Value::Closure` 分支)。门面
///   `collection.wll` 的 `_SORT_LT` 里**唯一**的 `CALL(cmp, a, b)` 就是比较器
///   的调用点 ⇒ 在 `SORT` 路径上这个计数**就是**比较器调用次数;门面自己的内部
///   闭包(`_SORT_LT` 等)按名调用 ⇒ **不计入**。
///   ⚠ 这**不是**「`SORT` 被调用了几次」那种调用点计数:比较器**每被调用一次**
///   就多记一笔 ⇒ 排序内部换比较策略(比数变多/变少)立刻被看见。
/// * `string_comparisons` —— **值比较原语**里「字符串 vs 字符串」的比较次数。
///   比较器写成 `FUN((a, b), <(a, b))` ⇒ **每次比较器调用恰好一次值比较**
///   ⇒ 两个计数必须相等(互为交叉核对)。⚠ 只记字符串这一类 ⇒ 排序循环里那些
///   **整数**的循环控制比较(`<(i, LEN(rest))` 之类)不混进来。
#[test]
fn sort_comparator_calls_are_at_most_n_squared() {
    const SMALL: usize = 16;
    const LARGE: usize = 64;
    let small_src = format!(
        "IMPORT(\"wlwl:std.collection\", [\"SORT\"]); SORT({}, FUN((a, b), <(a, b)))",
        string_array_literal(SMALL)
    );
    let large_src = format!(
        "IMPORT(\"wlwl:std.collection\", [\"SORT\"]); SORT({}, FUN((a, b), <(a, b)))",
        string_array_literal(LARGE)
    );
    let (small_val, small) = value_and_work_of(&small_src);
    let (large_val, large) = value_and_work_of(&large_src);

    // 语义:升序,同一批元素。
    assert_eq!(
        small_val,
        format!(
            "[{}]",
            (0..SMALL)
                .map(|i| format!("s{i:02}"))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        "SORT must return the same elements in ascending order"
    );
    assert_eq!(
        large_val,
        format!(
            "[{}]",
            (0..LARGE)
                .map(|i| format!("s{i:02}"))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        "SORT must return the same elements in ascending order"
    );
    // 比较器调用次数 = n(n-1)/2。
    assert_eq!(
        small.comparator_calls,
        (SMALL * (SMALL - 1) / 2) as u64,
        "SORT over {SMALL} elements must call the comparator n(n-1)/2 times (called {})",
        small.comparator_calls
    );
    assert_eq!(
        large.comparator_calls,
        (LARGE * (LARGE - 1) / 2) as u64,
        "SORT over {LARGE} elements must call the comparator n(n-1)/2 times (called {})",
        large.comparator_calls
    );
    // 交叉核对:值比较原语的字符串比较次数必须与比较器调用次数相等
    // (比较器每次调用恰好一次 `<`)。
    assert_eq!(
        small.string_comparisons, small.comparator_calls,
        "each comparator call must perform exactly one value comparison (comparisons={}, calls={})",
        small.string_comparisons, small.comparator_calls
    );
    assert_eq!(
        large.string_comparisons, large.comparator_calls,
        "each comparator call must perform exactly one value comparison (comparisons={}, calls={})",
        large.string_comparisons, large.comparator_calls
    );
    // 规范性上界:O(n²) 最坏(c = 1)。
    assert!(
        small.comparator_calls <= (SMALL as u64) * (SMALL as u64),
        "the comparator call count must stay within the O(n^2) worst-case bound (called {})",
        small.comparator_calls
    );
    assert!(
        large.comparator_calls <= (LARGE as u64) * (LARGE as u64),
        "the comparator call count must stay within the O(n^2) worst-case bound (called {})",
        large.comparator_calls
    );
    // 下界断言(§18.5.2 的纪律):计量点真被走到。
    assert!(
        small.comparator_calls > 0,
        "the comparator call path must be counted at all (called = {})",
        small.comparator_calls
    );
    // 二次:两档读数之比 ≈ 规模之比的平方(= 16),不是线性(= 4)。
    assert!(
        large.comparator_calls > small.comparator_calls * (LARGE / SMALL) as u64,
        "the comparator call count must grow faster than linearly (small={}, large={})",
        small.comparator_calls,
        large.comparator_calls
    );
}
