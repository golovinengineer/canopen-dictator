# Example object dictionary (test assets)

A fictional device ("Example Vendor" / "Example Device") that uses most of
what CANopenEditor and CanFestival can express. The golden tests compare
Dictator's importers, exporters and generators against what the original
tools produce from it. No real project data.

## Files

| File | Made by | From |
| --- | --- | --- |
| `example.xdd` | CANopenEditor v4.2.3 | the source; edit it in CANopenEditor |
| `example.eds` | CANopenEditor | `example.xdd` |
| `example.md` | CANopenEditor (documentation export) | `example.xdd` |
| `OD.c`, `OD.h` | CANopenEditor (CANopenNode v4 exporter) | `example.xdd` |
| `example.od` | CanFestival objdictedit, via `canfestival_chain.py` | `example.eds` |
| `ObjDict.c`, `ObjDict.h` | CanFestival gen_cfile, via `canfestival_chain.py` | `example.od` |
| `example.objdict.toml` | Dictator | `example.xdd` plus the CanFestival settings below |

## Regenerating

After changing `example.xdd` (CANopenEditor's command line runs on macOS
with `dotnet`; the GUI export menu gives the same files):

```sh
cd assets/example
E="dotnet ~/tools/CANopenEditor-v4/net8.0/EDSSharp.dll"
$E --infile example.xdd --outfile example.eds
$E --infile example.xdd --outfile OD.c --type CanOpenNodeV4
$E --infile example.xdd --outfile example.md --type DocumentationMarkup
container exec co_stack_gen python2.7 /workdir/assets/example/canfestival_chain.py \
    /workdir/assets/example/example.eds /workdir/assets/example/example.od \
    /workdir/assets/example/ObjDict.c
```

Then re-import `example.objdict.toml` with Dictator and set the CanFestival
settings again (the test `checked_in_sample_project_matches_xdd` says
which).

## What it covers

- every basic data type as a VAR (0x2000-0x200E), every access kind
  (`ro`, `wo`, `rw`, `rwr`, `rww`, `const`), PDO mapping `no` / `optional` /
  `tpdo` / `rpdo`, limits, `$NODEID` formulas in communication and
  manufacturer objects
- ARRAYs, RECORDs with holes (CANopenNode v4 PDO parameters, 0x2101),
  RPDO/TPDO mappings including a dummy entry, disabled objects (0x1013,
  0x1029, 0x2FFF)
- CANopenNode properties: storage groups (incl. a custom `PERSIST_APP`),
  count labels, `CO_flagsPDO`, `CO_stringLengthMin`, LSS slave, dummy usage
- multi-line descriptions with markup characters, quotes and a non-ASCII
  character

CanFestival side, set in `canfestival_chain.py` as a user would in
objdictedit: node name `ExampleDevice`, callbacks on 0x2000, 0x2006, 0x2101,
0x2102, 0x6200, save flags on 0x2012 and 0x2102 subs 1-3. Left out there:
0x200D (CanFestival has no UNICODE_STRING) and 0x200E (gen_cfile refuses a
mapped DOMAIN without an initial value). The octet string 0x200C is set back
to `01 02 03 04` because CanFestival's EDS parser reads CANopenEditor's
`01020304` as an octal number.

## Tool behaviour found while making these

- CANopenEditor writes OCTET_STRING and DOMAIN alike as `BITSTRING` in XDD
  and tells them apart by the default value (with one: OCTET_STRING).
- In EDS it writes the octet string default without spaces, and
  `;TPDODetectCos=1` only on VARs.
- Its documentation export shows only `type=` for RPDOs, and `(len=N)` after
  string types with `CO_stringLengthMin`. Run under .NET 8 on macOS, it puts
  a narrow no-break space (U+202F) before AM/PM.
- gen_cfile writes negative numbers as `0x-64` (not valid C) and a doubled
  `const CONSTSTORE`; the tests undo both in the reference before comparing.
