# Development plan

Tracking document for the CANopen object dictionary editor. Update the status
tables when a thread finishes something; add findings to "Notes for whoever
picks this up" rather than rediscovering them.

## Goal

One native app (Rust + Slint) that replaces the chain
CANopenEditor (`.xdd`) -> EDS -> CanFestival `objdictedit.py` (Python 2 in
Docker) -> `gen_cfile.py`: edit the object dictionary in a git-friendly
project file and export EDS, CanFestival-compatible C and Markdown docs.

## Decisions

| Decision | Why |
| --- | --- |
| Native project file is TOML (`*.objdict.toml`), XDD is import-only | XDD splits each object over three XML sections and rewrites timestamps on save, so diffs are unreadable |
| Hand-written TOML writer, one line per subindex, no column padding, no timestamps | Stable output; a change touches only the lines it concerns |
| C output must equal what the old chain produced | Firmware references the generated symbols; verified against `assets/legacy_objdict/ObjDict.c/.h` |
| CanFestival settings (node name/id/type, callbacks, save flags) are first-class project fields | XDD has no place for them |
| Callbacks default to off | CanFestival's own default; enabled per object |
| Exports use the project's `created` time for creation *and* modification | Re-exporting an unchanged project gives byte-identical files |

## Status

### Done (MVP)

| Area | Where | Verified by |
| --- | --- | --- |
| Model | `src/model.rs` | used by all tests |
| XDD import (CANopenEditor 4.x flavour) | `src/xdd.rs` | `tests/project.rs` |
| TOML project load/save | `src/project.rs` | `tests/project.rs` (lossless, stable, escaping, checked-in sample) |
| EDS export, CANopenEditor layout | `src/export/eds.rs` | `tests/eds.rs`: equal to sample except the two `Modification*` lines |
| CanFestival C/H export | `src/canfestival/` | `tests/canfestival.rs`: token-identical to the sample (it is clang-formatted, so whitespace is ignored) |
| Markdown docs export, CANopenEditor layout | `src/export/markdown.rs` | `tests/markdown.rs`: equal except tool/project/modified lines |
| Command line `export` / `import` | `src/main.rs`, `src/io.rs` | run by hand |
| GUI: open/save, import XDD, filter, add/delete/renumber object, change kind, add/remove subindex, edit fields, device + generator settings, exports | `ui/main.slint`, `src/gui.rs` | layout checked from a `slint-viewer` screenshot; app start checked; **editing actions not yet exercised by hand or by tests** |

### Backlog, grouped as candidate threads

1. **GUI hardening**
   - click through every action once; move the editing operations
     (`change_kind`, `sync_highest_sub`, add/remove sub) from `src/gui.rs`
     into the library and unit-test them
   - undo/redo; keyboard shortcuts and a menu bar; remember export targets in
     the project so re-export is one click
   - combo box for node type; multi-line device description
2. **Validation**
   - panel listing problems before export: duplicate C names after
     `format_name`, defaults outside limits or type range, PDO mappings that
     point at missing/unmappable objects, sub 0 inconsistent with subs,
     project kind conflicting with CanFestival's DS-301 table (currently an
     export error)
3. **PDO mapping editor**
   - pick mapped variables from a list instead of typing `0x22000508`;
     show bit usage per PDO
4. **Object templates**
   - insert standard communication objects (heartbeat, SDO server, n-th
     RPDO/TPDO pair) with correct defaults; clone / renumber ranges
5. **Importers / exporters**
   - XDD export (so a project can go back to CANopenEditor)
   - EDS import, CanFestival `.od` import, DCF export
   - EDS: LSS / boot-up flags, `CompactSubObj`, `ObjFlags`
6. **C generator completeness** (all marked `TODO` in `src/canfestival/`)
   - user value-range types (indexes 0x0001-0x0260) and `valueRangeTest` cases
   - per-entry string `buffer_size`, pointer variables, DOMAIN checks
   - device profiles (`config/*.prf`: DS-401, DS-402 ...) and DS-302
   - 24/40/48/56-bit integers, TIME types
7. **Project file**
   - keep user comments and unknown keys on save (`toml_edit`)
   - format versioning / migration when `format` changes
8. **Packaging**: app icon, macOS bundle, Windows/Linux builds, CI

## Notes for whoever picks this up

### Project file format

Documented at the top of `src/project.rs`; `assets/legacy_objdict/legacy.objdict.toml`
is a full example. A VAR keeps its value fields on the object table; ARRAY and
RECORD have a `sub = [ {...}, ]` array with sub 0 written explicitly, and
per-subindex descriptions in `[object.0xNNNN.doc]`. Defaults and limits are
strings so `0x22000508` and `$NODEID+0x180` survive as typed.

### What "CanFestival-compatible" means here

`src/canfestival/` is a port of CanFestival's `node.py`, `eds_utils.py` and
`gen_cfile.py` (reference copy: `co_objdictgen`). The old chain
imported an EDS, and that import, not the templates, decides most names:

- Indexes in CanFestival's DS-301 table (`ds301.rs`, 0x1000-0x1BFF) take entry
  name, sub-entry names, types, access and the callback flag from the table.
  The project's names for those objects never reach the C code
  (`_obj1800_COB_ID_used_by_PDO`, not "COB-ID used by TPDO"; 0x1008 is CONST
  whatever the project says).
- Every other ARRAY/RECORD becomes one variable per subindex
  (`<node>_<Object>_<Sub>`), sub 0 is a read-only UNS8 `Number of Entries`.
- Missing subindexes are filled with `Compatibility Entry` (see 0x1800 sub 4).
- Sub 0 in C is the highest subindex present, **not** the project default
  (0x1A00 has default 6 but the C says 8), and 0x1003 sub 0 is always 0.
- `$NODEID+x` is evaluated with the node id from `[canfestival]`.
- Disabled objects are left out of every export.

Deliberate differences from the Python tool: decimal defaults with a leading
zero are not read as octal; a negative default of a VAR below 0x2000 keeps its
sign; a CONST sub of a mapped record gets one `const CONSTSTORE`, not two.

### XDD import

Each object is joined from `CANopenObjectList` and `parameterList` via
`uniqueIDRef`. A parameter has either a `<description>` or a `<label>`; the
label only repeats the name and is ignored. `BITSTRING` is imported as
OCTET_STRING because CANopenEditor writes DOMAIN the same way.

### Testing

`cargo test` runs the golden tests against `assets/legacy_objdict/`. The C test
enables callbacks on 0x2000+ before generating, because the reference `.od`
had them set by hand; the checked-in sample project has them enabled too.

### UI work

`slint-viewer --check ui/main.slint` compiles the UI alone;
`slint-viewer --screenshot out.png --load-data data.json ui/main.slint`
renders it with fake `App` data. `row` is a reserved property name in Slint
components (grid placement), hence `item`.

## Open items

- **Licences.** `src/canfestival/` is a port of LGPL-2.1 CanFestival code, and
  Slint is offered under GPLv3, a royalty-free licence and a commercial one.
  Decide how the app is distributed before publishing it.
- **Modification date** in EDS/docs equals the creation date (see Decisions).
  If a real one is wanted, it has to come from somewhere other than the
  project file, or accept a changed line on every save.
