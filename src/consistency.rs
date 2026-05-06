use crate::{HasEntries, is_formatted, make_bundle, parse_as_fluent_resource};
use anyhow::{Context, Result, bail};
use fluent::FluentResource;
use std::{collections::HashSet, path::Path};

fn check_internal_consistency(fluent_resource: &FluentResource) -> Result<()> {
    // Checks for duplicate definitions.
    let _ = make_bundle(fluent_resource)?;
    if !is_formatted(fluent_resource.source())? {
        bail!("File does not conform to expected formatting.");
    }
    Ok(())
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
    S: AsRef<str> + 'a,
    O: IntoIterator<Item = &'a (S, FluentResource)>,
>(
    default_resource: (&str, &FluentResource),
    other_resources: O,
) -> Result<()> {
    check_internal_consistency(default_resource.1)
        .with_context(|| format!("Errors in {:?}", default_resource.0))?;
    let message_vars_default = default_resource.1.all_message_vars()?;
    let message_ids_default = HashSet::from_iter(message_vars_default.keys().copied());
    for (name, res) in other_resources {
        check_internal_consistency(res)
            .with_context(|| format!("Errors in {:?}", name.as_ref()))?;
        let message_ids_res = res.message_ids();
        check_for_extra_ids(&message_ids_default, &message_ids_res)
            .with_context(|| format!("Errors in {:?}", name.as_ref()))?;
        res.check_variables_of_all_messages(&message_vars_default)?;
    }
    Ok(())
}

pub fn check_all_resource_files<
    'a,
    P: AsRef<Path>,
    Q: AsRef<Path> + 'a,
    O: IntoIterator<Item = &'a Q>,
>(
    default: P,
    others: O,
) -> Result<()> {
    let default_resource = parse_as_fluent_resource(&default)?;
    let other_resources = others
        .into_iter()
        .map(|path| parse_as_fluent_resource(path).map(|res| (format!("{:?}", path.as_ref()), res)))
        .collect::<Result<Vec<_>>>()?;
    check_all_resources(
        (&format!("{:?}", default.as_ref()), &default_resource),
        &other_resources,
    )
}
