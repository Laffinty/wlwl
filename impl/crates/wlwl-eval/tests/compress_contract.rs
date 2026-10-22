//! `wlwl:std.compress` 成员契约 —— 标准库规范 §20(v0.11.3 M6 / addendum-07)。
//!
//! ## ⚠️ 这册**不冻结**压缩输出,而冻结「能解开别人的输出」
//!
//! **压缩输出的字节是实现定义的**:同一份输入,zlib / miniz_oxide / zlib-ng 产出的
//! DEFLATE 流**互不相同**(块划分、lazy matching、熵编码选择都是实现自由)。
//! ⇒ 把我们的输出逐字节冻进契约,等于**冻结 flate2 的内部选择**:换个后端就红,
//! 而且**没有任何独立实现能产出同样的字节** —— 那条「冻结」守不住任何东西。
//!
//! 真正该冻的是三件可跨实现成立的事:
//!
//! 1. **往返**:`COMPRESS` → `DECOMPRESS` 恒等于原输入(逐字节)。
//! 2. **独立向量交叉核对**:下面的 gzip / zlib 流是 **Python 标准库**产出的
//!    (`gzip.compress(mtime=0)` / `zlib.compress`),**与本实现无共享代码** ⇒
//!    我们能解开它们,证明解码器是**真的按 RFC 1950/1952 实现**的,而不是
//!    「只会解开自己写的东西」。
//! 3. **确定性**:同 `(data, level)` 两次压缩产出**逐字节相同**(本实现内)。
//!
//! ## 压缩炸弹上限是**规范条款**,不是实现里的一个可调 `const`
//!
//! §17.4 那条「越限 → `ERR` 而非 OOM」在 `wlwl-std` 的单测里**被真的执行**
//! (`over_limit_decompression_is_an_err_value_not_an_oom`,zstd 与 flate 两个
//! 执行点各跑一遍)。⚠ **语言层造不出这个用例**:它需要 > 64 MiB 的
//! `ARRAY[INTEGER]`,而「在解释器里物化一百万个以上的值」正是这条护栏要防的
//! 形状 —— 用被测对象去造测试数据会让测试自己先被打挂。所以「上限是规范性
//! 条款」在语言层仍然只断言**文档里写着** `MAX_DECOMPRESSED`。

mod common;

use std::path::PathBuf;
use wlwl_eval::Evaluator;

struct Case {
    name: &'static str,
    src: &'static str,
    expect: &'static str,
}

/// 104 字节的载荷(测试与 probe 共用同一条,便于互相对照)。
const PLAIN: &str = "wlwl compress: the quick brown fox jumps over the lazy dog. the quick brown fox jumps over the lazy dog.";

/// **Python `gzip.compress(mtime=0)` 的输出**(RFC 1952, 78 字节)——
/// 独立第二实现,`flate2` 无共享代码。
const PY_GZIP_HEX: &str = "1f8b08000000000000ff2bcf29cf5148cecf2d284a2d2eb65228c94855282ccd4cce56482aca2fcf5348cbaf50c82acd2d2856c82f4b2d024be72456552aa4e4a7eb91a41800c9ebdbd868000000";

/// **Python `zlib.compress(level=6)` 的输出**(RFC 1950, 66 字节)。
const PY_ZLIB_HEX: &str = "789c2bcf29cf5148cecf2d284a2d2eb65228c94855282ccd4cce56482aca2fcf5348cbaf50c82acd2d2856c82f4b2d024be72456552aa4e4a7eb91a41800e861261b";

/// **Python `gzip.compress(b"AAA", mtime=0) + gzip.compress(b"BBB", mtime=0)`**
/// (RFC 1952 §2.2 的**多成员**文件,46 字节)—— 独立第二实现。
const PY_GZIP_MULTI_HEX: &str =
    "1f8b08000000000002ff7374740400a731a066030000001f8b08000000000002ff7372720200878dc2d603000000";

/// **Python `gzip.compress(b"wlwl", mtime=0) + b"TRAILING-GARBAGE"`**(40 字节)
/// —— 成员之后还有 16 字节垃圾。
const PY_GZIP_TAIL_HEX: &str =
    "1f8b08000000000002ff2bcf29cf01003fb71bb204000000545241494c494e472d47415242414745";

