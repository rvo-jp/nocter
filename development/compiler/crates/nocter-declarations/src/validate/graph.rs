use nocter_model::{BodyId, ModuleId};

use crate::{
    BodyOwner, CallableOwner, DeclarationProgram, ExportedEntity, ImportScope, ImportTarget,
    Visibility,
};

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
        let source_module = match import.scope() {
            ImportScope::Module(module) => {
                require(
                    program.modules().get(module),
                    DeclarationDomain::Import,
                    DeclarationDomain::Module,
                )?;
                module
            }
            ImportScope::Body(body) => body_module(program, body)?,
        };
        validate_visibility(
            program,
            source_module,
            import.visibility(),
            DeclarationDomain::Import,
        )?;
        if matches!(import.scope(), ImportScope::Body(_))
            && import.visibility() != Visibility::Private
        {
            return Err(ProgramIntegrityError::InvalidVisibility(
                DeclarationDomain::Import,
            ));
        }
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
                    let target_module = exported_entity_module(program, name.target())?;
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

fn exported_entity_module(
    program: &DeclarationProgram,
    entity: ExportedEntity,
) -> Result<ModuleId, ProgramIntegrityError> {
    let declarations = program.declarations();
    let site = match entity {
        ExportedEntity::Module(module) => return Ok(module),
        ExportedEntity::NominalType(id) => require(
            declarations.nominal_types().get(id),
            DeclarationDomain::Import,
            DeclarationDomain::NominalType,
        )?
        .site(),
        ExportedEntity::TypeAlias(id) => require(
            declarations.type_aliases().get(id),
            DeclarationDomain::Import,
            DeclarationDomain::TypeAlias,
        )?
        .site(),
        ExportedEntity::Interface(id) => require(
            declarations.interfaces().get(id),
            DeclarationDomain::Import,
            DeclarationDomain::Interface,
        )?
        .site(),
        ExportedEntity::Callable(id) => {
            let callable = require(
                declarations.callables().get(id),
                DeclarationDomain::Import,
                DeclarationDomain::Callable,
            )?;
            if !matches!(callable.owner(), CallableOwner::Module(_)) {
                return Err(ProgramIntegrityError::OwnerMismatch(
                    DeclarationDomain::Import,
                ));
            }
            callable.site()
        }
    };
    Ok(require_site(program, site, DeclarationDomain::Import)?.module())
}

pub(super) fn validate_package_targets(
    program: &DeclarationProgram,
) -> Result<(), ProgramIntegrityError> {
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
    }
    Ok(())
}

fn body_module(
    program: &DeclarationProgram,
    body: BodyId,
) -> Result<ModuleId, ProgramIntegrityError> {
    let body = require(
        program.declarations().bodies().get(body),
        DeclarationDomain::Import,
        DeclarationDomain::Body,
    )?;
    let site = match body.owner() {
        BodyOwner::Callable(owner) => require(
            program.declarations().callables().get(owner),
            DeclarationDomain::Import,
            DeclarationDomain::Callable,
        )?
        .site(),
        BodyOwner::Drop(owner) => require(
            program.declarations().drops().get(owner),
            DeclarationDomain::Import,
            DeclarationDomain::Drop,
        )?
        .site(),
        BodyOwner::Test(owner) => require(
            program.declarations().tests().get(owner),
            DeclarationDomain::Import,
            DeclarationDomain::Test,
        )?
        .site(),
    };
    Ok(require_site(program, site, DeclarationDomain::Import)?.module())
}
