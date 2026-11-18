//! AST -> `Diagram` instantiation and lane assignment.
//!
//! Port of the reference `DiagramTreeBuilder` (plus the `ScreenNodeBuilder`
//! orchestration: it finishes by running [`super::grid::run`]). The arena in
//! [`super::model`] replaces the reference's process-global `Element.get()`
//! registries with owned ids.

use crate::ast::{self, EdgeType, ExtensionKind, Stmt};
use crate::lexer::unquote;

use super::BuildError;
use super::grid;
use super::link::link_from_attrs;
use super::model::{
    AttrMap, Diagram, DiagramId, Direction, Edge, Lane, Legend, LegendEntry, LegendPosition,
    NodeId, Numbering, Orientation,
};

pub(super) fn build_diagram(
    id: DiagramId,
    ast_diagram: &ast::Diagram,
) -> Result<(Diagram, Vec<String>), BuildError> {
    let mut diagram = Diagram::new(id);
    instantiate(&mut diagram, &ast_diagram.stmts, None)?;

    // autolane plugin (if enabled) runs before automatic lane assignment.
    let autolane = diagram
        .plugins
        .iter()
        .any(|(name, _)| name == "autolane" || name.ends_with(".autolane"));
    if autolane {
        crate::plugins::autolane::apply(&mut diagram);
    }

    assign_lanes(&mut diagram);
    grid::run(&mut diagram);
    let warnings = super::numbering::assign(&mut diagram);
    Ok((diagram, warnings))
}

fn instantiate(
    diagram: &mut Diagram,
    stmts: &[Stmt],
    lane: Option<usize>,
) -> Result<(), BuildError> {
    for stmt in stmts {
        match stmt {
            Stmt::Node(node_stmt) => {
                let id = intern(diagram, &node_stmt.id);
                apply_node_attrs(diagram, id, &node_stmt.attrs)?;
                belong_to(diagram, id, lane)?;
            }
            Stmt::Edge(edge_stmt) => {
                let from: Vec<NodeId> = edge_stmt
                    .from_nodes
                    .iter()
                    .map(|raw| intern(diagram, raw))
                    .collect();
                let to: Vec<NodeId> = edge_stmt
                    .to_nodes
                    .iter()
                    .map(|raw| intern(diagram, raw))
                    .collect();

                for &id in from.iter().chain(to.iter()) {
                    belong_to(diagram, id, lane)?;
                }

                for &a in &from {
                    for &b in &to {
                        let idx = find_or_create_edge(diagram, a, b);
                        diagram.edges[idx].dir = convert_direction(edge_stmt.edge_type);
                        apply_edge_attrs(diagram, idx, &edge_stmt.attrs)?;
                    }
                }
            }
            Stmt::Lane(lane_stmt) => {
                let lane_id = lane_stmt.id.as_deref().map(unquote);
                let lane_idx = get_or_create_lane(diagram, lane_id);
                instantiate(diagram, &lane_stmt.stmts, Some(lane_idx))?;
            }
            Stmt::Attr(attr) => {
                let name = unquote(&attr.name);
                let value = attr.value.as_deref().map(unquote);
                match lane {
                    Some(lane_idx) => apply_lane_attr(diagram, lane_idx, &name, value),
                    None => apply_diagram_attr(diagram, &name, value)?,
                }
            }
            Stmt::Extension(extension) => match extension.kind {
                ExtensionKind::Class => {
                    diagram
                        .classes
                        .insert(unquote(&extension.name), attr_map(&extension.attrs));
                }
                ExtensionKind::Plugin => {
                    diagram
                        .plugins
                        .push((unquote(&extension.name), attr_map(&extension.attrs)));
                }
            },
            Stmt::Legend(legend) => build_legend(diagram, legend),
        }
    }
    Ok(())
}

fn intern(diagram: &mut Diagram, raw_name: &str) -> NodeId {
    let name = unquote(raw_name);
    diagram.intern(&name)
}

fn find_or_create_edge(diagram: &mut Diagram, from: NodeId, to: NodeId) -> usize {
    if let Some(idx) = diagram
        .edges
        .iter()
        .position(|e| e.from == from && e.to == to)
    {
        return idx;
    }
    diagram.edges.push(Edge {
        from,
        to,
        dir: Direction::Forward,
        label: None,
        color: None,
        style: None,
        attrs: AttrMap::new(),
    });
    diagram.edges.len() - 1
}

fn get_or_create_lane(diagram: &mut Diagram, id: Option<String>) -> usize {
    // Named lanes are shared; anonymous lanes are always distinct.
    if let Some(name) = &id
        && let Some(idx) = diagram
            .lanes
            .iter()
            .position(|lane| lane.id.as_deref() == Some(name.as_str()))
    {
        return idx;
    }
    diagram.lanes.push(Lane::new(id));
    diagram.lanes.len() - 1
}

