//! Executable specs. `cucumber` parses `features/*.feature` and runs the step
//! definitions below, so the Gherkin **is** the behavioural test source.
//!
//! `fail_on_skipped()` is not decoration: without it a step whose definition
//! is missing or misspelled is silently *skipped*, and a feature file full of
//! skipped steps reports green. That is the exact false-green this suite exists
//! to prevent.

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

use chrono::{DateTime, Utc};
use cucumber::{given, then, when, World};

use std::collections::HashSet;

use hashid::generate::{digest, seed};
use hashid::{
    Base36Hash, HashLength, Id, IdConfig, IdError, MalformedReason, Minter, Prefix, Request,
    Slug, StoreError, MAX_PREFIX_LEN,
};

/// The instant every scenario runs at unless it says otherwise.
const DEFAULT_INSTANT: &str = "2026-07-10T12:34:56.789012345Z";

/// A hash length a scenario named explicitly.
fn hash_length(value: usize) -> HashLength {
    HashLength::new(value).expect("scenario hash lengths are within the grammar")
}

// ---------------------------------------------------------------------------
// A registry that can also be saturated, or broken.
// ---------------------------------------------------------------------------

/// The store under a scenario.
///
/// `Saturated` and `Broken` are the two shapes a `HashSet` cannot express, and
/// they are precisely the ones worth testing: a registry where every id is
/// taken must produce an error rather than a duplicate, and a registry that
/// cannot answer must abort rather than guess.
#[derive(Debug)]
enum Store {
    /// A real, finite set of known ids.
    Memory(HashSet<Id>),
    /// Every id exists. Nothing can ever be minted.
    Saturated,
    /// Every id whose hash is 12 characters or fewer exists.
    ///
    /// This is the only shape that forces the generator all the way down to its
    /// last resort — splicing the nonce into the hash — while still leaving it
    /// somewhere to land. A finite `HashSet` cannot express it.
    EveryShortHash,
    /// Every lookup fails.
    Broken,
}

impl Default for Store {
    fn default() -> Self {
        Self::Memory(HashSet::new())
    }
}

impl Store {
    fn memory_mut(&mut self) -> &mut HashSet<Id> {
        match self {
            Self::Memory(ids) => ids,
            other => panic!("scenario needs an in-memory store, but it is {other:?}"),
        }
    }

    fn snapshot(&self) -> Vec<Id> {
        match self {
            Self::Memory(ids) => ids.iter().cloned().collect(),
            _ => Vec::new(),
        }
    }
}

impl Store {
    /// The one question minting asks, as the closure `Minter::mint` takes.
    fn taken(&self, id: &Id) -> Result<bool, StoreError> {
        match self {
            Self::Memory(ids) => Ok(ids.contains(id)),
            Self::Saturated => Ok(true),
            Self::EveryShortHash => Ok(id.hash().len() <= 12),
            Self::Broken => Err(StoreError::new("store offline")),
        }
    }
}

// ---------------------------------------------------------------------------
// World
// ---------------------------------------------------------------------------

/// One scenario's state.
#[derive(Debug, World)]
#[world(init = Self::new)]
struct IdWorld {
    instant: DateTime<Utc>,
    population: usize,
    generator: Minter,
    store: Store,

    /// The store's contents immediately before the most recent mint.
    before_mint: Vec<Id>,

    minted: Option<Id>,
    second: Option<Id>,
    parsed: Option<Id>,
    prefix: Option<Prefix>,
    prefix_ok: Option<bool>,
    valid: Option<bool>,
    config: Option<IdConfig>,
    error: Option<IdError>,
}

impl IdWorld {
    fn new() -> Self {
        let prefix: Prefix = Prefix::parse("id").unwrap();
        Self {
            instant: DEFAULT_INSTANT.parse().unwrap(),
            population: 0,
            generator: Minter::with_config(IdConfig::new(prefix)),
            store: Store::default(),
            before_mint: Vec::new(),
            minted: None,
            second: None,
            parsed: None,
            prefix: None,
            prefix_ok: None,
            valid: None,
            config: None,
            error: None,
        }
    }

