//! Every vector in `spec/vectors.json`, run against this implementation.
//!
//! The file drives the tests, not a list in this file: the run iterates the
//! groups the suite actually declares, so adding a group to `vectors.json`
//! either runs or fails loudly — it cannot silently go unchecked, which a
//! hand-maintained list of group names would eventually allow.

use hashid_conformance::runner::{check_group, render, Failure};
use hashid_conformance::{suite, Suite};

#[test]
fn the_suite_declares_the_specification_it_belongs_to() {
    let suite: &Suite = suite();
    assert_eq!(suite.spec_version, "1.0.0");
    assert!(suite.vectors_revision >= 1);
    assert!(!suite.groups.is_empty(), "the suite has no groups");
}

#[test]
fn every_vector_holds() {
    let mut report: Vec<String> = Vec::new();
    let mut checked: usize = 0;

    for (group, vectors) in &suite().groups {
        checked += vectors.len();
        let failures: Vec<Failure> = check_group(group);
        if !failures.is_empty() {
            report.push(format!(
                "{group} ({} of {} failed):\n{}",
                failures.len(),
                vectors.len(),
                render(&failures)
            ));
        }
    }

    assert!(checked > 100, "only {checked} vectors ran — the suite is not loading");
    assert!(
        report.is_empty(),
        "{} of {} vector groups failed:\n\n{}",
        report.len(),
        suite().groups.len(),
        report.join("\n\n")
    );
}
