//! [v0.11.3 M6 / addendum-08 W-04] `std.rand` 基准 —— 口径:**release 直调
//! `StdFn`**,不经解释器(与 `kdf.rs` 段同款),Windows MSVC。
//!
//! ## 这一组基准要回答的是**一个具体疑问**,不是「测个时间」
//!
//! ADR-0027 A2 选 `DICT` 表示状态,明写「代价是每次调用有一次**字典构造**,
//! **要进基准(不得假设它免费)**」。⇒ 本文件量的**就是**那个代价:
//!
//! 1. `seed` —— 熵 → SplitMix64 → 状态字典,**每次**都新建 dict;
//! 2. `draw_int` / `draw_uniform` / `draw_codepoint` —— 每次调用都要
//!    **拆**入参字典(4 次键查找)+ **建**出参字典(4 次 `Value` 装箱);
//! 3. `raw_xoshiro` —— **同一序列但完全绕过 dict**。它与 `draw_*` 的差值
//!    就是「`DICT` 状态的代价」,这正是 A2 写下的那个未知量。
//!
//! ## 计时断言为什么**不在这里也不在单测里**
//!
//! `addendum-04` 的教训:`PBKDF2_ITER` 计划写「100 000 轮 ≤ 100 ms」实测 138.8 ms,
//! 那个阈值源自一个**对实现语言偏乐观的估计**,不是实现病态 ⇒ 处置是把绝对阈值
//! 换成**回归闸**(实测值 + 余量)。本文件同样只提供**量级与形状**的读数,
//! 全表用「> 110% 基线均值即回归」这条统一口径;`baseline.txt` 记下本机读数。
//!
//! **「绝不返 1.0」「无偏」这类机制性质不靠计时证明**:前者靠 `(next_u64() >> 11)`
//! 的构造 + 单测的 200 000 次,后者靠单测的**确定性**拒绝测试
//! (`2^64 mod 7 = 2` ⇒ 最高两个 `u64` 必然被拒)—— 统计测试永远抓不到量级
//! `n / 2^64` 的偏差。

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use wlwl_std::Value;

/// 播种用的固定熵。**不用随机熵**:基准的读数必须可复现。
const ENTROPY: &str = "wlwl";

fn int(v: i64) -> Value {
    Value::Integer(v)
}

fn state() -> Value {
    Value::String(ENTROPY.to_string())
}

/// 直调成员需要 `StdHost`。`std.rand` 是纯计算、不碰宿主,所以这个
/// `impl` 里任何一次 `ctx()` 调用都会 panic —— 基准因此**顺带**证明了
/// 「不经宿主」这件事(单测里同款断言)。
struct NullHost;

impl wlwl_std::StdHost for NullHost {
    fn ctx(&mut self) -> &mut wlwl_std::StdCtx {
        panic!("std.rand must not touch StdCtx")
    }
    fn call(
        &mut self,
        _f: &Value,
        _args: Vec<Value>,
        _name: &str,
    ) -> wlwl_error::WlwlResult<wlwl_std::Outcome> {
        panic!("std.rand has no member that calls back into the host")
    }
    fn diag(&mut self, code: wlwl_error::ErrorCode, message: String) -> wlwl_error::WlwlError {
        wlwl_error::WlwlDiagnostic::new(code, message, wlwl_error::Location::point("<bench>", 0, 0))
            .into()
    }
}

fn bench_seed(c: &mut Criterion) {
    let mut group = c.benchmark_group("rand_seed");
    let mut host = NullHost;
    group.throughput(Throughput::Elements(1));
    group.bench_function("seed_4_bytes", |b| {
        b.iter(|| {
            let out = wlwl_std::rand::seed(&mut host, vec![state()]).expect("SEED");
            black_box(out.value)
        })
    });
    group.finish();
}

fn bench_draw(c: &mut Criterion) {
    let mut group = c.benchmark_group("rand_draw");
    let mut host = NullHost;
    // 每档抽 1000 次 —— 单次成本在纳秒量级,少于这个数读数会被计时器噪声吃掉。
    const N: usize = 1000;
    group.throughput(Throughput::Elements(N as u64));

    // 进组的初始状态只建一次(它是**基准输入**,不是被测对象)。
    let base = wlwl_std::rand::seed(&mut host, vec![state()])
        .expect("SEED")
        .value;

    group.bench_function("int_range_0_1e9", |b| {
        b.iter(|| {
            let mut cur = base.clone();
            for _ in 0..N {
                let out =
                    wlwl_std::rand::int_range(&mut host, vec![cur, int(0), int(1_000_000_000)])
                        .expect("INT_RANGE");
                let Value::Array(pair) = out.value else {
                    panic!()
                };
                cur = pair[1].clone();
            }
            black_box(cur)
        })
    });

    group.bench_function("uniform_unit", |b| {
        b.iter(|| {
            let mut cur = base.clone();
            for _ in 0..N {
                let out = wlwl_std::rand::uniform(&mut host, vec![cur]).expect("UNIFORM");
                let Value::Array(pair) = out.value else {
                    panic!()
                };
                cur = pair[1].clone();
            }
            black_box(cur)
        })
    });

    group.bench_function("uniform_0_10", |b| {
        b.iter(|| {
            let mut cur = base.clone();
            for _ in 0..N {
                let out = wlwl_std::rand::uniform(
                    &mut host,
                    vec![cur, Value::Float(0.0), Value::Float(10.0)],
                )
                .expect("UNIFORM");
                let Value::Array(pair) = out.value else {
                    panic!()
                };
                cur = pair[1].clone();
            }
            black_box(cur)
        })
    });

    group.bench_function("codepoint", |b| {
        b.iter(|| {
            let mut cur = base.clone();
            for _ in 0..N {
                let out = wlwl_std::rand::codepoint(&mut host, vec![cur]).expect("CODEPOINT");
                let Value::Array(pair) = out.value else {
                    panic!()
                };
                cur = pair[1].clone();
            }
            black_box(cur)
        })
    });

    group.finish();
}

/// **绕过 `DICT` 的同一序列** —— 与 `rand_draw` 的差值就是 A2 那句
/// 「字典构造要进基准」的答案。
///
/// ⚠️ 它走的是**同一份冻结向量**(同一个 `SEED("wlwl")`),所以两条基准
/// 量的是同一件事的两种实现,差值可解释。
fn bench_raw_generator(c: &mut Criterion) {
    let mut group = c.benchmark_group("rand_raw");
    const N: usize = 1000;
    group.throughput(Throughput::Elements(N as u64));

    // 用契约表里那个**逐字节冻结**的状态当起点(同熵同状态)。
    let st: [u64; 4] = [
        6764085530779015468i64 as u64,
        -6853809058185053683i64 as u64,
        -3836445681651209747i64 as u64,
        -5665664888633704124i64 as u64,
    ];

    group.bench_function("int_range_0_1e9_no_dict", |b| {
        b.iter(|| {
            let mut r = wlwl_std::rand::Xoshiro::new(st);
            let mut acc: u64 = 0;
            for _ in 0..N {
                acc = acc.wrapping_add(r.unbiased_below(1_000_000_000));
            }
            black_box(acc)
        })
    });

    group.bench_function("uniform_unit_no_dict", |b| {
        b.iter(|| {
            let mut r = wlwl_std::rand::Xoshiro::new(st);
            let mut acc = 0.0f64;
            for _ in 0..N {
                acc += r.unit();
            }
            black_box(acc)
        })
    });

    group.finish();
}

criterion_group!(benches, bench_seed, bench_draw, bench_raw_generator);
criterion_main!(benches);
