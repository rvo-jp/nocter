# nocter-syntax

## Responsibility

Transform one immutable source file into lexical tokens, diagnostics, and a lossless immutable syntax
tree for an explicit parse goal.

## Contract

The crate consumes `nocter-source` and the closed language vocabulary. It publishes token and node
identities, source-preserving trees, documentation trivia, structural queries, literal decoding,
syntax-owned subtree completeness, and a reusable parse product that binds only to equal normalized
source text. Source-backed semantic helpers accept `BoundSyntax`, which proves the source file and
tree share one exact source identity. It also publishes a canonical declaration-syntax surface
that excludes body contents, trivia, and source identities without interpreting a declaration.
Each pruned semantic body root—callable block or constant/static initializer expression—is
published separately as exact normalized body bytes keyed by its stable declaration-surface
locator. Its source-syntax provider
contract lets callers choose direct or revisioned parsing without exposing either mechanism to
package or discovery code. It does not resolve names or apply semantic rules.

## Internal Responsibilities

- lexing and token subdivision
- bounded future private-function and closed-record recognition through the explicit `V2CoreUnit`
  development parse goal, with a distinct root, explicit input/binding operation nodes, unit values,
  named and structural tuple data types, record/tuple values, member suffixes and joint postfix transfer nodes.
  Marked value-capture entries use the same joint caret token. Structural initializers use
  comma-delimited named fields. Control-header parsing leaves the first outer brace to its body,
  while nested delimiters restore ordinary expression recognition without type lookup. Outer
  header parentheses are groups, not closure heads consuming the control body's brace; an
  actual closure body in a header requires a containing delimiter.
  Local annotations, formal parameter headers and function results recognize bounded
  `func`/`mut`/`once` callable contracts through one annotation production, with unnamed
  data/input-operation parameters. Nested callback contracts remain outside this goal; record
  fields remain ordinary data shapes. `mut` and `once` are contextual
  only at an invocation head followed by `(`. Data annotations share recursive tuple/group and
  fixed-array recognition. Array lengths use the ordinary private expression parser, retaining one
  expression root under `FixedArrayType`; this recognizes source without normalizing a length or
  granting an array schema. Dynamic slice types remain outside this goal.
  Array lists, repetitions and joint bracket indexing retain distinct nodes in the same goal.
  `array_repetition_parts` supplies the seed/count source roles to lowering and checking without
  interpreting either expression. Delimiter-owned newlines and nesting limits apply equally to
  these values; mixed list/repetition forms, spreads and spaced indexing have no fallback.
  Tuples have at least two components, while `()` is only the unit expression. Tuple member suffixes
  retain integer tokens without interpreting their positions. Declaration/type/field lookup remains semantic work. Its lexical
  profile retires old modifier words and reserves `throw` without changing released `SourceFile`
  parsing or ordinary `lex`
- future closure head recognition in the same private goal, using one transactional discriminator
  for captures, inputs and result annotations before a body; unit and grouped expressions retain
  their own categories. Recognition does not establish environment or body acceptance
- the closed contextual-spelling catalog shared by parser comparisons and lexical validation; its
  conformance test consumes the published and development grammar tables within their version scopes
- canonical source-name and portable directory-module-segment validation
- event-based parsing and flat tree construction
- one callable-prefix recognizer shared by declaration classification, declaration parsing, and
  structural callable types; it consumes the language-owned modifier order and grammar categories
  and retains authored modifier nodes without effect or execution interpretation
- one structural `ProvenanceClause` in callable-result, input, receiver, structural-callable, and
  annotated-local positions without resolving its names
- syntax diagnostics and missing/error elements
- structural navigation and documentation extraction
- value-free floating-literal decimal/suffix decomposition shared by semantic consumers
- exact node-completeness queries for recovery consumers
- validated rebinding of source-independent parse work into one current `SourceMap` identity domain
- source-neutral declaration-syntax canonicalization with explicit body pruning
- exact per-body source surfaces under stable declaration locators
- bidirectional body-local node/token locators for source-neutral semantic recipes
- the shared direct/reusable source-syntax provider boundary

Integer magnitude decoding has one syntax-owned implementation. It validates the complete lexical
token and returns an unsigned magnitude; checking and compile-time evaluation retain responsibility
for sign, target range, and expected type. Future body typing shares that decoder rather than
maintaining another radix/underscore parser.

## Invariants

- Every syntax token retains its lexical-token identity and exact normalized range.
- Every independently built tree has an opaque identity carried by its `NodeId` values. Tree clones
  retain that identity; a separate parse of the same source cannot lend nodes to the first tree.
- `SourceId` and `NodeId` integrity tokens stay out of deterministic debug and presentation text.
- Floating-literal suffix spelling is decomposed once by syntax. Semantic consumers map that
  lexical suffix to a type and send only the decimal component to target evaluation.
- Contextual spellings remain identifier tokens; parser responsibilities select them through the
  typed catalog rather than duplicating source-text literals.
- Package, discovery, lowering, and tooling consume the syntax-owned module-segment validator;
  none maintains a second keyword or identifier exclusion list.
- Parser recovery preserves authored structure without inventing semantic success.
- Callable modifiers have dedicated nodes. One ordered vocabulary recognizes them in every
  supported syntax context; downstream stages never recover guarantees from token text or
  declaration spelling.
- The future core goal never falls back to released borrow/type/expression productions. Its call
  grammar has no access-qualified type or prefix update/transfer assertion, including under
  grouping. `const func` retains a dedicated compile-time modifier node; repeated, retired or
  async prefixes have no private grammar fallback. Required constant and immutable-static declarations retain their
  initializer as an expression body; a bodyless initializer or callable type annotation is outside
  this bounded goal. Postfix transfer is a distinct unary node, preserving its position in a chain;
  source ownership and repeated/fresh-source rejection remain semantic work. Contextual input and alias words are recognized structurally,
  never through name/type lookup. Its root
  cannot be admitted by the released target-selection/compile-input boundary. This goal is not
  exposed as a CLI language-version switch; declaration/body conformance remains separate.
- A consumer never infers subtree completeness from file-wide diagnostic presence.
- Bounded ambiguity is parsed once transactionally rather than reparsed after lookahead.
- Reusing parse work rewrites every embedded source identity and rejects unequal normalized text.
- Declaration-implementation block and initializer edits and their body-local diagnostics do not
  change the declaration-syntax surface; declaration shape, body presence, and declaration-local
  diagnostics do. Blocks inside header expressions remain complete declaration inputs, including
  unexecuted branches and their diagnostics. The syntax owner selects independent bodies by their
  enclosing declaration role, rather than pruning every block node.
- Editing one body changes only that body's exact surface; unchanged sibling bodies retain equal
  locators and bytes.
- A `BodySyntaxProjection` is a complete immutable locator table. The checking body-source catalog
  constructs it once for a current body and lends that same table to every semantic recipe
  capture and materialization operation.
