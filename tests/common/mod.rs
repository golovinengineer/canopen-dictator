#![allow(dead_code)]
use std::path::PathBuf;

use dictator::model::ObjectDictionary;

pub fn asset(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("assets/legacy_objdict")
        .join(name)
}

pub fn read_asset(name: &str) -> String {
    std::fs::read_to_string(asset(name)).unwrap()
}

/// The sample project as imported from the CANopenEditor XDD.
pub fn sample_xdd() -> ObjectDictionary {
    dictator::xdd::load(&asset("legacy_objdict.xdd")).unwrap()
}
