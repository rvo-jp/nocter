# JSON Values and Text

This file is part of the Nocter language specification. The specification entry point is
[README.md](README.md).

## Future Direction: v0.22.0 JSON API

The contract in this chapter is adopted for v0.22.0 but is not implemented by the published
v0.21.0 toolchain. Until the implementation milestone removes this notice, `std/json` is not a
current standard module.

The future `std/json` module represents and exchanges JSON as defined by
[RFC 8259](https://www.rfc-editor.org/rfc/rfc8259). It is an ordinary standard-library module. The
compiler does not recognize JSON values, JSON number spelling, object names, parsing, or generation
as language primitives.

## Public Surface

```nct
use /io.Writer
use /map.Map
use /mem.TryAllocator
use /string.String
use /vec.Vec

/// One exact, validated JSON number token.
pub struct Number

/// One owning JSON value.
pub enum Value {
    null
    boolean(value: bool)
    number(value: Number)
    string(value: String)
    array(value: Vec<Value>)
    object(value: Map<String, Value>)
}

construct Number {
    /// Validates and copies exactly one JSON number token.
    pub func parse(text: &str): Self!

    /// Validates and copies exactly one JSON number token with recoverable allocation.
    pub func try_parse(allocator: &+TryAllocator, text: &str): Self! from allocator
}

instance Number {
    /// Returns the exact validated token spelling retained by this value.
    pub method &self.text(): &str

    /// Returns the mathematical value when it is exactly representable as i64.
    pub method &self.as_i64(): i64?

    /// Returns the mathematical value when it is exactly representable as u64.
    pub method &self.as_u64(): u64?
}

/// Parses one complete JSON text using the current allocation context.
pub func parse(text: &str): Value!

/// Parses one complete JSON text with recoverable allocation.
pub func try_parse(allocator: &+TryAllocator, text: &str): Value! from allocator

/// Generates one compact JSON text using the current allocation context.
pub func stringify(value: &Value): String

/// Generates one compact JSON text with recoverable allocation.
pub func try_stringify(
    allocator: &+TryAllocator,
    value: &Value,
): String! from allocator

/// Writes one compact JSON text without constructing a complete intermediate String.
pub func write<W>(destination: &+W, value: &Value): void! where W impl Writer

/// Writes one compact JSON text with recoverable traversal-stack allocation.
pub func try_write<W>(
    allocator: &+TryAllocator,
    destination: &+W,
    value: &Value,
): void! where W impl Writer
```

`Number`, `Value`, `parse`, `try_parse`, `stringify`, `try_stringify`, `write`, and `try_write` are
the complete initial public surface. Parsing from a stream, pretty printing, canonical member
ordering, generic serialization derivation, and floating-point conversion are not implied by these
declarations.

## Common Use

Parse one complete text and return its compact spelling:

```nct
use std/json.{parse, stringify}
use std/string.String

func normalize(text: &str): String! {
    let value = parse(text)?
    return stringify(&value)
}
```

Write the same compact spelling directly to any `Writer` without first constructing the complete
output String:

```nct
use std/io.Writer
use std/json.{parse, write}

func normalize_into<W>(destination: &+W, text: &str): void! where W impl Writer {
    let value = parse(text)?
    write(destination, &value)?
    return
}
```

Use one explicit recoverable allocator for both the owning value and returned text:

```nct
use std/json.{try_parse, try_stringify}
use std/mem.TryAllocator
use std/string.String

func try_normalize(allocator: &+TryAllocator, text: &str): String! from allocator {
    let value = try_parse(allocator, text)?
    return try_stringify(allocator, &value)?
}
```

The complete runnable [json-normalize example](../examples/json-normalize/index.nct) composes
process arguments, UTF-8 filesystem input, parsing, public error reporting, and Writer generation.
It uses no JSON-specific filesystem or operating-system operation.

## JSON Value Model

`Value` owns all decoded data. A successful parser result retains no borrow of its input. String
values and object names are decoded into owning `String` values; arrays own `Vec<Value>`; objects
own `Map<String, Value>`.

An object is an unordered mapping. Parsing does not preserve source member order, and generation
does not promise a stable member order across processes, collection seeds, or standard-library
versions. Programs that need ordered or canonical output require a separate future contract rather
than relying on the current private Map representation.

JSON object names must be unique after escape decoding. The parser rejects duplicates instead of
silently selecting the first or last member. Therefore `"name"` and `"\u006Eame"` are duplicate
names in the same object. This policy follows RFC 8259's interoperability guidance and preserves
the one-value-per-equality-class invariant of `Map`.

Array order is preserved exactly. A complete JSON text may contain any JSON value at its root; it
is not restricted to an object or array.

## Exact Number Model

`Number` stores the exact validated JSON number token, including its sign, fraction spelling,
exponent marker case, and exponent sign. Parsing and generation do not first convert through a
binary floating-point representation. For example, `-0`, `1.0`, `1e2`, and `1E+2` retain those
spellings.

A valid number follows the complete RFC 8259 number grammar:

- an optional leading `-`;
- `0` or a nonzero decimal digit followed by decimal digits;
- an optional fraction containing `.` and at least one decimal digit;
- an optional exponent containing `e` or `E`, an optional sign, and at least one decimal digit.

Leading `+`, leading zeroes, missing fraction digits, missing exponent digits, `NaN`, and infinity
are invalid. `Number.parse` and `Number.try_parse` accept exactly one number token and do not accept
leading or trailing JSON whitespace.

`as_i64` and `as_u64` use mathematical value, not token shape. They accept a fraction or exponent
only when the resulting value is an exact integer in range. Thus `1.0` and `1e2` may produce `1`
and `100`, while `1.5`, `-1` as `u64`, and an out-of-range magnitude produce `none`. Conversion is
allocation-free and never rounds.

This exact token model deliberately does not define equality between `Number` values. Numeric
equivalence, such as whether `1`, `1.0`, and `1e0` compare equal, requires a separate explicit
contract. `Value` therefore gains no implicit recursive equality operation in this milestone.

## Parsing

`parse` and `try_parse` accept exactly one complete JSON text with optional JSON whitespace before
and after it. JSON whitespace consists only of space, horizontal tab, line feed, and carriage
return. Non-JSON extensions such as comments, trailing commas, unquoted names, single-quoted
strings, and non-decimal numbers are rejected.

The input type `&str` already guarantees well-formed UTF-8. JSON string parsing additionally:

- rejects an unescaped quotation mark, reverse solidus, or U+0000 through U+001F;
- recognizes every RFC 8259 two-character escape;
- decodes `\uXXXX` escapes case-insensitively;
- combines a valid UTF-16 surrogate pair into one Unicode scalar value;
- rejects an unpaired high or low surrogate;
- compares decoded object names by their resulting UTF-8 encoding bytes.

A leading U+FEFF byte order mark is rejected. Generators never emit one. This keeps one strict input
grammar while preserving the RFC requirement that exchanged JSON use UTF-8.

The parser uses an owning, explicit container stack rather than the native call stack for JSON
nesting. The API sets no separate fixed nesting-depth limit. Input length, owned collection
capacity, and available allocation still impose ordinary target resource limits.

## Generation

`stringify`, `try_stringify`, `write`, and `try_write` generate grammar-conforming compact UTF-8
JSON without insignificant whitespace. They emit:

- `null`, `true`, and `false` for the corresponding scalar variants;
- the exact validated token retained by `Number`;
- array elements in their `Vec` order;
- object members in the Map's unspecified iteration order.

String generation escapes quotation mark and reverse solidus. It uses `\b`, `\f`, `\n`, `\r`, and
`\t` for those control values, `\u00XX` with uppercase hexadecimal digits for every other control
value, and emits every other Unicode scalar directly as UTF-8. Solidus is not escaped. These rules
give strings one generated spelling without promising canonical object order or normalized number
spelling.

`write` and `try_write` forward a destination `Writer` failure unchanged and do not construct the
complete JSON text in memory. Traversal uses an explicit stack proportional to value nesting plus
fixed-size local encoding buffers; it does not allocate a second JSON tree. `write` allocates that
stack in the current context, while `try_write` uses the supplied `TryAllocator`. The writer remains
responsible for its own internal allocation and I/O policy.

## Allocation and Failure

`parse`, `stringify`, and `write` use the current allocation context. Failure to allocate terminates
under the ordinary allocator policy. `try_parse`, `try_stringify`, and `try_write` route every
parser, result, output, or traversal allocation they own through the supplied `TryAllocator`;
allocation errors retain their existing `std.mem.*` code. A destination writer still owns its own
failure and allocation policy. No partially constructed `Value` or `String` is returned.

Parsing rejects malformed input with built-in `error` code `std.json.invalid_syntax`. A duplicate
decoded object name uses `std.json.duplicate_name`. The error message identifies the zero-based
UTF-8 byte offset at which parsing could no longer satisfy the contract, but exact diagnostic prose
is not stable API. `Number.parse` uses `std.json.invalid_syntax` with an offset relative to its
number-token input.

Failure cleanup owns one source of truth: the parser's token under construction, explicit
container stack, and optional completed root. Every owner is moved forward or dropped once. No
parallel initialized-entry table or caller-supplied cleanup count is part of the public or private
contract.

## Non-goals

The initial JSON API does not provide:

- comments, trailing commas, JSON5, or another extended grammar;
- duplicate-name retention or first/last-wins behavior;
- source byte ranges on every `Value` node;
- borrowed or lazy DOM values;
- event-based or streaming input parsing;
- pretty, sorted, or canonical generation;
- `f32` or `f64` conversion;
- reflection, attributes, or automatic struct-to-JSON derivation;
- compiler-known JSON syntax or lowering.

These features require separate contracts. They must not be inferred from private parser or Map
implementation details.
