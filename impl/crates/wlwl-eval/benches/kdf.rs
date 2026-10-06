//! [v0.11.3 M6 / W-05] `std.encode` 安全成员基准 —— 口径:**release 直调
//! `StdFn`**,不经解释器(与 `encode.rs` 段同款),Windows MSVC。
//!
//! **为什么必须是 release**:两个 KDF 的成本**就是**它们的价值,单测跑的是
//! 未优化构建(实测 debug 下 600 000 轮 `PBKDF2_ITER` ≈ 11 s,单是这一条就把
//! 整个 probe 套件拖慢四倍)。addendum-04 §4 那条「100 000 轮 ≤ 100 ms」的
//! 性能断言**只可能在这里钉** —— 在单测里写死毫秒数会因未优化构建恒红,而恒红
//! 的门禁等于没有门禁。
//!
//! 三组:
//! 1. **KDF 成本**(`pbkdf2_iter` / `argon2id`)—— 决定 OWASP 下界在**本机**
//!    到底值多少时间,以及调用方能不能接受;
//! 2. **熵源**(`random_hex` / `random_bytes`)—— addendum-04 §4 的
//!    「`RANDOM_BYTES(4096)` ≤ 50 µs」;
//! 3. **常数时间比较的线性形状**(`timing_safe_eq`)—— **每字节成本必须持平**。
//!    这一列**上升**就说明有提前返回;持平只说明「没有随内容变化的分支」,
//!    真正的机制断言在 `wlwl-std` 的单元测试里(且做过变异验证),基准这里
//!    只提供量级与形状的读数。

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use wlwl_eval::Evaluator;
use wlwl_std::Value;

/// OWASP 2025 的 PBKDF2-HMAC-SHA-256 最小配置。**成员层不能低于它**
/// (下界越界报 `E0030`),所以基准只能从它起步 —— 「100 000 轮」的读数由
/// 本档**线性折算**(PBKDF2 的成本按构造与 `iter` 严格成正比:`U_1..U_c` 是
/// 一条长度为 `c` 的链,没有别的乘法项)。
const OWASP_PBKDF2_ITER: i64 = 600_000;
/// Argon2id 的 OWASP 2025 最小配置:m = 19 456 KiB / t = 2 / p = 1。
const OWASP_ARGON2_M: i64 = 19_456;
const OWASP_ARGON2_T: i64 = 2;
const OWASP_ARGON2_P: i64 = 1;
/// 8 字节盐(两个 KDF 都够得着的最短值;`RANDOM_HEX(4)` 正好给这个)。
const SALT_HEX: &str = "0001020304050607";

fn int(v: i64) -> Value {
    Value::Integer(v)
}

fn bench_kdf_cost(c: &mut Criterion) {
    let mut group = c.benchmark_group("kdf");
    let mut ev = Evaluator::new();

    // PBKDF2:两个档位用来钉「成本与 iter 成正比」这条线性形状。
    group.bench_function("pbkdf2_iter_600k_len32", |b| {
        b.iter(|| {
            let out = wlwl_std::encode::pbkdf2_iter(
                &mut ev,
                vec![
                    Value::String(black_box("passwd".to_string())),
                    Value::String(black_box(SALT_HEX.to_string())),
                    int(OWASP_PBKDF2_ITER),
                    int(32),
                    Value::String("sha256".into()),
                ],
            )
            .expect("pbkdf2_iter succeeds");
            black_box(out);
        });
    });
    group.bench_function("pbkdf2_iter_1m_len32", |b| {
        b.iter(|| {
            let out = wlwl_std::encode::pbkdf2_iter(
                &mut ev,
                vec![
                    Value::String(black_box("passwd".to_string())),
                    Value::String(black_box(SALT_HEX.to_string())),
                    int(1_000_000),
                    int(32),
                    Value::String("sha256".into()),
                ],
            )
            .expect("pbkdf2_iter succeeds");
            black_box(out);
        });
    });

    // Argon2id:OWASP 下界一档,外加 t=3 的一档看轮数的边际成本。
    group.bench_function("argon2id_owasp_floor_m19m_t2", |b| {
        b.iter(|| {
            let out = wlwl_std::encode::argon2id(
                &mut ev,
                vec![
                    Value::String(black_box("passwd".to_string())),
                    Value::String(black_box(SALT_HEX.to_string())),
                    int(OWASP_ARGON2_T),
                    int(OWASP_ARGON2_M),
                    int(OWASP_ARGON2_P),
                    int(32),
                ],
            )
            .expect("argon2id succeeds");
            black_box(out);
        });
    });
    group.bench_function("argon2id_m19m_t3", |b| {
        b.iter(|| {
            let out = wlwl_std::encode::argon2id(
                &mut ev,
                vec![
                    Value::String(black_box("passwd".to_string())),
                    Value::String(black_box(SALT_HEX.to_string())),
                    int(3),
                    int(OWASP_ARGON2_M),
                    int(OWASP_ARGON2_P),
                    int(32),
                ],
            )
            .expect("argon2id succeeds");
            black_box(out);
        });
    });
    group.finish();
}

