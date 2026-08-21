use nocter_source::SourceFile;
use nocter_syntax::SyntaxTree;

use crate::DiscoveryError;

pub(crate) fn authored_package_targets(
    source: &SourceFile,
    tree: &SyntaxTree,
) -> Result<nocter_package::PackageDeclaration, DiscoveryError> {
    nocter_package::decode_package_declaration(source, tree)
        .map_err(DiscoveryError::PackageDeclaration)
}
