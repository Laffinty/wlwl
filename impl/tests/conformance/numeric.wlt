// conformance/numeric.wlt - section 16.5 mandatory test 4
// Demonstrates: section 9.5 arithmetic + comparison + INT conversion
// happy path. NEG(INTEGER_MIN) overflow and FLOAT-to-INT out-of-range
// probes are omitted because v0.4 does not yet expose those builtins
// (P4-H1-002).

LET(a, 40);
LET(b, 2);
LET(sum, +(a, b));
LET(prod, *(a, b));
LET(less, <(b, a));
PRINT("sum =", sum);
PRINT("prod =", prod);
PRINT("less =", less);