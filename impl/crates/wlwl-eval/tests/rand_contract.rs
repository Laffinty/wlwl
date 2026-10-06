//! `wlwl:std.rand` 成员契约 —— 标准库规范 §17(v0.11.3 M6 / addendum-08)。
//!
//! ## 这册钉的**不是**统计性质,而是**逐字节的可复现性**
//!
//! `std.rand` 的全部存在理由是「同一熵 ⇒ 同一输出」。所以本表的主体是
//! **冻结向量**:给定熵 `"wlwl"`,第一次、第二次、第三次抽签的值**逐字节**固定。
//! 算法一换这条就红 ⇒ 换算法必须同批更新并在 CHANGELOG 显式记录。
//!
//! **期望值取自一份独立的 Python 第二实现**,不是从本实现导出来的 ——
//! 否则「冻结」只会冻住「当前实现恰好长什么样」。
//!
//! 分布 / 均值 / 方差这类**统计**性质**不在本表**:它们是多项分布,
//! 写进 `cargo test` 只会得到一条**必然偶尔变红**的假门禁(实测踩过:
//! 70000 次抽 7 个值,计数 `[10006, 9967, 10153, …]`,最大偏差 1.7σ,完全健康)。
//! ⇒ 统计量在 `wlwl-std` 的单测里用 **4σ 容差**断言,「无偏」的**确定性**证明
//! (`2^64 mod 7 = 2` ⇒ 最高两个 `u64` 必然被拒)也在单测里。
//!
//! ## ⚠️ 本册最重要的那条:状态必须被**喂回去**
//!
//! `DICT` 是**不可变**的(语言规范 §2.1),`SET_PROP` / `INDEX_SET` 对字典也是
//! 「返回**新**字典、接收者不变」(§13.4)。⇒ 抽签成员返回 **`[值, 新状态]`**,
//! 必须把新状态喂回去才能拿到下一个数。
//!
//! 早期实现返回标量,于是同一个 `rng` 反复抽拿到的是**同一个数** —— 这不是实现
//! 慢,那是「值传递 + 不可变状态 = 永不前进」这条语言事实的直接后果。
//! `two_draws_advance_the_state` 是它的守卫。

use std::path::PathBuf;
use wlwl_eval::Evaluator;

struct Case {
    name: &'static str,
    src: &'static str,
    expect: &'static str,
}

