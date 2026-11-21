//! `StoreError`, and the accessors the domain exposes.
//!
//! The Gherkin suite drives the crate through its use cases, which leaves the
//! small value-object accessors reachable but unasserted — mutation testing
//! found exactly that hole. These close it: every public method here is
//! exercised for zero, one, and many.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test code — a terse panic on a broken invariant beats Result plumbing"
)]

use std::error::Error as _;
use std::io;

use chrono::{DateTime, Utc};
use hashid::{
    Base36Hash, IdError, Prefix, Request, Slug, StoreError, MAX_HASH_LEN, MAX_ID_LEN,
    MAX_PREFIX_LEN,
};

// ---------------------------------------------------------------------------
// StoreError
// ---------------------------------------------------------------------------

#[test]
fn a_store_error_carries_its_message_and_no_source() {
    let err: StoreError = StoreError::new("store offline");

    assert_eq!(err.message(), "store offline");
    assert_eq!(err.to_string(), "id lookup failed: store offline");
    assert!(err.source().is_none());
}

#[test]
fn a_store_error_preserves_the_underlying_cause() {
    let cause: io::Error = io::Error::new(io::ErrorKind::BrokenPipe, "socket closed");
    let err: StoreError = StoreError::with_source("query failed", cause);

    assert_eq!(err.message(), "query failed");
    assert_eq!(err.to_string(), "id lookup failed: query failed");
    assert_eq!(
        err.source().map(ToString::to_string),
        Some("socket closed".to_string()),
        "the cause must survive, or a caller cannot diagnose the failure"
    );
}

// ---------------------------------------------------------------------------
// Value-object accessors
// ---------------------------------------------------------------------------

#[test]
fn a_prefix_exposes_its_text_and_length() {
    let prefix: Prefix = Prefix::parse("my-proj").unwrap();

    assert_eq!(prefix.as_str(), "my-proj");
    assert_eq!(prefix.as_ref() as &str, "my-proj");
    assert_eq!(prefix.to_string(), "my-proj");
    assert_eq!(prefix.len(), 7);
    assert!(
        !prefix.is_empty(),
        "a Prefix cannot be empty by construction"
    );
}

#[test]
fn a_slug_exposes_its_text_and_length() {
    let slug: Slug = Slug::normalize("Survey The Users").unwrap();

    assert_eq!(slug.as_str(), "survey-the-users");
    assert_eq!(slug.as_ref() as &str, "survey-the-users");
    assert_eq!(slug.to_string(), "survey-the-users");
    assert_eq!(slug.len(), 16);
    assert!(!slug.is_empty(), "a Slug cannot be empty by construction");
}

#[test]
fn a_hash_exposes_its_text_and_length() {
    let hash: Base36Hash = Base36Hash::parse("8cda").unwrap();

    assert_eq!(hash.as_str(), "8cda");
    assert_eq!(hash.as_ref() as &str, "8cda");
    assert_eq!(hash.to_string(), "8cda");
    assert_eq!(hash.len(), 4);
    assert!(
        !hash.is_empty(),
        "a Base36Hash cannot be empty by construction"
    );
}

#[test]
fn a_truncated_slug_drops_the_hyphen_the_cut_exposes() {
    let slug: Slug = Slug::normalize("survey-the-users").unwrap();

    // Cutting at 7 lands on "survey-", whose trailing hyphen must go.
    assert_eq!(slug.truncated_to(7).unwrap().as_str(), "survey");
    assert_eq!(slug.truncated_to(6).unwrap().as_str(), "survey");
    assert_eq!(slug.truncated_to(99).unwrap().as_str(), "survey-the-users");
    assert!(
        slug.truncated_to(0).is_none(),
        "no room at all means no slug, not an empty one"
    );
}

#[test]
fn a_prefix_joined_with_a_slug_stays_within_the_prefix_budget() {
    let prefix: Prefix = Prefix::parse("proj").unwrap();
    let slug: Slug = Slug::normalize("survey-the-users").unwrap();

    let joined: Prefix = prefix.joined_with(&slug).unwrap();

    assert_eq!(joined.as_str(), "proj-survey-the-users");
    assert!(joined.len() <= MAX_PREFIX_LEN);
}

