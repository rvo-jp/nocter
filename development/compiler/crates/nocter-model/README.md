# nocter-model

## Responsibility

Own dependency-light semantic identity domains, package/target identities, symbols, structural type
keys, immutable type snapshots, and type-construction authority.

## Contract

Semantic stages exchange model IDs only with the immutable authority that gives them meaning. The
crate publishes read-only type products and owner-scoped construction operations; it does not know
syntax, source ranges, declaration storage, editor features, or machine layout.

## Internal Responsibilities

- dense semantic ID domains and arenas
- source-independent compile-time expression identities
- dependency-light capability-evidence identities owned by checking authorities
- deterministic symbols and package identities
- structural type interning and projections
- structural callable capabilities and independent authored allocation/nonblocking guarantee
  identity
- canonical structural callable input/result provenance and one order-independent graph implication
  operation
- exact-lineage type transactions
- persistent closure identity sequences

## Invariants

- Type identity never depends on spelling, source order, or source location.
- Callable guarantee differences participate in type identity and survive every projection and
  transaction.
- Callable provenance implication requires every union branch, treats cycles simultaneously, and
  cannot be replaced by source-order traversal or one-path reachability.
- A read-only `TypeStore` cannot open a transaction.
- Sibling or stale authorities cannot exchange or commit bare identities.
- An append-only descendant proves an exact immutable type prefix through persistent value
  identity; structural equality from an unrelated store cannot forge that relationship.
- `TypeCursor` exposes only a stable prefix boundary and relative suffix positions, leaving the
  representation of `TypeId` private.
- Storage implementation remains private to the semantic owner.
