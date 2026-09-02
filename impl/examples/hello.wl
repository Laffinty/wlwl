// hello.wl — Phase 1 demo
// Demonstrates: LET, variable lookup, function call, array/dict literal, block.

LET(name, "world");
PRINT("hello,", name);

LET(nums, [1, 2, 3, 4, 5]);
PRINT("nums:", nums);

LET(ages, ["alice": 30, "bob": 25]);
PRINT("ages:", ages);

LET(x, 42);
PRINT("x is", x);
