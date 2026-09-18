//! The resolution use case: fragment in, real ID out.

use hashid::{parse_id, Id, IdError, MalformedReason, Prefix, StoreError};

use crate::config::ResolverConfig;
use crate::error::{ResolveError, Result};
use crate::matcher::matches_fragment;
use crate::resolved::{MatchType, ResolvedId};

/// Answers "does this exact ID exist?" — an indexed point lookup.
pub trait Exists: Fn(&Id) -> core::result::Result<bool, StoreError> {}
impl<T: Fn(&Id) -> core::result::Result<bool, StoreError>> Exists for T {}

/// Answers "which IDs might match this fragment?" — over-returning is free.
pub trait Candidates: Fn(&str) -> core::result::Result<Vec<Id>, StoreError> {}
impl<T: Fn(&str) -> core::result::Result<Vec<Id>, StoreError>> Candidates for T {}

/// Turns what a human typed into the ID they meant.
///
/// Four steps, in order, stopping at the first that succeeds:
///
/// 1. **Exact** — the input is already a complete, existing ID.
/// 2. **Prefix-normalized** — the input is a bare hash; prepend the default
///    prefix and try again.
/// 3. **Substring** — the input is a fragment of a hash. Exactly one match
///    wins.
/// 4. **Ambiguous or absent** — several matches, or none. Both are errors, and
///    the ambiguous one names every candidate so the caller can offer a choice.
///
/// ```
/// # use hashid::Id;
/// # use hashid_resolve::{MatchType, Resolver};
/// let known: Vec<Id> = vec![Id::parse("br-abc123")?, Id::parse("br-xyz789")?];
/// let resolver: Resolver = Resolver::new("br")?;
///
/// let exists = |id: &Id| Ok(known.contains(id));
/// let candidates = |_: &str| Ok(known.clone());
///
/// // A bare hash gets the default prefix.
/// let resolved = resolver.resolve("abc123", &exists, &candidates)?;
/// assert_eq!(resolved.match_type, MatchType::PrefixNormalized);
///
/// // A fragment resolves when it is unambiguous.
/// let resolved = resolver.resolve("xyz", &exists, &candidates)?;
/// assert_eq!(resolved.id.to_string(), "br-xyz789");
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone)]
pub struct Resolver {
    config: ResolverConfig,
}

impl Resolver {
    /// A resolver that prepends `default_prefix` to a bare hash.
    ///
    /// The prefix is normalized the lenient way, as [`hashid::Minter::new`]
    /// does, because it usually comes from the same config value.
    ///
    /// # Errors
    ///
    /// [`ResolveError::Malformed`] when no usable character survives.
    pub fn new(default_prefix: &str) -> Result<Self> {
        Ok(Self {
            config: ResolverConfig::new(Prefix::normalize(default_prefix)?),
        })
    }

    /// A resolver bound to an explicit `config`.
    #[must_use]
    pub fn with_config(config: ResolverConfig) -> Self {
        Self { config }
    }

    /// The policy this resolver applies.
    #[must_use]
    pub fn config(&self) -> &ResolverConfig {
        &self.config
    }

    /// Resolve `input` against `index`.
    ///
    /// Input is trimmed and lowercased before matching, so `  BR-ABC123  `
    /// resolves exactly as `br-abc123` does.
    ///
    /// `candidates` may over-return freely: this method applies
    /// [`matches_fragment`] to everything it gets back, so the rule lives here
    /// rather than in your store.
    ///
    /// # Errors
    ///
    /// - [`ResolveError::Malformed`] when `input` is empty.
    /// - [`ResolveError::Ambiguous`] when a fragment matches more than one ID.
    ///   The error carries every match, sorted, so a CLI can print them.
    /// - [`ResolveError::NotFound`] when nothing matches.
    /// - [`ResolveError::Store`] when a lookup cannot answer. Resolution
    ///   propagates the failure rather than reporting a spurious "not found" —
    ///   an unreachable database and a genuinely absent ID are not the same
    ///   answer, and a caller that deletes on "not found" would very much like
    ///   to know the difference.
    pub fn resolve(
        &self,
        input: &str,
        exists: impl Exists,
        candidates: impl Candidates,
    ) -> Result<ResolvedId> {
        let trimmed: &str = input.trim();
        if trimmed.is_empty() {
            return Err(IdError::malformed(input, MalformedReason::Empty).into());
        }
        let lowered: String = trimmed.to_lowercase();

        if let Some(resolved) = Self::match_exact(&lowered, input, &exists)? {
            return Ok(resolved);
        }
        if let Some(resolved) = self.match_with_default_prefix(&lowered, input, &exists)? {
            return Ok(resolved);
        }
        if let Some(resolved) = self.match_substring(&lowered, input, &candidates)? {
            return Ok(resolved);
        }

        Err(ResolveError::NotFound {
            fragment: trimmed.to_owned(),
        })
    }

