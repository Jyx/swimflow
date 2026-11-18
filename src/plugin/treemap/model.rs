//! The resolved treemap model: names and values with the weights computed.
//!
//! The parser keeps the explicit values as written. The model fills in the
//! defaults: a leaf without a value has weight 1, a container without a value
//! has the sum of its children, and an explicit container value is kept as
//! written (it decides the container's own share among its siblings).

use std::collections::BTreeMap;

use super::ast::{self, Attr, attr_value};

/// A resolved node of a treemap.
#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    /// The source name.
    pub name: String,
    /// The text drawn instead of the name (`label` attribute).
    pub label: String,
    /// The resolved weight used by the layout.
    pub weight: f64,
    /// The attributes the renderer reads.
    pub attrs: Vec<Attr>,
    /// The children, in source order.
    pub children: Vec<Node>,
    /// The line the node starts on (for diagnostics).
    pub line: usize,
}

impl Node {
    /// The value of the attribute `key`, if it has one.
    pub fn attr(&self, key: &str) -> Option<&str> {
        attr_value(&self.attrs, key)
    }
}

/// One resolved treemap.
#[derive(Debug, Clone, PartialEq)]
pub struct Diagram {
    /// The block name, if it has one.
    pub id: Option<String>,
    /// The label of the root rectangle.
    pub label: String,
    /// The attributes of the root rectangle.
    pub root_attrs: Vec<Attr>,
    /// Diagram-level attributes (`width`, `height`, `showvalue`).
    pub attrs: Vec<Attr>,
    /// The children of the root, in source order.
    pub children: Vec<Node>,
    /// The sum of the children (shown as the root's value).
    pub weight: f64,
}

impl Diagram {
    /// The value of the diagram attribute `key`, if it has one.
    pub fn attr(&self, key: &str) -> Option<&str> {
        attr_value(&self.attrs, key)
    }
}

/// A resolved file: one or more treemaps.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Document {
    pub diagrams: Vec<Diagram>,
}

/// Build the resolved document from the syntax tree.
///
/// Class definitions are collected first, so `class` may be written before or
/// after the nodes that use it. A class contributes its attributes as
/// defaults; the attributes written on the node win. An unknown class is an
/// error, as in the activity diagram.
pub fn build(ast: &ast::Document) -> Result<Document, String> {
    let mut diagrams = Vec::new();
    for treemap in &ast.treemaps {
        let classes: BTreeMap<&str, &[Attr]> = treemap
            .classes
            .iter()
            .map(|class| (class.name.as_str(), class.attrs.as_slice()))
            .collect();
        let root_attrs = resolve_attrs(&treemap.root.attrs, &classes)?;
        let label = treemap
            .root
            .name
            .clone()
            .or_else(|| treemap.id.clone())
            .unwrap_or_else(|| "root".to_string());
        let label = attr_value(&root_attrs, "label")
            .map(str::to_string)
            .unwrap_or(label);
        let children: Vec<Node> = treemap
            .children
            .iter()
            .map(|node| build_node(node, &classes))
            .collect::<Result<_, _>>()?;
        let weight = children.iter().map(|node| node.weight).sum();
        diagrams.push(Diagram {
            id: treemap.id.clone(),
            label,
            root_attrs,
            attrs: treemap.attrs.clone(),
            children,
            weight,
        });
    }
    Ok(Document { diagrams })
}

fn build_node(node: &ast::Node, classes: &BTreeMap<&str, &[Attr]>) -> Result<Node, String> {
    let attrs = resolve_attrs(&node.attrs, classes)?;
    let children: Vec<Node> = node
        .children
        .iter()
        .map(|child| build_node(child, classes))
        .collect::<Result<_, _>>()?;
    let weight = node.value.unwrap_or_else(|| {
        if children.is_empty() {
            1.0
        } else {
            children.iter().map(|child| child.weight).sum()
        }
    });
    let label = attr_value(&attrs, "label")
        .map(str::to_string)
        .unwrap_or_else(|| node.name.clone());
    Ok(Node {
        name: node.name.clone(),
        label,
        weight,
        attrs,
        children,
        line: node.line,
    })
}

/// Apply the class named by the `class` attribute, then the node's own
/// attributes. The node wins because [`attr_value`] reads the list backwards.
fn resolve_attrs(attrs: &[Attr], classes: &BTreeMap<&str, &[Attr]>) -> Result<Vec<Attr>, String> {
    let mut merged = Vec::new();
    if let Some(class_name) = attr_value(attrs, "class") {
        let class_attrs = classes
            .get(class_name)
            .ok_or_else(|| format!("unknown class: {class_name}"))?;
        merged.extend(class_attrs.iter().cloned());
    }
    merged.extend(attrs.iter().cloned());
    Ok(merged)
}

#[cfg(test)]
mod tests {
    use super::super::parser::parse;
    use super::*;

    fn model(source: &str) -> Document {
        build(&parse(source).unwrap()).unwrap()
    }

    #[test]
    fn leaves_default_to_one() {
        let document = model("treemap { A; B: 2; }");
        assert_eq!(document.diagrams[0].children[0].weight, 1.0);
        assert_eq!(document.diagrams[0].children[1].weight, 2.0);
        assert_eq!(document.diagrams[0].weight, 3.0);
    }

    #[test]
    fn a_container_defaults_to_the_sum_of_its_children() {
        let document = model("treemap { A { B: 2; C: 3; } D: 5; }");
        let a = &document.diagrams[0].children[0];
        assert_eq!(a.weight, 5.0);
        assert_eq!(document.diagrams[0].weight, 10.0);
    }

    #[test]
    fn an_explicit_container_value_wins() {
        let document = model("treemap { A: 20 { B: 2; C: 3; } D: 5; }");
        assert_eq!(document.diagrams[0].children[0].weight, 20.0);
        assert_eq!(document.diagrams[0].weight, 25.0);
    }

    #[test]
    fn a_class_provides_defaults_and_the_node_overrides_them() {
        let document = model(
            "treemap { class warm [color = \"#fff3bf\", textcolor = red];              A: 1 [class = warm, textcolor = blue]; }",
        );
        let a = &document.diagrams[0].children[0];
        assert_eq!(a.attr("color"), Some("#fff3bf"));
        assert_eq!(a.attr("textcolor"), Some("blue"));
        assert_eq!(a.attr("class"), Some("warm"));
    }

    #[test]
    fn classes_may_be_defined_after_use_and_on_the_root() {
        let document = model(
            "treemap { root \"R\" [class = later]; A: 1 [class = later];              class later [color = green]; }",
        );
        assert_eq!(document.diagrams[0].root_attrs[0].key, "color");
        assert_eq!(
            document.diagrams[0].children[0].attr("color"),
            Some("green")
        );
    }

    #[test]
    fn an_unknown_class_is_an_error() {
        let ast = parse("treemap { A [class = nope]; }").unwrap();
        let err = build(&ast).unwrap_err();
        assert!(err.contains("unknown class"), "{err}");
        assert!(err.contains("nope"), "{err}");
    }

    #[test]
    fn labels_and_root_defaults_are_resolved() {
        let document = model("treemap Budget { root \"Monthly\" { A [label = Alpha]; } }");
        assert_eq!(document.diagrams[0].label, "Monthly");
        assert_eq!(document.diagrams[0].children[0].label, "Alpha");
        let unnamed = model("treemap { A; }");
        assert_eq!(unnamed.diagrams[0].label, "root");
    }
}
