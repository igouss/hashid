//! Adapters: ready-made implementations of the [`crate::port`] traits.
//!
//! Boundaries, in ECB terms. Nothing in the domain depends on anything here —
//! delete this module and the crate still compiles, it just gives you more to
//! write yourself.

pub mod closure;
pub mod in_memory;

pub use closure::{FnIndex, FnRegistry};
pub use in_memory::InMemoryIds;
