"""Exercises the generated Python bindings against the real shared library.

This is the gate on the UniFFI layer. It is deliberately written the way a
*user* would write it — importing the generated module, implementing the store
interface in Python, catching the generated exceptions — so that anything
awkward about the generated API shows up here first.

It also doubles as the proof that the API is small enough: TestDerivable below
writes, in plain Python, everything the binding deliberately does not export.

Run it with tools/check-python-bindings.sh.
"""

import sys
import unittest

import hashid

# The instant every golden value below is frozen at.
FROZEN = 1783686896789012345


class PythonStore(hashid.IdStore):
    """An IdStore implemented in Python.

    The binding ships no store, so this is not a stand-in for one — it is the
    only kind there is, and the whole of the integration. One method. A real
    one queries a table where this one queries a set; `hashid` cannot tell the
    difference, which is the point.
    """

    def __init__(self, ids=(), broken=False):
        super().__init__()
        self.ids = set(ids)
        self.broken = broken
        self.contains_calls = 0

    def contains(self, id):
        self.contains_calls += 1
        if self.broken:
            raise hashid.StoreError.Failed("the python database is on fire")
        return id in self.ids


class TestTheFreeFunctions(unittest.TestCase):
    def test_parse_splits_at_the_last_hyphen(self):
        parsed = hashid.parse_id("bead-me-up-3e9.1.2")
        self.assertEqual(parsed.text, "bead-me-up-3e9.1.2")
        self.assertEqual(parsed.prefix, "bead-me-up")
        self.assertEqual(parsed.hash, "3e9")
        self.assertEqual(parsed.child_path, [1, 2])

    def test_parse_names_the_rule_that_broke(self):
        with self.assertRaises(hashid.HashidError.Malformed) as caught:
            hashid.parse_id("bd-ABC")
        self.assertEqual(caught.exception.reason, hashid.MalformedReason.HASH_CHARSET)

    def test_child_segments_canonicalize(self):
        self.assertEqual(hashid.parse_id("br-abc.01").text, "br-abc.1")

    def test_validity_is_a_plain_bool(self):
        self.assertTrue(hashid.is_valid_id("br-8cda"))
        self.assertFalse(hashid.is_valid_id("br-8CDA"))

    def test_normalize_prefix_is_lenient_and_refuses_to_invent(self):
        self.assertEqual(hashid.normalize_prefix("  Project-Name_2!  "), "project-name_2")
        with self.assertRaises(hashid.HashidError.UnusablePrefix):
            hashid.normalize_prefix("!!!")

    def test_now_nanos_advances(self):
        self.assertGreater(hashid.now_nanos(), FROZEN)


class TestDerivable(unittest.TestCase):
    """What the binding deliberately does NOT export, written in Python.

    If any of these stops being a one-liner, the cut was wrong and the function
    belongs back in the API.
    """

    def test_hierarchy_is_a_property_of_the_parsed_parts(self):
        id = hashid.parse_id("br-abc123.1.2")

        depth = len(id.child_path)
        is_root = not id.child_path
        parent = id.text.rsplit(".", 1)[0] if id.child_path else None
        child = f"{id.text}.7"

        self.assertEqual(depth, 2)
        self.assertFalse(is_root)
        self.assertEqual(parent, "br-abc123.1")
        self.assertEqual(child, "br-abc123.1.2.7")
        self.assertIsNone(hashid.parse_id("br-abc123").child_path or None)

    def test_descendancy_is_three_comparisons(self):
        def descends(a, b):
            a, b = hashid.parse_id(a), hashid.parse_id(b)
            return (a.prefix == b.prefix and a.hash == b.hash
                    and len(a.child_path) > len(b.child_path)
                    and a.child_path[:len(b.child_path)] == b.child_path)

        self.assertTrue(descends("br-abc123.1.2", "br-abc123"))
        self.assertFalse(descends("br-abc123", "br-abc123"))
        self.assertFalse(descends("br-abc123.2", "br-abc123.1"))

    def test_checking_a_namespace_is_a_membership_test(self):
        allowed = {"bd", "br"}
        self.assertIn(hashid.parse_id("bd-abc123").prefix, allowed)
        self.assertNotIn(hashid.parse_id("zz-abc123").prefix, allowed)


