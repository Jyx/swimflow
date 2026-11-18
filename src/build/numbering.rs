//! Sequence-number assignment.
//!
//! Rules:
//!
//! * `off` — only explicit `numbered = N` survive.
//! * `auto` — assign the next free integer in definition order.
//! * `topological` — Kahn's algorithm over the directed edges. Cycle-closing
//!   (back) edges are dropped first, so a feedback loop can never collapse the
//!   whole diagram to definition order.
//! * `depth` — longest-path distance from a start node (again over the
//!   cycle-free edges); nodes that can run in parallel share a stage, so their
//!   numbers stay adjacent.
//!
//! Explicit numbers always win; a duplicate explicit number keeps the first
//! occurrence and produces a warning. `numbered = none` opts a node out.
//! Assignment is deterministic: stable ordering, no hashing.

use std::collections::BTreeSet;

use super::model::{Diagram, Numbering};

/// Assign sequence numbers to `diagram`'s nodes, returning any warnings.
pub fn assign(diagram: &mut Diagram) -> Vec<String> {
    let mut warnings = Vec::new();
    let mut used: BTreeSet<u32> = BTreeSet::new();

    // Explicit numbers win; the first occurrence reserves the value.
    for i in 0..diagram.nodes.len() {
        let Some(number) = diagram.nodes[i].number else {
            continue;
        };
        if !used.insert(number) {
            warnings.push(format!(
                "duplicate sequence number {number} on node {:?}; keeping the first",
                diagram.nodes[i].name
            ));
            diagram.nodes[i].number = None;
        }
    }

    match diagram.numbering {
        Numbering::Off => {}
        Numbering::Auto => {
            let order: Vec<usize> = (0..diagram.nodes.len()).collect();
            assign_in_order(diagram, &order, &mut used);
        }
        Numbering::Topological => {
            let order = topological_order(diagram);
            assign_in_order(diagram, &order, &mut used);
        }
        Numbering::Depth => {
            let order = depth_order(diagram);
            assign_in_order(diagram, &order, &mut used);
        }
    }

    warnings
}

fn assign_in_order(diagram: &mut Diagram, order: &[usize], used: &mut BTreeSet<u32>) {
    let mut next = 1;
    for &index in order {
        let node = &mut diagram.nodes[index];
        if node.number.is_some() || node.number_disabled {
            continue;
        }
        while used.contains(&next) {
            next += 1;
        }
        node.number = Some(next);
        used.insert(next);
        next += 1;
    }
}

/// Kahn's algorithm with definition-order tie-breaking, run on the graph with
/// cycle-closing back edges removed.
fn topological_order(diagram: &Diagram) -> Vec<usize> {
    let successors = successors(diagram);
    let back = back_edges(&successors);
    kahn(&successors, &back)
}

/// Number by longest-path distance from the start, then by definition order.
///
/// Back edges are ignored when measuring depth, so a feedback loop cannot make
/// the distance unbounded. Parallel branches land in the same stage and get
/// consecutive numbers.
fn depth_order(diagram: &Diagram) -> Vec<usize> {
    let successors = successors(diagram);
    let back = back_edges(&successors);
    let order = kahn(&successors, &back);

    let count = order.len();
    let mut depth = vec![0usize; count];
    for &node in &order {
        for &next in &successors[node] {
            if !back[node].contains(&next) && depth[node] + 1 > depth[next] {
                depth[next] = depth[node] + 1;
            }
        }
    }

    let mut numbered: Vec<usize> = (0..count).collect();
    numbered.sort_by_key(|&node| (depth[node], node));
    numbered
}

/// Successor lists in definition order; self-loops do not constrain numbering.
fn successors(diagram: &Diagram) -> Vec<Vec<usize>> {
    let mut successors: Vec<Vec<usize>> = vec![Vec::new(); diagram.nodes.len()];
    for edge in &diagram.edges {
        let from = edge.from.index();
        let to = edge.to.index();
        if from != to {
            successors[from].push(to);
        }
    }
    successors
}

/// Kahn's algorithm with definition-order tie-breaking. `back` lists the edges
/// to ignore (they close a cycle).
fn kahn(successors: &[Vec<usize>], back: &[BTreeSet<usize>]) -> Vec<usize> {
    let count = successors.len();
    let mut indegree = vec![0usize; count];
    for (from, nexts) in successors.iter().enumerate() {
        for &to in nexts {
            if !back[from].contains(&to) {
                indegree[to] += 1;
            }
        }
    }

    let mut available: BTreeSet<usize> = (0..count).filter(|&i| indegree[i] == 0).collect();
    let mut order = Vec::with_capacity(count);
    while let Some(&index) = available.iter().next() {
        available.remove(&index);
        order.push(index);
        for &next in &successors[index] {
            if back[index].contains(&next) {
                continue;
            }
            indegree[next] -= 1;
            if indegree[next] == 0 {
                available.insert(next);
            }
        }
    }
    order
}

