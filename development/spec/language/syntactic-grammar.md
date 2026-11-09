# Syntactic Grammar


## Contextual Spellings

These additional identifier-shaped spellings have the adopted v2.0.0 roles below. Other adopted
positions are defined by this chapter's productions. This table does not claim a complete item
or module grammar; unspecified productions remain outside the development specification.

| Spelling | Contextual grammar position |
| --- | --- |
| `read` | scoped read binding only |
| `edit` | scoped update binding, input operation, enum branch or for operation |
| `take` | value-receiving input, enum branch or for operation |
| `mut` | updating invocation head of a callable type |
| `once` | consuming invocation head of a callable type |

## Files and Declarations

These productions own the adopted file/declaration envelopes for
[modules](modules.md), [type behavior](type-behavior.md) and [interfaces](interfaces.md).
File role is checked after recognition: a root cannot contain a body/initializer just because
the shared production can recognize one. An ordinary interface fragment completing defaults
and a root interface contract have the same syntactic head and one explicitly paired identity.
Similarly, a public struct contract and its private representation are not two nominal types.

```text
V2SourceFile = newline* (V2PackageDirectives newline+)?
               (V2Dependencies (newline+ V2Items)? | V2Items)? newline* eof
V2Dependencies = V2Dependency (newline+ V2Dependency)*
V2Dependency = "see" V2SeePath | V2ItemDirectives? "use" V2UseTree
V2SeePath = ("./" | "../"+) ModuleSegment ("/" ModuleSegment)* ".nct"
V2UseTree = V2ModulePath ("as" Name)? | "/" "as" Name
          | (V2ModulePath | "/") "." V2ImportSelection
V2ImportSelection = Name ("as" Name)? | "{" NonEmptyList(V2SelectedName) "}"
V2SelectedName = Name ("as" Name)?
V2ModulePath = ModuleSegment ("/" ModuleSegment)*
             | "/" ModuleSegment ("/" ModuleSegment)*
             | ("./" | "../"+) ModuleSegment ("/" ModuleSegment)*
V2ItemDirectives = V2ItemDirective (newline+ V2ItemDirective)* newline+
V2ItemDirective = "#" ("visibility" | "target") ":" StringLiteral

V2Items = LineSequence(V2Item)
V2Item = V2ItemDirectives? (V2Function | V2Primitive | V2Constant | V2Static
         | V2Alias | V2Struct | V2Enum | V2Interface | V2TypeBehavior | V2Test)
V2Function = V2FunctionHead V2GenericParameters? V2Parameters V2CallableTail V2Block?
V2Primitive = "primitive" "func" Name V2GenericParameters? V2Parameters V2CallableTail
            | "primitive" "type" (Name | "void" | "never")
V2Constant = "const" Name ":" V2Type ("=" V2Expression)?
V2Static = "static" Name ":" V2Type ("=" V2Expression)?
V2Alias = "type" Name V2GenericParameters? "=" V2Type V2WhereClause?
V2Struct = "copy"? "struct" Name V2GenericParameters? V2WhereClause?
           ("{" LineSequence(V2Field) "}")?
V2Field = V2ItemDirectives? Name ":" V2Type
V2Enum = "copy"? "enum" Name V2GenericParameters? V2WhereClause?
         ("{" LineSequence(V2Variant) "}")?
V2Variant = Name ("(" NonEmptyList(V2PayloadField) ")")?
V2PayloadField = Name ":" V2Type
V2Interface = "interface" Name V2GenericParameters? V2WhereClause?
              "{" LineSequence(V2InterfaceMember) "}"
V2InterfaceMember = "type" Name V2InterfaceBounds?
                  | V2InterfaceCallable | V2Comparison | V2IndexProjection
                  | V2CurrentProjection | V2Expansion
V2InterfaceBounds = "impl" InterfaceApplication ("+" InterfaceApplication)*
V2InterfaceCallable = "default"? (V2FunctionHead | V2MethodHead)
                      V2GenericParameters? V2Parameters V2CallableTail V2Block?
V2TypeBehavior = "impl" V2DeclarationPattern V2WhereClause?
                 "{" LineSequence(V2BehaviorMember) "}"
V2BehaviorMember = V2ItemDirectives? (V2Function | V2Method | V2Literal
                   | V2StructuralGrant | V2Conformance | V2Comparison
                   | V2IndexProjection | V2CurrentProjection | V2Coercion | V2Expansion | V2Destructor)
V2Method = V2MethodHead V2GenericParameters? V2Parameters V2CallableTail V2Block?
V2StructuralGrant = "literal" "{" List(Name) "}"
V2Literal = "const"? "literal" ("[" "]" | "[" ":" "]" | EmptyStringLiteral)
            V2Parameters V2CallableTail V2Block?
V2Coercion = "const"? "coerce" V2Receiver "as" V2Type V2OriginClause? V2Block?
V2Conformance = "impl" V2NamedType ("{" NonEmptyList(V2ConformanceBinding) "}")?
V2ConformanceBinding = "." Name "=" V2Type | V2OperationMapping
V2OperationMapping = Name "=" Name
V2Destructor = "drop" V2Block
V2Test = "test" Name V2Block
V2DeclarationPattern = Name ("<" NonEmptyList(Name) ">")?
                       | BuiltinScalarType | "str" | "error" | "[" Name "]"
```

