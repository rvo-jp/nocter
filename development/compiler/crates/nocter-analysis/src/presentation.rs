use std::fmt::{self, Write};

use nocter_checking::{
    CheckedPredicate, CheckedProgram, GenericArguments, LocalBindingKind,
    RequiredConformanceMethod, SelectedConstructionEntry, SelectedConstructionSurface,
};
use nocter_declarations::{
    CallableKind, CallableOwner, DeclarationGraph, ExpansionCapability, ExportedEntity,
    NominalShape, ParameterRole, RequirementKind, RequirementSubject, StructuralCapability,
    Visibility,
};
use nocter_model::{
    BorrowCapability, BuiltinType, CallableCapability, Symbol, TypeId, TypeKind, TypeStore,
};
use nocter_source_index::SemanticEntity;

mod signature;
pub(crate) mod visible_spelling;

pub(super) use signature::{closure_signature_presentation, static_signature_presentation};

/// Canonical source-language presentation derived from checked semantics, never source slicing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticPresentation {
    code: Box<str>,
}

/// An internal inconsistency while rendering checked semantic data.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PresentationError {
    InvalidEntity(SemanticEntity),
    InvalidConstruction(nocter_model::NominalTypeId),
    ConstructionSurface(nocter_checking::ConstructionSurfaceSelectionError),
}

impl fmt::Display for PresentationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidEntity(entity) => {
                write!(formatter, "cannot render semantic entity {entity:?}")
            }
            Self::InvalidConstruction(nominal) => {
                write!(
                    formatter,
                    "cannot render construction surface for {nominal:?}"
                )
            }
            Self::ConstructionSurface(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for PresentationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ConstructionSurface(error) => Some(error),
            Self::InvalidEntity(_) | Self::InvalidConstruction(_) => None,
        }
    }
}

impl From<nocter_checking::ConstructionSurfaceSelectionError> for PresentationError {
    fn from(error: nocter_checking::ConstructionSurfaceSelectionError) -> Self {
        Self::ConstructionSurface(error)
    }
}

impl SemanticPresentation {
    #[must_use]
    pub fn code(&self) -> &str {
        &self.code
    }
}

pub(super) fn presentation(
    checked: &CheckedProgram,
    entity: SemanticEntity,
    spellings: &visible_spelling::VisibleSpellings,
) -> Option<SemanticPresentation> {
    let graph = checked.graph();
    let mut renderer = Renderer::new(graph, checked.types(), spellings);
    renderer.entity(Some(checked), entity)?;
    Some(SemanticPresentation {
        code: renderer.output.into_boxed_str(),
    })
}

pub(super) fn type_presentation_with_spellings(
    checked: &CheckedProgram,
    ty: TypeId,
    spellings: &visible_spelling::VisibleSpellings,
) -> Option<SemanticPresentation> {
    let mut renderer = Renderer::new(checked.graph(), checked.types(), spellings);
    renderer.ty(ty)?;
    Some(SemanticPresentation {
        code: renderer.output.into_boxed_str(),
    })
}

pub(super) fn recovery_type_presentation(
    projection: &nocter_model::TypeProjection,
    graph: &DeclarationGraph,
    from: nocter_model::ModuleId,
) -> Option<SemanticPresentation> {
    let spellings = visible_spelling::VisibleSpellings::new(graph, from);
    let mut renderer = Renderer::new(graph, projection.types(), &spellings);
    renderer.ty(projection.root())?;
    Some(SemanticPresentation {
        code: renderer.output.into_boxed_str(),
    })
}

pub(super) fn hover_presentation(
    checked: &CheckedProgram,
    entity: SemanticEntity,
    from: nocter_model::ModuleId,
    source_index: &nocter_source_index::SourceIndex,
    source: nocter_source::SourceId,
) -> Result<SemanticPresentation, PresentationError> {
    let graph = checked.graph();
    let spellings =
        visible_spelling::VisibleSpellings::for_source(graph, from, source_index, source);
    let mut renderer = Renderer::new(graph, checked.types(), &spellings);
    renderer
        .entity(Some(checked), entity)
        .ok_or(PresentationError::InvalidEntity(entity))?;
    if let SemanticEntity::NominalType(nominal) = entity {
        let surface = checked
            .construction_surfaces()
            .public_surface(graph, nominal, from)?;
        renderer
            .nominal_construction_surface(nominal, &surface)
            .ok_or(PresentationError::InvalidConstruction(nominal))?;
    }
    Ok(SemanticPresentation {
        code: renderer.output.into_boxed_str(),
    })
}

pub(super) fn prepared_presentation(
    prepared: &nocter_checking::PreparedSemanticProgram,
    entity: SemanticEntity,
    spellings: &visible_spelling::VisibleSpellings,
) -> Option<SemanticPresentation> {
    semantic_presentation(prepared.graph(), prepared.types(), entity, spellings)
}

pub(super) fn name_recovery_presentation(
    recovery: &nocter_checking::NameAnalysisRecovery,
    entity: SemanticEntity,
    spellings: &visible_spelling::VisibleSpellings,
) -> Option<SemanticPresentation> {
    semantic_presentation(recovery.graph(), recovery.types(), entity, spellings)
}

pub(super) fn declaration_presentation(
    recovery: &nocter_checking::DeclarationAnalysisRecovery,
    entity: SemanticEntity,
    spellings: &visible_spelling::VisibleSpellings,
) -> Option<SemanticPresentation> {
    semantic_presentation(recovery.graph(), recovery.types(), entity, spellings)
}

