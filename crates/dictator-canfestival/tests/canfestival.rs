// SPDX-License-Identifier: LGPL-2.1-or-later
// Part of the Rust port of CanFestival objdictgen; see lib.rs for copyright.

mod common;

use dictator_canfestival::gen_c;

/// `ObjDict.c/.h` were made by CanFestival's gen_cfile from `example.od`,
/// which objdictedit imported from `example.eds` (see assets/example).
#[test]
fn c_and_header_match_canfestival_output() {
    let (c, h) = gen_c::generate(&common::canfestival_project(), "ObjDict.h").unwrap();
    common::assert_same_tokens(&h, &common::read_asset("ObjDict.h"), "ObjDict.h");
    common::assert_same_tokens(&c, &common::read_asset("ObjDict.c"), "ObjDict.c");
}
