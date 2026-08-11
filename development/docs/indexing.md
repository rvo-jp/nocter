# Index Selection and Lowering

Indexing has one semantic selection boundary in `typecheck/indexing.rs`. The selector receives the
target type, index type, requested readonly or readwrite access, lexical generic requirements, the
source-defined operator surface, and visible borrow coercions. It returns one direct
array/slice/`str` projection, one lexical requirement, or one source-defined callable. The latter
may be reached directly or through one borrow coercion. AST shape and source spelling are not
semantic fallbacks.

`TypecheckIndexPlan` is the immutable handoff to later stages. It records source spans, target,
index, and element types, access capability, projection kind, requirement identity, optional
conversion plan, and exact callable identity for a declaration. Fact collection records readonly
plans for value access and readwrite plans for assignment targets and `&+` borrows. Generic
specialization substitutes the plan and re-runs the same type-level selector against the concrete
resolver; `specialize_index_plan_across_resolvers` is the common resolver-view and precedence
service used by analysis and lowering. An unresolved requirement cannot reach native lowering.

Ordinary conversion facts contain the selected receiver coercion for nongeneric indexing. Generic
lowering obtains the specialized conversion from the index plan. Primitive leaves invoke a
coercion body at most once and reuse the existing checked projection. Declared leaves lower their
synthetic callable view through the ordinary static borrow-return ABI. Their bodies own bounds
policy; the caller does not add a projection or bounds check after receiving the element pointer.

`instance` stores equality and index declarations in one operator-member enum. Each declaration
provides a compiler-private `MethodDecl` view. Qualification, visibility, body joining, provenance,
reachability, specialization, static call resolution, editor identity, and generic substitutions
therefore reuse method infrastructure without exposing the reserved internal callable names as
members or completion candidates.

Aggregate element replacement uses a drop-aware staging service shared by primitive slice and
declared pointer destinations. It builds the replacement in an unowned temporary slot, copies the
old element from the checked destination, destroys that old value, and transfers the replacement
bytes into the element. Copy aggregates retain their direct-copy path; move-only elements do not
acquire a second lexical owner in either staging slot.

`BorrowSource::AggregateIndex` represents a dynamic fixed-array element address. It carries the
aggregate storage identity, base offset, stabilized index, length, and stride. The backend performs
the same bounds check and address calculation whether the borrow becomes an ordinary call argument
or the result of an index declaration body. This closes the earlier constant-index-only borrow
path without teaching source-defined operators about backend aggregate layout.

Integer binary lowering snapshots each deferred memory projection before lowering the next
operand. Indexed values can require scratch registers for checked address calculation, so leaving
such an operand deferred would let materializing the other operand overwrite its value and would
violate left-to-right evaluation. Constants and stable local locations remain direct values and do
not consume an unnecessary temporary.

The ownership model continues to describe an indexed borrow as a projection of the original
container place. A declaration call or coercion does not create independent storage: a borrow of
`values[index]` retains the `values` loan, and a readwrite projection requires both a readwrite
declaration and a writable source place. Source-declared `from` uses the common result-provenance
AST, parser, formatter, JSON, completion, and presentation services; inferable receiver provenance
normally remains omitted.

Editor occurrences attach the declaration's opening bracket to the same callable identity recorded
on each selected index expression. Hover, definition, references, and rename consume that identity;
completion derives missing equality, readonly-index, and readwrite-index templates from the
operator-member enum. Member completion filters reserved callable names, and semantic tokens retain
`self` plus the index binding as parameters rather than reconstructing ranges from punctuation.
