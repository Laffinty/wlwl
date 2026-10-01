//! [v0.2 Phase F1] — eval hot-path benchmarks.
//!
//! **Ten** benches live in this file. Five from Phase F1 (plan §3 F1):
//!   1. simple_loop_1m       — 1M FOR iterations, integer arithmetic
//!   2. closure_density       — repeated counter + closure calls
//!   3. string_concat         — 1k-rep string concatenation
//!   4. array_higher_order    — MAP / FILTER / REDUCE on 1000-element ARRAY
//!   5. error_propagation     — chained OK / ERR through UNWRAP_OR
//!
//! And five from [v0.11 M5] — the **R1 hot members** of the stdlib
//! foundation (ADR-0021 / stdlib spec §5–§8). These exist to answer one
//! question with data rather than opinion: after `std.collection` became
//! pure wlwl (M3-1), is the language fast enough for the members that
//! programs actually call in loops?
//!   6. r1_range_build        — `RANGE(0, 5k)`: the R2 → R1 delta in
//!                             isolation. This is the number that decides
//!                             whether the plan §4 risk "RANGE is a
//!                             performance amplifier" actually bites.
//!   7. r1_collection_map     — `MAP` over 300 elements
//!   8. r1_collection_sort    — `SORT` over 100 elements (R1 SORT is a
//!                             stable selection sort; see collection.wll
//!                             for why it isn't an insertion sort)
//!   9. r1_str_join           — `std.str` `JOIN` over 300 elements
//!  10. r1_math_facade        — `std.math` `MIN`/`MAX`/`FLOOR`/`ROUND`
//!                             in a 20k loop: the pure-wlwl facade's
//!                             per-call cost
//!
//! Benches 1/4/5 all drive their loop with `RANGE`, so their M3 deltas
//! *include* the R1 RANGE cost — deliberately, because that is what a
//! caller pays. Bench 6 exists to separate the two.
//!
//! Baseline numbers live in `benches/baseline.txt`; the M5 section there
//! records the §17.7 口径 (CPU / rustc / profile / LTO / ≥20 samples,
//! median) plus the M2 值直通 and M3 R1 before/after pairs, taken with
//! this same file checked out at `30b70a7^` (pre-M2) and `e2248b8`
//! (post-M3-0, collection still R2) in separate worktrees.
//!
//! Bench design notes:
//!   * parse once outside `b.iter` (we measure eval, not parse)
//!   * fresh `Evaluator` per iteration to mirror `wlwl run <file>`
//!     (state setup + eval; not a warmed cache)
//!   * `black_box` on the eval result prevents the optimizer from
//!     discarding the work
//!
//! Phase I1 (spec §6.6 alignment): the workload sources were rewritten
//! from LET-rebind accumulation (the former D030 semantics, now
//! illegal — a loop-body LET shadows) to the two conformant patterns:
//! FOR + RANGE for the driver, and captured-cell closures (§6.4) where
//! the benchmark's subject is mutation itself. `simple_loop_1m`
//! deliberately avoids closures so it still measures loop dispatch +
//! LET + arithmetic; its workload therefore changed and the Phase G8
//! baseline was regenerated (see baseline.txt header + deviations).

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use wlwl_ast::Expr;
use wlwl_eval::Evaluator;
use wlwl_parser::parse_with_warnings;

const FILE: &str = "<bench>";

/// Parse once, eval many.
fn parse_program(src: &str) -> Expr {
    let (ast, _warns) = parse_with_warnings(src, FILE).expect("bench parse");
    ast
}

/// Fresh `Evaluator` per iteration so we measure the full run loop
/// (state setup + eval), not a warmed cache.
fn run_eval(src: &str, ast: &Expr) {
    let mut ev = Evaluator::new().with_source(src, FILE);
    let v = ev.eval(ast).expect("bench eval");
    black_box(v);
}

// ---------- 1. simple_loop_1m -------------------------------------------
//
// Spec §6.6 v0.4: "simple loop 1M iterations under 30 seconds". This is
// the headline throughput benchmark.
//
// Measures pure loop dispatch + LET + integer arithmetic: FOR over a
// pre-built RANGE, body binds a fresh shadowing LET per iteration. No
// closure calls, no cell writes (those are bench 2's subject).
fn bench_simple_loop_1m(c: &mut Criterion) {
    let src = "\
IMPORT(\"wlwl:std.collection\", [\"RANGE\"]);
FOR(i, RANGE(1, 1000001, 1),
    LET(discard, +(i, 1))
);
";
    let ast = parse_program(src);
    c.bench_function("simple_loop_1m", |b| {
        b.iter(|| run_eval(src, &ast));
    });
}

