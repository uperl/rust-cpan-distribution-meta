use serde_json::{Map, Value};
use std::fmt;

/// A recognised version of the CPAN Meta Spec.
///
/// The spec has gone through a number of revisions. The `1.x` series describe
/// `META.yml`; version `2` describes `META.json` (and may also be serialised as
/// YAML). See [`CPAN::Meta::History`](https://metacpan.org/pod/CPAN::Meta::History).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SpecVersion {
    /// Spec version 1.0 (March 2003).
    V1_0,
    /// Spec version 1.1 (May 2003).
    V1_1,
    /// Spec version 1.2 (August 2005).
    V1_2,
    /// Spec version 1.3 (November 2006).
    V1_3,
    /// Spec version 1.4 (June 2008).
    V1_4,
    /// Spec version 2 (April 2010) — the current version.
    V2,
    /// A version string that is not one of the known revisions.
    Other(String),
}

impl SpecVersion {
    /// The major component of the spec version (`1` or `2`).
    pub fn major(&self) -> u64 {
        match self {
            SpecVersion::V1_0
            | SpecVersion::V1_1
            | SpecVersion::V1_2
            | SpecVersion::V1_3
            | SpecVersion::V1_4 => 1,
            SpecVersion::V2 => 2,
            SpecVersion::Other(s) => s
                .split('.')
                .next()
                .and_then(|m| m.trim_start_matches('v').parse().ok())
                .unwrap_or(2),
        }
    }

    /// Whether this is a `1.x` spec version, whose fields must be upgraded to
    /// the version 2 structure.
    pub fn is_legacy(&self) -> bool {
        self.major() < 2
    }

    /// Parse a version value from the `meta-spec` field. Accepts strings and
    /// numbers (`2`, `"2"`, `1.4`, `"1.4"`, `"v1.4.0"`).
    pub(crate) fn parse(value: &str) -> SpecVersion {
        let v = value.trim().trim_start_matches('v');
        // Compare on major/minor only.
        let mut parts = v.split('.');
        let major = parts.next().unwrap_or("");
        let minor = parts.next().unwrap_or("0");
        match (major, minor) {
            ("1", "0") => SpecVersion::V1_0,
            ("1", "1") => SpecVersion::V1_1,
            ("1", "2") => SpecVersion::V1_2,
            ("1", "3") => SpecVersion::V1_3,
            ("1", "4") => SpecVersion::V1_4,
            ("2", _) => SpecVersion::V2,
            _ => SpecVersion::Other(value.trim().to_string()),
        }
    }
}

impl Default for SpecVersion {
    /// The current spec version, [`SpecVersion::V2`].
    fn default() -> Self {
        SpecVersion::V2
    }
}

impl fmt::Display for SpecVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            SpecVersion::V1_0 => "1.0",
            SpecVersion::V1_1 => "1.1",
            SpecVersion::V1_2 => "1.2",
            SpecVersion::V1_3 => "1.3",
            SpecVersion::V1_4 => "1.4",
            SpecVersion::V2 => "2",
            SpecVersion::Other(s) => s,
        };
        f.write_str(s)
    }
}

/// Best-effort detection of the spec version of a raw metadata document.
///
/// Uses the `meta-spec` (or `meta_spec`) field if present. Falling back:
/// documents that use the version 2 `prereqs` / `release_status` fields are
/// treated as version 2; everything else is assumed to be the original 1.0
/// spec, which had no `meta-spec` field. This mirrors `CPAN::Meta::Converter`.
pub(crate) fn detect(doc: &Map<String, Value>) -> SpecVersion {
    if let Some(meta_spec) = doc.get("meta-spec").or_else(|| doc.get("meta_spec")) {
        let raw = match meta_spec {
            Value::Object(map) => map.get("version"),
            other => Some(other),
        };
        if let Some(v) = raw
            && let Some(s) = value_to_version_string(v)
        {
            return SpecVersion::parse(&s);
        }
    }

    if doc.get("prereqs").is_some() || doc.get("release_status").is_some() {
        return SpecVersion::V2;
    }

    SpecVersion::V1_0
}

fn value_to_version_string(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}