`"./"`, `"../"` and `".nct"` denote adjacent path-token sequences, not new compound tokens.
Comments/blank lines are trivia for directive attachment. Visibility eligibility follows the
module chapter, not merely the presence of an optional prefix in these productions. An outer
`impl` cannot establish a visibility mode for its members. An inner conformance has one optional
binding block: `.Item = T` and `read = read_bytes` are syntactically distinct entries without
type lookup. Do not consume a type-binding block as part of the interface name and then require
another block for mappings. Empty binding blocks and duplicate bindings are invalid.

These envelopes deliberately depend on the separately owned value/access productions below,
lexical names/lists/trivia, and the [whole-source draft](../../design/source-grammar.md)
for still-unclosed package metadata, special operation and expression
productions. Recognizing an envelope does not migrate those dependencies into adopted semantics.
Typed sequence/string entries and explicit conversions have their
[construction](literal-construction.md) and [conversion](explicit-conversions.md) owners.
Recognition does not establish source/evaluator implementation. Generic solving, primitives,
target selection and package discovery still need qualification before compiler admission.
A complete whole-language grammar is not claimed here; published productions are never an
implicit fallback.

### Typed Literal and Conversion Expressions

```text
V2TypedLiteral = V2Owner gap (V2SequenceBody | StringLiteral)
V2Owner = V2NamedType | BuiltinScalarType | "str" | "error"
V2SequenceBody = "[" (":" List(V2SequenceSpread) | List(V2SequenceElement)) "]"
V2SequenceElement = V2Expression | V2SequenceSpread | V2Expression ":" V2Expression
V2SequenceSpread = "..." V2Expression
V2Convert = V2Unary ("as" V2Type)*
```

V2Primary admits V2TypedLiteral; its remaining alternatives are in the whole-source draft.
Here gap means non-joint owner/delimiter tokens on the same logical line, separated by space,
tab or a removed same-line comment; an intervening statement newline does not continue a literal.
It distinguishes a typed literal from joint indexing. The sequence node
retains ordinary, keyed and expanded contributions without type lookup. Empty `[]`, `[:]`,
direct keyed contributions, mixed entries and spread-only bodies select or reject shapes under
the construction owner, not a semantic reparse. A StringLiteral here has no interpolation.
The leading colon extends empty keyed syntax to spread-only keyed bodies; it is not an optional
prefix for direct keyed contributions.
Conversion is an explicit, left-associative expression level; a conversion target's outcome
suffix belongs to its type. Parenthesize the conversion before propagating its result.
`using` allocation overrides remain outside adopted literal syntax.

## Access Syntax

These productions define the adopted value/input/callable/alias source boundary. They are separate
from the released grammar. The file/declaration envelopes above extend their source design;
complete literal/operator, platform and standard-library migrations are not
implied by these productions. The implementation
design records their remaining entry gates; no old grammar is used as a fallback.

