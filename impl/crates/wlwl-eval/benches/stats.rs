//! [v0.11.3 M7 / addendum-10 W-07] `std.math` 统计族九成员的规模基准 ——
//! 口径:**release 直调 `StdFn`**,不经解释器(与 `kdf.rs` 段同款),Windows MSVC。
//!
//! 九成员:`MEDIAN` / `MODE` / `VARIANCE_S` / `VARIANCE_P` / `STDDEV_S` /
//! `STDDEV_P` / `PERCENTILE` / `CORRELATION` / `LINEAR_FIT`,分三组:
//!
//! ```text
//!   stats_variance   VARIANCE_S/P · STDDEV_S/P   单趟 O(n)(stddev 多一次 sqrt)
//!   stats_order      MEDIAN · PERCENTILE          排序 O(n log n)
//!   stats_mode_quadratic  MODE                   **O(n²)**,档位单独封顶(见下)
//!   stats_pair       CORRELATION · LINEAR_FIT    两趟 O(n)
//! ```
//!
//! ## ⚠ `MODE` 是平方级的(D13-015)
//!
//! `stats.rs::mode` 对**每个**元素全量扫一遍数组数它的出现次数 ⇒ n × n 次比较,
//! **与语料互异程度无关**(真实数据只是让常数小一点,阶数不变)。10 万元素需要
//! 10¹⁰ 次比较,基准会跑到天荒地老 ⇒ 本文件给它**单独一档封顶的阶梯**
//! `stats_mode_quadratic`(500/1 000/2 000/4 000),并**故意留 ×2 阶梯**让平方在
//! 读数里看得见(每翻一档应当 ×4)。**发现它的是基准,不是代码审阅** —— 这正是
//! W-07 要认证的东西。修法(HashMap 计数,与 `std.collection` 的 `DEDUP_BY`
//! 同款)是一次独立裁决,**不在 W-07 范围**。
//!
//! ## ⚠ 语料必须**乱序**:这一条比档位选择重要
//!
//! 本仓其余 bench 一律用 `RANGE(0, n, 1)` 建被测数组(线性且便宜)。统计族**不能**
//! 照抄:`MEDIAN` / `PERCENTILE` 要排序,而 Rust 的 pdqsort 对**已升序**输入有
//! 快路径。`RANGE(0, n, 1)` 恰好就是升序 ⇒ 量到的是排序的最好情况,会把两个
//! 成员的成本低估一大截,且低估幅度随 n 变化 ⇒ **形状读数直接失真**。
//! 故本文件用**固定种子的 LCG** 造确定性伪随机浮点语料:可复现、不引入依赖、
//! 且天然乱序。
//!
//! 代价要说清:LCG 低位有短周期,不过 `MEDIAN` / 方差这类统计量只关心分布的一阶矩
//! 与次序统计量,低位周期不影响读数;而**可复现性**才是这里真正需要的性质。
//!
//! ## ⚠ 必须配一条**同形状基线**相减(段十 同款,不是可选项)
//!
//! 九个成员的签名都是 `fn(…, Vec<Value>)`,即**按值拿走**数组。而
//! `black_box(xs.clone())` 那句克隆落在 `b.iter` **之内** —— 一次迭代要先深拷
//! n 个 `Value` 才能调成员。那笔拷贝是 **O(n)**,会随 n 放大:第一版量到
//! `VARIANCE_S` 每元素成本 1 k = 12.0 ns、5 k = 11.9 ns、25 k 却跳到 **25.8 ns**,
//! 那不是成员变慢了,是拷贝开始占大头(100 k 档会是 3.2 MB/次)。
//!
//! ⇒ 本文件加一组 `stats_baseline`:**同形状、同档位、只克隆不调成员**,落表前
//! 逐项相减。这与 `baseline.txt` 段十(`std.result`)是同一条纪律:**跨形状减基线
//! 没有意义**,基线必须与被测项**逐档同形状**。
//!
//! ## 计划 §4 那条性能目标已经作废
//!
//! 计划原文:`MEAN` 10 万元素 ≥ 100 MB/s(依赖 00)。⚠ **`MEAN` 在 W-02 被砍了**
//! (`MEAN ≡ /(SUM(arr), LEN(arr))`,连 `REDUCE` 都不用),所以那条阈值**失去了它要量
//! 的那个成员** —— 不能拿 `SUM` 或别的成员顶上去说「达标」。本段只提供九成员
//! 自身的量级与形状读数;阈值如何重定见 `baseline.txt` 段十二。
//!
//! 取数命令:
//!
//! ```text
//! cargo bench -p wlwl-eval --bench stats -- \
//!     --warm-up-time 1 --measurement-time 2 --sample-size 10
//! ```

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use wlwl_error::WlwlResult;
use wlwl_eval::Evaluator;
use wlwl_std::{Outcome, StdHost, Value};

