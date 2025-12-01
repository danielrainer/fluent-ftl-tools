use std::{io::Read, process::exit};

use fluent_ftl_tools::{format_resource, parse_str_as_syntax_resource, serialize_resource};

fn main() {
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input).unwrap();
    macro_rules! fail {
        () => {{
            print!("{input}");
            exit(1);
        }};
    }
    let resource = parse_str_as_syntax_resource(&input)
        .map_err(|_| fail!())
        .unwrap();
    let formatted_resource = format_resource(resource).map_err(|_| fail!()).unwrap();
    print!("{}", serialize_resource(&formatted_resource));
}