    fn request<'a>(&self, title: &'a str) -> Request<'a> {
        Request::new(title)
            .at(self.instant)
            .expect("the scenario instant is representable")
            .with_population(self.population)
    }

    /// The id most recently produced, whether by minting or by parsing.
    fn subject(&self) -> &Id {
        self.minted
            .as_ref()
            .or(self.parsed.as_ref())
            .expect("scenario has produced no id")
    }

    fn error(&self) -> &IdError {
        self.error.as_ref().expect("scenario expected a failure")
    }

    fn record(&mut self, outcome: Result<Id, IdError>) {
        match outcome {
            Ok(id) => self.minted = Some(id),
            Err(err) => self.error = Some(err),
        }
    }
}

// ---------------------------------------------------------------------------
// Given: the world
// ---------------------------------------------------------------------------

#[given(expr = "the clock reads {string}")]
fn clock_reads(world: &mut IdWorld, instant: String) {
    world.instant = instant.parse().expect("scenario instant parses");
}

#[given(expr = "a generator with prefix {string}")]
fn generator_with_prefix(world: &mut IdWorld, prefix: String) {
    let prefix: Prefix = Prefix::parse(&prefix).expect("scenario prefix is legal");
    world.generator = Minter::with_config(IdConfig::new(prefix));
}

#[given(expr = "a generator with prefix {string} and hash lengths {int} to {int}")]
fn generator_with_hash_lengths(world: &mut IdWorld, prefix: String, min: usize, max: usize) {
    let prefix: Prefix = Prefix::parse(&prefix).expect("scenario prefix is legal");
    let config: IdConfig = IdConfig::new(prefix)
        .with_hash_lengths(min, max)
        .expect("scenario hash bounds are legal");
    world.generator = Minter::with_config(config);
}

#[given(expr = "a generator whose prefix already fills the prefix budget")]
fn generator_with_maximal_prefix(world: &mut IdWorld) {
    let prefix: Prefix =
        Prefix::parse(&"p".repeat(MAX_PREFIX_LEN)).expect("64 p's is a legal prefix");
    world.generator = Minter::with_config(IdConfig::new(prefix));
}

#[given(expr = "a population of {int}")]
fn population_of(world: &mut IdWorld, population: usize) {
    world.population = population;
}

#[given(expr = "the registry is empty")]
fn registry_is_empty(world: &mut IdWorld) {
    world.store = Store::Memory(HashSet::new());
}

#[given(expr = "the registry holds {string}")]
fn registry_holds(world: &mut IdWorld, raw: String) {
    let id: Id = Id::parse(&raw).expect("scenario id is well formed");
    world.store.memory_mut().insert(id);
}

#[given(expr = "a registry in which every id is already taken")]
fn registry_saturated(world: &mut IdWorld) {
    world.store = Store::Saturated;
}

#[given(expr = "a registry in which every hash of {int} characters or fewer is taken")]
fn registry_every_short_hash(world: &mut IdWorld, ceiling: usize) {
    assert_eq!(
        ceiling, 12,
        "the store models the 12-character fallback hash"
    );
    world.store = Store::EveryShortHash;
}

#[given(expr = "a registry whose lookups fail")]
fn registry_broken(world: &mut IdWorld) {
    world.store = Store::Broken;
}

#[given(expr = "the registry already holds the candidate for {string} at nonce {int}")]
fn registry_holds_candidate(world: &mut IdWorld, title: String, nonce: u32) {
    let length: HashLength = world.generator.hash_length_for(world.population);
    let taken: Id = world
        .generator
        .mint_unchecked(&world.request(&title), nonce, length);
    world.store.memory_mut().insert(taken);
}

#[given(expr = "the registry already holds every {int}-character candidate for {string}")]
fn registry_holds_every_candidate(world: &mut IdWorld, length: usize, title: String) {
    let length: HashLength = hash_length(length);
    // The generator tries nonces 0..10 at each length before widening.
    let taken: Vec<Id> = (0..10)
        .map(|nonce: u32| {
            world
                .generator
                .mint_unchecked(&world.request(&title), nonce, length)
        })
        .collect();
    world.store.memory_mut().extend(taken);
}

#[given(expr = "the registry already holds the {int}-character fallback candidate for {string}")]
fn registry_holds_fallback(world: &mut IdWorld, length: usize, title: String) {
    let length: HashLength = hash_length(length);
    let taken: Id = world
        .generator
        .mint_unchecked(&world.request(&title), 0, length);
    world.store.memory_mut().insert(taken);
}

