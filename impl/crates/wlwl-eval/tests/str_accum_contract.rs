//! [G-17] **字符串累加器**（`s = s + part` 的就地改写）的契约表。
//!
//! ## 它补的是什么
//!
//! 数组（`CellVec`）与字典（`CellDict`）都有「循环内累加器」的**就地改写**优化
//! ⇒ 循环 n 次的总工作量是 **O(n)**。
//! **字符串没有** ⇒ `s = s + part` 每轮都要把整条字符串**重拷一遍** ⇒ **O(n²)**。
//!
//! ## 四条契约
//!
//! 1. 循环里拼出来的字符串**逐字正确**（优化不得改可观察面）；
//! 2. 循环内的工作量**线性** —— 用**计数器**（`string_bytes_copied`）量，
//!    **不用墙钟**（机器负载一变就红 ⇒ 假门禁）；
//! 3. 可观察面与普通字符串**逐字相同**（`TYPE` / `LEN` / `==` / 渲染）；
//! 4. 不可变绑定 + 累加器形状**仍然**报 `E0024`（D13-099 的不变量：优化
//!    **不得**改绑定的可变性）。

use std::path::PathBuf;
use wlwl_eval::Evaluator;
use wlwl_value::{reset_value_work_counters, value_work_counters, ValueWorkCounters};

fn scratch_dir() -> PathBuf {
    std::env::temp_dir().join("wlwl_str_accum_contract")
}

/// 跑一段源码，返回 `(末条语句的值, 值级工作量计数)`。
fn value_and_work_of(src: &str) -> (String, ValueWorkCounters) {
    let ast = wlwl_parser::parse(src, "t.wll").expect("src must parse");
    reset_value_work_counters();
    let mut ev = Evaluator::new().with_base_dir(scratch_dir());
    let v = ev.eval(&ast).expect("program must run");
    (v.display(), value_work_counters())
}

/// 循环体：把 `part` 追加到 `s`。
///
/// ⚠ 形状必须是 **`SET(s, +(s, part))`** —— 累加器识别只认这一种（`PUSH` /
/// `+` / `CONCAT` 的第一个实参是累加器本身），且**累加器的每一次出现都必须在
/// 累加器位** ⇒ 循环体里**不能**再拿 `s` 做别的（那会让识别 miss）。
fn accum_loop(n: i64, part: &str) -> String {
    format!(
        r#"
LET MUT(s, "");
LET MUT(i, 0);
WHILE(<(i, {n}), (
    SET(s, +(s, "{part}"));
    SET(i, +(i, 1))
));
s
"#
    )
}

#[test]
fn the_accumulated_string_is_correct() {
    let (value, _) = value_and_work_of(&accum_loop(5, "ab"));
    assert_eq!(
        value, "ababababab",
        "the accumulated string must be correct"
    );
}

#[test]
fn the_accumulator_work_is_linear() {
    // n 次追加、每次追加 part_len 字节。
    //
    // ⚠ **上界取 4 倍**而不是「恰好」：循环结束时要**换回**普通字符串
    //   （一次整体克隆，O(n)）⇒ 计数里会多出这一份 ⇒ 线性但常数 > 1。
    //   写成「恰好 n*part_len」在实现调整后会假红。
    //
    // ⚠ 未优化时是 O(n²)：n=200、part=10 ⇒ 约 200 000 字节 ⇒ 远超线性上界
    //   ⇒ 这条断言**能抓住**「优化回退」。
    const N: i64 = 200;
    const PART_LEN: u64 = 10;
    let (_, work) = value_and_work_of(&accum_loop(N, "0123456789"));
    let linear_bound = 4 * (N as u64) * PART_LEN;
    assert!(
        work.string_bytes_copied <= linear_bound,
        "the accumulator must copy O(n) bytes, copied {} (linear bound {linear_bound})",
        work.string_bytes_copied
    );
}

