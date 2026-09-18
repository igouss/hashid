/*
 * Exercises every exported function through the real shared library.
 *
 * This is the gate that keeps hashid.h honest. C does not check a declaration
 * against the definition across an ABI, so a header that drifts compiles
 * happily and then reads the wrong bytes. Calling every function and asserting
 * on values catches that; tools/check-c-abi.sh adds the symbol comparison that
 * catches a declaration with no definition at all.
 */

#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

#include "hashid.h"

static int checks = 0;

#define CHECK(cond)                                                            \
    do {                                                                       \
        checks++;                                                              \
        if (!(cond)) {                                                         \
            fprintf(stderr, "FAIL %s:%d: %s\n", __FILE__, __LINE__, #cond);    \
            if (hashid_last_error_message() != NULL) {                         \
                fprintf(stderr, "     last error: %s\n",                       \
                        hashid_last_error_message());                          \
            }                                                                  \
            return 1;                                                          \
        }                                                                      \
    } while (0)

#define CHECK_STR(actual, expected)                                            \
    do {                                                                       \
        checks++;                                                              \
        if ((actual) == NULL || strcmp((actual), (expected)) != 0) {           \
            fprintf(stderr, "FAIL %s:%d: expected \"%s\", got \"%s\"\n",       \
                    __FILE__, __LINE__, (expected),                            \
                    (actual) == NULL ? "(null)" : (actual));                   \
            return 1;                                                          \
        }                                                                      \
    } while (0)

/* The instant every golden value in this file is frozen at. */
static const int64_t FROZEN = 1783686896789012345LL;

static hashid_request frozen_request(void) {
    hashid_request request = {
        .title = "Test Issue", .description = "Description", .creator = "user",
        .created_at_nanos = FROZEN, .population = 0,
    };
    return request;
}

/* ------------------------------------------------------------------------- */
/* The caller's store. There is no other kind: the library ships none.        */
/* ------------------------------------------------------------------------- */

typedef struct {
    char   ids[8][64];
    size_t len;
    int    broken;     /* non-zero: report "I do not know" */
    int    calls;      /* how many times the minter asked   */
} fake_store;

static void remember(fake_store *store, const char *id) {
    snprintf(store->ids[store->len++], sizeof store->ids[0], "%s", id);
}

static int32_t fake_contains(void *user_data, const char *id, const char **error) {
    fake_store *store = (fake_store *)user_data;
    store->calls++;
    if (store->broken) {
        *error = "the fake database is on fire";
        return -1;
    }
    for (size_t i = 0; i < store->len; i++) {
        if (strcmp(store->ids[i], id) == 0) {
            return 1;
        }
    }
    return 0;
}

/* ------------------------------------------------------------------------- */

static int test_free_functions(void) {
    CHECK(hashid_version() != NULL);
    CHECK_STR(hashid_spec_version(), "1.0.0");
    CHECK(hashid_now_nanos() > FROZEN);

    hashid_id parsed;
    CHECK(hashid_parse_id("bead-me-up-3e9.1.2", &parsed) == HASHID_OK);
    CHECK_STR(parsed.text, "bead-me-up-3e9.1.2");
    CHECK_STR(parsed.prefix, "bead-me-up"); /* split at the LAST '-' */
    CHECK_STR(parsed.hash, "3e9");
    CHECK(parsed.child_path_len == 2);
    CHECK(parsed.child_path[0] == 1 && parsed.child_path[1] == 2);
    hashid_id_free(&parsed);
    CHECK(parsed.text == NULL); /* freeing zeroes the struct */

    hashid_id root;
    CHECK(hashid_parse_id("br-8cda", &root) == HASHID_OK);
    CHECK(root.child_path_len == 0 && root.child_path == NULL);
    hashid_id_free(&root);

    /* Near-miss: uppercase is rejected, not folded, and the reason says so. */
    hashid_id rejected;
    CHECK(hashid_parse_id("bd-ABC", &rejected) == HASHID_ERR_MALFORMED);
    CHECK(hashid_last_error_reason() == HASHID_REASON_HASH_CHARSET);
    CHECK(hashid_parse_id(NULL, &rejected) == HASHID_ERR_NULL_ARGUMENT);
    CHECK(hashid_parse_id("br-abc", NULL) == HASHID_ERR_NULL_ARGUMENT);

    CHECK(hashid_is_valid_id("br-8cda"));
    CHECK(!hashid_is_valid_id("br-8CDA"));
    CHECK(!hashid_is_valid_id(NULL));

    char *out = NULL;
    CHECK(hashid_normalize_prefix("  Project-Name_2!  ", &out) == HASHID_OK);
    CHECK_STR(out, "project-name_2");
    hashid_string_free(out);
    CHECK(hashid_normalize_prefix("!!!", &out) == HASHID_ERR_UNUSABLE_PREFIX);
    return 0;
}

static int test_minting(void) {
    hashid_config     config = {"br", 3, 8, 0.25};
    hashid_minter *minter = NULL;
    CHECK(hashid_minter_with_config(&config, &minter) == HASHID_OK);

    hashid_request request = frozen_request();
    char          *id = NULL;

    CHECK(hashid_minter_mint_unchecked(minter, &request, 0, 3, &id) == HASHID_OK);
    CHECK_STR(id, "br-grz");
    hashid_string_free(id);
    CHECK(hashid_minter_mint_unchecked(minter, &request, 1, 3, &id) == HASHID_OK);
    CHECK_STR(id, "br-2cs");
    hashid_string_free(id);
    /* Near-miss: zero is not a length, and 41 is past the grammar. */
    CHECK(hashid_minter_mint_unchecked(minter, &request, 0, 0, &id)
          == HASHID_ERR_INVALID_HASH_LENGTH);
    CHECK(hashid_minter_mint_unchecked(minter, &request, 0, 41, &id)
          == HASHID_ERR_INVALID_HASH_LENGTH);

    /* An empty store: nonce 0 wins, asked exactly once. */
    fake_store empty_backing = {0};
    CHECK(hashid_minter_mint(minter, &request, fake_contains, &empty_backing, &id) == HASHID_OK);
    CHECK(empty_backing.calls == 1);
    CHECK_STR(id, "br-grz");
    hashid_string_free(id);

    /* A store holding it: the ladder steps to nonce 1 rather than hand back an
       ID it has been told is taken. */
    fake_store backing = {0};
    remember(&backing, "br-grz");
    CHECK(hashid_minter_mint(minter, &request, fake_contains, &backing, &id) == HASHID_OK);
    CHECK_STR(id, "br-2cs");

    /* Recording it is your write, into your store — and the next mint sees it.
       The library never writes to your set, so this is the only way it can. */
    remember(&backing, id);
    hashid_string_free(id);

    CHECK(hashid_minter_mint(minter, &request, fake_contains, &backing, &id) == HASHID_OK);
    CHECK_STR(id, "br-qwx");
    hashid_string_free(id);

    CHECK(hashid_minter_mint_with_slug(minter, &request, "Survey The Users",
                                       fake_contains, &backing, &id) == HASHID_OK);
    CHECK_STR(id, "br-survey-the-users-grz");
    hashid_string_free(id);

    /* A store that cannot answer aborts the mint rather than being guessed at. */
    backing.broken = 1;
    CHECK(hashid_minter_mint(minter, &request, fake_contains, &backing, &id) == HASHID_ERR_STORE);
    CHECK(strstr(hashid_last_error_message(), "on fire") != NULL);
    backing.broken = 0;

    /* A prefix is all a minter needs, and it is normalized for you. */
    hashid_minter *lenient = NULL;
    CHECK(hashid_minter_new("  Project-Name_2!  ", &lenient) == HASHID_OK);
    CHECK(hashid_minter_mint(lenient, &request, fake_contains, &empty_backing, &id) == HASHID_OK);
    CHECK(strncmp(id, "project-name_2-", 15) == 0);
    hashid_string_free(id);
    hashid_minter_free(lenient);

    /* Near-miss: a prefix with nothing usable in it is refused, not invented. */
    hashid_minter *unusable = NULL;
    CHECK(hashid_minter_new("!!!", &unusable) == HASHID_ERR_UNUSABLE_PREFIX);
    CHECK(hashid_minter_new(NULL, &unusable) == HASHID_ERR_NULL_ARGUMENT);

    /* A number outside its field's range is refused, never clamped: a
       saturating conversion would turn each of these into a plausible value
       and say nothing. */
    hashid_minter *unused_bounds = NULL;
    hashid_config  huge_bounds = {"br", 3, 4294967295u, 0.25};
    hashid_status  bounds = hashid_minter_with_config(&huge_bounds, &unused_bounds);
    CHECK(bounds == HASHID_ERR_INVALID_HASH_BOUNDS || bounds == HASHID_ERR_OUT_OF_RANGE);

    /* Bad configuration is refused before any ID is minted. */
    hashid_minter *unused = NULL;
    hashid_config  bad_prefix = {"BR", 3, 8, 0.25};
    CHECK(hashid_minter_with_config(&bad_prefix, &unused) == HASHID_ERR_MALFORMED);
    hashid_config bad_bounds = {"br", 9, 3, 0.25};
    CHECK(hashid_minter_with_config(&bad_bounds, &unused) == HASHID_ERR_INVALID_HASH_BOUNDS);
    hashid_config bad_prob = {"br", 3, 8, 1.5};
    CHECK(hashid_minter_with_config(&bad_prob, &unused) == HASHID_ERR_INVALID_COLLISION_PROB);

    hashid_minter_free(minter);
    return 0;
}

static int test_frees_accept_null(void) {
    /* Freeing NULL, and double-freeing a zeroed struct, are both no-ops. */
    hashid_string_free(NULL);
    hashid_id_free(NULL);
    hashid_minter_free(NULL);
    return 0;
}

int main(void) {
    if (test_free_functions()) return 1;
    if (test_minting()) return 1;
    if (test_frees_accept_null()) return 1;

    printf("ok — %d checks passed against libhashid_c\n", checks);
    return 0;
}
