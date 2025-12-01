use std::collections::HashSet;

use fluent::FluentResource;
use fluent_ftl_tools::{
    is_formatted, make_bundle, parse_as_fluent_resource, parse_cli_args,
    parse_file_args_with_required_file,
};
use fluent_syntax::ast::Entry;

fn print_help() {
    eprintln!(
        "Usage:\n\
        The first argument specifies the default language file, which must contain all message IDs.\n\
        The remaining arguments specify files whose message IDs must also appear in the default language file \
        and whose message variable usage should be checked for consistency.\n\
        Arguments can be paths to directories, in which case all files directly in the specified directory with the `.ftl` extension will be used.\n\
        The first argument must identify exactly one file.\n\
        If no second argument is given, all `.ftl` files in the current directory are used."
    );
}
fn main() {
    macro_rules! fail {
        ($fmt:literal $(, $arg:expr)* $(,)?) => {
            eprintln!($fmt $(, $arg)*);
            std::process::exit(1);
        };
    }
    let (default_language_path, other_paths) = parse_cli_args(parse_file_args_with_required_file)
        .unwrap_or_else(|e| {
            eprintln!("Error:\n{e}");
            print_help();
            std::process::exit(1);
        });
    let default_resource = match parse_as_fluent_resource(&default_language_path) {
        Ok(default_resource) => default_resource,
        Err(e) => {
            fail!("Error parsing {default_language_path:?}:\n{e}");
        }
    };
    let mut other_resources = Vec::with_capacity(other_paths.len());
    for path in &other_paths {
        match parse_as_fluent_resource(path) {
            Ok(resource) => other_resources.push((path, resource)),
            Err(e) => {
                fail!("Error parsing {path:?}:\n{e}");
            }
        }
    }
    if let Err(e) = check_internal_consistency(&default_resource) {
        fail!("Errors in {default_language_path:?}:\n{e}");
    }
    for (path, resource) in &other_resources {
        if let Err(e) = check_internal_consistency(resource) {
            fail!("Errors in {path:?}:\n{e}");
        }
    }
    for (path, resource) in &other_resources {
        if let Err(e) = check_for_extra_ids(&default_resource, resource) {
            fail!("Errors in {path:?}:\n{e}");
        }
        // TODO: Check if the variables used with each message match.
    }
}

fn check_internal_consistency(fluent_resource: &FluentResource) -> Result<(), String> {
    // Checks for duplicate definitions.
    let _ = make_bundle(fluent_resource)?;
    if !is_formatted(fluent_resource.source())? {
        return Err("File does not conform to expected formatting.\n".into());
    }
    // TODO: Check if referenced terms exist and their variables are set.
    // This requires recursive AST traversal.
    Ok(())
}

fn extract_messages(resource: &FluentResource) -> HashSet<&str> {
    let mut messages = HashSet::new();
    for entry in resource.entries() {
        if let Entry::Message(message) = entry {
            messages.insert(message.id.name);
        }
    }
    messages
}

fn check_for_extra_ids(default: &FluentResource, other: &FluentResource) -> Result<(), String> {
    let messages_default = extract_messages(default);
    let messages_other = extract_messages(other);
    let mut extra_messages = messages_other.difference(&messages_default).peekable();
    if extra_messages.peek().is_none() {
        return Ok(());
    }
    let mut error_message = String::from("Unexpected message identifiers found:\n");
    for message in extra_messages {
        error_message.push_str(message);
        error_message.push('\n');
    }
    Err(error_message)
}
