//! **失败口径钉子**(一致性册 §6.2 的 V-05 · 规范仓 `check_spec.py` 第 8 项)。
//!
//! 规范册的成员表「失败」列声明的每个诊断码,实现侧**必须有断言位**钉住 ——
//! `check_spec.py` 扫描 `src:` / `expect:` 行算「已钉」面,没钉住的就是
//! 「失败口径对拍缺口」。本表专收**别处契约覆盖不到**的钉子(单命名空间、
//! 单成员调用,好让扫描器把码归属到正确的成员)。
//!
//! ⚠ 扫描器的归属规则(`_case_pin`):`src:` 里只能出现**一个** IMPORT 命名
//!   空间、被调用的已导入成员只能有**一个** ⇒ 每条用例都按这个形状写。
//! ⚠ 本表的用例**全部真的跑**(见 `the_failure_surface_pins_hold`)——
//!   只给扫描器看、不跑的「钉子」是假绿。

use wlwl_eval::Evaluator;

/// 一条钉子:程序 + 期望(`!CODE` 前缀匹配)+ 预置环境(可选)。
struct Pin {
    name: &'static str,
    src: &'static str,
    expect: &'static str,
    /// 跑这条用例前把进程环境变量 `TEMP`/`TMP`/`TMPDIR` 指到一个**不存在**的
    /// 目录(用完即还原)—— 用来**逼出**环境类失败(仅 `temp_dir_*` 用)。
    poison_temp: bool,
}

const PINS: &[Pin] = &[
    // ── 宿主挂起点成员在任务体外 ⇒ E0014(并发册 §17.0;W-10 调用时判定)──
    Pin {
        name: "process_run_async_outside_a_task_body_is_e0014",
        src: r#"IMPORT("wlwl:std.process", ["PROCESS_RUN_ASYNC"]); PROCESS_RUN_ASYNC(["echo"])"#,
        expect: "!E0014",
        poison_temp: false,
    },
    Pin {
        name: "http_request_async_outside_a_task_body_is_e0014",
        src: r#"IMPORT("wlwl:std.net", ["HTTP_REQUEST_ASYNC"]); HTTP_REQUEST_ASYNC("GET", "http://127.0.0.1:1/")"#,
        expect: "!E0014",
        poison_temp: false,
    },
    // ── std.log 的章级 blanket(元数 E0022 / 类型 E0030)──
    // ⚠ blanket 的「代表性用例」要挂在**失败列里声明了码的成员**名下
    //   (§23 表里带码的是 `LOG_METRICS` 与 `METRIC_COUNTER`)⇒ 钉这两个。
    Pin {
        name: "log_metrics_with_extra_argument_is_e0022",
        src: r#"IMPORT("wlwl:std.log", ["LOG_METRICS"]); LOG_METRICS(1)"#,
        expect: "!E0022",
        poison_temp: false,
    },
    Pin {
        name: "metric_counter_with_a_non_string_name_is_e0030",
        src: r#"IMPORT("wlwl:std.log", ["METRIC_COUNTER"]); METRIC_COUNTER(1)"#,
        expect: "!E0030",
        poison_temp: false,
    },
    Pin {
        name: "log_with_no_arguments_is_e0022",
        src: r#"IMPORT("wlwl:std.log", ["LOG"]); LOG()"#,
        expect: "!E0022",
        poison_temp: false,
    },
    Pin {
        name: "log_with_a_non_string_level_is_e0030",
        src: r#"IMPORT("wlwl:std.log", ["LOG"]); LOG(1, "x")"#,
        expect: "!E0030",
        poison_temp: false,
    },
    // ── std.sync 的章级 blanket(元数 E0022 / 类型 E0030)──
    Pin {
        name: "counter_with_no_arguments_is_e0022",
        src: r#"IMPORT("wlwl:std.sync", ["COUNTER"]); COUNTER()"#,
        expect: "!E0022",
        poison_temp: false,
    },
    Pin {
        name: "counter_with_a_non_integer_initial_is_e0030",
        src: r#"IMPORT("wlwl:std.sync", ["COUNTER"]); COUNTER("x")"#,
        expect: "!E0030",
        poison_temp: false,
    },
    // ── std.signal 的章级 blanket(类型 E0030:认不得的信号名 / 非 STRING)──
    Pin {
        name: "on_signal_with_a_non_string_signal_is_e0030",
        src: r#"IMPORT("wlwl:std.signal", ["ON_SIGNAL"]); ON_SIGNAL(1, 2)"#,
        expect: "!E0030",
        poison_temp: false,
    },
    // ── std.test::TEMP_DIR 的环境失败 ⇒ E0060(逼法见 `poison_temp`)──
    Pin {
        name: "temp_dir_creation_failure_is_e0060",
        src: r#"IMPORT("wlwl:std.test", ["TEMP_DIR"]); TEMP_DIR()"#,
        expect: "!E0060",
        poison_temp: true,
    },
    // ── std.format::FORMAT_NAMED 的模板解析失败 ⇒ E0039(未闭合占位)──
    Pin {
        name: "format_named_with_an_unclosed_placeholder_is_e0039",
        src: r#"IMPORT("wlwl:std.format", ["FORMAT_NAMED"]); FORMAT_NAMED("{unclosed", ["k": "v"])"#,
        expect: "!E0039",
        poison_temp: false,
    },
    // ── std.http 的章级 blanket E0022(挂在失败列带码的 `CLIENT_IP` 名下)──
    Pin {
        name: "client_ip_with_missing_arguments_is_e0022",
        src: r#"IMPORT("wlwl:std.http", ["CLIENT_IP"]); CLIENT_IP()"#,
        expect: "!E0022",
        poison_temp: false,
    },
];

