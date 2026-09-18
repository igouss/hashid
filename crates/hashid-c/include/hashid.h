/*
 * hashid — content-addressed, human-typeable short IDs.
 *
 *     br-8cda        myproj-survey-the-users-3f9k1        br-8cda.1.2
 *
 * Short enough to retype from a terminal. Stable enough to put in a commit
 * message. Unique enough to key a database on.
 *
 * The format itself is specified, language-independently, in
 * spec/hashid-spec.md, with conformance vectors in spec/vectors.json.
 *
 * SPDX-License-Identifier: MIT OR Apache-2.0
 *
 * ---------------------------------------------------------------------------
 * THE THREE RULES
 * ---------------------------------------------------------------------------
 *
 * 1. STATUS. Every function returns hashid_status; HASHID_OK is 0. On failure,
 *    out-parameters are untouched and hashid_last_error_message() says why.
 *
 * 2. OWNERSHIP. Every pointer handed back is yours; release it with the
 *    matching hashid_*_free. Every pointer you pass in is borrowed for the
 *    call. The one exception is hashid_last_error_message(), borrowed until
 *    your next call on the same thread.
 *
 * 3. NO UNWINDING. Every entry point catches panics and reports
 *    HASHID_ERR_PANIC, so a bug here cannot corrupt your stack.
 *
 * ---------------------------------------------------------------------------
 * WHAT IS DELIBERATELY NOT HERE
 * ---------------------------------------------------------------------------
 *
 * Anything you can derive from hashid_parse_id in a line. From a hashid_id you
 * have prefix, hash and child_path, so:
 *
 *     depth      id.child_path_len
 *     is root    id.child_path_len == 0
 *     n-th child sprintf("%s.%u", id.text, n)
 *     parent     id.text up to the last '.'
 *
 * An export you have to look up costs more than the line it saves.
 *
 * ---------------------------------------------------------------------------
 * A COMPLETE EXAMPLE
 * ---------------------------------------------------------------------------
 *
 *     // The IDs you already have, wherever they really live. This library
 *     // ships no store: you answer one question about yours.
 *     static const char *known[] = { "myproj-8cda" };
 *
 *     static int32_t known_contains(void *ud, const char *id, const char **err) {
 *         (void)ud; (void)err;
 *         for (size_t i = 0; i < 1; i++)
 *             if (strcmp(known[i], id) == 0) return 1;
 *         return 0;
 *     }
 *
 *     hashid_minter *minter = NULL;
 *     hashid_minter_new("myproj", &minter);
 *
 *     hashid_request request = {
 *         .title            = "Ship the thing",
 *         .description      = NULL,
 *         .creator          = "iouri",
 *         .created_at_nanos = hashid_now_nanos(),
 *         .population       = 1,                    // an estimate is fine
 *     };
 *
 *     char *id = NULL;
 *     if (hashid_minter_mint(minter, &request, known_contains, NULL, &id) == HASHID_OK) {
 *         printf("%s\n", id);                       // myproj-3f9k
 *         remember(id);                             // your store, your write
 *         hashid_string_free(id);
 *     } else {
 *         fprintf(stderr, "%s\n", hashid_last_error_message());
 *     }
 *
 *     hashid_minter_free(minter);
 */

#ifndef HASHID_H
#define HASHID_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* ========================================================================== */
/* Status                                                                     */
/* ========================================================================== */

typedef enum hashid_status {
    HASHID_OK                         = 0,
    /* An input does not satisfy the ID grammar. hashid_last_error_reason()
       says which rule broke, so you can highlight one field. */
    HASHID_ERR_MALFORMED              = 1,
    HASHID_ERR_PREFIX_MISMATCH        = 2,
    /* 3 and 4 were AMBIGUOUS and NOT_FOUND, which resolution owned. They are
       retired rather than reused: a number that changes meaning is how an old
       binary and a new header come to disagree silently. */
    /* A prefix normalized away to nothing. No substitute is invented. */
    HASHID_ERR_UNUSABLE_PREFIX        = 5,
    HASHID_ERR_INVALID_HASH_BOUNDS    = 6,
    HASHID_ERR_INVALID_HASH_LENGTH    = 7,
    HASHID_ERR_INVALID_COLLISION_PROB = 8,
    /* Ran out of candidates. Reported INSTEAD of an ID known to be taken. */
    HASHID_ERR_EXHAUSTED              = 9,
    /* Your store could not answer. Distinct from NOT_FOUND on purpose: an
       unreachable database and an absent ID are different answers. */
    HASHID_ERR_STORE                  = 10,
    HASHID_ERR_NULL_ARGUMENT          = 11,
    HASHID_ERR_INVALID_UTF8           = 12,
    /* A panic was caught at the boundary. Treat as a bug report. */
    HASHID_ERR_PANIC                  = 13,
    /* A failure this header has no name for: the library is newer than you. */
    HASHID_ERR_UNKNOWN                = 14,
    /* A number in a struct you passed is outside the range its field allows,
       or an instant has no representation in nanoseconds. Reported rather than
       clamped: a saturating conversion turns a mistake into a plausible number
       and tells you nothing. */
    HASHID_ERR_OUT_OF_RANGE           = 15
} hashid_status;

