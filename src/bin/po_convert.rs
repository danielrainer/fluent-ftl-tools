use anyhow::{Context, Result, anyhow, bail, ensure};
use fluent_ftl_tools::{
    HasEntries, format_resource, get_file_paths_with_extension_from_directory, parse_cli_args,
    parse_directory, parse_fluent_identifiers, parse_single_file_arg, parse_str_as_syntax_resource,
    parse_usize, serialize_resource_to_file,
};
use fluent_syntax::ast::{Entry, Resource};
use gettext_po_file_parser::{get_message_at_line_index, parse_po_file};
use std::{
    collections::HashMap,
    io::ErrorKind,
    path::{Path, PathBuf},
};

const DEFAULT_LANGUAGE: &str = "en";

fn print_help() {
    eprintln!(
        "\n\
        Usage:\n\
        po-convert id_source_file id_source_line_number po_dir ftl_dir new_message_id [variable_name ...]\n\
        \n\
        This program can be used to convert a gettext PO message to a Fluent FTL entry.\n\
        The first two arguments are used to specify the message, \
        with the first argument, `id_source_file`, specifying the path to a PO file, \
        and the second argument, `id_source_line_number`, \
        specifying a line number within this file (starting at 1). \
        At this line, a `msgid` should be found.\n\
        The next argument, `po_dir`, tells the program in which directory the existing PO files are located.\n\
        Similarly, `ftl_dir` specifies in which directory the FTL files are stored.\n\
        For each file `po_dir/lang.po` (must have the `.po` extension), \
        this program checks if it contains a `msgstr` \
        for the `msgid` specified via `id_source_file` and `id_source_line_number`. \
        If so, an entry will be added to `ftl_dir/lang.ftl`. This file will be created if it does not exist.\n\
        For the language of the `id_source_file`, there is additional special handling:\n\
        Let `id_source_file=some/path/source.po`.\n\
        Then, if `po_dir/source.po` does not exist or does not contain a `msgstr` for the selected `msgid`, \
        there will still be an entry added to `ftl_dir/source.ftl`, derived from the selected `msgid`.\n\
        \n\
        How the gettext messages should be translated to Fluent messages is specified via the remaining arguments.\n\
        `new_message_id` is always required and gives the message ID which should be used in all FTL files. \
        It must be unique with respect to all other entries in the FTL files.\n\
        Additional arguments should be specified iff the message was used as a printf-style format string \
        containing at least one conversion specifier. \
        For each specifier, one `variable_name` must be given.\n\
        Note that only the basic specifiers `%c`, `%d`, `%s`, `%u` are supported. \
        `%%` will always be converted to single `%`. \
        "
    );
}

fn main() -> Result<()> {
    let (id_source_file_path, args) = parse_cli_args(parse_single_file_arg).with_context(|| {
        print_help();
        "Expected arg for PO file but got none."
    })?;
    let id_source_file_content = std::fs::read(&id_source_file_path).with_context(|| {
        print_help();
        format!("Failed to read PO file {id_source_file_path:?}")
    })?;

    let (line_number, args) = parse_usize(args).with_context(|| {
        print_help();
        "Expected argument for line number but got none."
    })?;
    let line_number = line_number.with_context(|| {
        print_help();
        "Failed to parse line number"
    })?;
    ensure!(
        line_number != 0,
        "Line number 0 is invalid. The first line number is 1."
    );

    let (po_dir, args) = parse_directory(args).with_context(|| {
        print_help();
        "Failed to parse PO directory"
    })?;

    let (ftl_dir, args) = parse_directory(args).with_context(|| {
        print_help();
        "Failed to parse FTL directory"
    })?;

    let ids = parse_fluent_identifiers(args).with_context(|| {
        print_help();
        "Failed to parse Fluent identifiers"
    })?;
    let Some(fluent_message_id) = ids.first() else {
        print_help();
        bail!("Expected message ID but found none.");
    };
    let variable_names = &ids[1..];

    // CLI arg parsing done.

    let (gettext_msgid, _) = get_message_at_line_index(line_number - 1, &id_source_file_content)
        .with_context(|| format!("Failed to get message from PO file {id_source_file_path:?}"))?;

    let lang_to_msgstr = build_lang_to_msgstr_map(&gettext_msgid, po_dir)
        .context("Failed to obtain msgstrs from PO files")?;
    let lang_to_fluent_entry =
        build_lang_to_fluent_entry_map(lang_to_msgstr, fluent_message_id, variable_names)
            .context("Failed to convert msgstrs to Fluent entries")?;
    let mut message_value_for_source = String::new();
    fluent_message_value_from_po_string(
        &gettext_msgid,
        variable_names,
        &mut message_value_for_source,
    )
    .context("Failed to generate Fluent message value for source code suggestion.")?;
    let resources = build_updated_fluent_resources(lang_to_fluent_entry, &ftl_dir)
        .context("Could not create all updated Fluent resources")?;
    write_resources_to_files(resources).context("Writing Fluent resources to FTL files failed")?;
    println!(
        "Suggested Rust code to use the new message:\n{}",
        rust_source_suggestion(fluent_message_id, message_value_for_source, variable_names)
    );
    Ok(())
}

