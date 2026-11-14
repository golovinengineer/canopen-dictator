mod common;

use dictator::canfestival::gen_c;

/// The reference files were run through clang-format, so only the token
/// stream is comparable: drop all whitespace.
fn squash(text: &str) -> String {
    text.chars().filter(|c| !c.is_whitespace()).collect()
}

fn assert_same(ours: &str, theirs: &str, what: &str) {
    let (a, b) = (squash(ours), squash(theirs));
    if a == b {
        return;
    }
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let at = a.iter().zip(&b).position(|(x, y)| x != y).unwrap_or(a.len().min(b.len()));
    let ctx = |v: &[char]| -> String {
        v[at.saturating_sub(120)..(at + 120).min(v.len())].iter().collect()
    };
    panic!("{what} differs at squashed offset {at}\n ours:   {}\n theirs: {}", ctx(&a), ctx(&b));
}

#[test]
fn c_and_header_match_canfestival_output() {
    let mut od = common::sample_xdd();
    // Callbacks were enabled by hand on every manufacturer object in the
    // original .od; the XDD cannot express that and the default is off.
    for (i, o) in od.objects.iter_mut() {
        o.callback = *i >= 0x2000;
    }
    let (c, h) = gen_c::generate(&od, "ObjDict.h").unwrap();
    assert_same(&h, &common::read_asset("ObjDict.h"), "ObjDict.h");
    assert_same(&c, &common::read_asset("ObjDict.c"), "ObjDict.c");
}
