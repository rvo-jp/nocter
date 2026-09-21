# Durable Local Byte Storage

The compiler-checked [`store` module contract](index.nct) is the sole authority for the public
surface. `Store` is one owning, byte-keyed local state container. It copies keys and values on
`set`, lends retained values from `get`, stages removal in memory, and changes durable state only
when `commit` succeeds. Closing a dirty store discards its uncommitted in-memory changes.

`Store` is deliberately not a database abstraction. Application code owns serialization, schema
evolution, concurrent-task synchronization, and transaction boundaries. One store has one writer;
opening the same path more than once, using it from multiple processes, or mutating its file through
another API is unsupported.

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
Invalid complete magic, version, flags, sequence, checksum, configured length, duplicate snapshot
key, or sequence continuity is corruption and never becomes visible state.

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
