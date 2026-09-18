#!/usr/bin/env bash
#
# Keeps `crates/hashid-c/include/hashid.h` honest.
#
# C does not check a declaration against its definition across an ABI, so a
# header that has drifted still compiles and then reads the wrong bytes. Two
# checks close that:
#
#   1. Every function declared in the header is exported by the built library.
#      Catches a declaration with no definition, and a rename on one side only.
#   2. The smoke test calls every one of them and asserts on the values.
#      Catches a signature that changed shape.
#
# Run: tools/check-c-abi.sh [--release]
set -euo pipefail

cd "$(dirname "$0")/.."

PROFILE_DIR=debug
CARGO_ARGS=()
if [[ "${1:-}" == "--release" ]]; then
    PROFILE_DIR=release
    CARGO_ARGS=(--release)
fi

HEADER=crates/hashid-c/include/hashid.h
SMOKE=crates/hashid-c/tests/smoke.c
LIBDIR="target/${PROFILE_DIR}"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

echo "==> building libhashid_c"
cargo build -p hashid-c "${CARGO_ARGS[@]}"

LIB="${LIBDIR}/libhashid_c.so"
[[ -f "$LIB" ]] || LIB="${LIBDIR}/libhashid_c.dylib"
if [[ ! -f "$LIB" ]]; then
    echo "no shared library found under ${LIBDIR}" >&2
    exit 1
fi

echo "==> checking every declared function is exported"
# Declarations look like `hashid_status hashid_foo(...)` or `void hashid_bar(`.
# Take every hashid_* identifier immediately followed by '(' that is not a
# struct field (those are indented and parenthesised as function pointers).
declared=$(grep -oE '\bhashid_[a-z0-9_]+\(' "$HEADER" \
    | tr -d '(' | sort -u)
exported=$(nm -D --defined-only "$LIB" 2>/dev/null \
    | awk '{print $3}' | grep -E '^hashid_' | sort -u)

missing=0
while read -r symbol; do
    [[ -z "$symbol" ]] && continue
    if ! grep -qx "$symbol" <<< "$exported"; then
        echo "  MISSING from the library: $symbol" >&2
        missing=1
    fi
done <<< "$declared"

undeclared=0
while read -r symbol; do
    [[ -z "$symbol" ]] && continue
    if ! grep -qx "$symbol" <<< "$declared"; then
        echo "  exported but NOT declared in the header: $symbol" >&2
        undeclared=1
    fi
done <<< "$exported"

if [[ "$missing" == 1 || "$undeclared" == 1 ]]; then
    echo "header and library disagree" >&2
    exit 1
fi
echo "  $(wc -l <<< "$declared") functions, header and library agree"

echo "==> compiling and running the smoke test"
cc -std=c11 -Wall -Wextra -Werror -Wpedantic \
   -I crates/hashid-c/include \
   "$SMOKE" -o "$WORK/smoke" \
   -L "$LIBDIR" -lhashid_c -Wl,-rpath,"$PWD/$LIBDIR"
"$WORK/smoke"