fn bench_entropy(c: &mut Criterion) {
    let mut group = c.benchmark_group("entropy");
    let mut ev = Evaluator::new();
    group.throughput(Throughput::Bytes(4096));
    group.bench_function("random_hex_4096", |b| {
        b.iter(|| {
            let out = wlwl_std::encode::random_hex(&mut ev, vec![int(4096)]).expect("random_hex");
            black_box(out);
        });
    });
    group.bench_function("random_bytes_4096", |b| {
        b.iter(|| {
            let out =
                wlwl_std::encode::random_bytes(&mut ev, vec![int(4096)]).expect("random_bytes");
            black_box(out);
        });
    });
    // 小额抽样是真实场景(16 字节盐 / 32 字节 tag),单独列一档。
    group.throughput(Throughput::Bytes(16));
    group.bench_function("random_hex_16_salt", |b| {
        b.iter(|| {
            let out = wlwl_std::encode::random_hex(&mut ev, vec![int(16)]).expect("random_hex");
            black_box(out);
        });
    });
    group.finish();
}

fn bench_timing_safe_eq(c: &mut Criterion) {
    let mut group = c.benchmark_group("timing_safe_eq");
    let mut ev = Evaluator::new();
    // 尺寸跨三个数量级。**每字节成本持平** = 没有随长度/内容变化的提前返回。
    for n in [64usize, 1024, 16_384, 262_144] {
        let s = "a".repeat(n);
        let mut differs_at_front = s.clone();
        differs_at_front.replace_range(0..1, "z");
        group.throughput(Throughput::Bytes(n as u64));
        group.bench_function(format!("equal_{n}"), |b| {
            b.iter(|| {
                let out = wlwl_std::encode::timing_safe_eq(
                    &mut ev,
                    vec![
                        Value::String(black_box(s.clone())),
                        Value::String(black_box(s.clone())),
                    ],
                )
                .expect("timing_safe_eq");
                black_box(out);
            });
        });
        group.bench_function(format!("differs_at_front_{n}"), |b| {
            b.iter(|| {
                let out = wlwl_std::encode::timing_safe_eq(
                    &mut ev,
                    vec![
                        Value::String(black_box(s.clone())),
                        Value::String(black_box(differs_at_front.clone())),
                    ],
                )
                .expect("timing_safe_eq");
                black_box(out);
            });
        });
        // 长度不等:走 `max(len)` 步,成本应与同长度的相等路径同量级。
        // **这一档是「不提前返回」最直观的读数** —— 若实现短路,它会塌到 0。
        group.bench_function(format!("shorter_{n}"), |b| {
            b.iter(|| {
                let out = wlwl_std::encode::timing_safe_eq(
                    &mut ev,
                    vec![
                        Value::String(black_box(s.clone())),
                        Value::String("".into()),
                    ],
                )
                .expect("timing_safe_eq");
                black_box(out);
            });
        });
    }
    group.finish();
}

criterion_group!(benches, bench_kdf_cost, bench_entropy, bench_timing_safe_eq);
criterion_main!(benches);
