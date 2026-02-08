use std::{
    collections::HashMap,
    hash::Hash,
    path::{Path, PathBuf},
};

use crate::{format_resource, get_message_ids, parse_as_syntax_resource};
use fluent_syntax::ast::{
    Expression, Identifier, InlineExpression, Message, Pattern, PatternElement, Resource,
};

pub struct Args {
    pub old_id: String,
    pub new_id: Option<String>,
    pub variable_update: HashMap<String, String>,
    pub files: Vec<PathBuf>,
}

pub fn rename_in_all_files<I, P>(
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
    let mut resource = parse_as_syntax_resource(file.as_ref())?;
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
