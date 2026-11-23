// SPDX-License-Identifier: MIT OR Apache-2.0

//! Drawing the terminal UI. Everything is drawn from the [`App`] state on
//! every frame; nothing here changes the dictionary.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Position, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Cell, Clear, List, ListItem, Paragraph, Row, Table, Wrap};

use canopen_dictator_core::editor::ListRow;
use canopen_dictator_core::model::PdoMapping;

use super::app::{App, Confirm, ExportKind, Field, FieldKind, Focus, MakeTarget, Mode, Screen, Target};
use crate::note::object_note;

const ACCENT: Color = Color::Cyan;
const MUTED: Color = Color::DarkGray;
const ERROR: Color = Color::Red;

fn pane(title: impl Into<String>, focused: bool) -> Block<'static> {
    let style = if focused { Style::new().fg(ACCENT) } else { Style::new().fg(MUTED) };
    Block::bordered()
        .border_type(if focused { BorderType::Thick } else { BorderType::Rounded })
        .border_style(style)
        .title(Span::styled(format!(" {} ", title.into()), if focused { style.bold() } else { Style::new() }))
}

pub fn draw(f: &mut Frame, app: &mut App) {
    let [body, status, keys] = Layout::vertical([Constraint::Fill(1), Constraint::Length(1), Constraint::Length(1)]).areas(f.area());
    match app.screen {
        Screen::Editor => draw_editor(f, app, body),
        Screen::Settings => draw_settings(f, app, body),
    }
    draw_status(f, app, status);
    draw_keys(f, app, keys);
    draw_popup(f, app);
}

// ----------------------------------------------------------------- editor

fn draw_editor(f: &mut Frame, app: &mut App, area: Rect) {
    let [list, middle, inspector] =
        Layout::horizontal([Constraint::Fill(3), Constraint::Fill(4), Constraint::Fill(3)]).areas(area);
    draw_list(f, app, list);

    let note = app.ed.selected().map(object_note).unwrap_or_default();
    // Borders and a column of padding on each side.
    let note_width = middle.width.saturating_sub(4) as usize;
    let note_lines = if note.is_empty() { 0 } else { wrapped_lines(&note, note_width) + 1 };
    let fields = app.object_fields().len().max(1) as u16;
    let [object, subs] =
        Layout::vertical([Constraint::Length(fields + note_lines + 2), Constraint::Fill(1)]).areas(middle);
    draw_object(f, app, object, &note);
    draw_subs(f, app, subs);
    draw_sub(f, app, inspector);
}

/// How many lines `text` takes when word-wrapped to `width` columns.
fn wrapped_lines(text: &str, width: usize) -> u16 {
    let width = width.max(1);
    let (mut lines, mut used) = (1u16, 0usize);
    for word in text.split_whitespace() {
        let len = word.chars().count();
        if used > 0 && used + 1 + len > width {
            lines += 1;
            used = 0;
        }
        used += if used > 0 { 1 + len } else { len };
        // A word longer than the line is broken across lines.
        while used > width {
            lines += 1;
            used -= width;
        }
    }
    lines
}

