// SPDX-License-Identifier: MIT OR Apache-2.0

mod common;

use dictator_core::export::eds;

/// Lines that legitimately differ: the sample was exported at a later time
/// than the project was created, and we keep no modification timestamp.
fn stable(text: &str) -> Vec<&str> {
    text.lines()
        .filter(|l| !l.starts_with("ModificationTime=") && !l.starts_with("ModificationDate="))
        .collect()
}

#[test]
fn eds_matches_canopeneditor_export() {
    let od = common::sample_xdd();
    let ours = eds::generate(&od, "example.eds");
    let theirs = common::read_asset("example.eds");
    let (a, b) = (stable(&ours), stable(&theirs));
    for (n, (x, y)) in a.iter().zip(b.iter()).enumerate() {
        assert_eq!(x, y, "first difference at filtered line {}", n + 1);
    }
    assert_eq!(a.len(), b.len());
}
