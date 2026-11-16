// SPDX-License-Identifier: MIT OR Apache-2.0

//! File-level operations shared by the GUI and the command line.

use std::path::Path;

use anyhow::{Context, Result, anyhow};

use dictator_canfestival as canfestival;
use dictator_core::model::ObjectDictionary;
use dictator_core::{export, project, xdd};

pub fn is_xdd(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| matches!(e.to_ascii_lowercase().as_str(), "xdd" | "xdc" | "xpd"))
}

/// Open a native project or import an XDD, by file extension.
pub fn open(path: &Path) -> Result<ObjectDictionary> {
    if is_xdd(path) {
        xdd::load(path)
    } else {
        project::load(path)
    }
}

fn file_name(path: &Path) -> Result<String> {
    path.file_name()
        .and_then(|n| n.to_str())
        .map(str::to_string)
        .ok_or_else(|| anyhow!("{} has no file name", path.display()))
}

fn write(path: &Path, text: String) -> Result<()> {
    std::fs::write(path, text).with_context(|| format!("writing {}", path.display()))
}

/// Write `<path>` and the matching `.h` next to it.
pub fn export_c(od: &ObjectDictionary, path: &Path) -> Result<()> {
    let header = path.with_extension("h");
    let (c, h) = canfestival::gen_c::generate(od, &file_name(&header)?)?;
    write(path, c)?;
    write(&header, h)
}

pub fn export_eds(od: &ObjectDictionary, path: &Path) -> Result<()> {
    write(path, export::eds::generate(od, &file_name(path)?))
}

/// `project_file` is the name shown in the document header.
pub fn export_markdown(od: &ObjectDictionary, path: &Path, project_file: &str) -> Result<()> {
    write(path, export::markdown::generate(od, project_file))
}
