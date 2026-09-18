//! The minting use case: content in, unique ID out.

use crate::domain::config::IdConfig;
use crate::domain::hash::Base36Hash;
use crate::domain::length::HashLength;
use crate::domain::id::Id;
use crate::domain::prefix::Prefix;
use crate::domain::request::Request;
use crate::domain::slug::Slug;
use crate::error::{IdError, Result};
use crate::generate::digest::digest;
use crate::generate::length::optimal_length;
use crate::generate::seed::seed;
use crate::store::StoreError;

/// How many nonces to try before widening the hash.
const NONCES_PER_LENGTH: u32 = 10;
/// Hash length used once the configured maximum is exhausted.
const FALLBACK_HASH_LEN: HashLength = HashLength::clamped(12);
/// Nonce past which the fallback also appends the nonce itself to the hash.
const DESPERATE_NONCE: u32 = 1_000;
/// Nonce past which the generator gives up and reports exhaustion.
const MAX_NONCE: u32 = 2_000;

/// Mints unique IDs, asking your store one question per candidate.
///
/// ```
/// # use hashid::{Id, Minter, Request};
/// let minter: Minter = Minter::new("proj")?;
///
/// let id: Id = minter.mint(&Request::new("Ship it"), |_: &Id| Ok(false))?;
/// assert_eq!(id.prefix().as_str(), "proj");
/// # Ok::<(), hashid::IdError>(())
/// ```
#[derive(Debug, Clone)]
pub struct Minter {
    config: IdConfig,
}

impl Minter {
    /// A minter for `prefix`, with the default entropy policy.
    ///
    /// The prefix is normalized the lenient way — trimmed, lowercased, illegal
    /// characters dropped — because it usually comes from a human or a config
    /// file. `  Project-Name_2!  ` becomes `project-name_2`.
    ///
    /// # Errors
    ///
    /// [`IdError::UnusablePrefix`] when no usable character survives. No
    /// substitute is invented: a silent fallback would attach real IDs to a
    /// namespace you never chose.
    pub fn new(prefix: &str) -> Result<Self> {
        Ok(Self {
            config: IdConfig::new(Prefix::normalize(prefix)?),
        })
    }

    /// A minter bound to an explicit `config`, for a non-default entropy
    /// policy or a prefix you have already validated strictly.
    #[must_use]
    pub fn with_config(config: IdConfig) -> Self {
        Self { config }
    }

    /// The policy this minter mints under.
    #[must_use]
    pub fn config(&self) -> &IdConfig {
        &self.config
    }

    /// The hash length this generator would choose for a given `population`.
    ///
    /// Exposed because it is useful to assert on, and because a caller sizing a
    /// migration wants to know what it is about to produce.
    #[must_use]
    pub fn hash_length_for(&self, population: usize) -> HashLength {
        optimal_length(&self.config, population)
    }

    /// Mint an ID from `request` at an explicit `nonce` and `hash_length`,
    /// **without consulting your store**.
    ///
    /// Purely deterministic: identical arguments always yield an identical ID.
    /// This is the content-addressing primitive — useful for reproducing a
    /// known ID, or for tests — but it offers no uniqueness guarantee
    /// whatsoever. If you want a unique ID, call [`Minter::mint`].
    ///
    /// The name says `unchecked` because that is the entire contract. An
    /// innocuously-named convenience wrapper that quietly skips the collision
    /// check is a trap; this one cannot be called by accident.
    #[must_use]
    pub fn mint_unchecked(
        &self,
        request: &Request<'_>,
        nonce: u32,
        hash_length: HashLength,
    ) -> Id {
        let hash: Base36Hash = digest(&seed(request, nonce), hash_length);
        Id::root(self.config.prefix().clone(), hash)
    }

