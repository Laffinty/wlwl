//! WLWL package manifest + lock file (v0.3 §13.8).
//!
//! ## Scope
//!
//! - `manifest` — parse + validate a `wlwl.toml`. The on-disk schema
//!   follows v0.3 §13.8: a `[package]` block, a `[dependencies]`
//!   map keyed by `<namespace>:<name>`, an optional `[namespaces]`
//!   override map, and an optional `[features]` block.
//!
//! - `lock` — generate / read a `wlwl.lock` JSON file that records
//!   the resolved paths / versions / source hashes for every
//!   dependency. JSON chosen over TOML so the lock stays stable
//!   even if the manifest schema evolves.
//!
//! ## Why a separate crate
//!
//! `wlwl-eval`'s `ModuleLoader` needs the manifest to resolve
//! `IMPORT("myteam:utils", ...)` paths; the `wlwl` CLI also needs it
//! to honour `[package].entry` and to emit `wlwl.lock`. Pulling this
//! into its own crate keeps parsing / validation independent of the
//! tree-walking interpreter, and avoids growing `wlwl-eval` further
//! (it is already 2,400+ lines).

pub mod manifest;
pub mod lock;