use nocter_model::TypeKind;

use crate::{CallableOwner, DeclarationProgram, NominalShape};

use super::{
    DeclarationDomain, ProgramIntegrityError, require, require_site, require_symbol, require_type,
    unique,
};

pub(super) fn validate_types(program: &DeclarationProgram) -> Result<(), ProgramIntegrityError> {
    for (_, kind) in program.types().iter() {
        match kind {
            TypeKind::Closure { .. } => {
                return Err(ProgramIntegrityError::InvalidPosition(
                    DeclarationDomain::Type,
                ));
            }
            TypeKind::Builtin(_)
            | TypeKind::Pointer(_)
            | TypeKind::Borrow { .. }
            | TypeKind::Slice(_)
            | TypeKind::FixedArray { .. }
            | TypeKind::PackEntry { .. }
            | TypeKind::Callable(_)
            | TypeKind::Optional(_)
            | TypeKind::Fallible(_) => {}
            TypeKind::GenericParameter(parameter) => {
                require(
                    program.declarations().generic_parameters().get(*parameter),
                    DeclarationDomain::Type,
                    DeclarationDomain::GenericParameter,
                )?;
            }
            TypeKind::InterfaceSelf(interface) => {
                require(
                    program.declarations().interfaces().get(*interface),
                    DeclarationDomain::Type,
                    DeclarationDomain::Interface,
                )?;
            }
            TypeKind::Nominal {
                definition,
                arguments,
            } => {
                let declaration = require(
                    program.declarations().nominal_types().get(*definition),
                    DeclarationDomain::Type,
                    DeclarationDomain::NominalType,
                )?;
                if arguments.len() != declaration.generic_parameters().len() {
                    return Err(ProgramIntegrityError::InvalidPosition(
                        DeclarationDomain::Type,
                    ));
                }
            }
            TypeKind::AssociatedProjection { associated, .. } => {
                require(
                    program.declarations().associated_types().get(*associated),
                    DeclarationDomain::Type,
                    DeclarationDomain::AssociatedType,
                )?;
            }
            TypeKind::Opaque {
                definition,
                arguments,
            } => {
                let declaration = require(
                    program.declarations().opaque_types().get(*definition),
                    DeclarationDomain::Type,
                    DeclarationDomain::OpaqueType,
                )?;
                if arguments.len() != declaration.generic_parameters().len() {
                    return Err(ProgramIntegrityError::InvalidPosition(
                        DeclarationDomain::Type,
                    ));
                }
            }
        }
    }
    Ok(())
}

