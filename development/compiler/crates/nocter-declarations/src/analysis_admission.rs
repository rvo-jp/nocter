use std::collections::{BTreeSet, HashMap};

use nocter_model::{
    BorrowCapability, BuiltinType, ConformanceId, ConstructionId, DropId, InstanceId, ModuleId,
    NominalTypeId, TypeId, TypeKind,
};

use crate::{
    BuiltinAttachment, CallableKind, DeclarationProgram, LiteralShape, NominalShape, ParameterRole,
    Visibility,
};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum AttachmentTarget {
    Nominal(NominalTypeId),
    Builtin(BuiltinType),
    Slice,
}

/// Exact declaration containers that may participate in editor semantic selection after an
/// authored declaration rule rejected compilation.
///
/// Rejected containers remain in the structurally valid declaration graph so their bodies,
/// parameters, and local bindings can be analyzed. Invalid constructions and drops, plus
/// unauthorized construction, instance, conformance, and drop attachments, are excluded from
/// global operation lookup. This admission scan covers every container independently of which
/// declaration supplied the first canonical diagnostic.
#[derive(Debug)]
pub struct DeclarationAnalysisAdmission {
    constructions: BTreeSet<ConstructionId>,
    instances: BTreeSet<InstanceId>,
    conformances: BTreeSet<ConformanceId>,
    drops: BTreeSet<DropId>,
}

impl DeclarationAnalysisAdmission {
    pub(crate) fn for_rejected(program: &DeclarationProgram) -> Self {
        let graph = program.graph();
        let construction_target_counts = graph.declarations().constructions().iter().fold(
            HashMap::<AttachmentTarget, usize>::new(),
            |mut counts, (_, declaration)| {
                if let Some(target) = attachment_target(program, declaration.target()) {
                    *counts.entry(target).or_default() += 1;
                }
                counts
            },
        );
        let constructions = graph
            .declarations()
            .constructions()
            .iter()
            .filter_map(|(id, declaration)| {
                let module = graph.declaration_sites().get(declaration.site())?.module();
                construction_is_admissible(
                    program,
                    declaration,
                    module,
                    &construction_target_counts,
                )
                .then_some(id)
            })
            .collect();
        let instances = graph
            .declarations()
            .instances()
            .iter()
            .filter_map(|(id, declaration)| {
                let module = graph.declaration_sites().get(declaration.site())?.module();
                inherent_target_is_authorized(program, declaration.target(), module).then_some(id)
            })
            .collect();
        let conformances = graph
            .declarations()
            .conformances()
            .iter()
            .filter_map(|(id, declaration)| {
                let module = graph.declaration_sites().get(declaration.site())?.module();
                conformance_target_is_authorized(program, declaration.target(), module)
                    .then_some(id)
            })
            .collect();
        let drop_target_counts = graph.declarations().drops().iter().fold(
            HashMap::<NominalTypeId, usize>::new(),
            |mut counts, (_, declaration)| {
                if let Some(AttachmentTarget::Nominal(target)) =
                    attachment_target(program, declaration.target())
                {
                    *counts.entry(target).or_default() += 1;
                }
                counts
            },
        );
        let drops = graph
            .declarations()
            .drops()
            .iter()
            .filter_map(|(id, declaration)| {
                let module = graph.declaration_sites().get(declaration.site())?.module();
                drop_is_admissible(program, declaration.target(), module, &drop_target_counts)
                    .then_some(id)
            })
            .collect();
        Self {
            constructions,
            instances,
            conformances,
            drops,
        }
    }

    #[must_use]
    pub fn admits_construction(&self, declaration: ConstructionId) -> bool {
        self.constructions.contains(&declaration)
    }

    #[must_use]
    pub fn admits_instance(&self, declaration: InstanceId) -> bool {
        self.instances.contains(&declaration)
    }

    #[must_use]
    pub fn admits_conformance(&self, declaration: ConformanceId) -> bool {
        self.conformances.contains(&declaration)
    }

    #[must_use]
    pub fn admits_drop(&self, declaration: DropId) -> bool {
        self.drops.contains(&declaration)
    }
}

fn construction_is_admissible(
    program: &DeclarationProgram,
    declaration: &crate::ConstructionDeclaration,
    module: ModuleId,
    target_counts: &HashMap<AttachmentTarget, usize>,
) -> bool {
    let Some(target) = attachment_target(program, declaration.target()) else {
        return false;
    };
    if !inherent_target_is_authorized(program, declaration.target(), module)
        || target_counts.get(&target) != Some(&1)
    {
        return false;
    }
    declaration.members().iter().all(|member| {
        let Some(member) = program.graph().declarations().callables().get(*member) else {
            return false;
        };
        outcome_payload(program, member.result()) == Some(declaration.target())
            && match member.kind() {
                CallableKind::Literal(shape) => {
                    valid_literal_signature(program, member, declaration.target(), shape)
                }
                _ => true,
            }
    })
}

