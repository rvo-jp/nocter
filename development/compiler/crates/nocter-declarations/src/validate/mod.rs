use std::{collections::HashSet, fmt, hash::Hash};

use nocter_model::{
    BorrowCapability, BuiltinType, CallableCapability, ConstantValue, GenericParameterId, ModuleId,
    Symbol, TypeId, TypeKind,
};

use crate::{
    BodyOwner, CallableKind, CallableOwner, DeclarationProgram, GenericOwner, ParameterOwner,
    ParameterRole, Visibility,
};

mod attachment_rules;
mod attachments;
mod callables;
mod graph;
mod outcome;
mod requirements;
mod rules;
mod types;
mod violation;

pub(crate) use outcome::BodyAnalysisCapability;
pub use violation::{
    DeclarationRule, DeclarationValidationReport, DeclarationViolation, ProgramValidationError,
};

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeclarationDomain {
    Package,
    StandardLibrary,
    Module,
    Namespace,
    PackageTarget,
    Import,
    DeclarationSite,
    NominalType,
    TypeAlias,
    Interface,
    AssociatedType,
    Constant,
    Callable,
    Construction,
    Instance,
    Conformance,
    Drop,
    Test,
    Field,
    Variant,
    GenericParameter,
    Parameter,
    Requirement,
    Body,
    OpaqueType,
    Type,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProgramIntegrityError {
    UnknownSymbol(DeclarationDomain),
    UnknownType(DeclarationDomain),
    UnknownReference {
        owner: DeclarationDomain,
        target: DeclarationDomain,
    },
    OwnerMismatch(DeclarationDomain),
    DuplicateReference(DeclarationDomain),
    InvalidPosition(DeclarationDomain),
    InvalidDeclarationShape(DeclarationDomain),
    InvalidCallableShape,
    InvalidVisibility(DeclarationDomain),
    EmptyImportSelection,
}

impl fmt::Display for ProgramIntegrityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownSymbol(owner) => write!(formatter, "{owner:?} contains an unknown symbol"),
            Self::UnknownType(owner) => write!(formatter, "{owner:?} contains an unknown type"),
            Self::UnknownReference { owner, target } => {
                write!(
                    formatter,
                    "{owner:?} contains an unknown {target:?} reference"
                )
            }
            Self::OwnerMismatch(domain) => {
                write!(formatter, "{domain:?} is not owned by its recorded parent")
            }
            Self::DuplicateReference(domain) => {
                write!(formatter, "{domain:?} contains a duplicate identity")
            }
            Self::InvalidPosition(domain) => {
                write!(formatter, "{domain:?} has a non-canonical position")
            }
            Self::InvalidDeclarationShape(domain) => {
                write!(formatter, "{domain:?} has an invalid declaration shape")
            }
            Self::InvalidCallableShape => {
                formatter.write_str("callable kind, owner, name, receiver, or body is inconsistent")
            }
            Self::InvalidVisibility(domain) => {
                write!(
                    formatter,
                    "{domain:?} has an invalid normalized visibility boundary"
                )
            }
            Self::EmptyImportSelection => {
                formatter.write_str("resolved selected-name import is empty")
            }
        }
    }
}

impl std::error::Error for ProgramIntegrityError {}

pub(crate) fn validate_integrity(
    program: &DeclarationProgram,
) -> Result<(), ProgramIntegrityError> {
    types::validate_types(program)?;
    graph::validate_packages_modules_sites(program)?;
    graph::validate_standard_declarations(program)?;
    types::validate_nominal_types(program)?;
    types::validate_aliases_interfaces(program)?;
    validate_constants(program)?;
    callables::validate(program)?;
    validate_constructions_instances_conformances(program)?;
    validate_drops_tests(program)?;
    attachments::validate_ownership(program)?;
    validate_generic_parameters(program)?;
    validate_parameters(program)?;
    requirements::validate(program)?;
    validate_bodies(program)?;
    validate_opaque_types(program)?;
    graph::validate_namespaces(program)?;
    graph::validate_imports(program)?;
    graph::validate_package_targets(program)?;
    Ok(())
}

pub(crate) fn validate_language_rules(
    program: &DeclarationProgram,
) -> Result<outcome::DeclarationValidation, ProgramIntegrityError> {
    let mut collector = outcome::ValidationCollector::new();
    rules::validate(program, &mut collector);
    attachments::validate_rules(program, &mut collector)?;
    Ok(collector.finish(program))
}

