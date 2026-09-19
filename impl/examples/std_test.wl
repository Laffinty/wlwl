// examples/std_test.wl — uses wlwl:std.test module (Phase B7 §15.9)
// Demonstrates: ASSERT / EXPECT_EQ / EXPECT_ERR framework.

IMPORT("@std/test") AS test;

LET(t, RUN_TESTS);
LET(result, t([
    ASSERT(+(1, 1), 2),
    ASSERT_EQ(+(2, 2), 4),
    EXPECT_ERR(DIV(1, 0)),
]));
PRINT(result);
