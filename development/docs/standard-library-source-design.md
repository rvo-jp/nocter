# Standard Library Source Design

This document defines repository policy for implementing the public standard-library contracts in
[`spec/`](../../spec/README.md). Language-level module and style rules remain owned by
[`spec/01-modules-use.md`](../../spec/01-modules-use.md) and
[`spec/16-source-style-formatting.md`](../../spec/16-source-style-formatting.md).

## Module Root Responsibility

Every standard-library `index.nct` is a contract root, not a general implementation file. A reader
must be able to identify the module's externally visible and package-visible surface without
following implementation sources.

The root owns documentation, required signature imports, re-exports, public data representation,
opaque nominal contracts, bodyless callable contracts, interface requirements, explicit default
contracts, construction selection, instance-operation contracts, and conformance contracts.

Implementation sources own private representation, callable bodies, destruction, allocation and
pointer mechanics, target operations, and helper algorithms. Name each source for one stable
responsibility such as `storage.nct`, `mutation.nct`, `defaults.nct`, or `darwin.nct`; do not use a
generic `impl.nct` when the module has more than one implementation responsibility.

## Include Graph

The root directly includes every source that completes one of its contracts. Each completing
source directly includes the root. This reciprocal edge is declaration identity, not a visibility
shortcut.

An implementation source explicitly includes every other source whose private declaration or
representation it uses. Direct-only include semantics must remain visible in the authored graph;
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

- its root declares every public and package-visible contract and no implementation source adds
  exported surface;
- ordinary definition navigation selects the root contract and implementation navigation selects
  its body;
- opaque types reveal no private fields through source presentation, hover, or completion;
- every private cross-source dependency has one authored direct include;
- target-independent APIs do not expose platform declarations;
- focused semantic and native tests pass through the same package graph used by users;
- any remaining inline body satisfies the exception above and is called out during review.
