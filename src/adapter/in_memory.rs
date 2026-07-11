//! A registry backed by a `HashSet`.

use std::collections::HashSet;

use crate::domain::id::Id;
use crate::port::{IdIndex, IdRegistry, RegistryError};
use crate::resolve::matcher::matches_fragment;

/// Every ID you know about, held in memory.
///
/// This is not only a test double. Loading the full ID set up front and minting
/// against it is the right shape whenever the set is small and you are about to
/// mint in bulk: it turns a per-candidate round trip into one query.
///
/// Lookups are infallible here, so [`IdRegistry::contains`] never returns
/// `Err` — the fallible signature exists for adapters that talk to something
/// that can actually break.
///
/// ```
/// # use hashid::{Id, InMemoryIds, IdRegistry};
/// let mut known: InMemoryIds = InMemoryIds::new();
/// known.insert(Id::parse("br-abc123")?);
///
/// assert!(known.contains(&Id::parse("br-abc123")?)?);
/// assert!(!known.contains(&Id::parse("br-xyz789")?)?);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InMemoryIds {
    ids: HashSet<Id>,
}

impl InMemoryIds {
    /// An empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Record `id` as taken. Returns `false` if it already was.
    pub fn insert(&mut self, id: Id) -> bool {
        self.ids.insert(id)
    }

    /// Forget `id`. Returns `true` if it was there.
    pub fn remove(&mut self, id: &Id) -> bool {
        self.ids.remove(id)
    }

    /// How many IDs are held.
    #[must_use]
    pub fn len(&self) -> usize {
        self.ids.len()
    }

    /// Are there no IDs at all?
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    /// Iterate the held IDs, in no particular order.
    pub fn iter(&self) -> impl Iterator<Item = &Id> {
        self.ids.iter()
    }
}

impl IdRegistry for InMemoryIds {
    fn contains(&self, id: &Id) -> Result<bool, RegistryError> {
        Ok(self.ids.contains(id))
    }
}

impl IdIndex for InMemoryIds {
    fn find_by_hash_fragment(&self, fragment: &str) -> Result<Vec<Id>, RegistryError> {
        Ok(self
            .ids
            .iter()
            .filter(|id: &&Id| matches_fragment(id, fragment))
            .cloned()
            .collect())
    }
}

impl FromIterator<Id> for InMemoryIds {
    fn from_iter<T: IntoIterator<Item = Id>>(iter: T) -> Self {
        Self {
            ids: iter.into_iter().collect(),
        }
    }
}

impl Extend<Id> for InMemoryIds {
    fn extend<T: IntoIterator<Item = Id>>(&mut self, iter: T) {
        self.ids.extend(iter);
    }
}

impl<'a> IntoIterator for &'a InMemoryIds {
    type Item = &'a Id;
    type IntoIter = std::collections::hash_set::Iter<'a, Id>;

    fn into_iter(self) -> Self::IntoIter {
        self.ids.iter()
    }
}
