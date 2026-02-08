use std::{collections::HashSet, path::Path};

use crate::{get_message_ids, parse_as_syntax_resource};

fn find_missing_message_ids<'a, P: AsRef<Path>>(
    expected_ids: &HashSet<&'a str>,
    path: P,
) -> Result<Vec<&'a str>, String> {
    let resource = parse_as_syntax_resource(&path)?;
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

pub fn find_missing_message_ids_in_files<P: AsRef<Path>, Q: AsRef<Path>>(
    expected: P,
    to_check: &[Q],
) -> Result<Option<String>, String> {
    let expected_resource = parse_as_syntax_resource(&expected)?;
    let expected_ids = get_message_ids(&expected_resource);
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