pub(super) fn required_conformance_method_presentation(
    recovery: &nocter_checking::DeclarationAnalysisRecovery,
    required: &RequiredConformanceMethod,
    from: nocter_model::ModuleId,
) -> Option<SemanticPresentation> {
    let spellings = visible_spelling::VisibleSpellings::new(recovery.graph(), from);
    let mut renderer = Renderer::new(recovery.graph(), recovery.types(), &spellings);
    renderer.required_conformance_method(required)?;
    Some(SemanticPresentation {
        code: renderer.output.into_boxed_str(),
    })
}

fn semantic_presentation(
    graph: &DeclarationGraph,
    types: &TypeStore,
    entity: SemanticEntity,
    spellings: &visible_spelling::VisibleSpellings,
) -> Option<SemanticPresentation> {
    let mut renderer = Renderer::new(graph, types, spellings);
    renderer.entity(None, entity)?;
    Some(SemanticPresentation {
        code: renderer.output.into_boxed_str(),
    })
}

pub(super) struct Renderer<'a> {
    graph: &'a DeclarationGraph,
    types: &'a TypeStore,
    output: String,
    generics: Option<&'a GenericArguments>,
    record_parameters: bool,
    parameter_ranges: Vec<(usize, usize)>,
    self_type: Option<TypeId>,
    spellings: &'a visible_spelling::VisibleSpellings,
}

impl<'a> Renderer<'a> {
    fn new(
        graph: &'a DeclarationGraph,
        types: &'a TypeStore,
        spellings: &'a visible_spelling::VisibleSpellings,
    ) -> Self {
        Self {
            graph,
            types,
            output: String::new(),
            generics: None,
            record_parameters: false,
            parameter_ranges: Vec::new(),
            self_type: None,
            spellings,
        }
    }

    fn entity(&mut self, checked: Option<&CheckedProgram>, entity: SemanticEntity) -> Option<()> {
        match entity {
            SemanticEntity::Module(_) => {
                self.workspace_entity(entity)?;
            }
            SemanticEntity::NominalType(_)
            | SemanticEntity::TypeAlias(_)
            | SemanticEntity::Interface(_)
            | SemanticEntity::AssociatedType(_) => self.type_entity(entity)?,
            SemanticEntity::Callable(_) | SemanticEntity::Field(_) | SemanticEntity::Variant(_) => {
                self.member_entity(entity)?;
            }
            SemanticEntity::GenericParameter(_)
            | SemanticEntity::Constant(_)
            | SemanticEntity::Parameter(_)
            | SemanticEntity::LocalBinding(..)
            | SemanticEntity::Capture(..)
            | SemanticEntity::Test(_) => self.value_entity(checked, entity)?,
            SemanticEntity::Package(_)
            | SemanticEntity::PackageTarget(_)
            | SemanticEntity::Import(_)
            | SemanticEntity::DeclarationSite(_)
            | SemanticEntity::Construction(_)
            | SemanticEntity::Instance(_)
            | SemanticEntity::Conformance(_)
            | SemanticEntity::Drop(_)
            | SemanticEntity::Requirement(_)
            | SemanticEntity::Body(_)
            | SemanticEntity::BodyScope(..)
            | SemanticEntity::BodyNode(..)
            | SemanticEntity::OpaqueType(_) => return None,
        }
        Some(())
    }

    fn workspace_entity(&mut self, entity: SemanticEntity) -> Option<()> {
        match entity {
            SemanticEntity::Module(id) => self.module(id)?,
            _ => return None,
        }
        Some(())
    }

    fn type_entity(&mut self, entity: SemanticEntity) -> Option<()> {
        let declarations = self.graph.declarations();
        match entity {
            SemanticEntity::NominalType(id) => {
                let declaration = declarations.nominal_types().get(id)?;
                self.visibility(declaration.site())?;
                let keyword = match declaration.shape() {
                    NominalShape::Struct { .. } => "struct",
                    NominalShape::Enum { .. } => "enum",
                };
                write!(self.output, "{keyword} ").ok()?;
                self.output.push_str(self.symbol(declaration.name())?);
                self.generic_parameters(declaration.generic_parameters())?;
                self.requirements(declaration.requirements())?;
            }
            SemanticEntity::TypeAlias(id) => {
                let declaration = declarations.type_aliases().get(id)?;
                self.visibility(declaration.site())?;
                self.output.push_str("type ");
                self.output.push_str(self.symbol(declaration.name())?);
                self.generic_parameters(declaration.generic_parameters())?;
                self.output.push_str(" = ");
                self.ty(declaration.target())?;
                self.requirements(declaration.requirements())?;
            }
            SemanticEntity::Interface(id) => {
                let declaration = declarations.interfaces().get(id)?;
                self.visibility(declaration.site())?;
                self.output.push_str("interface ");
                self.output.push_str(self.symbol(declaration.name())?);
                self.generic_parameters(declaration.generic_parameters())?;
                self.requirements(declaration.requirements())?;
            }
            SemanticEntity::AssociatedType(id) => {
                let declaration = declarations.associated_types().get(id)?;
                let owner = declarations.interfaces().get(declaration.interface())?;
                self.visibility(declaration.site())?;
                self.output.push_str("type ");
                self.exported_name(
                    ExportedEntity::Interface(declaration.interface()),
                    owner.name(),
                )?;
                self.output.push('.');
                self.output.push_str(self.symbol(declaration.name())?);
            }
            _ => return None,
        }
        Some(())
    }

