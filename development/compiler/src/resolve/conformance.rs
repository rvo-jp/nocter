use super::signatures::method_signatures;
use super::{GenericRequirements, InterfaceConformance};
use crate::ast::{ImplDecl, ImplMember};

pub(super) fn interface_conformance(impl_: &ImplDecl) -> Option<InterfaceConformance> {
    Some(InterfaceConformance {
        declaration_span: impl_.span,
        generic_parameters: impl_
            .generics
            .parameters
            .iter()
            .map(|parameter| parameter.name.clone())
            .collect(),
        generic_parameter_requirements: impl_
            .generics
            .parameters
            .iter()
            .map(|parameter| {
                let mut requirements = GenericRequirements::from_parameter(parameter);
                requirements.extend_from_clause(&parameter.name, impl_.requirements.as_ref());
                requirements
            })
            .collect(),
        where_clause: impl_.requirements.clone(),
        interface_ty: impl_.interface_ty.clone()?,
        target_ty: impl_.target_ty.clone(),
        associated_types: impl_
            .members
            .iter()
            .filter_map(|member| match member {
                ImplMember::AssociatedType(binding) => {
                    Some(super::AssociatedTypeBindingSignature {
                        name: binding.name.clone(),
                        name_span: binding.name_span,
                        declaration_span: binding.span,
                        value: binding.value.clone(),
                    })
                }
                ImplMember::Method(_) | ImplMember::Drop(_) => None,
            })
            .collect(),
        methods: method_signatures(impl_).collect(),
    })
}
