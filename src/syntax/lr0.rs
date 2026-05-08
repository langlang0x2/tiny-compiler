use std::collections::{BTreeMap, HashMap, HashSet};

use super::{Grammar, SymbolType};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct LR0Item {
    pub production: usize,
    pub dot_position: usize,
}

#[derive(Clone, Debug)]
pub struct ItemSet {
    pub state_id: usize,
    pub items: Vec<LR0Item>,
}

#[derive(Clone, Debug)]
pub struct TransitionEdge {
    pub driver_symbol: usize,
    pub from_item_set: usize,
    pub to_item_set: usize,
}

#[derive(Clone, Debug)]
pub struct Dfa {
    pub item_sets: Vec<ItemSet>,
    pub edges: Vec<TransitionEdge>,
}

fn get_closure(grammar: &Grammar, item_set: &mut ItemSet) {
    loop {
        let mut changed = false;
        let existing = item_set
            .items
            .iter()
            .map(|item| (item.production, item.dot_position))
            .collect::<HashSet<_>>();
        let mut additions = Vec::new();

        for item in &item_set.items {
            let production = &grammar.productions[item.production];
            if item.dot_position >= production.body.len() {
                continue;
            }
            let symbol_after_dot = production.body[item.dot_position];
            if grammar.symbol_type(symbol_after_dot) != SymbolType::NonTerminal {
                continue;
            }
            let non_terminal_id = grammar.symbols[symbol_after_dot].non_terminal_id.unwrap();
            for production_id in &grammar.non_terminals[non_terminal_id].production_ids {
                if !existing.contains(&(*production_id, 0))
                    && !additions.iter().any(|new_item: &LR0Item| {
                        new_item.production == *production_id && new_item.dot_position == 0
                    })
                {
                    additions.push(LR0Item {
                        production: *production_id,
                        dot_position: 0,
                    });
                    changed = true;
                }
            }
        }

        if !changed {
            break;
        }
        item_set.items.extend(additions);
    }

    item_set
        .items
        .sort_by_key(|item| (item.production, item.dot_position));
}

fn exhaust_transition(
    grammar: &Grammar,
    dfa: &mut Dfa,
    state_id: usize,
    canonical_map: &mut HashMap<Vec<(usize, usize)>, usize>,
) {
    let item_set = dfa.item_sets[state_id].clone();
    let mut grouped_items: BTreeMap<usize, Vec<LR0Item>> = BTreeMap::new();

    for item in &item_set.items {
        let production = &grammar.productions[item.production];
        if item.dot_position >= production.body.len() {
            continue;
        }
        let driver_symbol = production.body[item.dot_position];
        grouped_items
            .entry(driver_symbol)
            .or_default()
            .push(LR0Item {
                production: item.production,
                dot_position: item.dot_position + 1,
            });
    }

    for (driver_symbol, mut core_items) in grouped_items {
        core_items.sort_by_key(|item| (item.production, item.dot_position));
        core_items.dedup_by_key(|item| (item.production, item.dot_position));

        let mut next_item_set = ItemSet {
            state_id: 0,
            items: core_items,
        };
        get_closure(grammar, &mut next_item_set);

        let key = canonical_key(&next_item_set);
        let to_state_id = if let Some(existing_state_id) = canonical_map.get(&key) {
            *existing_state_id
        } else {
            let new_state_id = dfa.item_sets.len();
            next_item_set.state_id = new_state_id;
            dfa.item_sets.push(next_item_set);
            canonical_map.insert(key, new_state_id);
            new_state_id
        };

        dfa.edges.push(TransitionEdge {
            driver_symbol,
            from_item_set: state_id,
            to_item_set: to_state_id,
        });
    }
}

pub fn build_lr0_dfa(grammar: &Grammar) -> Dfa {
    let start_production = grammar.non_terminals[grammar.start_non_terminal].production_ids[0];
    let mut start_set = ItemSet {
        state_id: 0,
        items: vec![LR0Item {
            production: start_production,
            dot_position: 0,
        }],
    };
    get_closure(grammar, &mut start_set);

    let mut dfa = Dfa {
        item_sets: vec![start_set],
        edges: Vec::new(),
    };
    let mut canonical_map = HashMap::new();
    canonical_map.insert(canonical_key(&dfa.item_sets[0]), 0);

    let mut cursor = 0;
    while cursor < dfa.item_sets.len() {
        exhaust_transition(grammar, &mut dfa, cursor, &mut canonical_map);
        cursor += 1;
    }

    dfa
}

fn canonical_key(item_set: &ItemSet) -> Vec<(usize, usize)> {
    let mut key = item_set
        .items
        .iter()
        .map(|item| (item.production, item.dot_position))
        .collect::<Vec<_>>();
    key.sort_unstable();
    key
}
