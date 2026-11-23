// SPDX-License-Identifier: MIT OR Apache-2.0

//! Ratatui front end, started when Dictator runs in a terminal. It drives
//! the same [`canopen_dictator_core::editor::Editor`] as the Slint GUI.

mod app;
mod input;
mod ui;

use std::path::PathBuf;

use anyhow::Result;
use ratatui::crossterm::event::{self, Event, KeyEventKind};

use app::App;

pub fn run(initial: Option<PathBuf>) -> Result<()> {
    let mut app = App::new();
    if let Some(path) = initial {
        app.open_path(&path);
    }
    // Switches to the alternate screen in raw mode, and puts the terminal
    // back on a panic.
    let mut terminal = ratatui::init();
    let result = (|| -> Result<()> {
        while !app.quit {
            terminal.draw(|f| ui::draw(f, &mut app))?;
            if let Event::Key(key) = event::read()?
                && key.kind != KeyEventKind::Release
            {
                app.on_key(key);
            }
        }
        Ok(())
    })();
    ratatui::restore();
    result
}
