#!/usr/bin/env bash
#
# Generates the Python bindings and runs the smoke test against them.
#
# The bindings are regenerated every run rather than committed: a stale
# checked-in binding that no longer matches the scaffolding is the exact
# failure UniFFI's checksum guard exists to catch, and regenerating means it
# can never happen here.
set -euo pipefail

cd "$(dirname "$0")/.."

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

echo "==> building libhashid_ffi"
cargo build -p hashid-ffi

echo "==> generating python bindings"
cargo run -q --bin uniffi-bindgen -- generate \
    --library target/debug/libhashid_ffi.so \
    --language python \
    --out-dir "$WORK"

cp target/debug/libhashid_ffi.so "$WORK/"
cp bindings/python/test_hashid.py bindings/python/test_conformance.py "$WORK/"
cp spec/vectors.json "$WORK/"

echo "==> running the smoke test"
(cd "$WORK" && python3 test_hashid.py)

# The vector file exists to prove two implementations agree. Running it only
# against the Rust core would leave the boundary — where a drifting
# re-implementation actually lives — ungated.
echo "==> running spec/vectors.json through the bindings"
(cd "$WORK" && python3 test_conformance.py)
