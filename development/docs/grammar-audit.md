# Grammar Closure Audit

This document tracks the v0.14.0 grammar gate without defining language behavior. Normative tokens,
accepted source forms, and semantics remain under [`spec/`](../../spec/README.md). The completed
gate will give syntactic recognition one normative owner so the new parser never has to infer a
grammar from examples spread across topical chapters.

## Authority Boundary

The final public grammar chapter will own only these questions:

- which token sequences form a source file
- which declarations and members are legal in each syntactic container
- which token sequences form types, requirements, statements, patterns, and expressions
- precedence, associativity, and contextual-keyword positions
- where newline and comma-list rules enter the grammar

Topical chapters continue to own resolution, visibility, typing, ownership, evaluation, ABI,
runtime behavior, diagnostics, and tooling presentation. They will link to grammar productions
instead of maintaining competing recognition rules.

## Inventory

| Grammar domain | Current normative sources | Closure work |
|---|---|---|
| Source text, tokens, comments, literals, newlines | `spec/13-lexical-grammar.md` | Keep as the lexical authority and export named terminals to the syntactic grammar. |
| Package directives and target gates | `spec/15-command-line-interface.md`, `spec/11-stdlib-primitives-os.md` | Define package-file prefix order, directive values, repetition, and declaration attachment. |
| Imports and visibility | `spec/01-modules-use.md` | Consolidate private, scoped, selected, alias, re-export, and same-module source forms. |
| Top-level declarations | `spec/02`, `03`, `05`, `08`, `11`, `17` through `24` | Define one `Item` production and exact member containers. |
| Types and result provenance | `spec/02`, `04`, `06`, `08`, `18`, `22` | Define one precedence grammar for prefix types, outcome suffixes, projections, callable contracts, opaque results, and `from`. |
| Generic parameters and requirements | `spec/08`, `22`, `23`, `24` | Reuse one generic list and one `where` predicate grammar in every eligible declaration. |
| Bindings, statements, and blocks | `spec/02`, `03`, `05`, `20` | Define declaration and expression statements, block imports, assignment, control transfer, drop, and body results. |
| Expressions and precedence | `spec/02`, `03`, `04`, `05`, `07`, `13`, `17`, `18`, `22` through `24` | Replace prose precedence with one complete expression grammar. |
| Patterns | `spec/02` | Integrate enum patterns, fallback `_`, target ownership prefixes, and container-specific restrictions. |
| Construction and literals | `spec/02`, `07`, `13`, `17`, `19` | Distinguish fixed arrays, typed literals, named-field construction, enum variants, strings, bytes, and interpolation. |
| Closures and callable contracts | `spec/08`, `18` | Integrate the closed capture/parameter grammar and callable type forms. |
| Native tests | `spec/20-native-testing.md` | Add the fixed `test Name Block` declaration to `Item`. |

## Findings

The lexical boundary is already substantially centralized. Syntactic productions are not: there
is no single normative `SourceFile`, `Item`, `Member`, `Type`, `Statement`, `Pattern`, or
`Expression` authority. Several focused productions exist, but a parser would still have to combine
prose and examples from multiple chapters. That does not satisfy the grammar gate.

The consolidation must not copy semantics into the grammar chapter. In particular, visibility
reachability, declaration ownership, type well-formedness, copyability, provenance eligibility,
operator selection, and control-flow validity remain later checks even when their source shapes are
recognized by one production.

## Work Order

1. Define notation, lexical-terminal imports, `SourceFile`, directive prefixes, `Item`, visibility,
   and every declaration/member container.
2. Define the complete type, generic, requirement, and result-contract grammar.
3. Define blocks, imports, bindings, assignment, control flow, patterns, and body-result positions.
4. Define expressions from postfix forms through binary precedence, outcome elimination,
   assignment exclusion, closures, construction, and literals.
5. Replace scattered formal productions with links to the canonical owner while preserving each
   topical semantic rule.
6. Add valid, boundary, and invalid syntax cases for every production and audit contextual
   keywords against the lexical chapter.

## Closure Gate

The grammar is closed only when every supported source form reaches one production, every removed
or deferred form is rejected without a compatibility production, and no parser choice depends on
name resolution, type information, declaration order, or a semantic fallback. Only then may the new
source/syntax compiler workspace be created.
