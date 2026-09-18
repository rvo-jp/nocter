# Binary Scalar Codecs

The compiler-checked [`bytes` module contract](index.nct) is the sole authority for public
signatures. This guide records the failure and composition guarantees shared by those declarations.

`PrefixDecode<T>` is the shared result for variable-width scalar prefixes. `decoded` contains the
value and exact consumed width. `incomplete` means additional input could complete the prefix.
`overflow` means the representation cannot fit its declared scalar domain. `non_canonical` means
the same value has a shorter accepted representation. Only `decoded` authorizes cursor movement;
the other variants contain no partial value or speculative width.

Every fixed-width decoder reads one prefix and returns `none` before producing a value when the
input is too short. Bytes after that prefix are not inspected. Unsigned and signed integer decoding
preserves the exact mathematical or two's-complement representation and does not depend on the
target's native byte order. Floating-point decoding preserves the complete IEEE bit representation,
including signed zero and NaN payloads.

Every fixed-width encoder checks output capacity before the first mutation, so insufficient output
returns `false` with every byte unchanged. Bytes after the encoded prefix are untouched. Encoding
performs no allocation.

`decode_uleb128` and `encode_uleb128` own canonical unsigned LEB128. A terminal zero payload after
the first byte is non-canonical. More than ten bytes, a continuing tenth byte, or a tenth payload
greater than one overflows `u64`. `decode_zigzag_uleb128` and `encode_zigzag_uleb128` apply the
ZigZag mapping across the complete `i64` range before using that same unsigned representation.
Variable-width encoders return the exact committed width, or return `none` without changing output
when the complete encoding does not fit.

`uleb128_width` and `zigzag_uleb128_width` expose the same canonical width decision used by the
encoders. Storage owners may reserve or validate a complete destination before exposing writable
capacity, but they do not reproduce the representation algorithm.

This module does not own input position or storage. Use [`scan.ByteCursor`](../scan/README.md) to
select and advance through borrowed input, [`fixed.ByteBuffer`](../fixed/README.md) for inline
bounded staging, and [`io`](../io/README.md) for external streams. Those modules delegate scalar
conversion to this contract rather than defining additional byte-order rules.

Dynamic byte vectors reserve the reported width and let this module encode directly into
uncommitted storage. Blocking and asynchronous writers accept the resulting complete slice. No
storage or stream adapter owns another endian or variable-width implementation, and a failed
encoding cannot publish a partial value to any destination.
