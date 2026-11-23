//! Resolve a closure's formal inputs before the body forms their named bindings.
use crate::value_access::{
    Failure, Rule,
    annotations::Annotation,
    checker::Checker,
    ir::{BodyInput, Local, LocalOrigin},
};
use nocter_declarations::value_access::{
    FunctionConstraint, FunctionDataPattern, FunctionInput, FunctionPattern,
};
use nocter_model::{AccessStatementId, TypeId, ValueInputOperation};
use nocter_syntax::{ContextualSpelling, NodeId, NodeKind, SyntaxOrigin};

impl Checker<'_, '_> {
    fn closure_pattern_input(
        &mut self,
        pattern: &FunctionDataPattern,
        position: usize,
        ordinal: &mut usize,
        syntax: SyntaxOrigin,
    ) -> Result<nocter_model::TypeId, Failure> {
        let data = match pattern {
            FunctionDataPattern::Data(data) => return Ok(*data),
            FunctionDataPattern::Function(pattern) => {
                let data = self.closure_input_data(
                    Annotation::Function((**pattern).clone()),
                    position,
                    Some(*ordinal),
                    syntax,
                )?;
                *ordinal += 1;
                return Ok(data);
            }
            FunctionDataPattern::Optional(payload) => {
                let payload = self.closure_pattern_input(payload, position, ordinal, syntax)?;
                self.types.optional(payload)
            }
            FunctionDataPattern::Fallible(payload) => {
                let payload = self.closure_pattern_input(payload, position, ordinal, syntax)?;
                self.types.fallible(payload)
            }
            FunctionDataPattern::Tuple(fields) => {
                let fields = fields
                    .iter()
                    .map(|field| self.closure_pattern_input(field, position, ordinal, syntax))
                    .collect::<Result<Vec<_>, _>>()?;
                self.types.tuple(&fields)
            }
            FunctionDataPattern::Array { element, length } => {
                let element = self.closure_pattern_input(element, position, ordinal, syntax)?;
                self.types.array(element, *length)
            }
        };
        data.map_err(|_| Failure::InconsistentSyntax)
    }
    pub(in crate::value_access) fn closure_input_data(
        &mut self,
        annotation: Annotation,
        position: usize,
        component: Option<usize>,
        syntax: SyntaxOrigin,
    ) -> Result<nocter_model::TypeId, Failure> {
        let Annotation::Function(pattern) = annotation else {
            return annotation.data().ok_or(Failure::InconsistentSyntax);
        };
        let unsupported = |checker: &Self| {
            checker.reject_message(Rule::UnknownType, syntax,
            "this anonymous callable input needs established ordinary signature data; deeper callable input/result witnesses are not qualified yet".into(), Vec::new())
        };
        let FunctionDataPattern::Data(result) = pattern.result else {
            return unsupported(self);
        };
        let inputs = pattern
            .inputs
            .into_iter()
            .map(|input| match input.data {
                FunctionDataPattern::Data(data) => Ok(FunctionInput {
                    data,
                    operation: input.operation,
                }),
                _ => Err(()),
            })
            .collect::<Result<Box<[_]>, _>>();
        let Ok(inputs) = inputs else {
            return unsupported(self);
        };
        let nocter_model::ValueCode::Closure(owner) = self.code else {
            return Err(Failure::InconsistentSyntax);
        };
        let contract = FunctionConstraint {
            invocation: pattern.invocation,
            inputs,
            result,
        };
        let data = match component {
            Some(ordinal) => self
                .types
                .closure_component(owner, position, ordinal, contract),
            None => self.types.closure_parameter(owner, position, contract),
        };
        match data {
            Ok(data) => Ok(data),
            Err(nocter_declarations::value_access::BodyTypeError::UnsupportedCallableSignature) => {
                unsupported(self)
            }
            Err(_) => Err(Failure::InconsistentSyntax),
        }
    }
    pub(super) fn closure_inputs(
        &mut self,
        parameters: NodeId,
        expected: Option<&FunctionPattern>,
        entry: &mut Vec<AccessStatementId>,
    ) -> Result<Vec<FunctionInput>, Failure> {
        let mut inputs = Vec::new();
        let parameters_syntax = self.children(parameters);
        if expected.is_some_and(|contract| contract.inputs.len() != parameters_syntax.len()) {
            return self.reject_message(
                Rule::TypeMismatch,
                parameters,
                "the number of closure inputs does not match the selected callable contract".into(),
                Vec::new(),
            );
        }
        for (index, parameter) in parameters_syntax.into_iter().enumerate() {
            let selected = expected.map(|contract| &contract.inputs[index]);
            let origin = if let Ok(pattern) = self.direct(parameter, NodeKind::BindingPattern) {
                SyntaxOrigin::Node(pattern)
            } else {
                let token = self.token(parameter)?;
                let name = self.text(token)?;
                if self.shadows_name(name, token)? {
                    return self.reject(Rule::ShadowedName, token);
                }
                SyntaxOrigin::Token(token)
            };
            let data = self.select_closure_input_data(parameter, index, selected)?;
            let operation = self
                .select_closure_input_operation(parameter, selected.map(|input| input.operation))?;
            let local = self.locals.insert(Local {
                data,
                kind: operation.binding_kind(),
                origin: LocalOrigin::ClosureParameter,
            });
            self.inputs.push(BodyInput { local, operation });
            self.origins.locals.insert(local, self.site(origin)?);
            match origin {
                SyntaxOrigin::Node(pattern) => self.form_closure_pattern(pattern, local, entry)?,
                SyntaxOrigin::Token(name) => self.bind_local(self.text(name)?.to_owned(), local),
            }
            inputs.push(FunctionInput { data, operation });
        }
        Ok(inputs)
    }

