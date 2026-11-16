//! Read-only reconciliation against one immutable declaration's signature graph.
use super::{Program, ReferenceNode, resolve_function_definition, resolve_reference, supported};
use crate::value_access::{FunctionConstraint, FunctionWitness, ValueDefinition};
use nocter_model::{TypeId, TypeKind};

#[derive(Clone, Copy)]
struct HeaderContracts<'a> {
    origin: nocter_model::ValueFunctionOrigin,
    owner: nocter_model::ValueContractOwner,
    root: Option<&'a FunctionConstraint>,
    components: &'a [FunctionConstraint],
    types: &'a [FunctionConstraint],
}

impl Program {
    /// Reconcile an input against its immutable header, without interning or binding a value.
    #[must_use]
    pub fn existing_input_value_reference(
        &self,
        input: nocter_model::AccessParameterId,
        value: &ValueDefinition,
    ) -> Option<TypeId> {
        let parameter = self.parameters.get(input)?;
        self.existing_header_value_reference(
            value,
            HeaderContracts {
                origin: nocter_model::ValueFunctionOrigin::Parameter(input.into()),
                root: parameter.contract.as_ref(),
                components: &parameter.components,
                types: &parameter.type_contracts,
                owner: nocter_model::ValueContractOwner::Parameter(input),
            },
        )
    }
    /// Reconcile a result against its immutable header; this supplies no runtime producer.
    #[must_use]
    pub fn existing_result_value_reference(
        &self,
        owner: nocter_model::AccessCodeId,
        value: &ValueDefinition,
    ) -> Option<TypeId> {
        let code = self.codes.get(owner)?;
        let root = matches!(value, ValueDefinition::Function(_));
        self.existing_header_value_reference(
            value,
            HeaderContracts {
                origin: nocter_model::ValueFunctionOrigin::Result(owner.into()),
                root: root.then(|| code.result_contracts.first()).flatten(),
                components: if root { &[] } else { &code.result_contracts },
                types: &code.type_contracts,
                owner: nocter_model::ValueContractOwner::Result(owner),
            },
        )
    }
    fn existing_header_value_reference(
        &self,
        value: &ValueDefinition,
        header: HeaderContracts<'_>,
    ) -> Option<TypeId> {
        let mut component = 0;
        let mut nested = 0;
        let mut construct = |node| match node {
            ReferenceNode::Data(TypeKind::Builtin(data)) if !supported(data) => None,
            ReferenceNode::Data(kind) => self.types.store().identity(&kind),
            ReferenceNode::Function {
                contract,
                in_signature,
            } => {
                let (origin, expected) = if in_signature {
                    let ordinal = nested;
                    nested += 1;
                    (
                        nocter_model::ValueFunctionOrigin::Contract(
                            nocter_model::ValueCallableContract {
                                owner: header.owner,
                                ordinal,
                            },
                        ),
                        header.types.get(ordinal),
                    )
                } else {
                    let ordinal = component;
                    component += 1;
                    let origin = match header.origin {
                        nocter_model::ValueFunctionOrigin::Parameter(
                            nocter_model::ValueParameter::Declaration(input),
                        ) => nocter_model::ValueFunctionOrigin::Parameter(
                            nocter_model::ValueParameter::Component {
                                input: input.into(),
                                ordinal,
                            },
                        ),
                        nocter_model::ValueFunctionOrigin::Result(result) => {
                            nocter_model::ValueFunctionOrigin::Result(
                                nocter_model::ValueCallableResult {
                                    owner: result.owner,
                                    ordinal,
                                },
                            )
                        }
                        _ => return None,
                    };
                    (origin, header.components.get(ordinal))
                };
                (expected == Some(&contract))
                    .then(|| {
                        self.function_data(FunctionWitness {
                            origin,
                            invocation: contract.invocation,
                        })
                    })
                    .flatten()
            }
        };
        let data = match value {
            ValueDefinition::Data(data) if header.root.is_none() => {
                resolve_reference(data, &mut construct)?
            }
            ValueDefinition::Function(definition) => {
                let contract = resolve_function_definition(definition, &mut construct)?;
                if header.root != Some(&contract) {
                    return None;
                }
                self.function_data(FunctionWitness {
                    origin: header.origin,
                    invocation: contract.invocation,
                })?
            }
            ValueDefinition::Data(_) => return None,
        };
        (component == header.components.len() && nested == header.types.len()).then_some(data)
    }
}
