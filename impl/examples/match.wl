// examples/match.wl — pattern-match dispatch (v0.3 §7.6)
// Demonstrates: MATCH with multiple arms + default fallback.

LET(classify, FUN((n), MATCH(n,
    0, "zero",
    1, "one",
    2, "two",
    _, "many")));
PRINT(classify(0));
PRINT(classify(1));
PRINT(classify(7));
