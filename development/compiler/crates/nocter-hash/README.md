# nocter-hash

## Responsibility

Provide the repository's deterministic cryptographic hashing operations without importing package,
installation, or executable-image policy.

## Contract

Consumers supply bytes and receive exact digest values or renderings. The crate does not read files,
select package identities, or decide what content must be hashed.

## Invariants

- Equal bytes always produce the same cross-process result.
- Callers cannot replace content identity with platform or iteration order.