fn draw_list(f: &mut Frame, app: &mut App, area: Rect) {
    let focused = app.screen == Screen::Editor && app.focus == Focus::List;
    let count = app.ed.od.objects.len();
    let block = pane(format!("Objects ({count})"), focused);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let filtering = matches!(app.mode, Mode::Filter(_));
    let show_filter = filtering || !app.ed.filter().is_empty();
    let [filter_area, rows_area] =
        Layout::vertical([Constraint::Length(show_filter as u16), Constraint::Fill(1)]).areas(inner);
    if show_filter {
        let prefix = Span::styled("/ ", Style::new().fg(ACCENT));
        match &app.mode {
            Mode::Filter(input) => {
                let (text, cursor) = input.view(filter_area.width.saturating_sub(2) as usize);
                f.render_widget(Line::from(vec![prefix, Span::raw(text)]), filter_area);
                f.set_cursor_position(Position::new(filter_area.x + 2 + cursor as u16, filter_area.y));
            }
            _ => f.render_widget(Line::from(vec![prefix, Span::raw(app.ed.filter().to_string()).italic()]), filter_area),
        }
    }

    let rows = app.ed.visible_objects();
    if rows.is_empty() {
        f.render_widget(Paragraph::new("No matching objects").fg(MUTED), rows_area);
        return;
    }
    let width = rows_area.width as usize;
    let items: Vec<ListItem> = rows
        .iter()
        .map(|row| match row {
            ListRow::Header(title) => ListItem::new(Line::styled(*title, Style::new().fg(MUTED).add_modifier(Modifier::BOLD))),
            ListRow::Object(index) => {
                let o = &app.ed.od.objects[index];
                let kind = o.kind.short().to_uppercase();
                let name_width = width.saturating_sub(4 + 2 + kind.len() + 2);
                let name: String = o.name.chars().take(name_width).collect();
                let pad = name_width.saturating_sub(name.chars().count());
                let mut style = Style::new();
                if o.disabled {
                    style = style.fg(MUTED).add_modifier(Modifier::CROSSED_OUT);
                }
                ListItem::new(Line::from(vec![
                    Span::styled(format!(" {index:04X} "), Style::new().fg(if o.disabled { MUTED } else { ACCENT })),
                    Span::styled(name, style),
                    Span::raw(" ".repeat(pad + 1)),
                    Span::styled(kind, Style::new().fg(MUTED)),
                ]))
            }
        })
        .collect();
    let selected = app.ed.selected().and_then(|s| rows.iter().position(|r| *r == ListRow::Object(s)));
    app.list_state.select(selected);
    let highlight = if focused { Style::new().bg(ACCENT).fg(Color::Black) } else { Style::new().add_modifier(Modifier::REVERSED) };
    f.render_stateful_widget(List::new(items).highlight_style(highlight), rows_area, &mut app.list_state);
}

/// Draw form rows; `cursor` is set when the form has the focus. A row
/// being edited shows the input and places the terminal cursor.
fn draw_fields(f: &mut Frame, app: &App, area: Rect, fields: &[Field], cursor: Option<usize>, make: MakeTarget) {
    let height = area.height as usize;
    let offset = cursor.map_or(0, |c| (c + 1).saturating_sub(height));
    // A leading space, the longest label, and two spaces before the value.
    let label_width = fields.iter().map(|f| f.label.chars().count()).max().unwrap_or(0) as u16 + 3;
    let value_width = area.width.saturating_sub(label_width) as usize;
    let mut lines = Vec::new();
    for (i, field) in fields.iter().enumerate().skip(offset).take(height) {
        let selected = cursor == Some(i);
        let label = format!(" {:<w$}", field.label, w = label_width as usize - 1);
        let label = Span::styled(label, if selected { Style::new().fg(ACCENT).bold() } else { Style::new().fg(MUTED) });
        let editing = match &app.mode {
            Mode::Edit { target, input } if selected && *target == make(field.key) => Some((target, input)),
            _ => None,
        };
        let value = if let Some((target, input)) = editing {
            let ok = app.field_ok(target, input.text());
            let (text, col) = input.view(value_width.saturating_sub(1));
            f.set_cursor_position(Position::new(area.x + label_width + col as u16, area.y + (i - offset) as u16));
            let style = if ok { Style::new().add_modifier(Modifier::UNDERLINED) } else { Style::new().fg(ERROR).add_modifier(Modifier::UNDERLINED) };
            Span::styled(format!("{text:<value_width$}"), style)
        } else {
            let mut text = field.display();
            if let FieldKind::Choice(_) = field.kind {
                text.push_str(" ▾");
            }
            let mut style = Style::new();
            if field.locked.is_some() {
                style = style.fg(MUTED);
            }
            // A stored value that won't export, e.g. from a hand-edited file.
            if !app.field_ok(&make(field.key), &field.value) {
                style = style.fg(ERROR);
            }
            if selected {
                style = style.add_modifier(Modifier::REVERSED);
            }
            let text: String = text.chars().take(value_width).collect();
            Span::styled(text, style)
        };
        lines.push(Line::from(vec![label, value]));
    }
    f.render_widget(Paragraph::new(lines), area);
}

