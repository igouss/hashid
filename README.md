# hashid

Content-addressed, human-typeable short IDs — in Rust, and in every language
that can call Rust.

```text
br-8cda
myproj-survey-the-users-3f9k1
br-8cda.1.2
```

Short enough to retype from a terminal. Stable enough to put in a commit
message. Unique enough to key a database on.

A UUID is unique and unusable: nobody retypes `f47ac10b-58cc-4372-a567-0e02b2c3d479`
into a terminal, and it tells you nothing about what it names. A counter is
typeable but needs a central allocator, which is exactly what you do not have
when two branches, two laptops, or two agents are minting concurrently. `hashid`
sits between them: IDs are derived from the content they name, so they need no
coordination; they are three to eight base36 characters, so a human can retype
one; and they carry your namespace, so you can tell at a glance what one is.

## Pick your language

| Language | How |
|---|---|
| **Rust** | `cargo add hashid` — [crate docs](crates/hashid/README.md) |
| **Kotlin, Swift, Python, Ruby** | generate from [`hashid-ffi`](crates/hashid-ffi/README.md) with UniFFI |
| **Java, Go, C#, Dart, React Native** | the same, via a third-party UniFFI backend |
| **C, C++, Zig, Objective-C** | link [`hashid-c`](crates/hashid-c/README.md) and include [`hashid.h`](crates/hashid-c/include/hashid.h) |
| **Node, PHP, Lua, R, Julia, …** | the C ABI, through your language's FFI |
| **Anything else** | implement it: [`spec/`](spec/) specifies the format completely, with 193 conformance vectors |

UniFFI has no C backend — its scaffolding is a C ABI, but an internal one with
no header and no stability promise — which is why `hashid-c` is hand-written.

```sh
# Kotlin / Swift / Python / Ruby
cargo build -p hashid-ffi --release
cargo run --bin uniffi-bindgen -- generate \
    --library target/release/libhashid_ffi.so --language python --out-dir out/

# C and everything that speaks C
cargo build -p hashid-c --release     # libhashid_c.{so,a}
```

[`spec/binding-guide.md`](spec/binding-guide.md) walks through all three routes,
including the four mistakes most likely to produce IDs that look right and are
not.

## A small API

Minting an ID takes a prefix, a title, and a lookup:

```python
minter = hashid.Minter("myproj")
id = minter.mint(hashid.Request(title="Ship the thing",
                                created_at_nanos=hashid.now_nanos()), store)
```

Both bindings expose the same small surface: four free functions — `parse_id`,
`is_valid_id`, `normalize_prefix`, `now_nanos` — a minter, and one store
interface with one method.

Neither ships a store, and neither does the Rust core, where the lookup is a
plain closure rather than an interface at all. A store shipped with a library is
a hash set that ends up in production holding a set which stopped fitting in
memory two releases ago; yours is the only thing that knows where the IDs really
live. Answering *is this ID taken?* is the whole integration — one indexed
lookup, and against a real database it is one line over a method you already
have.

Everything derivable from `parse_id` is deliberately absent. `Id` carries
`prefix`, `hash` and `child_path`, so an ID's depth, its root-ness, its parent
and its n-th child are each one line in your own language. An export you have
to look up costs more than the line it saves, and
`bindings/python/test_hashid.py` keeps that claim honest by writing out every
one of them.

## What an ID is

```text
br - survey-the-users - 3f9k1 . 2
^^   ^^^^^^^^^^^^^^^^   ^^^^^   ^
│    │                  │       └─ child path (optional, hierarchical)
│    │                  └───────── base36 hash, adaptively sized
│    └──────────────────────────── slug (optional, human-readable)
└───────────────────────────────── prefix (your namespace)
```

The hash is **content-addressed**: your title, description, creator, and
creation timestamp are length-prefixed, SHA-256'd, and rendered in base36.
Identical content at an identical instant yields an identical ID, in every
language.

Its **length adapts** to how many IDs you already have, via a birthday bound: a
young dataset gets three characters, a dataset of a million has silently widened
to eight. You never pick a length.

Uniqueness is **not left to the hash**. Every candidate is checked against your
store, and on collision the minter escalates — more nonces, then a longer
hash, then a wider fallback. If it genuinely runs out it returns an error; it
will not hand back an ID it knows is taken.

## Repository layout

```text
crates/hashid              minting. The domain: no I/O, no unsafe, three deps.
crates/hashid-resolve      fragment → id, for a CLI. Depends on the core.
crates/hashid-ffi          UniFFI adapter → Kotlin, Swift, Python, Ruby, Java.
crates/hashid-c            C ABI adapter → C, and everything that speaks C.
crates/hashid-conformance  runs spec/vectors.json against the implementation.
spec/                      the language-independent format specification.
bindings/                  per-language smoke tests.
tools/                     the verification scripts.
```

Minting and resolution ship apart because they ask different things of your
store. Minting asks *is this taken?*, which any store answers with an indexed
lookup. Resolution needs a search. A caller who only mints — the common case,
and what the library is named after — never writes the search.

The adapters depend on the core; nothing depends on an adapter. That direction
is enforced by the crate graph rather than hoped for in review, which is also
what lets `hashid` itself keep `unsafe_code = "forbid"` while the C ABI does
nothing but `unsafe`.

## The specification

[`spec/hashid-spec.md`](spec/hashid-spec.md) specifies the format completely and
independently of any implementation: the grammar, the seed encoding byte by
byte, the digest, the adaptive length, the escalation ladder, resolution, and
the error taxonomy. Every rule has a stable identifier.

[`spec/vectors.json`](spec/vectors.json) is its machine-checkable half — 193
vectors in 22 groups, each naming the rules it pins. Vectors come in
witness/near-miss pairs wherever a rule draws a line, so the boundary is pinned
and not merely the happy path.

The vectors are **hand-authored and checked in**, not emitted from the
implementation. A golden file the code writes for itself can never fail, and a
suite that can never fail is not a gate. When the two disagree, that is a
finding either way — and on their first run together, the vectors were wrong
twice and the code was right.

They run against the Rust core **and through the generated bindings**: 114 of
the 193 cross the real UniFFI boundary. That matters, because the boundary is
where a drifting re-implementation actually lives — a vector suite that only
ever ran against the core would be checking the one thing already covered by
Gherkin, property and golden tests. The 79 it does not reach pin the wire format
and minting internals, which are not a binding's job; the runner names them, and
fails if a new group is neither reached nor named.

## Verifying

```console
$ cargo test --workspace          # Rust: Gherkin, properties, golden vectors, conformance
$ tools/check-c-abi.sh            # C: header/library symbol agreement, then 68 assertions
$ tools/check-python-bindings.sh  # UniFFI: generate bindings, then run the vectors through them
```

`check-c-abi.sh` exists because C does not check a declaration against its
definition across an ABI: a header that has drifted still compiles, and then
reads the wrong bytes.

## License

MIT OR Apache-2.0