fn validate_constants(program: &DeclarationProgram) -> Result<(), ProgramIntegrityError> {
    for (_, constant) in program.declarations().constants().iter() {
        require_site(program, constant.site(), DeclarationDomain::Constant)?;
        require_symbol(program, constant.name(), DeclarationDomain::Constant)?;
        require_type(program, constant.ty(), DeclarationDomain::Constant)?;
        let valid = match constant.value() {
            ConstantValue::Bool(_) => constant.ty() == program.types().builtin(BuiltinType::Bool),
            ConstantValue::Integer(value) => program
                .types()
                .get(constant.ty())
                .and_then(|ty| match ty {
                    TypeKind::Builtin(builtin) => Some(*builtin),
                    _ => None,
                })
                .is_some_and(|builtin| constant_integer_fits(*value, builtin)),
            ConstantValue::Text(_) => matches!(
                program.types().get(constant.ty()),
                Some(TypeKind::Borrow {
                    capability: BorrowCapability::Readonly,
                    referent,
                }) if *referent == program.types().builtin(BuiltinType::Str)
            ),
        };
        if !valid {
            return Err(ProgramIntegrityError::InvalidDeclarationShape(
                DeclarationDomain::Constant,
            ));
        }
    }
    Ok(())
}

fn constant_integer_fits(value: i128, builtin: BuiltinType) -> bool {
    match builtin {
        BuiltinType::I8 => i8::try_from(value).is_ok(),
        BuiltinType::I16 => i16::try_from(value).is_ok(),
        BuiltinType::I32 => i32::try_from(value).is_ok(),
        BuiltinType::I64 | BuiltinType::Isize => i64::try_from(value).is_ok(),
        BuiltinType::U8 => u8::try_from(value).is_ok(),
        BuiltinType::U16 => u16::try_from(value).is_ok(),
        BuiltinType::U32 => u32::try_from(value).is_ok(),
        BuiltinType::U64 | BuiltinType::Usize => u64::try_from(value).is_ok(),
        _ => false,
    }
}

fn validate_constructions_instances_conformances(
    program: &DeclarationProgram,
) -> Result<(), ProgramIntegrityError> {
    let declarations = program.declarations();
    for (id, construction) in declarations.constructions().iter() {
        require_site(
            program,
            construction.site(),
            DeclarationDomain::Construction,
        )?;
        require_type(
            program,
            construction.target(),
            DeclarationDomain::Construction,
        )?;
        unique(
            construction.generic_parameters(),
            DeclarationDomain::Construction,
        )?;
        if construction.members().is_empty() {
            return Err(ProgramIntegrityError::InvalidDeclarationShape(
                DeclarationDomain::Construction,
            ));
        }
        unique(construction.members(), DeclarationDomain::Construction)?;
        for member in construction.members() {
            let member = require(
                declarations.callables().get(*member),
                DeclarationDomain::Construction,
                DeclarationDomain::Callable,
            )?;
            if member.owner() != CallableOwner::Construction(id) {
                return Err(ProgramIntegrityError::OwnerMismatch(
                    DeclarationDomain::Callable,
                ));
            }
        }
    }
    for (id, instance) in declarations.instances().iter() {
        require_site(program, instance.site(), DeclarationDomain::Instance)?;
        require_type(program, instance.target(), DeclarationDomain::Instance)?;
        unique(instance.generic_parameters(), DeclarationDomain::Instance)?;
        unique(instance.requirements(), DeclarationDomain::Instance)?;
        unique(instance.members(), DeclarationDomain::Instance)?;
        for member in instance.members() {
            let member = require(
                declarations.callables().get(*member),
                DeclarationDomain::Instance,
                DeclarationDomain::Callable,
            )?;
            if member.owner() != CallableOwner::Instance(id) {
                return Err(ProgramIntegrityError::OwnerMismatch(
                    DeclarationDomain::Callable,
                ));
            }
        }
    }
    for (id, conformance) in declarations.conformances().iter() {
        require_site(program, conformance.site(), DeclarationDomain::Conformance)?;
        requirements::validate_interface_application(
            program,
            conformance.interface(),
            DeclarationDomain::Conformance,
        )?;
        require_type(
            program,
            conformance.target(),
            DeclarationDomain::Conformance,
        )?;
        unique(
            conformance.generic_parameters(),
            DeclarationDomain::Conformance,
        )?;
        unique(conformance.requirements(), DeclarationDomain::Conformance)?;
        unique(conformance.methods(), DeclarationDomain::Conformance)?;
        validate_complete_conformance(program, conformance)?;
        for method in conformance.methods() {
            let method = require(
                declarations.callables().get(*method),
                DeclarationDomain::Conformance,
                DeclarationDomain::Callable,
            )?;
            if method.owner() != CallableOwner::Conformance(id) {
                return Err(ProgramIntegrityError::OwnerMismatch(
                    DeclarationDomain::Callable,
                ));
            }
        }
    }
    Ok(())
}

