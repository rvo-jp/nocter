# Erased Callable Design

This document owns the compiler boundary for explicit runtime callable erasure. It does not define
general interface objects or replace statically witnessed structural callable types.

## Source Model

`func`, `&func`, and `&+func` remain statically witnessed callable annotations. Prefixing the
complete callable type with contextual `any` creates a distinct, sized, owning erased-callable
type:

```nct
any &func(Request): future Response!
any &+func(Event): void
any func(Job): Result
```

Callable modifiers remain part of the enclosed contract:

```nct
any noalloc &func(Request): future Response!
any blocking &+func(Event): void
```

The capability still describes invocation:

- `any &func` permits repeated invocation through readonly access;
- `any &+func` permits repeated invocation through readwrite access;
- `any func` permits one consuming invocation.

`any` describes representation. It does not make a closure copyable, relax its capability, infer a
stronger effect guarantee, or erase result provenance. Every erased callable is move-only because
the hidden environment may own move-only state. Its destruction destroys the hidden environment
exactly once.

Unlike a statically witnessed callable annotation, an erased callable is a complete data-bearing
type. It may be used for fields, variant payloads, aliases, generic arguments, parameters, local
bindings, and named results. Different concrete closure witnesses may therefore occupy values of
one erased-callable type.

## Erasure Boundary

Converting one concrete callable witness into `any ... func` is one checked operation. The
operation:

1. validates the exact structural contract and one-way guarantee weakening;
2. freezes the concrete environment layout and invocation body;
3. allocates compiler-owned environment storage;
4. moves the environment into that storage;
5. produces a fixed-size erased value containing opaque environment ownership and selected
   invocation/destruction identities.

Erasure is therefore a possible allocation even when invocation carries `noalloc`. The two facts
are intentionally independent: `noalloc` constrains calls through the finished erased value, not
construction of that value. Execution analysis sees an explicit erasure operation and rejects it
inside an allocation-free body. The compiler-owned storage follows the same self-contained mapped
storage policy as a future frame and is released by the frozen destroy target; it does not pretend
to be application allocator storage. Capture provenance remains attached to the resulting value,
so erasure cannot make a captured borrow escape.

Expected-type conversion may request erasure where the destination contract is explicitly an
erased callable, including an initialized annotation, an `as any ... func` conversion, or an erased
call parameter. There is no untyped or representation-inferred boxing: the destination's semantic
type is the authority. The erasure remains an allocation operation in the checked caller, so a
`noalloc` body cannot hide it behind argument conversion. APIs should accept statically witnessed
generic callables when they do not need to store heterogeneous values.

## Semantic Authority

Checking owns the complete erasure decision. A checked erasure records:

- the erased callable contract;
- the concrete closure or callable witness;
- the selected invocation capability;
- any guarantee weakening;
- the environment move and destruction dependency;
- the result provenance contract.

Calls through erased values use a distinct checked dispatch variant. It retains the erased value,
contract, and invocation access but contains no name, interface requirement, or lookup input.
Provenance, loans, ownership, and execution consume that same checked operation. None rediscovers
whether a value is erased from its type spelling.

## Executable and ABI Closure

`ExecutableProgram` closes every reached erasure site into an immutable erased-callable descriptor.
The descriptor joins the concrete environment representation, one monomorphized invoke item, and
one destruction plan. MIR receives only dense descriptor identities and already-selected indirect
call operations.

Machine layout owns the physical erased value. The initial representation is three machine words:

- opaque environment pointer;
- invoke address;
- destroy address.

This is a compiler ABI, not a public source layout. Machine lowering classifies arguments and
results from the preserved callable contract and invokes the frozen target through the stored
address. The ARM64 encoder receives an indirect-call machine operation; it does not understand
closures, callable contracts, or environment layout.

An owned consuming call transfers environment ownership to the invoke target. A readonly or
readwrite call retains ownership in the erased value and passes the corresponding environment
access. Normal destruction invokes the frozen destroy target only while ownership remains.

## Rejected Alternatives

- Reinterpreting `func` as erased would pessimize existing generic and local static dispatch and
  silently change allocation behavior.
- A general `any Interface` object would add unrelated witness-table, associated-type, and object
  safety rules before HTTP routing needs them.
- A borrowed two-word erased view cannot own heterogeneous handlers in a router and would require
  a second lifetime-bearing storage owner.
- Inline small-object optimization would make construction and movement representation-dependent
  before measurements justify the additional states.
- Recovering invocation targets from function names or machine symbols would duplicate checked
  dispatch and violate the executable-program boundary.

## Practical HTTP Boundary

The first standard-library consumer stores readonly repeatable request handlers:

```nct
type Handler = any &func(IncomingRequest, RouteMatch): future ServerConnection?!
```

Readonly invocation permits concurrent requests without granting hidden mutable access to handler
state. Applications that need shared mutation must capture an explicit concurrency-safe owner;
the router does not serialize handlers or own a task registry.

Routing selects method and validated request-target structure synchronously. Invocation produces a
future which remains owned by the application and may be inserted into its `TaskGroup`. The router
does not spawn, detach, limit, cancel, or drain work. Existing responder, body cursor, timeout,
persistent-connection, and graceful-shutdown authorities remain unchanged.
