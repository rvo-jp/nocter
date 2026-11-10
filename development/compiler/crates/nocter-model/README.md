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
- define-once sparse arenas over reserved domains, with persistent immutable views and exact
  single-writer ancestry; absent slots contain no provisional semantic value
- source-independent compile-time expression identities
- `ValueStatic` place keys linked to initializer declarations; the declaration authority validates
  the role before granting a global place, independently of code/function use
- immutable `FrozenData` literal DAGs for the closed value-access path, with topological ordered aggregate-component
  edges and shared component views; source typing, evaluation, nominal identity and layout remain
  outside this representation
- dependency-light capability-evidence identities owned by checking authorities
- deterministic symbols and package identities
- structural type interning and projections
- one builtin numeric profile and its mathematical integer bounds, independent of host word size
- future function-witness keys pairing a concrete declaration, symbolic formal input or symbolic
  result family or selected call application with an invocation view; projection preserves them
  without source names or runtime layout. Applications pair caller/operation/callee identities.
  Formal/result/application witnesses remain nonconcrete and may carry storage; named witnesses
  are concrete capture-free leaves. Canonical body replay and result closure retain one authority
- root/captured body code identities: `AccessCodeId` identifies reserved declarations and header
  type-expression code. `ValueExpression` identifies a required root by enclosing body and local
  slot without appending a declaration. `ValueCodeRoot` separates their checking/type branches;
  closure and application keys retain that complete root. `ValueCode` selects a root or captured
  body, independently of its source owner. A body-local expression has no function origin;
  declaration roles and callable witnesses remain declaration-owned facts
- body-owned closure code keys independent of runtime environments; data/schema owners, rather
  than the key or invocation view, establish capture Copy and destruction properties
- logical field paths with record-field identities, tuple positions and concrete u64 array indices, concatenation and
  prefix/overlap relations; the declaration owner validates each selection against its containing data schema, independently of memory offsets
  or host ordinals. Concrete indices do not encode runtime selector identity or prove a dynamic relation
- independent value-input operations, local binding kinds, transient operation-access descriptors,
  and distinct future code/input/field and body-local/selected-place/operation/control-flow-point identity domains;
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
