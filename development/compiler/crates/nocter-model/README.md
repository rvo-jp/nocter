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
- immutable `FrozenData` literal DAGs for the closed value-access path, with topological component
  edges and shared component views; source typing, evaluation, nominal identity and layout remain
  outside this representation
- dependency-light capability-evidence identities owned by checking authorities
- deterministic symbols and package identities
- structural type interning and projections
- future function-witness keys pairing a concrete declaration, symbolic formal input or symbolic
  result family or selected call application with an invocation view; projection preserves them
  without source names or runtime layout. Applications pair caller/operation/callee identities.
  Formal/result/application witnesses remain nonconcrete and may carry storage; named witnesses
  are concrete capture-free leaves. Canonical body replay and result closure retain one authority
- named/captured body code identities, with a derived named source owner used only for source reuse
- body-owned closure code keys independent of runtime environments; data/schema owners, rather
  than the key or invocation view, establish capture Copy and destruction properties
- logical field paths with concatenation and prefix/overlap relations; the declaration owner validates each field
  against its containing data schema, independently of memory offsets
- independent value-input operations, local binding kinds, transient operation-access descriptors,
  and distinct future callable/input/field and body-local/selected-place/operation/control-flow-point identity domains;
  these are not aliases for borrow capability or an implicit permission-coercion ordering
- a distinct private instance identity domain for the target owner's root-closed value program
- structural callable capabilities and independent authored allocation/nonblocking guarantee
  identity
- canonical structural callable input/result provenance and one order-independent graph implication
  operation
- exact-lineage type transactions
- persistent closure identity sequences

## Invariants

- Type identity never depends on spelling, source order, or source location.
- A generic application is one explicit authored-order sequence of type-or-constant values. The
  model exposes no type-only conversion that could silently omit a constant argument.
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
