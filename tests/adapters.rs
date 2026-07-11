//! The boundary layer, and the accessors the domain exposes.
//!
//! The Gherkin suite drives the crate through its use cases, which leaves the
//! adapters and the small value-object accessors reachable but unasserted —
//! mutation testing found exactly that hole. These close it: every public
//! method here is exercised for zero, one, and many.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test code — a terse panic on a broken invariant beats Result plumbing"
)]

use std::error::Error as _;
use std::io;

use hashid::resolve::find_matching;
use hashid::{
    Base36Hash, FnIndex, FnRegistry, Id, IdIndex, IdRegistry, InMemoryIds, Prefix, RegistryError,
    Slug, MAX_HASH_LEN, MAX_ID_LEN, MAX_PREFIX_LEN,
};

fn id(raw: &str) -> Id {
    Id::parse(raw).expect("test id is well formed")
}

// ---------------------------------------------------------------------------
// InMemoryIds
// ---------------------------------------------------------------------------

#[test]
fn an_empty_registry_holds_nothing() {
    let known: InMemoryIds = InMemoryIds::new();

    assert_eq!(known.len(), 0);
    assert!(known.is_empty());
    assert!(!known.contains(&id("br-abc123")).unwrap());
    assert_eq!(known.iter().count(), 0);
}

#[test]
fn a_registry_with_one_id_holds_exactly_that_id() {
    let mut known: InMemoryIds = InMemoryIds::new();

    assert!(
        known.insert(id("br-abc123")),
        "a fresh id is newly inserted"
    );

    assert_eq!(known.len(), 1);
    assert!(!known.is_empty());
    assert!(known.contains(&id("br-abc123")).unwrap());
    assert!(!known.contains(&id("br-xyz789")).unwrap());
    assert_eq!(known.iter().count(), 1);
}

#[test]
fn inserting_the_same_id_twice_is_reported_and_does_not_grow_the_registry() {
    let mut known: InMemoryIds = InMemoryIds::new();

    assert!(known.insert(id("br-abc123")));
    assert!(
        !known.insert(id("br-abc123")),
        "a repeat insert reports false"
    );

    assert_eq!(known.len(), 1);
}

#[test]
fn removing_a_held_id_reports_true_and_shrinks_the_registry() {
    let mut known: InMemoryIds = InMemoryIds::new();
    known.insert(id("br-abc123"));

    assert!(
        known.remove(&id("br-abc123")),
        "removing a held id reports true"
    );

    assert!(known.is_empty());
    assert!(!known.contains(&id("br-abc123")).unwrap());
}

#[test]
fn removing_an_absent_id_reports_false_and_changes_nothing() {
    let mut known: InMemoryIds = InMemoryIds::new();
    known.insert(id("br-abc123"));

    assert!(
        !known.remove(&id("br-xyz789")),
        "removing an absent id reports false"
    );

    assert_eq!(known.len(), 1);
}

#[test]
fn a_registry_can_be_collected_from_ids() {
    let known: InMemoryIds = ["br-abc123", "br-abd456", "br-xyz789"]
        .into_iter()
        .map(id)
        .collect();

    assert_eq!(known.len(), 3);
    assert_eq!((&known).into_iter().count(), 3);
}

#[test]
fn a_registry_can_be_extended() {
    let mut known: InMemoryIds = InMemoryIds::new();
    known.insert(id("br-abc123"));

    known.extend([id("br-abd456"), id("br-xyz789")]);

    assert_eq!(known.len(), 3);
    assert!(known.contains(&id("br-abd456")).unwrap());
}

#[test]
fn a_registry_finds_ids_by_hash_fragment() {
    let known: InMemoryIds = ["br-abc123", "br-abd456", "br-xyz789"]
        .into_iter()
        .map(id)
        .collect();

    assert_eq!(known.find_by_hash_fragment("zzz").unwrap().len(), 0);
    assert_eq!(known.find_by_hash_fragment("xyz").unwrap().len(), 1);
    assert_eq!(known.find_by_hash_fragment("ab").unwrap().len(), 2);
}

// ---------------------------------------------------------------------------
// The `&T` forwarding impls — a borrowed registry is still a registry.
// ---------------------------------------------------------------------------

