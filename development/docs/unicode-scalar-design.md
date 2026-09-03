# Unicode Scalar Representation Boundary

This document owns the cross-crate representation and authority boundary for v0.34.0 Unicode scalar
values. Public behavior belongs in [`spec/34-unicode-scalars.md`](../../spec/34-unicode-scalars.md).

## Authority Chain

```text
UTF-8 source
  -> syntax token + syntax-owned decoded literal
  -> checked char constant
  -> MIR char constant
  -> runtime primitive Char
  -> machine scalar selected from stored layout
  -> ARM64 immediate or register value
```

Syntax alone scans quote boundaries, escape spelling, and decoded scalar cardinality. Its public
literal decoder returns a typed byte or Unicode scalar value. Checking selects the built-in type and
publishes a typed constant; it cannot inspect quote contents. Constant evaluation carries the same
source-independent scalar. MIR and Machine retain a character constant variant so validation cannot
mistake a `char` value for an integer merely because both occupy 32 bits.

`RuntimePrimitive::Char` is distinct from `RuntimePrimitive::Unsigned(32)`. Machine layout assigns
both size 4 and alignment 4 but retains their separate identities. ABI classification consumes the
stored scalar layout. ARM64 selection consumes an already validated scalar immediate and cannot
decide Unicode validity.

## Dynamic Construction

Only source literals and one package-internal primitive boundary create a `char` from integer bits.
The public `char.from_u32` implementation checks the scalar range in ordinary Nocter source before
calling that primitive. The inverse package-internal primitive exposes the exact scalar as `u32`.
Both primitive roles are pure scalar transport; they do not own Unicode validity policy.

The primitive declarations live in the standard `char` module, the bundled standard profile names
their exact physical locations, Target validates their signatures once, and ARM64 implements them as
representation-preserving scalar moves. User packages cannot declare or call the unchecked role.

## UTF-8 Authority

`std/internal/utf8` owns one decode step over a byte view and offset. The result distinguishes a
decoded scalar and width from failure; callers test end-of-input before requesting a step. Complete
validation repeatedly consumes this operation, and `Chars` consumes the same operation over a
borrowed valid `str`. Encoding remains owned by the existing `encode_scalar` operation.

The decoder returns `u32`, not `char`, to keep the low-level UTF-8 module independent of the public
`char` module. `Chars` converts the already validated scalar through `char.from_u32`; failure there is
an internal invariant violation, not an alternate end-of-iteration result.

`String.try_push(char)` encodes first, reserves complete capacity second, writes all initialized
bytes third, and publishes the new logical length last. Allocation failure therefore cannot expose
a partial scalar. Formatting delegates to this operation and does not encode UTF-8 independently.

## Required Invariants

- `char` and `u32` never share semantic or runtime primitive identity.
- Every stored `char` is a Unicode scalar.
- Syntax decoding is the only interpretation of authored character and byte literal text.
- UTF-8 leading-byte, continuation, overlong, surrogate, and maximum-value rules have one standard-
  library implementation.
- Every `Chars` offset is either the byte length or a scalar boundary in its retained text.
- Layout and ABI are computed from runtime shape once; ARM64 does not infer width from source types.
- Formatter and editor features consume token and semantic products rather than parsing quotes.
- A malformed literal remains a source diagnostic and cannot become an internal compiler error.
