use std::collections::BTreeMap;

use nocter_declarations::{DeclarationProgramBuilder, PackageTarget};
use nocter_model::{ModuleId, PackageId, PackageTargetKind};
use nocter_source_index::{SemanticEntity, SourceOrigin, SourceRole};

use crate::{
    ModuleIdentity, ModuleSourceKind, PackageInput, PackageMode, PackageTargetResolutionInput,
    ReservationError, SurfaceSource,
};

/// Reserves discovery-selected package targets without interpreting an authored module path.
pub(crate) fn reserve_package_targets(
    packages: &[PackageInput<'_>],
    resolutions: &[PackageTargetResolutionInput],
    package_ids: &BTreeMap<crate::PackageIdentity, PackageId>,
    module_ids: &BTreeMap<ModuleIdentity, ModuleId>,
    program: &mut DeclarationProgramBuilder,
    source_index: &mut crate::frontend_projection::FrontendProjectionBuilder,
) -> Result<(), ReservationError> {
    let mut selected_names = BTreeMap::new();
    for resolution in resolutions {
        let declaration = resolution.declaration();
        let package = packages
            .iter()
            .find(|package| {
                package
                    .declaration()
                    .is_some_and(|input| input.syntax().source() == declaration.source())
            })
            .ok_or(ReservationError::InvalidPackageTarget(declaration))?;
        let package_declaration = package
            .declaration()
            .ok_or(ReservationError::InvalidPackageTarget(declaration))?;
        let tree = package_declaration.syntax();
        let name_symbol = program
            .symbols()
            .get(resolution.name())
            .ok_or_else(|| ReservationError::MissingSymbol(resolution.name().into()))?;
        let package_id = *package_ids
            .get(package.identity())
            .ok_or_else(|| ReservationError::UnknownPackage(resolution.module().clone()))?;
        let module_id = *module_ids
            .get(resolution.module())
            .ok_or_else(|| ReservationError::UnknownModule(resolution.module().clone()))?;
        if selected_names
            .insert((package_id, resolution.kind(), name_symbol), declaration)
            .is_some()
        {
            return Err(ReservationError::DuplicatePackageTarget(declaration));
        }
        let id = program.add_package_target(PackageTarget::new(
            package_id,
            module_id,
            name_symbol,
            resolution.kind(),
            resolution.declaration_order(),
        ))?;
        source_index.insert(
            SemanticEntity::PackageTarget(id),
            SourceRole::Declaration,
            SourceOrigin::from_node(tree, resolution.name_literal())
                .map_err(|_| ReservationError::InconsistentSource(tree.source()))?,
        )?;
    }
    Ok(())
}

pub(crate) fn reserve_single_file_targets(
    packages: &[PackageInput<'_>],
    sources: &[SurfaceSource<'_>],
    package_ids: &BTreeMap<crate::PackageIdentity, PackageId>,
    module_ids: &BTreeMap<ModuleIdentity, ModuleId>,
    program: &mut DeclarationProgramBuilder,
    source_index: &mut crate::frontend_projection::FrontendProjectionBuilder,
) -> Result<(), ReservationError> {
    for package in packages
        .iter()
        .filter(|package| package.mode() == PackageMode::SingleFile)
    {
        let module_identity = ModuleIdentity::new(package.identity().clone(), Vec::<&str>::new());
        let package_id = *package_ids
            .get(package.identity())
            .ok_or_else(|| ReservationError::UnknownPackage(module_identity.clone()))?;
        let module_id = *module_ids
            .get(&module_identity)
            .ok_or_else(|| ReservationError::UnknownModule(module_identity.clone()))?;
        let name = program
            .symbols()
            .get(package.display_name())
            .ok_or_else(|| ReservationError::MissingSymbol(package.display_name().into()))?;
        let source = sources
            .iter()
            .find(|source| {
                source.module() == &module_identity && source.kind() == ModuleSourceKind::SingleFile
            })
            .ok_or_else(|| ReservationError::UnknownModule(module_identity.clone()))?;
        let target = program.add_package_target(PackageTarget::new(
            package_id,
            module_id,
            name,
            PackageTargetKind::Executable,
            0,
        ))?;
        source_index.insert(
            SemanticEntity::PackageTarget(target),
            SourceRole::Declaration,
            SourceOrigin::from_node(source.syntax(), source.syntax().root_id())
                .map_err(|_| ReservationError::InconsistentSource(source.syntax().source()))?,
        )?;
    }
    Ok(())
}
