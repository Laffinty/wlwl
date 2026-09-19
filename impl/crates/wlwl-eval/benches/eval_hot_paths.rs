//! [v0.2 Phase F1] — eval hot-path benchmarks.
//!
//! Five benches land in this file (plan §3 F1):
//!   1. simple_loop_1m       — 1M WHILE iterations, integer arithmetic
//!   2. closure_density       — repeated counter + closure calls
//!   3. string_concat         — 1k-rep string concatenation
//!   4. array_higher_order    — MAP / FILTER / REDUCE on 1000-element ARRAY
//!   5. error_propagation     — chained OK / ERR through UNWRAP_OR
//!
//! Baseline numbers (this machine, this commit) live in
//! `benches/baseline.txt` and are the source of truth for
//! Phase G8 perf-regression gate (fail if bench > 110% baseline).
//!
//! Bench design notes:
//!   * parse once outside `b.iter` (we measure eval, not parse)
//!   * fresh `Evaluator` per iteration to mirror `wlwl run <file>`
//!     (state setup + eval; not a warmed cache)
//!   * `black_box` on the eval result prevents the optimizer from
//!     discarding the work

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
// Spec §6.6 v0.3 (carried into v0.4 §3.6): "simple loop 1M iterations
// under 30 seconds". This is the headline throughput benchmark.
//
// Uses LET rebinding (D030: LET updates outer scope) instead of SET to
// avoid the cell-mutable upgrade path; we want to measure pure
// arithmetic + LET + WHILE dispatch, not the closure-capture upgrade.
fn bench_simple_loop_1m(c: &mut Criterion) {
    let src = "\
LET(total, 0);
LET(i, 1);
WHILE(<(i, 1000001),
    LET(total, +(total, i));
    LET(i, +(i, 1))
);
";
    let ast = parse_program(src);
    c.bench_function("simple_loop_1m", |b| {
        b.iter(|| run_eval(src, &ast));
    });
}

// ---------- 2. closure_density ------------------------------------------
//
// 100k invocations of a counter closure. Exercises:
//   * FUN creation / dispatch
//   * cell-mutable upgrade on first call
//   * cell write + read per iteration
fn bench_closure_density(c: &mut Criterion) {
    let src = "\
LET(make_counter, FUN((),
    LET(count, 0);
    FUN((),
        LET(_, SET(count, +(count, 1)));
        count
    )
));
LET(c, make_counter());
LET(s, 0);
LET(i, 0);
WHILE(<(i, 100000),
    LET(_, c());
    LET(_, c());
    LET(_, c());
    LET(i, +(i, 1))
);
";
    let ast = parse_program(src);
    c.bench_function("closure_density", |b| {
        b.iter(|| run_eval(src, &ast));
    });
}

// ---------- 3. string_concat --------------------------------------------
//
// 1000 iterations of `s = s + "x"`. Exercises string allocation +
// concatenation in the eval path. Uses LET rebind (D030).
fn bench_string_concat(c: &mut Criterion) {
    let src = "\
LET(s, \"\");
LET(i, 0);
WHILE(<(i, 1000),
    LET(s, +(s, \"x\"));
    LET(i, +(i, 1))
);
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
//   * array literal construction
//   * std callback dispatch through NativeInvoke::Std
//   * per-element arithmetic inside the callback
fn bench_array_higher_order(c: &mut Criterion) {
    let src = "\
IMPORT(\"wlwl:std.collection\", [\"MAP\", \"FILTER\", \"REDUCE\"]);
LET(arr, []);
LET(i, 0);
WHILE(<(i, 1000),
    LET(arr, +(arr, [+(i, 1)]));
    LET(i, +(i, 1))
);
LET(doubled, MAP(arr, FUN((x), +(x, x))));
LET(evens, FILTER(doubled, FUN((x), ==(%(x, 2), 0))));
LET(sum, REDUCE(evens, FUN((a, b), +(a, b)), 0));
";
    let ast = parse_program(src);
    c.bench_function("array_higher_order", |b| {
        b.iter(|| run_eval(src, &ast));
    });
}

// ---------- 5. error_propagation ----------------------------------------
//
// 10k iterations of a function that returns OK(...), consumed via
// UNWRAP_OR (canonical §12.2 form). Exercises ERR-transparent
// propagation through the consumer registry (A6).
fn bench_error_propagation(c: &mut Criterion) {
    let src = "\
LET(try_add, FUN((x), OK(+(x, 1))));
LET(s, 0);
LET(i, 0);
WHILE(<(i, 10000),
    LET(_, UNWRAP_OR(try_add(i), 0));
    LET(_, UNWRAP_OR(try_add(i), 0));
    LET(_, UNWRAP_OR(try_add(i), 0));
    LET(s, +(s, UNWRAP_OR(try_add(i), 0)));
    LET(i, +(i, 1))
);
";
    let ast = parse_program(src);
    c.bench_function("error_propagation", |b| {
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
);
criterion_main!(benches);