    fn member_entity(&mut self, entity: SemanticEntity) -> Option<()> {
        let declarations = self.graph.declarations();
        match entity {
            SemanticEntity::Callable(id) => self.callable(id)?,
            SemanticEntity::Field(id) => {
                let field = declarations.fields().get(id)?;
                let owner = declarations.nominal_types().get(field.owner())?;
                self.visibility(field.site())?;
                self.output.push_str("field ");
                self.exported_name(ExportedEntity::NominalType(field.owner()), owner.name())?;
                self.output.push('.');
                self.output.push_str(self.symbol(field.name())?);
                self.output.push_str(": ");
                self.ty(field.ty())?;
            }
            SemanticEntity::Variant(id) => {
                let variant = declarations.variants().get(id)?;
                let owner = declarations.nominal_types().get(variant.owner())?;
                self.visibility(variant.site())?;
                self.output.push_str("variant ");
                self.exported_name(ExportedEntity::NominalType(variant.owner()), owner.name())?;
                self.output.push('.');
                self.output.push_str(self.symbol(variant.name())?);
                self.parameters(variant.payload())?;
            }
            _ => return None,
        }
        Some(())
    }

    fn value_entity(
        &mut self,
        checked: Option<&CheckedProgram>,
        entity: SemanticEntity,
    ) -> Option<()> {
        let declarations = self.graph.declarations();
        match entity {
            SemanticEntity::Constant(id) => {
                let declaration = declarations.constants().get(id)?;
                self.visibility(declaration.site())?;
                self.output.push_str("const ");
                self.output.push_str(self.symbol(declaration.name())?);
                self.output.push_str(": ");
                self.ty(declaration.ty())?;
                self.output.push_str(" = ");
                match declaration.value() {
                    nocter_model::ConstantValue::Bool(value) => {
                        self.output.push_str(if *value { "true" } else { "false" });
                    }
                    nocter_model::ConstantValue::Integer(value) => {
                        write!(self.output, "{value}").ok()?;
                    }
                    nocter_model::ConstantValue::Text(value) => {
                        write_string_literal(&mut self.output, value).ok()?;
                    }
                }
            }
            SemanticEntity::GenericParameter(id) => {
                let parameter = declarations.generic_parameters().get(id)?;
                write!(
                    self.output,
                    "type parameter {}",
                    self.symbol(parameter.name())?
                )
                .ok()?;
            }
            SemanticEntity::Parameter(id) => {
                let parameter = declarations.parameters().get(id)?;
                write!(
                    self.output,
                    "parameter {}: ",
                    self.symbol(parameter.name())?
                )
                .ok()?;
                self.ty(parameter.ty())?;
            }
            SemanticEntity::LocalBinding(body, id) => {
                let checked = checked?;
                let local = checked.bodies().get(body)?.locals().get(id)?;
                let introducer = match local.declaration().kind() {
                    LocalBindingKind::Mutable => "var",
                    LocalBindingKind::Immutable
                    | LocalBindingKind::PatternPayload
                    | LocalBindingKind::Loop
                    | LocalBindingKind::Region
                    | LocalBindingKind::Catch
                    | LocalBindingKind::ClosureParameter => "let",
                };
                write!(
                    self.output,
                    "{introducer} {}: ",
                    self.symbol(local.declaration().name())?
                )
                .ok()?;
                self.ty(local.ty())?;
            }
            SemanticEntity::Capture(body, id) => {
                let checked = checked?;
                let capture = checked.bodies().get(body)?.captures().get(id)?;
                write!(
                    self.output,
                    "capture {}: ",
                    self.symbol(capture.declaration().name())?
                )
                .ok()?;
                self.ty(capture.ty())?;
            }
            SemanticEntity::Test(id) => {
                let declaration = declarations.tests().get(id)?;
                write!(self.output, "test \"{}\"", self.symbol(declaration.name())?).ok()?;
            }
            _ => return None,
        }
        Some(())
    }

    fn module(&mut self, id: nocter_model::ModuleId) -> Option<()> {
        let start = self.output.len();
        self.output.push_str("module ");
        if self.visible_name(ExportedEntity::Module(id))? {
            return Some(());
        }
        self.output.truncate(start);
        let module = self.graph.modules().get(id)?;
        let package = self.graph.packages().get(module.package())?;
        write!(
            self.output,
            "module {}",
            self.symbol(package.display_name())?
        )
        .ok()?;
        for segment in module.path().segments() {
            write!(self.output, "/{}", self.symbol(*segment)?).ok()?;
        }
        Some(())
    }