    /// Resolve every input, failing on the first that cannot be resolved.
    ///
    /// # Errors
    ///
    /// As [`Resolver::resolve`], for the first input that fails.
    pub fn resolve_all(
        &self,
        inputs: &[String],
        exists: impl Exists,
        candidates: impl Candidates,
    ) -> Result<Vec<ResolvedId>> {
        inputs
            .iter()
            .map(|input: &String| self.resolve(input, &exists, &candidates))
            .collect()
    }

    /// Step 1: the input is already a complete, existing ID.
    fn match_exact(
        lowered: &str,
        input: &str,
        exists: &impl Exists,
    ) -> Result<Option<ResolvedId>> {
        let Ok(id) = parse_id(lowered) else {
            return Ok(None);
        };
        if !exists(&id)? {
            return Ok(None);
        }
        Ok(Some(ResolvedId {
            id,
            match_type: MatchType::Exact,
            input: input.to_owned(),
        }))
    }

    /// Step 2: the input is a bare hash, missing its namespace.
    fn match_with_default_prefix(
        &self,
        lowered: &str,
        input: &str,
        exists: &impl Exists,
    ) -> Result<Option<ResolvedId>> {
        if lowered.contains('-') {
            return Ok(None);
        }
        let qualified: String = format!("{}-{lowered}", self.config.default_prefix());
        let Ok(id) = parse_id(&qualified) else {
            return Ok(None);
        };
        if !exists(&id)? {
            return Ok(None);
        }
        Ok(Some(ResolvedId {
            id,
            match_type: MatchType::PrefixNormalized,
            input: input.to_owned(),
        }))
    }

    /// Step 3: the input is a fragment of a hash.
    fn match_substring(
        &self,
        lowered: &str,
        input: &str,
        candidates: &impl Candidates,
    ) -> Result<Option<ResolvedId>> {
        if !self.config.allow_substring_match() {
            return Ok(None);
        }

        // A `-` means the user pinned a namespace: `br-ab` searches for `ab`,
        // but only within `br`.
        //
        // The degenerate splits need no guard. `-abc` yields the namespace ""
        // and no `Prefix` can be empty, so the retain below matches nothing;
        // `abc-` yields an empty fragment, which returns early. Both land on
        // the same "no match" the unguarded path would have produced.
        let (namespace, fragment): (Option<&str>, &str) = match lowered.rsplit_once('-') {
            Some((prefix, rest)) => (Some(prefix), rest),
            None => (None, lowered),
        };
        if fragment.is_empty() {
            return Ok(None);
        }

        // The store narrows; this applies the rule. Over-returning therefore
        // costs only time, and no adapter can turn an ambiguous match into a
        // confident wrong one by forgetting the child-path rule.
        let mut matches: Vec<Id> = candidates(fragment)?;
        matches.retain(|id: &Id| matches_fragment(id, fragment));
        if let Some(namespace) = namespace {
            matches.retain(|id: &Id| id.prefix().as_str() == namespace);
        }

        match matches.len() {
            0 => Ok(None),
            1 => {
                let id: Id = matches
                    .pop()
                    .unwrap_or_else(|| unreachable!("length was just checked to be 1"));
                Ok(Some(ResolvedId {
                    id,
                    match_type: MatchType::Substring,
                    input: input.to_owned(),
                }))
            }
            _ => {
                let mut rendered: Vec<String> =
                    matches.iter().map(ToString::to_string).collect::<Vec<_>>();
                rendered.sort();
                Err(ResolveError::Ambiguous {
                    fragment: input.trim().to_owned(),
                    matches: rendered,
                })
            }
        }
    }
}