/// Find the edges that close a cycle with an iterative depth-first search.
///
/// A back edge points at a node still on the DFS stack. Removing every back
/// edge leaves a DAG; unlike dropping cycles after Kahn has already stalled,
/// this keeps the acyclic part reachable from its real start node.
fn back_edges(successors: &[Vec<usize>]) -> Vec<BTreeSet<usize>> {
    let count = successors.len();
    let mut back: Vec<BTreeSet<usize>> = vec![BTreeSet::new(); count];
    // 0 = unvisited, 1 = on the DFS stack, 2 = finished.
    let mut state = vec![0u8; count];

    for root in 0..count {
        if state[root] != 0 {
            continue;
        }
        state[root] = 1;
        let mut stack = vec![(root, 0usize)];
        while let Some(&(node, next)) = stack.last() {
            if next < successors[node].len() {
                stack.last_mut().expect("stack is non-empty").1 += 1;
                let child = successors[node][next];
                match state[child] {
                    0 => {
                        state[child] = 1;
                        stack.push((child, 0));
                    }
                    1 => {
                        back[node].insert(child);
                    }
                    _ => {}
                }
            } else {
                state[node] = 2;
                stack.pop();
            }
        }
    }
    back
}

#[cfg(test)]
mod tests {
    use crate::build;

    fn numbers(src: &str) -> Vec<(String, Option<u32>)> {
        let set = build::build_str(src).unwrap();
        let diagram = &set.root().diagrams[0];
        diagram
            .nodes
            .iter()
            .map(|node| (node.name.clone(), node.number))
            .collect()
    }

    #[test]
    fn off_keeps_only_explicit() {
        let result = numbers("{ A [numbered = 2]; B; C [numbered = 1]; }");
        assert_eq!(
            result,
            vec![
                ("A".to_string(), Some(2)),
                ("B".to_string(), None),
                ("C".to_string(), Some(1)),
            ]
        );
    }

    #[test]
    fn auto_assigns_in_definition_order() {
        let result = numbers("{ numbering = auto; B; A; C; }");
        assert_eq!(
            result,
            vec![
                ("B".to_string(), Some(1)),
                ("A".to_string(), Some(2)),
                ("C".to_string(), Some(3)),
            ]
        );
    }

    #[test]
    fn explicit_reserves_and_none_opts_out() {
        let result = numbers("{ numbering = auto; A [numbered = 2]; B; C [numbered = none]; }");
        assert_eq!(
            result,
            vec![
                ("A".to_string(), Some(2)),
                ("B".to_string(), Some(1)),
                ("C".to_string(), None),
            ]
        );
    }

    #[test]
    fn topological_follows_edges() {
        let result = numbers("{ numbering = topological; C -> A; A -> B; }");
        // C has no incoming edge, then A, then B.
        assert_eq!(
            result,
            vec![
                ("C".to_string(), Some(1)),
                ("A".to_string(), Some(2)),
                ("B".to_string(), Some(3)),
            ]
        );
    }

    #[test]
    fn topological_cycle_with_a_source_keeps_the_flow() {
        let result = numbers("{ numbering = topological; A -> B; B -> A; C; }");
        // A and C are both starts; definition order puts A first, and the
        // feedback loop no longer drags the cycle behind C.
        assert_eq!(
            result,
            vec![
                ("A".to_string(), Some(1)),
                ("B".to_string(), Some(2)),
                ("C".to_string(), Some(3)),
            ]
        );
    }

    #[test]
    fn topological_cycle_without_a_source_uses_edge_direction() {
        // Every node is in the A -> B -> C -> A cycle and the definition order
        // (A, C, B) disagrees with the edges.
        let result = numbers("{ numbering = topological; A; C; B; A -> B; B -> C; C -> A; }");
        assert_eq!(
            result,
            vec![
                ("A".to_string(), Some(1)),
                ("C".to_string(), Some(3)),
                ("B".to_string(), Some(2)),
            ]
        );
    }

    #[test]
    fn depth_groups_parallel_steps_into_stages() {
        let result = numbers("{ numbering = depth; A -> B; A -> C; B -> D; C -> D; }");
        // A starts (stage 0), B and C share stage 1, D merges at stage 2.
        assert_eq!(
            result,
            vec![
                ("A".to_string(), Some(1)),
                ("B".to_string(), Some(2)),
                ("C".to_string(), Some(3)),
                ("D".to_string(), Some(4)),
            ]
        );
    }

    #[test]
    fn depth_ignores_cycle_closing_edges() {
        let result = numbers("{ numbering = depth; A -> B; B -> C; C -> A; }");
        // The feedback C -> A is dropped, so depths are A=0, B=1, C=2.
        assert_eq!(
            result,
            vec![
                ("A".to_string(), Some(1)),
                ("B".to_string(), Some(2)),
                ("C".to_string(), Some(3)),
            ]
        );
    }

    #[test]
    fn duplicate_explicit_number_warns_and_keeps_first() {
        let set =
            build::build_str("{ numbering = auto; A [numbered = 1]; B [numbered = 1]; }").unwrap();
        assert_eq!(set.root().warnings.len(), 1);
        assert!(set.root().warnings[0].contains("duplicate sequence number 1"));
        let diagram = &set.root().diagrams[0];
        assert_eq!(diagram.nodes[0].number, Some(1));
        assert_eq!(diagram.nodes[1].number, Some(2));
    }
}
