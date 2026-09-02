//! `wlwl.lock` — auto-generated dependency lock file (v0.3 §13.8).
//!
//! JSON (not TOML) so the lock is stable across manifest schema
//! evolution: a future `wlwl.toml` field that this batch does not
//! understand does not silently round-trip into a different lock
//! shape.
//!
//! ## Format
//!
//! ```json
//! {
//!   "schema_version": "0.3.1",
//!   "entries": [
//!     {
//!       "name": "myteam:utils",
//!       "path": "../utils",
//!       "version": null,
//!       "hash": "deadbeef…"
//!     },
//!     …
//!   ]
//! }
//! ```
//!
//! ## Hashing
//!
//! `hash` is the lowercase hex SHA-256 of every `<name>.wl` file
//! inside the dependency directory, concatenated in sorted order.
//! This is a deterministic, content-only fingerprint that detects
//! source changes without depending on filesystem metadata. When a
//! version constraint is used (no `path`), `hash` is `None` and
//! the entry is left to the central-registry machinery in v0.4.

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const CURRENT_SCHEMA_VERSION: &str = "0.3.1";

/// A single entry in `wlwl.lock`. The struct is symmetric for path
/// dependencies and (future) version dependencies: exactly one of
/// `path` and `version` is set, but we keep both fields optional
/// so the JSON representation is forward-compatible with v0.4.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LockEntry {
    /// `<namespace>:<name>`, matching the manifest dependency key.
    pub name: String,
    /// Local path, relative to the manifest directory, for
    /// path-style dependencies. `None` for version-style.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// Version constraint string, for version-style dependencies.
    /// `None` for path-style.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Lowercase hex SHA-256 of the dependency's `.wl` source files.
    /// `None` when the dependency has no local path (e.g. central
    /// registry entries in v0.4).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
}

/// Top-level lock file. `schema_version` lets the loader reject
/// locks produced by a future incompatible wlwl build.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Lockfile {
    pub schema_version: String,
    pub entries: Vec<LockEntry>,
}

#[derive(Debug)]
pub enum LockError {
    /// JSON parse failure.
    Json(serde_json::Error),
    /// I/O failure (read or write).
    Io(std::io::Error),
    /// Schema version mismatch (e.g. lock from a future wlwl build).
    UnsupportedSchemaVersion(String),
    /// Tried to hash a dependency directory but no `.wl` files were
    /// found — usually means the path is wrong.
    NoSourceFiles(PathBuf),
}

impl fmt::Display for LockError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LockError::Json(e) => write!(f, "wlwl.lock JSON parse error: {}", e),
            LockError::Io(e) => write!(f, "wlwl.lock I/O error: {}", e),
            LockError::UnsupportedSchemaVersion(v) => {
                write!(f, "wlwl.lock schema_version {} is not supported here", v)
            }
            LockError::NoSourceFiles(p) => {
                write!(f, "no .wl source files in dependency directory {}", p.display())
            }
        }
    }
}

impl std::error::Error for LockError {}

impl From<serde_json::Error> for LockError {
    fn from(e: serde_json::Error) -> Self { LockError::Json(e) }
}
impl From<std::io::Error> for LockError {
    fn from(e: std::io::Error) -> Self { LockError::Io(e) }
}

/// Read and parse a `wlwl.lock` file. Returns `Ok(None)` when the
/// file does not exist (the caller should then generate a fresh
/// lock). `Err` covers parse, IO, and schema-version failures.
pub fn read(path: &Path) -> Result<Option<Lockfile>, LockError> {
    let s = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(LockError::Io(e)),
    };
    let lf: Lockfile = serde_json::from_str(&s)?;
    if lf.schema_version != CURRENT_SCHEMA_VERSION {
        return Err(LockError::UnsupportedSchemaVersion(lf.schema_version));
    }
    Ok(Some(lf))
}

