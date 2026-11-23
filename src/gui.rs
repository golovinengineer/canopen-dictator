// SPDX-License-Identifier: MIT OR Apache-2.0

//! Slint front end: pushes the editing session into `App` global
//! properties and hands the edits the UI reports back to the [`Editor`].
//!
// TODO: undo/redo.
// TODO: validation panel (duplicate names, PDO mapping consistency, limits).
// TODO: PDO mapping editor, standard-object templates.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use anyhow::Result;
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};

use canopen_dictator_core::editor::{self, Editor, ListRow, SettingField};
use canopen_dictator_core::model::*;
use canopen_dictator_core::project;

use crate::io;
use crate::note::object_note;
use crate::settings::Settings;

slint::include_modules!();

struct Gui {
    ui: MainWindow,
    editor: RefCell<Editor>,
    objects: Rc<VecModel<ObjectRow>>,
    subs: Rc<VecModel<SubRow>>,
    settings: RefCell<Settings>,
}

fn names<T>(
    items: impl IntoIterator<Item = T>,
    f: impl Fn(T) -> &'static str,
) -> ModelRc<SharedString> {
    let v: Vec<SharedString> = items.into_iter().map(|i| f(i).into()).collect();
    ModelRc::new(VecModel::from(v))
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
        pdo: if s.pdo == PdoMapping::No {
            ""
        } else {
            s.pdo.short()
        }
        .into(),
        default_value: s.default.as_str().into(),
    }
}

fn form(fields: Vec<SettingField>) -> ModelRc<FormField> {
    let rows: Vec<FormField> = fields
        .into_iter()
        .map(|f| FormField {
            key: f.key.into(),
            label: f.label.into(),
            value: f.value.into(),
            hint: f.hint.into(),
        })
        .collect();
    ModelRc::new(VecModel::from(rows))
}

