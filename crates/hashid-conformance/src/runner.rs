//! Running the vectors against this implementation.
//!
//! One function per group would be twenty-two functions that differ only in
//! which fields they read. Instead every group is evaluated to a
//! [`serde_json::Value`] and compared against the vector's `expect` — so the
//! comparison, the error handling and the reporting are written once, and a
//! group is the handful of lines that actually computes something.

use chrono::{DateTime, Utc};
use serde_json::{json, Value};

use std::collections::HashSet;

use hashid::{
    base36, digest, parse_id, seed, validate_prefix, HashLength, Id, IdConfig, IdError, Minter,
    Prefix, Request, Slug, StoreError,
};
use hashid_resolve::{matches_fragment, MatchType, ResolveError, Resolver, ResolverConfig};

use crate::{suite, Vector};

/// One vector that did not hold.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    /// The vector's stable identifier, e.g. `SEED-004`.
    pub vector: String,
    /// What disagreed.
    pub detail: String,
}

/// Render a list of failures for an assertion message.
#[must_use]
pub fn render(failures: &[Failure]) -> String {
    failures
        .iter()
        .map(|failure: &Failure| format!("  {}: {}", failure.vector, failure.detail))
        .collect::<Vec<String>>()
        .join("\n")
}

// ---------------------------------------------------------------------------
// Reading a vector's fields
// ---------------------------------------------------------------------------

/// A field that is missing or the wrong type is a defect in the vector file,
/// reported as loudly as a failing expectation.
type Read<T> = Result<T, String>;

/// A required string field.
fn text<'a>(vector: &'a Vector, key: &str) -> Read<&'a str> {
    vector
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("vector has no string field `{key}`"))
}

/// A required unsigned field, narrowed to `T`.
fn number<T: TryFrom<u64>>(vector: &Vector, key: &str) -> Read<T> {
    let raw: u64 = vector
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("vector has no unsigned field `{key}`"))?;
    T::try_from(raw).map_err(|_| format!("`{key}` = {raw} does not fit its type"))
}

/// A required boolean field.
fn flag(vector: &Vector, key: &str) -> Read<bool> {
    vector
        .get(key)
        .and_then(Value::as_bool)
        .ok_or_else(|| format!("vector has no boolean field `{key}`"))
}

/// A required float field.
fn float(vector: &Vector, key: &str) -> Read<f64> {
    vector
        .get(key)
        .and_then(Value::as_f64)
        .ok_or_else(|| format!("vector has no float field `{key}`"))
}

/// A required array-of-strings field.
fn list<'a>(vector: &'a Vector, key: &str) -> Read<Vec<&'a str>> {
    vector
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("vector has no array field `{key}`"))?
        .iter()
        .map(|entry: &Value| {
            entry
                .as_str()
                .ok_or_else(|| format!("`{key}` holds a non-string entry"))
        })
        .collect()
}

/// A 64-bit integer carried as a decimal string, because JSON numbers cannot
/// hold one exactly.
fn big(vector: &Vector, key: &str) -> Read<i64> {
    text(vector, key)?
        .parse::<i64>()
        .map_err(|err| format!("`{key}` is not an i64: {err}"))
}

/// The prefix a vector names, already validated.
fn prefix(vector: &Vector, key: &str) -> Read<Prefix> {
    let raw: &str = text(vector, key)?;
    Prefix::parse(raw).map_err(|err| format!("vector's `{key}` `{raw}` is not legal: {err}"))
}

/// The store a vector's `taken` or `known` list describes.
fn registry(vector: &Vector, key: &str) -> Read<HashSet<Id>> {
    if text(vector, "store").ok() == Some("failing") {
        return Ok(HashSet::new());
    }
    list(vector, key)?
        .into_iter()
        .map(|raw: &str| {
            Id::parse(raw).map_err(|err| format!("vector lists `{raw}`, not a legal id: {err}"))
        })
        .collect()
}

/// Is this a vector whose store cannot answer?
fn store_fails(vector: &Vector) -> bool {
    text(vector, "store").ok() == Some("failing")
}

