//! Runs [`spec/vectors.json`](../../../spec/vectors.json) against this
//! implementation.
//!
//! # Which way the trust runs
//!
//! `vectors.json` is **hand-authored and checked in**. It is not emitted from
//! the implementation, and there is no regenerate step — deliberately. A golden
//! file the code writes for itself can never fail, and a suite that can never
//! fail is not a gate, it is decoration.
//!
//! So the vectors are the specification's machine-checkable half, and the
//! implementation is what is under test. When the two disagree, that is a
//! finding either way: the code is wrong, or the rule was.
//!
//! The one place this weakens is the cryptographic groups. Nobody can derive a
//! SHA-256 output by reading a rule, so `digest` and `mint_unchecked`
//! expectations were taken from a run and are frozen thereafter. They pin the
//! wire format against drift, which is what they are for; they cannot prove it
//! was right on the day it was chosen. Everything else is derivable from
//! `hashid-spec.md` and was written from it.
//!
//! # One shape, not twenty-two
//!
//! Every vector is `{ id, rules, note, …inputs…, expect | error }`, and every
//! group is that shape with different input keys. So the vectors are read as
//! plain JSON and dispatched by group name in [`runner::evaluate`], rather than
//! through a struct and a check function per group. That is the same shape a
//! reimplementation in any language sees, which is the point: a table test
//! should not need twenty-two tables.
//!
//! ```text
//! cargo test -p hashid-conformance
//! ```

pub mod runner;

use std::collections::{BTreeMap, BTreeSet};
use std::sync::LazyLock;

use serde::Deserialize;
use serde_json::{Map, Value};

pub use runner::{evaluate, Failure};

/// The checked-in vector file, compiled in so the tests cannot drift from it.
const VECTORS: &str = include_str!("../../../spec/vectors.json");

/// One vector: identifiers, inputs, and an expectation, as written.
pub type Vector = Map<String, Value>;

/// The whole vector file.
#[derive(Debug, Deserialize)]
pub struct Suite {
    /// The specification version these vectors belong to.
    pub spec_version: String,
    /// Bumped whenever a vector is added, changed, or retired.
    pub vectors_revision: u32,
    /// Rules no vector can pin, each with the reason it cannot.
    ///
    /// Data rather than prose so that the coverage gate and a human reader
    /// consult the same list, and neither can drift from the other.
    pub uncovered_rules: BTreeMap<String, String>,
    /// Every group, keyed by capability.
    pub groups: BTreeMap<String, Vec<Vector>>,
}

/// Parsed once per test binary: the file is 64 KB and there are two dozen
/// tests, so parsing per call would be two dozen needless parses.
static SUITE: LazyLock<Suite> = LazyLock::new(|| {
    match serde_json::from_str::<Suite>(VECTORS) {
        Ok(suite) => suite,
        Err(err) => panic!("spec/vectors.json does not parse: {err}"),
    }
});

/// The checked-in vector suite.
#[must_use]
pub fn suite() -> &'static Suite {
    &SUITE
}

/// Every rule identifier any vector names.
static RULE_REFERENCES: LazyLock<BTreeSet<String>> = LazyLock::new(|| {
    suite()
        .groups
        .values()
        .flatten()
        .filter_map(|vector: &Vector| vector.get("rules")?.as_array())
        .flatten()
        .filter_map(|rule: &Value| Some(rule.as_str()?.to_owned()))
        .collect()
});

/// Every rule identifier any vector names.
#[must_use]
pub fn rule_references() -> &'static BTreeSet<String> {
    &RULE_REFERENCES
}
