//! `wlwl.toml` manifest parser (v0.3 §13.8).
//!
//! Schema (the on-disk format is TOML; see spec §13.8):
//!
//! ```toml
//! [package]
//! name = "myapp"
//! version = "0.1.0"
//! entry = "src/main.wll"
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
    /// v0.4 §13.8 新增:本包依赖的 WLWL 语义版本。规范钉为**必填**;
    /// 实现按 Option 反序列化以兼容 v0.3 时代的 manifest,缺失时
    /// 跳过校验(偏差记录:deviations P4-C3-001)。存在时由
    /// `check_language_version` 在加载期校验,不匹配 → E0044。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language_version: Option<String>,
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
    /// containing `<name>.wll`.
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
                write!(
                    f,
                    "invalid package name {:?}: must match ^[a-z][a-z0-9-]*$",
                    n
                )
            }
            ManifestError::InvalidNamespaceName(n) => {
                write!(f, "invalid namespace {:?}: must match ^[a-z][a-z0-9-]*$", n)
            }
            ManifestError::InvalidDependencyKey(k) => {
                write!(
                    f,
                    "invalid dependency key {:?}: must be <namespace>:<name>",
                    k
                )
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

/// The raw `[features]` table, independent of every other block.
///
/// [`Manifest::features`] is exactly this type; the alias exists so the
/// feature resolvers can be written once against the table instead of
/// against the full manifest (see [`parse_features`]).
pub type FeaturesTable = BTreeMap<String, toml::Value>;

/// A deliberately **partial** view of `wlwl.toml` that reads nothing but
/// the `[features]` table.
///
/// [v0.10.1 / R10-010] Why this exists: [`Package`]'s `name` / `version` /
/// `entry` carry no `#[serde(default)]`, so a `[features]`-only manifest
/// fails inside `toml::from_str` -- *before* `validate` ever runs. The
/// two feature loaders in `wlwl-cli` swallowed that `Err` and fell back to
/// `Off`, which made `gradual_typing = "error"` a silent no-op: the static
/// pass never ran, the exit code stayed `0`, and nothing was printed. The
/// user had written the documented incantation verbatim (CHANGELOG §0.2,
/// `SKILL.md` §41-43, spec §9.4) and still got a green build.
///
/// This type lets those loaders honour the feature table even when the
/// rest of the manifest is incomplete. It **does not** relax [`parse`]:
/// package validation is unchanged and still owns every other caller
/// (dependency resolution, lock files, `wlwl build`, module loading). A
/// `[features]`-only manifest is still not a buildable project -- it just
/// no longer silently discards the switch the author did write.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct FeaturesOnly {
    #[serde(default)]
    pub features: FeaturesTable,
}

/// Read just the `[features]` table, ignoring `[package]` entirely.
///
/// Returns `Err` only when the TOML itself is malformed, in which case the
/// feature table cannot be recovered at all.
pub fn parse_features(s: &str) -> Result<FeaturesOnly, ManifestError> {
    Ok(toml::from_str::<FeaturesOnly>(s)?)
}

/// The value shape a `[features]` key is expected to have.
///
/// A key that exists but carries the wrong shape is as broken as a key that
/// doesn't exist at all: `strict_types = "yes"` reads as `false` and
/// `channel_large_buf_threshold = "big"` reads as the default, both with zero
/// diagnostics. So both failure modes are reported together.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeatureShape {
    Boolean,
    /// One of the three string tiers; the exact set is per-key.
    Tier,
    /// A non-negative integer.
    NonNegativeInt,
}

impl FeatureShape {
    /// Human-readable form used in the `W0001` message.
    pub fn describe(self) -> &'static str {
        match self {
            FeatureShape::Boolean => "BOOLEAN (`true` / `false`)",
            FeatureShape::Tier => "STRING (`\"off\"` | `\"warn\"` | `\"error\"`)",
            FeatureShape::NonNegativeInt => "non-negative INTEGER",
        }
    }

    fn matches(self, v: &toml::Value) -> bool {
        match self {
            FeatureShape::Boolean => matches!(v, toml::Value::Boolean(_)),
            FeatureShape::Tier => matches!(v, toml::Value::String(_)),
            FeatureShape::NonNegativeInt => matches!(v, toml::Value::Integer(n) if *n >= 0),
        }
    }
}

/// Every `[features]` key the implementation actually reads, with its shape.
///
/// [v0.10.2] This table is the single source of truth for "is this key real?".
/// It had to exist because the seven accessors each did their own
/// `features.get("…")` and **none of them ever looked at the leftovers**: a
/// misspelled key was accepted, unread, and silently fell back to the default
/// — the user wrote the right semantics, misspelled the key, got the opposite
/// behaviour, and was told nothing.
///
/// The `gradual_typing` / `match_exhaustiveness` *value* checks already exist
/// (`GradualTypingSetting::invalid_value` and its twin), so this table covers
/// them at the shape level only — the tier vocabulary is still validated where
/// it already was, to keep one implementation of "which value means which tier".
pub const KNOWN_FEATURE_KEYS: [(&str, FeatureShape); 7] = [
    ("allow_builtin_shadow", FeatureShape::Boolean),
    ("strict_types", FeatureShape::Boolean),
    ("strict_deadlock_detect", FeatureShape::Boolean),
    ("native_channel_close", FeatureShape::Boolean),
    ("gradual_typing", FeatureShape::Tier),
    ("match_exhaustiveness", FeatureShape::Tier),
    ("channel_large_buf_threshold", FeatureShape::NonNegativeInt),
];

/// Problems found in a `[features]` table, as `(key, reason)` pairs.
///
/// `reason` is already user-facing prose — callers render it into `W0001`
/// directly rather than re-deriving the wording, so the message can never
/// drift from the check that produced it.
pub type FeatureProblems = Vec<(String, String)>;

