#![allow(dead_code)]

use std::collections::{BTreeSet, HashMap, VecDeque};

use super::nfa::{CharSetTable, DriverType, Edge, Graph, State, StateType};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScannedToken {
    pub category: String,
    pub lexeme: String,
    pub line: usize,
    pub column: usize,
}

pub fn nfa_to_dfa(nfa: &Graph, charset_table: &mut CharSetTable) -> Result<Graph, String> {
    let char_classes = build_char_classes(nfa, charset_table)?;
    let start_set = nfa.epsilon_closure(&BTreeSet::from([nfa.start_state]));
    let mut subset_to_state = HashMap::new();
    let mut queue = VecDeque::new();
    let mut dfa_states = Vec::new();
    let mut dfa_edges = Vec::new();

    subset_to_state.insert(start_set.clone(), 0usize);
    queue.push_back(start_set.clone());
    dfa_states.push(build_dfa_state(0, &start_set, nfa));

    while let Some(current_set) = queue.pop_front() {
        let from_id = subset_to_state[&current_set];

        for &(class_id, representative) in &char_classes {
            let next_set = nfa_dtran_char(nfa, &current_set, representative, charset_table)?;
            if next_set.is_empty() {
                continue;
            }

            let to_id = if let Some(id) = subset_to_state.get(&next_set) {
                *id
            } else {
                let id = dfa_states.len();
                subset_to_state.insert(next_set.clone(), id);
                queue.push_back(next_set.clone());
                dfa_states.push(build_dfa_state(id, &next_set, nfa));
                id
            };

            dfa_edges.push(Edge {
                from_state: from_id,
                to_state: to_id,
                driver_type: DriverType::Charset,
                driver_id: Some(class_id),
            });
        }
    }

    let end_state = dfa_states
        .iter()
        .find(|state| state.state_type == StateType::Match)
        .map(|state| state.id)
        .unwrap_or(0);

    Ok(Graph {
        states: dfa_states,
        edges: dfa_edges,
        start_state: 0,
        end_state,
    })
}

pub fn dfa_match(
    dfa: &Graph,
    charset_table: &CharSetTable,
    input: &str,
) -> Result<Option<String>, String> {
    let mut current_state = dfa.start_state;

    for ch in input.chars() {
        let next_state = dfa
            .edges
            .iter()
            .find(|edge| {
                edge.from_state == current_state
                    && edge.driver_id.is_some()
                    && edge_matches(edge.driver_type, edge.driver_id.unwrap(), ch, charset_table)
                        .unwrap_or(false)
            })
            .map(|edge| edge.to_state);

        match next_state {
            Some(state_id) => current_state = state_id,
            None => return Ok(None),
        }
    }

    let state = dfa
        .states
        .iter()
        .find(|state| state.id == current_state)
        .ok_or_else(|| format!("unknown dfa state: {current_state}"))?;

    if state.state_type == StateType::Match {
        Ok(state.category.clone())
    } else {
        Ok(None)
    }
}

