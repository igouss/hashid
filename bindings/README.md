# Per-language smoke tests

Generated bindings are **not** checked in. They are regenerated on every run,
because a stale binding that no longer matches the scaffolding is exactly the
failure UniFFI's checksum guard exists to catch — and regenerating means it
cannot happen here.

What lives here is the test that gets run *through* the generated bindings,
written the way a user would write it: importing the generated module,
implementing the store interface in the target language, catching the generated
exceptions. Anything awkward about the generated API shows up here first.

```text
python/test_hashid.py       25 tests, written the way a user writes them
python/test_conformance.py  spec/vectors.json, run through the bindings
```

Both are run by `tools/check-python-bindings.sh`.

`test_conformance.py` is the one that matters most. The vector file exists to
prove two implementations agree, and running it only against the Rust core would
leave the boundary ungated — which is exactly where a drifting re-implementation
lives. 114 of the 193 vectors cross the real boundary; the rest pin the wire
format and minting internals, which a binding does not expose. A group that is
neither run nor named as out of scope fails the test.

It also keeps the API cut honest: the binding does not export `id_depth`,
`id_parent`, `id_child`, `is_descendant_of` or `validate_prefix`, so their
vectors are checked through the one-liners the docs tell you to write instead.
If a derivation stops reproducing its vectors, the function belonged in the API
after all.

To add a language, generate its bindings (see
[`spec/binding-guide.md`](../spec/binding-guide.md)), write the equivalent test,
and add a `tools/check-<language>-bindings.sh` that generates, places the shared
library, and runs it.

The C ABI's equivalent lives beside the crate, at
[`crates/hashid-c/tests/smoke.c`](../crates/hashid-c/tests/smoke.c), because it
is compiled against a header in that crate rather than generated.
