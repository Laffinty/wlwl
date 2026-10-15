//! [v0.11.3 M7 / addendum-07 收尾] `std.compress` 规模基准 —— 补 README 门禁行
//! 登记的最后一处基准段缺口(`02` / `04` / `08` 都有,本段没有)。
//!
//! ## 为什么另起文件
//!
//! `encode.rs` 那一段量的是**吞吐指标**(§7.4 的 SHA-256 ≥ 100 MB/s),
//! 本段量的是**编解码器往返成本** —— `std.compress` 的成员是纯 R2 包装,
//! 成本几乎全在编解码器本体,不在语言层。两者不构成同一条曲线。
//!
//! ## 入参是 `ARRAY[INTEGER]`,不是 `STRING`
//!
//! ⚠ 写这个基准时我第一版按 `STRING` 喂,被实现当场拒掉:
//! `E0030 ZSTD_COMPRESS: expected a byte array of integers, got string`。
//! `compress.rs` 的 `bytes_of` 只认 `Value::Array` 且逐元素要
//! `0..=255` 的 `INTEGER` —— **静默截断会让「同一个载荷」有两种解释,
//! 字节面最不能容忍二义性**。这与 `std.fs` 的字节通道、`RANDOM_BYTES`
//! 同一口径:**`ARRAY[INTEGER]` 是本语言唯一无损的字节载体**。
//!
//! ## 用 `iter_batched`,把每轮的 `Value` 深拷贝**排除在计时之外**
//!
//! ⚠ 第一版用 `b.iter(|| member(vec![corpus.clone()]))`,16 k 个 `Value::Integer`
//! 的深拷贝**直接主导了读数** —— 实测 `clone_only` 对照组 144 µs,而整条
//! `compress_redundant/deflate_raw` 约 800 µs,拷贝占了 **18%**;更要命的是
//! 它还是**最大的噪声源**:同一份代码两轮之间 `compress_redundant/*` 摆动
//! **2.3 倍**(349 → 799 µs),而纯拷贝那组只摆 3%。
//!
//! 成员**只读**输入(`bytes_of` 先拷进 `Vec<u8>` 再干活,不改原值),
//! 所以 `iter_batched` 把「造输入」放进 setup 段是安全的,且这才是
//! 「这个成员有多贵」的正确答案。
//! ⇒ 顺带**去掉**了 `clone_only` 对照组:它测的是我们自己的测量装置,
//! 不是被测对象 —— 但这个发现已经写在这里,别再把它当读数的一部分。
//!
//! ## 这段**刻意不断言**压缩比,也不冻结输出字节
//!
//! 契约表已写明理由:同样载荷 Python 的 zlib 压到 66,而 `miniz_oxide`
//! 压到 **115**(**比原文还大**)。压缩比与输出字节都是**实现定义的**
//! (块划分 / lazy matching / 熵编码选择都自由)。正确性由
//! `compress_contract.rs` 的往返 + 交叉核对守,不进基准断言。
//!
//! ## 两种语料,缺一不可
//!
//! - `redundant` —— 高冗余自然语言。DEFLATE / zstd 的**字典**能吃到。
//! - `mixed` —— 固定种子 LCG 的确定性伪随机字节。**不可压缩**。
//!   ⚠ 只测 `redundant` 会让读数好看得没有意义:那测的是「熵已经没了」,
//!   而真实负载里熵编码那一段正是最贵的。只测 `mixed` 则测不到字典。
//!   **两者之比才是这一族成员的真实行为。**

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use wlwl_error::WlwlResult;
use wlwl_std::{Outcome, StdHost, Value};

/// 成员统一签名(与 `stats.rs` 同款)。
type Member = fn(&mut dyn StdHost, Vec<Value>) -> WlwlResult<Outcome>;

/// 直调成员需要 `StdHost`。`std.compress` 是**纯计算**(不碰宿主、不读文件),
/// 所以这个 `impl` 里任何一次 `ctx()` 调用都会 panic —— 与 `compress.rs`
/// 单测里的同名断言同款,基准因此**顺带**证明了「不经宿主」。
struct NullHost;

impl StdHost for NullHost {
    fn ctx(&mut self) -> &mut wlwl_std::StdCtx {
        panic!("std.compress must not touch StdCtx")
    }
    fn call(
        &mut self,
        _f: &Value,
        _args: Vec<Value>,
        _name: &str,
    ) -> wlwl_error::WlwlResult<Outcome> {
        panic!("std.compress has no member that calls back into the host")
    }
    fn diag(&mut self, code: wlwl_error::ErrorCode, message: String) -> wlwl_error::WlwlError {
        wlwl_error::WlwlDiagnostic::new(code, message, wlwl_error::Location::point("<bench>", 0, 0))
            .into()
    }
}

/// 四种格式的压缩成员 —— 名字即 `SPEC.functions` 的键。
const COMPRESSORS: [(&str, Member); 4] = [
    ("zstd", wlwl_std::compress::zstd_compress as Member),
    ("zlib", wlwl_std::compress::zlib_compress as Member),
    ("gzip", wlwl_std::compress::gzip_compress as Member),
    (
        "deflate_raw",
        wlwl_std::compress::deflate_raw_compress as Member,
    ),
];

