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
  edges, compact repeated/default-and-sparse-replacement nodes and shared component views;
  source typing, evaluation, nominal identity and layout remain
  outside this representation
- dependency-light capability-evidence identities owned by checking authorities
- deterministic symbols and package identities
- structural type interning and projections
- one builtin numeric profile and its mathematical integer bounds, independent of host word size
- one source-visible index-check disposition, reexported by checking, MIR and Machine rather than
  copied into separate enum definitions or conversion classifiers
- future function-witness keys pairing a concrete declaration, symbolic formal input or symbolic
  result family or selected call application with an invocation view; projection preserves them
  without source names or runtime layout. Applications pair caller/operation/callee identities.
  Formal/result/application witnesses remain nonconcrete and may carry storage; named witnesses
  are concrete capture-free leaves. Canonical body replay and result closure retain one authority
- root/captured body code identities: `AccessCodeId` identifies reserved declarations and header
  type-expression code. `ValueExpression` identifies a required root by enclosing body and local
  slot without appending a declaration. `ValueCodeRoot` separates their checking/type branches;
  closure keys retain that complete root. `ValueCode` selects a root or captured
  body, independently of its source owner. Applications retain that actual lexical `ValueCode`,
  so equal expression IDs in a root and its closures remain distinct. A body-local expression has no function origin;
  declaration roles and callable witnesses remain declaration-owned facts
- body-owned closure code keys independent of runtime environments; data/schema owners, rather
  than the key or invocation view, establish capture Copy and destruction properties
- one structural path algebra over record fields, tuple positions and parameterized array indices;
  concrete u64 paths serve declaration projection and ownership, while evaluated-index relations
  are supplied explicitly for symbolic recipes. Paths grant no schema, bounds or access evidence
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

## Structural Projection Relations

`ValueProjection<I>` and `ValuePath<I>` describe builtin structural selections with an index-key
domain supplied by their owner. `ValueField` and `FieldPath` are their concrete u64 domain, used by
declarations and the concrete resource ledger. Concrete prefix/overlap queries use the same
relation algorithm as symbolic recipes; no parallel path classifier is maintained.

`ValuePath::relation` consumes caller-established `IndexRelation` facts and reports equality,
ancestry, disjointness or possible overlap. Unknown index equality never proves containment or
disjointness on its own. A later distinct component can still prove separation inside homogeneous
array elements. Different component families provide no proof when reached by the comparison;
the declaration owner separately validates each path's shape and bounds on its selected root.
Physical offsets, including equal offsets of zero-sized data, do not enter this relation.

Recipe equality and ordering describe keys only. Even equal keys require an explicit evaluated
index fact: a source operation can run again after mutation or a loop edge. Symbolic paths have no
context-free prefix/overlap convenience API. Checking must establish facts in the same root,
activation and stable-value context; this model does not create snapshots, track their extents,
perform branch reasoning or admit runtime-selected source places. The
[selection closure](../../../design/compiler-realization.md#evaluated-array-selection-closure)
owns those remaining integration obligations.

`IndexBoundsCheck` transports Required, ProvenInBounds or ProvenTrap from checking. It grants no
schema or index fact by itself. Consumers must preserve the selected disposition rather than
strengthen it from a constant representation or target layout; each product separately qualifies
the continuations it can realize. The model does not derive or execute those proofs.

`ArrayRange<I>` describes a constant half-open interval or a before/inclusive-through/after span
around an existing checked index. `try_map_index` changes its backing identity without treating a
key as a number; `resolve` checks that an actual saved value remains below the containing length.
Through and after share the same safe successor at maximum usize counts and partition an array
without per-element state. These span operations create no initialization, ownership or source
bounds proof.
