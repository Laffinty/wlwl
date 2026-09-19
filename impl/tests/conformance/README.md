# Conformance suite (`tests/conformance/`)

Spec v0.4 §16.5 conformance test suite. Each `.wlt` file is a
minimal WLWL program demonstrating one of the 10 mandatory test
categories from §16.5.

## 10 mandatory categories

| File | §16.5 mandatory test |
|---|---|
| `core_subsets.wlt`     | 1. core production rules (`§1-§10`) |
| `err_propagation.wlt`  | 2. ERR transparent propagation (`§12.7` consumers) |
| `index_bounds.wlt`     | 3. INDEX_GET / INDEX_SET boundary |
| `numeric.wlt`         | 4. §9.5 numeric conversions |
| `match_patterns.wlt`   | 5. MATCH exhaustive arms + `_` + `*rest` |
| `destruct.wlt`         | 6. §7.5 pattern bindings |
| `closure_cell.wlt`     | 7. §6.4 cell capture semantics |
| `module_paths.wlt`     | 8. module-path forms + MVS |
| `format_template.wlt`  | 9. §10.6 FORMAT templates |
| `error_schema.wlt`     | 10. JSONL schema stability |

## Run

```bash
cargo test --test conformance              # full conformance suite
cargo test --test conformance dispatch    # individual test
```

The Rust harness in `tests/conformance.rs` reads each `.wlt` file,
shells out to the workspace-built `wlwl run` binary, and asserts
that the run exits with status 0 AND that the JSONL stream matches
the snapshot in `crates/wlwl-error/src/snapshots/` (where
applicable; per §16.5 only `error_schema.wlt` requires JSONL
structural verification — the others are run-and-output-compare).
