# Binary Scalar Codecs

The compiler-checked [`bytes` module contract](index.nct) is the sole authority for public
signatures. This guide records the failure and composition guarantees shared by those declarations.

Every decoder reads a fixed-width prefix and returns `none` before producing a value when the input
is too short. Bytes after that prefix are not inspected. Integer decoding is mathematical and does
not depend on the target's native byte order. Floating-point decoding preserves the complete IEEE
bit representation, including signed zero and NaN payloads.

Every encoder writes a fixed-width prefix. It checks output capacity before the first mutation, so
insufficient output returns `false` with every byte unchanged. Bytes after the encoded prefix are
untouched. Encoding performs no allocation.

This module does not own input position or storage. Use [`scan.ByteCursor`](../scan/README.md) to
select and advance through borrowed input, [`fixed.ByteBuffer`](../fixed/README.md) for inline
bounded staging, and [`io`](../io/README.md) for external streams. Those modules delegate scalar
conversion to this contract rather than defining additional byte-order rules.

Dynamic output copies one completely encoded fixed-buffer view through
[`Vec.extend_from_slice`](../vec/README.md). Blocking and asynchronous writers already accept the
same complete slice. No dynamic or stream adapter owns another endian implementation, and failed
fixed-buffer encoding cannot expose a partial value to either destination.
