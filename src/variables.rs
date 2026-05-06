use anyhow::{Result, anyhow, bail};
use fluent_syntax::ast::{
    CallArguments, Expression, Identifier, InlineExpression, Message, PatternElement, Term,
};
use std::collections::{HashMap, HashSet};

const MAX_RECURSION_DEPTH: usize = 100;
const RECURSION_DEPTH_ERROR: &str =
    "Exceeded the maximum recursion depth. This might indicate a reference cycle in the FTL data.";
macro_rules! check_depth {
    ($depth:expr) => {
        if $depth > MAX_RECURSION_DEPTH {
            bail!("{RECURSION_DEPTH_ERROR}");
        }
    };
}

fn get_term_by_id<'a, S: AsRef<str>>(
    term_id: &str,
    terms: &HashMap<&'a str, &'a Term<S>>,
) -> Result<&'a Term<S>> {
    terms
        .get(term_id)
        .copied()
        .ok_or(anyhow!("No term with id '{term_id}' found."))
}

fn get_message_by_id<'a, S: AsRef<str>>(
    message_id: &str,
    messages: &HashMap<&'a str, &'a Message<S>>,
) -> Result<&'a Message<S>> {
    messages
        .get(message_id)
        .copied()
        .ok_or(anyhow!("No message with id '{message_id}' found."))
}

fn get_variables_of_term<'a, S: AsRef<str>>(
    term_id: &'a str,
    attribute: &'a Option<Identifier<S>>,
    messages: &HashMap<&'a str, &'a Message<S>>,
    terms: &HashMap<&'a str, &'a Term<S>>,
    variables_found: &mut HashSet<&'a str>,
    bound_variables: &HashSet<&'a str>,
    depth: usize,
) -> Result<()> {
    check_depth!(depth);
    let term = get_term_by_id(term_id, terms)?;
    if let Some(attribute_to_visit) = attribute {
        let attribute_name = attribute_to_visit.name.as_ref();
        for attribute in &term.attributes {
            if attribute.id.name.as_ref() == attribute_name {
                for pattern_element in &attribute.value.elements {
                    get_variables_of_pattern_element(
                        pattern_element,
                        messages,
                        terms,
                        variables_found,
                        bound_variables,
                        depth + 1,
                    )?;
                }
                return Ok(());
            }
        }
        bail!(
            "Tried to use attribute '{attribute_name}' of term '{term_id}', which does not exist."
        );
    }
    for pattern_element in &term.value.elements {
        get_variables_of_pattern_element(
            pattern_element,
            messages,
            terms,
            variables_found,
            bound_variables,
            depth + 1,
        )?;
    }
    for attribute in &term.attributes {
        for pattern_element in &attribute.value.elements {
            get_variables_of_pattern_element(
                pattern_element,
                messages,
                terms,
                variables_found,
                bound_variables,
                depth + 1,
            )?;
        }
    }
    Ok(())
}

fn get_variables_of_call_arguments<'a, S: AsRef<str>>(
    call_arguments: &'a CallArguments<S>,
    messages: &HashMap<&'a str, &'a Message<S>>,
    terms: &HashMap<&'a str, &'a Term<S>>,
    variables_found: &mut HashSet<&'a str>,
    bound_variables: &HashSet<&'a str>,
    depth: usize,
) -> Result<HashSet<&'a str>> {
    check_depth!(depth);
    for inline_expression in &call_arguments.positional {
        get_variables_of_inline_expression(
            inline_expression,
            messages,
            terms,
            variables_found,
            bound_variables,
            depth + 1,
        )?;
    }
    let mut introduced_variables = HashSet::new();
    for named_argument in &call_arguments.named {
        introduced_variables.insert(named_argument.name.name.as_ref());
        get_variables_of_inline_expression(
            &named_argument.value,
            messages,
            terms,
            variables_found,
            bound_variables,
            depth + 1,
        )?;
    }
    Ok(HashSet::from_iter(
        introduced_variables.union(bound_variables).copied(),
    ))
}

