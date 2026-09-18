# hashid

Content-addressed, human-typeable short IDs for Rust.

```text
br-8cda
myproj-survey-the-users-3f9k1
br-8cda.1.2
```

Short enough to retype from a terminal. Stable enough to put in a commit
message. Unique enough to key a database on.

```rust
use std::collections::HashSet;
use hashid::{Id, Minter, Request};

let mut known: HashSet<String> = HashSet::new();
let minter = Minter::new("myproj").expect("a usable prefix");

let request = Request::new("Ship the thing")
    .with_creator("iouri")
    .with_population(known.len());

// `Request::at(DateTime)` pins the instant instead of taking now, and is
// fallible: an instant outside 1677..=2262 has no nanosecond representation,
// and hashing one as zero would mint a single colliding ID for all of them.

// You bring the store. The crate asks it one question and never opens it.
let id = minter
    .mint(&request, |id: &Id| Ok(known.contains(&id.to_string())))
    .expect("minting succeeds");

assert!(id.to_string().starts_with("myproj-"));   // myproj-8cda
known.insert(id.to_string());
```

Against a real store that closure is one line over a method you already have:

```text
minter.mint(&request, |id| db.id_exists(&id.to_string()))?
```

There is no trait to implement and no adapter to write, because one question
does not need an interface.

## From another language

This crate is the Rust core. Bindings for Kotlin, Swift, Python, Ruby and Java
are generated from [`hashid-ffi`](../hashid-ffi); C and everything that speaks C
link [`hashid-c`](../hashid-c); and [`spec/`](../../spec) specifies the format
completely, with conformance vectors, for anything else. The
[repository README](../../README.md) has the table.

## Why not a UUID, or a counter?

A UUID is unique and unusable: nobody retypes `f47ac10b-58cc-4372-a567-0e02b2c3d479`
into a terminal, and it tells you nothing about what it names. A counter is
typeable but needs a central allocator, which is exactly what you do not have
when two branches, two laptops, or two agents are minting concurrently.

`hashid` sits between them. IDs are derived from the content they name, so they
need no coordination to mint; they are three to eight base36 characters, so a
human can retype one; and they carry your namespace, so you can tell at a glance
what a `br-` is versus a `myproj-`.

## Anatomy

```text
br - survey-the-users - 3f9k1 . 2
^^   ^^^^^^^^^^^^^^^^   ^^^^^   ^
│    │                  │       └─ child path   (optional, hierarchical)
│    │                  └───────── base36 hash  (adaptively sized)
│    └──────────────────────────── slug         (optional, human-readable)
└───────────────────────────────── prefix       (your namespace)
```

The prefix may itself contain hyphens, so an ID splits at its **last** hyphen.
That is what lets `bead-me-up-3e9` keep the prefix `bead-me-up`, and
`external:jira-123` keep `external:jira`.

## How the hash is built

1. **Seed.** Your `title`, `description`, `creator`, `created_at` (nanoseconds)
   and a nonce are concatenated, each written as `<byte-length>:<value>`.

   The length prefix is not decoration. A naive `title|description` join lets a
   caller move the field boundary — title `a|b` with no description would seed
   exactly as title `a` with description `b` — so two genuinely different
   records could be made to collide *on demand*. Length-prefixing makes the
   boundaries unforgeable.

2. **Digest.** SHA-256, then the leading 8 bytes as a `u64`, then base36.

3. **Truncate.** Keep the **last** `n` characters, not the first. The
   least-significant base36 digits vary fastest; the leading digits of a `u64`
   barely move, so truncating from the head would throw away most of the
   entropy at short lengths.

4. **Size.** `n` is not fixed. It comes from a birthday bound against the
   population you already have:

   ```text
   P(collision) ≈ 1 - e^(-population² / 2·36ⁿ)
   ```

   The shortest `n` keeping `P` under 25% (configurable) wins. In practice: a
   few hundred records get 3 characters, ten thousand get 6, a million get 8.
   You never pick a length.

Because the seed is content plus timestamp, minting is **deterministic**: the
same content at the same instant always yields the same ID. The timestamp is
what keeps two identically-titled records apart.

## Collisions are handled, not hoped away

A 25% birthday bound sounds alarming until you notice the hash is not what
guarantees uniqueness — the registry check is. Every candidate is checked
against your store, and on collision the generator escalates:

| Tier | Strategy                                     |
| ---- | -------------------------------------------- |
| 1    | Ten nonces at the adaptive length            |
| 2    | Widen the hash by one character, repeat      |
| 3    | Fall back to a 12-character hash, walk the nonce |
| 4    | Past a thousand of those, splice the nonce into the hash too |

If the ladder genuinely runs out, minting returns `IdError::Exhausted`. It does
**not** return an ID it has been told is taken. A duplicate ID is silent data
corruption; an error is merely loud.

## Resolving what a human typed

