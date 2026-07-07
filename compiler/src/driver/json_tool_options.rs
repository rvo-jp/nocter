use std::ffi::OsString;
use std::path::PathBuf;

pub(super) fn parse_json_tool_command(args: &[OsString]) -> Result<PathBuf, String> {
    if args.is_empty() {
        unreachable!("parse_json_tool_command requires a command");
    }

    if args.len() == 1 {
        return Err("missing source file".to_string());
    }

    if args.len() == 2 {
        return Err("missing `--format json`".to_string());
    }

    if !is_arg(&args[2], "--format") {
        return Err(format!(
            "unexpected argument `{}`",
            args[2].to_string_lossy()
        ));
    }

    if args.len() == 3 {
        return Err("expected `--format json`".to_string());
    }

    if !is_arg(&args[3], "json") {
        return Err("expected `--format json`".to_string());
    }

    if args.len() > 4 {
        return Err(format!(
            "unexpected argument `{}`",
            args[4].to_string_lossy()
        ));
    }

    Ok(PathBuf::from(args[1].clone()))
}

fn is_arg(arg: &OsString, expected: &str) -> bool {
    arg.to_string_lossy() == expected
}
