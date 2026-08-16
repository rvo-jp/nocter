use crate::type_relations::{SubstitutionError, TypeSubstitution};
use nocter_declarations::{DeclarationGraph, NominalShape};
use nocter_model::{FieldId, ModuleId, NominalTypeId, TypeId, TypeKind, TypeStore};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SelectedField {
    owner: NominalTypeId,
    field: FieldId,
    ty: TypeId,
}

impl SelectedField {
    pub(crate) const fn owner(self) -> NominalTypeId {
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
    from: ModuleId,
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
    let field = selected.ok_or(FieldSelectionError::MissingField(definition))?;
    let declaration = graph
        .declarations()
        .fields()
        .get(field)
        .ok_or(FieldSelectionError::UnknownField(field))?;
    let site = graph
        .declaration_sites()
        .get(declaration.site())
        .ok_or(FieldSelectionError::UnknownFieldSite(field))?;
    if !graph.is_visible_from(site.visibility(), from, site.module()) {
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
        owner: definition,
        field,
        ty,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FieldSelectionError {
    UnknownType(TypeId),
    NoFields(TypeId),
    UnknownNominal(NominalTypeId),
    UnknownField(FieldId),
    UnknownFieldSite(FieldId),
    MissingField(NominalTypeId),
    AmbiguousField(NominalTypeId),
    InaccessibleField(FieldId),
    GenericArity(NominalTypeId),
    Substitution(SubstitutionError),
}