/// Serialise a `Lockfile` to a pretty-printed JSON string.
pub fn to_string_pretty(lf: &Lockfile) -> Result<String, LockError> {
    Ok(serde_json::to_string_pretty(lf)?)
}

/// Write a `Lockfile` atomically: write to `<path>.tmp`, rename.
/// This avoids leaving a half-written lock if the process is killed
/// mid-write (lock files are committed to source control).
pub fn write(path: &Path, lf: &Lockfile) -> Result<(), LockError> {
    let s = to_string_pretty(lf)?;
    let tmp = path.with_extension("lock.tmp");
    fs::write(&tmp, s)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

/// Hash the contents of a dependency directory: lowercase hex
/// SHA-256 over every `.wl` file, sorted by relative path. Returns
/// `None` if the directory does not exist or is empty.
pub fn hash_dependency_dir(dir: &Path) -> Result<Option<String>, LockError> {
    if !dir.is_dir() {
        return Ok(None);
    }
    let mut paths: Vec<PathBuf> = Vec::new();
    collect_wl_files(dir, &mut paths)?;
    if paths.is_empty() {
        return Ok(None);
    }
    paths.sort();
    let mut hasher = Sha256::new();
    for p in &paths {
        let rel = p.strip_prefix(dir).unwrap_or(p);
        // Include the relative path so that renaming a file changes
        // the hash even when the content is identical.
        hasher.update(rel.to_string_lossy().as_bytes());
        hasher.update(b"\0");
        hasher.update(&fs::read(p)?);
    }
    Ok(Some(hex_lower(&hasher.finalize())))
}

fn collect_wl_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), LockError> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_wl_files(&path, out)?;
        } else if path.extension().and_then(|s| s.to_str()) == Some("wl") {
            out.push(path);
        }
    }
    Ok(())
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push(HEX[(b >> 4) as usize] as char);
        s.push(HEX[(b & 0x0f) as usize] as char);
    }
    s
}

/// Minimal in-tree SHA-256 (no extra crate dep).
/// Adapted from the FIPS 180-4 reference; we keep the implementation
/// local to keep `wlwl-toml` dependency-light.
struct Sha256 {
    state: [u32; 8],
    buffer: Vec<u8>,
    bits_len: u64,
}

impl Sha256 {
    fn new() -> Self {
        Self {
            state: [
                0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
                0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
            ],
            buffer: Vec::with_capacity(64),
            bits_len: 0,
        }
    }

    fn update(&mut self, data: &[u8]) {
        self.bits_len = self.bits_len.wrapping_add((data.len() as u64) * 8);
        for chunk in data.chunks(64 - self.buffer.len()) {
            self.buffer.extend_from_slice(chunk);
            if self.buffer.len() == 64 {
                let block: [u8; 64] = self.buffer[..].try_into().unwrap();
                self.compress(&block);
                self.buffer.clear();
            }
        }
    }

    fn finalize(mut self) -> Vec<u8> {
        self.buffer.push(0x80);
        while self.buffer.len() < 56 {
            self.buffer.push(0);
        }
        self.buffer.extend_from_slice(&self.bits_len.to_be_bytes());
        let buf = std::mem::take(&mut self.buffer);
        for block in buf.chunks_exact(64) {
            let arr: [u8; 64] = block.try_into().unwrap();
            self.compress(&arr);
        }
        let mut out = Vec::with_capacity(32);
        for word in &self.state {
            out.extend_from_slice(&word.to_be_bytes());
        }
        out
    }

