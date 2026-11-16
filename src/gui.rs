// SPDX-License-Identifier: MIT OR Apache-2.0

//! Slint front end: pushes the model into `App` global properties and
//! applies the edits the UI reports back.
//!
// TODO: undo/redo.
// TODO: validation panel (duplicate names, PDO mapping consistency, limits).
// TODO: PDO mapping editor, standard-object templates.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use anyhow::{Result, anyhow, bail};
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};

use dictator_canfestival::ds301;
use dictator_core::model::*;
use dictator_core::project;

use crate::io;

slint::include_modules!();

#[derive(Default)]
struct State {
    od: ObjectDictionary,
    path: Option<PathBuf>,
    dirty: bool,
    selected: Option<u16>,
    /// Position in the selected object's `subs`.
    selected_sub: Option<usize>,
    filter: String,
}

struct Gui {
    ui: MainWindow,
    state: RefCell<State>,
    objects: Rc<VecModel<ObjectRow>>,
    subs: Rc<VecModel<SubRow>>,
}

fn names<T>(items: impl IntoIterator<Item = T>, f: impl Fn(T) -> &'static str) -> ModelRc<SharedString> {
    let v: Vec<SharedString> = items.into_iter().map(|i| f(i).into()).collect();
    ModelRc::new(VecModel::from(v))
}

fn parse_hex_index(text: &str) -> Result<u16> {
    let t = text.trim();
    let t = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")).unwrap_or(t);
    let v = u16::from_str_radix(t, 16).map_err(|_| anyhow!("{text:?} is not a hex index"))?;
    if v < 0x1000 {
        bail!("index 0x{v:04X} is below 0x1000 (reserved for data types)");
    }
    Ok(v)
}

fn group_title(index: u16) -> &'static str {
    match index {
        0x0000..=0x1FFF => "Communication  0x1000 - 0x1FFF",
        0x2000..=0x5FFF => "Manufacturer  0x2000 - 0x5FFF",
        _ => "Device profile  0x6000 +",
    }
}

fn object_row(index: u16, o: &Object) -> ObjectRow {
    ObjectRow {
        index: index as i32,
        index_text: format!("{index:04X}").into(),
        name: o.name.as_str().into(),
        kind: o.kind.short().to_uppercase().into(),
        disabled: o.disabled,
        header: false,
    }
}

fn sub_row(s: &SubObject) -> SubRow {
    SubRow {
        sub: s.sub as i32,
        sub_text: format!("{:02X}", s.sub).into(),
        name: s.name.as_str().into(),
        type_name: s.data_type.short().into(),
        access: s.access.short().into(),
        pdo: if s.pdo == PdoMapping::No { "" } else { s.pdo.short() }.into(),
        default_value: s.default.as_str().into(),
    }
}

fn field(key: &str, label: &str, value: String, hint: &str) -> FormField {
    FormField { key: key.into(), label: label.into(), value: value.into(), hint: hint.into() }
}

/// Keep sub 0 ("highest sub-index") in step with the subindexes present.
fn sync_highest_sub(obj: &mut Object) {
    if obj.kind == ObjectKind::Var {
        return;
    }
    let max = obj.max_sub();
    if let Some(s0) = obj.subs.iter_mut().find(|s| s.sub == 0) {
        s0.default = format!("0x{max:02X}");
    }
}

