//! Capability: mint a new ID from content.
//!
//! Split by the question each file answers:
//!
//! - [`seed`](mod@seed) — which exact bytes get hashed?
//! - [`digest`](mod@digest) — how do those bytes become a short base36 string?
//! - [`length`] — how many characters of it do we need?
//! - [`generator`] — what if that ID is already taken?
//!
//! [`seed()`], [`digest()`], and [`base36()`] are public on purpose: together
//! they *are* the wire format. Anyone reimplementing `hashid` in another
//! language, or asserting that a stored ID is still reproducible, needs to be
//! able to call them directly.

pub mod digest;
pub mod generator;
pub mod length;
pub mod seed;

pub use digest::{base36, digest};
pub use generator::Minter;
pub use seed::seed;
