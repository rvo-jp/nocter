use crate::type_relations::{SubstitutionError, TypeSubstitution};
use nocter_declarations::{DeclarationGraph, NominalShape};
use nocter_model::{FieldId, NominalTypeId, TypeId, TypeKind, TypeStore};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SelectedField {
    owner: Option<NominalTypeId>,
    field: FieldId,
    ty: TypeId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SelectedStructuralField {
    field: FieldId,
    ty: TypeId,
}

impl SelectedStructuralField {
    pub(crate) const fn field(self) -> FieldId {
        self.field
    }

    pub(crate) const fn ty(self) -> TypeId {
        self.ty
    }
}

impl SelectedField {
    pub(crate) const fn owner(self) -> Option<NominalTypeId> {
        self.owner
    }

    pub(crate) const fn field(self) -> FieldId {
        self.field
    }

    pub(crate) const fn ty(self) -> TypeId {
        self.ty
    }
}

/// Selects one exact visible field and substitutes its owner's actual generic arguments.
pub(crate) fn select_field(
    graph: &DeclarationGraph,
    types: &mut TypeStore,
    from: crate::SourceAccessContext<'_>,
    base: TypeId,
    name: &str,
) -> Result<SelectedField, FieldSelectionError> {
    let (definition, arguments) = match types.get(base) {
        Some(TypeKind::Nominal {
            definition,
            arguments,
        }) => (*definition, arguments.clone()),
        Some(_) => return Err(FieldSelectionError::NoFields(base)),
        None => return Err(FieldSelectionError::UnknownType(base)),
    };
    let nominal = graph
        .declarations()
        .nominal_types()
        .get(definition)
        .ok_or(FieldSelectionError::UnknownNominal(definition))?;
    let NominalShape::Struct { fields, .. } = nominal.shape() else {
        return Err(FieldSelectionError::NoFields(base));
    };
    let mut selected = None;
    for field in fields.iter().copied() {
        let declaration = graph
            .declarations()
            .fields()
            .get(field)
            .ok_or(FieldSelectionError::UnknownField(field))?;
        if graph.symbols().spelling(declaration.name()) != Some(name) {
            continue;
        }
        if selected.replace(field).is_some() {
            return Err(FieldSelectionError::AmbiguousField(definition));
        }
    }
    let field = selected.ok_or(FieldSelectionError::MissingField(base))?;
    let declaration = graph
        .declarations()
        .fields()
        .get(field)
        .ok_or(FieldSelectionError::UnknownField(field))?;
    let visible = match from.site_is_visible(graph, declaration.site()) {
        Ok(visible) => visible,
        Err(crate::source_visibility::SourceVisibilityError::MissingSite(_)) => {
            return Err(FieldSelectionError::UnknownFieldSite(field));
        }
        Err(crate::source_visibility::SourceVisibilityError::Access(error)) => {
            return Err(FieldSelectionError::SourceAccess(error));
        }
    };
    if !visible {
        return Err(FieldSelectionError::InaccessibleField(field));
    }
    if nominal.generic_parameters().len() != arguments.len() {
        return Err(FieldSelectionError::GenericArity(definition));
    }
    let mut substitution = TypeSubstitution::default();
    for (parameter, argument) in nominal
        .generic_parameters()
        .iter()
        .copied()
        .zip(arguments.iter().copied())
    {
        substitution.bind_generic(parameter, argument);
    }
    let ty = substitution
        .apply_type(types, declaration.ty())
        .map_err(FieldSelectionError::Substitution)?;
    Ok(SelectedField {
        owner: Some(definition),
        field,
        ty,
    })
}

/// Validates one field identity selected by the construction-surface table.
///
/// The returned type is the owner's generic pattern. Aggregate inference specializes it only
/// after all source-order field evidence has been collected.
pub(crate) fn select_structural_field(
    graph: &DeclarationGraph,
    from: crate::SourceAccessContext<'_>,
    owner: NominalTypeId,
    field: FieldId,
) -> Result<SelectedStructuralField, FieldSelectionError> {
    let declaration = graph
        .declarations()
        .fields()
        .get(field)
        .ok_or(FieldSelectionError::UnknownField(field))?;
    if declaration.owner() != owner {
        return Err(FieldSelectionError::UnknownField(field));
    }
    let visible = match from.site_is_visible(graph, declaration.site()) {
        Ok(visible) => visible,
        Err(crate::source_visibility::SourceVisibilityError::MissingSite(_)) => {
            return Err(FieldSelectionError::UnknownFieldSite(field));
        }
        Err(crate::source_visibility::SourceVisibilityError::Access(error)) => {
            return Err(FieldSelectionError::SourceAccess(error));
        }
    };
    if !visible {
        return Err(FieldSelectionError::InaccessibleField(field));
    }
    Ok(SelectedStructuralField {
        field,
        ty: declaration.ty(),
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FieldSelectionError {
    UnknownType(TypeId),
    NoFields(TypeId),
    UnknownNominal(NominalTypeId),
    UnknownField(FieldId),
    UnknownFieldSite(FieldId),
    MissingField(TypeId),
    AmbiguousField(NominalTypeId),
    InaccessibleField(FieldId),
    GenericArity(NominalTypeId),
    Substitution(SubstitutionError),
    SourceAccess(nocter_frontend_bindings::SourceAccessError),
}