fn draw_object(f: &mut Frame, app: &App, area: Rect, note: &str) {
    let focused = app.focus == Focus::Object;
    let title = match app.ed.selected_object() {
        Some((index, o)) => format!("0x{index:04X} {}", o.name),
        None => "Object".into(),
    };
    let block = pane(title, focused);
    let inner = block.inner(area);
    f.render_widget(block, area);
    let fields = app.object_fields();
    if fields.is_empty() {
        f.render_widget(Paragraph::new(" No object selected; press a to add one").fg(MUTED), inner);
        return;
    }
    let [form, rest] = Layout::vertical([Constraint::Length(fields.len() as u16), Constraint::Fill(1)]).areas(inner);
    let cursor = focused.then(|| app.obj_field.min(fields.len() - 1));
    draw_fields(f, app, form, &fields, cursor, Target::Object);
    if !note.is_empty() {
        let [_, note_area] = Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]).areas(rest);
        let note_area = Rect { x: note_area.x + 1, width: note_area.width.saturating_sub(2), ..note_area };
        f.render_widget(Paragraph::new(note.to_string()).fg(Color::Yellow).italic().wrap(Wrap { trim: true }), note_area);
    }
}

fn draw_subs(f: &mut Frame, app: &mut App, area: Rect) {
    let focused = app.focus == Focus::Subs;
    let Some((_, o)) = app.ed.selected_object() else {
        f.render_widget(pane("Subindexes", focused), area);
        return;
    };
    // Narrow panes drop the PDO and then the access column, so names stay readable.
    let inner = area.width.saturating_sub(2);
    let (access, pdo) = (inner >= 50, inner >= 60);
    let mut titles = vec!["Sub", "Name", "Type"];
    let mut widths = vec![Constraint::Length(3), Constraint::Fill(3), Constraint::Length(7)];
    if access {
        titles.push("Access");
        widths.push(Constraint::Length(6));
    }
    if pdo {
        titles.push("PDO");
        widths.push(Constraint::Length(8));
    }
    titles.push("Default");
    widths.push(Constraint::Fill(2));
    let header = Row::new(titles).style(Style::new().fg(MUTED).bold());
    let rows: Vec<Row> = o
        .subs
        .iter()
        .map(|s| {
            let mut cells = vec![
                Cell::from(format!("{:02X}", s.sub)).fg(ACCENT),
                Cell::from(s.name.clone()),
                Cell::from(s.data_type.short()),
            ];
            if access {
                cells.push(Cell::from(s.access.short()));
            }
            if pdo {
                cells.push(Cell::from(if s.pdo == PdoMapping::No { "" } else { s.pdo.short() }));
            }
            cells.push(Cell::from(s.default.clone()));
            Row::new(cells)
        })
        .collect();
    let highlight = if focused { Style::new().bg(ACCENT).fg(Color::Black) } else { Style::new().add_modifier(Modifier::REVERSED) };
    let table = Table::new(rows, widths)
        .header(header)
        .block(pane(format!("Subindexes ({})", o.subs.len()), focused))
        .row_highlight_style(highlight);
    app.subs_state.select(app.ed.selected_sub());
    f.render_stateful_widget(table, area, &mut app.subs_state);
}

