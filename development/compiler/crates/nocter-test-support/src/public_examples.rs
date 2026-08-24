/// One argument supplied to a public package example.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PublicExampleArgument {
    /// The absolute path of the temporary input file owned by the test.
    InputPath,
    /// One authored command-line argument.
    Text(&'static str),
}

/// One file made available to every execution scenario for an example.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PublicExampleInput {
    name: &'static str,
    contents: &'static [u8],
}

impl PublicExampleInput {
    #[must_use]
    pub const fn name(self) -> &'static str {
        self.name
    }

    #[must_use]
    pub const fn contents(self) -> &'static [u8] {
        self.contents
    }
}

/// One exact process invocation required of a public package example.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PublicExampleRun {
    name: &'static str,
    arguments: &'static [PublicExampleArgument],
    status: i32,
    stdout: &'static [u8],
    stderr: &'static [u8],
}

impl PublicExampleRun {
    #[must_use]
    pub const fn name(self) -> &'static str {
        self.name
    }

    #[must_use]
    pub const fn arguments(self) -> &'static [PublicExampleArgument] {
        self.arguments
    }

    #[must_use]
    pub const fn status(self) -> i32 {
        self.status
    }

    #[must_use]
    pub const fn stdout(self) -> &'static [u8] {
        self.stdout
    }

    #[must_use]
    pub const fn stderr(self) -> &'static [u8] {
        self.stderr
    }
}

/// The repository-owned execution contract for one public package example.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PublicPackageExample {
    directory: &'static str,
    package_identity: &'static str,
    executable: &'static str,
    input: Option<PublicExampleInput>,
    runs: &'static [PublicExampleRun],
}

impl PublicPackageExample {
    #[must_use]
    pub const fn directory(self) -> &'static str {
        self.directory
    }

    #[must_use]
    pub const fn package_identity(self) -> &'static str {
        self.package_identity
    }

    #[must_use]
    pub const fn executable(self) -> &'static str {
        self.executable
    }

    #[must_use]
    pub const fn input(self) -> Option<PublicExampleInput> {
        self.input
    }

    #[must_use]
    pub const fn runs(self) -> &'static [PublicExampleRun] {
        self.runs
    }
}

/// Every public package example that must cross native compilation and execution.
pub const PUBLIC_PACKAGE_EXAMPLES: &[PublicPackageExample] = &[
    PublicPackageExample {
        directory: "file-summary",
        package_identity: "workspace:file-summary",
        executable: "file-summary",
        input: Some(PublicExampleInput {
            name: "input.txt",
            contents: b"first\nsecond\n",
        }),
        runs: &[
            PublicExampleRun {
                name: "usage",
                arguments: &[],
                status: 2,
                stdout: b"usage: file-summary PATH\n",
                stderr: b"",
            },
            PublicExampleRun {
                name: "success",
                arguments: &[PublicExampleArgument::InputPath],
                status: 0,
                stdout: b"2\n",
                stderr: b"",
            },
        ],
    },
    PublicPackageExample {
        directory: "text-report",
        package_identity: "workspace:text-report",
        executable: "text-report",
        input: Some(PublicExampleInput {
            name: "input.txt",
            contents: b"alpha\nbeta alpha\ngamma\n",
        }),
        runs: &[
            PublicExampleRun {
                name: "usage",
                arguments: &[],
                status: 2,
                stdout: b"usage: text-report PATH NEEDLE\n",
                stderr: b"",
            },
            PublicExampleRun {
                name: "missing-needle",
                arguments: &[PublicExampleArgument::InputPath],
                status: 2,
                stdout: b"usage: text-report PATH NEEDLE\n",
                stderr: b"",
            },
            PublicExampleRun {
                name: "success",
                arguments: &[
                    PublicExampleArgument::InputPath,
                    PublicExampleArgument::Text("alpha"),
                ],
                status: 0,
                stdout: b"lines: 3\nmatching: 2\n",
                stderr: b"",
            },
        ],
    },
];
