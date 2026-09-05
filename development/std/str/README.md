# Borrowed Text

The compiler-checked [`str` module contract](index.nct) is the sole authority for exact public
declarations. Owned UTF-8 storage belongs to [`String`](../string/README.md); this module owns
borrowed byte-oriented search, validated views, iteration, and owned results derived from `str`.

Search and range indices are UTF-8 byte offsets. `get_range` returns `none` when `start > end`, an
endpoint is outside the input, or an endpoint divides a UTF-8 encoding. Empty ranges and the full
input range are valid. The result borrows `self`; it never reconstructs provenance from an integer
address.

`strip_prefix` and `strip_suffix` compare exact UTF-8 bytes and return a view into `self`. The affix
is an input to comparison, not a storage origin of the returned view. An empty affix matches and
returns the complete input.

`split_once` finds the first exact separator and returns the borrowed prefix and suffix around it.
The separator is excluded from both positions. An empty separator returns the empty view at the
start followed by the complete input; a missing separator returns `none`. Both returned views
borrow only `self`, and the operation performs no allocation.

ASCII whitespace is exactly space (`0x20`), horizontal tab (`0x09`), line feed (`0x0A`), vertical
tab (`0x0B`), form feed (`0x0C`), and carriage return (`0x0D`). ASCII trimming removes matching
bytes only from the named edges and returns the largest remaining borrowed view. It performs no
allocation and does not apply Unicode property tables or normalization. An all-whitespace input
returns the empty view positioned at the end of the input and still borrows that input.

`split_views` rejects an empty separator with `std.str.empty_separator`. Otherwise it yields the
same component boundaries as owned `split`, including empty components for empty input, adjacent
separators, and leading or trailing separators. `SplitIter` retains both text and separator while
it can advance. Each yielded component is a borrowed `&str` in source order.

`lines` recognizes LF and CRLF terminators. It omits each terminator, removes CR only immediately
before LF, preserves every other CR, yields no item for empty input, and does not add an empty item
after a final terminator. `LinesIter` retains its input and allocates no storage.

`repeat` concatenates the complete input `count` times. Zero repetitions or empty input produce an
empty owned string. The operation checks complete required capacity before mutation and never wraps
an unrepresentable size.

`replace_all` rejects an empty pattern with `std.str.empty_pattern`. It scans left to right,
replaces non-overlapping matches, and resumes after each complete pattern. Replacement text is
copied verbatim and is not searched recursively. A missing pattern produces an independent copy of
the input. The result is always well-formed owned UTF-8.

`str` defines exact byte equality and lexical strict ordering once. `String` reaches those
operations through readonly coercion and does not duplicate their algorithms. Unicode scalar
properties and transformations are defined by [Unicode Text and Scalars](../char/README.md).
