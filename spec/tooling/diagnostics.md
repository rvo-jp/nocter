# Diagnostics

## Diagnostic Policy

Compiler diagnostics are part of the Nocter user experience and must explain source-level concepts clearly.

Diagnostics for type checking, ownership, borrowing, initialization state, visibility, imports, fallible values, optionals, primitive boundaries, and target selection should include:

- what is invalid
- which source construct is invalid
- why it is invalid
- the most useful correction direction when one is known

Diagnostic text must use Nocter source concepts such as `binding`, `borrow`, `move`, `drop`,
`pub(../)`, `pub(/)`, `T!`, `error`, `T?`, `T?!`, `entry function`, `primitive`, and `Nocter home`.
It should not expose backend implementation details such as temporary register allocation, Mach-O
offsets, internal AST node names, or recovery placeholders.

## Format

Errors use stable error codes and source spans.

Minimum source diagnostic shape:

```text
error[E0396]: cannot move `file` while it is borrowed
  --> app.nct:12:18
   |
12 |     close(move file)
   |                ^^^^ move occurs here
   |
note: readonly borrow created here
  --> app.nct:10:16
   |
10 |     inspect(&file)
   |              ^^^^
help: end the borrow before moving `file`
```

Rules:

- Every source-level compiler error should have an error code in the form `E0000`.
- A source-level diagnostic must have one primary span when the compiler can point to source text.
- Related spans should be emitted when they explain the cause, such as the creation of a borrow or the original move.
- `note` explains context or cause.
- `help` gives a correction direction when the compiler can suggest one without guessing intent.
- Command-line, target-selection, filesystem, and Nocter-home discovery errors may omit source spans, but must still name the failing argument, path, target, or environment variable.
- Error codes should not encode compiler phase. The same user-visible mistake should keep the same code even if implementation phases change.
- Error codes become compatibility-sensitive once published in user-facing documentation.

Formatting diagnostic codes:

- `E0601`: formatter cannot safely rewrite a source that contains comments.
- `E0602`: formatter check found a source file that would change.

`E0601` points to the first comment that prevents a lossless rewrite. `E0602` is spanless because
the complete canonical output differs rather than one source range being invalid.

Spanless CLI diagnostic codes:

- `E0700`: command-line syntax or unsupported command.
- `E0701`: target selection failed.
- `E0702`: filesystem path or permission failure.
- `E0703`: Nocter home resolution or validation failed.
- `E0704`: temporary executable preparation or execution handoff failed before user code started.
- `E0800`: package source, package-root, executable declaration, or package target selection failed.
- `E0900`: an internal compiler consistency or lowering failure occurred.

Source-backed lexical diagnostics:

- `E0100`: an unexpected character appears outside the lexical grammar.
- `E0101`: a block comment is not terminated.
- `E0102`: an integer literal has invalid digits or separator placement.
- `E0103`: a floating-point literal has invalid punctuation, digits, separators, exponent, or suffix.
- `E0104`: a string literal is not terminated.
- `E0105`: a single-line string contains a newline.
- `E0106`: multiline string content begins on the opening-delimiter line.
- `E0107`: a string, character, or byte literal contains an invalid escape sequence.
- `E0108`: a string escape does not encode valid UTF-8.
- `E0109`: multiline string indentation is inconsistent with its closing delimiter.
- `E0110`: a byte literal is not terminated.
- `E0111`: a byte literal contains a newline.
- `E0112`: a byte literal does not encode exactly one byte.
- `E0113`: permanently reserved and not assigned to a diagnostic.
- `E0114`: a string interpolation is not terminated.
- `E0115`: a character literal is not terminated.
- `E0116`: a character literal contains a newline.
- `E0117`: a character literal does not decode to exactly one valid Unicode scalar.

Source-backed syntactic diagnostics:

- `E0120`: required syntax is missing at the parser's committed position.
- `E0121`: a top-level `see` or `use` declaration appears after the first item.
- `E0122`: source nesting exceeds the compiler's supported nesting limit.

Lexer and parser errors use the same source-diagnostic envelope as every other source error. Their
primary span identifies the exact malformed or missing source form.

Source-backed module-surface diagnostics:

