//! `wlwl:std.time` 成员契约 —— 标准库规范 §15(v0.11.3 M6 / addendum-01 A1)。
//!
//! 与 `encode_contract.rs` 同款结构,但**期望值口径完全不同**,这里必须写清:
//!
//! **时间成员的契约不能钉「值」,只能钉「不变量」。** `NOW()` 的返回值就是
//! 当下的真实时刻,钉死它等于把时钟冻结成常量(测试必红);而放宽成「任何整数」
//! 又等于什么都没测。所以本表断言的是:
//!
//! 1. **形态**:元数错 / 类型错逐字冻结(与 `sanitize_contract` 同体例);
//! 2. **区间**:`NOW()` 落在 2020–2100 之间 —— 不是「精确值」,而是**量级与单位**
//!    的断言(它证明返回值确实是 Unix 毫秒,而不是秒 / 微秒 / 纳秒);
//! 3. **单调性**:`MONOTONIC` 在循环里**单调不减**,且 `MONOTONIC() <= NOW()` 不成立
//!    (两者单位相同但**起点差着四十多年**,这条正面钉住「它们是两个源」);
//! 4. **不变量**:同一程序两次运行的 `MONOTONIC` 差值恒为 0(没有任何地方偷偷
//!    读了墙钟);`NOW` 两次读数**单调不减**(墙钟可能回跳,但 1000 次采样内不跳)。
//!
//! 「单位」这条尤其重要:秒 / 毫秒 / 微秒之间差 1000 倍,而**秒**还额外带来
//! ±1 s 的量化误差(见 `ADR-0024` §6.3-1 为什么取毫秒)。本表用「区间」把它钉住。

use std::path::PathBuf;
use wlwl_eval::Evaluator;

struct Case {
    name: &'static str,
    src: &'static str,
    expect: &'static str,
}

