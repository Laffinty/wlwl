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
//! §17.4 那条「越限 → `ERR` 而非 OOM」由 `wlwl-std` 的单测守着上限常量本身;
//! 语言层能验的是「越限路径确实产出 `ERR` 值而不是 panic」。

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
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("wlwl_compress_{nanos}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
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

/// **往返**:四条格式各自压→解,恒等于原文。
///
/// ⚠ **这里刻意不断言「压缩比」。** 第一版断言了「压缩后更小」,当场红:
/// 同样的 104 字节载荷,Python 的 zlib 压到 66,而 `miniz_oxide` 压到 **115**(比原文
/// 还大)。**压缩比是实现定义的**(块划分 / lazy matching / 熵编码选择都自由)
/// —— 这正是本文件头与规范 §20.3 写的那条。「确实压得动」由 `wlwl-std` 的
/// `repetitive_input_actually_shrinks` 用**高冗余**载荷守(那个各实现都压得动)。
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
        .find("## 20 `std.compress`")
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
        .join("stdlib")
        .join("wlwl-stdlib-spec-v0.11.md")
}

/// 「压缩不是加密」与「解压输出上限」是**两条语言级条款**,必须留在规范里 ——
/// 前者防调用方把 gzip 当「藏起来」,后者防压缩炸弹打满内存。
#[test]
fn the_two_normative_clauses_are_written_down() {
    let text = std::fs::read_to_string(spec_path()).expect("stdlib spec readable");
    for needle in ["压缩不是加密", "MAX_DECOMPRESSED", "不是加密"] {
        assert!(
            text.contains(needle),
            "stdlib spec lost `{needle}` — the no-encryption / bomb-cap clauses are \
             language-level facts, not implementation notes"
        );
    }
}
