use std::{
    collections::{HashMap, HashSet},
    env::Args,
    num::ParseIntError,
    path::{Path, PathBuf},
};

use fluent::{FluentBundle, FluentError, FluentResource};
use fluent_syntax::{
    ast::{Comment, Entry, Resource},
    parser::{Parser, ParserError},
    serializer::{Options, Serializer},
};
use unic_langid::LanguageIdentifier;

pub fn parse_str_as_syntax_resource(input: &str) -> Result<Resource<String>, String> {
    let parser = Parser::new(input.to_owned());
    parser
        .parse()
        .map_err(|(_, errors)| display_parse_errors(&errors, input))
}

pub fn parse_as_syntax_resource<P: AsRef<Path>>(path: P) -> Result<Resource<String>, String> {
    let file_content = std::fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read from {:?}:\n{e}", path.as_ref()))?;
    let parser = Parser::new(file_content.clone());
    parser
        .parse()
        .map_err(|(_, errors)| display_parse_errors(&errors, &file_content))
}

pub fn parse_as_fluent_resource<P: AsRef<Path>>(path: P) -> Result<FluentResource, String> {
    let file_content = std::fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read from {:?}:\n{e}", path.as_ref()))?;
    FluentResource::try_new(file_content.clone())
        .map_err(|(_, errors)| display_parse_errors(&errors, &file_content))
}

pub fn make_bundle(resource: &FluentResource) -> Result<FluentBundle<&FluentResource>, String> {
    let lang_id: LanguageIdentifier = "en".parse().unwrap();
    let mut bundle = FluentBundle::new(vec![lang_id]);
    bundle
        .add_resource(resource)
        .map_err(|errors| display_fluent_errors(&errors))?;
    Ok(bundle)
}

fn display_parse_error(error: &ParserError, file_content: &str) -> String {
    // Carriage returns could break position computation, since we assume that each line is
    // terminated by one newline and nothing else.
    assert!(!file_content.contains('\r'));
    let mut pos = 0;
    let mut affected_lines = vec![];
    for (line_number, line) in file_content.lines().enumerate() {
        pos += line.len() + 1;
        if pos > error.pos.start {
            affected_lines.push((line_number + 1, line.to_string()));
        }
        if pos >= error.pos.end {
            break;
        }
    }
    let number_width = usize::try_from(
        affected_lines
            .last()
            .unwrap()
            .0
            .checked_ilog10()
            .unwrap_or(0),
    )
    .unwrap()
        + 1;
    let mut result = String::from("\n");
    for (line_number, line) in &affected_lines {
        result.push_str(&format!("{line_number:number_width$}:{line}\n"));
    }
    result.push_str(&format!("Reason: {:?}\n", error.kind));
    result
}

fn display_parse_errors(errors: &[ParserError], file_content: &str) -> String {
    let mut error_message = "Parser errors:\n".to_string();
    for error in errors {
        error_message.push_str(&display_parse_error(error, file_content));
    }
    error_message.push('\n');
    error_message
}

fn display_fluent_errors(errors: &[FluentError]) -> String {
    let mut error_message = String::new();
    for error in errors {
        error_message.push_str(&format!("{error}\n"));
    }
    error_message
}

