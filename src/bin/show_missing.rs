use fluent_ftl_tools::{
    missing::find_missing_message_ids_in_files, parse_cli_args, parse_file_args_with_required_file,
};

fn print_help() {
    eprintln!(
        "Usage:\n\
        The first argument specifies the file containing the expected message IDs.\n\
        For the remaining arguments, the message IDs appearing in the first argument file but not in them will be printed.\n\
        Arguments can be paths to directories, in which case all files directly in the specified directory with the `.ftl` extension will be used.\n\
        The first argument must identify exactly one file.\n\
        If no second argument is given, all `.ftl` files in the current directory are used."
    );
}

fn main() {
    let (expected_ids_path, other_paths) = parse_cli_args(parse_file_args_with_required_file)
        .unwrap_or_else(|e| {
            eprintln!("Error:\n{e}");
            print_help();
            std::process::exit(1);
        });
    match find_missing_message_ids_in_files(&expected_ids_path, &other_paths) {
        Ok(None) => {
            println!("No missing messages.");
        }
        Ok(Some(missing_message)) => {
            println!("{missing_message}");
        }
        Err(e) => {
            eprintln!("Error:\n{e}");
            std::process::exit(1);
        }
    }
}
