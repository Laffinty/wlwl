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
) -> Result<i32, WlwlError> {
    match v {
        None | Some(Value::Null) => Ok(0),
        Some(Value::Integer(n)) => {
            let n = i32::try_from(*n).unwrap_or(i32::MAX);
            if n < lo || n > hi {
                return Err(host.diag(
                    ErrorCode::E0030,
                    format!("{name}: level {n} is outside {lo}..={hi}"),
                ));
            }
            Ok(n)
        }
        Some(other) => Err(type_err(host, name, "a level integer", other)),
    }
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
    match zstd::bulk::compress(&data, level) {
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
    let mut out = Vec::new();
    let read = std::io::Read::take(&mut dec, MAX_DECOMPRESSED as u64 + 1).read_to_end(&mut out);
    match read {
        Ok(_) if out.len() as u64 > MAX_DECOMPRESSED as u64 => {
            ok(io_err(NAME, std::io::Error::other("limit"), true))
        }
        Ok(_) => ok(to_array(out)),
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
    build: fn(&[u8], i32) -> std::io::Result<Vec<u8>>,
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
    let reader = match build(data) {
        Ok(r) => r,
        Err(e) => return ok(err(name, format!("bad header: {e}"))),
    };
    let mut out = Vec::new();
    // `take(LIMIT + 1)` 同时做到两件事:把内存**封顶**在 LIMIT + 1,
    // 以及「读满了 LIMIT + 1 就知道超了」。
    let read = reader
        .take(MAX_DECOMPRESSED as u64 + 1)
        .read_to_end(&mut out);
    match read {
        Ok(_) if out.len() > MAX_DECOMPRESSED => {
            ok(io_err(name, std::io::Error::other("limit"), true))
        }
        Ok(_) => ok(to_array(out)),
        Err(e) => ok(err(name, format!("decompress: {e}"))),
    }
}

/// `ZLIB_COMPRESS(data, level?) -> ARRAY[INTEGER]` —— RFC 1950(zlib 包装,Adler-32)。
pub fn zlib_compress(host: &mut dyn StdHost, args: Vec<Value>) -> Result<Outcome, WlwlError> {
    flate_compress(host, "ZLIB_COMPRESS", args, (0, 9), |data, lvl| {
        let mut e =
            flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::new(lvl as u32));
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
        let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::new(lvl as u32));
        e.write_all(data)?;
        e.finish()
    })
}

/// `GZIP_DECOMPRESS(data) -> ARRAY[INTEGER]`。⚠ CRC32 不符 → `ERR`。
pub fn gzip_decompress(host: &mut dyn StdHost, args: Vec<Value>) -> Result<Outcome, WlwlError> {
    flate_decompress(host, "GZIP_DECOMPRESS", args, |data| {
        Ok(Box::new(flate2::read::GzDecoder::new(
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
        let mut e =
            flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::new(lvl as u32));
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
    #[test]
    fn repetitive_input_actually_shrinks() {
        let mut h = NullHost;
        let d = "ab".repeat(20_000).into_bytes();
        let out = zstd_compress(&mut h, vec![arr(&d)]).expect("zstd");
        let c = back(&out.value).len();
        assert!(
            c * 50 < d.len(),
            "40 KB of \"ab\" should compress far below 1 KB, got {c} bytes"
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
    /// ⚠ 64 MiB 的**真炸弹**在单测里造不出来(要先占住 64 MiB),所以这里
    /// 断言两件可测的事:① 上限常量被写死(不是可调 `const`);② 非帧输入
    /// 走 `ERR` 而不是 panic。真炸弹由契约表的「上限是规范性条款」条款守着。
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