#[test]
fn the_accumulator_work_is_linear_at_the_registered_readings() {
    // [v0.12.0 / Q4] **一致性册 §5.4 的 X-25 行**要登记的读数 ——
    // 那一行原本记的是「未达标：n = 16 ⇒ **136 字节**、n = 64 ⇒ **2080 字节**
    // （= n(n+1)/2 ⇒ Θ(n²)）」。
    //
    // ⚠ 计量口径：**每次追加 1 字节**（part 长度 = 1）⇒ 未优化时第 i 轮的 `+`
    //   拷贝 i 字节 ⇒ Σ(1..n) = n(n+1)/2 ⇒ n=16 ⇒ 136、n=64 ⇒ 2080
    //   （与那一行的数字**逐字对得上**）。
    //
    // ⚠ 优化后应当**线性** ⇒ 计数 ≲ c·n（c = 常数）。
    //   本用例把**两个档位的读数打出来**（`--nocapture` 可见）⇒
    //   规范 §5.4 的「现状」列要填的**新读数**就从这里取。
    for n in [16i64, 64] {
        let (_, work) = value_and_work_of(&accum_loop(n, "x"));
        let quadratic = (n * (n + 1) / 2) as u64;
        let linear_bound = 8 * n as u64;
        println!(
            "X-25 reading: n = {n} ⇒ string_bytes_copied = {} (quadratic would be {quadratic}, \
             linear bound {linear_bound})",
            work.string_bytes_copied
        );
        assert!(
            work.string_bytes_copied <= linear_bound,
            "the accumulator must copy O(n) bytes: n={n} copied {} (bound {linear_bound})",
            work.string_bytes_copied
        );
    }
}

#[test]
fn the_observable_surface_matches_a_plain_string() {
    // 循环结束后累加器已换回**普通字符串** ⇒ 它的可观察面必须与普通字符串逐字相同。
    let (value, _) = value_and_work_of(
        r#"
LET MUT(s, "");
LET MUT(i, 0);
WHILE(<(i, 3), (
    SET(s, +(s, "xy"));
    SET(i, +(i, 1))
));
[TYPE(s), LEN(s), s, ==(s, "xyxyxy")]
"#,
    );
    assert_eq!(
        value, "[STRING, 6, xyxyxy, TRUE]",
        "the accumulated string must be indistinguishable from a plain one"
    );
}

#[test]
fn an_immutable_binding_still_reports_e0024() {
    // ⚠ D13-099 的不变量：优化**只能就地改写已有绑定的值**，**不得**改它的可变性
    //   （否则 `LET(acc, "")`（不可变）+ 累加器形状的循环会**不报** `E0024`
    //   ⇒ 那是可观察的语义破坏）。
    let ast = wlwl_parser::parse(
        r#"
LET(s, "");
LET MUT(i, 0);
WHILE(<(i, 3), (
    SET(s, +(s, "x"));
    SET(i, +(i, 1))
));
s
"#,
        "t.wll",
    )
    .expect("src must parse");
    let mut ev = Evaluator::new().with_base_dir(scratch_dir());
    let err = ev
        .eval(&ast)
        .expect_err("SET on an immutable binding must fail");
    assert_eq!(
        err.diagnostic().code.as_str(),
        "E0024",
        "the accumulator optimization must not change the binding's mutability"
    );
}

#[test]
fn an_accumulator_that_escapes_its_position_is_not_optimized_but_stays_correct() {
    // 「宁可 miss 多，不可误判」：累加器**出现在累加器位之外** ⇒ 识别必须 miss
    // （不优化）⇒ 但**结果仍然正确**（miss 的后果只是性能没提升）。
    //
    // ⚠ **这个形状就是本项目服务器自己的形状**：`acc` 既被追加、又被
    //   `CONTAINS_SUB(acc, "\r\n\r\n")` **检查** ⇒ 累加器出现在累加器位之外
    //   ⇒ 逃逸分析判否 ⇒ **不优化**。
    //   ⇒ 也就是说：**「边追加边检查」这种最常见的写法拿不到这个优化** ——
    //   优化只覆盖「纯追加、循环里不看累加器」的形状。
    let (value, _) = value_and_work_of(
        r#"
IMPORT("wlwl:std.str", ["CONTAINS_SUB"]);
LET MUT(s, "");
LET MUT(i, 0);
WHILE(<(i, 3), (
    SET(s, +(s, "xy"));
    SET(i, +(i, 1));
    IF(CONTAINS_SUB(s, "zz"), 0, 0)
));
s
"#,
    );
    assert_eq!(
        value, "xyxyxy",
        "the result must stay correct even when the optimization misses"
    );
}