fn draw_sub(f: &mut Frame, app: &App, area: Rect) {
    let focused = app.focus == Focus::Sub;
    let title = match app.ed.selected_sub_object() {
        Some(s) => format!("Sub 0x{:02X}", s.sub),
        None => "Subindex".into(),
    };
    let block = pane(title, focused);
    let inner = block.inner(area);
    f.render_widget(block, area);
    let fields = app.sub_fields();
    if fields.is_empty() {
        f.render_widget(Paragraph::new(" No subindex selected").fg(MUTED), inner);
        return;
    }
    let [form, hint] = Layout::vertical([Constraint::Length(fields.len() as u16), Constraint::Fill(1)]).areas(inner);
    let cursor = focused.then(|| app.sub_field.min(fields.len() - 1));
    draw_fields(f, app, form, &fields, cursor, Target::Sub);
    if let Some(field) = cursor.and_then(|c| fields.get(c)) {
        draw_hint(f, field, hint);
    }
}

fn draw_hint(f: &mut Frame, field: &Field, area: Rect) {
    let text = field.locked.unwrap_or(field.hint);
    if text.is_empty() || area.height < 2 {
        return;
    }
    let [_, area] = Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]).areas(area);
    f.render_widget(Paragraph::new(format!(" {text}")).fg(MUTED).wrap(Wrap { trim: false }), area);
}

// --------------------------------------------------------------- settings

fn draw_settings(f: &mut Frame, app: &App, area: Rect) {
    let (fields, device_count) = app.setting_fields();
    let width = area.width.min(96);
    let area = Rect { x: area.x + (area.width - width) / 2, width, ..area };
    let [device, canfestival, hint] = Layout::vertical([
        Constraint::Length(device_count as u16 + 2),
        Constraint::Length((fields.len() - device_count) as u16 + 2),
        Constraint::Fill(1),
    ])
    .areas(area);
    let cursor = app.setting_field.min(fields.len() - 1);
    let in_device = cursor < device_count;

    let block = pane("Device", in_device);
    let inner = block.inner(device);
    f.render_widget(block, device);
    draw_fields(f, app, inner, &fields[..device_count], in_device.then_some(cursor), Target::Setting);

    let block = pane("CanFestival generator", !in_device);
    let inner = block.inner(canfestival);
    f.render_widget(block, canfestival);
    let cf_cursor = (!in_device).then(|| cursor - device_count);
    draw_fields(f, app, inner, &fields[device_count..], cf_cursor, Target::Setting);

    draw_hint(f, &fields[cursor], hint);
}

// ----------------------------------------------------------- status, keys

fn draw_status(f: &mut Frame, app: &App, area: Rect) {
    if let Mode::Prompt { prompt, input } = &app.mode {
        let label = format!(" {}: ", prompt.label());
        let label_width = label.chars().count() as u16;
        let ok = app.prompt_ok(prompt, input.text());
        let (text, col) = input.view(area.width.saturating_sub(label_width + 1) as usize);
        let style = if ok { Style::new() } else { Style::new().fg(ERROR) };
        f.render_widget(Line::from(vec![Span::styled(label, Style::new().fg(ACCENT).bold()), Span::styled(text, style)]), area);
        f.set_cursor_position(Position::new(area.x + label_width + col as u16, area.y));
        return;
    }
    let mut spans = vec![Span::styled(format!(" {} ", app.ed.file_label()), Style::new().bg(ACCENT).fg(Color::Black).bold())];
    if app.ed.dirty() {
        spans.push(Span::styled(" ● modified ", Style::new().fg(Color::Yellow)));
    }
    spans.push(Span::raw(" "));
    let style = if app.status_error { Style::new().fg(ERROR).bold() } else { Style::new() };
    spans.push(Span::styled(app.status.clone(), style));
    f.render_widget(Line::from(spans), area);
}