fn get_variables_of_inline_expression<'a, S: AsRef<str>>(
    inline_expression: &'a InlineExpression<S>,
    messages: &HashMap<&'a str, &'a Message<S>>,
    terms: &HashMap<&'a str, &'a Term<S>>,
    variables_found: &mut HashSet<&'a str>,
    bound_variables: &HashSet<&'a str>,
    depth: usize,
) -> Result<()> {
    check_depth!(depth);
    match inline_expression {
        InlineExpression::StringLiteral { .. } => {}
        InlineExpression::NumberLiteral { .. } => {}
        InlineExpression::FunctionReference { arguments, .. } => {
            get_variables_of_call_arguments(
                arguments,
                messages,
                terms,
                variables_found,
                bound_variables,
                depth + 1,
            )?;
        }
        InlineExpression::MessageReference { id, attribute } => {
            get_variables_of_message(
                id.name.as_ref(),
                attribute,
                messages,
                terms,
                variables_found,
                bound_variables,
                depth + 1,
            )?;
        }
        InlineExpression::TermReference {
            id,
            attribute,
            arguments,
        } => {
            if let Some(call_arguments) = arguments {
                let updated_bound_variables = get_variables_of_call_arguments(
                    call_arguments,
                    messages,
                    terms,
                    variables_found,
                    bound_variables,
                    depth + 1,
                )?;
                get_variables_of_term(
                    id.name.as_ref(),
                    attribute,
                    messages,
                    terms,
                    variables_found,
                    &updated_bound_variables,
                    depth + 1,
                )?;
            } else {
                get_variables_of_term(
                    id.name.as_ref(),
                    attribute,
                    messages,
                    terms,
                    variables_found,
                    bound_variables,
                    depth + 1,
                )?;
            }
        }
        InlineExpression::VariableReference { id } => {
            variables_found.insert(id.name.as_ref());
        }
        InlineExpression::Placeable { expression } => {
            get_variables_of_expression(
                expression,
                messages,
                terms,
                variables_found,
                bound_variables,
                depth + 1,
            )?;
        }
    }
    Ok(())
}

fn get_variables_of_expression<'a, S: AsRef<str>>(
    expression: &'a Expression<S>,
    messages: &HashMap<&'a str, &'a Message<S>>,
    terms: &HashMap<&'a str, &'a Term<S>>,
    variables_found: &mut HashSet<&'a str>,
    bound_variables: &HashSet<&'a str>,
    depth: usize,
) -> Result<()> {
    check_depth!(depth);
    match expression {
        Expression::Select { selector, variants } => {
            get_variables_of_inline_expression(
                selector,
                messages,
                terms,
                variables_found,
                bound_variables,
                depth + 1,
            )?;
            for variant in variants {
                for pattern_element in &variant.value.elements {
                    get_variables_of_pattern_element(
                        pattern_element,
                        messages,
                        terms,
                        variables_found,
                        bound_variables,
                        depth + 1,
                    )?;
                }
            }
        }
        Expression::Inline(inline_expression) => {
            get_variables_of_inline_expression(
                inline_expression,
                messages,
                terms,
                variables_found,
                bound_variables,
                depth + 1,
            )?;
        }
    }
    Ok(())
}

fn get_variables_of_pattern_element<'a, S: AsRef<str>>(
    pattern_element: &'a PatternElement<S>,
    messages: &HashMap<&'a str, &'a Message<S>>,
    terms: &HashMap<&'a str, &'a Term<S>>,
    variables_found: &mut HashSet<&'a str>,
    bound_variables: &HashSet<&'a str>,
    depth: usize,
) -> Result<()> {
    check_depth!(depth);
    if let PatternElement::Placeable { expression } = pattern_element {
        get_variables_of_expression(
            expression,
            messages,
            terms,
            variables_found,
            bound_variables,
            depth + 1,
        )?;
    }
    Ok(())
}

