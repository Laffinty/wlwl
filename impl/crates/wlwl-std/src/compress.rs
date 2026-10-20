//! [v0.11.3 M6 / addendum-07] `wlwl:std.compress` —— Zstandard 与 DEFLATE 家族。
//!
//! **R2 全员**。形态理由(addendum-07 §3.1 / §3.2):
//!
//! * **为什么引 crate 而不手写**:zstd 压缩端(FSE / Huffman / 块划分)手写是周级
//!   工作量且正确性极难自证;而自研 DEFLATE **解压端**是压缩炸弹 / OOB / 无界
//!   分配的第一现场 —— 本文件自己那条「解压输出上限」规范条款就说明这面攻击是
//!   真的 ⇒ 最不该自担的一类。许可证已实测放行(`deny.toml` 未改)。
//! * **为什么独立成命名空间**:`std.encode` 是「文本编解码 + 摘要」,成员都是
//!   **STRING 进 STRING 出**;压缩是**字节流**,塞进去会破掉那条隐含契约。
//! * **字节形状继承 `addendum-06` §3.1(b)**,本模块**不重新裁决** —— 两份各定一次
//!   就是 std 里两种字节表示,那是最难还的债。
//!
//! ## ⚠️ 压缩**不是加密**
//!
//! 规范明写这一条(与 §11.5「哈希不是加密」同款体例)。压缩不提供机密性:任何
//! 人拿到输入都能解开。调用方若把它当「藏起来」用,那是安全缺陷不是用法。
//! 机密性走 `std.encode` 之外的手段(传输层加密)。

use crate::{ModuleSpec, StdFn};
use wlwl_error::ErrorCode;
use wlwl_error::WlwlError;
use wlwl_value::{Outcome, StdHost, Value};

/// **解压输出上限** —— 规范性条款,不是实现里的一个可调 `const`。
///
/// 压缩炸弹(几十字节压缩出几 GB)是**真实攻击面**:不设上限就是「把调用方的
/// 内存当作攻击者的杠杆」。越限返回 `ERR([kind: "CompressError", op, reason])`,
/// **不是** OOM、**不是** panic。
///
/// 64 MiB 的来历:够放下真实产物 / 测试向量 / 一次 HTTP 载荷,又小到「炸不动」。
/// ⚠️ **它也是本批的一条已知限制**:本批明确**不做流式 API**,所以 > 64 MiB 的
/// 单个载荷**没有解压路径** —— 想处理它得先解压到文件,或等流式成员落地。
pub const MAX_DECOMPRESSED: usize = 64 * 1024 * 1024;

/// 定步读取解压输出,让 `Vec` 的 **`capacity` 与 `len` 同阶**(D13-074)。
///
/// ⚠ **缺陷形态**:`Read::take(cap).read_to_end(&mut out)` —— `read_to_end` 是
///   **倍增**增长(Rust 的实现按 `reserve` 翻倍)⇒ 一个**恰好** 64 MiB 的合法
///   载荷会让 `capacity` 涨到 ~128 MiB ⇒ **实测峰值约为文档上限的 2×**。
///   而规范 §20.2 承诺的是「超过 64 MiB 返 `ERR`,**不是** OOM」——
///   峰值 2× 在内存受限环境里**就是** OOM 的前置条件。
/// ⚠ **修法不是「读前预分配」**:那会让**每一次**解压都先占 64 MiB,
///   比现在更坏(规范 §20.2 已记下这条否决理由)。
///   ⇒ 改为**定步**:每次读固定一块,每读一块就**精确**增长那一块,
///   于是 `capacity` 始终 == `len` ⇒ 峰值与上限**同阶**。
/// ⚠ **实测(2026-10-20,负向验证)**:缺陷形态下 4 MiB 的载荷拿到
///   **8 MiB** 的 `capacity` —— 正好 2× ⇒ 规范 §20.2 那句「实测峰值约为
///   文档上限的 2×」由此从推测变成**量出来的数**。
/// ⚠ 用 `try_reserve_exact` 而不是 `reserve`:后者**会**为摊还而多要一倍;
///   前者只多要「这一块」,且分配失败可以变成一个 `io::Error`(**不能** panic)
///   —— 「越限不是 OOM」这条承诺的前提之一就是**不把进程打死**。
///
/// 最多读 `cap + 1` 字节:多一个就说明**超了**(与旧形态的 `take(cap + 1)`
/// 同一口径,调用方看 `len > cap` 即可判越限)。
fn read_capped_exact(r: &mut impl Read, cap: usize) -> std::io::Result<Vec<u8>> {
    const CHUNK: usize = 64 * 1024;
    let mut out: Vec<u8> = Vec::new();
    let mut buf = [0u8; CHUNK];
    let limit = cap + 1;
    while out.len() < limit {
        let want = CHUNK.min(limit - out.len());
        let n = r.read(&mut buf[..want])?;
        if n == 0 {
            break;
        }
        out.try_reserve_exact(n)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::OutOfMemory, e.to_string()))?;
        out.extend_from_slice(&buf[..n]);
    }
    Ok(out)
}