/// Check a raw feature table against [`KNOWN_FEATURE_KEYS`].
///
/// Shared by [`Manifest`] and [`FeaturesOnly`] so the two can't drift — the
/// same discipline as `resolve_gradual_typing` (R10-010).
pub fn check_feature_table(features: &FeaturesTable) -> FeatureProblems {
    let mut out = FeatureProblems::new();
    // BTreeMap iteration is already sorted, so output order is deterministic.
    for (key, value) in features {
        match KNOWN_FEATURE_KEYS.iter().find(|(k, _)| k == key) {
            None => {
                let known: Vec<&str> = KNOWN_FEATURE_KEYS.iter().map(|(k, _)| *k).collect();
                out.push((
                    key.clone(),
                    format!(
                        "unknown [features] key `{key}` (no such key is read by wlwl); \
                         known keys: {}",
                        known.join(" / ")
                    ),
                ));
            }
            Some((_, shape)) if !shape.matches(value) => {
                out.push((
                    key.clone(),
                    format!(
                        "[features] key `{key}` expects {}, got {}",
                        shape.describe(),
                        value.type_str()
                    ),
                ));
            }
            Some(_) => {}
        }
    }
    out
}

impl FeaturesOnly {
    /// Same three-level resolution as [`Manifest::gradual_typing`].
    pub fn gradual_typing(&self) -> GradualTypingSetting {
        resolve_gradual_typing(&self.features)
    }

    /// Same three-level resolution as [`Manifest::match_exhaustiveness`].
    pub fn match_exhaustiveness(&self) -> MatchExhaustivenessSetting {
        resolve_match_exhaustiveness(&self.features)
    }

    /// Same resolution as [`Manifest::strict_types`].
    ///
    /// Kept as a method rather than exposing the raw `toml::Value` so that
    /// callers (notably `wlwl-cli`) never need a `toml` dependency.
    pub fn strict_types(&self) -> bool {
        matches!(
            self.features.get("strict_types"),
            Some(toml::Value::Boolean(true))
        )
    }

    /// [v0.10.2] Unknown keys / wrong value shapes in `[features]`.
    pub fn problems(&self) -> FeatureProblems {
        check_feature_table(&self.features)
    }
}

/// Resolve `[features] gradual_typing` from a raw feature table.
///
/// Shared by [`Manifest::gradual_typing`] and [`FeaturesOnly::gradual_typing`]
/// so the two can never drift: the "which value means which tier" decision
/// must have exactly one implementation (R10-010).
pub fn resolve_gradual_typing(features: &FeaturesTable) -> GradualTypingSetting {
    let Some(raw) = features.get("gradual_typing") else {
        return GradualTypingSetting::default();
    };
    let Some(text) = raw.as_str() else {
        return GradualTypingSetting::rejected(raw.to_string());
    };
    match text.trim().to_ascii_lowercase().as_str() {
        "off" => GradualTypingSetting::resolved(GradualTyping::Off),
        "warn" => GradualTypingSetting::resolved(GradualTyping::Warn),
        "error" => GradualTypingSetting::resolved(GradualTyping::Error),
        _ => GradualTypingSetting::rejected(text.to_string()),
    }
}

/// Resolve `[features] match_exhaustiveness` from a raw feature table.
///
/// Shared counterpart of [`resolve_gradual_typing`]; the key-absent case
/// follows `gradual_typing` (plan §5.1), so it re-resolves that first.
pub fn resolve_match_exhaustiveness(features: &FeaturesTable) -> MatchExhaustivenessSetting {
    let gradual = resolve_gradual_typing(features);
    let Some(raw) = features.get("match_exhaustiveness") else {
        return MatchExhaustivenessSetting::following(gradual);
    };
    let Some(text) = raw.as_str() else {
        return MatchExhaustivenessSetting::rejected(raw.to_string(), gradual);
    };
    match text.trim().to_ascii_lowercase().as_str() {
        "off" => MatchExhaustivenessSetting::resolved(MatchExhaustiveness::Off),
        "warn" => MatchExhaustivenessSetting::resolved(MatchExhaustiveness::Warn),
        "error" => MatchExhaustivenessSetting::resolved(MatchExhaustiveness::Error),
        _ => MatchExhaustivenessSetting::rejected(text.to_string(), gradual),
    }
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

/// The WLWL language version this implementation supports (spec v0.4).
/// `check_language_version` compares a package's declared
/// `language_version` against this.
pub const SUPPORTED_LANGUAGE_VERSION: (u64, u64) = (0, 4);

/// A `language_version` mismatch (spec v0.4 §13.8). The message the
/// caller renders is exactly the spec's E0044 wording:
/// `language_version mismatch: package requires X.Y, implementation supports A.B`.
#[derive(Debug, Clone, PartialEq)]
pub struct VersionMismatch {
    /// The version the package declared (normalized to `X.Y`).
    pub required: String,
    /// The version this implementation supports (`A.B`).
    pub supported: String,
}

impl fmt::Display for VersionMismatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "language_version mismatch: package requires {}, implementation supports {}",
            self.required, self.supported
        )
    }
}

/// Parse a `language_version` string (`"X.Y"` / `"X.Y.Z"` / leading
/// `v` tolerated) into `(major, minor)`. Returns `None` when the
/// string does not carry at least `X.Y`.
fn parse_language_version(s: &str) -> Option<(u64, u64)> {
    let s = s.trim().trim_start_matches('v');
    let mut it = s.split('.');
    let major = it.next()?.trim().parse::<u64>().ok()?;
    let minor = it.next().unwrap_or("0").trim().parse::<u64>().ok()?;
    Some((major, minor))
}

