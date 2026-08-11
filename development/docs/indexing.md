# Index Selection and Lowering

Indexing has one semantic selection boundary in `typecheck/indexing.rs`. The selector receives the
target type, index type, requested readonly or readwrite access, lexical generic requirements, and
resolved coercion surface. It returns one direct array/slice/`str` projection, one lexical
requirement, or one accessible receiver coercion to a built-in projection. AST shape and source
spelling are not semantic fallbacks.

`TypecheckIndexPlan` is the immutable handoff to later stages. It records source spans, target,
index, and element types, access capability, projection kind, requirement identity, and the
optional conversion plan. Fact collection records readonly plans for value access and readwrite
plans for assignment targets and `&+` borrows. Generic specialization substitutes the plan and
re-runs the same type-level selector against the concrete resolver; an unresolved requirement
cannot reach native lowering.

Ordinary conversion facts contain the selected receiver coercion for nongeneric indexing. Generic
lowering obtains the specialized conversion from the index plan. Slice lowering then invokes the
coercion body exactly once and reuses the existing checked slice projection. `Vec<T>` therefore
does not own a parallel indexing implementation or bounds-check path.

Aggregate element replacement uses a separate drop-aware staging service. It evaluates and
materializes the selected index before the replacement, builds the replacement in an unowned
temporary slot, copies the old element through the checked slice projection, destroys that old
value, and transfers the replacement bytes into the element. An out-of-bounds index traps before
destruction or overwrite. Copy aggregates retain their cheaper direct-copy path; move-only
elements do not acquire a second lexical owner in either staging slot.

Integer binary lowering snapshots each deferred memory projection before lowering the next
operand. Indexed values can require scratch registers for checked address calculation, so leaving
such an operand deferred would let materializing the other operand overwrite its value and would
violate left-to-right evaluation. Constants and stable local locations remain direct values and do
not consume an unnecessary temporary.

The ownership model continues to describe an indexed borrow as a projection of the original
container place. A coercion does not create independent storage: a borrow of `values[index]`
retains the `values` loan, and a readwrite projection requires a writable source place.