pub static SPEC: ModuleSpec = ModuleSpec {
    path: "wlwl:std.compress",
    functions: &[
        ("ZSTD_COMPRESS", zstd_compress as StdFn),
        ("ZSTD_DECOMPRESS", zstd_decompress as StdFn),
        ("ZLIB_COMPRESS", zlib_compress as StdFn),
        ("ZLIB_DECOMPRESS", zlib_decompress as StdFn),
        ("GZIP_COMPRESS", gzip_compress as StdFn),
        ("GZIP_DECOMPRESS", gzip_decompress as StdFn),
        ("DEFLATE_RAW_COMPRESS", deflate_raw_compress as StdFn),
        ("DEFLATE_RAW_DECOMPRESS", deflate_raw_decompress as StdFn),
    ],
};

// ── 助手 ───────────────────────────────────────────────────────────────

fn arity(host: &mut dyn StdHost, name: &str, got: usize, want: usize) -> WlwlError {
    host.diag(
        ErrorCode::E0022,
        format!("{name}: function expects {want} argument(s), got {got}"),
    )
}

fn type_err(host: &mut dyn StdHost, name: &str, want: &str, got: &Value) -> WlwlError {
    host.diag(
        ErrorCode::E0030,
        format!("{name}: expected {want}, got {}", crate::value_kind(got)),
    )
}

fn ok(v: Value) -> Result<Outcome, WlwlError> {
    Ok(Outcome::normal(v))
}

fn err(op: &str, reason: impl Into<String>) -> Value {
    Value::Err(Box::new(Value::Dict(vec![
        (
            Value::String("kind".into()),
            Value::String("CompressError".into()),
        ),
        (Value::String("op".into()), Value::String(op.into())),
        (Value::String("reason".into()), Value::String(reason.into())),
    ])))
}

/// `ARRAY[INTEGER]`(元素 `0..=255`)⇄ `Vec<u8>`。形状由 `addendum-06` 定,
/// 本模块只做校验,**不重新裁决**。
fn bytes_of(host: &mut dyn StdHost, name: &str, v: &Value) -> Result<Vec<u8>, WlwlError> {
    let Value::Array(items) = v else {
        return Err(type_err(host, name, "a byte array of integers", v));
    };
    let mut out = Vec::with_capacity(items.len());
    for it in items {
        let Value::Integer(b) = it else {
            return Err(type_err(host, name, "a byte array of integers", it));
        };
        // 静默截断会让「同一个载荷」有两种解释 —— 字节面最不能容忍二义性。
        if !(0..=255).contains(b) {
            return Err(host.diag(
                ErrorCode::E0030,
                format!("{name}: byte {b} is outside 0..=255"),
            ));
        }
        out.push(*b as u8);
    }
    Ok(out)
}

fn to_array(bytes: Vec<u8>) -> Value {
    Value::Array(
        bytes
            .into_iter()
            .map(|b| Value::Integer(i64::from(b)))
            .collect(),
    )
}

/// `level?` —— 缺省 = **库默认**,**默认不是推荐值**(与 `std.encode` 的 KDF
/// 参数同款立场:写清「不写就拿默认,想要特定权衡就显式写」)。
///
/// 越界 → `E0030`,**不静默 clamp** —— clamp 会让「我传了 22」和「我传了 999」
/// 产出同一个结果,那是在骗调用方。
fn level_of(
    host: &mut dyn StdHost,
    name: &str,
    v: Option<&Value>,
    lo: i32,
    hi: i32,
) -> Result<Option<i32>, WlwlError> {
    match v {
        // ⚠🔴 **[D13-026 / N-012] 缺省必须返回 `None` 而不是 `Some(0)`。**
        // 「`0` 恰好等于各库的缺省」只对 **zstd** 成立(zstd 的
        // `ZSTD_c_compressionLevel = 0` 明确定义为「用库默认」),对
        // **flate2** 恰好相反:`Compression::new(0)` 是 **stored 块 ——
        // 即「完全不压缩」**。所以统一返回 `0` 等于给 zlib / gzip /
        // deflate 三条线装了个「缺省 = 不压」,而本函数的文档写的是
        // 「缺省 = 库默认」。
        //
        // ⚠ 自证就在自家契约里:`compress_contract.rs` 的 `every_format_
        // round_trips` 注记着「同样的 104 字节载荷,Python 的 zlib 压到 66,
        // 而 miniz_oxide 压到 **115**(比原文还大)」—— 那个 115 正是
        // stored 块的形状(104 + 11 字节容器开销)。当时它被归因成「压缩比
        // 是实现定义的」,于是这条 bug 被**写进了契约当规范**。那几行注释
        // 已同步改正 —— 错误的归因留着,下一个人还会读到同一个误导。
        None | Some(Value::Null) => Ok(None),
        Some(Value::Integer(n)) => {
            let n = i32::try_from(*n).unwrap_or(i32::MAX);
            if n < lo || n > hi {
                return Err(host.diag(
                    ErrorCode::E0030,
                    format!("{name}: level {n} is outside {lo}..={hi}"),
                ));
            }
            Ok(Some(n))
        }
        Some(other) => Err(type_err(host, name, "a level integer", other)),
    }
}

