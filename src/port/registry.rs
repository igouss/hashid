//! The driven ports: what the crate needs *from you*.
//!
//! The crate never opens a database. It asks two questions, and you answer
//! them however your store answers them.

use crate::domain::id::Id;
use crate::port::error::RegistryError;

/// Answers "does this ID already exist?".
///
/// The generator consults this port on every candidate, so an implementation
/// should make the lookup cheap — an index hit, or a pre-loaded set.
///
/// # Why this is fallible
///
/// Returning `Result` rather than `bool` is deliberate. A store-backed adapter
/// *can* fail, and if the port cannot say so, the adapter has to pick a lie:
/// return `false` (inventing a free ID that may already be taken) or `true`
/// (burning a good candidate). Callers of infallible ports end up smuggling the
/// real error out through a `RefCell` on the side. Making the port honest
/// deletes that whole category of workaround.
pub trait IdRegistry {
    /// Is `id` already taken?
    ///
    /// # Errors
    ///
    /// [`RegistryError`] when the store cannot answer. The generator aborts and
    /// propagates rather than guessing.
    fn contains(&self, id: &Id) -> Result<bool, RegistryError>;
}

/// Adds the lookup the *resolver* needs: find IDs by a fragment of their hash.
///
/// Separate from [`IdRegistry`] because minting needs only an existence check;
/// a caller who mints but never resolves should not have to implement a search.
pub trait IdIndex: IdRegistry {
    /// Every known ID whose hash contains `fragment`.
    ///
    /// `fragment` may carry a child path (`64up6.4`), in which case the child
    /// path must match exactly while the hash matches as a substring.
    ///
    /// # Errors
    ///
    /// [`RegistryError`] when the store cannot answer.
    fn find_by_hash_fragment(&self, fragment: &str) -> Result<Vec<Id>, RegistryError>;
}

impl<T: IdRegistry + ?Sized> IdRegistry for &T {
    fn contains(&self, id: &Id) -> Result<bool, RegistryError> {
        (**self).contains(id)
    }
}

impl<T: IdIndex + ?Sized> IdIndex for &T {
    fn find_by_hash_fragment(&self, fragment: &str) -> Result<Vec<Id>, RegistryError> {
        (**self).find_by_hash_fragment(fragment)
    }
}