/// ×5 阶梯,含计划 §4 点名的 **10 万元素**那一档。
const TIERS: [usize; 4] = [1_000, 5_000, 25_000, 100_000];

/// ⚠ `MODE` 专用阶梯 —— 见 `bench_mode_quadratic` 的注释。**不要**调到 `TIERS`。
const MODE_TIERS: [usize; 4] = [500, 1_000, 2_000, 4_000];

/// `stats.rs` 里九个成员的统一签名(`R<T>` 在那边是私有的 `WlwlResult`)。
type Member = fn(&mut dyn StdHost, Vec<Value>) -> WlwlResult<Outcome>;

/// 确定性伪随机语料 —— 见模块注释「语料必须乱序」。
fn corpus(n: usize) -> Vec<Value> {
    let mut s: u64 = 0x2545_F491_4F6C_DD1D;
    (0..n)
        .map(|_| {
            s = s
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let unit = ((s >> 11) as f64) / ((1u64 << 53) as f64);
            Value::Float(unit * 1000.0)
        })
        .collect()
}

fn bench_variance(c: &mut Criterion) {
    const CASES: [(&str, Member); 4] = [
        ("variance_s", wlwl_std::stats::variance_s as Member),
        ("variance_p", wlwl_std::stats::variance_p as Member),
        ("stddev_s", wlwl_std::stats::stddev_s as Member),
        ("stddev_p", wlwl_std::stats::stddev_p as Member),
    ];
    let mut group = c.benchmark_group("stats_variance");
    let mut ev = Evaluator::new();
    for &n in &TIERS {
        let xs = Value::Array(corpus(n));
        group.throughput(Throughput::Elements(n as u64));
        for (name, f) in CASES {
            group.bench_with_input(BenchmarkId::new(name, n), &n, |b, _| {
                b.iter(|| {
                    let out = f(&mut ev, vec![black_box(xs.clone())]).expect("stats");
                    black_box(out);
                });
            });
        }
    }
    group.finish();
}

fn bench_order(c: &mut Criterion) {
    let mut group = c.benchmark_group("stats_order");
    let mut ev = Evaluator::new();
    for &n in &TIERS {
        let xs = Value::Array(corpus(n));
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::new("median", n), &n, |b, _| {
            b.iter(|| {
                let out =
                    wlwl_std::stats::median(&mut ev, vec![black_box(xs.clone())]).expect("stats");
                black_box(out);
            });
        });
        group.bench_with_input(BenchmarkId::new("percentile", n), &n, |b, _| {
            b.iter(|| {
                // ⚠ `p` 必须是 **FLOAT**:`PERCENTILE` 对 `Integer` 报 E0030
                // (`p` 是数据不是参数,但类型判定是严格的)。传 Integer 会让整份
                // 基准在第一档就 panic —— 这是本文件第一版的实际经过。
                let out = wlwl_std::stats::percentile(
                    &mut ev,
                    vec![black_box(xs.clone()), Value::Float(95.0)],
                )
                .expect("stats");
                black_box(out);
            });
        });
    }
    group.finish();
}

