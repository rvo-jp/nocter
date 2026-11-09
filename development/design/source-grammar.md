# Remaining Source Grammar

## Scope

This is a non-normative draft for unresolved v2.0.0 productions. The
[owning grammar](../spec/language/syntactic-grammar.md) defines adopted productions; this file
imports them without editable copies. Recognition here does not admit a feature to the compiler
or select its semantics. Each remaining subject needs its owning contract, integrated source
examples and conformance before promotion. Important syntax-direction changes follow the
[approval boundary](../../AGENTS.md).

Imports include Name, ModuleSegment, literal/string/interpolation tokens, StringLiteral,
EmptyStringLiteral, BuiltinScalarType, trivia/newline/eof, gap and jointness, and the List,
NonEmptyList and LineSequence notation. No released expression or declaration root is inherited.
Type lookup cannot change a parse.

## Package Metadata

Exact schemas, file roles, dependency selection, entry and target behavior remain unresolved.

```text
V2PackageDirectives = V2PackageDirective (newline+ V2PackageDirective)*
V2PackageDirective = "#" ("package" | "dependencies" | "executable" | "test")
                      ":" V2DirectiveValue
V2DirectiveValue = StringLiteral | integer_literal | "{" List(V2DirectiveField) "}"
V2DirectiveField = Name ":" V2DirectiveValue
```

## Comparison and Coercion Operations

These families still need their complete operation, capability, failure and const contracts.
Index/current projections and synchronous expansion already have separate adopted owners.

```text
V2Comparison = "operator" "(" "self" ("==" | "<") Name ":" "Self" ")"
                ":" "bool" V2WhereClause? V2Block?
V2Coercion = "coerce" V2Receiver "as" V2Type V2OriginClause? V2Block?
```

## Expressions, Control and Typed Literals

The following block connects the adopted productions to the remaining block, expression,
package, region, typed literal and async-traversal questions. It does not adopt an unclosed
for-await protocol, dynamic sequence type, allocation override or coercion merely by spelling it.

```text
V2Block = "{" newline* (V2LocalUses (newline+ V2Entries)? | V2Entries)? newline* "}"
V2LocalUses = ("use" V2UseTree) (newline+ "use" V2UseTree)*
V2Entries = LineSequence(V2Entry)
V2Entry = V2Statement | V2Expression
V2Statement = V2Binding | V2Return | V2Throw | V2Drop
              | V2Postfix V2AssignmentOperator V2Expression | "break" | "continue"
              | "while" V2Header V2Block | "loop" V2Block
              | V2For | V2ForAwait
              | "region" Name "using" V2AllocatorPlace V2Block
V2AssignmentOperator = "=" | "+=" | "-=" | "*=" | "/=" | "%="
V2Header = V2Expression
V2ForAwait = "for" "await" V2InputOperation? V2Pattern "in" V2Header V2Block
V2AllocatorPlace = Name ("." (Name | integer_literal))*
V2Or = V2And ("||" V2And)*
V2And = V2Equality ("&&" V2Equality)*
V2Equality = V2Ordering (("==" | "!=") V2Ordering)?
V2Ordering = V2Shift (("<" | "<=" | ">" | ">=") V2Shift)?
V2Shift = V2Add (("<<" | ">>") V2Add)*
V2Add = V2Multiply (("+" | "-") V2Multiply)*
V2Multiply = V2Convert (("*" | "/" | "%") V2Convert)*
V2Convert = V2Unary ("as" V2Type)*
V2Primary = Name | "Self" | BuiltinScalarType | "str" | "error"
            | integer_literal | byte_literal | "true" | "false" | "none" | "(" ")"
            | "(" V2Expression ")"
            | V2TupleValue
            | V2Closure | V2If | V2Match | V2String | V2GenericOwnerMember
            | V2Owner V2RecordBody | V2Owner gap V2SequenceBody V2AllocationOverride?
            | V2Owner gap StringLiteral V2AllocationOverride? | V2Array
V2String = string_start (string_text | interpolation_start V2Expression interpolation_end)* string_end
V2GenericOwnerMember = V2GenericOwner "." Name
V2GenericOwner = Name V2GenericArguments ("." Name V2GenericArguments?)*
                | (Name | "Self") ("." Name)* "." Name V2GenericArguments
                  ("." Name V2GenericArguments?)*
V2Owner = V2NamedType | BuiltinScalarType | "str" | "error"
V2RecordBody = "{" List(V2FieldInitializer) "}"
V2FieldInitializer = Name ":" V2Expression
V2SequenceBody = "[" (":" | List(V2SequenceElement)) "]"
V2SequenceElement = V2Expression | "..." V2Expression | V2Expression ":" V2Expression
V2AllocationOverride = "using" V2AllocatorPlace
```

StructuralConstantExpression is a delimiter-bounded V2 expression restricted to the owning
compile-time domain. V2Header stops at the first outer opening brace; nested delimiters restore
ordinary expression parsing. Comparison levels allow one ungrouped operator; repeated binary
levels associate left. Preserve final block values, unit completion and RHS-first assignment.
There is no result-suppressing semicolon. An index opener is joint with its base; a spaced typed
literal is a separate family. Postfix transfer and generic delimiters follow the adopted owner.

Ordinary calls produce values. Field/element access remains a place only under its selected
projection contract. Record construction requires its explicit structural grant. Interface
selection is for a genuine collision, not a redundant style. Keep invalid targets in syntax for
semantic diagnostics rather than reparsing after a failed authority/type check.

## Closure Requirements

Normalize notation against the adopted grammar and check the combined nonterminal graph.
Validate delimiter depth, generic closers, contextual operation words, closure/group recognition,
postfix jointness, control-body boundaries and recovery with real parser cases. Each promotion
also requires its source-role, visibility, type, access, effect and cleanup contract. A complete
nonterminal inventory alone does not prove an unambiguous parser or a correct executable language.
