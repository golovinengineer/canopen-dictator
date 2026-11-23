// SPDX-License-Identifier: LGPL-2.1-or-later
// Part of the Rust port of CanFestival objdictgen; see lib.rs for copyright.

//! CanFestival's built-in DS-301 mapping (`MappingDictionary` in `node.py`).
//!
//! For indexes found here CanFestival ignores the names, types and access
//! rights of the imported file and uses this table instead.
//!
// TODO: device profiles (config/*.prf, e.g. DS-401/DS-402) and DS-302.

/// Structure flags (`OD_*` in node.py).
pub const OD_SUBINDEX: u8 = 1;
pub const OD_MULTIPLE_SUBINDEXES: u8 = 2;
pub const OD_IDENTICAL_SUBINDEXES: u8 = 4;
pub const OD_IDENTICAL_INDEXES: u8 = 8;

pub const VAR: u8 = OD_SUBINDEX;
/// Named sub-entries (what CiA calls a RECORD).
pub const ARRAY: u8 = OD_SUBINDEX | OD_MULTIPLE_SUBINDEXES;
/// Identical sub-entries (what CiA calls an ARRAY).
pub const REC: u8 = OD_SUBINDEX | OD_MULTIPLE_SUBINDEXES | OD_IDENTICAL_SUBINDEXES;
pub const PLURIARRAY: u8 = ARRAY | OD_IDENTICAL_INDEXES;
pub const PLURIREC: u8 = REC | OD_IDENTICAL_INDEXES;

/// Default used when the dictionary gives no value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Default {
    /// Default of the data type (0 or empty string).
    Type,
    /// `$NODEID + offset`.
    NodeId(u32),
    /// RPDO COB-ID: `$NODEID + 0x200 + 0x100*n` for the first four, else disabled.
    RpdoCobId,
    /// TPDO COB-ID: `$NODEID + 0x180 + 0x100*n` for the first four, else disabled.
    TpdoCobId,
}

