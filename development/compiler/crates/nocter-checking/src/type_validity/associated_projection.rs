use nocter_compile_input::CompileUnitInput;
use nocter_declarations::DeclarationGraph;
use nocter_frontend_bindings::{AssociatedProjectionUse, FrontendBindings};
use nocter_source_index::SourceOrigin;
use nocter_syntax::SyntaxOrigin;

use crate::interface_implementation::{
    AssociatedImplementationSelection, InterfaceImplementationTable,
    select_associated_implementation,
};
use crate::type_relations::is_concrete_type;

use super::{DeclarationTypeValidityError, TypeValidityInternalError, TypeValidityRule};

/// Validates concrete associated selections against the sole checked implementation authority.
///
/// Name normalization establishes the associated declaration identity. This pass alone decides
/// whether an implementation applies; declaration lowering never duplicates refinements,
/// conditional requirements, or overlap semantics.
pub(crate) fn validate_associated_projection_uses(
    input: &CompileUnitInput<'_>,
    graph: &DeclarationGraph,
    types: &mut nocter_model::TypeTransaction,
    bindings: &FrontendBindings,
    implementations: &InterfaceImplementationTable,
) -> Result<(), DeclarationTypeValidityError> {
    for projection in bindings.associated_projection_uses() {
        if !is_concrete_type(types, projection.base())
            .map_err(TypeValidityInternalError::Substitution)?
        {
            continue;
        }
        let declaration = graph
            .declarations()
            .associated_types()
            .get(projection.associated())
            .ok_or(TypeValidityInternalError::MissingAssociatedType(
                projection.associated(),
            ))?;
        let selection = select_associated_implementation(
            types,
            implementations,
            &[] as &[crate::CheckedRequirement],
            &[],
            projection.base(),
            declaration.interface(),
        )
        .map_err(TypeValidityInternalError::Substitution)?;
        let rule = match selection {
            AssociatedImplementationSelection::None => {
                Some(TypeValidityRule::UnavailableAssociatedProjection)
            }
            AssociatedImplementationSelection::Ambiguous => {
                Some(TypeValidityRule::AmbiguousAssociatedProjection)
            }
            AssociatedImplementationSelection::Unique(selection) => {
                let implementation = selection.declaration();
                let entry = implementations.entries().get(&implementation).ok_or(
                    TypeValidityInternalError::MissingInterfaceImplementation(implementation),
                )?;
                if entry.associated_type(projection.associated()).is_none() {
                    return Err(TypeValidityInternalError::MissingAssociatedBinding {
                        implementation,
                        associated: projection.associated(),
                    }
                    .into());
                }
                None
            }
        };
        if let Some(rule) = rule {
            return Err(DeclarationTypeValidityError::Rule(
                rule.diagnostic(projection_origin(input, *projection)?),
            ));
        }
    }
    Ok(())
}

fn projection_origin(
    input: &CompileUnitInput<'_>,
    projection: AssociatedProjectionUse,
) -> Result<SourceOrigin, TypeValidityInternalError> {
    let syntax = projection.origin();
    let source = match syntax {
        SyntaxOrigin::Node(node) => node.source(),
        SyntaxOrigin::Token(token) => token.source(),
    };
    let tree = input
        .syntax_tree(source)
        .ok_or(TypeValidityInternalError::MissingAssociatedProjectionSource(syntax))?;
    match syntax {
        SyntaxOrigin::Node(node) => SourceOrigin::from_node(tree, node)
            .map_err(|_| TypeValidityInternalError::MissingAssociatedProjectionSource(syntax)),
        SyntaxOrigin::Token(token) => SourceOrigin::from_token(tree, token)
            .map_err(|_| TypeValidityInternalError::MissingAssociatedProjectionSource(syntax)),
    }
}
