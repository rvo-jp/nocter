# Lexical Grammar

## Access Tokens

Relative to the [published keyword inventory](../../../spec/language/lexical-grammar.md#identifiers), v2.0.0 adds `throw` and removes
`pub`, `construct`, `instance`, `move`, `noalloc`, `notrap` and `blocking`. Other reserved words
remain. Removed words can be ordinary names but have no retired declaration/operator role;
this is not acceptance of old syntax. This delta defines future tokenization only, leaving
the published lexer/highlighter inventory unchanged until a qualified release transition.

Future value/alias declarations begin with `let`, `var`, `read` or `edit`. `read` and `edit` are
contextual spellings in binding positions. `edit` and `take` are contextual parameter/receiver
operations. `mut` and `once` are structural callable type heads alongside `func`; no nominal-name
lookup selects their grammar. Contextual spellings remain identifier-shaped outside their defined
positions. A v2.0.0 parser must use its complete contextual catalog, not silently inherit the
published one through an unspecified fallback.

Edit/take also have the selected enum branch-input positions in match/if-is headers. Their
prefix requires separating gap and a completed operand/test; joint calls/indexing keep an
ordinary identifier reading. Read is not an explicit branch-input prefix. Copy can prefix enum
as well as struct declarations. These positions introduce no new reserved spelling or lookup-
dependent tokenization; the owning grammar specifies the contextual boundary.

Edit/take also select synchronous for operations only when a complete pattern and in delimiter
follow; `for take in source` binds an unmarked identifier take. Joint `cursor[]` is a zero-input
current-place projection, distinct from one-input `cursor[()]` and spaced typed literal syntax.
No new reserved word, unary access prefix or yielded-reference token is introduced.

Ordinary data types have no `&`/`^` access qualifier, and expressions have no unary access/transfer
prefix or `move` expression. Receiver operations are words, not `&self`/`^self`. Binary `&&` remains
conjunction; unary `&&value` has no access-prefix interpretation. Postfix `place^` is the sole
explicit transfer spelling and must be joint with its operand. It can precede member/call/outcome
suffixes, as in `buffer^.finish()` and `stored^?`. Capture entries before `;` can carry this same
suffix. There is no prefix or type/name qualifier.

The existing ellipsis starts a final ordinary/keyed pack parameter, including in a closure head,
or a contribution in a call tail. It is not a standalone unary operator or access assertion.
Contextual input-operation words still require a complete following binder/type; a pack named
take remains unmarked when no following name/type completes an operation-prefixed alternative.

`from` retains its contextual contract role. On an edit parameter it describes post-call state;
on a result it describes the delivered value. Origin names resolve after the complete input set
is collected, allowing a later parameter as the source. Tokenization never depends on this lookup.
Unit `()` uses ordinary parentheses; closure versus unit/group/tuple recognition is syntactic.

Retired `pub`, `construct`, `instance`, `noalloc`, `notrap` and `blocking` have no future declaration
role. This section changes future recognition only, not the released keyword inventory or website
highlighter. The [future grammar](syntactic-grammar.md#access-syntax) owns
precise positions; full declaration/module grammar migration is a separate stated gate.
