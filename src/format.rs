use std::path::Path;

use crate::{
    format_resource, is_formatted, parse_str_as_syntax_resource, serialize_resource,
    serialize_resource_to_file,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormattingMode {
    Check,
    Rewrite,
}

pub fn format_path<P: AsRef<Path>>(path: P, mode: FormattingMode) -> Result<(), String> {
    let path = path.as_ref();
    let file_content =
        std::fs::read_to_string(path).map_err(|e| format!("Failed to read from {path:?}:\n{e}"))?;
    match mode {
        FormattingMode::Check => {
            if is_formatted(&file_content)? {
                Ok(())
            } else {
                Err(format!("Content of {path:?} is not formatted correctly."))
            }
        }
        FormattingMode::Rewrite => {
            let resource = parse_str_as_syntax_resource(&file_content)
                .map_err(|e| format!("Failed to parse {path:?}:\n{e}"))?;
            let formatted_resource = format_resource(resource).map_err(|e| {
                format!("File {path:?} does not conform to the expected subset of FTL syntax:\n{e}")
            })?;
            match serialize_resource_to_file(&formatted_resource, path) {
                Ok(()) => Ok(()),
                Err(e) => Err(format!(
                    "Failed to serialize resource to file {path:?}:\n{e}",
                )),
            }
        }
    }
}

pub fn format_text(text: &str) -> Result<String, String> {
    let resource = parse_str_as_syntax_resource(text)?;
    let formatted_resource = format_resource(resource)?;
    Ok(serialize_resource(&formatted_resource))
}