pub fn format_resource(resource: Resource<String>) -> Result<Resource<String>, String> {
    use fluent_syntax::ast::Entry;
    macro_rules! fail {
        ($fmt:literal $(, $arg:expr)* $(,)?) => {
            return Err(format!($fmt $(, $arg)*))
        };
    }
    fn serialize_comment(comment: &Comment<String>, prefix: &str) -> String {
        let mut serializer = Serializer::new(Options { with_junk: false });
        serializer.serialize_free_comment(comment, prefix);
        serializer.into_serialized_text()
    }

    let mut sorted_entries = Vec::with_capacity(resource.body.len());
    let mut terms = vec![];
    let mut messages = vec![];

    let mut initial_comments_over = false;
    for entry in resource.body {
        if !matches!(entry, Entry::ResourceComment(_)) {
            initial_comments_over = true;
        }
        match entry {
            Entry::Message(message) => {
                messages.push(message);
            }
            Entry::Term(term) => {
                terms.push(term);
            }
            Entry::Comment(comment) => {
                fail!(
                    "Found standalone comment.\n\
                    Comments must either be resource-level comments (###) at the beginning of the file \
                    or attached to messages or terms by putting them directly above them.\n\
                    Affected comment:\n\
                    {}",
                    serialize_comment(&comment, "#")
                );
            }
            Entry::GroupComment(comment) => {
                fail!(
                    "Found standalone comment.\n\
                    Comments must either be resource-level comments (###) at the beginning of the file \
                    or attached to messages or terms by putting them directly above them.\n\
                    Affected comment:\n\
                    {}",
                    serialize_comment(&comment, "##")
                );
            }
            Entry::ResourceComment(comment) => {
                if initial_comments_over {
                    fail!(
                        "Found resource comment after the initial section. Relevant comment:\n{}",
                        serialize_comment(&comment, "###")
                    );
                }
                sorted_entries.push(Entry::ResourceComment(comment));
            }
            Entry::Junk { content } => {
                fail!("Resource contains junk:\n{content}");
            }
        }
    }

    terms.sort_by(|a, b| a.id.name.cmp(&b.id.name));
    messages.sort_by(|a, b| a.id.name.cmp(&b.id.name));

    for term in terms {
        sorted_entries.push(Entry::Term(term));
    }
    for message in messages {
        sorted_entries.push(Entry::Message(message));
    }
    Ok(Resource {
        body: sorted_entries,
    })
}

pub fn is_formatted(ftl_data: &str) -> Result<bool, String> {
    let resource = Parser::new(ftl_data.to_string())
        .parse()
        .map_err(|(_, errors)| display_parse_errors(&errors, ftl_data))?;
    let formatted_resource = format_resource(resource)?;
    let formatted_resource_text = serialize_resource(&formatted_resource);
    Ok(formatted_resource_text == ftl_data)
}

pub fn get_file_paths_with_extension_from_directory<P: AsRef<Path>>(
    path: P,
    expected_extension: &str,
) -> Result<Vec<PathBuf>, String> {
    let path = path.as_ref();
    let mut files = vec![];
    for dir_entry in
        std::fs::read_dir(path).map_err(|e| format!("Failed to read directory {path:?}: {e}"))?
    {
        let dir_entry = dir_entry
            .map_err(|e| format!("Failed to read directory entry of path {path:?}: {e}"))?;
        if dir_entry
            .metadata()
            .map_err(|e| {
                format!(
                    "Failed to get metadata for path {:?}: {e}",
                    dir_entry.path()
                )
            })?
            .is_file()
            && let Some(extension) = dir_entry.path().extension()
            && extension == expected_extension
        {
            files.push(dir_entry.path());
        }
    }
    Ok(files)
}

/// For a given path:
/// - If it's a file with the, returns that path.
/// - If it's a directory, returns paths to all direct child files with the `.ftl` extension in the
///   directory.
/// - Otherwise, an error message is returned. Symbolic links are unsupported.
fn get_ftl_paths_from_path<P: AsRef<Path>>(path: P) -> Result<Vec<PathBuf>, String> {
    let path = path.as_ref().to_path_buf();
    let mut files = vec![];
    let metadata = std::fs::metadata(&path)
        .map_err(|e| format!("Failed to obtain metadata for path {path:?}: {e}"))?;
    if metadata.is_symlink() {
        return Err(format!(
            "Symbolic links are not supported. Affected path: {path:?}"
        ));
    }
    if metadata.is_file() {
        files.push(path);
        return Ok(files);
    }
    if metadata.is_dir() {
        let files = get_file_paths_with_extension_from_directory(&path, "po")?;
        return Ok(files);
    }
    panic!("Path {path:?} has an unexpected type.");
}

