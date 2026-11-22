//! One ordinary call ABI and guarded finite-case environment selection.
use super::{
    Builder, Failure, MachineCall, MachineCallAllocation, MachineCallTarget, MachineLayoutKind,
    MachineOperationKind, MachineValueId, RuntimeType, ValueInputOperation, mir,
};

impl Builder<'_> {
    pub(super) fn call(&mut self, call: &mir::Call) -> Result<(), Failure> {
        let arguments = self.call_inputs(
            &call.inputs,
            matches!(call.target, mir::CallTarget::Primitive(_)),
        )?;
        let target = match call.target {
            mir::CallTarget::Selected => return Err(Self::invalid()),
            mir::CallTarget::Body(id) => MachineCallTarget::Direct(
                *self.context.functions.get(&id).ok_or_else(Self::invalid)?,
            ),
            mir::CallTarget::Primitive(role) => MachineCallTarget::Primitive(
                self.context
                    .primitives
                    .get(&role)
                    .ok_or_else(Self::invalid)?
                    .clone(),
            ),
        };
        self.call_answer(target, arguments, &call.answer)
    }
    pub(super) fn selected_call(&mut self, call: &mir::SelectedCall) -> Result<(), Failure> {
        let base = self.place(&call.carrier)?;
        let arguments = self.call_inputs(&call.inputs, false)?;
        let Some(MachineLayoutKind::CallableAlternatives {
            payload_offset,
            cases,
            ..
        }) = self
            .context
            .layouts
            .get(call.carrier.data)
            .map(crate::MachineLayout::kind)
        else {
            return Err(Self::invalid());
        };
        if cases.len() != call.cases.len()
            || cases
                .iter()
                .zip(&call.cases)
                .any(|(data, case)| *data != case.data)
        {
            return Err(Self::invalid());
        }
        let payload_offset = *payload_offset;
        self.select_callable_case(base, call.carrier.data, |builder, index| {
            let case = call.cases.get(index).ok_or_else(Self::invalid)?;
            let Some(RuntimeType::StaticCallable { environment }) =
                builder.context.program.runtime().types().get(case.data)
            else {
                return Err(Self::invalid());
            };
            let mut selected =
                Vec::with_capacity(arguments.len() + usize::from(environment.is_some()));
            if let Some(data) = *environment {
                // This address exists only inside the selected tag branch. Inactive environments
                // are never projected, loaded, copied or passed to any candidate body.
                let source = builder.offset_address(base, data, payload_offset)?;
                selected.push(builder.value(
                    data,
                    true,
                    MachineOperationKind::AddressOf { source },
                )?);
            }
            selected.extend_from_slice(&arguments);
            let target = MachineCallTarget::Direct(
                *builder
                    .context
                    .functions
                    .get(&case.target)
                    .ok_or_else(Self::invalid)?,
            );
            builder.call_answer(target, selected, &call.answer)
        })
    }

    pub(super) fn selected_drive(&mut self, drive: &mir::SelectedDrive) -> Result<(), Failure> {
        let selector = self.load(&drive.selector)?;
        let tags = drive.cases.iter().map(|(tag, _)| i128::from(*tag));
        self.select_integer_cases(selector, tags, |builder, index| {
            let &(_, child) = drive.cases.get(index).ok_or_else(Self::invalid)?;
            let target = MachineCallTarget::Direct(
                *builder
                    .context
                    .functions
                    .get(&child)
                    .ok_or_else(Self::invalid)?,
            );
            builder.call_answer(target, Vec::new(), &drive.answer)
        })
    }

    fn call_inputs(
        &mut self,
        inputs: &[mir::CallInput],
        primitive: bool,
    ) -> Result<Vec<MachineValueId>, Failure> {
        let mut arguments = Vec::new();
        for input in inputs {
            // Body read/edit channels receive an address. The trusted runtime's closed ABI
            // receives the descriptor value at that address; this load grants no source Copy.
            arguments.push(
                if input.operation == ValueInputOperation::Take || primitive {
                    self.load(&input.source)?
                } else {
                    let source = self.place(&input.source)?;
                    self.value(
                        input.source.data,
                        true,
                        MachineOperationKind::AddressOf { source },
                    )?
                },
            );
        }
        Ok(arguments)
    }

    fn call_answer(
        &mut self,
        target: MachineCallTarget,
        arguments: Vec<MachineValueId>,
        answer: &mir::Place,
    ) -> Result<(), Failure> {
        let value = self.value(
            answer.data,
            false,
            MachineOperationKind::Call(MachineCall::new(
                target,
                arguments,
                MachineCallAllocation::Inherit,
                None,
            )),
        )?;
        self.store(answer, value)
    }
}
