# nocter-source

## Responsibility

Own normalized UTF-8 source bytes, source identities, byte ranges, spans, line indexes, and
UTF-8/UTF-16 coordinate conversion.

## Contract

Consumers receive immutable `SourceFile` and `SourceMap` values and validated coordinate
conversions. `SourceFile` also owns the exact comparison between normalized source and a named raw
byte input, so authority boundaries do not reproduce ingestion rules. Every `SourceId` combines a
compact local index with an opaque issuance identity. A map lookup validates both parts instead of
trusting the index. `SourceIdentityDomain` lets one revision family retain that complete identity
only while the exact physical name and normalized text remain current. The crate does not lex,
parse, resolve names, or assign semantic meaning to a range.

## Internal Responsibilities

- source storage and identity
- half-open byte spans and ranges
- line indexing
- validated LSP coordinate conversion

## Invariants

- Compiler phases store normalized byte offsets, never editor positions.
- Invalid UTF-8 or invalid coordinate boundaries are rejected at this owner.
- Independently ingested sources never compare equal merely because they occupy the same local
  index.
- Independently created identity domains never alias, even for equal names and bytes.
- Source ordering follows the local index in the current immutable map. The opaque issuance part
  is only a foreign-value tiebreaker and cannot make semantic order depend on process history.
- Maps in one revision domain reuse an identity for unchanged source text. Any content transition
  issues a new identity, including a later return to earlier text, so source-backed cache entries
  cannot become current again accidentally.
- A revision domain retains only the current normalized text per physical name. `SourceFile` shares
  that immutable storage instead of duplicating it.
- Debug output treats the revision domain as an authority token and never expands its retained
  source names or text.
- `SourceName` remains display and lookup metadata. Package or source-graph owners, not this byte
  store, decide where a canonical name must be unique.
- A source range cannot identify a semantic entity without a separate projection contract.
