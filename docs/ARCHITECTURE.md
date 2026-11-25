# Dictator: architecture and notes

Design decisions and notes on each file format, for whoever works on
Dictator. Planned work and bugs live in the
[issue tracker](https://github.com/golovinengineer/canopen-dictator/issues); add
findings here rather than rediscovering them.

## Goal

One native app (Rust + Slint) that replaces the chain
CANopenEditor (`.xdd`) -> EDS -> CanFestival `objdictedit.py` (Python 2 in
Docker) -> `gen_cfile.py`: edit the object dictionary in a git-friendly
project file and export EDS, CanFestival-compatible C and Markdown docs.

## Decisions

| Decision | Why |
| --- | --- |
| Native project file is TOML (`*.objdict`), XDD is import-only | XDD splits each object over three XML sections and rewrites timestamps on save, so diffs are unreadable |
| Hand-written TOML writer, one line per subindex, no column padding, no timestamps | Stable output; a change touches only the lines it concerns |
| C output must equal what the old chain produced | Firmware references the generated symbols; verified against gen_cfile output in `assets/example/` |
| CanFestival settings (node name/id/type, callbacks, save flags) are first-class project fields | XDD has no place for them |
| Callbacks default to off | CanFestival's own default; enabled per object |
| Exports use the project's `created` time for creation *and* modification | Re-exporting an unchanged project gives byte-identical files |
| Workspace of three crates; `canopen-dictator-canfestival` is LGPL-2.1-or-later, the rest MIT OR Apache-2.0 | The C generator is a port of LGPL CanFestival code and cannot be relicensed; keeping it in its own crate leaves everything else permissive |
| Slint used under its Royalty-free License 2.0, attribution via `AboutSlint` in the About dialog | Desktop use is free under any app licence; the GPLv3 option would force GPL on the whole binary |

## Notes

### Two front ends, one editing session

`canopen_dictator_core::editor::Editor` owns the dictionary, the file path, the
saved copy (for dirty tracking), the selection and the filter. Both
`src/gui.rs` and `src/tui/` call it with the same string field keys and
text values; put new editing behaviour there, not in a front end, so both
get it. Front ends keep only presentation: Slint models and `rfd` dialogs in
the GUI; focus, inline inputs, prompts and popups in the TUI. Both run in
one process: there is no client/server split, since nothing needs to be
shared between processes. During GUI work use `cargo run -- --gui`, as a
plain `cargo run` from a terminal now opens the TUI.

### Project file format

Documented at the top of `crates/canopen-dictator-core/src/project.rs`; `assets/example/example.objdict`
is a full example. A VAR keeps its value fields on the object table; ARRAY and
RECORD have a `sub = [ {...}, ]` array with sub 0 written explicitly, and
per-subindex descriptions in `[object.0xNNNN.doc]`. Defaults and limits are
strings so `0x22000508` and `$NODEID+0x180` survive as typed.

### What "CanFestival-compatible" means here

`crates/canopen-dictator-canfestival` is a port of CanFestival's `node.py`, `eds_utils.py` and
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
label only repeats the name and is ignored. CANopenEditor writes OCTET_STRING
and DOMAIN alike as `BITSTRING` and tells them apart by the default value;
the import does the same (with a default: OCTET_STRING). More CANopenEditor
behaviour is listed in `assets/example/README.md`.

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
  import has fewer objects than the XDD import of the same project.

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

The export golden test clears descriptions before comparing, because
objdictedit's EDS import drops them.

To check an export in real objdictedit, the `objdictgen_img` image (built
from `tools/objdictgen/Dockerfile`, run with Apple's `container` as
`co_stack_gen`) mounts the repository at `/workdir`; write test files under
`target/` so nothing lands in `assets/`.

### Testing

`cargo test --workspace` runs the golden tests against `assets/example/`
(plain `cargo test` at the root only tests the app crate). That directory's
README says how each reference file was made and how to regenerate them.
The CanFestival tests leave out 0x200D and 0x200E, as the CanFestival
references do; the checked-in project carries the CanFestival settings
(node name, callbacks, save flags) that were set in objdictedit.

### UI work

`slint-viewer --check ui/main.slint` compiles the UI alone;
`slint-viewer --screenshot out.png --load-data data.json ui/main.slint`
renders it with fake `App` data. `row` is a reserved property name in Slint
components (grid placement), hence `item`.

## Licences

Settled (see Decisions and `README.md`). Keep in mind:

- code ported from CanFestival goes into `canopen-dictator-canfestival` only, with an
  `LGPL-2.1-or-later` SPDX header; `canopen-dictator-core` and `canopen-dictator-canopennode`
  must not depend on it;
- importers and exporters for CANopenEditor formats (XDD, its EDS and
  Markdown layout) are written from the file format and sample files, never
  translated from libEDSsharp's C# source (GPL), so they stay MIT/Apache;
- if the app ever targets an embedded device, the Slint royalty-free licence
  does not cover that.
