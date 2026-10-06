//! [v0.11.3 M6 / addendum-02 W-04] `std.regex` 基准 —— **复杂度契约的 release 读数**。
//!
//! 口径:**release 直调 `StdFn`**(与 `encode.rs` / `kdf.rs` 同款),Windows MSVC,
//! criterion warm-up 1 s / measurement 2 s / 10 样本。
//!
//! ## 为什么这些数字只能在这里钉
//!
//! 契约里那两条病态用例(`(a*)*b` 在 10 000 字符上终止且不匹配)断言的是
//! **机制**,而计划 §4 要的「病态输入 ≤ 50 ms」是一个**计时**数字 ——
//! 计时断言写进 `cargo test` 在 CI 上必然抖动,于是就是一条随机变红的门禁。
//! 所以:**机制进单测 / 契约,阈值进这里**。这是 `addendum-04` W-05 那条
//! 纪律(「该钉的写在钉得住的地方,钉不住的写明为什么钉不住」)的第二次应用。
//!
//! `re_pathological_10k` **就是那条回归闸**:若哪天实现里混进了回溯,这一档
//! 会从毫秒级掉到「跑不完」,基准超时而不是给出错答案。

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use wlwl_eval::Evaluator;
use wlwl_std::Value;

/// 10 KB 语料:约 1/8 的位置是数字,其余是字母 —— 足够让 `[0-9]+` 真的做功。
fn corpus(n: usize) -> String {
    let mut s = String::with_capacity(n + 1);
    while s.len() < n {
        s.push_str("abcdefgh12345678");
    }
    s.truncate(n);
    s
}

fn compile_pat(ev: &mut Evaluator, pattern: &str) -> Value {
    let v = wlwl_std::regex::re(ev, vec![Value::String(black_box(pattern.to_string()))])
        .expect("RE compiles");
    black_box(v.value)
}

fn search(ev: &mut Evaluator, pat: &Value, text: &str) -> Option<Value> {
    let out = wlwl_std::regex::re_search(
        ev,
        vec![
            black_box(pat.clone()),
            Value::String(black_box(text.to_string())),
        ],
    )
    .expect("RE_SEARCH runs");
    match black_box(out.value) {
        Value::Null => None,
        v => Some(v),
    }
}

fn bench_regex(c: &mut Criterion) {
    let mut ev = Evaluator::new();
    let text = corpus(10_000);

    let mut group = c.benchmark_group("regex");
    group.throughput(Throughput::Bytes(text.len() as u64));

    // ① 常规线性:字符类扫描。这是「10 KB 语料 ≥ 20 MB/s」那条指标的读数。
    let digit = compile_pat(&mut ev, "[0-9]+");
    group.bench_function("re_search_digits_10kb", |b| {
        b.iter(|| black_box(search(&mut ev, &digit, &text)));
    });

    // ② 带捕获组与交替 —— 真实业务里更常见,也比纯字符类贵。
    let word = compile_pat(&mut ev, "([a-z]+)([0-9]+)");
    group.bench_function("re_search_groups_10kb", |b| {
        b.iter(|| black_box(search(&mut ev, &word, &text)));
    });

    // ③ 全量替换:同一趟扫描要走完捕获与拼接。
    group.bench_function("re_replace_groups_10kb", |b| {
        b.iter(|| {
            let out = wlwl_std::regex::re_replace(
                &mut ev,
                vec![
                    black_box(word.clone()),
                    Value::String(black_box(text.clone())),
                    Value::String("$2-$1".into()),
                ],
            )
            .expect("RE_REPLACE runs");
            black_box(out.value);
        });
    });
    group.finish();

    // ── 复杂度契约:病态输入 ──────────────────────────────────────────
    let mut group = c.benchmark_group("regex_pathological");
    let bomb = compile_pat(&mut ev, "(a*)*b");
    for n in [1_000usize, 10_000, 100_000] {
        let text = "a".repeat(n);
        group.throughput(Throughput::Bytes(n as u64));
        group.bench_function(format!("re_pathological_{n}a"), |b| {
            b.iter(|| black_box(search(&mut ev, &bomb, &text)));
        });
    }
    group.finish();

    // ── 编译成本:模式大小与**输入长度无关**(线性承诺的另一半) ────────
    let mut group = c.benchmark_group("regex_compile");
    for pat in ["abc", "(a*)*b", "[a-z]+([0-9]|[a-z])*"] {
        group.bench_function(format!("compile_{}", pat.len()), |b| {
            b.iter(|| {
                let out =
                    wlwl_std::regex::re(&mut ev, vec![Value::String(black_box(pat.to_string()))])
                        .expect("RE compiles");
                black_box(out.value);
            });
        });
    }
    group.finish();
}

criterion_group!(benches, bench_regex);
criterion_main!(benches);
