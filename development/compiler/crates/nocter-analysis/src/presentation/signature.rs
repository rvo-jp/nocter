use nocter_checking::{CheckedProgram, GenericArguments, StaticDispatch, StaticSelection};
use nocter_declarations::{RequirementKind, StructuralCapability};
use nocter_model::{CallableCapability, TypeId, TypeStore};

use super::{Renderer, SemanticPresentation};

pub(crate) struct RenderedSignature {
    pub(crate) presentation: SemanticPresentation,
    pub(crate) parameter_ranges: Box<[(usize, usize)]>,
}

pub(crate) fn static_signature_presentation(
    checked: &CheckedProgram,
    selection: &StaticSelection,
) -> Option<RenderedSignature> {
    let graph = checked.graph();
    let mut renderer =
        Renderer::with_generics(graph, checked.types(), selection.generic_arguments());
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
        StaticDispatch::StructuralRequirement(requirement) => {
            let requirement = graph.declarations().requirements().get(requirement)?;
            let RequirementKind::Capability {
                capability: StructuralCapability::Callable(contract),
                ..
            } = requirement.kind()
            else {
                return None;
            };
            renderer.callable_contract(contract)?;
        }
    }
    Some(renderer.finish_signature())
}

pub(crate) fn closure_signature_presentation(
    checked: &CheckedProgram,
    closure: nocter_model::ClosureId,
) -> Option<RenderedSignature> {
    let signature = checked.closures().get(closure)?.signature();
    let mut renderer = Renderer::for_signature(checked.graph(), checked.types());
    renderer.callable_shape(
        signature.capability(),
        signature.parameters(),
        signature.result(),
    )?;
    Some(renderer.finish_signature())
}

impl<'a> Renderer<'a> {
    const fn with_generics(
        graph: &'a nocter_declarations::DeclarationGraph,
        types: &'a TypeStore,
        generics: &'a GenericArguments,
    ) -> Self {
        Self {
            graph,
            types,
            output: String::new(),
            generics: Some(generics),
            record_parameters: true,
            parameter_ranges: Vec::new(),
        }
    }

    fn for_signature(
        graph: &'a nocter_declarations::DeclarationGraph,
        types: &'a TypeStore,
    ) -> Self {
        Self {
            graph,
            types,
            output: String::new(),
            generics: None,
            record_parameters: true,
            parameter_ranges: Vec::new(),
        }
    }

    fn callable_shape(
        &mut self,
        capability: CallableCapability,
        parameters: &[TypeId],
        result: TypeId,
    ) -> Option<()> {
        self.output.push_str(match capability {
            CallableCapability::Readonly => "&func",
            CallableCapability::ReadWrite => "&+func",
            CallableCapability::Owned => "func",
        });
        self.output.push('(');
        for (index, parameter) in parameters.iter().copied().enumerate() {
            if index != 0 {
                self.output.push_str(", ");
            }
            let start = self.output.len();
            self.ty(parameter)?;
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