#[derive(Debug, Clone, Copy)]
pub struct SubInfo {
    /// May end in a `%d[(expr)]` format suffix, see [`format_name`].
    pub name: &'static str,
    /// CiA data type index.
    pub ty: u16,
    pub access: &'static str,
    /// Number of consecutive subindexes this description covers.
    pub nbmax: Option<u32>,
    pub default: Default,
    /// May be mapped into a PDO.
    pub pdo: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct EntryInfo {
    pub index: u16,
    pub name: &'static str,
    pub structure: u8,
    pub callback: bool,
    /// For `OD_IDENTICAL_INDEXES`: how many consecutive indexes share this.
    pub nbmax: u32,
    pub subs: &'static [SubInfo],
}

const fn s(name: &'static str, ty: u16, access: &'static str) -> SubInfo {
    SubInfo {
        name,
        ty,
        access,
        nbmax: None,
        default: Default::Type,
        pdo: false,
    }
}
const fn sn(name: &'static str, ty: u16, access: &'static str, nbmax: u32) -> SubInfo {
    SubInfo {
        name,
        ty,
        access,
        nbmax: Some(nbmax),
        default: Default::Type,
        pdo: false,
    }
}
const fn sd(name: &'static str, ty: u16, access: &'static str, default: Default) -> SubInfo {
    SubInfo {
        name,
        ty,
        access,
        nbmax: None,
        default,
        pdo: false,
    }
}
/// PDO-mappable.
const fn sp(name: &'static str, ty: u16, access: &'static str) -> SubInfo {
    SubInfo {
        name,
        ty,
        access,
        nbmax: None,
        default: Default::Type,
        pdo: true,
    }
}
const fn e(index: u16, name: &'static str, structure: u8, subs: &'static [SubInfo]) -> EntryInfo {
    EntryInfo {
        index,
        name,
        structure,
        callback: false,
        nbmax: 1,
        subs,
    }
}
const fn cb(mut entry: EntryInfo) -> EntryInfo {
    entry.callback = true;
    entry
}
const fn pluri(mut entry: EntryInfo, nbmax: u32) -> EntryInfo {
    entry.nbmax = nbmax;
    entry
}

const PDO_PARAM_RX: &[SubInfo] = &[
    s("Highest SubIndex Supported", 0x05, "ro"),
    sd("COB ID used by PDO", 0x07, "rw", Default::RpdoCobId),
    s("Transmission Type", 0x05, "rw"),
    s("Inhibit Time", 0x06, "rw"),
    s("Compatibility Entry", 0x05, "rw"),
    s("Event Timer", 0x06, "rw"),
    s("SYNC start value", 0x05, "rw"),
];
const PDO_PARAM_TX: &[SubInfo] = &[
    s("Highest SubIndex Supported", 0x05, "ro"),
    sd("COB ID used by PDO", 0x07, "rw", Default::TpdoCobId),
    s("Transmission Type", 0x05, "rw"),
    s("Inhibit Time", 0x06, "rw"),
    s("Compatibility Entry", 0x05, "rw"),
    s("Event Timer", 0x06, "rw"),
    s("SYNC start value", 0x05, "rw"),
];

pub const MAPPING: &[EntryInfo] = &[
    e(0x1000, "Device Type", VAR, &[s("Device Type", 0x07, "ro")]),
    e(
        0x1001,
        "Error Register",
        VAR,
        &[sp("Error Register", 0x05, "ro")],
    ),
    e(
        0x1002,
        "Manufacturer Status Register",
        VAR,
        &[sp("Manufacturer Status Register", 0x07, "ro")],
    ),
    cb(e(
        0x1003,
        "Pre-defined Error Field",
        REC,
        &[
            s("Number of Errors", 0x05, "rw"),
            sn("Standard Error Field", 0x07, "ro", 0xFE),
        ],
    )),
    cb(e(
        0x1005,
        "SYNC COB ID",
        VAR,
        &[s("SYNC COB ID", 0x07, "rw")],
    )),
    cb(e(
        0x1006,
        "Communication / Cycle Period",
        VAR,
        &[s("Communication Cycle Period", 0x07, "rw")],
    )),
    e(
        0x1007,
        "Synchronous Window Length",
        VAR,
        &[s("Synchronous Window Length", 0x07, "rw")],
    ),
    e(
        0x1008,
        "Manufacturer Device Name",
        VAR,
        &[s("Manufacturer Device Name", 0x09, "const")],
    ),
    e(
        0x1009,
        "Manufacturer Hardware Version",
        VAR,
        &[s("Manufacturer Hardware Version", 0x09, "const")],
    ),
    e(
        0x100A,
        "Manufacturer Software Version",
        VAR,
        &[s("Manufacturer Software Version", 0x09, "const")],
    ),
    e(0x100C, "Guard Time", VAR, &[s("Guard Time", 0x06, "rw")]),
    e(
        0x100D,
        "Life Time Factor",
        VAR,
        &[s("Life Time Factor", 0x05, "rw")],
    ),
    e(
        0x1010,
        "Store parameters",
        ARRAY,
        &[
            s("Number of Entries", 0x05, "ro"),
            s("Save All Parameters", 0x07, "rw"),
            s("Save Communication Parameters", 0x07, "rw"),
            s("Save Application Parameters", 0x07, "rw"),
            sn(
                "Save Manufacturer Parameters %d[(sub - 3)]",
                0x07,
                "rw",
                0x7C,
            ),
        ],
    ),
    e(
        0x1011,
        "Restore Default Parameters",
        ARRAY,
        &[
            s("Number of Entries", 0x05, "ro"),
            s("Restore All Default Parameters", 0x07, "rw"),
            s("Restore Communication Default Parameters", 0x07, "rw"),
            s("Restore Application Default Parameters", 0x07, "rw"),
            sn(
                "Restore Manufacturer Defined Default Parameters %d[(sub - 3)]",
                0x07,
                "rw",
                0x7C,
            ),
        ],
    ),
    e(0x1012, "TIME COB ID", VAR, &[s("TIME COB ID", 0x07, "rw")]),
    e(
        0x1013,
        "High Resolution Timestamp",
        VAR,
        &[sp("High Resolution Time Stamp", 0x07, "rw")],
    ),
    e(
        0x1014,
        "Emergency COB ID",
        VAR,
        &[sd("Emergency COB ID", 0x07, "rw", Default::NodeId(0x80))],
    ),
    e(
        0x1015,
        "Inhibit Time Emergency",
        VAR,
        &[s("Inhibit Time Emergency", 0x06, "rw")],
    ),
    e(
        0x1016,
        "Consumer Heartbeat Time",
        REC,
        &[
            s("Number of Entries", 0x05, "ro"),
            sn("Consumer Heartbeat Time", 0x07, "rw", 0x7F),
        ],
    ),
    cb(e(
        0x1017,
        "Producer Heartbeat Time",
        VAR,
        &[s("Producer Heartbeat Time", 0x06, "rw")],
    )),
    e(
        0x1018,
        "Identity",
        ARRAY,
        &[
            s("Number of Entries", 0x05, "ro"),
            s("Vendor ID", 0x07, "ro"),
            s("Product Code", 0x07, "ro"),
            s("Revision Number", 0x07, "ro"),
            s("Serial Number", 0x07, "ro"),
        ],
    ),
    e(
        0x1019,
        "Synchronous counter overflow value",
        VAR,
        &[s("Synchronous counter overflow value", 0x05, "rw")],
    ),
    e(
        0x1020,
        "Verify Configuration",
        ARRAY,
        &[
            s("Number of Entries", 0x05, "ro"),
            s("Configuration Date", 0x07, "rw"),
            s("Configuration Time", 0x07, "rw"),
        ],
    ),
    e(
        0x1023,
        "OS Command",
        ARRAY,
        &[
            s("Number of Entries", 0x05, "ro"),
            s("Command", 0x0A, "rw"),
            s("Status", 0x05, "ro"),
            s("Reply", 0x0A, "ro"),
        ],
    ),
    e(
        0x1024,
        "OS Command Mode",
        VAR,
        &[s("OS Command Mode", 0x05, "wo")],
    ),
    e(
        0x1025,
        "OS Debugger Interface",
        ARRAY,
        &[
            s("Number of Entries", 0x05, "ro"),
            s("Command", 0x0A, "rw"),
            s("Status", 0x05, "ro"),
            s("Reply", 0x0A, "ro"),
        ],
    ),
    e(
        0x1026,
        "OS Prompt",
        ARRAY,
        &[
            s("Number of Entries", 0x05, "ro"),
            sp("StdIn", 0x05, "wo"),
            sp("StdOut", 0x05, "ro"),
            sp("StdErr", 0x05, "ro"),
        ],
    ),
    e(
        0x1027,
        "Module List",
        REC,
        &[
            s("Number of Connected Modules", 0x05, "ro"),
            sn("Module %d[(sub)]", 0x06, "ro", 0xFE),
        ],
    ),
    e(
        0x1028,
        "Emergency Consumer",
        REC,
        &[
            s("Number of Consumed Emergency Objects", 0x05, "ro"),
            sn("Emergency Consumer", 0x07, "rw", 0x7F),
        ],
    ),
    e(
        0x1029,
        "Error Behavior",
        ARRAY,
        &[
            s("Number of Error Classes", 0x05, "ro"),
            s("Communication Error", 0x05, "rw"),
            sn("Device Profile", 0x05, "rw", 0xFE),
        ],
    ),
    e(
        0x1200,
        "Server SDO Parameter",
        ARRAY,
        &[
            s("Number of Entries", 0x05, "ro"),
            sd(
                "COB ID Client to Server (Receive SDO)",
                0x07,
                "ro",
                Default::NodeId(0x600),
            ),
            sd(
                "COB ID Server to Client (Transmit SDO)",
                0x07,
                "ro",
                Default::NodeId(0x580),
            ),
        ],
    ),
    pluri(
        e(
            0x1201,
            "Additional Server SDO %d Parameter[(idx)]",
            PLURIARRAY,
            &[
                s("Number of Entries", 0x05, "ro"),
                s("COB ID Client to Server (Receive SDO)", 0x07, "ro"),
                s("COB ID Server to Client (Transmit SDO)", 0x07, "ro"),
                s("Node ID of the SDO Client", 0x05, "ro"),
            ],
        ),
        0x7F,
    ),
    pluri(
        e(
            0x1280,
            "Client SDO %d Parameter[(idx)]",
            PLURIARRAY,
            &[
                s("Number of Entries", 0x05, "ro"),
                s("COB ID Client to Server (Transmit SDO)", 0x07, "rw"),
                s("COB ID Server to Client (Receive SDO)", 0x07, "rw"),
                s("Node ID of the SDO Server", 0x05, "rw"),
            ],
        ),
        0x100,
    ),
    pluri(
        e(
            0x1400,
            "Receive PDO %d Parameter[(idx)]",
            PLURIARRAY,
            PDO_PARAM_RX,
        ),
        0x200,
    ),
    pluri(
        e(
            0x1600,
            "Receive PDO %d Mapping[(idx)]",
            PLURIREC,
            &[
                s("Number of Entries", 0x05, "rw"),
                sn(
                    "PDO %d Mapping for an application object %d[(idx,sub)]",
                    0x07,
                    "rw",
                    0x40,
                ),
            ],
        ),
        0x200,
    ),
    cb(pluri(
        e(
            0x1800,
            "Transmit PDO %d Parameter[(idx)]",
            PLURIARRAY,
            PDO_PARAM_TX,
        ),
        0x200,
    )),
    pluri(
        e(
            0x1A00,
            "Transmit PDO %d Mapping[(idx)]",
            PLURIREC,
            &[
                s("Number of Entries", 0x05, "rw"),
                sn(
                    "PDO %d Mapping for a process data variable %d[(idx,sub)]",
                    0x07,
                    "rw",
                    0x40,
                ),
            ],
        ),
        0x200,
    ),
];

/// `FindIndex`: the table entry describing `index`, if any.
pub fn find(index: u16) -> Option<&'static EntryInfo> {
    if let Some(entry) = MAPPING.iter().find(|m| m.index == index) {
        return Some(entry);
    }
    MAPPING.iter().find(|m| {
        m.structure & OD_IDENTICAL_INDEXES != 0
            && (m.index as u32) < index as u32
            && (index as u32) < m.index as u32 + m.nbmax
    })
}