/* Which grammar rule the most recent HASHID_ERR_MALFORMED named. */
typedef enum hashid_malformed_reason {
    HASHID_REASON_NONE              = 0,  /* last failure was not Malformed */
    HASHID_REASON_EMPTY             = 1,
    HASHID_REASON_MISSING_SEPARATOR = 2,
    HASHID_REASON_EMPTY_PREFIX      = 3,
    HASHID_REASON_PREFIX_TOO_LONG   = 4,
    HASHID_REASON_PREFIX_CHARSET    = 5,
    HASHID_REASON_EMPTY_HASH        = 6,
    HASHID_REASON_HASH_TOO_LONG     = 7,
    HASHID_REASON_HASH_CHARSET      = 8,
    HASHID_REASON_CHILD_SEGMENT     = 9
} hashid_malformed_reason;

/* The most recent failure ON THIS THREAD, or NULL if the last call succeeded.
   Borrowed: valid only until your next hashid_* call on this thread. */
const char *hashid_last_error_message(void);

/* The hashid_malformed_reason of the most recent failure, or
   HASHID_REASON_NONE. A value this header does not name is a rule newer than
   your header; treat it as "malformed, reason unknown". */
int32_t hashid_last_error_reason(void);

/* Static; do not free. Two libraries reporting the same spec version mint
   identical IDs for identical content. */
const char *hashid_version(void);
const char *hashid_spec_version(void);

/* ========================================================================== */
/* Owned values                                                               */
/* ========================================================================== */

/* A parsed identifier, taken apart. Release with hashid_id_free. */
typedef struct hashid_id {
    char     *text;            /* canonical rendering, e.g. "br-8cda.1.2" */
    char     *prefix;          /* namespace half, e.g. "br"               */
    char     *hash;            /* base36 half, e.g. "8cda"                */
    uint32_t *child_path;      /* child_path_len segments, NULL for root  */
    size_t    child_path_len;  /* 0 for a root ID                         */
} hashid_id;

/* Each zeroes what it frees, so a second call is harmless, and each accepts
   NULL. */
void hashid_string_free(char *value);
void hashid_id_free(hashid_id *id);

/* ========================================================================== */
/* Borrowed inputs                                                            */
/* ========================================================================== */

/* The content an ID is derived from, plus the context needed to size it.
 *
 * IDs are content-addressed: the same fields and nonce always produce the same
 * ID, in every language.
 *
 * created_at_nanos is nanoseconds since the Unix epoch, UTC, and NOT a
 * platform date type, because this number is hashed verbatim: two
 * implementations that round it differently mint different IDs for the same
 * record. hashid_now_nanos() produces one.
 *
 * population — how many IDs already exist — drives the adaptive hash length.
 * Passing 0 when you have a million records silently mints minimum-entropy
 * IDs. */
typedef struct hashid_request {
    const char *title;             /* required        */
    const char *description;       /* NULL for absent */
    const char *creator;           /* NULL for absent */
    int64_t     created_at_nanos;
    uint64_t    population;
} hashid_request;

/* How IDs should look. prefix is taken STRICTLY — it must already be legal;
   run human input through hashid_normalize_prefix() first, because nothing
   here renames your namespace silently. The defaults are { prefix, 3, 8,
   0.25 }: hashes of 3-8 base36 characters, under a 25% birthday bound. */
typedef struct hashid_config {
    const char *prefix;
    uint32_t    min_hash_length;
    uint32_t    max_hash_length;
    double      max_collision_prob;
} hashid_config;

/* ========================================================================== */
/* The five free functions                                                    */
/* ========================================================================== */

/* Parse raw into its parts. The prefix may itself contain '-'
   ("bead-me-up-3e9" has prefix "bead-me-up"), so the split is at the LAST '-'.
   Free *out with hashid_id_free. */
hashid_status hashid_parse_id(const char *raw, hashid_id *out);

/* The predicate form, for when you do not need the reason. NULL and non-UTF-8
   are simply false. */
bool hashid_is_valid_id(const char *raw);

/* Coerce human input into a legal prefix: trim, lowercase ASCII, drop
   everything outside [a-z0-9_-.:#], clamp to 64 bytes, strip trailing
   punctuation. "  Project-Name_2!  " becomes "project-name_2". Fails with
   HASHID_ERR_UNUSABLE_PREFIX when nothing usable survives. Free *out with
   hashid_string_free. */