- `E0230`: a source other than `index.nct` declares non-private visibility.
- `E0231`: a bodyless nominal is used outside an eligible public contract in `index.nct`.
- `E0232`: a bodyless public construction contract member in `index.nct` omits its required explicit
  non-private visibility. Private inline construction members do not require visibility.
- `E0233`: a `#target` gate names a target not recognized by this compiler release. The primary
  span is the gate's string literal.
- `E0234`: an interface contract method in `index.nct` omits bare `pub`. Private implementation
  fragments omit visibility and may only complete declared default methods.
- `E0235`: a primitive type declaration is not the exact declaration selected for one compiler
  built-in type role.

Source-backed namespace diagnostics:

- `E0240`: a declaration name or import alias uses a name reserved for a built-in type.
- `E0241`: declarations or imports introduce the same name more than once in one namespace.
- `E0242`: an authored visibility boundary uses more `../` components than its declaring module
  has ancestors.
- `E0243`: a constant or static declaration name does not use ASCII `UPPER_SNAKE_CASE`.

Source-backed import diagnostics:

- `E0260`: a selected import name does not exist in the target module.
- `E0261`: a re-export visibility boundary is wider than the selected name's visibility boundary.
- `E0262`: source code explicitly imports the compiler-managed standard prelude. The primary span
  is the authored module path; ordinary source modules receive that prelude implicitly.
- `E0263`: an authored `use` path cannot resolve to exactly one directory module within its package
  and dependency boundaries.
- `E0264`: a top-level selected import does not name a type-namespace declaration.
- `E0265`: a namespace alias is attached to a public module re-export.
- `E0412`: a selected import name is outside its declared visibility boundary from the importing
  module.

Source-backed module-topology diagnostics:

- `E0270`: an authored `see` does not name exactly one permitted `.nct` source through its
  required relative path, or a discovery edge does not match that exact same-module source.
- `E0271`: authored module imports form a dependency cycle. The primary span and related notes
  identify one deterministic complete cycle of `use` declarations.
- `E0276`: a package directive appears outside the package root `index.nct`.

Source-backed generic-binder diagnostics:

- `E0280`: an explicit generic binder uses `Self` or a name reserved for a built-in type.
- `E0281`: one explicit generic parameter list declares the same binder more than once. The second
  binder is primary and the first declaration is related.
- `E0282`: a nested declaration introduces an explicit generic binder with the same name as an
  inherited binder. The nested binder is primary and the inherited declaration is related.

Repeated names in a declaration target pattern refer to the first binder and do not constitute
duplicate declarations.

Source-backed declaration-header type diagnostics:

- `E0290`: a name used by a header type, declaration pattern, capability, requirement, or opaque
  result is unknown in that type context.
- `E0291`: a resolved name does not denote a type entity valid in its context.
- `E0292`: a type application supplies the wrong number of generic arguments, supplies arguments
  to `Self` or a generic parameter, or supplies arguments to an associated selection.
- `E0293`: `Self` appears outside a type-owning declaration.
- `E0295`: a structural callable type repeats a named parameter. The later parameter is primary
  and the first parameter is related.
- `E0296`: a structural callable result-provenance clause names no parameter of that callable.
- `E0297`: a structural callable result-provenance clause repeats an origin. The later origin is
  primary and the first origin is related.
- `E0298`: an opaque result binding names no associated type of its selected interface.
- `E0299`: an opaque result repeats an associated-type binding. The later binding is primary and
  the first binding is related.
- `E0300`: an opaque interface application supplies an associated binding in generic angle brackets.
- `E0301`: a parsed generic requirement has a semantically invalid requirement shape.
- `E0302`: a declaration-pattern binder refinement contains the binder that it replaces.
- `E0303`: an interface requirement repeats an associated-type binding. The later binding is
  primary and the first binding is related.
- `E0304`: a declaration repeats a `copy` requirement for the same generic parameter. The later
  parameter is primary and the first parameter is related.
- `E0305`: a declaration repeats the same interface requirement for the same subject and type
  arguments. The later interface name is primary and the first name is related.
- `E0306`: a declaration pattern repeats a binder refinement for the same generic parameter. The
  later refinement is primary and the first refinement is related.

