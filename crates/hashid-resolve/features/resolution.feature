Feature: Resolving what a human typed

  Short ids exist to be retyped, and nobody retypes the whole thing. Resolution
  walks four steps in order: the exact id, the id missing its prefix, a unique
  fragment of the hash, and then failure — either ambiguous or absent.

  Background:
    Given a resolver with default prefix "br"
    And the registry holds "br-abc123"
    And the registry holds "br-abd456"
    And the registry holds "br-xyz789"
    And the registry holds "br-abc123.1"
    And the registry holds "other-def111"

  Scenario: A complete id matches exactly
    When I resolve "br-abc123"
    Then resolution succeeds
    And the resolved id is "br-abc123"
    And the match was exact

  Scenario: A bare hash gets the default prefix
    When I resolve "abc123"
    Then resolution succeeds
    And the resolved id is "br-abc123"
    And the match was prefix-normalized

  Scenario: A unique fragment resolves
    When I resolve "xyz"
    Then resolution succeeds
    And the resolved id is "br-xyz789"
    And the match was a substring

  Scenario: A complete child id matches exactly
    When I resolve "br-abc123.1"
    Then resolution succeeds
    And the resolved id is "br-abc123.1"
    And the match was exact

  Scenario: Input is trimmed and case-folded before matching
    When I resolve "  BR-ABC123  "
    Then resolution succeeds
    And the resolved id is "br-abc123"

  Scenario: A fragment can be pinned to a namespace
    When I resolve "other-def"
    Then resolution succeeds
    And the resolved id is "other-def111"

  Rule: A fragment carrying a child path pins that child exactly

    Scenario: The child path must match, while the hash matches loosely
      Given the registry holds "br-64up6"
      And the registry holds "br-64up6.4"
      And the registry holds "br-64up6.5"
      When I resolve "4up6.4"
      Then resolution succeeds
      And the resolved id is "br-64up6.4"
      And the match was a substring

    Scenario: A sibling child is not a match
      Given the registry holds "br-64up6"
      And the registry holds "br-64up6.4"
      And the registry holds "br-64up6.5"
      When I resolve "4up6.9"
      Then resolution fails as not found

    Scenario: Without a child path, the parent and all its children match
      Given the registry holds "br-64up6"
      And the registry holds "br-64up6.4"
      And the registry holds "br-64up6.5"
      When I resolve "4up6"
      Then resolution fails as ambiguous
      And the ambiguity lists "br-64up6"
      And the ambiguity lists "br-64up6.4"
      And the ambiguity lists "br-64up6.5"

  Rule: Several fragments can be resolved in one go

    Scenario: Every fragment resolves
      When I resolve "abc123" and "xyz789"
      Then both fragments resolve
      And the batch resolved "br-abc123"
      And the batch resolved "br-xyz789"

    Scenario: One unresolvable fragment fails the whole batch
      When I resolve "abc123" and "zzz"
      Then resolution fails as not found

  Rule: Guessing wrong is worse than not guessing

    Scenario: A fragment matching two ids is ambiguous
      When I resolve "ab"
      Then resolution fails as ambiguous
      And the ambiguity lists "br-abc123"
      And the ambiguity lists "br-abd456"

    Scenario: A fragment matching nothing is not found
      When I resolve "zzz"
      Then resolution fails as not found

    Scenario: Empty input is refused
      When I resolve ""
      Then resolution fails as malformed

    Scenario: A registry that cannot answer aborts resolution
      Given a registry whose lookups fail
      When I resolve "br-abc123"
      Then resolution fails because the registry could not answer

  Rule: Substring matching can be switched off where guessing is a footgun

    Scenario: With substring matching off, a fragment no longer resolves
      Given substring matching is disabled
      When I resolve "xyz"
      Then resolution fails as not found

    Scenario: With substring matching off, a complete id still resolves
      Given substring matching is disabled
      When I resolve "br-xyz789"
      Then resolution succeeds
      And the resolved id is "br-xyz789"
