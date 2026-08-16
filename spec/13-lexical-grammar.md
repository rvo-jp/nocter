# Lexical Grammar

This file is part of the Nocter language specification.
The specification entry point is [README.md](README.md).

## Source Text

Nocter source files are UTF-8 text files with explicit, simple lexical rules.

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
- Block comments do not nest.
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

Identifiers are ASCII-only.

Identifier grammar:

```text
identifier = [A-Za-z_][A-Za-z0-9_]*
```

Rules:

- Unicode letters are not accepted in identifiers.
- Reserved keywords are not identifiers.
- `nocter` is an ordinary identifier and has no visibility meaning.
- `copy` is not a reserved keyword. It is emitted as an identifier token; the parser recognizes it
  contextually in `copy struct` and `where copy T` source forms.
- `where` is not a reserved keyword. It is recognized contextually in declaration constraint
  clauses.
- `some` is not a reserved keyword. It is emitted as an identifier token and recognized
  contextually at the start of an opaque type atom in a callable result.
- `destruct` is a reserved keyword for top-level destruction declarations.
- `drop` is not a reserved keyword. It is emitted as an identifier token; the parser recognizes
  an explicit drop statement only by its statement-position source form.
- `interface` is a reserved keyword.
- `from` and `import` are not reserved keywords. They are emitted as
  identifier tokens; top-level legacy import syntax is diagnosed as removed
  syntax by the parser.
- The parser recognizes `from` contextually after a callable return type. This does not reserve
  `from` as a general identifier.
- `alloc` is not a reserved keyword. It is an ordinary identifier, including the standard
  `Allocator.alloc` method. Obsolete result-modifier forms such as `alloc func` receive a focused
  parser diagnostic and do not produce a compatibility AST.
- `trait` is not a reserved keyword. It is emitted as an identifier token;
  top-level trait syntax is diagnosed as removed syntax by the parser.
- `Self` has identifier spelling but is reserved as contextual type syntax in
  inherent member type positions. It is not a valid binding, declaration, field,
  variant, module, type parameter, or import alias name.
- `error` is not a reserved keyword. In type positions, the exact spelling `error` is compiler built-in type syntax. In value positions, it is an ordinary identifier, so `catch error { ... }` binds a local value named `error`.
- A single `_` is the one-slot wildcard or discard spelling in enum payload pattern positions and
  in the local discard initializer `let _ = expression`. It never abbreviates multiple enum
  payload positions. A discard initializer creates no binding. `_` is not a valid binding,
  declaration, field, variant, type parameter, or import alias name.
- Identifiers beginning with `_` are otherwise valid.

Reserved keyword tokens:

```text
as
break
catch
continue
construct
conform
else
enum
false
for
func
if
in
instance
interface
is
let
literal
loop
match
method
move
never
none
otherwise
operator
primitive
pub
region
return
struct
test
true
type
use
using
var
void
while
```

Built-in type spellings such as `bool`, integer types, `usize`, `isize`, `str`,
`void`, and `never` are reserved type syntax, not importable user names. The
special `error` type spelling is contextual as described above.

Package metadata uses the same `test` token in `#test`; package parsing treats it as that
directive's exact name rather than as a general identifier.

Module-directory segments and imported source-file stems use snake_case identifiers:

```text
file_name
std/io
std/process
```

Rules:

- Module path segments must use lowercase ASCII letters, digits, and underscores.
- A module path segment must not start with a digit.
- A module path segment must not be a reserved keyword.
- A module path segment must not be `_`.

## Statement Separation

Nocter does not use semicolons as statement terminators. The `;` token is reserved for grammar
positions that explicitly require it: fixed-size array types `[T; N]` and the separator between
explicit closure captures and closure parameters.

Rules:

- At statement-capable nesting depth, a newline separates statements when the tokens before it can
  end a statement.
