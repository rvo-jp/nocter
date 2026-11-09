# Syntactic Grammar


## Contextual Spellings

These additional identifier-shaped spellings have the adopted v2.0.0 roles below. Other adopted
positions are defined by this chapter's productions. This table does not claim a complete item
or module grammar; unspecified productions remain outside the development specification.

| Spelling | Contextual grammar position |
| --- | --- |
| `read` | scoped read binding only |
| `edit` | scoped update binding, input operation or enum branch input operation |
| `take` | value-receiving input or enum branch input operation |
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
V2InterfaceBounds = "impl" InterfaceApplication ("+" InterfaceApplication)*
V2InterfaceCallable = "default"? (V2FunctionHead | V2MethodHead)
                      V2GenericParameters? V2Parameters V2CallableTail V2Block?
V2TypeBehavior = "impl" V2DeclarationPattern V2WhereClause?
                 "{" LineSequence(V2BehaviorMember) "}"
V2BehaviorMember = V2ItemDirectives? (V2Function | V2Method | V2Literal
                   | V2StructuralGrant | V2Conformance | V2Comparison
                   | V2IndexProjection | V2Coercion | V2Expansion | V2Destructor)
V2Method = V2MethodHead V2GenericParameters? V2Parameters V2CallableTail V2Block?
V2StructuralGrant = "literal" "{" List(Name) "}"
V2Literal = "literal" ("[" "]" | "[" ":" "]" | EmptyStringLiteral)
            V2Parameters V2CallableTail V2Block?
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
lexical names/lists/trivia, and the [whole-source draft](../../history/reviews/v2.0.0-source-grammar-assembly.md)
for still-unclosed package metadata, special operation and expression
productions. Recognizing an envelope does not migrate those dependencies into adopted semantics.
In particular typed sequence/string literal and general user expansion behavior, generic solving,
primitives, target selection and package discovery still need their owning contracts before
compiler admission. A complete whole-language grammar is not claimed here; published productions
are never an implicit fallback.

## Access Syntax

These productions close the S12 value/input/callable/alias source boundary. They are separate
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
V2IndexProjection = "operator" "(" "edit"? "self" "[" V2OrdinaryParameter "]" ")"
                    ":" V2Type V2OriginClause? V2WhereClause? V2Block?

V2Closure = "(" V2ClosureHead ")" (":" V2Type V2OriginClause?)? V2Block
V2ClosureHead = (V2ValueCaptures ";" V2ClosureParameters? | V2ClosureParameters)?
V2ValueCaptures = NonEmptyList(Name)
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
             | "(" "edit"? V2Type "[" V2Type "]" ")" ":" V2Type
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
and receiving component preparation are resolved once after recognition. Other expansion
operators and typed literal contributions remain separate draft subjects.

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
async prefix on an anonymous closure or structural callable type, no call-site transfer marker,
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

An explicit value-capture segment must be nonempty and ends with `;`. `(file; path: Path) { ... }`
and `(file;) { ... }` capture values; `(path: Path) { ... }` has no explicit capture segment.
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

### First Access Validation Fragment

The pre-S12 `V2CoreUnit` implementation used a different grammar and value model. Its former
contract is preserved in the non-normative
[checkpoint snapshot](../../history/reviews/v2.0.0-access-checkpoint-contracts.md).
It is not a subset of the adopted S12 language and must not be exposed as a compatibility mode.
The [implementation design](../../history/reviews/v2.0.0-value-binding-implementation.md)
sets the coherent replacement boundary: declarations, typing, initialization, accesses, resources
and source fixtures must share one accepted-unit authority. The
[bounded implementation record](../../history/reviews/v2.0.0-value-binding-front-end.md)
distinguishes that replacement from the earlier checkpoint and from full-language support.

### Bounded Value Binding Fragment

The bounded S12/S16 replacement uses the internal `V2CoreUnit` goal below for definitions in one
supplied private module. This is its new source contract, not a compatibility variant or a public
package format. Its implementation and qualification are tracked in the
[development record](../../history/reviews/v2.0.0-dependent-callable-results.md).
The old corpus cannot qualify this grammar by relabeling its status. No bodies are thereby
permitted in contract-only `index.nct`.

This fragment imports future Name/literal/trivia/token/newline rules, `List`/`LineSequence`, and
`V2InputOperation`. It imports no released type, statement or expression root:

