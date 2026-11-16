// SPDX-License-Identifier: MIT OR Apache-2.0

//! File-level operations shared by the GUI and the command line.

use std::path::Path;

use anyhow::{Context, Result, anyhow};

use dictator_canfestival as canfestival;
use dictator_core::model::ObjectDictionary;
use dictator_core::{export, project, xdd};

/// File extensions (lowercase) that are imported rather than opened.
pub const IMPORT_EXTENSIONS: [&str; 4] = ["xdd", "xdc", "xpd", "od"];

fn extension(path: &Path) -> String {
    path.extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default()
}

/// XDD or CanFestival `.od`: becomes a new, unsaved project.
pub fn is_import(path: &Path) -> bool {
    IMPORT_EXTENSIONS.contains(&extension(path).as_str())
}

/// Open a native project or import an XDD / `.od`, by file extension.
/// Also returns what an import could not carry over.
pub fn open(path: &Path) -> Result<(ObjectDictionary, Vec<String>)> {
    match extension(path).as_str() {
        "od" => canfestival::od::load(path).map(|i| (i.od, i.warnings)),
        "xdd" | "xdc" | "xpd" => Ok((xdd::load(path)?, Vec::new())),
        _ => Ok((project::load(path)?, Vec::new())),
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

/// CanFestival `.od` project, for objdictedit.
pub fn export_od(od: &ObjectDictionary, path: &Path) -> Result<()> {
    write(path, canfestival::od::generate(od)?)
}

pub fn export_eds(od: &ObjectDictionary, path: &Path) -> Result<()> {
    write(path, export::eds::generate(od, &file_name(path)?))
}

/// `project_file` is the name shown in the document header.
pub fn export_markdown(od: &ObjectDictionary, path: &Path, project_file: &str) -> Result<()> {
    write(path, export::markdown::generate(od, project_file))
}
