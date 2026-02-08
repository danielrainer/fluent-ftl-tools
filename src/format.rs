use std::path::Path;

use crate::{
    format_resource, parse_as_syntax_resource, parse_str_as_syntax_resource, serialize_resource,
    serialize_resource_to_file,
};

pub fn format_path<P: AsRef<Path>>(path: P) -> Result<(), String> {
    let path = path.as_ref();
    let resource = parse_as_syntax_resource(path)?;
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

pub fn format_text(text: &str) -> Result<String, String> {
    let resource = parse_str_as_syntax_resource(text)?;
    let formatted_resource = format_resource(resource)?;
    Ok(serialize_resource(&formatted_resource))
}