const CASES: &[Case] = &[
    // ── 播种:状态逐字节冻结 ───────────────────────────────────────────
    Case {
        name: "seed_state_is_byte_frozen",
        src: concat!(
            r#"IMPORT("wlwl:std.rand", ["SEED", "UNIFORM", "INT_RANGE", "CODEPOINT"]); "#,
            r#"SEED("wlwl")"#
        ),
        // FNV-1a-64("wlwl") → SplitMix64 展开 4 × u64 → 按 i64 位模式显示。
        // ⚠️ 显示为有符号数是**故意的**:`Value::Integer` 是 i64,而 xoshiro
        // 状态是 u64(见 rand.rs 文件头)。⚠️ `DICT` 的 display **不带引号**
        // (键直接渲染),这是本仓 `Value::display()` 的既有形状。
        expect: concat!(
            r#"[s0: 6764085530779015468, s1: -6853809058185053683, "#,
            r#"s2: -3836445681651209747, s3: -5665664888633704124]"#
        ),
    },
    Case {
        name: "seed_of_empty_entropy_differs_from_a_non_empty_one",
        src: concat!(
            r#"IMPORT("wlwl:std.rand", ["SEED", "UNIFORM", "INT_RANGE", "CODEPOINT"]); "#,
            r#"=(SEED(""), SEED("wlwl"))"#
        ),
        // 空熵是**合法**的(不是错误),但必须给出**另一个**状态。
        expect: "FALSE",
    },
    Case {
        name: "seed_is_deterministic",
        src: concat!(
            r#"IMPORT("wlwl:std.rand", ["SEED", "UNIFORM", "INT_RANGE", "CODEPOINT"]); "#,
            r#"=(SEED("wlwl"), SEED("wlwl"))"#
        ),
        // 同一熵 ⇒ 同一状态 ⇒ 同一序列。这是整册的地基。
        expect: "TRUE",
    },
    // ── INT_RANGE:三个**喂回状态**的连续抽签 ──────────────────────────
    Case {
        name: "int_range_threads_three_draws",
        src: concat!(
            r#"IMPORT("wlwl:std.rand", ["SEED", "UNIFORM", "INT_RANGE", "CODEPOINT"]); "#,
            r#"LET(r0, SEED("wlwl"));"#,
            r#"LET([v1, r1], INT_RANGE(r0, 0, 100));"#,
            r#"LET([v2, r2], INT_RANGE(r1, 0, 100));"#,
            r#"LET([v3, r3], INT_RANGE(r2, 0, 100));"#,
            r#"[v1, v2, v3]"#
        ),
        // 期望值来自独立 Python 实现。注意这三个数**各不相同** ——
        // 若实现忘了把推进后的状态交回去,它们会全部相等。
        expect: "[59, 78, 53]",
    },
    Case {
        name: "int_range_state_after_one_draw_is_byte_frozen",
        src: concat!(
            r#"IMPORT("wlwl:std.rand", ["SEED", "UNIFORM", "INT_RANGE", "CODEPOINT"]); "#,
            r#"LET([v, r1], INT_RANGE(SEED("wlwl"), 0, 100)); r1"#
        ),
        // 状态本身也被冻结:它证明「成员交回的是**推进后**的状态」,
        // 而不是一个恰好等于入参的副本。
        expect: concat!(
            r#"[s0: 5504305597402767973, s1: 4034820580446177996, "#,
            r#"s2: 6264487314914963649, s3: 8604446185847429122]"#
        ),
    },
    Case {
        name: "two_draws_advance_the_state",
        src: concat!(
            r#"IMPORT("wlwl:std.rand", ["SEED", "UNIFORM", "INT_RANGE", "CODEPOINT"]); "#,
            r#"LET(r0, SEED("wlwl"));"#,
            r#"LET([v1, r1], INT_RANGE(r0, 0, 1000000000));"#,
            r#"LET([v2, r2], INT_RANGE(r1, 0, 1000000000));"#,
            r#"[=(v1, v2), =(r1, r0)]"#
        ),
        // ⚠️ 这条是「状态必须喂回去」的守卫,而且**两个分量都要 FALSE**:
        // 值不同(证明真的抽了新数)+ 状态不同(证明交回的是**推进后**的状态,
        // 不是原样退回入参)。写成「两者相等」的单条断言会弱一半。
        expect: "[FALSE, FALSE]",
    },
    Case {
        name: "int_range_is_reproducible_across_independent_chains",
        src: concat!(
            r#"IMPORT("wlwl:std.rand", ["SEED", "UNIFORM", "INT_RANGE", "CODEPOINT"]); "#,
            r#"LET([a1, a2], INT_RANGE(SEED("wlwl"), 0, 100));"#,
            r#"LET([b1, b2], INT_RANGE(SEED("wlwl"), 0, 100));"#,
            r#"[a1, b1, =(a1, b1), =(a2, b2)]"#
        ),
        // 两条**完全独立**的链(各自从 SEED 开始)⇒ 完全相同。
        // 有任何隐藏全局状态时这条不可能成立(ADR-0027 A1 的守卫)。
        expect: "[59, 59, TRUE, TRUE]",
    },
    Case {
        name: "int_range_crosses_zero",
        src: concat!(
            r#"IMPORT("wlwl:std.rand", ["SEED", "UNIFORM", "INT_RANGE", "CODEPOINT"]); "#,
            r#"LET(r0, SEED("wlwl"));"#,
            r#"LET([v1, r1], INT_RANGE(r0, -5, 5));"#,
            r#"LET([v2, r2], INT_RANGE(r1, -5, 5));"#,
            r#"LET([v3, r3], INT_RANGE(r2, -5, 5));"#,
            r#"[v1, v2, v3]"#
        ),
        // 区间可以跨负数 —— `lo >= hi` 才是空区间。
        expect: "[4, 3, -2]",
    },
    // ── UNIFORM ──────────────────────────────────────────────────────
    Case {
        name: "uniform_unit_interval_is_byte_frozen",
        src: concat!(
            r#"IMPORT("wlwl:std.rand", ["SEED", "UNIFORM", "INT_RANGE", "CODEPOINT"]); "#,
            r#"LET(r0, SEED("wlwl"));"#,
            r#"LET([v1, r1], UNIFORM(r0));"#,
            r#"LET([v2, r2], UNIFORM(r1));"#,
            r#"LET([v3, r3], UNIFORM(r2));"#,
            r#"[v1, v2, v3]"#
        ),
        // ⚠️ 这三个值取自 53 bit 尾数,格式是**非指数**十进制 —— 因为
        // `Value::Float::display()` 走 Rust f64 最短往返,而 Python 的 `repr`
        // 在指数区间(`1.1e-16`)与它**不同**。冻结脚本对此有显式校验。
        expect: "[0.24385640050283885, 0.7961367080718161, 0.5884155445564618]",
    },
    Case {
        name: "uniform_bounded_is_byte_frozen",
        src: concat!(
            r#"IMPORT("wlwl:std.rand", ["SEED", "UNIFORM", "INT_RANGE", "CODEPOINT"]); "#,
            r#"LET(r0, SEED("wlwl"));"#,
            r#"LET([v1, r1], UNIFORM(r0, 0, 10));"#,
            r#"LET([v2, r2], UNIFORM(r1, 0, 10));"#,
            r#"LET([v3, r3], UNIFORM(r2, 0, 10));"#,
            r#"[v1, v2, v3]"#
        ),
        // 端点写**整数**字面量也合法(内部提升为 FLOAT)——
        // 让 `UNIFORM(r, 0, 10)` 报 E0030 是刻意的用户敌意。
        expect: "[2.4385640050283888, 7.961367080718161, 5.884155445564618]",
    },
    Case {
        name: "uniform_signed_interval_is_byte_frozen",
        src: concat!(
            r#"IMPORT("wlwl:std.rand", ["SEED", "UNIFORM", "INT_RANGE", "CODEPOINT"]); "#,
            r#"LET(r0, SEED("wlwl"));"#,
            r#"LET([v1, r1], UNIFORM(r0, -5.0, 3.0));"#,
            r#"LET([v2, r2], UNIFORM(r1, -5.0, 3.0));"#,
            r#"LET([v3, r3], UNIFORM(r2, -5.0, 3.0));"#,
            r#"[v1, v2, v3]"#
        ),
        expect: "[-3.049148795977289, 1.369093664574529, -0.29267564354830533]",
    },
    Case {
        name: "uniform_unit_interval_never_reaches_one",
        src: concat!(
            r#"IMPORT("wlwl:std.rand", ["SEED", "UNIFORM", "INT_RANGE", "CODEPOINT"]); "#,
            r#"IMPORT("wlwl:std.collection", ["RANGE"]);"#,
            r#"LET(r0, SEED("wlwl"));"#,
            r#"LET MUT(ok, TRUE);"#,
            r#"LET MUT(cur, r0);"#,
            r#"FOR(i, RANGE(300), ("#,
            r#"  LET([v, nxt], UNIFORM(cur));"#,
            r#"  IF(||(>=(v, 1.0), <(v, 0.0)), SET(ok, FALSE), NULL);"#,
            r#"  SET(cur, nxt)"#,
            r#" ));"#,
            r#"ok"#
        ),
        // 「绝不返 1.0」必须**被保证**(右移 11 位取 53 bit),而不是
        // 「恰好不会」。300 次抽样是对这条的回归锁;真正的保证在实现的
        // `(next_u64() >> 11)` 与单测的 200 000 次里。
        expect: "TRUE",
    },
    Case {
        name: "uniform_never_returns_the_exclusive_upper_bound_in_a_narrow_interval",
        src: concat!(
            r#"IMPORT("wlwl:std.rand", ["SEED", "UNIFORM", "INT_RANGE", "CODEPOINT"]); "#,
            r#"LET([v, r1], UNIFORM(SEED("wlwl"), 1000000000.0, 1000000001.0));"#,
            r#"<(v, 1000000001.0)"#
        ),
        // `a` 远大于区间宽度时 `(b - a)` 的量级会低于半个 ulp,浮点舍入会把结果
        // **顶到 `b`**。这条钉住「右端不包含」是被保证的(实现里夹一次)。
        expect: "TRUE",
    },
    // ── CODEPOINT ─────────────────────────────────────────────────────
    Case {
        name: "code_point_is_byte_frozen",
        src: concat!(
            r#"IMPORT("wlwl:std.rand", ["SEED", "UNIFORM", "INT_RANGE", "CODEPOINT"]); "#,
            r#"LET(r0, SEED("wlwl"));"#,
            r#"LET([c1, r1], CODEPOINT(r0));"#,
            r#"LET([c2, r2], CODEPOINT(r1));"#,
            r#"LET([c3, r3], CODEPOINT(r2));"#,
            r#"[c1, c2, c3]"#
        ),
        // ⚠️ 字符串字面量**没有 `\uXXXX` 转义**(§1.8 的转义集只有
        // `\n \t \r \\ \" \/ \0 \b \f $`),所以 .wll 源码里只能是**原字符**;
        // 而**期望值**这一侧是 Rust 字符串,用 `\u{...}` 写码点才不会被
        // 肉眼打错(手打过一次:U+8F85B 被我打成了 `賻`,那是 U+8D3B)。
        //   U+8F85B / U+EC892 / U+517D9(由独立 Python 实现产出)。
        // 「恰一个字符」「非代理区」「非永久非字符」三条由 `wlwl-std` 单测的
        // 100 000 次抽样守 —— 语言层没有码点取值的成员,写不出那条断言。
        expect: "[\u{8f85b}, \u{ec892}, \u{517d9}]",
    },
    // ── 形态:元数 / 类型 / 值域 ────────────────────────────────────────
    Case {
        name: "two_argument_uniform_is_e0022_not_a_default",
        src: concat!(
            r#"IMPORT("wlwl:std.rand", ["SEED", "UNIFORM", "INT_RANGE", "CODEPOINT"]); "#,
            r#"UNIFORM(SEED("wlwl"), 0.0, 1.0, 2.0)"#
        ),
        // `StdFn` 签名层没有可选形参构造 ⇒ 一个成员内部判 `{1, 3}`。
        // ⚠️ **2 参必须是错误**,不能被当成「`a` 取默认」——那是最容易长出
        // 语义漏洞的地方。
        expect: "!E0022 UNIFORM: function expects 1 argument(s), got 4",
    },
    Case {
        name: "uniform_rejects_an_empty_interval",
        src: concat!(
            r#"IMPORT("wlwl:std.rand", ["SEED", "UNIFORM", "INT_RANGE", "CODEPOINT"]); "#,
            r#"UNIFORM(SEED("wlwl"), 1.0, 1.0)"#
        ),
        expect:
            "!E0030 UNIFORM: [a, b) is empty when a >= b (or an endpoint is NaN), got a = 1, b = 1",
    },
    Case {
        name: "int_range_rejects_an_empty_interval",
        src: concat!(
            r#"IMPORT("wlwl:std.rand", ["SEED", "UNIFORM", "INT_RANGE", "CODEPOINT"]); "#,
            r#"INT_RANGE(SEED("wlwl"), 5, 5)"#
        ),
        expect: "!E0030 INT_RANGE: [lo, hi) is empty when lo >= hi, got lo = 5, hi = 5",
    },
    Case {
        name: "int_range_arity_three",
        src: concat!(
            r#"IMPORT("wlwl:std.rand", ["SEED", "UNIFORM", "INT_RANGE", "CODEPOINT"]); "#,
            r#"INT_RANGE(SEED("wlwl"), 0)"#
        ),
        expect: "!E0022 INT_RANGE: function expects 3 argument(s), got 2",
    },
    Case {
        name: "code_point_arity_one",
        src: concat!(
            r#"IMPORT("wlwl:std.rand", ["SEED", "UNIFORM", "INT_RANGE", "CODEPOINT"]); "#,
            r#"CODEPOINT(SEED("wlwl"), 1)"#
        ),
        expect: "!E0022 CODEPOINT: function expects 1 argument(s), got 2",
    },
    Case {
        name: "seed_rejects_a_non_entropy_argument",
        src: concat!(
            r#"IMPORT("wlwl:std.rand", ["SEED", "UNIFORM", "INT_RANGE", "CODEPOINT"]); "#,
            r#"SEED(42)"#
        ),
        expect: "!E0030 SEED: expected a string or a byte array, got integer",
    },
    Case {
        name: "seed_rejects_an_out_of_range_byte",
        src: concat!(
            r#"IMPORT("wlwl:std.rand", ["SEED", "UNIFORM", "INT_RANGE", "CODEPOINT"]); "#,
            r#"SEED([256])"#
        ),
        // 静默截断会让「同一个熵」有两种解释。
        expect: "!E0030 SEED: byte 256 is outside 0..=255",
    },
    Case {
        name: "a_non_dictionary_is_not_a_rng",
        src: concat!(
            r#"IMPORT("wlwl:std.rand", ["SEED", "UNIFORM", "INT_RANGE", "CODEPOINT"]); "#,
            r#"UNIFORM(42)"#
        ),
        expect: "!E0030 UNIFORM: expected a RNG state dictionary, got integer",
    },
    Case {
        name: "a_state_dictionary_missing_a_word_is_rejected",
        src: concat!(
            r#"IMPORT("wlwl:std.rand", ["SEED", "UNIFORM", "INT_RANGE", "CODEPOINT"]); "#,
            r#"UNIFORM(["s0": 1])"#
        ),
        // ⚠️ 用**字典字面量**而不是 `DICT()`:后者在本仓不是可直接调用的全局名
        // (`E0020 undefined name DICT`),用它会先撞上「未定义名」,测不到
        // 「缺 `s1`」这一层。⚠️ 也不能写 `[]` —— 那是**空数组**。
        expect: "!E0030 UNIFORM: state dictionary is missing `s1` (see std.rand::SEED)",
    },
    Case {
        name: "the_all_zero_state_is_rejected",
        src: concat!(
            r#"IMPORT("wlwl:std.rand", ["SEED", "UNIFORM", "INT_RANGE", "CODEPOINT"]); "#,
            r#"UNIFORM(["s0": 0, "s1": 0, "s2": 0, "s3": 0])"#
        ),
        // xoshiro 的全零态是**吸收态**(恒输出 0)。播种侧已经挡过一次,
        // 读侧再挡一次 —— 用户可以手写这个字典。
        expect: concat!(
            "!E0030 UNIFORM: state is the xoshiro all-zero state, which is absorbing ",
            "(always 0); re-seed with SEED"
        ),
    },
];

