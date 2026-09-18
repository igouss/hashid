//! Capability: turn text into a validated [`Id`], or say precisely why not.
//!
//! This is the crate's trust boundary. Every ID arriving from outside — a CLI
//! argument, an imported file, an API payload — passes through here, and what
//! comes out the far side cannot be malformed.
//!
//! [`Id`]: crate::domain::id::Id

pub mod grammar;
pub mod validate;

pub use grammar::{is_valid_id, parse_id};
pub use validate::validate_prefix;