```text
V2InputOperation = "edit" | "take"
V2BindingOperation = "let" | "var" | "read" | "edit"
V2InvocationKind = "func" | "mut" | "once"

V2Type = V2TypeHead V2OutcomeSuffix?
V2OutcomeSuffix = "?" "?"* "!"? | "!"
V2TypeHead = "*" V2TypeHead | "future" V2Type | V2TypeAtom
V2TypeAtom = BuiltinScalarType | "str" | "error" | "void" | "never"
           | V2NamedType | "[" V2Type "]" | "[" V2Type ";" V2Expression "]"
           | "(" V2Type ")" | "(" V2Type "," V2Type ("," V2Type)* ","? ")"
           | V2CallableType | "any" V2CallableType
V2NamedType = (Name V2GenericArguments? | "Self") V2TypeMember*
V2TypeMember = "." Name V2GenericArguments? | V2InterfaceSelection
V2InterfaceSelection = "." "(" V2NamedType "." Name ")"
V2GenericArguments = "<" NonEmptyList(V2GenericArgument) ">"
V2GenericArgument = V2Type | StructuralConstantExpression

V2OriginClause = "from" V2Origin ("|" V2Origin)*
V2Origin = Name | "self" | "static"

V2Parameter = V2OrdinaryParameter | V2PackParameter | V2KeyedParameter
V2OrdinaryParameter = V2InputOperation? V2Pattern ":" V2Type V2OriginClause?
V2PackParameter = "..." V2InputOperation? Name ":" V2Type V2OriginClause?
V2KeyedParameter = "..." V2InputOperation? Name ":" V2Type ":" V2Type V2OriginClause?
V2Parameters = "(" List(V2Parameter) ")"
V2CallableType = "const"? V2CallableKind "(" List(V2CallableParameter) ")"
                 ":" V2Type V2OriginClause?
V2CallableKind = "func" | "once" | "mut" V2OriginClause?
V2CallableParameter = V2KeyedParameter | V2CallablePackParameter | V2OrdinaryCallableParameter
V2OrdinaryCallableParameter = V2InputOperation? (V2Pattern ":" V2Type | V2Type) V2OriginClause?
V2CallablePackParameter = "..." V2InputOperation? (Name ":" V2Type | V2Type) V2OriginClause?

V2FunctionHead = "const"? "async"? "func" Name
V2MethodHead = "const"? "async"? "method" V2Receiver "." Name
V2Receiver = V2InputOperation? "self"
           | "(" "edit" "self" V2OriginClause ")"
V2CallableTail = ":" (V2Type | "some" InterfaceApplication V2OutcomeSuffix?)
                  V2OriginClause? V2WhereClause?

V2Pattern = Name | "_" | "(" V2Pattern "," V2Pattern ("," V2Pattern)* ","? ")"
V2Binding = V2BindingOperation V2Pattern (":" V2Type)? "=" V2Expression
V2Return = "return" V2Expression?
V2Throw = "throw" V2Expression
V2Drop = "drop" Name
V2IndexProjection = "const"? "operator" "(" "edit"? "self" "[" V2OrdinaryParameter "]" ")"
                    ":" V2Type V2OriginClause? V2WhereClause? V2Block?
V2CurrentProjection = "const"? "operator" "(" "edit"? "self" "[" "]" ")"
                      ":" V2Type V2OriginClause? V2WhereClause? V2Block?

V2Closure = "(" V2ClosureHead ")" (":" V2Type V2OriginClause?)? V2Block
V2ClosureHead = (V2ValueCaptures ";" V2ClosureParameters? | V2ClosureParameters)?
V2ValueCaptures = NonEmptyList(V2ValueCapture)
V2ValueCapture = Name V2TransferSuffix?
V2ClosureParameters = NonEmptyList(V2ClosureParameter)
V2ClosureParameter = V2InputOperation? V2Pattern (":" V2Type)? | V2ClosurePackParameter
V2ClosurePackParameter = "..." V2InputOperation? Name (":" V2Type (":" V2Type)?)?
```

`BuiltinScalarType`, `Name`, `List`, `NonEmptyList`, `InterfaceApplication` and the constant-expression
subset denote their lexical/structural responsibilities, with recursively used types replaced by
V2 types. `V2WhereClause` defined here retains the selected generic requirement families and excludes
retired authored effect guarantees. Their special-operation subjects still need their owning
semantic contracts. These are intentional named dependencies, not a claim that the fragment is a closed
standalone parser grammar. Named function/method heads take the full generic list, parameter list
and callable tail before an optional body under the owning module rules.