#[given(
    expr = "the registry already holds every {int}-character candidate for {string} with slug {string}"
)]
fn registry_holds_every_slugged_candidate(
    world: &mut IdWorld,
    length: usize,
    title: String,
    slug: String,
) {
    let length: HashLength = hash_length(length);
    // Mirror what `mint_with_slug` would try: the same hashes, but under the
    // slugged prefix. A registry full of these forces the slug ladder to climb.
    let slugged: Prefix = world
        .generator
        .config()
        .prefix()
        .joined_with(&Slug::normalize(&slug).expect("scenario slug is usable"))
        .expect("scenario slug fits the prefix budget");

    let taken: Vec<Id> = (0..10)
        .map(|nonce: u32| {
            let hash: Base36Hash = digest(&seed(&world.request(&title), nonce), length);
            Id::parse(&format!("{slugged}-{hash}")).expect("a slugged candidate is a legal id")
        })
        .collect();
    world.store.memory_mut().extend(taken);
}

#[given(expr = "the id {string}")]
fn the_id(world: &mut IdWorld, raw: String) {
    world.parsed = Some(Id::parse(&raw).expect("scenario id is well formed"));
}

// ---------------------------------------------------------------------------
// When: minting
// ---------------------------------------------------------------------------

#[when(expr = "I mint an id for {string}")]
fn mint(world: &mut IdWorld, title: String) {
    world.before_mint = world.store.snapshot();
    let outcome = world.generator.mint(&world.request(&title), |id: &Id| world.store.taken(id));
    world.record(outcome);
}

#[when(expr = "I mint a second id for {string}")]
fn mint_second(world: &mut IdWorld, title: String) {
    world.second = world
        .generator
        .mint(&world.request(&title), |id: &Id| world.store.taken(id))
        .ok();
}

#[when(expr = "I mint a second id for {string} described as {string}")]
fn mint_second_described(world: &mut IdWorld, title: String, description: String) {
    let request: Request<'_> = world.request(&title).with_description(&description);
    world.second = world.generator.mint(&request, |id: &Id| world.store.taken(id)).ok();
}

#[when(expr = "I mint a second id for {string} created by {string}")]
fn mint_second_created_by(world: &mut IdWorld, title: String, creator: String) {
    let request: Request<'_> = world.request(&title).with_creator(&creator);
    world.second = world.generator.mint(&request, |id: &Id| world.store.taken(id)).ok();
}

#[when(expr = "I mint an id for {string} with slug {string}")]
fn mint_with_slug(world: &mut IdWorld, title: String, slug: String) {
    world.before_mint = world.store.snapshot();
    let outcome = world
        .generator
        .mint_with_slug(&world.request(&title), &slug, |id: &Id| world.store.taken(id));
    world.record(outcome);
}

#[when(expr = "I mint an id for {string} with a slug of {int} letters")]
fn mint_with_long_slug(world: &mut IdWorld, title: String, letters: usize) {
    let slug: String = "a".repeat(letters);
    world.before_mint = world.store.snapshot();
    let outcome = world
        .generator
        .mint_with_slug(&world.request(&title), &slug, |id: &Id| world.store.taken(id));
    world.record(outcome);
}

// ---------------------------------------------------------------------------
// When: parsing, prefixes, resolution
// ---------------------------------------------------------------------------

#[when(expr = "I parse {string}")]
fn parse(world: &mut IdWorld, raw: String) {
    match Id::parse(&raw) {
        Ok(id) => world.parsed = Some(id),
        Err(err) => world.error = Some(err),
    }
}

#[when(expr = "I normalize the prefix {string}")]
fn normalize_prefix(world: &mut IdWorld, raw: String) {
    match Prefix::normalize(&raw) {
        Ok(prefix) => world.prefix = Some(prefix),
        Err(err) => world.error = Some(err),
    }
}

#[when(expr = "I parse the prefix {string}")]
fn parse_prefix(world: &mut IdWorld, raw: String) {
    match Prefix::parse(&raw) {
        Ok(prefix) => world.prefix = Some(prefix),
        Err(err) => world.error = Some(err),
    }
}

#[when(expr = "I abbreviate the prefix {string}")]
fn abbreviate_prefix(world: &mut IdWorld, raw: String) {
    let prefix: Prefix = Prefix::normalize(&raw).expect("scenario prefix is usable");
    world.prefix = Some(prefix.abbreviated());
}

