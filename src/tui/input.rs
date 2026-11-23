// SPDX-License-Identifier: MIT OR Apache-2.0

//! A one-line text input with readline-style keys, and path completion
//! for the file prompts.

use std::path::{MAIN_SEPARATOR, Path, PathBuf};

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Clone, Debug, Default)]
pub struct Input {
    text: String,
    /// Cursor position in characters.
    cursor: usize,
}

impl Input {
    /// An input holding `text`, with the cursor at the end.
    pub fn new(text: impl Into<String>) -> Self {
        let text = text.into();
        let cursor = text.chars().count();
        Input { text, cursor }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn set(&mut self, text: impl Into<String>) {
        *self = Input::new(text);
    }

    fn byte(&self, char_pos: usize) -> usize {
        self.text
            .char_indices()
            .nth(char_pos)
            .map_or(self.text.len(), |(b, _)| b)
    }

    fn len(&self) -> usize {
        self.text.chars().count()
    }

    /// Start of the word before the cursor, for Ctrl-W and Alt-B.
    fn word_start(&self) -> usize {
        let chars: Vec<char> = self.text.chars().collect();
        let mut i = self.cursor;
        while i > 0 && !chars[i - 1].is_alphanumeric() {
            i -= 1;
        }
        while i > 0 && chars[i - 1].is_alphanumeric() {
            i -= 1;
        }
        i
    }

    fn word_end(&self) -> usize {
        let chars: Vec<char> = self.text.chars().collect();
        let mut i = self.cursor;
        while i < chars.len() && !chars[i].is_alphanumeric() {
            i += 1;
        }
        while i < chars.len() && chars[i].is_alphanumeric() {
            i += 1;
        }
        i
    }

    /// Apply an editing key. Returns whether the key was used.
    pub fn handle(&mut self, key: KeyEvent) -> bool {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        match key.code {
            KeyCode::Char('a') if ctrl => self.cursor = 0,
            KeyCode::Char('e') if ctrl => self.cursor = self.len(),
            KeyCode::Char('b') if ctrl => self.cursor = self.cursor.saturating_sub(1),
            KeyCode::Char('f') if ctrl => self.cursor = (self.cursor + 1).min(self.len()),
            KeyCode::Char('b') if alt => self.cursor = self.word_start(),
            KeyCode::Char('f') if alt => self.cursor = self.word_end(),
            KeyCode::Char('u') if ctrl => {
                let b = self.byte(self.cursor);
                self.text.replace_range(..b, "");
                self.cursor = 0;
            }
            KeyCode::Char('k') if ctrl => {
                let b = self.byte(self.cursor);
                self.text.truncate(b);
            }
            KeyCode::Char('w') if ctrl => {
                let start = self.word_start();
                let (a, b) = (self.byte(start), self.byte(self.cursor));
                self.text.replace_range(a..b, "");
                self.cursor = start;
            }
            KeyCode::Char(c) if !ctrl => {
                let b = self.byte(self.cursor);
                self.text.insert(b, c);
                self.cursor += 1;
            }
            KeyCode::Backspace if self.cursor > 0 => {
                let (a, b) = (self.byte(self.cursor - 1), self.byte(self.cursor));
                self.text.replace_range(a..b, "");
                self.cursor -= 1;
            }
            KeyCode::Delete if self.cursor < self.len() => {
                let (a, b) = (self.byte(self.cursor), self.byte(self.cursor + 1));
                self.text.replace_range(a..b, "");
            }
            KeyCode::Left if alt || ctrl => self.cursor = self.word_start(),
            KeyCode::Right if alt || ctrl => self.cursor = self.word_end(),
            KeyCode::Left => self.cursor = self.cursor.saturating_sub(1),
            KeyCode::Right => self.cursor = (self.cursor + 1).min(self.len()),
            KeyCode::Home => self.cursor = 0,
            KeyCode::End => self.cursor = self.len(),
            KeyCode::Backspace | KeyCode::Delete => {}
            _ => return false,
        }
        true
    }

    /// The part of the text that fits in `width` columns, scrolled so the
    /// cursor shows, and the cursor's column in it. Line breaks show as ⏎.
    pub fn view(&self, width: usize) -> (String, usize) {
        let width = width.max(1);
        let start = (self.cursor + 1).saturating_sub(width);
        let text = self
            .text
            .chars()
            .skip(start)
            .take(width)
            .map(|c| if c == '\n' { '⏎' } else { c })
            .collect();
        (text, self.cursor - start)
    }
}

/// `~` at the start of a typed path means the home directory.
pub fn expand(text: &str) -> PathBuf {
    let home = std::env::var_os("HOME").filter(|h| !h.is_empty());
    match (text.strip_prefix('~'), home) {
        (Some(rest), Some(home)) if rest.is_empty() || rest.starts_with(['/', MAIN_SEPARATOR]) => {
            PathBuf::from(home).join(rest.trim_start_matches(['/', MAIN_SEPARATOR]))
        }
        _ => PathBuf::from(text),
    }
}

/// Complete the last component of a typed path as far as the directory
/// entries that start with it agree. A single matching directory gets a
/// trailing separator, so the next Tab lists into it. Returns the
/// completed text and the matching names when more than one remains.
pub fn complete_path(text: &str) -> (String, Vec<String>) {
    let split = text.rfind(['/', MAIN_SEPARATOR]).map_or(0, |i| i + 1);
    let (dir_text, prefix) = text.split_at(split);
    let dir = if dir_text.is_empty() {
        PathBuf::from(".")
    } else {
        expand(dir_text)
    };
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return (text.to_string(), Vec::new());
    };
    let mut matches: Vec<(String, bool)> = entries
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            let hidden = name.starts_with('.') && !prefix.starts_with('.');
            (name.starts_with(prefix) && !hidden).then(|| (name, e.path().is_dir()))
        })
        .collect();
    matches.sort();
    match matches.as_slice() {
        [] => (text.to_string(), Vec::new()),
        [(name, is_dir)] => {
            let sep = if *is_dir { "/" } else { "" };
            (format!("{dir_text}{name}{sep}"), Vec::new())
        }
        many => {
            let mut common = many[0].0.as_str();
            for (name, _) in many {
                while !name.starts_with(common) {
                    let mut chars = common.chars();
                    chars.next_back();
                    common = chars.as_str();
                }
            }
            let names = many
                .iter()
                .map(|(n, d)| if *d { format!("{n}/") } else { n.clone() })
                .collect();
            (format!("{dir_text}{common}"), names)
        }
    }
}