    fn callable(&mut self, id: nocter_model::CallableId) -> Option<()> {
        let declarations = self.graph.declarations();
        let callable = declarations.callables().get(id)?;
        self.visibility(callable.site())?;
        if let CallableOwner::Construction(owner) = callable.owner()
            && declarations.constructions().get(owner)?.default_member() == Some(id)
        {
            self.output.push_str("default ");
        }
        if matches!(callable.owner(), CallableOwner::Interface(_)) && callable.body().is_some() {
            self.output.push_str("default ");
        }
        match callable.kind() {
            CallableKind::Primitive => {
                self.output.push_str("primitive func ");
                self.output.push_str(self.symbol(callable.name()?)?);
            }
            CallableKind::Function | CallableKind::ConstructionFunction => {
                self.output.push_str("func ");
                self.owner_prefix(callable.owner())?;
                self.output.push_str(self.symbol(callable.name()?)?);
            }
            CallableKind::Method => {
                self.output.push_str("method ");
                let receiver = declarations.parameters().get(callable.receiver()?)?;
                self.receiver(receiver.role(), callable.owner())?;
                self.output.push('.');
                self.output.push_str(self.symbol(callable.name()?)?);
            }
            CallableKind::Literal(shape) => {
                self.output.push_str("literal ");
                let CallableOwner::Construction(owner) = callable.owner() else {
                    return None;
                };
                self.ty(declarations.constructions().get(owner)?.target())?;
                self.output.push(' ');
                self.output.push_str(match shape {
                    nocter_declarations::LiteralShape::Sequence => "[]",
                    nocter_declarations::LiteralShape::String => "\"\"",
                });
            }
            CallableKind::Coercion => {
                self.output.push_str("coerce ");
                let receiver = declarations.parameters().get(callable.receiver()?)?;
                self.receiver(receiver.role(), callable.owner())?;
                self.output.push_str(" as ");
                self.ty(callable.result())?;
                return Some(());
            }
            CallableKind::Equality
            | CallableKind::Ordering
            | CallableKind::Index
            | CallableKind::Expansion => {
                self.operator(callable)?;
                return Some(());
            }
        }
        self.generic_parameters(callable.generic_parameters())?;
        self.parameters(callable.parameters())?;
        self.output.push_str(": ");
        self.ty(callable.result())?;
        self.provenance(callable)?;
        self.requirements(callable.requirements())?;
        Some(())
    }

    fn required_conformance_method(&mut self, required: &RequiredConformanceMethod) -> Option<()> {
        let declarations = self.graph.declarations();
        let callable = declarations.callables().get(required.interface_method())?;
        self.output.push_str("method ");
        self.output.push_str(match required.receiver() {
            CallableCapability::Readonly => "&self.",
            CallableCapability::ReadWrite => "&+self.",
            CallableCapability::Owned => "self.",
        });
        self.output.push_str(self.symbol(callable.name()?)?);
        self.generic_parameters(required.generic_parameters())?;
        self.output.push('(');
        for (index, parameter) in required.parameters().iter().copied().enumerate() {
            if index != 0 {
                self.output.push_str(", ");
            }
            if parameter.is_argument_pack() {
                self.output.push_str("...");
            }
            let declaration = declarations.parameters().get(parameter.declaration())?;
            self.output.push_str(self.symbol(declaration.name())?);
            self.output.push_str(": ");
            self.ty(parameter.ty())?;
        }
        self.output.push_str("): ");
        self.ty(required.result())?;
        self.checked_requirements(required.requirements())
    }

    fn checked_requirements(&mut self, requirements: &[CheckedPredicate]) -> Option<()> {
        if requirements.is_empty() {
            return Some(());
        }
        self.output.push_str(" where ");
        for (index, requirement) in requirements.iter().enumerate() {
            if index != 0 {
                self.output.push_str(", ");
            }
            self.checked_requirement(requirement)?;
        }
        Some(())
    }

    fn checked_requirement(&mut self, requirement: &CheckedPredicate) -> Option<()> {
        match requirement {
            CheckedPredicate::Capability {
                subject,
                capability,
            } => {
                self.ty(*subject)?;
                self.output.push_str(": ");
                self.structural_capability(capability)?;
            }
            CheckedPredicate::Copy(ty) => {
                self.output.push_str("copy ");
                self.ty(*ty)?;
            }
            CheckedPredicate::TypeEquality { left, right } => {
                self.ty(*left)?;
                self.output.push_str(" = ");
                self.ty(*right)?;
            }
            CheckedPredicate::Equality(ty) | CheckedPredicate::Ordering(ty) => {
                self.output.push_str("(&");
                self.ty(*ty)?;
                self.output
                    .push_str(if matches!(requirement, CheckedPredicate::Equality(_)) {
                        " == &"
                    } else {
                        " < &"
                    });
                self.ty(*ty)?;
                self.output.push_str("): bool");
            }
            CheckedPredicate::Index {
                capability,
                container,
                index,
                result,
            } => {
                self.output.push('(');
                self.output.push_str(match capability {
                    BorrowCapability::Readonly => "&",
                    BorrowCapability::ReadWrite => "&+",
                });
                self.ty(*container)?;
                self.output.push('[');
                self.ty(*index)?;
                self.output.push_str("]): ");
                self.ty(*result)?;
            }
            CheckedPredicate::Coercion { source, target } => {
                self.ty(*source)?;
                self.output.push_str(" as ");
                self.ty(*target)?;
            }
            CheckedPredicate::Expansion {
                capability,
                source,
                result,
            } => {
                self.output.push_str("(...");
                self.output.push_str(match capability {
                    ExpansionCapability::Readonly => "&",
                    ExpansionCapability::ReadWrite => "&+",
                    ExpansionCapability::Owned => "",
                });
                self.ty(*source)?;
                self.output.push_str("): ");
                self.ty(*result)?;
            }
        }
        Some(())
    }

