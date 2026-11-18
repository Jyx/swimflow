//! The state of an interactive view that an export should reproduce.
//!
//! The live preview lets the user change what a diagram shows without
//! touching its source: which diagram of a file is open, which branches of a
//! tree are folded, which rows of a manifest are hidden. When the page downloads a file it sends that state along,
//! so the file looks like the screen. A diagram type reads the parts it
//! understands and ignores the rest.

use std::collections::{BTreeMap, BTreeSet};

/// What the user did to the view of a rendered file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ViewState {
    /// The key of the diagram (section) that is open.
    pub active: Option<String>,
    /// The nodes whose children are folded away, by diagram key. A diagram
    /// that has an entry is described completely: every node that is not in
    /// the set is unfolded, whatever the source says. A diagram without an
    /// entry keeps the state the source asks for.
    pub collapsed: BTreeMap<String, BTreeSet<usize>>,
    /// What the user hid, by diagram key, as tokens the diagram type defines
    /// (the manifest uses `r3`, `c1`, `m:removed`, `only:c2`). A diagram that
    /// has an entry is described completely; one without keeps the source.
    pub hidden: BTreeMap<String, BTreeSet<String>>,
}

impl ViewState {
    /// Whether the view is untouched.
    pub fn is_empty(&self) -> bool {
        self.active.is_none() && self.collapsed.is_empty() && self.hidden.is_empty()
    }
}
