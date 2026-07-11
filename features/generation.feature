Feature: Minting an id from content

  An id is derived from the thing it names, not from a counter. The same
  content at the same instant always yields the same id; the hash widens on its
  own as the dataset grows.

  Background:
    Given the clock reads "2026-07-10T12:34:56.789012345Z"
    And a generator with prefix "proj"

  Scenario: Minting into an empty registry
    Given the registry is empty
    When I mint an id for "Ship the thing"
    Then minting succeeds
    And the id has prefix "proj"
    And the id has a hash of 3 characters
    And the id is a root id

  Scenario: The same content at the same instant mints the same id
    Given the registry is empty
    When I mint an id for "Ship the thing"
    And I mint a second id for "Ship the thing"
    Then the two ids are identical

  Scenario: Different content mints a different id
    Given the registry is empty
    When I mint an id for "Ship the thing"
    And I mint a second id for "Ship a different thing"
    Then the two ids differ

  Scenario: A description is part of the content
    Given the registry is empty
    When I mint an id for "Ship the thing"
    And I mint a second id for "Ship the thing" described as "but carefully"
    Then the two ids differ

  Scenario: A creator is part of the content
    Given the registry is empty
    When I mint an id for "Ship the thing"
    And I mint a second id for "Ship the thing" created by "iouri"
    Then the two ids differ

  Rule: The hash is sized to the population it must stay unique within

    Scenario: Zero existing ids need only the minimum hash
      Given the registry is empty
      And a population of 0
      When I mint an id for "Ship the thing"
      Then the id has a hash of 3 characters

    Scenario: A hundred existing ids still need only the minimum hash
      Given the registry is empty
      And a population of 100
      When I mint an id for "Ship the thing"
      Then the id has a hash of 3 characters

    Scenario: The last population that still fits in the minimum hash
      Given the registry is empty
      And a population of 163
      When I mint an id for "Ship the thing"
      Then the id has a hash of 3 characters

    Scenario: One id past that bound, and the hash widens
      Given the registry is empty
      And a population of 164
      When I mint an id for "Ship the thing"
      Then the id has a hash of 4 characters

    Scenario: A hundred thousand existing ids need a wider hash
      Given the registry is empty
      And a population of 100000
      When I mint an id for "Ship the thing"
      Then the id has a hash of 7 characters

    Scenario: A saturating population stops at the configured maximum
      Given the registry is empty
      And a population of 100000000
      When I mint an id for "Ship the thing"
      Then the id has a hash of 8 characters