fn validate_complete_conformance(
    program: &DeclarationProgram,
    conformance: &crate::ConformanceDeclaration,
) -> Result<(), ProgramIntegrityError> {
    let interface_id = conformance.interface().interface();
    requirements::validate_associated_bindings(
        program,
        conformance.associated_types(),
        interface_id,
        DeclarationDomain::Conformance,
    )?;
    Ok(())
}

fn validate_drops_tests(program: &DeclarationProgram) -> Result<(), ProgramIntegrityError> {
    let declarations = program.declarations();
    for (id, drop) in declarations.drops().iter() {
        require_site(program, drop.site(), DeclarationDomain::Drop)?;
        require_type(program, drop.target(), DeclarationDomain::Drop)?;
        unique(drop.generic_parameters(), DeclarationDomain::Drop)?;
        let body = require(
            declarations.bodies().get(drop.body()),
            DeclarationDomain::Drop,
            DeclarationDomain::Body,
        )?;
        if body.owner() != BodyOwner::Drop(id) {
            return Err(ProgramIntegrityError::OwnerMismatch(
                DeclarationDomain::Body,
            ));
        }
        let receiver = require(
            declarations.parameters().get(drop.receiver()),
            DeclarationDomain::Drop,
            DeclarationDomain::Parameter,
        )?;
        if receiver.owner() != ParameterOwner::Drop(id)
            || receiver.role() != ParameterRole::Receiver(CallableCapability::ReadWrite)
            || receiver.ty() != drop.target()
        {
            return Err(ProgramIntegrityError::OwnerMismatch(
                DeclarationDomain::Parameter,
            ));
        }
    }
    for (id, test) in declarations.tests().iter() {
        require_site(program, test.site(), DeclarationDomain::Test)?;
        require_symbol(program, test.name(), DeclarationDomain::Test)?;
        let body = require(
            declarations.bodies().get(test.body()),
            DeclarationDomain::Test,
            DeclarationDomain::Body,
        )?;
        if body.owner() != BodyOwner::Test(id) {
            return Err(ProgramIntegrityError::OwnerMismatch(
                DeclarationDomain::Body,
            ));
        }
    }
    Ok(())
}

fn validate_generic_parameters(program: &DeclarationProgram) -> Result<(), ProgramIntegrityError> {
    for (id, parameter) in program.declarations().generic_parameters().iter() {
        require_symbol(
            program,
            parameter.name(),
            DeclarationDomain::GenericParameter,
        )?;
        let list = generic_owner_list(program, parameter.owner()).ok_or(
            ProgramIntegrityError::OwnerMismatch(DeclarationDomain::GenericParameter),
        )?;
        if list.get(parameter.position()) != Some(&id) {
            return Err(ProgramIntegrityError::InvalidPosition(
                DeclarationDomain::GenericParameter,
            ));
        }
    }
    Ok(())
}

fn generic_owner_list(
    program: &DeclarationProgram,
    owner: GenericOwner,
) -> Option<&[GenericParameterId]> {
    let declarations = program.declarations();
    match owner {
        GenericOwner::NominalType(owner) => declarations
            .nominal_types()
            .get(owner)
            .map(crate::NominalTypeDeclaration::generic_parameters),
        GenericOwner::TypeAlias(owner) => declarations
            .type_aliases()
            .get(owner)
            .map(crate::TypeAliasDeclaration::generic_parameters),
        GenericOwner::Interface(owner) => declarations
            .interfaces()
            .get(owner)
            .map(crate::InterfaceDeclaration::generic_parameters),
        GenericOwner::Callable(owner) => declarations
            .callables()
            .get(owner)
            .map(crate::CallableDeclaration::generic_parameters),
        GenericOwner::Construction(owner) => declarations
            .constructions()
            .get(owner)
            .map(crate::ConstructionDeclaration::generic_parameters),
        GenericOwner::Instance(owner) => declarations
            .instances()
            .get(owner)
            .map(crate::InstanceDeclaration::generic_parameters),
        GenericOwner::Conformance(owner) => declarations
            .conformances()
            .get(owner)
            .map(crate::ConformanceDeclaration::generic_parameters),
        GenericOwner::Drop(owner) => declarations
            .drops()
            .get(owner)
            .map(crate::DropDeclaration::generic_parameters),
    }
}

