//! `std.collection` 规模基准 —— [v0.11.3 M5 / addendum-00 L0-A-1]
//!
//! **这个文件要回答一个问题:把成员沉 R2(L0-A)到底能省多少?**
//! 回答之前必须先把两笔平方级成本分开,否则量出来的数没有归属:
//!
//! ```text
//!   R1 建数组链   r1_build_*        逐次 +(arr, [v]) —— 复制整个不可变数组。
//!                                       D11-012 根因。**L0-A 不修它**(L0-B 才修),
//!                                       因为它住在语言层,不在成员里。
//!   成员自身      collection_map_*   在一张**已经建好**的数组上跑成员。
//!     (×4)        collection_filter_*   这才是 L0-A(归层)能改善的那一笔。
//!                 collection_chunk_*
//!                 collection_window_*
//! ```
//!
//! **为什么建数组用 `RANGE` 而不是逐次 `+`**:自 v0.11 M5 起 `RANGE` 已沉 R2
//! (规范 §5 成员表的层列「混合(R1 门面 + R2 `RANGE`)」;锁测试
//! `range_is_bound_straight_to_the_r2_kernel` 钉住形态),它线性且便宜。若在这里
//! 沿用 `+(arr, [v])` 建被测数组,**建数组的平方级会盖过成员本身的成本**,
//! 于是无论成员有没有沉 R2,曲线都是平方的 —— 那样测出来的数无法归因到
//! 归层,也没有「归层前后对照」可言。
//!
//! 口径与本仓其余 bench 同款:
//!   * 解析一次、每轮新建 `Evaluator`(量的是 eval,不是 parse,也不是热缓存)
//!   * `black_box` 结果防优化器吞掉工作
//!   * 取数命令必须带下面三个旗,否则 criterion 默认 100 样本 × 秒级单次 =
//!     数十分钟起(而 `WINDOW` 单次已经是 24 s 级)
//!
//! ```text
//! cargo bench -p wlwl-eval --bench collection -- \
//!     --warm-up-time 1 --measurement-time 2 --sample-size 10
//! ```
//!
//! 四个档位 1k / 2k / 4k / 8k 是 ×2 阶梯,取每档**每元素成本**看形状。
//! 现状是平方级 ⇒ 每元素成本随尺寸**上涨**;归层后成员自身那四档应持平。
//! `r1_build` 那一组**归层后不会变平**,那是 L0-B 的对象,别拿它当 L0-A 的验收。

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use wlwl_ast::Expr;
use wlwl_eval::Evaluator;
use wlwl_parser::parse_with_warnings;

const FILE: &str = "<bench>";

/// ×2 阶梯。`CHUNK` / `WINDOW` 的第二实参按规范 §5 取常数 10。
const TIERS: [usize; 4] = [1_000, 2_000, 4_000, 8_000];
const WIN: usize = 10;

fn parse_program(src: &str) -> Expr {
    let (ast, _warns) = parse_with_warnings(src, FILE).expect("bench parse");
    ast
}

/// Fresh `Evaluator` per iteration(与 `eval_hot_paths.rs` 同款)。
fn run_eval(src: &str, ast: &Expr) {
    let mut ev = Evaluator::new().with_source(src, FILE);
    let v = ev.eval(ast).expect("bench eval");
    black_box(v);
}

/// 1) R1 建数组链 —— D11-012 的根因本身。`LEN` 是全局内建(附录 G),不需 IMPORT。
fn r1_build_src(n: usize) -> String {
    format!(
        "IMPORT(\"wlwl:std.collection\", [\"RANGE\"]);\n\
         LET(main, FUN((), (\n\
         \x20   LET(arr, []);\n\
         \x20   LET(append, FUN((v), (SET(arr, +(arr, [v])); arr)));\n\
         \x20   FOR(i, RANGE(0, {n}, 1), append(i));\n\
         \x20   LEN(arr)\n\
         )));\n\
         main();\n"
    )
}

/// 2) 成员基准 —— 数组由 R2 `RANGE` 建好(线性),被测成员的成本不被淹没。
fn member_src(n: usize, imports: &[&str], call: &str) -> String {
    let mut list = String::from("\"RANGE\"");
    for name in imports {
        list.push_str(&format!(", \"{name}\""));
    }
    format!(
        "IMPORT(\"wlwl:std.collection\", [{list}]);\n\
         LET(main, FUN((), (\n\
         \x20   LET(arr, RANGE(0, {n}, 1));\n\
         \x20   LEN({call})\n\
         )));\n\
         main();\n"
    )
}

// ---------- 1. r1_build:R1 建数组链(D11-012 根因)----------------------

fn bench_r1_build(c: &mut Criterion) {
    let mut g = c.benchmark_group("r1_build");
    for &n in &TIERS {
        let src = r1_build_src(n);
        let ast = parse_program(&src);
        g.throughput(Throughput::Elements(n as u64));
        g.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| run_eval(&src, &ast));
        });
    }
    g.finish();
}

// ---------- 2~5. 成员自身(L0-A 归层的对象)---------------------------
//
// 四条形状都不同,免得只测到一个:
//   map     逐元素闭包 + 回调派发(NativeInvoke::Std)
//   filter  同上但**结果更短**(n/2),故建结果数组的成本减半 —— 与 map
//           的曲线差异能顺带验证成本确实出在「建结果数组」上
//   chunk   O(n) 块 × 常数 k,输出 Θ(n·k) 个元素
//   window  输出 (n-k+1) × k = Θ(n·k),**且是这四个里最贵的**:
//           它的输出本身就是平方量级,不是实现写得差(规范 §5.1 原话)

fn bench_map(c: &mut Criterion) {
    let mut g = c.benchmark_group("collection_map");
    for &n in &TIERS {
        let src = member_src(n, &["MAP"], "MAP(arr, FUN((x), +(x, x)))");
        let ast = parse_program(&src);
        g.throughput(Throughput::Elements(n as u64));
        g.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| run_eval(&src, &ast));
        });
    }
    g.finish();
}

fn bench_filter(c: &mut Criterion) {
    let mut g = c.benchmark_group("collection_filter");
    for &n in &TIERS {
        let src = member_src(n, &["FILTER"], "FILTER(arr, FUN((x), ==(%(x, 2), 0)))");
        let ast = parse_program(&src);
        g.throughput(Throughput::Elements(n as u64));
        g.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| run_eval(&src, &ast));
        });
    }
    g.finish();
}

fn bench_chunk(c: &mut Criterion) {
    let mut g = c.benchmark_group("collection_chunk");
    for &n in &TIERS {
        let src = member_src(n, &["CHUNK"], &format!("CHUNK(arr, {WIN})"));
        let ast = parse_program(&src);
        g.throughput(Throughput::Elements(n as u64));
        g.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| run_eval(&src, &ast));
        });
    }
    g.finish();
}

fn bench_window(c: &mut Criterion) {
    let mut g = c.benchmark_group("collection_window");
    for &n in &TIERS {
        let src = member_src(n, &["WINDOW"], &format!("WINDOW(arr, {WIN})"));
        let ast = parse_program(&src);
        g.throughput(Throughput::Elements(n as u64));
        g.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| run_eval(&src, &ast));
        });
    }
    g.finish();
}

criterion_group!(
    benches,
    bench_r1_build,
    bench_map,
    bench_filter,
    bench_chunk,
    bench_window,
);
criterion_main!(benches);
