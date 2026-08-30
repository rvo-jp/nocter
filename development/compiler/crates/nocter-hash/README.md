# nocter-hash

## Responsibility

Provide the repository's deterministic cryptographic hashing operations without importing package,
installation, or executable-image policy.

## Contract

Consumers supply complete bytes or an ordered byte stream and receive an exact digest. The crate
does not read files, select package identities, or decide what content must be hashed.

## Invariants

- Equal bytes always produce the same cross-process result.
- Incremental chunk boundaries cannot change the resulting digest.
- Callers cannot replace content identity with platform or iteration order.
