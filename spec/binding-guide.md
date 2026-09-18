# Writing a hashid binding

Three routes. Pick by what your language can already do.

| Your language | Route | Effort |
|---|---|---|
| Kotlin, Swift, Python, Ruby | **A — UniFFI**, first-party backend | one command |
| Java, Kotlin Multiplatform, Go, C#, Dart, React Native | **A — UniFFI**, third-party backend | one command, one extra tool |
| C, C++, Zig, Objective-C | **B — the C ABI** | link and include |
| Anything with an FFI but no UniFFI backend (Node, PHP, Perl, Lua, R, Julia…) | **B — the C ABI**, through your FFI layer | a day |
| Pure JavaScript/TypeScript, or any runtime where a native library is unwelcome | **C — reimplement** | a day, and the vectors tell you when you are done |

Whichever route you take, [`vectors.json`](vectors.json) is how you find out
whether it works. Route C *needs* it; routes A and B should still run it,
because a binding that marshals a string wrongly fails exactly one vector and
otherwise looks fine.

`bindings/python/test_conformance.py` is a worked example of doing that through
generated bindings — 114 of the 193 vectors, with the groups a binding cannot
reach named explicitly rather than quietly skipped.

---

## Route A — UniFFI

`crates/hashid-ffi` is a UniFFI component. Generating bindings is one command.

```sh
# 1. Build the shared library.
cargo build -p hashid-ffi --release

# 2. Generate bindings. --language is one of: kotlin, swift, python, ruby.
cargo run --bin uniffi-bindgen -- generate \
    --library target/release/libhashid_ffi.so \
    --language kotlin \
    --out-dir out/kotlin
```

Ship the generated source **and** the shared library for each platform you
support (`.so` on Linux, `.dylib` on macOS, `.dll` on Windows). They are a
matched pair: UniFFI embeds a checksum of the interface in both, and a mismatch
fails loudly at load rather than quietly misreading the ABI.

### Third-party backends

Separate tools that consume the same `--library` argument. Only one is on
crates.io; the rest install from their repositories, so check the current
install line there rather than trusting this table:

| Language | Tool | Install |
|---|---|---|
| Java | `uniffi-bindgen-java` | `cargo install uniffi-bindgen-java` (needs the same `uniffi` version) |
| Go | `uniffi-bindgen-go` | `cargo install --git` (NordSecurity) |
| C# | `uniffi-bindgen-cs` | `cargo install --git` (NordSecurity) |
| C++ | `uniffi-bindgen-cpp` | `cargo install --git` (NordSecurity) |
| React Native | `uniffi-bindgen-react-native` | npm (Mozilla) |

**There is no C backend, first-party or third-party.** UniFFI's scaffolding is
a C ABI, but an internal one — `RustBuffer` structs, status out-parameters, no
header, and no stability promise across versions. For C, take route B.

Java is also reachable through the Kotlin backend, since Kotlin output is
ordinary JVM bytecode. Prefer `uniffi-bindgen-java` if you want an API that
reads as Java rather than as Kotlin seen from Java; it takes the library
positionally rather than via `--library`, and uses the JVM's Foreign Function &
Memory API, so it needs no JNA.

A generator built against a different `uniffi` version than the scaffolding
cannot read the library's metadata at all — it fails while extracting it, not
at runtime. Match the versions.

### How small the API is

Four free functions — `parseId`, `isValidId`, `normalizePrefix`, `nowNanos` —
one object (`Minter`), and one interface with one method (`IdStore`).

Resolution — turning a typed fragment back into an ID — is not part of the
minting bindings. It asks a different thing of your store: minting asks *is this
taken?*, which any store answers with an indexed lookup, while resolution needs
a search. See `crates/hashid-resolve`.

There is no built-in store, and that is deliberate: a store shipped with a
binding is a hash set that ends up in production holding a set which stopped
fitting in memory two releases ago. Yours is the only one that knows where the
IDs really live.

Everything derivable from `parseId` is deliberately absent, because an export
you have to look up costs more than the line it saves. `Id` carries `prefix`,
`hash` and `childPath`, so an ID's depth is `childPath.size`, a root is
`childPath.isEmpty`, its n-th child is `"$id.$n"`, and its parent is the text
up to the last `.`.

### What the generated API looks like

```kotlin
val store  = MyStore()              // your IdStore, see below
val minter = Minter("myproj")       // prefix normalized for you

val request = Request(
    title = "Ship the thing",
    creator = "iouri",
    createdAtNanos = nowNanos(),
    population = store.size,
)
val id = minter.mint(request, store)   // "myproj-8cda"
store.remember(id)                  // your store, your write
```