These diagnostics identify the exact name token, argument container, requirement, or duplicate pair
that violates the rule.

Source-backed declaration-header type-normalization diagnostics:

- `E0310`: type aliases form a recursive expansion cycle. The primary alias and ordered related
  aliases form one complete deterministic cycle of declaration-name tokens.
- `E0311`: an associated selection names no associated type available from its base type.
- `E0312`: an associated selection has more than one applicable associated declaration.
- `E0313`: a structural callable can carry result storage but its omitted result provenance has no
  unique inference. The callable type is primary; an explicit `from` clause is required.
- `E0314`: the type after a callable-requirement colon does not normalize to a callable type.

These diagnostics identify the authored type, associated selection, provenance clause, or
requirement that cannot be normalized.

Source-backed declaration-definition diagnostics:

- `E0315`: a callable declaration result-provenance clause names neither its receiver nor one of
  its parameters, and does not name `static`.
- `E0316`: a callable declaration result-provenance clause repeats an origin. The later origin is
  primary and the first is related.
- `E0317`: a bodyless callable has a storage-bearing result and multiple eligible input origins,
  so result provenance cannot be inferred uniquely. The authored result type is primary.
- `E0318`: an interface implementation binding names no associated type declared by its interface.
- `E0319`: an interface implementation repeats an associated-type binding. The later binding name is primary and
  the first is related.
- `E0321`: a constant or static declaration uses a type outside its closed compile-time domain.
- `E0322`: a constant or static initializer expression contains an operation unavailable during
  compile-time evaluation.
- `E0323`: a constant or static initializer expression does not produce the type required by its
  declaration or type context.
- `E0324`: the complete authored constant dependency graph contains a cycle.
- `E0325`: evaluated constant arithmetic overflows, divides by zero, uses an invalid shift count, or
  performs an integer conversion whose result is not representable.
- `E0326`: an argument pack is not the one final parameter of a supported callable, or a sequence
  literal does not declare exactly its one required pack.
- `E0327`: the complete interface-prerequisite graph contains a cycle.
- `E0328`: an interface and its transitive prerequisites expose two different methods or associated
  types with the same effective member name.

`E0322` through `E0325` also apply to a fixed-array length in a body annotation. Every diagnostic in
this family identifies the declaration, initializer, dependency, or length expression that violates
the rule.

Source-backed body-name diagnostics:

- `E0340`: a value-position name is unknown in the current lexical and module lookup context.
- `E0341`: a parameter, local binding, pattern binding, loop binding, region binding, closure
  parameter, catch binding, or block import collides with an enclosing lexical name, an authored
  module name, a built-in type name, or the contextual `Self` type form. Synthetic prelude
  fallback names do not cause this error.
- `E0342`: a block import selects a name that the target module does not export.
- `E0343`: a block import selects an authored name outside its visibility boundary.
- `E0344`: an explicit closure capture does not name a local, parameter, or capture in the
  immediately enclosing callable body.
- `E0345`: a closure capture is repeated, or a closure parameter collides with a capture.
- `E0346`: a closure body uses an enclosing callable binding without listing an explicit capture.
- `E0347`: a member name selected through a module namespace is not exported by that module.
- `E0348`: a member selected through a module namespace is outside the current module's visibility
  boundary.
- `E0349`: a block-scope selected import does not name a type-namespace declaration.

These diagnostics identify the exact declaration, import, capture, or reference token involved in
the name error.

Source-backed interface-implementation diagnostics:

- `E0350`: an interface implementation has no inherent method for a requirement without a default.
- `E0351`: more than one inherent method can satisfy the same interface requirement.
- `E0352`: a same-name inherent method disagrees with the normalized interface method contract.
- `E0353`: two instance/interface patterns can denote the same implementation after binder
  refinement. No more-specific declaration wins.
- `E0354`: a selected associated type does not satisfy an interface or callable capability declared
  by its associated-type declaration.
- `E0355`: two instance target patterns can denote the same type after binder refinement. No
  declaration-order or more-specific rule selects one.
- `E0356`: one instance repeats a borrow-coercion identity with the same receiver capability and
  canonical target type.