#[when(expr = "I require the prefix {string}")]
fn require_prefix(world: &mut IdWorld, expected: String) {
    let expected: Prefix = Prefix::parse(&expected).expect("scenario prefix is legal");
    world.prefix_ok = Some(hashid::validate_prefix(world.subject(), &expected, &[]).is_ok());
}

#[when(expr = "I require the prefix {string} while also allowing {string}")]
fn require_prefix_allowing(world: &mut IdWorld, expected: String, allowed: String) {
    let expected: Prefix = Prefix::parse(&expected).expect("scenario prefix is legal");
    let allowed: Prefix = Prefix::parse(&allowed).expect("scenario prefix is legal");
    world.prefix_ok = Some(hashid::validate_prefix(world.subject(), &expected, &[allowed]).is_ok());
}

#[when(expr = "I check whether {string} is a valid id")]
fn check_validity(world: &mut IdWorld, raw: String) {
    world.valid = Some(hashid::is_valid_id(&raw));
}

#[when(expr = "I configure hash lengths {int} to {int}")]
fn configure_hash_lengths(world: &mut IdWorld, min: usize, max: usize) {
    match IdConfig::default().with_hash_lengths(min, max) {
        Ok(config) => world.config = Some(config),
        Err(err) => world.error = Some(err),
    }
}

#[when(expr = "I configure a collision probability of {float}")]
fn configure_collision_prob(world: &mut IdWorld, value: f64) {
    match IdConfig::default().with_max_collision_prob(value) {
        Ok(config) => world.config = Some(config),
        Err(err) => world.error = Some(err),
    }
}

// ---------------------------------------------------------------------------
// Then: minting outcomes
// ---------------------------------------------------------------------------

#[then(expr = "minting succeeds")]
fn minting_succeeds(world: &mut IdWorld) {
    assert!(
        world.minted.is_some(),
        "expected a minted id, got {:?}",
        world.error
    );
}

#[then(expr = "minting fails because the id space is exhausted")]
fn minting_exhausted(world: &mut IdWorld) {
    assert!(
        matches!(world.error(), IdError::Exhausted { .. }),
        "expected Exhausted, got {:?}",
        world.error()
    );
    assert!(world.minted.is_none(), "an id was minted anyway");
}

#[then(expr = "minting fails because the registry could not answer")]
fn minting_registry_failed(world: &mut IdWorld) {
    assert!(
        matches!(world.error(), IdError::Store(_)),
        "expected Registry, got {:?}",
        world.error()
    );
    assert!(world.minted.is_none(), "an id was minted anyway");
}

#[then(expr = "the id is one the registry had not seen")]
fn id_is_fresh(world: &mut IdWorld) {
    let minted: &Id = world.subject();
    assert!(
        !world.before_mint.contains(minted),
        "generator returned an id the registry already held: {minted}"
    );
}

#[then(expr = "the two ids are identical")]
fn two_ids_identical(world: &mut IdWorld) {
    assert_eq!(world.minted, world.second);
}

#[then(expr = "the two ids differ")]
fn two_ids_differ(world: &mut IdWorld) {
    assert_ne!(world.minted, world.second);
}

// ---------------------------------------------------------------------------
// Then: id shape
// ---------------------------------------------------------------------------

#[then(expr = "the id has prefix {string}")]
fn id_has_prefix(world: &mut IdWorld, expected: String) {
    assert_eq!(world.subject().prefix().as_str(), expected);
}

#[then(expr = "the id has a hash of {int} characters")]
fn id_hash_length(world: &mut IdWorld, expected: usize) {
    assert_eq!(world.subject().hash().len(), expected);
}

#[then(expr = "the id has a non-empty hash")]
fn id_hash_non_empty(world: &mut IdWorld) {
    assert!(!world.subject().hash().as_str().is_empty());
}

#[then(expr = "the id's hash ends with {string}")]
fn id_hash_ends_with(world: &mut IdWorld, expected: String) {
    let hash: &str = world.subject().hash().as_str();
    assert!(
        hash.ends_with(&expected),
        "expected hash {hash} to end with {expected}"
    );
}

#[then(expr = "the id starts with {string}")]
fn id_starts_with(world: &mut IdWorld, expected: String) {
    let rendered: String = world.subject().to_string();
    assert!(
        rendered.starts_with(&expected),
        "expected {rendered} to start with {expected}"
    );
}