    fn nominal_construction_surface(
        &mut self,
        nominal: nocter_model::NominalTypeId,
        surface: &SelectedConstructionSurface,
    ) -> Option<()> {
        let declarations = self.graph.declarations();
        let nominal_declaration = declarations.nominal_types().get(nominal)?;
        let structural = surface
            .entries()
            .contains(&SelectedConstructionEntry::Structural);
        let has_variants = surface
            .entries()
            .iter()
            .any(|entry| matches!(entry, SelectedConstructionEntry::Variant(_)));
        if structural {
            let NominalShape::Struct { fields, .. } = nominal_declaration.shape() else {
                return None;
            };
            self.output.push_str(" {\n");
            for field in fields.iter().copied() {
                let declaration = declarations.fields().get(field)?;
                self.output.push_str("    ");
                self.visibility(declaration.site())?;
                self.output.push_str(self.symbol(declaration.name())?);
                self.output.push_str(": ");
                self.ty(declaration.ty())?;
                self.output.push('\n');
            }
            self.output.push('}');
        } else if has_variants {
            self.output.push_str(" {\n");
            for entry in surface.entries() {
                let SelectedConstructionEntry::Variant(variant) = *entry else {
                    continue;
                };
                let declaration = declarations.variants().get(variant)?;
                self.output.push_str("    ");
                self.output.push_str(self.symbol(declaration.name())?);
                if !declaration.payload().is_empty() {
                    self.parameters(declaration.payload())?;
                }
                self.output.push('\n');
            }
            self.output.push('}');
        }

        let Some(construction_id) = surface.declaration() else {
            return Some(());
        };
        let construction = declarations.constructions().get(construction_id)?;
        self.output.push_str("\n\nconstruct ");
        self.declaration_type_pattern(construction.target())?;
        self.output.push_str(" {");
        let has_members = surface
            .entries()
            .iter()
            .any(|entry| matches!(entry, SelectedConstructionEntry::Callable(_)));
        if !has_members {
            self.output.push('}');
            return Some(());
        }
        self.self_type = Some(construction.target());
        for (index, entry) in surface.entries().iter().enumerate() {
            let SelectedConstructionEntry::Callable(member) = *entry else {
                continue;
            };
            self.output.push_str("\n    ");
            self.construction_member(member, surface.is_default(index))?;
        }
        self.self_type = None;
        self.output.push_str("\n}");
        Some(())
    }

    fn construction_member(
        &mut self,
        id: nocter_model::CallableId,
        is_default: bool,
    ) -> Option<()> {
        let callable = self.graph.declarations().callables().get(id)?;
        self.visibility(callable.site())?;
        if is_default {
            self.output.push_str("default ");
        }
        match callable.kind() {
            CallableKind::ConstructionFunction => {
                self.output.push_str("func ");
                self.output.push_str(self.symbol(callable.name()?)?);
            }
            CallableKind::Literal(shape) => {
                self.output.push_str("literal ");
                self.output.push_str(match shape {
                    nocter_declarations::LiteralShape::Sequence => "[]",
                    nocter_declarations::LiteralShape::String => "\"\"",
                });
            }
            _ => return None,
        }
        self.generic_parameters(callable.generic_parameters())?;
        self.parameters(callable.parameters())?;
        self.output.push_str(": ");
        self.ty(callable.result())?;
        self.provenance(callable)?;
        self.requirements(callable.requirements())
    }

    fn operator(&mut self, callable: &nocter_declarations::CallableDeclaration) -> Option<()> {
        let declarations = self.graph.declarations();
        let receiver = declarations.parameters().get(callable.receiver()?)?;
        self.output.push_str("operator (");
        if callable.kind() == CallableKind::Expansion {
            self.output.push_str("...");
        }
        self.receiver(receiver.role(), callable.owner())?;
        match callable.kind() {
            CallableKind::Equality | CallableKind::Ordering => {
                self.output
                    .push_str(if callable.kind() == CallableKind::Equality {
                        " == "
                    } else {
                        " < "
                    });
                self.parameter(callable.parameters().first().copied()?)?;
            }
            CallableKind::Index => {
                self.output.push('[');
                self.parameter(callable.parameters().first().copied()?)?;
                self.output.push(']');
            }
            CallableKind::Expansion => {}
            CallableKind::Function
            | CallableKind::Primitive
            | CallableKind::Method
            | CallableKind::ConstructionFunction
            | CallableKind::Literal(_)
            | CallableKind::Coercion => return None,
        }
        self.output.push_str("): ");
        self.ty(callable.result())?;
        self.provenance(callable)?;
        self.requirements(callable.requirements())
    }

    fn owner_prefix(&mut self, owner: CallableOwner) -> Option<()> {
        let declarations = self.graph.declarations();
        let target = match owner {
            CallableOwner::Module(_) => return Some(()),
            CallableOwner::Construction(id) => declarations.constructions().get(id)?.target(),
            CallableOwner::Instance(id) => declarations.instances().get(id)?.target(),
            CallableOwner::Conformance(id) => declarations.conformances().get(id)?.target(),
            CallableOwner::Interface(id) => {
                let declaration = declarations.interfaces().get(id)?;
                self.exported_name(ExportedEntity::Interface(id), declaration.name())?;
                self.output.push('.');
                return Some(());
            }
        };
        self.ty(target)?;
        self.output.push('.');
        Some(())
    }

    fn receiver(&mut self, role: ParameterRole, owner: CallableOwner) -> Option<()> {
        let ParameterRole::Receiver(capability) = role else {
            return None;
        };
        match capability {
            CallableCapability::Readonly => self.output.push('&'),
            CallableCapability::ReadWrite => self.output.push_str("&+"),
            CallableCapability::Owned => {}
        }
        match owner {
            CallableOwner::Instance(id) => {
                self.ty(self.graph.declarations().instances().get(id)?.target())
            }
            CallableOwner::Conformance(id) => {
                self.ty(self.graph.declarations().conformances().get(id)?.target())
            }
            CallableOwner::Interface(id) => {
                self.output
                    .push_str(self.symbol(self.graph.declarations().interfaces().get(id)?.name())?);
                Some(())
            }
            CallableOwner::Construction(_) | CallableOwner::Module(_) => None,
        }
    }

