# The hashid specification

**Version 1.0.0.** Normative. Everything an implementation in any language must
do to mint, parse, and resolve `hashid` identifiers that interoperate with every
other implementation.

This document is paired with [`vectors.json`](vectors.json), a machine-readable
conformance suite. The prose states each rule; the vectors check it. Where the
two disagree, that is a defect in this repository and not a licence to choose —
please report it.

---

## 1. Scope and conformance

An **implementation** is any body of code that produces or consumes `hashid`
identifiers. An implementation is **conformant** when it satisfies every rule
marked SHALL below, and reproduces every vector in `vectors.json` that belongs
to a capability it implements.

Capabilities are separable. An implementation MAY implement only parsing, or
only the deterministic mint, and still be conformant for those capabilities.
The vector file groups vectors by capability so that a partial implementation
can run exactly the subset it claims.

Rules carry stable identifiers (`SD-2`, `RS-5`). Identifiers are never renumbered
or reused; a retired rule is marked withdrawn and its number retained. Each
vector names the rules it exercises, which is the traceability spine between the
two files.

### 1.1 Key words

MUST / SHALL, MUST NOT, SHOULD, and MAY are used as in RFC 2119.

### 1.2 What is *not* specified

- **Storage.** The library asks your store two questions (§10). How you answer
  them is out of scope.
- **Concurrency.** Minting is a pure function of content and a store snapshot.
  Serializing concurrent mints is the caller's problem.
- **Transport.** An ID is a string of ASCII bytes. Anything that carries ASCII
  carries an ID.

---

## 2. Terminology

| Term | Meaning |
|---|---|
| **prefix** | The namespace half of an ID, left of the final `-`. May itself contain `-`, and may carry an embedded slug. |
| **hash** | The uniquifying half: base36 characters, right of the final `-`. |
| **child path** | A `.`-separated suffix of integers giving a position in a hierarchy. |
| **slug** | A human-readable label folded into the prefix at mint time. |
| **seed** | The exact string that gets hashed. |
| **nonce** | A counter mixed into the seed, used to escalate on collision. |
| **population** | How many IDs already exist. Drives the adaptive hash length. |

Byte counts are **bytes of UTF-8**, never characters, everywhere in this
document. `héllo wörld` is 13 bytes and 11 characters; where the two differ,
bytes win.

---

## 3. The grammar

```abnf
id           = prefix "-" hash [ child-path ]

prefix       = 1*64prefix-char
prefix-char  = %x61-7A / %x30-39 / "_" / "-" / "." / ":" / "#"   ; a-z 0-9 _-.:#

hash         = 1*40base36-char
base36-char  = %x30-39 / %x61-7A                                 ; 0-9 a-z

child-path   = 1*( "." segment )
segment      = 1*10DIGIT                                         ; value <= 4294967295
```

- **GR-1** An implementation SHALL accept exactly the strings this grammar
  generates, and reject every other string.
- **GR-2** WHEN splitting an ID into prefix and remainder, an implementation
  SHALL split at the **last** `-`, not the first. This is what lets
  `bead-me-up-3e9` keep the prefix `bead-me-up` and `external:jira-123` keep
  `external:jira`. Splitting at the first `-` is the single most likely bug in a
  new implementation.
- **GR-3** The prefix SHALL be 1 to 64 bytes, every byte drawn from
  `prefix-char`. Uppercase is **rejected**, not folded (see PF-1).
- **GR-4** The hash SHALL be 1 to 40 bytes, every byte drawn from `base36-char`.
- **GR-5** Each child segment SHALL parse as a base-10 unsigned 32-bit integer.
  A segment that is empty, negative, non-numeric, or greater than `4294967295`
  makes the whole ID malformed.
- **GR-6** The **canonical rendering** of an ID SHALL be
  `prefix "-" hash *( "." decimal-segment )` where each segment is rendered
  without leading zeros. `br-abc.01` is a legal input that canonicalizes to
  `br-abc.1`. An implementation SHALL round-trip through the canonical form.
