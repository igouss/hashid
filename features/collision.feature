Feature: Escalating when an id is already taken

  The hash makes collisions rare; the registry check is what makes them
  impossible. When a candidate is taken the generator climbs a ladder — more
  nonces, then a longer hash, then a wider fallback — and if it truly runs out
  of room it says so.

  It never, under any circumstances, returns an id it has been told is taken.
  A duplicate id is silent data corruption; an error is merely loud.

  Background:
    Given the clock reads "2026-07-10T12:34:56.789012345Z"

  Scenario: Nothing is taken, so the first candidate wins
    Given a generator with prefix "bd"
    And the registry is empty
    When I mint an id for "Test Issue"
    Then minting succeeds
    And the id is one the registry had not seen

  Scenario: One taken candidate is stepped over
    Given a generator with prefix "bd"
    And the registry is empty
    And the registry already holds the candidate for "Test Issue" at nonce 0
    When I mint an id for "Test Issue"
    Then minting succeeds
    And the id is one the registry had not seen

  Scenario: Every candidate at the minimum length is taken, so the hash widens
    Given a generator with prefix "bd"
    And the registry is empty
    And the registry already holds every 3-character candidate for "Test Issue"
    When I mint an id for "Test Issue"
    Then minting succeeds
    And the id is one the registry had not seen
    And the id has a hash of 4 characters

  Scenario: The hash budget is spent, so the wide fallback takes over
    Given a generator with prefix "bd" and hash lengths 3 to 3
    And the registry is empty
    And the registry already holds every 3-character candidate for "Test Issue"
    When I mint an id for "Test Issue"
    Then minting succeeds
    And the id is one the registry had not seen
    And the id has a hash of 12 characters

  Scenario: Even the first fallback candidate is taken
    Given a generator with prefix "bd" and hash lengths 3 to 3
    And the registry is empty
    And the registry already holds every 3-character candidate for "Test Issue"
    And the registry already holds the 12-character fallback candidate for "Test Issue"
    When I mint an id for "Test Issue"
    Then minting succeeds
    And the id is one the registry had not seen

  Rule: A slugged id escalates the same way, and keeps its slug as long as it can

    Scenario: A taken slugged candidate widens the hash, slug intact
      Given a generator with prefix "proj"
      And the registry is empty
      And the registry already holds every 3-character candidate for "Test Issue" with slug "my-thing"
      When I mint an id for "Test Issue" with slug "my-thing"
      Then minting succeeds
      And the id is one the registry had not seen
      And the id has prefix "proj-my-thing"
      And the id has a hash of 4 characters

    Scenario: A saturated slugged namespace drops the slug rather than overrun the prefix
      Given a generator with prefix "proj" and hash lengths 3 to 3
      And the registry is empty
      And the registry already holds every 3-character candidate for "Test Issue" with slug "my-thing"
      When I mint an id for "Test Issue" with slug "my-thing"
      Then minting succeeds
      And the id is one the registry had not seen
      And the id has no slug

  Rule: The last resort splices the nonce into the hash itself

    Scenario: Every plain hash is taken, so the nonce joins the hash
      Given a generator with prefix "bd" and hash lengths 3 to 3
      And a registry in which every hash of 12 characters or fewer is taken
      When I mint an id for "Test Issue"
      Then minting succeeds
      And the id is one the registry had not seen
      # The wide fallback hash is 12 characters; the first nonce past the
      # thousand-collision threshold is 1001, spliced on to make 16.
      And the id has a hash of 16 characters
      And the id's hash ends with "1001"

  Rule: An honest failure beats a convenient duplicate

    Scenario: A registry in which everything is taken reports exhaustion
      Given a generator with prefix "bd" and hash lengths 3 to 3
      And a registry in which every id is already taken
      When I mint an id for "Test Issue"
      Then minting fails because the id space is exhausted
      # The full cost of the ladder: 10 candidates at the fixed hash length,
      # then 2001 wide-hash candidates (nonces 0 through 2000), then 1001
      # nonce-spliced ones (nonces 1001 through 2001).
      And the failure reports 3012 rejected candidates

    Scenario: A registry that cannot answer aborts the mint
      Given a generator with prefix "bd"
      And a registry whose lookups fail
      When I mint an id for "Test Issue"
      Then minting fails because the registry could not answer