// Maps from resource language to the corresponding msgstr.
// Uses the specified msgid for the language of the file specified via the first CLI argument if
// that language does not have a msgstr.
fn build_lang_to_msgstr_map<P: AsRef<Path>>(
    msgid: &str,
    po_dir: P,
) -> Result<HashMap<String, Option<String>>> {
    let po_file_paths =
        get_file_paths_with_extension_from_directory(&po_dir, "po").with_context(|| {
            format!(
                "Failed to get files from PO directory {:?}",
                po_dir.as_ref()
            )
        })?;
    let mut map = HashMap::new();
    for path in po_file_paths {
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read file {path:?}"))?;
        let po_data = parse_po_file(content.as_bytes())
            .with_context(|| format!("Failed to parse PO file {path:?}"))?;
        let msgstr = match po_data.get(msgid) {
            Some(msgstr) => {
                if msgstr.is_empty() {
                    None
                } else {
                    Some(msgstr.to_owned())
                }
            }
            None => None,
        };
        // get_file_paths_with_extension_from_directory ensures that `path` is a file with the `.po` extension.
        let language = path
            .file_stem()
            .unwrap()
            .to_str()
            .ok_or_else(|| anyhow!("File name {path:?} must valid UTF-8."))?
            .to_string();
        map.insert(language, msgstr);
    }
    // This could be auto-generated, but adding it here prevents having to call the tool for
    // updating en.ftl.
    map.insert(DEFAULT_LANGUAGE.to_owned(), Some(msgid.to_string()));
    Ok(map)
}

fn convert_conversion_specifier<S: AsRef<str>>(
    input_chars: &[char],
    input_char_index: &mut usize,
    replacement_variables: &[S],
    variable_index: &mut usize,
    result_string: &mut String,
) -> Result<()> {
    *input_char_index += 1;
    let mut specifier_chars = input_chars[(*input_char_index)..].iter();
    // character after %
    let Some(c) = specifier_chars.next() else {
        bail!("Ran out of characters in conversion specifier.");
    };
    match c {
        '%' => {
            result_string.push('%');
            return Ok(());
        }
        'c' | 'd' | 's' | 'u' => {
            if *variable_index >= replacement_variables.len() {
                bail!("Insufficient number of variables provided.");
            }
            result_string.push_str("{ $");
            result_string.push_str(replacement_variables[*variable_index].as_ref());
            result_string.push_str(" }");
            *variable_index += 1;
        }
        _ => {
            bail!("Unsupported conversion specifier. Please convert manually.");
        }
    }
    Ok(())
}

fn fluent_message_value_from_po_string<S: AsRef<str>>(
    format_string: &str,
    replacement_variables: &[S],
    output: &mut String,
) -> Result<()> {
    let mut variable_index = 0;
    let input_chars: Vec<char> = format_string.chars().collect();
    let mut input_char_index = 0;
    while input_char_index < input_chars.len() {
        match input_chars[input_char_index] {
            '{' => {
                output.push_str("{\"{\"}");
            }
            '}' => {
                output.push_str("{\"}\"}");
            }
            '\n' => {
                output.push_str("{ \"\\u000A\" }");
            }
            '%' => {
                convert_conversion_specifier(
                    &input_chars,
                    &mut input_char_index,
                    replacement_variables,
                    &mut variable_index,
                    output,
                )
                .context("Problem with conversion specifier")?;
            }
            c => {
                output.push(c);
            }
        }
        input_char_index += 1;
    }
    if variable_index < replacement_variables.len() {
        bail!(
            "Excess arguments. There are {variable_index} conversion specifiers in\n{format_string}\n\
            but {} variable names were provided.",
            replacement_variables.len()
        );
    }
    Ok(())
}