/// Validate the manifest's declared `language_version` against
/// `SUPPORTED_LANGUAGE_VERSION` (spec v0.4 §13.8, Phase C3).
///
/// Compatibility rule: same major, and the package's minor must be
/// **≤** the implementation's minor (an implementation supports its
/// own minor and all lower ones). `None` (field absent) is tolerated
/// for v0.3-era manifests — see deviations P4-C3-001.
pub fn check_language_version(m: &Manifest) -> Result<(), VersionMismatch> {
    let Some(declared) = m.package.language_version.as_deref() else {
        return Ok(());
    };
    let supported = format!(
        "{}.{}",
        SUPPORTED_LANGUAGE_VERSION.0, SUPPORTED_LANGUAGE_VERSION.1
    );
    let Some((req_major, req_minor)) = parse_language_version(declared) else {
        // Unparseable version strings cannot be honored — treat as a
        // mismatch so the user sees E0044 with both versions spelled
        // out instead of a silently ignored field.
        return Err(VersionMismatch {
            required: declared.to_string(),
            supported,
        });
    };
    if req_major == SUPPORTED_LANGUAGE_VERSION.0 && req_minor <= SUPPORTED_LANGUAGE_VERSION.1 {
        return Ok(());
    }
    Err(VersionMismatch {
        required: format!("{}.{}", req_major, req_minor),
        supported,
    })
}

/// `[features] gradual_typing` 的三档取值(v0.10 Step 2 / ADR-0020 A3)。
///
/// 与运行时 `strict_types`(布尔)是**两个正交概念**:
/// - `strict_types` = 运行时按 `TYPE` 名做顶层形状比对(ADR-0010),默认 `false`;
/// - `gradual_typing` = 编译期静态 pass 的严重级(ADR-0020 A3),默认 `Off`。
///
/// 两者可同时开启,先静态拦、再运行时兜底;互不影响。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GradualTyping {
    /// 不跑静态 pass(默认)。保证 v0.9 程序行为与性能完全不变。
    #[default]
    Off,
    /// 跑静态 pass,失配发 `W0110`-`W0112`,不阻塞退出码。
    Warn,
    /// 跑静态 pass,失配发 `E0110`-`E0112`,阻塞退出码。
    Error,
}

impl GradualTyping {
    /// 清单里的规范拼写(与 `wlwl.toml` 写法一致)。
    pub fn as_str(&self) -> &'static str {
        match self {
            GradualTyping::Off => "off",
            GradualTyping::Warn => "warn",
            GradualTyping::Error => "error",
        }
    }
}

/// `gradual_typing` 的读取结果 + 非法值回执。
///
/// 清单里写了无法识别的值时**回落 `Off` 而不是报错**:开关写错不该让程序
/// 跑不起来(与 `strict_types` 的非布尔值回落 `false` 同一先例)。但回落
/// 必须是**可见的** —— `invalid_value` 携带原始文本,调用方据此发一条
/// 诊断,而不是静默吞掉用户的笔误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GradualTypingSetting {
    mode: GradualTyping,
    invalid: Option<String>,
}

impl GradualTypingSetting {
    /// 默认档(键缺失 / 合法取值均由此构造)。
    fn resolved(mode: GradualTyping) -> GradualTypingSetting {
        GradualTypingSetting {
            mode,
            invalid: None,
        }
    }

    /// 非法值已回落到 `Off`,并携带原始文本。
    fn rejected(raw: impl Into<String>) -> GradualTypingSetting {
        GradualTypingSetting {
            mode: GradualTyping::Off,
            invalid: Some(raw.into()),
        }
    }

    /// 生效档位。
    pub fn mode(&self) -> GradualTyping {
        self.mode
    }

    /// 是否需要跑静态 pass。`off` 时为 `false` —— 调用方据此**完全不调用**
    /// checker,这是「默认零破坏、零开销」的实现点。
    pub fn is_enabled(&self) -> bool {
        self.mode != GradualTyping::Off
    }

    /// 清单里的非法值原文(已回落到 `off`);合法或缺省时为 `None`。
    pub fn invalid_value(&self) -> Option<&str> {
        self.invalid.as_deref()
    }
}

impl Default for GradualTypingSetting {
    fn default() -> GradualTypingSetting {
        GradualTypingSetting::resolved(GradualTyping::Off)
    }
}

/// v0.10 Step 8(计划书 §5.1)`[features] match_exhaustiveness` 的三档取值。
///
/// 与 [`GradualTyping`] 同构但**可缺省跟随**:`match_exhaustiveness` 不写
/// 时取 `gradual_typing` 的档(计划书原文「默认随 `gradual_typing`」),
/// 写了才独立生效。这样「渐进类型」这一个总开关就够用,需要单独关掉
/// MATCH 检查时再显式写 `"off"`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MatchExhaustiveness {
    /// 不跑 MATCH 穷尽性 / 可达性检查。
    #[default]
    Off,
    /// 缺构造子发 `W0116`;不可达子句恒发 `W0117`。不阻塞退出码。
    Warn,
    /// 缺构造子发 `E0116`(**阻塞**);不可达子句**仍恒为** `W0117`。
    Error,
}

impl MatchExhaustiveness {
    /// 清单里的规范拼写(与 `wlwl.toml` 写法一致)。
    pub fn as_str(&self) -> &'static str {
        match self {
            MatchExhaustiveness::Off => "off",
            MatchExhaustiveness::Warn => "warn",
            MatchExhaustiveness::Error => "error",
        }
    }
}

/// `match_exhaustiveness` 的读取结果:生效档位 + 非法值回执。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchExhaustivenessSetting {
    mode: MatchExhaustiveness,
    invalid: Option<String>,
}

impl MatchExhaustivenessSetting {
    /// 键缺失 → **跟随 `gradual_typing`**(而不是回落 `off`)。
    pub fn following(fallback: GradualTypingSetting) -> MatchExhaustivenessSetting {
        MatchExhaustivenessSetting {
            mode: match fallback.mode() {
                GradualTyping::Off => MatchExhaustiveness::Off,
                GradualTyping::Warn => MatchExhaustiveness::Warn,
                GradualTyping::Error => MatchExhaustiveness::Error,
            },
            invalid: None,
        }
    }

