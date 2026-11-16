// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(dead_code)]
use std::path::PathBuf;

use dictator_core::model::ObjectDictionary;
use dictator_core::project;

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
    dictator_core::xdd::load(&asset("example.xdd")).unwrap()
}

/// The checked-in project: the XDD plus CanFestival settings.
pub fn sample_project() -> ObjectDictionary {
    project::load(&asset("example.objdict.toml")).unwrap()
}
