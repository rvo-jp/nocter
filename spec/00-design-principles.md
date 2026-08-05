# Design Principles

This file is part of the Nocter language specification.
The specification entry point is [README.md](README.md).

Nocter is guided by three durable design pillars:

- simplicity
- encapsulation
- foolproof design

These principles are not slogans. They are constraints used to choose between
otherwise plausible language features, standard-library APIs, distribution
models, diagnostics, and tooling behavior.

The motivation is practical. Trying a programming language should not require
installing a pile of unrelated tools. Leaving a language should not require
untangling system-wide package state. Using an API should not require reading
private implementation details. Writing or generating source should not require
choosing between several spellings for the same idea.

When Nocter can solve a problem by making the visible surface smaller, the
contract clearer, or the invalid form impossible to write, it should prefer that
solution.

## Simplicity

Nocter treats simplicity as a product rule, not only as a syntax rule.

The distribution model should be understandable at a glance. A normal user
installs one `.nocter/` directory containing the compiler, metadata, and the
standard library, then exposes the compiler through one symlink in a directory
already on `PATH`. Removing Nocter should mean deleting that symlink and that
directory. This matters because easy removal makes the first trial lower risk.

The compiler should be self-contained for normal use. Nocter should not ask
users to install LLVM, `clang`, `as`, `ld`, a platform SDK, a language-specific
package manager, or an external runtime library just to compile an ordinary
program. Developer builds may depend on Rust and Cargo, but released Nocter
archives should keep the user-facing shape small.

The source language should also keep one canonical way to express a concept.
Nocter should reject duplicate syntax when existing forms already cover the
same meaning. This is why v0.2.0 favors `if` and `match` as value-producing
expressions over separate conditional operators, `use` as the single import
form, and `otherwise` as the single optional fallback form.

Simplicity does not mean weak APIs. It means APIs and syntax should earn their
place by reducing the amount a user, maintainer, or AI assistant must remember.

## Encapsulation

Nocter makes privacy the default. Public API is the exception and must be marked
explicitly with `pub`.

Visible information should be small. A user should be able to understand what a
module, type, function, method, or interface offers by reading its public
contract. They should not need to inspect hidden fields, private helper
functions, allocation strategy, or unrelated implementation files to perform an
ordinary task.

Nocter deliberately avoids class inheritance in v0.2.0. Designs that require users
to reason about parent-class behavior, subclass overrides, protected internals,
or fragile implementation coupling work against Nocter's encapsulation goal.
Nocter should prefer modules, value types, inherent methods, and explicit interfaces.

Interfaces exist to describe public capability. Phase 10 default methods may reuse behavior derived
only from that capability; they cannot expose internal structure or add state. An interface member
must be public, and conformance is explicit through forms such as `impl Printable for User` so
accidental shape matches do not silently become API commitments.

Embedding is Nocter's planned composition-based reuse feature. It is separate
from interfaces: an interface describes capability, while embedding owns a
contained value and promotes only that value's public contract. Embedding must
not give the owner access to the contained value's private implementation, and
the contained value must not gain access back to the owner.

Standard-library and compiler-trusted boundaries must follow the same rule.
Low-level primitives may exist inside the active Nocter home, but ordinary
source should interact with them through public Nocter APIs.

## Foolproof Design

Nocter's foolproof goal is misuse resistance. It should be hard to choose the
wrong operation by accident, and mistakes should fail as early and as locally as
possible.

Foolproof design comes from the first two pillars:

- encapsulation hides states and operations that callers do not need
- simple syntax reduces alternate forms and partial mental models

When a rule can be enforced by the typechecker, resolver, parser, formatter, or
CLI before backend lowering or runtime execution, Nocter should enforce it
there. Unsupported runtime forms should become diagnostics before machine code
is emitted.

The language should prefer contracts that make invalid states unrepresentable or
obvious:

- `let`, `var`, `&T`, and `&+T` expose assignment and borrow capability
- `T!` exposes recoverable failure
- `T?` exposes absence
- `drop` exposes deterministic cleanup
- private-by-default declarations prevent accidental API expansion
- explicit interface conformance prevents accidental public capability

Foolproof does not mean hiding power from advanced users. It means the visible
surface should guide ordinary code toward correct use, and the compiler should
report misuse before it becomes hidden behavior.

## Design Decision Rules

Future Nocter features should answer these questions before adoption:

- Does this add a second spelling for an existing concept?
- Does this force users to inspect private implementation details?
- Does this make ordinary installation or uninstallation more complicated?
- Can this be an ordinary standard-library API instead of syntax?
- Can misuse be diagnosed before backend lowering or runtime execution?
- Does this keep source equally readable for humans and AI assistants?

If a feature fails one of these questions, the default answer is to redesign it,
defer it, or reject it.