fn get_variables_of_message<'a, S: AsRef<str>>(
    message_id: &'a str,
    attribute: &'a Option<Identifier<S>>,
    messages: &HashMap<&'a str, &'a Message<S>>,
    terms: &HashMap<&'a str, &'a Term<S>>,
    variables_found: &mut HashSet<&'a str>,
    bound_variables: &HashSet<&'a str>,
    depth: usize,
) -> Result<()> {
    check_depth!(depth);
    let message = get_message_by_id(message_id, messages)?;
    if let Some(attribute_to_visit) = attribute {
        let attribute_name = attribute_to_visit.name.as_ref();
        for attribute in &message.attributes {
            if attribute.id.name.as_ref() == attribute_name {
                for pattern_element in &attribute.value.elements {
                    get_variables_of_pattern_element(
                        pattern_element,
                        messages,
                        terms,
                        variables_found,
                        bound_variables,
                        depth + 1,
                    )?;
                }
                return Ok(());
            }
        }
        bail!(
            "Tried to use attribute '{attribute_name}' of message '{message_id}', which does not exist."
        );
    }
    if let Some(pattern) = &message.value {
        for pattern_element in &pattern.elements {
            get_variables_of_pattern_element(
                pattern_element,
                messages,
                terms,
                variables_found,
                bound_variables,
                depth + 1,
            )?;
        }
    }
    for attribute in &message.attributes {
        for pattern_element in &attribute.value.elements {
            get_variables_of_pattern_element(
                pattern_element,
                messages,
                terms,
                variables_found,
                bound_variables,
                depth + 1,
            )?;
        }
    }
    Ok(())
}

/// Returns the set of all unbound variables used in the message and its recursive dependencies.
pub(crate) fn variables_in_message_impl<'a, S: AsRef<str>>(
    message_id: &'a str,
    messages: &HashMap<&'a str, &'a Message<S>>,
    terms: &HashMap<&'a str, &'a Term<S>>,
) -> Result<HashSet<&'a str>> {
    let mut variables_found = HashSet::new();
    let bound_variables = HashSet::new();
    get_variables_of_message(
        message_id,
        &None,
        messages,
        terms,
        &mut variables_found,
        &bound_variables,
        0,
    )?;
    Ok(variables_found)
}

fn report_set_mismatches(
    error_str: &mut String,
    message_id: &str,
    expected: &HashSet<&str>,
    actual: &HashSet<&str>,
) {
    use std::fmt::Write as _;
    let mut missing = expected.difference(actual).peekable();
    if missing.peek().is_some() {
        let _ = writeln!(
            error_str,
            "The following variables were expected but not used in message {message_id}:"
        );
        for variable in missing {
            let _ = write!(error_str, " {variable}");
        }
        let _ = writeln!(error_str);
    }
    let mut unexpected = actual.difference(expected).peekable();
    if unexpected.peek().is_some() {
        let _ = writeln!(
            error_str,
            "The following variables were not expected but used in the message {message_id}:"
        );
        for variable in unexpected {
            let _ = write!(error_str, " {variable}");
        }
        let _ = writeln!(error_str);
    }
}

pub(crate) fn check_if_expected_variables_match_message_impl<'a, S: AsRef<str>>(
    expected: &HashSet<&str>,
    message_id: &'a str,
    messages: &HashMap<&'a str, &'a Message<S>>,
    terms: &HashMap<&'a str, &'a Term<S>>,
) -> Result<()> {
    let actual = variables_in_message_impl(message_id, messages, terms)?;
    let mut errors = String::new();
    report_set_mismatches(&mut errors, message_id, expected, &actual);
    if errors.is_empty() {
        Ok(())
    } else {
        bail!("{errors}")
    }
}

pub(crate) fn check_variables_of_all_messages_impl(
    expected: &HashMap<&str, HashSet<&str>>,
    actual: &HashMap<&str, HashSet<&str>>,
) -> Result<()> {
    let mut errors = String::new();
    for (&id, actual_vars) in actual {
        if let Some(expected_vars) = expected.get(id) {
            report_set_mismatches(&mut errors, id, expected_vars, actual_vars);
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        bail!("{errors}")
    }
}