fn draw_keys(f: &mut Frame, app: &App, area: Rect) {
    // A prompt covers the status line, so its errors show here instead.
    if matches!(app.mode, Mode::Prompt { .. }) && app.status_error {
        f.render_widget(Line::styled(format!(" {}", app.status), Style::new().fg(ERROR).bold()), area);
        return;
    }
    // Tab completion lists the candidates in the same place.
    if matches!(&app.mode, Mode::Prompt { prompt, .. } if prompt.is_path()) && !app.status.is_empty() {
        f.render_widget(Line::styled(format!(" {}", app.status), Style::new().fg(MUTED)), area);
        return;
    }
    let keys: &[(&str, &str)] = match (&app.mode, app.screen, app.focus) {
        (Mode::Filter(_), ..) => &[("type", "filter"), ("↑↓", "move"), ("⏎", "done"), ("esc", "clear")],
        (Mode::Edit { .. }, ..) => &[("⏎", "apply"), ("esc", "cancel"), ("^U/^K", "clear"), ("^W", "delete word")],
        (Mode::Prompt { prompt, .. }, ..) if prompt.is_path() => &[("⏎", "ok"), ("tab", "complete"), ("esc", "cancel")],
        (Mode::Prompt { .. }, ..) => &[("⏎", "ok"), ("esc", "cancel")],
        (Mode::Normal, Screen::Settings, _) => {
            &[("↑↓", "move"), ("⏎", "edit"), ("s/esc", "objects"), ("x", "export"), ("^S", "save"), ("?", "help"), ("q", "quit")]
        }
        (Mode::Normal, _, Focus::List) => &[
            ("↑↓", "move"),
            ("⏎", "open"),
            ("/", "filter"),
            ("a", "add"),
            ("d", "delete"),
            ("tab", "pane"),
            ("s", "settings"),
            ("x", "export"),
            ("^S", "save"),
            ("?", "help"),
            ("q", "quit"),
        ],
        (Mode::Normal, _, Focus::Subs) => {
            &[("↑↓", "move"), ("⏎", "edit"), ("a", "add sub"), ("d", "delete sub"), ("←", "back"), ("tab", "pane"), ("?", "help")]
        }
        (Mode::Normal, ..) => &[("↑↓", "field"), ("⏎", "edit"), ("space", "toggle"), ("←→", "pane"), ("^S", "save"), ("?", "help")],
        _ => &[],
    };
    let mut spans = vec![Span::raw(" ")];
    for (key, what) in keys {
        spans.push(Span::styled(*key, Style::new().fg(ACCENT).bold()));
        spans.push(Span::styled(format!(" {what}  "), Style::new().fg(MUTED)));
    }
    f.render_widget(Line::from(spans), area);
}

// ----------------------------------------------------------------- popups

fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    Rect { x: area.x + (area.width - width) / 2, y: area.y + (area.height - height) / 2, width, height }
}

fn popup(f: &mut Frame, title: &str, width: u16, height: u16) -> Rect {
    let area = centered(f.area(), width, height);
    f.render_widget(Clear, area);
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(ACCENT))
        .title(Span::styled(format!(" {title} "), Style::new().fg(ACCENT).bold()));
    let inner = block.inner(area);
    f.render_widget(block, area);
    inner
}

fn key_line(key: &str, what: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!(" {key:<12}"), Style::new().fg(ACCENT).bold()),
        Span::raw(what.to_string()),
    ])
}

