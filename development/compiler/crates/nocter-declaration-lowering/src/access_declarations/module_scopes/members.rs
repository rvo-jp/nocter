//! Type-owned declarations bind to the nominal visible at the physical `impl` occurrence.
use super::{ModuleSourcesFailure, PairedModuleSources, integrity, symbol, syntax};
use crate::access_declarations::{
    Failure, TypeSyntax,
    headers::{self, Header, ParameterHeader},
};
use crate::{SurfaceDeclarationId, SurfaceDeclarationKind, SurfaceSourceId};
use nocter_declarations::value_access::{
    DataReference, DeclarationId, Reservations, ScopeId, ValueDefinition,
};
use nocter_model::{AccessCodeId, NominalTypeId, Symbol};
use nocter_syntax::{
    BoundSyntax, NodeId, NodeKind, direct_identifier, direct_node, type_behavior_owner,
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug)]
pub(super) struct MemberOccurrence {
    pub record: NominalTypeId,
    pub nominal_contract: SurfaceDeclarationId,
    pub lookup: ScopeId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TypeMemberKind {
    Construction,
    Method,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TypeMember {
    pub code: AccessCodeId,
    pub kind: TypeMemberKind,
    pub accessible: bool,
}

#[derive(Debug)]
pub(super) struct Members {
    pub occurrences: BTreeMap<SurfaceDeclarationId, MemberOccurrence>,
    names: BTreeMap<(NominalTypeId, Symbol), SurfaceDeclarationId>,
}

impl Members {
    pub fn index(
        paired: &PairedModuleSources<'_>,
        occurrences: BTreeMap<SurfaceDeclarationId, MemberOccurrence>,
    ) -> Result<Self, ModuleSourcesFailure> {
        let inventory = &paired.inventory;
        let mut names = BTreeMap::new();
        for (&id, member) in &occurrences {
            let declaration = inventory.declarations()[id.index()];
            let canonical = paired.contracts.representative(id);
            if canonical != id {
                if occurrences
                    .get(&canonical)
                    .is_none_or(|contract| contract.record != member.record)
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
            let token = declaration.name().ok_or_else(integrity)?;
            let name = symbol(paired, declaration.source(), token)?;
            if let Some(first) = names.insert((member.record, name), id) {
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
        Ok(Self { occurrences, names })
    }

    pub fn constrain_audiences(
        &self,
        paired: &mut PairedModuleSources<'_>,
    ) -> Result<(), ModuleSourcesFailure> {
        for (&id, member) in &self.occurrences {
            let inventory = &paired.inventory;
            if inventory.audience(id).is_some()
                && inventory.audience(member.nominal_contract).is_none()
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
                .constrain_member(id, member.nominal_contract)
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
        if result_owner(result) == Some(member.record) {
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
        owner: NominalTypeId,
        name: Symbol,
    ) -> Option<TypeMember> {
        let &id = self.names.get(&(owner, name))?;
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
            _ => return None,
        };
        Some(TypeMember {
            code,
            kind,
            accessible: visible,
        })
    }
}

pub(super) fn method_header(
    syntax: BoundSyntax<'_>,
    owner: NodeId,
    declaration: NodeId,
) -> Result<Header, Failure> {
    let mut header = headers::function(syntax, declaration)?;
    let tree = syntax.tree();
    let receiver =
        direct_node(tree, declaration, NodeKind::Receiver).ok_or(Failure::InconsistentSyntax)?;
    let name = direct_identifier(tree, receiver).ok_or(Failure::InconsistentSyntax)?;
    let operation = crate::access_declarations::input_operation(syntax, receiver)?;
    let (ty, owner_name) = type_behavior_owner(tree, owner).ok_or(Failure::InconsistentSyntax)?;
    header.parameters.insert(
        0,
        ParameterHeader {
            name,
            data: TypeSyntax::Named(Box::new([owner_name])),
            ty,
            operation,
        },
    );
    Ok(header)
}

fn result_owner(result: &ValueDefinition) -> Option<NominalTypeId> {
    let ValueDefinition::Data(data) = result else {
        return None;
    };
    let mut data = data;
    loop {
        data = match data {
            DataReference::Record(record) => return Some(*record),
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
            .insert((record, contract));
    }
    let self_name = inventory.surface.symbols().get("Self");
    let mut scopes_by_record = BTreeMap::new();
    let mut owners = BTreeMap::new();
    let mut members = BTreeMap::new();
    for (raw, declaration) in inventory.declaration_entries() {
        let source = declaration.source();
        match declaration.kind() {
            SurfaceDeclarationKind::TypeBehavior => {
                let bound = syntax(paired, source)?;
                let (pattern, name) =
                    type_behavior_owner(bound.tree(), declaration.node()).ok_or_else(integrity)?;
                let name = symbol(paired, source, name)?;
                let mut selected = BTreeSet::new();
                for visible in super::super::module_sources::visible_sources(paired, source) {
                    selected.extend(records.get(&(visible, name)).into_iter().flatten().copied());
                }
                if selected.len() != 1 {
                    return Err(ModuleSourcesFailure::Access(
                        super::super::source_roles::role_failure(
                            bound,
                            super::super::SourceRoleRule::TypeBehaviorOwner,
                            pattern,
                        ),
                    ));
                }
                let (record, nominal_contract) = selected.pop_first().ok_or_else(integrity)?;
                let lookup = match scopes_by_record.entry((source, record)) {
                    std::collections::btree_map::Entry::Occupied(entry) => *entry.get(),
                    std::collections::btree_map::Entry::Vacant(entry) => {
                        let lookup = reservations
                            .new_lexical_scope(scopes[source.index()])
                            .map_err(|_| integrity())?;
                        if let Some(name) = self_name {
                            reservations
                                .bind(lookup, name, record)
                                .map_err(|_| integrity())?;
                        }
                        *entry.insert(lookup)
                    }
                };
                owners.insert(
                    raw,
                    MemberOccurrence {
                        record,
                        nominal_contract,
                        lookup,
                    },
                );
            }
            SurfaceDeclarationKind::ConstructionFunction
            | SurfaceDeclarationKind::InherentMethod => {
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
