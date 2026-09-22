# Durable Local Byte Storage

The compiler-checked [`store` module contract](index.nct) is the sole authority for the public
surface. `Store` is one process-exclusive, owning byte-keyed local state container. It copies keys
and values on `set`, lends retained values from `get`, stages removal in memory, and changes durable
state only when `commit` succeeds. Closing a dirty store discards its uncommitted in-memory changes.

`Store` is deliberately not a database abstraction. Application code owns serialization, schema
evolution, concurrent-task synchronization, and transaction boundaries. Opening a path acquires a
non-waiting operating-system lock on its stable `.lock` sibling. A second owner in the same or
another process fails with `std.store.already_open`; closing the store or terminating its process
releases ownership. The sibling may remain on disk because its bytes do not represent ownership.
Mutating the journal or lock sibling through another API remains unsupported.

## Bounds and Cost

`StoreLimits` bounds each key, each value, retained entry count, one complete snapshot, physical
journal bytes, and records retained between compactions. Recovery validates those bounds before
allocating lengths read from storage. The standard policy retains at most 64 KiB per key, 1 MiB per
value, 65,536 entries, a 16 MiB snapshot, a 256 MiB journal, and 4,096 records between compactions.

The current state representation preserves deterministic insertion order. Exact lookup,
replacement, and removal are linear in entry count; encoding one commit is linear in total retained
state. This tradeoff keeps the durable format independent of hash-table implementation details and
avoids imposing a hash contract on byte keys. Applications needing large indexed datasets should
use an external database rather than treating this bounded local store as one.

## Snapshot and Journal Formats

Each committed payload is a complete snapshot. A snapshot starts with the four-byte `NCTS` magic,
little-endian version and zero flags, and a 64-bit entry count. Each following entry contains 64-bit
key and value lengths followed by their exact bytes. The current snapshot version is `1`. Duplicate
keys, trailing bytes, unsupported flags or versions, and configured-limit violations are
corruption.

Each journal record contains the four-byte `NCTJ` magic, little-endian version and zero flags, a
nonzero 64-bit commit sequence, a 64-bit payload length, the snapshot payload, and an ISO-HDLC
CRC-32 of the complete header and payload. The current journal version is `1`. The first retained
sequence may be greater than one so compaction preserves global commit identity; every later record
must increment it by exactly one.

Recovery accepts a partial final header, or a valid final header whose complete body and checksum
do not reach the observed end, as an interrupted commit and truncates that tail before append.
Invalid complete journal magic, version, flags, sequence, checksum, configured length, or sequence
continuity is corruption. An invalid final snapshot, including a duplicate key, is likewise
corruption and never becomes visible state.

Recovery verifies the header, sequence, bounds, and checksum of every complete journal record, but
retains and decodes only the final complete snapshot. Superseded snapshot bodies are not copied
into a second recovery collection and are never reconstructed as obsolete application states.

Recovery builds one temporary seeded hash index while decoding, so duplicate validation compares
only equal-hash candidates rather than scanning every prior persisted key. The index retains only
hashes and entry positions and is discarded after the authoritative insertion-ordered state has
been reconstructed.

## Commit, Compaction, and Failure

Opening an absent path durably creates its directory entry before retaining one append owner. A
commit frames the complete current snapshot, writes every byte, synchronizes the file, and only then
advances the visible committed sequence. When the next record would exceed the configured journal
byte or record bound, commit closes the append owner and durably replaces the journal with that one
new record. `compact` performs the same replacement for clean state without advancing its sequence;
it rejects uncommitted changes.

Any cancellation, write failure, synchronization failure, or indeterminate replacement makes the
current owner terminal. The operation may already have reached storage, so callers must reopen the
path to resolve whether its sequence committed. Retrying through the same owner is forbidden.
Reopening deterministically accepts either the prior durable journal or the complete replacement,
never an in-memory mutation that did not cross a successful commit.

The journal does not duplicate byte order, checksum, file-service, or atomic replacement rules.
`std/bytes`, `std/checksum`, `std/io`, and `std/fs` remain the authorities for those mechanics.
