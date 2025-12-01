//! Program for renaming IDs and variables in FTL files.
//! Call without arguments to see usage information.

use std::{
    collections::HashMap,
    hash::Hash,
    path::{Path, PathBuf},
};

use fluent_ftl_tools::{
    format_resource, get_message_ids, parse_as_syntax_resource, parse_cli_args, parse_file_args,
    serialize_resources_to_files,
};
use fluent_syntax::ast::{
    Expression, Identifier, InlineExpression, Message, Pattern, PatternElement, Resource,
};

fn print_help() {
    eprintln!(
        "\n\
        Usage:\n\
        This program has a single mandatory argument at the first position. \
        It must be an ID which appears in the FTL files the program operates on. \
        If the ID should be renamed, use the format 'old_id=new_id'.\n\
        If any variables used with this ID should be renamed, use the same format for the following arguments, \
        one per variable, in the form 'old_variable_name=new_variable_name'\n\
        By default, this program will try to operate on all files with the '.ftl' extension in the current directory. \
        To change this, specify '--' after the ID (and variables, if any), followed by at least one path. \
        Each path should either point to a FTL file, \
        or a directory, which will result in all direct child files with the '.ftl' extension being included.\n\
        \n\
        Example:\n\
        {} old_id=new_id old_var_1=new_var_1 old_var_2=new_var_2 -- default.ftl ftl/",
        std::env::args().next().unwrap()
    )
}

struct Args {
    old_id: String,
    new_id: Option<String>,
    variable_update: HashMap<String, String>,
    files: Vec<PathBuf>,
}

fn main() {
    let args = parse_cli_args(parse_args).unwrap_or_else(|e| {
        eprintln!("Error:\n{e}");
        print_help();
        std::process::exit(1);
    });
    let resources = rename_in_all_files(
        &args.files,
        &args.old_id,
        &args.new_id,
        &args.variable_update,
    )
    .unwrap_or_else(|e| {
        eprintln!("Failed to perform renaming:\n{e}");
        std::process::exit(1);
    });
    serialize_resources_to_files(&resources).unwrap_or_else(|e| {
        eprintln!("Failed to update files:\n{e}");
        std::process::exit(1);
    });
}

fn parse_args<I: Iterator<Item = String>>(mut args: I) -> Result<Args, String> {
    let Some(id_arg) = args.next() else {
        return Err("No argument specified.".into());
    };
    let old_id;
    let new_id;
    match id_arg.split_once('=') {
        Some((old, new)) => {
            old_id = old.into();
            new_id = Some(new.into());
        }
        None => {
            old_id = id_arg;
            new_id = None;
        }
    }
    let mut variable_update = HashMap::new();
    for arg in args.by_ref() {
        match arg.split_once('=') {
            Some((old, new)) => {
                variable_update.insert(old.into(), new.into());
            }
            None => {
                if arg != "--" {
                    return Err("Invalid arguments.".into());
                }
                break;
            }
        }
    }
    // remaining arguments specify files
    let files = parse_file_args(args)?;
    Ok(Args {
        old_id,
        new_id,
        variable_update,
        files,
    })
}

fn rename_in_all_files<I, P>(
    files: I,
    old_id: &str,
    new_id: &Option<String>,
    variable_update: &HashMap<String, String>,
) -> Result<HashMap<P, Resource<String>>, String>
where
    I: IntoIterator<Item = P>,
    P: AsRef<Path> + Hash + Eq,
{
    let mut resources = HashMap::new();
    for file in files.into_iter() {
        let resource = rename_in_file(&file, old_id, new_id, variable_update)?;
        let resource = format_resource(resource)?;
        resources.insert(file, resource);
    }
    Ok(resources)
}

fn rename_in_file<P: AsRef<Path>>(
    file: P,
    old_id: &str,
    new_id: &Option<String>,
    variable_update: &HashMap<String, String>,
) -> Result<Resource<String>, String> {
    let mut resource = match parse_as_syntax_resource(file.as_ref()) {
        Ok(resource) => resource,
        Err(e) => {
            return Err(format!("Failed to parse {:?}:\n{e}", file.as_ref()));
        }
    };
    let existing_ids = get_message_ids(&resource);
    if !existing_ids.contains(old_id) {
        eprintln!(
            "file {:?}: ID '{old_id}' does not exist. No renaming performed.",
            file.as_ref()
        );
        return Ok(resource);
    }
    if let Some(new_id) = new_id
        && existing_ids.contains(new_id.as_str())
    {
        return Err(format!(
            "file {:?}: ID '{new_id}' already exists.",
            file.as_ref()
        ));
    }

    for entry in &mut resource.body {
        if let fluent_syntax::ast::Entry::Message(message) = entry
            && message.id.name == old_id
        {
            rename_message(message, new_id, variable_update)
                .map_err(|e| format!("file {:?}: message '{old_id}': {e}", file.as_ref()))?;
            // IDs must be unique, so we won't miss any by stopping here.
            break;
        }
    }
    Ok(resource)
}

fn rename_message(
    message: &mut Message<String>,
    new_id: &Option<String>,
    variable_update: &HashMap<String, String>,
) -> Result<(), String> {
    if let Some(pattern) = &mut message.value {
        for (old_variable_name, new_variable_name) in variable_update {
            if !pattern_contains_variable(pattern, old_variable_name) {
                return Err(format!(
                    "Variable '{old_variable_name}' does not exist in message."
                ));
            }
            if pattern_contains_variable(pattern, new_variable_name) {
                return Err(format!(
                    "Variable '{new_variable_name}' already exists in message."
                ));
            }
        }
        for (old_variable_name, new_variable_name) in variable_update {
            rename_variable_in_pattern(pattern, old_variable_name, new_variable_name);
        }
    }
    if let Some(new_id) = new_id {
        message.id.name = new_id.into();
    }
    Ok(())
}

fn pattern_contains_variable(pattern: &Pattern<String>, variable: &str) -> bool {
    pattern.elements.iter().any(|element| {
        if let PatternElement::Placeable { expression } = element {
            let placable_name = match expression {
                Expression::Select { selector, .. } => selector,
                Expression::Inline(inline_expression) => inline_expression,
            };
            if let InlineExpression::VariableReference {
                id: Identifier { name },
            } = placable_name
            {
                name == variable
            } else {
                false
            }
        } else {
            false
        }
    })
}

fn rename_variable_in_pattern(pattern: &mut Pattern<String>, old_name: &str, new_name: &str) {
    for element in &mut pattern.elements {
        if let PatternElement::Placeable { expression } = element {
            match expression {
                Expression::Select { selector, variants } => {
                    if let InlineExpression::VariableReference {
                        id: Identifier { name },
                    } = selector
                        && name == old_name
                    {
                        *name = new_name.into();
                        for variant in variants {
                            // Recursion is needed because variants can refer to the variable used
                            // as the selector.
                            rename_variable_in_pattern(&mut variant.value, old_name, new_name);
                        }
                    }
                }
                Expression::Inline(inline_expression) => {
                    if let InlineExpression::VariableReference {
                        id: Identifier { name },
                    } = inline_expression
                        && name == old_name
                    {
                        *name = new_name.into();
                    }
                }
            }
        }
    }
}