fn draw_popup(f: &mut Frame, app: &App) {
    match &app.mode {
        Mode::Pick { title, options, cursor, .. } => {
            let width = options.iter().map(|o| o.len()).max().unwrap_or(10).max(title.len()) as u16 + 8;
            let inner = popup(f, title, width, options.len() as u16 + 2);
            let items: Vec<ListItem> = options.iter().map(|o| ListItem::new(format!(" {o}"))).collect();
            let mut state = ratatui::widgets::ListState::default().with_selected(Some(*cursor));
            f.render_stateful_widget(List::new(items).highlight_style(Style::new().bg(ACCENT).fg(Color::Black)), inner, &mut state);
        }
        Mode::Confirm(c) => {
            let question = match c {
                Confirm::DeleteObject(index) => {
                    let name = app.ed.od.objects.get(index).map(|o| o.name.as_str()).unwrap_or("");
                    format!("Delete object 0x{index:04X} {name}?")
                }
                Confirm::DeleteSub => match app.ed.selected_sub_object() {
                    Some(s) => format!("Delete subindex 0x{:02X} {}?", s.sub, s.name),
                    None => "Delete subindex?".into(),
                },
                Confirm::OverwriteSave { path, .. } | Confirm::OverwriteExport { path, .. } => {
                    format!("{} exists. Replace it?", path.display())
                }
            };
            let width = (question.chars().count() as u16 + 6).clamp(30, 90).min(f.area().width);
            let height = wrapped_lines(&question, width.saturating_sub(3) as usize) + 4;
            let inner = popup(f, "Confirm", width, height);
            let text = vec![Line::raw(format!(" {question}")), Line::raw(""), key_line("y / n", "yes / no")];
            f.render_widget(Paragraph::new(text).wrap(Wrap { trim: false }), inner);
        }
        Mode::Unsaved(_) => {
            let inner = popup(f, "Unsaved changes", 52, 7);
            let text = vec![
                Line::raw(format!(" Save the changes to {}?", app.ed.file_label())),
                Line::raw(""),
                key_line("y", "save"),
                key_line("n", "don't save"),
                key_line("esc", "cancel"),
            ];
            f.render_widget(Paragraph::new(text), inner);
        }
        Mode::Export(cursor) => {
            let inner = popup(f, "Export", 40, ExportKind::ALL.len() as u16 + 2);
            let items: Vec<ListItem> = ExportKind::ALL
                .iter()
                .map(|k| {
                    ListItem::new(Line::from(vec![
                        Span::styled(format!(" {} ", k.key()), Style::new().fg(ACCENT).bold()),
                        Span::raw(k.label()),
                    ]))
                })
                .collect();
            let mut state = ratatui::widgets::ListState::default().with_selected(Some(*cursor));
            f.render_stateful_widget(List::new(items).highlight_style(Style::new().add_modifier(Modifier::REVERSED)), inner, &mut state);
        }
        Mode::Message { title, lines } => {
            let width = f.area().width.saturating_sub(8).min(100);
            let inner = popup(f, title, width, (lines.len() as u16 + 4).min(f.area().height.saturating_sub(4)));
            let mut text: Vec<Line> = lines.iter().map(|l| Line::raw(format!(" • {l}"))).collect();
            text.push(Line::raw(""));
            text.push(Line::styled(" Press Enter to close", Style::new().fg(MUTED)));
            f.render_widget(Paragraph::new(text).wrap(Wrap { trim: false }), inner);
        }
        Mode::Help => {
            let text = vec![
                Line::styled(" Moving", Style::new().bold()),
                key_line("↑↓ j k", "move in a list or form; PgUp/PgDn, g/G jump"),
                key_line("tab ⇧tab", "next / previous pane"),
                key_line("← → h l", "pane to the left / right; ⏎ in a list opens"),
                key_line("/", "filter objects by name, index or subindex name"),
                Line::raw(""),
                Line::styled(" Editing", Style::new().bold()),
                key_line("⏎ e", "edit field, pick a choice, toggle a check box"),
                key_line("space", "toggle a check box"),
                key_line("a", "add an object (list, object) or a subindex"),
                key_line("d del", "delete the object or subindex"),
                key_line("s F2", "device and generator settings"),
                Line::raw(""),
                Line::styled(" Files", Style::new().bold()),
                key_line("^S", "save (asks for a file the first time)"),
                key_line("^O ^N", "open a project or import .xdd / .od; new project"),
                key_line("x", "export C, .od, EDS or Markdown"),
                key_line("q ^C", "quit"),
                Line::raw(""),
                Line::styled(" Text fields: ^A/^E start/end, ^U/^K clear, ^W delete word", Style::new().fg(MUTED)),
                Line::styled(" Press any key to close", Style::new().fg(MUTED)),
            ];
            let inner = popup(f, "Dictator keys", 70, text.len() as u16 + 2);
            f.render_widget(Paragraph::new(text), inner);
        }
        _ => {}
    }
}
