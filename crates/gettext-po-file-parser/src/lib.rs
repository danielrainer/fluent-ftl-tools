use std::{collections::HashMap, io::BufRead};

mod parsing_state {
    use std::collections::HashMap;

    pub(super) struct ParsingState {
        entry_state: EntryState,
        entries: HashMap<String, String>,
        line_number: usize,
    }

    #[derive(Clone)]
    enum EntryState {
        // Clean state, between entries. Can start parsing a new entry
        WaitingForEntry,
        StartedMsgid(String),
        StartedMsgstr(String, String),
    }

    enum LineType {
        Ignored,
        MsgidStart(String),
        MsgstrStart(String),
        QuotedString(String),
        Unsupported(String),
        Invalid(String),
    }

    fn parse_c_string_literal(literal: &str) -> Result<String, String> {
        let mut chars = literal.chars();
        let Some(first_char) = chars.next() else {
            return Err("Expected double-quote delimited string literal, but got nothing.".into());
        };
        if first_char != '"' {
            return Err(format!(
                "Expected double quote at start of string literal but got '{first_char}'"
            ));
        }
        let Some(last_char) = chars.next_back() else {
            return Err(
                "Expected double-quote at the end of a string literal, but got nothing.".into(),
            );
        };
        if last_char != '"' {
            return Err(format!(
                "Expected double quote at end of string literal but got '{last_char}'"
            ));
        }
        // remaining chars will now be the actual string content.

        let mut parsed_string = String::new();
        let mut is_escaped = false;
        for character in chars {
            if is_escaped {
                // This is a subset of escape sequences allowed in C string literals.
                // Some escape sequences don't make much sense in the context of PO files,
                // so if they appear it is probably unintentional and we warn about it.
                // In the case of octal and hexadecimal escapes, not allowing them avoid having to
                // deal with potentially non-UTF-8 values.
                // Unicode escape sequences (\uhhhh and \Uhhhhhhhh) are also unsupported, which
                // matches GNU gettext.
                match character {
                    'a' => {
                        parsed_string.push('\u{7}');
                    }
                    'b' => {
                        parsed_string.push('\u{8}');
                    }
                    'f' => {
                        parsed_string.push('\u{c}');
                    }
                    'n' => {
                        parsed_string.push('\n');
                    }
                    't' => {
                        parsed_string.push('\t');
                    }
                    'v' => {
                        parsed_string.push('\u{b}');
                    }
                    '"' => {
                        parsed_string.push('"');
                    }
                    '\\' => {
                        parsed_string.push('\\');
                    }
                    other => {
                        return Err(format!("Unsupported escaped character: {other}"));
                    }
                }
                is_escaped = false;
            } else {
                if character == '"' {
                    return Err("Unescaped double quote appeared in string literal before it was supposed to end.".into());
                }
                if character == '\\' {
                    is_escaped = true;
                } else {
                    parsed_string.push(character);
                }
            }
        }

        if is_escaped {
            return Err("Unterminated escape at the end of the string.".into());
        }

        Ok(parsed_string)
    }

    fn determine_line_type(line: &str) -> LineType {
        let mut chars = line.chars();
        let Some(first_char) = chars.next() else {
            return LineType::Ignored;
        };
        if first_char == '#' {
            return LineType::Ignored;
        }
        if line.starts_with("msgctxt ") {
            return LineType::Unsupported("msgctxt is not supported.".into());
        }
        if line.starts_with("msgid_plural ") {
            return LineType::Unsupported("msgid_plural is not supported.".into());
        }
        if line.starts_with("msgstr[") {
            return LineType::Unsupported("Indexed msgstr is not supported.".into());
        }
        let msgid_prefix = "msgid ";
        if line.starts_with(msgid_prefix) {
            let (_, potential_literal) = line.split_at(msgid_prefix.len());
            match parse_c_string_literal(potential_literal) {
                Ok(parsed_literal) => return LineType::MsgidStart(parsed_literal),
                Err(err) => {
                    return LineType::Invalid(format!(
                        "Expected C-style string literal after 'msgid ', but failed to parse one: {err}"
                    ));
                }
            }
        }
        let msgstr_prefix = "msgstr ";
        if line.starts_with(msgstr_prefix) {
            let (_, potential_literal) = line.split_at(msgstr_prefix.len());
            match parse_c_string_literal(potential_literal) {
                Ok(parsed_literal) => return LineType::MsgstrStart(parsed_literal),
                Err(err) => {
                    return LineType::Invalid(format!(
                        "Expected C-style string literal after 'msgstr ', but failed to parse one: {err}"
                    ));
                }
            }
        }
        if line.starts_with('"') {
            match parse_c_string_literal(line) {
                Ok(parsed_literal) => return LineType::QuotedString(parsed_literal),
                Err(err) => {
                    return LineType::Invalid(format!(
                        "Expected C-style string literal, but failed to parse one: {err}"
                    ));
                }
            }
        }
        LineType::Invalid("Line did not match the expected format.".into())
    }

    impl ParsingState {
        pub(super) fn new() -> Self {
            ParsingState {
                entry_state: EntryState::WaitingForEntry,
                entries: HashMap::new(),
                line_number: 0,
            }
        }

