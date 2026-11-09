# Recoverable Errors

## Value Access

An owned `error` retains its private immutable failure graph, not access into the views originally
used to construct its text. That graph has no caller-managed region requirement. Moving the handle
transfers its cleanup obligation; destroying it releases its owned graph according to the error
storage contract and invokes no user-defined destructor or callback. Reading the handle does not
copy it. Access returned from an accessor still depends on the live handle.

These concrete snapshot/storage facts do not make other move-only values independent. A future
aggregate, closure, or future can carry outside access and storage dependencies even when owned.
