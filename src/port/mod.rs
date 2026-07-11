//! Ports: the interfaces the crate needs the outside world to satisfy.
//!
//! Dependencies point inward. These traits are defined here, in terms of
//! domain types only, and implemented out in [`crate::adapter`] (or in your
//! code, against your store).

pub mod error;
pub mod registry;

pub use error::RegistryError;
pub use registry::{IdIndex, IdRegistry};
