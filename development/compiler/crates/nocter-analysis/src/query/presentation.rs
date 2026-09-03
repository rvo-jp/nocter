use std::fmt::{self, Write};

use nocter_checking::{
    CheckedPredicate, CheckedProgram, GenericArguments, LocalBindingKind,
    RequiredInterfaceImplementationMethod,
};
use nocter_declarations::{
    AssociatedTypeBinding, CallableKind, CallableOwner, DeclarationGraph, ExpansionCapability,
    ExportedEntity, InterfaceApplication, NominalShape, ParameterRole, RequirementKind,
    RequirementSubject, Visibility,
};
use nocter_model::{BorrowCapability, CallableCapability, Symbol, TypeId, TypeKind, TypeStore};
use nocter_source_index::SemanticEntity;

mod signature;
pub(in crate::query) mod visible_spelling;

pub(super) use signature::{
    StaticSignatureSource, closure_signature_presentation, static_signature_presentation,
};

/// Canonical source-language presentation derived from checked semantics, never source slicing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticPresentation {
    code: Box<str>,
}

/// An internal inconsistency while rendering checked semantic data.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PresentationError {
    Evidence(crate::EvidenceIntegrityError),
    InvalidEntity(SemanticEntity),
    InvalidNominalPresentation(nocter_model::NominalTypeId),
    SourceVisibility,
}

impl fmt::Display for PresentationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Evidence(error) => error.fmt(formatter),
            Self::InvalidEntity(entity) => {
                write!(formatter, "cannot render semantic entity {entity:?}")
            }
            Self::InvalidNominalPresentation(nominal) => {
                write!(
                    formatter,
                    "cannot render nominal hover presentation for {nominal:?}"
                )
            }
            Self::SourceVisibility => formatter
                .write_str("semantic presentation has an invalid source visibility context"),
        }
    }
}

impl std::error::Error for PresentationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Evidence(error) => Some(error),
            Self::InvalidEntity(_)
            | Self::InvalidNominalPresentation(_)
            | Self::SourceVisibility => None,
        }
    }
}

impl From<crate::EvidenceIntegrityError> for PresentationError {
    fn from(error: crate::EvidenceIntegrityError) -> Self {
        Self::Evidence(error)
    }
}

