// conformance/err_propagation.wlt - section 16.5 mandatory test 2
// Demonstrates: ERR transparent propagation - division by zero
// emits a well-formed JSONL ERR line per section 12.7.

LET(div, FUN((n, d), /(n, d)));      // section 9.5 division

// div(1, 0) returns ERR (section 12.6 short-circuit). The harness
// sees this as exit code 1 with well-formed JSONL on stderr.
LET(_unused, div(1, 0));