pub fn parse_cli_args<T, F: FnOnce(Args) -> T>(parser: F) -> T {
    let mut args = std::env::args();
    // skip program name
    args.next();
    parser(args)
}

fn parse_next_arg<I: Iterator<Item = String>, T, P: FnOnce(String) -> T>(
    mut args: I,
    parser: P,
) -> Option<(T, I)> {
    args.next().map(|arg| (parser(arg), args))
}

pub fn parse_usize<I: Iterator<Item = String>>(
    args: I,
) -> Option<(Result<usize, ParseIntError>, I)> {
    parse_next_arg(args, |arg| arg.parse())
}

pub fn parse_single_file_arg<I: Iterator<Item = String>>(args: I) -> Option<(PathBuf, I)> {
    parse_next_arg(args, PathBuf::from)
}

/// https://github.com/projectfluent/fluent/blob/master/spec/fluent.ebnf
/// Identifier          ::= [a-zA-Z] [a-zA-Z0-9_-]*
pub fn parse_fluent_identifier<S: AsRef<str>>(arg: S) -> Result<(), String> {
    let str_arg = arg.as_ref();
    let mut chars = str_arg.chars();
    if let Some(first) = chars.next() {
        if !first.is_ascii_alphabetic() {
            return Err(format!(
                "Identifier must start with an alphabetic ASCII character but starts with '{first}'."
            ));
        }
    } else {
        return Err("Identifier may not be empty.".into());
    }
    for c in chars {
        if (!c.is_ascii_alphanumeric()) && c != '_' && c != '-' {
            return Err(format!(
                "Identifier characters must be alphanumeric ASCII or '_' or '-'. Found '{c}' instead."
            ));
        }
    }
    Ok(())
}

pub fn parse_fluent_identifiers<I: Iterator<Item = String>>(
    args: I,
) -> Result<Vec<String>, String> {
    let mut ids = vec![];
    for arg in args {
        match parse_fluent_identifier(&arg) {
            Ok(()) => ids.push(arg),
            Err(e) => {
                return Err(format!("Failed to parse Fluent identifier:\n{e}"));
            }
        }
    }
    Ok(ids)
}

pub fn parse_until_double_minus<I: Iterator<Item = String>, T, F: Fn(String) -> T>(
    mut args: I,
    parser: F,
) -> Option<(Vec<T>, I)> {
    let mut parsed = vec![];
    while let Some(arg) = args.next() {
        if arg == "--" {
            return Some((parsed, args));
        }
        parsed.push(parser(arg));
    }
    None
}

pub fn parse_directory<I: Iterator<Item = String>>(mut args: I) -> Result<(PathBuf, I), String> {
    let Some(path) = args.next() else {
        return Err("Tried to get directory argument but found no more argument.".into());
    };
    let path = PathBuf::from(path);
    let metadata = std::fs::metadata(&path)
        .map_err(|e| format!("Failed to obtain metadata for path {path:?}: {e}"))?;
    if !metadata.is_dir() {
        return Err(format!("Path {path:?} is not a directory."));
    }
    Ok((path, args))
}

pub fn parse_file_args<I: Iterator<Item = String>>(mut args: I) -> Result<Vec<PathBuf>, String> {
    let mut file_paths = match args.next() {
        Some(path) => get_ftl_paths_from_path(&path)?,
        None => {
            // default to all `.ftl` files in cwd
            get_ftl_paths_from_path(
                std::env::current_dir()
                    .map_err(|e| format!("Failed to obtain working directory: {e}"))?
                    .to_str()
                    .ok_or_else(|| {
                        "Current working directory has a non-UTF-8 path, which is unsupported."
                            .to_string()
                    })?,
            )?
        }
    };
    for path in args {
        file_paths.append(&mut get_ftl_paths_from_path(&path)?);
    }
    Ok(file_paths)
}

