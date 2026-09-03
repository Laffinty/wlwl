// showcase.wl — Phase 4 std.io / std.fs / std.json demo.
//
// Demonstrates:
//   - IMPORT from the `wlwl:std.io` namespace (PRINT / INPUT)
//   - IMPORT from the `wlwl:std.fs` namespace (READ_FILE / WRITE_FILE / EXISTS)
//   - IMPORT from the `wlwl:std.json` namespace (PARSE / STRINGIFY)
//   - Cross-directory module use (this file lives under impl/examples/)
//   - Type-annotation slots (parsed, not checked)

// ── std.io ──────────────────────────────────────────────────────
IMPORT("wlwl:std.io", ["PRINT"]);
PRINT("== WLWL Phase 4 showcase ==");

// ── std.fs + std.json round-trip ───────────────────────────────
IMPORT("wlwl:std.fs",   ["READ_FILE", "WRITE_FILE", "EXISTS"]);
IMPORT("wlwl:std.json", ["PARSE", "STRINGIFY"]);

LET(path, "build/showcase.json");
LET(doc, ["name": "wlwl", "phase": 4, "ok": true]);
LET(ok, WRITE_FILE(path, STRINGIFY(doc, null)));
IF(EXISTS(path),
    LET(parsed, PARSE(READ_FILE(path)));
    PRINT("round-trip:", parsed)
);