mod common;

use dictator::export::markdown;

/// Drop CRs (the reference mixes CRLF and LF) and the lines that name the
/// generating tool, the project file and the modification time.
fn stable(text: &str) -> Vec<String> {
    text.replace('\r', "")
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
    let ours = stable(&markdown::generate(&od, "legacy.objdict.toml"));
    let theirs = stable(&common::read_asset("legacy_objdict.md"));
    for (n, (x, y)) in ours.iter().zip(theirs.iter()).enumerate() {
        assert_eq!(x, y, "first difference at filtered line {}", n + 1);
    }
    assert_eq!(ours.len(), theirs.len());
}
