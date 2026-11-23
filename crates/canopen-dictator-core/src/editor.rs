// SPDX-License-Identifier: MIT OR Apache-2.0

//! The editing session behind every front end: the open dictionary, where
//! it lives on disk, what is selected, and the edits the user can make.
//! Nothing here knows about windows or terminals; the Slint GUI and the
//! terminal UI both drive an [`Editor`] and redraw from its state.
//!
//! Fields are addressed by the same string keys in every front end
//! (`"name"`, `"default"`, `"node_id"`, …) and values travel as text, as the
//! user typed them; parsing and checking happens here, once.

use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow, bail};

use crate::model::*;
use crate::project;

/// One line of the object list: a group heading or an object.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListRow {
    Header(&'static str),
    Object(u16),
}

/// A device or generator setting, for a form.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SettingField {
    pub key: &'static str,
    pub label: &'static str,
    pub value: String,
    pub hint: &'static str,
}

#[derive(Debug, Default)]
pub struct Editor {
    pub od: ObjectDictionary,
    pub path: Option<PathBuf>,
    /// The dictionary as last opened or saved; `None` when it has never
    /// been saved as a project, such as after an import.
    saved: Option<ObjectDictionary>,
    selected: Option<u16>,
    /// Position in the selected object's `subs`.
    selected_sub: Option<usize>,
    filter: String,
}

/// Parse an object index typed in hex, with or without `0x`. Indexes below
/// 0x1000 hold data type definitions and can't be edited.
pub fn parse_hex_index(text: &str) -> Result<u16> {
    let t = text.trim();
    let t = t
        .strip_prefix("0x")
        .or_else(|| t.strip_prefix("0X"))
        .unwrap_or(t);
    let v = u16::from_str_radix(t, 16).map_err(|_| anyhow!("{text:?} is not a hex index"))?;
    if v < 0x1000 {
        bail!("index 0x{v:04X} is below 0x1000 (reserved for data types)");
    }
    Ok(v)
}

/// The heading of the object list section `index` belongs to.
pub fn group_title(index: u16) -> &'static str {
    match index {
        0x0000..=0x1FFF => "Communication  0x1000 - 0x1FFF",
        0x2000..=0x5FFF => "Manufacturer  0x2000 - 0x5FFF",
        _ => "Device profile  0x6000 +",
    }
}

/// Whether `text` is accepted as a default, minimum or maximum of type `t`.
pub fn value_ok(text: &str, t: DataType) -> bool {
    check_value(text, t).is_ok()
}

/// Whether the `default`, `min` or `max` field (`key`) of a subindex with
/// these values will export: it fits type `t`, and the default is inside
/// the limits / the minimum is not above the maximum ([`check_limits`]).
/// The GUI calls it with the texts as typed.
pub fn value_field_ok(key: &str, default: &str, min: &str, max: &str, t: DataType) -> bool {
    let (text, blamed) = match key {
        "default" => (default, "default"),
        "min" => (min, "minimum"),
        "max" => (max, ""),
        _ => return true,
    };
    check_value(text, t).is_ok()
        && !check_limits(default, min, max, t)
            .iter()
            .any(|(field, _)| *field == blamed)
}

/// How a value field is called in messages.
fn value_label(key: &str) -> &'static str {
    match key {
        "min" => "Minimum",
        "max" => "Maximum",
        _ => "Default",
    }
}

/// Keep sub 0 ("highest sub-index") in step with the subindexes present.
pub fn sync_highest_sub(obj: &mut Object) {
    if obj.kind == ObjectKind::Var {
        return;
    }
    let max = obj.max_sub();
    if let Some(s0) = obj.subs.iter_mut().find(|s| s.sub == 0) {
        s0.default = format!("0x{max:02X}");
    }
}

