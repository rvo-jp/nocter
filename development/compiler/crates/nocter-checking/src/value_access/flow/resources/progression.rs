//! Atomic endpoint/backing updates following the checking-owned numerical prefix proof.
use super::{
    ArrayFrontier, Holder, INVALID, Missing, OwnedValue, Payload, Place, Result, RetainedIndex,
    State, Step, TypedQuery, payload,
};
use nocter_declarations::value_access::PayloadSchema;
use nocter_model::BuiltinType;

impl State {
    /// The caller supplies the current operation's closed prefix proof. This independently
    /// validates structural ownership/coverage; it never derives numbers from recipe identity.
    pub(in crate::value_access::flow) fn advance_array(
        &mut self,
        typed: &TypedQuery,
        index: RetainedIndex,
        frontier: ArrayFrontier,
        destination: Holder,
        steps: &mut Vec<Step>,
    ) -> Result<()> {
        let array = index.array(typed).ok_or(INVALID)?;
        if self.code != Some(typed.code()) || frontier.array(typed).as_ref() != Some(&array) {
            return Err(INVALID);
        }
        let mut staged = self.clone();
        let mut changes = Vec::new();
        let (array_payload, element, previous) = staged.prefix_payloads(typed, &array, frontier)?;
        staged.insert(destination, element, false)?;
        changes.push(Step::MoveArrayElement {
            array: array.clone(),
            index,
            destination,
            payload: element,
        });
        let endpoint = Holder::ArrayFrontier(frontier);
        if previous {
            let old = staged.remove_root(endpoint)?;
            changes.push(Step::Dispose {
                source: endpoint.place(),
                payload: old.payload,
            });
        }
        let source = Holder::Expression(index.operand()).place();
        let number = staged.take(typed.program(), &source)?;
        if typed.program().data_kind(number.data()) != Some(BuiltinType::Usize) {
            return Err(INVALID);
        }
        // Unframed: the prefix owns this scalar's lifetime. Payload delivery precedes reuse.
        staged.owned.insert(endpoint, OwnedValue::new(number));
        changes.push(Step::Move {
            source,
            destination: endpoint.place(),
            payload: number,
        });
        let owner = staged.owned.get_mut(&array.holder()).ok_or(INVALID)?;
        owner.missing.retain(|hole| !hole.under(array.fields()));
        owner.missing.insert(Missing::ArrayPrefix {
            array: array.fields().clone(),
            frontier,
        });
        changes.push(Step::ConsolidateArrayPrefix {
            array,
            payload: array_payload,
            frontier,
        });
        staged.retire_unused_backing(&mut changes)?;
        *self = staged;
        steps.extend(changes);
        Ok(())
    }
    fn prefix_payloads(
        &self,
        typed: &TypedQuery,
        array: &Place,
        frontier: ArrayFrontier,
    ) -> Result<(Payload, Payload, bool)> {
        let owner = self.owned.get(&array.holder()).ok_or(INVALID)?;
        let array_payload = owner.container(typed.program(), array.fields())?;
        if typed
            .program()
            .containing_destructor(owner.payload.data(), array.fields().fields())
            .is_some()
        {
            return Err(INVALID);
        }
        let Some(PayloadSchema::Array(schema)) = typed.program().data_schema(array_payload.data())
        else {
            return Err(INVALID);
        };
        if schema.is_empty() || schema.length() != frontier.bound() {
            return Err(INVALID);
        }
        let expected = Missing::ArrayPrefix {
            array: array.fields().clone(),
            frontier,
        };
        let overlapping: Vec<_> = owner
            .missing
            .iter()
            .filter(|hole| hole.overlaps(array.fields()))
            .collect();
        let previous = match overlapping.as_slice() {
            [] if !self.contains(Holder::ArrayFrontier(frontier)) => false,
            [missing] if **missing == expected => {
                let endpoint = self
                    .owned
                    .get(&Holder::ArrayFrontier(frontier))
                    .ok_or(INVALID)?;
                if !endpoint.missing.is_empty()
                    || typed.program().data_kind(endpoint.payload.data())
                        != Some(BuiltinType::Usize)
                {
                    return Err(INVALID);
                }
                true
            }
            _ => return Err(INVALID),
        };
        Ok((
            array_payload,
            payload(typed.program(), schema.element())?,
            previous,
        ))
    }
}