- **GR-7** The total length of an ID is **not** independently enforced. A root
  ID is at most 105 bytes (64 + 1 + 40) as a consequence of GR-3 and GR-4, and a
  child path extends beyond that. An implementation SHALL NOT reject an ID
  merely for exceeding 105 bytes.

---

## 4. Prefixes

### 4.1 Strict parsing

- **PF-1** `parse_prefix(raw)` SHALL accept `raw` unchanged when it satisfies
  GR-3, and SHALL fail otherwise. `BR` fails; it is not folded to `br`.

### 4.2 Lenient normalization

- **PF-2** `normalize_prefix(raw)` SHALL apply these steps, in this order:
  1. Trim leading and trailing whitespace.
  2. For each character in order: fold ASCII uppercase to lowercase, then keep
     it if it is a `prefix-char` and drop it otherwise. Non-ASCII characters are
     dropped, never transliterated.
  3. Keep at most the first 64 characters that survived step 2.
  4. Strip every trailing `_`, `-`, `.`, `:` and `#`.

  The clamp in step 3 counts *kept* characters, and the strip in step 4 runs
  after the clamp — so a prefix truncated mid-punctuation does not leave a
  dangling separator that would produce `br--8cda`.
- **PF-3** WHEN nothing survives PF-2, `normalize_prefix` SHALL fail with
  `UnusablePrefix`. An implementation SHALL NOT substitute a default. A silent
  fallback attaches real IDs to a namespace the caller never chose.

### 4.3 Abbreviation

- **PF-4** `abbreviate_prefix(prefix)` SHALL return:
  - `prefix` unchanged when it is 6 bytes or fewer; otherwise
  - the concatenated first characters of its words, when splitting on every
    non-ASCII-alphanumeric character and discarding empties yields more than one
    word (`my_project-name` → `mpn`); otherwise
  - the first 3 ASCII-alphanumeric characters of `prefix`
    (`superlongname` → `sup`); otherwise
  - `prefix` unchanged, when that yields nothing.

### 4.4 Namespace validation

- **PF-5** `validate_prefix(id, expected, also_allowed)` SHALL succeed when
  `id`'s prefix equals `expected` or appears in `also_allowed`, and SHALL fail
  with `PrefixMismatch` otherwise. Comparison is exact; no normalization is
  applied to either side.

  The allow-list exists for imports: a dataset merged in from another project
  keeps its own prefix, and you want to accept it without accepting everything.

---

## 5. Slugs

- **SL-1** `normalize_slug(raw)` SHALL scan `raw` character by character,
  building an output string:
  - fold ASCII uppercase to lowercase; if the result is `a-z` or `0-9`, append
    it and clear the pending-separator flag;
  - otherwise, if the pending-separator flag is clear **and** the output is not
    empty, append `-` and set the flag.

  A leading run of separators therefore produces nothing, and an interior run
  produces exactly one `-`.
- **SL-2** The output SHALL then have every trailing `-` removed; if it is
  longer than 48 bytes it SHALL be truncated to 48 bytes and have trailing `-`
  removed again.
- **SL-3** WHEN the result is empty, `normalize_slug` SHALL yield "no slug" —
  not an error, and not an empty slug. The caller falls back to a hash-only ID.
- **SL-4** `truncate_slug(slug, max)` SHALL return `slug` unchanged when it is
  at most `max` bytes; otherwise truncate to `max` bytes and remove trailing
  `-`, yielding "no slug" if nothing remains.
- **SL-5** `join_prefix_slug(prefix, slug)` SHALL compute
  `budget = 64 - (len(prefix) + 1)`, yielding "does not fit" when that
  subtraction would go below zero; apply SL-4 with `max = budget`; and return
  `prefix + "-" + truncated`. The `+ 1` is the byte spent on the joining `-`.

---

## 6. The seed

The seed is the exact string that gets hashed. Getting it wrong is the failure
mode that produces *plausible* IDs that no other implementation agrees with, so
it is specified byte by byte.

- **SD-1** `seed(request, nonce)` SHALL be the concatenation, in this order and
  with no separator between them, of five encoded fields:
  `title`, `description`, `creator`, `timestamp`, `nonce`.
