#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};

pub const CHARSET_ID_BASE: usize = 1_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DriverType {
    Null,
    Char,
    Charset,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateType {
    Match,
    Unmatch,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharSet {
    pub index_id: usize,
    pub segment_id: usize,
    pub from_char: char,
    pub to_char: char,
}

#[derive(Debug, Clone, Default)]
pub struct CharSetTable {
    rows: Vec<CharSet>,
    next_id: usize,
}

impl CharSetTable {
    pub fn new() -> Self {
        Self {
            rows: Vec::new(),
            next_id: CHARSET_ID_BASE,
        }
    }

    pub fn rows(&self) -> &[CharSet] {
        &self.rows
    }

    pub fn range(&mut self, from_char: char, to_char: char) -> Result<usize, String> {
        if from_char > to_char {
            return Err(format!("invalid range: '{from_char}'~'{to_char}'"));
        }
        Ok(self.insert_segments(vec![(from_char, to_char)]))
    }

    pub fn union_chars(&mut self, c1: char, c2: char) -> usize {
        self.insert_segments(vec![(c1, c1), (c2, c2)])
    }

    pub fn union_charset_char(&mut self, charset_id: usize, c: char) -> Result<usize, String> {
        let mut segments = self.segments(charset_id)?;
        segments.push((c, c));
        Ok(self.insert_segments(segments))
    }

    pub fn union_charsets(
        &mut self,
        charset_id1: usize,
        charset_id2: usize,
    ) -> Result<usize, String> {
        let mut segments = self.segments(charset_id1)?;
        segments.extend(self.segments(charset_id2)?);
        Ok(self.insert_segments(segments))
    }

    pub fn difference_charset_char(
        &mut self,
        charset_id: usize,
        c: char,
    ) -> Result<usize, String> {
        let mut result = Vec::new();

        for (from, to) in self.segments(charset_id)? {
            if c < from || c > to {
                result.push((from, to));
                continue;
            }
            if from < c {
                result.push((from, char::from_u32(c as u32 - 1).unwrap()));
            }
            if c < to {
                result.push((char::from_u32(c as u32 + 1).unwrap(), to));
            }
        }

        Ok(self.insert_segments(result))
    }

    pub fn segments(&self, charset_id: usize) -> Result<Vec<(char, char)>, String> {
        let mut segments: Vec<(char, char)> = self
            .rows
            .iter()
            .filter(|row| row.index_id == charset_id)
            .map(|row| (row.from_char, row.to_char))
            .collect();

        if segments.is_empty() {
            return Err(format!("unknown charset id: {charset_id}"));
        }

        segments.sort_unstable();
        Ok(segments)
    }

    pub fn contains(&self, charset_id: usize, ch: char) -> Result<bool, String> {
        Ok(self
            .segments(charset_id)?
            .into_iter()
            .any(|(from, to)| from <= ch && ch <= to))
    }

    pub fn from_chars(&mut self, chars: &BTreeSet<char>) -> usize {
        let segments = chars.iter().copied().map(|ch| (ch, ch)).collect();
        self.insert_segments(segments)
    }

    fn insert_segments(&mut self, segments: Vec<(char, char)>) -> usize {
        let segments = normalize_segments(segments);
        let index_id = self.next_id;
        self.next_id += 1;

        for (segment_index, (from_char, to_char)) in segments.into_iter().enumerate() {
            self.rows.push(CharSet {
                index_id,
                segment_id: segment_index + 1,
                from_char,
                to_char,
            });
        }

        index_id
    }
}

fn normalize_segments(mut segments: Vec<(char, char)>) -> Vec<(char, char)> {
    if segments.is_empty() {
        return Vec::new();
    }

    segments.sort_unstable();
    let mut merged = Vec::new();

    for (from, to) in segments {
        if let Some((last_from, last_to)) = merged.last_mut() {
            let next = (*last_to as u32).saturating_add(1);
            if (from as u32) <= next {
                if to > *last_to {
                    *last_to = to;
                }
                let _ = last_from;
                continue;
            }
        }
        merged.push((from, to));
    }

    merged
}

#[derive(Debug, Clone)]
pub struct Edge {
    pub from_state: usize,
    pub to_state: usize,
    pub driver_type: DriverType,
    pub driver_id: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct State {
    pub id: usize,
    pub state_type: StateType,
    pub category: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct Graph {
    pub states: Vec<State>,
    pub edges: Vec<Edge>,
    pub start_state: usize,
    pub end_state: usize,
}

impl Graph {
    pub fn mark_accepting(&mut self, category: Option<String>) {
        if let Some(state) = self.states.iter_mut().find(|state| state.id == self.end_state) {
            state.state_type = StateType::Match;
            state.category = category;
        }
    }

    pub fn move_states(
        &self,
        states: &BTreeSet<usize>,
        driver_type: DriverType,
        driver_id: usize,
    ) -> BTreeSet<usize> {
        self.edges
            .iter()
            .filter(|edge| {
                states.contains(&edge.from_state)
                    && edge.driver_type == driver_type
                    && edge.driver_id == Some(driver_id)
            })
            .map(|edge| edge.to_state)
            .collect()
    }

    pub fn epsilon_closure(&self, states: &BTreeSet<usize>) -> BTreeSet<usize> {
        let mut closure = states.clone();
        let mut stack: Vec<usize> = states.iter().copied().collect();

        while let Some(state_id) = stack.pop() {
            for edge in self.edges.iter().filter(|edge| {
                edge.from_state == state_id && edge.driver_type == DriverType::Null
            }) {
                if closure.insert(edge.to_state) {
                    stack.push(edge.to_state);
                }
            }
        }

        closure
    }

    pub fn dtran(
        &self,
        states: &BTreeSet<usize>,
        driver_type: DriverType,
        driver_id: usize,
    ) -> BTreeSet<usize> {
        let moved = self.move_states(states, driver_type, driver_id);
        self.epsilon_closure(&moved)
    }

    pub fn alphabet(&self) -> Vec<(DriverType, usize)> {
        self.edges
            .iter()
            .filter_map(|edge| match (edge.driver_type, edge.driver_id) {
                (DriverType::Null, _) => None,
                (_, Some(driver_id)) => Some((edge.driver_type, driver_id)),
                _ => None,
            })
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    fn shifted(&self, offset: usize) -> Self {
        Self {
            states: self
                .states
                .iter()
                .map(|state| State {
                    id: state.id + offset,
                    state_type: state.state_type,
                    category: state.category.clone(),
                })
                .collect(),
            edges: self
                .edges
                .iter()
                .map(|edge| Edge {
                    from_state: edge.from_state + offset,
                    to_state: edge.to_state + offset,
                    driver_type: edge.driver_type,
                    driver_id: edge.driver_id,
                })
                .collect(),
            start_state: self.start_state + offset,
            end_state: self.end_state + offset,
        }
    }
}

pub fn generate_basic_nfa(driver_type: DriverType, driver_id: usize) -> Graph {
    Graph {
        states: vec![
            State {
                id: 0,
                state_type: StateType::Unmatch,
                category: None,
            },
            State {
                id: 1,
                state_type: StateType::Match,
                category: None,
            },
        ],
        edges: vec![Edge {
            from_state: 0,
            to_state: 1,
            driver_type,
            driver_id: Some(driver_id),
        }],
        start_state: 0,
        end_state: 1,
    }
}

pub fn union_nfa(left: &Graph, right: &Graph) -> Graph {
    let left = left.shifted(1);
    let right = right.shifted(left.states.len() + 1);
    let end_state = right.states.len() + left.states.len() + 1;

    let mut states = Vec::new();
    states.push(State {
        id: 0,
        state_type: StateType::Unmatch,
        category: None,
    });
    states.extend(left.states.clone());
    states.extend(right.states.clone());
    states.push(State {
        id: end_state,
        state_type: StateType::Match,
        category: None,
    });

    demote_match_states(&mut states, &[left.end_state, right.end_state]);

    let mut edges = Vec::new();
    edges.extend(left.edges.clone());
    edges.extend(right.edges.clone());
    edges.push(epsilon(0, left.start_state));
    edges.push(epsilon(0, right.start_state));
    edges.push(epsilon(left.end_state, end_state));
    edges.push(epsilon(right.end_state, end_state));

    Graph {
        states,
        edges,
        start_state: 0,
        end_state,
    }
}

pub fn product_nfa(left: &Graph, right: &Graph) -> Graph {
    let left = left.clone();
    let right = right.shifted(left.states.len() - 1);

    let mut states = left.states;
    if let Some(last) = states.last_mut() {
        last.state_type = StateType::Unmatch;
        last.category = None;
    }
    states.extend(right.states.into_iter().skip(1));

    let mut edges = left.edges;
    edges.extend(right.edges);

    Graph {
        start_state: left.start_state,
        end_state: right.end_state,
        states,
        edges,
    }
}

pub fn plus_closure_nfa(graph: &Graph) -> Graph {
    let inner = graph.shifted(1);
    let end_state = inner.states.len() + 1;

    let mut states = Vec::new();
    states.push(State {
        id: 0,
        state_type: StateType::Unmatch,
        category: None,
    });
    states.extend(inner.states.clone());
    states.push(State {
        id: end_state,
        state_type: StateType::Match,
        category: None,
    });

    demote_match_states(&mut states, &[inner.end_state]);

    let mut edges = inner.edges.clone();
    edges.push(epsilon(0, inner.start_state));
    edges.push(epsilon(inner.end_state, inner.start_state));
    edges.push(epsilon(inner.end_state, end_state));

    Graph {
        states,
        edges,
        start_state: 0,
        end_state,
    }
}

pub fn closure_nfa(graph: &Graph) -> Graph {
    let inner = graph.shifted(1);
    let end_state = inner.states.len() + 1;

    let mut states = Vec::new();
    states.push(State {
        id: 0,
        state_type: StateType::Unmatch,
        category: None,
    });
    states.extend(inner.states.clone());
    states.push(State {
        id: end_state,
        state_type: StateType::Match,
        category: None,
    });

    demote_match_states(&mut states, &[inner.end_state]);

    let mut edges = inner.edges.clone();
    edges.push(epsilon(0, inner.start_state));
    edges.push(epsilon(0, end_state));
    edges.push(epsilon(inner.end_state, inner.start_state));
    edges.push(epsilon(inner.end_state, end_state));

    Graph {
        states,
        edges,
        start_state: 0,
        end_state,
    }
}

pub fn zero_or_one_nfa(graph: &Graph) -> Graph {
    let inner = graph.shifted(1);
    let end_state = inner.states.len() + 1;

    let mut states = Vec::new();
    states.push(State {
        id: 0,
        state_type: StateType::Unmatch,
        category: None,
    });
    states.extend(inner.states.clone());
    states.push(State {
        id: end_state,
        state_type: StateType::Match,
        category: None,
    });

    demote_match_states(&mut states, &[inner.end_state]);

    let mut edges = inner.edges.clone();
    edges.push(epsilon(0, inner.start_state));
    edges.push(epsilon(0, end_state));
    edges.push(epsilon(inner.end_state, end_state));

    Graph {
        states,
        edges,
        start_state: 0,
        end_state,
    }
}

pub fn merge_nfas(graphs: &[Graph]) -> Graph {
    if graphs.is_empty() {
        return Graph::default();
    }

    let mut states = vec![State {
        id: 0,
        state_type: StateType::Unmatch,
        category: None,
    }];
    let mut edges = Vec::new();
    let mut next_offset = 1;

    for graph in graphs {
        let shifted = graph.shifted(next_offset);
        edges.push(epsilon(0, shifted.start_state));
        states.extend(shifted.states);
        edges.extend(shifted.edges);
        next_offset = states.len();
    }

    Graph {
        states,
        edges,
        start_state: 0,
        end_state: 0,
    }
}

fn epsilon(from_state: usize, to_state: usize) -> Edge {
    Edge {
        from_state,
        to_state,
        driver_type: DriverType::Null,
        driver_id: None,
    }
}

fn demote_match_states(states: &mut [State], ids: &[usize]) {
    let lookup: BTreeMap<usize, ()> = ids.iter().copied().map(|id| (id, ())).collect();
    for state in states {
        if lookup.contains_key(&state.id) {
            state.state_type = StateType::Unmatch;
            state.category = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        closure_nfa, generate_basic_nfa, merge_nfas, plus_closure_nfa, product_nfa, union_nfa,
        zero_or_one_nfa, CharSetTable, DriverType, StateType,
    };
    use std::collections::BTreeSet;

    #[test]
    fn charset_ops_create_expected_segments() {
        let mut table = CharSetTable::new();
        let letter = table.range('a', 'z').unwrap();
        let with_underscore = table.union_charset_char(letter, '_').unwrap();
        let removed = table.difference_charset_char(with_underscore, 'm').unwrap();

        assert_eq!(table.segments(letter).unwrap(), vec![('a', 'z')]);
        assert_eq!(table.segments(with_underscore).unwrap(), vec![('_', '_'), ('a', 'z')]);
        assert_eq!(
            table.segments(removed).unwrap(),
            vec![('_', '_'), ('a', 'l'), ('n', 'z')]
        );
    }

    #[test]
    fn thompson_basic_shapes_work() {
        let basic = generate_basic_nfa(DriverType::Char, 'a' as usize);
        assert_eq!(basic.states.len(), 2);
        assert_eq!(basic.edges.len(), 1);

        let plus = plus_closure_nfa(&basic);
        let star = closure_nfa(&basic);
        let option = zero_or_one_nfa(&basic);
        let concat = product_nfa(&basic, &basic);
        let union = union_nfa(&basic, &basic);

        assert_eq!(plus.start_state, 0);
        assert_eq!(star.start_state, 0);
        assert_eq!(option.start_state, 0);
        assert_eq!(concat.states.last().unwrap().state_type, StateType::Match);
        assert_eq!(union.states.last().unwrap().state_type, StateType::Match);
    }

    #[test]
    fn graph_supports_move_and_closure() {
        let basic = generate_basic_nfa(DriverType::Char, 'a' as usize);
        let star = closure_nfa(&basic);
        let start = BTreeSet::from([star.start_state]);

        let closure = star.epsilon_closure(&start);
        let moved = star.move_states(&closure, DriverType::Char, 'a' as usize);
        let dtran = star.dtran(&closure, DriverType::Char, 'a' as usize);

        assert!(closure.contains(&star.start_state));
        assert!(!moved.is_empty());
        assert!(dtran.contains(&star.end_state));
    }

    #[test]
    fn merge_nfas_creates_shared_start_state() {
        let left = generate_basic_nfa(DriverType::Char, 'a' as usize);
        let right = generate_basic_nfa(DriverType::Char, 'b' as usize);

        let merged = merge_nfas(&[left, right]);

        assert_eq!(merged.start_state, 0);
        assert_eq!(
            merged
                .edges
                .iter()
                .filter(|edge| edge.from_state == 0 && edge.driver_type == DriverType::Null)
                .count(),
            2
        );
    }
}
