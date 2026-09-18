# Changelog

All notable changes to this project are documented here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and
this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added — `hashid` from other languages

- **`hashid-ffi`** — a [UniFFI](https://github.com/mozilla/uniffi-rs) component.
  `cargo run --bin uniffi-bindgen -- generate --library … --language kotlin`
  produces bindings for Kotlin, Swift, Python and Ruby; third-party backends
  cover Java, Go, C# and React Native. IDs cross the boundary as strings and
  instants as nanoseconds, because both are hashed verbatim and a rounded
  timestamp mints a different ID with nothing to warn you.

  The surface is deliberately small: four free functions (`parse_id`,
  `is_valid_id`, `normalize_prefix`, `now_nanos`), one object (`Minter`), and
  one interface with one method (`IdStore`). Anything derivable from `parse_id` —
  an ID's depth, root-ness, parent, n-th child, descendancy, a fragment search
  over a list — is left to the host language, because an export you have to
  look up costs more than the line it saves.
- **`hashid-c`** — a hand-written C ABI with
  [`hashid.h`](crates/hashid-c/include/hashid.h), reaching C, C++, Zig, and
  every language with an FFI but no UniFFI backend. Sixteen functions,
  mirroring `hashid-ffi`'s surface. Status codes, one `hashid_*_free` per owned
  value, and a panic fence at every entry point, because a Rust panic unwinding
  into C frames is undefined behaviour.

  Hand-written because UniFFI has no C backend, first-party or third-party: its
  scaffolding is a C ABI, but an internal one, with no header and no stability
  promise across versions.

  Its store mirrors `hashid-ffi`'s: a single `hashid_contains_fn` you pass to
  `hashid_minter_mint`, with no store handle, no vtable and no sink, because
  minting asks exactly one question. The set is yours, and the library neither
  holds a copy nor writes to it.
- **`spec/hashid-spec.md`** — the format specified independently of any
  implementation: grammar, seed encoding byte by byte, digest, adaptive length,
  escalation ladder, resolution, error taxonomy. Every rule carries a stable
  identifier.
- **`spec/vectors.json`** — 193 conformance vectors in 22 groups, each naming
  the rules it pins, in witness/near-miss pairs wherever a rule draws a line.
  Hand-authored from the specification and checked in, *not* emitted from the
  implementation: a golden file the code writes for itself can never fail. Run
  by `cargo test -p hashid-conformance`; on their first run together the vectors
  were wrong twice and the code was right.
- **`spec/binding-guide.md`** — the three routes to a binding, and the four
  mistakes that produce IDs which look right and are not.
- The vectors run **through the generated bindings**, not only against the Rust
  core: `bindings/python/test_conformance.py` puts 114 of the 193 across the
  real UniFFI boundary. A suite that only ever ran against the core would be
  re-checking what Gherkin, property and golden tests already cover, while
  leaving ungated the one place a drifting re-implementation lives. It also
  keeps the API cut honest — the groups whose functions were removed are checked
  through the one-liners the docs prescribe.
- **`tools/check-c-abi.sh`** and **`tools/check-python-bindings.sh`** — the
  gates on each boundary. The first compares the header's declarations against
  the library's exported symbols before running 48 C assertions, because C does
  not check a declaration against its definition across an ABI.
- `seed`, `digest` and `base36` are now re-exported at the crate root, as
  `parse`'s and `resolve`'s free functions already were. They *are* the wire
  format; anyone reimplementing `hashid` has to be able to reach them.

### Fixed

- **`digest(input, 0)` panicked.** A `Base36Hash` cannot be empty, so zero
  characters had nothing to return, and the function's own "# Panics: Never" was
  untrue. Reachable from safe public API — and across an FFI boundary a Rust
  panic is a host-process abort, not an exception.

### Fixed

- **An unrepresentable instant was silently hashed as zero.** `Request` held a
  `DateTime<Utc>`, and `seed` read it through `timestamp_nanos_opt().unwrap_or(0)`.
  A `DateTime<Utc>` can hold instants that have no nanosecond representation —
  before 1677-09-21 or after 2262-04-11 — and every one of them hashed as the
  same zero. Two records a century apart would have minted the same ID, and
  nothing would have said so.

  `Request` now stores validated nanoseconds and keeps the field private.
  `Request::at(DateTime)` is fallible and returns the new
  `IdError::TimestampOutOfRange`; `Request::at_nanos(i64)` is total, because
  every `i64` is a representable instant. That is also what the boundaries now
  hand over, so neither binding converts a date type at all.

- **`hashid-ffi` reported a timestamp failure as a store failure.** The
  `From<IdError>` conversion had a catch-all folding every unrecognised variant
  into `HashidError::Store`, which would have sent a caller to inspect their
  database over someone else's bug. Unrecognised variants now surface as the
  new `HashidError::Unknown`, mirroring the C ABI's long-standing
  `HASHID_ERR_UNKNOWN`.

### Changed

- **Breaking: no conversion across a boundary clamps.** Every narrowing that
  could lose a value returned a saturated one: a negative population became
  zero, a nonce of `-1` became four billion, an over-large hash length became
  `usize::MAX`. Each turned a caller's mistake into a plausible number and said
  nothing about it — the same failure this library exists to prevent, one layer
  down.

  `hashid-ffi` gains `HashidError::OutOfRange { field, value }` and `hashid-c`
  gains `HASHID_ERR_OUT_OF_RANGE = 15`; every conversion that can fail now
  returns one, naming the field. Widenings that cannot fail are written as
  total functions. What remains saturating is confined to the core, provably
  unreachable, and documented at the line.

- **`uniffi` 0.29 → 0.32.** The Java backend, `uniffi-bindgen-java`, is built
  against `uniffi_bindgen` 0.32 and could not read a 0.29 library's metadata at
  all — it failed while extracting it, which is the same version-skew the
  binding docs warn about. Java now generates, compiles and runs.

  One caller-visible consequence: 0.32 requires a foreign callback
  implementation to subclass the generated interface rather than merely
  duck-type it, so a Python `IdStore` is now `class MyStore(hashid.IdStore)`.

- **Breaking: every number crossing `hashid-ffi` is signed.** Rust's `u32` and
  `u64` render as Kotlin's `UInt` and `ULong`, which are inline value classes.
  The JVM mangles the name of any method mentioning one, so `mintUnchecked` was
  not callable from Java and `Request` was not constructible from it. Nothing
  this binding exposes can reach `i64::MAX`, so signing costs a range nobody
  can use and buys an API that Java — and every other language without unsigned
  types — can actually call.

  `Id.child_path` crosses as `i64`, not `i32`: the grammar allows a segment up
  to `u32::MAX`, and `i32` clamped it silently. `spec/vectors.json` caught that
  on the first run — `PARSE-008` exists as a witness for exactly that ceiling.

- **Breaking: `hashid` mints, and does nothing else.**

  The crate ships no store, no port trait, and no resolution. What is left is
  `Minter`, `Request`, the value objects, `parse_id`, and `StoreError`.

  **Minting takes a closure.** `Minter::mint(&request, |id| …)` replaces
  `IdGenerator::mint(&request, &registry)`; the `port` module, `IdRegistry` and
  `IdIndex` are gone, and so is the `adapter` module with `InMemoryIds`,
  `FnRegistry` and `FnIndex`. A trait for one method, with one shape of
  implementation, was ceremony — the closure adapters existed only to undo it.
  Against a real store the closure is one line over a method you already have,
  which is what the upstream this was extracted from always did.

  A store shipped with a library is worse than ceremony: it is a hash set that
  ends up in production holding a set which stopped fitting in memory two
  releases ago. Yours is the only thing that knows where the IDs really live.

  **Resolution moved to `hashid-resolve`.** Minting asks *is this taken?*,
  which any store answers with an indexed lookup. Resolution needs a search.
  Shipping them together meant every caller who mints met a search port they
  would never implement — and had to be told, in three separate documents, that
  raising from it was an honest thing to do. `IdError::Ambiguous` and
  `NotFound` moved with it, into `ResolveError`.

  Its fragment port also changed shape: `candidates` is now **best-effort
  narrowing**, and the crate applies the matching rule to what comes back.
  Over-returning costs only time, so `select id from ids` is a correct
  implementation, which is what the upstream SQLite store actually does. The
  old contract made the adapter apply a rule with a sharp edge — a fragment
  carrying a child path matches the hash loosely but the child path exactly —
  and an adapter that got it wrong produced a confident wrong answer rather
  than an error.

  **`Minter::new(prefix)`** normalizes leniently and is the constructor most
  callers want; `Minter::with_config` takes an `IdConfig` for a non-default
  entropy policy. `Request::new(title)` defaults the instant to now and
  `population` to zero — a wrong population costs extra lookups, never
  correctness, because every candidate is still checked.

  **`RegistryError` is `StoreError`**, and `IdError::Registry` is
  `IdError::Store`.

  The same cut runs through both bindings. `hashid-ffi` exports `Minter` and a
  one-method `IdStore`; `Generator`, `Resolver`, `ResolvedId`, `ResolverOptions`,
  `MatchKind` and `matches_fragment` are gone. `hashid-c` drops the store
  handle, the vtable, the fragment sink and the resolver for a single
  `hashid_contains_fn` passed straight to `hashid_minter_mint` — 23 exported
  functions down to 16.

  Status codes 3 and 4 (`HASHID_ERR_AMBIGUOUS`, `HASHID_ERR_NOT_FOUND`) are
  retired rather than reused: a number that changes meaning is how an old binary
  and a new header come to disagree silently.
- **Breaking:** hash lengths are now a `HashLength`, a value in `1..=40`, rather
  than a bare `usize`. This affects `digest`, `IdGenerator::mint_unchecked`,
  `IdGenerator::hash_length_for`, and `IdConfig`'s two length accessors.
  `HashLength::new` is fallible; everything inside the crate is then total by
  construction, and each boundary converts once, at the edge. The alternative —
  documenting the panic — leaves every binding to re-derive the same check, and
  a boundary that re-decides a domain rule is one that will disagree with it.
- **Breaking:** new `IdError::InvalidHashLength` variant, distinct from
  `InvalidHashBounds`: one reports an unsatisfiable *range*, the other a single
  unrenderable length. `IdError` is `#[non_exhaustive]`, so matches still
  compile.
- The repository is now a cargo workspace, with the core crate at
  `crates/hashid`. Its dependency graph, feature set and published contents are
  unchanged; the adapters depend on it and nothing depends on them, which is
  what lets it keep `unsafe_code = "forbid"`.

## [0.1.0] — 2026-07-10

Initial release. Extracted from
[beads_rust](https://github.com/Dicklesworthstone/beads_rust) (`src/util/id.rs`),
where the scheme has been minting issue IDs in production.

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
