//! `wlwl.toml` manifest parser (v0.3 §13.8).
//!
//! Schema (the on-disk format is TOML; see spec §13.8):
//!
//! ```toml
//! [package]
//! name = "myapp"
//! version = "0.1.0"
//! entry = "src/main.wl"
//!
//! [dependencies]
//! "myteam:utils" = { path = "../utils" }
//! "huggingface:client" = "^0.5.0"   # v0.4 enabled
//!
//! [namespaces]
//! "myteam" = "./vendor/myteam"      # explicit override
//!
//! [features]
//! strict_types = false
//! ```
//!
//! ## Design
//!
//! - The `[package]` block is required. `name`, `version`, and
//!   `entry` are mandatory; missing any of them is a parse error.
//! - The `[dependencies]` block is a map keyed by `<namespace>:<name>`.
//!   Each value is either a short string (a version constraint) or a
//!   table with `path` / `version` / `optional` fields.
//! - The `[namespaces]` block is a map of namespace prefix to a path
//!   (relative to the manifest directory). It is an explicit
//!   override; the `wlwl-eval` side also auto-infers namespaces
//!   from `[dependencies]` keys.
//! - The `[features]` block is a flat map of feature name to a
//!   `toml::Value`. The actual feature semantics live in
//!   `wlwl-eval`; this crate only preserves them as opaque values
//!   so a future `strict_types` flag (spec §2.6) can be read without
//!   a schema bump.

use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// A `[package]` block. All three required fields are validated by
/// `parse`; we keep them as plain `String` here so that an invalid
/// version string (e.g. `"abc"`) does not error at the struct
/// deserialization step — that is a separate concern.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Package {
    pub name: String,
    pub version: String,
    pub entry: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
}

/// One entry in `[dependencies]`. TOML `untagged` deserialisation
/// accepts either a bare string (short-form version constraint) or a
/// table with `path` / `version` / `optional` fields.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Dependency {
    /// Short form: `"^0.5.0"` or `"1.2.3"`.
    Version(String),
    /// Long form: `{ path = "..." }` or `{ version = "...", optional = true }`.
    Detailed(DetailedDep),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DetailedDep {
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub optional: bool,
}

impl Dependency {
    /// Returns the local path of a path-style dependency, if any.
    /// Used by `ModuleLoader` to resolve `myteam:utils` to a directory
    /// containing `<name>.wl`.
    pub fn local_path(&self) -> Option<&str> {
        match self {
            Dependency::Detailed(d) => d.path.as_deref(),
            Dependency::Version(_) => None,
        }
    }
}

/// Top-level manifest. `dependencies` / `namespaces` / `features` are
/// all optional and default to empty.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    pub package: Package,
    #[serde(default)]
    pub dependencies: BTreeMap<String, Dependency>,
    #[serde(default)]
    pub namespaces: BTreeMap<String, String>,
    #[serde(default)]
    pub features: BTreeMap<String, toml::Value>,
}

/// Parse errors. The variants cover the four classes of failure we
/// surface distinctly: malformed TOML, invalid identifier, invalid
/// namespace name, and bad package metadata. The `wlwl-eval` side
/// maps these to specific error codes (E0042 for IO, E0043 for
/// namespace syntax, E0100 for internal; this batch uses E0100 for
/// schema violations because they are programmer / manifest-author
/// mistakes, not runtime conditions).
#[derive(Debug, Clone)]
pub enum ManifestError {
    /// `toml::de::Error` propagated unchanged; preserves the
    /// span / line / column info.
    Toml(toml::de::Error),
    /// Package `name` violates the lowercase-plus-hyphen rule.
    InvalidPackageName(String),
    /// A namespace prefix violates the rule: must start with a
    /// lowercase letter, followed by lowercase letters / digits /
    /// hyphens.
    InvalidNamespaceName(String),
    /// A dependency key is missing the `:` separator (i.e. not in
    /// `<namespace>:<name>` form).
    InvalidDependencyKey(String),
    /// A dependency is empty (no `path` and no `version`).
    EmptyDependency(String),
    /// `entry` is an empty string.
    MissingEntry,
}