fn validate_parameters(program: &DeclarationProgram) -> Result<(), ProgramIntegrityError> {
    let declarations = program.declarations();
    for (id, parameter) in declarations.parameters().iter() {
        require_symbol(program, parameter.name(), DeclarationDomain::Parameter)?;
        require_type(program, parameter.ty(), DeclarationDomain::Parameter)?;
        match parameter.owner() {
            ParameterOwner::Callable(owner) => {
                let callable = require(
                    declarations.callables().get(owner),
                    DeclarationDomain::Parameter,
                    DeclarationDomain::Callable,
                )?;
                match parameter.role() {
                    ParameterRole::Ordinary { position }
                    | ParameterRole::ArgumentPack { position }
                        if callable.parameters().get(position) == Some(&id) => {}
                    ParameterRole::Receiver(capability)
                        if callable.receiver() == Some(id)
                            && valid_receiver_capability(callable.kind(), capability) => {}
                    ParameterRole::Ordinary { .. }
                    | ParameterRole::ArgumentPack { .. }
                    | ParameterRole::Receiver(_) => {
                        return Err(ProgramIntegrityError::InvalidPosition(
                            DeclarationDomain::Parameter,
                        ));
                    }
                }
            }
            ParameterOwner::Variant(owner) => {
                let variant = require(
                    declarations.variants().get(owner),
                    DeclarationDomain::Parameter,
                    DeclarationDomain::Variant,
                )?;
                let ParameterRole::Ordinary { position } = parameter.role() else {
                    return Err(ProgramIntegrityError::OwnerMismatch(
                        DeclarationDomain::Parameter,
                    ));
                };
                if variant.payload().get(position) != Some(&id) {
                    return Err(ProgramIntegrityError::InvalidPosition(
                        DeclarationDomain::Parameter,
                    ));
                }
            }
            ParameterOwner::Drop(owner) => {
                let drop = require(
                    declarations.drops().get(owner),
                    DeclarationDomain::Parameter,
                    DeclarationDomain::Drop,
                )?;
                if drop.receiver() != id
                    || parameter.role() != ParameterRole::Receiver(CallableCapability::ReadWrite)
                    || parameter.ty() != drop.target()
                {
                    return Err(ProgramIntegrityError::OwnerMismatch(
                        DeclarationDomain::Parameter,
                    ));
                }
            }
        }
    }
    Ok(())
}

fn valid_receiver_capability(kind: CallableKind, capability: CallableCapability) -> bool {
    match kind {
        CallableKind::Coercion | CallableKind::Index => capability != CallableCapability::Owned,
        CallableKind::Equality | CallableKind::Ordering => {
            capability == CallableCapability::Readonly
        }
        CallableKind::Method | CallableKind::Expansion => true,
        CallableKind::Function
        | CallableKind::Primitive
        | CallableKind::ConstructionFunction
        | CallableKind::Literal(_) => false,
    }
}

fn validate_bodies(program: &DeclarationProgram) -> Result<(), ProgramIntegrityError> {
    let declarations = program.declarations();
    for (id, body) in declarations.bodies().iter() {
        let reciprocal = match body.owner() {
            BodyOwner::Callable(owner) => declarations
                .callables()
                .get(owner)
                .is_some_and(|owner| owner.body() == Some(id)),
            BodyOwner::Drop(owner) => declarations
                .drops()
                .get(owner)
                .is_some_and(|owner| owner.body() == id),
            BodyOwner::Test(owner) => declarations
                .tests()
                .get(owner)
                .is_some_and(|owner| owner.body() == id),
        };
        if !reciprocal {
            return Err(ProgramIntegrityError::OwnerMismatch(
                DeclarationDomain::Body,
            ));
        }
    }
    Ok(())
}

