# Source-Neutral Configuration

The compiler-checked [`config` module contract](index.nct) is the sole authority for the public API.
The module defines configuration policy independently of JSON, process environment, command-line,
filesystem, service, or logging representations.

## Model

A `Schema` owns an ordered set of exact field names. Each `Field` selects text, Boolean, signed, or
unsigned storage; may be required or secret; and may carry an inclusive constraint appropriate to
its type. Text length means UTF-8 byte length. Reversed bounds and constraints applied to the wrong
field type are rejected before the field can enter a schema.

A `Source` owns one diagnostic provenance label and uniquely named candidate values. It does not
choose precedence and cannot validate against a schema. A `Builder` consumes an owned schema and
applies sources in caller-authored order. A later source replaces an earlier value for the same
field. Duplicate names within one source and unknown fields are errors rather than implicit
last-write wins behavior.

Applying a source is failure-atomic. The builder validates and normalizes every candidate before it
changes any retained value. Text candidates are kept as text for text fields and may be parsed as
exact `true`/`false`, signed decimal, or unsigned decimal for the corresponding typed field. Already
typed candidates must match the declared kind exactly. Constraints are checked after normalization.

`finish` rejects an absent required field and consumes the builder into an immutable
`Configuration`. Typed accessors distinguish an absent optional value from an unknown field or a
wrong accessor. Each present value retains the label of the source that last supplied it.

## Secret Safety

Secret classification belongs to the schema, not to a source adapter. `display` always returns
`[redacted]` for a present secret field. Validation errors may contain the field name and source
label but never candidate bytes. Direct typed access remains available because application code
must be able to use a configured credential; callers that expose such a value outside the
configuration API remain responsible for that explicit disclosure.

## Source Adapters

The child modules [`config/json`](json/index.nct),
[`config/environment`](environment/index.nct), and
[`config/arguments`](arguments/index.nct) project existing standard-library values into the same
`Source` vocabulary. They never receive a schema or builder, so they cannot choose precedence,
validate a field kind, publish provenance independently, or bypass redaction.

`config/json.from_object` requires one JSON object. Exact member names become field names. JSON
strings and exact number tokens become textual candidates, leaving signed-versus-unsigned and
constraint decisions to the schema; Booleans remain typed. Null, nested objects, arrays, and a
non-object root are rejected. JSON object order has no configuration meaning.

`config/environment.add` performs one explicit field-to-variable mapping against the current
process environment. It applies no case conversion, prefix stripping, or other naming convention.
An absent variable adds no candidate. Process environment storage is immutable and process-lived
from Nocter code, so separately selected values observe the same process snapshot.

`config/arguments.add_flag`, `add_option`, and `add_positional` read an already validated
`ParsedArguments`; they never reparse token text. Only explicitly present arguments add candidates.
The caller supplies the Boolean produced by an occurring flag, allowing either enabling or
disabling flags. A repeated option cannot collapse into one scalar candidate and is rejected.
The underlying `ParsedArguments` API treats an undefined name like an absent name, so applications
should keep adapter mappings next to their command-line schema; required configuration fields still
detect a missing required mapping at finalization.

Authored defaults use `Source` directly. A typical application applies defaults, a JSON source,
selected environment variables, and selected command-line arguments in that authored order. No
adapter kind has an intrinsic precedence.
