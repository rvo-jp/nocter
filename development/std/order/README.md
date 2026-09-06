# Total Ordering

The compiler-checked [`std/order` contract](index.nct) separates the existence of `==` and `<`
from the stronger promise that they form one total order. `TotalOrder` has no runtime witness and no
method of its own. Its equality and strict-comparison prerequisites make the promised operations
available to generic code, while an explicit `impl TotalOrder` supplies the algebraic guarantee.

The standard library implements the contract for integers, Unicode scalar values, borrowed UTF-8
text, and owned strings. Floating-point values deliberately do not implement it because NaN is
unordered under ordinary comparison. Their `total_compare` methods instead return `Ordering` and
order all IEEE representations, including signed zero and NaN payloads.

Algorithms that require deterministic ordering, such as in-place slice sorting, require
`T impl TotalOrder`. An API that merely performs a caller-defined comparison may keep a structural
operator requirement instead.