    fn compress(&mut self, block: &[u8; 64]) {
        const K: [u32; 64] = [
            0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
            0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
            0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
            0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
            0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
            0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
            0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
            0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
        ];
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes(block[4 * i..4 * i + 4].try_into().unwrap());
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let mut a = self.state[0];
        let mut b = self.state[1];
        let mut c = self.state[2];
        let mut d = self.state[3];
        let mut e = self.state[4];
        let mut f = self.state[5];
        let mut g = self.state[6];
        let mut h = self.state[7];
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ (!e & g);
            let temp1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }
        self.state[0] = self.state[0].wrapping_add(a);
        self.state[1] = self.state[1].wrapping_add(b);
        self.state[2] = self.state[2].wrapping_add(c);
        self.state[3] = self.state[3].wrapping_add(d);
        self.state[4] = self.state[4].wrapping_add(e);
        self.state[5] = self.state[5].wrapping_add(f);
        self.state[6] = self.state[6].wrapping_add(g);
        self.state[7] = self.state[7].wrapping_add(h);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn tempdir(suffix: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!(
            "wlwl_lock_test_{}_{}{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            suffix
        ));
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn write_file(dir: &Path, name: &str, body: &str) {
        let mut f = fs::File::create(dir.join(name)).unwrap();
        f.write_all(body.as_bytes()).unwrap();
    }

    #[test]
    fn sha256_of_known_input() {
        // SHA-256("") = e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
        let h = Sha256::new();
        let out = h.finalize();
        assert_eq!(
            hex_lower(&out),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn sha256_of_abc() {
        // SHA-256("abc") = ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad
        let mut h = Sha256::new();
        h.update(b"abc");
        let out = h.finalize();
        assert_eq!(
            hex_lower(&out),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn roundtrip_lockfile_json() {
        let lf = Lockfile {
            schema_version: CURRENT_SCHEMA_VERSION.into(),
            entries: vec![LockEntry {
                name: "myteam:utils".into(),
                path: Some("../utils".into()),
                version: None,
                hash: Some("deadbeef".into()),
            }],
        };
        let s = to_string_pretty(&lf).unwrap();
        let lf2: Lockfile = serde_json::from_str(&s).unwrap();
        assert_eq!(lf, lf2);
    }

    #[test]
    fn write_then_read_lock() {
        let dir = tempdir(".lock_rt");
        let path = dir.join("wlwl.lock");
        let lf = Lockfile {
            schema_version: CURRENT_SCHEMA_VERSION.into(),
            entries: vec![],
        };
        write(&path, &lf).unwrap();
        let lf2 = read(&path).unwrap().unwrap();
        assert_eq!(lf, lf2);
        // File should not still be the .tmp file.
        assert!(!dir.join("wlwl.lock.tmp").exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn read_missing_returns_none() {
        let dir = tempdir(".lock_missing");
        let lf = read(&dir.join("nope.lock")).unwrap();
        assert!(lf.is_none());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn rejects_wrong_schema_version() {
        let dir = tempdir(".lock_bad");
        let path = dir.join("wlwl.lock");
        fs::write(&path, r#"{"schema_version":"99.0.0","entries":[]}"#).unwrap();
        let err = read(&path).unwrap_err();
        assert!(matches!(err, LockError::UnsupportedSchemaVersion(_)));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn hash_dependency_dir_is_deterministic() {
        let dir = tempdir(".lock_hash");
        write_file(&dir, "a.wl", "LET(x, 1);");
        write_file(&dir, "b.wl", "LET(y, 2);");
        let h1 = hash_dependency_dir(&dir).unwrap().unwrap();
        let h2 = hash_dependency_dir(&dir).unwrap().unwrap();
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 64); // 32 bytes hex
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn hash_changes_when_content_changes() {
        let dir = tempdir(".lock_hash_chg");
        write_file(&dir, "a.wl", "LET(x, 1);");
        let h1 = hash_dependency_dir(&dir).unwrap().unwrap();
        write_file(&dir, "a.wl", "LET(x, 2);");
        let h2 = hash_dependency_dir(&dir).unwrap().unwrap();
        assert_ne!(h1, h2);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn hash_is_none_for_empty_dir() {
        let dir = tempdir(".lock_empty");
        assert!(hash_dependency_dir(&dir).unwrap().is_none());
        let _ = fs::remove_dir_all(&dir);
    }
}
