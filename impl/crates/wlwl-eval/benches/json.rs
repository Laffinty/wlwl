//! [v0.11.3 M7 / addendum-09 **W-06**] `std.json` 规模基准与**规模契约**。
//!
//! 口径:**release 直调 `StdFn`**,不经解释器(与 `kdf.rs` / `stats.rs` 段同款),
//! Windows MSVC,`[profile.bench] debug=true opt-level=3` / LTO 否。
//!
//! ```text
//! cargo bench -p wlwl-eval --bench json -- \
//!     --warm-up-time 1 --measurement-time 2 --sample-size 10
//! ```
//!
//! ## ⚠ 三条纪律(与 `stats.rs` 段同款,不是新规矩)
//!
//! 1. **语料要匹配最坏输入。** `stats.rs` 那段用固定种子 LCG 造**乱序**浮点,
//!    因为 pdqsort 对升序输入有快路径。本段没有那个陷阱(本段量的是解析与查表,
//!    不是排序),但**仍**不用 `RANGE` 造 JSON:那会造出全等差的整数数组,
//!    键名短、数字短,与真实响应差得远。⇒ 用固定种子 LCG 造**键名不等长、
//!    数值不等宽**的文档。
//! 2. **净成本要够大才相减。** `stats.rs` 段记过一次教训:净值只占原始读数 5.5%
//!    时噪声直接淹没净值(还减出「比小档更便宜」这种不可能的结果)。本段每组
//!    的成员成本都在总耗时的可观比例上,所以相减安全 —— 但落表时**仍标注
//!    net/raw**,让人能自己判断。
//! 3. **计时阈值不进 `cargo test`** —— 只记读数,阈值留在 `baseline.txt`。
//!
//! ## 两组,对着 `addendum-09` §4 的两条契约
//!
//! * `json_parse_*` —— **规模契约**:1 MB 上每字节成本与 1 KB 档**持平**
//!   (这是 00 治本之后第一次可以真正验的一条)。
//! * `json_get_*` —— §4 的「`JSON_GET` ≥ 100 MB/s」目标。
//!
//! ⚠ **口径提醒**:`JSON_GET` 那条的分子是「文档字节数」,分母是**成员一次调用的
//!   完整耗时**。而 `Value::Dict` 是线性扫描(`ADR-0025` Q1 裁定本批不动它),
//!   所以这一列**必然随文档变大而下降**。§4 那条 100 MB/s 应当按**小档**读,
//!   拿它去卡大档是把一个越大的数字越好的指标当成了常数。

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use wlwl_eval::Evaluator;
use wlwl_value::Value;

/// **字节**制 ×10 阶梯 —— 契约比的是「1 MB 档 vs 1 KB 档的每字节成本」,所以档位
/// **必须以字节计**,不能以元素/键计。
///
/// ⚠ 第一版这里写的是 `[1_000, 10_000, 100_000, 1_000_000]` **键数**,于是标着
///   「1 MB」的那一档实际是 ~19 MB。标签与实物不符的基准比没有基准更坏:它会让
///   「1 MB 档」这四个字在 `baseline.txt` 里指向一个错的数。⇒ 改成按目标字节数
///   生成文档,并在落表时同时记下**实际字节数**(语料是随机生成的,不会是整数 KB)。
const TIERS: [usize; 4] = [1_024, 10_240, 102_400, 1_048_576];

/// 确定性伪随机 —— 键名不等长、数值不等宽,接近真实响应。
struct Rng(u64);

impl Rng {
    fn next_u64(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 11
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next_u64() % n
    }
}

/// 造一个**扁平**的 JSON 对象,大小**约为** `target_bytes` 字节。
///
/// ⚠ **刻意扁平**:深嵌套会让 `JSON_GET` 的路径变长,而规模契约要量的是
///   **文档大小**这一个变量。深嵌套的形状由 `json_get_deep` 那一组单独测。
/// ⚠ 键长与数值宽度都抖动 ⇒ 平均项宽不是常数,所以「多少个键填满 N 字节」要
///   **先造出来再量**,不能拿 `N / 14` 估。
fn flat_doc(target_bytes: usize) -> String {
    let mut r = Rng(0x2545_F491_4F6C_DD1D);
    let mut s = String::with_capacity(target_bytes + 16);
    s.push('{');
    let mut i = 0usize;
    while s.len() < target_bytes {
        if i > 0 {
            s.push(',');
        }
        let pad = r.below(9);
        s.push_str("\"key");
        for _ in 0..pad {
            s.push((b'a' + (r.below(26) as u8)) as char);
        }
        s.push_str(&i.to_string());
        s.push_str("\":");
        s.push_str(&(r.below(1_000_000)).to_string());
        i += 1;
    }
    s.push('}');
    s
}

