// [v0.2 Phase G6] fuzz target — evaluator.
//
// Fuzzes `wlwl_eval::Evaluator::eval` against source text that parses
// without error. This is the most expensive target because it drives a
// full tree-walking interpreter on each input.
//
// Run:
//   cd impl/fuzz
//   cargo +nightly fuzz run fuzz_target_eval -- -max_total_time=120

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(src) = std::str::from_utf8(data) else {
        return;
    };
    // `parse` 自己跑词法;解析不过就跳过(那是 parser target 的职责)。
    let Ok(ast) = wlwl_parser::parse(src, "fuzz.wll") else {
        return;
    };
    // [v0.10.3] 原文写的是 `set_max_steps(1024)` + `run_program(&ast)` ——
    // **两个方法在仓库里都不存在**,所以这个 target 从来编译不过。真实的
    // 入口是 `eval(&Expr)`,已改用它。
    //
    // 至于「每轮限时」:原来靠 `set_max_steps`,而**步数预算本身从未实现**。
    // 本次不补这个特性(那是运行期的新功能,不属于健壮性批次),改由 libFuzzer
    // 自己的 `-timeout` / `-max_total_time` 兜底:单轮慢输入表现为一个慢
    // unit,而不是无界挂住。缺口记在 CHANGELOG。
    let mut evaluator = wlwl_eval::Evaluator::new();
    // 无论成功还是诊断,都不该 panic —— 这正是本 target 要守的东西。
    let _ = evaluator.eval(&ast);
});
