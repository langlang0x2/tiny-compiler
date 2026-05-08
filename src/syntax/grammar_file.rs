use std::fs;

use super::Grammar;

pub fn load_grammar(path: &str) -> Result<Grammar, String> {
    let content = fs::read_to_string(path)
        .map_err(|err| format!("failed to read grammar file: {path} ({err})"))?;
    parse_grammar(&content)
}

pub fn parse_grammar(content: &str) -> Result<Grammar, String> {
    let mut grammar = Grammar::new();
    let mut start_symbol = None;
    let mut productions = Vec::new();

    for (line_index, raw_line) in content.lines().enumerate() {
        let line_number = line_index + 1;
        let line = strip_comment(raw_line).trim();
        if line.is_empty() {
            continue;
        }

        if let Some(rest) = line.strip_prefix("%start") {
            let name = single_value(rest, line_number, "%start")?;
            grammar.add_non_terminal(name);
            start_symbol = Some(name.to_string());
            continue;
        }

        if let Some(rest) = line.strip_prefix("%terminals") {
            for terminal in split_symbols(rest) {
                grammar.add_terminal(terminal);
            }
            continue;
        }

        if let Some(rest) = line.strip_prefix("%nonterminals") {
            for non_terminal in split_symbols(rest) {
                grammar.add_non_terminal(non_terminal);
            }
            continue;
        }

        let (head, body_text) = line
            .split_once("->")
            .ok_or_else(|| format!("line {line_number} is missing `->`"))?;
        let head = head.trim();
        if head.is_empty() {
            return Err(format!("line {line_number} is missing production head"));
        }
        grammar.add_non_terminal(head);

        for alternative in body_text.split('|') {
            let body = split_symbols(alternative)
                .into_iter()
                .filter(|symbol| *symbol != "e")
                .map(str::to_string)
                .collect::<Vec<_>>();
            productions.push((head.to_string(), body));
        }
    }

    let start_symbol =
        start_symbol.ok_or_else(|| "grammar file is missing `%start`".to_string())?;
    grammar.set_start_symbol(&start_symbol);

    for (head, body) in productions {
        for symbol in &body {
            if grammar.has_symbol(symbol) {
                continue;
            }
            grammar.add_terminal(symbol);
        }

        let body_refs = body.iter().map(String::as_str).collect::<Vec<_>>();
        grammar.add_production(&head, &body_refs);
    }

    grammar.compute_first_sets();
    grammar.compute_follow_sets();
    Ok(grammar)
}

fn single_value<'a>(rest: &'a str, line_number: usize, directive: &str) -> Result<&'a str, String> {
    let values = split_symbols(rest);
    match values.as_slice() {
        [value] => Ok(value),
        [] => Err(format!(
            "line {line_number} is missing value for {directive}"
        )),
        _ => Err(format!(
            "line {line_number} has too many values for {directive}"
        )),
    }
}

fn split_symbols(text: &str) -> Vec<&str> {
    text.split_whitespace().collect()
}

fn strip_comment(line: &str) -> &str {
    line.split_once('#').map_or(line, |(before, _)| before)
}