- **SD-2** Each field SHALL be encoded as `<byte-length> ":" <value>`, where
  `<byte-length>` is the field's length in bytes of UTF-8, rendered in base-10
  without leading zeros.

  This length prefix is load-bearing, not decoration. A naive
  `title|description|creator` join lets a caller move the boundary: title `a|b`
  with no description would seed identically to title `a` with description `b`,
  so two genuinely different records would collide by construction rather than
  by chance. The length prefix makes the boundaries unforgeable.
- **SD-3** An absent `description` or `creator` SHALL be encoded as the empty
  string, i.e. `0:`. An implementation SHALL NOT omit the field.
- **SD-4** `timestamp` SHALL be the creation instant in **nanoseconds since the
  Unix epoch**, UTC, rendered in base-10. WHEN the instant cannot be represented
  in a signed 64-bit count of nanoseconds — before 1677-09-21T00:12:43.145224192Z
  or after 2262-04-11T23:47:16.854775807Z — the timestamp SHALL be `0`.

  Nanoseconds, not a language-native date type, because this number is hashed
  verbatim. Two implementations that round it differently mint different IDs for
  the same record, and neither can tell.
- **SD-5** `nonce` SHALL be an unsigned 32-bit integer rendered in base-10, and
  SHALL be the last field.

**Worked example.** Title `title`, description `desc`, creator `me`, instant
`2026-07-10T12:34:56.789012345Z` (= `1783686896789012345` nanoseconds), nonce 0:

```
5:title4:desc2:me19:17836868967890123451:0
└┬─────┘└┬────┘└┬──┘└┬──────────────────┘└┬┘
 title   descr  creator   timestamp       nonce
```

---

## 7. Base36

- **B36-1** The alphabet SHALL be `0123456789abcdefghijklmnopqrstuvwxyz`, in
  that order — digits, then lowercase letters, matching ASCII order.
- **B36-2** `base36(0)` SHALL be `"0"`.
- **B36-3** `base36(n)` for `n > 0` SHALL be the standard positional rendering
  with no leading zeros and no padding. `base36(35)` is `z`, `base36(36)` is
  `10`, and `base36(2^64 - 1)` is `3w5e11264sgsf`.

---

## 8. The digest

- **DG-1** `digest(input, length)` SHALL compute `SHA-256(utf8_bytes(input))`.
- **DG-2** It SHALL take the **first 8 bytes** of that 32-byte digest and read
  them as a **big-endian unsigned 64-bit integer**.
- **DG-3** It SHALL render that integer per §7, giving at most 13 characters.
- **DG-4** WHEN the rendering is shorter than `length`, it SHALL be left-padded
  with `0` to exactly `length` characters.
- **DG-5** It SHALL return the **last** `length` characters of the (possibly
  padded) rendering — not the first.

  Truncating from the tail is deliberate. The leading base36 digits of a 64-bit
  value barely move across inputs; taking the head would throw away most of the
  entropy at short lengths. The tail keeps the digits that vary fastest, and it
  has the pleasant consequence that a shorter hash is always a suffix of a
  longer one for the same seed: `digest(x, 3)` is the last three characters of
  `digest(x, 8)`.
- **DG-6** `length` SHALL be in `1..=40`. Zero is not a length — there is no
  hash of zero characters — and an implementation SHALL report an error rather
  than return an empty hash. A value above 40 violates GR-4.

---

## 9. Adaptive hash length

The hash length is not fixed. It is chosen per mint from the current population,
so a young dataset gets short, typeable IDs and a large one widens on its own.

- **LN-1** For a base36 hash of `len` characters there are `36^len` distinct
  values, and drawing `n` of them collides with probability approximately

  ```
  P(len, n) = 1 - e^(-n² / (2 · 36^len))
  ```

  An implementation SHALL use this estimate.
- **LN-2** `hash_length_for(config, population)` SHALL return the **first**
  `len` in `config.min_hash_length ..= config.max_hash_length`, ascending, for
  which `P(len, population) < config.max_collision_prob`.
- **LN-3** WHEN no length in that range satisfies LN-2, it SHALL return
  `config.max_hash_length`.

