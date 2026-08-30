/// One argument supplied to a public package example.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PublicExampleArgument {
    /// One path relative to the temporary fixture root.
    FixturePath(&'static str),
    /// One authored command-line argument.
    Text(&'static str),
}

/// One filesystem entry made available to every execution scenario for an example.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PublicExampleFixture {
    File {
        path: &'static str,
        contents: &'static [u8],
    },
    Directory {
        path: &'static str,
    },
    Symlink {
        path: &'static str,
        target: &'static str,
    },
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
    fixtures: &'static [PublicExampleFixture],
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
    pub const fn fixtures(self) -> &'static [PublicExampleFixture] {
        self.fixtures
    }

    #[must_use]
    pub const fn runs(self) -> &'static [PublicExampleRun] {
        self.runs
    }
}

/// Every public package example that must cross native compilation and execution.
pub const PUBLIC_PACKAGE_EXAMPLES: &[PublicPackageExample] = &[
    PublicPackageExample {
        directory: "line-frequency",
        package_identity: "workspace:line-frequency",
        executable: "line-frequency",
        fixtures: &[PublicExampleFixture::File {
            path: "input.txt",
            contents: b"alpha\nbeta\nalpha\n",
        }],
        runs: &[
            PublicExampleRun {
                name: "usage",
                arguments: &[],
                status: 2,
                stdout: b"usage: line-frequency PATH NEEDLE\n",
                stderr: b"",
            },
            PublicExampleRun {
                name: "success",
                arguments: &[
                    PublicExampleArgument::FixturePath("input.txt"),
                    PublicExampleArgument::Text("alpha"),
                ],
                status: 0,
                stdout: b"lines: 3\nunique: 2\nmatching: 2\n",
                stderr: b"",
            },
        ],
    },
    PublicPackageExample {
        directory: "file-summary",
        package_identity: "workspace:file-summary",
        executable: "file-summary",
        fixtures: &[PublicExampleFixture::File {
            path: "input.txt",
            contents: b"first\nsecond\n",
        }],
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
                arguments: &[PublicExampleArgument::FixturePath("input.txt")],
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
        fixtures: &[PublicExampleFixture::File {
            path: "input.txt",
            contents: b"alpha\nbeta alpha\ngamma\n",
        }],
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
                arguments: &[PublicExampleArgument::FixturePath("input.txt")],
                status: 2,
                stdout: b"usage: text-report PATH NEEDLE\n",
                stderr: b"",
            },
            PublicExampleRun {
                name: "success",
                arguments: &[
                    PublicExampleArgument::FixturePath("input.txt"),
                    PublicExampleArgument::Text("alpha"),
                ],
                status: 0,
                stdout: b"lines: 3\nmatching: 2\n",
                stderr: b"",
            },
        ],
    },
    PublicPackageExample {
        directory: "text-search",
        package_identity: "workspace:text-search",
        executable: "text-search",
        fixtures: &[
            PublicExampleFixture::File {
                path: "tree/zeta.txt",
                contents: b"needle zeta\nquiet\n",
            },
            PublicExampleFixture::File {
                path: "tree/alpha.txt",
                contents: b"first\nneedle alpha\n",
            },
            PublicExampleFixture::File {
                path: "tree/nested/middle.txt",
                contents: b"needle middle\nlast needle\n",
            },
            PublicExampleFixture::Symlink {
                path: "tree/nested/loop",
                target: "..",
            },
            PublicExampleFixture::Directory { path: "tree/empty" },
            PublicExampleFixture::File {
                path: "invalid/bad.txt",
                contents: b"ok\nbad\xff\n",
            },
        ],
        runs: &[
            PublicExampleRun {
                name: "usage",
                arguments: &[],
                status: 2,
                stdout: b"",
                stderr: b"usage: text-search NEEDLE ROOT\n",
            },
            PublicExampleRun {
                name: "matches",
                arguments: &[
                    PublicExampleArgument::Text("needle"),
                    PublicExampleArgument::FixturePath("tree"),
                ],
                status: 0,
                stdout: concat!(
                    "alpha.txt:2:needle alpha\n",
                    "nested/middle.txt:1:needle middle\n",
                    "nested/middle.txt:2:last needle\n",
                    "zeta.txt:1:needle zeta\n",
                )
                .as_bytes(),
                stderr: b"",
            },
            PublicExampleRun {
                name: "no-match",
                arguments: &[
                    PublicExampleArgument::Text("absent"),
                    PublicExampleArgument::FixturePath("tree"),
                ],
                status: 1,
                stdout: b"",
                stderr: b"",
            },
            PublicExampleRun {
                name: "missing-root",
                arguments: &[
                    PublicExampleArgument::Text("needle"),
                    PublicExampleArgument::FixturePath("missing"),
                ],
                status: 2,
                stdout: b"",
                stderr: b"text-search: std.io.not_found: cannot read directory missing\n",
            },
            PublicExampleRun {
                name: "invalid-utf8",
                arguments: &[
                    PublicExampleArgument::Text("bad"),
                    PublicExampleArgument::FixturePath("invalid"),
                ],
                status: 2,
                stdout: b"",
                stderr: b"text-search: std.string.invalid_utf8: cannot read bad.txt\n",
            },
        ],
    },
];
