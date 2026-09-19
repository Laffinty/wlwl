// examples/format.wl — FORMAT builtin + template syntax (§10.6)
// Demonstrates: positional `{0}`, named `{name}`, and `:fmt`
// specifiers. Phase B5 implements the global `FORMAT(...)` builtin.

LET(greeting, FORMAT("Hello, {0}!", "world"));
PRINT(greeting);

LET(rect, FORMAT("rect={width}x{height}", {width: 320, height: 200}));
PRINT(rect);

LET(pi, FORMAT("pi≈{value:.3f}", {value: 3.14159}));
PRINT(pi);