impl fmt::Display for ManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ManifestError::Toml(e) => write!(f, "manifest TOML parse error: {}", e),
            ManifestError::InvalidPackageName(n) => {
                write!(f, "invalid package name {:?}: must match ^[a-z][a-z0-9-]*$", n)
            }
            ManifestError::InvalidNamespaceName(n) => {
                write!(f, "invalid namespace {:?}: must match ^[a-z][a-z0-9-]*$", n)
            }
            ManifestError::InvalidDependencyKey(k) => {
                write!(f, "invalid dependency key {:?}: must be <namespace>:<name>", k)
            }
            ManifestError::EmptyDependency(k) => {
                write!(f, "dependency {:?} has neither `path` nor `version`", k)
            }
            ManifestError::MissingEntry => write!(f, "[package] entry is missing or empty"),
        }
    }
}

impl std::error::Error for ManifestError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ManifestError::Toml(e) => Some(e),
            _ => None,
        }
    }
}

impl From<toml::de::Error> for ManifestError {
    fn from(e: toml::de::Error) -> Self {
        ManifestError::Toml(e)
    }
}

/// Parse a `wlwl.toml` from a string. Validates:
/// - package `name` matches `^[a-z][a-z0-9-]*$`
/// - package `version` is non-empty
/// - package `entry` is non-empty
/// - each dependency key matches `^<ns>:<name>$` with valid `ns` and
///   non-empty `name`
/// - each namespace key matches `^[a-z][a-z0-9-]*$`
/// - each dependency has at least one of `path` / `version`
pub fn parse(s: &str) -> Result<Manifest, ManifestError> {
    let m: Manifest = toml::from_str(s)?;
    validate(&m)?;
    Ok(m)
}

fn validate(m: &Manifest) -> Result<(), ManifestError> {
    if !is_valid_package_name(&m.package.name) {
        return Err(ManifestError::InvalidPackageName(m.package.name.clone()));
    }
    if m.package.version.is_empty() {
        // `toml` deserialisation would already have failed on a
        // missing field, but an explicit empty-string guard covers
        // `version = ""` which TOML accepts.
        return Err(ManifestError::InvalidPackageName(format!(
            "version is empty for package `{}`",
            m.package.name
        )));
    }
    if m.package.entry.trim().is_empty() {
        return Err(ManifestError::MissingEntry);
    }
    for key in m.dependencies.keys() {
        let (ns, name) = split_dep_key(key)?;
        if !is_valid_namespace_name(ns) {
            return Err(ManifestError::InvalidNamespaceName(ns.to_string()));
        }
        if name.is_empty() {
            return Err(ManifestError::InvalidDependencyKey(key.clone()));
        }
        let dep = &m.dependencies[key];
        if dep.local_path().is_none() && dep_is_versionless(dep) {
            return Err(ManifestError::EmptyDependency(key.clone()));
        }
    }
    for ns in m.namespaces.keys() {
        if !is_valid_namespace_name(ns) {
            return Err(ManifestError::InvalidNamespaceName(ns.clone()));
        }
    }
    Ok(())
}

