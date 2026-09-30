// [v0.2 Phase G6] fuzz target — parser.
//
// Fuzzes `wlwl_parser::parse` against byte sequences that are valid UTF-8.
// The fuzzer driver first checks the input decodes, then runs the parser.
//
// Run:
//   cd impl/fuzz
//   cargo +nightly fuzz run fuzz_target_parser -- -max_total_time=60

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // [v0.10.3] 原文是先 `tokenize` 再 `parse(&tokens, …)`,但 `parse` 的签名
    // 是 `parse(input: &str, file: &str)` —— **它自己跑词法**,不吃 token 列表。
    // 原来的写法引用了一个不存在的 `tokenize`,所以这个 target 从来编译不过。
    // 现在直接喂 `&str`;非法 UTF-8 跳过(同 lexer target 的理由)。
    if let Ok(src) = std::str::from_utf8(data) {
        let _ = wlwl_parser::parse(src, "fuzz.wll");
    }
});
