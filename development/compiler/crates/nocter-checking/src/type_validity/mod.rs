mod associated_projection;
mod program;
mod rule;
mod shape;

#[cfg(test)]
mod tests;

pub(crate) use associated_projection::validate_associated_projection_uses;
pub use program::{
    DeclarationTypeValidityError, TypeValidityInternalError, validate_declaration_types,
};
pub use rule::TypeValidityRule;
pub use shape::{TypePosition, TypeValidityFailure, TypeValidityViolation, validate_type};