/// 把 wlwl 源码里的 `[12, 34, …]` 字节数组展开成常量。测试用字面量写死
/// 独立向量,而**载荷**这一侧同样写死字面量,免得测试里跑一段转换逻辑
/// (那会让「期望值」变成「由被测代码算出来的」)。
fn bytes_src(hex: &str) -> String {
    (0..hex.len() / 2)
        .map(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap())
        .map(|b| b.to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

const CASES: &[Case] = &[
    // ── 独立向量交叉核对(Python 标准库产出)─────────────────────────
    Case {
        name: "we_decode_a_python_gzip_stream",
        src: "", // 见下方 runtime 组:这个用例由 `py_vectors_are_decodable` 跑
        expect: "TRUE",
    },
    // ── 形态 ────────────────────────────────────────────────────────
    Case {
        name: "zstd_compress_arity",
        src: concat!(
            r#"IMPORT("wlwl:std.compress", ["ZSTD_COMPRESS"]); "#,
            r#"ZSTD_COMPRESS()"#
        ),
        expect: "!E0022 ZSTD_COMPRESS: function expects 1 argument(s), got 0",
    },
    Case {
        name: "decompress_members_take_exactly_one",
        src: concat!(
            r#"IMPORT("wlwl:std.compress", ["ZLIB_DECOMPRESS"]); "#,
            r#"ZLIB_DECOMPRESS([78, 156], 6)"#
        ),
        expect: "!E0022 ZLIB_DECOMPRESS: function expects 1 argument(s), got 2",
    },
    Case {
        name: "a_non_array_payload_is_e0030",
        src: concat!(
            r#"IMPORT("wlwl:std.compress", ["GZIP_COMPRESS"]); "#,
            r#"GZIP_COMPRESS("not bytes")"#
        ),
        expect: "!E0030 GZIP_COMPRESS: expected a byte array of integers, got string",
    },
    Case {
        name: "an_out_of_range_byte_is_e0030_never_truncated",
        src: concat!(
            r#"IMPORT("wlwl:std.compress", ["GZIP_COMPRESS"]); "#,
            r#"GZIP_COMPRESS([1, 256])"#
        ),
        // 静默截断会让「同一个载荷」有两种解释 —— 字节面最不能容忍二义性。
        expect: "!E0030 GZIP_COMPRESS: byte 256 is outside 0..=255",
    },
    Case {
        name: "a_level_out_of_range_is_e0030_and_never_clamped",
        src: concat!(
            r#"IMPORT("wlwl:std.compress", ["GZIP_COMPRESS"]); "#,
            r#"GZIP_COMPRESS([65, 66], 99)"#
        ),
        // clamp 会让「我传了 9」和「我传了 999」产出同一个结果,那是在骗调用方。
        expect: "!E0030 GZIP_COMPRESS: level 99 is outside 0..=9",
    },
    Case {
        name: "a_zstd_level_out_of_range_is_e0030",
        src: concat!(
            r#"IMPORT("wlwl:std.compress", ["ZSTD_COMPRESS"]); "#,
            r#"ZSTD_COMPRESS([65], 23)"#
        ),
        expect: "!E0030 ZSTD_COMPRESS: level 23 is outside -131072..=22",
    },
    // ── [D13-026 / N-012]缺省级别**必须真的压缩** ──────────────────────
    //
    // ⚠ 载荷用 `MAP(RANGE(0, 2000), …)` 造而不是手写字面量:2000 个元素的
    //   `ARRAY[INTEGER]` 手写没法读,而 `REPEAT("ab", n)` 造出的是**字符串**,
    //   压缩成员只收字节数组。
    // ⚠ 判据写成「压完 **小于一半**」这种**结构性命题**,而不是某个字节数 ——
    //   后者会把实现细节冻死,换个 zlib 版本就红。2000 字节全同载荷在
    //   deflate level 6 下是几十字节量级;stored 则是 2000 + 容器开销。
    Case {
        name: "default_level_actually_compresses_a_redundant_payload",
        src: concat!(
            r#"IMPORT("wlwl:std.compress", ["GZIP_COMPRESS"]); "#,
            r#"IMPORT("wlwl:std.collection", ["MAP", "RANGE"]); "#,
            r#"LET(d, MAP(RANGE(0, 2000), FUN((x), 65))); "#,
            r#"=(<(LEN(GZIP_COMPRESS(d)), 1000), TRUE)"#
        ),
        expect: "TRUE",
    },
    Case {
        name: "default_level_matches_the_library_default_not_zero",
        src: concat!(
            r#"IMPORT("wlwl:std.compress", ["ZLIB_COMPRESS"]); "#,
            r#"IMPORT("wlwl:std.collection", ["MAP", "RANGE"]); "#,
            r#"LET(d, MAP(RANGE(0, 2000), FUN((x), 65))); "#,
            r#"=(LEN(ZLIB_COMPRESS(d, 6)), LEN(ZLIB_COMPRESS(d)))"#
        ),
        // 缺省 == 显式 6(flate2 的 `Compression::default()`)。⚠ 这条同时钉住
        // 「缺省不是 0」:显式传 0 会得到 stored,体积差两个数量级。
        expect: "TRUE",
    },
    Case {
        name: "empty_input_is_an_err_value_not_a_panic",
        src: concat!(
            r#"IMPORT("wlwl:std.compress", ["ZSTD_DECOMPRESS"]); "#,
            r#"IS_ERR(ZSTD_DECOMPRESS([]))"#
        ),
        expect: "TRUE",
    },
    Case {
        name: "garbage_input_is_an_err_value_not_a_panic",
        src: concat!(
            r#"IMPORT("wlwl:std.compress", ["ZSTD_DECOMPRESS", "ZLIB_DECOMPRESS", "GZIP_DECOMPRESS"]); "#,
            r#"&&(IS_ERR(ZSTD_DECOMPRESS([1, 2, 3])), "#,
            r#"&&(IS_ERR(ZLIB_DECOMPRESS([1, 2, 3])), "#,
            r#"IS_ERR(GZIP_DECOMPRESS([1, 2, 3]))))"#
        ),
        // 三条解码路径对垃圾都必须返 `ERR`,**不许 panic** —— 解码器吃损坏
        // 输入是最经典的安全面(越界读 / 无界分配)。
        expect: "TRUE",
    },
    Case {
        name: "the_failure_payload_carries_kind_op_reason",
        src: concat!(
            r#"IMPORT("wlwl:std.compress", ["GZIP_DECOMPRESS"]); "#,
            r#"ERR_PAYLOAD(GZIP_DECOMPRESS([1, 2, 3]))"#
        ),
        expect: concat!(
            "[kind: CompressError, op: GZIP_DECOMPRESS, reason: decompress: ",
            "unexpected end of file]"
        ),
    },
];

fn scratch_dir() -> PathBuf {
    common::scratch("wlwl_compress")
}

fn actual(src: &str) -> String {
    let ast =
        wlwl_parser::parse(src, "t.wll").unwrap_or_else(|e| panic!("case must parse: {src}\n{e}"));
    let mut ev = Evaluator::new().with_base_dir(scratch_dir());
    match ev.eval(&ast) {
        Ok(v) => v.display(),
        Err(e) => {
            let code = e.diagnostic().code.as_str();
            if e.diagnostic().message.is_empty() {
                format!("!{code}")
            } else {
                format!("!{code} {}", e.diagnostic().message)
            }
        }
    }
}

#[test]
fn compress_contract() {
    let bad: Vec<String> = CASES
        .iter()
        .filter(|c| !c.src.is_empty())
        .filter_map(|c| {
            let got = actual(c.src);
            if got == c.expect {
                None
            } else {
                Some(format!(
                    "{name}:\n      expected: {want}\n      actual:   {got}",
                    name = c.name,
                    want = c.expect
                ))
            }
        })
        .collect();
    assert!(
        bad.is_empty(),
        "wlwl:std.compress contract drift:\n    {}",
        bad.join("\n    ")
    );
}

/// **独立向量交叉核对** —— 本册最重的一条。
///
/// 用 **Python 标准库**产出的 gzip / zlib 流喂给我们的解码器,必须解出原文。
/// 这证明解码器是**按 RFC 1950 / 1952 实现的**,而不是「只会解开自己写的东西」
/// —— 后者是一种非常常见的自证循环缺陷(用 `std.compress` 压再解,永远「通过」)。
#[test]
fn py_vectors_are_decodable() {
    let py_gzip = bytes_src(PY_GZIP_HEX);
    let src = format!(
        r#"IMPORT("wlwl:std.compress", ["GZIP_DECOMPRESS"]); GZIP_DECOMPRESS([{py_gzip}])"#
    );
    let out = actual(&src);
    let plain: Vec<String> = PLAIN.bytes().map(|b| b.to_string()).collect();
    assert_eq!(
        out,
        format!("[{}]", plain.join(", ")),
        "we must decode a Python-produced gzip stream to the original bytes"
    );

    let py_zlib = bytes_src(PY_ZLIB_HEX);
    let src = format!(
        r#"IMPORT("wlwl:std.compress", ["ZLIB_DECOMPRESS"]); ZLIB_DECOMPRESS([{py_zlib}])"#
    );
    assert_eq!(
        actual(&src),
        format!("[{}]", plain.join(", ")),
        "we must decode a Python-produced zlib stream to the original bytes"
    );
}

/// ⚠🔴 **[D13-048 / N-027] 多成员必须**全部**解开;流后的尾随字节必须**报错**。
///
/// **出处**:RFC 1952 §2.2 —— 一个 gzip 文件是若干成员首尾相接,解压结果是
/// 拼接;第二段不是成员就不是这个格式的一部分。**独立第二实现**(Python
/// 3.13.13)同判:`gzip.decompress(多成员)` → `b'AAABBB'`,加尾随垃圾 → 抛
/// `BadGzipFile: Not a gzipped file (b'TR')`。
///
/// 此前本实现用**单成员** `flate2::read::GzDecoder` ⇒ 第二个成员被**静默丢弃**
/// 且不报错 —— 与「⚠ CRC32 不符 → `ERR`」给人的完整性承诺相悖:一个「校验过
/// 校验和」的返回值实际只覆盖了输入的一部分。
#[test]
fn py_multi_member_gzip_is_fully_decoded_and_trailing_garbage_is_refused() {
    let decoded = actual(&format!(
        r#"IMPORT("wlwl:std.compress", ["GZIP_DECOMPRESS"]); GZIP_DECOMPRESS([{}])"#,
        bytes_src(PY_GZIP_MULTI_HEX)
    ));
    assert_eq!(
        decoded, "[65, 65, 65, 66, 66, 66]",
        "RFC 1952 §2.2: members concatenate and no member may be dropped"
    );
    let refused = actual(&format!(
        r#"IMPORT("wlwl:std.compress", ["GZIP_DECOMPRESS"]); IS_ERR(GZIP_DECOMPRESS([{}]))"#,
        bytes_src(PY_GZIP_TAIL_HEX)
    ));
    assert_eq!(
        refused, "TRUE",
        "bytes after the last member must be an ERR, never dropped in silence"
    );
}

/// **往返**:四条格式各自压→解,恒等于原文。
///
/// ⚠ **这里刻意不断言「压缩比」。** 第一版断言了「压缩后更小」,当场红:
/// 同样的 104 字节载荷,Python 的 zlib 压到 66,而本实现压到 **115**(比原文
/// 还大)。
///
/// 🔴 **[D13-026]那段注记原先的归因是错的,现已改正。** 它当时写的是
/// 「**压缩比是实现定义的**(块划分 / lazy matching / 熵编码选择都自由)」
/// —— 那句仍然成立,但它**不是**此处变大的原因。真实原因是:`level_of` 在
/// 调用方**没给** `level` 时返回 `Some(0)`,而 `flate2::Compression::new(0)`
/// 是 **stored 块(完全不压)** —— 104 字节 + 11 字节容器开销 = **115**,
/// 数字精确吻合。
///
/// ⚠ **「压缩比是实现定义的」不能拿来解释任何一次反常。** 一个听起来合理
/// 的解释一旦写进契约就成了规范,下一个读到这里的人会拿它当挡箭牌,而真正的
/// bug 从此有了官方背书。这正是本仓 D13-008「已改实现而非冻结错误输出」
/// 要防的形态 —— 这次连**解释**一起改了。
///
/// 「确实压得动」现在由 `wlwl-std` 的 `repetitive_input_actually_shrinks`
/// 用**高冗余**载荷守,⚠ 且它已从「只测 zstd」扩到**四条线全测** —— zstd
/// 恰好是唯一没坏的那条(它的 `level 0` 就是库默认),只测它等于没测。
#[test]
fn every_format_round_trips() {
    let plain: Vec<String> = PLAIN.bytes().map(|b| b.to_string()).collect();
    let lit = plain.join(", ");
    for (name, expr) in [
        ("zstd", r#"ZSTD_DECOMPRESS(ZSTD_COMPRESS([LIT]))"#),
        ("zlib", r#"ZLIB_DECOMPRESS(ZLIB_COMPRESS([LIT]))"#),
        ("gzip", r#"GZIP_DECOMPRESS(GZIP_COMPRESS([LIT]))"#),
        (
            "deflate-raw",
            r#"DEFLATE_RAW_DECOMPRESS(DEFLATE_RAW_COMPRESS([LIT]))"#,
        ),
    ] {
        let src = format!(
            r#"IMPORT("wlwl:std.compress", ["ZSTD_COMPRESS","ZSTD_DECOMPRESS","ZLIB_COMPRESS","ZLIB_DECOMPRESS","GZIP_COMPRESS","GZIP_DECOMPRESS","DEFLATE_RAW_COMPRESS","DEFLATE_RAW_DECOMPRESS"]); {EXPR}"#,
            EXPR = expr.replace("LIT", &lit)
        );
        assert_eq!(actual(&src), format!("[{lit}]"), "{name} must round-trip");
    }
}

/// **确定性**:同输入两次压缩产出**逐字节相同**(本实现内)。
#[test]
fn compression_is_deterministic() {
    let plain: Vec<String> = PLAIN.bytes().map(|b| b.to_string()).collect();
    let lit = plain.join(", ");
    for (name, expr) in [
        ("zstd", r#"ZSTD_COMPRESS([LIT])"#),
        ("zlib", r#"ZLIB_COMPRESS([LIT])"#),
        ("gzip", r#"GZIP_COMPRESS([LIT])"#),
        ("raw", r#"DEFLATE_RAW_COMPRESS([LIT])"#),
    ] {
        let src = format!(
            r#"IMPORT("wlwl:std.compress", ["ZSTD_COMPRESS","ZLIB_COMPRESS","GZIP_COMPRESS","DEFLATE_RAW_COMPRESS"]); =({EXPR}, {EXPR})"#,
            EXPR = expr.replace("LIT", &lit)
        );
        assert_eq!(
            actual(&src),
            "TRUE",
            "{name} compression must be deterministic"
        );
    }
}

/// 规范 §20 的成员表 ↔ `SPEC.functions`,**成员数 = 8** 进守卫(非目标也要钉:
/// 不做归档、不做流式、不做 BZip2/LZMA/LZ4/Snappy)。
#[test]
fn compress_member_set_matches_the_spec_table() {
    let text = std::fs::read_to_string(spec_path()).expect("stdlib spec readable");
    let start = text
        .find("## 标准库规范 · 20 `std.compress`")
        .expect("§20 header exists");
    let end = text[start..]
        .find("\n## ")
        .map(|i| start + i)
        .expect("a following top-level heading exists");
    // 收在「下一章」内 —— 但 `## 20` 之后的下一个 `##` 就是附录 A,所以这里
    // 直接取到它为止即可。
    let body = &text[start..end];
    let mut spec: Vec<String> = Vec::new();
    for line in body.lines() {
        let Some(rest) = line.strip_prefix("| `") else {
            continue;
        };
        let name: String = rest
            .chars()
            .take_while(|c| *c != '`' && *c != '(')
            .collect();
        if !name.is_empty()
            && name
                .chars()
                .all(|c| c.is_ascii_uppercase() || c == '_' || c.is_ascii_digit())
            && !spec.contains(&name)
        {
            spec.push(name);
        }
    }
    let impls: Vec<String> = wlwl_std::resolve("wlwl:std.compress")
        .expect("wlwl:std.compress resolves")
        .functions()
        .iter()
        .map(|(n, _)| (*n).to_string())
        .collect();
    assert_eq!(spec.len(), 8, "§20 table found {spec:?}");
    for n in &impls {
        assert!(spec.contains(n), "§20 table missed `{n}`: {spec:?}");
    }
    assert_eq!(impls.len(), 8, "wlwl:std.compress exports 8 members");
}

fn spec_path() -> PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
        .join("docs")
        .join("wlwl-spec-reference.md")
}

/// 「压缩不是加密」与「解压输出上限」是**两条语言级条款**,必须留在规范里 ——
/// 前者防调用方把 gzip 当「藏起来」,后者防压缩炸弹打满内存。
///
/// ⚠ **判据收在 §20 章内**,不落在整本合并册上。落在整本时,它守住的只是
///   「全册别处碰巧没有同样的字」—— 将来别的章写出同样的字,这条守卫会
///   **当场失去它守的那件事**,而读数仍然是绿的(静默失效)。
#[test]
fn the_two_normative_clauses_are_written_down() {
    let text = std::fs::read_to_string(spec_path()).expect("stdlib spec readable");
    let start = text
        .find("## 标准库规范 · 20 `std.compress`")
        .expect("§20 header exists");
    let end = text[start..]
        .find("\n## ")
        .map(|i| start + i)
        .expect("a following top-level heading exists");
    let body = &text[start..end];
    // ⚠ 两个锚点在 §20 内**各自唯一**:§20.1 的「压缩不是加密」裁决句,以及
    //   §20.2 的解压上限硬边界句。只钉「§20 里出现过 `MAX_DECOMPRESSED`」
    //   不够 —— 那个字样在 §20 里出现两次(成员表 + §20.2),删掉 §20.2 的
    //   裁决句照样绿。
    for needle in [
        "**压缩不是加密**:压缩**不提供机密性**",
        "解压成员输出超过 **`MAX_DECOMPRESSED` = 64 MiB**",
    ] {
        assert!(
            body.contains(needle),
            "stdlib spec §20 lost `{needle}` — the no-encryption / bomb-cap clauses are \
             language-level facts, not implementation notes"
        );
    }
}