/// Turn `obj` into a `kind`. A VAR's value becomes element 1 of an array or
/// record, and the first element of an array or record becomes the value of
/// a VAR.
pub fn change_kind(obj: &mut Object, kind: ObjectKind) {
    match (obj.kind, kind) {
        (a, b) if a == b => {}
        (ObjectKind::Var, _) => {
            let mut value = obj
                .subs
                .pop()
                .unwrap_or_else(|| SubObject::new(0, "", DataType::U32, Access::Rw));
            value.sub = 1;
            obj.subs = vec![
                SubObject::new(0, "Highest sub-index supported", DataType::U8, Access::Ro),
                value,
            ];
        }
        (_, ObjectKind::Var) => {
            let keep = if obj.subs.len() > 1 { 1 } else { 0 };
            let mut value = obj
                .subs
                .get(keep)
                .cloned()
                .unwrap_or_else(|| SubObject::new(0, "", DataType::U32, Access::Rw));
            value.sub = 0;
            value.name = obj.name.clone();
            value.description.clear();
            obj.subs = vec![value];
        }
        _ => {}
    }
    obj.kind = kind;
    sync_highest_sub(obj);
}

/// Set the device or CanFestival setting `key` from its text.
pub fn apply_setting(od: &mut ObjectDictionary, key: &str, value: &str) -> Result<()> {
    fn num<T: std::str::FromStr>(value: &str, what: &str) -> Result<T> {
        value
            .trim()
            .parse()
            .map_err(|_| anyhow!("{what}: {value:?} is not a valid number"))
    }
    let v = value.to_string();
    let (d, c) = (&mut od.device, &mut od.canfestival);
    match key {
        "vendor_name" => d.vendor_name = v,
        "vendor_id" => d.vendor_id = v,
        "product_name" => d.product_name = v,
        "product_id" => d.product_id = v,
        "description" => d.description = v,
        "file_version" => d.file_version = v,
        "created_by" => d.created_by = v,
        "baud_rates" => {
            d.baud_rates = value
                .split(',')
                .map(str::trim)
                .filter(|p| !p.is_empty())
                .map(|p| num::<u32>(p, "baud rate"))
                .collect::<Result<_>>()?
        }
        "granularity" => d.granularity = num(value, "granularity")?,
        "rpdo_count" => d.rpdo_count = num(value, "RPDO count")?,
        "tpdo_count" => d.tpdo_count = num(value, "TPDO count")?,
        "node_name" => c.node_name = v.trim().to_string(),
        "node_id" => {
            let id: u8 = num(value, "node ID")?;
            if id > 127 {
                bail!("node ID must be 0..127");
            }
            c.node_id = id;
        }
        "node_type" => {
            c.node_type = match value.trim() {
                "slave" => NodeType::Slave,
                "master" => NodeType::Master,
                other => bail!("node type must be slave or master, not {other:?}"),
            }
        }
        "default_string_size" => c.default_string_size = num(value, "default string size")?,
        _ => bail!("unknown setting {key}"),
    }
    Ok(())
}

fn field(
    key: &'static str,
    label: &'static str,
    value: String,
    hint: &'static str,
) -> SettingField {
    SettingField {
        key,
        label,
        value,
        hint,
    }
}

impl Editor {
    /// A session on `od`. `dirty` marks it as never saved, as after an
    /// import, so it counts as having unsaved changes.
    pub fn new(od: ObjectDictionary, path: Option<PathBuf>, dirty: bool) -> Self {
        let first = od.objects.keys().next().copied();
        let saved = (!dirty).then(|| od.clone());
        Editor {
            od,
            path,
            saved,
            selected: first,
            selected_sub: first.map(|_| 0),
            filter: String::new(),
        }
    }

    /// Whether the dictionary differs from the saved one. Comparing the
    /// content means an edit that changes nothing, or is undone by hand,
    /// leaves the project clean.
    pub fn dirty(&self) -> bool {
        self.saved.as_ref() != Some(&self.od)
    }