/// ⚠ **`MODE` 是平方级的**(D13-015),所以它的档位**必须封顶**,不能用 `TIERS`。
///
/// `stats.rs::mode` 的实现是:对**每个**元素 `x` 全量扫一遍数组数它的出现次数
/// (`xs.iter().filter(|y| **y == x).count()`)。那是 n × n 次比较,**与语料的
/// 互异程度无关** —— 真实数据(取值很少)只是让常数小一点,渐近阶数不变。
/// 10 万元素 ⇒ 10¹⁰ 次比较,基准会跑到天荒地老。
///
/// 所以本组用 `MODE_TIERS`(封顶 4 000)并**故意留一个 ×2 阶梯**,让平方这件事
/// 本身在读数里看得见:每翻一档总时间应当 **×4**。这比把它藏起来有价值 ——
/// 平方级是必须被看见的事实,不是要绕开的麻烦。
fn bench_mode_quadratic(c: &mut Criterion) {
    let mut group = c.benchmark_group("stats_mode_quadratic");
    let mut ev = Evaluator::new();
    for &n in &MODE_TIERS {
        let xs = Value::Array(corpus(n));
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| {
                let out =
                    wlwl_std::stats::mode(&mut ev, vec![black_box(xs.clone())]).expect("stats");
                black_box(out);
            });
        });
    }
    group.finish();
}

fn bench_pair(c: &mut Criterion) {
    const CASES: [(&str, Member); 2] = [
        ("correlation", wlwl_std::stats::correlation as Member),
        ("linear_fit", wlwl_std::stats::linear_fit as Member),
    ];
    let mut group = c.benchmark_group("stats_pair");
    let mut ev = Evaluator::new();
    for &n in &TIERS {
        let xs = Value::Array(corpus(n));
        // ⚠ 第二条序列**不是**第一条的副本:两条完全相同会让相关系数恒为 1.0,
        // 那是最容易触发退化分支的输入,量到的是特例而不是成员的成本。
        let ys = Value::Array(corpus(n).into_iter().rev().collect());
        group.throughput(Throughput::Elements(n as u64));
        for (name, f) in CASES {
            group.bench_with_input(BenchmarkId::new(name, n), &n, |b, _| {
                b.iter(|| {
                    let out = f(&mut ev, vec![black_box(xs.clone()), black_box(ys.clone())])
                        .expect("stats");
                    black_box(out);
                });
            });
        }
    }
    group.finish();
}

/// ⚠ **落表前必须减掉的那条基线** —— 只克隆、不调成员。
///
/// 九个成员都按值拿走数组,所以 `b.iter` 里必有 `xs.clone()`;它是 O(n) 的,
/// 不减掉就会随 n 放大并歪掉形状读数(见模块注释)。`pair` 那组要克隆**两条**,
/// 基线也必须是两条 —— 基线必须与被测项**逐档同形状**。
fn bench_baseline(c: &mut Criterion) {
    let mut group = c.benchmark_group("stats_baseline");
    for &n in &TIERS {
        let xs = Value::Array(corpus(n));
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| {
                black_box(xs.clone());
            });
        });
        group.bench_with_input(BenchmarkId::new("pair", n), &n, |b, _| {
            // ⚠ `ys` 必须在 `b.iter` **之外**建好:它对应被测项里那条同样在迭代外
            // 建好的 `ys`。把语料构造放进迭代就是**过度相减**(LCG 那趟比成员本身
            // 还贵),减出来的净值会是负数。
            let ys = Value::Array(corpus(n).into_iter().rev().collect());
            b.iter(|| {
                black_box((xs.clone(), ys.clone()));
            });
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_baseline,
    bench_variance,
    bench_order,
    bench_mode_quadratic,
    bench_pair,
);
criterion_main!(benches);
