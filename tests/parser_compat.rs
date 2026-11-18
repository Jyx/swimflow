//! Parser compatibility corpus.
//!
//! `PARITY` expectations were generated from the reference Python parser
//! parser so `cargo test` verifies accept/reject parity without
//! needing Python. `EXTENSIONS` are swimflow syntax that upstream rejects.

use swimflow::parser;

/// (source, expected_to_parse) — generated from the reference parser.
const PARITY: &[(&str, bool)] = &[
    (r##"{ A -> B; }"##, true),
    (r##"{ A -> B -> C [label = x]; }"##, true),
    (r##"{ A, B [color = red]; }"##, true),
    (r##"{ A; orientation = landscape; }"##, true),
    (
        r##"{ class red [color = red]; lane foo [color = blue]; A [class = red]; }"##,
        true,
    ),
    (r##"{ lane { A; C; } lane { B; } A -> B -> C; }"##, true),
    (r##"{ A -> B [label = "x"]; }"##, true),
    (r##"actdiag { A }"##, true),
    (r##"diagram { A }"##, true),
    (r##"diagram named { A }"##, true),
    (r##"{ "a b" -> c; }"##, true),
    (r##"{ 'a b' -> c; }"##, true),
    (r##"{ A -- B; }"##, true),
    (r##"{ A <- B; }"##, true),
    (r##"{ A <-> B; }"##, true),
    (
        r##"{ // comment
 A; }"##,
        true,
    ),
    (
        r##"{ # comment
 A; }"##,
        true,
    ),
    (r##"{ /* c */ A; }"##, true),
    (r##"{ A [color = "#ff0000"]; }"##, true),
    (r##"{ A [foo]; }"##, true),
    (r##"{ A [foo, bar]; }"##, true),
    (
        r##"{ A -> B [label = """multi
line"""]; }"##,
        true,
    ),
    (r##"{ あ -> B; }"##, true),
    (r##"{ A1 -> B2.3; }"##, true),
    (r##"{ C, D [attr = value, attr2 = value2]; }"##, true),
    (r##"{ A -> B; B -> C; }"##, true),
    (r##"{ plugin autolane; }"##, true),
    (r##"{ A = B; }"##, true),
    (r##"{ }"##, true),
    (r##"{ A }"##, true),
    (r##"{}"##, true),
    (r##"{ lane; }"##, true),
    (r##"{ lane foo }"##, true),
    (r##"{ lane foo bar }"##, true),
    (r##"{ class; }"##, true),
    (r##"{ plugin; }"##, true),
    (r##"{ class = red; }"##, true),
    (r##"{ class -> B; }"##, true),
    (r##"{ A--B }"##, true),
    (r##"{ A-B }"##, true),
    (r##"{ A.B }"##, true),
    (r##"{ 42 }"##, true),
    (r##"{ -1 }"##, false),
    (r##"{ .5 -> x; }"##, true),
    (r##"{ lane foo [attr]; }"##, true),
    (r##"{ lane foo [color = red]; }"##, true),
    (r##"{ lane { A; B; } }"##, true),
    (r##"{ lane "quoted" { A; } }"##, true),
    (r##"{ A [label = "a\"b"]; }"##, true),
    (r##"{ A [label = 'x']; }"##, true),
    (r##"{ A -> B <- C; }"##, true),
    (r##"{ A, B -> C, D; }"##, true),
    (r##"{ A->B }"##, false),
    (r##"{ A; "##, false),
    (r##"{ A -> }"##, false),
    (r##"{ A [ = B]; }"##, false),
    (r##"{ A [color = ]; }"##, false),
    (r##"{ A -> B -> ; }"##, false),
    (r##"{ : }"##, false),
    (r##"{ A -<> B }"##, false),
    (r##"{ A @ B }"##, false),
    (r##"{ "unterminated }"##, false),
    (r##"{ A [foo = bar }"##, false),
];

/// swimflow extensions not understood by the reference implementation.
const EXTENSIONS: &[&str] = &[
    r##"diagram a { X; } diagram b { Y; }"##,
    r##"{ A [ref = "#detail"]; }"##,
    r##"{ legend { class red; numbered; } A; }"##,
    r##"{ numbering = auto; A; B [numbered = 2]; }"##,
    // Inline lane-block attrs (upstream rejects `lane foo [attrs] { ... }`).
    r##"{ lane foo [color = "#ff0000"] { A; } }"##,
];

#[test]
fn accept_reject_matches_reference() {
    for (src, expect_ok) in PARITY {
        let result = parser::parse(src);
        assert_eq!(
            result.is_ok(),
            *expect_ok,
            "input {src:?} => parsed={:?}",
            result.err()
        );
    }
}

#[test]
fn swimflow_extensions_parse() {
    for src in EXTENSIONS {
        parser::parse(src).unwrap_or_else(|e| panic!("{src:?}: {e}"));
    }
}