/// 跑一段程序,取渲染结果;诊断 ⇒ `!CODE message`(与 http_contract 同款)。
fn actual(src: &str) -> String {
    let ast = wlwl_parser::parse(src, "pin.wll")
        .unwrap_or_else(|e| panic!("pin source must parse: {src}\n{e}"));
    let mut ev = Evaluator::new();
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

fn matches(expect: &str, got: &str) -> bool {
    if expect.len() == 6 && expect.starts_with('!') && got.starts_with(expect) {
        return true;
    }
    expect == got
}

/// 逼出环境类失败:先把一个**普通文件**建在系统临时区,再把 `TEMP` / `TMP` /
/// `TMPDIR` 指到**那个文件之下的子路径** ⇒ `create_dir_all` 必然失败
/// (「不是目录」),`TEMP_DIR` 于是报 `E0060`。返回还原闭包。
fn poison_temp_env() -> Box<dyn FnOnce()> {
    let marker = std::env::temp_dir().join(format!("wlwl-pin-poison-{}", std::process::id()));
    std::fs::write(&marker, b"not a directory").expect("write the poison marker file");
    let poisoned = marker.join("child");
    let poisoned = poisoned.to_string_lossy().into_owned();
    #[cfg(windows)]
    const KEYS: [&str; 2] = ["TEMP", "TMP"];
    #[cfg(not(windows))]
    const KEYS: [&str; 3] = ["TMPDIR", "TEMP", "TMP"];
    let saved: Vec<(String, Option<String>)> = KEYS
        .iter()
        .map(|k| (k.to_string(), std::env::var(k).ok()))
        .collect();
    for k in KEYS {
        std::env::set_var(k, &poisoned);
    }
    Box::new(move || {
        for (k, v) in saved {
            match v {
                Some(v) => std::env::set_var(k, v),
                None => std::env::remove_var(k),
            }
        }
        let _ = std::fs::remove_file(&marker);
    })
}

#[test]
fn the_failure_surface_pins_hold() {
    for pin in PINS {
        let restore = if pin.poison_temp {
            Some(poison_temp_env())
        } else {
            None
        };
        let got = actual(pin.src);
        if let Some(r) = restore {
            r();
        }
        assert!(
            matches(pin.expect, &got),
            "pin: {name}\n  expected: {want}\n  actual:   {got}",
            name = pin.name,
            want = pin.expect,
            got = got
        );
    }
}