impl Gui {
    fn app(&self) -> App<'_> {
        self.ui.global::<App>()
    }

    /// Bring back the window size and pane widths of the last run. The UI
    /// clamps the pane widths to their limits, so no checks are needed here.
    fn restore_layout(&self) {
        let s = self.settings.borrow();
        let app = self.app();
        if let Some(w) = s.number("panes", "list-width") {
            app.set_wanted_list_width(w);
        }
        if let Some(w) = s.number("panes", "inspector-width") {
            app.set_wanted_inspector_width(w);
        }
        let window = self.ui.window();
        if let (Some(w), Some(h)) = (s.number("window", "width"), s.number("window", "height"))
            && w > 0.0
            && h > 0.0
        {
            // The window's minimum size still applies.
            window.set_size(slint::LogicalSize::new(w, h));
        }
        if s.flag("window", "maximized") == Some(true) {
            window.set_maximized(true);
        }
    }

    /// Remember the window size and pane widths for the next run. Called
    /// while the window is still shown, as it is about to close.
    fn save_layout(&self) {
        let mut s = self.settings.borrow_mut();
        let app = self.app();
        s.set_number("panes", "list-width", app.get_wanted_list_width());
        s.set_number("panes", "inspector-width", app.get_wanted_inspector_width());
        let window = self.ui.window();
        let maximized = window.is_maximized();
        // A maximized or full-screen window is the size of the screen; keep
        // the size it goes back to instead.
        if !maximized && !window.is_fullscreen() {
            let size = window.size().to_logical(window.scale_factor());
            s.set_number("window", "width", size.width);
            s.set_number("window", "height", size.height);
        }
        s.set_flag("window", "maximized", maximized);
        if let Err(e) = s.save() {
            eprintln!("dictator: settings not saved: {e:#}");
        }
    }

    fn status(&self, text: impl Into<SharedString>) {
        self.app().set_status(text.into());
        self.app().set_status_is_error(false);
    }

    fn error(&self, e: impl std::fmt::Display) {
        self.app().set_status(format!("{e:#}").into());
        self.app().set_status_is_error(true);
    }

    /// Show in the title whether the dictionary has unsaved changes.
    fn refresh_dirty(&self) {
        let dirty = self.editor.borrow().dirty();
        self.app().set_dirty(dirty);
    }

    // ------------------------------------------------------------ refresh

    fn refresh_title(&self) {
        let ed = self.editor.borrow();
        self.app().set_file_label(ed.file_label().into());
        self.app().set_dirty(ed.dirty());
    }

    fn refresh_objects(&self) {
        let ed = self.editor.borrow();
        let rows: Vec<ObjectRow> = ed
            .visible_objects()
            .into_iter()
            .map(|row| match row {
                ListRow::Header(title) => ObjectRow {
                    name: title.into(),
                    header: true,
                    index: -1,
                    ..Default::default()
                },
                ListRow::Object(index) => object_row(index, &ed.od.objects[&index]),
            })
            .collect();
        self.objects.set_vec(rows);
        self.app()
            .set_selected_index(ed.selected().map(i32::from).unwrap_or(-1));
        drop(ed);
        self.reveal();
    }

    /// Scroll the selected object and subindex into view.
    fn reveal(&self) {
        let selected = self.editor.borrow().selected().map(i32::from);
        let row = selected
            .and_then(|index| {
                self.objects
                    .iter()
                    .position(|r| !r.header && r.index == index)
            })
            .map_or(-1, |row| row as i32);
        let app = self.app();
        app.set_selected_row(row);
        app.set_reveal_serial(app.get_reveal_serial().wrapping_add(1));
    }

    fn refresh_object(&self) {
        let ed = self.editor.borrow();
        let app = self.app();
        app.set_selected_index(ed.selected().map(i32::from).unwrap_or(-1));
        let Some((index, obj)) = ed.selected_object() else {
            app.set_has_object(false);
            app.set_has_sub(false);
            self.subs.set_vec(Vec::new());
            return;
        };
        app.set_has_object(true);
        app.set_obj_is_var(obj.kind == ObjectKind::Var);
        app.set_obj_index_text(format!("{index:04X}").into());
        app.set_obj_name(obj.name.as_str().into());
        app.set_obj_kind(
            ObjectKind::ALL
                .iter()
                .position(|k| *k == obj.kind)
                .unwrap_or(0) as i32,
        );
        app.set_obj_enabled(!obj.disabled);
        app.set_obj_callback(obj.callback);
        app.set_obj_description(obj.description.as_str().into());
        app.set_obj_note(object_note(index).into());
        self.subs
            .set_vec(obj.subs.iter().map(sub_row).collect::<Vec<_>>());
        drop(ed);
        self.refresh_sub();
        self.reveal();
    }

    fn refresh_sub(&self) {
        let ed = self.editor.borrow();
        let app = self.app();
        app.set_selected_sub(ed.selected_sub().map(|p| p as i32).unwrap_or(-1));
        let Some(s) = ed.selected_sub_object() else {
            app.set_has_sub(false);
            return;
        };
        app.set_has_sub(true);
        app.set_sub_name(s.name.as_str().into());
        app.set_sub_type(
            DataType::ALL
                .iter()
                .position(|t| *t == s.data_type)
                .unwrap_or(0) as i32,
        );
        app.set_sub_access(Access::ALL.iter().position(|t| *t == s.access).unwrap_or(0) as i32);
        app.set_sub_pdo(
            PdoMapping::ALL
                .iter()
                .position(|t| *t == s.pdo)
                .unwrap_or(0) as i32,
        );
        app.set_sub_default(s.default.as_str().into());
        app.set_sub_min(s.min.as_str().into());
        app.set_sub_max(s.max.as_str().into());
        app.set_sub_description(s.description.as_str().into());
        app.set_sub_save(s.save);
    }

    fn refresh_settings(&self) {
        let ed = self.editor.borrow();
        self.app().set_device_fields(form(ed.device_fields()));
        self.app()
            .set_canfestival_fields(form(ed.canfestival_fields()));
    }

    fn refresh_all(&self) {
        self.refresh_title();
        self.refresh_objects();
        self.refresh_object();
        self.refresh_settings();
    }

    /// Update the list row of the selected object in place.
    fn update_object_row(&self) {
        let ed = self.editor.borrow();
        let Some((index, obj)) = ed.selected_object() else {
            return;
        };
        for i in 0..self.objects.row_count() {
            if self
                .objects
                .row_data(i)
                .is_some_and(|r| !r.header && r.index == index as i32)
            {
                self.objects.set_row_data(i, object_row(index, obj));
            }
        }
    }

    /// Update the table row of the selected subindex in place.
    fn update_sub_row(&self) {
        let ed = self.editor.borrow();
        if let (Some(pos), Some(s)) = (ed.selected_sub(), ed.selected_sub_object()) {
            self.subs.set_row_data(pos, sub_row(s));
        }
    }

    // ------------------------------------------------------------ editing

    fn object_edited(&self, key: &str, value: &str) -> Result<()> {
        self.editor.borrow_mut().edit_object(key, value)?;
        match key {
            "index" => {
                self.refresh_objects();
                self.refresh_object();
            }
            "name" => {
                self.update_object_row();
                // A VAR's value is named after the object.
                if self
                    .editor
                    .borrow()
                    .selected_object()
                    .is_some_and(|(_, o)| o.kind == ObjectKind::Var)
                {
                    self.update_sub_row();
                    self.app().set_sub_name(value.into());
                }
            }
            "kind" => {
                self.update_object_row();
                self.refresh_object();
            }
            "enabled" => self.update_object_row(),
            _ => {}
        }
        self.refresh_dirty();
        Ok(())
    }

    fn sub_edited(&self, key: &str, value: &str) -> Result<()> {
        // A refused default/min/max leaves the typed text in its field, with
        // the red outline and the reason in the status bar, so the user can
        // finish typing; leaving the field puts the stored value back.
        self.editor.borrow_mut().edit_sub(key, value)?;
        self.update_sub_row();
        self.refresh_dirty();
        let problem = match key {
            "type" | "default" | "min" | "max" => self.editor.borrow().value_problem(),
            _ => None,
        };
        match problem {
            // The outline of a value that won't export follows the texts and
            // the type by itself; say why it turned red.
            Some(problem) => self.error(problem),
            None if self.app().get_status_is_error() => self.status(""),
            None => {}
        }
        Ok(())
    }

    /// Show the stored default, minimum and maximum again, dropping text
    /// that was refused while typing.
    fn sub_values_left(&self) {
        let ed = self.editor.borrow();
        let Some(s) = ed.selected_sub_object() else {
            return;
        };
        let app = self.app();
        app.set_sub_default(s.default.as_str().into());
        app.set_sub_min(s.min.as_str().into());
        app.set_sub_max(s.max.as_str().into());
    }

    fn add_object(&self, index: &str, name: &str, kind: &str) -> Result<()> {
        let index = self.editor.borrow_mut().add_object(index, name, kind)?;
        self.app().set_filter("".into());
        self.refresh_dirty();
        self.refresh_objects();
        self.refresh_object();
        self.status(format!("Added object 0x{index:04X}"));
        Ok(())
    }

    fn delete_object(&self) -> Result<()> {
        let removed = self.editor.borrow_mut().delete_object()?;
        self.refresh_dirty();
        self.refresh_objects();
        self.refresh_object();
        self.status(format!("Deleted object 0x{removed:04X}"));
        Ok(())
    }

    fn add_sub(&self) -> Result<()> {
        self.editor.borrow_mut().add_sub()?;
        self.refresh_dirty();
        self.refresh_object();
        Ok(())
    }

    fn delete_sub(&self) -> Result<()> {
        self.editor.borrow_mut().delete_sub()?;
        self.refresh_dirty();
        self.refresh_object();
        Ok(())
    }

    fn settings_edited(&self, key: &str, value: &str) -> Result<()> {
        self.editor.borrow_mut().edit_setting(key, value)?;
        self.refresh_dirty();
        self.status("");
        Ok(())
    }

    // -------------------------------------------------------------- files

    /// Offer to save unsaved changes before they would be lost. Returns
    /// whether to go ahead: false on Cancel, or when saving failed or the
    /// Save As dialog was cancelled.
    fn settle_unsaved(&self) -> bool {
        if !self.editor.borrow().dirty() {
            return true;
        }
        const SAVE: &str = "Save";
        const DONT_SAVE: &str = "Don't Save";
        let label = self.app().get_file_label();
        let answer = rfd::MessageDialog::new()
            .set_title("Unsaved changes")
            .set_description(format!("Save the changes to {label}?"))
            .set_buttons(rfd::MessageButtons::YesNoCancelCustom(
                SAVE.into(),
                DONT_SAVE.into(),
                "Cancel".into(),
            ))
            .show();
        match answer {
            rfd::MessageDialogResult::Custom(b) if b == SAVE => match self.save_project() {
                Ok(()) => !self.editor.borrow().dirty(),
                Err(e) => {
                    self.error(e);
                    false
                }
            },
            rfd::MessageDialogResult::Custom(b) if b == DONT_SAVE => true,
            _ => false,
        }
    }

    fn load(&self, od: ObjectDictionary, path: Option<PathBuf>, dirty: bool) {
        *self.editor.borrow_mut() = Editor::new(od, path, dirty);
        self.app().set_filter("".into());
        self.refresh_all();
    }

    fn open_path(&self, path: &Path) -> Result<()> {
        let (od, warnings) = io::open(path)?;
        let message = io::open_message(path, od.objects.len(), &warnings);
        if io::is_import(path) {
            // An import becomes an unsaved project; Save asks for a TOML path.
            self.load(od, None, true);
        } else {
            self.load(od, Some(path.to_path_buf()), false);
        }
        self.status(message);
        Ok(())
    }

    fn dialog(&self) -> rfd::FileDialog {
        match self.editor.borrow().project_dir() {
            Some(dir) => rfd::FileDialog::new().set_directory(dir),
            None => rfd::FileDialog::new(),
        }
    }

    fn open_project(&self) -> Result<()> {
        if !self.settle_unsaved() {
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
        self.editor.borrow_mut().save_to(path.clone())?;
        self.refresh_title();
        self.status(format!("Saved {}", path.display()));
        Ok(())
    }

    fn save_project_as(&self) -> Result<()> {
        let suggested = {
            let ed = self.editor.borrow();
            match ed.path {
                Some(_) => ed.file_label().to_string(),
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
        let path = self.editor.borrow().path.clone();
        match path {
            Some(path) => self.save_to(path),
            None => self.save_project_as(),
        }
    }

    /// Ask for a target and run an exporter.
    // TODO: remember export targets in the project so re-export is one click.
    fn export(
        &self,
        what: &str,
        default_name: &str,
        ext: &str,
        run: impl FnOnce(&ObjectDictionary, &Path) -> Result<()>,
    ) -> Result<()> {
        let Some(path) = self
            .dialog()
            .add_filter(what, &[ext])
            .set_file_name(default_name)
            .save_file()
        else {
            return Ok(());
        };
        run(&self.editor.borrow().od, &path)?;
        self.status(format!("Exported {what} to {}", path.display()));
        Ok(())
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
        editor: RefCell::new(Editor::default()),
        objects: Rc::new(VecModel::default()),
        subs: Rc::new(VecModel::default()),
        settings: RefCell::new(Settings::load()),
    });
    gui.restore_layout();
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
        if g.settle_unsaved() {
            g.load(ObjectDictionary::new_default(), None, false);
            g.status("New project");
        }
        Ok(())
    });
    on!(on_open_project, |g| g.open_project());
    on!(on_save_project, |g| g.save_project());
    on!(on_save_project_as, |g| g.save_project_as());
    on!(on_export_c, |g| g.export(
        "CanFestival C code",
        "ObjDict.c",
        "c",
        io::export_c
    ));
    on!(on_export_od, |g| g.export(
        "CanFestival project",
        "ObjDict.od",
        "od",
        io::export_od
    ));
    on!(on_export_eds, |g| {
        let name = format!("{}.eds", g.editor.borrow().project_stem());
        g.export("EDS", &name, "eds", io::export_eds)
    });
    on!(on_export_docs, |g| {
        let stem = g.editor.borrow().project_stem();
        let project = g.app().get_file_label().to_string();
        g.export("documentation", &format!("{stem}.md"), "md", |od, path| {
            io::export_markdown(od, path, &project)
        })
    });
    // Hiding the only window ends `run()`.
    on!(on_quit, |g| {
        if g.settle_unsaved() {
            g.save_layout();
            g.ui.hide().map_err(Into::into)
        } else {
            Ok(())
        }
    });
    on!(on_filter_changed, |g, text| {
        g.editor.borrow_mut().set_filter(&text);
        g.refresh_objects();
        Ok(())
    });
    on!(on_select_object, |g, index| {
        g.editor
            .borrow_mut()
            .select_object(u16::try_from(index).ok());
        g.refresh_object();
        Ok(())
    });
    on!(on_add_object, |g, index, name, kind| g
        .add_object(&index, &name, &kind));
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
        g.editor.borrow_mut().select_sub(usize::try_from(pos).ok());
        g.refresh_sub();
        Ok(())
    });
    on!(on_add_sub, |g| g.add_sub());
    on!(on_delete_sub, |g| g.delete_sub());
    on!(on_sub_edited, |g, key, value| g.sub_edited(&key, &value));
    on!(on_sub_value_left, |g| {
        g.sub_values_left();
        Ok(())
    });
    on!(on_settings_edited, |g, key, value| g
        .settings_edited(&key, &value));

    // Pure checks behind the red outline of fields whose text won't be accepted.
    let weak = Rc::downgrade(&gui);
    app.on_index_ok(move |text, current| {
        weak.upgrade().is_none_or(|g| {
            g.editor
                .borrow()
                .index_ok(&text, u16::try_from(current).ok())
        })
    });
    app.on_value_ok(|key, default, min, max, type_index| {
        let t = usize::try_from(type_index)
            .ok()
            .and_then(|i| DataType::ALL.get(i).copied());
        t.is_none_or(|t| editor::value_field_ok(&key, &default, &min, &max, t))
    });
    let weak = Rc::downgrade(&gui);
    app.on_setting_ok(move |key, value| {
        weak.upgrade()
            .is_none_or(|g| g.editor.borrow().setting_ok(&key, &value))
    });

    let weak = Rc::downgrade(&gui);
    gui.ui
        .window()
        .on_close_requested(move || match weak.upgrade() {
            Some(g) if !g.settle_unsaved() => slint::CloseRequestResponse::KeepWindowShown,
            Some(g) => {
                g.save_layout();
                slint::CloseRequestResponse::HideWindow
            }
            None => slint::CloseRequestResponse::HideWindow,
        });

    gui.ui.run()?;
    Ok(())
}