/// A lookup that always fails, for the vectors that check the failure is
/// propagated rather than read as a negative answer.
fn unavailable<T>() -> Result<T, StoreError> {
    Err(StoreError::new("the vector's store is unavailable"))
}

/// The content a vector's `request` object describes.
fn request(vector: &Vector) -> Read<Request<'_>> {
    let object: &Vector = vector
        .get("request")
        .and_then(Value::as_object)
        .ok_or_else(|| "vector has no `request` object".to_owned())?;

    let created_at: DateTime<Utc> = DateTime::from_timestamp_nanos(big(object, "created_at_nanos")?);
    let population: usize = number::<u64>(object, "population")
        .and_then(|value: u64| {
            usize::try_from(value).map_err(|_| format!("`population` {value} exceeds usize"))
        })?;

    let mut core: Request<'_> = Request::new(text(object, "title")?)
        .at(created_at)
        .map_err(|err| format!("vector's `created_at_nanos` is unrepresentable: {err}"))?
        .with_population(population);
    if let Some(description) = object.get("description").and_then(Value::as_str) {
        core = core.with_description(description);
    }
    if let Some(creator) = object.get("creator").and_then(Value::as_str) {
        core = core.with_creator(creator);
    }
    Ok(core)
}

/// The entropy policy a vector describes, defaulting to the standard one.
///
/// A vector that names no `prefix` gets a placeholder: the groups in that
/// position vary the policy, not the namespace, and the answer is the same
/// under any legal prefix.
fn policy(vector: &Vector) -> Read<Result<IdConfig, IdError>> {
    let namespace: Prefix = match vector.get("prefix") {
        Some(_) => prefix(vector, "prefix")?,
        None => Prefix::parse("x").unwrap_or_else(|_| unreachable!("`x` is a legal prefix")),
    };
    let base: IdConfig = IdConfig::new(namespace);
    let (Ok(min), Ok(max)) = (
        number::<usize>(vector, "min_hash_length"),
        number::<usize>(vector, "max_hash_length"),
    ) else {
        return Ok(Ok(base));
    };
    let probability: f64 = float(vector, "max_collision_prob").unwrap_or(0.25);
    Ok(base
        .with_hash_lengths(min, max)
        .and_then(|config: IdConfig| config.with_max_collision_prob(probability)))
}

/// The hash length a vector names.
fn hash_length(vector: &Vector, key: &str) -> Read<Result<HashLength, IdError>> {
    Ok(HashLength::new(number::<usize>(vector, key)?))
}

// ---------------------------------------------------------------------------
// The groups
// ---------------------------------------------------------------------------

/// The name the specification gives a match type.
fn match_kind(match_type: MatchType) -> &'static str {
    match match_type {
        MatchType::Exact => "Exact",
        MatchType::PrefixNormalized => "PrefixNormalized",
        MatchType::Substring => "Substring",
    }
}

