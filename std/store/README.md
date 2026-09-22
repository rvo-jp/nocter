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

The authoritative state preserves deterministic insertion order while a retained seeded hash index
maps each observed hash to exact-key candidate slots. Exact lookup, replacement, and removal have
expected constant cost and always confirm the complete key after hashing. Removed slots join a free
list and can be reused without shifting later entries or changing insertion order. An ordinary
commit encodes only the staged mutations. A checkpoint is linear in total retained state. The hash
index is transient: it neither determines observable order nor appears in the durable format.

One in-memory mutation batch records changes since the last successful commit. Its encoded size is
bounded by the complete-record limit. If another change would exceed that bound, the batch releases
its accumulated copies and remembers only that the next commit requires a checkpoint. Retained
entries remain the sole current-state authority in either case; the batch is publication intent,
not a second state representation.

`entries()` returns an allocation-free lending cursor over the authoritative insertion order. Each
yield borrows the retained key and value without copying them, and the borrow ends before the cursor
can advance again. The cursor reports its exact remaining length. Mutating the store while such a
cursor or yielded entry is live is rejected by the ownership checker rather than synchronized at
runtime.

## Snapshot and Journal Formats

A checkpoint payload is a complete snapshot. A snapshot starts with the four-byte `NCTS` magic,
little-endian version and zero flags, and a 64-bit entry count. Each following entry contains
64-bit key and value lengths followed by their exact bytes. The current snapshot version is `1`.
Duplicate keys, trailing bytes, unsupported flags or versions, and configured-limit violations are
corruption.

Each journal record contains the four-byte `NCTJ` magic, little-endian version and flags, a nonzero
64-bit commit sequence, a 64-bit payload length, the payload, and an ISO-HDLC CRC-32 of the complete
header and payload. Journal version `2` is current. Flag `1` identifies a complete checkpoint whose
payload is the snapshot format above. Flag `2` identifies an atomic mutation batch. The first
retained sequence may be greater than one because checkpoint replacement preserves global commit
identity; every later record must increment it by exactly one.

A mutation batch starts with the four-byte `NCTM` magic, little-endian version `1`, zero flags, and a
64-bit operation count. Each operation has a one-byte kind, seven zero reserved bytes, 64-bit key
and value lengths, and the exact key and value bytes. Set is kind `1`. Remove is kind `2` and must
have a zero value length. Recovery applies operations in authored order only after the outer record
is complete and its checksum is valid, so a batch is entirely visible or entirely absent.

Recovery accepts a partial final header, or a valid final header whose complete body and checksum
do not reach the observed end, as an interrupted commit and truncates that tail before append.
Invalid complete journal magic, version, flags, sequence, checksum, configured length, sequence
continuity, snapshot, or mutation encoding is corruption and never becomes visible state. A current
journal cannot mix current and legacy records.

Version `1` is the published v0.66.0 legacy format: zero flags and one complete snapshot in every
record. Recovery validates every complete legacy record, retains only the final snapshot, and then
durably replaces the file with one version `2` checkpoint at the same committed sequence. The
writer never appends a current record to a legacy file and never guesses a version from payload
bytes. Version classification is owned by journal framing; Store replay sees an explicit record
kind.

Current recovery starts from an empty state or a checkpoint and applies each later complete mutation
batch once. A first mutation record is valid only at sequence one, where its base is the empty
store. A checkpoint replaces, rather than supplements, earlier retained state. Recovery constructs
the authoritative insertion-ordered state and its retained seeded hash index together. Duplicate
snapshot validation therefore compares only equal-hash candidates rather than scanning every prior
persisted key. Recovered lookup uses that same index; recovery does not build and then discard a
second representation.

## Commit, Compaction, and Failure

Opening an absent path durably creates its directory entry before retaining one append owner. A
normal commit frames the complete staged mutation batch, writes every byte, synchronizes the file,
and only then advances the visible committed sequence. If staging exceeded the complete-record
bound, or appending would exceed the journal byte or record bound, commit deterministically encodes
the authoritative state and durably replaces the journal with one checkpoint at the new sequence.
`compact` performs the same checkpoint replacement for clean state without advancing its sequence;
it rejects uncommitted changes.

Any cancellation, write failure, synchronization failure, or indeterminate replacement makes the
current owner terminal. The operation may already have reached storage, so callers must reopen the
path to resolve whether its sequence committed. Retrying through the same owner is forbidden.
Reopening deterministically accepts either the prior durable journal or the complete replacement,
never an in-memory mutation that did not cross a successful commit.

The journal does not duplicate byte order, checksum, file-service, or atomic replacement rules.
`std/bytes`, `std/checksum`, `std/io`, and `std/fs` remain the authorities for those mechanics.