fn belong_to(diagram: &mut Diagram, id: NodeId, lane: Option<usize>) -> Result<(), BuildError> {
    if let Some(lane_idx) = lane {
        if let Some(existing) = diagram.node(id).lane
            && existing != lane_idx
        {
            return Err(BuildError::Message(format!(
                "node {:?} could not belong to two lanes",
                diagram.node(id).name
            )));
        }
        diagram.node_mut(id).lane = Some(lane_idx);
        if !diagram.lanes[lane_idx].nodes.contains(&id) {
            diagram.lanes[lane_idx].nodes.push(id);
        }
    }
    Ok(())
}

fn convert_direction(edge_type: EdgeType) -> Direction {
    match edge_type {
        EdgeType::Arrow => Direction::Forward,
        EdgeType::ReverseArrow => Direction::Reverse,
        EdgeType::Line => Direction::Undirected,
        EdgeType::DoubleArrow => Direction::Bidirectional,
    }
}

fn attr_map(attrs: &[ast::Attr]) -> AttrMap {
    attrs
        .iter()
        .map(|attr| (unquote(&attr.name), attr.value.as_deref().map(unquote)))
        .collect()
}

// ---------------------------------------------------------------------------
// Attribute application
// ---------------------------------------------------------------------------

fn apply_node_attrs(
    diagram: &mut Diagram,
    id: NodeId,
    attrs: &[ast::Attr],
) -> Result<(), BuildError> {
    for attr in attrs {
        let name = unquote(&attr.name);
        let value = attr.value.as_deref().map(unquote);
        diagram
            .node_mut(id)
            .attrs
            .insert(name.clone(), value.clone());
        apply_node_attr(diagram, id, &name, value)?;
    }

    // `ref`/`href` are resolved from the node's full attribute set.
    let link = link_from_attrs(&diagram.node(id).attrs).map_err(BuildError::Message)?;
    diagram.node_mut(id).link = link;
    Ok(())
}

fn apply_node_attr(
    diagram: &mut Diagram,
    id: NodeId,
    name: &str,
    value: Option<String>,
) -> Result<(), BuildError> {
    match name {
        "class" => {
            if let Some(class_name) = value {
                let class_attrs =
                    diagram.classes.get(&class_name).cloned().ok_or_else(|| {
                        BuildError::Message(format!("unknown class: {class_name}"))
                    })?;
                diagram.node_mut(id).class = Some(class_name);
                for (attr_name, attr_value) in class_attrs {
                    apply_node_attr(diagram, id, &attr_name, attr_value)?;
                }
            }
        }
        "label" => {
            if let Some(value) = value {
                diagram.node_mut(id).label = value;
            }
        }
        "numbered" | "number" => {
            if let Some(value) = value {
                if value.eq_ignore_ascii_case("none") {
                    diagram.node_mut(id).number = None;
                    diagram.node_mut(id).number_disabled = true;
                } else {
                    let number = value.parse::<u32>().map_err(|_| {
                        BuildError::Message(format!("invalid sequence number: {value:?}"))
                    })?;
                    diagram.node_mut(id).number = Some(number);
                    diagram.node_mut(id).number_disabled = false;
                }
            }
        }
        "description" => {
            diagram.node_mut(id).description = value;
        }
        // Integer geometry attributes (`Element.int_attrs`). `colwidth`/
        // `colheight` size the node on the layout grid; `width`/`height` set
        // its pixel size (used by `metrics.rs`).
        "colwidth" => {
            diagram.node_mut(id).grid.colwidth = parse_int_attr(name, value.as_deref())?;
        }
        "colheight" => {
            diagram.node_mut(id).grid.colheight = parse_int_attr(name, value.as_deref())?;
        }
        "width" => diagram.node_mut(id).width = Some(parse_int_attr(name, value.as_deref())?),
        "height" => diagram.node_mut(id).height = Some(parse_int_attr(name, value.as_deref())?),
        "shape" => diagram.node_mut(id).shape = value,
        "color" => diagram.node_mut(id).color = value,
        "textcolor" => diagram.node_mut(id).textcolor = value,
        "linecolor" => diagram.node_mut(id).linecolor = value,
        _ => {}
    }
    Ok(())
}

fn apply_edge_attrs(
    diagram: &mut Diagram,
    idx: usize,
    attrs: &[ast::Attr],
) -> Result<(), BuildError> {
    for attr in attrs {
        let name = unquote(&attr.name);
        let value = attr.value.as_deref().map(unquote);
        diagram.edges[idx].attrs.insert(name.clone(), value.clone());
        match name.as_str() {
            "label" => diagram.edges[idx].label = value,
            "color" => diagram.edges[idx].color = value,
            "style" => diagram.edges[idx].style = value,
            _ => {}
        }
    }
    Ok(())
}

fn apply_lane_attr(diagram: &mut Diagram, idx: usize, name: &str, value: Option<String>) {
    diagram.lanes[idx]
        .attrs
        .insert(name.to_string(), value.clone());
    match name {
        "label" => diagram.lanes[idx].label = value,
        "color" => diagram.lanes[idx].color = value,
        "href" => diagram.lanes[idx].href = value,
        _ => {}
    }
}

