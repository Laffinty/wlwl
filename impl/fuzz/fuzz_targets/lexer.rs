// [v0.2 Phase G6] fuzz target — lexer.
//
// Fuzzes `wlwl_lexer::tokenize` against arbitrary byte sequences.
// Goal: catch panic / over-read in the lexer for any source-shape
// produced by the parser's grammar.
//
// Run:
//   cd impl/fuzz
//   cargo +nightly fuzz run fuzz_target_lexer -- -max_total_time=60
//
// The corpus grows across runs; each variant lives in
// `impl/fuzz/corpus/fuzz_target_lexer/`.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // We must tolerate arbitrary bytes; lexer should never panic
    // regardless of input.
    let _tokens = wlwl_lexer::tokenize(data, "fuzz.wll");
});