No ordinary type or expression has an `&`/`^` access prefix. No result marker occurs before a
name. Parameters use `edit value: T` or `take value: T`; anonymous callable inputs use `edit T`
or `take T`. Recognize a contextual input-operation word only when a following pattern/type
completes that production; an identifier such as `edit: T` is an ordinary unmarked name.
Variadic `...` precedes the input operation and remains limited to an eligible final parameter.
Pattern recognition does not prove arity, Copy, extraction rights or drop validity.
A keyed pack always names its binding, including in a callable type:
`func(... entries: K: V): R` or `func(... take entries: K: V): R`.
`func(... K: V): R` has only the ordinary named-pack interpretation (name K, element type V),
never a key/value interpretation selected by type lookup. Keyed pack names bind semantic input
positions just as other named callable inputs do; the names are not part of type identity.
Pack parameters require one name rather than a per-entry tuple pattern or discard. An ordinary
non-variadic tuple parameter retains its pattern form. Closure packs permit omitted types under
the selected expected contract; a keyed annotation writes both types. Index projections have
one ordinary index input, without a variadic/keyed index header.
The [variadic rules](variadic-inputs.md) own pack body operations, preparation and cleanup;
their compiler/body/ABI qualification remains separate from recognition.

Origin attachment is syntactic. A from following an ungrouped callable type belongs to the
nearest still-open callable result, not an enclosing parameter/result selected by name lookup.
To attach an enclosing clause, delimit that complete callable-containing type first, for example
`edit action: (mut(): void) from buffer` or `: (func(): View) from buffer`. This also applies when
future wraps the trailing callable type. A second consecutive from cannot bypass the required
grouping to spell the enclosing relation. Origins inside the type resolve in their own callable
header; an unknown name is an error, never a reason to reinterpret the clause as outer state.

### Generic Arguments and Requirements

[Generic arguments and constants](generics-and-constants.md) own the following requirement
roles and their inference/staging constraints. [Opaque results](opaque-results.md) own the
`some InterfaceApplication` result branch above. Interface applications can bind associated
types but do not introduce dynamic interface objects.

```text
V2GenericParameters = "<" NonEmptyList(V2GenericParameter) ">"
V2GenericParameter = Name | "const" Name ":" V2Type
InterfaceApplication = V2NamedType V2AssociatedBindings?
V2AssociatedBindings = "{" NonEmptyList(V2AssociatedBinding) "}"
V2AssociatedBinding = "." Name "=" V2Type
V2WhereClause = "where" V2Predicate ("," V2Predicate)*
V2Predicate = V2RequirementSubject "impl" InterfaceApplication
             | Name ":" V2Type | "copy" V2RequirementSubject | V2Type "=" V2Type
             | "(" V2Type ("==" | "<") V2Type ")" ":" V2Type
             | "(" "edit"? V2Type "[" V2Type? "]" ")" ":" V2Type
             | V2InputOperation? V2Type "as" V2Type
             | "(" "..." V2InputOperation? V2RequirementSubject ")" ":" V2Type
V2RequirementSubject = (Name | "Self") V2TypeMember*
StructuralConstantExpression = V2Expression
```

An argument belonging to both the type and expression grammar retains one neutral source
structure. The selected declaration's ordered slot kind determines its one elaboration; parsing
does not inspect the argument name's type/value binding. The union production above does not
authorize two competing semantic parses or a second source parse. Requirements using special
operations still need those operations' owning semantics; recognition alone supplies no evidence.

An explicit call's `<` is joint with its named/member callee; a complete argument list must be
followed by its call opener on the same logical line. An explicit owner list instead precedes
its member, record or spaced typed-literal delimiter. A recognized generic form that fails
checking is not reparsed as comparisons. Generic closers delimit ungrouped constant arguments;
put comparisons or shifts involving `>` inside parentheses, such as `Fixed<u8, (WIDTH >> 1)>`.
Nested generic `>>` closes two lists in that delimiter context. This distinction uses syntactic
depth/jointness, not name lookup. Complete constant-expression evaluation is a checking contract,
not a different call-free parser language for array lengths.

In type position `T.(Source.Item)` selects an associated declaration only for an actual collision.
Its final outer `.Name` is the selected member; the preceding nested path/type arguments name
the interface. Unique `T.Item` needs no selector, and redundant qualification is rejected. The
same `V2InterfaceSelection` shape selects an ambiguous interface operation in expression position.
Associated bindings use the named interface's declarations; conformance blocks continue using
their own single mixed-binding production rather than this requirement block.

