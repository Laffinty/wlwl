// examples/closure_cell.wl — closure cell-capture semantics (§6.4)
// Demonstrates: a closure that captures a `LET` cell and shares
// mutations across calls.

LET(n, 0);
LET(step, FUN((), LET(n, +(n, 1))));
LET(c, step());
LET(c, step());
LET(c, step());
PRINT(c);   // 3
