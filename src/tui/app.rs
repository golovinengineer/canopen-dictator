// SPDX-License-Identifier: MIT OR Apache-2.0

//! Terminal UI state and key handling. Edits go through the same
//! [`Editor`] as the GUI; this module only adds what a terminal needs:
//! which pane has the focus, inline field editing, prompts and popups.

use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::widgets::{ListState, TableState};

use canopen_dictator_core::editor::{Editor, ListRow, SettingField, value_ok};
use canopen_dictator_core::model::*;
use canopen_dictator_core::project;

use super::input::{Input, complete_path, display_path, expand};
use crate::io;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Focus {
    List,
    Object,
    Subs,
    Sub,
}

impl Focus {
    const ORDER: [Focus; 4] = [Focus::List, Focus::Object, Focus::Subs, Focus::Sub];

    fn step(self, by: isize) -> Focus {
        let i = Self::ORDER.iter().position(|f| *f == self).unwrap_or(0) as isize;
        Self::ORDER[(i + by).rem_euclid(Self::ORDER.len() as isize) as usize]
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Screen {
    Editor,
    Settings,
}

/// Builds the [`Target`] of a form field from its key.
pub type MakeTarget = fn(&'static str) -> Target;

/// What an edited value is written to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    Object(&'static str),
    Sub(&'static str),
    Setting(&'static str),
    /// The kind picked for a new object, after its index and name.
    NewObject { index: String, name: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FieldKind {
    Text,
    Bool,
    Choice(Vec<&'static str>),
}

/// One row of a form.
#[derive(Clone, Debug)]
pub struct Field {
    pub key: &'static str,
    pub label: &'static str,
    pub value: String,
    pub kind: FieldKind,
    /// Shown but not editable, with the reason.
    pub locked: Option<&'static str>,
    pub hint: &'static str,
}

impl Field {
    fn new(key: &'static str, label: &'static str, value: impl Into<String>, kind: FieldKind) -> Self {
        Field { key, label, value: value.into(), kind, locked: None, hint: "" }
    }

    fn hint(mut self, hint: &'static str) -> Self {
        self.hint = hint;
        self
    }

    /// The text shown for the value.
    pub fn display(&self) -> String {
        match self.kind {
            FieldKind::Bool if self.value == "true" => "[x]".into(),
            FieldKind::Bool => "[ ]".into(),
            _ => self.value.replace('\n', "⏎"),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ExportKind {
    C,
    Od,
    Eds,
    Markdown,
}

impl ExportKind {
    pub const ALL: [ExportKind; 4] = [ExportKind::C, ExportKind::Od, ExportKind::Eds, ExportKind::Markdown];

    pub fn key(self) -> char {
        match self {
            ExportKind::C => 'c',
            ExportKind::Od => 'o',
            ExportKind::Eds => 'e',
            ExportKind::Markdown => 'm',
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            ExportKind::C => "CanFestival C code (.c + .h)",
            ExportKind::Od => "CanFestival project (.od)",
            ExportKind::Eds => "EDS",
            ExportKind::Markdown => "Markdown documentation",
        }
    }

    fn file_name(self, ed: &Editor) -> String {
        match self {
            ExportKind::C => "ObjDict.c".into(),
            ExportKind::Od => "ObjDict.od".into(),
            ExportKind::Eds => format!("{}.eds", ed.project_stem()),
            ExportKind::Markdown => format!("{}.md", ed.project_stem()),
        }
    }

    fn run(self, ed: &Editor, path: &Path) -> Result<()> {
        match self {
            ExportKind::C => io::export_c(&ed.od, path),
            ExportKind::Od => io::export_od(&ed.od, path),
            ExportKind::Eds => io::export_eds(&ed.od, path),
            ExportKind::Markdown => io::export_markdown(&ed.od, path, ed.file_label()),
        }
    }
}

/// What to do once unsaved changes are settled.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum After {
    Quit,
    New,
    Open,
}

#[derive(Clone, Debug)]
pub enum Prompt {
    NewIndex,
    NewName { index: String },
    Open,
    SaveAs { after: Option<After> },
    Export(ExportKind),
}

impl Prompt {
    pub fn label(&self) -> String {
        match self {
            Prompt::NewIndex => "New object index (hex)".into(),
            Prompt::NewName { index } => format!("Name of object 0x{}", index.trim().trim_start_matches("0x").to_uppercase()),
            Prompt::Open => "Open".into(),
            Prompt::SaveAs { .. } => "Save as".into(),
            Prompt::Export(k) => format!("Export {} to", k.label()),
        }
    }

    pub fn is_path(&self) -> bool {
        matches!(self, Prompt::Open | Prompt::SaveAs { .. } | Prompt::Export(_))
    }
}

#[derive(Clone, Debug)]
pub enum Confirm {
    DeleteObject(u16),
    DeleteSub,
    OverwriteSave { path: PathBuf, after: Option<After> },
    OverwriteExport { kind: ExportKind, path: PathBuf },
}

#[derive(Clone, Debug)]
pub enum Mode {
    Normal,
    Filter(Input),
    Edit { target: Target, input: Input },
    Pick { target: Target, title: String, options: Vec<&'static str>, cursor: usize },
    Prompt { prompt: Prompt, input: Input },
    Confirm(Confirm),
    Unsaved(After),
    Export(usize),
    Help,
    Message { title: String, lines: Vec<String> },
}

pub struct App {
    pub ed: Editor,
    pub screen: Screen,
    pub focus: Focus,
    pub mode: Mode,
    pub obj_field: usize,
    pub sub_field: usize,
    pub setting_field: usize,
    pub status: String,
    pub status_error: bool,
    pub quit: bool,
    pub list_state: ListState,
    pub subs_state: TableState,
    /// The file an unsaved project was imported from, to suggest a
    /// project file name next to it.
    import_source: Option<PathBuf>,
}

const VALUE_HINT: &str = "Decimal or 0x hex that fits the type, TRUE/FALSE, $NODEID or $NODEID+offset; empty for none";

impl App {
    pub fn new() -> Self {
        App {
            ed: Editor::new(ObjectDictionary::new_default(), None, false),
            screen: Screen::Editor,
            focus: Focus::List,
            mode: Mode::Normal,
            obj_field: 0,
            sub_field: 0,
            setting_field: 0,
            status: "New project".into(),
            status_error: false,
            quit: false,
            list_state: ListState::default(),
            subs_state: TableState::default(),
            import_source: None,
        }
    }

    fn info(&mut self, text: impl Into<String>) {
        self.status = text.into();
        self.status_error = false;
    }

    fn error(&mut self, e: impl std::fmt::Display) {
        self.status = format!("{e:#}");
        self.status_error = true;
    }

    fn report(&mut self, result: Result<()>) {
        if let Err(e) = result {
            self.error(e);
        }
    }

    // ------------------------------------------------------------- forms

    pub fn object_fields(&self) -> Vec<Field> {
        let Some((index, o)) = self.ed.selected_object() else { return Vec::new() };
        vec![
            Field::new("index", "Index", format!("{index:04X}"), FieldKind::Text).hint("Hex, from 0x1000; renumbers the object"),
            Field::new("name", "Name", &o.name, FieldKind::Text),
            Field::new("kind", "Kind", o.kind.short(), FieldKind::Choice(ObjectKind::ALL.map(ObjectKind::short).to_vec())),
            Field::new("enabled", "Enabled", (!o.disabled).to_string(), FieldKind::Bool).hint("Disabled objects are left out of every export"),
            Field::new("callback", "Callback", o.callback.to_string(), FieldKind::Bool).hint("Generate a CanFestival write callback for this object"),
            Field::new("description", "Description", &o.description, FieldKind::Text),
        ]
    }

    pub fn sub_fields(&self) -> Vec<Field> {
        let Some((_, o)) = self.ed.selected_object() else { return Vec::new() };
        let Some(s) = self.ed.selected_sub_object() else { return Vec::new() };
        let mut name = Field::new("name", "Name", &s.name, FieldKind::Text);
        if o.kind == ObjectKind::Var {
            name.locked = Some("A VAR's value is named after the object");
        }
        vec![
            name,
            Field::new("type", "Type", s.data_type.cia_name(), FieldKind::Choice(DataType::ALL.map(DataType::cia_name).to_vec())),
            Field::new("access", "Access", s.access.short(), FieldKind::Choice(Access::ALL.map(Access::short).to_vec())),
            Field::new("pdo", "PDO mapping", s.pdo.short(), FieldKind::Choice(PdoMapping::ALL.map(PdoMapping::short).to_vec())),
            Field::new("default", "Default", &s.default, FieldKind::Text).hint(VALUE_HINT),
            Field::new("min", "Minimum", &s.min, FieldKind::Text).hint(VALUE_HINT),
            Field::new("max", "Maximum", &s.max, FieldKind::Text).hint(VALUE_HINT),
            Field::new("save", "Save", s.save.to_string(), FieldKind::Bool).hint("Store in non-volatile memory (CanFestival)"),
            Field::new("description", "Description", &s.description, FieldKind::Text),
        ]
    }

    fn setting_field(f: SettingField) -> Field {
        let kind = if f.key == "node_type" { FieldKind::Choice(vec!["slave", "master"]) } else { FieldKind::Text };
        Field { key: f.key, label: f.label, value: f.value, kind, locked: None, hint: f.hint }
    }

    /// Device fields, then CanFestival fields; also returns how many are
    /// device fields.
    pub fn setting_fields(&self) -> (Vec<Field>, usize) {
        let device = self.ed.device_fields();
        let n = device.len();
        let fields = device.into_iter().chain(self.ed.canfestival_fields()).map(Self::setting_field).collect();
        (fields, n)
    }

    /// The form under the cursor and the cursor in it, for the focused pane.
    fn current_form(&self) -> Option<(Vec<Field>, usize, MakeTarget)> {
        match (self.screen, self.focus) {
            (Screen::Settings, _) => Some((self.setting_fields().0, self.setting_field, Target::Setting)),
            (Screen::Editor, Focus::Object) => Some((self.object_fields(), self.obj_field, Target::Object)),
            (Screen::Editor, Focus::Sub) => Some((self.sub_fields(), self.sub_field, Target::Sub)),
            _ => None,
        }
    }

    fn form_cursor(&mut self) -> &mut usize {
        match (self.screen, self.focus) {
            (Screen::Settings, _) => &mut self.setting_field,
            (_, Focus::Sub) => &mut self.sub_field,
            _ => &mut self.obj_field,
        }
    }

    /// Whether `text` would be accepted by `target`; drives the red input.
    pub fn input_ok(&self, target: &Target, text: &str) -> bool {
        match target {
            Target::Object("index") => self.ed.index_ok(text, self.ed.selected()),
            Target::Sub("default" | "min" | "max") => {
                self.ed.selected_sub_object().is_none_or(|s| value_ok(text, s.data_type))
            }
            Target::Setting(key) => self.ed.setting_ok(key, text),
            _ => true,
        }
    }

    /// Whether `text` in `target` will export; drives the red field. Wider
    /// than `input_ok`: a default outside the limits is accepted on Enter
    /// (so a range can be moved in any order) but still shown in red.
    pub fn field_ok(&self, target: &Target, text: &str) -> bool {
        match target {
            Target::Sub(key @ ("default" | "min" | "max")) => self.ed.value_field_ok(key, text),
            _ => self.input_ok(target, text),
        }
    }

    /// Put what's wrong with the selected subindex's values in the status
    /// bar, after an edit that may have made them wrong.
    fn report_value_problem(&mut self) {
        if let Some(problem) = self.ed.value_problem() {
            self.error(problem);
        }
    }

    /// Why `input_ok` refuses `text`.
    fn input_problem(&self, target: &Target, text: &str) -> String {
        match target {
            Target::Object("index") => match canopen_dictator_core::editor::parse_hex_index(text) {
                Ok(i) => format!("index 0x{i:04X} is already used"),
                Err(e) => format!("{e:#}"),
            },
            Target::Sub(_) => match self.ed.selected_sub_object() {
                Some(s) => check_value(text, s.data_type).err().unwrap_or_default(),
                None => String::new(),
            },
            Target::Setting(key) => {
                let mut od = self.ed.od.clone();
                canopen_dictator_core::editor::apply_setting(&mut od, key, text).err().map(|e| format!("{e:#}")).unwrap_or_default()
            }
            _ => String::new(),
        }
    }

    pub fn prompt_ok(&self, prompt: &Prompt, text: &str) -> bool {
        match prompt {
            Prompt::NewIndex => self.ed.index_ok(text, None),
            p if p.is_path() => !text.trim().is_empty(),
            _ => true,
        }
    }

    fn apply(&mut self, target: &Target, value: &str) -> Result<()> {
        match target {
            Target::Object(key) => self.ed.edit_object(key, value),
            Target::Sub(key) => self.ed.edit_sub(key, value),
            Target::Setting(key) => self.ed.edit_setting(key, value),
            Target::NewObject { index, name } => {
                let index = self.ed.add_object(index, name, value)?;
                self.focus = Focus::Object;
                self.obj_field = 1;
                self.info(format!("Added object 0x{index:04X}"));
                Ok(())
            }
        }
    }

    /// Enter on a form row: edit text, toggle a check box, pick a choice.
    fn activate(&mut self, toggle_only: bool) {
        let Some((fields, cursor, target)) = self.current_form() else { return };
        let Some(f) = fields.get(cursor) else { return };
        if let Some(why) = f.locked {
            self.info(why);
            return;
        }
        let target = target(f.key);
        match &f.kind {
            FieldKind::Bool => {
                let value = (f.value != "true").to_string();
                let result = self.apply(&target, &value);
                self.report(result);
            }
            _ if toggle_only => {}
            FieldKind::Text => self.mode = Mode::Edit { target, input: Input::new(f.value.clone()) },
            FieldKind::Choice(options) => {
                let cursor = options.iter().position(|o| *o == f.value).unwrap_or(0);
                self.mode = Mode::Pick { target, title: f.label.to_string(), options: options.clone(), cursor };
            }
        }
    }

    // ---------------------------------------------------------- selection

    fn visible_indexes(&self) -> Vec<u16> {
        self.ed
            .visible_objects()
            .into_iter()
            .filter_map(|r| match r {
                ListRow::Object(i) => Some(i),
                ListRow::Header(_) => None,
            })
            .collect()
    }

    fn select(&mut self, index: Option<u16>) {
        if index != self.ed.selected() {
            self.ed.select_object(index);
            self.sub_field = 0;
        }
    }

    /// Move the object selection by `by` visible objects; `isize::MIN` and
    /// `isize::MAX` go to the first and last.
    fn move_object(&mut self, by: isize) {
        let visible = self.visible_indexes();
        if visible.is_empty() {
            return;
        }
        let pos = self.ed.selected().and_then(|s| visible.iter().position(|i| *i == s));
        let next = match pos {
            None => 0,
            Some(p) => (p as isize).saturating_add(by).clamp(0, visible.len() as isize - 1) as usize,
        };
        self.select(Some(visible[next]));
    }

    /// After the filter changed: keep the selection on a visible object.
    fn keep_selection_visible(&mut self) {
        let visible = self.visible_indexes();
        if self.ed.selected().is_none_or(|s| !visible.contains(&s)) && !visible.is_empty() {
            self.select(Some(visible[0]));
        }
    }

    fn move_sub(&mut self, by: isize) {
        let Some((_, o)) = self.ed.selected_object() else { return };
        if o.subs.is_empty() {
            return;
        }
        let pos = self.ed.selected_sub().unwrap_or(0) as isize;
        let next = pos.saturating_add(by).clamp(0, o.subs.len() as isize - 1) as usize;
        self.ed.select_sub(Some(next));
    }

    fn move_field(&mut self, by: isize) {
        let len = self.current_form().map_or(0, |f| f.0.len());
        if len == 0 {
            return;
        }
        let cursor = self.form_cursor();
        *cursor = (*cursor as isize).saturating_add(by).clamp(0, len as isize - 1) as usize;
    }

    // -------------------------------------------------------------- files

    /// `name` in the project's directory, as text for a prompt.
    fn beside_project(&self, name: &str) -> String {
        match self.ed.project_dir().filter(|d| !d.as_os_str().is_empty()) {
            Some(dir) => display_path(&dir.join(name)),
            None => name.to_string(),
        }
    }

    pub fn open_path(&mut self, path: &Path) {
        match io::open(path) {
            Ok((od, warnings)) => {
                self.info(io::open_message(path, od.objects.len(), &warnings));
                let import = io::is_import(path);
                if import {
                    self.ed = Editor::new(od, None, true);
                    self.import_source = Some(path.to_path_buf());
                } else {
                    self.ed = Editor::new(od, Some(path.to_path_buf()), false);
                    self.import_source = None;
                }
                if !warnings.is_empty() {
                    let title = if import { "Import warnings" } else { "Warnings" };
                    self.mode = Mode::Message { title: format!("{title} ({})", warnings.len()), lines: warnings };
                }
                self.reset_view();
            }
            Err(e) => self.error(e),
        }
    }

    fn reset_view(&mut self) {
        self.focus = Focus::List;
        self.obj_field = 0;
        self.sub_field = 0;
        self.list_state = ListState::default();
        self.subs_state = TableState::default();
    }

    fn new_project(&mut self) {
        self.ed = Editor::new(ObjectDictionary::new_default(), None, false);
        self.import_source = None;
        self.reset_view();
        self.info("New project");
    }

    /// Ask about unsaved changes before `after`, or just do it.
    fn guard(&mut self, after: After) {
        if self.ed.dirty() {
            self.mode = Mode::Unsaved(after);
        } else {
            self.then(after);
        }
    }

    fn then(&mut self, after: After) {
        self.mode = Mode::Normal;
        match after {
            After::Quit => self.quit = true,
            After::New => self.new_project(),
            After::Open => {
                let start = self.beside_project("");
                self.mode = Mode::Prompt { prompt: Prompt::Open, input: Input::new(start) };
            }
        }
    }

    /// Save to the project file, or ask for one; then carry on with `after`.
    fn save(&mut self, after: Option<After>) {
        match self.ed.path.clone() {
            Some(path) => self.save_to(path, after),
            None => {
                let name = match &self.import_source {
                    Some(src) => {
                        let stem = src.file_stem().and_then(|s| s.to_str()).unwrap_or("project");
                        display_path(&src.with_file_name(format!("{stem}.{}", project::EXTENSION)))
                    }
                    None => format!("project.{}", project::EXTENSION),
                };
                self.mode = Mode::Prompt { prompt: Prompt::SaveAs { after }, input: Input::new(name) };
            }
        }
    }

    fn save_to(&mut self, path: PathBuf, after: Option<After>) {
        self.mode = Mode::Normal;
        match self.ed.save_to(path.clone()) {
            Ok(()) => {
                self.import_source = None;
                self.info(format!("Saved {}", path.display()));
                if let Some(after) = after {
                    self.then(after);
                }
            }
            Err(e) => self.error(e),
        }
    }

    fn export_to(&mut self, kind: ExportKind, path: PathBuf) {
        self.mode = Mode::Normal;
        match kind.run(&self.ed, &path) {
            Ok(()) => self.info(format!("Exported {} to {}", kind.label(), path.display())),
            Err(e) => self.error(e),
        }
    }

    fn start_export(&mut self, kind: ExportKind) {
        let path = self.beside_project(&kind.file_name(&self.ed));
        self.mode = Mode::Prompt { prompt: Prompt::Export(kind), input: Input::new(path) };
    }

    // --------------------------------------------------------------- keys

    pub fn on_key(&mut self, key: KeyEvent) {
        let was_prompt = matches!(self.mode, Mode::Prompt { .. });
        self.handle_key(key);
        // The line under a prompt lists Tab completions; start it empty.
        if !was_prompt && matches!(self.mode, Mode::Prompt { .. }) && !self.status_error {
            self.info("");
        }
    }

    fn handle_key(&mut self, key: KeyEvent) {
        let mode = std::mem::replace(&mut self.mode, Mode::Normal);
        self.mode = match mode {
            Mode::Normal => {
                self.normal_key(key);
                return;
            }
            Mode::Filter(input) => self.filter_key(input, key),
            Mode::Edit { target, input } => self.edit_key(target, input, key),
            Mode::Pick { target, title, options, cursor } => self.pick_key(target, title, options, cursor, key),
            Mode::Prompt { prompt, input } => self.prompt_key(prompt, input, key),
            Mode::Confirm(c) => self.confirm_key(c, key),
            Mode::Unsaved(after) => self.unsaved_key(after, key),
            Mode::Export(cursor) => self.export_key(cursor, key),
            Mode::Help => Mode::Normal,
            Mode::Message { title, lines } => match key.code {
                KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') | KeyCode::Char(' ') => Mode::Normal,
                _ => Mode::Message { title, lines },
            },
        };
    }

    /// Mode handlers return the mode to continue in. Those that call into
    /// actions which may open a prompt or popup themselves (`then`, `save`,
    /// …) return whatever that action left in `self.mode`.
    fn normal_key(&mut self, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Char('s') if ctrl => self.save(None),
            KeyCode::Char('o') if ctrl => self.guard(After::Open),
            KeyCode::Char('n') if ctrl => self.guard(After::New),
            KeyCode::Char('c') if ctrl => self.guard(After::Quit),
            KeyCode::Char('q') => self.guard(After::Quit),
            KeyCode::Char('?') | KeyCode::F(1) => self.mode = Mode::Help,
            KeyCode::Char('x') => self.mode = Mode::Export(0),
            KeyCode::Char('s') | KeyCode::F(2) => {
                self.screen = match self.screen {
                    Screen::Editor => Screen::Settings,
                    Screen::Settings => Screen::Editor,
                };
            }
            _ => match self.screen {
                Screen::Editor => self.editor_key(key),
                Screen::Settings => self.settings_key(key),
            },
        }
    }

    fn settings_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => self.screen = Screen::Editor,
            KeyCode::Up | KeyCode::Char('k') => self.move_field(-1),
            KeyCode::Down | KeyCode::Char('j') => self.move_field(1),
            KeyCode::Home | KeyCode::Char('g') => self.move_field(isize::MIN),
            KeyCode::End | KeyCode::Char('G') => self.move_field(isize::MAX),
            KeyCode::Enter | KeyCode::Char('e') => self.activate(false),
            KeyCode::Char(' ') => self.activate(true),
            _ => {}
        }
    }

    fn editor_key(&mut self, key: KeyEvent) {
        let page = 10;
        match key.code {
            KeyCode::Tab => self.focus = self.focus.step(1),
            KeyCode::BackTab => self.focus = self.focus.step(-1),
            KeyCode::Char('/') => {
                self.focus = Focus::List;
                self.mode = Mode::Filter(Input::new(self.ed.filter()));
            }
            _ => {}
        }
        if matches!(key.code, KeyCode::Tab | KeyCode::BackTab | KeyCode::Char('/')) {
            return;
        }
        match self.focus {
            Focus::List => match key.code {
                KeyCode::Up | KeyCode::Char('k') => self.move_object(-1),
                KeyCode::Down | KeyCode::Char('j') => self.move_object(1),
                KeyCode::PageUp => self.move_object(-page),
                KeyCode::PageDown => self.move_object(page),
                KeyCode::Home | KeyCode::Char('g') => self.move_object(isize::MIN),
                KeyCode::End | KeyCode::Char('G') => self.move_object(isize::MAX),
                KeyCode::Enter | KeyCode::Right | KeyCode::Char('l') => self.focus = Focus::Object,
                KeyCode::Char('a') => self.add_object(),
                KeyCode::Char('d') | KeyCode::Delete => self.ask_delete_object(),
                KeyCode::Esc if !self.ed.filter().is_empty() => {
                    self.ed.set_filter("");
                    self.info("Filter cleared");
                }
                _ => {}
            },
            Focus::Object => match key.code {
                KeyCode::Up | KeyCode::Char('k') => self.move_field(-1),
                KeyCode::Down | KeyCode::Char('j') => self.move_field(1),
                KeyCode::Enter | KeyCode::Char('e') => self.activate(false),
                KeyCode::Char(' ') => self.activate(true),
                KeyCode::Left | KeyCode::Char('h') | KeyCode::Esc => self.focus = Focus::List,
                KeyCode::Right | KeyCode::Char('l') => self.focus = Focus::Subs,
                KeyCode::Char('a') => self.add_object(),
                KeyCode::Char('d') | KeyCode::Delete => self.ask_delete_object(),
                _ => {}
            },
            Focus::Subs => match key.code {
                KeyCode::Up | KeyCode::Char('k') => self.move_sub(-1),
                KeyCode::Down | KeyCode::Char('j') => self.move_sub(1),
                KeyCode::PageUp => self.move_sub(-page),
                KeyCode::PageDown => self.move_sub(page),
                KeyCode::Home | KeyCode::Char('g') => self.move_sub(isize::MIN),
                KeyCode::End | KeyCode::Char('G') => self.move_sub(isize::MAX),
                KeyCode::Enter | KeyCode::Right | KeyCode::Char('l') => self.focus = Focus::Sub,
                KeyCode::Left | KeyCode::Char('h') | KeyCode::Esc => self.focus = Focus::Object,
                KeyCode::Char('a') => self.add_sub(),
                KeyCode::Char('d') | KeyCode::Delete => self.ask_delete_sub(),
                _ => {}
            },
            Focus::Sub => match key.code {
                KeyCode::Up | KeyCode::Char('k') => self.move_field(-1),
                KeyCode::Down | KeyCode::Char('j') => self.move_field(1),
                KeyCode::Enter | KeyCode::Char('e') => self.activate(false),
                KeyCode::Char(' ') => self.activate(true),
                KeyCode::Left | KeyCode::Char('h') | KeyCode::Esc => self.focus = Focus::Subs,
                KeyCode::Char('a') => self.add_sub(),
                KeyCode::Char('d') | KeyCode::Delete => self.ask_delete_sub(),
                _ => {}
            },
        }
    }

    fn add_object(&mut self) {
        // Suggest the next free index after the selection.
        let from = self.ed.selected().map_or(0x2000, |i| i.saturating_add(1));
        let suggestion = self.ed.free_index(from).map(|i| format!("{i:04X}")).unwrap_or_default();
        self.mode = Mode::Prompt { prompt: Prompt::NewIndex, input: Input::new(suggestion) };
    }

    fn ask_delete_object(&mut self) {
        match self.ed.selected() {
            Some(index) => self.mode = Mode::Confirm(Confirm::DeleteObject(index)),
            None => self.error("no object selected"),
        }
    }

    fn add_sub(&mut self) {
        match self.ed.add_sub() {
            Ok(()) => {
                self.focus = Focus::Sub;
                self.sub_field = 0;
                self.info("Added subindex");
            }
            Err(e) => self.error(e),
        }
    }

    fn ask_delete_sub(&mut self) {
        let check = match (self.ed.selected_object(), self.ed.selected_sub_object()) {
            (Some((_, o)), _) if o.kind == ObjectKind::Var => Err(anyhow!("a VAR has no subindexes")),
            (_, Some(s)) if s.sub == 0 => Err(anyhow!("subindex 0 cannot be removed")),
            (_, Some(_)) => Ok(()),
            _ => Err(anyhow!("no subindex selected")),
        };
        match check {
            Ok(()) => self.mode = Mode::Confirm(Confirm::DeleteSub),
            Err(e) => self.error(e),
        }
    }

    fn filter_key(&mut self, mut input: Input, key: KeyEvent) -> Mode {
        match key.code {
            KeyCode::Enter => return Mode::Normal,
            KeyCode::Esc => {
                self.ed.set_filter("");
                return Mode::Normal;
            }
            KeyCode::Up => self.move_object(-1),
            KeyCode::Down => self.move_object(1),
            _ => {
                if input.handle(key) {
                    self.ed.set_filter(input.text());
                    self.keep_selection_visible();
                }
            }
        }
        Mode::Filter(input)
    }

    fn edit_key(&mut self, target: Target, mut input: Input, key: KeyEvent) -> Mode {
        match key.code {
            KeyCode::Esc => Mode::Normal,
            KeyCode::Enter if !self.input_ok(&target, input.text()) => {
                self.error(self.input_problem(&target, input.text()));
                Mode::Edit { target, input }
            }
            KeyCode::Enter => match self.apply(&target, input.text()) {
                Ok(()) => {
                    self.info("");
                    if matches!(target, Target::Sub("default" | "min" | "max")) {
                        self.report_value_problem();
                    }
                    Mode::Normal
                }
                Err(e) => {
                    self.error(e);
                    Mode::Edit { target, input }
                }
            },
            _ => {
                input.handle(key);
                Mode::Edit { target, input }
            }
        }
    }

    fn pick_key(&mut self, target: Target, title: String, options: Vec<&'static str>, mut cursor: usize, key: KeyEvent) -> Mode {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => return Mode::Normal,
            KeyCode::Up | KeyCode::Char('k') => cursor = cursor.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => cursor = (cursor + 1).min(options.len().saturating_sub(1)),
            KeyCode::Home | KeyCode::Char('g') => cursor = 0,
            KeyCode::End | KeyCode::Char('G') => cursor = options.len().saturating_sub(1),
            KeyCode::Enter => {
                let Some(value) = options.get(cursor) else { return Mode::Normal };
                let result = self.apply(&target, value);
                let type_changed = result.is_ok() && matches!(target, Target::Sub("type"));
                self.report(result);
                // Values that no longer fit the new type turn red; say why.
                if type_changed {
                    self.report_value_problem();
                }
                return Mode::Normal;
            }
            _ => {}
        }
        Mode::Pick { target, title, options, cursor }
    }

    fn prompt_key(&mut self, prompt: Prompt, mut input: Input, key: KeyEvent) -> Mode {
        match key.code {
            KeyCode::Esc => {
                self.info("");
                Mode::Normal
            }
            KeyCode::Tab if prompt.is_path() => {
                let (text, names) = complete_path(input.text());
                input.set(text);
                if names.len() > 1 {
                    self.info(names.join("  "));
                }
                Mode::Prompt { prompt, input }
            }
            KeyCode::Enter => self.submit(prompt, input),
            _ => {
                if input.handle(key) {
                    self.info("");
                }
                Mode::Prompt { prompt, input }
            }
        }
    }

    fn submit(&mut self, prompt: Prompt, input: Input) -> Mode {
        let text = input.text().trim().to_string();
        if !self.prompt_ok(&prompt, &text) {
            let why = match prompt {
                Prompt::NewIndex => match canopen_dictator_core::editor::parse_hex_index(&text) {
                    Ok(i) => format!("index 0x{i:04X} is already used"),
                    Err(e) => format!("{e:#}"),
                },
                _ => "enter a file path".into(),
            };
            self.error(why);
            return Mode::Prompt { prompt, input };
        }
        self.info("");
        match prompt {
            Prompt::NewIndex => Mode::Prompt { prompt: Prompt::NewName { index: text }, input: Input::new("") },
            Prompt::NewName { index } => {
                let options = ObjectKind::ALL.map(ObjectKind::short).to_vec();
                Mode::Pick { target: Target::NewObject { index, name: text }, title: "Kind".into(), options, cursor: 0 }
            }
            Prompt::Open => {
                self.open_path(&expand(&text));
                // Opening may have shown import warnings.
                std::mem::replace(&mut self.mode, Mode::Normal)
            }
            Prompt::SaveAs { after } => {
                let path = expand(&text);
                if path.exists() && self.ed.path.as_deref() != Some(path.as_path()) {
                    Mode::Confirm(Confirm::OverwriteSave { path, after })
                } else {
                    self.save_to(path, after);
                    std::mem::replace(&mut self.mode, Mode::Normal)
                }
            }
            Prompt::Export(kind) => {
                let path = expand(&text);
                if path.exists() {
                    Mode::Confirm(Confirm::OverwriteExport { kind, path })
                } else {
                    self.export_to(kind, path);
                    Mode::Normal
                }
            }
        }
    }

    fn confirm_key(&mut self, confirm: Confirm, key: KeyEvent) -> Mode {
        match key.code {
            KeyCode::Char('y' | 'Y') | KeyCode::Enter => {}
            KeyCode::Char('n' | 'N') | KeyCode::Esc => return Mode::Normal,
            _ => return Mode::Confirm(confirm),
        }
        match confirm {
            Confirm::DeleteObject(_) => match self.ed.delete_object() {
                Ok(index) => self.info(format!("Deleted object 0x{index:04X}")),
                Err(e) => self.error(e),
            },
            Confirm::DeleteSub => match self.ed.delete_sub() {
                Ok(()) => self.info("Deleted subindex"),
                Err(e) => self.error(e),
            },
            Confirm::OverwriteSave { path, after } => {
                self.save_to(path, after);
                return std::mem::replace(&mut self.mode, Mode::Normal);
            }
            Confirm::OverwriteExport { kind, path } => self.export_to(kind, path),
        }
        Mode::Normal
    }

    fn unsaved_key(&mut self, after: After, key: KeyEvent) -> Mode {
        match key.code {
            KeyCode::Char('y' | 'Y' | 's' | 'S') | KeyCode::Enter => self.save(Some(after)),
            KeyCode::Char('n' | 'N' | 'd' | 'D') => self.then(after),
            KeyCode::Esc | KeyCode::Char('c' | 'C') => return Mode::Normal,
            _ => return Mode::Unsaved(after),
        }
        std::mem::replace(&mut self.mode, Mode::Normal)
    }

    fn export_key(&mut self, mut cursor: usize, key: KeyEvent) -> Mode {
        let n = ExportKind::ALL.len();
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => return Mode::Normal,
            KeyCode::Up | KeyCode::Char('k') => cursor = cursor.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => cursor = (cursor + 1).min(n - 1),
            KeyCode::Enter => {
                self.start_export(ExportKind::ALL[cursor]);
                return std::mem::replace(&mut self.mode, Mode::Normal);
            }
            KeyCode::Char(c) => {
                if let Some(kind) = ExportKind::ALL.into_iter().find(|k| k.key() == c) {
                    self.start_export(kind);
                    return std::mem::replace(&mut self.mode, Mode::Normal);
                }
            }
            _ => {}
        }
        Mode::Export(cursor)
    }
}