pub fn dfa_scan(
    dfa: &Graph,
    charset_table: &CharSetTable,
    input: &str,
) -> Result<Vec<ScannedToken>, String> {
    let chars = input.chars().collect::<Vec<_>>();
    let mut tokens = Vec::new();
    let mut index = 0;
    let mut line = 1;
    let mut column = 1;

    while index < chars.len() {
        let ch = chars[index];

        if ch.is_whitespace() {
            advance_position(ch, &mut line, &mut column);
            index += 1;
            continue;
        }

        if ch == '{' {
            let start_line = line;
            let start_column = column;
            index += 1;
            advance_position(ch, &mut line, &mut column);
            while index < chars.len() && chars[index] != '}' {
                let skipped = chars[index];
                advance_position(skipped, &mut line, &mut column);
                index += 1;
            }
            if index == chars.len() {
                return Err(format!(
                    "unterminated comment at line {start_line}, column {start_column}"
                ));
            }
            advance_position(chars[index], &mut line, &mut column);
            index += 1;
            continue;
        }

        let start_index = index;
        let start_line = line;
        let start_column = column;
        let mut current_state = dfa.start_state;
        let mut probe = index;
        let mut last_match: Option<(usize, String)> =
            accepting_category(dfa, current_state).map(|category| (probe, category));

        while probe < chars.len() {
            let Some(next_state) = next_state(dfa, charset_table, current_state, chars[probe])?
            else {
                break;
            };
            current_state = next_state;
            probe += 1;
            if let Some(category) = accepting_category(dfa, current_state) {
                last_match = Some((probe, category));
            }
        }

        let Some((end_index, category)) = last_match else {
            return Err(format!(
                "unrecognized character '{}' at line {}, column {}",
                ch, start_line, start_column
            ));
        };

        let lexeme = chars[start_index..end_index].iter().collect::<String>();
        for consumed in &chars[start_index..end_index] {
            advance_position(*consumed, &mut line, &mut column);
        }
        index = end_index;

        tokens.push(ScannedToken {
            category,
            lexeme,
            line: start_line,
            column: start_column,
        });
    }

    Ok(tokens)
}

fn next_state(
    dfa: &Graph,
    charset_table: &CharSetTable,
    current_state: usize,
    ch: char,
) -> Result<Option<usize>, String> {
    for edge in dfa
        .edges
        .iter()
        .filter(|edge| edge.from_state == current_state)
    {
        let Some(driver_id) = edge.driver_id else {
            continue;
        };
        if edge_matches(edge.driver_type, driver_id, ch, charset_table)? {
            return Ok(Some(edge.to_state));
        }
    }
    Ok(None)
}

fn accepting_category(dfa: &Graph, state_id: usize) -> Option<String> {
    dfa.states
        .iter()
        .find(|state| state.id == state_id && state.state_type == StateType::Match)
        .and_then(|state| state.category.clone())
}

fn advance_position(ch: char, line: &mut usize, column: &mut usize) {
    if ch == '\n' {
        *line += 1;
        *column = 1;
    } else {
        *column += 1;
    }
}

fn edge_matches(
    driver_type: DriverType,
    driver_id: usize,
    ch: char,
    charset_table: &CharSetTable,
) -> Result<bool, String> {
    match driver_type {
        DriverType::Null => Ok(false),
        DriverType::Char => Ok(driver_id == ch as usize),
        DriverType::Charset => charset_table.contains(driver_id, ch),
    }
}

fn collect_input_chars(nfa: &Graph, charset_table: &CharSetTable) -> Result<Vec<char>, String> {
    let mut chars = BTreeSet::new();

    for edge in &nfa.edges {
        let Some(driver_id) = edge.driver_id else {
            continue;
        };

        match edge.driver_type {
            DriverType::Null => {}
            DriverType::Char => {
                let ch = char::from_u32(driver_id as u32)
                    .ok_or_else(|| format!("invalid char id: {driver_id}"))?;
                chars.insert(ch);
            }
            DriverType::Charset => {
                for (from, to) in charset_table.segments(driver_id)? {
                    let mut code = from as u32;
                    while code <= to as u32 {
                        let ch = char::from_u32(code)
                            .ok_or_else(|| format!("invalid char code: {code}"))?;
                        chars.insert(ch);
                        code += 1;
                    }
                }
            }
        }
    }

    Ok(chars.into_iter().collect())
}

fn nfa_dtran_char(
    nfa: &Graph,
    states: &BTreeSet<usize>,
    ch: char,
    charset_table: &CharSetTable,
) -> Result<BTreeSet<usize>, String> {
    let mut moved = BTreeSet::new();

    for edge in &nfa.edges {
        if !states.contains(&edge.from_state) {
            continue;
        }

        let Some(driver_id) = edge.driver_id else {
            continue;
        };

        let matches = match edge.driver_type {
            DriverType::Null => false,
            DriverType::Char => driver_id == ch as usize,
            DriverType::Charset => charset_table.contains(driver_id, ch)?,
        };

        if matches {
            moved.insert(edge.to_state);
        }
    }

    Ok(nfa.epsilon_closure(&moved))
}

