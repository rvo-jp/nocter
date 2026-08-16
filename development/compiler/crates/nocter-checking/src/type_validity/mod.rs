mod program;
mod rule;
mod shape;

#[cfg(test)]
mod tests;

pub use program::{
    DeclarationTypeValidityError, TypeValidityInternalError, validate_declaration_types,
};
pub use rule::TypeValidityRule;
pub use shape::{TypePosition, TypeValidityFailure, TypeValidityViolation, validate_type};
