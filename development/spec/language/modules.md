# Modules, Use Declarations, and Source Visibility

## Scope

This chapter owns the adopted v2.0.0 directory, source-visibility, contract-placement and
contract/definition rules. [Type behavior](type-behavior.md) and
[interfaces](interfaces.md) own member semantics; [syntax](syntactic-grammar.md#files-and-declarations)
owns recognition. These are unreleased contracts, not claims of compiler or package-tool support.
An admitted file role does not admit an otherwise unclosed generic, literal, enum, target,
primitive, async or constant-evaluation feature.

## Directory Modules and Source Namespaces

A directory containing `index.nct` defines a module. A source belongs to its nearest ancestor
module within its package. Directories without `index.nct` are source folders, not namespaces;
source basenames do not define modules. Module identity consists of exact package identity and
normalized module-directory path. No `module` declaration or special `internal.nct` role exists.

```text
project/
    index.nct
    buffer.nct                 // root module
    details/storage.nct        // root module: details has no index.nct
    codec/index.nct            // child module codec
    codec/decode.nct           // child module codec
```

Selecting a module inventories all its `.nct` sources, descending through source folders and
stopping at another module or package. Inventory is not source visibility. Every selected source
must be checked, including unused helpers; inventory does not put its private names in another
source's scope. Compiler access to bodies for verification or optimization likewise grants no
source-level access. A semantic declaration identity is distinct from its physical source location.

Unqualified authored lookup uses lexical bindings/parameters, the current source's declarations,
and declarations authored in directly seen sources. Authored declarations/imports that collide in
one source scope are errors rather than an ordered search. A contract and its checked definition
are one identity, not a collision. Imports and any synthetic prelude are source-local; seeing a
source does not inherit its imports. Compiler-managed fallback names do not become authored
exports. This chapter selects no v2.0.0 prelude export list or package discovery implementation.

## Direct see Dependencies

`see` names one physical `.nct` source, using `./` or complete repeated `../` components relative
to the authored source directory. Both canonical sources must belong to the same module and
package. The target must exist. Reject directories, omitted extensions, package-absolute paths,
dependency aliases, crossing a module boundary, and embedded traversal such as `./../`.

A `see` is top-level, directional, direct-only and unqualified. It has no alias, selection,
visibility directive or block form. It exposes only declarations authored in its exact target;
neither that target's `see` edges nor its `use` bindings are propagated. Cycles and repeated edges
have set semantics and do not recursively load or export names.

```text
// a.nct
see ./b.nct
// b is visible here; c is not visible merely because b.nct sees c.nct.
```

Contract roots may use `see` to identify their implementation sources. This does not publish
helpers in those sources. Reciprocal `see` is required for a public contract/definition pair;
ordinary helper visibility requires only the direction actually used. `see` constrains what an
author must inspect, not the compiler's whole-program optimization scope.

## use and Re-exports

`use` selects a directory module, never an ordinary source. Module paths use `/` without `.nct`:
`/codec` is package-root-relative, `./codec` and repeated `../` are source-directory-relative,
and a non-relative first segment names a selected dependency. Relative resolution cannot leave
the package. Namespace members use `.` after the module path has resolved.

```text
use ./codec                  // bind namespace codec
use ./codec as encoding      // bind namespace encoding
use ./codec.Decoder          // bind a type-namespace name
use ./codec.{Decoder, Format as WireFormat}
use / as root                // a root namespace needs an explicit local name
```

Selected names are types, aliases or interfaces. Functions, constants and statics remain under
their module namespace; `use ./codec.decode` cannot flatten a function into the local value
scope. No wildcard, dotted module path or extension-probing form exists. Bindings and aliases
cannot collide with other authored names in the same namespace/scope. The value and type
namespaces remain distinct. Module imports/re-exports cannot form a cycle; `see` cycles remain
independent and valid.

Every authored `use` in `index.nct` is both a local binding and a re-export, including an aliased
namespace or selected name. A root has no import-only escape hatch. Removing a re-export is an
API change even when no signature still uses it. Re-exporting preserves the target identity and
does not create another type, conformance, body or flattened namespace. In ordinary sources and
blocks, `use` only creates local bindings. Top-level dependencies precede items; block imports
precede executable entries. A block import is a compile-time dependency regardless of reachability.

## Contract-Only Roots

Only `index.nct` authors a module's outward contract. Omitted visibility means external access;
`pub` is not a v2.0.0 declaration form. Being inventoried, seen, or used by a public body does not
publish a private declaration. No executable body or value initializer may occur in a root.

| Subject | index.nct | Ordinary source |
| --- | --- | --- |
| Public function, method, construction, literal, coercion or operator | Complete bodyless contract | Matched complete header and body |
| Public struct | Nominal/Copy contract and selected visible fields; a structural grant closes the schema | Complete representation when the root does not close it |
| Public enum | Complete braced variant schema, or an opaque nominal/Copy header | Complete representation only for an opaque root |
| Public interface | Associated types, required signatures, explicit default signatures | Matched default bodies only |
| Public type's conformance | Interface application and associated bindings | Matching implementation bindings, where needed |
| Public constant/static | Name and data type, no initializer | One matching value definition |
| Private helper, type or implementation import | Not permitted | Full private declaration/definition |
| Destructor or test body | Not permitted | Full body under its owning rules |
| Documentation, see, package metadata | Permitted in their selected roles | Only where that role permits |

The root can name a package entry contract such as `main`, but its body belongs in an ordinary
source. Metadata does not export a name. A root's `impl Type` is a container of contracts, not an
executable implementation. A signature-only private helper in the root is not a private exception.
Target/primitive/package metadata requires its own selected contract; this table does not supply it.

### Nominal Representation and Visible Fields

A root struct without a structural literal grant declares a nominal/Copy contract and optionally
selected visible fields. It requires exactly one complete representation definition in a
reciprocally seen ordinary source. That definition repeats the nominal/generic/Copy header and
declares every semantic field, with its data type, in representation declaration order.

Those visible field declarations must match the corresponding field identities/types in the complete
definition and retain their relative declaration order. Other fields remain private. A public
field list alone promises neither exhaustive representation nor structural construction. Private
code that can see the complete definition may construct that complete record under the nominal
payload rules. An outside caller needs an explicit [structural literal grant](type-behavior.md#structural-literal-grants)
or a construction function. A `copy struct` contract must be repeated and verified on the complete
representation, including private fields; privacy cannot conceal a non-Copy payload or destructor.

A structural literal grant instead explicitly closes a braced root field schema: those are all
semantic fields, and the root owns the complete schema. No private representation completion is
permitted or required in this case. This avoids repeating a plain public record merely to enable
its synthesized construction. A bare opaque struct cannot acquire a structural grant without a
braced field contract. The explicit grant, never the presence/absence of another file, determines
schema ownership. Neither form fixes physical layout. Adding a hidden semantic field cannot
silently preserve a structural construction promise.

For enums, a braced root owns the complete variant/payload schema; a bare opaque root requires
one complete private definition with the same header under the same reciprocal pairing rule.
There is no private completion of a braced public enum or subset of hidden variants in that
exhaustive contract. [Enum schemas](structs-and-enums.md#public-and-opaque-schemas) own construction,
Copy and use-site exposure. Physical layout/ABI remains separate from field/variant identity.

## Path-Based Visibility

`#visibility` attaches to one eligible following root declaration/member, optionally in a group
with another eligible item directive. It cannot change a continuing default.

| Value | Audience, relative to the declaring module |
| --- | --- |
| Omitted | All packages |
| `"/"` | All modules in this exact package |
| `"./"` | This module and its descendants |
| `"../"`, `"../../"`, ... | The selected ancestor module and its descendants |

Only those complete path forms are valid. Reject named paths, `"package"`, dependency aliases,
filesystem paths and traversal above the package root. The body's location and importing source
do not change the boundary. `"./"` does not mean source-private.

Eligible subjects are a root's named API declarations, visible struct fields, callable/type-behavior
members, structural grants and `use` re-exports. The effective audience of a member is the
intersection of its own boundary and its owner's boundary. A directive cannot widen an owner.
An exported signature must expose only names and contracts accessible throughout that effective
audience, including bounds and origin-bearing data. A re-export's requested effective audience
must be contained in its target's audience; a widening is an error, not silent narrowing.

A `use` selection group has one audience applying to all its selected names; each target must
satisfy it. A namespace alias exports that namespace, not wider access to its members. Direct
module access remains governed by the original declarations, not the re-export's alias.

Do not attach visibility to an outer `impl Type`, `see`, a default implementation fragment,
destructor, implementation body, associated binding or operation mapping. Interface members and
explicit conformance facts have the audience of their owning contract (and, for a conformance,
the interface); they cannot be individually restricted to make one capability mean different
operation sets in different callers. A public owner's conformance must be contracted in the root;
an implementation source cannot add a hidden public conformance. A private nominal may declare
its own private conformances in its ordinary definition/behavior sources.

Consecutive item directives attach to exactly one following eligible item/member; comments and
blank lines are trivia, not a scope reset. Reject duplicate directive names, unsupported subjects
and a group without a following item. A group cannot skip a dependency or declaration to find
a later eligible target. `#target` eligibility/target selection remains a separate platform gate.

## Complete Implementation Signatures

Keep complete signatures in both the root contract and its corresponding implementation. This
future rule is adopted alongside contract-only `index.nct` roots, operation-bearing inputs, and
unified `impl`; it does not change the published syntax. The implementation remains readable
without opening the root merely to recover receiver/parameter names, types, or permissions.

```text
// index.nct; the Buffer contract is assumed to be in scope.
see ./buffer.nct
impl Buffer {
    method edit self.append(value: u8): void
}
```

```text
// buffer.nct
see ./index.nct
impl Buffer {
    method edit self.append(value: u8): void {
        self.storage.append(value)
    }
}
```

The implementation repeats the full receiver, generic/parameter list, result, bounds, applicable
origin clauses, and retained contract modifiers. There is no abbreviated `method append { ... }`
or `func name { ... }` body form. A new private helper likewise declares its own full signature.
Existing interface mappings select their checked helper; they do not create a short body-head
alias or require a new redundant root declaration beyond the interface contract.

The repeated header asserts agreement with one authoritative contract; it does not choose a
second public API. Retain direct contract/definition pairing and canonical header agreement:
select the candidate by module, owner/interface scope, callable kind, and member identity, then
check its complete header against the contract. Parameter/generic names, modes, types, bounds,
result shape, authored origins, and retained modifiers must match in canonical source notation.
Whitespace/comments are not header differences. The root alone owns publication, visibility,
and public declaration documentation. A body cannot strengthen the API from inferred usage.

A signature mismatch is a diagnostic at the differing components with both declaration locations;
it must not become a new private overload or trigger a search for a better-fitting body. Missing
and duplicate definitions remain errors independent of source traversal order. In particular,
`ReadView<File>?` and `ReadView<File?>` cannot match merely because their leaf File type agrees:
the illustrative capability types have different constructor order under the [future type-scope rules](values-and-types.md#type-constructor-scope).

Both physical sources must directly see one another. Pair only within that explicit relation;
transitive visibility, `use`, a coincidentally equal signature or a nearer file cannot supply a
definition. Resolve names in each header's own source context and require both canonical header
agreement and identical resolved contracts. Identically spelled aliases bound to different types
do not agree. A differently spelled equivalent type does not satisfy the selected complete-header
repetition rule either. Visibility and public documentation belong only to the root.

This chooses source readability over eliminating repeated header text. The compiler checks that
repetition; editor navigation may link the two declarations but is not needed to recover an
omitted signature. Constant/static matching uses the same name/type agreement and one-definition
rule; it does not select runtime initialization or CTFE behavior. A private interface fragment may
only complete declared defaults, not add associated types or requirements. Interface conformance
bindings follow [their separate contract](interfaces.md#conformance-and-implementation-bindings),
not function body pairing.

Calls and public navigation use the contract identity; body checking and diagnostics retain the
definition location. References/rename cover both locations as one semantic declaration. A bodyless
opaque result contract retains its declaring callable identity; witness inference belongs to the
owning result contract and cannot be repeated independently in each caller. Full opaque-result,
generic, target and native admission is not established by this pairing rule.