fn drop_is_admissible(
    program: &DeclarationProgram,
    target: TypeId,
    module: ModuleId,
    target_counts: &HashMap<NominalTypeId, usize>,
) -> bool {
    let Some(AttachmentTarget::Nominal(definition)) = attachment_target(program, target) else {
        return false;
    };
    if target_counts.get(&definition) != Some(&1) {
        return false;
    }
    let declarations = program.graph().declarations();
    let Some(nominal) = declarations.nominal_types().get(definition) else {
        return false;
    };
    let owned_by_module = program
        .graph()
        .declaration_sites()
        .get(nominal.site())
        .is_some_and(|site| site.module() == module);
    owned_by_module
        && match nominal.shape() {
            NominalShape::Struct {
                copy_declared: false,
                ..
            } => true,
            NominalShape::Enum { variants } => variants.iter().any(|variant| {
                declarations
                    .variants()
                    .get(*variant)
                    .is_some_and(|variant| !variant.payload().is_empty())
            }),
            NominalShape::Struct {
                copy_declared: true,
                ..
            } => false,
        }
}

pub(crate) fn outcome_payload(program: &DeclarationProgram, mut ty: TypeId) -> Option<TypeId> {
    loop {
        match program.types().get(ty)? {
            TypeKind::Optional(payload) | TypeKind::Fallible(payload) => ty = *payload,
            _ => return Some(ty),
        }
    }
}

pub(crate) fn valid_literal_signature(
    program: &DeclarationProgram,
    callable: &crate::CallableDeclaration,
    target: TypeId,
    shape: LiteralShape,
) -> bool {
    let Some(site) = program.graph().declaration_sites().get(callable.site()) else {
        return false;
    };
    let [parameter] = callable.parameters() else {
        return false;
    };
    let Some(parameter) = program.graph().declarations().parameters().get(*parameter) else {
        return false;
    };
    if site.visibility() != Visibility::Public || callable.result() != target {
        return false;
    }
    match shape {
        LiteralShape::Sequence => parameter.role() == ParameterRole::ArgumentPack { position: 0 },
        LiteralShape::String => {
            parameter.role() == ParameterRole::Ordinary { position: 0 }
                && matches!(
                    program.types().get(parameter.ty()),
                    Some(TypeKind::Borrow {
                        capability: BorrowCapability::Readonly,
                        referent,
                    }) if *referent == program.types().builtin(BuiltinType::Str)
                )
        }
    }
}

pub(crate) fn attachment_target(
    program: &DeclarationProgram,
    ty: TypeId,
) -> Option<AttachmentTarget> {
    match program.types().get(ty)? {
        TypeKind::Nominal { definition, .. } => Some(AttachmentTarget::Nominal(*definition)),
        TypeKind::Builtin(builtin) => Some(AttachmentTarget::Builtin(*builtin)),
        TypeKind::Slice(_) => Some(AttachmentTarget::Slice),
        _ => None,
    }
}

pub(crate) fn inherent_target_is_authorized(
    program: &DeclarationProgram,
    ty: TypeId,
    module: ModuleId,
) -> bool {
    match attachment_target(program, ty) {
        Some(AttachmentTarget::Nominal(definition)) => program
            .graph()
            .declarations()
            .nominal_types()
            .get(definition)
            .and_then(|declaration| program.graph().declaration_sites().get(declaration.site()))
            .is_some_and(|site| site.module() == module),
        Some(AttachmentTarget::Builtin(builtin)) => builtin_attachment(builtin)
            .is_some_and(|attachment| is_standard_attachment_module(program, module, attachment)),
        Some(AttachmentTarget::Slice) => {
            is_standard_attachment_module(program, module, BuiltinAttachment::Slice)
        }
        None => false,
    }
}

pub(crate) fn conformance_target_is_authorized(
    program: &DeclarationProgram,
    ty: TypeId,
    module: ModuleId,
) -> bool {
    match attachment_target(program, ty) {
        Some(AttachmentTarget::Nominal(_)) => true,
        Some(AttachmentTarget::Builtin(_) | AttachmentTarget::Slice) => {
            is_standard_package_module(program, module)
        }
        None => false,
    }
}

pub(crate) fn builtin_attachment(builtin: BuiltinType) -> Option<BuiltinAttachment> {
    match builtin {
        BuiltinType::Bool
        | BuiltinType::I8
        | BuiltinType::I16
        | BuiltinType::I32
        | BuiltinType::I64
        | BuiltinType::U8
        | BuiltinType::U16
        | BuiltinType::U32
        | BuiltinType::U64
        | BuiltinType::Usize
        | BuiltinType::Isize => Some(BuiltinAttachment::Scalar),
        BuiltinType::Str => Some(BuiltinAttachment::Str),
        BuiltinType::Error => Some(BuiltinAttachment::Error),
        BuiltinType::Void | BuiltinType::Never => None,
    }
}

pub(crate) fn is_standard_attachment_module(
    program: &DeclarationProgram,
    module: ModuleId,
    attachment: BuiltinAttachment,
) -> bool {
    program
        .graph()
        .standard_library()
        .and_then(|standard| standard.attachment_module(attachment))
        == Some(module)
}

pub(crate) fn is_standard_package_module(program: &DeclarationProgram, module: ModuleId) -> bool {
    let package = program
        .graph()
        .modules()
        .get(module)
        .map(crate::Module::package);
    package.is_some() && package == program.graph().standard_package()
}