    /// Mint an ID your store does not already hold.
    ///
    /// Walks an escalation ladder, checking every candidate:
    ///
    /// 1. Ten nonces at the adaptive hash length for `request.population`.
    /// 2. All ten taken? Widen the hash by one character and repeat, up to
    ///    [`IdConfig::max_hash_length`].
    /// 3. Still stuck? Fall back to a 12-character hash and walk the nonce
    ///    upward.
    /// 4. Past a thousand of those, append the nonce to the hash as well.
    ///
    /// # Errors
    ///
    /// - [`IdError::Exhausted`] if the ladder runs out. The generator returns
    ///   this **instead of** an ID it knows to be taken — a duplicate ID is a
    ///   silent data-corruption bug, an error is merely a loud one.
    /// - [`IdError::Store`] if your lookup cannot answer. The minter does
    ///   not guess.
    pub fn mint<F>(&self, request: &Request<'_>, mut taken: F) -> Result<Id>
    where
        F: FnMut(&Id) -> core::result::Result<bool, StoreError>,
    {
        let mut attempts: u32 = 0;
        let mut length: HashLength = self.hash_length_for(request.population);

        loop {
            for nonce in 0..NONCES_PER_LENGTH {
                let candidate: Id = self.mint_unchecked(request, nonce, length);
                attempts += 1;
                if !taken(&candidate)? {
                    return Ok(candidate);
                }
            }

            if length < self.config.max_hash_length() {
                length = length.wider();
            } else {
                return self.mint_from_fallback(request, &mut taken, attempts);
            }
        }
    }

    /// Mint an ID carrying a human-readable `slug`, e.g.
    /// `proj-survey-my-thing-8cda`.
    ///
    /// The slug sits between prefix and hash, and the hash is still always
    /// appended — two records may share a slug, so the slug alone can never
    /// carry uniqueness.
    ///
    /// Falls back to a plain hash-only ID (via [`Minter::mint`]) when the
    /// slug normalizes to nothing, when it cannot fit the prefix budget, or
    /// when the slugged form exhausts its hash lengths. Producing a valid ID
    /// always beats honouring a cosmetic request.
    ///
    /// # Errors
    ///
    /// As [`Minter::mint`].
    pub fn mint_with_slug<F>(
        &self,
        request: &Request<'_>,
        slug: &str,
        mut taken: F,
    ) -> Result<Id>
    where
        F: FnMut(&Id) -> core::result::Result<bool, StoreError>,
    {
        let Some(slug) = Slug::normalize(slug) else {
            return self.mint(request, &mut taken);
        };
        let Some(slugged) = self.config.prefix().joined_with(&slug) else {
            return self.mint(request, &mut taken);
        };

        let mut length: HashLength = self.hash_length_for(request.population);

        loop {
            for nonce in 0..NONCES_PER_LENGTH {
                let hash: Base36Hash = digest(&seed(request, nonce), length);
                let candidate: Id = Id::root(slugged.clone(), hash);
                if !taken(&candidate)? {
                    return Ok(candidate);
                }
            }

            if length < self.config.max_hash_length() {
                length = length.wider();
            } else {
                // The slugged namespace is saturated. Drop the slug rather
                // than grow the prefix past what the grammar allows.
                return self.mint(request, &mut taken);
            }
        }
    }

    /// Tiers 3 and 4 of the ladder: a wide hash, then a wide hash plus nonce.
    fn mint_from_fallback<F>(
        &self,
        request: &Request<'_>,
        taken: &mut F,
        mut attempts: u32,
    ) -> Result<Id>
    where
        F: FnMut(&Id) -> core::result::Result<bool, StoreError>,
    {
        let prefix: &Prefix = self.config.prefix();
        let mut nonce: u32 = 0;

        loop {
            let hash: Base36Hash = digest(&seed(request, nonce), FALLBACK_HASH_LEN);

            let candidate: Id = Id::root(prefix.clone(), hash.clone());
            attempts += 1;
            if !taken(&candidate)? {
                return Ok(candidate);
            }

            nonce += 1;

            if nonce > DESPERATE_NONCE {
                // Splice the nonce into the hash itself for extra room. Decimal
                // digits are valid base36, so the result still parses.
                let widened: Base36Hash = Base36Hash::parse(&format!("{hash}{nonce}"))?;
                let candidate: Id = Id::root(prefix.clone(), widened);
                attempts += 1;
                if !taken(&candidate)? {
                    return Ok(candidate);
                }
            }

            if nonce > MAX_NONCE {
                return Err(IdError::Exhausted {
                    prefix: prefix.as_str().to_owned(),
                    attempts,
                });
            }
        }
    }
}
