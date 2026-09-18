# CRC-32 Checksums

The compiler-checked [`checksum` module contract](index.nct) is the sole authority for public
signatures. It provides the reflected ISO-HDLC CRC-32 parameters: polynomial `0x04C11DB7`
(`0xEDB88320` in reflected form), initial value `0xFFFFFFFF`, input and output reflection, and final
exclusive-or `0xFFFFFFFF`. The check value for `123456789` is `0xCBF43926`.

`crc32` computes one complete checksum. `Crc32` applies the same state transition incrementally;
`update_byte` and `update` share that transition, and input chunk boundaries do not affect the
result. `value` does not mutate or consume the state, so repeated observation is stable and later
updates continue from the bytes already supplied. All operations are allocation-free.

The implementation owns one reflected four-bit lookup table. Standard-library tests derive all
sixteen entries from the reflected polynomial before checking public vectors and arbitrary chunk
boundaries. No transport, record framing, storage, or parsing component maintains another CRC-32
transition.

CRC-32 detects common accidental corruption. It is not collision-resistant, does not authenticate
data, and must not be used as a security boundary.
