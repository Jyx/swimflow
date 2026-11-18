//! Link targets: external URLs (`href`) and internal diagram drill-downs
//! (`ref`).
//!
//! `ref` value forms:
//!
//! | value                 | meaning                                   |
//! |-----------------------|-------------------------------------------|
//! | `detail`              | diagram `detail` in the same file         |
//! | `#detail`             | same                                      |
//! | `other.diag`          | the resolved/default diagram in `other.diag` |
//! | `other.diag#detail`   | diagram `detail` in `other.diag`          |

use std::path::PathBuf;

use crate::build::model::{AttrMap, DiagramId};

/// A link attached to a node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Link {
    /// External URL from `href`.
    External(String),
    /// Internal diagram reference from `ref`.
    Diagram(LinkSpec),
}

/// An unresolved internal reference.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LinkSpec {
    /// `None` means "same file as the referring node".
    pub file: Option<PathBuf>,
    /// `None` means "the resolved/default diagram in the target file".
    pub diagram: Option<DiagramId>,
}

/// A link resolved to a concrete diagram in a loaded [`super::model::DocumentSet`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedLink {
    /// Index into `DocumentSet::documents`.
    pub document: usize,
    pub diagram: DiagramId,
}

/// Extract a link from a node's attributes.
///
/// `ref` and `href` are mutually exclusive.
pub fn link_from_attrs(attrs: &AttrMap) -> Result<Option<Link>, String> {
    let reference = attrs.get("ref").and_then(|v| v.as_deref());
    let href = attrs.get("href").and_then(|v| v.as_deref());

    match (reference, href) {
        (Some(_), Some(_)) => Err("`ref` and `href` are mutually exclusive".to_string()),
        (Some(value), None) => Ok(Some(Link::Diagram(parse_ref(value)))),
        (None, Some(value)) => Ok(Some(Link::External(value.to_string()))),
        (None, None) => Ok(None),
    }
}

/// Parse the value of a `ref` attribute into a [`LinkSpec`].
pub fn parse_ref(value: &str) -> LinkSpec {
    if let Some(diagram) = value.strip_prefix('#') {
        LinkSpec {
            file: None,
            diagram: Some(diagram.to_string()),
        }
    } else if let Some((file, diagram)) = value.split_once('#') {
        LinkSpec {
            file: Some(PathBuf::from(file)),
            diagram: Some(diagram.to_string()),
        }
    } else if looks_like_file(value) {
        LinkSpec {
            file: Some(PathBuf::from(value)),
            diagram: None,
        }
    } else {
        LinkSpec {
            file: None,
            diagram: Some(value.to_string()),
        }
    }
}

/// Heuristic used to disambiguate `detail` (diagram id) from `other.diag`
/// (file). A `.diag` extension or a path separator marks a file reference.
fn looks_like_file(value: &str) -> bool {
    value.ends_with(".diag") || value.contains('/') || value.contains('\\')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_document_forms() {
        assert_eq!(
            parse_ref("detail"),
            LinkSpec {
                file: None,
                diagram: Some("detail".into())
            }
        );
        assert_eq!(parse_ref("#detail"), parse_ref("detail"));
    }

    #[test]
    fn file_forms() {
        assert_eq!(
            parse_ref("other.diag"),
            LinkSpec {
                file: Some("other.diag".into()),
                diagram: None
            }
        );
        assert_eq!(
            parse_ref("other.diag#detail"),
            LinkSpec {
                file: Some("other.diag".into()),
                diagram: Some("detail".into())
            }
        );
        assert_eq!(
            parse_ref("sub/other.diag"),
            LinkSpec {
                file: Some("sub/other.diag".into()),
                diagram: None
            }
        );
    }
}
