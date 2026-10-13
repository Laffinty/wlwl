//! L0-B 可变累加器规模基准 —— [v0.11.3 M7 / ADR-0032 §7-S4]
//!
//! **这个文件是 ADR-0032 D3 三件套里的第二件(基准守卫)。** 第一件计数器
//! (`acc_opt_hit` / `acc_opt_miss`)在 S2 落地,第三件契约测试也在 S2;这一件
//! 回答的是另一个问题:**优化还生效吗?**
//!
//! ## 为什么不能直接复用 `collection.rs` 里的 `r1_build_*`
//!
//! `r1_build_*` 走的是 `FOR(i, RANGE(0,n,1), append(i))` —— **闭包 + FOR**。
//! 而 `recognize_accumulator` 只在 `eval_while` 里调用,所以那一组**永远判否**,
//! L0-B 对它一点作用都没有。拿它当 L0-B 的守卫会得到一个「怎么都不变」的
//! 绿色读数,守的是别的东西。要量的形状是手写 `WHILE` + `SET(acc, 追加(acc,…))`。
//!
//! ## 四组,前三维优化、后一组是**对照组**
//!
//! ```text
//!   l0b_plus     SET(acc, +(acc, [i]))
//!   l0b_push     SET(acc, PUSH(acc, i))
//!   l0b_concat   SET(acc, CONCAT(acc, [i]))
//!   l0b_control  同 `l0b_push` 的源码,只在循环体末尾多一句 LEN(acc)
//! ```
//!
//! ⚠ **`l0b_control` 是这一组里最要紧的一组。** `LEN(acc)` 让 `acc` 在循环体里
//! 多了一次**非累加器位**的出现,于是 `scan_acc_uses` 数出 `bad > 0`,识别判否。
//! 它与前三组**只差这一句**,所以两者之差就是「优化本身」的净值 —— 若哪天有人
//! 把优化改坏,前三组会与对照组一起变慢(信号 1.0×),而不是一起变快
//! (伪绿色)。没有对照组的话,一次「整条解释器都变快了」的改动会让守卫误判为
//! 优化仍在生效。
//!
//! 前三组之间也要互相看:ADR-0032 D2 实测三种追加写法成本落在噪声内
//! (8 k 时 889/906/940 ms,极差 5.7%),所以它们**必须都平**。只平一个说明
//! 识别谓词漏了形态 —— 那正是 D2 要防的静默性能回退。
//!
//! ## 判据:每元素成本**持平**,不是「总时间变短」
//!
//! 每档报 `Throughput::Elements(n)`,criterion 直接给 ns/元素。四档 1k→8k
//! 的比值就是「持平」与否的判据:线性 ⇒ 比值 ≈ 1(实测为**下降**,因为固定开销
//! 被摊薄);平方 ⇒ 比值 ≈ n/起始档(8 倍)。
//!
//! ## 口径与本仓其余 bench 同款
//!
//!   * 解析一次、每轮新建 `Evaluator`(量 eval,不量 parse)
//!   * `black_box` 结果防优化器吞掉工作
//!
//! ```text
//! cargo bench -p wlwl-eval --bench accumulator -- \
//!     --warm-up-time 1 --measurement-time 2 --sample-size 10
//! ```
//!
//! ⚠ **`l0b_control` 在 8 k 档单次约 1.1 s**(优化前就是平方级),criterion 会
//! 因此只跑很少几轮迭代。这是**预期行为**:对照组本来就该慢,它的价值是那个
//! 数量级的差,不是精度。

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use wlwl_ast::Expr;
use wlwl_eval::Evaluator;
use wlwl_parser::parse_with_warnings;

const FILE: &str = "<bench>";

/// ×2 阶梯,与 `collection.rs` 的 `TIERS` 同,便于并排读。
const TIERS: [usize; 4] = [1_000, 2_000, 4_000, 8_000];

fn parse_program(src: &str) -> Expr {
    let (ast, _warns) = parse_with_warnings(src, FILE).expect("bench parse");
    ast
}

/// Fresh `Evaluator` per iteration(与 `collection.rs` 同款)。
fn run_eval(src: &str, ast: &Expr) {
    let mut ev = Evaluator::new().with_source(src, FILE);
    let v = ev.eval(ast).expect("bench eval");
    black_box(v);
}

/// 手写累加器循环。`append` 是那一行追加语句,`tail` 是循环体末尾的可选尾巴。
///
/// ⚠ 计数器 `i` 与累加器 `acc` **同形**(`SET(i, +(i, 1))` 也是候选),判别式
/// 靠「出现在循环条件里的是计数器」把它们分开 —— 这条判别式本身在 S2 的契约里
/// 钉着(`counter_is_not_an_accumulator`),这里不重复说明。
fn acc_src(n: usize, append: &str, tail: &str) -> String {
    format!(
        "LET(main, FUN((), (\n\
         \x20   LET MUT(acc, []);\n\
         \x20   LET MUT(i, 0);\n\
         \x20   WHILE(<(i, {n}), ({append} SET(i, +(i, 1)){tail}));\n\
         \x20   LEN(acc)\n\
         \x20 )));\n\
         \x20main();\n"
    )
}

/// 三形态各一组。D2 实测三者成本同量级,判据是它们**都**持平。
fn bench_plus(c: &mut Criterion) {
    bench_shape(c, "l0b_plus", "SET(acc, +(acc, [i]));");
}

fn bench_push(c: &mut Criterion) {
    bench_shape(c, "l0b_push", "SET(acc, PUSH(acc, i));");
}

fn bench_concat(c: &mut Criterion) {
    bench_shape(c, "l0b_concat", "SET(acc, CONCAT(acc, [i]));");
}

/// 对照组:同一份源码,只多一句 `LEN(acc)` ⇒ `bad > 0` ⇒ 识别判否 ⇒ 平方级。
fn bench_control(c: &mut Criterion) {
    bench_shape_tail(c, "l0b_control", "SET(acc, PUSH(acc, i));", "; LEN(acc)");
}

fn bench_shape(c: &mut Criterion, group: &str, append: &str) {
    bench_shape_tail(c, group, append, "");
}

fn bench_shape_tail(c: &mut Criterion, group: &str, append: &str, tail: &str) {
    let mut g = c.benchmark_group(group);
    for &n in &TIERS {
        let src = acc_src(n, append, tail);
        let ast = parse_program(&src);
        g.throughput(Throughput::Elements(n as u64));
        g.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| run_eval(&src, &ast));
        });
    }
    g.finish();
}

criterion_group!(benches, bench_plus, bench_push, bench_concat, bench_control,);
criterion_main!(benches);
