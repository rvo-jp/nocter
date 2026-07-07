//! Comment scanning and documentation attachment shared by compiler tooling.

mod documentation;
mod scan;

pub use documentation::{AttachedDocumentation, DocumentationTarget, attach_documentation};
pub use scan::{Comment, CommentKind, collect_comments, first_comment_span};

#[cfg(test)]
mod tests;
