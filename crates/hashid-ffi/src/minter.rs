//! The minting use case, as a foreign object.

use std::sync::Arc;

use hashid::{HashLength, Id as CoreId, Minter as CoreMinter};

use crate::error::HashidError;
use crate::model::{MinterConfig, Request};
use crate::num::{to_u32, to_usize};
use crate::store::{taken, IdStore};

/// Mints unique IDs against a store you supply.
///
/// Hold on to one per namespace; it is immutable and safe to share across
/// threads.
#[derive(Debug, uniffi::Object)]
pub struct Minter {
    inner: CoreMinter,
}

#[uniffi::export]
impl Minter {
    /// A minter for `prefix`, with the default entropy policy.
    ///
    /// The prefix is normalized the lenient way — trimmed, lowercased, illegal
    /// characters dropped — because it usually comes from a human or a config
    /// file. `  Project-Name_2!  ` becomes `project-name_2`.
    ///
    /// # Errors
    ///
    /// `HashidError::UnusablePrefix` when no usable character survives. No
    /// substitute is invented: a silent fallback would attach real IDs to a
    /// namespace you never chose.
    #[uniffi::constructor]
    pub fn new(prefix: String) -> Result<Arc<Self>, HashidError> {
        Ok(Arc::new(Self {
            inner: CoreMinter::new(&prefix)?,
        }))
    }

    /// A minter bound to an explicit `config`, for a non-default entropy
    /// policy or a prefix you have already validated strictly.
    ///
    /// # Errors
    ///
    /// `HashidError::Malformed` if `config.prefix` is not already a legal
    /// prefix — `MinterConfig` takes it strictly, so run it through
    /// `normalize_prefix` first if it comes from a human.
    /// `HashidError::InvalidHashBounds` or `HashidError::InvalidCollisionProb`
    /// if the entropy policy is not satisfiable.
    #[uniffi::constructor]
    pub fn with_config(config: MinterConfig) -> Result<Arc<Self>, HashidError> {
        Ok(Arc::new(Self {
            inner: CoreMinter::with_config(config.to_core()?),
        }))
    }

    /// Mint an ID at an explicit `nonce` and `hash_length`, **without
    /// consulting any store**.
    ///
    /// Purely deterministic: identical arguments yield an identical ID in every
    /// language. This is the content-addressing primitive — use it to reproduce
    /// a known ID or to check an implementation against the conformance
    /// vectors. It offers no uniqueness guarantee whatsoever; for that, call
    /// `Minter::mint`.
    ///
    /// The name says `unchecked` because that is the entire contract.
    ///
    /// # Errors
    ///
    /// `HashidError::InvalidHashLength` when `hash_length` is zero or wider
    /// than the grammar allows. There is no hash of zero characters, so there
    /// is nothing honest to return.
    pub fn mint_unchecked(
        &self,
        request: Request,
        nonce: i64,
        hash_length: i64,
    ) -> Result<String, HashidError> {
        let hash_length: HashLength =
            HashLength::new(to_usize("hashLength", hash_length)?)?;
        Ok(self
            .inner
            .mint_unchecked(
                &request.as_core()?,
                to_u32("nonce", nonce)?,
                hash_length,
            )
            .to_string())
    }

    /// Mint an ID that `store` does not already hold.
    ///
    /// Walks an escalation ladder, checking every candidate: ten nonces at the
    /// adaptive length, then a wider hash, then a 12-character fallback, then
    /// the fallback with the nonce spliced in.
    ///
    /// # Errors
    ///
    /// - `HashidError::Exhausted` if the ladder runs out. Returned *instead
    ///   of* an ID known to be taken: a duplicate ID is a silent
    ///   data-corruption bug, an error is merely a loud one.
    /// - `HashidError::Store` if your store could not answer. The generator
    ///   does not guess.
    pub fn mint(
        &self,
        request: Request,
        store: Arc<dyn IdStore>,
    ) -> Result<String, HashidError> {
        let minted: CoreId = self.inner
            .mint(&request.as_core()?, |id: &CoreId| taken(store.as_ref(), id))?;
        Ok(minted.to_string())
    }

    /// Mint an ID carrying a human-readable slug, e.g.
    /// `proj-survey-my-thing-8cda`.
    ///
    /// The slug sits between prefix and hash, and the hash is still always
    /// appended — two records may share a slug, so the slug alone can never
    /// carry uniqueness.
    ///
    /// Falls back to a plain hash-only ID when the slug normalizes to nothing,
    /// when it cannot fit the prefix budget, or when the slugged namespace is
    /// saturated. Producing a valid ID always beats honouring a cosmetic
    /// request.
    ///
    /// # Errors
    ///
    /// As `Minter::mint`.
    pub fn mint_with_slug(
        &self,
        request: Request,
        slug: String,
        store: Arc<dyn IdStore>,
    ) -> Result<String, HashidError> {
        let minted: CoreId =
            self.inner
                .mint_with_slug(&request.as_core()?, &slug, |id: &CoreId| taken(store.as_ref(), id))?;
        Ok(minted.to_string())
    }
}

