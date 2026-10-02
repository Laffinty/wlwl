//! [v0.11.3 M1] `std.sanitize` 成员基准 —— 性能契约(计划 §0 性能条款 / §7.4)。
//!
//! 口径:**直调 `StdFn`**,不经解释器循环 —— 量的是成员本身的吞吐,不是
//! 「解释器调用 + 成员」的混合体。`Evaluator` 实现 `StdHost`,仅作为诊断
//! 通道传入(成功路径不触它);`args` 每次迭代重新构造(`Value::String`
//! 的 clone 计入成本 —— 一次 10 KB memcpy,相对转义本体可忽略,注释在此
//! 是为了诚实,不是为了辩解)。
//!
//! 线性承诺(§0 性能条款 ②):这些基准固定 10 KB 语料;尺寸扫描的
//! 超线性检查由 W-08 的 R1 原型对照 + baseline.txt M1 段承担。

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use wlwl_eval::Evaluator;
use wlwl_std::{Outcome, Value};

/// ~10 KB、实体丰富的语料:转义 / 解码两条路径都会真实工作。
fn corpus() -> String {
    let base = "a<b>&\"' 世界 héllo &copy; <tag>text</tag>\n".to_string();
    let mut s = String::new();
    while s.len() < 10_000 {
        s.push_str(&base);
    }
    s
}

fn bench_sanitize(c: &mut Criterion) {
    let mut group = c.benchmark_group("sanitize");
    let s = corpus();
    // escaped 语料 = 用本模块的 ESCAPE 现做(含大量实体,UNESCAPE 真实工作)。
    let escaped: String = {
        let mut ev = Evaluator::new();
        match wlwl_std::sanitize::html_escape(&mut ev, vec![Value::String(s.clone())])
            .expect("escape succeeds")
        {
            Outcome {
                value: Value::String(x),
                ..
            } => x,
            other => panic!("unexpected outcome: {other:?}"),
        }
    };

    group.throughput(Throughput::Bytes(s.len() as u64));
    group.bench_function("html_escape_10kb", |b| {
        let mut ev = Evaluator::new();
        b.iter(|| {
            let out: Outcome =
                wlwl_std::sanitize::html_escape(&mut ev, vec![Value::String(black_box(s.clone()))])
                    .expect("escape succeeds");
            black_box(out);
        });
    });

    group.throughput(Throughput::Bytes(escaped.len() as u64));
    group.bench_function("html_unescape_10kb", |b| {
        let mut ev = Evaluator::new();
        b.iter(|| {
            let out: Outcome = wlwl_std::sanitize::html_unescape(
                &mut ev,
                vec![Value::String(black_box(escaped.clone()))],
            )
            .expect("unescape succeeds");
            black_box(out);
        });
    });
    group.finish();
}

criterion_group!(benches, bench_sanitize);
criterion_main!(benches);
