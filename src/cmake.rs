// SPDX-License-Identifier: MIT OR Apache-2.0

//! `dictator cmake`: the CMake package from `cmake/`, written to a per-user
//! directory and registered in CMake's user package registry, so a plain
//! `find_package(Dictator)` works however Dictator was installed.

use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;

use anyhow::{Context, Result, anyhow, bail};

const FILES: [(&str, &str); 3] = [
    ("Dictator.cmake", include_str!("../cmake/Dictator.cmake")),
    (
        "DictatorConfig.cmake",
        include_str!("../cmake/DictatorConfig.cmake"),
    ),
    (
        "DictatorConfigVersion.cmake",
        include_str!("../cmake/DictatorConfigVersion.cmake"),
    ),
];

fn home() -> Result<PathBuf> {
    std::env::var_os("HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| anyhow!("HOME is not set"))
}

/// Application Support on macOS, `$XDG_DATA_HOME` (or `~/.local/share`)
/// elsewhere.
pub fn dir() -> Result<PathBuf> {
    if cfg!(windows) {
        bail!("dictator cmake is not supported on Windows");
    }
    let base = if cfg!(target_os = "macos") {
        home()?.join("Library/Application Support")
    } else {
        match std::env::var_os("XDG_DATA_HOME").filter(|v| !v.is_empty()) {
            Some(data) => PathBuf::from(data),
            None => home()?.join(".local/share"),
        }
    };
    Ok(base.join("dictator").join("cmake"))
}

/// A fixed entry name, so installing again replaces the entry.
fn registry_entry() -> Result<PathBuf> {
    Ok(home()?.join(".cmake/packages/Dictator/dictator"))
}

/// Write the package, defaulting `DICTATOR_EXECUTABLE` to this binary, and
/// register it. Returns the files written.
pub fn install() -> Result<Vec<PathBuf>> {
    let exe = std::env::current_exe()
        .and_then(fs::canonicalize)
        .context("finding the dictator binary")?;
    let exe = exe
        .to_str()
        .ok_or_else(|| anyhow!("{} is not a UTF-8 path", exe.display()))?
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('$', "\\$");
    // find_package always runs the version file first, so the default set
    // there reaches the config file too. EXISTS also replaces a cached path
    // from an earlier install whose binary has since moved.
    let default = format!(
        "if(NOT EXISTS \"${{DICTATOR_EXECUTABLE}}\")\n  \
         set(DICTATOR_EXECUTABLE \"{exe}\" CACHE FILEPATH \"dictator executable\" FORCE)\n\
         endif()\n"
    );
    let dir = dir()?;
    let mut written = Vec::new();
    for (path, text) in FILES
        .iter()
        .map(|(name, text)| {
            let text = if *name == "DictatorConfigVersion.cmake" {
                format!("{default}{text}")
            } else {
                text.to_string()
            };
            (dir.join(name), text)
        })
        .chain([(registry_entry()?, dir.display().to_string())])
    {
        let parent = path.parent().expect("package files are in a directory");
        fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
        fs::write(&path, text).with_context(|| format!("writing {}", path.display()))?;
        written.push(path);
    }
    Ok(written)
}

/// `dictator cmake install | uninstall | dir`.
pub fn command(args: &[String]) -> Result<()> {
    match args {
        [cmd] if cmd == "install" => {
            for path in install()? {
                println!("wrote {}", path.display());
            }
        }
        [cmd] if cmd == "uninstall" => {
            let dir = dir()?;
            for path in FILES
                .iter()
                .map(|(name, _)| dir.join(name))
                .chain([registry_entry()?])
            {
                match fs::remove_file(&path) {
                    Ok(()) => println!("removed {}", path.display()),
                    Err(e) if e.kind() == ErrorKind::NotFound => {}
                    Err(e) => {
                        return Err(e).with_context(|| format!("removing {}", path.display()));
                    }
                }
            }
            match fs::remove_dir(&dir) {
                Err(e)
                    if e.kind() != ErrorKind::NotFound
                        && e.kind() != ErrorKind::DirectoryNotEmpty =>
                {
                    return Err(e).with_context(|| format!("removing {}", dir.display()));
                }
                _ => {}
            }
        }
        [cmd] if cmd == "dir" => println!("{}", dir()?.display()),
        _ => bail!("cmake needs one of install, uninstall, dir"),
    }
    Ok(())
}