        pub(super) fn add_line(&mut self, line: &str) -> Result<(), String> {
            let state = self.entry_state.clone();
            self.line_number += 1;
            match determine_line_type(line) {
                LineType::Ignored => match state {
                    EntryState::WaitingForEntry => {
                        self.entry_state = EntryState::WaitingForEntry;
                    }
                    EntryState::StartedMsgid(msgid) => {
                        return Err(format!(
                            "line {}, msgid \"{msgid}\": msgid must be directly followed by msgstr.",
                            self.line_number
                        ));
                    }
                    EntryState::StartedMsgstr(msgid, msgstr) => {
                        if let Err(err) = self.add_entry(msgid, msgstr) {
                            return Err(format!("line {}: {err}", self.line_number));
                        }
                        self.entry_state = EntryState::WaitingForEntry;
                    }
                },
                LineType::MsgidStart(msgid) => match state {
                    EntryState::WaitingForEntry => {
                        self.entry_state = EntryState::StartedMsgid(msgid);
                    }
                    EntryState::StartedMsgid(second_msgid) => {
                        return Err(format!(
                            "line {}: two consecutive msgids: \"{msgid}\" and \"{second_msgid}\"",
                            self.line_number
                        ));
                    }
                    EntryState::StartedMsgstr(old_msgid, old_msgstr) => {
                        if let Err(err) = self.add_entry(old_msgid, old_msgstr) {
                            return Err(format!("line {}: {err}", self.line_number));
                        }
                        self.entry_state = EntryState::StartedMsgid(msgid);
                    }
                },
                LineType::MsgstrStart(msgstr) => match state {
                    EntryState::WaitingForEntry => {
                        return Err(format!(
                            "line {}: msgstr \"{msgstr}\" without preceding msgid.",
                            self.line_number
                        ));
                    }
                    EntryState::StartedMsgid(msgid) => {
                        self.entry_state = EntryState::StartedMsgstr(msgid, msgstr);
                    }
                    EntryState::StartedMsgstr(msgid, first_msgstr) => {
                        return Err(format!(
                            "line {}: two consecutive msgstrs for msgid \"{msgid}\": \"{first_msgstr}\" and \"{msgstr}\"",
                            self.line_number
                        ));
                    }
                },
                LineType::QuotedString(string) => match state {
                    EntryState::WaitingForEntry => {
                        return Err(format!(
                            "line {}: string literal not part of a msgid or msgstr: \"{string}\"",
                            self.line_number,
                        ));
                    }
                    EntryState::StartedMsgid(mut msgid) => {
                        msgid.push_str(&string);
                        self.entry_state = EntryState::StartedMsgid(msgid)
                    }
                    EntryState::StartedMsgstr(msgid, mut msgstr) => {
                        msgstr.push_str(&string);
                        self.entry_state = EntryState::StartedMsgstr(msgid, msgstr)
                    }
                },
                LineType::Unsupported(err) => {
                    return Err(format!(
                        "Unsupported syntax found in line {}: {err}",
                        self.line_number
                    ));
                }
                LineType::Invalid(err) => {
                    return Err(format!(
                        "Invalid syntax found in line {}: {err}",
                        self.line_number
                    ));
                }
            }
            Ok(())
        }

        /// Call this after all lines have been parsed to obtain the parsed localization map.
        pub(super) fn finish(mut self) -> Result<HashMap<String, String>, String> {
            match &self.entry_state {
                EntryState::WaitingForEntry => {}
                EntryState::StartedMsgid(msgid) => {
                    return Err(format!(
                        "Trailing msgid '{msgid}' without corresponding msgstr."
                    ));
                }
                EntryState::StartedMsgstr(msgid, msgstr) => {
                    self.add_entry(msgid.to_owned(), msgstr.to_owned())?;
                }
            }
            Ok(self.entries)
        }

        /// This adds entries with empty msgstr to enable duplicate msgid detection.
        fn add_entry(&mut self, msgid: String, msgstr: String) -> Result<(), String> {
            match self.entries.insert(msgid.clone(), msgstr.clone()) {
                Some(original_msgstr) => Err(format!(
                    "Duplicate msgid '{msgid}'. First translated as {original_msgstr}, then as {msgstr}"
                )),
                None => Ok(()),
            }
        }

        pub(super) fn has_entry(&self) -> bool {
            !self.entries.is_empty()
        }
    }
}

pub fn parse_po_file(content: &[u8]) -> Result<HashMap<String, String>, String> {
    let mut state = parsing_state::ParsingState::new();
    for line in content.lines() {
        match line {
            Ok(line) => {
                state.add_line(&line)?;
            }
            Err(err) => return Err(format!("Failed to read line: {err}")),
        }
    }
    state.finish()
}

/// Indices start at 0.
pub fn get_message_at_line_index(
    mut index: usize,
    content: &[u8],
) -> Result<(String, String), String> {
    let mut lines = vec![];
    for line in content.lines() {
        match line {
            Ok(line) => {
                lines.push(line);
            }
            Err(err) => return Err(format!("Failed to read line: {err}")),
        }
    }
    if index >= lines.len() {
        return Err("Specified index exceeds the number of lines in the file".into());
    }
    let mut state = parsing_state::ParsingState::new();
    while let Err(err) = state.add_line(&lines[index]) {
        if index == 0 {
            return Err(format!("Failed to parse PO file: {err}"));
        }
        state = parsing_state::ParsingState::new();
        index -= 1;
    }
    while !state.has_entry() && index < lines.len() - 1 {
        index += 1;
        state.add_line(&lines[index])?;
    }
    let mut entries = state.finish()?.into_iter();
    let Some(entry) = entries.next() else {
        return Err("Did not find any entry.".into());
    };
    if entries.next().is_some() {
        panic!("Found more than one entry.");
    }
    Ok(entry)
}
