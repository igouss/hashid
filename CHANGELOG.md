# Changelog

All notable changes to this project are documented here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and
this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] — 2026-07-10

Initial release. Extracted from the [beads](https://github.com/steveyegge/beads)
Rust implementation (`src/util/id.rs`), where the scheme has been minting issue
IDs in production.

### The wire format is unchanged

`hashid` reproduces the original's IDs byte for byte from the same inputs. The
seed encoding (length-prefixed fields), the digest (SHA-256 → leading 8 bytes →
`u64` → base36 → last *n* characters), the birthday-bound length sizing, and the
collision escalation ladder are all preserved exactly.

`tests/conformance.rs` pins this against 25 golden vectors captured from the
original implementation at a frozen instant. An ID minted by beads still parses,
resolves, and reproduces here.

### Added

- `Id` — one parsed, always-well-formed identifier type. `Hash + Eq + Ord`, so it
  drops straight into a `HashSet`. Root and hierarchical (`br-abc123.1.2`) forms,
  with `parent()`, `child()`, `depth()`, and `is_descendant_of()`.
- `Prefix`, `Slug`, `Base36Hash` — validated value objects. Two prefix
  constructors with two different contracts: `normalize` (lenient, for humans)
  and `parse` (strict, for machines).
- `IdGenerator` — `mint`, `mint_with_slug`, and `mint_unchecked`.
- `IdResolver` — exact → prefix-normalized → substring → ambiguous/absent.
- `IdRegistry` and `IdIndex` ports; `InMemoryIds`, `FnRegistry`, `FnIndex`
  adapters.
- `MalformedReason` — a parse failure names the grammar rule it broke.
- 90 Gherkin scenarios, 13 property tests, 27 adapter tests, 25 golden
  conformance vectors. Verified with `cargo-mutants`; the only survivors are
  equivalent mutants, documented in the README.

### Changed from the original

Behavioural differences a caller migrating from beads must know about:

- **Exhaustion is an error, not a duplicate.** The original's escalation ladder
  ended by returning an ID it had already been told was taken (`nonce > 2000`
  returned unconditionally). That is silent data corruption. Minting now returns
  `IdError::Exhausted` instead.
- **The registry port is fallible.** `exists: Fn(&str) -> bool` forced every
  store-backed adapter to swallow lookup errors and answer `true` or `false`
  anyway — callers in the original smuggle the real error out through a
  `RefCell` on the side. `IdRegistry::contains` returns `Result`, and the
  generator propagates rather than guessing.
- **An unusable prefix is refused, not defaulted.** The original silently
  substituted `"br"` when a configured prefix normalized to nothing, attaching
  real IDs to a namespace the caller never chose. Now `IdError::UnusablePrefix`.
- **`generate_id()` is gone.** It hardcoded `|_| false` as its collision check —
  an innocuously-named public function that never checked for collisions. Its
  replacement is `mint_unchecked()`, which cannot be called by accident.
- **`population` (was `issue_count`) is a positional argument** of
  `IdRequest::new`, not a struct field that defaults to zero. In the original, a
  caller who forgot it silently minted minimum-entropy IDs into a large dataset —
  and one did.
- **One `resolve`, not two.** The original carried parallel `resolve` and
  `resolve_fallible` implementations, ~120 duplicated lines. The fallible port
  makes the infallible variant unnecessary.
- **Ambiguous matches are sorted**, so the candidate list is deterministic.
- **The default prefix is `id`,** not `br`. This crate has no opinion about
  beads.
