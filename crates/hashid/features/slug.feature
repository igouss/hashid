Feature: Embedding a human-readable slug

  A slug turns "proj-8cda" into "proj-survey-the-users-8cda": recognisable in a
  commit message, still unique. The hash is always appended — two records may
  share a slug, so a slug can never carry uniqueness on its own.

  Background:
    Given the clock reads "2026-07-10T12:34:56.789012345Z"
    And a generator with prefix "proj"
    And the registry is empty

  Scenario: A clean slug is embedded verbatim
    When I mint an id for "Whatever" with slug "survey-the-users"
    Then minting succeeds
    And the id starts with "proj-survey-the-users-"
    And the id has prefix "proj-survey-the-users"
    And the id has a non-empty hash

  Scenario: A messy slug is normalized
    When I mint an id for "Whatever" with slug "Hello World!"
    Then minting succeeds
    And the id starts with "proj-hello-world-"

  Scenario: Runs of punctuation collapse to a single hyphen
    When I mint an id for "Whatever" with slug "a   b/c.d!!e"
    Then minting succeeds
    And the id starts with "proj-a-b-c-d-e-"

  Scenario: Non-ASCII characters are dropped, not transliterated
    When I mint an id for "Whatever" with slug "café-résumé"
    Then minting succeeds
    And the id starts with "proj-caf-r-sum-"

  Scenario: An over-long slug is capped and left without a trailing hyphen
    When I mint an id for "Whatever" with a slug of 60 letters
    Then minting succeeds
    And the slug portion of the id is 48 characters
    And the slug portion of the id does not end with a hyphen

  Rule: A slug that cannot be honoured is dropped, never faked

    Scenario: A slug that normalizes to nothing falls back to a hash-only id
      When I mint an id for "Whatever" with slug "!!!"
      Then minting succeeds
      And the id has prefix "proj"
      And the id has a non-empty hash

    Scenario: An empty slug falls back to a hash-only id
      When I mint an id for "Whatever" with slug ""
      Then minting succeeds
      And the id has prefix "proj"

    Scenario: A slug with no room left in the prefix budget is dropped
      Given a generator whose prefix already fills the prefix budget
      When I mint an id for "Whatever" with slug "slug"
      Then minting succeeds
      And the id has no slug

  Scenario: A slugged id round-trips through the parser
    When I mint an id for "Whatever" with slug "survey-the-users"
    Then the id parses back to itself