class TestMinting(unittest.TestCase):
    def setUp(self):
        self.generator = hashid.Minter("br")
        self.request = hashid.Request(title="Test Issue", description="Description",
                                      creator="user", created_at_nanos=FROZEN,
                                      population=0)

    def test_a_prefix_is_all_a_minter_needs(self):
        minted = (hashid.Minter("  Project-Name_2!  ")
                  .mint(hashid.Request(title="x", created_at_nanos=FROZEN), PythonStore()))
        self.assertTrue(minted.startswith("project-name_2-"), minted)
        with self.assertRaises(hashid.HashidError.UnusablePrefix):
            hashid.Minter("!!!")

    def test_config_defaults_need_no_helper(self):
        config = hashid.MinterConfig(prefix="br")
        self.assertEqual((config.min_hash_length, config.max_hash_length,
                          config.max_collision_prob), (3, 8, 0.25))

    def test_an_explicit_config_takes_its_prefix_strictly(self):
        with self.assertRaises(hashid.HashidError.Malformed):
            hashid.Minter.with_config(hashid.MinterConfig(prefix="BR"))

    def test_deterministic_mint(self):
        self.assertEqual(self.generator.mint_unchecked(self.request, 0, 3), "br-grz")
        self.assertEqual(self.generator.mint_unchecked(self.request, 1, 3), "br-2cs")
        self.assertEqual(self.generator.mint_unchecked(self.request, 0, 8), "br-4scx9grz")

    def test_zero_is_not_a_hash_length(self):
        with self.assertRaises(hashid.HashidError.InvalidHashLength):
            self.generator.mint_unchecked(self.request, 0, 0)

    def test_prefix_does_not_perturb_the_hash(self):
        other = hashid.Minter("bd")
        self.assertEqual(other.mint_unchecked(self.request, 0, 3), "bd-grz")

    def test_an_empty_store_takes_the_first_candidate(self):
        store = PythonStore()
        self.assertEqual(self.generator.mint(self.request, store), "br-grz")
        self.assertEqual(store.contains_calls, 1)

    def test_slugged_mint(self):
        minted = self.generator.mint_with_slug(self.request, "Survey The Users",
                                               PythonStore())
        self.assertEqual(minted, "br-survey-the-users-grz")

    def test_a_taken_candidate_steps_the_ladder(self):
        store = PythonStore(["br-grz"])
        self.assertEqual(self.generator.mint(self.request, store), "br-2cs")
        self.assertEqual(store.contains_calls, 2)

    def test_the_ladder_steps_past_every_taken_candidate(self):
        store = PythonStore(["br-grz", "br-2cs"])
        self.assertEqual(self.generator.mint(self.request, store), "br-qwx")
        self.assertEqual(store.contains_calls, 3)   # nonces 0 and 1 taken, 2 wins

    def test_a_number_out_of_range_is_refused_not_clamped(self):
        """A saturating conversion would turn each of these into a plausible
        number and say nothing. Every one must be an error instead."""
        cases = {
            "population": lambda: hashid.Request(title="x", created_at_nanos=FROZEN,
                                                 population=-1),
            "nonce": lambda: self.generator.mint_unchecked(self.request, -1, 3),
            "hashLength": lambda: self.generator.mint_unchecked(self.request, 0, -3),
        }
        for field, call in cases.items():
            with self.subTest(field=field):
                with self.assertRaises(hashid.HashidError.OutOfRange) as caught:
                    minted = call()
                    # A Request is a plain record: it only fails when used.
                    self.generator.mint(minted, PythonStore())
                self.assertEqual(caught.exception.field, field)

    def test_a_broken_store_is_not_guessed_at(self):
        with self.assertRaises(hashid.HashidError.Store) as caught:
            self.generator.mint(self.request, PythonStore(broken=True))
        self.assertIn("on fire", caught.exception.message)


if __name__ == "__main__":
    print(f"python {sys.version.split()[0]} against hashid bindings", file=sys.stderr)
    unittest.main(verbosity=2)
