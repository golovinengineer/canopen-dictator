// SPDX-License-Identifier: MIT OR Apache-2.0

mod common;

use dictator_core::export::markdown;

/// Drop CRs (the reference mixes CRLF and LF) and the lines that name the
/// generating tool, the project file and the modification time. .NET on
/// macOS writes a narrow no-break space before AM/PM; Windows a space.
fn stable(text: &str) -> Vec<String> {
    text.replace('\r', "")
        .replace('\u{202f}', " ")
        .lines()
        .filter(|l| {
            !l.starts_with("| Modified ")
                && !l.starts_with("| Project File")
                && !l.starts_with("This file was automatically generated")
        })
        .map(str::to_string)
        .collect()
}

#[test]
fn docs_match_canopeneditor_export() {
    let od = common::sample_xdd();
    let ours = stable(&markdown::generate(&od, "example.xdd"));
    let theirs = stable(&common::read_asset("example.md"));
    for (n, (x, y)) in ours.iter().zip(theirs.iter()).enumerate() {
        assert_eq!(x, y, "first difference at filtered line {}", n + 1);
    }
    assert_eq!(ours.len(), theirs.len());
}
