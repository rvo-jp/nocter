use nocter_model::ModuleId;

use crate::{CallableOwner, DeclarationProgram, ExportedEntity, ImportTarget, StandardDeclaration};

use super::{
    DeclarationDomain, ProgramIntegrityError, require, require_site, require_symbol,
    validate_visibility,
};

pub(super) fn validate_packages_modules_sites(
    program: &DeclarationProgram,
) -> Result<(), ProgramIntegrityError> {
    for (_, package) in program.packages().iter() {
        require_symbol(program, package.display_name(), DeclarationDomain::Package)?;
    }
    for (_, module) in program.modules().iter() {
        require(
            program.packages().get(module.package()),
            DeclarationDomain::Module,
            DeclarationDomain::Package,
        )?;
        for segment in module.path().segments() {
            require_symbol(program, *segment, DeclarationDomain::Module)?;
        }
    }
    for (_, site) in program.declaration_sites().iter() {
        require(
            program.modules().get(site.module()),
            DeclarationDomain::DeclarationSite,
            DeclarationDomain::Module,
        )?;
        validate_visibility(
            program,
            site.module(),
            site.visibility(),
            DeclarationDomain::DeclarationSite,
        )?;
    }
    Ok(())
}

pub(super) fn validate_imports(program: &DeclarationProgram) -> Result<(), ProgramIntegrityError> {
    for (_, import) in program.imports().iter() {
        let source_module = import.module();
        require(
            program.modules().get(source_module),
            DeclarationDomain::Import,
            DeclarationDomain::Module,
        )?;
        validate_visibility(
            program,
            source_module,
            import.visibility(),
            DeclarationDomain::Import,
        )?;
        require(
            program.modules().get(import.target().module()),
            DeclarationDomain::Import,
            DeclarationDomain::Module,
        )?;
        match import.target() {
            ImportTarget::Namespace { .. } => require_symbol(
                program,
                import
                    .target()
                    .namespace_name()
                    .expect("namespace has a name"),
                DeclarationDomain::Import,
            )?,
            ImportTarget::Selected { names, .. } => {
                if names.is_empty() {
                    return Err(ProgramIntegrityError::EmptyImportSelection);
                }
                for name in names {
                    require_symbol(program, name.exported_name(), DeclarationDomain::Import)?;
                    require_symbol(program, name.local_name(), DeclarationDomain::Import)?;
                    let target_module =
                        exported_entity_module(program, name.target(), DeclarationDomain::Import)?;
                    if target_module != import.target().module() {
                        return Err(ProgramIntegrityError::OwnerMismatch(
                            DeclarationDomain::Import,
                        ));
                    }
                }
            }
        }
    }
    Ok(())
}

pub(super) fn validate_namespaces(
    program: &DeclarationProgram,
) -> Result<(), ProgramIntegrityError> {
    for (module, namespace) in program.module_namespaces().iter() {
        require(
            program.modules().get(module),
            DeclarationDomain::Module,
            DeclarationDomain::Module,
        )?;
        for entry in namespace.authored() {
            require_symbol(program, entry.name(), DeclarationDomain::Namespace)?;
            validate_visibility(
                program,
                module,
                entry.visibility(),
                DeclarationDomain::Namespace,
            )?;
            exported_entity_module(program, entry.target(), DeclarationDomain::Namespace)?;
        }
        for entry in namespace.fallback() {
            require_symbol(program, entry.name(), DeclarationDomain::Namespace)?;
            exported_entity_module(program, entry.target(), DeclarationDomain::Namespace)?;
        }
    }
    Ok(())
}