`mint` takes the interface, so implementing it over your database is the whole
integration — one method:

```kotlin
class PostgresStore(private val db: DataSource) : IdStore {
    override fun contains(id: String): Boolean =
        try { db.queryExists("select 1 from ids where id = ?", id) }
        catch (e: SQLException) { throw StoreException.Failed(e.message ?: "lookup failed") }

    override fun findByHashFragment(fragment: String): List<String> =
        db.queryStrings("select id from ids where hash like ?", "%$fragment%")
          .filter { matchesFragment(it, fragment) }
}
```

Note what the `catch` does: it **raises**, it does not return `false`. See
[Fallibility is not optional](#fallibility-is-not-optional).

---

## Route B — the C ABI

`crates/hashid-c` exports a plain C ABI. The header,
[`hashid.h`](../crates/hashid-c/include/hashid.h), is the normative description
of it and documents every function.

```sh
cargo build -p hashid-c --release
# target/release/libhashid_c.so     — for dlopen / LoadLibrary / JNA / ctypes
# target/release/libhashid_c.a      — for static linking into a C binary
```

```c
#include "hashid.h"

/* No built-in store here either: one callback over yours. */
static int32_t nothing_taken(void *ctx, const char *id, const char **err) {
    (void)ctx; (void)id; (void)err;
    return 0;
}

hashid_minter *minter = NULL;
hashid_minter_new("myproj", &minter);

hashid_request request = {
    .title = "Ship the thing", .description = NULL, .creator = "iouri",
    .created_at_nanos = hashid_now_nanos(), .population = 0,
};

char *id = NULL;
if (hashid_minter_mint(minter, &request, nothing_taken, NULL, &id) == HASHID_OK) {
    puts(id);
    hashid_string_free(id);
}
hashid_minter_free(minter);
```

### The three rules

1. **Status codes.** Every function returns `hashid_status`; `HASHID_OK` is 0.
   On failure, out-parameters are untouched and `hashid_last_error_message()`
   describes what happened.
2. **Ownership.** Every pointer the library returns is yours, released with the
   matching `hashid_*_free`. Every pointer you pass in is borrowed for the call.
   The one exception is `hashid_last_error_message()`, borrowed until your next
   call on that thread.
3. **No unwinding.** Every entry point catches panics and reports
   `HASHID_ERR_PANIC`, so the library cannot corrupt your stack.

### Per-language notes

- **Java (JDK 22+)** — the Foreign Function & Memory API. Model `hashid_request`
  and `hashid_config` with `MemoryLayout.structLayout`, and every `char **out`
  as an `Arena`-allocated pointer slot. Copy the returned string with
  `MemorySegment.getString` **before** calling `hashid_string_free`.
- **Java (older)** — JNA. `Structure` for the by-value structs, `PointerByReference`
  for `char **out`, and a `Callback` subinterface per vtable entry. Keep a strong
  reference to your callbacks for the store's lifetime, or the GC will collect
  them out from under the library.
- **Node.js** — write an N-API addon rather than using a runtime FFI shim: the
  store callbacks have to run on the JS thread, and N-API's thread-safe function
  is the sane way to arrange that. For a pure-JS build, take route C instead.
- **Python** — `ctypes` works directly. Declare `restype`/`argtypes` for every
  function; the default `c_int` return will silently misread on some ABIs.
- **Go** — cgo. Remember that a Go pointer may not be stored in C memory across
  a call, so pass a handle (an index into a Go-side map) as `user_data`, not a
  Go pointer.
- **C#** — P/Invoke with `[StructLayout(LayoutKind.Sequential)]`, and
  `GCHandle` to pin the delegates you put in the vtable.

### Implementing a store in C

One function pointer plus a `user_data`. There is no store handle to create and
free, and no vtable, because minting asks exactly one question:

```c
static int32_t my_contains(void *ctx, const char *id, const char **error) {
    switch (db_lookup(ctx, id)) {
        case FOUND:     return 1;
        case NOT_FOUND: return 0;
        default:        *error = "database unreachable"; return -1;
    }
}

hashid_minter_mint(minter, &request, my_contains, my_db, &id);
```

The `-1` is the point: see [Fallibility is not
optional](#fallibility-is-not-optional).

---

## Route C — reimplement from the specification

The algorithm is small and has no dependencies beyond SHA-256, which every
standard library has. A complete implementation is a few hundred lines.

Read [`hashid-spec.md`](hashid-spec.md) and implement in this order — each step
is checkable on its own before you build the next on top:

| Order | Capability | Spec | Vector group |
|---|---|---|---|
| 1 | base36 rendering | §7 | `base36` |
| 2 | the grammar: parse and render | §3 | `parse_id`, `format_id` |
| 3 | prefixes | §4 | `normalize_prefix`, `parse_prefix`, `abbreviate_prefix` |
| 4 | slugs | §5 | `normalize_slug`, `join_prefix_slug` |
|   | *(a binding need not expose 3 and 4 beyond `normalize_prefix`; a reimplementation still needs them internally, for slugged minting)* | | |
| 5 | the seed | §6 | `seed` |
| 6 | the digest | §8 | `digest` |
| 7 | adaptive length | §9 | `hash_length` |
| 8 | the deterministic mint | §11.1 | `mint_unchecked` |
| 9 | hierarchy and matching | §12, §13 | `id_parent`, `id_child`, `id_depth`, `is_descendant_of`, `matches_fragment` |
| 10 | the escalation ladder | §11.2 | `mint` |
| 11 | slugged minting | §11.3 | `mint_with_slug` |
| 12 | resolution | §14 | `resolve` |

Steps 1–8 are pure functions of their inputs, so you can get the whole wire
format right before you have a store at all. Steps 10–12 need one, and a set
held in memory is enough to run the vectors with.

### Running the vectors

`vectors.json` is a table test. Load it, iterate the groups you implement, and
compare. In JavaScript:

```js
const suite = JSON.parse(await fs.readFile("spec/vectors.json", "utf8"));

for (const v of suite.groups.digest) {
  if (v.error) {
    assert.throws(() => digest(v.input, v.length), new RegExp(v.error), v.id);
  } else {
    assert.equal(digest(v.input, v.length), v.expect, v.id);
  }
}
```

Two conventions matter:

- **Big integers are strings.** `base36` values and `created_at_nanos` exceed
  what a JSON double carries exactly, so they are decimal strings. Parse them
  with your language's 64-bit integer type — in JavaScript, `BigInt`.
- **`expect: null` is an outcome, not an absence.** For `normalize_slug`,
  `join_prefix_slug` and `id_parent`, "no value" is a legitimate, specified
  result. Do not confuse it with an error.

### The four things most likely to go wrong

Every one of these produces plausible-looking IDs that no other implementation
agrees with, which is the worst kind of bug to have. Each has a vector.

1. **Splitting the ID at the first `-` instead of the last.** `bead-me-up-3e9`
   has prefix `bead-me-up`, not `bead`. — `PARSE-002`
2. **Taking the first `length` characters of the base36 encoding instead of the
   last.** The leading digits of a 64-bit value barely vary; the tail is where
   the entropy is. — `DIG-002` through `DIG-005`
3. **Joining seed fields without length prefixes.** Title `a|b` must not seed
   the same as title `a` with description `b`. — `SEED-004` / `SEED-005`
4. **Rounding the timestamp.** It is nanoseconds, hashed verbatim. Milliseconds
   times a million is *not* the same number, and nothing will tell you. —
   `SEED-001`

### Fallibility is not optional

Whatever route you take, the store's two questions must be able to answer
"I do not know", distinctly from "no".

If they cannot, an implementation talking to a real database has to pick a lie
when the connection drops: answer "not taken" and invent an ID that may already
exist, or answer "taken" and burn a good candidate. Both are silent, and the
first corrupts data. A binding that swallows the exception and returns `false`
has quietly removed the guarantee the whole library is for.

The same applies upward: resolution must report a store failure as a store
failure, never as "not found". A caller that deletes on "not found" would very
much like to know the difference.

---

## Declaring conformance

A binding is conformant when:

- [ ] Every vector in every group it implements passes.
- [ ] The store is the caller's: the binding exports the interface — or, where
      the host language has closures, no interface at all — never an
      implementation of one.
- [ ] The store interface can report failure, distinctly from a negative answer.
- [ ] A store failure propagates out of `mint` rather than being swallowed.
- [ ] `Malformed` errors carry *which* rule broke, not just that something did.
- [ ] `Ambiguous` errors carry every match, sorted.
- [ ] "No value" outcomes — an unusable slug, a root's parent, a slug that does
      not fit — are distinguishable from errors.
- [ ] Timestamps are nanoseconds, unrounded, all the way to the seed.

Report the spec version you target, so that two implementations can tell at a
glance whether they agree. This repository's is `1.0.0`; the C ABI reports it
from `hashid_spec_version()`.

If you publish one, open an issue — it belongs in the README's table.
