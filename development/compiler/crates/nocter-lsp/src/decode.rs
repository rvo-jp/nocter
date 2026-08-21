use nocter_json::{Member, Value};

use crate::{ParameterError, ParameterErrorKind};

pub(crate) fn required(value: Option<Value>, path: &str) -> Result<Value, ParameterError> {
    value.ok_or_else(|| ParameterError::new(ParameterErrorKind::MissingField, path))
}

pub(crate) fn string(value: Value, path: &str) -> Result<Box<str>, ParameterError> {
    match value {
        Value::String(value) => Ok(value),
        _ => Err(ParameterError::new(
            ParameterErrorKind::ExpectedString,
            path,
        )),
    }
}

pub(crate) fn integer(value: Value, path: &str) -> Result<i32, ParameterError> {
    match value {
        Value::Number(value) => value
            .parse()
            .map_err(|_| ParameterError::new(ParameterErrorKind::ExpectedInteger, path)),
        _ => Err(ParameterError::new(
            ParameterErrorKind::ExpectedInteger,
            path,
        )),
    }
}

pub(crate) fn unsigned(value: Value, path: &str) -> Result<u32, ParameterError> {
    match value {
        Value::Number(value) => value
            .parse()
            .map_err(|_| ParameterError::new(ParameterErrorKind::ExpectedNonnegativeInteger, path)),
        _ => Err(ParameterError::new(
            ParameterErrorKind::ExpectedNonnegativeInteger,
            path,
        )),
    }
}

pub(crate) fn boolean(value: &Value, path: &str) -> Result<bool, ParameterError> {
    match value {
        Value::Bool(value) => Ok(*value),
        _ => Err(ParameterError::new(
            ParameterErrorKind::ExpectedBoolean,
            path,
        )),
    }
}

pub(crate) fn array(value: Value, path: &str) -> Result<Vec<Value>, ParameterError> {
    match value {
        Value::Array(value) => Ok(value),
        _ => Err(ParameterError::new(ParameterErrorKind::ExpectedArray, path)),
    }
}

pub(crate) struct Object {
    members: Vec<Member>,
    path: Box<str>,
}

impl Object {
    pub(crate) fn new(value: Value, path: impl Into<Box<str>>) -> Result<Self, ParameterError> {
        let path = path.into();
        match value {
            Value::Object(members) => Ok(Self { members, path }),
            _ => Err(ParameterError::new(
                ParameterErrorKind::ExpectedObject,
                path,
            )),
        }
    }

    pub(crate) fn contains(&self, name: &str) -> bool {
        self.members
            .iter()
            .any(|member| member.name.as_ref() == name)
    }

    pub(crate) fn take(&mut self, name: &str) -> Result<Value, ParameterError> {
        self.take_optional(name)?.ok_or_else(|| {
            ParameterError::new(ParameterErrorKind::MissingField, self.field_path(name))
        })
    }

    pub(crate) fn take_optional(&mut self, name: &str) -> Result<Option<Value>, ParameterError> {
        let Some(index) = self
            .members
            .iter()
            .position(|member| member.name.as_ref() == name)
        else {
            return Ok(None);
        };
        if self.members[index + 1..]
            .iter()
            .any(|member| member.name.as_ref() == name)
        {
            return Err(ParameterError::new(
                ParameterErrorKind::DuplicateField,
                self.field_path(name),
            ));
        }
        Ok(Some(self.members.remove(index).value))
    }

    fn field_path(&self, name: &str) -> Box<str> {
        format!("{}.{name}", self.path).into_boxed_str()
    }
}
