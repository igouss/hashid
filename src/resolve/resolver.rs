//! The resolution use case: fragment in, real ID out.

use crate::domain::id::Id;
use crate::domain::resolved::{MatchType, ResolvedId};
use crate::error::{IdError, MalformedReason, Result};
use crate::parse::parse_id;
use crate::port::IdIndex;
use crate::resolve::config::ResolverConfig;

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
/// # use hashid::{Id, IdResolver, InMemoryIds, MatchType, Prefix, ResolverConfig};
/// let known: InMemoryIds = ["br-abc123", "br-xyz789"]
///     .iter()
///     .map(|raw| Id::parse(raw))
///     .collect::<Result<InMemoryIds, _>>()?;
///
/// let resolver: IdResolver = IdResolver::new(ResolverConfig::new(Prefix::parse("br")?));
///
/// // A bare hash gets the default prefix.
/// let resolved = resolver.resolve("abc123", &known)?;
/// assert_eq!(resolved.match_type, MatchType::PrefixNormalized);
///
/// // A fragment resolves when it is unambiguous.
/// let resolved = resolver.resolve("xyz", &known)?;
/// assert_eq!(resolved.id.to_string(), "br-xyz789");
/// # Ok::<(), hashid::IdError>(())
/// ```
#[derive(Debug, Clone)]
pub struct IdResolver {
    config: ResolverConfig,
}

impl IdResolver {
    /// A resolver bound to `config`.
    #[must_use]
    pub fn new(config: ResolverConfig) -> Self {
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
    /// # Errors
    ///
    /// - [`IdError::Malformed`] when `input` is empty.
    /// - [`IdError::Ambiguous`] when a fragment matches more than one ID. The
    ///   error carries every match, sorted, so a CLI can print them.
    /// - [`IdError::NotFound`] when nothing matches.
    /// - [`IdError::Registry`] when the index cannot answer. Resolution
    ///   propagates the failure rather than reporting a spurious "not found" —
    ///   an unreachable database and a genuinely absent ID are not the same
    ///   answer, and a caller that deletes on "not found" would very much like
    ///   to know the difference.
    pub fn resolve<I: IdIndex + ?Sized>(&self, input: &str, index: &I) -> Result<ResolvedId> {
        let trimmed: &str = input.trim();
        if trimmed.is_empty() {
            return Err(IdError::malformed(input, MalformedReason::Empty));
        }
        let lowered: String = trimmed.to_lowercase();

        if let Some(resolved) = Self::match_exact(&lowered, input, index)? {
            return Ok(resolved);
        }
        if let Some(resolved) = self.match_with_default_prefix(&lowered, input, index)? {
            return Ok(resolved);
        }
        if let Some(resolved) = self.match_substring(&lowered, input, index)? {
            return Ok(resolved);
        }

        Err(IdError::NotFound {
            fragment: trimmed.to_owned(),
        })
    }

    /// Resolve every input, failing on the first that cannot be resolved.
    ///
    /// # Errors
    ///
    /// As [`IdResolver::resolve`], for the first input that fails.
    pub fn resolve_all<I: IdIndex + ?Sized>(
        &self,
        inputs: &[String],
        index: &I,
    ) -> Result<Vec<ResolvedId>> {
        inputs
            .iter()
            .map(|input: &String| self.resolve(input, index))
            .collect()
    }

    /// Step 1: the input is already a complete, existing ID.
    fn match_exact<I: IdIndex + ?Sized>(
        lowered: &str,
        input: &str,
        index: &I,
    ) -> Result<Option<ResolvedId>> {
        let Ok(id) = parse_id(lowered) else {
            return Ok(None);
        };
        if !index.contains(&id)? {
            return Ok(None);
        }
        Ok(Some(ResolvedId {
            id,
            match_type: MatchType::Exact,
            input: input.to_owned(),
        }))
    }

    /// Step 2: the input is a bare hash, missing its namespace.
    fn match_with_default_prefix<I: IdIndex + ?Sized>(
        &self,
        lowered: &str,
        input: &str,
        index: &I,
    ) -> Result<Option<ResolvedId>> {
        if lowered.contains('-') {
            return Ok(None);
        }
        let qualified: String = format!("{}-{lowered}", self.config.default_prefix());
        let Ok(id) = parse_id(&qualified) else {
            return Ok(None);
        };
        if !index.contains(&id)? {
            return Ok(None);
        }
        Ok(Some(ResolvedId {
            id,
            match_type: MatchType::PrefixNormalized,
            input: input.to_owned(),
        }))
    }

    /// Step 3: the input is a fragment of a hash.
    fn match_substring<I: IdIndex + ?Sized>(
        &self,
        lowered: &str,
        input: &str,
        index: &I,
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

        let mut matches: Vec<Id> = index.find_by_hash_fragment(fragment)?;
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
                Err(IdError::Ambiguous {
                    fragment: input.trim().to_owned(),
                    matches: rendered,
                })
            }
        }
    }
}
