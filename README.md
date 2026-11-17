# Dictator

A cross-platform CANopen object dictionary editor. You edit the dictionary
in a plain-text project file that stays readable in git diffs. Dictator
imports existing projects from other tools, and from the project file it
generates the C object dictionary for your CANopen stack, EDS and Markdown
docs.

It runs as a desktop window (Slint) or as a terminal UI (Ratatui) from the
same binary. A small command line handles imports and exports in scripts and
CI.

```
┌ Objects ──────────────────┬ 0x1800 TPDO communication parameter ─┬ Sub 0x01 ──────────────┐
│ Communication             │ Index     1800                        │ Name     COB-ID used…  │
│  1000 Device type         │ Name      TPDO communication param…   │ Type     UNSIGNED32    │
│  1001 Error register      │ Kind      RECORD                      │ Access   rw            │
│  1017 Producer heartbeat  │ Sub  Name                    Default  │ Default  $NODEID+0x180 │
│ ▸1800 TPDO communication  │ 00   Highest sub-index su…   0x06     │ PDO      no            │
│  1A00 TPDO mapping param… │ 01   COB-ID used by TPDO     $NODEID… │                        │
│ Manufacturer              │ 02   Transmission type       254      │                        │
│  2000 …                   │ 03   Inhibit time            0        │                        │
└───────────────────────────┴───────────────────────────────────────┴────────────────────────┘
 example.objdict.toml                                              ↑↓ move  ⏎ edit  ? help
```

## Why

CANopen tooling is split by stack and by platform. Each C stack comes with
its own editor and its own file format: CanFestival has `objdictedit`
(Python 2), CANopenNode has CANopenEditor (.NET, mainly Windows), and the
commercial tools are Windows-only. EDS is the format everyone shares, but it
isn't pleasant to edit or review. Moving a device between stacks, or
between machines, usually means re-entering the dictionary.

Dictator aims to be one editor for all of them:

- **Cross-platform, one binary.** Written in Rust, with a native window
  (Slint) and a terminal UI (Ratatui). It's developed on macOS, and the same
  code targets Linux and Windows. The command line needs neither front end,
  so it runs on any CI runner or headless box.
- **Many formats in, many out.** Bring in a CANopenEditor XDD or a
  CanFestival `.od` today. EDS import is next. From one project, generate
  EDS for integrators, Markdown docs, and the C object dictionary for your
  stack.
- **C library for your stack.** CanFestival C is done and token-identical to
  CanFestival's own `gen_cfile`, so firmware symbol names don't change when
  you switch. CANopenNode v4 `OD.c`/`OD.h` is next.
- **A project file made for git.** One `*.objdict.toml` per device, written
  in a stable order with one line per subindex and no timestamps. A change
  shows up in review as exactly the lines it touched.
