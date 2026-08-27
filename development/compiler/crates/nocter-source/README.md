# nocter-source

## Responsibility

Own normalized UTF-8 source bytes, source identities, byte ranges, spans, line indexes, and
UTF-8/UTF-16 coordinate conversion.

## Contract

Consumers receive immutable `SourceFile` and `SourceMap` values and validated coordinate
conversions. The crate does not lex, parse, resolve names, or assign semantic meaning to a range.

## Internal Responsibilities

- source storage and identity
- half-open byte spans and ranges
- line indexing
- validated LSP coordinate conversion

## Invariants

- Compiler phases store normalized byte offsets, never editor positions.
- Invalid UTF-8 or invalid coordinate boundaries are rejected at this owner.
- A source range cannot identify a semantic entity without a separate projection contract.
