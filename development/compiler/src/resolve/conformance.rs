use super::signatures::conformance_method_signatures;
use super::{GenericRequirements, InterfaceConformance};
use crate::ast::{ConformanceDecl, ConformanceMember};

pub(super) fn interface_conformance(conformance: &ConformanceDecl) -> InterfaceConformance {
    InterfaceConformance {
        declaration_span: conformance.span,
        generic_parameters: conformance
            .generics
            .parameters
            .iter()
            .map(|parameter| parameter.name.clone())
            .collect(),
        generic_parameter_requirements: conformance
            .generics
            .parameters
            .iter()
            .map(|parameter| {
                GenericRequirements::for_parameter(
                    &parameter.name,
                    conformance.requirements.as_ref(),
                )
            })
            .collect(),
        where_clause: conformance.requirements.clone(),
        interface_ty: conformance.interface_ty.clone(),
        target_ty: conformance.target_ty.clone(),
        associated_types: conformance
            .members
            .iter()
            .filter_map(|member| match member {
                ConformanceMember::AssociatedType(binding) => {
                    Some(super::AssociatedTypeBindingSignature {
                        name: binding.name.clone(),
                        name_span: binding.name_span,
                        declaration_span: binding.span,
                        value: binding.value.clone(),
                    })
                }
                ConformanceMember::Method(_) => None,
            })
            .collect(),
        methods: conformance_method_signatures(conformance).collect(),
    }
}
