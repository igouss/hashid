Feature: Configuring the id format

  The entropy policy is the one thing a caller can get wrong at construction
  time rather than at use time. A range that cannot be satisfied, or a
  "probability" outside [0,1], is refused up front rather than quietly clamped —
  a clamped bound would silently mint ids at a width nobody asked for.

  Rule: Hash length bounds must be a satisfiable range

    Scenario: A sane range is accepted
      When I configure hash lengths 4 to 10
      Then the configuration is accepted

    Scenario: A zero lower bound is rejected
      When I configure hash lengths 0 to 8
      Then the configuration is rejected as invalid hash bounds

    Scenario: An inverted range is rejected
      When I configure hash lengths 5 to 3
      Then the configuration is rejected as invalid hash bounds

    Scenario: An upper bound past the grammar's ceiling is rejected
      When I configure hash lengths 3 to 41
      Then the configuration is rejected as invalid hash bounds

    Scenario: The grammar's ceiling itself is accepted
      When I configure hash lengths 3 to 40
      Then the configuration is accepted

    Scenario: A single fixed length is accepted
      When I configure hash lengths 6 to 6
      Then the configuration is accepted

  Rule: The collision bound must be a probability

    Scenario: A sane probability is accepted
      When I configure a collision probability of 0.5
      Then the configuration is accepted

    Scenario: Zero is rejected
      When I configure a collision probability of 0.0
      Then the configuration is rejected as an invalid probability

    Scenario: One is rejected
      When I configure a collision probability of 1.0
      Then the configuration is rejected as an invalid probability

    Scenario: A negative probability is rejected
      When I configure a collision probability of -0.5
      Then the configuration is rejected as an invalid probability
