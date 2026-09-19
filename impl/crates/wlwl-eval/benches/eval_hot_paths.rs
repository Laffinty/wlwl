//! [v0.2 Phase F1] — eval hot-path benchmarks.
//!
//! Five benches land in this file (plan §3 F1):
//!   1. simple_loop_1m       — 1M FOR iterations, integer arithmetic
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

criterion_group!(
    benches,
    bench_simple_loop_1m,
    bench_closure_density,
    bench_string_concat,
    bench_array_higher_order,
    bench_error_propagation,
);
criterion_main!(benches);
