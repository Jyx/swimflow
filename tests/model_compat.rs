//! Model compatibility corpus.
//!
//! Expectations were generated from the reference builder
//! (`ScreenNodeBuilder.build`), so `cargo test` checks lane assignment,
//! node labels and edge structure without needing Python.

use swimflow::build;

/// (source, expected `build::dump` output).
///
/// One case per line keeps the corpus readable; rustfmt's 2024 style edition would
/// split every tuple over four lines, so formatting is skipped here.
#[rustfmt::skip]
const CASES: &[(&str, &str)] = &[
    ("{\n  lane {\n    A; C;\n  }\n  lane {\n    B;\n  }\n\n  A -> B -> C;\n}\n", "diagram default\nlane 0 nodes=A,C\nlane 1 nodes=B\nnode A label=A lane=0\nnode C label=C lane=0\nnode B label=B lane=1\nedge A B f\nedge B C f\n"),
    ("{ A -> B; }", "diagram default\nlane 0 nodes=A,B\nnode A label=A lane=0\nnode B label=B lane=0\nedge A B f\n"),
    ("{ lane foo { A; } A -> B; }", "diagram default\nlane 0 nodes=A,B\nnode A label=A lane=0\nnode B label=B lane=0\nedge A B f\n"),
    ("{ A -> B -> C; lane x { B; } }", "diagram default\nlane 0 nodes=B,A,C\nnode A label=A lane=0\nnode B label=B lane=0\nnode C label=C lane=0\nedge A B f\nedge B C f\n"),
    ("{ lane a { X; } lane b { Y; } X -> Y; }", "diagram default\nlane 0 nodes=X\nlane 1 nodes=Y\nnode X label=X lane=0\nnode Y label=Y lane=1\nedge X Y f\n"),
    ("{ A; B; C; A -> B -> C; }", "diagram default\nlane 0 nodes=A,B,C\nnode A label=A lane=0\nnode B label=B lane=0\nnode C label=C lane=0\nedge A B f\nedge B C f\n"),
    ("{ lane a { P; } lane b { Q; } lane c { R; } P -> Q -> R; }", "diagram default\nlane 0 nodes=P\nlane 1 nodes=Q\nlane 2 nodes=R\nnode P label=P lane=0\nnode Q label=Q lane=1\nnode R label=R lane=2\nedge P Q f\nedge Q R f\n"),
    ("{ lane x { A; } lane y { } B; }", "diagram default\nlane 0 nodes=A,B\nnode A label=A lane=0\nnode B label=B lane=0\n"),
    ("{ A <- B; }", "diagram default\nlane 0 nodes=A,B\nnode A label=A lane=0\nnode B label=B lane=0\nedge A B r\n"),
    ("{ A -- B; }", "diagram default\nlane 0 nodes=A,B\nnode A label=A lane=0\nnode B label=B lane=0\nedge A B u\n"),
    ("{ A <-> B; }", "diagram default\nlane 0 nodes=A,B\nnode A label=A lane=0\nnode B label=B lane=0\nedge A B b\n"),
    ("{ A [label = foo]; }", "diagram default\nlane 0 nodes=A\nnode A label=foo lane=0\n"),
    ("{ A, B [color = red]; }", "diagram default\nlane 0 nodes=A,B\nnode A label=A lane=0\nnode B label=B lane=0\n"),
    ("{ A -> B; A -> B [color = red]; }", "diagram default\nlane 0 nodes=A,B\nnode A label=A lane=0\nnode B label=B lane=0\nedge A B f\n"),
    ("{ A -> B; B -> C; C -> A; }", "diagram default\nlane 0 nodes=A,B,C\nnode A label=A lane=0\nnode B label=B lane=0\nnode C label=C lane=0\nedge A B f\nedge B C f\nedge C A f\n"),
    ("{ lane a { A; B; } lane b { C; } A -> C -> B; }", "diagram default\nlane 0 nodes=A,B\nlane 1 nodes=C\nnode A label=A lane=0\nnode B label=B lane=0\nnode C label=C lane=1\nedge A C f\nedge C B f\n"),
    // Inline lane-block attrs (swimflow extension) behave like a declaration.
    ("{ lane foo [color = red] { A; } }", "diagram default\nlane 0 nodes=A\nnode A label=A lane=0\n"),
];

/// Inputs the reference builder rejects.
const ERROR_CASES: &[&str] = &["{ lane a { X; } lane b { X; } }"];

#[test]
fn model_matches_reference() {
    for (src, expected) in CASES {
        let set = build::build_str(src).unwrap_or_else(|e| panic!("{src:?}: {e}"));
        assert_eq!(build::dump(&set), *expected, "input {src:?}");
    }
}

#[test]
fn reference_error_cases_also_fail() {
    for src in ERROR_CASES {
        assert!(build::build_str(src).is_err(), "expected error for {src:?}");
    }
}
