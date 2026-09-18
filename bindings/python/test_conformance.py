"""Runs spec/vectors.json through the generated Python bindings.

Why this exists: the vector file's whole purpose is proving that two
implementations agree, and until this ran it only ever ran against the Rust
core — which already has Gherkin, property and golden suites. The boundary,
where a drifting re-implementation actually lives, was ungated.

It also keeps the API cut honest. The binding no longer exports `id_depth`,
`id_parent`, `format_id` and friends; the vectors for those groups are checked
here through the one-liners a user would write instead. If a derivation stops
reproducing its vectors, the function belonged in the API after all.
"""

import json
import os
import sys
import unittest

import hashid

VECTORS = os.path.join(os.path.dirname(__file__), "vectors.json")


def request_of(spec):
    return hashid.Request(
        title=spec["title"],
        description=spec.get("description"),
        creator=spec.get("creator"),
        created_at_nanos=int(spec["created_at_nanos"]),
        population=spec["population"],
    )


class HeldStore(hashid.IdStore):
    """The vector's known IDs, as an IdStore.

    The binding ships no store, so the harness writes one — the same three
    lines a caller writes, which is the point: the vectors cross the boundary
    the way real code does.
    """

    def __init__(self, ids):
        super().__init__()
        self.ids = set(ids)

    def contains(self, id):
        return id in self.ids


class FailingStore(hashid.IdStore):
    """A store that cannot answer, for the vectors that check ES-7."""

    def contains(self, id):
        raise hashid.StoreError.Failed("the vector's store is unavailable")


def store_for(vector, key):
    if vector.get("store") == "failing":
        return FailingStore()
    return HeldStore(vector.get(key, []))


def generator_for(vector):
    config = hashid.MinterConfig(prefix=vector["prefix"])
    if "min_hash_length" in vector:
        config.min_hash_length = vector["min_hash_length"]
        config.max_hash_length = vector["max_hash_length"]
    if "max_collision_prob" in vector:
        config.max_collision_prob = vector["max_collision_prob"]
    return hashid.Minter.with_config(config)


# --- One evaluator per group the binding can reach. -------------------------
#
# The first block is the exported API. The second is the API the binding
# deliberately does NOT export, evaluated the way the docs tell a caller to.

def parse_id(v):
    id = hashid.parse_id(v["input"])
    return {"text": id.text, "prefix": id.prefix, "hash": id.hash,
            "child_path": id.child_path}


def mint(v):
    return generator_for(v).mint(request_of(v["request"]), store_for(v, "taken"))


def derived_parent(v):
    id = hashid.parse_id(v["input"])
    return id.text.rsplit(".", 1)[0] if id.child_path else None


def derived_descends(v):
    a, b = hashid.parse_id(v["input"]), hashid.parse_id(v["ancestor"])
    return (a.prefix == b.prefix and a.hash == b.hash
            and len(a.child_path) > len(b.child_path)
            and a.child_path[:len(b.child_path)] == b.child_path)


def derived_validate_prefix(v):
    allowed = {v["expected"], *v["also_allowed"]}
    return hashid.parse_id(v["input"]).prefix in allowed


EVALUATORS = {
    # Exported API.
    "parse_id": parse_id,
    "normalize_prefix": lambda v: hashid.normalize_prefix(v["input"]),
    "mint_unchecked": lambda v: generator_for(v).mint_unchecked(
        request_of(v["request"]), v["nonce"], v["hash_length"]),
    "mint": mint,
    "mint_with_slug": lambda v: generator_for(v).mint_with_slug(
        request_of(v["request"]), v["slug"], store_for(v, "taken")),
    "config_validation": lambda v: generator_for(dict(v, prefix="x")) and None,
    # Deliberately not exported — the documented one-liners, under test.
    "id_depth": lambda v: len(hashid.parse_id(v["input"]).child_path),
    "id_parent": derived_parent,
    "id_child": lambda v: f'{hashid.parse_id(v["input"]).text}.{v["n"]}',
    "is_descendant_of": derived_descends,
    "validate_prefix": derived_validate_prefix,
}

# Groups this binding cannot reach.
#
# `seed`/`digest`/`base36` are the wire format, exercised end to end by
# `mint_unchecked`; `hash_length` and the prefix/slug helpers are internals of
# minting. `resolve` and `matches_fragment` belong to `hashid-resolve`, which
# this binding does not cover — resolution is a search, minting is a lookup,
# and they ship apart.
UNREACHABLE = {"base36", "seed", "digest", "hash_length", "format_id",
               "parse_prefix", "abbreviate_prefix", "normalize_slug",
               "join_prefix_slug", "resolve", "matches_fragment"}


def screaming(camel):
    out = ""
    for i, c in enumerate(camel):
        if c.isupper() and i:
            out += "_"
        out += c.upper()
    return out


class TestVectors(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        with open(VECTORS, encoding="utf-8") as handle:
            cls.suite = json.load(handle)

    def test_every_group_is_reached_or_named(self):
        groups = set(self.suite["groups"])
        unaccounted = groups - set(EVALUATORS) - UNREACHABLE
        self.assertFalse(unaccounted,
                         f"groups neither run nor listed as unreachable: {unaccounted}")

    def test_vectors_hold_through_the_bindings(self):
        ran = 0
        for group, vectors in self.suite["groups"].items():
            evaluate = EVALUATORS.get(group)
            if evaluate is None:
                continue
            for vector in vectors:
                with self.subTest(vector=vector["id"], note=vector.get("note", "")):
                    ran += 1
                    self.check(evaluate, vector)
        # A floor, not a target: it exists so a harness that silently stopped
        # evaluating anything fails loudly. It dropped from 100 to 80 when
        # resolution left this binding — exactly 28 vectors (17 `resolve`,
        # 11 `matches_fragment`), which `hashid-resolve` now runs instead.
        self.assertGreater(ran, 80, f"only {ran} vectors ran through the bindings")
        print(f"\n{ran} of {sum(len(v) for v in self.suite['groups'].values())} vectors "
              f"ran through the bindings "
              f"({len(EVALUATORS)} groups reached, {len(UNREACHABLE)} not a binding's job)",
              file=sys.stderr)

    def check(self, evaluate, vector):
        if "error" in vector:
            expected = getattr(hashid.HashidError, vector["error"])
            with self.assertRaises(expected) as caught:
                evaluate(vector)
            if "reason" in vector:
                self.assertEqual(caught.exception.reason,
                                 getattr(hashid.MalformedReason, screaming(vector["reason"])))
            if "matches" in vector:
                self.assertEqual(caught.exception.matches, vector["matches"])
        else:
            self.assertEqual(evaluate(vector), vector["expect"])


if __name__ == "__main__":
    print(f"python {sys.version.split()[0]} against spec/vectors.json", file=sys.stderr)
    unittest.main(verbosity=1)
