# Unicode Scalar Values

Nocter's `char` type represents one Unicode scalar value. A scalar is an integer in
`U+0000..=U+10FFFF` excluding the surrogate range `U+D800..=U+DFFF`. It is not a UTF-8 byte, an
extended grapheme cluster, a displayed glyph, or a locale-sensitive character.

## Type and Representation

`char` is a compiler built-in copy type whose exact source declaration belongs to the
compiler-checked [`std/char` contract](../../development/std/char/index.nct).

Every runtime `char` value is a valid Unicode scalar. Its stored representation is the scalar's
unsigned value with size 4 and alignment 4. It remains a distinct type from `u32`; arithmetic and
integer casts do not implicitly create `char` values. The target layout determines the physical
representation without giving `char` the arithmetic or conversion semantics of `u32`.

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

The standard package owns `char` construction, observation, comparison, hashing, formatting, and
Unicode property operations. Their exact declarations and observable behavior are defined by
[Unicode Text and Scalars](../../development/std/char/README.md), not by the language grammar.

## UTF-8 Scalar Iteration

`str.len()` remains a UTF-8 byte count. The standard library provides explicit scalar traversal;
the language does not reinterpret text indices as scalar positions. See
[Unicode Text and Scalars](../../development/std/char/README.md#utf-8-scalar-iteration).

## Owned String Integration

Owned-string scalar operations belong to the standard-library
[`String` contract](../../development/std/string/index.nct) and its
[Unicode behavior](../../development/std/char/README.md#owned-string-integration).

## Tooling

Tokens and AST output identify character literals separately from byte and string literals. The
formatter preserves a valid authored character literal whose spelling is already canonical and
uses uppercase hexadecimal digits for generated `\u{...}` spellings. Semantic highlighting treats
the complete literal as a scalar literal. Hover displays `char`; completion and navigation consume
the ordinary built-in declaration and instance indexes.

Malformed character literals produce the same source-backed lexical diagnostics in CLI and LSP
operation. Exact diagnostic codes and messages belong to the
[diagnostic catalog](../tooling/diagnostics.md#format).

## Non-goals

- user-perceived character indexing;
- grapheme-cluster or word-boundary segmentation;
- display width, fonts, collation, or normalization;
- locale-sensitive case conversion or other locale-dependent text behavior;
- indexing `str` by scalar position;
- implicit conversion among `char`, integers, bytes, strings, or one-element collections;
- changing existing byte offsets returned by search and range APIs;
- a second UTF-8 validator or encoder.
