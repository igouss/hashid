//! Golden vectors: the ID format is frozen.
//!
//! Every expected value below was produced by running the *original*
//! implementation this crate was extracted from (`beads_rust`, `src/util/id.rs`)
//! verbatim, against a frozen instant. They are hard-coded here, so this suite
//! depends on nothing but `hashid` itself.
//!
//! These tests are the reason the extraction can be trusted. A refactor that
//! quietly changed the seed encoding, the digest truncation, or the base36
//! alphabet would keep every behavioural test green — IDs would still *look*
//! like IDs — while silently orphaning every ID ever minted. That is exactly
//! the false green worth spending a test suite on.
//!
//! If you change any expected string here, you have changed the wire format.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test code — a terse panic on a broken invariant beats Result plumbing"
)]

use chrono::{DateTime, Utc};
use hashid::generate::{base36, digest, seed};
use hashid::{HashLength, IdConfig, Minter, Prefix, Request};

/// The instant every vector was frozen at: `1783686896789012345` nanos.
const FROZEN: &str = "2026-07-10T12:34:56.789012345Z";

fn frozen_instant() -> DateTime<Utc> {
    FROZEN
        .parse::<DateTime<Utc>>()
        .expect("frozen instant parses")
}

/// A hash length, for a vector that pins one explicitly.
fn len(value: usize) -> HashLength {
    HashLength::new(value).expect("golden lengths are within the grammar")
}

fn generator_for(prefix: &str) -> Minter {
    let prefix: Prefix = Prefix::parse(prefix).expect("golden prefixes are legal");
    Minter::with_config(IdConfig::new(prefix))
}

/// `request(title)` at the frozen instant; population is irrelevant to the
/// vectors below, which all pin the hash length explicitly.
fn request(title: &str) -> Request<'_> {
    Request::new(title)
        .at(frozen_instant())
        .expect("the frozen instant is representable")
}

// ---------------------------------------------------------------------------
// Seed encoding: `<byte-len>:<value>` per field, in a fixed order.
// ---------------------------------------------------------------------------

#[test]
fn seed_encodes_all_five_fields_length_prefixed() {
    let req: Request<'_> = request("title").with_description("desc").with_creator("me");
    assert_eq!(seed(&req, 0), "5:title4:desc2:me19:17836868967890123451:0");
}

#[test]
fn seed_encodes_absent_optional_fields_as_empty() {
    assert_eq!(
        seed(&request("title"), 0),
        "5:title0:0:19:17836868967890123451:0"
    );
}

#[test]
fn seed_encodes_empty_title() {
    assert_eq!(seed(&request(""), 0), "0:0:0:19:17836868967890123451:0");
}

#[test]
fn seed_keeps_field_boundaries_unforgeable() {
    // The whole reason for length-prefixing: title "a" + description "b" must
    // not seed the same as title "a|b" with no description.
    let split_fields: String = seed(&request("a").with_description("b"), 0);
    let delimiter_inside_title: String = seed(&request("a|b"), 0);

    assert_eq!(split_fields, "1:a1:b0:19:17836868967890123451:0");
    assert_eq!(delimiter_inside_title, "3:a|b0:0:19:17836868967890123451:0");
    assert_ne!(split_fields, delimiter_inside_title);
}

#[test]
fn seed_counts_bytes_not_characters() {
    // "héllo wörld" is 11 chars but 13 bytes; "multi\nline" is 10 bytes.
    let req: Request<'_> = request("héllo wörld")
        .with_description("multi\nline")
        .with_creator("iouri");
    assert_eq!(
        seed(&req, 3),
        "13:héllo wörld10:multi\nline5:iouri19:17836868967890123451:3"
    );
}

#[test]
fn seed_carries_the_nonce_last() {
    assert_eq!(
        seed(&request("Test Issue"), 7),
        "10:Test Issue0:0:19:17836868967890123451:7"
    );
}

#[test]
fn seed_handles_the_maximum_nonce() {
    let req: Request<'_> = request("x").with_description("").with_creator("");
    assert_eq!(
        seed(&req, u32::MAX),
        "1:x0:0:19:178368689678901234510:4294967295"
    );
}

// ---------------------------------------------------------------------------
// Digest: SHA-256 → leading 8 bytes → u64 → base36 → last N characters.
// ---------------------------------------------------------------------------

#[test]
fn digest_takes_the_last_n_characters() {
    // Each shorter length is a suffix of the longer one — that is what
    // "truncate from the tail" means, and it is load-bearing.
    assert_eq!(digest("test input", len(1)).as_str(), "v");
    assert_eq!(digest("test input", len(3)).as_str(), "44v");
    assert_eq!(digest("test input", len(4)).as_str(), "j44v");
    assert_eq!(digest("test input", len(8)).as_str(), "tgwoj44v");
    assert_eq!(digest("test input", len(12)).as_str(), "ehtwtgwoj44v");
}

