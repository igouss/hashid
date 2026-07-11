Feature: Parsing an id

  This is the crate's trust boundary. Every id arriving from outside — a CLI
  argument, an imported file, an API payload — comes through here, and what
  leaves cannot be malformed. A rejection names the rule that was broken.

  Rule: The prefix runs to the LAST hyphen, so prefixes may contain hyphens

    Scenario: A simple id
      When I parse "bd-abc123"
      Then parsing succeeds
      And the prefix is "bd"
      And the hash is "abc123"

    Scenario: A hyphenated prefix
      When I parse "bead-me-up-3e9"
      Then parsing succeeds
      And the prefix is "bead-me-up"
      And the hash is "3e9"

    Scenario: A hyphenated prefix with a word-like hash
      When I parse "my-proj-abcd"
      Then parsing succeeds
      And the prefix is "my-proj"
      And the hash is "abcd"

    Scenario: An external-style prefix carrying a colon
      When I parse "external:jira-123"
      Then parsing succeeds
      And the prefix is "external:jira"
      And the hash is "123"

    Scenario: An external-style prefix carrying a hash mark
      When I parse "ext:github#repo-456"
      Then parsing succeeds
      And the prefix is "ext:github#repo"
      And the hash is "456"

  Rule: A malformed id names the rule it broke

    Scenario: No separator at all
      When I parse "bdabc123"
      Then parsing fails because the separator is missing

    Scenario: Nothing before the separator
      When I parse "-abc123"
      Then parsing fails because the prefix is empty

    Scenario: Nothing after the separator
      When I parse "bd-"
      Then parsing fails because the hash is empty

    Scenario: An uppercase hash is not base36
      When I parse "bd-ABC"
      Then parsing fails because the hash is not base36

    Scenario: A child segment that is not a number
      When I parse "bd-abc123.x"
      Then parsing fails because a child segment is not a number

    Scenario: An empty child segment
      When I parse "bd-abc123."
      Then parsing fails because a child segment is not a number

  Scenario: A wide fallback hash is accepted
    When I parse "bd-abc123456789"
    Then parsing succeeds
    And the hash is "abc123456789"

  Scenario: The desperate fallback format still parses
    When I parse "bd-abc1234567891001"
    Then parsing succeeds

  Rule: Validity can be asked without paying for a parse

    Scenario: A well-formed id is reported valid
      When I check whether "bd-abc123.1" is a valid id
      Then it is reported valid

    Scenario: A malformed id is reported invalid
      When I check whether "invalid" is a valid id
      Then it is reported invalid
