// SPDX-License-Identifier: MIT OR Apache-2.0

use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Result, anyhow, bail};
use canopen_dictator_core::project;

#[cfg(feature = "gui")]
mod gui;
mod io;
#[cfg(any(feature = "gui", feature = "tui"))]
mod note;
#[cfg(feature = "gui")]
mod settings;
#[cfg(feature = "tui")]
mod tui;

const USAGE: &str = "\
dictator - CANopen object dictionary editor

Usage:
  dictator [--gui | --tui] [PROJECT]
      Open the editor, optionally loading a .objdict.toml project, or
      importing a CANopenEditor .xdd or a CanFestival .od.
      Started from a terminal it runs in the terminal; otherwise, or with
      --gui, it opens a window. --tui forces the terminal UI. The
      DICTATOR_UI environment variable (gui or tui) sets the default.

  dictator export PROJECT [--c FILE.c] [--od FILE.od] [--eds FILE.eds] [--md FILE.md]
      Generate CanFestival C (+ .h next to it), a CanFestival .od project,
      EDS and Markdown docs.

  dictator import FILE.xdd|FILE.od OUT.objdict.toml
      Convert a CANopenEditor XDD or a CanFestival .od into a project file.

  dictator -h | --help
      Print this help.

  dictator -V | -v | --version
      Print the version.
";

fn export(args: &[String]) -> Result<()> {
    let mut input: Option<PathBuf> = None;
    let mut outputs: Vec<(String, PathBuf)> = Vec::new();
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--c" | "--od" | "--eds" | "--md" => {
                let path = it
                    .next()
                    .ok_or_else(|| anyhow!("{arg} needs a file path"))?;
                outputs.push((arg.clone(), PathBuf::from(path)));
            }
            other if other.starts_with("--") => bail!("unknown option {other}"),
            other if input.is_none() => input = Some(PathBuf::from(other)),
            other => bail!("unexpected argument {other}"),
        }
    }
    let input = input.ok_or_else(|| anyhow!("export needs a project file"))?;
    if outputs.is_empty() {
        bail!("export needs at least one of --c, --od, --eds, --md");
    }
    // Values that won't export fail here, once, rather than as a warning
    // from the open followed by the same text as the error.
    let (od, warnings) = io::open(&input)?;
    io::check_values(&od)?;
    print_warnings(warnings);
    let project_name = input
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default();
    for (kind, path) in outputs {
        match kind.as_str() {
            "--c" => io::export_c(&od, &path)?,
            "--od" => io::export_od(&od, &path)?,
            "--eds" => io::export_eds(&od, &path)?,
            _ => io::export_markdown(&od, &path, project_name)?,
        }
        println!("wrote {}", path.display());
    }
    Ok(())
}

fn print_warnings(warnings: Vec<String>) {
    for w in warnings {
        eprintln!("warning: {w}");
    }
}

/// `io::open`, printing import warnings to stderr.
fn open(path: &Path) -> Result<canopen_dictator_core::model::ObjectDictionary> {
    let (od, warnings) = io::open(path)?;
    print_warnings(warnings);
    Ok(od)
}

fn import(args: &[String]) -> Result<()> {
    let [input, output] = args else {
        bail!("import needs an input .xdd or .od and an output .objdict.toml");
    };
    let od = open(Path::new(input))?;
    let output_path = Path::new(output);
    io::create_parent(output_path)?;
    project::save(&od, output_path)?;
    println!("wrote {output}");
    Ok(())
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Ui {
    Gui,
    Tui,
}

impl Ui {
    fn parse(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "gui" => Some(Ui::Gui),
            "tui" => Some(Ui::Tui),
            _ => None,
        }
    }

    fn built(self) -> bool {
        match self {
            Ui::Gui => cfg!(feature = "gui"),
            Ui::Tui => cfg!(feature = "tui"),
        }
    }
}

/// The front end to start: the one asked for with `--gui` / `--tui`, else
/// `DICTATOR_UI`, else the terminal UI when both stdin and stdout are a
/// terminal. An app started from Finder, the Dock or a desktop launcher has
/// no terminal and gets the window. When only one front end is built, the
/// automatic choice falls back to it; an explicit request does not.
fn choose_ui(flag: Option<Ui>) -> Result<Ui> {
    let env = std::env::var("DICTATOR_UI")
        .ok()
        .and_then(|v| Ui::parse(&v));
    let explicit = flag.or(env);
    let ui = explicit.unwrap_or_else(|| {
        if std::io::stdin().is_terminal() && std::io::stdout().is_terminal() {
            Ui::Tui
        } else {
            Ui::Gui
        }
    });
    if ui.built() {
        return Ok(ui);
    }
    let other = if ui == Ui::Gui { Ui::Tui } else { Ui::Gui };
    match explicit {
        None if other.built() => Ok(other),
        _ => bail!(
            "this build of dictator has no {} user interface",
            if ui == Ui::Gui { "window" } else { "terminal" }
        ),
    }
}

/// Open the editor: `[--gui | --tui] [PROJECT]`.
fn edit(args: &[String]) -> Result<()> {
    let mut flag = None;
    let mut project = None;
    for arg in args {
        match arg.as_str() {
            "--gui" => flag = Some(Ui::Gui),
            "--tui" => flag = Some(Ui::Tui),
            other if other.starts_with('-') => bail!("unknown option {other}\n\n{USAGE}"),
            other if project.is_none() => project = Some(PathBuf::from(other)),
            other => bail!("unexpected argument {other}\n\n{USAGE}"),
        }
    }
    match choose_ui(flag)? {
        #[cfg(feature = "gui")]
        Ui::Gui => gui::run(project),
        #[cfg(feature = "tui")]
        Ui::Tui => tui::run(project),
        #[allow(unreachable_patterns)]
        _ => unreachable!("choose_ui only returns a front end that is built"),
    }
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
        Some("-V" | "-v" | "--version") => {
            println!("dictator {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        _ => edit(&args),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::FAILURE
        }
    }
}