/// 把「调用方给没给 `level`」翻译成 **flate2 的压缩级别**。
///
/// ⚠ 缺省走 [`flate2::Compression::default()`](flate2::Compression) 而不是
/// `new(0)` —— 见 `level_of` 里 D13-026 那段论证。
#[must_use]
fn flate_level(lvl: Option<i32>) -> flate2::Compression {
    lvl.map_or_else(flate2::Compression::default, |n| {
        flate2::Compression::new(n as u32)
    })
}

/// 元数:1 或 2(`data`, `level?`)。
fn arity_1_or_2(host: &mut dyn StdHost, name: &str, got: usize) -> Result<(), WlwlError> {
    if got == 1 || got == 2 {
        Ok(())
    } else {
        Err(arity(host, name, got, 1))
    }
}

/// 把 `io::Error` 翻译成 `CompressError`。⚠️ 「解压输出超过上限」是其中
/// **唯一**由我们自己制造的那一种,措辞单独给它,便于调用方区分「数据坏了」
/// 与「载荷太大」。
fn io_err(op: &str, e: std::io::Error, hit_limit: bool) -> Value {
    if hit_limit {
        err(
            op,
            format!(
                "decompressed output exceeds the {MAX_DECOMPRESSED}-byte limit \
                 (compression bomb guard; this build does not stream)"
            ),
        )
    } else {
        err(op, e.to_string())
    }
}

// ── Zstandard (RFC 8878) ───────────────────────────────────────────────

/// `ZSTD_COMPRESS(data, level?) -> ARRAY[INTEGER]`
///
/// `level` 缺省取 **zstd 库默认**(3);范围 `-131072..=22`(负数 = 更快、压缩比更低)。
/// 越界 → `E0030`。
///
/// **确定性**:同一 `(data, level)` 两次压缩产出**逐字节相同** —— 契约表有向量钉它。
pub fn zstd_compress(host: &mut dyn StdHost, args: Vec<Value>) -> Result<Outcome, WlwlError> {
    const NAME: &str = "ZSTD_COMPRESS";
    arity_1_or_2(host, NAME, args.len())?;
    let data = bytes_of(host, NAME, &args[0])?;
    let level = level_of(host, NAME, args.get(1), -131_072, 22)?;
    // ⚠ zstd 的 `compression_level = 0` **确实**定义为「用库默认(3)」,所以这条
    //   线 `unwrap_or(0)` 是对的 —— 与 flate2 的「0 = 不压」正好相反,见 D13-026。
    match zstd::bulk::compress(&data, level.unwrap_or(0)) {
        Ok(out) => ok(to_array(out)),
        Err(e) => ok(err(NAME, format!("compress: {e}"))),
    }
}

/// `ZSTD_DECOMPRESS(data) -> ARRAY[INTEGER]`
///
/// ⚠ 输出超过 [`MAX_DECOMPRESSED`] → `ERR`(压缩炸弹护栏),**不是** OOM。
pub fn zstd_decompress(host: &mut dyn StdHost, args: Vec<Value>) -> Result<Outcome, WlwlError> {
    const NAME: &str = "ZSTD_DECOMPRESS";
    if args.len() != 1 {
        return Err(arity(host, NAME, args.len(), 1));
    }
    let data = bytes_of(host, NAME, &args[0])?;
    if data.is_empty() {
        return ok(err(NAME, "empty input is not a zstd frame"));
    }
    // ⚠ **不用** `zstd::bulk::decompress`:它按帧头声明的长度一次性分配,
    // 那是把调用方的内存交给攻击者。走流式解码 + `take` 上限。
    let reader = std::io::Cursor::new(data);
    let mut dec = match zstd::stream::read::Decoder::new(reader) {
        Ok(d) => d,
        Err(e) => return ok(err(NAME, format!("not a zstd frame: {e}"))),
    };
    let read = read_capped_exact(&mut dec, MAX_DECOMPRESSED);
    match read {
        Ok(out) if out.len() as u64 > MAX_DECOMPRESSED as u64 => {
            ok(io_err(NAME, std::io::Error::other("limit"), true))
        }
        Ok(out) => ok(to_array(out)),
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
            ok(err(NAME, format!("truncated frame: {e}")))
        }
        Err(e) => ok(err(NAME, format!("decompress: {e}"))),
    }
}

// ── zlib (RFC 1950) / gzip (RFC 1952) / raw DEFLATE (RFC 1951) ──────────
//
// 三条线共用同一套「压缩 / 解压 + 上限」逻辑,只有容器不同 ⇒ 一份模板,
// 三个薄封装。**zlib 与 gzip 的校验和因此必然被验证**(flate2 的解码器会核对
// Adler-32 / CRC32),这正是「损坏输入 → ERR 而不是 panic」这条契约的基础。

