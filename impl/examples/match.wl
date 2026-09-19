// examples/match.wl — pattern-match dispatch (v0.4 §7.6)
// Demonstrates: MATCH with an ARRAY of [pattern, result] clauses plus
// a default fallback. (v0.3's flat-clause form is gone in v0.4.)

LET(classify, FUN((n), MATCH(n,
    [
        [0, "zero"],
        [1, "one"],
        [2, "two"]
    ],
    "many")));
PRINT(classify(0));    // → zero
PRINT(classify(1));    // → one
PRINT(classify(7));    // → many