pub fn parse_file_args_with_required_file<I: Iterator<Item = String>>(
    mut args: I,
) -> Result<(PathBuf, Vec<PathBuf>), String> {
    let Some(required_file_path) = args.next() else {
        return Err("Too few arguments.".into());
    };
    let required_file_paths = get_ftl_paths_from_path(required_file_path)?;
    let required_file_path = match required_file_paths.len() {
        1 => required_file_paths[0].clone(),
        other => {
            return Err(format!(
                "First argument must refer to exactly one file.\n\
                Instead, it refers to {other}:\n{:?}\n\
                ",
                required_file_paths
            ));
        }
    };
    let other_file_paths = parse_file_args(args)?;
    Ok((required_file_path, other_file_paths))
}

fn tempfile<P: AsRef<Path>>(path: P) -> std::io::Result<PathBuf> {
    let mut path = path.as_ref().to_path_buf();
    let mut file_name = path
        .file_name()
        .expect("Called tempfile() with path not pointing to a file.")
        .to_owned();
    file_name.push("-ftl-tools-tmp_XXXXXX");
    path.set_file_name(file_name);
    Ok(nix::unistd::mkstemp(&path)?.1)
}

pub fn serialize_resource(resource: &Resource<String>) -> String {
    let mut serializer = Serializer::new(Options { with_junk: true });
    serializer.serialize_resource(resource);
    serializer.into_serialized_text()
}

pub fn serialize_resource_to_file<P: AsRef<Path>>(
    resource: &Resource<String>,
    path: P,
) -> std::io::Result<()> {
    let tmp_file = tempfile(&path)?;
    let text = serialize_resource(resource);
    std::fs::write(&tmp_file, text)?;
    std::fs::rename(&tmp_file, path)?;
    let _ = std::fs::remove_file(&tmp_file);
    Ok(())
}

pub fn serialize_resources_to_files<P: AsRef<Path>>(
    resources: &HashMap<P, Resource<String>>,
) -> std::io::Result<()> {
    let mut tmp_files = Vec::with_capacity(resources.len());
    for (path, resource) in resources {
        let new_path = tempfile(path)?;
        let backup_path = tempfile(path)?;
        let text = serialize_resource(resource);
        std::fs::write(&new_path, text)?;
        std::fs::copy(path, &backup_path)?;
        tmp_files.push((path, new_path, backup_path, false));
    }
    // At this point, the original files are still unmodified and backed up.
    // For atomic updates, renaming from `new_path` to `path` is performed.
    let mut error = None;
    for (path, new_path, _, updated) in &mut tmp_files {
        if let Err(e) = std::fs::rename(new_path, path) {
            error = Some(e);
        } else {
            *updated = true;
        }
    }
    if let Some(e) = error {
        // If an error occurred, we keep the temp files and tell the user about what happened.
        let mut error_message = format!(
            "Error occurred when updating files: {e}\n\
            The old and new versions of all files have been written to temporary files.\n\
            The following lines indicate for each FTL file whether it was updated, \
            and where to find the old and new versions.\n"
        );
        for (path, new_path, backup_path, updated) in tmp_files {
            let updated = if updated {
                "has been updated"
            } else {
                "has not been updated"
            };
            error_message.push_str(&format!(
                "{:?} {updated}. New version in {:?}. Old version in {:?}",
                path.as_ref(),
                new_path,
                backup_path
            ));
        }
        Err(std::io::Error::other(error_message))
    } else {
        for (_, new_path, backup_path, _) in tmp_files {
            let _ = std::fs::remove_file(new_path);
            let _ = std::fs::remove_file(backup_path);
        }
        Ok(())
    }
}

pub fn get_message_ids(resource: &Resource<String>) -> HashSet<&str> {
    let mut ids = HashSet::new();
    for entry in &resource.body {
        if let Entry::Message(message) = entry {
            ids.insert(message.id.name.as_str());
        }
    }
    ids
}