fn apply_diagram_attr(
    diagram: &mut Diagram,
    name: &str,
    value: Option<String>,
) -> Result<(), BuildError> {
    diagram.attrs.insert(name.to_string(), value.clone());
    match name {
        "orientation" => {
            diagram.orientation = match value.as_deref() {
                Some("portrait") => Orientation::Portrait,
                Some("landscape") => Orientation::Landscape,
                other => {
                    return Err(BuildError::Message(format!(
                        "unknown diagram orientation: {other:?}"
                    )));
                }
            };
        }
        "numbering" => {
            diagram.numbering = match value.as_deref() {
                Some("off") | None => Numbering::Off,
                Some("auto") => Numbering::Auto,
                Some("topological") => Numbering::Topological,
                Some("depth") => Numbering::Depth,
                other => {
                    return Err(BuildError::Message(format!(
                        "unknown numbering mode: {other:?}"
                    )));
                }
            };
        }
        // Pixel-metrics defaults (`Diagram.int_attrs`).
        "node_width" => diagram.node_width = Some(parse_int_attr(name, value.as_deref())?),
        "node_height" => diagram.node_height = Some(parse_int_attr(name, value.as_deref())?),
        "span_width" => diagram.span_width = Some(parse_int_attr(name, value.as_deref())?),
        "span_height" => diagram.span_height = Some(parse_int_attr(name, value.as_deref())?),
        _ => {}
    }
    Ok(())
}

fn parse_int_attr(name: &str, value: Option<&str>) -> Result<i32, BuildError> {
    value
        .and_then(|raw| raw.trim().parse::<i32>().ok())
        .ok_or_else(|| BuildError::Message(format!("invalid integer for {name}: {value:?}")))
}

fn build_legend(diagram: &mut Diagram, legend: &ast::LegendStmt) {
    let attrs = attr_map(&legend.attrs);
    let title = attrs.get("title").cloned().flatten();
    let position = match attrs.get("position").and_then(|v| v.as_deref()) {
        Some("top") => LegendPosition::Top,
        Some("left") => LegendPosition::Left,
        Some("right") => LegendPosition::Right,
        _ => LegendPosition::Bottom,
    };

    let entries = legend
        .items
        .iter()
        .map(|item| match item {
            ast::LegendItem::Class { name, attrs } => {
                let map = attr_map(attrs);
                LegendEntry::Class {
                    name: unquote(name),
                    label: map.get("label").cloned().flatten(),
                    attrs: map,
                }
            }
            ast::LegendItem::Entry { label, attrs } => LegendEntry::Free {
                label: unquote(label),
                attrs: attr_map(attrs),
            },
            ast::LegendItem::Numbered => LegendEntry::SequenceKey,
        })
        .collect();

    diagram.legend = Some(Legend {
        title,
        position,
        entries,
    });
}

// ---------------------------------------------------------------------------
// Lane assignment (port of DiagramTreeBuilder.build's second half)
// ---------------------------------------------------------------------------

fn assign_lanes(diagram: &mut Diagram) {
    if diagram.lanes.is_empty() {
        diagram.lanes.push(Lane::new(None));
    }

    let node_count = diagram.nodes.len();
    for i in 0..node_count {
        let id = NodeId(i as u32);
        if diagram.node(id).lane.is_some() {
            continue;
        }

        // Incoming edges ("parents"); cheap scan is fine for diagram sizes.
        let mut parents: Vec<(usize, usize)> = Vec::new();
        for edge in &diagram.edges {
            if edge.to == id
                && let Some(lane) = diagram.node(edge.from).lane
            {
                parents.push((edge.from.index(), lane));
            }
        }
        parents.sort_by_key(|(order, _)| *order);

        let lane = match parents.first() {
            Some(&(_, lane)) => lane,
            None => 0,
        };
        diagram.node_mut(id).lane = Some(lane);
        if !diagram.lanes[lane].nodes.contains(&id) {
            diagram.lanes[lane].nodes.push(id);
        }
    }

    remove_empty_lanes(diagram);
}

fn remove_empty_lanes(diagram: &mut Diagram) {
    if diagram.lanes.iter().all(|lane| !lane.nodes.is_empty()) {
        return;
    }

    let lanes = std::mem::take(&mut diagram.lanes);
    let mut remap: Vec<Option<usize>> = vec![None; lanes.len()];
    let mut kept = Vec::new();
    for (old, lane) in lanes.into_iter().enumerate() {
        if !lane.nodes.is_empty() {
            remap[old] = Some(kept.len());
            kept.push(lane);
        }
    }
    diagram.lanes = kept;

    for node in &mut diagram.nodes {
        if let Some(old) = node.lane {
            node.lane = remap[old];
        }
    }
}
