//! The one thing this binding needs from you: a lookup.
//!
//! Minting asks a single question — *is this ID already taken?* — and
//! [`IdStore`] is that question. Implement it over whatever actually holds your
//! IDs: a table, a key-value store, a set you loaded at startup.
//!
//! There is no store implementation here to reach for instead. That is the
//! point: a store shipped with a binding is a hash set that ends up in
//! production holding a set which stopped fitting in memory two releases ago,
//! and every language `UniFFI` reaches already has a better one built in.
//!
//! The question is **fallible**, and that is load-bearing. If the interface
//! could not say "I do not know", an implementation talking to a real database
//! would have to pick a lie when the connection drops: answer `false` and
//! invent a free ID that may already be taken, or answer `true` and burn a good
//! candidate. Both are silent, and the first corrupts data. Raising
//! `StoreError` is the third option, and the minter propagates it rather than
//! guessing.
//!
//! In Rust this is a closure — `hashid` has no trait for it, because one
//! question does not need an interface. It is an interface here only because
//! no foreign ABI can carry a Rust closure.

use hashid::{Id as CoreId, StoreError as CoreStoreError};

use crate::error::StoreError;

/// Your ID store, as the one question this library asks of it.
///
/// # What a mint costs
///
/// One `contains` call in the ordinary case. The minter only asks again when a
/// candidate is taken, and the escalation ladder is bounded, so a store that
/// answers from an index is answering about once per ID minted.
#[uniffi::export(with_foreign)]
pub trait IdStore: Send + Sync {
    /// Is `id` already taken?
    ///
    /// # Errors
    ///
    /// `StoreError` when the store cannot answer. Do not guess: the minter
    /// handles "I do not know" correctly and cannot handle a wrong "no".
    fn contains(&self, id: String) -> Result<bool, StoreError>;
}

/// Ask an [`IdStore`] about one candidate, in the shape the core wants.
///
/// The core asks in [`CoreId`]s and this interface answers in strings, so
/// something has to render on the way out. Doing it here, in one place, is what
/// keeps `Minter` free of the conversion.
pub(crate) fn taken(store: &dyn IdStore, id: &CoreId) -> Result<bool, CoreStoreError> {
    store.contains(id.to_string()).map_err(CoreStoreError::from)
}