/// 对应的四个解压成员。**顺序必须与 [`COMPRESSORS`] 一一对应** ——
/// 解压基准要用压缩的产物做输入,靠下标配对,名字对不上就在建输入时炸。
const DECOMPRESSORS: [(&str, Member); 4] = [
    ("zstd", wlwl_std::compress::zstd_decompress as Member),
    ("zlib", wlwl_std::compress::zlib_decompress as Member),
    ("gzip", wlwl_std::compress::gzip_decompress as Member),
    (
        "deflate_raw",
        wlwl_std::compress::deflate_raw_decompress as Member,
    ),
];

const CORPUS_BYTES: usize = 16 * 1024;

/// `ARRAY[INTEGER]` 语料 —— 高冗余自然语言,字典能吃到。
fn redundant() -> Value {
    let base = "wlwl compress: the quick brown fox jumps over the lazy dog. \
                 \u{4e16}\u{754c} h\u{e9}llo <tag>text</tag> a<b>&\"' ";
    let raw = base.as_bytes();
    Value::Array(
        (0..CORPUS_BYTES)
            .map(|i| Value::Integer(raw[i % raw.len()] as i64))
            .collect(),
    )
}

/// `ARRAY[INTEGER]` 语料 —— 固定种子伪随机**字节**,不可压缩。
/// ⚠ 固定种子:否则两次读数不可比(与 `stats.rs` 的 LCG 同款)。
fn mixed() -> Value {
    let mut st: u64 = 0x2545_F491_4F6C_DD1D;
    Value::Array(
        (0..CORPUS_BYTES)
            .map(|_| {
                st = st
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1_442_695_040_888_963_407);
                Value::Integer(((st >> 33) % 256) as i64)
            })
            .collect(),
    )
}

/// 从成员的返回值里取载荷数组 —— 失败(ERR / 上限)在这里就炸掉。
///
/// ⚠ **不要**只断 `Result` 就当成功:这些成员**失败时也返回 `Ok`**,
/// 只是载荷是 `ERR` 值(契约表的「损坏输入返回 ERR」)。一条建立在
/// ERR 载荷上的读数没有意义,所以这里显式断载荷形状。
fn array_of(out: Outcome, name: &str) -> Value {
    match out.value {
        Value::Array(a) => Value::Array(a),
        other => panic!("{name} did not return ARRAY[INTEGER], got {other:?}"),
    }
}

/// 对照组:只克隆语料、不调成员。
///
/// ⚠ 它测的是**我们自己的测量装置**,不是被测对象 —— 保留它是为了让
/// 「`iter_batched` 排除了多少」这件事有据可查,不是拿它减成员读数。
fn bench_clone_only(c: &mut Criterion) {
    let mut group = c.benchmark_group("compress_control");
    let input = redundant();
    group.throughput(Throughput::Bytes(CORPUS_BYTES as u64));
    group.bench_function("clone_only_redundant", |b| {
        b.iter(|| black_box(input.clone()))
    });
    let input = mixed();
    group.bench_function("clone_only_mixed", |b| b.iter(|| black_box(input.clone())));
    group.finish();
}

fn bench_compress(c: &mut Criterion) {
    let mut host = NullHost;
    for (corpus_name, corpus) in [("redundant", redundant()), ("mixed", mixed())] {
        let mut group = c.benchmark_group(format!("compress_{corpus_name}"));
        group.throughput(Throughput::Bytes(CORPUS_BYTES as u64));
        for (name, member) in COMPRESSORS {
            group.bench_function(name, |b| {
                b.iter_batched(
                    || vec![corpus.clone()],
                    |args| {
                        let out = member(&mut host, args)
                            .unwrap_or_else(|e| panic!("{name} returned Err: {e}"));
                        black_box(array_of(out, name))
                    },
                    criterion::BatchSize::SmallInput,
                )
            });
        }
        group.finish();
    }
}

fn bench_decompress(c: &mut Criterion) {
    let mut host = NullHost;
    // 解压的输入 = 压缩的产物,**建在计时循环之外**(它是基准输入,
    // 不是被测对象 —— 把它算进去就是把两次编解码混成一条读数)。
    let corpus = redundant();
    let packed: Vec<Value> = COMPRESSORS
        .iter()
        .zip(DECOMPRESSORS.iter())
        .map(|((cname, cfn), (dname, _dfn))| {
            assert_eq!(cname, dname, "COMPRESSORS / DECOMPRESSORS out of order");
            cfn(&mut host, vec![corpus.clone()])
                .map(|out| array_of(out, cname))
                .unwrap_or_else(|e| panic!("{cname} returned Err: {e}"))
        })
        .collect();

    let mut group = c.benchmark_group("compress_decompress");
    // 解压的输入是**压缩后**的字节,吞吐按原文口径报(CORPUS_BYTES),
    // 否则同一个「MiB/s」在压缩与解压两组之间不可比。
    group.throughput(Throughput::Bytes(CORPUS_BYTES as u64));
    for ((dname, dfn), input) in DECOMPRESSORS.iter().zip(packed.iter()) {
        let dname = *dname;
        group.bench_function(dname, |b| {
            b.iter_batched(
                || vec![input.clone()],
                |args| {
                    let out = dfn(&mut host, args)
                        .unwrap_or_else(|e| panic!("{dname} returned Err: {e}"));
                    black_box(array_of(out, dname))
                },
                criterion::BatchSize::SmallInput,
            )
        });
    }
    group.finish();
}

criterion_group!(benches, bench_clone_only, bench_compress, bench_decompress);
criterion_main!(benches);