hashid_status hashid_normalize_prefix(const char *raw, char **out);

/* The current instant, in nanoseconds since the Unix epoch, UTC. */
int64_t hashid_now_nanos(void);

/* ========================================================================== */
/* Your store                                                                 */
/* ========================================================================== */

/* This library ships NO store, and that is deliberate: a store shipped with a
 * library is a hash set that ends up in production holding a set which stopped
 * fitting in memory two releases ago. Your IDs already live somewhere — an
 * array, a sqlite table, an index — and one callback over that is both less
 * code than a second copy of it and the only version that cannot go stale.
 *
 * There is no store handle to create and free, and no vtable to fill in,
 * because minting asks exactly one question.
 *
 * Is id already taken? Return 1 for taken, 0 for free, and anything NEGATIVE
 * for "I could not find out".
 *
 * That third answer is load-bearing. If the callback could not say "I do not
 * know", a store talking to a real database would have to pick a lie when the
 * connection drops: answer "not taken" and invent an ID that may already
 * exist, or answer "taken" and burn a good candidate. Both are silent, and the
 * first corrupts data. The minter propagates instead of guessing, as
 * HASHID_ERR_STORE.
 *
 * On failure you may set *error to a NUL-terminated string; the library copies
 * it before returning, so you keep ownership. user_data is whatever you passed
 * alongside this pointer, handed straight back, and may be NULL.
 *
 * Called once per candidate, so make it cheap — an index hit. The minter only
 * asks again when a candidate is taken, so a store answering from an index is
 * answering about once per ID minted. */
typedef int32_t (*hashid_contains_fn)(void       *user_data,
                                      const char *id,
                                      const char **error);

/* ========================================================================== */
/* Minting                                                                    */
/* ========================================================================== */

typedef struct hashid_minter hashid_minter;

/* A minter for prefix, with the default entropy policy. The prefix is
   normalized the lenient way — "  Project-Name_2!  " becomes "project-name_2" —
   because it usually comes from a human or a config file. */
hashid_status hashid_minter_new(const char *prefix, hashid_minter **out);

/* A minter bound to an explicit config, whose prefix is taken STRICTLY. */
hashid_status hashid_minter_with_config(const hashid_config *config, hashid_minter **out);

void          hashid_minter_free(hashid_minter *minter);

/* Mint an ID your store does not already hold.
 *
 * Walks an escalation ladder, checking every candidate: ten nonces at the
 * adaptive length, then a wider hash, then a 12-character fallback, then the
 * fallback with the nonce spliced in.
 *
 * Fails with HASHID_ERR_EXHAUSTED rather than return an ID it knows to be
 * taken — a duplicate ID is a silent data-corruption bug, an error is merely a
 * loud one — and with HASHID_ERR_STORE when your store could not answer.
 *
 * Free *out with hashid_string_free. */
hashid_status hashid_minter_mint(const hashid_minter  *minter,
                                 const hashid_request *request,
                                 hashid_contains_fn    contains,
                                 void                 *user_data,
                                 char                **out);

/* Mint an ID carrying a human-readable slug: "proj-survey-my-thing-8cda".
 *
 * The slug sits between prefix and hash, and the hash is still always
 * appended, because two records may share a slug. Falls back to a plain
 * hash-only ID when the slug normalizes to nothing, cannot fit the prefix
 * budget, or its namespace is saturated: producing a valid ID always beats
 * honouring a cosmetic request. */
hashid_status hashid_minter_mint_with_slug(const hashid_minter  *minter,
                                           const hashid_request *request,
                                           const char           *slug,
                                           hashid_contains_fn    contains,
                                           void                 *user_data,
                                           char                **out);

/* Mint at an explicit nonce and hash_length, CONSULTING NO STORE.
 *
 * Purely deterministic: identical arguments yield an identical ID in every
 * language. This is the content-addressing primitive — use it to reproduce a
 * known ID, or to check against the conformance vectors. It offers NO
 * uniqueness guarantee; for that, call hashid_minter_mint. */
hashid_status hashid_minter_mint_unchecked(const hashid_minter  *minter,
                                           const hashid_request *request,
                                           uint32_t              nonce,
                                           uint32_t              hash_length,
                                           char                **out);

/* ========================================================================== */
/* Resolution                                                                 */
/* ========================================================================== */

/* Turning a typed fragment like "8cd" back into "myproj-8cda" is not here.
   Minting asks "is this taken?", which any store answers with an indexed
   lookup; resolution needs a search, which is a different shape with a
   different cost. They ship apart, so a caller who only mints never writes a
   lookup they will not use. See crates/hashid-resolve. */

#ifdef __cplusplus
} /* extern "C" */
#endif

#endif /* HASHID_H */
