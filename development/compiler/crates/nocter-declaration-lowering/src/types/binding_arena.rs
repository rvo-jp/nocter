use std::collections::{HashMap, HashSet};

use crate::SurfaceDeclarationId;
use nocter_syntax::NodeId;

use super::normalization_origins::NormalizationOrigins;
use super::{BoundTypeId, BoundTypeKind};

/// Mutable header-type arena and its temporary source-side indexes during binding.
#[derive(Debug, Default)]
pub(super) struct BindingArena {
    pub(super) kinds: Vec<BoundTypeKind>,
    pub(super) roots: HashMap<NodeId, BoundTypeId>,
    pub(super) root_declarations: HashMap<NodeId, SurfaceDeclarationId>,
    pub(super) usize_expression_declarations: HashMap<NodeId, SurfaceDeclarationId>,
    pub(super) constant_argument_types: HashSet<super::BoundTypeId>,
    pub(super) origins: NormalizationOrigins,
}

impl BindingArena {
    pub(super) fn record_usize_expressions(
        &mut self,
        arguments: &[super::BoundGenericValue],
        declaration: SurfaceDeclarationId,
    ) {
        self.usize_expression_declarations.extend(
            arguments
                .iter()
                .filter_map(|argument| argument.usize_expression())
                .map(|expression| (expression, declaration)),
        );
    }
}
