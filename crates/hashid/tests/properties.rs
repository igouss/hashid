//! Property tests: the rules, as opposed to the examples.
//!
//! The Gherkin suite pins behaviour at chosen points. These pin the invariants
//! that must hold at *every* point — most importantly the one the whole crate
//! exists to provide: the generator never returns an id the registry already
//! holds.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test code — a terse panic on a broken invariant beats Result plumbing"
)]

use std::collections::HashSet;
use std::fmt::Write as _;

use chrono::{DateTime, TimeZone as _, Utc};
use proptest::prelude::*;

use hashid::generate::{base36, digest, seed};
use hashid::{
    HashLength, Id, IdConfig, Minter, Prefix, Request, Slug,
    MAX_HASH_LEN,
    MAX_PREFIX_LEN, MAX_SLUG_LEN,
};

/// A lookup that finds nothing: the minter always gets its first candidate.
///
/// The `Result` is not optional despite never being `Err`: it is the shape
/// `Minter::mint` takes, because a real lookup can fail.
#[allow(
    clippy::unnecessary_wraps,
    reason = "the signature is dictated by `Minter::mint`, not by this body"
)]
fn nothing_taken(_: &Id) -> Result<bool, hashid::StoreError> {
    Ok(false)
}

/// A fixed instant, so a property that must not depend on the clock cannot
/// accidentally pass because the clock moved.
fn instant() -> DateTime<Utc> {
    Utc.timestamp_nanos(1_783_686_896_789_012_345)
}

fn generator(prefix: &str) -> Minter {
    Minter::with_config(IdConfig::new(Prefix::parse(prefix).unwrap()))
}

/// Decode a base36 string back to a `u64`, for round-trip checking.
fn from_base36(text: &str) -> Option<u64> {
    text.chars().try_fold(0_u64, |acc: u64, c: char| {
        let digit: u64 = u64::from(c.to_digit(36)?);
        acc.checked_mul(36)?.checked_add(digit)
    })
}