/// `path` shown relative to the working directory when it is inside it.
pub fn display_path(path: &Path) -> String {
    std::env::current_dir()
        .ok()
        .and_then(|cwd| path.strip_prefix(cwd).ok().map(Path::to_path_buf))
        .unwrap_or_else(|| path.to_path_buf())
        .display()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::crossterm::event::KeyEvent;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn ctrl(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
    }

    #[test]
    fn editing_keys() {
        let mut i = Input::new("héllo world");
        i.handle(ctrl('w'));
        assert_eq!(i.text(), "héllo ");
        i.handle(key(KeyCode::Home));
        i.handle(key(KeyCode::Delete));
        i.handle(key(KeyCode::Char('H')));
        assert_eq!(i.text(), "Héllo ");
        i.handle(key(KeyCode::Right));
        i.handle(ctrl('k'));
        assert_eq!(i.text(), "Hé");
        i.handle(key(KeyCode::Backspace));
        assert_eq!(i.text(), "H");
        i.handle(ctrl('u'));
        assert_eq!(i.text(), "");
    }

    #[test]
    fn view_scrolls_to_cursor() {
        let i = Input::new("abcdefgh");
        assert_eq!(i.view(4), ("fgh".to_string(), 3));
        let mut i = Input::new("ab\ncd");
        i.handle(key(KeyCode::Home));
        assert_eq!(i.view(10), ("ab⏎cd".to_string(), 0));
    }

    #[test]
    fn completes_paths() {
        let dir = std::env::temp_dir().join(format!("dictator-complete-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("projects")).unwrap();
        std::fs::write(dir.join("proj-a.toml"), "").unwrap();
        std::fs::write(dir.join("proj-b.toml"), "").unwrap();
        let base = format!("{}/", dir.display());
        let (text, names) = complete_path(&format!("{base}pro"));
        assert_eq!(text, format!("{base}proj"));
        assert_eq!(names, vec!["proj-a.toml", "proj-b.toml", "projects/"]);
        assert_eq!(
            complete_path(&format!("{base}proje")).0,
            format!("{base}projects/")
        );
        assert_eq!(complete_path(&format!("{base}zzz")).0, format!("{base}zzz"));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
