// conformance/core_subsets.wlt - section 16.5 mandatory test 1
// Demonstrates: section 1-10 production rules via minimal .wl programs.
// Each LET / FUN / PRINT line exercises a core language subset.

LET(name, "world");                  // section 2.1 let-binding
LET(add, FUN((a, b), +(a, b)));      // section 2.4 fn-binding
LET(sum, add(2, 3));                 // section 2.6 call
PRINT("hello,", name);                // section 2.7 print
PRINT("sum =", sum);                  // section 9.5 arithmetic