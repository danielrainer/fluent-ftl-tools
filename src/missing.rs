use crate::{HasEntries, parse_as_syntax_resource};
use anyhow::Result;
use std::{collections::HashSet, path::Path};

fn find_missing_message_ids<'a, P: AsRef<Path>>(
    expected_ids: &HashSet<&'a str>,
    path: P,
) -> Result<Vec<&'a str>> {
    let resource = parse_as_syntax_resource(&path)?;
    let present_ids = resource.message_ids();
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

pub fn find_missing_message_ids_in_files<P: AsRef<Path>, Q: AsRef<Path>>(
    expected: P,
    to_check: &[Q],
) -> Result<Option<String>> {
    let expected_resource = parse_as_syntax_resource(&expected)?;
    let expected_ids = expected_resource.message_ids();
    let mut missing_message = String::new();
    for path in to_check {
        let missing_ids = find_missing_message_ids(&expected_ids, path)?;
        if !missing_ids.is_empty() {
            missing_message.push_str(&format!("Message IDs missing in {:?}:\n", path.as_ref()));
            for id in missing_ids {
                missing_message.push_str(id);
                missing_message.push('\n');
            }
            missing_message.push('\n');
        }
    }
    if missing_message.is_empty() {
        Ok(None)
    } else {
        Ok(Some(missing_message))
    }
}
