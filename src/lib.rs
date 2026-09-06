//! Parse CPAN distribution meta files (`META.json` / `META.yml`) into a typed
//! Rust structure.
//!
//! The [CPAN Meta Spec](https://metacpan.org/pod/CPAN::Meta::Spec) has gone
//! through several revisions. `META.yml` files use the `1.x` series; `META.json`
//! files use version `2`. This crate accepts **any** of those versions, in
//! **either** JSON or YAML serialisation, and always returns a [`Meta`] value
//! normalised to the version 2 structure — so consumers only deal with one
//! shape. The spec version of the original document is preserved on
//! [`Meta::spec_version`].
//!
//! Older spec versions are upgraded following the same rules as Perl's
//! [`CPAN::Meta::Converter`](https://metacpan.org/pod/CPAN::Meta::Converter):
//! the flat `requires` / `build_requires` / `configure_requires` / `recommends`
//! / `conflicts` fields become the phase/relationship [`Prereqs`] map, a single
//! `license` string becomes a list of normalised license tokens, string
//! `resources` become structured [`Repository`] / [`Bugtracker`] values, the
//! `private` field becomes [`NoIndex`], and so on.
//!
//! # Examples
//!
//! ```
//! let json = r#"{
//!   "name": "Foo-Bar",
//!   "version": "1.23",
//!   "abstract": "a foo for bars",
//!   "author": ["Jane Doe <jane@example.com>"],
//!   "license": ["perl_5"],
//!   "dynamic_config": 0,
//!   "release_status": "stable",
//!   "meta-spec": { "version": 2 },
//!   "prereqs": {
//!     "runtime": { "requires": { "perl": "5.010", "Carp": "0" } }
//!   }
//! }"#;
//!
//! let meta = cpan_distribution_meta::parse(json).unwrap();
//! assert_eq!(meta.name, "Foo-Bar");
//! assert_eq!(meta.prereqs.runtime.requires["perl"], "5.010");
//! ```
//!
//! ```
//! // An old META.yml (spec 1.4) is upgraded to the version 2 structure.
//! let yaml = r#"
//! ---
//! name: Foo-Bar
//! version: 1.23
//! abstract: a foo for bars
//! author:
//!   - Jane Doe <jane@example.com>
//! license: perl
//! requires:
//!   perl: 5.006
//!   Carp: 0
//! build_requires:
//!   Test::More: 0
//! resources:
//!   repository: http://github.com/jane/foo-bar
//! meta-spec:
//!   version: 1.4
//! "#;
//!
//! let meta = cpan_distribution_meta::parse(yaml).unwrap();
//! assert_eq!(meta.license, ["perl_5"]);
//! assert_eq!(meta.prereqs.runtime.requires["Carp"], "0");
//! assert_eq!(meta.prereqs.build.requires["Test::More"], "0");
//! assert_eq!(
//!     meta.resources.repository.unwrap().web.unwrap(),
//!     "http://github.com/jane/foo-bar"
//! );
//! ```
//!
//! # A note on YAML numbers
//!
//! `META.yml` files conventionally quote version numbers. An *unquoted*
//! `version: 1.10` is parsed by the YAML layer as the float `1.1`, losing the
//! trailing zero — this matches the behaviour of most YAML 1.1/1.2 parsers.
//! Quote version-like scalars to be safe.

#![warn(missing_docs)]

mod convert;
mod error;
mod meta;
mod spec;
pub mod version_range;

pub use error::{Error, Result};
pub use meta::{
    Bugtracker, Meta, MetaSpec, NoIndex, OptionalFeature, Phase, Prereqs, Provides, ReleaseStatus,
    Repository, Resources, Value,
};
pub use spec::SpecVersion;

use std::io::Read;
use std::path::Path;

use error::Format;
use serde_json::Value as JsonValue;

/// Parse metadata from a string, auto-detecting JSON vs YAML.
///
/// Detection is by content: a document whose first non-whitespace character is
/// `{` or `[` is treated as JSON, otherwise as YAML. If the guessed format
/// fails to parse, the other is tried before giving up.
pub fn parse(input: &str) -> Result<Meta> {
    let (value, _) = parse_document(input)?;
    build(value)
}

/// Parse metadata that is known to be JSON.
pub fn parse_json(input: &str) -> Result<Meta> {
    let value: JsonValue = serde_json::from_str(input).map_err(Error::Json)?;
    build(value)
}

/// Parse metadata that is known to be YAML.
pub fn parse_yaml(input: &str) -> Result<Meta> {
    let value: JsonValue = serde_norway::from_str(input).map_err(Error::Yaml)?;
    build(value)
}

/// Parse metadata from raw bytes, auto-detecting JSON vs YAML.
///
/// The bytes must be UTF-8. A leading UTF-8 BOM, if present, is ignored.
pub fn parse_slice(input: &[u8]) -> Result<Meta> {
    let text = std::str::from_utf8(input)
        .map_err(|e| Error::Invalid(format!("input is not valid UTF-8: {e}")))?;
    parse(text.strip_prefix('\u{feff}').unwrap_or(text))
}

/// Read metadata from any [`Read`] source, auto-detecting JSON vs YAML.
pub fn from_reader<R: Read>(mut reader: R) -> Result<Meta> {
    let mut buf = Vec::new();
    reader.read_to_end(&mut buf)?;
    parse_slice(&buf)
}

/// Read and parse a `META.json` / `META.yml` file from disk.
///
/// The format is detected from the file contents, not the extension, so a
/// misnamed file still parses.
pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Meta> {
    let bytes = std::fs::read(path)?;
    parse_slice(&bytes)
}

fn looks_like_json(input: &str) -> bool {
    matches!(input.trim_start().as_bytes().first(), Some(b'{' | b'['))
}

fn parse_document(input: &str) -> Result<(JsonValue, Format)> {
    if input.trim().is_empty() {
        return Err(Error::UnknownFormat);
    }

    let (first, second) = if looks_like_json(input) {
        (Format::Json, Format::Yaml)
    } else {
        (Format::Yaml, Format::Json)
    };

    let try_one = |fmt: Format| -> std::result::Result<JsonValue, Error> {
        match fmt {
            Format::Json => serde_json::from_str(input).map_err(Error::Json),
            Format::Yaml => serde_norway::from_str(input).map_err(Error::Yaml),
        }
    };

    match try_one(first) {
        Ok(v) => Ok((v, first)),
        Err(first_err) => match try_one(second) {
            Ok(v) => Ok((v, second)),
            Err(_) => Err(first_err),
        },
    }
}

fn build(value: JsonValue) -> Result<Meta> {
    let mut obj = match value {
        JsonValue::Object(map) => map,
        _ => return Err(Error::NotAMapping),
    };

    let spec_version = spec::detect(&obj);

    if !obj.contains_key("name") {
        return Err(Error::MissingField("name"));
    }

    convert::normalize(&mut obj, &spec_version);

    let mut meta: Meta = serde_json::from_value(JsonValue::Object(obj)).map_err(Error::Json)?;
    meta.spec_version = spec_version;
    Ok(meta)
}
