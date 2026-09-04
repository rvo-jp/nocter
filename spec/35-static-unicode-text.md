# Static Data and Unicode Text

**v0.35.0 candidate language and standard-library contract.** Publication status belongs to the
release index. This chapter extends the scalar foundation without changing the meaning of `char`,
UTF-8 byte offsets, or existing ASCII-specific APIs.

## Unicode Version

Unicode-dependent behavior is fixed to the final
[Unicode Standard and Unicode Character Database version 17.0.0](https://www.unicode.org/versions/Unicode17.0.0/).
Draft or beta data, including Unicode 18.0.0 draft data, is not a valid input.
Changing the Unicode version changes observable program behavior and therefore requires a later
Nocter release.

The public contract uses Unicode properties and default case conversion only where this chapter
names them. It does not silently reinterpret existing byte-oriented search, slicing, length, or
ASCII operations.

## Immutable Static Data

`static` declares one immutable, addressable value whose initialized representation is embedded in
the executable before program execution:

```nct
static ASCII_LIMITS: [u32; 2] = [65, 90]

pub static PROTOCOL_MARKERS: [u8; 3]
```

A bodyless public declaration in a module root joins exactly one private definition with the same
declaration kind, name, and type, using the same contract/implementation rule as `const`. An
initialized public static may remain inline when its value is itself the intended public contract.

A static name uses `UPPER_SNAKE_CASE` and denotes a readonly place with `static` provenance. It can
be read when its type is copyable, indexed according to its type, or borrowed as `&STATIC_NAME`.
It cannot be assigned, moved, mutably borrowed, dropped, or used as an allocation context. No
runtime initializer or initialization-order relation exists.

The v0.35.0 static-initializer domain contains:

- boolean, integer, character, and non-interpolated string literals;
- references to `const` values;
- the pure unary, binary, and conversion constant expressions defined for `const`;
- fixed-array literals whose elements recursively belong to this domain.

The declared static type must recursively contain only `bool`, integer types, `char`, readonly
`&str`, and fixed arrays of those types. Owned values, nominal values, pointers, mutable borrows,
slices, optionals, fallible values, callables, generic-dependent values, and values with destruction
are rejected. Every readonly string reference in a static initializer refers to embedded static
text.

The compiler evaluates a static initializer once during semantic construction and publishes one
typed frozen value to executable lowering. The selected machine layout owns its size, alignment,
and byte encoding. The executable-format layer places the resulting bytes in readonly mapped data
and performs no source-level evaluation. The backend cannot inspect initializer syntax or
reconstruct aggregate values.

`const` remains a storage-independent value and does not become an alias for `static`. `static`
exists for immutable data whose address and indexed storage are part of execution.

## Unicode Character Properties

The standard `char` instance adds allocation-free property queries:

```nct
instance char {
    pub noalloc method self.is_whitespace(): bool
    pub noalloc method self.is_alphabetic(): bool
    pub noalloc method self.is_lowercase(): bool
    pub noalloc method self.is_uppercase(): bool
    pub noalloc method self.is_decimal_digit(): bool
}
```

`is_whitespace` uses the Unicode `White_Space` property. `is_alphabetic`, `is_lowercase`, and
`is_uppercase` use the corresponding derived core properties. `is_decimal_digit` is true exactly
for general category `Decimal_Number` (`Nd`); it is narrower than the Unicode numeric property and
does not return a numeric value.

Existing `is_ascii` and `is_ascii_digit` retain their closed ASCII meanings and do not consult the
Unicode tables.

## Unicode Whitespace Views

Borrowed Unicode trimming returns views into the original text:

```nct
instance str {
    pub noalloc method &self.trim_start(): &str from self
    pub noalloc method &self.trim_end(): &str from self
    pub noalloc method &self.trim(): &str from self
}
```

These methods remove the longest leading, trailing, or two-sided sequence of scalars for which
`char.is_whitespace()` is true. Returned byte boundaries are scalar boundaries. Existing
`trim_ascii_start`, `trim_ascii_end`, and `trim_ascii` remain available when callers require the
stable ASCII whitespace set independently of the pinned Unicode version.

## Default Case Conversion

Borrowed text supplies locale-independent full default case conversion:

```nct
instance str {
    pub method &self.to_lowercase(): String
    pub method &self.to_uppercase(): String

    pub method &self.try_to_lowercase(
        allocator: &+TryAllocator,
    ): String! from allocator

    pub method &self.try_to_uppercase(
        allocator: &+TryAllocator,
    ): String! from allocator
}
```

Conversion follows Unicode 17.0.0 default casing, including unconditional multi-scalar mappings
and locale-independent contextual mappings. Entries conditional on a language or locale are not
selected. One input scalar may therefore produce zero, one, or multiple output scalars; no
`char -> char` case-conversion API is defined.

The ordinary methods allocate in the current allocation context and abort only for allocation
failure. The `try_` methods use the supplied recoverable allocator. They either return one complete,
valid UTF-8 `String` or return an error without publishing partial result storage. Input text remains
unchanged and borrowed for the duration of the call only.

Case conversion is not normalization, caseless comparison, or case folding. Canonically equivalent
inputs may retain distinct normalized forms.

## Boundary-safe String Removal

The owning String instance adds scalar-safe suffix mutation:

```nct
instance String {
    pub noalloc method &+self.pop(): char?
    pub noalloc method &+self.truncate(byte_len: usize): void!
}
```

`pop` removes and returns the final scalar, or returns `none` for an empty String. `truncate` does
nothing when `byte_len` is at least the current byte length, removes the complete suffix when the
index is a scalar boundary, and returns `std.string.not_char_boundary` when an in-range index lies
inside an encoding. Failure leaves the String unchanged. Neither operation changes capacity or can
produce invalid UTF-8.

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

Static declarations participate in parsing, formatting, tokens and AST output, diagnostics,
navigation, references, rename, completion, hover, and semantic highlighting through one static
identity. Hover renders the canonical declaration kind, name, and type, distinguishes `static`
storage from `const` values, and never prints an initializer's potentially large contents.

Unicode methods are ordinary standard-source declarations. Editor features reach those declarations
through the existing semantic query products and do not embed property or casing tables.

## Non-goals for v0.35.0

- Unicode normalization, grapheme or word segmentation, display width, collation, or locale-aware
  casing;
- case folding or caseless search;
- scalar-position indexing of `str`;
- mutable static storage, thread-local storage, runtime global initialization, or initialization
  order;
- nominal, owned, generic, or destructible static values;
- adopting draft Unicode 18.0.0 data.
