// SPDX-License-Identifier: LGPL-2.1-or-later
// Part of the Rust port of CanFestival objdictgen; see lib.rs for copyright.

mod common;

use canopen_dictator_canfestival::gen_c;

/// `ObjDict.c/.h` were made by CanFestival's gen_cfile from `example.od`,
/// which objdictedit imported from `example.eds` (see assets/example).
#[test]
fn c_and_header_match_canfestival_output() {
    let (c, h) = gen_c::generate(&common::canfestival_project(), "ObjDict.h").unwrap();
    common::assert_same_tokens(&h, &common::read_asset("ObjDict.h"), "ObjDict.h");
    common::assert_same_tokens(&c, &common::read_asset("ObjDict.c"), "ObjDict.c");
}

/// The checked-in example keeps a UNICODE_STRING object (0x200D), which
/// CanFestival has no C type for: the export names it and says how to get
/// past it. Disabled, the rest exports (DOMAIN 0x200E without a default is
/// fine for the generator).
#[test]
fn unicode_string_object_is_named_and_can_be_disabled() {
    let mut od =
        canopen_dictator_core::project::load(&common::asset("example.objdict.toml")).unwrap();
    let err = gen_c::generate(&od, "ObjDict.h").unwrap_err().to_string();
    assert!(
        err.contains("0x200D Demo unicode string (UNICODE_STRING)"),
        "{err}"
    );
    assert!(err.contains("disabled = true"), "{err}");
    assert!(!err.contains("!!!"), "{err}");

    od.objects.get_mut(&0x200D).unwrap().disabled = true;
    gen_c::generate(&od, "ObjDict.h").unwrap();
}