    fn select_closure_input_data(
        &mut self,
        parameter: NodeId,
        index: usize,
        selected: Option<&FunctionInput<FunctionDataPattern>>,
    ) -> Result<TypeId, Failure> {
        let annotation = if let Ok(ty) = self.direct(parameter, NodeKind::Type) {
            let annotation = self.closure_input_annotation(ty, index)?;
            if selected.is_some_and(|selected| match (&annotation, &selected.data) {
                (Annotation::Data(actual), FunctionDataPattern::Data(required)) => {
                    *actual != *required
                        && !self.program().matches_compound_input(*required, *actual)
                }
                (Annotation::Data(actual), pattern) => self
                    .program()
                    .constrain_data_pattern(*actual, pattern)
                    .is_err(),
                (Annotation::Function(actual), FunctionDataPattern::Function(required)) => {
                    actual != &**required
                }
                (Annotation::Function(actual), FunctionDataPattern::Data(required)) => {
                    self.contextual_type(*required).map_or(true, |expected| {
                        expected != Annotation::Function(actual.clone())
                    })
                }
                _ => true,
            }) {
                return self.reject_message(
                    Rule::TypeMismatch,
                    ty,
                    "the explicit parameter data does not match the selected callable contract"
                        .into(),
                    Vec::new(),
                );
            }
            annotation
        } else if let Some(selected) = selected {
            match &selected.data {
                FunctionDataPattern::Data(data) => self.contextual_type(*data)?,
                FunctionDataPattern::Function(pattern) => Annotation::Function((**pattern).clone()),
                compound => Annotation::Data(self.closure_pattern_input(
                    compound,
                    index,
                    &mut 0,
                    SyntaxOrigin::Node(parameter),
                )?),
            }
        } else {
            return self.reject_message(Rule::UnknownType, parameter, "this private closure scope needs parameter data from an expected contract or explicit annotation".into(), Vec::new());
        };
        self.closure_input_data(annotation, index, None, SyntaxOrigin::Node(parameter))
    }

    fn select_closure_input_operation(
        &self,
        parameter: NodeId,
        selected: Option<ValueInputOperation>,
    ) -> Result<ValueInputOperation, Failure> {
        let Ok(operation) = self.direct(parameter, NodeKind::InputOperation) else {
            return Ok(selected.unwrap_or(ValueInputOperation::Read));
        };
        let explicit = match ContextualSpelling::from_spelling(self.text(self.token(operation)?)?) {
            Some(ContextualSpelling::Edit) => ValueInputOperation::Edit,
            Some(ContextualSpelling::Take) => ValueInputOperation::Take,
            _ => return Err(Failure::InconsistentSyntax),
        };
        if selected.is_some_and(|required| required != explicit) {
            return self.reject_message(
                Rule::TypeMismatch,
                operation,
                "the explicit parameter operation does not match the selected callable contract"
                    .into(),
                Vec::new(),
            );
        }
        Ok(explicit)
    }
}
