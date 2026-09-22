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

JSON, environment, and CLI adapters are separate responsibilities. They may construct a `Source`,
but they must not implement their own precedence, schema validation, constraints, provenance, or
redaction rules.
