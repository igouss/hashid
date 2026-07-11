//! The minting use case: content in, unique ID out.

use crate::domain::config::IdConfig;
use crate::domain::hash::Base36Hash;
use crate::domain::id::Id;
use crate::domain::prefix::Prefix;
use crate::domain::request::IdRequest;
use crate::domain::slug::Slug;
use crate::error::{IdError, Result};
use crate::generate::digest::digest;
use crate::generate::length::optimal_length;
use crate::generate::seed::seed;
use crate::port::IdRegistry;

/// How many nonces to try before widening the hash.
const NONCES_PER_LENGTH: u32 = 10;
/// Hash length used once the configured maximum is exhausted.
const FALLBACK_HASH_LEN: usize = 12;
/// Nonce past which the fallback also appends the nonce itself to the hash.
const DESPERATE_NONCE: u32 = 1_000;
/// Nonce past which the generator gives up and reports exhaustion.
const MAX_NONCE: u32 = 2_000;

/// Mints unique IDs against a caller-supplied registry.
///
/// ```
/// # use chrono::Utc;
/// # use hashid::{IdConfig, IdGenerator, IdRequest, InMemoryIds, Prefix};
/// let generator: IdGenerator = IdGenerator::new(IdConfig::new(Prefix::normalize("proj")?));
/// let taken: InMemoryIds = InMemoryIds::new();
///
/// let id = generator.mint(&IdRequest::new("Ship it", Utc::now(), 0), &taken)?;
/// assert_eq!(id.prefix().as_str(), "proj");
/// # Ok::<(), hashid::IdError>(())
/// ```
#[derive(Debug, Clone)]
pub struct IdGenerator {
    config: IdConfig,
}

impl IdGenerator {
    /// A generator bound to `config`.
    #[must_use]
    pub fn new(config: IdConfig) -> Self {
        Self { config }
    }

    /// The policy this generator mints under.
    #[must_use]
    pub fn config(&self) -> &IdConfig {
        &self.config
    }

    /// The hash length this generator would choose for a given `population`.
    ///
    /// Exposed because it is useful to assert on, and because a caller sizing a
    /// migration wants to know what it is about to produce.
    #[must_use]
    pub fn hash_length_for(&self, population: usize) -> usize {
        optimal_length(&self.config, population)
    }

    /// Mint an ID from `request` at an explicit `nonce` and `hash_length`,
    /// **without consulting any registry**.
    ///
    /// Purely deterministic: identical arguments always yield an identical ID.
    /// This is the content-addressing primitive — useful for reproducing a
    /// known ID, or for tests — but it offers no uniqueness guarantee
    /// whatsoever. If you want a unique ID, call [`IdGenerator::mint`].
    ///
    /// The name says `unchecked` because that is the entire contract. An
    /// innocuously-named convenience wrapper that quietly skips the collision
    /// check is a trap; this one cannot be called by accident.
    #[must_use]
    pub fn mint_unchecked(&self, request: &IdRequest<'_>, nonce: u32, hash_length: usize) -> Id {
        let hash: Base36Hash = digest(&seed(request, nonce), hash_length);
        Id::root(self.config.prefix().clone(), hash)
    }

    /// Mint an ID that `registry` does not already hold.
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
    /// - [`IdError::Registry`] if the registry cannot answer a lookup. The
    ///   generator does not guess.
    pub fn mint<R: IdRegistry + ?Sized>(
        &self,
        request: &IdRequest<'_>,
        registry: &R,
    ) -> Result<Id> {
        let mut attempts: u32 = 0;
        let mut length: usize = self.hash_length_for(request.population);

        loop {
            for nonce in 0..NONCES_PER_LENGTH {
                let candidate: Id = self.mint_unchecked(request, nonce, length);
                attempts += 1;
                if !registry.contains(&candidate)? {
                    return Ok(candidate);
                }
            }

            if length < self.config.max_hash_length() {
                length += 1;
            } else {
                return self.mint_from_fallback(request, registry, attempts);
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
    /// Falls back to a plain hash-only ID (via [`IdGenerator::mint`]) when the
    /// slug normalizes to nothing, when it cannot fit the prefix budget, or
    /// when the slugged form exhausts its hash lengths. Producing a valid ID
    /// always beats honouring a cosmetic request.
    ///
    /// # Errors
    ///
    /// As [`IdGenerator::mint`].
    pub fn mint_with_slug<R: IdRegistry + ?Sized>(
        &self,
        request: &IdRequest<'_>,
        slug: &str,
        registry: &R,
    ) -> Result<Id> {
        let Some(slug) = Slug::normalize(slug) else {
            return self.mint(request, registry);
        };
        let Some(slugged) = self.config.prefix().joined_with(&slug) else {
            return self.mint(request, registry);
        };

        let mut length: usize = self.hash_length_for(request.population);

        loop {
            for nonce in 0..NONCES_PER_LENGTH {
                let hash: Base36Hash = digest(&seed(request, nonce), length);
                let candidate: Id = Id::root(slugged.clone(), hash);
                if !registry.contains(&candidate)? {
                    return Ok(candidate);
                }
            }

            if length < self.config.max_hash_length() {
                length += 1;
            } else {
                // The slugged namespace is saturated. Drop the slug rather
                // than grow the prefix past what the grammar allows.
                return self.mint(request, registry);
            }
        }
    }

    /// Tiers 3 and 4 of the ladder: a wide hash, then a wide hash plus nonce.
    fn mint_from_fallback<R: IdRegistry + ?Sized>(
        &self,
        request: &IdRequest<'_>,
        registry: &R,
        mut attempts: u32,
    ) -> Result<Id> {
        let prefix: &Prefix = self.config.prefix();
        let mut nonce: u32 = 0;

        loop {
            let hash: Base36Hash = digest(&seed(request, nonce), FALLBACK_HASH_LEN);

            let candidate: Id = Id::root(prefix.clone(), hash.clone());
            attempts += 1;
            if !registry.contains(&candidate)? {
                return Ok(candidate);
            }

            nonce += 1;

            if nonce > DESPERATE_NONCE {
                // Splice the nonce into the hash itself for extra room. Decimal
                // digits are valid base36, so the result still parses.
                let widened: Base36Hash = Base36Hash::parse(&format!("{hash}{nonce}"))?;
                let candidate: Id = Id::root(prefix.clone(), widened);
                attempts += 1;
                if !registry.contains(&candidate)? {
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
