# Built-in Type Method Surfaces

This document owns the compiler boundary that attaches source declarations to built-in unsized
types. Public method behavior belongs in the [string and view specification](../../spec/07-strings-arrays-views-pointers.md), and receiver selection belongs in the
[borrow-coercion specification](../../spec/22-borrow-coercions.md).

## Ownership Model

`str` and `[T]` are compiler type identities, not nominal declarations injected into the prelude.
One compiler-level `BuiltinTypeOwner` registry owns each identity's canonical spelling and
implementation module. Frontend loading, authority validation, resolver collection, trusted
primitive validation, type checking, and editor analysis consume that registry instead of
repeating owner or path tables.

Their inherent methods nevertheless require ordinary source identities for type checking,
lowering, diagnostics, and editor navigation. The resolver therefore stores a
`BuiltinTypeSurface` beside the nominal symbol table. A surface records its registry owner,
implementation span, generic shape, and source-derived method signatures; it does not create a
constructible struct, fields, coercions, `drop`, or a source-visible type name.

The active Nocter home owns exactly one implementation unit per built-in owner:

| Owner | Authority |
|---|---|
| `str` | `std/str` |
| `[T]` | `std/slice` |

The frontend loads both units for every package analysis. Loading does not inject value imports.
Project files and other standard modules cannot declare competing built-in implementations.
Authority validation uses the canonical installed module path rather than a textual prefix or the
current working directory.

## Declaration Validation

The resolver recognizes the built-in target syntax once and validates its complete shape before
collecting methods. `impl str` has no generic parameters. `impl<T> [T]` has exactly one parameter,
and its element reference must name that parameter. Built-in implementation blocks contain only
public borrowed methods with bodies. They cannot implement interfaces, declare owned receivers,
construct values, or attach unrelated members.

Duplicate method identities are diagnosed while surfaces are collected. Parser, type checker, IR,
and LSP code do not repeat module-authority rules. Malformed source is omitted from the surface and
reported through stable frontend or resolver diagnostics.

## Trusted Representation Boundary

Source methods implement public behavior. Operations that source cannot express use narrow,
typed `pub(nocter)` primitives registered by module, visibility, generic shape, parameter types,
and result type:

- `std/str.str_len_raw`
- `std/str.str_ptr_addr_raw`
- `std/slice.slice_len_raw`
- `std/slice.slice_ptr_addr_raw`

Lowering recognizes the resolved primitive role after registry validation. It never treats a
public member named `len`, `is_empty`, `ptr`, or `bytes` as an intrinsic. Higher operations remain
ordinary calls and source bodies.

Pointer extraction uses typed `StrPointer` and `SlicePointer` IR values so the backend selects the
data word explicitly instead of reconstructing view layout from offsets. Pointer-returning normal
calls share the scalar-word ABI path, allowing a source method result to flow into `std/ptr.addr`
and other pointer consumers.

## Receiver Plans

Typecheck first performs ordinary inherent and interface lookup on the original receiver. If no
candidate exists, it enumerates accessible one-step borrow coercions, applies the concrete source
substitutions, and looks up the requested method on each exact target owner. An original candidate
shadows coercion even when its receiver capability is unavailable.

Target capability participates in selection. A readwrite method requires a readwrite coercion
target. When readonly and readwrite paths reach the same readonly declaration, selection retains
the minimum-capability path instead of reporting a false ambiguity. Distinct declarations remain
ambiguous. The resulting method-call fact owns the selected coercion, method declaration,
substitution, and callable target; ownership, provenance, IR, and analysis consume that fact.

Receiver-coercion results borrow the original source place. Borrow collection records a whole-place
loan for owned aggregates even when their fields contain borrow-like values. This prevents move,
drop, mutation, and region escape until the coerced result's last use without treating every
borrow-containing aggregate expression as an already borrowed value.

## Editor Boundary

Hover, completion, signature help, definition, references, and rename use the selected method's
source span. Direct `str` and slice completion reads the built-in surfaces. Nominal completion may
add method-only candidates from one-step coercion targets; original names shadow those candidates,
identical declaration spans collapse across capability paths, and different declarations remain
ambiguous.

Presentation renders the canonical concrete receiver (`&str`, `&[u8]`, or `&+[u8]`) while
navigation retains the declaration in `std/str` or `std/slice`. No synthetic `String` or `Vec<T>`
member is created for editor convenience.

## Verification Boundary

Unit fixtures that deliberately construct a minimal Nocter home use one shared helper to install
valid built-in surfaces. Distributed-home tests use the packaged sources and cover direct views,
owning-type receiver coercion, pointer identity, capability selection, ownership conflicts, and
editor declaration identity. A missing implementation unit is an invalid installation rather than
a signal to restore compiler-invented methods.

`std/string` and `std/vec` retain private raw-view helpers because their coercion and interface
implementations must expose initialized private representation. Those helpers are not public
compatibility aliases. User code reaches borrowed behavior through source-declared `str` and slice
methods, expected-type coercion, or explicit `as`.