// ---------- 2. closure_density ------------------------------------------
//
// 300k invocations of a counter closure (100k iterations x 3 calls).
// Exercises:
//   * FUN creation / dispatch
//   * cell-mutable upgrade on first call
//   * cell write + read per call
// The driver is FOR + RANGE (spec §7.3/§10.5); mutation lives inside
// the counter closure (spec §6.4).
fn bench_closure_density(c: &mut Criterion) {
    let src = "\
IMPORT(\"wlwl:std.collection\", [\"RANGE\"]);
LET(make_counter, FUN((),
    (LET(count, 0);
     FUN((), (SET(count, +(count, 1)); count)))
));
LET(c, make_counter());
FOR(i, RANGE(0, 100000, 1),
    (LET(_, c());
     LET(_, c());
     LET(_, c()))
);
";
    let ast = parse_program(src);
    c.bench_function("closure_density", |b| {
        b.iter(|| run_eval(src, &ast));
    });
}

// ---------- 3. string_concat --------------------------------------------
//
// 1000 iterations of `s = s + "x"` through a captured-cell closure.
// Exercises string allocation + concatenation in the eval path.
fn bench_string_concat(c: &mut Criterion) {
    let src = "\
IMPORT(\"wlwl:std.collection\", [\"RANGE\"]);
LET(main, FUN((),
    (LET(s, \"\");
     LET(append, FUN((v), (SET(s, +(s, v)); s)));
     FOR(i, RANGE(0, 1000, 1), append(\"x\"));
     s)
));
main();
";
    let ast = parse_program(src);
    c.bench_function("string_concat", |b| {
        b.iter(|| run_eval(src, &ast));
    });
}

// ---------- 4. array_higher_order ---------------------------------------
//
// MAP / FILTER / REDUCE on a 1000-element integer array via
// `wlwl:std.collection`. Exercises:
//   * array construction (captured-cell append per element)
//   * std callback dispatch through NativeInvoke::Std
//   * per-element arithmetic inside the callback
fn bench_array_higher_order(c: &mut Criterion) {
    let src = "\
IMPORT(\"wlwl:std.collection\", [\"MAP\", \"FILTER\", \"REDUCE\", \"RANGE\"]);
LET(main, FUN((),
    (LET(arr, []);
     LET(append, FUN((v), (SET(arr, +(arr, [v])); arr)));
     FOR(i, RANGE(0, 1000, 1), append(+(i, 1)));
     LET(doubled, MAP(arr, FUN((x), +(x, x))));
     LET(evens, FILTER(doubled, FUN((x), ==(%(x, 2), 0))));
     REDUCE(evens, FUN((a, b), +(a, b)), 0))
));
main();
";
    let ast = parse_program(src);
    c.bench_function("array_higher_order", |b| {
        b.iter(|| run_eval(src, &ast));
    });
}

// ---------- 5. error_propagation ----------------------------------------
//
// 40k iterations of a function that returns OK(...), consumed via
// UNWRAP_OR (canonical §12.2 form). Exercises ERR-transparent
// propagation through the consumer registry (A6).
fn bench_error_propagation(c: &mut Criterion) {
    let src = "\
IMPORT(\"wlwl:std.collection\", [\"RANGE\"]);
LET(try_add, FUN((x), OK(+(x, 1))));
LET(main, FUN((),
    (LET(s, 0);
     LET(add, FUN((v), (SET(s, +(s, v)); s)));
     FOR(i, RANGE(0, 10000, 1),
         (LET(_, UNWRAP_OR(try_add(i), 0));
          LET(_, UNWRAP_OR(try_add(i), 0));
          LET(_, UNWRAP_OR(try_add(i), 0));
          add(UNWRAP_OR(try_add(i), 0)))
     );
     s)
));
main();
";
    let ast = parse_program(src);
    c.bench_function("error_propagation", |b| {
        b.iter(|| run_eval(src, &ast));
    });
}

// ---------- 6. r1_range_build ------------------------------------------
//
// [v0.11 M5] `RANGE(0, 5k)` in isolation. Before M3-1 this was one
// native Rust loop producing 100k elements; now it is 100k iterations of an
// interpreted `WHILE` + `PUSH`. This bench is the measurement that decides
// whether the plan §4 risk row "RANGE is a performance amplifier" bites —
// and `simple_loop_1m` (1M iterations, spec §6.6 budget < 30 s) is the
// consequence if it does.
fn bench_r1_range_build(c: &mut Criterion) {
    let src = "\
IMPORT(\"wlwl:std.collection\", [\"RANGE\"]);
LET(main, FUN((), LEN(RANGE(0, 5000, 1))));
main();
";
    let ast = parse_program(src);
    c.bench_function("r1_range_build", |b| {
        b.iter(|| run_eval(src, &ast));
    });
}

// ---------- 7. r1_collection_map ---------------------------------------
//
// [v0.11 M5] `MAP` over 300 integers — the R1 member programs hit hardest
// in a loop, and the one whose R2 → R1 delta is most visible (R2 pushed
// into a Rust Vec; R1 rebuilds the array per element via `PUSH`).
fn bench_r1_collection_map(c: &mut Criterion) {
    let src = "\
IMPORT(\"wlwl:std.collection\", [\"MAP\", \"RANGE\"]);
LET(main, FUN((), (
    LET(arr, []); \
    LET(append, FUN((v), (SET(arr, +(arr, [v])); arr))); \
    FOR(i, RANGE(0, 300, 1), append(+(i, 1))); \
    LEN(MAP(arr, FUN((x), *(x, 2)))))));
main();
";
    let ast = parse_program(src);
    c.bench_function("r1_collection_map", |b| {
        b.iter(|| run_eval(src, &ast));
    });
}

// ---------- 8. r1_collection_sort --------------------------------------
//
// [v0.11 M5] `SORT` over 100 integers. R1 SORT is a **stable selection
// sort**: every round does one `SLICE` + one `CONCAT` + one `PUSH`, so it
// is O(n²) comparisons but only O(n) array copies — the copy count is what
// matters here, since wlwl arrays are immutable and every element move is
// a full O(n) clone. 200 elements keeps the bench in the seconds range on
// a debug-info build; the O(n²) shape means the curve, not the absolute
// number, is what to read.
fn bench_r1_collection_sort(c: &mut Criterion) {
    let src = "\
IMPORT(\"wlwl:std.collection\", [\"SORT\", \"RANGE\"]);
LET(main, FUN((), (
    LET(arr, []); \
    LET(append, FUN((v), (SET(arr, +(arr, [v])); arr))); \
    FOR(i, RANGE(0, 100, 1), append(%(i, 7))); \
    LEN(SORT(arr)))));
main();
";
    let ast = parse_program(src);
    c.bench_function("r1_collection_sort", |b| {
        b.iter(|| run_eval(src, &ast));
    });
}

// ---------- 9. r1_str_join ---------------------------------------------
//
// [v0.11 M5] `std.str` `JOIN` over 300 rendered values. `JOIN` is the
// §6 member most likely to sit inside a reporting / logging loop.
fn bench_r1_str_join(c: &mut Criterion) {
    let src = "\
IMPORT(\"wlwl:std.collection\", [\"RANGE\"]);
IMPORT(\"wlwl:std.str\", [\"JOIN\"]);
LET(main, FUN((), (
    LET(arr, []); \
    LET(append, FUN((v), (SET(arr, +(arr, [v])); arr))); \
    FOR(i, RANGE(0, 300, 1), append(+(i, 1))); \
    LEN(JOIN(arr, \",\")))));
main();
";
    let ast = parse_program(src);
    c.bench_function("r1_str_join", |b| {
        b.iter(|| run_eval(src, &ast));
    });
}

// ---------- 10. r1_math_facade -----------------------------------------
//
// [v0.11 M5] `std.math` facade in a 20k loop. This is the *pure wlwl*
// half of a mixed module (ADR-0021): MIN/MAX/FLOOR/ROUND are expressible in
// wlwl, so every one of them is an interpreted closure call plus a small
// chain of comparisons. SQRT/POW are deliberately **not** here — they are
// R2 kernels, and what that buys is measured by the correctness benches,
// not by this loop.
fn bench_r1_math_facade(c: &mut Criterion) {
    let src = "\
IMPORT(\"wlwl:std.collection\", [\"RANGE\"]);
IMPORT(\"wlwl:std.math\", [\"MIN\", \"MAX\", \"FLOOR\", \"ROUND\"]);
LET(main, FUN((), (
    LET(s, 0); \
    LET(accum, FUN((v), (SET(s, +(s, v)); s))); \
    FOR(i, RANGE(0, 20000, 1), \
        accum(+(ROUND(MIN(MAX(i, 0), 100)), FLOOR(/(*(i, 3), 2))))); \
    s)));
main();
";
    let ast = parse_program(src);
    c.bench_function("r1_math_facade", |b| {
        b.iter(|| run_eval(src, &ast));
    });
}

criterion_group!(
    benches,
    bench_simple_loop_1m,
    bench_closure_density,
    bench_string_concat,
    bench_array_higher_order,
    bench_error_propagation,
    bench_r1_range_build,
    bench_r1_collection_map,
    bench_r1_collection_sort,
    bench_r1_str_join,
    bench_r1_math_facade,
);
criterion_main!(benches);