    /// The project's file name, or "Untitled".
    pub fn file_label(&self) -> &str {
        self.path
            .as_deref()
            .and_then(Path::file_name)
            .and_then(|n| n.to_str())
            .unwrap_or("Untitled")
    }

    /// The project's file name without the project extension, as a base
    /// for export file names; "ObjDict" for an unsaved project.
    pub fn project_stem(&self) -> String {
        self.path
            .as_deref()
            .and_then(Path::file_name)
            .and_then(|n| n.to_str())
            .map(|n| {
                n.trim_end_matches(&format!(".{}", project::EXTENSION))
                    .trim_end_matches(".toml")
                    .to_string()
            })
            .unwrap_or_else(|| "ObjDict".to_string())
    }

    pub fn project_dir(&self) -> Option<&Path> {
        self.path.as_deref().and_then(Path::parent)
    }

    /// Save to `path`, which becomes the project's file.
    pub fn save_to(&mut self, path: PathBuf) -> Result<()> {
        project::save(&self.od, &path)?;
        self.path = Some(path);
        self.saved = Some(self.od.clone());
        Ok(())
    }

    // ---------------------------------------------------------- selection

    pub fn selected(&self) -> Option<u16> {
        self.selected
    }

    pub fn selected_object(&self) -> Option<(u16, &Object)> {
        let index = self.selected?;
        self.od.objects.get(&index).map(|o| (index, o))
    }

    pub fn selected_sub(&self) -> Option<usize> {
        self.selected_sub
    }

    pub fn selected_sub_object(&self) -> Option<&SubObject> {
        self.selected_object()?.1.subs.get(self.selected_sub?)
    }

    /// Select the object at `index`, if there is one, and its first subindex.
    pub fn select_object(&mut self, index: Option<u16>) {
        self.selected = index.filter(|i| self.od.objects.contains_key(i));
        self.selected_sub = self.selected.map(|_| 0);
    }

    pub fn select_sub(&mut self, pos: Option<usize>) {
        self.selected_sub = pos;
    }

    pub fn filter(&self) -> &str {
        &self.filter
    }

    pub fn set_filter(&mut self, text: &str) {
        self.filter = text.to_string();
    }

    /// The object list as shown: objects matching the filter (by name, hex
    /// index or subindex name), under the heading of their range.
    pub fn visible_objects(&self) -> Vec<ListRow> {
        let needle = self.filter.trim().to_lowercase();
        let mut rows = Vec::new();
        let mut group = "";
        for (index, o) in &self.od.objects {
            let hit = needle.is_empty()
                || o.name.to_lowercase().contains(&needle)
                || format!("{index:04x}").contains(needle.trim_start_matches("0x"))
                || o.subs
                    .iter()
                    .any(|s| s.name.to_lowercase().contains(&needle));
            if !hit {
                continue;
            }
            let title = group_title(*index);
            if title != group {
                group = title;
                rows.push(ListRow::Header(title));
            }
            rows.push(ListRow::Object(*index));
        }
        rows
    }

    // ------------------------------------------------------------ editing

    fn with_object<T>(&mut self, f: impl FnOnce(&mut Object) -> Result<T>) -> Result<T> {
        let index = self.selected.ok_or_else(|| anyhow!("no object selected"))?;
        let obj = self
            .od
            .objects
            .get_mut(&index)
            .ok_or_else(|| anyhow!("no object selected"))?;
        f(obj)
    }