### Patterns and Enum Branches

[Patterns](patterns.md) own tuple binding, branch input operations and cleanup;
[enum schemas](structs-and-enums.md#enum-schemas-and-construction) own nominal variants and
visibility. Local/parameter patterns continue using V2Pattern above; refutable enum patterns
occur only in these branch forms:

```text
V2If = "if" V2IfCondition V2Block ("else" (V2Block | V2If))?
V2IfCondition = V2PatternInput "is" V2EnumPattern | V2Header
V2PatternInput = V2InputOperation gap V2Header | V2Header
V2Match = "match" V2PatternInput "{" LineSequence(V2MatchArm) "}"
V2MatchArm = (V2EnumPattern | "_") V2Block
V2EnumPattern = V2NamedType "." Name ("(" NonEmptyList(V2Pattern) ")")?
```

Require at least one match arm and valid structural coverage. The optional input operation is
edit/take, with unmarked read; there is no explicit-read alternate. A qualified enum pattern's
last outer `.Name` is its variant, not another part of its owner path. Exact identity, payload
arity and tuple shape are checked after recognition without type-name-driven reparsing.

The operation prefix requires separating gap on the logical header line and a complete operand.
Joint `take(value)` or `edit[index]` continues an ordinary identifier expression. Where both a
completed prefix and an identifier-expression reading exist, select the prefix syntactically;
group the identifier expression to request that reading, for example `match (take - offset)`.
In if, the prefix role requires the completed enum `is` condition; an ordinary Boolean condition
has no such mode. A bad source type/authority cannot trigger another parse. These contextual
boundaries need parser/recovery fixtures before compiler admission.

Tuple patterns have at least two leaves at each tuple node; unit remains `()` as a value. Enum
payload lists are nonempty when parentheses occur. A payloadless variant omits parentheses in
declaration, construction and patterns. Pattern parentheses add no per-leaf operation aliases,
rest syntax, implicit payload omission or nested enum-pattern family.

### Fixed Aggregate Construction

[Tuples and fixed arrays](tuples-and-arrays.md) own these value constructors and their
component selection, Copy and partial-initialization rules:

```text
V2TupleValue = "(" V2Expression "," V2Expression ("," V2Expression)* ","? ")"
V2Array = "[" (V2Expression ";" V2Expression | List(V2Expression)) "]"
```

`()` remains void; `(expression)` remains grouping. A tuple needs two or more components;
an array list can be empty or have one element. A repetition count is an expression recognized
without a type-name lookup and then checked as required compile-time usize data. Its semicolon
is inside brackets, not a general statement/result separator. Fixed arrays have no spread or
keyed entries. The typed sequence/string literal family remains in the assembly draft.

Primitive tuple/array selection uses the common postfix productions. Their shared recognition
does not make built-in owning subplaces and user-declared index projections the same operation.
The checker records the selected category and supply operation once, including computed-value
selection, without reparsing after an authority or initialization failure.

### Argument Expansion

[Variadic inputs](variadic-inputs.md#calls-and-built-in-expansion) own these contribution roles:

```text
V2CallArgument = V2Expression | "..." V2Expression | V2Expression ":" V2Expression
```

The selected formal header determines the fixed-prefix/ordinary-tail/keyed-tail roles; syntax
never searches another target or treats a key as a parameter-name label. Spread contributes to
the final pack only. It is not a standalone expression, a fixed-prefix arity splice or an
invocation of an undeclared iterator/user expansion operator. Built-in pack/fixed-array carriers
and receiving component preparation are resolved once after recognition. General synchronous
expansion is owned below; typed literal contributions remain a separate draft subject.

### Postfix Transfer Sources

[Value supply](value-access.md#explicit-transfer-sources) owns source retirement and commit:

```text
V2Postfix = V2Primary V2PostfixPart*
V2PostfixPart = "(" List(V2CallArgument) ")" | "." Name | "." integer_literal
                | "[" V2Expression? "]" | V2InterfaceSelection | V2TransferSuffix
                | V2GenericArguments "(" List(V2CallArgument) ")"
V2TransferSuffix = "^"
```

The suffix is joint with its immediately preceding place/sequence source. Postfix parts bind
left to right: `buffer^.finish()` retains a marked direct receiver; `record.field^` marks the
field; `record^.field` acquires the whole owner before computed field selection. Parentheses
preserve a marked direct source. A source requiring ^ cannot also move without it; fresh values
cannot take a redundant suffix. There is no prefix/type/binding/result-name form or overloaded
caret. Binary caret is not selected by this grammar and cannot be recovered through whitespace,
type lookup or failed transfer checking.

`value^?` / `value^!` first supplies the marked outcome carrier; the existing extraction layer
still admits one suffix. Closure value-capture entries are names with an optional joint ^,
where an unmarked entry copies only with evidence and a marked entry transfers even Copy data.
Pack^ is restricted to the sequence-source/capture/expansion roles; it is not ordinary data.
Missing ownership, repeated transfer or a suffix on a fresh/computed result is a semantic
diagnostic, never another parse. Exact jointness/recovery cases require parser qualification.

### Synchronous Iteration and Expansion Operators

[Iteration](iteration.md) owns loop acquisition, step/binding and exit scheduling. The
[traversal interfaces](../library/iteration.md) own exact library declaration roles:

```text
V2For = "for" V2InputOperation? V2Pattern "in" V2Header V2Block
V2Expansion = "const"? "operator" "(" "..." V2Receiver ")"
              ":" V2Type V2OriginClause? V2WhereClause? V2Block?
```

Read is unmarked; edit/take choose one fixed traversal/source family before checking authority.
Recognize an operation prefix only when its following complete pattern and in delimiter fit;
`for take in source` instead binds an unmarked name take. There is one common tuple pattern,
not a separate colon-key binder or range-only source syntax. A source header is an ordinary
expression stopped at its outer block delimiter, without type-name reparsing.

Read/edit current projections use the zero-input V2CurrentProjection family above. A joint
postfix `[]` requests that place; `[()]` retains a different one-input index identity. The
generic projection requirement above admits both families by whether an index type occurs.
An expansion result is ordinary data with the required cursor conformance, not a new opaque
result declaration, implicit future, failure layer or coroutine. Bodyless required interface
expansion operations bind an associated cursor type under ordinary conformance rules.

For-await remains a separate draft production, without automatic async/fallible protocol choice.
These recognition rules do not implement parser, effect/root, cursor, executable or target paths.

### Outcome Elimination and Recovery

[Outcomes](outcomes.md) own contextual construction, exact forwarding, failure exit, propagation
and scoped payload projection. The source envelope is:

```text
V2Expression = V2Or V2Recovery*
V2Recovery = "catch" (Name | "_") V2Block | "otherwise" V2Block
V2Unary = ("!" | "-") V2Unary | V2Await ("?" | "!")?
V2Await = "await" V2Await | V2Postfix
```

Remaining binary/postfix/block productions stay in the whole-source assembly until their
semantic families migrate. At an existing-place access boundary, the same outcome syntax can
select a payload projection; it is not another syntax form or a type qualifier. The checked
operation distinguishes value extraction from read/edit access and records tag, cleanup and
propagation obligations. A catch binding inherits that selected operation on the failure payload.
An ordinary catch owns its received error; a projected catch aliases the retained error.
`some(...)` remains an ordinary call to a resolved declaration. No Some/None or success/failure
match pattern, exception handler block or adjacent double extraction is introduced here.

### Asynchronous Staging

The async positions in function/method/interface heads and the future/await productions follow
[asynchronous computations](asynchronous-computations.md). The head's declared result describes
the driven output, not an implicitly fallible constructor. `await operation()?` extracts the
output outcome after awaiting; `await (make_future()?)` instead extracts an immediate outcome
before awaiting its future. Constructor order/grouping is not inferred from the type name.

Shared heads may recognize `const async`, but that staging combination is invalid. There is no
async prefix on an anonymous closure or structural callable type, no input-mode assertion,
and no authored effect qualifier to complete the syntax. An ordinary closure/function returning
future T remains immediate. `for await` in the remaining draft needs its own library iteration
contract; the producer/await rules do not silently admit it or define an executor entry.

An interface default prefixes the same ordinary callable head, for example
`default async method edit self.skip(): void!`. Removing default leaves the existing method
head grammar; there is no separate modifier-order family or `async default` alternative. The
root contract and completion fragment repeat the same complete header.

An origin on an edit input is its post-state bound. Origins on read/take inputs constrain incoming
carried provenance under the owning type/schema contract; they never turn a read into an edit or
an owner. Names resolve against the complete header, including later inputs, not source order.
The parenthesized edit receiver permits its own post-state bound without overloading the method
result's clause. Unqualified `self`, `edit self` and `take self` are the only simple receiver forms;
there is no expanded typed receiver or optional symbolic form.

`mut from input (...)` describes the surviving environment's post-state. The clause after the
result type instead describes the result. A `func` or `once` head cannot have the mutating-state
clause. A callable result consumes its complete V2 type, so `func(i32): File!` and
`(func(i32): File)!` have different constructor trees. Grouping preserves those distinctions.

An explicit value-capture segment must be nonempty and ends with `;`. `(file^; path: Path) { ... }`
and `(file^;) { ... }` transfer a source; unmarked entries require component Copy evidence.
`(path: Path) { ... }` has no explicit capture segment.
The old `(^file; ...)` has no production. An expected callable contract can supply omitted closure
input operations; explicit operations must agree. Without an expected callable contract,
unmarked closure inputs default to read before checking the body. Body use cannot silently select
edit/take. Capture and data-type inference remain separate. Modes are never chosen from a call's
spelling or its later use. A closure result annotation does not duplicate a result access permission.

An index projection is the special existing-element protocol defined by the
[future indexing rules](equality-and-indexing.md#index-projections).
Its `return place`/final place result is not an ordinary function's value result. Read/edit
projection identities are selected from the source operation before authority checking; no
permission-based fallback is admitted.

### Expressions, Places and Statement Boundaries

The binary arithmetic, conversion, comparison, short-circuit and recovery precedence levels keep
their structural order. At unary level retain `!` and `-`, with `await` and postfix outcome
extraction in their existing precedence positions. Remove unary `&`, `&+`, `^` and the `move`
production. A unary `&&` token cannot be split into access operators. Binary `&&`/`||` remain
short-circuit operations. An expression outcome layer still admits one suffix, `?` or `!`;
nested layers need grouping. Repeated type optionality is not repeated expression extraction.

Postfix member, tuple-position, index and call structure is retained. A selected generic call
also admits explicit type/constant arguments under the existing generic-call disambiguation
contract, including `decode<Config>(bytes)`. Interface disambiguation uses the adopted
`value.(Interface.member)(...)` form when ordinary lookup is ambiguous. No UFCS, call-site
receiver assertions or direct argument-mode assertions are added.

A `read`/`edit` initializer must denote an existing place: a name, a grouped place, or its checked
field/element/range/outcome projection, including surviving-place recovery under the outcome
protocol. Parsing retains a non-place initializer for a focused diagnostic;
lookup/type analysis does not reparse it. These declarations activate access immediately. A plain
value-binding initializer instead constructs a value even if it is a bare place. Calls select
their input operations before planning direct-place preparation; a computed conditional operand
has already constructed its ordinary branch value and cannot be reinterpreted as an alias.

At a call/control input boundary, that produced value can lend read/edit through its owned
evaluation temporary under the value-access contract. It remains a value expression, not an
assignment/local-alias place. A direct fixed/read place is not snapshotted to invent edit rights.

Blocks retain a prefix of local `use` declarations, newline-separated statements, a possible
final expression and ordinary unit completion. A non-final expression must be unit or terminating.
Assignment remains a statement with RHS-first semantics; it has no expression precedence.
`return` without a same-line operand is bare unit return. `throw` requires an operand. `_` discards
one leaf; a complete discard statement uses only `let _`, without a type annotation. Alias
patterns require at least one named leaf. Neither aliases nor fixed owners can be retargeted by
assignment. Control/iteration patterns use the selected common tuple-pattern contract; a binding
pattern's operation applies consistently to its leaves.

A valid source program cannot depend on the retired `pub`/`construct`/`instance` or authored
`noalloc`/`notrap`/`blocking` roles. Complete future item grammar must enforce contract-only roots,
one-item path visibility and unified `impl` before module discovery is integrated. Reusing the
released `SourceFile` or `Item` productions without that migration is not permitted.

### Implementation Scope

The [private handoff](../../architecture/pipeline/value-access-declarations.md) owns actual
frontend support and implementation gaps. The [realization requirements](../../design/compiler-realization.md)
define remaining integrated admission work. Neither recognition nor an existing source corpus
qualifies full-language implementation or permits an alternate source mode.
