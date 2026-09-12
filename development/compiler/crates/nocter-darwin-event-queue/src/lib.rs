//! Safe ownership boundary around Darwin's native event queue.

#[cfg(target_os = "macos")]
mod darwin;

#[cfg(target_os = "macos")]
pub use darwin::{EventFilter, EventQueue, EventQueueError, NativeEvent};