fn scratch_dir() -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("wlwl_rand_{nanos}"));
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
fn rand_contract() {
    let bad: Vec<String> = CASES
        .iter()
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
        "wlwl:std.rand contract drift:\n    {}",
        bad.join("\n    ")
    );
}

/// 规范 §17 的成员表 ↔ `SPEC.functions`,且**成员数 = 4** 进守卫。
///
/// `SEED` / `UNIFORM` / `INT_RANGE` / `CODEPOINT`。**非目标也要钉**:
/// 没有 `NORMAL` / 泊松 / 指数,也没有任何「全局当前 RNG」——
/// 下一个人「顺手加一个」时这条会红。
#[test]
fn rand_member_set_matches_the_spec_table() {
    let text = std::fs::read_to_string(spec_path()).expect("stdlib spec readable");
    let start = text.find("## 17 ").expect("§17 header exists");
    let end = text[start..]
        .find("\n## ")
        .map(|i| start + i)
        .expect("a following top-level heading exists");
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
            // 去重:`UNIFORM` 在表里占**两行**(单参形态与三参形态),
            // 而这条守卫守的是**成员集合**,不是表格行数。
            && !spec.contains(&name)
        {
            spec.push(name);
        }
    }

    let impls: Vec<String> = wlwl_std::resolve("wlwl:std.rand")
        .expect("wlwl:std.rand resolves")
        .functions()
        .iter()
        .map(|(n, _)| (*n).to_string())
        .collect();

    assert_eq!(
        spec.len(),
        4,
        "§17 table extractor found {} member(s): {spec:?} — the table's shape changed \
         and the extractor needs updating",
        spec.len()
    );
    for n in ["SEED", "UNIFORM", "INT_RANGE", "CODEPOINT"] {
        assert!(
            spec.contains(&n.to_string()),
            "§17 table missed `{n}`: {spec:?}"
        );
        assert!(
            impls.contains(&n.to_string()),
            "implementation does not export `{n}`: {impls:?}"
        );
    }
    assert_eq!(impls.len(), 4, "wlwl:std.rand exports exactly 4 members");
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

