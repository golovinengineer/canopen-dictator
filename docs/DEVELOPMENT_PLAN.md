# Dictator: development plan

Tracking document for Dictator, the CANopen object dictionary editor. Update the status
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
| Workspace of three crates; `dictator-canfestival` is LGPL-2.1-or-later, the rest MIT OR Apache-2.0 | The C generator is a port of LGPL CanFestival code and cannot be relicensed; keeping it in its own crate leaves everything else permissive |
| Slint used under its Royalty-free License 2.0, attribution via `AboutSlint` in the About dialog | Desktop use is free under any app licence; the GPLv3 option would force GPL on the whole binary |

## Status

### Done (MVP)

| Area | Where | Verified by |
| --- | --- | --- |
| Model | `crates/dictator-core/src/model.rs` | used by all tests |
| XDD import (CANopenEditor 4.x flavour) | `crates/dictator-core/src/xdd.rs` | `crates/dictator-core/tests/project.rs` |
| TOML project load/save | `crates/dictator-core/src/project.rs` | `crates/dictator-core/tests/project.rs` (lossless, stable, escaping, checked-in sample) |
| EDS export, CANopenEditor layout | `crates/dictator-core/src/export/eds.rs` | `crates/dictator-core/tests/eds.rs`: equal to sample except the two `Modification*` lines |
| CanFestival `.od` import | `crates/dictator-canfestival/src/od/` | `crates/dictator-canfestival/tests/od_import.rs`: C generated from the imported `ObjDict.od` is token-identical to the sample; project-file round trip is lossless |
| CanFestival `.od` export | `crates/dictator-canfestival/src/od/` | `crates/dictator-canfestival/tests/od_export.rs`: the XDD sample exports an `.od` with the same content as `ObjDict.od` (made by the old chain); `.od` -> project -> `.od` keeps the original content; output is byte-stable. Checked by hand in the objdictedit container: the export opens with `NodeManager.OpenFileInCurrent`, and CanFestival's `gen_cfile` on it gives the same C as Dictator (apart from the deliberate differences listed below) |
| CanFestival C/H export | `crates/dictator-canfestival/` | `crates/dictator-canfestival/tests/canfestival.rs`: token-identical to the sample (it is clang-formatted, so whitespace is ignored) |
| Markdown docs export, CANopenEditor layout | `crates/dictator-core/src/export/markdown.rs` | `crates/dictator-core/tests/markdown.rs`: equal except tool/project/modified lines |
| Command line `export` (`--c`, `--od`, `--eds`, `--md`) / `import` (`.xdd`, `.od`) | `src/main.rs`, `src/io.rs` | run by hand |
| GUI: open/save, import XDD / `.od`, filter, add/delete/renumber object, change kind, add/remove subindex, edit fields, device + generator settings, exports | `ui/main.slint`, `src/gui.rs` | layout checked from a `slint-viewer` screenshot; app start checked; **editing actions not yet exercised by hand or by tests** |

### Backlog, grouped as candidate threads

Crate layout the threads work towards: `dictator-core` keeps the model, the
project file and the tool-neutral exports (EDS, Markdown); each tool gets a
crate with its importer, exporter and C generator:
`dictator-canfestival` (`.od`, CanFestival C: done) and
`dictator-canopennode` (XDD, CANopenNode C: thread 5).

1. **GUI hardening**
   - click through every action once; move the editing operations
     (`change_kind`, `sync_highest_sub`, add/remove sub) from `src/gui.rs`
     into `dictator-core` and unit-test them
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
5. **CANopenNode crate** (`crates/dictator-canopennode`, MIT OR Apache-2.0)
   - create the crate; move the XDD import out of `dictator-core` into it
     (`io::open` and the tests follow)
   - XDD export, so a project can go back to CANopenEditor; golden test:
     XDD -> project -> XDD keeps the content of `legacy_objdict.xdd`
     (compare parsed, CANopenEditor rewrites timestamps)
   - CANopenNode v4 `OD.c` / `OD.h` generator, in the form CANopenEditor's
     v4 exporter writes them; golden files: export them with CANopenEditor
     (`~/tools/CANopenEditor-v4`) from `legacy_objdict.xdd`, check them in
     under `assets/legacy_objdict/`
   - wire up: `--canopennode FILE.c` (CLI), an export button (GUI)
   - licence: CANopenNode is Apache-2.0, CANopenEditor / libEDSsharp is GPL:
     write everything from the formats and from files CANopenEditor
     produced, never by translating its C# (see Open items)
6. **Importers / exporters (tool-neutral)**
   - EDS import, DCF export
   - EDS: LSS / boot-up flags, `CompactSubObj`, `ObjFlags`