- `E0357`: an instance-owned operator, coercion, index, or expansion declaration has a normalized
  signature outside the closed operation grammar.
- `E0358`: an explicit interface implementation does not prove one prerequisite declared by that
  interface for every specialization admitted by the implementation pattern.

Diagnostic signatures use the same substituted interface arguments, `Self` type, associated
bindings, method generics, and binder refinements as ordinary interface checking and dispatch.

Source-backed normalized type-position diagnostics:

- `E0360`: an outcome type repeats an optional or fallible layer, or contains more than two
  outcome layers.
- `E0361`: an optional outcome has `void` as its eventual payload.
- `E0362`: an optional or fallible outcome has `never` as its eventual payload.
- `E0363`: `never` appears in a data-bearing position instead of as a complete callable result.
- `E0364`: `void` appears in a data-bearing position instead of as completion, the direct success
  payload of `void!`, or the pointee of `*void`.
- `E0365`: unsized `str` or `[T]` appears by value rather than behind an indirection.
- `E0366`: a field of a `copy struct` is move-only for every specialization of that declaration.
- `E0367`: a concrete associated projection has no applicable implementation of the associated
  declaration's owner interface.
- `E0368`: a concrete associated projection admits more than one application of the associated
  declaration's owner interface.

These rules apply after alias expansion and concrete generic substitution. A type alias may directly
name `void`, `never`, `str`, or `[T]`; the position where that alias is used determines whether the
expanded type is valid.

Source-backed checked-body diagnostics:

- `E0370`: an expression type is incompatible with its expected destination type.
- `E0371`: using a place would require an implicit ownership move.
- `E0372`: a non-final expression statement produces an unused value.
- `E0373`: a callable can complete without producing its declared result.
- `E0375`: an integer literal is outside the expected integer type's range.
- `E0376`: explicit `move` targets a copyable value.
- `E0377`: explicit `move` targets storage that the place does not own.
- `E0378`: a place may be uninitialized at the selected use.
- `E0379`: a selected field does not exist on the base type.
- `E0380`: a selected field is outside its visibility boundary.
- `E0381`: a field move would partially initialize a struct with its own drop declaration.
- `E0382`: `break` or `continue` has no enclosing loop in the callable body.
- `E0383`: explicit `drop` does not target a move-only owned binding.
- `E0384`: an assignment target is not a writable place.
- `E0385`: a field cannot be reinitialized through an unavailable parent place.
- `E0386`: compound assignment lacks a writable initialized integer target or matching right-hand
  side.
- `E0387`: a readwrite borrow does not target a writable place.
- `E0388`: no unique accessible index operation accepts both the receiver and index type.
- `E0389`: no unique accessible equality or strict-ordering operation accepts both operands.
- `E0390`: a call has no valid callable target, capability, arity, argument, generic substitution,
  and requirement plan.
- `E0391`: a construction expression has no valid accessible structural, variant, or authored
  construction entry and complete type plan.
- `E0392`: an outcome operation is incompatible with its operand or enclosing callable result.
- `E0393`: an enum pattern is incompatible with its subject or payload bindings.
- `E0394`: match arms do not form one complete, unambiguous enum partition.
- `E0395`: a returned storage-bearing value retains an origin outside the callable's effective
  result-provenance contract. Local, owned-parameter, temporary, expired-region, and unknown
  storage cannot escape; an interface implementation method also cannot exceed its interface method's
  external-origin bound.
- `E0396`: a new readonly or readwrite borrow overlaps an incompatible source-level live loan.
- `E0397`: moving, dropping, assigning, or mutating a place conflicts with a source-level live
  loan, including a loan observed by a pending type-owned drop body.
- `E0398`: a storage-bearing value crosses a lexical scope, region, temporary-statement, or
  destination boundary that outlives its storage source.
- `E0399`: a `using` allocation-context place is not an established aborting allocator or
  allocation context.
- `E0400`: a string interpolation expression lacks one exact implementation of the active standard
  `Format` interface.
- `E0401`: an argument-spread source lacks one expansion operation for its copy, borrow, or move
  mode.