```text
V2CoreUnit = LineSequence(V2CoreFunction | V2CoreRecord)
V2CoreRecord = "copy"? "struct" Name "{" LineSequence(Name ":" V2CoreType) "}"
V2CoreFunction = "func" Name "(" List(V2CoreParameter) ")" ":" V2CoreAnnotation V2CoreBlock
V2CoreParameter = V2InputOperation? Name ":" V2CoreAnnotation
V2CoreType = Name | "void"
V2CoreAnnotation = V2CoreType | V2InvocationKind "(" List(V2InputOperation? V2CoreType) ")" ":" V2CoreType
V2CoreBlock = "{" LineSequence(V2CoreEntry) "}"
V2CoreEntry = V2CoreStatement | V2CoreExpression
V2CoreStatement = ("let" | "var" | "read" | "edit") (Name | "_")
                   (":" V2CoreAnnotation)? "=" V2CoreExpression
                 | V2CorePostfix V2CoreAssignment V2CoreExpression
                 | "return" V2CoreExpression? | "drop" Name
                 | "break" | "continue"
                 | "while" V2CoreExpression V2CoreBlock | "loop" V2CoreBlock
V2CoreAssignment = "=" | "+=" | "-=" | "*=" | "/=" | "%="
V2CoreExpression = V2CoreOr
V2CoreOr = V2CoreAnd ("||" V2CoreAnd)*
V2CoreAnd = V2CoreEquality ("&&" V2CoreEquality)*
V2CoreEquality = V2CoreOrdering (("==" | "!=") V2CoreOrdering)?
V2CoreOrdering = V2CoreAdd (("<" | "<=" | ">" | ">=") V2CoreAdd)?
V2CoreAdd = V2CoreMultiply (("+" | "-") V2CoreMultiply)*
V2CoreMultiply = V2CorePrefix (("*" | "/" | "%") V2CorePrefix)*
V2CorePrefix = ("!" | "-") V2CorePrefix | V2CorePostfix
V2CorePostfix = V2CorePrimary (("(" List(V2CoreExpression) ")") | ("." Name))*
V2CorePrimary = V2CoreRecordValue | V2CoreClosure | Name | integer_literal | "true" | "false" | "(" ")"
               | "(" V2CoreExpression ")" | V2CoreIf
V2CoreRecordValue = Name "{" List(Name ":" V2CoreExpression) "}"
V2CoreIf = "if" V2CoreExpression V2CoreBlock
           ("else" (V2CoreBlock | V2CoreIf))?
V2CoreClosure = "(" (NonEmptyList(Name) ";")? List(V2CoreClosureParameter) ")"
                (":" V2CoreType)? V2CoreBlock
V2CoreClosureParameter = V2InputOperation? Name (":" V2CoreType)?
```

Call openers stay on their callee's logical line. Parentheses preserve a place's category. Binary levels
associate left; comparisons allow one ungrouped operator; short-circuit right operands are
conditional edges. A return without a same-line operand is bare unit return. Blocks supply their
final expression or normal unit completion. A non-final expression must be unit or terminating.
Recognized non-place alias initializers/assignment targets are semantic errors, not alternate parses.
A complete discard is exactly `let _ = expression`, without an annotation; other `_` bindings
in this bounded grammar fail validation.

