use fluent_ftl_tools::{
    consistency::check_all_resource_files, parse_cli_args, parse_file_args_with_required_file,
};

fn print_help() {
    eprintln!(
        "Usage:\n\
        The first argument specifies the default language file, which must contain all message IDs.\n\
        The remaining arguments specify files whose message IDs must also appear in the default language file \
        and whose message variable usage should be checked for consistency.\n\
        Arguments can be paths to directories, in which case all files directly in the specified directory with the `.ftl` extension will be used.\n\
        The first argument must identify exactly one file.\n\
        If no second argument is given, all `.ftl` files in the current directory are used."
    );
}
fn main() {
    let (default_language_path, other_paths) = parse_cli_args(parse_file_args_with_required_file)
        .unwrap_or_else(|e| {
            eprintln!("Error:\n{e}");
            print_help();
            std::process::exit(1);
        });
    if let Err(e) = check_all_resource_files(&default_language_path, &other_paths) {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
