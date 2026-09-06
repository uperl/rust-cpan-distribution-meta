use std::fmt;

/// Errors that can occur while reading and parsing a CPAN distribution meta file.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// The input could not be parsed as JSON.
    #[error("failed to parse metadata as JSON: {0}")]
    Json(#[source] serde_json::Error),

    /// The input could not be parsed as YAML.
    #[error("failed to parse metadata as YAML: {0}")]
    Yaml(#[source] serde_norway::Error),

    /// An I/O error occurred while reading the input.
    #[error("failed to read metadata: {0}")]
    Io(#[from] std::io::Error),

    /// The input did not look like either JSON or YAML.
    #[error("could not determine metadata format (expected JSON or YAML)")]
    UnknownFormat,

    /// The parsed document was not a mapping / object at the top level.
    #[error("metadata is not a mapping at the top level")]
    NotAMapping,

    /// A field required by every spec version was missing.
    #[error("metadata is missing required field `{0}`")]
    MissingField(&'static str),

    /// The metadata was structurally valid but semantically invalid.
    #[error("invalid metadata: {0}")]
    Invalid(String),
}

/// Convenience alias for results returned by this crate.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Which format a parse attempt was made against, used for diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Format {
    Json,
    Yaml,
}

impl fmt::Display for Format {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Format::Json => f.write_str("JSON"),
            Format::Yaml => f.write_str("YAML"),
        }
    }
}
