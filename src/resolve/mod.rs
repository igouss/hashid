//! Capability: turn a partial, human-typed fragment into the ID it meant.
//!
//! Short IDs exist to be retyped, and a human retyping `br-8cda9f21` will
//! reliably type `8cda` instead. Resolution is what makes that work.

pub mod config;
pub mod matcher;
pub mod resolver;

pub use config::ResolverConfig;
pub use matcher::{find_matching, matches_fragment};
pub use resolver::IdResolver;
