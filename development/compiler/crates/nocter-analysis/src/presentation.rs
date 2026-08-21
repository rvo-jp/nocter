use std::fmt::Write;

use nocter_checking::CheckedProgram;
use nocter_declarations::{
    CallableKind, CallableOwner, DeclarationGraph, ExpansionCapability, NominalShape,
    ParameterRole, RequirementKind, RequirementSubject, StructuralCapability, Visibility,
};
use nocter_model::{
    BorrowCapability, BuiltinType, CallableCapability, Symbol, TypeId, TypeKind, TypeStore,
};
use nocter_source_index::SemanticEntity;

/// Canonical source-language presentation derived from checked semantics, never source slicing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticPresentation {
    code: Box<str>,
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
) -> Option<SemanticPresentation> {
    let graph = checked.graph();
    let mut renderer = Renderer::new(graph, checked.types());
    renderer.entity(checked, entity)?;
    Some(SemanticPresentation {
        code: renderer.output.into_boxed_str(),
    })
}

struct Renderer<'a> {
    graph: &'a DeclarationGraph,
    types: &'a TypeStore,
    output: String,
}

impl<'a> Renderer<'a> {
    const fn new(graph: &'a DeclarationGraph, types: &'a TypeStore) -> Self {
        Self {
            graph,
            types,
            output: String::new(),
        }
    }

    fn entity(&mut self, checked: &CheckedProgram, entity: SemanticEntity) -> Option<()> {
        match entity {
            SemanticEntity::Module(_) => {
                self.workspace_entity(entity)?;
            }
            SemanticEntity::NominalType(_)
            | SemanticEntity::TypeAlias(_)
            | SemanticEntity::Interface(_)
            | SemanticEntity::AssociatedType(_) => self.type_entity(entity)?,
            SemanticEntity::Callable(_)
            | SemanticEntity::Field(_)
            | SemanticEntity::BuiltinField(_)
            | SemanticEntity::Variant(_) => self.member_entity(entity)?,
            SemanticEntity::GenericParameter(_)
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
                write!(
                    self.output,
                    "{keyword} {}",
                    self.symbol(declaration.name())?
                )
                .ok()?;
                self.generic_parameters(declaration.generic_parameters())?;
                self.requirements(declaration.requirements())?;
            }
            SemanticEntity::TypeAlias(id) => {
                let declaration = declarations.type_aliases().get(id)?;
                self.visibility(declaration.site())?;
                write!(self.output, "type {}", self.symbol(declaration.name())?).ok()?;
                self.generic_parameters(declaration.generic_parameters())?;
                self.output.push_str(" = ");
                self.ty(declaration.target())?;
                self.requirements(declaration.requirements())?;
            }
            SemanticEntity::Interface(id) => {
                let declaration = declarations.interfaces().get(id)?;
                self.visibility(declaration.site())?;
                write!(
                    self.output,
                    "interface {}",
                    self.symbol(declaration.name())?
                )
                .ok()?;
                self.generic_parameters(declaration.generic_parameters())?;
                self.requirements(declaration.requirements())?;
            }
            SemanticEntity::AssociatedType(id) => {
                let declaration = declarations.associated_types().get(id)?;
                let owner = declarations.interfaces().get(declaration.interface())?;
                self.visibility(declaration.site())?;
                write!(
                    self.output,
                    "type {}.{}",
                    self.symbol(owner.name())?,
                    self.symbol(declaration.name())?
                )
                .ok()?;
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
                write!(
                    self.output,
                    "field {}.{}: ",
                    self.symbol(owner.name())?,
                    self.symbol(field.name())?
                )
                .ok()?;
                self.ty(field.ty())?;
            }
            SemanticEntity::BuiltinField(field) => self.output.push_str(match field {
                nocter_model::BuiltinField::ErrorCode => "field error.code: &str",
                nocter_model::BuiltinField::ErrorMessage => "field error.message: &str",
            }),
            SemanticEntity::Variant(id) => {
                let variant = declarations.variants().get(id)?;
                let owner = declarations.nominal_types().get(variant.owner())?;
                self.visibility(variant.site())?;
                write!(
                    self.output,
                    "variant {}.{}",
                    self.symbol(owner.name())?,
                    self.symbol(variant.name())?
                )
                .ok()?;
                self.parameters(variant.payload())?;
            }
            _ => return None,
        }
        Some(())
    }

    fn value_entity(&mut self, checked: &CheckedProgram, entity: SemanticEntity) -> Option<()> {
        let declarations = self.graph.declarations();
        match entity {
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
                let local = checked.bodies().get(body)?.locals().get(id)?;
                write!(
                    self.output,
                    "let {}: ",
                    self.symbol(local.declaration().name())?
                )
                .ok()?;
                self.ty(local.ty())?;
            }
            SemanticEntity::Capture(body, id) => {
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
                write!(self.output, "{}.", self.symbol(declaration.name())?).ok()?;
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
            let parameter = self.graph.declarations().generic_parameters().get(id)?;
            self.output.push_str(self.symbol(parameter.name())?);
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
            self.parameter(id)?;
        }
        self.output.push(')');
        Some(())
    }

    fn parameter(&mut self, id: nocter_model::ParameterId) -> Option<()> {
        let parameter = self.graph.declarations().parameters().get(id)?;
        if let ParameterRole::Ordinary { variadic: true, .. } = parameter.role() {
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
                self.output.push_str(self.symbol(declaration.name())?);
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
        let parameter = self.graph.declarations().generic_parameters().get(id)?;
        self.output.push_str(self.symbol(parameter.name())?);
        Some(())
    }

    fn ty(&mut self, id: TypeId) -> Option<()> {
        match self.types.get(id)? {
            TypeKind::Builtin(builtin) => self.output.push_str(builtin_spelling(*builtin)),
            TypeKind::GenericParameter(id) => {
                let parameter = self.graph.declarations().generic_parameters().get(*id)?;
                self.output.push_str(self.symbol(parameter.name())?);
            }
            TypeKind::InterfaceSelf(id) => {
                let declaration = self.graph.declarations().interfaces().get(*id)?;
                self.output.push_str(self.symbol(declaration.name())?);
            }
            TypeKind::Nominal {
                definition,
                arguments,
            } => {
                let declaration = self.graph.declarations().nominal_types().get(*definition)?;
                self.output.push_str(self.symbol(declaration.name())?);
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
                self.output.push_str(self.symbol(interface.name())?);
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
            if named {
                write!(self.output, "p{index}: ").ok()?;
            }
            self.ty(parameter)?;
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
