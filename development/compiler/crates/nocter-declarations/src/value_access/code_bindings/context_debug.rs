//! Diagnostic formatting retains graph sharing instead of recursively expanding instance trees.
use super::ConcreteWitness;
use crate::value_access::{DataInstanceKey, data_instances::Context};
use nocter_model::{TypeId, ValueCode, ValueInvocation};
use std::{collections::BTreeMap, fmt, sync::Arc};

#[derive(Clone, Copy)]
enum Node<'a> {
    Callable(&'a ConcreteWitness),
    Data(&'a DataInstanceKey),
}
#[derive(Eq, Ord, PartialEq, PartialOrd)]
enum Identity {
    Callable(ValueCode, ValueInvocation, Option<(TypeId, *const Context)>),
    Data(TypeId, *const Context),
}
fn data_identity(data: &DataInstanceKey) -> (TypeId, *const Context) {
    (data.data(), Arc::as_ptr(&data.context))
}
impl Node<'_> {
    fn identity(self) -> Identity {
        match self {
            Self::Callable(witness) => Identity::Callable(
                witness.code(),
                witness.invocation(),
                witness.environment().map(data_identity),
            ),
            Self::Data(data) => {
                let (data, context) = data_identity(data);
                Identity::Data(data, context)
            }
        }
    }
}
#[derive(Default)]
struct Graph<'a> {
    ids: BTreeMap<Identity, usize>,
    nodes: Vec<Node<'a>>,
}
impl<'a> Graph<'a> {
    fn intern(&mut self, node: Node<'a>) -> usize {
        let next = self.nodes.len();
        *self.ids.entry(node.identity()).or_insert_with(|| {
            self.nodes.push(node);
            next
        })
    }
}
struct Record(String);
impl fmt::Debug for Record {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}
fn format(root: Node<'_>, name: &str, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    let mut graph = Graph::default();
    let root = graph.intern(root);
    let mut records = Vec::new();
    while records.len() < graph.nodes.len() {
        let record = match graph.nodes[records.len()] {
            Node::Callable(witness) => {
                let environment = witness
                    .environment()
                    .map(|key| graph.intern(Node::Data(key)));
                format!(
                    "Callable {{ code: {:?}, invocation: {:?}, environment: {environment:?} }}",
                    witness.code(),
                    witness.invocation()
                )
            }
            Node::Data(key) => {
                let context = match &*key.context {
                    Context::Closed => "Closed".to_owned(),
                    Context::Function(witness) => {
                        format!("Function({})", graph.intern(Node::Callable(witness)))
                    }
                    Context::Components(children) => {
                        let children: Vec<_> = children
                            .iter()
                            .map(|(&field, key)| (field, graph.intern(Node::Data(key))))
                            .collect();
                        format!("Components({children:?})")
                    }
                };
                format!("Data {{ data: {:?}, context: {context} }}", key.data())
            }
        };
        records.push(Record(record));
    }
    formatter
        .debug_struct(name)
        .field("root", &root)
        .field("nodes", &records)
        .finish()
}
impl fmt::Debug for ConcreteWitness {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        format(Node::Callable(self), "ConcreteWitness", formatter)
    }
}
impl fmt::Debug for DataInstanceKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        format(Node::Data(self), "DataInstanceKey", formatter)
    }
}