use std::io::{Read, Write};

fn flate_compress(
    host: &mut dyn StdHost,
    name: &'static str,
    args: Vec<Value>,
    level: (i32, i32),
    build: fn(&[u8], Option<i32>) -> std::io::Result<Vec<u8>>,
) -> Result<Outcome, WlwlError> {
    arity_1_or_2(host, name, args.len())?;
    let data = bytes_of(host, name, &args[0])?;
    let lvl = level_of(host, name, args.get(1), level.0, level.1)?;
    match build(&data, lvl) {
        Ok(out) => ok(to_array(out)),
        Err(e) => ok(err(name, format!("compress: {e}"))),
    }
}

fn flate_decompress(
    host: &mut dyn StdHost,
    name: &'static str,
    args: Vec<Value>,
    build: fn(Vec<u8>) -> std::io::Result<Box<dyn Read + Send>>,
) -> Result<Outcome, WlwlError> {
    if args.len() != 1 {
        return Err(arity(host, name, args.len(), 1));
    }
    let data = bytes_of(host, name, &args[0])?;
    if data.is_empty() {
        return ok(err(name, "empty input"));
    }
    // ⚠ 数据**按值**交给解码器:解码器必须**拥有**它,不能借用一个就要
    // 返回的局部 `Vec`(否则 `Box<dyn Read>` 活得比它短)。
    let mut reader = match build(data) {
        Ok(r) => r,
        Err(e) => return ok(err(name, format!("bad header: {e}"))),
    };
    // `read_capped_exact(LIMIT)` 同时做到两件事:把内存**封顶**在 LIMIT + 1
    // (D13-074:定步增长,`capacity` 与 `len` 同阶,不再翻倍),
    // 以及「读满了 LIMIT + 1 就知道超了」。
    let read = read_capped_exact(&mut reader, MAX_DECOMPRESSED);
    match read {
        Ok(out) if out.len() > MAX_DECOMPRESSED => {
            ok(io_err(name, std::io::Error::other("limit"), true))
        }
        Ok(out) => ok(to_array(out)),
        Err(e) => ok(err(name, format!("decompress: {e}"))),
    }
}

/// `ZLIB_COMPRESS(data, level?) -> ARRAY[INTEGER]` —— RFC 1950(zlib 包装,Adler-32)。
pub fn zlib_compress(host: &mut dyn StdHost, args: Vec<Value>) -> Result<Outcome, WlwlError> {
    flate_compress(host, "ZLIB_COMPRESS", args, (0, 9), |data, lvl| {
        let mut e = flate2::write::ZlibEncoder::new(Vec::new(), flate_level(lvl));
        e.write_all(data)?;
        e.finish()
    })
}

/// `ZLIB_DECOMPRESS(data) -> ARRAY[INTEGER]`。⚠ 校验和不符 → `ERR`。
pub fn zlib_decompress(host: &mut dyn StdHost, args: Vec<Value>) -> Result<Outcome, WlwlError> {
    flate_decompress(host, "ZLIB_DECOMPRESS", args, |data| {
        Ok(Box::new(flate2::read::ZlibDecoder::new(
            std::io::Cursor::new(data),
        )))
    })
}

/// `GZIP_COMPRESS(data, level?) -> ARRAY[INTEGER]` —— RFC 1952(CRC32 + mtime=0)。
pub fn gzip_compress(host: &mut dyn StdHost, args: Vec<Value>) -> Result<Outcome, WlwlError> {
    flate_compress(host, "GZIP_COMPRESS", args, (0, 9), |data, lvl| {
        let mut e = flate2::write::GzEncoder::new(Vec::new(), flate_level(lvl));
        e.write_all(data)?;
        e.finish()
    })
}

