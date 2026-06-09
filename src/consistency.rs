use crate::{
    HasEntries,
    annotate::{Annotation, MessageAnnotationRead},
    is_formatted, make_bundle, parse_as_syntax_resource, parse_str_as_fluent_resource,
    parse_str_as_syntax_resource, serialize_resources_to_files,
};
use anyhow::{Context, Result, bail};
use fluent_syntax::ast::{Entry, Resource};
use std::{
    collections::{HashMap, HashSet},
    hash::Hash,
    path::Path,
};

fn check_for_outdated_annotation<'a, S: AsRef<str> + 'a>(resource: &'a Resource<S>) -> Result<()> {
    let messages = resource.messages();
    for (id, message) in messages {
        if message.has_annotation(Annotation::Outdated)? {
            bail!(
                "Outdated message: {id}\n\
                Translations are marked as outdated when the original message definition changed.\n\
                These annotations must be resolved to avoid having incorrect translations.\n\
                See the `cargo xtask fluent resolve-outdated` tool."
            );
        }
    }
    Ok(())
}

fn check_internal_consistency(resource_str: &str, resource: &Resource<String>) -> Result<()> {
    // Checks for duplicate definitions.
    // Ideally, we would not parse again here, but construct the `FluentResource` directly from the
    // resource, but the `fluent-rs` API does not allow for that.
    let _ = make_bundle(&parse_str_as_fluent_resource(resource_str)?)?;
    if !is_formatted(resource_str, resource.to_owned())? {
        bail!("File does not conform to expected formatting.");
    }
    check_for_outdated_annotation(resource)
}

fn check_for_extra_ids(
    messages_default: &HashSet<&str>,
    messages_other: &HashSet<&str>,
) -> Result<()> {
    let mut extra_messages = messages_other.difference(messages_default).peekable();
    if extra_messages.peek().is_none() {
        return Ok(());
    }
    let mut error_message = String::from("Unexpected message identifiers found:\n");
    for message in extra_messages {
        error_message.push_str(message);
        error_message.push('\n');
    }
    bail!("{error_message}");
}

pub fn check_all_resources<
    'a,
    P: AsRef<Path>,
    Q: AsRef<Path> + 'a,
    S: Into<String>,
    T: Into<String>,
    O: IntoIterator<Item = (Q, T)>,
>(
    default_language: (P, S),
    other_languages: O,
) -> Result<()> {
    let default_language_string = default_language.1.into();
    let default_language_resource = parse_str_as_syntax_resource(&default_language_string)
        .with_context(|| format!("Errors in {:?}", default_language.0.as_ref()))?;
    check_internal_consistency(&default_language_string, &default_language_resource)
        .with_context(|| format!("Errors in {:?}", default_language.0.as_ref()))?;
    let message_vars_default = default_language_resource.all_message_vars()?;
    let message_ids_default = HashSet::from_iter(message_vars_default.keys().copied());
    for (path, other_str) in other_languages {
        let other_language_string = other_str.into();
        let other_language_resource = parse_str_as_syntax_resource(&other_language_string)
            .with_context(|| format!("Errors in {:?}", path.as_ref()))?;
        check_internal_consistency(&other_language_string, &other_language_resource)
            .with_context(|| format!("Errors in {:?}", path.as_ref()))?;
        let message_ids_res = other_language_resource.message_ids();
        check_for_extra_ids(&message_ids_default, &message_ids_res)
            .with_context(|| format!("Errors in {:?}", path.as_ref()))?;
        other_language_resource.check_variables_of_all_messages(&message_vars_default)?;
    }
    Ok(())
}

pub fn check_all_resource_files<P: AsRef<Path>, Q: AsRef<Path>, O: IntoIterator<Item = Q>>(
    default: P,
    others: O,
) -> Result<()> {
    let default_resource_str = std::fs::read_to_string(&default)?;
    let other_resources = others
        .into_iter()
        .map(|path| {
            std::fs::read_to_string(&path)
                .with_context(|| format!("Failed to read FTL file {:?}", path.as_ref()))
                .map(|s| (path, s))
        })
        .collect::<Result<Vec<_>>>()?;
    check_all_resources((default, default_resource_str), other_resources)
}

/// Remove translations of messages which do not appear in the default language or which use
/// variables not used for this message in the default language.
pub fn remove_inconsistent_translations<
    'a,
    P: AsRef<Path> + Eq + Hash + 'a,
    O: IntoIterator<Item = &'a P>,
>(
    default_resource_vars: HashMap<&str, HashSet<&str>>,
    others: O,
) -> Result<()> {
    let other_resources = others
        .into_iter()
        .map(|path| parse_as_syntax_resource(path).map(|res| (path, res)))
        .collect::<Result<Vec<_>>>()?;
    let mut updated_resources = HashMap::with_capacity(other_resources.len());
    for (path, res) in other_resources {
        let new_body = res
            .body
            .into_iter()
            .filter(|entry| {
                if let Entry::Message(message) = entry {
                    default_resource_vars.contains_key(&message.id.name.as_str())
                } else {
                    true
                }
            })
            .collect();
        let res_without_obsolete_messages = Resource { body: new_body };
        let translation_vars = res_without_obsolete_messages
            .all_message_vars()
            .with_context(|| format!("Failed to get message variables from {:?}", path.as_ref()))?;
        let new_body: Vec<_> = res_without_obsolete_messages
            .body
            .iter()
            .filter(|entry| {
                if let Entry::Message(message) = entry {
                    let id = message.id.name.as_str();
                    default_resource_vars.get(id) == translation_vars.get(id)
                } else {
                    true
                }
            })
            .cloned()
            .collect();
        let updated_resource = Resource { body: new_body };
        updated_resources.insert(path, updated_resource);
    }
    serialize_resources_to_files(&updated_resources)
}