fn build_char_classes(
    nfa: &Graph,
    charset_table: &mut CharSetTable,
) -> Result<Vec<(usize, char)>, String> {
    let chars = collect_input_chars(nfa, charset_table)?;
    let consuming_edges = nfa
        .edges
        .iter()
        .filter(|edge| edge.driver_type != DriverType::Null && edge.driver_id.is_some())
        .cloned()
        .collect::<Vec<_>>();
    let mut classes: HashMap<Vec<bool>, BTreeSet<char>> = HashMap::new();

    for ch in chars {
        let mut signature = Vec::with_capacity(consuming_edges.len());
        for edge in &consuming_edges {
            let driver_id = edge.driver_id.unwrap();
            let matched = match edge.driver_type {
                DriverType::Null => false,
                DriverType::Char => driver_id == ch as usize,
                DriverType::Charset => charset_table.contains(driver_id, ch)?,
            };
            signature.push(matched);
        }
        classes.entry(signature).or_default().insert(ch);
    }

    let mut class_ids = Vec::new();
    for chars in classes.into_values() {
        let representative = *chars.iter().next().unwrap();
        let class_id = charset_table.from_chars(&chars);
        class_ids.push((class_id, representative));
    }
    class_ids.sort_unstable_by_key(|(class_id, _)| *class_id);
    Ok(class_ids)
}

