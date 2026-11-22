# Packages and Package Metadata

## Scope and Retained Data Contract

This chapter selects v2.0.0 package metadata and executable entry behavior. Released directive
notation and data rules apply where listed below.

The following sections of [Packages and Package Source](../../../spec/language/packages.md) are
incorporated for their data rules, without duplicating editable schemas here:

- **Package Root**: package identity is declared by exactly one top-level `#package` in the root
  `index.nct`; documentation and the package directive prefix precede dependencies and declarations;
  nested package roots are distinct from child modules. Its executable source example is not a v2
  source production.
- **Directive Data**: the closed directive names, record data, comma delimiters and rejection of
  unknown directives. Records, strings and integer tokens have the explicit productions below.
- **Dependencies**: aliases, mutually exclusive source forms, source-specific fields, exact-selection
  validation and canonical identities. Acquisition commands are not selected by this language rule.
- **Executable Targets** and **Test Targets**: target records, required/optional fields, independent
  target-name domains and package-relative directory-module selection. Links to released entry/CLI
  behavior do not adopt those behaviors into v2.
- **Implicit Standard-Library Package**: compiler-selected package identity, reserved `std` binding
  and installation validation. This selects neither a v2 prelude export list nor released primitive,
  library, ABI, command or installation implementations.

V2 source names, strings, separators and lexical validation use the
[adopted lexical grammar](lexical-grammar.md). Package metadata is declarative data, not an expression:
no computed string, interpolation, executable initializer or general value constructor is allowed.
Invalid data retains its exact physical source location; parsing a directive does not validate it.

## Source Roles and Execution

The [module contract](modules.md#contract-only-roots) owns root placement and publication. The package
root remains contract-only: metadata does not permit a function body, initializer, destructor or
private helper in `index.nct`. Metadata itself exports no declaration. A target selects a directory
module. Its entry contract is authored in that module's `index.nct` and paired with a complete
implementation in a directly reciprocal ordinary source. The presence of a target does not prove
its entry signature, effects, runtime support or ABI are valid.

An ordinary source, explicit single-file source or child-module root cannot author package
metadata. A descendant root with its own `#package` belongs to another package, not the enclosing
module inventory. The selected package graph applies one explicit source grammar throughout its
roots, dependency packages, module sources and recovery products; it cannot reinterpret a retained
root with another grammar. This consistency requirement does not imply source compatibility between
released and v2 packages.

## Executable Entry

`#executable` selects the top-level function `main` authored in the selected module's
`index.nct`. A helper reached through `see`, an import or re-export, or a `main` in another
module cannot supply this contract. Entry selection takes place in the declaring module, so the
contract may restrict its audience. `main` remains an ordinary callable name.

V2 retains the signature and process result rules from
[Executable Entry](../../../spec/overview.md#executable-entry). The function has no generic or
value parameters. Immediate and async forms admit `void`, `i32`, or `usize`, and any of
these results may end in `!` to permit failure. Completion determines the process status and an
unhandled `error` produces a failure report. The process adapter drives an async entry under the
[v2 future contract](asynchronous-computations.md). The retained rules select neither a CLI
interface nor an installed standard library or platform ABI.

## Grammar

These productions complete the package prefix referenced by
[Files and Declarations](syntactic-grammar.md#files-and-declarations):

```text
V2PackageDirectives = V2PackageDirective (newline+ V2PackageDirective)*
V2PackageDirective = "#" ("package" | "dependencies" | "executable" | "test")
                      ":" V2DirectiveValue
V2DirectiveValue = StringLiteral | integer_literal | "{" List(V2DirectiveField) "}"
V2DirectiveField = Name ":" V2DirectiveValue
```

The prefix precedes `see`, `use` and items. Item directives such as `#visibility` and `#target`
cannot attach to, gate or change package metadata. A late package directive is invalid, including
inside a block. File-role and data-schema checks are independent of syntactic recognition.
