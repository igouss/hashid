//! Entities and value objects. Framework-free: nothing here knows about
//! storage, I/O, or any adapter.
//!
//! Every type in this layer is *correct by construction* — a [`Prefix`],
//! [`Slug`], [`Base36Hash`], or [`Id`] you are holding has already satisfied
//! the grammar, so nothing downstream re-validates it.
//!
//! [`Prefix`]: prefix::Prefix
//! [`Slug`]: slug::Slug
//! [`Base36Hash`]: hash::Base36Hash
//! [`Id`]: id::Id

pub mod config;
pub mod hash;
pub mod id;
pub mod length;
pub mod limits;
pub mod prefix;
pub mod request;
pub mod slug;
