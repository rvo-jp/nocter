# Operating-System Boundary

The checked [`std/internal/os` contract](index.nct) owns the package-internal portable records and
conversion helpers between target adapters and public standard-library modules. This is an
implementation contract, not a user-facing standard-library API.

Target adapters translate raw syscall results into one common result model and preserve an unknown
numeric error code without misclassifying it. Higher-level filesystem, I/O, and process modules
consume only the portable classification and raw code; they do not interpret register positions,
carry flags, descriptor ordering, close-on-exec flags, or wait-status bits.

Ordinary syscall results carry one successful value and one error code. The Darwin subprocess
operations that produce two success words use a distinct pair-result shape, so the common syscall
record does not acquire an unused field. Both success words are zero on failure; the error code is
zero on success.

The compiler retains the inherited environment-vector address as immutable process-entry context.
A source-private process primitive exposes only that opaque address. Standard-library source owns
entry decoding and inheritance policy.

Public validation, retry policy, partial-transfer handling, handle ownership, and operation
semantics remain in `std/io`, `std/fs`, and `std/process`. This module supplies target facts and does
not decide their public policy.