fn validate_opaque_types(program: &DeclarationProgram) -> Result<(), ProgramIntegrityError> {
    for (id, opaque) in program.declarations().opaque_types().iter() {
        let callable = require(
            program.declarations().callables().get(opaque.owner()),
            DeclarationDomain::OpaqueType,
            DeclarationDomain::Callable,
        )?;
        if opaque_result(program, callable.result()) != Some(id) {
            return Err(ProgramIntegrityError::InvalidDeclarationShape(
                DeclarationDomain::OpaqueType,
            ));
        }
        unique(opaque.generic_parameters(), DeclarationDomain::OpaqueType)?;
        for parameter in opaque.generic_parameters() {
            require(
                program.declarations().generic_parameters().get(*parameter),
                DeclarationDomain::OpaqueType,
                DeclarationDomain::GenericParameter,
            )?;
        }
        requirements::validate_interface_application(
            program,
            opaque.interface(),
            DeclarationDomain::OpaqueType,
        )?;
        requirements::validate_associated_bindings(
            program,
            opaque.associated_types(),
            opaque.interface().interface(),
            DeclarationDomain::OpaqueType,
        )?;
    }
    Ok(())
}

fn opaque_result(
    program: &DeclarationProgram,
    mut ty: TypeId,
) -> Option<nocter_model::OpaqueTypeId> {
    loop {
        match program.types().get(ty)? {
            nocter_model::TypeKind::Optional(payload)
            | nocter_model::TypeKind::Fallible(payload) => ty = *payload,
            nocter_model::TypeKind::Opaque { definition, .. } => return Some(*definition),
            _ => return None,
        }
    }
}

fn validate_visibility(
    program: &DeclarationProgram,
    declaring_module: ModuleId,
    visibility: Visibility,
    domain: DeclarationDomain,
) -> Result<(), ProgramIntegrityError> {
    let declaring = require(
        program.modules().get(declaring_module),
        domain,
        DeclarationDomain::Module,
    )?;
    match visibility {
        Visibility::Private | Visibility::Public => Ok(()),
        Visibility::Package(package) if package == declaring.package() => Ok(()),
        Visibility::Descendants(boundary) => {
            let boundary = require(
                program.modules().get(boundary),
                domain,
                DeclarationDomain::Module,
            )?;
            if boundary.package() == declaring.package()
                && boundary.path().is_ancestor_of(declaring.path())
            {
                Ok(())
            } else {
                Err(ProgramIntegrityError::InvalidVisibility(domain))
            }
        }
        Visibility::Package(_) => Err(ProgramIntegrityError::InvalidVisibility(domain)),
    }
}

fn require_site(
    program: &DeclarationProgram,
    site: nocter_model::DeclarationSiteId,
    owner: DeclarationDomain,
) -> Result<&crate::DeclarationSite, ProgramIntegrityError> {
    require(
        program.declaration_sites().get(site),
        owner,
        DeclarationDomain::DeclarationSite,
    )
}

fn require_symbol(
    program: &DeclarationProgram,
    symbol: Symbol,
    owner: DeclarationDomain,
) -> Result<(), ProgramIntegrityError> {
    program
        .symbols()
        .spelling(symbol)
        .map(|_| ())
        .ok_or(ProgramIntegrityError::UnknownSymbol(owner))
}

fn require_optional_symbol(
    program: &DeclarationProgram,
    symbol: Option<Symbol>,
    owner: DeclarationDomain,
) -> Result<(), ProgramIntegrityError> {
    symbol.map_or(Ok(()), |symbol| require_symbol(program, symbol, owner))
}

fn require_type(
    program: &DeclarationProgram,
    ty: TypeId,
    owner: DeclarationDomain,
) -> Result<(), ProgramIntegrityError> {
    program
        .types()
        .get(ty)
        .map(|_| ())
        .ok_or(ProgramIntegrityError::UnknownType(owner))
}

fn require<T>(
    value: Option<&T>,
    owner: DeclarationDomain,
    target: DeclarationDomain,
) -> Result<&T, ProgramIntegrityError> {
    value.ok_or(ProgramIntegrityError::UnknownReference { owner, target })
}

fn unique<T: Copy + Eq + Hash>(
    values: &[T],
    owner: DeclarationDomain,
) -> Result<(), ProgramIntegrityError> {
    let mut seen = HashSet::with_capacity(values.len());
    if values.iter().copied().all(|value| seen.insert(value)) {
        Ok(())
    } else {
        Err(ProgramIntegrityError::DuplicateReference(owner))
    }
}
