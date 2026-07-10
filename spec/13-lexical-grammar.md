# Lexical Grammar

This file is part of the Nocter language specification.
The specification entry point is [README.md](README.md).

## Source Text

Adopted: Nocter v0 source files are UTF-8 text files with explicit, simple lexical rules.

Rules:

- `.nct` files are decoded as UTF-8 before lexing.
- Invalid UTF-8 is a source diagnostic.
- LF and CRLF line endings are accepted.
- CRLF is normalized to LF before lexing.
- A raw carriage return byte that is not part of CRLF is invalid; use the `\r` escape in literals when a carriage-return byte is intended.
- Source locations in diagnostics are reported after line-ending normalization.

## Whitespace and Comments

Whitespace separates tokens. It has no meaning beyond token separation and statement separation.

Rules:

- Space, horizontal tab, and newline are whitespace.
- Indentation has no syntactic meaning.
- `//` starts a line comment and runs until the next line ending or end of file.
- `/*` starts a block comment and runs until the next `*/`.
- `///` starts an item doc line comment for the next documentable construct.
- `/**` starts an item doc block comment for the next documentable construct, except for `/**/` and comments beginning with `/***`.
- `//!` starts a file doc line comment.
- `/*!` starts a file doc block comment.
- Block comments do not nest in v0.
- Unterminated block comments are lexical errors.
- Comments are not recognized inside string literals or byte literals.
- Newlines inside block comments still count as line breaks for diagnostics and statement separation.
- `////`, `/**/`, and block comments beginning with `/***` are normal comments, not doc comments.

Examples:

```nct
//! File-level documentation.

/// Item documentation.
func answer(): i32 {
    return 42
}

let a = 1 // line comment

/*
    block comment
*/
let b = 2
```

## Identifiers

Adopted: identifiers are ASCII-only in v0.

Identifier grammar:

```text
identifier = [A-Za-z_][A-Za-z0-9_]*
```

Rules:

- Unicode letters are not accepted in identifiers in v0.
- Reserved keywords are not identifiers.
- `nocter` is not a reserved keyword except as the contextual visibility scope in `pub(nocter)`.
- `Self` has identifier spelling but is reserved as contextual type syntax in `impl` and `trait` type positions. It is not a valid binding, declaration, field, variant, module, type parameter, or import alias name.
- `error` is not a reserved keyword. In type positions, the exact spelling `error` is compiler built-in type syntax. In value positions, it is an ordinary identifier, so `catch error { ... }` binds a local value named `error`.
- A single `_` is reserved for a future discard or wildcard design and is not a valid binding, declaration, field, variant, type parameter, or import alias name in v0.
- Identifiers beginning with `_` are otherwise valid.

Module file and directory names use snake_case identifiers:

```text
file_name
std/io
os/macos
```

Rules:

- Module path segments must use lowercase ASCII letters, digits, and underscores.
- A module path segment must not start with a digit.
- A module path segment must not be a reserved keyword.
- A module path segment must not be `_`.

## Statement Separation

Nocter does not use semicolons as statement terminators in v0. The `;` token is reserved for grammar positions that explicitly require it, such as fixed-size array types `[T; N]`.

Rules:

- A newline separates statements where the grammar can end a statement.
- A closing brace `}` ends the current block or arm.
- A semicolon does not terminate a statement. Outside an explicit grammar position such as `[T; N]`, it is a syntax error.
- Multi-line expressions are valid only when the expression syntax clearly continues, such as inside calls, literals, indexes, parenthesized expressions, or before an operator that requires a right operand.
- Whitespace other than newline is only a token separator.

## Tokenization

The lexer uses longest-match tokenization for multi-character tokens.

Adopted v0 lexer boundary:

- The lexer receives a `SourceId` and normalized UTF-8 source text from `SourceMap`.
- The lexer returns a token stream and diagnostics.
- The token stream includes keyword tokens, newline tokens, and one EOF token.
- Comments are not emitted as tokens.
- Literal tokens keep their source text; final literal value interpretation belongs to later compiler stages except for lexical validity checks.
- Invalid lexical constructs produce diagnostics. The v0 lexer may stop after the first unrecoverable lexical error.
- `nocter tokens app.nct --format json` emits a JSON envelope even when lexer diagnostics are present.

Initial token categories:

```text
identifier
keyword
integer_literal
string_literal
byte_literal
newline
punctuation
eof
```

Keyword rules:

