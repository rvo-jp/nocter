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
- `coerce` is not a reserved keyword. It is emitted as an identifier token and recognized
  contextually as an `instance` member declaration.
- `default` is not a reserved keyword. It is emitted as an identifier token and recognized
  contextually between a construction member's visibility and its `func` or `literal` keyword.
- `drop` is not a reserved keyword. It is emitted as an identifier token; the parser recognizes
  it contextually in top-level `drop Type(&+self) { ... }` declarations and statement-position
  `drop name` forms.
- `self` is not a reserved keyword. It is emitted as an identifier token and recognized
  contextually as the fixed receiver in method, operator, coercion, and drop declaration forms.
  Outside a receiver position it is an ordinary identifier spelling unless a semantic namespace
  rule rejects that use.
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
- `error` is not a reserved keyword. In type positions, the exact spelling `error` resolves through
  the compiler-selected primitive-type declaration. In value positions, it is an ordinary
  identifier, so `catch error { ... }` binds a local value named `error`.
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
const
continue
construct
else
enum
false
for
func
if
see
in
instance
interface
impl
is
let
literal
loop
match
method
move
never
noalloc
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

Named built-in type spellings such as `bool`, integer types, `usize`, `isize`, `str`, `error`,
`void`, and `never` are reserved declaration and type-binder names. Their exact
compiler-selected `primitive type` declarations supply the semantic identity and source target;
they are not imported names. `void` and `never` remain keywords, while the other spellings are
contextual identifiers.

Package metadata uses the same `test` token in `#test`; package parsing treats it as that
directive's exact name rather than as a general identifier.

Module-directory segments and `see` source-path segments use snake_case identifiers:

```text
file_name
./search.nct
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

Every comma-delimited syntax form uses the shared empty or non-empty list production from
[Syntactic Grammar](25-syntactic-grammar.md#notation). The enclosing production chooses whether the
list may be empty.

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
- Integer, byte, and string-component tokens keep their source text; final literal value
  interpretation belongs to later compiler stages except for lexical validity checks.
- Every non-EOF token records whether its source span is byte-adjacent to the next emitted token.
  Spaces, horizontal tabs, or a removed comment make the tokens non-joint. A normalized LF remains
  an emitted `newline` token rather than spacing metadata. The parser uses this lexical fact for
  the closed syntax positions that distinguish adjacency, such as indexing from a typed sequence;
  it never re-reads source bytes to reconstruct spacing.
- Invalid lexical constructs produce diagnostics. The lexer may stop after the first unrecoverable lexical error.
- `nocter tokens app.nct --format json` emits a JSON envelope even when lexer diagnostics are present.

Token categories:

```text
identifier
keyword
integer_literal
byte_literal
string_start
string_text
interpolation_start
interpolation_end
string_end
newline
punctuation
eof
```

Keyword rules:

- Reserved keywords are emitted as keyword tokens.
- `nocter` is emitted as an identifier token.
- `drop` is emitted as an identifier token; the parser treats `drop Type(&+self) { ... }` at top
  level and `drop name` in statement position as contextual source forms.
- `Self` may be emitted as an identifier-shaped token by the lexer, but the parser treats that exact spelling contextually as type syntax only where [Values and Types](02-values-types.md#self-type-syntax) allows it.
- `error` is emitted as an identifier token; semantic resolution selects its compiler-managed
  primitive-type declaration in type and construction-owner positions.
- `ok`, `some`, `unsafe`, and `trusted` are not reserved and are emitted as identifier tokens.
  The parser recognizes `some` contextually only in the opaque result type form defined by
  [Generics, Interfaces, and Methods](08-generics-interfaces-embedding-methods.md#static-opaque-results).
- `default` is emitted as an identifier token and recognized only before an interface default
  `method`.
- `alloc` is emitted as an identifier token and has no contextual keyword classification.

Newline rules:

- A normalized LF line ending is emitted as a `newline` token.
- Parser rules decide whether a newline can separate statements.
- The terminating newline after a line comment is emitted as a `newline` token.
- LF bytes inside a block comment are emitted as `newline` tokens so block comments can preserve statement separation.
- Comment text itself is not emitted as tokens.
- Doc comment text is not emitted as ordinary tokens; compiler tooling scans source text to attach doc comments to symbols for future docs and LSP features.
- After removing `///` or `//!`, documentation extraction removes at most one following ASCII
  space or tab. Adjacent line-doc comments contribute one Markdown line each.
- After removing `/**` or `/*!` and the closing `*/`, documentation extraction removes an empty
  outer first or last line, removes the common space/tab indentation of nonempty lines, and then
  removes a decorative leading `*` plus at most one following space or tab when every nonempty line
  has that decoration. A single-line block doc removes at most one boundary space or tab on each
  side. Extraction does not reflow or otherwise rewrite Markdown text.
- Adjacent documentation comments are joined with one newline. An empty line or an intervening
  ordinary comment breaks an item-documentation attachment.
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
- `>>` is one token. In expression grammar it is right shift. The syntactic grammar may consume it
  as two adjacent generic-list closers when two open type-argument lists require them; this is a
  token subdivision determined solely by the type grammar, not by name resolution.
- `&&` is one token. In an infix expression position it is logical conjunction. Where type or unary
  grammar is already expecting a prefix operator, the syntactic grammar may consume it as two
  adjacent readonly `&` prefixes. The position alone selects subdivision; a parser never consults
  operand types.
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

At the start of a package-root `index.nct`, package directives accept the declarative strings,
integers, and records admitted by the package grammar. These values are data: they do not perform
lookup, calls, interpolation, allocation, or target execution. `#target` remains a declaration
directive and is recognized only before an eligible top-level declaration. A `#` token in any
other source position is a syntax error.

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
- Negative numbers are parsed as unary `-` applied to an integer literal, not as a negative literal
  token. The type checker recognizes a directly grouped literal operand when validating the signed
  minimum value; the lexer does not fuse those tokens.
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

String tokenization is uniform for plain and interpolated strings:

- `string_start` covers the opening `"` or `"""` delimiter and records which delimiter form was
  used.
- `string_text` covers each maximal non-empty source-text segment between the opening delimiter,
  an interpolation, and the closing delimiter. It retains escaped source spelling; decoding and
  multi-line indentation removal happen after parsing.
- `interpolation_start` covers an unescaped `${`.
- Tokens inside interpolation use the ordinary lexer rules.
- `interpolation_end` covers the `}` that matches the current `${`. Nested expression braces are
  ordinary punctuation and do not end interpolation.
- `string_end` covers the closing delimiter.
- Empty text segments are omitted. The empty single-line string `""` therefore emits one
  `string_start` immediately followed by one joint `string_end`.

The lexer maintains a stack for nested string and interpolation states. This is lexical delimiter
matching, not expression parsing: the parser still decides whether the ordinary tokens between
`interpolation_start` and `interpolation_end` form one valid expression. Component spans cover the
complete authored string source without overlap or gaps, which lets diagnostics and formatting
project directly back to source.

Representative token shapes:

```text
"hello"            string_start string_text string_end
""                 string_start string_end
"hello ${name}"    string_start string_text interpolation_start identifier
                    interpolation_end string_end
```

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
- An interpolated string source form is an expression-level construct, not the non-interpolated
  `StringLiteral` grammar subset. Its type, allocation behavior, evaluation order, and formatting
  rules are specified in
  [Strings, Arrays, Views, and Pointers](07-strings-arrays-views-pointers.md#string-interpolation).
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
