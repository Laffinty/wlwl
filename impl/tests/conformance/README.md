# Conformance suite (`tests/conformance/`)

Spec v0.4 §16.5 conformance test suite. Each `.wll` file is a
minimal WLWL program demonstrating one of the 10 mandatory test
categories from §16.5.

## 10 mandatory categories

| File | §16.5 mandatory test |
|---|---|
| `core_subsets.wll`     | 1. core production rules (`§1-§10`) |
| `err_propagation.wll`  | 2. ERR transparent propagation (`§12.7` consumers) |
| `index_bounds.wll`     | 3. INDEX_GET / INDEX_SET boundary |
| `numeric.wll`         | 4. §9.5 numeric conversions |
| `match_patterns.wll`   | 5. MATCH exhaustive arms + `_` + `*rest` |
| `destruct.wll`         | 6. §7.5 pattern bindings |
| `closure_cell.wll`     | 7. §6.4 cell capture semantics |
| `module_paths.wll`     | 8. module-path forms + MVS |
| `format_template.wll`  | 9. §10.6 FORMAT templates |
| `error_schema.wll`     | 10. JSONL schema stability |

## Run

```bash
cargo test --test conformance              # full conformance suite
cargo test --test conformance dispatch    # individual test
```

The Rust harness in `tests/conformance.rs` reads each `.wll` file,
shells out to the workspace-built `wlwl run` binary, and asserts
that the run exits with status 0 AND that the JSONL stream matches
the snapshot in `crates/wlwl-error/src/snapshots/` (where
applicable; per §16.5 only `error_schema.wll` requires JSONL
structural verification — the others are run-and-output-compare).
