# Native Assertions

The compiler-checked [`std/testing` contract](index.nct) is the sole authority for exact public
declarations. This guide defines the assertion behavior shared by native tests and ordinary source
without repeating signatures.

Assertions are ordinary fallible functions, not compiler intrinsics. A failed boolean assertion
uses the stable code `std.testing.assertion_failed`; a failed equality assertion uses
`std.testing.not_equal`. Their static messages require no allocation, so constructing the failure
cannot itself become an allocation failure.

Equality assertions borrow both values and select the same structural equality operation as an
ordinary comparison expression. They neither consume their operands nor introduce a separate
equality interface. Failure uses normal `error` propagation and follows the same cleanup rules as
any other fallible call.
