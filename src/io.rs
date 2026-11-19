// SPDX-License-Identifier: MIT OR Apache-2.0

//! File-level operations shared by the GUI, the terminal UI and the command line.

use std::path::Path;

use anyhow::{Context, Result, anyhow, bail};

use dictator_canfestival as canfestival;
use dictator_core::model::ObjectDictionary;
use dictator_core::{export, project, xdd};

/// File extensions (lowercase) that are imported rather than opened.
#[cfg(any(feature = "gui", feature = "tui"))]
pub const IMPORT_EXTENSIONS: [&str; 4] = ["xdd", "xdc", "xpd", "od"];

fn extension(path: &Path) -> String {
    path.extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default()
}

/// XDD or CanFestival `.od`: becomes a new, unsaved project.
#[cfg(any(feature = "gui", feature = "tui"))]
pub fn is_import(path: &Path) -> bool {
    IMPORT_EXTENSIONS.contains(&extension(path).as_str())
}

/// Open a native project or import an XDD / `.od`, by file extension.
/// Also returns warnings: what an import could not carry over, and the
/// values that won't export (see [`ObjectDictionary::value_problems`]).
/// Those values are kept, so they can be fixed in the editor.
pub fn open(path: &Path) -> Result<(ObjectDictionary, Vec<String>)> {
    let (od, mut warnings) = match extension(path).as_str() {
        "od" => canfestival::od::load(path).map(|i| (i.od, i.warnings))?,
        "xdd" | "xdc" | "xpd" => (xdd::load(path)?, Vec::new()),
        _ => (project::load(path)?, Vec::new()),
    };
    warnings.extend(od.value_problems().into_iter().map(|p| format!("{p}; fix it before exporting")));
    Ok((od, warnings))
}

/// Refuse to export values the exporters would write wrongly, such as a
/// default of 300 for an UNSIGNED8 (C would truncate it, EDS would carry
/// it as is).
pub fn check_values(od: &ObjectDictionary) -> Result<()> {
    let problems = od.value_problems();
    match problems.len() {
        0 => Ok(()),
        1 => bail!("not exported: {}", problems[0]),
        n => bail!("not exported, {n} values don't fit their type: {}", problems.join("; ")),
    }
}

fn file_name(path: &Path) -> Result<String> {
    path.file_name()
        .and_then(|n| n.to_str())
        .map(str::to_string)
        .ok_or_else(|| anyhow!("{} has no file name", path.display()))
}

/// Create the directories `path` will be written into, if missing (CI jobs
/// export into a fresh `generated/`).
pub fn create_parent(path: &Path) -> Result<()> {
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    Ok(())
}

/// Write an export, creating missing parent directories.
fn write(path: &Path, text: String) -> Result<()> {
    create_parent(path)?;
    std::fs::write(path, text).with_context(|| format!("writing {}", path.display()))
}

/// Write `<path>` and the matching `.h` next to it.
pub fn export_c(od: &ObjectDictionary, path: &Path) -> Result<()> {
    check_values(od)?;
    let header = path.with_extension("h");
    let (c, h) = canfestival::gen_c::generate(od, &file_name(&header)?)?;
    write(path, c)?;
    write(&header, h)
}

/// CanFestival `.od` project, for objdictedit.
pub fn export_od(od: &ObjectDictionary, path: &Path) -> Result<()> {
    check_values(od)?;
    write(path, canfestival::od::generate(od)?)
}

pub fn export_eds(od: &ObjectDictionary, path: &Path) -> Result<()> {
    check_values(od)?;
    write(path, export::eds::generate(od, &file_name(path)?))
}

/// `project_file` is the name shown in the document header.
pub fn export_markdown(od: &ObjectDictionary, path: &Path, project_file: &str) -> Result<()> {
    check_values(od)?;
    write(path, export::markdown::generate(od, project_file))
}

/// The status line after opening or importing `count` objects from `path`.
#[cfg(any(feature = "gui", feature = "tui"))]
pub fn open_message(path: &Path, count: usize, warnings: &[String]) -> String {
    let mut msg = if is_import(path) {
        format!("Imported {count} objects from {}; save to create a project file", path.display())
    } else {
        format!("Opened {} ({count} objects)", path.display())
    };
    if !warnings.is_empty() {
        msg += &format!(" ({} warnings: {})", warnings.len(), warnings.join("; "));
    }
    msg
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn asset(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/example").join(name)
    }

    #[test]
    fn example_files_open_without_value_problems() {
        for name in ["example.objdict.toml", "example.xdd", "example.od"] {
            let (_, warnings) = open(&asset(name)).unwrap();
            assert!(!warnings.iter().any(|w| w.contains("before exporting")), "{name}: {warnings:?}");
        }
    }

    #[test]
    fn values_that_dont_fit_stop_every_export_and_warn_on_open() {
        let mut od = project::load(&asset("example.objdict.toml")).unwrap();
        od.objects.get_mut(&0x1001).unwrap().subs[0].default = "300".into();

        let dir = std::env::temp_dir().join(format!("dictator-io-test-{}", std::process::id()));
        type Export = fn(&ObjectDictionary, &Path) -> Result<()>;
        let exports: [(&str, Export); 4] = [
            ("ObjDict.c", export_c),
            ("example.od", export_od),
            ("example.eds", export_eds),
            ("example.md", |od, p| export_markdown(od, p, "example.objdict.toml")),
        ];
        for (name, export) in exports {
            let path = dir.join(name);
            let err = export(&od, &path).unwrap_err();
            assert_eq!(format!("{err:#}"), "not exported: 0x1001: default 300 does not fit UNSIGNED8");
            assert!(!path.exists(), "{name} was written");
        }

        // Opening a project with such a value keeps it and says so.
        let saved = dir.join("bad.objdict.toml");
        std::fs::create_dir_all(&dir).unwrap();
        project::save(&od, &saved).unwrap();
        let (opened, warnings) = open(&saved).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(opened.objects[&0x1001].subs[0].default, "300");
        assert_eq!(warnings, ["0x1001: default 300 does not fit UNSIGNED8; fix it before exporting"]);
    }
}
