//! Pixel-metrics compatibility corpus.
//!
//! Expectations were generated from the reference metrics
//! (`DiagramMetrics`). Node/lane/frame boxes are
//! font-independent, so they are compared with tolerance 0.

use swimflow::build;
use swimflow::metrics;

/// (source, expected `metrics::dump_pixels` JSON).
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
        r##"{"diagrams":[{"pagesize":[448,360],"nodes":[{"name":"A","box":[64,120,192,160]},{"name":"C","box":[64,280,192,320]},{"name":"B","box":[256,200,384,240]}],"lanes":[{"textbox":[64,38,192,100],"headerbox":[32,38,224,100]},{"textbox":[256,38,384,100],"headerbox":[224,38,416,100]}],"frame":{"headerbox":[32,38,416,100],"outline":[32,38,416,340],"separators":[[[32,100],[416,100]],[[224,38],[224,340]]]}}]}"##,
    ),
    (
        r##"{ A -> B; }"##,
        r##"{"diagrams":[{"pagesize":[256,280],"nodes":[{"name":"A","box":[64,120,192,160]},{"name":"B","box":[64,200,192,240]}],"lanes":[{"textbox":[64,38,192,100],"headerbox":[32,38,224,100]}],"frame":{"headerbox":[32,38,224,100],"outline":[32,38,224,260],"separators":[[[32,100],[224,100]]]}}]}"##,
    ),
    (
        r##"{ A -> B -> C; }"##,
        r##"{"diagrams":[{"pagesize":[256,360],"nodes":[{"name":"A","box":[64,120,192,160]},{"name":"B","box":[64,200,192,240]},{"name":"C","box":[64,280,192,320]}],"lanes":[{"textbox":[64,38,192,100],"headerbox":[32,38,224,100]}],"frame":{"headerbox":[32,38,224,100],"outline":[32,38,224,340],"separators":[[[32,100],[224,100]]]}}]}"##,
    ),
    (
        r##"{ A -> B -> C; lane foo { A; } lane bar { B; } }"##,
        r##"{"diagrams":[{"pagesize":[448,360],"nodes":[{"name":"A","box":[64,120,192,160]},{"name":"B","box":[256,200,384,240]},{"name":"C","box":[256,280,384,320]}],"lanes":[{"textbox":[64,38,192,100],"headerbox":[32,38,224,100]},{"textbox":[256,38,384,100],"headerbox":[224,38,416,100]}],"frame":{"headerbox":[32,38,416,100],"outline":[32,38,416,340],"separators":[[[32,100],[416,100]],[[224,38],[224,340]]]}}]}"##,
    ),
    (
        r##"{ A -> B; A -> C; }"##,
        r##"{"diagrams":[{"pagesize":[448,280],"nodes":[{"name":"A","box":[64,120,192,160]},{"name":"B","box":[64,200,192,240]},{"name":"C","box":[256,200,384,240]}],"lanes":[{"textbox":[64,38,384,100],"headerbox":[32,38,416,100]}],"frame":{"headerbox":[32,38,416,100],"outline":[32,38,416,260],"separators":[[[32,100],[416,100]]]}}]}"##,
    ),
    (
        r##"{ A -> B; A -> C; B -> D; C -> D; }"##,
        r##"{"diagrams":[{"pagesize":[448,360],"nodes":[{"name":"A","box":[64,120,192,160]},{"name":"B","box":[64,200,192,240]},{"name":"C","box":[256,200,384,240]},{"name":"D","box":[64,280,192,320]}],"lanes":[{"textbox":[64,38,384,100],"headerbox":[32,38,416,100]}],"frame":{"headerbox":[32,38,416,100],"outline":[32,38,416,340],"separators":[[[32,100],[416,100]]]}}]}"##,
    ),
    (
        r##"{ A -> B; B -> C; C -> A; }"##,
        r##"{"diagrams":[{"pagesize":[256,360],"nodes":[{"name":"A","box":[64,120,192,160]},{"name":"B","box":[64,200,192,240]},{"name":"C","box":[64,280,192,320]}],"lanes":[{"textbox":[64,38,192,100],"headerbox":[32,38,224,100]}],"frame":{"headerbox":[32,38,224,100],"outline":[32,38,224,340],"separators":[[[32,100],[224,100]]]}}]}"##,
    ),
    (
        r##"{ lane a { X; } lane b { Y; } X -> Y; }"##,
        r##"{"diagrams":[{"pagesize":[448,280],"nodes":[{"name":"X","box":[64,120,192,160]},{"name":"Y","box":[256,200,384,240]}],"lanes":[{"textbox":[64,38,192,100],"headerbox":[32,38,224,100]},{"textbox":[256,38,384,100],"headerbox":[224,38,416,100]}],"frame":{"headerbox":[32,38,416,100],"outline":[32,38,416,260],"separators":[[[32,100],[416,100]],[[224,38],[224,260]]]}}]}"##,
    ),
    (
        r##"{ lane a { P; } lane b { Q; } lane c { R; } P -> Q -> R; }"##,
        r##"{"diagrams":[{"pagesize":[640,360],"nodes":[{"name":"P","box":[64,120,192,160]},{"name":"Q","box":[256,200,384,240]},{"name":"R","box":[448,280,576,320]}],"lanes":[{"textbox":[64,38,192,100],"headerbox":[32,38,224,100]},{"textbox":[256,38,384,100],"headerbox":[224,38,416,100]},{"textbox":[448,38,576,100],"headerbox":[416,38,608,100]}],"frame":{"headerbox":[32,38,608,100],"outline":[32,38,608,340],"separators":[[[32,100],[608,100]],[[224,38],[224,340]],[[416,38],[416,340]]]}}]}"##,
    ),
    (
        r##"{ lane a { A; B; } lane b { C; } A -> C -> B; }"##,
        r##"{"diagrams":[{"pagesize":[448,360],"nodes":[{"name":"A","box":[64,120,192,160]},{"name":"B","box":[64,280,192,320]},{"name":"C","box":[256,200,384,240]}],"lanes":[{"textbox":[64,38,192,100],"headerbox":[32,38,224,100]},{"textbox":[256,38,384,100],"headerbox":[224,38,416,100]}],"frame":{"headerbox":[32,38,416,100],"outline":[32,38,416,340],"separators":[[[32,100],[416,100]],[[224,38],[224,340]]]}}]}"##,
    ),
    (
        r##"{ lane x { A; } lane y { B; C; } lane z { D; } A -> B; A -> C; B -> D; C -> D; }"##,
        r##"{"diagrams":[{"pagesize":[832,360],"nodes":[{"name":"A","box":[64,120,192,160]},{"name":"B","box":[256,200,384,240]},{"name":"C","box":[448,200,576,240]},{"name":"D","box":[640,280,768,320]}],"lanes":[{"textbox":[64,38,192,100],"headerbox":[32,38,224,100]},{"textbox":[256,38,576,100],"headerbox":[224,38,608,100]},{"textbox":[640,38,768,100],"headerbox":[608,38,800,100]}],"frame":{"headerbox":[32,38,800,100],"outline":[32,38,800,340],"separators":[[[32,100],[800,100]],[[224,38],[224,340]],[[608,38],[608,340]]]}}]}"##,
    ),
    (
        r##"{ A [width = 200, height = 50]; A -> B; }"##,
        r##"{"diagrams":[{"pagesize":[328,290],"nodes":[{"name":"A","box":[64,120,264,170]},{"name":"B","box":[100,210,228,250]}],"lanes":[{"textbox":[64,38,264,100],"headerbox":[68,38,260,100]}],"frame":{"headerbox":[32,38,296,100],"outline":[32,38,296,270],"separators":[[[32,100],[296,100]]]}}]}"##,
    ),
    (
        r##"{ A [width = 200]; B [width = 10]; A -> B; }"##,
        r##"{"diagrams":[{"pagesize":[328,280],"nodes":[{"name":"A","box":[64,120,264,160]},{"name":"B","box":[159,200,169,240]}],"lanes":[{"textbox":[64,38,264,100],"headerbox":[68,38,260,100]}],"frame":{"headerbox":[32,38,296,100],"outline":[32,38,296,260],"separators":[[[32,100],[296,100]]]}}]}"##,
    ),
    (
        r##"{ A [colwidth = 2]; A -> B; }"##,
        r##"{"diagrams":[{"pagesize":[256,360],"nodes":[{"name":"A","box":[64,120,192,240]},{"name":"B","box":[64,280,192,320]}],"lanes":[{"textbox":[64,38,192,100],"headerbox":[32,38,224,100]}],"frame":{"headerbox":[32,38,224,100],"outline":[32,38,224,340],"separators":[[[32,100],[224,100]]]}}]}"##,
    ),
    (
        r##"{ A [colheight = 2]; A -> B; }"##,
        r##"{"diagrams":[{"pagesize":[448,280],"nodes":[{"name":"A","box":[64,120,384,160]},{"name":"B","box":[64,200,192,240]}],"lanes":[{"textbox":[64,38,384,100],"headerbox":[32,38,416,100]}],"frame":{"headerbox":[32,38,416,100],"outline":[32,38,416,260],"separators":[[[32,100],[416,100]]]}}]}"##,
    ),
    (
        r##"{ A [colwidth = 2, colheight = 2]; A -> B -> C; }"##,
        r##"{"diagrams":[{"pagesize":[448,440],"nodes":[{"name":"A","box":[64,120,384,240]},{"name":"B","box":[64,280,192,320]},{"name":"C","box":[64,360,192,400]}],"lanes":[{"textbox":[64,38,384,100],"headerbox":[32,38,416,100]}],"frame":{"headerbox":[32,38,416,100],"outline":[32,38,416,420],"separators":[[[32,100],[416,100]]]}}]}"##,
    ),
    (
        r##"{ node_width = 100; A -> B; }"##,
        r##"{"diagrams":[{"pagesize":[228,280],"nodes":[{"name":"A","box":[64,120,164,160]},{"name":"B","box":[64,200,164,240]}],"lanes":[{"textbox":[64,38,164,100],"headerbox":[32,38,196,100]}],"frame":{"headerbox":[32,38,196,100],"outline":[32,38,196,260],"separators":[[[32,100],[196,100]]]}}]}"##,
    ),
    (
        r##"{ node_width = 100; node_height = 30; A -> B; }"##,
        r##"{"diagrams":[{"pagesize":[228,250],"nodes":[{"name":"A","box":[64,110,164,140]},{"name":"B","box":[64,180,164,210]}],"lanes":[{"textbox":[64,38,164,90],"headerbox":[32,38,196,90]}],"frame":{"headerbox":[32,38,196,90],"outline":[32,38,196,230],"separators":[[[32,90],[196,90]]]}}]}"##,
    ),
    (
        r##"{ span_width = 20; span_height = 10; A -> B; }"##,
        r##"{"diagrams":[{"pagesize":[168,160],"nodes":[{"name":"A","box":[20,60,148,100]},{"name":"B","box":[20,110,148,150]}],"lanes":[{"textbox":[20,8,148,55],"headerbox":[10,8,158,55]}],"frame":{"headerbox":[10,8,158,55],"outline":[10,8,158,155],"separators":[[[10,55],[158,55]]]}}]}"##,
    ),
    (
        r##"{ span_width = 20; span_height = 10; lane a { A; } lane b { B; } A -> B; }"##,
        r##"{"diagrams":[{"pagesize":[316,160],"nodes":[{"name":"A","box":[20,60,148,100]},{"name":"B","box":[168,110,296,150]}],"lanes":[{"textbox":[20,8,148,55],"headerbox":[10,8,158,55]},{"textbox":[168,8,296,55],"headerbox":[158,8,306,55]}],"frame":{"headerbox":[10,8,306,55],"outline":[10,8,306,155],"separators":[[[10,55],[306,55]],[[158,8],[158,155]]]}}]}"##,
    ),
    (
        r##"{ orientation = landscape; A -> B -> C; lane foo { A; } lane bar { B; } }"##,
        r##"{"diagrams":[{"pagesize":[832,200],"nodes":[{"name":"A","box":[256,40,384,80]},{"name":"B","box":[448,120,576,160]},{"name":"C","box":[640,120,768,160]}],"lanes":[{"textbox":[256,-42,384,20],"headerbox":[224,-42,416,20]},{"textbox":[448,-42,768,20],"headerbox":[416,-42,800,20]}],"frame":{"headerbox":[224,-42,800,20],"outline":[224,-42,800,180],"separators":[[[224,20],[800,20]],[[416,-42],[416,180]]]}}]}"##,
    ),
    (
        r##"{ orientation = landscape; A -> B; A -> C; }"##,
        r##"{"diagrams":[{"pagesize":[640,200],"nodes":[{"name":"A","box":[256,40,384,80]},{"name":"B","box":[448,40,576,80]},{"name":"C","box":[448,120,576,160]}],"lanes":[{"textbox":[256,-42,576,20],"headerbox":[224,-42,608,20]}],"frame":{"headerbox":[224,-42,608,20],"outline":[224,-42,608,180],"separators":[[[224,20],[608,20]]]}}]}"##,
    ),
    (
        r##"{ }"##,
        r##"{"diagrams":[{"pagesize":[256,200],"nodes":[],"lanes":[],"frame":{"headerbox":[32,38,224,100],"outline":[32,38,224,180],"separators":[[[32,100],[224,100]]]}}]}"##,
    ),
];

#[test]
fn pixels_match_reference() {
    for (src, expected) in CASES {
        let set = build::build_str(src).unwrap_or_else(|e| panic!("{src:?}: {e}"));
        assert_eq!(metrics::dump_pixels(&set), *expected, "input {src:?}");
    }
}