fn build_dfa_state(id: usize, subset: &BTreeSet<usize>, nfa: &Graph) -> State {
    let accepting_state = subset
        .iter()
        .filter_map(|state_id| nfa.states.iter().find(|state| state.id == *state_id))
        .find(|state| state.state_type == StateType::Match);

    match accepting_state {
        Some(state) => State {
            id,
            state_type: StateType::Match,
            category: state.category.clone(),
        },
        None => State {
            id,
            state_type: StateType::Unmatch,
            category: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{dfa_match, nfa_to_dfa};
    use crate::lexer::{
        build_charset_table, build_lexer_program, build_token_regular_tables, dfa_scan, merge_nfas,
        rule::parse_rules,
    };

    #[test]
    fn subset_construction_builds_dfa_for_id() {
        let rules = parse_rules(
            r#"
letter 'a'~'z' | 'A'~'Z'
digit '0'~'9'
ID letter (letter | digit)*
"#,
        )
        .unwrap();
        let (mut charset_table, _) = build_charset_table(&rules).unwrap();
        let table = build_token_regular_tables(&rules)
            .unwrap()
            .into_iter()
            .find(|table| table.token_name == "ID")
            .unwrap();
        let nfa = table.build_nfa(&mut charset_table).unwrap();

        let dfa = nfa_to_dfa(&nfa, &mut charset_table).unwrap();

        assert!(!dfa.states.is_empty());
        assert!(!dfa.edges.is_empty());
        assert!(
            dfa.states
                .iter()
                .any(|state| state.category.as_deref() == Some("ID"))
        );
    }

    #[test]
    fn subset_construction_works_after_merging_token_nfas() {
        let rules = parse_rules(
            r#"
letter 'a'~'z' | 'A'~'Z'
digit '0'~'9'
IF 'i''f'
ID letter (letter | digit)*
"#,
        )
        .unwrap();
        let (mut charset_table, _) = build_charset_table(&rules).unwrap();
        let nfas = build_token_regular_tables(&rules)
            .unwrap()
            .into_iter()
            .map(|table| table.build_nfa(&mut charset_table).unwrap())
            .collect::<Vec<_>>();

        let merged_nfa = merge_nfas(&nfas);
        let dfa = nfa_to_dfa(&merged_nfa, &mut charset_table).unwrap();

        assert!(!dfa.states.is_empty());
        assert!(!dfa.edges.is_empty());
        assert!(
            dfa.states
                .iter()
                .any(|state| matches!(state.category.as_deref(), Some("IF") | Some("ID")))
        );
    }

    #[test]
    fn dfa_match_can_recognize_a_token() {
        let rules = parse_rules(
            r#"
letter 'a'~'z' | 'A'~'Z'
digit '0'~'9'
ID letter (letter | digit)*
NUM digit+
"#,
        )
        .unwrap();
        let (mut charset_table, _) = build_charset_table(&rules).unwrap();
        let nfas = build_token_regular_tables(&rules)
            .unwrap()
            .into_iter()
            .map(|table| table.build_nfa(&mut charset_table).unwrap())
            .collect::<Vec<_>>();

        let merged_nfa = merge_nfas(&nfas);
        let dfa = nfa_to_dfa(&merged_nfa, &mut charset_table).unwrap();

        assert_eq!(
            dfa_match(&dfa, &charset_table, "abc123").unwrap(),
            Some("ID".to_string())
        );
        assert_eq!(
            dfa_match(&dfa, &charset_table, "12345").unwrap(),
            Some("NUM".to_string())
        );
        assert_eq!(dfa_match(&dfa, &charset_table, "@@@").unwrap(), None);
    }

    #[test]
    fn dfa_match_handles_charset_and_char_overlap() {
        let rules = parse_rules("A ('a'|'b')*'a''b''b'").unwrap();
        let (mut charset_table, _) = build_charset_table(&rules).unwrap();
        let nfas = build_token_regular_tables(&rules)
            .unwrap()
            .into_iter()
            .map(|table| table.build_nfa(&mut charset_table).unwrap())
            .collect::<Vec<_>>();

        let merged_nfa = merge_nfas(&nfas);
        let dfa = nfa_to_dfa(&merged_nfa, &mut charset_table).unwrap();

        assert_eq!(
            dfa_match(&dfa, &charset_table, "aabb").unwrap(),
            Some("A".to_string())
        );
    }

    #[test]
    fn dfa_uses_compressed_char_classes() {
        let rules = parse_rules(
            r#"
letter 'a'~'z' | 'A'~'Z'
digit '0'~'9'
ID letter (letter | digit)*
"#,
        )
        .unwrap();
        let (mut charset_table, _) = build_charset_table(&rules).unwrap();
        let nfas = build_token_regular_tables(&rules)
            .unwrap()
            .into_iter()
            .map(|table| table.build_nfa(&mut charset_table).unwrap())
            .collect::<Vec<_>>();

        let merged_nfa = merge_nfas(&nfas);
        let dfa = nfa_to_dfa(&merged_nfa, &mut charset_table).unwrap();

        assert!(dfa.edges.len() < 62);
    }

    #[test]
    fn dfa_scan_tokenizes_tiny_source() {
        let rules = parse_rules(
            r#"
letter -> 'a'~'z' | 'A'~'Z'
digit -> '0'~'9'
READ -> 'r''e''a''d'
IF -> 'i''f'
THEN -> 't''h''e''n'
END -> 'e''n''d'
ID -> letter (letter | digit)*
NUM -> digit+
ASSIGN -> ':''='
SEMI -> ';'
LT -> '<'
"#,
        )
        .unwrap();
        let mut program = build_lexer_program(&rules).unwrap();
        let dfa = nfa_to_dfa(&program.nfa, &mut program.charset_table).unwrap();
        let tokens = dfa_scan(
            &dfa,
            &program.charset_table,
            "read x; { comment }\nif 0 < x then\n  x := 1\nend",
        )
        .unwrap();

        let pairs = tokens
            .iter()
            .map(|token| (token.category.as_str(), token.lexeme.as_str()))
            .collect::<Vec<_>>();

        assert_eq!(
            pairs,
            vec![
                ("READ", "read"),
                ("ID", "x"),
                ("SEMI", ";"),
                ("IF", "if"),
                ("NUM", "0"),
                ("LT", "<"),
                ("ID", "x"),
                ("THEN", "then"),
                ("ID", "x"),
                ("ASSIGN", ":="),
                ("NUM", "1"),
                ("END", "end"),
            ]
        );
    }
}
