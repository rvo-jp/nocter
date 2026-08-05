use super::InterfaceConformance;
use super::signatures::method_signatures;
use crate::ast::ImplDecl;

pub(super) fn interface_conformance(impl_: &ImplDecl) -> Option<InterfaceConformance> {
    Some(InterfaceConformance {
        declaration_span: impl_.span,
        generic_parameters: impl_
            .generics
            .parameters
            .iter()
            .map(|parameter| parameter.name.clone())
            .collect(),
        generic_parameter_bounds: impl_
            .generics
            .parameters
            .iter()
            .map(|parameter| parameter.bounds.clone())
            .collect(),
        interface_ty: impl_.interface_ty.clone()?,
        target_ty: impl_.target_ty.clone(),
        methods: method_signatures(impl_).collect(),
    })
}
