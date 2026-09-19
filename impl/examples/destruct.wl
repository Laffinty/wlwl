// examples/destruct.wl — destructure-binding patterns (v0.3 §7.5)
// LET accepts patterns: array [a, b, c] or dict {key: var}.

LET([x, y, z], [10, 20, 30]);
PRINT(x);
PRINT(y);
PRINT(z);

LET({name, age}, {name: "wlwl", age: 0.4});
PRINT(name);
PRINT(age);
