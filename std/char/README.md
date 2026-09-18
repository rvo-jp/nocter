# Unicode Text and Scalars

This chapter extends the scalar foundation without changing the meaning of `char`, UTF-8
byte offsets, or existing ASCII-specific APIs.

## Unicode Version

Unicode-dependent behavior is fixed to the final
[Unicode Standard and Unicode Character Database version 17.0.0](https://www.unicode.org/versions/Unicode17.0.0/).
Draft or beta data, including Unicode 18.0.0 draft data, is not a valid input.
Changing the Unicode version changes observable program behavior and therefore requires a later
Nocter release.

The public contract uses Unicode properties and default case conversion only where this chapter
names them. It does not silently reinterpret existing byte-oriented search, slicing, length, or
ASCII operations.

## Core Scalar Operations

The compiler-checked [`char` contract](index.nct) is the sole authority for exact construction,
observation, comparison, and property-query declarations. `char.from_u32` returns `none` for a
surrogate or value above `U+10FFFF`; `code_point` exposes the exact scalar value. Equality and
ordering compare scalar values. `utf8_len` returns 1, 2, 3, or 4. ASCII classification recognizes
only the corresponding ASCII range and does not claim Unicode property or locale behavior.

`char` implements `Hash` using its scalar value and `Format` by appending its exact UTF-8 encoding.
Equal values therefore hash equally regardless of their authored literal spelling.

## Unicode Character Properties

The standard `char` instance adds the allocation-free property queries declared by its
compiler-checked [public contract](index.nct).

`is_whitespace` uses the Unicode `White_Space` property. `is_alphabetic`, `is_lowercase`, and
`is_uppercase` use the corresponding derived core properties. `is_decimal_digit` is true exactly
for general category `Decimal_Number` (`Nd`); it is narrower than the Unicode numeric property and
does not return a numeric value.

Existing `is_ascii` and `is_ascii_digit` retain their closed ASCII meanings and do not consult the
Unicode tables.

## UTF-8 Scalar Iteration

The compiler-checked [`str` contract](../str/index.nct) owns `Chars`, `chars`, and `char_count`.
`str.len()` remains a UTF-8 byte count; scalar traversal is explicit. `chars` visits Unicode scalars
in source order and performs no allocation. `Chars.next` returns `none` only after the complete
borrowed text has been consumed. Its internal byte offset always lands on a UTF-8 scalar boundary.
The iterator borrows the original `str`; it neither copies text nor extends its lifetime.

The standard package has one package-internal UTF-8 scalar decoder. Validation and iteration both
consume its step result. Neither `str`, `String`, formatting, JSON, nor another module may maintain
a second table of leading-byte ranges or continuation rules.

## Unicode Whitespace Views

Borrowed Unicode trimming uses the operations declared by the compiler-checked
[`str` contract](../str/index.nct) and returns views into the original text.

These methods remove the longest leading, trailing, or two-sided sequence of scalars for which
`char.is_whitespace()` is true. Returned byte boundaries are scalar boundaries. Existing
`trim_ascii_start`, `trim_ascii_end`, and `trim_ascii` remain available when callers require the
stable ASCII whitespace set independently of the pinned Unicode version.

## Default Case Conversion

Borrowed text supplies the locale-independent full default case conversions declared by the
compiler-checked [`str` contract](../str/index.nct).

Conversion follows Unicode 17.0.0 default casing, including unconditional multi-scalar mappings
and locale-independent contextual mappings. Entries conditional on a language or locale are not
selected. Every generated mapping is nonempty, so one input scalar produces one or multiple output
scalars; no `char -> char` case-conversion API is defined. Generation fails if a future pinned
Unicode corpus violates that representation contract.

The ordinary methods allocate in the current allocation context and abort only for allocation
failure. The `try_` methods use the supplied recoverable allocator. They either return one complete,
valid UTF-8 `String` or return an error without publishing partial result storage. Input text remains
unchanged and borrowed for the duration of the call only.

Case conversion is not normalization, caseless comparison, or case folding. Canonically equivalent
inputs may retain distinct normalized forms.

## Boundary-safe String Removal

The owning `String` instance adds the scalar-safe suffix mutations declared by its compiler-checked
[public contract](../string/index.nct).

`pop` removes and returns the final scalar, or returns `none` for an empty String. `truncate` does
nothing when `byte_len` is at least the current byte length, removes the complete suffix when the
index is a scalar boundary, and returns `std.string.not_char_boundary` when an in-range index lies
inside an encoding. Failure leaves the String unchanged. Neither operation changes capacity or can
produce invalid UTF-8.

## Owned String Integration

The compiler-checked [`String` contract](../string/index.nct) owns scalar append. `try_push` either
appends the complete scalar encoding or leaves the logical string unchanged when storage growth
fails. `push` converts only that allocation failure to the ordinary allocation abort. Neither
operation can introduce invalid UTF-8. Both operations use the same package-internal encoder as
scalar UTF-8 construction.

## Generated Unicode Data

The distributed standard library contains generated readonly tables, not a runtime dependency on
Unicode files or an operating-system locale database. Generation consumes only the pinned final
Unicode 17.0.0 data files, records their content digests, and produces deterministic Nocter source.
Normal compiler builds, package builds, tests, and release qualification perform no network access
for Unicode data.

Property and casing algorithms consume one package-internal lookup contract. Public `char`, `str`,
and `String` implementations cannot duplicate generated ranges, mapping records, or contextual
casing rules. Generated tables contain data only; they do not define public semantics.

## Tooling

Unicode methods are ordinary standard-source declarations. Editor features reach those declarations
through the existing semantic query products and do not embed property or casing tables.

## Non-goals

- Unicode normalization, grapheme or word segmentation, display width, collation, or locale-aware
  casing;
- case folding or caseless search;
- scalar-position indexing of `str`;
- adopting draft Unicode 18.0.0 data.
