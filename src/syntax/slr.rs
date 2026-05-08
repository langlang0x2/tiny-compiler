use std::collections::HashMap;

use super::{Dfa, Grammar, SymbolType};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionCategory {
    Reduce,
    Shift,
    Accept,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActionCell {
    pub state_id: usize,
    pub terminal_symbol_name: String,
    pub action_type: ActionCategory,
    pub id: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GotoCell {
    pub state_id: usize,
    pub non_terminal_symbol_name: String,
    pub next_state_id: usize,
}

#[derive(Clone, Debug)]
pub struct ParseTable {
    pub action_cells: Vec<ActionCell>,
    pub goto_cells: Vec<GotoCell>,
    pub conflicts: Vec<String>,
}

pub fn build_slr_parse_table(grammar: &Grammar, dfa: &Dfa) -> ParseTable {
    let mut action_map: HashMap<(usize, String), ActionCell> = HashMap::new();
    let mut goto_map: HashMap<(usize, String), GotoCell> = HashMap::new();
    let mut conflicts = Vec::new();

    for edge in &dfa.edges {
        let symbol = &grammar.symbols[edge.driver_symbol];
        match symbol.symbol_type {
            SymbolType::Terminal => {
                let cell = ActionCell {
                    state_id: edge.from_item_set,
                    terminal_symbol_name: symbol.name.clone(),
                    action_type: ActionCategory::Shift,
                    id: edge.to_item_set,
                };
                insert_action(&mut action_map, cell, &mut conflicts);
            }
            SymbolType::NonTerminal => {
                let cell = GotoCell {
                    state_id: edge.from_item_set,
                    non_terminal_symbol_name: symbol.name.clone(),
                    next_state_id: edge.to_item_set,
                };
                insert_goto(&mut goto_map, cell, &mut conflicts);
            }
            SymbolType::Null => {}
        }
    }

    for item_set in &dfa.item_sets {
        for item in &item_set.items {
            if item.dot_position != grammar.item_body_len(item.production) {
                continue;
            }

            let production = &grammar.productions[item.production];
            if production.head == grammar.start_non_terminal {
                let accept = ActionCell {
                    state_id: item_set.state_id,
                    terminal_symbol_name: grammar.terminal_name(grammar.eof_terminal).to_string(),
                    action_type: ActionCategory::Accept,
                    id: 0,
                };
                insert_action(&mut action_map, accept, &mut conflicts);
                continue;
            }

            let follow_set = &grammar.non_terminals[production.head].follow_set;
            for terminal in follow_set {
                if *terminal == grammar.epsilon_terminal {
                    continue;
                }
                let reduce = ActionCell {
                    state_id: item_set.state_id,
                    terminal_symbol_name: grammar.terminal_name(*terminal).to_string(),
                    action_type: ActionCategory::Reduce,
                    id: production.production_id,
                };
                insert_action(&mut action_map, reduce, &mut conflicts);
            }
        }
    }

    let mut action_cells = action_map.into_values().collect::<Vec<_>>();
    action_cells.sort_by(|a, b| {
        (a.state_id, a.terminal_symbol_name.as_str()).cmp(&(b.state_id, b.terminal_symbol_name.as_str()))
    });
    let mut goto_cells = goto_map.into_values().collect::<Vec<_>>();
    goto_cells.sort_by(|a, b| {
        (a.state_id, a.non_terminal_symbol_name.as_str()).cmp(&(b.state_id, b.non_terminal_symbol_name.as_str()))
    });

    ParseTable {
        action_cells,
        goto_cells,
        conflicts,
    }
}

pub fn is_slr1(parse_table: &ParseTable) -> bool {
    parse_table.conflicts.is_empty()
}

fn insert_action(
    action_map: &mut HashMap<(usize, String), ActionCell>,
    cell: ActionCell,
    conflicts: &mut Vec<String>,
) {
    let key = (cell.state_id, cell.terminal_symbol_name.clone());
    if let Some(existing) = action_map.get(&key) {
        if existing != &cell {
            conflicts.push(format!(
                "ACTION conflict at state {}, symbol {}: {:?}{} vs {:?}{}",
                cell.state_id,
                cell.terminal_symbol_name,
                existing.action_type,
                existing.id,
                cell.action_type,
                cell.id
            ));
        }
    } else {
        action_map.insert(key, cell);
    }
}

fn insert_goto(
    goto_map: &mut HashMap<(usize, String), GotoCell>,
    cell: GotoCell,
    conflicts: &mut Vec<String>,
) {
    let key = (cell.state_id, cell.non_terminal_symbol_name.clone());
    if let Some(existing) = goto_map.get(&key) {
        if existing != &cell {
            conflicts.push(format!(
                "GOTO conflict at state {}, symbol {}: {} vs {}",
                cell.state_id,
                cell.non_terminal_symbol_name,
                existing.next_state_id,
                cell.next_state_id
            ));
        }
    } else {
        goto_map.insert(key, cell);
    }
}
