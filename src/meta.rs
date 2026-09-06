//! The Rust data model for a CPAN distribution meta file.
//!
//! Every parsed document is normalised to the **version 2** structure of the
//! [CPAN Meta Spec](https://metacpan.org/pod/CPAN::Meta::Spec), regardless of
//! the spec version of the input. The original spec version is available via
//! [`Meta::spec_version`].

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::spec::SpecVersion;

/// An arbitrary JSON/YAML value, used for custom (`x_*`) and unrecognised keys.
pub type Value = serde_json::Value;

/// A parsed CPAN distribution meta file, normalised to spec version 2.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Meta {
    /// Distribution name, e.g. `Module-Build`. Required by every spec version.
    pub name: String,

    /// Distribution version. Kept verbatim as a string; `"0"` if the input had
    /// no version (only valid in spec 1.0).
    pub version: String,

    /// Short description of the distribution's purpose.
    #[serde(rename = "abstract", default, skip_serializing_if = "Option::is_none")]
    pub abstract_: Option<String>,

    /// Contact author(s), each conventionally `"Name <email>"`. May be empty for
    /// old or partial metadata.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub author: Vec<String>,

    /// License identifier(s), normalised to spec version 2 tokens (e.g.
    /// `perl_5`, `apache_2_0`, `mit`). Unrecognised licenses become `unknown`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub license: Vec<String>,

    /// The tool that generated the metadata.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generated_by: Option<String>,

    /// Whether `Build.PL` / `Makefile.PL` must be executed to determine
    /// prerequisites. Defaults to `true` when absent.
    #[serde(default = "default_true")]
    pub dynamic_config: bool,

    /// Release maturity. Defaults to [`ReleaseStatus::Stable`].
    #[serde(default)]
    pub release_status: ReleaseStatus,

    /// The `meta-spec` field: the spec version the *normalised* structure
    /// conforms to (always `2`) plus an optional documentation URL.
    #[serde(rename = "meta-spec", default)]
    pub meta_spec: MetaSpec,

    /// Extended description of the distribution.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Free-form keywords describing the distribution.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keywords: Vec<String>,

    /// Prerequisites, organised by phase and relationship.
    #[serde(default, skip_serializing_if = "Prereqs::is_empty")]
    pub prereqs: Prereqs,

    /// Optional features the distribution can provide, each with its own
    /// prerequisites.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub optional_features: BTreeMap<String, OptionalFeature>,

    /// Packages provided by this distribution, keyed by package name.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub provides: BTreeMap<String, Provides>,

    /// Files, directories, packages and namespaces that should not be indexed.
    #[serde(default, skip_serializing_if = "NoIndex::is_empty")]
    pub no_index: NoIndex,

    /// Related resources (homepage, repository, bug tracker, ...).
    #[serde(default, skip_serializing_if = "Resources::is_empty")]
    pub resources: Resources,

    /// Custom `x_*` keys and any other keys not defined by the spec, preserved
    /// verbatim.
    #[serde(flatten)]
    pub custom: BTreeMap<String, Value>,

    /// The spec version of the *input* document (before normalisation).
    /// Not part of the serialised form.
    #[serde(skip)]
    pub spec_version: SpecVersion,
}

impl Meta {
    /// The spec version the input document declared (or was inferred to use).
    pub fn spec_version(&self) -> &SpecVersion {
        &self.spec_version
    }
}

/// The `meta-spec` field.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MetaSpec {
    /// Spec version. Always `"2"` for the normalised structure.
    #[serde(default = "default_spec_version")]
    pub version: String,
    /// URL of the spec document, if the input provided one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

impl Default for MetaSpec {
    fn default() -> Self {
        MetaSpec {
            version: "2".to_string(),
            url: None,
        }
    }
}

/// Release maturity of the distribution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReleaseStatus {
    /// A normal public release.
    #[default]
    Stable,
    /// A release for testing only (e.g. a developer / `-TRIAL` release).
    Testing,
    /// An unstable release not intended for general testing.
    Unstable,
}

/// Prerequisites organised by phase.
///
/// Each phase (`configure`, `build`, `test`, `runtime`, `develop`) holds a set
/// of relationships. Legacy `requires` / `build_requires` /
/// `configure_requires` / `recommends` / `conflicts` fields from spec 1.x are
/// mapped into this structure.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Prereqs {
    /// Requirements for configuring the distribution (running `Build.PL`).
    #[serde(default, skip_serializing_if = "Phase::is_empty")]
    pub configure: Phase,
    /// Requirements for building the distribution.
    #[serde(default, skip_serializing_if = "Phase::is_empty")]
    pub build: Phase,
    /// Requirements for running the test suite.
    #[serde(default, skip_serializing_if = "Phase::is_empty")]
    pub test: Phase,
    /// Requirements for normal runtime use.
    #[serde(default, skip_serializing_if = "Phase::is_empty")]
    pub runtime: Phase,
    /// Requirements for authoring / developing the distribution.
    #[serde(default, skip_serializing_if = "Phase::is_empty")]
    pub develop: Phase,
    /// Any other (non-standard) phases.
    #[serde(flatten)]
    pub other: BTreeMap<String, Phase>,
}