/// `StringFormat`: expand a `text %d[(idx,sub)]` name template.
pub fn format_name(template: &str, idx: u32, sub: i64) -> String {
    let Some(open) = template.rfind('[') else {
        return template.to_string();
    };
    if !template.ends_with(']') {
        return template.to_string();
    }
    let (text, args) = (&template[..open], &template[open + 1..template.len() - 1]);
    let args = args
        .strip_prefix('(')
        .and_then(|a| a.strip_suffix(')'))
        .unwrap_or(args);
    let mut values = args
        .split(',')
        .map(|a| eval_expr(a, idx as i64, sub).unwrap_or(0));
    let mut out = String::new();
    let mut rest = text;
    while let Some(pos) = rest.find("%d") {
        out.push_str(&rest[..pos]);
        out.push_str(&values.next().unwrap_or(0).to_string());
        rest = &rest[pos + 2..];
    }
    out.push_str(rest);
    out
}

/// Evaluate the arithmetic in a name template: integers, `idx`, `sub`,
/// `+ - * /` and parentheses (Python evaluates it with `eval`).
fn eval_expr(text: &str, idx: i64, sub: i64) -> Option<i64> {
    struct P<'a> {
        s: &'a [u8],
        pos: usize,
        idx: i64,
        sub: i64,
    }
    impl P<'_> {
        fn skip(&mut self) {
            while self
                .s
                .get(self.pos)
                .is_some_and(|c| c.is_ascii_whitespace())
            {
                self.pos += 1;
            }
        }
        fn peek(&mut self) -> Option<u8> {
            self.skip();
            self.s.get(self.pos).copied()
        }
        fn expr(&mut self) -> Option<i64> {
            let mut v = self.term()?;
            while let Some(op @ (b'+' | b'-')) = self.peek() {
                self.pos += 1;
                let r = self.term()?;
                v = if op == b'+' { v + r } else { v - r };
            }
            Some(v)
        }
        fn term(&mut self) -> Option<i64> {
            let mut v = self.factor()?;
            while let Some(op @ (b'*' | b'/')) = self.peek() {
                self.pos += 1;
                let r = self.factor()?;
                v = if op == b'*' { v * r } else { v.checked_div(r)? };
            }
            Some(v)
        }
        fn factor(&mut self) -> Option<i64> {
            match self.peek()? {
                b'(' => {
                    self.pos += 1;
                    let v = self.expr()?;
                    (self.peek()? == b')').then(|| self.pos += 1)?;
                    Some(v)
                }
                b'-' => {
                    self.pos += 1;
                    Some(-self.factor()?)
                }
                c if c.is_ascii_digit() => {
                    let start = self.pos;
                    while self
                        .s
                        .get(self.pos)
                        .is_some_and(|c| c.is_ascii_alphanumeric())
                    {
                        self.pos += 1;
                    }
                    let t = std::str::from_utf8(&self.s[start..self.pos]).ok()?;
                    match t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
                        Some(h) => i64::from_str_radix(h, 16).ok(),
                        None => t.parse().ok(),
                    }
                }
                _ => {
                    let rest = &self.s[self.pos..];
                    let (v, n) = if rest.starts_with(b"idx") {
                        (self.idx, 3)
                    } else if rest.starts_with(b"sub") {
                        (self.sub, 3)
                    } else {
                        return None;
                    };
                    self.pos += n;
                    Some(v)
                }
            }
        }
    }
    let mut p = P {
        s: text.as_bytes(),
        pos: 0,
        idx,
        sub,
    };
    let v = p.expr()?;
    (p.peek().is_none()).then_some(v)
}

