use nocter_diagnostics::{DiagnosticNote, SourceDiagnostic};
use nocter_source_index::SourceOrigin;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum BodyRule {
    TypeMismatch,
    ImplicitMove,
    InvalidStatementValue,
    MissingBodyResult,
    IntegerOutOfRange,
    MoveCopyValue,
    InvalidMoveSource,
    UninitializedPlace,
    UnknownField,
    InaccessibleField,
    PartialMoveThroughDrop,
    InvalidLoopControl,
    InvalidExplicitDrop,
    InvalidAssignmentTarget,
    InvalidReinitialization,
    InvalidCompoundAssignment,
    InvalidReadWriteBorrow,
    InvalidIndexOperation,
    InvalidComparisonOperation,
    InvalidCall,
    InvalidConstruction,
    InvalidOutcomeOperation,
    InvalidPatternOperation,
    InvalidMatchCoverage,
    InvalidResultProvenance,
    ConflictingLoan,
    BorrowedPlaceMutation,
    InvalidStorageEscape,
    InvalidAllocationContext,
    InvalidInterpolation,
    InvalidSpreadAcquisition,
    InvalidSpreadIterator,
    InvalidSpreadElement,
    InvalidCollectionAcquisition,
    InvalidCollectionIterator,
    InvalidBodyTypeUse,
    InvalidDiscardBinding,
    InvalidOpaqueWitness,
    InvalidLiteralPackUse,
}

