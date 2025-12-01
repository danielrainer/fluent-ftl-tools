use std::{collections::HashSet, path::Path};

use fluent_ftl_tools::{
    get_message_ids, parse_as_syntax_resource, parse_cli_args, parse_file_args_with_required_file,
};

fn print_help() {
    eprintln!(
        "Usage:\n\
        The first argument specifies the file containing the expected message IDs.\n\
        For the remaining arguments, the message IDs appearing in the first argument file but not in them will be printed.\n\
        Arguments can be paths to directories, in which case all files directly in the specified directory with the `.ftl` extension will be used.\n\
        The first argument must identify exactly one file.\n\
        If no second argument is given, all `.ftl` files in the current directory are used."
    );
}

fn main() {
    let (expected_ids_path, other_paths) = parse_cli_args(parse_file_args_with_required_file)
        .unwrap_or_else(|e| {
            eprintln!("Error:\n{e}");
            print_help();
            std::process::exit(1);
        });
    let expected = match parse_as_syntax_resource(&expected_ids_path) {
        Ok(expected) => expected,
        Err(e) => {
            eprintln!("Failed to parse expected message IDs file {expected_ids_path:?}:\n{e}");
            std::process::exit(1);
        }
    };
    let expected_ids = get_message_ids(&expected);
    let mut success = true;
    for path in other_paths {
        match find_missing_message_ids(&expected_ids, &path) {
            Ok(missing) => {
                if missing.is_empty() {
                    println!(
                        "All message IDs present in {expected_ids_path:?} are also present in {path:?}."
                    );
                } else {
                    let mut message = format!(
                        "Message IDs present in {expected_ids_path:?} but not in {path:?}:\n"
                    );
                    for id in missing {
                        message.push_str(id);
                        message.push('\n');
                    }
                    println!("{message}");
                }
            }
            Err(e) => {
                eprintln!("{e}");
                success = false;
            }
        }
    }
    if !success {
        std::process::exit(1);
    }
}

pub fn find_missing_message_ids<'a, P: AsRef<Path>>(
    expected_ids: &HashSet<&'a str>,
    path: P,
) -> Result<Vec<&'a str>, String> {
    let resource = match parse_as_syntax_resource(&path) {
        Ok(resource) => resource,
        Err(e) => {
            return Err(format!("Failed to parse {:?}:\n{e}", path.as_ref()));
        }
    };
    let present_ids = get_message_ids(&resource);
    let mut missing = vec![];
    for &id in expected_ids {
        if !present_ids.contains(id) {
            missing.push(id);
        }
    }
    // Ensure consistent order
    missing.sort();
    Ok(missing)
}