- `E0402`: the iterator selected for argument spread does not provide one exact `Iterator` and
  `ExactSizeIterator` contract.
- `E0403`: an argument-spread item is incompatible with its copy, borrow, or move contribution.
- `E0404`: a collection-loop source lacks one acquisition for its explicit `&`, `&+`, `move`, or
  bare direct-iterator form.
- `E0405`: the type acquired by a collection loop does not provide one exact trusted `Iterator`
  contract.
- `E0406`: a type annotation in a body does not resolve to one visible semantic type with complete
  arguments and satisfied requirements.
- `E0407`: a discard binding uses `var` or carries a type annotation instead of the exact
  `let _ = expression` form.
- `E0408`: an opaque callable result has no reachable concrete witness, selects different witness
  types on reachable success paths, selects a type that does not implement the advertised
  interface, or disagrees with an advertised associated-type binding.
- `E0409`: an argument pack is used as an ordinary value or mixed with other pack contributions
  during forwarding. A pack is compiler-owned, non-escaping input and supports `items.len()`,
  consuming `for item in items`, and sole tail forwarding as `target(...items)`.
- `E0410`: a readonly borrow does not target an addressable place. Constants and produced values
  must be used directly.
- `E0411`: a `noalloc` callable can reach an allocation operation, a callable without an
  allocation-free contract or body proof, a primitive whose registry effect is not
  allocation-free, or destruction whose allocation effect cannot be proven absent.
- `E0413`: a tuple projection is not a canonical decimal position, the base is not a tuple, or the
  selected position is outside the tuple's arity.
- `E0414`: a finite floating-point literal rounds to infinity, or a nonzero literal rounds to zero,
  in its selected `f32` or `f64` type.

`E0388`, `E0389`, and `E0390` cover both absence and ambiguity where their operation admits
candidates. None reports a declaration selected only by source order.

Source-backed declaration contract diagnostics:

- `E0250`: a bodyless public callable contract has no implementation body.
- `E0251`: the selected private implementation body does not exactly match its public contract.
- `E0252`: more than one private implementation body matches the same public contract.
- `E0253`: body omission is used outside a public callable contract in `index.nct`, or on a callable
  form that requires an inline body.
- `E0254`: an implementation source introduces an `impl Interface` fact instead of declaring that
  program-wide fact in `index.nct`.
- `E0255`: a bodyless public nominal contract has no complete private representation definition.
- `E0256`: a private nominal representation does not exactly match its public contract's kind,
  name, generic parameters, requirements, or `copy` contract.
- `E0257`: more than one private representation completes the same public nominal contract.
- `E0258`: a represented nominal declaration is completed again.
- `E0259`: an implementation interface fragment supplies a default body without one exact default
  method contract in the reciprocally seen `index.nct` interface.
- `E0272`: a bodyless public constant or static contract has no private initializer definition.
- `E0273`: a private constant or static initializer does not exactly match its public contract.
- `E0274`: more than one private initializer matches the same public constant or static contract.
- `E0275`: a constant or static omits its initializer outside a visible root contract in
  `index.nct`.

Source-backed declaration-header diagnostics:

- `E0200`: an enum declares no variants.
- `E0201`: a construction or instance declaration is outside the target type's ownership boundary.
- `E0202`: one type family has more than one construction declaration.
- `E0203`: a construction member does not produce its owning type, directly or through supported
  outcome layers.
- `E0204`: a drop declaration targets a type that cannot own that declaration.
- `E0205`: one type family has more than one drop declaration.
- `E0206`: a `copy struct` declares a drop body.
- `E0207`: a payloadless enum declares a drop body.
- `E0208`: a primitive function declaration is outside the exact selected standard package.
- `E0209`: a built-in interface implementation is outside the exact selected standard package.
- `E0210`: an interface implementation target is neither a nominal type nor an authorized built-in type.
- `E0211`: an interface implementation does not bind every associated type declared by its interface.
- `E0212`: an opaque result appears on an unsupported callable or a callable without a source body.
- `E0213`: a literal member does not match the language-defined signature for its literal shape.

Each result identifies its primary declaration and, when useful, one related declaration. Every
output format reports the same locations.

## Source and Span Contract