fn change_kind(obj: &mut Object, kind: ObjectKind) {
    match (obj.kind, kind) {
        (a, b) if a == b => {}
        (ObjectKind::Var, _) => {
            let mut value = obj.subs.pop().unwrap_or_else(|| SubObject::new(0, "", DataType::U32, Access::Rw));
            value.sub = 1;
            obj.subs = vec![SubObject::new(0, "Highest sub-index supported", DataType::U8, Access::Ro), value];
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

impl Gui {
    fn app(&self) -> App<'_> {
        self.ui.global::<App>()
    }

    fn status(&self, text: impl Into<SharedString>) {
        self.app().set_status(text.into());
        self.app().set_status_is_error(false);
    }

    fn error(&self, e: impl std::fmt::Display) {
        self.app().set_status(format!("{e:#}").into());
        self.app().set_status_is_error(true);
    }

    fn mark_dirty(&self) {
        self.state.borrow_mut().dirty = true;
        self.app().set_dirty(true);
    }

    // ------------------------------------------------------------ refresh

    fn refresh_title(&self) {
        let st = self.state.borrow();
        let label = st
            .path
            .as_deref()
            .and_then(Path::file_name)
            .and_then(|n| n.to_str())
            .unwrap_or("Untitled");
        self.app().set_file_label(label.into());
        self.app().set_dirty(st.dirty);
    }

    fn refresh_objects(&self) {
        let st = self.state.borrow();
        let needle = st.filter.trim().to_lowercase();
        let mut rows = Vec::new();
        let mut group = "";
        for (index, o) in &st.od.objects {
            let hit = needle.is_empty()
                || o.name.to_lowercase().contains(&needle)
                || format!("{index:04x}").contains(needle.trim_start_matches("0x"))
                || o.subs.iter().any(|s| s.name.to_lowercase().contains(&needle));
            if !hit {
                continue;
            }
            let title = group_title(*index);
            if title != group {
                group = title;
                rows.push(ObjectRow { name: title.into(), header: true, index: -1, ..Default::default() });
            }
            rows.push(object_row(*index, o));
        }
        self.objects.set_vec(rows);
        self.app().set_selected_index(st.selected.map(|i| i as i32).unwrap_or(-1));
        drop(st);
        self.reveal();
    }

    /// Scroll the selected object and subindex into view.
    fn reveal(&self) {
        let selected = self.state.borrow().selected.map(i32::from);
        let row = selected
            .and_then(|index| self.objects.iter().position(|r| !r.header && r.index == index))
            .map_or(-1, |row| row as i32);
        let app = self.app();
        app.set_selected_row(row);
        app.set_reveal_serial(app.get_reveal_serial().wrapping_add(1));
    }

    fn refresh_object(&self) {
        let st = self.state.borrow();
        let app = self.app();
        let Some((index, obj)) = st.selected.and_then(|i| st.od.objects.get(&i).map(|o| (i, o))) else {
            app.set_has_object(false);
            app.set_has_sub(false);
            self.subs.set_vec(Vec::new());
            return;
        };
        app.set_has_object(true);
        app.set_obj_is_var(obj.kind == ObjectKind::Var);
        app.set_obj_index_text(format!("{index:04X}").into());
        app.set_obj_name(obj.name.as_str().into());
        app.set_obj_kind(ObjectKind::ALL.iter().position(|k| *k == obj.kind).unwrap_or(0) as i32);
        app.set_obj_enabled(!obj.disabled);
        app.set_obj_callback(obj.callback);
        app.set_obj_description(obj.description.as_str().into());
        let note = match ds301::find(index) {
            Some(t) => format!(
                "CanFestival defines this index itself as \"{}\": names, types and access rights in the generated C come from its DS-301 table{}.",
                ds301::entry_name(t, index),
                if t.callback { ", and callbacks are always generated" } else { "" }
            ),
            None => String::new(),
        };
        app.set_obj_note(note.into());
        self.subs.set_vec(obj.subs.iter().map(sub_row).collect::<Vec<_>>());
        drop(st);
        self.refresh_sub();
        self.reveal();
    }

    fn refresh_sub(&self) {
        let st = self.state.borrow();
        let app = self.app();
        let sub = st
            .selected
            .and_then(|i| st.od.objects.get(&i))
            .and_then(|o| st.selected_sub.and_then(|p| o.subs.get(p)));
        app.set_selected_sub(st.selected_sub.map(|p| p as i32).unwrap_or(-1));
        let Some(s) = sub else {
            app.set_has_sub(false);
            return;
        };
        app.set_has_sub(true);
        app.set_sub_name(s.name.as_str().into());
        app.set_sub_type(DataType::ALL.iter().position(|t| *t == s.data_type).unwrap_or(0) as i32);
        app.set_sub_access(Access::ALL.iter().position(|t| *t == s.access).unwrap_or(0) as i32);
        app.set_sub_pdo(PdoMapping::ALL.iter().position(|t| *t == s.pdo).unwrap_or(0) as i32);
        app.set_sub_default(s.default.as_str().into());
        app.set_sub_min(s.min.as_str().into());
        app.set_sub_max(s.max.as_str().into());
        app.set_sub_description(s.description.as_str().into());
        app.set_sub_save(s.save);
    }

    fn refresh_settings(&self) {
        let st = self.state.borrow();
        let d = &st.od.device;
        let rates: Vec<String> = d.baud_rates.iter().map(|b| b.to_string()).collect();
        let device = vec![
            field("vendor_name", "Vendor name", d.vendor_name.clone(), ""),
            field("vendor_id", "Vendor ID", d.vendor_id.clone(), ""),
            field("product_name", "Product name", d.product_name.clone(), ""),
            field("product_id", "Product ID", d.product_id.clone(), ""),
            field("description", "Description", d.description.clone(), ""),
            field("file_version", "File version", d.file_version.clone(), ""),
            field("created_by", "Created by", d.created_by.clone(), ""),
            field("baud_rates", "Baud rates", rates.join(", "), "kbit/s, comma separated: 10, 20, 50, 125, 250, 500, 800, 1000"),
            field("granularity", "Granularity", d.granularity.to_string(), "PDO mapping granularity in bits"),
            field("rpdo_count", "RPDO count", d.rpdo_count.to_string(), ""),
            field("tpdo_count", "TPDO count", d.tpdo_count.to_string(), ""),
        ];
        let c = &st.od.canfestival;
        let cf = vec![
            field("node_name", "Node name", c.node_name.clone(), "Prefix of generated symbols, e.g. <name>_Data"),
            field("node_id", "Default node ID", c.node_id.to_string(), "Also used to evaluate $NODEID in defaults"),
            field("node_type", "Node type", c.node_type.short().to_string(), "slave or master"),
            field("default_string_size", "Default string size", c.default_string_size.to_string(), "Minimum buffer size of string objects"),
        ];
        self.app().set_device_fields(ModelRc::new(VecModel::from(device)));
        self.app().set_canfestival_fields(ModelRc::new(VecModel::from(cf)));
    }

    fn refresh_all(&self) {
        self.refresh_title();
        self.refresh_objects();
        self.refresh_object();
        self.refresh_settings();
    }

    /// Update the list row of the selected object in place.
    fn update_object_row(&self) {
        let st = self.state.borrow();
        let Some(index) = st.selected else { return };
        let Some(obj) = st.od.objects.get(&index) else { return };
        for i in 0..self.objects.row_count() {
            if self.objects.row_data(i).is_some_and(|r| !r.header && r.index == index as i32) {
                self.objects.set_row_data(i, object_row(index, obj));
            }
        }
    }

    // ------------------------------------------------------------ editing

    fn select_object(&self, index: Option<u16>) {
        {
            let mut st = self.state.borrow_mut();
            st.selected = index.filter(|i| st.od.objects.contains_key(i));
            st.selected_sub = st.selected.map(|_| 0);
        }
        self.app().set_selected_index(index.map(|i| i as i32).unwrap_or(-1));
        self.refresh_object();
    }

    fn with_object(&self, f: impl FnOnce(&mut Object) -> Result<()>) -> Result<()> {
        let mut st = self.state.borrow_mut();
        let index = st.selected.ok_or_else(|| anyhow!("no object selected"))?;
        let obj = st.od.objects.get_mut(&index).ok_or_else(|| anyhow!("no object selected"))?;
        f(obj)
    }

    fn object_edited(&self, key: &str, value: &str) -> Result<()> {
        match key {
            "index" => {
                let new = parse_hex_index(value)?;
                {
                    let mut st = self.state.borrow_mut();
                    let old = st.selected.ok_or_else(|| anyhow!("no object selected"))?;
                    if new != old {
                        if st.od.objects.contains_key(&new) {
                            bail!("index 0x{new:04X} is already used");
                        }
                        let obj = st.od.objects.remove(&old).ok_or_else(|| anyhow!("no object selected"))?;
                        st.od.objects.insert(new, obj);
                        st.selected = Some(new);
                    }
                }
                self.refresh_objects();
                self.refresh_object();
            }
            "name" => {
                self.with_object(|o| {
                    o.name = value.to_string();
                    if o.kind == ObjectKind::Var
                        && let Some(s) = o.subs.first_mut() {
                            s.name = value.to_string();
                        }
                    Ok(())
                })?;
                self.update_object_row();
                let st = self.state.borrow();
                if let Some(o) = st.selected.and_then(|i| st.od.objects.get(&i))
                    && o.kind == ObjectKind::Var {
                        self.subs.set_row_data(0, sub_row(&o.subs[0]));
                        self.app().set_sub_name(value.into());
                    }
            }
            "kind" => {
                let kind = ObjectKind::from_short(value).ok_or_else(|| anyhow!("unknown kind {value}"))?;
                self.with_object(|o| {
                    change_kind(o, kind);
                    Ok(())
                })?;
                self.state.borrow_mut().selected_sub = Some(0);
                self.update_object_row();
                self.refresh_object();
            }
            "enabled" => {
                self.with_object(|o| {
                    o.disabled = value != "true";
                    Ok(())
                })?;
                self.update_object_row();
            }
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
        self.mark_dirty();
        Ok(())
    }

    fn sub_edited(&self, key: &str, value: &str) -> Result<()> {
        let pos = self.state.borrow().selected_sub.ok_or_else(|| anyhow!("no subindex selected"))?;
        let mut row = None;
        self.with_object(|o| {
            let is_var = o.kind == ObjectKind::Var;
            let s = o.subs.get_mut(pos).ok_or_else(|| anyhow!("no subindex selected"))?;
            match key {
                "name" if !is_var => s.name = value.to_string(),
                "name" => {}
                "type" => {
                    s.data_type = DataType::ALL
                        .into_iter()
                        .find(|t| t.cia_name() == value)
                        .ok_or_else(|| anyhow!("unknown type {value}"))?
                }
                "access" => s.access = Access::from_short(value).ok_or_else(|| anyhow!("unknown access {value}"))?,
                "pdo" => s.pdo = PdoMapping::from_short(value).ok_or_else(|| anyhow!("unknown mapping {value}"))?,
                "default" => s.default = value.to_string(),
                "min" => s.min = value.to_string(),
                "max" => s.max = value.to_string(),
                "description" => s.description = value.to_string(),
                "save" => s.save = value == "true",
                _ => bail!("unknown subindex field {key}"),
            }
            row = Some(sub_row(s));
            Ok(())
        })?;
        if let Some(row) = row {
            self.subs.set_row_data(pos, row);
        }
        self.mark_dirty();
        Ok(())
    }

    fn add_object(&self, index: &str, name: &str, kind: &str) -> Result<()> {
        let index = parse_hex_index(index)?;
        let kind = ObjectKind::from_short(kind).ok_or_else(|| anyhow!("unknown kind {kind}"))?;
        let name = if name.trim().is_empty() { "New object" } else { name.trim() };
        {
            let mut st = self.state.borrow_mut();
            if st.od.objects.contains_key(&index) {
                bail!("index 0x{index:04X} is already used");
            }
            st.od.objects.insert(index, Object::new(name, kind));
            st.filter.clear();
        }
        self.app().set_filter("".into());
        self.mark_dirty();
        self.refresh_objects();
        self.select_object(Some(index));
        self.status(format!("Added object 0x{index:04X}"));
        Ok(())
    }

    fn delete_object(&self) -> Result<()> {
        let next;
        let removed;
        {
            let mut st = self.state.borrow_mut();
            let index = st.selected.ok_or_else(|| anyhow!("no object selected"))?;
            st.od.objects.remove(&index);
            removed = index;
            next = st
                .od
                .objects
                .range(index..)
                .next()
                .or_else(|| st.od.objects.range(..index).next_back())
                .map(|(i, _)| *i);
        }
        self.mark_dirty();
        self.refresh_objects();
        self.select_object(next);
        self.status(format!("Deleted object 0x{removed:04X}"));
        Ok(())
    }

    fn add_sub(&self) -> Result<()> {
        let mut new_pos = 0;
        self.with_object(|o| {
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
            new_pos = o.subs.iter().position(|s| s.sub == sub).unwrap_or(0);
            Ok(())
        })?;
        self.state.borrow_mut().selected_sub = Some(new_pos);
        self.mark_dirty();
        self.refresh_object();
        Ok(())
    }

    fn delete_sub(&self) -> Result<()> {
        let pos = self.state.borrow().selected_sub.ok_or_else(|| anyhow!("no subindex selected"))?;
        let mut len = 0;
        self.with_object(|o| {
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
            len = o.subs.len();
            Ok(())
        })?;
        self.state.borrow_mut().selected_sub = Some(pos.min(len.saturating_sub(1)));
        self.mark_dirty();
        self.refresh_object();
        Ok(())
    }

    fn settings_edited(&self, key: &str, value: &str) -> Result<()> {
        fn num<T: std::str::FromStr>(value: &str, what: &str) -> Result<T> {
            value.trim().parse().map_err(|_| anyhow!("{what}: {value:?} is not a valid number"))
        }
        {
            let mut st = self.state.borrow_mut();
            let v = value.to_string();
            let (d, c) = {
                let od = &mut st.od;
                (&mut od.device, &mut od.canfestival)
            };
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
        }
        self.mark_dirty();
        self.status("");
        Ok(())
    }

    // -------------------------------------------------------------- files

    fn confirm_discard(&self) -> bool {
        if !self.state.borrow().dirty {
            return true;
        }
        rfd::MessageDialog::new()
            .set_title("Unsaved changes")
            .set_description("The project has unsaved changes. Discard them?")
            .set_buttons(rfd::MessageButtons::YesNo)
            .show()
            == rfd::MessageDialogResult::Yes
    }

    fn load(&self, od: ObjectDictionary, path: Option<PathBuf>, dirty: bool) {
        {
            let mut st = self.state.borrow_mut();
            let first = od.objects.keys().next().copied();
            *st = State { od, path, dirty, selected: first, selected_sub: first.map(|_| 0), filter: String::new() };
        }
        self.app().set_filter("".into());
        self.refresh_all();
    }

    fn open_path(&self, path: &Path) -> Result<()> {
        let (od, warnings) = io::open(path)?;
        let count = od.objects.len();
        if io::is_import(path) {
            // An import becomes an unsaved project; Save asks for a TOML path.
            self.load(od, None, true);
            let mut msg = format!(
                "Imported {count} objects from {}; save to create a project file",
                path.display()
            );
            if !warnings.is_empty() {
                msg += &format!(" ({} warnings: {})", warnings.len(), warnings.join("; "));
            }
            self.status(msg);
        } else {
            self.load(od, Some(path.to_path_buf()), false);
            self.status(format!("Opened {} ({count} objects)", path.display()));
        }
        Ok(())
    }

    fn project_dir(&self) -> Option<PathBuf> {
        self.state.borrow().path.as_deref().and_then(Path::parent).map(Path::to_path_buf)
    }

    fn dialog(&self) -> rfd::FileDialog {
        match self.project_dir() {
            Some(dir) => rfd::FileDialog::new().set_directory(dir),
            None => rfd::FileDialog::new(),
        }
    }

    fn open_project(&self) -> Result<()> {
        if !self.confirm_discard() {
            return Ok(());
        }
        let picked = self
            .dialog()
            .add_filter("Object dictionary", &["toml", "xdd", "xdc", "od"])
            .pick_file();
        match picked {
            Some(path) => self.open_path(&path),
            None => Ok(()),
        }
    }

    fn save_to(&self, path: PathBuf) -> Result<()> {
        project::save(&self.state.borrow().od, &path)?;
        {
            let mut st = self.state.borrow_mut();
            st.path = Some(path.clone());
            st.dirty = false;
        }
        self.refresh_title();
        self.status(format!("Saved {}", path.display()));
        Ok(())
    }

    fn save_project_as(&self) -> Result<()> {
        let suggested = {
            let st = self.state.borrow();
            match st.path.as_deref().and_then(Path::file_name).and_then(|n| n.to_str()) {
                Some(n) => n.to_string(),
                None => format!("project.{}", project::EXTENSION),
            }
        };
        match self
            .dialog()
            .add_filter("Object dictionary project", &["toml"])
            .set_file_name(suggested)
            .save_file()
        {
            Some(path) => self.save_to(path),
            None => Ok(()),
        }
    }

    fn save_project(&self) -> Result<()> {
        let path = self.state.borrow().path.clone();
        match path {
            Some(path) => self.save_to(path),
            None => self.save_project_as(),
        }
    }

    /// Ask for a target and run an exporter.
    // TODO: remember export targets in the project so re-export is one click.
    fn export(&self, what: &str, default_name: &str, ext: &str, run: impl FnOnce(&ObjectDictionary, &Path) -> Result<()>) -> Result<()> {
        let Some(path) = self.dialog().add_filter(what, &[ext]).set_file_name(default_name).save_file() else {
            return Ok(());
        };
        run(&self.state.borrow().od, &path)?;
        self.status(format!("Exported {what} to {}", path.display()));
        Ok(())
    }

    fn project_stem(&self) -> String {
        let st = self.state.borrow();
        st.path
            .as_deref()
            .and_then(Path::file_name)
            .and_then(|n| n.to_str())
            .map(|n| n.trim_end_matches(&format!(".{}", project::EXTENSION)).trim_end_matches(".toml").to_string())
            .unwrap_or_else(|| "ObjDict".to_string())
    }
}

/// Run a handler and show its error in the status bar.
fn report(gui: &Gui, result: Result<()>) {
    if let Err(e) = result {
        gui.error(e);
    }
}

pub fn run(initial: Option<PathBuf>) -> Result<()> {
    let ui = MainWindow::new()?;
    let gui = Rc::new(Gui {
        ui,
        state: RefCell::new(State::default()),
        objects: Rc::new(VecModel::default()),
        subs: Rc::new(VecModel::default()),
    });
    let app = gui.app();
    app.set_objects(gui.objects.clone().into());
    app.set_subs(gui.subs.clone().into());
    app.set_version(env!("CARGO_PKG_VERSION").into());
    app.set_kind_names(names(ObjectKind::ALL, ObjectKind::short));
    app.set_type_names(names(DataType::ALL, DataType::cia_name));
    app.set_access_names(names(Access::ALL, Access::short));
    app.set_pdo_names(names(PdoMapping::ALL, PdoMapping::short));

    gui.load(ObjectDictionary::new_default(), None, false);
    gui.status("New project");
    if let Some(path) = initial {
        report(&gui, gui.open_path(&path));
    }

    // Each handler holds a weak reference so the window can be dropped.
    macro_rules! on {
        ($register:ident, |$g:ident $(, $arg:ident)*| $body:expr) => {{
            let weak = Rc::downgrade(&gui);
            app.$register(move |$($arg),*| {
                if let Some($g) = weak.upgrade() {
                    let result: Result<()> = $body;
                    report(&$g, result);
                }
            });
        }};
    }

    on!(on_new_project, |g| {
        if g.confirm_discard() {
            g.load(ObjectDictionary::new_default(), None, false);
            g.status("New project");
        }
        Ok(())
    });
    on!(on_open_project, |g| g.open_project());
    on!(on_save_project, |g| g.save_project());
    on!(on_save_project_as, |g| g.save_project_as());
    on!(on_export_c, |g| g.export("CanFestival C code", "ObjDict.c", "c", io::export_c));
    on!(on_export_od, |g| g.export("CanFestival project", "ObjDict.od", "od", io::export_od));
    on!(on_export_eds, |g| {
        let name = format!("{}.eds", g.project_stem());
        g.export("EDS", &name, "eds", io::export_eds)
    });
    on!(on_export_docs, |g| {
        let stem = g.project_stem();
        let project = g.app().get_file_label().to_string();
        g.export("documentation", &format!("{stem}.md"), "md", |od, path| io::export_markdown(od, path, &project))
    });
    // Hiding the only window ends `run()`.
    on!(on_quit, |g| if g.confirm_discard() { g.ui.hide().map_err(Into::into) } else { Ok(()) });
    on!(on_filter_changed, |g, text| {
        g.state.borrow_mut().filter = text.to_string();
        g.refresh_objects();
        Ok(())
    });
    on!(on_select_object, |g, index| {
        g.select_object(u16::try_from(index).ok());
        Ok(())
    });
    on!(on_add_object, |g, index, name, kind| g.add_object(&index, &name, &kind));
    on!(on_delete_object, |g| g.delete_object());
    on!(on_object_edited, |g, key, value| {
        let result = g.object_edited(&key, &value);
        if result.is_err() {
            // Put the displayed values back in line with the model.
            g.refresh_object();
        }
        result
    });
    on!(on_select_sub, |g, pos| {
        g.state.borrow_mut().selected_sub = usize::try_from(pos).ok();
        g.refresh_sub();
        Ok(())
    });
    on!(on_add_sub, |g| g.add_sub());
    on!(on_delete_sub, |g| g.delete_sub());
    on!(on_sub_edited, |g, key, value| g.sub_edited(&key, &value));
    on!(on_settings_edited, |g, key, value| g.settings_edited(&key, &value));

    let weak = Rc::downgrade(&gui);
    gui.ui.window().on_close_requested(move || match weak.upgrade() {
        Some(g) if !g.confirm_discard() => slint::CloseRequestResponse::KeepWindowShown,
        _ => slint::CloseRequestResponse::HideWindow,
    });

    gui.ui.run()?;
    Ok(())
}
