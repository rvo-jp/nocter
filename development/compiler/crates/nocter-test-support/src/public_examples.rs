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
    ExecutableFile {
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
    stdin: &'static [u8],
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
    pub const fn stdin(self) -> &'static [u8] {
        self.stdin
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
        directory: "subprocess-output",
        package_identity: "workspace:subprocess-output",
        executable: "subprocess-output",
        fixtures: &[PublicExampleFixture::ExecutableFile {
            path: "helper.sh",
            contents: include_bytes!("../../../../../examples/subprocess-output/helper.sh"),
        }],
        runs: &[PublicExampleRun {
            name: "captured-text",
            arguments: &[],
            stdin: b"",
            status: 0,
            stdout: include_bytes!("../../../../../examples/subprocess-output/sample-output.txt"),
            stderr: b"",
        }],
    },
    PublicPackageExample {
        directory: "subprocess-status",
        package_identity: "workspace:subprocess-status",
        executable: "subprocess-status",
        fixtures: &[PublicExampleFixture::ExecutableFile {
            path: "helper.sh",
            contents: include_bytes!("../../../../../examples/subprocess-status/helper.sh"),
        }],
        runs: &[PublicExampleRun {
            name: "nonzero-status",
            arguments: &[],
            stdin: b"",
            status: 0,
            stdout: b"helper exited with code 17\n",
            stderr: b"",
        }],
    },
    PublicPackageExample {
        directory: "json-normalize",
        package_identity: "workspace:json-normalize",
        executable: "json-normalize",
        fixtures: &[
            PublicExampleFixture::File {
                path: "valid.json",
                contents: b"  {\"items\": [true, \"\xc3\xa9\", 1E+2]}\n",
            },
            PublicExampleFixture::File {
                path: "invalid.json",
                contents: b"[1,]\n",
            },
        ],
        runs: &[
            PublicExampleRun {
                name: "usage",
                arguments: &[],
                stdin: b"",
                status: 2,
                stdout: b"",
                stderr: b"usage: json-normalize PATH\n",
            },
            PublicExampleRun {
                name: "success",
                arguments: &[PublicExampleArgument::FixturePath("valid.json")],
                stdin: b"",
                status: 0,
                stdout: b"{\"items\":[true,\"\xc3\xa9\",1E+2]}\n",
                stderr: b"",
            },
            PublicExampleRun {
                name: "invalid-json",
                arguments: &[PublicExampleArgument::FixturePath("invalid.json")],
                stdin: b"",
                status: 2,
                stdout: b"",
                stderr: b"json-normalize: std.json.invalid_syntax: invalid JSON syntax at byte 3\n",
            },
        ],
    },
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
                stdin: b"",
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
                stdin: b"",
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
                stdin: b"",
                status: 2,
                stdout: b"usage: file-summary PATH\n",
                stderr: b"",
            },
            PublicExampleRun {
                name: "success",
                arguments: &[PublicExampleArgument::FixturePath("input.txt")],
                stdin: b"",
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
                stdin: b"",
                status: 2,
                stdout: b"usage: text-report PATH NEEDLE\n",
                stderr: b"",
            },
            PublicExampleRun {
                name: "missing-needle",
                arguments: &[PublicExampleArgument::FixturePath("input.txt")],
                stdin: b"",
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
                stdin: b"",
                status: 0,
                stdout: b"lines: 3\nmatching: 2\n",
                stderr: b"",
            },
        ],
    },
    PublicPackageExample {
        directory: "text-banner",
        package_identity: "workspace:text-banner",
        executable: "text-banner",
        fixtures: &[],
        runs: &[
            PublicExampleRun {
                name: "usage",
                arguments: &[],
                stdin: b"",
                status: 2,
                stdout: b"",
                stderr: b"usage: text-banner TEXT\n",
            },
            PublicExampleRun {
                name: "success",
                arguments: &[PublicExampleArgument::Text("  alpha beta  ")],
                stdin: b"",
                status: 0,
                stdout: b"==========\ntext: alpha-beta\nbytes: 10\n==========\n",
                stderr: b"",
            },
        ],
    },
    PublicPackageExample {
        directory: "stdin-prefix",
        package_identity: "workspace:stdin-prefix",
        executable: "stdin-prefix",
        fixtures: &[],
        runs: &[
            PublicExampleRun {
                name: "usage",
                arguments: &[],
                stdin: b"",
                status: 2,
                stdout: b"",
                stderr: b"usage: stdin-prefix PREFIX\n",
            },
            PublicExampleRun {
                name: "sample",
                arguments: &[PublicExampleArgument::Text("> ")],
                stdin: include_bytes!("../../../../../examples/stdin-prefix/sample.txt"),
                status: 0,
                stdout: include_bytes!("../../../../../examples/stdin-prefix/sample-output.txt"),
                stderr: b"",
            },
            PublicExampleRun {
                name: "lines",
                arguments: &[PublicExampleArgument::Text("> ")],
                stdin: b"\nalpha\r\nlone\rbeta\n\xf0\x9f\x98\x80 split\nfinal",
                status: 0,
                stdout: b"> \n> alpha\n> lone\rbeta\n> \xf0\x9f\x98\x80 split\n> final\n",
                stderr: b"",
            },
            PublicExampleRun {
                name: "invalid-utf8",
                arguments: &[PublicExampleArgument::Text("> ")],
                stdin: b"good\nbad\xff\n",
                status: 2,
                stdout: b"> good\n",
                stderr: b"stdin-prefix: std.string.invalid_utf8: invalid UTF-8\n",
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
                stdin: b"",
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
                stdin: b"",
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
                stdin: b"",
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
                stdin: b"",
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
                stdin: b"",
                status: 2,
                stdout: b"",
                stderr: b"text-search: std.string.invalid_utf8: cannot read bad.txt\n",
            },
        ],
    },
];
