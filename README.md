# Fluent FTL tools

Moved to [Codeberg](https://codeberg.org/danielrainer/fluent-ftl-tools).

## Tools

Contains a few tools for working with Fluent FTL files:

### check-consistency

This tool is not very fleshed out yet.
So far, it checks whether FTL files can be parsed, conform to the expected format, contain no duplicate IDs, and contain only message IDs also found in the default language.
Additional checks which would be useful but are not implemented yet:

- Check if the same set of variables is used for each message across files.
- Check if referenced terms exist.
- Check if the variables used in terms are defined (syntactiacally, by appearing in the message from which the term is used)

### format

For scripting/CLI use to format FTL files according to our custom format.
That is, we don't allow standalone comments except for file-level comments in the beginning, and we require all terms to appear before all messages, and both terms and messages are sorted alphabetically by their IDs.

### format-stdin

Also for formatting, but it takes its input from stdin and writes the formatted version to stdout (or echos the input if something goes wrong).
This is intended to be used for auto-formatting in editors.

### po-convert

Can be used to convert entries of gettext PO files to Fluent messages.
It is only useful for the transition from gettext to Fluent.

### rename

Used for renaming message IDs or variables within messages.
Solves the problem of applying the renaming to all FTL files.

### show-missing

Allows translators to see which messages are not yet contained in a FTL file (but are present in another one).

## Building

Note that this code relies on a version of `fluent-syntax` which has not been mainlined yet.
A working version can be found at
https://github.com/danielrainer/fluent-rs/tree/make_parser_pub
However, the `Cargo.toml` specifies a path dependency, so you will have to put a working version of `fluent-rs` at the specified relative path or modify `Cargo.toml` to run the code.
