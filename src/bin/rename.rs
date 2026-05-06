//! Program for renaming IDs and variables in FTL files.
//! Call without arguments to see usage information.

use anyhow::{Context, Result, bail};
use fluent_ftl_tools::{
    parse_cli_args, parse_file_args,
    rename::{Args, rename_in_all_files},
    serialize_resources_to_files,
};
use std::collections::HashMap;

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

fn main() -> Result<()> {
    let args = parse_cli_args(parse_args).with_context(|| {
        print_help();
        "Invalid arguments."
    })?;
    let resources = rename_in_all_files(
        &args.files,
        &args.old_id,
        &args.new_id,
        &args.variable_update,
    )
    .context("Failed to perform renaming.")?;
    serialize_resources_to_files(&resources).context("Failed to update files.")?;
    Ok(())
}

fn parse_args<I: Iterator<Item = String>>(mut args: I) -> Result<Args> {
    let Some(id_arg) = args.next() else {
        bail!("No argument specified.");
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
                    bail!("Invalid arguments.");
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