7. **CanFestival C generator completeness** (all marked `TODO` in `crates/dictator-canfestival/`)
   - user value-range types (indexes 0x0001-0x0260) and `valueRangeTest` cases
   - per-entry string `buffer_size`, pointer variables, DOMAIN checks
   - device profiles (`config/*.prf`: DS-401, DS-402 ...) and DS-302
   - 24/40/48/56-bit integers, TIME types (also needed by `.od` import/export)
8. **Project file**
   - keep user comments and unknown keys on save (`toml_edit`)
   - format versioning / migration when `format` changes
9. **Packaging**: app icon, macOS bundle, Windows/Linux builds, CI

## Notes for whoever picks this up

### Project file format

Documented at the top of `crates/dictator-core/src/project.rs`; `assets/legacy_objdict/legacy.objdict.toml`
is a full example. A VAR keeps its value fields on the object table; ARRAY and
RECORD have a `sub = [ {...}, ]` array with sub 0 written explicitly, and
per-subindex descriptions in `[object.0xNNNN.doc]`. Defaults and limits are
strings so `0x22000508` and `$NODEID+0x180` survive as typed.

### What "CanFestival-compatible" means here

`crates/dictator-canfestival` is a port of CanFestival's `node.py`, `eds_utils.py` and
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

### `.od` import and export

An `.od` is CanFestival's `node.Node` saved with `gnosis.xml.pickle`
(`od/pickle.rs` reads and writes the subset it uses, including `refid`
back-references on reading). String values are not plain XML: gnosis writes
`repr()` of the XML-escaped bytes (`safe_string`: `\n`, `\\`, `\xc2\xb0`, `'`
as `&apos;`) and reads them back by `exec`ing a Python literal
(`unsafe_string`), so a raw line break in an attribute makes objdictedit
fail to open the file.
Names, types and access come from the file's `Profile` / `DS302` /
`UserMapping` tables, then from the DS-301 table, in that order, as in
`node.py`. Things to know:

- the node id in the file is ignored (objdictedit resets it to 0 on open);
- CanFestival's PDO flag is a bool, so mappable subs become `optional`;
- a VAR's sub name is replaced by the entry name (the project file keeps one
  name per VAR; differs for 0x1006 in the DS-301 table);
- per-entry `buffer_size` is dropped with a warning; user value-range types
  (0x00A0+) and 24/40/48/56-bit or TIME types fail the import, same gaps as
  the C generator;
- objects that were disabled in the XDD never reached the `.od`, so an `.od`
  import of the sample has 12 fewer objects than the XDD import.

Export builds the node the way objdictedit's EDS import does
(`eds_utils.py: GenerateNode`): DS-301 table indexes get only a value; other
objects get a user mapping (`struct` 1 for VAR, 3 for ARRAY and RECORD alike,
so an ARRAY comes back as a RECORD), sub 0 `Number of Entries`,
`Compatibility Entry` in holes; missing values take the table default,
including the PDO COB-ID formula `{True:"$NODEID+0x%X80"%(base+1),...}`,
which the importer evaluates. Not carried: limits, `rwr`/`rww`, TPDO vs RPDO,
storage group, count label, device information other than name and
description, disabled objects. The object-level comment of an ARRAY/RECORD
is stored in `ParamsDictionary[index]["comment"]`, which objdictedit keeps
but does not show. Ids in the written file are sequential, so the same
project always gives the same bytes.

The XDD golden test clears descriptions before comparing, because
objdictedit's EDS import drops them.

To check an export in real objdictedit, the `objdictgen_img` image (built
from `assets/legacy_objdict/Dockerfile`, run with Apple's `container` as
`co_stack_gen`) mounts the repository at `/workdir`; write test files under
`target/` so nothing lands in `assets/`.

### Testing

`cargo test --workspace` runs the golden tests against `assets/legacy_objdict/`
(plain `cargo test` at the root only tests the app crate). The C test
enables callbacks on 0x2000+ before generating, because the reference `.od`
had them set by hand; the checked-in sample project has them enabled too.

### UI work

`slint-viewer --check ui/main.slint` compiles the UI alone;
`slint-viewer --screenshot out.png --load-data data.json ui/main.slint`
renders it with fake `App` data. `row` is a reserved property name in Slint
components (grid placement), hence `item`.

## Open items

- **Licences** are settled (see Decisions and `README.md`). Keep in mind:
  code ported from CanFestival goes into `dictator-canfestival` only, with an
  `LGPL-2.1-or-later` SPDX header; `dictator-core` and `dictator-canopennode`
  must not depend on it. If
  the app ever targets an embedded device, the Slint royalty-free licence does
  not cover that.
  Importers and exporters for CANopenEditor formats (XDD, its EDS and
  Markdown layout) are written from the file format and sample files, never
  translated from libEDSsharp's C# source (GPL), so they stay MIT/Apache.
- **Modification date** in EDS/docs equals the creation date (see Decisions).
  If a real one is wanted, it has to come from somewhere other than the
  project file, or accept a changed line on every save.
