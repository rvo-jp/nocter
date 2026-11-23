//! Type-owned declarations bind to the owner authorized at the physical `impl` occurrence.
use super::{ModuleSourcesFailure, PairedModuleSources, integrity, symbol, syntax};
use crate::access_declarations::{
    Failure, TypeSyntax,
    headers::{self, Header, ParameterHeader},
};
use crate::{SurfaceDeclarationId, SurfaceDeclarationKind, SurfaceSourceId};
use nocter_declarations::value_access::{
    AggregateForm, BehaviorOwner, DataReference, DeclarationId, PayloadSchema, Program,
    Reservations, ScopeId, ValueDefinition,
};
use nocter_model::{AccessCodeId, BuiltinType, Symbol, TypeId};
use nocter_syntax::{
    BoundSyntax, NodeId, NodeKind, direct_identifier, direct_node, type_behavior_owner,
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug)]
pub(super) struct MemberOccurrence {
    pub owner: BehaviorOwner,
    pub owner_contract: SurfaceDeclarationId,
    pub lookup: ScopeId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TypeMemberKind {
    Construction,
    Method,
    Conversion,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TypeMember {
    pub code: AccessCodeId,
    pub kind: TypeMemberKind,
    pub accessible: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConversionSelection {
    Missing,
    Require(AccessCodeId),
    Selected(TypeMember),
    Ambiguous,
}

#[derive(Debug)]
pub(super) struct Members {
    pub occurrences: BTreeMap<SurfaceDeclarationId, MemberOccurrence>,
    names: BTreeMap<(BehaviorOwner, Symbol), SurfaceDeclarationId>,
    conversions: BTreeMap<BehaviorOwner, Vec<SurfaceDeclarationId>>,
}

impl Members {
    pub fn index(
        paired: &PairedModuleSources<'_>,
        occurrences: BTreeMap<SurfaceDeclarationId, MemberOccurrence>,
    ) -> Result<Self, ModuleSourcesFailure> {
        let inventory = &paired.inventory;
        let mut names = BTreeMap::new();
        let mut conversions = BTreeMap::<_, Vec<_>>::new();
        for (&id, member) in &occurrences {
            let declaration = inventory.declarations()[id.index()];
            let canonical = paired.contracts.representative(id);
            if canonical != id {
                if occurrences
                    .get(&canonical)
                    .is_none_or(|contract| contract.owner != member.owner)
                {
                    let owner = declaration.owner().ok_or_else(integrity)?;
                    let subject = inventory.declarations()[owner.index()].node();
                    return Err(ModuleSourcesFailure::Access(
                        crate::access_declarations::source_roles::role_failure(
                            syntax(paired, declaration.source())?,
                            crate::access_declarations::SourceRoleRule::TypeBehaviorOwner,
                            subject,
                        ),
                    ));
                }
                continue;
            }
            if declaration.kind() == SurfaceDeclarationKind::Coercion {
                conversions.entry(member.owner).or_default().push(id);
                continue;
            }
            let token = declaration.name().ok_or_else(integrity)?;
            let name = symbol(paired, declaration.source(), token)?;
            if let Some(first) = names.insert((member.owner, name), id) {
                let first = inventory.declarations()[first.index()]
                    .name()
                    .ok_or_else(integrity)?;
                let violation = crate::NamespaceViolation::name_collision(
                    nocter_syntax::SyntaxOrigin::Token(first),
                    nocter_syntax::SyntaxOrigin::Token(token),
                );
                let diagnostic =
                    crate::NamespaceDiagnostic::project_surface(violation, &inventory.surface)
                        .map_err(|_| integrity())?;
                return Err(ModuleSourcesFailure::Access(Failure::Namespace(Box::new(
                    diagnostic,
                ))));
            }
        }
        Ok(Self {
            occurrences,
            names,
            conversions,
        })
    }

    pub fn constrain_audiences(
        &self,
        paired: &mut PairedModuleSources<'_>,
    ) -> Result<(), ModuleSourcesFailure> {
        for (&id, member) in &self.occurrences {
            let inventory = &paired.inventory;
            if inventory.audience(id).is_some()
                && inventory.audience(member.owner_contract).is_none()
            {
                let declaration = inventory.declarations()[id.index()];
                let owner = declaration.owner().ok_or_else(integrity)?;
                return Err(ModuleSourcesFailure::Access(
                    super::super::source_roles::role_failure(
                        syntax(paired, declaration.source())?,
                        super::super::SourceRoleRule::TypeBehaviorOwner,
                        inventory.declarations()[owner.index()].node(),
                    ),
                ));
            }
            paired
                .inventory
                .audiences
                .constrain_member(id, member.owner_contract)
                .map_err(ModuleSourcesFailure::Access)?;
        }
        Ok(())
    }

    pub fn verify_result(
        &self,
        id: SurfaceDeclarationId,
        syntax: BoundSyntax<'_>,
        result_syntax: NodeId,
        result: &ValueDefinition,
    ) -> Result<(), ModuleSourcesFailure> {
        let member = self.occurrences.get(&id).ok_or_else(integrity)?;
        if result_owner(result) == Some(member.owner) {
            return Ok(());
        }
        Err(ModuleSourcesFailure::Access(
            super::super::source_roles::role_failure(
                syntax,
                super::super::SourceRoleRule::ConstructionResult,
                result_syntax,
            ),
        ))
    }

    pub fn lookup(
        &self,
        binding: &super::ModuleSourceBinding<'_>,
        caller: nocter_source::SourceId,
        owner: BehaviorOwner,
        name: Symbol,
    ) -> Option<TypeMember> {
        let &id = self.names.get(&(owner, name))?;
        Self::member(binding, caller, id)
    }

    pub fn conversion(
        &self,
        binding: &super::ModuleSourceBinding<'_>,
        caller: nocter_source::SourceId,
        program: &Program,
        owner: BehaviorOwner,
        target: TypeId,
    ) -> ConversionSelection {
        let mut pending = None;
        let mut selected = None;
        let target_schema = program.data_schema(target);
        for &id in self.conversions.get(&owner).into_iter().flatten() {
            let Some(member) = Self::member(binding, caller, id) else {
                continue;
            };
            match program.codes().get(member.code) {
                None if binding.catalog.parsed.get(&id).is_some_and(|header| {
                    let super::ParsedHeader::Code(header) = header else {
                        return false;
                    };
                    target_schema
                        .as_ref()
                        .is_none_or(|schema| could_name_target(&header.result_data, schema))
                }) =>
                {
                    pending.get_or_insert(member.code);
                }
                Some(code) if code.result() == target => {
                    if selected.replace(member).is_some() {
                        return ConversionSelection::Ambiguous;
                    }
                }
                Some(_) | None => {}
            }
        }
        // Every completed unit validates uniqueness, including entries whose target still
        // requires a constant computed through a known, exact conversion.
        selected.map_or_else(
            || pending.map_or(ConversionSelection::Missing, ConversionSelection::Require),
            ConversionSelection::Selected,
        )
    }

    pub fn verify_conversions(
        &self,
        binding: &super::ModuleSourceBinding<'_>,
        program: &Program,
    ) -> Result<(), ModuleSourcesFailure> {
        for entries in self.conversions.values() {
            let mut targets = BTreeSet::new();
            for &id in entries {
                let declaration = binding.paired.inventory.declarations()[id.index()];
                let bound = syntax(&binding.paired, declaration.source())?;
                let Some(DeclarationId::Code(code)) = binding.declarations[id.index()] else {
                    return Err(integrity());
                };
                let code = program.codes().get(code).ok_or_else(integrity)?;
                let &[receiver] = code.parameters() else {
                    return Err(integrity());
                };
                let receiver = program.parameters().get(receiver).ok_or_else(integrity)?;
                let rule = if receiver.data() == code.result() {
                    super::super::SourceRoleRule::IdentityConversion
                } else if !targets.insert(code.result()) {
                    super::super::SourceRoleRule::DuplicateConversion
                } else {
                    continue;
                };
                let result = direct_node(bound.tree(), declaration.node(), NodeKind::Type)
                    .ok_or_else(integrity)?;
                return Err(ModuleSourcesFailure::Access(
                    super::super::source_roles::role_failure(bound, rule, result),
                ));
            }
        }
        Ok(())
    }

    fn member(
        binding: &super::ModuleSourceBinding<'_>,
        caller: nocter_source::SourceId,
        id: SurfaceDeclarationId,
    ) -> Option<TypeMember> {
        let caller = binding.physical_source(caller)?;
        let inventory = &binding.paired.inventory;
        let declaration = inventory.declarations()[id.index()];
        let visible = inventory.audience(id).map_or_else(
            || {
                crate::access_declarations::module_sources::visible_sources(&binding.paired, caller)
                    .any(|source| source == declaration.source())
            },
            |audience| audience.contains(inventory.sources()[caller.index()].module()),
        );
        let DeclarationId::Code(code) = binding.declarations[id.index()]? else {
            return None;
        };
        let kind = match declaration.kind() {
            SurfaceDeclarationKind::ConstructionFunction => TypeMemberKind::Construction,
            SurfaceDeclarationKind::InherentMethod => TypeMemberKind::Method,
            SurfaceDeclarationKind::Coercion => TypeMemberKind::Conversion,
            _ => return None,
        };
        Some(TypeMember {
            code,
            kind,
            accessible: visible,
        })
    }
}

// Excluding a structurally impossible recipe never supplies a target identity. In particular,
// an unrelated fixed-array count must not become a dependency of a scalar conversion.
fn could_name_target(recipe: &TypeSyntax, target: &PayloadSchema<'_>) -> bool {
    match recipe {
        TypeSyntax::Named(_) => true,
        TypeSyntax::Callable(_) => matches!(target, PayloadSchema::Function(_)),
        TypeSyntax::Future { .. } => matches!(target, PayloadSchema::Future(_)),
        TypeSyntax::Optional { .. } => matches!(target, PayloadSchema::Optional(_)),
        TypeSyntax::Fallible { .. } => matches!(target, PayloadSchema::Fallible(_)),
        TypeSyntax::Array { .. } => matches!(target, PayloadSchema::Array(_)),
        TypeSyntax::Tuple(_) => matches!(target, PayloadSchema::Aggregate(shape)
            if matches!(shape.form(), AggregateForm::Tuple(_))),
    }
}

pub(super) fn method_header(
    syntax: BoundSyntax<'_>,
    owner: NodeId,
    declaration: NodeId,
) -> Result<Header, Failure> {
    let mut header = headers::function(syntax, declaration)?;
    header
        .parameters
        .insert(0, receiver_header(syntax, owner, declaration)?);
    Ok(header)
}

pub(super) fn conversion_header(
    syntax: BoundSyntax<'_>,
    owner: NodeId,
    declaration: NodeId,
) -> Result<Header, Failure> {
    let tree = syntax.tree();
    let name = nocter_syntax::declaration_contextual_keyword_token(tree, declaration)
        .ok_or(Failure::InconsistentSyntax)?;
    let result_syntax =
        direct_node(tree, declaration, NodeKind::Type).ok_or(Failure::InconsistentSyntax)?;
    let result_data = super::super::project_type_syntax(syntax, result_syntax)?;
    let mut pending = vec![&result_data];
    while let Some(ty) = pending.pop() {
        match ty {
            TypeSyntax::Callable(_) | TypeSyntax::Future { .. } => {
                return Err(super::super::source_roles::role_failure(
                    syntax,
                    super::super::SourceRoleRule::UnqualifiedConversionTarget,
                    result_syntax,
                ));
            }
            TypeSyntax::Optional { payload, .. } | TypeSyntax::Fallible { payload, .. } => {
                pending.push(payload);
            }
            TypeSyntax::Array { element, .. } => pending.push(element),
            TypeSyntax::Tuple(fields) => pending.extend(fields.iter()),
            TypeSyntax::Named(_) => {}
        }
    }
    Ok(Header {
        name,
        kind: headers::immediate_function(syntax, declaration),
        deferred_modifier: None,
        parameters: vec![receiver_header(syntax, owner, declaration)?],
        result_syntax,
        result_data,
        body: nocter_syntax::declaration_body(tree, declaration),
    })
}

fn receiver_header(
    syntax: BoundSyntax<'_>,
    owner: NodeId,
    declaration: NodeId,
) -> Result<ParameterHeader, Failure> {
    let tree = syntax.tree();
    let receiver =
        direct_node(tree, declaration, NodeKind::Receiver).ok_or(Failure::InconsistentSyntax)?;
    let name = direct_identifier(tree, receiver).ok_or(Failure::InconsistentSyntax)?;
    let operation = crate::access_declarations::input_operation(syntax, receiver)?;
    let (ty, owner_name) = type_behavior_owner(tree, owner).ok_or(Failure::InconsistentSyntax)?;
    Ok(ParameterHeader {
        name,
        data: TypeSyntax::Named(Box::new([owner_name])),
        ty,
        operation,
    })
}

fn result_owner(result: &ValueDefinition) -> Option<BehaviorOwner> {
    let ValueDefinition::Data(data) = result else {
        return None;
    };
    let mut data = data;
    loop {
        data = match data {
            DataReference::Record(record) => return Some(BehaviorOwner::Record(*record)),
            DataReference::Builtin(data) => return Some(BehaviorOwner::Builtin(*data)),
            DataReference::Optional { payload, .. } | DataReference::Fallible(payload) => payload,
            _ => return None,
        };
    }
}

/// Reserve one attached scope per physical source and selected nominal. Every member in that
/// source shares the same `Self` binding, while its normal names remain source-local.
pub(super) fn occurrences(
    paired: &PairedModuleSources<'_>,
    declarations: &[Option<DeclarationId>],
    scopes: &[ScopeId],
    reservations: &mut Reservations,
) -> Result<BTreeMap<SurfaceDeclarationId, MemberOccurrence>, ModuleSourcesFailure> {
    let inventory = &paired.inventory;
    if !inventory
        .declarations()
        .iter()
        .any(|declaration| declaration.kind() == SurfaceDeclarationKind::TypeBehavior)
    {
        return Ok(BTreeMap::new());
    }
    let mut records = BTreeMap::<(SurfaceSourceId, Symbol), BTreeSet<_>>::new();
    for (raw, declaration) in inventory.declaration_entries() {
        let contract = paired.contracts.representative(raw);
        let Some(DeclarationId::Record(record)) = declarations[contract.index()] else {
            continue;
        };
        let name = declaration.name().ok_or_else(integrity)?;
        let name = symbol(paired, declaration.source(), name)?;
        records
            .entry((declaration.source(), name))
            .or_default()
            .insert((BehaviorOwner::Record(record), contract));
    }
    let mut scopes_by_owner = BTreeMap::new();
    let mut owners = BTreeMap::new();
    let mut members = BTreeMap::new();
    for (raw, declaration) in inventory.declaration_entries() {
        let source = declaration.source();
        match declaration.kind() {
            SurfaceDeclarationKind::TypeBehavior => {
                let bound = syntax(paired, source)?;
                let (pattern, name) =
                    type_behavior_owner(bound.tree(), declaration.node()).ok_or_else(integrity)?;
                let spelling = bound.text_at(name.range()).ok_or_else(integrity)?;
                let selected = if let Some(builtin) = BuiltinType::from_spelling(spelling) {
                    let module = inventory.sources()[source.index()].module();
                    inventory.builtin_types.iter().find_map(|authority| {
                        let contract = authority.declaration();
                        let physical = inventory.declarations()[contract.index()].source();
                        (authority.builtin() == builtin
                            && inventory.sources()[physical.index()].module() == module)
                            .then_some((BehaviorOwner::Builtin(builtin), contract))
                    })
                } else {
                    let name = symbol(paired, source, name)?;
                    let mut seen = BTreeSet::new();
                    for visible in super::super::module_sources::visible_sources(paired, source) {
                        seen.extend(records.get(&(visible, name)).into_iter().flatten().copied());
                    }
                    (seen.len() == 1).then(|| seen.pop_first()).flatten()
                };
                let Some((owner, owner_contract)) = selected else {
                    return Err(ModuleSourcesFailure::Access(
                        super::super::source_roles::role_failure(
                            bound,
                            super::super::SourceRoleRule::TypeBehaviorOwner,
                            pattern,
                        ),
                    ));
                };
                let lookup = match scopes_by_owner.entry((source, owner)) {
                    std::collections::btree_map::Entry::Occupied(entry) => *entry.get(),
                    std::collections::btree_map::Entry::Vacant(entry) => {
                        let lookup = reservations
                            .new_behavior_scope(scopes[source.index()], owner)
                            .map_err(|_| integrity())?;
                        *entry.insert(lookup)
                    }
                };
                owners.insert(
                    raw,
                    MemberOccurrence {
                        owner,
                        owner_contract,
                        lookup,
                    },
                );
            }
            SurfaceDeclarationKind::ConstructionFunction
            | SurfaceDeclarationKind::InherentMethod
            | SurfaceDeclarationKind::Coercion => {
                let owner = declaration
                    .owner()
                    .and_then(|id| owners.get(&id))
                    .copied()
                    .ok_or_else(integrity)?;
                members.insert(raw, owner);
            }
            _ => {}
        }
    }
    Ok(members)
}
