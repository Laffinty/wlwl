// [v0.2 Phase G6] fuzz target — lexer.
//
// Fuzzes `wlwl_lexer::lex` against arbitrary byte sequences.
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
    // We must tolerate arbitrary bytes; the lexer should never panic
    // regardless of input.
    //
    // [v0.10.3] 两处修正,否则这个 target **编译不过**(它从来没有编译过):
    // 1. 函数名是 `lex` 不是 `tokenize`;2. 它吃 `&str` 不是 `&[u8]`。
    //    libFuzzer 给的是任意字节,不是合法 UTF-8,所以先 `from_utf8` ——
    //    非法序列直接跳过,那属于「输入不是源文件」,不是词法器的职责。
    if let Ok(src) = std::str::from_utf8(data) {
        let _ = wlwl_lexer::lex(src, "fuzz.wll");
    }
});
