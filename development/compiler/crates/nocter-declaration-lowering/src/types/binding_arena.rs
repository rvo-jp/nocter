use std::collections::HashMap;

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
    pub(super) origins: NormalizationOrigins,
}
