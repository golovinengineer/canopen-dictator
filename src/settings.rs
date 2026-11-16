// SPDX-License-Identifier: MIT OR Apache-2.0

//! Per-user application settings kept between runs: a small TOML file in
//! the platform's configuration directory, organised in sections such as
//! `[window]`. Keys this version doesn't know are kept when saving, so a
//! setting added later survives a run of an older build.

use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};
use toml::{Table, Value};

pub struct Settings {
    table: Table,
}

/// `settings.toml` in the per-user configuration directory: Application
/// Support on macOS, `%APPDATA%` on Windows, `$XDG_CONFIG_HOME` (or
/// `~/.config`) elsewhere.
fn path() -> Option<PathBuf> {
    let env = |name| std::env::var_os(name).filter(|v| !v.is_empty()).map(PathBuf::from);
    let base = if cfg!(target_os = "macos") {
        env("HOME")?.join("Library/Application Support")
    } else if cfg!(windows) {
        env("APPDATA")?
    } else {
        env("XDG_CONFIG_HOME").or_else(|| Some(env("HOME")?.join(".config")))?
    };
    Some(base.join("dictator").join("settings.toml"))
}

impl Settings {
    /// The saved settings, or none when there is no file yet. A file that
    /// can't be read is reported and treated as empty: losing the settings
    /// shouldn't keep the editor from starting.
    pub fn load() -> Self {
        let table = path()
            .filter(|p| p.exists())
            .map(|p| {
                std::fs::read_to_string(&p)
                    .map_err(anyhow::Error::from)
                    .and_then(|text| Ok(text.parse::<Table>()?))
                    .unwrap_or_else(|e| {
                        eprintln!("dictator: ignoring settings in {}: {e}", p.display());
                        Table::new()
                    })
            })
            .unwrap_or_default();
        Settings { table }
    }

    pub fn save(&self) -> Result<()> {
        let path = path().ok_or_else(|| anyhow!("no configuration directory for settings"))?;
        let dir = path.parent().expect("settings path has a directory");
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        // Written aside and renamed, so a crash mid-write can't leave half a file.
        let tmp = path.with_extension("toml.tmp");
        std::fs::write(&tmp, self.table.to_string()).with_context(|| format!("writing {}", tmp.display()))?;
        std::fs::rename(&tmp, &path).with_context(|| format!("writing {}", path.display()))?;
        Ok(())
    }

    fn get(&self, section: &str, key: &str) -> Option<&Value> {
        self.table.get(section)?.as_table()?.get(key)
    }

    fn set(&mut self, section: &str, key: &str, value: Value) {
        let entry = self.table.entry(section).or_insert_with(|| Value::Table(Table::new()));
        if !entry.is_table() {
            *entry = Value::Table(Table::new());
        }
        if let Value::Table(t) = entry {
            t.insert(key.to_string(), value);
        }
    }

    /// A finite number; integers are accepted too, as people may edit the file.
    pub fn number(&self, section: &str, key: &str) -> Option<f32> {
        let n = match self.get(section, key)? {
            Value::Float(f) => *f,
            Value::Integer(i) => *i as f64,
            _ => return None,
        };
        n.is_finite().then_some(n as f32)
    }

    pub fn set_number(&mut self, section: &str, key: &str, value: f32) {
        // Whole pixels read better in the file than 351.99997.
        self.set(section, key, Value::Integer(value.round() as i64));
    }

    pub fn flag(&self, section: &str, key: &str) -> Option<bool> {
        self.get(section, key)?.as_bool()
    }

    pub fn set_flag(&mut self, section: &str, key: &str, value: bool) {
        self.set(section, key, Value::Boolean(value));
    }
}