#[test]
fn digest_left_pads_when_the_encoding_is_short() {
    // A u64 is at most 13 base36 characters, so 20 must be zero-padded.
    assert_eq!(digest("test input", len(20)).as_str(), "00000002ehtwtgwoj44v");
}

#[test]
fn digest_hashes_the_empty_string() {
    assert_eq!(digest("", len(3)).as_str(), "3tg");
    assert_eq!(digest("", len(8)).as_str(), "kheu33tg");
    assert_eq!(digest("", len(12)).as_str(), "gng7kheu33tg");
}

#[test]
fn digest_hashes_a_single_character() {
    assert_eq!(digest("a", len(3)).as_str(), "oay");
    assert_eq!(digest("a", len(8)).as_str(), "vyj5voay");
    assert_eq!(digest("a", len(12)).as_str(), "2wshvyj5voay");
}

// ---------------------------------------------------------------------------
// Base36 encoding.
// ---------------------------------------------------------------------------

#[test]
fn base36_encodes_zero() {
    assert_eq!(base36(0), "0");
}

#[test]
fn base36_encodes_the_alphabet_boundaries() {
    assert_eq!(base36(1), "1");
    assert_eq!(base36(10), "a");
    assert_eq!(base36(35), "z");
    assert_eq!(base36(36), "10");
}

#[test]
fn base36_rolls_over_at_each_digit() {
    assert_eq!(base36(1_295), "zz");
    assert_eq!(base36(1_296), "100");
}

#[test]
fn base36_encodes_the_largest_u64() {
    assert_eq!(base36(u64::MAX), "3w5e11264sgsf");
}

// ---------------------------------------------------------------------------
// Whole IDs, end to end.
// ---------------------------------------------------------------------------

#[test]
fn mints_the_golden_id_for_a_full_request() {
    let req: Request<'_> = request("Test Issue")
        .with_description("Description")
        .with_creator("user");
    assert_eq!(
        generator_for("br").mint_unchecked(&req, 0, len(3)).to_string(),
        "br-grz"
    );
}

#[test]
fn a_different_nonce_yields_a_different_golden_id() {
    let req: Request<'_> = request("Test Issue")
        .with_description("Description")
        .with_creator("user");
    assert_eq!(
        generator_for("br").mint_unchecked(&req, 1, len(3)).to_string(),
        "br-2cs"
    );
}

#[test]
fn a_longer_hash_yields_the_golden_wide_id() {
    let req: Request<'_> = request("Test Issue")
        .with_description("Description")
        .with_creator("user");
    assert_eq!(
        generator_for("br").mint_unchecked(&req, 0, len(8)).to_string(),
        "br-4scx9grz"
    );
}

#[test]
fn the_twelve_char_fallback_hash_is_golden() {
    assert_eq!(
        generator_for("br")
            .mint_unchecked(&request("Test Issue"), 0, len(12))
            .to_string(),
        "br-lsdz9lnsvdtv"
    );
}

#[test]
fn the_prefix_does_not_perturb_the_hash() {
    // Same content, different namespace: the hash is identical, and `bd-dtv`
    // is the 3-char suffix of the 12-char `br-lsdz9lnsvdtv` above.
    assert_eq!(
        generator_for("bd")
            .mint_unchecked(&request("Test Issue"), 0, len(3))
            .to_string(),
        "bd-dtv"
    );
}

#[test]
fn mints_the_golden_id_for_a_multi_character_prefix() {
    assert_eq!(
        generator_for("myproj")
            .mint_unchecked(&request("Whatever"), 0, len(4))
            .to_string(),
        "myproj-8xhu"
    );
}

#[test]
fn mints_the_golden_id_for_an_empty_title() {
    assert_eq!(
        generator_for("br")
            .mint_unchecked(&request(""), 0, len(3))
            .to_string(),
        "br-ody"
    );
}

// ---------------------------------------------------------------------------
// Adaptive hash length: the birthday bound, pinned.
// ---------------------------------------------------------------------------

#[test]
fn a_small_population_gets_the_minimum_hash_length() {
    let generator: Minter = generator_for("br");
    assert_eq!(generator.hash_length_for(0).get(), 3);
    assert_eq!(generator.hash_length_for(1).get(), 3);
    assert_eq!(generator.hash_length_for(10).get(), 3);
    assert_eq!(generator.hash_length_for(100).get(), 3);
}

#[test]
fn a_growing_population_widens_the_hash() {
    let generator: Minter = generator_for("br");
    assert_eq!(generator.hash_length_for(1_000).get(), 5);
    assert_eq!(generator.hash_length_for(10_000).get(), 6);
    assert_eq!(generator.hash_length_for(100_000).get(), 7);
}

#[test]
fn a_huge_population_saturates_at_the_configured_maximum() {
    let generator: Minter = generator_for("br");
    assert_eq!(generator.hash_length_for(1_000_000).get(), 8);
    assert_eq!(generator.hash_length_for(100_000_000).get(), 8);
}