    /// Set a field of the selected object: `index` (hex), `name`, `kind`
    /// (`var` / `array` / `record`), `enabled`, `callback` (`true` / `false`) or
    /// `description`.
    pub fn edit_object(&mut self, key: &str, value: &str) -> Result<()> {
        match key {
            "index" => {
                let new = parse_hex_index(value)?;
                let old = self.selected.ok_or_else(|| anyhow!("no object selected"))?;
                if new != old {
                    if self.od.objects.contains_key(&new) {
                        bail!("index 0x{new:04X} is already used");
                    }
                    let obj = self
                        .od
                        .objects
                        .remove(&old)
                        .ok_or_else(|| anyhow!("no object selected"))?;
                    self.od.objects.insert(new, obj);
                    self.selected = Some(new);
                }
            }
            "name" => self.with_object(|o| {
                o.name = value.to_string();
                // A VAR's single value carries the object's name.
                if o.kind == ObjectKind::Var
                    && let Some(s) = o.subs.first_mut()
                {
                    s.name = value.to_string();
                }
                Ok(())
            })?,
            "kind" => {
                let kind =
                    ObjectKind::from_short(value).ok_or_else(|| anyhow!("unknown kind {value}"))?;
                self.with_object(|o| {
                    change_kind(o, kind);
                    Ok(())
                })?;
                self.selected_sub = Some(0);
            }
            "enabled" => self.with_object(|o| {
                o.disabled = value != "true";
                Ok(())
            })?,
            "callback" => self.with_object(|o| {
                o.callback = value == "true";
                Ok(())
            })?,
            "description" => self.with_object(|o| {
                o.description = value.to_string();
                Ok(())
            })?,
            _ => bail!("unknown object field {key}"),
        }
        Ok(())
    }

    /// Set a field of the selected subindex: `name`, `type` (CiA name such
    /// as `UNSIGNED16`), `access`, `pdo`, `default`, `min`, `max`,
    /// `description` or `save` (`true` / `false`). A VAR's name follows the
    /// object's, so `name` is ignored there.
    ///
    /// A default, minimum or maximum that won't export (see
    /// [`check_value`]) is refused and the field keeps its value. A type
    /// change is always taken, even when the values no longer fit the new
    /// type, and so is a default outside the limits (refusing it would make
    /// moving a range depend on the order of the edits);
    /// [`Editor::value_problem`] says what is wrong.
    pub fn edit_sub(&mut self, key: &str, value: &str) -> Result<()> {
        let pos = self
            .selected_sub
            .ok_or_else(|| anyhow!("no subindex selected"))?;
        self.with_object(|o| {
            let is_var = o.kind == ObjectKind::Var;
            let s = o
                .subs
                .get_mut(pos)
                .ok_or_else(|| anyhow!("no subindex selected"))?;
            match key {
                "name" if !is_var => s.name = value.to_string(),
                "name" => {}
                "type" => {
                    s.data_type = DataType::ALL
                        .into_iter()
                        .find(|t| t.cia_name() == value)
                        .ok_or_else(|| anyhow!("unknown type {value}"))?
                }
                "access" => {
                    s.access = Access::from_short(value)
                        .ok_or_else(|| anyhow!("unknown access {value}"))?
                }
                "pdo" => {
                    s.pdo = PdoMapping::from_short(value)
                        .ok_or_else(|| anyhow!("unknown mapping {value}"))?
                }
                "default" | "min" | "max" => {
                    if let Err(why) = check_value(value, s.data_type) {
                        bail!("{} not changed: {why}", value_label(key));
                    }
                    let field = match key {
                        "default" => &mut s.default,
                        "min" => &mut s.min,
                        _ => &mut s.max,
                    };
                    *field = value.to_string();
                }
                "description" => s.description = value.to_string(),
                "save" => s.save = value == "true",
                _ => bail!("unknown subindex field {key}"),
            }
            Ok(())
        })
    }

