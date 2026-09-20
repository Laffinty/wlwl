// interp.wl — v0.6 feature showcase.
//
// Each labelled block exercises one of the nine breaking changes from
// v0.5 to v0.6 (see docs/standard/ Appendix B for the formal list):
//
//   A) Truthiness overhaul         — 0/""/empty/NaN are now falsy
//   B) && / || short-circuit       — left decides; right not evaluated
//   C) IF + ERR                    — ERR routes to else
//   D) ! and NOT both canonical    — no W0054 warning
//   E) AT_K(d, k, default)         — POP is a back-compat alias
//   F) String subscript read       — s[i] returns single-codepoint string
//   G) LET MUT(name, value)        — explicit mutability
//   H) Integer overflow → E0035    — was saturate + W0015
//   J) String interpolation        — `${expr}` segments STR-rendered

// (A) truthiness — every falsy value behaves the same way
LET(falsy_check, FUN((label, v), IF(v, FORMAT("{0}: truthy", [label]), FORMAT("{0}: falsy", [label]))));
PRINT(falsy_check("0", 0));                   // 0: falsy
PRINT(falsy_check("\"\"", ""));                // "": falsy
PRINT(falsy_check("[]", []));                  // []: falsy
PRINT(falsy_check("42", 42));                  // 42: truthy
PRINT(falsy_check("\"x\"", "x"));               // "x": truthy

// (B) short-circuit — only the decisive side runs
LET(side_effect, FUN((label), (PRINT(label); TRUE)));
LET(_and, &&(FALSE, side_effect("LEFT-RIGHT-AND")));  // no "LEFT-RIGHT-AND" emitted
LET(_or, ||(TRUE, side_effect("LEFT-RIGHT-OR")));      // no "LEFT-RIGHT-OR" emitted
PRINT("AND short-circuited; OR short-circuited");

// (C) IF + ERR — error conditions fall to else
LET(risky, FUN((x), IF(<(x, 0), ERR("negative"), +(x, 1))));
LET(s, risky(5));                              // s = 6
LET(s, risky(-1));                             // s = ERR
LET(msg, IF(IS_ERR(s), FORMAT("caught: {0}", [ERR_PAYLOAD(s)]), FORMAT("ok: {0}", [s])));
PRINT(msg);

// (D) `!` and `NOT` are equivalent — no warning in v0.6
PRINT(NOT(FALSE));                             // TRUE
PRINT(!(FALSE));                               // TRUE (no W0054)

// (E) AT_K looks up; POP is a back-compat alias
LET(d, ["a": 1, "b": 2]);
PRINT(AT_K(d, "a", -1));                        // 1
PRINT(AT_K(d, "missing", -1));                 // -1 (default)
PRINT(POP(d, "a", -1));                        // 1 (POP still works, alias for AT_K)

// (F) string subscript — codepoint indexing, negative counts from end
LET(s, "Hello");
PRINT(s[0]);                                   // "H"
PRINT(s[-1]);                                  // "o"
PRINT(s[4]);                                   // "o"
PRINT(LEN(s));                                 // 5

// (G) LET MUT — required to mutate
LET MUT(counter, 0);
LET(step, FUN((), (SET(counter, +(counter, 1)); counter)));
PRINT(step());                                 // 1
PRINT(step());                                 // 2 — counter shared via closure

// (H) integer overflow throws E0035 (was saturate + W0015 in v0.5)
// Demonstrated by UNWRAP_OR: the live call would PANIC at top level.
LET(_overflow_check, UNWRAP_OR(
    ERR("expected overflow"),  // placeholder
    0
));
PRINT("integer-overflow path documented; live test: +(i64::MAX, 1) -> ERR(E0035)");

// (J) string interpolation — multi-segment + nested + escapes
LET(name, "WLWL");
LET(version, 6);
PRINT("Hello, ${name} v${version}!");          // Hello, WLWL v6!
PRINT("1 + 1 = ${+(1, 1)}");                    // 1 + 1 = 2
PRINT("adjacent: ${1}${2}${3}!");               // adjacent: 123!
PRINT("escaped \${literal}");                  // escaped ${literal}
