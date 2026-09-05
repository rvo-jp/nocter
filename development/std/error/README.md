# Recoverable Errors

The compiler-checked [`std/error` contract](index.nct) is the sole authority for the exact public
declaration and members of the built-in `error` type. The language-level `T!` outcome and
propagation rules belong to [Errors and Optionals](../../../spec/language/errors-and-optionals.md).

An error is an owned, move-only handle to immutable failure information. Construction snapshots the
code and message into storage independent of the input views. Adding context consumes the previous
handle, preserves its root code, snapshots the new message, and returns a new outer error node.
Code and message accessors return readonly views tied to the error handle; exact code comparison
returns an independent boolean.

Error codes are open UTF-8 strings. Standard-library codes use stable dotted names such as
`std.io.not_found`, `std.mem.out_of_memory`, and `std.process.invalid_encoding`; applications and
packages may define their own prefixes. No `Error` or `ErrorCode` compatibility alias exists.

Dynamic construction is infallible at the source boundary. If private error-storage allocation
cannot continue, execution terminates. A recoverable allocation-failure path uses a prebuilt static
error and therefore cannot recursively allocate another error while reporting exhaustion.

Public standard-library operations expose only `error` through `T!`. Target-specific raw error
records and helpers that construct standard errors remain private implementation details.