impl BodyRule {
    pub const ALL: &'static [Self] = &[
        Self::TypeMismatch,
        Self::ImplicitMove,
        Self::InvalidStatementValue,
        Self::MissingBodyResult,
        Self::IntegerOutOfRange,
        Self::MoveCopyValue,
        Self::InvalidMoveSource,
        Self::UninitializedPlace,
        Self::UnknownField,
        Self::InaccessibleField,
        Self::PartialMoveThroughDrop,
        Self::InvalidLoopControl,
        Self::InvalidExplicitDrop,
        Self::InvalidAssignmentTarget,
        Self::InvalidReinitialization,
        Self::InvalidCompoundAssignment,
        Self::InvalidReadWriteBorrow,
        Self::InvalidIndexOperation,
        Self::InvalidComparisonOperation,
        Self::InvalidCall,
        Self::InvalidConstruction,
        Self::InvalidOutcomeOperation,
        Self::InvalidPatternOperation,
        Self::InvalidMatchCoverage,
        Self::InvalidResultProvenance,
        Self::ConflictingLoan,
        Self::BorrowedPlaceMutation,
        Self::InvalidStorageEscape,
        Self::InvalidAllocationContext,
        Self::InvalidInterpolation,
        Self::InvalidSpreadAcquisition,
        Self::InvalidSpreadIterator,
        Self::InvalidSpreadElement,
        Self::InvalidCollectionAcquisition,
        Self::InvalidCollectionIterator,
        Self::InvalidBodyTypeUse,
        Self::InvalidDiscardBinding,
        Self::InvalidOpaqueWitness,
        Self::InvalidLiteralPackUse,
    ];

    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::TypeMismatch => "E0370",
            Self::ImplicitMove => "E0371",
            Self::InvalidStatementValue => "E0372",
            Self::MissingBodyResult => "E0373",
            Self::IntegerOutOfRange => "E0375",
            Self::MoveCopyValue => "E0376",
            Self::InvalidMoveSource => "E0377",
            Self::UninitializedPlace => "E0378",
            Self::UnknownField => "E0379",
            Self::InaccessibleField => "E0380",
            Self::PartialMoveThroughDrop => "E0381",
            Self::InvalidLoopControl => "E0382",
            Self::InvalidExplicitDrop => "E0383",
            Self::InvalidAssignmentTarget => "E0384",
            Self::InvalidReinitialization => "E0385",
            Self::InvalidCompoundAssignment => "E0386",
            Self::InvalidReadWriteBorrow => "E0387",
            Self::InvalidIndexOperation => "E0388",
            Self::InvalidComparisonOperation => "E0389",
            Self::InvalidCall => "E0390",
            Self::InvalidConstruction => "E0391",
            Self::InvalidOutcomeOperation => "E0392",
            Self::InvalidPatternOperation => "E0393",
            Self::InvalidMatchCoverage => "E0394",
            Self::InvalidResultProvenance => "E0395",
            Self::ConflictingLoan => "E0396",
            Self::BorrowedPlaceMutation => "E0397",
            Self::InvalidStorageEscape => "E0398",
            Self::InvalidAllocationContext => "E0399",
            Self::InvalidInterpolation => "E0400",
            Self::InvalidSpreadAcquisition => "E0401",
            Self::InvalidSpreadIterator => "E0402",
            Self::InvalidSpreadElement => "E0403",
            Self::InvalidCollectionAcquisition => "E0404",
            Self::InvalidCollectionIterator => "E0405",
            Self::InvalidBodyTypeUse => "E0406",
            Self::InvalidDiscardBinding => "E0407",
            Self::InvalidOpaqueWitness => "E0408",
            Self::InvalidLiteralPackUse => "E0409",
        }
    }

    pub(crate) fn diagnostic(self, primary: SourceOrigin) -> SourceDiagnostic {
        self.diagnostic_with_notes(primary, [])
    }

    pub(crate) fn diagnostic_with_notes(
        self,
        primary: SourceOrigin,
        notes: impl Into<Box<[DiagnosticNote]>>,
    ) -> SourceDiagnostic {
        let (message, help) = self.message_and_help();
        SourceDiagnostic::new(self.code(), message, primary, notes, Some(help))
    }

    fn message_and_help(self) -> (&'static str, &'static str) {
        match self {
            Self::TypeMismatch
            | Self::ImplicitMove
            | Self::InvalidStatementValue
            | Self::MissingBodyResult
            | Self::IntegerOutOfRange
            | Self::MoveCopyValue
            | Self::InvalidMoveSource
            | Self::UninitializedPlace
            | Self::UnknownField
            | Self::InaccessibleField
            | Self::PartialMoveThroughDrop => self.value_message(),
            Self::InvalidSpreadAcquisition
            | Self::InvalidSpreadIterator
            | Self::InvalidSpreadElement
            | Self::InvalidCollectionAcquisition
            | Self::InvalidCollectionIterator => self.iteration_message(),
            Self::InvalidBodyTypeUse | Self::InvalidDiscardBinding => self.binding_message(),
            Self::InvalidOpaqueWitness => (
                "opaque result does not select one valid concrete witness",
                "return one consistent type that conforms to the advertised interface and associated bindings",
            ),
            _ => self.operation_message(),
        }
    }

    fn value_message(self) -> (&'static str, &'static str) {
        match self {
            Self::TypeMismatch => (
                "expression type is incompatible with its expected destination type",
                "produce the exact expected type or use one applicable explicit conversion",
            ),
            Self::ImplicitMove => (
                "using this place would require an implicit move",
                "write `move place` when ownership transfer is intended",
            ),
            Self::InvalidStatementValue => (
                "non-final expression statement produces a value",
                "bind the value, make it the body result, or explicitly discard it with `let _ =`",
            ),
            Self::MissingBodyResult => (
                "callable can complete without producing its declared result",
                "add a body result or return on every reachable path",
            ),
            Self::IntegerOutOfRange => (
                "integer literal is outside the expected integer type's range",
                "use a value representable by the destination integer type",
            ),
            Self::MoveCopyValue => (
                "copyable value cannot be moved explicitly",
                "use the place directly; copying leaves the source initialized",
            ),
            Self::InvalidMoveSource => (
                "this place does not own storage that can be moved",
                "move an owned local, parameter, capture, or named struct field",
            ),
            Self::UninitializedPlace => (
                "place may be uninitialized at this use",
                "initialize it on every reachable path before using it",
            ),
            Self::UnknownField => (
                "type has no field with this name",
                "select a field declared by the base struct",
            ),
            Self::InaccessibleField => (
                "field is not visible from this module",
                "use an accessible field or a public API of the defining module",
            ),
            Self::PartialMoveThroughDrop => (
                "field move would partially initialize a struct with a drop declaration",
                "move the complete struct or keep every field initialized",
            ),
            _ => unreachable!("non-value body rule"),
        }
    }

    fn operation_message(self) -> (&'static str, &'static str) {
        match self {
            Self::InvalidLoopControl => (
                "loop control has no enclosing loop in this callable body",
                "place `break` or `continue` inside the loop it should target",
            ),
            Self::InvalidExplicitDrop => (
                "explicit drop requires a move-only owned binding",
                "remove `drop` for a copy or borrow binding",
            ),
            Self::InvalidAssignmentTarget => (
                "assignment target is not a writable place",
                "assign to a `var` binding, writable field, or readwrite index place",
            ),
            Self::InvalidReinitialization => (
                "this field cannot be initialized through an unavailable parent",
                "reinitialize the complete `var` binding before assigning through its fields",
            ),
            Self::InvalidCompoundAssignment => (
                "compound assignment requires a writable initialized integer place and matching RHS",
                "use a mutable integer destination and a right-hand side of the same integer type",
            ),
            Self::InvalidReadWriteBorrow => (
                "readwrite borrow requires a writable place",
                "borrow a `var` binding, writable field, or readwrite index place",
            ),
            Self::InvalidIndexOperation => (
                "no unique accessible index operation accepts this receiver and index",
                "use an index type with one applicable operation or add an accessible index declaration",
            ),
            Self::InvalidComparisonOperation => (
                "no unique accessible comparison operation accepts these operands",
                "use operands with one applicable primitive or source-defined comparison",
            ),
            Self::InvalidCall => (
                "this call has no valid callee, argument, and substitution plan",
                "use a callable target with matching capability, arity, argument types, and type evidence",
            ),
            Self::InvalidConstruction => (
                "this expression has no valid construction entry and type plan",
                "use an accessible structural or variant entry with complete fields, payloads, and type evidence",
            ),
            Self::InvalidOutcomeOperation => (
                "this outcome operation is incompatible with its operand or enclosing callable",
                "use `?`, `!`, `catch`, or `otherwise` with the matching optional or fallible layer",
            ),
            Self::InvalidPatternOperation => (
                "this enum pattern is incompatible with its target or payload bindings",
                "use the target enum's exact qualifier and variant with one slot per payload field",
            ),
            Self::InvalidMatchCoverage => (
                "match arms do not form one complete, unambiguous enum partition",
                "cover every variant exactly once or end the match with one `_` fallback arm",
            ),
            Self::InvalidResultProvenance => (
                "returned value carries storage outside the callable's result-provenance contract",
                "return static, current-allocation, or declared input-derived storage and do not let local, temporary, region, or unknown storage escape",
            ),
            Self::ConflictingLoan => (
                "borrow overlaps an incompatible live loan",
                "end the earlier loan's last use before creating this borrow",
            ),
            Self::BorrowedPlaceMutation => (
                "operation conflicts with a live loan of this place",
                "end the loan's last use before moving, dropping, assigning, or mutating this place",
            ),
            Self::InvalidStorageEscape => (
                "value carries storage that does not outlive its destination",
                "keep the value within its source scope, region, or temporary-owning statement",
            ),
            Self::InvalidAllocationContext => (
                "allocation override is not an established aborting allocator or context",
                "use a place whose exact standard semantic role permits aborting allocation",
            ),
            Self::InvalidInterpolation => (
                "interpolation value does not conform to the selected standard Format contract",
                "add an explicit conformance to the active standard Format interface or interpolate a supported value",
            ),
            Self::InvalidLiteralPackUse => (
                "literal element pack is used outside its non-escaping surface",
                "use only `items.len()` or consuming `for item in items` inside the sequence literal body",
            ),
            _ => unreachable!("non-operation body rule"),
        }
    }

    fn iteration_message(self) -> (&'static str, &'static str) {
        match self {
            Self::InvalidSpreadAcquisition => (
                "spread source has no unique expansion operation for the requested ownership mode",
                "provide one matching readonly or owned expansion, or move a value that directly conforms to Iterator",
            ),
            Self::InvalidSpreadIterator => (
                "spread source does not provide one exact-size iterator contract",
                "make the selected iterator conform uniquely to both Iterator and ExactSizeIterator",
            ),
            Self::InvalidSpreadElement => (
                "spread iterator item is incompatible with this spread mode or literal element type",
                "yield readonly references for copy and borrow spreads, and make copied referents copyable",
            ),
            Self::InvalidCollectionAcquisition => (
                "collection source has no unique acquisition for its explicit ownership mode",
                "use a direct iterator, or provide one expansion matching `&`, `&+`, or `move`",
            ),
            Self::InvalidCollectionIterator => (
                "collection acquisition does not provide one trusted iterator contract",
                "make the acquired type conform uniquely to the active standard Iterator interface",
            ),
            _ => unreachable!("non-iteration body rule"),
        }
    }

    fn binding_message(self) -> (&'static str, &'static str) {
        match self {
            Self::InvalidBodyTypeUse => (
                "body type annotation does not resolve to one valid semantic type",
                "use a visible type with complete generic arguments and satisfied requirements",
            ),
            Self::InvalidDiscardBinding => (
                "discard binding cannot be mutable or carry a type annotation",
                "write `let _ = expression` without `var` or an annotation",
            ),
            _ => unreachable!("non-binding body rule"),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::BodyRule;

    #[test]
    fn body_rule_codes_are_closed_and_unique() {
        let codes = BodyRule::ALL
            .iter()
            .copied()
            .map(BodyRule::code)
            .collect::<HashSet<_>>();
        assert_eq!(codes.len(), BodyRule::ALL.len());
    }
}
