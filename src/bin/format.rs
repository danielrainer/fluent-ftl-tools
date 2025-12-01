use std::{path::Path, process::exit};

use fluent_ftl_tools::{
    format_resource, parse_as_syntax_resource, parse_cli_args, parse_file_args,
    serialize_resource_to_file,
};

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

fn format_path<P: AsRef<Path>>(path: P) -> Result<(), String> {
    let path = path.as_ref();
    let resource = match parse_as_syntax_resource(path) {
        Ok(resource) => resource,
        Err(errors) => {
            return Err(format!("Failed to parse {path:?}:\n{errors}"));
        }
    };
    match format_resource(resource) {
        Ok(formatted_resource) => match serialize_resource_to_file(&formatted_resource, path) {
            Ok(()) => Ok(()),
            Err(e) => Err(format!(
                "Failed to serialize resource to file {path:?}:\n{e}",
            )),
        },
        Err(error_message) => Err(format!(
            "File {path:?} does not conform to the expected subset of FTL syntax:\n{error_message}"
        )),
    }
}
