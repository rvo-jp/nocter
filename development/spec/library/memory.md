# Allocation and Failure

## Storage Optimization

The future [storage rules](../language/memory-and-regions.md#storage-optimization)
distinguish logical value/resource obligations from compiler-selected physical backing. Automatic
frame/environment/transport storage need not invoke a heap allocator when another valid plan
implements it. No dummy request is needed to reproduce an optional backing plan's out-of-memory
termination. This does not change the current published declarations or behavior.

Exposed recoverable requests, failure atomicity, allocator/budget effects, layout validation,
backend identity, and required cleanup remain obligations of their selected API. An unused result
is not permission to remove them. A proven zero-size or otherwise inert path may be simplified
without weakening the general contract. Allocation operation identities and checked summaries
supply the evidence; the compiler does not infer privilege from an ordinary source name.

Eliding physical storage cannot silently change an owned buffer's semantic origin or allocator
for later growth/release. Nor can it move a required allocation failure beyond a capacity or
ownership-commit checkpoint. Unknown effects retain a conservative plan. API migration and
certificate validation are separate from this adopted future direction; no new modifier is added.
