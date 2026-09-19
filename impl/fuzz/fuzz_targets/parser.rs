// [v0.2 Phase G6] fuzz target — parser.
//
// Fuzzes `wlwl_parser::parse` against byte sequences that survive
// `wlwl_lexer::tokenize` without error. The fuzzer driver first
// runs the lexer, then if no lex error fires, runs the parser.
//
// Run:
//   cd impl/fuzz
//   cargo +nightly fuzz run fuzz_target_parser -- -max_total_time=60

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let tokens = match wlwl_lexer::tokenize(data, "fuzz.wl") {
        Ok(ts) => ts,
        Err(_) => return,
    };
    let _ast = wlwl_parser::parse(&tokens, "fuzz.wl");
});
