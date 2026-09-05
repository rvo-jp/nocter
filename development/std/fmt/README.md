# Formatting

The compiler-checked [`std/fmt.Format` declaration](index.nct) is the exact static interface
contract. [String Interpolation](../../../spec/language/sequences-and-text.md#string-interpolation)
defines how the language selects it. `Format` requires one recoverable append operation and derives
the aborting operation used by owned interpolation.

The distributed library conforms `str`, `String`, `bool`, and every built-in integer. A nominal
project type may implement the interface and build its representation with canonical members:
`output.try_push_str` for text and `value.try_format_into(output)` for nested formatted values.
`try_format_into` reports recoverable destination growth failure and may leave a successfully
appended prefix. The default `format_into` converts that failure to the ordinary allocation-abort
policy used by interpolation. Formatting dispatch is static and does not require a runtime
interface object.

`std/fmt` publishes the `Format` contract, not one append function for every built-in type. The
closed scalar append operations used by the old surface are replaced by each scalar's one
`try_format_into` implementation. Type-owned `try_to_string` and standard-library diagnostics call
that same method. This keeps decimal spelling under one implementation authority without
presenting a closed type matrix as a general formatting API.

Formatting and byte output deliberately remain distinct contracts. `Format` reports only mutation
failure from its owned `String` destination; `Writer` may report an I/O failure after publishing a
prefix outside the process. Their sole value boundary is well-formed `&str`. The standard library
does not define a second direct-to-writer formatting protocol or reinterpret a writer error as
allocation failure.