impl From<nocter_checking::SourceVisibilityError> for PresentationError {
    fn from(_: nocter_checking::SourceVisibilityError) -> Self {
        Self::SourceVisibility
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
    renderer.entity(checked_body(checked, entity), entity)?;
    Some(SemanticPresentation {
        code: renderer.output.into_boxed_str(),
    })
}

pub(super) fn type_presentation_with_spellings(
    graph: &DeclarationGraph,
    types: &TypeStore,
    ty: TypeId,
    spellings: &visible_spelling::VisibleSpellings,
) -> Option<SemanticPresentation> {
    let mut renderer = Renderer::new(graph, types, spellings);
    renderer.ty(ty)?;
    Some(SemanticPresentation {
        code: renderer.output.into_boxed_str(),
    })
}

pub(super) fn recovery_type_presentation(
    projection: &nocter_model::TypeProjection,
    graph: &DeclarationGraph,
    spellings: &visible_spelling::VisibleSpellings,
) -> Option<SemanticPresentation> {
    let mut renderer = Renderer::new(graph, projection.types(), spellings);
    renderer.ty(projection.root())?;
    Some(SemanticPresentation {
        code: renderer.output.into_boxed_str(),
    })
}

pub(super) fn hover_presentation(
    checked: &CheckedProgram,
    entity: SemanticEntity,
    spellings: &visible_spelling::VisibleSpellings,
    source: nocter_source::SourceId,
) -> Result<SemanticPresentation, PresentationError> {
    let graph = checked.graph();
    let source_access = checked.source_access_context(source)?;
    let mut renderer = Renderer::new(graph, checked.types(), spellings);
    renderer
        .entity(checked_body(checked, entity), entity)
        .ok_or(PresentationError::InvalidEntity(entity))?;
    if let SemanticEntity::NominalType(nominal) = entity {
        renderer
            .nominal_hover_shape(nominal, source_access)?
            .ok_or(PresentationError::InvalidNominalPresentation(nominal))?;
    }
    Ok(SemanticPresentation {
        code: renderer.output.into_boxed_str(),
    })
}

pub(super) fn evidence_presentation(
    graph: &DeclarationGraph,
    types: &TypeStore,
    body: Option<&nocter_checking::CheckedBody>,
    entity: SemanticEntity,
    spellings: &visible_spelling::VisibleSpellings,
) -> Option<SemanticPresentation> {
    let mut renderer = Renderer::new(graph, types, spellings);
    renderer.entity(body, entity)?;
    Some(SemanticPresentation {
        code: renderer.output.into_boxed_str(),
    })
}

pub(super) fn required_interface_implementation_method_presentation(
    graph: &DeclarationGraph,
    types: &TypeStore,
    required: &RequiredInterfaceImplementationMethod,
    spellings: &visible_spelling::VisibleSpellings,
) -> Option<SemanticPresentation> {
    let mut renderer = Renderer::new(graph, types, spellings);
    renderer.required_interface_implementation_method(required)?;
    Some(SemanticPresentation {
        code: renderer.output.into_boxed_str(),
    })
}

fn checked_body(
    checked: &CheckedProgram,
    entity: SemanticEntity,
) -> Option<&nocter_checking::CheckedBody> {
    match entity {
        SemanticEntity::LocalBinding(body, _)
        | SemanticEntity::Capture(body, _)
        | SemanticEntity::PlaceProjection(body, ..) => checked.bodies().get(body),
        _ => None,
    }
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

    fn entity(
        &mut self,
        body: Option<&nocter_checking::CheckedBody>,
        entity: SemanticEntity,
    ) -> Option<()> {
        match entity {
            SemanticEntity::Module(_) => {
                self.workspace_entity(entity)?;
            }
            SemanticEntity::NominalType(_)
            | SemanticEntity::BuiltinType(_)
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
            | SemanticEntity::Test(_) => self.value_entity(body, entity)?,
            SemanticEntity::PlaceProjection(_, place, projection) => {
                self.place_projection(body?, place, projection)?;
            }
            SemanticEntity::Package(_)
            | SemanticEntity::PackageTarget(_)
            | SemanticEntity::Import(_)
            | SemanticEntity::DeclarationSite(_)
            | SemanticEntity::Construction(_)
            | SemanticEntity::Instance(_)
            | SemanticEntity::InterfaceImplementation(_)
            | SemanticEntity::Drop(_)
            | SemanticEntity::Requirement(_)
            | SemanticEntity::CapabilityEvidence(_)
            | SemanticEntity::Body(_)
            | SemanticEntity::BodyScope(..)
            | SemanticEntity::BodyNode(..)
            | SemanticEntity::OpaqueType(_) => return None,
        }
        Some(())
    }

    fn place_projection(
        &mut self,
        body: &nocter_checking::CheckedBody,
        place: nocter_model::PlaceId,
        projection: usize,
    ) -> Option<()> {
        let place = body.places().get(place)?;
        let selected = place.projections().get(projection)?;
        let nocter_checking::PlaceProjection::TupleElement { index, ty } = selected else {
            return None;
        };
        let receiver = projection
            .checked_sub(1)
            .and_then(|previous| place.projections().get(previous))
            .map_or(place.root_ty(), nocter_checking::PlaceProjection::ty);
        self.ty(receiver)?;
        write!(self.output, ".{index}: ").ok()?;
        self.ty(*ty)
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
            SemanticEntity::BuiltinType(builtin) => {
                self.output.push_str("primitive type ");
                self.output.push_str(builtin.spelling());
            }
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
        body_evidence: Option<&nocter_checking::CheckedBody>,
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
                    nocter_model::ConstantValue::Character(value) => {
                        write_character_literal(&mut self.output, *value).ok()?;
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
                self.parameter_shape(parameter)?;
            }
            SemanticEntity::LocalBinding(body, id) => {
                let _ = body;
                let local = body_evidence?.locals().get(id)?;
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
                let _ = body;
                let capture = body_evidence?.captures().get(id)?;
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
        self.callable_guarantees(callable.guarantees());
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
                    nocter_declarations::LiteralShape::Mapping => "[:]",
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

    fn required_interface_implementation_method(
        &mut self,
        required: &RequiredInterfaceImplementationMethod,
    ) -> Option<()> {
        let declarations = self.graph.declarations();
        let callable = declarations.callables().get(required.interface_method())?;
        self.callable_guarantees(callable.guarantees());
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
            if parameter.argument_pack().is_some() {
                self.output.push_str("...");
            }
            let declaration = declarations.parameters().get(parameter.declaration())?;
            self.output.push_str(self.symbol(declaration.name())?);
            self.output.push_str(": ");
            if let Some(pack) = parameter.argument_pack() {
                self.argument_pack_types(pack)?;
            } else {
                self.ty(parameter.ty())?;
            }
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
            CheckedPredicate::Interface {
                subject,
                application,
                associated_types,
            } => {
                self.ty(*subject)?;
                self.output.push_str(" impl ");
                self.interface_application(application)?;
                self.associated_bindings(associated_types)?;
            }
            CheckedPredicate::Callable { subject, contract } => {
                self.ty(*subject)?;
                self.output.push_str(": ");
                self.callable_contract(contract)?;
            }
            CheckedPredicate::Copy(ty) => {
                self.output.push_str("copy ");
                self.ty(*ty)?;
            }
            CheckedPredicate::BinderRefinement {
                binder,
                replacement,
            } => {
                self.ty(*binder)?;
                self.output.push_str(" = ");
                self.ty(*replacement)?;
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

    fn nominal_hover_shape(
        &mut self,
        nominal: nocter_model::NominalTypeId,
        from: nocter_checking::SourceAccessContext<'_>,
    ) -> Result<Option<()>, nocter_checking::SourceVisibilityError> {
        let declarations = self.graph.declarations();
        let Some(nominal_declaration) = declarations.nominal_types().get(nominal) else {
            return Ok(None);
        };
        if !from.representation_is_visible(nominal)? {
            return Ok(Some(()));
        }
        match nominal_declaration.shape() {
            NominalShape::Struct { fields, .. } => {
                let mut field_declarations = Vec::with_capacity(fields.len());
                for field in fields.iter().copied() {
                    let Some(declaration) = declarations.fields().get(field) else {
                        return Ok(None);
                    };
                    if !from.site_is_visible(self.graph, declaration.site())? {
                        return Ok(Some(()));
                    }
                    field_declarations.push(declaration);
                }
                self.output.push_str(" {\n");
                for declaration in field_declarations {
                    self.output.push_str("    ");
                    if self.visibility(declaration.site()).is_none() {
                        return Ok(None);
                    }
                    let Some(name) = self.symbol(declaration.name()) else {
                        return Ok(None);
                    };
                    self.output.push_str(name);
                    self.output.push_str(": ");
                    if self.ty(declaration.ty()).is_none() {
                        return Ok(None);
                    }
                    self.output.push('\n');
                }
                self.output.push('}');
            }
            NominalShape::Enum { variants } => {
                let mut variant_declarations = Vec::with_capacity(variants.len());
                for variant in variants.iter().copied() {
                    let Some(declaration) = declarations.variants().get(variant) else {
                        return Ok(None);
                    };
                    if !from.site_is_visible(self.graph, declaration.site())? {
                        return Ok(Some(()));
                    }
                    variant_declarations.push(declaration);
                }
                self.output.push_str(" {\n");
                for declaration in variant_declarations {
                    self.output.push_str("    ");
                    let Some(name) = self.symbol(declaration.name()) else {
                        return Ok(None);
                    };
                    self.output.push_str(name);
                    if !declaration.payload().is_empty()
                        && self.parameters(declaration.payload()).is_none()
                    {
                        return Ok(None);
                    }
                    self.output.push('\n');
                }
                self.output.push('}');
            }
        }
        Ok(Some(()))
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
        self.parameter_shape(parameter)
    }

    fn parameter_shape(&mut self, parameter: &nocter_declarations::Parameter) -> Option<()> {
        if let Some(pack) = parameter.argument_pack() {
            self.argument_pack_types(pack)
        } else {
            self.ty(parameter.ty())
        }
    }

    fn argument_pack_types(&mut self, pack: nocter_model::ArgumentPackType) -> Option<()> {
        match pack {
            nocter_model::ArgumentPack::Values(element) => self.ty(element),
            nocter_model::ArgumentPack::Keyed { key, value } => {
                self.ty(key)?;
                self.output.push_str(": ");
                self.ty(value)
            }
        }
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
        for (index, requirement) in requirements.iter().enumerate() {
            if index != 0 {
                self.output.push_str(", ");
            }
            let requirement = self.graph.declarations().requirements().get(*requirement)?;
            self.requirement(requirement.kind())?;
        }
        Some(())
    }

    fn requirement(&mut self, requirement: &RequirementKind) -> Option<()> {
        match requirement {
            RequirementKind::Interface {
                subject,
                application,
                associated_types,
            } => {
                self.requirement_subject(*subject)?;
                self.output.push_str(" impl ");
                self.interface_application(application)?;
                self.associated_bindings(associated_types)?;
            }
            RequirementKind::Callable { subject, contract } => {
                self.generic_parameter(*subject)?;
                self.output.push_str(": ");
                self.callable_contract(contract)?;
            }
            RequirementKind::Copy(parameter) => {
                self.output.push_str("copy ");
                self.generic_parameter(*parameter)?;
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
                self.ty(*operand)?;
                self.output
                    .push_str(if matches!(requirement, RequirementKind::Equality { .. }) {
                        " == &"
                    } else {
                        " < &"
                    });
                self.ty(*operand)?;
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
                self.ty(*container)?;
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
                self.ty(*source)?;
                self.output.push_str("): ");
                self.ty(*result)?;
            }
        }
        Some(())
    }

    fn interface_application(&mut self, application: &InterfaceApplication) -> Option<()> {
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
        Some(())
    }

    fn associated_bindings(&mut self, bindings: &[AssociatedTypeBinding]) -> Option<()> {
        if bindings.is_empty() {
            return Some(());
        }
        self.output.push_str(" { ");
        for (index, binding) in bindings.iter().enumerate() {
            if index != 0 {
                self.output.push_str(", ");
            }
            let declaration = self
                .graph
                .declarations()
                .associated_types()
                .get(binding.declaration())?;
            self.output.push('.');
            self.output.push_str(self.symbol(declaration.name())?);
            self.output.push_str(" = ");
            self.ty(binding.ty())?;
        }
        self.output.push_str(" }");
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
            RequirementSubject::InterfaceSelf(_) => {
                self.output.push_str("Self");
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
            TypeKind::Builtin(builtin) => self.output.push_str(builtin.spelling()),
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
            TypeKind::Tuple(elements) => self.tuple_type(elements)?,
            TypeKind::Closure { .. } => self.output.push_str("closure"),
            TypeKind::Callable(contract) => {
                self.callable_contract(contract)?;
            }
            // Pack entries are compiler-owned ABI elements and cannot be named in source.
            TypeKind::PackEntry { .. } => return None,
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

    fn tuple_type(&mut self, elements: &nocter_model::TupleElements) -> Option<()> {
        self.output.push('(');
        for (index, element) in elements.iter().enumerate() {
            if index != 0 {
                self.output.push_str(", ");
            }
            self.ty(element)?;
        }
        self.output.push(')');
        Some(())
    }

    fn callable_contract(&mut self, contract: &nocter_model::CallableContract) -> Option<()> {
        self.callable_guarantees(contract.guarantees());
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
            self.ty(pack.primary())?;
            if let Some(value) = pack.value() {
                self.output.push_str(": ");
                self.ty(value)?;
            }
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

    fn callable_guarantees(&mut self, guarantees: nocter_model::CallableGuarantees) {
        if guarantees.allocation() == nocter_model::AllocationGuarantee::NoAllocation {
            self.output.push_str("noalloc ");
        }
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

fn write_character_literal(output: &mut String, value: u32) -> fmt::Result {
    output.push('\'');
    match char::from_u32(value) {
        Some('\n') => output.push_str("\\n"),
        Some('\r') => output.push_str("\\r"),
        Some('\t') => output.push_str("\\t"),
        Some('\0') => output.push_str("\\0"),
        Some('\\') => output.push_str("\\\\"),
        Some('\'') => output.push_str("\\'"),
        Some(character) if !character.is_control() => output.push(character),
        Some(_) => write!(output, "\\u{{{value:X}}}")?,
        None => return Err(fmt::Error),
    }
    output.push('\'');
    Ok(())
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