#[then(expr = "the id is a root id")]
fn id_is_root(world: &mut IdWorld) {
    assert!(world.subject().is_root());
}

#[then(expr = "the id is not a root id")]
fn id_is_not_root(world: &mut IdWorld) {
    assert!(!world.subject().is_root());
}

#[then(expr = "the id parses back to itself")]
fn id_round_trips(world: &mut IdWorld) {
    let rendered: String = world.subject().to_string();
    let reparsed: Id = Id::parse(&rendered).expect("a minted id must parse");
    assert_eq!(&reparsed, world.subject());
}

#[then(expr = "the slug portion of the id is {int} characters")]
fn slug_portion_length(world: &mut IdWorld, expected: usize) {
    assert_eq!(slug_portion(world).len(), expected);
}

#[then(expr = "the slug portion of the id does not end with a hyphen")]
fn slug_portion_no_trailing_hyphen(world: &mut IdWorld) {
    assert!(!slug_portion(world).ends_with('-'));
}

#[then(expr = "the id has no slug")]
fn id_has_no_slug(world: &mut IdWorld) {
    assert_eq!(
        world.subject().prefix(),
        world.generator.config().prefix(),
        "prefix should be the configured one, with no slug spliced in"
    );
}

/// The part of the id's prefix beyond the generator's configured prefix.
fn slug_portion(world: &IdWorld) -> String {
    let configured: String = format!("{}-", world.generator.config().prefix());
    world
        .subject()
        .prefix()
        .as_str()
        .strip_prefix(&configured)
        .unwrap_or_default()
        .to_owned()
}

// ---------------------------------------------------------------------------
// Then: hierarchy
// ---------------------------------------------------------------------------

#[then(expr = "the id has depth {int}")]
fn id_depth(world: &mut IdWorld, expected: usize) {
    assert_eq!(world.subject().depth(), expected);
}

#[then(expr = "the id has no parent")]
fn id_has_no_parent(world: &mut IdWorld) {
    assert!(world.subject().parent().is_none());
}

#[then(expr = "the parent is {string}")]
fn id_parent_is(world: &mut IdWorld, expected: String) {
    let parent: Id = world.subject().parent().expect("expected a parent");
    assert_eq!(parent.to_string(), expected);
}

#[then(expr = "the id is a descendant of {string}")]
fn id_is_descendant(world: &mut IdWorld, ancestor: String) {
    let ancestor: Id = Id::parse(&ancestor).expect("scenario ancestor is well formed");
    assert!(world.subject().is_descendant_of(&ancestor));
}

#[then(expr = "the id is not a descendant of {string}")]
fn id_is_not_descendant(world: &mut IdWorld, ancestor: String) {
    let ancestor: Id = Id::parse(&ancestor).expect("scenario ancestor is well formed");
    assert!(!world.subject().is_descendant_of(&ancestor));
}

// ---------------------------------------------------------------------------
// Then: parsing
// ---------------------------------------------------------------------------

#[then(expr = "parsing succeeds")]
fn parsing_succeeds(world: &mut IdWorld) {
    assert!(
        world.parsed.is_some(),
        "expected a parsed id, got {:?}",
        world.error
    );
}

#[then(expr = "the prefix is {string}")]
fn parsed_prefix_is(world: &mut IdWorld, expected: String) {
    assert_eq!(world.subject().prefix().as_str(), expected);
}

#[then(expr = "the hash is {string}")]
fn parsed_hash_is(world: &mut IdWorld, expected: String) {
    assert_eq!(world.subject().hash().as_str(), expected);
}

#[then(expr = "parsing fails because the separator is missing")]
fn parse_fails_missing_separator(world: &mut IdWorld) {
    assert_malformed(world, MalformedReason::MissingSeparator);
}

#[then(expr = "parsing fails because the prefix is empty")]
fn parse_fails_empty_prefix(world: &mut IdWorld) {
    assert_malformed(world, MalformedReason::EmptyPrefix);
}

#[then(expr = "parsing fails because the hash is empty")]
fn parse_fails_empty_hash(world: &mut IdWorld) {
    assert_malformed(world, MalformedReason::EmptyHash);
}

#[then(expr = "parsing fails because the hash is not base36")]
fn parse_fails_hash_charset(world: &mut IdWorld) {
    assert_malformed(world, MalformedReason::HashCharset);
}

