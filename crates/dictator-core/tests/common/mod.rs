// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(dead_code)]
use std::path::PathBuf;

use dictator_core::model::ObjectDictionary;

pub fn asset(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/legacy_objdict")
        .join(name)
}

pub fn read_asset(name: &str) -> String {
    std::fs::read_to_string(asset(name)).unwrap()
}

/// The sample project as imported from the CANopenEditor XDD.
pub fn sample_xdd() -> ObjectDictionary {
    dictator_core::xdd::load(&asset("legacy_objdict.xdd")).unwrap()
}