proptest! {
    /// Whatever we mint, we can parse — and it comes back unchanged.
    ///
    /// This is the closure property. Generation and parsing are written
    /// independently; nothing but a test forces them to agree on the grammar.
    #[test]
    fn every_minted_id_round_trips_through_the_parser(title in "\\PC{0,200}") {
        let request: Request<'_> = Request::new(&title).at(instant()).expect("the scenario instant is representable");
        let minted: Id = generator("br").mint(&request, nothing_taken).unwrap();

        let reparsed: Id = Id::parse(&minted.to_string()).expect("a minted id must parse");
        prop_assert_eq!(reparsed, minted);
    }

    /// Content-addressing: same content, same instant, same id. Always.
    #[test]
    fn minting_is_deterministic(
        title in "\\PC{1,100}",
        description in proptest::option::of("\\PC{0,200}"),
        creator in proptest::option::of("[a-z]{3,10}"),
    ) {
        let build = || -> Request<'_> {
            let mut request: Request<'_> = Request::new(&title).at(instant()).expect("the scenario instant is representable");
            request.description = description.as_deref();
            request.creator = creator.as_deref();
            request
        };
        let generator: Minter = generator("br");

        let first: Id = generator.mint(&build(), nothing_taken).unwrap();
        let second: Id = generator.mint(&build(), nothing_taken).unwrap();

        prop_assert_eq!(first, second);
    }

    /// The configured prefix survives minting, whatever the content.
    #[test]
    fn minting_preserves_the_configured_prefix(
        prefix in "[a-z]{1,15}",
        title in "\\PC{1,50}",
    ) {
        let generator: Minter = Minter::with_config(
            IdConfig::new(Prefix::normalize(&prefix).unwrap())
        );
        let request: Request<'_> = Request::new(&title).at(instant()).expect("the scenario instant is representable");

        let minted: Id = generator.mint(&request, nothing_taken).unwrap();

        prop_assert_eq!(minted.prefix().as_str(), prefix);
    }

    /// The digest is exactly as long as asked, and always base36.
    #[test]
    fn a_digest_has_the_requested_length(
        input in "\\PC{0,100}",
        length in 1_usize..=MAX_HASH_LEN,
    ) {
        let length: HashLength = HashLength::new(length).unwrap();
        let hash = digest(&input, length);

        prop_assert_eq!(hash.len(), length.get());
        prop_assert!(
            hash.as_str().chars().all(|c: char| c.is_ascii_lowercase() || c.is_ascii_digit()),
            "not base36: {}", hash
        );
    }

    /// Base36 encoding is lossless.
    #[test]
    fn base36_round_trips(num in any::<u64>()) {
        prop_assert_eq!(from_base36(&base36(num)), Some(num));
    }

    /// The seed is injective over content: distinct content, distinct seed.
    ///
    /// This is what length-prefixing buys, and it is the difference between a
    /// collision being merely improbable and a collision being *constructible*.
    /// A naive `title|description` join would let a caller move the boundary —
    /// title `a|b` would seed exactly as title `a` with description `b` — so
    /// two genuinely different records could be made to collide on demand.
    ///
    /// An absent field and an empty field are deliberately the same content
    /// (`None` hashes as `""`), so the pairs are normalized before being
    /// compared.
    #[test]
    fn the_seed_is_injective_over_content(
        first_title in "\\PC{0,40}",
        first_description in proptest::option::of("\\PC{0,40}"),
        second_title in "\\PC{0,40}",
        second_description in proptest::option::of("\\PC{0,40}"),
    ) {
        let first: (&str, &str) = (&first_title, first_description.as_deref().unwrap_or(""));
        let second: (&str, &str) = (&second_title, second_description.as_deref().unwrap_or(""));
        prop_assume!(first != second);

        let seed_of = |content: (&str, &str)| -> String {
            let mut request: Request<'_> = Request::new(content.0).at(instant()).expect("the scenario instant is representable");
            request.description = Some(content.1);
            seed(&request, 0)
        };

        prop_assert_ne!(seed_of(first), seed_of(second));
    }

    /// A slug is always safe to splice into an id.
    #[test]
    fn a_normalized_slug_is_always_id_safe(raw in "\\PC{0,200}") {
        let Some(slug) = Slug::normalize(&raw) else {
            return Ok(());  // Nothing usable survived; the caller falls back.
        };
        let text: &str = slug.as_str();

        prop_assert!(!text.is_empty());
        prop_assert!(text.len() <= MAX_SLUG_LEN);
        prop_assert!(!text.starts_with('-'), "leading hyphen: {text}");
        prop_assert!(!text.ends_with('-'), "trailing hyphen: {text}");
        prop_assert!(!text.contains("--"), "doubled hyphen: {text}");
        prop_assert!(
            text.chars().all(|c: char| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
            "unsafe character: {text}"
        );
    }

    /// Truncating a slug produces a slug that still fits, and is still a slug.
    ///
    /// The interesting direction is the failure: `truncated_to` must never hand
    /// back something *longer* than the budget it was given, because the only
    /// caller is the prefix-budget check, and an over-long return there emits an
    /// id the crate's own parser would reject.
    #[test]
    fn a_truncated_slug_fits_its_budget_and_stays_id_safe(
        raw in "\\PC{0,120}",
        budget in 0_usize..60,
    ) {
        let Some(slug) = Slug::normalize(&raw) else {
            return Ok(());  // Nothing usable survived.
        };

        match slug.truncated_to(budget) {
            Some(cut) => {
                prop_assert!(cut.len() <= budget, "`{cut}` exceeds the budget of {budget}");
                prop_assert!(!cut.as_str().is_empty());
                prop_assert!(!cut.as_str().ends_with('-'), "trailing hyphen: {cut}");
            }
            None => prop_assert!(
                slug.len() > budget,
                "a slug of length {} should have fitted a budget of {budget}",
                slug.len()
            ),
        }
    }

    /// A prefix joined with a slug never overruns the prefix budget.
    ///
    /// If it did, minting would emit ids that `parse_id` then rejects — the
    /// crate would be producing values it cannot read back.
    #[test]
    fn a_slugged_prefix_never_overruns_the_budget(
        prefix in "[a-z]{1,64}",
        slug in "\\PC{1,80}",
    ) {
        let prefix: Prefix = Prefix::parse(&prefix).unwrap();
        let Some(slug) = Slug::normalize(&slug) else {
            return Ok(());  // Nothing usable survived.
        };

        let Some(joined) = prefix.joined_with(&slug) else {
            return Ok(());  // No room; the caller falls back to a hash-only id.
        };

        prop_assert!(
            joined.len() <= MAX_PREFIX_LEN,
            "`{joined}` is {} bytes, over the {MAX_PREFIX_LEN}-byte budget",
            joined.len()
        );
        prop_assert!(
            Prefix::parse(joined.as_str()).is_ok(),
            "a joined prefix must still be a legal prefix: {joined}"
        );
    }

    /// Lenient normalization is a subset of strict parsing.
    ///
    /// Whatever `Prefix::normalize` accepts from a human, `Prefix::parse` must
    /// accept back from a machine. If these two ever disagree, a config file
    /// could produce ids that the crate's own parser then rejects.
    #[test]
    fn a_normalized_prefix_always_parses_strictly(raw in "\\PC{0,120}") {
        let Ok(normalized) = Prefix::normalize(&raw) else {
            return Ok(());  // Nothing usable survived.
        };
        let reparsed = Prefix::parse(normalized.as_str());

        prop_assert!(
            reparsed.is_ok(),
            "normalize produced `{}`, which parse rejects",
            normalized
        );
        prop_assert_eq!(reparsed.unwrap(), normalized);
    }

    /// A hierarchical id keeps its full child path through a round trip.
    #[test]
    fn a_child_path_survives_a_round_trip(
        hash in "[a-z0-9]{3,12}",
        path in proptest::collection::vec(0_u32..1000, 0..5),
    ) {
        let suffix: String = path.iter().fold(String::new(), |mut acc: String, segment: &u32| {
            let _ = write!(acc, ".{segment}");
            acc
        });
        let raw: String = format!("br-{hash}{suffix}");

        let parsed: Id = Id::parse(&raw).expect("a well-formed id must parse");

        prop_assert_eq!(parsed.depth(), path.len());
        prop_assert_eq!(parsed.child_path(), path.as_slice());
        prop_assert_eq!(parsed.to_string(), raw);
    }

    /// The adaptive hash length never shrinks as the population grows.
    ///
    /// A population that grew must never mint *narrower* ids than it used to.
    #[test]
    fn the_hash_length_never_shrinks_as_the_population_grows(
        smaller in 0_usize..1_000_000,
        delta in 0_usize..1_000_000,
    ) {
        let generator: Minter = generator("br");
        let larger: usize = smaller + delta;

        prop_assert!(
            generator.hash_length_for(larger) >= generator.hash_length_for(smaller),
            "population {smaller} -> {larger} narrowed the hash"
        );
    }

    /// **The** safety property: minting into a registry never returns an id the
    /// registry already holds — for zero, one, or many pre-existing ids.
    ///
    /// Note the registry grows as we go, so this also covers the case where the
    /// generator must step over an id it minted moments earlier.
    #[test]
    fn minting_never_returns_an_id_that_is_already_taken(
        titles in proptest::collection::hash_set("\\PC{1,60}", 0..20),
    ) {
        let generator: Minter = generator("br");
        let mut known: std::collections::HashSet<Id> = std::collections::HashSet::new();
        let mut minted: HashSet<Id> = HashSet::new();

        titles.iter().try_for_each(|title: &String| -> Result<(), TestCaseError> {
            let request: Request<'_> = Request::new(title).at(instant()).expect("the scenario instant is representable")
                .with_population(known.len());
            let id: Id = generator
                .mint(&request, |id: &Id| Ok(known.contains(id)))
                .expect("minting must succeed");

            prop_assert!(
                !known.contains(&id),
                "generator returned an id the registry already held: {id}"
            );
            prop_assert!(minted.insert(id.clone()), "duplicate id minted: {id}");
            known.insert(id);
            Ok(())
        })?;

        prop_assert_eq!(known.len(), titles.len());
    }
}