/// Run one vector, yielding what the implementation produced.
///
/// The outer `Result` reports a malformed *vector*; the inner one reports what
/// the implementation did with a well-formed vector. Each arm reads every field
/// it needs first, so a vector missing one is reported as a vector defect
/// rather than as a failing expectation.
///
/// # Errors
///
/// A field the group needs is missing, the wrong type, or names an unknown
/// group — all defects in `spec/vectors.json` rather than in the code.
#[allow(
    clippy::too_many_lines,
    reason = "one arm per capability, each a few lines: the length is the \
              catalogue, and splitting it would only hide the list"
)]
fn evaluate_minting(group: &str, vector: &Vector) -> Read<Result<Value, IdError>> {
    Ok(match group {
        "base36" => {
            let value: u64 = text(vector, "value")?
                .parse::<u64>()
                .map_err(|err| format!("`value` is not a u64: {err}"))?;
            Ok(json!(base36(value)))
        }

        "seed" => {
            let content: Request<'_> = request(vector)?;
            let nonce: u32 = number(vector, "nonce")?;
            Ok(json!(seed(&content, nonce)))
        }

        "digest" => {
            let input: &str = text(vector, "input")?;
            hash_length(vector, "length")?
                .map(|length: HashLength| json!(digest(input, length).as_str()))
        }

        "hash_length" => {
            let population: usize = usize::try_from(number::<u64>(vector, "population")?)
                .map_err(|_| "`population` exceeds usize".to_owned())?;
            policy(vector)?.map(|policy: IdConfig| {
                json!(Minter::with_config(policy).hash_length_for(population).get())
            })
        }

        "config_validation" => policy(vector)?.map(|_| Value::Null),

        "mint_unchecked" => {
            let generator: Minter = Minter::with_config(IdConfig::new(prefix(vector, "prefix")?));
            let content: Request<'_> = request(vector)?;
            let nonce: u32 = number(vector, "nonce")?;
            hash_length(vector, "hash_length")?.map(|length: HashLength| {
                json!(generator.mint_unchecked(&content, nonce, length).to_string())
            })
        }

        "parse_id" => parse_id(text(vector, "input")?).map(|id: Id| {
            json!({
                "text": id.to_string(),
                "prefix": id.prefix().as_str(),
                "hash": id.hash().as_str(),
                "child_path": id.child_path(),
            })
        }),

        "format_id" => render_id(vector)?.map(Value::from),

        "normalize_prefix" => {
            Prefix::normalize(text(vector, "input")?).map(|p: Prefix| json!(p.as_str()))
        }

        "parse_prefix" => Prefix::parse(text(vector, "input")?).map(|p: Prefix| json!(p.as_str())),

        "abbreviate_prefix" => Prefix::parse(text(vector, "input")?)
            .map(|p: Prefix| json!(p.abbreviated().as_str())),

        "normalize_slug" => Ok(json!(Slug::normalize(text(vector, "input")?)
            .map(|slug: Slug| slug.as_str().to_owned()))),

        "join_prefix_slug" => {
            let namespace: Prefix = prefix(vector, "prefix")?;
            Ok(json!(Slug::normalize(text(vector, "slug")?)
                .and_then(|slug: Slug| namespace.joined_with(&slug))
                .map(|joined: Prefix| joined.as_str().to_owned())))
        }

        "id_parent" => parse_id(text(vector, "input")?)
            .map(|id: Id| json!(id.parent().map(|parent: Id| parent.to_string()))),

        "id_child" => {
            let n: u32 = number(vector, "n")?;
            parse_id(text(vector, "input")?).map(|id: Id| json!(id.child(n).to_string()))
        }

        "id_depth" => parse_id(text(vector, "input")?).map(|id: Id| json!(id.depth())),

        "is_descendant_of" => {
            let ancestor: &str = text(vector, "ancestor")?;
            parse_id(text(vector, "input")?).and_then(|id: Id| {
                parse_id(ancestor).map(|ancestor: Id| json!(id.is_descendant_of(&ancestor)))
            })
        }

        "matches_fragment" => {
            let fragment: &str = text(vector, "fragment")?;
            parse_id(text(vector, "input")?).map(|id: Id| json!(matches_fragment(&id, fragment)))
        }

        "validate_prefix" => {
            let expected: Prefix = prefix(vector, "expected")?;
            let allowed: Result<Vec<Prefix>, IdError> = list(vector, "also_allowed")?
                .into_iter()
                .map(Prefix::parse)
                .collect();
            match (parse_id(text(vector, "input")?), allowed) {
                (Ok(id), Ok(allowed)) => {
                    Ok(json!(validate_prefix(&id, &expected, &allowed).is_ok()))
                }
                (Err(err), _) | (_, Err(err)) => Err(err),
            }
        }

        "mint" | "mint_with_slug" => {
            let content: Request<'_> = request(vector)?;
            let held: HashSet<Id> = registry(vector, "taken")?;
            let slug: Option<&str> = (group == "mint_with_slug")
                .then(|| text(vector, "slug"))
                .transpose()?;
            let broken: bool = store_fails(vector);

            match policy(vector)? {
                Err(err) => Err(err),
                Ok(policy) => {
                    let generator: Minter = Minter::with_config(policy);
                    match (slug, broken) {
                        (Some(slug), false) => {
                            generator.mint_with_slug(&content, slug, |id: &Id| Ok(held.contains(id)))
                        }
                        (Some(slug), true) => {
                            generator.mint_with_slug(&content, slug, |_: &Id| unavailable())
                        }
                        (None, false) => generator.mint(&content, |id: &Id| Ok(held.contains(id))),
                        (None, true) => generator.mint(&content, |_: &Id| unavailable()),
                    }
                    .map(|id: Id| json!(id.to_string()))
                }
            }
        }

        other => return Err(format!("no runner knows the group `{other}`")),
    })
}