    /// What is wrong with the default, minimum and maximum of the selected
    /// subindex, for the status bar; `None` when they will export. Values
    /// that don't fit the type come first (a default of 300 after the type
    /// became `UNSIGNED8`), then a default outside the limits. Front ends
    /// show it after editing the type or one of the values.
    pub fn value_problem(&self) -> Option<String> {
        let s = self.selected_sub_object()?;
        let bad: Vec<String> = s
            .bad_values()
            .into_iter()
            .map(|(field, v, _)| format!("{field} {v}"))
            .collect();
        let (what, it) = if !bad.is_empty() {
            let (verb, it) = if bad.len() == 1 {
                ("does", "it")
            } else {
                ("do", "them")
            };
            (
                format!(
                    "{} {verb} not fit {}",
                    bad.join(", "),
                    s.data_type.cia_name()
                ),
                it,
            )
        } else {
            let limits: Vec<String> = s
                .limit_problems()
                .into_iter()
                .map(|(field, why)| format!("{field} {why}"))
                .collect();
            if limits.is_empty() {
                return None;
            }
            (
                limits.join(", "),
                if limits.len() == 1 { "it" } else { "them" },
            )
        };
        Some(format!(
            "{}{}; fix {it} before exporting",
            what[..1].to_uppercase(),
            &what[1..]
        ))
    }

    /// Whether `text` in the selected subindex's `default`, `min` or `max`
    /// field, with the other two as stored, will export; drives the red
    /// field. Unlike [`Editor::edit_sub`], this also refuses a default
    /// outside the limits and a minimum above the maximum.
    pub fn value_field_ok(&self, key: &str, text: &str) -> bool {
        self.selected_sub_object().is_none_or(|s| {
            let mut values = [s.default.as_str(), s.min.as_str(), s.max.as_str()];
            match key {
                "default" => values[0] = text,
                "min" => values[1] = text,
                "max" => values[2] = text,
                _ => return true,
            }
            let [default, min, max] = values;
            value_field_ok(key, default, min, max, s.data_type)
        })
    }

    /// Add an object and select it. An empty name becomes "New object";
    /// the filter is cleared so the new object shows.
    pub fn add_object(&mut self, index: &str, name: &str, kind: &str) -> Result<u16> {
        let index = parse_hex_index(index)?;
        let kind = ObjectKind::from_short(kind).ok_or_else(|| anyhow!("unknown kind {kind}"))?;
        let name = if name.trim().is_empty() {
            "New object"
        } else {
            name.trim()
        };
        if self.od.objects.contains_key(&index) {
            bail!("index 0x{index:04X} is already used");
        }
        self.od.objects.insert(index, Object::new(name, kind));
        self.filter.clear();
        self.select_object(Some(index));
        Ok(index)
    }

    /// The first free index from `from` up, for suggesting a new object.
    pub fn free_index(&self, from: u16) -> Option<u16> {
        (from.max(0x1000)..=u16::MAX).find(|i| !self.od.objects.contains_key(i))
    }

    /// Delete the selected object and select its neighbour. Returns the
    /// deleted index.
    pub fn delete_object(&mut self) -> Result<u16> {
        let index = self.selected.ok_or_else(|| anyhow!("no object selected"))?;
        self.od.objects.remove(&index);
        let next = self
            .od
            .objects
            .range(index..)
            .next()
            .or_else(|| self.od.objects.range(..index).next_back())
            .map(|(i, _)| *i);
        self.select_object(next);
        Ok(index)
    }

    /// Add a subindex after the last one of the selected array or record
    /// and select it. Array elements copy the previous element.
    pub fn add_sub(&mut self) -> Result<()> {
        let new_pos = self.with_object(|o| {
            if o.kind == ObjectKind::Var {
                bail!("a VAR has no subindexes; change its kind first");
            }
            let max = o.max_sub();
            if max == 0xFE {
                bail!("no free subindex left");
            }
            let sub = if o.subs.is_empty() { 0 } else { max + 1 };
            // New entries of an array follow the previous element.
            let template = o.subs.iter().rev().find(|s| s.sub != 0).cloned();
            let mut s = match (o.kind, template) {
                (ObjectKind::Array, Some(mut t)) => {
                    t.description.clear();
                    t
                }
                _ => SubObject::new(sub, "", DataType::U8, Access::Rw),
            };
            s.sub = sub;
            if o.kind != ObjectKind::Array || s.name.is_empty() {
                s.name = format!("Sub {sub}");
            }
            if s.default.is_empty() {
                s.default = "0".into();
            }
            o.subs.push(s);
            o.subs.sort_by_key(|s| s.sub);
            sync_highest_sub(o);
            Ok(o.subs.iter().position(|s| s.sub == sub).unwrap_or(0))
        })?;
        self.selected_sub = Some(new_pos);
        Ok(())
    }