pub(super) fn validate_standard_declarations(
    program: &DeclarationProgram,
) -> Result<(), ProgramIntegrityError> {
    let Some(standard) = program.standard_library() else {
        return Ok(());
    };
    for (_, declaration) in standard.declarations() {
        let module = match declaration {
            StandardDeclaration::BuiltinType(builtin) => standard
                .builtin_type_module(builtin)
                .ok_or(ProgramIntegrityError::UnknownReference {
                    owner: DeclarationDomain::StandardLibrary,
                    target: DeclarationDomain::Module,
                })?,
            StandardDeclaration::NominalType(id) => standard_site_module(
                program,
                require(
                    program.declarations().nominal_types().get(id),
                    DeclarationDomain::StandardLibrary,
                    DeclarationDomain::NominalType,
                )?
                .site(),
            )?,
            StandardDeclaration::Interface(id) => standard_site_module(
                program,
                require(
                    program.declarations().interfaces().get(id),
                    DeclarationDomain::StandardLibrary,
                    DeclarationDomain::Interface,
                )?
                .site(),
            )?,
            StandardDeclaration::AssociatedType(id) => standard_site_module(
                program,
                require(
                    program.declarations().associated_types().get(id),
                    DeclarationDomain::StandardLibrary,
                    DeclarationDomain::AssociatedType,
                )?
                .site(),
            )?,
            StandardDeclaration::Callable(id) => standard_site_module(
                program,
                require(
                    program.declarations().callables().get(id),
                    DeclarationDomain::StandardLibrary,
                    DeclarationDomain::Callable,
                )?
                .site(),
            )?,
        };
        let package = require(
            program.modules().get(module),
            DeclarationDomain::StandardLibrary,
            DeclarationDomain::Module,
        )?
        .package();
        if package != standard.package() {
            return Err(ProgramIntegrityError::OwnerMismatch(
                DeclarationDomain::StandardLibrary,
            ));
        }
    }
    Ok(())
}

fn standard_site_module(
    program: &DeclarationProgram,
    site: nocter_model::DeclarationSiteId,
) -> Result<ModuleId, ProgramIntegrityError> {
    Ok(require_site(program, site, DeclarationDomain::StandardLibrary)?.module())
}

fn exported_entity_module(
    program: &DeclarationProgram,
    entity: ExportedEntity,
    owner: DeclarationDomain,
) -> Result<ModuleId, ProgramIntegrityError> {
    let declarations = program.declarations();
    let site = match entity {
        ExportedEntity::Module(module) => {
            require(
                program.modules().get(module),
                owner,
                DeclarationDomain::Module,
            )?;
            return Ok(module);
        }
        ExportedEntity::BuiltinType(builtin) => {
            return program
                .standard_library()
                .and_then(|standard| standard.builtin_type_module(builtin))
                .ok_or(ProgramIntegrityError::OwnerMismatch(owner));
        }
        ExportedEntity::NominalType(id) => require(
            declarations.nominal_types().get(id),
            owner,
            DeclarationDomain::NominalType,
        )?
        .site(),
        ExportedEntity::TypeAlias(id) => require(
            declarations.type_aliases().get(id),
            owner,
            DeclarationDomain::TypeAlias,
        )?
        .site(),
        ExportedEntity::Interface(id) => require(
            declarations.interfaces().get(id),
            owner,
            DeclarationDomain::Interface,
        )?
        .site(),
        ExportedEntity::Constant(id) => require(
            declarations.constants().get(id),
            owner,
            DeclarationDomain::Constant,
        )?
        .site(),
        ExportedEntity::Callable(id) => {
            let callable = require(
                declarations.callables().get(id),
                owner,
                DeclarationDomain::Callable,
            )?;
            if !matches!(callable.owner(), CallableOwner::Module(_)) {
                return Err(ProgramIntegrityError::OwnerMismatch(owner));
            }
            callable.site()
        }
    };
    Ok(require_site(program, site, owner)?.module())
}

pub(super) fn validate_package_targets(
    program: &DeclarationProgram,
) -> Result<(), ProgramIntegrityError> {
    let mut names = std::collections::HashSet::new();
    let mut positions = std::collections::HashSet::new();
    let mut previous = None;
    for (_, target) in program.package_targets().iter() {
        require_symbol(program, target.name(), DeclarationDomain::PackageTarget)?;
        require(
            program.packages().get(target.package()),
            DeclarationDomain::PackageTarget,
            DeclarationDomain::Package,
        )?;
        let module = require(
            program.modules().get(target.module()),
            DeclarationDomain::PackageTarget,
            DeclarationDomain::Module,
        )?;
        if module.package() != target.package() {
            return Err(ProgramIntegrityError::OwnerMismatch(
                DeclarationDomain::PackageTarget,
            ));
        }
        if !names.insert((target.package(), target.kind(), target.name()))
            || !positions.insert((target.package(), target.declaration_order()))
        {
            return Err(ProgramIntegrityError::DuplicateReference(
                DeclarationDomain::PackageTarget,
            ));
        }
        let position = (target.package(), target.declaration_order());
        if previous.is_some_and(|previous| previous > position) {
            return Err(ProgramIntegrityError::InvalidPosition(
                DeclarationDomain::PackageTarget,
            ));
        }
        previous = Some(position);
    }
    Ok(())
}