    fn resolved(mode: MatchExhaustiveness) -> MatchExhaustivenessSetting {
        MatchExhaustivenessSetting {
            mode,
            invalid: None,
        }
    }

    /// 非法值已回落到「跟随 `gradual_typing`」,并携带原始文本。
    fn rejected(
        raw: impl Into<String>,
        fallback: GradualTypingSetting,
    ) -> MatchExhaustivenessSetting {
        MatchExhaustivenessSetting {
            mode: Self::following(fallback).mode,
            invalid: Some(raw.into()),
        }
    }

    /// 生效档位。
    pub fn mode(&self) -> MatchExhaustiveness {
        self.mode
    }

    /// 是否需要跑 MATCH 检查。`off` 时为 `false`。
    pub fn is_enabled(&self) -> bool {
        self.mode != MatchExhaustiveness::Off
    }

    /// 清单里的非法值原文(已回落到跟随 `gradual_typing`);合法或缺省时为 `None`。
    pub fn invalid_value(&self) -> Option<&str> {
        self.invalid.as_deref()
    }
}

impl Manifest {
    /// v0.4 §6.6 / §13.8 `[features] allow_builtin_shadow` flag
    /// (Phase C5). Defaults to `false` — shadowing a global builtin
    /// raises E0025 unless this is `true` (then W0030 instead).
    pub fn allow_builtin_shadow(&self) -> bool {
        matches!(
            self.features.get("allow_builtin_shadow"),
            Some(toml::Value::Boolean(true))
        )
    }
    /// v0.4 §2.7 / §13.8 `[features] strict_types` flag (Phase E1).
    /// Defaults to `false`; when `true`, the evaluator inserts
    /// transient cast checks at three boundaries (per spec §2.7):
    ///
    /// 1. **Function entry (public API)** -- param type annotation vs
    ///    actual `TYPE` of the argument -> `E0033`.
    /// 2. **IMPORT boundary** -- imported binding's actual value vs
    ///    the contract type declared by the exporting module -> `E0033`.
    /// 3. **FFI boundary** -- out of scope for v0.4 (no FFI).
    ///
    /// When `false` (the default), behavior is identical to v0.3:
    /// type annotations are parsed but ignored at runtime, no
    /// boundary checks fire.
    ///
    /// The v0.4 spec also pegs an engineering budget
    /// (non-strict >= 0% overhead, strict <= 10% overhead); this is
    /// not normative -- see spec §2.7 last paragraph.
    pub fn strict_types(&self) -> bool {
        matches!(
            self.features.get("strict_types"),
            Some(toml::Value::Boolean(true))
        )
    }

    /// v0.10 Step 2 / ADR-0020 A3 `[features] gradual_typing` 开关。
    ///
    /// 取值 `"off"`(默认)/ `"warn"` / `"error"`,大小写与首尾空白不敏感。
    /// **只读**:本方法不写回清单,也不碰依赖求解 / 锁文件 —— `wlwl-toml`
    /// 的 manifest / lock / MVS 面按 ADR-0020 Decision 5 冻结。
    ///
    /// 非法值(非字符串、或不在三档之内)回落到 `Off` 并把原文放进
    /// [`GradualTypingSetting::invalid_value`],由调用方发诊断。
    pub fn gradual_typing(&self) -> GradualTypingSetting {
        resolve_gradual_typing(&self.features)
    }

    /// v0.10 Step 8 / 计划书 §5.1 `[features] match_exhaustiveness` 开关。
    ///
    /// 取值 `"off"` / `"warn"` / `"error"`,大小写与首尾空白不敏感。
    /// **键缺失时跟随 `gradual_typing`**(计划书原文),非法值回落到「跟随」
    /// 并把原文放进 [`MatchExhaustivenessSetting::invalid_value`],由调用方
    /// 发诊断 —— 与 `gradual_typing` 同一套「不静默吞笔误」的纪律。
    ///
    /// **只读**:与 `gradual_typing` 一样不碰依赖求解 / 锁文件
    /// (ADR-0020 Decision 5)。
    pub fn match_exhaustiveness(&self) -> MatchExhaustivenessSetting {
        resolve_match_exhaustiveness(&self.features)
    }

    /// v0.9 `[features] strict_deadlock_detect` (ADR-0017 §3.4).
    /// Defaults to `true` — L1 deadlock surfaces as top-level `E0065`.
    /// Set `false` to downgrade to the soft `W0065` warning plus the
    /// legacy `E0053` "no peer" diagnostic (dev-mode opt-out).
    pub fn strict_deadlock_detect(&self) -> bool {
        !matches!(
            self.features.get("strict_deadlock_detect"),
            Some(toml::Value::Boolean(false))
        )
    }

    /// v0.9 `[features] native_channel_close` (ADR-0018 opt-in).
    /// Defaults to `false` — post-close `CHANNEL_RECV` returns the
    /// structured `ERR(kind="ChannelClosed")` payload. When `true`,
    /// post-close `CHANNEL_RECV` raises a hard diagnostic instead
    /// (v0.9.0 reuses `E0054`; `E0055` stays removed from the
    /// registry per ADR-0018 option B).
    pub fn native_channel_close(&self) -> bool {
        matches!(
            self.features.get("native_channel_close"),
            Some(toml::Value::Boolean(true))
        )
    }

    /// v0.9 `[features] channel_large_buf_threshold = N` (plan §5.3).
    /// `CHANNEL_NEW(buf)` with `buf > threshold` emits `W0066`. The
    /// default threshold is 1024; `0` disables the warning.
    pub fn channel_large_buf_threshold(&self) -> usize {
        match self.features.get("channel_large_buf_threshold") {
            Some(toml::Value::Integer(n)) if *n >= 0 => *n as usize,
            _ => DEFAULT_LARGE_BUF_THRESHOLD,
        }
    }

