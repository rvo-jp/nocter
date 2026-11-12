//! Result substitution may add captured-environment edges; headers already own nominal closure.
use super::{BodyTypeError, Program};
use crate::value_access::{AggregateForm, FunctionWitness, PayloadSchema};
use nocter_dependency_query::{
    DependencyProgress, DependencyQuery, DependencyQueryError, DependencyStep,
};
use nocter_model::{TypeId, ValueFunctionOrigin};

pub(super) fn validate(program: &Program) -> Result<(), BodyTypeError> {
    let mut query = DependencyQuery::default();
    let mut domain = Environments(program);
    for (&id, schema) in &program.closures {
        let data = program
            .function_data(FunctionWitness {
                origin: ValueFunctionOrigin::Closure(id),
                invocation: schema.contract().invocation,
            })
            .ok_or(BodyTypeError::InvalidExtension)?;
        match query.resolve_steps(&mut domain, data) {
            Ok(_) => {}
            Err(DependencyQueryError::Computation(error)) => return Err(error),
            Err(DependencyQueryError::Cycle(path)) => {
                let id = path
                    .iter()
                    .find_map(|&data| match program.data_schema(data) {
                        Some(PayloadSchema::Function(FunctionWitness {
                            origin: ValueFunctionOrigin::Closure(id),
                            ..
                        })) => Some(id),
                        _ => None,
                    })
                    .ok_or(BodyTypeError::InvalidExtension)?;
                return Err(BodyTypeError::RecursiveEnvironment(id));
            }
        }
    }
    Ok(())
}
struct Environments<'a>(&'a Program);
impl DependencyProgress<TypeId, (), BodyTypeError> for Environments<'_> {
    fn advance(
        &mut self,
        query: &DependencyQuery<TypeId, ()>,
        data: TypeId,
    ) -> Result<DependencyStep<TypeId, ()>, BodyTypeError> {
        let children: Vec<_> = match self
            .0
            .data_schema(data)
            .ok_or(BodyTypeError::InvalidExtension)?
        {
            PayloadSchema::Function(FunctionWitness {
                origin: ValueFunctionOrigin::Closure(id),
                ..
            }) => vec![
                self.0
                    .closure(id)
                    .ok_or(BodyTypeError::InvalidExtension)?
                    .environment(),
            ],
            PayloadSchema::Aggregate(schema)
                if matches!(schema.form(), AggregateForm::Tuple(_)) =>
            {
                schema
                    .components()
                    .map(|component| component.data)
                    .collect()
            }
            PayloadSchema::Array(array) if !array.is_empty() => vec![array.element()],
            // Ready nominal schemas cannot contain body-owned code families in this domain.
            // Their finite-containment proof stays with header publication. Named functions own
            // only unit, not their inputs/results. Unknown result/formal payloads gain no proof:
            // executable admission still requires their separate closed-payload evidence.
            _ => Vec::new(),
        };
        let pending = children
            .into_iter()
            .filter(|child| query.completed(child).is_none())
            .collect::<Vec<_>>();
        Ok(if pending.is_empty() {
            DependencyStep::Complete(())
        } else {
            DependencyStep::Require(pending.into())
        })
    }
    fn no_progress(&mut self, _: TypeId) -> BodyTypeError {
        BodyTypeError::InvalidExtension
    }
}