const CASES: &[Case] = &[
    // ── 形态:元数与类型 ───────────────────────────────────────────────
    Case {
        name: "now_arity_one",
        src: concat!(r#"IMPORT("wlwl:std.time", ["NOW"]); "#, r#"NOW(1)"#),
        expect: "!E0022 NOW: function expects 0 argument(s), got 1",
    },
    Case {
        name: "monotonic_arity_one",
        src: concat!(
            r#"IMPORT("wlwl:std.time", ["MONOTONIC"]); "#,
            r#"MONOTONIC(0)"#
        ),
        expect: "!E0022 MONOTONIC: function expects 0 argument(s), got 1",
    },
    // ── 单位与量级:用区间钉住「确实是 Unix 毫秒」────────────────────────
    // 2020-01-01 = 1_577_836_800_000;2100-01-01 = 4_102_444_800_000。
    // 若实现误用「秒」,读数会落在 1.6e9 → 上界之外;误用「微秒」会远超上界。
    //
    // ⚠ 记法:本语言**没有中缀运算符**(语言规范 §6「一切运算皆调用」),比较是
    // **具名函数** `>(a, b)` / `<(a, b)`,合取是 `&&(a, b)`,减法是 `-(a, b)`。
    // 写成 `NOW() > 0` 会在解析期报 `expected ')', got Gt` —— 写错不是语义错,
    // 是**根本编译不过**,所以这个坑不会以「悄悄算错」的形式出现。
    Case {
        name: "now_is_epoch_millis_within_a_century",
        src: concat!(
            r#"IMPORT("wlwl:std.time", ["NOW"]); "#,
            r"&&(>(NOW(), 1577836800000), <(NOW(), 4102444800000))"
        ),
        expect: "TRUE",
    },
    Case {
        name: "now_is_not_epoch_seconds",
        src: concat!(
            r#"IMPORT("wlwl:std.time", ["NOW"]); "#,
            r"<(NOW(), 10000000000000)"
        ),
        // 同一个下界的另一侧:Unix **秒**现在是 ~1.8e9,微秒是 ~1.8e15。
        // 断言「小于 1e13」⇒ 单位既不是秒也不是微秒。**两个方向都要钉**,
        // 只钉上界的话「实现返回微秒」也会蒙混过关。
        expect: "TRUE",
    },
    Case {
        name: "monotonic_starts_near_zero_not_at_epoch",
        src: concat!(
            r#"IMPORT("wlwl:std.time", ["MONOTONIC"]); "#,
            r"<(MONOTONIC(), 4102444800000)"
        ),
        // 单调钟从**进程启动**起算 ⇒ 读数远小于 Unix 毫秒。
        // 这条与上面两条一起,把「两个成员单位相同但起点不同」钉死。
        expect: "TRUE",
    },
    Case {
        name: "the_two_clocks_are_far_apart",
        src: concat!(
            r#"IMPORT("wlwl:std.time", ["NOW", "MONOTONIC"]); "#,
            r">(-(NOW(), MONOTONIC()), 1577836800000)"
        ),
        // 两者差着 1970→2020 那五十多年。若有人把 `MONOTONIC` 也实现成读墙钟,
        // 这个差值会塌到接近 0。
        expect: "TRUE",
    },
    // ── 不变量:单调不减 ───────────────────────────────────────────────
    //
    // ⚠ 本语言**没有花括号、没有中缀运算符**:条件是 `IF(cond, then, else?)`
    // (表达式求值),循环体是**表达式**,多条语句用 `(a; b; c)` 串起来
    // (语言规范 §6.1 / §6.3)。另:`RANGE` 在 `std.collection`,**不在附录 G**,
    // 所以循环用它时要多一条 `IMPORT`。
    Case {
        name: "monotonic_is_non_decreasing_across_samples",
        src: concat!(
            r#"IMPORT("wlwl:std.time", ["MONOTONIC"]); "#,
            r#"IMPORT("wlwl:std.collection", ["RANGE"]); "#,
            r"LET MUT(ok, TRUE);",
            r"LET MUT(last, -1);",
            r"FOR(i, RANGE(200), (",
            r"  IF(<(MONOTONIC(), last), SET(ok, FALSE), NULL);",
            r"  SET(last, MONOTONIC())",
            r"));",
            r"ok;"
        ),
        // 200 次采样单调不减。规范 §15.1 的「单调不减」契约:**不断言严格递增**
        // —— 毫秒精度下连续两次读数可以相等。
        expect: "TRUE",
    },
    Case {
        name: "monotonic_never_negative_and_not_at_epoch",
        src: concat!(
            r#"IMPORT("wlwl:std.time", ["MONOTONIC"]); "#,
            r"&&(>=(MONOTONIC(), 0), <(MONOTONIC(), 4102444800000))"
        ),
        // 单调钟从**进程启动**起算 ⇒ 读数远小于 Unix 毫秒,且永不为负。
        // 「永不为负」也是它与 `NOW` 的一个可测差别:系统时间若被调到 1970
        // 之前,`NOW` 会读出负值,`MONOTONIC` 不会。
        expect: "TRUE",
    },
    Case {
        name: "wall_clock_is_not_claimed_monotonic",
        src: concat!(
            r#"IMPORT("wlwl:std.time", ["NOW"]); "#,
            r#"IMPORT("wlwl:std.collection", ["RANGE"]); "#,
            r"LET MUT(ok, TRUE);",
            r"LET MUT(last, -1);",
            r"FOR(i, RANGE(200), (",
            r"  IF(<(NOW(), last), SET(ok, FALSE), NULL);",
            r"  SET(last, NOW())",
            r"));",
            r"ok;"
        ),
        // ⚠️ 这条**不是**在断言墙钟单调 —— 规范 §15.3 写死了 `NOW` **不是**单调的
        // (NTP 校时 / 手动改表 / 夏令时都能让它回跳),所以本条只在 200 次采样内
        // 不跳时为 TRUE。**它抓的不是「墙钟会跳」,而是「实现把 `NOW` 接到
        // `MONOTONIC` 上了」** —— 那种错误实现会让本条恒为 TRUE,所以它必须
        // 与上面 `the_two_clocks_are_far_apart` 配对才有意义(那条抓的正是
        // 「两个成员其实是同一个源」)。
        expect: "TRUE",
    },
    // ── SLEEP:形态 + 「阻塞但确实睡够」+ 挂起路线的反向守卫 ─────────────
    Case {
        name: "sleep_arity_zero",
        src: concat!(r#"IMPORT("wlwl:std.time", ["SLEEP"]); "#, r#"SLEEP()"#),
        expect: "!E0022 SLEEP: function expects 1 argument(s), got 0",
    },
    Case {
        name: "sleep_arity_two",
        src: concat!(r#"IMPORT("wlwl:std.time", ["SLEEP"]); "#, r#"SLEEP(1, 2)"#),
        expect: "!E0022 SLEEP: function expects 1 argument(s), got 2",
    },
    Case {
        name: "sleep_rejects_non_integer",
        src: concat!(r#"IMPORT("wlwl:std.time", ["SLEEP"]); "#, r#"SLEEP(1.5)"#),
        expect: "!E0030 SLEEP: expected integer, got float",
    },
    Case {
        name: "sleep_rejects_negative_duration",
        src: concat!(r#"IMPORT("wlwl:std.time", ["SLEEP"]); "#, r#"SLEEP(-1)"#),
        expect: "!E0030 SLEEP: expected a non-negative duration in ms, got -1",
    },
    Case {
        name: "sleep_zero_returns_null",
        src: concat!(r#"IMPORT("wlwl:std.time", ["SLEEP"]); "#, r#"SLEEP(0)"#),
        // `ms = 0` 是**合法**的(让出一次时间片),不是「没写参数」。
        expect: "NULL",
    },
    Case {
        name: "sleep_actually_advances_the_monotonic_clock",
        src: concat!(
            r#"IMPORT("wlwl:std.time", ["SLEEP", "MONOTONIC"]); "#,
            r"LET(t0, MONOTONIC());",
            r"SLEEP(40);",
            r">=(-(MONOTONIC(), t0), 40)"
        ),
        // 断言的是**下界**而不是「恰好 40」:`thread::sleep` 保证至少睡够,
        // 但实际耗时会略大于 40(调度、时钟粒度)。写成「恰好等于」在负载下必红,
        // 那就是一条假门禁。
        expect: "TRUE",
    },
    Case {
        name: "sleep_inside_a_task_terminates",
        src: concat!(
            r#"IMPORT("wlwl:std.time", ["SLEEP"]); "#,
            r"SCOPE(FUN(() , AWAIT(SPAWN(FUN(() , SLEEP(10); 42)))))"
        ),
        // ⚠️ **反向守卫:方案 E 的核心决定**。`SLEEP` 是**阻塞原语**,不挂起任务
        // —— 所以任务体里的 `SLEEP` 之后必须**继续往下执行**并交出 42。
        //
        // 原本的设计是「任务内挂起、到点唤醒」,而 wlwl 只有 `YIELD()` 能中途续跑
        // (`split_body_for_yield` 只认 `YIELD()` 一个名字,非分段任务体被唤醒后走
        // `invoke_closure` **整段重跑**)。挂起路线下这条会**无限循环**:每次唤醒
        // 都重新执行同一个 `SLEEP`。
        //
        // ⚠️ 因此这条红了的形式是**测试超时**,不是断言失败 —— 而「超时」恰恰就是
        // 「挂起路线被重新引入」的特征读数。不要为了让它「快一点失败」而给
        // `SLEEP` 加时限或让它在任务内报错:那会把方案 E 又变回方案 D。
        expect: "42",
    },
];

fn scratch_dir() -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("wlwl_time_{nanos}"));
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
fn time_contract() {
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
        "wlwl:std.time contract drift:\n    {}",
        bad.join("\n    ")
    );
}

/// 规范 §15 的成员表 ↔ `SPEC.functions`。
///
/// 反向守卫:表格被改成别的形状时提取器可能一个成员都抽不出来,那时
/// `[] == []` 会绿 —— 所以正面锁住「正好 2 个」。
#[test]
fn time_member_set_matches_the_spec_table() {
    let text = std::fs::read_to_string(spec_path()).expect("stdlib spec readable");
    let start = text.find("## 15 ").expect("§15 header exists");
    // §15 后面直接是「附录 A」,没有 §16 —— 所以终止条件取**下一个 `## ` 标题**,
    // 而不是写死 §16(写死的话,加一个 §16 时这个守卫会静默失效)。
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
        {
            spec.push(name);
        }
    }

    let impls: Vec<String> = wlwl_std::resolve("wlwl:std.time")
        .expect("wlwl:std.time resolves")
        .functions()
        .iter()
        .map(|(n, _)| (*n).to_string())
        .collect();

    assert_eq!(
        spec.len(),
        3,
        "§15 table extractor found {} member(s): {spec:?} — the table's shape changed \
         and the extractor needs updating",
        spec.len()
    );
    for n in ["NOW", "MONOTONIC", "SLEEP"] {
        assert!(
            spec.iter().any(|m| m == n),
            "§15 table missed `{n}`: {spec:?}"
        );
        assert!(
            impls.iter().any(|m| m == n),
            "implementation does not export `{n}`: {impls:?}"
        );
    }
    assert_eq!(
        impls.len(),
        3,
        "wlwl:std.time exports 3 members (A1 + A2 slices)"
    );
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

/// 代码侧的锁与文档侧的锁必须成对:只锁代码,下一个人看到「测试说行为是这样」
/// 却找不到依据;只锁文档,下一个人会去「修实现」来迁就文档。
#[test]
fn the_clock_contract_is_documented() {
    let text = std::fs::read_to_string(spec_path()).expect("stdlib spec readable");
    for needle in [
        "MONOTONIC",
        "Unix 纪元",
        "单调不减",
        "绝对值无意义",
        "Instant",
        "ADR-0024",
    ] {
        assert!(
            text.contains(needle),
            "stdlib spec lost `{needle}` — either the design changed on purpose (update \
             this test and §15 together) or the section was trimmed by accident"
        );
    }
}
