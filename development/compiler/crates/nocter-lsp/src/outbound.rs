use std::collections::BTreeMap;
use std::fmt;

use nocter_json::Value;

use crate::{RequestId, ResponseResult, render_request};

/// One rendered server-to-client request retained beside its correlation identity.
#[derive(Clone, Debug)]
pub struct OutboundRequest {
    id: RequestId,
    method: Box<str>,
    body: String,
}

impl OutboundRequest {
    #[must_use]
    pub const fn id(&self) -> &RequestId {
        &self.id
    }

    #[must_use]
    pub const fn method(&self) -> &str {
        &self.method
    }

    #[must_use]
    pub fn body(&self) -> &str {
        &self.body
    }
}

/// One correlated response with the exact method identity retained by its request.
#[derive(Clone, Debug, PartialEq)]
pub struct CompletedRequest {
    id: RequestId,
    method: Box<str>,
    result: ResponseResult,
}

impl CompletedRequest {
    #[must_use]
    pub const fn id(&self) -> &RequestId {
        &self.id
    }

    #[must_use]
    pub const fn method(&self) -> &str {
        &self.method
    }

    #[must_use]
    pub const fn result(&self) -> &ResponseResult {
        &self.result
    }
}

/// Monotonic server-request identity allocator and exact pending-response table.
#[derive(Debug, Default)]
pub struct OutboundRequests {
    next: i32,
    pending: BTreeMap<RequestId, Box<str>>,
}

impl OutboundRequests {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            next: 0,
            pending: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.pending.len()
    }

    /// Allocates and retains one request before returning its rendered body.
    ///
    /// # Errors
    ///
    /// Returns an error when the signed JSON-RPC identity domain is exhausted.
    pub fn begin(
        &mut self,
        method: impl Into<Box<str>>,
        params: &Value,
    ) -> Result<OutboundRequest, OutboundRequestError> {
        let next = self
            .next
            .checked_add(1)
            .ok_or(OutboundRequestError::IdentityExhausted)?;
        self.next = next;
        let id = RequestId::Integer(next);
        let method = method.into();
        let previous = self.pending.insert(id.clone(), method.clone());
        debug_assert!(previous.is_none(), "monotonic request id must be unique");
        Ok(OutboundRequest {
            body: render_request(&id, &method, params),
            id,
            method,
        })
    }

    /// Correlates one response exactly once.
    ///
    /// # Errors
    ///
    /// Returns an error when the client replies with an unknown or already completed identity.
    pub fn complete(
        &mut self,
        id: RequestId,
        result: ResponseResult,
    ) -> Result<CompletedRequest, OutboundRequestError> {
        let method = self
            .pending
            .remove(&id)
            .ok_or_else(|| OutboundRequestError::UnknownResponse(id.clone()))?;
        Ok(CompletedRequest { id, method, result })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OutboundRequestError {
    IdentityExhausted,
    UnknownResponse(RequestId),
}

impl fmt::Display for OutboundRequestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IdentityExhausted => {
                formatter.write_str("language-server request identity space is exhausted")
            }
            Self::UnknownResponse(id) => {
                write!(formatter, "client response has unknown request id {id:?}")
            }
        }
    }
}

impl std::error::Error for OutboundRequestError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn correlates_each_response_to_one_retained_method() {
        let mut requests = OutboundRequests::new();
        let request = requests
            .begin("client/registerCapability", &Value::Object(Vec::new()))
            .unwrap();
        assert_eq!(request.id(), &RequestId::Integer(1));
        assert!(request.body().contains("\"id\":1"));
        assert_eq!(requests.len(), 1);

        let completed = requests
            .complete(RequestId::Integer(1), ResponseResult::Success(Value::Null))
            .unwrap();
        assert_eq!(completed.method(), "client/registerCapability");
        assert!(requests.is_empty());
        assert!(matches!(
            requests.complete(RequestId::Integer(1), ResponseResult::Success(Value::Null)),
            Err(OutboundRequestError::UnknownResponse(RequestId::Integer(1)))
        ));
    }
}