/// `FindEntryName` with the index number filled in.
pub fn entry_name(info: &EntryInfo, index: u16) -> String {
    if info.structure & OD_IDENTICAL_INDEXES != 0 {
        format_name(info.name, (index - info.index) as u32 + 1, 0)
    } else {
        info.name.to_string()
    }
}

/// `FindSubentryInfos` (without the name formatting).
pub fn sub_info(info: &EntryInfo, sub: u8) -> Option<&'static SubInfo> {
    let sub = sub as u32;
    if info.structure & OD_IDENTICAL_SUBINDEXES != 0 {
        if sub == 0 {
            info.subs.first()
        } else {
            info.subs.get(1).filter(|s| sub <= s.nbmax.unwrap_or(0))
        }
    } else if info.structure & OD_MULTIPLE_SUBINDEXES != 0 {
        let mut idx = 0;
        for s in info.subs {
            match s.nbmax {
                Some(n) => {
                    if idx <= sub && sub < idx + n {
                        return Some(s);
                    }
                    idx += n;
                }
                None => {
                    if sub == idx {
                        return Some(s);
                    }
                    idx += 1;
                }
            }
        }
        None
    } else if sub == 0 {
        info.subs.first()
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_formatted_like_canfestival() {
        let tx = find(0x1802).unwrap();
        assert_eq!(entry_name(tx, 0x1802), "Transmit PDO 3 Parameter");
        let map = find(0x1A01).unwrap();
        let s = sub_info(map, 3).unwrap();
        assert_eq!(
            format_name(s.name, 2, 3),
            "PDO 2 Mapping for a process data variable 3"
        );
        let store = find(0x1010).unwrap();
        assert_eq!(
            format_name(sub_info(store, 4).unwrap().name, 1, 4),
            "Save Manufacturer Parameters 1"
        );
        assert!(find(0x1021).is_none());
        assert_eq!(find(0x1200).unwrap().index, 0x1200);
        assert_eq!(find(0x1201).unwrap().index, 0x1201);
    }

    #[test]
    fn name_expressions() {
        assert_eq!(format_name("Out %d[(sub - 1)]", 1, 5), "Out 4");
        assert_eq!(format_name("Ch %d.%d[(idx*2, sub+0x10)]", 3, 1), "Ch 6.17");
        assert_eq!(format_name("Plain", 1, 1), "Plain");
    }
}