    fn generic_parameters(
        &mut self,
        parameters: &[nocter_model::GenericParameterId],
    ) -> Option<()> {
        if parameters.is_empty() {
            return Some(());
        }
        self.output.push('<');
        for (index, id) in parameters.iter().copied().enumerate() {
            if index != 0 {
                self.output.push_str(", ");
            }
            if let Some(argument) = self.generics.and_then(|arguments| arguments.get(id)) {
                self.ty(argument)?;
            } else {
                let parameter = self.graph.declarations().generic_parameters().get(id)?;
                self.output.push_str(self.symbol(parameter.name())?);
            }
        }
        self.output.push('>');
        Some(())
    }

    fn parameters(&mut self, parameters: &[nocter_model::ParameterId]) -> Option<()> {
        self.output.push('(');
        for (index, id) in parameters.iter().copied().enumerate() {
            if index != 0 {
                self.output.push_str(", ");
            }
            let start = self.output.len();
            self.parameter(id)?;
            self.record_parameter(start);
        }
        self.output.push(')');
        Some(())
    }

    fn parameter(&mut self, id: nocter_model::ParameterId) -> Option<()> {
        let parameter = self.graph.declarations().parameters().get(id)?;
        if let ParameterRole::ArgumentPack { .. } = parameter.role() {
            self.output.push_str("...");
        }
        self.output.push_str(self.symbol(parameter.name())?);
        self.output.push_str(": ");
        self.ty(parameter.ty())
    }

    fn visibility(&mut self, site: nocter_model::DeclarationSiteId) -> Option<()> {
        let site = self.graph.declaration_sites().get(site)?;
        match site.visibility() {
            Visibility::Private => {}
            Visibility::Public => self.output.push_str("pub "),
            Visibility::Package(_) => self.output.push_str("pub(/) "),
            Visibility::Descendants(boundary) => {
                let current = self.graph.modules().get(site.module())?;
                let boundary = self.graph.modules().get(boundary)?;
                if current.package() != boundary.package()
                    || !boundary.path().is_ancestor_of(current.path())
                {
                    return None;
                }
                self.output.push_str("pub(");
                let parents = current.path().segments().len() - boundary.path().segments().len();
                if parents == 0 {
                    self.output.push_str("./");
                } else {
                    for _ in 0..parents {
                        self.output.push_str("../");
                    }
                }
                self.output.push_str(") ");
            }
        }
        Some(())
    }

    fn provenance(&mut self, callable: &nocter_declarations::CallableDeclaration) -> Option<()> {
        let nocter_declarations::ProvenanceAnnotation::Explicit { includes_static } =
            callable.provenance_annotation()
        else {
            return Some(());
        };
        let origins = callable.provenance().declared_origins()?;
        self.output.push_str(" from ");
        if includes_static {
            self.output.push_str("static");
        }
        for origin in origins {
            if includes_static || !self.output.ends_with(" from ") {
                self.output.push_str(" | ");
            }
            match origin {
                nocter_declarations::ProvenanceOrigin::Receiver => self.output.push_str("self"),
                nocter_declarations::ProvenanceOrigin::Parameter(id) => {
                    let parameter = self.graph.declarations().parameters().get(*id)?;
                    self.output.push_str(self.symbol(parameter.name())?);
                }
            }
        }
        Some(())
    }

    fn requirements(&mut self, requirements: &[nocter_model::RequirementId]) -> Option<()> {
        if requirements.is_empty() {
            return Some(());
        }
        self.output.push_str(" where ");
        let mut index = 0;
        while index < requirements.len() {
            if index != 0 {
                self.output.push_str(", ");
            }
            let requirement = self
                .graph
                .declarations()
                .requirements()
                .get(requirements[index])?;
            let RequirementKind::Capability {
                subject,
                capability,
            } = requirement.kind()
            else {
                self.requirement(requirement.kind())?;
                index += 1;
                continue;
            };
            self.requirement_subject(*subject)?;
            self.output.push_str(": ");
            self.structural_capability(capability)?;
            index += 1;
            while index < requirements.len() {
                let next = self
                    .graph
                    .declarations()
                    .requirements()
                    .get(requirements[index])?;
                let RequirementKind::Capability {
                    subject: next_subject,
                    capability: next_capability,
                } = next.kind()
                else {
                    break;
                };
                if next_subject != subject {
                    break;
                }
                self.output.push_str(" + ");
                self.structural_capability(next_capability)?;
                index += 1;
            }
        }
        Some(())
    }

