//! Parsing of the version-range mini-language used by prerequisite values.
//!
//! From the [spec](https://metacpan.org/pod/CPAN::Meta::Spec#Version-Ranges):
//! a bare version `V` means "at least `V`"; `0` means "any version"; the
//! operators `<`, `<=`, `>`, `>=`, `==`, `!=` may prefix a version; and
//! multiple clauses separated by commas are ANDed together.

use std::fmt;

/// A comparison operator in a version range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    /// `<`
    Less,
    /// `<=`
    LessEqual,
    /// `>`
    Greater,
    /// `>=`
    GreaterEqual,
    /// `==`
    Equal,
    /// `!=`
    NotEqual,
}

impl fmt::Display for Op {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Op::Less => "<",
            Op::LessEqual => "<=",
            Op::Greater => ">",
            Op::GreaterEqual => ">=",
            Op::Equal => "==",
            Op::NotEqual => "!=",
        })
    }
}

/// A single clause of a version range, such as `>= 1.2`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Constraint {
    /// The comparison operator.
    pub op: Op,
    /// The version literal, kept verbatim.
    pub version: String,
}

impl fmt::Display for Constraint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.op, self.version)
    }
}

/// Parse a version-range string into its constraints.
///
/// A bare version becomes a single [`Op::GreaterEqual`] constraint. The special
/// value `0` (meaning "any version") yields an empty list. Unparseable clauses
/// are skipped.
///
/// ```
/// use cpan_distribution_meta::version_range::{parse, Op};
///
/// let c = parse(">= 1.2, != 1.5, < 2.0");
/// assert_eq!(c.len(), 3);
/// assert_eq!(c[0].op, Op::GreaterEqual);
/// assert_eq!(c[0].version, "1.2");
///
/// assert!(parse("0").is_empty());
/// assert_eq!(parse("1.10")[0].op, Op::GreaterEqual);
/// ```
pub fn parse(range: &str) -> Vec<Constraint> {
    let mut out = Vec::new();
    for clause in range.split(',') {
        let clause = clause.trim();
        if clause.is_empty() {
            continue;
        }

        let (op, rest) = if let Some(r) = clause.strip_prefix("<=") {
            (Op::LessEqual, r)
        } else if let Some(r) = clause.strip_prefix(">=") {
            (Op::GreaterEqual, r)
        } else if let Some(r) = clause.strip_prefix("==") {
            (Op::Equal, r)
        } else if let Some(r) = clause.strip_prefix("!=") {
            (Op::NotEqual, r)
        } else if let Some(r) = clause.strip_prefix('<') {
            (Op::Less, r)
        } else if let Some(r) = clause.strip_prefix('>') {
            (Op::Greater, r)
        } else {
            // Bare version: "at least", unless it is the "any version" marker.
            if clause == "0" {
                continue;
            }
            out.push(Constraint {
                op: Op::GreaterEqual,
                version: clause.to_string(),
            });
            continue;
        };

        let version = rest.trim();
        if !version.is_empty() {
            out.push(Constraint {
                op,
                version: version.to_string(),
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bare_version_is_at_least() {
        assert_eq!(
            parse("1.5"),
            [Constraint {
                op: Op::GreaterEqual,
                version: "1.5".into()
            }]
        );
    }

    #[test]
    fn any_version_marker_is_empty() {
        assert!(parse("0").is_empty());
        assert!(parse("").is_empty());
        assert!(parse("  ,  ").is_empty());
    }

    #[test]
    fn all_operators() {
        let c = parse("< 1.0, <= 2.0, > 3.0, >= 4.0, == 5.0, != 6.0");
        assert_eq!(
            c,
            [
                Constraint {
                    op: Op::Less,
                    version: "1.0".into()
                },
                Constraint {
                    op: Op::LessEqual,
                    version: "2.0".into()
                },
                Constraint {
                    op: Op::Greater,
                    version: "3.0".into()
                },
                Constraint {
                    op: Op::GreaterEqual,
                    version: "4.0".into()
                },
                Constraint {
                    op: Op::Equal,
                    version: "5.0".into()
                },
                Constraint {
                    op: Op::NotEqual,
                    version: "6.0".into()
                },
            ]
        );
    }

    #[test]
    fn tight_spacing_and_display() {
        let c = parse(">=1.2,!=1.5");
        assert_eq!(c.len(), 2);
        assert_eq!(c[0].to_string(), ">= 1.2");
        assert_eq!(c[1].to_string(), "!= 1.5");
    }
}
