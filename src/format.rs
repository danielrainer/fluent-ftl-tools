use anyhow::{Context, Result, bail};
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

pub fn format_path<P: AsRef<Path>>(path: P, mode: FormattingMode) -> Result<()> {
    let path = path.as_ref();
    let file_content =
        std::fs::read_to_string(path).with_context(|| format!("Failed to read from {path:?}"))?;
    let resource = parse_str_as_syntax_resource(&file_content)
        .with_context(|| format!("Failed to parse {path:?}"))?;
    match mode {
        FormattingMode::Check => {
            if is_formatted(&file_content, resource)? {
                Ok(())
            } else {
                bail!("Content of {path:?} is not formatted correctly.")
            }
        }
        FormattingMode::Rewrite => {
            let formatted_resource = format_resource(resource).with_context(|| {
                format!("File {path:?} does not conform to the expected subset of FTL syntax")
            })?;
            serialize_resource_to_file(&formatted_resource, path)
                .with_context(|| format!("Failed to serialize resource to file {path:?}"))?;
            Ok(())
        }
    }
}

pub fn format_text(text: impl Into<String>) -> Result<String> {
    let resource = parse_str_as_syntax_resource(text)?;
    let formatted_resource = format_resource(resource)?;
    Ok(serialize_resource(&formatted_resource))
}
