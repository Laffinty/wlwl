//! [v0.11.3 M2] `std.encode` 哈希成员基准 —— §7.4 指标:SHA256 ≥ 100 MB/s。
//!
//! 直调 `StdFn`(不经解释器循环);`Value::String` 的 clone 计入
//! (~10 KB memcpy,相对 SHA-256 本体可忽略)。线性承诺的超线性检查由
//! 尺寸变体承担(后续按需追加)。

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use wlwl_eval::Evaluator;
use wlwl_std::Value;

fn corpus() -> String {
    let base = "a<b>&\"' 世界 héllo &copy; <tag>text</tag>\n".to_string();
    let mut s = String::new();
    while s.len() < 10_000 {
        s.push_str(&base);
    }
    s
}

fn bench_encode(c: &mut Criterion) {
    let mut group = c.benchmark_group("encode");
    let s = corpus();
    group.throughput(Throughput::Bytes(s.len() as u64));
    group.bench_function("sha256_10kb", |b| {
        let mut ev = Evaluator::new();
        b.iter(|| {
            let out = wlwl_std::encode::sha256(&mut ev, vec![Value::String(black_box(s.clone()))])
                .expect("sha256 succeeds");
            black_box(out);
        });
    });
    group.finish();
}

criterion_group!(benches, bench_encode);
criterion_main!(benches);
