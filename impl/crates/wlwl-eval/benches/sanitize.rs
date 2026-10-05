//! [v0.11.3 M1 / M3] `std.sanitize` 成员基准 —— 性能契约(计划 §0 性能条款 / §7.4)。
//!
//! 口径:**直调 `StdFn`**,不经解释器循环 —— 量的是成员本身的吞吐,不是
//! 「解释器调用 + 成员」的混合体。`Evaluator` 实现 `StdHost`,仅作为诊断
//! 通道传入(成功路径不触它);`args` 每次迭代重新构造(`Value::String`
//! 的 clone 计入成本 —— 一次 10 KB memcpy,相对转义本体可忽略,注释在此
//! 是为了诚实,不是为了辩解)。
//!
//! 线性承诺(§0 性能条款 ②):`html_sanitize_1kb_article` 与
//! `html_sanitize_100kb_article` 是**同一语料的两个尺寸档**,每字节成本
//! 不得随尺寸增长(100× 尺寸 ⇒ 每字节时间应在噪声内持平)。数值人工核对
//! 后写入 `baseline.txt` M3 段 —— 基准本身**不是断言**,断言在
//! `sanitize_contract.rs` 的病态契约里。
//!
//! §7.4 指标:`SANITIZE_HTML` 10 KB 文章 **≥ 10 MB/s**;病态深嵌套
//! **线性且不栈溢出**(不溢出由契约测试断言,此处只出数字)。

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

criterion_group!(benches, bench_sanitize, bench_sanitize_pipeline);
criterion_main!(benches);

/// 约 `target` 字节的「文章」语料:段落 + 排版标记 + 链接 + 实体 + 注释,
/// 覆盖净化器四段流水线的真实比例(文本多、标记少、实体适量)。
fn article(target: usize) -> String {
    let unit = "<p>Some <b>bold</b> &amp; <i>italic</i> text with an \
                <a href=\"https://example.com/a?x=1&amp;y=2\" title=\"t\">\
                entity&nbsp;rich</a> link.</p><!-- note --><ul><li>item \
                &copy; 2026</li></ul>\n"
        .to_string();
    let mut s = String::with_capacity(target + unit.len());
    while s.len() < target {
        s.push_str(&unit);
    }
    s
}

/// 约 `target` 字节的「评论」语料:短文本 + 内联标记 + 零散实体,无块级结构。
fn comment(target: usize) -> String {
    let unit = "nice post! <b>really</b> &amp; <a href=\"/u/1\">@who</a> \
                &lt;3 &copy; héllo 🌍 "
        .to_string();
    let mut s = String::with_capacity(target + unit.len());
    while s.len() < target {
        s.push_str(&unit);
    }
    s
}

/// 病态深嵌套:10 万层(计划 W-11「深嵌套 ≥ 10 万层不栈溢出」)。
fn deep_nesting(depth: usize) -> String {
    let mut s = String::with_capacity(depth * 7);
    for _ in 0..depth {
        s.push_str("<b>");
    }
    s.push('x');
    for _ in 0..depth {
        s.push_str("</b>");
    }
    s
}

fn bench_sanitize_pipeline(c: &mut Criterion) {
    let mut g = c.benchmark_group("sanitize");

    let run = |s: &str| -> Outcome {
        let mut ev = Evaluator::new();
        wlwl_std::sanitize::html_sanitize(&mut ev, vec![Value::String(black_box(s.to_string()))])
            .expect("sanitize succeeds")
    };

    // §7.4 主指标:10 KB 文章 ≥ 10 MB/s
    let art10k = article(10_000);
    g.throughput(Throughput::Bytes(art10k.len() as u64));
    g.bench_function("html_sanitize_10kb_article", |b| {
        b.iter(|| black_box(run(&art10k)));
    });

    let cmt1k = comment(1_000);
    g.throughput(Throughput::Bytes(cmt1k.len() as u64));
    g.bench_function("html_sanitize_1kb_comment", |b| {
        b.iter(|| black_box(run(&cmt1k)));
    });

    // 线性承诺的两个尺寸档(同一构造,100× 尺寸)
    let art1k = article(1_000);
    g.throughput(Throughput::Bytes(art1k.len() as u64));
    g.bench_function("html_sanitize_1kb_article", |b| {
        b.iter(|| black_box(run(&art1k)));
    });

    let art100k = article(100_000);
    g.throughput(Throughput::Bytes(art100k.len() as u64));
    g.bench_function("html_sanitize_100kb_article", |b| {
        b.iter(|| black_box(run(&art100k)));
    });

    let deep = deep_nesting(100_000);
    g.throughput(Throughput::Bytes(deep.len() as u64));
    g.bench_function("html_sanitize_pathological_nesting", |b| {
        b.iter(|| black_box(run(&deep)));
    });

    g.finish();
}
