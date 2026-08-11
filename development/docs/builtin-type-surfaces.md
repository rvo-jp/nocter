# Built-in Type Source Surfaces

This document owns the compiler boundary that attaches source construction, instances, and
interface conformances to compiler-built-in types. Public construction, method, and formatting
behavior belongs in the
[string and view specification](../../spec/07-strings-arrays-views-pointers.md), and receiver
selection belongs in the [borrow-coercion specification](../../spec/22-borrow-coercions.md).

## Ownership Model

`str`, `[T]`, `error`, `bool`, and integer types are compiler identities, not nominal declarations
injected into the prelude. One compiler-level `BuiltinTypeOwner` registry owns each identity's
canonical spelling and source authority. Frontend loading, authority validation, resolver collection,
trusted primitive validation, type checking, and editor analysis consume that registry instead of
repeating owner or path tables.

Their public source members nevertheless require ordinary identities for type checking, lowering,
diagnostics, and editor navigation. The resolver therefore stores a
`BuiltinTypeSurface` beside the nominal symbol table. A surface records its registry owner,
source declaration span, generic shape, source-derived construction entries, methods, and
conformances. It does not create a nominal struct, fields, coercions, `drop`, or a source-visible
value name.

Each registry entry records one inherent-source module, allowed surface categories, conformance
authority, and whether the frontend must load that module implicitly:

| Owner | Source module | Instance | Construction | Implicit load |
|---|---|---:|---:|---:|
| `str` | `std/str` | yes | no | yes |
| `[T]` | `std/slice` | yes | no | yes |
| `error` | `std/error` | no | yes | yes |
| `bool`, integers | `std/num` | yes | yes | no |

Implicit loading is deduplicated by module, so multiple scalar owners may share `std/num` without
reloading it. Loading does not inject value imports. A non-implicit surface participates when its
module is loaded through ordinary source dependencies. Project files and other standard modules
cannot declare competing inherent surfaces. Authority validation uses the canonical installed
module path rather than a textual prefix or the current working directory.

## Conformance Authority

The same surface can retain ordinary `InterfaceConformance` records. The exact implicit standard
library may declare conformances for any registered built-in owner; project packages may not. The
authority is package-wide because the interface module and the type's inherent-instance module are
independent responsibilities.

`std/fmt` is the first consumer. It defines `Format` conformances for `str`, `bool`, and every
integer. Resolver qualification preserves the interface declaration and method source identities,
then attaches them to the matching built-in surface. `String: Format` is nominal and uses the
parallel package-coherent nominal conformance collector.

Type checking does not branch on built-in formatting kinds. Its common type-to-surface query feeds
conformance selection, method lookup, interface validation, associated-type normalization, and
protocol-method planning. This boundary is reusable by future standard equality and hashing
contracts without extending interpolation code.

## Declaration Validation

The resolver recognizes built-in target syntax once and validates its complete shape before
collecting source members. `instance str` has no generic parameters. `instance [T]` has exactly
one parameter, and its element reference must name that parameter. Built-in instance blocks
contain only public borrowed methods with bodies. `construct error` uses its canonical scalar
target, contains public members with bodies, produces `Self`, and has at most one default member.
Detached associated construction functions are rejected.

Duplicate method identities are diagnosed while surfaces are collected. Parser, type checker, IR,
and LSP code do not repeat module-authority rules. Malformed source is omitted from the surface and
reported through stable frontend or resolver diagnostics.

## Trusted Representation Boundary

Source methods implement public behavior. Operations that source cannot express use narrow,
typed `pub(/)` primitives in the exact implicit standard-library package, registered by module,
visibility, generic shape, parameter types, and result type:

- `std/str.str_len_raw`
- `std/str.str_ptr_addr_raw`
- `std/slice.slice_len_raw`
- `std/slice.slice_ptr_addr_raw`
- `std/error.new_error`

Lowering recognizes the resolved primitive role after registry validation. It never treats a
public member named `len`, `is_empty`, `ptr`, `bytes`, or `new` as an intrinsic. Higher operations
remain ordinary calls and source bodies. Native failure-payload lowering accepts the resolved
source member through its `(&str, &str) -> error` ABI shape, independently of its spelling.

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

Hover, completion, signature help, definition, references, and rename use the selected source
member's span. Direct `str`, slice, and `error` completion reads the built-in surfaces. Nominal
completion may add method-only candidates from one-step coercion targets; original names shadow
those candidates,
identical declaration spans collapse across capability paths, and different declarations remain
ambiguous.

Presentation renders the canonical concrete receiver (`&str`, `&[u8]`, or `&+[u8]`) while
navigation retains the declaration in `std/str`, `std/slice`, or `std/error`. No synthetic
`String`, `Vec<T>`, or `Error` member is created for editor convenience.

## Verification Boundary

Unit fixtures that deliberately construct a minimal Nocter home use one shared helper to install
valid built-in surfaces, including construction required by implicit loading. Distributed-home
tests use the packaged sources and cover direct views, owning-type receiver coercion, pointer
identity, capability selection, ownership conflicts, and editor declaration identity. A missing
implicitly loaded surface unit is an invalid installation
rather than a signal to restore compiler-invented members.

`std/string` and `std/vec` retain private raw-view helpers because their coercion and interface
implementations must expose initialized private representation. Those helpers are not public
compatibility aliases. User code reaches borrowed behavior through source-declared `str` and slice
methods, expected-type coercion, or explicit `as`.