Resolve all named headers before bodies. Every parameter's mode comes from its head: read when
unmarked, otherwise edit/take. The result is always a delivered value, never I/U access. No
body-selected mode, result qualifier, `from`, generic, receiver,
pack, outcome, async, const or module production is inherited into this fragment. Such constructs
are outside this private goal, not prohibited in the complete language. Direct calls resolve one
named function (including transparent grouping) and its argument count/contract before permission
checks. Functions are not storage places. A place here is a local/input name followed by zero or more declared record-field selections,
with transparent grouping. Resolve each field once through its containing record; two distinct
fields are disjoint logical places, while a field overlaps its containing place. Member selection
from computed record values follows [value selection](value-access.md#field-selection-from-values);
indexing remains outside this bounded fragment. Structural record values
initialize every field exactly once in authored order under ordinary value construction; follow
the nominal payload construction and interrupted-cleanup contract. At the outer level of `if`/
`while` conditions, the first `{` starts the control body; nested delimiters restore record-value
syntax without type lookup. Record data names resolve before bodies, including forward references. Follow
[nominal payload rules](structs-and-enums.md#nominal-value-payloads) for
finite containment, explicit Copy opt-in and closed field types. All fields are private-unit
members in this fragment; visibility and module contracts are not inherited.

Use the owning value-binding algorithm: Copy snapshots readable admitted data; otherwise move an
eligible owner. Read/edit aliases never supply non-Copy extraction. Fixed owners may transfer/drop
but cannot be assigned; mutable owners and take inputs can update/reinitialize their slots. Edit
aliases/inputs update their original places and must remain valid on outward recoverable exits.
Read inputs remain read even for Copy data, and a read Copy input can supply a separate take
snapshot. Two taking uses of a known-Copy scalar can succeed; two transfers of one non-Copy owner
cannot commit together. There are no source-level access/transfer prefix operations.

`bool`, `i32` and `void` are Copy and have empty carried-access/external-storage requirements and
trivial cleanup. The opaque move-only `error` owns its private immutable graph under the
[error contract](../library/error.md#value-access); it has empty
outside D/S and graph cleanup. These are exact admitted data contracts, not implications of Copy
or value ownership. No constructor or failure outcome is introduced for error by this fragment.
A read error parameter therefore cannot be returned as an error value; a take error can transfer
it. Closed record payloads compose these exact leaf contracts and retain their schema cleanup.
All admitted value results are independent of callee-local storage under those contracts. Ordinary read/
edit aliases can still have R into local/input storage and need ordinary access validation.

Primitive arithmetic/ordering use i32, logic uses bool, and equality compares matching bool/i32/
void values under the [primitive equality rule](equality-and-indexing.md#core-primitive-equality).
Uncontextualized integer literals default to i32; decode magnitude before range checks and admit
`-2147483648` through grouping of its literal operand. Preserve checked overflow/division/remainder
semantics and each prepared scalar payload. Unit `()` is the one void value. A non-producing local
initializer cannot infer a data type; an annotation can provide it. Termination does not invent a
stored value or a continuing flow edge.

At ordinary conditional/block value boundaries, normally producing branches apply Copy-or-move
to a common inferred/expected data type. A terminating branch contributes no answer. An if without
else completes with unit and its normally producing then branch must be unit. A computed conditional
operand has already constructed its answer; a direct named operand instead follows the selected
call's read/edit/take preparation. Do not reinterpret computed moves as delayed direct reservations.
A condition that terminates contributes no continuing branch; structurally unreachable code still
has its own name/type obligations without fictitious runtime resource reads.

Assignment is RHS-first. Reinitialize only writable owning slots; clean initialized old values
conditionally and never read missing ones. Compound assignment reads its target only after the
right-hand side. Propagate definite initialization, access demand and owner state through every
reachable branch/backedge/exit; include zero-iteration while paths and actual break/continue
successors. Non-breaking loops and literal-true while without a reachable break have no normal
completion. Never derive a continuing edge from a dead break or choose Copy from later liveness.

Stage value results before normal cleanup. Track owner responsibility in one ledger and alias/
reservation relationships in the shared graph; access validation cannot invent a second survival
set. Validate all definitions, including unused/recursive ones, before publishing an accepted unit.
Exact source rebinding preserves the same immutable declaration identities and every body.
Private front-end acceptance alone grants no target admission, native ABI, CLI mode or
released-program adapter. The complete model's capability/state-footprint obligations remain required
before admitting dependent aggregates or general callable environments.

Within the bounded value-binding fragment, named function values and local callable annotations
use [named function witnesses](callables.md#named-function-witnesses). Such an annotation has
unnamed, nonvariadic inputs over `V2CoreType`; it checks the selected named or formal witness's
input operations/data and result, and may select its permitted invocation adapter. Calls through these
values remain statically selected. Top-level callable parameters also admit these flat contracts
under [statically witnessed inputs](callables.md#statically-witnessed-callable-inputs). Callable
produced results lacking an established witness for a required input, nested callback contracts, record
fields containing callable contracts, general captured environments and erased
callables remain outside this bounded fragment. This limit does not change their full future grammar or justify treating
every structural callable contract as a capture-free Copy value.

Callable result annotations in this fragment follow
[statically witnessed results](callables.md#statically-witnessed-callable-results): their reachable
producers must close to one named or own-input witness relation before unit acceptance, with
actual witnesses substituted independently at each call.
An interrupted call may retain only its structural result contract, under the
[known-shape rules](values-and-types.md#known-shape-without-a-produced-value), without a produced
callable environment or executable edge.

Closure head recognition is structural and distinguishes grouped expressions and unit without
type lookup. The [bounded owned-capture scope](callables.md#bounded-owned-capture-producers)
admits exactly one explicit ordinary value capture, complete ordinary input/result data annotations,
and no nested or inferred outside captures. Recognition of a broader head does not admit it.
No released closure production or symbolic capture prefix is a fallback. Closure invocation
access is inferred from reached operations on its owned environment; explicit input operations
remain independent. Contextual completion of omitted input operations and data annotations is
outside this first producer scope.