/// 代码侧的锁与文档侧的锁必须成对。
#[test]
fn the_reproducibility_contract_is_documented() {
    let text = std::fs::read_to_string(spec_path()).expect("stdlib spec readable");
    for needle in [
        "SEED",
        "xoshiro256++",
        "SplitMix64",
        "不可变",
        "隐藏状态",
        "ADR-0027",
    ] {
        assert!(
            text.contains(needle),
            "stdlib spec lost `{needle}` — either the design changed on purpose (update \
             this test and §17 together) or the section was trimmed by accident"
        );
    }
}

/// A7 的过渡形态必须**留在规范里** —— 这是 ADR-0027 明写的要求。
///
/// 不写它,它就默认长期住在 `std`,而规范 §14 已登记「`std.ai` / `std.agent`
/// 降级为官方包」的方向,两处会互相矛盾。这条测试就是为了让「那句话被删掉」
/// 变成一件响的事。
#[test]
fn the_transitional_status_of_std_rand_is_still_written_down() {
    let text = std::fs::read_to_string(spec_path()).expect("stdlib spec readable");
    assert!(
        text.contains("过渡") && text.contains("官方包"),
        "stdlib spec no longer records that `std.rand`'s home in std is a TRANSITIONAL \
         form (official-package mechanism does not exist yet). ADR-0027 A7 requires this \
         sentence; without it the layout silently becomes permanent."
    );
}