fn fluent_message_from_po_string<S: AsRef<str>>(
    format_string: &str,
    fluent_message_id: &str,
    replacement_variables: &[S],
) -> Result<String> {
    let mut result = fluent_message_id.to_string();
    result.push_str(" = ");
    fluent_message_value_from_po_string(format_string, replacement_variables, &mut result)?;
    Ok(result)
}

fn build_lang_to_fluent_entry_map<S: AsRef<str>>(
    lang_to_msgstr: HashMap<String, Option<String>>,
    fluent_message_id: &str,
    replacement_variables: &[S],
) -> Result<HashMap<String, Option<Entry<String>>>> {
    let mut map = HashMap::new();
    for (lang, po_string) in lang_to_msgstr {
        let fluent_string = match po_string {
            Some(po_string) => {
                let fluent_string = fluent_message_from_po_string(
                    &po_string,
                    fluent_message_id,
                    replacement_variables,
                )
                .with_context(|| format!("Failed to convert PO string \"{po_string}\""))?;
                let new_resource = parse_str_as_syntax_resource(fluent_string)
                    .expect("Building Fluent entry went wrong.");
                let entry = new_resource
                    .body
                    .first()
                    .expect("Failed to parse message.")
                    .to_owned();
                Some(entry)
            }
            None => None,
        };
        map.insert(lang, fluent_string);
    }
    Ok(map)
}

fn build_updated_fluent_resources<P: AsRef<Path>>(
    lang_to_fluent_entry: HashMap<String, Option<Entry<String>>>,
    ftl_dir: P,
) -> Result<HashMap<PathBuf, Option<Resource<String>>>> {
    let ftl_dir = ftl_dir.as_ref();
    let mut lang_to_resource = HashMap::new();
    for (lang, entry) in lang_to_fluent_entry {
        let ftl_file_path = ftl_dir.join(format!("{lang}.ftl"));
        let Some(entry) = entry else {
            lang_to_resource.insert(ftl_file_path, None);
            continue;
        };
        let formatted_resource = match std::fs::read_to_string(&ftl_file_path) {
            Ok(content) => {
                let mut resource = parse_str_as_syntax_resource(content).with_context(|| {
                    format!("Failed to parse content of {ftl_file_path:?} as Fluent resource")
                })?;
                let existing_message_ids = resource.message_ids();
                let message_id = match &entry {
                    Entry::Message(message) => &message.id.name,
                    _ => panic!("Entry must be a message."),
                };
                if existing_message_ids.contains(message_id.as_str()) {
                    bail!("File {ftl_file_path:?} already contains the message ID '{message_id}'.");
                }
                resource.body.push(entry);
                format_resource(resource)
                    .with_context(|| format!("Failed to format resource in {ftl_file_path:?}"))?
            }
            Err(e) => {
                if e.kind() == ErrorKind::NotFound {
                    Resource { body: vec![entry] }
                } else {
                    bail!("Failed to read file {ftl_file_path:?}");
                }
            }
        };
        lang_to_resource.insert(ftl_file_path, Some(formatted_resource));
    }
    Ok(lang_to_resource)
}

fn write_resources_to_files(resources: HashMap<PathBuf, Option<Resource<String>>>) -> Result<()> {
    let mut errors = String::new();
    for (path, resource) in resources {
        let Some(resource) = resource else {
            println!("No update for {path:?}.");
            continue;
        };
        if let Err(e) = serialize_resource_to_file(&resource, &path) {
            errors.push_str(&format!("Failed to update FTL file {path:?}:\n{e}\n"));
        } else {
            println!("Updated {path:?}.");
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        bail!("{errors}")
    }
}

fn rust_source_suggestion<S1: AsRef<str>, S2: AsRef<str>, S3: AsRef<str>>(
    fluent_message_id: S1,
    fluent_message_value: S2,
    variable_names: &[S3],
) -> String {
    let mut result = String::from("localize!(");
    if !variable_names.is_empty() {
        result.push_str("\n    ");
    }
    result.push('"');
    result.push_str(fluent_message_id.as_ref());
    result.push_str("\" = \"");
    result.push_str(fluent_message_value.as_ref());
    result.push('"');
    if !variable_names.is_empty() {
        result.push_str(",\n");
        for var in variable_names {
            result.push_str("    ");
            result.push_str(var.as_ref());
            result.push_str(" = todo!(),\n");
        }
    }
    result.push(')');
    result
}
