# Dictator

CANopen object dictionary editor (Rust + Slint). Edits the object dictionary
in a git-friendly `*.objdict.toml` project file and exports EDS,
CanFestival-compatible C and Markdown docs. See
[docs/DEVELOPMENT_PLAN.md](docs/DEVELOPMENT_PLAN.md).

## Layout

| Crate | Contents | Licence |
| --- | --- | --- |
| `dictator` (repository root) | GUI and command line | MIT OR Apache-2.0 |
| `crates/dictator-core` | model, project file, XDD import, EDS and Markdown export | MIT OR Apache-2.0 |
| `crates/dictator-canfestival` | CanFestival C generator, a port of CanFestival `objdictgen` | LGPL-2.1-or-later |

## Licence

Dictator is licensed under either of [Apache License 2.0](LICENSE-APACHE) or
[MIT](LICENSE-MIT), at your option, **except** `crates/dictator-canfestival`.
That crate is derived from CanFestival (© Edouard Tisserant, Francis Dupin,
Laurent Bessard) and is licensed under the
[GNU LGPL 2.1 or later](crates/dictator-canfestival/COPYING).
A `dictator` binary contains both, so whoever distributes one also takes on the
LGPL obligations for that crate (provide its source, allow relinking).

The GUI uses [Slint](https://slint.dev) under the Slint Royalty-free License
2.0. Its attribution condition is met by the `AboutSlint` widget in the
app's About dialog.

The authors of Dictator place no licence conditions on the files you generate
with it (C, EDS, Markdown).
