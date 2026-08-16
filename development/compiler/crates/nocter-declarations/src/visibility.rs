use nocter_model::{ModuleId, PackageId};

/// A declaration's normalized access boundary.
///
/// Relative authored scopes are resolved once during lowering. Later stages never reinterpret
/// `pub(../)` from a source path.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Visibility {
    Private,
    Descendants(ModuleId),
    Package(PackageId),
    Public,
}
