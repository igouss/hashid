//! Every rule in the specification is either pinned by a vector or listed as
//! unpinnable, with a reason.
//!
//! This gate exists because the claim it checks was made in prose first, and
//! was wrong: twelve rules had no vector. Prose cannot notice that. The
//! exemption list lives in `vectors.json` as data rather than in this file as a
//! constant, so the list a reader sees and the list the build enforces are the
//! same one.

#![allow(
    clippy::expect_used,
    reason = "test code — a terse panic on a broken invariant beats Result plumbing"
)]

use std::collections::BTreeSet;

use hashid_conformance::{rule_references, suite, Suite};

/// The specification, compiled in so this test cannot drift from it.
const SPEC: &str = include_str!("../../../spec/hashid-spec.md");

/// Every rule the specification declares.
///
/// A declaration is a list item opening in bold with the rule's identifier:
/// `- **GR-1** ...`, or `- **RS-3 (Exact).** ...`.
fn declared_rules() -> BTreeSet<String> {
    SPEC.lines()
        .filter_map(|line: &str| line.trim_start().strip_prefix("- **"))
        .filter_map(|rest: &str| {
            let identifier: String = rest
                .chars()
                .take_while(|c: &char| c.is_ascii_uppercase() || c.is_ascii_digit() || *c == '-')
                .collect();
            is_rule_identifier(&identifier).then_some(identifier)
        })
        .collect()
}

/// Does `text` look like `XX-9`: letters, a hyphen, then digits?
fn is_rule_identifier(text: &str) -> bool {
    let Some((head, tail)) = text.split_once('-') else {
        return false;
    };
    !head.is_empty()
        && head.chars().all(|c: char| c.is_ascii_uppercase() || c.is_ascii_digit())
        && !tail.is_empty()
        && tail.chars().all(|c: char| c.is_ascii_digit())
}

#[test]
fn the_specification_declares_rules_this_test_can_find() {
    // If the parse silently found nothing, every assertion below would pass
    // vacuously. Check the check first.
    let declared: BTreeSet<String> = declared_rules();
    assert!(
        declared.len() > 50,
        "only {} rules parsed out of the specification — the parse is broken, \
         not the specification",
        declared.len()
    );
    for expected in ["GR-1", "SD-2", "DG-6", "RS-3", "PF-5", "ES-7"] {
        assert!(
            declared.contains(expected),
            "rule `{expected}` was not found by the parse"
        );
    }
}

#[test]
fn every_vector_names_a_rule_the_specification_declares() {
    let dangling: Vec<String> = rule_references()
        .difference(&declared_rules())
        .cloned()
        .collect();
    assert!(
        dangling.is_empty(),
        "vectors name rules the specification does not declare: {dangling:?}"
    );
}

#[test]
fn every_rule_is_pinned_by_a_vector_or_documented_as_unpinnable() {
    let suite: &Suite = suite();
    let referenced: &BTreeSet<String> = rule_references();
    let exempt: BTreeSet<String> = suite.uncovered_rules.keys().cloned().collect();

    let unaccounted: Vec<String> = declared_rules()
        .difference(referenced)
        .filter(|rule: &&String| !exempt.contains(*rule))
        .cloned()
        .collect();
    assert!(
        unaccounted.is_empty(),
        "these rules have no vector and are not listed in `uncovered_rules`: \
         {unaccounted:?}\nAdd a vector, or add the rule with the reason it \
         cannot have one."
    );
}

#[test]
fn nothing_is_exempted_that_a_vector_already_pins() {
    let suite: &Suite = suite();
    let referenced: &BTreeSet<String> = rule_references();
    let redundant: Vec<&String> = suite
        .uncovered_rules
        .keys()
        .filter(|rule: &&String| referenced.contains(*rule))
        .collect();
    assert!(
        redundant.is_empty(),
        "these rules are listed as unpinnable but a vector pins them: {redundant:?}"
    );
}

#[test]
fn every_exemption_carries_a_reason() {
    let suite: &Suite = suite();
    for (rule, reason) in &suite.uncovered_rules {
        assert!(
            reason.len() > 20,
            "exemption for `{rule}` needs a reason, not `{reason}`"
        );
    }
}
