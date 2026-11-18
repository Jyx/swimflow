//! Port of the reference `plugins/autolane.py`.
//!
//! Nodes named `<lane>_<rest>` are placed into the matching named lane and
//! relabelled to `<rest>`. The reference implements this as a node-creation event
//! handler; here it is a deterministic post-pass run after all lanes exist.

use crate::build::model::{Diagram, NodeId};

pub fn apply(diagram: &mut Diagram) {
    let lanes: Vec<(usize, String)> = diagram
        .lanes
        .iter()
        .enumerate()
        .filter_map(|(idx, lane)| lane.id.clone().map(|id| (idx, id)))
        .collect();

    for i in 0..diagram.nodes.len() {
        let id = NodeId(i as u32);
        if diagram.node(id).lane.is_some() {
            continue;
        }

        let name = diagram.node(id).name.clone();
        for (lane_idx, lane_id) in &lanes {
            let prefix = format!("{lane_id}_");
            if let Some(rest) = name.strip_prefix(&prefix) {
                diagram.node_mut(id).label = rest.to_string();
                diagram.node_mut(id).lane = Some(*lane_idx);
                if !diagram.lanes[*lane_idx].nodes.contains(&id) {
                    diagram.lanes[*lane_idx].nodes.push(id);
                }
                break;
            }
        }
    }
}