A non-trivial collision probability here is fine, and that is the design: every
candidate is checked against the store anyway (§11). LN-2 only decides how often
that check has to do real work; it is not what guarantees uniqueness.

At the default policy — lengths 3 to 8, bound 0.25 — the answers are 3 for a
population of 100, 5 for 1 000, 6 for 10 000, 7 for 100 000, and 8 from
1 000 000 up.

- **LN-4** `min_hash_length` and `max_hash_length` SHALL satisfy
  `1 <= min <= max <= 40`; a configuration that does not is an error, reported
  before any ID is minted.
- **LN-5** `max_collision_prob` SHALL satisfy `0.0 < p < 1.0`.

---

## 10. The store

An implementation that mints or resolves SHALL obtain answers to two questions
from the caller:

- **ST-1** `contains(id) -> boolean` — is this ID already taken?
- **ST-2** `find_by_hash_fragment(fragment) -> list of id` — every known ID
  matching `fragment` per §13.
- **ST-3** Both SHALL be **fallible**, and a failure SHALL be distinguishable
  from a negative answer.

  ST-3 is not ceremony. If the store cannot say "I do not know", an
  implementation talking to a real database has to pick a lie when the
  connection drops: answer "not taken" and invent an ID that may already exist,
  or answer "taken" and burn a good candidate. Both are silent. An honest error
  is the third option, and §11 requires it be propagated.
- **ST-4** WHEN `find_by_hash_fragment` returns a string that is not a
  well-formed ID, the implementation SHALL report a store failure. It SHALL NOT
  silently skip the entry.

---

## 11. Minting

### 11.1 The deterministic primitive

- **MT-1** `mint_unchecked(config, request, nonce, length)` SHALL be

  ```
  config.prefix + "-" + digest(seed(request, nonce), length)
  ```

  It consults no store and offers no uniqueness guarantee. It is the
  content-addressing primitive: identical arguments yield an identical ID in
  every language, forever.
- **MT-2** The prefix SHALL NOT perturb the hash. Two configs differing only in
  prefix, given the same request, nonce and length, SHALL produce the same hash.

### 11.2 The escalation ladder

`mint(config, request, store)` SHALL walk the following ladder, returning the
first candidate for which `contains` answers false.

- **ES-1** Start at `length = hash_length_for(config, request.population)`. Try
  nonces `0, 1, … 9` at that length.
- **ES-2** WHEN all ten are taken and `length < config.max_hash_length`,
  increment `length` by one and repeat ES-1.
- **ES-3** WHEN all ten are taken and `length == config.max_hash_length`, enter
  the fallback: set `length = 12` and `nonce = 0`, and try
  `mint_unchecked(config, request, nonce, 12)`, incrementing `nonce` by one each
  time.

  The fallback length is 12 regardless of `config.max_hash_length`. It is a
  deliberate escape from the configured ceiling, not a continuation of it.
- **ES-4** WHILE in the fallback, WHEN `nonce > 1000`, the implementation SHALL
  additionally try the candidate whose hash is the 12-character fallback hash
  with the decimal nonce appended — `digest(seed(request, nonce), 12) + nonce`.
  Decimal digits are valid base36, so the result still satisfies GR-4.
- **ES-5** WHEN `nonce > 2000`, the implementation SHALL stop and report
  `Exhausted`, carrying the prefix and the number of candidates rejected.
- **ES-6** An implementation SHALL NOT return an ID that `contains` reported as
  taken, under any circumstance, including exhaustion. A duplicate ID is a
  silent data-corruption bug; an error is merely a loud one.
- **ES-7** WHEN `contains` fails, the implementation SHALL abort and propagate
  the failure. It SHALL NOT treat a failure as "not taken" and SHALL NOT retry
  past it.

### 11.3 Slugged minting

`mint_with_slug(config, request, slug, store)` mints
`proj-survey-my-thing-8cda` — the slug sits between prefix and hash, and the
hash is still always appended, because two records may share a slug.

- **SM-1** The implementation SHALL normalize `slug` per SL-1..SL-3 and join it
  to `config.prefix` per SL-5, producing a composite prefix. It SHALL then walk
  ES-1 and ES-2 against that composite prefix.
