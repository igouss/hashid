//! The executable specification for resolution.
//!
//! `features/resolution.feature` is the source of truth; these steps are the
//! only thing standing between it and the crate.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code — a terse panic on a broken invariant beats Result plumbing"
)]
#![allow(
    clippy::needless_pass_by_ref_mut,
    clippy::needless_pass_by_value,
    reason = "cucumber mandates the step signature: `&mut World` plus owned \
              captures deserialized from the Gherkin"
)]

use std::collections::HashSet;

use cucumber::{given, then, when, World};

use hashid::{Id, StoreError};
use hashid_resolve::{MatchType, ResolveError, ResolvedId, Resolver, ResolverConfig};

/// The store under a scenario.
///
/// `Broken` is the shape a `HashSet` cannot express, and it is the one worth
/// testing: a store that cannot answer must abort resolution rather than
/// report a spurious "not found".
#[derive(Debug, Default)]
struct Store {
    ids: HashSet<Id>,
    broken: bool,
}

impl Store {
    fn exists(&self, id: &Id) -> Result<bool, StoreError> {
        if self.broken {
            return Err(StoreError::new("store offline"));
        }
        Ok(self.ids.contains(id))
    }

    /// Deliberately returns *everything*, applying no filter at all.
    ///
    /// That is legal — the contract is best-effort narrowing — and it is what
    /// the real `SQLite` store this crate was extracted from does. Answering the
    /// widest possible way here is what proves the crate applies the fragment
    /// rule itself rather than trusting the caller to have applied it.
    fn candidates(&self, _fragment: &str) -> Result<Vec<Id>, StoreError> {
        if self.broken {
            return Err(StoreError::new("store offline"));
        }
        Ok(self.ids.iter().cloned().collect())
    }
}

/// One scenario's state.
#[derive(Debug, World)]
#[world(init = Self::new)]
struct ResolveWorld {
    resolver: Resolver,
    store: Store,
    resolved: Option<ResolvedId>,
    batch: Option<Vec<ResolvedId>>,
    error: Option<ResolveError>,
}

impl ResolveWorld {
    fn new() -> Self {
        Self {
            resolver: Resolver::new("id").unwrap(),
            store: Store::default(),
            resolved: None,
            batch: None,
            error: None,
        }
    }

    fn error(&self) -> &ResolveError {
        self.error.as_ref().expect("scenario expected a failure")
    }

    fn resolve(&mut self, input: &str) {
        let outcome = self.resolver.resolve(
            input,
            |id: &Id| self.store.exists(id),
            |fragment: &str| self.store.candidates(fragment),
        );
        match outcome {
            Ok(resolved) => {
                self.resolved = Some(resolved);
                self.error = None;
            }
            Err(err) => {
                self.resolved = None;
                self.error = Some(err);
            }
        }
    }
}

// --- Given -----------------------------------------------------------------

#[given(expr = "a resolver with default prefix {string}")]
fn resolver_with_prefix(world: &mut ResolveWorld, prefix: String) {
    world.resolver = Resolver::new(&prefix).expect("scenario prefixes are usable");
}

#[given(expr = "substring matching is disabled")]
fn substring_disabled(world: &mut ResolveWorld) {
    let config: ResolverConfig = world
        .resolver
        .config()
        .clone()
        .with_substring_match(false);
    world.resolver = Resolver::with_config(config);
}

#[given(expr = "the registry holds {string}")]
fn registry_holds(world: &mut ResolveWorld, id: String) {
    world.store.ids.insert(Id::parse(&id).expect("scenario ids are well formed"));
}

#[given("a registry whose lookups fail")]
fn registry_broken(world: &mut ResolveWorld) {
    world.store.broken = true;
}

// --- When ------------------------------------------------------------------

#[when(expr = "I resolve {string}")]
fn resolve_one(world: &mut ResolveWorld, input: String) {
    world.resolve(&input);
}

