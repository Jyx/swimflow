//! Grid geometry compatibility corpus.
//!
//! Expectations were generated from the reference layout
//! (`ScreenNodeBuilder.build`), so `cargo test` checks integer-grid geometry
//! without needing Python. Grid coordinates are compared strictly.

use swimflow::build;

/// (source, expected `build::dump_geometry` JSON).
const CASES: &[(&str, &str)] = &[
    (
        r##"{
  lane {
    A; C;
  }
  lane {
    B;
  }

  A -> B -> C;
}
"##,
        r##"{"diagrams":[{"width":2,"height":3,"nodes":[{"name":"A","x":0,"y":0,"colwidth":1,"colheight":1,"lane":0},{"name":"C","x":0,"y":2,"colwidth":1,"colheight":1,"lane":0},{"name":"B","x":1,"y":1,"colwidth":1,"colheight":1,"lane":1}],"lanes":[{"x":0,"y":0,"colwidth":1,"colheight":3},{"x":1,"y":1,"colwidth":1,"colheight":1}],"edges":[{"from":"A","to":"B","dir":"forward","fromxy":[0,0],"toxy":[1,1]},{"from":"B","to":"C","dir":"forward","fromxy":[1,1],"toxy":[0,2]}]}]}"##,
    ),
    (
        r##"{ A -> B; }"##,
        r##"{"diagrams":[{"width":1,"height":2,"nodes":[{"name":"A","x":0,"y":0,"colwidth":1,"colheight":1,"lane":0},{"name":"B","x":0,"y":1,"colwidth":1,"colheight":1,"lane":0}],"lanes":[{"x":0,"y":0,"colwidth":1,"colheight":2}],"edges":[{"from":"A","to":"B","dir":"forward","fromxy":[0,0],"toxy":[0,1]}]}]}"##,
    ),
    (
        r##"{ A -> B -> C; }"##,
        r##"{"diagrams":[{"width":1,"height":3,"nodes":[{"name":"A","x":0,"y":0,"colwidth":1,"colheight":1,"lane":0},{"name":"B","x":0,"y":1,"colwidth":1,"colheight":1,"lane":0},{"name":"C","x":0,"y":2,"colwidth":1,"colheight":1,"lane":0}],"lanes":[{"x":0,"y":0,"colwidth":1,"colheight":3}],"edges":[{"from":"A","to":"B","dir":"forward","fromxy":[0,0],"toxy":[0,1]},{"from":"B","to":"C","dir":"forward","fromxy":[0,1],"toxy":[0,2]}]}]}"##,
    ),
    (
        r##"{ A -> B -> C; lane foo { A; } lane bar { B; } }"##,
        r##"{"diagrams":[{"width":2,"height":3,"nodes":[{"name":"A","x":0,"y":0,"colwidth":1,"colheight":1,"lane":0},{"name":"B","x":1,"y":1,"colwidth":1,"colheight":1,"lane":1},{"name":"C","x":1,"y":2,"colwidth":1,"colheight":1,"lane":1}],"lanes":[{"x":0,"y":0,"colwidth":1,"colheight":1},{"x":1,"y":1,"colwidth":1,"colheight":2}],"edges":[{"from":"A","to":"B","dir":"forward","fromxy":[0,0],"toxy":[1,1]},{"from":"B","to":"C","dir":"forward","fromxy":[1,1],"toxy":[1,2]}]}]}"##,
    ),
    (
        r##"{ A -> B; A -> C; }"##,
        r##"{"diagrams":[{"width":2,"height":2,"nodes":[{"name":"A","x":0,"y":0,"colwidth":1,"colheight":1,"lane":0},{"name":"B","x":0,"y":1,"colwidth":1,"colheight":1,"lane":0},{"name":"C","x":1,"y":1,"colwidth":1,"colheight":1,"lane":0}],"lanes":[{"x":0,"y":0,"colwidth":2,"colheight":2}],"edges":[{"from":"A","to":"B","dir":"forward","fromxy":[0,0],"toxy":[0,1]},{"from":"A","to":"C","dir":"forward","fromxy":[0,0],"toxy":[1,1]}]}]}"##,
    ),
    (
        r##"{ A -> B; A -> C; B -> D; C -> D; }"##,
        r##"{"diagrams":[{"width":2,"height":3,"nodes":[{"name":"A","x":0,"y":0,"colwidth":1,"colheight":1,"lane":0},{"name":"B","x":0,"y":1,"colwidth":1,"colheight":1,"lane":0},{"name":"C","x":1,"y":1,"colwidth":1,"colheight":1,"lane":0},{"name":"D","x":0,"y":2,"colwidth":1,"colheight":1,"lane":0}],"lanes":[{"x":0,"y":0,"colwidth":2,"colheight":3}],"edges":[{"from":"A","to":"B","dir":"forward","fromxy":[0,0],"toxy":[0,1]},{"from":"A","to":"C","dir":"forward","fromxy":[0,0],"toxy":[1,1]},{"from":"B","to":"D","dir":"forward","fromxy":[0,1],"toxy":[0,2]},{"from":"C","to":"D","dir":"forward","fromxy":[1,1],"toxy":[0,2]}]}]}"##,
    ),
    (
        r##"{ A -> B; B -> C; C -> A; }"##,
        r##"{"diagrams":[{"width":1,"height":3,"nodes":[{"name":"A","x":0,"y":0,"colwidth":1,"colheight":1,"lane":0},{"name":"B","x":0,"y":1,"colwidth":1,"colheight":1,"lane":0},{"name":"C","x":0,"y":2,"colwidth":1,"colheight":1,"lane":0}],"lanes":[{"x":0,"y":0,"colwidth":1,"colheight":3}],"edges":[{"from":"A","to":"B","dir":"forward","fromxy":[0,0],"toxy":[0,1]},{"from":"B","to":"C","dir":"forward","fromxy":[0,1],"toxy":[0,2]},{"from":"C","to":"A","dir":"forward","fromxy":[0,2],"toxy":[0,0]}]}]}"##,
    ),
    (
        r##"{ A -> B -> C -> D; A -> D; }"##,
        r##"{"diagrams":[{"width":1,"height":4,"nodes":[{"name":"A","x":0,"y":0,"colwidth":1,"colheight":1,"lane":0},{"name":"C","x":0,"y":2,"colwidth":1,"colheight":1,"lane":0},{"name":"B","x":0,"y":1,"colwidth":1,"colheight":1,"lane":0},{"name":"D","x":0,"y":3,"colwidth":1,"colheight":1,"lane":0}],"lanes":[{"x":0,"y":0,"colwidth":1,"colheight":4}],"edges":[{"from":"A","to":"B","dir":"forward","fromxy":[0,0],"toxy":[0,1]},{"from":"A","to":"D","dir":"forward","fromxy":[0,0],"toxy":[0,3]},{"from":"B","to":"C","dir":"forward","fromxy":[0,1],"toxy":[0,2]},{"from":"C","to":"D","dir":"forward","fromxy":[0,2],"toxy":[0,3]}]}]}"##,
    ),
    (
        r##"{ A -> B; C -> B; A -> C; }"##,
        r##"{"diagrams":[{"width":1,"height":3,"nodes":[{"name":"A","x":0,"y":0,"colwidth":1,"colheight":1,"lane":0},{"name":"C","x":0,"y":1,"colwidth":1,"colheight":1,"lane":0},{"name":"B","x":0,"y":2,"colwidth":1,"colheight":1,"lane":0}],"lanes":[{"x":0,"y":0,"colwidth":1,"colheight":3}],"edges":[{"from":"A","to":"B","dir":"forward","fromxy":[0,0],"toxy":[0,2]},{"from":"A","to":"C","dir":"forward","fromxy":[0,0],"toxy":[0,1]},{"from":"C","to":"B","dir":"forward","fromxy":[0,1],"toxy":[0,2]}]}]}"##,
    ),
    (
        r##"{ lane a { X; } lane b { Y; } X -> Y; }"##,
        r##"{"diagrams":[{"width":2,"height":2,"nodes":[{"name":"X","x":0,"y":0,"colwidth":1,"colheight":1,"lane":0},{"name":"Y","x":1,"y":1,"colwidth":1,"colheight":1,"lane":1}],"lanes":[{"x":0,"y":0,"colwidth":1,"colheight":1},{"x":1,"y":1,"colwidth":1,"colheight":1}],"edges":[{"from":"X","to":"Y","dir":"forward","fromxy":[0,0],"toxy":[1,1]}]}]}"##,
    ),
    (
        r##"{ lane a { P; } lane b { Q; } lane c { R; } P -> Q -> R; }"##,
        r##"{"diagrams":[{"width":3,"height":3,"nodes":[{"name":"P","x":0,"y":0,"colwidth":1,"colheight":1,"lane":0},{"name":"Q","x":1,"y":1,"colwidth":1,"colheight":1,"lane":1},{"name":"R","x":2,"y":2,"colwidth":1,"colheight":1,"lane":2}],"lanes":[{"x":0,"y":0,"colwidth":1,"colheight":1},{"x":1,"y":1,"colwidth":1,"colheight":1},{"x":2,"y":2,"colwidth":1,"colheight":1}],"edges":[{"from":"P","to":"Q","dir":"forward","fromxy":[0,0],"toxy":[1,1]},{"from":"Q","to":"R","dir":"forward","fromxy":[1,1],"toxy":[2,2]}]}]}"##,
    ),
    (
        r##"{ lane a { A; B; } lane b { C; } A -> C -> B; }"##,
        r##"{"diagrams":[{"width":2,"height":3,"nodes":[{"name":"A","x":0,"y":0,"colwidth":1,"colheight":1,"lane":0},{"name":"B","x":0,"y":2,"colwidth":1,"colheight":1,"lane":0},{"name":"C","x":1,"y":1,"colwidth":1,"colheight":1,"lane":1}],"lanes":[{"x":0,"y":0,"colwidth":1,"colheight":3},{"x":1,"y":1,"colwidth":1,"colheight":1}],"edges":[{"from":"A","to":"C","dir":"forward","fromxy":[0,0],"toxy":[1,1]},{"from":"C","to":"B","dir":"forward","fromxy":[1,1],"toxy":[0,2]}]}]}"##,
    ),
    (
        r##"{ lane a { A; } lane b { B; } A -> B; A -> C; }"##,
        r##"{"diagrams":[{"width":2,"height":2,"nodes":[{"name":"A","x":0,"y":0,"colwidth":1,"colheight":1,"lane":0},{"name":"B","x":1,"y":1,"colwidth":1,"colheight":1,"lane":1},{"name":"C","x":0,"y":1,"colwidth":1,"colheight":1,"lane":0}],"lanes":[{"x":0,"y":0,"colwidth":1,"colheight":2},{"x":1,"y":1,"colwidth":1,"colheight":1}],"edges":[{"from":"A","to":"B","dir":"forward","fromxy":[0,0],"toxy":[1,1]},{"from":"A","to":"C","dir":"forward","fromxy":[0,0],"toxy":[0,1]}]}]}"##,
    ),
    (
        r##"{ A -> B -> C; C -> D; B -> D; }"##,
        r##"{"diagrams":[{"width":1,"height":4,"nodes":[{"name":"A","x":0,"y":0,"colwidth":1,"colheight":1,"lane":0},{"name":"B","x":0,"y":1,"colwidth":1,"colheight":1,"lane":0},{"name":"C","x":0,"y":2,"colwidth":1,"colheight":1,"lane":0},{"name":"D","x":0,"y":3,"colwidth":1,"colheight":1,"lane":0}],"lanes":[{"x":0,"y":0,"colwidth":1,"colheight":4}],"edges":[{"from":"A","to":"B","dir":"forward","fromxy":[0,0],"toxy":[0,1]},{"from":"B","to":"C","dir":"forward","fromxy":[0,1],"toxy":[0,2]},{"from":"B","to":"D","dir":"forward","fromxy":[0,1],"toxy":[0,3]},{"from":"C","to":"D","dir":"forward","fromxy":[0,2],"toxy":[0,3]}]}]}"##,
    ),
    (
        r##"{ A -> B; A -> C; A -> D; }"##,
        r##"{"diagrams":[{"width":3,"height":2,"nodes":[{"name":"A","x":0,"y":0,"colwidth":1,"colheight":1,"lane":0},{"name":"B","x":0,"y":1,"colwidth":1,"colheight":1,"lane":0},{"name":"C","x":1,"y":1,"colwidth":1,"colheight":1,"lane":0},{"name":"D","x":2,"y":1,"colwidth":1,"colheight":1,"lane":0}],"lanes":[{"x":0,"y":0,"colwidth":3,"colheight":2}],"edges":[{"from":"A","to":"B","dir":"forward","fromxy":[0,0],"toxy":[0,1]},{"from":"A","to":"C","dir":"forward","fromxy":[0,0],"toxy":[1,1]},{"from":"A","to":"D","dir":"forward","fromxy":[0,0],"toxy":[2,1]}]}]}"##,
    ),
    (
        r##"{ A -> D; B -> D; C -> D; A -> B; B -> C; }"##,
        r##"{"diagrams":[{"width":1,"height":4,"nodes":[{"name":"A","x":0,"y":0,"colwidth":1,"colheight":1,"lane":0},{"name":"B","x":0,"y":1,"colwidth":1,"colheight":1,"lane":0},{"name":"C","x":0,"y":2,"colwidth":1,"colheight":1,"lane":0},{"name":"D","x":0,"y":3,"colwidth":1,"colheight":1,"lane":0}],"lanes":[{"x":0,"y":0,"colwidth":1,"colheight":4}],"edges":[{"from":"A","to":"B","dir":"forward","fromxy":[0,0],"toxy":[0,1]},{"from":"A","to":"D","dir":"forward","fromxy":[0,0],"toxy":[0,3]},{"from":"B","to":"C","dir":"forward","fromxy":[0,1],"toxy":[0,2]},{"from":"B","to":"D","dir":"forward","fromxy":[0,1],"toxy":[0,3]},{"from":"C","to":"D","dir":"forward","fromxy":[0,2],"toxy":[0,3]}]}]}"##,
    ),
    (
        r##"{ A -> B -> C -> A; C -> D; }"##,
        r##"{"diagrams":[{"width":1,"height":4,"nodes":[{"name":"A","x":0,"y":0,"colwidth":1,"colheight":1,"lane":0},{"name":"B","x":0,"y":1,"colwidth":1,"colheight":1,"lane":0},{"name":"C","x":0,"y":2,"colwidth":1,"colheight":1,"lane":0},{"name":"D","x":0,"y":3,"colwidth":1,"colheight":1,"lane":0}],"lanes":[{"x":0,"y":0,"colwidth":1,"colheight":4}],"edges":[{"from":"A","to":"B","dir":"forward","fromxy":[0,0],"toxy":[0,1]},{"from":"B","to":"C","dir":"forward","fromxy":[0,1],"toxy":[0,2]},{"from":"C","to":"A","dir":"forward","fromxy":[0,2],"toxy":[0,0]},{"from":"C","to":"D","dir":"forward","fromxy":[0,2],"toxy":[0,3]}]}]}"##,
    ),
    (
        r##"{ lane x { A; } lane y { B; C; } lane z { D; } A -> B; A -> C; B -> D; C -> D; }"##,
        r##"{"diagrams":[{"width":4,"height":3,"nodes":[{"name":"A","x":0,"y":0,"colwidth":1,"colheight":1,"lane":0},{"name":"B","x":1,"y":1,"colwidth":1,"colheight":1,"lane":1},{"name":"C","x":2,"y":1,"colwidth":1,"colheight":1,"lane":1},{"name":"D","x":3,"y":2,"colwidth":1,"colheight":1,"lane":2}],"lanes":[{"x":0,"y":0,"colwidth":1,"colheight":1},{"x":1,"y":1,"colwidth":2,"colheight":1},{"x":3,"y":2,"colwidth":1,"colheight":1}],"edges":[{"from":"A","to":"B","dir":"forward","fromxy":[0,0],"toxy":[1,1]},{"from":"A","to":"C","dir":"forward","fromxy":[0,0],"toxy":[2,1]},{"from":"B","to":"D","dir":"forward","fromxy":[1,1],"toxy":[3,2]},{"from":"C","to":"D","dir":"forward","fromxy":[2,1],"toxy":[3,2]}]}]}"##,
    ),
    (
        r##"{ orientation = landscape; A -> B -> C; lane foo { A; } lane bar { B; } }"##,
        r##"{"diagrams":[{"width":3,"height":2,"nodes":[{"name":"A","x":0,"y":0,"colwidth":1,"colheight":1,"lane":0},{"name":"B","x":1,"y":1,"colwidth":1,"colheight":1,"lane":1},{"name":"C","x":2,"y":1,"colwidth":1,"colheight":1,"lane":1}],"lanes":[{"x":0,"y":0,"colwidth":1,"colheight":1},{"x":1,"y":1,"colwidth":2,"colheight":1}],"edges":[{"from":"A","to":"B","dir":"forward","fromxy":[0,0],"toxy":[1,1]},{"from":"B","to":"C","dir":"forward","fromxy":[1,1],"toxy":[2,1]}]}]}"##,
    ),
    (
        r##"{ A, B [color = red]; A -> B; C; }"##,
        r##"{"diagrams":[{"width":2,"height":2,"nodes":[{"name":"A","x":0,"y":0,"colwidth":1,"colheight":1,"lane":0},{"name":"B","x":0,"y":1,"colwidth":1,"colheight":1,"lane":0},{"name":"C","x":1,"y":0,"colwidth":1,"colheight":1,"lane":0}],"lanes":[{"x":0,"y":0,"colwidth":2,"colheight":2}],"edges":[{"from":"A","to":"B","dir":"forward","fromxy":[0,0],"toxy":[0,1]}]}]}"##,
    ),
    (
        r##"{ A -> B [label = x]; B -> C; }"##,
        r##"{"diagrams":[{"width":1,"height":3,"nodes":[{"name":"A","x":0,"y":0,"colwidth":1,"colheight":1,"lane":0},{"name":"B","x":0,"y":1,"colwidth":1,"colheight":1,"lane":0},{"name":"C","x":0,"y":2,"colwidth":1,"colheight":1,"lane":0}],"lanes":[{"x":0,"y":0,"colwidth":1,"colheight":3}],"edges":[{"from":"A","to":"B","dir":"forward","fromxy":[0,0],"toxy":[0,1]},{"from":"B","to":"C","dir":"forward","fromxy":[0,1],"toxy":[0,2]}]}]}"##,
    ),
    (
        r##"{ A -- B; B <- C; C <-> D; }"##,
        r##"{"diagrams":[{"width":1,"height":4,"nodes":[{"name":"A","x":0,"y":0,"colwidth":1,"colheight":1,"lane":0},{"name":"B","x":0,"y":1,"colwidth":1,"colheight":1,"lane":0},{"name":"C","x":0,"y":2,"colwidth":1,"colheight":1,"lane":0},{"name":"D","x":0,"y":3,"colwidth":1,"colheight":1,"lane":0}],"lanes":[{"x":0,"y":0,"colwidth":1,"colheight":4}],"edges":[{"from":"A","to":"B","dir":"none","fromxy":[0,0],"toxy":[0,1]},{"from":"B","to":"C","dir":"back","fromxy":[0,1],"toxy":[0,2]},{"from":"C","to":"D","dir":"both","fromxy":[0,2],"toxy":[0,3]}]}]}"##,
    ),
    (
        r##"{ lane { A; C; } lane { B; } A -> B -> C; }"##,
        r##"{"diagrams":[{"width":2,"height":3,"nodes":[{"name":"A","x":0,"y":0,"colwidth":1,"colheight":1,"lane":0},{"name":"C","x":0,"y":2,"colwidth":1,"colheight":1,"lane":0},{"name":"B","x":1,"y":1,"colwidth":1,"colheight":1,"lane":1}],"lanes":[{"x":0,"y":0,"colwidth":1,"colheight":3},{"x":1,"y":1,"colwidth":1,"colheight":1}],"edges":[{"from":"A","to":"B","dir":"forward","fromxy":[0,0],"toxy":[1,1]},{"from":"B","to":"C","dir":"forward","fromxy":[1,1],"toxy":[0,2]}]}]}"##,
    ),
];

#[test]
fn geometry_matches_reference() {
    for (src, expected) in CASES {
        let set = build::build_str(src).unwrap_or_else(|e| panic!("{src:?}: {e}"));
        assert_eq!(build::dump_geometry(&set), *expected, "input {src:?}");
    }
}
