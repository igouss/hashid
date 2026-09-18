# hashid-ffi

`hashid` for Kotlin, Swift, Python, Ruby, Java, and anything else
[UniFFI](https://github.com/mozilla/uniffi-rs) reaches.

Content-addressed, human-typeable short IDs — `br-8cda`,
`myproj-survey-the-users-3f9k1`, `br-8cda.1.2`. See the
[repository README](../../README.md) for what they are and why.

## Generating bindings

```sh
cargo build -p hashid-ffi --release

cargo run --bin uniffi-bindgen -- generate \
    --library target/release/libhashid_ffi.so \
    --language kotlin \
    --out-dir out/kotlin
```

`--language` is `kotlin`, `swift`, `python` or `ruby`. Third-party backends —
`uniffi-bindgen-java`, `-go`, `-cs`, `-cpp`, `-react-native` — install with
`cargo install` and take the same library. They must be built against the same
`uniffi` version this crate pins (`0.32`); an older generator cannot read the
metadata and fails while extracting it, which is the same version-skew this
section's next paragraph warns about.

For Java, `uniffi-bindgen-java` takes the library positionally rather than via
`--library`, and needs no JNA — it uses the JVM's Foreign Function & Memory
API:

```sh
cargo install uniffi-bindgen-java
uniffi-bindgen-java generate target/release/libhashid_ffi.so --out-dir out/java

javac -d classes $(find out/java -name '*.java') YourCode.java
java --enable-native-access=ALL-UNNAMED -cp classes -Djava.library.path=. YourCode
```

Ship the generated source **and** the shared library for each platform. They are
a matched pair: UniFFI embeds an interface checksum in both, so a mismatch fails
at load rather than quietly misreading the ABI.

## Using it

This binding ships no store. `IdStore` is the one question it asks of yours, and
minting is one call:

```python
import hashid

class MyStore(hashid.IdStore):       # an IdStore, implemented in Python
    def __init__(self):
        super().__init__()
        self.ids = set()

    def contains(self, id):
        return id in self.ids

store  = MyStore()
minter = hashid.Minter("myproj")     # prefix normalized for you

request = hashid.Request(title="Ship the thing",
                         creator="iouri",
                         created_at_nanos=hashid.now_nanos(),
                         population=len(store.ids))

id = minter.mint(request, store)     # "myproj-8cda"
store.ids.add(id)
```

Against a real store `contains` is one line over a query you already have —
`select 1 from ids where id = ?`. A `MinterConfig` is available via
`Minter.with_config` when you want a non-default entropy policy, but a prefix is
all a minter needs.

## How small it is

Four free functions — `parse_id`, `is_valid_id`, `normalize_prefix`,
`now_nanos` — one object (`Minter`) and one interface with one method
(`IdStore`).

Everything derivable from `parse_id` is deliberately absent, because an export
you have to look up costs more than the line it saves. `Id` carries `prefix`,
`hash` and `child_path`:

```python
id = hashid.parse_id("br-abc123.1.2")

depth  = len(id.child_path)                                     # 2
root   = not id.child_path                                      # False
parent = id.text.rsplit(".", 1)[0] if id.child_path else None   # br-abc123.1
child  = f"{id.text}.7"                                         # br-abc123.1.2.7
```

`bindings/python/test_hashid.py` has a `TestDerivable` case that writes out
every one of these. If one of them stops being a one-liner, the cut was wrong.

## Two things to know

**IDs cross as strings, not objects.** `br-8cda` is already the most portable
representation there is, and the API re-parses on the way in — so an ID reaching
the core is well-formed no matter what the caller assembled. `parse_id` takes
one apart when you need the pieces.

**Instants cross as nanoseconds since the Unix epoch**, not as a language-native
date. That number is hashed verbatim: two implementations that round it
differently mint different IDs for the same record, and neither can tell.
`now_nanos()` gives you one.

**Every number crosses signed.** Rust's `u32` and `u64` render as Kotlin's
`UInt` and `ULong`, which are inline value classes — the JVM mangles the name of
any method mentioning one, so `mintUnchecked` would not be callable from Java at
all, and `Request` would not be constructible. Nothing this binding exposes can
reach `i64::MAX`, so signing costs a range nobody can use. A child-path segment
crosses as `i64` rather than `i32` because the grammar allows it up to
`u32::MAX`, which an `i32` would silently clamp.

## Your store is the only store

There is no built-in one, and that is deliberate. A store shipped with a binding
is a hash set that ends up in production holding a set which stopped fitting in
memory two releases ago — and every language UniFFI reaches already has a better
one built in. `IdStore` is one method:

```kotlin
class PostgresStore(private val db: DataSource) : IdStore {
    override fun contains(id: String): Boolean =
        try { db.queryExists("select 1 from ids where id = ?", id) }
        catch (e: SQLException) { throw StoreException.Failed(e.message ?: "lookup failed") }
}

minter.mint(request, PostgresStore(db))
```

In Rust there is not even an interface — `hashid` takes a closure, because one
question does not need one. It is an interface here only because no foreign ABI
can carry a Rust closure.

### The method is fallible on purpose

The `throw` above is the part that matters. If the interface could not say "I do
not know", a store talking to a real database would have to pick a lie when the
connection drops: answer `false` and invent an ID that may already exist, or
answer `true` and burn a good candidate. Both are silent, and the first corrupts
data. A binding that swallows the exception and returns `false` has quietly
removed the guarantee the library is for.

`contains` is called once per candidate, so make it cheap. The minter only asks
again when a candidate is taken, so a store answering from an index answers
about once per ID minted.

### `population` wants an estimate, not a count

It picks the starting hash length via a birthday bound, and defaults to zero.
Getting it wrong costs extra lookups, never correctness — every candidate is
still checked. Being out by a factor of two moves the length by at most one
character, so a cached or approximate row count is the right thing to pass, not
a `select count(*)` per mint.

## Resolution is not here

Turning a typed fragment like `8cd` back into `myproj-8cda` lives in
[`hashid-resolve`](../hashid-resolve), and this binding does not cover it.
Minting asks *is this taken?*, which any store answers with an indexed lookup;
resolution needs a search. They ship apart so that a caller who only mints never
implements a lookup they will not use.

## Why this is a separate crate

`hashid` itself keeps `unsafe_code = "forbid"` and three dependencies. This
crate holds the `extern "C"` scaffolding UniFFI generates, so the domain never
has to, and contains no rules of its own — every decision it appears to make is
delegated to the core. A boundary that re-decides a domain rule is a boundary
that will eventually disagree with it.

For a language UniFFI does not reach, [`crates/hashid-c`](../hashid-c) exposes a
plain C ABI and [`spec/`](../../spec) specifies the format completely.

## Verifying

```console
$ tools/check-python-bindings.sh
```

Generates the bindings and runs 20 tests through them, including an `IdStore`
implemented in Python and the store-failure path, then runs 86 of the 193
specification vectors across the real boundary.

## License

MIT OR Apache-2.0
