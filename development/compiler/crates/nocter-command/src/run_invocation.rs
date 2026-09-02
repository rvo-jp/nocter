use std::ffi::{OsStr, OsString};
use std::fmt;
use std::process::Command;

/// Opaque operating-system arguments forwarded to one program launched by `nocter run`.
///
/// These values exclude the launcher-provided argument zero. They remain native OS strings from
/// command parsing through process launch and are never compiler inputs.
#[derive(Default)]
pub struct RunProgramArguments {
    values: Box<[OsString]>,
}

impl RunProgramArguments {
    #[must_use]
    pub fn new(values: impl IntoIterator<Item = OsString>) -> Self {
        Self {
            values: values.into_iter().collect(),
        }
    }

    #[cfg(test)]
    pub(crate) const fn as_slice(&self) -> &[OsString] {
        &self.values
    }

    #[cfg(test)]
    pub(crate) const fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub(crate) fn apply_to(self, command: &mut Command) {
        command.args(self.values);
    }
}

impl fmt::Debug for RunProgramArguments {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RunProgramArguments")
            .field("count", &self.values.len())
            .finish_non_exhaustive()
    }
}

/// The sole structural partition of a `run` invocation.
pub(crate) struct PartitionedRunInvocation {
    compiler_arguments: Vec<OsString>,
    program_arguments: RunProgramArguments,
}

impl PartitionedRunInvocation {
    pub(crate) fn into_parts(self) -> (Vec<OsString>, RunProgramArguments) {
        (self.compiler_arguments, self.program_arguments)
    }
}

/// Removes the first standalone separator and closes both sides of a `run` invocation.
pub(crate) fn partition_run_invocation(
    arguments: impl IntoIterator<Item = OsString>,
) -> PartitionedRunInvocation {
    let mut compiler_arguments = Vec::new();
    let mut arguments = arguments.into_iter();
    let program_arguments = loop {
        match arguments.next() {
            Some(argument) if argument == OsStr::new("--") => {
                break RunProgramArguments::new(arguments);
            }
            Some(argument) => compiler_arguments.push(argument),
            None => break RunProgramArguments::default(),
        }
    };
    PartitionedRunInvocation {
        compiler_arguments,
        program_arguments,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_separator_closes_compiler_arguments_and_preserves_the_opaque_tail() {
        let partitioned = partition_run_invocation([
            "source.nct".into(),
            "--".into(),
            "--target".into(),
            "".into(),
            "--".into(),
        ]);
        let (compiler, program) = partitioned.into_parts();

        assert_eq!(compiler, [OsString::from("source.nct")]);
        assert_eq!(
            program.as_slice(),
            [
                OsString::from("--target"),
                OsString::new(),
                OsString::from("--"),
            ]
        );
    }

    #[test]
    fn invocation_without_a_separator_has_no_program_arguments() {
        let partitioned = partition_run_invocation(["source.nct".into()]);
        let (compiler, program) = partitioned.into_parts();

        assert_eq!(compiler, [OsString::from("source.nct")]);
        assert!(program.is_empty());
    }

    #[test]
    fn debug_output_reports_shape_without_revealing_argument_values() {
        let arguments = RunProgramArguments::new([OsString::from("private-value")]);
        let rendered = format!("{arguments:?}");

        assert!(rendered.contains("count: 1"));
        assert!(!rendered.contains("private-value"));
    }

    #[cfg(unix)]
    #[test]
    fn partition_does_not_decode_native_operating_system_arguments() {
        use std::os::unix::ffi::{OsStrExt, OsStringExt};

        let native = OsString::from_vec(vec![0xff, b'a']);
        let partitioned = partition_run_invocation(["--".into(), native]);
        let (_, program) = partitioned.into_parts();

        assert_eq!(program.as_slice()[0].as_os_str().as_bytes(), &[0xff, b'a']);
    }
}