- One such newline is instead a continuation newline when the first token on the next physical line
  is a continuation leader. A continuation leader is a token that cannot begin an expression or
  statement in that position and can extend the expression immediately before the newline.
- The continuation leaders are `.`, `=`, `+=`, `-=`, `*=`, `/=`, `%=`, `+`, `*`, `/`, `%`,
  `<<`, `>>`, `<`, `<=`, `>`, `>=`, `==`, `!=`, `&&`, `||`, `as`, `catch`, and `otherwise`. The
  context-specific `is` and `..<` tokens are also continuation leaders in `if` pattern conditions
  and range `for` headers respectively.
- `-` is not a continuation leader because it can begin a unary expression. Put binary `-` at the
  end of the previous line when its right operand continues on the next line.
- `!`, `&`, `&+`, and `move` are not continuation leaders because they can begin expressions.
  Postfix `?` and postfix `!` remain attached to the expression they modify and cannot begin a
  continuation line.
- `move` consumes a syntactic move place before immediately following outcome suffixes are attached.
  Consequently `move value?` tokenizes and parses as `(move value)?`; whitespace does not turn it
  into `move (value?)`.
- At most one `?` or `!` outcome suffix may attach at one expression layer. Adjacent `??`, `!!`,
  `?!`, and `!?` are syntax errors regardless of whitespace. Parentheses create another expression
  layer, so `(value?)?` is distinct and valid when both type layers support propagation.
- `(` and `[` are not continuation leaders. A call or index opener must remain on the same line as
  its callee or indexed expression.
- Two or more consecutive newline tokens at statement-capable nesting depth are never collapsed
  into a continuation newline. A blank or comment-only intervening line therefore ends any
  possible leading-token continuation.
- A closing brace `}` ends the current block or arm.
- A semicolon does not terminate a statement. Outside `[T; N]` or a closure capture separator, it
  is a syntax error.
- A newline also continues an expression when the tokens before it cannot end the expression, such
  as after a binary operator, or when the enclosing call, literal, index, or parenthesized-expression
  grammar is still consuming the expression.
- Whitespace other than newline is only a token separator.

Examples:

```nct
let total = left
    + right
    * scale

let difference = left -
    right

let result = values
    .map(transform)
    .filter(predicate)

count
    += 1

let rendered = render(
    input,
)
```

The following does not continue the first line because unary `-` can begin an expression:

```nct
let difference = left
-right
```

The following is not a call because `(` is not a continuation leader:

```nct
let rendered = render
(input)
```

## Comma-Delimited Lists

Every syntax form declared as a comma-delimited list uses one layout-independent separator rule:

```text
DelimitedList(Item) = Item ("," Item)* [","]
```

The enclosing grammar separately decides whether the list may be empty.

Rules:

- A comma is required between adjacent items. A newline never replaces it.
- One trailing comma is accepted before the closing delimiter on either a single line or multiple
  lines.
- The same rule applies when an explicit grammar token such as the closure-capture `;` ends one
  comma-delimited segment inside a larger delimiter pair.
- Two adjacent commas and more than one trailing comma are invalid.
- A comma is invalid outside a grammar position that declares a comma-delimited list or separator.
- Non-delimited sequences such as a `where` predicate clause, struct declaration fields, enum
  variants, match arms, and body statements retain their own separator rules and do not gain a
  trailing comma.
- Enum declaration payloads, variant constructor arguments, and enum pattern payload slots are
  comma-delimited lists. Pattern payload slots additionally require exact declaration arity.
- Source formatting, not parsing, determines whether the accepted trailing comma appears in
  canonical output.

For example, all of these are syntactically valid before formatting:

```nct
call(first, second)
call(first, second,)
let values = [1, 2, 3,]
let user = User { id: 1, name: text, }
```

## Tokenization

The lexer uses longest-match tokenization for multi-character tokens.

Lexer boundary:

