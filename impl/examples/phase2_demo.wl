// phase2_demo.wl — Demonstrates Phase 2 features:
// control flow, FUN, ERR transparent propagation, modules.

IMPORT("math", ["add", "PI"]);

// ── Control flow: WHILE + IF + BREAK ──────────────────────────────
LET(total, 0);
LET(i, 1);
WHILE(<(i, 11),
    IF(==(%(i, 2), 0),
        LET(total, +(total, i))
    );
    LET(i, +(i, 1))
);
PRINT("sum of evens 1..10 =", total);   // → 30

// ── FUN + recursion: factorial ───────────────────────────────────
LET(fact, FUN((n),
    IF(<=(n, 1),
        1,
        *(n, fact(-(n, 1)))
    )
));
PRINT("5! =", fact(5));                  // → 120

// ── §12.6 ERR transparent propagation ────────────────────────────
// + with an ERR arg short-circuits; OR_DIE consumes the ERR.
// (wrap in OK so OR_DIE accepts the result)
LET(safe, OR_DIE(OK(+(1, 2)), -1));         // safe = 3
LET(boom, OR_DIE(+(1, ERR("oops")), -1));   // boom = -1 (short-circuit)
PRINT("safe =", safe, "boom =", boom);

// ── Module use: IMPORT("math", ["add", "PI"]) ─────────────────────
PRINT("add(2, 3) =", add(2, 3));         // → 5
PRINT("PI =", PI);                        // → 3.14159