fn is_valid_package_name(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_lowercase() => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

fn is_valid_namespace_name(s: &str) -> bool {
    // Same rule as package name per spec §13.6: lowercase letters,
    // digits, hyphens; first char must be a letter.
    is_valid_package_name(s)
}

fn split_dep_key(k: &str) -> Result<(&str, &str), ManifestError> {
    match k.split_once(':') {
        Some((ns, name)) => Ok((ns, name)),
        None => Err(ManifestError::InvalidDependencyKey(k.to_string())),
    }
}

fn dep_is_versionless(d: &Dependency) -> bool {
    match d {
        Dependency::Version(_) => false, // has a version
        Dependency::Detailed(d) => d.version.is_none(),
    }
}

/// Resolve a `<namespace>:<name>` reference to a local directory,
/// using `[namespaces]` as an override and `[dependencies]` as the
/// fallback. Returns the directory relative to the manifest path
/// (the caller is responsible for joining with the manifest
/// directory's parent).
///
/// Returns `None` if the reference is not registered; the caller
/// surfaces E0040 (not found) or E0043 (namespace syntax).
pub fn resolve_namespace<'a>(
    manifest: &'a Manifest,
    ns: &str,
    name: &str,
) -> Option<PathBuf> {
    // 1. Explicit `[namespaces]` override.
    if let Some(p) = manifest.namespaces.get(ns) {
        return Some(PathBuf::from(p));
    }
    // 2. `[dependencies]` auto-inference.
    let key = format!("{}:{}", ns, name);
    if let Some(dep) = manifest.dependencies.get(&key) {
        if let Some(p) = dep.local_path() {
            return Some(PathBuf::from(p));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
[package]
name = "myapp"
version = "0.1.0"
entry = "src/main.wl"
description = "A WLWL app"
license = "MIT"

[dependencies]
"myteam:utils" = { path = "../utils" }
"huggingface:client" = "^0.5.0"
"json:parser" = "1.2.3"
"strict:math" = { version = ">=1.0.0, <2.0.0", optional = true }

[namespaces]
"myteam" = "./vendor/myteam"

[features]
strict_types = false
default_encoding = "utf-8"
"#;

    #[test]
    fn parse_full_sample() {
        let m = parse(SAMPLE).unwrap();
        assert_eq!(m.package.name, "myapp");
        assert_eq!(m.package.version, "0.1.0");
        assert_eq!(m.package.entry, "src/main.wl");
        assert_eq!(m.package.description.as_deref(), Some("A WLWL app"));
        assert_eq!(m.package.license.as_deref(), Some("MIT"));
        assert_eq!(m.dependencies.len(), 4);
        assert!(matches!(
            m.dependencies["json:parser"],
            Dependency::Version(ref v) if v == "1.2.3"
        ));
        assert!(matches!(
            m.dependencies["strict:math"],
            Dependency::Detailed(ref d) if d.version.as_deref() == Some(">=1.0.0, <2.0.0") && d.optional
        ));
        assert_eq!(m.namespaces["myteam"], "./vendor/myteam");
        assert_eq!(m.features.len(), 2);
    }

    #[test]
    fn parse_minimal() {
        let m = parse(r#"
[package]
name = "tiny"
version = "0.0.1"
entry = "main.wl"
"#).unwrap();
        assert!(m.dependencies.is_empty());
        assert!(m.namespaces.is_empty());
        assert!(m.features.is_empty());
    }

    #[test]
    fn rejects_uppercase_package_name() {
        let err = parse(r#"
[package]
name = "MyApp"
version = "0.1.0"
entry = "main.wl"
"#)
        .unwrap_err();
        assert!(matches!(err, ManifestError::InvalidPackageName(_)));
    }

    #[test]
    fn rejects_empty_entry() {
        let err = parse(r#"
[package]
name = "ok"
version = "0.1.0"
entry = ""
"#)
        .unwrap_err();
        assert!(matches!(err, ManifestError::MissingEntry));
    }

    #[test]
    fn rejects_dep_key_without_colon() {
        let err = parse(r#"
[package]
name = "ok"
version = "0.1.0"
entry = "main.wl"

[dependencies]
"myteam utils" = { path = "../utils" }
"#)
        .unwrap_err();
        assert!(matches!(err, ManifestError::InvalidDependencyKey(_)));
    }

    #[test]
    fn rejects_dep_with_neither_path_nor_version() {
        let err = parse(r#"
[package]
name = "ok"
version = "0.1.0"
entry = "main.wl"

[dependencies]
"myteam:utils" = { optional = true }
"#)
        .unwrap_err();
        assert!(matches!(err, ManifestError::EmptyDependency(_)));
    }

    #[test]
    fn rejects_invalid_namespace_name() {
        let err = parse(r#"
[package]
name = "ok"
version = "0.1.0"
entry = "main.wl"

[namespaces]
"MyTeam" = "./vendor/myteam"
"#)
        .unwrap_err();
        assert!(matches!(err, ManifestError::InvalidNamespaceName(_)));
    }

    #[test]
    fn rejects_toml_syntax_error() {
        let err = parse("this is = not [valid").unwrap_err();
        assert!(matches!(err, ManifestError::Toml(_)));
    }

    #[test]
    fn dependency_local_path() {
        let d = Dependency::Detailed(DetailedDep {
            path: Some("../foo".into()),
            version: None,
            optional: false,
        });
        assert_eq!(d.local_path(), Some("../foo"));

        let d2 = Dependency::Version("1.2.3".into());
        assert_eq!(d2.local_path(), None);
    }

    #[test]
    fn resolve_namespace_uses_explicit_then_dep() {
        let m = parse(SAMPLE).unwrap();
        // Explicit [namespaces] override beats [dependencies].
        let p = resolve_namespace(&m, "myteam", "utils").unwrap();
        assert_eq!(p, PathBuf::from("./vendor/myteam"));
        // No explicit [namespaces] for huggingface; falls back to
        // [dependencies], but it has no `path` -> None.
        assert!(resolve_namespace(&m, "huggingface", "client").is_none());
        // Unregistered -> None.
        assert!(resolve_namespace(&m, "unknown", "x").is_none());
    }
    // ---- P3-009d: ManifestError Display + source + validation edge cases ----

    #[test]
    fn manifest_error_display_toml_variant() {
        // Build a Toml variant by parsing invalid TOML.
        let err = parse("not = [valid").unwrap_err();
        let s = err.to_string();
        assert!(s.starts_with("manifest TOML parse error: "), "got: {}", s);
    }

    #[test]
    fn manifest_error_display_invalid_package_name() {
        let e = ManifestError::InvalidPackageName("Bad Name".into());
        let s = e.to_string();
        assert!(s.contains("\"Bad Name\""), "got: {}", s);
        assert!(s.contains("must match"), "got: {}", s);
    }

    #[test]
    fn manifest_error_display_invalid_namespace_name() {
        let e = ManifestError::InvalidNamespaceName("MyTeam".into());
        let s = e.to_string();
        assert!(s.contains("\"MyTeam\""), "got: {}", s);
        assert!(s.contains("namespace"), "got: {}", s);
    }

    #[test]
    fn manifest_error_display_invalid_dependency_key() {
        let e = ManifestError::InvalidDependencyKey("myteam utils".into());
        let s = e.to_string();
        assert!(s.contains("\"myteam utils\""), "got: {}", s);
        assert!(s.contains("<namespace>:<name>"), "got: {}", s);
    }

    #[test]
    fn manifest_error_display_empty_dependency() {
        let e = ManifestError::EmptyDependency("myteam:utils".into());
        let s = e.to_string();
        assert!(s.contains("\"myteam:utils\""), "got: {}", s);
        assert!(s.contains("neither"), "got: {}", s);
    }

    #[test]
    fn manifest_error_display_missing_entry() {
        let e = ManifestError::MissingEntry;
        assert_eq!(e.to_string(), "[package] entry is missing or empty");
    }

    #[test]
    fn manifest_error_source_toml_variant_returns_inner() {
        let err = parse("not = [valid").unwrap_err();
        if let ManifestError::Toml(t) = &err {
            // The std::error::Error::source should hand back the toml error.
            let src = std::error::Error::source(&err);
            assert!(src.is_some());
            // The source must be the same toml::de::Error we have.
            let some_src = src.unwrap();
            let _ = some_src; // type-check only
            // The toml error must be displayable.
            assert!(!t.to_string().is_empty());
        } else {
            panic!("expected Toml variant, got {:?}", err);
        }
    }

    #[test]
    fn manifest_error_source_non_toml_returns_none() {
        let err = ManifestError::MissingEntry;
        assert!(std::error::Error::source(&err).is_none());
        let err = ManifestError::EmptyDependency("x".into());
        assert!(std::error::Error::source(&err).is_none());
    }

    #[test]
    fn rejects_empty_package_name() {
        // Empty name fails the lowercase-plus-hyphen check (the empty
        // string doesn't start with a letter).
        let err = parse(r#"
[package]
name = ""
version = "0.1.0"
entry = "main.wl"
"#).unwrap_err();
        assert!(matches!(err, ManifestError::InvalidPackageName(_)));
    }

    #[test]
    fn rejects_invalid_namespace_via_dep_key() {
        // The namespace is the part before ':' in a dep key. If it
        // doesn't match the lowercase rule, surface InvalidNamespaceName.
        let err = parse(r#"
[package]
name = "ok"
version = "0.1.0"
entry = "main.wl"

[dependencies]
"MyTeam:utils" = { path = "../utils" }
"#).unwrap_err();
        assert!(matches!(err, ManifestError::InvalidNamespaceName(_)));
    }
}