- The lexer receives a `SourceId` and normalized UTF-8 source text from `SourceMap`.
- The lexer returns a token stream and diagnostics.
- The token stream includes keyword tokens, newline tokens, and one EOF token.
- Comments are not emitted as tokens.
- Literal tokens keep their source text; final literal value interpretation belongs to later compiler stages except for lexical validity checks.
- Invalid lexical constructs produce diagnostics. The lexer may stop after the first unrecoverable lexical error.
- `nocter tokens app.nct --format json` emits a JSON envelope even when lexer diagnostics are present.

Token categories:

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
- `nocter` is emitted as an identifier token.
- `destruct` is emitted as a keyword token.
- `drop` is emitted as an identifier token; the parser treats `drop name` in statement position as
  a contextual source form.
- `Self` may be emitted as an identifier-shaped token by the lexer, but the parser treats that exact spelling contextually as type syntax only where [Values and Types](02-values-types.md#self-type-syntax) allows it.
- `error` is emitted as an identifier token; the parser treats it contextually as built-in type syntax only in type positions.
- `ok`, `some`, `unsafe`, and `trusted` are not reserved and are emitted as identifier tokens.
  The parser recognizes `some` contextually only in the opaque result type form defined by
  [Generics, Interfaces, and Methods](08-generics-interfaces-embedding-methods.md#static-opaque-results).
- `default` is not reserved. The parser recognizes it contextually inside `construct` blocks.
- `alloc` is emitted as an identifier token and has no contextual keyword classification.

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
..<
...
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
#
```

Rules:

- `&+` is one token. It is used for readwrite borrow syntax.
- `..<` is one token. It is used only in `for name in start..<end` range syntax.
- `#` is punctuation. It begins a declarative directive. Directive names remain identifiers rather
  than reserved keywords.
- `@` is reserved for possible future attribute-like syntax and is invalid outside string literals, byte literals, and comments.
- Unary `+expr` is not part of the language even though `+` is a valid additive operator token.

`#target: "target-name"` is tokenized as ordinary punctuation, identifier, punctuation, and string
tokens:

```text
# target : "target-name"
```

At the start of package-root `nocter.nct`, package directives accept declarative strings, integers,
booleans, lists, and records. These values are data: they do not perform lookup, calls,
interpolation, allocation, or target execution. `#target` remains a declaration directive and is
recognized only before an eligible top-level declaration. A `#` token in any other source position
is a syntax error.

## Integer Literals

Integer literals support decimal, hexadecimal, binary, and `_` digit separators.

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
- Integer literals have no type suffix.
- Negative numbers are parsed as unary `-` applied to an integer literal, not as a negative literal token.
- Float literals are not supported. Syntax such as `1.0`, `.5`, and `1e3` is invalid.

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
- Plain single-quoted literals such as `'a'` are invalid.
- Raw newlines are invalid inside single-line string literals and byte literals.
- Raw newlines are valid inside multi-line string literals.
- Raw string literals and Unicode escape syntax are not supported.
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

Escapes:

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
- Braces inside nested expressions, such as struct literals, blocks, `if` expressions, and `match` expressions, participate in normal brace matching.
- Newline handling inside an interpolation expression follows ordinary expression grammar, not string-literal text rules.
- Escapes in literal text segments are interpreted before the final text is constructed.
- An interpolated string source form is an expression-level construct, not a plain string literal token. Its type, allocation behavior, evaluation order, and formatting rules are specified in [Strings, Arrays, Views, and Pointers](07-strings-arrays-views-pointers.md#string-interpolation).
- To include the literal characters `${` in string text, write `\${`.

The type and storage rules for string and byte literals are specified in [Strings, Arrays, Views, and Pointers](07-strings-arrays-views-pointers.md#string-and-byte-literals).

## Unsupported Tokens and Forms

The following lexical features are intentionally unsupported:

- Unicode identifiers
- nested block comments
- semicolon statement terminators
- float literals
- integer type suffixes
- plain character literals
- raw string literals
- Unicode escape syntax
- attribute syntax
