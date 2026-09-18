# hashid-resolve

Turn what a human typed back into the [`hashid`](../hashid) they meant.

Short IDs exist to be retyped, and nobody retypes the whole thing. Given
`br-8cda9f21`, a human types `8cda`. This crate turns that back.

```rust
use hashid::Id;
use hashid_resolve::{MatchType, Resolver};

let known: Vec<Id> = ["br-abc123", "br-xyz789"]
    .iter()
    .map(|raw| Id::parse(raw).expect("well-formed id"))
    .collect();

let resolver = Resolver::new("br").expect("usable prefix");

let resolved = resolver.resolve(
    "xyz",
    |id: &Id| Ok(known.contains(id)),
    |_fragment: &str| Ok(known.clone()),
).expect("resolves");

assert_eq!(resolved.id.to_string(), "br-xyz789");
assert_eq!(resolved.match_type, MatchType::Substring);
```

## Four steps, in order

| Step | Input | Result |
| --- | --- | --- |
| 1 | a complete existing ID | `Exact` |
| 2 | a bare hash | `PrefixNormalized` — the default prefix is prepended |
| 3 | a fragment of a hash matching exactly one ID | `Substring` |
| 4 | several matches, or none | `Ambiguous` or `NotFound` |

An ambiguous fragment is an error carrying *every* candidate, sorted, so a CLI
can print the choice rather than guess.

## The two lookups

`exists` is an indexed point lookup — `select 1 from ids where id = ?`.

`candidates` is **best-effort narrowing**. Return anything that might match and
this crate applies the fragment rule to what comes back, so over-returning costs
only time. Every one of these is correct:

```sql
select id from ids where hash like '%' || ? || '%'   -- has a hash column
select id from ids where id   like '%' || ? || '%'   -- has only the id text
select id from ids                                    -- small table
```

Under-returning is the only way to get a wrong answer, and it is the one thing a
store cannot be forgiven for: a candidate you omit is a match this crate will
never see.

That split is deliberate. The fragment rule has a sharp edge — a fragment
carrying a child path (`64up6.4`) matches the hash loosely but the child path
**exactly**, or else searching for `64up6.4` returns the parent `br-64up6` and
every sibling. Keeping that rule here rather than in the contract means no
adapter can get it wrong.

## Both lookups are fallible

An unreachable database and a genuinely absent ID are not the same answer.
`ResolveError::Store` and `ResolveError::NotFound` are separate variants, and a
caller that deletes on "not found" would very much like to know the difference.

## Why this is a separate crate

Minting asks *is this ID taken?*, which any store answers with an indexed
lookup. Resolution needs a search. Keeping them apart means a caller who only
mints — the common case, and what `hashid` is named after — never meets the
search, and never implements a lookup it will not use.

## Testing

`features/resolution.feature` is the behavioural source: 17 scenarios, 168
steps, run through `cucumber`. The store behind them deliberately returns
*everything* from `candidates`, applying no filter at all, which is what proves
this crate applies the fragment rule itself.

```console
$ cargo test -p hashid-resolve
```

## License

MIT OR Apache-2.0
