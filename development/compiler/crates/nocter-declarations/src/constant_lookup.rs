use nocter_model::{ConstantId, ConstantValue};

/// Read-only constant-value capability shared by structural and completed semantic products.
///
/// Consumers that only present constants do not need authority over immutable statics or over the
/// construction stage that produced each value.
pub trait ConstantValueLookup {
    #[must_use]
    fn constant(&self, id: ConstantId) -> Option<&ConstantValue>;
}

impl ConstantValueLookup for crate::StructuralConstantTable {
    fn constant(&self, id: ConstantId) -> Option<&ConstantValue> {
        self.constants().get(&id)
    }
}

impl ConstantValueLookup for crate::DeclarationValueTable {
    fn constant(&self, id: ConstantId) -> Option<&ConstantValue> {
        self.constants().get(id)
    }
}
