# Standard Library Source Design

This document defines repository policy for implementing the public standard-library contracts in
[`spec/`](../../spec/README.md). Language-level module and style rules remain owned by
[`spec/01-modules-use.md`](../../spec/01-modules-use.md) and
[`spec/16-source-style-formatting.md`](../../spec/16-source-style-formatting.md).

## Module Root Responsibility

Every standard-library `index.nct` is a user contract root, not a general implementation file. A
reader must be able to identify the module's externally visible surface without following
implementation sources or reading package plumbing.

The root owns documentation, required signature imports, re-exports, public data representation,
opaque nominal contracts, bodyless public callable contracts, interface requirements, explicit
default contracts, construction selection, instance-operation contracts, and public conformance
heads with their associated type bindings. Required conformance method signatures remain solely in
their interface.

A package-only subsystem that does not require another module's private representation owns an
explicit module below `std/internal`. No implementation source may add restricted-visible
declarations: module roots remain the sole surface authority. A package-only operation that is
inseparable from a public type's private representation may therefore remain in that public module
root only when moving it would expose the representation or change the public type identity.
`std/mem` raw-buffer transfer and owned-growth operations are the current reviewed exception. Do
not put independent package plumbing in a public root merely to make it discoverable, and do not
hide a cross-module contract inside an implementation body.

Implementation sources own private representation, callable bodies, destruction, allocation and
pointer mechanics, target operations, and helper algorithms. Name each source for one stable
responsibility such as `storage.nct`, `mutation.nct`, `defaults.nct`, or `darwin.nct`; do not use a
generic `impl.nct` when the module has more than one implementation responsibility.

Restricted visibility must be no wider than its actual consumers. A `pub(/)` function in a
standard-library contract source must have a semantic reference from another module in the `std`
package. Helpers used only by implementation sources in their declaring module stay private and
those sources use direct `see` edges. Compiler-owned primitives are exempt because their
declaration itself defines a target or runtime boundary even when current Nocter source has no
caller. The authored-standard-library test enforces this rule from the checked source index rather
than name matching.

The same test freezes the reviewed module-dependency relation from discovery's resolved `use`
edges. Adding an edge requires an ownership review and an explicit update to that relation; source
spelling is never reparsed to infer dependencies.

Every import from one standard-library module to another uses the package-absolute `/module` form.
The dependency spelling `std/module` means “the package bound to the `std` dependency name”; inside
an ordinary checkout analysis that name selects the installed toolchain package, not the authored
package being edited. Package-absolute self imports keep one source tree valid both when selected
as the toolchain standard package and when opened as an ordinary package for diagnostics and LSP
queries. The authored-standard-library test checks this source portability rule independently from
the resolved dependency graph.

Stable error codes are behavioral API; helper functions that construct those errors are not.
Error factories remain private unless callers must select them independently for a documented
reason.

## Current Foundation Ownership

The reconstructed foundation has the following implementation owners:

| Responsibility | Owner |
| --- | --- |
| public address observation and borrow-to-pointer conversion | `std/ptr` |
| raw address projection, byte copying, and typed value movement | `std/internal/ptr` |
| allocation abort boundary | `std/internal/mem` |
| target-neutral OS error facts | `std/internal/os` |
| Darwin syscall, errno, mmap, file-mode, and metadata-layout facts | `std/internal/os/darwin` |
| allocator and raw-buffer policy | `std/mem` |
| borrowed UTF-8 search, ranges, and iterators | `std/str` |
| owned UTF-8 storage, construction, validation, and mutation | `std/string` |
| initialized-prefix ownership and mutation | `std/vec` |

Within `std/mem`, `std/string`, and `std/vec`, representation, construction, mutation, validation,
and destruction live in responsibility-named sources. `std/str/owned.nct` is the explicit edge
from borrowed text algorithms to `String` and `Vec`; the allocation-free search and view sources do
not import owned collection modules. `std/process` reuses `std/string.is_valid_utf8` rather than
owning another validator.

## Source Visibility Graph

The root directly sees every source that completes one of its contracts. Each completing
source directly sees the root. This reciprocal edge is declaration identity, not a visibility
shortcut.

An implementation source explicitly sees every other source whose private declaration or
representation it uses. Direct-only see semantics must remain visible in the authored graph;
an aggregator source must not simulate transitive private visibility.

Private helper-only sources may be reached from their direct implementation consumers without a
root edge. They cannot complete a root contract unless the reciprocal direct edge exists.

## Inline Exceptions

An inline root body is acceptable only when it is representation-independent and its complete
behavior is clearer than a separated declaration. It must contain no control-flow branch, loop,
mutation, allocation, I/O, target operation, or private representation access. Constant results
and direct forwarding are the intended cases.

Interface defaults follow the same rule. Nontrivial defaults use a bodyless `pub default method`
contract in the root and a private `default method` body in `defaults.nct` or another
responsibility-named source.

The current authored standard library intentionally uses no inline root bodies. Its discovery test
rejects every `Block` node in a standard-library module root. Introducing even a trivial exception
therefore requires an explicit policy and test change rather than an unnoticed convenience edit.

A `drop` declaration is not a bodyless API contract: its grammar always carries a body. Put it only
in the responsibility-named private source that owns the type's destruction behavior.

## Review Gate

A standard-library module migration is complete only when:

- its root declares every public contract, every package-only contract has an explicit internal
  owner, and no implementation source adds exported surface;
- ordinary definition navigation selects the root contract and implementation navigation selects
  its body;
- opaque types reveal no private fields through source presentation, hover, or completion;
- every private cross-source dependency has one authored direct see;
- target-independent APIs do not expose platform declarations;
- focused semantic and native tests pass through the same package graph used by users;
- any remaining inline body satisfies the exception above and is called out during review.
