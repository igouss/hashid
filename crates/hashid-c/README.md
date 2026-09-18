# hashid-c

A plain C ABI for `hashid` — content-addressed, human-typeable short IDs,
reachable from C and from every language that can call C.

[`include/hashid.h`](include/hashid.h) is the normative description of this ABI
and documents every function. See the [repository README](../../README.md) for
what the IDs are and why.

## Building

```sh
cargo build -p hashid-c --release
```

```text
target/release/libhashid_c.so   dlopen / LoadLibrary / JNA / ctypes / P/Invoke
target/release/libhashid_c.a    static linking into a C or C++ binary
```

```sh
cc -I crates/hashid-c/include your.c -L target/release -lhashid_c
```

## Using it

This library ships no store. You answer one question about yours:

```c
#include "hashid.h"

/* The IDs you already have, wherever they really live. */
static const char *known[] = { "myproj-8cda" };

static int32_t known_contains(void *ctx, const char *id, const char **err) {
    (void)ctx; (void)err;
    for (size_t i = 0; i < 1; i++)
        if (strcmp(known[i], id) == 0) return 1;
    return 0;
}

hashid_minter *minter = NULL;
hashid_minter_new("myproj", &minter);         /* prefix normalized for you */

hashid_request request = {
    .title            = "Ship the thing",
    .description      = NULL,
    .creator          = "iouri",
    .created_at_nanos = hashid_now_nanos(),
    .population       = 1,                    /* an estimate is fine */
};

char *id = NULL;
if (hashid_minter_mint(minter, &request, known_contains, NULL, &id) == HASHID_OK) {
    printf("%s\n", id);                       /* myproj-3f9k */
    remember(id);                             /* your store, your write */
    hashid_string_free(id);
} else {
    fprintf(stderr, "%s\n", hashid_last_error_message());
}

hashid_minter_free(minter);
```

`hashid_minter_with_config` takes a `hashid_config` instead, for a non-default
entropy policy or a prefix you have already validated strictly.

## How small it is

Sixteen functions. Four of them are the whole pure API — `hashid_parse_id`,
`hashid_is_valid_id`, `hashid_normalize_prefix`, `hashid_now_nanos` — and the
rest are the minter handle, minting, and frees.

Everything derivable from `hashid_parse_id` is deliberately absent, because an
export you have to look up costs more than the line it saves. From a `hashid_id`
you have `prefix`, `hash` and `child_path`, so an ID's depth is
`child_path_len`, a root is `child_path_len == 0`, and its n-th child is
`sprintf("%s.%u", id.text, n)`.

## Your store is the only store

There is no built-in one, and no store handle or vtable either, because minting
asks exactly one question. A store shipped with a library is a hash set that
ends up in production holding a set which stopped fitting in memory two releases
ago. Your IDs already live somewhere — an array, a sqlite table, an index — and
one callback over that is both less code than a second copy of it and the only
version that cannot go stale.

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

The `-1` return is the point. If the callback could not say "I do not know", a
store talking to a real database would have to pick a lie when the connection
drops: answer "not taken" and invent an ID that may already exist, or answer
"taken" and burn a good candidate. Both are silent, and the first corrupts data.
The minter propagates it as `HASHID_ERR_STORE` rather than guessing.

`contains` is called once per candidate, so make it cheap. The minter only asks
again when a candidate is taken, so a store answering from an index answers
about once per ID minted.

## Resolution is not here

Turning a typed fragment like `8cd` back into `myproj-8cda` lives in
[`hashid-resolve`](../hashid-resolve), which this ABI does not cover. Minting
asks *is this taken?*, which any store answers with an indexed lookup;
resolution needs a search. They ship apart so that a caller who only mints never
writes the search.

## The three rules

1. **Status codes.** Every function returns `hashid_status`; `HASHID_OK` is 0.
   On failure, out-parameters are untouched and `hashid_last_error_message()`
   describes what happened. For a malformed input, `hashid_last_error_reason()`
   names *which* grammar rule broke, so you can highlight one field rather than
   reject the whole input.
2. **Ownership.** Every pointer the library returns is yours, released with the
   matching `hashid_*_free`. Every pointer you pass in is borrowed for the call
   and never retained. The one exception is `hashid_last_error_message()`,
   borrowed until your next call on that thread. Each `free` zeroes what it
   frees and accepts `NULL`, so a double call is harmless.
3. **No unwinding.** Every entry point catches panics and reports
   `HASHID_ERR_PANIC`. A Rust panic crossing into C frames is undefined
   behaviour, so it does not happen here.

## Why this exists rather than a generated binding

UniFFI has no C backend, first-party or third-party. Its scaffolding *is* a C
ABI, but an internal one — `RustBuffer` structs, status out-parameters, no
header, and no stability promise across versions. Writing C against it means
writing against something designed to change. So this layer is hand-written,
and kept as small as it can be.

## Verifying

```console
$ tools/check-c-abi.sh
==> checking every declared function is exported
  16 functions, header and library agree
==> compiling and running the smoke test
ok — 48 checks passed against libhashid_c
```

Two checks, because C does not verify a declaration against its definition
across an ABI: a header that has drifted still compiles, and then reads the
wrong bytes. The symbol comparison catches a declaration with no definition;
[`tests/smoke.c`](tests/smoke.c) calls every function and asserts on values,
which catches a signature that changed shape.

## Why this is a separate crate

`hashid` itself keeps `unsafe_code = "forbid"` and three dependencies. This
crate holds every `unsafe extern "C"` boundary so the domain never has to, and
contains no rules of its own — every decision it appears to make is delegated to
the core.

For Kotlin, Swift, Python, Ruby or Java via generated bindings, use
[`crates/hashid-ffi`](../hashid-ffi). For a language with neither,
[`spec/`](../../spec) specifies the format completely and ships conformance
vectors.

## License

MIT OR Apache-2.0
