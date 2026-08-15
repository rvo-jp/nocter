//! Nocter ABI lowering and layout rules.

mod functions;
mod layout;
mod model;
#[cfg(test)]
mod tests;
mod types;

pub use functions::{
    function_abi_from_signature, function_abi_from_signature_with_resolver,
    function_parameter_abi_word_count_from_signature,
    function_parameter_abi_word_count_from_signature_with_resolver,
    function_parameters_abi_from_signature, function_parameters_abi_from_signature_with_resolver,
    function_success_return_passing_from_signature,
    function_success_return_passing_from_signature_with_resolver,
};
pub use layout::{
    array_element_stride, classify_value, layout_array, layout_enum, layout_of, layout_struct,
};
pub use model::{
    ABI_WORD_SIZE, ARGUMENT_REGISTER_COUNT, AbiEnum, AbiEnumVariant, AbiField, AbiParameter,
    AbiReturn, AbiType, AbiTypeContract, AbiTypeError, AbiValue, DIRECT_VALUE_MAX_SIZE,
    FieldLayout, FunctionAbi, LayoutError, ParameterPassing, ReturnPassing, StructLayout,
    ValueClassification, ValueLayout,
};
pub use types::{
    abi_type_contract_from_type_expr, abi_type_contract_from_type_expr_with_resolver,
    abi_type_from_type_expr, abi_type_from_type_expr_with_resolver, abi_value_from_type_expr,
    abi_value_from_type_expr_with_resolver,
};