- **Reproducible builds.** Exporting an unchanged project gives
  byte-identical files, so the C code can be generated in CI instead of
  committed (see [Generating code in CI](#generating-code-in-ci)).

## Formats

| Format | Import | Export | Notes |
| --- | :---: | :---: | --- |
| `*.objdict.toml` (Dictator project) | ✓ | ✓ | the native format |
| CANopenEditor XDD (4.x) | ✓ | | |
| CanFestival `.od` | ✓ | ✓ | opens in `objdictedit` |
| CanFestival C (`.c` + `.h`) | | ✓ | token-identical to `gen_cfile` |
| EDS | | ✓ | CANopenEditor's layout |
| Markdown documentation | | ✓ | CANopenEditor's layout |

CANopenNode v4 `OD.c`/`OD.h`, XDD export and EDS import are planned; see
[the development plan](docs/DEVELOPMENT_PLAN.md).

## Installing

Dictator is not packaged yet. Build it from source with a recent stable Rust
toolchain (the crates use edition 2024):

```sh
git clone https://github.com/golovinengineer/dictator.git
cd dictator
cargo install --path .
```

That builds both front ends. For a terminal-only binary without Slint, for
example on a machine you reach over SSH, use:

```sh
cargo install --path . --no-default-features --features tui
```

With `--no-default-features` alone you get just the command line
(`import` and `export`), which is what a CI job needs.

On Linux, the Slint window needs the usual desktop development packages
(fontconfig, xkbcommon, and Wayland or X11). See
[Slint's platform notes](https://slint.dev/docs) if the build complains.

## Quick start

Open the example project that comes with the repository:

```sh
dictator assets/example/example.objdict.toml
```

Bring in an existing CANopenEditor or CanFestival project:

```sh
dictator import MyDevice.xdd MyDevice.objdict.toml
```

Generate everything the firmware and the integrators need:

```sh
dictator export MyDevice.objdict.toml \
    --c src/ObjDict.c --eds MyDevice.eds --md docs/MyDevice.md
```

You can also open an `.xdd` or `.od` directly in the editor. Saving then
asks for a new `.objdict.toml` name next to it.

## Window or terminal

```sh
dictator [--gui | --tui] [PROJECT]
```

Dictator picks the front end in this order:

1. `--gui` or `--tui` on the command line;
2. the `DICTATOR_UI` environment variable (`gui` or `tui`);
3. the terminal UI when both stdin and stdout are a terminal, otherwise the
   window.

If you prefer the window even when you start Dictator from a shell, put
`export DICTATOR_UI=gui` in your shell profile.

Both front ends edit the same way. The window shows invalid values with a
red outline. The terminal UI won't accept them and tells you why.

> **macOS:** double-clicking the bare `dictator` binary in Finder opens it in
> your terminal app, so you get the terminal UI. Use `--gui` until a `.app`
> bundle is available.

### Terminal UI keys

Press `?` in the app for this list.

| Keys | Action |
| --- | --- |
| `↑↓` `j` `k`, `PgUp` `PgDn`, `g` `G` | move in a list or form |
| `Tab` / `Shift-Tab`, `←→` `h` `l` | switch pane |
| `/` | filter objects by name, index or subindex name |
| `Enter` `e` | edit a field, pick a choice, toggle a check box |
| `Space` | toggle a check box |
| `a` | add an object or a subindex |
| `d` `Del` | delete the object or subindex (asks first) |
| `s` `F2` | device and generator settings |
| `Ctrl-S` | save (asks for a file name the first time) |
| `Ctrl-O` / `Ctrl-N` | open a project or import `.xdd` / `.od` / new project |
| `x` | export C, `.od`, EDS or Markdown |
| `q` `Ctrl-C` | quit (asks about unsaved changes) |

Text fields take readline keys: `Ctrl-A`/`Ctrl-E` to jump to the start or
end, `Ctrl-U`/`Ctrl-K` to clear, `Ctrl-W` to delete a word. File prompts
complete paths with `Tab` and expand `~`.

## Command line

```
dictator export PROJECT [--c FILE.c] [--od FILE.od] [--eds FILE.eds] [--md FILE.md]
dictator import FILE.xdd|FILE.od OUT.objdict.toml
```

`export` writes every requested file in one run, and `--c` writes the
matching `.h` next to the `.c`. Neither command needs a display or a
terminal.

## Generating code in CI

Keep only the `*.objdict.toml` in the firmware repository and let the
pipeline produce the C library, the EDS and the docs from it. Every export
of the same project gives the same bytes, so the result is reproducible.

Build Dictator with `--no-default-features` for CI. That drops both front
ends, so the runner needs no GUI or terminal libraries, only a Rust
toolchain. There are no released binaries yet, so pin the commit you
tested with `--rev`.

**GitHub Actions**, `.github/workflows/object-dictionary.yml`:

```yaml
name: object dictionary
on: [push, pull_request]

jobs:
  generate:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/cache@v4
        with:
          path: ~/.cargo/bin/dictator
          key: dictator-<commit>
      - name: Install Dictator
        run: |
          command -v dictator || cargo install --locked --no-default-features \
            --git https://github.com/golovinengineer/dictator --rev <commit>
      - name: Generate
        run: |
          dictator export device.objdict.toml \
            --c generated/ObjDict.c --eds generated/device.eds --md generated/device.md
      - uses: actions/upload-artifact@v4
        with:
          name: object-dictionary
          path: generated/
```

**GitLab CI**, `.gitlab-ci.yml`:

```yaml
object-dictionary:
  image: rust:latest
  variables:
    CARGO_HOME: $CI_PROJECT_DIR/.cargo
  cache:
    key: dictator-<commit>
    paths: [.cargo/bin/]
  script:
    - export PATH="$CARGO_HOME/bin:$PATH"
    - command -v dictator || cargo install --locked --no-default-features
        --git https://github.com/golovinengineer/dictator --rev <commit>
    - dictator export device.objdict.toml
        --c generated/ObjDict.c --eds generated/device.eds --md generated/device.md
  artifacts:
    paths: [generated/]
```

Later jobs can pick up the generated files as artifacts. Running the export
as a step of the firmware build itself works the same way. Attach the EDS
and the Markdown to a release so integrators get files that match the
firmware.

**If you'd rather commit the generated files**, for example so the firmware
builds without Dictator, add a check that fails when someone edits the
project but forgets to regenerate:

```sh
dictator export device.objdict.toml --c firmware/ObjDict.c
git diff --exit-code firmware/
```

## The project file

A project holds device information, CanFestival generator settings and one
table per object:

```toml
format = 1

[device]
vendor_name = "Example Vendor"
product_name = "Example Device"
baud_rates = [125, 250, 500, 1000]
rpdo_count = 4
tpdo_count = 4

[canfestival]
node_name = "ExampleDevice"
node_id = 0
node_type = "slave"

[object.0x1017]
name = "Producer heartbeat time"
kind = "var"
type = "u16"
access = "rw"
default = "1000"

[object.0x1800]
name = "TPDO communication parameter"
kind = "record"
sub = [
  { sub = 0x00, name = "Highest sub-index supported", type = "u8", access = "ro", default = "0x06" },
  { sub = 0x01, name = "COB-ID used by TPDO", type = "u32", access = "rw", default = "$NODEID+0x180" },
  { sub = 0x02, name = "Transmission type", type = "u8", access = "rw", default = "254" },
]
```

- A VAR keeps its value on the object table.
- An ARRAY or RECORD lists its subindexes in `sub`, with sub 0 written
  explicitly.
- Defaults and limits are strings, so `0x22000508` and `$NODEID+0x180` stay
  as you typed them.

The format is documented at the top of
[`crates/dictator-core/src/project.rs`](crates/dictator-core/src/project.rs),
and [`assets/example/example.objdict.toml`](assets/example/example.objdict.toml)
uses every feature.

You can edit the file by hand. Dictator rewrites it in its canonical form
on save, but comments are not kept yet.

## CanFestival compatibility

The C generator is a port of CanFestival's `objdictgen`, and it keeps that
tool's naming rules. Two things can be surprising:

- Objects in CanFestival's DS-301 table (0x1000–0x1BFF) take their C names,
  types and access from that table, not from your project.
- Sub 0 of an array is the highest subindex present, not the default you
  set.

The [development plan](docs/DEVELOPMENT_PLAN.md#what-canfestival-compatible-means-here)
lists every rule and the few deliberate differences from the Python tool.

## Status

Dictator is at 0.1, and the main workflow works: import, edit, save and
export in both front ends. The golden tests check each importer and exporter
against files made by the original tools. Planned next:

- a validation panel;
- a PDO mapping editor;
- templates for standard objects;
- undo/redo;
- CANopenNode output;
- a CMake module that generates the object dictionary as part of the
  firmware build;
- packaging.

The [development plan](docs/DEVELOPMENT_PLAN.md) tracks all of it.

## Development

```sh
cargo test --workspace                # unit and golden tests
cargo clippy --workspace --all-targets
cargo run -- --gui path/to/project    # the window (a plain `cargo run` in a terminal opens the TUI)
```

| Crate | Contents | Licence |
| --- | --- | --- |
| `dictator` (repository root) | GUI, terminal UI and command line | MIT OR Apache-2.0 |
| `crates/dictator-core` | model, project file, editing session, XDD import, EDS and Markdown export | MIT OR Apache-2.0 |
| `crates/dictator-canfestival` | CanFestival C generator, `.od` import and export, a port of CanFestival `objdictgen` | LGPL-2.1-or-later |

Both front ends drive the same editing session
(`dictator_core::editor::Editor`). New editing behaviour belongs there, so
the window and the terminal UI both get it.

The golden files in [`assets/example/`](assets/example/README.md) were
produced by CANopenEditor and CanFestival. That README explains how to
regenerate them. [`docs/DEVELOPMENT_PLAN.md`](docs/DEVELOPMENT_PLAN.md)
records the design decisions, the backlog and notes on each file format.

## Licence

Dictator is licensed under either of [Apache License 2.0](LICENSE-APACHE) or
[MIT](LICENSE-MIT), at your option, **except** `crates/dictator-canfestival`.
That crate is derived from CanFestival (© Edouard Tisserant, Francis Dupin,
Laurent Bessard) and is licensed under the
[GNU LGPL 2.1 or later](crates/dictator-canfestival/COPYING).
A `dictator` binary contains both, so whoever distributes one also takes on
the LGPL obligations for that crate: provide its source and allow relinking.

The GUI uses [Slint](https://slint.dev) under the Slint Royalty-free License
2.0. The `AboutSlint` widget in the app's About dialog meets its attribution
condition.

The authors of Dictator place no licence conditions on the files you
generate with it (C, `.od`, EDS, Markdown).
