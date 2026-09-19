// conformance/error_schema.wlt - section 16.5 mandatory test 10
// Demonstrates: section 14.2 error schema 1.1.0 JSONL stability.
// The harness captures stderr lines and asserts each non-empty
// line is well-formed JSON with all 13 mandatory fields.

LET(div, FUN((n, d), /(n, d)));
LET(_boom1, div(1, 0));   // E0049
LET(_boom2, div(10, 0));  // E0049 (second occurrence)