#[test]
fn a_borrowed_registry_forwards_both_port_methods() {
    let known: InMemoryIds = ["br-abc123"].into_iter().map(id).collect();
    let borrowed: &InMemoryIds = &known;

    assert!(IdRegistry::contains(&borrowed, &id("br-abc123")).unwrap());
    assert!(!IdRegistry::contains(&borrowed, &id("br-xyz789")).unwrap());
    assert_eq!(
        IdIndex::find_by_hash_fragment(&borrowed, "abc")
            .unwrap()
            .len(),
        1
    );
}

// ---------------------------------------------------------------------------
// Closure adapters
// ---------------------------------------------------------------------------

#[test]
fn a_closure_registry_answers_both_ways() {
    let registry: FnRegistry<_> =
        FnRegistry::new(|candidate: &Id| Ok(candidate.to_string() == "br-abc123"));

    assert!(registry.contains(&id("br-abc123")).unwrap());
    assert!(!registry.contains(&id("br-xyz789")).unwrap());
}

#[test]
fn a_closure_registry_can_report_failure() {
    let registry: FnRegistry<_> =
        FnRegistry::new(|_: &Id| Err(RegistryError::new("store offline")));

    assert!(registry.contains(&id("br-abc123")).is_err());
}

#[test]
fn a_closure_index_answers_both_of_its_questions() {
    let known: Vec<Id> = ["br-abc123", "br-xyz789"].into_iter().map(id).collect();

    let index: FnIndex<_, _> = FnIndex::new(
        |candidate: &Id| Ok(known.contains(candidate)),
        |fragment: &str| Ok(find_matching(&known, fragment)),
    );

    assert!(index.contains(&id("br-abc123")).unwrap());
    assert!(!index.contains(&id("br-aaa000")).unwrap());
    assert_eq!(index.find_by_hash_fragment("xyz").unwrap().len(), 1);
    assert_eq!(index.find_by_hash_fragment("zzz").unwrap().len(), 0);
}

// ---------------------------------------------------------------------------
// find_matching — the fragment rule, applied to a slice.
// ---------------------------------------------------------------------------

#[test]
fn find_matching_returns_nothing_when_no_hash_contains_the_fragment() {
    let known: Vec<Id> = ["br-abc123", "br-xyz789"].into_iter().map(id).collect();

    assert!(find_matching(&known, "zzz").is_empty());
}

#[test]
fn find_matching_returns_the_single_hash_containing_the_fragment() {
    let known: Vec<Id> = ["br-abc123", "br-xyz789"].into_iter().map(id).collect();

    let found: Vec<Id> = find_matching(&known, "xyz");

    assert_eq!(found, vec![id("br-xyz789")]);
}

#[test]
fn find_matching_returns_every_hash_containing_the_fragment() {
    let known: Vec<Id> = ["br-abc123", "br-abd456", "br-xyz789"]
        .into_iter()
        .map(id)
        .collect();

    let found: Vec<Id> = find_matching(&known, "ab");

    assert_eq!(found, vec![id("br-abc123"), id("br-abd456")]);
}

#[test]
fn find_matching_treats_a_child_path_in_the_fragment_as_exact() {
    let known: Vec<Id> = ["br-64up6", "br-64up6.4", "br-64up6.5"]
        .into_iter()
        .map(id)
        .collect();

    // The hash matches loosely, but the child path must match exactly: the
    // parent and the sibling are both excluded.
    assert_eq!(find_matching(&known, "4up6.4"), vec![id("br-64up6.4")]);
    assert!(find_matching(&known, "4up6.9").is_empty());

    // With no child path, the parent and both children all match.
    assert_eq!(find_matching(&known, "4up6").len(), 3);
}

// ---------------------------------------------------------------------------
// RegistryError
// ---------------------------------------------------------------------------

#[test]
fn a_registry_error_carries_its_message_and_no_source() {
    let err: RegistryError = RegistryError::new("store offline");

    assert_eq!(err.message(), "store offline");
    assert_eq!(err.to_string(), "id registry lookup failed: store offline");
    assert!(err.source().is_none());
}

#[test]
fn a_registry_error_preserves_the_underlying_cause() {
    let cause: io::Error = io::Error::new(io::ErrorKind::BrokenPipe, "socket closed");
    let err: RegistryError = RegistryError::with_source("query failed", cause);

    assert_eq!(err.message(), "query failed");
    assert_eq!(err.to_string(), "id registry lookup failed: query failed");
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
