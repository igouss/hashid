//! The entity: a parsed, always-well-formed identifier.
//!
//! There is exactly one ID type in this crate, and it cannot hold a malformed
//! value. Grammar lives in [`crate::parse`] (which constructs `Id`s); this
//! module owns the entity and its hierarchy behaviour.

use core::fmt;

use crate::domain::hash::Base36Hash;
use crate::domain::prefix::Prefix;

/// A validated identifier: `<prefix>-<hash>`, optionally with a child path
/// (`<prefix>-<hash>.1.2`).
///
/// `Id` is `Eq + Hash + Ord`, so it drops straight into a `HashSet`, a
/// `BTreeMap`, or a sorted list without a wrapper.
///
/// ```
/// # use hashid::Id;
/// let id: Id = "br-abc123.1".parse()?;
/// assert_eq!(id.prefix().as_str(), "br");
/// assert_eq!(id.hash().as_str(), "abc123");
/// assert_eq!(id.depth(), 1);
/// assert_eq!(id.parent().map(|p| p.to_string()), Some("br-abc123".to_string()));
/// # Ok::<(), hashid::IdError>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Id {
    prefix: Prefix,
    hash: Base36Hash,
    child_path: Vec<u32>,
}

impl Id {
    /// Assemble an ID from parts that are already validated.
    ///
    /// Crate-internal: public construction goes through [`crate::parse`] or the
    /// generator, both of which enforce the grammar.
    pub(crate) fn from_parts(prefix: Prefix, hash: Base36Hash, child_path: Vec<u32>) -> Self {
        Self {
            prefix,
            hash,
            child_path,
        }
    }

    /// Assemble a root ID (no child path) from validated parts.
    pub(crate) fn root(prefix: Prefix, hash: Base36Hash) -> Self {
        Self::from_parts(prefix, hash, Vec::new())
    }

    /// The namespace half.
    #[must_use]
    pub fn prefix(&self) -> &Prefix {
        &self.prefix
    }

    /// The uniquifying half.
    #[must_use]
    pub fn hash(&self) -> &Base36Hash {
        &self.hash
    }

    /// The child path, empty for a root ID.
    #[must_use]
    pub fn child_path(&self) -> &[u32] {
        &self.child_path
    }

    /// Is this a top-level ID?
    #[must_use]
    pub fn is_root(&self) -> bool {
        self.child_path.is_empty()
    }

    /// How deep in the hierarchy: `0` for a root, `1` for its child, and so on.
    #[must_use]
    pub fn depth(&self) -> usize {
        self.child_path.len()
    }

    /// This ID's parent, or `None` if it is a root.
    #[must_use]
    pub fn parent(&self) -> Option<Self> {
        let (_, ancestors) = self.child_path.split_last()?;
        Some(Self::from_parts(
            self.prefix.clone(),
            self.hash.clone(),
            ancestors.to_vec(),
        ))
    }

    /// The `n`-th child of this ID.
    #[must_use]
    pub fn child(&self, n: u32) -> Self {
        let mut path: Vec<u32> = self.child_path.clone();
        path.push(n);
        Self::from_parts(self.prefix.clone(), self.hash.clone(), path)
    }

    /// Is this ID a descendant — child, grandchild, further — of `ancestor`?
    ///
    /// A root is never a descendant of itself.
    #[must_use]
    pub fn is_descendant_of(&self, ancestor: &Self) -> bool {
        self.prefix == ancestor.prefix
            && self.hash == ancestor.hash
            && self.child_path.len() > ancestor.child_path.len()
            && self.child_path.starts_with(&ancestor.child_path)
    }
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}-{}", self.prefix, self.hash)?;
        for segment in &self.child_path {
            write!(f, ".{segment}")?;
        }
        Ok(())
    }
}