Source positions in compiler output are byte-based. Each position refers to exactly one loaded
source through its display path and canonical absolute path when known.

Rules:

- Start and end offsets are UTF-8 byte offsets in normalized source text.
- End offsets are exclusive.
- The compiler normalizes CRLF to LF before computing spans.
- Bare carriage return is a source error.
- Line and column pairs are derived from those byte offsets for human-readable diagnostics, JSON
  output, editor protocols, and source-inspection tools.

Public JSON span shape:

```json
{
  "file": "app.nct",
  "absolute_path": "/Users/me/project/app.nct",
  "start_byte": 120,
  "end_byte": 124,
  "start_line": 12,
  "start_column_byte": 18,
  "end_line": 12,
  "end_column_byte": 22
}
```

JSON span rules:

- `file` is the diagnostic display path.
- `absolute_path` is the canonical absolute path used for editor integration and source-file identity, or `null` when unknown.
- `start_byte` and `end_byte` are UTF-8 byte offsets after line-ending normalization.
- `end_byte` is exclusive.
- `start_line` and `end_line` are 1-based line numbers after line-ending normalization.
- `start_column_byte` and `end_column_byte` are 1-based UTF-8 byte columns within the normalized line.
- These JSON line and column fields are not LSP positions.
- LSP adapters convert byte offsets or byte columns into the client position encoding.
- Display path rules and canonical source-file identity are specified in
  [Modules, Use Declarations, and Source Visibility](../language/modules.md#source-and-module-identity).

## Machine-Readable JSON Diagnostics

`nocter check --format json` writes exactly one JSON object to stdout.

Top-level envelope:

```json
{
  "schema": "nocter.diagnostics",
  "version": 1,
  "ok": false,
  "command": "check",
  "target": "arm64-darwin",
  "root": "/Users/me/project/index.nct",
  "root_absolute_path": "/Users/me/project/index.nct",
  "diagnostics": []
}
```

Envelope rules:

- The top-level JSON value is always one object.
- The output is not newline-delimited JSON.
- The output is not a top-level array.
- `schema` is always `"nocter.diagnostics"` for this envelope.
- `version` is the integer schema version. The initial version is `1`.
- `ok` is `true` only when `diagnostics` is empty.
- `ok` is `false` when at least one diagnostic is present.
- `command` is the command that produced the diagnostics, initially `"check"`.
- `target` is the active target when known, or `null` if target selection did not complete.
- `root` is the selected package `index.nct` or explicit file display path when known, or `null` if
  input selection did not complete.
- `root_absolute_path` is the canonical absolute path of the root file when known, or `null` if root-file discovery did not complete.
- `diagnostics` is an array of diagnostic objects.
- Human-readable progress text, logs, or diagnostics must not be mixed into stdout.

Diagnostic object:

```json
{
  "code": "E0396",
  "severity": "error",
  "message": "cannot move `file` while it is borrowed",
  "primary_span": {
    "file": "app.nct",
    "absolute_path": "/Users/me/project/app.nct",
    "start_byte": 120,
    "end_byte": 124,
    "start_line": 12,
    "start_column_byte": 18,
    "end_line": 12,
    "end_column_byte": 22
  },
  "notes": [
    {
      "message": "readonly borrow created here",
      "span": {
        "file": "app.nct",
        "absolute_path": "/Users/me/project/app.nct",
        "start_byte": 84,
        "end_byte": 88,
        "start_line": 10,
        "start_column_byte": 16,
        "end_line": 10,
        "end_column_byte": 20
      }
    }
  ],
  "help": "end the borrow before moving `file`"
}
```

Diagnostic object rules:

- `code` uses the `E0000` form for source diagnostics, command-line errors, filesystem errors, Nocter-home errors, target-selection errors, and internal compiler errors that are reported through this JSON format.
- `severity` is currently `"error"`.
- Future schema versions may add `"warning"` and `"info"`.
- `message` is the primary human-readable diagnostic message.
- `primary_span` is a span object when a source location is known.
- `primary_span` is `null` for diagnostics without a useful source span, such as command-line, filesystem, Nocter-home, or target-selection errors.
- `notes` is an array. It is empty when there are no related notes.
- Each note has a `message` and a `span`.
- A note `span` may be `null` when the note is not tied to a source location.
- `help` is a string when the compiler has a concrete correction direction.
- `help` is `null` when no useful correction direction is known.

Span rules:

- Diagnostic JSON uses the public JSON span shape defined in [Source And Span Model](#source-and-span-model).
- `primary_span` and note `span` may be `null`.
- Non-null spans must include byte offsets and byte-column positions.

Example for a command-line or filesystem diagnostic:

```json
{
  "schema": "nocter.diagnostics",
  "version": 1,
  "ok": false,
  "command": "check",
  "target": null,
  "root": "missing.nct",
  "root_absolute_path": null,
  "diagnostics": [
    {
      "code": "E0702",
      "severity": "error",
      "message": "root file `missing.nct` was not found",
      "primary_span": null,
      "notes": [],
      "help": "check the path passed to `nocter check`"
    }
  ]
}
```

## Error Recovery

Parser recovery may be conservative; semantic diagnostics should avoid cascades.

Rules:

- Analysis may stop after the first unrecoverable syntax error.
- Once the source is syntactically complete enough to analyze, multiple independent errors may be
  reported.
- A statement after proven terminal control flow still reports independent name, visibility, type,
  call-contract, and structural-place errors. It does not report diagnostics that require a
  fictional post-terminal initialization, move, loan, or provenance state.
- Cascaded errors should be suppressed when they are caused by an earlier error.
- Diagnostics must not mention internal recovery placeholders.
- If too many independent errors are found, the compiler may stop after a limit and report that
  additional errors were suppressed.
- A diagnostic should prefer the earliest source location that caused the invalid program.

## Diagnostic Specificity

The owning [language](../language/README.md), [platform](../platform/README.md), and
[tooling](README.md) chapters define which programs and operations are invalid. The code catalog
above is the sole assignment of numeric diagnostic identities; this chapter does not restate
language validity as a second error inventory.

When one code covers several related invalid forms, its message must identify the exact authored
operation and explain the violated source-level rule. In particular:

- name, import, visibility, and declaration diagnostics identify the authored name or path and any
  conflicting or inaccessible declaration;
- ownership and borrow diagnostics identify the affected source place and distinguish missing
  ownership from missing write permission;
- compound-assignment diagnostics describe the authored compound operation rather than a fictional
  desugared assignment;
- closure-capture diagnostics identify the authored capture and enclosing binding rather than an
  anonymous environment field;
- optional and fallible diagnostics name the immediate outcome layer and recommend only a canonical
  propagation or recovery form valid for that layer;
- generic-inference diagnostics name the unresolved parameter and the missing inference source
  instead of selecting a default type;
- primitive, target, package, and Nocter-home diagnostics identify the exact selected boundary that
  failed validation.

A correction may be described in `help` only when it follows from the failed rule without guessing
the programmer's intent. Editor code actions have the additional validation requirements defined by
[Tooling and Editor Integration](editor.md#diagnostics-code-actions-and-hints).

## Examples

Visibility:

```text
error[E0412]: import `std/internal/ptr` cannot access `pub(/)` name `from_addr`
  --> app.nct:3:31
   |
3 | use std/internal/ptr
   |                               ^^^^^^^^^
   |
note: `from_addr` is declared as `pub(/)` in the implicit `std` package
help: use a public safe API instead
```

Fallible propagation outside a fallible function:

```text
error[E0392]: postfix `?` would fail with `error`, but function `load` is not fallible
  --> app.nct:8:16
   |
8 |     let file = File.open(path)?
   |                              ^
   |
note: current function returns `String`
help: change the return type to `String!` or handle the failure with `catch`
```

Maybe initialized binding:

```text
error[E0378]: `file` may be uninitialized on this path
  --> app.nct:11:9
   |
11 |     file.read()?
   |     ^^^^
   |
note: `file` is moved on one branch above
help: reinitialize `file` on every path before using it
```

## Warnings

Warnings are not part of the current diagnostic surface.

Future warnings may be added for style, portability, or likely mistakes, but warnings must not change language semantics. A warning must not be required for memory safety or type soundness.
