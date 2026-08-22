use std::collections::BTreeSet;
use std::fmt;

use nocter_declarations::DeclarationGraph;
use nocter_model::{FieldId, ModuleId, NominalTypeId, Symbol};

use crate::{
    CheckedProgram, ConstructionSurfaceSelectionError, ConstructionSurfaceTable,
    PreparedSemanticProgram, SelectedConstructionEntry,
};

/// One still-uninitialized field offered inside a structural construction expression.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuralFieldCompletionCandidate {
    field: FieldId,
    name: Symbol,
}

impl StructuralFieldCompletionCandidate {
    #[must_use]
    pub const fn field(self) -> FieldId {
        self.field
    }

    #[must_use]
    pub const fn name(self) -> Symbol {
        self.name
    }
}

/// Failure to derive structural-field completion from immutable compiler authorities.
#[derive(Debug)]
pub enum StructuralFieldCompletionError {
    Surface(ConstructionSurfaceSelectionError),
    MissingStructuralEntry(NominalTypeId),
    MissingField(FieldId),
    InvalidField(FieldId),
    DuplicateInitializedField(FieldId),
}

impl fmt::Display for StructuralFieldCompletionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Surface(error) => error.fmt(formatter),
            Self::MissingStructuralEntry(definition) => write!(
                formatter,
                "structural completion type {definition:?} has no structural field entry"
            ),
            Self::MissingField(field) => {
                write!(formatter, "structural completion field {field:?} is absent")
            }
            Self::InvalidField(field) => write!(
                formatter,
                "structural completion field {field:?} belongs to another type"
            ),
            Self::DuplicateInitializedField(field) => write!(
                formatter,
                "structural completion field {field:?} is initialized more than once"
            ),
        }
    }
}

impl std::error::Error for StructuralFieldCompletionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Surface(error) => Some(error),
            Self::MissingStructuralEntry(_)
            | Self::MissingField(_)
            | Self::InvalidField(_)
            | Self::DuplicateInitializedField(_) => None,
        }
    }
}

impl From<ConstructionSurfaceSelectionError> for StructuralFieldCompletionError {
    fn from(error: ConstructionSurfaceSelectionError) -> Self {
        Self::Surface(error)
    }
}

impl CheckedProgram {
    /// Enumerates fields not yet initialized in an accessible structural construction.
    ///
    /// # Errors
    ///
    /// Returns an error when checked field identities disagree with the canonical construction
    /// surface or declaration graph.
    pub fn structural_field_completions(
        &self,
        definition: NominalTypeId,
        module: ModuleId,
        initialized: &[FieldId],
    ) -> Result<Box<[StructuralFieldCompletionCandidate]>, StructuralFieldCompletionError> {
        select_structural_field_completions(
            self.graph(),
            self.construction_surfaces(),
            definition,
            module,
            initialized,
        )
    }
}

impl PreparedSemanticProgram {
    /// Enumerates fields not yet initialized from the completed pre-body semantic authority.
    ///
    /// # Errors
    ///
    /// Returns an error when retained field identities disagree with the canonical construction
    /// surface or declaration graph.
    pub fn structural_field_completions(
        &self,
        definition: NominalTypeId,
        module: ModuleId,
        initialized: &[FieldId],
    ) -> Result<Box<[StructuralFieldCompletionCandidate]>, StructuralFieldCompletionError> {
        select_structural_field_completions(
            self.graph(),
            self.construction_surfaces(),
            definition,
            module,
            initialized,
        )
    }
}

fn select_structural_field_completions(
    graph: &DeclarationGraph,
    surfaces: &ConstructionSurfaceTable,
    definition: NominalTypeId,
    module: ModuleId,
    initialized: &[FieldId],
) -> Result<Box<[StructuralFieldCompletionCandidate]>, StructuralFieldCompletionError> {
    let surface = surfaces.accessible_surface(graph, definition, module)?;
    if !surface
        .entries()
        .contains(&SelectedConstructionEntry::Structural)
    {
        return Ok(Box::new([]));
    }
    let declared = surfaces
        .structural_fields(graph, definition, module)?
        .ok_or(StructuralFieldCompletionError::MissingStructuralEntry(
            definition,
        ))?;
    let mut used = BTreeSet::new();
    for field in initialized.iter().copied() {
        let declaration = graph
            .declarations()
            .fields()
            .get(field)
            .ok_or(StructuralFieldCompletionError::MissingField(field))?;
        if declaration.owner() != definition || !declared.contains(&field) {
            return Err(StructuralFieldCompletionError::InvalidField(field));
        }
        if !used.insert(field) {
            return Err(StructuralFieldCompletionError::DuplicateInitializedField(
                field,
            ));
        }
    }
    declared
        .iter()
        .copied()
        .filter(|field| !used.contains(field))
        .map(|field| {
            let declaration = graph
                .declarations()
                .fields()
                .get(field)
                .ok_or(StructuralFieldCompletionError::MissingField(field))?;
            if declaration.owner() != definition {
                return Err(StructuralFieldCompletionError::InvalidField(field));
            }
            Ok(StructuralFieldCompletionCandidate {
                field,
                name: declaration.name(),
            })
        })
        .collect::<Result<Vec<_>, _>>()
        .map(Vec::into_boxed_slice)
}

#[cfg(test)]
mod tests {
    use nocter_declaration_lowering::lower_compile_unit_declarations;
    use nocter_declarations::ExportedEntity;

    use super::select_structural_field_completions;
    use crate::prepare_program_checking;
    use crate::test_support::Fixture;

    #[test]
    fn completion_preserves_field_order_and_the_structural_visibility_boundary() {
        let fixture = Fixture::with_child(
            "use ./child\n",
            concat!(
                "pub struct Record {\n",
                "    pub first: i32\n",
                "    pub(./) second: i32\n",
                "    pub third: i32\n",
                "}\n",
            ),
        );
        let input = fixture.input(false);
        let lowered = lower_compile_unit_declarations(&input).unwrap();
        let (program, source_index) = lowered.into_parts();
        let prepared = prepare_program_checking(&input, program, source_index).unwrap();
        let graph = prepared.graph();
        let child_name = graph.symbols().get("child").unwrap();
        let child = graph
            .modules()
            .iter()
            .find(|(_, module)| module.path().segments() == [child_name])
            .map(|(id, _)| id)
            .unwrap();
        let package = graph.modules().get(child).unwrap().package();
        let root = graph
            .modules()
            .iter()
            .find(|(_, module)| module.package() == package && module.path().segments().is_empty())
            .map(|(id, _)| id)
            .unwrap();
        let record_name = graph.symbols().get("Record").unwrap();
        let ExportedEntity::NominalType(record) = graph.lookup_local(child, record_name).unwrap()
        else {
            panic!("Record is not nominal");
        };
        let fields = prepared
            .construction_surfaces()
            .structural_fields(graph, record, child)
            .unwrap()
            .unwrap();

        let candidates = select_structural_field_completions(
            graph,
            prepared.construction_surfaces(),
            record,
            child,
            &fields[1..2],
        )
        .unwrap();
        assert_eq!(
            candidates
                .iter()
                .map(|candidate| graph.symbols().spelling(candidate.name()).unwrap())
                .collect::<Vec<_>>(),
            ["first", "third"]
        );
        assert!(
            select_structural_field_completions(
                graph,
                prepared.construction_surfaces(),
                record,
                root,
                &[],
            )
            .unwrap()
            .is_empty()
        );
    }
}
