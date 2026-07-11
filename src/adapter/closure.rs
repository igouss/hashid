//! Registry adapters built from plain closures.
//!
//! For when your store is already a couple of function calls away and a whole
//! `impl` block would be ceremony.

use crate::domain::id::Id;
use crate::port::{IdIndex, IdRegistry, RegistryError};

/// An [`IdRegistry`] made from a closure.
///
/// ```
/// # use hashid::{FnRegistry, Id, IdRegistry};
/// let taken = ["br-abc123"];
/// let registry = FnRegistry::new(|id: &Id| Ok(taken.contains(&id.to_string().as_str())));
///
/// assert!(registry.contains(&Id::parse("br-abc123")?)?);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, Copy)]
pub struct FnRegistry<F> {
    contains: F,
}

impl<F> FnRegistry<F>
where
    F: Fn(&Id) -> Result<bool, RegistryError>,
{
    /// Wrap `contains` as a registry.
    #[must_use]
    pub fn new(contains: F) -> Self {
        Self { contains }
    }
}

impl<F> IdRegistry for FnRegistry<F>
where
    F: Fn(&Id) -> Result<bool, RegistryError>,
{
    fn contains(&self, id: &Id) -> Result<bool, RegistryError> {
        (self.contains)(id)
    }
}

/// An [`IdIndex`] made from a pair of closures.
#[derive(Debug, Clone, Copy)]
pub struct FnIndex<F, G> {
    contains: F,
    find: G,
}

impl<F, G> FnIndex<F, G>
where
    F: Fn(&Id) -> Result<bool, RegistryError>,
    G: Fn(&str) -> Result<Vec<Id>, RegistryError>,
{
    /// Wrap an existence check and a fragment search as an index.
    #[must_use]
    pub fn new(contains: F, find: G) -> Self {
        Self { contains, find }
    }
}

impl<F, G> IdRegistry for FnIndex<F, G>
where
    F: Fn(&Id) -> Result<bool, RegistryError>,
    G: Fn(&str) -> Result<Vec<Id>, RegistryError>,
{
    fn contains(&self, id: &Id) -> Result<bool, RegistryError> {
        (self.contains)(id)
    }
}

impl<F, G> IdIndex for FnIndex<F, G>
where
    F: Fn(&Id) -> Result<bool, RegistryError>,
    G: Fn(&str) -> Result<Vec<Id>, RegistryError>,
{
    fn find_by_hash_fragment(&self, fragment: &str) -> Result<Vec<Id>, RegistryError> {
        (self.find)(fragment)
    }
}