impl Prereqs {
    /// Whether there are no prerequisites at all.
    pub fn is_empty(&self) -> bool {
        self.configure.is_empty()
            && self.build.is_empty()
            && self.test.is_empty()
            && self.runtime.is_empty()
            && self.develop.is_empty()
            && self.other.is_empty()
    }

    /// Look up a phase by name (`"configure"`, `"build"`, `"test"`, `"runtime"`,
    /// `"develop"`, or a custom phase).
    pub fn phase(&self, name: &str) -> Option<&Phase> {
        match name {
            "configure" => Some(&self.configure),
            "build" => Some(&self.build),
            "test" => Some(&self.test),
            "runtime" => Some(&self.runtime),
            "develop" => Some(&self.develop),
            other => self.other.get(other),
        }
    }
}

/// The relationships within a single prerequisite phase. Values are version
/// range strings (see [`crate::version_range`]).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Phase {
    /// Modules that must be installed.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub requires: BTreeMap<String, String>,
    /// Modules that are strongly encouraged.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub recommends: BTreeMap<String, String>,
    /// Modules that are optional enhancements.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub suggests: BTreeMap<String, String>,
    /// Modules that must *not* be installed.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub conflicts: BTreeMap<String, String>,
    /// Any other (non-standard) relationships.
    #[serde(flatten)]
    pub other: BTreeMap<String, BTreeMap<String, String>>,
}

impl Phase {
    /// Whether this phase has no relationships.
    pub fn is_empty(&self) -> bool {
        self.requires.is_empty()
            && self.recommends.is_empty()
            && self.suggests.is_empty()
            && self.conflicts.is_empty()
            && self.other.is_empty()
    }

    /// Look up a relationship by name (`"requires"`, `"recommends"`,
    /// `"suggests"`, `"conflicts"`, or a custom relationship).
    pub fn relation(&self, name: &str) -> Option<&BTreeMap<String, String>> {
        match name {
            "requires" => Some(&self.requires),
            "recommends" => Some(&self.recommends),
            "suggests" => Some(&self.suggests),
            "conflicts" => Some(&self.conflicts),
            other => self.other.get(other),
        }
    }
}

/// An entry in [`Meta::optional_features`].
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OptionalFeature {
    /// Human-readable description of the feature.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Prerequisites this feature adds. The `configure` phase is not meaningful
    /// here but is preserved if present.
    #[serde(default, skip_serializing_if = "Prereqs::is_empty")]
    pub prereqs: Prereqs,
}

/// An entry in [`Meta::provides`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Provides {
    /// Unix-style relative path to the file that contains the package.
    #[serde(default)]
    pub file: String,
    /// Version of the package, if known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

/// The `no_index` field: things a CPAN indexer should ignore.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct NoIndex {
    /// Individual files (relative, Unix-style paths).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub file: Vec<String>,
    /// Directory trees (relative, Unix-style paths). The spec 1.2 `dir` key is
    /// accepted as an alias.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub directory: Vec<String>,
    /// Exact package names.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub package: Vec<String>,
    /// Namespaces (descendants excluded, the namespace itself is not).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub namespace: Vec<String>,
}

impl NoIndex {
    /// Whether nothing is excluded from indexing.
    pub fn is_empty(&self) -> bool {
        self.file.is_empty()
            && self.directory.is_empty()
            && self.package.is_empty()
            && self.namespace.is_empty()
    }
}

/// The `resources` field.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Resources {
    /// Project homepage.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub homepage: Option<String>,
    /// URLs of license documents.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub license: Vec<String>,
    /// Bug tracker information.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bugtracker: Option<Bugtracker>,
    /// Source repository information.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repository: Option<Repository>,
    /// Custom (`x_*` / capitalised) resource keys, preserved verbatim.
    #[serde(flatten)]
    pub custom: BTreeMap<String, Value>,
}

impl Resources {
    /// Whether no resources are recorded.
    pub fn is_empty(&self) -> bool {
        self.homepage.is_none()
            && self.license.is_empty()
            && self.bugtracker.is_none()
            && self.repository.is_none()
            && self.custom.is_empty()
    }
}

/// The `resources.bugtracker` field.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Bugtracker {
    /// Web interface URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub web: Option<String>,
    /// Email address for reporting bugs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mailto: Option<String>,
}

/// The `resources.repository` field.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Repository {
    /// URL for anonymous checkout / clone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Web interface URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub web: Option<String>,
    /// Version control system type (`git`, `svn`, `hg`, ...).
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub type_: Option<String>,
}

fn default_true() -> bool {
    true
}

fn default_spec_version() -> String {
    "2".to_string()
}