    /// Delete the selected subindex; sub 0 stays.
    pub fn delete_sub(&mut self) -> Result<()> {
        let pos = self
            .selected_sub
            .ok_or_else(|| anyhow!("no subindex selected"))?;
        let len = self.with_object(|o| {
            if o.kind == ObjectKind::Var {
                bail!("a VAR has no subindexes");
            }
            if o.subs.get(pos).is_some_and(|s| s.sub == 0) {
                bail!("subindex 0 cannot be removed");
            }
            if pos < o.subs.len() {
                o.subs.remove(pos);
            }
            sync_highest_sub(o);
            Ok(o.subs.len())
        })?;
        self.selected_sub = Some(pos.min(len.saturating_sub(1)));
        Ok(())
    }

    pub fn edit_setting(&mut self, key: &str, value: &str) -> Result<()> {
        apply_setting(&mut self.od, key, value)
    }

    /// The device information fields, in form order.
    pub fn device_fields(&self) -> Vec<SettingField> {
        let d = &self.od.device;
        let rates: Vec<String> = d.baud_rates.iter().map(|b| b.to_string()).collect();
        vec![
            field("vendor_name", "Vendor name", d.vendor_name.clone(), ""),
            field("vendor_id", "Vendor ID", d.vendor_id.clone(), ""),
            field("product_name", "Product name", d.product_name.clone(), ""),
            field("product_id", "Product ID", d.product_id.clone(), ""),
            field("description", "Description", d.description.clone(), ""),
            field("file_version", "File version", d.file_version.clone(), ""),
            field("created_by", "Created by", d.created_by.clone(), ""),
            field(
                "baud_rates",
                "Baud rates",
                rates.join(", "),
                "kbit/s, comma separated: 10, 20, 50, 125, 250, 500, 800, 1000",
            ),
            field(
                "granularity",
                "Granularity",
                d.granularity.to_string(),
                "PDO mapping granularity in bits",
            ),
            field("rpdo_count", "RPDO count", d.rpdo_count.to_string(), ""),
            field("tpdo_count", "TPDO count", d.tpdo_count.to_string(), ""),
        ]
    }

    /// The CanFestival generator settings, in form order.
    pub fn canfestival_fields(&self) -> Vec<SettingField> {
        let c = &self.od.canfestival;
        vec![
            field(
                "node_name",
                "Node name",
                c.node_name.clone(),
                "Prefix of generated symbols, e.g. <name>_Data",
            ),
            field(
                "node_id",
                "Default node ID",
                c.node_id.to_string(),
                "Also used to evaluate $NODEID in defaults",
            ),
            field(
                "node_type",
                "Node type",
                c.node_type.short().to_string(),
                "slave or master",
            ),
            field(
                "default_string_size",
                "Default string size",
                c.default_string_size.to_string(),
                "Minimum buffer size of string objects",
            ),
        ]
    }

    // --------------------------------------------------------- validation

    /// Whether `text` can become the index of the object at `current`
    /// (`None` for a new object): a free hex index from 0x1000.
    pub fn index_ok(&self, text: &str, current: Option<u16>) -> bool {
        let Ok(index) = parse_hex_index(text) else {
            return false;
        };
        Some(index) == current || !self.od.objects.contains_key(&index)
    }

    /// Whether `value` is accepted for the setting `key`.
    pub fn setting_ok(&self, key: &str, value: &str) -> bool {
        let mut od = self.od.clone();
        apply_setting(&mut od, key, value).is_ok()
    }
}