    fn requirement(&mut self, requirement: &RequirementKind) -> Option<()> {
        match requirement {
            RequirementKind::Capability {
                subject,
                capability,
            } => {
                self.requirement_subject(*subject)?;
                self.output.push_str(": ");
                self.structural_capability(capability)?;
            }
            RequirementKind::Copy(parameter) => {
                self.output.push_str("copy ");
                self.generic_parameter(*parameter)?;
            }
            RequirementKind::TypeEquality { left, right } => {
                self.ty(*left)?;
                self.output.push_str(" = ");
                self.ty(*right)?;
            }
            RequirementKind::BinderRefinement {
                parameter,
                replacement,
            } => {
                self.generic_parameter(*parameter)?;
                self.output.push_str(" = ");
                self.ty(*replacement)?;
            }
            RequirementKind::Equality { operand } | RequirementKind::Ordering { operand } => {
                self.output.push_str("(&");
                self.generic_parameter(*operand)?;
                self.output
                    .push_str(if matches!(requirement, RequirementKind::Equality { .. }) {
                        " == &"
                    } else {
                        " < &"
                    });
                self.generic_parameter(*operand)?;
                self.output.push_str("): bool");
            }
            RequirementKind::Index {
                capability,
                container,
                index,
                result,
            } => {
                self.output.push('(');
                self.output.push_str(match capability {
                    BorrowCapability::Readonly => "&",
                    BorrowCapability::ReadWrite => "&+",
                });
                self.generic_parameter(*container)?;
                self.output.push('[');
                self.ty(*index)?;
                self.output.push_str("]): ");
                self.ty(*result)?;
            }
            RequirementKind::Coercion { source, target } => {
                self.ty(*source)?;
                self.output.push_str(" as ");
                self.ty(*target)?;
            }
            RequirementKind::Expansion {
                capability,
                source,
                result,
            } => {
                self.output.push_str("(...");
                self.output.push_str(match capability {
                    ExpansionCapability::Readonly => "&",
                    ExpansionCapability::ReadWrite => "&+",
                    ExpansionCapability::Owned => "",
                });
                self.generic_parameter(*source)?;
                self.output.push_str("): ");
                self.ty(*result)?;
            }
        }
        Some(())
    }

    fn structural_capability(&mut self, capability: &StructuralCapability) -> Option<()> {
        match capability {
            StructuralCapability::Interface(application) => {
                let declaration = self
                    .graph
                    .declarations()
                    .interfaces()
                    .get(application.interface())?;
                self.exported_name(
                    ExportedEntity::Interface(application.interface()),
                    declaration.name(),
                )?;
                self.type_arguments(application.arguments())?;
            }
            StructuralCapability::Callable(contract) => {
                self.callable_contract(contract)?;
            }
        }
        Some(())
    }

    fn requirement_subject(&mut self, subject: RequirementSubject) -> Option<()> {
        match subject {
            RequirementSubject::GenericParameter(parameter) => self.generic_parameter(parameter),
            RequirementSubject::AssociatedType(associated) => {
                let declaration = self
                    .graph
                    .declarations()
                    .associated_types()
                    .get(associated)?;
                self.output.push_str("Self.");
                self.output.push_str(self.symbol(declaration.name())?);
                Some(())
            }
        }
    }

    fn generic_parameter(&mut self, id: nocter_model::GenericParameterId) -> Option<()> {
        if let Some(argument) = self.generics.and_then(|arguments| arguments.get(id)) {
            return self.ty(argument);
        }
        let parameter = self.graph.declarations().generic_parameters().get(id)?;
        self.output.push_str(self.symbol(parameter.name())?);
        Some(())
    }

    fn ty(&mut self, ty: TypeId) -> Option<()> {
        if self.self_type == Some(ty) {
            self.output.push_str("Self");
            return Some(());
        }
        match self.types.get(ty)? {
            TypeKind::Builtin(builtin) => self.output.push_str(builtin_spelling(*builtin)),
            TypeKind::GenericParameter(id) => {
                if let Some(argument) = self.generics.and_then(|arguments| arguments.get(*id))
                    && argument != ty
                {
                    self.ty(argument)?;
                } else {
                    let parameter = self.graph.declarations().generic_parameters().get(*id)?;
                    self.output.push_str(self.symbol(parameter.name())?);
                }
            }
            TypeKind::InterfaceSelf(id) => {
                let declaration = self.graph.declarations().interfaces().get(*id)?;
                self.exported_name(ExportedEntity::Interface(*id), declaration.name())?;
            }
            TypeKind::Nominal {
                definition,
                arguments,
            } => {
                let declaration = self.graph.declarations().nominal_types().get(*definition)?;
                self.exported_name(ExportedEntity::NominalType(*definition), declaration.name())?;
                self.type_arguments(arguments)?;
            }
            TypeKind::AssociatedProjection { base, associated } => {
                self.ty(*base)?;
                self.output.push('.');
                let declaration = self
                    .graph
                    .declarations()
                    .associated_types()
                    .get(*associated)?;
                self.output.push_str(self.symbol(declaration.name())?);
            }
            TypeKind::Opaque {
                definition,
                arguments,
            } => {
                let declaration = self.graph.declarations().opaque_types().get(*definition)?;
                self.output.push_str("some ");
                let interface = self
                    .graph
                    .declarations()
                    .interfaces()
                    .get(declaration.interface().interface())?;
                self.exported_name(
                    ExportedEntity::Interface(declaration.interface().interface()),
                    interface.name(),
                )?;
                self.type_arguments(arguments)?;
            }
            TypeKind::Pointer(pointee) => {
                self.output.push('*');
                self.prefix_type(*pointee)?;
            }
            TypeKind::Borrow {
                capability,
                referent,
            } => {
                self.output.push_str(match capability {
                    BorrowCapability::Readonly => "&",
                    BorrowCapability::ReadWrite => "&+",
                });
                self.prefix_type(*referent)?;
            }
            TypeKind::Slice(element) => {
                self.output.push('[');
                self.ty(*element)?;
                self.output.push(']');
            }
            TypeKind::FixedArray { element, length } => {
                self.output.push('[');
                self.ty(*element)?;
                write!(self.output, "; {length}]").ok()?;
            }
            TypeKind::Closure { .. } => self.output.push_str("closure"),
            TypeKind::Callable(contract) => {
                self.callable_contract(contract)?;
            }
            TypeKind::Optional(payload) => {
                self.ty(*payload)?;
                self.output.push('?');
            }
            TypeKind::Fallible(payload) => {
                self.ty(*payload)?;
                self.output.push('!');
            }
        }
        Some(())
    }

