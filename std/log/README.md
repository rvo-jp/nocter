# Structured Operational Logging

The compiler-checked [`std/log` contract](index.nct) represents one event as an explicit timestamp,
severity, stable event name, and uniquely named typed fields. It does not read a clock, discover
request context, use a process-global logger, or select an output destination. Applications pass
those capabilities at the boundary where they are owned.

`Event.to_json` and `Event.to_json_line` reuse `std/json`, so JSON number spelling and escaping have
one authority. `write_json_line` accepts an explicit `BlockingWriter` and appends one LF byte after
the compact JSON value. Field order and JSON object member order are not an API contract.

`Field.redacted` does not accept or retain a secret. It records only the fixed marker
`"[redacted]"`. This makes redaction a construction boundary rather than a rendering option that
could later be disabled. Types such as `SessionId` that intentionally omit generic formatting
cannot enter a text field without an explicit reveal operation.

Events reject empty names, names containing control bytes, and duplicate field names. The API does
not impose a global schema for event or field names; applications should use stable dotted event
names and stable field meanings within their own compatibility boundary.
