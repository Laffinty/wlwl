// [v0.2 Phase G6] fuzz target — evaluator.
//
// Fuzzes `wlwl_eval::Evaluator` against program bytes that survive
// parse without error. This is the most expensive target because it
// drives a full tree-walking interpreter on each input.
//
// Run:
//   cd impl/fuzz
//   cargo +nightly fuzz run fuzz_target_eval -- -max_total_time=120

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let tokens = match wlwl_lexer::tokenize(data, "fuzz.wl") {
        Ok(ts) => ts,
        Err(_) => return,
    };
    let ast = match wlwl_parser::parse(&tokens, "fuzz.wl") {
        Ok(a) => a,
        Err(_) => return,
    };
    // Cap evaluator step count to bound run time per iteration.
    let mut evaluator = wlwl_eval::Evaluator::new();
    evaluator.set_max_steps(1024);
    let _ = evaluator.run_program(&ast);
});