    /// [v0.10.2] Unknown keys / wrong value shapes in `[features]`.
    ///
    /// The evaluator reads four of these keys (`allow_builtin_shadow`,
    /// `strict_deadlock_detect`, `native_channel_close`,
    /// `channel_large_buf_threshold`) and `wlwl-cli` reads the rest. Neither
    /// side used to validate the key *names*, so a typo silently disabled
    /// whatever it was meant to enable. The CLI surfaces this as `W0001`.
    pub fn problems(&self) -> FeatureProblems {
        check_feature_table(&self.features)
    }
}

/// Default `CHANNEL_NEW` buffer size above which `W0066` fires.
pub const DEFAULT_LARGE_BUF_THRESHOLD: usize = 1024;

/// Resolve a `<namespace>:<name>` reference to a local directory,
/// using `[namespaces]` as an override and `[dependencies]` as the
/// fallback. Returns the directory relative to the manifest path
/// (the caller is responsible for joining with the manifest
/// directory's parent).
///
/// Returns `None` if the reference is not registered; the caller
/// surfaces E0040 (not found) or E0043 (namespace syntax).
pub fn resolve_namespace(manifest: &Manifest, ns: &str, name: &str) -> Option<PathBuf> {
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

    // ---- [v0.10.2] unknown-key / wrong-shape detection --------------
    //
    // The bug these lock: every accessor did its own `features.get("…")`
    // and none of them looked at the leftovers, so a misspelled key was
    // accepted, unread, and silently fell back to the default.

    #[test]
    fn all_known_keys_produce_no_problems() {
        // Regression guard for the table itself: if `KNOWN_FEATURE_KEYS`
        // ever drops an entry, a *correct* manifest starts warning.
        for (key, shape) in KNOWN_FEATURE_KEYS {
            let value = match shape {
                FeatureShape::Boolean => "true",
                FeatureShape::Tier => "\"off\"",
                FeatureShape::NonNegativeInt => "10",
            };
            let src = format!("[features]\n{key} = {value}\n");
            let f = parse_features(&src).expect("parses");
            assert_eq!(
                f.problems(),
                Vec::new(),
                "a valid `{key}` must not be reported"
            );
        }
    }

    #[test]
    fn misspelled_key_is_reported() {
        // The exact shape that used to be silent: right semantics,
        // misspelled key, opposite behaviour, no diagnostics.
        let f = parse_features("[features]\nnative_channel_clsoe = true\n").expect("parses");
        let problems = f.problems();
        assert_eq!(problems.len(), 1, "expected exactly one problem");
        assert!(
            problems[0].1.contains("native_channel_clsoe"),
            "{}",
            problems[0].1
        );
        assert!(problems[0].1.contains("unknown"), "{}", problems[0].1);
    }

    #[test]
    fn wrong_value_shape_is_reported() {
        for (src, expect_in_msg) in [
            ("[features]\nnative_channel_close = \"yes\"\n", "BOOLEAN"),
            (
                "[features]\nchannel_large_buf_threshold = \"big\"\n",
                "non-negative",
            ),
            (
                "[features]\nchannel_large_buf_threshold = -1\n",
                "non-negative",
            ),
            ("[features]\ngradual_typing = true\n", "STRING"),
        ] {
            let f = parse_features(src).expect("parses");
            let problems = f.problems();
            assert_eq!(problems.len(), 1, "expected one problem for {src:?}");
            assert!(
                problems[0].1.contains(expect_in_msg),
                "{:?} should mention {expect_in_msg}, got {}",
                src,
                problems[0].1
            );
        }
    }

    #[test]
    fn several_bad_keys_are_all_reported_in_sorted_order() {
        let f = parse_features("[features]\nzzz = 1\naaa = 2\n").expect("parses");
        let problems = f.problems();
        assert_eq!(
            problems.len(),
            2,
            "both must be reported, not just the first"
        );
        // BTreeMap order -> deterministic output.
        assert!(problems[0].1.starts_with("unknown [features] key `aaa`"));
        assert!(problems[1].1.starts_with("unknown [features] key `zzz`"));
    }

    #[test]
    fn no_features_table_is_clean() {
        let f = parse_features("[package]\nname = \"p\"\n").expect("parses");
        assert!(f.problems().is_empty());
    }

    #[test]
    fn manifest_problems_matches_features_only() {
        // The two entry points must not drift (same discipline as
        // `resolve_gradual_typing`).
        let src = "[package]\nname = \"p\"\nversion = \"0.1.0\"\nentry = \"m.wll\"\n\n\
                   [features]\ntypo_key = true\n";
        let m = parse(src).expect("valid manifest");
        let f = parse_features(src).expect("valid features");
        assert_eq!(m.problems(), f.problems());
    }

    const SAMPLE: &str = r#"
[package]
name = "myapp"
version = "0.1.0"
entry = "src/main.wll"
language_version = "0.4"
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
        assert_eq!(m.package.entry, "src/main.wll");
        assert_eq!(m.package.language_version.as_deref(), Some("0.4"));
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
        let m = parse(
            r#"
[package]
name = "tiny"
version = "0.0.1"
entry = "main.wll"
"#,
        )
        .unwrap();
        assert!(m.dependencies.is_empty());
        assert!(m.namespaces.is_empty());
        assert!(m.features.is_empty());
    }

    #[test]
    fn rejects_uppercase_package_name() {
        let err = parse(
            r#"
[package]
name = "MyApp"
version = "0.1.0"
entry = "main.wll"
"#,
        )
        .unwrap_err();
        assert!(matches!(err, ManifestError::InvalidPackageName(_)));
    }

    #[test]
    fn rejects_empty_entry() {
        let err = parse(
            r#"
[package]
name = "ok"
version = "0.1.0"
entry = ""
"#,
        )
        .unwrap_err();
        assert!(matches!(err, ManifestError::MissingEntry));
    }

    #[test]
    fn rejects_dep_key_without_colon() {
        let err = parse(
            r#"
[package]
name = "ok"
version = "0.1.0"
entry = "main.wll"

[dependencies]
"myteam utils" = { path = "../utils" }
"#,
        )
        .unwrap_err();
        assert!(matches!(err, ManifestError::InvalidDependencyKey(_)));
    }

    #[test]
    fn rejects_dep_with_neither_path_nor_version() {
        let err = parse(
            r#"
[package]
name = "ok"
version = "0.1.0"
entry = "main.wll"

[dependencies]
"myteam:utils" = { optional = true }
"#,
        )
        .unwrap_err();
        assert!(matches!(err, ManifestError::EmptyDependency(_)));
    }

    #[test]
    fn rejects_invalid_namespace_name() {
        let err = parse(
            r#"
[package]
name = "ok"
version = "0.1.0"
entry = "main.wll"

[namespaces]
"MyTeam" = "./vendor/myteam"
"#,
        )
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
        let err = parse(
            r#"
[package]
name = ""
version = "0.1.0"
entry = "main.wll"
"#,
        )
        .unwrap_err();
        assert!(matches!(err, ManifestError::InvalidPackageName(_)));
    }

    #[test]
    fn rejects_invalid_namespace_via_dep_key() {
        // The namespace is the part before ':' in a dep key. If it
        // doesn't match the lowercase rule, surface InvalidNamespaceName.
        let err = parse(
            r#"
[package]
name = "ok"
version = "0.1.0"
entry = "main.wll"

[dependencies]
"MyTeam:utils" = { path = "../utils" }
"#,
        )
        .unwrap_err();
        assert!(matches!(err, ManifestError::InvalidNamespaceName(_)));
    }

    // ---- Phase C3 (spec v0.4 §13.8): language_version ----

    #[test]
    fn language_version_absent_is_tolerated() {
        let m = parse(
            r#"
[package]
name = "tiny"
version = "0.0.1"
entry = "main.wll"
"#,
        )
        .unwrap();
        assert!(m.package.language_version.is_none());
        assert!(check_language_version(&m).is_ok());
    }

    #[test]
    fn language_version_same_and_lower_minor_ok() {
        for v in ["0.4", "0.3", "0.4.1", "v0.2"] {
            let m = parse(&format!(
                r#"
[package]
name = "tiny"
version = "0.0.1"
entry = "main.wll"
language_version = "{}"
"#,
                v
            ))
            .unwrap();
            assert!(
                check_language_version(&m).is_ok(),
                "{} should be compatible",
                v
            );
        }
    }

    #[test]
    fn language_version_mismatch_higher_minor() {
        let m = parse(
            r#"
[package]
name = "tiny"
version = "0.0.1"
entry = "main.wll"
language_version = "0.5"
"#,
        )
        .unwrap();
        let err = check_language_version(&m).unwrap_err();
        assert_eq!(err.required, "0.5");
        assert_eq!(err.supported, "0.4");
        assert_eq!(
            err.to_string(),
            "language_version mismatch: package requires 0.5, implementation supports 0.4"
        );
    }

    #[test]
    fn language_version_mismatch_major() {
        let m = parse(
            r#"
[package]
name = "tiny"
version = "0.0.1"
entry = "main.wll"
language_version = "1.0"
"#,
        )
        .unwrap();
        assert!(check_language_version(&m).is_err());
    }

    #[test]
    fn language_version_unparseable_is_mismatch() {
        let m = parse(
            r#"
[package]
name = "tiny"
version = "0.0.1"
entry = "main.wll"
language_version = "banana"
"#,
        )
        .unwrap();
        let err = check_language_version(&m).unwrap_err();
        assert_eq!(err.required, "banana");
    }

    // ---- Phase C5 (spec v0.4 §6.6 / §13.8): allow_builtin_shadow ----

    #[test]
    fn allow_builtin_shadow_defaults_false() {
        let m = parse(
            r#"
[package]
name = "tiny"
version = "0.0.1"
entry = "main.wll"
"#,
        )
        .unwrap();
        assert!(!m.allow_builtin_shadow());
    }

    #[test]
    fn allow_builtin_shadow_true_when_flagged() {
        let m = parse(
            r#"
[package]
name = "tiny"
version = "0.0.1"
entry = "main.wll"

[features]
allow_builtin_shadow = true
"#,
        )
        .unwrap();
        assert!(m.allow_builtin_shadow());
    }

    #[test]
    fn allow_builtin_shadow_false_value_is_false() {
        let m = parse(
            r#"
[package]
name = "tiny"
version = "0.0.1"
entry = "main.wll"

[features]
allow_builtin_shadow = false
"#,
        )
        .unwrap();
        assert!(!m.allow_builtin_shadow());
    }
    // ---- Phase E1 (spec v0.4 §2.7 / §13.8): strict_types ----

    #[test]
    fn strict_types_defaults_false() {
        let m = parse(
            r#"
[package]
name = "tiny"
version = "0.0.1"
entry = "main.wll"
"#,
        )
        .unwrap();
        assert!(!m.strict_types());
    }

    #[test]
    fn strict_types_true_when_flagged() {
        let m = parse(
            r#"
[package]
name = "tiny"
version = "0.0.1"
entry = "main.wll"

[features]
strict_types = true
"#,
        )
        .unwrap();
        assert!(m.strict_types());
    }

    #[test]
    fn strict_types_false_value_is_false() {
        let m = parse(
            r#"
[package]
name = "tiny"
version = "0.0.1"
entry = "main.wll"

[features]
strict_types = false
"#,
        )
        .unwrap();
        assert!(!m.strict_types());
    }

    #[test]
    fn strict_types_non_boolean_is_false() {
        // Spec §13.8 — `strict_types` is a boolean feature. A non-boolean
        // value (string / int / table) means "no strict_types".
        // We do NOT raise an error at the manifest level; the engine
        // simply treats the feature as off. This mirrors the
        // `allow_builtin_shadow` precedent.
        let m = parse(
            r#"
[package]
name = "tiny"
version = "0.0.1"
entry = "main.wll"

[features]
strict_types = "yes"
"#,
        )
        .unwrap();
        assert!(!m.strict_types());

        let m = parse(
            r#"
[package]
name = "tiny"
version = "0.0.1"
entry = "main.wll"

[features]
strict_types = 1
"#,
        )
        .unwrap();
        assert!(!m.strict_types());
    }

    #[test]
    fn strict_types_coexists_with_other_features() {
        // Co-existence with allow_builtin_shadow (Phase C5) -- both
        // can be set independently; one does not imply the other.
        let m = parse(
            r#"
[package]
name = "tiny"
version = "0.0.1"
entry = "main.wll"

[features]
strict_types = true
allow_builtin_shadow = true
"#,
        )
        .unwrap();
        assert!(m.strict_types());
        assert!(m.allow_builtin_shadow());
    }

    // ---- v0.10 Step 2 (ADR-0020 A3 / decision D-1): gradual_typing ----

    fn manifest_with_feature(value: &str) -> Manifest {
        let src = format!(
            r#"
[package]
name = "tiny"
version = "0.0.1"
entry = "main.wll"

[features]
gradual_typing = {value}
"#
        );
        parse(&src).expect("manifest must parse")
    }

    #[test]
    fn gradual_typing_defaults_to_off() {
        let m = parse(
            r#"
[package]
name = "tiny"
version = "0.0.1"
entry = "main.wll"
"#,
        )
        .unwrap();
        let setting = m.gradual_typing();
        assert_eq!(setting.mode(), GradualTyping::Off);
        assert!(!setting.is_enabled());
        assert!(setting.invalid_value().is_none());
    }

    #[test]
    fn gradual_typing_reads_all_three_levels() {
        for (raw, expected, enabled) in [
            ("\"off\"", GradualTyping::Off, false),
            ("\"warn\"", GradualTyping::Warn, true),
            ("\"error\"", GradualTyping::Error, true),
        ] {
            let setting = manifest_with_feature(raw).gradual_typing();
            assert_eq!(setting.mode(), expected, "raw: {raw}");
            assert_eq!(setting.is_enabled(), enabled, "raw: {raw}");
            assert!(setting.invalid_value().is_none(), "raw: {raw}");
        }
    }

    #[test]
    fn gradual_typing_is_case_and_whitespace_insensitive() {
        assert_eq!(
            manifest_with_feature("\" WARN \"").gradual_typing().mode(),
            GradualTyping::Warn
        );
        assert_eq!(
            manifest_with_feature("\"Error\"").gradual_typing().mode(),
            GradualTyping::Error
        );
    }

    #[test]
    fn gradual_typing_invalid_value_falls_back_to_off_but_reports() {
        for raw in ["\"true\"", "\"loud\"", "\"\"", "1", "true"] {
            let setting = manifest_with_feature(raw).gradual_typing();
            assert_eq!(
                setting.mode(),
                GradualTyping::Off,
                "must fall back to off for {raw}"
            );
            assert!(!setting.is_enabled(), "must stay disabled for {raw}");
            assert!(
                setting.invalid_value().is_some(),
                "must report the rejected value for {raw}"
            );
        }
    }

    // ---- v0.10.1 (R10-010): [features] 独立于 [package] 生效 ----------

    /// `[features]` 单段的清单:完整解析**必须失败**,宽松解析**必须成功**。
    ///
    /// 这对断言是 R10-010 的全部要害 —— v0.10 的静默失效正是因为
    /// 宽松路径不存在,调用方只有 `parse` 一个选择,失败即回落 `off`。
    #[test]
    fn features_only_manifest_is_rejected_by_parse_but_readable_by_parse_features() {
        let src = "[features]\ngradual_typing = \"error\"\n";
        assert!(
            parse(src).is_err(),
            "a [features]-only file must still not be a valid manifest"
        );
        let f = parse_features(src).expect("features must still be readable");
        assert_eq!(f.gradual_typing().mode(), GradualTyping::Error);
    }

    /// 宽松解析与完整解析对**同一张特性表**给出同样的判定。
    ///
    /// 两套判定一旦漂移,就会出现「补全 `[package]` 后行为变了」这种
    /// 极难查的不一致 —— 所以这里逐档比对,而不是各测各的。
    #[test]
    fn features_only_and_full_manifest_resolve_identically() {
        for value in ["\"off\"", "\"warn\"", "\"error\"", "\" WARN \"", "\"loud\""] {
            let loose_src = format!("[features]\ngradual_typing = {value}\n");
            let full_src = format!(
                "[package]\nname = \"tiny\"\nversion = \"0.0.1\"\nentry = \"main.wll\"\n\n[features]\ngradual_typing = {value}\n"
            );
            let loose = parse_features(&loose_src).unwrap().gradual_typing();
            let full = parse(&full_src).unwrap().gradual_typing();
            assert_eq!(loose, full, "gradual_typing drifted for {value}");
        }
        for value in ["\"off\"", "\"warn\"", "\"error\"", "\"loud\""] {
            let loose_src =
                format!("[features]\ngradual_typing = \"error\"\nmatch_exhaustiveness = {value}\n");
            let full_src = format!(
                "[package]\nname = \"tiny\"\nversion = \"0.0.1\"\nentry = \"main.wll\"\n\n[features]\ngradual_typing = \"error\"\nmatch_exhaustiveness = {value}\n"
            );
            let loose = parse_features(&loose_src).unwrap().match_exhaustiveness();
            let full = parse(&full_src).unwrap().match_exhaustiveness();
            assert_eq!(loose, full, "match_exhaustiveness drifted for {value}");
        }
    }

    /// TOML 本身坏掉时,宽松路径也必须失败 —— 它救不了语法错。
    ///
    /// 调用方据此区分「清单残缺但特性生效」与「什么都读不到」,
    /// 两种情况的回落语义完全不同。
    #[test]
    fn parse_features_rejects_malformed_toml() {
        for bad in [
            "[features\ng gradual_typing = \"error\"\n",
            "[features]\ngradual_typing = \n",
            "not toml at all ===",
        ] {
            assert!(
                parse_features(bad).is_err(),
                "must reject malformed TOML: {bad:?}"
            );
        }
    }

    /// `strict_types` 同样不受 `[package]` 缺失影响。
    #[test]
    fn features_only_manifest_still_reads_strict_types() {
        let f = parse_features("[features]\nstrict_types = true\n").unwrap();
        assert!(f.strict_types());
        let off = parse_features("[features]\nstrict_types = false\n").unwrap();
        assert!(!off.strict_types());
        let absent = parse_features("[features]\n").unwrap();
        assert!(!absent.strict_types());
    }

    /// 完全没有 `[features]` 段时,宽松解析给空表而不是报错。
    #[test]
    fn parse_features_tolerates_a_manifest_with_no_features_block() {
        let f = parse_features("[package]\nname=\"a\"\nversion=\"1\"\nentry=\"m.wll\"\n").unwrap();
        assert!(f.features.is_empty());
        assert_eq!(f.gradual_typing().mode(), GradualTyping::Off);
        assert_eq!(f.match_exhaustiveness().mode(), MatchExhaustiveness::Off);
    }

    #[test]
    fn gradual_typing_coexists_with_strict_types() {
        // The two switches are orthogonal: compile-time pass severity
        // and runtime cast check are independent settings.
        let m = parse(
            r#"
[package]
name = "tiny"
version = "0.0.1"
entry = "main.wll"

[features]
gradual_typing = "error"
strict_types = true
"#,
        )
        .unwrap();
        assert_eq!(m.gradual_typing().mode(), GradualTyping::Error);
        assert!(m.strict_types());
    }

    #[test]
    fn gradual_typing_as_str_round_trips() {
        for level in [
            GradualTyping::Off,
            GradualTyping::Warn,
            GradualTyping::Error,
        ] {
            let setting =
                manifest_with_feature(&format!("\"{}\"", level.as_str())).gradual_typing();
            assert_eq!(setting.mode(), level);
        }
    }

    // ---- v0.10 Step 8 / plan §5.1: match_exhaustiveness ----

    fn manifest_with_two_features(gradual: &str, match_exh: &str) -> Manifest {
        let src = format!(
            r#"
[package]
name = "tiny"
version = "0.0.1"
entry = "main.wll"

[features]
gradual_typing = {gradual}
match_exhaustiveness = {match_exh}
"#
        );
        parse(&src).expect("manifest must parse")
    }

    /// 计划书原文:「默认随 `gradual_typing`」。这是本键最重要的一条语义 ——
    /// 少写一个键就该由总开关决定,而不是默默回落 `off`。
    #[test]
    fn match_exhaustiveness_follows_gradual_typing_when_absent() {
        for (gradual, expected) in [
            ("\"off\"", MatchExhaustiveness::Off),
            ("\"warn\"", MatchExhaustiveness::Warn),
            ("\"error\"", MatchExhaustiveness::Error),
        ] {
            let m = manifest_with_feature(gradual);
            let setting = m.match_exhaustiveness();
            assert_eq!(setting.mode(), expected, "gradual: {gradual}");
            assert_eq!(setting.is_enabled(), expected != MatchExhaustiveness::Off);
            assert!(setting.invalid_value().is_none());
        }
    }

    /// 显式写就独立生效,包括「关掉 MATCH 但保留其它静态检查」。
    #[test]
    fn match_exhaustiveness_overrides_gradual_typing() {
        let m = manifest_with_two_features("\"error\"", "\"off\"");
        let setting = m.match_exhaustiveness();
        assert_eq!(setting.mode(), MatchExhaustiveness::Off);
        assert!(!setting.is_enabled());
        // 总开关没被动过。
        assert_eq!(m.gradual_typing().mode(), GradualTyping::Error);
    }

    #[test]
    fn match_exhaustiveness_reads_all_three_levels() {
        for (raw, expected) in [
            ("\"off\"", MatchExhaustiveness::Off),
            ("\" WARN \"", MatchExhaustiveness::Warn),
            ("\"Error\"", MatchExhaustiveness::Error),
        ] {
            let setting = manifest_with_two_features("\"error\"", raw).match_exhaustiveness();
            assert_eq!(setting.mode(), expected, "raw: {raw}");
            assert!(setting.invalid_value().is_none(), "raw: {raw}");
        }
    }

    #[test]
    fn match_exhaustiveness_invalid_value_falls_back_to_following_but_reports() {
        for raw in ["\"true\"", "\"strict\"", "\"\"", "1", "true"] {
            let setting = manifest_with_two_features("\"warn\"", raw).match_exhaustiveness();
            assert_eq!(
                setting.mode(),
                MatchExhaustiveness::Warn,
                "must fall back to following gradual_typing for {raw}"
            );
            assert!(
                setting.invalid_value().is_some(),
                "must report the rejected value for {raw}"
            );
        }
    }

    #[test]
    fn match_exhaustiveness_as_str_round_trips() {
        for level in [
            MatchExhaustiveness::Off,
            MatchExhaustiveness::Warn,
            MatchExhaustiveness::Error,
        ] {
            let setting =
                manifest_with_two_features("\"error\"", &format!("\"{}\"", level.as_str()))
                    .match_exhaustiveness();
            assert_eq!(setting.mode(), level);
        }
    }
}