- **SM-2** The implementation SHALL fall back to plain `mint` (§11.2), dropping
  the slug entirely, in each of three cases: the slug normalizes to nothing; the
  join does not fit the 64-byte prefix budget; or ES-2 exhausts
  `config.max_hash_length` in the slugged namespace. Producing a valid ID always
  beats honouring a cosmetic request.
- **SM-3** The slugged ladder SHALL NOT enter the §11.2 fallback directly. It
  drops the slug first (SM-2) and the plain ladder may then reach the fallback.
  Growing a composite prefix past 64 bytes is not an option the grammar allows.

---

## 12. Hierarchy

- **HR-1** `parent(id)` SHALL return the ID with its last child segment removed,
  or "no parent" when the ID is a root.
- **HR-2** `child(id, n)` SHALL return the ID with `n` appended as a new last
  child segment.
- **HR-3** `depth(id)` SHALL be the number of child segments: `0` for a root.
  `is_root(id)` SHALL be `depth(id) == 0`.
- **HR-4** `is_descendant_of(id, ancestor)` SHALL be true exactly when the two
  share a prefix and a hash, `id` has strictly more child segments, and `id`'s
  child path begins with `ancestor`'s. An ID is never a descendant of itself.

---

## 13. Fragment matching

- **FM-1** `matches_fragment(id, fragment)` SHALL split `fragment` at its
  **first** `.` into a hash part and an optional child part.
- **FM-2** It SHALL require that `id`'s hash **contains** the hash part as a
  substring — anywhere, not only as a prefix.
- **FM-3** WHEN the fragment carries a child part, it SHALL additionally require
  that `id`'s child path, rendered as `.`-joined decimals, equals that child
  part **exactly**, and that `id` is not a root.

  Exactly, not by prefix: otherwise searching for `64up6.4` would return the
  parent `br-64up6` and every sibling of the child you asked for.
- **FM-4** WHEN the fragment carries no child part, `id`'s child path SHALL be
  ignored. `8cda` matches `br-8cda`, `br-8cda.1` and `br-8cda.1.2` alike.

---

## 14. Resolution

`resolve(input, options, store)` turns what a human typed into the ID they
meant. Short IDs exist to be retyped, and nobody retypes the whole thing.

- **RS-1** The implementation SHALL trim `input`, then lowercase it, and match
  against the lowercased form. The caller's original input SHALL be preserved
  verbatim in the result, for echoing back.
- **RS-2** WHEN the trimmed input is empty, it SHALL fail with `Malformed`
  carrying reason `Empty`.

It SHALL then try the following steps in order, returning at the first that
succeeds.

- **RS-3 (Exact).** WHEN the lowercased input parses as an ID per §3 and
  `contains` reports it present, return it with match kind `Exact`.
- **RS-4 (PrefixNormalized).** WHEN the lowercased input contains no `-`, form
  `options.default_prefix + "-" + input`; if that parses and `contains` reports
  it present, return it with match kind `PrefixNormalized`.
- **RS-5 (Substring).** WHEN `options.allow_substring_match` is true:
  1. Split the lowercased input at its **last** `-`. If there is one, the left
     part is a **namespace pin** and the right part is the fragment; otherwise
     the whole input is the fragment and there is no pin.
  2. WHEN the fragment is empty, this step does not match.
  3. Call `find_by_hash_fragment(fragment)`. WHEN there is a namespace pin,
     discard every result whose prefix is not **exactly** the pin.
  4. Exactly one survivor: return it with match kind `Substring`.
  5. More than one: fail with `Ambiguous`, carrying the caller's trimmed input
     and **every** match, sorted lexicographically, so a caller can present a
     choice.
  6. None: this step does not match.
- **RS-6** WHEN no step matches, fail with `NotFound` carrying the trimmed input.
- **RS-7** WHEN the store fails at any step, the implementation SHALL propagate
  that failure rather than report `NotFound`. An unreachable database and a
  genuinely absent ID are not the same answer, and a caller that deletes on
  "not found" would very much like to know the difference.
