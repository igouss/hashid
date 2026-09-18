Feature: Prefixes

  The prefix is the namespace an id lives in. Human input gets normalized into
  one; machine input must already be one.

  Rule: Normalizing is lenient, because humans type it

    Scenario: Case, whitespace, and punctuation are cleaned up
      When I normalize the prefix "  Project-Name_2!  "
      Then the prefix normalizes to "project-name_2"

    Scenario: A trailing separator is stripped, so no id gets a doubled hyphen
      When I normalize the prefix "proj-"
      Then the prefix normalizes to "proj"

    Scenario: A prefix with nothing usable in it is refused, not defaulted
      When I normalize the prefix "!!!"
      Then normalizing fails because the prefix is unusable

  Rule: Parsing is strict, because machines wrote it

    Scenario: A legal prefix is accepted unchanged
      When I parse the prefix "my-proj"
      Then the prefix parses to "my-proj"

    Scenario: An uppercase prefix is rejected rather than quietly folded
      When I parse the prefix "BD"
      Then parsing the prefix fails

  Rule: Long prefixes abbreviate to something a human will type

    Scenario: A short prefix is left alone
      When I abbreviate the prefix "proj"
      Then the prefix abbreviates to "proj"

    Scenario: A multi-word prefix collapses to its initials
      When I abbreviate the prefix "My_Project-Name"
      Then the prefix abbreviates to "mpn"

    Scenario: A long single word keeps its first three characters
      When I abbreviate the prefix "superlongname"
      Then the prefix abbreviates to "sup"

  Rule: An id may be required to belong to a namespace you accept

    Scenario: The expected prefix is accepted
      Given the id "bd-abc123"
      When I require the prefix "bd"
      Then the prefix requirement is met

    Scenario: An explicitly allowed prefix is accepted
      Given the id "bd-abc123"
      When I require the prefix "other" while also allowing "bd"
      Then the prefix requirement is met

    Scenario: An unknown prefix is rejected
      Given the id "bd-abc123"
      When I require the prefix "other"
      Then the prefix requirement is not met