#[test]
fn a_prefix_that_fills_the_budget_leaves_no_room_for_a_slug() {
    let full: Prefix = Prefix::parse(&"p".repeat(MAX_PREFIX_LEN)).unwrap();
    let slug: Slug = Slug::normalize("slug").unwrap();

    assert!(
        full.joined_with(&slug).is_none(),
        "an oversized prefix must be refused, not emitted"
    );
}

#[test]
fn a_prefix_one_short_of_the_budget_leaves_room_for_exactly_nothing() {
    // MAX - 1 characters of prefix, plus the joining '-', is the whole budget.
    // There is no room left for even a one-character slug.
    let nearly_full: Prefix = Prefix::parse(&"p".repeat(MAX_PREFIX_LEN - 1)).unwrap();
    let slug: Slug = Slug::normalize("slug").unwrap();

    assert!(nearly_full.joined_with(&slug).is_none());
}

#[test]
fn a_prefix_two_short_of_the_budget_leaves_room_for_a_single_character() {
    let nearly_full: Prefix = Prefix::parse(&"p".repeat(MAX_PREFIX_LEN - 2)).unwrap();
    let slug: Slug = Slug::normalize("slug").unwrap();

    let joined: Prefix = nearly_full
        .joined_with(&slug)
        .expect("one character of slug still fits");

    assert_eq!(joined.len(), MAX_PREFIX_LEN);
    assert!(joined.as_str().ends_with("-s"));
}

// ---------------------------------------------------------------------------
// Limits
// ---------------------------------------------------------------------------

#[test]
fn the_id_length_ceiling_is_the_sum_of_its_parts() {
    // prefix + '-' + hash.
    assert_eq!(MAX_ID_LEN, MAX_PREFIX_LEN + 1 + MAX_HASH_LEN);
    assert_eq!(MAX_ID_LEN, 105);
}

#[test]
fn from_nanos_pins_the_instant_it_is_given() {
    assert_eq!(Request::from_nanos("Ship it", 42).created_at_nanos(), 42);
}

#[test]
fn from_nanos_accepts_the_extremes_of_i64() {
    assert_eq!(Request::from_nanos("Ship it", i64::MIN).created_at_nanos(), i64::MIN);
    assert_eq!(Request::from_nanos("Ship it", i64::MAX).created_at_nanos(), i64::MAX);
}

// ---------------------------------------------------------------------------
// Numbers that cannot be represented are refused, never coerced
// ---------------------------------------------------------------------------

#[test]
fn an_instant_outside_the_nanosecond_range_is_refused() {
    // 2263 is past the last instant `i64` nanoseconds can express.
    let too_late: DateTime<Utc> = "2263-01-01T00:00:00Z".parse().unwrap();

    let refused = Request::new("Ship it").at(too_late);

    assert!(
        matches!(refused, Err(IdError::TimestampOutOfRange { .. })),
        "expected a refusal, got {refused:?}"
    );
}

#[test]
fn an_instant_before_the_nanosecond_range_is_refused() {
    let too_early: DateTime<Utc> = "1600-01-01T00:00:00Z".parse().unwrap();

    assert!(matches!(
        Request::new("Ship it").at(too_early),
        Err(IdError::TimestampOutOfRange { .. })
    ));
}

#[test]
fn a_refused_instant_does_not_reach_the_hash() {
    // The point of refusing: two different out-of-range instants must not
    // collapse onto one seed. If `at` coerced to zero they would mint the
    // same ID, silently, for every record dated outside the range.
    let a: DateTime<Utc> = "2263-01-01T00:00:00Z".parse().unwrap();
    let b: DateTime<Utc> = "2400-01-01T00:00:00Z".parse().unwrap();

    assert!(Request::new("Ship it").at(a).is_err());
    assert!(Request::new("Ship it").at(b).is_err());
}

#[test]
fn nanoseconds_cross_verbatim_and_need_no_validation() {
    // Every i64 is a representable instant, so the boundary form is total.
    let request: Request<'_> = Request::new("Ship it").at_nanos(i64::MIN);
    assert_eq!(request.created_at_nanos(), i64::MIN);

    let request: Request<'_> = Request::new("Ship it").at_nanos(i64::MAX);
    assert_eq!(request.created_at_nanos(), i64::MAX);
}
