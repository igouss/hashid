//! The fragment rule, applied to a slice.
//!
//! `find_matching` is the rule this crate keeps rather than pushing onto your
//! store. These pin it for zero, one and many — and for the child-path case,
//! which is the edge an adapter would have got wrong.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test code — a terse panic on a broken invariant beats Result plumbing"
)]

use hashid::Id;
use hashid_resolve::find_matching;

fn id(raw: &str) -> Id {
    Id::parse(raw).expect("test id is well formed")
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

