# Unicode Scalar Values

**v0.34.0 language and standard-library contract.** Publication status belongs to the
release index; this chapter defines the published behavior.

Nocter's `char` type represents one Unicode scalar value. A scalar is an integer in
`U+0000..=U+10FFFF` excluding the surrogate range `U+D800..=U+DFFF`. It is not a UTF-8 byte, an
extended grapheme cluster, a displayed glyph, or a locale-sensitive character.

## Type and Representation

`char` is a compiler built-in copy type declared by the standard package:

```nct
pub primitive type char
```

Every runtime `char` value is a valid Unicode scalar. Its stored representation is the scalar's
unsigned value with size 4 and alignment 4. It remains a distinct type from `u32`; arithmetic and
integer casts do not implicitly create `char` values. Target layout owns the physical representation
once, and later machine lowering consumes that layout without treating `char` as `u32` semantics.

## Character Literals

A character literal uses one pair of single quotes and has type `char`:

```nct
let latin = 'A'
let lambda = 'λ'
let newline = '\n'
let face = '\u{1F600}'
```

The content must decode to exactly one Unicode scalar. Supported escapes are `\n`, `\r`, `\t`,
`\0`, `\\`, `\"`, `\'`, and `\u{H...}` with one through six ASCII hexadecimal digits. Hexadecimal
scalar escapes are case-insensitive and do not permit separators. A surrogate, an out-of-range
value, empty content, multiple scalars, a newline, an invalid escape, or a missing closing quote is
a lexical diagnostic.

Byte literals remain distinct:

```nct
let byte: u8 = b'A'
let raw: u8 = b'\xFF'
```

`b'…'` decodes exactly one byte and never produces `char`. Character and byte decoding are owned by
the syntax layer. Checking receives a decoded scalar or byte and does not parse source text again.

Character literals are valid constant expressions. Their canonical value is the Unicode scalar,
not the authored escape spelling.

## Core `char` Surface

The standard package provides:

```nct
construct char {
    pub noalloc func from_u32(value: u32): Self?
}

instance char {
    pub noalloc method self.code_point(): u32
    pub noalloc method self.utf8_len(): usize
    pub noalloc method self.is_ascii(): bool
    pub noalloc method self.is_ascii_digit(): bool

    pub noalloc operator (&self == other: &Self): bool
    pub noalloc operator (&self < other: &Self): bool
}
```

`from_u32` returns `none` for a surrogate or value above `U+10FFFF`. `code_point` exposes the exact
scalar value. Equality and ordering compare scalar values. `utf8_len` returns 1, 2, 3, or 4.
ASCII classification recognizes only the corresponding ASCII range and does not claim Unicode
property or locale behavior.

`char` implements `Hash` using its scalar value and `Format` by appending its exact UTF-8 encoding.
Equal values therefore hash equally regardless of their authored literal spelling.

## UTF-8 Scalar Iteration

`str.len()` remains a UTF-8 byte count. Scalar traversal is explicit:

```nct
pub struct Chars

instance Chars {
    impl Iterator { .Item = char }
}

instance str {
    pub noalloc method &self.chars(): Chars from self
    pub noalloc method &self.char_count(): usize
}
```

`chars` visits Unicode scalars in source order and performs no allocation. `Chars.next` returns
`none` only after the complete borrowed text has been consumed. Its internal byte offset always
lands on a UTF-8 scalar boundary. The iterator borrows the original `str`; it neither copies text nor
extends its lifetime.

The standard package has one package-internal UTF-8 scalar decoder. Validation and iteration both
consume its step result. Neither `str`, `String`, formatting, JSON, nor another module may maintain
a second table of leading-byte ranges or continuation rules.

## Owned String Integration

`String` appends one scalar through the same package-internal encoder already used for scalar UTF-8
construction:

```nct
instance String {
    pub method &+self.push(value: char): void
    pub method &+self.try_push(value: char): void!
}
```

`try_push` either appends the complete scalar encoding or leaves the logical string unchanged when
storage growth fails. `push` converts only that allocation failure to the ordinary allocation abort.
Neither operation can introduce invalid UTF-8.

## Tooling

Tokens and AST output identify character literals separately from byte and string literals. The
formatter preserves a valid authored character literal whose spelling is already canonical and
uses uppercase hexadecimal digits for generated `\u{...}` spellings. Semantic highlighting treats
the complete literal as a scalar literal. Hover displays `char`; completion and navigation consume
the ordinary built-in declaration and instance indexes.

Malformed character literals retain lexical diagnostics in CLI and LSP operation. They cannot
surface as an internal checker, MIR, machine, or editor error.

## Candidate Diagnostics

- `E0115`: a character literal is not terminated;
- `E0116`: a character literal contains a newline;
- `E0117`: a character literal does not decode to exactly one valid Unicode scalar.

Invalid escape spelling continues to use `E0107`. `E0113`, which rejected every plain
single-quoted literal before this contract, remains retired rather than being reassigned.

## Non-goals for v0.34.0

- grapheme-cluster segmentation or user-perceived character indexing;
- display width, fonts, collation, normalization, or locale behavior;
- Unicode general-category, word-boundary, or case-mapping tables;
- indexing `str` by scalar position;
- implicit conversion among `char`, integers, bytes, strings, or one-element collections;
- changing existing byte offsets returned by search and range APIs;
- a second UTF-8 validator or encoder.