/// Assemble an ID from a `format_id` vector's parts, re-validating.
fn render_id(vector: &Vector) -> Read<Result<String, IdError>> {
    let segments: Vec<u64> = vector
        .get("child_path")
        .and_then(Value::as_array)
        .ok_or_else(|| "vector has no array field `child_path`".to_owned())?
        .iter()
        .map(|entry: &Value| {
            entry
                .as_u64()
                .ok_or_else(|| "`child_path` holds a non-integer".to_owned())
        })
        .collect::<Read<Vec<u64>>>()?;

    let prefix_text: &str = text(vector, "prefix")?;
    let hash_text: &str = text(vector, "hash")?;
    Ok((|| -> Result<String, IdError> {
        let prefix: Prefix = Prefix::parse(prefix_text)?;
        let hash: hashid::Base36Hash = hashid::Base36Hash::parse(hash_text)?;
        let mut rendered: String = format!("{prefix}-{hash}");
        for segment in &segments {
            rendered.push('.');
            rendered.push_str(&segment.to_string());
        }
        parse_id(&rendered)?;
        Ok(rendered)
    })())
}


/// The `resolve` group, which lives in `hashid-resolve` rather than the core.
fn evaluate_resolution(vector: &Vector) -> Read<Result<Value, ResolveError>> {
    let resolver: Resolver = Resolver::with_config(
        ResolverConfig::new(prefix(vector, "default_prefix")?)
            .with_substring_match(flag(vector, "allow_substring_match")?),
    );
    let known: HashSet<Id> = registry(vector, "known")?;
    let input: &str = text(vector, "input")?;

    let outcome = if store_fails(vector) {
        resolver.resolve(input, |_: &Id| unavailable(), |_: &str| unavailable())
    } else {
        // The store answers the widest possible way, applying no filter at
        // all. `hashid-resolve` applies the fragment rule itself, so this is
        // legal — and running the vectors through it is what proves it.
        resolver.resolve(
            input,
            |id: &Id| Ok(known.contains(id)),
            |_: &str| Ok(known.iter().cloned().collect()),
        )
    };

    Ok(outcome.map(|resolved| {
        json!({ "id": resolved.id.to_string(),
                "match_kind": match_kind(resolved.match_type) })
    }))
}

/// Run one vector's group, whichever crate owns it.
///
/// # Errors
///
/// A message naming what the vector itself got wrong, as distinct from the
/// implementation disagreeing with it.
pub fn evaluate(group: &str, vector: &Vector) -> Read<Result<Value, VectorError>> {
    if group == "resolve" {
        return Ok(evaluate_resolution(vector)?.map_err(VectorError::Resolve));
    }
    Ok(evaluate_minting(group, vector)?.map_err(VectorError::Id))
}

// ---------------------------------------------------------------------------
// Comparing
// ---------------------------------------------------------------------------

/// A failure from either crate the vectors cover.
///
/// `spec/vectors.json` spans minting and resolution, which now live in
/// `hashid` and `hashid-resolve` respectively. The specification's error
/// taxonomy did not split with them, so the runner rejoins the two here.
#[derive(Debug)]
pub enum VectorError {
    /// A failure from the core crate.
    Id(IdError),
    /// A failure from the resolution crate.
    Resolve(ResolveError),
}

impl From<IdError> for VectorError {
    fn from(err: IdError) -> Self {
        Self::Id(err)
    }
}

impl From<ResolveError> for VectorError {
    fn from(err: ResolveError) -> Self {
        Self::Resolve(err)
    }
}