/// `GZIP_DECOMPRESS(data) -> ARRAY[INTEGER]`。⚠ CRC32 不符 → `ERR`。
///
/// ⚠🔴 **[D13-048 / N-027] 这里必须是 `MultiGzDecoder`,不是 `GzDecoder`。**
///
/// **RFC 1952 §2.2** —— 一个 gzip 文件是**若干成员首尾相接**,解压结果是它们
/// 解出内容的拼接(`cat a.gz b.gz > ab.gz` 是合法构造)。`GzDecoder` 只吃
/// **第一个**成员就返回 EOF ⇒ 第二个成员被**静默丢弃**、不报错。那与本成员
/// 文档里「⚠ CRC32 不符 → `ERR`」给人的完整性承诺直接相悖:一个「校验和被
/// 验证过」的返回值,实际只覆盖了输入的一部分。
///
/// **尾随字节(流后多出来的垃圾)**:换 `MultiGzDecoder` 之后它同样**不再静默**
/// —— 解完一个成员后 flate2 会去看还有没有下一个成员,垃圾头解析失败 ⇒ 整条
/// 解压返回错误 ⇒ 本函数返 `ERR`。
/// ⚠ **本条是实测的**(flate2 1.1.10 + Python 3.13.13),不是推断:
/// - 多成员(两个 `gzip.compress(b"AAA")` / `b"BBB"`)⇒ 解出 `[65,65,65,66,66,66]`;
/// - 成员后接 16 字节垃圾 ⇒ `Err(InvalidInput)`;Python `gzip.decompress` 对同一
///   输入抛 `BadGzipFile` —— 两个独立实现在这里同判。
/// - ⚠🔴 **审查报告里建议的「校验 `Cursor::position() == len`」是无效的**:
///   `flate2::read::` 的解码器内部套了 `BufReader`,会**预读**,实测尾随垃圾
///   情形下 `position()` 仍等于 `len`(38 == 38)。靠它去抓尾随字节会得到一个
///   永远绿的守卫。抓住它的是**解码器自己的错误**,不是位置。
///
/// ⚠ 为什么 `ZLIB_DECOMPRESS` / `DEFLATE_RAW_DECOMPRESS` **不加**尾随字节检查:
/// RFC 1950 的 zlib 流没有「多成员」这个概念,zlib API 把它叫 `unused_data`
/// 并**允许**调用方忽略;Python `zlib.decompress(zlib_stream + b"JUNK")` 同样
/// 正常返回数据(实测)。gzip 那条之所以要严,是因为 RFC 1952 **定义了**成员
/// 拼接语义 ⇒ 尾随字节要么是第二个成员(该解),要么是垃圾(该报)。
pub fn gzip_decompress(host: &mut dyn StdHost, args: Vec<Value>) -> Result<Outcome, WlwlError> {
    flate_decompress(host, "GZIP_DECOMPRESS", args, |data| {
        Ok(Box::new(flate2::read::MultiGzDecoder::new(
            std::io::Cursor::new(data),
        )))
    })
}

/// `DEFLATE_RAW_COMPRESS(data, level?) -> ARRAY[INTEGER]` —— RFC 1951 **裸流**
/// (无容器、无校验和),给自定义容器用。
pub fn deflate_raw_compress(
    host: &mut dyn StdHost,
    args: Vec<Value>,
) -> Result<Outcome, WlwlError> {
    flate_compress(host, "DEFLATE_RAW_COMPRESS", args, (0, 9), |data, lvl| {
        let mut e = flate2::write::DeflateEncoder::new(Vec::new(), flate_level(lvl));
        e.write_all(data)?;
        e.finish()
    })
}