- Reserved keywords are emitted as keyword tokens.
- `nocter` is emitted as an identifier token; parser and resolver treat it contextually in `pub(nocter)`.
- `Self` may be emitted as an identifier-shaped token by the lexer, but the parser treats that exact spelling contextually as type syntax only where [Values and Types](02-values-types.md#self-type-syntax) allows it.
- `error` is emitted as an identifier token; the parser treats it contextually as built-in type syntax only in type positions.
- `ok`, `some`, `unsafe`, and `trusted` are not reserved in v0 and are emitted as identifier tokens.

Newline rules:

- A normalized LF line ending is emitted as a `newline` token.
- Parser rules decide whether a newline can separate statements.
- The terminating newline after a line comment is emitted as a `newline` token.
- LF bytes inside a block comment are emitted as `newline` tokens so block comments can preserve statement separation.
- Comment text itself is not emitted as tokens.
- Doc comment text is not emitted as ordinary tokens; compiler tooling scans source text to attach doc comments to symbols for future docs and LSP features.
- Newlines inside single-line string literals and byte literals are lexical errors.
- Newlines inside multi-line string literals are literal content and are not emitted as statement-separating `newline` tokens.

EOF rules:

- The lexer emits exactly one EOF token.
- EOF span is empty at the end of the normalized source text.

Examples of single lexical tokens:

```text
&+
??
..<
==
!=
<=
>=
&&
||
<<
>>
+=
-=
*=
/=
%=
```

Rules:

- `&+` is one token. It is used for readwrite borrow syntax.
- `??` is one token. It is used for optional default expressions.
- `?{` is not one token. Pattern conditional expressions are parsed from `?` followed by `{`.
- `..<` is one token. It is used only in the initial `for name in start..<end` range syntax.
- `@` is reserved for possible future attribute-like syntax and is invalid in v0 outside string literals, byte literals, and comments.
- Unary `+expr` is not part of the language even though `+` is a valid additive operator token.

## Integer Literals

Adopted: integer literals support decimal, hexadecimal, binary, and `_` digit separators.

Forms:

```text
10
1_000
0xFF
0xFF_FF
0b1010
0b1010_0101
```

Rules:

- Decimal integer literals use digits `0` through `9`.
- Hexadecimal integer literals use the lowercase prefix `0x`.
- Hexadecimal digits may be `0` through `9`, `a` through `f`, or `A` through `F`.
- Binary integer literals use the lowercase prefix `0b`.
- Binary digits may be `0` or `1`.
- `_` may appear only between two valid digits of the literal's base.
- `_` must not appear at the start or end of a literal.
- `_` must not appear immediately after `0x` or `0b`.
- Adjacent `_` separators are invalid.
- Integer literals have no type suffix in v0.
- Negative numbers are parsed as unary `-` applied to an integer literal, not as a negative literal token.
- Float literals are not part of v0. Syntax such as `1.0`, `.5`, and `1e3` is invalid.

The type rules for integer literals are specified in [Values and Types](02-values-types.md#integer-literals).

## String and Byte Literals

Single-line string literals use double quotes:

```nct
let name = "Nocter"
```

Multi-line string literals use triple double quotes:

```nct
let message = """
    first line
    second line
    """
```

Byte literals use `b'...'`:

```nct
let newline: u8 = b'\n'
let marker: u8 = b'\xFF'
```

Rules:

- A single-line string literal starts with `"` and ends at the next unescaped `"`.
- A multi-line string literal starts with `"""` and ends at a closing `"""` delimiter.
- A byte literal starts with `b'` and ends at the next unescaped `'`.
- No whitespace is allowed between `b` and `'` in a byte literal.
- Plain single-quoted literals such as `'a'` are invalid in v0.
- Raw newlines are invalid inside single-line string literals and byte literals.
- Raw newlines are valid inside multi-line string literals.
- Raw string literals are not part of v0.
- Unicode escape syntax is not part of v0.
- Escapes are interpreted by the compiler before literal bytes are placed into the output executable.
- A string literal must decode to valid UTF-8 after escapes are processed.
- A byte literal must decode to exactly one byte.
- Comments are not recognized inside single-line string literals, multi-line string literals, byte literals, or interpolation text segments.

Multi-line string literal rules:

- The opening `"""` delimiter must be followed immediately by a normalized LF.
- The opening delimiter's LF is not part of the literal value.
- The closing `"""` delimiter must appear after optional spaces or horizontal tabs at the start of a source line.
- The closing delimiter's indentation is the exact byte prefix before the closing `"""`.
- That exact indentation prefix is removed from each non-empty content line.
- A non-empty content line that does not start with the closing delimiter's indentation prefix is invalid.
- Empty content lines remain empty and do not need to contain the indentation prefix.
- Spaces and tabs are compared byte-for-byte. Tabs are not expanded to columns.
- The LF immediately before the closing delimiter is not part of the literal value.
- The closing delimiter ends the multi-line string literal. Following source text is tokenized normally.
- A `"""` sequence that is not in closing-delimiter position is ordinary literal content.

Initial escapes:

```text
\n      newline, byte 0x0A
\r      carriage return, byte 0x0D
\t      horizontal tab, byte 0x09
\0      NUL, byte 0x00
\\      backslash
\"      double quote
\'      single quote
\$      dollar sign
\xNN    byte with two hexadecimal digits
```

In a byte literal, `\xNN` may produce any byte from `0x00` through `0xFF`.

In a string literal, `\xNN` inserts that byte into the literal byte sequence. The final string literal must still be valid UTF-8.

## String Interpolation

String interpolation inserts expressions into string source forms with `${expr}`.

Examples:

```nct
let path_text = "path: ${path}"
let report = """
    name: ${name}
    count: ${count}
    """
```

Rules:

- Interpolation is recognized in single-line string forms and multi-line string forms.
- Interpolation is not recognized in byte literals.
- `${` begins an interpolation expression unless the `$` is escaped as `\$`.
- The interpolation expression is parsed as a normal Nocter expression.
- The expression ends at the matching `}` for the `${`.
- Braces inside nested expressions, such as struct literals, blocks, and pattern conditionals, participate in normal brace matching.
- Newline handling inside an interpolation expression follows ordinary expression grammar, not string-literal text rules.
- Escapes in literal text segments are interpreted before the final text is constructed.
- An interpolated string source form is an expression-level construct, not a plain string literal token. Its type, allocation behavior, evaluation order, and formatting rules are specified in [Strings, Arrays, Views, and Pointers](07-strings-arrays-views-pointers.md#string-interpolation).
- To include the literal characters `${` in string text, write `\${`.

The type and storage rules for string and byte literals are specified in [Strings, Arrays, Views, and Pointers](07-strings-arrays-views-pointers.md#string-and-byte-literals).

## Not Adopted in v0

The following lexical features are intentionally not part of v0:

- Unicode identifiers
- nested block comments
- semicolon statement terminators
- float literals
- integer type suffixes
- plain character literals
- raw string literals
- Unicode escape syntax
- attribute syntax
