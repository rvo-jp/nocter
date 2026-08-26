use std::collections::HashMap;

use nocter_model::TypeAliasId;
use nocter_source_index::SyntaxOrigin;

use super::BoundTypeId;

/// Syntax subjects needed only while canonicalizing the bound header arena.
///
/// The map is consumed before the immutable declaration program is frozen. It prevents
/// normalization diagnostics from searching syntax by semantic ID or placing source coordinates
/// in canonical type identity.
#[derive(Debug, Default)]
pub(super) struct NormalizationOrigins {
    bound: HashMap<BoundTypeId, SyntaxOrigin>,
    aliases: HashMap<TypeAliasId, SyntaxOrigin>,
}

impl NormalizationOrigins {
    pub(super) fn record_bound_if_absent(&mut self, ty: BoundTypeId, origin: SyntaxOrigin) {
        self.bound.entry(ty).or_insert(origin);
    }

    pub(super) fn record_bound(&mut self, ty: BoundTypeId, origin: SyntaxOrigin) {
        self.bound.insert(ty, origin);
    }

    pub(super) fn record_alias(&mut self, alias: TypeAliasId, origin: SyntaxOrigin) {
        self.aliases.insert(alias, origin);
    }

    pub(super) fn bound(&self, ty: BoundTypeId) -> Option<SyntaxOrigin> {
        self.bound.get(&ty).copied()
    }

    pub(super) fn alias(&self, alias: TypeAliasId) -> Option<SyntaxOrigin> {
        self.aliases.get(&alias).copied()
    }
}
