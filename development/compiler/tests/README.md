# Compiler Test Corpora

This directory owns reusable non-Rust inputs consumed by compiler tests. It is not a second test
runner and does not own expected compiler behavior; each consuming crate test owns its assertions.

## Fixture Classes

- [`fixtures/v2-access/`](fixtures/v2-access/README.md) contains future access-model requirements
  for an explicitly supplied private-module test goal. Production syntax and declaration tests consume the
  manifest; body checking remains pending. Its inventory checker validates only
  registration/ranges/shapes. Do not include sources in a released-language glob or count parsing
  as semantic acceptance.

- `fixtures/syntax/` contains parser and semantic-boundary source corpora plus structural snapshots.
- `fixtures/standard/` contains source programs used by command and editor integration tests against
  the physical standard library.
- `fixtures/native/` contains source programs compiled through the complete native session. The
  `session/` subtree holds multi-file or responsibility-specific inputs selected by
  `nocter-native-session` tests.

Keep a short inline source in a Rust test when its exact spelling is the assertion or when the test
constructs it dynamically. Put a reusable program or a substantial static package here so source
formatting, module boundaries, and ownership remain visible without reading an escaped Rust string.

The [grammar conformance matrix](grammar.md) derives syntax and semantic-boundary coverage from the
normative grammar without becoming another source-language authority.