Turning a typed fragment like `8cd` back into `myproj-8cda` lives in
[`hashid-resolve`](../hashid-resolve). It is a different job with a different
shape: minting asks *is this taken?*, which any store answers with an indexed
lookup, while resolution needs a search. Keeping them apart means a caller who
only mints never implements a lookup they will not use.

## Architecture

Hexagonal, dependencies pointing inward. The crate performs no I/O, has no async
runtime, no storage engine, and no `unsafe`.

```text
    domain/     Id · Prefix · Slug · Base36Hash · HashLength · IdConfig · Request
                correct by construction — you cannot hold a malformed one
       ▲
       │
    generate/   seed → digest → length → escalation ladder
    parse/      the trust boundary: text in, validated Id out
       ▲
       │
    store.rs    StoreError ← the one thing the crate needs from you
```

There is no port module and no adapter module, and that is the point. Minting
asks one question, so it takes a closure — a trait for a single method, with a
single shape of implementation, is ceremony. A store shipped with a library is
worse: a `HashSet` that ends up in production holding a set that stopped fitting
in memory two releases ago.

The lookup is **fallible** on purpose. An unreachable database and a genuinely
free ID are not the same answer, and if the closure could not say so it would
have to pick a lie: return `false` and invent an ID that may already exist, or
return `true` and burn a good candidate. Both are silent, and the first corrupts
data.

## Testing

- **`features/*.feature`** — 73 executable scenarios (Gherkin, via `cucumber`).
  The specs *are* the behavioural test source.
- **`tests/properties.rs`** — the invariants, including the one that matters:
  minting never returns an ID the store already holds, for zero, one, or many
  pre-existing IDs.
- **`tests/conformance.rs`** — golden vectors freezing the wire format. A
  refactor that quietly changed the seed encoding or the digest truncation would
  keep every behavioural test green while silently orphaning every ID ever
  minted. These catch that.
- **[`spec/vectors.json`](../../spec/vectors.json)** — 193 language-independent
  vectors, run by `cargo test -p hashid-conformance`. Unlike the golden file
  above, these are hand-authored from the specification rather than captured
  from a run, so they can disagree with the implementation — and did, twice, the
  first time the two met.

```console
$ cargo test
```

### Mutation testing

The suite is checked with [`cargo-mutants`](https://mutants.rs), which rewrites
the source and asserts the tests notice. It is the only way to find out whether a
test *constrains* behaviour or merely *executes* it — the first run caught 65% of
mutants, and every hole it exposed was a real one (the child-path fragment rule,
every `IdConfig` validation boundary, `is_valid_id`, `resolve_all`, the prefix
budget's off-by-one). It also found dead code: an unreachable guard in
`Prefix::abbreviated`, and a redundant one in the resolver.

```console
$ cargo mutants
```

A handful of mutants survive on purpose, because they are **equivalent** — the
mutated program cannot behave differently from the original:

| Survivor | Why it cannot be killed |
| --- | --- |
| `digest`: `\|` → `^` | The low byte is always zero after `<< 8`, so OR and XOR agree. |
| `digest`: `len < n` → `<=` | Zero-padding a string to its own length is a no-op. |
| `optimal_length`: `p < max` → `<=` | Would need two `f64`s to compare exactly equal. |
| `Slug::normalize`: `len > 48` → `>=` | Trailing hyphens are already trimmed, so truncating to the current length changes nothing. |

## Provenance

Extracted from [beads_rust](https://github.com/Dicklesworthstone/beads_rust)
(`src/util/id.rs`), where this scheme has been minting issue IDs in anger. The
algorithm is preserved **byte for byte** — `hashid` reproduces the same IDs from
the same inputs, and `tests/conformance.rs` pins that against golden vectors
taken from the original. The API around it was rebuilt:

| Original                                        | Here                                                             |
| ----------------------------------------------- | ---------------------------------------------------------------- |
| IDs are `String`; `ParsedId` is a separate type | One `Id` type, parsed, `Hash + Eq + Ord`, cannot hold a bad value |
| `exists: Fn(&str) -> bool`, infallible          | the same closure, but **fallible** — no more smuggling errors out via `RefCell` |
| Exhaustion returns a known-duplicate ID         | Exhaustion returns `IdError::Exhausted`                          |
| `generate_id()` silently skips collision checks | `mint_unchecked()` — the name is the contract                    |
| `issue_count` is an easily-forgotten positional | `population` defaults, and a wrong one costs lookups, not correctness |
| `InvalidId { id }` for every grammar violation  | `MalformedReason` names the rule that broke                      |
| Bad prefix silently becomes `"br"`              | `IdError::UnusablePrefix` — no inventing a namespace you didn't pick |
| `resolve` and `resolve_fallible`, duplicated    | One fallible `resolve`, in [`hashid-resolve`](../hashid-resolve) |

## License

MIT OR Apache-2.0