pub(super) fn validate_nominal_types(
    program: &DeclarationProgram,
) -> Result<(), ProgramIntegrityError> {
    let declarations = program.declarations();
    for (id, nominal) in declarations.nominal_types().iter() {
        require_site(program, nominal.site(), DeclarationDomain::NominalType)?;
        require_symbol(program, nominal.name(), DeclarationDomain::NominalType)?;
        unique(nominal.generic_parameters(), DeclarationDomain::NominalType)?;
        unique(nominal.requirements(), DeclarationDomain::NominalType)?;
        match nominal.shape() {
            NominalShape::Struct { fields, .. } => {
                unique(fields, DeclarationDomain::NominalType)?;
                for field in fields {
                    let field = require(
                        declarations.fields().get(*field),
                        DeclarationDomain::NominalType,
                        DeclarationDomain::Field,
                    )?;
                    if field.owner() != id {
                        return Err(ProgramIntegrityError::OwnerMismatch(
                            DeclarationDomain::Field,
                        ));
                    }
                }
            }
            NominalShape::Enum { variants } => {
                unique(variants, DeclarationDomain::NominalType)?;
                for variant in variants {
                    let variant = require(
                        declarations.variants().get(*variant),
                        DeclarationDomain::NominalType,
                        DeclarationDomain::Variant,
                    )?;
                    if variant.owner() != id {
                        return Err(ProgramIntegrityError::OwnerMismatch(
                            DeclarationDomain::Variant,
                        ));
                    }
                }
            }
        }
    }
    for (id, field) in declarations.fields().iter() {
        require_site(program, field.site(), DeclarationDomain::Field)?;
        require_symbol(program, field.name(), DeclarationDomain::Field)?;
        require_type(program, field.ty(), DeclarationDomain::Field)?;
        let nominal = require(
            declarations.nominal_types().get(field.owner()),
            DeclarationDomain::Field,
            DeclarationDomain::NominalType,
        )?;
        if !matches!(nominal.shape(), NominalShape::Struct { fields, .. } if fields.contains(&id)) {
            return Err(ProgramIntegrityError::OwnerMismatch(
                DeclarationDomain::Field,
            ));
        }
    }
    for (id, variant) in declarations.variants().iter() {
        require_site(program, variant.site(), DeclarationDomain::Variant)?;
        require_symbol(program, variant.name(), DeclarationDomain::Variant)?;
        unique(variant.payload(), DeclarationDomain::Variant)?;
        let nominal = require(
            declarations.nominal_types().get(variant.owner()),
            DeclarationDomain::Variant,
            DeclarationDomain::NominalType,
        )?;
        if !matches!(nominal.shape(), NominalShape::Enum { variants } if variants.contains(&id)) {
            return Err(ProgramIntegrityError::OwnerMismatch(
                DeclarationDomain::Variant,
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_aliases_interfaces(
    program: &DeclarationProgram,
) -> Result<(), ProgramIntegrityError> {
    let declarations = program.declarations();
    for (_, alias) in declarations.type_aliases().iter() {
        require_site(program, alias.site(), DeclarationDomain::TypeAlias)?;
        require_symbol(program, alias.name(), DeclarationDomain::TypeAlias)?;
        require_type(program, alias.target(), DeclarationDomain::TypeAlias)?;
        unique(alias.generic_parameters(), DeclarationDomain::TypeAlias)?;
        unique(alias.requirements(), DeclarationDomain::TypeAlias)?;
    }
    for (id, interface) in declarations.interfaces().iter() {
        require_site(program, interface.site(), DeclarationDomain::Interface)?;
        require_symbol(program, interface.name(), DeclarationDomain::Interface)?;
        unique(interface.generic_parameters(), DeclarationDomain::Interface)?;
        unique(interface.requirements(), DeclarationDomain::Interface)?;
        unique(interface.associated_types(), DeclarationDomain::Interface)?;
        unique(interface.methods(), DeclarationDomain::Interface)?;
        for associated in interface.associated_types() {
            let associated = require(
                declarations.associated_types().get(*associated),
                DeclarationDomain::Interface,
                DeclarationDomain::AssociatedType,
            )?;
            if associated.interface() != id {
                return Err(ProgramIntegrityError::OwnerMismatch(
                    DeclarationDomain::AssociatedType,
                ));
            }
        }
        for method in interface.methods() {
            let method = require(
                declarations.callables().get(*method),
                DeclarationDomain::Interface,
                DeclarationDomain::Callable,
            )?;
            if method.owner() != CallableOwner::Interface(id) {
                return Err(ProgramIntegrityError::OwnerMismatch(
                    DeclarationDomain::Callable,
                ));
            }
        }
    }
    for (id, associated) in declarations.associated_types().iter() {
        require_site(
            program,
            associated.site(),
            DeclarationDomain::AssociatedType,
        )?;
        require_symbol(
            program,
            associated.name(),
            DeclarationDomain::AssociatedType,
        )?;
        unique(associated.bounds(), DeclarationDomain::AssociatedType)?;
        let interface = require(
            declarations.interfaces().get(associated.interface()),
            DeclarationDomain::AssociatedType,
            DeclarationDomain::Interface,
        )?;
        if !interface.associated_types().contains(&id) {
            return Err(ProgramIntegrityError::OwnerMismatch(
                DeclarationDomain::AssociatedType,
            ));
        }
    }
    Ok(())
}
