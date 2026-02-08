use std::process::exit;

use fluent_ftl_tools::{format::format_path, parse_cli_args, parse_file_args};

fn main() {
    let paths = parse_cli_args(parse_file_args).unwrap_or_else(|e| {
        eprintln!("{e}");
        exit(1);
    });
    let mut success = true;
    for path in paths {
        if let Err(e) = format_path(path) {
            eprintln!("{e}");
            success = false;
        }
    }
    if !success {
        exit(1);
    }
}
