// SPDX-License-Identifier: LGPL-2.1-or-later
// Part of the Rust port of CanFestival objdictgen; see lib.rs for copyright.

#![allow(dead_code)]
use std::path::PathBuf;

use canopen_dictator_core::model::ObjectDictionary;
use canopen_dictator_core::project;

/// A file of the neutral example set (see `assets/example/README.md`).
pub fn asset(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/example")
        .join(name)
}

pub fn read_asset(name: &str) -> String {
    std::fs::read_to_string(asset(name)).unwrap()
}

/// The example as imported from the CANopenEditor XDD.
pub fn sample_xdd() -> ObjectDictionary {
    canopen_dictator_core::xdd::load(&asset("example.xdd")).unwrap()
}

/// The checked-in project as the CanFestival references were made from it:
/// CanFestival settings included, and the two objects CanFestival cannot
/// take left out (UNICODE_STRING 0x200D; DOMAIN 0x200E has no initial
/// value, which gen_cfile refuses).
pub fn canfestival_project() -> ObjectDictionary {
    let mut od = project::load(&asset("example.objdict.toml")).unwrap();
    for i in [0x200D, 0x200E] {
        od.objects.get_mut(&i).unwrap().disabled = true;
    }
    od
}

/// Compare generated C with raw gen_cfile output. Only the token stream is
/// comparable (formatting differs), and two gen_cfile bugs that Dictator
/// deliberately does not copy are undone in the reference: `0x-64` for a
/// negative number (not valid C) and a doubled `const CONSTSTORE`.
pub fn assert_same_tokens(ours: &str, theirs: &str, what: &str) {
    let squash = |t: &str| t.chars().filter(|c| !c.is_whitespace()).collect::<String>();
    let theirs = squash(theirs)
        .replace("0x-", "-0x")
        .replace("constCONSTSTOREconstCONSTSTORE", "constCONSTSTORE");
    let (a, b): (Vec<char>, Vec<char>) = (squash(ours).chars().collect(), theirs.chars().collect());
    if a == b {
        return;
    }
    let at = a.iter().zip(&b).position(|(x, y)| x != y).unwrap_or(a.len().min(b.len()));
    let ctx = |v: &[char]| -> String { v[at.saturating_sub(120)..(at + 120).min(v.len())].iter().collect() };
    panic!("{what} differs at squashed offset {at}\n ours:   {}\n theirs: {}", ctx(&a), ctx(&b));
}