    fn callable_contract(&mut self, contract: &nocter_model::CallableContract) -> Option<()> {
        self.output.push_str(match contract.capability() {
            CallableCapability::Readonly => "&func",
            CallableCapability::ReadWrite => "&+func",
            CallableCapability::Owned => "func",
        });
        self.output.push('(');
        let named = !contract.provenance().origins().is_empty();
        for (index, parameter) in contract.parameters().iter().copied().enumerate() {
            if index != 0 {
                self.output.push_str(", ");
            }
            let start = self.output.len();
            if named {
                write!(self.output, "p{index}: ").ok()?;
            }
            self.ty(parameter)?;
            self.record_parameter(start);
        }
        if let Some(pack) = contract.pack() {
            if !contract.parameters().is_empty() {
                self.output.push_str(", ");
            }
            let start = self.output.len();
            self.output.push_str("...");
            if named {
                write!(self.output, "p{}: ", contract.parameters().len()).ok()?;
            }
            self.ty(pack)?;
            self.record_parameter(start);
        }
        self.output.push_str("): ");
        self.ty(contract.result())?;
        if named {
            self.output.push_str(" from ");
            for (index, origin) in contract.provenance().origins().iter().enumerate() {
                if index != 0 {
                    self.output.push_str(" | ");
                }
                write!(self.output, "p{}", origin.position()).ok()?;
            }
        }
        Some(())
    }

    fn record_parameter(&mut self, start: usize) {
        if self.record_parameters {
            self.parameter_ranges.push((start, self.output.len()));
        }
    }

    fn prefix_type(&mut self, id: TypeId) -> Option<()> {
        let grouped = matches!(
            self.types.get(id)?,
            TypeKind::Optional(_) | TypeKind::Fallible(_)
        );
        if grouped {
            self.output.push('(');
        }
        self.ty(id)?;
        if grouped {
            self.output.push(')');
        }
        Some(())
    }

    fn type_arguments(&mut self, arguments: &[TypeId]) -> Option<()> {
        if arguments.is_empty() {
            return Some(());
        }
        self.output.push('<');
        for (index, argument) in arguments.iter().copied().enumerate() {
            if index != 0 {
                self.output.push_str(", ");
            }
            self.ty(argument)?;
        }
        self.output.push('>');
        Some(())
    }

    fn symbol(&self, symbol: Symbol) -> Option<&'a str> {
        self.graph.symbols().spelling(symbol)
    }

    fn exported_name(&mut self, entity: ExportedEntity, fallback: Symbol) -> Option<()> {
        if self.visible_name(entity)? {
            return Some(());
        }
        self.output.push_str(self.symbol(fallback)?);
        Some(())
    }

    fn declaration_pattern_name(&mut self, entity: ExportedEntity, fallback: Symbol) -> Option<()> {
        if let Some([name]) = self.spellings.get(entity) {
            self.output.push_str(self.graph.symbols().spelling(*name)?);
        } else {
            self.output.push_str(self.symbol(fallback)?);
        }
        Some(())
    }

    fn declaration_type_pattern(&mut self, ty: TypeId) -> Option<()> {
        let TypeKind::Nominal {
            definition,
            arguments,
        } = self.types.get(ty)?
        else {
            return None;
        };
        let declaration = self.graph.declarations().nominal_types().get(*definition)?;
        self.declaration_pattern_name(
            ExportedEntity::NominalType(*definition),
            declaration.name(),
        )?;
        self.type_arguments(arguments)
    }

    fn visible_name(&mut self, entity: ExportedEntity) -> Option<bool> {
        let Some(spelling) = self.spellings.get(entity) else {
            return Some(false);
        };
        for (index, segment) in spelling.iter().copied().enumerate() {
            if index != 0 {
                self.output.push('.');
            }
            self.output
                .push_str(self.graph.symbols().spelling(segment)?);
        }
        Some(true)
    }
}

fn write_string_literal(output: &mut String, value: &str) -> fmt::Result {
    output.push('"');
    for character in value.chars() {
        match character {
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            '\0' => output.push_str("\\0"),
            '\\' => output.push_str("\\\\"),
            '"' => output.push_str("\\\""),
            '$' => output.push_str("\\$"),
            character if character.is_ascii_control() => {
                write!(output, "\\x{:02X}", u32::from(character))?;
            }
            character => output.write_char(character)?,
        }
    }
    output.push('"');
    Ok(())
}

const fn builtin_spelling(builtin: BuiltinType) -> &'static str {
    match builtin {
        BuiltinType::Bool => "bool",
        BuiltinType::I8 => "i8",
        BuiltinType::I16 => "i16",
        BuiltinType::I32 => "i32",
        BuiltinType::I64 => "i64",
        BuiltinType::U8 => "u8",
        BuiltinType::U16 => "u16",
        BuiltinType::U32 => "u32",
        BuiltinType::U64 => "u64",
        BuiltinType::Usize => "usize",
        BuiltinType::Isize => "isize",
        BuiltinType::Str => "str",
        BuiltinType::Error => "error",
        BuiltinType::Void => "void",
        BuiltinType::Never => "never",
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn constant_text_presentation_is_valid_nocter_source() {
        let mut output = String::new();
        super::write_string_literal(&mut output, "line\n\u{7f}\"${値}").unwrap();
        assert_eq!(output, "\"line\\n\\x7F\\\"\\${値}\"");
    }
}
