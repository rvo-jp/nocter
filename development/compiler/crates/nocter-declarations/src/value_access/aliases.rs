//! Transparent naming metadata projected from the canonical data authority.
use super::{
    DataReference, DeclarationId, DeclarationRule, DeclarationViolation, HeaderFailure,
    HeaderPosition, ScopeId, data, records,
};
use nocter_model::{Symbol, TypeAliasId, TypeAuthority, TypeId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeAlias {
    data: TypeId,
    reference: DataReference,
}

impl TypeAlias {
    #[must_use]
    pub const fn data(&self) -> TypeId {
        self.data
    }

    pub(super) const fn reference(&self) -> &DataReference {
        &self.reference
    }
}

#[derive(Debug)]
pub(super) struct Slot {
    pub scope: ScopeId,
    pub name: Symbol,
    pub definition: Option<DataReference>,
}

impl Slot {
    pub const fn new(scope: ScopeId, name: Symbol) -> Self {
        Self {
            scope,
            name,
            definition: None,
        }
    }

    pub fn publish(
        &self,
        alias: TypeAliasId,
        records: &records::Builder,
        types: &mut TypeAuthority,
    ) -> Result<TypeAlias, HeaderFailure> {
        let violation = |rule| DeclarationViolation {
            declaration: DeclarationId::Alias(alias),
            position: HeaderPosition::Result,
            rule,
        };
        let reference = self.definition.as_ref().ok_or_else(|| {
            HeaderFailure::Rejected(vec![violation(DeclarationRule::MissingDefinition)])
        })?;
        let mut violations = Vec::new();
        if reference.has_repeated_failure() {
            violations.push(violation(DeclarationRule::RepeatedFailure));
        }
        reference.visit_leaves(|part| {
            let rule = if matches!(part, DataReference::Function(_) | DataReference::Future(_)) {
                Some(DeclarationRule::UnsupportedAliasTarget)
            } else {
                super::data_rule(part, records)
            };
            if let Some(rule) = rule {
                violations.push(violation(rule));
            }
        });
        if !violations.is_empty() {
            return Err(HeaderFailure::Rejected(violations));
        }
        let mut writer = types.transaction();
        let data = data::resolve_reference(reference, |part| match part {
            data::ReferenceNode::Data(kind) => writer.intern(kind).ok(),
            data::ReferenceNode::Function { .. } => None,
        })
        .ok_or(HeaderFailure::Inconsistent)?;
        *types = writer
            .commit(types)
            .map_err(|_| HeaderFailure::Inconsistent)?;
        Ok(TypeAlias {
            data,
            reference: reference.clone(),
        })
    }
}
