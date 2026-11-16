// SPDX-License-Identifier: MIT OR Apache-2.0

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Result, anyhow, bail};
use dictator_core::project;

mod gui;
mod io;

const USAGE: &str = "\
dictator - CANopen object dictionary editor

Usage:
  dictator [PROJECT]
      Open the editor, optionally loading a .objdict.toml project, or
      importing a CANopenEditor .xdd or a CanFestival .od.

  dictator export PROJECT [--c FILE.c] [--eds FILE.eds] [--md FILE.md]
      Generate CanFestival C (+ .h next to it), EDS and Markdown docs.

  dictator import FILE.xdd|FILE.od OUT.objdict.toml
      Convert a CANopenEditor XDD or a CanFestival .od into a project file.
";

fn export(args: &[String]) -> Result<()> {
    let mut input: Option<PathBuf> = None;
    let mut outputs: Vec<(String, PathBuf)> = Vec::new();
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--c" | "--eds" | "--md" => {
                let path = it.next().ok_or_else(|| anyhow!("{arg} needs a file path"))?;
                outputs.push((arg.clone(), PathBuf::from(path)));
            }
            other if other.starts_with("--") => bail!("unknown option {other}"),
            other if input.is_none() => input = Some(PathBuf::from(other)),
            other => bail!("unexpected argument {other}"),
        }
    }
    let input = input.ok_or_else(|| anyhow!("export needs a project file"))?;
    if outputs.is_empty() {
        bail!("export needs at least one of --c, --eds, --md");
    }
    let od = open(&input)?;
    let project_name = input.file_name().and_then(|n| n.to_str()).unwrap_or_default();
    for (kind, path) in outputs {
        match kind.as_str() {
            "--c" => io::export_c(&od, &path)?,
            "--eds" => io::export_eds(&od, &path)?,
            _ => io::export_markdown(&od, &path, project_name)?,
        }
        println!("wrote {}", path.display());
    }
    Ok(())
}

/// `io::open`, printing import warnings to stderr.
fn open(path: &Path) -> Result<dictator_core::model::ObjectDictionary> {
    let (od, warnings) = io::open(path)?;
    for w in warnings {
        eprintln!("warning: {w}");
    }
    Ok(od)
}

fn import(args: &[String]) -> Result<()> {
    let [input, output] = args else {
        bail!("import needs an input .xdd or .od and an output .objdict.toml");
    };
    let od = open(Path::new(input))?;
    project::save(&od, Path::new(output))?;
    println!("wrote {output}");
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("export") => export(&args[1..]),
        Some("import") => import(&args[1..]),
        Some("-h" | "--help" | "help") => {
            print!("{USAGE}");
            Ok(())
        }
        Some(other) if other.starts_with('-') => Err(anyhow!("unknown option {other}\n\n{USAGE}")),
        Some(path) => gui::run(Some(PathBuf::from(path))),
        None => gui::run(None),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::FAILURE
        }
    }
}
