/// One direction of nonblocking descriptor progress.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ReadinessDirection {
    Readable,
    Writable,
}

/// One opaque asynchronous wait interest before target ABI encoding.
///
/// Timer deadlines use the monotonic counter domain supplied to the selected reactor. Operation
/// code remains the authority for interpreting readiness, timeout, closure, and error after the
/// computation resumes.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ReactorInterest {
    Descriptor {
        descriptor: u64,
        direction: ReadinessDirection,
    },
    Timer {
        deadline: u64,
    },
}