#[when(expr = "I resolve {string} and {string}")]
fn resolve_two(world: &mut ResolveWorld, first: String, second: String) {
    let inputs: Vec<String> = vec![first, second];
    let outcome = world.resolver.resolve_all(
        &inputs,
        |id: &Id| world.store.exists(id),
        |fragment: &str| world.store.candidates(fragment),
    );
    match outcome {
        Ok(batch) => {
            world.batch = Some(batch);
            world.error = None;
        }
        Err(err) => {
            world.batch = None;
            world.error = Some(err);
        }
    }
}

// --- Then ------------------------------------------------------------------

#[then("resolution succeeds")]
fn resolution_succeeds(world: &mut ResolveWorld) {
    assert!(
        world.resolved.is_some(),
        "expected a resolved id, got {:?}",
        world.error
    );
}

#[then(expr = "the resolved id is {string}")]
fn resolved_id_is(world: &mut ResolveWorld, expected: String) {
    assert_eq!(
        world.resolved.as_ref().expect("nothing resolved").id.to_string(),
        expected
    );
}

#[then("the match was exact")]
fn match_was_exact(world: &mut ResolveWorld) {
    assert_eq!(world.match_type(), MatchType::Exact);
}

#[then("the match was prefix-normalized")]
fn match_was_prefix_normalized(world: &mut ResolveWorld) {
    assert_eq!(world.match_type(), MatchType::PrefixNormalized);
}

#[then("the match was a substring")]
fn match_was_substring(world: &mut ResolveWorld) {
    assert_eq!(world.match_type(), MatchType::Substring);
}

#[then("resolution fails as ambiguous")]
fn fails_ambiguous(world: &mut ResolveWorld) {
    assert!(
        matches!(world.error(), ResolveError::Ambiguous { .. }),
        "expected ambiguity, got {:?}",
        world.error()
    );
    assert!(world.resolved.is_none(), "an ambiguous fragment resolved");
}

#[then(expr = "the ambiguity lists {string}")]
fn ambiguity_lists(world: &mut ResolveWorld, expected: String) {
    let ResolveError::Ambiguous { matches, .. } = world.error() else {
        panic!("expected ambiguity, got {:?}", world.error());
    };
    assert!(matches.contains(&expected), "{matches:?} omits {expected}");
}

#[then("resolution fails as not found")]
fn fails_not_found(world: &mut ResolveWorld) {
    assert!(
        matches!(world.error(), ResolveError::NotFound { .. }),
        "expected not found, got {:?}",
        world.error()
    );
    assert!(world.resolved.is_none(), "a missing fragment resolved");
}

#[then("resolution fails as malformed")]
fn fails_malformed(world: &mut ResolveWorld) {
    assert!(
        matches!(world.error(), ResolveError::Malformed(_)),
        "expected malformed, got {:?}",
        world.error()
    );
}

#[then("resolution fails because the registry could not answer")]
fn fails_store(world: &mut ResolveWorld) {
    assert!(
        matches!(world.error(), ResolveError::Store(_)),
        "expected a store failure, got {:?}",
        world.error()
    );
    assert!(world.resolved.is_none(), "resolution guessed anyway");
}

#[then("both fragments resolve")]
fn both_resolve(world: &mut ResolveWorld) {
    assert_eq!(
        world.batch.as_ref().expect("no batch resolved").len(),
        2,
        "expected both fragments to resolve"
    );
}

#[then(expr = "the batch resolved {string}")]
fn batch_resolved(world: &mut ResolveWorld, expected: String) {
    assert!(
        world
            .batch
            .as_ref()
            .expect("no batch resolved")
            .iter()
            .any(|resolved: &ResolvedId| resolved.id.to_string() == expected),
        "batch omits {expected}"
    );
}

impl ResolveWorld {
    fn match_type(&self) -> MatchType {
        self.resolved
            .as_ref()
            .expect("nothing resolved")
            .match_type
    }
}

#[tokio::test]
async fn resolution_specification() {
    ResolveWorld::cucumber()
        .fail_on_skipped()
        .run_and_exit("features")
        .await;
}