/// `DEFLATE_RAW_DECOMPRESS(data) -> ARRAY[INTEGER]`。
///
/// ⚠ 裸流**没有校验和** ⇒ 「数据损坏」在这里**不可检测**,截断的输入可能解出
/// 一段**前缀正确**的垃圾而不报错。这是格式本身的性质,不是实现的偷懒;
/// 需要完整性保证就用 zlib(Adler-32)或 gzip(CRC32)。
pub fn deflate_raw_decompress(
    host: &mut dyn StdHost,
    args: Vec<Value>,
) -> Result<Outcome, WlwlError> {
    flate_decompress(host, "DEFLATE_RAW_DECOMPRESS", args, |data| {
        Ok(Box::new(flate2::read::DeflateDecoder::new(
            std::io::Cursor::new(data),
        )))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use wlwl_value::StdCtx;

    /// `std.compress` 是**纯计算**(不碰宿主、不读文件)⇒ `ctx()` panic 是
    /// 有意的断言,不是占位。
    struct NullHost;

    impl StdHost for NullHost {
        fn ctx(&mut self) -> &mut StdCtx {
            panic!("std.compress must not touch StdCtx")
        }
        fn call(&mut self, _f: &Value, _a: Vec<Value>, _n: &str) -> Result<Outcome, WlwlError> {
            panic!("std.compress has no member that calls back into the host")
        }
        fn diag(&mut self, code: ErrorCode, message: String) -> WlwlError {
            wlwl_error::WlwlDiagnostic::new(
                code,
                message,
                wlwl_error::Location::point("<test>", 0, 0),
            )
            .into()
        }
    }

    fn arr(v: &[u8]) -> Value {
        to_array(v.to_vec())
    }

    fn back(out: &Value) -> Vec<u8> {
        let Value::Array(items) = out else {
            panic!("expected an array, got {out:?}")
        };
        items
            .iter()
            .map(|v| match v {
                Value::Integer(b) => {
                    assert!((0..=255).contains(b), "byte {b} outside 0..=255");
                    *b as u8
                }
                other => panic!("non-integer element {other:?}"),
            })
            .collect()
    }

    fn payload() -> Vec<u8> {
        b"wlwl compress: the quick brown fox jumps over the lazy dog. \
           the quick brown fox jumps over the lazy dog."
            .to_vec()
    }

    /// 四条格式的**往返**:每条都必须还原出**逐字节相同**的输入。
    ///
    /// ⚠ 压缩与解压**分两步**写,不写成嵌套表达式 —— 嵌套会让 `&mut h` 被借
    /// 两次(编译期借用检查直接拒)。
    #[test]
    fn all_four_formats_round_trip() {
        let mut h = NullHost;
        let d = payload();

        let zstd_c = zstd_compress(&mut h, vec![arr(&d)]).expect("zstd").value;
        let zstd_d = zstd_decompress(&mut h, vec![zstd_c.clone()])
            .expect("zstd d")
            .value;
        assert_eq!(back(&zstd_d), d, "zstd must round-trip byte-exactly");
        assert_ne!(zstd_c, arr(&d), "zstd: nothing was compressed");

        let zlib_c = zlib_compress(&mut h, vec![arr(&d)]).expect("zlib").value;
        let zlib_d = zlib_decompress(&mut h, vec![zlib_c.clone()])
            .expect("zlib d")
            .value;
        assert_eq!(back(&zlib_d), d, "zlib must round-trip byte-exactly");
        assert_ne!(zlib_c, arr(&d), "zlib: nothing was compressed");

        let gz_c = gzip_compress(&mut h, vec![arr(&d)]).expect("gzip").value;
        let gz_d = gzip_decompress(&mut h, vec![gz_c.clone()])
            .expect("gzip d")
            .value;
        assert_eq!(back(&gz_d), d, "gzip must round-trip byte-exactly");
        assert_ne!(gz_c, arr(&d), "gzip: nothing was compressed");

        let raw_c = deflate_raw_compress(&mut h, vec![arr(&d)])
            .expect("raw")
            .value;
        let raw_d = deflate_raw_decompress(&mut h, vec![raw_c.clone()])
            .expect("raw d")
            .value;
        assert_eq!(back(&raw_d), d, "deflate-raw must round-trip byte-exactly");
        assert_ne!(raw_c, arr(&d), "deflate-raw: nothing was compressed");
    }

    /// **确定性**:同输入两次压缩产出**逐字节相同**。这是本语言的卖点之一,
    /// 也是契约表冻向量的依据。
    #[test]
    fn compression_is_deterministic() {
        let mut h = NullHost;
        let d = payload();
        assert_eq!(
            zstd_compress(&mut h, vec![arr(&d)]).unwrap().value,
            zstd_compress(&mut h, vec![arr(&d)]).unwrap().value
        );
        assert_eq!(
            gzip_compress(&mut h, vec![arr(&d)]).unwrap().value,
            gzip_compress(&mut h, vec![arr(&d)]).unwrap().value
        );
        assert_eq!(
            zlib_compress(&mut h, vec![arr(&d)]).unwrap().value,
            zlib_compress(&mut h, vec![arr(&d)]).unwrap().value
        );
        assert_eq!(
            deflate_raw_compress(&mut h, vec![arr(&d)]).unwrap().value,
            deflate_raw_compress(&mut h, vec![arr(&d)]).unwrap().value
        );
    }

    /// 真压缩比:重复串必须被压小,否则「压缩」这个词是空的。
    ///
    /// ⚠🔴 **[D13-026 / N-012]这条此前只测 `zstd`** —— 而 zstd 恰好是**唯一没坏**
    /// 的那一条(它的 `level 0` 就是库默认)。真正坏掉的 zlib / gzip /
    /// deflate 三条线当时**零覆盖**,于是「缺省 = 不压缩」这条 bug 从缝里漏过去,
    /// 还被 `compress_contract` 记成「压缩比是实现定义的」。现在四条线都测。
    #[test]
    fn repetitive_input_actually_shrinks() {
        let d = "ab".repeat(20_000).into_bytes();
        for (name, out) in [
            (
                "zstd",
                zstd_compress(&mut NullHost, vec![arr(&d)]).expect("zstd"),
            ),
            (
                "zlib",
                zlib_compress(&mut NullHost, vec![arr(&d)]).expect("zlib"),
            ),
            (
                "gzip",
                gzip_compress(&mut NullHost, vec![arr(&d)]).expect("gzip"),
            ),
            (
                "deflate",
                deflate_raw_compress(&mut NullHost, vec![arr(&d)]).expect("deflate"),
            ),
        ] {
            let c = back(&out.value).len();
            assert!(
                c * 50 < d.len(),
                "{name} at the **default** level: 40 KB of \"ab\" should compress far below \
                 1 KB, got {c} bytes — a default that stores is not compression"
            );
        }
    }

    /// ⚠ **缺省级别 ≠ `0`。** zstd 的 `0` 是「库默认」,flate2 的 `0` 是
    /// 「stored / 完全不压」—— 两者的 `0` 含义相反,这正是 D13-026 的根因。
    /// 显式传 `0` 时三条 flate 线**应当**就是 stored(那是调用方的选择),
    /// 本测试只钉「缺省 ≠ 0」这一条,免得有人把两条路又合并回去。
    #[test]
    fn explicit_level_zero_is_stored_but_the_default_is_not() {
        let mut h = NullHost;
        let d = "ab".repeat(20_000).into_bytes();
        let default_len = back(
            &gzip_compress(&mut NullHost, vec![arr(&d)])
                .expect("gzip")
                .value,
        )
        .len();
        let zero_len = back(
            &gzip_compress(&mut h, vec![arr(&d), Value::Integer(0)])
                .expect("gzip level 0")
                .value,
        )
        .len();
        assert!(
            zero_len >= d.len(),
            "explicit level 0 should be stored ({zero_len} >= {})",
            d.len()
        );
        assert!(
            default_len * 2 < zero_len,
            "the default level must actually compress: default {default_len} vs level-0 {zero_len}"
        );
    }

    /// **损坏输入必须返回 `ERR`,绝不能 panic。**
    ///
    /// 两条**分开的**断言,因为它们的真伪强度不同:
    ///
    /// 1. **每一个**字节翻转都**不许 panic**(只要求「有产出」);
    /// 2. **尾部校验和区**的字节翻转**必须**被抓住。
    ///
    /// ⚠️ 第 2 条**不能**推广到整个帧:容器**头部**的字节是元数据
    /// (gzip 的 MTIME / FLG / OS、zlib 的 CMF/FLG),翻它们仍可能得到一个
    /// **合法**的帧 —— 我第一版就是这么写的,结果 `gzip byte 3` 翻完照样解开,
    /// 断言当场变红。**「每个字节翻一下都该失败」是错的断言**,不是实现漏了校验。
    #[test]
    fn corrupt_input_never_panics_and_the_checksum_is_verified() {
        let mut h = NullHost;
        let d = payload();
        for (name, good, decode) in [
            (
                "zlib",
                back(&zlib_compress(&mut h, vec![arr(&d)]).unwrap().value),
                zlib_decompress as fn(&mut dyn StdHost, Vec<Value>) -> Result<Outcome, WlwlError>,
            ),
            (
                "gzip",
                back(&gzip_compress(&mut h, vec![arr(&d)]).unwrap().value),
                gzip_decompress as fn(&mut dyn StdHost, Vec<Value>) -> Result<Outcome, WlwlError>,
            ),
        ] {
            for i in 0..good.len() {
                let mut bad = good.clone();
                bad[i] = bad[i].wrapping_add(1);
                // ① 不许 panic:返回值错误也算通过这一层。
                let _ = decode(&mut h, vec![arr(&bad)])
                    .unwrap_or_else(|e| panic!("{name}: byte {i} raised a diagnostic: {e}"));
            }
            // ② 尾部校验和区(最后 8 字节:CRC32 + ISIZE / Adler-32)必须被抓。
            let tail = good.len().saturating_sub(8);
            for i in tail..good.len() {
                let mut bad = good.clone();
                bad[i] = bad[i].wrapping_add(1);
                let r = decode(&mut h, vec![arr(&bad)]).unwrap_or_else(|e| {
                    panic!("{name}: trailer byte {i} raised a diagnostic: {e}")
                });
                assert!(
                    matches!(r.value, Value::Err(_)),
                    "{name}: trailer byte {i} flipped but the frame decoded clean — \
                     the checksum is not being verified"
                );
            }
        }
    }

    /// **压缩炸弹护栏**:越限返回 `ERR`,不是 OOM、不是 panic。
    ///
    /// ⚠ 这条只钉**常量本身**与「非帧输入走 `ERR`」;**越限那条分支的执行
    /// 覆盖在下一条测试里** —— `MAX_DECOMPRESSED` 是规范条款,不该被随手改。
    #[test]
    fn the_bomb_guard_limit_is_a_written_constant() {
        assert_eq!(
            MAX_DECOMPRESSED,
            64 * 1024 * 1024,
            "the decompression cap is a SPEC clause; changing the number is a spec change"
        );
        let mut h = NullHost;
        let out = zstd_decompress(&mut h, vec![arr(&[1, 2, 3])]).expect("returns");
        assert!(
            matches!(out.value, Value::Err(_)),
            "a 3-byte non-frame must be an ERR value"
        );
    }

    /// ⚠🔴 **[D13-049 / N-028] 越限分支此前**零执行覆盖**。**
    ///
    /// 旧结论「64 MiB 的真炸弹在单测里造不出来」是错的:造炸弹不需要真造 GB,
    /// 只要输出**刚过上限一个字节**。addendum-07 §7 W-05 打 ✅ 时写的是
    /// 「单测守着常量本身」—— 而它自己的验收条款要求「越限 → `ERR` 而不是
    /// OOM」⇒ 结项声明相对自己的条款是虚的。
    ///
    /// D13-074:解压输出的 `capacity` 必须与 `len` **同阶**。
    ///
    /// ⚠ 缺陷形态用 `read_to_end`(**倍增**增长)⇒ 一个恰好压到上限的合法载荷
    ///   会拿到 ~2× 的 `capacity` ⇒ 峰值约为文档上限的 2×,与规范 §20.2 的
    ///   「超过 64 MiB 返 `ERR`,**不是** OOM」相抵触(峰值 2× 在内存受限环境里
    ///   就是 OOM 的前置条件)。
    /// ⚠ 这条守卫**量出** `capacity` 与 `len` 的关系 —— 只看长度的话,
    ///   缺陷形态与修好的形态**一模一样**,这条守卫会假绿。
    #[test]
    fn the_decompression_output_capacity_stays_close_to_its_length() {
        let payload = vec![b'A'; 4 * 1024 * 1024];
        let mut s = payload.as_slice();
        let out = read_capped_exact(&mut s, MAX_DECOMPRESSED).expect("read must succeed");
        assert_eq!(out.len(), payload.len());
        assert!(
            out.capacity() <= out.len() + 64 * 1024,
            "capacity {} must stay within one chunk of len {} — the doubling growth is back",
            out.capacity(),
            out.len()
        );
    }

    /// 越限时**读到的字节数**必须**封顶**在 `MAX + 1` —— 不能因为定步读取
    /// 就一路读到底(那会让「炸弹」把内存吃光才报越限)。
    #[test]
    fn the_over_limit_read_stops_at_one_byte_past_the_cap() {
        let payload = vec![b'A'; MAX_DECOMPRESSED + 4096];
        let mut s = payload.as_slice();
        let out = read_capped_exact(&mut s, MAX_DECOMPRESSED).expect("read must succeed");
        assert!(
            out.len() <= MAX_DECOMPRESSED + 1,
            "the read must stop at MAX + 1 byte(s), got {}",
            out.len()
        );
    }

    /// ⚠ **判据是语义,不是本实现的输出**:断言「解压超过上限返回一个
    /// `ERR` **值**、而不是 `ERR` 诊断、且进程活着走完」—— 不去比对任何
    /// 字节数、不抄实现的错误文本。
    #[test]
    fn over_limit_decompression_is_an_err_value_not_an_oom() {
        // 载荷 = 上限 **加一个字节** ⇒ 必然越限,而与「压缩后多大」无关。
        let bomb = vec![b'A'; MAX_DECOMPRESSED + 1];
        let packed = zstd::bulk::compress(&bomb, 1).expect("compress");
        let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        e.write_all(&bomb).expect("gzip");
        let gz = e.finish().expect("gzip finish");
        // ⚠ 先把 64 MiB 的**源**放掉再解压:否则两个 64 MiB 的缓冲同时活着,
        //   峰值会翻倍。压缩流本身只有几十 KiB。
        drop(bomb);
        // 结构性命题(不是某个字节数):这确实是个炸弹 —— 几十 KiB 打出 64 MiB+。
        assert!(
            packed.len() * 100 < MAX_DECOMPRESSED,
            "the fixture must actually be a bomb: {} bytes in, {} out",
            packed.len(),
            MAX_DECOMPRESSED + 1
        );

        let mut h = NullHost;
        let z = zstd_decompress(&mut h, vec![arr(&packed)]);
        assert!(
            matches!(&z, Ok(o) if matches!(o.value, Value::Err(_))),
            "zstd over the cap must be an ERR **value**, got {z:?}"
        );

        // 同一条护栏的 flate 侧(`flate_decompress` 是另一个执行点)。
        let g = gzip_decompress(&mut h, vec![arr(&gz)]);
        assert!(
            matches!(&g, Ok(o) if matches!(o.value, Value::Err(_))),
            "gzip over the cap must be an ERR **value**, got {g:?}"
        );
        // 走到这里进程还活着 ⇒ 不是 OOM、不是 abort。
    }

    #[test]
    fn level_out_of_range_is_e0030_and_never_clamped() {
        let mut h = NullHost;
        let e = gzip_compress(&mut h, vec![arr(b"x"), Value::Integer(99)]).unwrap_err();
        assert_eq!(e.diagnostic().code, ErrorCode::E0030);
        assert!(e.diagnostic().message.contains("outside"));
    }

    #[test]
    fn shape_errors_are_diagnostics() {
        let mut h = NullHost;
        assert_eq!(
            zstd_compress(&mut h, vec![]).unwrap_err().diagnostic().code,
            ErrorCode::E0022
        );
        assert_eq!(
            zstd_decompress(&mut h, vec![Value::String("x".into())])
                .unwrap_err()
                .diagnostic()
                .code,
            ErrorCode::E0030
        );
    }

    #[test]
    fn spec_lists_all_eight_members() {
        let names: Vec<&str> = SPEC.functions.iter().map(|(n, _)| *n).collect();
        assert_eq!(
            names,
            vec![
                "ZSTD_COMPRESS",
                "ZSTD_DECOMPRESS",
                "ZLIB_COMPRESS",
                "ZLIB_DECOMPRESS",
                "GZIP_COMPRESS",
                "GZIP_DECOMPRESS",
                "DEFLATE_RAW_COMPRESS",
                "DEFLATE_RAW_DECOMPRESS",
            ]
        );
    }
}