#[then(expr = "parsing fails because a child segment is not a number")]
fn parse_fails_child_segment(world: &mut IdWorld) {
    assert_malformed(world, MalformedReason::ChildSegment);
}

fn assert_malformed(world: &IdWorld, expected: MalformedReason) {
    match world.error() {
        IdError::Malformed { reason, .. } => assert_eq!(*reason, expected),
        other => panic!("expected Malformed({expected:?}), got {other:?}"),
    }
    assert!(world.parsed.is_none(), "a malformed id parsed anyway");
}

// ---------------------------------------------------------------------------
// Then: prefixes
// ---------------------------------------------------------------------------

#[then(expr = "the prefix normalizes to {string}")]
fn prefix_normalizes_to(world: &mut IdWorld, expected: String) {
    assert_eq!(prefix_of(world).as_str(), expected);
}

#[then(expr = "the prefix parses to {string}")]
fn prefix_parses_to(world: &mut IdWorld, expected: String) {
    assert_eq!(prefix_of(world).as_str(), expected);
}

#[then(expr = "the prefix abbreviates to {string}")]
fn prefix_abbreviates_to(world: &mut IdWorld, expected: String) {
    assert_eq!(prefix_of(world).as_str(), expected);
}

#[then(expr = "normalizing fails because the prefix is unusable")]
fn prefix_normalize_fails(world: &mut IdWorld) {
    assert!(
        matches!(world.error(), IdError::UnusablePrefix { .. }),
        "expected UnusablePrefix, got {:?}",
        world.error()
    );
    assert!(world.prefix.is_none(), "an unusable prefix was accepted");
}

#[then(expr = "parsing the prefix fails")]
fn prefix_parse_fails(world: &mut IdWorld) {
    assert!(
        matches!(world.error(), IdError::Malformed { .. }),
        "expected Malformed, got {:?}",
        world.error()
    );
    assert!(world.prefix.is_none(), "an illegal prefix was accepted");
}

#[then(expr = "the prefix requirement is met")]
fn prefix_requirement_met(world: &mut IdWorld) {
    assert_eq!(world.prefix_ok, Some(true));
}

#[then(expr = "the prefix requirement is not met")]
fn prefix_requirement_not_met(world: &mut IdWorld) {
    assert_eq!(world.prefix_ok, Some(false));
}

fn prefix_of(world: &IdWorld) -> &Prefix {
    world.prefix.as_ref().expect("scenario produced no prefix")
}

// ---------------------------------------------------------------------------
// Then: validity and configuration
// ---------------------------------------------------------------------------

#[then(expr = "it is reported valid")]
fn reported_valid(world: &mut IdWorld) {
    assert_eq!(world.valid, Some(true));
}

#[then(expr = "it is reported invalid")]
fn reported_invalid(world: &mut IdWorld) {
    assert_eq!(world.valid, Some(false));
}

#[then(expr = "the configuration is accepted")]
fn configuration_accepted(world: &mut IdWorld) {
    assert!(
        world.config.is_some(),
        "expected a config, got {:?}",
        world.error
    );
}

#[then(expr = "the configuration is rejected as invalid hash bounds")]
fn configuration_rejected_bounds(world: &mut IdWorld) {
    assert!(
        matches!(world.error(), IdError::InvalidHashBounds { .. }),
        "expected InvalidHashBounds, got {:?}",
        world.error()
    );
    assert!(world.config.is_none(), "an invalid config was accepted");
}

#[then(expr = "the configuration is rejected as an invalid probability")]
fn configuration_rejected_probability(world: &mut IdWorld) {
    assert!(
        matches!(world.error(), IdError::InvalidCollisionProb { .. }),
        "expected InvalidCollisionProb, got {:?}",
        world.error()
    );
    assert!(world.config.is_none(), "an invalid config was accepted");
}

#[then(expr = "the failure reports {int} rejected candidates")]
fn failure_reports_attempts(world: &mut IdWorld, expected: u32) {
    match world.error() {
        IdError::Exhausted { attempts, .. } => assert_eq!(
            *attempts, expected,
            "exhaustion must account for every candidate the ladder rejected"
        ),
        other => panic!("expected Exhausted, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Runner
// ---------------------------------------------------------------------------

#[tokio::test]
async fn executable_specs() {
    IdWorld::cucumber()
        .fail_on_skipped()
        .run_and_exit("features")
        .await;
}