- **RS-8** WHEN `options.allow_substring_match` is false, RS-5 SHALL be skipped
  entirely: only a complete ID or a complete hash resolves.

---

## 15. Errors

An implementation SHALL be able to report each of the following distinctly. The
names are normative; the rendering is not.

| Code | Carries | Raised by |
|---|---|---|
| `Malformed` | the input, and a reason (below) | §3, RS-2 |
| `PrefixMismatch` | expected prefix, found prefix | prefix validation |
| `Ambiguous` | the fragment, and every match, sorted | RS-5 |
| `NotFound` | the fragment | RS-6 |
| `UnusablePrefix` | the raw prefix | PF-3 |
| `InvalidHashBounds` | min, max, and the limit 40 | LN-4 |
| `InvalidHashLength` | the length, and the limit 40 | DG-6 |
| `InvalidCollisionProb` | the value | LN-5 |
| `Exhausted` | the prefix, and the attempt count | ES-5 |
| `Store` | the store's own message | ST-3 |

`Malformed` reasons, which an implementation SHALL distinguish: `Empty`,
`MissingSeparator`, `EmptyPrefix`, `PrefixTooLong`, `PrefixCharset`,
`EmptyHash`, `HashTooLong`, `HashCharset`, `ChildSegment`.

Reporting *which* rule broke, rather than a stringly "invalid id", is what lets
a caller highlight one field in a form instead of rejecting the whole input.

---

## 16. Conformance vectors

[`vectors.json`](vectors.json) is the machine-checkable half of this document.
Its shape:

```jsonc
{
  "spec_version": "1.0.0",
  "groups": {
    "<capability>": [
      {
        "id": "SEED-004",        // stable, unique across the file
        "rules": ["SD-2","SD-3"],// the rules this vector pins
        "note": "...",           // why this case exists
        "...inputs...": ...,     // group-specific
        "expect": ...            // or "error": "<code>"
      }
    ]
  }
}
```

Every vector names the rules it exercises. A vector whose expectation is an
error carries `"error"` instead of `"expect"`.

### 16.1 What the vectors do not cover

Every rule in this document is either pinned by at least one vector or listed,
with its reason, in `vectors.json`'s `uncovered_rules`. That list is data rather
than prose precisely so that it can be checked: a rule that is neither vectored
nor listed fails the build, so this section cannot quietly go out of date.

Today's exemptions fall into two kinds. **ES-4, ES-5 and SM-3** need a store
holding thousands of specific candidates, or a saturated slugged namespace;
listing those would be most of the file, to pin three branches. **ST-1 to ST-4**
are obligations on *your* store rather than on the implementation's output, so
there is nothing for a vector to assert about them.

What the vectors can and do check is that the implementation honours the store
contract: a store that fails must surface as a `Store` error from both minting
(ES-7) and resolution (RS-7), never as "not taken" or "not found". Those vectors
carry `"store": "failing"` instead of a list of IDs.

Vectors come in **witness / near-miss** pairs wherever a rule draws a line: the
witness is an instance the rule must accept, the near-miss differs in exactly
one salient dimension and must be rejected. The pair is what pins the boundary —
`br-abc` and `br-ABC`, length 40 and length 41, `budget` and `budget - 1`.

Run them as a table test. A conformant implementation reproduces every vector in
every group it claims.

---

## 17. Versioning

This specification is versioned independently of any implementation.

- The **wire format** — §6, §7, §8 and MT-1 — is frozen at version 1. A change
  to any of them orphans every ID ever minted, and would be a new major version
  with a new name, not a revision of this one.
- Behavioural rules outside the wire format may gain clarifications in a minor
  version, provided no previously-conformant implementation becomes
  non-conformant.
- New `Malformed` reasons and new error codes may be added in a minor version.
  An implementation SHALL tolerate an unknown reason rather than fail to parse
  the error.

---

## 18. Writing a binding

See [`binding-guide.md`](binding-guide.md). In short: if your language has a
UniFFI backend, generate the bindings and ship them; if it has a C FFI, link
`hashid-c` and use [`hashid.h`](../crates/hashid-c/include/hashid.h); if neither,
reimplement from this document and prove it with `vectors.json`.
