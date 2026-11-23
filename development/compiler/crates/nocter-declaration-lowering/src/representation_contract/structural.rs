//! Structural grants close the already authored root field set before representation pairing.
use super::{field_name, is_bodyless_nominal, source_kind};
use crate::contract::bound_syntax;
use crate::{
    DeclarationContractError as Error, DeclarationSurface, ModuleSourceKind, StructuralGrantIssue,
    SurfaceDeclarationId, SurfaceDeclarationKind, SurfaceSourceId,
};
use nocter_syntax::{direct_identifier_iter, type_behavior_owner};
use std::collections::BTreeMap;

pub(crate) fn collect_structural_grants(
    surface: &DeclarationSurface<'_>,
) -> Result<BTreeMap<SurfaceDeclarationId, SurfaceDeclarationId>, Error> {
    let declarations = surface.declarations();
    let grants = declarations
        .iter()
        .copied()
        .enumerate()
        .filter(|(_, item)| item.kind() == SurfaceDeclarationKind::StructuralLiteralGrant);
    let mut grants = grants.peekable();
    if grants.peek().is_none() {
        return Ok(BTreeMap::new());
    }
    let mut roots = BTreeMap::<(SurfaceSourceId, &str), Option<SurfaceDeclarationId>>::new();
    for (index, record) in declarations.iter().copied().enumerate() {
        if record.kind() != SurfaceDeclarationKind::Struct
            || source_kind(surface, record)? != ModuleSourceKind::Root
        {
            continue;
        }
        let invalid = || Error::InconsistentSurface(record.node());
        let syntax = bound_syntax(surface, record).ok_or_else(invalid)?;
        let name = record.name().ok_or_else(invalid)?;
        let name = syntax.text_at(name.range()).ok_or_else(invalid)?;
        roots
            .entry((record.source(), name))
            .and_modify(|root| *root = None)
            .or_insert(Some(SurfaceDeclarationId::from_index(index)));
    }

    let mut selected = BTreeMap::<SurfaceDeclarationId, SurfaceDeclarationId>::new();
    for (index, grant) in grants {
        let invalid = || Error::InconsistentSurface(grant.node());
        let reject = |issue, related| Error::InvalidStructuralGrant {
            primary: grant.node(),
            related,
            issue,
        };
        if source_kind(surface, grant)? != ModuleSourceKind::Root {
            return Err(reject(StructuralGrantIssue::RootSchema, None));
        }
        let syntax = bound_syntax(surface, grant).ok_or_else(invalid)?;
        let owner = grant.owner().ok_or_else(invalid)?;
        let (_, name) = type_behavior_owner(syntax.tree(), declarations[owner.index()].node())
            .ok_or_else(invalid)?;
        let name = syntax.text_at(name.range()).ok_or_else(invalid)?;
        let Some(&Some(record)) = roots.get(&(grant.source(), name)) else {
            return Err(reject(StructuralGrantIssue::RootSchema, None));
        };
        let nominal = declarations[record.index()];
        if is_bodyless_nominal(surface, nominal) {
            return Err(reject(
                StructuralGrantIssue::RootSchema,
                Some(nominal.node()),
            ));
        }
        if let Some(previous) = selected.insert(record, SurfaceDeclarationId::from_index(index)) {
            return Err(reject(
                StructuralGrantIssue::DuplicateGrant,
                Some(declarations[previous.index()].node()),
            ));
        }
        let fields = surface
            .record_fields(record)
            .map(|(_, field)| field_name(surface, field))
            .collect::<Result<Vec<_>, _>>()?;
        let listed = direct_identifier_iter(syntax.tree(), grant.node())
            .map(|token| syntax.text_at(token.range()));
        if !fields.into_iter().map(Some).eq(listed) {
            return Err(reject(
                StructuralGrantIssue::FieldList,
                Some(nominal.node()),
            ));
        }
    }
    Ok(selected)
}
