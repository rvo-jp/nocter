use nocter_checking::{
    CheckedProgram, ClosureParameter, GenericArguments, StaticDispatch, StaticSelection,
};
use nocter_declarations::RequirementKind;
use nocter_model::{CallableCapability, TypeId, TypeStore};

use super::{Renderer, SemanticPresentation};

pub(in crate::query) struct RenderedSignature {
    pub(in crate::query) presentation: SemanticPresentation,
    pub(in crate::query) parameter_ranges: Box<[(usize, usize)]>,
}

pub(in crate::query) fn static_signature_presentation(
    graph: &nocter_declarations::DeclarationGraph,
    types: &TypeStore,
    selection: &StaticSelection,
    spellings: &super::visible_spelling::VisibleSpellings,
) -> Option<RenderedSignature> {
    let mut renderer =
        Renderer::with_generics(graph, types, selection.generic_arguments(), spellings);
    match selection.dispatch() {
        StaticDispatch::Direct(callable)
        | StaticDispatch::InterfaceMethod {
            method: callable, ..
        }
        | StaticDispatch::InterfaceSelfMethod {
            method: callable, ..
        }
        | StaticDispatch::InterfaceDefault {
            method: callable, ..
        }
        | StaticDispatch::OpaqueMethod {
            method: callable, ..
        } => renderer.callable(callable)?,
        StaticDispatch::StructuralRequirement { requirement, .. } => {
            let requirement = graph.declarations().requirements().get(requirement)?;
            let RequirementKind::Callable { contract, .. } = requirement.kind() else {
                return None;
            };
            renderer.callable_contract(contract)?;
        }
    }
    Some(renderer.finish_signature())
}

pub(in crate::query) fn closure_signature_presentation(
    checked: &CheckedProgram,
    closure: nocter_model::ClosureId,
    spellings: &super::visible_spelling::VisibleSpellings,
) -> Option<RenderedSignature> {
    let signature = checked.closures().get(closure)?.signature();
    let mut renderer = Renderer::for_signature(checked.graph(), checked.types(), spellings);
    renderer.callable_shape(
        signature.capability(),
        signature.parameters(),
        signature.result(),
    )?;
    Some(renderer.finish_signature())
}

impl<'a> Renderer<'a> {
    fn with_generics(
        graph: &'a nocter_declarations::DeclarationGraph,
        types: &'a TypeStore,
        generics: &'a GenericArguments,
        spellings: &'a super::visible_spelling::VisibleSpellings,
    ) -> Self {
        Self {
            graph,
            types,
            output: String::new(),
            generics: Some(generics),
            record_parameters: true,
            parameter_ranges: Vec::new(),
            self_type: None,
            spellings,
        }
    }

    fn for_signature(
        graph: &'a nocter_declarations::DeclarationGraph,
        types: &'a TypeStore,
        spellings: &'a super::visible_spelling::VisibleSpellings,
    ) -> Self {
        Self {
            graph,
            types,
            output: String::new(),
            generics: None,
            record_parameters: true,
            parameter_ranges: Vec::new(),
            self_type: None,
            spellings,
        }
    }

    fn callable_shape(
        &mut self,
        capability: CallableCapability,
        parameters: &[ClosureParameter],
        result: TypeId,
    ) -> Option<()> {
        self.output.push_str(match capability {
            CallableCapability::Readonly => "&func",
            CallableCapability::ReadWrite => "&+func",
            CallableCapability::Owned => "func",
        });
        self.output.push('(');
        for (index, parameter) in parameters.iter().enumerate() {
            if index != 0 {
                self.output.push_str(", ");
            }
            let start = self.output.len();
            self.ty(parameter.ty())?;
            self.record_parameter(start);
        }
        self.output.push_str("): ");
        self.ty(result)
    }

    fn finish_signature(self) -> RenderedSignature {
        RenderedSignature {
            presentation: SemanticPresentation {
                code: self.output.into_boxed_str(),
            },
            parameter_ranges: self.parameter_ranges.into_boxed_slice(),
        }
    }
}
