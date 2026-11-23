//! Import physical DAG edges and charge sparse storage before allocating or cloning them.
use super::{Data, Node, Value};
use crate::state::{
    Rule,
    budget::{Resident, Storage},
};
use std::{collections::BTreeMap, num::NonZeroU64, sync::Arc};
impl Storage {
    pub(in crate::state) fn primitive(&self, data: Data) -> Result<Value, Rule> {
        if data.composite() {
            return Err(Rule::InvalidPlan);
        }
        let allocation = self.items(data.storage_items()?)?;
        Ok(Value {
            node: Some(Arc::new(Node {
                data,
                allocation: Some(allocation),
            })),
        })
    }
    pub(in crate::state) fn variant(
        &self,
        primary: bool,
        payload: Option<Value>,
    ) -> Result<Value, Rule> {
        let allocation = self.items(usize::from(payload.is_some()))?;
        if payload
            .as_ref()
            .is_some_and(|value| !value.belongs_to(self))
        {
            return Err(Rule::InvalidPlan);
        }
        Ok(Value {
            node: Some(Arc::new(Node {
                data: Data::Variant { primary, payload },
                allocation: Some(allocation),
            })),
        })
    }
    pub(in crate::state) fn aggregate(
        &self,
        fields: impl ExactSizeIterator<Item = Value>,
    ) -> Result<Value, Rule> {
        self.sequence(fields, Data::Aggregate)
    }
    fn enumeration(
        &self,
        tag: u64,
        fields: impl ExactSizeIterator<Item = Value>,
    ) -> Result<Value, Rule> {
        self.sequence(fields, |fields| Data::Enum { tag, fields })
    }
    fn sequence(
        &self,
        fields: impl ExactSizeIterator<Item = Value>,
        data: impl FnOnce(Box<[Value]>) -> Data,
    ) -> Result<Value, Rule> {
        let allocation = self.items(fields.len())?;
        let fields = fields.collect::<Box<[_]>>();
        if fields.iter().any(|value| !value.belongs_to(self)) {
            return Err(Rule::InvalidPlan);
        }
        Ok(Value {
            node: Some(Arc::new(Node {
                data: data(fields),
                allocation: Some(allocation),
            })),
        })
    }
    pub(in crate::state) fn repeated(
        &self,
        element: Value,
        length: NonZeroU64,
        replacements: impl ExactSizeIterator<Item = (u64, Value)>,
    ) -> Result<Value, Rule> {
        let count = replacements.len();
        let units = Data::repeated_items(count)?;
        let allocation = self.items(units)?;
        let replacements: BTreeMap<_, _> = replacements.collect();
        if !element.belongs_to(self)
            || replacements.len() != count
            || replacements
                .iter()
                .any(|(&index, value)| index >= length.get() || !value.belongs_to(self))
        {
            return Err(Rule::InvalidPlan);
        }
        Ok(Value {
            node: Some(Arc::new(Node {
                data: Data::Repeated {
                    element,
                    length,
                    replacements,
                },
                allocation: Some(allocation),
            })),
        })
    }
    pub(in crate::state) fn import(&self, value: &Value) -> Result<Value, Rule> {
        if value.belongs_to(self) {
            return Ok(value.clone());
        }
        if !value.data().composite() {
            return self.primitive(value.data().clone());
        }
        let mut pending = vec![self.entry((value.clone(), false))?];
        let mut completed: BTreeMap<usize, Resident<Value>> = BTreeMap::new();
        while let Some(task) = pending.pop() {
            let (source, expanded) = &task.value;
            let identity = source.identity();
            if completed.contains_key(&identity) {
                continue;
            }
            if !source.belongs_to(self) && !*expanded && source.data().composite() {
                pending.push(self.entry((source.clone(), true))?);
                for field in source.data().children().rev() {
                    pending.push(self.entry((field.clone(), false))?);
                }
                continue;
            }
            let imported = |value: &Value| {
                completed
                    .get(&value.identity())
                    .expect("postorder value")
                    .value
                    .clone()
            };
            let value = if source.belongs_to(self) {
                source.clone()
            } else {
                match source.data() {
                    Data::Aggregate(fields) => self.aggregate(fields.iter().map(imported))?,
                    Data::Enum { tag, fields } => {
                        self.enumeration(*tag, fields.iter().map(imported))?
                    }
                    Data::Variant { primary, payload } => {
                        self.variant(*primary, payload.as_ref().map(imported))?
                    }
                    Data::Repeated {
                        element,
                        length,
                        replacements,
                    } => self.repeated(
                        imported(element),
                        *length,
                        replacements
                            .iter()
                            .map(|(&index, value)| (index, imported(value))),
                    )?,
                    data => self.primitive(data.clone())?,
                }
            };
            completed.insert(identity, self.entry(value)?);
        }
        completed
            .get(&value.identity())
            .map(|entry| entry.value.clone())
            .ok_or(Rule::InvalidPlan)
    }
}
