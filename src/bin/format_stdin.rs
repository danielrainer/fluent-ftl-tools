use std::{io::Read, process::exit};

use fluent_ftl_tools::format::format_text;

fn main() {
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input).unwrap();
    match format_text(&input) {
        Ok(formatted_text) => print!("{formatted_text}"),
        Err(_) => {
            print!("{input}");
            exit(1);
        }
    }
}
