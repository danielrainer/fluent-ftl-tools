use anyhow::{Context, Result};
use fluent_ftl_tools::{
    format::{FormattingMode, format_path},
    parse_cli_args, parse_file_args,
};

fn main() -> Result<()> {
    let paths = parse_cli_args(parse_file_args).context("Invalid arguments")?;
    for path in paths {
        format_path(path, FormattingMode::Rewrite)?;
    }
    Ok(())
}