impl std::fmt::Display for VectorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Id(err) => write!(f, "{err}"),
            Self::Resolve(err) => write!(f, "{err}"),
        }
    }
}

/// The specification's name for an error.
fn error_code(err: &VectorError) -> &'static str {
    match err {
        VectorError::Id(IdError::Malformed { .. }) => "Malformed",
        VectorError::Id(IdError::PrefixMismatch { .. }) => "PrefixMismatch",
        VectorError::Id(IdError::UnusablePrefix { .. }) => "UnusablePrefix",
        VectorError::Id(IdError::InvalidHashBounds { .. }) => "InvalidHashBounds",
        VectorError::Id(IdError::InvalidHashLength { .. }) => "InvalidHashLength",
        VectorError::Id(IdError::InvalidCollisionProb { .. }) => "InvalidCollisionProb",
        VectorError::Id(IdError::Exhausted { .. }) => "Exhausted",
        VectorError::Id(IdError::Store(_)) | VectorError::Resolve(ResolveError::Store(_)) => "Store",
        VectorError::Resolve(ResolveError::Ambiguous { .. }) => "Ambiguous",
        VectorError::Resolve(ResolveError::NotFound { .. }) => "NotFound",
        VectorError::Resolve(ResolveError::Malformed(inner)) => error_code(&VectorError::Id(
            // A resolve-side malformed input is the same failure by another route.
            match inner {
                IdError::Malformed { id, reason } => IdError::malformed(id, *reason),
                _ => return "Unknown",
            },
        )),
        _ => "Unknown",
    }
}

/// Check one vector, returning what disagreed.
fn check(group: &str, vector: &Vector) -> Result<(), String> {
    let actual: Result<Value, VectorError> = evaluate(group, vector)?;

    match (vector.get("expect"), vector.get("error")) {
        (Some(expected), None) => match actual {
            Ok(value) if &value == expected => Ok(()),
            Ok(value) => Err(format!("expected {expected}, got {value}")),
            Err(err) => Err(format!("expected {expected}, got error `{err}`")),
        },
        (None, Some(code)) => match actual {
            Ok(value) => Err(format!("expected error {code}, got {value}")),
            Err(err) => {
                let found: &str = error_code(&err);
                if Some(found) != code.as_str() {
                    return Err(format!("expected error {code}, got `{found}` ({err})"));
                }
                if let Some(reason) = vector.get("reason").and_then(Value::as_str) {
                    if let VectorError::Id(IdError::Malformed { reason: actual, .. })
                        | VectorError::Resolve(ResolveError::Malformed(IdError::Malformed {
                            reason: actual,
                            ..
                        })) = &err
                    {
                        let actual: String = format!("{actual:?}");
                        if actual != reason {
                            return Err(format!("expected reason `{reason}`, got `{actual}`"));
                        }
                    }
                }
                if let (Some(expected), VectorError::Resolve(ResolveError::Ambiguous { matches, .. })) =
                    (vector.get("matches"), &err)
                {
                    let found: Value = json!(matches);
                    if &found != expected {
                        return Err(format!("expected matches {expected}, got {found}"));
                    }
                }
                Ok(())
            }
        },
        _ => Err("vector declares neither `expect` nor `error`".to_owned()),
    }
}

/// Run every vector in `group`, returning every failure.
///
/// # Panics
///
/// If `group` is not in the suite. The tests name the groups they run, so a
/// typo is a defect in the test, not a reason to silently pass.
#[must_use]
pub fn check_group(group: &str) -> Vec<Failure> {
    let vectors: &Vec<Vector> = suite()
        .groups
        .get(group)
        .unwrap_or_else(|| panic!("spec/vectors.json has no group `{group}`"));
    assert!(!vectors.is_empty(), "group `{group}` has no vectors");

    vectors
        .iter()
        .filter_map(|vector: &Vector| {
            let id: String = vector
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("<unnamed>")
                .to_owned();
            check(group, vector)
                .err()
                .map(|detail: String| Failure { vector: id, detail })
        })
        .collect()
}