/// 一个**深**文档:`outer` 下挂 `depth` 层,每层一个 `next`。
/// 用来量「路径深度」这一个变量。
fn deep_doc(depth: usize) -> String {
    let mut s = String::from("{\"outer\":");
    for _ in 0..depth {
        s.push_str("{\"next\":");
    }
    s.push_str("\"leaf\"");
    for _ in 0..depth {
        s.push('}');
    }
    s.push('}');
    s
}

fn bench_parse(c: &mut Criterion) {
    let mut g = c.benchmark_group("json_parse");
    let mut ev = Evaluator::new();
    for &n in &TIERS {
        let text = flat_doc(n);
        g.throughput(Throughput::Bytes(text.len() as u64));
        // 同一份文本的 tokenize,用来把「词法」与「解析成值」两笔分开。
        g.bench_with_input(BenchmarkId::new("value", n), &n, |b, _| {
            b.iter(|| {
                let out = wlwl_std::json::std_parse(&mut ev, vec![Value::String(text.clone())])
                    .expect("parse");
                black_box(out);
            });
        });
        g.bench_with_input(BenchmarkId::new("tokenize", n), &n, |b, _| {
            b.iter(|| {
                let out = wlwl_std::json::json_tokenize(&mut ev, vec![Value::String(text.clone())])
                    .expect("tokenize");
                black_box(out);
            });
        });
        g.bench_with_input(BenchmarkId::new("validate", n), &n, |b, _| {
            b.iter(|| {
                let out = wlwl_std::json::json_validate(&mut ev, vec![Value::String(text.clone())])
                    .expect("validate");
                black_box(out);
            });
        });
    }
    g.finish();
}

fn bench_get(c: &mut Criterion) {
    let mut g = c.benchmark_group("json_get");
    let mut ev = Evaluator::new();
    // ⚠ 这里 `n` 是**键数**(不是字节):`JSON_GET` 的形状读数按「键数」归一
    //   更直观,而吞吐那一列仍然按 `Throughput::Bytes` 报。
    for &n in &[1_000usize, 10_000, 100_000] {
        let text = flat_doc(n * 14);
        g.throughput(Throughput::Bytes(text.len() as u64));
        // 取一个**靠后**的键 —— 最坏情形(线性扫描要走到底)。
        // 文档与路径都在 `b.iter` **之外**备好:计划 §4 那条「`JSON_GET` ≥ 100 MB/s」
        // 量的是**成员自身的成本**,把一次完整 `PARSE` 计进每次迭代会把它算到成员
        // 头上 —— 那是两个成员的钱。第一版就是这么写的。
        //
        // ⚠ **但入参的深拷贝留在循环里,而且必须留**:`JSON_GET(doc, path)` 按值收
        //   `doc`,而 wlwl 的 `Value` 是**深拷贝的属主树** ⇒ **每一次调用都要深拷贝
        //   整份文档**。这是调用方**真的**要付的钱(与 W-04 里 `JSON_SET` /
        //   `JSON_DELETE` 那笔同源)。减掉它等于报一个用户拿不到的数。
        let doc = wlwl_std::json::std_parse(&mut ev, vec![Value::String(text.clone())])
            .expect("parse")
            .value;
        let path = Value::Array(vec![Value::String(format!("key{}", n - 1))]);
        g.bench_with_input(BenchmarkId::new("last_key", n), &n, |b, _| {
            b.iter(|| {
                let out = wlwl_std::json::json_get(
                    &mut ev,
                    vec![black_box(doc.clone()), black_box(path.clone())],
                )
                .expect("get");
                black_box(out);
            });
        });
    }
    g.finish();
}

/// 路径**深度**这一个变量:深度 4 / 16 / 64,文档大小随之增长。
fn bench_get_deep(c: &mut Criterion) {
    let mut g = c.benchmark_group("json_get_deep");
    let mut ev = Evaluator::new();
    for &depth in &[4usize, 16, 64] {
        let text = deep_doc(depth);
        let mut path: Vec<Value> = vec![Value::String("outer".into())];
        for _ in 0..depth {
            path.push(Value::String("next".into()));
        }
        path.push(Value::String("leaf".into()));
        g.throughput(Throughput::Elements(depth as u64));
        let doc = wlwl_std::json::std_parse(&mut ev, vec![Value::String(text.clone())])
            .expect("parse")
            .value;
        let path_v = Value::Array(path);
        g.bench_with_input(BenchmarkId::from_parameter(depth), &depth, |b, _| {
            b.iter(|| {
                let out = wlwl_std::json::json_get(
                    &mut ev,
                    vec![black_box(doc.clone()), black_box(path_v.clone())],
                )
                .expect("get");
                black_box(out);
            });
        });
    }
    g.finish();
}

criterion_group!(benches, bench_parse, bench_get, bench_get_deep,);
criterion_main!(benches);
