Feature: Hierarchical ids

  A child id extends its parent with a dotted path: "bd-abc123.1", then
  "bd-abc123.1.2". The path is structural — the parent is recoverable from the
  child without a lookup.

  Scenario: A root id has no parent
    When I parse "bd-abc123"
    Then parsing succeeds
    And the id is a root id
    And the id has depth 0
    And the id has no parent

  Scenario: A child knows its parent
    When I parse "bd-abc123.1"
    Then parsing succeeds
    And the id is not a root id
    And the id has depth 1
    And the parent is "bd-abc123"

  Scenario: A grandchild knows its immediate parent
    When I parse "bd-abc123.1.2"
    Then parsing succeeds
    And the id has depth 2
    And the parent is "bd-abc123.1"

  Scenario: Descent is transitive
    When I parse "bd-abc123.1.2"
    Then the id is a descendant of "bd-abc123"
    And the id is a descendant of "bd-abc123.1"

  Scenario: An unrelated id is not an ancestor
    When I parse "bd-abc123.1"
    Then the id is not a descendant of "bd-xyz789"

  Scenario: An id is not a descendant of itself
    When I parse "bd-abc123"
    Then the id is not a descendant of "bd-abc123"

  Scenario: A child id renders back to the string it came from
    When I parse "bd-abc123.1.2"
    Then the id parses back to itself
