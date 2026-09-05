# Slices

The compiler-checked [`slice` module contract](index.nct) is the sole authority for exact public
declarations on the built-in `[T]` type. This chapter defines their longer observable behavior
without repeating signatures.

Readonly lookup and indexing borrow initialized elements without transferring ownership. Equality,
containment, and position compare borrowed elements only when the selected element type supplies
the required equality operation. Strict ordering is lexicographic and relies only on the selected
element order. [`Vec<T>`](../vec/README.md) reaches these operations through one-step borrow
coercions, so direct Vec indexing and slice indexing select the same implementation.

Readwrite slices order elements in place with constant auxiliary storage and `O(n log n)`
worst-case comparisons and moves. No later element is strictly less than an earlier element after
sorting. Equivalent elements may change relative order. Rearrangement transfers values without
copying or destroying them. The selected comparison may allocate or trap, so the operation does
not claim a transitive `noalloc` guarantee.

Pointer observation returns a non-owning address and grants no additional access or lifetime.
Internal raw-view construction remains package-private and is not a public slice operation.
